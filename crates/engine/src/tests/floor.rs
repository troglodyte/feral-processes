//! Floor items: where an interrupted carrier's load goes.

use super::support::*;
use crate::components::{Carrying, FloorItem};
use crate::floors::FloorId;
use crate::game::base::floor::{nearest_free_cells, take_floor_unit};
use crate::items::FloorThing;
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

fn core_unit() -> FloorThing {
    FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT))
}

#[test]
fn a_dropped_load_lands_one_unit_per_tile_and_ends_the_hold() {
    let mut g = game_with_home();
    let who = loaded_staff(&mut g, 6, 6);
    g.drop_load(who);
    assert!(g.world.get::<Carrying>(who).is_none());
    let things = floor_things(&mut g);
    assert_eq!(things.len(), LOAD as usize);
    assert!(
        things.iter().any(|(c, _)| *c == (6, 6)),
        "its own tile first"
    );
    assert!(things.iter().all(|(_, t)| *t == core_unit()));
}

#[test]
fn two_drops_on_one_tile_spread_instead_of_merging() {
    let mut g = game();
    lay_floor_box(&mut g, (0, 0), (4, 4));
    let a = carrier(&mut g, 1, 1, ids::CORE_FRAGMENT, 2);
    let b = carrier(&mut g, 1, 1, ids::CORE_FRAGMENT, 3);
    g.drop_load(a);
    g.drop_load(b);
    let cells: std::collections::HashSet<_> =
        floor_things(&mut g).into_iter().map(|(c, _)| c).collect();
    assert_eq!(cells.len(), 5, "one unit per tile");
}

#[test]
fn drop_with_no_load_is_a_no_op() {
    let mut g = game();
    let who = g.world.spawn(Position { x: 0, y: 0 }).id();
    g.drop_load(who);
    assert!(floor_things(&mut g).is_empty());
}

#[test]
fn take_floor_unit_takes_a_plain_item_and_despawns_it() {
    let mut g = game();
    lay_floor_box(&mut g, (2, 2), (2, 2));
    g.set_down(Position { x: 2, y: 2 }, vec![core_unit()]);
    let e = crate::game::base::floor::floor_item_at(&mut g.world, Position { x: 2, y: 2 }).unwrap();
    let item = ItemId::from(ids::CORE_FRAGMENT);
    assert!(take_floor_unit(&mut g.world, e, &item));
    assert!(g.world.get_entity(e).is_err());
    assert!(!take_floor_unit(&mut g.world, e, &item), "already gone");
}

#[test]
fn take_floor_unit_leaves_gear_and_programs_on_their_tile() {
    let mut g = game();
    lay_floor_box(&mut g, (0, 0), (1, 0));
    let gear = FloorThing::Gear(crate::items::GearCopy::plain(ItemId::from(
        ids::MONOFILAMENT_WHIP,
    )));
    let program = FloorThing::Program(crate::items::DownedProgram {
        species: "scrapper".to_string(),
        level: 3,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    });
    g.set_down(Position { x: 0, y: 0 }, vec![gear, program]);
    let rows: Vec<_> = g
        .world
        .query::<(Entity, &FloorItem)>()
        .iter(&g.world)
        .map(|(e, _)| e)
        .collect();
    assert_eq!(rows.len(), 2);
    for e in rows {
        for id in [ids::MONOFILAMENT_WHIP, ids::CORE_FRAGMENT] {
            assert!(!take_floor_unit(&mut g.world, e, &ItemId::from(id)));
        }
    }
    assert_eq!(floor_things(&mut g).len(), 2, "both still lie there");
}

fn save_and_load(g: &mut Game, name: &str) -> Game {
    let dir = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("floor.bin");
    g.save(&path).unwrap();
    Game::load(&path, &test_assets_dir()).unwrap()
}

fn a_gear_copy() -> FloorThing {
    FloorThing::Gear(crate::items::GearCopy::plain(ItemId::from(
        ids::MONOFILAMENT_WHIP,
    )))
}

fn a_downed_program() -> FloorThing {
    FloorThing::Program(crate::items::DownedProgram {
        species: "scrapper".to_string(),
        level: 3,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    })
}

