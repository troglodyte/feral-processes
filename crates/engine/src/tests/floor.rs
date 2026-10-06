//! Floor piles: where an interrupted carrier's load goes.

use super::support::*;
use crate::components::{Carrying, FloorPile};
use crate::floors::FloorId;
use crate::game::base::floor::{drop_load, floor_pile_at, spawn_floor_pile, take_from_pile};
use crate::*;

fn game() -> Game {
    Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn carrier(game: &mut Game, x: i32, y: i32, item: &str, qty: u32) -> Entity {
    game.world
        .spawn((
            Position { x, y },
            Carrying {
                item: ItemId::from(item),
                qty,
            },
        ))
        .id()
}

fn pile_items(game: &mut Game, x: i32, y: i32) -> Vec<(ItemId, u32)> {
    let e = floor_pile_at(&mut game.world, Position { x, y }).expect("a pile");
    game.world
        .get::<FloorPile>(e)
        .unwrap()
        .items
        .iter()
        .map(|(i, n)| (i.clone(), *n))
        .collect()
}

fn pile_count(game: &mut Game) -> usize {
    game.world.query::<&FloorPile>().iter(&game.world).count()
}

#[test]
fn drop_makes_a_pile_on_the_carriers_tile_and_ends_the_hold() {
    let mut g = game();
    let who = carrier(&mut g, 3, 4, ids::CORE_FRAGMENT, 5);
    drop_load(&mut g.world, who);
    assert!(g.world.get::<Carrying>(who).is_none());
    assert_eq!(
        pile_items(&mut g, 3, 4),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]
    );
}

#[test]
fn two_drops_on_one_tile_merge_into_one_pile() {
    let mut g = game();
    let a = carrier(&mut g, 1, 1, ids::CORE_FRAGMENT, 2);
    let b = carrier(&mut g, 1, 1, ids::CORE_FRAGMENT, 3);
    drop_load(&mut g.world, a);
    drop_load(&mut g.world, b);
    assert_eq!(pile_count(&mut g), 1);
    assert_eq!(
        pile_items(&mut g, 1, 1),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 5)]
    );
}

#[test]
fn drop_with_no_load_is_a_no_op() {
    let mut g = game();
    let who = g.world.spawn(Position { x: 0, y: 0 }).id();
    drop_load(&mut g.world, who);
    assert_eq!(pile_count(&mut g), 0);
}

#[test]
fn take_from_pile_takes_partial_then_whole_and_despawns() {
    let mut g = game();
    let who = carrier(&mut g, 2, 2, ids::CORE_FRAGMENT, 5);
    drop_load(&mut g.world, who);
    let pile = floor_pile_at(&mut g.world, Position { x: 2, y: 2 }).unwrap();
    let item = ItemId::from(ids::CORE_FRAGMENT);
    assert_eq!(take_from_pile(&mut g.world, pile, &item, 2), 2);
    assert_eq!(pile_items(&mut g, 2, 2), vec![(item.clone(), 3)]);
    assert_eq!(take_from_pile(&mut g.world, pile, &item, 99), 3);
    assert!(g.world.get_entity(pile).is_err(), "emptied pile despawns");
}

#[test]
fn take_from_a_missing_pile_returns_zero() {
    let mut g = game();
    let who = carrier(&mut g, 2, 2, ids::CORE_FRAGMENT, 1);
    drop_load(&mut g.world, who);
    let pile = floor_pile_at(&mut g.world, Position { x: 2, y: 2 }).unwrap();
    let item = ItemId::from(ids::CORE_FRAGMENT);
    take_from_pile(&mut g.world, pile, &item, 1);
    assert_eq!(take_from_pile(&mut g.world, pile, &item, 1), 0);
}

#[test]
fn a_pile_survives_save_and_load() {
    let mut g = game();
    let who = carrier(&mut g, -3, 7, ids::CORE_FRAGMENT, 4);
    drop_load(&mut g.world, who);
    let dir = std::env::temp_dir().join("feral-floor-pile-save");
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("floor.bin");
    g.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    assert_eq!(pile_count(&mut loaded), 1);
    assert_eq!(
        pile_items(&mut loaded, -3, 7),
        vec![(ItemId::from(ids::CORE_FRAGMENT), 4)]
    );
}

#[test]
fn a_save_without_the_field_loads_with_no_piles() {
    let data: crate::save::SaveData = {
        let mut g = game();
        let dir = std::env::temp_dir().join("feral-floor-pile-old");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.bin");
        g.save(&path).unwrap();
        crate::save::load_from_file(&path).unwrap()
    };
    let text = ron::to_string(&data).unwrap();
    let stripped = text.replacen("floor_piles:[],", "", 1);
    assert_ne!(text, stripped, "the field is serialised by name");
    let parsed: crate::save::SaveData = ron::from_str(&stripped).unwrap();
    assert!(parsed.floor_piles.is_empty());
}

