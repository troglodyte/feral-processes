//! When a program leaves its post, where it goes, and what it costs the base
//! when it cannot.
//!
//! A reserve that has run below its need's `critical` takes the program off
//! shift; it walks to the nearest structure servicing that need, stands there
//! until it is `content`, and goes back to work. **One gate decides whether a
//! need may pull a body off a post, and failing that gate *is* acting out** —
//! there is no third state where a program is stalled but content.

use std::collections::BTreeMap;

use crate::base_grid::BaseGrid;
use crate::components::{Needs, OffShift, Position, Structure};
use crate::game::base::hauling::{NoPost, at_station, step_to_post};
use crate::needs::{NeedDb, NeedId};
use crate::resources::Locale;
use crate::structures::{StructureDb, StructureId};
use bevy_ecs::prelude::Entity;

use crate::Game;

/// Where the base's amenities stand, indexed by what they service.
///
/// Built **once per caller pass** over the structure query, never once per
/// program: a bevy system builds one a tick and `drift_idle_staff` one a beat.
/// Two cheap builds beat one stale cached copy, and a cached one would be a
/// new `Resource` and another query-iteration-order shift.
pub(crate) struct Amenities {
    /// `need -> (tile, rate, radius)`, each list sorted by tile so a tie
    /// resolves the same way every run.
    by_need: BTreeMap<NeedId, Vec<(Position, f32, i32)>>,
}

impl Amenities {
    /// Takes an iterator so a bevy system and a `&Game` can both build one.
    pub(crate) fn build<'a>(
        structures: impl Iterator<Item = (&'a StructureId, &'a Position)>,
        db: &StructureDb,
    ) -> Self {
        let mut by_need: BTreeMap<NeedId, Vec<(Position, f32, i32)>> = BTreeMap::new();
        for (kind, pos) in structures {
            let Some(def) = db.get(kind) else {
                continue;
            };
            for service in &def.services {
                // A nonsense rate is refused here rather than downstream, so
                // an amenity that cannot refill anything is not one a program
                // will walk across the base to stand at.
                let Some(rate) = service.rate() else {
                    continue;
                };
                by_need
                    .entry(service.need.clone())
                    .or_default()
                    .push((*pos, rate, service.radius));
            }
        }
        // **A total order, not bevy's iteration order.** `min_by_key` returns
        // the first of several equal minima, so two amenities equidistant from
        // one program must already be in a settled order before anything asks.
        for sites in by_need.values_mut() {
            sites.sort_by_key(|(p, _, _)| (p.x, p.y));
        }
        Self { by_need }
    }

    /// Whether anything in the base services this need **at all**. The second
    /// clause of the gate: a need nothing answers is a stall, not an errand.
    pub(crate) fn has(&self, need: &NeedId) -> bool {
        self.by_need.contains_key(need)
    }

    /// The amenity a program at `from` would walk to, with its rate and reach.
    ///
    /// Ties break on a **total** `(chebyshev distance, x, y)` order — the list
    /// is already sorted by tile, and `min_by_key` takes the first of several
    /// equal minima, which is where bevy's unstable iteration order would
    /// otherwise leak in.
    pub(crate) fn nearest(&self, need: &NeedId, from: Position) -> Option<(Position, f32, i32)> {
        self.by_need
            .get(need)?
            .iter()
            .min_by_key(|(p, _, _)| ((p.x - from.x).abs().max((p.y - from.y).abs()), p.x, p.y))
            .copied()
    }
}

/// The need furthest below its own `critical`, or `None` if none is.
///
/// Measured as a **fraction of `critical`** so two needs with different
/// thresholds compare fairly — a Slack at 24 of 25 is barely short, a
/// Coherence at 4 of 20 is desperate, and comparing the raw numbers would
/// pick the wrong one. Ties break by id.
pub(crate) fn pressing_need(needs: &Needs, db: &NeedDb) -> Option<NeedId> {
    let mut best: Option<(f32, &NeedId)> = None;
    for (id, value) in needs.iter() {
        let Some(def) = db.get(id) else {
            continue;
        };
        if def.critical <= 0.0 || value >= def.critical {
            continue;
        }
        let shortfall = (def.critical - value) / def.critical;
        // `>` and not `>=`, so the first of two equal shortfalls wins — and
        // `Needs::iter` is id-ordered, which makes that a settled tie-break
        // rather than a map's whim.
        if best.is_none_or(|(worst, _)| shortfall > worst) {
            best = Some((shortfall, id));
        }
    }
    best.map(|(_, id)| id.clone())
}

