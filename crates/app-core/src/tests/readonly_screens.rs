//! The three screens you open, read, and close: the message history, the
//! structure roster and the recipe chains. Reading one costs no game time,
//! which is most of what these tests are checking.
//!
//! The roster is the one that is no longer purely read-only — Enter staffs
//! the highlighted structure, which `tests::building` covers. What stays true
//! of it here is that *looking* is free.

use super::support::*;
use crate::*;

#[test]
fn l_opens_the_history_and_esc_returns_to_the_map() {
    let mut app = test_app(90);
    app.handle_key(GameKey::Char('L'));
    assert_eq!(app.mode, Mode::History);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn the_base_menu_opens_the_structure_roster_and_esc_backs_into_it() {
    // A deployed structure, because the row is hidden when the roster would
    // be empty (see `App::base_menu_rows`).
    let mut app = app_owning_a_program_and_a_compiler(91, &[]);
    open_via_menu(&mut app, 'b', "Structure roster");
    assert_eq!(app.mode, Mode::Structures);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// Up/Down are the scroll, and the popup's window follows the highlighted
/// row (see `popup_layout`) — so moving the highlight is the whole of
/// scrolling, and it must not fall out of the screen doing it.
#[test]
fn up_and_down_scroll_the_history_without_leaving_it() {
    let mut app = test_app(92);
    // Two rows to scroll between, and they have to be two *different* lines:
    // `r` rests (refused this far from Home, but the refusal is a guaranteed
    // line) and `e` drains a starting Power Cell, each with a guaranteed line
    // whatever the seed rolled onto the map, and no line in common. Pressing
    // `r` twice would fold into a single row — see
    // `repeated_lines_are_one_scrollable_row`.
    app.handle_key(GameKey::Char('r'));
    app.handle_key(GameKey::Char('e'));
    let lines = app
        .game
        .as_ref()
        .unwrap()
        .message_history(MESSAGE_LOG_CAP)
        .len();
    assert!(lines >= 2, "the test needs two rows to scroll between");
    app.handle_key(GameKey::Char('L'));
    // Lines run oldest-first, matching the map's pane, so the newest is the
    // last row — and it is the one the player opened the screen to read.
    assert_eq!(
        app.menu_selected,
        lines - 1,
        "the history opens on its newest line"
    );

    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::History);
    assert_eq!(app.menu_selected, lines - 2, "Up scrolls back in time");
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, lines - 1);
    assert_eq!(app.mode, Mode::History, "scrolling never leaves the screen");
}

/// The screen scrolls the *folded* rows (see `Game::message_history`), so its
/// row count has to come from the same place the renderer's does. Counting raw
/// lines instead would open the screen on a row that isn't drawn and let the
/// highlight run off the end of the list.
#[test]
fn repeated_lines_are_one_scrollable_row() {
    let mut app = test_app(96);
    // Two drains rather than two rests: a charged field rest rolls
    // `REST_AMBUSH_CHANCE`, so the second `r` could be answered by the
    // battle gate instead of repeating the first line.
    app.handle_key(GameKey::Char('e'));
    app.handle_key(GameKey::Char('e'));
    let game = app.game.as_ref().unwrap();
    let raw = game.message_log(MESSAGE_LOG_CAP).len();
    let rows = game.message_history(MESSAGE_LOG_CAP).len();
    assert!(
        rows < raw,
        "two identical lines should fold into one row ({raw} lines, {rows} rows)"
    );

    app.handle_key(GameKey::Char('L'));
    assert_eq!(
        app.menu_selected,
        rows - 1,
        "the history opens on its newest row, counted after the fold"
    );
    app.handle_key(GameKey::Down);
    assert_eq!(
        app.menu_selected, 0,
        "Down from the last folded row wraps to the first"
    );
}

