//! `Mode::WorldMap` — the surface at chunk scale. Every figure on it comes
//! from `Game::world_map`; this file holds only the pan centre, the row
//! cursor and the compass key.

use crate::*;

impl App {
    /// The view the screen draws, at the pan centre and the shared radius.
    pub fn world_map(&mut self) -> Option<WorldMapView> {
        let center = self.world_map_center;
        self.game.as_mut()?.world_map(center, WORLD_MAP_VIEW_RADIUS)
    }

    /// The side list: `WorldMapView::marks`, which the engine already hands
    /// over nearest the party first. Gui and the key handler both read it,
    /// so a row letter means the same mark in each.
    pub fn world_map_marks(&mut self) -> Vec<WorldMapMark> {
        self.world_map().map(|v| v.marks).unwrap_or_default()
    }

    pub(crate) fn open_world_map(&mut self) {
        let Some(party) = self
            .game
            .as_mut()
            .and_then(|g| g.world_map((0, 0), 0))
            .map(|v| v.party)
        else {
            return;
        };
        self.world_map_center = party;
        self.menu_selected = 0;
        self.mode = Mode::WorldMap;
    }

    pub(crate) fn handle_world_map_key(&mut self, key: GameKey) {
        let (dx, dy) = match key {
            GameKey::Esc => {
                self.close_screen();
                return;
            }
            GameKey::Left => (-1, 0),
            GameKey::Right => (1, 0),
            GameKey::Up => (0, -1),
            GameKey::Down => (0, 1),
            GameKey::Char('P') => {
                if let Some(v) = self.game.as_mut().and_then(|g| g.world_map((0, 0), 0)) {
                    self.world_map_center = v.party;
                }
                return;
            }
            GameKey::Char('C') => {
                let marks = self.world_map_marks();
                if let Some(mark) = marks.get(self.menu_selected) {
                    self.steer_compass_at(mark);
                }
                return;
            }
            // Arrows pan here, so `selected_index`'s Up/Down never see them.
            GameKey::Char(_) => {
                let len = self.world_map_marks().len();
                if let Some(idx) = self.selected_index(key, len) {
                    self.menu_selected = idx;
                }
                return;
            }
            _ => return,
        };
        self.world_map_center.0 += dx;
        self.world_map_center.1 += dy;
    }

    /// Points the compass at `mark`, or refuses when the mark is map-only.
    /// The refusal is a row fragment, `ability_unavailable`'s convention.
    pub(crate) fn steer_compass_at(&mut self, mark: &WorldMapMark) {
        let Some(target) = mark.target else {
            self.refuse("a nest is not a destination");
            return;
        };
        if let Some(game) = self.game.as_mut() {
            game.set_compass_bearing(Some(target));
        }
        self.status_line = None;
    }
}
