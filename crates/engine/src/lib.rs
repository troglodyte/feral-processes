pub mod abilities;
pub mod balance;
pub mod battle;
pub mod components;
pub mod difficulty;
mod game;
pub mod items;
pub mod items_db;
pub mod perks;
pub mod progression;
pub mod research;
pub mod resources;
pub mod save;
pub mod species;
pub mod structures;
pub mod systems;
pub mod taming;
pub mod views;
pub mod world;

use std::collections::HashMap;
use std::path::Path;

pub use bevy_ecs::prelude::Entity;
use bevy_ecs::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use abilities::AbilityDb;
use battle::{
    ActionKind, ActionOption, AllyOption, BattleAction, EnemyGroup, PartyCommand, PartyCommandKind,
    SpecialOption, TargetSpec,
};
use components::{
    ActiveBuff, ActiveStatus, BuffKind, CombatBuff, Creature, CustomName, Decompiler, Durability,
    Equipment, EquippedItem, Experience, FusionCount, Glyph, GlyphColor, Hostile, Inventory,
    ItemFusions, MAX_INDIVIDUAL_ROLL, MIN_INDIVIDUAL_ROLL, NEED_MAX, Needs, Nest, NestGuardian,
    PassiveProcessor, Perks, Player, Position, Potential, ResourceNode, Stats, StatusEffects,
    StatusKind, Structure, StructureTier, Tamed, Task, TaskKind, Temporary, WanderAi, ZonePortal,
};
use items::{EquipmentSlot, EquipmentStats, ItemId, ids};
use items_db::ItemDb;
pub use perks::Perk;
use research::{ResearchDb, ResearchDef};
pub use research::{ResearchId, ResearchRecipe};
use resources::{
    BASE_PET_CAPACITY, BattleState, EffectQueue, GameClock, GameOver, GameRng, MAX_PARTY_SIZE,
    MessageLog, Party, Platform, PlayerEntity, Research, ZoneLevel, ZoneSpawnPoint,
};
pub use resources::{DifficultyMode, EffectKind, MessageKind, VisualEffect};
use species::{MoveDef, SpecialAbility, SpeciesDb, SpeciesDef, SpeciesId};
use structures::{StructureDb, StructureDef, StructureId, TradeDef};
pub use views::*;
use world::{Biome, Tile, WorldMap};

/// How many ticks a full night's recharge cycle advances the clock by.
const REST_TICKS: u32 = 40;

/// Relative weight each party member carries in a wild program's target
/// roll. Ranks are *soft*: everyone stays targetable, slot order only
/// changes the odds — a back-slot member is hit
/// `FRONT_SLOT_AGGRO_WEIGHT / BACK_SLOT_AGGRO_WEIGHT` times less often than
/// a front-slot one, never zero times. Bracing (see `Game::begin_defend`)
/// adds `DEFEND_AGGRO_WEIGHT` on top, which is what makes Defend a
/// party-level play rather than a selfish one.
const FRONT_SLOT_AGGRO_WEIGHT: u32 = 3;
const BACK_SLOT_AGGRO_WEIGHT: u32 = 1;
const DEFEND_AGGRO_WEIGHT: u32 = 4;

/// How many party slots count as the front line for `FRONT_SLOT_AGGRO_WEIGHT`
/// — the player plus the first two companions.
const FRONT_SLOTS: usize = 3;

/// Tile distance per step of `DISTANCE_STAT_STEP_BONUS`, counted from
/// `Game::distance_from_danger_origin` — the base platform's edge once a
/// Home exists, `ZoneSpawnPoint` before then. See
/// `Game::distance_stat_multiplier`.
const DISTANCE_STAT_STEP_TILES: i32 = 15;

/// Stat growth added per `DISTANCE_STAT_STEP_TILES` step away from the
/// zone's spawn point, on top of `ZoneLevel::stat_multiplier` — a gentler,
/// linear (not doubling) knob than zone depth, since it's optional
/// distance covered within a zone you can always retreat from, not a
/// one-way commitment like breaching deeper.
const DISTANCE_STAT_STEP_BONUS: f32 = 0.25;

/// Cap on `distance_stat_multiplier`, so wandering far enough doesn't
/// scale stats forever within a single zone — unlike zone depth, which
/// really is unbounded.
const MAX_DISTANCE_STAT_MULTIPLIER: f32 = 3.0;

