//! The pixel editor behind the Sprite Forge and the character wizard's
//! drawing step: a `CanvasEditor` plus frames, a frame pace and a forge-level
//! undo, and the keys and pointer gestures that drive them.
//!
//! It knows nothing of what it is drawing. A host owns the subject, the save
//! and the leave keys; the editor reports a key as `SpriteKey::Unhandled` when
//! it is the host's to answer (`Enter`, `Esc`, `s`), and the host describes
//! itself to the renderer through `EditorChrome`.
//!
//! Composes a `CanvasEditor` per frame — see that module's doc comment for
//! why the mechanics are shared and the sink is not.

use std::collections::VecDeque;

use feral_processes_engine::components::GlyphColor;
use feral_processes_engine::icon::{Canvas, SPRITE_PALETTE};

use crate::GameKey;
use crate::app::canvas_editor::{CanvasEditor, CanvasKey, CanvasView, ICON_UNDO_DEPTH, Tool};

/// The sprite canvas's edge — always 16, never a brush-dependent size (the
/// brush is a footprint over this fixed grid, not a second canvas format;
/// see the design doc's "The canvas is 16x16 always"). `ICON_SIZE` is the
/// same 16, named for the *pixel* geometry the icon editor draws at; reused
/// here rather than a second `pub const SPRITE_EDGE = 16` because the two
/// really are one number, documented as such in `engine::icon`'s own doc
/// comment ("The *sprite* is 16x16 (`ICON_SIZE`)").
const SPRITE_EDGE: usize = feral_processes_engine::ICON_SIZE;

pub use feral_processes_engine::{DEFAULT_SPRITE_FRAME_MS, MAX_SPRITE_FRAMES};

/// What one `[-]`/`[=]` press moves a sprite's frame time by.
pub const SPRITE_FRAME_MS_STEP: u32 = 100;
pub const SPRITE_FRAME_MS_MIN: u32 = 100;
pub const SPRITE_FRAME_MS_MAX: u32 = 2000;

/// Swatches per row of the sprite palette grid: 64 swatches (the transparent
/// one and 63 colours) as four rows of sixteen, nine families of six colours
/// reading across and the greys leading.
const SPRITE_PALETTE_COLS: u8 = 16;

/// What a subject's preview multiplies against — the `SpriteSubject::tint`
/// a picker row and a live editor preview both read.
///
/// A small enum over the two sources rather than a second field: a species
/// or structure (or the player, or the anchor) previews against a
/// `GlyphColor`, a floor finish previews against a `FloorShade`, and a
/// subject is always exactly one or the other.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SubjectTint {
    Glyph(Option<GlyphColor>),
    Shade(feral_processes_engine::floors::FloorShade),
}

/// Where a pointer landed on `Mode::SpriteEditor`, already resolved to a
/// cell or a swatch by the gui — never a pixel. The gui owns the canvas and
/// swatch rects (it draws them), so it tests the pointer against those
/// itself; app-core never receives a pixel and never learns the rects
/// exist.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerHit {
    Cell(u8, u8),
    Swatch(u8),
    Button(EditorButton),
    Subject(usize),
}

/// A clickable stand-in for one of the editor's keys. **A button has no
/// effect of its own**: `handle_pointer` presses it by calling the key
/// handler with `key()`, so a button and its key cannot drift apart.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EditorButton {
    Save,
    Undo,
    Clear,
    Brush,
    Tool,
    Back,
    Frame1,
    Frame2,
    DeleteFrame,
    Slower,
    Faster,
    Keep,
    Discard,
}

impl EditorButton {
    pub fn key(self) -> GameKey {
        match self {
            EditorButton::Save => GameKey::Char('s'),
            EditorButton::Undo => GameKey::Char('u'),
            EditorButton::Clear => GameKey::Char('x'),
            EditorButton::Brush => GameKey::Char('g'),
            EditorButton::Tool => GameKey::Char('f'),
            EditorButton::Back => GameKey::Esc,
            EditorButton::Frame1 => GameKey::Char('1'),
            EditorButton::Frame2 => GameKey::Char('2'),
            EditorButton::DeleteFrame => GameKey::Char('D'),
            EditorButton::Slower => GameKey::Char('-'),
            EditorButton::Faster => GameKey::Char('='),
            EditorButton::Keep => GameKey::Enter,
            EditorButton::Discard => GameKey::Esc,
        }
    }
}

