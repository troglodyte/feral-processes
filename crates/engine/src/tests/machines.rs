//! Configurable machines: a placed one stands with nothing set, and what it
//! was set to survives a save.

use super::support::*;
use crate::components::{FloorItem, MachineRecipe};
use crate::items::FloorThing;
use crate::*;

fn save_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_machines_{tag}_{}.bin",
        std::process::id()
    ))
}

fn base_with(kind: &str) -> Game {
    let mut game = Game::new(21001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    place_now(&mut game, kind, 1, 0).unwrap();
    game
}

fn set_recipe(game: &mut Game, machine: Entity, item: &str) {
    game.world
        .entity_mut(machine)
        .insert(MachineRecipe(Some(ItemId::from(item))));
}

fn reload(game: &mut Game, tag: &str, edit: impl FnOnce(String) -> String) -> Game {
    let path = save_path(tag);
    game.save(&path).unwrap();
    let data = crate::save::load_from_file(&path).unwrap();
    let text = edit(crate::save::to_ron(&data).unwrap());
    let data = crate::save::from_ron(&text).unwrap();
    crate::save::save_to_file(&path, &data).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    loaded
}

#[test]
fn a_placed_machine_starts_with_nothing_set() {
    let mut game = base_with("factory");
    let factory = first_structure(&mut game, "factory");
    assert_eq!(
        game.world.get::<MachineRecipe>(factory),
        Some(&MachineRecipe(None))
    );
    assert!(game.world.get::<ResourceNode>(factory).is_none());
    game.tick();
    assert_eq!(
        game.world.get::<MachineStatus>(factory),
        Some(&MachineStatus::Unconfigured)
    );
}

#[test]
fn a_machines_recipe_survives_save_and_load() {
    let mut game = base_with("factory");
    let factory = first_structure(&mut game, "factory");
    set_recipe(&mut game, factory, "bytecode_block");
    let path = save_path("survives");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let factory = first_structure(&mut loaded, "factory");
    assert_eq!(
        loaded.world.get::<MachineRecipe>(factory),
        Some(&MachineRecipe(Some(ItemId::from("bytecode_block"))))
    );
}

#[test]
fn a_saved_recipe_the_def_no_longer_lists_loads_unconfigured() {
    let mut game = base_with("factory");
    let factory = first_structure(&mut game, "factory");
    set_recipe(&mut game, factory, "bytecode_block");
    let mut loaded = reload(&mut game, "unlisted", |text| {
        assert!(
            text.contains("recipe:Some(\"bytecode_block\")")
                || text.contains("recipe: Some(\"bytecode_block\")")
        );
        text.replace("Some(\"bytecode_block\")", "Some(\"honeypot\")")
    });
    let factory = first_structure(&mut loaded, "factory");
    assert_eq!(
        loaded.world.get::<MachineRecipe>(factory),
        Some(&MachineRecipe(None))
    );
    assert!(
        loaded
            .world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text.contains("which it no longer makes")),
        "the player is told why the machine came back with nothing set"
    );
}

#[test]
fn a_reloaded_extractor_rebuilds_its_node_from_the_saved_recipe() {
    let mut game = base_with("extractor");
    let extractor = first_structure(&mut game, "extractor");
    set_recipe(&mut game, extractor, "core_fragment");
    game.world.entity_mut(extractor).insert(StructureTier(3));
    let path = save_path("extractor");
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let extractor = first_structure(&mut loaded, "extractor");
    let node = loaded.world.get::<ResourceNode>(extractor).unwrap();
    assert_eq!(node.resource, ItemId::from("core_fragment"));
    assert_eq!(node.level, Some(3));
}

// ---- setting a recipe ---------------------------------------------------

use crate::views::RecipePace;

fn item(id: &str) -> ItemId {
    ItemId::from(id)
}

