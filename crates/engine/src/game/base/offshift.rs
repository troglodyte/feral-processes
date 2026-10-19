//! When a program leaves its post, where it goes, and what it costs the base
//! when it cannot.
//!
//! A reserve that has run below its need's `critical` takes the program off
//! shift; it walks to the nearest structure servicing that need, mills in its
//! reach until it is `content`, and goes back to work. **One gate decides whether a
//! need may pull a body off a post, and failing that gate *is* acting out** —
//! there is no third state where a program is stalled but content.

use std::collections::{BTreeMap, HashSet};

use crate::base_grid::BaseGrid;
use crate::components::{Needs, OffShift, Position, Structure};
use crate::game::base::hauling::{NoPost, Step, at_station, step_to_post};
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
    /// Every amenity once, whatever it services — the morale errand's index,
    /// where `by_need` is the needs errand's.
    ///
    /// A separate list rather than a fold over `by_need` at the read: a
    /// structure servicing two needs appears in that map twice, and the
    /// morale errand walks to a *building*, not to a service. Deduped by
    /// tile and sorted by it, `by_need`'s reason exactly.
    sites: Vec<(Position, StructureId, i32)>,
    /// `(need, tile) -> places still free`, filled by `Game::amenities` from
    /// what the staff hold. Empty for a system's own build, which allocates
    /// places itself and never routes; an absent entry reads as free.
    free: BTreeMap<(NeedId, (i32, i32)), usize>,
}