// ---------------------------------------------------------------------------
// The paths that used to destroy a carrier's load. Each leaves a pile on the
// carrier's tile and no `Carrying`.
// ---------------------------------------------------------------------------

use crate::components::{Downed, Durability, Task, TaskKind};

const LOAD: u32 = 3;

fn load() -> Carrying {
    Carrying {
        item: ItemId::from(ids::CORE_FRAGMENT),
        qty: LOAD,
    }
}

/// A staff program on its own tile, holding a load.
fn loaded_staff(game: &mut Game, x: i32, y: i32) -> Entity {
    let worker = spawn_tamed(game, 100, 3);
    game.world
        .entity_mut(worker)
        .insert((Position { x, y }, load()));
    worker
}

fn assert_dropped(game: &mut Game, worker: Entity, x: i32, y: i32) {
    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "still holding the load"
    );
    assert_eq!(
        pile_items(game, x, y),
        vec![(ItemId::from(ids::CORE_FRAGMENT), LOAD)]
    );
}

fn posted_carrier(game: &mut Game) -> (Entity, Entity) {
    stand_in_base(game);
    place_home(game);
    let node = spawn_machine_at(game, "mining_node", 2, 0);
    let worker = loaded_staff(game, 6, 6);
    game.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: node,
        progress: 0,
        required: 10,
    });
    (node, worker)
}

#[test]
fn a_demolished_post_drops_its_carriers_load() {
    let mut g = game();
    let (node, worker) = posted_carrier(&mut g);
    g.remove_structure(node).unwrap();
    assert!(g.world.get::<Task>(worker).is_none());
    assert_dropped(&mut g, worker, 6, 6);
}

#[test]
fn a_destroyed_post_drops_its_carriers_load() {
    let mut g = game();
    let (node, worker) = posted_carrier(&mut g);
    g.world
        .entity_mut(node)
        .insert(Durability { hp: 10, max_hp: 10 });
    g.damage_structure(node, 100_000, "Mining Node", "a GC Entropy Sweep");
    assert!(g.world.get::<Task>(worker).is_none());
    assert_dropped(&mut g, worker, 6, 6);
}

#[test]
fn a_siphon_lock_in_drops_the_load() {
    let mut g = game();
    stand_in_base(&mut g);
    let siphon = spawn_structure_at(&mut g, "power_siphon", 3, 3);
    let worker = loaded_staff(&mut g, 6, 6);
    g.siphon_program(worker, siphon).unwrap();
    assert_dropped(&mut g, worker, 6, 6);
}

#[test]
fn pinning_for_study_drops_the_load() {
    let mut g = game();
    stand_in_base(&mut g);
    place_home(&mut g);
    give(&mut g, &ItemId::from(ids::CORE_FRAGMENT), 50);
    place_now(&mut g, "research_node", 1, -3).unwrap();
    let station = g.find_blocking_structure_at(1, -3).unwrap();
    let worker = spawn_tamed(&mut g, 100, 3);
    g.world.entity_mut(worker).insert(load());
    let at = *g.world.get::<Position>(worker).unwrap();
    g.pin_subject(worker, station).unwrap();
    assert_dropped(&mut g, worker, at.x, at.y);
}

/// A staff body at a mine under a real work order, holding a load.
fn ordered_carrier(game: &mut Game) -> Entity {
    stand_in_base(game);
    place_home(game);
    spawn_machine_at(game, "mining_node", 2, 0);
    let worker = spawn_tamed(game, 100, 3);
    game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 50))
        .unwrap();
    game.tick();
    assert!(
        game.world.get::<Task>(worker).is_some(),
        "precondition: posted by the scheduler"
    );
    game.world.entity_mut(worker).insert(load());
    worker
}

#[test]
fn freeing_a_downed_carrier_drops_the_load() {
    let mut g = game();
    let worker = ordered_carrier(&mut g);
    let at = *g.world.get::<Position>(worker).unwrap();
    g.world.entity_mut(worker).insert(Downed);
    g.tick();
    assert!(g.world.get::<Task>(worker).is_none());
    assert_dropped(&mut g, worker, at.x, at.y);
}

fn put_pile(game: &mut Game, x: i32, y: i32, item: &str, qty: u32) {
    spawn_floor_pile(&mut game.world, Position { x, y }, ItemId::from(item), qty);
}

