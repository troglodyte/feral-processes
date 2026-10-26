//! Player gear built from designs (ADR 0031). A design is a slot plus parts;
//! `compile` turns it into an ordinary `ItemDef`. Nothing here touches
//! `Game`. Only weapons have parts today: armour and modules add a part enum
//! and a `compile` arm, and the `slot` match below refuses them until then.
//!
//! - `weapon`: the catalogue, fit rules, `compile` and `decompose`.
//! - `power`: `design_power`, the rating the ceiling and dominance tests use.

mod plan;
mod power;
pub mod weapon;

use serde::{Deserialize, Serialize};

use crate::affixes::{AffixDb, AffixId};
use crate::items::EquipmentSlot;
use crate::items_db::ItemDef;

pub use plan::{BuildPlan, check_design, parts_used, plan_build};
pub(crate) use power::design_power;
pub use weapon::{FitError, WeaponPart};

/// The research node that raises a design's modifier cap to `cap`.
pub fn capacity_node_id(cap: usize) -> String {
    format!("weapon_capacity/{cap}")
}

#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum GearPart {
    Weapon(WeaponPart),
    Affix(AffixId),
}

/// A saved recipe. Its `ItemDef` is derived every load, never stored.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct GearDesign {
    pub id: u32,
    pub name: String,
    pub slot: EquipmentSlot,
    pub parts: Vec<GearPart>,
}

impl GearDesign {
    /// The weapon parts, in the order they were picked.
    pub fn weapon_parts(&self) -> Vec<WeaponPart> {
        self.parts
            .iter()
            .filter_map(|p| match p {
                GearPart::Weapon(w) => Some(*w),
                GearPart::Affix(_) => None,
            })
            .collect()
    }

    /// The affixes folded into the weapon.
    pub fn affixes(&self) -> Vec<&AffixId> {
        self.parts
            .iter()
            .filter_map(|p| match p {
                GearPart::Affix(id) => Some(id),
                GearPart::Weapon(_) => None,
            })
            .collect()
    }

    /// The frame the design is built on, if it has exactly one.
    pub fn frame(&self) -> Option<(weapon::Frame, crate::routine_craft::Version)> {
        weapon::frame_of(&self.weapon_parts()).ok()
    }
}

/// Where a copy of a design's weapon was found, which is why it cannot be
/// deleted.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DesignHolder {
    /// Plain copies in the pack, or non-plain ones in the gear ledger.
    Pack,
    /// Worn by the player.
    Worn,
    /// Worn by a companion.
    Companion,
    /// On a trader's buyback shelf.
    Buyback,
    /// Set down on a floor or stockpile tile.
    Floor,
}

/// Why a design verb was turned down. Typed per ADR 0027: a screen words it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum GearCraftRefusal {
    /// A battle is running or the run is over.
    Busy,
    /// No Workbench stands.
    NoWorkbench,
    /// A part whose research the player has not done.
    PartNotResearched(WeaponPart),
    /// A material bill the player cannot meet.
    ShortOf {
        item: crate::items::ItemId,
        need: u32,
        held: u32,
    },
    /// Parts the design needs and the player does not hold.
    MissingParts(Vec<(WeaponPart, u32)>),
    NameEmpty,
    NameTooLong {
        max: usize,
    },
    Fit(FitError),
    Compile(CompileError),
    /// An affix neither researched nor salvaged.
    AffixNotKnown(AffixId),
    NoSuchDesign(u32),
    /// A copy of the design still exists.
    InUse(DesignHolder),
}

/// The player's name for a design, trimmed, or why it will not do.
pub fn check_name(name: &str) -> Result<String, GearCraftRefusal> {
    let name = name.trim();
    let chars = name.chars().count();
    if chars < crate::tuning::GEAR_NAME_MIN {
        return Err(GearCraftRefusal::NameEmpty);
    }
    if chars > crate::tuning::GEAR_NAME_MAX {
        return Err(GearCraftRefusal::NameTooLong {
            max: crate::tuning::GEAR_NAME_MAX,
        });
    }
    Ok(name.to_string())
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum CompileError {
    Fit(FitError),
    UnknownAffix(AffixId),
    AffixDoesNotFit(AffixId),
    /// Only weapons have parts yet.
    UnsupportedSlot(EquipmentSlot),
}

pub fn compile(design: &GearDesign, affixes: &AffixDb) -> Result<ItemDef, CompileError> {
    match design.slot {
        EquipmentSlot::Weapon => weapon::compile(design, affixes),
        slot => Err(CompileError::UnsupportedSlot(slot)),
    }
}

#[cfg(test)]
mod tests;
