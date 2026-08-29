//! Starting, saving, loading and ending a session.

use crate::*;

impl App {
    /// Where the game's content tree was resolved to, for a frontend that
    /// needs to load an asset of its own.
    ///
    /// Exposed rather than re-derived: `crates/launcher/src/paths.rs` is the
    /// one place a runtime path is decided, and this is already the value it
    /// handed over. A frontend resolving `assets/` for itself works on the
    /// build machine and nowhere else.
    pub fn assets_dir(&self) -> &Path {
        &self.assets_dir
    }

    pub fn new(
        assets_dir: PathBuf,
        saves_dir: PathBuf,
        history_path: PathBuf,
        profile_path: PathBuf,
        arenas_dir: PathBuf,
        telemetry_path: PathBuf,
    ) -> Self {
        let (profile, profile_warning) = Profile::load(&profile_path);
        // A failed load leaves an empty ladder and an empty screen rather
        // than refusing to start — the same warn-and-carry-on contract every
        // asset db has. `Game::new` reports the per-file warnings itself.
        let (achievement_db, _) =
            AchievementDb::load_dir(&assets_dir.join("achievements")).unwrap_or_default();
        // Same contract: a missing directory leaves an empty index rather
        // than a failed start.
        let (help_db, _) = HelpDb::load_dir(&assets_dir.join("help")).unwrap_or_default();
        Self {
            mode: Mode::MainMenu,
            game: None,
            status_line: profile_warning,
            log_filter: LogFilter::default(),
            info_tab: InfoTab::default(),
            history_written: false,
            assets_dir,
            saves_dir,
            current_save_path: None,
            pending_save: None,
            history_path,
            profile_path,
            profile,
            achievement_db,
            help_db,
            help_stack: Vec::new(),
            quit: false,
            pending_structure: None,
            pending_post_structure: None,
            menu_origin: None,
            etch_return: None,
            pending_remove_structure: None,
            pending_manifest: None,
            pending_structure_manifest: None,
            pending_description: None,
            manifest_origin: ManifestOrigin::default(),
            pending_fuse_first: None,
            pending_fuse_second: None,
            fuse_name_input: String::new(),
            pending_rename: None,
            rename_input: String::new(),
            pending_routine_holder: None,
            pending_refactor_target: None,
            pending_develop_target: None,
            pending_extract_program: None,
            pending_extract_index: None,
            pending_field_routine: None,
            field_cursor: None,
            excavate_cursor: None,
            excavate_anchor: None,
            pending_battle_action: None,
            pending_party_attack: false,
            pending_special_ability: None,
            pending_inventory_item: None,
            pending_inspect: None,
            pending_swap_slot: None,
            pending_swap_target: None,
            pending_equip_program: None,
            pending_memory_program: None,
            pending_erase: None,
            erase_quantity_input: String::new(),
            basket_rows: Vec::new(),
            basket_amounts: Vec::new(),
            caravan_amounts: Vec::new(),
            basket_room: None,
            pending_craft: None,
            craft_quantity_input: String::new(),
            careful_craft: false,
            pending_order: None,
            order_quantity_input: String::new(),
            standing_order: false,
            order_priority: OrderPriority::default(),
            pending_trade_structure: None,
            pending_trade_choice: None,
            trade_origin: TradeOrigin::Trader,
            pending_trade_program: None,
            trade_quantity_input: String::new(),
            zoom: 2,
            stack_zoom: STACK_MAP_MIN_ZOOM,
            log_expanded: false,
            menu_selected: 0,
            last_autosave_tick: 0,
            pending_sounds: Vec::new(),
            reveal: BattleReveal::default(),
            status_age: 0.0,
            last_realtime_tick: Instant::now(),
            arenas_dir,
            arena: None,
            pending_arena_pick: None,
            arena_save_input: String::new(),
            arena_enabled: crate::app::arena::dev_arena_enabled(),
            telemetry_enabled: crate::app::dev_console::dev_flag("FERAL_DEV_LOG"),
            telemetry_path,
            dev_console: crate::app::dev_console::dev_console_enabled(),
            dev_templates: None,
        }
    }

