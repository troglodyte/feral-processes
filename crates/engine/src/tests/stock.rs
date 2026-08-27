//! The base stock strip: what the base's machines and depots are holding,
//! as the one-row readout draws it.

use super::support::*;
use crate::items::ids;
use crate::*;

fn put_output(game: &mut Game, machine: Entity, item: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
    *stock.output.entry(ItemId::from(item)).or_default() += qty;
}

fn row(game: &Game, tag: &str) -> Option<u32> {
    game.base_stock()
        .into_iter()
        .find(|r| r.tag == tag)
        .map(|r| r.qty)
}

/// The strip reads the same buffers `base_holding` sums, which is what makes
/// it a readout of the base rather than a second opinion about it.
#[test]
fn the_strip_sums_machine_and_depot_buffers_into_one_pile_per_item() {
    let mut game = Game::new(30, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);
    let depot = spawn_machine_at(&mut game, "depot", 4, 0);
    put_output(&mut game, mine, ids::CORE_FRAGMENT, 7);
    put_output(&mut game, depot, ids::CORE_FRAGMENT, 5);
    put_output(&mut game, depot, ids::BYTECODE_BLOCK, 3);

    assert_eq!(
        row(&game, "CF"),
        Some(12),
        "one pile per item, summed across every buffer holding it"
    );
    assert_eq!(row(&game, "BB"), Some(3));
}

/// What the player is carrying is a different figure and belongs to a
/// different screen — the strip claims to say what the *base* has.
#[test]
fn the_strip_does_not_count_what_the_player_is_carrying() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    let carried = game
        .world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .items
        .iter()
        .find(|(i, _)| i.as_str() == ids::CORE_FRAGMENT)
        .map(|(_, q)| *q)
        .unwrap_or(0);
    assert!(carried > 0, "precondition: the player starts holding some");

    assert_eq!(row(&game, "CF"), None, "a full pack is not a stocked base");
}

/// The strip is one row wide, so it lists what a base *stocks* — the
/// materials and currencies recipes are denominated in. A weapon sitting in
/// a buffer would cost a pile off the end of the row to say something the
/// gear screens already say better.
#[test]
fn the_strip_lists_materials_and_currencies_and_nothing_else() {
    let mut game = Game::new(32, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 4, 0);
    put_output(&mut game, depot, ids::CORE_FRAGMENT, 2);
    put_output(&mut game, depot, ids::MONOFILAMENT_WHIP, 4);

    let listed: Vec<String> = game.base_stock().into_iter().map(|r| r.tag).collect();
    assert!(listed.contains(&"CF".to_string()), "a material is listed");
    assert!(
        !listed.iter().any(|t| t == "MW"),
        "a weapon in a buffer is not stock: {listed:?}"
    );
}

/// A base with nothing in it draws nothing, and that is the state the strip
/// spends most of a fresh run in.
#[test]
fn a_base_holding_nothing_lists_nothing() {
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    assert!(game.base_stock().is_empty());
}

/// The order is the item id's, not the quantity's. A strip that re-sorted as
/// buffers filled and drained would move every tag under the player's eye on
/// the tick they were reading it.
#[test]
fn the_piles_keep_one_order_however_they_fill() {
    let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    let depot = spawn_machine_at(&mut game, "depot", 4, 0);
    put_output(&mut game, depot, ids::CORE_FRAGMENT, 1);
    put_output(&mut game, depot, ids::BYTECODE_BLOCK, 99);
    let before: Vec<String> = game.base_stock().into_iter().map(|r| r.tag).collect();

    put_output(&mut game, depot, ids::CORE_FRAGMENT, 500);
    let after: Vec<String> = game.base_stock().into_iter().map(|r| r.tag).collect();

    assert_eq!(before, after, "the biggest pile does not jump to the front");
    assert_eq!(before, vec!["BB".to_string(), "CF".to_string()]);
}

/// The one pile no output buffer can ever hold. `deliver_payout` banks a
/// Research Node's yield straight past the node's own `output`, so walking
/// the buffers alone left the base's only banked product with no row at
/// all — and `research_data.ron` had carried an `abbrev` of `R` for the
/// strip's benefit the whole time it could not be drawn.
#[test]
fn a_banked_pool_is_a_pile_the_strip_lists() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    grant_research_data(&mut game, 30);

    assert_eq!(
        row(&game, "R"),
        Some(30),
        "a banked pool is the base's holding too, however it is stored"
    );
}

/// A machine set up to make something keeps its tag on the strip while the
/// buffer behind it is empty, so the row the player has learnt to read does
/// not reshuffle every time a hauler clears a shelf.
#[test]
fn a_machine_holds_its_pile_on_the_strip_while_its_buffer_is_empty() {
    let mut game = Game::new(36, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    let mine = spawn_machine_at(&mut game, "mining_node", 2, 0);

    assert_eq!(
        row(&game, "CF"),
        Some(0),
        "a deployed Mining Node is the base saying it makes Core Fragments"
    );

    put_output(&mut game, mine, ids::CORE_FRAGMENT, 4);
    assert_eq!(row(&game, "CF"), Some(4), "the same row, now filling");
}

/// An assembler declares no `work` block at all — what it makes is
/// `assembles.item` — so a rule reading only `work.produces` would leave
/// every crafting machine in the base off the strip until its first unit
/// landed.
#[test]
fn an_assembler_holds_the_pile_it_builds() {
    let mut game = Game::new(37, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    spawn_machine_at(&mut game, "lathe", 2, 0);

    assert_eq!(
        row(&game, "BS"),
        Some(0),
        "a Lathe assembles Blank Substrate and says so before it makes one"
    );
}

/// The rule is what the base is set up to *make*, not what it could hold.
/// A Depot makes nothing, so it seeds no pile of its own — otherwise a
/// storage building would put a row on the strip for every item in the game.
#[test]
fn a_depot_seeds_no_pile_of_its_own() {
    let mut game = Game::new(38, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    spawn_machine_at(&mut game, "depot", 4, 0);

    assert!(
        game.base_stock().is_empty(),
        "an empty Depot holds nothing and makes nothing: {:?}",
        game.base_stock()
    );
}
