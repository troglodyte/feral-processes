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

/// The point of the feature, asserted end to end rather than by inspecting
/// the buffer directly: a deposit is not a stash, it is handing the base
/// your materials, and `base_stock` (what `base_holding` feeds) and
/// `collectable_adjacent` are the two readers that make that true.
#[test]
fn deposited_goods_land_in_output_and_are_visible_to_base_holding_and_collect() {
    let mut game = Game::new(970, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

    let landed = game.deposit_items(&[(ItemId::from(ids::CORE_FRAGMENT), 5)]);
    assert_eq!(landed, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);

    let row = game
        .base_stock()
        .into_iter()
        .find(|r| r.item == ItemId::from(ids::CORE_FRAGMENT));
    assert_eq!(row.map(|r| r.qty), Some(5), "base_holding sees the deposit");
    assert_eq!(
        game.collectable_adjacent(),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 5)],
        "and a collect could take it straight back out"
    );
}

/// Reporting what landed rather than what was asked for is `apply_damage`'s
/// rule: a log line printing the requested figure claims goods the base
/// never received.
#[test]
fn an_over_ask_against_the_pack_is_clamped_to_what_is_held() {
    let mut game = Game::new(971, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 3)]);

    let landed = game.deposit_items(&[(ItemId::from(ids::CORE_FRAGMENT), 50)]);
    assert_eq!(
        landed,
        vec![(ItemId::from(ids::CORE_FRAGMENT), 3)],
        "reports what landed, not what was asked for"
    );
    assert_eq!(count_item(&game, ids::CORE_FRAGMENT), 0);
}

/// Never past `capacity`: an over-capacity write would make that field a
/// suggestion, and a full Depot is a decided failure mode rather than an
/// exception to one.
#[test]
fn an_over_ask_against_room_is_clamped_to_what_fits() {
    let mut game = Game::new(972, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let depot = stocked_depot(&mut game, p.x + 1, p.y, 5, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 50)]);

    let landed = game.deposit_items(&[(ItemId::from(ids::CORE_FRAGMENT), 50)]);
    assert_eq!(landed, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);
    let stock = game.world.get::<Stock>(depot).unwrap();
    assert_eq!(stock.output_used(), 5);
    assert!(stock.output_used() <= stock.capacity, "never past capacity");
}

/// The sort carries collect's reason unchanged, in the giving direction: a
/// basket larger than the first Depot's room has to spill into the second in
/// the same `(x, y)` order every run. Spawned in the reverse of their tile
/// order, `assembler_system`'s test's trick.
#[test]
fn a_basket_larger_than_the_first_depots_room_spills_into_the_second_in_x_y_order() {
    let mut game = Game::new(973, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);

    let east = stocked_depot(&mut game, p.x + 1, p.y, 10, &[]);
    let west = stocked_depot(&mut game, p.x - 1, p.y, 3, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

    let landed = game.deposit_items(&[(ItemId::from(ids::CORE_FRAGMENT), 5)]);
    assert_eq!(landed, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);

    let in_output = |e: Entity| {
        game.world
            .get::<Stock>(e)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0)
    };
    assert_eq!(
        in_output(west),
        3,
        "west fills first and takes all its room"
    );
    assert_eq!(
        in_output(east),
        2,
        "the remaining 2 spill into the next Depot east"
    );
}

/// A full Depot is a decided failure mode: nothing moves, nothing is said,
/// and no turn is spent — the same shape as an empty shelf is for collect.
#[test]
fn a_full_depot_takes_nothing_and_spends_no_tick() {
    let mut game = Game::new(974, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 3, &[(ids::CORE_FRAGMENT, 3)]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 2)]);
    let before = game.world.resource::<GameClock>().tick;

    let landed = game.deposit_items(&[(ItemId::from(ids::CORE_FRAGMENT), 2)]);
    assert!(landed.is_empty());
    assert_eq!(
        count_item(&game, ids::CORE_FRAGMENT),
        2,
        "nothing left the pack"
    );
    assert_eq!(game.world.resource::<GameClock>().tick, before);
}

