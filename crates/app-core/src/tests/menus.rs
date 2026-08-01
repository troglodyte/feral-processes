//! Row shortcuts and menu navigation.

use super::support::*;
use crate::*;
use feral_processes_engine::ManifestSubject;

/// The invariant every menu renderer leans on: the key a row advertises
/// is the key that picks it. Twelve research nodes and ten deployable
/// structures both run past the nine rows a digit can address, and rows
/// beyond that used to be unreachable by shortcut entirely.
#[test]
fn every_row_shortcut_selects_the_row_it_labels() {
    let mut app = test_app(920);
    let len = 35;
    for idx in 0..len {
        let shortcut = menu_shortcut(idx);
        assert_eq!(
            app.selected_index(GameKey::Char(shortcut), len),
            Some(idx),
            "row {idx} is labelled [{shortcut}] but that key picks something else"
        );
    }
}

#[test]
fn row_shortcuts_run_digits_first_then_letters() {
    assert_eq!(menu_shortcut(0), '1');
    assert_eq!(menu_shortcut(8), '9');
    assert_eq!(menu_shortcut(9), 'a');
    assert_eq!(menu_shortcut(11), 'c');
    assert_eq!(menu_shortcut(34), 'z');
    assert_eq!(
        menu_shortcut(35),
        '-',
        "past 'z' a row should advertise no key rather than a dead one"
    );
}

/// The main-menu, save, difficulty and demolish-confirm handlers all
/// map letters to their own actions through `.or_else`, which only
/// works while `selected_index` leaves letters alone. They're short
/// menus, so the letter rows never come into play — but nothing else
/// enforces that, so lock it in.
#[test]
fn letters_pick_no_row_in_a_menu_shorter_than_ten_rows() {
    let mut app = test_app(921);
    for len in 1..=DIGIT_ROWS {
        for c in ['a', 'l', 'n', 'q', 'x', 'y', 'f', 'm', 'p'] {
            assert_eq!(
                app.selected_index(GameKey::Char(c), len),
                None,
                "[{c}] must stay free for a {len}-row menu's own shortcuts"
            );
        }
    }
}

#[test]
fn d_opens_the_manifest_picker_and_esc_backs_out() {
    let mut app = test_app(70);
    app.handle_key(GameKey::Char('d'));
    assert_eq!(app.mode, Mode::ManifestPick);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn the_picker_always_offers_you_as_its_first_row() {
    let mut app = test_app(71);
    let player = app.game.as_ref().unwrap().player_status().position;

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Manifest);
    let subject = app.pending_manifest.expect("row 1 picks a subject");
    let view = app.game.as_ref().unwrap().manifest(subject).unwrap();
    assert_eq!(view.name, "You", "you are always the first row");
    let ManifestSubject::Player(p) = view.subject else {
        panic!("row 1 is the player");
    };
    assert_eq!(p.position, player);
}

#[test]
fn the_picker_lists_every_owned_program_after_you() {
    let mut app = app_owning_distant_programs(72, 2);
    let subjects = app.manifest_subjects();
    assert_eq!(subjects.len(), 3, "you plus two programs");

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(app.pending_manifest, Some(subjects[1]));
}

#[test]
fn left_and_right_cycle_the_owned_subjects_and_wrap_at_both_ends() {
    let mut app = app_owning_distant_programs(73, 2);
    let subjects = app.manifest_subjects();
    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.pending_manifest, Some(subjects[0]));

    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(subjects[1]));
    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(subjects[2]));
    app.handle_key(GameKey::Right);
    assert_eq!(
        app.pending_manifest,
        Some(subjects[0]),
        "past the last subject wraps to the first"
    );
    app.handle_key(GameKey::Left);
    assert_eq!(
        app.pending_manifest,
        Some(subjects[2]),
        "before the first subject wraps to the last"
    );
    assert_eq!(app.mode, Mode::Manifest, "cycling never leaves the screen");
}