/// The roster is a list of things, not a timeline, so it opens at the top
/// like every other menu — the Home, which the rest of the base is measured
/// from.
#[test]
fn up_and_down_scroll_the_roster_without_leaving_it() {
    let mut app = app_owning_a_program_and_a_compiler(93, &[]);
    open_via_menu(&mut app, 'b', "Structure roster");
    assert_eq!(app.menu_selected, 0, "the roster opens at its first row");
    app.handle_key(GameKey::Down);
    assert_eq!(app.mode, Mode::Structures);
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Structures);
}

/// Looking is free: no tick, and nothing to resolve. A screen that advanced
/// the clock would let the player pass time by staring at a list — and raids,
/// needs and cronjobs all run off that clock.
///
/// Scrolling only. Enter on the roster is now an action with its own cost and
/// its own tests, so pressing it here would be asserting that a *refusal*
/// doesn't tick, which is a weaker claim wearing this one's name.
#[test]
fn scrolling_a_list_does_not_advance_the_game() {
    let mut app = app_owning_a_program_and_a_compiler(94, &[]);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('L'));
    open_via_menu(&mut app, 'b', "Structure roster");
    for key in [
        GameKey::Down,
        GameKey::Down,
        GameKey::Up,
        GameKey::Esc,
        GameKey::Esc,
    ] {
        app.handle_key(key);
    }
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        before,
        "reading a list must not pass game time"
    );
}

/// Nothing on the history is selectable, so the keys that commit a choice
/// everywhere else have nothing to commit here and must not be mistaken for
/// a way out either.
#[test]
fn enter_and_row_shortcuts_do_nothing_on_the_history() {
    let mut app = app_owning_a_program_and_a_compiler(95, &[]);
    app.handle_key(GameKey::Char('L'));
    for key in [GameKey::Enter, GameKey::Char('1'), GameKey::Char('a')] {
        app.handle_key(key);
        assert_eq!(app.mode, Mode::History, "{key:?} should do nothing here");
    }
}

#[test]
fn the_base_menu_opens_the_recipes_screen_and_esc_backs_into_it() {
    let mut app = test_app(97);
    open_via_menu(&mut app, 'b', "Recipes");
    assert_eq!(app.mode, Mode::Recipes);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// Scrolling moves per *chain*, not per drawn line — the steps under a
/// product are sub-rows the highlight skips, the same shape the roster uses.
#[test]
fn up_and_down_scroll_the_recipes_by_chain() {
    let mut app = test_app(99);
    let chains = app.game.as_ref().unwrap().recipe_chains().len();
    assert!(chains >= 2, "the shipped assets have several chains");
    open_via_menu(&mut app, 'b', "Recipes");
    assert_eq!(app.menu_selected, 0, "the list opens at its first chain");

    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Recipes);
    assert_eq!(app.menu_selected, chains - 1, "Up from the first wraps");
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 0);
    assert_eq!(app.mode, Mode::Recipes, "scrolling never leaves the screen");
}

/// Read-only means read-only, same as the other two.
#[test]
fn the_recipes_screen_does_not_advance_the_game() {
    let mut app = test_app(100);
    let before = app.game.as_ref().unwrap().current_tick();
    open_via_menu(&mut app, 'b', "Recipes");
    for key in [
        GameKey::Down,
        GameKey::Enter,
        GameKey::Char('1'),
        GameKey::Char('a'),
    ] {
        app.handle_key(key);
        assert_eq!(app.mode, Mode::Recipes, "{key:?} should do nothing here");
    }
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        before,
        "reading a list must not pass game time"
    );
}

