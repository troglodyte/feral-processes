//! The player's own roster — status, the programs in the party and the
//! bank, and moving programs between them.

use crate::tuning::{FUSION_LESSER_STAT_DIVISOR, MAX_FUSIONS};
use crate::*;

/// Which of the four roles a program you own is filling.
///
/// The roles are **disjoint and exhaustive**: every program on the roster
/// is in exactly one, and there is no "owned but doing nothing" state —
/// that is the whole of the auto-staffing rule. Derived through
/// `Game::program_role` and never stored, so the party roster and the wield
/// stay the only things that decide it.
///
/// Staff and `resources::Party` therefore remain disjoint sets drawing on
/// one `pet_capacity` roster, which is the tension `tuning.rs` already
/// states: every program working the base is one absent from the party.
/// What changed is that nothing has to *maintain* that any more.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProgramRole {
    /// Carried as the player's weapon — `resources::WieldedProgram`. Takes
    /// precedence over the other two, and is why a wielded program is not
    /// swept up by the auto-staffing rule despite being outside the party.
    Wielded,
    /// Fighting alongside the player — `resources::Party`.
    InParty,
    /// Away from the base on a sortie — `resources::Sorties`. Ranked
    /// between the party and the labour pool: a dispatched program is not
    /// staff, which is what takes it out of `schedule_base_labour`,
    /// `drift_idle_staff`, `base_entropy_system`, `needs_drain_system` and
    /// the surface map in one edit rather than five.
    Sortie,
    /// Everything else you own: the base's labour pool, posted and unposted
    /// by `game::base::work_orders`'s scheduler and by nothing else.
    Staff,
}

impl ProgramRole {
    /// Where a run of this role sits on the roster screen, and so the order
    /// `Game::owned_pets` groups the list into.
    ///
    /// **Not the declaration order**, which is a precedence: `Wielded` wins
    /// the role *itself* because a wielded program is outside the party and
    /// must not be swept up as staff, while the party leads the *screen*
    /// because slot order is mechanical and arranging it is what the screen
    /// is for. Exhaustive, `cell_mark`'s rule — a fifth role reaching a
    /// fallback here would file its programs under whichever heading sorted
    /// in front of them.
    pub fn roster_rank(self) -> u8 {
        match self {
            ProgramRole::InParty => 0,
            ProgramRole::Wielded => 1,
            ProgramRole::Sortie => 2,
            ProgramRole::Staff => 3,
        }
    }
}

/// The role rule itself, over values rather than a `Game`.
///
/// A free function for `game::stack::surfaced`'s reason: `base_entropy_system`
/// is a bevy system and has no `Game` to ask, but must not carry a second
/// copy of who counts as staff — a cell reverting under a body seals it in
/// solid rock for the rest of the run, and a drifted copy is how that comes
/// back. `Game::program_role` is the other applier.
pub(crate) fn role_of(
    creature: Entity,
    owner: Entity,
    player: Entity,
    party: &Party,
    wielded: Option<Entity>,
    sorties: &crate::resources::Sorties,
) -> Option<ProgramRole> {
    if owner != player {
        return None;
    }
    if wielded == Some(creature) {
        return Some(ProgramRole::Wielded);
    }
    if party.0.contains(&creature) {
        return Some(ProgramRole::InParty);
    }
    if sorties.contains(creature) {
        return Some(ProgramRole::Sortie);
    }
    Some(ProgramRole::Staff)
}

/// The four resources that decide a role, as one system parameter.
///
/// A bevy system that narrows through `role_of` needs every one of them and
/// nothing else out of them, so they travel together — which is what keeps
/// a fifth role source from being three signature edits and a lint. A thin
/// adapter and never a second copy of the rule: `of` is one call to the
/// free function above.
#[derive(bevy_ecs::system::SystemParam)]
pub struct Roles<'w> {
    player: Res<'w, PlayerEntity>,
    party: Res<'w, Party>,
    wielded: Res<'w, WieldedProgram>,
    sorties: Res<'w, crate::resources::Sorties>,
}

impl Roles<'_> {
    pub(crate) fn of(&self, creature: Entity, owner: Entity) -> Option<ProgramRole> {
        role_of(
            creature,
            owner,
            self.player.0,
            &self.party,
            self.wielded.0,
            &self.sorties,
        )
    }
}

