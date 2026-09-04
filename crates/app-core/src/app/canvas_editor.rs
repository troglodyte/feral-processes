//! Shared canvas-editing mechanics: the cursor clamp, the paint guard, the
//! undo ring and the shared half of the key table.
//!
//! `IconEditor` (`crates/app-core/src/app/icon_editor.rs`) composes one of
//! these to draw the player's 8x8 map avatar; the dev-only sprite editor
//! composes a second, wider one on the same mechanics. **Share the canvas
//! mechanics, not the state machine**: the sink, the outcome and the
//! wizard's `Enter`/`Esc` interception differ between the two editors and
//! stay on their own side — `CanvasEditor` never returns an outcome, only
//! whether it recognised the key.
//!
//! **Undo is whole `Canvas` snapshots, not a diff.** `ICON_UNDO_DEPTH` of
//! them is small enough that simple wins. A snapshot is pushed only when a
//! keystroke actually changes the canvas — a held `Space` on a cell already
//! the selected colour would otherwise fill the history with nothing, and
//! undo would stop reaching the edit the player wants back.
//!
//! **The brush is a footprint and a step together.** At brush *n* an arrow
//! moves the cursor a whole *n*-cell stride and its landing coordinates are
//! snapped to a multiple of *n*, and one paint fills the whole *n*x*n* block
//! anchored there. At brush 1 every one of those is a no-op, which is what
//! keeps the icon editor — permanently at brush 1 — behaving exactly as it
//! did before the brush existed.
//!
//! **A stroke is one undo entry.** `begin_stroke` snapshots once, up front,
//! before anything is known to change; every `paint_at` until `end_stroke`
//! records nothing further. Without this a mouse drag — the reason the
//! brush and the stroke both exist — would cost one undo per cell crossed.

use std::collections::VecDeque;

use crate::*;
use feral_processes_engine::icon::Canvas;

/// How far back `u` reaches. Here rather than in `tuning.rs`: that file is
/// how hard the game is, and this prices no fight, gates no progression and
/// is invisible outside two dev/creation screens.
pub(crate) const ICON_UNDO_DEPTH: usize = 32;

/// The lowest selectable swatch. Zero is not on the list — it means
/// transparent, and `Backspace` is the verb that reaches it.
const FIRST_COLOUR: u8 = 1;

/// Which of the editor's two panels the arrows are driving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CanvasFocus {
    Canvas,
    Palette,
}

/// What a canvas screen draws — the grid flattened row-major, the three
/// cursors laid over it, and the brush.
///
/// `cells` is a flat copy rather than the `Canvas` itself because the
/// screen draws per-cell rectangles and never a texture: the grid lines and
/// the cursor need per-cell rects anyway, and drawing it that way is what
/// keeps a texture from being minted on every keystroke.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CanvasView {
    pub cells: Vec<u8>,
    pub edge: u8,
    pub cursor: (u8, u8),
    pub selected: u8,
    pub focus: CanvasFocus,
    pub brush: u8,
}

/// Whether `CanvasEditor::handle_key` recognised the key. `Enter` and `Esc`
/// are always `Unhandled` here — they are the sink's own outcome, not the
/// canvas's, and a caller routes them by taking this back.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CanvasKey {
    Handled,
    Unhandled,
}

pub(crate) struct CanvasEditor {
    canvas: Canvas,
    cursor: (u8, u8),
    selected: u8,
    focus: CanvasFocus,
    brush: u8,
    /// The highest selectable swatch, one per opening caller — the icon
    /// editor's 15 and the sprite editor's 19 are different palettes, and
    /// `Canvas` itself knows nothing about which one it is drawn from.
    palette_len: u8,
    history: VecDeque<Canvas>,
    /// Whether a stroke is open — set by `begin_stroke`, cleared by
    /// `end_stroke`. While set, `record` is a no-op: the one snapshot
    /// `begin_stroke` already pushed stands for the whole stroke.
    stroke: bool,
}