fn bare_game() -> Game {
    let mut game = Game::new(21002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    stand_in_base(&mut game);
    game
}

fn view_rows(game: &Game, machine: Entity) -> Vec<(ItemId, RecipePace)> {
    game.machine_recipe_view(machine)
        .unwrap()
        .rows
        .into_iter()
        .map(|r| (r.item, r.pace))
        .collect()
}

fn pace_of(game: &mut Game, kind: &str, recipe: &str) -> RecipePace {
    let machine = spawn_machine_with_recipe_at(game, kind, recipe, 40, 40);
    game.machine_recipe_view(machine)
        .unwrap()
        .rows
        .into_iter()
        .find(|r| r.item == item(recipe))
        .unwrap()
        .pace
}

fn units(game: &Game, e: Entity, which: &str) -> u32 {
    let stock = game.world.get::<Stock>(e).unwrap();
    let map = if which == "input" {
        &stock.input
    } else {
        &stock.output
    };
    map.values().sum()
}

/// Plain item units lying on the floor: one per floor item.
fn floor_units(game: &mut Game) -> u32 {
    game.world
        .query::<&FloorItem>()
        .iter(&game.world)
        .filter(|f| matches!(f.0, FloorThing::Item(_)))
        .count() as u32
}

fn fill(game: &mut Game, e: Entity, input: u32, output: u32) {
    let mut stock = game.world.get_mut::<Stock>(e).unwrap();
    stock.input.insert(item("core_fragment"), input);
    stock.output.insert(item("bytecode_block"), output);
}

fn depot_at(game: &mut Game, x: i32, y: i32) -> Entity {
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "depot")
        .unwrap();
    game.spawn_structure(&def, x, y, None)
}

fn a_fresh_factory(game: &mut Game, x: i32, y: i32) -> Entity {
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "factory")
        .unwrap();
    game.spawn_structure(&def, x, y, None)
}

#[test]
fn the_menu_lists_only_researched_recipes() {
    let mut game = bare_game();
    let extractor = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 40, 40);
    let rows = view_rows(&game, extractor);
    assert_eq!(
        rows.iter().map(|(i, _)| i.clone()).collect::<Vec<_>>(),
        vec![item("core_fragment")]
    );
    unlock_research_chain(&mut game, "power_grid");
    let rows = view_rows(&game, extractor);
    assert!(rows.iter().any(|(i, _)| *i == item("power_cell")));
    assert!(!rows.iter().any(|(i, _)| *i == item("cache_grain")));
    let view = game.machine_recipe_view(extractor).unwrap();
    assert_eq!(view.current, Some(item("core_fragment")));
    assert!(
        view.rows
            .iter()
            .find(|r| r.current)
            .unwrap()
            .inputs
            .is_empty()
    );
}

#[test]
fn a_recipe_pace_is_a_band_never_a_number() {
    let mut game = bare_game();
    unlock_research_chain(&mut game, "power_grid");
    assert_eq!(
        pace_of(&mut game, "extractor", "power_cell"),
        RecipePace::Quick
    );
    assert_eq!(
        pace_of(&mut game, "extractor", "core_fragment"),
        RecipePace::Steady
    );
    unlock_research_chain(&mut game, "routine_fabrication");
    assert_eq!(
        pace_of(&mut game, "assembler", "routine_disk"),
        RecipePace::Slow
    );
    unlock_research_chain(&mut game, "armor_bench");
    assert_eq!(
        pace_of(&mut game, "assembler", "hardened_shell"),
        RecipePace::VerySlow
    );
    assert_eq!(RecipePace::VerySlow.label(), "very slow");
    assert_eq!(RecipePace::Quick.label(), "quick");
}

#[test]
fn setting_an_unresearched_recipe_is_refused() {
    let mut game = bare_game();
    let extractor = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 40, 40);
    let name = game
        .research_defs()
        .into_iter()
        .find(|d| d.id == "power_grid")
        .unwrap()
        .name;
    let err = game
        .set_machine_recipe(extractor, &item("power_cell"))
        .unwrap_err();
    assert_eq!(err, format!("Power Cell needs {name} researched first."));
    assert_eq!(
        game.world.get::<MachineRecipe>(extractor),
        Some(&MachineRecipe(Some(item("core_fragment"))))
    );
}

