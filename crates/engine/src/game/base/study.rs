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
        if self
            .base_bodies()
            .into_iter()
            .any(|(_, p)| (p.x, p.y) == pen)
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
        self.world
            .entity_mut(program)
            .insert(components::UnderStudy { station });
        let name = self.creature_label(program);
        self.log(format!("{name} is pinned in the Research Station's pen."));
        Ok(())
    }

    /// Unpins `program`, returning it to `ProgramRole::Staff`.
    ///
    /// **Every refusal lands before anything is written**, `pin_subject`'s
    /// rule. Task 7 (Part C) adds the refusal this door is really for — an
    /// active `ResearchDef::requires_subject` project loses its subject out
    /// from under it otherwise — but that field does not exist on this
    /// branch yet, so there is nothing to check against until it lands; this
    /// is not an oversight to "fix" without it.
    pub fn unpin_subject(&mut self, program: Entity) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if self.world.get::<components::UnderStudy>(program).is_none() {
            return Err("That program isn't pinned for study.".into());
        }
        self.world
            .entity_mut(program)
            .remove::<components::UnderStudy>();
        let name = self.creature_label(program);
        self.log(format!("{name} is unpinned and rejoins the base staff."));
        Ok(())
    }
}
