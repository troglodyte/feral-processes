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
        let needs = self.world.get::<PowerReserve>(player).unwrap();
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
        let def = self.effective_mitigation(player);
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
        // a special copy is cargo like any other, and every screen that
        // lists "what does the player have" wants both. Rare tier and then
        // fusion tier are the tiebreaks inside a category, so an item's
        // special rows sit beside its plain one instead of drifting apart as
        // categories are re-sorted.
        let special = self.world.get::<GearCopies>(player);
        let mut inventory: Vec<InventoryRow> = inv
            .items
            .iter()
            .filter(|(item, _)| !db.get(item.as_str()).is_some_and(|d| d.banked))
            .map(|(item, qty)| InventoryRow {
                copy: GearCopy::plain(item.clone()),
                qty: *qty,
            })
            .chain(
                special
                    .into_iter()
                    .flat_map(|f| f.copies.iter())
                    .map(|(copy, qty)| InventoryRow {
                        copy: copy.clone(),
                        qty: *qty,
                    }),
            )
            .collect();
        inventory.sort_by_key(|row| {
            (
                self.category_sort_key(&row.copy.item),
                row.copy.rarity,
                row.copy.tier,
            )
        });
        PlayerStatus {
            position: (pos.x, pos.y),
            hp: stats.hp,
            max_hp: stats.max_hp,
            atk,
            def,
            strength: stats.max_hp + atk + def,
            decompiler,
            power: needs.get(),
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

    /// `creature_name`, rare-tier prefixed and zone-tagged, falling back to
    /// a generic label if `entity` isn't a `Creature`.
    ///
    /// The prefix goes here rather than in `zone_tagged_name` deliberately.
    /// That one is also called directly by `EnemyGroupView::species_name`
    /// (`game/combat_round.rs`), and the battle roster draws its name into a
    /// fixed `NAME_W` cell that "Overclocked Scrapper 2" overflows — the
    /// roster carries the tier as its own short tag instead, outside the
    /// column. A `CustomName` gets the prefix too, which is right: renaming
    /// a program does not make it ordinary.
    pub(crate) fn creature_label(&self, entity: Entity) -> String {
        match self.creature_name(entity) {
            Some(name) => {
                let named = match self.rarity_of(entity).label() {
                    Some(tier) => format!("{tier} {name}"),
                    None => name,
                };
                self.zone_tagged_name(entity, named)
            }
            None => "Program".to_string(),
        }
    }

    /// The rare-spawn tier of `entity`, or `Ordinary` for anything without
    /// the component — which is every ordinary creature, every structure and
    /// every hand-built test fixture. The one reader, so no caller has to
    /// know the component is optional.
    pub(crate) fn rarity_of(&self, entity: Entity) -> Rarity {
        self.world
            .get::<Rarity>(entity)
            .copied()
            .unwrap_or_default()
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

    /// Whether `entity` is a boss — either because it was spawned as one
    /// (`components::Boss`, written at every boss spawn, rolled or apex) or
    /// because its species is apex (`SpeciesDef::is_boss`).
    ///
    /// The species half stays because a fixture that hand-spawns an apex
    /// species outside `spawn_pack` never gets a component, and must still be
    /// a boss. `false` for anything that isn't a creature, or whose species
    /// failed to resolve.
    pub(crate) fn is_boss_creature(&self, entity: Entity) -> bool {
        if self.world.get::<crate::components::Boss>(entity).is_some() {
            return true;
        }
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
            def: stats.mitigation,
            power: stats.power(),
            status: self.status_label(entity),
            ability: self.ability_label(entity),
            gear: self.gear_tag(entity),
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

    /// The zone tier `entity` is scaled to (`components::ZonePortal`), or 1
    /// for anything without the component — a hand-spawned fixture, or a
    /// creature from a save predating it. One reader for the same reason
    /// `rarity_of` is: absent-means-tier-1 is a rule, and four call sites
    /// each spelling `map_or(1, ..)` is four chances to pick a different one.
    pub(crate) fn zone_tier(&self, entity: Entity) -> u32 {
        self.world.get::<ZonePortal>(entity).map_or(1, |z| z.0)
    }

    /// How many of `entity`'s zone tiers were bought with Recompile Kernels
    /// rather than earned by being tamed that deep — 0 for anything without
    /// the component. Same one-reader argument as `rarity_of`.
    pub(crate) fn purchased_tiers(&self, entity: Entity) -> u32 {
        self.world.get::<PurchasedTiers>(entity).map_or(0, |t| t.0)
    }

    /// How many percentage upgrades have been spent on `entity`, 0 for
    /// anything without the component — every program that has never been
    /// refactored, and every hand-built test fixture. The one reader, so no
    /// caller has to know it is optional.
    pub(crate) fn refactor_count(&self, entity: Entity) -> u32 {
        self.world
            .get::<Refactors>(entity)
            .map(|r| r.0)
            .unwrap_or(0)
    }

    /// The highest level `entity` may reach: `CREATURE_MAX_LEVEL` plus
    /// `LEVELS_PER_RING` for every Kernel Ring open on it (see
    /// `components::KernelRing`; absent means none).
    ///
    /// The one expression of a companion's ceiling, so no call site
    /// multiplies rings out itself. Two callers deliberately do *not* use it:
    /// `systems.rs`'s cronjob payout, whose own `WORK_XP_LEVEL_CAP` guard
    /// stops a posted worker below the base cap already — which is what keeps
    /// a developed program from being ground up at a Mining Node — and the two
    /// arena sites, which have no entity and take
    /// `tuning::absolute_companion_level_cap()` instead.
    pub fn companion_level_cap(&self, entity: Entity) -> u32 {
        let rings = self.world.get::<KernelRing>(entity).map_or(0, |r| r.0);
        crate::tuning::CREATURE_MAX_LEVEL + rings * crate::tuning::LEVELS_PER_RING
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
                    def: stats.mitigation,
                    power: stats.power(),
                    party_slot: slot_of(&entity).map(|s| s as u32),
                    activity: self.program_activity(entity),
                    quality: self.potential_quality_label(entity),
                    fusions: self.fusion_count(entity),
                    refactors: self.refactor_count(entity),
                    ring: self.world.get::<KernelRing>(entity).map_or(0, |r| r.0),
                    talents: self.talent_points(entity).spent,
                    rarity: self.rarity_of(entity),
                    wielded: self.wielded_program() == Some(entity),
                    gear: self.gear_tag(entity),
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
            StatusKind::Exposed => format!("Exposed ({})", active.remaining),
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
        // Both halves of the party/staff exclusion, and the `Task` half is
        // not enough on its own: a program left marked `BaseStaff` would be
        // re-posted by `schedule_base_labour` on the very next tick, so the
        // marker has to come off with the job it authorises.
        self.world
            .entity_mut(creature)
            .remove::<Task>()
            .remove::<BaseStaff>();
        self.world.resource_mut::<Party>().0.push(creature);
        let name = self.creature_label(creature);
        self.log(format!("{name} falls in alongside you."));
        Ok(())
    }

    /// Gives `creature`, a tamed program you own, to the base — see
    /// `components::BaseStaff`. From here the work order scheduler decides
    /// where it stands; the player does not post it by hand.
    ///
    /// Staff and party are disjoint sets, so this stands the program down
    /// as a companion exactly as `assign_cronjob` used to. Every refusal
    /// runs before anything is written, the same ordering argument
    /// `install_routine` makes about the disk.
    pub fn assign_base_staff(&mut self, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let owner = self
            .world
            .get::<Tamed>(creature)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != self.player_entity() {
            return Err("You don't control that program.".into());
        }
        if self.world.get::<BaseStaff>(creature).is_some() {
            return Err("That program is already assigned to the base.".into());
        }
        // Same door `add_companion` closes from the other side: a wielded
        // program is a weapon in your hands, not a body in the base.
        if self.wielded_program() == Some(creature) {
            self.world.insert_resource(WieldedProgram(None));
            let name = self.creature_label(creature);
            self.log(format!("You lower {name} and it takes its own footing."));
        }
        if self.world.resource::<Party>().0.contains(&creature) {
            self.world
                .resource_mut::<Party>()
                .0
                .retain(|&e| e != creature);
            self.log_base("It stands down as your companion to work the base.");
        }
        self.world.entity_mut(creature).insert(BaseStaff);
        let name = self.creature_label(creature);
        self.log_base(format!("{name} reports to the base."));
        Ok(())
    }

    /// Takes `creature` back off the base staff. Its posting goes with it —
    /// a program off the pool is not one the scheduler may move, so leaving
    /// the `Task` behind would strand a body on a machine nothing was
    /// tracking.
    pub fn release_base_staff(&mut self, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self.world.get::<BaseStaff>(creature).is_none() {
            return Err("That program isn't on the base staff.".into());
        }
        self.world
            .entity_mut(creature)
            .remove::<BaseStaff>()
            .remove::<Task>()
            .remove::<Carrying>();
        let name = self.creature_label(creature);
        self.log_base(format!("{name} stands down from the base."));
        Ok(())
    }

    /// Every program on the base staff, in a **stable total order**.
    ///
    /// Sorted rather than left in query order for the reason
    /// `assembler_system` sorts its machines: bevy's iteration order is not
    /// stable, and a scheduler that filled wants in a different order
    /// between runs is a flaky test and a base that behaves differently
    /// after a reload.
    pub fn base_staff(&self) -> Vec<Entity> {
        let mut staff: Vec<Entity> = self
            .world
            .iter_entities()
            .filter(|e| e.contains::<BaseStaff>() && e.contains::<Tamed>())
            .map(|e| e.id())
            .collect();
        staff.sort();
        staff
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
        // still fail — `slot_occupant` refuses a worn item that has
        // dropped out of the item set. Standing the program down before it
        // would leave a refused wield having emptied a party slot for nothing.
        let displaced = self
            .world
            .get::<Equipment>(player)
            .is_some_and(|e| e.weapon.is_some());
        if displaced {
            self.unequip(player, EquipmentSlot::Weapon)?;
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

    /// What `wearer` has on in `slot`, or `None` for an empty slot — and for
    /// an entity with no `Equipment` at all, which is what a program that has
    /// never been geared looks like (see `Game::equip`).
    ///
    /// The one public read of a *companion's* loadout. The player's own is
    /// already three fields on `PlayerStatus`, and that stays: this exists so
    /// the swap picker can measure the worn copy of whichever wearer it was
    /// opened for, rather than matching on the player's three by slot.
    pub fn worn(&self, wearer: Entity, slot: EquipmentSlot) -> Option<EquippedItem> {
        self.world.get::<Equipment>(wearer)?.get(slot)
    }

    /// `wearer`'s loadout as one cell — `w|a|m`, with a filled slot named by
    /// its `EquipmentSlot::initial` and an empty one held open by a dot.
    ///
    /// Fixed width on purpose: the roster and the status panel both list
    /// several programs at once, and a cell that shrank when a slot was bare
    /// would leave the marks unaligned down the list, which is the one thing
    /// this is for. It also cannot be built in a renderer — two screens draw
    /// it, and a program's loadout must not read one way in the panel and
    /// another in the roster it was opened from.
    pub(crate) fn gear_tag(&self, wearer: Entity) -> String {
        EquipmentSlot::ALL
            .iter()
            .map(|&slot| match self.worn(wearer, slot) {
                Some(_) => slot.initial(),
                None => '.',
            })
            .map(String::from)
            .collect::<Vec<_>>()
            .join("|")
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
        // Before the snapshot below, and that ordering is the whole
        // correctness argument: `fuse_stat` combines both parents' `Stats`
        // into the child's, so a gear bonus still sitting in one of them is
        // banked into a new program permanently, with no record of where it
        // came from. This function does its own reap and never calls
        // `dissolve_tamed_program`, so it carries its own strip.
        for e in [a, b] {
            self.strip_gear(e);
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
        // The dominant parent — whose species and level the child takes — is
        // also the one whose *development* it inherits. A ring cost a lair
        // guardian and the talents cost the levels it bought, so neither may
        // evaporate here; and taking only one parent's is what stops fusion
        // being a way to launder two developed programs into one.
        let (species_id, level, dominant) = if exp_a.level >= exp_b.level {
            (species_a, exp_a.level, a)
        } else {
            (species_b, exp_b.level, b)
        };
        let fused_ring = self.world.get::<KernelRing>(dominant).copied();
        let fused_talents = self.world.get::<Talents>(dominant).cloned();
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
        let fused_def = fuse_stat(stats_a.mitigation, stats_b.mitigation);
        let fused_potential = Potential::averaged(potential_a, potential_b);
        // The better of the two parents, the same shape `FusionCount` takes
        // (`max(a, b) + 1`) and for the same reason: fusing away an
        // Overclocked program must not quietly launder it into an ordinary
        // one. Averaging it the way `Potential` is averaged would need a
        // tier between the tiers, which the enum deliberately doesn't have.
        //
        // Carried as a *tag* only — `fuse_stat` above already derives the
        // stats from parents whose numbers include their own multiplier, so
        // applying `stat_mult` here would pay for the tier twice. See
        // `Rarity`'s doc.
        let fused_rarity = self.rarity_of(a).max(self.rarity_of(b));
        // Both take the better parent for exactly the argument above, and
        // both used to be dropped here: the tier was hardcoded to 1, which
        // was harmless while it was a display tag and stopped being harmless
        // the moment `refactor_companion` multiplied current stats and capped
        // itself against it. `fuse_stat` carries the parents' numbers
        // forward, so resetting either bound turns fuse → bump → fuse into an
        // unbounded stat loop, and lets a fusion launder a program that has
        // spent all five upgrade slots back into a fresh one.
        let fused_zone = self.zone_tier(a).max(self.zone_tier(b));
        let fused_refactors = self.refactor_count(a).max(self.refactor_count(b));
        let fused_purchased = self.purchased_tiers(a).max(self.purchased_tiers(b));

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
        // Read before the spawn borrow below. This is the door with no
        // compiler barrier behind it — fusion assembles its own component
        // list rather than going through `adopt_program` — so it takes the
        // shared `roster_parts` and overrides only the one piece that is
        // genuinely fusion's own, the child's level.
        let parts = self.roster_parts();
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
                mitigation: fused_def,
            },
            fused_potential,
            ZonePortal(fused_zone),
            StatusEffects::default(),
            FusionCount(fused_depth),
            fused_rarity,
            Refactors(fused_refactors),
            PurchasedTiers(fused_purchased),
        ));
        fused.insert(parts);
        // After `parts`, which carries a level-1 default: a fused child's
        // level is derived from its parents.
        fused.insert(Experience {
            level,
            xp: 0,
            xp_to_next: progression::xp_for_level(level),
        });
        // Inserted only when present, the absent-means-none idiom
        // `components::KernelRing` documents — and never re-applied: a `Stat`
        // talent's effect is already in `fuse_stat`'s inputs, which were the
        // parents' own numbers.
        if let Some(ring) = fused_ring {
            fused.insert(ring);
        }
        if let Some(talents) = fused_talents {
            fused.insert(talents);
        }
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
