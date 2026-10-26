//! Player gear built from designs (ADR 0031). A design is a slot plus parts;
//! `compile` turns it into an ordinary `ItemDef`. Nothing here touches
//! `Game`. Only weapons have parts today: armour and modules add a part enum
//! and a `compile` arm, and the `slot` match below refuses them until then.
//!
//! - `weapon`: the catalogue, fit rules, `compile` and `decompose`.
//! - `power`: `design_power`, the rating the ceiling and dominance tests use.

// Rated by the censuses alone until the Game door (phase 2) prices a build.
#[cfg(test)]
mod power;
pub mod weapon;

use serde::{Deserialize, Serialize};

use crate::affixes::{AffixDb, AffixId};
use crate::items::EquipmentSlot;
use crate::items_db::ItemDef;

#[cfg(test)]
pub(crate) use power::design_power;
pub use weapon::{FitError, WeaponPart};

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
