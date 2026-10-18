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
    let node = spawn_machine_with_recipe_at(&mut g, "extractor", "core_fragment", 2, 0);
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
    assert!(g.world.get::<Task>(worker).is_some(), "the task was kept");
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

// ---------------------------------------------------------------------------
// Stock: a plain item on a stockpile cell is the base's.
// ---------------------------------------------------------------------------

use crate::components::Stock;
use crate::game::base::work_orders::base_holding;

fn frag() -> ItemId {
    ItemId::from(ids::CORE_FRAGMENT)
}

fn shelf_with(g: &mut Game, x: i32, y: i32, qty: u32) -> Entity {
    let shelf = spawn_machine_at(g, "extractor", x, y);
    g.world
        .get_mut::<Stock>(shelf)
        .unwrap()
        .output
        .insert(frag(), qty);
    shelf
}

fn shelf_qty(g: &Game, shelf: Entity) -> u32 {
    g.world
        .get::<Stock>(shelf)
        .unwrap()
        .output
        .get(&frag())
        .copied()
        .unwrap_or(0)
}

#[test]
fn a_stockpiled_item_counts_and_a_loose_one_does_not() {
    let mut g = game_with_home();
    with_pile(&mut g);
    shelf_with(&mut g, 2, 0, 3);
    assert_eq!(base_holding(&g, &frag()), 3);
    loose(&mut g, -3, 0);
    assert_eq!(base_holding(&g, &frag()), 3, "loose does not count");
    loose(&mut g, 4, 0);
    assert_eq!(base_holding(&g, &frag()), 4);
}

#[test]
fn spending_takes_shelves_first_then_stockpiles_in_tile_order() {
    let mut g = game_with_home();
    with_pile(&mut g);
    let shelf = shelf_with(&mut g, 2, 0, 1);
    loose(&mut g, 5, 0);
    loose(&mut g, 4, 0);
    let taken = crate::game::base::stock::spend_from_base(
        &mut g,
        &frag(),
        2,
        crate::base_ledger::ConsumeSource::Base,
    );
    assert_eq!(taken, 2);
    assert_eq!(shelf_qty(&g, shelf), 0);
    assert_eq!(floor_things(&mut g), vec![((5, 0), core())]);
}

#[test]
fn a_bill_is_checked_and_paid_across_shelf_and_stockpile() {
    let mut g = game_with_home();
    with_pile(&mut g);
    shelf_with(&mut g, 2, 0, 1);
    loose(&mut g, 4, 0);
    let bill = [(frag(), 3)];
    assert!(!crate::game::base::stock::spend_bill_from_base(
        &mut g,
        &bill,
        crate::base_ledger::ConsumeSource::Base
    ));
    assert_eq!(
        floor_things(&mut g).len(),
        1,
        "a refused bill moves nothing"
    );
    assert!(crate::game::base::stock::spend_bill_from_base(
        &mut g,
        &[(frag(), 2)],
        crate::base_ledger::ConsumeSource::Base
    ));
    assert!(floor_things(&mut g).is_empty());
}

#[test]
fn the_stock_block_counts_stockpiled_items() {
    let mut g = game_with_home();
    with_pile(&mut g);
    shelf_with(&mut g, 2, 0, 2);
    loose(&mut g, 4, 0);
    loose(&mut g, -3, 0);
    let row = g
        .base_stock()
        .into_iter()
        .find(|r| r.item == frag())
        .expect("a row");
    assert_eq!(row.qty, 3);
}

#[test]
fn a_stockpiled_gear_copy_or_program_is_not_stock() {
    let mut g = game_with_home();
    with_pile(&mut g);
    let copy = crate::items::GearCopy::plain(frag());
    spawn_floor_item(&mut g.world, at(4, 0), FloorThing::Gear(copy));
    assert!(crate::game::base::stock::stockpiled(&g).is_empty());
    assert_eq!(base_holding(&g, &frag()), 0);
    assert!(
        g.base_stock()
            .iter()
            .all(|r| r.item != frag() || r.qty == 0)
    );
    let taken = crate::game::base::stock::spend_from_base(
        &mut g,
        &frag(),
        1,
        crate::base_ledger::ConsumeSource::Base,
    );
    assert_eq!(taken, 0);
    assert_eq!(floor_things(&mut g).len(), 1, "gear is left where it lies");
}

