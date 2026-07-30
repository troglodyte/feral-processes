//! Installing, removing and inspecting routines — the abilities that occupy
//! a party member's slots. Extraction lives here too.

use crate::components::Routines;
use crate::*;

impl Game {
    /// Whether `item` is a loose routine rather than ordinary cargo.
    pub fn is_routine(&self, item: &ItemId) -> bool {
        self.world
            .resource::<ItemDb>()
            .get(item.as_str())
            .is_some_and(|d| d.routine.is_some())
    }

    /// `id`'s display name, falling back to the raw id if the ability set
    /// doesn't define it (a mod removed since a save referenced it). Every
    /// routine log line resolves through here so none of them read a raw
    /// snake_case id.
    pub(crate) fn ability_display_name(&self, id: &str) -> String {
        self.world
            .resource::<AbilityDb>()
            .get(id)
            .map(|a| a.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    /// `entity`'s slots in menu order, filled and empty alike.
    pub fn routine_view(&self, entity: Entity) -> Vec<RoutineSlotView> {
        let db = self.world.resource::<AbilityDb>();
        let installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        (0..self.routine_slots(entity))
            .map(
                |index| match installed.get(index).and_then(|id| db.get(id)) {
                    Some(def) => RoutineSlotView {
                        index,
                        ability: Some(def.id.clone()),
                        name: def.name.clone(),
                        description: def.description.clone(),
                    },
                    None => RoutineSlotView {
                        index,
                        ability: None,
                        name: "(empty)".to_string(),
                        description: String::new(),
                    },
                },
            )
            .collect()
    }

    /// Builds one `RoutineHolderView` row for `entity`, labelled `name`.
    /// Shared by `routine_holders` (every program you own) and
    /// `Game::field_cast_targets` (just you and your active `Party`) so the
    /// two lists can't describe the same holder's slot count two different
    /// ways.
    pub(crate) fn routine_holder_view(
        &mut self,
        entity: Entity,
        name: String,
    ) -> RoutineHolderView {
        let level = self
            .world
            .get::<Experience>(entity)
            .map(|e| e.level)
            .unwrap_or(1);
        RoutineHolderView {
            entity,
            name,
            level,
            filled: self
                .world
                .get::<Routines>(entity)
                .map(|r| r.0.len())
                .unwrap_or(0),
            slots: self.routine_slots(entity),
        }
    }

    /// You, then every program you own — everyone who has routine slots.
    pub fn routine_holders(&mut self) -> Vec<RoutineHolderView> {
        let player = self.player_entity();
        let mut holders = vec![self.routine_holder_view(player, "You".to_string())];
        for pet in self.owned_pets() {
            holders.push(self.routine_holder_view(pet.entity, pet.name.clone()));
        }
        holders
    }

    /// Loose routines held in inventory, id-sorted so the picker's numbering
    /// is stable between sessions.
    pub fn loose_routines(&self) -> Vec<RoutineItemView> {
        let db = self.world.resource::<ItemDb>();
        let Some(inv) = self.world.get::<Inventory>(self.player_entity()) else {
            return Vec::new();
        };
        let mut rows: Vec<RoutineItemView> = inv
            .items
            .iter()
            .filter(|(_, count)| *count > 0)
            .filter_map(|(item, count)| {
                let def = db.get(item.as_str())?;
                def.routine.as_ref()?;
                Some(RoutineItemView {
                    item: item.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    count: *count,
                })
            })
            .collect();
        rows.sort_by(|a, b| a.item.as_str().cmp(b.item.as_str()));
        rows
    }

    /// Whether `entity` is a routine holder the player actually controls —
    /// themself, or a program `Tamed` to them. A save-loaded wild creature
    /// carries an empty `Routines` too (the common creature bundle inserts
    /// it before the `if c.tamed` branch), so without this check
    /// `install_routine`/`uninstall_routine` would accept an entity no menu
    /// ever offers but nothing here refused either — the same ownership
    /// gate `extract_routine` and `sell_companion` already both apply.
    fn owns_routine_holder(&self, entity: Entity) -> bool {
        entity == self.player_entity()
            || self
                .world
                .get::<Tamed>(entity)
                .is_some_and(|t| t.owner == self.player_entity())
    }

    /// Spends one loose `item` and fills `entity`'s first free slot with the
    /// routine it carries. Free and unrestricted outside battle.
    pub fn install_routine(&mut self, entity: Entity, item: &ItemId) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.owns_routine_holder(entity) {
            return Err("You don't control that program.".into());
        }
        let ability = self
            .world
            .resource::<ItemDb>()
            .get(item.as_str())
            .and_then(|d| d.routine.clone())
            .ok_or_else(|| "That isn't a routine.".to_string())?;
        let player = self.player_entity();
        if self
            .world
            .get::<Inventory>(player)
            .map(|i| i.count(item))
            .unwrap_or(0)
            == 0
        {
            return Err("You don't have that routine.".into());
        }
        let mut installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        if installed.len() >= self.routine_slots(entity) {
            return Err("There's no free routine slot — pop one out first.".into());
        }
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(item.clone(), 1);
        let ability_name = self.ability_display_name(&ability);
        installed.push(ability);
        self.world.entity_mut(entity).insert(Routines(installed));
        let name = self.routine_holder_label(entity);
        self.log(format!("{name} now runs {ability_name}."));
        Ok(())
    }

    /// Frees `slot` and returns its routine to inventory as an item.
    ///
    /// `check_room` is a no-op for every routine item the shipped set can
    /// produce — `ItemDb::synthesize_routines` always mints `bank_limit:
    /// None` — so this can't actually refuse today. It runs anyway, and
    /// still runs *before* the slot is cleared: a modder can author their
    /// own item with both `routine` and `bank_limit` set, and that ordering
    /// is what keeps such an item from being eaten if the check ever does
    /// fail (the same reasoning `sell_item` documents for its own ordering).
    pub fn uninstall_routine(&mut self, entity: Entity, slot: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.owns_routine_holder(entity) {
            return Err("You don't control that program.".into());
        }
        let installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        let ability = installed
            .get(slot)
            .cloned()
            .ok_or_else(|| "That slot is empty.".to_string())?;
        let item = abilities::routine_item_id(&ability);
        self.check_room(&item, 1)?;

        let mut installed = installed;
        installed.remove(slot);
        self.world.entity_mut(entity).insert(Routines(installed));
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(item, 1);
        let name = self.routine_holder_label(entity);
        self.log(format!("{name} stops running that routine."));
        Ok(())
    }

    /// "You" for the player, the program's display name otherwise — the one
    /// place that distinction is worded, so every routine log line reads the
    /// same.
    pub(crate) fn routine_holder_label(&self, entity: Entity) -> String {
        if entity == self.player_entity() {
            "You".to_string()
        } else {
            self.creature_label(entity)
        }
    }

    /// Whether a routine-extraction bench is standing anywhere. Ownership,
    /// not proximity — see `StructureDef::extracts_routines`.
    pub fn can_extract_routines(&self) -> bool {
        self.world
            .resource::<StructureDb>()
            .all()
            .filter(|def| def.extracts_routines)
            .any(|def| self.has_structure(&def.id))
    }

    /// Display name of a bench that would allow extraction, for the refusal
    /// message — no code names a structure id.
    fn extraction_bench_name(&self) -> String {
        self.world
            .resource::<StructureDb>()
            .all()
            .find(|def| def.extracts_routines)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| "an extraction bench".to_string())
    }

