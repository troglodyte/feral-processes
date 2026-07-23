pub mod balance;
pub mod battle;
pub mod components;
pub mod difficulty;
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
pub mod world;

use std::collections::HashMap;
use std::path::Path;

pub use bevy_ecs::prelude::Entity;
use bevy_ecs::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use components::{
    ActiveBuff, ActiveStatus, BuffKind, Creature, CustomName, Decompiler, Durability, Equipment,
    EquippedItem, Experience, FusionCount, Glyph, GlyphColor, Hostile, Inventory, ItemFusions,
    MAX_INDIVIDUAL_ROLL, MIN_INDIVIDUAL_ROLL, Needs, Nest, NestGuardian, PassiveProcessor, Perks,
    Player, PlayerBuff, Position, Potential, ResourceNode, Stats, StatusEffects, StatusKind,
    Structure, Tamed, Task, TaskKind, Temporary, WanderAi, ZonePortal,
};
use items::{EquipmentSlot, EquipmentStats, ItemId, ids};
use items_db::ItemDb;
pub use perks::Perk;
use research::{ResearchDb, ResearchDef};
pub use research::{ResearchId, ResearchRecipe};
use resources::{
    BattleState, EffectQueue, GameClock, GameOver, GameRng, MAX_PARTY_SIZE, MessageLog, Party,
    PlayerEntity, Research, ZoneLevel, ZoneSpawnPoint,
};
pub use resources::{DifficultyMode, EffectKind, MessageKind, VisualEffect};
use species::{MoveDef, SpecialAbility, SpeciesDb, SpeciesDef, SpeciesId};
use structures::{StructureDb, StructureDef, StructureId, TradeDef};
use world::{Biome, Tile, WorldMap};

/// How many ticks a full night's recharge cycle advances the clock by.
const REST_TICKS: u32 = 40;

/// Chance a wild program's retaliation targets the active companion instead
/// of the player, if one is present. The companion is a battle-worthy
/// program in its own right, not invulnerable cover.
const COMPANION_RETALIATION_CHANCE: f64 = 0.3;

/// Tile distance from `ZoneSpawnPoint` per step of `DISTANCE_STAT_STEP_BONUS`
/// — see `Game::distance_stat_multiplier`.
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

/// Tile distance from `ZoneSpawnPoint` per extra pack member a wild spawn
/// can roll — see `Game::max_pack_size`. Twice `DISTANCE_STAT_STEP_TILES`:
/// packs grow into their zone's cap more gradually than per-creature stats
/// do.
const PACK_SIZE_STEP_TILES: i32 = DISTANCE_STAT_STEP_TILES * 2;

/// How tightly a pack's members cluster around the tile a spawn roll
/// picked (`Game::try_spawn_habitat_creature`), and how far `gather_pack`
/// searches from whichever member the player bumped into — both use the
/// same radius so a whole spawned cluster is guaranteed to pull together
/// into one fight.
const PACK_GATHER_RADIUS: i32 = 2;

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

/// Battle rounds a companion's default rally buff (see
/// `Game::rally_player`) lasts when its species defines no
/// `special_ability`.
const RALLY_DURATION: u32 = 3;

/// Fatigue the player spends each time they command a companion in battle
/// (see `Game::battle_command_companion`) — the rally/special-ability
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
const RAID_CHANCE_PER_TICK: f64 = 0.02;

/// Damage a raid deals to a structure's `Durability` when it has no
/// assigned cronjob worker defending it.
const RAID_DAMAGE: u32 = 10;

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
const MAX_BUILD_DISTANCE_FROM_HOME: i32 = 15;

/// Fraction of a structure's current build cost refunded when it's removed
/// (see `Game::remove_structure`), rounded down per item. Applies uniformly
/// whether the structure is removed directly or swept up in a Home's
/// cascading removal.
const STRUCTURE_REMOVAL_REFUND_PERCENT: u32 = 30;

/// How often (in ticks) damaged structures passively regenerate — a slow
/// trickle, not a substitute for staying ahead of raids.
const STRUCTURE_REGEN_INTERVAL: u64 = 20;

/// How much `Durability` a damaged structure regenerates every
/// `STRUCTURE_REGEN_INTERVAL` ticks.
const STRUCTURE_REGEN_AMOUNT: u32 = 2;

