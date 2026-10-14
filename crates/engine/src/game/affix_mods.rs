//! Fitting and stripping affixes at a Mod Bench.
//!
//! Apply is a recipe paid in materials and limited to researched affixes;
//! remove is free and destroys the affix. Both re-key the copy the way
//! `Game::fuse_item` does, and return the new key because the copy the
//! caller named no longer exists under it.

use crate::affixes::AffixId;
use crate::items::{EquipmentSlot, GearCopy};
use crate::*;

/// The structure apply and remove require. Only its presence matters — the
/// player need not stand at it — so this is `has_structure`'s rule, not a
/// recipe's `requires_structure` field: a copy is not a recipe.
const MOD_BENCH: &str = "mod_bench";

impl Game {
    /// How many affixes `copy` may carry — one, plus one per fusion, and
    /// none for anything a bench does not take (`bench_slot`).
    pub fn affix_slots(&self, copy: &GearCopy) -> u32 {
        if self.bench_slot(&copy.item).is_none() {
            return 0;
        }
        1 + copy.tier
    }

    /// The slot `item` fills if a Mod Bench takes it: Weapon and Armor only.
    /// A Module buys taming or a stat line the affix pool was never priced
    /// against, so it is refused rather than left to an affix that happens
    /// to name no slot.
    fn bench_slot(&self, item: &crate::items::ItemId) -> Option<EquipmentSlot> {
        self.equipment_of(item)
            .map(|(slot, _)| slot)
            .filter(|slot| matches!(slot, EquipmentSlot::Weapon | EquipmentSlot::Armor))
    }

    /// Researched affixes that may go on `copy`'s slot, sorted by id. Not
    /// filtered by free slots or by what the pack can pay: the picker shows
    /// those as refusals, so the player sees what exists.
    pub fn appliable_affixes(&self, copy: &GearCopy) -> Vec<AffixId> {
        let Some(slot) = self.bench_slot(&copy.item) else {
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
        let modded = self.rekey_copy(copy, copy.reaffixed(affixes), slot, worn);
        let msg = format!("You fit {label} to {}.", self.item_name(&modded.item));
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
        let stripped = self.rekey_copy(copy, copy.reaffixed(affixes), slot, worn);
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

    /// Which slot `copy` fits and whether it is the worn one, for a copy the
    /// Mod Bench takes.
    fn locate_copy(
        &self,
        copy: &GearCopy,
    ) -> Result<(EquipmentSlot, Option<EquippedItem>), String> {
        let Some(slot) = self.bench_slot(&copy.item) else {
            return Err(format!("{} can't be modified.", self.item_name(&copy.item)));
        };
        self.locate_in_slot(copy, slot)
    }

    /// Whether the player wears `copy` in `slot`, and that they hold it at
    /// all. The worn copy wins a tie with an identical carried one,
    /// `fuse_copy`'s rule. The slot is the caller's because what each bench
    /// accepts differs: the Mod Bench refuses a Module, the Refit Bench does
    /// not.
    pub(crate) fn locate_in_slot(
        &self,
        copy: &GearCopy,
        slot: EquipmentSlot,
    ) -> Result<(EquipmentSlot, Option<EquippedItem>), String> {
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

    /// Replaces `copy` with `new`, wherever it is. Through
    /// `take_copies`/`add_copies` when carried, because changing a property
    /// can make a copy plain (or not) and change its store; a worn one is
    /// swapped in its slot by `swap_worn_copy`.
    pub(crate) fn rekey_copy(
        &mut self,
        copy: &GearCopy,
        new: GearCopy,
        slot: EquipmentSlot,
        worn: Option<EquippedItem>,
    ) -> GearCopy {
        let Some(worn) = worn else {
            self.take_copies(copy, 1);
            self.add_copies(&new, 1);
            return new;
        };
        let player = self.player_entity();
        self.swap_worn_copy(player, slot, &worn, new.clone());
        new
    }

    /// Swaps the copy `wearer` has on in `slot` for `new` in place, keeping
    /// its gear level. The `-1`/`+1` delta pair is `fuse_copy`'s, so `Stats`
    /// ends exact however the copy's worth changed; wear, mods and refits
    /// all go through here rather than writing the slot themselves.
    pub(crate) fn swap_worn_copy(
        &mut self,
        wearer: Entity,
        slot: EquipmentSlot,
        old: &EquippedItem,
        new: GearCopy,
    ) {
        let promoted = EquippedItem {
            copy: new,
            level: old.level,
        };
        if let Some(mods) = self.worn_bonus(old) {
            self.apply_equipment_delta(wearer, mods, -1);
        }
        if let Some(mods) = self.worn_bonus(&promoted) {
            self.apply_equipment_delta(wearer, mods, 1);
        }
        *self
            .world
            .get_mut::<Equipment>(wearer)
            .unwrap()
            .slot_mut(slot) = Some(promoted);
    }
}
