//! `App::map_keybar`: the map's bottom bar follows what the arrows and
//! `Esc` mean right now, and names only the keys that work where the party
//! stands.

use super::stack_market::app_at_a_market;
use super::support::*;
use crate::*;

fn keys(app: &App) -> Vec<String> {
    app.map_keybar()
        .into_iter()
        .flatten()
        .map(|(k, _)| k)
        .collect()
}

fn label(app: &App, key: &str) -> Option<String> {
    app.map_keybar()
        .into_iter()
        .flatten()
        .find(|(k, _)| k == key)
        .map(|(_, l)| l)
}

fn app_in_base(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    app
}

#[test]
fn base_space_offers_mining_and_trade() {
    let app = app_in_base(5101);
    let keys = keys(&app);
    for key in ["< >", "b", "c", "n", "t", "s"] {
        assert!(keys.contains(&key.to_string()), "{key} missing: {keys:?}");
    }
    assert!(!keys.contains(&"g".to_string()), "{keys:?}");
}

/// `n` is a toggle, so its label says which way it is set — the excavate
/// bar's `plot (on)` convention.
#[test]
fn the_mine_label_says_when_mining_is_on() {
    let mut app = app_in_base(5102);
    assert_eq!(label(&app, "n").as_deref(), Some("mine"));
    app.handle_key(GameKey::Char('n'));
    assert!(app.game.as_ref().unwrap().mining());
    assert_eq!(label(&app, "n").as_deref(), Some("mine (on)"));
}

/// Both refuse on the open grid: the rock and the traders are through the
/// anchor.
#[test]
fn the_open_grid_drops_mining_and_trade() {
    let app = test_app(5103);
    assert!(!app.game.as_ref().unwrap().in_base());
    let keys = keys(&app);
    assert!(!keys.contains(&"n".to_string()), "{keys:?}");
    assert!(!keys.contains(&"t".to_string()), "{keys:?}");
    assert!(keys.contains(&"c".to_string()), "{keys:?}");
}

/// `c` is unbound underground and `n` refuses; `g` and `o` are the Stack's
/// own and nothing else on the screen names them.
#[test]
fn the_stack_names_its_own_keys_and_drops_the_dead_ones() {
    let app = app_underground(5104);
    let keys = keys(&app);
    for dead in ["c", "n", "t"] {
        assert!(!keys.contains(&dead.to_string()), "{dead}: {keys:?}");
    }
    assert_eq!(label(&app, "g").as_deref(), Some("map"));
    assert_eq!(label(&app, "o").as_deref(), Some("adopt"));
}

#[test]
fn a_stack_market_brings_trade_back() {
    let app = app_at_a_market(0);
    assert_eq!(label(&app, "t").as_deref(), Some("trade"));
}

/// While the camera follows a program the arrows and `Esc` hand it back,
/// so the bar says that rather than listing verbs.
#[test]
fn watching_a_program_takes_over_the_bar() {
    let mut app = app_owning_distant_programs(5105, 1);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    for _ in 0..4 {
        app.handle_key(GameKey::Char('.'));
    }
    let staff = app.game.as_ref().unwrap().base_staff()[0];
    app.watching = Some(staff);
    assert!(app.watch_center().is_some(), "fixture is not watching");

    assert_eq!(keys(&app), ["↑↓←→", "Esc", "Space", ","]);
    assert_eq!(label(&app, "Esc").as_deref(), Some("stop watching"));

    app.handle_key(GameKey::Esc);
    assert!(
        keys(&app).contains(&"< >".to_string()),
        "bar not handed back"
    );
}

/// A popup over the map takes `Esc` for itself, so the watch bar's "Esc
/// stop watching" would be a lie there; the camera is still held for when
/// the map is the screen again.
#[test]
fn a_popup_over_a_watched_map_does_not_offer_to_stop_watching() {
    let mut app = app_owning_distant_programs(5108, 1);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    for _ in 0..4 {
        app.handle_key(GameKey::Char('.'));
    }
    let staff = app.game.as_ref().unwrap().base_staff()[0];
    app.watching = Some(staff);
    assert!(app.watch_center().is_some(), "fixture is not watching");

    app.handle_key(GameKey::Char('i'));
    assert_ne!(app.mode, Mode::Playing);
    assert_eq!(app.watching, Some(staff), "the popup released the camera");
    assert!(keys(&app).contains(&"< >".to_string()), "{:?}", keys(&app));
}

/// Any key but a step or the clock's cancels a walk, and the clock keys are
/// the ones that matter while it runs — a travel set while paused waits on
/// Space.
#[test]
fn a_walk_in_progress_takes_over_the_bar() {
    let mut app = test_app(5106);
    clear_the_area_around_player(&mut app);
    let (x, y) = app.game.as_ref().unwrap().player_status().position;
    app.paused = true;
    app.travel_to(x + 3, y);

    assert_eq!(keys(&app), ["Esc", "Space", ","]);
    assert_eq!(label(&app, "Esc").as_deref(), Some("stop walking"));
    assert_eq!(label(&app, "Space").as_deref(), Some("resume"));

    app.handle_key(GameKey::Char(' '));
    assert_eq!(label(&app, "Space").as_deref(), Some("pause"));
    app.handle_key(GameKey::Esc);
    assert!(app.walk.is_none());
    assert!(
        keys(&app).contains(&"< >".to_string()),
        "bar not handed back"
    );
}

/// The plan's bar still outranks the map's, as one group.
#[test]
fn the_excavate_plan_keeps_its_own_bar() {
    let mut app = app_in_base(5107);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(app.mode, Mode::Excavate);
    assert_eq!(app.map_keybar(), vec![app.plan_keybar().unwrap()]);
}

/// Every locale's bar keeps the crossing pair and the screens first, which
/// is what `the_keybar_fits_the_log_pane` measures the cut against.
#[test]
fn every_locale_bar_leads_with_the_crossing_and_the_screens() {
    for locale in [
        MapLocale::Base,
        MapLocale::OpenGrid,
        MapLocale::Stack { market: false },
        MapLocale::Stack { market: true },
    ] {
        for mining in [false, true] {
            let bar = locale_keybar(locale, mining);
            assert_eq!(bar[0], [("< >".into(), "ascend/descend".into())]);
            let screens: Vec<&str> = bar[1].iter().map(|(k, _)| k.as_str()).collect();
            assert_eq!(screens, ["b", "i", "D", "?", "Esc"], "{locale:?}");
        }
    }
}
