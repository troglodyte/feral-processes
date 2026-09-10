//! When a program stops working because of how it feels about the place.
//!
//! `OffShift`'s gate on a different meter, and deliberately built to its
//! shape rather than to a new one. A need runs down and something in the base
//! services it; morale has no reserve to refill, so the errand buys a
//! **memory** instead — `Game::note_respites` writes a fondness for the
//! amenity the body took its break at, and `Game::morale` folds it back
//! through the one door every other memory already goes through. There is no
//! second meter and nothing new is stored about the route.
//!
//! **The errand is gated on there being one.** A base with no amenity, or one
//! walled off from where the body stands, keeps its disgruntled programs in
//! the posting pool — which is where `Game::refuses_post` governs them. That
//! is what stops the mild rung quietly becoming unreachable when the severe
//! one takes everybody out of the pool anyway.
//!
//! **The one thing that must not lapse here is the hysteresis.** In below
//! `MORALE_DOWNS_TOOLS_AT`, out at `MORALE_RECOVERED_AT`, and the gap between
//! them is the feature. Read off the current value alone, a body downs tools
//! and picks them up again on alternate ticks at the boundary — the same
//! flicker `components::OffShift` exists to stop, and the reason both are
//! stored rather than derived.

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::TaskKind;
use crate::components::{
    Carrying, CarryingProgram, Disgruntled, Downed, Grievance, OffShift, Position,
};
use crate::game::base::hauling::{NoPost, step_to_post};
use crate::game::base::offshift::{Amenities, in_reach};
use crate::resources::Locale;
use crate::tuning::{
    MORALE_DOWNS_TOOLS_AT, MORALE_LASHES_OUT_AT, MORALE_RECOVERED_AT, MORALE_SULKS_AT,
};
use bevy_ecs::prelude::Entity;

impl Game {
    /// Inserts, keeps or removes `Disgruntled` for each of `staff`.
    ///
    /// Run beside `update_off_shift`, before the posting half of
    /// `schedule_base_labour` reads `on_shift` — a body that downs tools this
    /// tick must not also be given a job this tick.
    ///
    /// **`morale` and not `opinion_of`.** This is a claim about the body, not
    /// about any one machine or tile: a program that has had a bad run
    /// everywhere is what acting out is, and a program that resents one
    /// machine has always been `drift_idle_staff`'s avoidance rule instead.
    /// The mirror of the choice the parking hook made, one level up.
    ///
    /// It draws no RNG, writes no log line and touches no `Task`. The
    /// standdown is `schedule_base_labour`'s, through the `on_shift` filter,
    /// which already frees a body and already reports the shortfall through
    /// `LabourDemand` — teaching a second function to do either would give
    /// the same state two writers.
    pub(crate) fn update_disgruntled(&mut self, staff: &[Entity]) {
        for &worker in staff {
            let morale = self.morale(worker);
            let marked = self.world.get::<Disgruntled>(worker).copied();
            // Asymmetric on purpose, and the asymmetry *is* the hysteresis:
            // the entry test is only asked of a body still working, and the
            // exit test only of one that has already stopped. A single
            // comparison against one number is the bug this shape exists to
            // make unwritable.
            match marked {
                Some(held) => {
                    if morale >= MORALE_RECOVERED_AT {
                        self.world.entity_mut(worker).remove::<Disgruntled>();
                    } else if let Some(now) = reached(morale)
                        && now > held.grievance
                    {
                        // **Ratchets, never eases.** Severity only ever
                        // climbs while the marker is held; a body comes back
                        // by recovering, not by wobbling up across the inner
                        // line. Easing here would give the boundary between
                        // the two rungs its own flicker — post, unpost, post
                        // — restarting a cronjob's progress every other tick,
                        // which is the anti-thrash rule this gate sits above.
                        // **The latch is carried across the ratchet.** The
                        // severity climbing is not news about the route, and
                        // a fresh `stranded: false` here would restart the
                        // per-beat Dijkstra the latch exists to stop.
                        self.world.entity_mut(worker).insert(Disgruntled {
                            grievance: now,
                            stranded: held.stranded,
                        });
                    }
                }
                None => {
                    if let Some(grievance) = reached(morale) {
                        self.world.entity_mut(worker).insert(Disgruntled {
                            grievance,
                            stranded: false,
                        });
                    }
                }
            }
        }
    }
}

