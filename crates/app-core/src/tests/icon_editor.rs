//! The 8x8 icon editor — its whole key table, driven through
//! `IconEditor::handle_key` and read back through `IconEditor::view`.
//!
//! `the_arrows_act_on_the_focused_panel_alone` is the load-bearing one.
//! Two panels sharing four arrow keys is the reason `GameKey::Tab` exists
//! at all, and the failure it guards against — an arrow painting or moving
//! the cursor while the palette has focus — reads on screen as the editor
//! being possessed rather than as a key doing the wrong thing.

use super::support::*;
use crate::app::icon_editor::{ICON_UNDO_DEPTH, IconEditor, IconEditorOutcome};
use crate::*;
use feral_processes_engine::{ICON_GRID, PlayerIcon};

/// An editor open on a blank canvas — what the wizard hands it for a
/// player who has never drawn one.
fn blank_editor() -> IconEditor {
    IconEditor::open(PlayerIcon::default())
}

/// Walks the cursor to `(x, y)` from wherever it is, using the editor's
/// own keys. The clamp at each edge is what makes the reset legal: seven
/// Lefts and seven Ups land on `(0, 0)` from any cell on the grid.
fn move_cursor_to(editor: &mut IconEditor, x: u8, y: u8) {
    assert_eq!(
        editor.view().focus,
        CanvasFocus::Canvas,
        "the cursor only moves while the canvas has focus"
    );
    for _ in 0..ICON_GRID - 1 {
        editor.handle_key(GameKey::Left);
        editor.handle_key(GameKey::Up);
    }
    for _ in 0..x {
        editor.handle_key(GameKey::Right);
    }
    for _ in 0..y {
        editor.handle_key(GameKey::Down);
    }
    assert_eq!(editor.view().cursor, (x, y));
}

/// The drawn cell at `(x, y)` as the view reports it.
fn cell(editor: &IconEditor, x: u8, y: u8) -> u8 {
    editor.view().cells[y as usize * ICON_GRID + x as usize]
}

#[test]
fn the_editor_opens_on_the_canvas_at_the_origin_with_the_first_colour() {
    let view = blank_editor().view();
    assert_eq!(view.focus, CanvasFocus::Canvas);
    assert_eq!(view.cursor, (0, 0));
    assert_eq!(view.selected, 1, "index 0 is transparent, not a swatch");
    assert!(view.cells.iter().all(|&p| p == 0));
}

#[test]
fn tab_moves_focus_between_the_canvas_and_the_palette() {
    let mut editor = blank_editor();
    editor.handle_key(GameKey::Tab);
    assert_eq!(editor.view().focus, CanvasFocus::Palette);
    editor.handle_key(GameKey::Tab);
    assert_eq!(editor.view().focus, CanvasFocus::Canvas);
}

#[test]
fn arrows_move_the_cursor_while_the_canvas_has_focus() {
    let mut editor = blank_editor();
    editor.handle_key(GameKey::Right);
    editor.handle_key(GameKey::Right);
    editor.handle_key(GameKey::Down);
    assert_eq!(editor.view().cursor, (2, 1));
    editor.handle_key(GameKey::Left);
    editor.handle_key(GameKey::Up);
    assert_eq!(editor.view().cursor, (1, 0));
}

/// The whole reason `Tab` exists: with the palette focused the arrows must
/// walk the swatches and leave the cursor exactly where it was.
#[test]
fn the_arrows_act_on_the_focused_panel_alone() {
    let mut editor = blank_editor();
    editor.handle_key(GameKey::Right);
    editor.handle_key(GameKey::Down);
    let parked = editor.view().cursor;

    editor.handle_key(GameKey::Tab);
    for _ in 0..3 {
        editor.handle_key(GameKey::Right);
    }
    editor.handle_key(GameKey::Down);
    let view = editor.view();
    assert_eq!(view.cursor, parked, "the cursor is not the focused panel");
    assert_eq!(view.selected, 5, "four forward steps from the first swatch");

    editor.handle_key(GameKey::Tab);
    editor.handle_key(GameKey::Right);
    let view = editor.view();
    assert_eq!(view.cursor, (parked.0 + 1, parked.1));
    assert_eq!(view.selected, 5, "the palette is not the focused panel now");
}

