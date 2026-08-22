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
    stand_in_base(&mut game);
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
    stand_in_base(&mut game);
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
    stand_in_base(&mut game);
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
    stand_in_base(&mut game);
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
    stand_in_base(&mut game);
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

/// What the base is offering is pooled across the neighbours, not reported
/// per structure: the screen asks "how much Core Fragment can I have", and
/// two nodes holding some each is one answer, not two rows the player has to
/// add up.
#[test]
fn what_is_on_offer_is_pooled_across_neighbours_in_item_order() {
    let mut game = Game::new(946, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);

    stocked_structure(
        &mut game,
        "mining_node",
        p.x - 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 2)],
    );
    stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 3), (ids::POWER_CELL, 1)],
    );

    let mut expected = vec![
        (ItemId::from(ids::CORE_FRAGMENT), 5),
        (ItemId::from(ids::POWER_CELL), 1),
    ];
    expected.sort();
    assert_eq!(
        game.collectable_adjacent(),
        expected,
        "one row per item, summed, in ItemId order"
    );
}

/// The neighbour scan is sorted by tile for `assembler_system`'s reason:
/// bevy's query iteration order is not stable, and a *partial* take across
/// two neighbours holding the same item has to drain them in the same order
/// every run. Spawned in the reverse of their tile order, so an unsorted
/// scan genuinely flips.
#[test]
fn the_neighbour_scan_is_sorted_by_tile() {
    let mut game = Game::new(947, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);

    let east = stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 1)],
    );
    let west = stocked_structure(
        &mut game,
        "mining_node",
        p.x - 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 1)],
    );
    let south = stocked_structure(
        &mut game,
        "mining_node",
        p.x,
        p.y + 1,
        &[(ids::CORE_FRAGMENT, 1)],
    );
    let north = stocked_structure(
        &mut game,
        "mining_node",
        p.x,
        p.y - 1,
        &[(ids::CORE_FRAGMENT, 1)],
    );

    assert_eq!(
        game.adjacent_stock(),
        vec![west, north, south, east],
        "(x, y) order, whatever order they were spawned in"
    );
}

/// An empty buffer is not a row, and a diagonal neighbour is not in reach —
/// the offer must name exactly what a commit could take.
#[test]
fn an_empty_buffer_and_a_diagonal_neighbour_offer_nothing() {
    let mut game = Game::new(948, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);

    stocked_structure(&mut game, "mining_node", p.x + 1, p.y, &[]);
    stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y + 1,
        &[(ids::CORE_FRAGMENT, 5)],
    );

    assert!(game.collectable_adjacent().is_empty());
}

/// All four guards in one test: the battle guard alone passes against a bare
/// early return that swallows the others.
///
/// `require_base` and `base_pos` are the same locale condition and so cannot
/// be separated by state — the surface case exercises both. What tells them
/// apart is the mutation: removing either leaves the other refusing.
#[test]
fn nothing_is_on_offer_while_the_party_cannot_collect() {
    let mut game = Game::new(949, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 4)],
    );
    assert!(
        !game.collectable_adjacent().is_empty(),
        "the fixture is offering something to begin with"
    );

    game.world.resource_mut::<GameOver>().reason = Some("test".to_string());
    assert!(game.collectable_adjacent().is_empty(), "game over");
    game.world.resource_mut::<GameOver>().reason = None;

    let player = game.player_entity();
    insert_battle(&mut game, player, Vec::new());
    assert!(game.collectable_adjacent().is_empty(), "in a battle");
    game.world.remove_resource::<BattleState>();

    *game.world.resource_mut::<Locale>() = Locale::Surface;
    assert!(
        game.collectable_adjacent().is_empty(),
        "out of base space entirely"
    );
}
