//! Being under study — the Research Station's pen.
//!
//! The home for `Game::study_pen` because the pin and unpin doors that join
//! it here in a later task read and write the same corner, and
//! `building.rs` is already past 1,200 lines owning a different
//! responsibility: placement and demolition, not what a structure's
//! footprint is *for*.

use std::collections::{HashMap, HashSet};

use crate::base_grid::BaseGrid;
use crate::game::base::hauling::NoPost;
use crate::game::pursuit::walk_field;
use crate::resources::ActiveResearch;
use crate::tuning::haul_walk_radius;
use crate::world::NEIGHBOURS;
use crate::*;

impl Game {
    /// The footprint cell diagonally opposite `structure`'s anchor — its
    /// pen — or `None` when the structure's def does not declare `studies`.
    ///
    /// **The one door.** The pin, the walk, the block check, the draw and
    /// the consumption all call this rather than each re-deriving a corner
    /// from a footprint and a side.
    ///
    /// `Game::step_to_study` below is this door's first production caller,
    /// which is what retires the `#[allow(dead_code)]` Task 3 left on this
    /// function — the plan expected `Game::pin_subject` (Task 5) to be the
    /// one to do it, but the walk needed it one task sooner.
    pub(crate) fn study_pen(&self, structure: Entity) -> Option<(i32, i32)> {
        let kind = self.world.get::<Structure>(structure)?.kind.clone();
        let def = self.world.resource::<StructureDb>().get(&kind)?;
        if !def.studies {
            return None;
        }
        let pos = self.world.get::<Position>(structure)?;
        let side = i32::from(def.footprint);
        Some((pos.x + side - 1, pos.y + side - 1))
    }

    /// The `(x, y)`-sorted first standing `studies` structure, or `None` if
    /// none stands — `assembler_system`'s sorting rule, so `research_block`'s
    /// gate and `settle_research`'s "which subject is spent" read the same
    /// station and cannot resolve two Stations differently between runs.
    ///
    /// `&self`, like `producers_of` beside it: `research_block` is read from
    /// the screen as well as from `select_research`, so this walks
    /// `World::iter_entities` rather than taking the `&mut self` a bevy
    /// query needs.
    pub fn study_station(&self) -> Option<Entity> {
        self.first_study_station()
    }

