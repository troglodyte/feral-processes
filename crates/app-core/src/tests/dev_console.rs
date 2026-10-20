//! The dev console — a keypad for provoking events that are otherwise
//! rare enough to make a visual feature untestable by hand.
//!
//! What is asserted here is *dispatch*: that the gate holds, that the
//! screen opens and closes, and that firing a row reaches the engine. What
//! each trigger actually does to the world is the engine's own suite
//! (`tests/raids.rs`), because `Game::world` is private and app-core cannot
//! set up a structure to raid.

use super::support::*;
use crate::*;
use feral_processes_engine::EnemyStrength;

/// The gate is a field read once in `App::new`, not a live env lookup, so a
/// test can open it without touching an environment the parallel suite
/// shares — the same reasoning `dev_arena_enabled` records.
fn console_app(seed: u32) -> App {
    let mut app = test_app(seed);
    app.enable_dev_console_for_test();
    app
}

#[test]
fn the_console_stays_shut_without_the_dev_flag() {
    let mut app = test_app(1);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "the console must be unreachable in a build a player is running"
    );
}

#[test]
fn the_console_opens_from_the_map_and_esc_returns() {
    let mut app = console_app(2);

    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    assert_eq!(app.mode, Mode::DevConsole);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// Firing a trigger leaves the console standing. Every use of this thing is
/// "do it again and watch harder" — a screen that closed on each press would
/// make a four-key sequence into twelve.
#[test]
fn firing_a_trigger_leaves_the_console_open() {
    let mut app = console_app(3);
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));

    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::DevConsole);
}

/// The renderer draws `rows[i].label` and the handler dispatches
/// `rows[i].action`, so the table has to be the only source of both — the
/// group-menu invariant applied to a smaller screen.
#[test]
fn every_console_row_has_a_label_and_the_selection_stays_in_range() {
    let mut app = console_app(4);
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    let rows = App::dev_console_rows();
    assert!(!rows.is_empty());
    for row in rows {
        assert!(!row.label.is_empty());
    }

    for _ in 0..rows.len() * 2 {
        app.handle_key(GameKey::Down);
        assert!(app.menu_selected < rows.len());
    }
    for _ in 0..rows.len() * 2 {
        app.handle_key(GameKey::Up);
        assert!(app.menu_selected < rows.len());
    }
}

/// Rows are dispatched by their `action`, so two rows sharing one is a row
/// that silently cannot be reached — the copy-paste mistake this table
/// invites, and invisible on screen because both labels still draw.
#[test]
fn no_two_console_rows_fire_the_same_action() {
    let rows = App::dev_console_rows();
    for (i, row) in rows.iter().enumerate() {
        for other in &rows[i + 1..] {
            assert_ne!(
                row.action, other.action,
                "'{}' and '{}' both fire {:?}",
                row.label, other.label, row.action
            );
        }
    }
}

/// The one trigger whose effect app-core can see for itself: needs decay
/// every tick, so burning cycles has to move them.
#[test]
fn the_tick_trigger_advances_the_world() {
    let mut app = console_app(5);
    let before = app.game.as_ref().unwrap().player_status().power;
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    let row = App::dev_console_rows()
        .iter()
        .position(|r| r.action == DevAction::AdvanceTicks)
        .expect("the table must offer a tick trigger");
    app.menu_selected = row;

    app.handle_key(GameKey::Enter);

    let after = app.game.as_ref().unwrap().player_status().power;
    assert!(
        after < before,
        "burning {DEV_CONSOLE_TICKS} cycles should have moved Power: {before} -> {after}"
    );
}

/// The one row that changes the run rather than provoking an event in it.
/// A press climbs the ladder and says where it landed.
#[test]
fn the_strength_trigger_climbs_the_ladder_and_reports_it() {
    let mut app = console_app(6);
    assert_eq!(
        app.game.as_ref().unwrap().enemy_strength(),
        EnemyStrength::Standard,
        "a run starts on the floor"
    );
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    app.menu_selected = App::dev_console_rows()
        .iter()
        .position(|r| r.action == DevAction::CycleEnemyStrength)
        .expect("the table must offer the strength trigger");

    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.game.as_ref().unwrap().enemy_strength(),
        EnemyStrength::Elevated
    );
    let line = app.status_line.clone().expect("a press has to report back");
    assert!(
        line.contains("Elevated"),
        "the band it landed on is the one thing the press must say: {line}"
    );
}

