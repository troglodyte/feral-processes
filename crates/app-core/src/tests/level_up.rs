//! `Mode::LevelUp` — opened from `App::show_next_notification` once a
//! level lands and the player is back on the map, ahead of the
//! notification queue that door already gates.

use feral_processes_engine::arena::{OpponentSpec, PlayerSource, Scenario};
use feral_processes_engine::notifications::NotificationKind;
use feral_processes_engine::progression::xp_for_level;
use feral_processes_engine::save;
use feral_processes_engine::tuning::STAT_POINTS_PER_LEVEL;

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
        walk(
            &mut app,
            match (target.pos.0 - player.0, target.pos.1 - player.1) {
                (1, 0) => GameKey::Right,
                (-1, 0) => GameKey::Left,
                (0, 1) => GameKey::Down,
                _ => GameKey::Up,
            },
        );
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

/// An arena session never opens the page, because a staged fight cannot
/// level anyone (`LevellingFrozen`): the fixture is one XP short of a level,
/// so without the freeze its kill would leave a report in the engine.
#[test]
fn an_arena_session_never_opens_the_page() {
    let assets_dir = test_assets_dir();
    let path = scratch_path("arena_level_up", 9505);
    let mut fresh = Game::new(9505, DifficultyMode::Forgiving, &assets_dir).unwrap();
    fresh.save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    // Unkillable so the fight cannot end any other way than a win, and
    // primed to level on the first kill, `about_to_level`'s reason.
    // Max HP is derived from Parity now, so the headroom is bought there.
    data.player.attributes.insert("parity".into(), 200_000);
    data.player.hp = 1_000_000;
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
    assert!(
        app.game.as_mut().unwrap().take_level_up_report().is_none(),
        "a staged fight must not level the player"
    );

    let _ = std::fs::remove_file(&path);
}

fn allocation_row(app: &App, id: &str) -> usize {
    let want = feral_processes_engine::attributes::AttributeId::from(id);
    app.allocation_rows()
        .iter()
        .position(|row| matches!(row, CreationRow::Attribute { id, .. } if *id == want))
        .unwrap_or_else(|| panic!("no Points row for {id}"))
}

fn stat_points(app: &App) -> u32 {
    app.game.as_ref().unwrap().player_status().stat_points
}

fn attribute(app: &App, id: &str) -> i32 {
    app.game
        .as_ref()
        .unwrap()
        .player_attributes()
        .get(&id.into())
        .unwrap()
}

/// A level-up leaves the report page with points to place: `Enter` opens
/// the Points screen, spending and `Enter` commit through the engine, the
/// commit carries on to Perks while Perk Points are unspent, and `Esc`
/// there returns to the map.
#[test]
fn the_flow_runs_level_up_then_points_then_perks_then_playing() {
    let mut app = app_about_to_level(9505);
    win_it_and_leave_results(&mut app);
    assert_eq!(app.mode, Mode::LevelUp, "{:?}", app.status_line);
    let banked = stat_points(&app);
    assert_eq!(banked, STAT_POINTS_PER_LEVEL);
    let parity_before = attribute(&app, "parity");

    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::AllocateStats, "{:?}", app.status_line);
    assert!(app.pending_level_up.is_none());
    assert_eq!(app.allocation_points_left(), banked);

    app.menu_selected = allocation_row(&app, "parity");
    app.handle_key(GameKey::ShiftRight);
    assert_eq!(
        app.allocation_points_left(),
        0,
        "Shift+Right spends the pool"
    );
    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.mode,
        Mode::Perks,
        "unspent Perk Points carry on to Perks"
    );
    assert_eq!(stat_points(&app), 0);
    assert_eq!(attribute(&app, "parity"), parity_before + banked as i32);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// `Esc` on the Points screen leaves without spending: the points stay