/// One node of the research tree as the menus see it — see
/// `Game::research_nodes`.
pub struct ResearchStatus {
    pub id: ResearchId,
    pub name: String,
    pub description: String,
    pub cost: u32,
    pub state: ResearchState,
    /// Whether the player can pay `cost` right now. Independent of `state`:
    /// a node can be `Available` but unaffordable, or affordable but
    /// `Locked`.
    pub affordable: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ResearchState {
    Unlocked,
    Available,
    /// Display names of the prerequisites still missing — the menu shows
    /// *why* a node can't be taken rather than just greying it out.
    Locked {
        missing: Vec<String>,
    },
}

pub struct PlayerStatus {
    pub position: (i32, i32),
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    pub decompiler: i32,
    pub hunger: f32,
    pub fatigue: f32,
    pub inventory: Vec<(ItemId, u32)>,
    /// Units of ordinary cargo currently carried — what
    /// `inventory_capacity` limits. Excludes banked currency (see
    /// `ItemId::bank_limit`), so it will not match the sum of `inventory`
    /// when Research Data is held.
    pub inventory_used: u32,
    /// The player's current carrying capacity, base plus every deployed
    /// structure's `inventory_bonus`.
    pub inventory_capacity: u32,
    pub level: u32,
    pub xp: u32,
    pub xp_to_next: u32,
    pub weapon: Option<EquippedItem>,
    pub armor: Option<EquippedItem>,
    pub module: Option<EquippedItem>,
    /// The player's active battle party (see `resources::Party`), in
    /// party-slot order.
    pub companions: Vec<CompanionInfo>,
    /// Which zone sector the player is currently breached into. See
    /// `ZoneLevel`.
    pub zone: u32,
    /// Unspent Perk Points (see `perks::Perk`), earned 1 per level gained.
    pub perk_points: u32,
    /// Which perks have been unlocked so far.
    pub unlocked_perks: Vec<Perk>,
}

/// Full stats for one tamed program the player owns, wherever it is on the
/// map — shown by the pets/roster screen so you can check on (or manage) a
/// cronjob worker without walking over to it. See `Game::owned_pets`.
pub struct PetInfo {
    pub entity: Entity,
    pub name: String,
    pub level: u32,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    pub is_companion: bool,
    /// The label of the structure this pet is cronjob-assigned to, if any.
    pub job_structure: Option<String>,
    /// This individual's rolled quality tier (see `components::Potential`),
    /// e.g. "Excellent (94%)" — `None` for a creature with no `Potential`
    /// (shouldn't happen for anything spawned going forward, but possible
    /// for an old save predating this component).
    pub quality: Option<String>,
    /// How many fusions deep this program's lineage is, 0 to `MAX_FUSIONS`
    /// — see `components::FusionCount`. At `MAX_FUSIONS` it can no longer
    /// be fused.
    pub fusions: u32,
}

/// Snapshot of the player's active companion, shown in the status panel
/// and during an intrusion.
pub struct CompanionInfo {
    pub entity: Entity,
    pub name: String,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    /// The companion's current battle status condition, if any (see
    /// `status_label`) — e.g. "Bleeding (2)". Always `None` outside a
    /// battle, since status effects are scoped to a single intrusion.
    pub status: Option<String>,
    /// Terse name of what commanding this companion in battle would do
    /// right now (e.g. "Rally Team") — see `Game::companion_ability_label`.
    /// Shown in the Command Companion picker so the player can see what
    /// they're about to use before picking a party member with more than
    /// one active.
    pub ability: String,
}

#[derive(Clone)]
pub struct EntityView {
    pub entity: Entity,
    pub pos: (i32, i32),
    pub glyph: char,
    pub color: GlyphColor,
    pub label: String,
    pub is_player: bool,
    pub is_tamed: bool,
    pub is_companion: bool,
    pub is_hostile: bool,
    pub is_structure: bool,
    /// Whether this (structure) entity is the base's Home — the anchor for
    /// the 15-tile build radius, and the one whose removal cascades to
    /// every other structure (see `Game::remove_structure`).
    pub is_home: bool,
    pub is_boss: bool,
    pub can_work: bool,
    /// Whether this (structure) entity is a trading post (see
    /// `StructureDef::trade`).
    pub can_trade: bool,
    /// Whether this (tamed) entity is currently assigned to a cronjob.
    pub has_job: bool,
    /// If `has_job`, the label of the structure this (tamed) entity is
    /// assigned to.
    pub job_structure: Option<String>,
    /// If this is a structure, the label of the (tamed) entity currently
    /// working it via cronjob, if any.
    pub structure_worker: Option<String>,
    pub hp_fraction: Option<f32>,
    pub level: Option<u32>,
    /// If this is a structure, its current/max raid `Durability`.
    pub durability: Option<(u32, u32)>,
    /// How many fusions deep this (creature) entity's lineage is, 0 to
    /// `MAX_FUSIONS` — see `components::FusionCount`. At `MAX_FUSIONS` it
    /// can no longer be an input to a fusion, which the fuse menus show.
    pub fusions: u32,
}

pub struct BattleView {
    pub wild_name: String,
    pub wild_is_boss: bool,
    pub wild_hp: i32,
    pub wild_max_hp: i32,
    pub wild_atk: i32,
    pub wild_def: i32,
    /// A rough overall-strength scalar for the wild program — see
    /// `components::Stats::power`.
    pub wild_power: i32,
    pub player_hp: i32,
    pub player_max_hp: i32,
    pub player_atk: i32,
    pub player_def: i32,
    /// A rough overall-strength scalar for the player — see
    /// `components::Stats::power`.
    pub player_power: i32,
    pub player_decompiler: i32,
    pub log: Vec<String>,
    pub can_tame: bool,
    /// Estimated chance (0.0-1.0) that a decompile attempt would succeed
    /// right now, given the wild program's current HP fraction and its
    /// species' difficulty. Shown to the player even if they have no ICE
    /// Breaker yet, so they can decide whether it's worth going to compile one.
    pub decompile_chance: f32,
    /// The player's active battle party (see `resources::Party`), in
    /// party-slot order.
    pub companions: Vec<CompanionInfo>,
    /// The player's current battle status condition, if any — see `status_label`.
    pub player_status_effect: Option<String>,
    /// The wild program's current battle status condition, if any.
    pub wild_status_effect: Option<String>,
    /// How many more wild programs are waiting behind the current one in
    /// this pack (see `resources::BattleState::wild_creatures`) — 0 for an
    /// ordinary solo encounter.
    pub pack_remaining: usize,
}

/// One entry in `Game::craft_recipes` — compiling `result` consumes `cost`.
pub struct CraftRecipe {
    pub result: ItemId,
    pub cost: Vec<(ItemId, u32)>,
}

/// Full species-level detail on a single creature, shown by `Game::inspect`
/// so the player can scope a program out before bumping into it and
/// triggering an intrusion.
pub struct InspectView {
    pub name: String,
    pub glyph: char,
    pub color: GlyphColor,
    pub level: Option<u32>,
    pub hp: i32,
    pub max_hp: i32,
    pub atk: i32,
    pub def: i32,
    /// A rough overall-strength scalar — see `components::Stats::power`.
    pub power: i32,
    pub is_hostile: bool,
    pub is_tamed: bool,
    pub is_boss: bool,
    pub taming_difficulty: f32,
    /// Estimated decompile chance if an intrusion started right now, using
    /// the creature's current HP fraction — same formula as `BattleView`.
    pub decompile_chance: f32,
    pub habitats: Vec<Biome>,
    pub moves: Vec<MoveDef>,
    pub work_resource: Option<ItemId>,
    /// This individual's rolled quality tier (see `components::Potential`),
    /// e.g. "Excellent (94%)" — `None` for an entity with no `Potential`.
    pub quality: Option<String>,
    /// How many fusions deep this program's lineage is, 0 to `MAX_FUSIONS`
    /// — see `components::FusionCount`. At `MAX_FUSIONS` it can no longer
    /// be fused.
    pub fusions: u32,
}

pub struct Game {
    world: World,
    schedule: Schedule,
}

impl Game {
    pub fn new(seed: u32, difficulty: DifficultyMode, assets_dir: &Path) -> std::io::Result<Self> {
        let (species_db, mut load_warnings) = SpeciesDb::load_dir(&assets_dir.join("species"))?;
        let (structure_db, structure_warnings) =
            StructureDb::load_dir(&assets_dir.join("structures"))?;
        load_warnings.extend(structure_warnings);
        let (research_db, research_warnings) =
            ResearchDb::load_dir(&assets_dir.join("research"), &structure_db)?;
        load_warnings.extend(research_warnings);
        let (item_db, item_warnings) = ItemDb::load_dir(&assets_dir.join("items"))?;
        load_warnings.extend(item_warnings);
        let missing = item_db.missing_roles();
        if !missing.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "item set is missing required economy role(s): {}",
                    missing.join(", ")
                ),
            ));
        }

        let mut world_map = WorldMap::new(seed);
        let start = find_walkable_start(&mut world_map);

        let mut world = World::new();
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(research_db);
        world.insert_resource(item_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock::default());
        world.insert_resource(GameRng(StdRng::seed_from_u64(seed as u64)));
        world.insert_resource(MessageLog::default());
        world.insert_resource(EffectQueue::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(difficulty);
        world.insert_resource(Party::default());
        world.insert_resource(Research::default());
        world.insert_resource(ZoneLevel::default());
        world.insert_resource(ZoneSpawnPoint {
            x: start.0,
            y: start.1,
        });

        let player = world
            .spawn((
                Player,
                Position {
                    x: start.0,
                    y: start.1,
                },
                Glyph {
                    ch: '@',
                    color: GlyphColor::Cyan,
                },
                components::PLAYER_BASE_STATS,
                Needs::default(),
                Experience::default(),
                Decompiler::default(),
                Equipment::default(),
                Inventory {
                    items: vec![
                        (ItemId::from(ids::ICE_BREAKER), 3),
                        (ItemId::from(ids::POWER_CELL), 3),
                        (ItemId::from(ids::CORE_FRAGMENT), 5),
                    ],
                },
                ItemFusions::default(),
                StatusEffects::default(),
                PlayerBuff::default(),
                Perks::default(),
            ))
            .id();
        world.insert_resource(PlayerEntity(player));

        let mut schedule = Schedule::default();
        schedule.add_systems((
            systems::needs_decay_system,
            systems::wander_ai_system,
            systems::task_progress_system,
            systems::passive_process_system,
            difficulty::death_handling_system,
        ));

        let mut game = Self { world, schedule };
        for warning in load_warnings {
            game.log(warning);
        }
        game.spawn_initial_creatures(14);
        game.log("Connection established. You materialize at the edge of the Grid.");
        Ok(game)
    }

    pub fn load(path: &Path, assets_dir: &Path) -> std::io::Result<Self> {
        let data = save::load_from_file(path)?;
        let (species_db, mut load_warnings) = SpeciesDb::load_dir(&assets_dir.join("species"))?;
        let (structure_db, structure_warnings) =
            StructureDb::load_dir(&assets_dir.join("structures"))?;
        load_warnings.extend(structure_warnings);
        let (research_db, research_warnings) =
            ResearchDb::load_dir(&assets_dir.join("research"), &structure_db)?;
        load_warnings.extend(research_warnings);
        let (item_db, item_warnings) = ItemDb::load_dir(&assets_dir.join("items"))?;
        load_warnings.extend(item_warnings);
        let missing = item_db.missing_roles();
        if !missing.is_empty() {
            return Err(std::io::Error::new(
                std::io::ErrorKind::InvalidData,
                format!(
                    "item set is missing required economy role(s): {}",
                    missing.join(", ")
                ),
            ));
        }

        let mut world_map = WorldMap::new(data.seed);
        let overrides: HashMap<(i32, i32), Tile> = data.tile_overrides.into_iter().collect();
        world_map.restore_overrides(overrides);

        let mut world = World::new();
        world.insert_resource(species_db);
        world.insert_resource(structure_db);
        world.insert_resource(research_db);
        world.insert_resource(item_db);
        world.insert_resource(world_map);
        world.insert_resource(GameClock { tick: data.tick });
        world.insert_resource(GameRng(StdRng::seed_from_u64(data.seed as u64 ^ data.tick)));
        world.insert_resource(MessageLog::default());
        world.insert_resource(EffectQueue::default());
        world.insert_resource(GameOver::default());
        world.insert_resource(data.difficulty);
        world.insert_resource(Party::default());
        world.insert_resource(Research(data.researched.into_iter().collect()));
        world.insert_resource(ZoneLevel(data.zone));
        world.insert_resource(ZoneSpawnPoint {
            x: data.spawn_point.0,
            y: data.spawn_point.1,
        });

        let player = world
            .spawn((
                Player,
                Position {
                    x: data.player.position.0,
                    y: data.player.position.1,
                },
                Glyph {
                    ch: '@',
                    color: GlyphColor::Cyan,
                },
                Stats {
                    hp: data.player.hp,
                    max_hp: data.player.max_hp,
                    atk: data.player.atk,
                    def: data.player.def,
                },
                Needs {
                    hunger: data.player.hunger,
                    fatigue: data.player.fatigue,
                },
                Experience {
                    level: data.player.level,
                    xp: data.player.xp,
                    xp_to_next: data.player.xp_to_next,
                },
                Decompiler {
                    skill: data.player.decompiler,
                },
                Equipment {
                    weapon: data.player.weapon.map(|item| EquippedItem {
                        item,
                        level: data.player.weapon_level,
                        fusion_tier: data.player.weapon_fusion_tier,
                    }),
                    armor: data.player.armor.map(|item| EquippedItem {
                        item,
                        level: data.player.armor_level,
                        fusion_tier: data.player.armor_fusion_tier,
                    }),
                    module: data.player.module.map(|item| EquippedItem {
                        item,
                        level: data.player.module_level,
                        fusion_tier: data.player.module_fusion_tier,
                    }),
                },
                Inventory {
                    items: data.player.inventory,
                },
                ItemFusions {
                    tiers: data.player.item_fusions,
                },
                StatusEffects::default(),
                PlayerBuff::default(),
                Perks {
                    points: data.player.perk_points,
                    unlocked: data.player.unlocked_perks,
                },
            ))
            .id();
        world.insert_resource(PlayerEntity(player));

        let mut schedule = Schedule::default();
        schedule.add_systems((
            systems::needs_decay_system,
            systems::wander_ai_system,
            systems::task_progress_system,
            systems::passive_process_system,
            difficulty::death_handling_system,
        ));

        let mut game = Self { world, schedule };
        for warning in load_warnings {
            game.log(warning);
        }

        let mut pending_cronjobs: Vec<(Entity, save::CronjobSave)> = Vec::new();
        let mut party: Vec<Entity> = Vec::new();
        for c in data.creatures {
            let Some(species) = game.world.resource::<SpeciesDb>().get(&c.species).cloned() else {
                continue;
            };
            let is_companion = c.is_companion;
            let mut entity = game.world.spawn((
                Creature {
                    species: species.id.clone(),
                },
                Position {
                    x: c.position.0,
                    y: c.position.1,
                },
                Glyph {
                    ch: species.glyph,
                    color: species.color,
                },
                Stats {
                    hp: c.hp,
                    max_hp: c.max_hp,
                    atk: c.atk,
                    def: c.def,
                },
                Potential {
                    hp_roll: c.hp_roll,
                    atk_roll: c.atk_roll,
                    def_roll: c.def_roll,
                    growth_roll: c.growth_roll,
                },
                ZonePortal(c.zone),
                StatusEffects::default(),
                FusionCount(c.fusions),
            ));
            if let Some(name) = c.custom_name.clone() {
                entity.insert(CustomName(name));
            }
            if c.tamed {
                let creature_id = entity.id();
                entity.insert((
                    Tamed { owner: player },
                    Experience {
                        level: c.level,
                        xp: c.xp,
                        xp_to_next: c.xp_to_next,
                    },
                ));
                if is_companion {
                    party.push(creature_id);
                } else if let Some(cronjob) = c.cronjob {
                    pending_cronjobs.push((creature_id, cronjob));
                }
            } else {
                entity.insert((Hostile, WanderAi::default()));
            }
        }
        party.truncate(MAX_PARTY_SIZE);
        game.world.insert_resource(Party(party));

        let mut structure_positions: HashMap<(i32, i32), Entity> = HashMap::new();
        let currency = game.currency();
        for s in data.structures {
            let Some(def) = game.world.resource::<StructureDb>().get(&s.kind).cloned() else {
                continue;
            };
            let mut entity = game.world.spawn((
                Structure {
                    kind: def.id.clone(),
                },
                Position {
                    x: s.position.0,
                    y: s.position.1,
                },
                Glyph {
                    ch: def.glyph,
                    color: def.color,
                },
                Durability {
                    hp: s.durability.unwrap_or(def.durability).min(def.durability),
                    max_hp: def.durability,
                },
            ));
            let structure_id = entity.id();
            structure_positions.insert(s.position, structure_id);
            if let Some(amount) = s.resource_amount {
                let resource = def
                    .work
                    .as_ref()
                    .map(|w| w.produces.clone())
                    .unwrap_or_else(|| currency.clone());
                let capacity = def.work.as_ref().map(|w| w.capacity).unwrap_or(5);
                let level = def.work.as_ref().and_then(|w| w.level);
                entity.insert(ResourceNode {
                    resource,
                    amount,
                    capacity,
                    level,
                });
            }
            if def.passive_process.is_some() {
                entity.insert(PassiveProcessor::default());
            }
        }

        // Reconnect each restored cronjob to its target structure now that
        // both sides exist. A structure is matched by position (entity ids
        // aren't stable across a save/load round trip) — if it's gone,
        // the assignment is silently dropped rather than crashing.
        for (worker, cronjob) in pending_cronjobs {
            if let Some(&target) = structure_positions.get(&cronjob.target_position) {
                game.world.entity_mut(worker).insert(Task {
                    kind: match cronjob.kind {
                        save::CronjobKind::GatherResource => TaskKind::GatherResource,
                        save::CronjobKind::Guard => TaskKind::Guard,
                    },
                    target,
                    progress: cronjob.progress,
                    required: cronjob.required,
                });
            }
        }

        game.log("Session restored. Reconnecting to the Grid.");
        Ok(game)
    }

    pub fn save(&mut self, path: &Path) -> std::io::Result<()> {
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let stats = *self.world.get::<Stats>(player).unwrap();
        let needs = *self.world.get::<Needs>(player).unwrap();
        let exp = *self.world.get::<Experience>(player).unwrap();
        let decompiler = self.world.get::<Decompiler>(player).unwrap().skill;
        let equipment = self.world.get::<Equipment>(player).unwrap().clone();
        let inventory = self.world.get::<Inventory>(player).unwrap().items.clone();
        let item_fusions = self
            .world
            .get::<ItemFusions>(player)
            .map(|f| f.tiers.clone())
            .unwrap_or_default();
        let perks = self.world.get::<Perks>(player).cloned().unwrap_or_default();

        let party_entities = self.world.resource::<Party>().0.clone();
        let mut creatures = Vec::new();
        let mut creature_query = self.world.query::<(
            Entity,
            &Creature,
            &Position,
            &Stats,
            Option<&Tamed>,
            Option<&Experience>,
            Option<&Task>,
            Option<&ZonePortal>,
            Option<&CustomName>,
            Option<&Potential>,
            Option<&FusionCount>,
        )>();
        for (
            entity,
            creature,
            pos,
            stats,
            tamed,
            exp,
            task,
            spawn_zone,
            custom_name,
            potential,
            fusions,
        ) in creature_query.iter(&self.world)
        {
            let potential = potential.copied().unwrap_or(Potential::NEUTRAL);
            let cronjob = task.and_then(|t| {
                self.world
                    .get::<Position>(t.target)
                    .map(|target_pos| save::CronjobSave {
                        target_position: (target_pos.x, target_pos.y),
                        progress: t.progress,
                        required: t.required,
                        kind: match t.kind {
                            TaskKind::GatherResource => save::CronjobKind::GatherResource,
                            TaskKind::Guard => save::CronjobKind::Guard,
                        },
                    })
            });
            creatures.push(save::CreatureSave {
                species: creature.species.clone(),
                position: (pos.x, pos.y),
                hp: stats.hp,
                max_hp: stats.max_hp,
                atk: stats.atk,
                def: stats.def,
                tamed: tamed.is_some(),
                level: exp.map(|e| e.level).unwrap_or(1),
                xp: exp.map(|e| e.xp).unwrap_or(0),
                xp_to_next: exp.map(|e| e.xp_to_next).unwrap_or(20),
                cronjob,
                is_companion: party_entities.contains(&entity),
                zone: spawn_zone.map(|z| z.0).unwrap_or(1),
                custom_name: custom_name.map(|c| c.0.clone()),
                hp_roll: potential.hp_roll,
                atk_roll: potential.atk_roll,
                def_roll: potential.def_roll,
                growth_roll: potential.growth_roll,
                fusions: fusions.map(|f| f.0).unwrap_or(0),
            });
        }

        let mut structures = Vec::new();
        let mut structure_query = self.world.query::<(
            &Structure,
            &Position,
            Option<&ResourceNode>,
            Option<&Durability>,
        )>();
        for (structure, pos, node, durability) in structure_query.iter(&self.world) {
            structures.push(save::StructureSave {
                kind: structure.kind.clone(),
                position: (pos.x, pos.y),
                resource_amount: node.map(|n| n.amount),
                durability: durability.map(|d| d.hp),
            });
        }

        let tile_overrides = self
            .world
            .resource::<WorldMap>()
            .overrides()
            .iter()
            .map(|(k, v)| (*k, *v))
            .collect();

        let data = save::SaveData {
            seed: self.world.resource::<WorldMap>().seed(),
            tick: self.world.resource::<GameClock>().tick,
            difficulty: *self.world.resource::<DifficultyMode>(),
            player: save::PlayerSave {
                position: (pos.x, pos.y),
                hp: stats.hp,
                max_hp: stats.max_hp,
                atk: stats.atk,
                def: stats.def,
                hunger: needs.hunger,
                fatigue: needs.fatigue,
                inventory,
                level: exp.level,
                xp: exp.xp,
                xp_to_next: exp.xp_to_next,
                decompiler,
                weapon: equipment.weapon.as_ref().map(|e| e.item.clone()),
                weapon_level: equipment.weapon.as_ref().map(|e| e.level).unwrap_or(1),
                weapon_fusion_tier: equipment
                    .weapon
                    .as_ref()
                    .map(|e| e.fusion_tier)
                    .unwrap_or(0),
                armor: equipment.armor.as_ref().map(|e| e.item.clone()),
                armor_level: equipment.armor.as_ref().map(|e| e.level).unwrap_or(1),
                armor_fusion_tier: equipment.armor.as_ref().map(|e| e.fusion_tier).unwrap_or(0),
                module: equipment.module.as_ref().map(|e| e.item.clone()),
                module_level: equipment.module.as_ref().map(|e| e.level).unwrap_or(1),
                module_fusion_tier: equipment
                    .module
                    .as_ref()
                    .map(|e| e.fusion_tier)
                    .unwrap_or(0),
                item_fusions,
                perk_points: perks.points,
                unlocked_perks: perks.unlocked,
            },
            creatures,
            structures,
            tile_overrides,
            zone: self.world.resource::<ZoneLevel>().0,
            spawn_point: {
                let p = self.world.resource::<ZoneSpawnPoint>();
                (p.x, p.y)
            },
            researched: {
                let mut ids: Vec<ResearchId> = self
                    .world
                    .resource::<Research>()
                    .0
                    .iter()
                    .cloned()
                    .collect();
                ids.sort();
                ids
            },
        };
        save::save_to_file(path, &data)
    }

    pub fn history_summary(&mut self) -> Option<String> {
        let reason = self.world.resource::<GameOver>().reason.clone()?;
        let tick = self.world.resource::<GameClock>().tick;
        let mut query = self.world.query_filtered::<(), With<Tamed>>();
        let tamed_count = query.iter(&self.world).count();
        Some(format!(
            "Session ended at cycle {tick}: {reason}. Programs compiled: {tamed_count}."
        ))
    }

    pub fn write_history(&mut self, path: &Path) -> std::io::Result<()> {
        if let Some(summary) = self.history_summary() {
            save::append_run_history(path, &summary)
        } else {
            Ok(())
        }
    }

    fn player_entity(&self) -> Entity {
        self.world.resource::<PlayerEntity>().0
    }

    fn log(&mut self, s: impl Into<String>) {
        self.world.resource_mut::<MessageLog>().push(s);
    }

    fn log_kind(&mut self, kind: MessageKind, s: impl Into<String>) {
        self.world.resource_mut::<MessageLog>().push_kind(kind, s);
    }

    pub fn message_log(&self, n: usize) -> Vec<(MessageKind, String)> {
        self.world.resource::<MessageLog>().recent(n).to_vec()
    }

    pub fn is_game_over(&self) -> Option<String> {
        self.world.resource::<GameOver>().reason.clone()
    }

    /// How many ticks (see `tick`) have elapsed this session. Exposed so a
    /// caller (e.g. the TUI's autosave timer) can pace itself against game
    /// time rather than wall-clock time or its own separate counter.
    pub fn current_tick(&self) -> u64 {
        self.world.resource::<GameClock>().tick
    }

    pub fn has_active_battle(&self) -> bool {
        self.world.get_resource::<BattleState>().is_some()
    }

    /// Advances the world clock with no player action behind it — the hook
    /// a frontend's real-time loop calls once a second so the world keeps
    /// moving while the player is idle. A no-op during battle (turns there
    /// are paced by battle actions, not the wall clock) or after game over.
    pub fn idle_tick(&mut self) {
        if self.has_active_battle() {
            return;
        }
        self.tick();
    }

    fn tick(&mut self) {
        self.tick_inner(true);
    }

    /// Shared implementation behind `tick`. `age_temporary` controls
    /// whether this tick counts toward any `Temporary` structure's
    /// remaining lifespan (see `age_temporary_structures`) — `rest`'s
    /// internal loop passes `false` so resting near a Recharger Node
    /// doesn't burn down its lifespan any faster than leaving it standing
    /// idle would.
    fn tick_inner(&mut self, age_temporary: bool) {
        if self.is_game_over().is_some() {
            return;
        }
        self.maybe_spawn_wild_creature();
        self.schedule.run(&mut self.world);
        self.structure_regen();
        self.raid_check();
        self.nest_respawn_tick();
        if age_temporary {
            self.age_temporary_structures();
        }
        self.world.resource_mut::<GameClock>().tick += 1;
    }

    /// Ages every deployed `Temporary` structure by one tick, collapsing
    /// (despawning) any that just ran out — dropping a dangling
    /// cronjob/guard `Task` pointed at it the same way `remove_structure`
    /// does, but with no material refund since this is decay, not a
    /// deliberate demolition.
    fn age_temporary_structures(&mut self) {
        let expired: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &mut Temporary)>();
            query
                .iter_mut(&mut self.world)
                .filter_map(|(entity, mut temp)| {
                    temp.ticks_remaining = temp.ticks_remaining.saturating_sub(1);
                    (temp.ticks_remaining == 0).then_some(entity)
                })
                .collect()
        };
        for entity in expired {
            if let Some(kind) = self.world.get::<Structure>(entity).map(|s| s.kind.clone()) {
                let name = self
                    .world
                    .resource::<StructureDb>()
                    .get(&kind)
                    .map(|d| d.name.clone())
                    .unwrap_or(kind);
                self.log(format!("The {name} burns out and collapses."));
            }
            let workers: Vec<Entity> = {
                let mut tasks = self.world.query::<(Entity, &Task)>();
                tasks
                    .iter(&self.world)
                    .filter(|(_, t)| t.target == entity)
                    .map(|(w, _)| w)
                    .collect()
            };
            for worker in workers {
                self.world.entity_mut(worker).remove::<Task>();
            }
            self.world.despawn(entity);
        }
    }

    pub fn move_player(&mut self, dx: i32, dy: i32) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let (nx, ny) = (pos.x + dx, pos.y + dy);

        if let Some(target) = self.find_wild_creature_at(nx, ny) {
            let pack = self.gather_pack(target);
            self.start_battle(pack);
            self.tick();
            return;
        }
        if let Some(nest) = self.find_nest_at(nx, ny) {
            self.attack_nest(nest);
            self.tick();
            return;
        }
        if self.find_zone_portal_at(nx, ny).is_some() {
            self.enter_next_zone();
            self.tick();
            return;
        }
        if self.find_blocking_structure_at(nx, ny).is_some() {
            return;
        }
        let walkable = self.world.resource_mut::<WorldMap>().tile(nx, ny).walkable;
        if walkable {
            let mut p = self.world.get_mut::<Position>(player).unwrap();
            p.x = nx;
            p.y = ny;
        }
        self.tick();
    }

    pub fn eat(&mut self, item: ItemId) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        // Task 4 replaces this whole method with data-driven `use_item` and
        // deletes this transitional literal.
        if item != ItemId::from(ids::POWER_CELL) {
            self.log("You can't consume that.");
            return;
        }
        let player = self.player_entity();
        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item, 1);
        if taken == 0 {
            self.log("You have no power cells.");
            return;
        }
        {
            let mut needs = self.world.get_mut::<Needs>(player).unwrap();
            needs.hunger = (needs.hunger + 25.0).min(100.0);
        }
        self.log("You drain a power cell. Reserves replenished somewhat.");
        self.tick();
    }

    /// Power down for the night: many ticks pass at once (power reserves
    /// drain accordingly, tamed programs keep processing, rogue programs
    /// keep roaming), then Fatigue and Integrity are both restored to full.
    /// Requires the player to be standing within a Recharger Node's radius
    /// (`StructureDef::enables_rest`) — there's no other way to rest.
    /// Beyond that gate, there's no separate "rest" system beyond replaying
    /// the normal tick loop plus a Fatigue/HP reset at the end (via
    /// `tick_inner(false)`, so these ticks don't age the Recharger Node
    /// itself — see `age_temporary_structures`). If Power runs out and you
    /// take lethal damage mid-rest, the loop bails out via the
    /// `is_game_over` check before either restore happens.
    pub fn rest(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        if self.nearby_rest_structure(player_pos).is_none() {
            self.log("You need to be near a Recharger Node to power down and rest.");
            return;
        }
        self.log("You drop into low-power standby to recharge.");
        for _ in 0..REST_TICKS {
            if self.is_game_over().is_some() {
                return;
            }
            self.tick_inner(false);
        }
        let player = self.player_entity();
        {
            let mut needs = self.world.get_mut::<Needs>(player).unwrap();
            needs.fatigue = 100.0;
        }
        {
            let mut stats = self.world.get_mut::<Stats>(player).unwrap();
            stats.hp = stats.max_hp;
        }
        // Every tamed program you own gets fully healed too, not just your
        // active party — including any left behind defending a structure
        // from a raid while you were away.
        let owned: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Tamed), With<Creature>>();
            query
                .iter(&self.world)
                .filter(|(_, t)| t.owner == player)
                .map(|(e, _)| e)
                .collect()
        };
        for creature in owned {
            if let Some(mut stats) = self.world.get_mut::<Stats>(creature) {
                stats.hp = stats.max_hp;
            }
        }
        self.log("You come back online, fully recharged and repaired.");
    }

    /// Stand in place for a single tick — lets the world (wander AI,
    /// cronjob production, needs decay) advance by one step without moving
    /// or taking any other action. Distinct from `rest`, which advances
    /// `REST_TICKS` at once and restores Fatigue.
    pub fn wait(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        self.tick();
    }

    /// Awards unsolicited income — a scan find, battle loot, a boss cache —
    /// clamped to whatever room is left, returning how many units landed.
    /// Income clamps rather than refusing so a full buffer can never stall
    /// a battle from resolving or a cronjob worker from running; the loss
    /// is logged so it is never silent.
    fn grant_loot(&mut self, item: ItemId, qty: u32) -> u32 {
        let capacity = self.inventory_capacity();
        let player = self.player_entity();
        let added = self
            .world
            .resource_scope(|world, db: bevy_ecs::prelude::Mut<ItemDb>| {
                world.get_mut::<Inventory>(player).unwrap().add_capped(
                    item.clone(),
                    qty,
                    capacity,
                    &db,
                )
            });
        if added < qty {
            let lost = qty - added;
            let label = if self.bank_limit_of(&item).is_some() {
                "Research bank"
            } else {
                "Buffer"
            };
            let name = self.item_name(&item).to_string();
            self.log(format!("{label} full — {lost} {name} lost."));
        }
        added
    }

    /// Scan the current sector for salvageable Core Fragments. Chance
    /// depends on the sector's biome; besides starting inventory and combat
    /// drops, this and structure cronjobs are the only ways to replenish
    /// Core Fragments — the raw material Power Cells and ICE Breakers are
    /// compiled from (see `craft_recipes`).
    pub fn forage(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let player = self.player_entity();
        let pos = *self.world.get::<Position>(player).unwrap();
        let biome = self
            .world
            .resource_mut::<WorldMap>()
            .tile(pos.x, pos.y)
            .biome;
        let chance = forage_chance(biome, self.player_perk_level(Perk::KeenScavenger));
        let found = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance)
        };
        if found {
            if self.grant_loot(self.currency(), 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    "You scan the sector and recover a core fragment.",
                );
            }
        } else {
            self.log("You scan the sector but find nothing salvageable.");
        }
        self.tick();
    }

    /// How many levels of `perk` the player has bought — 0 if none.
    pub fn player_perk_level(&self, perk: Perk) -> u32 {
        let player = self.player_entity();
        self.world
            .get::<Perks>(player)
            .map(|p| p.level(perk))
            .unwrap_or(0)
    }

    /// The player's effective Decompiler skill for decompile-chance
    /// calculations: their real `Decompiler` stat plus
    /// `EXPLOIT_FOCUS_BONUS_PER_LEVEL` for every level of `Perk::ExploitFocus`.
    fn player_decompiler_skill(&self) -> i32 {
        let player = self.player_entity();
        let base = self
            .world
            .get::<Decompiler>(player)
            .map(|d| d.skill)
            .unwrap_or(0);
        base + EXPLOIT_FOCUS_BONUS_PER_LEVEL * self.player_perk_level(Perk::ExploitFocus) as i32
    }

    /// Spends Perk Points to buy another level of `perk` (see
    /// `perks::Perk`). Perks are repeatable — there's no cap on levels,
    /// only on how many Perk Points you've earned.
    pub fn unlock_perk(&mut self, perk: Perk) -> Result<(), String> {
        if self.is_game_over().is_some() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let level = {
            let mut perks = self
                .world
                .get_mut::<Perks>(player)
                .ok_or_else(|| "No perks available.".to_string())?;
            if perks.points < perk.cost() {
                return Err(format!(
                    "Not enough Perk Points (need {}, have {}).",
                    perk.cost(),
                    perks.points
                ));
            }
            perks.points -= perk.cost();
            perks.unlocked.push(perk);
            perks.level(perk)
        };
        match perk {
            Perk::Attacker => {
                if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
                    stats.atk += ATTACKER_BONUS_PER_LEVEL;
                }
            }
            Perk::Defender => {
                if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
                    stats.def += DEFENDER_BONUS_PER_LEVEL;
                }
            }
            Perk::Buffer => {
                if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
                    let bonus = ((stats.max_hp as f32 * BUFFER_BONUS_PERCENT_PER_LEVEL).round()
                        as i32)
                        .max(BUFFER_MIN_BONUS_PER_LEVEL);
                    stats.max_hp += bonus;
                    stats.hp = stats.max_hp;
                }
            }
            _ => {}
        }
        self.log(format!(
            "You buy the {} perk (level {level}).",
            perk.display_name()
        ));
        Ok(())
    }

    pub fn is_researched(&self, id: &str) -> bool {
        self.world.resource::<Research>().0.contains(id)
    }

    /// Display names of `def`'s prerequisites that aren't unlocked yet, in
    /// the order the file lists them.
    fn missing_prereqs(&self, def: &ResearchDef) -> Vec<String> {
        let db = self.world.resource::<ResearchDb>();
        def.requires
            .iter()
            .filter(|id| !self.is_researched(id))
            .map(|id| {
                db.get(id)
                    .map(|d| d.name.clone())
                    .unwrap_or_else(|| id.clone())
            })
            .collect()
    }

    /// Every research node, ordered the way the menu shows them: available
    /// first, then locked, then already-unlocked, each group cheapest-first
    /// (see `ResearchDb::all`). Ordering lives here rather than in each
    /// renderer so both peers agree on what `[3]` means.
    pub fn research_nodes(&self) -> Vec<ResearchStatus> {
        let research_currency = self.research_currency();
        let held = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(research_currency))
            .unwrap_or(0);
        let mut nodes: Vec<ResearchStatus> = self
            .world
            .resource::<ResearchDb>()
            .all()
            .map(|def| {
                let state = if self.is_researched(&def.id) {
                    ResearchState::Unlocked
                } else {
                    let missing = self.missing_prereqs(def);
                    if missing.is_empty() {
                        ResearchState::Available
                    } else {
                        ResearchState::Locked { missing }
                    }
                };
                ResearchStatus {
                    id: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    cost: def.cost,
                    state,
                    affordable: held >= def.cost,
                }
            })
            .collect();
        // `sort_by_key` is stable, so cheapest-first survives inside each group.
        nodes.sort_by_key(|n| match n.state {
            ResearchState::Available => 0,
            ResearchState::Locked { .. } => 1,
            ResearchState::Unlocked => 2,
        });
        nodes
    }

    /// Unlocks `id`, consuming its Research Data cost. Fails with an
    /// explicit message when the id is unknown, it's already unlocked, a
    /// prerequisite is missing, or the player can't pay.
    pub fn unlock_research(&mut self, id: &str) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let def = self
            .world
            .resource::<ResearchDb>()
            .get(id)
            .cloned()
            .ok_or_else(|| "Unknown research.".to_string())?;
        if self.is_researched(id) {
            return Err(format!("{} is already researched.", def.name));
        }
        let missing = self.missing_prereqs(&def);
        if !missing.is_empty() {
            return Err(format!("Requires {} first.", missing.join(", ")));
        }
        let player = self.player_entity();
        let research_currency = self.research_currency();
        let held = self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(research_currency.clone());
        if held < def.cost {
            return Err(format!("Not enough Research Data ({held}/{}).", def.cost));
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(research_currency, def.cost);
        self.world
            .resource_mut::<Research>()
            .0
            .insert(def.id.clone());
        self.log(format!("Research complete: {}.", def.name));
        Ok(())
    }

    /// The full list of things the player can compile right now: the
    /// always-available starter recipes (every item declaring a `craftable`
    /// def, see `assets/items/*.ron`), plus every recipe from an unlocked
    /// research node whose bench (`ResearchRecipe::requires_structure`) is
    /// currently deployed. Recipe data lives in `assets/{items,research}/*.ron`
    /// so a mod can add one without touching Rust.
    pub fn craft_recipes(&self) -> Vec<CraftRecipe> {
        let mut recipes: Vec<CraftRecipe> = self
            .world
            .resource::<ItemDb>()
            .all()
            .filter_map(|def| {
                def.craftable.as_ref().map(|c| CraftRecipe {
                    result: def.id.clone(),
                    cost: c.cost.clone(),
                })
            })
            .collect();
        for def in self.world.resource::<ResearchDb>().all() {
            if !self.is_researched(&def.id) {
                continue;
            }
            for recipe in &def.unlocks_recipes {
                let bench_ready = recipe
                    .requires_structure
                    .as_ref()
                    .is_none_or(|s| self.has_structure(s));
                if bench_ready {
                    recipes.push(CraftRecipe {
                        result: recipe.result.clone(),
                        cost: recipe.cost.clone(),
                    });
                }
            }
        }
        recipes
    }

    /// Whether a structure of `kind` exists anywhere right now. Every
    /// structure is player-built, so this doubles as "has the player built
    /// one of these" — backs `ResearchRecipe::requires_structure`, the bench
    /// a researched recipe needs deployed before it shows up (see
    /// `craft_recipes`).
    fn has_structure(&self, kind: &str) -> bool {
        self.world
            .iter_entities()
            .any(|e| e.get::<Structure>().is_some_and(|s| s.kind == kind))
    }

    /// The actual per-unit cost to compile `result` right now: its
    /// `craft_recipes` entry, with each quantity reduced by
    /// `LEAN_COMPILER_DISCOUNT_PER_LEVEL` for every level of
    /// `Perk::LeanCompiler` (down to a minimum of 1 each). Empty if
    /// `result` has no recipe.
    pub fn craft_cost(&self, result: ItemId) -> Vec<(ItemId, u32)> {
        let Some(recipe) = self
            .craft_recipes()
            .into_iter()
            .find(|r| r.result == result)
        else {
            return Vec::new();
        };
        let discount =
            LEAN_COMPILER_DISCOUNT_PER_LEVEL * self.player_perk_level(Perk::LeanCompiler);
        recipe
            .cost
            .into_iter()
            .map(|(item, qty)| (item, qty.saturating_sub(discount).max(1)))
            .collect()
    }

    /// The most whole units of `result` the player can afford to compile
    /// right now, given `craft_cost` (already Lean-Compiler-adjusted) and
    /// their current inventory. 0 if `result` has no recipe or they can't
    /// afford even one unit yet.
    pub fn max_craftable(&self, result: ItemId) -> u32 {
        let cost = self.craft_cost(result);
        if cost.is_empty() {
            return 0;
        }
        let inv = self.world.get::<Inventory>(self.player_entity()).unwrap();
        cost.iter()
            .map(|(item, qty)| inv.count(item.clone()) / (*qty).max(1))
            .min()
            .unwrap_or(0)
    }

    /// Compiles `quantity` units of `result` per its `craft_recipes` entry.
    pub fn craft(&mut self, result: ItemId, quantity: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if quantity == 0 {
            return Err("Compile at least 1.".into());
        }
        if self.craft_recipes().iter().all(|r| r.result != result) {
            return Err(format!("{} can't be compiled.", self.item_name(&result)));
        }
        let player = self.player_entity();
        let cost = self.craft_cost(result.clone());
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                if inv.count(item.clone()) < *qty * quantity {
                    return Err(format!(
                        "Compiling {} {} needs {} {}.",
                        quantity,
                        self.item_name(&result),
                        qty * quantity,
                        self.item_name(item)
                    ));
                }
            }
        }
        self.check_room(result.clone(), quantity)?;
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.take(item.clone(), *qty * quantity);
            }
            inv.add(result.clone(), quantity);
        }
        self.log_kind(
            MessageKind::Loot,
            format!(
                "You compile {} {} from salvaged components.",
                quantity,
                self.item_name(&result)
            ),
        );
        self.tick();
        Ok(())
    }

    /// Adds (`sign` = 1) or removes (`sign` = -1) an equipped item's stat
    /// bonus from the player's `Stats`/`Decompiler`. Shared by `equip` and
    /// `unequip` so the two stay symmetric.
    fn apply_equipment_delta(&mut self, player: Entity, mods: items::EquipmentStats, sign: i32) {
        if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
            stats.atk += sign * mods.atk;
            stats.def += sign * mods.def;
        }
        if mods.decompiler != 0
            && let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player)
        {
            decompiler.skill += sign * mods.decompiler;
        }
    }

    /// Equips `item` from inventory into its slot, swapping out (and
    /// returning to inventory) whatever was there before. The bonus applied
    /// is scaled for the current `resources::ZoneLevel` — see
    /// `items::EquipmentStats::scaled_for_level` — so gear equipped after
    /// breaching deeper is stronger than the same item equipped earlier.
    pub fn equip(&mut self, item: ItemId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let Some((slot, base_mods)) = self.equipment_of(&item) else {
            return Err(format!("{} can't be equipped.", self.item_name(&item)));
        };
        let player = self.player_entity();
        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item.clone(), 1);
        if taken == 0 {
            return Err(format!("You don't have a {}.", self.item_name(&item)));
        }
        let level = self.world.resource::<ZoneLevel>().0;
        let fusion_tier = self
            .world
            .get::<ItemFusions>(player)
            .map(|f| f.tier(item.clone()))
            .unwrap_or(0);

        let old_item = {
            let mut equipment = self.world.get_mut::<Equipment>(player).unwrap();
            equipment.slot_mut(slot).replace(EquippedItem {
                item: item.clone(),
                level,
                fusion_tier,
            })
        };
        if let Some(old) = old_item {
            let (_, old_base_mods) = self
                .equipment_of(&old.item)
                .ok_or_else(|| format!("{} can't be equipped.", self.item_name(&old.item)))?;
            self.apply_equipment_delta(
                player,
                old_base_mods
                    .scaled_for_level(old.level)
                    .fused_for_tier(old.fusion_tier),
                -1,
            );
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .add(old.item, 1);
        }
        self.apply_equipment_delta(
            player,
            base_mods
                .scaled_for_level(level)
                .fused_for_tier(fusion_tier),
            1,
        );
        let mut notes = Vec::new();
        if level > 1 {
            notes.push(format!("level {level}"));
        }
        if fusion_tier > 0 {
            notes.push(format!("fusion tier {fusion_tier}"));
        }
        let note = if notes.is_empty() {
            String::new()
        } else {
            format!(" ({})", notes.join(", "))
        };
        self.log(format!("You equip {}{note}.", self.item_name(&item)));
        self.tick();
        Ok(())
    }

    /// Unequips whatever's in `slot`, returning it to inventory.
    pub fn unequip(&mut self, slot: EquipmentSlot) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        // Room must be checked before the item leaves its Equipment slot: a
        // refusal after removal would leave the gear in neither place,
        // destroying it.
        let equipped_item = self
            .world
            .get::<Equipment>(player)
            .and_then(|e| e.get(slot))
            .map(|eq| eq.item);
        if let Some(item) = equipped_item {
            self.check_room(item, 1)?;
        }
        let removed = {
            let mut equipment = self.world.get_mut::<Equipment>(player).unwrap();
            equipment.slot_mut(slot).take()
        };
        let Some(equipped) = removed else {
            return Err(format!("Nothing equipped in your {} slot.", slot.label()));
        };
        let (_, base_mods) = self
            .equipment_of(&equipped.item)
            .ok_or_else(|| format!("{} can't be equipped.", self.item_name(&equipped.item)))?;
        self.apply_equipment_delta(
            player,
            base_mods
                .scaled_for_level(equipped.level)
                .fused_for_tier(equipped.fusion_tier),
            -1,
        );
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(equipped.item.clone(), 1);
        self.log(format!("You unequip {}.", self.item_name(&equipped.item)));
        self.tick();
        Ok(())
    }

    /// How many times `item` has been fused so far — see `fuse_item`.
    pub fn item_fusion_tier(&self, item: ItemId) -> u32 {
        self.world
            .get::<ItemFusions>(self.player_entity())
            .map(|f| f.tier(item))
            .unwrap_or(0)
    }

    /// Consumes `items::ITEM_FUSION_COST` copies of `item` from inventory to
    /// permanently boost that item type's equipped bonus by another
    /// `items::ITEM_FUSION_BONUS_PER_TIER` (see `ItemFusions`,
    /// `EquipmentStats::fused_for_tier`) — a sink for extra copies of gear
    /// you're not going to wear multiple of. Only equippable items qualify;
    /// the new tier applies the next time the item is equipped, not
    /// retroactively to a copy already worn.
    pub fn fuse_item(&mut self, item: ItemId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self.equipment_of(&item).is_none() {
            return Err(format!("{} can't be fused.", self.item_name(&item)));
        }
        let name = self.item_name(&item).to_string();
        let player = self.player_entity();
        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item.clone(), items::ITEM_FUSION_COST);
        if taken < items::ITEM_FUSION_COST {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .add(item.clone(), taken);
            return Err(format!(
                "Need {} {name} to fuse (have {taken}).",
                items::ITEM_FUSION_COST,
            ));
        }
        let mut fusions = self.world.get_mut::<ItemFusions>(player).unwrap();
        fusions.increment(item.clone());
        let tier = fusions.tier(item);
        self.log(format!(
            "You fuse {} {name} into a tier {tier} bonus ({}% stronger equipped).",
            items::ITEM_FUSION_COST,
            (tier as f64 * items::ITEM_FUSION_BONUS_PER_TIER * 100.0).round() as i32
        ));
        self.tick();
        Ok(())
    }

    /// Permanently removes `qty` of `item` from inventory. Only ever acts on
    /// unequipped inventory stock; an equipped item must be unequipped first.
    pub fn erase_item(&mut self, item: ItemId, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item.clone(), qty);
        if taken == 0 {
            return Err(format!("You don't have any {}.", self.item_name(&item)));
        }
        self.log(format!("You erase {taken} {}.", self.item_name(&item)));
        self.tick();
        Ok(())
    }

    pub fn place_structure(&mut self, structure_id: &str, dx: i32, dy: i32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't deploy right now.".into());
        }
        let def = self
            .world
            .resource::<StructureDb>()
            .get(structure_id)
            .cloned()
            .ok_or_else(|| "Unknown structure".to_string())?;
        if !self.structure_unlocked(structure_id) {
            return Err(format!("{} hasn't been researched yet.", def.name));
        }
        if structure_id != HOME_STRUCTURE_ID && !self.has_structure(HOME_STRUCTURE_ID) {
            return Err("Deploy a Home first before building anything else.".into());
        }
        if structure_id == HOME_STRUCTURE_ID && self.has_structure(HOME_STRUCTURE_ID) {
            return Err("A Home is already deployed. Remove it before building another.".into());
        }
        let player = self.player_entity();
        let ppos = *self.world.get::<Position>(player).unwrap();
        let (x, y) = (ppos.x + dx, ppos.y + dy);

        if structure_id != HOME_STRUCTURE_ID {
            let home = self.home_position().expect("checked above: a Home exists");
            if (x - home.x).abs() > MAX_BUILD_DISTANCE_FROM_HOME
                || (y - home.y).abs() > MAX_BUILD_DISTANCE_FROM_HOME
            {
                return Err(format!(
                    "Too far from Home — structures must be built within {MAX_BUILD_DISTANCE_FROM_HOME} tiles of it."
                ));
            }
        }

        let walkable = self.world.resource_mut::<WorldMap>().tile(x, y).walkable;
        if !walkable {
            return Err("Can't deploy onto that terrain.".into());
        }
        if self.find_blocking_structure_at(x, y).is_some() {
            return Err("Something is already deployed there.".into());
        }
        let build_cost = self.structure_build_cost(&def);
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &build_cost {
                if inv.count(item.clone()) < *qty {
                    return Err(format!("Not enough {}.", self.item_name(item)));
                }
            }
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &build_cost {
                inv.take(item.clone(), *qty);
            }
        }

        let mut entity = self.world.spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x, y },
            Glyph {
                ch: def.glyph,
                color: def.color,
            },
            Durability {
                hp: def.durability,
                max_hp: def.durability,
            },
        ));
        if let Some(work) = &def.work {
            entity.insert(ResourceNode {
                resource: work.produces.clone(),
                amount: work.capacity,
                capacity: work.capacity,
                level: work.level,
            });
        }
        if def.passive_process.is_some() {
            entity.insert(PassiveProcessor::default());
        }
        if let Some(temp) = &def.temporary {
            entity.insert(Temporary {
                ticks_remaining: temp.max_ticks,
            });
        }
        self.log(format!("You deploy a {}.", def.name));
        self.tick();
        Ok(())
    }

    /// Demolishes `structure`, refunding `STRUCTURE_REMOVAL_REFUND_PERCENT`
    /// of its current build cost. Removing the Home is a special case: it
    /// cascades to demolish every other structure along with it (each
    /// refunding its own share the same way), since nothing else can exist
    /// outside a Home's `MAX_BUILD_DISTANCE_FROM_HOME` radius anyway.
    /// Frontends are expected to warn the player about that cascade before
    /// calling this for a Home — this method itself performs the removal
    /// unconditionally, with no confirmation step of its own.
    pub fn remove_structure(&mut self, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let kind = self
            .world
            .get::<Structure>(structure)
            .ok_or_else(|| "That structure is already gone.".to_string())?
            .kind
            .clone();
        let is_home = kind == HOME_STRUCTURE_ID;
        let removed_name = self
            .world
            .resource::<StructureDb>()
            .get(&kind)
            .map(|d| d.name.clone())
            .unwrap_or(kind.clone());

        let mut targets = vec![structure];
        if is_home {
            let mut query = self.world.query::<(Entity, &Structure)>();
            targets.extend(
                query
                    .iter(&self.world)
                    .filter(|(e, s)| *e != structure && s.kind != HOME_STRUCTURE_ID)
                    .map(|(e, _)| e),
            );
        }
        let removed_count = targets.len();

        let mut refund: Vec<(ItemId, u32)> = Vec::new();
        for &target in &targets {
            let Some(target_kind) = self.world.get::<Structure>(target).map(|s| s.kind.clone())
            else {
                continue;
            };
            let Some(def) = self
                .world
                .resource::<StructureDb>()
                .get(&target_kind)
                .cloned()
            else {
                continue;
            };
            for (item, qty) in self.structure_build_cost(&def) {
                let share = qty * STRUCTURE_REMOVAL_REFUND_PERCENT / 100;
                if share == 0 {
                    continue;
                }
                match refund.iter_mut().find(|(i, _)| *i == item) {
                    Some((_, total)) => *total += share,
                    None => refund.push((item, share)),
                }
            }
            let workers: Vec<Entity> = {
                let mut tasks = self.world.query::<(Entity, &Task)>();
                tasks
                    .iter(&self.world)
                    .filter(|(_, t)| t.target == target)
                    .map(|(w, _)| w)
                    .collect()
            };
            for worker in workers {
                self.world.entity_mut(worker).remove::<Task>();
            }
            self.world.despawn(target);
        }

        // Route through `grant_loot`, not a direct `add`: demolishing a Home
        // cascades every other structure's refund in one shot, easily
        // enough to blow past the buffer with no message.
        for (item, qty) in &refund {
            self.grant_loot(item.clone(), *qty);
        }
        let refund_note = if refund.is_empty() {
            String::new()
        } else {
            let parts: Vec<String> = refund
                .iter()
                .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
                .collect();
            format!(" You recover {}.", parts.join(", "))
        };
        if is_home && removed_count > 1 {
            self.log_kind(
                MessageKind::Loot,
                format!(
                    "You demolish the Home — without it, {} other base structure{} collapse{}.{refund_note}",
                    removed_count - 1,
                    if removed_count - 1 == 1 { "" } else { "s" },
                    if removed_count - 1 == 1 { "s" } else { "" },
                ),
            );
        } else {
            self.log_kind(
                MessageKind::Loot,
                format!("You demolish the {removed_name}.{refund_note}"),
            );
        }
        self.tick();
        Ok(())
    }

    pub fn assign_cronjob(&mut self, worker: Entity, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let owner = self
            .world
            .get::<Tamed>(worker)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        if self.world.get::<ResourceNode>(structure).is_none() {
            return Err("That structure can't be worked.".into());
        }
        let structure_kind = self.world.get::<Structure>(structure).unwrap().kind.clone();
        let ticks = self
            .world
            .resource::<StructureDb>()
            .get(&structure_kind)
            .and_then(|d| d.work.as_ref())
            .map(|w| w.ticks_per_unit)
            .unwrap_or(5);
        if self.world.resource::<Party>().0.contains(&worker) {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|&e| e != worker);
            self.log("It stands down as your companion to run this cronjob.");
        }
        self.world.entity_mut(worker).insert(Task {
            kind: TaskKind::GatherResource,
            target: structure,
            progress: 0,
            required: ticks,
        });
        self.log("Cronjob scheduled.");
        self.tick();
        Ok(())
    }

    /// Posts `worker` (a tamed program you own) to guard `structure`
    /// against raids (see `raid_check`), without assigning it a cronjob.
    /// Unlike `assign_cronjob`, this works on any structure — including
    /// ones with no `work` recipe at all, like a Home or Terminal — since
    /// defending doesn't require producing anything. A structure that's
    /// already cronjob-worked is already defended by its worker; this is
    /// for posting a guard on structures that otherwise have no defender.
    pub fn assign_guard(&mut self, worker: Entity, structure: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let owner = self
            .world
            .get::<Tamed>(worker)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        if self.world.get::<Structure>(structure).is_none() {
            return Err("That's not a structure.".into());
        }
        if self.world.resource::<Party>().0.contains(&worker) {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|&e| e != worker);
            self.log("It stands down as your companion to guard this structure.");
        }
        self.world.entity_mut(worker).insert(Task {
            kind: TaskKind::Guard,
            target: structure,
            progress: 0,
            required: 0,
        });
        self.log("It takes up a defensive position.");
        self.tick();
        Ok(())
    }

    /// Every alive `Hostile` creature within `PACK_GATHER_RADIUS` tiles of
    /// `anchor` (Chebyshev distance) — the whole cluster a group spawn
    /// roll placed together (see `try_spawn_habitat_creature`) joins the
    /// fight at once when the player bumps into any one of them. `anchor`
    /// is always first, becoming the initial front target. Truncated to
    /// `max_pack_size` at `anchor`'s own position as a safety cap, in case
    /// unrelated wandering creatures happened to drift into the same
    /// cluster since they spawned.
    fn gather_pack(&mut self, anchor: Entity) -> Vec<Entity> {
        let Some(anchor_pos) = self.world.get::<Position>(anchor).copied() else {
            return vec![anchor];
        };
        let mut pack = vec![anchor];
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Hostile>>();
        for (e, pos) in query.iter(&self.world) {
            if e == anchor {
                continue;
            }
            let dist = (pos.x - anchor_pos.x)
                .abs()
                .max((pos.y - anchor_pos.y).abs());
            if dist <= PACK_GATHER_RADIUS {
                pack.push(e);
            }
        }
        let cap = (self.max_pack_size(anchor_pos.x, anchor_pos.y) as usize).max(1);
        pack.truncate(cap);
        pack
    }

    fn start_battle(&mut self, pack: Vec<Entity>) {
        let player = self.player_entity();
        let anchor = pack[0];
        let name = self
            .world
            .get::<Creature>(anchor)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.name.clone())
            .unwrap_or_else(|| "program".to_string());
        let others = pack.len() - 1;
        self.world.insert_resource(BattleState {
            player,
            wild_creatures: pack,
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        if others > 0 {
            self.log(format!(
                "A pack of rogue programs intercepts your signal — a {name} takes point, {others} more behind it!"
            ));
        } else {
            self.log(format!("A rogue {name} intercepts your signal!"));
        }
    }

    /// The front pack member of the active battle, if any — see
    /// `resources::BattleState::wild_creatures`.
    fn front_wild_creature(&self) -> Option<Entity> {
        self.world
            .get_resource::<BattleState>()?
            .wild_creatures
            .first()
            .copied()
    }

    pub fn battle_view(&self) -> Option<BattleView> {
        let battle = self.world.get_resource::<BattleState>()?;
        let wild = *battle.wild_creatures.first()?;
        let pack_remaining = battle.wild_creatures.len() - 1;
        let wild_stats = self.world.get::<Stats>(wild)?;
        let wild_creature = self.world.get::<Creature>(wild)?;
        let species_db = self.world.get_resource::<SpeciesDb>()?;
        let species = species_db.get(&wild_creature.species);
        let wild_name = species
            .map(|s| self.zone_tagged_name(wild, s.name.clone()))
            .unwrap_or_default();
        let wild_is_boss = species.is_some_and(|s| s.is_boss);
        let taming_difficulty = species.map(|s| s.taming_difficulty).unwrap_or(0.5);
        let player_stats = self.world.get::<Stats>(battle.player)?;
        let decompiler_skill = self.player_decompiler_skill();
        let can_tame = self
            .world
            .get::<Inventory>(battle.player)
            .map(|i| i.count(ItemId::from(ids::ICE_BREAKER)) > 0)
            .unwrap_or(false);
        let potency = self
            .world
            .resource::<ItemDb>()
            .get(ids::ICE_BREAKER)
            .and_then(|d| d.taming_potency)
            .unwrap_or(0.0);
        let decompile_chance = taming::capture_chance(
            wild_stats.hp_fraction(),
            potency,
            taming_difficulty,
            decompiler_skill,
        );
        let player_atk = self.effective_atk(battle.player);
        let player_def = self.effective_def(battle.player);
        Some(BattleView {
            wild_name,
            wild_is_boss,
            wild_hp: wild_stats.hp,
            wild_max_hp: wild_stats.max_hp,
            wild_atk: wild_stats.atk,
            wild_def: wild_stats.def,
            wild_power: wild_stats.power(),
            player_hp: player_stats.hp,
            player_max_hp: player_stats.max_hp,
            player_atk,
            player_power: player_stats.max_hp + player_atk + player_def,
            player_def,
            player_decompiler: decompiler_skill,
            log: battle.log.clone(),
            can_tame,
            decompile_chance,
            pack_remaining,
            player_status_effect: self.status_label(battle.player),
            wild_status_effect: self.status_label(wild),
            companions: self.party_info(),
        })
    }

    pub fn battle_attack(&mut self) {
        if self.is_game_over().is_some() {
            return;
        }
        let Some((player, front)) = self
            .world
            .get_resource::<BattleState>()
            .and_then(|b| b.wild_creatures.first().map(|&w| (b.player, w)))
        else {
            return;
        };

        if self.is_stunned(player) {
            self.log("Your process stalls — stunned, you lose this turn!");
        } else {
            let (p_atk, w_def) = {
                let w = *self.world.get::<Stats>(front).unwrap();
                (self.effective_atk(player), w.def)
            };
            let dmg = battle::compute_damage(p_atk, w_def, 5);
            self.apply_damage(front, dmg);
            self.log(format!("You unleash a data strike for {dmg} damage."));

            if !self.creature_alive(front) && self.finish_front_pack_member(player) {
                self.tick();
                return;
            }
        }

        self.resolve_post_action(player);
        self.tick();
    }

    /// Commands `companion` (a member of the active party — see
    /// `resources::Party`) to act this round instead of the player: it
    /// grants the player a temporary combat buff rather than attacking
    /// directly, using its species' `special_ability` if it has one, or a
    /// generic ATK rally otherwise. The wild pack's retaliation (see
    /// `resolve_post_action`) can still land on the player or any party
    /// member regardless of who acted this round.
    pub fn battle_command_companion(&mut self, companion: Entity) {
        if self.is_game_over().is_some() {
            return;
        }
        let Some(player) = self.world.get_resource::<BattleState>().map(|b| b.player) else {
            return;
        };
        if !self.world.resource::<Party>().0.contains(&companion) {
            self.log("That program isn't in your active party.");
            return;
        }
        let name = self.creature_label(companion);

        if self.is_stunned(companion) {
            self.log(format!("{name} stalls — stunned, it can't act!"));
        } else {
            let Some(front) = self.front_wild_creature() else {
                return;
            };
            let ability = self
                .world
                .get::<Creature>(companion)
                .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
                .and_then(|s| s.special_ability.clone());
            match ability {
                Some(ability) => self.use_special_ability(&ability, &name, player, front),
                None => self.rally_player(companion, &name, player),
            }
            if let Some(mut needs) = self.world.get_mut::<Needs>(player) {
                needs.fatigue = (needs.fatigue - COMPANION_COMMAND_FATIGUE_COST).max(0.0);
            }
        }

        self.resolve_post_action(player);
        self.tick();
    }

    /// Every currently-alive member of the active pack retaliates this
    /// round — the core of what makes a multi-creature pack meaningfully
    /// more dangerous than a solo encounter of the same species. Each one
    /// independently rolls its own move and target (see `wild_retaliate`).
    fn all_wild_retaliate(&mut self, player: Entity) {
        let Some(battle) = self.world.get_resource::<BattleState>() else {
            return;
        };
        let pack = battle.wild_creatures.clone();
        for wild in pack {
            if self.creature_alive(wild) {
                self.wild_retaliate(wild, player);
            }
        }
    }

    /// Drops the current front pack member from
    /// `BattleState::wild_creatures` (the caller is responsible for
    /// whatever happened to it — a kill or a successful tame). Returns
    /// whether that emptied the pack.
    fn pop_front_pack_member(&mut self) -> bool {
        let mut battle = self.world.resource_mut::<BattleState>();
        if !battle.wild_creatures.is_empty() {
            battle.wild_creatures.remove(0);
        }
        battle.wild_creatures.is_empty()
    }

    /// Handles the front pack member dying (from a direct hit or a status
    /// tick): logs the kill, awards its loot/XP, despawns it, and drops it
    /// from the pack. If that was the last member, the whole encounter
    /// ends in a win (`BattleState` removed) and this returns `true`;
    /// otherwise the next pack member becomes the new front and the fight
    /// continues, returning `false`.
    fn finish_front_pack_member(&mut self, player: Entity) -> bool {
        let Some(front) = self.front_wild_creature() else {
            return true;
        };
        self.log("The rogue program crashes and deletes itself!");
        let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
        self.award_player_xp(player, wild_max_hp as u32);
        self.award_loot(front);
        let nest = self.world.get::<NestGuardian>(front).map(|g| g.nest);
        self.world.despawn(front);
        if let Some(nest) = nest
            && let Some(mut n) = self.world.get_mut::<Nest>(nest)
        {
            n.pending_respawns.push(NEST_RESPAWN_TICKS);
        }
        if self.pop_front_pack_member() {
            self.clear_battle_status_effects(player, front);
            self.world.remove_resource::<BattleState>();
            true
        } else {
            self.log("Another rogue program from the pack engages!");
            false
        }
    }

    /// Shared end-of-round resolution used by every battle action once the
    /// player's (or a companion's) move has resolved: the whole pack
    /// retaliates, status effects tick for the front target and player,
    /// and a status-effect kill (e.g. a lingering Bleed finishing off the
    /// front) or the player's death is handled the same way a direct hit
    /// would be.
    fn resolve_post_action(&mut self, player: Entity) {
        self.all_wild_retaliate(player);
        let Some(front) = self.front_wild_creature() else {
            return;
        };
        self.tick_all_status_effects(front, player);
        if !self.creature_alive(front) {
            self.finish_front_pack_member(player);
            return;
        }
        if !self.creature_alive(player) {
            self.clear_battle_status_effects(player, front);
            self.world.remove_resource::<BattleState>();
        }
    }

    /// Default companion command when its species defines no
    /// `special_ability`: rallies the player, temporarily boosting their
    /// ATK by a third of the companion's own — a stronger companion grants
    /// a stronger rally.
    fn rally_player(&mut self, companion: Entity, name: &str, player: Entity) {
        let power = (self.world.get::<Stats>(companion).unwrap().atk / 3).max(1);
        if let Some(mut buff) = self.world.get_mut::<PlayerBuff>(player) {
            buff.active = Some(ActiveBuff {
                kind: BuffKind::Atk,
                remaining: RALLY_DURATION,
                power,
            });
        }
        self.log(format!("{name} rallies you, boosting your attack!"));
    }

    /// Executes `ability` (a companion's `SpeciesDef::special_ability`) on
    /// behalf of `companion`'s command — see `battle_command_companion`.
    fn use_special_ability(
        &mut self,
        ability: &SpecialAbility,
        name: &str,
        player: Entity,
        wild: Entity,
    ) {
        match *ability {
            SpecialAbility::Rally { power, duration } => {
                if let Some(mut buff) = self.world.get_mut::<PlayerBuff>(player) {
                    buff.active = Some(ActiveBuff {
                        kind: BuffKind::Atk,
                        remaining: duration,
                        power,
                    });
                }
                self.log(format!("{name} rallies you, boosting your attack!"));
            }
            SpecialAbility::Shield { power, duration } => {
                if let Some(mut buff) = self.world.get_mut::<PlayerBuff>(player) {
                    buff.active = Some(ActiveBuff {
                        kind: BuffKind::Def,
                        remaining: duration,
                        power,
                    });
                }
                self.log(format!("{name} shields you, boosting your defense!"));
            }
            SpecialAbility::Heal { power } => {
                if let Some(mut stats) = self.world.get_mut::<Stats>(player) {
                    stats.hp = (stats.hp + power).min(stats.max_hp);
                }
                self.log(format!("{name} patches your process for {power} HP."));
            }
            SpecialAbility::Debuff {
                kind,
                power,
                duration,
            } => {
                if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(wild) {
                    statuses.active = Some(ActiveStatus {
                        kind,
                        remaining: duration,
                        power,
                    });
                }
                match kind {
                    StatusKind::Bleed => {
                        self.log(format!("{name} corrupts the rogue program's data!"))
                    }
                    StatusKind::Stun => self.log(format!("{name} locks up the rogue program!")),
                }
            }
        }
    }

    /// `entity`'s effective ATK for damage purposes: its real `Stats`
    /// value, plus an active `PlayerBuff::Atk` bonus if any. If `entity` is
    /// the player, this also adds the standing party bonus (see
    /// `party_stat_bonus`) and applies the low-power attack penalty (see
    /// `battle::power_attack_multiplier`) — both are player-only effects.
    /// `entity` isn't always the player: `wild_retaliate` can call this
    /// (via `effective_def`) with a companion that's eating the hit
    /// instead, and a companion has neither a `Party` bonus of its own nor
    /// `Needs` to run low on.
    fn effective_atk(&self, entity: Entity) -> i32 {
        let base = self.world.get::<Stats>(entity).map(|s| s.atk).unwrap_or(0);
        let bonus = self
            .world
            .get::<PlayerBuff>(entity)
            .and_then(|b| b.active)
            .filter(|a| a.kind == BuffKind::Atk)
            .map(|a| a.power)
            .unwrap_or(0);
        if entity != self.player_entity() {
            return base + bonus;
        }
        let total = base + bonus + self.party_stat_bonus().0;
        let hunger = self
            .world
            .get::<Needs>(entity)
            .map(|n| n.hunger)
            .unwrap_or(100.0);
        ((total as f32) * battle::power_attack_multiplier(hunger)).round() as i32
    }

    /// `entity`'s effective DEF against incoming damage: its real `Stats`
    /// value, plus an active `PlayerBuff::Def` bonus if any, plus the
    /// standing party bonus (see `party_stat_bonus`) if `entity` is the
    /// player. Same non-player-safe behavior as `effective_atk`.
    fn effective_def(&self, entity: Entity) -> i32 {
        let base = self.world.get::<Stats>(entity).map(|s| s.def).unwrap_or(0);
        let bonus = self
            .world
            .get::<PlayerBuff>(entity)
            .and_then(|b| b.active)
            .filter(|a| a.kind == BuffKind::Def)
            .map(|a| a.power)
            .unwrap_or(0);
        if entity != self.player_entity() {
            return base + bonus;
        }
        base + bonus + self.party_stat_bonus().1
    }

    /// Standing `(atk, def)` bonus the player gets just for having programs
    /// in their active party — each member contributes 10% of its own
    /// current ATK and DEF (minimum 1 each), summed across the party.
    /// Computed live from each companion's current `Stats` rather than
    /// baked into the player's own `Stats` on add/remove, so it stays
    /// correct automatically as a companion levels up, is fused, or dies —
    /// no separate bookkeeping to keep in sync.
    fn party_stat_bonus(&self) -> (i32, i32) {
        self.world
            .resource::<Party>()
            .0
            .iter()
            .filter_map(|&e| self.world.get::<Stats>(e))
            .fold((0, 0), |(atk, def), s| {
                (atk + (s.atk / 10).max(1), def + (s.def / 10).max(1))
            })
    }

    /// Defeated (not tamed) rogue programs drop whatever resource their
    /// species is associated with, if any — the same `work_resource` used
    /// to decide what a tamed member of that species can gather.
    fn award_loot(&mut self, wild: Entity) {
        let Some(species_id) = self.world.get::<Creature>(wild).map(|c| c.species.clone()) else {
            return;
        };
        let Some(species) = self.world.resource::<SpeciesDb>().get(&species_id).cloned() else {
            return;
        };

        if let Some(resource) = species.work_resource {
            let qty = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(1..=2)
            };
            let landed = self.grant_loot(resource.clone(), qty);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("It drops {} {}.", landed, self.item_name(&resource)),
                );
            }
        }

        if let Some((item, chance)) = species.equipment_drop {
            let roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(chance as f64)
            };
            if roll && self.grant_loot(item.clone(), 1) > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("It also drops a {}!", self.item_name(&item)),
                );
            }
        }

        if species.is_boss {
            let qty = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(BOSS_PORTAL_FRAGMENT_DROP)
            };
            let landed = self.grant_loot(self.craft_currency(), qty);
            if landed > 0 {
                self.log_kind(
                    MessageKind::Loot,
                    format!("Its crash leaves behind a cache of {landed} portal fragments!"),
                );
            }
        } else {
            let portal_fragment_roll = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(PORTAL_FRAGMENT_DROP_CHANCE)
            };
            if portal_fragment_roll && self.grant_loot(self.craft_currency(), 1) > 0 {
                self.log_kind(MessageKind::Loot, "It leaves behind a portal fragment.");
            }
        }
    }

    /// Awards `amount` XP to the player, growing stats and fully healing on
    /// any level-up gained, then awards every current party member half as
    /// much (see `award_party_xp`) — fighting beside you pays off even on
    /// rounds where only the player's hit actually lands. Silently does
    /// nothing for the player if they're somehow missing an `Experience`
    /// component (shouldn't happen in practice).
    fn award_player_xp(&mut self, player: Entity, amount: u32) {
        let (levels, new_level) = {
            let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
            let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, player) else {
                return;
            };
            let levels = progression::add_xp(
                &mut exp,
                &mut stats,
                amount,
                progression::BASELINE_GROWTH_MULTIPLIER,
                // The player has no level ceiling — only creatures do.
                None,
            );
            (levels, exp.level)
        };
        if levels > 0 {
            if let Some(mut decompiler) = self.world.get_mut::<Decompiler>(player) {
                decompiler.skill += DECOMPILER_SKILL_PER_LEVEL * levels as i32;
            }
            if let Some(mut perks) = self.world.get_mut::<Perks>(player) {
                perks.points += PERK_POINTS_PER_LEVEL * levels;
            }
            self.log_kind(
                MessageKind::LevelUp,
                format!("You gain {amount} XP and reach level {new_level}!"),
            );
        } else {
            self.log(format!("You gain {amount} XP."));
        }
        self.award_party_xp(amount / PARTY_XP_DIVISOR);
    }

    /// Awards `amount` XP to every program in the active party (see
    /// `resources::Party`), each independently able to level up from it —
    /// the party-wide, half-rate companion to `award_player_xp`. A no-op
    /// for any party member somehow missing `Experience` (shouldn't happen
    /// in practice) or if the party is empty. Only logs a level-up, not
    /// every ordinary gain, so a busy fight doesn't flood the feed with a
    /// line per party member per kill.
    fn award_party_xp(&mut self, amount: u32) {
        if amount == 0 {
            return;
        }
        let party = self.world.resource::<Party>().0.clone();
        for companion in party {
            let species_growth = self
                .world
                .get::<Creature>(companion)
                .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
                .map(|s| s.growth_multiplier)
                .unwrap_or(progression::BASELINE_GROWTH_MULTIPLIER);
            let individual_roll = self
                .world
                .get::<Potential>(companion)
                .map(|p| p.growth_roll)
                .unwrap_or(Potential::NEUTRAL.growth_roll);
            let growth_multiplier = species_growth * individual_roll;
            let leveled = {
                let mut query = self.world.query::<(&mut Experience, &mut Stats)>();
                let Ok((mut exp, mut stats)) = query.get_mut(&mut self.world, companion) else {
                    continue;
                };
                progression::add_xp(
                    &mut exp,
                    &mut stats,
                    amount,
                    growth_multiplier,
                    Some(progression::CREATURE_MAX_LEVEL),
                ) > 0
            };
            if leveled {
                let name = self.creature_label(companion);
                let level = self.world.get::<Experience>(companion).unwrap().level;
                self.log_kind(
                    MessageKind::LevelUp,
                    format!("{name} gains {amount} XP and levels up to {level}!"),
                );
            }
        }
    }

    pub fn battle_decompile(&mut self) {
        if self.is_game_over().is_some() {
            return;
        }
        let Some(player) = self.world.get_resource::<BattleState>().map(|b| b.player) else {
            return;
        };

        if self.is_stunned(player) {
            self.log("Your process stalls — stunned, you lose this turn!");
            self.resolve_post_action(player);
            self.tick();
            return;
        }

        let taken = self
            .world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(ItemId::from(ids::ICE_BREAKER), 1);
        if taken == 0 {
            self.log("You have no ICE Breaker.");
            return;
        }

        let Some(front) = self.front_wild_creature() else {
            return;
        };
        let (hp_fraction, species_id) = {
            let stats = *self.world.get::<Stats>(front).unwrap();
            let species = self.world.get::<Creature>(front).unwrap().species.clone();
            (stats.hp_fraction(), species)
        };
        let taming_difficulty = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .map(|s| s.taming_difficulty)
            .unwrap_or(0.5);
        let potency = self
            .world
            .resource::<ItemDb>()
            .get(ids::ICE_BREAKER)
            .and_then(|d| d.taming_potency)
            .unwrap_or(0.0);
        let decompiler_skill = self.player_decompiler_skill();
        let chance =
            taming::capture_chance(hp_fraction, potency, taming_difficulty, decompiler_skill);
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(chance as f64)
        };

        if roll {
            let wild_max_hp = self.world.get::<Stats>(front).unwrap().max_hp;
            let nest = self.world.get::<NestGuardian>(front).map(|g| g.nest);
            self.world
                .entity_mut(front)
                .remove::<(Hostile, WanderAi, NestGuardian)>();
            self.world
                .entity_mut(front)
                .insert((Tamed { owner: player }, Experience::default()));
            if let Some(nest) = nest
                && let Some(mut n) = self.world.get_mut::<Nest>(nest)
            {
                n.pending_respawns.push(NEST_RESPAWN_TICKS);
            }
            self.log("ICE breached! The program now runs under your control.");
            self.award_player_xp(player, wild_max_hp as u32);
            if self.pop_front_pack_member() {
                self.clear_battle_status_effects(player, front);
                self.world.remove_resource::<BattleState>();
                self.tick();
                return;
            }
            self.log("Another rogue program from the pack engages!");
            self.resolve_post_action(player);
            self.tick();
            return;
        }

        self.log("The program's ICE holds — decompile failed!");
        self.resolve_post_action(player);
        self.tick();
    }

    pub fn battle_flee(&mut self) {
        if self.is_game_over().is_some() {
            return;
        }
        let Some(player) = self.world.get_resource::<BattleState>().map(|b| b.player) else {
            return;
        };
        let got_hit = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(0.5)
        };
        if got_hit {
            self.log("You jack out, but not before taking a parting counter-strike!");
            self.all_wild_retaliate(player);
        } else {
            self.log("You jack out safely.");
        }
        // A forced jack-out costs a little progress too — nothing drastic,
        // same mild setback as a flatline (see `death_handling_system`).
        if let Some(mut exp) = self.world.get_mut::<Experience>(player) {
            let xp_lost = progression::apply_setback_xp_penalty(&mut exp);
            if xp_lost > 0 {
                self.log(format!("Bailing out costs you {xp_lost} XP."));
            }
        }
        if let Some(front) = self.front_wild_creature() {
            self.clear_battle_status_effects(player, front);
        }
        self.world.remove_resource::<BattleState>();
        self.tick();
    }

    /// The wild creature strikes back at whoever's exposed: normally the
    /// player, but if a companion is fighting alongside them, there's a
    /// `COMPANION_RETALIATION_CHANCE` chance it eats the hit instead.
    fn wild_retaliate(&mut self, wild: Entity, player: Entity) {
        let species_id = self.world.get::<Creature>(wild).unwrap().species.clone();
        let move_count = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .unwrap()
            .moves
            .len();
        let idx = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(0..move_count)
        };
        let mv = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .unwrap()
            .moves[idx]
            .clone();

        let party = self.world.resource::<Party>().0.clone();
        let targets_companion = !party.is_empty() && {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(COMPANION_RETALIATION_CHANCE)
        };
        let target = if targets_companion {
            let idx = {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_range(0..party.len())
            };
            party[idx]
        } else {
            player
        };

        let (w_atk, t_def) = {
            let w = *self.world.get::<Stats>(wild).unwrap();
            (w.atk, self.effective_def(target))
        };
        let dmg = battle::compute_damage(w_atk, t_def, mv.power);
        self.apply_damage(target, dmg);

        if targets_companion {
            let name = self.creature_label(target);
            self.log(format!(
                "The rogue program executes {} on {} for {} damage.",
                mv.name, name, dmg
            ));
            if !self.creature_alive(target) {
                self.log(format!("{name} is knocked offline and stands down."));
                self.world
                    .resource_mut::<Party>()
                    .0
                    .retain(|&e| e != target);
            } else if let Some(effect) = &mv.effect {
                self.apply_status_effect(target, effect, &name);
            }
        } else {
            self.log(format!(
                "The rogue program executes {} for {} damage.",
                mv.name, dmg
            ));
            if self.creature_alive(target)
                && let Some(effect) = &mv.effect
            {
                self.apply_status_effect(target, effect, "You");
            }
        }
    }

    fn apply_damage(&mut self, target: Entity, dmg: i32) {
        if let Some(mut stats) = self.world.get_mut::<Stats>(target) {
            stats.hp = (stats.hp - dmg).max(0);
        }
    }

    fn creature_alive(&self, e: Entity) -> bool {
        self.world
            .get::<Stats>(e)
            .map(|s| s.hp > 0)
            .unwrap_or(false)
    }

    /// Rolls `effect.chance`; on success, overwrites `target`'s active
    /// status condition (see `StatusEffects`) and logs it. A miss is
    /// silent — the move's direct damage still landed, it just didn't also
    /// inflict its status this time.
    fn apply_status_effect(
        &mut self,
        target: Entity,
        effect: &species::MoveEffect,
        target_label: &str,
    ) {
        let applied = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(effect.chance as f64)
        };
        if !applied {
            return;
        }
        if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(target) {
            statuses.active = Some(ActiveStatus {
                kind: effect.kind,
                remaining: effect.duration,
                power: effect.power,
            });
        }
        match effect.kind {
            StatusKind::Bleed => {
                self.log(format!("{target_label} starts bleeding corrupted data!"))
            }
            StatusKind::Stun => self.log(format!("{target_label} locks up, stunned!")),
        }
    }

    /// Whether `entity` currently has an active `Stun` status. Doesn't
    /// consume it — `consume_stun` does that once a round is confirmed to
    /// skip.
    fn is_stunned(&self, entity: Entity) -> bool {
        self.world
            .get::<StatusEffects>(entity)
            .and_then(|s| s.active)
            .is_some_and(|a| a.kind == StatusKind::Stun)
    }

    /// End-of-round status upkeep for one combatant: `Bleed` deals its
    /// damage, then the active effect's remaining-rounds counter ticks
    /// down, clearing it once it hits 0.
    fn tick_status_effects(&mut self, entity: Entity, label: &str) {
        let Some(active) = self
            .world
            .get::<StatusEffects>(entity)
            .and_then(|s| s.active)
        else {
            return;
        };

        if active.kind == StatusKind::Bleed {
            self.apply_damage(entity, active.power);
            self.log(format!("{label} takes {} bleed damage.", active.power));
        }

        let remaining = active.remaining.saturating_sub(1);
        if let Some(mut statuses) = self.world.get_mut::<StatusEffects>(entity) {
            statuses.active = if remaining == 0 {
                None
            } else {
                Some(ActiveStatus {
                    remaining,
                    ..active
                })
            };
        }
        if remaining == 0 {
            match active.kind {
                StatusKind::Bleed => self.log(format!("{label}'s bleed clears.")),
                StatusKind::Stun => self.log(format!("{label} shakes off the stun.")),
            }
        }
    }

    /// End-of-round upkeep for the player's active combat buff (see
    /// `PlayerBuff`) — ticks its remaining-rounds counter down, clearing it
    /// (with a log line) once it expires.
    fn tick_player_buff(&mut self, player: Entity) {
        let Some(mut buff) = self.world.get_mut::<PlayerBuff>(player) else {
            return;
        };
        let Some(active) = buff.active else {
            return;
        };
        let remaining = active.remaining.saturating_sub(1);
        buff.active = if remaining == 0 {
            None
        } else {
            Some(ActiveBuff {
                remaining,
                ..active
            })
        };
        if remaining == 0 {
            let stat = match active.kind {
                BuffKind::Atk => "attack",
                BuffKind::Def => "defense",
            };
            self.log(format!("Your {stat} boost fades."));
        }
    }

    /// Ticks end-of-round status upkeep for every combatant that could
    /// have one: the wild creature, the player, and the active companion
    /// (if any) — `wild_retaliate`'s target selection means the companion
    /// can pick up a status even on a round where it didn't act. Also ticks
    /// the player's active combat buff, if any (see `PlayerBuff`).
    fn tick_all_status_effects(&mut self, wild: Entity, player: Entity) {
        let wild_label = self.entity_label(wild);
        self.tick_status_effects(wild, &wild_label);
        let player_label = self.entity_label(player);
        self.tick_status_effects(player, &player_label);
        self.tick_player_buff(player);
        let party = self.world.resource::<Party>().0.clone();
        for companion in party {
            let companion_label = self.creature_label(companion);
            self.tick_status_effects(companion, &companion_label);
        }
    }

    /// Clears any residual status effects from the player, `wild`, and
    /// every party member, and the player's active combat buff (see
    /// `PlayerBuff`). Status conditions are scoped to a single intrusion, so
    /// nothing should carry forward once one ends, however it ends. `wild`
    /// may already be despawned (a kill), in which case clearing it is a
    /// no-op.
    fn clear_battle_status_effects(&mut self, player: Entity, wild: Entity) {
        if let Some(mut s) = self.world.get_mut::<StatusEffects>(player) {
            s.active = None;
        }
        if let Some(mut b) = self.world.get_mut::<PlayerBuff>(player) {
            b.active = None;
        }
        if let Some(mut s) = self.world.get_mut::<StatusEffects>(wild) {
            s.active = None;
        }
        let party = self.world.resource::<Party>().0.clone();
        for companion in party {
            if let Some(mut s) = self.world.get_mut::<StatusEffects>(companion) {
                s.active = None;
            }
        }
    }

    fn find_wild_creature_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), (With<Creature>, Without<Tamed>)>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Finds a `Nest` at `(x, y)`, if any — checked in `move_player`
    /// before the ordinary blocking-structure check, so walking into a
    /// nest tile attacks it instead of just being blocked.
    fn find_nest_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Nest>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// Deals one hit of the player's `effective_atk` (against no defense
    /// — a nest has none, only a `Durability` pool) to `nest`. A nest
    /// never retaliates, unlike an ordinary wild-creature encounter — see
    /// the nests design doc for why this deliberately isn't routed
    /// through `BattleState`. Destroying it strips `NestGuardian` from
    /// every creature tethered to it (they resume ordinary wandering) and
    /// despawns the nest, which implicitly cancels anything left in its
    /// `Nest::pending_respawns`.
    fn attack_nest(&mut self, nest: Entity) {
        let player = self.player_entity();
        let label = self.entity_label(nest);
        let dmg = battle::compute_damage(self.effective_atk(player), 0, 5) as u32;
        let Some(mut durability) = self.world.get_mut::<Durability>(nest) else {
            return;
        };
        durability.hp = durability.hp.saturating_sub(dmg);
        let destroyed = durability.hp == 0;
        if destroyed {
            self.log(format!("The {label} crashes and collapses!"));
            let guardians: Vec<Entity> = {
                let mut query = self.world.query::<(Entity, &NestGuardian)>();
                query
                    .iter(&self.world)
                    .filter(|(_, g)| g.nest == nest)
                    .map(|(e, _)| e)
                    .collect()
            };
            for guardian in guardians {
                self.world.entity_mut(guardian).remove::<NestGuardian>();
            }
            self.world.despawn(nest);
        } else {
            self.log(format!(
                "You unleash a data strike into the {label} for {dmg} damage."
            ));
        }
    }

    fn find_blocking_structure_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position), With<Structure>>();
        query
            .iter(&self.world)
            .find(|(_, p)| p.x == x && p.y == y)
            .map(|(e, _)| e)
    }

    /// The Home structure's position, if one is deployed anywhere right
    /// now — the anchor `place_structure` measures the build radius from.
    fn home_position(&mut self) -> Option<Position> {
        let mut query = self.world.query::<(&Structure, &Position)>();
        query
            .iter(&self.world)
            .find(|(s, _)| s.kind == HOME_STRUCTURE_ID)
            .map(|(_, p)| *p)
    }

    /// Any deployed structure whose def sets `enables_rest` and is within
    /// its radius of `player_pos` — gates `Game::rest`.
    fn nearby_rest_structure(&mut self, player_pos: Position) -> Option<Entity> {
        let mut query = self.world.query::<(Entity, &Structure, &Position)>();
        let hits: Vec<(Entity, StructureId, Position)> = query
            .iter(&self.world)
            .map(|(e, s, p)| (e, s.kind.clone(), *p))
            .collect();
        let db = self.world.resource::<StructureDb>();
        hits.into_iter().find_map(|(entity, kind, pos)| {
            let radius = db.get(&kind)?.enables_rest.as_ref()?.radius;
            if (pos.x - player_pos.x).abs() <= radius && (pos.y - player_pos.y).abs() <= radius {
                Some(entity)
            } else {
                None
            }
        })
    }

    /// Finds a zone-portal structure (`StructureDef::zone_portal`) at
    /// `(x, y)`, if any — checked before the generic blocking-structure
    /// check in `move_player` so walking onto one breaches the zone instead
    /// of just bumping into it.
    fn find_zone_portal_at(&mut self, x: i32, y: i32) -> Option<Entity> {
        let mut query = self
            .world
            .query_filtered::<(Entity, &Position, &Structure), ()>();
        let (entity, kind) = query
            .iter(&self.world)
            .find(|(_, p, _)| p.x == x && p.y == y)
            .map(|(e, _, s)| (e, s.kind.clone()))?;
        self.world
            .resource::<StructureDb>()
            .get(&kind)
            .is_some_and(|d| d.zone_portal)
            .then_some(entity)
    }

    /// Breaches the player (and any tamed programs they own) forward into
    /// the next zone sector: wild programs and all structures in the
    /// current zone are left behind (despawned — there's no portal back
    /// down), a fresh sector is generated from a new seed, and wild
    /// programs there spawn with stats scaled by the new zone's
    /// `ZoneLevel::stat_multiplier`.
    fn enter_next_zone(&mut self) {
        let stale: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, Or<(With<Hostile>, With<Structure>, With<Nest>)>>();
            query.iter(&self.world).collect()
        };
        for e in stale {
            self.world.despawn(e);
        }
        // Any cronjob a tamed program was running just lost its target
        // structure above; drop the dangling task rather than leave it
        // pointing at a despawned entity.
        let dangling_tasks: Vec<Entity> = {
            let mut query = self.world.query_filtered::<Entity, With<Task>>();
            query.iter(&self.world).collect()
        };
        for e in dangling_tasks {
            self.world.entity_mut(e).remove::<Task>();
        }

        let new_level = {
            let mut zone = self.world.resource_mut::<ZoneLevel>();
            zone.0 += 1;
            zone.0
        };
        let new_seed = self
            .world
            .resource::<WorldMap>()
            .seed()
            .wrapping_add(0x9E37_79B9);
        let mut new_map = WorldMap::new(new_seed);
        let start = find_walkable_start(&mut new_map);
        self.world.insert_resource(new_map);
        self.world.insert_resource(ZoneSpawnPoint {
            x: start.0,
            y: start.1,
        });

        let travelers: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, Or<(With<Player>, With<Tamed>)>>();
            query.iter(&self.world).collect()
        };
        for e in travelers {
            if let Some(mut pos) = self.world.get_mut::<Position>(e) {
                pos.x = start.0;
                pos.y = start.1;
            }
        }

        self.log(format!(
            "You breach the portal and materialize in a level {new_level} sector. Hostile signal strength has spiked."
        ));
        self.spawn_initial_creatures(14);
    }

    /// Spawns a wild creature of `species_id` at `(x, y)`, returning its
    /// `Entity` — `None` only if `species_id` isn't in `SpeciesDb` (every
    /// real call site passes an id it already validated against
    /// `SpeciesDb`, so this is a defensive no-op path, not an expected
    /// outcome). `spawn_nest_guardian` uses the returned entity to attach
    /// `NestGuardian`.
    fn spawn_wild_creature(&mut self, species_id: &str, x: i32, y: i32) -> Option<Entity> {
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(species_id)
            .cloned()?;
        let zone_level = self.world.resource::<ZoneLevel>();
        let mult = zone_level.stat_multiplier() as f32;
        let zone = zone_level.0;
        let dist_mult = self.distance_stat_multiplier(x, y);
        let potential = self.roll_potential();
        let scale = |base: i32, roll: f32| ((base as f32) * mult * dist_mult * roll).round() as i32;
        Some(
            self.world
                .spawn((
                    Creature {
                        species: species.id.clone(),
                    },
                    Position { x, y },
                    Glyph {
                        ch: species.glyph,
                        color: species.color,
                    },
                    Stats {
                        hp: scale(species.base_hp, potential.hp_roll),
                        max_hp: scale(species.base_hp, potential.hp_roll),
                        atk: scale(species.base_atk, potential.atk_roll),
                        def: scale(species.base_def, potential.def_roll),
                    },
                    potential,
                    Hostile,
                    WanderAi::default(),
                    ZonePortal(zone),
                    StatusEffects::default(),
                ))
                .id(),
        )
    }

    /// Spawns a `Nest` for `species_id` at `(x, y)`, plus an initial
    /// `NEST_GUARDIAN_MIN..=NEST_GUARDIAN_MAX` guardians clustered within
    /// `NEST_TETHER_RADIUS` of it.
    fn spawn_nest(&mut self, species_id: &str, x: i32, y: i32) {
        let Some(species) = self.world.resource::<SpeciesDb>().get(species_id).cloned() else {
            return;
        };
        let nest = self
            .world
            .spawn((
                Nest {
                    species: species.id.clone(),
                    pending_respawns: Vec::new(),
                },
                Position { x, y },
                Glyph {
                    ch: 'N',
                    color: species.color,
                },
                Durability {
                    hp: NEST_DURABILITY,
                    max_hp: NEST_DURABILITY,
                },
            ))
            .id();
        let guardian_count = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(NEST_GUARDIAN_MIN..=NEST_GUARDIAN_MAX)
        };
        for _ in 0..guardian_count {
            self.spawn_nest_guardian(nest, species_id, x, y);
        }
    }

    /// Spawns one `species_id` wild creature tethered to `nest`, at a
    /// random offset within `NEST_TETHER_RADIUS` of `(nest_x, nest_y)` —
    /// used both for a nest's initial guardians (`spawn_nest`) and for
    /// respawns (`nest_respawn_tick`). Walkability isn't rechecked for the
    /// offset tile, matching the existing looseness
    /// `try_spawn_habitat_creature` already has for pack members.
    fn spawn_nest_guardian(&mut self, nest: Entity, species_id: &str, nest_x: i32, nest_y: i32) {
        let (gx, gy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (
                nest_x + rng.0.random_range(-NEST_TETHER_RADIUS..=NEST_TETHER_RADIUS),
                nest_y + rng.0.random_range(-NEST_TETHER_RADIUS..=NEST_TETHER_RADIUS),
            )
        };
        if let Some(guardian) = self.spawn_wild_creature(species_id, gx, gy) {
            self.world
                .entity_mut(guardian)
                .insert(NestGuardian { nest });
        }
    }

    /// Stat multiplier for a wild spawn at `(x, y)`, from how far it is
    /// (Chebyshev distance — matching 8-directional movement, so it's
    /// "how many moves away") from `ZoneSpawnPoint`: `1.0` right at spawn,
    /// growing by `DISTANCE_STAT_STEP_BONUS` every
    /// `DISTANCE_STAT_STEP_TILES`, capped at `MAX_DISTANCE_STAT_MULTIPLIER`.
    /// Applied multiplicatively with `ZoneLevel::stat_multiplier` in
    /// `spawn_wild_creature` — venturing away from where you breached in
    /// is its own escalating risk, independent of zone depth.
    fn distance_stat_multiplier(&self, x: i32, y: i32) -> f32 {
        let spawn = self.world.resource::<ZoneSpawnPoint>();
        let dist = (x - spawn.x).abs().max((y - spawn.y).abs());
        let mult = 1.0 + (dist / DISTANCE_STAT_STEP_TILES) as f32 * DISTANCE_STAT_STEP_BONUS;
        mult.min(MAX_DISTANCE_STAT_MULTIPLIER)
    }

    /// Rolls a fresh `Potential` for a newly created creature — see
    /// `spawn_wild_creature`/`fuse_companions`. Each of the four fields is
    /// independently uniform in `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`
    /// — the "same species, different stats" mechanic.
    fn roll_potential(&mut self) -> Potential {
        let mut rng = self.world.resource_mut::<GameRng>();
        Potential {
            hp_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            atk_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            def_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
            growth_roll: rng
                .0
                .random_range(MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL),
        }
    }

    /// Maximum wild pack size at `(x, y)`: capped at `zone + 1` (zone 1 →
    /// 2, zone 2 → 3, ...), reached gradually the farther `(x, y)` is from
    /// `ZoneSpawnPoint` — solo right at spawn, then one more potential
    /// packmate every `PACK_SIZE_STEP_TILES`. Used both to pick how many
    /// creatures a group spawn roll places together
    /// (`try_spawn_habitat_creature`) and as a hard ceiling on how many
    /// can ever end up in one fight (`gather_pack`).
    fn max_pack_size(&self, x: i32, y: i32) -> u32 {
        let zone = self.world.resource::<ZoneLevel>().0;
        let cap = zone + 1;
        let spawn = self.world.resource::<ZoneSpawnPoint>();
        let dist = (x - spawn.x).abs().max((y - spawn.y).abs());
        let grown = 1 + (dist / PACK_SIZE_STEP_TILES) as u32;
        grown.min(cap)
    }

    /// Spawns `count` wild creatures near the player, retrying with a fresh
    /// random offset whenever a roll whiffs (an unwalkable tile, or a biome
    /// with no matching species) rather than giving up on that slot — a
    /// freshly generated zone's terrain noise can otherwise leave large
    /// unwalkable or habitat-sparse patches right around the player's
    /// start point (see `find_walkable_start`, which always searches out
    /// from world origin), and a blind one-attempt-per-slot approach would
    /// leave the zone nearly empty whenever that happens. Bounded to
    /// `count * 20` attempts so a pathologically bad pocket can't loop
    /// forever instead of just spawning fewer than `count`.
    fn spawn_initial_creatures(&mut self, count: usize) {
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        let mut spawned = 0;
        let mut attempts = 0;
        while spawned < count && attempts < count * 20 {
            attempts += 1;
            let (dx, dy) = {
                let mut rng = self.world.resource_mut::<GameRng>();
                (rng.0.random_range(-15..=15), rng.0.random_range(-15..=15))
            };
            if self.try_spawn_habitat_creature(player_pos.x + dx, player_pos.y + dy) {
                spawned += 1;
            }
        }
    }

    fn maybe_spawn_wild_creature(&mut self) {
        let player_pos = *self.world.get::<Position>(self.player_entity()).unwrap();
        // Roll first: culling is wasted work if nothing was going to spawn.
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(0.05)
        };
        if !roll {
            return;
        }
        // At `WILD_CREATURE_CAP`, make room by despawning the `Hostile`
        // farthest (Chebyshev, matching 8-directional movement) from where
        // the player is now — the one least likely to ever be encountered
        // again. `NestGuardian`s are eligible like any other hostile; a cull
        // is a plain despawn, so it deliberately doesn't feed the nest's
        // `pending_respawns` the way an actual defeat does. Guardian counts
        // are best-effort once a nest is far behind the player.
        let hostiles: Vec<(Entity, i32)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), With<Hostile>>();
            query
                .iter(&self.world)
                .map(|(e, p)| {
                    (
                        e,
                        (p.x - player_pos.x).abs().max((p.y - player_pos.y).abs()),
                    )
                })
                .collect()
        };
        if hostiles.len() >= WILD_CREATURE_CAP
            && let Some(&(farthest, _)) = hostiles.iter().max_by_key(|(_, dist)| *dist)
        {
            self.world.despawn(farthest);
        }
        let (dx, dy) = {
            let mut rng = self.world.resource_mut::<GameRng>();
            (rng.0.random_range(-12..=12), rng.0.random_range(-12..=12))
        };
        self.try_spawn_habitat_creature(player_pos.x + dx, player_pos.y + dy);
    }

    /// Slow passive healing for damaged structures — every
    /// `STRUCTURE_REGEN_INTERVAL` ticks, everything below max `Durability`
    /// recovers `STRUCTURE_REGEN_AMOUNT`.
    fn structure_regen(&mut self) {
        let tick = self.world.resource::<GameClock>().tick;
        if !tick.is_multiple_of(STRUCTURE_REGEN_INTERVAL) {
            return;
        }
        let mut query = self.world.query::<&mut Durability>();
        for mut durability in query.iter_mut(&mut self.world) {
            durability.hp = (durability.hp + STRUCTURE_REGEN_AMOUNT).min(durability.max_hp);
        }
    }

    /// Advances every `Nest`'s `pending_respawns` countdown by one tick,
    /// spawning a replacement guardian for each entry that reaches 0 (a
    /// nest can have more than one entry reach 0 on the same tick, e.g.
    /// two guardians killed together, so this spawns once per ready
    /// entry, not once per nest). Called directly from `tick` —
    /// not registered on `self.schedule` — because it needs
    /// `spawn_nest_guardian`, a `Game` method unreachable from a bevy
    /// system function.
    fn nest_respawn_tick(&mut self) {
        let ready: Vec<(Entity, SpeciesId, Position, usize)> = {
            let mut query = self.world.query::<(Entity, &mut Nest, &Position)>();
            query
                .iter_mut(&mut self.world)
                .filter_map(|(entity, mut nest, pos)| {
                    for slot in nest.pending_respawns.iter_mut() {
                        *slot = slot.saturating_sub(1);
                    }
                    let ready_count = nest.pending_respawns.iter().filter(|&&t| t == 0).count();
                    if ready_count == 0 {
                        return None;
                    }
                    nest.pending_respawns.retain(|&t| t != 0);
                    Some((entity, nest.species.clone(), *pos, ready_count))
                })
                .collect()
        };
        for (nest, species, pos, count) in ready {
            for _ in 0..count {
                self.spawn_nest_guardian(nest, &species, pos.x, pos.y);
            }
        }
    }

    /// Rolls `RAID_CHANCE_PER_TICK`; on success, picks one deployed
    /// structure at random and either damages it directly (undefended) or
    /// has its assigned cronjob worker, if any, fight the raid off —
    /// reducing the structure's damage by the worker's Defense, at the
    /// cost of `RAID_DEFENDER_DAMAGE` to the worker. A worker knocked to 0
    /// HP stands down from the cronjob (like a knocked-out companion, not
    /// destroyed — `rest` heals it back up along with every other tamed
    /// program you own). A structure whose `Durability` reaches 0 is
    /// destroyed and any cronjob assignment on it is dropped.
    /// Total raid-damage reduction contributed by every deployed structure
    /// with `StructureDef::raid_defense` set (e.g. a Shield) — a base-wide
    /// network, not tied to any one structure. Destroying one of these
    /// structures in a raid naturally shrinks this, since it's recomputed
    /// fresh from whatever's still standing.
    /// Drains every `VisualEffect` queued since the last call — the visual
    /// counterpart to `App::take_sounds`. A frontend without effects can
    /// drop the result, but must still call it so the queue doesn't sit at
    /// its cap.
    pub fn take_effects(&mut self) -> Vec<VisualEffect> {
        self.world.resource_mut::<EffectQueue>().take()
    }

    /// Queues `kind` at `structure`'s tile, if it has one. Raid targets are
    /// selected by `With<Durability>`, which doesn't imply `Position` —
    /// a flash on the wrong tile would be worse than none, so a positionless
    /// entity queues nothing.
    fn push_effect(&mut self, structure: Entity, kind: EffectKind) {
        let Some(pos) = self.world.get::<Position>(structure).map(|p| (p.x, p.y)) else {
            return;
        };
        self.world.resource_mut::<EffectQueue>().push(pos, kind);
    }

    /// Whether any deployed structure contributes raid defense — the seam
    /// frontends use to show the shield network as active without reaching
    /// into `StructureDb` themselves.
    pub fn raid_defense_active(&self) -> bool {
        self.total_raid_defense() > 0
    }

    fn total_raid_defense(&self) -> u32 {
        let structure_db = self.world.resource::<StructureDb>();
        self.world
            .iter_entities()
            .filter_map(|e| e.get::<Structure>())
            .filter_map(|s| structure_db.get(&s.kind))
            .map(|def| def.raid_defense)
            .sum()
    }

    fn raid_check(&mut self) {
        let roll = {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(RAID_CHANCE_PER_TICK)
        };
        if !roll {
            return;
        }
        let targets: Vec<Entity> = {
            let mut query = self
                .world
                .query_filtered::<Entity, (With<Durability>, Without<Nest>)>();
            query.iter(&self.world).collect()
        };
        if targets.is_empty() {
            return;
        }
        let target = {
            let mut rng = self.world.resource_mut::<GameRng>();
            let idx = rng.0.random_range(0..targets.len());
            targets[idx]
        };
        let target_label = self.entity_label(target);
        let raid_damage = RAID_DAMAGE.saturating_sub(self.total_raid_defense());

        let defender = {
            let mut query = self.world.query::<(Entity, &Task)>();
            query
                .iter(&self.world)
                .find(|(_, t)| t.target == target)
                .map(|(e, _)| e)
        };

        let Some(worker) = defender else {
            if raid_damage > 0 {
                self.damage_structure(target, raid_damage, &target_label);
            } else {
                self.push_effect(target, EffectKind::Deflected);
                self.log(format!(
                    "Your shield network fends off a raid on {target_label} without a scratch!"
                ));
            }
            return;
        };

        let worker_def = self.world.get::<Stats>(worker).map(|s| s.def).unwrap_or(0);
        let mitigated = raid_damage.saturating_sub(worker_def.max(0) as u32);
        let worker_label = self.creature_label(worker);
        if mitigated > 0 {
            self.damage_structure(target, mitigated, &target_label);
        } else {
            self.push_effect(target, EffectKind::Deflected);
            self.log(format!(
                "{worker_label} fends off a raid on {target_label} without a scratch!"
            ));
        }
        self.apply_damage(worker, RAID_DEFENDER_DAMAGE);
        if !self.creature_alive(worker) {
            self.log(format!(
                "{worker_label} is knocked offline defending {target_label} and stands down from its cronjob."
            ));
            self.world.entity_mut(worker).remove::<Task>();
        }
    }

    /// Applies `dmg` to `structure`'s `Durability`, destroying (despawning)
    /// it and clearing any cronjob assignment pointing at it if that
    /// brings it to 0.
    fn damage_structure(&mut self, structure: Entity, dmg: u32, label: &str) {
        let Some(mut durability) = self.world.get_mut::<Durability>(structure) else {
            return;
        };
        durability.hp = durability.hp.saturating_sub(dmg);
        let destroyed = durability.hp == 0;
        // Queued before the despawn below, which takes the `Position` the
        // effect needs with it.
        self.push_effect(
            structure,
            if destroyed {
                EffectKind::Destroyed
            } else {
                EffectKind::Hit
            },
        );
        if destroyed {
            self.log_kind(
                MessageKind::Raid,
                format!("{label} is destroyed in a raid!"),
            );
            let workers: Vec<Entity> = {
                let mut query = self.world.query::<(Entity, &Task)>();
                query
                    .iter(&self.world)
                    .filter(|(_, t)| t.target == structure)
                    .map(|(e, _)| e)
                    .collect()
            };
            for w in workers {
                self.world.entity_mut(w).remove::<Task>();
            }
            self.world.despawn(structure);
        } else {
            self.log_kind(
                MessageKind::Raid,
                format!("{label} takes {dmg} raid damage!"),
            );
        }
    }

    /// Attempts to spawn one habitat-appropriate wild creature (or, away
    /// from the zone's spawn point, a small pack of the same species — see
    /// `max_pack_size`) at `(x, y)`, returning whether it actually spawned
    /// anything — `false` on an unwalkable tile or a biome with no
    /// matching species, so callers (see `spawn_initial_creatures`) can
    /// retry elsewhere instead of silently losing that spawn slot.
    fn try_spawn_habitat_creature(&mut self, x: i32, y: i32) -> bool {
        let tile = self.world.resource_mut::<WorldMap>().tile(x, y);
        if !tile.walkable {
            return false;
        }
        let species_db = self.world.resource::<SpeciesDb>();
        let candidates: Vec<String> = species_db
            .habitat_matches(tile.biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        let boss_candidates: Vec<String> = species_db
            .boss_habitat_matches(tile.biome)
            .into_iter()
            .map(|s| s.id.clone())
            .collect();
        if candidates.is_empty() && boss_candidates.is_empty() {
            return false;
        }
        // A boss takes the tile's one spawn slot instead of an ordinary
        // habitat creature, but only rarely, and only where one is defined
        // for this biome at all.
        let spawn_boss = !boss_candidates.is_empty() && {
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_bool(BOSS_SPAWN_CHANCE)
        };
        let pool = if spawn_boss || candidates.is_empty() {
            &boss_candidates
        } else {
            &candidates
        };
        let pick = {
            let mut rng = self.world.resource_mut::<GameRng>();
            let idx = rng.0.random_range(0..pool.len());
            pool[idx].clone()
        };

        // A nest takes the tile's spawn slot instead of an ordinary pack,
        // same "rare special outcome" shape as the boss roll above — but
        // only ever considered for the non-boss pick, and only for a
        // species that opted in via `SpeciesDef::can_nest`. The RNG draw
        // only happens when `can_nest` is true, so this never shifts the
        // RNG sequence for the (overwhelmingly common) non-nesting case.
        if !spawn_boss {
            let can_nest = self
                .world
                .resource::<SpeciesDb>()
                .get(&pick)
                .is_some_and(|s| s.can_nest);
            let spawn_nest_roll = can_nest && {
                let mut rng = self.world.resource_mut::<GameRng>();
                rng.0.random_bool(NEST_SPAWN_CHANCE)
            };
            if spawn_nest_roll {
                self.spawn_nest(&pick, x, y);
                return true;
            }
        }

        // Bosses always spawn alone — packs are an ordinary-encounter
        // mechanic, not something to stack onto an already-tough boss
        // fight.
        let group_size = if spawn_boss {
            1
        } else {
            let max_pack = self.max_pack_size(x, y);
            let mut rng = self.world.resource_mut::<GameRng>();
            rng.0.random_range(1..=max_pack)
        };
        for i in 0..group_size {
            // The first member anchors the roll's own tile; the rest
            // cluster loosely around it (walkability isn't rechecked for
            // these — same looseness the rest of spawning already has).
            let (gx, gy) = if i == 0 {
                (x, y)
            } else {
                let mut rng = self.world.resource_mut::<GameRng>();
                (
                    x + rng.0.random_range(-PACK_GATHER_RADIUS..=PACK_GATHER_RADIUS),
                    y + rng.0.random_range(-PACK_GATHER_RADIUS..=PACK_GATHER_RADIUS),
                )
            };
            self.spawn_wild_creature(&pick, gx, gy);
        }
        true
    }

    pub fn player_status(&self) -> PlayerStatus {
        let inventory_capacity = self.inventory_capacity();
        let player = self.player_entity();
        let stats = self.world.get::<Stats>(player).unwrap();
        let needs = self.world.get::<Needs>(player).unwrap();
        let pos = self.world.get::<Position>(player).unwrap();
        let inv = self.world.get::<Inventory>(player).unwrap();
        let exp = self.world.get::<Experience>(player).unwrap();
        let decompiler = self
            .world
            .get::<Decompiler>(player)
            .map(|d| d.skill)
            .unwrap_or(0);
        let equipment = self
            .world
            .get::<Equipment>(player)
            .cloned()
            .unwrap_or_default();
        let perks = self.world.get::<Perks>(player);
        let atk = self.effective_atk(player);
        let def = self.effective_def(player);
        let db = self.world.resource::<ItemDb>();
        PlayerStatus {
            position: (pos.x, pos.y),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk,
            def,
            power: stats.max_hp + atk + def,
            decompiler,
            hunger: needs.hunger,
            fatigue: needs.fatigue,
            inventory: inv.items.clone(),
            inventory_used: inv.cargo_used(db),
            inventory_capacity,
            level: exp.level,
            xp: exp.xp,
            xp_to_next: exp.xp_to_next,
            weapon: equipment.weapon,
            armor: equipment.armor,
            module: equipment.module,
            companions: self.party_info(),
            zone: self.world.resource::<ZoneLevel>().0,
            perk_points: perks.map(|p| p.points).unwrap_or(0),
            unlocked_perks: perks.map(|p| p.unlocked.clone()).unwrap_or_default(),
        }
    }

    /// A creature's own display name: the player's `CustomName` if they set
    /// one (currently only via `Game::fuse_companions`), else its species
    /// name (falling back to the raw species id if the species definition
    /// is somehow missing). `None` if `entity` isn't a `Creature` at all.
    fn creature_name(&self, entity: Entity) -> Option<String> {
        let c = self.world.get::<Creature>(entity)?;
        if let Some(custom) = self.world.get::<CustomName>(entity) {
            return Some(custom.0.clone());
        }
        Some(
            self.world
                .resource::<SpeciesDb>()
                .get(&c.species)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| c.species.clone()),
        )
    }

    /// `creature_name`, zone-tagged, falling back to a generic label if
    /// `entity` isn't a `Creature`.
    fn creature_label(&self, entity: Entity) -> String {
        match self.creature_name(entity) {
            Some(name) => self.zone_tagged_name(entity, name),
            None => "Program".to_string(),
        }
    }

    /// Appends a creature's `ZonePortal` to its species name for display
    /// (e.g. "Scrapper 2"), so a deeper-zone catch reads differently from a
    /// shallow one at a glance. Falls back to the bare name if the entity
    /// has no `ZonePortal` — expected for creatures hand-spawned outside the
    /// normal `spawn_wild_creature` path (e.g. in tests).
    fn zone_tagged_name(&self, entity: Entity, name: String) -> String {
        match self.world.get::<ZonePortal>(entity) {
            Some(zone) => format!("{name} {}", zone.0),
            None => name,
        }
    }

    /// Whether `entity` is a creature of a boss species (`SpeciesDef::is_boss`).
    /// `false` for anything that isn't a creature, or whose species failed
    /// to resolve.
    fn is_boss_creature(&self, entity: Entity) -> bool {
        self.world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .is_some_and(|s| s.is_boss)
    }

    fn companion_info(&self, entity: Entity) -> Option<CompanionInfo> {
        let stats = self.world.get::<Stats>(entity)?;
        Some(CompanionInfo {
            entity,
            name: self.creature_label(entity),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            def: stats.def,
            power: stats.power(),
            status: self.status_label(entity),
            ability: self.companion_ability_label(entity),
        })
    }

    /// Terse label for what commanding `entity` in battle would do right
    /// now: its species' own `special_ability` if it has one (see
    /// `SpecialAbility::short_name`), or "Rally Team" for the generic
    /// Attack Rally every companion falls back to otherwise.
    fn companion_ability_label(&self, entity: Entity) -> String {
        let ability = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .and_then(|s| s.special_ability.clone());
        match ability {
            Some(ability) => ability.short_name().to_string(),
            None => "Rally Team".to_string(),
        }
    }

    /// How many fusions deep `entity`'s lineage is (see
    /// `components::FusionCount`) — 0 for anything caught or spawned
    /// normally, up to `MAX_FUSIONS`, at which point it can't be fused
    /// again.
    pub fn fusion_count(&self, entity: Entity) -> u32 {
        self.world
            .get::<FusionCount>(entity)
            .map(|f| f.0)
            .unwrap_or(0)
    }

    /// Display string for `entity`'s rolled `Potential`, e.g.
    /// "Excellent (94%)" — `None` if it has no `Potential` component (an
    /// old save predating it, or a non-creature entity).
    fn potential_quality_label(&self, entity: Entity) -> Option<String> {
        let potential = self.world.get::<Potential>(entity)?;
        Some(format!(
            "{} ({}%)",
            potential.quality_label(),
            potential.quality_percent()
        ))
    }

    /// Snapshot of every current party member (see `resources::Party`), in
    /// party-slot order.
    fn party_info(&self) -> Vec<CompanionInfo> {
        self.world
            .resource::<Party>()
            .0
            .iter()
            .filter_map(|&e| self.companion_info(e))
            .collect()
    }

    /// Full stats for every tamed program the player owns, anywhere on the
    /// map — unlike `view_entities`, not limited to what's currently in
    /// view. Lets you check on a cronjob worker's HP/level without walking
    /// over to it.
    pub fn owned_pets(&mut self) -> Vec<PetInfo> {
        let player = self.player_entity();
        let party = self.world.resource::<Party>().0.clone();
        let owned: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &Tamed)>();
            query
                .iter(&self.world)
                .filter(|(_, t)| t.owner == player)
                .map(|(e, _)| e)
                .collect()
        };
        owned
            .into_iter()
            .filter_map(|entity| {
                let stats = *self.world.get::<Stats>(entity)?;
                let level = self
                    .world
                    .get::<Experience>(entity)
                    .map(|e| e.level)
                    .unwrap_or(1);
                let job_structure = self
                    .world
                    .get::<Task>(entity)
                    .map(|t| t.target)
                    .map(|target| self.entity_label(target));
                Some(PetInfo {
                    entity,
                    name: self.creature_label(entity),
                    level,
                    hp: stats.hp,
                    max_hp: stats.max_hp,
                    atk: stats.atk,
                    def: stats.def,
                    power: stats.power(),
                    is_companion: party.contains(&entity),
                    job_structure,
                    quality: self.potential_quality_label(entity),
                    fusions: self.fusion_count(entity),
                })
            })
            .collect()
    }

    /// Display string for `entity`'s current active status condition, if
    /// any — e.g. "Bleeding (2)" or "Stunned (1)", the number being battle
    /// rounds remaining. `None` if it has no active condition.
    fn status_label(&self, entity: Entity) -> Option<String> {
        let active = self.world.get::<StatusEffects>(entity)?.active?;
        Some(match active.kind {
            StatusKind::Bleed => format!("Bleeding ({})", active.remaining),
            StatusKind::Stun => format!("Stunned ({})", active.remaining),
        })
    }

    /// Adds `creature` (a tamed program you own) to your active battle
    /// party (see `resources::Party`), up to `MAX_PARTY_SIZE` at once.
    /// Clears an in-progress cronjob task on it first — a program can only
    /// be doing one job (working a structure, or fighting beside you) at a
    /// time.
    pub fn add_companion(&mut self, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != player {
            return Err("You don't control that program.".into());
        }
        if self.world.resource::<Party>().0.contains(&creature) {
            return Err("That program is already in your party.".into());
        }
        if self.world.resource::<Party>().0.len() >= MAX_PARTY_SIZE {
            return Err(format!(
                "Your party is full ({MAX_PARTY_SIZE} max) — stand one down first."
            ));
        }
        self.world.entity_mut(creature).remove::<Task>();
        self.world.resource_mut::<Party>().0.push(creature);
        let name = self.creature_label(creature);
        self.log(format!("{name} falls in alongside you."));
        Ok(())
    }

    /// Stands `creature` down from the active party, if it's a member — it
    /// remains a tamed program, just no longer commandable in battle. A
    /// no-op (no log) if it wasn't in the party to begin with.
    pub fn remove_companion(&mut self, creature: Entity) {
        let was_present = {
            let mut party = self.world.resource_mut::<Party>();
            let before = party.0.len();
            party.0.retain(|&e| e != creature);
            party.0.len() != before
        };
        if was_present {
            let name = self.creature_label(creature);
            self.log(format!("{name} falls back from active duty."));
        }
    }

    /// Fuses two of the player's tamed programs (`a` and `b`, any species,
    /// party members or not) into one new tamed program, consuming both.
    /// The result keeps the species (and so the moves/work aptitude) of
    /// whichever input is the higher level — ties favor `a` — at that same
    /// level, with each stat computed as `higher + lower / 2` so a fusion
    /// is always stronger than either input alone without simply summing
    /// them (which would make repeated fusion runaway). A resource sink for
    /// duplicate catches: there's no separate item cost, since losing two
    /// programs to gain one is the cost.
    ///
    /// Fusion depth is capped: neither input may already be `MAX_FUSIONS`
    /// deep (see `components::FusionCount`), and the result is one deeper
    /// than its deepest input.
    /// `custom_name`, if given, is trimmed and truncated to
    /// `MAX_CUSTOM_NAME_LEN` characters and becomes the fused program's
    /// display name everywhere (see `CustomName`) instead of its species
    /// name. Blank (or all-whitespace) is treated the same as `None`.
    pub fn fuse_companions(
        &mut self,
        a: Entity,
        b: Entity,
        custom_name: Option<String>,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if a == b {
            return Err("Pick two different programs to fuse.".into());
        }
        let player = self.player_entity();
        for e in [a, b] {
            let owner = self
                .world
                .get::<Tamed>(e)
                .ok_or_else(|| "Both programs must be compiled under your control.".to_string())?
                .owner;
            if owner != player {
                return Err("You don't control both programs.".into());
            }
        }
        for e in [a, b] {
            if self.fusion_count(e) >= MAX_FUSIONS {
                let name = self.creature_label(e);
                return Err(format!(
                    "{name} has already been fused {MAX_FUSIONS} times — it can't be fused again."
                ));
            }
        }
        let fused_depth = self.fusion_count(a).max(self.fusion_count(b)) + 1;
        let (species_a, exp_a, stats_a, potential_a) = (
            self.world.get::<Creature>(a).unwrap().species.clone(),
            *self.world.get::<Experience>(a).unwrap(),
            *self.world.get::<Stats>(a).unwrap(),
            self.world
                .get::<Potential>(a)
                .copied()
                .unwrap_or(Potential::NEUTRAL),
        );
        let (species_b, exp_b, stats_b, potential_b) = (
            self.world.get::<Creature>(b).unwrap().species.clone(),
            *self.world.get::<Experience>(b).unwrap(),
            *self.world.get::<Stats>(b).unwrap(),
            self.world
                .get::<Potential>(b)
                .copied()
                .unwrap_or(Potential::NEUTRAL),
        );
        let (species_id, level) = if exp_a.level >= exp_b.level {
            (species_a, exp_a.level)
        } else {
            (species_b, exp_b.level)
        };
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .cloned()
            .ok_or_else(|| "That species is no longer available.".to_string())?;

        fn fuse_stat(x: i32, y: i32) -> i32 {
            x.max(y) + x.min(y) / 2
        }
        let fused_hp = fuse_stat(stats_a.max_hp, stats_b.max_hp);
        let fused_atk = fuse_stat(stats_a.atk, stats_b.atk);
        let fused_def = fuse_stat(stats_a.def, stats_b.def);
        let fused_potential = Potential::averaged(potential_a, potential_b);

        let name_a = self.creature_label(a);
        let name_b = self.creature_label(b);
        self.world
            .resource_mut::<Party>()
            .0
            .retain(|&e| e != a && e != b);
        self.world.despawn(a);
        self.world.despawn(b);

        let final_name: Option<String> = custom_name.and_then(|n| {
            let trimmed = n.trim();
            (!trimmed.is_empty()).then(|| {
                trimmed
                    .chars()
                    .take(MAX_CUSTOM_NAME_LEN)
                    .collect::<String>()
            })
        });

        let player_pos = *self.world.get::<Position>(player).unwrap();
        let mut fused = self.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Glyph {
                ch: species.glyph,
                color: species.color,
            },
            Stats {
                hp: fused_hp,
                max_hp: fused_hp,
                atk: fused_atk,
                def: fused_def,
            },
            fused_potential,
            Tamed { owner: player },
            Experience {
                level,
                xp: 0,
                xp_to_next: progression::xp_for_level(level),
            },
            ZonePortal(1),
            StatusEffects::default(),
            FusionCount(fused_depth),
        ));
        if let Some(name) = &final_name {
            fused.insert(CustomName(name.clone()));
        }
        self.log(match &final_name {
            Some(name) => format!(
                "You fuse {name_a} and {name_b} into {name}, a new {}.",
                species.name
            ),
            None => format!(
                "You fuse {name_a} and {name_b} into a new {}.",
                species.name
            ),
        });
        Ok(())
    }

    /// Where the player materialized on breaching into the current zone —
    /// see `resources::ZoneSpawnPoint`. Both frontends mark this on the map
    /// so a player can navigate back toward the (comparatively) safer
    /// ground near it, per `distance_stat_multiplier`.
    pub fn zone_spawn_point(&self) -> (i32, i32) {
        let p = self.world.resource::<ZoneSpawnPoint>();
        (p.x, p.y)
    }

    pub fn view_tiles(&mut self, half_w: i32, half_h: i32) -> Vec<Vec<Tile>> {
        let center = *self.world.get::<Position>(self.player_entity()).unwrap();
        let mut world_map = self.world.resource_mut::<WorldMap>();
        let mut rows = Vec::new();
        for ty in -half_h..=half_h {
            let mut row = Vec::new();
            for tx in -half_w..=half_w {
                row.push(world_map.tile(center.x + tx, center.y + ty));
            }
            rows.push(row);
        }
        rows
    }

    /// Finds the nearest creature generally toward (dx, dy) from the
    /// player — the read-only "look in a direction" counterpart to
    /// `move_player`. `(dx, dy)` is one of the four cardinal unit vectors.
    /// A creature counts as "that way" if it's within the 90° cone
    /// centered on the chosen direction (i.e. leans at least as much
    /// toward that axis as away from it) and within `max_range` tiles —
    /// a strict single-tile-wide ray would almost never line up with a
    /// wandering creature's exact row/column, so this is deliberately
    /// forgiving. Ignores terrain walkability (this never moves anything,
    /// just looks), and only ever matches creatures, not structures or
    /// the player.
    pub fn find_creature_in_direction(
        &mut self,
        dx: i32,
        dy: i32,
        max_range: i32,
    ) -> Option<Entity> {
        let player = self.player_entity();
        let start = *self.world.get::<Position>(player).unwrap();
        let mut query = self.world.query::<(Entity, &Position, &Creature)>();
        query
            .iter(&self.world)
            .filter_map(|(entity, pos, _)| {
                let (ddx, ddy) = (pos.x - start.x, pos.y - start.y);
                let in_cone = if dx != 0 {
                    ddx.signum() == dx && ddx.abs() >= ddy.abs()
                } else {
                    ddy.signum() == dy && ddy.abs() >= ddx.abs()
                };
                let dist = ddx.abs().max(ddy.abs());
                (in_cone && dist >= 1 && dist <= max_range).then_some((entity, dist))
            })
            .min_by_key(|(_, dist)| *dist)
            .map(|(entity, _)| entity)
    }

    /// Display label for any entity — species name for a creature,
    /// structure name for a structure, `"You"` otherwise. Shared by
    /// `view_entities` for both an entity's own label and cross-references
    /// (a worker's assigned structure, a structure's assigned worker).
    fn entity_label(&self, entity: Entity) -> String {
        if let Some(name) = self.creature_name(entity) {
            self.zone_tagged_name(entity, name)
        } else if let Some(s) = self.world.get::<Structure>(entity) {
            self.world
                .resource::<StructureDb>()
                .get(&s.kind)
                .map(|d| d.name.clone())
                .unwrap_or_else(|| s.kind.clone())
        } else if let Some(nest) = self.world.get::<Nest>(entity) {
            let species_name = self
                .world
                .resource::<SpeciesDb>()
                .get(&nest.species)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| nest.species.clone());
            format!("{species_name} Nest")
        } else {
            "You".to_string()
        }
    }

    pub fn view_entities(&mut self, half_w: i32, half_h: i32) -> Vec<EntityView> {
        let center = *self.world.get::<Position>(self.player_entity()).unwrap();
        let mut query = self.world.query::<(Entity, &Position, &Glyph)>();
        let hits: Vec<(Entity, Position, Glyph)> = query
            .iter(&self.world)
            .filter(|(_, p, _)| {
                (p.x - center.x).abs() <= half_w && (p.y - center.y).abs() <= half_h
            })
            .map(|(e, p, g)| (e, *p, *g))
            .collect();

        let worker_by_structure: HashMap<Entity, Entity> = {
            let mut tasks = self.world.query::<(Entity, &Task)>();
            tasks
                .iter(&self.world)
                .map(|(worker, task)| (task.target, worker))
                .collect()
        };

        let player_power = self
            .world
            .get::<Stats>(self.player_entity())
            .unwrap()
            .power();

        hits.into_iter()
            .map(|(entity, pos, glyph)| {
                let is_player = self.world.get::<Player>(entity).is_some();
                let is_tamed = self.world.get::<Tamed>(entity).is_some();
                let is_companion = self.world.resource::<Party>().0.contains(&entity);
                let is_hostile = self.world.get::<Hostile>(entity).is_some();
                let is_structure = self.world.get::<Structure>(entity).is_some();
                let is_home = self
                    .world
                    .get::<Structure>(entity)
                    .is_some_and(|s| s.kind == HOME_STRUCTURE_ID);
                let is_boss = self.is_boss_creature(entity);
                let can_work = self.world.get::<ResourceNode>(entity).is_some();
                let can_trade = self.trade_options(entity).is_some();
                let task_target = self.world.get::<Task>(entity).map(|t| t.target);
                let has_job = task_target.is_some();
                let job_structure = task_target.map(|target| self.entity_label(target));
                let structure_worker = if is_structure {
                    worker_by_structure
                        .get(&entity)
                        .map(|&worker| self.entity_label(worker))
                } else {
                    None
                };
                let stats = self.world.get::<Stats>(entity);
                let hp_fraction = stats.map(|s| s.hp_fraction());
                // Hostile wild programs are recolored by difficulty relative
                // to the player's current power, rather than shown in their
                // species' authored color — see `difficulty_color`. Everyone
                // and everything else (the player, tamed/companion programs,
                // structures) keeps its normal glyph color.
                let color = if is_hostile {
                    stats
                        .map(|s| difficulty_color(s.power(), player_power, is_boss))
                        .unwrap_or(glyph.color)
                } else {
                    glyph.color
                };
                let level = self.world.get::<Experience>(entity).map(|e| e.level);
                let durability = self
                    .world
                    .get::<Durability>(entity)
                    .map(|d| (d.hp, d.max_hp));
                let label = self.entity_label(entity);
                EntityView {
                    entity,
                    pos: (pos.x, pos.y),
                    glyph: glyph.ch,
                    color,
                    label,
                    is_player,
                    is_tamed,
                    is_companion,
                    is_hostile,
                    is_structure,
                    is_home,
                    is_boss,
                    can_work,
                    can_trade,
                    has_job,
                    job_structure,
                    structure_worker,
                    hp_fraction,
                    level,
                    durability,
                    fusions: self.fusion_count(entity),
                }
            })
            .collect()
    }

    /// Species-level detail on a creature `view_entities` reported nearby.
    /// Read-only — looking a program over never triggers an intrusion.
    /// Returns `None` for anything that isn't a creature (e.g. a structure
    /// or the player) or whose species failed to resolve.
    pub fn inspect(&self, entity: Entity) -> Option<InspectView> {
        let creature = self.world.get::<Creature>(entity)?;
        let species = self.world.resource::<SpeciesDb>().get(&creature.species)?;
        let stats = self.world.get::<Stats>(entity)?;
        let level = self.world.get::<Experience>(entity).map(|e| e.level);
        let is_hostile = self.world.get::<Hostile>(entity).is_some();
        let is_tamed = self.world.get::<Tamed>(entity).is_some();
        let decompiler_skill = self.player_decompiler_skill();
        let potency = self
            .world
            .resource::<ItemDb>()
            .get(ids::ICE_BREAKER)
            .and_then(|d| d.taming_potency)
            .unwrap_or(0.0);
        let decompile_chance = taming::capture_chance(
            stats.hp_fraction(),
            potency,
            species.taming_difficulty,
            decompiler_skill,
        );
        let display_name = self
            .world
            .get::<CustomName>(entity)
            .map(|c| c.0.clone())
            .unwrap_or_else(|| species.name.clone());
        Some(InspectView {
            name: self.zone_tagged_name(entity, display_name),
            glyph: species.glyph,
            color: species.color,
            level,
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            def: stats.def,
            power: stats.power(),
            is_hostile,
            is_tamed,
            is_boss: species.is_boss,
            taming_difficulty: species.taming_difficulty,
            decompile_chance,
            habitats: species.habitats.clone(),
            moves: species.moves.clone(),
            work_resource: species.work_resource.clone(),
            quality: self.potential_quality_label(entity),
            fusions: self.fusion_count(entity),
        })
    }

    /// Every deployed structure that's a symlink target (its def has
    /// `teleport_cost` set), anywhere on the map — unlike `view_entities`,
    /// this isn't limited to a scan radius, since the whole point of a
    /// symlink is reaching it from far away.
    pub fn symlink_targets(&mut self) -> Vec<EntityView> {
        let mut query = self
            .world
            .query::<(Entity, &Position, &Glyph, &Structure)>();
        let hits: Vec<(Entity, Position, Glyph, StructureId)> = query
            .iter(&self.world)
            .map(|(e, p, g, s)| (e, *p, *g, s.kind.clone()))
            .collect();

        let db = self.world.resource::<StructureDb>();
        hits.into_iter()
            .filter(|(_, _, _, kind)| db.get(kind).is_some_and(|d| d.teleport_cost.is_some()))
            .map(|(entity, pos, glyph, kind)| EntityView {
                entity,
                pos: (pos.x, pos.y),
                glyph: glyph.ch,
                color: glyph.color,
                label: self.entity_label(entity),
                is_player: false,
                is_tamed: false,
                is_companion: false,
                is_hostile: false,
                is_structure: true,
                is_home: kind == HOME_STRUCTURE_ID,
                is_boss: false,
                can_work: false,
                can_trade: false,
                has_job: false,
                job_structure: None,
                structure_worker: None,
                hp_fraction: None,
                level: None,
                durability: self
                    .world
                    .get::<Durability>(entity)
                    .map(|d| (d.hp, d.max_hp)),
                fusions: 0,
            })
            .collect()
    }

    /// The item cost to symlink to `target`, if it's a symlink-capable
    /// structure — used both by `use_symlink` itself and by the TUI to show
    /// the cost before the player commits to it.
    pub fn symlink_cost(&self, target: Entity) -> Option<Vec<(ItemId, u32)>> {
        let kind = self.world.get::<Structure>(target)?.kind.clone();
        self.world
            .resource::<StructureDb>()
            .get(&kind)
            .and_then(|d| d.teleport_cost.clone())
    }

    /// "Use symlink" — instantly teleports the player to `target` (a
    /// symlink-capable structure from `symlink_targets`), paying its
    /// `teleport_cost` from inventory.
    pub fn use_symlink(&mut self, target: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self.world.get::<Structure>(target).is_none() {
            return Err("That's not a structure.".to_string());
        }
        let cost = self
            .symlink_cost(target)
            .ok_or_else(|| "That structure has no symlink.".to_string())?;
        let player = self.player_entity();
        {
            let inv = self.world.get::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                if inv.count(item.clone()) < *qty {
                    return Err(format!("Not enough {}.", self.item_name(item)));
                }
            }
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            for (item, qty) in &cost {
                inv.take(item.clone(), *qty);
            }
        }
        let target_pos = *self.world.get::<Position>(target).unwrap();
        let name = self.entity_label(target);
        {
            let mut pos = self.world.get_mut::<Position>(player).unwrap();
            pos.x = target_pos.x;
            pos.y = target_pos.y;
        }
        self.log(format!("You use a symlink and teleport to {name}."));
        self.tick();
        Ok(())
    }

    pub fn structure_defs(&self) -> Vec<StructureDef> {
        self.world
            .resource::<StructureDb>()
            .all()
            .cloned()
            .collect()
    }

    /// The display name for `id`, falling back to the raw id if the item set
    /// doesn't define it (a save referencing a since-removed mod item). The
    /// fallback borrows `id`, so the returned reference is bound to the
    /// shorter of `self` and `id`.
    pub fn item_name<'a>(&'a self, id: &'a ItemId) -> &'a str {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .map(|d| d.name.as_str())
            .unwrap_or_else(|| id.as_str())
    }

    pub fn is_equippable(&self, id: &ItemId) -> bool {
        self.equipment_of(id).is_some()
    }

    pub fn equipment_of(&self, id: &ItemId) -> Option<(EquipmentSlot, EquipmentStats)> {
        self.world.resource::<ItemDb>().get(id.as_str())?.equipment
    }

    pub fn is_consumable(&self, id: &ItemId) -> bool {
        self.world
            .resource::<ItemDb>()
            .get(id.as_str())
            .is_some_and(|d| d.consume.is_some())
    }

    pub fn bank_limit_of(&self, id: &ItemId) -> Option<u32> {
        self.world.resource::<ItemDb>().get(id.as_str())?.bank_limit
    }

    pub fn currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .currency()
            .expect("validated at startup")
            .clone()
    }

    pub fn research_currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .research_currency()
            .expect("validated at startup")
            .clone()
    }

    pub fn craft_currency(&self) -> ItemId {
        self.world
            .resource::<ItemDb>()
            .craft_currency()
            .expect("validated at startup")
            .clone()
    }

    /// Whether `structure_id` may be built right now. A structure named by
    /// no research file is unlocked by default — that's what keeps Home, the
    /// Mining Node, the Research Node, the Recharger Node and the Zone
    /// Portal available from turn one without a hardcoded whitelist, and
    /// what keeps a structure mod that ships no research file working
    /// unchanged.
    fn structure_unlocked(&self, structure_id: &str) -> bool {
        let db = self.world.resource::<ResearchDb>();
        let mut gates = db
            .all()
            .filter(|def| def.unlocks_structures.iter().any(|s| s == structure_id))
            .peekable();
        if gates.peek().is_none() {
            return true;
        }
        gates.any(|def| self.is_researched(&def.id))
    }

    /// The structures the build menu offers: `structure_defs` minus anything
    /// still behind unfinished research. `structure_defs` itself stays
    /// unfiltered — it's the general lookup, not the menu.
    pub fn buildable_structure_defs(&self) -> Vec<StructureDef> {
        self.world
            .resource::<StructureDb>()
            .all()
            .filter(|def| self.structure_unlocked(&def.id))
            .cloned()
            .collect()
    }

    /// A one-line summary of everything `def` actually does, for the build
    /// menu. Derived from the def's capability fields plus the research db
    /// rather than an authored `description` field, so a structure a modder
    /// drops in gets an accurate line for free. Lives here rather than in
    /// each renderer because the bench clause needs `ResearchDb`, which the
    /// renderers can't see.
    pub fn structure_description(&self, def: &StructureDef) -> String {
        let mut parts = Vec::new();
        if let Some(work) = &def.work {
            parts.push(format!("cronjob -> {}", self.item_name(&work.produces)));
        }
        if let Some(passive) = &def.passive_process {
            parts.push(format!(
                "passive within {} tiles: {} -> {}",
                passive.radius,
                self.item_name(&passive.consumes),
                self.item_name(&passive.produces)
            ));
        }
        let bench_for: Vec<&str> = self
            .world
            .resource::<ResearchDb>()
            .all()
            .flat_map(|node| &node.unlocks_recipes)
            .filter(|recipe| recipe.requires_structure.as_deref() == Some(def.id.as_str()))
            .map(|recipe| self.item_name(&recipe.result))
            .collect();
        if !bench_for.is_empty() {
            parts.push(format!("compile bench: {}", bench_for.join(", ")));
        }
        if let Some(cost) = &def.teleport_cost {
            let cost = cost
                .iter()
                .map(|(item, qty)| format!("{qty} {}", self.item_name(item)))
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!("symlink target: teleport here for {cost}"));
        }
        if def.zone_portal {
            parts.push("breaches to the next zone; cost scales with zone level".to_string());
        }
        if let Some(trade) = &def.trade {
            let buys = trade
                .buy
                .iter()
                .map(|(item, _)| self.item_name(item))
                .collect::<Vec<_>>()
                .join(", ");
            parts.push(format!(
                "trade: sell anything for {} Core Fragment each, buy {buys}",
                trade.sell_rate
            ));
        }
        if def.raid_defense > 0 {
            parts.push(format!(
                "-{} raid damage to every deployed structure",
                def.raid_defense
            ));
        }
        if def.inventory_bonus > 0 {
            parts.push(format!("+{} inventory capacity", def.inventory_bonus));
        }
        if let Some(rest) = &def.enables_rest {
            parts.push(format!("lets you recharge within {} tiles", rest.radius));
        }
        if let Some(temp) = &def.temporary {
            parts.push(format!("collapses after {} ticks", temp.max_ticks));
        }
        if parts.is_empty() {
            parts.push("no effect yet".to_string());
        }
        parts.join("; ")
    }

    /// How many units of cargo the player can carry right now: the base
    /// capacity plus every deployed structure's `inventory_bonus`. Derived
    /// on each call rather than cached, so a Data Cache lost to a raid
    /// shrinks the buffer with no invalidation step and the save format
    /// stays unchanged.
    pub fn inventory_capacity(&self) -> u32 {
        let kinds: Vec<StructureId> = self
            .world
            .iter_entities()
            .filter_map(|e| e.get::<Structure>().map(|s| s.kind.clone()))
            .collect();
        let db = self.world.resource::<StructureDb>();
        structures::inventory_capacity_for(kinds.iter().map(|k| k.as_str()), db)
    }

    /// Units of cargo currently carried, excluding banked currency.
    pub fn inventory_used(&self) -> u32 {
        let db = self.world.resource::<ItemDb>();
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.cargo_used(db))
            .unwrap_or(0)
    }

    /// `Ok(())` if `qty` more of `item` would fit. Used by the paths where
    /// the player pays an input cost — compiling, buying, unequipping —
    /// since clamping those would destroy value the player already spent.
    fn check_room(&self, item: ItemId, qty: u32) -> Result<(), String> {
        let capacity = self.inventory_capacity();
        let db = self.world.resource::<ItemDb>();
        let inv = self.world.get::<Inventory>(self.player_entity()).unwrap();
        let (used, ceiling, label) = match db.get(item.as_str()).and_then(|d| d.bank_limit) {
            Some(limit) => (inv.count(item), limit, "Research bank"),
            None => (inv.cargo_used(db), capacity, "Buffer"),
        };
        if used + qty > ceiling {
            return Err(format!("{label} full ({used}/{ceiling})."));
        }
        Ok(())
    }

    /// The actual item cost to deploy `def` right now: `def.build_cost`
    /// unchanged for a normal structure, or each amount scaled by the
    /// current zone level for a zone-portal structure (see
    /// `StructureDef::zone_portal`) — breaching deeper costs more raw
    /// material each time.
    pub fn structure_build_cost(&self, def: &StructureDef) -> Vec<(ItemId, u32)> {
        let multiplier = if def.zone_portal {
            self.world.resource::<ZoneLevel>().0
        } else {
            1
        };
        def.build_cost
            .iter()
            .map(|(item, qty)| (item.clone(), qty * multiplier))
            .collect()
    }

    pub fn species_defs(&self) -> Vec<SpeciesDef> {
        self.world.resource::<SpeciesDb>().all().cloned().collect()
    }

    /// `entity`'s trading-post terms (see `StructureDef::trade`), if it's a
    /// structure with any — used both by `sell_item`/`buy_item` and by the
    /// TUI to show prices before the player commits.
    pub fn trade_options(&self, entity: Entity) -> Option<TradeDef> {
        let kind = self.world.get::<Structure>(entity)?.kind.clone();
        self.world
            .resource::<StructureDb>()
            .get(&kind)?
            .trade
            .clone()
    }

    /// Sells `qty` of `item` from inventory to the trading post `structure`,
    /// crediting Core Fragments at its flat `sell_rate` per unit. Core
    /// Fragments themselves can't be sold (trading them for more of the
    /// same thing is meaningless, and would be exploitable if a modded
    /// `sell_rate` was ever above 1).
    pub fn sell_item(&mut self, structure: Entity, item: ItemId, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if qty == 0 {
            return Err("Sell at least 1.".into());
        }
        let currency = self.currency();
        if item == currency {
            return Err("Core Fragments aren't worth trading for more Core Fragments.".into());
        }
        let trade = self
            .trade_options(structure)
            .ok_or_else(|| "That structure doesn't trade.".to_string())?;
        let player = self.player_entity();
        let have = self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(item.clone());
        if have == 0 {
            return Err(format!("You don't have any {}.", self.item_name(&item)));
        }
        let taken = have.min(qty);
        let payout = trade.sell_rate * taken;
        // Refuse rather than clamp: the item is already gone once `take`
        // runs, so checking room only after taking would let a refusal
        // destroy the sold item for nothing.
        self.check_room(currency.clone(), payout)?;
        let name = self.item_name(&item).to_string();
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            inv.take(item, taken);
            inv.add(currency, payout);
        }
        self.log(format!(
            "You sell {taken} {name} for {payout} Core Fragments."
        ));
        self.tick();
        Ok(())
    }

    /// Buys `qty` of `item` from the trading post `structure`, at its
    /// listed per-unit Core Fragment cost.
    pub fn buy_item(&mut self, structure: Entity, item: ItemId, qty: u32) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if qty == 0 {
            return Err("Buy at least 1.".into());
        }
        let trade = self
            .trade_options(structure)
            .ok_or_else(|| "That structure doesn't trade.".to_string())?;
        let (_, unit_cost) = trade
            .buy
            .iter()
            .find(|(i, _)| *i == item)
            .ok_or_else(|| format!("{} isn't for sale here.", self.item_name(&item)))?;
        let total_cost = unit_cost * qty;
        let currency = self.currency();
        let player = self.player_entity();
        if self
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(currency.clone())
            < total_cost
        {
            return Err(format!("Not enough Core Fragments (need {total_cost})."));
        }
        self.check_room(item.clone(), qty)?;
        let name = self.item_name(&item).to_string();
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            inv.take(currency, total_cost);
            inv.add(item, qty);
        }
        self.log(format!(
            "You buy {qty} {name} for {total_cost} Core Fragments."
        ));
        self.tick();
        Ok(())
    }
}

