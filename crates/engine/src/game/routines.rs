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

    /// One routine's mechanics for the inspect page — what makes it run,
    /// what it lands on, what it does, and what it costs.
    ///
    /// **`caster` is who would run it, and every magnitude is scaled for
    /// them.** An ability's authored `power` is its level-1 figure, so a
    /// screen printing it directly quotes a number the cast never uses.
    /// Both scaling axes are the cast's own — `abilities::scaled_range` and
    /// friends for level, `Game::ability_affinity` for the caster's
    /// category multiplier — which is what keeps this page from becoming a
    /// third copy of the damage chain.
    ///
    /// For gear, the caster is the wearer: a granted passive fires as
    /// whoever is wearing it.
    ///
    /// `None` for an id the current ability set doesn't define, which a save
    /// naming a since-removed mod routine produces. Every caller is a
    /// renderer that would turn a refusal back into the same empty draw.
    pub fn routine_detail(
        &self,
        id: &crate::abilities::AbilityId,
        caster: Entity,
    ) -> Option<RoutineDetailView> {
        let def = self.world.resource::<AbilityDb>().get(id)?.clone();
        let level = self.ability_user_level(caster);
        let affinity = self.ability_affinity(caster, &def.effect);
        Some(RoutineDetailView {
            name: def.name.clone(),
            description: def.description.clone(),
            when: match def.triggers {
                Some(trigger) => trigger.phrase(),
                // Not "chosen as a Special": the two field-only effects are
                // never offered in a battle at all, and a page telling a
                // player to look for Phase on the Special menu sends them
                // somewhere it can never appear.
                None if def.effect.field_only() => {
                    "Cast outside battle from the routine list".to_string()
                }
                None => "Chosen as a Special in battle".to_string(),
            },
            target: def.target.phrase().to_string(),
            effect: self.routine_effect_label(&def, level, affinity),
            cooldown: def.cooldown,
            power_cost: crate::abilities::routine_power_cost(&def),
            rolls_to_hit: matches!(
                def.effect,
                AbilityEffect::Damage { .. } | AbilityEffect::Drain { .. }
            ),
        })
    }

    /// What a routine does, in one line, with its magnitudes already scaled
    /// for a caster at `level` with `affinity`.
    ///
    /// **Exhaustive on `AbilityEffect`**, the rule `render/stack.rs`'s
    /// `cell_mark` records: as a `_ =>` arm an eleventh effect would ship
    /// with the page silently saying nothing about it.
    fn routine_effect_label(&self, def: &AbilityDef, level: u32, affinity: f32) -> String {
        use crate::abilities::{scaled_hp_power, scaled_range, scaled_stat_power};
        match &def.effect {
            AbilityEffect::Damage {
                power,
                spread,
                status,
            } => {
                let band = scaled_range(
                    crate::battle::DamageRange::centred(*power, *spread),
                    level,
                    affinity,
                );
                let mut line = format!("Damage {}", self.damage_range_label(band));
                // The rider's chance is a property of the move and is not
                // scaled by anything, so it prints as authored.
                if let Some(status) = status {
                    line.push_str(&format!(
                        ", {:.0}% to inflict {}",
                        status.chance * 100.0,
                        status.kind.label()
                    ));
                }
                line
            }
            AbilityEffect::Heal { power, spread } => {
                let band = scaled_range(
                    crate::battle::DamageRange::centred(*power, *spread),
                    level,
                    affinity,
                );
                format!("Restores {} Integrity", self.damage_range_label(band))
            }
            AbilityEffect::Buff {
                kind,
                power,
                duration,
            } => format!(
                "{} {:+} for {duration} rounds",
                kind.label(),
                scaled_stat_power(*power, level, affinity)
            ),
            AbilityEffect::Debuff {
                kind,
                power,
                duration,
            } => format!(
                "Inflicts {} ({}) for {duration} rounds",
                kind.label(),
                scaled_hp_power(*power, level, affinity)
            ),
            AbilityEffect::Drain {
                power,
                spread,
                heal_fraction,
            } => {
                let band = scaled_range(
                    crate::battle::DamageRange::centred(*power, *spread),
                    level,
                    affinity,
                );
                format!(
                    "Damage {}, healing the caster {:.0}% of it",
                    self.damage_range_label(band),
                    heal_fraction * 100.0
                )
            }
            AbilityEffect::Cleanse => "Clears the recipient's status condition".to_string(),
            AbilityEffect::Decompile => "Attempts a capture, spending a catalyst".to_string(),
            AbilityEffect::FieldBuff {
                kind,
                power,
                duration,
                interval,
            } => {
                let magnitude =
                    kind.magnitude_label(scaled_stat_power(*power, level, affinity), *interval);
                // A `0` here is an until-rest buff whose count nothing
                // reads, never a buff that expires the turn it is cast —
                // see `AbilityEffect::FieldBuff::duration`.
                match kind.runs_until_rest() {
                    true => format!("{magnitude} until the party rests"),
                    false => format!("{magnitude} for {duration} turns"),
                }
            }
            AbilityEffect::Phase => "Steps the party through one solid cell".to_string(),
            AbilityEffect::Jump => {
                "Moves the party to a cell you point at, fatally if it is solid".to_string()
            }
        }
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
    ///
    /// Always false for an exclusive routine. Nothing writes one into
    /// `KnownRoutines`, which is the whole of what makes it exclusive: a
    /// routine you know is one you can etch onto blank disks forever, so
    /// knowledge is the only thing in this game that duplicates.
    pub fn knows_routine(&self, ability: &str) -> bool {
        self.world.resource::<KnownRoutines>().0.contains(ability)
    }

    /// Whether `ability` is an exclusive routine — one nobody can learn or
    /// etch, reachable only as an already-written disk off a boss or a Stack
    /// trader's shelf.
    ///
    /// An ability this build no longer has is *not* exclusive: a save can
    /// reference a routine a mod has since removed, and treating an unknown
    /// id as exclusive would silently refuse to etch a disk the player has
    /// every right to. Unknown means gone, and gone is handled by the
    /// `AbilityDb::get` that fails right after.
    pub fn routine_is_exclusive(&self, ability: &str) -> bool {
        self.world
            .resource::<AbilityDb>()
            .get(ability)
            .is_some_and(|def| def.exclusive)
    }

    /// Every routine the player knows, name-sorted so the etch picker's
    /// numbering is stable between sessions. Knowing one is half of an
    /// etch; the other half is `blank_disks_held`.
    ///
    /// Exclusive routines are filtered out explicitly, even though nothing
    /// is supposed to put one in `KnownRoutines` in the first place. That
    /// is deliberate belt-and-braces: `etch_disk` refuses them anyway, so
    /// without this a leak — a save written by a modded build, a future
    /// grant that forgets the rule — would show up as a picker row that
    /// always fails rather than as a routine quietly missing. Two cheap
    /// checks, and the loud failure mode is the one on the outside.
    pub fn etchable_routines(&self) -> Vec<KnownRoutineView> {
        let db = self.world.resource::<AbilityDb>();
        let mut rows: Vec<KnownRoutineView> = self
            .world
            .resource::<KnownRoutines>()
            .0
            .iter()
            .filter_map(|id| db.get(id))
            .filter(|def| !def.exclusive)
            .map(|def| KnownRoutineView {
                ability: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
                held: self.etched_disks_of(&def.id),
            })
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    /// Blank Routine Disks in cargo — what an etch spends.
    pub fn blank_disks_held(&self) -> u32 {
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&ItemId::from(ids::ROUTINE_DISK)))
            .unwrap_or(0)
    }

    /// Every etched disk in cargo, name-sorted — what an *install* spends,
    /// and the picker the routine screen offers.
    ///
    /// Derived by walking cargo for the `etched_` prefix rather than by
    /// walking the ability set and counting each: cargo is short, the
    /// ability set is not, and a disk whose ability a mod has removed drops
    /// out here on its own rather than needing a rule.
    pub fn etched_disks_held(&self) -> Vec<EtchedDiskView> {
        let db = self.world.resource::<AbilityDb>();
        let mut rows: Vec<EtchedDiskView> = self
            .world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.items.clone())
            .unwrap_or_default()
            .into_iter()
            .filter(|(_, qty)| *qty > 0)
            .filter_map(|(item, qty)| {
                let def = db.get(item.etched_ability()?)?;
                Some(EtchedDiskView {
                    ability: def.id.clone(),
                    name: def.name.clone(),
                    description: def.description.clone(),
                    exclusive: def.exclusive,
                    qty,
                })
            })
            .collect();
        rows.sort_by(|a, b| a.name.cmp(&b.name));
        rows
    }

    /// How many etched disks of `ability` are in cargo.
    pub fn etched_disks_of(&self, ability: &str) -> u32 {
        self.world
            .get::<Inventory>(self.player_entity())
            .map(|inv| inv.count(&ItemId::etched(ability)))
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

    /// Burns one blank Routine Disk to write `ability` onto it, producing an
    /// etched disk in cargo. Knowing the routine is not enough — the blank
    /// is the manufactured half, and it is gone for good.
    ///
    /// The first half of what `install_routine` used to do in one step. The
    /// split exists so that a routine can arrive in the world *already*
    /// etched, off a boss or a trader's shelf, without anyone having to know
    /// it: an exclusive routine is simply one that never reaches this
    /// function, because nothing puts it in `KnownRoutines`.
    ///
    /// The blank is spent last, after every refusal has been cleared, for
    /// the reason `use_symlink` drops the locale only once its checks have
    /// all passed: nothing here may consume the disk on a path that then
    /// fails.
    pub fn etch_disk(&mut self, ability: &str) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        // Checked before `knows_routine`, and this is the only reason this
        // branch exists at all: an exclusive routine is never known, so the
        // check below would already refuse it — with "you don't know that
        // routine", which is true and tells the player nothing about why
        // they never will.
        if self.routine_is_exclusive(ability) {
            let name = self.ability_display_name(ability);
            return Err(format!(
                "{name} can't be written to a blank. That one only comes already etched."
            ));
        }
        if !self.knows_routine(ability) {
            return Err("You don't know that routine.".into());
        }
        let blank = ItemId::from(ids::ROUTINE_DISK);
        if self.blank_disks_held() == 0 {
            return Err(format!(
                "You need a blank {} to write that routine onto.",
                self.item_name(&blank)
            ));
        }
        // No cargo-room check, here or in `extract_routine`'s exclusive
        // branch: the Buffer is unbounded (`Inventory::cargo_used` is a
        // display figure, not a cap) and `grant_loot` always lands `qty` in
        // full. If a hold limit is ever added, both of those become paths
        // that can take a blank — or destroy a program — and hand back
        // nothing, and both need a check before the `take`.
        let etched = ItemId::etched(ability);
        let player = self.player_entity();
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(blank, 1);
        self.grant_loot(etched.clone(), 1);
        let ability_name = self.ability_display_name(ability);
        self.log(format!("You burn {ability_name} onto a blank disk."));
        Ok(())
    }

    /// Spends one etched disk to write its routine into `entity`'s first
    /// free slot. The disk is consumed — what is in the slot *is* the disk,
    /// and `uninstall_routine` does not hand it back.
    ///
    /// The second half of the old `install_routine`, and the only way a
    /// routine ever reaches a slot outside a species' innate kit. That
    /// single door is what makes the exclusive pool enforceable: there is
    /// one place to look for how a routine got where it is.
    ///
    /// Ordered like `etch_disk` above and `buy_market_offer` beside it —
    /// every refusal lands before the disk is taken.
    pub fn install_disk(&mut self, entity: Entity, ability: &str) -> Result<(), String> {
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
        if installed.iter().any(|id| id == ability) {
            let name = self.routine_holder_label(entity);
            let ability_name = self.ability_display_name(ability);
            return Err(format!("{name} already runs {ability_name}."));
        }
        if installed.len() >= self.routine_slots(entity) {
            return Err("There's no free routine slot — pop one out first.".into());
        }
        let etched = ItemId::etched(ability);
        if self.etched_disks_of(ability) == 0 {
            return Err(format!("You're not carrying {}.", self.item_name(&etched)));
        }
        let player = self.player_entity();
        self.world
            .get_mut::<Inventory>(player)
            .unwrap()
            .take(etched, 1);
        let ability_name = self.ability_display_name(ability);
        self.write_routine(entity, ability);
        let name = self.routine_holder_label(entity);
        self.log(format!("{name} now runs {ability_name}."));
        Ok(())
    }

    /// Writes `ability` into `entity`'s next free slot, and nothing else —
    /// no ownership check, no disk, no knowledge.
    ///
    /// Every one of those absences is the caller's business. `install_disk`
    /// above is the door the player comes through and states all three in
    /// its own vocabulary; `Game::install_innate_routines` fills a freshly
    /// compiled program's species kit and owes none of them. What the two
    /// must agree on is what a slot is and how one gets filled, which is the
    /// whole of what lives here.
    ///
    /// Silently does nothing if there is no room. Both callers check first —
    /// this refusing loudly would be a second copy of a rule they each
    /// already have to state.
    pub(crate) fn write_routine(&mut self, entity: Entity, ability: &str) {
        let Some(mut installed) = self.world.get::<Routines>(entity).map(|r| r.0.clone()) else {
            return;
        };
        if installed.len() >= self.routine_slots(entity) {
            return;
        }
        installed.push(ability.to_string());
        self.world.entity_mut(entity).insert(Routines(installed));
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

    /// Destroys `creature` and salvages exactly one of its routines — the
    /// one at `index` in `extractable_routines`. Everything else installed
    /// on it is lost with it.
    ///
    /// What comes out depends on whether the routine can be learned:
    ///
    /// - **Ordinary** — the *knowledge*. No disk comes out, and etching one
    ///   still costs a blank. A routine the player already knows is refused
    ///   rather than accepted as a no-op, checked before the program is
    ///   despawned: knowledge does not stack, so taking it twice would
    ///   destroy a program for nothing.
    /// - **Exclusive** — the *disk*, popped back out intact, and nothing
    ///   learned. This is the only way to move an exclusive routine off a
    ///   program, and it costs the whole program to do it. There is still
    ///   exactly one copy in the run afterwards, which is the invariant the
    ///   entire exclusive pool rests on.
    ///
    /// The "already known" refusal is deliberately not applied to the
    /// exclusive branch. An exclusive routine is never known, so the check
    /// would never fire — and if it somehow did, refusing would strand a
    /// disk on a program forever.
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
        let exclusive = self.routine_is_exclusive(&ability);
        if !exclusive && self.knows_routine(&ability) {
            return Err("You already know that routine.".into());
        }

        let name = self.dissolve_tamed_program(creature);
        let ability_name = self.ability_display_name(&ability);
        if exclusive {
            let disk = ItemId::etched(&ability);
            self.grant_loot(disk.clone(), 1);
            self.log(format!(
                "You break {name} down and pry its {ability_name} disk back out intact."
            ));
        } else {
            self.world
                .resource_mut::<KnownRoutines>()
                .0
                .insert(ability.clone());
            self.log(format!(
                "You break {name} down and learn its {ability_name} routine."
            ));
        }
        self.tick();
        Ok(())
    }
}
