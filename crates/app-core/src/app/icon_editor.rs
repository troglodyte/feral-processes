//! The player's 8x8 icon editor: its state, and the whole of its key
//! table.
//!
//! Not a `Mode`, for `creation.rs`'s reason — it hangs off the wizard's
//! Icon step as an `Option<IconEditor>` rather than costing every `Mode`
//! census a row for a screen reachable from exactly one other screen.
//!
//! **Two panels, and `GameKey::Tab` between them.** The arrows act on
//! whichever has focus, so they mean one thing at a time instead of
//! meaning something different depending on a mode the player has to
//! remember they are in. Which panel has focus is drawn, so the answer is
//! on the screen rather than in their head. Everything else is
//! unconditional: `Space` paints with the selected colour wherever focus
//! sits, because picking a swatch and painting with it without tabbing
//! back is the gesture the split is supposed to buy.
//!
//! **Undo is whole `PlayerIcon` snapshots, not a diff.** 64 bytes each and
//! `ICON_UNDO_DEPTH` of them is 2 KB, which is small enough that simple
//! wins. Only a keystroke that actually moves a cell pushes one — a held
//! `Space` on a cell already the selected colour would otherwise fill the
//! history with nothing, and undo would stop reaching the edit the player
//! wants back.

use std::collections::VecDeque;

use crate::*;
use feral_processes_engine::{ICON_GRID, ICON_PALETTE, PlayerIcon};

/// How far back `u` reaches.
///
/// Here rather than in `tuning.rs`: that file is how hard the game is, and
/// this prices no fight, gates no progression and is invisible outside one
/// screen.
pub const ICON_UNDO_DEPTH: usize = 32;

/// The lowest selectable swatch. Zero is not on the list — it means
/// transparent, and `Backspace` is the verb that reaches it.
const FIRST_COLOUR: u8 = 1;
const LAST_COLOUR: u8 = ICON_PALETTE.len() as u8;

/// Which of the editor's two panels the arrows are driving.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IconFocus {
    Canvas,
    Palette,
}

/// What the icon editor screen draws — the canvas flattened row-major, and
/// the three cursors laid over it.
///
/// `cells` is a flat copy rather than the `PlayerIcon` itself because the
/// screen draws per-cell rectangles and never a texture: the grid lines and
/// the cursor need per-cell rects anyway, and drawing it that way is what
/// keeps a texture from being minted on every keystroke. They are *cells*
/// and not pixels: each one paints an `ICON_CELL_PIXELS` block of the
/// 16x16 sprite the upload builds.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IconEditorView {
    pub cells: [u8; ICON_GRID * ICON_GRID],
    pub cursor: (u8, u8),
    pub selected: u8,
    pub focus: IconFocus,
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

pub(crate) struct IconEditor {
    icon: PlayerIcon,
    /// What the editor opened with. `Esc` puts it back, which is what lets
    /// `Discard` carry no icon: discarding is defined as the editor ending
    /// on what it started with, rather than as a caller remembering to
    /// throw away a canvas it never looked at.
    opened_with: PlayerIcon,
    cursor: (u8, u8),
    selected: u8,
    focus: IconFocus,
    history: VecDeque<PlayerIcon>,
}

impl IconEditor {
    /// Opens on `icon` — the drawing in progress, or a blank canvas for a
    /// player who has never opened this screen.
    pub(crate) fn open(icon: PlayerIcon) -> Self {
        IconEditor {
            opened_with: icon.clone(),
            icon,
            cursor: (0, 0),
            selected: FIRST_COLOUR,
            focus: IconFocus::Canvas,
            history: VecDeque::new(),
        }
    }

    /// The drawing as it stands — and, after either ending, the drawing
    /// the wizard should act on: `Keep` leaves what was drawn here and
    /// `Discard` leaves what the editor opened with.
    pub(crate) fn icon(&self) -> &PlayerIcon {
        &self.icon
    }