/// `Game::forage`'s success chance for `biome`, boosted by
/// `KEEN_SCAVENGER_BONUS_PER_LEVEL` for every level of `keen_scavenger_level`
/// (capped at 1.0) — pulled out of the method so the formula is
/// unit-testable without an RNG.
fn forage_chance(biome: Biome, keen_scavenger_level: u32) -> f64 {
    let chance = match biome {
        Biome::Mainframe | Biome::OpenGrid => 0.6,
        Biome::NullSector => 0.3,
        Biome::StaticField => 0.15,
        Biome::DataVoid | Biome::BlackIce => 0.0,
    };
    if chance > 0.0 && keen_scavenger_level > 0 {
        (chance + KEEN_SCAVENGER_BONUS_PER_LEVEL * keen_scavenger_level as f64).min(1.0)
    } else {
        chance
    }
}

/// Old-school "con"-style map coloring for a hostile wild program, relative
/// to the player's current `Stats::power`. A boss is always Magenta
/// regardless of the ratio; everything else runs Green (easy) → Yellow
/// (even) → Orange (tough) → Red (hard) as `creature_power` grows past
/// `player_power`. Pulled out of `view_entities` so the bucketing is
/// unit-testable without spinning up a `Game`.
fn difficulty_color(creature_power: i32, player_power: i32, is_boss: bool) -> GlyphColor {
    if is_boss {
        return GlyphColor::Magenta;
    }
    let ratio = creature_power as f64 / player_power.max(1) as f64;
    if ratio <= DIFFICULTY_EASY_MAX {
        GlyphColor::Green
    } else if ratio <= DIFFICULTY_EVEN_MAX {
        GlyphColor::Yellow
    } else if ratio <= DIFFICULTY_TOUGH_MAX {
        GlyphColor::Orange
    } else {
        GlyphColor::Red
    }
}

