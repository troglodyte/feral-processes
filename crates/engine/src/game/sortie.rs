//! Sending a squad of base staff away from the base to fight for a while.
//!
//! The whole feature's behaviour: reach, the board, dispatch, the trip and
//! the return. `crate::sorties` is the catalogue and holds no game logic;
//! `resources::Sortie` is the in-flight record.

use bevy_ecs::prelude::*;

use crate::Game;
use crate::components::{Glyph, Position, Stats, Structure};
use crate::items::ItemId;

/// Whether the player can read the board, and whether they can sign for a
/// squad.
///
/// Three states rather than two booleans, for `NoPost::BoxedIn`'s reason:
/// "no Relay built" and "not standing in the base" leave the player
/// different errands, and a screen that cannot tell them apart says the
/// wrong sentence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DispatchReach {
    NoRelay,
    OffBase,
    AtRelay,
}

/// Why a dispatch was refused.
///
/// Typed rather than a `String`, `ContractRefusal`'s reason: each of these
/// leaves the player a different errand, and app-core words them for the
/// screen. `NotStaff` and `Downed` are distinct for that reason too — the
/// first wants the program unpartied, the second wants it repaired.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SortieRefusal {
    NotAtRelay,
    /// This site is not on the current board. Kept apart from `NotAtRelay`
    /// because one is a walk and the other is a wait.
    NotOffered,
    NoSquad,
    /// One program named twice. Refused rather than silently deduped: the
    /// squad size is what the provisioning is priced off, so a quiet dedupe
    /// would charge for a body that never went.
    Duplicate,
    NotStaff(String),
    Downed(String),
    Wounded(String),
    /// The base would be left with nobody in it.
    WouldEmptyTheBase,
    Unprovisioned {
        item: ItemId,
        need: u32,
        held: u32,
    },
}

impl Game {
    /// Where the player is standing, as far as the Relay is concerned —
    /// shared by a sortie's board and a caravan route's dispatch, since both
    /// gate on the same structure. Renamed off `SortieReach` for exactly
    /// that reason: routes reuse this call rather than a second reach check,
    /// and a name that still said "sortie" would lie about what it now
    /// gates.
    ///
    /// **It measures the base, never the distance to the Relay** — this is
    /// `Game::broker_reach` one verb along, and for its argument:
    /// `place_structure` refuses everything but a Home until a Home is
    /// standing and every structure has to stand on laid floor, so a Relay
    /// is in the base by construction. "Is the player in the base" is
    /// therefore the whole question, which since the base moved out of
    /// phase reads as: the party is in base space, standing on
    /// `BaseCell::Floor`.
    ///
    /// Floor and not merely `walkable`, `broker_reach`'s rule: the mast is
    /// reachable from the base's laid ground, not from a corridor mined out
    /// past its edge.
    pub fn dispatch_reach(&mut self) -> DispatchReach {
        if !self.has_relay() {
            return DispatchReach::NoRelay;
        }
        let Some((x, y)) = self.base_pos() else {
            return DispatchReach::OffBase;
        };
        if self
            .world
            .resource::<crate::base_grid::BaseGrid>()
            .is_floor(x, y)
        {
            DispatchReach::AtRelay
        } else {
            DispatchReach::OffBase
        }
    }

    /// Whether the run has a Relay standing at all, wherever it is.
    pub(crate) fn has_relay(&mut self) -> bool {
        let mut query = self.world.query_filtered::<Entity, With<Structure>>();
        let standing: Vec<Entity> = query.iter(&self.world).collect();
        standing
            .into_iter()
            .any(|entity| self.dispatches_sorties(entity))
    }

    /// Whether `entity` is a structure a squad can be dispatched from —
    /// read off the def's flag and never off the shipped id, so a mod's
    /// second dispatch structure works without an engine change.
    fn dispatches_sorties(&self, entity: Entity) -> bool {
        let Some(kind) = self.world.get::<Structure>(entity).map(|s| &s.kind) else {
            return false;
        };
        self.world
            .resource::<crate::structures::StructureDb>()
            .get(kind)
            .is_some_and(|def| def.dispatches_sorties)
    }