impl Game {
    /// Whether this program has stopped taking postings altogether — the
    /// severe rung, and the only one the `on_shift` filter cares about.
    ///
    /// A sulking program is still in the pool: it works, just not
    /// everywhere. Reading the marker's presence instead of its severity is
    /// what would collapse the ladder back to one rung.
    ///
    /// **`>=` and not `==`.** `LashingOut` is strictly worse than
    /// `DownedTools` rather than a second axis, so a program that has started
    /// fighting has certainly stopped working — read as equality it would be
    /// handed jobs again on the way past the rung that took them away.
    pub(crate) fn has_downed_tools(&self, who: Entity) -> bool {
        self.world
            .get::<Disgruntled>(who)
            .is_some_and(|d| d.grievance >= Grievance::DownedTools)
    }

    /// Whether `worker` refuses to be posted to `post`.
    ///
    /// Only a sulking body refuses anything, and only a machine it holds a
    /// grudge against — `MEMORY_AVOIDANCE_THRESHOLD` against the structure's
    /// **kind**, which is the subject a `Structure` memory names. The same
    /// constant and the same comparison `drift_idle_staff` declines a tile
    /// on, so a program will not be posted somewhere it would not even stand.
    ///
    /// **Signed, so a fondness can never trigger a refusal**, which is the
    /// rule the parking hook states one level up.
    ///
    /// A `DigSite` is not a structure and has no kind to resent, so an
    /// `Excavate` want is never refused here — the arm skips structurally
    /// rather than by a check.
    pub(crate) fn refuses_post(&self, worker: Entity, post: Entity, kind: TaskKind) -> bool {
        if kind == TaskKind::Excavate {
            return false;
        }
        if self
            .world
            .get::<Disgruntled>(worker)
            .is_none_or(|d| d.grievance != Grievance::Sulking)
        {
            return false;
        }
        let Some(structure) = self.world.get::<crate::components::Structure>(post) else {
            return false;
        };
        let subject = crate::components::MemorySubject::Structure(structure.kind.clone());
        self.opinion_of(worker, &subject) < crate::tuning::MEMORY_AVOIDANCE_THRESHOLD
    }

    /// The index in `idle` of the last body willing to take `post`, or `None`
    /// if every one of them refuses it.
    ///
    /// Scanned from the end so the deepest-first order the caller built is
    /// preserved for everyone who is willing — a sulking body is stepped
    /// over, not promoted past.
    pub(crate) fn willing_index(
        &self,
        idle: &[Entity],
        post: Entity,
        kind: TaskKind,
    ) -> Option<usize> {
        (0..idle.len())
            .rev()
            .find(|&i| !self.refuses_post(idle[i], post, kind))
    }
}

/// Which rung `morale` has reached, or `None` for a program still content.
///
/// The entry side of the gate only — the exit is a single comparison against
/// `MORALE_RECOVERED_AT`, which is what keeps the whole ladder to one
/// hysteresis gap rather than one per rung.
pub(crate) fn reached(morale: f32) -> Option<Grievance> {
    // Worst-first, so the arms below are only reached by a program that has
    // not already cleared a deeper line.
    if morale <= MORALE_LASHES_OUT_AT {
        Some(Grievance::LashingOut)
    } else if morale <= MORALE_DOWNS_TOOLS_AT {
        Some(Grievance::DownedTools)
    } else if morale <= MORALE_SULKS_AT {
        Some(Grievance::Sulking)
    } else {
        None
    }
}

impl Game {
    /// Whether `who` is away from the line taking a break at an amenity.
    ///
    /// **The gate, stated once so it cannot drift.** All three must hold:
    ///
    /// 1. the body is `Disgruntled` at any rung — the mild one included,
    ///    which is what "a dip pulls a body off a post" means,
    /// 2. the errand has not been given up as unwalkable (`stranded`),
    /// 3. the base has somewhere to unwind at all.
    ///
    /// Failing it leaves the body **in the posting pool**, not stalled: that
    /// is the whole difference from `update_off_shift`'s gate, where failing
    /// *is* acting out. A program in a bad mood at a base with nowhere to go
    /// still works, and `refuses_post` decides where.
    pub(crate) fn on_respite(&self, who: Entity, amenities: &Amenities) -> bool {
        amenities.any()
            && self
                .world
                .get::<Disgruntled>(who)
                .is_some_and(|d| !d.stranded)
    }

