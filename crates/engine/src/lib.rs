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
mod tests {
    use super::*;
    use crate::game::inspection::difficulty_color;
    use crate::game::turn::forage_chance;
    use crate::game::zone::find_walkable_start;
    use std::path::Path;

    /// Mirror of the shipped starter-recipe costs in
    /// `assets/items/{ice_breaker,power_cell}.ron` — the recipes are
    /// data-driven now (see `Game::craft_recipes`), so these live here only
    /// to keep the compile/discount tests asserting against a known number.
    const ICE_BREAKER_CORE_COST: u32 = 3;
    const POWER_CELL_CORE_COST: u32 = 2;

    fn test_assets_dir() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets")
    }

    /// Plans `action` for the player and a plain attack for every
    /// companion, then resolves the round. The one-line stand-in for the
    /// old single-action entry points, for tests that care about what a
    /// round *does* rather than how it gets planned — the planning API has
    /// its own tests.
    fn resolve_round_with(game: &mut Game, action: BattleAction) {
        let slots = game
            .world
            .get_resource::<BattleState>()
            .map(|b| b.planned.len())
            .unwrap_or(0);
        if game.battle_set_action(0, action).is_err() {
            return;
        }
        for slot in 1..slots {
            let _ = game.battle_set_action(slot, BattleAction::Attack { group: 0 });
        }
        game.battle_resolve_round();
    }

    fn player_attacks(game: &mut Game) {
        resolve_round_with(game, BattleAction::Attack { group: 0 });
    }

    fn player_decompiles(game: &mut Game) {
        resolve_round_with(game, BattleAction::Decompile { group: 0 });
    }

    /// Resolves a round in which `companion` uses its Special (the rally or
    /// species ability that commanding it used to trigger) and everyone
    /// else braces. Defend deals no damage, so anything that happens to the
    /// enemy in such a round is attributable to the Special alone.
    fn companion_uses_special(
        game: &mut Game,
        companion: Entity,
        ability: usize,
        target: battle::SpecialTarget,
    ) {
        let slot = game
            .world
            .resource::<Party>()
            .0
            .iter()
            .position(|&e| e == companion)
            .map(|i| i + 1);
        let Some(slot) = slot else {
            return;
        };
        let slots = game
            .world
            .get_resource::<BattleState>()
            .map(|b| b.planned.len())
            .unwrap_or(0);
        for other in 0..slots {
            let action = if other == slot {
                BattleAction::Special { ability, target }
            } else {
                BattleAction::Defend
            };
            let _ = game.battle_set_action(other, action);
        }
        game.battle_resolve_round();
    }

    /// Starts a battle against `enemies`, partitioned into species groups
    /// exactly as `start_battle` does. Tests that use this build their
    /// combatants by hand to pin down the precise stats a case needs, which
    /// `start_battle`'s own spawn path can't express — this keeps them
    /// saying "these programs are in the fight" without restating the
    /// partition, and without the opening log line.
    fn insert_battle(game: &mut Game, player: Entity, enemies: Vec<Entity>) {
        let groups = game.group_pack(enemies);
        let slots = game.world.resource::<Party>().0.len() + 1;
        game.world.insert_resource(BattleState {
            player,
            groups,
            round: 1,
            planned: vec![None; slots],
            finished: false,
            player_won: false,
        });
    }

    /// Copies the shipped `species`/`structures`/`research`/`items` asset
    /// dirs into a fresh scratch dir, skipping the item files named in
    /// `omit_items` and writing `extra_items` (filename, RON body) on top —
    /// a stand-in for a modded install. The caller removes the directory
    /// once its `Game` is done with it.
    fn modded_assets_dir(
        tag: &str,
        omit_items: &[&str],
        extra_items: &[(&str, &str)],
        extra_species: &[(&str, &str)],
    ) -> std::path::PathBuf {
        static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "feral_processes_{tag}_{}_{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let shipped = test_assets_dir();
        for sub in ["species", "structures", "research", "items"] {
            let dst = dir.join(sub);
            std::fs::create_dir_all(&dst).unwrap();
            for entry in std::fs::read_dir(shipped.join(sub)).unwrap() {
                let entry = entry.unwrap();
                let name = entry.file_name();
                if sub == "items" && omit_items.contains(&name.to_str().unwrap_or_default()) {
                    continue;
                }
                std::fs::copy(entry.path(), dst.join(name)).unwrap();
            }
        }
        for (name, body) in extra_items {
            std::fs::write(dir.join("items").join(name), body).unwrap();
        }
        for (name, body) in extra_species {
            std::fs::write(dir.join("species").join(name), body).unwrap();
        }
        dir
    }

    /// A modded install missing `core_fragment.ron` — the item that holds
    /// the Currency economy role — so `Game::new`'s missing-role startup
    /// abort (see `ItemDb::missing_roles`) can be exercised against an
    /// otherwise-valid item set.
    fn assets_dir_missing_currency_item() -> std::path::PathBuf {
        modded_assets_dir("missing_currency", &["core_fragment.ron"], &[], &[])
    }

    /// Gives the player `n` Research Data, bypassing the Research Node so
    /// the test doesn't depend on tick timing or a tamed worker.
    fn grant_research_data(game: &mut Game, n: u32) {
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::RESEARCH_DATA), n);
    }

    /// Deploys a Data Cache next to the player without going through
    /// `place_structure`, sidestepping its Home/cost/radius requirements —
    /// those aren't what the capacity tests are about.
    fn spawn_data_cache(game: &mut Game, offset: i32) {
        let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.world.spawn((
            Structure {
                kind: "data_cache".to_string(),
            },
            Position {
                x: pos.x + offset,
                y: pos.y,
            },
        ));
    }

    #[test]
    fn pet_capacity_grows_with_each_deployed_data_cache() {
        let mut game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY);

        spawn_data_cache(&mut game, 1);
        assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY + 2);

        spawn_data_cache(&mut game, 2);
        assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY + 4, "caches stack");
    }

    #[test]
    fn destroying_a_data_cache_shrinks_the_pet_capacity_back() {
        let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        spawn_data_cache(&mut game, 1);
        assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY + 2);

        let cache = game
            .world
            .iter_entities()
            .find(|e| e.get::<Structure>().is_some_and(|s| s.kind == "data_cache"))
            .map(|e| e.id())
            .expect("the spawned cache should be findable");
        game.world.despawn(cache);

        assert_eq!(
            game.pet_capacity(),
            BASE_PET_CAPACITY,
            "capacity is derived, so a destroyed cache needs no invalidation"
        );
    }

    #[test]
    fn inventory_used_counts_cargo_but_not_research_data() {
        let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Starting inventory is 3 ICE Breaker + 3 Power Cell + 5 Core Fragment.
        assert_eq!(game.inventory_used(), 11);

        grant_research_data(&mut game, 90);
        assert_eq!(
            game.inventory_used(),
            11,
            "banked research must not consume carrying capacity"
        );

        assert_eq!(game.player_status().inventory_used, 11);
    }

    #[test]
    fn the_buffer_is_unbounded_so_cargo_actions_never_refuse_for_space() {
        let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Pile on far more cargo than the old 30-unit cap ever allowed.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 10_000);

        game.craft(&ItemId::from(ids::POWER_CELL), 1)
            .expect("compiling never runs out of Buffer space now");
        let landed = game.grant_loot(ItemId::from(ids::PORTAL_FRAGMENT), 6);
        assert_eq!(
            landed, 6,
            "every looted unit lands — the Buffer can't fill up"
        );
    }

    /// Unlocks `id` and every prerequisite it needs, funding the whole
    /// chain — so a test that just needs a research-gated structure on the
    /// map doesn't have to model the tree itself.
    fn unlock_research_chain(game: &mut Game, id: &str) {
        fn order(game: &Game, id: &str, out: &mut Vec<String>) {
            let Some(def) = game.world.resource::<ResearchDb>().get(id).cloned() else {
                return;
            };
            for req in &def.requires {
                order(game, req, out);
            }
            if !out.contains(&def.id) {
                out.push(def.id);
            }
        }
        grant_research_data(game, 1000);
        let mut chain = Vec::new();
        order(game, id, &mut chain);
        for node in chain {
            if !game.is_researched(&node) {
                game.unlock_research(&node).unwrap();
            }
        }
    }

    fn research_data_held(game: &Game) -> u32 {
        game.player_status()
            .inventory
            .iter()
            .find(|(item, _)| *item == ItemId::from(ids::RESEARCH_DATA))
            .map(|(_, n)| *n)
            .unwrap_or(0)
    }

    /// Tames a program and puts it to work on a node producing `resource`,
    /// so a cronjob is guaranteed to be running — the assertions below are
    /// vacuous if nothing is assigned.
    fn assign_worker_producing(game: &mut Game, resource: ItemId) {
        let worker = spawn_tamed(game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "test_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource,
                    amount: 20,
                    capacity: 20,
                    level: None,
                },
            ))
            .id();
        game.assign_cronjob(worker, structure).unwrap();
    }

    #[test]
    fn a_cronjob_worker_fills_the_unbounded_buffer_past_the_old_cap() {
        let mut game = Game::new(708, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assign_worker_producing(&mut game, ItemId::from(ids::CORE_FRAGMENT));
        let before = game.inventory_used();

        for _ in 0..100 {
            game.tick();
        }

        assert!(
            game.inventory_used() > before,
            "a working cronjob keeps depositing cargo — the Buffer never fills up"
        );
    }

    #[test]
    fn a_research_cronjob_banks_research_data_over_time() {
        let mut game = Game::new(709, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assign_worker_producing(&mut game, ItemId::from(ids::RESEARCH_DATA));
        let before = research_data_held(&game);

        for _ in 0..100 {
            game.tick();
        }

        assert!(
            research_data_held(&game) > before,
            "a research cronjob must bank research over time (was {before}, now {})",
            research_data_held(&game)
        );
    }

    #[test]
    fn a_save_round_trip_preserves_unlocked_research() {
        let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "weapon_bench");

        let path =
            std::env::temp_dir().join(format!("feral_research_save_{}.bin", std::process::id()));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert!(loaded.is_researched("automation"));
        assert!(loaded.is_researched("weapon_bench"));
        assert!(
            !loaded.is_researched("commerce"),
            "loading must not invent research the player never took"
        );
    }

    /// Everything compilable from turn one: the two consumable starters plus
    /// the Scavenged gear tier, which declares a `craftable` with no
    /// `requires_structure`. Anything else must be gated behind research, a
    /// bench, or both — so this set is pinned rather than counted.
    #[test]
    fn only_the_starters_and_scavenged_gear_need_no_research_or_bench() {
        let game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let mut results: Vec<String> = game
            .craft_recipes()
            .into_iter()
            .map(|r| r.result.as_str().to_string())
            .collect();
        results.sort();
        assert_eq!(
            results,
            [
                "handshake_forge",
                "ice_breaker",
                "kinetic_edge",
                "packet_buffer",
                "power_cell",
                "probe_daemon",
                "scrap_ward",
                "shiv_routine",
            ],
            "nothing else is free"
        );
    }

    #[test]
    fn a_researched_recipe_stays_hidden_until_its_bench_is_built() {
        let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "overclock");

        let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
        assert!(
            !results.contains(&ItemId::from(ids::OVERCLOCK_CORE)),
            "the blueprint alone isn't enough — you still need the Fabricator"
        );

        place_home(&mut game, 1, 0);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 200);
        game.place_structure("fabricator", 0, 1).unwrap();

        let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
        assert!(results.contains(&ItemId::from(ids::OVERCLOCK_CORE)));
    }

    #[test]
    fn a_built_bench_alone_does_not_unlock_its_recipe() {
        let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "weapon_bench");
        place_home(&mut game, 1, 0);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 200);
        game.place_structure("fabricator", 0, 1).unwrap();

        let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
        assert!(
            !results.contains(&ItemId::from(ids::OVERCLOCK_CORE)),
            "the Fabricator is a bench now, not an unlock"
        );
    }

    /// The Standard/Premium gear tiers declare their own recipe with a
    /// `requires_structure` bench and no research node of their own. Building
    /// the bench is the whole unlock — but it is still a real gate.
    #[test]
    fn an_item_declared_recipe_stays_hidden_until_its_bench_is_built() {
        let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let arc_lance = ItemId::from("arc_lance");

        assert!(
            !game.craft_recipes().iter().any(|r| r.result == arc_lance),
            "a bench-gated item recipe must not be free from turn one"
        );

        // The Fabricator itself is research-gated; that gates the bench, not
        // the recipe, which has no research node of its own.
        unlock_research_chain(&mut game, "weapon_bench");
        place_home(&mut game, 1, 0);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 200);
        game.place_structure("fabricator", 0, 1).unwrap();

        assert!(
            game.craft_recipes().iter().any(|r| r.result == arc_lance),
            "standing the bench should be enough — no research node names this recipe"
        );
    }

    /// Gear sources can be declared from either side. Both are honoured, an
    /// item named twice is rolled once at the better chance, and the list is
    /// ordered so a seeded run always spends its rolls the same way.
    #[test]
    fn equipment_drops_merge_both_declaration_sides_taking_the_better_chance() {
        let game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let mut scrapper = game
            .species_defs()
            .into_iter()
            .find(|s| s.id == "scrapper")
            .expect("scrapper ships");
        let arc_lance = ItemId::from("arc_lance");
        let chance_of = |drops: &[(ItemId, f32)], id: &ItemId| {
            drops
                .iter()
                .find(|(i, _)| i == id)
                .unwrap_or_else(|| panic!("{} should be droppable here", id.as_str()))
                .1
        };

        // Item side alone: arc_lance.ron names scrapper.
        let drops = game.equipment_drops_for(&scrapper);
        assert_eq!(chance_of(&drops, &arc_lance), 0.1);
        let mut sorted = drops.clone();
        sorted.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        assert_eq!(drops, sorted, "roll order must be deterministic");

        // Declared on both sides: one entry, the better chance.
        scrapper.equipment_drop = Some((arc_lance.clone(), 0.5));
        let drops = game.equipment_drops_for(&scrapper);
        assert_eq!(
            drops.iter().filter(|(i, _)| *i == arc_lance).count(),
            1,
            "declared twice, rolled once"
        );
        assert_eq!(chance_of(&drops, &arc_lance), 0.5);

        // The weaker of the two loses, whichever side it came from.
        scrapper.equipment_drop = Some((arc_lance.clone(), 0.02));
        let drops = game.equipment_drops_for(&scrapper);
        assert_eq!(chance_of(&drops, &arc_lance), 0.1);
    }

    /// A species-side `equipment_drop` is legacy but still supported, so a
    /// third-party species mod that predates item-side `droppable` keeps
    /// dropping what it always did.
    #[test]
    fn a_species_side_equipment_drop_still_works_on_its_own() {
        let game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let mut sprite = game
            .species_defs()
            .into_iter()
            .find(|s| s.id == "sprite")
            .expect("sprite ships");
        // Nothing names power_cell in a `droppable`, so this can only arrive
        // from the species side.
        let power_cell = ItemId::from(ids::POWER_CELL);
        sprite.equipment_drop = Some((power_cell.clone(), 0.25));

        let drops = game.equipment_drops_for(&sprite);
        assert!(drops.contains(&(power_cell, 0.25)));
    }

    #[test]
    fn a_researched_recipe_carries_the_cost_from_its_ron_file() {
        let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "overclock");
        place_home(&mut game, 1, 0);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 200);
        game.place_structure("fabricator", 0, 1).unwrap();

        assert_eq!(
            game.craft_cost(&ItemId::from(ids::OVERCLOCK_CORE)),
            vec![(ItemId::from(ids::PORTAL_FRAGMENT), 6)]
        );
    }

    #[test]
    fn a_structure_named_by_no_research_file_is_buildable_from_the_start() {
        let game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let ids: Vec<String> = game
            .buildable_structure_defs()
            .into_iter()
            .map(|d| d.id)
            .collect();
        let mut sorted = ids.clone();
        sorted.sort();
        assert_eq!(
            sorted,
            vec![
                "data_cache".to_string(),
                "home".to_string(),
                "mining_node".to_string(),
                "portal".to_string(),
                "recharger_node".to_string(),
                "research_node".to_string(),
            ],
            "exactly the structures named by no research file start available"
        );
    }

    #[test]
    fn a_research_gated_structure_is_hidden_from_the_build_menu_until_researched() {
        let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let hidden: Vec<String> = game
            .buildable_structure_defs()
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert!(!hidden.contains(&"fabricator".to_string()));

        grant_research_data(&mut game, 40);
        game.unlock_research("automation").unwrap();
        game.unlock_research("weapon_bench").unwrap();

        let shown: Vec<String> = game
            .buildable_structure_defs()
            .into_iter()
            .map(|d| d.id)
            .collect();
        assert!(shown.contains(&"fabricator".to_string()));
    }

    #[test]
    fn placing_an_unresearched_structure_is_rejected_even_when_called_directly() {
        let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, 1, 0);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 200);
        let err = game.place_structure("fabricator", 0, 1).unwrap_err();
        assert!(
            err.contains("researched"),
            "filtering the menu is not a gate: {err}"
        );
    }

    #[test]
    fn nothing_is_researched_at_the_start_of_a_game() {
        let game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(!game.is_researched("automation"));
        assert!(
            game.research_nodes()
                .iter()
                .all(|n| n.state != ResearchState::Unlocked),
            "a fresh game starts with an entirely locked tree"
        );
    }

    #[test]
    fn unlocking_research_consumes_exactly_its_cost() {
        let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        grant_research_data(&mut game, 20);
        game.unlock_research("automation").unwrap();
        assert!(game.is_researched("automation"));
        assert_eq!(
            research_data_held(&game),
            12,
            "automation costs 8 of the 20 granted"
        );
    }

    #[test]
    fn unlocking_research_fails_without_enough_research_data() {
        let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        grant_research_data(&mut game, 7);
        let err = game.unlock_research("automation").unwrap_err();
        assert!(err.contains("Research Data"), "got: {err}");
        assert!(!game.is_researched("automation"));
    }

    #[test]
    fn unlocking_research_fails_while_a_prerequisite_is_missing() {
        let mut game = Game::new(64, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        grant_research_data(&mut game, 500);
        let err = game.unlock_research("weapon_bench").unwrap_err();
        assert!(
            err.contains("Automation"),
            "the error should name the missing prereq: {err}"
        );
        assert!(!game.is_researched("weapon_bench"));
        assert_eq!(
            research_data_held(&game),
            500,
            "a rejected unlock must not charge the player"
        );
    }

    #[test]
    fn a_locked_node_reports_which_prerequisites_are_missing() {
        let game = Game::new(65, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let node = game
            .research_nodes()
            .into_iter()
            .find(|n| n.id == "weapon_bench")
            .unwrap();
        assert_eq!(
            node.state,
            ResearchState::Locked {
                missing: vec!["Automation".to_string()]
            }
        );
    }

    #[test]
    fn a_prerequisite_free_node_is_available_immediately() {
        let game = Game::new(66, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let node = game
            .research_nodes()
            .into_iter()
            .find(|n| n.id == "automation")
            .unwrap();
        assert_eq!(node.state, ResearchState::Available);
        assert!(
            !node.affordable,
            "available is about prereqs; affordability is separate"
        );
    }

    #[test]
    fn researching_the_same_node_twice_is_rejected() {
        let mut game = Game::new(67, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        grant_research_data(&mut game, 40);
        game.unlock_research("automation").unwrap();
        let err = game.unlock_research("automation").unwrap_err();
        assert!(err.contains("already"), "got: {err}");
    }

    #[test]
    fn unknown_research_is_rejected() {
        let mut game = Game::new(68, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(game.unlock_research("not_a_node").is_err());
    }

    #[test]
    fn research_nodes_lists_available_before_locked_before_unlocked() {
        let mut game = Game::new(69, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        grant_research_data(&mut game, 40);
        game.unlock_research("automation").unwrap();
        let ranks: Vec<u8> = game
            .research_nodes()
            .iter()
            .map(|n| match n.state {
                ResearchState::Available => 0,
                ResearchState::Locked { .. } => 1,
                ResearchState::Unlocked => 2,
            })
            .collect();
        let mut sorted = ranks.clone();
        sorted.sort();
        assert_eq!(ranks, sorted, "menu order must group by state");
    }

    #[test]
    fn the_data_cache_is_buildable_without_any_research() {
        let game = Game::new(710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(
            game.buildable_structure_defs()
                .iter()
                .any(|d| d.id == "data_cache"),
            "buffer expansion must not be gated behind research the player \
             can't afford while the cap is at its tightest"
        );
    }

    #[test]
    fn no_research_node_is_left_unlocking_nothing() {
        let game = Game::new(711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        for node in game.research_nodes() {
            let def = game
                .world
                .resource::<ResearchDb>()
                .get(&node.id)
                .expect("a listed node should exist in the db");
            assert!(
                !def.unlocks_structures.is_empty() || !def.unlocks_recipes.is_empty(),
                "{} unlocks nothing and is dead weight in the tree",
                node.id
            );
        }
    }

    #[test]
    fn the_research_node_is_a_cronjob_worked_research_data_source() {
        let game = Game::new(60, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "research_node")
            .expect("research_node.ron should load");
        let work = def.work.expect("the Research Node must be workable");
        assert_eq!(work.produces, ItemId::from(ids::RESEARCH_DATA));
    }

    /// Deploys a Home just off the player's current position (`dx`, `dy`
    /// relative, so it doesn't collide with whatever the caller places
    /// next) — `place_structure` refuses anything else until a Home
    /// exists, so most structure-placement tests need this first.
    fn place_home(game: &mut Game, dx: i32, dy: i32) {
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 5);
        game.place_structure("home", dx, dy).unwrap();
    }

    /// How many of `id` the player is holding.
    fn count_item(game: &Game, id: &str) -> u32 {
        let player = game.player_entity();
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(id))
    }

    fn run_one_full_gather_cycle(game: &mut Game, resource: &str) -> u32 {
        run_one_full_gather_cycle_at_tier(game, resource, None)
    }

    /// Runs exactly one completed gather cycle against a hand-built node
    /// producing `resource` at `tier`, and returns how many units landed in
    /// the player's inventory.
    ///
    /// `level: None` on the node means it always yields (see
    /// `systems::mining_success_chance`), which is what keeps the payout
    /// assertions off the RNG entirely.
    fn run_one_full_gather_cycle_at_tier(
        game: &mut Game,
        resource: &str,
        tier: Option<u32>,
    ) -> u32 {
        let worker = spawn_tamed(game, 10, 3);
        let mut structure = game.world.spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: 3, y: 4 },
            ResourceNode {
                resource: ItemId::from(resource),
                amount: 5,
                capacity: 5,
                level: None,
            },
        ));
        if let Some(t) = tier {
            structure.insert(StructureTier(t));
        }
        let structure = structure.id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        let before = count_item(game, resource);
        game.tick();
        count_item(game, resource) - before
    }

    fn find_structure_by_kind(game: &mut Game, kind: &str) -> Option<Entity> {
        let mut query = game.world.query::<(Entity, &Structure)>();
        query
            .iter(&game.world)
            .find(|(_, s)| s.kind == kind)
            .map(|(e, _)| e)
    }

    #[test]
    fn placing_a_home_stamps_a_walkable_platform_across_the_build_radius() {
        let mut game = Game::new(920, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
        place_home(&mut game, 0, 1);
        let (hx, hy) = (ppos.x, ppos.y + 1);

        let mut map = game.world.resource_mut::<WorldMap>();
        for (dx, dy) in [
            (0, 0),
            (MAX_BUILD_DISTANCE_FROM_HOME, MAX_BUILD_DISTANCE_FROM_HOME),
            (-MAX_BUILD_DISTANCE_FROM_HOME, MAX_BUILD_DISTANCE_FROM_HOME),
        ] {
            let tile = map.tile(hx + dx, hy + dy);
            assert_eq!(
                tile.biome,
                Biome::Platform,
                "({dx}, {dy}) from Home should be platform floor"
            );
            assert!(tile.walkable, "platform floor must always be walkable");
        }
        assert_ne!(
            map.tile(hx + MAX_BUILD_DISTANCE_FROM_HOME + 1, hy).biome,
            Biome::Platform,
            "one tile past the build radius should still be natural terrain"
        );
    }

    #[test]
    fn placing_a_home_obliterates_hostiles_and_nests_inside_the_radius_only() {
        let mut game = Game::new(921, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();

        let inside = game
            .world
            .spawn((
                Hostile,
                Position {
                    x: ppos.x + 3,
                    y: ppos.y + 3,
                },
            ))
            .id();
        let outside = game
            .world
            .spawn((
                Hostile,
                Position {
                    x: ppos.x + MAX_BUILD_DISTANCE_FROM_HOME + 2,
                    y: ppos.y,
                },
            ))
            .id();
        let nest_inside = game
            .world
            .spawn((
                Nest {
                    species: "sprite".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position {
                    x: ppos.x - 2,
                    y: ppos.y + 1,
                },
            ))
            .id();

        place_home(&mut game, 0, 0);

        assert!(
            game.world.get_entity(inside).is_err(),
            "a hostile inside the radius is obliterated"
        );
        assert!(
            game.world.get_entity(nest_inside).is_err(),
            "a nest inside the radius is obliterated"
        );
        assert!(
            game.world.get_entity(outside).is_ok(),
            "a hostile outside the radius survives"
        );
    }

    #[test]
    fn obliterating_a_nest_untethers_a_guardian_standing_outside_the_radius() {
        let mut game = Game::new(922, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();

        let nest = game
            .world
            .spawn((
                Nest {
                    species: "sprite".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position {
                    x: ppos.x + 1,
                    y: ppos.y,
                },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                NestGuardian { nest },
                Position {
                    x: ppos.x + MAX_BUILD_DISTANCE_FROM_HOME + 3,
                    y: ppos.y,
                },
            ))
            .id();

        place_home(&mut game, 0, 0);

        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "a guardian outside the slab must lose its tether when its nest is obliterated, \
             not keep pointing at a despawned entity"
        );
    }

    #[test]
    fn demolishing_the_home_clears_the_platform_back_to_natural_terrain() {
        let mut game = Game::new(923, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
        place_home(&mut game, 0, 1);

        let home = find_structure_by_kind(&mut game, "home").expect("the Home should be deployed");
        game.remove_structure(home).unwrap();

        assert_ne!(
            game.world
                .resource_mut::<WorldMap>()
                .tile(ppos.x, ppos.y + 1)
                .biome,
            Biome::Platform,
            "demolishing the Home should leave no orphan sanctuary behind"
        );
        assert!(
            game.world.resource::<Platform>().center.is_none(),
            "the platform resource should forget its center once the Home is gone"
        );
    }

    #[test]
    fn no_wild_creature_ever_spawns_on_platform_floor() {
        let mut game = Game::new(924, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let ppos = *game.world.get::<Position>(game.player_entity()).unwrap();
        place_home(&mut game, 0, 0);

        for _ in 0..400 {
            game.try_spawn_habitat_creature(ppos.x + 2, ppos.y + 2);
        }

        // Counted by position, not as a global Hostile tally: Game::new
        // seeds the zone with wild programs and only those inside the build
        // radius are obliterated, so survivors further out are expected and
        // have nothing to do with what this test is asserting.
        let on_platform = {
            let mut query = game.world.query_filtered::<&Position, With<Hostile>>();
            let positions: Vec<Position> = query.iter(&game.world).copied().collect();
            let mut map = game.world.resource_mut::<WorldMap>();
            positions
                .iter()
                .filter(|p| map.tile(p.x, p.y).biome == Biome::Platform)
                .count()
        };
        assert_eq!(
            on_platform, 0,
            "platform floor has no habitat species, so nothing can spawn on it"
        );
    }

    #[test]
    fn award_loot_grants_the_species_work_resource() {
        let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| s.work_resource.is_some())
            .expect("at least one species should have a work_resource for this test");
        let resource = species.work_resource.unwrap();

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position { x: 0, y: 0 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        let before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&resource);
        game.award_loot(wild);
        let after = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&resource);

        assert!(
            after > before,
            "defeating the program should have granted {resource:?}"
        );
        let tagged = game
            .message_log(10)
            .into_iter()
            .any(|(kind, _)| kind == MessageKind::Loot);
        assert!(
            tagged,
            "a resource drop should log a MessageKind::Loot line, got: {:?}",
            game.message_log(10)
        );
    }

    #[test]
    fn award_loot_grants_nothing_for_species_without_a_work_resource() {
        let mut game = Game::new(2, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| s.work_resource.is_none())
            .expect("at least one species should have no work_resource for this test");

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position { x: 0, y: 0 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        // Portal Fragments are a universal drop, and gear arrives on its own
        // `droppable` channel — count neither, so this only measures whether
        // the absent `work_resource` stayed silent.
        let count_resources = |game: &Game| -> u32 {
            let gear_ids: Vec<ItemId> = game
                .world
                .resource::<ItemDb>()
                .all()
                .filter(|d| d.equipment.is_some())
                .map(|d| d.id.clone())
                .collect();
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .items
                .iter()
                .filter(|(item, _)| {
                    *item != ItemId::from(ids::PORTAL_FRAGMENT) && !gear_ids.contains(item)
                })
                .map(|(_, q)| *q)
                .sum()
        };
        let before = count_resources(&game);
        game.award_loot(wild);
        let after = count_resources(&game);

        assert_eq!(
            before, after,
            "no-resource species shouldn't add anything besides a possible portal fragment"
        );
    }

    #[test]
    fn inspect_reports_species_detail_without_starting_a_battle() {
        let mut game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: species.base_hp,
                    max_hp: species.base_hp,
                    atk: species.base_atk,
                    def: species.base_def,
                },
            ))
            .id();

        let view = game
            .inspect(wild)
            .expect("wild creature should be inspectable");
        assert_eq!(view.name, species.name);
        assert!(view.is_hostile);
        assert!(!view.is_tamed);
        assert_eq!(view.max_hp, species.base_hp);
        let chance = view
            .decompile_chance
            .expect("the starting kit includes a taming catalyst");
        assert!((0.0..=1.0).contains(&chance));
        assert!(
            !game.has_active_battle(),
            "inspecting must not trigger an intrusion"
        );
    }

    #[test]
    fn inspect_returns_none_for_non_creature_entities() {
        let game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        assert!(game.inspect(player).is_none());
    }

    #[test]
    fn use_symlink_teleports_the_player_to_the_structure_and_charges_the_cost() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.teleport_cost.is_some())
            .expect("a symlink-capable structure (Home) should exist");
        let cost = def.teleport_cost.clone().unwrap();

        let home = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 50, y: 50 },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
            ))
            .id();

        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.add(item.clone(), *qty);
            }
        }
        let before: Vec<u32> = cost
            .iter()
            .map(|(item, _)| game.world.get::<Inventory>(player).unwrap().count(item))
            .collect();

        let targets = game.symlink_targets();
        assert!(
            targets.iter().any(|t| t.entity == home),
            "Home should be a symlink target"
        );

        game.use_symlink(home).unwrap();

        let pos = *game.world.get::<Position>(player).unwrap();
        assert_eq!(
            pos,
            Position { x: 50, y: 50 },
            "symlink should teleport the player onto the structure"
        );
        for ((item, qty), before) in cost.iter().zip(before) {
            let after = game.world.get::<Inventory>(player).unwrap().count(item);
            assert_eq!(
                after,
                before - qty,
                "the teleport cost should be fully consumed"
            );
        }
    }

    #[test]
    fn use_symlink_fails_without_enough_of_the_cost() {
        let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.teleport_cost.is_some())
            .expect("a symlink-capable structure (Home) should exist");

        let home = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 20, y: 20 },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
            ))
            .id();

        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
        }

        let before_pos = *game.world.get::<Position>(player).unwrap();
        assert!(game.use_symlink(home).is_err());
        let after_pos = *game.world.get::<Position>(player).unwrap();
        assert_eq!(
            before_pos, after_pos,
            "a failed symlink shouldn't move the player"
        );
    }

    #[test]
    fn place_structure_rejects_anything_but_home_until_a_home_exists() {
        let mut game = Game::new(300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "armor_bench");
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 20);

        assert!(
            game.place_structure("armory", 1, 0).is_err(),
            "nothing should be buildable before a Home exists"
        );
        assert_eq!(
            game.view_entities(10, 10)
                .into_iter()
                .filter(|e| e.is_structure)
                .count(),
            0,
            "the rejected placement shouldn't have spawned anything"
        );

        game.place_structure("home", -1, 0).unwrap();
        game.place_structure("armory", 1, 0).unwrap();
        assert_eq!(
            game.view_entities(10, 10)
                .into_iter()
                .filter(|e| e.is_structure)
                .count(),
            2,
            "once a Home exists, other structures should be buildable"
        );
    }

    #[test]
    fn place_structure_rejects_a_second_home() {
        let mut game = Game::new(301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, -1, 0);

        let err = game
            .place_structure("home", 1, 0)
            .expect_err("a second Home shouldn't be buildable while one already exists");
        assert!(err.contains("already deployed"), "unexpected error: {err}");
    }

    #[test]
    fn place_structure_rejects_building_beyond_max_distance_from_home() {
        let mut game = Game::new(302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "armor_bench");
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 20);
        place_home(&mut game, 0, 1);

        // Walk far enough away that the next placement lands outside the
        // build radius from Home.
        game.world.get_mut::<Position>(player).unwrap().x += 20;
        let err = game
            .place_structure("armory", 1, 0)
            .expect_err("structures beyond MAX_BUILD_DISTANCE_FROM_HOME shouldn't be buildable");
        assert!(err.contains("Too far from Home"), "unexpected error: {err}");

        // Walking back within range should make it buildable again.
        game.world.get_mut::<Position>(player).unwrap().x -= 20;
        game.place_structure("armory", 1, 0)
            .expect("building back within range of Home should succeed");
    }

    #[test]
    fn remove_structure_refunds_a_percentage_of_its_build_cost() {
        let mut game = Game::new(303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "armor_bench");
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 20);
        place_home(&mut game, -1, 0);
        game.place_structure("armory", 1, 0).unwrap();
        let armory = game
            .view_entities(10, 10)
            .into_iter()
            .find(|e| e.is_structure && !e.is_home)
            .unwrap()
            .entity;

        let before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT));
        game.remove_structure(armory).unwrap();
        let after = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT));

        assert!(
            after > before,
            "demolishing a structure should refund some of its build cost"
        );
        assert_eq!(
            game.view_entities(10, 10)
                .into_iter()
                .filter(|e| e.is_structure)
                .count(),
            1,
            "only the Home should remain after demolishing the armory"
        );
    }

    #[test]
    fn removing_home_cascades_to_destroy_every_other_structure_and_refunds_each() {
        let mut game = Game::new(304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "armor_bench");
        unlock_research_chain(&mut game, "weapon_bench");
        let player = game.player_entity();
        // Just enough Core Fragments to afford Home + armory + fabricator
        // and no more: a big surplus (as a naive "plenty of buffer" amount
        // would be) leaves cargo sitting at or above capacity once combined
        // with starting gear, which would clamp the refund this test exists
        // to check — see `removing_home_cascade_refund_is_capped_to_available_room`
        // for that clamping behavior instead.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 31);
        place_home(&mut game, -1, 0);
        game.place_structure("armory", 1, 0).unwrap();
        game.place_structure("fabricator", 0, 1).unwrap();
        let home = game
            .view_entities(10, 10)
            .into_iter()
            .find(|e| e.is_home)
            .unwrap()
            .entity;

        let before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT));
        game.remove_structure(home).unwrap();
        let after = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT));

        assert_eq!(
            game.view_entities(10, 10)
                .into_iter()
                .filter(|e| e.is_structure)
                .count(),
            0,
            "removing Home should cascade to remove every other structure too"
        );
        assert!(
            after > before,
            "the cascade should refund a share of every demolished structure's cost, including Home's own"
        );
    }

    #[test]
    fn armory_and_fabricator_are_not_cronjob_workable() {
        let game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        for id in ["armory", "fabricator"] {
            let def = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("{id}.ron should load as a structure"));
            assert!(
                def.work.is_none(),
                "{id} should unlock crafting instead of being cronjob-workable"
            );
        }
    }

    #[test]
    fn every_structure_describes_its_actual_capability() {
        let game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        for def in game.structure_defs() {
            // Every shipped structure now has a real capability, so "no effect
            // yet" always means the description derivation is missing a field
            // the structure actually uses — the Data Cache reached exactly that
            // state when `pet_slot_bonus` was added without updating this.
            assert_ne!(
                game.structure_description(&def),
                "no effect yet",
                "{} has an undescribed effect",
                def.id
            );
        }
    }

    #[test]
    fn structure_descriptions_cover_non_production_capabilities() {
        let game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let describe = |id: &str| {
            let def = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == id)
                .unwrap_or_else(|| panic!("{id}.ron should load as a structure"));
            game.structure_description(&def)
        };
        assert!(describe("armory").contains("Firewall Plating"));
        assert!(describe("fabricator").contains("Cortex Hack"));
        assert!(describe("home").contains("Power Cell"));
        assert!(describe("shield").contains("raid damage"));
        assert!(describe("data_cache").contains("pet slots"));
        assert!(describe("recharger_node").contains("recharge"));
        assert!(describe("portal").contains("next zone"));
        assert!(describe("market").contains("trade"));
        assert!(describe("compiler").contains("ICE Breaker"));
        assert!(describe("terminal").contains("Power Cell"));
    }

    #[test]
    fn researching_and_building_an_armory_unlocks_firewall_plating() {
        let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "firewall");
        assert!(
            game.craft_recipes()
                .iter()
                .all(|r| r.result != ItemId::from(ids::FIREWALL_PLATING)),
            "Firewall Plating shouldn't be craftable before an Armory is built"
        );

        place_home(&mut game, -1, 0);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 18);
        game.place_structure("armory", 1, 0).unwrap();

        let recipe = game
            .craft_recipes()
            .into_iter()
            .find(|r| r.result == ItemId::from(ids::FIREWALL_PLATING))
            .expect("researching it and building an Armory should unlock the recipe");
        assert_eq!(recipe.cost, vec![(ItemId::from(ids::PORTAL_FRAGMENT), 6)]);

        // Exactly the recipe's cost (6), not a padded amount: any excess
        // pushes cargo over the inventory cap and the compile is refused.
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 6);
        game.craft(&ItemId::from(ids::FIREWALL_PLATING), 1).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(game.player_entity())
                .unwrap()
                .count(&ItemId::from(ids::FIREWALL_PLATING)),
            1
        );
    }

    #[test]
    fn cronjob_assignment_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(6, DifficultyMode::Forgiving, &assets).unwrap();

        let structure_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.work.is_some())
            .expect("at least one workable structure should exist");
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: structure_def.id.clone(),
                },
                Position { x: 3, y: 3 },
                ResourceNode {
                    resource: structure_def.work.as_ref().unwrap().produces.clone(),
                    amount: 20,
                    capacity: 20,
                    level: None,
                },
            ))
            .id();

        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let player = game.player_entity();
        game.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position { x: 3, y: 4 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
            Tamed { owner: player },
            Experience::default(),
            Task {
                kind: TaskKind::GatherResource,
                target: structure,
                progress: 3,
                required: 6,
            },
        ));

        let path = std::env::temp_dir().join(format!(
            "feral_processes_cronjob_test_{}_{}.bin",
            std::process::id(),
            6
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut query = loaded.world.query::<&Task>();
        let task = query
            .iter(&loaded.world)
            .next()
            .expect("restored creature should still have its cronjob task");
        assert_eq!(task.progress, 3);
        assert_eq!(task.required, 6);
        let target_pos = loaded
            .world
            .get::<Position>(task.target)
            .expect("task target should resolve to a structure entity");
        assert_eq!((target_pos.x, target_pos.y), (3, 3));
    }

    #[test]
    fn a_mined_out_node_refills_instead_of_stalling_the_cronjob() {
        let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::from(ids::CORE_FRAGMENT),
                    amount: 1,
                    capacity: 2,
                    level: None,
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        // First tick mines the last unit down to 0.
        game.tick();
        assert_eq!(game.world.get::<ResourceNode>(structure).unwrap().amount, 0);

        // The node refills to capacity on the next tick rather than
        // leaving the assigned creature permanently idle.
        game.tick();
        assert_eq!(game.world.get::<ResourceNode>(structure).unwrap().amount, 1);
        assert!(
            game.world.get::<Task>(worker).is_some(),
            "the cronjob should keep running once the node refills"
        );
    }

    #[test]
    fn cronjob_work_grants_no_more_xp_once_the_worker_hits_the_work_level_cap() {
        let mut game = Game::new(301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.world.get_mut::<Experience>(worker).unwrap().level = systems::WORK_XP_LEVEL_CAP;
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::from(ids::CORE_FRAGMENT),
                    amount: 5,
                    capacity: 5,
                    level: None,
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        for _ in 0..3 {
            game.tick();
        }

        let exp = game.world.get::<Experience>(worker).unwrap();
        assert_eq!(
            exp.level,
            systems::WORK_XP_LEVEL_CAP,
            "a capped worker shouldn't level further from cronjob work"
        );
        assert_eq!(
            exp.xp, 0,
            "a capped worker shouldn't earn any work XP at all"
        );
    }

    #[test]
    fn cronjob_work_still_grants_xp_below_the_work_level_cap() {
        let mut game = Game::new(302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        assert!(
            game.world.get::<Experience>(worker).unwrap().level < systems::WORK_XP_LEVEL_CAP,
            "a freshly tamed program should start well under the work level cap"
        );
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::from(ids::CORE_FRAGMENT),
                    amount: 5,
                    capacity: 5,
                    level: None,
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        game.tick();

        let xp = game.world.get::<Experience>(worker).unwrap().xp;
        assert!(xp > 0, "a worker under the cap should still earn work XP");
    }

    #[test]
    fn a_leveled_node_doesnt_always_yield_on_a_completed_cycle() {
        let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::from(ids::CORE_FRAGMENT),
                    amount: 20,
                    capacity: 20,
                    level: Some(1),
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: 1,
        });

        let player = game.player_entity();
        let starting_fragments = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT));

        for _ in 0..40 {
            game.tick();
        }

        let gained = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT))
            - starting_fragments;
        assert!(
            gained < 40,
            "a level-1 node succeeding on every single one of 40 cycles is implausible at ~50% odds, got {gained}"
        );
    }

    #[test]
    fn player_decompiler_skill_grows_on_level_up_and_survives_save_load() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();

        assert_eq!(
            game.player_status().decompiler,
            0,
            "should start with no decompiler skill"
        );

        game.award_player_xp(player, 20);
        assert_eq!(
            game.player_status().level,
            2,
            "20 xp should be enough to reach level 2"
        );
        assert_eq!(
            game.player_status().decompiler,
            DECOMPILER_SKILL_PER_LEVEL,
            "one level gained should grant one point of decompiler skill"
        );

        let path = std::env::temp_dir().join(format!(
            "feral_processes_decompiler_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.player_status().decompiler,
            DECOMPILER_SKILL_PER_LEVEL,
            "decompiler skill should survive a save/load round trip"
        );
    }

    #[test]
    fn equip_grants_stat_bonus_and_removes_item_from_inventory() {
        let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
        let atk_before = game.player_status().atk;

        game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

        let status = game.player_status();
        assert_eq!(
            status.atk,
            atk_before + 3,
            "weapon should grant its Attack bonus"
        );
        assert_eq!(
            status.weapon,
            Some(EquippedItem {
                item: ItemId::from(ids::OVERCLOCK_CORE),
                level: 1,
                fusion_tier: 0
            })
        );
        assert!(
            status
                .inventory
                .iter()
                .all(|(i, _)| *i != ItemId::from(ids::OVERCLOCK_CORE)),
            "equipped item should leave the inventory stack"
        );
    }

    #[test]
    fn equipping_gear_in_a_deeper_zone_scales_its_bonus_100_percent_per_level() {
        let mut game = Game::new(8, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.resource_mut::<ZoneLevel>().0 = 3;
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
        let atk_before = game.player_status().atk;

        game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

        let status = game.player_status();
        // Base +3 ATK, scaled 2x per level above 1: level 3 = 3 * 2^2 = 12.
        assert_eq!(
            status.atk,
            atk_before + 12,
            "gear equipped at zone level 3 should be scaled 2x per level"
        );
        assert_eq!(
            status.weapon,
            Some(EquippedItem {
                item: ItemId::from(ids::OVERCLOCK_CORE),
                level: 3,
                fusion_tier: 0
            })
        );

        game.unequip(EquipmentSlot::Weapon).unwrap();
        assert_eq!(
            game.player_status().atk,
            atk_before,
            "unequipping should remove exactly the level-scaled bonus that was granted"
        );
    }

    #[test]
    fn equipping_the_same_slot_again_swaps_without_double_counting_the_bonus() {
        let mut game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 2);
        let atk_before = game.player_status().atk;

        game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
        assert_eq!(game.player_status().atk, atk_before + 3);

        // Equipping into an already-occupied slot swaps the old item back
        // to inventory and must not stack the bonus a second time.
        game.equip(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
        let status = game.player_status();
        assert_eq!(
            status.atk,
            atk_before + 3,
            "re-equipping must not double the bonus"
        );
        assert_eq!(
            status
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::from(ids::OVERCLOCK_CORE))
                .map(|(_, q)| *q),
            Some(1),
            "the swapped-out copy should return to inventory"
        );
    }

    #[test]
    fn unequip_removes_bonus_and_returns_item_to_inventory() {
        let mut game = Game::new(10, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::FIREWALL_PLATING), 1);
        let def_before = game.player_status().def;
        game.equip(&ItemId::from(ids::FIREWALL_PLATING)).unwrap();
        assert_eq!(game.player_status().def, def_before + 3);

        game.unequip(EquipmentSlot::Armor).unwrap();

        let status = game.player_status();
        assert_eq!(status.def, def_before, "unequip should remove the bonus");
        assert_eq!(status.armor, None);
        assert_eq!(
            status
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::from(ids::FIREWALL_PLATING))
                .map(|(_, q)| *q),
            Some(1)
        );
    }

    #[test]
    fn unequip_errors_on_an_empty_slot() {
        let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(game.unequip(EquipmentSlot::Weapon).is_err());
    }

    #[test]
    fn unequipping_an_item_with_no_itemdb_entry_errors_instead_of_panicking() {
        // A save can restore an `EquippedItem` id that `ItemDb::load_dir`
        // has since warned-and-skipped (the mod's .ron was renamed, broken,
        // or removed) — `Game::load` doesn't validate equipment slots
        // against the item set, so `equipment_of` can no longer resolve
        // the id by the time the player tries to unequip it.
        let mut game = Game::new(712, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let broken = ItemId::from("a_removed_mod_item");
        game.world.get_mut::<Equipment>(player).unwrap().weapon = Some(EquippedItem {
            item: broken.clone(),
            level: 1,
            fusion_tier: 0,
        });
        let inventory_before = game.world.get::<Inventory>(player).unwrap().items.clone();
        let stats_before = {
            let stats = game.world.get::<Stats>(player).unwrap();
            (stats.atk, stats.def)
        };
        let decompiler_before = game.world.get::<Decompiler>(player).map(|d| d.skill);

        let result = game.unequip(EquipmentSlot::Weapon);

        assert!(
            result.is_err(),
            "unequipping an item absent from ItemDb should error, not panic"
        );
        assert_eq!(
            game.player_status().weapon.map(|eq| eq.item),
            Some(broken),
            "a refused unequip must leave the item in its slot, not destroy it"
        );
        assert_eq!(
            game.world.get::<Inventory>(player).unwrap().items,
            inventory_before,
            "a refused unequip must not touch the inventory"
        );
        let stats_after = game.world.get::<Stats>(player).unwrap();
        assert_eq!(
            (stats_after.atk, stats_after.def),
            stats_before,
            "a refused unequip must not alter stats"
        );
        assert_eq!(
            game.world.get::<Decompiler>(player).map(|d| d.skill),
            decompiler_before,
            "a refused unequip must not alter decompiler skill"
        );
    }

    #[test]
    fn equipping_over_a_slot_holding_an_item_with_no_itemdb_entry_errors_instead_of_panicking() {
        // Same failure mode as the unequip case above, but hit via the
        // swap-out path when equipping a new item into an already-occupied
        // slot whose old occupant's data is gone.
        let mut game = Game::new(713, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let broken = ItemId::from("a_removed_mod_item");
        game.world.get_mut::<Equipment>(player).unwrap().weapon = Some(EquippedItem {
            item: broken.clone(),
            level: 1,
            fusion_tier: 0,
        });
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
        let inventory_before = game.world.get::<Inventory>(player).unwrap().items.clone();
        let stats_before = {
            let stats = game.world.get::<Stats>(player).unwrap();
            (stats.atk, stats.def)
        };
        let decompiler_before = game.world.get::<Decompiler>(player).map(|d| d.skill);

        let result = game.equip(&ItemId::from(ids::OVERCLOCK_CORE));

        assert!(
            result.is_err(),
            "equipping over a slot whose old item is absent from ItemDb should error, not panic"
        );
        assert_eq!(
            game.player_status().weapon.map(|eq| eq.item),
            Some(broken),
            "a refused equip must leave the old item in its slot, not destroy it"
        );
        assert_eq!(
            game.world.get::<Inventory>(player).unwrap().items,
            inventory_before,
            "a refused equip must not consume the new item from inventory"
        );
        let stats_after = game.world.get::<Stats>(player).unwrap();
        assert_eq!(
            (stats_after.atk, stats_after.def),
            stats_before,
            "a refused equip must not alter stats"
        );
        assert_eq!(
            game.world.get::<Decompiler>(player).map(|d| d.skill),
            decompiler_before,
            "a refused equip must not alter decompiler skill"
        );
    }

    #[test]
    fn fuse_item_consumes_two_copies_and_raises_the_fusion_tier() {
        let mut game = Game::new(200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 3);

        game.fuse_item(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

        assert_eq!(game.item_fusion_tier(&ItemId::from(ids::OVERCLOCK_CORE)), 1);
        assert_eq!(
            game.player_status()
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::from(ids::OVERCLOCK_CORE))
                .map(|(_, q)| *q),
            Some(1),
            "fusing should consume 2 of the 3 copies"
        );
    }

    #[test]
    fn fuse_item_bonus_scales_the_equipped_stat_bonus() {
        let mut game = Game::new(201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Ablative Plating's base is +4 def, so a 10%/tier bonus is visible
        // (unlike a +3 item, where 10% rounds away to nothing at tier 1).
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::ABLATIVE_PLATING), 6);

        let def_before = game.player_status().def;
        game.equip(&ItemId::from(ids::ABLATIVE_PLATING)).unwrap();
        assert_eq!(
            game.player_status().def,
            def_before + 4,
            "unfused equip should grant the plain base bonus"
        );
        game.unequip(EquipmentSlot::Armor).unwrap();

        game.fuse_item(&ItemId::from(ids::ABLATIVE_PLATING))
            .unwrap();
        game.fuse_item(&ItemId::from(ids::ABLATIVE_PLATING))
            .unwrap();
        assert_eq!(
            game.item_fusion_tier(&ItemId::from(ids::ABLATIVE_PLATING)),
            2
        );

        game.equip(&ItemId::from(ids::ABLATIVE_PLATING)).unwrap();
        assert_eq!(
            game.player_status().def,
            def_before + 5,
            "tier 2 is +20%: 4 * 1.2 = 4.8, rounds to 5"
        );
    }

    #[test]
    fn fuse_item_rejects_non_equipment_and_insufficient_stock() {
        let mut game = Game::new(202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        assert!(
            game.fuse_item(&ItemId::from(ids::CORE_FRAGMENT)).is_err(),
            "plain resources aren't equipment and can't be fused"
        );

        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
        assert!(
            game.fuse_item(&ItemId::from(ids::OVERCLOCK_CORE)).is_err(),
            "fusing needs 2 copies, only 1 is available"
        );
        assert_eq!(
            game.player_status()
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::from(ids::OVERCLOCK_CORE))
                .map(|(_, q)| *q),
            Some(1),
            "a failed fuse should not consume the lone copy"
        );
    }

    #[test]
    fn fusing_a_worn_item_counts_it_and_upgrades_the_worn_copy_live() {
        let armor = ItemId::from(ids::ABLATIVE_PLATING);
        let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // One copy to wear, two spares.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(armor.clone(), 3);
        let base_def = game.player_status().def;

        game.equip(&armor).unwrap();
        assert_eq!(
            game.player_status().def,
            base_def + 4,
            "Ablative Plating's base is +4 def while worn, unfused"
        );

        let held = |g: &Game| {
            g.player_status()
                .inventory
                .iter()
                .find(|(i, _)| *i == armor)
                .map(|(_, q)| *q)
                .unwrap_or(0)
        };
        assert_eq!(held(&game), 2, "equipping consumed one of the three copies");

        // The worn copy counts as one of the two a fusion needs, so a single
        // spare is enough.
        game.fuse_item(&armor).unwrap();
        assert_eq!(game.item_fusion_tier(&armor), 1);
        assert_eq!(
            held(&game),
            1,
            "only one spare consumed — the worn copy counted for the other"
        );

        // Second fuse reaches tier 2, where +20% is visible: 4 * 1.2 = 4.8 -> 5.
        game.fuse_item(&armor).unwrap();
        assert_eq!(game.item_fusion_tier(&armor), 2);
        assert_eq!(held(&game), 0);
        assert_eq!(
            game.player_status().def,
            base_def + 5,
            "the worn copy picks up the new tier live, without a re-equip"
        );
    }

    #[test]
    fn fusing_a_worn_item_still_needs_one_spare() {
        let armor = ItemId::from(ids::ABLATIVE_PLATING);
        let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(armor.clone(), 1);
        game.equip(&armor).unwrap(); // now zero spares held
        let err = game.fuse_item(&armor).unwrap_err();
        assert_eq!(err, "Need 1 Ablative Plating to fuse (have 0).");
        assert_eq!(
            game.item_fusion_tier(&armor),
            0,
            "a refused fuse changes nothing"
        );
    }

    #[test]
    fn fusing_needs_two_spares_when_a_different_item_is_worn() {
        let worn = ItemId::from(ids::FIREWALL_PLATING); // armor
        let target = ItemId::from(ids::ABLATIVE_PLATING); // also armor, different item
        let mut game = Game::new(706, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.add(worn.clone(), 1);
            inv.add(target.clone(), 1);
        }
        game.equip(&worn).unwrap(); // Firewall Plating occupies the Armor slot
        // The worn armor is a different item, so it can't count toward fusing
        // Ablative Plating — that still needs two spares.
        let err = game.fuse_item(&target).unwrap_err();
        assert_eq!(err, "Need 2 Ablative Plating to fuse (have 1).");
    }

    #[test]
    fn a_successful_fuse_returns_its_confirmation_line() {
        let core = ItemId::from(ids::OVERCLOCK_CORE);
        let mut game = Game::new(707, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(core.clone(), 2);
        let msg = game.fuse_item(&core).unwrap();
        assert!(
            msg.contains("fuse") && msg.contains('%'),
            "a fuse must hand back a confirmation to surface, got: {msg}"
        );
    }

    #[test]
    fn item_fusion_tier_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(203, DifficultyMode::Forgiving, &assets).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 2);
        game.fuse_item(&ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_fusion_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.item_fusion_tier(&ItemId::from(ids::OVERCLOCK_CORE)),
            1
        );
    }

    #[test]
    fn erase_item_removes_the_full_stack() {
        let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::NEURAL_AMPLIFIER), 3);

        game.erase_item(&ItemId::from(ids::NEURAL_AMPLIFIER), 3)
            .unwrap();
        assert!(
            game.player_status()
                .inventory
                .iter()
                .all(|(i, _)| *i != ItemId::from(ids::NEURAL_AMPLIFIER))
        );

        assert!(
            game.erase_item(&ItemId::from(ids::NEURAL_AMPLIFIER), 1)
                .is_err(),
            "erasing from an empty stack should error"
        );
    }

    #[test]
    fn equipped_gear_and_its_bonus_survive_save_and_load() {
        let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::NEURAL_AMPLIFIER), 1);
        game.equip(&ItemId::from(ids::NEURAL_AMPLIFIER)).unwrap();
        let decompiler_after_equip = game.player_status().decompiler;

        let path = std::env::temp_dir().join(format!(
            "feral_processes_equipment_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        let status = loaded.player_status();
        assert_eq!(
            status.module,
            Some(EquippedItem {
                item: ItemId::from(ids::NEURAL_AMPLIFIER),
                level: 1,
                fusion_tier: 0
            })
        );
        assert_eq!(status.decompiler, decompiler_after_equip);
    }

    #[test]
    fn game_new_aborts_startup_when_the_item_set_is_missing_the_currency_role() {
        // The economy can't run without a Currency-role item — see
        // `ItemDb::missing_roles` — so `Game::new` must abort before the
        // world is built rather than let play reach `Game::currency()`'s
        // `.expect("validated at startup")` deep in gameplay.
        let dir = assets_dir_missing_currency_item();
        let result = Game::new(900, DifficultyMode::Forgiving, &dir);
        let _ = std::fs::remove_dir_all(&dir);

        // `Game` isn't `Debug` (it wraps a `bevy_ecs::World`), so this can't
        // use `Result::expect_err` / `unwrap_err`.
        let Err(err) = result else {
            panic!("startup should abort rather than run with no item holding the Currency role");
        };
        assert!(
            err.to_string().contains("Currency"),
            "error should name the missing role: {err}"
        );
    }

    #[test]
    fn game_load_aborts_when_the_item_set_is_missing_the_currency_role() {
        // Resuming a save is the other door into the same world, and it
        // reaches the same `Game::currency()` `.expect("validated at
        // startup")` — so an item set that lost its Currency-role holder
        // between saving and loading has to be refused here too, not only
        // in `Game::new`.
        let mut game = Game::new(902, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let path = std::env::temp_dir().join(format!(
            "feral_missing_currency_load_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();

        let dir = assets_dir_missing_currency_item();
        let result = Game::load(&path, &dir);
        let _ = std::fs::remove_dir_all(&dir);
        let _ = std::fs::remove_file(&path);

        // `Game` isn't `Debug` (it wraps a `bevy_ecs::World`), so this can't
        // use `Result::expect_err` / `unwrap_err`.
        let Err(err) = result else {
            panic!(
                "loading should abort rather than resume with no item holding the Currency role"
            );
        };
        assert!(
            err.to_string().contains("Currency"),
            "error should name the missing role: {err}"
        );
    }

    #[test]
    fn every_shipped_asset_file_loads_without_a_warning() {
        // A malformed shipped asset is warn-and-skipped like a mod's would
        // be, so it costs the player content silently instead of failing the
        // build. This is the only thing that catches it — a serde attribute
        // missing from `ItemId` once made every asset load fail this way.
        let game = Game::new(901, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

        let skipped: Vec<String> = game
            .message_log(usize::MAX)
            .into_iter()
            .map(|(_, text)| text)
            .filter(|text| text.contains("skipped invalid"))
            .collect();

        assert!(
            skipped.is_empty(),
            "shipped assets must all parse: {skipped:#?}"
        );
    }

    /// The initial world spawns 14 wild creatures scattered around the
    /// player, so directional-inspect tests clear whatever landed along
    /// their search ray first — otherwise they'd be at the mercy of the
    /// seed's RNG instead of testing the method itself.
    fn clear_creatures_east_of_player(game: &mut Game, start: Position, range: i32) {
        // Matches the same 90° eastward cone `find_creature_in_direction`
        // itself uses, not just the exact row — otherwise a wild creature
        // that merely leans east (without being exactly on the player's
        // row) would survive the cleanup and make the test flaky.
        let stale: Vec<Entity> = {
            let mut query = game.world.query::<(Entity, &Position, &Creature)>();
            query
                .iter(&game.world)
                .filter(|(_, pos, _)| {
                    let (ddx, ddy) = (pos.x - start.x, pos.y - start.y);
                    ddx > 0 && ddx >= ddy.abs() && ddx <= range
                })
                .map(|(e, ..)| e)
                .collect()
        };
        for e in stale {
            game.world.despawn(e);
        }
    }

    #[test]
    fn find_creature_in_direction_finds_the_nearest_match_along_the_line() {
        let mut game = Game::new(14, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let start = *game.world.get::<Position>(player).unwrap();
        let species = game.species_defs().into_iter().next().unwrap();
        clear_creatures_east_of_player(&mut game, start, 10);

        assert!(game.find_creature_in_direction(1, 0, 10).is_none());

        let far = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: start.x + 5,
                    y: start.y,
                },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        let near = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: start.x + 2,
                    y: start.y,
                },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        let found = game.find_creature_in_direction(1, 0, 10);
        assert_eq!(
            found,
            Some(near),
            "the nearer creature along the ray should win"
        );
        assert_ne!(found, Some(far));
    }

    #[test]
    fn find_creature_in_direction_respects_max_range() {
        let mut game = Game::new(15, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let start = *game.world.get::<Position>(player).unwrap();
        let species = game.species_defs().into_iter().next().unwrap();
        clear_creatures_east_of_player(&mut game, start, 10);
        game.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + 10,
                y: start.y,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ));

        assert!(
            game.find_creature_in_direction(1, 0, 5).is_none(),
            "creature is out of range"
        );
        assert!(
            game.find_creature_in_direction(1, 0, 10).is_some(),
            "creature should be within range"
        );
    }

    #[test]
    fn find_creature_in_direction_matches_a_90_degree_cone_not_just_the_exact_row() {
        let mut game = Game::new(17, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let start = *game.world.get::<Position>(player).unwrap();
        let species = game.species_defs().into_iter().next().unwrap();
        clear_creatures_east_of_player(&mut game, start, 10);

        // Leans east more than north/south (ddx=4 >= |ddy|=3) — inside the cone.
        let diagonal_ish = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: start.x + 4,
                    y: start.y - 3,
                },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        assert_eq!(
            game.find_creature_in_direction(1, 0, 10),
            Some(diagonal_ish)
        );

        // Leans north more than east (ddy=-8, ddx=2) — outside the eastward cone.
        game.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + 2,
                y: start.y - 8,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                def: 1,
            },
        ));
        assert_eq!(
            game.find_creature_in_direction(1, 0, 10),
            Some(diagonal_ish),
            "a creature that leans mostly north shouldn't win the eastward search"
        );
    }

    #[test]
    fn player_status_power_matches_max_hp_plus_atk_plus_def() {
        let game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let status = game.player_status();
        assert_eq!(status.power, status.max_hp + status.atk + status.def);
    }

    /// The map's Integrity gauge and the battle screen's "You" bar are two
    /// readouts of one number. Nothing may fork them — not the entity they
    /// resolve, not a buff, not a stale view.
    #[test]
    fn battle_view_integrity_matches_the_map_status_panel() {
        let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = start_battle_with_a_wild_program(&mut game);
        let player = game.player_entity();
        assert_eq!(
            game.world.resource::<BattleState>().player,
            player,
            "the battle must be fought by the entity the map panel reads"
        );

        // Outlast the pack without killing it: a fight that ends mid-loop
        // would drop the battle view and stop comparing.
        {
            let mut w = game.world.get_mut::<Stats>(wild).unwrap();
            w.hp = 10_000;
            w.max_hp = 10_000;
            w.atk = 50;
        }
        {
            let mut p = game.world.get_mut::<Stats>(player).unwrap();
            p.hp = 5_000;
            p.max_hp = 5_000;
        }

        let start_hp = game.player_status().hp;
        for round in 0..10 {
            player_attacks(&mut game);
            let status = game.player_status();
            let view = game
                .battle_view()
                .unwrap_or_else(|| panic!("battle ended early at round {round}"));
            let player_row = &view.party[0];
            assert_eq!(player_row.hp, status.hp, "hp diverged at round {round}");
            assert_eq!(
                player_row.max_hp, status.max_hp,
                "max_hp diverged at round {round}"
            );
        }
        assert!(
            game.player_status().hp < start_hp,
            "the wild program never landed a hit, so the comparison proved nothing"
        );
    }

    #[test]
    fn wait_advances_one_tick_without_moving() {
        let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let pos_before = *game.world.get::<Position>(player).unwrap();
        let tick_before = game.world.resource::<GameClock>().tick;

        game.wait();

        let pos_after = *game.world.get::<Position>(player).unwrap();
        let tick_after = game.world.resource::<GameClock>().tick;
        assert_eq!(pos_after, pos_before, "waiting shouldn't move the player");
        assert_eq!(
            tick_after,
            tick_before + 1,
            "waiting should advance exactly one tick"
        );
    }

    #[test]
    fn current_tick_matches_the_internal_game_clock() {
        let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(
            game.current_tick(),
            0,
            "a fresh game should start at tick 0"
        );

        game.wait();
        game.wait();

        assert_eq!(
            game.current_tick(),
            2,
            "current_tick should track GameClock exactly"
        );
    }

    #[test]
    fn idle_tick_advances_the_clock_outside_battle_but_not_during_one() {
        let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

        game.idle_tick();
        assert_eq!(
            game.current_tick(),
            1,
            "idle_tick should advance the clock with no battle active"
        );

        let player = game.player_entity();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        insert_battle(&mut game, player, vec![wild]);
        game.idle_tick();
        assert_eq!(
            game.current_tick(),
            1,
            "idle_tick should be a no-op while a battle is active"
        );
    }

    #[test]
    fn rest_fully_heals_and_restores_fatigue() {
        let mut game = Game::new(18, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.hp = 1;
        }
        {
            let mut needs = game.world.get_mut::<Needs>(player).unwrap();
            needs.fatigue = 10.0;
        }
        spawn_rest_structure_at_player(&mut game);

        game.rest();

        let stats = *game.world.get::<Stats>(player).unwrap();
        let needs = *game.world.get::<Needs>(player).unwrap();
        assert_eq!(stats.hp, stats.max_hp, "rest should fully heal Integrity");
        assert_eq!(needs.fatigue, 100.0, "rest should fully restore Fatigue");
    }

    #[test]
    fn rest_also_fully_heals_the_active_companion() {
        let mut game = Game::new(29, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();
        {
            let mut stats = game.world.get_mut::<Stats>(companion).unwrap();
            stats.hp = 1;
        }
        spawn_rest_structure_at_player(&mut game);

        game.rest();

        let stats = *game.world.get::<Stats>(companion).unwrap();
        assert_eq!(
            stats.hp, stats.max_hp,
            "rest should fully heal the active companion too"
        );
    }

    #[test]
    fn successful_decompile_removes_wander_ai_so_the_tamed_creature_stops_roaming() {
        let mut game = Game::new(19, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");

        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                WanderAi::default(),
                Position { x: 3, y: 3 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);
        // Near-dead target + maxed decompiler skill + plenty of breakers,
        // so the capture-chance clamp (95%) makes a handful of attempts
        // succeed for certain, without needing to control the RNG directly.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::ICE_BREAKER), 50);
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

        for _ in 0..50 {
            if game.world.get::<Tamed>(wild).is_some() {
                break;
            }
            player_decompiles(&mut game);
        }

        assert!(
            game.world.get::<Tamed>(wild).is_some(),
            "creature should have been tamed"
        );
        assert!(game.world.get::<Hostile>(wild).is_none());
        assert!(
            game.world.get::<WanderAi>(wild).is_none(),
            "a tamed creature must stop roaming like a wild one"
        );
    }

    /// Replaces the player's whole inventory with `stock`, so a taming test
    /// states exactly which catalysts are on hand instead of inheriting
    /// whatever `Game::new`'s starting kit holds.
    fn set_inventory(game: &mut Game, stock: &[(&str, u32)]) {
        let player = game.player_entity();
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        inv.items.clear();
        for (id, qty) in stock {
            inv.add(ItemId::from(*id), *qty);
        }
    }

    /// Spawns a wild program on the player's tile and opens an intrusion on
    /// it — the state `battle_decompile` needs.
    fn start_battle_with_a_wild_program(game: &mut Game) -> Entity {
        let wild = spawn_wild_on_player_tile(game);
        game.start_battle(vec![wild]);
        wild
    }

    #[test]
    fn decompile_spends_the_highest_potency_catalyst_held_not_the_shipped_one() {
        // The mod case `taming_potency` exists for: a dropped-in catalyst
        // stronger than the shipped ICE Breaker must be the one resolved
        // and consumed, with no Rust change.
        let dir = modded_assets_dir(
            "strong_catalyst",
            &[],
            &[(
                "master_key.ron",
                r#"(id: "master_key", name: "Master Key", taming_potency: Some(0.9))"#,
            )],
            &[],
        );
        let mut game = Game::new(3100, DifficultyMode::Forgiving, &dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        start_battle_with_a_wild_program(&mut game);
        set_inventory(&mut game, &[(ids::ICE_BREAKER, 1), ("master_key", 1)]);

        player_decompiles(&mut game);

        let inv = game.world.get::<Inventory>(game.player_entity()).unwrap();
        assert_eq!(
            inv.count(&ItemId::from("master_key")),
            0,
            "the strongest catalyst held should be the one spent"
        );
        assert_eq!(
            inv.count(&ItemId::from(ids::ICE_BREAKER)),
            1,
            "the weaker catalyst must be left untouched"
        );
    }

    #[test]
    fn decompiling_with_no_catalyst_is_refused_without_naming_a_shipped_item() {
        let mut game = Game::new(3101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = start_battle_with_a_wild_program(&mut game);
        set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

        player_decompiles(&mut game);

        assert!(
            game.world.get::<Tamed>(wild).is_none(),
            "a decompile with no catalyst must not tame anything"
        );
        let refusal = game
            .message_log(usize::MAX)
            .into_iter()
            .map(|(_, line)| line)
            .find(|line| line.starts_with("You have no"))
            .expect("the refusal should be logged");
        let shipped_names: Vec<String> = game
            .world
            .resource::<ItemDb>()
            .all()
            .map(|d| d.name.clone())
            .collect();
        for name in shipped_names {
            assert!(
                !refusal.contains(&name),
                "the refusal must not name a specific item, got: {refusal}"
            );
        }
    }

    #[test]
    fn two_catalysts_of_equal_potency_resolve_to_the_first_id_alphabetically() {
        let dir = modded_assets_dir(
            "tied_catalysts",
            &[],
            &[
                (
                    "alpha_key.ron",
                    r#"(id: "alpha_key", name: "Alpha Key", taming_potency: Some(0.5))"#,
                ),
                (
                    "omega_key.ron",
                    r#"(id: "omega_key", name: "Omega Key", taming_potency: Some(0.5))"#,
                ),
            ],
            &[],
        );
        let mut game = Game::new(3102, DifficultyMode::Forgiving, &dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        start_battle_with_a_wild_program(&mut game);
        // Stocked in reverse so the tie can't be won by inventory order.
        set_inventory(&mut game, &[("omega_key", 1), ("alpha_key", 1)]);

        player_decompiles(&mut game);

        let inv = game.world.get::<Inventory>(game.player_entity()).unwrap();
        assert_eq!(
            inv.count(&ItemId::from("alpha_key")),
            0,
            "a tie should resolve to the first item id alphabetically"
        );
        assert_eq!(inv.count(&ItemId::from("omega_key")), 1);
    }

    #[test]
    fn the_decompile_preview_follows_the_catalyst_held_not_a_fixed_item() {
        let dir = modded_assets_dir(
            "preview_catalyst",
            &[],
            &[(
                "master_key.ron",
                r#"(id: "master_key", name: "Master Key", taming_potency: Some(0.9))"#,
            )],
            &[],
        );
        let mut game = Game::new(3104, DifficultyMode::Forgiving, &dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        let wild = spawn_wild_on_player_tile(&mut game);

        set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);
        let with_shipped = game
            .inspect(wild)
            .unwrap()
            .decompile_chance
            .expect("holding a catalyst should quote odds");
        set_inventory(&mut game, &[("master_key", 1)]);
        let with_mod = game
            .inspect(wild)
            .unwrap()
            .decompile_chance
            .expect("holding a catalyst should quote odds");
        assert!(
            with_mod > with_shipped,
            "a stronger catalyst must preview better odds: {with_mod} vs {with_shipped}"
        );

        set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 1)]);
        assert!(
            game.inspect(wild).unwrap().decompile_chance.is_none(),
            "with no catalyst there are no odds to quote — the action is unavailable"
        );
    }

    #[test]
    fn battle_view_offers_no_decompile_odds_without_a_catalyst() {
        let mut game = Game::new(3105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        start_battle_with_a_wild_program(&mut game);
        assert!(
            game.battle_view().unwrap().groups[0]
                .decompile_chance
                .is_some(),
            "the starting kit holds a catalyst"
        );

        set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

        // This is also what gates the engine-emitted de[c]ompile option.
        assert!(
            game.battle_view().unwrap().groups[0]
                .decompile_chance
                .is_none()
        );
    }

    #[test]
    fn the_shipped_ice_breaker_still_tames_for_a_player_holding_only_it() {
        let mut game = Game::new(3103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let wild = start_battle_with_a_wild_program(&mut game);
        set_inventory(&mut game, &[(ids::ICE_BREAKER, 50)]);
        // Maxed Decompiler skill pins capture_chance to its 0.95 clamp, so
        // 50 seeded attempts succeed without the test depending on a
        // particular roll.
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

        let mut attempts = 0;
        for _ in 0..50 {
            if game.world.get::<Tamed>(wild).is_some() {
                break;
            }
            player_decompiles(&mut game);
            attempts += 1;
        }

        assert!(
            game.world.get::<Tamed>(wild).is_some(),
            "the shipped catalyst must still tame exactly as before"
        );
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::ICE_BREAKER)),
            50 - attempts,
            "one ICE Breaker per attempt, same as before"
        );
    }

    #[test]
    fn raid_check_never_targets_a_nest_even_as_the_only_durability_holder() {
        let mut game = Game::new(600, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Strip every other Durability holder so a Nest would be the only
        // possible target if it weren't explicitly excluded.
        let existing: Vec<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<Durability>>();
            query.iter(&game.world).collect()
        };
        for e in existing {
            game.world.despawn(e);
        }
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 10, y: 10 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();

        for _ in 0..500 {
            game.raid_check();
        }

        assert_eq!(
            game.world.get::<Durability>(nest).unwrap().hp,
            NEST_DURABILITY,
            "a Nest must never take raid damage, even when it's the only Durability holder"
        );
    }

    #[test]
    fn entering_a_zone_portal_despawns_nests_left_behind_in_the_old_zone() {
        let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 10, y: 10 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();

        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();
        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));

        game.move_player(1, 0);

        // Note: `enter_next_zone` spawns fresh initial creatures for the new
        // zone, which can legitimately include brand-new nests — so this
        // must check the specific entity spawned above, not just count all
        // `Nest` entities in the world.
        assert!(
            game.world.get_entity(nest).is_err(),
            "a Nest left behind in the old zone must be despawned on zone transition, not just its guardians"
        );
    }

    #[test]
    fn spawn_nest_creates_a_tethered_guardian_cluster() {
        let mut game = Game::new(601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

        // `Game::new` runs its own initial habitat-spawn rolls, which can
        // themselves occasionally create a Nest (now that species like
        // scrapper have can_nest: true) before this test's own explicit
        // spawn_nest call ever runs. Capture whatever nests already exist
        // first, so the assertions below only ever look at the nest this
        // test itself created, not a world-wide count that a background
        // spawn could inflate.
        let pre_existing_nests: std::collections::HashSet<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<Nest>>();
            query.iter(&game.world).collect()
        };
        game.spawn_nest("scrapper", 30, 30);

        let nests: Vec<(Entity, Position)> = {
            let mut query = game.world.query::<(Entity, &Nest, &Position)>();
            query
                .iter(&game.world)
                .filter(|(e, _, _)| !pre_existing_nests.contains(e))
                .map(|(e, _, p)| (e, *p))
                .collect()
        };
        assert_eq!(
            nests.len(),
            1,
            "spawn_nest should create exactly one new Nest entity"
        );
        let (nest, nest_pos) = nests[0];
        assert_eq!(nest_pos, Position { x: 30, y: 30 });
        assert_eq!(
            game.world.get::<Durability>(nest).unwrap().hp,
            NEST_DURABILITY
        );

        let guardians: Vec<Position> = {
            let mut query = game.world.query::<(&NestGuardian, &Position)>();
            query
                .iter(&game.world)
                .filter(|(g, _)| g.nest == nest)
                .map(|(_, p)| *p)
                .collect()
        };
        assert!(
            guardians.len() >= NEST_GUARDIAN_MIN as usize
                && guardians.len() <= NEST_GUARDIAN_MAX as usize,
            "expected {}..={} guardians, got {}",
            NEST_GUARDIAN_MIN,
            NEST_GUARDIAN_MAX,
            guardians.len()
        );
        for pos in guardians {
            let dist = (pos.x - 30).abs().max((pos.y - 30).abs());
            assert!(
                dist <= NEST_TETHER_RADIUS,
                "guardian spawned {dist} tiles from its nest, past the {NEST_TETHER_RADIUS}-tile tether"
            );
        }
    }

    #[test]
    fn guardian_never_wanders_beyond_the_nest_tether_radius() {
        let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.spawn_nest("scrapper", 40, 40);

        let (nest, nest_pos) = {
            let mut query = game.world.query::<(Entity, &Nest, &Position)>();
            let (e, _, p) = query.iter(&game.world).next().expect("nest should exist");
            (e, *p)
        };
        let guardians: Vec<Entity> = {
            let mut query = game.world.query::<(Entity, &NestGuardian)>();
            query
                .iter(&game.world)
                .filter(|(_, g)| g.nest == nest)
                .map(|(e, _)| e)
                .collect()
        };
        assert!(!guardians.is_empty());

        for _ in 0..200 {
            game.tick();
            for &guardian in &guardians {
                let pos = *game.world.get::<Position>(guardian).unwrap();
                let dist = (pos.x - nest_pos.x).abs().max((pos.y - nest_pos.y).abs());
                assert!(
                    dist <= NEST_TETHER_RADIUS,
                    "guardian wandered {dist} tiles from its nest, past the {NEST_TETHER_RADIUS}-tile tether"
                );
            }
        }
    }

    #[test]
    fn craft_consumes_cost_and_grants_the_result() {
        let mut game = Game::new(20, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            inv.add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST);
        }

        game.craft(&ItemId::from(ids::ICE_BREAKER), 1).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
            0,
            "cost should be fully consumed"
        );
        assert_eq!(
            inv.count(&ItemId::from(ids::ICE_BREAKER)),
            1,
            "the recipe's result should be granted"
        );
    }

    #[test]
    fn craft_multiple_scales_cost_and_result() {
        let mut game = Game::new(30, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            inv.add(ItemId::from(ids::CORE_FRAGMENT), ICE_BREAKER_CORE_COST * 3);
        }

        game.craft(&ItemId::from(ids::ICE_BREAKER), 3).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
            0,
            "cost should scale with quantity"
        );
        assert_eq!(
            inv.count(&ItemId::from(ids::ICE_BREAKER)),
            3,
            "quantity units should be granted"
        );
    }

    #[test]
    fn max_craftable_floors_to_the_cheapest_affordable_whole_unit() {
        let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            // ICE_BREAKER_CORE_COST per unit; 7 fragments afford 2 whole
            // units with 1 left over, not 3.
            inv.add(
                ItemId::from(ids::CORE_FRAGMENT),
                ICE_BREAKER_CORE_COST * 2 + 1,
            );
        }

        assert_eq!(game.max_craftable(&ItemId::from(ids::ICE_BREAKER)), 2);
    }

    #[test]
    fn max_craftable_is_zero_with_no_recipe_or_no_resources() {
        let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .items
            .clear();

        assert_eq!(
            game.max_craftable(&ItemId::from(ids::ICE_BREAKER)),
            0,
            "no resources at all"
        );
        assert_eq!(
            game.max_craftable(&ItemId::from(ids::CORE_FRAGMENT)),
            0,
            "no recipe exists for this item"
        );
    }

    #[test]
    fn craft_fails_without_enough_of_the_cost() {
        let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
        }

        assert!(game.craft(&ItemId::from(ids::ICE_BREAKER), 1).is_err());
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::ICE_BREAKER)),
            0
        );
    }

    #[test]
    fn craft_rejects_a_result_with_no_recipe() {
        let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(game.craft(&ItemId::from(ids::CORE_FRAGMENT), 1).is_err());
    }

    #[test]
    fn structure_defs_order_pins_home_mining_research_compiler_first_and_is_stable_across_sessions()
    {
        // StructureDb is backed by a HashMap, whose iteration order is
        // randomized per-instance — without an explicit sort, the build
        // menu's [1], [2], ... numbering would shuffle between sessions
        // even though the mod files never changed. Multiple seeds (each a
        // fresh StructureDb/HashMap instance) should all agree.
        let seeds = [40, 41, 42, 43];
        let mut orders = Vec::new();
        for seed in seeds {
            let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let ids: Vec<String> = game.structure_defs().into_iter().map(|d| d.id).collect();
            assert_eq!(
                &ids[..4],
                ["home", "mining_node", "research_node", "compiler"],
                "the four starter structures should always lead the build menu"
            );
            let mut rest_sorted = ids[4..].to_vec();
            rest_sorted.sort();
            assert_eq!(
                ids[4..],
                rest_sorted[..],
                "everything after the pinned four should still be alphabetical"
            );
            orders.push(ids);
        }
        assert!(
            orders.windows(2).all(|w| w[0] == w[1]),
            "structure order should be identical across fresh sessions, got {orders:?}"
        );
    }

    #[test]
    fn species_defs_order_is_sorted_by_id_and_stable_across_sessions() {
        let seeds = [44, 45, 46, 47];
        let mut orders = Vec::new();
        for seed in seeds {
            let game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let ids: Vec<String> = game.species_defs().into_iter().map(|d| d.id).collect();
            let mut sorted = ids.clone();
            sorted.sort();
            assert_eq!(ids, sorted, "species_defs() should already be sorted by id");
            orders.push(ids);
        }
        assert!(
            orders.windows(2).all(|w| w[0] == w[1]),
            "species order should be identical across fresh sessions, got {orders:?}"
        );
    }

    #[test]
    fn battle_flee_applies_the_same_mild_xp_setback_as_a_death() {
        let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Experience>(player).unwrap().xp = 10;
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 3, y: 3 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 0,
                    def: 1,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        game.battle_flee();

        assert_eq!(
            game.world.get::<Experience>(player).unwrap().xp,
            8,
            "fleeing should dock the same 20% setback as a death"
        );
        assert!(!game.has_active_battle(), "fleeing should end the battle");
    }

    fn spawn_tamed(game: &mut Game, hp: i32, atk: i32) -> Entity {
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        game.world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position { x: 3, y: 3 },
                Stats {
                    hp,
                    max_hp: hp,
                    atk,
                    def: 1,
                },
                Tamed { owner: player },
                Experience::default(),
            ))
            .id()
    }

    /// Spawns a minimal wild (untamed, `Hostile`) `Creature` on the
    /// player's own tile, suitable to pass straight into `start_battle` —
    /// mirrors `spawn_tamed`'s pattern but without `Tamed`/`Experience`,
    /// since a wild pack member has neither.
    fn spawn_wild_on_player_tile(game: &mut Game) -> Entity {
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        game.world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position {
                    x: player_pos.x,
                    y: player_pos.y,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 0,
                    def: 1,
                },
            ))
            .id()
    }

    /// Deploys a Home directly on the player's current tile — `Game::rest`
    /// requires a rest-enabling structure nearby, so tests exercising `rest`
    /// need one in place first. Spawned directly rather than through
    /// `place_structure` to sidestep its cost and one-Home-only
    /// requirements, which aren't what these tests are about.
    fn spawn_rest_structure_at_player(game: &mut Game) {
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.world.spawn((
            Structure {
                kind: "home".to_string(),
            },
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
        ));
    }

    #[test]
    fn set_companion_rejects_a_wild_creature() {
        let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 3, y: 3 },
                Stats {
                    hp: 5,
                    max_hp: 5,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        assert!(game.add_companion(wild).is_err());
        assert!(game.player_status().companions.is_empty());
    }

    #[test]
    fn set_companion_clears_any_active_cronjob_task() {
        let mut game = Game::new(24, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 2,
            required: 5,
        });

        game.add_companion(worker).unwrap();

        assert!(
            game.world.get::<Task>(worker).is_none(),
            "companion duty should cancel the cronjob"
        );
        assert_eq!(
            game.player_status().companions.first().map(|c| c.hp),
            Some(10)
        );
    }

    #[test]
    fn assigning_cronjob_to_the_active_companion_clears_companion_status() {
        let assets = test_assets_dir();
        let mut game = Game::new(25, DifficultyMode::Forgiving, &assets).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.add_companion(worker).unwrap();
        assert!(!game.player_status().companions.is_empty());

        let structure_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.work.is_some())
            .expect("at least one workable structure should exist");
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: structure_def.id.clone(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: structure_def.work.as_ref().unwrap().produces.clone(),
                    amount: 20,
                    capacity: 20,
                    level: None,
                },
            ))
            .id();

        game.assign_cronjob(worker, structure).unwrap();

        assert!(
            game.player_status().companions.is_empty(),
            "running a cronjob should stand the companion down"
        );
        assert!(game.world.get::<Task>(worker).is_some());
    }

    #[test]
    fn clear_companion_reverts_to_no_companion() {
        let mut game = Game::new(26, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.add_companion(worker).unwrap();
        assert!(!game.player_status().companions.is_empty());

        game.remove_companion(worker);

        assert!(game.player_status().companions.is_empty());
    }

    #[test]
    fn owned_pets_reports_every_owned_creature_regardless_of_location_or_job() {
        let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();

        let far_worker = spawn_tamed(&mut game, 12, 4);
        game.world
            .entity_mut(far_worker)
            .insert(Position { x: 500, y: 500 });
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 500, y: 501 },
            ))
            .id();
        game.world.entity_mut(far_worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 1,
            required: 5,
        });

        let idle = spawn_tamed(&mut game, 5, 2);
        game.world
            .entity_mut(idle)
            .insert(Position { x: 999, y: 999 });

        let pets = game.owned_pets();
        assert_eq!(
            pets.len(),
            3,
            "every owned tamed creature should be reported, wherever it is"
        );

        let companion_info = pets.iter().find(|p| p.entity == companion).unwrap();
        assert!(companion_info.is_companion);
        assert_eq!(companion_info.activity, "in party");

        let worker_info = pets.iter().find(|p| p.entity == far_worker).unwrap();
        assert!(!worker_info.is_companion);
        assert_ne!(
            worker_info.activity, "idle",
            "a far-off cronjob worker should still be reported as working"
        );
        assert_eq!(worker_info.hp, 12);
        assert_eq!(worker_info.atk, 4);

        let idle_info = pets.iter().find(|p| p.entity == idle).unwrap();
        assert!(!idle_info.is_companion);
        assert_eq!(idle_info.activity, "idle");
    }

    #[test]
    fn a_companions_special_rallies_the_player_instead_of_attacking() {
        let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 20);
        game.add_companion(companion).unwrap();

        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 1,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        companion_uses_special(
            &mut game,
            companion,
            0,
            battle::SpecialTarget::Ally { slot: 0 },
        );

        let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            wild_hp, 100,
            "commanding a companion should never damage the wild creature directly"
        );
        let buff = game.world.get::<CombatBuff>(player).unwrap().active;
        assert!(
            buff.is_some_and(|b| b.kind == BuffKind::Atk),
            "commanding a companion with no special ability should rally (ATK buff) the player"
        );
    }

    /// Sets up a single-round battle with one companion (stunned or not)
    /// and returns how much the player's fatigue dropped from commanding
    /// it. Shared by the two fatigue-cost tests below.
    fn fatigue_spent_commanding_companion(seed: u32, stunned: bool) -> f32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 20);
        game.add_companion(companion).unwrap();
        if stunned {
            game.world.entity_mut(companion).insert(StatusEffects {
                active: Some(ActiveStatus {
                    kind: StatusKind::Stun,
                    remaining: 1,
                    power: 0,
                }),
            });
        }

        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 0,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        let fatigue_before = game.world.get::<Needs>(player).unwrap().fatigue;
        companion_uses_special(
            &mut game,
            companion,
            0,
            battle::SpecialTarget::Ally { slot: 0 },
        );
        let fatigue_after = game.world.get::<Needs>(player).unwrap().fatigue;
        fatigue_before - fatigue_after
    }

    #[test]
    fn commanding_a_companion_in_battle_costs_more_fatigue_than_a_stunned_one() {
        // Both paths advance the clock by one tick (a resolved round
        // always ticks at the end), so both pay the same small natural
        // fatigue decay regardless — comparing the two deltas rather than
        // asserting an absolute number isolates just the companion-command
        // cost from that shared per-tick decay.
        let active = fatigue_spent_commanding_companion(84, false);
        let stunned = fatigue_spent_commanding_companion(85, true);
        assert!(
            (active - stunned - COMPANION_COMMAND_FATIGUE_COST).abs() < 0.001,
            "commanding an active companion should cost exactly {COMPANION_COMMAND_FATIGUE_COST} \
             more fatigue than commanding a stunned one, which doesn't actually act: \
             active spent {active}, stunned spent {stunned}"
        );
    }

    #[test]
    fn an_atk_buff_increases_damage_dealt_and_expires_after_its_duration() {
        let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<CombatBuff>(player).unwrap().active = Some(ActiveBuff {
            kind: BuffKind::Atk,
            remaining: 1,
            power: 50,
        });

        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 10_000,
                    max_hp: 10_000,
                    atk: 0,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        player_attacks(&mut game);

        let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
        assert!(
            wild_hp < 10_000 - 50,
            "a +50 ATK buff should meaningfully increase damage dealt"
        );
        assert!(
            game.world
                .get::<CombatBuff>(player)
                .unwrap()
                .active
                .is_none(),
            "a 1-round buff should expire once the round it covered ticks down"
        );
    }

    #[test]
    fn special_ability_heal_restores_player_hp_and_debuff_afflicts_the_wild_creature() {
        let mut game = Game::new(19, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Stats>(player).unwrap().hp = 5;

        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 1,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();

        game.use_special_ability(&SpecialAbility::Heal { power: 8 }, "TestBot", player);
        let hp = game.world.get::<Stats>(player).unwrap().hp;
        assert_eq!(
            hp, 13,
            "Heal should restore the player's HP by its power, capped at max_hp"
        );

        game.use_special_ability(
            &SpecialAbility::Debuff {
                kind: StatusKind::Bleed,
                power: 4,
                duration: 2,
            },
            "TestBot",
            wild,
        );
        let active = game.world.get::<StatusEffects>(wild).unwrap().active;
        assert!(
            active.is_some_and(|a| a.kind == StatusKind::Bleed && a.power == 4 && a.remaining == 2),
            "Debuff should inflict the given status condition on the wild creature"
        );
    }

    /// A species declaring two abilities, so the multi-ability paths can be
    /// exercised without waiting on shipped content to grow any — no shipped
    /// species declares `special_abilities` at all yet.
    const TWO_ABILITY_SPECIES: &str = r#"(
        id: "test_medic",
        name: "Test Medic",
        glyph: 'm',
        color: Cyan,
        base_hp: 10,
        base_atk: 4,
        base_def: 2,
        taming_difficulty: 0.5,
        habitats: [OpenGrid],
        base_speed: 10,
        moves: [(name: "Poke", power: 3)],
        special_abilities: [
            Heal(power: 8),
            Shield(power: 4, duration: 3),
        ],
    )"#;

    #[test]
    fn companion_ability_label_shows_special_ability_or_a_computed_attack_rally() {
        let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let all_species = game.species_defs();
        let no_ability_species = all_species
            .iter()
            .find(|s| s.special_abilities.is_empty())
            .expect("at least one species with no special ability")
            .id
            .clone();

        let plain = game
            .world
            .spawn((
                Creature {
                    species: no_ability_species,
                },
                Position { x: 3, y: 3 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 30,
                    def: 1,
                },
                Tamed { owner: player },
                Experience::default(),
            ))
            .id();
        game.add_companion(plain).unwrap();
        let plain_ability = game.player_status().companions[0].ability.clone();
        assert_eq!(
            plain_ability, "Rally",
            "a species with no special_abilities should show the generic rally fallback"
        );
    }

    /// Spawns a tamed member of `TWO_ABILITY_SPECIES` into the party of a
    /// game built on a modded install that ships it.
    fn game_with_two_ability_companion() -> (Game, Entity) {
        let dir = modded_assets_dir(
            "two_ability_species",
            &[],
            &[],
            &[("test_medic.ron", TWO_ABILITY_SPECIES)],
        );
        let mut game = Game::new(94, DifficultyMode::Forgiving, &dir).unwrap();
        let player = game.player_entity();
        let medic = game
            .world
            .spawn((
                Creature {
                    species: "test_medic".to_string(),
                },
                Position { x: 3, y: 3 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 5,
                    def: 1,
                },
                Tamed { owner: player },
                Experience::default(),
            ))
            .id();
        game.add_companion(medic).unwrap();
        (game, medic)
    }

    #[test]
    fn a_species_with_several_abilities_offers_each_one_in_menu_order() {
        let (game, _) = game_with_two_ability_companion();
        let options = game.battle_special_options(1);
        assert_eq!(
            options.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
            vec!["Heal", "Shield"],
            "the picker should list the species' abilities in declaration order"
        );
        assert_eq!(
            options.iter().map(|o| o.index).collect::<Vec<_>>(),
            vec![0, 1],
            "index is what BattleAction::Special carries, so it must match position"
        );
        assert_eq!(options[0].detail, "Heal: +8 HP");
    }

    #[test]
    fn a_companion_declaring_no_abilities_still_offers_exactly_the_fallback_rally() {
        let mut game = Game::new(95, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();

        let options = game.battle_special_options(1);
        assert_eq!(
            options.len(),
            1,
            "the fallback is resolved into the list, so the menu is never empty"
        );
        assert_eq!(options[0].name, "Rally");
    }

    #[test]
    fn item_blurbs_gloss_what_a_shipped_item_actually_does() {
        let game = Game::new(96, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(
            game.item_blurb(&ItemId::from(ids::POWER_CELL)).as_deref(),
            Some("+25 power"),
            "a consumable should quote what it restores"
        );
        assert_eq!(
            game.item_blurb(&ItemId::from("arc_lance")).as_deref(),
            Some("+3 atk"),
            "equipment should quote the stats it grants"
        );
        assert_eq!(
            game.item_blurb(&ItemId::from("black_ice_pick")).as_deref(),
            Some("+3 atk +2 decomp"),
            "an item granting several stats should list each"
        );
        assert_eq!(
            game.item_blurb(&ItemId::from(ids::CORE_FRAGMENT)),
            None,
            "a plain currency has nothing to gloss and reads fine as itself"
        );
    }

    #[test]
    fn every_compilable_item_either_has_a_blurb_or_is_plain_currency() {
        let game = Game::new(97, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        for recipe in game.craft_recipes() {
            let blurb = game.item_blurb(&recipe.result);
            assert!(
                blurb.is_some(),
                "{} is compilable but the compile menu would say nothing about it",
                recipe.result
            );
        }
    }

    #[test]
    fn buffs_and_heals_aim_at_the_party_while_debuffs_aim_at_the_enemy() {
        use species::SpecialTargeting;
        assert_eq!(
            SpecialAbility::Heal { power: 8 }.targeting(),
            SpecialTargeting::Ally
        );
        assert_eq!(
            SpecialAbility::Rally {
                power: 3,
                duration: 2
            }
            .targeting(),
            SpecialTargeting::Ally
        );
        assert_eq!(
            SpecialAbility::Shield {
                power: 3,
                duration: 2
            }
            .targeting(),
            SpecialTargeting::Ally
        );
        assert_eq!(
            SpecialAbility::Debuff {
                kind: StatusKind::Bleed,
                power: 3,
                duration: 2
            }
            .targeting(),
            SpecialTargeting::Enemy
        );
    }

    /// The whole point of aiming a buff: it has to land on a companion, not
    /// just the player. Only the player is *spawned* holding a `CombatBuff`,
    /// so this is the case that silently did nothing before `arm_buff`.
    #[test]
    fn a_buff_aimed_at_a_companion_actually_reaches_it() {
        let (mut game, medic) = game_with_two_ability_companion();
        start_battle_with_a_wild_program(&mut game);
        let before = game.effective_def(medic);

        // Slot 1 is the medic itself; index 1 is its Shield.
        companion_uses_special(&mut game, medic, 1, battle::SpecialTarget::Ally { slot: 1 });

        assert!(
            matches!(
                game.world.get::<CombatBuff>(medic).and_then(|b| b.active),
                Some(ActiveBuff {
                    kind: BuffKind::Def,
                    ..
                })
            ),
            "a companion with no CombatBuff component must have one inserted, not be skipped"
        );
        assert!(
            game.effective_def(medic) > before,
            "the buff has to actually raise the companion's defense"
        );
    }

    /// A party-facing Special must not need a living enemy group to resolve,
    /// since its target isn't a group at all.
    #[test]
    fn healing_an_ally_does_not_depend_on_a_valid_enemy_group() {
        let (mut game, _) = game_with_two_ability_companion();
        let player = game.player_entity();
        game.world.get_mut::<Stats>(player).unwrap().hp = 5;
        start_battle_with_a_wild_program(&mut game);

        game.battle_set_action(
            1,
            BattleAction::Special {
                ability: 0,
                target: battle::SpecialTarget::Ally { slot: 0 },
            },
        )
        .expect("an ally-targeted Special has no group to reject");

        assert_eq!(
            game.world.get::<Stats>(player).unwrap().hp,
            5,
            "planning alone shouldn't heal — that happens on resolve"
        );
    }

    #[test]
    fn the_chosen_ability_index_decides_which_special_resolves() {
        let (mut game, medic) = game_with_two_ability_companion();
        let player = game.player_entity();
        game.world.get_mut::<Stats>(player).unwrap().hp = 1;
        start_battle_with_a_wild_program(&mut game);

        // Index 1 is Shield, which buffs DEF and must not heal.
        companion_uses_special(&mut game, medic, 1, battle::SpecialTarget::Ally { slot: 0 });
        assert_eq!(
            game.world.get::<Stats>(player).unwrap().hp,
            1,
            "picking Shield must not run Heal, the ability at index 0"
        );
        assert!(
            matches!(
                game.world.get::<CombatBuff>(player).and_then(|b| b.active),
                Some(ActiveBuff {
                    kind: BuffKind::Def,
                    ..
                })
            ),
            "picking Shield should raise DEF"
        );
    }

    #[test]
    fn award_player_xp_also_grants_party_members_half_as_much() {
        let mut game = Game::new(36, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let in_party = spawn_tamed(&mut game, 10, 3);
        game.add_companion(in_party).unwrap();
        let not_in_party = spawn_tamed(&mut game, 10, 3);

        game.award_player_xp(player, 10);

        assert_eq!(
            game.world.get::<Experience>(in_party).unwrap().xp,
            5,
            "a party member should gain half the player's XP"
        );
        assert_eq!(
            game.world.get::<Experience>(not_in_party).unwrap().xp,
            0,
            "a tamed program outside the party shouldn't gain any XP from a kill"
        );
    }

    #[test]
    fn award_player_xp_can_level_up_a_party_member_independently_of_the_player() {
        let mut game = Game::new(37, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.world
            .get_mut::<Experience>(companion)
            .unwrap()
            .xp_to_next = 5;
        game.add_companion(companion).unwrap();

        game.award_player_xp(player, 10);

        let exp = game.world.get::<Experience>(companion).unwrap();
        assert_eq!(
            exp.level, 2,
            "5 XP against a 5-XP requirement should level the companion up"
        );
    }

    #[test]
    fn higher_growth_multiplier_species_out_grows_a_baseline_one_via_award_party_xp() {
        let mut game = Game::new(419, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game.species_defs();
        let baseline_id = species
            .iter()
            .find(|s| s.growth_multiplier == progression::BASELINE_GROWTH_MULTIPLIER)
            .expect("base roster should have at least one baseline-growth species")
            .id
            .clone();
        let boosted_id = species
            .iter()
            .find(|s| s.growth_multiplier > progression::BASELINE_GROWTH_MULTIPLIER)
            .expect("base roster should have at least one higher-growth species")
            .id
            .clone();

        let spawn = |game: &mut Game, species: String| {
            game.world
                .spawn((
                    Creature { species },
                    Position { x: 3, y: 3 },
                    Stats {
                        hp: 100,
                        max_hp: 100,
                        atk: 10,
                        def: 10,
                    },
                    Tamed { owner: player },
                    Experience {
                        level: 1,
                        xp: 0,
                        xp_to_next: 1,
                    },
                ))
                .id()
        };
        let baseline = spawn(&mut game, baseline_id);
        let boosted = spawn(&mut game, boosted_id);
        game.add_companion(baseline).unwrap();
        game.add_companion(boosted).unwrap();

        // xp_to_next is rigged to 1 above, so any non-zero party XP levels
        // both companions up exactly once.
        game.award_player_xp(player, 2);

        let baseline_hp = game.world.get::<Stats>(baseline).unwrap().max_hp;
        let boosted_hp = game.world.get::<Stats>(boosted).unwrap().max_hp;
        assert!(
            boosted_hp > baseline_hp,
            "a higher growth_multiplier species should out-grow a baseline one: {boosted_hp} vs {baseline_hp}"
        );
    }

    #[test]
    fn spawn_wild_creature_rolls_individual_stat_variance_within_a_species() {
        let mut game = Game::new(420, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species_id = game.species_defs().into_iter().next().unwrap().id;
        for _ in 0..15 {
            game.spawn_wild_creature(&species_id, 5, 5);
        }

        let mut query = game
            .world
            .query_filtered::<(&Position, &Stats), With<Hostile>>();
        let max_hps: Vec<i32> = query
            .iter(&game.world)
            .filter(|(p, _)| p.x == 5 && p.y == 5)
            .map(|(_, s)| s.max_hp)
            .collect();
        assert_eq!(max_hps.len(), 15);
        assert!(
            max_hps.iter().any(|&hp| hp != max_hps[0]),
            "spawning the same species repeatedly should roll different individual stats, got {max_hps:?}"
        );
    }

    #[test]
    fn wild_spawn_cap_is_not_exhausted_by_tamed_creatures() {
        let mut game = Game::new(422, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species_id = game.species_defs().into_iter().next().unwrap().id;
        for _ in 0..24 {
            game.world.spawn((
                Creature {
                    species: species_id.clone(),
                },
                Position { x: 0, y: 0 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
                Tamed { owner: player },
            ));
        }

        // `Game::new` already seeds 14 initial (hostile) wild creatures, so
        // the true wild population here is 14 — comfortably under any
        // reasonable cap — even though total `Creature` entities (wild +
        // tamed) is 38.
        let mut creature_query = game.world.query_filtered::<(), With<Creature>>();
        let before = creature_query.iter(&game.world).count();

        for _ in 0..500 {
            game.maybe_spawn_wild_creature();
        }

        let after = creature_query.iter(&game.world).count();
        assert!(
            after > before,
            "wild creatures should still be able to spawn even when the map already has \
             24 tamed (non-hostile) programs on it, but the population stayed at {before} \
             after 500 attempts"
        );
    }

    #[test]
    fn a_full_wild_population_far_away_is_culled_so_spawns_near_the_player_still_happen() {
        let mut game = Game::new(423, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species_id = game.species_defs().into_iter().next().unwrap().id;
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();

        // Fill the cap with a wild population the player wandered away from,
        // far outside the (-12..=12) radius `maybe_spawn_wild_creature` ever
        // spawns into around the player's *current* position.
        let mut hostile_query = game.world.query_filtered::<(), With<Hostile>>();
        let already = hostile_query.iter(&game.world).count();
        let distant: Vec<Entity> = (0..WILD_CREATURE_CAP - already)
            .map(|_| {
                game.world
                    .spawn((
                        Creature {
                            species: species_id.clone(),
                        },
                        Position {
                            x: player_pos.x + 500,
                            y: player_pos.y + 500,
                        },
                        Stats {
                            hp: 10,
                            max_hp: 10,
                            atk: 1,
                            def: 1,
                        },
                        Hostile,
                    ))
                    .id()
            })
            .collect();

        let mut nearby_query = game.world.query_filtered::<&Position, With<Hostile>>();
        let nearby_before = nearby_query
            .iter(&game.world)
            .filter(|p| (p.x - player_pos.x).abs() <= 20 && (p.y - player_pos.y).abs() <= 20)
            .count();

        for _ in 0..500 {
            game.maybe_spawn_wild_creature();
        }

        let nearby_after = nearby_query
            .iter(&game.world)
            .filter(|p| (p.x - player_pos.x).abs() <= 20 && (p.y - player_pos.y).abs() <= 20)
            .count();

        assert!(
            nearby_after > nearby_before,
            "a wild population the player left behind elsewhere on the map shouldn't be able \
             to block new spawns near the player's current position, but nothing spawned \
             nearby in 500 attempts (nearby count stayed at {nearby_before})"
        );

        let surviving_distant = distant
            .iter()
            .filter(|&&e| game.world.get_entity(e).is_ok())
            .count();
        assert!(
            surviving_distant < distant.len(),
            "the distant population should have been culled to make room, but all \
             {} of them survived",
            distant.len()
        );
    }

    #[test]
    fn nest_guardians_are_eligible_to_be_culled_for_spawn_room() {
        let mut game = Game::new(424, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species_id = game.species_defs().into_iter().next().unwrap().id;
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();

        let nest = game
            .world
            .spawn((
                Nest {
                    species: species_id.clone(),
                    pending_respawns: Vec::new(),
                },
                Position {
                    x: player_pos.x + 500,
                    y: player_pos.y + 500,
                },
                Durability {
                    hp: 100,
                    max_hp: 100,
                },
            ))
            .id();

        // Fill the cap entirely with guardians of that far-away nest — the
        // farthest hostile from the player is always going to be one of them.
        let mut hostile_query = game.world.query_filtered::<(), With<Hostile>>();
        let already = hostile_query.iter(&game.world).count();
        for _ in 0..WILD_CREATURE_CAP - already {
            game.world.spawn((
                Creature {
                    species: species_id.clone(),
                },
                Position {
                    x: player_pos.x + 500,
                    y: player_pos.y + 500,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
            ));
        }

        let nearby_before = {
            let mut query = game.world.query_filtered::<&Position, With<Hostile>>();
            query
                .iter(&game.world)
                .filter(|p| (p.x - player_pos.x).abs() <= 20 && (p.y - player_pos.y).abs() <= 20)
                .count()
        };

        for _ in 0..500 {
            game.maybe_spawn_wild_creature();
        }

        let mut hostile_query = game.world.query_filtered::<&Position, With<Hostile>>();
        let nearby_after = hostile_query
            .iter(&game.world)
            .filter(|p| (p.x - player_pos.x).abs() <= 20 && (p.y - player_pos.y).abs() <= 20)
            .count();
        assert!(
            nearby_after > nearby_before,
            "guardians of a nest the player left behind shouldn't block spawns near the \
             player, but nothing spawned nearby in 500 attempts"
        );

        let mut guardian_query = game.world.query_filtered::<(), With<NestGuardian>>();
        let guardians_left = guardian_query.iter(&game.world).count();
        assert!(
            guardians_left < WILD_CREATURE_CAP - already,
            "the farthest hostile should be culled even when it's a nest guardian, but \
             all {guardians_left} guardians survived"
        );
    }

    #[test]
    fn individual_growth_roll_scales_stat_gains_independently_of_species_growth_multiplier() {
        let mut game = Game::new(421, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species_id = game.species_defs().into_iter().next().unwrap().id;

        let low_roll = game
            .world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Position { x: 3, y: 3 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 10,
                    def: 10,
                },
                Potential {
                    hp_roll: 1.0,
                    atk_roll: 1.0,
                    def_roll: 1.0,
                    growth_roll: MIN_INDIVIDUAL_ROLL,
                },
                Tamed { owner: player },
                Experience {
                    level: 1,
                    xp: 0,
                    xp_to_next: 1,
                },
            ))
            .id();
        let high_roll = game
            .world
            .spawn((
                Creature {
                    species: species_id,
                },
                Position { x: 3, y: 3 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 10,
                    def: 10,
                },
                Potential {
                    hp_roll: 1.0,
                    atk_roll: 1.0,
                    def_roll: 1.0,
                    growth_roll: MAX_INDIVIDUAL_ROLL,
                },
                Tamed { owner: player },
                Experience {
                    level: 1,
                    xp: 0,
                    xp_to_next: 1,
                },
            ))
            .id();
        game.add_companion(low_roll).unwrap();
        game.add_companion(high_roll).unwrap();

        // xp_to_next is rigged to 1 above, so any non-zero party XP levels
        // both companions up exactly once, at the same species (and so the
        // same growth_multiplier) — only their individual growth_roll differs.
        game.award_player_xp(player, 2);

        let low_hp = game.world.get::<Stats>(low_roll).unwrap().max_hp;
        let high_hp = game.world.get::<Stats>(high_roll).unwrap().max_hp;
        assert!(
            high_hp > low_hp,
            "a higher individual growth_roll should out-grow a lower one at the same species: {high_hp} vs {low_hp}"
        );
    }

    #[test]
    fn fuse_companions_averages_the_parents_potential() {
        let mut game = Game::new(422, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game.species_defs();
        let species_a = species[0].id.clone();
        let species_b = species[1 % species.len()].id.clone();

        let a = game
            .world
            .spawn((
                Creature { species: species_a },
                Position { x: 3, y: 3 },
                Stats {
                    hp: 20,
                    max_hp: 20,
                    atk: 10,
                    def: 4,
                },
                Potential {
                    hp_roll: 0.8,
                    atk_roll: 0.8,
                    def_roll: 0.8,
                    growth_roll: 0.8,
                },
                Tamed { owner: player },
                Experience {
                    level: 5,
                    xp: 3,
                    xp_to_next: 100,
                },
            ))
            .id();
        let b = game
            .world
            .spawn((
                Creature { species: species_b },
                Position { x: 4, y: 4 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 6,
                    def: 2,
                },
                Potential {
                    hp_roll: 1.2,
                    atk_roll: 1.2,
                    def_roll: 1.2,
                    growth_roll: 1.2,
                },
                Tamed { owner: player },
                Experience {
                    level: 2,
                    xp: 1,
                    xp_to_next: 40,
                },
            ))
            .id();

        game.fuse_companions(a, b, None).unwrap();

        let mut query = game.world.query::<(&Potential, &Tamed)>();
        let (potential, _) = query
            .iter(&game.world)
            .find(|(_, t)| t.owner == player)
            .expect("a fused creature should exist");
        assert_eq!(
            potential.hp_roll, 1.0,
            "fused rolls should average the two parents'"
        );
        assert_eq!(potential.growth_roll, 1.0);
    }

    #[test]
    fn a_creatures_potential_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(423, DifficultyMode::Forgiving, &assets).unwrap();
        let player = game.player_entity();
        let species = game.species_defs().into_iter().next().unwrap();
        game.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 1,
                def: 1,
            },
            Potential {
                hp_roll: 1.15,
                atk_roll: 0.85,
                def_roll: 1.05,
                growth_roll: 1.2,
            },
            Tamed { owner: player },
            Experience::default(),
        ));

        let path = std::env::temp_dir().join(format!(
            "feral_processes_potential_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut query = loaded.world.query::<(&Potential, &Tamed)>();
        let (potential, _) = query
            .iter(&loaded.world)
            .find(|(_, t)| t.owner == player)
            .expect("restored creature should still have its Potential");
        assert_eq!(potential.hp_roll, 1.15);
        assert_eq!(potential.atk_roll, 0.85);
        assert_eq!(potential.def_roll, 1.05);
        assert_eq!(potential.growth_roll, 1.2);
    }

    #[test]
    fn player_level_up_message_is_tagged_message_kind_level_up() {
        let mut game = Game::new(39, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Experience>(player).unwrap().xp_to_next = 5;

        game.award_player_xp(player, 5);

        let tagged = game
            .message_log(10)
            .into_iter()
            .any(|(kind, text)| kind == MessageKind::LevelUp && text.contains("reach level"));
        assert!(
            tagged,
            "leveling up should log a MessageKind::LevelUp line, got: {:?}",
            game.message_log(10)
        );
    }

    #[test]
    fn killing_a_wild_creature_in_battle_awards_the_active_companion_half_xp() {
        let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();

        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        player_attacks(&mut game);

        assert_eq!(
            game.world.get::<Experience>(companion).unwrap().xp,
            5,
            "killing a 10-max-HP wild program should award the party member half its max HP as XP"
        );
    }

    /// `wild_retaliate` rolls per-call whether a companion soaks the hit, so
    /// this drives it across many seeds and checks both outcomes occur —
    /// proof the roll is live, not that any single call behaves one way.
    #[test]
    fn wild_retaliation_can_land_on_either_the_player_or_the_companion() {
        let species_id = {
            let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.species_defs()
                .into_iter()
                .next()
                .expect("at least one species")
                .id
                .clone()
        };

        let mut companion_hit = false;
        let mut player_hit = false;

        for seed in 0..60u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let player = game.player_entity();
            let companion = spawn_tamed(&mut game, 1000, 1);
            game.add_companion(companion).unwrap();
            let player_hp_before = game.world.get::<Stats>(player).unwrap().hp;

            let wild = game
                .world
                .spawn((
                    Creature {
                        species: species_id.clone(),
                    },
                    Hostile,
                    Position { x: 5, y: 5 },
                    Stats {
                        hp: 1000,
                        max_hp: 1000,
                        atk: 5,
                        def: 0,
                    },
                ))
                .id();
            insert_battle(&mut game, player, vec![wild]);

            player_attacks(&mut game);

            let companion_hp = game.world.get::<Stats>(companion).unwrap().hp;
            let player_hp_after = game.world.get::<Stats>(player).unwrap().hp;
            if companion_hp < 1000 {
                companion_hit = true;
            }
            if player_hp_after < player_hp_before {
                player_hit = true;
            }
            if companion_hit && player_hit {
                break;
            }
        }

        assert!(
            companion_hit,
            "across 60 battles, the companion should have taken at least one hit"
        );
        assert!(
            player_hit,
            "across 60 battles, the player should have taken at least one hit"
        );
    }

    #[test]
    fn effective_def_excludes_the_players_party_bonus_when_a_companion_is_the_target() {
        // `wild_retaliate` calls `effective_def` on whichever entity got
        // hit — the player, or (per the test above) a companion. The
        // player's passive party bonus (see `party_stat_bonus`) must only
        // ever land on the player, never get double-applied to a
        // companion's own defense just because it's a party member too.
        let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 30);
        game.world.get_mut::<Stats>(a).unwrap().def = 20;
        game.add_companion(a).unwrap();
        // A second party member gives the *player's* bonus a nonzero,
        // easy-to-notice value if it ever leaked onto `a`.
        let b = spawn_tamed(&mut game, 10, 200);
        game.add_companion(b).unwrap();

        let raw_def = game.world.get::<Stats>(a).unwrap().def;
        assert_eq!(
            game.effective_def(a),
            raw_def,
            "a companion's effective DEF as a retaliation target must be its own raw Stats, \
             not inflated by the player's party bonus"
        );
    }

    #[test]
    fn a_knocked_out_companion_stands_down_once_the_battle_ends() {
        let species_id = {
            let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.species_defs()
                .into_iter()
                .next()
                .expect("at least one species")
                .id
                .clone()
        };

        // The companion-targeting roll is 30% per call; a 1-HP companion is
        // guaranteed to hit 0 the moment it's targeted (damage is always
        // >= 1). Across 60 seeds the odds of never once rolling the
        // companion are astronomically small, so this deterministically
        // exercises the knockout path without needing to fake the RNG.
        for seed in 0..60u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let player = game.player_entity();
            let companion = spawn_tamed(&mut game, 1, 1);
            game.add_companion(companion).unwrap();

            let wild = game
                .world
                .spawn((
                    Creature {
                        species: species_id.clone(),
                    },
                    Hostile,
                    Position { x: 5, y: 5 },
                    Stats {
                        hp: 1000,
                        max_hp: 1000,
                        atk: 50,
                        def: 0,
                    },
                ))
                .id();
            insert_battle(&mut game, player, vec![wild]);

            game.wild_retaliate(wild, 0, player);
            if game.world.get::<Stats>(companion).unwrap().hp == 0 {
                // It keeps its place while the fight runs: `planned` indexes
                // `Party` positionally, so removing it here would shift every
                // member behind it into the wrong slot.
                assert_eq!(
                    game.player_status().companions.len(),
                    1,
                    "a downed companion holds its slot until the battle ends"
                );
                game.battle_flee();
                assert!(
                    game.player_status().companions.is_empty(),
                    "ending the battle should have stood the downed companion down"
                );
                return;
            }
        }
        panic!("companion was never targeted across 60 seeds — retaliation roll may be broken");
    }

    #[test]
    fn companion_status_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(28, DifficultyMode::Forgiving, &assets).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.add_companion(worker).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_companion_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let status = loaded.player_status();
        assert!(
            !status.companions.is_empty(),
            "the active companion should survive a save/load round trip"
        );
    }

    #[test]
    fn party_accepts_up_to_max_party_size_and_rejects_beyond_that() {
        let mut game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let members: Vec<Entity> = (0..MAX_PARTY_SIZE)
            .map(|_| spawn_tamed(&mut game, 10, 3))
            .collect();
        for &m in &members {
            game.add_companion(m).unwrap();
        }
        assert_eq!(game.player_status().companions.len(), MAX_PARTY_SIZE);

        let one_too_many = spawn_tamed(&mut game, 10, 3);
        assert!(
            game.add_companion(one_too_many).is_err(),
            "adding a 4th member to a full 3-slot party should fail"
        );
        assert_eq!(game.player_status().companions.len(), MAX_PARTY_SIZE);
    }

    #[test]
    fn pet_count_tallies_every_owned_program_regardless_of_party_membership() {
        let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(game.pet_count(), 0);
        let a = spawn_tamed(&mut game, 10, 3);
        let _b = spawn_tamed(&mut game, 10, 3);
        assert_eq!(game.pet_count(), 2, "both owned programs count as pets");
        // Adding one to the active party doesn't change the total owned.
        game.add_companion(a).unwrap();
        assert_eq!(game.pet_count(), 2, "a party member is still a pet");
    }

    #[test]
    fn taming_is_refused_when_the_roster_is_full_and_a_data_cache_makes_room() {
        let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Fill the base roster of 3 owned pets.
        for _ in 0..BASE_PET_CAPACITY {
            spawn_tamed(&mut game, 10, 3);
        }
        assert_eq!(game.pet_count(), BASE_PET_CAPACITY);

        start_battle_with_a_wild_program(&mut game);
        set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);
        player_decompiles(&mut game);

        let held = |g: &Game| {
            g.world
                .get::<Inventory>(g.player_entity())
                .unwrap()
                .count(&ItemId::from(ids::ICE_BREAKER))
        };
        assert_eq!(
            held(&game),
            1,
            "a full roster must refuse before the catalyst is spent"
        );
        assert!(
            game.message_log(usize::MAX)
                .into_iter()
                .any(|(_, l)| l.contains("roster is full")),
            "the refusal should say the roster is full"
        );

        // A Data Cache raises the cap to 5, so the same attempt now has room
        // and runs (spending the catalyst) instead of being refused.
        spawn_data_cache(&mut game, 1);
        assert_eq!(game.pet_capacity(), BASE_PET_CAPACITY + 2);
        player_decompiles(&mut game);
        assert_eq!(
            held(&game),
            0,
            "with a cache deployed the roster has room, so the decompile runs and spends the catalyst"
        );
    }

    #[test]
    fn adding_the_same_companion_twice_is_rejected() {
        let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();
        assert!(
            game.add_companion(companion).is_err(),
            "a program already in the party can't be added again"
        );
        assert_eq!(game.player_status().companions.len(), 1);
    }

    #[test]
    fn removing_one_party_member_leaves_the_others_active() {
        let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.add_companion(a).unwrap();
        game.add_companion(b).unwrap();

        game.remove_companion(a);

        assert_eq!(game.player_status().companions.len(), 1);
        assert!(
            game.player_status()
                .companions
                .first()
                .is_some_and(|c| c.hp == 10)
        );
        assert!(!game.world.resource::<Party>().0.contains(&a));
        assert!(game.world.resource::<Party>().0.contains(&b));
    }

    #[test]
    fn party_members_grant_a_passive_ten_percent_atk_def_bonus_that_stacks_updates_live_and_disappears_on_removal()
     {
        let mut game = Game::new(75, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let base_atk = game.player_status().atk;
        let base_def = game.player_status().def;

        // `spawn_tamed` fixes def at 1, so 10% of it floors to 0 and should
        // clamp up to the stated minimum of 1 rather than contributing 0.
        let a = spawn_tamed(&mut game, 10, 30);
        game.add_companion(a).unwrap();
        let status = game.player_status();
        assert_eq!(status.atk, base_atk + 3, "10% of a's 30 ATK is 3");
        assert_eq!(
            status.def,
            base_def + 1,
            "10% of a's 1 DEF floors to 0, minimum 1 applies"
        );

        // A second party member's bonus stacks on top of the first's.
        let b = spawn_tamed(&mut game, 10, 50);
        game.add_companion(b).unwrap();
        let status = game.player_status();
        assert_eq!(
            status.atk,
            base_atk + 3 + 5,
            "10% of b's 50 ATK is 5, stacked with a's"
        );
        assert_eq!(status.def, base_def + 1 + 1);

        // The bonus is computed live from each companion's current Stats,
        // not baked in at add_companion time — a level-up (simulated here
        // by mutating Stats directly, same as `progression::add_xp` would)
        // should be reflected immediately with no extra bookkeeping.
        game.world.get_mut::<Stats>(a).unwrap().atk = 60;
        let status = game.player_status();
        assert_eq!(
            status.atk,
            base_atk + 6 + 5,
            "a's stronger ATK should raise its contribution"
        );

        game.remove_companion(a);
        game.remove_companion(b);
        let status = game.player_status();
        assert_eq!(
            status.atk, base_atk,
            "bonus should vanish once every companion leaves the party"
        );
        assert_eq!(status.def, base_def);
    }

    #[test]
    fn dropping_below_half_power_weakens_the_players_attack() {
        let mut game = Game::new(76, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let full_atk = game.player_status().atk;

        // At and above the threshold, no penalty at all.
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        assert_eq!(
            game.player_status().atk,
            full_atk,
            "50 power is still full strength"
        );

        // Below it, a linear falloff — checked at a couple of points rather
        // than re-deriving the formula, since `battle::power_attack_multiplier`
        // already has its own dedicated unit tests for the exact curve.
        game.world.get_mut::<Needs>(player).unwrap().hunger = 25.0;
        let quarter_power_atk = game.player_status().atk;
        assert!(
            quarter_power_atk < full_atk,
            "attack should be weaker at 25 power than at full power"
        );

        game.world.get_mut::<Needs>(player).unwrap().hunger = 0.0;
        let zero_power_atk = game.player_status().atk;
        assert!(
            zero_power_atk < quarter_power_atk,
            "attack should keep weakening as power keeps dropping"
        );
        assert_eq!(
            zero_power_atk,
            (full_atk as f32 * 0.5).round() as i32,
            "the penalty floors at half strength, even fully starved"
        );
    }

    #[test]
    fn a_special_is_refused_for_a_program_not_in_the_party() {
        let mut game = Game::new(73, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let not_in_party = spawn_tamed(&mut game, 10, 20);

        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 1,
                    def: 0,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);

        companion_uses_special(
            &mut game,
            not_in_party,
            0,
            battle::SpecialTarget::Ally { slot: 0 },
        );

        let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            wild_hp, 100,
            "a program outside the active party shouldn't be able to act in battle"
        );
    }

    #[test]
    fn rest_heals_every_party_member() {
        let mut game = Game::new(74, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.add_companion(a).unwrap();
        game.add_companion(b).unwrap();
        for e in [a, b] {
            game.world.get_mut::<Stats>(e).unwrap().hp = 1;
        }
        spawn_rest_structure_at_player(&mut game);

        game.rest();

        assert_eq!(game.world.get::<Stats>(a).unwrap().hp, 10);
        assert_eq!(game.world.get::<Stats>(b).unwrap().hp, 10);
    }

    #[test]
    fn recharger_node_loads_as_a_permanent_base_wide_power_source() {
        let game = Game::new(400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "recharger_node")
            .expect("recharger_node.ron should load");
        assert_eq!(def.build_cost, vec![(ItemId::from(ids::CORE_FRAGMENT), 10)]);
        let regen = def
            .power_regen
            .as_ref()
            .expect("the Recharger Node should regenerate Power");
        assert_eq!(regen.per_tick, 1.0);
        assert_eq!(
            regen.radius, MAX_BUILD_DISTANCE_FROM_HOME,
            "the Recharger Node should cover the whole base"
        );
        assert!(
            def.enables_rest.is_none(),
            "resting moved to Home; the Recharger Node is no longer a rest gate"
        );
        assert!(
            def.temporary.is_none(),
            "the Recharger Node should be a permanent structure"
        );
    }

    /// Deploys a Recharger Node `dx`/`dy` tiles from the player, bypassing
    /// `place_structure`'s Home and cost requirements — this is about the
    /// regen system, not the build rules.
    fn spawn_recharger_node(game: &mut Game, dx: i32, dy: i32) {
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.world.spawn((
            Structure {
                kind: "recharger_node".to_string(),
            },
            Position {
                x: player_pos.x + dx,
                y: player_pos.y + dy,
            },
        ));
    }

    #[test]
    fn a_recharger_node_in_range_nets_power_upward_on_a_real_tick() {
        let mut game = Game::new(403, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        spawn_recharger_node(&mut game, 0, 0);

        game.wait();

        let hunger = game.world.get::<Needs>(player).unwrap().hunger;
        assert!(
            (hunger - 50.85).abs() < 1e-4,
            "expected +1.0 regen less 0.15 decay, got {hunger}"
        );
    }

    #[test]
    fn a_recharger_node_past_the_base_footprint_does_not_reach_the_player() {
        let mut game = Game::new(404, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        spawn_recharger_node(&mut game, MAX_BUILD_DISTANCE_FROM_HOME + 1, 0);

        game.wait();

        let hunger = game.world.get::<Needs>(player).unwrap().hunger;
        assert!(
            (hunger - 49.85).abs() < 1e-4,
            "expected decay only, got {hunger}"
        );
    }

    #[test]
    fn reaching_a_recharger_node_while_drained_costs_no_integrity() {
        let mut game = Game::new(405, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 0.1;
        let before = *game.world.get::<Stats>(player).unwrap();
        spawn_recharger_node(&mut game, 0, 0);

        game.wait();

        let after = *game.world.get::<Stats>(player).unwrap();
        assert_eq!(
            after.hp, before.hp,
            "regen runs before decay, so arriving drained must not cost Integrity"
        );
    }

    #[test]
    fn home_enables_rest_across_the_whole_base_footprint() {
        let game = Game::new(402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "home")
            .expect("home.ron should load");
        assert_eq!(
            def.enables_rest
                .as_ref()
                .expect("Home should be the rest gate")
                .radius,
            MAX_BUILD_DISTANCE_FROM_HOME,
            "Home's rest radius should cover exactly the base footprint"
        );
    }

    #[test]
    fn rest_is_a_no_op_without_a_nearby_rest_structure() {
        let mut game = Game::new(401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut needs = game.world.get_mut::<Needs>(player).unwrap();
            needs.fatigue = 10.0;
        }

        game.rest();

        let needs = *game.world.get::<Needs>(player).unwrap();
        assert_eq!(
            needs.fatigue, 10.0,
            "resting with no Home in range shouldn't restore anything"
        );
    }

    #[test]
    fn fuse_companions_combines_stats_and_keeps_the_higher_level_species() {
        let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game.species_defs();
        let species_a = species[0].id.clone();
        let species_b = species[1 % species.len()].id.clone();

        let a = game
            .world
            .spawn((
                Creature { species: species_a },
                Position { x: 3, y: 3 },
                Stats {
                    hp: 20,
                    max_hp: 20,
                    atk: 10,
                    def: 4,
                },
                Tamed { owner: player },
                Experience {
                    level: 5,
                    xp: 3,
                    xp_to_next: 100,
                },
            ))
            .id();
        let b = game
            .world
            .spawn((
                Creature {
                    species: species_b.clone(),
                },
                Position { x: 4, y: 4 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 6,
                    def: 2,
                },
                Tamed { owner: player },
                Experience {
                    level: 2,
                    xp: 1,
                    xp_to_next: 40,
                },
            ))
            .id();

        game.fuse_companions(a, b, None).unwrap();

        assert!(
            game.world.get::<Creature>(a).is_none(),
            "the first input should be consumed"
        );
        assert!(
            game.world.get::<Creature>(b).is_none(),
            "the second input should be consumed"
        );

        let mut query = game
            .world
            .query::<(&Creature, &Stats, &Experience, &Tamed)>();
        let (creature, stats, exp, _) = query
            .iter(&game.world)
            .find(|(_, _, _, t)| t.owner == player)
            .expect("a fused creature should exist");
        assert_eq!(
            exp.level, 5,
            "fusion should keep the higher level (ties favor `a`)"
        );
        assert_eq!(exp.xp, 0);
        assert_eq!(exp.xp_to_next, progression::xp_for_level(5));
        assert_eq!(
            stats.max_hp,
            20 + 10 / 2,
            "fused HP should be higher + lower/2"
        );
        assert_eq!(stats.atk, 10 + 6 / 2);
        assert_eq!(stats.def, 4 + 2 / 2);
        assert_ne!(
            creature.species, species_b,
            "the lower-level input's species shouldn't win the tie"
        );
    }

    #[test]
    fn fuse_companions_applies_a_custom_name_truncated_to_the_max_length() {
        let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.fuse_companions(a, b, Some("Way Too Long A Name".to_string()))
            .unwrap();

        let fused = game.owned_pets();
        assert_eq!(
            fused.len(),
            1,
            "fusing two owned programs should leave exactly one"
        );
        // PetInfo::name is zone-tagged (every fused program gets
        // `ZonePortal(1)`, always shown per `entity_label`'s own test
        // coverage), so strip that " 1" suffix before checking the
        // truncated custom name itself.
        let base_name = fused[0]
            .name
            .strip_suffix(" 1")
            .expect("a freshly fused program should be zone-tagged");
        assert_eq!(
            base_name.chars().count(),
            MAX_CUSTOM_NAME_LEN,
            "an overlong custom name should be truncated, not rejected"
        );
        assert!(
            "Way Too Long A Name".starts_with(base_name),
            "the truncated name should be a prefix of what was typed, got {base_name:?}"
        );
    }

    #[test]
    fn fuse_companions_with_no_name_or_blank_name_keeps_the_species_name() {
        let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // `spawn_tamed` always uses this same species (`species_defs()` is
        // stably sorted), and fusing two same-level, same-species programs
        // keeps it — capturing it directly here avoids having to pick the
        // fused entity back out of a world that also has 14 unrelated wild
        // creatures in it from `Game::new`.
        let species_name = game.species_defs().into_iter().next().unwrap().name;
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.fuse_companions(a, b, None).unwrap();
        let no_name = game.owned_pets();
        assert_eq!(no_name.len(), 1);
        // Every fused program gets `ZonePortal(1)` (see `fuse_companions`),
        // which `creature_label`/`PetInfo::name` always zone-tags — even at
        // zone 1, per `entity_label`'s own test coverage — so the expected
        // fallback name carries that same " 1" suffix, not the bare species name.
        let expected_default_name = format!("{species_name} 1");
        assert_eq!(
            no_name[0].name, expected_default_name,
            "no name given should fall back to the (zone-tagged) species name"
        );

        let c = spawn_tamed(&mut game, 10, 3);
        let d = spawn_tamed(&mut game, 10, 3);
        game.fuse_companions(c, d, Some("   ".to_string())).unwrap();
        let pets = game.owned_pets();
        let blank_named = pets.iter().find(|p| p.entity != no_name[0].entity).unwrap();
        assert_eq!(
            blank_named.name, expected_default_name,
            "an all-whitespace name should also fall back to the species name, not become blank"
        );
    }

    #[test]
    fn a_fused_programs_custom_name_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(92, DifficultyMode::Forgiving, &assets).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.fuse_companions(a, b, Some("Zappy".to_string()))
            .unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_fuse_name_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let pets = loaded.owned_pets();
        assert_eq!(pets.len(), 1);
        // Zone-tagged the same as any other fused program — see the
        // truncation test above for why " 1" is expected here too.
        assert_eq!(
            pets[0].name, "Zappy 1",
            "a custom name should survive a save/load round trip"
        );
    }

    #[test]
    fn fuse_companions_rejects_fusing_a_program_with_itself() {
        let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        assert!(game.fuse_companions(a, a, None).is_err());
    }

    #[test]
    fn fuse_companions_rejects_a_wild_creature() {
        let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let species = game.species_defs().into_iter().next().unwrap();
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 5,
                    max_hp: 5,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        assert!(game.fuse_companions(a, wild, None).is_err());
        assert!(
            game.world.get::<Creature>(a).is_some(),
            "a failed fusion shouldn't consume either input"
        );
        assert!(game.world.get::<Creature>(wild).is_some());
    }

    #[test]
    fn fuse_companions_removes_fused_members_from_the_active_party() {
        let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        game.add_companion(a).unwrap();
        game.add_companion(b).unwrap();

        game.fuse_companions(a, b, None).unwrap();

        assert!(!game.world.resource::<Party>().0.contains(&a));
        assert!(!game.world.resource::<Party>().0.contains(&b));
    }

    /// The player has no level ceiling, while their party members stop at
    /// `progression::CREATURE_MAX_LEVEL` — one big XP award should push
    /// the player past that ceiling and leave the companion pinned to it.
    #[test]
    fn player_levels_past_the_creature_cap_but_companions_dont() {
        let mut game = Game::new(105, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();

        // Party members earn half the player's award (PARTY_XP_DIVISOR),
        // so this is far past the cap for both of them.
        game.award_player_xp(player, 1_000_000);

        let player_level = game.world.get::<Experience>(player).unwrap().level;
        let companion_level = game.world.get::<Experience>(companion).unwrap().level;
        assert!(
            player_level > progression::CREATURE_MAX_LEVEL,
            "the player should keep leveling past the creature ceiling, got {player_level}"
        );
        assert_eq!(
            companion_level,
            progression::CREATURE_MAX_LEVEL,
            "a companion should still stop at the creature ceiling"
        );
    }

    /// Fuses `game`'s two freshest tamed programs together repeatedly to
    /// build up a lineage `depth` fusions deep, returning that program.
    fn fuse_to_depth(game: &mut Game, depth: u32) -> Entity {
        let mut current = spawn_tamed(game, 10, 3);
        for _ in 0..depth {
            let partner = spawn_tamed(game, 10, 3);
            game.fuse_companions(current, partner, None).unwrap();
            current = game
                .owned_pets()
                .into_iter()
                .max_by_key(|p| p.fusions)
                .expect("the fusion result should be owned")
                .entity;
        }
        current
    }

    #[test]
    fn fusing_two_fresh_programs_gives_a_result_one_fusion_deep() {
        let mut game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = spawn_tamed(&mut game, 10, 3);
        let b = spawn_tamed(&mut game, 10, 3);
        assert_eq!(game.fusion_count(a), 0, "a caught program starts unfused");

        game.fuse_companions(a, b, None).unwrap();

        let pets = game.owned_pets();
        assert_eq!(pets.len(), 1);
        assert_eq!(pets[0].fusions, 1);
    }

    #[test]
    fn a_fusion_result_is_one_deeper_than_its_deepest_input() {
        let mut game = Game::new(102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let deep = fuse_to_depth(&mut game, 2);
        let fresh = spawn_tamed(&mut game, 10, 3);
        assert_eq!(game.fusion_count(deep), 2);

        game.fuse_companions(deep, fresh, None).unwrap();

        let result = game
            .owned_pets()
            .into_iter()
            .max_by_key(|p| p.fusions)
            .unwrap();
        assert_eq!(
            result.fusions, 3,
            "depth should follow the deeper parent, not the sum of both"
        );
    }

    #[test]
    fn fuse_companions_rejects_a_program_already_at_the_fusion_cap() {
        let mut game = Game::new(103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let maxed = fuse_to_depth(&mut game, MAX_FUSIONS);
        assert_eq!(game.fusion_count(maxed), MAX_FUSIONS);
        let fresh = spawn_tamed(&mut game, 10, 3);
        let owned_before = game.owned_pets().len();

        assert!(
            game.fuse_companions(maxed, fresh, None).is_err(),
            "a maxed-out program shouldn't be usable as a fusion input"
        );
        // ...in either slot.
        assert!(game.fuse_companions(fresh, maxed, None).is_err());

        assert_eq!(
            game.owned_pets().len(),
            owned_before,
            "a rejected fusion shouldn't consume either input"
        );
    }

    #[test]
    fn fusion_depth_survives_a_save_load_round_trip() {
        let mut game = Game::new(104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let maxed = fuse_to_depth(&mut game, MAX_FUSIONS);
        game.add_companion(maxed).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_fusion_cap_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        let restored = loaded
            .owned_pets()
            .into_iter()
            .max_by_key(|p| p.fusions)
            .expect("the fused program should survive the round trip");
        assert_eq!(
            restored.fusions, MAX_FUSIONS,
            "a maxed lineage must stay maxed across a save, not reset to fusable"
        );
    }

    /// A trading structure, spawned without paying for it.
    fn spawn_market(game: &mut Game) -> Entity {
        let kind = game
            .structure_defs()
            .into_iter()
            .find(|d| {
                d.trade
                    .as_ref()
                    .is_some_and(|t| t.program_sell_divisor.is_some())
            })
            .expect("a trader that buys programs should ship")
            .id
            .clone();
        game.world
            .spawn((Structure { kind }, Position { x: 5, y: 5 }))
            .id()
    }

    fn fragments(game: &Game) -> u32 {
        game.world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT))
    }

    /// A guard used to read as being "on a cronjob" everywhere a program was
    /// listed: `PetInfo::job_structure` was `Task.target`'s label with no
    /// regard for `TaskKind`, and all three of its consumers wrapped it as
    /// "on a cronjob". Party membership was shown nowhere at all.
    #[test]
    fn program_activity_tells_a_guard_apart_from_a_worker() {
        let mut game = Game::new(130, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let idle = spawn_tamed(&mut game, 30, 5);
        let fighter = spawn_tamed(&mut game, 30, 5);
        let guard = spawn_tamed(&mut game, 30, 5);
        let market = spawn_market(&mut game);

        assert_eq!(game.program_activity(idle), "idle");

        game.add_companion(fighter).unwrap();
        assert_eq!(game.program_activity(fighter), "in party");

        game.assign_guard(guard, market).unwrap();
        let label = game.program_activity(guard);
        assert!(
            label.starts_with("guarding "),
            "a guard must not read as a worker, got {label:?}"
        );
        assert!(
            label.contains(&game.entity_label(market)),
            "and it must name what it is guarding, got {label:?}"
        );
    }

    /// A cronjob worker reads as the structure it works, with no verb — the
    /// bare name is what distinguishes it from a guard.
    #[test]
    fn program_activity_names_the_structure_a_worker_is_on() {
        let mut game = Game::new(131, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 30, 5);
        let node = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 4, y: 4 },
                ResourceNode {
                    resource: ItemId::from(ids::CORE_FRAGMENT),
                    amount: 5,
                    capacity: 5,
                    level: None,
                },
            ))
            .id();

        game.assign_cronjob(worker, node).unwrap();
        let label = game.program_activity(worker);
        assert_eq!(label, game.entity_label(node));
        assert!(
            !label.starts_with("guarding "),
            "a worker must not read as a guard"
        );
    }

    /// The trader's rows carry it too, so the screen that permanently erases
    /// a program says what that program is currently doing.
    #[test]
    fn a_sale_row_carries_the_programs_activity() {
        let mut game = Game::new(132, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let market = spawn_market(&mut game);
        let fighter = spawn_tamed(&mut game, 30, 5);
        game.add_companion(fighter).unwrap();

        let options = game.program_sale_options(market);
        let row = options
            .iter()
            .find(|o| o.entity == fighter)
            .expect("the party member is still sellable");
        assert_eq!(row.activity, "in party");
    }

    #[test]
    fn selling_a_program_pays_a_tenth_of_its_power_and_despawns_it() {
        let mut game = Game::new(120, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let market = spawn_market(&mut game);
        // power = max_hp + atk + def = 60 + 8 + 2 = 70, so 70/10 = 7.
        let pet = spawn_tamed(&mut game, 60, 8);
        game.world.get_mut::<Stats>(pet).unwrap().def = 2;

        let before = fragments(&game);
        game.sell_companion(market, pet).unwrap();

        assert_eq!(fragments(&game), before + 7, "a tenth of 70 power");
        assert!(
            game.world.get::<Stats>(pet).is_none(),
            "the sold program has to be gone, not merely stood down"
        );
    }

    /// The floor exists so a sale can never destroy a program for nothing.
    #[test]
    fn a_program_too_weak_to_price_still_sells_for_one_fragment() {
        let mut game = Game::new(121, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let market = spawn_market(&mut game);
        let pet = spawn_tamed(&mut game, 2, 1);
        game.world.get_mut::<Stats>(pet).unwrap().def = 0;

        let before = fragments(&game);
        game.sell_companion(market, pet).unwrap();
        assert_eq!(fragments(&game), before + 1, "3 power still pays 1, not 0");
    }

    /// `sell_companion` checks room for the payout before despawning, the
    /// same ordering `sell_item` documents. That guard cannot currently fire:
    /// `check_room` only refuses a bank-limited item, and the only shipped
    /// item with a `bank_limit` is Research Data, not the trade currency.
    ///
    /// It stays anyway, because which item is currency and whether it is
    /// banked are both `assets/items/` data — a mod can make this reachable
    /// without touching Rust. This test pins the assumption that makes the
    /// guard currently inert, so that if a future change banks the currency
    /// it fails here and points at the ordering rather than surfacing as
    /// programs vanishing for no payment.
    #[test]
    fn the_trade_currency_is_unbanked_so_a_payout_can_always_land() {
        let game = Game::new(122, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let currency = game.currency();
        assert_eq!(
            game.world
                .resource::<ItemDb>()
                .get(currency.as_str())
                .and_then(|d| d.bank_limit),
            None,
            "if the currency gains a bank_limit, re-check sell_companion's \
             check_room-before-despawn ordering — a refusal after the despawn \
             would destroy the program for nothing"
        );
    }

    /// Whatever the reason, a refused sale must leave the program alive.
    #[test]
    fn a_refused_sale_never_destroys_the_program() {
        let mut game = Game::new(127, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let market = spawn_market(&mut game);
        let pet = spawn_tamed(&mut game, 30, 5);

        // Not the player's to sell.
        let stranger = game.world.spawn(()).id();
        game.world.get_mut::<Tamed>(pet).unwrap().owner = stranger;
        assert!(game.sell_companion(market, pet).is_err());
        assert!(game.world.get::<Stats>(pet).is_some());

        // Mid-battle.
        game.world.get_mut::<Tamed>(pet).unwrap().owner = game.player_entity();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        game.start_battle(vec![wild]);
        assert!(game.sell_companion(market, pet).is_err());
        assert!(
            game.world.get::<Stats>(pet).is_some(),
            "a program must survive a sale refused mid-intrusion"
        );
    }

    #[test]
    fn a_trader_that_does_not_buy_programs_refuses() {
        let mut game = Game::new(123, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let kind = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_none())
            .expect("plenty of structures don't trade")
            .id
            .clone();
        let not_a_trader = game
            .world
            .spawn((Structure { kind }, Position { x: 5, y: 5 }))
            .id();
        let pet = spawn_tamed(&mut game, 30, 5);

        assert!(game.sell_companion(not_a_trader, pet).is_err());
        assert!(game.world.get::<Stats>(pet).is_some());
    }

    #[test]
    fn selling_detaches_the_program_from_its_party_slot_and_its_job() {
        let mut game = Game::new(124, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let market = spawn_market(&mut game);
        let worker = spawn_tamed(&mut game, 30, 5);
        let fighter = spawn_tamed(&mut game, 30, 5);
        game.add_companion(fighter).unwrap();
        game.assign_guard(worker, market).unwrap();
        assert!(game.world.get::<Task>(worker).is_some());

        game.sell_companion(market, worker).unwrap();
        game.sell_companion(market, fighter).unwrap();

        assert!(
            !game.world.resource::<Party>().0.contains(&fighter),
            "a sold party member must leave the party"
        );
        assert!(
            game.player_status().companions.is_empty(),
            "nothing sold should still be listed"
        );
    }

    /// The whole point of the feature: a full roster stops being a dead end.
    #[test]
    fn selling_a_program_frees_a_roster_slot() {
        let mut game = Game::new(125, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let market = spawn_market(&mut game);
        let capacity = game.pet_capacity();
        let pets: Vec<Entity> = (0..capacity)
            .map(|_| spawn_tamed(&mut game, 30, 5))
            .collect();
        assert_eq!(game.pet_count(), capacity, "roster should be full");

        game.sell_companion(market, pets[0]).unwrap();

        assert_eq!(
            game.pet_count(),
            capacity - 1,
            "selling has to free the slot, or the feature does nothing"
        );
    }

    #[test]
    fn program_sale_options_price_each_program_and_are_empty_for_a_non_buyer() {
        let mut game = Game::new(126, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let market = spawn_market(&mut game);
        let pet = spawn_tamed(&mut game, 60, 8);
        game.world.get_mut::<Stats>(pet).unwrap().def = 2;

        let options = game.program_sale_options(market);
        assert_eq!(options.len(), 1);
        assert_eq!(options[0].entity, pet);
        assert_eq!(options[0].power, 70);
        assert_eq!(options[0].payout, 7);

        let kind = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_none())
            .unwrap()
            .id
            .clone();
        let plain = game
            .world
            .spawn((Structure { kind }, Position { x: 6, y: 6 }))
            .id();
        assert!(game.program_sale_options(plain).is_empty());
    }

    #[test]
    fn sell_item_pays_out_core_fragments_at_the_structures_sell_rate() {
        let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_some())
            .expect("a trading structure (Black Market) should exist");
        let market = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();

        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::FIREWALL_PLATING), 3);
        let cf_before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT));

        game.sell_item(market, ItemId::from(ids::FIREWALL_PLATING), 2)
            .unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(&ItemId::from(ids::FIREWALL_PLATING)),
            1,
            "only the sold quantity should leave the inventory"
        );
        let sell_rate = def.trade.as_ref().unwrap().sell_rate;
        assert_eq!(
            inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
            cf_before + sell_rate * 2
        );
    }

    #[test]
    fn sell_item_rejects_core_fragments_and_items_you_dont_have() {
        let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_some())
            .unwrap();
        let market = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();

        assert!(
            game.sell_item(market, ItemId::from(ids::CORE_FRAGMENT), 1)
                .is_err()
        );
        assert!(
            game.sell_item(market, ItemId::from(ids::NEURAL_AMPLIFIER), 1)
                .is_err(),
            "can't sell what you don't have"
        );
    }

    #[test]
    fn buy_item_charges_core_fragments_and_grants_the_item() {
        let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_some())
            .unwrap();
        let (buy_item, unit_cost) = def.trade.as_ref().unwrap().buy[0].clone();
        let market = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.items.clear();
            inv.add(ItemId::from(ids::CORE_FRAGMENT), unit_cost * 2);
        }

        game.buy_item(market, buy_item.clone(), 2).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
            0,
            "the full cost should be charged"
        );
        assert_eq!(inv.count(&buy_item), 2);
    }

    #[test]
    fn buy_item_fails_without_enough_core_fragments_or_for_an_unlisted_item() {
        let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.trade.is_some())
            .unwrap();
        let (buy_item, _) = def.trade.as_ref().unwrap().buy[0].clone();
        let market = game
            .world
            .spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .items
            .clear();

        assert!(
            game.buy_item(market, buy_item, 1).is_err(),
            "no Core Fragments should fail the purchase"
        );
        assert!(
            game.buy_item(market, ItemId::from(ids::CORE_FRAGMENT), 1)
                .is_err(),
            "an item not on the buy list shouldn't be purchasable"
        );
    }

    #[test]
    fn damage_structure_destroys_it_and_clears_its_cronjob_at_zero_durability() {
        let mut game = Game::new(100, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 10, max_hp: 30 },
            ))
            .id();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 1,
            required: 5,
        });

        game.damage_structure(structure, 10, "Mining Node");

        assert!(
            game.world.get::<Structure>(structure).is_none(),
            "0 durability should destroy the structure"
        );
        assert!(
            game.world.get::<Task>(worker).is_none(),
            "the destroyed structure's cronjob should be cleared"
        );
    }

    #[test]
    fn damage_structure_just_reduces_durability_when_it_survives() {
        let mut game = Game::new(101, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 20, max_hp: 30 },
            ))
            .id();

        game.damage_structure(structure, 10, "Mining Node");

        assert_eq!(game.world.get::<Durability>(structure).unwrap().hp, 10);
        assert!(
            game.world.get::<Structure>(structure).is_some(),
            "a structure with remaining durability should survive"
        );
    }

    /// Finds the deployed Home, if any. Home is the only structure of its
    /// kind, so the first match is the only match.
    fn find_home(game: &mut Game) -> Option<Entity> {
        let mut query = game.world.query::<(Entity, &Structure)>();
        query
            .iter(&game.world)
            .find(|(_, s)| s.kind == HOME_STRUCTURE_ID)
            .map(|(e, _)| e)
    }

    #[test]
    fn home_loads_as_non_raidable_and_other_structures_default_to_raidable() {
        let game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let defs = game.structure_defs();

        let home = defs
            .iter()
            .find(|d| d.id == "home")
            .expect("home should load");
        assert!(!home.raidable, "home.ron must set raidable: false");

        let mining = defs
            .iter()
            .find(|d| d.id == "mining_node")
            .expect("mining_node should load");
        assert!(
            mining.raidable,
            "a structure file that omits `raidable` must default to raidable"
        );
    }

    #[test]
    fn deploying_home_gives_it_no_durability_pool() {
        let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, -1, 0);
        let home = find_home(&mut game).expect("place_home should have spawned a Home");

        assert!(
            game.world.get::<Durability>(home).is_none(),
            "a non-raidable structure must not carry a Durability pool at all"
        );
    }

    #[test]
    fn deploying_a_raidable_structure_still_gives_it_a_durability_pool() {
        // Seed 300 is known to have walkable terrain at both offsets — it's
        // the seed `place_structure_rejects_anything_but_home_until_a_home_exists`
        // already places two structures on.
        let mut game = Game::new(300, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, -1, 0);
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 20);
        game.place_structure("mining_node", 1, 0).unwrap();

        let node = {
            let mut query = game.world.query::<(Entity, &Structure)>();
            query
                .iter(&game.world)
                .find(|(_, s)| s.kind == "mining_node")
                .map(|(e, _)| e)
                .expect("the mining node should have been deployed")
        };

        let durability = game
            .world
            .get::<Durability>(node)
            .expect("a raidable structure must still get its Durability pool");
        assert_eq!(durability.hp, durability.max_hp);
        assert!(durability.max_hp > 0);
    }

    #[test]
    fn raid_check_never_targets_home_even_as_the_only_structure() {
        let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Strip every pre-existing Durability holder (habitat nests and
        // anything else the world seeded) so a raid has no legal target left
        // at all if Home genuinely isn't one.
        let existing: Vec<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<Durability>>();
            query.iter(&game.world).collect()
        };
        for e in existing {
            game.world.despawn(e);
        }
        place_home(&mut game, -1, 0);

        for _ in 0..500 {
            game.raid_check();
        }

        let home_still_standing = {
            let mut query = game.world.query::<&Structure>();
            query.iter(&game.world).any(|s| s.kind == HOME_STRUCTURE_ID)
        };
        assert!(
            home_still_standing,
            "Home must survive every raid roll — it can't be a raid target at all"
        );
        let home = find_home(&mut game).expect("checked above: Home is standing");
        assert!(
            game.world.get::<Durability>(home).is_none(),
            "Home must still have no Durability pool after the raid rolls"
        );
    }

    #[test]
    fn home_survives_save_and_load_without_gaining_a_durability_pool() {
        let assets = test_assets_dir();
        let mut game = Game::new(703, DifficultyMode::Forgiving, &assets).unwrap();
        place_home(&mut game, -1, 0);

        let path = std::env::temp_dir().join(format!(
            "feral_processes_home_raidable_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let home = find_home(&mut loaded).expect("Home should survive a save/load round trip");
        assert!(
            loaded.world.get::<Durability>(home).is_none(),
            "the load path must not re-attach Durability to a non-raidable structure"
        );
    }

    /// How many `raid_check` rolls each seed gets in the sweeps below.
    /// `RAID_CHANCE_PER_TICK` is a per-call roll, so a single call per seed
    /// leaves a ~2.7% chance of a 300-seed sweep never firing at all — which
    /// unsorted habitat lookup can turn from a stable pass into a flake by
    /// shifting RNG consumption between runs. Seven attempts takes that to
    /// ~1e-11. Every sweep returns on the first fire, so no target ever takes
    /// a second hit.
    const RAID_ATTEMPTS_PER_SEED: u32 = 7;

    #[test]
    fn raid_check_can_damage_an_undefended_structure() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let Some(durability) = game.world.get::<Durability>(structure) else {
                    // Destroyed outright — tolerate rather than assume it can't happen.
                    return;
                };
                if durability.hp < 30 {
                    return;
                }
            }
        }
        panic!(
            "raid_check never damaged the structure across 300 seeds — the raid roll may be broken"
        );
    }

    #[test]
    fn raid_damage_message_is_tagged_message_kind_raid() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.world.spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ));

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let tagged = game
                    .message_log(10)
                    .into_iter()
                    .any(|(kind, _)| kind == MessageKind::Raid);
                if tagged {
                    return;
                }
            }
        }
        panic!(
            "raid_check never logged a MessageKind::Raid line across 300 seeds — the raid roll may be broken"
        );
    }

    #[test]
    fn shield_structure_loads_with_no_work_and_a_raid_defense_bonus() {
        let game = Game::new(9, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "shield")
            .expect("shield.ron should load as a structure");
        assert!(
            def.work.is_none(),
            "a shield defends passively, not via cronjob work"
        );
        assert!(
            def.raid_defense > 0,
            "a shield should contribute a nonzero raid_defense bonus"
        );
    }

    #[test]
    fn deployed_shields_reduce_raid_damage_to_an_undefended_structure() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let shield_defense = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == "shield")
                .unwrap()
                .raid_defense;
            game.world.spawn((
                Structure {
                    kind: "shield".to_string(),
                },
                Position { x: 1, y: 1 },
            ));
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let Some(durability) = game.world.get::<Durability>(structure) else {
                    return;
                };
                if durability.hp < 30 {
                    assert_eq!(
                        durability.hp,
                        30 - (RAID_DAMAGE - shield_defense),
                        "a raid on an undefended structure should be reduced by the deployed shield's raid_defense"
                    );
                    return;
                }
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }

    #[test]
    fn damaging_a_structure_queues_a_hit_effect_at_its_position() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();

        game.damage_structure(structure, 5, "Mining Node");

        let effects = game.take_effects();
        assert_eq!(effects.len(), 1, "one hit should queue one effect");
        assert_eq!(effects[0].kind, EffectKind::Hit);
        assert_eq!(effects[0].pos, (5, 5));
    }

    #[test]
    fn destroying_a_structure_queues_a_destroyed_effect() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 2, y: 3 },
                Durability { hp: 4, max_hp: 30 },
            ))
            .id();

        game.damage_structure(structure, 10, "Mining Node");

        let effects = game.take_effects();
        assert_eq!(effects.len(), 1);
        assert_eq!(
            effects[0].kind,
            EffectKind::Destroyed,
            "a killing blow should queue Destroyed, not Hit"
        );
        assert_eq!(effects[0].pos, (2, 3));
    }

    #[test]
    fn damaging_a_structure_with_no_position_queues_nothing() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();

        game.damage_structure(structure, 5, "Mining Node");

        assert!(
            game.take_effects().is_empty(),
            "a flash with no known tile is worse than no flash"
        );
    }

    #[test]
    fn a_raid_fully_absorbed_by_the_shield_network_queues_a_deflected_effect() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            // Enough shields that RAID_DAMAGE is reduced to zero.
            let shield_defense = game
                .structure_defs()
                .into_iter()
                .find(|d| d.id == "shield")
                .unwrap()
                .raid_defense
                .max(1);
            let needed = RAID_DAMAGE.div_ceil(shield_defense);
            for _ in 0..needed {
                game.world.spawn((
                    Structure {
                        kind: "shield".to_string(),
                    },
                    Position { x: 1, y: 1 },
                ));
            }
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let effects = game.take_effects();
                if effects.is_empty() {
                    continue;
                }
                let target = effects
                    .iter()
                    .find(|e| e.pos == (5, 5))
                    .expect("the raid should have targeted the only durable structure");
                assert_eq!(
                    target.kind,
                    EffectKind::Deflected,
                    "a raid the shield network zeroes out should deflect, not hit"
                );
                assert_eq!(
                    game.world.get::<Durability>(structure).unwrap().hp,
                    30,
                    "a deflected raid should leave durability untouched"
                );
                return;
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }

    #[test]
    fn a_raid_fended_off_by_a_cronjob_worker_queues_a_deflected_effect() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();
            // Defense far above RAID_DAMAGE, so the worker fully mitigates.
            game.world.spawn((
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 1,
                    def: 500,
                },
                Position { x: 5, y: 5 },
                Task {
                    kind: TaskKind::Guard,
                    target: structure,
                    progress: 0,
                    required: 10,
                },
            ));

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let effects = game.take_effects();
                if effects.is_empty() {
                    continue;
                }
                assert_eq!(effects[0].kind, EffectKind::Deflected);
                assert_eq!(effects[0].pos, (5, 5));
                assert_eq!(
                    game.world.get::<Durability>(structure).unwrap().hp,
                    30,
                    "a fully mitigated raid should leave durability untouched"
                );
                return;
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }

    #[test]
    fn take_effects_drains_the_queue() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();

        game.damage_structure(structure, 1, "Mining Node");

        assert_eq!(game.take_effects().len(), 1);
        assert!(
            game.take_effects().is_empty(),
            "a second drain should come back empty"
        );
    }

    #[test]
    fn the_effect_queue_drops_the_oldest_effects_past_its_cap() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability {
                    hp: 10_000,
                    max_hp: 10_000,
                },
            ))
            .id();

        for _ in 0..(resources::EFFECT_QUEUE_CAP + 10) {
            game.damage_structure(structure, 1, "Mining Node");
        }

        assert_eq!(
            game.take_effects().len(),
            resources::EFFECT_QUEUE_CAP,
            "a frontend that never drains must not grow the queue without bound"
        );
    }

    #[test]
    fn raid_defense_active_tracks_whether_any_shield_is_standing() {
        let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(!game.raid_defense_active());
        game.world.spawn((
            Structure {
                kind: "shield".to_string(),
            },
            Position { x: 1, y: 1 },
        ));
        assert!(game.raid_defense_active());
    }

    #[test]
    fn assign_guard_refuses_a_structure_that_cant_be_raided() {
        let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let home = game
            .world
            .spawn((
                Structure {
                    kind: "home".to_string(),
                },
                Position { x: 5, y: 5 },
            ))
            .id();
        let worker = spawn_tamed(&mut game, 50, 3);

        let err = game
            .assign_guard(worker, home)
            .expect_err("guarding a non-raidable structure should be refused");
        assert!(err.contains("can't be raided"), "unexpected error: {err}");
        assert!(
            game.world.get::<Task>(worker).is_none(),
            "a refused guard must not leave a Task behind"
        );
    }

    #[test]
    fn assign_guard_defends_a_structure_with_no_work_recipe() {
        let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Terminal, not Home: Home is non-raidable now, so it's the one
        // structure a guard is refused on.
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "terminal".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();
        let worker = spawn_tamed(&mut game, 50, 3);

        game.assign_guard(worker, structure).unwrap();

        let task = game
            .world
            .get::<Task>(worker)
            .expect("guarding should assign a Task");
        assert_eq!(task.kind, TaskKind::Guard);
        assert_eq!(task.target, structure);
    }

    #[test]
    fn a_guard_task_never_produces_resources_even_on_a_workable_node() {
        let mut game = Game::new(5, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let worker = spawn_tamed(&mut game, 10, 3);
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 3, y: 4 },
                ResourceNode {
                    resource: ItemId::from(ids::CORE_FRAGMENT),
                    amount: 5,
                    capacity: 5,
                    level: None,
                },
            ))
            .id();
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::Guard,
            target: structure,
            progress: 0,
            required: 1,
        });

        for _ in 0..10 {
            game.tick();
        }

        assert_eq!(
            game.world.get::<ResourceNode>(structure).unwrap().amount,
            5,
            "a guard shouldn't advance the node's gather cycle at all"
        );
    }

    #[test]
    fn guard_assignment_on_a_non_resource_structure_survives_save_and_load() {
        let mut game = Game::new(6, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Terminal, not Home: Home is non-raidable, so guarding it is refused.
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "terminal".to_string(),
                },
                Position { x: 3, y: 3 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();
        let worker = spawn_tamed(&mut game, 10, 3);
        game.assign_guard(worker, structure).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_guard_test_{}_{}.bin",
            std::process::id(),
            6
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut query = loaded.world.query::<&Task>();
        let task = query
            .iter(&loaded.world)
            .next()
            .expect("restored creature should still have its guard assignment");
        assert_eq!(task.kind, TaskKind::Guard);
        let target_pos = loaded
            .world
            .get::<Position>(task.target)
            .expect("guard task target should resolve to the structure entity");
        assert_eq!((target_pos.x, target_pos.y), (3, 3));
    }

    #[test]
    fn raid_check_defended_by_a_worker_reduces_structure_damage_and_hurts_the_worker() {
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let structure = game
                .world
                .spawn((
                    Structure {
                        kind: "mining_node".to_string(),
                    },
                    Position { x: 5, y: 5 },
                    Durability { hp: 30, max_hp: 30 },
                ))
                .id();
            let worker = spawn_tamed(&mut game, 50, 3);
            game.world.get_mut::<Stats>(worker).unwrap().def = 100; // fully mitigates RAID_DAMAGE
            game.world.entity_mut(worker).insert(Task {
                kind: TaskKind::GatherResource,
                target: structure,
                progress: 0,
                required: 5,
            });

            for _ in 0..RAID_ATTEMPTS_PER_SEED {
                game.raid_check();

                let worker_hp = game.world.get::<Stats>(worker).unwrap().hp;
                if worker_hp < 50 {
                    // The raid rolled this attempt: the structure should be
                    // untouched (fully mitigated) and the worker should have
                    // taken the defender's cost.
                    assert_eq!(
                        game.world.get::<Durability>(structure).unwrap().hp,
                        30,
                        "a worker with overwhelming Defense should fully mitigate the raid"
                    );
                    assert_eq!(worker_hp, 50 - RAID_DEFENDER_DAMAGE);
                    return;
                }
            }
        }
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
    }

    /// Raids should be survivable attrition, not a countdown. Eight hits to
    /// destroy a default-durability structure is the property; the exact
    /// constants are free to move underneath it.
    #[test]
    fn a_structure_survives_seven_raids_worth_of_damage() {
        let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let durability = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "mining_node")
            .expect("mining_node.ron should load")
            .durability;
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability {
                    hp: durability,
                    max_hp: durability,
                },
            ))
            .id();

        for _ in 0..7 {
            game.damage_structure(structure, RAID_DAMAGE, "Mining Node");
        }

        assert!(
            game.world.get::<Durability>(structure).is_some(),
            "seven raids should not destroy a structure at full durability"
        );
    }

    /// One regen interval has to fully undo one raid, or the base loses the
    /// attrition race no matter how the player plays.
    #[test]
    fn one_regen_interval_fully_undoes_one_raids_damage() {
        let mut game = Game::new(12, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ))
            .id();

        game.damage_structure(structure, RAID_DAMAGE, "Mining Node");
        assert_eq!(
            game.world.get::<Durability>(structure).unwrap().hp,
            30 - RAID_DAMAGE,
            "the raid should have landed before regen is tested"
        );

        game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;
        game.structure_regen();

        assert_eq!(
            game.world.get::<Durability>(structure).unwrap().hp,
            30,
            "one regen interval should fully undo one raid's damage"
        );
    }

    /// The shield network should ramp, not cliff: the first Shield has to
    /// leave damage on the table, or `raid_defense` has drifted into
    /// granting total immunity for one build.
    #[test]
    fn a_single_shield_reduces_raid_damage_without_erasing_it() {
        let game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let shield_defense = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "shield")
            .expect("shield.ron should load")
            .raid_defense;

        assert!(
            shield_defense > 0,
            "a Shield that reduces nothing is not a Shield"
        );
        assert!(
            shield_defense < RAID_DAMAGE,
            "one Shield must not fully absorb a raid — the network should ramp, not cliff"
        );
    }

    #[test]
    fn structure_regen_heals_damaged_structures_over_time() {
        let mut game = Game::new(102, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 10, max_hp: 30 },
            ))
            .id();
        game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

        game.structure_regen();

        assert_eq!(
            game.world.get::<Durability>(structure).unwrap().hp,
            10 + STRUCTURE_REGEN_AMOUNT
        );
    }

    #[test]
    fn structure_regen_does_not_exceed_max_durability() {
        let mut game = Game::new(103, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 29, max_hp: 30 },
            ))
            .id();
        game.world.resource_mut::<GameClock>().tick = STRUCTURE_REGEN_INTERVAL;

        game.structure_regen();

        assert_eq!(game.world.get::<Durability>(structure).unwrap().hp, 30);
    }

    #[test]
    fn structures_survive_save_and_load_with_their_durability() {
        let assets = test_assets_dir();
        let mut game = Game::new(104, DifficultyMode::Forgiving, &assets).unwrap();
        let structure_def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "mining_node")
            .unwrap();
        game.world.spawn((
            Structure {
                kind: structure_def.id.clone(),
            },
            Position { x: 5, y: 5 },
            Durability {
                hp: 12,
                max_hp: structure_def.durability,
            },
        ));

        let path = std::env::temp_dir().join(format!(
            "feral_processes_structure_durability_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let mut query = loaded.world.query::<&Durability>();
        let durability = query
            .iter(&loaded.world)
            .next()
            .expect("the structure should survive a save/load round trip");
        assert_eq!(durability.hp, 12);
        assert_eq!(durability.max_hp, structure_def.durability);
    }

    #[test]
    fn difficulty_color_buckets_relative_power_into_con_colors() {
        assert_eq!(
            difficulty_color(50, 100, false),
            GlyphColor::Green,
            "much weaker than the player"
        );
        assert_eq!(
            difficulty_color(100, 100, false),
            GlyphColor::Yellow,
            "an even match"
        );
        assert_eq!(
            difficulty_color(140, 100, false),
            GlyphColor::Orange,
            "notably tougher"
        );
        assert_eq!(
            difficulty_color(200, 100, false),
            GlyphColor::Red,
            "far stronger than the player"
        );
    }

    #[test]
    fn difficulty_color_is_always_magenta_for_a_boss_regardless_of_power() {
        assert_eq!(difficulty_color(1, 1000, true), GlyphColor::Magenta);
        assert_eq!(difficulty_color(1000, 1, true), GlyphColor::Magenta);
    }

    #[test]
    fn difficulty_color_never_divides_by_zero_player_power() {
        assert_eq!(difficulty_color(10, 0, false), GlyphColor::Red);
    }

    #[test]
    fn forage_chance_applies_keen_scavenger_per_level_but_never_boosts_a_zero_chance_biome() {
        assert_eq!(forage_chance(Biome::OpenGrid, 0), 0.6);
        assert_eq!(
            forage_chance(Biome::OpenGrid, 1),
            0.6 + KEEN_SCAVENGER_BONUS_PER_LEVEL
        );
        assert_eq!(
            forage_chance(Biome::OpenGrid, 3),
            0.6 + KEEN_SCAVENGER_BONUS_PER_LEVEL * 3.0
        );
        assert_eq!(
            forage_chance(Biome::DataVoid, 1),
            0.0,
            "an unwalkable biome's 0% chance shouldn't be boosted into a nonzero one"
        );
        assert_eq!(
            forage_chance(Biome::Platform, 3),
            0.0,
            "a base platform is manufactured floor with nothing to scavenge, and no amount \
             of Keen Scavenger should turn a safe haven into a risk-free forage spot"
        );
    }

    #[test]
    fn unlock_perk_spends_points_and_can_be_bought_repeatedly() {
        let mut game = Game::new(110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 5;

        game.unlock_perk(Perk::KeenScavenger).unwrap();

        let status = game.player_status();
        assert_eq!(status.perk_points, 5 - Perk::KeenScavenger.cost());
        assert_eq!(status.unlocked_perks, vec![Perk::KeenScavenger]);
        assert_eq!(game.player_perk_level(Perk::KeenScavenger), 1);

        game.unlock_perk(Perk::KeenScavenger).unwrap();
        assert_eq!(
            game.player_perk_level(Perk::KeenScavenger),
            2,
            "buying the same perk again should stack another level, not be rejected"
        );
        assert_eq!(
            status.perk_points - Perk::KeenScavenger.cost(),
            game.player_status().perk_points
        );
    }

    #[test]
    fn unlock_perk_rejects_without_enough_points() {
        let mut game = Game::new(111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 0;

        assert!(game.unlock_perk(Perk::ExploitFocus).is_err());
        assert_eq!(game.player_perk_level(Perk::ExploitFocus), 0);
    }

    #[test]
    fn exploit_focus_boosts_effective_decompiler_skill_per_level() {
        let mut game = Game::new(112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one species");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 3, y: 3 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        let before = game.inspect(wild).unwrap().decompile_chance;

        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::ExploitFocus).unwrap();
        let after_one = game.inspect(wild).unwrap().decompile_chance;
        game.unlock_perk(Perk::ExploitFocus).unwrap();
        let after_two = game.inspect(wild).unwrap().decompile_chance;

        assert!(
            after_one > before,
            "Exploit Focus should raise the decompile chance shown for the same target"
        );
        assert!(
            after_two > after_one,
            "a second level of Exploit Focus should raise it further still"
        );
    }

    #[test]
    fn lean_compiler_discounts_craft_cost_per_level_but_never_below_one_each() {
        let mut game = Game::new(113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let base_cost = game.craft_cost(&ItemId::from(ids::POWER_CELL));
        assert_eq!(
            base_cost,
            vec![(ItemId::from(ids::CORE_FRAGMENT), POWER_CELL_CORE_COST)]
        );

        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::LeanCompiler).unwrap();
        let discounted = game.craft_cost(&ItemId::from(ids::POWER_CELL));
        assert_eq!(
            discounted,
            vec![(
                ItemId::from(ids::CORE_FRAGMENT),
                POWER_CELL_CORE_COST - LEAN_COMPILER_DISCOUNT_PER_LEVEL
            )]
        );

        for _ in 0..10 {
            game.world.get_mut::<Perks>(player).unwrap().points = 10;
            let _ = game.unlock_perk(Perk::LeanCompiler);
        }
        let floored = game.craft_cost(&ItemId::from(ids::POWER_CELL));
        assert_eq!(
            floored,
            vec![(ItemId::from(ids::CORE_FRAGMENT), 1)],
            "the discount should never drop the cost below 1"
        );
    }

    #[test]
    fn perk_state_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(114, DifficultyMode::Forgiving, &assets).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::LowPowerMode).unwrap();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::LowPowerMode).unwrap();
        let points_after_unlock = game.player_status().perk_points;

        let path = std::env::temp_dir().join(format!(
            "feral_processes_perk_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        let status = loaded.player_status();
        assert_eq!(status.perk_points, points_after_unlock);
        assert_eq!(
            status.unlocked_perks,
            vec![Perk::LowPowerMode, Perk::LowPowerMode]
        );
        assert_eq!(loaded.player_perk_level(Perk::LowPowerMode), 2);
    }

    #[test]
    fn attacker_perk_adds_permanent_atk_per_level() {
        let mut game = Game::new(115, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        let base_atk = game.player_status().atk;

        game.unlock_perk(Perk::Attacker).unwrap();
        assert_eq!(
            game.player_status().atk,
            base_atk + ATTACKER_BONUS_PER_LEVEL
        );

        game.unlock_perk(Perk::Attacker).unwrap();
        assert_eq!(
            game.player_status().atk,
            base_atk + ATTACKER_BONUS_PER_LEVEL * 2
        );
    }

    #[test]
    fn defender_perk_adds_permanent_def_per_level() {
        let mut game = Game::new(116, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        let base_def = game.player_status().def;

        game.unlock_perk(Perk::Defender).unwrap();
        assert_eq!(
            game.player_status().def,
            base_def + DEFENDER_BONUS_PER_LEVEL
        );
    }

    #[test]
    fn buffer_perk_adds_percent_max_hp_per_level_floored_and_fully_heals() {
        let mut game = Game::new(117, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        let base_max_hp = game.player_status().max_hp;
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.hp = 1;
        }

        game.unlock_perk(Perk::Buffer).unwrap();
        let status = game.player_status();
        // 1% of the starting max HP rounds to well under the floor, so the
        // minimum bonus is what actually applies here.
        assert_eq!(status.max_hp, base_max_hp + BUFFER_MIN_BONUS_PER_LEVEL);
        assert_eq!(
            status.hp, status.max_hp,
            "buying Buffer should fully heal, like a level-up does"
        );
    }

    #[test]
    fn buffer_perk_scales_past_the_floor_at_high_max_hp() {
        let mut game = Game::new(118, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.max_hp = 2000;
            stats.hp = 2000;
        }

        game.unlock_perk(Perk::Buffer).unwrap();
        let status = game.player_status();
        assert_eq!(
            status.max_hp, 2020,
            "1% of 2000 is 20, above the floor, so that's what should apply"
        );
    }

    #[test]
    fn entering_a_zone_portal_increments_zone_and_doubles_wild_stats() {
        let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(game.player_status().zone, 1);
        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();

        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));

        game.move_player(1, 0);

        assert_eq!(
            game.player_status().zone,
            2,
            "walking onto a zone portal should advance the zone level"
        );

        let species_db = game.species_defs();
        let mut query = game
            .world
            .query_filtered::<(&Creature, &Stats, &Position), With<Hostile>>();
        let results: Vec<_> = query
            .iter(&game.world)
            .map(|(c, s, p)| (c.species.clone(), s.max_hp, *p))
            .collect();
        assert!(
            !results.is_empty(),
            "zone 2 should have spawned wild creatures"
        );
        for (species_id, max_hp, _pos) in results {
            let species = species_db.iter().find(|s| s.id == species_id).unwrap();
            // Zone 2 doubles base stats at minimum (`ZoneLevel::stat_multiplier`);
            // `distance_stat_multiplier` can scale it up further (capped at
            // `MAX_DISTANCE_STAT_MULTIPLIER`) depending how far from the
            // zone's entry point it spawned, and each spawn's individual
            // `Potential::hp_roll` can additionally scale it within
            // `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`. Checked as a range
            // rather than an exact figure since `WanderAi` may have already
            // moved this creature from its spawn position by the time this
            // runs.
            assert!(
                (max_hp as f32) >= (species.base_hp as f32) * 2.0 * MIN_INDIVIDUAL_ROLL,
                "zone 2 wild creatures should have at least doubled stats, times the roll floor"
            );
            assert!(
                (max_hp as f32)
                    <= (species.base_hp as f32)
                        * 2.0
                        * MAX_DISTANCE_STAT_MULTIPLIER
                        * MAX_INDIVIDUAL_ROLL,
                "zone 2 wild creatures shouldn't exceed the zone doubling times the distance cap and roll ceiling"
            );
        }
    }

    #[test]
    fn distance_stat_multiplier_measures_from_the_zone_spawn_point_when_no_home_exists() {
        let game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();

        assert_eq!(
            game.distance_stat_multiplier(spawn.x, spawn.y),
            1.0,
            "right at the spawn point, distance shouldn't add any scaling"
        );
        assert_eq!(
            game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES - 1, spawn.y),
            1.0,
            "just short of a full step away should still read as no scaling"
        );
        assert!(
            (game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES, spawn.y) - 1.25)
                .abs()
                < f32::EPSILON,
            "one full step away should add one step of bonus"
        );
        assert!(
            (game.distance_stat_multiplier(spawn.x + DISTANCE_STAT_STEP_TILES * 2, spawn.y) - 1.5)
                .abs()
                < f32::EPSILON,
            "two full steps away should add two steps of bonus"
        );
        assert_eq!(
            game.distance_stat_multiplier(spawn.x + 10_000, spawn.y),
            MAX_DISTANCE_STAT_MULTIPLIER,
            "far enough away should cap rather than grow without bound"
        );
    }

    #[test]
    fn distance_stat_multiplier_treats_the_whole_platform_as_distance_zero() {
        let mut game = Game::new(930, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();
        place_home(&mut game, 0, 0);

        assert_eq!(
            game.distance_stat_multiplier(spawn.x + MAX_BUILD_DISTANCE_FROM_HOME, spawn.y),
            1.0,
            "the platform edge is still perfectly safe territory"
        );
        assert_eq!(
            game.distance_stat_multiplier(
                spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + DISTANCE_STAT_STEP_TILES - 1,
                spawn.y
            ),
            1.0,
            "one tile short of the first step past the edge is still unscaled"
        );
        assert!(
            (game.distance_stat_multiplier(
                spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + DISTANCE_STAT_STEP_TILES,
                spawn.y
            ) - 1.25)
                .abs()
                < f32::EPSILON,
            "the first step up lands one full step past the platform edge — 30 tiles from Home"
        );
        assert_eq!(
            game.distance_stat_multiplier(spawn.x + 10_000, spawn.y),
            MAX_DISTANCE_STAT_MULTIPLIER,
            "the cap is unchanged"
        );
    }

    #[test]
    fn max_pack_size_also_counts_from_the_platform_edge() {
        let mut game = Game::new(931, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.resource_mut::<ZoneLevel>().0 = 4;
        let spawn = *game.world.resource::<ZoneSpawnPoint>();
        place_home(&mut game, 0, 0);

        assert_eq!(
            game.max_pack_size(spawn.x + MAX_BUILD_DISTANCE_FROM_HOME, spawn.y),
            1,
            "packs shouldn't grow inside territory that's still stat-x1.0"
        );
        // The discriminating case: without the platform offset this is a
        // full PACK_SIZE_STEP_TILES from spawn and would already allow a
        // packmate. Measured from the platform edge it's only half a step.
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES, spawn.y),
            1,
            "a full step from spawn is only half a step from the platform edge"
        );
        assert_eq!(
            game.max_pack_size(
                spawn.x + MAX_BUILD_DISTANCE_FROM_HOME + PACK_SIZE_STEP_TILES,
                spawn.y
            ),
            2,
            "the first pack-size step lands one full step past the platform edge"
        );
    }

    #[test]
    fn max_pack_size_grows_with_zone_and_distance_and_caps_per_zone() {
        // No Home is placed, so there's no platform and distances count
        // straight from the spawn point — see the platform-edge test below
        // for the case where one exists.
        let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();

        assert_eq!(
            game.max_pack_size(spawn.x, spawn.y),
            1,
            "right at spawn, packs should always be solo"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES - 1, spawn.y),
            1,
            "just short of a full step away should still be solo"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES, spawn.y),
            2,
            "one full step away should allow a packmate"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES * 10, spawn.y),
            PACK_SIZE_PER_ZONE,
            "zone 1's cap should hold even far past the first step"
        );

        game.world.resource_mut::<ZoneLevel>().0 = 2;
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES, spawn.y),
            2,
            "zone 2 grows the same way per step, just with a higher cap"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES * 10, spawn.y),
            2 * PACK_SIZE_PER_ZONE,
            "far out in zone 2, the cap should be twice zone 1's"
        );

        // The absolute ceiling holds regardless of how deep the run gets —
        // otherwise a late-zone pack outgrows MAX_ENEMY_GROUPS entirely.
        game.world.resource_mut::<ZoneLevel>().0 = 99;
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES * 100, spawn.y),
            MAX_PACK_SIZE,
            "no zone may push a pack past MAX_PACK_SIZE"
        );
    }

    #[test]
    fn defeating_the_front_pack_member_continues_the_battle_against_the_next_one() {
        let species_id = {
            let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.species_defs()
                .into_iter()
                .next()
                .expect("at least one species")
                .id
                .clone()
        };
        let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut stats = game.world.get_mut::<Stats>(player).unwrap();
            stats.atk = 1000; // guarantees a one-shot kill on the front target below
        }
        let front = game
            .world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 0,
                },
            ))
            .id();
        let second = game
            .world
            .spawn((
                Creature {
                    species: species_id.clone(),
                },
                Hostile,
                Position { x: 6, y: 5 },
                Stats {
                    hp: 500,
                    max_hp: 500,
                    atk: 1,
                    def: 0,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![front, second]);

        player_attacks(&mut game);

        assert!(
            game.has_active_battle(),
            "a pack member is still alive, so the fight should continue rather than end"
        );
        let view = game
            .battle_view()
            .expect("battle should still be active with the second member up front");
        assert_eq!(
            view.groups.len(),
            1,
            "both members are the same species, so they share one group"
        );
        assert_eq!(
            view.groups[0].count, 1,
            "only the second (surviving) member should remain, now as the front"
        );
        assert_eq!(
            view.groups[0].front_hp, 500,
            "the new front should be the untouched second pack member"
        );
    }

    /// All-attack asks which group only when there is a choice to make. With
    /// a single group left the prompt is pure friction, which is the whole
    /// complaint this work started from.
    #[test]
    fn all_attack_needs_a_target_only_while_more_than_one_group_lives() {
        let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let mut species = game.species_defs().into_iter().map(|s| s.id);
        let first = species.next().unwrap();
        let second = species.next().expect("assets ship at least two species");

        let solo = game.spawn_wild_creature(&first, 5, 5).unwrap();
        insert_battle(&mut game, player, vec![solo]);
        let needs = |game: &Game| {
            game.battle_party_commands()
                .into_iter()
                .find(|c| c.kind == PartyCommandKind::AllAttack)
                .expect("all-attack should always be offered")
                .needs_target
        };
        assert!(
            !needs(&game),
            "one group means no choice, so all-attack shouldn't open a picker"
        );

        let a = game.spawn_wild_creature(&first, 5, 5).unwrap();
        let b = game.spawn_wild_creature(&second, 6, 5).unwrap();
        insert_battle(&mut game, player, vec![a, b]);
        assert_eq!(
            game.battle_view().unwrap().groups.len(),
            2,
            "two different species should partition into two groups — test premise"
        );
        assert!(
            needs(&game),
            "two groups means a real focus-fire choice, so all-attack must ask"
        );
    }

    /// The renderers draw this list verbatim instead of hardcoding strings.
    #[test]
    fn battle_party_commands_offers_all_attack_all_defend_and_jack_out() {
        let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game.species_defs().into_iter().next().unwrap().id;
        let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
        insert_battle(&mut game, player, vec![wild]);

        let commands = game.battle_party_commands();
        let keys: Vec<char> = commands.iter().map(|c| c.key).collect();
        assert_eq!(
            keys,
            vec!['A', 'D', 'j'],
            "uppercase for the party-wide pair, lowercase for jack out"
        );
        for command in &commands {
            assert!(
                command.label.contains(&format!("[{}]", command.key)),
                "{:?} advertises key {:?} but its label is {:?}",
                command.kind,
                command.key,
                command.label
            );
        }
    }

    /// `[A]`/`[D]` fill the party in one keypress, but must never overwrite a
    /// choice the player already made deliberately — they pressed it partway
    /// through planning, not before starting.
    #[test]
    fn battle_plan_remaining_fills_only_unplanned_slots() {
        let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game.species_defs().into_iter().next().unwrap().id;
        let companion = game.spawn_wild_creature(&species, 4, 5).unwrap();
        game.world.resource_mut::<Party>().0.push(companion);
        let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
        insert_battle(&mut game, player, vec![wild]);

        // Slot 0 (the player) picks for itself; slot 1 is left open.
        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .unwrap();
        game.battle_plan_remaining(BattleAction::Defend).unwrap();

        let planned = &game.world.resource::<BattleState>().planned;
        assert_eq!(
            planned[0],
            Some(BattleAction::Attack { group: 0 }),
            "the slot that was already planned must keep its own choice"
        );
        assert_eq!(
            planned[1],
            Some(BattleAction::Defend),
            "the open slot should have been filled"
        );
        assert!(
            game.battle_round_ready(),
            "every actionable slot is planned"
        );
    }

    /// A knocked-out companion's slot is skipped by `battle_active_slot` and
    /// doesn't block `battle_round_ready`. Filling it would hand an action to
    /// a member that can't take one.
    #[test]
    fn battle_plan_remaining_skips_a_slot_that_cannot_act() {
        let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let species = game.species_defs().into_iter().next().unwrap().id;
        let companion = game.spawn_wild_creature(&species, 4, 5).unwrap();
        game.world.resource_mut::<Party>().0.push(companion);
        let wild = game.spawn_wild_creature(&species, 5, 5).unwrap();
        insert_battle(&mut game, player, vec![wild]);

        // Drop the companion, so slot 1 can no longer act.
        game.world.get_mut::<Stats>(companion).unwrap().hp = 0;
        assert!(
            !game.slot_can_act(1),
            "a companion at 0 HP should not be able to act — test premise is wrong"
        );

        game.battle_plan_remaining(BattleAction::Defend).unwrap();

        let planned = &game.world.resource::<BattleState>().planned;
        assert_eq!(planned[0], Some(BattleAction::Defend));
        assert_eq!(
            planned[1], None,
            "a slot that can't act must stay unplanned, not be handed an action"
        );
    }

    /// Uppercase A and D became party-wide commands, which only works if the
    /// per-slot keys underneath them are Attack and Defend. Decompile moved
    /// off `d` to make room. Pinned here so a future re-key cannot silently
    /// swap a brace for a capture attempt that spends a taming catalyst.
    #[test]
    fn battle_action_keys_are_lowercase_with_defend_on_d_and_decompile_on_c() {
        let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        insert_battle(&mut game, player, vec![wild]);

        let options = game.battle_action_options(0);
        let key_for = |kind: ActionKind| {
            options
                .iter()
                .find(|o| o.kind == kind)
                .unwrap_or_else(|| panic!("the player's menu should offer {kind:?}"))
                .key
        };
        assert_eq!(key_for(ActionKind::Attack), 'a');
        assert_eq!(key_for(ActionKind::Defend), 'd');
        assert_eq!(key_for(ActionKind::Decompile), 'c');
        assert_eq!(key_for(ActionKind::UseItem), 'u');

        for option in &options {
            assert!(
                option.label.contains(&format!("[{}]", option.key)),
                "{:?} advertises key {:?} but its label is {:?} — the bracketed \
                 letter must be the lowercase key the player actually presses",
                option.kind,
                option.key,
                option.label
            );
        }
    }

    /// The resolve popup used to title itself with the round number. With the
    /// popup gone the log is the only place that boundary exists, so the
    /// separator has to be logged exactly once and numbered to match the
    /// planning header — not the post-increment round.
    #[test]
    fn resolving_a_round_logs_one_round_separator_numbered_for_the_round_that_ran() {
        let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        insert_battle(&mut game, player, vec![wild]);

        let round_before = game.battle_view().unwrap().round;
        game.battle_set_action(0, BattleAction::Defend).unwrap();
        game.battle_resolve_round();

        let separators: Vec<String> = game
            .message_log(200)
            .into_iter()
            .filter(|(kind, _)| *kind == MessageKind::Round)
            .map(|(_, text)| text)
            .collect();
        assert_eq!(
            separators.len(),
            1,
            "one resolved round should log exactly one separator, got {separators:?}"
        );
        assert!(
            separators[0].contains(&round_before.to_string()),
            "the separator should name the round that just ran ({round_before}), got {:?}",
            separators[0]
        );
    }

    /// The loss path: a round that kills the player has to end the fight,
    /// clearing `BattleState` so the game-over handling isn't left running
    /// against a battle that's still notionally active.
    /// Permadeath rather than Forgiving, because a Forgiving flatline
    /// soft-reboots the player back to life within the same tick — which
    /// would make "did the player die?" unobservable after the fact.
    #[test]
    fn a_round_that_kills_the_player_ends_the_battle() {
        let mut game = Game::new(96, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
        // A wild program that hits far harder than the player can survive,
        // and with enough HP that the player can't end it first.
        let wild = game.spawn_wild_creature("construct", 5, 5).unwrap();
        {
            let mut w = game.world.get_mut::<Stats>(wild).unwrap();
            w.hp = 100_000;
            w.max_hp = 100_000;
            w.atk = 100_000;
        }
        game.start_battle(vec![wild]);

        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .unwrap();
        game.battle_resolve_round();

        assert!(
            game.is_game_over().is_some(),
            "the setup should have flatlined the player outright"
        );
        assert!(
            !game.has_active_battle(),
            "a fight the player lost has to be over, not left active"
        );
    }

    /// A companion knocked offline mid-fight keeps its slot, at 0 HP. That
    /// slot must stop counting toward the round, or it sits forever
    /// awaiting an action that nothing can supply: the menu for a downed
    /// slot is empty, so no keypress can fill it and the round can never
    /// resolve. The player would be stuck with Jack Out as their only move.
    #[test]
    fn a_slot_whose_member_was_knocked_out_stops_holding_the_round_open() {
        let mut game = Game::new(94, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pet = spawn_tamed(&mut game, 30, 5);
        game.add_companion(pet).unwrap();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        game.start_battle(vec![wild]);
        assert_eq!(game.world.resource::<BattleState>().planned.len(), 2);

        // Exactly what `wild_retaliate` leaves behind when a companion hits
        // 0 HP: still in the party, holding its slot, unable to act.
        game.world.get_mut::<Stats>(pet).unwrap().hp = 0;

        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .unwrap();
        assert_eq!(
            game.battle_active_slot(),
            None,
            "the empty slot must not be waiting on an action nothing can give it"
        );
        assert!(
            game.battle_round_ready(),
            "with the only living member planned, the round has to be resolvable"
        );

        let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
        game.battle_resolve_round();
        assert!(
            game.world.get::<Stats>(wild).unwrap().hp < hp_before,
            "the round should actually have resolved"
        );
    }

    /// `BattleState::planned` indexes `Party` positionally (see
    /// `actor_entity`), so dropping a member the instant it falls shifts
    /// every member behind it forward a slot: the survivor answers to the
    /// fallen member's slot, inherits whatever was planned for it, and takes
    /// over its roster row. Membership therefore has to hold still for the
    /// whole battle, with `slot_can_act` — not removal — keeping a downed
    /// slot from holding the round open.
    #[test]
    fn a_companion_knocked_offline_keeps_its_slot_for_the_rest_of_the_battle() {
        let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let first = spawn_tamed(&mut game, 12, 5);
        let second = spawn_tamed(&mut game, 12, 5);
        game.add_companion(first).unwrap();
        game.add_companion(second).unwrap();

        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        {
            let mut w = game.world.get_mut::<Stats>(wild).unwrap();
            w.hp = 10_000;
            w.max_hp = 10_000;
            w.atk = 400;
        }
        game.start_battle(vec![wild]);
        // The player has to outlast the companions, or the battle ends
        // before the invariant can be observed.
        {
            let mut p = game.world.get_mut::<Stats>(game.player_entity()).unwrap();
            p.hp = 100_000;
            p.max_hp = 100_000;
        }

        let slot_owner: Vec<Entity> = game
            .battle_view()
            .unwrap()
            .party
            .iter()
            .map(|p| p.entity)
            .collect();
        assert_eq!(slot_owner.len(), 3, "player plus two companions");

        // Resolve until something falls. Bounded, and every round is a
        // no-choice Defend, so nothing here depends on the RNG landing a
        // particular way — only on the pack eventually connecting.
        let mut downed = false;
        for _ in 0..30 {
            if !game.has_active_battle() {
                break;
            }
            game.battle_plan_remaining(BattleAction::Defend).unwrap();
            game.battle_resolve_round();
            downed = [first, second]
                .iter()
                .any(|&e| game.world.get::<Stats>(e).is_none_or(|s| s.hp <= 0));
            if downed {
                break;
            }
        }
        assert!(downed, "the setup should have knocked a companion offline");
        assert!(
            game.has_active_battle(),
            "the fight has to still be running"
        );

        for (slot, &expected) in slot_owner.iter().enumerate() {
            assert_eq!(
                game.actor_entity(battle::Actor::Party(slot)),
                Some(expected),
                "slot {slot} changed hands mid-battle"
            );
        }
    }

    /// The planning API is the whole extensibility story: the engine emits
    /// the menu, renderers dispatch off it. A slot that does not exist must
    /// be refused rather than silently ignored.
    #[test]
    fn battle_set_action_refuses_a_slot_that_is_not_in_the_party() {
        let mut game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        game.start_battle(vec![wild]);

        // Slot 0 is the player and always exists; slot 1 needs a companion.
        assert!(
            game.battle_set_action(0, BattleAction::Attack { group: 0 })
                .is_ok()
        );
        let err = game
            .battle_set_action(1, BattleAction::Attack { group: 0 })
            .unwrap_err();
        assert!(
            err.contains("party"),
            "expected a party-slot error, got {err:?}"
        );
    }

    /// A Rally or Shield aimed at a companion has to die with the battle.
    /// `CombatBuff` only ticks down inside one, and `effective_atk` /
    /// `effective_def` read it unconditionally, so a survivor carries a free
    /// stat bonus onto the overworld and into every fight after it.
    #[test]
    fn a_buff_aimed_at_a_companion_does_not_outlive_the_battle() {
        let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pet = spawn_tamed(&mut game, 30, 5);
        game.add_companion(pet).unwrap();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        game.start_battle(vec![wild]);

        let def_before = game.effective_def(pet);
        game.use_special_ability(
            &SpecialAbility::Shield {
                power: 4,
                duration: 3,
            },
            "Test",
            pet,
        );
        assert!(
            game.effective_def(pet) > def_before,
            "the shield should be up while the fight runs"
        );

        game.battle_flee();
        assert_eq!(
            game.effective_def(pet),
            def_before,
            "the buff must not outlive the battle"
        );
    }

    /// The same argument, for the two indices an ally-targeted Special
    /// carries beyond the acting slot. Unchecked, both resolve to `None`
    /// mid-round and cost the member its turn in silence — while the player
    /// is still charged the fatigue for commanding it.
    #[test]
    fn battle_set_action_refuses_an_out_of_range_ally_slot_or_ability() {
        let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pet = spawn_tamed(&mut game, 20, 5);
        game.add_companion(pet).unwrap();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        game.start_battle(vec![wild]);

        let abilities = game.battle_special_options(1).len();
        assert!(
            abilities >= 1,
            "every companion has at least the fallback rally"
        );

        let err = game
            .battle_set_action(
                1,
                BattleAction::Special {
                    ability: 0,
                    target: battle::SpecialTarget::Ally { slot: 42 },
                },
            )
            .unwrap_err();
        assert!(
            err.contains("party"),
            "expected a party-slot error, got {err:?}"
        );

        let err = game
            .battle_set_action(
                1,
                BattleAction::Special {
                    ability: abilities,
                    target: battle::SpecialTarget::Ally { slot: 0 },
                },
            )
            .unwrap_err();
        assert!(
            err.contains("ability"),
            "expected an ability error, got {err:?}"
        );

        assert!(
            game.battle_set_action(
                1,
                BattleAction::Special {
                    ability: 0,
                    target: battle::SpecialTarget::Ally { slot: 0 },
                },
            )
            .is_ok(),
            "a valid ability aimed at a real slot must still be accepted"
        );
    }

    #[test]
    fn battle_resolve_round_is_a_no_op_until_every_slot_is_planned() {
        let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pet = spawn_tamed(&mut game, 30, 6);
        game.add_companion(pet).unwrap();
        let wild = game.spawn_wild_creature("construct", 5, 5).unwrap();
        game.start_battle(vec![wild]);

        let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .unwrap();
        assert!(
            !game.battle_round_ready(),
            "the companion has no action yet"
        );
        game.battle_resolve_round();
        assert_eq!(
            game.world.get::<Stats>(wild).unwrap().hp,
            hp_before,
            "resolving a half-planned round must do nothing at all"
        );

        game.battle_set_action(1, BattleAction::Attack { group: 0 })
            .unwrap();
        assert!(game.battle_round_ready());
        game.battle_resolve_round();
        assert!(game.world.get::<Stats>(wild).unwrap().hp < hp_before);
    }

    /// Backing up a slot is how the player corrects a misclick — the cursor
    /// has to walk back, not just blank the entry.
    #[test]
    fn battle_clear_action_walks_the_active_slot_back() {
        let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        game.start_battle(vec![wild]);

        assert_eq!(game.battle_active_slot(), Some(0));
        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .unwrap();
        assert_eq!(
            game.battle_active_slot(),
            None,
            "solo party is fully planned"
        );
        game.battle_clear_action(0);
        assert_eq!(game.battle_active_slot(), Some(0));
    }

    /// Defend has to actually reduce incoming damage, or it's a wasted turn
    /// dressed up as a choice. Same seed both times: neither Defend nor the
    /// player's flat strike draws from the RNG, so the two runs stay in
    /// lockstep and the only difference is the DEF bonus.
    #[test]
    fn defending_reduces_the_damage_a_party_member_takes_this_round() {
        let damage_taken = |defend: bool| {
            let mut game = Game::new(89, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let wild = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
            game.start_battle(vec![wild]);
            let player = game.player_entity();
            let before = game.world.get::<Stats>(player).unwrap().hp;
            game.battle_set_action(
                0,
                if defend {
                    BattleAction::Defend
                } else {
                    BattleAction::Attack { group: 0 }
                },
            )
            .unwrap();
            game.battle_resolve_round();
            before - game.world.get::<Stats>(player).unwrap().hp
        };
        assert!(
            damage_taken(true) < damage_taken(false),
            "a defended round must cost less HP than an undefended one"
        );
    }

    /// Defend is offered to companions, so a companion must be able to hold
    /// the buff it grants. Only the player is spawned carrying a buff slot,
    /// so without inserting one on demand a companion's Defend would log
    /// its message and change nothing.
    #[test]
    fn a_companion_can_hold_the_buff_defend_grants() {
        let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pet = spawn_tamed(&mut game, 30, 5);
        game.add_companion(pet).unwrap();
        let raw_def = game.world.get::<Stats>(pet).unwrap().def;

        game.begin_defend(pet);

        assert_eq!(
            game.effective_def(pet),
            raw_def + DEFEND_DEF_BONUS,
            "a bracing companion must actually gain the DEF, not silently no-op"
        );
    }

    /// Soft ranks, not hard ones: a back-slot member is hit *less*, never
    /// *not at all*. Both halves matter — a version that made back slots
    /// untouchable would pass a front-heavy assertion just as well, and
    /// would quietly turn the roster into a wall of invulnerable reserves.
    #[test]
    fn back_slot_party_members_draw_less_fire_but_are_still_reachable() {
        let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let mut slots = vec![player];
        for _ in 0..MAX_PARTY_SIZE {
            // Huge HP pools so nobody drops out of the pool mid-sample.
            let pet = spawn_tamed(&mut game, 100_000, 1);
            game.add_companion(pet).unwrap();
            slots.push(pet);
        }
        assert!(
            slots.len() > FRONT_SLOTS,
            "the sample needs at least one back slot to be meaningful"
        );

        let mut hits = vec![0u32; slots.len()];
        for _ in 0..4000 {
            let target = game.roll_enemy_target(player);
            let idx = slots.iter().position(|&e| e == target).unwrap();
            hits[idx] += 1;
        }

        let (front, back) = hits.split_at(FRONT_SLOTS);
        assert!(
            back.iter().all(|&h| h > 0),
            "every back slot must still be reachable, got {hits:?}"
        );
        let front_min = *front.iter().min().unwrap();
        let back_max = *back.iter().max().unwrap();
        assert!(
            front_min > back_max,
            "every front slot should outdraw every back slot, got {hits:?}"
        );
    }

    /// Bracing draws fire — that is what makes Defend a party-level play
    /// rather than a selfish one.
    #[test]
    fn a_bracing_member_draws_more_fire_than_it_otherwise_would() {
        let sample = |brace: bool| {
            let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let player = game.player_entity();
            let pet = spawn_tamed(&mut game, 100_000, 1);
            game.add_companion(pet).unwrap();
            if brace {
                game.begin_defend(pet);
            }
            (0..4000)
                .filter(|_| game.roll_enemy_target(player) == pet)
                .count()
        };
        assert!(
            sample(true) > sample(false),
            "a bracing companion must take more of the incoming fire"
        );
    }

    /// Party order is mechanically meaningful under soft ranks — front
    /// slots draw more fire — so it has to survive a save/load round trip.
    /// The roster order here deliberately differs from spawn order, which
    /// is what the party used to be rebuilt from.
    #[test]
    fn party_order_survives_a_save_load_round_trip() {
        let dir = std::env::temp_dir().join("feral_party_order_roundtrip");
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();

        let mut game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Distinct max HP is the identity that has to come back in order.
        let a = spawn_tamed(&mut game, 31, 3);
        let b = spawn_tamed(&mut game, 47, 3);
        let c = spawn_tamed(&mut game, 53, 3);
        for pet in [c, a, b] {
            game.add_companion(pet).unwrap();
        }
        let path = dir.join("slot.sav");
        game.save(&path).unwrap();

        let loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let order: Vec<i32> = loaded
            .world
            .resource::<Party>()
            .0
            .iter()
            .filter_map(|&e| loaded.world.get::<Stats>(e).map(|s| s.max_hp))
            .collect();
        assert_eq!(
            order,
            vec![53, 31, 47],
            "party order must round-trip exactly, not fall back to spawn order"
        );
        let _ = std::fs::remove_dir_all(&dir);
    }

    /// The reach rule is the balance valve that makes a big multi-group
    /// fight survivable. A back group with only melee moves can't connect
    /// at all.
    #[test]
    fn a_back_group_with_only_melee_moves_cannot_reach_the_party() {
        let mut game = Game::new(86, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // Scrapper, Sentinel and Construct are authored melee-only.
        let a = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
        let b = game.spawn_wild_creature("sentinel", 5, 6).unwrap();
        let c = game.spawn_wild_creature("construct", 5, 7).unwrap();
        game.start_battle(vec![a, b, c]);
        let player = game.player_entity();
        let hp_before = game.world.get::<Stats>(player).unwrap().hp;

        // Group 2 (Construct) is behind the engaged pair and melee-only.
        let construct = game.front_of_group(2).unwrap();
        for _ in 0..20 {
            game.wild_retaliate(construct, 2, player);
        }

        assert_eq!(
            game.world.get::<Stats>(player).unwrap().hp,
            hp_before,
            "a melee-only back group must deal no damage"
        );
    }

    /// ...but a back group holding a ranged move connects normally. Without
    /// this half, the test above would pass just as well against a bug that
    /// makes back groups unconditionally inert.
    #[test]
    fn a_back_group_with_a_ranged_move_still_connects() {
        let mut game = Game::new(87, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = game.spawn_wild_creature("scrapper", 5, 5).unwrap();
        let b = game.spawn_wild_creature("sentinel", 5, 6).unwrap();
        // Glitch's "Static Burst" is authored ranged.
        let c = game.spawn_wild_creature("glitch", 5, 7).unwrap();
        game.start_battle(vec![a, b, c]);
        let player = game.player_entity();
        let hp_before = game.world.get::<Stats>(player).unwrap().hp;

        let glitch = game.front_of_group(2).unwrap();
        game.wild_retaliate(glitch, 2, player);

        assert!(
            game.world.get::<Stats>(player).unwrap().hp < hp_before,
            "a ranged back group must be able to land a hit"
        );
    }

    /// An engaged group picks from its whole moveset, ranged or not —
    /// the restriction is about distance, not about the moves themselves.
    #[test]
    fn an_engaged_group_still_uses_its_melee_moves() {
        let mut game = Game::new(88, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let construct = game.spawn_wild_creature("construct", 5, 5).unwrap();
        game.start_battle(vec![construct]);
        let player = game.player_entity();
        let hp_before = game.world.get::<Stats>(player).unwrap().hp;

        game.wild_retaliate(construct, 0, player);

        assert!(
            game.world.get::<Stats>(player).unwrap().hp < hp_before,
            "a melee-only species in the front rank must still hit"
        );
    }

    /// A planned target can die earlier in the same round than the member
    /// who aimed at it, leaving a stale group index behind. Falling back to
    /// the front group is the difference between a wasted turn and an
    /// out-of-bounds panic.
    #[test]
    fn a_stale_target_group_index_falls_back_to_the_front_group() {
        let mut game = Game::new(84, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let glitch = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        let scrapper = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
        game.start_battle(vec![glitch, scrapper]);

        assert_eq!(game.retarget(1), Some(1), "group 1 is standing");

        game.world.get_mut::<Stats>(scrapper).unwrap().hp = 0;
        game.finish_group_member(1, player);

        assert_eq!(
            game.retarget(1),
            Some(0),
            "a stale index must fall back to the lowest surviving group"
        );
    }

    /// The whole party plans against the same group, and the first hit
    /// wipes it — so every later actor in the initiative order is holding a
    /// plan against a group that no longer exists, in a battle that has
    /// already ended. The round must unwind cleanly rather than panic.
    #[test]
    fn a_round_survives_its_target_dying_before_every_member_has_acted() {
        let mut game = Game::new(85, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let pet = spawn_tamed(&mut game, 40, 8);
        game.add_companion(pet).unwrap();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        game.start_battle(vec![wild]);
        // One HP, so whoever wins initiative ends the fight outright.
        game.world.get_mut::<Stats>(wild).unwrap().hp = 1;

        game.battle_set_action(0, BattleAction::Attack { group: 0 })
            .unwrap();
        game.battle_set_action(1, BattleAction::Attack { group: 0 })
            .unwrap();
        game.battle_resolve_round();

        assert!(
            !game.has_active_battle(),
            "the fight should have ended the moment the only group was wiped"
        );
    }

    /// The menu is data, not renderer strings. Decompile must report *why*
    /// it is unavailable so the UI can grey it with a reason instead of
    /// hiding it and leaving the player guessing.
    #[test]
    fn decompile_is_offered_with_a_reason_when_no_catalyst_is_held() {
        let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // `Inventory` exposes no `clear` — `items` is a public
        // `Vec<(ItemId, u32)>`, so empty it directly.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .items
            .clear();
        let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        game.start_battle(vec![wild]);

        let options = game.battle_action_options(0);
        let decompile = options
            .iter()
            .find(|o| o.kind == ActionKind::Decompile)
            .expect("Decompile must be listed even when unusable");
        assert!(
            decompile
                .unavailable
                .as_deref()
                .is_some_and(|r| r.contains("catalyst")),
            "expected a catalyst reason, got {:?}",
            decompile.unavailable
        );
    }

    /// Initiative order must be reproducible under a fixed seed. Every roll
    /// goes through the existing `GameRng`, so a seeded test can assert an
    /// exact order without touching the wall clock.
    #[test]
    fn initiative_order_is_reproducible_under_a_fixed_seed() {
        let order_for = |seed: u32| {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let a = game.spawn_wild_creature("glitch", 5, 5).unwrap();
            let b = game.spawn_wild_creature("construct", 5, 6).unwrap();
            game.start_battle(vec![a, b]);
            game.roll_initiative()
        };
        assert_eq!(order_for(1234), order_for(1234), "same seed, same order");
    }

    /// Speed has to actually bias the order, or the stat is decoration.
    /// Sampled rather than asserted per-round: a d10 on top of an 8-point
    /// gap still lets the Construct win occasionally, and a test that
    /// forbade that would be asserting the die doesn't exist.
    #[test]
    fn a_faster_species_wins_initiative_far_more_often_than_a_slower_one() {
        let mut sprite_first = 0;
        for seed in 0..200u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let sprite = game.spawn_wild_creature("sprite", 5, 5).unwrap();
            let construct = game.spawn_wild_creature("construct", 5, 6).unwrap();
            game.start_battle(vec![sprite, construct]);
            let order = game.roll_initiative();
            let pos = |e: Entity| {
                order
                    .iter()
                    .position(|a| game.actor_entity(*a) == Some(e))
                    .unwrap()
            };
            if pos(sprite) < pos(construct) {
                sprite_first += 1;
            }
        }
        assert!(
            sprite_first > 150,
            "a Sprite (14) should beat a Construct (6) far more often than not, got {sprite_first}/200"
        );
    }

    /// A pack partitions into one group per species, in first-appearance
    /// order. `gather_pack` walks an ECS query, so the deterministic order
    /// has to come from the partition step itself — an incidental query
    /// order is exactly the kind of thing that produced this repo's
    /// unsorted-habitat-lookup flake.
    #[test]
    fn a_mixed_pack_partitions_into_one_group_per_species_in_first_appearance_order() {
        let mut game = Game::new(77, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let a = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        let b = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
        let c = game.spawn_wild_creature("glitch", 5, 7).unwrap();
        let d = game.spawn_wild_creature("scrapper", 6, 5).unwrap();

        game.start_battle(vec![a, b, c, d]);

        let battle = game.world.resource::<BattleState>();
        assert_eq!(battle.groups.len(), 2, "two species means two groups");
        assert_eq!(battle.groups[0].species, "glitch", "glitch appeared first");
        assert_eq!(battle.groups[0].members, vec![a, c]);
        assert_eq!(battle.groups[1].species, "scrapper");
        assert_eq!(battle.groups[1].members, vec![b, d]);
    }

    /// Only `MAX_ENEMY_GROUPS` species can engage at once. The overflow stays
    /// on the map as ordinary hostiles rather than being despawned — the
    /// player meets them on the next bump.
    #[test]
    fn a_pack_of_more_than_four_species_engages_the_four_largest_and_leaves_the_rest() {
        let mut game = Game::new(78, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        // glitch x3, scrapper x2, virus x2, worm x2, sprite x1 -> sprite is
        // the smallest group and the one left out.
        let mut spawned = Vec::new();
        for (species, count) in [
            ("glitch", 3),
            ("scrapper", 2),
            ("virus", 2),
            ("worm", 2),
            ("sprite", 1),
        ] {
            for i in 0..count {
                spawned.push(game.spawn_wild_creature(species, 5, 5 + i).unwrap());
            }
        }
        let sprite = *spawned.last().unwrap();

        game.start_battle(spawned.clone());

        let battle = game.world.resource::<BattleState>();
        assert_eq!(battle.groups.len(), MAX_ENEMY_GROUPS);
        assert!(
            battle.groups.iter().all(|g| g.species != "sprite"),
            "the smallest group should be the one left out"
        );
        assert!(
            game.world.get_entity(sprite).is_ok(),
            "an un-engaged hostile must stay on the map, never be despawned"
        );
    }

    /// Wiping the front group promotes whatever sat behind it — the central
    /// tension of the reach rule: clearing front-to-back is not
    /// automatically correct, because it walks the back rank into melee.
    #[test]
    fn wiping_the_front_group_promotes_the_group_behind_it() {
        let mut game = Game::new(79, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let glitch = game.spawn_wild_creature("glitch", 5, 5).unwrap();
        let scrapper = game.spawn_wild_creature("scrapper", 5, 6).unwrap();
        game.start_battle(vec![glitch, scrapper]);
        let player = game.player_entity();

        assert_eq!(
            game.world.resource::<BattleState>().groups[0].species,
            "glitch"
        );

        game.world.get_mut::<Stats>(glitch).unwrap().hp = 0;
        let battle_over = game.finish_group_member(0, player);

        assert!(!battle_over, "the scrapper group is still standing");
        let battle = game.world.resource::<BattleState>();
        assert_eq!(battle.groups.len(), 1);
        assert_eq!(
            battle.groups[0].species, "scrapper",
            "the surviving group should have shifted into index 0"
        );
    }

    #[test]
    fn gather_pack_pulls_in_nearby_hostiles_and_caps_at_max_pack_size() {
        let species_id = {
            let game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.species_defs()
                .into_iter()
                .next()
                .expect("at least one species")
                .id
                .clone()
        };
        let mut game = Game::new(0, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let spawn = *game.world.resource::<ZoneSpawnPoint>();
        // Far enough out that zone 1's pack cap (2) is fully unlocked.
        let (ax, ay) = (spawn.x + PACK_SIZE_STEP_TILES * 5, spawn.y);
        let spawn_hostile = |game: &mut Game, x: i32, y: i32| {
            game.world
                .spawn((
                    Creature {
                        species: species_id.clone(),
                    },
                    Hostile,
                    Position { x, y },
                    Stats {
                        hp: 10,
                        max_hp: 10,
                        atk: 1,
                        def: 0,
                    },
                ))
                .id()
        };
        let anchor = spawn_hostile(&mut game, ax, ay);
        for i in 1..=3 {
            spawn_hostile(&mut game, ax + i, ay);
        }

        let pack = game.gather_pack(anchor);

        assert_eq!(
            pack[0], anchor,
            "the creature actually bumped into should always be the pack's front"
        );
        assert_eq!(
            pack.len(),
            PACK_SIZE_PER_ZONE as usize,
            "zone 1's pack cap should bind with 3 other Hostiles in range"
        );
    }

    #[test]
    fn a_creatures_display_label_is_tagged_with_its_spawn_zone() {
        let mut game = Game::new(50, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let species = game.species_defs().into_iter().next().unwrap();

        let zone1 = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 3, y: 3 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
                ZonePortal(1),
            ))
            .id();
        let zone2 = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 4, y: 4 },
                Stats {
                    hp: 2,
                    max_hp: 2,
                    atk: 2,
                    def: 2,
                },
                ZonePortal(2),
            ))
            .id();

        assert_eq!(game.entity_label(zone1), format!("{} 1", species.name));
        assert_eq!(game.entity_label(zone2), format!("{} 2", species.name));
        assert_eq!(
            game.inspect(zone2).unwrap().name,
            format!("{} 2", species.name)
        );
    }

    #[test]
    fn defeating_a_boss_guarantees_a_cache_of_portal_fragments() {
        let mut game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let boss = game
            .species_defs()
            .into_iter()
            .find(|s| s.is_boss)
            .expect("at least one boss species should exist in assets/species for this test");

        let wild = game
            .world
            .spawn((
                Creature {
                    species: boss.id.clone(),
                },
                Position { x: 0, y: 0 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        game.award_loot(wild);

        let qty = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::PORTAL_FRAGMENT));
        assert!(
            BOSS_PORTAL_FRAGMENT_DROP.contains(&qty),
            "boss kill should guarantee a portal fragment cache in {BOSS_PORTAL_FRAGMENT_DROP:?}, got {qty}"
        );
    }

    #[test]
    fn boss_creatures_are_flagged_in_entity_and_inspect_views() {
        let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let boss = game
            .species_defs()
            .into_iter()
            .find(|s| s.is_boss)
            .expect("at least one boss species should exist in assets/species for this test");
        let normal = game
            .species_defs()
            .into_iter()
            .find(|s| !s.is_boss)
            .expect("at least one non-boss species should exist");

        // Clear the world's own initial habitat population so the only
        // hostiles in view are the two this test spawns itself below —
        // otherwise a stray boss (or non-boss) from that initial spawn
        // roll could land within view range and make the assertions below
        // fragile to unrelated changes in spawn odds/roll counts.
        let initial_hostiles: Vec<Entity> = {
            let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
            query.iter(&game.world).collect()
        };
        for e in initial_hostiles {
            game.world.despawn(e);
        }

        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        let boss_entity = game
            .world
            .spawn((
                Creature {
                    species: boss.id.clone(),
                },
                Hostile,
                Position {
                    x: player_pos.x + 1,
                    y: player_pos.y,
                },
                Glyph {
                    ch: boss.glyph,
                    color: boss.color,
                },
                Stats {
                    hp: boss.base_hp,
                    max_hp: boss.base_hp,
                    atk: boss.base_atk,
                    def: boss.base_def,
                },
            ))
            .id();
        game.world.spawn((
            Creature {
                species: normal.id.clone(),
            },
            Hostile,
            Position {
                x: player_pos.x - 1,
                y: player_pos.y,
            },
            Glyph {
                ch: normal.glyph,
                color: normal.color,
            },
            Stats {
                hp: normal.base_hp,
                max_hp: normal.base_hp,
                atk: normal.base_atk,
                def: normal.base_def,
            },
        ));

        let views = game.view_entities(5, 5);
        let boss_view = views.iter().find(|v| v.entity == boss_entity).unwrap();
        assert!(
            boss_view.is_boss,
            "the boss creature's EntityView should be flagged is_boss"
        );
        let normal_views: Vec<_> = views
            .iter()
            .filter(|v| v.entity != boss_entity && v.is_hostile)
            .collect();
        assert!(
            normal_views.iter().all(|v| !v.is_boss),
            "non-boss creatures shouldn't be flagged is_boss"
        );

        assert!(
            game.inspect(boss_entity).unwrap().is_boss,
            "InspectView should also flag a boss creature"
        );
    }

    #[test]
    fn view_entities_colors_hostiles_by_difficulty_and_leaves_others_alone() {
        let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let player_pos = *game.world.get::<Position>(player).unwrap();
        game.world.get_mut::<Stats>(player).unwrap().atk = 0;
        game.world.get_mut::<Stats>(player).unwrap().def = 0;
        game.world.get_mut::<Stats>(player).unwrap().max_hp = 100;
        game.world.get_mut::<Stats>(player).unwrap().hp = 100;
        // Player power is now 100. An easy hostile is well under that; a
        // hard one is well over it.
        let easy = game
            .world
            .spawn((
                Creature {
                    species: "does_not_matter".to_string(),
                },
                Hostile,
                Position {
                    x: player_pos.x + 1,
                    y: player_pos.y,
                },
                Glyph {
                    ch: 'e',
                    color: GlyphColor::Cyan,
                },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        let hard = game
            .world
            .spawn((
                Creature {
                    species: "does_not_matter".to_string(),
                },
                Hostile,
                Position {
                    x: player_pos.x - 1,
                    y: player_pos.y,
                },
                Glyph {
                    ch: 'h',
                    color: GlyphColor::Cyan,
                },
                Stats {
                    hp: 300,
                    max_hp: 300,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        let tamed_worker = spawn_tamed(&mut game, 10, 3);
        game.world.entity_mut(tamed_worker).insert(Position {
            x: player_pos.x,
            y: player_pos.y + 1,
        });
        game.world.entity_mut(tamed_worker).insert(Glyph {
            ch: 't',
            color: GlyphColor::Cyan,
        });

        let views = game.view_entities(5, 5);
        let easy_view = views.iter().find(|v| v.entity == easy).unwrap();
        let hard_view = views.iter().find(|v| v.entity == hard).unwrap();
        let tamed_view = views.iter().find(|v| v.entity == tamed_worker).unwrap();

        assert_eq!(
            easy_view.color,
            GlyphColor::Green,
            "a much weaker hostile should read Green"
        );
        assert_eq!(
            hard_view.color,
            GlyphColor::Red,
            "a much stronger hostile should read Red"
        );
        assert_eq!(
            tamed_view.color,
            GlyphColor::Cyan,
            "a non-hostile entity should keep its own glyph color, not be difficulty-colored"
        );
    }

    #[test]
    fn stunned_player_loses_their_turn_but_wild_still_retaliates_and_stun_clears() {
        let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Deliberately effect-free, so the wild creature's own retaliation
        // can't re-apply (and thus reset the clock on) the status this test
        // is tracking.
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
            .expect("at least one species with no status-effect moves should exist for this test");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 50,
                    max_hp: 50,
                    atk: 3,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);
        game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
            kind: StatusKind::Stun,
            remaining: 1,
            power: 0,
        });

        let wild_hp_before = game.world.get::<Stats>(wild).unwrap().hp;
        player_attacks(&mut game);
        let wild_hp_after = game.world.get::<Stats>(wild).unwrap().hp;

        assert_eq!(
            wild_hp_before, wild_hp_after,
            "a stunned player shouldn't deal any attack damage"
        );
        assert!(
            game.world
                .get::<StatusEffects>(player)
                .unwrap()
                .active
                .is_none(),
            "the stun should clear after its one round elapses"
        );
    }

    #[test]
    fn bleed_status_deals_extra_damage_each_round_and_expires_after_its_duration() {
        let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Deliberately effect-free, so the wild creature's own retaliation
        // can't re-apply (and thus reset the clock on) the status this test
        // is tracking.
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
            .expect("at least one species with no status-effect moves should exist for this test");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 0,
                    def: 0,
                },
                StatusEffects {
                    active: Some(ActiveStatus {
                        kind: StatusKind::Bleed,
                        remaining: 2,
                        power: 5,
                    }),
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);
        let player_atk = game.world.get::<Stats>(player).unwrap().atk;
        let expected_attack_dmg = battle::compute_damage(player_atk, 0, 5);

        let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
        player_attacks(&mut game);
        let hp_after = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            hp_before - hp_after,
            expected_attack_dmg + 5,
            "wild should take its attack damage plus one round of bleed"
        );
        assert_eq!(
            game.world
                .get::<StatusEffects>(wild)
                .unwrap()
                .active
                .unwrap()
                .remaining,
            1
        );

        let hp_before2 = game.world.get::<Stats>(wild).unwrap().hp;
        player_attacks(&mut game);
        let hp_after2 = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            hp_before2 - hp_after2,
            expected_attack_dmg + 5,
            "the second bleed round should also tick"
        );
        assert!(
            game.world
                .get::<StatusEffects>(wild)
                .unwrap()
                .active
                .is_none(),
            "bleed should clear once its duration elapses"
        );
    }

    #[test]
    fn status_effects_are_cleared_once_the_battle_ends() {
        let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Deliberately effect-free, so the wild creature's own retaliation
        // can't re-apply (and thus reset the clock on) the status this test
        // is tracking.
        let species = game
            .species_defs()
            .into_iter()
            .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
            .expect("at least one species with no status-effect moves should exist for this test");
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 1,
                    max_hp: 1,
                    atk: 1,
                    def: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        insert_battle(&mut game, player, vec![wild]);
        game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
            kind: StatusKind::Bleed,
            remaining: 5,
            power: 1,
        });

        // 1 HP wild creature dies to the player's first attack, ending the battle.
        player_attacks(&mut game);

        assert!(
            !game.has_active_battle(),
            "the wild creature's death should end the battle"
        );
        assert!(
            game.world
                .get::<StatusEffects>(player)
                .unwrap()
                .active
                .is_none(),
            "leftover status effects should be cleared once the battle ends, however it ends"
        );
    }

    /// Deploys a Home plus a Mining Node beside it, returning both entities
    /// so a caller can assert on what survives a breach.
    fn build_a_base(game: &mut Game) -> (Entity, Entity) {
        // On the player's own tile, which is walkable by definition — the
        // Home's slab then guarantees everything around it is too, so this
        // works for any seed.
        place_home(game, 0, 0);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 12);
        game.place_structure("mining_node", 1, 1).unwrap();
        (
            find_structure_by_kind(game, "home").unwrap(),
            find_structure_by_kind(game, "mining_node").unwrap(),
        )
    }

    /// Deploys a Mining Node beside a Home with materials to spare, and
    /// returns it.
    fn deploy_upgradeable_node(game: &mut Game) -> Entity {
        place_home(game, 0, 1);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 12);
        game.place_structure("mining_node", 1, 1).unwrap();
        find_structure_by_kind(game, "mining_node").unwrap()
    }

    #[test]
    fn upgrading_a_node_costs_materials_and_raises_its_tier() {
        let mut game = Game::new(970, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let node = deploy_upgradeable_node(&mut game);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 20);

        assert_eq!(
            game.world.get::<StructureTier>(node).unwrap().0,
            1,
            "structures deploy at Mk1"
        );
        let before = count_item(&game, ids::CORE_FRAGMENT);

        game.upgrade_structure(node).unwrap();

        assert_eq!(game.world.get::<StructureTier>(node).unwrap().0, 2);
        assert_eq!(
            before - count_item(&game, ids::CORE_FRAGMENT),
            20,
            "reaching tier 2 costs the def's 10 per tier x 2"
        );
    }

    #[test]
    fn upgrading_a_node_makes_its_extraction_more_reliable() {
        let mut game = Game::new(971, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let node = deploy_upgradeable_node(&mut game);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 200);

        assert_eq!(game.world.get::<ResourceNode>(node).unwrap().level, Some(1));
        game.upgrade_structure(node).unwrap();
        assert_eq!(
            game.world.get::<ResourceNode>(node).unwrap().level,
            Some(2),
            "tier feeds ResourceNode.level, which already drives mining_success_chance"
        );
    }

    #[test]
    fn upgrading_refuses_past_max_tier_and_without_materials() {
        let mut game = Game::new(972, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let node = deploy_upgradeable_node(&mut game);

        let err = game
            .upgrade_structure(node)
            .expect_err("no materials left after building it");
        assert!(err.contains("Not enough"), "unexpected error: {err}");

        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 1000);
        let max = game
            .world
            .resource::<StructureDb>()
            .get("mining_node")
            .unwrap()
            .upgrade
            .as_ref()
            .unwrap()
            .max_tier;
        for _ in 1..max {
            game.upgrade_structure(node).unwrap();
        }
        let err = game
            .upgrade_structure(node)
            .expect_err("a maxed node can't be upgraded further");
        assert!(err.contains("fully upgraded"), "unexpected error: {err}");
    }

    #[test]
    fn a_structure_without_an_upgrade_def_cannot_be_upgraded() {
        let mut game = Game::new(973, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, 0, 1);
        let home = find_structure_by_kind(&mut game, "home").unwrap();
        let err = game
            .upgrade_structure(home)
            .expect_err("Home declares no upgrade path");
        assert!(err.contains("can't be upgraded"), "unexpected error: {err}");
    }

    #[test]
    fn tier_multiplies_payout_on_top_of_the_zone_multiplier() {
        let mut game = Game::new(974, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.resource_mut::<ZoneLevel>().0 = 3; // stat_multiplier() == 4

        let gained = run_one_full_gather_cycle_at_tier(&mut game, ids::CORE_FRAGMENT, Some(3));

        assert_eq!(gained, 12, "tier 3 x zone multiplier 4");
    }

    #[test]
    fn a_structures_tier_survives_a_save_and_load_round_trip() {
        let mut game = Game::new(975, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let node = deploy_upgradeable_node(&mut game);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 200);
        game.upgrade_structure(node).unwrap();
        game.upgrade_structure(node).unwrap();

        let path = std::env::temp_dir().join(format!("feral_tier_save_{}.bin", std::process::id()));
        game.save(&path).unwrap();
        let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
        let _ = std::fs::remove_file(&path);

        let restored = find_structure_by_kind(&mut loaded, "mining_node").unwrap();
        assert_eq!(
            loaded.world.get::<StructureTier>(restored).unwrap().0,
            3,
            "a Mk3 node must not come back as Mk1"
        );
        assert_eq!(
            loaded.world.get::<ResourceNode>(restored).unwrap().level,
            Some(3),
            "and its extraction reliability with it — WorkDef::level only carries the \
             tier-1 baseline"
        );
    }

    #[test]
    fn a_worked_node_pays_out_more_the_deeper_the_zone() {
        let mut game = Game::new(960, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.resource_mut::<ZoneLevel>().0 = 4;
        assert_eq!(
            game.world.resource::<ZoneLevel>().stat_multiplier(),
            8,
            "zone 4's multiplier is 1 << 3"
        );

        let gained = run_one_full_gather_cycle(&mut game, ids::CORE_FRAGMENT);

        assert_eq!(gained, 8, "a zone-4 node pays 8x what a zone-1 node pays");
    }

    #[test]
    fn a_zone_one_node_still_pays_exactly_one() {
        let mut game = Game::new(962, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(
            game.world.resource::<ZoneLevel>().0,
            1,
            "runs start at zone 1"
        );

        let gained = run_one_full_gather_cycle(&mut game, ids::CORE_FRAGMENT);

        assert_eq!(
            gained, 1,
            "zone 1's multiplier is 1 << 0 == 1, so the opening game is unchanged"
        );
    }

    #[test]
    fn a_banked_resource_never_scales_with_zone_depth() {
        let mut game = Game::new(961, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world.resource_mut::<ZoneLevel>().0 = 5;

        let gained = run_one_full_gather_cycle(&mut game, ids::RESEARCH_DATA);

        assert_eq!(
            gained, 1,
            "research_data has a bank_limit of 200 — scaling it would fill the bank in ~13 \
             cycles and turn the research economy into 'no room to store it' spam"
        );
    }

    #[test]
    fn stepping_through_a_portal_consumes_it_so_it_never_travels() {
        let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        place_home(&mut game, 0, 1);
        game.world
            .get_mut::<Inventory>(game.player_entity())
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 10);
        game.place_structure("portal", 1, 0).unwrap();

        game.move_player(1, 0);

        assert_eq!(
            game.world.resource::<ZoneLevel>().0,
            2,
            "stepping onto the portal breaches"
        );
        assert!(
            find_structure_by_kind(&mut game, "portal").is_none(),
            "a portal is one-use — carrying it forward would make every later breach free"
        );
        assert!(
            find_structure_by_kind(&mut game, "home").is_some(),
            "consuming the portal must not take the rest of the base with it"
        );
    }

    #[test]
    fn breaching_carries_every_structure_and_its_offset_from_home() {
        let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let (home, node) = build_a_base(&mut game);
        let before = {
            let h = *game.world.get::<Position>(home).unwrap();
            let n = *game.world.get::<Position>(node).unwrap();
            (n.x - h.x, n.y - h.y)
        };

        game.enter_next_zone();

        assert!(
            game.world.get_entity(home).is_ok(),
            "the Home travels through the breach"
        );
        assert!(
            game.world.get_entity(node).is_ok(),
            "so does everything built around it"
        );
        let h = *game.world.get::<Position>(home).unwrap();
        let n = *game.world.get::<Position>(node).unwrap();
        assert_eq!(
            (n.x - h.x, n.y - h.y),
            before,
            "the base's layout must be preserved exactly, not reshuffled"
        );
        let spawn = *game.world.resource::<ZoneSpawnPoint>();
        assert_eq!(
            (h.x, h.y),
            (spawn.x, spawn.y),
            "the Home lands at the new spawn point"
        );
    }

    #[test]
    fn breaching_with_a_base_still_populates_the_new_zone() {
        for seed in 0u32..12 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            build_a_base(&mut game);

            game.enter_next_zone();

            let hostiles = {
                let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
                query.iter(&game.world).count()
            };
            assert!(
                hostiles > 0,
                "seed {seed}: a zone breached into with a base must still have wild programs \
                 in it. The platform has no habitat species and is exactly as wide as the \
                 initial spawn scatter, so a scatter that never reaches past its edge leaves \
                 the whole zone empty."
            );
        }
    }

    #[test]
    fn breaching_preserves_structure_durability_and_node_stock() {
        let mut game = Game::new(941, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let (_home, node) = build_a_base(&mut game);
        game.world.get_mut::<Durability>(node).unwrap().hp = 7;
        game.world.get_mut::<ResourceNode>(node).unwrap().amount = 2;

        game.enter_next_zone();

        assert_eq!(
            game.world.get::<Durability>(node).unwrap().hp,
            7,
            "damage travels with the structure"
        );
        assert_eq!(
            game.world.get::<ResourceNode>(node).unwrap().amount,
            2,
            "so does mined-down stock"
        );
    }

    #[test]
    fn breaching_restamps_the_platform_around_the_new_spawn_point() {
        let mut game = Game::new(942, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        build_a_base(&mut game);

        game.enter_next_zone();

        let spawn = *game.world.resource::<ZoneSpawnPoint>();
        assert_eq!(
            game.world
                .resource_mut::<WorldMap>()
                .tile(spawn.x, spawn.y)
                .biome,
            Biome::Platform,
            "the slab is re-stamped on the new map"
        );
        assert_eq!(
            game.world.resource::<Platform>().center,
            Some((spawn.x, spawn.y)),
            "and the resource follows it"
        );
    }

    #[test]
    fn breaching_leaves_a_cronjob_assignment_pointing_at_a_live_structure() {
        let mut game = Game::new(943, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let (_home, node) = build_a_base(&mut game);
        let worker = spawn_tamed(&mut game, 10, 3);
        game.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: node,
            progress: 0,
            required: 10,
        });

        game.enter_next_zone();

        let task = game
            .world
            .get::<Task>(worker)
            .expect("the cronjob survives the breach");
        assert_eq!(
            task.target, node,
            "and still points at the structure that travelled with it"
        );
        assert!(
            game.world.get_entity(task.target).is_ok(),
            "which is still alive"
        );
    }

    #[test]
    fn zone_transition_carries_tamed_companions_and_the_base_but_leaves_wild_creatures_behind() {
        let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();

        // Clear anything the world's own initial habitat spawn happened to
        // place on the tiles this test is about to use for its own fixtures
        // (portal, home, wild) — the exact initial layout isn't this test's
        // concern, and asserting it stays untouched would make the test
        // fragile to unrelated changes in spawn odds/roll counts.
        let stray: Vec<Entity> = {
            let mut query = game.world.query::<(Entity, &Position)>();
            query
                .iter(&game.world)
                .filter(|(e, p)| {
                    *e != player
                        && ((p.x, p.y) == (ppos.x + 1, ppos.y)
                            || (p.x, p.y) == (ppos.x + 3, ppos.y)
                            || (p.x, p.y) == (ppos.x + 5, ppos.y))
                })
                .map(|(e, _)| e)
                .collect()
        };
        for e in stray {
            game.world.despawn(e);
        }

        let companion = spawn_tamed(&mut game, 10, 3);
        game.add_companion(companion).unwrap();

        let species = game.species_defs().into_iter().next().unwrap();
        let wild = game
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position {
                    x: ppos.x + 3,
                    y: ppos.y,
                },
                Stats {
                    hp: 5,
                    max_hp: 5,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        let home = game
            .world
            .spawn((
                Structure {
                    kind: "home".to_string(),
                },
                Position {
                    x: ppos.x + 5,
                    y: ppos.y,
                },
            ))
            .id();

        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));

        game.move_player(1, 0);

        assert_eq!(game.player_status().zone, 2);
        assert!(
            game.world.get::<Tamed>(companion).is_some(),
            "the companion should still be tamed after breaching"
        );
        assert!(
            game.world.get::<Creature>(wild).is_none(),
            "wild creatures should be left behind, not carried through the portal"
        );
        assert!(
            game.world.get::<Structure>(home).is_some(),
            "the base travels through the breach with the player"
        );
        let companion_pos = *game.world.get::<Position>(companion).unwrap();
        let player_pos = *game.world.get::<Position>(player).unwrap();
        assert_eq!(
            companion_pos, player_pos,
            "the companion should travel with the player into the new zone"
        );
    }

    #[test]
    fn breaching_wipes_the_currency_and_craft_currency_stacks() {
        let mut game = Game::new(945, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.add(ItemId::from(ids::PORTAL_FRAGMENT), 25);
            inv.add(ItemId::from(ids::CORE_FRAGMENT), 40);
        }

        game.enter_next_zone();

        assert_eq!(
            count_item(&game, ids::PORTAL_FRAGMENT),
            0,
            "the next zone's portal has to be funded in the zone you leave from"
        );
        assert_eq!(
            count_item(&game, ids::CORE_FRAGMENT),
            0,
            "and so does everything the base is bought with"
        );
    }

    #[test]
    fn breaching_keeps_everything_that_is_not_spendable_currency() {
        let mut game = Game::new(946, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.add(ItemId::from(ids::RESEARCH_DATA), 60);
            inv.add(ItemId::from(ids::POWER_CELL), 4);
        }
        game.world
            .get_mut::<ItemFusions>(player)
            .unwrap()
            .increment(ItemId::from(ids::ICE_BREAKER));

        game.enter_next_zone();

        assert_eq!(
            count_item(&game, ids::RESEARCH_DATA),
            60,
            "banked research is progress, not pocket money"
        );
        assert_eq!(
            count_item(&game, ids::POWER_CELL),
            7,
            "3 from the starting kit plus the 4 added; supplies are carried, not confiscated"
        );
        assert_eq!(
            count_item(&game, ids::ICE_BREAKER),
            3,
            "the starting kit's catalysts make the trip too"
        );
        assert_eq!(
            game.world
                .get::<ItemFusions>(player)
                .unwrap()
                .tier(&ItemId::from(ids::ICE_BREAKER)),
            1,
            "fusion progress is not currency"
        );
    }

    #[test]
    fn the_decohere_message_only_fires_when_there_was_something_to_lose() {
        let mut game = Game::new(947, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(ItemId::from(ids::CORE_FRAGMENT), u32::MAX);

        game.enter_next_zone();

        assert!(
            !game
                .message_log(20)
                .iter()
                .any(|(_, m)| m.contains("decohere")),
            "an empty wallet shouldn't be announced as a loss"
        );

        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 3);
        game.enter_next_zone();

        // "{qty} {name}", the same unpluralized shape `describe_structure`
        // uses for a teleport cost — item names are modder-supplied data, not
        // English to inflect.
        assert!(
            game.message_log(20)
                .iter()
                .any(|(_, m)| m.contains("3 Portal Fragment")),
            "a real loss is named and counted: {:?}",
            game.message_log(20)
        );
    }

    #[test]
    fn portal_cost_grows_by_half_the_base_rate_per_zone() {
        let mut game = Game::new(944, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let portal = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "portal")
            .expect("portal.ron should load");
        let fragments = |game: &Game, def: &StructureDef| {
            game.structure_build_cost(def)
                .into_iter()
                .find(|(item, _)| item.as_str() == ids::PORTAL_FRAGMENT)
                .map(|(_, qty)| qty)
                .expect("a portal is bought with portal fragments")
        };

        assert_eq!(fragments(&game, &portal), 10, "zone 1 pays the base rate");

        game.world.insert_resource(ZoneLevel(2));
        assert_eq!(
            fragments(&game, &portal),
            15,
            "each zone adds half the base rate, not another whole one"
        );

        game.world.insert_resource(ZoneLevel(5));
        assert_eq!(
            fragments(&game, &portal),
            30,
            "the ramp stays linear in the base rate all the way down"
        );

        let node = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "mining_node")
            .expect("mining_node.ron should load");
        assert_eq!(
            game.structure_build_cost(&node),
            node.build_cost,
            "only a zone-portal structure scales; everything else is flat at any depth"
        );
    }

    #[test]
    fn portal_build_cost_ramps_with_current_zone_level() {
        let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        place_home(&mut game, -1, 0);

        // Zone 1: base rate from portal.ron, 10 PortalFragment, unramped.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 10);
        game.place_structure("portal", 1, 0).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
            0,
            "zone 1 portal should cost the base rate"
        );

        game.move_player(1, 0);
        assert_eq!(game.player_status().zone, 2);
        // The Home travelled through the breach with the rest of the base
        // (see `breaching_carries_every_structure_and_its_offset_from_home`),
        // so the new zone needs no fresh Home before building.

        // Zone 2: base rate plus half of it again (10 + 5 = 15), not double.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 14);
        assert!(
            game.place_structure("portal", 1, 0).is_err(),
            "14 fragments shouldn't be enough for a zone-2 portal"
        );
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 1);
        game.place_structure("portal", 1, 0).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::PORTAL_FRAGMENT)),
            0,
            "zone 2 portal should cost the base rate plus half again"
        );
    }

    #[test]
    fn zone_level_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(43, DifficultyMode::Forgiving, &assets).unwrap();
        let player = game.player_entity();
        let ppos = *game.world.get::<Position>(player).unwrap();
        game.world.spawn((
            Structure {
                kind: "portal".to_string(),
            },
            Position {
                x: ppos.x + 1,
                y: ppos.y,
            },
        ));
        game.move_player(1, 0);
        assert_eq!(game.player_status().zone, 2);

        let path = std::env::temp_dir().join(format!(
            "feral_processes_zone_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.player_status().zone,
            2,
            "zone level should survive a save/load round trip"
        );
    }

    /// Regression test for a nearly-empty zone: `find_walkable_start`
    /// always re-centers a freshly generated zone's spawn box near world
    /// origin, and the terrain noise there has roughly the same period as
    /// that box — so a blind, one-attempt-per-slot spawn (the previous
    /// behavior of `spawn_initial_creatures`) could land almost all 14
    /// rolls on an unwalkable or habitat-mismatched tile for an unlucky
    /// seed, leaving the new zone feeling all but abandoned. Sweeps a
    /// range of seeds (rather than trusting one lucky one) to confirm the
    /// retry-until-`count` fix reliably delivers the full population.
    #[test]
    fn zone_transition_reliably_populates_the_new_zone_regardless_of_seed() {
        for seed in 0u32..20 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            let player = game.player_entity();
            let ppos = *game.world.get::<Position>(player).unwrap();
            // The zone-1 starting spawn can, for some seeds, happen to
            // place a wild creature right on the tile the portal is about
            // to go on — clear it so the walk onto the portal deterministically
            // enters the portal rather than picking a fight instead.
            let blockers: Vec<Entity> = {
                let mut query = game
                    .world
                    .query_filtered::<(Entity, &Position), With<Hostile>>();
                query
                    .iter(&game.world)
                    .filter(|(_, p)| p.x == ppos.x + 1 && p.y == ppos.y)
                    .map(|(e, _)| e)
                    .collect()
            };
            for e in blockers {
                game.world.despawn(e);
            }
            game.world.spawn((
                Structure {
                    kind: "portal".to_string(),
                },
                Position {
                    x: ppos.x + 1,
                    y: ppos.y,
                },
            ));
            game.move_player(1, 0);
            assert_eq!(
                game.player_status().zone,
                2,
                "seed {seed}: portal should advance the zone"
            );

            let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
            let count = query.iter(&game.world).count();
            assert!(
                count >= 14,
                "seed {seed}: zone 2 should have spawned at least the 14 requested wild \
                 creatures, found {count}"
            );
        }
    }

    #[test]
    fn bumping_a_nest_damages_it_and_destroying_it_frees_its_guardians() {
        let mut game = Game::new(603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Position>(player).unwrap().x = 49;
        game.world.get_mut::<Position>(player).unwrap().y = 50;

        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 50, y: 50 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability { hp: 5, max_hp: 5 },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
                Position { x: 52, y: 52 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        // Player's base ATK (6) vs. 0 defense, move_power 5 → well over 5
        // damage, so one bump is enough to destroy a 5-HP nest.
        game.move_player(1, 0);

        assert!(
            game.world.get::<Nest>(nest).is_none(),
            "nest should be destroyed by one bump"
        );
        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "guardian should lose its NestGuardian tether once the nest is destroyed"
        );
    }

    #[test]
    fn bumping_a_nest_with_high_hp_damages_it_without_destroying_it() {
        let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Position>(player).unwrap().x = 49;
        game.world.get_mut::<Position>(player).unwrap().y = 50;

        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 50, y: 50 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability { hp: 50, max_hp: 50 },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
                Position { x: 52, y: 52 },
                Stats {
                    hp: 10,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();

        // Player's base ATK (6) vs. 0 defense, move_power 5 → 11 damage,
        // well short of the nest's 50 HP, so one bump only dents it.
        game.move_player(1, 0);

        assert!(
            game.world.get::<Nest>(nest).is_some(),
            "nest should survive a single bump when it has 50 HP"
        );
        let hp = game.world.get::<Durability>(nest).unwrap().hp;
        assert!(
            hp < 50,
            "nest HP should have decreased from the bump, got {hp}"
        );
        assert!(hp > 0, "nest HP should still be positive, got {hp}");
        assert!(
            game.world.get::<NestGuardian>(guardian).is_some(),
            "guardian should keep its NestGuardian tether while the nest survives"
        );
    }

    #[test]
    fn killing_a_guardian_respawns_a_replacement_after_exactly_the_respawn_delay() {
        let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 60, y: 60 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                NestGuardian { nest },
                Position { x: 61, y: 61 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![guardian]);

        player_attacks(&mut game);

        // the round loop's own kill-resolution path (finish_group_member
        // returning true, the pack now empty) already calls self.tick() once
        // internally before returning — that tick already ran
        // nest_respawn_tick and decremented the entry we just pushed. So the
        // value observed here is NEST_RESPAWN_TICKS - 1, not the full delay.
        assert_eq!(
            game.world.get::<Nest>(nest).unwrap().pending_respawns,
            vec![NEST_RESPAWN_TICKS - 1],
            "killing a guardian should queue one respawn"
        );

        let guardian_count = |game: &mut Game| -> usize {
            let mut query = game.world.query::<&NestGuardian>();
            query.iter(&game.world).filter(|g| g.nest == nest).count()
        };

        for _ in 0..(NEST_RESPAWN_TICKS - 2) {
            game.tick();
        }
        assert_eq!(
            guardian_count(&mut game),
            0,
            "no replacement should spawn before its delay elapses"
        );

        game.tick();
        assert_eq!(
            guardian_count(&mut game),
            1,
            "a replacement should spawn exactly when its delay elapses"
        );
    }

    #[test]
    fn taming_a_guardian_also_queues_a_respawn() {
        let mut game = Game::new(605, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    pending_respawns: Vec::new(),
                },
                Position { x: 70, y: 70 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                WanderAi::default(),
                NestGuardian { nest },
                Position { x: 71, y: 71 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 1,
                    def: 1,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![guardian]);
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::ICE_BREAKER), 50);
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

        for _ in 0..50 {
            if game.world.get::<Tamed>(guardian).is_some() {
                break;
            }
            player_decompiles(&mut game);
        }

        assert!(game.world.get::<Tamed>(guardian).is_some());
        assert!(
            game.world.get::<NestGuardian>(guardian).is_none(),
            "a tamed creature should lose its nest tether"
        );
        // Same off-by-one as the kill test above: battle_decompile's
        // success path also calls self.tick() once internally before
        // returning, which already decremented the entry we just pushed.
        assert_eq!(
            game.world.get::<Nest>(nest).unwrap().pending_respawns,
            vec![NEST_RESPAWN_TICKS - 1],
            "taming a guardian should also queue one respawn"
        );
    }

    #[test]
    fn killing_a_guardian_whose_nest_is_already_gone_queues_nothing() {
        let mut game = Game::new(606, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // A dangling nest Entity — never actually spawned, standing in
        // for "the nest was destroyed before this guardian died."
        let gone_nest = game.world.spawn_empty().id();
        let guardian = game
            .world
            .spawn((
                Creature {
                    species: "scrapper".to_string(),
                },
                Hostile,
                NestGuardian { nest: gone_nest },
                Position { x: 80, y: 80 },
                Stats {
                    hp: 1,
                    max_hp: 10,
                    atk: 0,
                    def: 0,
                },
            ))
            .id();
        insert_battle(&mut game, player, vec![guardian]);

        // Should not panic even though `gone_nest` has no Nest component.
        player_attacks(&mut game);

        for _ in 0..(NEST_RESPAWN_TICKS + 5) {
            game.tick();
        }
        // Nothing to assert beyond "didn't panic" — there's no Nest left
        // to have queued anything on, and no new guardian entity for a
        // nonexistent nest.
    }

    #[test]
    fn nest_respawn_tick_spawns_one_guardian_per_ready_entry_not_one_per_nest() {
        let mut game = Game::new(607, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let nest = game
            .world
            .spawn((
                Nest {
                    species: "scrapper".to_string(),
                    // Two entries reach 0 on the same tick, and a third
                    // untouched entry that should survive, decremented but
                    // not fired — this proves nest_respawn_tick spawns once
                    // per ready entry, not once per nest.
                    pending_respawns: vec![1, 1, 5],
                },
                Position { x: 90, y: 90 },
                Glyph {
                    ch: 'N',
                    color: GlyphColor::Red,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();

        let guardian_count = |game: &mut Game| -> usize {
            let mut query = game.world.query::<&NestGuardian>();
            query.iter(&game.world).filter(|g| g.nest == nest).count()
        };
        assert_eq!(guardian_count(&mut game), 0, "no guardians before the tick");

        game.tick();

        assert_eq!(
            guardian_count(&mut game),
            2,
            "both entries reaching 0 on the same tick should each spawn a guardian"
        );
        assert_eq!(
            game.world.get::<Nest>(nest).unwrap().pending_respawns,
            vec![4],
            "the two fired entries should be removed and the untouched entry decremented once"
        );
    }

    #[test]
    fn use_item_applies_a_power_restore_and_consumes_one() {
        let mut game = Game::new(500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        // The player already starts holding Power Cells (see `Game::new`);
        // drain the default stock first so the stack is exactly 2 below.
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::POWER_CELL));
        inv.take(ItemId::from(ids::POWER_CELL), held);
        inv.add(ItemId::from(ids::POWER_CELL), 2);

        game.use_item(&ItemId::from(ids::POWER_CELL));

        // `use_item` ends with `self.tick()` like every other player action,
        // so `needs_decay_system` also shaves off one tick's worth of hunger
        // (see `HUNGER_DECAY_PER_TICK` in systems.rs) on top of the +25
        // restore — same shared-decay caveat documented on
        // `commanding_a_companion_in_battle_costs_more_fatigue_than_a_stunned_one`.
        assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, 75.0 - 0.15);
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::POWER_CELL)),
            1
        );
    }

    #[test]
    fn use_item_clamps_power_at_full() {
        let mut game = Game::new(501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 90.0;
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::POWER_CELL), 1);

        game.use_item(&ItemId::from(ids::POWER_CELL));

        // 90 + 25 clamps to 100 before the trailing tick's decay shaves off
        // 0.15 (see the comment in the test above) — had the clamp not
        // engaged, this would read 114.85 instead.
        assert_eq!(
            game.world.get::<Needs>(player).unwrap().hunger,
            100.0 - 0.15
        );
    }

    #[test]
    fn use_item_rejects_a_non_consumable() {
        let mut game = Game::new(502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // The player already starts holding Core Fragments (see
        // `Game::new`), so compare against a captured baseline rather than
        // an absolute count.
        let before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT));
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 3);

        game.use_item(&ItemId::from(ids::CORE_FRAGMENT));

        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::CORE_FRAGMENT)),
            before + 3,
            "a non-consumable must not be consumed"
        );
    }

    #[test]
    fn use_item_on_an_empty_stack_is_a_no_op() {
        let mut game = Game::new(503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // The player starts holding Power Cells (see `Game::new`), so drain
        // the stack to actually exercise the empty-stack path.
        let held = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::POWER_CELL));
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(ItemId::from(ids::POWER_CELL), held);
        let before = game.world.get::<Needs>(player).unwrap().hunger;

        game.use_item(&ItemId::from(ids::POWER_CELL));

        assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, before);
    }

    #[test]
    fn a_prebattle_buff_armed_on_the_map_is_live_at_the_next_intrusion() {
        let mut game = Game::new(504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Arm an Atk buff directly (models what a prebattle_buff consumable does).
        game.world.get_mut::<CombatBuff>(player).unwrap().active = Some(ActiveBuff {
            kind: BuffKind::Atk,
            remaining: 3,
            power: 5,
        });

        let wild = spawn_wild_on_player_tile(&mut game);
        game.start_battle(vec![wild]);

        let buff = game.world.get::<CombatBuff>(player).unwrap().active;
        assert!(
            matches!(
                buff,
                Some(ActiveBuff {
                    kind: BuffKind::Atk,
                    power: 5,
                    ..
                })
            ),
            "a buff armed before the fight must still be active when it starts"
        );
    }

    #[test]
    fn use_power_source_restores_power_and_consumes_one() {
        let mut game = Game::new(504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        // The player already starts holding Power Cells (see `Game::new`);
        // drain the default stock first so the stack is exactly 2 below.
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::POWER_CELL));
        inv.take(ItemId::from(ids::POWER_CELL), held);
        inv.add(ItemId::from(ids::POWER_CELL), 2);

        game.use_power_source();

        // `use_power_source` dispatches to `use_item`, which ends with
        // `self.tick()` like every other player action, so
        // `needs_decay_system` also shaves off one tick's worth of hunger
        // (see `HUNGER_DECAY_PER_TICK` in systems.rs) on top of the +25
        // restore — same shared-decay caveat as `use_item_applies_a_power_
        // restore_and_consumes_one` above.
        assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, 75.0 - 0.15);
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::POWER_CELL)),
            1
        );
    }

    #[test]
    fn use_power_source_with_nothing_to_recharge_from_is_a_no_op() {
        let mut game = Game::new(505, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        // Drain the default Power Cell stock (see `Game::new`) so no
        // power-restoring item remains; the Core Fragments the player also
        // starts with have no `consume` effect at all.
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::POWER_CELL));
        inv.take(ItemId::from(ids::POWER_CELL), held);
        let fragments_before = inv.count(&ItemId::from(ids::CORE_FRAGMENT));
        let hunger_before = game.world.get::<Needs>(player).unwrap().hunger;

        game.use_power_source();

        // No candidate item means no `use_item` dispatch, so unlike the
        // success path above there's no trailing `tick()` and hunger must
        // be untouched, not merely undecayed.
        assert_eq!(
            game.world.get::<Needs>(player).unwrap().hunger,
            hunger_before,
            "a failed recharge must not tick the game or touch Needs"
        );
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::CORE_FRAGMENT)),
            fragments_before,
            "a failed recharge must not consume an unrelated item"
        );
        assert!(
            game.message_log(10)
                .iter()
                .any(|(_, line)| line == "You have nothing to recharge from."),
            "expected the no-power-source message, got: {:?}",
            game.message_log(10)
        );
    }

    #[test]
    fn use_power_source_picks_the_power_item_over_an_earlier_non_power_item() {
        let mut game = Game::new(506, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
        // Drain all three starting stacks (see `Game::new`: Ice Breaker,
        // Power Cell, Core Fragment) and rebuild the inventory with the
        // non-power item (Core Fragment) added *first*, so it's ahead of
        // the Power Cell in `Inventory::items`. This pins selection to the
        // `ConsumeDef.power > 0.0` predicate rather than to iteration
        // order or to which `ItemId` happens to be checked first.
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let ice_breaker_held = inv.count(&ItemId::from(ids::ICE_BREAKER));
        inv.take(ItemId::from(ids::ICE_BREAKER), ice_breaker_held);
        let power_held = inv.count(&ItemId::from(ids::POWER_CELL));
        inv.take(ItemId::from(ids::POWER_CELL), power_held);
        let fragments_held = inv.count(&ItemId::from(ids::CORE_FRAGMENT));
        inv.take(ItemId::from(ids::CORE_FRAGMENT), fragments_held);
        inv.add(ItemId::from(ids::CORE_FRAGMENT), 5);
        inv.add(ItemId::from(ids::POWER_CELL), 2);
        assert_eq!(
            inv.items[0].0,
            ItemId::from(ids::CORE_FRAGMENT),
            "test setup: the non-power item must be first in iteration order"
        );

        game.use_power_source();

        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::POWER_CELL)),
            1,
            "the power-restoring item should have been the one consumed"
        );
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(&ItemId::from(ids::CORE_FRAGMENT)),
            5,
            "the earlier non-power item must be left untouched"
        );
        assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, 75.0 - 0.15);
    }
}
