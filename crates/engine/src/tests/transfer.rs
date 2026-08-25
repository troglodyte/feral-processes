//! One screen's worth of moving cargo, in both directions at once.

use super::support::*;
use crate::*;

/// A structure of `kind` at an absolute tile with `output` already in its
/// buffer. Bypasses `place_structure` — these tests are about reach and
/// room, not about the build rules.
fn stocked(
    game: &mut Game,
    kind: &str,
    x: i32,
    y: i32,
    capacity: u32,
    output: &[(&str, u32)],
) -> Entity {
    let mut stock = Stock::new(capacity);
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

fn row(game: &Game, id: &str) -> TransferRow {
    game.transfer_offer()
        .into_iter()
        .find(|r| r.item == ItemId::from(id))
        .unwrap_or_else(|| panic!("no row for {id}"))
}

/// The case the whole feature exists for: one item, on the shelf *and* in
/// the pack, is one row carrying both figures.
#[test]
fn an_item_on_both_sides_is_one_row_with_both_figures() {
    let mut game = Game::new(1740, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        200,
        &[(ids::CORE_FRAGMENT, 6)],
    );
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 4)]);

    let rows = game.transfer_offer();
    assert_eq!(rows.len(), 1, "one item, one row");
    assert_eq!(rows[0].on_shelves, 6);
    assert_eq!(rows[0].in_pack, 4);
}

/// Rows come back in `ItemId` order whichever side each was drawn from.
#[test]
fn rows_from_both_sides_come_back_in_item_order() {
    let mut game = Game::new(1741, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        200,
        &[(ids::POWER_CELL, 2)],
    );
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 3)]);

    let ids: Vec<ItemId> = game.transfer_offer().into_iter().map(|r| r.item).collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(ids, sorted);
    assert_eq!(ids.len(), 2);
}

/// A bank is not cargo, so it never offers a put — but a Research Node
/// produces one, so a banked item on a shelf is still a real take row.
#[test]
fn a_banked_item_offers_a_take_and_never_a_put() {
    let mut game = Game::new(1742, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        200,
        &[(ids::RESEARCH_DATA, 5)],
    );
    set_inventory(&mut game, &[(ids::RESEARCH_DATA, 40)]);

    let r = row(&game, ids::RESEARCH_DATA);
    assert_eq!(r.on_shelves, 5);
    assert_eq!(r.in_pack, 0, "a bank is not cargo");
}

/// Beside a machine that does not `stores`, the pack side is closed
/// entirely — pushing cargo into a Mining Node's output would read as
/// something that machine produced.
#[test]
fn a_non_storing_neighbour_offers_no_put_at_all() {
    let mut game = Game::new(1743, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        200,
        &[(ids::CORE_FRAGMENT, 6)],
    );
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 4)]);

    let rows = game.transfer_offer();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].on_shelves, 6, "the take side is untouched");
    assert_eq!(rows[0].in_pack, 0);
}

/// The three guards, each answering with an empty offer.
#[test]
fn nothing_is_offered_while_the_party_cannot_transfer() {
    let mut game = Game::new(1744, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        200,
        &[(ids::CORE_FRAGMENT, 6)],
    );
    assert!(!game.transfer_offer().is_empty(), "the fixture offers");

    game.world.resource_mut::<GameOver>().reason = Some("test".to_string());
    assert!(game.transfer_offer().is_empty(), "game over");
    game.world.resource_mut::<GameOver>().reason = None;

    let player = game.player_entity();
    insert_battle(&mut game, player, Vec::new());
    assert!(game.transfer_offer().is_empty(), "in a battle");
    game.world.remove_resource::<BattleState>();

    *game.world.resource_mut::<Locale>() = Locale::Surface;
    assert!(game.transfer_offer().is_empty(), "out of base space");
}

/// "No Depot beside you" and "a Depot with nothing left" are the two states
/// the screen must not collapse.
#[test]
fn transfer_room_tells_no_depot_apart_from_a_full_one() {
    let mut game = Game::new(1745, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);

    let node = stocked(&mut game, "mining_node", p.x + 1, p.y, 200, &[]);
    assert_eq!(game.transfer_room(), None, "a Mining Node has no room");
    game.world.despawn(node);

    let depot = stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        10,
        &[(ids::CORE_FRAGMENT, 10)],
    );
    assert_eq!(game.transfer_room(), Some(0), "a Depot at capacity");
    game.world.despawn(depot);

    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        10,
        &[(ids::CORE_FRAGMENT, 4)],
    );
    assert_eq!(game.transfer_room(), Some(6));
}

fn lines(game: &Game) -> Vec<String> {
    game.message_log(usize::MAX)
        .into_iter()
        .map(|e| e.text)
        .collect()
}