impl CanvasEditor {
    /// Opens on `canvas` at brush 1, cursor at the origin, the first
    /// swatch selected. `palette_len` bounds `selected` and `pick_swatch`
    /// for as long as this editor is open.
    pub(crate) fn open(canvas: Canvas, palette_len: u8) -> CanvasEditor {
        CanvasEditor {
            canvas,
            cursor: (0, 0),
            selected: FIRST_COLOUR,
            focus: CanvasFocus::Canvas,
            brush: 1,
            palette_len,
            history: VecDeque::new(),
            stroke: false,
        }
    }

    /// What the screen draws.
    pub(crate) fn view(&self) -> CanvasView {
        let edge = self.canvas.edge();
        let mut cells = Vec::with_capacity(edge * edge);
        for y in 0..edge {
            for x in 0..edge {
                cells.push(self.canvas.get(x, y));
            }
        }
        CanvasView {
            cells,
            edge: edge as u8,
            cursor: self.cursor,
            selected: self.selected,
            focus: self.focus,
            brush: self.brush,
        }
    }

    pub(crate) fn canvas(&self) -> &Canvas {
        &self.canvas
    }

    /// Replaces the drawing wholesale — the door a sink uses to put back
    /// what it opened with, or to load a different subject onto the same
    /// editor. Leaves the cursor, focus, brush and history untouched.
    pub(crate) fn set_canvas(&mut self, canvas: Canvas) {
        self.canvas = canvas;
    }

    /// 1 or 2; anything else is dropped rather than accepted and clamped —
    /// there is no third brush size to round toward. Re-snaps the cursor
    /// to the new brush's grid, since a cursor left at an odd coordinate
    /// from brush 1 would otherwise anchor a brush-2 block half off the
    /// cell the player is looking at.
    ///
    /// `IconEditor` never calls this — it opens at brush 1 and stays there
    /// — so this and the three verbs after it are unreachable from
    /// production code until the dev-only sprite editor (a later task)
    /// composes a second `CanvasEditor` and drives them. Exercised directly
    /// by `tests/canvas_editor.rs` in the meantime.
    #[allow(dead_code)]
    pub(crate) fn set_brush(&mut self, brush: u8) {
        if brush == 1 || brush == 2 {
            self.brush = brush;
            self.clamp_cursor();
        }
    }

    /// Opens a stroke: one snapshot, taken now, before anything is known to
    /// change. Every `paint_at` before the matching `end_stroke` records
    /// nothing further.
    #[allow(dead_code)]
    pub(crate) fn begin_stroke(&mut self) {
        self.push_snapshot();
        self.stroke = true;
    }

    #[allow(dead_code)]
    pub(crate) fn end_stroke(&mut self) {
        self.stroke = false;
    }

    /// Selects a swatch directly, clamped to the palette this editor opened
    /// with — the click-driven twin of the palette panel's arrow keys.
    #[allow(dead_code)]
    pub(crate) fn pick_swatch(&mut self, index: u8) {
        self.selected = (index as i32).clamp(FIRST_COLOUR as i32, self.palette_len as i32) as u8;
    }

    /// Paints the brush's whole footprint anchored at `(x, y)` with
    /// `index`, snapshotting first — unless every cell in the footprint
    /// already holds `index`, in which case nothing happened and nothing
    /// is recorded. At brush 1 this is exactly the old one-cell paint.
    pub(crate) fn paint_at(&mut self, x: u8, y: u8, index: u8) {
        let brush = self.brush as usize;
        let (bx, by) = (x as usize, y as usize);
        let already =
            (0..brush).all(|oy| (0..brush).all(|ox| self.canvas.get(bx + ox, by + oy) == index));
        if already {
            return;
        }
        self.record();
        for oy in 0..brush {
            for ox in 0..brush {
                self.canvas.set(bx + ox, by + oy, index);
            }
        }
    }

