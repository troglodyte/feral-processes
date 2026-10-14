//! Gear wear: how many fights a copy has left before it breaks.
//!
//! The count lives on the copy as `GearCopy::wear`, taken from zero, so a
//! full-condition copy is the neutral value everywhere. This file turns that
//! into the `n/max` figure screens print.

use crate::alerts::AlertKind;
use crate::components::{Reinforcement, Summoned};
use crate::items::{EquipmentSlot, GearCopy};
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

    /// Whether `copy` has run out of fights and gives no bonus.
    pub(crate) fn is_broken(&self, copy: &GearCopy) -> bool {
        self.condition_of(copy).0 == 0
    }

    /// One fight's wear: every equipped copy on each of `wearers` loses one
    /// fight of durability, win or lose.
    ///
    /// **Call it before anything that can strip the gear.** `bench_or_dissolve`
    /// returns a fallen program's gear to the pack, so wear taken afterwards
    /// would miss exactly the copies that just took the hits, and a dissolved
    /// program's gear would come back fresh.
    ///
    /// Reinforcements and summoned forks are skipped: they borrow a body for
    /// one fight and carry no gear of their own. Each step goes through
    /// `swap_worn_copy`, so `Stats` stays exact whatever the new copy is
    /// worth, broken included. A copy that is already broken takes no more.
    pub(crate) fn wear_gear(&mut self, wearers: &[Entity]) {
        for &wearer in wearers {
            if self.world.get::<Summoned>(wearer).is_some()
                || self.world.get::<Reinforcement>(wearer).is_some()
            {
                continue;
            }
            for slot in EquipmentSlot::ALL {
                let Some(worn) = self
                    .world
                    .get::<Equipment>(wearer)
                    .and_then(|e| e.get(slot))
                else {
                    continue;
                };
                if self.is_broken(&worn.copy) {
                    continue;
                }
                let before = self.condition_of(&worn.copy);
                let new = GearCopy {
                    wear: worn.copy.wear + 1,
                    ..worn.copy.clone()
                };
                let after = self.condition_of(&new);
                self.swap_worn_copy(wearer, slot, &worn, new);
                self.report_wear(wearer, slot, &worn.copy, before, after);
            }
        }
    }

    /// The log line and alert a wear step earns: always one when the copy
    /// breaks, and otherwise only on the step that first drops under the
    /// alert threshold.
    fn report_wear(
        &mut self,
        wearer: Entity,
        slot: EquipmentSlot,
        copy: &GearCopy,
        before: (u8, u8),
        after: (u8, u8),
    ) {
        let low = |(left, max): (u8, u8)| {
            u32::from(left) * 100 < u32::from(max) * crate::tuning::GEAR_WORN_ALERT_PERCENT
        };
        let broke = after.0 == 0;
        if !broke && (!low(after) || low(before)) {
            return;
        }
        let owner = if wearer == self.player_entity() {
            "Your".to_string()
        } else {
            format!("{}'s", self.creature_name(wearer).unwrap_or_default())
        };
        let item = self.item_name(&copy.item).to_string();
        let text = if broke {
            format!("{owner} {item} broke.")
        } else {
            format!(
                "{owner} {item} is nearly worn out ({}/{}).",
                after.0, after.1
            )
        };
        self.log(text.clone());
        self.post_alert(
            AlertKind::GearWorn,
            format!("{wearer:?}:{}", slot.label()),
            text,
        );
    }
}
