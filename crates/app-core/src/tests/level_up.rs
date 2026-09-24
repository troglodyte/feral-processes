//! `Mode::LevelUp` — opened from `App::show_next_notification` once a
//! level lands and the player is back on the map, ahead of the
//! notification queue that door already gates.

use feral_processes_engine::arena::{OpponentSpec, PlayerSource, Scenario};
use feral_processes_engine::notifications::NotificationKind;
use feral_processes_engine::progression::xp_for_level;
use feral_processes_engine::save;

use super::support::*;
use crate::*;

/// Weakens every wild program the world spawned to `hp`/`max_hp` — through
/// the save, the only door app-core has onto the engine's `World` (see
/// `tests::battle::weaken_every_wild`, restated here since it is private to
/// that module).
fn weaken_every_wild(app: &mut App, hp: i32) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("level_up_weaken", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    for creature in data.creatures.iter_mut().filter(|c| !c.tamed) {
        creature.hp = hp;
        creature.max_hp = hp;
    }
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// Sets the player's `xp` one short of its threshold, so the next kill's
/// XP — never zero, `progression::kill_xp`'s `.max(1)` — crosses a level.
///
/// **Not `xp_to_next`**: `Game::load` derives that field from `level`
/// (`crate::game::lifecycle`'s "derived, not read back" — `PlayerSave::
/// xp_to_next` is a stale second copy of `xp_for_level(level)` kept only so
/// removing it would cost a `SAVE_FORMAT_VERSION` bump), so writing it here
/// would be silently discarded on the very load this fixture reloads
/// through. The engine exposes no public setter either way:
/// `Game::award_player_xp` is `pub(crate)`, `tests::tools::app_with_known_
/// tool`'s reason for the same save-edit idiom.
fn about_to_level(app: &mut App) {
    let assets_dir = test_assets_dir();
    let path = scratch_path("level_up_xp", 0);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.xp = xp_for_level(data.player.level).saturating_sub(1);
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
}

/// `tests::battle::battling_app`'s seed search, rigged to level the player
/// on the kill: every wild program is weakened to 1 hp and the player's
/// `xp` set one short of its threshold before the walk into it, so `[R]`
/// both wins and levels in one round.
fn app_about_to_level(seed: u32) -> App {
    for probe in seed..seed + 200 {
        let mut app = test_app(probe);
        weaken_every_wild(&mut app, 1);
        about_to_level(&mut app);
        let game = app.game.as_mut().unwrap();
        let player = game.player_status().position;
        let target = game
            .view_entities(12, 12)
            .into_iter()
            .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
            .find(|e| (e.pos.0 - player.0).abs() + (e.pos.1 - player.1).abs() == 1);
        let Some(target) = target else { continue };
        app.handle_key(match (target.pos.0 - player.0, target.pos.1 - player.1) {
            (1, 0) => GameKey::Right,
            (-1, 0) => GameKey::Left,
            (0, 1) => GameKey::Down,
            _ => GameKey::Up,
        });
        if app.mode == Mode::Battle {
            let _ = app.take_sounds();
            app.finish_reveal();
            return app;
        }
    }
    panic!("no seed under 200 from {seed} put a lone wild program next to the player");
}

/// Wins the staged fight at once and leaves its results for the map —
/// `tests::battle::r_resolves_the_fight_and_opens_the_results_with_nothing_
/// unrevealed`'s own two keys.
fn win_it_and_leave_results(app: &mut App) {
    app.handle_key(GameKey::Char('R'));
    assert_eq!(app.mode, Mode::BattleResult, "{:?}", app.status_line);
    app.handle_key(GameKey::Enter);
}

fn queue(app: &mut App, kind: NotificationKind) {
    assert!(
        app.game
            .as_mut()
            .expect("a fixture with a game")
            .notify(kind),
        "an Always notification queues every time"
    );
}

/// The spec's first test: levelling in a fight, then leaving the results
/// screen, opens the page.
#[test]
fn levelling_in_a_fight_then_leaving_results_opens_the_page() {
    let mut app = app_about_to_level(9501);
    let starting_level = app.game.as_ref().unwrap().player_status().level;

    win_it_and_leave_results(&mut app);

    assert_eq!(app.mode, Mode::LevelUp, "{:?}", app.status_line);
    let report = app
        .pending_level_up
        .as_ref()
        .expect("the page is open with nothing to show");
    assert_eq!(report.from_level, starting_level);
    assert!(report.to_level > starting_level);
}

/// The spec's second test: a notification queued while the page is open
/// waits behind it and shows on `Esc`.
#[test]
fn a_queued_notification_waits_behind_the_page_and_shows_on_esc() {
    let mut app = app_about_to_level(9502);
    queue(&mut app, NotificationKind::Breach);

    win_it_and_leave_results(&mut app);
    assert_eq!(
        app.mode,
        Mode::LevelUp,
        "the level-up page takes the screen ahead of the queue"
    );
    assert!(app.pending_notification.is_none());

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Notification, "{:?}", app.status_line);
    assert!(app.pending_notification.is_some());
    assert!(app.pending_level_up.is_none());
}

