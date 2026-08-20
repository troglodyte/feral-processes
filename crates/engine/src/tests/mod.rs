//! The engine's unit tests, split by the subsystem they exercise.
//!
//! These live under `src/` rather than `tests/` because they reach past
//! `Game`'s public API into components and resources to build fixtures.

pub(crate) mod support;

mod achievements;
mod affixes;
mod assets;
mod base_grid;
mod base_space;
mod battle_timeline;
mod building;
mod catalog;
mod chains;
mod collect;
mod combat;
mod combat_abilities;
mod combat_packs;
mod combat_rewards;
mod combat_specials;
mod combat_status;
mod combat_targeting;
mod contracts;
mod crafting;
mod descriptions;
mod easter_eggs;
mod environment;
mod equipment;
mod exclusive_routines;
mod field;
mod gear_passives;
mod hauling;
mod inspection;
mod level_up;
mod listen;
mod message_log;
mod nemesis;
mod party;
mod perks;
mod policy;
mod power;
mod raids;
mod refactor;
mod research;
mod routines;
mod sectors;
mod spawning;
mod stack;
mod stack_movement;
mod talents;
mod taming;
mod taunt;
mod telemetry;
mod throw;
mod trade;
mod turn;
mod wielded;
mod wild_density_probe;
mod work_orders;
mod zone;
