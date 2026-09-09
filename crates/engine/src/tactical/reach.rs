//! How far a body can get this turn, and what it costs it to get there.
//!
//! The fifth caller of `game::pursuit::walk_field`, after the two on the
//! zone surface and the two in base space. It is a caller and not a second
//! walk on purpose: the search is the same search, and the one thing this
//! space disagrees with the others about — `Rough` ground costing two — is
//! exactly what the step rule is a cost function for.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::components::Creature;
use crate::game::pursuit::walk_field;
use crate::species::SpeciesDb;
use crate::tactical::TacticalBattle;
use crate::tuning::{
    DEFAULT_BASE_SPEED, TACTICAL_MOVE_BASE, TACTICAL_MOVE_MAX, TACTICAL_MOVE_MIN,
    TACTICAL_MOVE_SPEED_STEP,
};

/// What a body of `speed` may spend on movement in one tactical turn, or
/// `authored` where its species names a figure of its own.
///
/// **One derivation, and the clamp is the type's rather than the caller's**
/// — `movement_field` passes this straight to `walk_field` as its search
/// radius, so both bounds are correctness bounds and an authored figure is
/// held to them exactly as a derived one is.
///
/// `div_euclid` and not `/`: the band either side of `DEFAULT_BASE_SPEED`
/// has to be the same width, and truncating division rounds toward zero, so
/// plain `/` would make the band straddling the default twice as wide as
/// every other one and a body one point *below* average would move like an
/// average one.
pub fn allowance(speed: i32, authored: Option<u32>) -> u32 {
    let asked = match authored {
        Some(n) => i64::from(n),
        None => {
            i64::from(TACTICAL_MOVE_BASE)
                + i64::from(speed - DEFAULT_BASE_SPEED)
                    .div_euclid(i64::from(TACTICAL_MOVE_SPEED_STEP))
        }
    };
    asked.clamp(i64::from(TACTICAL_MOVE_MIN), i64::from(TACTICAL_MOVE_MAX)) as u32
}

impl Game {
    /// `allowance` asked of an entity: its combat speed, and whatever its
    /// species authored.
    ///
    /// `combat_speed` and not `species_base_speed`, so the player — who has
    /// no `Creature` and so no species — moves off `PLAYER_BASE_SPEED` the
    /// same way they roll initiative and hit and dodge off it. A body with
    /// no species authors nothing and always derives.
    pub fn movement_allowance(&self, entity: Entity) -> u32 {
        let authored = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .and_then(|def| def.movement);
        allowance(self.combat_speed(entity), authored)
    }
}

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

    /// A slow body, an average one and a fast one must actually differ, and
    /// the order must never invert.
    #[test]
    fn the_shipped_roster_spans_the_allowance_band() {
        let band: Vec<u32> = (6..=14).map(|speed| allowance(speed, None)).collect();
        assert!(
            band.windows(2).all(|w| w[0] <= w[1]),
            "a faster body must never move less far: {band:?}"
        );
        assert_eq!(
            (*band.first().unwrap(), *band.last().unwrap()),
            (2, 6),
            "the shipped roster's 6..14 spread must span two through six"
        );
    }

    /// The base is read against the deployment gap, so a change to either
    /// that silently turns closing into a march fails here.
    #[test]
    fn an_average_body_closes_the_deployment_gap_in_two_turns() {
        let average = allowance(DEFAULT_BASE_SPEED, None) as i32;
        assert!(
            average < crate::tuning::TACTICAL_DEPLOY_GAP,
            "closing in one turn leaves no room to position"
        );
        assert!(
            average * 2 >= crate::tuning::TACTICAL_DEPLOY_GAP,
            "closing must not take three turns"
        );
    }

    #[test]
    fn the_allowance_is_clamped_at_both_ends() {
        assert_eq!(allowance(-100, None), TACTICAL_MOVE_MIN);
        assert_eq!(allowance(1000, None), TACTICAL_MOVE_MAX);
        assert_eq!(allowance(DEFAULT_BASE_SPEED, Some(0)), TACTICAL_MOVE_MIN);
        assert_eq!(allowance(DEFAULT_BASE_SPEED, Some(999)), TACTICAL_MOVE_MAX);
    }

    #[test]
    fn an_authored_figure_overrides_the_speed_derivation() {
        let derived = allowance(DEFAULT_BASE_SPEED, None);
        let authored = TACTICAL_MOVE_MAX - 1;
        assert_ne!(
            derived, authored,
            "the fixture must be able to tell them apart"
        );
        assert_eq!(allowance(DEFAULT_BASE_SPEED, Some(authored)), authored);
    }

    /// The bands either side of the default are the same width. Truncating
    /// division would widen the one straddling it.
    #[test]
    fn the_bands_either_side_of_average_are_the_same_width() {
        let step = TACTICAL_MOVE_SPEED_STEP;
        let below: Vec<u32> = (DEFAULT_BASE_SPEED - step..DEFAULT_BASE_SPEED)
            .map(|s| allowance(s, None))
            .collect();
        let at: Vec<u32> = (DEFAULT_BASE_SPEED..DEFAULT_BASE_SPEED + step)
            .map(|s| allowance(s, None))
            .collect();
        assert!(below.iter().all(|&a| a == below[0]));
        assert!(at.iter().all(|&a| a == at[0]));
        assert_eq!(below[0] + 1, at[0], "the band below must be one cell short");
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
