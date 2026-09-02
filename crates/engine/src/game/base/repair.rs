//! What brings a downed program back: where the base's Repair Bays stand,
//! and the tick that writes Integrity into a program lying on one.
//!
//! A program benched by a Forgiving death (`components::Downed`) is out of
//! the labour pool and stays that way until it is whole again. Nothing else
//! in the game heals it and there is no timer quietly doing the job — the
//! way back is a structure the player chose to build.

use crate::base_grid::BaseGrid;
use crate::components::{Downed, Position, Stats, Structure};
use crate::game::base::hauling::{NoPost, step_to_post};
use crate::game::base::offshift::in_reach;
use crate::resources::Locale;
use crate::structures::{StructureDb, StructureId};
use crate::tuning::BAY_ADMISSION_HP_FRACTION;
use bevy_ecs::prelude::Entity;
use std::collections::HashSet;

use crate::Game;

/// Where the base's Repair Bays stand.
///
/// `offshift::Amenities` with the index taken out: a Bay is not keyed by
/// anything a program has to match, so a sorted `Vec` replaces the
/// `BTreeMap`. Built **once per caller pass**, never once per program, for
/// that type's stated reason — two cheap builds beat one stale cached copy,
/// and a cached one would be a new `Resource` and another
/// query-iteration-order shift.
pub(crate) struct Bays {
    /// `(tile, rate, radius)`, sorted by tile so a tie between two Bays
    /// equidistant from one program resolves the same way every run.
    sites: Vec<(Position, i32, i32)>,
}

impl Bays {
    /// Takes an iterator so a `&Game` and, should one ever want it, a bevy
    /// system can both build one — `Amenities::build`'s signature.
    pub(crate) fn build<'a>(
        structures: impl Iterator<Item = (&'a StructureId, &'a Position)>,
        db: &StructureDb,
    ) -> Self {
        let mut sites: Vec<(Position, i32, i32)> = structures
            .filter_map(|(kind, pos)| {
                let recovery = db.get(kind)?.recovery.as_ref()?;
                Some((*pos, recovery.rate(), recovery.radius))
            })
            .collect();
        // **A total order, not bevy's iteration order.** `min_by_key`
        // returns the first of several equal minima, so two Bays the same
        // distance from one program must already be in a settled order
        // before anything asks.
        sites.sort_by_key(|(p, _, _)| (p.x, p.y));
        Self { sites }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.sites.is_empty()
    }

    /// The Bay a program at `from` would walk to, with its rate and reach.
    ///
    /// Ties break on a **total** `(chebyshev distance, x, y)` order, for
    /// `Amenities::nearest`'s reason.
    pub(crate) fn nearest(&self, from: Position) -> Option<(Position, i32, i32)> {
        self.sites
            .iter()
            .min_by_key(|(p, _, _)| ((p.x - from.x).abs().max((p.y - from.y).abs()), p.x, p.y))
            .copied()
    }

    /// The Bay currently serving a program standing at `here`, with the rate
    /// it heals at — the nearest one, when that one is in reach.
    ///
    /// **The one expression of "this program is in a Bay."** `nearest` picks
    /// a candidate and `in_reach` accepts or rejects it, and the pair is
    /// asked twice: once by `run_repair_bays` to decide who to heal, once by
    /// `Game::recovering_programs` to decide who the map marks as mending.
    /// Written out at both sites the mark would be free to drift off the
    /// heal — a body lighting up while lying a tile too far away, or going
    /// dark while it is being healed — with nothing failing to compile and
    /// the fault reading as a rendering bug.
    pub(crate) fn serving(&self, here: Position) -> Option<(Position, i32)> {
        let (site, rate, radius) = self.nearest(here)?;
        in_reach(here, site, radius).then_some((site, rate))
    }
}