fn find_walkable_start(world_map: &mut WorldMap) -> (i32, i32) {
    for r in 0..64i32 {
        for dx in -r..=r {
            for dy in -r..=r {
                if r != 0 && dx.abs() != r && dy.abs() != r {
                    continue;
                }
                if world_map.tile(dx, dy).walkable {
                    return (dx, dy);
                }
            }
        }
    }
    (0, 0)
}

#[cfg(test)]
mod tests {
    use super::*;
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

    /// Copies the shipped `species`/`structures`/`research`/`items` asset
    /// dirs into a scratch dir, omitting `core_fragment.ron` — the item
    /// that holds the Currency economy role — so `Game::new`'s
    /// missing-role startup abort (see `ItemDb::missing_roles`) can be
    /// exercised against an otherwise-valid item set.
    fn assets_dir_missing_currency_item() -> std::path::PathBuf {
        let dir = std::env::temp_dir().join(format!(
            "feral_processes_missing_currency_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&dir);
        let shipped = test_assets_dir();
        for sub in ["species", "structures", "research", "items"] {
            let dst = dir.join(sub);
            std::fs::create_dir_all(&dst).unwrap();
            for entry in std::fs::read_dir(shipped.join(sub)).unwrap() {
                let entry = entry.unwrap();
                if sub == "items" && entry.file_name().to_str() == Some("core_fragment.ron") {
                    continue;
                }
                std::fs::copy(entry.path(), dst.join(entry.file_name())).unwrap();
            }
        }
        dir
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
    fn inventory_capacity_grows_with_each_deployed_data_cache() {
        let base = structures::BASE_INVENTORY_CAPACITY;
        let mut game = Game::new(700, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert_eq!(game.inventory_capacity(), base);

        spawn_data_cache(&mut game, 1);
        assert_eq!(game.inventory_capacity(), base + 10);

        spawn_data_cache(&mut game, 2);
        assert_eq!(game.inventory_capacity(), base + 20, "caches stack");
    }

    #[test]
    fn destroying_a_data_cache_shrinks_the_capacity_back() {
        let base = structures::BASE_INVENTORY_CAPACITY;
        let mut game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        spawn_data_cache(&mut game, 1);
        assert_eq!(game.inventory_capacity(), base + 10);

        let cache = game
            .world
            .iter_entities()
            .find(|e| e.get::<Structure>().is_some_and(|s| s.kind == "data_cache"))
            .map(|e| e.id())
            .expect("the spawned cache should be findable");
        game.world.despawn(cache);

        assert_eq!(
            game.inventory_capacity(),
            base,
            "capacity is derived, so a destroyed cache needs no invalidation"
        );
    }

    /// Fills the player's cargo to exactly the current capacity so the next
    /// pickup has nowhere to go.
    fn fill_buffer(game: &mut Game) {
        let capacity = game.inventory_capacity();
        let player = game.player_entity();
        let used = game.inventory_used();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), capacity - used);
    }

    #[test]
    fn compiling_into_a_full_buffer_refuses_and_consumes_nothing() {
        let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill_buffer(&mut game);
        let player = game.player_entity();
        let cores_before = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::from(ids::CORE_FRAGMENT));

        let err = game
            .craft(ItemId::from(ids::POWER_CELL), 1)
            .expect_err("a full buffer should refuse a compile");

        assert!(err.contains("Buffer full"), "got: {err}");
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(ItemId::from(ids::CORE_FRAGMENT)),
            cores_before,
            "a refused compile must not consume its inputs"
        );
    }

