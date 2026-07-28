//! Aiming the inspector at a tile, and the manifest screen it opens.

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
                self.pending_manifest = Some(entity);
                self.status_line = None;
                self.mode = Mode::Manifest;
            }
            None => {
                self.status_line = Some("Nothing in that direction.".to_string());
                self.mode = Mode::Playing;
            }
        }
    }

    /// You, then every program you own — everyone the manifest can page
    /// through with ←/→. A wild program reached via `i` is deliberately not
    /// in here: it is not yours to page to, and paging away from it would be
    /// a one-way trip.
    pub fn manifest_subjects(&mut self) -> Vec<Entity> {
        self.game
            .as_mut()
            .map(|game| game.manifest_subjects())
            .unwrap_or_default()
    }

    pub(crate) fn handle_manifest_key(&mut self, _key: GameKey) {
        self.pending_manifest = None;
        self.mode = Mode::Playing;
    }
}
