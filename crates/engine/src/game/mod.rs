//! Every `impl Game` block in the engine, split by what the methods do.
//!
//! `Game` itself stays in `lib.rs` — it is the whole public API surface
//! the renderer talks to, and these modules only add inherent methods to
//! it. Nothing here is reachable from outside the crate except through
//! `Game`.

pub(crate) mod building;
pub(crate) mod catalog;
pub(crate) mod combat;
pub(crate) mod combat_damage;
pub(crate) mod combat_enemy;
pub(crate) mod combat_rewards;
pub(crate) mod combat_round;
pub(crate) mod combat_status;
pub(crate) mod combat_teardown;
pub(crate) mod crafting;
pub(crate) mod dungeon;
pub(crate) mod field;
pub(crate) mod inspection;
pub(crate) mod lifecycle;
pub(crate) mod party;
pub(crate) mod routines;
pub(crate) mod spawning;
pub(crate) mod trade;
pub(crate) mod turn;
pub(crate) mod unlocks;
pub(crate) mod upkeep;
pub(crate) mod zone;