#[test]
fn setting_an_unlisted_recipe_is_refused() {
    let mut game = bare_game();
    let factory = a_fresh_factory(&mut game, 40, 40);
    let err = game
        .set_machine_recipe(factory, &item("core_fragment"))
        .unwrap_err();
    assert_eq!(err, "A Factory can't make Core Fragment.");
    assert_eq!(
        game.world.get::<MachineRecipe>(factory),
        Some(&MachineRecipe(None))
    );
    let depot = depot_at(&mut game, 42, 40);
    assert_eq!(
        game.set_machine_recipe(depot, &item("core_fragment"))
            .unwrap_err(),
        "That structure has nothing to set."
    );
}

#[test]
fn setting_the_same_recipe_again_is_refused() {
    let mut game = bare_game();
    let factory = spawn_machine_with_recipe_at(&mut game, "factory", "bytecode_block", 40, 40);
    let err = game
        .set_machine_recipe(factory, &item("bytecode_block"))
        .unwrap_err();
    assert_eq!(err, "It is already making Bytecode Block.");
}

/// A factory at (40, 40) holding 3 input and 2 output, with a worker posted
/// on it.
fn staffed_full_factory(game: &mut Game) -> (Entity, Entity) {
    let factory = spawn_machine_with_recipe_at(game, "factory", "bytecode_block", 40, 40);
    fill(game, factory, 3, 2);
    let worker = spawn_tamed(game, 10, 3);
    game.world.entity_mut(worker).insert((
        Position { x: 40, y: 41 },
        Task {
            kind: TaskKind::GatherResource,
            target: factory,
            progress: 5,
            required: 12,
        },
    ));
    (factory, worker)
}

#[test]
fn changing_a_recipe_empties_the_machine_into_a_depot() {
    let mut game = bare_game();
    let (factory, worker) = staffed_full_factory(&mut game);
    let depot = depot_at(&mut game, 41, 40);
    game.set_machine_recipe(factory, &item("charge_coil"))
        .unwrap();
    assert_eq!(units(&game, factory, "input"), 0);
    assert_eq!(units(&game, factory, "output"), 0);
    assert_eq!(units(&game, depot, "output"), 5);
    assert!(game.world.get::<Task>(worker).is_none());
    assert_eq!(
        game.world.get::<MachineRecipe>(factory),
        Some(&MachineRecipe(Some(item("charge_coil"))))
    );
    assert!(
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text
                == "The Factory is set to make Charge Coil. What it held goes to the Depots.")
    );
}

#[test]
fn changing_a_recipe_with_no_depot_leaves_the_units_beside_it() {
    let mut game = Game::new(21003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 500);
    stand_in_base(&mut game);
    let factory = place_machine(&mut game, "factory", "bytecode_block", 1, 0).unwrap();
    fill(&mut game, factory, 3, 2);
    game.set_machine_recipe(factory, &item("charge_coil"))
        .unwrap();
    assert_eq!(
        units(&game, factory, "input") + units(&game, factory, "output"),
        0
    );
    assert_eq!(floor_units(&mut game), 5);
}

#[test]
fn changing_a_recipe_unposts_its_worker_and_loses_nothing() {
    let mut game = bare_game();
    let (factory, worker) = staffed_full_factory(&mut game);
    fill(&mut game, factory, 0, 0);
    // Laid floor around the worker, or `set_down` stows the load in the pack.
    lay_floor_box(&mut game, (38, 38), (42, 43));
    game.world.entity_mut(worker).insert(Carrying {
        item: item("bytecode_block"),
        qty: 2,
    });
    game.set_machine_recipe(factory, &item("charge_coil"))
        .unwrap();
    assert!(game.world.get::<Task>(worker).is_none());
    assert!(game.world.get::<Carrying>(worker).is_none());
    assert_eq!(floor_units(&mut game), 2);
}

