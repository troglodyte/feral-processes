//! Stockpile zones: a standing mark on laid floor, drawn with the Excavation
//! box but never part of the dig plan.

use super::support::*;
use crate::base_grid::BaseGrid;
use crate::items::FloorThing;
use crate::*;

fn game() -> Game {
    Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn in_base(g: &mut Game) {
    stand_in_base_at(g, 0, 0);
}

#[test]
fn a_box_marks_only_laid_floor_as_stockpile() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (1, 0));
    g.toggle_stockpile_box((0, 0), (2, 0));
    assert_eq!(g.stockpile_cells(), vec![(0, 0), (1, 0)]);
}

#[test]
fn a_box_anchored_on_a_stockpile_clears_it() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (2, 0));
    g.toggle_stockpile_box((0, 0), (2, 0));
    assert_eq!(g.stockpile_cells().len(), 3);
    g.toggle_stockpile_box((1, 0), (2, 0));
    assert_eq!(g.stockpile_cells(), vec![(0, 0)]);
    g.toggle_stockpile_box((2, 0), (0, 0)); // anchor unmarked: marks again
    assert_eq!(g.stockpile_cells().len(), 3);
    g.toggle_stockpile_box((0, 0), (2, 0));
    assert!(g.stockpile_cells().is_empty());
}

#[test]
fn stockpile_marks_and_dig_marks_are_independent() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (0, 0));
    g.toggle_mark_box(
        (0, 0),
        (0, 0),
        Some(&FinishOrder::Apply(crate::floors::FloorId::from(
            "cobalt_carpet",
        ))),
    );
    g.toggle_stockpile_box((0, 0), (0, 0));
    let dig = g.marked_cells();
    assert!(!dig.is_empty(), "premise: the cell carries a dig mark");
    assert_eq!(g.stockpile_cells(), vec![(0, 0)]);
    g.toggle_stockpile_box((0, 0), (0, 0));
    assert!(g.stockpile_cells().is_empty());
    assert_eq!(g.marked_cells(), dig);
}

#[test]
fn stockpile_cells_survive_save_and_load() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (2, 1));
    g.toggle_stockpile_box((0, 0), (1, 1));
    let before = g.stockpile_cells();
    assert_eq!(before.len(), 4);
    let path = std::env::temp_dir().join(format!(
        "feral_processes_stockpile_round_trip_{}.sav",
        std::process::id()
    ));
    g.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let grid = loaded.world.resource::<BaseGrid>();
    let after: Vec<_> = grid.stockpile_cells().collect();
    assert_eq!(after, before);
}

#[test]
fn examining_a_stockpile_says_so() {
    let mut g = game();
    stand_in_base_at(&mut g, 0, 0);
    lay_floor_box(&mut g, (1, 0), (1, 0));
    g.toggle_stockpile_box((1, 0), (1, 0));
    let line = g.describe_base_rock(1, 0, 5).expect("a stockpile answers");
    assert!(line.to_lowercase().contains("a stockpile"), "{line}");
}

#[test]
fn a_stockpile_cell_built_over_stays_marked_but_takes_nothing() {
    let mut g = game();
    in_base(&mut g);
    lay_floor_box(&mut g, (0, 0), (2, 0));
    g.toggle_stockpile_box((1, 0), (1, 0));
    spawn_structure_at(&mut g, "wall", 1, 0);
    assert!(g.world.resource::<BaseGrid>().is_stockpile(1, 0));
    g.set_down(
        Position { x: 1, y: 0 },
        vec![FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT))],
    );
    assert!(
        floor_things(&mut g).iter().all(|(c, _)| *c != (1, 0)),
        "{:?}",
        floor_things(&mut g)
    );
}

// ---------------------------------------------------------------------------
// Tidying: an idle program carries loose floor items onto stockpile cells.
// ---------------------------------------------------------------------------

use crate::components::{Downed, FloorItem, OffShift, ProgramId, Task, TaskKind, Tidying};
use crate::game::base::floor::spawn_floor_item;

