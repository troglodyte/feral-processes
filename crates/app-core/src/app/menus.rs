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
            Some('q') => self.quit = true,
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

    pub(crate) fn handle_game_over_key(&mut self) {
        self.game = None;
        self.status_line = None;
        self.mode = Mode::MainMenu;
    }
}
