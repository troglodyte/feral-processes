//! Fitting and stripping affixes at a Mod Bench.
//!
//! Apply is a recipe paid in materials and limited to researched affixes;
//! remove is free and destroys the affix. Both re-key the copy the way
//! `Game::fuse_item` does, and return the new key because the copy the
//! caller named no longer exists under it.

use std::collections::BTreeMap;

use crate::affixes::AffixId;
use crate::items::{EquipmentSlot, GearCopy, ItemId};
use crate::*;

/// The structure apply and remove require. Only its presence matters — the
/// player need not stand at it — so this is `has_structure`'s rule, not a
/// recipe's `requires_structure` field: a copy is not a recipe.
const MOD_BENCH: &str = "mod_bench";

impl Game {
    /// How many affixes `copy` may carry — one, plus one per fusion.
    pub fn affix_slots(&self, copy: &GearCopy) -> u32 {
        1 + copy.tier
    }

    /// Researched affixes that may go on `copy`'s slot, sorted by id. Not
    /// filtered by free slots or by what the pack can pay: the picker shows
    /// those as refusals, so the player sees what exists.
    pub fn appliable_affixes(&self, copy: &GearCopy) -> Vec<AffixId> {
        let Some((slot, _)) = self.equipment_of(&copy.item) else {
            return Vec::new();
        };
        let mut found: Vec<AffixId> = self
            .world
            .resource::<AffixDb>()
            .all()
            .filter(|d| d.research.is_some() && d.fits(slot))
            .filter(|d| self.affix_researched(&d.id))
            .map(|d| d.id.clone())
            .collect();
        found.sort();
        found
    }

    /// Fits `affix` to `copy`, paying its `apply_cost` from the pack.
    /// Returns the copy's new key and the log line.
    ///
    /// Every refusal sits above the payment, so a refused apply spends
    /// nothing.
    pub fn apply_affix(
        &mut self,
        copy: &GearCopy,
        affix: &AffixId,
    ) -> Result<(GearCopy, String), String> {
        self.modding_allowed()?;
        let (slot, worn) = self.locate_copy(copy)?;
        let (label, cost) = {
            let def = self
                .world
                .resource::<AffixDb>()
                .get(affix)
                .ok_or_else(|| "Unknown affix.".to_string())?;
            let research = def
                .research
                .as_ref()
                .filter(|_| self.affix_researched(affix))
                .ok_or_else(|| format!("{} hasn't been researched.", def.label()))?;
            if !def.fits(slot) {
                return Err(format!(
                    "{} can't be fitted to {}.",
                    def.label(),
                    slot.label()
                ));
            }
            (def.label(), research.apply_cost.clone())
        };
        let slots = self.affix_slots(copy) as usize;
        if copy.affixes.len() >= slots {
            return Err(format!(
                "{} has no free affix slot ({slots} of {slots} used) — strip one first.",
                self.copy_name(copy)
            ));
        }
        self.pay_items(&cost)?;

        let mut affixes = copy.affixes.clone();
        affixes.push(affix.clone());
        let modded = self.rekey_copy(copy, affixes, slot, worn);
        let msg = format!("You fit {label} to {}.", self.copy_name(&modded));
        self.log(msg.clone());
        self.tick();
        Ok((modded, msg))
    }

    /// Strips one `affix` off `copy` for free. Removing one of two
    /// duplicates removes one. Returns the copy's new key and the log line.
    pub fn remove_affix(
        &mut self,
        copy: &GearCopy,
        affix: &AffixId,
    ) -> Result<(GearCopy, String), String> {
        self.modding_allowed()?;
        let (slot, worn) = self.locate_copy(copy)?;
        let Some(position) = copy.affixes.iter().position(|a| a == affix) else {
            return Err(format!(
                "{} doesn't carry that affix.",
                self.copy_name(copy)
            ));
        };
        let label = self
            .world
            .resource::<AffixDb>()
            .get(affix)
            .map_or_else(|| affix.as_str().to_string(), |d| d.label());
        let mut affixes = copy.affixes.clone();
        affixes.remove(position);
        let stripped = self.rekey_copy(copy, affixes, slot, worn);
        let msg = format!("You strip {label} from {}.", self.item_name(&stripped.item));
        self.log(msg.clone());
        self.tick();
        Ok((stripped, msg))
    }