fn pack_frags(g: &Game) -> u32 {
    g.world
        .get::<Inventory>(g.player_entity())
        .unwrap()
        .count(&frag())
}

#[test]
fn a_builder_fetches_from_a_stockpile_before_the_pack() {
    let mut g = game_with_home();
    with_pile(&mut g);
    g.world
        .get_mut::<Inventory>(g.player_entity())
        .unwrap()
        .add(frag(), 500);
    file_build(&mut g, "extractor", 1, 0).unwrap();
    let builder = spawn_tamed(&mut g, 500, 3);
    // Strictly nearer the pile than the pack: distance and tile rank ahead of
    // the store, which only breaks a tie on the very same tile.
    g.world.entity_mut(builder).insert(at(5, 1));
    spawn_floor_item(&mut g.world, at(4, 0), core());
    let pack = pack_frags(&g);
    let mut ticks = 0;
    while g
        .world
        .get::<crate::components::Carrying>(builder)
        .is_none()
    {
        assert!(ticks < 200, "the builder never picked anything up");
        g.tick();
        ticks += 1;
    }
    assert!(
        floor_things(&mut g).is_empty(),
        "the stockpile was drawn on"
    );
    assert_eq!(pack_frags(&g), pack, "the pack was drawn on first");
}

#[test]
fn on_the_pack_s_own_tile_a_stockpile_ranks_before_the_pack() {
    let mut g = game_with_home();
    let (bx, by) = g.base_pos().unwrap();
    g.toggle_stockpile_box((bx, by), (bx, by));
    g.world
        .get_mut::<Inventory>(g.player_entity())
        .unwrap()
        .add(frag(), 500);
    file_build(&mut g, "extractor", 1, 0).unwrap();
    let builder = spawn_tamed(&mut g, 500, 3);
    spawn_floor_item(&mut g.world, at(bx, by), core());
    let pack = pack_frags(&g);
    let mut ticks = 0;
    while g
        .world
        .get::<crate::components::Carrying>(builder)
        .is_none()
    {
        assert!(ticks < 200, "the builder never picked anything up");
        g.tick();
        ticks += 1;
    }
    assert!(
        floor_things(&mut g).is_empty(),
        "the stockpile was drawn on"
    );
    assert_eq!(pack_frags(&g), pack, "the pack was drawn on first");
}

#[test]
fn a_pack_still_feeds_a_build_when_no_shelf_or_stockpile_holds_it() {
    let mut g = game_with_home();
    with_pile(&mut g);
    g.world
        .get_mut::<Inventory>(g.player_entity())
        .unwrap()
        .add(frag(), 500);
    file_build(&mut g, "extractor", 1, 0).unwrap();
    spawn_tamed(&mut g, 500, 3);
    let mut ticks = 0;
    while pack_frags(&g) == 500 {
        assert!(ticks < 200, "the pack never fed the build");
        g.tick();
        ticks += 1;
    }
}

// ---------------------------------------------------------------------------
// Final-review fixes: nothing a tidier holds is destroyed or hidden.
// ---------------------------------------------------------------------------

#[test]
fn committing_a_tidier_to_a_build_sets_its_item_down() {
    let mut g = game_with_home();
    let worker = spawn_tamed(&mut g, 100, 3);
    g.world
        .entity_mut(worker)
        .insert((at(6, 6), Tidying(core())));
    let snapshot = g.commit_program(worker);
    assert!(snapshot.is_some(), "premise: the commit went through");
    assert_eq!(floor_things(&mut g), vec![((6, 6), core())]);
}

#[test]
fn a_floor_item_under_a_standing_structure_is_moved_off_its_footprint() {
    let mut g = game_with_home();
    loose(&mut g, 3, 3);
    spawn_structure_at(&mut g, "wall", 3, 3);
    let before = floor_things(&mut g);
    assert_eq!(before, vec![((3, 3), core())]);
    g.displace_floor_items_from(&[(3, 3)]);
    let after = floor_things(&mut g);
    assert_eq!(after.len(), 1, "conserved: {after:?}");
    assert_ne!(after[0].0, (3, 3));
}