fn game_with_home() -> Game {
    let mut g = game();
    place_home(&mut g);
    stand_in_base(&mut g);
    lay_floor_box(&mut g, (-10, -10), (10, 10));
    g
}

fn core() -> FloorThing {
    FloorThing::Item(ItemId::from(ids::CORE_FRAGMENT))
}

fn at(x: i32, y: i32) -> Position {
    Position { x, y }
}

fn loose(g: &mut Game, x: i32, y: i32) -> Entity {
    spawn_floor_item(&mut g.world, at(x, y), core())
}

/// Two stockpile cells at (4,0) and (5,0).
fn with_pile(g: &mut Game) {
    g.toggle_stockpile_box((4, 0), (5, 0));
    assert_eq!(g.stockpile_cells(), vec![(4, 0), (5, 0)]);
}

fn on_pile(g: &mut Game) -> usize {
    floor_things(g)
        .into_iter()
        .filter(|(c, _)| g.world.resource::<BaseGrid>().is_stockpile(c.0, c.1))
        .count()
}

#[test]
fn an_idle_program_carries_a_loose_item_onto_a_stockpile() {
    let mut g = game_with_home();
    with_pile(&mut g);
    spawn_tamed(&mut g, 100, 3);
    loose(&mut g, -3, 0);
    let mut ticks = 0;
    while on_pile(&mut g) == 0 {
        assert!(ticks < 200, "never delivered: {:?}", floor_things(&mut g));
        g.tick();
        ticks += 1;
    }
    let things = floor_things(&mut g);
    assert_eq!(things.len(), 1, "conserved");
    assert!(matches!(things[0].0, (4, 0) | (5, 0)), "{things:?}");
}

#[test]
fn a_program_handed_a_task_mid_carry_sets_its_item_down() {
    let mut g = game_with_home();
    with_pile(&mut g);
    let node = spawn_machine_at(&mut g, "mining_node", 2, 0);
    let worker = spawn_tamed(&mut g, 100, 3);
    g.world
        .entity_mut(worker)
        .insert((at(6, 6), Tidying(core())));
    g.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: node,
        progress: 0,
        required: 10,
    });
    g.tick();
    assert!(g.world.get::<Tidying>(worker).is_none());
    assert_eq!(floor_things(&mut g).len(), 1);
}

#[test]
fn a_downed_or_off_shift_tidier_sets_its_item_down_first() {
    for downed in [true, false] {
        let mut g = game_with_home();
        with_pile(&mut g);
        let worker = spawn_tamed(&mut g, 100, 3);
        g.world
            .entity_mut(worker)
            .insert((at(6, 6), Tidying(core())));
        if downed {
            g.world.entity_mut(worker).insert(Downed);
        } else {
            g.world.entity_mut(worker).insert(OffShift {
                need: crate::needs::NeedId::from("rest"),
            });
        }
        let amenities = g.amenities_for_test();
        let bays = g.repair_bays();
        g.drift_idle_staff_for_test(&[worker], &amenities, &bays);
        assert!(g.world.get::<Tidying>(worker).is_none(), "downed={downed}");
        assert_eq!(floor_things(&mut g).len(), 1, "downed={downed}");
    }
}

#[test]
fn nothing_is_carried_when_no_stockpile_cell_is_free() {
    let mut g = game_with_home();
    with_pile(&mut g);
    spawn_floor_item(&mut g.world, at(4, 0), core());
    spawn_floor_item(&mut g.world, at(5, 0), core());
    let worker = spawn_tamed(&mut g, 100, 3);
    loose(&mut g, -3, 0);
    for _ in 0..60 {
        g.tick();
        assert!(g.world.get::<Tidying>(worker).is_none());
    }
    assert!(floor_things(&mut g).iter().any(|(c, _)| *c == (-3, 0)));
    assert_eq!(floor_things(&mut g).len(), 3);
}