/// The Forge's buttons. The gui puts the first six on the bottom bar and
/// stacks the rest beside the preview, so every host's list keeps that
/// six-then-five shape.
pub const FORGE_BUTTONS: [EditorButton; 11] = [
    EditorButton::Save,
    EditorButton::Undo,
    EditorButton::Clear,
    EditorButton::Brush,
    EditorButton::Tool,
    EditorButton::Back,
    EditorButton::Frame1,
    EditorButton::Frame2,
    EditorButton::DeleteFrame,
    EditorButton::Slower,
    EditorButton::Faster,
];

/// The wizard's buttons: keep or discard in place of save and back.
pub const WIZARD_BUTTONS: [EditorButton; 11] = [
    EditorButton::Keep,
    EditorButton::Discard,
    EditorButton::Undo,
    EditorButton::Clear,
    EditorButton::Brush,
    EditorButton::Tool,
    EditorButton::Frame1,
    EditorButton::Frame2,
    EditorButton::DeleteFrame,
    EditorButton::Slower,
    EditorButton::Faster,
];

/// Which mouse button a `PointerHit` was reported for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerButton {
    Primary,
    /// Paints index 0 — erase, the same thing `Backspace` already means on
    /// this editor.
    Secondary,
    /// Picks the cell's colour, the eyedropper — `[i]`'s pointer twin.
    Middle,
}

/// Where in a click-or-drag gesture a `PointerHit` was reported.
///
/// **A whole drag is one undo entry, not one per cell.** `Down` opens a
/// stroke (`CanvasEditor::begin_stroke`, one snapshot taken before anything
/// is known to change), `Up` closes it (`end_stroke`), and `Drag` neither
/// opens nor closes one — every cell the gui reports while the button is
/// held lands inside the same stroke `Down` opened.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PointerPhase {
    Down,
    Drag,
    Up,
    /// Only the picker reads this, and only on a `PointerHit::Subject`.
    DoubleClick,
}

/// What differs between hosts of the editor: the header, the button set, the
/// preview's hue source and which variant a save would write.
pub struct EditorChrome {
    pub title: String,
    pub buttons: Vec<EditorButton>,
    pub tint: Option<SubjectTint>,
    pub full_colour: bool,
}

/// What `Mode::SpriteEditor` draws.
pub struct SpriteEditorView {
    pub canvas: CanvasView,
    pub title: String,
    pub buttons: Vec<EditorButton>,
    /// What the preview multiplies against; `None` draws it unmodified.
    pub tint: Option<SubjectTint>,
    pub palette: &'static [(u8, u8, u8)],
    /// The variant a save would write, so the preview can tint the way the
    /// map will.
    pub full_colour: bool,
    /// Swatches per drawn row, the transparent swatch counted — the same
    /// number the keyboard's Up and Down move by.
    pub palette_cols: u8,
    pub tool: Tool,
    pub frames: usize,
    pub active_frame: usize,
    pub frame_ms: u32,
    /// Every frame's cells, the active one live, so the preview can play the
    /// animation without a second call.
    pub frame_cells: Vec<Vec<u8>>,
}

/// Whether the editor took a key, or left it for the host to answer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SpriteKey {
    Handled,
    Unhandled,
}

/// One forge undo entry: every frame, and which was being edited.
type ForgeSnapshot = (Vec<Canvas>, usize);

pub(crate) struct SpriteEditor {
    editor: CanvasEditor,
    /// Every frame but the live one's latest pixels: `editor` owns the
    /// active frame's canvas while it is being edited, so `frames[active]`
    /// is stale until `frames()` or a switch folds it back in.
    frames: Vec<Canvas>,
    active_frame: usize,
    frame_ms: u32,
    /// `(frames, active_frame)` as they stood before each change. The shared
    /// editor's ring holds one bare canvas, which cannot undo a frame being
    /// added or deleted.
    history: VecDeque<ForgeSnapshot>,
}

impl SpriteEditor {
    /// Opens on `frames`, or on one blank 16x16 canvas when there are none.
    /// Blank is a legitimate opening state here, unlike the player's own
    /// `@`: nothing filters it away before it can be saved.
    pub(crate) fn open(mut frames: Vec<Canvas>, frame_ms: u32) -> SpriteEditor {
        if frames.is_empty() {
            frames.push(Canvas::new(SPRITE_EDGE));
        }
        SpriteEditor {
            editor: CanvasEditor::open(
                frames[0].clone(),
                SPRITE_PALETTE.len() as u8,
                SPRITE_PALETTE_COLS,
            ),
            frames,
            active_frame: 0,
            frame_ms,
            history: VecDeque::new(),
        }
    }