/// The ordering constraint. A Depot at exactly `capacity`, a pack holding
/// something it does not, and a basket that takes enough out to make room
/// for what goes in: both halves land only if the take runs first.
#[test]
fn a_transfer_takes_before_it_gives() {
    let mut game = Game::new(1746, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let depot = stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        10,
        &[(ids::CORE_FRAGMENT, 10)],
    );
    set_inventory(&mut game, &[(ids::POWER_CELL, 4)]);

    let (taken, given) = game.transfer_items(
        &[(ItemId::from(ids::CORE_FRAGMENT), 4)],
        &[(ItemId::from(ids::POWER_CELL), 4)],
    );
    assert_eq!(taken, vec![(ItemId::from(ids::CORE_FRAGMENT), 4)]);
    assert_eq!(
        given,
        vec![(ItemId::from(ids::POWER_CELL), 4)],
        "the take made the room the give needed"
    );
    let stock = game.world.get::<Stock>(depot).unwrap();
    assert_eq!(stock.output.get(&ItemId::from(ids::POWER_CELL)), Some(&4));
}

/// One commit is one action, and it says both halves once, take first.
#[test]
fn a_two_way_basket_ticks_once_and_logs_each_half_once() {
    let mut game = Game::new(1747, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        200,
        &[(ids::CORE_FRAGMENT, 6)],
    );
    set_inventory(&mut game, &[(ids::POWER_CELL, 2)]);

    let before = game.current_tick();
    game.transfer_items(
        &[(ItemId::from(ids::CORE_FRAGMENT), 2)],
        &[(ItemId::from(ids::POWER_CELL), 2)],
    );
    assert_eq!(game.current_tick(), before + 1, "one commit, one turn");

    let said = lines(&game);
    let collect_at = said.iter().position(|l| l.starts_with("You collect"));
    let put_at = said.iter().position(|l| l.starts_with("You put away"));
    assert!(collect_at.is_some() && put_at.is_some());
    assert!(collect_at < put_at, "what came is said before what went");
    assert_eq!(
        said.iter().filter(|l| l.starts_with("You collect")).count(),
        1
    );
}

/// Each line is absent when its half moved nothing.
#[test]
fn a_one_way_basket_says_only_its_own_half() {
    let mut game = Game::new(1748, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        200,
        &[(ids::CORE_FRAGMENT, 6)],
    );

    game.transfer_items(&[(ItemId::from(ids::CORE_FRAGMENT), 2)], &[]);
    assert!(lines(&game).iter().any(|l| l.starts_with("You collect")));
    assert!(!lines(&game).iter().any(|l| l.starts_with("You put away")));
}

/// An all-zero basket is a silent no-op costing no turn.
#[test]
fn an_all_zero_basket_spends_no_turn_and_says_nothing() {
    let mut game = Game::new(1749, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        200,
        &[(ids::CORE_FRAGMENT, 6)],
    );
    set_inventory(&mut game, &[(ids::POWER_CELL, 2)]);
    let before = game.current_tick();
    let said = lines(&game).len();

    let (taken, given) = game.transfer_items(
        &[(ItemId::from(ids::CORE_FRAGMENT), 0)],
        &[(ItemId::from(ids::POWER_CELL), 0)],
    );
    assert!(taken.is_empty() && given.is_empty());
    assert_eq!(game.current_tick(), before);
    assert_eq!(lines(&game).len(), said);
}

/// Both clamps still hold through the movers: an over-ask takes what is
/// there, and a give past the room leaves the surplus in the pack rather
/// than eating it.
#[test]
fn the_two_clamps_survive_the_merge() {
    let mut game = Game::new(1750, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        8,
        &[(ids::CORE_FRAGMENT, 3)],
    );
    set_inventory(&mut game, &[(ids::POWER_CELL, 20)]);

    let (taken, given) = game.transfer_items(
        &[(ItemId::from(ids::CORE_FRAGMENT), 99)],
        &[(ItemId::from(ids::POWER_CELL), 20)],
    );
    assert_eq!(taken, vec![(ItemId::from(ids::CORE_FRAGMENT), 3)]);
    assert_eq!(
        given,
        vec![(ItemId::from(ids::POWER_CELL), 8)],
        "the room, not the ask"
    );
    let player = game.player_entity();
    let held = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .items
        .iter()
        .find(|(i, _)| *i == ItemId::from(ids::POWER_CELL))
        .map(|(_, n)| *n)
        .unwrap();
    assert_eq!(held, 12, "the surplus stayed in the pack");
}

/// No `Stock` beside the party at all, and a `Stock` with nothing on either
/// side, leave the player different errands.
#[test]
fn the_two_refusals_say_different_things() {
    let mut game = Game::new(1751, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);

    game.refuse_transfer();
    assert_eq!(
        lines(&game).last().unwrap(),
        "There is nothing here to take from or put into."
    );

    let p = player_tile(&game);
    stocked(&mut game, "depot", p.x + 1, p.y, 200, &[]);
    game.refuse_transfer();
    assert_eq!(
        lines(&game).last().unwrap(),
        "There is nothing to move here."
    );
}

/// A guard refuses without claiming anything about the shelves.
#[test]
fn a_guarded_refusal_is_silent() {
    let mut game = Game::new(1752, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let before = lines(&game).len();

    game.world.resource_mut::<GameOver>().reason = Some("test".to_string());
    game.refuse_transfer();
    game.world.resource_mut::<GameOver>().reason = None;

    let player = game.player_entity();
    insert_battle(&mut game, player, Vec::new());
    game.refuse_transfer();
    game.world.remove_resource::<BattleState>();

    *game.world.resource_mut::<Locale>() = Locale::Surface;
    game.refuse_transfer();

    assert_eq!(lines(&game).len(), before, "no guard says a word");
}
