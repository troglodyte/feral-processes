//! The top-level key dispatch every screen's handler hangs off, and the
//! idle clock that keeps the world moving between key presses.

use crate::*;

impl App {
    /// Shared Up/Down/Enter handling layered on top of every menu's direct
    /// row shortcuts — this doesn't replace them, it's just another way to
    /// pick the same row. `len` is how many selectable rows the menu
    /// currently has. A typed shortcut (see `menu_shortcut`) resolves
    /// immediately to that 0-based index; Up/Down instead move
    /// `menu_selected` (wrapping) and return `None`; Enter resolves to
    /// whatever `menu_selected` currently highlights. Any other key, or an
    /// empty menu, returns `None`.
    pub(crate) fn selected_index(&mut self, key: GameKey, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        if let GameKey::Char(c) = key {
            if let Some(d) = c.to_digit(10) {
                let d = d as usize;
                return (d >= 1 && d <= len).then_some(d - 1);
            }
            if !c.is_ascii_alphabetic() {
                return None;
            }
            let idx = DIGIT_ROWS + (c.to_ascii_lowercase() as usize - 'a' as usize);
            return (idx < len).then_some(idx);
        }
        match key {
            GameKey::Up => {
                self.menu_selected = (self.menu_selected + len - 1) % len;
                None
            }
            GameKey::Down => {
                self.menu_selected = (self.menu_selected + 1) % len;
                None
            }
            GameKey::Enter => Some(self.menu_selected.min(len - 1)),
            _ => None,
        }
    }

    pub fn handle_key(&mut self, key: GameKey) {
        // A key pressed while narration is still scrolling in dumps the rest
        // and is not acted on. Without this the pacing would be a tax on
        // anyone who reads faster than it scrolls.
        if self.is_revealing() {
            self.finish_reveal();
            return;
        }
        // Restarts the refusal window: whatever this key produces (or leaves
        // standing) gets its full time on screen, rather than inheriting the
        // remainder of the previous message's.
        self.status_age = 0.0;
        let mode_before = self.mode;
        match self.mode {
            Mode::MainMenu => self.handle_main_menu_key(key),
            Mode::LoadGame => self.handle_load_game_key(key),
            Mode::SaveAction => self.handle_save_action_key(key),
            Mode::DifficultyPick => self.handle_difficulty_key(key),
            Mode::Playing => self.handle_playing_key(key),
            Mode::Battle => self.handle_battle_key(key),
            Mode::BattleTarget => self.handle_battle_target_key(key),
            Mode::BattleItem => self.handle_battle_item_key(key),
            Mode::BattleSpecial => self.handle_battle_special_key(key),
            Mode::BattleAlly => self.handle_battle_ally_key(key),
            Mode::Build => self.handle_build_key(key),
            Mode::BuildDirection => self.handle_build_direction_key(key),
            Mode::Craft => self.handle_craft_key(key),
            Mode::CraftQuantity => self.handle_craft_quantity_key(key),
            Mode::Cronjob => self.handle_cronjob_key(key),
            Mode::CronjobStructure => self.handle_cronjob_structure_key(key),
            Mode::Guard => self.handle_guard_key(key),
            Mode::GuardStructure => self.handle_guard_structure_key(key),
            Mode::Remove => self.handle_remove_key(key),
            Mode::RemoveConfirm => self.handle_remove_confirm_key(key),
            Mode::TradeProgramConfirm => self.handle_trade_program_confirm_key(key),
            Mode::Upgrade => self.handle_upgrade_key(key),
            Mode::Symlink => self.handle_symlink_key(key),
            Mode::InspectDirection => self.handle_inspect_direction_key(key),
            Mode::InspectDetail => self.handle_inspect_detail_key(key),
            Mode::Inventory => self.handle_inventory_key(key),
            Mode::InventoryItemAction => self.handle_inventory_item_action_key(key),
            Mode::EraseQuantity => self.handle_erase_quantity_key(key),
            Mode::Companion => self.handle_companion_key(key),
            Mode::Fuse => self.handle_fuse_key(key),
            Mode::FuseSecond => self.handle_fuse_second_key(key),
            Mode::FuseName => self.handle_fuse_name_key(key),
            Mode::RoutineTarget => self.handle_routine_target_key(key),
            Mode::Routines => self.handle_routines_key(key),
            Mode::RoutineInstall => self.handle_routine_install_key(key),
            Mode::Extract => self.handle_extract_key(key),
            Mode::ExtractPick => self.handle_extract_pick_key(key),
            Mode::ExtractConfirm => self.handle_extract_confirm_key(key),
            Mode::Trade => self.handle_trade_key(key),
            Mode::TradeAction => self.handle_trade_action_key(key),
            Mode::TradeQuantity => self.handle_trade_quantity_key(key),
            Mode::Perks => self.handle_perks_key(key),
            Mode::Research => self.handle_research_key(key),
            Mode::Help => self.handle_help_key(),
            Mode::GameOver => self.handle_game_over_key(),
        }
        // Every menu's arrow-key highlight (see `selected_index`) starts
        // fresh at the top of its list, rather than carrying over whatever
        // row happened to be highlighted on a previous, unrelated menu.
        if self.mode != mode_before {
            self.menu_selected = 0;
        }
        self.maybe_autosave();
    }

