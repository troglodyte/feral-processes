//! Which pane of the HUD's info column is open — see `InfoTab`, the digits
//! that pick one, and `App::sync_info_tab_to_locale` for the base-boundary
//! auto-switch layered on top of the manual pick.

use super::support::*;
use crate::*;

/// A fresh `App` before any game exists — the real `start_new_game` /
/// `load_game` doors are what this file's install-time tests are about, so
/// this stops one step short of `test_app`, which assigns `game` directly
/// and so never reaches `App::install_game`'s sync at all.
fn bare_app(seed: u32) -> App {
    App::new(
        test_assets_dir(),
        std::env::temp_dir().join(format!("feral_processes_appcore_infotab_{seed}_saves")),
        std::env::temp_dir().join(format!("feral_processes_appcore_infotab_{seed}.log")),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_infotab_{seed}_profile.ron"
        )),
        arenas_dir(),
        std::env::temp_dir().join(format!(
            "feral_processes_appcore_infotab_{seed}_telemetry.jsonl"
        )),
    )
}

#[test]
fn a_new_game_opens_on_crew_since_a_fresh_run_starts_outside_the_base() {
    let mut app = bare_app(1900);
    app.start_new_game(DifficultyMode::Forgiving, &CharacterChoice::default());
    assert!(
        !app.game.as_ref().unwrap().in_base(),
        "a fresh run has no base to stand in yet"
    );
    assert_eq!(app.info_tab, InfoTab::Crew);
}

#[test]
fn a_loaded_game_opens_on_the_tab_matching_where_it_left_the_party() {
    // `stand_in_base` reaches past `App` onto `Locale` through a save/reload
    // (the only door app-core has), so it is used here purely to build the
    // *save file's* content — the assertion is about `bare_app`'s real
    // `load_game`, not about the fixture's own shortcut.
    let mut source = test_app(1901);
    stand_in_base(&mut source);
    let path = scratch_path("info_tab_loaded", 1901);
    source.game.as_mut().unwrap().save(&path).unwrap();

    let mut app = bare_app(1901);
    app.load_game(path.clone());
    let _ = std::fs::remove_file(&path);

    assert!(app.game.as_ref().unwrap().in_base());
    assert_eq!(app.info_tab, InfoTab::Base);
}

/// The load-bearing crossing behaviour, both directions in one test — the
/// same shape as `the_link_keys_walk_out_of_the_base_and_back_in_through_the_anchor`
/// one file over, which proves `<`/`>` reach `Game::enter_base`/`leave_base`
/// in the first place.
#[test]
fn crossing_the_base_boundary_switches_the_tab_both_ways() {
    // Seeded through the real loader for the same reason as the test
    // above: `app_inside_a_small_base_with_programs` plants `Locale::Base`
    // by editing a save, which bypasses `install_game`'s sync, so the
    // tracked "which side" baseline would not match reality without a real
    // load in between.
    let mut source = app_inside_a_small_base_with_programs(1902, false, 0);
    let path = scratch_path("info_tab_crossing", 1902);
    source.game.as_mut().unwrap().save(&path).unwrap();

    let mut app = bare_app(1902);
    app.load_game(path.clone());
    let _ = std::fs::remove_file(&path);
    assert_eq!(app.info_tab, InfoTab::Base);

    app.handle_key(GameKey::Char('>'));
    assert!(
        !app.game.as_ref().unwrap().in_base(),
        "'>' must reach Game::leave_base"
    );
    assert_eq!(
        app.info_tab,
        InfoTab::Crew,
        "leaving base space must switch the tab to CREW"
    );

    app.handle_key(GameKey::Char('<'));
    assert!(
        app.game.as_ref().unwrap().in_base(),
        "'<' must reach Game::enter_base"
    );
    assert_eq!(
        app.info_tab,
        InfoTab::Base,
        "entering base space must switch the tab back to BASE"
    );
}

/// A manual digit pick is not a location — it must ride out any action that
/// keeps the party on the same side of the base boundary, and only actually
/// crossing that boundary may override it.
#[test]
fn a_manual_pick_survives_an_action_that_stays_on_the_same_side() {
    let mut app = test_app(1903);
    app.handle_key(GameKey::Char('3'));
    assert_eq!(app.info_tab, InfoTab::Pack);

    // A real action that reaches `after_tick` — the same hook the crossing
    // check rides — without leaving the surface.
    app.handle_key(GameKey::Up);

    assert!(!app.game.as_ref().unwrap().in_base());
    assert_eq!(
        app.info_tab,
        InfoTab::Pack,
        "an action that doesn't cross the boundary must not touch a manual pick"
    );
}

#[test]
fn the_digits_pick_a_pane() {
    let mut app = test_app(181);
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.info_tab, InfoTab::Crew);
    app.handle_key(GameKey::Char('3'));
    assert_eq!(app.info_tab, InfoTab::Pack);
    app.handle_key(GameKey::Char('3'));
    assert_eq!(
        app.info_tab,
        InfoTab::Pack,
        "a repeat is a no-op, not a cycle"
    );
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.info_tab, InfoTab::Base);
}

/// The whole reason these arms `return` rather than falling through:
/// changing which pane you are reading is not an action.
#[test]
fn a_digit_costs_no_turn() {
    let mut app = test_app(182);
    let before = app.game.as_ref().unwrap().current_tick();
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
}

/// **The load-bearing one.** `handle_stack_key` ends in `_ => {}`, so a key
/// the Stack path never sees is a swallowed keypress with no refusal and
/// nothing in the log — which is exactly how `r` (rest) shipped broken
/// underground. The column is drawn in both locales, so its keys have to
/// work in both.
#[test]
fn the_digits_work_underground() {
    let mut app = app_underground(183);
    assert!(app.game.as_ref().unwrap().is_underground());

    app.handle_key(GameKey::Char('3'));

    assert_eq!(app.info_tab, InfoTab::Pack);
    assert!(
        app.status_line.is_none(),
        "the key was refused rather than acted on: {:?}",
        app.status_line
    );
}

/// The row the renderer draws and the key that picks it must not disagree —
/// `LogFilter`'s `the_header_order_is_the_cycle_order`, one screen along.
#[test]
fn the_tab_order_is_the_digit_order() {
    for (i, tab) in InfoTab::ALL.iter().enumerate() {
        let mut app = test_app(184 + i as u32);
        let digit = char::from_digit(i as u32 + 1, 10).expect("a tab per digit");
        app.handle_key(GameKey::Char(digit));
        assert_eq!(app.info_tab, *tab, "{digit} did not open {}", tab.label());
    }
}
