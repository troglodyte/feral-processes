//! `Game::adjacent_structures`: which structures beside the party open the
//! hub, in what order, and with which pages.

use super::support::*;
use crate::StructurePage::*;
use crate::*;

fn with_shelf(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    let e = spawn_structure_at(game, kind, x, y);
    game.world.entity_mut(e).insert(Stock::new(50));
    e
}

fn fresh() -> Game {
    let mut game = Game::new(1740, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base_at(&mut game, 0, 0);
    game
}

#[test]
fn a_machine_north_and_a_depot_east_cycle_machine_first() {
    let mut game = fresh();
    let depot = with_shelf(&mut game, "depot", 1, 0);
    let machine = with_shelf(&mut game, "extractor", 0, -1);

    let found = game.adjacent_structures();
    assert_eq!(
        found.iter().map(|s| s.entity).collect::<Vec<_>>(),
        vec![machine, depot]
    );
    assert_eq!(found[0].dir, (0, -1));
    assert_eq!(found[0].pages, vec![Inventory, Recipe, Info]);
    assert_eq!(found[1].dir, (1, 0));
    assert_eq!(found[1].pages, vec![Inventory, Filter, Info]);
}

#[test]
fn the_cycle_runs_north_east_south_west() {
    let mut game = fresh();
    let w = with_shelf(&mut game, "depot", -1, 0);
    let s = with_shelf(&mut game, "depot", 0, 1);
    let e = with_shelf(&mut game, "depot", 1, 0);
    let n = with_shelf(&mut game, "depot", 0, -1);
    let order: Vec<_> = game
        .adjacent_structures()
        .into_iter()
        .map(|a| a.entity)
        .collect();
    assert_eq!(order, vec![n, e, s, w]);
}

#[test]
fn a_wide_structure_touching_two_sides_appears_once_at_the_first() {
    let mut game = fresh();
    // A 2x2 node anchored at (0,-1) covers (0,-1),(1,-1),(0,0),(1,0): both
    // the north and the east neighbour (it overlaps the party's own tile,
    // the only way a 2x2 can touch two orthogonal sides).
    let node = with_shelf(&mut game, "research_node", 0, -1);
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 1);
    assert_eq!(game.structure_footprint(&"research_node".to_string()), 2);
    let found = game.adjacent_structures();
    assert_eq!(found.len(), 1);
    assert_eq!(found[0].entity, node);
    assert_eq!(found[0].dir, (0, -1));
}

#[test]
fn a_wall_beside_you_is_not_in_the_cycle() {
    let mut game = fresh();
    spawn_structure_at(&mut game, "wall", 1, 0);
    assert!(game.adjacent_structures().is_empty());
}

#[test]
fn a_siphon_offers_its_page_and_a_terminal_its_inventory() {
    let mut game = fresh();
    let siphon = spawn_structure_at(&mut game, "power_siphon", 1, 0);
    let terminal = spawn_structure_at(&mut game, "storage_terminal", 0, 1);
    let found = game.adjacent_structures();
    assert_eq!(found.len(), 2);
    assert_eq!(
        (found[0].entity, found[0].pages.clone()),
        (siphon, vec![Siphon, Info])
    );
    assert_eq!(
        (found[1].entity, found[1].pages.clone()),
        (terminal, vec![Inventory, Info])
    );
}

#[test]
fn an_empty_teardown_rig_has_only_its_tool_and_a_loaded_one_an_inventory() {
    let mut game = fresh();
    let rig = with_shelf(&mut game, "teardown_rig", 1, 0);
    assert_eq!(game.adjacent_structures()[0].pages, vec![Tool, Info]);
    game.world
        .get_mut::<Stock>(rig)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 3);
    assert_eq!(
        game.adjacent_structures()[0].pages,
        vec![Inventory, Tool, Info]
    );
}

#[test]
fn nothing_is_offered_in_a_battle_or_outside_the_base() {
    let mut game = fresh();
    with_shelf(&mut game, "depot", 1, 0);
    assert_eq!(game.adjacent_structures().len(), 1);
    game.world.insert_resource(Locale::Surface);
    assert!(game.adjacent_structures().is_empty());
}
