//! What building a routine from the parts in hand asks for: the pure half
//! of `Game::build_routine` and of the Workbench's build page, so the page
//! previews exactly what the build will spend and refuse.

use std::collections::BTreeMap;

use super::catalogue::{BuildError, Subroutine, validate_build};
use crate::tuning::{CRAFT_NAME_MAX, CRAFT_NAME_MIN};

/// Why a build cannot go ahead.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum PlanError {
    Name(&'static str),
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
        return Err(PlanError::Name("Give the routine a name."));
    }
    if chars > CRAFT_NAME_MAX {
        return Err(PlanError::Name(
            "A routine's name is at most 24 characters.",
        ));
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
