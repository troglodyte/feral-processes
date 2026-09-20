//! Being under study — the Research Station's pen.
//!
//! The home for `Game::study_pen` because the pin and unpin doors that join
//! it here in a later task read and write the same corner, and
//! `building.rs` is already past 1,200 lines owning a different
//! responsibility: placement and demolition, not what a structure's
//! footprint is *for*.

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
        // A single pen can hold one body. `walk_field` roots its search at
        // `pen` and always marks its own root reachable at cost 0 — right
        // for `post_field`'s station faces, which are pre-filtered to ones
        // nobody occupies, and wrong here, where the pen itself is the one
        // cell that can already be spoken for. Checked before the field is
        // ever built, or the second subject to arrive this tick would read
        // the first's tile as a valid step rather than a blocked one.
        if blocked.contains(&pen) {
            return Err(NoPost::NoRoute);
        }
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        let grid = self.world.resource::<BaseGrid>();
        let start = (here.x, here.y);
        let field = walk_field(pen, haul_walk_radius(pocket_radius), |p| {
            (grid.walkable(p.0, p.1) && (p == start || !blocked.contains(&p))).then_some(1)
        });
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
}
