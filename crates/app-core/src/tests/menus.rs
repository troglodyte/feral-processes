//! Row shortcuts and menu navigation.

use super::support::*;
use crate::*;

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
