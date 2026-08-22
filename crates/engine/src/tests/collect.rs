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

/// The refusal is stated in `collect_adjacent` and nowhere else — app-core
/// routes its own empty case back through this function rather than keeping
/// a second copy of the sentence, so the sentence has to actually be here.
///
/// The guards are the other half, and they refuse *silently*: a collect
/// asked for during a battle or from the surface is not the base saying its
/// shelves are bare.
#[test]
fn a_collect_that_finds_nothing_says_so_and_a_refused_one_does_not() {
    let mut game = Game::new(956, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let refusal = "There is nothing to collect here.";
    let said = |game: &Game| {
        game.message_log(usize::MAX)
            .into_iter()
            .filter(|e| e.text == refusal)
            .count()
    };

    assert!(game.collect_adjacent().is_empty());
    assert_eq!(said(&game), 1, "an empty shelf says so");

    let player = game.player_entity();
    insert_battle(&mut game, player, Vec::new());
    assert!(game.collect_adjacent().is_empty());
    game.world.remove_resource::<BattleState>();

    *game.world.resource_mut::<Locale>() = Locale::Surface;
    assert!(game.collect_adjacent().is_empty());
    stand_in_base(&mut game);

    assert_eq!(
        said(&game),
        1,
        "and a guard refuses without claiming anything about the shelves"
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

/// How many `Loot` lines the log holds — the collect summary is one line
/// whatever the basket's size, so counting them is how the tests below tell
/// one summary from a line per item.
fn loot_lines(game: &Game) -> usize {
    game.message_log(usize::MAX)
        .into_iter()
        .filter(|e| e.kind == MessageKind::Loot)
        .count()
}

/// Asking for less than is on the shelf takes exactly that. The remainder
/// stays where the base's chains can still pull it, which is the whole point
/// of the change.
#[test]
fn asking_for_part_of_a_buffer_leaves_the_rest_in_it() {
    let mut game = Game::new(950, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let node = stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 10)],
    );

    let before = count_item(&game, ids::CORE_FRAGMENT);
    let got = game.collect_items(&[(ItemId::from(ids::CORE_FRAGMENT), 4)]);

    assert_eq!(got, vec![(ItemId::from(ids::CORE_FRAGMENT), 4)]);
    assert_eq!(count_item(&game, ids::CORE_FRAGMENT) - before, 4);
    assert_eq!(
        game.world
            .get::<Stock>(node)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied(),
        Some(6),
        "the six the player did not ask for are still the base's"
    );
}

/// The buffers can only shrink between the screen opening and the commit — a
/// raid, a hauler, an assembler pull — so an over-ask hands over what is
/// there rather than refusing the whole basket.
#[test]
fn an_over_ask_is_clamped_rather_than_refused() {
    let mut game = Game::new(951, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let node = stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 3)],
    );

    let before = count_item(&game, ids::CORE_FRAGMENT);
    let got = game.collect_items(&[(ItemId::from(ids::CORE_FRAGMENT), 99)]);

    assert_eq!(
        got,
        vec![(ItemId::from(ids::CORE_FRAGMENT), 3)],
        "what actually landed, never what was asked for"
    );
    assert_eq!(count_item(&game, ids::CORE_FRAGMENT) - before, 3);
    assert!(
        game.world.get::<Stock>(node).unwrap().output.is_empty(),
        "a fully drained buffer is removed, not left holding a zero"
    );
}

/// The test the sort exists for. Two neighbours holding the same item and a
/// request that spans both: the lower `(x, y)` tile is drained first, every
/// run. Spawned in the reverse of their tile order, so an unsorted scan
/// genuinely flips which one is left holding the remainder.
#[test]
fn a_partial_take_across_two_neighbours_drains_them_in_tile_order() {
    let mut game = Game::new(952, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let east = stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 5)],
    );
    let west = stocked_structure(
        &mut game,
        "mining_node",
        p.x - 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 5)],
    );

    let got = game.collect_items(&[(ItemId::from(ids::CORE_FRAGMENT), 7)]);

    assert_eq!(got, vec![(ItemId::from(ids::CORE_FRAGMENT), 7)]);
    assert!(
        game.world.get::<Stock>(west).unwrap().output.is_empty(),
        "the lower tile empties first"
    );
    assert_eq!(
        game.world
            .get::<Stock>(east)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied(),
        Some(3),
        "and the higher one holds the remainder"
    );
}

/// One commit is one action and one piece of news, whatever the basket
/// holds — per-item ticking would price a two-item haul at twice a one-item
/// one, and a line per item would bury the log pane.
#[test]
fn a_basket_of_two_items_ticks_once_and_logs_once() {
    let mut game = Game::new(953, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 4), (ids::POWER_CELL, 4)],
    );
    let ticks = game.world.resource::<GameClock>().tick;
    let lines = loot_lines(&game);

    game.collect_items(&[
        (ItemId::from(ids::CORE_FRAGMENT), 2),
        (ItemId::from(ids::POWER_CELL), 2),
    ]);

    assert_eq!(game.world.resource::<GameClock>().tick, ticks + 1);
    assert_eq!(loot_lines(&game) - lines, 1);
}

/// An empty basket is a refusal, and a refusal has never spent a tick here.
/// An all-zero basket is the same thing said longhand.
#[test]
fn an_empty_or_all_zero_basket_takes_nothing_and_costs_no_turn() {
    let mut game = Game::new(954, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 4)],
    );
    let ticks = game.world.resource::<GameClock>().tick;
    let lines = loot_lines(&game);
    let before = count_item(&game, ids::CORE_FRAGMENT);

    assert!(game.collect_items(&[]).is_empty());
    assert!(
        game.collect_items(&[
            (ItemId::from(ids::CORE_FRAGMENT), 0),
            (ItemId::from(ids::POWER_CELL), 0),
        ])
        .is_empty()
    );

    assert_eq!(count_item(&game, ids::CORE_FRAGMENT), before);
    assert_eq!(game.world.resource::<GameClock>().tick, ticks);
    assert_eq!(
        loot_lines(&game),
        lines,
        "nothing happened, so nothing said"
    );
}

/// The taking path holds the same guards the offer does. All four in one
/// test, for the reason the offer's own guard test states.
#[test]
fn nothing_is_taken_while_the_party_cannot_collect() {
    let mut game = Game::new(955, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_structure(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        &[(ids::CORE_FRAGMENT, 4)],
    );
    let want = vec![(ItemId::from(ids::CORE_FRAGMENT), 4)];
    let before = count_item(&game, ids::CORE_FRAGMENT);

    game.world.resource_mut::<GameOver>().reason = Some("test".to_string());
    assert!(game.collect_items(&want).is_empty(), "game over");
    game.world.resource_mut::<GameOver>().reason = None;

    let player = game.player_entity();
    insert_battle(&mut game, player, Vec::new());
    assert!(game.collect_items(&want).is_empty(), "in a battle");
    game.world.remove_resource::<BattleState>();

    *game.world.resource_mut::<Locale>() = Locale::Surface;
    assert!(
        game.collect_items(&want).is_empty(),
        "out of base space entirely"
    );
    stand_in_base(&mut game);

    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        before,
        "not one unit moved while any guard was up"
    );
    assert!(
        !game.collect_items(&want).is_empty(),
        "and the fixture was collectable all along"
    );
}
