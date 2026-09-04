//! The player's 8x8 icon editor: the wizard sink composed on
//! `CanvasEditor`'s shared mechanics.
//!
//! Not a `Mode`, for `creation.rs`'s reason — it hangs off the wizard's
//! Icon step as an `Option<IconEditor>` rather than costing every `Mode`
//! census a row for a screen reachable from exactly one other screen.
//!
//! **`IconEditor` keeps only what is its own**: what it opened with,
//! `Enter`/`Esc`, and the outcome. Every other keystroke — `Tab`, the
//! arrows, `Space`, `Backspace`, `u`, `x` — is `CanvasEditor`'s, taken back
//! here only as `CanvasKey::Unhandled` vs `Handled` and folded into
//! `IconEditorOutcome::Open` either way, since this screen does not care
//! which key it was, only whether it ended the screen.
//!
//! **The bridge between `Canvas` and `PlayerIcon` is a plain cell-by-cell
//! copy**, `canvas_from_icon`/`icon_from_canvas` below — `PlayerIcon`'s
//! codec and palette-range guard stay its own, so `CanvasEditor` never
//! touches a `PlayerIcon` and never needs to know its palette is 15 wide.

use crate::app::canvas_editor::{
    CanvasEditor, CanvasKey, CanvasView, ICON_UNDO_DEPTH as CANVAS_UNDO_DEPTH,
};
use crate::*;
use feral_processes_engine::icon::Canvas;
use feral_processes_engine::{ICON_GRID, ICON_PALETTE, PlayerIcon};

/// How far back `u` reaches — `CanvasEditor`'s own constant, re-exported
/// under its established name here since `tests/icon_editor.rs` names it
/// and must keep passing unchanged. Nothing in production code reads it
/// through this name; only that test import does.
#[allow(dead_code)]
pub(crate) const ICON_UNDO_DEPTH: usize = CANVAS_UNDO_DEPTH;

/// What the icon editor screen draws: `CanvasEditor`'s own view, unwrapped
/// to nothing else — the icon editor has no chrome of its own beyond it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconEditorView {
    pub canvas: CanvasView,
}

/// How one keypress left the editor: still open, or one of the two endings
/// the wizard has to tell apart.
///
/// Neither ending carries the drawing. `IconEditor::icon` is already
/// correct after both — `Esc` puts back what the editor opened with — so a
/// payload would be a 64-byte copy of a value the caller is holding
/// anyway, on an enum built once per keystroke.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum IconEditorOutcome {
    Open,
    Keep,
    Discard,
}

/// Builds the `Canvas` `CanvasEditor` draws from, one cell at a time —
/// `PlayerIcon::get` and `Canvas::set` are both public, so this needs
/// nothing from `PlayerIcon` beyond its ordinary reading API.
fn canvas_from_icon(icon: &PlayerIcon) -> Canvas {
    let mut canvas = Canvas::new(ICON_GRID);
    for y in 0..ICON_GRID {
        for x in 0..ICON_GRID {
            canvas.set(x, y, icon.get(x, y));
        }
    }
    canvas
}

/// `canvas_from_icon`'s inverse — `PlayerIcon::set` already refuses an
/// out-of-palette index, so a `Canvas` this editor could never have
/// produced still decodes safely.
fn icon_from_canvas(canvas: &Canvas) -> PlayerIcon {
    let mut icon = PlayerIcon::default();
    for y in 0..ICON_GRID {
        for x in 0..ICON_GRID {
            icon.set(x, y, canvas.get(x, y));
        }
    }
    icon
}

pub(crate) struct IconEditor {
    editor: CanvasEditor,
    /// What the editor opened with — and, once `Enter` is pressed, what was
    /// kept. `Esc` puts the *opened-with* value back onto the canvas but
    /// never touches this field, which is what lets `Discard` carry no
    /// icon: discarding is defined as the editor ending on what it started
    /// with, rather than as a caller remembering to throw away a canvas it
    /// never looked at.
    opened_with: PlayerIcon,
}

impl IconEditor {
    /// Opens on `icon` — the drawing in progress, or a blank canvas for a
    /// player who has never opened this screen.
    pub(crate) fn open(icon: PlayerIcon) -> Self {
        let canvas = canvas_from_icon(&icon);
        IconEditor {
            editor: CanvasEditor::open(canvas, ICON_PALETTE.len() as u8),
            opened_with: icon,
        }
    }

    /// The drawing as it stands — and, after either ending, the drawing
    /// the wizard should act on: `Keep` leaves what was drawn here and
    /// `Discard` leaves what the editor opened with.
    pub(crate) fn icon(&self) -> &PlayerIcon {
        &self.opened_with
    }

    /// What the screen draws.
    pub(crate) fn view(&self) -> IconEditorView {
        IconEditorView {
            canvas: self.editor.view(),
        }
    }

    /// `Enter`/`Esc` are this screen's own; every other key is
    /// `CanvasEditor`'s, and this editor does not care which one it was —
    /// only whether the screen is still open.
    pub(crate) fn handle_key(&mut self, key: GameKey) -> IconEditorOutcome {
        match key {
            GameKey::Enter => {
                self.opened_with = icon_from_canvas(self.editor.canvas());
                IconEditorOutcome::Keep
            }
            GameKey::Esc => {
                self.editor.set_canvas(canvas_from_icon(&self.opened_with));
                IconEditorOutcome::Discard
            }
            _ => {
                let _: CanvasKey = self.editor.handle_key(key);
                IconEditorOutcome::Open
            }
        }
    }
}

impl App {
    /// What the icon editor screen draws, or `None` while it is not open.
    pub fn icon_editor_view(&self) -> Option<IconEditorView> {
        self.creation_icon_editor.as_ref().map(IconEditor::view)
    }
}