/// How far from the player a zone's opening wild programs scatter (see
/// `Game::spawn_initial_creatures`). Widened by the platform radius when
/// the player has a base, since nothing can spawn on platform floor.
const INITIAL_SPAWN_SCATTER_TILES: i32 = 15;

/// Tile distance per extra pack member a wild spawn can roll, counted from
/// the same origin as `DISTANCE_STAT_STEP_TILES` (the platform's edge once
/// a Home exists) — see `Game::max_pack_size`. Twice `DISTANCE_STAT_STEP_TILES`:
/// packs grow into their zone's cap more gradually than per-creature stats
/// do.
const PACK_SIZE_STEP_TILES: i32 = DISTANCE_STAT_STEP_TILES * 2;

/// How tightly a pack's members cluster around the tile a spawn roll
/// picked (`Game::try_spawn_habitat_creature`), and how far `gather_pack`
/// searches from whichever member the player bumped into — both use the
/// same radius so a whole spawned cluster is guaranteed to pull together
/// into one fight.
const PACK_GATHER_RADIUS: i32 = 3;

/// Pack-size headroom each zone level unlocks, against `MAX_PACK_SIZE`.
/// Packs fight as species groups now, so a big pack is several small groups
/// with only the front two in melee range (`ENGAGED_GROUPS`) rather than a
/// flat multiplier on incoming damage.
const PACK_SIZE_PER_ZONE: u32 = 3;

/// Hard ceiling on one intrusion's wild pack, across every group.
const MAX_PACK_SIZE: u32 = 12;

/// How many distinct species groups can engage in one intrusion. A cluster
/// with more species than this engages its largest groups and leaves the
/// remainder standing on the map as ordinary hostiles — they're met on the
/// next bump rather than silently despawned.
const MAX_ENEMY_GROUPS: usize = 4;

/// How many enemy groups are in melee range of the party. Groups past this
/// index can only act with a move flagged `ranged`, which is what keeps a
/// four-group pack from simply quadrupling incoming damage — and what makes
/// wiping the front group a real decision, since it promotes a back group
/// into reach.
const ENGAGED_GROUPS: usize = 2;

/// How many `Hostile` creatures may exist across the whole map at once.
/// Wild creatures never despawn on their own, so without a bound the
/// world-wide population — and the per-tick AI cost of simulating it —
/// grows all session. Rather than blocking new spawns once the cap is
/// reached (which would let a population the player wandered away from
/// permanently starve the area they're actually in), reaching it culls
/// the `Hostile` farthest from the player to free a slot — see
/// `Game::maybe_spawn_wild_creature`. Tamed programs never count here at
/// all; they shouldn't crowd out wild spawns just by existing.
const WILD_CREATURE_CAP: usize = 100;

/// Initiative baseline for a species whose `.ron` file omits `base_speed` —
/// the midpoint of the shipped roster's range, so an un-annotated mod
/// species is neither free initiative nor dead weight.
pub(crate) const DEFAULT_BASE_SPEED: i32 = 10;

/// The player's initiative baseline. A shade above `DEFAULT_BASE_SPEED`: the
/// player acts first against an average opponent, but loses the roll to
/// anything genuinely fast.
const PLAYER_BASE_SPEED: i32 = 11;

/// Each round every combatant rolls `base_speed + rng(0..=INITIATIVE_DIE)`
/// and acts in descending order. Sized so a 4-point speed gap still loses
/// the roll sometimes — order should be a tendency, not a lookup table.
const INITIATIVE_DIE: i32 = 10;

/// Move power behind the player's own basic strike. The player has no
/// `Creature` component and so no species moveset — this is their one move,
/// with `Stats::atk` and equipment carrying the rest of the scaling.
const PLAYER_STRIKE_POWER: i32 = 5;

/// DEF granted for the round by the Defend action.
const DEFEND_DEF_BONUS: i32 = 6;

/// Battle rounds a companion's default rally buff (see
/// `Game::rally_player`) lasts when its species defines no
/// `special_ability`.
const RALLY_DURATION: u32 = 3;

/// Fatigue the player spends each time they command a companion in battle
/// (see `BattleAction::Special`) — the rally/special-ability
/// bonus isn't free, whichever kind the companion has.
const COMPANION_COMMAND_FATIGUE_COST: f32 = 5.0;

/// Longest name a player can give a fused program (see
/// `Game::fuse_companions`) — enforced by truncation, not rejection, so a
/// too-long name just gets shortened rather than failing the fusion.
pub const MAX_CUSTOM_NAME_LEN: usize = 12;

