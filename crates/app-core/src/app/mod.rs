//! Every `impl App` block, split by the screen its methods drive.
//!
//! `App` itself stays in `lib.rs` — it is the state the renderer reads
//! to draw a frame, and these modules only add inherent methods to it.

pub(crate) mod arena;
mod basket;
mod battle;
mod breeding;
pub(crate) mod building;
pub(crate) mod canvas_editor;
pub(crate) mod caravan;
pub(crate) mod contracts;
mod crafting;
pub(crate) mod creation;
pub(crate) mod depot_filter;
pub(crate) mod dev_console;
pub(crate) mod dispatch;
mod ending;
pub(crate) mod excavate;
mod extraction;
mod field;
pub(crate) mod group_menu;
mod hover;
pub(crate) mod icon_editor;
pub(crate) mod input;
mod inspection;
mod inventory;
pub(crate) mod level_up;
mod lifecycle;
mod menus;
mod mod_copy;
pub(crate) mod outposts;
mod party;
mod playing;
mod progression;
pub mod rig_tool;
mod routines;
pub(crate) mod settlement_board;
pub(crate) mod settlement_market;
mod siphon;
pub mod splice_rig;
pub(crate) mod sprite_forge;
pub(crate) mod stack_market;
pub(crate) mod stat_allocation;
mod tactical;
pub(crate) mod telemetry;
mod tools;
pub(crate) mod trade;
mod transfer;
pub(crate) mod travel;
mod world_map;