/// The spec's third test, plus correction 9: `P` opens `Mode::Perks`, and
/// `Esc` from there — `menu_origin` untouched by the level-up page — falls
/// through `App::close_screen` to `Mode::Playing`, not back to a page that
/// is already gone.
#[test]
fn p_opens_perks_and_esc_from_there_returns_to_playing() {
    let mut app = app_about_to_level(9503);
    win_it_and_leave_results(&mut app);
    assert_eq!(app.mode, Mode::LevelUp, "{:?}", app.status_line);

    app.handle_key(GameKey::Char('P'));
    assert_eq!(app.mode, Mode::Perks);
    assert!(app.pending_level_up.is_none());

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// A lowercase `p` is a no-op — lowercase letters are row selectors on
/// every other menu, and this page has no rows for one to select.
#[test]
fn a_lowercase_p_does_nothing() {
    let mut app = app_about_to_level(9504);
    win_it_and_leave_results(&mut app);
    assert_eq!(app.mode, Mode::LevelUp, "{:?}", app.status_line);

    app.handle_key(GameKey::Char('p'));

    assert_eq!(app.mode, Mode::LevelUp);
    assert!(app.pending_level_up.is_some());
}

/// An arena session never opens the page: `after_tick` returns before
/// `show_next_notification` while `in_arena()`, so a level earned in a
/// staged fight leaves its report sitting in the engine, undrained, rather
/// than surfacing on `App`.
#[test]
fn an_arena_session_never_opens_the_page() {
    let assets_dir = test_assets_dir();
    let path = scratch_path("arena_level_up", 9505);
    let mut fresh = Game::new(9505, DifficultyMode::Forgiving, &assets_dir).unwrap();
    fresh.save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    // Unkillable so the fight cannot end any other way than a win, and
    // primed to level on the first kill, `about_to_level`'s reason.
    data.player.hp = 1_000_000;
    data.player.max_hp = 1_000_000;
    data.player.xp = xp_for_level(data.player.level).saturating_sub(1);
    save::save_to_file(&path, &data).unwrap();

    let mut app = test_app(9505);
    app.game = None;
    app.mode = Mode::MainMenu;
    app.arena_enabled = true;
    app.handle_key(GameKey::Char('r'));
    assert_eq!(app.mode, Mode::ArenaBuilder, "{:?}", app.status_line);
    let session = app.arena.as_mut().expect("the builder opened a session");
    session.scenario = Scenario {
        player: PlayerSource::Save(path.clone()),
        opponents: vec![OpponentSpec {
            species: "sprite".into(),
            count: 1,
        }],
        seed: 1,
        ..Scenario::default()
    };
    app.handle_key(GameKey::Char('f'));
    assert_eq!(app.mode, Mode::Battle, "{:?}", app.status_line);

    for _ in 0..500 {
        if !app.mode.is_battle() {
            break;
        }
        app.finish_reveal();
        app.handle_key(match app.mode {
            Mode::Battle => GameKey::Char('A'),
            _ => GameKey::Enter,
        });
    }
    assert_eq!(app.mode, Mode::ArenaResult, "the fixture never resolved");
    assert!(
        app.arena
            .as_ref()
            .unwrap()
            .outcome
            .as_ref()
            .is_some_and(|o| o.won),
        "the fixture must actually win, or the kill below proves nothing"
    );

    assert_ne!(
        app.mode,
        Mode::LevelUp,
        "an arena session must never open the page"
    );
    assert!(app.pending_level_up.is_none());
    let report = app.game.as_mut().unwrap().take_level_up_report().expect(
        "the fixture's kill must actually have levelled the player, \
             or this proves nothing",
    );
    assert!(report.to_level > report.from_level);

    let _ = std::fs::remove_file(&path);
}
