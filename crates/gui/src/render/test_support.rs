//! Fixtures for `render/`'s own tests that need a `Game` or `App` state no
//! public constructor reaches directly — a tamed program, or a structure
//! standing that costs nothing to place.
//!
//! `Game`'s ECS `World` is private to the engine crate, so — same as every
//! analogous fixture in app-core's own `tests/support.rs`
//! (`crates/app-core/src/tests/support.rs`, see `tame_program_at_zone` and
//! `app_owning_a_program_and_a_compiler_deep`) — the only door onto a tamed
//! `Creature` or a hand-placed structure from outside that crate is a save
//! round trip: write the game out, edit the `SaveData` by hand, load it back
//! in. app-core's own versions of these fixtures are `pub(crate)` to that
//! crate and so cannot be imported here, but every piece the trick is built
//! from — `Game::save`/`Game::load`, `save::{load_from_file, save_to_file,
//! CreatureSave, StructureSave}` — is `pub`, so the pattern ports without
//! needing anything app-core-only. This module is gui's first use of it.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU32, Ordering};

use feral_processes_app_core::App;
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save::{self, CreatureSave};
use feral_processes_engine::{DifficultyMode, Game};

pub(super) fn test_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

pub(super) fn arenas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dev-arenas")
}

/// A scratch save path no other fixture call can be using — the `gui`-side
/// twin of app-core's own `scratch_path`, with a crate-specific prefix so
/// the two crates' test binaries — which can run at once — never share a
/// filename.
///
/// Counted rather than keyed only on `(fixture, seed)`: this crate's tests
/// run their cases as concurrent threads too, and app-core's doc comment on
/// its own version of this function records the race that shape already
/// caused there (`a_full_party_is_asked_slot_by_slot_and_only_then_resolves`
/// failed in the suite while passing alone). No reason to assume gui's test
/// binary is safe from the same trap just because it hasn't hit it yet.
pub(super) fn scratch_path(fixture: &str, seed: u32) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!("feral_processes_gui_{fixture}_{seed}_{unique}.sav"))
}

/// Deletes the scratch file it guards when it drops, unwind included — the
/// same guard app-core's `stand_in_base_at` uses, and for the same reason:
/// `save`, `load_from_file`, `save_to_file` and `Game::load` all sit between
/// this guard's construction and the plain `remove_file` a straight-line
/// version would use at the end, and every one of them can panic. This repo
/// has a recorded history of `/tmp` inode exhaustion from exactly that shape
/// of leak, so cleanup has to survive a panic partway through the fixture,
/// not just the happy path.
pub(super) struct RemoveOnDrop<'a>(pub &'a Path);

impl Drop for RemoveOnDrop<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0);
    }
}

/// A freshly created `Game`, the way `App::new` + `Game::new` build one in
/// production — no character-creation wizard, since these fixtures are not
/// testing the wizard and `render::tests::census_app` already covers that
/// path. Mirrors app-core's own `test_app`.
fn new_game(seed: u32) -> Game {
    let assets_dir = test_assets_dir();
    Game::new(seed, DifficultyMode::Forgiving, &assets_dir)
        .expect("the shipped asset tree builds a fresh game")
}

/// A `Game` carrying two tamed programs at zone 1, named for what each is
/// standing in for: one free to spend, one wielded.
///
/// **Why one is wielded rather than simply absent.** The picker's own
/// wiring — `Game::programs_for_build`, handed straight to
/// `party::companion_page_rows` in `building::draw_build_program` — is not
/// the roster's full membership; a fixture carrying only a *qualifying*
/// program cannot tell `programs_for_build` apart from `Game::owned_pets`,
/// since both would draw it. `programs_for_build` drops a wielded program
/// (`crates/engine/src/game/party.rs`'s `!p.wielded` filter); `owned_pets`
/// does not. Carrying one program only one of the two derivations keeps is
/// what makes a test built on this fixture able to catch the picker reading
/// the wrong one.
pub(super) fn game_with_a_free_and_a_wielded_program(seed: u32) -> Game {
    let mut game = new_game(seed);
    let path = scratch_path("build_program_candidates", seed);
    let _cleanup = RemoveOnDrop(&path);
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    let program = |wielded: bool, name: &str| CreatureSave {
        sortie_index: None,
        boss: false,
        species: species.clone(),
        position: (px, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 2,
        tamed: true,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 10,
        cronjob: None,
        party_slot: None,
        wielded,
        zone: 1,
        custom_name: Some(name.to_string()),
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        ring: 0,
        talents: Vec::new(),
        bought_stats: Default::default(),
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
        field_buffs: Vec::new(),
        nest_position: None,
        patrol_position: None,
        pursuing: false,
        carrying: None,
        rarity: Default::default(),
        nemesis_grudges: 0,
        equipment: Vec::new(),
        program_id: 0,
        disposition: None,
        disgruntled: None,
        memories: Vec::new(),
        needs: Default::default(),
        off_shift: None,
        staff: false,
        downed: false,
    };
    data.creatures.push(program(false, "Free Sparkgrub"));
    data.creatures.push(program(true, "Wielded Sparkgrub"));
    save::save_to_file(&path, &data).unwrap();
    Game::load(&path, &test_assets_dir()).unwrap()
}

/// An `App` with a founded base and a Compiler standing two cells east of
/// it, the party in base space beside it — a real, scan-reachable structure
/// for `App::upgradeable_structures` to find. Exists so a test can drive
/// `Mode::BuildProgram` off a genuine `PendingBuild::Upgrade`, and so
/// `building::build_commit`'s upgrade arm resolves a name off a real
/// `EntityView` rather than one its own `.find` could never have matched.
///
/// Home is founded through `Game::place_structure` — the same real call
/// `App::handle_build_direction_key` makes for it. The Compiler is written
/// straight into the save instead, the way app-core's
/// `app_owning_a_program_and_a_compiler_deep` places one:
/// `Game::place_structure` would spend a program this fixture has no need to
/// own, and the engine exposes no cheaper way to stand a structure up from
/// outside its own crate.
pub(super) fn app_in_base_with_a_compiler(seed: u32) -> App {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    let assets_dir = test_assets_dir();
    let tmp = std::env::temp_dir().join(format!(
        "feral_processes_gui_upgrade_fixture_{seed}_{unique}"
    ));
    let _ = std::fs::remove_dir_all(&tmp);
    std::fs::create_dir_all(&tmp).unwrap();
    let mut app = App::new(
        assets_dir.clone(),
        tmp.join("saves"),
        tmp.join("history.log"),
        tmp.join("profile.ron"),
        arenas_dir(),
        tmp.join("telemetry.jsonl"),
    );
    app.game = Some(new_game(seed));
    {
        let game = app.game.as_mut().expect("just set");
        game.place_structure("home", 0, 0, None)
            .expect("a fresh run can afford its first Home");
        while game.take_notification().is_some() {}
    }

    let path = scratch_path("upgrade_compiler", seed);
    let _cleanup = RemoveOnDrop(&path);
    app.game.as_mut().expect("just set").save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.locale = Locale::Base { x: 0, y: 0 };
    data.structures.push(save::StructureSave {
        kind: "compiler".to_string(),
        // Base space, two cells east of the Home the fixture just founded —
        // within `MENU_SCAN_RADIUS` of the party this edit also parks there.
        position: (2, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        hopper: Vec::new(),
        hopper_progress: 0,
    });
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    app
}