/// How many fusions deep a program's lineage may go before it's a
/// finished product (see `components::FusionCount`). A program at this
/// depth can't be fed into another fusion at all, so the stat-compounding
/// `fuse_stat` gives is bounded instead of being an endless duplicate
/// laundry.
pub const MAX_FUSIONS: u32 = 3;

/// How much the player's `Decompiler` skill grows per level gained.
const DECOMPILER_SKILL_PER_LEVEL: i32 = 1;

/// Perk Points (see `perks::Perk`) awarded per player level gained.
const PERK_POINTS_PER_LEVEL: u32 = 1;

/// Every party member (see `resources::Party`) gains `1 / PARTY_XP_DIVISOR`
/// of whatever XP the player just earned from a kill or successful
/// decompile — see `Game::award_party_xp`.
const PARTY_XP_DIVISOR: u32 = 2;

/// Bonus `Perk::KeenScavenger` adds to `Game::forage`'s success chance, per level.
const KEEN_SCAVENGER_BONUS_PER_LEVEL: f64 = 0.01;

/// `Perk::LowPowerMode`'s hunger-decay reduction, per level (the decay
/// multiplier is `1.0 - this * level`, floored at 0.0).
const LOW_POWER_MODE_REDUCTION_PER_LEVEL: f32 = 0.01;

/// Effective Decompiler skill `Perk::ExploitFocus` adds on top of the
/// player's real `Decompiler` stat, per level.
const EXPLOIT_FOCUS_BONUS_PER_LEVEL: i32 = 1;

/// Per-item discount `Perk::LeanCompiler` applies to `Game::craft` recipe
/// costs, per level (never below 1 each).
const LEAN_COMPILER_DISCOUNT_PER_LEVEL: u32 = 1;

/// Permanent ATK `Perk::Attacker` adds to the player's `Stats`, per level.
const ATTACKER_BONUS_PER_LEVEL: i32 = 1;

/// Permanent DEF `Perk::Defender` adds to the player's `Stats`, per level.
const DEFENDER_BONUS_PER_LEVEL: i32 = 1;

/// Percentage of current max Integrity `Perk::Buffer` adds to the
/// player's `Stats`, per level.
const BUFFER_BONUS_PERCENT_PER_LEVEL: f32 = 0.01;

/// Floor on `Perk::Buffer`'s per-level max Integrity bonus, so it's still
/// worth buying early when 1% of max Integrity would round to less than
/// this.
const BUFFER_MIN_BONUS_PER_LEVEL: i32 = 10;

/// Chance a defeated wild program additionally drops a Portal Fragment,
/// independent of its species' own `work_resource`/`equipment_drop`.
/// Fragments are the raw material for deploying a zone-portal structure
/// (see `StructureDef::zone_portal`).
const PORTAL_FRAGMENT_DROP_CHANCE: f64 = 0.35;

/// How much of a zone-portal structure's base `build_cost` is added to its
/// price per zone below the current one. Breaching deeper costs more, but
/// currency does not survive the trip (see `Game::enter_next_zone`), so
/// this is a ramp on a from-zero grind rather than a tax on a stockpile —
/// which is why it adds half the base rate per zone instead of doubling.
const ZONE_PORTAL_COST_GROWTH_PERCENT: u32 = 50;

/// The quantity a zone-portal structure costing `base_qty` of an item
/// charges at `zone`. Shared with `balance::ticks_to_afford_portal` so a
/// projection can't drift from the price the game actually charges.
pub(crate) fn zone_portal_cost(base_qty: u32, zone: u32) -> u32 {
    base_qty + base_qty * ZONE_PORTAL_COST_GROWTH_PERCENT * zone.saturating_sub(1) / 100
}

/// Chance a habitat spawn roll (see `Game::try_spawn_habitat_creature`)
/// picks a boss species instead of an ordinary one, when the tile's biome
/// has at least one boss defined for it.
const BOSS_SPAWN_CHANCE: f64 = 0.04;

/// Range of Portal Fragments a defeated boss guarantees, replacing the
/// flat `PORTAL_FRAGMENT_DROP_CHANCE` roll every other species gets.
const BOSS_PORTAL_FRAGMENT_DROP: std::ops::RangeInclusive<u32> = 3..=6;

