//! `App::shortcut_sheet`: what holding `?` lists, and that the bars never
//! name a key the sheet leaves out.

use super::support::*;
use crate::*;

fn sheet(area: ShortcutArea) -> ShortcutSheet {
    shortcut_sheet_for(area)
}

fn app_in_base(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base(&mut app);
    app
}

#[test]
fn each_map_area_gets_its_own_sheet() {
    let base = app_in_base(5201);
    assert_eq!(base.shortcut_area(), Some(ShortcutArea::Base));
    let s = base.shortcut_sheet().unwrap();
    assert_eq!(s.title, "SHORTCUTS — BASE");
    assert!(s.lists("v"));

    let mut field = test_app(5202);
    found_the_base(&mut field);
    stand_in_base_at(&mut field, 40, 40);
    assert_eq!(field.shortcut_area(), Some(ShortcutArea::Field));
    let s = field.shortcut_sheet().unwrap();
    assert_eq!(s.title, "SHORTCUTS — FIELD");
    assert!(!s.lists("v"), "v only makes sense in reach of the base");

    let stack = app_underground(5203);
    assert_eq!(stack.shortcut_area(), Some(ShortcutArea::Stack));
    let s = stack.shortcut_sheet().unwrap();
    assert_eq!(s.title, "SHORTCUTS — STACK");
    assert!(s.lists("< >") && !s.lists("c"));
}

/// Base is the field's list with more appended, never fewer.
#[test]
fn base_lists_everything_the_field_does() {
    let field = sheet(ShortcutArea::Field);
    let base = sheet(ShortcutArea::Base);
    for g in &field.groups {
        for (k, _) in &g.keys {
            assert!(base.lists(k), "Base lost {k:?}");
        }
    }
}

#[test]
fn both_battles_are_titled_tactical() {
    assert_eq!(
        sheet(ShortcutArea::TacticalMap).title,
        "SHORTCUTS — TACTICAL"
    );
    assert_eq!(
        sheet(ShortcutArea::ClassicBattle).title,
        "SHORTCUTS — TACTICAL"
    );
}

#[test]
fn the_sheet_is_live_in_the_two_battle_screens() {
    let group = super::battle::battling_app();
    assert_eq!(group.mode, Mode::Battle);
    assert_eq!(group.shortcut_area(), Some(ShortcutArea::ClassicBattle));

    let map = super::tactical::fighting(0);
    assert_eq!(map.mode, Mode::TacticalBattle);
    assert_eq!(map.shortcut_area(), Some(ShortcutArea::TacticalMap));
}

/// Text entry still types `?`: the sheet is `None` off the map.
#[test]
fn there_is_no_sheet_off_the_map() {
    let mut app = test_app(5204);
    app.mode = Mode::MainMenu;
    assert!(app.shortcut_sheet().is_none());
    app.mode = Mode::Help;
    assert!(app.shortcut_sheet().is_none());
    app.mode = Mode::Playing;
    app.handle_key(GameKey::Char('b'));
    assert_ne!(app.mode, Mode::Playing);
    assert!(app.shortcut_sheet().is_none(), "a hub screen");
}

/// The bar and the popup cannot disagree: every key a map bar shows is on
/// the sheet for the area it is shown in.
#[test]
fn every_map_bar_key_is_on_its_areas_sheet() {
    let cases = [
        (MapLocale::Field, ShortcutArea::Field),
        (MapLocale::Base, ShortcutArea::Base),
        (MapLocale::Stack { market: false }, ShortcutArea::Stack),
        (MapLocale::Stack { market: true }, ShortcutArea::Stack),
    ];
    for (locale, area) in cases {
        let sheet = shortcut_sheet_for(area);
        for mining in [false, true] {
            for (key, _) in locale_keybar(locale, mining).into_iter().flatten() {
                assert!(sheet.lists(&key), "{locale:?}: bar key {key:?} not listed");
            }
        }
    }
}

/// The watching and walking bars appear in every map area, so their keys
/// must be on all three sheets.
#[test]
fn the_watching_and_walking_bar_keys_are_on_every_map_sheet() {
    let mut watching = app_owning_distant_programs(5205, 1);
    found_the_base(&mut watching);
    stand_in_base(&mut watching);
    for _ in 0..4 {
        watching.handle_key(GameKey::Char('.'));
    }
    watching.watching = Some(watching.game.as_ref().unwrap().base_staff()[0]);
    assert!(watching.watch_center().is_some(), "fixture is not watching");

    let mut walking = test_app(5206);
    clear_the_area_around_player(&mut walking);
    let (x, y) = walking.game.as_ref().unwrap().player_status().position;
    walking.paused = true;
    walking.travel_to(x + 3, y);

    for app in [&watching, &walking] {
        let bar: Vec<String> = app
            .map_keybar()
            .into_iter()
            .flatten()
            .map(|(k, _)| k)
            .collect();
        assert!(bar.len() >= 3, "{bar:?}");
        for area in [ShortcutArea::Base, ShortcutArea::Field, ShortcutArea::Stack] {
            let sheet = shortcut_sheet_for(area);
            for key in &bar {
                assert!(sheet.lists(key), "{area:?}: bar key {key:?} not listed");
            }
        }
    }
}

/// Handlers win over the manual: each listed map key must be bound
/// somewhere on its screen. Spot-checks the ones a bar never shows.
#[test]
fn listed_keys_are_bound() {
    let mut app = app_in_base(5207);
    for key in ['N', 'u', 'L', 'x', 'm', 'z', 'd', 'a', 't', 'i', 'D'] {
        app.mode = Mode::Playing;
        app.handle_key(GameKey::Char(key));
        assert_ne!(app.mode, Mode::Playing, "{key} did not open anything");
    }
}