    /// The routines installed on `creature`, in slot order — what an
    /// extraction offers to salvage.
    pub fn extractable_routines(&self, creature: Entity) -> Vec<AbilityDef> {
        let db = self.world.resource::<AbilityDb>();
        self.world
            .get::<Routines>(creature)
            .map(|r| r.0.clone())
            .unwrap_or_default()
            .iter()
            .filter_map(|id| db.get(id).cloned())
            .collect()
    }

    /// Every routine currently installed on `a` or `b` that fusing them
    /// would destroy. `Game::fuse_companions` derives the result's kit fresh
    /// from its winning species via `install_innate_routines` rather than
    /// merging the parents' — so anything installed manually on either one
    /// (researched, extracted, or swapped in from a third program) does not
    /// carry over, even if the winning species happens to declare the same
    /// ability innately. Feeds both the `FuseName` warning and the log line
    /// fusion itself writes.
    pub fn fusion_routine_losses(&self, a: Entity, b: Entity) -> Vec<AbilityDef> {
        let mut lost = self.actor_abilities(a);
        for ability in self.actor_abilities(b) {
            if !lost.iter().any(|kept| kept.id == ability.id) {
                lost.push(ability);
            }
        }
        lost
    }

    /// Destroys `creature` and salvages exactly one of its routines — the
    /// one at `index` in `extractable_routines`. Everything else installed
    /// on it is lost with it.
    ///
    /// Room for the payout is checked before the program is despawned, for
    /// the reason `sell_companion` documents about its own ordering — a
    /// no-op for the shipped item set, same caveat as `uninstall_routine`'s.
    pub fn extract_routine(&mut self, creature: Entity, index: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.can_extract_routines() {
            return Err(format!(
                "You need {} standing somewhere to extract a routine.",
                self.extraction_bench_name()
            ));
        }
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        let ability = self
            .extractable_routines(creature)
            .get(index)
            .map(|def| def.id.clone())
            .ok_or_else(|| "That program has no such routine.".to_string())?;
        let item = abilities::routine_item_id(&ability);
        self.check_room(&item, 1)?;

        let name = self.dissolve_tamed_program(creature);
        self.world
            .get_mut::<Inventory>(self.player_entity())
            .unwrap()
            .add(item, 1);
        let ability_name = self.ability_display_name(&ability);
        self.log(format!(
            "You break {name} down and salvage its {ability_name} routine."
        ));
        self.tick();
        Ok(())
    }
}