#[test]
fn an_item_already_on_a_stockpile_is_not_moved() {
    let mut g = game_with_home();
    with_pile(&mut g);
    let worker = spawn_tamed(&mut g, 100, 3);
    spawn_floor_item(&mut g.world, at(4, 0), core());
    for _ in 0..60 {
        g.tick();
        assert!(g.world.get::<Tidying>(worker).is_none());
    }
    assert_eq!(floor_things(&mut g), vec![((4, 0), core())]);
}

#[test]
fn two_tidiers_never_stack_one_stockpile_cell() {
    let mut g = game_with_home();
    g.toggle_stockpile_box((4, 0), (4, 0));
    spawn_tamed(&mut g, 100, 3);
    spawn_tamed(&mut g, 100, 3);
    loose(&mut g, -3, 0);
    loose(&mut g, -3, 2);
    for _ in 0..300 {
        g.tick();
        let things = floor_things(&mut g);
        let cells: std::collections::HashSet<_> = things.iter().map(|(c, _)| *c).collect();
        let held = g.world.query::<&Tidying>().iter(&g.world).count();
        assert_eq!(cells.len(), things.len(), "one item to a tile");
        assert_eq!(things.len() + held, 2, "conserved");
    }
    assert_eq!(on_pile(&mut g), 1);
}

#[test]
fn a_tidiers_item_survives_save_and_load() {
    let mut g = game_with_home();
    with_pile(&mut g);
    let worker = spawn_tamed(&mut g, 100, 3);
    let copy = crate::items::GearCopy::plain(ItemId::from(ids::CORE_FRAGMENT));
    g.world
        .entity_mut(worker)
        .insert(Tidying(FloorThing::Gear(copy.clone())));
    let id = *g.world.get::<ProgramId>(worker).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_processes_tidying_round_trip_{}.sav",
        std::process::id()
    ));
    g.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let held: Vec<_> = loaded
        .world
        .query::<(&ProgramId, &Tidying)>()
        .iter(&loaded.world)
        .map(|(p, t)| (*p, t.0.clone()))
        .collect();
    assert_eq!(held, vec![(id, FloorThing::Gear(copy))]);
}

#[test]
fn a_tidier_sold_mid_carry_sets_its_item_down() {
    let mut g = game_with_home();
    let market = spawn_market(&mut g);
    let pet = spawn_tamed(&mut g, 60, 8);
    g.world.entity_mut(pet).insert((at(6, 6), Tidying(core())));
    g.sell_companion(market, pet).unwrap();
    assert_eq!(floor_things(&mut g), vec![((6, 6), core())]);
}

#[test]
fn a_base_without_stockpiles_never_tidies() {
    let mut g = game_with_home();
    spawn_tamed(&mut g, 100, 3);
    loose(&mut g, -3, 0);
    assert!(g.tidy_board().is_none());
    for _ in 0..100 {
        g.tick();
    }
    assert_eq!(g.world.query::<&Tidying>().iter(&g.world).count(), 0);
    assert_eq!(floor_things(&mut g), vec![((-3, 0), core())]);
    let _ = g.world.query::<&FloorItem>();
}

#[test]
fn a_body_that_has_downed_tools_never_tidies() {
    let mut g = game_with_home();
    with_pile(&mut g);
    let worker = spawn_tamed(&mut g, 100, 3);
    g.world
        .entity_mut(worker)
        .insert(crate::components::Disgruntled {
            grievance: crate::components::Grievance::DownedTools,
            stranded: false,
            told: false,
        });
    loose(&mut g, -3, 0);
    // The drift alone: a full tick would run `update_disgruntled`, which
    // clears the marker of a body whose morale is neutral.
    for _ in 0..40 {
        let amenities = g.amenities_for_test();
        let bays = g.repair_bays();
        g.drift_idle_staff_for_test(&[worker], &amenities, &bays);
        assert!(g.world.get::<Tidying>(worker).is_none());
    }
    assert_eq!(floor_things(&mut g), vec![((-3, 0), core())]);
}
