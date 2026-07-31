//! The screens outside a run: main menu, save picker, difficulty pick,
//! help, and the game-over page.

use crate::*;

impl App {
    pub(crate) fn handle_main_menu_key(&mut self, key: GameKey) {
        let mut options = vec!['n'];
        if !self.list_saves().is_empty() {
            options.push('l');
        }
        options.push('q');
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('n') => {
                self.status_line = None;
                self.mode = Mode::DifficultyPick;
            }
            Some('l') => {
                self.status_line = None;
                self.mode = Mode::LoadGame;
            }
            Some('q') => self.mode = Mode::QuitAppConfirm,
            _ => {}
        }
    }

    pub(crate) fn handle_load_game_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let saves = self.list_saves();
        if let Some(idx) = self.selected_index(key, saves.len()) {
            self.pending_save = Some(saves[idx].path.clone());
            self.mode = Mode::SaveAction;
        }
    }

    pub(crate) fn handle_save_action_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_save = None;
            self.mode = Mode::LoadGame;
            return;
        }
        let Some(path) = self.pending_save.clone() else {
            self.mode = Mode::LoadGame;
            return;
        };
        let options = ['l', 'x'];
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('l') => {
                self.pending_save = None;
                self.load_game(path);
            }
            Some('x') => {
                self.pending_save = None;
                match std::fs::remove_file(&path) {
                    Ok(()) => self.status_line = Some("Save deleted.".to_string()),
                    Err(e) => self.status_line = Some(format!("Delete failed: {e}")),
                }
                self.mode = Mode::LoadGame;
            }
            _ => {}
        }
    }

    pub(crate) fn handle_difficulty_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let options = ['p', 'f'];
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('p') => self.start_new_game(DifficultyMode::Permadeath),
            Some('f') => self.start_new_game(DifficultyMode::Forgiving),
            _ => {}
        }
    }

    pub(crate) fn handle_help_key(&mut self) {
        self.mode = Mode::Playing;
    }

    /// The message log in full. Nothing here is selectable, so the only keys
    /// that do anything are Up/Down — which move `menu_selected`, and with it
    /// the popup's scroll window (see `popup_layout`) — and Esc.
    ///
    /// Deliberately not closing on any key, the way `Help` and `FrameMap`
    /// do: those are glanced at, this one is read, and a screen you scroll
    /// through must not vanish under the keys you scroll it with.
    pub(crate) fn handle_history_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        // Folded rows, not raw lines: repeats are condensed into one row (see
        // `Game::message_history`), and the highlight *is* the scroll
        // position, so counting anything the renderer doesn't draw would let
        // it run off the end of the list.
        let rows = self
            .game
            .as_ref()
            .map(|g| g.message_history(MESSAGE_LOG_CAP).len())
            .unwrap_or(0);
        self.scroll(key, rows);
    }

    /// The structure roster. Read-only for the same reason the history is:
    /// assigning a worker, demolishing and upgrading each have their own
    /// screen, and this one exists to answer "what have I got, and what is on
    /// it" without becoming a fourth way to do any of that.
    pub(crate) fn handle_structures_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let rows = self
            .game
            .as_mut()
            .map(|g| g.structure_report().len())
            .unwrap_or(0);
        self.scroll(key, rows);
    }

    /// The map closes on any key, like the help screen: it is something you
    /// glance at mid-corridor, and making the player find the right key to
    /// put it away would be friction on the one screen meant to remove some.
    pub(crate) fn handle_frame_map_key(&mut self) {
        self.mode = Mode::Playing;
    }

    pub(crate) fn handle_game_over_key(&mut self) {
        self.game = None;
        self.status_line = None;
        self.mode = Mode::MainMenu;
    }
}