#[test]
fn an_extractor_keeps_its_tier_across_recipes() {
    let mut game = bare_game();
    unlock_research_chain(&mut game, "power_grid");
    let extractor = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 40, 40);
    game.world.entity_mut(extractor).insert(StructureTier(3));
    game.set_machine_recipe(extractor, &item("power_cell"))
        .unwrap();
    assert_eq!(
        game.world.get::<ResourceNode>(extractor).unwrap().level,
        None
    );
    game.set_machine_recipe(extractor, &item("core_fragment"))
        .unwrap();
    // The node is rebuilt from the tier the player upgraded to.
    assert_eq!(
        game.world.get::<ResourceNode>(extractor).unwrap().level,
        Some(3)
    );
    assert_eq!(game.world.get::<StructureTier>(extractor).unwrap().0, 3);
}

#[test]
fn the_first_recipe_filters_only_unfiltered_depots_beside_it() {
    let mut game = bare_game();
    let factory = a_fresh_factory(&mut game, 40, 40);
    let a = depot_at(&mut game, 41, 40);
    let b = depot_at(&mut game, 39, 40);
    game.set_depot_filter(b, &[item("charge_coil")], false);
    let b_before = game.world.get::<DepotFilter>(b).unwrap().denied.clone();
    game.set_machine_recipe(factory, &item("bytecode_block"))
        .unwrap();
    let a_denied = game.world.get::<DepotFilter>(a).unwrap().denied.clone();
    assert!(!a_denied.contains(&item("bytecode_block")));
    assert!(a_denied.contains(&item("core_fragment")));
    assert_eq!(game.world.get::<DepotFilter>(b).unwrap().denied, b_before);
    game.set_machine_recipe(factory, &item("charge_coil"))
        .unwrap();
    assert_eq!(game.world.get::<DepotFilter>(a).unwrap().denied, a_denied);
}

// ---------------------------------------------------------------------
// Task 4: every production reader goes per instance
// ---------------------------------------------------------------------

fn put_output(game: &mut Game, machine: Entity, item_id: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
    *stock.output.entry(item(item_id)).or_default() += qty;
}

fn put_input(game: &mut Game, machine: Entity, item_id: &str, qty: u32) {
    let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
    *stock.input.entry(item(item_id)).or_default() += qty;
}

/// The entities a body is currently posted to.
fn posts(game: &mut Game) -> Vec<Entity> {
    game.world
        .query::<&Task>()
        .iter(&game.world)
        .filter(|t| matches!(t.kind, TaskKind::GatherResource))
        .map(|t| t.target)
        .collect()
}

fn hire(game: &mut Game, n: usize) {
    for _ in 0..n {
        spawn_tamed(game, 10, 3);
    }
}

/// A powered, homed base with room around the origin for a line.
fn powered_base(seed: u32) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    stand_ample_grid_supply(&mut game);
    game
}

/// The cycle a worker of the standard speed bakes onto its `Task` when posted
/// to a freshly built `kind` set to `recipe` (a fixed-recipe kind ignores it).
fn baked_required(kind: &str, recipe: Option<&str>) -> u32 {
    let mut game = powered_base(21010);
    let machine = match recipe {
        Some(recipe) => spawn_machine_with_recipe_at(&mut game, kind, recipe, 2, 0),
        None => spawn_machine_at(&mut game, kind, 2, 0),
    };
    let worker = spawn_tamed(&mut game, 10, 3);
    stand_player_at_post(&mut game, machine);
    game.assign_cronjob(worker, machine).unwrap();
    game.world.get::<Task>(worker).unwrap().required
}

