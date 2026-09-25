//! `Game::travel_step` — the route query "walk here" and "follow that
//! hostile" are built on. A read: nothing here should ever move the player
//! or draw `resources::GameRng` on its own, so every test that isn't about
//! that claim itself drives the walk by hand through `stand_player_at` /
//! `stand_in_base_at`, the same way `move_player` would have moved it.

use super::support::*;
use crate::settlements::SettlementKey;
use crate::*;

/// Lays a rectangle of open, walkable ground on the zone surface —
/// deliberately generous around whatever the test carves out of it, so a
/// route never has to fall back on procedurally generated terrain outside
/// the test's control.
fn open_ground(
    game: &mut Game,
    x_range: std::ops::RangeInclusive<i32>,
    y_range: std::ops::RangeInclusive<i32>,
) {
    let mut map = game.world.resource_mut::<crate::world::WorldMap>();
    for x in x_range {
        for y in y_range.clone() {
            map.set_override(
                x,
                y,
                crate::world::Tile {
                    biome: crate::world::Biome::OpenGrid,
                    walkable: true,
                    rock_shade: None,
                },
            );
        }
    }
}

/// Despawns anything already standing in `x_range`/`y_range` that could
/// itself trip `Game::bump_tiles` — a wild creature, a nest, a surface link, a
/// settlement or a trap — without touching anything else (the anchor
/// included, which also carries a `Position`).
///
/// World generation stocks wild population and landmarks near the zone
/// spawn point regardless of whether anyone has walked there (see
/// `support::clear_creatures_along_ray`'s doc for the history), so a test
/// that plants its own obstacle on a specific cell near `(0, 0)` has to
/// clear whatever the seed already put there first, or the assertion is at
/// the mercy of that seed rather than of the obstacle the test actually
/// placed.
fn clear_bump_ladder_entities(
    game: &mut Game,
    x_range: std::ops::RangeInclusive<i32>,
    y_range: std::ops::RangeInclusive<i32>,
) {
    let candidates: Vec<(Entity, i32, i32)> = {
        let mut query = game.world.query::<(Entity, &Position)>();
        query
            .iter(&game.world)
            .map(|(e, p)| (e, p.x, p.y))
            .collect()
    };
    let player = game.player_entity();
    for (entity, x, y) in candidates {
        if entity == player || !x_range.contains(&x) || !y_range.contains(&y) {
            continue;
        }
        let is_bump_arm = game.world.get::<Creature>(entity).is_some()
            || game.world.get::<Nest>(entity).is_some()
            || game.world.get::<SurfaceLink>(entity).is_some()
            || game
                .world
                .get::<crate::components::Settlement>(entity)
                .is_some()
            || game.world.get::<crate::components::Trap>(entity).is_some();
        if is_bump_arm {
            game.world.despawn(entity);
        }
    }
}

/// Drives `travel_step` to convergence, applying each `Toward` step to the
/// player's actual position (surface `Position` or the base-space cell)
/// exactly as `App::update_realtime` would through `move_player` /
/// `move_in_base` — except the last step, which this stops short of
/// applying, since `Last`/`Arrived`/`NoRoute`/`Gone` are all terminal
/// answers a caller acts on rather than a tile this helper should step
/// onto.
///
/// Returns every tile actually stepped onto, in order, plus the terminal
/// answer.
fn walk_route(game: &mut Game, goal: TravelGoal, in_base: bool) -> (Vec<(i32, i32)>, TravelStep) {
    let mut visited = Vec::new();
    for _ in 0..64 {
        let step = game.travel_step(goal);
        let TravelStep::Toward(dx, dy) = step else {
            return (visited, step);
        };
        let (x, y) = if in_base {
            game.base_pos().unwrap()
        } else {
            let p = *game.world.get::<Position>(game.player_entity()).unwrap();
            (p.x, p.y)
        };
        let next = (x + dx, y + dy);
        if in_base {
            stand_in_base_at(game, next.0, next.1);
        } else {
            stand_player_at(game, next.0, next.1);
        }
        visited.push(next);
    }
    panic!("route did not converge to a terminal TravelStep within 64 steps");
}

#[test]
fn arrived_on_the_goal_tile() {
    let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_player_at(&mut game, 5, 5);
    assert_eq!(
        game.travel_step(TravelGoal::Tile(5, 5)),
        TravelStep::Arrived
    );
}

#[test]
fn last_step_beside_the_goal_is_an_ordinary_bump_delta() {
    let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_player_at(&mut game, 5, 5);
    assert_eq!(
        game.travel_step(TravelGoal::Tile(6, 6)),
        TravelStep::Last(1, 1)
    );
}

/// A hostile goal that stands on the tile itself must still answer `Last`
/// beside it rather than treating the occupied tile as unreachable — the
/// whole reason `Last` is answered on raw adjacency, before any route is
/// built.
#[test]
fn a_hostile_goal_answers_last_when_adjacent() {
    let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", pos.x + 1, pos.y);
    assert_eq!(
        game.travel_step(TravelGoal::Creature(wild)),
        TravelStep::Last(1, 0)
    );
}