/// Chance a habitat spawn roll (see `Game::try_spawn_habitat_creature`)
/// produces a Nest instead of an ordinary pack, for a species that has
/// `SpeciesDef::can_nest` set. Only rolled at all when `can_nest` is
/// true, mirroring how `BOSS_SPAWN_CHANCE` is only rolled when a boss
/// candidate exists — keeps the extra RNG draw out of the common
/// non-nesting path entirely.
const NEST_SPAWN_CHANCE: f64 = 0.06;

/// Chebyshev distance a `NestGuardian` may wander from its `Nest` — see
/// `systems::wander_ai_system`. `pub(crate)` so `systems.rs` (a sibling
/// module) can read it too.
pub(crate) const NEST_TETHER_RADIUS: i32 = 5;

/// Inclusive range of guardians a freshly spawned `Nest` starts with —
/// see `Game::spawn_nest`.
const NEST_GUARDIAN_MIN: u32 = 2;
const NEST_GUARDIAN_MAX: u32 = 5;

/// Ticks between a guardian's death/taming and its replacement spawning
/// — see `Game::nest_respawn_tick`.
const NEST_RESPAWN_TICKS: u32 = 10;

/// A Nest's starting/max `Durability` — double the default structure
/// durability (`structures::default_durability`), since it's meant to
/// take real, sustained effort to clear, not a single lucky hit.
const NEST_DURABILITY: u32 = 60;

/// Thresholds for `difficulty_color`'s old-school "con" coloring, as
/// upper bounds on a hostile program's power (see `Stats::power`) relative
/// to the player's own — anything at or under `DIFFICULTY_EASY_MAX` reads
/// Green, up through `DIFFICULTY_EVEN_MAX` reads Yellow, up through
/// `DIFFICULTY_TOUGH_MAX` reads Orange, and anything above that reads Red.
const DIFFICULTY_EASY_MAX: f64 = 0.7;
const DIFFICULTY_EVEN_MAX: f64 = 1.1;
const DIFFICULTY_TOUGH_MAX: f64 = 1.6;

/// Chance per tick (see `Game::raid_check`) that a random deployed
/// structure comes under raid, if any exist.
const RAID_CHANCE_PER_TICK: f64 = 0.012;

/// Damage a raid deals to a structure's `Durability` when it has no
/// assigned cronjob worker defending it. Deliberately small relative to
/// `structures::default_durability` (30): a raid is meant to be attrition
/// the base can recover from, not a three-hit countdown to losing the
/// structure outright.
const RAID_DAMAGE: u32 = 4;

/// Damage a defending cronjob worker takes fending off a raid on its
/// structure — win or lose, defending has a cost. The raid's damage to the
/// structure itself is reduced by the worker's Defense stat instead
/// (`RAID_DAMAGE.saturating_sub(worker_def)`).
const RAID_DEFENDER_DAMAGE: i32 = 6;

/// `StructureDef::id` of the one structure `Game::place_structure` will
/// let you deploy before any other — everything else requires a Home
/// already standing somewhere. Also what pins the build menu's ordering
/// (see `StructureDb::all`).
const HOME_STRUCTURE_ID: &str = "home";

/// Every non-Home structure must be deployed within this many tiles (per
/// axis, same box-radius style as `StructureDef::passive_process`'s
/// `radius`) of the Home structure — a base clusters around its Home
/// rather than sprawling across the map.
const MAX_BUILD_DISTANCE_FROM_HOME: i32 = 7;

/// Fraction of a structure's current build cost refunded when it's removed
/// (see `Game::remove_structure`), rounded down per item. Applies uniformly
/// whether the structure is removed directly or swept up in a Home's
/// cascading removal.
const STRUCTURE_REMOVAL_REFUND_PERCENT: u32 = 30;

/// How often (in ticks) damaged structures passively regenerate — a slow
/// trickle, not a substitute for staying ahead of raids.
const STRUCTURE_REGEN_INTERVAL: u64 = 20;

/// How much `Durability` a damaged structure regenerates every
/// `STRUCTURE_REGEN_INTERVAL` ticks — set to match `RAID_DAMAGE` so one
/// interval fully undoes one raid. Below that, a base loses the attrition
/// race no matter how it's played.
const STRUCTURE_REGEN_AMOUNT: u32 = 4;

/// The entire public API surface the renderer talks to via app-core. Its
/// methods live in the `game` module, split by topic; the renderer never
/// touches the ECS `World` directly.
pub struct Game {
    world: World,
    schedule: Schedule,
}

#[cfg(test)]
mod tests;