    /// Drains every `SoundEvent` queued since the last call — a frontend
    /// with audio calls this once per frame and plays whatever comes back;
    /// one without can just drop the result.
    pub fn take_sounds(&mut self) -> Vec<SoundEvent> {
        std::mem::take(&mut self.pending_sounds)
    }

    /// Every `*.bin` file in the saves directory, newest first. Missing
    /// directory reads as no saves rather than an error — nothing to show
    /// on a first run before anything's ever been saved.
    pub fn list_saves(&self) -> Vec<SaveEntry> {
        let Ok(entries) = std::fs::read_dir(&self.saves_dir) else {
            return Vec::new();
        };
        let mut saves: Vec<(std::time::SystemTime, SaveEntry)> = entries
            .filter_map(|e| e.ok())
            .filter(|e| e.path().extension().is_some_and(|ext| ext == "bin"))
            .map(|e| {
                let path = e.path();
                let modified = e
                    .metadata()
                    .and_then(|m| m.modified())
                    .unwrap_or(std::time::UNIX_EPOCH);
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                let summary = feral_processes_engine::save::load_from_file(&path)
                    .ok()
                    .map(|data| {
                        format!(
                            "Lv{} · Zone {} · {:?} · tick {}",
                            data.player.level, data.zone, data.difficulty, data.tick
                        )
                    });
                (
                    modified,
                    SaveEntry {
                        path,
                        name,
                        summary,
                    },
                )
            })
            .collect();
        saves.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
        saves.into_iter().map(|(_, entry)| entry).collect()
    }

