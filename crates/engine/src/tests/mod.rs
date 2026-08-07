//! The engine's unit tests, split by the subsystem they exercise.
//!
//! These live under `src/` rather than `tests/` because they reach past
//! `Game`'s public API into components and resources to build fixtures.

pub(crate) mod support;

mod achievements;
mod assets;
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
mod crafting;
mod easter_eggs;
mod equipment;
mod field;
mod hauling;
mod inspection;
mod listen;
mod message_log;
mod party;
mod perks;
mod raids;
mod research;
mod routines;
mod spawning;
mod stack;
mod stack_movement;
mod taming;
mod taunt;
mod throw;
mod trade;
mod turn;
mod wielded;
mod zone;
