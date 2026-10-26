//! The decision half of a build: what a design spends, and which parts the
//! player lacks. Pure, so the Forge preview and `Game::build_design` share
//! one answer.

use std::collections::BTreeMap;

use super::weapon::{self, WeaponPart};
use super::{FitError, GearCraftRefusal, GearDesign, GearPart};
use crate::affixes::{AffixDb, AffixId};
use crate::items::ItemId;

/// What building one copy of a design spends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPlan {
    /// Each part used and how many.
    pub parts: Vec<(WeaponPart, u32)>,
    /// The frame's material bill.
    pub bill: (ItemId, u32),
}

/// The parts of `design` with how many each uses, in part order.
pub fn parts_used(design: &GearDesign) -> Vec<(WeaponPart, u32)> {
    let mut used: BTreeMap<WeaponPart, u32> = BTreeMap::new();
    for part in design.weapon_parts() {
        *used.entry(part).or_default() += 1;
    }
    used.into_iter().collect()
}

/// What `design` costs to build against `held` parts, or the parts it lacks.
pub fn plan_build(
    design: &GearDesign,
    held: impl Fn(WeaponPart) -> u32,
) -> Result<BuildPlan, GearCraftRefusal> {
    let (frame, version) = design
        .frame()
        .ok_or(GearCraftRefusal::Fit(FitError::NoFrame))?;
    let parts = parts_used(design);
    let missing: Vec<(WeaponPart, u32)> = parts
        .iter()
        .filter_map(|(part, need)| {
            let have = held(*part);
            (have < *need).then_some((*part, need - have))
        })
        .collect();
    if !missing.is_empty() {
        return Err(GearCraftRefusal::MissingParts(missing));
    }
    Ok(BuildPlan {
        parts,
        bill: weapon::frame_bill(frame, version),
    })
}

/// Whether `parts` under `name` is a design the player may save: a name,
/// a legal fit under `cap`, only parts they may use, and only affixes they
/// know. Returns the trimmed name.
pub fn check_design(
    name: &str,
    parts: &[GearPart],
    cap: usize,
    usable: impl Fn(WeaponPart) -> bool,
    affix_known: impl Fn(&AffixId) -> bool,
    affixes: &AffixDb,
) -> Result<String, GearCraftRefusal> {
    let name = super::check_name(name)?;
    let design = GearDesign {
        id: 0,
        name: name.clone(),
        slot: crate::items::EquipmentSlot::Weapon,
        parts: parts.to_vec(),
    };
    for part in design.weapon_parts() {
        if !usable(part) {
            return Err(GearCraftRefusal::PartNotResearched(part));
        }
    }
    for id in design.affixes() {
        if !affix_known(id) {
            return Err(GearCraftRefusal::AffixNotKnown(id.clone()));
        }
    }
    weapon::check(&design.weapon_parts(), design.affixes().len(), cap)
        .map_err(GearCraftRefusal::Fit)?;
    super::compile(&design, affixes).map_err(GearCraftRefusal::Compile)?;
    Ok(name)
}