/// A wild program near the player, and the cardinal direction it lies in.
/// Found by scanning `view_entities` rather than guessing a direction, so the
/// test doesn't depend on where a seed happened to put one.
fn a_wild_program_and_its_direction(app: &mut App) -> (Entity, GameKey) {
    let game = app.game.as_mut().unwrap();
    let player = game.player_status().position;
    let wild = game
        .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .find(|e| e.is_hostile)
        .expect("a fresh map has a wild program within scan radius");
    let (dx, dy) = (wild.pos.0 - player.0, wild.pos.1 - player.1);
    let key = if dx.abs() >= dy.abs() {
        if dx > 0 {
            GameKey::Right
        } else {
            GameKey::Left
        }
    } else if dy > 0 {
        GameKey::Down
    } else {
        GameKey::Up
    };
    (wild.entity, key)
}

#[test]
fn cycling_does_nothing_when_the_subject_is_a_program_you_do_not_own() {
    let mut app = test_app(74);
    // A wild program reached through `i` + direction is not in the owned
    // list, so there is nothing to cycle to.
    let (wild, _) = a_wild_program_and_its_direction(&mut app);

    app.pending_manifest = Some(wild);
    app.mode = Mode::Manifest;
    app.handle_key(GameKey::Right);
    assert_eq!(app.pending_manifest, Some(wild));
    assert_eq!(app.mode, Mode::Manifest);
}

#[test]
fn esc_returns_to_the_picker_when_the_manifest_was_opened_from_it() {
    let mut app = app_owning_distant_programs(75, 1);
    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Manifest);

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::ManifestPick,
        "back to the list you came from"
    );
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "a second Esc leaves for the map");
    assert_eq!(app.pending_manifest, None);
}

/// Reached with `i` there is no list behind the sheet, so Esc goes straight
/// back to the map rather than dropping the player into a picker they never
/// opened — and which wouldn't list the wild program anyway.
#[test]
fn esc_leaves_for_the_map_when_the_manifest_was_opened_from_the_world() {
    let mut app = test_app(77);
    let (_, direction) = a_wild_program_and_its_direction(&mut app);
    app.handle_key(GameKey::Char('i'));
    app.handle_key(direction);
    assert_eq!(app.mode, Mode::Manifest);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.pending_manifest, None);
}

/// The list re-opens on whoever the sheet was showing, not on the row
/// originally picked — after paging with ←/→ those differ, and a highlight
/// that disagrees with the screen you just left reads as a bug.
#[test]
fn returning_to_the_picker_highlights_the_subject_you_were_reading() {
    let mut app = app_owning_distant_programs(78, 2);
    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    let showing = app.pending_manifest;

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::ManifestPick);
    assert_eq!(
        app.menu_selected, 2,
        "the highlight follows the sheet, not the original pick"
    );
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.pending_manifest, showing,
        "Enter on the restored row re-opens the same subject"
    );
}

#[test]
fn inspecting_a_direction_still_lands_on_the_manifest() {
    let mut app = test_app(76);
    let (_, direction) = a_wild_program_and_its_direction(&mut app);

    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::InspectDirection);
    app.handle_key(direction);
    assert_eq!(app.mode, Mode::Manifest);
    assert!(
        app.pending_manifest.is_some(),
        "the inspected program is the manifest's subject"
    );
}

/// Uppercase is reserved for actions, so the trade screens can bind `S`
/// and `B` to sell-one and buy-one without shadowing the rows those
/// letters label. Lowercase keeps picking rows exactly as it did.
#[test]
fn uppercase_letters_pick_no_row() {
    let mut app = test_app(922);
    let len = 35;
    for (lower, idx) in [('a', 9), ('b', 10), ('s', 27), ('z', 34)] {
        assert_eq!(
            app.selected_index(GameKey::Char(lower), len),
            Some(idx),
            "lowercase [{lower}] must still pick row {idx}"
        );
        assert_eq!(
            app.selected_index(GameKey::Char(lower.to_ascii_uppercase()), len),
            None,
            "uppercase [{}] must be free for an action",
            lower.to_ascii_uppercase()
        );
    }
}