/// The ladder wraps, so one row reaches every rung — including back to the
/// floor, which is the only way a dev undoes a press.
#[test]
fn cycling_past_the_top_returns_to_the_floor() {
    let mut app = console_app(7);
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    app.menu_selected = App::dev_console_rows()
        .iter()
        .position(|r| r.action == DevAction::CycleEnemyStrength)
        .unwrap();

    for _ in 0..EnemyStrength::all().len() {
        app.handle_key(GameKey::Enter);
    }

    assert_eq!(
        app.game.as_ref().unwrap().enemy_strength(),
        EnemyStrength::Standard,
        "a full lap must come back to the floor"
    );
}

/// The master switch's resolution, exercised as the pure decision it is.
///
/// `resolve_tool_flag` is split from `dev_tool_flag` precisely so these can
/// run without `set_var` — the parallel suite shares one environment, and a
/// test that wrote `FERAL_DEV` would decide the gate for whatever else was
/// running at the time. The same split `launcher::paths::layout` makes for
/// `FERAL_ASSETS_DIR`, for the same reason.
mod master_flag {
    use crate::app::dev_console::resolve_tool_flag;
    use std::ffi::OsStr;

    /// Reads as "is this tool on, given its own value and the master's".
    fn on(specific: Option<&str>, master: Option<&str>) -> bool {
        resolve_tool_flag(specific.map(OsStr::new), master.map(OsStr::new))
    }

    #[test]
    fn a_player_build_sets_neither_and_gets_nothing() {
        assert!(!on(None, None));
    }

    #[test]
    fn the_master_flag_alone_opens_a_tool() {
        assert!(
            on(None, Some("1")),
            "FERAL_DEV=1 is the whole point: one name for the everyday case"
        );
    }

    #[test]
    fn a_specific_flag_alone_still_opens_its_own_tool() {
        assert!(
            on(Some("1"), None),
            "every script and doc that names a single flag must keep working"
        );
    }

    #[test]
    fn a_specific_zero_vetoes_the_master() {
        assert!(
            !on(Some("0"), Some("1")),
            "FERAL_DEV=1 FERAL_DEV_ARENA=0 is how one tool is left out"
        );
    }

    #[test]
    fn a_master_zero_opens_nothing() {
        assert!(!on(None, Some("0")));
    }

    /// Empty is unset, not off — the rule `dev_flag` has always had, and the
    /// one `paths::layout` cites. So it falls through rather than vetoing.
    #[test]
    fn an_empty_specific_value_falls_through_to_the_master() {
        assert!(on(Some(""), Some("1")));
        assert!(!on(Some(""), Some("0")));
        assert!(!on(Some(""), None));
    }

    /// The `!= "0"` half is a single-value exception, not a truthiness
    /// parser: `dev_flag` has never accepted "false" as off and this must
    /// not quietly start.
    #[test]
    fn any_other_value_reads_as_on() {
        assert!(on(Some("true"), None));
        assert!(on(None, Some("false")));
        assert!(on(Some("00"), None));
    }
}

/// Opens the keypad with the cursor on `action`'s row and presses Enter.
fn fire_row(app: &mut App, action: DevAction) {
    app.handle_key(GameKey::Char(DEV_CONSOLE_KEY));
    app.menu_selected = App::dev_console_rows()
        .iter()
        .position(|r| r.action == action)
        .expect("the table must offer this row");
    app.handle_key(GameKey::Enter);
}

#[test]
fn the_free_build_row_toggles_and_its_label_shows_the_state() {
    let mut app = console_app(8);
    let row = App::dev_console_rows()
        .iter()
        .find(|r| r.action == DevAction::ToggleFreeBuild)
        .expect("the table must offer free build");
    assert!(app.dev_row_label(row).ends_with("(off)"));

    fire_row(&mut app, DevAction::ToggleFreeBuild);

    assert!(app.game.as_ref().unwrap().dev_free_build());
    assert!(app.dev_row_label(row).ends_with("(on)"));
    assert_eq!(app.mode, Mode::DevConsole);
    assert!(app.status_line.is_some(), "a toggle has to say what it did");

    fire_row_again(&mut app);
    assert!(!app.game.as_ref().unwrap().dev_free_build());
}

/// Enter on the row the cursor is already on.
fn fire_row_again(app: &mut App) {
    app.handle_key(GameKey::Enter);
}

#[test]
fn the_other_rows_keep_their_static_label() {
    let app = console_app(9);
    for row in App::dev_console_rows()
        .iter()
        .filter(|r| r.action != DevAction::ToggleFreeBuild)
    {
        assert_eq!(app.dev_row_label(row), row.label);
    }
}