    /// Every frame with the live canvas folded into the active one.
    pub(crate) fn frames(&self) -> Vec<Canvas> {
        let mut frames = self.frames.clone();
        frames[self.active_frame] = self.editor.canvas().clone();
        frames
    }

    pub(crate) fn frame_ms(&self) -> u32 {
        self.frame_ms
    }

    fn snapshot(&self) -> ForgeSnapshot {
        (self.frames(), self.active_frame)
    }

    fn remember(&mut self, snapshot: ForgeSnapshot) {
        if self.history.len() == ICON_UNDO_DEPTH {
            self.history.pop_front();
        }
        self.history.push_back(snapshot);
    }

    /// Runs a shared-editor edit and keeps a forge undo entry for it if the
    /// editor took a snapshot. Compared by the editor's running count, not
    /// its ring depth: a full ring stays the same length while still
    /// taking snapshots.
    fn delegate<R>(&mut self, edit: impl FnOnce(&mut CanvasEditor) -> R) -> R {
        let before = self.snapshot();
        let taken = self.editor.snapshots_taken();
        let result = edit(&mut self.editor);
        if self.editor.snapshots_taken() != taken {
            self.remember(before);
        }
        result
    }

    /// `[1]`/`[2]`. The first `[2]` on a one-frame sprite makes frame 2 as a
    /// copy of frame 1; anything past the next unmade frame is ignored.
    fn select_frame(&mut self, index: usize) {
        if index == self.active_frame || index > self.frames.len() || index >= MAX_SPRITE_FRAMES {
            return;
        }
        let before = self.snapshot();
        self.frames = self.frames();
        if index == self.frames.len() {
            self.frames.push(self.frames[self.active_frame].clone());
            self.remember(before);
        }
        self.active_frame = index;
        self.editor.set_canvas(self.frames[index].clone());
    }

    /// `[D]`: drops frame 2 and goes back to frame 1.
    fn delete_second_frame(&mut self) {
        if self.frames.len() < 2 {
            return;
        }
        let before = self.snapshot();
        self.frames = self.frames();
        self.frames.truncate(1);
        self.active_frame = 0;
        self.editor.set_canvas(self.frames[0].clone());
        self.remember(before);
    }

    /// The forge's own `[u]`, restoring frames and the active frame together.
    fn undo(&mut self) {
        if let Some((frames, active)) = self.history.pop_back() {
            self.frames = frames;
            self.active_frame = active;
            self.editor.set_canvas(self.frames[active].clone());
        }
    }

    fn step_frame_ms(&mut self, up: bool) {
        self.frame_ms = if up {
            self.frame_ms.saturating_add(SPRITE_FRAME_MS_STEP)
        } else {
            self.frame_ms.saturating_sub(SPRITE_FRAME_MS_STEP)
        }
        .clamp(SPRITE_FRAME_MS_MIN, SPRITE_FRAME_MS_MAX);
    }

    /// The editor's own keys, on top of `CanvasEditor`'s shared table (arrows,
    /// `Space`/`Backspace`, `x`, `Tab`): `[g]` toggles the brush 1<->2, `[f]`
    /// the tool, `[1]`/`[2]` pick a frame, `[D]` deletes the second, `[-]`/
    /// `[=]` set the pace, `[u]` undoes and `[i]` picks the colour under the
    /// cursor. `Enter`, `Esc` and `[s]` are not the editor's: they come back
    /// `Unhandled` for the host to answer.
    pub(crate) fn handle_key(&mut self, key: GameKey) -> SpriteKey {
        match key {
            GameKey::Char('g') => {
                let next = if self.editor.view().brush == 1 { 2 } else { 1 };
                self.editor.set_brush(next);
            }
            GameKey::Char('f') => self.editor.toggle_tool(),
            GameKey::Char('1') => self.select_frame(0),
            GameKey::Char('2') => self.select_frame(1),
            GameKey::Char('D') => self.delete_second_frame(),
            GameKey::Char('-') => self.step_frame_ms(false),
            GameKey::Char('=') => self.step_frame_ms(true),
            GameKey::Char('u') => self.undo(),
            GameKey::Char('i') => {
                let (x, y) = self.editor.view().cursor;
                self.editor.pick_colour_at(x, y);
            }
            _ => {
                return match self.delegate(|editor| editor.handle_key(key)) {
                    CanvasKey::Handled => SpriteKey::Handled,
                    CanvasKey::Unhandled => SpriteKey::Unhandled,
                };
            }
        }
        SpriteKey::Handled
    }

