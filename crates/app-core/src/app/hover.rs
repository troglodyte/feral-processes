//! The map's delayed hover label — what the pointer is resting on.

use feral_processes_engine::views::drawn_on_surface_map;

use crate::*;

impl App {
    /// The lines the map's hover label shows for tile `(x, y)`: the label of
    /// each entity drawn there, then the biome under them. Empty wherever
    /// `surface_map_game` refuses — a pointer over a popup, or over a map
    /// with no surface tiles, points at nothing.
    ///
    /// Filtered by `views::drawn_on_surface_map`, the rule the map draws by,
    /// so the label can never name a posted program the map hides.
    pub fn hover_lines(&mut self, x: i32, y: i32) -> Vec<String> {
        let Some(game) = self.surface_map_game() else {
            return Vec::new();
        };
        let mut lines: Vec<String> = game
            .view_entities_at((x, y), 0, 0)
            .into_iter()
            .filter(|e| drawn_on_surface_map(e.is_tamed, e.position_is_honest))
            .map(|e| e.label)
            .collect();
        if let Some(tile) = game
            .view_tiles_at((x, y), 0, 0)
            .first()
            .and_then(|r| r.first())
        {
            lines.push(tile.biome.name().to_string());
        }
        lines
    }
}
