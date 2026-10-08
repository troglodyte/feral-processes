//! The Excavation plan: `Mode::Excavate`, where a base's next wing is drawn
//! before anything is cut.
//!
//! A **mode, not an action**. Nothing here reaches `Game::tick`, so a player
//! can plan a whole corridor without spending a turn on it — walking a
//! cursor around a big box costs nothing, whatever the plan turns out to
//! mark.

use crate::*;

/// How far from the party the cursor may be walked, in cells.
///
/// A bound rather than the unbounded space `BaseGrid` actually is, for two
/// reasons that happen to agree: the map pane is centred on the party and
/// does not scroll while the mode is open, so a cursor past the pane's edge
/// would be a cursor the player cannot see; and it caps how much rock one
/// keypress can mark, which is what stops a mis-drawn box becoming a plan
/// the player has to erase cell by cell.
const CURSOR_RANGE: i32 = 12;

/// One row of the brush picker — what picking it sets, what it is called,
/// and the swatch a finish row draws beside its name.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct BrushRow {
    pub brush: Option<FinishOrder>,
    pub label: String,
    pub shade: Option<feral_processes_engine::floors::FloorShade>,
}

impl App {
    /// Moves the cursor, drops and lifts the anchor, and commits the box.
    ///
    /// `space` is the one verb: the first drops an anchor, the second
    /// commits the box the cursor has been previewing. Whether that box
    /// marks or clears is the **engine's** decision, from the anchor cell —
    /// see `Game::toggle_mark_box` — so this never has to hold a mode of its
    /// own for erasing.
    ///
    /// `esc` takes the anchor back before it takes the mode, so one press is
    /// never two undos.
    pub(crate) fn handle_excavate_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            if self.excavate_anchor.take().is_none() {
                self.excavate_cursor = None;
                self.mode = Mode::Playing;
            }
            return;
        }
        let Some(party) = self.game.as_ref().and_then(|g| g.base_pos()) else {
            // The party left base space out from under the mode — nothing
            // in here can happen anywhere else, so it closes rather than
            // drawing over the open grid.
            self.excavate_cursor = None;
            self.excavate_anchor = None;
            self.mode = Mode::Playing;
            return;
        };
        let Some((cx, cy)) = self.excavate_cursor else {
            self.mode = Mode::Playing;
            return;
        };
        let (dx, dy) = match key {
            GameKey::Up | GameKey::Char('k') => (0, -1),
            GameKey::Down | GameKey::Char('j') => (0, 1),
            GameKey::Left | GameKey::Char('h') => (-1, 0),
            GameKey::Right | GameKey::Char('l') => (1, 0),
            GameKey::Char(' ') | GameKey::Enter => {
                match self.excavate_anchor.take() {
                    Some(anchor) => {
                        if let Some(game) = &mut self.game {
                            game.toggle_mark_box(anchor, (cx, cy), self.excavate_brush.as_ref());
                        }
                    }
                    None => self.excavate_anchor = Some((cx, cy)),
                }
                return;
            }
            GameKey::Char('F') => {
                // Inert over an empty `FloorDb`: plain is the only brush
                // there is, and a picker of one row is a screen that does
                // nothing.
                if self.excavate_brush_rows().len() > 1 {
                    self.mode = Mode::ExcavateBrush;
                }
                return;
            }
            _ => return,
        };
        self.excavate_cursor = Some((
            (cx + dx).clamp(party.0 - CURSOR_RANGE, party.0 + CURSOR_RANGE),
            (cy + dy).clamp(party.1 - CURSOR_RANGE, party.1 + CURSOR_RANGE),
        ));
    }

    /// The picker's rows: plain, every loaded finish in id order, strip.
    /// The one list both the keys and the popup read, so a row the player
    /// sees is the row a key picks. Empty with no game, and plain alone
    /// with an empty `FloorDb` — strip with no finish to lift is not a
    /// brush anyone asked for.
    pub fn excavate_brush_rows(&self) -> Vec<BrushRow> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        let defs = game.floor_defs();
        let mut rows = vec![BrushRow {
            brush: None,
            label: "plain — cut rock, lay tile".to_string(),
            shade: None,
        }];
        if defs.is_empty() {
            return rows;
        }
        rows.extend(defs.into_iter().map(|d| BrushRow {
            brush: Some(FinishOrder::Apply(d.id)),
            label: d.name,
            shade: Some(d.shade),
        }));
        rows.push(BrushRow {
            brush: Some(FinishOrder::Strip),
            label: "strip — lift a finish".to_string(),
            shade: None,
        });
        rows
    }

    /// The row the brush in hand sits on, so the picker opens there and
    /// Enter alone keeps it. A brush naming a finish no longer loaded falls
    /// back to plain's row.
    pub(crate) fn excavate_brush_row(&self) -> usize {
        self.excavate_brush_rows()
            .iter()
            .position(|r| r.brush == self.excavate_brush)
            .unwrap_or(0)
    }

    pub(crate) fn handle_excavate_brush_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Excavate;
            return;
        }
        let mut rows = self.excavate_brush_rows();
        if let Some(idx) = self.selected_index(key, rows.len()) {
            self.excavate_brush = rows.swap_remove(idx).brush;
            self.mode = Mode::Excavate;
        }
    }

    /// What the header shows for the current brush — `None` when there is
    /// no finish content loaded at all, so a base with an empty
    /// `assets/floors/` draws exactly today's screen.
    pub fn excavate_brush_label(&self) -> Option<String> {
        let defs = self.game.as_ref()?.floor_defs();
        if defs.is_empty() {
            return None;
        }
        Some(match &self.excavate_brush {
            None => "Brush: plain [F]".to_string(),
            Some(FinishOrder::Apply(id)) => {
                let name = defs
                    .iter()
                    .find(|d| &d.id == id)
                    .map(|d| d.name.as_str())
                    .unwrap_or(id.as_str());
                format!("Brush: {name} [F]")
            }
            Some(FinishOrder::Strip) => "Brush: strip [F]".to_string(),
        })
    }
}