    /// A cell paints (the selected swatch on `Primary`, index 0 — erase — on
    /// `Secondary`), a `Swatch` selects. `PointerHit::Button` and `Subject`
    /// are the host's: a button is a key press the host routes itself.
    ///
    /// **`PointerHit::Swatch` carries `swatch_at`'s 0-based drawn position,
    /// and that is the palette index itself**: `draw_swatch_grid` draws the
    /// transparent swatch (index 0) first and palette entry `n` at position
    /// `n`. It used to be a `+ 1` conversion here, and getting that wrong
    /// selected the swatch left of the one outlined; with no conversion
    /// there is nothing at this seam to get wrong.
    pub(crate) fn handle_pointer(
        &mut self,
        hit: PointerHit,
        button: PointerButton,
        phase: PointerPhase,
    ) {
        self.delegate(|editor| {
            if phase == PointerPhase::Down {
                editor.begin_stroke();
            }
            match hit {
                PointerHit::Cell(x, y) if button == PointerButton::Middle => {
                    editor.pick_colour_at(x, y);
                }
                // A fill is one press, not a gesture: only `Down` acts.
                PointerHit::Cell(..)
                    if editor.tool() == Tool::Fill && phase != PointerPhase::Down => {}
                PointerHit::Cell(x, y) => {
                    let index = match button {
                        PointerButton::Primary => editor.view().selected,
                        PointerButton::Secondary | PointerButton::Middle => 0,
                    };
                    editor.apply_tool(x, y, index);
                }
                PointerHit::Swatch(index) => editor.pick_swatch(index),
                PointerHit::Button(_) | PointerHit::Subject(_) => {}
            }
            if phase == PointerPhase::Up {
                editor.end_stroke();
            }
        });
    }

    /// What the screen draws — `CanvasEditor`'s own view, the host's chrome,
    /// and the palette a `CanvasView`'s bare indices need to become colour.
    pub(crate) fn view(&self, chrome: EditorChrome) -> SpriteEditorView {
        let frame_cells = self
            .frames()
            .iter()
            .map(|canvas| {
                let edge = canvas.edge();
                (0..edge)
                    .flat_map(|y| (0..edge).map(move |x| (x, y)))
                    .map(|(x, y)| canvas.get(x, y))
                    .collect()
            })
            .collect();
        SpriteEditorView {
            canvas: self.editor.view(),
            title: chrome.title,
            buttons: chrome.buttons,
            tint: chrome.tint,
            palette: &SPRITE_PALETTE,
            full_colour: chrome.full_colour,
            palette_cols: SPRITE_PALETTE_COLS,
            tool: self.editor.tool(),
            frames: self.frames.len(),
            active_frame: self.active_frame,
            frame_ms: self.frame_ms,
            frame_cells,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn enter_esc_and_s_come_back_unhandled() {
        let mut editor = SpriteEditor::open(Vec::new(), DEFAULT_SPRITE_FRAME_MS);
        for key in [GameKey::Enter, GameKey::Esc, GameKey::Char('s')] {
            assert_eq!(editor.handle_key(key), SpriteKey::Unhandled, "{key:?}");
        }
    }

    #[test]
    fn the_forge_keys_are_handled_and_change_the_editor() {
        let mut editor = SpriteEditor::open(Vec::new(), DEFAULT_SPRITE_FRAME_MS);
        assert_eq!(editor.handle_key(GameKey::Char('2')), SpriteKey::Handled);
        assert_eq!(editor.frames().len(), 2);
        assert_eq!(editor.handle_key(GameKey::Char('=')), SpriteKey::Handled);
        assert_eq!(
            editor.frame_ms(),
            DEFAULT_SPRITE_FRAME_MS + SPRITE_FRAME_MS_STEP
        );
    }

    #[test]
    fn the_view_carries_the_chrome_it_was_given() {
        let editor = SpriteEditor::open(Vec::new(), DEFAULT_SPRITE_FRAME_MS);
        let view = editor.view(EditorChrome {
            title: "x".into(),
            buttons: WIZARD_BUTTONS.to_vec(),
            tint: None,
            full_colour: false,
        });
        assert_eq!(view.title, "x");
        assert_eq!(view.buttons, WIZARD_BUTTONS.to_vec());
        assert_eq!(view.tint, None);
    }

    #[test]
    fn keep_is_enter_and_discard_is_esc() {
        assert_eq!(EditorButton::Keep.key(), GameKey::Enter);
        assert_eq!(EditorButton::Discard.key(), GameKey::Esc);
    }
}
