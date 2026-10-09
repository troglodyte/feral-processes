//! The Basin Exit prompt and `Mode::Ending`.

use feral_processes_engine::resources::Locale;
use feral_processes_engine::save;

use super::support::*;
use crate::*;

fn structure(kind: &str, x: i32, y: i32) -> save::StructureSave {
    save::StructureSave {
        kind: kind.to_string(),
        position: (x, y),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
        pod_charged: None,
        incubating: Vec::new(),
    }
}

/// A base with an exit one step east of the party, all ten keys held and
/// zone 10 reached. Through the save, the only door app-core has onto either.
fn app_beside_the_exit(keys: u16) -> App {
    app_beside_the_exit_seeded(5511, keys)
}

fn app_beside_the_exit_seeded(seed: u32, keys: u16) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    let path = scratch_path("basin_exit", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.locale = Locale::Base { x: 1, y: 0 };
    data.zone = 10;
    data.player.phase_keys.held = keys;
    data.structures.push(structure("basin_exit", 2, 0));
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
    drain_notifications(&mut app);
    app
}

fn story_complete(app: &App) -> bool {
    app.game.as_ref().unwrap().phase_keys().story_complete
}

#[test]
fn stepping_onto_the_exit_asks_first() {
    let mut app = app_beside_the_exit(0x3FF);
    walk(&mut app, GameKey::Right);
    assert_eq!(app.mode, Mode::BasinExitConfirm);
    assert!(
        !story_complete(&app),
        "nothing happens until it is answered"
    );
}

#[test]
fn declining_stays_and_changes_nothing() {
    for key in [GameKey::Char('n'), GameKey::Esc] {
        let mut app = app_beside_the_exit(0x3FF);
        walk(&mut app, GameKey::Right);
        app.handle_key(key);
        dismiss_notifications(&mut app);
        assert_eq!(app.mode, Mode::Playing);
        assert!(!story_complete(&app));
    }
}

#[test]
fn confirming_opens_the_ending_and_sets_the_flag() {
    let mut app = app_beside_the_exit(0x3FF);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('y'));
    assert_eq!(app.mode, Mode::Ending);
    assert_eq!(app.ending_page, 0);
    assert!(!app.ending_screens.is_empty());
    assert!(story_complete(&app));
}

#[test]
fn the_ending_pages_forward_back_and_then_resumes_play() {
    let mut app = app_beside_the_exit(0x3FF);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('y'));
    let pages = app.ending_screens.len();
    assert!(pages > 1, "the shipped ending is several screens");

    app.handle_key(GameKey::Left);
    assert_eq!(app.ending_page, 0, "cannot go before the first page");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.ending_page, 1);
    app.handle_key(GameKey::Left);
    assert_eq!(app.ending_page, 0);
    for _ in 0..pages {
        assert_eq!(app.mode, Mode::Ending);
        app.handle_key(GameKey::Enter);
    }
    assert_eq!(app.mode, Mode::Escaped, "the score card sits before play");
    assert!(app.ending_screens.is_empty());
    app.handle_key(GameKey::Enter);
    assert!(matches!(app.mode, Mode::Playing | Mode::Notification));
}

#[test]
fn esc_skips_the_rest_of_the_ending() {
    let mut app = app_beside_the_exit(0x3FF);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('y'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Escaped);
    app.handle_key(GameKey::Esc);
    assert!(matches!(app.mode, Mode::Playing | Mode::Notification));
}

#[test]
fn a_standing_exit_without_the_keys_is_refused_with_no_prompt() {
    let mut app = app_beside_the_exit(0x1FF);
    walk(&mut app, GameKey::Right);
    dismiss_notifications(&mut app);
    assert_eq!(app.mode, Mode::Playing);
    assert!(!story_complete(&app));
}

#[test]
fn play_goes_on_after_the_ending() {
    let mut app = app_beside_the_exit(0x3FF);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('y'));
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    dismiss_notifications(&mut app);
    let tick = app.game.as_ref().unwrap().current_tick();
    walk(&mut app, GameKey::Left);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.game.as_ref().unwrap().current_tick() > tick);
}

// Run score banking. Each test builds its app from its own seed, and
// `test_app` keys the profile file on the seed, so no two tests share one.

fn banked(app: &App) -> u64 {
    feral_processes_engine::achievements::Profile::load(&app.profile_path)
        .0
        .lifetime_score
}

fn card_total(app: &mut App) -> u64 {
    app.game.as_mut().unwrap().score_card().total
}

/// Drops the player to zero hp through a save and ticks, as a killing blow
/// would. Mirrors `saves.rs`' flatline fixture.
fn flatline(app: &mut App) {
    let path = scratch_path("score_flatline", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.hp = 0;
    // Forgiving respawns; only Permadeath ends the run.
    data.difficulty = feral_processes_engine::DifficultyMode::Permadeath;
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
    // The tick `death_handling_system` reacts to, then the same check the
    // map's key handler makes after it.
    app.game.as_mut().unwrap().wait();
    app.check_game_over();
}

#[test]
fn a_death_banks_the_run_score_into_the_profile_file() {
    let mut app = app_beside_the_exit_seeded(5601, 0x3FF);
    let total = card_total(&mut app);
    assert!(total > 0, "ten keys have to be worth something");
    assert_eq!(banked(&app), 0);
    flatline(&mut app);
    assert_eq!(app.mode, Mode::GameOver);
    assert_eq!(banked(&app), card_total(&mut app));
    assert!(banked(&app) >= total);
    assert_eq!(app.profile().lifetime_score, banked(&app));
}

#[test]
fn an_escape_then_a_death_sums_to_the_card_total() {
    let mut app = app_beside_the_exit_seeded(5602, 0x3FF);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('y'));
    assert_eq!(app.mode, Mode::Ending);
    let at_escape = banked(&app);
    assert!(at_escape > 0, "the escape banks on the spot");
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    dismiss_notifications(&mut app);
    flatline(&mut app);
    assert_eq!(app.mode, Mode::GameOver);
    let total = card_total(&mut app);
    assert_eq!(banked(&app), total, "no double count, nothing dropped");
}

#[test]
fn an_escape_banks_exactly_once() {
    let mut app = app_beside_the_exit_seeded(5603, 0x3FF);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('y'));
    let once = banked(&app);
    assert_eq!(once, card_total(&mut app));
    app.bank_run_score();
    assert_eq!(
        banked(&app),
        once,
        "the latch is `banked`, not the call site"
    );
}

#[test]
fn abandoning_a_run_banks_nothing() {
    let mut app = app_beside_the_exit_seeded(5604, 0x3FF);
    assert!(card_total(&mut app) > 0);
    app.mode = Mode::QuitRunConfirm;
    app.handle_key(GameKey::Char('q'));
    assert_eq!(app.mode, Mode::MainMenu);
    assert_eq!(banked(&app), 0);
    assert_eq!(app.profile().lifetime_score, 0);
}

/// Quit without saving, reload the pre-escape save, escape again: the first
/// escape banked into the profile but the run on disk never learned it, so
/// the second would have banked the same card a second time.
#[test]
fn an_escape_is_saved_so_a_reload_cannot_bank_it_twice() {
    let mut app = app_beside_the_exit_seeded(5605, 0x3FF);
    let path = scratch_path("escape_seal", 5605);
    app.current_save_path = Some(path.clone());
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('y'));
    assert!(banked(&app) > 0, "the escape banks on the spot");

    let mut reloaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    assert_eq!(
        reloaded.bank_run_score(),
        0,
        "the saved run carries `banked`, so banking again adds nothing"
    );
}
