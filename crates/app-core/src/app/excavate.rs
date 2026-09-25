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
                self.cycle_excavate_brush();
                return;
            }
            _ => return,
        };
        self.excavate_cursor = Some((
            (cx + dx).clamp(party.0 - CURSOR_RANGE, party.0 + CURSOR_RANGE),
            (cy + dy).clamp(party.1 - CURSOR_RANGE, party.1 + CURSOR_RANGE),
        ));
    }

    /// `[F]`: plain → each loaded finish, in id order → strip → plain.
    ///
    /// **Inert with an empty `FloorDb`.** There is nothing to apply and
    /// nothing to strip, so cycling would otherwise flip between `None` and
    /// a `Strip` the player has no finish to have asked for.
    fn cycle_excavate_brush(&mut self) {
        let Some(game) = &self.game else { return };
        let defs = game.floor_defs();
        if defs.is_empty() {
            return;
        }
        self.excavate_brush = match &self.excavate_brush {
            None => Some(FinishOrder::Apply(defs[0].id.clone())),
            Some(FinishOrder::Apply(id)) => defs
                .iter()
                .position(|d| &d.id == id)
                .and_then(|i| defs.get(i + 1))
                .map(|next| FinishOrder::Apply(next.id.clone()))
                .or(Some(FinishOrder::Strip)),
            Some(FinishOrder::Strip) => None,
        };
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
