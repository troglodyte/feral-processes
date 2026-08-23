//! Putting plain cargo into an adjacent Depot's `Stock::output` — the mirror
//! of `collect.rs`, giving instead of taking.

use super::support::*;
use crate::*;

/// Puts a Depot at an absolute tile with a chosen `capacity`, and output
/// already sitting in it if any is given. Bypasses `place_structure` — these
/// tests are about reach and room, not about the build rules.
///
/// A twin of `collect.rs`'s `stocked_structure`, kept as its own copy rather
/// than shared: each test file's fixtures are private to it, the same way
/// `collect.rs`'s own helper is invisible here.
fn stocked_depot(game: &mut Game, x: i32, y: i32, capacity: u32, output: &[(&str, u32)]) -> Entity {
    let mut stock = Stock::new(capacity);
    for (id, n) in output {
        stock.output.insert(ItemId::from(*id), *n);
    }
    game.world
        .spawn((
            Structure {
                kind: "depot".to_string(),
            },
            Position { x, y },
            stock,
        ))
        .id()
}

fn player_tile(game: &Game) -> Position {
    *game.world.get::<Position>(game.player_entity()).unwrap()
}

/// What the pack has to offer, beside a Depot.
#[test]
fn depositable_lists_the_packs_plain_rows_beside_a_depot() {
    let mut game = Game::new(960, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 4)]);

    assert_eq!(
        game.depositable(),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 4)]
    );
}

/// The trap `PlayerStatus::inventory` already closes: a bank is not cargo,
/// and putting Research Data in a Depot would make it spendable by the base
/// as though it were.
#[test]
fn depositable_excludes_banked_items() {
    let mut game = Game::new(961, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    set_inventory(
        &mut game,
        &[(ids::CORE_FRAGMENT, 2), (ids::RESEARCH_DATA, 40)],
    );

    assert_eq!(
        game.depositable(),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 2)],
        "Research Data is a bank, not cargo"
    );
}

/// `Stock` keys by `ItemId` alone, so a rare or fused copy put in would come
/// back ordinary — `Inventory` is by definition the plain-copy store, and
/// this list must read only that one.
#[test]
fn depositable_excludes_gear_copies_entirely() {
    let mut game = Game::new(962, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 1)]);
    let player = game.player_entity();
    let rare = ItemId::from(ids::ICE_BREAKER);
    game.world
        .get_mut::<GearCopies>(player)
        .unwrap()
        .add(gear(&rare, 1), 1);

    assert_eq!(
        game.depositable(),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 1)],
        "a rare or fused copy in the pack is not offered"
    );
}

/// `Inventory::items` is a `Vec` in insertion order; without an explicit
/// sort the rows would come back in pickup order, disagreeing with the log
/// line a deposit later prints.
#[test]
fn depositable_rows_come_back_sorted_by_item_id() {
    let mut game = Game::new(963, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    // Reverse-alphabetical pickup order.
    set_inventory(&mut game, &[(ids::POWER_CELL, 1), (ids::CORE_FRAGMENT, 1)]);

    assert_eq!(
        game.depositable(),
        vec![
            (ItemId::from(ids::CORE_FRAGMENT), 1),
            (ItemId::from(ids::POWER_CELL), 1),
        ],
        "alphabetical, not pickup order"
    );
}

/// The whole difference from `collectable_adjacent`: a Mining Node has a
/// `Stock` but does not `stores`, so it is not a valid place to put cargo —
/// mirroring collect exactly would let the player push materials into a
/// machine's own output as though that machine had produced them.
#[test]
fn depositable_is_empty_beside_a_stock_that_does_not_store() {
    let mut game = Game::new(964, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    game.world.spawn((
        Structure {
            kind: "mining_node".to_string(),
        },
        Position { x: p.x + 1, y: p.y },
        Stock::new(crate::tuning::DEFAULT_OUTPUT_CAPACITY),
    ));

    assert!(
        game.depositable().is_empty(),
        "a Mining Node's buffer must not accept cargo"
    );
}

/// All the guards in one test, mirroring collect's own: nothing adjacent, an
/// active battle, game over, and `require_base` failing on both the surface
/// and underground.
#[test]
fn nothing_is_on_offer_while_the_party_cannot_deposit() {
    let mut game = Game::new(965, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    assert!(
        game.depositable().is_empty(),
        "nothing adjacent to begin with"
    );

    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    assert!(
        !game.depositable().is_empty(),
        "the fixture is offering something to begin with"
    );

    game.world.resource_mut::<GameOver>().reason = Some("test".to_string());
    assert!(game.depositable().is_empty(), "game over");
    game.world.resource_mut::<GameOver>().reason = None;

    let player = game.player_entity();
    insert_battle(&mut game, player, Vec::new());
    assert!(game.depositable().is_empty(), "in a battle");
    game.world.remove_resource::<BattleState>();

    *game.world.resource_mut::<Locale>() = Locale::Surface;
    assert!(game.depositable().is_empty(), "out on the surface");
    stand_in_base(&mut game);

    descend(&mut game);
    assert!(game.depositable().is_empty(), "underground");
}

/// The sort carries collect's reason unchanged: a partial fill across two
/// Depots must drain them in the same order every run. Spawned in the
/// reverse of their tile order, `assembler_system`'s test's trick, so an
/// unsorted scan genuinely flips.
#[test]
fn adjacent_depots_are_in_x_y_order_whatever_order_they_were_spawned_in() {
    let mut game = Game::new(966, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);

    let east = stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    let west = stocked_depot(&mut game, p.x - 1, p.y, 200, &[]);

    assert_eq!(game.adjacent_depots(), vec![west, east]);
}

/// `deposit_room` sums room across every adjacent Depot, and falls as a
/// Depot fills — it is the shared budget the picker has to enforce live.
#[test]
fn deposit_room_sums_across_depots_and_drops_as_one_fills() {
    let mut game = Game::new(967, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);

    let first = stocked_depot(&mut game, p.x + 1, p.y, 10, &[]);
    stocked_depot(&mut game, p.x - 1, p.y, 5, &[]);
    assert_eq!(game.deposit_room(), 15);

    game.world
        .get_mut::<Stock>(first)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 4);
    assert_eq!(game.deposit_room(), 11, "the filled Depot has 4 less room");
}
