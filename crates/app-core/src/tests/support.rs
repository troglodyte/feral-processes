//! Fixtures shared by the app-core tests.

use std::sync::atomic::{AtomicU32, Ordering};

use feral_processes_engine::save::{self, CreatureSave};

use crate::*;

pub(crate) fn test_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// A scratch path no other fixture call can be using.
///
/// Keying these on `(fixture, seed)` alone was not enough: the test binary
/// runs its cases as concurrent threads, so two tests that reach for the
/// same fixture with the same seed shared one file and raced — one loading
/// what the other had half-written. That is not hypothetical, it is how
/// `a_full_party_is_asked_slot_by_slot_and_only_then_resolves` failed in the
/// suite while passing alone, and `app_at_a_trading_post(921, ..)` had the
/// same collision waiting in two other tests.
///
/// A counter rather than a timestamp or a random suffix, deliberately: it is
/// unique across the process without making the run depend on a clock or on
/// RNG nobody seeded.
fn scratch_path(fixture: &str, seed: u32) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "feral_processes_appcore_{fixture}_{seed}_{unique}.sav"
    ))
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
    let path = scratch_path("distant", seed);
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
    let path = scratch_path("extract", seed);
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

/// A game where the player stands next to a Black Market — the shipped
/// trader that buys programs as well as items — holding exactly
/// `inventory`, and owning one tamed program so the trader's program rows
/// are populated too. Built by editing a save and reloading it, for the
/// same reason `app_owning_a_program_and_a_compiler` is: staging a trading
/// post through the build flow needs a Home, build clearance and 16 Core
/// Fragments, and the player starts with 5.
pub(crate) fn app_at_a_trading_post(seed: u32, inventory: &[(&str, u32)]) -> App {
    app_at_trading_posts(seed, inventory, 1)
}

/// `app_at_a_trading_post` with `posts` traders in range instead of one —
/// the case where "sell this" is no longer a complete instruction.
pub(crate) fn app_at_trading_posts(seed: u32, inventory: &[(&str, u32)], posts: i32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("market", seed);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = inventory
        .iter()
        .map(|(item, qty)| (ItemId::from(*item), *qty))
        .collect();
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        species,
        position: (px + 2, py),
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
    for n in 0..posts {
        data.structures.push(save::StructureSave {
            kind: "market".to_string(),
            // Spread along -y so none lands on the player, the program at
            // `px + 2` or each other.
            position: (px + 1, py - n),
            resource_amount: None,
            durability: None,
            tier: None,
        });
    }
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
    let path = scratch_path("field", seed);
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
    let path = scratch_path("field_own", seed);
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

/// A game where the player has one program standing in the active `Party`,
/// so a battle opens with two planning slots rather than the single one
/// every other battle fixture here produces. Built by editing a save and
/// reloading it, for the reason `app_owning_a_program_and_a_compiler` is:
/// nothing outside the engine can hand-place a tamed program, and
/// `party_slot` is the save field that puts one on the roster.
pub(crate) fn app_with_companions_in_the_party(seed: u32, count: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("party", seed);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    for slot in 0..count {
        data.creatures.push(CreatureSave {
            species: species.clone(),
            position: (px, py),
            hp: 30,
            max_hp: 30,
            atk: 3,
            def: 1,
            tamed: true,
            level: 1,
            xp: 0,
            xp_to_next: 20,
            cronjob: None,
            party_slot: Some(slot),
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
    app.mode = Mode::Playing;
    app
}
