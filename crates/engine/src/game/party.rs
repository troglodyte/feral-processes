//! The player's own roster — status, the programs in the party and the
//! bank, and moving programs between them.

use crate::tuning::{FUSION_LESSER_STAT_DIVISOR, MAX_FUSIONS};
use crate::*;

impl Game {
    pub fn player_status(&self) -> PlayerStatus {
        let pet_count = self.pet_count();
        let pet_capacity = self.pet_capacity();
        let player = self.player_entity();
        let stats = self.world.get::<Stats>(player).unwrap();
        let needs = self.world.get::<Needs>(player).unwrap();
        let pos = self.world.get::<Position>(player).unwrap();
        let inv = self.world.get::<Inventory>(player).unwrap();
        let exp = self.world.get::<Experience>(player).unwrap();
        let decompiler = self
            .world
            .get::<Decompiler>(player)
            .map(|d| d.skill)
            .unwrap_or(0);
        let equipment = self
            .world
            .get::<Equipment>(player)
            .cloned()
            .unwrap_or_default();
        let perks = self.world.get::<Perks>(player);
        let atk = self.effective_atk(player);
        let def = self.effective_def(player);
        let db = self.world.resource::<ItemDb>();
        // Grouped here, in the view, and deliberately not in `Inventory`:
        // that component's order is persisted through `PlayerSave`, so
        // sorting it would rewrite save contents and overwrite pickup order
        // to change what is only ever a display concern.
        //
        // Banked items are filtered out here rather than at each screen,
        // because this one list is what every consumer of "what does the
        // player have" reads — the inventory screen, the base panel,
        // `cost_display`'s have/need columns, and the trade screen's sell
        // rows. A bank is not cargo and is not a good, so it belongs in
        // none of them; the one screen that wants the number asks for it
        // by name through `Game::banked`.
        //
        // The two stores are merged here rather than handed out separately:
        // a fused copy is cargo like any other, and every screen that lists
        // "what does the player have" wants both. Tier is the tiebreak
        // inside a category so an item's fused rows sit beside its plain
        // one instead of drifting apart as categories are re-sorted.
        let fused = self.world.get::<FusedGear>(player);
        let mut inventory: Vec<InventoryRow> = inv
            .items
            .iter()
            .filter(|(item, _)| !db.get(item.as_str()).is_some_and(|d| d.banked))
            .map(|(item, qty)| InventoryRow {
                item: item.clone(),
                tier: 0,
                qty: *qty,
            })
            .chain(
                fused
                    .into_iter()
                    .flat_map(|f| f.copies.iter())
                    .map(|(item, tier, qty)| InventoryRow {
                        item: item.clone(),
                        tier: *tier,
                        qty: *qty,
                    }),
            )
            .collect();
        inventory.sort_by_key(|row| (self.category_sort_key(&row.item), row.tier));
        PlayerStatus {
            position: (pos.x, pos.y),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk,
            def,
            power: stats.max_hp + atk + def,
            decompiler,
            hunger: needs.hunger,
            fatigue: needs.fatigue,
            inventory,
            inventory_used: self.inventory_used(),
            pet_count,
            pet_capacity,
            level: exp.level,
            xp: exp.xp,
            xp_to_next: exp.xp_to_next,
            weapon: equipment.weapon,
            wielded: self.wielded_program().map(|e| WieldedView {
                name: self.creature_label(e),
                level: self
                    .world
                    .get::<Experience>(e)
                    .map(|x| x.level)
                    .unwrap_or(1),
                bonus: self.wielded_stat_bonus(),
            }),
            armor: equipment.armor,
            module: equipment.module,
            companions: self.party_info(),
            zone: self.world.resource::<ZoneLevel>().0,
            perk_points: perks.map(|p| p.points).unwrap_or(0),
            unlocked_perks: perks.map(|p| p.unlocked.clone()).unwrap_or_default(),
        }
    }