/// Ties in the field are broken by `NEIGHBOURS` order, deterministically:
/// three of the player's neighbours are equally close to the goal here
/// (Chebyshev distance treats a diagonal step as cheap as a straight one),
/// and `(1, -1)` is first among them in `world::NEIGHBOURS`.
#[test]
fn first_step_breaks_a_tie_by_neighbours_order() {
    let mut game = Game::new(9004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_ground(&mut game, -3..=8, -4..=4);
    clear_bump_ladder_entities(&mut game, -3..=8, -4..=4);
    stand_player_at(&mut game, 0, 0);
    assert_eq!(
        game.travel_step(TravelGoal::Tile(5, 0)),
        TravelStep::Toward(1, -1),
        "(1, -1), (1, 0) and (1, 1) are all Chebyshev distance 4 from the goal; \
         NEIGHBOURS lists (1, -1) first"
    );
}

/// The regression this feature exists for: a route must never cross a tile
/// that would trip `move_player`'s bump ladder mid-walk — here, a hostile
/// planted on the exact cell an unobstructed route would otherwise take.
///
/// **Deliberately not "somewhere on the straight line."** In open
/// 8-directional ground, shifting a route by one row around a single point
/// obstacle costs nothing extra (a diagonal step keeps making horizontal
/// progress), so a test that only checks the obstacle's tile is absent from
/// the final path can pass by coincidence — the tie-break alone might
/// dodge it, obstacle or not, exactly as `first_step_breaks_a_tie_by_
/// neighbours_order` computes. Planting the hostile *at* that computed tie
/// winner and asserting the runner-up closes that hole: without `bump_tiles`
/// in the cost function, this would answer `Toward(1, -1)` instead, onto
/// the hostile.
#[test]
fn a_route_never_steps_onto_a_hostile_blocking_the_favoured_cell() {
    let mut game = Game::new(9005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_ground(&mut game, -3..=13, -6..=6);
    clear_bump_ladder_entities(&mut game, -3..=13, -6..=6);
    stand_player_at(&mut game, 0, 0);
    // (1, -1), (1, 0) and (1, 1) are the three-way tie toward goal (5, 0);
    // NEIGHBOURS order favours (1, -1) — see the tie-break test above.
    spawn_wild_without_routine(&mut game, "scrapper", 1, -1);

    assert_eq!(
        game.travel_step(TravelGoal::Tile(5, 0)),
        TravelStep::Toward(1, 0),
        "the route must fall through to the tie's runner-up rather than \
         stepping onto the hostile occupying the favoured cell"
    );
}

/// The bump ladder's fourth arm, and its own test: a settlement must never
/// be routed *through* — that would open a visit the player never asked
/// for. Same construction as the hostile test above and for the same
/// reason: the obstacle sits on the cell the tie-break would otherwise
/// favour, not merely "on the straight line", so a route that dodges it
/// only by coincidence would still fail this.
#[test]
fn a_route_never_steps_onto_a_settlement_blocking_the_favoured_cell() {
    let mut game = Game::new(9006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_ground(&mut game, -3..=13, -6..=6);
    clear_bump_ladder_entities(&mut game, -3..=13, -6..=6);
    stand_player_at(&mut game, 0, 0);
    place_settlement(&mut game, SettlementKey { rx: 0, ry: 0 }, 1, -1);

    assert_eq!(
        game.travel_step(TravelGoal::Tile(5, 0)),
        TravelStep::Toward(1, 0),
        "the route must fall through to the tie's runner-up rather than \
         stepping onto the settlement occupying the favoured cell"
    );
}

/// A hostile goal is routed *to*: the walk keeps taking `Toward` steps
/// until it is Chebyshev-adjacent to wherever the hostile actually stands,
/// then answers `Last` pointed straight at it.
#[test]
fn a_hostile_goal_is_routed_to_and_last_is_answered_beside_it() {
    let mut game = Game::new(9007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_ground(&mut game, -3..=13, -6..=6);
    stand_player_at(&mut game, 0, 0);
    let wild = spawn_wild_without_routine(&mut game, "scrapper", 6, 0);

    let (_visited, terminal) = walk_route(&mut game, TravelGoal::Creature(wild), false);

    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let wild_pos = *game.world.get::<Position>(wild).unwrap();
    assert_eq!(
        terminal,
        TravelStep::Last(wild_pos.x - pos.x, wild_pos.y - pos.y)
    );
    let distance = (wild_pos.x - pos.x).abs().max((wild_pos.y - pos.y).abs());
    assert_eq!(distance, 1, "Last must only fire from an adjacent tile");
}

/// A player boxed in on all eight sides by hostiles has no neighbour a
/// route may ever step onto, however far the goal is — `NoRoute`, and
/// never a route that quietly cuts through one of them.
#[test]
fn no_route_when_boxed_in_by_hostiles() {
    let mut game = Game::new(9008, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    open_ground(&mut game, -5..=25, -5..=25);
    stand_player_at(&mut game, 0, 0);
    for (dx, dy) in crate::world::NEIGHBOURS {
        spawn_wild_without_routine(&mut game, "scrapper", dx, dy);
    }

    assert_eq!(
        game.travel_step(TravelGoal::Tile(20, 0)),
        TravelStep::NoRoute
    );
}

/// A `Creature` goal that has been despawned mid-travel answers `Gone`
/// rather than panicking or resolving to a stale tile.
#[test]
fn gone_after_the_creature_goal_is_despawned() {
    let mut game = Game::new(9009, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", 8, 8);
    game.world.despawn(wild);
    assert_eq!(
        game.travel_step(TravelGoal::Creature(wild)),
        TravelStep::Gone
    );
}

/// The base-space counterpart of the surface detour tests: a route must
/// avoid solid rock (never floored, so never walkable) and a body standing
/// in base space (`Game::blocked_tiles`) alike, without a new body scan of
/// its own.
#[test]
fn a_base_route_avoids_solid_rock_and_bodies() {
    let mut game = Game::new(9010, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 0, 0);
    {
        let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
        for x in -1..=9 {
            for y in -3..=3 {
                if (x, y) != (4, 0) {
                    grid.lay_floor(x, y);
                }
            }
        }
    }
    let player = game.player_entity();
    game.world
        .spawn((Tamed { owner: player }, Position { x: 4, y: 1 }));
    game.world
        .spawn((Tamed { owner: player }, Position { x: 4, y: -1 }));

    let (visited, terminal) = walk_route(&mut game, TravelGoal::Tile(9, 0), true);

    assert!(
        !visited.contains(&(4, 0)),
        "the route stepped onto solid rock: {visited:?}"
    );
    assert!(
        !visited.contains(&(4, 1)) && !visited.contains(&(4, -1)),
        "the route stepped onto a body: {visited:?}"
    );
    assert!(
        matches!(terminal, TravelStep::Last(..)),
        "the route must still reach the goal's neighbourhood: {terminal:?}"
    );

    // The broad walk above can dodge a body by the same tie-break
    // coincidence `a_route_never_steps_onto_a_hostile_blocking_the_
    // favoured_cell`'s doc explains — a one-row shift around a single point
    // costs nothing extra in Chebyshev movement, obstacle or not. Standing
    // one cell short of the rock pins the exact decision: from (3, 0),
    // (4, -1) and (4, 1) are the cheapest walkable neighbours toward (9, 0)
    // (the rock at (4, 0) itself is unwalkable outright), tied and both
    // bodied — so the real runner-up is (3, -1), and without
    // `Game::blocked_tiles` in the cost function the answer would be
    // `Toward(1, -1)`, straight onto the body at (4, -1).
    stand_in_base_at(&mut game, 3, 0);
    assert_eq!(
        game.travel_step(TravelGoal::Tile(9, 0)),
        TravelStep::Toward(0, -1)
    );
}

/// A base route must cross an ordinary structure's own anchor — the way
/// `Game::move_in_base` itself walks over a machine rather than refusing
/// it — while a one-tile corridor still means only one way through, so a
/// route that treated the anchor as a wall (`Game::blocked_tiles`'s old
/// behaviour) would answer `NoRoute` here instead of routing straight
/// across it.
#[test]
fn a_base_route_walks_over_a_structures_own_anchor() {
    let mut game = Game::new(9012, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, -3, 0);
    {
        let mut grid = game.world.resource_mut::<crate::base_grid::BaseGrid>();
        for x in -3..=9 {
            grid.lay_floor(x, 0);
        }
    }
    spawn_structure_at(&mut game, "data_cache", 3, 0);

    let (visited, terminal) = walk_route(&mut game, TravelGoal::Tile(9, 0), true);

    assert!(
        visited.contains(&(3, 0)),
        "a route in a one-tile corridor must cross a structure's anchor, \
         which move_in_base walks over freely: {visited:?}"
    );
    assert!(
        matches!(terminal, TravelStep::Last(..)),
        "the route must still reach the goal's neighbourhood: {terminal:?}"
    );
}

/// `travel_step` is a pure read: a plain tile goal and a `Creature` goal
/// that resolves to the player's own tile (an `Arrived` answer, the
/// cheapest branch that still touches the `World`) must both leave the
/// shared RNG stream exactly where they found it — the Predation no-draw
/// test's shape, `rng_unadvanced_by`.
#[test]
fn travel_step_draws_no_gamerng() {
    assert!(
        rng_unadvanced_by(9011, |game| {
            let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
            let _ = game.travel_step(TravelGoal::Tile(pos.x + 40, pos.y + 40));
            let player = game.player_entity();
            let _ = game.travel_step(TravelGoal::Creature(player));
        }),
        "travel_step must never touch the shared GameRng stream"
    );
}
