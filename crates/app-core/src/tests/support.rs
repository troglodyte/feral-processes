//! Fixtures shared by the app-core tests.

use std::sync::atomic::{AtomicU32, Ordering};

use feral_processes_engine::affixes::AffixId;
use feral_processes_engine::components::Rarity;
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save::{self, CreatureSave};
use feral_processes_engine::stack::{Dir, FrameSpec, generate};

use crate::*;

pub(crate) fn test_assets_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets")
}

/// The shipped `dev-arenas/`, for fixtures that never open the arena. A
/// test that *saves* a scenario takes a scratch copy instead — see
/// `tests/arena.rs::app_with_scratch_arenas` — since this one is source.
pub(crate) fn arenas_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../dev-arenas")
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
pub(crate) fn scratch_path(fixture: &str, seed: u32) -> PathBuf {
    static NEXT: AtomicU32 = AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, Ordering::Relaxed);
    std::env::temp_dir().join(format!(
        "feral_processes_appcore_{fixture}_{seed}_{unique}.sav"
    ))
}

/// Puts an already-built `App`'s party out of phase, in base space.
///
/// Through a save round trip because that is the only door app-core has
/// onto `resources::Locale` — the engine's `World` is private to it, which
/// is why every fixture in this file that needs base space writes
/// `data.locale` rather than reaching in. For a test about something else
/// that now has to be *at home* to do it: party assignment is a base verb
/// (`Game::require_base`), so the fixture crosses the same way the player
/// does.
pub(crate) fn stand_inside_the_base(app: &mut App) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("into_base", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.locale = Locale::Base { x: 0, y: 0 };
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

pub(crate) fn test_app(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let saves_dir = std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_saves"));
    let history_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}.log"));
    let profile_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_profile.ron"));
    let _ = std::fs::remove_file(&profile_path);
    let mut app = App::new(
        assets_dir.clone(),
        saves_dir,
        history_path,
        profile_path,
        arenas_dir(),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_test_{seed}_telemetry.jsonl"
        )),
    );
    app.game = Game::new(seed, DifficultyMode::Forgiving, &assets_dir).ok();
    app.mode = Mode::Playing;
    app
}

/// A game where the player owns `count` programs parked well outside
/// `MENU_SCAN_RADIUS` — a cronjob worker left at a far-flung node, say.
/// Built by editing a save and reloading it, since the engine deliberately
/// exposes no way to hand-place a tamed program from outside the crate.
pub(crate) fn app_owning_distant_programs(seed: u32, count: i32) -> App {
    distant_programs(seed, |game| {
        let species = game.species_defs()[0].id.clone();
        (0..count).map(|_| species.clone()).collect()
    })
}

/// `app_owning_distant_programs` with the species named per program, for a
/// test that has to tell two rows apart by something the roster decides —
/// work speed, aptitude, class. The default fixture gives every program the
/// same species, which cannot catch a screen that reads row `i`'s facts off
/// program `j`.
pub(crate) fn app_owning_distant_programs_of(seed: u32, species: &[&str]) -> App {
    distant_programs(seed, |_| species.iter().map(|s| s.to_string()).collect())
}

