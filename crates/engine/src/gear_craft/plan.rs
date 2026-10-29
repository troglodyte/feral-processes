//! The decision half of a build: what a design spends, and which parts the
//! player lacks. Pure, so the Build weapon preview and
//! `Game::compile_weapon` share one answer.

use std::collections::BTreeMap;

use super::weapon::{self, WeaponPart};
use super::{FitError, GearCraftRefusal, GearDesign, GearPart};
use crate::affixes::{AffixDb, AffixId};
use crate::items::ItemId;

/// Parts with how many.
pub type PartCounts = Vec<(WeaponPart, u32)>;

/// What a build reads: the design and three views of what is in reach.
/// `dyn`, not generic, because the sibling `routine_craft` split miscompiled
/// at `opt-level = 1` with `Game`'s closures inlined.
pub struct BuildInput<'a> {
    pub design: &'a GearDesign,
    /// Parts in reach.
    pub held: &'a dyn Fn(WeaponPart) -> u32,
    /// Whether the part's research is done (not merely held).
    pub known: &'a dyn Fn(WeaponPart) -> bool,
    /// Materials in reach.
    pub in_reach: &'a dyn Fn(&ItemId) -> u32,
}

/// What building one copy of a design spends.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPlan {
    /// Held parts the build uses.
    pub spend: Vec<(WeaponPart, u32)>,
    /// The shortfall, crafted first.
    pub craft: Vec<(WeaponPart, u32)>,
    /// The frame's bill plus crafting `craft`, summed per item.
    pub bill: Vec<(ItemId, u32)>,
}

/// The parts of `design` with how many each uses, in part order.
pub fn parts_used(design: &GearDesign) -> Vec<(WeaponPart, u32)> {
    let mut used: BTreeMap<WeaponPart, u32> = BTreeMap::new();
    for part in design.weapon_parts() {
        *used.entry(part).or_default() += 1;
    }
    used.into_iter().collect()
}

/// Splits what `design` uses into held parts spent and a shortfall crafted.
pub fn split_held(
    design: &GearDesign,
    held: &dyn Fn(WeaponPart) -> u32,
) -> (PartCounts, PartCounts) {
    let mut spend = Vec::new();
    let mut craft = Vec::new();
    for (part, need) in parts_used(design) {
        // `min`, not a saturating subtraction: at `opt-level = 1` the
        // subtraction form came back as a spend of ~22000.
        let use_held = held(part).min(need);
        let short = need - use_held;
        if use_held > 0 {
            spend.push((part, use_held));
        }
        if short > 0 {
            craft.push((part, short));
        }
    }
    (spend, craft)
}

/// The frame's bill plus each crafted part's, one line per item. Empty of
/// the frame's share when the design has no single Frame.
pub fn material_bill(design: &GearDesign, craft: &[(WeaponPart, u32)]) -> Vec<(ItemId, u32)> {
    let mut bill: Vec<(ItemId, u32)> = Vec::new();
    let mut add = |item: ItemId, qty: u32| match bill.iter_mut().find(|(i, _)| *i == item) {
        Some((_, n)) => *n += qty,
        None => bill.push((item, qty)),
    };
    if let Some((frame, version)) = design.frame() {
        let (item, qty) = weapon::frame_bill(frame, version);
        add(item, qty);
    }
    for (part, count) in craft {
        let (item, per) = weapon::part_bill(*part);
        add(item, per * count);
    }
    bill
}

/// The crafted shortfall that can never be crafted: a Proc (salvage only)
/// or a part not yet researched.
pub fn uncraftable(
    craft: &[(WeaponPart, u32)],
    known: &dyn Fn(WeaponPart) -> bool,
) -> Vec<WeaponPart> {
    craft
        .iter()
        .map(|(part, _)| *part)
        .filter(|part| matches!(part, WeaponPart::Proc(_)) || !known(*part))
        .collect()
}

/// Whether `input.design` can be built and what it spends, crafting any
/// missing part. Refuses no Frame, then a Proc to craft, then an
/// unresearched part to craft, then the first item short in reach.
pub fn plan_build(input: &BuildInput) -> Result<BuildPlan, GearCraftRefusal> {
    if input.design.frame().is_none() {
        return Err(GearCraftRefusal::Fit(FitError::NoFrame));
    }
    let (spend, craft) = split_held(input.design, input.held);
    let blocked = uncraftable(&craft, input.known);
    if let Some(proc) = blocked.iter().find(|p| matches!(p, WeaponPart::Proc(_))) {
        return Err(GearCraftRefusal::SalvageOnly(*proc));
    }
    if let Some(part) = blocked.first() {
        return Err(GearCraftRefusal::PartNotResearched(*part));
    }
    let bill = material_bill(input.design, &craft);
    if let Some((item, need)) = bill.iter().find(|(i, n)| (input.in_reach)(i) < *n) {
        return Err(GearCraftRefusal::ShortOf {
            item: item.clone(),
            need: *need,
            held: (input.in_reach)(item),
        });
    }
    Ok(BuildPlan { spend, craft, bill })
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