    /// Whether the scheduler may hand `who` a job this beat — the posting
    /// half's filter, as one predicate rather than four chained closures.
    ///
    /// Written once because the `Carrying` escape is not uniform across the
    /// exclusions and a reader has to be able to see that at a glance: an
    /// off-shift body, a body that has downed tools and a body on respite all
    /// keep it, because freeing one holding a load destroys the goods and it
    /// is still standing in the base with something the line is waiting on. A
    /// `Downed` body does not, because it is going to the Bay regardless.
    ///
    /// **The two morale exclusions are not the same question, and that is the
    /// whole of the ladder keeping two rungs.** `has_downed_tools` is
    /// unconditional: a program at -50 does not work, and whether the base
    /// has anywhere to unwind has nothing to do with it. `on_respite` is
    /// gated on there being an errand to take, so a *sulking* program at a
    /// base with no amenity stays in the pool — which is where
    /// `refuses_post`, the mild rung's own consequence, governs it. Collapse
    /// them and that rung has nobody left to apply to.
    pub(crate) fn is_on_shift(&self, who: Entity, amenities: &Amenities) -> bool {
        if self.world.get::<Downed>(who).is_some() {
            return false;
        }
        // `CarryingProgram` beside `Carrying`, and for exactly its reason:
        // freeing a body mid-trip destroys what it is holding. A carrier is
        // a kill the player cannot get back, so the omission is worse here
        // than it is for a stack of fragments.
        if self.world.get::<Carrying>(who).is_some()
            || self.world.get::<CarryingProgram>(who).is_some()
        {
            return true;
        }
        self.world.get::<OffShift>(who).is_none()
            && !self.has_downed_tools(who)
            && !self.on_respite(who, amenities)
    }

    /// One step toward somewhere to stop, or the walk's verdict when there is
    /// no route.
    ///
    /// `step_off_shift`'s twin, and deliberately its shape: **this is the one
    /// place a route is judged**, an `Err` latches, and a body already in
    /// reach stands still rather than being offered a tile that would walk it
    /// straight back out again.
    pub(crate) fn step_respite(
        &mut self,
        worker: Entity,
        amenities: &Amenities,
    ) -> Result<(), NoPost> {
        let here = self
            .world
            .get::<Position>(worker)
            .copied()
            .ok_or(NoPost::NoRoute)?;
        let (site, _, radius) = amenities.nearest_any(here).ok_or(NoPost::NoRoute)?;
        if in_reach(here, site, radius) {
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
            // exactly as a hauler and an off-shift body do.
            return Ok(());
        };
        // The party's cell is the one rejection `step_to_post` cannot make
        // for itself: `Locale` is where the party stands in base space, and
        // the player's `Position` is pinned to the anchor out on the surface.
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

    /// Gives the errand up: the amenity exists and this body cannot walk to
    /// it, so it goes back in the posting pool and the walk is not attempted
    /// again until the mood recovers and takes the marker with it.
    ///
    /// **No memory is written here**, and that is the one asymmetry with
    /// `Game::fray`. A need that goes unanswered earns a grudge because the
    /// base failed at something it could have done; deepening the mood of a
    /// body that is already low enough to have gone looking would be a loop
    /// with no floor under it — every failed walk making the next one more
    /// certain. The player is told, because the line is the errand.
    pub(crate) fn strand_respite(&mut self, worker: Entity) {
        let Some(mut marker) = self.world.get_mut::<Disgruntled>(worker) else {
            return;
        };
        if marker.stranded {
            return;
        }
        marker.stranded = true;
        let who = self.creature_label(worker);
        self.log_base(format!(
            "{who} can't find a way to anywhere in this base worth stopping at."
        ));
    }
}
