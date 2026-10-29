//! `Game::travel_step`: a read-only route query for "walk here" — a click
//! on the map or a chase after a moving hostile (`travel-on-the-clock`).
//! `App::update_realtime` is the one caller in app-core, and it spends what
//! this answers through `move_player`, exactly as an
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
    /// `move_player` does there today (fight, visit,
    /// swing, wall) happens with nothing added on top, and the caller
    /// clears the travel once it has spent this step.
    Last(i32, i32),
    /// Already standing on the goal.
    Arrived,
    /// No route to the goal exists inside the search box.
    NoRoute,
    /// A `Creature` goal that no longer exists, or has walked out of sight.
    Gone,
}

impl Game {
    /// One step of a route toward `goal`, taken from wherever the party is
    /// standing right now. Underground always answers `NoRoute`: the
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
        let Some(current) = self.field_pos() else {
            return TravelStep::NoRoute;
        };
        let Some(goal_tile) = self.resolve_travel_goal(goal) else {
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
        // Collected once for the whole search box rather than asked per
        // cell — `Game::bump_tiles`' own reason. The base's bodies join them:
        // a step is never planned through a program standing in the way.
        let mut obstacles = self.bump_tiles(goal_tile, radius);
        obstacles.extend(self.base_bodies().into_iter().map(|(_, p)| (p.x, p.y)));
        let field = walk_field(goal_tile, radius, |cell| {
            // The goal cell is exempt: the field is rooted there, so what it
            // costs to enter never decides anything, but a goal a route's own
            // rule would refuse is a contradiction not worth carrying.
            if cell == goal_tile {
                return Some(1);
            }
            // `base_step_blocked` is `move_player`'s own refusal (ground
            // nothing stands on, or a barrier); a machine is walked over.
            (!self.base_step_blocked(cell.0, cell.1) && !obstacles.contains(&cell)).then_some(1)
        });

        // The player's own neighbour with the lowest field value. Chebyshev
        // distance prices a diagonal like a straight step, so most moves
        // tie three ways; the tie goes to the step pointing most directly
        // at the goal, or the walk bows a row off the line and back.
        // Exact ties (two diagonals either side of a straight goal) keep
        // `NEIGHBOURS` order — `min_by` keeps the first of an equal run.
        NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (current.0 + dx, current.1 + dy))
            .filter_map(|n| field.get(&n).map(|&cost| (n, cost)))
            .min_by(|&(a, cost_a), &(b, cost_b)| {
                cost_a.cmp(&cost_b).then_with(|| {
                    let step = |n: (i32, i32)| (n.0 - current.0, n.1 - current.1);
                    heading_fit(step(b), delta).total_cmp(&heading_fit(step(a), delta))
                })
            })
            .map_or(TravelStep::NoRoute, |(n, _)| {
                TravelStep::Toward(n.0 - current.0, n.1 - current.1)
            })
    }

    /// Resolves `goal` to a tile, or `None` for a `Creature` goal
    /// `travel_step` must answer `Gone` for.
    fn resolve_travel_goal(&self, goal: TravelGoal) -> Option<(i32, i32)> {
        match goal {
            TravelGoal::Tile(x, y) => Some((x, y)),
            TravelGoal::Creature(entity) => {
                self.world.get::<Stats>(entity)?;
                // A hostile that walked out of sight is gone to the player,
                // or steering toward its live tile would give it away.
                if !self.is_shown(entity, false) {
                    return None;
                }
                self.world.get::<Position>(entity).map(|p| (p.x, p.y))
            }
        }
    }
}

/// How closely `step` points along `toward`, larger is closer: the cosine
/// of the angle between them, scaled by `toward`'s length, which every
/// step compared in one call shares.
fn heading_fit(step: (i32, i32), toward: (i32, i32)) -> f64 {
    let dot = f64::from(step.0 * toward.0 + step.1 * toward.1);
    dot / f64::from(step.0 * step.0 + step.1 * step.1).sqrt()
}