impl Amenities {
    /// Takes an iterator so a bevy system and a `&Game` can both build one.
    pub(crate) fn build<'a>(
        structures: impl Iterator<Item = (&'a StructureId, &'a Position)>,
        db: &StructureDb,
    ) -> Self {
        let mut by_need: BTreeMap<NeedId, Vec<(Position, f32, i32)>> = BTreeMap::new();
        // Keyed by tile so a structure servicing two needs lands here once,
        // holding the **widest** reach it offers: the morale errand asks
        // "am I at this building", and the narrowest of two radii would put
        // a body in reach of the amenity for one question and not the other.
        let mut sites: BTreeMap<(i32, i32), (Position, StructureId, i32)> = BTreeMap::new();
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
                sites
                    .entry((pos.x, pos.y))
                    .and_modify(|(_, _, reach)| *reach = (*reach).max(service.radius))
                    .or_insert((*pos, kind.clone(), service.radius));
            }
        }
        // **A total order, not bevy's iteration order.** `min_by_key` returns
        // the first of several equal minima, so two amenities equidistant from
        // one program must already be in a settled order before anything asks.
        for sites in by_need.values_mut() {
            sites.sort_by_key(|(p, _, _)| (p.x, p.y));
        }
        // A `BTreeMap` keyed by tile is already in that order.
        Self {
            by_need,
            sites: sites.into_values().collect(),
            free: BTreeMap::new(),
        }
    }

    /// The same index with each amenity's free places recorded, for routing.
    pub(crate) fn with_free_places(mut self, free: BTreeMap<(NeedId, (i32, i32)), usize>) -> Self {
        self.free = free;
        self
    }

    /// Every amenity servicing `need`, sorted by tile: `(tile, rate, radius)`.
    pub(crate) fn servicing(&self, need: &NeedId) -> &[(Position, f32, i32)] {
        self.by_need.get(need).map_or(&[], Vec::as_slice)
    }

    /// Where an `OffShift(need)` program at `from` should head: the amenity
    /// whose place it already holds, else the nearest with a free place, else
    /// the nearest at all, where it waits in reach still draining.
    ///
    /// `nearest`'s total order decides every tie.
    pub(crate) fn route(
        &self,
        need: &NeedId,
        from: Position,
        held: Option<(i32, i32)>,
    ) -> Option<(Position, f32, i32)> {
        let list = self.by_need.get(need)?;
        if let Some(own) = held.and_then(|h| list.iter().find(|(p, _, _)| (p.x, p.y) == h)) {
            return Some(*own);
        }
        let distance = |p: &Position| (p.x - from.x).abs().max((p.y - from.y).abs());
        list.iter()
            .filter(|(p, _, _)| {
                self.free
                    .get(&(need.clone(), (p.x, p.y)))
                    .is_none_or(|free| *free > 0)
            })
            .min_by_key(|(p, _, _)| (distance(p), p.x, p.y))
            .copied()
            .or_else(|| self.nearest(need, from))
    }

    /// Whether the base has anywhere to unwind at all — the morale errand's
    /// half of the gate, `has`'s counterpart. A base with no amenity offers
    /// no errand, which is what keeps `Game::refuses_post` reachable.
    pub(crate) fn any(&self) -> bool {
        !self.sites.is_empty()
    }

    /// The amenity a program at `from` would take a break at, whatever it
    /// services, with the kind it will come away fond of and its reach.
    ///
    /// `nearest`'s total order — the list is sorted by tile and `min_by_key`
    /// takes the first of several equal minima, so two equidistant amenities
    /// resolve the same way whichever order they were built in.
    pub(crate) fn nearest_any(&self, from: Position) -> Option<(Position, StructureId, i32)> {
        self.sites
            .iter()
            .min_by_key(|(p, _, _)| ((p.x - from.x).abs().max((p.y - from.y).abs()), p.x, p.y))
            .cloned()
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

/// How many programs an amenity can serve at once: the capacity, or fewer
/// when fewer tiles are open to stand on. **One function, called by the sim
/// and by `need_coverage`**, so a boxed-in amenity reads the same on the
/// coverage screen as it behaves in the tick.
pub(crate) fn effective_places(open_tiles: usize) -> usize {
    open_tiles.min(crate::tuning::AMENITY_CAPACITY)
}

/// Tiles in reach of an amenity that a program could stand on: walkable
/// ground holding no structure (`taken` is every structure's footprint,
/// `Game::structure_tiles`' set). Reach is `in_reach`'s: Chebyshev
/// `max(radius, 1)`. A body standing there does not close the tile; bodies
/// squeeze past.
pub(crate) fn open_tiles(
    grid: &BaseGrid,
    site: Position,
    radius: i32,
    taken: &HashSet<(i32, i32)>,
) -> usize {
    let reach = radius.max(1);
    (site.x - reach..=site.x + reach)
        .flat_map(|x| (site.y - reach..=site.y + reach).map(move |y| (x, y)))
        .filter(|&(x, y)| grid.walkable(x, y) && !taken.contains(&(x, y)))
        .count()
}

/// `effective_places` over the ground, for a world that has no base grid
/// (a hand-built fixture) reading the bare capacity.
pub(crate) fn places_of(
    grid: Option<&BaseGrid>,
    site: Position,
    radius: i32,
    taken: &HashSet<(i32, i32)>,
) -> usize {
    grid.map_or(crate::tuning::AMENITY_CAPACITY, |g| {
        effective_places(open_tiles(g, site, radius, taken))
    })
}

/// One staff program as the place allocation sees it.
pub(crate) struct Contender {
    pub who: Entity,
    pub at: Position,
    /// The amenity tile that held this program's place last tick.
    pub held: Option<(i32, i32)>,
    /// Below `content` and off duty: may claim a place it does not hold. A
    /// program posted at a job never claims one, so it cannot hold a place
    /// against an off-shift program that walked there for it.
    pub wants: bool,
    /// The reserve, which ranks newcomers: the emptiest is served first.
    pub reserve: f32,
    /// Short of a full reserve: may keep a place it holds.
    pub keeps: bool,
}

/// Who holds a place at which amenity this tick, for one need. `sites` are
/// `(tile, places, radius)` in tile order.
///
/// **Incumbents first, across every amenity, and only then the newcomers.**
/// A program in reach keeps the place it holds whatever walks up beside it —
/// entity order breaks ties among incumbents only when places shrank — and a
/// free place goes to the emptiest waiting program, then the nearest, then
/// entity order.
/// Deciding incumbents for all amenities before any newcomer is what stops a
/// program in reach of two from walking out of the place it holds.
pub(crate) fn allocate(
    sites: &[(Position, usize, i32)],
    contenders: &[Contender],
) -> BTreeMap<Entity, (i32, i32)> {
    let mut order: Vec<&Contender> = contenders.iter().collect();
    order.sort_by_key(|c| c.who);
    let mut granted: BTreeMap<Entity, (i32, i32)> = BTreeMap::new();
    let mut left: Vec<usize> = sites.iter().map(|(_, places, _)| *places).collect();
    for (i, (site, _, radius)) in sites.iter().enumerate() {
        for c in &order {
            if left[i] == 0 {
                break;
            }
            if c.keeps
                && c.held == Some((site.x, site.y))
                && in_reach(c.at, *site, *radius)
                && !granted.contains_key(&c.who)
            {
                granted.insert(c.who, (site.x, site.y));
                left[i] -= 1;
            }
        }
    }
    for (i, (site, _, radius)) in sites.iter().enumerate() {
        let mut waiting: Vec<&&Contender> = order
            .iter()
            .filter(|c| c.wants && !granted.contains_key(&c.who) && in_reach(c.at, *site, *radius))
            .collect();
        waiting.sort_by(|a, b| {
            let dist = |c: &Contender| (c.at.x - site.x).abs().max((c.at.y - site.y).abs());
            a.reserve
                .total_cmp(&b.reserve)
                .then_with(|| dist(a).cmp(&dist(b)))
                .then_with(|| a.who.cmp(&b.who))
        });
        for c in waiting.into_iter().take(left[i]) {
            granted.insert(c.who, (site.x, site.y));
        }
    }
    granted
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
    /// How far `who`'s reserves have run down, signed and baselined at zero
    /// — the screens' door onto `needs::strain`, and a **caller** of it
    /// rather than a second fold. `Game::morale`'s shape exactly.
    ///
    /// A body carrying no `Needs` reads zero rather than panicking: hostiles,
    /// structures and the player are safe here without a branch.
    pub fn need_strain(&self, who: Entity) -> f32 {
        self.world
            .get::<Needs>(who)
            .map(|n| crate::needs::strain(n, self.world.resource::<NeedDb>()))
            .unwrap_or(0.0)
    }

    /// Where `who`'s reserves stand, one row per loaded def **in id order** —
    /// never by value, or the labels move under the eye reading them.
    ///
    /// Empty for a program carrying no `Needs` and for an install with
    /// `assets/needs/` deleted, so the manifest omits the section entirely
    /// rather than drawing it empty.
    pub fn need_rows(&self, who: Entity) -> Vec<crate::views::NeedRow> {
        let Some(store) = self.world.get::<Needs>(who) else {
            return Vec::new();
        };
        let errand = self.world.get::<OffShift>(who).map(|o| o.need.clone());
        self.world
            .resource::<NeedDb>()
            .iter()
            .filter_map(|def| {
                let value = store.get(&def.id)?;
                Some(crate::views::NeedRow {
                    name: def.name.clone(),
                    band: crate::views::need_band(value / crate::needs::NEED_MAX),
                    servicing: (errand.as_ref() == Some(&def.id)).then(|| def.servicing.clone()),
                })
            })
            .collect()
    }

    /// Every reserve `who` carries as a number, sorted by need id.
    ///
    /// `need_rows` is the player's read (bands, names); this is the
    /// headless bench's — it samples levels against the def's thresholds.
    /// A view rather than a `World` accessor, because the renderer and
    /// bench must never be handed the `World`. Empty for a body with no
    /// `Needs`.
    pub fn need_levels(&self, who: Entity) -> Vec<crate::views::NeedLevel> {
        let Some(store) = self.world.get::<Needs>(who) else {
            return Vec::new();
        };
        let db = self.world.resource::<NeedDb>();
        let mut levels: Vec<_> = store
            .iter()
            .filter_map(|(id, level)| {
                let def = db.get(id)?;
                Some(crate::views::NeedLevel {
                    id: id.to_string(),
                    level,
                    critical: def.critical,
                    content: def.content,
                })
            })
            .collect();
        levels.sort_by(|a, b| a.id.cmp(&b.id));
        levels
    }

    /// The rung `who` has reached on the grievance ladder, by wire name, or
    /// `None` if not disgruntled. A view for the headless bench, for the
    /// same reason as `need_levels`.
    pub fn grievance(&self, who: Entity) -> Option<&'static str> {
        self.world
            .get::<crate::components::Disgruntled>(who)
            .map(|d| d.grievance.as_str())
    }

    /// Whether the scheduler would hand `who` a job this beat — `is_on_shift`
    /// for a `&self` caller, so a reader of the base asks the sim's own
    /// question instead of reconstructing it from `program_errand_label`,
    /// which says nothing of a downed body or a program that downed tools at
    /// a base with nowhere to go.
    pub fn on_shift(&self, who: Entity) -> bool {
        self.is_on_shift(who, &self.amenities_here())
    }

    /// The examine line's tail: what `who` has walked off to do, or `None`
    /// for a program that is on shift.
    ///
    /// Two errands, and the need outranks the mood exactly as the drift's
    /// arms do. A need names itself through the def's own `servicing` string;
    /// a respite has no def behind it, so the verb is fixed here beside
    /// `"recovering"` and `"in party"` rather than being authored.
    pub fn program_errand_label(&self, who: Entity) -> Option<String> {
        if let Some(need) = self.world.get::<OffShift>(who) {
            return self
                .world
                .resource::<NeedDb>()
                .get(&need.need)
                .map(|def| def.servicing.clone());
        }
        // The same three clauses `on_respite` reads, with the amenity half
        // asked of the world directly — this is a `&self` screen door and
        // cannot build the beat's index.
        let taking_one = self
            .world
            .get::<crate::components::Disgruntled>(who)
            .is_some_and(|d| !d.stranded);
        (taking_one && self.base_has_an_amenity()).then(|| "Taking a break".to_string())
    }

    /// Whether anything in the base services a need at all — `Amenities::any`
    /// for a caller that has only `&self`.
    ///
    /// Short-circuits on the first one, and is deliberately **not** a cached
    /// figure: a cache would be a new `Resource` and another query-iteration-
    /// order shift, which is the same call `Amenities` itself makes.
    pub(crate) fn base_has_an_amenity(&self) -> bool {
        let db = self.world.resource::<StructureDb>();
        self.world.iter_entities().any(|e| {
            e.get::<Structure>()
                .and_then(|s| db.get(&s.kind))
                .is_some_and(|def| def.services.iter().any(|s| s.rate().is_some()))
        })
    }

    /// Builds this pass's amenity index off the world's own structures, with
    /// each amenity's free places read off what the staff hold.
    pub(crate) fn amenities(&mut self) -> Amenities {
        let mut query = self.world.query::<(&Structure, &Position)>();
        let sites: Vec<(StructureId, Position)> = query
            .iter(&self.world)
            .map(|(s, p)| (s.kind.clone(), *p))
            .collect();
        let amenities = Amenities::build(
            sites.iter().map(|(kind, p)| (kind, p)),
            self.world.resource::<StructureDb>(),
        );
        let taken = self.structure_tiles();
        let mut held: BTreeMap<(NeedId, (i32, i32)), usize> = BTreeMap::new();
        let mut needs = self.world.query::<&Needs>();
        for n in needs.iter(&self.world) {
            for (need, tile) in n.serviced() {
                *held.entry((need.clone(), tile)).or_default() += 1;
            }
        }
        let grid = self.world.get_resource::<BaseGrid>();
        let mut free = BTreeMap::new();
        for (need, list) in &amenities.by_need {
            for (site, _, radius) in list {
                let places = places_of(grid, *site, *radius, &taken);
                let used = held
                    .get(&(need.clone(), (site.x, site.y)))
                    .copied()
                    .unwrap_or(0);
                free.insert(
                    (need.clone(), (site.x, site.y)),
                    places.saturating_sub(used),
                );
            }
        }
        amenities.with_free_places(free)
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
    // No shipped amenity or Repair Bay declares a footprint past 1, so
    // `at_station`'s side is the default here —
    // `tests::assets::every_amenity_and_repair_bay_declares_a_footprint_of_one`
    // is the census that holds it.
    at_station(here, site, 1) || ((site.x - here.x).abs().max((site.y - here.y).abs()) <= radius)
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
    ///
    /// `Ok(Some((site, radius)))` is arrival: the leash `drift_idle_staff`
    /// mills the body inside, so it does not read as frozen at the building.
    pub(crate) fn step_off_shift(
        &mut self,
        worker: Entity,
        amenities: &Amenities,
    ) -> Result<Option<(Position, i32)>, NoPost> {
        let Some(need) = self.world.get::<OffShift>(worker).map(|o| o.need.clone()) else {
            return Ok(None);
        };
        let here = self
            .world
            .get::<Position>(worker)
            .copied()
            .ok_or(NoPost::NoRoute)?;
        let held = self
            .world
            .get::<Needs>(worker)
            .and_then(|n| n.serviced_at(&need));
        let (site, _, radius) = amenities.route(&need, here, held).ok_or(NoPost::NoRoute)?;
        if in_reach(here, site, radius) {
            // Arrived. It stays in reach until it is content — the drift's
            // unleashed shape would walk it straight back off again.
            return Ok(Some((site, radius)));
        }
        self.walk_idle_toward(worker, here, site).map(|()| None)
    }

    /// One rigid step from `here` toward `site`: the walk tail `step_respite`,
    /// `step_off_shift` and the tidy errand share. `Err` is the one place a
    /// route is judged; `Ok` covers a step taken and a wait alike.
    ///
    /// No shipped amenity is wider than 1, so `side` is the default — held by
    /// `tests::assets::every_amenity_and_repair_bay_declares_a_footprint_of_one`.
    pub(crate) fn walk_idle_toward(
        &mut self,
        worker: Entity,
        here: Position,
        site: Position,
    ) -> Result<(), NoPost> {
        let blocked = self.blocked_tiles().rigid();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        let Some(Step { to: tile, .. }) = step_to_post(
            self.world.resource::<BaseGrid>(),
            here,
            site,
            1,
            &blocked,
            pocket_radius,
        )?
        else {
            // The field admits nowhere better than where it stands. It waits,
            // exactly as a hauler does.
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
            // A grudge does not heal by accident: rivals sharing an amenity
            // earn no warmth from it.
            .filter(|&id| !self.bond(worker, id).avoids())
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
    /// edge, says so once — and writes the grudge on one of the two branches.
    ///
    /// Both halves of failing the gate come through here — nothing in the
    /// base services this need, and the amenity is walled off from where this
    /// program stands — because they are one state as far as the program is
    /// concerned and one latch as far as the base is. They say **different
    /// sentences**, because they leave the player different errands: that is
    /// `NoPost::BoxedIn`-versus-`NoRoute`'s rule one level up.
    ///
    /// **The two branches write different memories, and the asymmetry is
    /// about blame rather than about feeling.** The unreachable branch earns
    /// `frayed_here`, `MemorySubject::BaseTile` at the program's **own**
    /// `Position` — `note_strandings`' subject and for its reason: "worn thin
    /// here" is a claim about where the body is standing, and it is what
    /// `drift_idle_staff` reads back. The base earns that when it *had* an
    /// answer and could not deliver it — a Bay walled off, or destroyed —
    /// which is a complaint with an errand attached.
    ///
    /// The quiet branch earns `ran_down`, `MemorySubject::Nothing`. A base
    /// with no amenity at all has done nothing to be *held against it*: the
    /// player may not have researched one, may not have the materials, and
    /// has never been told they want one, so a memory naming a tile there is
    /// a program resenting the base for a building that was never an option
    /// — measured against a real save that was worth -27 on its own, most of
    /// the way to a standdown, with no lever anywhere to answer it. A
    /// `Nothing` subject holds nothing responsible, so the rule stands and
    /// the meter still moves. **What is withheld is the blame, not the
    /// memory**, and that distinction is what makes needs reach the morale
    /// ladder at all instead of being decorative there.
    ///
    /// **Neither is written before `base_is_established`.** While a base is
    /// still getting started, needs do not count against it. The lines are
    /// said either way, because the line is the errand.
    pub(crate) fn fray(&mut self, worker: Entity, need: &NeedId, unreachable: bool) {
        let Some(mut store) = self.world.get_mut::<Needs>(worker) else {
            return;
        };
        if !store.latch(need) {
            return;
        }
        let at = self.world.get::<Position>(worker).copied();
        let who = self.creature_label(worker);
        let tick = self.current_tick();
        self.record(|_| crate::telemetry::Record::Fray {
            tick,
            who: who.clone(),
            need: need.to_string(),
            unreachable,
        });
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
        if !self.base_is_established() {
            return;
        }
        match (unreachable, at) {
            (true, Some(at)) => {
                self.remember(
                    worker,
                    "frayed_here",
                    crate::components::MemorySubject::BaseTile { x: at.x, y: at.y },
                );
            }
            (false, _) => {
                self.remember(
                    worker,
                    "ran_down",
                    crate::components::MemorySubject::Nothing,
                );
            }
            (true, None) => {}
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
