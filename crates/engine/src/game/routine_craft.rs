//! Routine crafting (ADR 0029): the verbs between the player's pack and a
//! routine in a slot. Crafting spends a Routine Disk and a material for a
//! part; a build spends parts for a routine; installing writes that routine
//! into a slot; uninstalling or losing the holder destroys it.
//!
//! Each verb is gather, decide, apply: the decision is `routine_craft`'s
//! pure `plan_build` and `material`, so the Workbench page previews exactly
//! what these refuse and spend.

use crate::abilities::AbilityId;
use crate::base_ledger::{ConsumeSource, LootSource};
use crate::components::Routines;
use crate::resources::{CraftedRoutines, Research};
use crate::routine_craft::{self, CraftedRoutine, PlanError, Subroutine, Version};
use crate::routine_tree::{capacity_node_id, part_node_id};
use crate::tuning::{
    CRAFT_ID_PREFIX, CRAFT_PART_CAP_MAX, CRAFT_PART_CAP_START, CRAFT_STARTER_NAME,
};
use crate::*;

impl Game {
    /// How many parts a routine may hold now: the start cap raised by one for
    /// each capacity node researched. Derived from `Research`, so there is
    /// no second copy of it to drift.
    pub fn part_cap(&self) -> usize {
        let research = self.world.resource::<Research>();
        ((CRAFT_PART_CAP_START + 1)..=CRAFT_PART_CAP_MAX)
            .take_while(|cap| research.0.contains(&capacity_node_id(*cap)))
            .last()
            .unwrap_or(CRAFT_PART_CAP_START)
    }

    /// Whether `part` has been researched, which is what lets the player
    /// craft it. A part that was extracted can be used without this.
    pub fn subroutine_known(&self, part: &Subroutine) -> bool {
        self.world
            .resource::<Research>()
            .0
            .contains(&part_node_id(part))
    }