    /// Releases battle narration into the log pane at
    /// `REVEAL_LINES_PER_SECOND`, so a resolved round reads as it arrives
    /// rather than landing as a block. A frontend calls this once a frame
    /// with that frame's delta.
    ///
    /// Takes the delta rather than reading a clock: the suite forbids
    /// wall-clock dependence, and an injected `dt` is what makes the pacing
    /// testable without a sleep.
    pub fn advance_reveal(&mut self, dt: f32) {
        let Some(game) = &self.game else { return };
        let id = game.battle_log_id();
        let total = game.battle_log().len();
        if self.reveal.battle_id != id {
            self.reveal = BattleReveal {
                battle_id: id,
                ..BattleReveal::default()
            };
        }
        if self.reveal.revealed >= total {
            return;
        }
        self.reveal.accumulated += dt * REVEAL_LINES_PER_SECOND;
        while self.reveal.accumulated >= 1.0 && self.reveal.revealed < total {
            self.reveal.accumulated -= 1.0;
            self.reveal.revealed += 1;
        }
        // Credit left over once the last line is out would otherwise be
        // banked and spent the instant the next round logs, dumping it whole
        // — the very thing this pacing exists to prevent.
        if self.reveal.revealed >= total {
            self.reveal.accumulated = 0.0;
        }
    }

    /// Ages the status line out after `STATUS_LINE_SECONDS`, so a refusal
    /// stops covering the action bar it was drawn over. A frontend calls
    /// this once a frame alongside `advance_reveal`, and for the same
    /// reason it takes the frame's delta rather than reading a clock.
    pub fn advance_status(&mut self, dt: f32) {
        if self.status_line.is_none() {
            return;
        }
        self.status_age += dt;
        if self.status_age >= STATUS_LINE_SECONDS {
            self.status_line = None;
            self.status_age = 0.0;
        }
    }

    /// Whether narration is still scrolling in. While this holds, a frontend
    /// suppresses the action bar and `handle_key` skips rather than acting.
    pub fn is_revealing(&self) -> bool {
        let Some(game) = &self.game else {
            return false;
        };
        self.reveal.revealed < game.battle_log().len()
    }

    /// The battle pane's lines: this battle's narration, truncated to what
    /// has been revealed. The pane draws the tail of this once it overflows,
    /// which is what makes lines scroll up as new ones arrive.
    pub fn revealed_battle_log(&self) -> Vec<(MessageKind, String)> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        let mut lines = game.battle_log();
        lines.truncate(self.reveal.revealed);
        lines
    }

    /// How many lines the *base* screen must chop off the tail of
    /// `Game::message_log` — the battle results that have not scrolled in
    /// yet. Zero except in the moments after a battle ends.
    pub fn hidden_log_lines(&self) -> usize {
        let Some(game) = &self.game else { return 0 };
        game.battle_log().len().saturating_sub(self.reveal.revealed)
    }

    /// Releases every remaining line at once — the skip.
    pub(crate) fn finish_reveal(&mut self) {
        self.reveal.revealed = self.game.as_ref().map_or(0, |g| g.battle_log().len());
        self.reveal.accumulated = 0.0;
    }

    /// Starts this battle's narration over from nothing.
    pub(crate) fn restart_reveal(&mut self) {
        let id = self.game.as_ref().map_or(0, |g| g.battle_log_id());
        self.reveal = BattleReveal {
            battle_id: id,
            ..BattleReveal::default()
        };
    }

    /// Advances the world by one idle tick if a real second has passed
    /// since the last one — called every frame by a frontend's own loop
    /// (independent of `handle_key`, which only fires on input) so the
    /// world keeps moving while the player sits idle. Ticking only happens
    /// in `Mode::Playing`: every other mode — battle included, since
    /// entering one switches away from `Playing` — is treated as paused,
    /// and the wall-clock timer resets rather than banking elapsed time,
    /// so coming back from a menu never triggers a burst of catch-up ticks.
    pub fn update_realtime(&mut self) {
        if self.mode != Mode::Playing {
            self.last_realtime_tick = Instant::now();
            return;
        }
        let Some(game) = &mut self.game else {
            self.last_realtime_tick = Instant::now();
            return;
        };
        if self.last_realtime_tick.elapsed() < REALTIME_TICK_INTERVAL {
            return;
        }
        self.last_realtime_tick = Instant::now();
        game.idle_tick();
        self.maybe_autosave();
    }
}