    /// The offers standing at the Relay, or `None` with no Relay built.
    ///
    /// **Derived, never stored** — the Broker board's rule and for its
    /// reasons: recomputed on every read from the world seed, `ZoneLevel`
    /// and the clock epoch, so there is no save field, no roll to scum, and
    /// it rotates on its own as the epoch advances.
    ///
    /// Draws **no** `GameRng` at all. A draw here would not survive a reload
    /// and would shift every later roll in the run — `stack::generate`'s
    /// rule. Selection and each site's battle count both fold their own seed
    /// and reduce it through `derive::index`, never `%`: for a small pool `%`
    /// reads nothing but the seed's lowest bit and silently anti-correlates
    /// two draws taken off one fold.
    ///
    /// An empty catalogue gives an empty `Vec` and **not** `None`, which
    /// means "no Relay" — the two leave the player different errands, which
    /// is `DispatchReach`'s own argument one level down.
    pub fn sortie_board(&mut self) -> Option<Vec<crate::views::SortieRow>> {
        if self.dispatch_reach() == DispatchReach::NoRelay {
            return None;
        }
        let seed = self.sortie_board_seed();
        let mut pool: Vec<crate::sorties::SortieDef> = self
            .world
            .resource::<crate::sorties::SortieDb>()
            .iter()
            .cloned()
            .collect();
        let mut rows = Vec::new();
        // Drawn without replacement, so one epoch's board never offers the
        // same site twice. `swap_remove` is what makes the walk O(slots)
        // and is safe for reproducibility because the pool it starts from
        // is id-sorted and every index is derived, not rolled.
        for slot in 0..crate::tuning::SORTIE_BOARD_SLOTS {
            if pool.is_empty() {
                break;
            }
            let pick = crate::derive::index(salt(seed, b"slot", slot as u64), pool.len());
            let def = pool.swap_remove(pick);
            let span = (def.battles_max - def.battles_min + 1) as usize;
            let battles = def.battles_min
                + crate::derive::index(salt(seed, def.id.as_str().as_bytes(), slot as u64), span)
                    as u32;
            rows.push(crate::views::SortieRow {
                id: def.id.clone(),
                name: def.name.clone(),
                description: def.description.clone(),
                risk: def.risk,
                battles,
                ticks: Self::sortie_duration(def.risk, battles),
            });
        }
        Some(rows)
    }

    /// The board's seed: the world seed, the sector and the epoch, folded
    /// FNV-1a a byte at a time.
    ///
    /// Byte-at-a-time rather than one XOR-and-multiply per word, for
    /// `FrameSpec::salted`'s measured reason and `Game::board_seed`'s: a
    /// whole-word XOR leaves low output bits a fixed function of the input,
    /// and consecutive epochs differ in exactly one low bit.
    fn sortie_board_seed(&self) -> u64 {
        let epoch = self.current_tick() / crate::tuning::SORTIE_BOARD_ROTATION_TICKS;
        let mut h = 0xcbf2_9ce4_8422_2325_u64;
        for word in [
            self.world.resource::<crate::world::WorldMap>().seed() as u64,
            self.world.resource::<crate::resources::ZoneLevel>().0 as u64,
            epoch,
            crate::tuning::SORTIE_SALT,
        ] {
            h = crate::game::contracts::fold(h, &word.to_le_bytes());
        }
        h
    }

    /// What the provisioning for a squad of `squad` bodies running
    /// `battles` fights costs the base.
    ///
    /// Priced per battle *and* per body, because both are what provisions
    /// have to cover. Denominated in the **build currency**, which is
    /// role-derived rather than named in Rust and is what the base's shelves
    /// actually hold — the figure the stock strip is already showing.
    pub fn sortie_provision_cost(&self, battles: u32, squad: usize) -> Vec<(ItemId, u32)> {
        let units = crate::tuning::SORTIE_PROVISION_PER_BATTLE * battles * squad as u32;
        vec![(self.currency(), units)]
    }