#[test]
fn the_cursor_does_not_wrap_at_any_edge() {
    let mut editor = blank_editor();
    editor.handle_key(GameKey::Left);
    editor.handle_key(GameKey::Up);
    assert_eq!(editor.view().cursor, (0, 0));

    let last = (ICON_GRID - 1) as u8;
    for _ in 0..ICON_GRID + 4 {
        editor.handle_key(GameKey::Right);
        editor.handle_key(GameKey::Down);
    }
    assert_eq!(editor.view().cursor, (last, last));
}

#[test]
fn the_palette_selection_does_not_wrap_at_either_end() {
    let mut editor = blank_editor();
    editor.handle_key(GameKey::Tab);
    editor.handle_key(GameKey::Left);
    assert_eq!(editor.view().selected, 1, "index 0 is not a swatch");
    for _ in 0..40 {
        editor.handle_key(GameKey::Right);
    }
    assert_eq!(
        editor.view().selected,
        feral_processes_engine::ICON_PALETTE.len() as u8
    );
}

#[test]
fn space_paints_the_cursor_cell_with_the_selected_colour() {
    let mut editor = blank_editor();
    editor.handle_key(GameKey::Tab);
    editor.handle_key(GameKey::Right);
    editor.handle_key(GameKey::Right);
    editor.handle_key(GameKey::Tab);
    move_cursor_to(&mut editor, 4, 6);
    editor.handle_key(GameKey::Char(' '));
    assert_eq!(cell(&editor, 4, 6), 3);
    assert_eq!(cell(&editor, 0, 0), 0, "no other cell moved");
}

/// Space is not focus-dependent — only the arrows are. Picking a swatch
/// and painting with it without tabbing back is the gesture the two-panel
/// split is supposed to buy, not a second thing to remember.
#[test]
fn space_paints_while_the_palette_has_focus() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 2, 3);
    editor.handle_key(GameKey::Tab);
    editor.handle_key(GameKey::Right);
    editor.handle_key(GameKey::Char(' '));
    assert_eq!(cell(&editor, 2, 3), 2);
}

#[test]
fn backspace_erases_the_cursor_cell() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 5, 5);
    editor.handle_key(GameKey::Char(' '));
    assert_eq!(cell(&editor, 5, 5), 1);
    editor.handle_key(GameKey::Backspace);
    assert_eq!(cell(&editor, 5, 5), 0);
}

#[test]
fn x_clears_the_whole_canvas() {
    let mut editor = blank_editor();
    for x in 0..4u8 {
        move_cursor_to(&mut editor, x, 1);
        editor.handle_key(GameKey::Char(' '));
    }
    editor.handle_key(GameKey::Char('x'));
    assert!(editor.view().cells.iter().all(|&p| p == 0));
}

#[test]
fn u_undoes_the_last_edit() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 1, 1);
    editor.handle_key(GameKey::Char(' '));
    move_cursor_to(&mut editor, 2, 2);
    editor.handle_key(GameKey::Char(' '));
    editor.handle_key(GameKey::Char('u'));
    assert_eq!(cell(&editor, 2, 2), 0, "the second paint is undone");
    assert_eq!(cell(&editor, 1, 1), 1, "the first is not");
}

#[test]
fn u_undoes_a_clear() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 7, 7);
    editor.handle_key(GameKey::Char(' '));
    editor.handle_key(GameKey::Char('x'));
    editor.handle_key(GameKey::Char('u'));
    assert_eq!(cell(&editor, 7, 7), 1);
}

#[test]
fn u_on_an_empty_history_changes_nothing() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 3, 3);
    editor.handle_key(GameKey::Char(' '));
    for _ in 0..5 {
        editor.handle_key(GameKey::Char('u'));
    }
    assert!(editor.view().cells.iter().all(|&p| p == 0));
}

/// Holding `Space` on a cell already the selected colour must not fill the
/// history with nothing — an undo that walks back through no-ops stops
/// reaching the edit the player actually wants back.
#[test]
fn repainting_a_cell_the_colour_it_already_holds_pushes_no_undo_entry() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 1, 1);
    editor.handle_key(GameKey::Char(' '));
    move_cursor_to(&mut editor, 2, 2);
    for _ in 0..20 {
        editor.handle_key(GameKey::Char(' '));
    }
    editor.handle_key(GameKey::Char('u'));
    assert_eq!(cell(&editor, 2, 2), 0);
    editor.handle_key(GameKey::Char('u'));
    assert_eq!(
        cell(&editor, 1, 1),
        0,
        "two undos reach two real edits, not twenty repaints"
    );
}