#[test]
fn placing_a_structure_over_a_floor_item_keeps_the_item_on_open_floor() {
    let mut g = game_with_home();
    loose(&mut g, 3, 3);
    g.place_structure("wall", 3, 3, None).unwrap();
    let things = floor_things(&mut g);
    assert_eq!(things.len(), 1, "conserved: {things:?}");
    assert_ne!(things[0].0, (3, 3), "left under the site");
}

// ---- clearing: idle staff who carry finished output out of a machine ----

use crate::components::{Carrying, Clearing, StandingJob};

fn depot_at(g: &mut Game, x: i32, y: i32) -> Entity {
    let def = g
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "depot")
        .expect("a shipped Depot");
    g.spawn_structure(&def, x, y, None)
}

fn fill_depot(g: &mut Game, depot: Entity) {
    let mut stock = g.world.get_mut::<Stock>(depot).unwrap();
    let cap = stock.capacity;
    stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), cap);
}

/// An Extractor at `(x, y)` holding `qty` finished Core Fragments and no
/// worker: a filled order's leftovers.
fn machine_holding(g: &mut Game, x: i32, y: i32, qty: u32) -> Entity {
    let m = spawn_machine_with_recipe_at(g, "extractor", "core_fragment", x, y);
    g.world
        .get_mut::<Stock>(m)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), qty);
    m
}

fn output_of(g: &Game, e: Entity) -> u32 {
    g.world.get::<Stock>(e).unwrap().output_used()
}

/// Three stockpile cells at (4,0), (5,0) and (6,0).
fn with_three_cells(g: &mut Game) {
    g.toggle_stockpile_box((4, 0), (6, 0));
    assert_eq!(g.stockpile_cells().len(), 3);
}

/// The player's case: every Depot full, the stockpile empty, and the order's
/// disks sitting in the machine that made them.
#[test]
fn an_idle_program_clears_a_machine_onto_the_stockpile_when_every_depot_is_full() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    let depot = depot_at(&mut g, -4, -4);
    fill_depot(&mut g, depot);
    let machine = machine_holding(&mut g, 2, 0, 3);
    let worker = spawn_tamed(&mut g, 100, 3);
    let mut ticks = 0;
    while output_of(&g, machine) > 0 || on_pile(&mut g) < 3 {
        assert!(
            ticks < 300,
            "never cleared: machine holds {}, floor {:?}",
            output_of(&g, machine),
            floor_things(&mut g)
        );
        g.tick();
        ticks += 1;
    }
    assert_eq!(floor_things(&mut g).len(), 3, "conserved");
    assert!(g.world.get::<Carrying>(worker).is_none());
    assert!(g.world.get::<Clearing>(worker).is_none());
}

#[test]
fn a_depot_with_room_is_chosen_over_the_stockpile() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    let depot = depot_at(&mut g, -4, -4);
    let machine = machine_holding(&mut g, 2, 0, 3);
    let worker = spawn_tamed(&mut g, 100, 3);
    let mut ticks = 0;
    while output_of(&g, machine) > 0 || g.world.get::<Carrying>(worker).is_some() {
        assert!(ticks < 300, "never cleared");
        g.tick();
        ticks += 1;
    }
    assert_eq!(output_of(&g, depot), 3, "the Depot took it");
    assert_eq!(on_pile(&mut g), 0, "nothing went to the stockpile");
    assert!(floor_things(&mut g).is_empty());
}

/// Destination before pickup. The board exists here — the refusing Depot
/// has room — and that is the point: a body that picked up first and asked
/// where to go second would be left holding product nothing takes.
#[test]
fn nothing_is_picked_up_when_neither_a_depot_nor_the_stockpile_has_room() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    for x in 4..=6 {
        spawn_floor_item(&mut g.world, at(x, 0), core());
    }
    let full = depot_at(&mut g, -4, -4);
    fill_depot(&mut g, full);
    let refusing = depot_at(&mut g, -4, 4);
    g.set_depot_filter(refusing, &[ItemId::from(ids::CORE_FRAGMENT)], false);
    assert!(
        g.clear_board().is_some(),
        "a Depot with room: the board is built"
    );
    let machine = machine_holding(&mut g, 2, 0, 3);
    let worker = spawn_tamed(&mut g, 100, 3);
    for _ in 0..150 {
        g.tick();
        assert!(
            g.world.get::<Carrying>(worker).is_none(),
            "picked up with nowhere to go"
        );
    }
    assert_eq!(output_of(&g, machine), 3, "it stays in the machine");
}

