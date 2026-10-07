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
    let mut app = test_app(5511);
    found_the_base(&mut app);
    let path = scratch_path("basin_exit", 5511);
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
    assert!(matches!(app.mode, Mode::Playing | Mode::Notification));
    assert!(app.ending_screens.is_empty());
}

#[test]
fn esc_skips_the_rest_of_the_ending() {
    let mut app = app_beside_the_exit(0x3FF);
    walk(&mut app, GameKey::Right);
    app.handle_key(GameKey::Char('y'));
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
    dismiss_notifications(&mut app);
    let tick = app.game.as_ref().unwrap().current_tick();
    walk(&mut app, GameKey::Left);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.game.as_ref().unwrap().current_tick() > tick);
}
