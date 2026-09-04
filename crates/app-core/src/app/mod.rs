//! Every `impl App` block, split by the screen its methods drive.
//!
//! `App` itself stays in `lib.rs` — it is the state the renderer reads
//! to draw a frame, and these modules only add inherent methods to it.

pub(crate) mod arena;
mod basket;
mod battle;
pub(crate) mod building;
pub(crate) mod canvas_editor;
pub(crate) mod caravan;
pub(crate) mod contracts;
mod crafting;
pub(crate) mod creation;
pub(crate) mod dev_console;
mod excavate;
mod extraction;
mod field;
pub(crate) mod group_menu;
pub(crate) mod icon_editor;
pub(crate) mod input;
mod inspection;
mod inventory;
mod lifecycle;
mod menus;
mod party;
mod playing;
mod progression;
mod routines;
pub(crate) mod sprite_forge;
pub(crate) mod stack_market;
pub(crate) mod telemetry;
pub(crate) mod trade;
mod transfer;
