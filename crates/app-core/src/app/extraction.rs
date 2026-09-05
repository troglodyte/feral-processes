//! `Mode::DownedPrograms`: the downed-program store, reached from the pack
//! with `D`, and the tool-and-yield picker for one held program — see
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`, section
//! 6.

use crate::*;

impl App {
    /// Two phases sharing one `Mode`, `App::pending_downed_program_index`
    /// the whole difference — `None` is the list, `Some(index)` is that
    /// row's tool-and-yield page. Not `Mode::Develop`'s two-`Mode` shape:
    /// there is no per-tier ladder here to keep apart from the list.
    ///
    /// Row counts come from the engine on every keypress
    /// (`Game::downed_program_rows`/`Game::extraction_options`) rather than
    /// being cached, so a store that shrank under the player (an extraction
    /// just spent, a program benched elsewhere) cannot be indexed past its
    /// own end — `selected_index`'s bound is exactly `len`.
    pub(crate) fn handle_downed_programs_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            if self.pending_downed_program_index.take().is_none() {
                // Reached from the pack alone (`D` on `Mode::Inventory`),
                // never through a group menu — `close_screen`'s
                // `menu_origin` is for `Mode::BaseMenu`/`Mode::PartyMenu`
                // and was never set opening this screen, so it would send
                // Esc to `Mode::Playing` instead of back to the pack.
                self.mode = Mode::Inventory;
            } else {
                self.menu_selected = 0;
            }
            return;
        }
        match self.pending_downed_program_index {
            None => {
                let Some(len) = self.game.as_ref().map(|g| g.downed_program_rows().len()) else {
                    return;
                };
                if let Some(idx) = self.selected_index(key, len) {
                    self.pending_downed_program_index = Some(idx);
                    self.menu_selected = 0;
                }
            }
            Some(program_index) => {
                let Some(options) = self
                    .game
                    .as_ref()
                    .map(|g| g.extraction_options(program_index))
                else {
                    return;
                };
                let Some(tool_idx) = self.selected_index(key, options.len()) else {
                    return;
                };
                let tool_id = options[tool_idx].tool.clone();
                let outcome = self
                    .game
                    .as_mut()
                    .unwrap()
                    .extract_program(program_index, &tool_id);
                self.report(outcome);
                // Back to the list either way: on success the row is gone,
                // and on a refusal (game over, a battle opening under the
                // player) the tool page for it is no longer a page worth
                // being on.
                self.pending_downed_program_index = None;
                self.menu_selected = 0;
            }
        }
    }
}
