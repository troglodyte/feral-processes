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
fn a_node_built_beside_an_unfiltered_depot_leaves_it_unfiltered() {
    let mut game = base_game(9002);
    let depot = build(&mut game, "depot", 1, 0);
    build(&mut game, "mining_node", 2, 0);

    assert!(
        game.world.get::<DepotFilter>(depot).is_none(),
        "only placing a Depot filters it"
    );
    assert!(takes(&game, depot, &ItemId::from(ids::POWER_CELL)));
}

#[test]
fn a_node_built_beside_a_filtered_depot_leaves_the_players_filter() {
    let mut game = base_game(9003);
    let depot = build(&mut game, "depot", 1, 0);
    let cell = ItemId::from(ids::POWER_CELL);
    game.set_depot_filter(depot, &[frag(), cell.clone()], false);
    build(&mut game, "mining_node", 2, 0);

    assert!(!takes(&game, depot, &frag()), "the node does not lift it");
    assert!(!takes(&game, depot, &cell));
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

// ---- the intake -------------------------------------------------------

/// A machine of `kind` at an absolute tile, on a lit grid.
fn machine(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    stand_ample_grid_supply(game);
    spawn_machine_at(game, kind, x, y)
}

/// A node holding `qty` fragments, worked or not.
fn node(game: &mut Game, x: i32, y: i32, qty: u32, worked: bool) -> Entity {
    let node = machine(game, "mining_node", x, y);
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(frag(), qty);
    if worked {
        post(game, node);
    }
    node
}

fn post(game: &mut Game, machine: Entity) {
    let worker = spawn_tamed(game, 10, 3);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required: 1_000_000,
    });
}

/// A Depot that lets fragments in, standing at an absolute tile.
fn depot_for_fragments(game: &mut Game, x: i32, y: i32) -> Entity {
    let depot = machine(game, "depot", x, y);
    game.set_depot_filter(depot, &[frag()], true);
    depot
}

fn output_of(game: &Game, e: Entity, item: &ItemId) -> u32 {
    game.world
        .get::<Stock>(e)
        .unwrap()
        .output
        .get(item)
        .copied()
        .unwrap_or(0)
}

