//! Bystanders walk clear of a tantrum.
//!
//! Purely for the look of the thing: every other program in the room a
//! fight is in walks out of it, holds its distance until the fight has been
//! over for `TANTRUM_FLEE_LINGER_TICKS`, and then goes back to whatever it
//! was doing. A posted worker keeps its `Task` and its progress throughout;
//! what stops it being dragged back to its post is that every per-tick
//! walker skips a body carrying `components::Fleeing`.
//!
//! **No RNG is drawn, anywhere in here.** The goal is a deterministic
//! function of the board — cost, then cell, then entity — so a brawl
//! cannot shift the stream by having an audience.
//!
//! Shaped as gather -> `decide_flight` (pure) -> apply, the repo's
//! functional-core rule. Bystanders walk with the *rigid* occupancy (a body
//! is a wall): this is a stroll, not a squeeze, and it is what keeps two
//! fleers from ever sharing a cell.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::base_grid::BaseGrid;
use crate::components::{Downed, Fleeing, Position, Task, TaskKind};
use crate::resources::{Brawls, GameClock, Locale};
use crate::rooms;
use crate::tuning::{TANTRUM_FLEE_LINGER_TICKS, TANTRUM_FLEE_TILES};
use crate::world::NEIGHBOURS;

type Cell = (i32, i32);

fn chebyshev(a: Cell, b: Cell) -> i32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs())
}

/// A room a brawl is in, as far as picking a goal needs to know it.
pub(crate) struct RoomShape {
    pub cells: HashSet<Cell>,
    /// The commons has no walls to leave by, so it takes the distance rule.
    pub commons: bool,
}

/// One body that should be walking clear this beat.
pub(crate) struct Bystander {
    /// Stable ordering key (`Entity::to_bits`): the tie-break and the order
    /// goals are claimed in.
    pub key: u64,
    /// Index into `FlightInput::rooms`: the room this body stands in.
    pub room: usize,
    /// The brawl's centre — the victim's cell, so a bystander never walks
    /// toward the fighter being hit.
    pub from: Cell,
    /// The goal it is already walking to, preferred while it is still good.
    pub current: Option<Cell>,
    /// Laid-floor cells this body can walk to, with their walking cost.
    /// Anything held by a body or a structure is already absent.
    pub reach: HashMap<Cell, u32>,
}

pub(crate) struct FlightInput {
    pub rooms: Vec<RoomShape>,
    pub bystanders: Vec<Bystander>,
    /// Cells no goal may be: a party standing there, a structure footprint.
    pub taken: HashSet<Cell>,
}

/// Where each bystander that has somewhere to go is headed, by `key`.
///
/// 1. In a walled room with an exit: the nearest reachable floor outside it.
/// 2. Otherwise the nearest reachable floor at least `TANTRUM_FLEE_TILES`
///    from the brawl.
/// 3. Otherwise the farthest reachable floor from the brawl.
///
/// "Nearest" is walking cost, then cell, so the answer never depends on a
/// hash order. Goals are claimed as they are chosen (in key order), so two
/// bystanders never share one.
pub(crate) fn decide_flight(input: &FlightInput) -> Vec<(u64, Cell)> {
    let mut order: Vec<&Bystander> = input.bystanders.iter().collect();
    order.sort_by_key(|b| b.key);
    let mut claimed = input.taken.clone();
    let mut goals = Vec::new();
    for b in order {
        let room = &input.rooms[b.room];
        let free = |c: &Cell| !claimed.contains(c);
        // Rank 0 is "keep what you have", so a body already walking does not
        // change its mind because somebody else moved.
        let near = |c: &Cell, cost: u32| (Some(*c) != b.current, cost, c.0, c.1);
        let outside = (!room.commons)
            .then(|| {
                b.reach
                    .iter()
                    .filter(|(c, _)| free(c) && !room.cells.contains(c))
                    .map(|(c, &cost)| (near(c, cost), *c))
                    .min()
            })
            .flatten();
        let pick = outside
            .or_else(|| {
                b.reach
                    .iter()
                    .filter(|(c, _)| free(c) && chebyshev(**c, b.from) >= TANTRUM_FLEE_TILES)
                    .map(|(c, &cost)| (near(c, cost), *c))
                    .min()
            })
            .or_else(|| {
                b.reach
                    .iter()
                    .filter(|(c, _)| free(c))
                    .map(|(c, &cost)| {
                        let (kept, cost, x, y) = near(c, cost);
                        (
                            (std::cmp::Reverse(chebyshev(*c, b.from)), kept, cost, x, y),
                            *c,
                        )
                    })
                    .min()
                    .map(|(k, c)| ((k.1, k.2, k.3, k.4), c))
            })
            .map(|(_, c)| c);
        if let Some(goal) = pick {
            claimed.insert(goal);
            goals.push((b.key, goal));
        }
    }
    goals
}

