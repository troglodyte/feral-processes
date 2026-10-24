//! What building a routine from the parts in hand asks for: the pure half
//! of `Game::build_routine` and of the Workbench's build page, so the page
//! previews exactly what the build will spend and refuse.

use std::collections::BTreeMap;

use super::catalogue::{
    AreaKind, BuildError, Core, RangeKind, StatusKind, Subroutine, fits, validate_build,
};
use crate::damage::DamageType;
use crate::tuning::{CRAFT_NAME_MAX, CRAFT_NAME_MIN};
use crate::views::{BuildSlotRow, PartOption};

/// Why a build cannot go ahead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    Name(String),
    Build(BuildError),
    /// The parts the player does not hold enough of, with how many short.
    Missing(Vec<(Subroutine, u32)>),
}

impl std::fmt::Display for PlanError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            PlanError::Name(why) => f.write_str(why),
            PlanError::Build(e) => e.fmt(f),
            PlanError::Missing(short) => {
                let names: Vec<String> = short.iter().map(|(p, _)| p.label()).collect();
                write!(f, "You are missing {}.", names.join(", "))
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

/// Whether the build is allowed and payable; the parts it spends if so.
pub fn plan_build(
    name: &str,
    parts: &[Subroutine],
    cap: usize,
    held: impl Fn(&Subroutine) -> u32,
) -> Result<(String, Vec<(Subroutine, u32)>), PlanError> {
    let name = check_name(name)?;
    validate_build(parts, cap).map_err(PlanError::Build)?;
    let short = parts_missing(parts, held);
    if !short.is_empty() {
        return Err(PlanError::Missing(short));
    }
    Ok((name, parts_used(parts)))
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
/// researched part of that slot the Core takes, each with how many the
/// player holds. Empty for a slot the Core takes nothing in. A part held
/// but not researched stays hidden: the list is what research has opened.
pub fn part_options(
    slot: &str,
    parts: &[Subroutine],
    known: impl Fn(&Subroutine) -> bool,
    held: impl Fn(&Subroutine) -> u32,
) -> Vec<PartOption> {
    let core = core_of(parts);
    Subroutine::all()
        .into_iter()
        .filter(|p| p.slot() == slot && known(p))
        .filter(|p| match (slot, core) {
            ("Core", _) => true,
            (_, Some(core)) => fits(core, p).is_ok(),
            (_, None) => false,
        })
        .map(|part| PartOption {
            label: part.label(),
            held: held(&part),
            part,
        })
        .collect()
}
