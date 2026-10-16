//! Configurable machines: a placed one stands with nothing set, and what it
//! was set to survives a save.

use super::support::*;
use crate::components::MachineRecipe;
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
