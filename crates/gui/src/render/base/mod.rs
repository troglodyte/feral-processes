//! The map screen: terrain, entities, effects, and the status panel beside them.

use super::marks::*;
use super::outposts::draw_outpost_marks;
use super::stack::draw_stack;
use super::terrain::*;
use super::wildlife::draw_wildlife;
use super::*;
use feral_processes_engine::TravelGoal;
use feral_processes_engine::views::DigMark;
use feral_processes_engine::views::FloorLook;
use feral_processes_engine::views::PinMark;
use feral_processes_engine::views::drawn_on_surface_map;
use feral_processes_engine::views::{FarmTileState, FarmTileView};
use feral_processes_engine::views::{Sight, shown_at};

mod click_layout;
mod overlay_style;
mod plan_overlays;
mod playing;
mod surface_map;

pub(super) use click_layout::*;
pub(crate) use click_layout::{MapClickLayout, tile_at_px};
pub(super) use overlay_style::*;
pub(crate) use overlay_style::{CUTTING_OUTLINE, floor_item_mark};
pub(super) use plan_overlays::*;
pub(super) use playing::*;
use surface_map::*;

#[cfg(test)]
mod tests;
