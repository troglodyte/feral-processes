//! Aiming the inspector at a tile and reading the result.

use crate::*;

impl App {
    /// Picks a direction (arrows/hjkl) and inspects the first creature the
    /// engine finds stepping that way from the player, rather than picking
    /// from a numbered list of grid coordinates.
    pub(crate) fn handle_inspect_direction_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let dir = match key {
            GameKey::Up | GameKey::Char('k') => Some((0, -1)),
            GameKey::Down | GameKey::Char('j') => Some((0, 1)),
            GameKey::Left | GameKey::Char('h') => Some((-1, 0)),
            GameKey::Right | GameKey::Char('l') => Some((1, 0)),
            _ => None,
        };
        let Some((dx, dy)) = dir else { return };
        let Some(game) = &mut self.game else { return };
        match game.find_creature_in_direction(dx, dy, MENU_SCAN_RADIUS) {
            Some(entity) => {
                self.pending_inspect = Some(entity);
                self.status_line = None;
                self.mode = Mode::InspectDetail;
            }
            None => {
                self.status_line = Some("Nothing in that direction.".to_string());
                self.mode = Mode::Playing;
            }
        }
    }

    pub(crate) fn handle_inspect_detail_key(&mut self, _key: GameKey) {
        self.pending_inspect = None;
        self.mode = Mode::Playing;
    }
}
