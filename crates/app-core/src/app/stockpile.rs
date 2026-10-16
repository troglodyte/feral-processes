//! `Mode::Stockpile` — zoning floor as a stockpile with the Excavation
//! plan's cursor and box. A mode, not an action: nothing here ticks.

use super::excavate::BoxKey;
use crate::*;

impl App {
    pub(crate) fn handle_stockpile_key(&mut self, key: GameKey) {
        if let BoxKey::Commit { anchor, corner } = self.box_cursor_key(key)
            && let Some(game) = &mut self.game
        {
            game.toggle_stockpile_box(anchor, corner);
        }
    }
}