    /// A creature's own display name: the player's `CustomName` if they set
    /// one (`Game::fuse_companions` or `Game::rename_companion`), else its species
    /// name (falling back to the raw species id if the species definition
    /// is somehow missing). `None` if `entity` isn't a `Creature` at all.
    pub(crate) fn creature_name(&self, entity: Entity) -> Option<String> {
        let c = self.world.get::<Creature>(entity)?;
        if let Some(custom) = self.world.get::<CustomName>(entity) {
            return Some(custom.0.clone());
        }
        Some(
            self.world
                .resource::<SpeciesDb>()
                .get(&c.species)
                .map(|s| s.name.clone())
                .unwrap_or_else(|| c.species.clone()),
        )
    }

    /// `creature_name`, zone-tagged, falling back to a generic label if
    /// `entity` isn't a `Creature`.
    pub(crate) fn creature_label(&self, entity: Entity) -> String {
        match self.creature_name(entity) {
            Some(name) => self.zone_tagged_name(entity, name),
            None => "Program".to_string(),
        }
    }

    /// Appends a creature's `ZonePortal` to its species name for display
    /// (e.g. "Scrapper 2"), so a deeper-zone catch reads differently from a
    /// shallow one at a glance. Falls back to the bare name if the entity
    /// has no `ZonePortal` — expected for creatures hand-spawned outside the
    /// normal `spawn_wild_creature` path (e.g. in tests).
    pub(crate) fn zone_tagged_name(&self, entity: Entity, name: String) -> String {
        match self.world.get::<ZonePortal>(entity) {
            Some(zone) => format!("{name} {}", zone.0),
            None => name,
        }
    }

    /// Whether `entity` is a creature of a boss species (`SpeciesDef::is_boss`).
    /// `false` for anything that isn't a creature, or whose species failed
    /// to resolve.
    pub(crate) fn is_boss_creature(&self, entity: Entity) -> bool {
        self.world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .is_some_and(|s| s.is_boss)
    }

    pub(crate) fn companion_info(&self, entity: Entity) -> Option<CompanionInfo> {
        let stats = self.world.get::<Stats>(entity)?;
        Some(CompanionInfo {
            entity,
            name: self.creature_label(entity),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk: stats.atk,
            def: stats.def,
            power: stats.power(),
            status: self.status_label(entity),
            ability: self.ability_label(entity),
        })
    }

    /// Terse label for what commanding `entity` in battle would do right
    /// now. A member with several routines reads as a count, since no one of
    /// them is *the* answer until the player picks in `Mode::BattleSpecial`.
    pub(crate) fn ability_label(&self, entity: Entity) -> String {
        match self.actor_abilities(entity).as_slice() {
            // Anyone can be empty now: an innate routine can be popped out,
            // and the player starts with only decompile installed.
            [] => "No routines installed".to_string(),
            [only] => only.name.clone(),
            many => format!("{} routines", many.len()),
        }
    }

    /// How many fusions deep `entity`'s lineage is (see
    /// `components::FusionCount`) — 0 for anything caught or spawned
    /// normally, up to `MAX_FUSIONS`, at which point it can't be fused
    /// again.
    pub fn fusion_count(&self, entity: Entity) -> u32 {
        self.world
            .get::<FusionCount>(entity)
            .map(|f| f.0)
            .unwrap_or(0)
    }

    /// Display string for `entity`'s rolled `Potential`, e.g.
    /// "Excellent (94%)" — `None` if it has no `Potential` component (an
    /// old save predating it, or a non-creature entity).
    pub(crate) fn potential_quality_label(&self, entity: Entity) -> Option<String> {
        let potential = self.world.get::<Potential>(entity)?;
        Some(format!(
            "{} ({}%)",
            potential.quality_label(),
            potential.quality_percent()
        ))
    }

    /// Snapshot of every current party member (see `resources::Party`), in
    /// party-slot order.
    pub(crate) fn party_info(&self) -> Vec<CompanionInfo> {
        self.world
            .resource::<Party>()
            .0
            .iter()
            .filter_map(|&e| self.companion_info(e))
            .collect()
    }