/// A Depot touching the machine with room for its product is the intake's
/// job, not a trip across the base.
#[test]
fn a_machine_beside_an_accepting_depot_with_room_is_not_cleared() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    depot_at(&mut g, 3, 0);
    machine_holding(&mut g, 2, 0, 3);
    let worker = spawn_tamed(&mut g, 100, 3);
    for _ in 0..150 {
        g.tick();
        assert!(
            g.world.get::<Carrying>(worker).is_none(),
            "the intake's machine was cleared by hand"
        );
    }
    assert_eq!(on_pile(&mut g), 0);
}

/// A feeder in a chain hands its output to the machine beside it. Carrying
/// it off to a Depot would starve that machine, so the feeder is left
/// alone — until its buffer is full, when the neighbour is not keeping up
/// and the overflow may go. The Tend's rule, shared rather than copied.
#[test]
fn a_feeder_beside_a_wanted_consumer_is_a_source_only_when_full() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    let feeder = machine_holding(&mut g, 2, 0, 3);
    let factory = spawn_machine_with_recipe_at(&mut g, "factory", "bytecode_block", 3, 0);
    assert!(
        g.clear_board().unwrap().has_source(feeder),
        "a Factory nothing has been ordered from is a bystander"
    );
    g.world.entity_mut(factory).insert(StandingJob {
        work: true,
        guard: false,
    });
    assert!(
        !g.clear_board().unwrap().has_source(feeder),
        "a feed buffer beside a wanted consumer"
    );
    let capacity = g.world.get::<Stock>(feeder).unwrap().capacity;
    g.world
        .get_mut::<Stock>(feeder)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), capacity);
    assert!(
        g.clear_board().unwrap().has_source(feeder),
        "a full feeder's overflow may go"
    );
    // And in play: with the consumer staffed and a second body idle, no
    // Core Fragment from the partial feed buffer reaches the stockpile. The
    // Factory's own Bytecode Blocks may: they are finished product beside
    // no consumer, which is exactly what the errand is for.
    g.world
        .get_mut::<Stock>(feeder)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), 3);
    spawn_tamed(&mut g, 100, 3);
    spawn_tamed(&mut g, 100, 3);
    for _ in 0..150 {
        g.tick();
        assert!(
            !floor_things(&mut g).iter().any(|(c, thing)| {
                *thing == core() && g.world.resource::<BaseGrid>().is_stockpile(c.0, c.1)
            }),
            "a feed buffer was carried off"
        );
    }
}

/// Product in a machine blocks production; a loose floor item blocks
/// nothing. The body starts beside the loose item and still goes to the
/// machine first.
#[test]
fn clearing_a_machine_outranks_tidying_a_loose_item() {
    let mut g = game_with_home();
    g.toggle_stockpile_box((4, 0), (7, 0));
    let machine = machine_holding(&mut g, -3, 0, 3);
    let worker = spawn_tamed(&mut g, 100, 3);
    let beside = g.world.get::<Position>(worker).copied().unwrap();
    loose(&mut g, beside.x, beside.y + 1);
    let mut ticks = 0;
    while on_pile(&mut g) == 0 {
        assert!(ticks < 300, "nothing reached the stockpile");
        g.tick();
        ticks += 1;
    }
    assert!(
        output_of(&g, machine) < 3,
        "the first thing on the pile came out of the machine"
    );
    assert!(
        floor_things(&mut g)
            .iter()
            .any(|(c, _)| *c == (beside.x, beside.y + 1)),
        "the loose item was left for later"
    );
}

