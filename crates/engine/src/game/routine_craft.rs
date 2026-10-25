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
use crate::views::{BuildPreview, BuildSlotRow, BuiltRoutineRow, PartOption, SubroutineRow};
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

    /// Blank Routine Disks in cargo — what crafting a part spends.
    pub fn blank_disks_held(&self) -> u32 {
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&ItemId::from(ids::ROUTINE_DISK)))
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
        self.grant_loot(ItemId::part(&part), 1, LootSource::Craft);
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
    pub(crate) fn mint_built(&mut self, name: &str, parts: &[Subroutine]) -> AbilityId {
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

    /// The build page's rows for the build `parts` describes.
    pub fn build_rows(&self, parts: &[Subroutine]) -> Vec<BuildSlotRow> {
        routine_craft::build_rows(parts)
    }

    /// The list `slot`'s row opens: the parts that fit the build and are
    /// researched or held, each with how many are held.
    pub fn build_part_options(&self, slot: &str, parts: &[Subroutine]) -> Vec<PartOption> {
        routine_craft::part_options(
            slot,
            parts,
            |p| self.subroutine_known(p),
            |p| self.parts_held(p),
        )
    }

    /// The build page's preview: exactly what `build_routine` would spend
    /// and refuse for this name and these parts.
    pub fn build_preview(&self, name: &str, parts: &[Subroutine]) -> BuildPreview {
        let cap = self.part_cap();
        // `compile` is only defined for a build that fits its Core, so a
        // half-made one previews no numbers rather than a wrong one.
        let (description, power_cost, cooldown) =
            if routine_craft::validate_build(parts, CRAFT_PART_CAP_MAX).is_ok() {
                let def = routine_craft::compile(&CraftedRoutine {
                    id: String::new(),
                    name: name.to_string(),
                    parts: parts.to_vec(),
                });
                (
                    def.description.clone(),
                    crate::abilities::routine_power_cost(&def),
                    def.cooldown,
                )
            } else {
                (String::new(), 0.0, 0)
            };
        let label = |list: Vec<(Subroutine, u32)>| -> Vec<(String, u32)> {
            list.into_iter().map(|(p, n)| (p.label(), n)).collect()
        };
        let blocked = routine_craft::plan_build(name, parts, cap, |p| self.parts_held(p))
            .err()
            .map(|e| e.to_string())
            .or_else(|| self.require_workbench_free().err());
        BuildPreview {
            description,
            power_cost,
            cooldown,
            parts_used: label(routine_craft::parts_used(parts)),
            parts_missing: label(routine_craft::parts_missing(parts, |p| self.parts_held(p))),
            part_count: parts.len(),
            part_cap: cap,
            blocked,
        }
    }

    /// The Subroutines page: every part with its slot, how many are held,
    /// what a craft costs and whether research has unlocked it.
    pub fn subroutine_rows(&self) -> Vec<SubroutineRow> {
        Subroutine::all()
            .into_iter()
            .map(|part| {
                let (material, material_count) = routine_craft::material(&part);
                let known = self.subroutine_known(&part);
                SubroutineRow {
                    label: part.label(),
                    slot: part.slot(),
                    held: self.parts_held(&part),
                    material: self.item_name(&material).to_string(),
                    material_count,
                    known,
                    locked: (!known).then_some("locked · research to unlock"),
                    part,
                }
            })
            .collect()
    }

    /// The Built routines page: routines waiting for a slot.
    pub fn built_routine_rows(&self) -> Vec<BuiltRoutineRow> {
        let crafted = self.world.resource::<CraftedRoutines>();
        let db = self.world.resource::<AbilityDb>();
        crafted
            .unslotted
            .iter()
            .filter_map(|id| {
                let record = crafted.built.iter().find(|r| r.id == *id)?;
                Some(BuiltRoutineRow {
                    id: id.clone(),
                    name: record.name.clone(),
                    description: db.get(id)?.description.clone(),
                    parts: record.parts.iter().map(Subroutine::label).collect(),
                })
            })
            .collect()
    }

    /// Whether `structure` is a bench whose hub carries the routine-crafting
    /// pages: the `extracts_routines` flag, with its research done.
    pub(crate) fn is_routine_bench(&self, structure: Entity) -> bool {
        self.world
            .get::<Structure>(structure)
            .and_then(|s| self.world.resource::<StructureDb>().get(&s.kind))
            .is_some_and(|def| def.extracts_routines && self.extraction_researched(def))
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
