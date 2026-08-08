//! The dev arena: the gate, the session, and the screens that edit and
//! fight a scenario.

use feral_processes_engine::arena::{OpponentSpec, PlayerSource, Scenario};

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

/// A scenario naming `opponents` against a `Fresh` player.
fn scenario(level: u32, zone: u32, opponents: &[(&str, u32)], seed: u64) -> Scenario {
    Scenario {
        player: PlayerSource::Fresh { level, zone },
        opponents: opponents
            .iter()
            .map(|(species, count)| OpponentSpec {
                species: (*species).into(),
                count: *count,
            })
            .collect(),
        seed,
        ..Scenario::default()
    }
}

/// An open arena session already staged into `Mode::Battle`.
fn app_fighting(seed: u32, scenario: Scenario) -> App {
    let mut app = app_with_arena(seed);
    app.handle_key(GameKey::Char('r'));
    let session = app.arena.as_mut().unwrap();
    session.seed = scenario.seed;
    session.scenario = scenario;
    app.handle_key(GameKey::Char('f'));
    app
}

/// A key press that is the test's own input rather than a skipped reveal —
/// narration scrolls in on a frontend's clock, and no frontend is running.
fn press(app: &mut App, key: GameKey) {
    app.finish_reveal();
    app.handle_key(key);
}

fn rounds_seen(app: &App) -> u32 {
    app.arena
        .as_ref()
        .and_then(|s| s.watch.as_ref())
        .map(|w| w.rounds())
        .expect("a staged fight has a watch")
}

/// Plays the fight out with the party's own All-Attack until it lands
/// somewhere that is not the battle screen.
fn fight_to_the_end(app: &mut App) {
    for _ in 0..500 {
        if app.mode != Mode::Battle {
            return;
        }
        press(app, GameKey::Char('A'));
    }
    panic!("the fixture never resolved: mode {:?}", app.mode);
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
fn an_arena_fight_opens_the_battle_screen() {
    let app = app_fighting(10, scenario(6, 1, &[("glitch", 1)], 1));

    assert_eq!(app.mode, Mode::Battle, "{:?}", app.status_line);
    assert!(app.game.as_ref().unwrap().has_active_battle());
    assert_eq!(rounds_seen(&app), 0, "nobody has acted yet");
}

#[test]
fn winning_an_arena_fight_lands_on_the_result() {
    let mut app = app_fighting(11, scenario(20, 1, &[("sprite", 1)], 3));

    fight_to_the_end(&mut app);

    assert_eq!(app.mode, Mode::ArenaResult);
    let record = app.arena.as_ref().unwrap().outcome.as_ref().unwrap();
    assert!(record.won, "{record:?}");
    assert!(record.rounds > 0);
    assert!(
        !record.transcript.is_empty(),
        "the blow-by-blow is what the screen is for"
    );
}

#[test]
fn an_arena_fight_writes_no_save() {
    let mut app = app_fighting(12, scenario(20, 1, &[("sprite", 1)], 3));
    assert!(
        app.current_save_path.is_none(),
        "a session opened from the main menu has no slot to write to"
    );

    fight_to_the_end(&mut app);

    assert!(app.current_save_path.is_none());
    assert!(
        app.list_saves().is_empty(),
        "an arena fight left a save behind"
    );
}

#[test]
fn an_arena_fight_writes_no_profile() {
    // Asserted on the file rather than on `App::profile`, because the
    // omission being tested is the *write*: a rung earned in the arena that
    // reached `profile.ron` would be paid out to every future new game.
    let mut app = app_fighting(13, scenario(30, 1, &[("wintermute", 1)], 5));
    let profile_path = app.profile_path.clone();
    let _ = std::fs::remove_file(&profile_path);

    fight_to_the_end(&mut app);

    assert_eq!(app.mode, Mode::ArenaResult);
    assert!(
        app.arena.as_ref().unwrap().outcome.as_ref().unwrap().won,
        "the fixture is meant to kill the boss, or it asserts nothing"
    );
    assert!(
        !profile_path.exists(),
        "an arena kill was written to the cross-run profile"
    );
}

#[test]
fn an_arena_loss_writes_no_run_history() {
    // A `Save` player source can carry Permadeath in, so a lost arena fight
    // is a reachable `is_game_over` — and it belongs on the result screen.
    let assets_dir = test_assets_dir();
    let path = std::env::temp_dir().join("feral_processes_arena_permadeath.bin");
    Game::new(9, DifficultyMode::Permadeath, &assets_dir)
        .unwrap()
        .save(&path)
        .unwrap();

    let mut app = app_fighting(
        14,
        Scenario {
            player: PlayerSource::Save(path.clone()),
            ..scenario(1, 1, &[("wintermute", 3)], 7)
        },
    );
    let history_path = app.history_path.clone();
    let _ = std::fs::remove_file(&history_path);

    fight_to_the_end(&mut app);
    let _ = std::fs::remove_file(&path);

    assert_eq!(app.mode, Mode::ArenaResult, "not the real Game Over page");
    assert!(
        !history_path.exists(),
        "an arena loss was written to the run history"
    );
}

#[test]
fn a_failed_jack_out_still_counts_its_round() {
    // The regression the unconditional `settle_after_round` fixes: a refused
    // flee resolves a round the tail never saw, and the HP sample for it was
    // lost. `Watch::rounds` is what a missed `observe` loses.
    //
    // A hopeless fight bottoms the escape chance out at `JACK_OUT_CHANCE_MIN`
    // rather than at zero, so the seed that refuses is hunted rather than
    // asserted — deterministically, since the arena seeds `GameRng` itself.
    for scenario_seed in 0..10 {
        let mut app = app_fighting(15, scenario(1, 1, &[("wintermute", 3)], scenario_seed));
        let before = rounds_seen(&app);

        press(&mut app, GameKey::Char('j'));

        if app.mode == Mode::Battle {
            assert!(
                rounds_seen(&app) > before,
                "the round the refused jack-out cost went unobserved"
            );
            return;
        }
    }
    panic!("no seed under 10 refused a jack-out, so this asserts nothing");
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