#[test]
fn a_clearer_posted_mid_trip_sets_its_load_down_unit_for_unit() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    let machine = machine_holding(&mut g, 2, 0, 0);
    let worker = spawn_tamed(&mut g, 100, 3);
    g.world.entity_mut(worker).insert((
        at(6, 6),
        Carrying {
            item: ItemId::from(ids::CORE_FRAGMENT),
            qty: 3,
        },
        Clearing,
    ));
    g.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required: 10,
    });
    g.tick();
    assert!(g.world.get::<Task>(worker).is_some(), "the task was kept");
    assert!(g.world.get::<Carrying>(worker).is_none());
    assert!(g.world.get::<Clearing>(worker).is_none());
    assert_eq!(floor_things(&mut g).len(), 3, "nothing destroyed");
}

/// With a shelf that has room, a posted clearer's load goes there rather
/// than onto the floor: product that was safely in a machine must not turn
/// into litter because a want appeared mid-walk.
#[test]
fn a_clearer_posted_mid_trip_shelves_its_load_when_a_depot_has_room() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    let depot = depot_at(&mut g, -4, -4);
    let machine = machine_holding(&mut g, 2, 0, 0);
    let worker = spawn_tamed(&mut g, 100, 3);
    g.world.entity_mut(worker).insert((
        at(6, 6),
        Carrying {
            item: ItemId::from(ids::CORE_FRAGMENT),
            qty: 3,
        },
        Clearing,
    ));
    g.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required: 10,
    });
    g.tick();
    assert!(g.world.get::<Carrying>(worker).is_none());
    assert!(g.world.get::<Clearing>(worker).is_none());
    assert_eq!(output_of(&g, depot), 3, "the load went onto the shelf");
    assert!(floor_things(&mut g).is_empty(), "nothing on the floor");
}

/// The shelving goes through the one put door: a Depot that refuses the item
/// takes nothing, and the load ends on the floor as it would with no shelf.
#[test]
fn a_posted_clearers_load_never_lands_on_a_shelf_that_refuses_it() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    let depot = depot_at(&mut g, -4, -4);
    g.set_depot_filter(depot, &[ItemId::from(ids::CORE_FRAGMENT)], false);
    let machine = machine_holding(&mut g, 2, 0, 0);
    let worker = spawn_tamed(&mut g, 100, 3);
    g.world.entity_mut(worker).insert((
        at(6, 6),
        Carrying {
            item: ItemId::from(ids::CORE_FRAGMENT),
            qty: 3,
        },
        Clearing,
    ));
    g.world.entity_mut(worker).insert(Task {
        kind: TaskKind::GatherResource,
        target: machine,
        progress: 0,
        required: 10,
    });
    g.tick();
    assert_eq!(output_of(&g, depot), 0, "a refusing shelf took the load");
    assert_eq!(floor_things(&mut g).len(), 3);
}

#[test]
fn a_base_with_no_depot_and_no_stockpile_never_clears() {
    let mut g = game_with_home();
    let machine = machine_holding(&mut g, 2, 0, 3);
    let worker = spawn_tamed(&mut g, 100, 3);
    assert!(g.clear_board().is_none());
    for _ in 0..150 {
        g.tick();
        assert!(g.world.get::<Carrying>(worker).is_none());
    }
    assert_eq!(output_of(&g, machine), 3);
}

#[test]
fn a_clearers_load_survives_save_and_load_as_its_own() {
    let mut g = game_with_home();
    with_three_cells(&mut g);
    let worker = spawn_tamed(&mut g, 100, 3);
    g.world.entity_mut(worker).insert((
        Carrying {
            item: ItemId::from(ids::CORE_FRAGMENT),
            qty: 3,
        },
        Clearing,
    ));
    let id = *g.world.get::<ProgramId>(worker).unwrap();
    let path = std::env::temp_dir().join(format!(
        "feral_processes_clearing_round_trip_{}.sav",
        std::process::id()
    ));
    g.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let held: Vec<_> = loaded
        .world
        .query::<(&ProgramId, &Carrying, &Clearing)>()
        .iter(&loaded.world)
        .map(|(p, c, _)| (*p, c.item.clone(), c.qty))
        .collect();
    assert_eq!(held, vec![(id, ItemId::from(ids::CORE_FRAGMENT), 3)]);
}