    /// `study_station`'s own implementation — kept private so the sort rule
    /// stays in one place; `study_station` is its public name for callers
    /// outside the engine crate (the base menu's row and `Mode::PinSubject`'s
    /// picker) that have no business naming a "first" anything themselves.
    fn first_study_station(&self) -> Option<Entity> {
        let db = self.world.resource::<StructureDb>();
        let mut found: Vec<(i32, i32, Entity)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let kind = &e.get::<Structure>()?.kind;
                let pos = e.get::<Position>()?;
                let def = db.get(kind)?;
                def.studies.then_some((pos.x, pos.y, e.id()))
            })
            .collect();
        found.sort();
        found.first().map(|(_, _, e)| *e)
    }

    /// The program standing in the pen of `first_study_station`'s chosen
    /// station — `None` when no `studies` structure stands, or when its pen
    /// is empty. **The one door**: `research_block`'s gate,
    /// `settle_research`'s "which subject is spent" and `Mode::PinSubject`'s
    /// own choice of what to draw all call this rather than each re-deriving
    /// a station and a corner, so they cannot read the base's one active
    /// subject differently. `pub` rather than `pub(crate)` since the screen
    /// that reaches it lives in app-core, not this crate.
    pub fn pinned_subject(&self) -> Option<Entity> {
        let station = self.first_study_station()?;
        let pen = self.study_pen(station)?;
        self.world.iter_entities().find_map(|e| {
            let under_study = e.get::<components::UnderStudy>()?;
            if under_study.station != station {
                return None;
            }
            let pos = e.get::<Position>()?;
            ((pos.x, pos.y) == pen).then_some(e.id())
        })
    }

    /// Every tamed program you own that is pinned somewhere — the second
    /// half of the walking pool `drift_idle_staff` needs, since
    /// `Game::base_staff` excludes a subject by construction the moment
    /// `role_of` reads its `components::UnderStudy` marker. Unsorted: the
    /// caller appends this to an already-sorted `base_staff()` list, and
    /// this feature does not need a second stable order to make sense of —
    /// two subjects sharing a station is not a case the walk special-cases.
    pub(crate) fn under_study_bodies(&mut self) -> Vec<Entity> {
        let player = self.player_entity();
        let mut query = self
            .world
            .query::<(Entity, &Tamed, &components::UnderStudy)>();
        query
            .iter(&self.world)
            .filter(|(_, tamed, _)| tamed.owner == player)
            .map(|(e, ..)| e)
            .collect()
    }

    /// One step of `worker` toward its pen, or `NoPost::NoRoute` when there
    /// is nowhere to go — `hauling::step_to_post`'s question asked of an
    /// exact destination cell rather than one of a footprint's outer faces.
    ///
    /// **Why not `step_to_post`:** that walk arrives at any of
    /// `hauling::station_candidates`' cells, which are chosen *outside* a
    /// structure's whole footprint — the pen is one cell *inside* it. Asking
    /// `step_to_post` for this would have it refuse the one tile this walk
    /// is aimed at.
    ///
    /// The step rule is `hauling::post_field`'s, character for character,
    /// because the two have to agree about which tiles a body in base space
    /// may cross; what differs is which end of the walk the field is rooted
    /// at, `hauling::crew_reach`'s reason for the same duplication — rooted
    /// at the pen rather than at the worker, so `Ok(())` on arrival is a
    /// plain coordinate comparison and never a field lookup.
    pub(crate) fn step_to_study(&mut self, worker: Entity) -> Result<(), NoPost> {
        let station = self
            .world
            .get::<components::UnderStudy>(worker)
            .ok_or(NoPost::NoRoute)?
            .station;
        let pen = self.study_pen(station).ok_or(NoPost::NoRoute)?;
        let here = self
            .world
            .get::<Position>(worker)
            .copied()
            .ok_or(NoPost::NoRoute)?;
        if (here.x, here.y) == pen {
            // Arrived. Standing here is the whole of "under study" — see
            // `components::UnderStudy`'s doc for why nothing latches it.
            return Ok(());
        }
        let blocked = self.blocked_tiles();
        let start = (here.x, here.y);
        let Some(field) = self.pen_walk_field(pen, &blocked, start) else {
            return Err(NoPost::NoRoute);
        };
        let Some(&cost) = field.get(&start) else {
            return Err(NoPost::NoRoute);
        };
        let step = NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (here.x + dx, here.y + dy))
            .filter_map(|n| field.get(&n).map(|&c| (c, n.0, n.1)))
            .min()
            .filter(|&(c, ..)| c < cost)
            .map(|(_, x, y)| Position { x, y });
        if let Some(tile) = step
            && let Some(mut pos) = self.world.get_mut::<Position>(worker)
        {
            *pos = tile;
        }
        Ok(())
    }

    /// The walk field rooted at `pen`, `None` when the pen itself is already
    /// spoken for — the one door both `step_to_study` and `pin_subject`'s
    /// reachability refusal go through, so a program that was refused for
    /// want of a route and one that is actually being walked cannot read the
    /// question differently.
    ///
    /// A single pen holds one body. `walk_field` roots its search at `pen`
    /// and always marks its own root reachable at cost 0 — right for
    /// `hauling::post_field`'s station faces, which are pre-filtered to ones
    /// nobody occupies, and wrong here, where the pen is the one cell that
    /// can already be occupied. Checked before the field is built, or a
    /// second subject would read the first's tile as a valid step rather
    /// than a blocked one.
    fn pen_walk_field(
        &mut self,
        pen: (i32, i32),
        blocked: &HashSet<(i32, i32)>,
        start: (i32, i32),
    ) -> Option<HashMap<(i32, i32), u32>> {
        if blocked.contains(&pen) {
            return None;
        }
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        let grid = self.world.resource::<BaseGrid>();
        Some(walk_field(pen, haul_walk_radius(pocket_radius), |p| {
            (grid.walkable(p.0, p.1) && (p == start || !blocked.contains(&p))).then_some(1)
        }))
    }

    /// Pins `program` — a tamed program you own — in `station`'s pen, so it
    /// becomes `ProgramRole::UnderStudy` (`components::UnderStudy`).
    ///
    /// **Every refusal lands before anything is written**, asserted per
    /// refusal — `select_research`'s rule, and for its reason: a single test
    /// over one of several refusals passes against all the ones that never
    /// write anyway.
    pub fn pin_subject(&mut self, program: Entity, station: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        let player = self.player_entity();
        let owner = self
            .world
            .get::<Tamed>(program)
            .ok_or_else(|| "That program isn't compiled under your control.".to_string())?
            .owner;
        if owner != player {
            return Err("You don't control that program.".into());
        }
        if self.world.get::<components::UnderStudy>(program).is_some() {
            return Err("That program is already under study.".into());
        }
        // A partied, wielded or away-on-sortie program comes home first —
        // saying so beats `pin_subject` silently recalling it, which would
        // strand whatever it was doing with no notice.
        if self.program_role(program) != Some(ProgramRole::Staff) {
            return Err(
                "Only a program on the base staff can be pinned for study — bring it home first."
                    .into(),
            );
        }
        let Some(pen) = self.study_pen(station) else {
            return Err("That structure has no pen to study anyone in.".into());
        };
        if !self.world.resource::<BaseGrid>().is_floor(pen.0, pen.1) {
            return Err("The pen has no floor under it.".into());
        }
        // Exempt `program` itself — `place_structure`'s body refusal exempts
        // the program paying for the build for the same reason: a program
        // that happens to already be standing on the pen (no walk needed at
        // all) must not trip this refusal against its own position.
        if self
            .base_bodies()
            .into_iter()
            .any(|(e, p)| e != program && (p.x, p.y) == pen)
        {
            return Err("Something is already standing in the pen.".into());
        }
        let here = self
            .world
            .get::<Position>(program)
            .copied()
            .ok_or_else(|| "That program has nowhere to walk from.".to_string())?;
        if (here.x, here.y) != pen {
            let blocked = self.blocked_tiles();
            let start = (here.x, here.y);
            let reachable = self
                .pen_walk_field(pen, &blocked, start)
                .is_some_and(|field| field.contains_key(&start));
            if !reachable {
                return Err("There's no route to the pen from here.".into());
            }
        }
        // A posted program **is** `Staff` — the role check above never sees
        // its `Task` — so pinning has to free it here or it never is:
        // `base_staff()` already excludes `UnderStudy`, so
        // `schedule_base_labour`'s free loop (which only ever visits
        // `base_staff()`) will never touch this body again, and
        // `drift_idle_staff`'s body loop skips anything still carrying a
        // `Task` before it ever reaches the `UnderStudy` arm — so a stale
        // `Task` also stops the walk to the pen, not only the posting.
        // `.remove::<Carrying>()` alongside it is `schedule_base_labour`'s
        // own free-loop rule for a body that is no longer in the pool
        // (`Downed`'s unconditional free, ahead of the `Carrying` escape):
        // the scheduler re-posts someone else next tick exactly as it does
        // when staff shrinks any other way.
        self.world
            .entity_mut(program)
            .insert(components::UnderStudy { station })
            .remove::<Task>()
            .remove::<Carrying>();
        let name = self.creature_label(program);
        self.log(format!("{name} is pinned in the Research Station's pen."));
        Ok(())
    }

    /// Unpins `program`, returning it to `ProgramRole::Staff`.
    ///
    /// **Every refusal lands before anything is written**, `pin_subject`'s
    /// rule. The third refusal — an active `ResearchDef::requires_subject`
    /// project loses its subject out from under it otherwise — is
    /// `select_research`'s "a project is already active" refusal in shape:
    /// it names the project and says abandoning it is how you change your
    /// mind, so a player is told rather than left to wonder why the key did
    /// nothing. Only refused for **this** program: it is the one
    /// `Game::pinned_subject` would spend, so unpinning a second Station's
    /// subject while an unrelated project runs is not this door's business.
    pub fn unpin_subject(&mut self, program: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self.world.get::<components::UnderStudy>(program).is_none() {
            return Err("That program isn't pinned for study.".into());
        }
        if let Some(active) = self.world.resource::<ActiveResearch>().id.clone() {
            let requires_subject = self
                .world
                .resource::<ResearchDb>()
                .get(&active)
                .is_some_and(|d| d.requires_subject);
            if requires_subject && self.pinned_subject() == Some(program) {
                let name = self
                    .world
                    .resource::<ResearchDb>()
                    .get(&active)
                    .map(|d| d.name.clone())
                    .unwrap_or(active);
                return Err(format!(
                    "The base needs this subject to research {name} — abandon the project \
                     first if you want to change your mind."
                ));
            }
        }
        self.world
            .entity_mut(program)
            .remove::<components::UnderStudy>();
        let name = self.creature_label(program);
        self.log(format!("{name} is unpinned and rejoins the base staff."));
        Ok(())
    }

    /// Whether `(x, y)` is a *non-anchor* footprint cell of a standing
    /// `studies` structure — one of the three walkable floor cells a
    /// Research Station draws with its own fill, never the anchor, which
    /// draws the structure's own glyph. **The renderer's only way to
    /// ask** — `view_finishes_at`'s precedent, so gui holds no copy of the
    /// footprint geometry.
    pub fn view_station_floor_at(&mut self, x: i32, y: i32) -> bool {
        let mut query = self.world.query::<(&Position, &Structure)>();
        let rows: Vec<(Position, StructureId)> = query
            .iter(&self.world)
            .map(|(p, s)| (*p, s.kind.clone()))
            .collect();
        let db = self.world.resource::<StructureDb>();
        rows.into_iter().any(|(p, kind)| {
            let Some(def) = db.get(&kind) else {
                return false;
            };
            if !def.studies {
                return false;
            }
            let anchor = (p.x, p.y);
            anchor != (x, y)
                && crate::tactical::footprint_cells_at(anchor, def.footprint).contains(&(x, y))
        })
    }

    /// Whether a body under study stands at `(x, y)` — the pin mark's own
    /// question. Reads `components::UnderStudy` bodies directly rather than
    /// re-deriving a pen: arrival is derived off a body's own `Position`
    /// (`components::UnderStudy`'s doc), so this is the same read in the
    /// other direction. **The renderer's only way to ask.**
    pub fn view_pinned_at(&mut self, x: i32, y: i32) -> bool {
        let mut query = self.world.query::<(&Position, &components::UnderStudy)>();
        query
            .iter(&self.world)
            .any(|(pos, _)| (pos.x, pos.y) == (x, y))
    }

    /// Both structure-destruction doors call this: a demolished or
    /// destroyed `studies` structure releases its subject back to
    /// `ProgramRole::Staff` and abandons whatever project is active —
    /// `clear_pending_build_at`'s rule with a second subject, since the door
    /// left out silently strands a program in a role nothing can get it out
    /// of, and nothing fails to compile.
    ///
    /// A no-op when `structure` holds no subject — most structures never
    /// will — so calling it beside `clear_pending_build_at` costs every
    /// other destruction path nothing.
    pub(crate) fn release_study_station(&mut self, structure: Entity) {
        let subject = self
            .world
            .query::<(Entity, &components::UnderStudy)>()
            .iter(&self.world)
            .find(|(_, u)| u.station == structure)
            .map(|(e, _)| e);
        let Some(subject) = subject else {
            return;
        };
        self.world
            .entity_mut(subject)
            .remove::<components::UnderStudy>();
        let name = self.creature_label(subject);
        self.log(format!(
            "{name} is released from the pen — the Research Station is gone."
        ));
        let _ = self.abandon_research();
    }
}