#[test]
fn a_configured_line_runs_like_the_structures_it_replaces() {
    let mut game = powered_base(21011);
    let extractor = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    let factory = spawn_machine_with_recipe_at(&mut game, "factory", "bytecode_block", 3, 0);
    let (digger, builder) = (spawn_tamed(&mut game, 10, 3), spawn_tamed(&mut game, 10, 3));
    stand_player_at_post(&mut game, extractor);
    game.assign_cronjob(digger, extractor).unwrap();
    stand_player_at_post(&mut game, factory);
    game.assign_cronjob(builder, factory).unwrap();
    park_at_post(&mut game, digger, extractor);
    park_at_post(&mut game, builder, factory);

    let required = game.world.get::<Task>(builder).unwrap().required;
    // 12 is what the Refinery's worker bakes at the same speed (recorded for
    // Task 14, which deletes the Refinery and pins this to the number).
    assert_eq!(required, baked_required("refinery", None));
    assert_eq!(required, 12);

    for _ in 0..200 {
        game.tick();
    }
    assert!(
        node_output(&game, factory, "bytecode_block") > 0,
        "the line should have produced bytecode"
    );
}

#[test]
fn an_unconfigured_machine_is_never_staffed() {
    let mut game = powered_base(21012);
    let factory = spawn_machine_at(&mut game, "factory", 3, 0);
    hire(&mut game, 2);
    game.queue_work_order(WorkOrder::batch(item("bytecode_block"), 5))
        .expect("an order for an item a machine could be set to make is accepted");
    for _ in 0..200 {
        game.tick();
    }
    assert!(!posts(&mut game).contains(&factory));
    assert!(!game.accepts_a_program(factory));
    assert!(game.set_standing_job(factory, true, false).is_err());
    assert_eq!(
        game.world.get::<MachineStatus>(factory),
        Some(&MachineStatus::Unconfigured)
    );
}