#[test]
fn floor_items_of_every_kind_survive_save_and_load() {
    let mut g = game_with_home();
    lay_floor_box(&mut g, (10, 10), (12, 10));
    for (x, thing) in [
        (10, core_unit()),
        (11, a_gear_copy()),
        (12, a_downed_program()),
    ] {
        g.set_down(Position { x, y: 10 }, vec![thing]);
    }
    let before = floor_things(&mut g);
    assert_eq!(before.len(), 3);
    let mut loaded = save_and_load(&mut g, "feral-floor-items-save");
    assert_eq!(floor_things(&mut loaded), before);
}

/// Loads `g` after splicing an old-format `floor_piles` field into its save:
/// the field is load-only now, so the writer cannot produce it.
fn legacy_pile_save(g: &mut Game, name: &str, pile_ron: &str) -> Game {
    let dir = std::env::temp_dir().join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("old.bin");
    g.save(&path).unwrap();
    let text = std::fs::read_to_string(&path).unwrap();
    assert!(!text.contains("floor_piles"), "never written any more");
    let spliced = text.replacen(
        "floor_items:",
        &format!("floor_piles: [{pile_ron}],\nfloor_items:"),
        1,
    );
    assert_ne!(text, spliced, "the field is serialised by name");
    std::fs::write(&path, spliced).unwrap();
    Game::load(&path, &test_assets_dir()).unwrap()
}

#[test]
fn an_old_pile_spreads_one_unit_per_tile_on_load() {
    let mut g = game_with_home();
    let pile = format!(
        r#"(position: (6, 6), items: [("{}", 4)])"#,
        ids::CORE_FRAGMENT
    );
    let mut loaded = legacy_pile_save(&mut g, "feral-floor-old-pile", &pile);
    let things = floor_things(&mut loaded);
    assert_eq!(things.len(), 4);
    let cells: std::collections::HashSet<_> = things.iter().map(|(c, _)| *c).collect();
    assert_eq!(cells.len(), 4, "distinct cells");
    assert!(things.iter().all(|(_, t)| *t == core_unit()));
}

#[test]
fn an_old_pile_too_big_for_its_pocket_spills_into_the_pack_on_load() {
    let mut g = game();
    lay_floor_box(&mut g, (0, 0), (1, 0));
    let before = held(&g, &ItemId::from(ids::CORE_FRAGMENT));
    let pile = format!(
        r#"(position: (0, 0), items: [("{}", 10)])"#,
        ids::CORE_FRAGMENT
    );
    let mut loaded = legacy_pile_save(&mut g, "feral-floor-old-pile-big", &pile);
    assert_eq!(floor_things(&mut loaded).len(), 2);
    assert_eq!(held(&loaded, &ItemId::from(ids::CORE_FRAGMENT)), before + 8);
}

#[test]
fn a_save_without_floor_items_loads_with_none() {
    let data: crate::save::SaveData = {
        let mut g = game();
        let dir = std::env::temp_dir().join("feral-floor-items-old");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("old.bin");
        g.save(&path).unwrap();
        crate::save::load_from_file(&path).unwrap()
    };
    let text = ron::to_string(&data).unwrap();
    let stripped = text.replacen("floor_items:[],", "", 1);
    assert_ne!(text, stripped, "the field is serialised by name");
    let parsed: crate::save::SaveData = ron::from_str(&stripped).unwrap();
    assert!(parsed.floor_items.is_empty());
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
    floor_the_pocket(game);
    let worker = spawn_tamed(game, 100, 3);
    game.world
        .entity_mut(worker)
        .insert((Position { x, y }, load()));
    worker
}

/// A builder re-posted from a stranded haul keeps its stale `Stranded`; the
/// load it then lifts for a site must not be set down by the stranded pass.
#[test]
fn a_builder_with_a_stale_stranded_keeps_its_load() {
    let mut g = game_with_home();
    let worker = loaded_staff(&mut g, 6, 6);
    g.world
        .entity_mut(worker)
        .insert(crate::components::Stranded { since: 0 });
    for _ in 0..crate::tuning::STRANDED_SET_DOWN_TICKS + 5 {
        g.tick();
    }
    g.set_down_stranded_loads();
    assert!(g.world.get::<Carrying>(worker).is_some());
    assert!(floor_things(&mut g).is_empty());
}