impl Game {
    /// The player's drawn 16x16 avatar, or `None` for a player who never
    /// opened the editor.
    ///
    /// The same read `views::PlayerLook::icon` makes, hoisted because the
    /// frontend needs it *before* it draws: the drawing has to be on the
    /// GPU by the time the map asks for it, and the map's own view is built
    /// inside the draw call that would already be too late to upload from.
    pub fn player_icon(&self) -> Option<&icon::PlayerIcon> {
        self.world
            .get::<PlayerIdentity>(self.player_entity())
            .and_then(|i| i.icon.as_ref())
    }

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
        let mitigation = self.effective_mitigation(player);
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
            mitigation,
            // `Stats::power` over the player's *effective* numbers — the
            // same scalar, not a second spelling of it.
            strength: Stats {
                atk,
                mitigation,
                ..*stats
            }
            .power(),
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
    pub fn creature_label(&self, entity: Entity) -> String {
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
            mitigation: stats.mitigation,
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

    /// The highest level anyone in this run may reach — the player and every
    /// companion alike.
    ///
    /// Linear in the zone, `ZoneLevel::stat_multiplier`'s reason: this curve
    /// races the enemy curve in the player's favour, and two curves of
    /// different order have an end wherever the coefficients are put.
    ///
    /// **It takes no entity, and that is the design.** One number for the
    /// whole party is what makes a companion worth developing — under the
    /// old per-entity ceiling a companion was capped six levels under the
    /// player and a Kernel Ring bought the difference back.
    ///
    /// It reads `ZoneLevel` and nothing else. Depth deliberately does not
    /// lift it: a deep frame is harder because what lives in it is scaled,
    /// not because the party is let out of the cap to meet it.
    pub fn level_cap(&self) -> u32 {
        crate::tuning::zone_level_cap(self.world.resource::<ZoneLevel>().0)
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
        // Grouped by role, then the party in slot order, then the name.
        //
        // The party leads because that order is mechanical: the front slot
        // draws the most fire (see `battle::slot_aggro_weight`) and the
        // companion screen exists to arrange it. Behind them the *role*
        // decides, so the roster is a run per `ProgramRole` and the screen can
        // head each run rather than mixing a program away on a sortie in
        // among the base staff. Inside a run the name settles it, since bevy's
        // query order is not stable and the four other screens reading this
        // list — fuse, extract, routines, manifest — have no slot to show and
        // were getting no order at all.
        owned.sort_by_key(|e| {
            (
                self.program_role(*e)
                    .map_or(u8::MAX, ProgramRole::roster_rank),
                slot_of(e).unwrap_or(usize::MAX),
                self.creature_label(*e),
            )
        });
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
                    mitigation: stats.mitigation,
                    power: stats.power(),
                    party_slot: slot_of(&entity).map(|s| s as u32),
                    role: self.program_role(entity)?,
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

    /// Every owned program that could be spent on a build of `tier`, in
    /// `owned_pets` order so the picker agrees with every other roster
    /// screen.
    ///
    /// `>=` and never `==`: a run whose roster has outgrown zone 1 must
    /// still be able to raise a Mk1, or deep play locks itself out of
    /// building.
    ///
    /// **Depth is not the whole filter.** A program's role is derived and
    /// there is no "owned but idle" state, so an unfiltered list offers the
    /// weapon in the player's hand and a body that is halfway across a
    /// sortie. Each exclusion below is for its own reason, and none is
    /// cosmetic: a carrier's goods are destroyed by freeing it, let alone
    /// despawning it; a sortied program is away and cannot be reached; a
    /// `Downed` one is the roster slot a wipe is supposed to cost.
    ///
    /// **The only derivation of this list.** The renderer draws what
    /// app-core counts and filters nothing itself.
    pub fn programs_for_build(&mut self, tier: u32) -> Vec<PetInfo> {
        self.owned_pets()
            .into_iter()
            .filter(|p| self.zone_tier(p.entity) >= tier)
            .filter(|p| !p.wielded)
            .filter(|p| {
                !self
                    .world
                    .resource::<crate::resources::Sorties>()
                    .contains(p.entity)
            })
            .filter(|p| {
                self.world
                    .get::<crate::components::Downed>(p.entity)
                    .is_none()
            })
            .filter(|p| self.world.get::<Carrying>(p.entity).is_none())
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
        // Who is in your party is decided at base. Losing a party in the
        // Stack means walking out alone — that is the point of the guard,
        // and it is only survivable because a Forgiving death benches a
        // program rather than destroying it.
        self.require_base()?;
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
        // A benched program is not a body you can press back into service.
        // The refusal names the two things that *do* free the roster slot
        // it is still holding, because a player with a full roster, every
        // program down and no Bay standing has an errand rather than a dead
        // end — see `components::Downed`.
        if self
            .world
            .get::<crate::components::Downed>(creature)
            .is_some()
        {
            return Err(
                "That program is down and needs a Repair Bay. You can still sell it \
                 or extract a routine from it."
                    .into(),
            );
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
        // The `Task` still has to be cleared by hand even though the role
        // is derived: `ProgramRole::InParty` stops the scheduler *posting*
        // this program again, but it does not undo a posting already made,
        // and a party member left holding a `Task` is a body the base still
        // counts as standing at a machine.
        self.world.entity_mut(creature).remove::<Task>();
        self.world.resource_mut::<Party>().0.push(creature);
        let name = self.creature_label(creature);
        self.log(format!("{name} falls in alongside you."));
        Ok(())
    }

    /// What role `creature` is in, or `None` if it is not a program you
    /// own. **The one derivation of that question**, and the one statement
    /// of the precedence between the roles.
    ///
    /// Roles are derived rather than stored, and they are disjoint by
    /// construction rather than by upkeep: `Party` and `WieldedProgram` are
    /// already authoritative about the two roles that are *chosen*, so
    /// `Staff` is what is left over. There is deliberately no "owned but
    /// idle" state — a program you are not fighting with and not holding is
    /// working the base, which is why nothing assigns staff by hand.
    ///
    /// The precedence used to be stated twice: `program_activity` ordered
    /// wielded ahead of party in its own prose, and `assign_base_staff`
    /// restated the same exclusion from the other side. A fourth role added
    /// to this enum is one arm here and a compiler error at every reader.
    pub fn program_role(&self, creature: Entity) -> Option<ProgramRole> {
        let owner = self.world.get::<Tamed>(creature)?.owner;
        role_of(
            creature,
            owner,
            self.player_entity(),
            self.world.resource::<Party>(),
            self.wielded_program(),
            self.world.resource::<crate::resources::Sorties>(),
        )
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
            .filter(|e| e.contains::<Tamed>())
            .map(|e| e.id())
            .filter(|&e| self.program_role(e) == Some(ProgramRole::Staff))
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
    /// The player's own "stand this one down" — `remove_companion` with the
    /// base guard on it, and the party screen's one door.
    ///
    /// **The mover stays guard-free and this wraps it**, rather than the
    /// guard going inside: `wield_program` calls `remove_companion` to stand
    /// a member down before taking it as a weapon, so a `require_base` in
    /// there would refuse wielding in the field as a side effect, through a
    /// function the player never invoked. `take_from_adjacent` /
    /// `give_to_adjacent`'s exact shape, one axis along: the mover is
    /// guard-free by construction and the caller owns the refusal.
    pub fn stand_down_companion(&mut self, creature: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        self.require_base()?;
        self.remove_companion(creature);
        Ok(())
    }

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

    /// Retires `e` from the roster and hands back what it was, so an order
    /// that is called off can give it home again.
    ///
    /// The second spending door beside `fuse_companions`, and it follows
    /// that function's teardown — retain out of `Party`, then
    /// `world.despawn` — with the loose ends a fusion sacrifice does not
    /// normally have closed first. A program offered to a build can be in
    /// states a fusion input is not.
    ///
    /// **The snapshot comes first, before anything moves.** Three of
    /// `CreatureSave`'s fields are *roles*, read off `Party`,
    /// `WieldedProgram` and `Sorties` at the moment of the call (see
    /// `creature_save_for`). A teardown that ran before the snapshot would
    /// record a program that was in no party and wielded nothing, and
    /// `refund_program` would hand back a stranger.
    ///
    /// **`None` is a refusal, and nothing has moved.** The caller must
    /// treat it as an error and not file the order: a site spawned with
    /// `program: None` after a refused commit is a structure raised for
    /// free, and nothing fails to compile. `Home` is the only order that
    /// legitimately carries `None`, and it never calls this.
    ///
    /// **The refusals are a second line of defence, not the guard.**
    /// `owned_pets` already answers ownership and `programs_for_build`
    /// already withholds a sortied, downed or carrying program from the
    /// picker. All four are restated here because a rule that lives only in
    /// a frontend is a rule the second frontend skips, and the cheapest of
    /// these mistakes destroys a carrier's load outright.
    ///
    /// One state is *handled* rather than refused: the **party slot**.
    /// `Party` is a raw `Vec<Entity>` that outlives its members and is read
    /// by every battle round, every roster draw and every save, so a slot
    /// left pointing at a despawned program is a dangling reference in all
    /// three.
    ///
    /// The **wield** needs nothing done to it, and the omission is the
    /// design rather than an oversight: `wielded_program` filters
    /// `resources::WieldedProgram` through an existence check exactly so
    /// that every despawning path — sale, extraction, fusion, death, and now
    /// this one — is immune without knowing the feature exists, and its doc
    /// asks in as many words that no caller tidy that into an explicit
    /// clear. A stale id cannot alias a live program either, since an
    /// `Entity` carries a generation. The one raw read of the resource,
    /// `Roles`, compares it against creatures coming out of a live query,
    /// which a despawned entity is never in. `refund_program` puts the
    /// weapon back in the hand off the snapshot, so the round trip is
    /// lossless regardless.
    ///
    /// A **posting** needs nothing done to it either. Occupancy is read off the
    /// live `Task` components rather than cached on the structure (see
    /// `displace_task_holder`), and `Task`, `OffShift` and `Carrying` all
    /// live on the body, so the despawn takes the whole posting with it and
    /// no machine is left naming a dead worker. What outlives the program is
    /// other programs' `idled_with` memories, and those name it by
    /// `ProgramId` — which is exactly why `refund_program` must not mint a
    /// new one.
    ///
    /// No log line: the caller announces the commit, naming the structure
    /// and the program in one sentence, and a "you lower it" line from here
    /// would narrate a program that no longer exists.
    pub(crate) fn commit_program(&mut self, e: Entity) -> Option<save::CreatureSave> {
        // Ownership is the outermost guard: nothing about an `Entity`
        // argument says the thing is yours, and a wild creature standing in
        // the base is one of these too.
        if self.world.get::<Tamed>(e).map(|t| t.owner) != Some(self.player_entity()) {
            return None;
        }
        // Away, and unreachable — and `Sorties` is the third resource
        // holding a raw `Entity`: committing one leaves a squad counting
        // down around a body that is not there.
        if self
            .world
            .resource::<crate::resources::Sorties>()
            .contains(e)
        {
            return None;
        }
        // The roster slot a wipe is meant to cost. Spending a downed
        // program and cancelling the order would hand it back whole, which
        // is a repair with no Repair Bay.
        if self.world.get::<crate::components::Downed>(e).is_some() {
            return None;
        }
        // The load is destroyed by freeing the carrier, let alone by
        // despawning it, and nothing in the base has a claim on it to
        // return it to.
        if self.world.get::<Carrying>(e).is_some() {
            return None;
        }
        let snapshot = self.creature_save_for(e)?;
        self.world.resource_mut::<Party>().0.retain(|&x| x != e);
        self.world.despawn(e);
        Some(snapshot)
    }

    /// Puts a committed program back on the roster as it left, and in the
    /// role it left from.
    ///
    /// Through `spawn_creature_from_save`, and so through the same component
    /// set `roster_parts` mints — the one barrier every door into the roster
    /// passes. A program that came back around it would be short components
    /// and silently remember nothing, and nothing would fail to compile.
    ///
    /// The snapshot's own `ProgramId` is kept rather than reissued, which is
    /// load-bearing in both directions: a fresh id orphans this program's
    /// memories *and* every other program's memories naming it as their
    /// subject. Memory timestamps survive for the same reason
    /// `spawn_creature_from_save` states — intensity is derived from
    /// `GameClock` on every read, so re-stamping would make a refund
    /// *deepen* an old grudge.
    ///
    /// **`None` means the snapshot names a species this install no longer
    /// ships** — a `.ron` deleted between sessions, the one failure
    /// `spawn_creature_from_save` has. It is returned rather than unwrapped
    /// because deleting a species file is a supported thing to do: a cancel
    /// that panicked would take the run down over an order the player was
    /// calling off anyway.
    ///
    /// **The roles are restored, not just the entity.** The snapshot was
    /// taken before the commit retired the program, so it records what it
    /// was doing — and a cancelled order that quietly disarmed the player or
    /// emptied a battle slot would be a second cost the cancel never
    /// advertised. Both are conditional on the world still having room,
    /// because time passed while the order stood: the hand may be full and
    /// the party may have filled up behind it. Neither may be forced —
    /// `BattleState::planned` indexes `Party` positionally, so an overfilled
    /// party is a sixth slot nothing plans for.
    pub(crate) fn refund_program(&mut self, c: &save::CreatureSave) -> Option<Entity> {
        let player = self.player_entity();
        // Seeded from the live counter and written back below, rather than
        // asserting `c.program_id != 0` on the way in. `spawn_creature_from_save`
        // mints into its context and never into the resource — `Game::load`
        // does that write itself — so a scratch context that was built at
        // zero and dropped would both hand out a colliding id *and* lose the
        // one it minted. A committed program always carries a real id and so
        // never mints at all; seeding is what keeps that from being the only
        // reason this is safe.
        let next_program_id = self.world.resource::<crate::resources::NextProgramId>().0;
        let mut ctx = crate::game::lifecycle::CreatureRestore::new(
            player,
            next_program_id,
            // No nests to offer: that map is read only on the wild arm, and
            // a committed program is tamed by construction.
            std::collections::HashMap::new(),
        );
        let back = self.spawn_creature_from_save(c, &mut ctx)?;
        // Destructured rather than read field by field, `Game::load`'s
        // shape one step stricter: every field is named and **no `..`**, so
        // a new piece of deferred work on `CreatureRestore` stops this
        // function compiling until a refund decides what it owes it, rather
        // than being silently dropped on the floor. The two inputs are
        // discarded by name for the same reason.
        let crate::game::lifecycle::CreatureRestore {
            player: _,
            nest_positions: _,
            next_program_id,
            party_slots,
            sortie_members,
            pending_cronjobs,
            pending_patrols,
        } = ctx;
        self.world
            .insert_resource(crate::resources::NextProgramId(next_program_id));
        // A sortied program is refused at the commit door, so a snapshot
        // riding a build request can never carry one. Asserted rather than
        // applied: a non-empty list here means the commit guard has gone,
        // and quietly rebuilding a squad around a resurrected member would
        // hide that.
        debug_assert!(
            sortie_members.is_empty(),
            "a sortied program is never committed, so a refund never restores one"
        );
        // Wild-only — `spawn_creature_from_save` fills this on its untamed
        // arm, and `commit_program` refuses anything the player does not own.
        debug_assert!(pending_patrols.is_empty(), "a refunded program is tamed");
        // Deliberately dropped, and not for want of a structure to resolve
        // it against. `schedule_base_labour` posts staff again on the next
        // tick, and re-inserting the snapshot's `Task` into a base that has
        // moved on could put two bodies on one machine — the invariant
        // `displace_task_holder` exists to hold. The most a refund costs is
        // one part-finished tick of work, which is what a reload already
        // costs.
        drop(pending_cronjobs);
        // The wield first, and the two arms are exclusive by construction:
        // `wield_program` stands a member down, so a snapshot is never both
        // wielded and holding a slot.
        if c.wielded && self.wielded_program().is_none() {
            self.world.insert_resource(WieldedProgram(Some(back)));
        // `first` and not a loop: one snapshot spawns one creature, so this
        // carries at most one slot.
        } else if let Some(&(slot, member)) = party_slots.first() {
            let party = &self.world.resource::<Party>().0;
            if party.len() < MAX_PARTY_SIZE {
                // At the recorded index where the party still has one, so a
                // program that led the line comes back leading it — and
                // clamped to the end where the line has since shortened.
                let at = (slot as usize).min(party.len());
                self.world.resource_mut::<Party>().0.insert(at, member);
            }
        }
        Some(back)
    }
}