impl Game {
    /// One beat of bystanders: put the marker on everyone beside an open
    /// brawl, take one step toward each goal, and let it lapse
    /// `TANTRUM_FLEE_LINGER_TICKS` after the room goes quiet.
    pub(crate) fn run_flight(&mut self, staff: &[Entity]) {
        if !self.base_is_established() {
            return;
        }
        let open = self.world.resource::<Brawls>().open.clone();
        let mut marked: Vec<(Entity, Fleeing)> = {
            let mut q = self.world.query::<(Entity, &Fleeing)>();
            q.iter(&self.world).map(|(e, f)| (e, *f)).collect()
        };
        if open.is_empty() && marked.is_empty() {
            return;
        }
        marked.sort_by_key(|(e, _)| *e);
        let now = self.world.resource::<GameClock>().tick;

        let fighters: HashSet<Entity> = open.iter().flat_map(|b| [b.aggressor, b.victim]).collect();
        let ineligible = |game: &Game, e: Entity| {
            fighters.contains(&e)
                || game.world.get::<Downed>(e).is_some()
                || game
                    .world
                    .get::<Task>(e)
                    .is_some_and(|t| t.kind == TaskKind::Guard)
        };

        // Whoever is marked but no longer base staff, or has become a
        // fighter, a patient or a guard, simply stops: nothing here may walk
        // a body that should be lying still.
        for &(e, _) in &marked {
            if !staff.contains(&e) || ineligible(self, e) {
                self.world.entity_mut(e).remove::<Fleeing>();
            }
        }

        let rooms_now = rooms::of_world(&self.world);
        let home = self.home_position().map(|p| (p.x, p.y));
        let shapes: Vec<RoomShape> = rooms_now
            .rooms
            .iter()
            .map(|r| RoomShape {
                cells: r.cells.iter().copied().collect(),
                commons: home.is_some_and(|h| r.cells.contains(&h)),
            })
            .collect();
        let room_index = |at: Cell| {
            rooms_now
                .room_at(at.0, at.1)
                .and_then(|room| rooms_now.rooms.iter().position(|r| std::ptr::eq(r, room)))
        };

        let mut taken = self.structure_tiles();
        taken.extend(self.build_site_tiles());
        if let Locale::Base { x, y } = *self.world.resource::<Locale>() {
            taken.insert((x, y));
        }
        let blocked = self.blocked_tiles();
        let rigid = blocked.clone().rigid();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();

        let centres: Vec<(Cell, Vec<Cell>)> = open
            .iter()
            .filter_map(|b| {
                let v = self.world.get::<Position>(b.victim)?;
                let a = self.world.get::<Position>(b.aggressor)?;
                Some(((v.x, v.y), vec![(v.x, v.y), (a.x, a.y)]))
            })
            .collect();

        let mut bystanders = Vec::new();
        for &e in staff {
            if ineligible(self, e) {
                continue;
            }
            let Some(at) = self.world.get::<Position>(e).copied() else {
                continue;
            };
            let at = (at.x, at.y);
            let Some(room) = room_index(at) else { continue };
            let Some((from, _)) = centres
                .iter()
                .find(|(_, fighters_at)| fighters_at.iter().any(|&f| rooms_now.same_room(at, f)))
            else {
                continue;
            };
            let grid = self.world.resource::<BaseGrid>();
            let reach = crate::game::base::hauling::crew_reach(
                grid,
                Position { x: at.0, y: at.1 },
                &rigid,
                pocket_radius,
            )
            .into_iter()
            .filter(|(c, _)| grid.is_floor(c.0, c.1) && !taken.contains(c))
            .collect();
            bystanders.push(Bystander {
                key: e.to_bits(),
                room,
                from: *from,
                current: self.world.get::<Fleeing>(e).map(|f| f.goal),
                reach,
            });
        }

        // The rest of the roster holds its cell: a body that is not fleeing
        // is not a goal.
        let held: HashSet<Cell> = self
            .base_bodies()
            .into_iter()
            .map(|(_, p)| (p.x, p.y))
            .collect();
        let mut input = FlightInput {
            rooms: shapes,
            bystanders,
            taken,
        };
        let fleeing_now: HashSet<u64> = input.bystanders.iter().map(|b| b.key).collect();
        for e in staff {
            if fleeing_now.contains(&e.to_bits()) {
                continue;
            }
            if let Some(p) = self.world.get::<Position>(*e) {
                input.taken.insert((p.x, p.y));
            }
        }
        input.taken.extend(held.iter().copied().filter(|c| {
            // A fleer's own start cell is held by it; it is its to keep.
            !input.bystanders.iter().any(|b| b.reach.get(c) == Some(&0))
        }));

        let goals = decide_flight(&input);
        let goal_of: HashMap<u64, Cell> = goals.iter().copied().collect();

        // Apply: markers first, then one step each, in key order.
        let mut walking: Vec<(Entity, Cell)> = Vec::new();
        for &e in staff {
            if ineligible(self, e) {
                continue;
            }
            let held_marker = marked.iter().find(|(m, _)| *m == e).map(|(_, f)| *f);
            if let Some(&goal) = goal_of.get(&e.to_bits()) {
                self.world
                    .entity_mut(e)
                    .insert(Fleeing { goal, until: None });
                walking.push((e, goal));
            } else if let Some(f) = held_marker {
                // Quiet room: linger, then lapse.
                match f.until {
                    None => {
                        let until = now + TANTRUM_FLEE_LINGER_TICKS;
                        self.world.entity_mut(e).insert(Fleeing {
                            until: Some(until),
                            ..f
                        });
                        walking.push((e, f.goal));
                    }
                    Some(until) if until > now => walking.push((e, f.goal)),
                    Some(_) => {
                        self.world.entity_mut(e).remove::<Fleeing>();
                    }
                }
            }
        }
        walking.sort_by_key(|(e, _)| *e);
        let mut stepped_into: HashSet<Cell> = HashSet::new();
        for (e, goal) in walking {
            self.step_toward_cell(e, goal, &blocked, pocket_radius, &mut stepped_into);
        }
    }