    /// Sends `members` to the site `id` names on the current board.
    ///
    /// Every refusal lands **before anything is spent**,
    /// `commit_caravan_basket`'s rule. Only once every one of them has
    /// passed does the provisioning leave the shelves, through
    /// `stock::spend_from_base` — a teleport off the shelf is right here:
    /// this is a base cost paid at the Relay, not a build a body walks to.
    ///
    /// The record stores the **whole resolved site**, never the id or a
    /// board index. A board that rotates while the squad is out, or an
    /// `assets/sorties/` file edited between sessions, must not be able to
    /// rewrite or strand a trip already in flight — `ActiveContract` stores
    /// a whole `ContractDef` for exactly that reason.
    pub fn dispatch_sortie(
        &mut self,
        id: &crate::sorties::SortieId,
        members: &[Entity],
    ) -> Result<(), SortieRefusal> {
        if self.dispatch_reach() != DispatchReach::AtRelay {
            return Err(SortieRefusal::NotAtRelay);
        }
        let Some(row) = self
            .sortie_board()
            .unwrap_or_default()
            .into_iter()
            .find(|r| &r.id == id)
        else {
            return Err(SortieRefusal::NotOffered);
        };
        if members.is_empty() {
            return Err(SortieRefusal::NoSquad);
        }
        let mut seen: Vec<Entity> = members.to_vec();
        seen.sort();
        seen.dedup();
        if seen.len() != members.len() {
            return Err(SortieRefusal::Duplicate);
        }
        for &member in members {
            if self.program_role(member) != Some(crate::game::party::ProgramRole::Staff) {
                return Err(SortieRefusal::NotStaff(self.creature_label(member)));
            }
            if self
                .world
                .get::<crate::components::Downed>(member)
                .is_some()
            {
                return Err(SortieRefusal::Downed(self.creature_label(member)));
            }
            let Some(stats) = self.world.get::<Stats>(member) else {
                return Err(SortieRefusal::NotStaff(self.creature_label(member)));
            };
            if (stats.hp as f32) < stats.max_hp as f32 * crate::tuning::SORTIE_MIN_HP_FRACTION {
                return Err(SortieRefusal::Wounded(self.creature_label(member)));
            }
        }
        // The base is never emptied. Production stops dead and a sweep lands
        // on an empty base — the same category of guard as `max_deployed`.
        if self.base_staff().len() <= members.len() {
            return Err(SortieRefusal::WouldEmptyTheBase);
        }
        let cost = self.sortie_provision_cost(row.battles, members.len());
        for (item, qty) in &cost {
            if crate::game::base::work_orders::base_holding(self, item) < *qty {
                return Err(SortieRefusal::Unprovisioned {
                    item: item.clone(),
                    need: *qty,
                    held: crate::game::base::work_orders::base_holding(self, item),
                });
            }
        }

        for (item, qty) in &cost {
            crate::game::base::stock::spend_from_base(
                self,
                item,
                *qty,
                crate::base_ledger::ConsumeSource::Base,
            );
        }
        let site = self
            .world
            .resource::<crate::sorties::SortieDb>()
            .get(id)
            .cloned()
            .expect("a board row names a site the catalogue holds");
        let names: Vec<String> = members.iter().map(|&e| self.creature_label(e)).collect();
        self.queue_squad_walk(members, true);
        self.world
            .resource_mut::<crate::resources::Sorties>()
            .0
            .push(crate::resources::Sortie {
                risk: site.risk,
                site,
                members: members.to_vec(),
                ticks_total: row.ticks,
                ticks_elapsed: 0,
                battles_total: row.battles,
                battles_done: 0,
                aborted: false,
                loot: Vec::new(),
                programs: Vec::new(),
                xp: 0,
                kills: 0,
                casualties: Vec::new(),
            });
        self.log_base(format!(
            "{} {} out for {}.",
            names.join(", "),
            if names.len() == 1 { "ships" } else { "ship" },
            row.name
        ));
        Ok(())
    }