impl Game {
    /// Builds this pass's amenity index off the world's own structures.
    pub(crate) fn amenities(&mut self) -> Amenities {
        let mut query = self.world.query::<(&Structure, &Position)>();
        let sites: Vec<(StructureId, Position)> = query
            .iter(&self.world)
            .map(|(s, p)| (s.kind.clone(), *p))
            .collect();
        Amenities::build(
            sites.iter().map(|(kind, p)| (kind, p)),
            self.world.resource::<StructureDb>(),
        )
    }

    /// Inserts, keeps or removes `OffShift` for each of `staff`.
    ///
    /// **The gate, stated once so it cannot drift.** `OffShift(need)` is
    /// inserted when all three hold:
    ///
    /// 1. the reserve is below the def's `critical`,
    /// 2. `amenities.has(need)` — something in the base services it at all,
    /// 3. the need is **not latched** in `Needs::stalled_announced`.
    ///
    /// It is removed when the reserve reaches `content`, when the amenity
    /// stops existing, or when the walk reports `NoPost::NoRoute`.
    ///
    /// **Reachability is never asked as its own question here.** It is
    /// discovered by the walk, and a `NoRoute` sets the latch — which is what
    /// stops the obvious flicker of insert → failed step → remove → insert on
    /// every beat. One Dijkstra per newly off-shift body, then nothing until
    /// the need recovers.
    pub(crate) fn update_off_shift(&mut self, staff: &[Entity], amenities: &Amenities) {
        for &worker in staff {
            let Some(needs) = self.world.get::<Needs>(worker) else {
                continue;
            };
            let current = self.world.get::<OffShift>(worker).map(|o| o.need.clone());
            let db = self.world.resource::<NeedDb>();
            let mut finished = false;
            let verdict = match &current {
                Some(need) => {
                    let done = db
                        .get(need)
                        .is_none_or(|def| needs.get(need).is_none_or(|v| v >= def.content));
                    finished = done;
                    (!done && amenities.has(need)).then(|| need.clone())
                }
                None => pressing_need(needs, db)
                    .filter(|need| amenities.has(need) && !needs.is_latched(need)),
            };
            // The other half of failing the gate: nothing in the base answers
            // this need at all. Collected here and acted on below, because
            // `fray` takes `&mut self` and `needs` is a borrow of the world.
            let unanswered = match &current {
                Some(_) => None,
                None => pressing_need(needs, db).filter(|need| !amenities.has(need)),
            };
            // A reserve back above its own `critical` clears the latch, so a
            // need that recovers and runs down again complains a second time.
            let recovered: Vec<NeedId> = needs
                .iter()
                .filter(|(id, value)| db.get(id).is_some_and(|def| *value >= def.critical))
                .map(|(id, _)| id.clone())
                .collect();
            if !recovered.is_empty()
                && let Some(mut store) = self.world.get_mut::<Needs>(worker)
            {
                for id in recovered {
                    store.unlatch(&id);
                }
            }
            if let Some(need) = unanswered {
                self.fray(worker, &need, false);
            }
            // **The edge where servicing completes**, and the only place the
            // social memory is written. Once per stretch, never per tick:
            // `note_postings`' doc comment states the cost, and it applies
            // unchanged — a per-tick writer saturates `strike_cap` in three
            // ticks, makes `strikes` meaningless, and (because `remember`
            // evicts at the tail of every write) makes eviction eager for
            // exactly the programs living the most.
            if finished && let Some(need) = current.clone() {
                self.note_idling(worker, &need, amenities, staff);
            }
            match (current, verdict) {
                (Some(_), None) => {
                    self.world.entity_mut(worker).remove::<OffShift>();
                }
                (_, Some(need)) => {
                    self.world.entity_mut(worker).insert(OffShift { need });
                }
                (None, None) => {}
            }
        }
    }
}

/// Whether a program standing at `here` is being serviced by an amenity at
/// `site` with reach `radius`.
///
/// `0` means `at_station`'s reach — standing beside it — rather than standing
/// *on* it, which no program ever does: a structure's own tile is blocked.
/// Written as an `||` rather than as a special case so the two readings agree
/// at `radius: 1` instead of stepping over each other.
pub(crate) fn in_reach(here: Position, site: Position, radius: i32) -> bool {
    at_station(here, site) || ((site.x - here.x).abs().max((site.y - here.y).abs()) <= radius)
}

