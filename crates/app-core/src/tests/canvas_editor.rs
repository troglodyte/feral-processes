//! `CanvasEditor`'s own tests — the shared canvas mechanics `IconEditor`
//! and (later) the dev-only sprite editor both compose on. `tests/
//! icon_editor.rs` covers the same verbs through `IconEditor`'s key table
//! and must keep passing unchanged; this file is the mechanics on their
//! own terms, plus the three behaviours the brush and the stroke add.

use crate::app::canvas_editor::{CanvasEditor, CanvasFocus, CanvasKey, ICON_UNDO_DEPTH};
use crate::*;
use feral_processes_engine::icon::Canvas;

const EDGE: usize = 8;
const PALETTE_LEN: u8 = 15;

fn editor() -> CanvasEditor {
    CanvasEditor::open(Canvas::new(EDGE), PALETTE_LEN)
}

/// The drawn cell at `(x, y)` as the view reports it.
fn cell(editor: &CanvasEditor, x: u8, y: u8) -> u8 {
    editor.view().cells[y as usize * EDGE + x as usize]
}

/// Walks the cursor to `(x, y)` from wherever it is, at brush 1 — the
/// clamp at each edge is what makes the reset legal.
fn move_cursor_to(editor: &mut CanvasEditor, x: u8, y: u8) {
    assert_eq!(
        editor.view().focus,
        CanvasFocus::Canvas,
        "the cursor only moves while the canvas has focus"
    );
    for _ in 0..EDGE - 1 {
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

#[test]
fn the_editor_opens_on_the_canvas_at_the_origin_with_the_first_colour_and_brush_one() {
    let view = editor().view();
    assert_eq!(view.focus, CanvasFocus::Canvas);
    assert_eq!(view.cursor, (0, 0));
    assert_eq!(view.selected, 1, "index 0 is transparent, not a swatch");
    assert_eq!(view.brush, 1);
    assert_eq!(view.edge, EDGE as u8);
    assert!(view.cells.iter().all(|&p| p == 0));
}

#[test]
fn tab_moves_focus_between_the_canvas_and_the_palette() {
    let mut editor = editor();
    editor.handle_key(GameKey::Tab);
    assert_eq!(editor.view().focus, CanvasFocus::Palette);
    editor.handle_key(GameKey::Tab);
    assert_eq!(editor.view().focus, CanvasFocus::Canvas);
}

#[test]
fn brush_one_moves_and_paints_a_single_cell_same_as_before_brush_existed() {
    let mut editor = editor();
    move_cursor_to(&mut editor, 1, 1);
    editor.handle_key(GameKey::Char(' '));
    assert_eq!(cell(&editor, 1, 1), 1);
    for (x, y) in [(0, 0), (2, 1), (1, 2), (0, 1), (1, 0)] {
        assert_eq!(cell(&editor, x, y), 0, "no other cell moved ({x}, {y})");
    }
}

/// **The substance of this task, part 1.** At brush 2 an arrow moves the
/// cursor a whole brush-width and the landing coordinates are even, and a
/// single paint fills the whole 2x2 block anchored there — never just the
/// cell the cursor nominally sits on.
#[test]
fn the_brush_is_a_footprint_and_a_step() {
    let mut editor = editor();
    editor.set_brush(2);
    assert_eq!(editor.view().cursor, (0, 0));

    editor.handle_key(GameKey::Right);
    assert_eq!(
        editor.view().cursor,
        (2, 0),
        "one Right moves the cursor a whole brush-width, and lands even"
    );
    editor.handle_key(GameKey::Down);
    assert_eq!(editor.view().cursor, (2, 2));

    editor.handle_key(GameKey::Char(' '));
    for (x, y) in [(2, 2), (3, 2), (2, 3), (3, 3)] {
        assert_eq!(cell(&editor, x, y), 1, "the whole 2x2 block is painted");
    }
    for (x, y) in [(1, 2), (4, 2), (2, 1), (2, 4)] {
        assert_eq!(cell(&editor, x, y), 0, "outside the block untouched");
    }
}

/// **Part 2: the no-op guard survives the brush.** A paint whose block
/// already holds `index` in every cell must not record — widened from the
/// existing one-cell guard.
#[test]
fn the_no_op_guard_survives_the_brush() {
    let mut editor = editor();
    editor.set_brush(2);
    editor.handle_key(GameKey::Right); // cursor at (2, 0)
    editor.handle_key(GameKey::Char(' ')); // the one real paint
    for (x, y) in [(2, 0), (3, 0), (2, 1), (3, 1)] {
        assert_eq!(cell(&editor, x, y), 1);
    }

    // Repainting the same block with the same colour must push no further
    // undo entries.
    for _ in 0..10 {
        editor.handle_key(GameKey::Char(' '));
    }
    editor.handle_key(GameKey::Char('u'));
    assert!(
        editor.view().cells.iter().all(|&p| p == 0),
        "one undo must reach all the way back past the ten no-op repaints \
         to the one real paint"
    );
}

/// **Part 3: a stroke is one undo entry.** `begin_stroke` snapshots once;
/// every `paint_at` until `end_stroke` records nothing further, so one undo
/// takes back the whole stroke.
#[test]
fn a_stroke_is_one_undo_entry() {
    let mut editor = editor();
    editor.begin_stroke();
    editor.paint_at(0, 0, 3);
    editor.paint_at(1, 0, 3);
    editor.paint_at(2, 0, 3);
    editor.end_stroke();

    assert_eq!(cell(&editor, 0, 0), 3);
    assert_eq!(cell(&editor, 1, 0), 3);
    assert_eq!(cell(&editor, 2, 0), 3);

    editor.handle_key(GameKey::Char('u'));
    assert!(
        editor.view().cells.iter().all(|&p| p == 0),
        "one undo reaches all the way back to before the whole stroke"
    );
}

/// **The stroke's own snapshot must be lazy.** `begin_stroke` must not push
/// unconditionally: a stroke that never changes the canvas (a click on a
/// cell already holding the selected index — the same thing a swatch pick
/// does, since it never touches the canvas at all) must burn no undo slot.
/// Pre-fix, `begin_stroke` pushed a snapshot of the *already-painted*
/// canvas up front, so this undo landed on that duplicate — identical to
/// the current state — and never reached the real edit underneath it.
#[test]
fn a_stroke_that_changes_nothing_records_no_undo_entry() {
    let mut editor = editor();
    editor.paint_at(0, 0, 3); // the one real edit undo must reach

    editor.begin_stroke();
    editor.paint_at(0, 0, 3); // already index 3: no change
    editor.end_stroke();

    editor.handle_key(GameKey::Char('u'));
    assert_eq!(
        cell(&editor, 0, 0),
        0,
        "one undo must reach the real edit, not a duplicate no-op snapshot \
         the empty stroke pushed on top of it"
    );
}

/// The same proof from the pointer seam's other hit kind: a stroke that
/// only ever picks a swatch never touches the canvas at all, so it must
/// leave the undo history exactly as an empty stroke does above.
#[test]
fn a_stroke_over_a_swatch_pick_alone_records_no_undo_entry() {
    let mut editor = editor();
    editor.paint_at(0, 0, 3); // the one real edit undo must reach

    editor.begin_stroke();
    editor.pick_swatch(5);
    editor.end_stroke();

    editor.handle_key(GameKey::Char('u'));
    assert_eq!(
        cell(&editor, 0, 0),
        0,
        "a swatch-only stroke must push no undo entry to skip past"
    );
}

/// Outside a stroke, `paint_at` still records one entry per real edit — the
/// stroke collapsing three edits into one is the exception, not the new
/// default.
#[test]
fn outside_a_stroke_each_paint_records_its_own_undo_entry() {
    let mut editor = editor();
    editor.paint_at(0, 0, 3);
    editor.paint_at(1, 0, 3);
    editor.handle_key(GameKey::Char('u'));
    assert_eq!(cell(&editor, 1, 0), 0, "the second paint is undone");
    assert_eq!(cell(&editor, 0, 0), 3, "the first is not");
}

#[test]
fn neither_cursor_wraps_at_either_edge() {
    let mut editor = editor();
    editor.handle_key(GameKey::Left);
    editor.handle_key(GameKey::Up);
    assert_eq!(editor.view().cursor, (0, 0));

    let last = (EDGE - 1) as u8;
    for _ in 0..EDGE + 4 {
        editor.handle_key(GameKey::Right);
        editor.handle_key(GameKey::Down);
    }
    assert_eq!(editor.view().cursor, (last, last));
}

#[test]
fn the_palette_cursor_walks_on_all_four_arrows_and_clamps_at_both_ends() {
    let mut editor = editor();
    editor.handle_key(GameKey::Tab);

    editor.handle_key(GameKey::Left);
    assert_eq!(editor.view().selected, 1, "index 0 is not a swatch");
    editor.handle_key(GameKey::Up);
    assert_eq!(editor.view().selected, 1, "already at the floor");

    editor.handle_key(GameKey::Right);
    assert_eq!(editor.view().selected, 2);
    editor.handle_key(GameKey::Down);
    assert_eq!(editor.view().selected, 3);

    for _ in 0..40 {
        editor.handle_key(GameKey::Right);
    }
    assert_eq!(
        editor.view().selected,
        PALETTE_LEN,
        "clamped at the ceiling"
    );
}

#[test]
fn undo_past_an_empty_history_is_a_no_op() {
    let mut editor = editor();
    editor.handle_key(GameKey::Char('u'));
    assert!(editor.view().cells.iter().all(|&p| p == 0));

    editor.handle_key(GameKey::Char(' '));
    editor.handle_key(GameKey::Char('u'));
    editor.handle_key(GameKey::Char('u'));
    assert!(
        editor.view().cells.iter().all(|&p| p == 0),
        "the second undo, past the bottom, must not panic or change anything"
    );
}

/// The history is bounded, so the oldest edits fall off rather than the
/// editor growing without limit — `tests/icon_editor.rs`'s
/// `the_undo_history_is_bounded_at_icon_undo_depth`, generalised.
#[test]
fn the_history_is_capped_at_icon_undo_depth() {
    let edits = ICON_UNDO_DEPTH + 8;
    let mut editor = editor();
    for i in 0..edits {
        editor.paint_at((i % EDGE) as u8, (i / EDGE) as u8, 1);
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
fn set_brush_ignores_anything_but_one_or_two() {
    let mut editor = editor();
    editor.set_brush(5);
    assert_eq!(editor.view().brush, 1);
    editor.set_brush(0);
    assert_eq!(editor.view().brush, 1);
    editor.set_brush(2);
    assert_eq!(editor.view().brush, 2);
}

#[test]
fn pick_swatch_clamps_to_the_palette() {
    let mut editor = editor();
    editor.pick_swatch(0);
    assert_eq!(editor.view().selected, 1);
    editor.pick_swatch(200);
    assert_eq!(editor.view().selected, PALETTE_LEN);
    editor.pick_swatch(7);
    assert_eq!(editor.view().selected, 7);
}

#[test]
fn set_canvas_replaces_the_drawing_wholesale() {
    let mut editor = editor();
    editor.paint_at(0, 0, 3);
    let mut replacement = Canvas::new(EDGE);
    replacement.set(5, 5, 9);
    editor.set_canvas(replacement.clone());
    assert_eq!(editor.canvas(), &replacement);
}

#[test]
fn a_key_the_editor_does_not_bind_is_reported_unhandled() {
    let mut editor = editor();
    assert_eq!(editor.handle_key(GameKey::Char('q')), CanvasKey::Unhandled);
    // Enter and Esc are `IconEditor`'s own — the shared table never binds
    // them, which is how `IconEditor` takes them back.
    assert_eq!(editor.handle_key(GameKey::Enter), CanvasKey::Unhandled);
    assert_eq!(editor.handle_key(GameKey::Esc), CanvasKey::Unhandled);
}
