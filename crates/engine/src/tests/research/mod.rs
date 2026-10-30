//! The research tree and the recipes and structures it gates.

use crate::DecompileReach;
use crate::base_grid::BaseGrid;
use crate::components::Racked;
use crate::items::DownedProgram;
use crate::tests::support::*;
use crate::views::{PinMark, ResearchReadout};
use crate::*;

mod bills;
mod discovery_and_study;
mod gating;
mod lifecycle;
mod projects;
mod saves_and_stalls;
mod subject_gated;
mod zone_gates;

use bills::*;
use zone_gates::*;