#[test]
fn a_worked_nodes_output_moves_into_the_depot() {
    let mut game = Game::new(9010, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let depot = depot_for_fragments(&mut game, 41, 40);
    node(&mut game, 40, 40, 10, true);

    game.tick();

    assert!(output_of(&game, depot, &frag()) > 0);
}

#[test]
fn an_unstaffed_nodes_output_stays_put() {
    let mut game = Game::new(9011, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let depot = depot_for_fragments(&mut game, 41, 40);
    let node = node(&mut game, 40, 40, 10, false);

    for _ in 0..5 {
        game.tick();
    }

    assert_eq!(output_of(&game, depot, &frag()), 0);
    assert_eq!(output_of(&game, node, &frag()), 10);
}

#[test]
fn a_dark_nodes_output_stays_put() {
    let mut game = Game::new(9015, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // No grid supply stood, so the worked node draws power it cannot get.
    let depot = spawn_machine_at(&mut game, "depot", 41, 40);
    game.set_depot_filter(depot, &[frag()], true);
    let node = spawn_machine_at(&mut game, "mining_node", 40, 40);
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(frag(), 10);
    post(&mut game, node);

    for _ in 0..5 {
        game.tick();
    }

    assert!(
        game.world
            .resource::<crate::resources::PowerGrid>()
            .is_dark(node),
        "precondition"
    );
    assert_eq!(output_of(&game, depot, &frag()), 0);
    assert_eq!(output_of(&game, node, &frag()), 10);
}

#[test]
fn a_full_depot_stops_pulling_and_the_buffer_stays() {
    let mut game = Game::new(9012, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let depot = depot_for_fragments(&mut game, 41, 40);
    let node = node(&mut game, 40, 40, 30, true);
    let cap = game.world.get::<Stock>(depot).unwrap().capacity;
    // Three units of room, then none.
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from(ids::POWER_CELL), cap - 3);

    game.tick();
    let after_one = output_of(&game, depot, &frag());
    assert_eq!(after_one, 3, "the pull stops at the room the Depot has");
    for _ in 0..5 {
        game.tick();
    }

    assert_eq!(
        output_of(&game, depot, &frag()),
        3,
        "a full Depot pulls nothing more"
    );
    assert_eq!(game.world.get::<Stock>(depot).unwrap().output_room(), 0);
    assert!(
        output_of(&game, node, &frag()) >= 27,
        "the producer keeps what the Depot could not take"
    );
}

#[test]
fn an_item_the_filter_refuses_is_not_pulled() {
    let mut game = Game::new(9013, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let depot = depot_for_fragments(&mut game, 41, 40);
    game.set_depot_filter(depot, &[frag()], false);
    let node = node(&mut game, 40, 40, 10, true);

    for _ in 0..5 {
        game.tick();
    }

    assert_eq!(output_of(&game, depot, &frag()), 0);
    assert!(output_of(&game, node, &frag()) >= 10);
}

#[test]
fn fragments_reach_a_lathe_through_a_depot_with_no_carrier() {
    let mut game = Game::new(9014, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    node(&mut game, 40, 40, 100, true);
    let depot = depot_for_fragments(&mut game, 41, 40);
    game.set_depot_filter(depot, &[ItemId::from("blank_substrate")], true);
    let lathe = machine(&mut game, "lathe", 42, 40);
    post(&mut game, lathe);

    for _ in 0..10 {
        game.tick();
    }

    let fed = game.world.get::<Stock>(lathe).unwrap();
    assert!(
        fed.input.get(&frag()).copied().unwrap_or(0) > 0 || !fed.output.is_empty(),
        "the Lathe drew fragments out of the Depot the node filled"
    );
}

// ---- the drawn join ---------------------------------------------------

fn edges_of(game: &mut Game, structure: Entity) -> Vec<(i32, i32)> {
    let mut edges = game
        .linked_edges_by_structure()
        .remove(&structure)
        .unwrap_or_default();
    edges.sort();
    edges
}

#[test]
fn a_depot_is_drawn_joined_to_a_producer_whose_output_it_accepts() {
    let mut game = Game::new(9030, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = node(&mut game, 40, 40, 0, false);
    let depot = depot_for_fragments(&mut game, 41, 40);

    assert_eq!(edges_of(&mut game, depot), vec![(-1, 0)]);
    assert_eq!(edges_of(&mut game, node), vec![(1, 0)]);
}

#[test]
fn a_depot_that_refuses_the_output_is_not_drawn_joined() {
    let mut game = Game::new(9031, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = node(&mut game, 40, 40, 0, false);
    let depot = depot_for_fragments(&mut game, 41, 40);
    game.set_depot_filter(depot, &[frag()], false);

    assert!(edges_of(&mut game, depot).is_empty());
    assert!(edges_of(&mut game, node).is_empty());
}

#[test]
fn a_depot_is_drawn_joined_to_a_consumer_whose_ingredient_it_accepts() {
    let mut game = Game::new(9032, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = node(&mut game, 40, 40, 0, false);
    let depot = depot_for_fragments(&mut game, 41, 40);
    game.set_depot_filter(depot, &[ItemId::from("blank_substrate")], false);
    let lathe = machine(&mut game, "lathe", 42, 40);

    assert_eq!(edges_of(&mut game, depot), vec![(-1, 0), (1, 0)]);
    assert_eq!(edges_of(&mut game, node), vec![(1, 0)]);
    assert_eq!(edges_of(&mut game, lathe), vec![(-1, 0)]);
}

#[test]
fn a_depot_that_refuses_everything_is_not_joined_to_a_consumer() {
    let mut game = Game::new(9033, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let depot = depot_for_fragments(&mut game, 41, 40);
    game.set_all_depot_filters(depot, false);
    let lathe = machine(&mut game, "lathe", 42, 40);

    assert!(edges_of(&mut game, depot).is_empty());
    assert!(edges_of(&mut game, lathe).is_empty());
}