#[test]
fn erasing_an_already_transparent_cell_pushes_no_undo_entry() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 1, 1);
    editor.handle_key(GameKey::Char(' '));
    move_cursor_to(&mut editor, 5, 5);
    for _ in 0..10 {
        editor.handle_key(GameKey::Backspace);
    }
    editor.handle_key(GameKey::Char('u'));
    assert_eq!(cell(&editor, 1, 1), 0, "one undo reaches the one edit");
}

#[test]
fn clearing_an_already_blank_canvas_pushes_no_undo_entry() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 1, 1);
    editor.handle_key(GameKey::Char(' '));
    editor.handle_key(GameKey::Char('x'));
    for _ in 0..10 {
        editor.handle_key(GameKey::Char('x'));
    }
    editor.handle_key(GameKey::Char('u'));
    assert_eq!(
        cell(&editor, 1, 1),
        1,
        "one undo reaches back past the clear"
    );
}

/// The history is bounded, so the oldest edits fall off rather than the
/// editor growing without limit. Driving more edits than it holds and then
/// undoing past the bottom must land on the state the dropped entries
/// describe, and must not panic.
#[test]
fn the_undo_history_is_bounded_at_icon_undo_depth() {
    let edits = ICON_UNDO_DEPTH + 8;
    let mut editor = blank_editor();
    for i in 0..edits {
        move_cursor_to(&mut editor, (i % ICON_GRID) as u8, (i / ICON_GRID) as u8);
        editor.handle_key(GameKey::Char(' '));
    }
    for _ in 0..ICON_UNDO_DEPTH + 1 {
        editor.handle_key(GameKey::Char('u'));
    }
    let view = editor.view();
    let still_painted = view.cells.iter().filter(|&&p| p != 0).count();
    assert_eq!(
        still_painted,
        edits - ICON_UNDO_DEPTH,
        "the oldest edits are past the bottom of the history and stay drawn"
    );
}

#[test]
fn enter_keeps_what_was_drawn() {
    let mut editor = blank_editor();
    move_cursor_to(&mut editor, 6, 2);
    editor.handle_key(GameKey::Char(' '));
    assert_eq!(editor.handle_key(GameKey::Enter), IconEditorOutcome::Keep);
    let icon = editor.icon();
    assert_eq!(icon.get(6, 2), 1);
    assert!(!icon.is_blank());
}

/// `Esc` discards, and the editor proves it by ending on the icon it
/// opened with rather than on a half-edited canvas the caller would have
/// to remember to throw away.
#[test]
fn esc_discards_and_puts_back_the_icon_it_opened_with() {
    let mut opened_with = PlayerIcon::default();
    opened_with.set(0, 0, 4);
    let mut editor = IconEditor::open(opened_with.clone());
    move_cursor_to(&mut editor, 6, 6);
    editor.handle_key(GameKey::Char(' '));
    editor.handle_key(GameKey::Char('x'));

    let outcome = editor.handle_key(GameKey::Esc);
    assert_eq!(outcome, IconEditorOutcome::Discard);
    assert_eq!(cell(&editor, 0, 0), 4);
    assert_eq!(cell(&editor, 6, 6), 0);
    assert_eq!(*editor.icon(), opened_with, "the wizard reads this back");
}

#[test]
fn a_key_the_editor_does_not_bind_leaves_it_open_and_unchanged() {
    let mut editor = blank_editor();
    let before = editor.view();
    assert_eq!(
        editor.handle_key(GameKey::Char('q')),
        IconEditorOutcome::Open
    );
    assert_eq!(
        editor.handle_key(GameKey::ShiftRight),
        IconEditorOutcome::Open
    );
    assert_eq!(editor.view(), before);
}

#[test]
fn every_bound_key_but_the_two_endings_leaves_the_editor_open() {
    let mut editor = blank_editor();
    for key in [
        GameKey::Tab,
        GameKey::Up,
        GameKey::Down,
        GameKey::Left,
        GameKey::Right,
        GameKey::Char(' '),
        GameKey::Backspace,
        GameKey::Char('u'),
        GameKey::Char('x'),
    ] {
        assert_eq!(
            editor.handle_key(key),
            IconEditorOutcome::Open,
            "{key:?} is an edit, not an ending"
        );
    }
}

#[test]
fn there_is_no_editor_view_while_the_editor_is_not_open() {
    let app = test_app(1);
    assert!(app.icon_editor_view().is_none());
}
