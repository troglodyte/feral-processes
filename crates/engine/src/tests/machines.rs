//! Configurable machines: a placed one stands with nothing set, and what it
//! was set to survives a save.

use super::support::*;
use crate::components::{FloorPile, MachineRecipe};
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

fn pile_total(game: &mut Game) -> u32 {
    game.world
        .query::<&FloorPile>()
        .iter(&game.world)
        .flat_map(|p| p.items.values())
        .sum()
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
    let at = *game.world.get::<Position>(factory).unwrap();
    game.set_machine_recipe(factory, &item("charge_coil"))
        .unwrap();
    assert_eq!(
        units(&game, factory, "input") + units(&game, factory, "output"),
        0
    );
    let total: u32 = {
        let mut q = game.world.query::<(&FloorPile, &Position)>();
        q.iter(&game.world)
            .filter(|(_, p)| (p.x - at.x).abs() + (p.y - at.y).abs() == 1)
            .flat_map(|(pile, _)| pile.items.values())
            .sum()
    };
    assert_eq!(total, 5);
}

#[test]
fn changing_a_recipe_unposts_its_worker_and_loses_nothing() {
    let mut game = bare_game();
    let (factory, worker) = staffed_full_factory(&mut game);
    fill(&mut game, factory, 0, 0);
    game.world.entity_mut(worker).insert(Carrying {
        item: item("bytecode_block"),
        qty: 2,
    });
    game.set_machine_recipe(factory, &item("charge_coil"))
        .unwrap();
    assert!(game.world.get::<Task>(worker).is_none());
    assert!(game.world.get::<Carrying>(worker).is_none());
    assert_eq!(pile_total(&mut game), 2);
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
