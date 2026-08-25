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

// The rules below survived the merge of the collect and deposit doors into
// this one, and these are their only cover: the orthogonal reach, the
// `(x, y)` scan order both movers walk in, and the two clamps.

/// The player pulls by exactly the rule a machine does — four orthogonal
/// tiles, never a diagonal. Standing in the crook of an L empties three
/// buildings; sprawling your base out costs you trips.
#[test]
fn a_take_reaches_every_orthogonal_neighbour_and_no_diagonal_one() {
    let mut game = Game::new(1753, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    for (dx, dy) in [(0, -1), (0, 1), (-1, 0), (1, 0)] {
        stocked(
            &mut game,
            "mining_node",
            p.x + dx,
            p.y + dy,
            20,
            &[(ids::CORE_FRAGMENT, 2)],
        );
    }
    let diagonal = stocked(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y + 1,
        20,
        &[(ids::CORE_FRAGMENT, 5)],
    );

    let before = count_item(&game, ids::CORE_FRAGMENT);
    let (taken, _) = game.transfer_items(&[(ItemId::from(ids::CORE_FRAGMENT), 99)], &[]);

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

/// A take can no more reach a machine's `input` than a neighbouring machine
/// can. Without this the player could strip the ingredients out from under a
/// working assembler.
#[test]
fn a_take_leaves_a_neighbours_input_untouched() {
    let mut game = Game::new(1754, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let node = stocked(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        20,
        &[(ids::CORE_FRAGMENT, 3)],
    );
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .input
        .insert(ItemId::from(ids::POWER_CELL), 4);

    game.transfer_items(&[(ItemId::from(ids::CORE_FRAGMENT), 3)], &[]);

    let stock = game.world.get::<Stock>(node).unwrap();
    assert!(stock.output.is_empty(), "the output is emptied");
    assert_eq!(
        stock.input.get(&ItemId::from(ids::POWER_CELL)).copied(),
        Some(4),
        "the input is not the player's to take"
    );
}

/// The shelf side is pooled across every neighbour, one row per item.
#[test]
fn the_shelf_side_is_pooled_across_neighbours() {
    let mut game = Game::new(1755, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(
        &mut game,
        "mining_node",
        p.x - 1,
        p.y,
        20,
        &[(ids::CORE_FRAGMENT, 2)],
    );
    stocked(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        20,
        &[(ids::CORE_FRAGMENT, 3), (ids::POWER_CELL, 1)],
    );

    assert_eq!(row(&game, ids::CORE_FRAGMENT).on_shelves, 5);
    assert_eq!(row(&game, ids::POWER_CELL).on_shelves, 1);
}

/// The neighbour scan is sorted by tile for `assembler_system`'s reason:
/// bevy's query iteration order is not stable, and a *partial* take across
/// two neighbours holding the same item has to drain them in the same order
/// every run. Spawned in the reverse of their tile order, so an unsorted
/// scan genuinely flips.
#[test]
fn the_neighbour_scan_is_sorted_by_tile() {
    let mut game = Game::new(1756, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let east = stocked(&mut game, "mining_node", p.x + 1, p.y, 20, &[]);
    let west = stocked(&mut game, "mining_node", p.x - 1, p.y, 20, &[]);
    assert_eq!(game.adjacent_stock(), vec![west, east]);

    let east = stocked(&mut game, "depot", p.x, p.y + 1, 200, &[]);
    let west = stocked(&mut game, "depot", p.x, p.y - 1, 200, &[]);
    assert_eq!(
        game.adjacent_depots(),
        vec![west, east],
        "and the Depot filter leaves that order alone"
    );
}

/// Asking for less than is on the shelf takes exactly that. The remainder
/// stays where the base's chains can still pull it.
#[test]
fn asking_for_part_of_a_buffer_leaves_the_rest_in_it() {
    let mut game = Game::new(1757, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let node = stocked(
        &mut game,
        "mining_node",
        p.x + 1,
        p.y,
        20,
        &[(ids::CORE_FRAGMENT, 10)],
    );

    let before = count_item(&game, ids::CORE_FRAGMENT);
    let (got, _) = game.transfer_items(&[(ItemId::from(ids::CORE_FRAGMENT), 4)], &[]);

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

/// A partial take across two neighbours drains them in tile order, and a
/// give that outgrows the first Depot spills into the next one in the same
/// order. Both walks are the same `(x, y)` scan.
#[test]
fn both_movers_walk_their_neighbours_in_tile_order() {
    let mut game = Game::new(1758, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let east = stocked(
        &mut game,
        "depot",
        p.x + 1,
        p.y,
        10,
        &[(ids::CORE_FRAGMENT, 5)],
    );
    let west = stocked(
        &mut game,
        "depot",
        p.x - 1,
        p.y,
        8,
        &[(ids::CORE_FRAGMENT, 5)],
    );
    set_inventory(&mut game, &[(ids::POWER_CELL, 5)]);

    let held = |game: &Game, e: Entity, id: &str| {
        game.world
            .get::<Stock>(e)
            .unwrap()
            .output
            .get(&ItemId::from(id))
            .copied()
            .unwrap_or(0)
    };

    let (taken, given) = game.transfer_items(
        &[(ItemId::from(ids::CORE_FRAGMENT), 7)],
        &[(ItemId::from(ids::POWER_CELL), 5)],
    );
    assert_eq!(taken, vec![(ItemId::from(ids::CORE_FRAGMENT), 7)]);
    assert_eq!(
        held(&game, west, ids::CORE_FRAGMENT),
        0,
        "the lower tile empties first"
    );
    assert_eq!(
        held(&game, east, ids::CORE_FRAGMENT),
        3,
        "and the higher one holds the remainder"
    );

    assert_eq!(given, vec![(ItemId::from(ids::POWER_CELL), 5)]);
    assert_eq!(
        held(&game, west, ids::POWER_CELL),
        5,
        "and a give fills the lower tile first too"
    );
}

/// A give that outgrows the first Depot's room spills into the next.
#[test]
fn a_give_larger_than_the_first_depots_room_spills_into_the_second() {
    let mut game = Game::new(1759, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let east = stocked(&mut game, "depot", p.x + 1, p.y, 10, &[]);
    let west = stocked(&mut game, "depot", p.x - 1, p.y, 3, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

    let (_, given) = game.transfer_items(&[], &[(ItemId::from(ids::CORE_FRAGMENT), 5)]);
    assert_eq!(given, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);

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
    assert_eq!(in_output(east), 2, "the remaining 2 spill into the next");
}

/// `Inventory` is by definition the plain-copy store, so a rare or fused
/// copy is never offered as cargo — `Stock` keys by `ItemId` alone and would
/// hand it back ordinary.
#[test]
fn a_gear_copy_in_the_pack_is_never_offered() {
    let mut game = Game::new(1760, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(&mut game, "depot", p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 1)]);
    let player = game.player_entity();
    let rare = ItemId::from(ids::ICE_BREAKER);
    game.world
        .get_mut::<GearCopies>(player)
        .unwrap()
        .add(gear(&rare, 1), 1);

    let offered: Vec<ItemId> = game
        .transfer_offer()
        .into_iter()
        .filter(|r| r.in_pack > 0)
        .map(|r| r.item)
        .collect();
    assert_eq!(offered, vec![ItemId::from(ids::CORE_FRAGMENT)]);
}

/// Room falls as a Depot fills — it is the shared budget the picker enforces
/// live.
#[test]
fn transfer_room_sums_across_depots_and_drops_as_one_fills() {
    let mut game = Game::new(1761, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let first = stocked(&mut game, "depot", p.x + 1, p.y, 10, &[]);
    stocked(&mut game, "depot", p.x - 1, p.y, 5, &[]);
    assert_eq!(game.transfer_room(), Some(15));

    game.world
        .get_mut::<Stock>(first)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 4);
    assert_eq!(game.transfer_room(), Some(11));
}

/// A give is not a stash: it is handing the base your materials, and
/// `base_stock` and the take side of the next offer are the two readers that
/// make that true.
#[test]
fn given_goods_land_in_output_and_the_base_can_see_them() {
    let mut game = Game::new(1762, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    stocked(&mut game, "depot", p.x + 1, p.y, 200, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);

    let (_, landed) = game.transfer_items(&[], &[(ItemId::from(ids::CORE_FRAGMENT), 5)]);
    assert_eq!(landed, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);

    let held = game
        .base_stock()
        .into_iter()
        .find(|r| r.item == ItemId::from(ids::CORE_FRAGMENT));
    assert_eq!(held.map(|r| r.qty), Some(5), "base_holding sees it");
    assert_eq!(
        row(&game, ids::CORE_FRAGMENT).on_shelves,
        5,
        "and the next take could pull it straight back out"
    );
}
