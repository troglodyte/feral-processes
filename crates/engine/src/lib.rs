pub mod abilities;
pub mod balance_sim;
pub mod battle;
pub mod components;
pub mod difficulty;
pub mod dungeon;
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
    AbilityCooldowns, ActiveBuff, ActiveStatus, BuffKind, CombatBuff, Creature, CustomName,
    Decompiler, Durability, Equipment, EquippedItem, Experience, FusionCount, Glyph, GlyphColor,
    Hostile, Inventory, ItemFusions, NEED_MAX, Needs, Nest, NestGuardian, PassiveProcessor, Perks,
    Player, Position, Potential, ResourceNode, Routines, Stats, StatusEffects, StatusKind,
    Structure, StructureTier, Tamed, Task, TaskKind, Temporary, WanderAi, ZonePortal,
};
use items::{EquipmentSlot, EquipmentStats, ItemId, ids};
use items_db::ItemDb;
#[cfg(test)]
use items_db::ItemDef;
pub use perks::Perk;
use research::{ResearchDb, ResearchDef};
pub use research::{ResearchId, ResearchRecipe};
use resources::{
    BattleState, BuybackLedger, EffectQueue, GameClock, GameOver, GameRng, MessageLog, Party,
    Platform, PlayerEntity, Research, ZoneLevel, ZoneSpawnPoint,
};
pub use resources::{DifficultyMode, EffectKind, MessageKind, VisualEffect};
use species::{MoveDef, SpeciesDb, SpeciesDef, SpeciesId};
use structures::{StructureDb, StructureDef, StructureId, TradeDef};
pub use views::*;
use world::{Biome, Tile, WorldMap};

/// Longest name a player can give a fused program (see
/// `Game::fuse_companions`) — enforced by truncation, not rejection, so a
/// too-long name just gets shortened rather than failing the fusion.
pub const MAX_CUSTOM_NAME_LEN: usize = 12;

/// The quantity a zone-portal structure costing `base_qty` of an item
/// charges at `zone`. Shared with `balance_sim::ticks_to_afford_portal` so a
/// projection can't drift from the price the game actually charges.
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