    /// How many of `part` the player holds.
    pub fn parts_held(&self, part: &Subroutine) -> u32 {
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&ItemId::part(part)))
            .unwrap_or(0)
    }

    /// Crafts one `part` at the Workbench for one Routine Disk and the
    /// part's material (`routine_craft::material`).
    pub fn craft_subroutine(&mut self, part: Subroutine) -> Result<(), String> {
        self.require_workbench_free()?;
        if !self.subroutine_known(&part) {
            return Err("Research that part first.".into());
        }
        let disk = ItemId::from(ids::ROUTINE_DISK);
        let (material, count) = routine_craft::material(&part);
        let player = self.player_entity();
        let (disks, mats) = {
            let inv = self.world.get::<Inventory>(player).ok_or("No pack.")?;
            (inv.count(&disk), inv.count(&material))
        };
        if disks == 0 {
            return Err(format!("You need a blank {}.", self.item_name(&disk)));
        }
        if mats < count {
            return Err(format!(
                "You need {count} {} (you have {mats}).",
                self.item_name(&material)
            ));
        }
        {
            let mut inv = self.world.get_mut::<Inventory>(player).unwrap();
            inv.take(disk.clone(), 1);
            inv.take(material.clone(), count);
        }
        self.note_consumed(&disk, 1, ConsumeSource::Craft);
        self.note_consumed(&material, count, ConsumeSource::Craft);
        self.grant_loot(ItemId::part(&part), 1, LootSource::Etch);
        self.log(format!("You compile a {} part.", part.label()));
        Ok(())
    }

    /// Builds a routine named `name` from `parts`, spending the parts.
    /// The routine waits unslotted (the Built routines page) until
    /// `install_built` writes it into a slot.
    pub fn build_routine(&mut self, name: &str, parts: &[Subroutine]) -> Result<AbilityId, String> {
        self.require_workbench_free()?;
        let cap = self.part_cap();
        let (name, used) = routine_craft::plan_build(name, parts, cap, |p| self.parts_held(p))
            .map_err(|e: PlanError| e.to_string())?;
        let player = self.player_entity();
        for (part, count) in &used {
            let item = ItemId::part(part);
            self.world
                .get_mut::<Inventory>(player)
                .unwrap()
                .take(item.clone(), *count);
            self.note_consumed(&item, *count, ConsumeSource::Craft);
        }
        let id = self.mint_built(&name, parts);
        self.world
            .resource_mut::<CraftedRoutines>()
            .unslotted
            .push(id.clone());
        self.log(format!("You compile {name}."));
        Ok(id)
    }

    /// Writes the unslotted built routine `id` into `entity`'s next free
    /// slot.
    pub fn install_built(&mut self, entity: Entity, id: &str) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.owns_routine_holder(entity) {
            return Err("You don't control that program.".into());
        }
        if !self
            .world
            .resource::<CraftedRoutines>()
            .unslotted
            .iter()
            .any(|u| u == id)
        {
            return Err("You haven't built that routine.".into());
        }
        let def = self
            .world
            .resource::<AbilityDb>()
            .get(id)
            .cloned()
            .ok_or("That routine is gone.")?;
        if entity != self.player_entity() && matches!(def.effect, AbilityEffect::Emulate { .. }) {
            return Err("Only you can emulate.".into());
        }
        let held = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.len())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        if held >= self.routine_slots(entity) {
            return Err("There's no free routine slot — pop one out first.".into());
        }
        self.write_routine(entity, id);
        self.world
            .resource_mut::<CraftedRoutines>()
            .unslotted
            .retain(|u| u != id);
        let holder = self.routine_holder_label(entity);
        self.log(format!("{holder} now runs {}.", def.name));
        Ok(())
    }

    /// Destroys every built routine in `ids`: out of the record and out of
    /// the crafted tier. Ids that are not built (an innate routine) are
    /// ignored, so a leave site can hand over a holder's whole kit.
    pub(crate) fn destroy_built(&mut self, ids: &[AbilityId]) {
        let mut crafted = self.world.resource_mut::<CraftedRoutines>();
        let before = crafted.built.len();
        crafted.built.retain(|r| !ids.contains(&r.id));
        crafted.unslotted.retain(|u| !ids.contains(u));
        if crafted.built.len() != before {
            self.sync_crafted();
        }
    }

    /// A holder is leaving for good: its built routines go with it. A
    /// downed, posted or sortie program keeps them, so only the leave sites
    /// call this.
    pub(crate) fn destroy_built_held_by(&mut self, holder: Entity) {
        let ids = self
            .world
            .get::<Routines>(holder)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        self.destroy_built(&ids);
    }

    /// Hands the player `parts` as items, the one door extraction and boss
    /// drops share.
    pub(crate) fn grant_parts(&mut self, parts: &[Subroutine], source: LootSource) {
        for (part, count) in routine_craft::parts_used(parts) {
            self.grant_loot(ItemId::part(&part), count, source);
        }
    }

    /// Rebuilds `AbilityDb`'s crafted tier from the surviving routines, so
    /// no def outlives its routine.
    pub(crate) fn sync_crafted(&mut self) {
        let built = self.world.resource::<CraftedRoutines>().built.clone();
        self.world.resource_mut::<AbilityDb>().set_crafted(&built);
    }

    /// Records a new built routine under the next `crafted/<n>` id, which is
    /// never reused.
    fn mint_built(&mut self, name: &str, parts: &[Subroutine]) -> AbilityId {
        let mut crafted = self.world.resource_mut::<CraftedRoutines>();
        crafted.next_id += 1;
        let id = format!("{CRAFT_ID_PREFIX}{}", crafted.next_id);
        crafted.built.push(CraftedRoutine {
            id: id.clone(),
            name: name.to_string(),
            parts: parts.to_vec(),
        });
        self.sync_crafted();
        id
    }

    /// The routine a new run starts with beside Decompile: Heal v1.0, built
    /// and installed, and the Heal part researched so it can be built again.
    pub(crate) fn grant_starting_routine(&mut self) {
        let heal = Subroutine::Core(routine_craft::Core::Heal, Version::V1);
        self.world
            .resource_mut::<Research>()
            .0
            .insert(part_node_id(&heal));
        let id = self.mint_built(CRAFT_STARTER_NAME, &[heal]);
        let player = self.player_entity();
        self.write_routine(player, &id);
    }

    /// The Workbench pages and the verbs behind them share one gate: nothing
    /// mid-fight or after the end, and a bench that extracts routines
    /// standing somewhere.
    fn require_workbench_free(&self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.can_extract_routines() {
            return Err("You need a Workbench standing to compile routines.".into());
        }
        Ok(())
    }
}