/// A run whose base has made something, written through the save the way
/// every fixture here reaches state the `Game` API does not expose.
fn app_with_base_output(seed: u32) -> App {
    use feral_processes_engine::base_ledger::Event;
    use feral_processes_engine::items::ItemId;
    use feral_processes_engine::save;

    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("base_output", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.base_ledger.fold(
        0,
        1,
        &Event::Extract {
            item: ItemId("core_fragment".to_string()),
            rolled: 5,
            landed: 5,
            ok: true,
        },
    );
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

#[test]
fn the_base_menu_opens_the_output_page_and_esc_backs_into_it() {
    let mut app = app_with_base_output(101);
    open_via_menu(&mut app, 'b', "Base output");
    assert_eq!(app.mode, Mode::BaseOutput);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::BaseMenu, "Esc walks back up one level");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

/// The group menu's contract: a row is offered only when the screen behind
/// it has something to show. A run that has produced nothing has an empty
/// ledger, and an empty page reads as a broken one.
#[test]
fn a_base_that_has_made_nothing_is_not_offered_the_page() {
    let mut app = test_app(102);
    app.handle_key(GameKey::Char('b'));
    assert!(
        !app.base_menu_rows()
            .iter()
            .any(|r| r.label == "Base output"),
        "the row is offered before the base has made anything"
    );
}

/// Reading is free, and nothing but Esc is bound — the page has no scroll,
/// so an arrow must not quietly move a highlight that is never drawn.
#[test]
fn the_output_page_costs_no_time_and_binds_nothing_but_esc() {
    let mut app = app_with_base_output(103);
    let before = app.game.as_ref().unwrap().current_tick();
    open_via_menu(&mut app, 'b', "Base output");
    for key in [
        GameKey::Down,
        GameKey::Up,
        GameKey::Enter,
        GameKey::Char('1'),
    ] {
        app.handle_key(key);
        assert_eq!(app.mode, Mode::BaseOutput, "{key:?} should do nothing here");
    }
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        before,
        "reading a page must not pass game time"
    );
}

// ---------------------------------------------------------------------------
// The compass — a fourth screen you open, read and close, and the one of the
// four that also acts: Enter points it at the highlighted destination.

#[test]
fn u_opens_the_compass_and_esc_returns_to_the_map() {
    let mut app = test_app(120);
    app.handle_key(GameKey::Char('u'));
    assert_eq!(app.mode, Mode::Compass);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn scrolling_the_compass_never_leaves_it() {
    let mut app = test_app(121);
    app.handle_key(GameKey::Char('u'));
    for key in [GameKey::Up, GameKey::Down, GameKey::Down, GameKey::Up] {
        app.handle_key(key);
        assert_eq!(app.mode, Mode::Compass, "{key:?} should only scroll");
    }
}

#[test]
fn enter_points_the_compass_at_the_highlighted_row() {
    let mut app = test_app(122);
    app.handle_key(GameKey::Char('u'));
    let first = app
        .game
        .as_mut()
        .unwrap()
        .compass_targets()
        .remove(0)
        .target;

    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.game
            .as_mut()
            .unwrap()
            .compass_bearing()
            .map(|r| r.target),
        Some(first),
        "the selection is the highlighted row, and it is the engine's to hold"
    );
}

#[test]
fn uppercase_x_clears_the_selection() {
    let mut app = test_app(123);
    app.handle_key(GameKey::Char('u'));
    app.handle_key(GameKey::Enter);
    assert!(app.game.as_mut().unwrap().compass_bearing().is_some());

    app.handle_key(GameKey::Char('X'));

    assert_eq!(
        app.game.as_mut().unwrap().compass_bearing(),
        None,
        "uppercase, because lowercase letters are row selectors"
    );
    assert_eq!(app.mode, Mode::Compass, "clearing is not a way out");
}

#[test]
fn the_selection_survives_closing_and_reopening_the_screen() {
    let mut app = test_app(124);
    app.handle_key(GameKey::Char('u'));
    app.handle_key(GameKey::Enter);
    let chosen = app
        .game
        .as_mut()
        .unwrap()
        .compass_bearing()
        .map(|r| r.target);
    assert!(chosen.is_some());

    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Char('u'));

    assert_eq!(
        app.game
            .as_mut()
            .unwrap()
            .compass_bearing()
            .map(|r| r.target),
        chosen,
        "the selection is saved state, not a property of the screen being open"
    );
}

/// Opening the compass is reading the map, not acting on it — `L`'s rule.
#[test]
fn opening_the_compass_costs_no_game_time() {
    let mut app = test_app(125);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('u'));
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}