/// banked and no attribute moves.
#[test]
fn esc_on_the_points_screen_keeps_the_points() {
    let mut app = app_about_to_level(9506);
    win_it_and_leave_results(&mut app);
    let banked = stat_points(&app);
    let parity_before = attribute(&app, "parity");

    app.handle_key(GameKey::Enter);
    app.menu_selected = allocation_row(&app, "parity");
    app.handle_key(GameKey::ShiftRight);
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(stat_points(&app), banked);
    assert_eq!(attribute(&app, "parity"), parity_before);
    assert!(app.stat_allocation.is_none());
}

/// Unlike creation's pool, banked points are never lost by leaving, so
/// `Enter` with points still unspent is allowed and they stay banked.
#[test]
fn the_owned_points_screen_can_be_left_with_points_unspent() {
    let mut app = app_about_to_level(9507);
    win_it_and_leave_results(&mut app);
    let banked = stat_points(&app);

    app.handle_key(GameKey::Enter);
    app.menu_selected = allocation_row(&app, "parity");
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Enter);

    assert_ne!(app.mode, Mode::AllocateStats, "{:?}", app.status_line);
    assert_eq!(stat_points(&app), banked - 1);
}

/// Uppercase `S` on the Perks screen opens the Points screen and `Esc`
/// returns to Perks; at no points it is refused through `App::refuse`.
#[test]
fn s_on_the_perks_screen_opens_points_and_is_refused_at_zero() {
    let mut app = test_app(9508);
    app.mode = Mode::Perks;

    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::Perks, "nothing banked, nothing to spend");
    assert!(app.status_line.is_some(), "the refusal must say why");

    let mut app = app_about_to_level(9509);
    win_it_and_leave_results(&mut app);
    app.handle_key(GameKey::Char('P'));
    assert_eq!(app.mode, Mode::Perks);
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::AllocateStats, "{:?}", app.status_line);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::Perks,
        "opened from Perks, Esc goes back to it"
    );
}

/// The screen's preview is the engine formula called: after spending on
/// Parity, its row's Max HP figure moves by exactly what the commit then
/// gives the player.
#[test]
fn the_preview_equals_what_the_commit_delivers() {
    let mut app = app_about_to_level(9510);
    win_it_and_leave_results(&mut app);
    app.handle_key(GameKey::Enter);
    let row = allocation_row(&app, "parity");
    app.menu_selected = row;
    app.handle_key(GameKey::ShiftRight);
    let CreationRow::Attribute { effects, .. } = app.allocation_rows()[row].clone() else {
        panic!("not an attribute row");
    };
    let (_, _, after) = effects
        .into_iter()
        .find(|(stat, _, _)| *stat == feral_processes_engine::attributes::DerivedStat::MaxHp)
        .expect("Parity feeds Max HP");

    app.handle_key(GameKey::Enter);
    let max_hp = app.game.as_ref().unwrap().player_status().max_hp;
    assert_eq!(after, max_hp as f32);
}

/// Perks and gear sit on top of the attributes' derivation, so the preview
/// must carry them too: its "before" is the HUD's figure and its "after" is
/// what the commit delivers, with a perk's Atk receipt held.
#[test]
fn the_preview_carries_what_perks_and_gear_add() {
    let assets_dir = test_assets_dir();
    let mut app = test_app(9511);
    let path = scratch_path("points_preview_bonus", 9511);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.bought_stats.atk = 7;
    data.player.stat_points = STAT_POINTS_PER_LEVEL;
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);

    app.mode = Mode::Perks;
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::AllocateStats, "{:?}", app.status_line);
    let row = allocation_row(&app, "analysis");
    app.menu_selected = row;
    app.handle_key(GameKey::ShiftRight);
    let CreationRow::Attribute { effects, .. } = app.allocation_rows()[row].clone() else {
        panic!("not an attribute row");
    };
    let (_, before, after) = effects
        .into_iter()
        .find(|(stat, _, _)| *stat == feral_processes_engine::attributes::DerivedStat::Atk)
        .expect("Analysis feeds Atk");
    let hud = |app: &App| app.game.as_ref().unwrap().player_status().atk as f32;
    assert_eq!(before, hud(&app));

    app.handle_key(GameKey::Enter);
    assert_eq!(after, hud(&app));
}
