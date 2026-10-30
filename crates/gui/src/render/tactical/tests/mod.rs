use super::*;
use crate::fx::FrameCues;
use crate::paint::{painted_rect_fill_count, painted_text, with_painter};
use crate::text::ui_metrics;
use feral_processes_engine::world::Biome;
use feral_processes_engine::{DifficultyMode, Game};

mod action_bar;
mod bodies;
mod camera;
mod effects_strip;
mod forecast_props;
mod ground;
mod squads;
mod support;
mod washes;

use support::*;