/// The same program the cargo fixture gives, developed: `level` and `ring`
/// written straight onto its save record and reloaded.
///
/// Through the save because nothing in the engine's public API can level a
/// companion or open a ring for free — `award_party_xp` is `pub(crate)` and
/// `open_kernel_ring` charges for it, which is the point of both. Three
/// Privilege Rings ride along in cargo so the ring half of the screen has
/// something to spend.
pub(crate) fn app_owning_a_developed_program(seed: u32, level: u32, ring: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app =
        app_owning_a_program_and_a_compiler_with_cargo(seed, &[], &[("privilege_ring", 3)]);
    let path = scratch_path("developed", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    for c in data.creatures.iter_mut().filter(|c| c.tamed) {
        c.level = level;
        c.ring = ring;
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

/// The shared body: one program per name `pick` returns. It takes the
/// `Game` because the count-based caller wants whichever species the roster
/// happens to list first, and that is not knowable until one is loaded.
fn distant_programs(seed: u32, pick: impl FnOnce(&Game) -> Vec<String>) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("distant", seed);
    let game = app.game.as_mut().unwrap();
    let species = pick(game);
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    for (i, species) in species.iter().enumerate() {
        let i = i as i32;
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px + MENU_SCAN_RADIUS + 10 + i, py),
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
            wielded: false,
            zone: 1,
            custom_name: None,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            ring: 0,
            talents: Vec::new(),
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Default::default(),
            nemesis_grudges: 0,
            equipment: Vec::new(),
            program_id: 0,
            disposition: None,
            memories: Vec::new(),
            needs: Default::default(),
            off_shift: None,
            staff: false,
            downed: false,
        });
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

/// Puts one wild program exactly `east` tiles due east of the player and
/// clears the rest of that row, then returns it.
///
/// `Game::find_target_in_direction` reads the row and nothing beside it, so
/// an inspect test has to *own* that row rather than hope the seed left it
/// clear — and the clearing is what makes the returned entity the answer
/// rather than merely *an* answer. Built by editing a save and reloading it,
/// for the same reason `app_owning_distant_programs` is: the engine exposes
/// no way to hand-place a creature from outside the crate.
pub(crate) fn place_wild_program_east(app: &mut App, east: i32) -> Entity {
    let assets_dir = test_assets_dir();
    let path = scratch_path("wild_east", east as u32);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    // Only the row, because only the row can be seen from here — a wider
    // sweep would hide a ray that had been widened back into a cone.
    data.creatures
        .retain(|c| !(c.position.1 == py && c.position.0 > px));
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        position: (px + east, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 2,
        tamed: false,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 10,
        cronjob: None,
        party_slot: None,
        wielded: false,
        zone: 1,
        custom_name: None,
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        ring: 0,
        talents: Vec::new(),
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
        field_buffs: Vec::new(),
        nest_position: None,
        pursuing: false,
        carrying: None,
        rarity: Default::default(),
        nemesis_grudges: 0,
        equipment: Vec::new(),
        program_id: 0,
        disposition: None,
        memories: Vec::new(),
        needs: Default::default(),
        off_shift: None,
        staff: false,
        downed: false,
    });
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);

    let game = app.game.as_mut().unwrap();
    game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .find(|e| e.pos == (px + east, py) && e.is_hostile)
        .expect("the program was just placed on the eastward row")
        .entity
}

/// A game where the player owns one tamed program carrying `routines` and
/// has a Compiler standing, so the extraction flow has both of its
/// preconditions. Built by editing a save and reloading it, for the same
/// reason `app_owning_distant_programs` is.
pub(crate) fn app_owning_a_program_and_a_compiler(seed: u32, routines: &[&str]) -> App {
    app_owning_a_program_and_a_compiler_with_cargo(seed, routines, &[])
}

/// The same, plus exactly `cargo` in the player's inventory — the refactor
/// flow needs a program *and* something to spend on it, and the second half
/// is the one that changes during a run.
pub(crate) fn app_owning_a_program_and_a_compiler_with_cargo(
    seed: u32,
    routines: &[&str],
    cargo: &[(&str, u32)],
) -> App {
    app_owning_a_program_and_a_compiler_deep(seed, routines, cargo, false)
}