    /// Full stats for every tamed program the player owns, anywhere on the
    /// map — unlike `view_entities`, not limited to what's currently in
    /// view. Lets you check on a cronjob worker's HP/level without walking
    /// over to it.
    ///
    /// Party members lead the list in slot order, everything else follows in
    /// spawn order. The sort lives here rather than in a renderer because
    /// app-core maps number keys by index while gui draws the rows: sorting
    /// in one and not the other would pick a different program than the one
    /// the player pressed.
    pub fn owned_pets(&mut self) -> Vec<PetInfo> {
        let player = self.player_entity();
        let party = self.world.resource::<Party>().0.clone();
        let mut owned: Vec<Entity> = {
            let mut query = self.world.query::<(Entity, &Tamed)>();
            query
                .iter(&self.world)
                .filter(|(_, t)| t.owner == player)
                .map(|(e, _)| e)
                .collect()
        };
        let slot_of = |entity: &Entity| party.iter().position(|p| p == entity);
        // The party leads, in slot order, because that order is mechanical:
        // the front slot draws the most fire (see `battle::slot_aggro_weight`)
        // and the companion screen exists to arrange it. Behind them the name
        // decides, since bevy's query order is not stable and the four other
        // screens reading this list — fuse, extract, routines, manifest — have
        // no slot to show and were getting no order at all.
        owned.sort_by_key(|e| (slot_of(e).unwrap_or(usize::MAX), self.creature_label(*e)));
        owned
            .into_iter()
            .filter_map(|entity| {
                let stats = *self.world.get::<Stats>(entity)?;
                let level = self
                    .world
                    .get::<Experience>(entity)
                    .map(|e| e.level)
                    .unwrap_or(1);
                let glyph = self.world.get::<Glyph>(entity);
                Some(PetInfo {
                    entity,
                    glyph: glyph.map(|g| g.ch).unwrap_or('?'),
                    color: glyph.map(|g| g.color).unwrap_or(GlyphColor::White),
                    name: self.creature_label(entity),
                    level,
                    hp: stats.hp,
                    max_hp: stats.max_hp,
                    atk: stats.atk,
                    def: stats.def,
                    power: stats.power(),
                    party_slot: slot_of(&entity).map(|s| s as u32),
                    activity: self.program_activity(entity),
                    quality: self.potential_quality_label(entity),
                    fusions: self.fusion_count(entity),
                    wielded: self.wielded_program() == Some(entity),
                })
            })
            .collect()
    }

    /// You, then every program you own — everyone the manifest screen can
    /// page through. Same membership and order as `owned_pets` with the
    /// player prepended, so the two can't disagree about what you have.
    pub fn manifest_subjects(&mut self) -> Vec<Entity> {
        let mut subjects = vec![self.player_entity()];
        subjects.extend(self.owned_pets().into_iter().map(|p| p.entity));
        subjects
    }

    /// Display string for `entity`'s current active status condition, if
    /// any — e.g. "Bleeding (2)" or "Stunned (1)", the number being battle
    /// rounds remaining. `None` if it has no active condition.
    pub(crate) fn status_label(&self, entity: Entity) -> Option<String> {
        let active = self.world.get::<StatusEffects>(entity)?.active?;
        Some(match active.kind {
            StatusKind::Bleed => format!("Bleeding ({})", active.remaining),
            StatusKind::Stun => format!("Stunned ({})", active.remaining),
        })
    }

    /// Adds `creature` (a tamed program you own) to your active battle
    /// party (see `resources::Party`), up to `MAX_PARTY_SIZE` at once.
    /// Clears an in-progress cronjob task on it first — a program can only
    /// be doing one job (working a structure, or fighting beside you) at a
    /// time.
    pub fn add_companion(&mut self, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != player {
            return Err("You don't control that program.".into());
        }
        if self.world.resource::<Party>().0.contains(&creature) {
            return Err("That program is already in your party.".into());
        }
        if self.world.resource::<Party>().0.len() >= MAX_PARTY_SIZE {
            return Err(format!(
                "Your party is full ({MAX_PARTY_SIZE} max) — stand one down first."
            ));
        }
        // The other door of the wield/party exclusion — see
        // `wield_program`, which stands a member down for the same reason.
        // Last, after every refusal above, so a party that turns out to be
        // full doesn't disarm the player on the way to saying so.
        if self.wielded_program() == Some(creature) {
            self.world.insert_resource(WieldedProgram(None));
            let name = self.creature_label(creature);
            self.log(format!("You lower {name} and it takes its own footing."));
        }
        self.world.entity_mut(creature).remove::<Task>();
        self.world.resource_mut::<Party>().0.push(creature);
        let name = self.creature_label(creature);
        self.log(format!("{name} falls in alongside you."));
        Ok(())
    }

