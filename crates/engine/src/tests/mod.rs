//! The engine's unit tests, split by the subsystem they exercise.
//!
//! These live under `src/` rather than `tests/` because they reach past
//! `Game`'s public API into components and resources to build fixtures.

mod support;

mod assets;
mod building;
mod catalog;
mod combat;
mod combat_rewards;
mod combat_specials;
mod crafting;
mod equipment;
mod inspection;
mod party;
mod perks;
mod raids;
mod research;
mod spawning;
mod taming;
mod trade;
mod turn;
mod zone;