/// The same again, optionally four frames down. `underground` is a parameter
/// rather than a second fixture because the point of the Stack variant is
/// that *nothing else about the game differs* — a row that changes has
/// changed because of the locale and not because two fixtures drifted.
pub(crate) fn app_owning_a_program_and_a_compiler_deep(
    seed: u32,
    routines: &[&str],
    cargo: &[(&str, u32)],
    underground: bool,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("extract", seed);
    found_the_base(&mut app);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // Extended, never assigned over: replacing the whole inventory would
    // silently delete the starting kit `Game::new` grants, and the next test
    // written against this fixture would fail for a reason with nothing to do
    // with what it was testing.
    data.player
        .inventory
        .extend(cargo.iter().map(|(item, qty)| (ItemId::from(*item), *qty)));
    let (px, py) = data.player.position;
    data.creatures.push(CreatureSave {
        sortie_index: None,
        boss: false,
        species,
        position: (px + 1, py),
        hp: 10,
        max_hp: 10,
        atk: 3,
        mitigation: 1,
        tamed: true,
        power: 100.0,
        level: 1,
        xp: 0,
        xp_to_next: 20,
        cronjob: None,
        party_slot: None,
        wielded: false,
        zone: 1,
        custom_name: None,
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        ring: 0,
        talents: Vec::new(),
        routines: routines.iter().map(|r| r.to_string()).collect(),
        field_buffs: Vec::new(),
        nest_position: None,
        pursuing: false,
        carrying: None,
        rarity: Default::default(),
        nemesis_grudges: 0,
        equipment: Vec::new(),
        program_id: 0,
        disposition: None,
        memories: Vec::new(),
        needs: Default::default(),
        off_shift: None,
        staff: false,
        downed: false,
    });
    data.structures.push(save::StructureSave {
        kind: "compiler".to_string(),
        // Base space, two cells east of the Home the fixture just founded.
        position: (2, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
    });
    if underground {
        data.locale = Locale::Stack {
            depth: 1,
            frames: 2,
            x: 1,
            y: 1,
            facing: feral_processes_engine::stack::Dir::North,
            entrance: data.player.position,
        };
    }
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
        sortie_index: None,
        boss: false,
        species,
        position: (px + 2, py),
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
        wielded: false,
        zone: 1,
        custom_name: None,
        hp_roll: 1.0,
        atk_roll: 1.0,
        def_roll: 1.0,
        growth_roll: 1.0,
        fusions: 0,
        refactors: 0,
        purchased_tiers: 0,
        ring: 0,
        talents: Vec::new(),
        routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
        field_buffs: Vec::new(),
        nest_position: None,
        pursuing: false,
        carrying: None,
        rarity: Default::default(),
        nemesis_grudges: 0,
        equipment: Vec::new(),
        program_id: 0,
        disposition: None,
        memories: Vec::new(),
        needs: Default::default(),
        off_shift: None,
        staff: false,
        downed: false,
    });
    for n in 0..posts {
        data.structures.push(save::StructureSave {
            kind: "market".to_string(),
            // Base space, spread along -y so none lands on the Home at the
            // origin, the player, or each other.
            position: (1, -n),
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: Vec::new(),
            standing_work: false,
            standing_guard: false,
        });
    }
    // A trader is a deployed `Structure`, and every structure stands in base
    // space — so a player at a counter is a player out of phase, and buying
    // and selling are `Game::require_base`.
    data.locale = Locale::Base { x: 0, y: 0 };
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// Founds the run's base: deploys the first Home through the public API,
/// which stands it on base space's own origin and lays the starting pocket
/// around it.
///
/// Every fixture that hand-places a structure into the base needs this
/// first. A structure standing on unmined rock has no station tile beside
/// it, `Game::broker_reach` reads the floor under the party, and
/// `place_structure` refuses a cell with no floor — so a base written
/// straight into a save with no pocket under it is not a base.
///
/// Deployed rather than written into the save because founding is the one
/// build made from the open grid, and it is the only thing that lays floor
/// this slice.
pub(crate) fn found_the_base(app: &mut App) {
    app.game
        .as_mut()
        .expect("a fixture with a game")
        .place_structure("home", 0, 0)
        .expect("a fresh run can afford its first Home, and founds from the open grid");
}

/// Puts `app`'s party out of phase, inside base space, by the same
/// save-edit-reload trick `app_underground` uses.
pub(crate) fn stand_in_base(app: &mut App) {
    stand_in_base_at(app, 0, 0);
}

/// Deletes the `.sav` it was built from when it drops, unwind included —
/// `save`, `load_from_file`, `save_to_file` and `Game::load` all sit
/// between this guard's construction and the plain `remove_file` a
/// straight-line version would use at the end, and every one of them can
/// panic. This repo has a recorded history of `/tmp` inode exhaustion from
/// exactly that shape of leak, so the cleanup has to survive a panic partway
/// through, not just the happy path.
struct RemoveOnDrop<'a>(&'a std::path::Path);

impl Drop for RemoveOnDrop<'_> {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.0);
    }
}

/// The same, on a chosen base-space cell — for a test that needs the party
/// somewhere other than the exit, the pocket's edge most often.
pub(crate) fn stand_in_base_at(app: &mut App, x: i32, y: i32) {
    // Counted rather than keyed on a seed, the same as `app_underground`:
    // tests run in parallel and several share a seed.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let path = std::env::temp_dir().join(format!("feral_processes_appcore_base_{unique}.sav"));
    let _cleanup = RemoveOnDrop(&path);
    let game = app.game.as_mut().expect("a fixture with a game");
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.locale = Locale::Base { x, y };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
}

