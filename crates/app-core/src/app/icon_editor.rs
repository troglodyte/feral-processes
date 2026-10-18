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
use feral_processes_engine::icon::{Canvas, SPRITE_PALETTE};
use feral_processes_engine::{DEFAULT_SPRITE_FRAME_MS, PlayerIcon};

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

/// The first frame is what this single-canvas editor draws on.
fn canvas_from_icon(icon: &PlayerIcon) -> Canvas {
    icon.frames()[0].clone()
}

/// `canvas_from_icon`'s inverse — `PlayerIcon::new` refuses an
/// out-of-palette index, which this editor cannot paint.
fn icon_from_canvas(canvas: &Canvas) -> PlayerIcon {
    PlayerIcon::new(vec![canvas.clone()], DEFAULT_SPRITE_FRAME_MS)
        .expect("the editor paints palette indices on a 16x16 canvas")
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
            editor: CanvasEditor::open(
                canvas,
                SPRITE_PALETTE.len() as u8,
                SPRITE_PALETTE.len() as u8 + 1,
            ),
            opened_with: icon,
        }
    }

    /// **The last *committed* drawing, not the live canvas mid-edit — M3,
    /// final review.** `opened_with` only changes on `Enter`
    /// (`handle_key`'s `Keep` arm resyncs it to the canvas right there), so
    /// this is safe to read as "the drawing as it stands" only *immediately
    /// after* a `Keep` — which is the one real caller's only call site
    /// (`creation.rs::handle_creation_key`'s `Keep` arm). Read it between
    /// keystrokes and it is stale by definition; `Discard` never touches
    /// this field at all, which is what lets it carry no icon of its own —
    /// discarding is defined as the editor ending on what it started with.
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