impl Game {
    /// One step toward the amenity for `worker`'s `OffShift` need, or the
    /// gate's verdict when there is no route.
    ///
    /// **This is the only place reachability is decided.** It is not asked as
    /// its own question anywhere else: an `Err` here sets the latch and drops
    /// `OffShift`, which is what stops the insert → failed step → remove →
    /// insert flicker the gate would otherwise run on every beat.
    ///
    /// Rides `hauling::step_to_post`, the walk the dig crew already takes.
    /// There is no second walk.
    pub(crate) fn step_off_shift(
        &mut self,
        worker: Entity,
        amenities: &Amenities,
    ) -> Result<(), NoPost> {
        let Some(need) = self.world.get::<OffShift>(worker).map(|o| o.need.clone()) else {
            return Ok(());
        };
        let here = self
            .world
            .get::<Position>(worker)
            .copied()
            .ok_or(NoPost::NoRoute)?;
        let (site, _, radius) = amenities.nearest(&need, here).ok_or(NoPost::NoRoute)?;
        if in_reach(here, site, radius) {
            // Arrived. It stands here until it is content — the drift's other
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
        // An off-shift body is still a body: the party's cell is the one
        // rejection `step_to_post` cannot make for itself, since `Locale` is
        // where the party stands in base space and the player's `Position` is
        // pinned to the anchor out on the surface.
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

    /// Writes an `idled_with` memory naming every *other* program that was in
    /// reach of the same amenity at the moment `worker` finished with it.
    ///
    /// Named by `ProgramId` and not by `Entity`, `Game::remember`'s rule: an
    /// entity id is not stable across a save round trip and a program's
    /// identity is.
    fn note_idling(
        &mut self,
        worker: Entity,
        need: &NeedId,
        amenities: &Amenities,
        staff: &[Entity],
    ) {
        let Some(here) = self.world.get::<Position>(worker).copied() else {
            return;
        };
        let Some((site, _, radius)) = amenities.nearest(need, here) else {
            return;
        };
        // A program whose reserve filled somewhere else — a mod's wide radius,
        // or the amenity demolished under it — was not idling *with* anyone.
        if !in_reach(here, site, radius) {
            return;
        }
        let company: Vec<crate::components::ProgramId> = staff
            .iter()
            .filter(|&&other| other != worker)
            .filter(|&&other| {
                self.world
                    .get::<Position>(other)
                    .is_some_and(|p| in_reach(*p, site, radius))
            })
            .filter_map(|&other| {
                self.world
                    .get::<crate::components::ProgramId>(other)
                    .copied()
            })
            .collect();
        for id in company {
            self.remember(
                worker,
                "idled_with",
                crate::components::MemorySubject::Program(id),
            );
        }
    }

    /// **The one edge.** Latches `need` on `worker` and, if that was the
    /// edge, says so once and writes the grudge.
    ///
    /// Both halves of failing the gate come through here — nothing in the
    /// base services this need, and the amenity is walled off from where this
    /// program stands — because they are one state as far as the program is
    /// concerned and one latch as far as the base is. They say **different
    /// sentences**, because they leave the player different errands: that is
    /// `NoPost::BoxedIn`-versus-`NoRoute`'s rule one level up.
    ///
    /// The grudge is `MemorySubject::BaseTile` at the program's **own**
    /// `Position`, `note_strandings`' subject and for its reason: "worn thin
    /// here" is a claim about where the body is standing, and it is what
    /// `drift_idle_staff` reads back.
    pub(crate) fn fray(&mut self, worker: Entity, need: &NeedId, unreachable: bool) {
        let Some(mut store) = self.world.get_mut::<Needs>(worker) else {
            return;
        };
        if !store.latch(need) {
            return;
        }
        let at = self.world.get::<Position>(worker).copied();
        let who = self.creature_label(worker);
        let what = self
            .world
            .resource::<NeedDb>()
            .get(need)
            .map(|def| def.name.clone())
            .unwrap_or_else(|| need.to_string());
        if unreachable {
            self.log_base(format!(
                "{who} can't find a way to anything that would restore its {what}."
            ));
        } else {
            self.log_base(format!(
                "{who} is out of {what} and there's nothing in the base that restores it."
            ));
        }
        if let Some(at) = at {
            self.remember(
                worker,
                "frayed_here",
                crate::components::MemorySubject::BaseTile { x: at.x, y: at.y },
            );
        }
    }

    /// Gives the errand up and latches the need: the amenity exists but this
    /// body cannot reach it, which is a different complaint from there being
    /// no amenity at all and leaves the player a different errand.
    pub(crate) fn strand_off_shift(&mut self, worker: Entity) {
        let Some(need) = self.world.get::<OffShift>(worker).map(|o| o.need.clone()) else {
            return;
        };
        self.fray(worker, &need, true);
        self.world.entity_mut(worker).remove::<OffShift>();
    }
}