    /// The refusals both doors share: not now, and no Mod Bench.
    fn modding_allowed(&self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.has_structure(MOD_BENCH) {
            return Err("Build a Mod Bench first.".into());
        }
        Ok(())
    }

    /// Which slot `copy` fits and whether it is the worn one. The worn copy
    /// wins a tie with an identical carried one, `fuse_copy`'s rule.
    fn locate_copy(
        &self,
        copy: &GearCopy,
    ) -> Result<(EquipmentSlot, Option<EquippedItem>), String> {
        let Some((slot, _)) = self.equipment_of(&copy.item) else {
            return Err(format!("{} can't be modified.", self.item_name(&copy.item)));
        };
        let player = self.player_entity();
        let worn = self
            .world
            .get::<Equipment>(player)
            .and_then(|e| e.get(slot))
            .filter(|w| &w.copy == copy);
        if worn.is_none() && self.count_copies(copy) == 0 {
            return Err(format!("You don't have {}.", self.copy_name(copy)));
        }
        Ok((slot, worn))
    }

    /// Replaces `copy` with the same copy carrying `affixes`, wherever it
    /// is. Through `take_copies`/`add_copies` when carried, because
    /// removing the last affix can make a copy plain and change its store;
    /// a worn one is swapped in its slot with `fuse_copy`'s delta pair.
    fn rekey_copy(
        &mut self,
        copy: &GearCopy,
        affixes: Vec<AffixId>,
        slot: EquipmentSlot,
        worn: Option<EquippedItem>,
    ) -> GearCopy {
        let new = GearCopy::with_affixes(
            copy.item.clone(),
            copy.rarity,
            copy.tier,
            affixes,
            copy.quality,
        );
        let Some(worn) = worn else {
            self.take_copies(copy, 1);
            self.add_copies(&new, 1);
            return new;
        };
        let player = self.player_entity();
        let promoted = EquippedItem {
            copy: new.clone(),
            level: worn.level,
        };
        if let Some(mods) = self.worn_bonus(&worn) {
            self.apply_equipment_delta(player, mods, -1);
        }
        if let Some(mods) = self.worn_bonus(&promoted) {
            self.apply_equipment_delta(player, mods, 1);
        }
        *self
            .world
            .get_mut::<Equipment>(player)
            .unwrap()
            .slot_mut(slot) = Some(promoted);
        new
    }

    /// Takes `cost` out of the pack, or refuses naming what is short.
    /// Every line is checked before any is taken, and the taking is booked
    /// as craft consumption like `take_hand_craft_unit`'s.
    fn pay_items(&mut self, cost: &[(ItemId, u32)]) -> Result<(), String> {
        let mut need: BTreeMap<&ItemId, u32> = BTreeMap::new();
        for (item, qty) in cost {
            *need.entry(item).or_default() += qty;
        }
        let player = self.player_entity();
        let short: Vec<String> = {
            let inv = self.world.get::<Inventory>(player).unwrap();
            need.iter()
                .filter(|(item, qty)| inv.count(item) < **qty)
                .map(|(item, qty)| {
                    format!("{qty} {} (have {})", self.item_name(item), inv.count(item))
                })
                .collect()
        };
        if !short.is_empty() {
            return Err(format!("Need {}.", short.join(", ")));
        }
        for (item, qty) in need {
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(item.clone(), qty);
            self.note_consumed(item, qty, crate::base_ledger::ConsumeSource::Craft);
        }
        Ok(())
    }
}