fn assert_dropped(game: &mut Game, worker: Entity, x: i32, y: i32) {
    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "still holding the load"
    );
    let things = floor_things(game);
    assert_eq!(things.len(), LOAD as usize);
    assert!(things.iter().all(|(_, t)| *t == core_unit()));
    assert!(
        things.iter().any(|(c, _)| *c == (x, y)),
        "its own tile first"
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
    floor_the_pocket(&mut g);
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
    floor_the_pocket(game);
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

fn put_item(game: &mut Game, x: i32, y: i32, thing: FloorThing) {
    lay_floor_box(game, (x, y), (x, y));
    game.set_down(Position { x, y }, vec![thing]);
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
fn a_floor_item_in_base_space_has_a_view_row_naming_it() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    put_item(&mut g, 2, 1, core_unit());
    let rows = g.floor_items();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].pos, (2, 1));
    assert_eq!(rows[0].name, name_of(&g, ids::CORE_FRAGMENT));
    assert!(matches!(rows[0].look, crate::views::FloorLook::Item(_)));
}

#[test]
fn a_floor_item_is_not_a_view_row_on_a_zone_surface() {
    let mut g = game();
    put_item(&mut g, 2, 1, core_unit());
    assert!(g.floor_items().is_empty());
}

#[test]
fn examining_a_floor_item_names_it() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    put_item(&mut g, 1, 0, core_unit());
    let line = g.describe_base_rock(1, 0, 5).expect("a floor item answers");
    let name = name_of(&g, ids::CORE_FRAGMENT);
    assert!(line.starts_with(&format!("{name} on the floor")), "{line}");
}

#[test]
fn examining_a_downed_program_on_the_floor_names_it() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    put_item(&mut g, 1, 0, a_downed_program());
    let line = g.describe_base_rock(1, 0, 5).expect("a floor item answers");
    assert!(line.contains(", downed, on the floor"), "{line}");
}

#[test]
fn examining_a_floor_item_on_a_finish_names_both() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    {
        let mut grid = g.world.resource_mut::<base_grid::BaseGrid>();
        grid.lay_floor(1, 0);
        assert!(grid.set_finish(1, 0, FloorId::from("cobalt_carpet")));
    }
    g.set_down(Position { x: 1, y: 0 }, vec![core_unit()]);
    let line = g.describe_base_rock(1, 0, 5).unwrap();
    assert!(line.contains("Cobalt Carpet underfoot"), "{line}");
    assert!(
        line.contains("; Core Fragment on the floor")
            || line.contains("; ") && line.contains(" on the floor"),
        "{line}"
    );
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

/// Floor under the whole starting pocket: `open` cells are walkable but a
/// set-down only lands on laid floor.
fn floor_the_pocket(game: &mut Game) {
    lay_floor_box(game, (-10, -10), (10, 10));
}

fn game_with_home() -> Game {
    let mut game = game();
    place_home(&mut game);
    stand_in_base(&mut game);
    floor_the_pocket(&mut game);
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

#[test]
fn nearest_free_cells_takes_the_drop_point_first_then_walks_outward() {
    let corridor = |c: (i32, i32)| c.1 == 0 && (0..5).contains(&c.0);
    assert_eq!(
        nearest_free_cells((2, 0), 3, corridor, corridor),
        vec![(2, 0), (1, 0), (3, 0)]
    );
}

#[test]
fn nearest_free_cells_never_reaches_past_what_cannot_be_crossed() {
    let free = |c: (i32, i32)| c == (2, 0) || c == (0, 2);
    let crossable = |c: (i32, i32)| c.0 == 0 && (0..=2).contains(&c.1);
    assert_eq!(nearest_free_cells((0, 0), 2, free, crossable), vec![(0, 2)]);
}

fn scrap() -> FloorThing {
    FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT))
}

