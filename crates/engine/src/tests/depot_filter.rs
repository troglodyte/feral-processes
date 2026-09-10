//! What a Depot refuses to take in: the rule, its two writers, and the door
//! the player's own hands put cargo through.
//!
//! The crew's half of the same rule is in `tests/hauling.rs`, beside the
//! fixtures that walk a worker to a shelf.

use super::support::*;
use crate::*;

/// A Depot at an absolute tile with `output` already in its buffer.
/// Bypasses `place_structure` — these tests are about what a shelf will
/// take, not about the build rules.
fn depot(game: &mut Game, x: i32, y: i32, output: &[(&str, u32)]) -> Entity {
    let mut stock = Stock::new(200);
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

fn row(game: &Game, id: &str) -> TransferRow {
    game.transfer_offer()
        .into_iter()
        .find(|r| r.item == ItemId::from(id))
        .unwrap_or_else(|| panic!("no row for {id}"))
}

fn deny(game: &mut Game, at: Entity, id: &str) {
    game.set_depot_filter(at, &ItemId::from(id), false);
}

/// The state every Depot in every existing save is in: no component, and
/// so nothing refused.
#[test]
fn an_untouched_depot_takes_anything() {
    let mut game = Game::new(2200, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[]);

    assert!(
        game.world.get::<DepotFilter>(shelf).is_none(),
        "a Depot nobody has configured carries no filter at all"
    );
    for item in game.filterable_items() {
        assert!(game.depot_accepts(shelf, &item), "{item:?} must be allowed");
    }
}

/// The component is a representation of "some things are refused", so
/// lifting the last denial must take it away again rather than leave an
/// empty set behind for the save to carry.
#[test]
fn allowing_the_last_denied_item_removes_the_component() {
    let mut game = Game::new(2201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[]);

    deny(&mut game, shelf, ids::CORE_FRAGMENT);
    assert!(game.world.get::<DepotFilter>(shelf).is_some());

    game.set_depot_filter(shelf, &ItemId::from(ids::CORE_FRAGMENT), true);
    assert!(
        game.world.get::<DepotFilter>(shelf).is_none(),
        "the last denial lifted leaves no component behind"
    );
}

/// `[D]` then `[A]`, the two keys the screen offers, back to where it
/// started.
#[test]
fn deny_all_then_allow_all_round_trips() {
    let mut game = Game::new(2202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[]);

    game.set_all_depot_filters(shelf, false);
    let denied = game.filterable_items();
    assert!(!denied.is_empty(), "the catalogue must not be empty");
    for item in &denied {
        assert!(
            !game.depot_accepts(shelf, item),
            "{item:?} must be refused after deny-all"
        );
    }

    game.set_all_depot_filters(shelf, true);
    assert!(game.world.get::<DepotFilter>(shelf).is_none());
}

/// Neither exclusion is decoration: a banked payout never reaches a shelf,
/// and the trade currency is what cargo is priced in.
#[test]
fn the_filter_list_offers_no_currency_and_nothing_banked() {
    let game = Game::new(2203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let listed = game.filterable_items();

    assert!(
        !listed.contains(&game.trade_currency()),
        "no trade currency"
    );
    for item in &listed {
        assert!(!game.is_banked(item), "{item:?} is banked and cannot land");
    }
}

/// The case the feature exists for: two shelves, one refusing, and the
/// cargo lands on the other.
#[test]
fn a_denied_item_lands_in_the_next_depot_instead() {
    let mut game = Game::new(2204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let first = depot(&mut game, p.x - 1, p.y, &[]);
    let second = depot(&mut game, p.x + 1, p.y, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);
    deny(&mut game, first, ids::CORE_FRAGMENT);

    let (_, given) = game.transfer_items(&TransferBasket::items(
        &[],
        &[(ItemId::from(ids::CORE_FRAGMENT), 5)],
    ));

    assert_eq!(given, vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]);
    assert_eq!(node_output(&game, first, ids::CORE_FRAGMENT), 0);
    assert_eq!(node_output(&game, second, ids::CORE_FRAGMENT), 5);
}

/// A refusal is not a hole in the pack: what nowhere will take stays where
/// it is.
#[test]
fn an_item_every_depot_refuses_cannot_be_put_at_all() {
    let mut game = Game::new(2205, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 5)]);
    deny(&mut game, shelf, ids::CORE_FRAGMENT);

    let listed = row(&game, ids::CORE_FRAGMENT);
    assert_eq!(
        listed.carried, 5,
        "the row stays, so the pack is not hidden"
    );
    assert_eq!(
        listed.can_put, 0,
        "but the screen must not offer a put nothing will take"
    );

    let (_, given) = game.transfer_items(&TransferBasket::items(
        &[],
        &[(ItemId::from(ids::CORE_FRAGMENT), 5)],
    ));
    assert!(given.is_empty());
    assert_eq!(
        game.world
            .get::<Inventory>(game.player_entity())
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        5,
        "and the pack keeps it"
    );
}

/// A filter says what may come **in**. Denying something already on the
/// shelf leaves it there and takes nothing away from the take side.
#[test]
fn a_denied_item_already_on_the_shelf_can_still_be_taken() {
    let mut game = Game::new(2206, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[(ids::CORE_FRAGMENT, 6)]);
    deny(&mut game, shelf, ids::CORE_FRAGMENT);

    assert_eq!(row(&game, ids::CORE_FRAGMENT).on_shelves, 6);
    let (taken, _) = game.transfer_items(&TransferBasket::items(
        &[(ItemId::from(ids::CORE_FRAGMENT), 6)],
        &[],
    ));
    assert_eq!(taken, vec![(ItemId::from(ids::CORE_FRAGMENT), 6)]);
}