fn name_of(game: &Game, item: &str) -> String {
    game.world
        .resource::<crate::items_db::ItemDb>()
        .get(item)
        .expect("item exists")
        .name
        .clone()
}

#[test]
fn a_pile_in_base_space_has_a_view_row_naming_its_contents() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    put_pile(&mut g, 2, 1, ids::CORE_FRAGMENT, 3);
    let rows = g.floor_piles();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].pos, (2, 1));
    assert_eq!(rows[0].items, vec![(name_of(&g, ids::CORE_FRAGMENT), 3)]);
}

#[test]
fn a_pile_is_not_a_view_row_on_a_zone_surface() {
    let mut g = game();
    put_pile(&mut g, 2, 1, ids::CORE_FRAGMENT, 3);
    assert!(g.floor_piles().is_empty());
}

#[test]
fn examining_a_pile_names_what_it_holds() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    g.world
        .resource_mut::<base_grid::BaseGrid>()
        .lay_floor(1, 0);
    put_pile(&mut g, 1, 0, ids::CORE_FRAGMENT, 3);
    let line = g.describe_base_rock(1, 0, 5).expect("a pile answers");
    let name = name_of(&g, ids::CORE_FRAGMENT);
    assert!(
        line.starts_with(&format!("A pile on the floor: 3 {name}")),
        "{line}"
    );
}

#[test]
fn examining_a_pile_on_a_finish_names_both() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    {
        let mut grid = g.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(1, 0);
        assert!(grid.set_finish(1, 0, FloorId::from("cobalt_carpet")));
    }
    put_pile(&mut g, 1, 0, ids::CORE_FRAGMENT, 3);
    let line = g.describe_base_rock(1, 0, 5).unwrap();
    assert!(line.contains("Cobalt Carpet underfoot"), "{line}");
    assert!(line.contains("; a pile on the floor: 3 "), "{line}");
}

/// A walled one-cell room on the real grid with one door and a bed in it, on the
/// commons' east side.
fn walled_room_with_a_bed(game: &mut Game) {
    for i in 1..=3 {
        for (x, y) in [(i, 1), (i, 3), (1, i), (3, i)] {
            let kind = if (x, y) == (2, 1) { "door" } else { "wall" };
            spawn_structure_at(game, kind, x, y);
        }
    }
    spawn_structure_at(game, "defrag_bay", 2, 2);
}

fn game_with_home() -> Game {
    let mut game = game();
    place_home(&mut game);
    stand_in_base(&mut game);
    game
}

#[test]
fn view_rooms_lists_the_walled_room_by_role_and_not_the_commons() {
    let mut game = game_with_home();
    walled_room_with_a_bed(&mut game);
    let rooms = game.view_rooms();
    assert_eq!(rooms.len(), 1, "{rooms:?}");
    let room = &rooms[0];
    assert_eq!(room.role, "quarters");
    assert_eq!(room.cells, vec![(2, 2)]);
    assert!(room.cells.contains(&room.label_at));
    assert_eq!(room.tint, (90, 140, 220));
}

#[test]
fn the_report_names_the_room_a_structure_stands_in_and_not_the_commons() {
    let mut game = game_with_home();
    walled_room_with_a_bed(&mut game);
    let report = game.structure_report();
    let at = |p: (i32, i32)| report.iter().find(|r| r.pos == p).unwrap();
    let bay = at((2, 2)).room.as_deref().expect("the bay is in a room");
    assert!(bay.ends_with("quarters"), "{bay}");
    assert_eq!(at((0, 0)).room, None, "the home sits in the commons");
}

/// Ticks for one cycle at a lathe standing in a walled room with
/// `finished` carpet cells, `with_bed` making it quarters rather than a
/// workshop; `None` leaves the lathe in the commons.
fn lathe_ticks(room: Option<(usize, bool)>) -> u32 {
    let mut game = game_with_home();
    if let Some((finished, with_bed)) = room {
        walled_room_with_finish(&mut game, finished);
        if with_bed {
            spawn_structure_at(&mut game, "defrag_bay", 4, 4);
        }
    }
    let lathe = spawn_structure_at(&mut game, "lathe", 3, 3);
    game.work_ticks_for(lathe, crate::tuning::DEFAULT_BASE_SPEED)
}

#[test]
fn a_fine_workshop_works_faster_than_the_commons() {
    let commons = lathe_ticks(None);
    let fine = lathe_ticks(Some((8, false)));
    assert_eq!(commons, 12);
    assert_eq!(fine, 11, "12 ticks at the Fine scale, rounded");
}

#[test]
fn a_machine_in_quarters_gets_no_workshop_scale() {
    assert_eq!(lathe_ticks(Some((8, true))), lathe_ticks(None));
}
