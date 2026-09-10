//! How far a body can get this turn, what it costs it to get there, and
//! what its routines cover once it stops.
//!
//! The fifth caller of `game::pursuit::walk_field`, after the two on the
//! zone surface and the two in base space. It is a caller and not a second
//! walk on purpose: the search is the same search, and the one thing this
//! space disagrees with the others about — `Rough` ground costing two — is
//! exactly what the step rule is a cost function for.

use std::collections::{HashMap, HashSet};

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::abilities::{AbilityRange, AbilityShape};
use crate::components::Creature;
use crate::game::pursuit::walk_field;
use crate::species::SpeciesDb;
use crate::tactical::map::Board;
use crate::tactical::{TacticalBattle, deploy};
use crate::tuning::{
    DEFAULT_BASE_SPEED, TACTICAL_MOVE_BASE, TACTICAL_MOVE_MAX, TACTICAL_MOVE_MIN,
    TACTICAL_MOVE_SPEED_STEP,
};
use crate::world::NEIGHBOURS;

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

/// The cells a body at `from` walks through to reach `to`, in the order it
/// enters them — `to` last, and the cell it is standing on left out, so the
/// length is the number of steps the walk costs it in turns of the pacing
/// loop.
///
/// **Descended from the cost field rather than searched for again.**
/// `walk_field` prices a step by the cell being *entered*, so a cell's
/// predecessor is a neighbour whose cost is exactly this cell's less what
/// entering this cell cost — one arithmetic identity, and no second walk to
/// come adrift of the first about which cells are legal.
///
/// **Tie-broken in the board's reading order**, (y, x), for
/// `walk_to_best_cell`'s reason: `movement_field` answers a `HashMap` and
/// iteration order over one is not stable between runs, so two equally short
/// approaches must not be walked differently in a seeded fight.
///
/// Empty where `to` is the cell already stood on, and empty where the field
/// never reached it — a caller with no path takes no steps and acts from
/// where it stands, which is the same answer either way.
pub fn path_to(
    board: &Board,
    field: &HashMap<(i32, i32), u32>,
    from: (i32, i32),
    to: (i32, i32),
) -> Vec<(i32, i32)> {
    let mut path = Vec::new();
    let mut cell = to;
    while cell != from {
        let Some(prev) = field
            .get(&cell)
            .zip(board.cell(cell.0, cell.1).movement_cost())
            .and_then(|(&cost, entering)| cost.checked_sub(entering))
            .and_then(|before| {
                let mut back: Vec<(i32, i32)> = NEIGHBOURS
                    .iter()
                    .map(|(dx, dy)| (cell.0 + dx, cell.1 + dy))
                    .filter(|n| field.get(n) == Some(&before))
                    .collect();
                back.sort_by_key(|&(x, y)| (y, x));
                back.first().copied()
            })
        else {
            return Vec::new();
        };
        path.push(cell);
        cell = prev;
    }
    path.reverse();
    path
}

/// How far apart two cells are, in steps.
///
/// Chebyshev, because movement is eight-way and every step costs at least
/// one: the diagonal that carries a body one cell nearer on both axes has to
/// read as one cell nearer, or a routine's range would disagree with the
/// walk that closed it.
pub fn distance(a: (i32, i32), b: (i32, i32)) -> u32 {
    (a.0 - b.0).abs().max((a.1 - b.1).abs()) as u32
}

/// Whether `aim` is a cell `from` may aim a routine of this `range` at.
pub fn in_range(from: (i32, i32), aim: (i32, i32), range: AbilityRange) -> bool {
    let d = distance(from, aim);
    d >= range.min && d <= range.max
}

/// Whether anything standing at `from` can see `to`.
///
/// A straight sample of the cells between them — the endpoints excluded, so
/// standing *in* cover neither blinds a body nor protects it, which is the
/// same asymmetry the Stack settled when it recorded that `walkable()` and
/// `blocks_sight()` are not complements. Sampled at `distance` steps rather
/// than walked, so the line a `Cone` checks and the line a `Line` draws
/// cannot disagree about which cells lie between two others.
pub fn line_of_sight(board: &Board, from: (i32, i32), to: (i32, i32)) -> bool {
    let steps = distance(from, to);
    for step in 1..steps {
        let t = f64::from(step) / f64::from(steps);
        let x = from.0 + ((to.0 - from.0) as f64 * t).round() as i32;
        let y = from.1 + ((to.1 - from.1) as f64 * t).round() as i32;
        if board.blocks_sight(x, y) {
            return false;
        }
    }
    true
}