    /// Queues the walk a squad is *seen* to make: out through base space's
    /// one door, or in through it.
    ///
    /// **Direction needs no field** — a departure and an arrival are the same
    /// cue with its ends swapped, `hauling::Errand`'s rule. The glyph is read
    /// off the body here rather than looked up by the frontend because a
    /// departing program is `ProgramRole::Sortie` the moment the record is
    /// pushed, and every view drops it from that instant.
    ///
    /// Three bodies queue nothing, all by omission rather than by a check: one
    /// whose entity is gone (a Permadeath casualty, despawned before the
    /// report is drawn), one standing somewhere that is not walkable base
    /// space (an adopted program that has not drifted yet), and one already
    /// standing on the door, which has no walk to draw.
    fn queue_squad_walk(&mut self, members: &[Entity], outbound: bool) {
        for &member in members {
            let Some((ch, color)) = self.world.get::<Glyph>(member).map(|g| (g.ch, g.color)) else {
                continue;
            };
            let Some(tile) = self.world.get::<Position>(member).map(|p| (p.x, p.y)) else {
                continue;
            };
            let door = crate::game::base_space::BASE_EXIT_CELL;
            let (from, to) = if outbound { (tile, door) } else { (door, tile) };
            self.queue_transit_walk(ch, color, from, to);
        }
    }

    /// One tick of every trip currently in flight.
    ///
    /// A **`Game` method, not a bevy system** — `run_dig_crew` and
    /// `run_repair_bays`' reason: it names programs through
    /// `creature_label`, it logs, and it damages through `apply_damage`, and
    /// a bevy system would have to be a second copy of all three.
    pub(crate) fn run_sorties(&mut self) {
        // `run_dig_crew`'s guard, and `nest_aggro_tick`'s obligation:
        // anything that can change the world from inside a tick inherits the
        // battle check. A squad's fight resolving mid-battle would spend the
        // player's own `GameRng` draws underneath the round they are looking
        // at.
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        if self
            .world
            .resource::<crate::resources::Sorties>()
            .0
            .is_empty()
        {
            return;
        }
        let mut index = 0;
        while index < self.world.resource::<crate::resources::Sorties>().0.len() {
            // A trip that came home removed its own record, so the record
            // behind it has slid into this index and must not be skipped.
            // **Read off the step rather than off the length**: comparing
            // `index` to the new length only catches a removal at the tail,
            // so a first trip returning while a second was still out cost
            // the second a tick, every time, in silence.
            if !self.step_sortie(index) {
                index += 1;
            }
        }
    }

    /// Advances one trip by a tick, fighting a battle if one is due.
    ///
    /// Returns whether the trip came home — i.e. whether the record at
    /// `index` is gone and something else has slid into it.
    fn step_sortie(&mut self, index: usize) -> bool {
        let (elapsed, total, due) = {
            let sortie = &mut self.world.resource_mut::<crate::resources::Sorties>().0[index];
            sortie.ticks_elapsed += 1;
            (
                sortie.ticks_elapsed,
                sortie.ticks_total,
                battles_due(
                    sortie.ticks_elapsed,
                    sortie.ticks_total,
                    sortie.battles_total,
                ),
            )
        };
        let aborted = self.world.resource::<crate::resources::Sorties>().0[index].aborted;
        while !aborted
            && self.world.resource::<crate::resources::Sorties>().0[index].battles_done < due
        {
            self.resolve_sortie_battle(index);
            if self.world.resource::<crate::resources::Sorties>().0[index].aborted {
                break;
            }
        }
        if elapsed >= total {
            self.return_sortie(index);
            return true;
        }
        false
    }

