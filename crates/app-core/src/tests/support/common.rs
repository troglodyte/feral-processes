//! Fixtures shared by the app-core tests: common.

use std::sync::atomic::{AtomicU32, Ordering};

use feral_processes_engine::save::{self};

use super::*;
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

/// Stands an already-built `App`'s party on the field at the Home's tile.
///
/// Through a save round trip because the engine's `World` is private to it,
/// which is why every fixture in this file that moves the party writes the
/// save rather than reaching in. For a test about something else that has to
/// be *at home* to do it: party assignment is a base verb, so the fixture
/// stands where the player would.
pub(crate) fn stand_inside_the_base(app: &mut App) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("into_base", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    stand_save_on_field(&mut data, 0, 0);
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// Presses `key` and spends the one tick `update_realtime` owes it at the
/// current `world_speed` — `travel-on-the-clock`'s replacement for a test
/// that used to read `handle_key` alone as "the player moved." An unpaused
/// arrow on the map only *queues* a step now; this is the shortest way for
/// a test that doesn't care about the clock itself to still see it land.
pub(crate) fn walk(app: &mut App, key: GameKey) {
    app.handle_key(key);
    app.update_realtime(1.0 / app.world_speed.ticks_per_second());
}

/// Bumps the run to zone 2 and overrides a `len`-tile-deep, `2 * band +
/// 1`-tile-wide block east of the player with `biome` —
/// `travel-on-the-clock`'s drag and weather-damage tests (task A and task
/// B) need real ground with a real `EnvironmentEffect`, which only exists
/// past zone 1 (`Game::environment_biome_at`'s gate).
///
/// The engine exposes no public door onto `WorldMap`, so this goes through
/// the same save-edit-reload trick every other fixture in this file uses,
/// writing `SaveData::tile_overrides` directly rather than
/// `WorldMap::set_override` (`pub(crate)` to the engine).
///
/// **A block, not a single row.** `Game::travel_step`'s route is built over
/// `walk_field`'s Chebyshev cost — a diagonal step costs the same as an
/// orthogonal one — so two neighbours can tie for "toward the goal" and
/// which one wins the tie is not this fixture's to predict. `band` widens
/// the override far enough off the row that whichever neighbour the router
/// actually picks still lands on the ground under test, and a travel test
/// asserts on *that* rather than on a specific tile the party crosses.
///
/// Which of the overridden tiles actually carries an effect is seed- and
/// position-specific — a `GroundCondition` claims a whole
/// `tuning::CONDITION_CELL_TILES` block keyed off `(seed, zone, biome,
/// block)`, and `Game::condition_at` that decides it is `pub(crate)` too —
/// so each call site picks a seed and an offset found by hand and says so.
pub(crate) fn override_biome_stretch_east(
    app: &mut App,
    biome: feral_processes_engine::world::Biome,
    len: i32,
    band: i32,
) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("biome_stretch", 0);
    let game = app.game.as_mut().unwrap();
    game.warp_to_zone(2).unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    for i in 1..=len {
        for j in -band..=band {
            data.tile_overrides.push((
                (px + i, py + j),
                feral_processes_engine::world::Tile {
                    biome,
                    walkable: true,
                    rock_shade: None,
                },
            ));
        }
    }
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    // `warp_to_zone` queues an achievement ("First Escalation") that only
    // resolves onto `Notifications` from inside a *tick* — a plain
    // `take_notification` drain here, before anything has ticked, sees
    // nothing. Left alone it takes `Mode::Playing` on the caller's own
    // first `update_realtime`, which reads as the walk itself being
    // interrupted when it is really this fixture's own setup doing it. So
    // this spends ticks and dismisses whatever they queue until the run
    // is quiet, before handing the app back — capped rather than an
    // unbounded loop, so a genuine regression here fails loudly instead of
    // hanging the suite.
    let tick = 1.0 / app.world_speed.ticks_per_second();
    for _ in 0..20 {
        app.update_realtime(tick);
        if app.mode == Mode::Notification {
            app.handle_key(GameKey::Esc);
        } else {
            break;
        }
    }
    assert_eq!(
        app.mode,
        Mode::Playing,
        "override_biome_stretch_east could not settle the run back onto Mode::Playing"
    );
}

/// Clears a box around the player of every wild creature, nest, surface
/// link and settlement — a movement or travel test's guarantee that a short
/// walk can't be interrupted by whatever the seed's world generation
/// happened to place nearby, wandering into the path included (clearing
/// only the exact row a walk would cross is not enough: `WanderAi` and
/// `pursuit_tick` both run every real tick, seed 2602's own reason a
/// same-row clear alone still landed a battle three ticks into a plain
/// eastward walk). `place_wild_program_east`'s row-clearing step, widened
/// to a box and without the placement that follows it.
pub(crate) fn clear_the_area_around_player(app: &mut App) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("clear_area", 0);
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let (px, py) = data.player.position;
    const RADIUS: i32 = 15;
    let near = |x: i32, y: i32| (x - px).abs() <= RADIUS && (y - py).abs() <= RADIUS;
    data.creatures.retain(|c| !near(c.position.0, c.position.1));
    data.nests.retain(|n| !near(n.position.0, n.position.1));
    data.link_sites.retain(|&(x, y)| !near(x, y));
    data.settlements.0.retain(|_, s| !near(s.tile.0, s.tile.1));
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// Drops the app's player to zero hp on a Permadeath run, through a save at
/// `path` (left in place), since a save is the only door app-core has onto
/// the engine's `World`. The loaded game gets the app's profile installed
/// back, or its first `flush_profile_writes` would clobber the app's copy
/// with the empty one `Game::load` leaves. The caller ticks, as a killing
/// blow would.
pub(crate) fn zero_the_players_hp(app: &mut App, path: &Path) {
    app.game.as_mut().unwrap().save(path).unwrap();
    let mut data = save::load_from_file(path).unwrap();
    data.player.hp = 0;
    data.difficulty = DifficultyMode::Permadeath;
    save::save_to_file(path, &data).unwrap();
    let mut game = Game::load(path, &test_assets_dir()).unwrap();
    game.install_profile(app.profile.clone());
    app.game = Some(game);
}

/// Puts `qty` of a plain `item` into the pack by editing the save, for gear
/// that `Game::dev_give_item` refuses.
pub(crate) fn put_in_pack(app: &mut App, item: &str, qty: u32) {
    let path = scratch_path("put_in_pack", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let _cleanup = RemoveOnDrop(&path);
    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory.push((ItemId::from(item), qty));
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
}

/// Reaches the manual's index the only way the player can: the game menu's
/// Manual page, from the map (Esc) or the main menu (`[O]`).
pub(crate) fn open_manual(app: &mut App) {
    if app.mode == Mode::MainMenu {
        app.handle_key(GameKey::Char('o'));
    } else {
        app.handle_key(GameKey::Esc);
    }
    for _ in 0..8 {
        if app.mode == Mode::ManualOptions {
            break;
        }
        app.handle_key(GameKey::Down);
    }
    assert_eq!(app.mode, Mode::ManualOptions, "no Manual page in the menu");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Help);
}