    #[test]
    fn unequipping_into_a_full_buffer_refuses_and_keeps_the_gear_equipped() {
        let mut game = Game::new(706, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
        game.equip(ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
        fill_buffer(&mut game);

        let err = game
            .unequip(EquipmentSlot::Weapon)
            .expect_err("a full buffer should refuse an unequip");

        assert!(err.contains("Buffer full"), "got: {err}");
        assert!(
            game.player_status().weapon.is_some(),
            "refused unequip must leave the gear equipped, not delete it"
        );
    }

    #[test]
    fn a_compile_still_works_with_exactly_enough_room() {
        let mut game = Game::new(707, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let capacity = game.inventory_capacity();
        let player = game.player_entity();
        let used = game.inventory_used();
        // `check_room` only ever measures the recipe's *output* quantity
        // (1 Power Cell) against pre-consumption cargo, never the net of
        // input minus output — so this passes because used(capacity - 1)
        // + 1 <= capacity, regardless of what the recipe consumes.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), capacity - used - 1);

        game.craft(ItemId::from(ids::POWER_CELL), 1)
            .expect("a compile that nets out under capacity should succeed");
    }

    #[test]
    fn foraging_into_a_full_buffer_loses_the_find_and_says_so() {
        let mut game = Game::new(703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        fill_buffer(&mut game);
        let before = game.inventory_used();

        // Forage until the RNG grants a find, so the assertion doesn't
        // depend on a specific seed's first roll.
        for _ in 0..200 {
            game.forage();
        }

        assert_eq!(
            game.inventory_used(),
            before,
            "a full buffer must not grow, however many finds are rolled"
        );
        assert_eq!(
            game.inventory_used(),
            game.inventory_capacity(),
            "and must stay exactly at capacity"
        );
    }

    #[test]
    fn a_partially_full_buffer_takes_only_what_fits() {
        let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let capacity = game.inventory_capacity();
        let player = game.player_entity();
        let used = game.inventory_used();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), capacity - used - 1);
        assert_eq!(game.inventory_used(), capacity - 1);

        let landed = game.grant_loot(ItemId::from(ids::PORTAL_FRAGMENT), 6);

        assert_eq!(landed, 1, "only the single unit of room should land");
        assert_eq!(game.inventory_used(), capacity);
        assert_eq!(
            game.player_status()
                .inventory
                .iter()
                .find(|(i, _)| *i == ItemId::from(ids::PORTAL_FRAGMENT))
                .map(|(_, q)| *q),
            Some(1)
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

        let status = game.player_status();
        assert_eq!(status.inventory_used, 11);
        assert_eq!(
            status.inventory_capacity,
            structures::BASE_INVENTORY_CAPACITY
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
    fn a_cronjob_worker_cannot_overfill_the_buffer() {
        let mut game = Game::new(708, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assign_worker_producing(&mut game, ItemId::from(ids::CORE_FRAGMENT));
        fill_buffer(&mut game);
        let capacity = game.inventory_capacity();

        for _ in 0..100 {
            game.tick();
        }

        assert_eq!(
            game.inventory_used(),
            capacity,
            "a working cronjob must fill the buffer to exactly capacity and stop"
        );
    }

    #[test]
    fn a_research_cronjob_keeps_banking_with_a_full_cargo_buffer() {
        let mut game = Game::new(709, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assign_worker_producing(&mut game, ItemId::from(ids::RESEARCH_DATA));
        fill_buffer(&mut game);
        let before = research_data_held(&game);

        for _ in 0..100 {
            game.tick();
        }

        assert!(
            research_data_held(&game) > before,
            "a full cargo buffer must not stop research from banking (was {before}, now {})",
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

    #[test]
    fn the_two_starter_recipes_need_no_research() {
        let game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
        assert!(results.contains(&ItemId::from(ids::ICE_BREAKER)));
        assert!(results.contains(&ItemId::from(ids::POWER_CELL)));
        assert_eq!(results.len(), 2, "nothing else is free");
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
            game.craft_cost(ItemId::from(ids::OVERCLOCK_CORE)),
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
            .count(resource.clone());
        game.award_loot(wild);
        let after = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(resource.clone());

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
            .find(|s| s.work_resource.is_none() && s.equipment_drop.is_none())
            .expect("at least one species should have neither a work_resource nor an equipment_drop for this test");

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

        // Portal Fragments are a universal drop independent of species, so
        // count everything *except* those to check the species-specific
        // channels stayed silent.
        let count_non_portal = |game: &Game| -> u32 {
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .items
                .iter()
                .filter(|(item, _)| *item != ItemId::from(ids::PORTAL_FRAGMENT))
                .map(|(_, q)| *q)
                .sum()
        };
        let before = count_non_portal(&game);
        game.award_loot(wild);
        let after = count_non_portal(&game);

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
        assert!((0.0..=1.0).contains(&view.decompile_chance));
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
            .map(|(item, _)| {
                game.world
                    .get::<Inventory>(player)
                    .unwrap()
                    .count(item.clone())
            })
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
            let after = game
                .world
                .get::<Inventory>(player)
                .unwrap()
                .count(item.clone());
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
        // 15-tile build radius from Home.
        game.world.get_mut::<Position>(player).unwrap().x += 20;
        let err = game
            .place_structure("armory", 1, 0)
            .expect_err("structures more than 15 tiles from Home shouldn't be buildable");
        assert!(err.contains("Too far from Home"), "unexpected error: {err}");

        // Walking back within range should make it buildable again.
        game.world.get_mut::<Position>(player).unwrap().x -= 20;
        game.place_structure("armory", 1, 0)
            .expect("building back within 15 tiles of Home should succeed");
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
            .count(ItemId::from(ids::CORE_FRAGMENT));
        game.remove_structure(armory).unwrap();
        let after = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::from(ids::CORE_FRAGMENT));

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
            .count(ItemId::from(ids::CORE_FRAGMENT));
        game.remove_structure(home).unwrap();
        let after = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::from(ids::CORE_FRAGMENT));

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
    fn removing_home_cascade_refund_is_capped_to_available_room() {
        let mut game = Game::new(305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        unlock_research_chain(&mut game, "armor_bench");
        unlock_research_chain(&mut game, "weapon_bench");
        let player = game.player_entity();
        let capacity = game.inventory_capacity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::CORE_FRAGMENT), 50);
        place_home(&mut game, -1, 0);
        game.place_structure("armory", 1, 0).unwrap();
        game.place_structure("fabricator", 0, 1).unwrap();
        let home = game
            .view_entities(10, 10)
            .into_iter()
            .find(|e| e.is_home)
            .unwrap()
            .entity;

        // Clear every starting item (gear plus leftover build materials)
        // and refill with something the refund doesn't touch, leaving
        // exactly 3 units of room — proving the cascade clamps rather than
        // relying on an incidentally-near-empty buffer.
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            for item in [
                ItemId::from(ids::ICE_BREAKER),
                ItemId::from(ids::POWER_CELL),
                ItemId::from(ids::CORE_FRAGMENT),
            ] {
                let held = inv.count(item.clone());
                inv.take(item, held);
            }
            inv.add(ItemId::from(ids::FIREWALL_PLATING), capacity - 3);
        }

        game.remove_structure(home).unwrap();

        assert!(
            game.inventory_used() <= capacity,
            "a demolition refund cascade must never push cargo past capacity"
        );
        assert!(
            game.message_log(10)
                .iter()
                .any(|(_, line)| line.contains("full")),
            "a clamped refund should say so, same as any other unsolicited income"
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
            // state when `inventory_bonus` was added without updating this.
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
        assert!(describe("data_cache").contains("inventory capacity"));
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
        game.craft(ItemId::from(ids::FIREWALL_PLATING), 1).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(game.player_entity())
                .unwrap()
                .count(ItemId::from(ids::FIREWALL_PLATING)),
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
            .count(ItemId::from(ids::CORE_FRAGMENT));

        for _ in 0..40 {
            game.tick();
        }

        let gained = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::from(ids::CORE_FRAGMENT))
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

        game.equip(ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

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

        game.equip(ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

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

        game.equip(ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
        assert_eq!(game.player_status().atk, atk_before + 3);

        // Equipping into an already-occupied slot swaps the old item back
        // to inventory and must not stack the bonus a second time.
        game.equip(ItemId::from(ids::OVERCLOCK_CORE)).unwrap();
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
        game.equip(ItemId::from(ids::FIREWALL_PLATING)).unwrap();
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
        game.world.get_mut::<Equipment>(player).unwrap().weapon = Some(EquippedItem {
            item: ItemId::from("a_removed_mod_item"),
            level: 1,
            fusion_tier: 0,
        });

        let result = game.unequip(EquipmentSlot::Weapon);

        assert!(
            result.is_err(),
            "unequipping an item absent from ItemDb should error, not panic"
        );
    }

    #[test]
    fn equipping_over_a_slot_holding_an_item_with_no_itemdb_entry_errors_instead_of_panicking() {
        // Same failure mode as the unequip case above, but hit via the
        // swap-out path when equipping a new item into an already-occupied
        // slot whose old occupant's data is gone.
        let mut game = Game::new(713, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Equipment>(player).unwrap().weapon = Some(EquippedItem {
            item: ItemId::from("a_removed_mod_item"),
            level: 1,
            fusion_tier: 0,
        });
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 1);

        let result = game.equip(ItemId::from(ids::OVERCLOCK_CORE));

        assert!(
            result.is_err(),
            "equipping over a slot whose old item is absent from ItemDb should error, not panic"
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

        game.fuse_item(ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

        assert_eq!(game.item_fusion_tier(ItemId::from(ids::OVERCLOCK_CORE)), 1);
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
        game.equip(ItemId::from(ids::ABLATIVE_PLATING)).unwrap();
        assert_eq!(
            game.player_status().def,
            def_before + 4,
            "unfused equip should grant the plain base bonus"
        );
        game.unequip(EquipmentSlot::Armor).unwrap();

        game.fuse_item(ItemId::from(ids::ABLATIVE_PLATING)).unwrap();
        game.fuse_item(ItemId::from(ids::ABLATIVE_PLATING)).unwrap();
        assert_eq!(
            game.item_fusion_tier(ItemId::from(ids::ABLATIVE_PLATING)),
            2
        );

        game.equip(ItemId::from(ids::ABLATIVE_PLATING)).unwrap();
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
            game.fuse_item(ItemId::from(ids::CORE_FRAGMENT)).is_err(),
            "plain resources aren't equipment and can't be fused"
        );

        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 1);
        assert!(
            game.fuse_item(ItemId::from(ids::OVERCLOCK_CORE)).is_err(),
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
    fn item_fusion_tier_survives_save_and_load() {
        let assets = test_assets_dir();
        let mut game = Game::new(203, DifficultyMode::Forgiving, &assets).unwrap();
        let player = game.player_entity();
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::OVERCLOCK_CORE), 2);
        game.fuse_item(ItemId::from(ids::OVERCLOCK_CORE)).unwrap();

        let path = std::env::temp_dir().join(format!(
            "feral_processes_fusion_test_{}.bin",
            std::process::id()
        ));
        game.save(&path).unwrap();
        let loaded = Game::load(&path, &assets).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            loaded.item_fusion_tier(ItemId::from(ids::OVERCLOCK_CORE)),
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

        game.erase_item(ItemId::from(ids::NEURAL_AMPLIFIER), 3)
            .unwrap();
        assert!(
            game.player_status()
                .inventory
                .iter()
                .all(|(i, _)| *i != ItemId::from(ids::NEURAL_AMPLIFIER))
        );

        assert!(
            game.erase_item(ItemId::from(ids::NEURAL_AMPLIFIER), 1)
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
        game.equip(ItemId::from(ids::NEURAL_AMPLIFIER)).unwrap();
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![player],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
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
        spawn_recharger_node_at_player(&mut game);

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
        spawn_recharger_node_at_player(&mut game);

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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
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
            game.battle_decompile();
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

        game.craft(ItemId::from(ids::ICE_BREAKER), 1).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(ItemId::from(ids::CORE_FRAGMENT)),
            0,
            "cost should be fully consumed"
        );
        assert_eq!(
            inv.count(ItemId::from(ids::ICE_BREAKER)),
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

        game.craft(ItemId::from(ids::ICE_BREAKER), 3).unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(ItemId::from(ids::CORE_FRAGMENT)),
            0,
            "cost should scale with quantity"
        );
        assert_eq!(
            inv.count(ItemId::from(ids::ICE_BREAKER)),
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

        assert_eq!(game.max_craftable(ItemId::from(ids::ICE_BREAKER)), 2);
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
            game.max_craftable(ItemId::from(ids::ICE_BREAKER)),
            0,
            "no resources at all"
        );
        assert_eq!(
            game.max_craftable(ItemId::from(ids::CORE_FRAGMENT)),
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

        assert!(game.craft(ItemId::from(ids::ICE_BREAKER), 1).is_err());
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(ItemId::from(ids::ICE_BREAKER)),
            0
        );
    }

    #[test]
    fn craft_rejects_a_result_with_no_recipe() {
        let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        assert!(game.craft(ItemId::from(ids::CORE_FRAGMENT), 1).is_err());
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

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

    /// Deploys a Recharger Node directly on the player's current tile —
    /// `Game::rest` requires one nearby, so tests exercising `rest` need
    /// this in place first. Spawned directly rather than through
    /// `place_structure` to sidestep its Home/cost/radius requirements,
    /// which aren't what these tests are about. The real Recharger Node is
    /// a permanent structure (no `Temporary` component), so this doesn't
    /// attach one either.
    fn spawn_recharger_node_at_player(game: &mut Game) {
        let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
        game.world.spawn((
            Structure {
                kind: "recharger_node".to_string(),
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
        assert_eq!(companion_info.job_structure, None);

        let worker_info = pets.iter().find(|p| p.entity == far_worker).unwrap();
        assert!(!worker_info.is_companion);
        assert!(
            worker_info.job_structure.is_some(),
            "a far-off cronjob worker should still be reported"
        );
        assert_eq!(worker_info.hp, 12);
        assert_eq!(worker_info.atk, 4);

        let idle_info = pets.iter().find(|p| p.entity == idle).unwrap();
        assert!(!idle_info.is_companion);
        assert_eq!(idle_info.job_structure, None);
    }

    #[test]
    fn battle_command_companion_rallies_the_player_instead_of_attacking() {
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_command_companion(companion);

        let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
        assert_eq!(
            wild_hp, 100,
            "commanding a companion should never damage the wild creature directly"
        );
        let buff = game.world.get::<PlayerBuff>(player).unwrap().active;
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        let fatigue_before = game.world.get::<Needs>(player).unwrap().fatigue;
        game.battle_command_companion(companion);
        let fatigue_after = game.world.get::<Needs>(player).unwrap().fatigue;
        fatigue_before - fatigue_after
    }

    #[test]
    fn commanding_a_companion_in_battle_costs_more_fatigue_than_a_stunned_one() {
        // Both paths advance the clock by one tick (`battle_command_companion`
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
        game.world.get_mut::<PlayerBuff>(player).unwrap().active = Some(ActiveBuff {
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

        let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
        assert!(
            wild_hp < 10_000 - 50,
            "a +50 ATK buff should meaningfully increase damage dealt"
        );
        assert!(
            game.world
                .get::<PlayerBuff>(player)
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

        game.use_special_ability(&SpecialAbility::Heal { power: 8 }, "TestBot", player, wild);
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
            player,
            wild,
        );
        let active = game.world.get::<StatusEffects>(wild).unwrap().active;
        assert!(
            active.is_some_and(|a| a.kind == StatusKind::Bleed && a.power == 4 && a.remaining == 2),
            "Debuff should inflict the given status condition on the wild creature"
        );
    }

    #[test]
    fn companion_ability_label_shows_special_ability_or_a_computed_attack_rally() {
        let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let all_species = game.species_defs();
        let no_ability_species = all_species
            .iter()
            .find(|s| s.special_ability.is_none())
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
            plain_ability, "Rally Team",
            "a species with no special_ability should show the generic Rally Team fallback"
        );

        if let Some((species_id, expected)) = all_species
            .iter()
            .find_map(|s| s.special_ability.clone().map(|a| (s.id.clone(), a)))
        {
            let with_ability = game
                .world
                .spawn((
                    Creature {
                        species: species_id,
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
            game.add_companion(with_ability).unwrap();
            let shown = game
                .player_status()
                .companions
                .iter()
                .find(|c| c.entity == with_ability)
                .unwrap()
                .ability
                .clone();
            assert_eq!(
                shown,
                expected.short_name(),
                "a species with a special_ability should show its own short name, not the generic rally"
            );
        }
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

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
            game.world.insert_resource(BattleState {
                player,
                wild_creatures: vec![wild],
                log: Vec::new(),
                finished: false,
                player_won: false,
            });

            game.battle_attack();

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
    fn a_knocked_out_companion_stands_down() {
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
            game.world.insert_resource(BattleState {
                player,
                wild_creatures: vec![wild],
                log: Vec::new(),
                finished: false,
                player_won: false,
            });

            game.wild_retaliate(wild, player);
            if game.world.get::<Stats>(companion).unwrap().hp == 0 {
                assert!(
                    game.player_status().companions.is_empty(),
                    "0 HP should have stood the companion down"
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
    fn battle_command_companion_rejects_a_program_not_in_the_party() {
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_command_companion(not_in_party);

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
        spawn_recharger_node_at_player(&mut game);

        game.rest();

        assert_eq!(game.world.get::<Stats>(a).unwrap().hp, 10);
        assert_eq!(game.world.get::<Stats>(b).unwrap().hp, 10);
    }

    #[test]
    fn recharger_node_structure_loads_with_the_expected_rest_schema_and_is_permanent() {
        let game = Game::new(400, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let def = game
            .structure_defs()
            .into_iter()
            .find(|d| d.id == "recharger_node")
            .expect("recharger_node.ron should load");
        assert_eq!(def.build_cost, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);
        assert_eq!(def.enables_rest.as_ref().unwrap().radius, 2);
        assert!(
            def.temporary.is_none(),
            "the Recharger Node should be a permanent structure"
        );
    }

    #[test]
    fn rest_is_a_no_op_without_a_nearby_recharger_node() {
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
            "resting without a nearby Recharger Node shouldn't restore anything"
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
            .count(ItemId::from(ids::CORE_FRAGMENT));

        game.sell_item(market, ItemId::from(ids::FIREWALL_PLATING), 2)
            .unwrap();

        let inv = game.world.get::<Inventory>(player).unwrap();
        assert_eq!(
            inv.count(ItemId::from(ids::FIREWALL_PLATING)),
            1,
            "only the sold quantity should leave the inventory"
        );
        let sell_rate = def.trade.as_ref().unwrap().sell_rate;
        assert_eq!(
            inv.count(ItemId::from(ids::CORE_FRAGMENT)),
            cf_before + sell_rate * 2
        );
    }

    #[test]
    fn sell_item_refuses_a_payout_that_would_overflow_the_buffer() {
        let mut game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let research_bank_limit = game
            .bank_limit_of(&ItemId::from(ids::RESEARCH_DATA))
            .expect("research_data ships with a bank limit");
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

        // Research Data is banked separately (200-unit limit) and exempt
        // from the cargo cap, so a player can plausibly hold far more of it
        // than the 20-unit buffer a fresh game starts with. `used` is read
        // before the fill — banked Research Data never counts as cargo, so
        // adding it first wouldn't change the number anyway.
        let capacity = game.inventory_capacity();
        let used = game.inventory_used();
        {
            let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
            inv.add(ItemId::from(ids::RESEARCH_DATA), research_bank_limit);
            inv.add(ItemId::from(ids::CORE_FRAGMENT), capacity - used);
        }

        let result = game.sell_item(
            market,
            ItemId::from(ids::RESEARCH_DATA),
            research_bank_limit,
        );

        assert!(
            result.is_err(),
            "selling a full Research Data bank must not blow past cargo capacity"
        );
        let held = game
            .world
            .get::<Inventory>(player)
            .unwrap()
            .count(ItemId::from(ids::RESEARCH_DATA));
        assert_eq!(
            held, research_bank_limit,
            "a refused sale must not consume the item being sold"
        );
        assert!(
            game.inventory_used() <= capacity,
            "cargo must never exceed capacity as a result of a sale"
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
            inv.count(ItemId::from(ids::CORE_FRAGMENT)),
            0,
            "the full cost should be charged"
        );
        assert_eq!(inv.count(buy_item), 2);
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

    #[test]
    fn raid_check_can_damage_an_undefended_structure() {
        // RAID_CHANCE_PER_TICK is a per-call roll; drive many seeds until it
        // fires at least once, same pattern as the wild-retaliation test.
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

            game.raid_check();

            let Some(durability) = game.world.get::<Durability>(structure) else {
                // Destroyed outright (30 durability, RAID_DAMAGE 10 — shouldn't
                // happen in one hit, but tolerate it rather than assume).
                return;
            };
            if durability.hp < 30 {
                return;
            }
        }
        panic!(
            "raid_check never damaged the structure across 300 seeds — the raid roll may be broken"
        );
    }

    #[test]
    fn raid_damage_message_is_tagged_message_kind_raid() {
        // Same seed-hunting pattern as raid_check_can_damage_an_undefended_structure
        // — RAID_CHANCE_PER_TICK is a per-call roll, so drive seeds until it fires.
        for seed in 0..300u32 {
            let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
            game.world.spawn((
                Structure {
                    kind: "mining_node".to_string(),
                },
                Position { x: 5, y: 5 },
                Durability { hp: 30, max_hp: 30 },
            ));

            game.raid_check();

            let tagged = game
                .message_log(10)
                .into_iter()
                .any(|(kind, _)| kind == MessageKind::Raid);
            if tagged {
                return;
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
    fn assign_guard_defends_a_structure_with_no_work_recipe() {
        let mut game = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "home".to_string(),
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
        let structure = game
            .world
            .spawn((
                Structure {
                    kind: "home".to_string(),
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
        panic!("raid_check never rolled across 300 seeds — the raid roll may be broken");
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
        let base_cost = game.craft_cost(ItemId::from(ids::POWER_CELL));
        assert_eq!(
            base_cost,
            vec![(ItemId::from(ids::CORE_FRAGMENT), POWER_CELL_CORE_COST)]
        );

        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::LeanCompiler).unwrap();
        let discounted = game.craft_cost(ItemId::from(ids::POWER_CELL));
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
        let floored = game.craft_cost(ItemId::from(ids::POWER_CELL));
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
    fn distance_stat_multiplier_grows_with_distance_from_the_zone_spawn_point_and_caps() {
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
    fn max_pack_size_grows_with_zone_and_distance_and_caps_per_zone() {
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
            "one full step away should allow a packmate, and zone 1's cap is 2"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES * 10, spawn.y),
            2,
            "zone 1's cap of 2 should hold even far past the first step"
        );

        game.world.resource_mut::<ZoneLevel>().0 = 2;
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES, spawn.y),
            2,
            "zone 2 grows the same way per step, just with a higher cap"
        );
        assert_eq!(
            game.max_pack_size(spawn.x + PACK_SIZE_STEP_TILES * 2, spawn.y),
            3,
            "two steps away should reach zone 2's cap of 3"
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![front, second],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

        assert!(
            game.has_active_battle(),
            "a pack member is still alive, so the fight should continue rather than end"
        );
        let view = game
            .battle_view()
            .expect("battle should still be active with the second member up front");
        assert_eq!(
            view.pack_remaining, 0,
            "only the second (surviving) member should remain, now as the front"
        );
        assert_eq!(
            view.wild_hp, 500,
            "the new front should be the untouched second pack member"
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
        assert!(
            pack.len() <= 2,
            "zone 1's pack cap is 2 even with 3 other Hostiles nearby, got {}",
            pack.len()
        );
        assert!(
            pack.len() >= 2,
            "at least one nearby Hostile should have joined the anchor"
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
            .count(ItemId::from(ids::PORTAL_FRAGMENT));
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
            kind: StatusKind::Stun,
            remaining: 1,
            power: 0,
        });

        let wild_hp_before = game.world.get::<Stats>(wild).unwrap().hp;
        game.battle_attack();
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        let player_atk = game.world.get::<Stats>(player).unwrap().atk;
        let expected_attack_dmg = battle::compute_damage(player_atk, 0, 5);

        let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
        game.battle_attack();
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
        game.battle_attack();
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![wild],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
            kind: StatusKind::Bleed,
            remaining: 5,
            power: 1,
        });

        // 1 HP wild creature dies to the player's first attack, ending the battle.
        game.battle_attack();

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

    #[test]
    fn zone_transition_carries_tamed_companions_but_leaves_structures_and_wild_creatures_behind() {
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
            game.world.get::<Structure>(home).is_none(),
            "structures should be left behind when breaching a zone"
        );
        let companion_pos = *game.world.get::<Position>(companion).unwrap();
        let player_pos = *game.world.get::<Position>(player).unwrap();
        assert_eq!(
            companion_pos, player_pos,
            "the companion should travel with the player into the new zone"
        );
    }

    #[test]
    fn portal_build_cost_scales_with_current_zone_level() {
        let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        place_home(&mut game, -1, 0);

        // Zone 1: base rate from portal.ron is 10 PortalFragment * zone 1 = 10.
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 10);
        game.place_structure("portal", 1, 0).unwrap();
        assert_eq!(
            game.world
                .get::<Inventory>(player)
                .unwrap()
                .count(ItemId::from(ids::PORTAL_FRAGMENT)),
            0,
            "zone 1 portal should cost the base rate"
        );

        game.move_player(1, 0);
        assert_eq!(game.player_status().zone, 2);
        // Zone transitions leave structures behind (see
        // `zone_transition_carries_tamed_companions_but_leaves_structures_and_wild_creatures_behind`),
        // so the new zone needs its own Home before anything else.
        place_home(&mut game, -1, 0);

        // Zone 2: cost should now be doubled (10 * zone level 2 = 20).
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::PORTAL_FRAGMENT), 19);
        assert!(
            game.place_structure("portal", 1, 0).is_err(),
            "19 fragments shouldn't be enough for a zone-2 portal"
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
                .count(ItemId::from(ids::PORTAL_FRAGMENT)),
            0,
            "zone 2 portal should cost double the base rate"
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        game.battle_attack();

        // battle_attack's own kill-resolution path (finish_front_pack_member
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });
        game.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .add(ItemId::from(ids::ICE_BREAKER), 50);
        game.world.get_mut::<Decompiler>(player).unwrap().skill = 50;

        for _ in 0..50 {
            if game.world.get::<Tamed>(guardian).is_some() {
                break;
            }
            game.battle_decompile();
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
        game.world.insert_resource(BattleState {
            player,
            wild_creatures: vec![guardian],
            log: Vec::new(),
            finished: false,
            player_won: false,
        });

        // Should not panic even though `gone_nest` has no Nest component.
        game.battle_attack();

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
}
