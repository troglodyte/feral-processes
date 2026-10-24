//! What the Workbench's three routine-crafting pages draw (ADR 0029): the
//! build page's slot rows, option lists and preview, the Subroutines list,
//! and the Built routines list. Every figure is read off the engine, so a
//! page can never promise a part the build would refuse.

use crate::abilities::AbilityId;
use crate::routine_craft::Subroutine;

/// One row of the build page: a part slot and what fills it.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildSlotRow {
    /// "Core", "Range", "Area", "Timing", "Damage type" or "Status".
    pub slot: &'static str,
    /// The parts chosen for this slot: none, one, or (Timing) several
    /// Trickles.
    pub chosen: Vec<Subroutine>,
    /// Why this Core takes nothing here, shown in place of a part. `None`
    /// when the slot is open to the chosen Core.
    pub unavailable: Option<&'static str>,
}

/// One entry of a slot's highlight-and-Enter list.
#[derive(Clone, Debug, PartialEq)]
pub struct PartOption {
    pub part: Subroutine,
    pub label: String,
    /// How many of this part the player holds.
    pub held: u32,
}

/// The live preview under the build rows: what the build reads as, costs,
/// uses and lacks, and the one reason it cannot compile yet.
#[derive(Clone, Debug, PartialEq)]
pub struct BuildPreview {
    /// The generated plain-language description; empty with no Core.
    pub description: String,
    /// What running it costs, through the same `routine_power_cost` a
    /// charge reads.
    pub power_cost: f32,
    /// Rounds before it can run again.
    pub cooldown: u32,
    /// Parts held in the build and how many of each, labelled.
    pub parts_used: Vec<(String, u32)>,
    /// Parts it needs and the player lacks, with how many short.
    pub parts_missing: Vec<(String, u32)>,
    /// Parts in the build and the most a routine may hold now.
    pub part_count: usize,
    pub part_cap: usize,
    /// Why Compile would be refused, or `None` when it would go through.
    pub blocked: Option<String>,
}

/// One row of the Subroutines list.
#[derive(Clone, Debug, PartialEq)]
pub struct SubroutineRow {
    pub part: Subroutine,
    /// "Heal v2.0", "Range: Near".
    pub label: String,
    /// Which build row it fills.
    pub slot: &'static str,
    pub held: u32,
    /// The material a craft spends beside a Routine Disk, and how many.
    pub material: String,
    pub material_count: u32,
    pub known: bool,
    /// "locked · research to unlock" for an unresearched part.
    pub locked: Option<&'static str>,
}

/// One row of the Built routines list: a routine waiting for a slot.
#[derive(Clone, Debug, PartialEq)]
pub struct BuiltRoutineRow {
    pub id: AbilityId,
    pub name: String,
    pub description: String,
    pub parts: Vec<String>,
}