#[test]
fn an_unconfigured_machine_draws_no_power() {
    let mut game = Game::new(21013, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    spawn_machine_with_recipe_at(&mut game, "factory", "bytecode_block", 3, 0);
    let idle = spawn_machine_at(&mut game, "assembler", 4, 0);
    let ledger = {
        let db = game.world.resource::<StructureDb>();
        let items = game.world.resource::<ItemDb>();
        crate::game::base::power::ledger(&game.world, db, items)
    };
    let drawn = |e: Entity| {
        ledger
            .entries
            .iter()
            .find(|entry| entry.entity == e)
            .map_or(0, |entry| entry.draw)
    };
    assert_eq!(drawn(idle), 0, "an unset Assembler is not in the draw");
    assert_eq!(ledger.draw, 1 + 2, "only the Extractor and Factory draw");
    assert!(
        ledger.draw <= ledger.supply,
        "the Home covers exactly the configured machines"
    );
    assert!(ledger.dark.is_empty(), "so nothing goes dark");
}

#[test]
fn an_unconfigured_machine_pulls_nothing_from_its_neighbours() {
    let mut game = powered_base(21014);
    let extractor = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    let factory = spawn_machine_at(&mut game, "factory", 3, 0);
    put_output(&mut game, extractor, "core_fragment", 10);
    // A body forced onto it, so the only thing stopping a pull is that the
    // machine makes nothing.
    let worker = spawn_tamed(&mut game, 10, 3);
    game.world.entity_mut(worker).insert((
        Position { x: 4, y: 0 },
        Task {
            kind: TaskKind::GatherResource,
            target: factory,
            progress: 0,
            required: 12,
        },
    ));
    for _ in 0..100 {
        game.tick();
    }
    assert_eq!(node_output(&game, extractor, "core_fragment"), 10);
    assert_eq!(units(&game, factory, "input"), 0);
}

#[test]
fn an_order_is_served_only_by_machines_set_to_its_item() {
    let mut game = powered_base(21015);
    spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    let right = spawn_machine_with_recipe_at(&mut game, "factory", "bytecode_block", 3, 0);
    let wrong = spawn_machine_with_recipe_at(&mut game, "factory", "charge_coil", 9, 0);
    put_input(&mut game, right, "core_fragment", 50);
    put_input(&mut game, wrong, "core_fragment", 50);
    hire(&mut game, 2);
    game.queue_work_order(WorkOrder::batch(item("bytecode_block"), 30))
        .unwrap();
    game.tick();
    let posted = posts(&mut game);
    assert!(posted.contains(&right));
    assert!(!posted.contains(&wrong));
}

#[test]
fn an_order_for_an_item_no_machine_is_set_to_make_waits_stalled() {
    let mut game = powered_base(21016);
    let factory = spawn_machine_at(&mut game, "factory", 3, 0);
    hire(&mut game, 1);
    game.queue_work_order(WorkOrder::batch(item("bytecode_block"), 5))
        .expect("accepted, to wait for a setting");
    let report = game.work_order_report();
    assert_eq!(report.len(), 1);
    assert_eq!(report[0].state, crate::views::OrderState::Stalled);
    assert_eq!(
        report[0].blocked_by.as_deref(),
        Some("No way to make this — set a Factory to Bytecode Block.")
    );

    set_recipe(&mut game, factory, "bytecode_block");
    put_input(&mut game, factory, "core_fragment", 50);
    for _ in 0..200 {
        game.tick();
        if game.work_order_report()[0].state != crate::views::OrderState::Stalled {
            return;
        }
    }
    panic!("setting the machine should wake the order");
}

#[test]
fn a_standing_order_stalls_when_its_machine_is_switched_and_resumes_when_set_back() {
    use crate::game::base::work_orders::{OrderStanding, standing_of};
    let mut game = powered_base(21017);
    let extractor = spawn_machine_with_recipe_at(&mut game, "extractor", "core_fragment", 2, 0);
    let factory = spawn_machine_with_recipe_at(&mut game, "factory", "bytecode_block", 3, 0);
    put_output(&mut game, extractor, "core_fragment", 20);
    game.queue_work_order(WorkOrder::level(item("bytecode_block"), 5))
        .unwrap();
    let order = game.work_orders()[0].clone();
    assert!(matches!(
        standing_of(&game, &order),
        OrderStanding::Wants(_)
    ));

    game.set_machine_recipe(factory, &item("charge_coil"))
        .unwrap();
    assert_eq!(game.work_orders().len(), 1, "the order stays queued");
    assert!(matches!(standing_of(&game, &order), OrderStanding::Stalled));

    game.set_machine_recipe(factory, &item("bytecode_block"))
        .unwrap();
    put_output(&mut game, extractor, "core_fragment", 20);
    assert!(matches!(
        standing_of(&game, &order),
        OrderStanding::Wants(_)
    ));
}

#[test]
fn power_cell_pays_tier_one_on_an_upgraded_extractor() {
    let mut game = base_with("depot");
    let extractor = place_machine(&mut game, "extractor", "power_cell", 0, 1).unwrap();
    game.world.entity_mut(extractor).insert(StructureTier(4));
    let zone = *game.world.resource::<ZoneLevel>();

    let paid = |game: &mut Game, item_id: &str| {
        let before = node_output(game, extractor, item_id);
        game.work_structure(extractor).unwrap();
        let mut ticks = 0;
        while node_output(game, extractor, item_id) == before && ticks < 80 {
            game.wait();
            ticks += 1;
        }
        node_output(game, extractor, item_id) - before
    };
    assert_eq!(
        paid(&mut game, "power_cell"),
        crate::systems::node_payout(1, zone)
    );

    game.set_machine_recipe(extractor, &item("core_fragment"))
        .unwrap();
    // Reliability off, so the measurement is the payout and not the roll.
    game.world.get_mut::<ResourceNode>(extractor).unwrap().level = None;
    assert_eq!(
        paid(&mut game, "core_fragment"),
        crate::systems::node_payout(4, zone)
    );
}

#[test]
fn a_depot_placed_beside_a_configured_machine_filters_for_its_output() {
    let mut game = bare_game();
    spawn_machine_with_recipe_at(&mut game, "factory", "bytecode_block", 40, 40);
    let depot = depot_at(&mut game, 41, 40);
    let filter = game
        .world
        .get::<crate::components::DepotFilter>(depot)
        .expect("a filter is written beside a configured machine");
    assert!(!filter.denied.contains(&item("bytecode_block")));
    assert!(filter.denied.contains(&item("core_fragment")));
}

#[test]
fn a_posted_worker_on_an_upgraded_power_cell_extractor_pays_tier_one() {
    let mut game = base_with("depot");
    let extractor = place_machine(&mut game, "extractor", "power_cell", 0, 1).unwrap();
    game.world.entity_mut(extractor).insert(StructureTier(4));
    let zone = *game.world.resource::<ZoneLevel>();
    let worker = spawn_tamed(&mut game, 10, 3);
    stand_player_at_post(&mut game, extractor);
    game.assign_cronjob(worker, extractor).unwrap();
    park_at_post(&mut game, worker, extractor);
    // Counted across every buffer: the crew carries a cycle's output to the
    // Depot beside the machine.
    let made = |game: &mut Game| -> u32 {
        game.world
            .query::<&Stock>()
            .iter(&game.world)
            .map(|s| s.output.get(&item("power_cell")).copied().unwrap_or(0))
            .sum()
    };
    let mut ticks = 0;
    while made(&mut game) == 0 && ticks < 80 {
        game.tick();
        ticks += 1;
    }
    assert_eq!(made(&mut game), crate::systems::node_payout(1, zone));
}

// ---- naming -------------------------------------------------------------

#[test]
fn a_set_machine_is_labelled_by_its_product() {
    let mut game = bare_game();
    let factory = spawn_machine_with_recipe_at(&mut game, "factory", "bytecode_block", 40, 40);
    let report = game.structure_report();
    let row = report.iter().find(|r| r.entity == factory).unwrap();
    assert_eq!(row.label, "Factory (Bytecode Block)");
    assert_eq!(row.recipe, Some(item("bytecode_block")));
    let depot = depot_at(&mut game, 42, 40);
    let report = game.structure_report();
    let row = report.iter().find(|r| r.entity == depot).unwrap();
    assert_eq!(row.recipe, None);
    assert!(!row.label.contains('('));
}

#[test]
fn an_unset_machine_reads_as_its_kind_and_is_flagged() {
    let mut game = bare_game();
    let factory = a_fresh_factory(&mut game, 40, 40);
    let report = game.structure_report();
    let row = report.iter().find(|r| r.entity == factory).unwrap();
    assert_eq!(row.label, "Factory");
    assert_eq!(row.recipe, None);
    let view = game
        .view_entities(60, 60)
        .into_iter()
        .find(|v| v.entity == factory)
        .unwrap();
    assert_eq!(view.label, "Factory");
    assert!(view.unconfigured);
    set_recipe_researched(&mut game, factory, "bytecode_block").unwrap();
    let view = game
        .view_entities(60, 60)
        .into_iter()
        .find(|v| v.entity == factory)
        .unwrap();
    assert_eq!(view.label, "Factory (Bytecode Block)");
    assert!(!view.unconfigured);
}

#[test]
fn a_stall_line_names_the_product() {
    let mut game = bare_game();
    let (factory, _worker) = staffed_full_factory(&mut game);
    let capacity = game.world.get::<Stock>(factory).unwrap().capacity;
    fill(&mut game, factory, 8, capacity);
    game.tick();
    assert!(
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .any(|l| l.text
                == "The Factory (Bytecode Block) is clogged — its output buffer is full."),
        "{:?}",
        game.world
            .resource::<MessageLog>()
            .lines
            .iter()
            .map(|l| &l.text)
            .collect::<Vec<_>>()
    );
}
