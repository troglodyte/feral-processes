//! Gear wear: how many fights a copy has left before it breaks.
//!
//! The count lives on the copy as `GearCopy::wear`, taken from zero, so a
//! full-condition copy is the neutral value everywhere. This file turns that
//! into the `n/max` figure screens print.

use crate::items::GearCopy;
use crate::*;

impl Game {
    /// Fights `item` survives when worn: its authored `durability`, else
    /// `tuning::GEAR_DURABILITY`.
    pub(crate) fn durability_of(&self, item: &crate::items::ItemId) -> u8 {
        self.world
            .resource::<ItemDb>()
            .get(item.as_str())
            .and_then(|d| d.durability)
            .unwrap_or(crate::tuning::GEAR_DURABILITY)
    }

    /// `(fights left, fights it started with)` for `copy`. The one place
    /// that subtracts wear from durability, so a screen and the break check
    /// cannot disagree about when a copy is spent.
    pub fn condition_of(&self, copy: &GearCopy) -> (u8, u8) {
        let max = self.durability_of(&copy.item);
        (max.saturating_sub(copy.wear), max)
    }
}
