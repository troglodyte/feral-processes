//! What building a routine from the parts in hand asks for: the pure half
//! of `Game::build_routine` and of the Workbench's build page, so the page
//! previews exactly what the build will spend and refuse.

use std::collections::BTreeMap;

use super::catalogue::{
    AreaKind, BuildError, Core, RangeKind, StatusKind, Subroutine, fits, material, validate_build,
};
use crate::damage::DamageType;
use crate::ids;
use crate::items::ItemId;
use crate::tuning::{CRAFT_NAME_MAX, CRAFT_NAME_MIN};
use crate::views::{BuildSlotRow, PartOption};

/// Why a build cannot go ahead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    Name(String),
    Build(BuildError),
    /// Parts the build is short of that cannot be crafted, not being
    /// researched.
    Unresearched(Vec<Subroutine>),
    /// The first item crafting the shortfall needs more of than is in
    /// reach: the item, how many are held, how many are needed. It carries
    /// an id, not a name, so `Game` words it.
    Short {
        item: ItemId,
        have: u32,
        need: u32,
    },
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::Name(why) => f.write_str(why),
            PlanError::Build(e) => e.fmt(f),
            PlanError::Unresearched(parts) => {
                let names: Vec<String> = parts.iter().map(Subroutine::label).collect();
                write!(f, "Research {} first.", names.join(", "))
            }
            PlanError::Short { item, have, need } => {
                write!(f, "You need {need} {} (you have {have}).", item.0)
            }
        }
    }
}

/// The player's name for a routine, trimmed, or why it will not do.
pub fn check_name(name: &str) -> Result<String, PlanError> {
    let name = name.trim();
    let chars = name.chars().count();
    if chars < CRAFT_NAME_MIN {
        return Err(PlanError::Name("Give the routine a name.".into()));
    }
    if chars > CRAFT_NAME_MAX {
        return Err(PlanError::Name(format!(
            "A routine's name is at most {CRAFT_NAME_MAX} characters."
        )));
    }
    Ok(name.to_string())
}

/// The parts `parts` uses, each with how many, in part order.
pub fn parts_used(parts: &[Subroutine]) -> Vec<(Subroutine, u32)> {
    let mut used: BTreeMap<Subroutine, u32> = BTreeMap::new();
    for part in parts {
        *used.entry(*part).or_default() += 1;
    }
    used.into_iter().collect()
}

/// What `parts` is short of, given what `held` says the player has: empty
/// when the build can be paid.
pub fn parts_missing(
    parts: &[Subroutine],
    held: impl Fn(&Subroutine) -> u32,
) -> Vec<(Subroutine, u32)> {
    parts_used(parts)
        .into_iter()
        .filter_map(|(part, need)| {
            let have = held(&part);
            (have < need).then(|| (part, need - have))
        })
        .collect()
}

/// What a build's parts come to: the held parts it spends, the shortfall it
/// crafts, the shortfall it cannot craft, and what crafting costs.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct BuildNeeds {
    pub spend: Vec<(Subroutine, u32)>,
    pub craft: Vec<(Subroutine, u32)>,
    pub unresearched: Vec<Subroutine>,
    /// `craft_bill` of `craft`.
    pub bill: Vec<(ItemId, u32)>,
}

/// Splits `parts` into what is held and what must be crafted. Held parts are
/// spent first; a shortfall is crafted only where it is `known`.
pub fn build_needs(
    parts: &[Subroutine],
    held: impl Fn(&Subroutine) -> u32,
    known: impl Fn(&Subroutine) -> bool,
) -> BuildNeeds {
    needs_of(parts, &held, &known)
}

// Takes `&dyn Fn`, not `impl Fn`, on evidence: rustc 1.95 at the dev
// profile's `opt-level = 1` miscompiled the generic body with `Game`'s two
// closures inlined (a held count of 0 came back as a craft of ~30000, or
// 2^31), while `opt-level = 0` was right. Keep the closures out of line.
fn needs_of(
    parts: &[Subroutine],
    held: &dyn Fn(&Subroutine) -> u32,
    known: &dyn Fn(&Subroutine) -> bool,
) -> BuildNeeds {
    let mut needs = BuildNeeds::default();
    for (part, need) in parts_used(parts) {
        let short = need.saturating_sub(held(&part));
        if short < need {
            needs.spend.push((part, need - short));
        }
        if short > 0 {
            if known(&part) {
                needs.craft.push((part, short));
            } else {
                needs.unresearched.push(part);
            }
        }
    }
    needs.bill = craft_bill(&needs.craft);
    needs
}