    /// One battle, **entirely inside this call**: spawn, fight, despawn.
    ///
    /// This is the load-bearing decision of the whole feature. No bevy
    /// system runs mid-method, so the opposition is never observed by the
    /// map, the examine ray, `cull_to_cap`, `ensure_local_population` or
    /// anything else — which means the feature does not have to teach four
    /// systems about a new space, and cannot reintroduce the "which space is
    /// this?" bug class. **A hostile that outlives its battle is a defect**,
    /// not a tuning question.
    fn resolve_sortie_battle(&mut self, index: usize) {
        let (risk, members) = {
            let sortie = &self.world.resource::<crate::resources::Sorties>().0[index];
            (sortie.risk, sortie.members.clone())
        };
        // The pool is drawn against the party's own anchor tile, which is a
        // real walkable surface tile of this sector — base-space coordinates
        // are a different space and `habitat_pools` would answer about
        // whatever surface ground happened to share the numbers.
        let (ax, ay) = self.anchor_position().unwrap_or((0, 0));
        let species = {
            let Some((candidates, _)) = self.habitat_pools(ax, ay, None, risk) else {
                return;
            };
            if candidates.is_empty() {
                return;
            }
            let mut rng = self.world.resource_mut::<crate::resources::GameRng>();
            let pick = rand::RngExt::random_range(&mut rng.0, 0..candidates.len());
            candidates[pick].clone()
        };
        let hostiles = self.spawn_pack(
            &species,
            false,
            SORTIE_SENTINEL.0,
            SORTIE_SENTINEL.1,
            crate::game::spawning::SpawnEscalation::surface(),
        );

        let mut casualty = None;
        let mut kills = 0;
        for _ in 0..crate::tuning::SORTIE_MAX_BATTLE_ROUNDS {
            // Sorted by entity for `assembler_system`'s reason: bevy's
            // iteration order is not stable and two squads would resolve
            // differently between runs.
            let mut actors: Vec<Entity> = members
                .iter()
                .chain(hostiles.iter())
                .copied()
                .filter(|&e| self.creature_alive(e))
                .collect();
            actors.sort();
            for actor in actors {
                if !self.creature_alive(actor) {
                    continue;
                }
                let ours = members.contains(&actor);
                let targets: Vec<Entity> = if ours { &hostiles } else { &members }
                    .iter()
                    .copied()
                    .filter(|&e| self.creature_alive(e))
                    .collect();
                let Some(&front) = targets.first() else {
                    break;
                };
                self.swing_for_the_squad(actor, front, &targets);
                self.tick_ability_cooldowns(actor);
            }
            kills = hostiles
                .iter()
                .filter(|&&e| !self.creature_alive(e))
                .count();
            casualty = members.iter().copied().find(|&e| !self.creature_alive(e));
            if casualty.is_some() || kills == hostiles.len() {
                break;
            }
        }

        // Priced while the fallen are still standing there: `kill_xp` and
        // `downed_program_for` both read the victim's own components, so
        // neither can move below the despawn.
        let mut earned = 0;
        let mut banked: Vec<crate::items::DownedProgram> = Vec::new();
        for &hostile in &hostiles {
            if self.creature_alive(hostile) {
                continue;
            }
            let paid = (self.kill_xp(hostile) as f32 * crate::tuning::SORTIE_XP_MULTIPLIER) as u32;
            earned += paid;
            for &member in &members {
                if self.creature_alive(member) {
                    self.award_companion_xp(member, paid);
                }
            }
            // Banked onto the trip, not pushed into the player's store: the
            // squad is carrying these home, and `return_sortie` is where
            // they arrive. `downed_program_for` is the same roll the field
            // kill uses, called rather than copied — `Perk::Teardown`'s old
            // trap.
            if let Some(program) = self.downed_program_for(hostile) {
                banked.push(program);
            }
        }

        // **Unconditional, and every hostile, living or not.** Whatever the
        // outcome, nothing of the opposition may outlive this call — that is
        // the whole of what keeps this feature out of the "which space is
        // this?" bug class.
        for &hostile in &hostiles {
            self.world.entity_mut(hostile).despawn();
        }

        let downed = casualty.map(|e| (e, self.bench_or_dissolve(e)));
        for &member in &members {
            if !self.creature_alive(member) {
                continue;
            }
            let heal = {
                let stats = self.world.get::<Stats>(member).expect("a living member");
                (stats.max_hp as f32 * crate::tuning::SORTIE_PROVISION_HEAL_FRACTION) as i32
            };
            self.restore_hp(member, heal);
        }

        let sortie = &mut self.world.resource_mut::<crate::resources::Sorties>().0[index];
        sortie.battles_done += 1;
        sortie.kills += kills as u32;
        sortie.programs.append(&mut banked);
        sortie.xp += earned;
        if let Some((entity, name)) = downed {
            sortie.aborted = true;
            sortie.casualties.push(name);
            // Dropped from the squad rather than left in it. Under
            // Permadeath `bench_or_dissolve` has despawned the entity, and a
            // record naming a dead id would panic the return line that goes
            // to label it; under Forgiving the program is `Downed` and walks
            // itself to a Repair Bay once the record is gone, which it
            // cannot do while it is still away.
            sortie.members.retain(|&e| e != entity);
        }
    }