impl Game {
    /// One tick of every Repair Bay: Integrity into whatever downed programs
    /// are standing in reach, and off the bench at full.
    ///
    /// **A `Game` method rather than a bevy system, `run_dig_crew`'s
    /// reason.** The recovery line has to name the program, and
    /// `Game::creature_label` is the one door from an entity to its name —
    /// rare tier, custom name and zone tag included. Rebuilt from a query's
    /// components instead it would be a second copy of that formula, which
    /// is the shape this repo has been bitten by four times. `restore_hp` is
    /// the other door: the one place HP goes up, the mirror of
    /// `apply_damage`.
    ///
    /// The scan centres on **each downed program's own `Position`**, which
    /// is the whole of what differs from `systems::power_regen_system` —
    /// that one centres on the party's base coordinates because it serves
    /// the player, and a program is wherever it is standing.
    pub(crate) fn run_repair_bays(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let bays = self.repair_bays();
        if bays.is_empty() {
            return;
        }
        // Sorted by tile for `run_build_crew`'s reason: bevy's iteration
        // order is not stable, and two programs recovering in a different
        // order between runs would put their log lines in a different order.
        let mut downed: Vec<(i32, i32, Entity)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), bevy_ecs::prelude::With<Downed>>();
            query
                .iter(&self.world)
                .map(|(e, p)| (p.x, p.y, e))
                .collect()
        };
        downed.sort();
        for (x, y, program) in downed {
            let here = Position { x, y };
            let Some((_, rate)) = bays.serving(here) else {
                continue;
            };
            // `restore_hp` caps at `max_hp` itself, so a Bay cannot overheal
            // and a rate of zero — a mod's negative one, floored — simply
            // lands nothing.
            self.restore_hp(program, rate);
            let Some(stats) = self.world.get::<Stats>(program).copied() else {
                continue;
            };
            if stats.hp < stats.max_hp {
                continue;
            }
            // **On the transition and nowhere else**, `set_machine_status`'
            // rule: entering a state is news, staying in it is not. The
            // marker coming off is what makes this an edge — a program
            // already at full carries no `Downed` and is not in this query.
            self.world.entity_mut(program).remove::<Downed>();
            let name = self.creature_label(program);
            self.log_base(format!("{name} is back on its feet."));
        }
    }

    /// The base's Bays, gathered from the structure query.
    ///
    /// One pass per caller, `Game::amenities`' shape — `run_repair_bays` and
    /// `drift_idle_staff` each build their own rather than sharing a cached
    /// copy.
    pub(crate) fn repair_bays(&mut self) -> Bays {
        let mut query = self.world.query::<(&Structure, &Position)>();
        let sites: Vec<(StructureId, Position)> = query
            .iter(&self.world)
            .map(|(s, p)| (s.kind.clone(), *p))
            .collect();
        Bays::build(
            sites.iter().map(|(kind, pos)| (kind, pos)),
            self.world.resource::<StructureDb>(),
        )
    }

    /// Marks every badly hurt staff program `Downed`, so the base's existing
    /// recovery machinery collects it.
    ///
    /// **A second door onto `components::Downed`, and the widening is the
    /// feature.** `bench_or_dissolve` was the only one, which meant a Bay
    /// served programs that had been *killed* and benched — and nothing
    /// else. A staff program that merely came out of a raid at a sliver of
    /// Integrity had no recovery route at all once resting stopped mending
    /// the base's own pool, and under Permadeath, where the bench does not
    /// exist and `Downed` is never inserted, a Bay served literally nobody.
    ///
    /// **Insertion is the whole of what this does**, and that is what keeps
    /// the rest of the feature free. The `on_shift` filter already drops a
    /// `Downed` body, `schedule_base_labour`'s diff already frees its post
    /// unconditionally, `drift_idle_staff` already walks it to a Bay, and
    /// `run_repair_bays` already heals it and lifts the marker at full — so
    /// the four sites that make a Bay work needed no edit, and the map's `+`
    /// followed for the same reason. A parallel "hurt" marker beside
    /// `Downed` would have been an edit at every one of them and a fifth
    /// state for each to disagree about.
    ///
    /// **`Staff` alone, and the role filter is the `staff` argument rather
    /// than a test in here.** The one caller passes `Game::base_staff`, which
    /// *is* the `Game::program_role` derivation — so a party member, a
    /// wielded program and a squad away on a sortie are all absent by
    /// construction, and re-asking inside the loop was a second copy of that
    /// question which no real call could ever answer differently.
    /// `update_disgruntled` takes the same list on the same terms and makes
    /// the same omission. What the roles mean here: a party member and a
    /// wielded program are the player's to mend by resting, and a `Sortie`
    /// program is not in the base to be walked anywhere — it is admitted, if
    /// it is still hurt, on the first beat after it comes home as `Staff`.
    ///
    /// **Nothing is admitted while no Bay stands.** `Downed` is a one-way
    /// door without one — `a_downed_program_with_no_bay_standing_stays_down`
    /// is the shipped rule — which is the right price for a program that
    /// died and quite the wrong one for a program that is merely hurt:
    /// benching it would delete a worker from the base for the rest of the
    /// run and never say so. With no Bay, a hurt program keeps working.
    ///
    /// Run beside `update_disgruntled`, before the posting half of
    /// `schedule_base_labour` reads `on_shift` — that function's rule: a body
    /// that leaves the line this tick must not also be handed a job this
    /// tick.
    pub(crate) fn admit_the_badly_hurt(&mut self, staff: &[Entity], bays: &Bays) {
        if bays.is_empty() {
            return;
        }
        for &worker in staff {
            if self.world.get::<Downed>(worker).is_some() {
                continue;
            }
            // The entry test is asked only of a body still working, which is
            // `update_disgruntled`'s asymmetry and the reason there is no
            // release line here: nothing in this loop can take the marker
            // off, so the boundary has no back edge to flicker across.
            let Some(stats) = self.world.get::<Stats>(worker) else {
                continue;
            };
            if stats.max_hp <= 0 || stats.hp <= 0 {
                continue;
            }
            if stats.hp_fraction() >= BAY_ADMISSION_HP_FRACTION {
                continue;
            }
            self.world.entity_mut(worker).insert(Downed);
            let name = self.creature_label(worker);
            self.log_base(format!("{name} breaks off for repairs."));
        }
    }

    /// Every downed program a Bay is mending this instant.
    ///
    /// What `EntityView::recovering` is built from, and it is derived on
    /// every look rather than stored: a program reaching full Integrity, one
    /// taking its last step into reach, and a Bay being demolished under one
    /// all change the answer with nothing to notice they did —
    /// `build_views`' own reason for rebuilding `attended` per call.
    ///
    /// **Keyed by `Entity` and not by tile, which is what makes it the
    /// program's own answer.** A Bay's `Position` identifies it as well as
    /// its entity would — no two structures share a tile — but a body does
    /// not have that: with `RecoveryDef::radius` past zero several downed
    /// programs mend from one Bay, and at zero they stand on the Bay's own
    /// cell, so a tile is neither unique to a patient nor distinct from the
    /// building. The mark belongs on the body being mended, so the set has
    /// to name bodies.
    ///
    /// **The same pass `run_repair_bays` makes**, through `Bays::serving`: a
    /// program wears the mark exactly while its Integrity is climbing, so a
    /// player watching the map and a program's Integrity climbing are reading
    /// one fact.
    pub(crate) fn recovering_programs(&mut self) -> HashSet<Entity> {
        let bays = self.repair_bays();
        if bays.is_empty() {
            return HashSet::new();
        }
        let downed: Vec<(Entity, Position)> = {
            let mut query = self
                .world
                .query_filtered::<(Entity, &Position), bevy_ecs::prelude::With<Downed>>();
            query.iter(&self.world).map(|(e, p)| (e, *p)).collect()
        };
        downed
            .into_iter()
            .filter(|(_, here)| bays.serving(*here).is_some())
            .map(|(e, _)| e)
            .collect()
    }

    /// One step toward the nearest Repair Bay, or `NoPost` when there is
    /// nowhere to go.
    ///
    /// `Game::step_off_shift`'s shape exactly, and it rides the same walk
    /// (`hauling::step_to_post`) — there is no second one. **What differs is
    /// the price of a failure.** An off-shift body latches its need and
    /// drops `OffShift`, because the gate would otherwise re-insert it and
    /// run insert → failed step → remove → insert every beat forever.
    /// Nothing re-inserts `Downed`, so there is no flicker to stop — and
    /// dropping it here would silently heal a program that could not reach a
    /// Bay, which is the one thing this feature must never do. The caller
    /// therefore holds on `Err` and leaves the marker alone.
    ///
    /// A base with no Bay standing answers `NoRoute` at the first line that
    /// asks, which is why a benched program lies where it fell rather than
    /// wandering: it is on an errand it cannot start, not idle.
    pub(crate) fn step_to_repair(&mut self, worker: Entity, bays: &Bays) -> Result<(), NoPost> {
        let here = self
            .world
            .get::<Position>(worker)
            .copied()
            .ok_or(NoPost::NoRoute)?;
        let (site, _, radius) = bays.nearest(here).ok_or(NoPost::NoRoute)?;
        if in_reach(here, site, radius) {
            // Arrived. It stands here until it is whole — the drift's other
            // shape would walk it straight back off again.
            return Ok(());
        }
        let blocked = self.structure_tiles();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        let Some(tile) = step_to_post(
            self.world.resource::<BaseGrid>(),
            here,
            site,
            &blocked,
            pocket_radius,
        )?
        else {
            // The field admits nowhere better than where it stands. It waits,
            // exactly as a hauler does.
            return Ok(());
        };
        // The party is the one rejection `step_to_post` cannot make for
        // itself: `Locale` is where the party stands in base space, and the
        // player's `Position` is pinned to the anchor out on the surface.
        if let Locale::Base { x, y } = *self.world.resource::<Locale>()
            && (x, y) == (tile.x, tile.y)
        {
            return Ok(());
        }
        if let Some(mut pos) = self.world.get_mut::<Position>(worker) {
            *pos = tile;
        }
        Ok(())
    }
}