    /// Equips `creature`, a tamed program you own, as your weapon (see
    /// `resources::WieldedProgram`). It lends you a standing ATK/DEF share
    /// (`wielded_stat_bonus`) and gives each of your strikes a chance to
    /// fire one of its installed routines. It is unharmed by this and gains
    /// nothing from it: a weapon, not a combatant.
    ///
    /// Mutually exclusive with both party membership and a worn weapon
    /// item, and the ordering is the whole of the safety here — the same
    /// argument `use_symlink` makes about `clear_stack`. Every refusal
    /// resolves before any state moves, so a rejected wield can neither
    /// strand a program between roles nor destroy the gear it displaced.
    ///
    /// Costs one turn, like `equip`. Step 4 calls `unequip`, which ticks on
    /// its own, so the tick below is conditional: one player action is one
    /// tick whether or not a weapon was displaced.
    pub fn wield_program(&mut self, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != player {
            return Err("You don't control that program.".into());
        }
        if self.wielded_program() == Some(creature) {
            return Err("You're already wielding that program.".into());
        }
        // The unequip comes first because it is the last thing here that can
        // still fail — `slot_occupant_with_mods` refuses a worn item that has
        // dropped out of the item set. Standing the program down before it
        // would leave a refused wield having emptied a party slot for nothing.
        let displaced = self
            .world
            .get::<Equipment>(player)
            .is_some_and(|e| e.weapon.is_some());
        if displaced {
            self.unequip(EquipmentSlot::Weapon)?;
        }
        self.remove_companion(creature);
        self.world.insert_resource(WieldedProgram(Some(creature)));
        let name = self.creature_label(creature);
        self.log(format!(
            "You take {name} in hand and level it like a blade."
        ));
        if !displaced {
            self.tick();
        }
        Ok(())
    }

    /// Puts the wielded program down, ending its bonus and its procs. It
    /// stays a tamed program you own and can be re-wielded or added to the
    /// party. Costs one turn, matching `unequip`.
    pub fn unwield_program(&mut self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let creature = self
            .wielded_program()
            .ok_or_else(|| "You aren't wielding a program.".to_string())?;
        self.world.insert_resource(WieldedProgram(None));
        let name = self.creature_label(creature);
        self.log(format!("You lower {name} and it takes its own footing."));
        self.tick();
        Ok(())
    }

    /// Stands `creature` down from the active party, if it's a member — it
    /// remains a tamed program, just no longer commandable in battle. A
    /// no-op (no log) if it wasn't in the party to begin with.
    pub fn remove_companion(&mut self, creature: Entity) {
        let was_present = {
            let mut party = self.world.resource_mut::<Party>();
            let before = party.0.len();
            party.0.retain(|&e| e != creature);
            party.0.len() != before
        };
        if was_present {
            let name = self.creature_label(creature);
            self.log(format!("{name} falls back from active duty."));
        }
    }

    /// The player-chosen name on `creature`, or `None` if it is still going
    /// by its species. Deliberately *not* `creature_name`, which always
    /// answers with something: a rename page seeded from that would put the
    /// species name in the edit buffer and turn "leave it alone" into
    /// "freeze today's species name onto it".
    pub fn custom_name(&self, creature: Entity) -> Option<String> {
        self.world.get::<CustomName>(creature).map(|n| n.0.clone())
    }

