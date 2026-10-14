//! The Refit Bench: pay materials, and a worn copy's condition returns to
//! full. Player-operated and instant, the Mod Bench's pattern — the bench
//! only has to be standing somewhere in the base.

use std::collections::BTreeMap;

use crate::items::{GearCopy, ItemId, ids};
use crate::*;

/// The structure a refit requires. Only its presence matters, as with
/// `affix_mods::MOD_BENCH`.
const REFIT_BENCH: &str = "refit_bench";

impl Game {
    /// Whether a Refit Bench stands anywhere in the base, so a screen can say
    /// so before the player pays attention to a bill they cannot pay.
    pub fn refit_bench_standing(&self) -> bool {
        self.has_structure(REFIT_BENCH)
    }

    /// What refitting `copy` costs, merged per item and sorted by id.
    ///
    /// A craftable item costs `REFIT_RECIPE_SHARE_PERCENT` of its raw recipe,
    /// rounded up per ingredient, plus `REFIT_COILS` Charge Coils. An item
    /// with no recipe only drops, so its bill scales by the copy's rarity
    /// rank instead. Empty for anything that is not worn gear.
    ///
    /// The raw `craftable.cost`, not `craft_recipes`: that one carries the
    /// player's own perk discount and bench surcharge, which are about
    /// compiling, not about mending.
    pub fn refit_cost(&self, copy: &GearCopy) -> Vec<(ItemId, u32)> {
        let Some(def) = self.world.resource::<ItemDb>().get(copy.item.as_str()) else {
            return Vec::new();
        };
        if def.equipment.is_none() {
            return Vec::new();
        }
        let mut bill: BTreeMap<ItemId, u32> = BTreeMap::new();
        match &def.craftable {
            Some(recipe) => {
                for (item, qty) in &recipe.cost {
                    let share = (qty * crate::tuning::REFIT_RECIPE_SHARE_PERCENT).div_ceil(100);
                    *bill.entry(item.clone()).or_default() += share;
                }
            }
            None => {
                *bill.entry(ItemId::from(ids::CORE_FRAGMENT)).or_default() +=
                    crate::tuning::REFIT_DROP_FRAGMENTS_PER_RANK * (copy.rarity.rank() + 1);
            }
        }
        *bill.entry(ItemId::from(ids::CHARGE_COIL)).or_default() += crate::tuning::REFIT_COILS;
        bill.into_iter().collect()
    }

    /// Restores `copy` to full condition, paying `refit_cost` from the
    /// base's stores and then the pack. Returns the copy's new key — it is
    /// plain again, so it may live in a different store — and the log line.
    ///
    /// Every refusal sits above the payment, and the payment itself is
    /// all-or-nothing: each ingredient is checked with `stores_hold` before
    /// any is spent, so a short purse spends nothing.
    pub fn refit_copy(&mut self, copy: &GearCopy) -> Result<(GearCopy, String), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.refit_bench_standing() {
            return Err("Build a Refit Bench first.".into());
        }
        let Some((slot, _)) = self.equipment_of(&copy.item) else {
            return Err(format!("{} can't be refitted.", self.item_name(&copy.item)));
        };
        let (slot, worn) = self.locate_in_slot(copy, slot)?;
        if copy.wear == 0 {
            return Err(format!(
                "{} is already in full condition.",
                self.copy_name(copy)
            ));
        }
        let cost = self.refit_cost(copy);
        let short: Vec<String> = cost
            .iter()
            .filter(|(item, qty)| self.stores_hold(item) < *qty)
            .map(|(item, qty)| {
                format!(
                    "{qty} {} (have {})",
                    self.item_name(item),
                    self.stores_hold(item)
                )
            })
            .collect();
        if !short.is_empty() {
            return Err(format!("Need {}.", short.join(", ")));
        }
        for (item, qty) in &cost {
            let paid = self.spend_from_stores(item, *qty);
            debug_assert!(paid, "stores_hold said the bill was covered");
        }

        let mended = GearCopy {
            wear: 0,
            ..copy.clone()
        };
        let renewed = self.rekey_copy(copy, mended, slot, worn);
        let msg = format!("You refit {}.", self.copy_name(&renewed));
        self.log(msg.clone());
        self.tick();
        Ok((renewed, msg))
    }
}
