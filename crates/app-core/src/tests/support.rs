//! Fixtures shared by the app-core tests.

use feral_processes_engine::save::{self, CreatureSave};

use crate::*;

pub(crate) fn test_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub(crate) fn test_app(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let saves_dir = std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_saves"));
    let history_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}.log"));
    let mut app = App::new(assets_dir.clone(), saves_dir, history_path);
    app.game = Game::new(seed, DifficultyMode::Forgiving, &assets_dir).ok();
    app.mode = Mode::Playing;
    app
}

/// A game where the player owns `count` programs parked well outside
/// `MENU_SCAN_RADIUS` — a cronjob worker left at a far-flung node, say.
/// Built by editing a save and reloading it, since the engine deliberately
/// exposes no way to hand-place a tamed program from outside the crate.
pub(crate) fn app_owning_distant_programs(seed: u32, count: i32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_distant_{seed}.sav"));
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    for i in 0..count {
        data.creatures.push(CreatureSave {
            species: species.clone(),
            position: (px + MENU_SCAN_RADIUS + 10 + i, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            def: 2,
            tamed: true,
            level: 1,
            xp: 0,
            xp_to_next: 10,
            cronjob: None,
            party_slot: None,
            zone: 1,
            custom_name: None,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            fusions: 0,
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
            field_buffs: Vec::new(),
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

/// A game where the player owns one tamed program carrying `routines` and
/// has a Compiler standing, so the extraction flow has both of its
/// preconditions. Built by editing a save and reloading it, for the same
/// reason `app_owning_distant_programs` is.
pub(crate) fn app_owning_a_program_and_a_compiler(seed: u32, routines: &[&str]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_extract_{seed}.sav"));
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        species,
        position: (px + 1, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        def: 1,
        tamed: true,
        level: 1,
        xp: 0,
        xp_to_next: 20,
        cronjob: None,
        party_slot: None,
        zone: 1,
        custom_name: None,
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        fusions: 0,
        routines: routines.iter().map(|r| r.to_string()).collect(),
        field_buffs: Vec::new(),
    });
    data.structures.push(save::StructureSave {
        kind: "compiler".to_string(),
        position: (px + 30, py + 30),
        resource_amount: None,
        durability: None,
        tier: None,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game where the player has `routines` installed (in place of the
/// default `decompile`) and `hunger` set to a chosen level, so a field-cast
/// test can pin affordability on either side of a routine's `power_cost`
/// exactly. Built by editing a save and reloading it, for the same reason
/// `app_owning_a_program_and_a_compiler` is.
pub(crate) fn app_with_player_routines(seed: u32, routines: &[&str], hunger: f32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_field_{seed}.sav"));
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = routines.iter().map(|r| r.to_string()).collect();
    data.player.hunger = hunger;
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// Same as `app_with_player_routines`, but the player also owns one program
/// (parked next to them) and one wild, unowned creature is nearby too — for
/// asserting the field-cast ally picker (`Mode::FieldCastAlly`) offers the
/// former and never the latter. Full Power, since affordability isn't what
/// these tests are checking.
pub(crate) fn app_with_owned_and_wild_neighbors(seed: u32, routines: &[&str]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_field_own_{seed}.sav"));
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = routines.iter().map(|r| r.to_string()).collect();
    data.player.hunger = 100.0;
    let (px, py) = data.player.position;
    for (offset, tamed) in [(1, true), (2, false)] {
        data.creatures.push(CreatureSave {
            species: species.clone(),
            position: (px + offset, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            def: 2,
            tamed,
            level: 1,
            xp: 0,
            xp_to_next: 10,
            cronjob: None,
            party_slot: None,
            zone: 1,
            custom_name: None,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            fusions: 0,
            routines: Vec::new(),
            field_buffs: Vec::new(),
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}
