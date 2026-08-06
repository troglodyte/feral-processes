//! Emptying adjacent structures' output buffers into the player's cargo.

use super::support::*;
use crate::*;

/// Puts a structure of `kind` at an absolute tile with `output` already in
/// its buffer, and returns it. Bypasses `place_structure` — these tests are
/// about reach, not about the build rules.
fn stocked_structure(
    game: &mut Game,
    kind: &str,
    x: i32,
    y: i32,
    output: &[(&str, u32)],
) -> Entity {
    let mut stock = Stock::new(crate::tuning::DEFAULT_OUTPUT_CAPACITY);
    for (id, n) in output {
        stock.output.insert(ItemId::from(*id), *n);
    }
    game.world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
            stock,
        ))
        .id()
}

fn player_tile(game: &Game) -> Position {
    *game.world.get::<Position>(game.player_entity()).unwrap()
}

/// The player pulls by exactly the rule a machine does — four orthogonal
/// tiles, never a diagonal. Standing in the crook of an L empties three
/// buildings; sprawling your base out costs you trips.
#[test]
fn collecting_empties_every_orthogonal_neighbour_and_no_diagonal_one() {
    let mut game = Game::new(940, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = player_tile(&game);

    for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
        stocked_structure(
            &mut game,
            "mining_node",
            p.x + dx,
            p.y + dy,
            &[(ids::CORE_FRAGMENT, 2)],
        );
    }
    let diagonal = stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y + 1,
        &[(ids::CORE_FRAGMENT, 5)],
    );

    let before = count_item(&game, ids::CORE_FRAGMENT);
    let taken = game.collect_adjacent();

    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT) - before,
        8,
        "all four orthogonal neighbours at once, and only those"
    );
    assert_eq!(taken, vec![(ItemId::from(ids::CORE_FRAGMENT), 8)]);
    assert_eq!(
        game.world
            .get::<Stock>(diagonal)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied(),
        Some(5),
        "a diagonal neighbour is out of reach, exactly as it is for a machine"
    );
}

/// A collect can no more reach a machine's `input` than a neighbouring
/// machine can. Without this the player could strip the ingredients out
/// from under a working assembler.
#[test]
fn collecting_leaves_a_neighbours_input_untouched() {
    let mut game = Game::new(941, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = player_tile(&game);

    let node = stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 3)],
    );
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .input
        .insert(ItemId::from(ids::POWER_CELL), 4);

    game.collect_adjacent();

    let stock = game.world.get::<Stock>(node).unwrap();
    assert!(stock.output.is_empty(), "the output is emptied");
    assert_eq!(
        stock.input.get(&ItemId::from(ids::POWER_CELL)).copied(),
        Some(4),
        "the input is not the player's to take"
    );
}

/// A misfired keypress must not cost a turn — the base ticks on, and a
/// player mashing `c` beside nothing would otherwise be spending time.
#[test]
fn collecting_with_nothing_adjacent_takes_nothing_and_costs_no_turn() {
    let mut game = Game::new(943, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let before = game.world.resource::<GameClock>().tick;

    assert!(game.collect_adjacent().is_empty());
    assert_eq!(
        game.world.resource::<GameClock>().tick,
        before,
        "nothing to collect is a refusal, not an action"
    );
}

/// An empty buffer is the same refusal as no buffer at all: the structure
/// is adjacent, but there is nothing in it.
#[test]
fn collecting_from_an_empty_neighbour_costs_no_turn() {
    let mut game = Game::new(944, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = player_tile(&game);
    stocked_structure(&mut game, "mining_node", p.x + 1, p.y, &[]);
    let before = game.world.resource::<GameClock>().tick;

    assert!(game.collect_adjacent().is_empty());
    assert_eq!(game.world.resource::<GameClock>().tick, before);
}

/// A successful collect is an action, and the base moves while you make it.
#[test]
fn a_successful_collect_costs_a_turn() {
    let mut game = Game::new(945, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let p = player_tile(&game);
    stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 1)],
    );
    let before = game.world.resource::<GameClock>().tick;

    assert!(!game.collect_adjacent().is_empty());
    assert_eq!(game.world.resource::<GameClock>().tick, before + 1);
}