    /// The shared half of the key table: cursor movement, paint, erase,
    /// undo and clear. `Enter` and `Esc` are deliberately absent — a caller
    /// takes them back as `Unhandled` and decides what they mean.
    pub(crate) fn handle_key(&mut self, key: GameKey) -> CanvasKey {
        match key {
            GameKey::Tab => {
                self.focus = match self.focus {
                    CanvasFocus::Canvas => CanvasFocus::Palette,
                    CanvasFocus::Palette => CanvasFocus::Canvas,
                };
            }
            GameKey::Up => self.step(0, -1),
            GameKey::Down => self.step(0, 1),
            GameKey::Left => self.step(-1, 0),
            GameKey::Right => self.step(1, 0),
            GameKey::Char(' ') => self.paint_at(self.cursor.0, self.cursor.1, self.selected),
            GameKey::Backspace => self.paint_at(self.cursor.0, self.cursor.1, 0),
            GameKey::Char('u') => self.undo(),
            GameKey::Char('x') => self.clear(),
            _ => return CanvasKey::Unhandled,
        }
        CanvasKey::Handled
    }

    /// One arrow press, on the focused panel alone. Neither cursor wraps.
    /// On the canvas the step is a whole brush-width and the landing
    /// coordinates are snapped to a multiple of the brush — see
    /// `Self::snap`. The palette is a sequence rather than a grid, so both
    /// axes walk it — back on Left and Up, forward on Right and Down.
    fn step(&mut self, dx: i32, dy: i32) {
        match self.focus {
            CanvasFocus::Canvas => {
                let brush = self.brush as i32;
                let last = self.last_anchor();
                self.cursor.0 = Self::snap(self.cursor.0 as i32 + dx * brush, last, brush);
                self.cursor.1 = Self::snap(self.cursor.1 as i32 + dy * brush, last, brush);
            }
            CanvasFocus::Palette => {
                self.selected = (self.selected as i32 + dx + dy)
                    .clamp(FIRST_COLOUR as i32, self.palette_len as i32)
                    as u8;
            }
        }
    }

    /// The highest legal brush anchor on this canvas — the point past which
    /// the brush's own footprint would run off the grid.
    fn last_anchor(&self) -> i32 {
        (self.canvas.edge() as i32 - self.brush as i32).max(0)
    }

    /// Clamps `v` to `0..=last`, then rounds down to the nearest multiple
    /// of `brush` — the one place a cursor coordinate is kept a legal
    /// anchor for the brush that is about to paint from it.
    fn snap(v: i32, last: i32, brush: i32) -> u8 {
        let clamped = v.clamp(0, last);
        (clamped - clamped.rem_euclid(brush.max(1))) as u8
    }

    /// `set_brush`'s only caller; unreachable in production until that is,
    /// for the same reason.
    #[allow(dead_code)]
    fn clamp_cursor(&mut self) {
        let brush = self.brush as i32;
        let last = self.last_anchor();
        self.cursor.0 = Self::snap(self.cursor.0 as i32, last, brush);
        self.cursor.1 = Self::snap(self.cursor.1 as i32, last, brush);
    }

    /// `paint_at`'s rule over the whole canvas: clearing a canvas that is
    /// already blank is not an edit.
    fn clear(&mut self) {
        if self.canvas.is_blank() {
            return;
        }
        self.record();
        self.canvas.clear();
    }

    fn undo(&mut self) {
        if let Some(previous) = self.history.pop_back() {
            self.canvas = previous;
        }
    }

    /// Snapshots the canvas as it stands — unless a stroke is open, in
    /// which case `begin_stroke` already took the one snapshot the whole
    /// stroke gets.
    fn record(&mut self) {
        if self.stroke {
            return;
        }
        self.push_snapshot();
    }

    /// The unconditional half of `record`: drops the oldest entry once the
    /// history is full, then pushes. `begin_stroke` calls this directly,
    /// bypassing the stroke guard `record` itself is gated by.
    fn push_snapshot(&mut self) {
        if self.history.len() == ICON_UNDO_DEPTH {
            self.history.pop_front();
        }
        self.history.push_back(self.canvas.clone());
    }
}
