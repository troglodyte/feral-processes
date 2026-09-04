//! Every `impl Game` block in the engine, split by what the methods do.
//!
//! `Game` itself stays in `lib.rs` — it is the whole public API surface
//! the renderer talks to, and these modules only add inherent methods to
//! it. Nothing here is reachable from outside the crate except through
//! `Game`.

pub(crate) mod achievements;
pub(crate) mod base;
pub(crate) mod base_space;
pub(crate) mod caravan;
pub(crate) mod catalog;
pub(crate) mod combat;
pub(crate) mod combat_damage;
pub(crate) mod combat_enemy;
pub(crate) mod combat_policy;
pub(crate) mod combat_rewards;
pub(crate) mod combat_round;
pub(crate) mod combat_status;
pub(crate) mod combat_teardown;
pub(crate) mod commerce;
pub(crate) mod contracts;
pub(crate) mod crafting;
pub(crate) mod creation;
pub(crate) mod descriptions;
pub(crate) mod environment;
pub(crate) mod field;
pub(crate) mod gear_power;
pub(crate) mod inspection;
pub(crate) mod lifecycle;
pub(crate) mod listen;
pub(crate) mod memories;
pub(crate) mod notify;
pub(crate) mod party;
pub(crate) mod passives;
pub(crate) mod pursuit;
pub(crate) mod refactor;
pub(crate) mod respec;
pub(crate) mod routines;
pub(crate) mod sortie;
pub(crate) mod spawning;
pub(crate) mod stack;
pub(crate) mod stack_features;
pub(crate) mod stack_market;
pub(crate) mod stack_movement;
pub(crate) mod stack_view;
pub(crate) mod talents;
pub(crate) mod taunt;
pub(crate) mod telemetry;
pub(crate) mod throw;
pub(crate) mod trace;
pub(crate) mod trade;
pub(crate) mod turn;
pub(crate) mod unlocks;
pub(crate) mod zone;
