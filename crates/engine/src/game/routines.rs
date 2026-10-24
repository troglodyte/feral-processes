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
    /// **`invoker` is who would run it, and every magnitude is scaled for
    /// them.** An ability's authored `power` is its level-1 figure, so a
    /// screen printing it directly quotes a number the invocation never uses.
    /// Both scaling axes are the invocation's own — `abilities::scaled_range` and
    /// friends for level, `Game::ability_affinity` for the invoker's
    /// category multiplier — which is what keeps this page from becoming a
    /// third copy of the damage chain.
    ///
    /// For gear, the invoker is the wearer: a granted passive fires as
    /// whoever is wearing it.
    ///
    /// `None` for an id the current ability set doesn't define, which a save
    /// naming a since-removed mod routine produces. Every caller is a
    /// renderer that would turn a refusal back into the same empty draw.
    pub fn routine_detail(
        &self,
        id: &crate::abilities::AbilityId,
        invoker: Entity,
    ) -> Option<RoutineDetailView> {
        let def = self.world.resource::<AbilityDb>().get(id)?.clone();
        let level = self.ability_user_level(invoker);
        let affinity = self.ability_affinity(invoker, &def.effect);
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
                    "Run outside battle from the routine list".to_string()
                }
                // The both-ways arm, read off the same predicate
                // `Game::field_routines` filters on — a priced `Heal` is a
                // Special *and* a field routine, and a page deriving that
                // for itself could promise a row the list never shows. The
                // unpriced ones fall through to the line below, which is the
                // whole visible consequence of `field_runnable`'s price
                // gate.
                None if def.field_runnable() => {
                    "Chosen as a Special in battle, or run outside battle from the routine list"
                        .to_string()
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
    /// for an invoker at `level` with `affinity`.
    ///
    /// A call, not a copy: `abilities::effect_label` owns the body, because
    /// the creation wizard prices its routine rows with no `Game` to ask.
    fn routine_effect_label(&self, def: &AbilityDef, level: u32, affinity: f32) -> String {
        crate::abilities::effect_label(def, level, affinity, self.world.resource::<StatusDb>())
    }

    /// Whether `entity` is a program the player controls — `Tamed`, or the
    /// player themselves. The one door `routine_view`'s concealment and the
    /// wild death line's own name filter both read, rather than
    /// `Game::program_role`, which narrows further to "and posted
    /// somewhere": a wielded, sortied or staffed program is still the
    /// player's own kit, never somebody else's to conceal.
    pub(crate) fn program_is_owned(&self, entity: Entity) -> bool {
        self.world.get::<Tamed>(entity).is_some() || self.world.get::<Player>(entity).is_some()
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
                        fixed: self.routine_is_permanent(&def.id),
                    },
                    None => RoutineSlotView {
                        index,
                        ability: None,
                        name: "(empty)".to_string(),
                        description: String::new(),
                        fixed: false,
                    },
                },
            )
            .collect()
    }

    /// Builds one `RoutineHolderView` row for `entity`, labelled `name`.
    /// Shared by `routine_holders` (every program you own) and
    /// `Game::field_routine_targets` (just you and your active `Party`) so the
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

    /// Whether `family` has been discovered — any of its rungs is in
    /// `DiscoveredRoutines` **or** `KnownRoutines`.
    pub fn family_discovered(&self, family: &str) -> bool {
        let discovered = &self
            .world
            .resource::<crate::resources::DiscoveredRoutines>()
            .0;
        let known = &self.world.resource::<KnownRoutines>().0;
        self.world
            .resource::<AbilityDb>()
            .all()
            .filter(|def| crate::routine_tree::family(def) == family)
            .any(|def| discovered.contains(&def.id) || known.contains(&def.id))
    }

    /// Whether `family` has a carrier at all — a positive `wild_weight` on
    /// any of its rungs, or a place in some species' kit.
    pub fn family_is_discoverable(&self, family: &str) -> bool {
        let abilities = self.world.resource::<AbilityDb>();
        let rungs: Vec<&str> = abilities
            .all()
            .filter(|def| crate::routine_tree::family(def) == family)
            .map(|def| def.id.as_str())
            .collect();
        if abilities
            .all()
            .any(|def| rungs.contains(&def.id.as_str()) && def.wild_weight > 0)
        {
            return true;
        }
        self.world.resource::<SpeciesDb>().all().any(|species| {
            species
                .abilities
                .iter()
                .any(|declared| rungs.contains(&declared.id.as_str()))
        })
    }

    /// Whether `ability` is **fixed** — welded to the slot it is born in.
    /// It can't be moved and can't be popped back out of the slot it
    /// occupies.
    ///
    /// `decompile` is the whole of the set and the reason it exists: it is
    /// the player's capture mechanism, granted once by `spawn_player` and by
    /// nothing else in the game.
    ///
    /// Keyed to `abilities::DECOMPILE_ABILITY_ID` rather than an
    /// asset-authored flag beside `exclusive`, because permanence is not a
    /// property a file may claim: `AbilityEffect::Decompile`, the constant
    /// itself and the hardcoded `spawn_player` grant are already engine
    /// facts, and a schema flag would let a mod mint a second routine nobody
    /// can ever remove.
    pub fn routine_is_permanent(&self, ability: &str) -> bool {
        crate::routine_tree::is_permanent(ability)
    }

    /// Whether `entity` is a routine holder the player actually controls —
    /// themself, or a program `Tamed` to them. A save-loaded wild creature
    /// carries an empty `Routines` too (the common creature bundle inserts
    /// it before the `if c.tamed` branch), so without this check
    /// `install_built`/`uninstall_routine` would accept an entity no menu
    /// ever offers but nothing here refused either — the same ownership
    /// gate `extract_routine` and `sell_companion` already both apply.
    pub(crate) fn owns_routine_holder(&self, entity: Entity) -> bool {
        entity == self.player_entity()
            || self
                .world
                .get::<Tamed>(entity)
                .is_some_and(|t| t.owner == self.player_entity())
    }

    /// Writes `ability` into `entity`'s next free slot, and nothing else —
    /// no ownership check.
    ///
    /// That is the caller's business. `Game::install_built` is the door the
    /// player comes through and states it in its own vocabulary;
    /// `Game::install_innate_routines` fills a freshly compiled program's
    /// species kit and owes none of it. What the two must agree on is what a
    /// slot is and how one gets filled, which is the whole of what lives
    /// here.
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

    /// Frees `slot`. A built routine pulled out is destroyed, and nothing is
    /// handed back.
    ///
    /// A permanent routine refuses. The check reads the ability *in the
    /// slot* rather than taking one as an argument, because a slot index is
    /// all the screen has to offer — see `App::handle_routines_key`.
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
        if self.routine_is_permanent(&installed[slot]) {
            let name = self.ability_display_name(&installed[slot]);
            return Err(format!(
                "{name} is welded into that slot. It doesn't come out."
            ));
        }
        let removed = installed.remove(slot);
        self.world.entity_mut(entity).insert(Routines(installed));
        // A built routine exists only in its slot or on the Built page, so
        // pulling it out destroys it (ADR 0029).
        self.destroy_built(&[removed]);
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
            .filter(|def| self.extraction_researched(def))
            .any(|def| self.has_structure(&def.id))
    }

    /// Display name of a structure carrying `flag`, for a message — no code
    /// names a structure id.
    ///
    /// The *catalogue's* answer, not a standing structure's: the one caller
    /// is a refusal telling the player what they need to build, so the
    /// structure it names is by definition one they do not have. A row about
    /// a bench that *is* standing must not use this — see
    /// `Game::extraction_bench`, which reads the name off the same structure
    /// that supplied its tier.
    fn bench_name(&self, flag: fn(&StructureDef) -> bool) -> String {
        self.world
            .resource::<StructureDb>()
            .all()
            .find(|def| flag(def))
            .map(|def| def.name.clone())
            .unwrap_or_else(|| "an extraction bench".to_string())
    }

    /// The routines installed on `creature`, in slot order — what an
    /// extraction offers to salvage.
    pub fn extractable_routines(&self, creature: Entity) -> Vec<ExtractableRoutineView> {
        let db = self.world.resource::<AbilityDb>();
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
    /// What comes out is the routine's parts (`parts_of_routine`) — Core and
    /// every modifier — and nothing else. This is the only way to move an
    /// exclusive routine off a program, and it costs the whole program to do
    /// it.
    pub fn extract_routine(&mut self, creature: Entity, index: usize) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.can_extract_routines() {
            return Err(format!(
                "You need {} standing somewhere to extract a routine.",
                self.bench_name(|def| def.extracts_routines)
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
        if self
            .world
            .get::<crate::components::Jailed>(creature)
            .is_some()
        {
            return Err(format!(
                "That program is {}",
                crate::game::party::HELD_IN_CELL
            ));
        }
        if self
            .world
            .get::<crate::components::Siphoned>(creature)
            .is_some()
        {
            return Err(format!(
                "That program is {}",
                crate::game::party::HELD_IN_SIPHON
            ));
        }
        // This despawns `creature` below (`dissolve_tamed_program`), and a
        // posted crew member despawned mid-shift would strand its outpost
        // counting a body that no longer exists — the outposts plan's own
        // census, `wield_program`'s reason one door over.
        if self
            .world
            .get::<crate::components::PostedAt>(creature)
            .is_some()
        {
            return Err("That program is posted at an outpost. Recall it first.".into());
        }
        let ability = self
            .extractable_routines(creature)
            .get(index)
            .map(|row| row.ability.clone())
            .ok_or_else(|| "That program has no such routine.".to_string())?;
        // Read before the dissolve: a built routine is destroyed with its
        // holder, and its record is where its parts are.
        let parts = self.parts_of_routine(&ability);
        if parts.is_empty() {
            return Err("There's nothing in that routine to take apart.".into());
        }
        let name = self.dissolve_tamed_program(creature, crate::bonds::Departure::LetGo);
        let ability_name = self.ability_display_name(&ability);
        self.take_routine(&parts);
        self.log(format!(
            "You break {name} down and take {ability_name} apart into its parts."
        ));
        self.tick();
        Ok(())
    }

    /// The parts `ability` is made of: a built routine's own record, an
    /// authored one's `decompose`. Empty for what no Core stands for.
    pub(crate) fn parts_of_routine(&self, ability: &str) -> Vec<crate::routine_craft::Subroutine> {
        let crafted = self.world.resource::<crate::resources::CraftedRoutines>();
        if let Some(built) = crafted.built.iter().find(|r| r.id == ability) {
            return built.parts.clone();
        }
        self.world
            .resource::<AbilityDb>()
            .get(ability)
            .and_then(crate::routine_craft::decompose)
            .unwrap_or_default()
    }

    /// The one place a routine comes off a program — `extract_routine`'s
    /// tamed-program door and `extract_program`'s `Routines`-tool door both
    /// call this. It hands over the routine's parts (ADR 0029) and nothing
    /// else.
    pub(crate) fn take_routine(&mut self, parts: &[crate::routine_craft::Subroutine]) {
        self.grant_parts(parts, LootSource::Extract);
    }
}
