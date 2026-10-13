//! A Depot beside a producer: the filter it gains once at build, the output
//! it pulls while the producer is worked, and the haulers and map joins that
//! have to agree with both.

use super::support::*;
use crate::*;

/// A base with a Home and a purse, standing in base space.
fn base_game(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    give(&mut game, &ItemId::from(ids::CORE_FRAGMENT), 2000);
    game
}

fn build(game: &mut Game, kind: &str, dx: i32, dy: i32) -> Entity {
    place_now(game, kind, dx, dy).unwrap_or_else(|e| panic!("{kind} at ({dx},{dy}): {e}"));
    let at = game.base_pos().expect("in base space");
    structure_at(game, at.0 + dx, at.1 + dy)
}

fn structure_at(game: &mut Game, x: i32, y: i32) -> Entity {
    let mut q = game.world.query::<(Entity, &Position, &Structure)>();
    q.iter(&game.world)
        .find(|(_, p, _)| (p.x, p.y) == (x, y))
        .map(|(e, ..)| e)
        .unwrap_or_else(|| panic!("a structure stands at ({x},{y})"))
}

fn frag() -> ItemId {
    ItemId::from(ids::CORE_FRAGMENT)
}

/// Whether the Depot takes `item`, asked through the one door.
fn takes(game: &Game, depot: Entity, item: &ItemId) -> bool {
    game.depot_accepts(depot, item)
}

/// Every filterable item except `allowed` is refused, and `allowed` is taken.
fn allow_only(game: &Game, depot: Entity, allowed: &[ItemId]) -> bool {
    game.filterable_items()
        .iter()
        .all(|i| takes(game, depot, i) == allowed.contains(i))
}

#[test]
fn a_depot_built_beside_a_node_allows_only_its_output() {
    let mut game = base_game(9001);
    build(&mut game, "mining_node", 2, 0);
    let depot = build(&mut game, "depot", 1, 0);

    assert!(allow_only(&game, depot, &[frag()]));
}

#[test]
fn a_node_built_beside_an_unfiltered_depot_filters_it() {
    let mut game = base_game(9002);
    let depot = build(&mut game, "depot", 1, 0);
    assert!(takes(&game, depot, &ItemId::from(ids::POWER_CELL)));
    build(&mut game, "mining_node", 2, 0);

    assert!(allow_only(&game, depot, &[frag()]));
}

#[test]
fn a_manual_filter_only_gains_the_producers_item() {
    let mut game = base_game(9003);
    let depot = build(&mut game, "depot", 1, 0);
    let cell = ItemId::from(ids::POWER_CELL);
    game.set_depot_filter(depot, &[frag(), cell.clone()], false);
    build(&mut game, "mining_node", 2, 0);

    assert!(takes(&game, depot, &frag()), "the node's output is let in");
    assert!(
        !takes(&game, depot, &cell),
        "and nothing the player denied is lifted"
    );
}

#[test]
fn a_depot_between_two_producers_allows_both_outputs() {
    let mut game = base_game(9004);
    build(&mut game, "mining_node", 1, 0);
    build(&mut game, "refinery", 3, 0);
    let depot = build(&mut game, "depot", 2, 0);

    assert!(allow_only(
        &game,
        depot,
        &[frag(), ItemId::from("bytecode_block")]
    ));
}

#[test]
fn a_banked_producer_writes_no_filter() {
    let mut game = base_game(9005);
    let depot = build(&mut game, "depot", 1, 0);
    build(&mut game, "research_node", 2, 0);

    assert!(
        game.world.get::<DepotFilter>(depot).is_none(),
        "a banked product never reaches a buffer, so a filter for it is noise"
    );
}

#[test]
fn the_filter_is_set_once_and_a_later_edit_stands() {
    let mut game = base_game(9006);
    build(&mut game, "mining_node", 2, 0);
    let depot = build(&mut game, "depot", 1, 0);
    game.set_depot_filter(depot, &[frag()], false);
    // An upgrade or any later tick must not re-derive it.
    game.tick();

    assert!(!takes(&game, depot, &frag()));
}
