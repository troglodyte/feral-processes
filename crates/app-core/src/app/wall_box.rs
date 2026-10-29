//! `Mode::WallBox` — painting wall requests with the Excavation plan's
//! cursor and box. A mode, not an action: nothing here ticks.

use super::excavate::BoxKey;
use crate::*;

impl App {
    pub(crate) fn handle_wall_box_key(&mut self, key: GameKey) {
        if let BoxKey::Commit { anchor, corner } = self.box_cursor_key(key)
            && let Some(game) = &mut self.game
        {
            let outcome = game.place_wall_box(anchor, corner).map(|_| ());
            self.report(outcome);
        }
    }
}