    /// What the screen draws.
    pub(crate) fn view(&self) -> IconEditorView {
        let mut cells = [0u8; ICON_GRID * ICON_GRID];
        for y in 0..ICON_GRID {
            for x in 0..ICON_GRID {
                cells[y * ICON_GRID + x] = self.icon.get(x, y);
            }
        }
        IconEditorView {
            cells,
            cursor: self.cursor,
            selected: self.selected,
            focus: self.focus,
        }
    }

    /// The editor's whole key table. Anything it does not bind leaves it
    /// open and untouched.
    pub(crate) fn handle_key(&mut self, key: GameKey) -> IconEditorOutcome {
        match key {
            GameKey::Tab => {
                self.focus = match self.focus {
                    IconFocus::Canvas => IconFocus::Palette,
                    IconFocus::Palette => IconFocus::Canvas,
                }
            }
            GameKey::Up => self.step(0, -1),
            GameKey::Down => self.step(0, 1),
            GameKey::Left => self.step(-1, 0),
            GameKey::Right => self.step(1, 0),
            GameKey::Char(' ') => self.paint(self.selected),
            GameKey::Backspace => self.paint(0),
            GameKey::Char('u') => self.undo(),
            GameKey::Char('x') => self.clear(),
            GameKey::Enter => return IconEditorOutcome::Keep,
            GameKey::Esc => {
                self.icon = self.opened_with.clone();
                return IconEditorOutcome::Discard;
            }
            _ => {}
        }
        IconEditorOutcome::Open
    }

    /// One arrow press, on the focused panel alone.
    ///
    /// Neither cursor wraps. On the canvas that is what makes "seven
    /// Lefts is the left edge" true from anywhere; on the palette it is
    /// what keeps a held arrow from cycling past the swatch the player was
    /// aiming at.
    ///
    /// The palette is a sequence rather than a grid, so both axes walk it —
    /// back on Left and Up, forward on Right and Down. How it is laid out
    /// is the screen's business, and a player who guesses the other axis
    /// gets a move rather than a dead key.
    fn step(&mut self, dx: i32, dy: i32) {
        match self.focus {
            IconFocus::Canvas => {
                let last = ICON_GRID as i32 - 1;
                self.cursor.0 = (self.cursor.0 as i32 + dx).clamp(0, last) as u8;
                self.cursor.1 = (self.cursor.1 as i32 + dy).clamp(0, last) as u8;
            }
            IconFocus::Palette => {
                self.selected = (self.selected as i32 + dx + dy)
                    .clamp(FIRST_COLOUR as i32, LAST_COLOUR as i32)
                    as u8;
            }
        }
    }

    /// Writes `index` into the cursor cell, snapshotting first — unless the
    /// cell already holds it, in which case nothing happened and nothing is
    /// recorded.
    fn paint(&mut self, index: u8) {
        let (x, y) = (self.cursor.0 as usize, self.cursor.1 as usize);
        if self.icon.get(x, y) == index {
            return;
        }
        self.record();
        self.icon.set(x, y, index);
    }

    /// `paint`'s rule over the whole canvas: clearing a canvas that is
    /// already blank is not an edit.
    fn clear(&mut self) {
        if self.icon.is_blank() {
            return;
        }
        self.record();
        self.icon.clear();
    }

    fn undo(&mut self) {
        if let Some(previous) = self.history.pop_back() {
            self.icon = previous;
        }
    }

    /// Snapshots the canvas as it stands, dropping the oldest entry once
    /// the history is full.
    fn record(&mut self) {
        if self.history.len() == ICON_UNDO_DEPTH {
            self.history.pop_front();
        }
        self.history.push_back(self.icon.clone());
    }
}

impl App {
    /// What the icon editor screen draws, or `None` while it is not open.
    pub fn icon_editor_view(&self) -> Option<IconEditorView> {
        self.creation_icon_editor.as_ref().map(IconEditor::view)
    }
}