#[test]
fn the_companion_row_adopts_one_or_says_why_not() {
    let mut app = console_app(10);
    let before = app.game.as_mut().unwrap().manifest_roster().len();

    fire_row(&mut app, DevAction::SpawnCompanion);

    let line = app.status_line.clone().expect("a press has to report back");
    let after = app.game.as_mut().unwrap().manifest_roster().len();
    if line.ends_with("is yours") {
        assert_eq!(after, before + 1);
    } else {
        assert_eq!(after, before, "a refusal adopts nothing: {line}");
    }
}

/// The two dev pickers: the item picker (rows from `Game::dev_plain_items`,
/// scrolled by `menu_selected`) and the zone picker (`dev_warp_targets`).
mod pickers {
    use super::*;

    fn open_item_picker(app: &mut App) {
        fire_row(app, DevAction::GiveItem);
    }

    #[test]
    fn the_item_row_opens_the_picker_and_esc_returns_to_the_keypad() {
        let mut app = console_app(11);
        open_item_picker(&mut app);
        assert_eq!(app.mode, Mode::DevItemPicker);

        app.handle_key(GameKey::Esc);

        assert_eq!(app.mode, Mode::DevConsole);
    }

    #[test]
    fn picking_an_item_gives_ten_and_stays_open() {
        let mut app = console_app(12);
        let (id, _, held) = app.game.as_ref().unwrap().dev_plain_items()[0].clone();
        open_item_picker(&mut app);

        app.handle_key(GameKey::Char('1'));

        assert_eq!(app.mode, Mode::DevItemPicker);
        let now = app
            .game
            .as_ref()
            .unwrap()
            .dev_plain_items()
            .into_iter()
            .find(|(i, _, _)| *i == id)
            .unwrap()
            .2;
        assert_eq!(now, held + 10);
        assert!(app.status_line.is_some());
        let rows = app.dev_item_rows();
        assert_eq!(rows[0].2, held + 10, "the row's count follows the gift");
    }

    #[test]
    fn scrolling_exposes_the_rows_past_the_last_selector() {
        let mut app = console_app(13);
        let total = app.game.as_ref().unwrap().dev_plain_items().len();
        assert!(total > 36, "the picker only needs scrolling with 36+ items");
        open_item_picker(&mut app);
        assert_eq!(app.dev_item_rows().len(), 35);
        assert_eq!(app.dev_item_rows().last().unwrap().0, 'z');

        for _ in 0..total {
            app.handle_key(GameKey::Down);
        }

        let rows = app.dev_item_rows();
        let last = app.game.as_ref().unwrap().dev_plain_items()[total - 1].clone();
        assert_eq!(
            rows.last().unwrap().1,
            last.1,
            "the list's end is reachable"
        );
        assert_eq!(rows.len(), 35);

        app.handle_key(GameKey::Char('z'));
        let gave = app
            .game
            .as_ref()
            .unwrap()
            .dev_plain_items()
            .into_iter()
            .find(|(i, _, _)| *i == last.0)
            .unwrap()
            .2;
        assert_eq!(
            gave,
            last.2 + 10,
            "a selector picks within the visible page"
        );
    }

    #[test]
    fn the_zone_picker_lists_zones_ahead_and_a_pick_warps_and_closes() {
        let mut app = console_app(14);
        let targets = app.game.as_ref().unwrap().dev_warp_targets();
        fire_row(&mut app, DevAction::WarpZone);
        assert_eq!(app.mode, Mode::DevZonePicker);
        assert_eq!(app.dev_zone_rows()[0].1, targets[0]);

        app.handle_key(GameKey::Char('1'));

        assert_ne!(app.mode, Mode::DevZonePicker, "a pick closes the picker");
        assert_eq!(
            app.game.as_ref().unwrap().dev_warp_targets()[0],
            targets[0] + 1,
            "the breach ran forward one zone"
        );
        assert!(app.status_line.is_some());
    }

    #[test]
    fn a_refused_warp_is_said_and_leaves_the_picker_open() {
        let mut app = crate::tests::battle::battling_app();
        app.enable_dev_console_for_test();
        app.mode = Mode::DevZonePicker;

        app.handle_key(GameKey::Char('1'));

        assert_eq!(app.mode, Mode::DevZonePicker);
        assert!(app.status_line.as_deref().unwrap().contains("battle"));
    }

    #[test]
    fn esc_on_the_zone_picker_returns_to_the_keypad() {
        let mut app = console_app(16);
        fire_row(&mut app, DevAction::WarpZone);
        app.handle_key(GameKey::Esc);
        assert_eq!(app.mode, Mode::DevConsole);
    }
}
