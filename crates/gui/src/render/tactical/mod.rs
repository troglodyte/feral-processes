//! The battle map, drawn into the map pane.
//!
//! **The map screen's grid with a different tile source.** It takes its
//! origin from `tile_origin_px` and its cell size from `map_cell` exactly as
//! the surface map does, so the camera, the zoom and the pane's own inset
//! are the ones already in use; what it does not share is the surface map's
//! loop, because a battle map has no biomes, no structures, no build sites
//! and no haul marks. The rules the two genuinely share — `ConRead::of` for
//! the con read, `glyph_color` for a body's hue, `Painter::sprite` for its
//! art, `marks::draw_rarity_bar` for a rare-spawn tier's bar — are called
//! here rather than restated.
//!
//! No vignette. The surface map dims with the player's Power because the
//! world is seen through a failing signal; a battle map is a discrete arena
//! the fight is lit by, and dimming it would hide the one thing the screen
//! exists to show.

use feral_processes_engine::Game;
use feral_processes_engine::tactical::map::BattleCell;
use feral_processes_engine::tactical::view::{
    PropView, TacticalBody, TacticalView, TamperTag, TurnRow,
};
use feral_processes_engine::world::Biome;

use super::RARITY_BAR_PX;
use super::base::{ConRead, tile_origin_px};
use super::history::history_rows;
use super::hud::layout::strip_inset;
use super::hud::palette;
use super::lighting::{Lighting, Lit};
use super::marks;
use super::marks::draw_rarity_bar;
use super::popup::{
    PopupSize, Row, draw_popup, item_row, push_key_footer, spent_item_row, text_row,
};
use crate::fx::{BOLT_THICKNESS_PX, Fx, cell_centers};
use crate::paint::{Color, Painter, Rect};
use crate::text::Metrics;
use feral_processes_app_core::{Mode, prop_readout, tactical_items_visible};
use feral_processes_engine::EmulationOption;
use feral_processes_engine::LogEntry;
use feral_processes_engine::battle::SpecialOption;

mod bodies;
mod cell_fields;
mod ground;
mod map;
mod panels;
mod strip;
mod washes;

use bodies::*;
use cell_fields::*;
pub(crate) use ground::FALLEN_MARK;
pub(super) use ground::*;
pub(super) use map::*;
pub(super) use panels::*;
pub(super) use strip::*;
pub(super) use washes::*;

#[cfg(test)]
mod tests;
