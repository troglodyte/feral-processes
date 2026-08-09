//! The profile's file: app-core owns the path, writes on every earn, and
//! never pays a loaded save for what its stats already hold.

use feral_processes_engine::achievements::{Earned, MainStat, Profile};
use feral_processes_engine::tuning::PLAYER_BASE_STATS;

use crate::tests::support::test_assets_dir;
use crate::*;

/// A scratch profile path no other case can be using — the suite runs its
/// cases as concurrent threads, so a shared file would race.
fn scratch_profile(tag: &str) -> PathBuf {
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let path = std::env::temp_dir().join(format!("feral_processes_profile_{tag}_{unique}.ron"));
    let _ = std::fs::remove_file(&path);
    path
}

fn app_with_profile(seed: u32, profile_path: PathBuf) -> App {
    let assets_dir = test_assets_dir();
    let saves_dir =
        std::env::temp_dir().join(format!("feral_processes_appcore_profile_{seed}_saves"));
    let history_path =
        std::env::temp_dir().join(format!("feral_processes_appcore_profile_{seed}.log"));
    App::new(
        assets_dir,
        saves_dir,
        history_path,
        profile_path,
        super::support::arenas_dir(),
        std::env::temp_dir().join("feral_processes_achievements_telemetry.jsonl"),
    )
}

/// A profile holding one stat rung with a known roll, written to `path` — the
/// state a previous run would have left behind.
fn seed_profile(path: &PathBuf) {
    let mut profile = Profile::default();
    profile.record(Earned {
        id: "breach_zone_2".into(),
        first_tick: 3,
        permadeath: false,
        rolled_stat: Some(MainStat::Atk),
    });
    profile.save(path).unwrap();
}

fn player_atk(app: &App) -> i32 {
    app.game.as_ref().unwrap().player_status().atk
}

/// Breaches to zone 2 through the real breach and lets one tick land, which
/// is what earns `breach_zone_2`.
fn breach_and_tick(app: &mut App) {
    app.game.as_mut().unwrap().warp_to_zone(2).unwrap();
    app.handle_key(GameKey::Char('.'));
}

#[test]
fn earning_an_achievement_writes_the_profile_immediately() {
    let path = scratch_profile("immediate");
    let mut app = app_with_profile(41, path.clone());
    app.start_new_game(DifficultyMode::Forgiving);
    breach_and_tick(&mut app);

    assert!(
        path.exists(),
        "the profile must be on disk mid-run — a permadeath run that ends \
         badly must not lose what it proved"
    );
    let (profile, warning) = Profile::load(&path);
    let _ = std::fs::remove_file(&path);
    assert_eq!(warning, None);
    assert!(
        profile
            .earned
            .iter()
            .any(|e| e.id.as_str() == "breach_zone_2"),
        "got {:?}",
        profile.earned
    );
}

#[test]
fn the_written_profile_reloads_equal() {
    let path = scratch_profile("reload");
    let mut app = app_with_profile(42, path.clone());
    app.start_new_game(DifficultyMode::Forgiving);
    breach_and_tick(&mut app);

    let (reloaded, _) = Profile::load(&path);
    let _ = std::fs::remove_file(&path);
    assert_eq!(reloaded.earned, app.profile().earned);
}

#[test]
fn a_new_game_starts_from_the_profile_on_disk() {
    let path = scratch_profile("payout");
    seed_profile(&path);

    let mut app = app_with_profile(43, path.clone());
    app.start_new_game(DifficultyMode::Forgiving);
    let _ = std::fs::remove_file(&path);

    assert_eq!(player_atk(&app), PLAYER_BASE_STATS.atk + 1);
}

/// The trap's end-to-end form: a save already has its bonus baked into
/// `Stats`, so a load that paid again would double it on every reload.
/// Task 5's engine tests only cover the halves.
#[test]
fn loading_a_save_does_not_re_apply_rewards() {
    let path = scratch_profile("noload");
    seed_profile(&path);

    let mut app = app_with_profile(44, path.clone());
    app.start_new_game(DifficultyMode::Forgiving);
    let paid = player_atk(&app);
    assert_eq!(
        paid,
        PLAYER_BASE_STATS.atk + 1,
        "the new game should be paid"
    );

    let save_path = scratch_profile("noload_save").with_extension("bin");
    app.game.as_mut().unwrap().save(&save_path).unwrap();
    app.load_game(save_path.clone());
    let _ = std::fs::remove_file(&save_path);
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        player_atk(&app),
        paid,
        "a load must not pay a second time for what the save already holds"
    );
}

#[test]
fn an_unwritable_profile_path_does_not_crash_the_tick() {
    // A directory standing where the file has to go: the write fails every
    // time, without the test having to guess at permissions.
    let path = std::env::temp_dir().join("feral_processes_profile_unwritable_dir");
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir_all(&path).unwrap();

    let mut app = app_with_profile(45, path.clone());
    app.start_new_game(DifficultyMode::Forgiving);
    breach_and_tick(&mut app);
    let _ = std::fs::remove_dir_all(&path);

    assert_eq!(app.mode, Mode::Playing, "the run should still be going");
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.to_lowercase().contains("profile")),
        "the failure should be reported, got {:?}",
        app.status_line
    );
}

#[test]
fn an_absent_profile_file_is_an_empty_profile_not_an_error() {
    let path = scratch_profile("absent");
    let app = app_with_profile(46, path);

    assert!(app.profile().earned.is_empty());
    assert_eq!(
        app.status_line, None,
        "a first run has no profile, which is not worth a warning"
    );
}

#[test]
fn the_screen_opens_from_the_main_menu_and_esc_returns_to_it() {
    let mut app = app_with_profile(47, scratch_profile("screen"));
    assert_eq!(app.mode, Mode::MainMenu);

    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.mode, Mode::Achievements);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::MainMenu,
        "Esc goes back to the menu it came from"
    );
}

/// The drift guard: app-core owns the row count and gui draws the rows, so
/// both have to come from one call. A renderer that rebuilt the list would
/// let the highlight land on a row that isn't drawn.
#[test]
fn the_screens_row_count_matches_the_report() {
    let path = scratch_profile("rowcount");
    seed_profile(&path);
    let mut app = app_with_profile(48, path.clone());
    let _ = std::fs::remove_file(&path);

    let rows = app.achievement_rows();
    assert_eq!(rows.len(), 13, "every authored rung is listed");
    assert_eq!(
        rows.iter().filter(|r| r.earned.is_some()).count(),
        1,
        "the seeded profile holds exactly one"
    );

    app.handle_key(GameKey::Char('a'));
    for _ in 0..rows.len() + 5 {
        app.handle_key(GameKey::Down);
    }
    assert_eq!(
        app.mode,
        Mode::Achievements,
        "scrolling must not leave the screen"
    );
    assert!(
        app.menu_selected < rows.len(),
        "the highlight stayed inside the drawn rows"
    );
}
