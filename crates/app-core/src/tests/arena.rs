//! The dev arena: the gate, the session, and the screens that edit and
//! fight a scenario.

use feral_processes_engine::arena::Scenario;

use super::support::{test_app, test_assets_dir};
use crate::*;

/// An app sitting on the main menu with the arena gate open.
///
/// The flag is set on the field rather than in the environment on purpose:
/// `std::env` is process-global and the suite runs its cases in parallel, so
/// a test that set `FERAL_DEV_ARENA` would turn the gate on for every other
/// test in flight. `App::new` stays the only reader of the variable.
fn app_with_arena(seed: u32) -> App {
    let mut app = test_app(seed);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.arena_enabled = true;
    app
}

#[test]
fn without_the_dev_flag_the_arena_row_is_absent() {
    let mut app = app_with_arena(1);
    app.arena_enabled = false;

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::MainMenu);
    assert!(app.arena.is_none());
}

#[test]
fn with_the_dev_flag_r_opens_the_builder() {
    let mut app = app_with_arena(2);

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::ArenaBuilder);
    assert!(app.arena.is_some());
}

#[test]
fn a_fresh_session_starts_from_the_default_scenario_unsaved() {
    let mut app = app_with_arena(3);

    app.handle_key(GameKey::Char('r'));

    let session = app.arena.as_ref().unwrap();
    assert_eq!(session.scenario, Scenario::default());
    assert_eq!(session.seed, session.scenario.seed);
    assert!(session.path.is_none(), "a new scenario has no file yet");
    assert!(session.warnings.is_empty());
    assert!(session.watch.is_none());
    assert!(session.outcome.is_none());
}

#[test]
fn esc_from_the_builder_drops_the_session() {
    let mut app = app_with_arena(4);
    app.handle_key(GameKey::Char('r'));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::MainMenu);
    assert!(
        app.arena.is_none(),
        "a scenario outliving its screen would be fought by the next session"
    );
}

#[test]
fn the_arena_needs_no_running_game() {
    // The screen hangs off the main menu, where `App::game` is `None` — so
    // anything it reads has to come from somewhere other than a `Game`.
    let mut app = app_with_arena(5);
    assert!(app.game.is_none());

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::ArenaBuilder);
}

#[test]
fn dev_templates_install_whether_or_not_the_gate_is_open() {
    // The launcher installs unconditionally: the gate decides visibility,
    // and installing only when gated would make one flag mean two things.
    let mut app = test_app(6);
    app.install_dev_templates(DevTemplates {
        names: vec!["extraction".to_string()],
        resolve: |_| Ok(test_assets_dir()),
    });
    assert!(app.dev_templates.is_some());
}