    /// Every trip currently away, worded for a screen.
    ///
    /// `&self` and **derives nothing back into the world** — a screen that
    /// rewrote what it draws would make the trip depend on whether anyone
    /// looked, which is `Game::memory_report`'s rule.
    pub fn sortie_reports(&self) -> Vec<crate::views::SortieReport> {
        self.world
            .resource::<crate::resources::Sorties>()
            .0
            .iter()
            .map(|s| crate::views::SortieReport {
                site: s.site.name.clone(),
                members: s.members.iter().map(|&e| self.creature_label(e)).collect(),
                casualties: s.casualties.clone(),
                kills: s.kills,
                xp: s.xp,
                battles_done: s.battles_done,
                battles_total: s.battles_total,
                ticks_left: s.ticks_total.saturating_sub(s.ticks_elapsed),
                aborted: s.aborted,
            })
            .collect()
    }

    /// A trip reaching its last tick: the record is dropped and one line
    /// says what came back.
    ///
    /// Members become `Staff` again **by omission** — nothing writes a role
    /// anywhere, which is the whole of why the fourth variant was worth
    /// having. A Forgiving casualty comes home still carrying `Downed` and
    /// walks itself to a Repair Bay through the existing `Downed` arm of
    /// `drift_idle_staff`; there is no new recovery path.
    ///
    /// No loot delivery here: extraction retired the direct kill drop that
    /// used to fill `Sortie::loot`, so the field is always empty and a
    /// delivery loop over it would never run. What the squad carries is
    /// `Sortie::programs`, and this is the door it arrives through —
    /// nothing writes the player's store mid-trip.
    fn return_sortie(&mut self, index: usize) {
        let sortie = self
            .world
            .resource_mut::<crate::resources::Sorties>()
            .0
            .remove(index);
        self.queue_squad_walk(&sortie.members, false);
        let names: Vec<String> = sortie
            .members
            .iter()
            .map(|&e| self.creature_label(e))
            .collect();
        let who = if names.is_empty() {
            "Nobody".to_string()
        } else {
            names.join(", ")
        };
        self.log_base(format!(
            "{who} came back from {} — {} down, {} XP.",
            sortie.site.name, sortie.kills, sortie.xp
        ));
        for lost in &sortie.casualties {
            self.log_base(format!("{lost} did not come back."));
        }
        let carried = sortie.programs.len();
        let mut delivered = 0;
        for program in sortie.programs {
            // Stops at the first refusal rather than trying each: once the
            // store is full it stays full, so continuing would log the same
            // line once per remaining program. `push_downed_program` says
            // it once and spec decision 9 keeps everything already held.
            if !self.push_downed_program(program) {
                break;
            }
            delivered += 1;
        }
        if carried > 0 {
            let plural = if carried == 1 { "" } else { "s" };
            self.log_base(format!(
                "They brought back {delivered} of {carried} downed program{plural}."
            ));
        }
    }

