//! How far a body can get this turn, and what it costs it to get there.
//!
//! The fifth caller of `game::pursuit::walk_field`, after the two on the
//! zone surface and the two in base space. It is a caller and not a second
//! walk on purpose: the search is the same search, and the one thing this
//! space disagrees with the others about — `Rough` ground costing two — is
//! exactly what the step rule is a cost function for.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Entity;

use crate::game::pursuit::walk_field;
use crate::tactical::TacticalBattle;

/// Every cell `body` could walk to this turn, and what reaching each one
/// costs it. The cell it is standing on is present at zero, because holding
/// still is a legal move.
///
/// **A body is a wall.** An occupied cell is neither crossed nor stopped on,
/// friend or foe — one rule rather than a pass-through set and a
/// destination set, and the same refusal `TacticalBattle::move_to` already
/// makes. It is also the whole of this model's zone of control: bodies that
/// can be walked through cannot hold a line.
///
/// A body that is not on the board reaches nothing.
pub fn movement_field(
    battle: &TacticalBattle,
    body: Entity,
    allowance: u32,
) -> HashMap<(i32, i32), u32> {
    let Some(origin) = battle.cell_of(body) else {
        return HashMap::new();
    };
    // Gathered once rather than scanned per successor: the walk asks about
    // every neighbour of every cell it reaches, and `occupant` is a linear
    // scan over the fight's whole roster.
    let occupied: HashSet<(i32, i32)> = battle.bodies().map(|(_, cell)| cell).collect();
    let board = &battle.board;

    // The radius is the budget. `walk_field` bounds a Chebyshev box and not
    // a cost, but no step costs less than one, so nothing outside a box of
    // half-width `allowance` can be inside a budget of `allowance` — the
    // box cannot cut off a cell the filter below would have kept.
    let radius = i32::try_from(allowance).unwrap_or(i32::MAX);
    let mut field = walk_field(origin, radius, |cell| {
        if occupied.contains(&cell) {
            return None;
        }
        board.cell(cell.0, cell.1).movement_cost()
    });
    field.retain(|_, cost| *cost <= allowance);
    field
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactical::map::{BattleSpec, Board};
    use crate::world::Biome;
    use bevy_ecs::world::World;

    fn spec() -> BattleSpec {
        BattleSpec {
            world_seed: 9,
            site: (3, 4),
            tick: 77,
            zone: 2,
            biome: Biome::OpenGrid,
            bodies: 2,
        }
    }

    /// A fight on a hand-written board, plus three spare bodies.
    fn fight(rows: &[&str]) -> (TacticalBattle, Vec<Entity>) {
        let mut world = World::new();
        let bodies = (0..3).map(|_| world.spawn_empty().id()).collect();
        (TacticalBattle::open(spec(), Board::from_rows(rows)), bodies)
    }

    #[test]
    fn holding_still_is_a_move_a_body_can_make() {
        let (mut battle, bodies) = fight(&["...", "...", "..."]);
        battle.place(bodies[0], (1, 1));
        let field = movement_field(&battle, bodies[0], 2);
        assert_eq!(field.get(&(1, 1)), Some(&0));
    }

    #[test]
    fn a_body_that_is_not_on_the_board_reaches_nothing() {
        let (battle, bodies) = fight(&["...", "...", "..."]);
        assert!(movement_field(&battle, bodies[0], 4).is_empty());
    }

    /// `Rough` is priced, not merely permitted — the whole reason
    /// `walk_field`'s step rule widened from a predicate.
    #[test]
    fn rough_ground_costs_two_and_open_ground_costs_one() {
        let (mut battle, bodies) = fight(&[".~~", "~~~", "~~~"]);
        battle.place(bodies[0], (0, 0));
        let rough = movement_field(&battle, bodies[0], 4);
        assert_eq!(rough.get(&(1, 1)), Some(&2), "one rough step costs two");
        assert_eq!(rough.get(&(2, 2)), Some(&4), "two rough steps cost four");

        let (mut open, open_bodies) = fight(&["...", "...", "..."]);
        open.place(open_bodies[0], (0, 0));
        let open = movement_field(&open, open_bodies[0], 4);
        assert_eq!(
            open.get(&(2, 2)),
            Some(&2),
            "the same two steps over open ground cost two"
        );
    }

    /// The budget is spent in cost, not in steps: a body with four points
    /// crosses four open cells or two rough ones.
    #[test]
    fn the_allowance_is_a_budget_and_not_a_step_count() {
        let (mut battle, bodies) = fight(&["~~~~~", "~~~~~", "~~~~~", "~~~~~", "~~~~~"]);
        battle.place(bodies[0], (0, 0));
        let field = movement_field(&battle, bodies[0], 4);
        assert!(
            field.contains_key(&(2, 0)),
            "two rough steps are affordable"
        );
        assert!(
            !field.contains_key(&(3, 0)),
            "a third rough step is not, at four points"
        );
        assert!(
            field.values().all(|&cost| cost <= 4),
            "nothing over budget may appear in the field"
        );
    }

    /// `Cover` and `Blocked` differ about sight and agree about crossing.
    #[test]
    fn neither_cover_nor_blocked_is_crossed() {
        let (mut battle, bodies) = fight(&["...", "#X#", "..."]);
        battle.place(bodies[0], (1, 0));
        let field = movement_field(&battle, bodies[0], 6);
        for wall in [(0, 1), (1, 1), (2, 1)] {
            assert!(!field.contains_key(&wall), "{wall:?} was stepped on");
        }
        for beyond in [(0, 2), (1, 2), (2, 2)] {
            assert!(!field.contains_key(&beyond), "{beyond:?} was reached");
        }
    }

    /// A body is a wall: not stopped on, and not walked through either.
    #[test]
    fn a_body_blocks_the_cell_it_stands_on_and_the_way_past_it() {
        let rows = ["XXXXX", ".....", "XXXXX", "XXXXX", "XXXXX"];
        let (mut open, open_bodies) = fight(&rows);
        open.place(open_bodies[0], (0, 1));
        assert!(
            movement_field(&open, open_bodies[0], 4).contains_key(&(3, 1)),
            "the corridor is walkable with nobody in it"
        );

        let (mut battle, bodies) = fight(&rows);
        battle.place(bodies[0], (0, 1));
        battle.place(bodies[1], (2, 1));
        let field = movement_field(&battle, bodies[0], 4);
        assert!(
            !field.contains_key(&(2, 1)),
            "an occupied cell must not be stopped on"
        );
        assert!(
            !field.contains_key(&(3, 1)),
            "an occupied cell must not be walked through"
        );
    }

    /// Off the board is off the field. Walking off the edge is a departure
    /// from the fight, which belongs to the turn model and not to a step.
    #[test]
    fn the_field_stops_at_the_board_edge() {
        let (mut battle, bodies) = fight(&["...", "...", "..."]);
        battle.place(bodies[0], (0, 0));
        let field = movement_field(&battle, bodies[0], 4);
        assert!(
            field.keys().all(|&(x, y)| battle.board.in_bounds(x, y)),
            "the field left the board"
        );
    }
}