/// `can_put` is a permission, and with filters it is also a quantity: a
/// Depot that refuses the item is not room for it, however empty it is.
#[test]
fn can_put_counts_only_the_depots_that_accept_the_item() {
    let mut game = Game::new(2207, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let refusing = depot(&mut game, p.x - 1, p.y, &[]);
    let accepting = depot(&mut game, p.x + 1, p.y, &[(ids::CORE_FRAGMENT, 197)]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 40)]);
    deny(&mut game, refusing, ids::CORE_FRAGMENT);

    assert_eq!(
        node_output(&game, accepting, ids::CORE_FRAGMENT),
        197,
        "precondition: three slots left on the only shelf that will take it"
    );
    assert_eq!(
        row(&game, ids::CORE_FRAGMENT).can_put,
        3,
        "the refusing Depot's 200 free slots are not room for this item"
    );
}

/// The shared budget the picker enforces across rows must not count a
/// Depot that will take nothing the party is carrying — the case where a
/// blind sum promises twice the room that exists.
#[test]
fn the_put_budget_ignores_a_depot_that_refuses_the_whole_pack() {
    let mut game = Game::new(2208, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let refusing = depot(&mut game, p.x - 1, p.y, &[]);
    depot(&mut game, p.x + 1, p.y, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 10)]);
    deny(&mut game, refusing, ids::CORE_FRAGMENT);

    assert_eq!(
        game.transfer_room(),
        Some(200),
        "one shelf's worth of room, not two"
    );
}

/// The `None` this call exists to preserve: a Depot that refuses
/// everything is still a Depot standing beside you, and reporting "no
/// Depot here" would send the player off to build one they already have.
#[test]
fn a_depot_that_refuses_everything_is_still_a_depot() {
    let mut game = Game::new(2209, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[]);
    set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 10)]);
    game.set_all_depot_filters(shelf, false);

    assert_eq!(
        game.transfer_room(),
        Some(0),
        "a full shelf and a closed one read the same, and neither is None"
    );
}

/// The screen's own derivation: one row per catalogue item, carrying what
/// this Depot holds of it.
#[test]
fn the_view_names_the_tile_and_marks_the_denied_rows() {
    let mut game = Game::new(2210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[(ids::CORE_FRAGMENT, 12)]);
    deny(&mut game, shelf, ids::CORE_FRAGMENT);

    let view = game.depot_filter_view(shelf).expect("a standing Depot");
    assert_eq!(view.tile, (p.x + 1, p.y));
    assert_eq!(view.room, 188);
    assert_eq!(view.rows.len(), game.filterable_items().len());

    let fragment = view
        .rows
        .iter()
        .find(|r| r.item == ItemId::from(ids::CORE_FRAGMENT))
        .expect("the catalogue lists it");
    assert!(!fragment.allowed);
    assert_eq!(
        fragment.held, 12,
        "denied and holding twelve is not a contradiction"
    );
}

/// The screen holds an entity; the building can go. `None` is what closes
/// it.
#[test]
fn the_view_is_none_for_something_that_is_not_a_depot() {
    let mut game = Game::new(2211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let node = game
        .world
        .spawn((
            Structure {
                kind: "mining_node".to_string(),
            },
            Position { x: p.x + 1, y: p.y },
            Stock::new(20),
        ))
        .id();

    assert!(game.depot_filter_view(node).is_none());
    game.set_all_depot_filters(node, false);
    assert!(
        game.world.get::<DepotFilter>(node).is_none(),
        "and nothing a machine's own buffer could read is written to it"
    );
}

/// A `#[serde(skip)]` or a missed restore arm leaves the RON round trip
/// green, so the gate is a real save and a real load.
#[test]
fn a_filter_survives_a_save_and_load() {
    let mut game = Game::new(2212, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[]);
    deny(&mut game, shelf, ids::CORE_FRAGMENT);
    deny(&mut game, shelf, ids::POWER_CELL);

    let path = std::env::temp_dir().join(format!(
        "feral_processes_depot_filter_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let mut q = loaded.world.query::<(&Structure, &DepotFilter)>();
    let (_, filter) = q
        .iter(&loaded.world)
        .find(|(s, _)| s.kind == "depot")
        .expect("the Depot comes back carrying its filter");
    assert_eq!(
        filter.denied,
        [
            ItemId::from(ids::CORE_FRAGMENT),
            ItemId::from(ids::POWER_CELL)
        ]
        .into_iter()
        .collect()
    );
}

/// **The one door into a Depot that is exempt.** A refund is the base
/// handing back goods it already owned, and `Game::return_material`'s
/// fallback ladder ends in units left in the dust when the player is not in
/// base space to catch them — so a filter honoured here would let a closed
/// shelf destroy materials while the player was out in the field.
#[test]
fn a_refund_ignores_the_filter() {
    let mut game = Game::new(2213, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let p = player_tile(&game);
    let shelf = depot(&mut game, p.x + 1, p.y, &[]);
    game.set_all_depot_filters(shelf, false);

    game.return_material(&ItemId::from(ids::CORE_FRAGMENT), 4);

    assert_eq!(
        node_output(&game, shelf, ids::CORE_FRAGMENT),
        4,
        "a refund lands on the shelf whatever it has been told to refuse"
    );
}

/// Not an assertion about the exact catalogue — a floor and a ceiling, so
/// a mod or a synthesised family that quietly made this screen thousands of
/// rows long fails here rather than at the keyboard.
#[test]
fn the_filter_list_is_a_screenful_and_not_a_catalogue_dump() {
    let game = Game::new(2214, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let listed = game.filterable_items().len();
    assert!(
        (20..=200).contains(&listed),
        "the shipped catalogue offers {listed} filter rows"
    );
}
