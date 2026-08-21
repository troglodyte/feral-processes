//! The top-level key dispatch every screen's handler hangs off, and the
//! idle clock that keeps the world moving between key presses.

use crate::*;

/// The transitions that re-enter a list the player was already in, and so
/// must not have their highlight reset to the top of it. Every other mode
/// change starts fresh — see `App::handle_key`.
///
/// The first two are a screen backing out into the one that opened it, and in
/// both the returning handler has already put `menu_selected` on the row the
/// side-trip was about: `leave_manifest` on whoever the sheet was showing,
/// `leave_staffing` on the structure that was staffed.
///
/// The roster's pair is the outward leg as well, which the others don't need.
/// `Mode::Manifest` indexes nothing with `menu_selected` — it pages with ←/→
/// off `pending_manifest` — so the roster's row is simply parked there for the
/// duration, and that parked value is what Esc restores when the sheet has
/// been paged onto the player, who has no roster row for `leave_manifest` to
/// find.
fn keeps_highlight(before: Mode, after: Mode) -> bool {
    matches!(
        (before, after),
        (Mode::Manifest, Mode::ManifestPick)
            | (Mode::StructureAssign, Mode::Structures)
            | (Mode::Companion, Mode::Manifest)
            | (Mode::Manifest, Mode::Companion)
            // Same pair as the manifest above, for the same reason: the
            // memories page indexes nothing with the highlight, so the row
            // the player was reading down the roster is what Esc returns to.
            | (Mode::Companion, Mode::CompanionMemories)
            | (Mode::CompanionMemories, Mode::Companion)
    )
}

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
                // Rows are labelled from `1` (see `menu_shortcut`), so `0`
                // labels nothing and must fall out here. Subtracting through
                // `checked_sub` rather than guarding with `d >= 1` is what
                // keeps that structural: `then_some` takes its argument by
                // value, so a guard leaves `0usize - 1` still evaluated.
                return (d as usize).checked_sub(1).filter(|&idx| idx < len);
            }
            // Lowercase only. Uppercase is reserved for screen actions —
            // `S`/`B` quick-trade a single unit off the highlighted row —
            // and a shifted letter that also picked a row would fire both
            // on one keypress, on a screen where one of them spends money.
            if !c.is_ascii_lowercase() {
                return None;
            }
            let idx = DIGIT_ROWS + (c as usize - 'a' as usize);
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

    /// Up/Down scrolling for a screen with nothing to pick: the read-only
    /// views (see `handle_history_key`) are drawn into the same popup as every
    /// menu, and its window follows the highlighted row, so moving that
    /// highlight *is* scrolling. `selected_index` already does the moving;
    /// what this adds is discarding the row it would have resolved to, in one
    /// named place rather than at each call site.
    pub(crate) fn scroll(&mut self, key: GameKey, len: usize) {
        let _ = self.selected_index(key, len);
    }

    /// Which row a freshly opened screen highlights. Every menu starts at its
    /// first, and the history starts at its last: its lines run oldest-first
    /// to match the map's pane, so the newest is at the bottom — and the line
    /// that just scrolled off that pane is the reason the screen was opened.
    fn opening_row(&self) -> usize {
        match self.mode {
            Mode::History => self
                .game
                .as_ref()
                .map(|g| g.message_history(MESSAGE_LOG_CAP).len().saturating_sub(1))
                .unwrap_or(0),
            _ => 0,
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
        // Whatever this key raises gets its full time on screen rather than
        // inheriting the remainder of the previous message's — but a message
        // it leaves *standing* keeps the window it was already ageing
        // through. Restarting the window unconditionally meant that in a
        // battle, where nothing clears a refusal on success and several keys
        // go into planning a round, a refusal never aged out at all.
        let carried = self.status_line.clone();
        let mode_before = self.mode;
        match self.mode {
            Mode::MainMenu => self.handle_main_menu_key(key),
            Mode::Achievements => self.handle_achievements_key(key),
            Mode::LoadGame => self.handle_load_game_key(key),
            Mode::SaveAction => self.handle_save_action_key(key),
            Mode::DifficultyPick => self.handle_difficulty_key(key),
            Mode::Playing => self.handle_playing_key(key),
            Mode::BaseMenu => self.handle_base_menu_key(key),
            Mode::PartyMenu => self.handle_party_menu_key(key),
            Mode::Battle => self.handle_battle_key(key),
            Mode::BattleTarget => self.handle_battle_target_key(key),
            Mode::BattleItem => self.handle_battle_item_key(key),
            Mode::BattleSpecial => self.handle_battle_special_key(key),
            Mode::BattleAlly => self.handle_battle_ally_key(key),
            Mode::Build => self.handle_build_key(key),
            Mode::DevConsole => self.handle_dev_console_key(key),
            Mode::BuildDirection => self.handle_build_direction_key(key),
            Mode::Craft => self.handle_craft_key(key),
            Mode::CraftQuantity => self.handle_craft_quantity_key(key),
            Mode::WorkOrders => self.handle_work_orders_key(key),
            Mode::WorkOrderPick => self.handle_work_order_pick_key(key),
            Mode::WorkOrderQuantity => self.handle_work_order_quantity_key(key),
            Mode::BaseStaff => self.handle_base_staff_key(key),
            Mode::WorkStructure => self.handle_work_structure_key(key),
            Mode::Remove => self.handle_remove_key(key),
            Mode::RemoveConfirm => self.handle_remove_confirm_key(key),
            Mode::RemoveDirection => self.handle_remove_direction_key(key),
            Mode::TradeProgramConfirm => self.handle_trade_program_confirm_key(key),
            Mode::Upgrade => self.handle_upgrade_key(key),
            Mode::Symlink => self.handle_symlink_key(key),
            Mode::InspectDirection => self.handle_inspect_direction_key(key),
            Mode::StructureManifest => self.handle_structure_manifest_key(key),
            Mode::CellDescribe => self.handle_cell_describe_key(key),
            Mode::Manifest => self.handle_manifest_key(key),
            Mode::ManifestPick => self.handle_manifest_pick_key(key),
            Mode::Inventory => self.handle_inventory_key(key),
            Mode::EquipSwap => self.handle_equip_swap_key(key),
            Mode::InventoryItemAction => self.handle_inventory_item_action_key(key),
            Mode::ItemDescribe => self.handle_item_describe_key(key),
            Mode::EraseQuantity => self.handle_erase_quantity_key(key),
            Mode::Companion => self.handle_companion_key(key),
            Mode::CompanionEquip => self.handle_companion_equip_key(key),
            Mode::CompanionMemories => self.handle_companion_memories_key(key),
            Mode::Fuse => self.handle_fuse_key(key),
            Mode::FuseSecond => self.handle_fuse_second_key(key),
            Mode::FuseName => self.handle_fuse_name_key(key),
            Mode::RenamePet => self.handle_rename_pet_key(key),
            Mode::RoutineTarget => self.handle_routine_target_key(key),
            Mode::Routines => self.handle_routines_key(key),
            Mode::RoutineInstall => self.handle_routine_install_key(key),
            Mode::RoutineEtch => self.handle_routine_etch_key(key),
            Mode::FieldCast => self.handle_field_cast_key(key),
            Mode::FieldCastAlly => self.handle_field_cast_ally_key(key),
            Mode::FieldCastCell => self.handle_field_cast_cell_key(key),
            Mode::Excavate => self.handle_excavate_key(key),
            Mode::Refactor => self.handle_refactor_key(key),
            Mode::RefactorItem => self.handle_refactor_item_key(key),
            Mode::Develop => self.handle_develop_key(key),
            Mode::DevelopProgram => self.handle_develop_program_key(key),
            Mode::Extract => self.handle_extract_key(key),
            Mode::ExtractPick => self.handle_extract_pick_key(key),
            Mode::ExtractConfirm => self.handle_extract_confirm_key(key),
            Mode::Trade => self.handle_trade_key(key),
            Mode::TradeAction => self.handle_trade_action_key(key),
            Mode::TradeQuantity => self.handle_trade_quantity_key(key),
            Mode::StackMarket => self.handle_stack_market_key(key),
            Mode::Perks => self.handle_perks_key(key),
            Mode::Research => self.handle_research_key(key),
            Mode::Contracts => self.handle_contracts_key(key),
            Mode::History => self.handle_history_key(key),
            Mode::Structures => self.handle_structures_key(key),
            Mode::StructureAssign => self.handle_structure_assign_key(key),
            Mode::Recipes => self.handle_recipes_key(key),
            Mode::Help => self.handle_help_key(key),
            Mode::HelpPage => self.handle_help_page_key(key),
            Mode::FrameMap => self.handle_frame_map_key(),
            Mode::GameOver => self.handle_game_over_key(),
            Mode::QuitRunConfirm => self.handle_quit_run_confirm_key(key),
            Mode::QuitAppConfirm => self.handle_quit_app_confirm_key(key),
            Mode::ArenaBuilder => self.handle_arena_builder_key(key),
            Mode::ArenaLoad => self.handle_arena_load_key(key),
            Mode::ArenaSave => self.handle_arena_save_key(key),
            Mode::ArenaPick => self.handle_arena_pick_key(key),
            Mode::ArenaResult => self.handle_arena_result_key(key),
            Mode::BattleResult => self.handle_battle_result_key(key),
        }
        // Every menu's arrow-key highlight (see `selected_index`) starts
        // fresh at the top of its list, rather than carrying over whatever
        // row happened to be highlighted on a previous, unrelated menu.
        if self.mode != mode_before && !keeps_highlight(mode_before, self.mode) {
            self.menu_selected = self.opening_row();
        }
        // A group menu's origin only outlives the screen it opened. Landing
        // on the map means the errand is over — completing an action drops
        // straight here — and an origin left standing would send some later,
        // unrelated screen's Esc back to a menu the player long since left.
        if self.mode == Mode::Playing {
            self.menu_origin = None;
        }
        self.after_tick();
        // Last, because `after_tick` is itself allowed to raise a message.
        if self.status_line != carried {
            self.status_age = 0.0;
        }
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
        let generation = game.battle_log_generation();
        let total = game.battle_log().len();
        if self.reveal.generation != generation {
            self.reveal = BattleReveal {
                generation,
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
        // banked and spent the instant more lines land, dumping them whole.
        // A new round resets the whole counter anyway; this covers the case
        // where the *same* range grows — the `tick` inside a battle action
        // can have a background system log into it after the reveal caught
        // up.
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

    /// How many lines are on screen right now.
    ///
    /// A count belonging to a superseded generation reads as zero rather
    /// than as itself: the round it counted has been replaced, and the new
    /// one has shown nothing yet. `advance_reveal` is what actually resets
    /// the counter, but it runs once a frame — so between a round resolving
    /// and the next frame, every reader has to agree the pane is empty, or
    /// the finished round flashes up whole before the new one starts
    /// scrolling.
    fn revealed_count(&self) -> usize {
        let Some(game) = &self.game else { return 0 };
        if self.reveal.generation != game.battle_log_generation() {
            return 0;
        }
        self.reveal.revealed
    }

    /// How many lines of the round's range have yet to scroll in — the one
    /// definition of "the narration is behind", read by `is_revealing` and by
    /// `hidden_log_lines`.
    ///
    /// **Zero off a battle screen, and that gate is the whole of it.**
    /// `MessageLog::round_start` is deliberately never closed — the results
    /// are still scrolling in after the fight, so the range has to outlive it
    /// — which means `Game::battle_log` goes on growing with ordinary map and
    /// base news for the rest of the run once a fight has opened it. The
    /// battle pane is the only screen that draws any of that as narration, so
    /// ungated this paced the *map's* log pane too and, through
    /// `handle_key`'s skip, swallowed a keypress for every line a running
    /// base logged. `Mode::is_battle` rather than "a battle is live" because
    /// `Mode::BattleResult` outlives `BattleState` and is exactly when the
    /// results are scrolling in.
    fn unrevealed(&self) -> usize {
        let Some(game) = &self.game else { return 0 };
        if !self.mode.is_battle() {
            return 0;
        }
        game.battle_log()
            .len()
            .saturating_sub(self.revealed_count())
    }

    /// Whether narration is still scrolling in. While this holds, a frontend
    /// suppresses the action bar and `handle_key` skips rather than acting.
    pub fn is_revealing(&self) -> bool {
        self.unrevealed() > 0
    }

    /// The battle screen's roster, stepped to what the narration has
    /// actually shown — see `Game::battle_view_at`.
    ///
    /// **A renderer draws this one, never `Game::battle_view`.** The two
    /// disagree for the second or two a round takes to scroll in, and the
    /// live one is right for exactly one caller: `battle_target_key`, which
    /// maps a typed group letter onto `BattleState::groups` and would
    /// otherwise resolve a letter against a group that has already died.
    /// That stays safe because `handle_key` skips rather than acting while
    /// `is_revealing` holds, which is what
    /// `a_key_pressed_mid_reveal_skips_instead_of_acting` pins.
    ///
    /// On `Mode::BattleResult` it answers from the closing roster instead,
    /// because `end_battle` has removed `BattleState` and the live view is
    /// `None` — that fallback is what lets a finished fight keep its own
    /// screen while the results scroll into the pane, rather than handing
    /// the player off to a summary page. Gated on the mode rather than on
    /// "no live battle" so a stale roster can never surface on the map.
    pub fn battle_view(&self) -> Option<BattleView> {
        let game = self.game.as_ref()?;
        if self.mode == Mode::BattleResult {
            return game.battle_result_view();
        }
        game.battle_view_at(self.revealed_count())
    }

    /// The battle pane's lines: this battle's narration, truncated to what
    /// has been revealed and stripped of base news — see `battle_rows`. The
    /// pane draws the tail of this once it overflows, which is what makes
    /// lines scroll up as new ones arrive.
    pub fn revealed_battle_log(&self) -> Vec<LogEntry> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        battle_rows(&game.battle_log(), self.revealed_count())
    }

    /// The battle pane's window on `revealed_battle_log`, `capacity` rows
    /// tall, walked back by however far the player has scrolled.
    ///
    /// Takes `&mut self` because it is also where the scroll is *clamped*:
    /// `capacity` is the frontend's, derived from the pixels the pane was
    /// given, so this is the only place that can tell an Up key that has run
    /// out of narration from one that has not. A frontend calls it once a
    /// frame, which is what keeps a held-down arrow from banking presses
    /// against a pane that stopped moving several of them ago.
    ///
    /// Without this the pane was pinned to the newest line and everything
    /// above it was unreachable — and unrecoverable, since a new round
    /// replaces the range (`Game::battle_log` is `since_round`) and
    /// `retain_outcomes_since_battle` deletes the blow-by-blow when the
    /// fight ends.
    pub fn battle_pane(&mut self, capacity: usize) -> BattlePane {
        let lines = self.revealed_battle_log();
        let slack = lines.len().saturating_sub(capacity);
        self.reveal.scroll = self.reveal.scroll.min(slack);
        let end = lines.len() - self.reveal.scroll;
        let start = end.saturating_sub(capacity);
        BattlePane {
            rows: lines[start..end].to_vec(),
            above: start,
            below: self.reveal.scroll,
        }
    }

    /// Walks the battle pane's window one line back or forward. Bounded here
    /// only by the narration itself; `battle_pane` applies the real ceiling
    /// once it knows how tall the pane is.
    pub(crate) fn scroll_battle_pane(&mut self, key: GameKey) -> bool {
        match key {
            GameKey::Up => {
                let lines = self.revealed_battle_log().len();
                self.reveal.scroll = (self.reveal.scroll + 1).min(lines.saturating_sub(1));
                true
            }
            GameKey::Down => {
                self.reveal.scroll = self.reveal.scroll.saturating_sub(1);
                true
            }
            _ => false,
        }
    }

    /// The map log pane's rows under the active filter, oldest first — see
    /// `pane_rows` for why the chop, the filter and the capacity cut happen in
    /// that order.
    ///
    /// Row selection lives here rather than in the renderer for the same
    /// reason the history screen's fold does: app-core owns what a read-only
    /// screen's rows *are*, and gui draws them.
    pub fn visible_log(&self, capacity: usize) -> Vec<LogEntry> {
        let Some(game) = &self.game else {
            return Vec::new();
        };
        let hidden = self.hidden_log_lines();
        pane_rows(
            &game.message_log(MESSAGE_LOG_CAP),
            hidden,
            self.log_filter,
            capacity,
        )
    }

    /// How many retained lines the pane's filter is holding back, for the
    /// header. Counted over the whole log rather than the pane's window: the
    /// point is to say that a channel you stopped watching has traffic in it.
    pub fn filtered_out_log_lines(&self) -> usize {
        let Some(game) = &self.game else { return 0 };
        filtered_out_count(&game.message_log(MESSAGE_LOG_CAP), self.log_filter)
    }

    /// How many lines the *base* screen must chop off the tail of
    /// `Game::message_log` — narration that has not scrolled in yet. Zero
    /// whenever the map is what's on screen, so the map's pane never holds a
    /// line back: see `unrevealed` for why that gate is where the pacing
    /// stops being a battle-pane concern.
    pub fn hidden_log_lines(&self) -> usize {
        self.unrevealed()
    }

    /// Releases every remaining line at once — the skip.
    pub(crate) fn finish_reveal(&mut self) {
        let Some(game) = &self.game else { return };
        self.reveal = BattleReveal {
            revealed: game.battle_log().len(),
            accumulated: 0.0,
            generation: game.battle_log_generation(),
            // The skip means "show me the end", so it lands on the end.
            scroll: 0,
        };
    }

    /// Starts this battle's narration over from nothing.
    pub(crate) fn restart_reveal(&mut self) {
        let generation = self.game.as_ref().map_or(0, |g| g.battle_log_generation());
        self.reveal = BattleReveal {
            generation,
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
        self.after_tick();
    }
}