/// Every cell a routine of this `shape`, run from `from` and aimed at `aim`,
/// covers.
///
/// **The invoker's own cell is covered by `Radius` alone.** A blast centred
/// on where you stand is the one shape that can catch you, which is what
/// makes `WholeParty`'s derived shape land on the invoker at all; a `Line`
/// and a `Cone` are cast away from the body casting them and start one cell
/// out.
///
/// Terrain is read by two of the four. `Line` stops at the first cell that
/// blocks sight and `Cone` drops any cell it cannot see, both through
/// `Board::blocks_sight` — a `Cover` cell, and nothing else. `Radius` is
/// stopped by nothing, because a blast that had to see its own far side
/// would need a second sight rule per cell in it, and `Single` names one
/// cell that was already in range.
pub fn shape_cells(
    board: &Board,
    from: (i32, i32),
    aim: (i32, i32),
    shape: AbilityShape,
) -> Vec<(i32, i32)> {
    match shape {
        AbilityShape::Single => vec![aim],
        AbilityShape::Radius { radius } => {
            let r = radius as i32;
            let mut cells = Vec::new();
            for y in (aim.1 - r)..=(aim.1 + r) {
                for x in (aim.0 - r)..=(aim.0 + r) {
                    if board.in_bounds(x, y) {
                        cells.push((x, y));
                    }
                }
            }
            cells
        }
        AbilityShape::Line { length } => {
            let step = deploy::bearing(from, aim);
            let mut cells = Vec::new();
            for i in 1..=length as i32 {
                let cell = (from.0 + step.0 * i, from.1 + step.1 * i);
                if !board.in_bounds(cell.0, cell.1) || board.blocks_sight(cell.0, cell.1) {
                    break;
                }
                cells.push(cell);
            }
            cells
        }
        AbilityShape::Cone { length, degrees } => {
            let facing = f64::from(aim.1 - from.1).atan2(f64::from(aim.0 - from.0));
            // Half the aperture either side of the facing, and the epsilon
            // is what keeps a wedge authored at 90 degrees holding the two
            // diagonals that sit exactly 45 degrees off it — on an
            // eight-way grid those are most of what a cone is for.
            let half = f64::from(degrees) / 2.0 * std::f64::consts::PI / 180.0 + 1e-9;
            let reach = length as i32;
            let mut cells = Vec::new();
            for y in (from.1 - reach)..=(from.1 + reach) {
                for x in (from.0 - reach)..=(from.0 + reach) {
                    let cell = (x, y);
                    if cell == from || !board.in_bounds(x, y) {
                        continue;
                    }
                    if distance(from, cell) > length {
                        continue;
                    }
                    let angle = f64::from(y - from.1).atan2(f64::from(x - from.0));
                    let mut off = (angle - facing).abs();
                    if off > std::f64::consts::PI {
                        off = std::f64::consts::TAU - off;
                    }
                    if off <= half && line_of_sight(board, from, cell) {
                        cells.push(cell);
                    }
                }
            }
            cells
        }
    }
}