/// One commit is one action: an empty or all-zero basket is a no-op, and a
/// basket that actually moves something spends exactly one tick.
#[test]
fn a_successful_deposit_costs_one_tick_an_empty_or_all_zero_basket_costs_none() {
    let mut game = Game::new(975, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);
    let before = game.world.resource::<GameClock>().tick;

    assert!(game.deposit_items(&[]).is_empty());
    assert_eq!(
        game.world.resource::<GameClock>().tick,
        before,
        "an empty basket spends nothing"
    );

    assert!(
        game.deposit_items(&[(ItemId::from(ids::CORE_FRAGMENT), 0)])
            .is_empty()
    );
    assert_eq!(
        game.world.resource::<GameClock>().tick,
        before,
        "an all-zero basket spends nothing"
    );

    assert!(
        !game
            .deposit_items(&[(ItemId::from(ids::CORE_FRAGMENT), 5)])
            .is_empty()
    );
    assert_eq!(game.world.resource::<GameClock>().tick, before + 1);
}

/// One line for the whole basket, whatever its size, naming what actually
/// landed — the mirror of collect's `Loot` line, though this is base news
/// rather than something looted.
#[test]
fn a_commit_logs_exactly_one_line_naming_what_landed() {
    let mut game = Game::new(976, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 3), (ids::POWER_CELL, 2)]);
    let core_name = game
        .item_name(&ItemId::from(ids::CORE_FRAGMENT))
        .to_string();
    let cell_name = game.item_name(&ItemId::from(ids::POWER_CELL)).to_string();

    game.deposit_items(&[
        (ItemId::from(ids::CORE_FRAGMENT), 3),
        (ItemId::from(ids::POWER_CELL), 2),
    ]);

    let lines: Vec<_> = game
        .message_log(usize::MAX)
        .into_iter()
        .filter(|e| e.text.starts_with("You put away"))
        .collect();
    assert_eq!(lines.len(), 1, "one line for the whole basket");
    assert!(
        lines[0].text.contains(&format!("3 {core_name}"))
            && lines[0].text.contains(&format!("2 {cell_name}")),
        "names what actually landed: {}",
        lines[0].text
    );
}

/// The refusal is stated here and nowhere else — app-core routes its own
/// empty case back through this function rather than keeping a second copy
/// of the sentence.
#[test]
fn deposit_adjacent_with_no_depot_says_so_and_spends_no_tick() {
    let mut game = Game::new(977, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let before = game.world.resource::<GameClock>().tick;

    assert!(game.deposit_adjacent().is_empty());
    assert_eq!(game.world.resource::<GameClock>().tick, before);

    let said = game
        .message_log(usize::MAX)
        .into_iter()
        .filter(|e| e.text == "There is nowhere here to put anything.")
        .count();
    assert_eq!(said, 1);
}

/// A Depot but nothing to put in it is a different errand than no Depot at
/// all, so it gets its own sentence.
#[test]
fn deposit_adjacent_beside_a_depot_with_an_empty_pack_says_so_and_spends_no_tick() {
    let mut game = Game::new(978, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked_depot(&mut game, p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[]);
    let before = game.world.resource::<GameClock>().tick;

    assert!(game.deposit_adjacent().is_empty());
    assert_eq!(game.world.resource::<GameClock>().tick, before);

    let said = game
        .message_log(usize::MAX)
        .into_iter()
        .filter(|e| e.text == "You have nothing to put away.")
        .count();
    assert_eq!(said, 1);
}

/// The guards refuse *silently*, as they always have: an action taken during
/// a battle, on game over, or off `require_base`'s locale is not the base
/// telling the player it has nowhere to put anything.
#[test]
fn deposit_adjacent_refuses_silently_off_locale_in_battle_and_on_game_over() {
    let mut game = Game::new(979, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let refusal = "There is nowhere here to put anything.";
    let said = |game: &Game| {
        game.message_log(usize::MAX)
            .into_iter()
            .filter(|e| e.text == refusal)
            .count()
    };

    assert!(game.deposit_adjacent().is_empty());
    assert_eq!(said(&game), 1, "an empty base says so");

    let player = game.player_entity();
    insert_battle(&mut game, player, Vec::new());
    assert!(game.deposit_adjacent().is_empty());
    game.world.remove_resource::<BattleState>();

    game.world.resource_mut::<GameOver>().reason = Some("test".to_string());
    assert!(game.deposit_adjacent().is_empty());
    game.world.resource_mut::<GameOver>().reason = None;

    *game.world.resource_mut::<Locale>() = Locale::Surface;
    assert!(game.deposit_adjacent().is_empty());
    stand_in_base(&mut game);

    descend(&mut game);
    assert!(game.deposit_adjacent().is_empty());

    assert_eq!(
        said(&game),
        1,
        "a guard refuses without claiming anything about the base"
    );
}
