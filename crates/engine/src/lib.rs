pub mod abilities;
pub mod achievements;
pub mod arena;
pub mod balance_sim;
pub mod battle;
pub mod components;
pub mod crash_logs;
pub mod descriptions;
pub mod difficulty;
mod game;
pub mod items;
pub mod items_db;
pub mod perks;
pub mod policy;
pub mod progression;
pub mod research;
pub mod resources;
pub mod save;
pub mod species;
pub mod stack;
pub mod structures;
pub mod systems;
pub mod taming;
pub mod telemetry;
pub mod tuning;
pub mod views;
pub mod world;

use crate::tuning::{
    BASE_PET_CAPACITY, MAX_INDIVIDUAL_ROLL, MAX_PARTY_SIZE, MIN_INDIVIDUAL_ROLL,
    ZONE_PORTAL_COST_GROWTH_PERCENT,
};
use std::collections::HashMap;
use std::path::Path;

pub use bevy_ecs::prelude::Entity;
use bevy_ecs::prelude::*;
use rand::rngs::StdRng;
use rand::{RngExt, SeedableRng};

use abilities::{AbilityDb, AbilityDef, AbilityEffect, AbilityTarget};
use battle::{
    ActionKind, ActionOption, AllyOption, BattleAction, EnemyGroup, PartyCommand, PartyCommandKind,
    SpecialOption, TargetSpec,
};
use components::{
    AbilityCooldowns, ActiveBuff, ActiveFieldBuff, ActiveStatus, BuffKind, BuffSource, Carrying,
    CombatBuff, Creature, CustomName, Decompiler, Durability, Equipment, EquippedItem, Experience,
    FieldBuff, FieldBuffKind, FusedGear, FusionCount, Glyph, GlyphColor, Hostile, Inventory,
    MachineStatus, NEED_MAX, Needs, Nest, NestGuardian, Perks, Player, Position, Potential,
    Pursuing, ResourceNode, Routines, StackSpawn, Stats, StatusEffects, StatusKind, Stock,
    Structure, StructureTier, SurfaceLink, Tamed, Task, TaskKind, Temporary, WanderAi, ZonePortal,
};
use items::{EquipmentSlot, EquipmentStats, ItemCategory, ItemId, ids};
use items_db::ItemDb;
#[cfg(test)]
use items_db::ItemDef;
pub use perks::{Perk, PerkDb, PerkDef};
use research::{ResearchDb, ResearchDef};
pub use research::{ResearchId, ResearchRecipe};
use resources::{
    BattleState, BattleTimeline, BuybackLedger, ClosingRoster, CurrentStack, EffectQueue,
    GameClock, GameOver, GameRng, KnownRoutines, Locale, MessageLog, Party, Platform, PlayerEntity,
    Research, RosterFrame, StackMemory, WieldedProgram, ZoneLevel, ZoneSpawnPoint,
};
pub use resources::{
    DifficultyMode, EffectKind, LogEntry, LogLine, MESSAGE_LOG_CAP, MessageKind, MessageSource,
    SlotShift, VisualEffect,
};
use species::{Affinities, MoveDef, SpeciesDb, SpeciesDef, SpeciesId};
use structures::{StructureDb, StructureDef, StructureId, TradeDef};
pub use views::*;
use world::{Biome, Tile, WorldMap};

/// Longest name a player can give a fused program (see
/// `Game::fuse_companions`) — enforced by truncation, not rejection, so a
/// too-long name just gets shortened rather than failing the fusion.
pub const MAX_CUSTOM_NAME_LEN: usize = 12;

/// How many characters the `standing_on` row will take, descriptive clause
/// and key-prompt suffix together.
///
/// That row is centred, drawn at `Metrics::font_size` and **unwrapped** —
/// nothing clips it, so an over-long line runs off the pane rather than
/// eliding. 48 leaves headroom over the longest literal the row carried
/// before the bank existed ("Rotten substrate  — moving on costs", 35) while
/// still keeping the clause a phrase rather than a sentence.
///
/// Proved in pixels by `crates/gui`'s
/// `the_longest_underfoot_line_fits_the_stack_pane`, at the narrowest
/// window size the UI supports. A number asserted in one place and repeated
/// in a doc comment somewhere else is how this measurement rots.
pub const MAX_UNDERFOOT_LINE: usize = 48;

/// The quantity a zone-portal structure costing `base_qty` of an item
/// charges at `zone`.
pub(crate) fn zone_portal_cost(base_qty: u32, zone: u32) -> u32 {
    base_qty + base_qty * ZONE_PORTAL_COST_GROWTH_PERCENT * zone.saturating_sub(1) / 100
}

/// `StructureDef::id` of the one structure `Game::place_structure` will
/// let you deploy before any other — everything else requires a Home
/// already standing somewhere. Also what pins the build menu's ordering
/// (see `StructureDb::all`).
const HOME_STRUCTURE_ID: &str = "home";

/// The entire public API surface the renderer talks to via app-core. Its
/// methods live in the `game` module, split by topic; the renderer never
/// touches the ECS `World` directly.
pub struct Game {
    world: World,
    schedule: Schedule,
}

#[cfg(test)]
mod tests;