    /// A fresh, filesystem-safe save filename for a just-started game —
    /// unique enough for one-per-second play sessions, which is the only
    /// case that matters here.
    fn new_save_path(&self) -> PathBuf {
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs())
            .unwrap_or(0);
        self.saves_dir.join(format!("save_{ts}.bin"))
    }

    pub(crate) fn start_new_game(&mut self, difficulty: DifficultyMode) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(1);
        match Game::new(seed, difficulty, &self.assets_dir) {
            Ok(mut game) => {
                // Re-read rather than trusting the copy loaded at startup:
                // an earlier run this session will have written to it.
                let (profile, warning) = Profile::load(&self.profile_path);
                self.profile = profile;
                game.install_profile(self.profile.clone());
                // The one place a profile is ever paid out. `load_game`
                // deliberately does not do this — see the comment there.
                game.grant_profile_rewards();
                self.last_autosave_tick = game.current_tick();
                self.install_game(game);
                self.current_save_path = Some(self.new_save_path());
                self.history_written = false;
                self.status_line = warning;
                self.mode = Mode::Playing;
                // Save immediately so the new slot shows up in the load
                // list (and survives a crash) even before the first
                // autosave interval elapses.
                self.save_game();
            }
            Err(e) => self.status_line = Some(format!("Failed to start game: {e}")),
        }
    }

    /// Public for the launcher's `--template` flag, which boots straight
    /// into a generated save rather than by way of the load menu. A failure
    /// leaves the app on the main menu with the reason in the status line,
    /// which is what that path wants too — a bad template should drop you
    /// into the game you could have started anyway, not kill the process.
    pub fn load_game(&mut self, path: PathBuf) {
        match Game::load(&path, &self.assets_dir) {
            Ok(mut game) => {
                let (profile, warning) = Profile::load(&self.profile_path);
                self.profile = profile;
                game.install_profile(self.profile.clone());
                // `grant_profile_rewards` is deliberately NOT called here,
                // and the omission is the whole of the never-on-load rule:
                // this save's stats and Perk Points already hold what the
                // profile paid when the run started, so paying again would
                // double them on every single reload. Installing is still
                // needed, or `achievement_system` would re-earn every rung
                // the profile already holds.
                self.last_autosave_tick = game.current_tick();
                self.install_game(game);
                self.current_save_path = Some(path);
                self.history_written = false;
                self.status_line = warning;
                self.mode = Mode::Playing;
            }
            Err(e) => self.status_line = Some(format!("Failed to load game: {e}")),
        }
    }

    /// Writes the run to its slot, reporting whether it landed. The bool is
    /// there for `Mode::QuitRunConfirm`'s save-and-quit, which must not throw
    /// the run away on the strength of a save that failed.
    pub(crate) fn save_game(&mut self) -> bool {
        let Some(path) = &self.current_save_path else {
            return false;
        };
        let Some(game) = &mut self.game else {
            return false;
        };
        match game.save(path) {
            Ok(()) => {
                self.status_line = Some("Game saved.".to_string());
                true
            }
            Err(e) => {
                self.status_line = Some(format!("Save failed: {e}"));
                false
            }
        }
    }

    /// Drops the run and returns to the main menu. The one path out of
    /// `Mode::Playing` that discards state, so it is only ever reached
    /// through `Mode::QuitRunConfirm`.
    fn leave_run(&mut self) {
        self.game = None;
        self.status_line = None;
        self.mode = Mode::MainMenu;
    }

    pub(crate) fn handle_quit_run_confirm_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let options = ['s', 'q', 'n'];
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        let picked = idx.map(|i| options[i]);
        // A failed save holds the player here with the error still on screen
        // rather than leaving anyway. Quitting after being asked to save
        // first, and dropping the run regardless, is the one outcome this
        // screen exists to prevent.
        if picked == Some('s') && !self.save_game() {
            return;
        }
        match picked {
            Some('s') | Some('q') => self.leave_run(),
            Some('n') => self.mode = Mode::Playing,
            _ => {}
        }
    }

    pub(crate) fn handle_quit_app_confirm_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let options = ['y', 'n'];
        let idx = self
            .selected_index(key, options.len())
            .or_else(|| match key {
                GameKey::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('y') => self.quit = true,
            Some('n') => self.mode = Mode::MainMenu,
            _ => {}
        }
    }

    /// Silently saves to the same slot `s` does, once at least
    /// `AUTOSAVE_INTERVAL_TICKS` game ticks have passed since the last one —
    /// checked after every keypress so it fires no matter which action
    /// (movement, rest, a cronjob cycle, ...) advanced the clock. Doesn't
    /// touch `status_line` on success so it doesn't cover up a more useful
    /// message from whatever the player just did; a failure does surface,
    /// since silently failing to protect their progress would be worse.
    pub(crate) fn maybe_autosave(&mut self) {
        let Some(path) = self.current_save_path.clone() else {
            return;
        };
        let Some(game) = &mut self.game else { return };
        let current = game.current_tick();
        if current.saturating_sub(self.last_autosave_tick) < AUTOSAVE_INTERVAL_TICKS {
            return;
        }
        self.last_autosave_tick = current;
        if let Err(e) = game.save(&path) {
            self.status_line = Some(format!("Autosave failed: {e}"));
        }
    }

    /// The cross-run profile as last read from or written to disk. The
    /// achievements screen reads this rather than the running `Game`'s copy,
    /// because it is reachable from the main menu where there is no run.
    pub fn profile(&self) -> &Profile {
        &self.profile
    }

    /// The achievements screen's rows — every authored rung, earned or not.
    ///
    /// The one source of both the row count this scrolls against and the rows
    /// gui draws, per the read-only-screen rule: a renderer that rebuilt the
    /// list could scroll to a row that isn't drawn.
    pub fn achievement_rows(&self) -> Vec<AchievementRow> {
        feral_processes_engine::achievements::report(&self.achievement_db, &self.profile)
    }

    /// Everything that has to happen after the world may have ticked, in one
    /// place so a third tick site cannot pick up one half and miss the other.
    ///
    /// **An arena session returns before any of it.** Both callees write to
    /// disk, and an arena fight is not a run: an autosave would need a slot
    /// this session deliberately has none of, and a rung earned here would
    /// land in the real `profile.ron` and then be paid out to every future
    /// new game by `grant_profile_rewards`. One guard covers both precisely
    /// because this function is the one place they happen.
    pub(crate) fn after_tick(&mut self) {
        // **Deliberately above the guard, and the exception is the point.**
        // The rule below exists so a tester's fight cannot corrupt a save or
        // pay a real profile reward; a dev-only file under `dev-logs/` does
        // neither, and the arena is the single place a recorded fight is
        // most wanted. `an_arena_fight_still_writes_telemetry` is what stops
        // this being folded back inside for tidiness.
        self.flush_battle_telemetry();
        if self.in_arena() {
            return;
        }
        self.flush_profile_writes();
        self.maybe_autosave();
    }

    /// The one door a `Game` becomes the live run through — a new game, a
    /// load, or an arena staging.
    ///
    /// It exists so telemetry is armed *before* the game can tick. Arming
    /// inside `flush_battle_telemetry` instead would leave the first
    /// `idle_tick` of a session — which can open a nest fight on its own —
    /// unrecorded, and a fourth install site added later would silently
    /// collect nothing at all.
    pub(crate) fn install_game(&mut self, mut game: Game) {
        if self.telemetry_enabled {
            game.enable_battle_telemetry();
        }
        self.game = Some(game);
    }

    /// Appends everything the engine recorded since the last tick, one JSON
    /// object per line.
    ///
    /// Every tick rather than at battle end: appending is cheap, and a
    /// crash mid-session should not cost the fight that caused it.
    ///
    /// A failed write reports on the status line and the run carries on —
    /// the contract `flush_profile_writes` keeps, and for the same reason: a
    /// dev log must never take a run down with it.
    fn flush_battle_telemetry(&mut self) {
        if !self.telemetry_enabled {
            return;
        }
        let Some(game) = &mut self.game else { return };
        let records = game.take_battle_telemetry();
        if records.is_empty() {
            return;
        }
        if let Err(e) = crate::app::telemetry::append_records(&self.telemetry_path, &records) {
            self.status_line = Some(format!("Could not write telemetry: {e}"));
        }
    }

    /// Writes `profile.ron` if this tick earned anything.
    ///
    /// Immediately, not at run end: a permadeath run that ends badly must not
    /// lose what it proved. A failed write costs the profile update and
    /// nothing else — it must never take the run down with it.
    fn flush_profile_writes(&mut self) {
        let Some(game) = &mut self.game else { return };
        if !game.take_pending_profile_writes() {
            return;
        }
        self.profile = game.profile().clone();
        if let Err(e) = self.profile.save(&self.profile_path) {
            self.status_line = Some(format!("Could not write profile: {e}"));
        }
    }

    /// Guarded separately from `after_tick` rather than folded into it,
    /// because it is not a post-tick concern and is not called from there:
    /// its three callers are the battle tail, the trade screen and the map,
    /// each asking whether the run has just ended.
    ///
    /// An arena fight can reach it — a `Save` player source carries its own
    /// difficulty in, so a lost fight against a Permadeath save *is* a
    /// game over — and what must not follow is a `run_history.log` entry
    /// for a fight that was never a run.
    pub(crate) fn check_game_over(&mut self) {
        let over = self
            .game
            .as_ref()
            .map(|g| g.is_game_over().is_some())
            .unwrap_or(false);
        if !over {
            return;
        }
        if self.in_arena() {
            self.finish_arena_fight();
            return;
        }
        if !self.history_written {
            if let Some(game) = &mut self.game {
                let _ = game.write_history(&self.history_path);
            }
            self.history_written = true;
        }
        // The other exit from `Mode::BattleResult`: a run that ends on the
        // losing round never gets a key press on the results screen, so
        // without this the fight's blow-by-blow would sit in the log the
        // history screen reads.
        if self.mode == Mode::BattleResult {
            self.leave_battle_result();
        }
        self.mode = Mode::GameOver;
    }
}