/// A game where the player has `routines` installed (in place of the
/// default `decompile`) and `hunger` set to a chosen level, so a field-routine
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
    data.player.power = hunger;
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An app whose player carries `routines` and is standing on depth 1 of a
/// Stack frame, at the frame's own entry cell facing north.
///
/// Written into the save rather than walked to: `Game::enter_stack` is
/// `pub(crate)` in the engine and app-core is outside that boundary, and a
/// fixture that hunted the zone for a surface link would be a test about
/// world generation.
pub(crate) fn app_underground_with_routines(seed: u32, routines: &[&str]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("field_stack", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.routines = routines.iter().map(|r| r.to_string()).collect();
    data.locale = feral_processes_engine::resources::Locale::Stack {
        depth: 1,
        frames: 2,
        // Overwritten below by the frame's own entry — the save only has to
        // put the party *somewhere* legal for the frame to regenerate.
        x: 1,
        y: 1,
        facing: feral_processes_engine::stack::Dir::North,
        entrance: data.player.position,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// Same as `app_with_player_routines`, but the player also owns one program
/// (parked next to them) and one wild, unowned creature is nearby too — for
/// asserting the field-routine ally picker (`Mode::FieldRoutineAlly`) offers the
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
    data.player.power = 100.0;
    let (px, py) = data.player.position;
    for (offset, tamed) in [(1, true), (2, false)] {
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px + offset, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 2,
            tamed,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 10,
            cronjob: None,
            party_slot: None,
            wielded: false,
            zone: 1,
            custom_name: None,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            ring: 0,
            talents: Vec::new(),
            routines: Vec::new(),
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Default::default(),
            nemesis_grudges: 0,
            equipment: Vec::new(),
            program_id: 0,
            disposition: None,
            memories: Vec::new(),
            needs: Default::default(),
            off_shift: None,
            staff: false,
            downed: false,
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
    app_with_companions_and_cargo(seed, count, &[])
}

/// `app_with_companions_in_the_party` with gear in the player's cargo, which
/// is where a companion's loadout comes from and returns to.
pub(crate) fn app_with_companions_and_cargo(
    seed: u32,
    count: u32,
    extra_cargo: &[(&str, u32)],
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("party", seed);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // Extended rather than assigned over, for the reason
    // `app_owning_a_program_and_a_compiler_deep` gives.
    data.player.inventory.extend(
        extra_cargo
            .iter()
            .map(|(item, qty)| (ItemId::from(*item), *qty)),
    );
    let (px, py) = data.player.position;
    for slot in 0..count {
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px, py),
            hp: 30,
            max_hp: 30,
            atk: 3,
            mitigation: 1,
            tamed: true,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 20,
            cronjob: None,
            party_slot: Some(slot),
            wielded: false,
            zone: 1,
            custom_name: None,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            ring: 0,
            talents: Vec::new(),
            routines: vec![feral_processes_engine::abilities::FALLBACK_ABILITY_ID.to_string()],
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Default::default(),
            nemesis_grudges: 0,
            equipment: Vec::new(),
            program_id: 0,
            disposition: None,
            memories: Vec::new(),
            needs: Default::default(),
            off_shift: None,
            staff: false,
            downed: false,
        });
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An `App` standing on the entry cell of Stack frame 1.
///
/// Built by editing a save and reloading it, the same trick
/// `app_owning_distant_programs` uses: the engine deliberately exposes no
/// way to drop the player into the Stack from outside the crate, since on a
/// real run that only ever happens by walking onto an entrance.
pub(crate) fn app_underground(seed: u32) -> App {
    // Counted rather than keyed on the seed alone: tests run in parallel and
    // several share a seed, so a seed-named scratch file has two of them
    // reading and deleting the same path.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path =
        std::env::temp_dir().join(format!("feral_processes_appcore_stack_{seed}_{unique}.sav"));
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let spec = FrameSpec {
        world_seed: data.seed,
        entrance: data.player.position,
        depth: 1,
        frames: 2,
    };
    let entry = generate(spec).entry;
    data.locale = Locale::Stack {
        depth: spec.depth,
        frames: spec.frames,
        x: entry.0,
        y: entry.1,
        facing: Dir::North,
        entrance: spec.entrance,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// `app_underground`, minus every Power Outlet the fresh game starts with —
/// the fixture a "rest is refused for want of a charge" test needs, since
/// `app_underground` inherits a new game's two.
pub(crate) fn app_underground_with_no_rest_charge(seed: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("stack_no_outlet", seed);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player
        .inventory
        .retain(|(id, _)| id.as_str() != feral_processes_engine::items::ids::OUTLET);
    let spec = FrameSpec {
        world_seed: data.seed,
        entrance: data.player.position,
        depth: 1,
        frames: 2,
    };
    let entry = generate(spec).entry;
    data.locale = Locale::Stack {
        depth: spec.depth,
        frames: spec.frames,
        x: entry.0,
        y: entry.1,
        facing: Dir::North,
        entrance: spec.entrance,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game where the player wears `weapon` — an item id plus the gear level
/// it was equipped at (see `components::EquippedItem`) — carries
/// `inventory`, and stands `zone` sectors deep.
///
/// Written straight into the save rather than staged through `Game::equip`,
/// which always stamps gear with the *current* zone level. The gap between
/// the level your weapon remembers and the one the zone would grant it now
/// is exactly what the swap screen's delta column reports, so there is no
/// other way to set it up. `Stats` is deliberately left at its unequipped
/// value: `equip_swap_rows` reads the recorded gear level and the item
/// catalogue, never the player's current attack.
pub(crate) fn app_wearing_weapon(
    seed: u32,
    weapon: Option<(&str, u32)>,
    inventory: &[(&str, u32)],
    zone: u32,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("gear", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory = inventory
        .iter()
        .map(|(item, qty)| (ItemId::from(*item), *qty))
        .collect();
    if let Some((item, level)) = weapon {
        data.player.weapon = Some(ItemId::from(item));
        data.player.weapon_level = level;
    }
    data.zone = zone;
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A game `zone` sectors deep, wearing nothing, carrying one copy of `item`
/// with `affix` on it.
///
/// Written into the save rather than staged through play because nothing
/// public grants an affixed copy: `Game::grant_gear_drop` is the only way one
/// enters the game and it is the engine's own, rolled off `GameRng`. An empty
/// slot is the point — with nothing worn, what the swap picker prints for a
/// candidate is what equipping it must actually grant, with no outgoing item's
/// figure in between.
pub(crate) fn app_carrying_affixed_gear(seed: u32, item: &str, affix: &str, zone: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("affixed", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.zone = zone;
    data.player.gear_copies = vec![(
        save::GearCopySave {
            item: ItemId::from(item),
            rarity: Rarity::Ordinary,
            tier: 0,
            affix: None,
            affixes: vec![AffixId::from(affix)],
            quality: feral_processes_engine::tuning::QUALITY_DEFAULT,
        },
        1,
    )];
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// An ordinary, unfused copy of `item` wearing `affix` — the affixed twin of
/// `gear` above.
pub(crate) fn affixed_gear(item: &str, affix: &str) -> GearCopy {
    GearCopy {
        rarity: Rarity::Ordinary,
        tier: 0,
        affixes: vec![AffixId::from(affix)],
        ..GearCopy::plain(ItemId::from(item))
    }
}

/// Opens a screen the way a player now has to: through its group menu.
///
/// Tests that used to press one retired key go through this instead of
/// hard-coding a row number — the rows are filtered by what is currently
/// possible (see `App::base_menu_rows`), so a row's position depends on the
/// fixture and pinning it would make these tests break for the wrong reason.
pub(crate) fn open_via_menu(app: &mut App, group: char, label: &str) {
    app.handle_key(GameKey::Char(group));
    let rows = match group {
        'b' => app.base_menu_rows(),
        'p' => app.party_menu_rows(),
        other => panic!("{other} is not a group menu key"),
    };
    let labels: Vec<_> = rows.iter().map(|r| r.label).collect();
    let idx = rows
        .iter()
        .position(|r| r.label == label)
        .unwrap_or_else(|| panic!("{label:?} is not offered right now; rows: {labels:?}"));
    app.handle_key(GameKey::Char(menu_shortcut(idx)));
}

/// A base the player is standing inside: a Home one tile north and an
/// ordinary Mining Node one tile east, with nothing to the south or west.
///
/// Built by editing a save for the same reason `app_owning_a_program_and_a_
/// compiler` is — deploying a second structure through the build flow needs
/// materials the player does not start with, and the layout is the whole
/// point here: the direct demolish key has to tell a Home from anything else
/// and both from an empty tile, which is three neighbours of one tile.
pub(crate) fn app_inside_a_small_base(seed: u32, underground: bool) -> App {
    app_inside_a_small_base_with_programs(seed, underground, 0)
}

/// The same base with `programs` tamed programs standing on the player's own
/// tile — what staffing the node from the roster needs, since the roster
/// shows a base and the picker offers a roster, and neither existing fixture
/// had both halves.
pub(crate) fn app_inside_a_small_base_with_programs(
    seed: u32,
    underground: bool,
    programs: usize,
) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("small_base", seed);
    found_the_base(&mut app);
    let game = app.game.as_mut().unwrap();
    let species = game.species_defs()[0].id.clone();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    // The Home is already standing on base space's origin, laid down by
    // `found_the_base`; this is the machine beside it.
    data.structures.push(save::StructureSave {
        kind: "mining_node".to_string(),
        position: (1, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
    });
    for _ in 0..programs {
        data.creatures.push(CreatureSave {
            sortie_index: None,
            boss: false,
            species: species.clone(),
            position: (px, py),
            hp: 10,
            max_hp: 10,
            atk: 3,
            mitigation: 1,
            tamed: true,
            power: 100.0,
            level: 1,
            xp: 0,
            xp_to_next: 20,
            cronjob: None,
            party_slot: None,
            wielded: false,
            zone: 1,
            custom_name: None,
            hp_roll: 1.0,
            atk_roll: 1.0,
            def_roll: 1.0,
            growth_roll: 1.0,
            fusions: 0,
            refactors: 0,
            purchased_tiers: 0,
            ring: 0,
            talents: Vec::new(),
            routines: Vec::new(),
            field_buffs: Vec::new(),
            nest_position: None,
            pursuing: false,
            carrying: None,
            rarity: Default::default(),
            nemesis_grudges: 0,
            equipment: Vec::new(),
            program_id: 0,
            disposition: None,
            memories: Vec::new(),
            needs: Default::default(),
            off_shift: None,
            staff: false,
            downed: false,
        });
    }
    data.locale = if underground {
        Locale::Stack {
            depth: 1,
            frames: 2,
            x: 1,
            y: 1,
            facing: Dir::North,
            entrance: data.player.position,
        }
    } else {
        // *Inside* the base, which is a place of its own now — the fixture's
        // name has always claimed this and the locale is what makes it true.
        // Every base action the callers press a key for is
        // `Game::require_base`.
        //
        // On the exit cell, which is where walking in through the anchor
        // puts you and where walking back out is allowed from — the Home
        // stands on it, and the machine is the cell east.
        Locale::Base { x: 0, y: 0 }
    };
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

/// A carried copy of `item` at fusion `tier`, ordinary rare tier — the
/// app-core twin of the engine's test helper of the same name, and here for
/// the same reason: the screens and handlers now name a whole
/// `items::GearCopy`, and almost every test means the plain one.
pub(crate) fn gear(item: &ItemId, tier: u32) -> GearCopy {
    GearCopy {
        rarity: Rarity::Ordinary,
        tier,
        affixes: Vec::new(),
        ..GearCopy::plain(item.clone())
    }
}

/// A game with a Contract Broker one tile east of the player, standing on a
/// base the player is also standing on — which is what `BrokerReach::AtBroker`
/// asks for, and what a run that owns a Broker looks like anyway, since
/// nothing but a Home may be deployed before a Home is. `underground` drops
/// the party into a Stack frame afterwards, which is what refuses the verbs
/// while leaving the board readable.
///
/// The Home is what puts the floor there: `found_the_base` lays the
/// starting pocket into `BaseGrid` when it deploys one, so a save with a
/// Broker and no Home loads as a base that does not exist.
///
/// Built by editing a save for the reason `app_at_trading_posts` is: the
/// engine exposes no way to hand-place a structure from outside the crate.
pub(crate) fn app_at_a_contract_broker(seed: u32, underground: bool) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("broker", seed);
    found_the_base(&mut app);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    // The Home is already standing, laid down by `found_the_base`; the desk
    // goes up beside it, on the pocket floor it laid.
    data.structures.push(save::StructureSave {
        kind: "contract_broker".to_string(),
        position: (1, 0),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
    });
    data.locale = if underground {
        Locale::Stack {
            depth: 1,
            frames: 2,
            x: 1,
            y: 1,
            facing: feral_processes_engine::stack::Dir::North,
            entrance: data.player.position,
        }
    } else {
        // Standing in the base with the desk, which is what `AtBroker` asks
        // for. One cell north of the Home so nothing shares a tile.
        Locale::Base { x: 0, y: 1 }
    };
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &assets_dir).ok();
    let _ = std::fs::remove_file(&path);
    app
}

/// Takes the party out of the base entirely, without walking there.
///
/// A save round trip rather than ten movement keys, which is what the
/// contracts fixtures already do and for the same reason — the engine hands
/// app-core no way to write a `Position` — plus one of its own: walking ten
/// tiles ticks the world ten times, and a wild program met on the way would
/// answer a question about the contracts screen with a battle.
pub(crate) fn walk_far_from_the_base(app: &mut App) {
    let path = scratch_path("off_base", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    // Out on the zone surface. Distance is not what puts a party off the
    // base any more — the base is a different coordinate space, and being
    // anywhere but in it is the whole of being off it.
    data.locale = Locale::Surface;
    save::save_to_file(&path, &data).unwrap();
    app.game = Game::load(&path, &test_assets_dir()).ok();
    let _ = std::fs::remove_file(&path);
}

/// A party standing in base space with two stocked machines either side of
/// them — the fixture the collect picker's tests are all built on.
///
/// The machines go in through a save edit rather than through play: the
/// engine hands app-core no way to write a `Stock`, which is the seam
/// working rather than a limitation of the test. `mining_node` on both sides
/// so the pair pools into one row, and the western one is the *lower* tile,
/// so a take that spans them has an order to be right about.
/// The party in base space beside `depots` Depots, each already holding
/// `filled` units so the room left is a figure the test chose, and carrying
/// exactly `pack`.
///
/// The pack is **set**, not added to: `Game::new` seeds a starting kit, so a
/// fixture that adds would be measuring the kit as well as its own rows.
pub(crate) fn app_beside_depots(seed: u32, depots: i32, filled: u32, pack: &[(&str, u32)]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("deposit", seed);
    let _cleanup = RemoveOnDrop(&path);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let ballast = if filled > 0 {
        vec![(
            feral_processes_engine::items::ItemId::from("core_fragment"),
            filled,
        )]
    } else {
        Vec::new()
    };
    for x in 0..depots {
        data.structures.push(save::StructureSave {
            kind: "depot".to_string(),
            // Orthogonal to the party's base cell, which is what a collect
            // and a deposit both reach by — see `collect::ORTHOGONAL`.
            position: (if x == 0 { 1 } else { -1 }, 0),
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: ballast.clone(),
            standing_work: false,
            standing_guard: false,
        });
    }
    data.player.inventory = pack
        .iter()
        .map(|(id, n)| (feral_processes_engine::items::ItemId::from(*id), *n))
        .collect();
    data.locale = Locale::Base { x: 0, y: 0 };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    app.mode = Mode::Playing;
    app
}

pub(crate) fn app_beside_stocked_machines(seed: u32, stock: &[(&str, u32)]) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("collect", seed);
    let _cleanup = RemoveOnDrop(&path);
    found_the_base(&mut app);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let output: Vec<(feral_processes_engine::items::ItemId, u32)> = stock
        .iter()
        .map(|(id, n)| (feral_processes_engine::items::ItemId::from(*id), *n))
        .collect();
    for x in [-1, 1] {
        data.structures.push(save::StructureSave {
            kind: "mining_node".to_string(),
            position: (x, 0),
            durability: None,
            tier: None,
            stock_input: Vec::new(),
            stock_output: output.clone(),
            standing_work: false,
            standing_guard: false,
        });
    }
    data.locale = Locale::Base { x: 0, y: 0 };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    app.mode = Mode::Playing;
    app
}
