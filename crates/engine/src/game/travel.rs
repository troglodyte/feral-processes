//! `Game::travel_step`: a read-only route query for "walk here" — a click
//! on the map or a chase after a moving hostile (`travel-on-the-clock`).
//! `App::update_realtime` is the one caller in app-core, and it spends what
//! this answers through `move_player` / `move_in_base`, exactly as an
//! arrow key does. This module writes nothing to the `World` and draws no
//! `resources::GameRng` — see the tests for the RNG claim, in the same
//! shape the Predation no-draw test uses.

use crate::game::pursuit::walk_field;
use crate::tuning::TRAVEL_ROUTE_MARGIN;
use crate::world::NEIGHBOURS;
use crate::*;

/// Where a travel is headed.
///
/// `Creature` is re-resolved to its current tile on every `travel_step`
/// call rather than captured once — that is what lets a travel *follow* a
/// moving hostile instead of walking to wherever it stood when the travel
/// was set.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelGoal {
    Tile(i32, i32),
    Creature(Entity),
}

/// One tick's answer to "which way does the walk go next" —
/// `Game::travel_step`'s whole return shape.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TravelStep {
    /// Step `(dx, dy)`; more of the route remains after it.
    Toward(i32, i32),
    /// Step `(dx, dy)`; the goal is Chebyshev-adjacent, so this is the
    /// route's last step and it is an **ordinary bump** — whatever
    /// `move_player` / `move_in_base` does there today (fight, visit,
    /// swing, wall) happens with nothing added on top, and the caller
    /// clears the travel once it has spent this step.
    Last(i32, i32),
    /// Already standing on the goal.
    Arrived,
    /// No route to the goal exists inside the search box.
    NoRoute,
    /// A `Creature` goal that no longer exists, or now stands in the other
    /// coordinate space from the one the travel is being asked about.
    Gone,
}

impl Game {
    /// One step of a route toward `goal`, taken from wherever the party is
    /// standing right now — the surface `Position`, or the base-space cell
    /// from `Game::base_pos`. Underground always answers `NoRoute`: the
    /// player's `Position` stays pinned to the Stack entrance for the whole
    /// dive (`resources::Locale`'s own doc), so there is no surface tile to
    /// route from down there.
    ///
    /// A read, never an act: nothing here writes to the `World`, and no
    /// branch draws `resources::GameRng`, so a caller may ask every tick to
    /// decide *whether* to spend one at no extra cost. `&mut self` only
    /// because `WorldMap::tile` (the surface branch's terrain read)
    /// generates chunks lazily.
    pub fn travel_step(&mut self, goal: TravelGoal) -> TravelStep {
        if self.is_underground() {
            return TravelStep::NoRoute;
        }
        let in_base = self.in_base();
        let Some(current) = self.travel_origin(in_base) else {
            return TravelStep::NoRoute;
        };
        let Some(goal_tile) = self.resolve_travel_goal(goal, in_base) else {
            return TravelStep::Gone;
        };

        let delta = (goal_tile.0 - current.0, goal_tile.1 - current.1);
        let distance = delta.0.abs().max(delta.1.abs());
        if distance == 0 {
            return TravelStep::Arrived;
        }
        // **The last step is always an ordinary bump, and the walk ends
        // with it.** Answered here on raw adjacency, before any route is
        // built, so a hostile standing on the goal — the case a route's own
        // cost function refuses everywhere else — is still reachable as
        // the intended target: `Last` is what lets a travel walk *up to and
        // into* a hostile rather than being unable to approach it at all.
        if distance == 1 {
            return TravelStep::Last(delta.0, delta.1);
        }

        let radius = distance + TRAVEL_ROUTE_MARGIN;
        let field = if in_base {
            // Bodies alone, not `Game::blocked_tiles` — that set also
            // refuses every structure's *anchor*, which `Game::move_in_base`
            // walks over freely; `Game::base_step_blocked` is the same
            // solid-rock/barrier refusal `move_in_base` itself answers with.
            let bodies: std::collections::HashSet<(i32, i32)> = self
                .base_bodies()
                .into_iter()
                .map(|(_, p)| (p.x, p.y))
                .collect();
            walk_field(goal_tile, radius, |cell| {
                // The goal cell is exempt from both refusals below — the
                // field is rooted there, so what it costs to enter never
                // actually decides anything (`origin` is always inserted at
                // cost 0), but a goal that a route's own rule would refuse
                // is a contradiction not worth carrying.
                if cell == goal_tile {
                    return Some(1);
                }
                (!self.base_step_blocked(cell.0, cell.1) && !bodies.contains(&cell)).then_some(1)
            })
        } else {
            // Collected once for the whole search box rather than asked per
            // cell — `Game::bump_tiles`' own reason.
            let obstacles = self.bump_tiles(goal_tile, radius);
            walk_field(goal_tile, radius, |cell| {
                if cell == goal_tile {
                    return Some(1);
                }
                let walkable = self
                    .world
                    .resource_mut::<WorldMap>()
                    .tile(cell.0, cell.1)
                    .walkable;
                (walkable && !obstacles.contains(&cell)).then_some(1)
            })
        };

        // The player's own neighbour with the lowest field value, ties
        // broken by `NEIGHBOURS` order — `Iterator::min_by_key` keeps the
        // first of an equal run, which is exactly that order since the map
        // above walks `NEIGHBOURS` itself.
        NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (current.0 + dx, current.1 + dy))
            .filter_map(|n| field.get(&n).map(|&cost| (n, cost)))
            .min_by_key(|&(_, cost)| cost)
            .map_or(TravelStep::NoRoute, |(n, _)| {
                TravelStep::Toward(n.0 - current.0, n.1 - current.1)
            })
    }

    /// The tile the party is walking from, in whichever space `in_base`
    /// names — `None` only when the surface player entity has somehow lost
    /// its `Position`, which does not happen in play.
    fn travel_origin(&self, in_base: bool) -> Option<(i32, i32)> {
        if in_base {
            self.base_pos()
        } else {
            let player = self.player_entity();
            self.world.get::<Position>(player).map(|p| (p.x, p.y))
        }
    }

    /// Resolves `goal` to a tile in the space `in_base` names, or `None`
    /// for a `Creature` goal `travel_step` must answer `Gone` for.
    fn resolve_travel_goal(&self, goal: TravelGoal, in_base: bool) -> Option<(i32, i32)> {
        match goal {
            TravelGoal::Tile(x, y) => Some((x, y)),
            TravelGoal::Creature(entity) => {
                self.world.get::<Stats>(entity)?;
                if self.stands_in_base_space(entity) != in_base {
                    return None;
                }
                self.world.get::<Position>(entity).map(|p| (p.x, p.y))
            }
        }
    }
}