    /// Renames a tamed program you own, or clears the name back to its
    /// species (see `CustomName`). Works wherever the program is — in the
    /// party, on a cronjob, standing guard — because it changes nothing
    /// about where it is or what it's doing.
    ///
    /// `name` goes through `CustomName::sanitize`, the same rule
    /// `fuse_companions` applies, so blank or all-whitespace drops the
    /// override rather than storing an empty name.
    ///
    /// Refused during a battle, and the reason is the log rather than the
    /// roster: `resources::BattleTimeline` stores **rendered rows**, so a
    /// name changed mid-fight would leave the rewound narration and the
    /// live roster disagreeing about who is being hit. Like
    /// `move_party_member` it doesn't tick — naming a program is free.
    pub fn rename_companion(
        &mut self,
        creature: Entity,
        name: Option<String>,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != player {
            return Err("You don't control that program.".into());
        }
        let was = self.creature_label(creature);
        match CustomName::sanitize(name) {
            Some(name) => {
                self.world.entity_mut(creature).insert(CustomName(name));
                let now = self.creature_label(creature);
                self.log(format!("{was} answers to {now} from now on."));
            }
            None => {
                self.world.entity_mut(creature).remove::<CustomName>();
                let now = self.creature_label(creature);
                self.log(format!("{was} goes back to being a plain {now}."));
            }
        }
        Ok(())
    }