    /// One combatant's swing: the highest-priority Special it can afford
    /// that is off cooldown, else a basic attack.
    ///
    /// Both sides run the same stated rule, and both resolve through the
    /// real doors — `use_ability` and `resolve_and_apply_attack` are
    /// `BattleState`-free, so hit chance, the crit/hit/fumble/miss ladder,
    /// damage bands, mitigation, affinity scaling and Power costs are all
    /// the ones a fight in front of the player uses.
    ///
    /// The trained enemy policy is deliberately **not** used: its selection
    /// path reads `BattleState`, and it exists to make fights against *the
    /// player* interesting — off-screen it would be modelling an audience
    /// that is not present.
    fn swing_for_the_squad(&mut self, actor: Entity, front: Entity, targets: &[Entity]) {
        let choice = self
            .actor_abilities(actor)
            .into_iter()
            .filter(|d| !d.effect.field_only() && !d.is_passive())
            .filter(|d| !matches!(d.effect, crate::abilities::AbilityEffect::Decompile))
            .find(|d| self.ability_unavailable(actor, d).is_none());
        match choice {
            Some(ability) => {
                let recipients = match ability.target {
                    crate::abilities::AbilityTarget::AllEnemies
                    | crate::abilities::AbilityTarget::WholeEnemyGroup => targets.to_vec(),
                    _ => vec![front],
                };
                // The charge sits here and not in `use_ability`, which the
                // wielded proc and hostile invocations share and which stays
                // free — the `BattleAction::Special` site's rule.
                self.spend_power(actor, crate::abilities::routine_power_cost(&ability));
                self.arm_cooldown(actor, &ability);
                let name = self.creature_label(actor);
                self.use_ability(&ability, actor, &name, &recipients);
            }
            None => {
                let swing = crate::battle::Swing::plain(self.natural_range_of(actor));
                self.resolve_and_apply_attack(actor, front, swing);
            }
        }
    }

    /// How long a trip to a site of this risk offset, running this many
    /// battles, takes.
    ///
    /// **The one place the figure is computed.** The board quotes it and the
    /// countdown runs it, `views::BuildOrderRow`'s rule that every figure on
    /// a screen is a call rather than a copy — a screen quoting one number
    /// while the countdown runs another is precisely the failure that rule
    /// exists for.
    ///
    /// It reads the site's **risk offset** and never the absolute danger
    /// band, or every trip late in a run would take enormously longer for no
    /// reason the player could name. And there is no term for squad size,
    /// level or power: a stronger squad shows up as better outcomes and
    /// never as a faster cycle.
    pub fn sortie_duration(risk: u32, battles: u32) -> u64 {
        crate::tuning::SORTIE_TRAVEL_BASE_TICKS
            + crate::tuning::SORTIE_TRAVEL_PER_RISK_TICKS * risk as u64
            + crate::tuning::SORTIE_TICKS_PER_BATTLE * battles as u64
    }
}

/// One draw's own seed, folded off the board's.
///
/// A separate fold per draw rather than one stream, `FrameSpec::salted`'s
/// rule: a site added to or removed from the catalogue must not reshuffle
/// which battle count the sites around it were offered at. Folded a byte at
/// a time and ending on the counter, because `derive::index` reads bit 63
/// and a value folded in as one whole word never reaches it.
fn salt(seed: u64, tag: &[u8], n: u64) -> u64 {
    let h = crate::game::contracts::fold(seed, tag);
    crate::game::contracts::fold(h, &n.to_le_bytes())
}

/// Where a sortie's opposition is placed for the instant it exists.
///
/// A fixed coordinate far outside any base or zone traffic. It needs no
/// walkability check and no uniqueness: nothing observes these entities —
/// they are spawned, fought and despawned inside one call — and
/// `spawn_pack`'s scatter is harmless here.
const SORTIE_SENTINEL: (i32, i32) = (1 << 22, 1 << 22);

/// How many of a trip's battles should have been fought by `elapsed`.
///
/// Fights fall at even intervals across the **middle** of the trip, with the
/// travel split half out and half back — so a squad is not fighting the
/// instant it leaves and is not fighting on the doorstep coming home.
fn battles_due(elapsed: u64, total: u64, battles: u32) -> u32 {
    if battles == 0 || total == 0 {
        return 0;
    }
    let fighting = crate::tuning::SORTIE_TICKS_PER_BATTLE * battles as u64;
    let travel = total.saturating_sub(fighting);
    let start = travel / 2;
    if elapsed <= start {
        return 0;
    }
    let into = elapsed - start;
    ((into / crate::tuning::SORTIE_TICKS_PER_BATTLE) as u32).min(battles)
}
