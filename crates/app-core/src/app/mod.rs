//! Every `impl App` block, split by the screen its methods drive.
//!
//! `App` itself stays in `lib.rs` — it is the state the renderer reads
//! to draw a frame, and these modules only add inherent methods to it.

mod battle;
mod building;
mod crafting;
mod input;
mod inspection;
mod inventory;
mod lifecycle;
mod menus;
mod party;
mod playing;
mod progression;
mod routines;
mod trade;
