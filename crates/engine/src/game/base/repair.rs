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
use bevy_ecs::prelude::Entity;

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
            let Some((site, rate, radius)) = bays.nearest(here) else {
                continue;
            };
            if !in_reach(here, site, radius) {
                continue;
            }
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