    /// One rigid step toward `goal` itself — not beside it, which is what
    /// `step_to_post` and `walk_idle_toward` would do (the field prices a held
    /// cell at the squeeze, so the route bends round bodies, and the step
    /// itself never lands on one): a bystander has to
    /// reach the cell it was sent to, or the distance rule is a lie by one.
    fn step_toward_cell(
        &mut self,
        who: Entity,
        goal: Cell,
        blocked: &crate::game::base::hauling::Occupancy,
        pocket_radius: i32,
        stepped_into: &mut HashSet<Cell>,
    ) {
        let Some(here) = self.world.get::<Position>(who).copied() else {
            return;
        };
        if (here.x, here.y) == goal {
            return;
        }
        let party = match *self.world.resource::<Locale>() {
            Locale::Base { x, y } => Some((x, y)),
            _ => None,
        };
        let field = crate::game::base::hauling::crew_reach(
            self.world.resource::<BaseGrid>(),
            Position {
                x: goal.0,
                y: goal.1,
            },
            blocked,
            pocket_radius,
        );
        let Some(&cost_here) = field.get(&(here.x, here.y)) else {
            return;
        };
        let next = NEIGHBOURS
            .iter()
            .map(|(dx, dy)| (here.x + dx, here.y + dy))
            .filter(|n| !stepped_into.contains(n) && Some(*n) != party && !blocked.holds_a_body(*n))
            .filter_map(|n| field.get(&n).map(|&c| (c, n)))
            .filter(|&(c, _)| c < cost_here)
            .min();
        if let Some((_, to)) = next {
            stepped_into.insert(to);
            if let Some(mut pos) = self.world.get_mut::<Position>(who) {
                *pos = Position { x: to.0, y: to.1 };
            }
            // A step taken by this walk is never a squeeze.
            self.world
                .entity_mut(who)
                .remove::<crate::components::Squeezing>();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn room(cells: &[Cell], commons: bool) -> RoomShape {
        RoomShape {
            cells: cells.iter().copied().collect(),
            commons,
        }
    }

    fn floor(a: Cell, b: Cell) -> HashMap<Cell, u32> {
        let mut m = HashMap::new();
        for x in a.0..=b.0 {
            for y in a.1..=b.1 {
                m.insert((x, y), (x - a.0).unsigned_abs() + (y - a.1).unsigned_abs());
            }
        }
        m
    }

    fn who(key: u64, room: usize, from: Cell, reach: HashMap<Cell, u32>) -> Bystander {
        Bystander {
            key,
            room,
            from,
            current: None,
            reach,
        }
    }

    #[test]
    fn a_walled_room_sends_its_bystander_outside_it() {
        let inside: Vec<Cell> = (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).collect();
        let input = FlightInput {
            rooms: vec![room(&inside, false)],
            bystanders: vec![who(1, 0, (1, 1), floor((0, 0), (8, 2)))],
            taken: HashSet::new(),
        };
        let goals = decide_flight(&input);
        assert_eq!(goals, vec![(1, (3, 0))]);
    }

    #[test]
    fn the_commons_sends_its_bystander_the_flee_distance_away() {
        let all: Vec<Cell> = (0..12).flat_map(|x| (0..3).map(move |y| (x, y))).collect();
        let input = FlightInput {
            rooms: vec![room(&all, true)],
            bystanders: vec![who(1, 0, (1, 1), floor((0, 0), (11, 2)))],
            taken: HashSet::new(),
        };
        let goals = decide_flight(&input);
        let (_, goal) = goals[0];
        assert!(chebyshev(goal, (1, 1)) >= TANTRUM_FLEE_TILES, "{goal:?}");
        // The nearest such cell, not just any.
        assert_eq!(goal, (6, 0));
    }

    #[test]
    fn a_room_with_no_exit_and_no_far_cell_sends_it_to_the_farthest() {
        let cells: Vec<Cell> = (0..4).map(|x| (x, 0)).collect();
        let input = FlightInput {
            rooms: vec![room(&cells, false)],
            bystanders: vec![who(1, 0, (0, 0), floor((0, 0), (3, 0)))],
            taken: HashSet::new(),
        };
        assert_eq!(decide_flight(&input), vec![(1, (3, 0))]);
    }

    #[test]
    fn two_bystanders_never_share_a_goal() {
        let inside: Vec<Cell> = (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).collect();
        let reach = floor((0, 0), (8, 2));
        let input = FlightInput {
            rooms: vec![room(&inside, false)],
            bystanders: vec![who(2, 0, (1, 1), reach.clone()), who(1, 0, (1, 1), reach)],
            taken: HashSet::new(),
        };
        let goals = decide_flight(&input);
        assert_eq!(goals.len(), 2);
        assert_ne!(goals[0].1, goals[1].1);
        assert_eq!(goals[0].0, 1, "claimed in key order");
    }

    #[test]
    fn a_taken_cell_is_never_a_goal() {
        let inside: Vec<Cell> = (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).collect();
        let input = FlightInput {
            rooms: vec![room(&inside, false)],
            bystanders: vec![who(1, 0, (1, 1), floor((0, 0), (8, 2)))],
            taken: [(3, 0)].into_iter().collect(),
        };
        assert_eq!(decide_flight(&input), vec![(1, (3, 1))]);
    }

    #[test]
    fn a_body_keeps_a_goal_that_is_still_good() {
        let inside: Vec<Cell> = (0..3).flat_map(|x| (0..3).map(move |y| (x, y))).collect();
        let mut b = who(1, 0, (1, 1), floor((0, 0), (8, 2)));
        b.current = Some((6, 2));
        let input = FlightInput {
            rooms: vec![room(&inside, false)],
            bystanders: vec![b],
            taken: HashSet::new(),
        };
        assert_eq!(decide_flight(&input), vec![(1, (6, 2))]);
    }
}