/// Everybody a routine of this `shape`, run by `actor` and aimed at `aim`,
/// lands on.
///
/// **Whichever side they are on.** Friendly fire is full and is the whole
/// reason a shape is worth aiming: a blast wide enough to catch three
/// hostiles is wide enough to catch the companion standing among them, and
/// a `Radius` heal mends whatever is in it. Nothing here reads `Hostile`,
/// and the omission is the feature.
///
/// In board order — the order the bodies were placed — for `TacticalBattle`'s
/// own reason: bevy's query order is not stable and a fight's recipients
/// must not resolve differently between runs.
pub fn recipients(
    battle: &TacticalBattle,
    actor: Entity,
    aim: (i32, i32),
    shape: AbilityShape,
) -> Vec<Entity> {
    let Some(from) = battle.cell_of(actor) else {
        return Vec::new();
    };
    let covered: HashSet<(i32, i32)> = shape_cells(&battle.board, from, aim, shape)
        .into_iter()
        .collect();
    battle
        .bodies()
        .filter(|(_, cell)| covered.contains(cell))
        .map(|(entity, _)| entity)
        .collect()
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

    /// The path is what the pacing loop spends, one cell a beat, so it holds
    /// the cells entered and never the one already stood on.
    #[test]
    fn a_path_holds_the_cells_walked_and_not_the_one_stood_on() {
        let (mut battle, bodies) = fight(&["......"; 6]);
        battle.place(bodies[0], (0, 0));
        let field = movement_field(&battle, bodies[0], 6);
        let path = path_to(&battle.board, &field, (0, 0), (3, 0));

        assert_eq!(path, vec![(1, 0), (2, 0), (3, 0)]);
        assert_eq!(
            path_to(&battle.board, &field, (0, 0), (0, 0)),
            Vec::new(),
            "standing still is no steps at all"
        );
    }

    /// Every entry is one Chebyshev step from the last, which is what makes
    /// each one a legal `Game::tactical_step`.
    #[test]
    fn every_entry_is_one_step_from_the_one_before_it() {
        let (mut battle, bodies) = fight(&[
            "..........",
            "..........",
            "..XXXXXX..",
            "..........",
            "..........",
            "..........",
            "..........",
            "..........",
            "..........",
            "..........",
        ]);
        battle.place(bodies[0], (4, 0));
        let field = movement_field(&battle, bodies[0], 8);
        let path = path_to(&battle.board, &field, (4, 0), (4, 4));

        assert!(
            !path.is_empty(),
            "the far side is reachable around the wall"
        );
        for pair in std::iter::once(&(4, 0))
            .chain(path.iter())
            .collect::<Vec<_>>()[..]
            .windows(2)
        {
            assert_eq!(distance(*pair[0], *pair[1]), 1, "{path:?} jumped a cell");
        }
        assert!(
            path.iter().all(|&(x, y)| battle.board.walkable(x, y)),
            "{path:?} crossed ground it cannot stand on"
        );
        assert_eq!(path.last(), Some(&(4, 4)));
    }

    /// Rough ground costs two, so the descent has to subtract what entering
    /// a cell cost rather than assuming every step is one.
    #[test]
    fn a_path_across_rough_ground_still_lands_on_its_cell() {
        let (mut battle, bodies) =
            fight(&["......", ".~~~~.", "......", "......", "......", "......"]);
        battle.place(bodies[0], (1, 0));
        let field = movement_field(&battle, bodies[0], 8);
        let path = path_to(&battle.board, &field, (1, 0), (2, 2));

        assert_eq!(path.last(), Some(&(2, 2)), "path was {path:?}");
        assert!(!path.is_empty());
    }

    /// A cell outside the budget was never reached, and a caller handed no
    /// path acts from where it stands.
    #[test]
    fn a_cell_the_field_never_reached_has_no_path() {
        let (mut battle, bodies) = fight(&["......"; 6]);
        battle.place(bodies[0], (0, 0));
        let field = movement_field(&battle, bodies[0], 2);

        assert_eq!(path_to(&battle.board, &field, (0, 0), (5, 5)), Vec::new());
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

    /// A hand-written board with a wall of cover across the middle.
    ///
    /// `.` open, `#` cover, `X` blocked — see `Board::from_rows`.
    fn walled() -> Board {
        Board::from_rows(&[
            ".......", ".......", ".......", "..###..", ".......", "...X...", ".......",
        ])
    }

    #[test]
    fn sight_stops_at_cover_and_carries_over_a_chasm() {
        let board = walled();
        assert!(
            !line_of_sight(&board, (3, 1), (3, 5)),
            "cover in the way did not stop the line"
        );
        assert!(
            line_of_sight(&board, (3, 4), (3, 6)),
            "a chasm is seen over, not through"
        );
    }

    /// Standing *in* cover neither blinds a body nor hides it: the endpoints
    /// are excluded, which is `walkable()`/`blocks_sight()` not being
    /// complements read from the other end.
    #[test]
    fn a_body_standing_in_cover_is_still_seen() {
        let board = walled();
        assert!(line_of_sight(&board, (3, 2), (3, 3)));
        assert!(line_of_sight(&board, (3, 3), (3, 2)));
    }

    #[test]
    fn a_line_runs_from_the_caster_and_stops_at_what_it_cannot_see_through() {
        let board = walled();
        let cells = shape_cells(&board, (3, 0), (3, 6), AbilityShape::Line { length: 6 });
        assert_eq!(
            cells,
            vec![(3, 1), (3, 2)],
            "the line either caught its own caster or ran through cover"
        );
    }

    /// The aim names a direction, not a destination — a line is cast from
    /// the caster whatever cell along it was picked.
    #[test]
    fn a_line_reads_its_aim_as_a_bearing() {
        let board = Board::from_rows(&["....", "....", "....", "...."]);
        let near = shape_cells(&board, (0, 0), (1, 0), AbilityShape::Line { length: 3 });
        let far = shape_cells(&board, (0, 0), (3, 0), AbilityShape::Line { length: 3 });
        assert_eq!(near, far);
        assert_eq!(near, vec![(1, 0), (2, 0), (3, 0)]);
    }

    /// A wedge authored at ninety degrees holds the two diagonals sitting
    /// exactly forty-five degrees off its facing. On an eight-way grid those
    /// are most of what a cone is for, and a strict comparison drops both.
    #[test]
    fn a_ninety_degree_cone_holds_its_diagonals() {
        let board = Board::from_rows(&[".....", ".....", ".....", ".....", "....."]);
        let cells = shape_cells(
            &board,
            (2, 2),
            (2, 0),
            AbilityShape::Cone {
                length: 2,
                degrees: 90,
            },
        );
        for expected in [(2, 1), (1, 1), (3, 1), (0, 0), (4, 0)] {
            assert!(
                cells.contains(&expected),
                "{expected:?} fell out of the cone"
            );
        }
        assert!(
            !cells.contains(&(2, 2)),
            "the cone caught the body that opened it"
        );
        assert!(
            !cells.contains(&(2, 3)),
            "the cone reached behind the body that opened it"
        );
    }

    #[test]
    fn a_cone_drops_what_it_cannot_see() {
        let board = walled();
        let cells = shape_cells(
            &board,
            (3, 1),
            (3, 6),
            AbilityShape::Cone {
                length: 4,
                degrees: 90,
            },
        );
        assert!(cells.contains(&(3, 2)));
        assert!(!cells.contains(&(3, 4)), "the cone reached through cover");
    }

    /// A blast is stopped by nothing — no sight rule, and none of the four
    /// cell kinds excluded.
    #[test]
    fn a_blast_reaches_behind_cover_and_over_a_chasm() {
        let board = walled();
        let cells = shape_cells(&board, (0, 0), (3, 4), AbilityShape::Radius { radius: 1 });
        assert!(cells.contains(&(3, 3)), "the blast stopped at cover");
        assert!(cells.contains(&(3, 5)), "the blast stopped at a chasm");
        assert_eq!(cells.len(), 9);
    }

    #[test]
    fn a_blast_is_clipped_by_the_board_and_not_by_the_arithmetic() {
        let board = Board::from_rows(&["....", "....", "....", "...."]);
        let cells = shape_cells(&board, (0, 0), (0, 0), AbilityShape::Radius { radius: 2 });
        assert!(cells.iter().all(|&(x, y)| board.in_bounds(x, y)));
        assert_eq!(cells.len(), 9);
    }

    #[test]
    fn a_single_target_shape_names_the_cell_it_was_aimed_at() {
        let board = walled();
        assert_eq!(
            shape_cells(&board, (0, 0), (5, 5), AbilityShape::Single),
            vec![(5, 5)]
        );
    }

    #[test]
    fn a_range_is_inclusive_at_both_ends_and_measured_in_steps() {
        let range = AbilityRange { min: 2, max: 3 };
        assert!(!in_range((0, 0), (1, 1), range), "point blank was allowed");
        assert!(in_range((0, 0), (2, 2), range));
        assert!(in_range((0, 0), (3, 0), range));
        assert!(!in_range((0, 0), (4, 4), range), "out of reach was allowed");
    }

    /// Full friendly fire: nothing in `recipients` reads `Hostile`, and
    /// these bodies carry no components at all — which is the proof.
    #[test]
    fn a_blast_lands_on_whoever_is_standing_in_it() {
        let (mut battle, bodies) = fight(&[".....", ".....", ".....", ".....", "....."]);
        battle.place(bodies[0], (2, 2));
        battle.place(bodies[1], (2, 3));
        battle.place(bodies[2], (0, 0));
        let caught = recipients(
            &battle,
            bodies[0],
            (2, 2),
            AbilityShape::Radius { radius: 1 },
        );
        assert_eq!(
            caught,
            vec![bodies[0], bodies[1]],
            "a blast centred on the caster must catch the caster and its neighbour, and \
             nobody standing outside it"
        );
    }

    /// A line is cast away from the body casting it, so the one shape that
    /// can catch its own caster is the blast above.
    #[test]
    fn a_line_never_catches_the_body_that_cast_it() {
        let (mut battle, bodies) = fight(&[".....", ".....", ".....", ".....", "....."]);
        battle.place(bodies[0], (0, 0));
        battle.place(bodies[1], (2, 0));
        let caught = recipients(&battle, bodies[0], (4, 0), AbilityShape::Line { length: 4 });
        assert_eq!(caught, vec![bodies[1]]);
    }

    #[test]
    fn a_body_that_is_not_on_the_board_lands_nothing() {
        let (mut battle, bodies) = fight(&[".....", ".....", ".....", ".....", "....."]);
        battle.place(bodies[1], (2, 2));
        assert!(
            recipients(
                &battle,
                bodies[0],
                (2, 2),
                AbilityShape::Radius { radius: 2 }
            )
            .is_empty()
        );
    }
}
