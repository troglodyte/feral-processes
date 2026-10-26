//! What fits what: one frame, one part per category, the cap. A pure check
//! returning a typed refusal, so a screen can say why a pick was refused.

use super::catalogue::{Category, Frame, WeaponPart};
use crate::routine_craft::{StatusKind, Version};
use crate::tuning::{GEAR_FRAME, TACTICAL_WEAPON_RANGE_MAX};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FitError {
    NoFrame,
    TwoFrames,
    /// A second part for a category that holds one.
    CategoryTaken(Category),
    /// More modifiers (affixes included) than the design may hold.
    OverCap {
        cap: usize,
    },
    /// Extend on a frame that already swings at the longest range.
    RangeAtMax(Frame),
    /// A stun on a battle map costs no turn, so it is no rider.
    StunRider,
}

/// The frame among `parts`, if exactly one is there.
pub fn frame_of(parts: &[WeaponPart]) -> Result<(Frame, Version), FitError> {
    let mut frames = parts.iter().filter_map(|p| match p {
        WeaponPart::Frame(f, v) => Some((*f, *v)),
        _ => None,
    });
    let first = frames.next().ok_or(FitError::NoFrame)?;
    if frames.next().is_some() {
        return Err(FitError::TwoFrames);
    }
    Ok(first)
}

/// Whether `parts` plus `affixes` more modifiers is a legal design under
/// `cap` modifiers. Rider and Exploit may name the same status.
pub fn check(parts: &[WeaponPart], affixes: usize, cap: usize) -> Result<(), FitError> {
    let (frame, _) = frame_of(parts)?;
    let mut seen: Vec<Category> = Vec::new();
    for part in parts {
        let category = part.category();
        if category == Category::Frame {
            continue;
        }
        if seen.contains(&category) {
            return Err(FitError::CategoryTaken(category));
        }
        seen.push(category);
        match part {
            WeaponPart::Rider(StatusKind::Stun) => return Err(FitError::StunRider),
            WeaponPart::Extend
                if GEAR_FRAME[frame.index()].range + 1 > TACTICAL_WEAPON_RANGE_MAX =>
            {
                return Err(FitError::RangeAtMax(frame));
            }
            _ => {}
        }
    }
    if seen.len() + affixes > cap {
        return Err(FitError::OverCap { cap });
    }
    Ok(())
}

/// Every legal frame-and-modifier set holding at most `cap` modifiers, no
/// affixes. The census every compile and power test walks.
pub fn all_designs_up_to_cap(cap: usize) -> Vec<Vec<WeaponPart>> {
    let frames: Vec<WeaponPart> = WeaponPart::options(Category::Frame);
    let mut sets: Vec<Vec<WeaponPart>> = vec![Vec::new()];
    for category in Category::MODIFIERS {
        let options = WeaponPart::options(category);
        let mut next = Vec::with_capacity(sets.len() * (options.len() + 1));
        for set in &sets {
            next.push(set.clone());
            if set.len() < cap {
                for option in &options {
                    let mut grown = set.clone();
                    grown.push(*option);
                    next.push(grown);
                }
            }
        }
        sets = next;
    }
    let mut designs = Vec::new();
    for frame in frames {
        for set in &sets {
            let mut parts = vec![frame];
            parts.extend(set.iter().copied());
            if check(&parts, 0, cap).is_ok() {
                designs.push(parts);
            }
        }
    }
    designs
}