#[test]
fn set_down_spreads_one_unit_per_tile_nearest_first() {
    let mut g = game();
    lay_floor_box(&mut g, (0, 0), (4, 0));
    g.set_down(Position { x: 2, y: 0 }, vec![scrap(); 3]);
    let cells: Vec<_> = floor_things(&mut g).into_iter().map(|(c, _)| c).collect();
    assert_eq!(cells, vec![(1, 0), (2, 0), (3, 0)]);
}

#[test]
fn set_down_skips_a_tile_that_already_holds_an_item_or_a_structure() {
    let mut g = game();
    let before = held(&g, &ItemId::from(ids::CORE_FRAGMENT));
    lay_floor_box(&mut g, (0, 0), (3, 0));
    g.set_down(Position { x: 0, y: 0 }, vec![scrap()]);
    spawn_structure_at(&mut g, "wall", 1, 0); // anchors a wall: not free and not crossable
    g.set_down(Position { x: 0, y: 0 }, vec![scrap()]);
    assert_eq!(
        floor_things(&mut g).len(),
        1,
        "nothing past the wall is reachable"
    );
    assert_eq!(held(&g, &ItemId::from(ids::CORE_FRAGMENT)), before + 1);
}

#[test]
fn a_drop_beside_a_wall_never_lands_behind_it() {
    let mut g = game();
    let before = held(&g, &ItemId::from(ids::CORE_FRAGMENT));
    // Walls stand on laid floor, so only the wall check keeps the far side out.
    lay_floor_box(&mut g, (0, 0), (2, 1));
    spawn_structure_at(&mut g, "wall", 1, 0);
    spawn_structure_at(&mut g, "wall", 1, 1);
    g.set_down(Position { x: 0, y: 0 }, vec![scrap(); 3]);
    let cells: Vec<_> = floor_things(&mut g).into_iter().map(|(c, _)| c).collect();
    assert_eq!(cells, vec![(0, 0), (0, 1)], "(2,0) is behind the wall");
    assert_eq!(held(&g, &ItemId::from(ids::CORE_FRAGMENT)), before + 1);
}

#[test]
fn set_down_with_no_free_floor_puts_items_in_the_pack_and_says_so() {
    let mut g = game(); // empty BaseGrid: nothing is floor
    let before = held(&g, &ItemId::from(ids::CORE_FRAGMENT));
    g.set_down(Position { x: 5, y: 5 }, vec![scrap(); 2]);
    assert_eq!(held(&g, &ItemId::from(ids::CORE_FRAGMENT)), before + 2);
    assert!(
        g.message_history(5)
            .iter()
            .any(|e| e.text.contains("goes into your pack"))
    );
}

#[test]
fn a_program_with_no_floor_goes_to_the_downed_store_then_a_rack() {
    use crate::components::{DownedPrograms, Racked};
    let mut g = game();
    let player = g.player_entity();
    let program = || crate::items::DownedProgram {
        species: "scrapper".to_string(),
        level: 3,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 70,
        carried: None,
    };
    let rack = spawn_structure_at(&mut g, "quarantine_rack", 9, 9);
    g.world.entity_mut(rack).insert(Racked::default());
    let at = Position { x: 5, y: 5 };

    g.set_down(at, vec![FloorThing::Program(program())]);
    assert_eq!(g.world.get::<DownedPrograms>(player).unwrap().0.len(), 1);
    assert!(g.world.get::<Racked>(rack).unwrap().0.is_empty());

    g.world.get_mut::<DownedPrograms>(player).unwrap().0 =
        vec![program(); crate::tuning::MAX_DOWNED_PROGRAMS];
    g.set_down(at, vec![FloorThing::Program(program())]);
    assert_eq!(g.world.get::<Racked>(rack).unwrap().0.len(), 1);
    assert_eq!(
        g.world.get::<DownedPrograms>(player).unwrap().0.len(),
        crate::tuning::MAX_DOWNED_PROGRAMS
    );
    assert!(
        g.message_history(5)
            .iter()
            .any(|e| e.text.contains("onto the Quarantine Rack"))
    );
}
