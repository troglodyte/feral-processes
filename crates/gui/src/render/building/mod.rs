//! The build, staffing, demolition, upgrade and symlink pickers.

use super::party::companion_rows;
use super::popup::*;
use super::*;
#[cfg(test)]
use feral_processes_app_core::ProgramRole;
use feral_processes_app_core::{
    BuildMenu, BuildRow, PendingBuild, Roster, RosterRow, WorkOrderRow,
};
use feral_processes_engine::components::BuildGoal;
use feral_processes_engine::structures::{StructureDef, StructureId};
use feral_processes_engine::{
    BaseOutputReport, BaseOutputRow, BuildCandidate, BuildEffect, LabourDemand, LineReport,
    OrderState, WorkColumn, WorkRow, WorkSection, WorkTable, program_tier_required,
};

mod build_commit;
mod build_menu;
mod build_program;
mod pin_siphon;
mod staffing;
mod structure_sheet;

pub(super) use build_commit::*;
pub(super) use build_menu::*;
pub(super) use build_program::*;
pub(super) use pin_siphon::*;
pub(super) use staffing::*;
pub(super) use structure_sheet::*;

#[cfg(test)]
mod tests;
