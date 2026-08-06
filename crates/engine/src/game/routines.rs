//! Installing, removing and inspecting routines — the abilities that occupy
//! a party member's slots. Extraction lives here too.

use crate::components::Routines;
use crate::*;

impl Game {
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
        let glyph = self.world.get::<Glyph>(entity);
        RoutineHolderView {
            entity,
            glyph: glyph.map(|g| g.ch).unwrap_or('?'),
            color: glyph.map(|g| g.color).unwrap_or(GlyphColor::White),
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

    /// Whether the player has learned `ability` — by researching a node that
    /// grants it, or by extracting it out of a program.
    pub fn knows_routine(&self, ability: &str) -> bool {
        self.world.resource::<KnownRoutines>().0.contains(ability)
    }

    /// Every routine the player knows, name-sorted so the install picker's
    /// numbering is stable between sessions. Knowing one is half of an
    /// install; the other half is `routine_disks_held`.
    pub fn installable_routines(&self) -> Vec<KnownRoutineView> {
        let db = self.world.resource::<AbilityDb>();
        let mut rows: Vec<KnownRoutineView> = self
            .world
            .resource::<KnownRoutines>()
            .0
            .iter()
            .filter_map(|id| db.get(id))
            .map(|def| KnownRoutineView {
                ability: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
            })
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    /// Blank Routine Disks in cargo — what an install spends.
    pub fn routine_disks_held(&self) -> u32 {
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&ItemId::from(ids::ROUTINE_DISK)))
            .unwrap_or(0)
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

    /// Burns one blank Routine Disk to write `ability` into `entity`'s first
    /// free slot. Knowing the routine is not enough — the disk is the
    /// manufactured half, and it is gone for good.
    ///
    /// The disk is spent last, after every refusal has been cleared, for the
    /// reason `use_symlink` drops the locale only once its checks have all
    /// passed: nothing here may consume the disk on a path that then fails.
    pub fn install_routine(&mut self, entity: Entity, ability: &str) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.owns_routine_holder(entity) {
            return Err("You don't control that program.".into());
        }
        if !self.knows_routine(ability) {
            return Err("You don't know that routine.".into());
        }
        let installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        if installed.len() >= self.routine_slots(entity) {
            return Err("There's no free routine slot — pop one out first.".into());
        }
        let disk = ItemId::from(ids::ROUTINE_DISK);
        if self.routine_disks_held() == 0 {
            return Err(format!(
                "You need a blank {} to write that routine onto.",
                self.item_name(&disk)
            ));
        }
        let player = self.player_entity();
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(disk, 1);
        let ability_name = self.ability_display_name(ability);
        let mut installed = installed;
        installed.push(ability.to_string());
        self.world.entity_mut(entity).insert(Routines(installed));
        let name = self.routine_holder_label(entity);
        self.log(format!("{name} now runs {ability_name}."));
        Ok(())
    }

    /// Frees `slot`. The disk that filled it was spent at install and is not
    /// recoverable, so this hands back nothing — what the player keeps is the
    /// knowledge, which they never lost.
    pub fn uninstall_routine(&mut self, entity: Entity, slot: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.owns_routine_holder(entity) {
            return Err("You don't control that program.".into());
        }
        let mut installed = self
            .world
            .get::<Routines>(entity)
            .map(|r| r.0.clone())
            .ok_or_else(|| "That can't hold routines.".to_string())?;
        if slot >= installed.len() {
            return Err("That slot is empty.".to_string());
        }
        installed.remove(slot);
        self.world.entity_mut(entity).insert(Routines(installed));
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
    /// extraction offers to salvage. A row already `known` is one
    /// `extract_routine` will refuse, so the picker can say so before the
    /// program is on the block.
    pub fn extractable_routines(&self, creature: Entity) -> Vec<ExtractableRoutineView> {
        let db = self.world.resource::<AbilityDb>();
        let known = self.world.resource::<KnownRoutines>();
        self.world
            .get::<Routines>(creature)
            .map(|r| r.0.clone())
            .unwrap_or_default()
            .iter()
            .filter_map(|id| db.get(id))
            .map(|def| ExtractableRoutineView {
                ability: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
                known: known.0.contains(&def.id),
            })
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

    /// Destroys `creature` and learns exactly one of its routines — the one
    /// at `index` in `extractable_routines`. Everything else installed on it
    /// is lost with it. What is salvaged is the *knowledge*: no disk comes
    /// out of this, and installing it still costs one.
    ///
    /// A routine the player already knows is refused rather than accepted as
    /// a no-op, checked before the program is despawned: knowledge does not
    /// stack, so taking it twice would destroy a program for nothing.
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
            .map(|row| row.ability.clone())
            .ok_or_else(|| "That program has no such routine.".to_string())?;
        if self.knows_routine(&ability) {
            return Err("You already know that routine.".into());
        }

        let name = self.dissolve_tamed_program(creature);
        self.world
            .resource_mut::<KnownRoutines>()
            .0
            .insert(ability.clone());
        let ability_name = self.ability_display_name(&ability);
        self.log(format!(
            "You break {name} down and learn its {ability_name} routine."
        ));
        self.tick();
        Ok(())
    }
}