/// What crafting `craft` costs, summed: one Routine Disk a part first, then
/// each material once, however many parts share it.
pub fn craft_bill(craft: &[(Subroutine, u32)]) -> Vec<(ItemId, u32)> {
    let disk = ItemId::from(ids::ROUTINE_DISK);
    let mut bill: Vec<(ItemId, u32)> = Vec::new();
    let mut add = |item: ItemId, qty: u32| match bill.iter_mut().find(|(i, _)| *i == item) {
        Some((_, n)) => *n += qty,
        None => bill.push((item, qty)),
    };
    for (_, count) in craft {
        add(disk.clone(), *count);
    }
    for (part, count) in craft {
        let (item, per) = material(part);
        add(item, per * count);
    }
    bill
}

/// A build that will go through: its trimmed name and what it uses.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BuildPlan {
    pub name: String,
    pub needs: BuildNeeds,
}

/// Whether the build is allowed and payable, crafting the shortfall, and
/// what it spends if so. Refuses the name, then the build, then a shortfall
/// that cannot be crafted, then the first item short in reach.
pub fn plan_build(
    name: &str,
    parts: &[Subroutine],
    cap: usize,
    held: impl Fn(&Subroutine) -> u32,
    known: impl Fn(&Subroutine) -> bool,
    in_reach: impl Fn(&ItemId) -> u32,
) -> Result<BuildPlan, PlanError> {
    let name = check_name(name)?;
    validate_build(parts, cap).map_err(PlanError::Build)?;
    let needs = build_needs(parts, held, known);
    if !needs.unresearched.is_empty() {
        return Err(PlanError::Unresearched(needs.unresearched));
    }
    if let Some((item, need)) = needs.bill.iter().find(|(i, n)| in_reach(i) < *n) {
        return Err(PlanError::Short {
            item: item.clone(),
            have: in_reach(item),
            need: *need,
        });
    }
    Ok(BuildPlan { name, needs })
}

/// The build page's rows, in page order.
pub const BUILD_SLOTS: [&str; 6] = ["Core", "Range", "Area", "Timing", "Damage type", "Status"];

/// The Core `parts` holds, if any.
fn core_of(parts: &[Subroutine]) -> Option<Core> {
    parts.iter().find_map(|p| match p {
        Subroutine::Core(core, _) => Some(*core),
        _ => None,
    })
}

/// A part of `slot` standing in for the whole slot when asking whether the
/// Core takes it at all.
fn probe(core: Core, slot: &str) -> Subroutine {
    match slot {
        "Range" => Subroutine::Range(RangeKind::Near),
        "Area" => Subroutine::Area(core.areas().first().copied().unwrap_or(AreaKind::Group)),
        "Timing" => Subroutine::Trickle,
        "Damage type" => Subroutine::DamageType(DamageType::Thermal),
        _ => Subroutine::Status(StatusKind::Bleed),
    }
}

/// One row per part slot for the build `parts` describes. A row the Core
/// takes nothing in carries the reason in place of a part.
pub fn build_rows(parts: &[Subroutine]) -> Vec<BuildSlotRow> {
    let core = core_of(parts);
    BUILD_SLOTS
        .iter()
        .map(|slot| {
            let chosen: Vec<Subroutine> = parts
                .iter()
                .copied()
                .filter(|p| p.slot() == *slot)
                .collect();
            let unavailable = match (core, *slot) {
                (_, "Core") => None,
                (None, _) => Some("Pick a Core first."),
                (Some(core), slot) => fits(core, &probe(core, slot)).err(),
            };
            BuildSlotRow {
                slot,
                chosen,
                unavailable,
            }
        })
        .collect()
}

/// The parts a slot's list offers for the build `parts` describes: every
/// part of that slot the Core takes that is researched or held, each with
/// how many the player holds. Empty for a slot the Core takes nothing in.
/// A held part needs no research: one from an extraction or a boss drop is
/// usable as it is, and research is what lets you craft more.
pub fn part_options(
    slot: &str,
    parts: &[Subroutine],
    known: impl Fn(&Subroutine) -> bool,
    held: impl Fn(&Subroutine) -> u32,
) -> Vec<PartOption> {
    let core = core_of(parts);
    Subroutine::all()
        .into_iter()
        .filter(|p| p.slot() == slot && (known(p) || held(p) > 0))
        .filter(|p| match (slot, core) {
            ("Core", _) => true,
            (_, Some(core)) => fits(core, p).is_ok(),
            (_, None) => false,
        })
        .map(|part| PartOption {
            label: part.label(),
            held: held(&part),
            craft_cost: known(&part).then(|| material(&part)),
            part,
        })
        .collect()
}
