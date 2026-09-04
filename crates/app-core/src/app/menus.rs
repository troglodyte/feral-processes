//! The screens outside a run: main menu, save picker, help, and the
//! game-over page. The creation wizard the main menu opens is its own
//! module, `app/creation.rs`.

use crate::*;

impl App {
    pub(crate) fn handle_main_menu_key(&mut self, key: GameKey) {
        let mut options = vec!['n'];
        if !self.list_saves().is_empty() {
            options.push('l');
        }
        options.push('a');
        if self.arena_enabled {
            options.push('r');
        }
        if self.sprite_forge_enabled() {
            options.push('d');
        }
        options.push('q');
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('n') => self.open_creation(),
            Some('l') => {
                self.status_line = None;
                self.mode = Mode::LoadGame;
            }
            Some('a') => {
                self.status_line = None;
                self.mode = Mode::Achievements;
            }
            Some('r') => self.open_arena(),
            Some('d') => {
                self.status_line = None;
                self.mode = Mode::SpritePicker;
            }
            Some('q') => self.mode = Mode::QuitAppConfirm,
            _ => {}
        }
    }

    /// Read-only, and the one screen reachable without a run — Esc goes back
    /// to the main menu rather than through `close_screen`, which returns to
    /// the map or the group menu a screen was opened from.
    pub(crate) fn handle_achievements_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let rows = self.achievement_rows().len();
        self.scroll(key, rows);
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

    /// The manual's index — an ordinary numbered menu, so more than nine
    /// topics is already solved by `menu_shortcut`. Esc closes back to
    /// wherever `?` was pressed.
    ///
    /// `?` no longer closes on any key. The screen is navigable now, and a
    /// screen you walk through must not vanish under the keys you walk it
    /// with — the same rule `handle_history_key` follows.
    pub(crate) fn handle_help_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.help_stack.clear();
            self.close_screen();
            return;
        }
        let rows = self.help_index_rows();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let Some(page) = self.help_db.pages().get(idx) else {
            return;
        };
        self.help_stack.push(page.id.clone());
        self.mode = Mode::HelpPage;
    }

    /// One page — a document, not a menu. Up/Down scroll it and Enter does
    /// nothing; a further-reading row is followed by typing its shortcut,
    /// and Esc pops one level of the reading trail.
    pub(crate) fn handle_help_page_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.help_stack.pop();
            if self.help_stack.is_empty() {
                self.mode = Mode::Help;
            }
            // The scroll *is* `menu_selected` here, and moving between two
            // pages is not a mode change, so nothing else would reset it.
            self.menu_selected = 0;
            return;
        }
        let Some(view) = self.help_page_view() else {
            return;
        };
        // A link row is addressed by position, not by its label: the label
        // is prose an author wrote in the middle of a sentence, and only the
        // page's own `links` list knows what it points at.
        if let GameKey::Char(c) = key
            && let Some(idx) = view.links.iter().position(|l| l.shortcut == c)
        {
            let target = self
                .current_help_page()
                .and_then(|p| p.links.get(idx))
                .map(|l| l.target.clone());
            if let Some(target) = target {
                self.help_stack.push(target);
                self.menu_selected = 0;
            }
            return;
        }
        self.scroll(key, view.prose.len());
    }

    /// The index's rows, in the db's own `(order, id)` order.
    pub fn help_index_rows(&self) -> Vec<HelpIndexRow> {
        self.help_db
            .pages()
            .iter()
            .map(|p| HelpIndexRow {
                title: p.title.clone(),
            })
            .collect()
    }

    /// The page on top of the reading trail, wrapped and ready to draw.
    pub fn help_page_view(&self) -> Option<HelpPageView> {
        let page = self.current_help_page()?;
        Some(HelpPageView {
            title: page.title.clone(),
            prose: help::page_rows(page, help::WRAP_COLUMNS),
            links: page
                .links
                .iter()
                .enumerate()
                .map(|(i, link)| HelpLinkRow {
                    shortcut: menu_shortcut(i),
                    label: link.label.clone(),
                })
                .collect(),
        })
    }

    fn current_help_page(&self) -> Option<&HelpPage> {
        self.help_db.page(self.help_stack.last()?)
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
            self.close_screen();
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

    /// The structure roster. Scrolls like the history, and Enter staffs the
    /// highlighted structure — see `Mode::Structures` for why this one screen
    /// acts where the other two read-only views don't.
    ///
    /// The refusals sit here rather than in the picker because the roster
    /// cannot filter its rows the way `App::upgradeable_structures` does:
    /// showing the whole base, workable or not, is what the screen is for. So
    /// a row that takes no worker says so instead of opening a picker that
    /// could only be refused on the far side.
    pub(crate) fn handle_structures_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let report = self
            .game
            .as_mut()
            .map(|g| g.structure_report())
            .unwrap_or_default();
        let Some(idx) = self.selected_index(key, report.len()) else {
            return;
        };
        // The picker this opens acts on the structure three ways, and
        // `Game::work_structure` — the one that stands the player at its
        // station tile — is behind `require_base`, so the screen is only
        // fully usable in one locale. The guard used to ask
        // `is_underground()`, which was the same question when there were
        // two locales and is not now: on the open grid the keypress was
        // permitted and the engine refused it a moment later, which reads to
        // the player as the key doing nothing.
        //
        // The other two rows toggle a standing job, which the engine leaves
        // unguarded — so this refuses slightly more than the engine would.
        // Deliberate: every structure stands in base space, and a screen
        // that opened out on the grid to offer two of its three rows would
        // be harder to explain than one that says where the base is.
        //
        // Refused at the keypress like the demolish key, rather than opening
        // a picker whose every row is a dead end — the roster itself still
        // reads fine from anywhere, which is why it carries no `base_only`
        // flag in `BASE_ROWS`.
        if self.game.as_ref().is_some_and(|g| !g.in_base()) {
            self.refuse("Not from out here — that's back at the base.");
            return;
        }
        if !report[idx].workable {
            self.refuse("Nothing can be posted to that.");
            return;
        }
        self.pending_post_structure = Some(report[idx].entity);
        self.mode = Mode::StructureAssign;
    }

    /// The recipe chains. Scrolls by *chain* rather than by drawn line: the
    /// steps under a product are sub-rows the renderer draws unselectable,
    /// exactly as the roster draws a structure's assignees, so app-core owns
    /// a count it can derive without knowing the renderer's line shape.
    pub(crate) fn handle_recipes_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
            return;
        }
        let rows = self
            .game
            .as_ref()
            .map(|g| g.recipe_chains().len())
            .unwrap_or(0);
        self.scroll(key, rows);
    }

    /// What the base has made. Esc and nothing else: the page has no scroll
    /// — `Game::base_output_report` caps its own rows — so there is no
    /// highlight for the arrows to move.
    pub(crate) fn handle_base_output_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.close_screen();
        }
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