    /// Shifts `creature` one slot along the battle line. Front slots draw
    /// more fire (see `battle::slot_aggro_weight`), so this is how the
    /// player decides who tanks — the only other way to change the order is
    /// to stand a program down and re-add it, which appends to the back.
    ///
    /// Refused during a battle: `BattleState::planned` indexes `Party`
    /// positionally, so a swap mid-round would hand two slots each other's
    /// planned action. Like `add_companion` it doesn't tick — shuffling the
    /// roster is free.
    pub fn move_party_member(&mut self, creature: Entity, shift: SlotShift) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let party = &self.world.resource::<Party>().0;
        let slot = party
            .iter()
            .position(|&e| e == creature)
            .ok_or_else(|| "That program isn't in your party.".to_string())?;
        let target = match shift {
            SlotShift::Forward => slot.checked_sub(1),
            SlotShift::Back => (slot + 1 < party.len()).then_some(slot + 1),
        };
        let name = self.creature_label(creature);
        let Some(target) = target else {
            return Err(match shift {
                SlotShift::Forward => format!("{name} already leads the party."),
                SlotShift::Back => format!("{name} is already at the back."),
            });
        };
        self.world.resource_mut::<Party>().0.swap(slot, target);
        self.log(format!("{name} moves to slot {}.", target + 1));
        Ok(())
    }

    /// Fuses two of the player's tamed programs (`a` and `b`, any species,
    /// party members or not) into one new tamed program, consuming both.
    /// The result keeps the species (and so the moves/work aptitude) of
    /// whichever input is the higher level — ties favor `a` — at that same
    /// level, with each stat computed as `higher + lower / 2` so a fusion
    /// is always stronger than either input alone without simply summing
    /// them (which would make repeated fusion runaway). A resource sink for
    /// duplicate catches: there's no separate item cost, since losing two
    /// programs to gain one is the cost.
    ///
    /// Fusion depth is capped: neither input may already be `MAX_FUSIONS`
    /// deep (see `components::FusionCount`), and the result is one deeper
    /// than its deepest input.
    /// `custom_name`, if given, is trimmed and truncated to
    /// `MAX_CUSTOM_NAME_LEN` characters and becomes the fused program's
    /// display name everywhere (see `CustomName`) instead of its species
    /// name. Blank (or all-whitespace) is treated the same as `None`.
    pub fn fuse_companions(
        &mut self,
        a: Entity,
        b: Entity,
        custom_name: Option<String>,
    ) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if a == b {
            return Err("Pick two different programs to fuse.".into());
        }
        let player = self.player_entity();
        for e in [a, b] {
            let owner = self
                .world
                .get::<Tamed>(e)
                .ok_or_else(|| "Both programs must be compiled under your control.".to_string())?
                .owner;
            if owner != player {
                return Err("You don't control both programs.".into());
            }
        }
        for e in [a, b] {
            if self.fusion_count(e) >= MAX_FUSIONS {
                let name = self.creature_label(e);
                return Err(format!(
                    "{name} has already been fused {MAX_FUSIONS} times — it can't be fused again."
                ));
            }
        }
        let fused_depth = self.fusion_count(a).max(self.fusion_count(b)) + 1;
        let (species_a, exp_a, stats_a, potential_a) = (
            self.world.get::<Creature>(a).unwrap().species.clone(),
            *self.world.get::<Experience>(a).unwrap(),
            *self.world.get::<Stats>(a).unwrap(),
            self.world
                .get::<Potential>(a)
                .copied()
                .unwrap_or(Potential::NEUTRAL),
        );
        let (species_b, exp_b, stats_b, potential_b) = (
            self.world.get::<Creature>(b).unwrap().species.clone(),
            *self.world.get::<Experience>(b).unwrap(),
            *self.world.get::<Stats>(b).unwrap(),
            self.world
                .get::<Potential>(b)
                .copied()
                .unwrap_or(Potential::NEUTRAL),
        );
        let (species_id, level) = if exp_a.level >= exp_b.level {
            (species_a, exp_a.level)
        } else {
            (species_b, exp_b.level)
        };
        let species = self
            .world
            .resource::<SpeciesDb>()
            .get(&species_id)
            .cloned()
            .ok_or_else(|| "That species is no longer available.".to_string())?;

        fn fuse_stat(x: i32, y: i32) -> i32 {
            x.max(y) + x.min(y) / FUSION_LESSER_STAT_DIVISOR
        }
        let fused_hp = fuse_stat(stats_a.max_hp, stats_b.max_hp);
        let fused_atk = fuse_stat(stats_a.atk, stats_b.atk);
        let fused_def = fuse_stat(stats_a.def, stats_b.def);
        let fused_potential = Potential::averaged(potential_a, potential_b);

        let name_a = self.creature_label(a);
        let name_b = self.creature_label(b);
        let lost = self.fusion_routine_losses(a, b);
        self.world
            .resource_mut::<Party>()
            .0
            .retain(|&e| e != a && e != b);
        self.world.despawn(a);
        self.world.despawn(b);

        let final_name = CustomName::sanitize(custom_name);

        let player_pos = *self.world.get::<Position>(player).unwrap();
        let mut fused = self.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Glyph {
                ch: species.glyph,
                color: species.color,
            },
            Stats {
                hp: fused_hp,
                max_hp: fused_hp,
                atk: fused_atk,
                def: fused_def,
            },
            fused_potential,
            Tamed { owner: player },
            Experience {
                level,
                xp: 0,
                xp_to_next: progression::xp_for_level(level),
            },
            ZonePortal(1),
            StatusEffects::default(),
            FusionCount(fused_depth),
        ));
        let fused_entity = fused.id();
        if let Some(name) = &final_name {
            fused.insert(CustomName(name.clone()));
        }
        self.install_innate_routines(fused_entity);
        self.log(match &final_name {
            Some(name) => format!(
                "You fuse {name_a} and {name_b} into {name}, a new {}.",
                species.name
            ),
            None => format!(
                "You fuse {name_a} and {name_b} into a new {}.",
                species.name
            ),
        });
        // Filtered against the freshly installed kit rather than logged
        // as-is: an ability on `lost` can still land in the result if the
        // winning species happens to declare it innately, and this is the
        // one place that distinction is knowable — before this, only the
        // input kits exist; after, only the output does.
        let new_kit = self
            .world
            .get::<Routines>(fused_entity)
            .map(|r| r.0.clone())
            .unwrap_or_default();
        let truly_lost: Vec<&str> = lost
            .iter()
            .filter(|a| !new_kit.contains(&a.id))
            .map(|a| a.name.as_str())
            .collect();
        if !truly_lost.is_empty() {
            self.log(format!(
                "Routines lost in the fusion: {}.",
                truly_lost.join(", ")
            ));
        }
        Ok(())
    }
}
