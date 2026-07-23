//! Shared, renderer-agnostic game-flow state machine.
//!
//! This crate owns `App`/`Mode` — what pressing a key does in a given
//! screen, save/load orchestration, autosave pacing — but knows nothing
//! about terminals or windows. Frontends (currently `feral-processes-tui`
//! and `feral-processes-gui`) translate their own input events into
//! `GameKey` and call `App::handle_key`, then read `App`'s public fields to
//! render however they like.

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

#[cfg(test)]
use feral_processes_engine::items::ids;
use feral_processes_engine::items::{
    EquipmentSlot, ITEM_FUSION_BONUS_PER_TIER, ITEM_FUSION_COST, ItemId,
};
use feral_processes_engine::{DifficultyMode, Entity, Game};

/// Radius (in tiles) scanned for the build/work menus, independent of the
/// visible viewport size.
pub const MENU_SCAN_RADIUS: i32 = 40;

/// How many menu rows the digits `1`-`9` can address before `menu_shortcut`
/// switches to letters.
const DIGIT_ROWS: usize = 9;

/// The key that picks menu row `index` (0-based), and the label a renderer
/// must print for it: `1`-`9` for the first nine rows, then `a`, `b`, `c`
/// and so on. Several menus run past nine rows — a dozen research nodes,
/// ten deployable structures, an inventory of any size — and a single digit
/// can't address those, so they'd otherwise be reachable only by Up/Down +
/// Enter. Menus that bind letters to their own actions all fit inside nine
/// rows, so the two never overlap.
///
/// Rows past the 35th run out of letters and return `'-'`, which no key
/// produces — they're reachable by Up/Down + Enter only, and the label says
/// so rather than advertising a key that does nothing.
pub fn menu_shortcut(index: usize) -> char {
    if index < DIGIT_ROWS {
        return char::from_digit(index as u32 + 1, 10).expect("a row under 9 is always a digit");
    }
    match u8::try_from(b'a' as usize + index - DIGIT_ROWS) {
        Ok(c @ b'a'..=b'z') => c as char,
        _ => '-',
    }
}

/// The actions offered for `item` on the `Mode::InventoryItemAction` page,
/// in display order, as (shortcut key, label) pairs. Both renderers draw
/// from this and `App::handle_inventory_item_action_key` dispatches from
/// it, so the rows shown and the keys accepted can't drift apart.
///
/// Fuse is listed for any equippable item regardless of how many copies are
/// held: hiding it below `ITEM_FUSION_COST` meant a player holding the
/// usual single copy of a piece of gear never learned the action existed.
/// `Game::fuse_item` refuses with a count when the stack is too small.
pub fn inventory_item_actions(game: &Game, item: &ItemId) -> Vec<(char, String)> {
    let mut actions = Vec::new();
    if game.is_equippable(item) {
        actions.push(('e', "[E]quip".to_string()));
        actions.push((
            'u',
            format!(
                "[U] Fuse ({ITEM_FUSION_COST} -> +{}% bonus)",
                (ITEM_FUSION_BONUS_PER_TIER * 100.0).round() as i32
            ),
        ));
    }
    if game.is_consumable(item) {
        actions.push(('c', "[C]onsume".to_string()));
    }
    actions.push(('x', "[X] Erase".to_string()));
    actions
}

/// How many game ticks (see `Game::current_tick`) pass between autosaves —
/// paced against game time rather than wall-clock time, so it's the same
/// whether the player is acting quickly or sitting on a menu.
const AUTOSAVE_INTERVAL_TICKS: u64 = 50;

/// Wall-clock spacing between idle ticks (see `App::update_realtime`) —
/// the world keeps moving once a second even while the player just sits on
/// `Mode::Playing` and touches nothing.
const REALTIME_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// A frontend-agnostic input event. Every renderer crate maps its own input
/// system's keys onto this small vocabulary before calling `App::handle_key`
/// — this is the seam that keeps `App` free of any UI-toolkit dependency.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GameKey {
    Up,
    Down,
    Left,
    Right,
    Char(char),
    Enter,
    Esc,
    Backspace,
}

/// A cue for a frontend to play a sound effect for — pushed by `App` as it
/// handles keys, drained by whichever frontend cares (`App::take_sounds`).
/// `App` itself never touches an audio device; this is just the same
/// renderer-agnostic seam `GameKey` is, in the other direction. A frontend
/// with no audio (the TUI) is free to just drop what it drains.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SoundEvent {
    /// A movement key actually moved the player (or was blocked/no-op —
    /// the engine gives no feedback to distinguish the two, so this fires
    /// on every movement key press that doesn't start a battle instead).
    Step,
    /// A movement key walked the player into a wild creature.
    BattleStart,
    /// The player or a companion took a battle action (attack, decompile
    /// attempt, or a companion command).
    Attack,
    /// The player jacked out of a battle.
    Flee,
    /// A battle ended with the wild creature gone and the player still
    /// standing.
    Victory,
    /// The run ended in `Mode::GameOver`.
    Defeat,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    MainMenu,
    DifficultyPick,
    /// Lists saves found in the saves directory (see `App::list_saves`);
    /// picking one moves to `Mode::SaveAction` to choose Load or Delete.
    LoadGame,
    /// Load-or-delete choice for the save picked from `Mode::LoadGame`.
    SaveAction,
    Playing,
    Battle,
    BattleCompanion,
    Build,
    BuildDirection,
    Craft,
    CraftQuantity,
    Cronjob,
    CronjobStructure,
    Guard,
    GuardStructure,
    /// Lists nearby structures to demolish (see `App::pending_remove_structure`).
    /// Picking the Home moves to `Mode::RemoveConfirm` instead of demolishing
    /// immediately, since it cascades; anything else is removed right away.
    Remove,
    /// Warns that demolishing the Home destroys every other base structure
    /// too, before `Game::remove_structure` is actually called.
    RemoveConfirm,
    Symlink,
    InspectDirection,
    InspectDetail,
    Inventory,
    InventoryItemAction,
    /// Second page of the erase flow: asks how many units of
    /// `pending_erase` to destroy before calling `Game::erase_item`. A
    /// hard inventory cap makes partial erasure the common case — dumping a
    /// whole stack to free two units of room is not a real option.
    EraseQuantity,
    Companion,
    Fuse,
    FuseSecond,
    /// Typing a name (`App::fuse_name_input`) for the program that'll
    /// result from fusing `pending_fuse_first`/`pending_fuse_second` —
    /// blank keeps the default species name. Reached after picking both
    /// programs in `Mode::Fuse`/`Mode::FuseSecond`; Enter actually runs the
    /// fusion.
    FuseName,
    Trade,
    TradeAction,
    TradeQuantity,
    Perks,
    /// The research tree (see `Game::research_nodes`). Stays open after each
    /// unlock so several nodes can be taken in one visit.
    Research,
    Help,
    GameOver,
}

/// A line item picked in `Mode::TradeAction`, awaiting a quantity from
/// `Mode::TradeQuantity` before `Game::sell_item`/`Game::buy_item` is
/// actually called.
#[derive(Clone)]
pub enum TradeChoice {
    Sell(ItemId),
    Buy(ItemId),
}

pub const MIN_ZOOM: u16 = 1;
pub const MAX_ZOOM: u16 = 4;

pub struct App {
    pub mode: Mode,
    pub game: Option<Game>,
    pub status_line: Option<String>,
    history_written: bool,
    assets_dir: PathBuf,
    /// Directory saves are read from/written to — see `App::list_saves`.
    saves_dir: PathBuf,
    /// Which file the active session's manual/auto-saves go to. `None`
    /// until a game is started (which immediately saves to claim a new
    /// slot) or loaded (which points this at the picked file).
    current_save_path: Option<PathBuf>,
    /// The save picked from `Mode::LoadGame`, awaiting a Load/Delete choice
    /// from `Mode::SaveAction`.
    pub pending_save: Option<PathBuf>,
    history_path: PathBuf,
    pub quit: bool,
    pending_structure: Option<String>,
    pending_worker: Option<Entity>,
    /// The structure picked in `Mode::Remove`, awaiting confirmation from
    /// `Mode::RemoveConfirm` if it's the Home (see `Game::remove_structure`).
    pending_remove_structure: Option<Entity>,
    pub pending_inspect: Option<Entity>,
    /// The first program picked in `Mode::Fuse`, awaiting a second from
    /// `Mode::FuseSecond` before `Game::fuse_companions` is actually called.
    pub pending_fuse_first: Option<Entity>,
    /// The second program picked in `Mode::FuseSecond`, awaiting a name
    /// from `Mode::FuseName` before `Game::fuse_companions` is actually
    /// called.
    pub pending_fuse_second: Option<Entity>,
    /// Characters typed so far on the fuse-naming page (see `Mode::FuseName`).
    pub fuse_name_input: String,
    pub pending_inventory_item: Option<ItemId>,
    /// The inventory item picked for erasure, awaiting a quantity from
    /// `Mode::EraseQuantity`.
    pub pending_erase: Option<ItemId>,
    /// Digits typed so far on the erase-quantity page.
    pub erase_quantity_input: String,
    /// The recipe result picked in `Mode::Craft`, awaiting a quantity from
    /// `Mode::CraftQuantity` before `Game::craft` is actually called.
    pub pending_craft: Option<ItemId>,
    /// Digits typed so far on the craft-quantity page.
    pub craft_quantity_input: String,
    /// The trading post picked in `Mode::Trade`, awaiting a line-item pick
    /// from `Mode::TradeAction`.
    pub pending_trade_structure: Option<Entity>,
    /// The sell/buy line item picked in `Mode::TradeAction`, awaiting a
    /// quantity from `Mode::TradeQuantity` before `Game::sell_item`/
    /// `Game::buy_item` is actually called.
    pub pending_trade_choice: Option<TradeChoice>,
    /// Digits typed so far on the trade-quantity page.
    pub trade_quantity_input: String,
    /// How many screen characters render each world tile along each axis.
    pub zoom: u16,
    /// Which row is highlighted on the current numbered/lettered menu, for
    /// Up/Down-plus-Enter navigation (see `App::selected_index`) — on top
    /// of, not instead of, typing a row's own number/letter directly.
    /// Reset to 0 every time a menu mode is entered.
    pub menu_selected: usize,
    /// The game tick (see `Game::current_tick`) as of the last autosave —
    /// reset to the current tick whenever a game starts or loads, so a
    /// resumed session doesn't immediately autosave on its very first move.
    last_autosave_tick: u64,
    /// Sound cues queued up by the most recent `handle_key` calls, awaiting
    /// `take_sounds` — see `SoundEvent`.
    pending_sounds: Vec<SoundEvent>,
    /// Wall-clock time of the last idle tick (see `App::update_realtime`) —
    /// reset whenever ticking is paused (any mode but `Playing`) so resuming
    /// play doesn't immediately fire a burst of catch-up ticks.
    last_realtime_tick: Instant,
}

/// One entry in the `Mode::LoadGame` list — a save file found in the saves
/// directory, with a short summary peeked from it (if it's still readable
/// under the current `save::SAVE_FORMAT_VERSION`).
pub struct SaveEntry {
    pub path: PathBuf,
    /// The filename without its extension, shown as the save's name.
    pub name: String,
    /// `None` if the file couldn't be read at all (wrong version, corrupt,
    /// ...) — still listed (so it can be deleted), just flagged as such.
    pub summary: Option<String>,
}

impl App {
    pub fn new(assets_dir: PathBuf, saves_dir: PathBuf, history_path: PathBuf) -> Self {
        Self {
            mode: Mode::MainMenu,
            game: None,
            status_line: None,
            history_written: false,
            assets_dir,
            saves_dir,
            current_save_path: None,
            pending_save: None,
            history_path,
            quit: false,
            pending_structure: None,
            pending_worker: None,
            pending_remove_structure: None,
            pending_inspect: None,
            pending_fuse_first: None,
            pending_fuse_second: None,
            fuse_name_input: String::new(),
            pending_inventory_item: None,
            pending_erase: None,
            erase_quantity_input: String::new(),
            pending_craft: None,
            craft_quantity_input: String::new(),
            pending_trade_structure: None,
            pending_trade_choice: None,
            trade_quantity_input: String::new(),
            zoom: 2,
            menu_selected: 0,
            last_autosave_tick: 0,
            pending_sounds: Vec::new(),
            last_realtime_tick: Instant::now(),
        }
    }

    /// Drains every `SoundEvent` queued since the last call — a frontend
    /// with audio calls this once per frame and plays whatever comes back;
    /// one with none (the TUI) can just drop the result.
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

    fn start_new_game(&mut self, difficulty: DifficultyMode) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(1);
        match Game::new(seed, difficulty, &self.assets_dir) {
            Ok(game) => {
                self.last_autosave_tick = game.current_tick();
                self.game = Some(game);
                self.current_save_path = Some(self.new_save_path());
                self.history_written = false;
                self.status_line = None;
                self.mode = Mode::Playing;
                // Save immediately so the new slot shows up in the load
                // list (and survives a crash) even before the first
                // autosave interval elapses.
                self.save_game();
            }
            Err(e) => self.status_line = Some(format!("Failed to start game: {e}")),
        }
    }

    fn load_game(&mut self, path: PathBuf) {
        match Game::load(&path, &self.assets_dir) {
            Ok(game) => {
                self.last_autosave_tick = game.current_tick();
                self.game = Some(game);
                self.current_save_path = Some(path);
                self.history_written = false;
                self.status_line = None;
                self.mode = Mode::Playing;
            }
            Err(e) => self.status_line = Some(format!("Failed to load game: {e}")),
        }
    }

    fn save_game(&mut self) {
        let Some(path) = &self.current_save_path else {
            return;
        };
        if let Some(game) = &mut self.game {
            match game.save(path) {
                Ok(()) => self.status_line = Some("Game saved.".to_string()),
                Err(e) => self.status_line = Some(format!("Save failed: {e}")),
            }
        }
    }

    /// Silently saves to the same slot `s` does, once at least
    /// `AUTOSAVE_INTERVAL_TICKS` game ticks have passed since the last one —
    /// checked after every keypress so it fires no matter which action
    /// (movement, rest, a cronjob cycle, ...) advanced the clock. Doesn't
    /// touch `status_line` on success so it doesn't cover up a more useful
    /// message from whatever the player just did; a failure does surface,
    /// since silently failing to protect their progress would be worse.
    fn maybe_autosave(&mut self) {
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

    fn check_game_over(&mut self) {
        let over = self
            .game
            .as_ref()
            .map(|g| g.is_game_over().is_some())
            .unwrap_or(false);
        if !over {
            return;
        }
        if !self.history_written {
            if let Some(game) = &mut self.game {
                let _ = game.write_history(&self.history_path);
            }
            self.history_written = true;
        }
        self.mode = Mode::GameOver;
    }

    /// Shared Up/Down/Enter handling layered on top of every menu's direct
    /// row shortcuts — this doesn't replace them, it's just another way to
    /// pick the same row. `len` is how many selectable rows the menu
    /// currently has. A typed shortcut (see `menu_shortcut`) resolves
    /// immediately to that 0-based index; Up/Down instead move
    /// `menu_selected` (wrapping) and return `None`; Enter resolves to
    /// whatever `menu_selected` currently highlights. Any other key, or an
    /// empty menu, returns `None`.
    fn selected_index(&mut self, key: GameKey, len: usize) -> Option<usize> {
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
        let mode_before = self.mode;
        match self.mode {
            Mode::MainMenu => self.handle_main_menu_key(key),
            Mode::LoadGame => self.handle_load_game_key(key),
            Mode::SaveAction => self.handle_save_action_key(key),
            Mode::DifficultyPick => self.handle_difficulty_key(key),
            Mode::Playing => self.handle_playing_key(key),
            Mode::Battle => self.handle_battle_key(key),
            Mode::BattleCompanion => self.handle_battle_companion_key(key),
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

    fn handle_main_menu_key(&mut self, key: GameKey) {
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

    fn handle_load_game_key(&mut self, key: GameKey) {
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

    fn handle_save_action_key(&mut self, key: GameKey) {
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

    fn handle_difficulty_key(&mut self, key: GameKey) {
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

    fn handle_playing_key(&mut self, key: GameKey) {
        match key {
            GameKey::Char('b') => {
                self.mode = Mode::Build;
                return;
            }
            GameKey::Char('c') => {
                self.mode = Mode::Craft;
                return;
            }
            GameKey::Char('w') => {
                self.mode = Mode::Cronjob;
                return;
            }
            GameKey::Char('G') => {
                self.mode = Mode::Guard;
                return;
            }
            GameKey::Char('R') => {
                self.mode = Mode::Remove;
                return;
            }
            GameKey::Char('u') => {
                self.mode = Mode::Symlink;
                return;
            }
            GameKey::Char('i') => {
                self.mode = Mode::InspectDirection;
                return;
            }
            GameKey::Char('v') => {
                self.mode = Mode::Inventory;
                return;
            }
            GameKey::Char('p') => {
                self.mode = Mode::Companion;
                return;
            }
            GameKey::Char('f') => {
                self.mode = Mode::Fuse;
                return;
            }
            GameKey::Char('t') => {
                self.mode = Mode::Trade;
                return;
            }
            GameKey::Char('x') => {
                self.mode = Mode::Perks;
                return;
            }
            GameKey::Char('T') => {
                self.mode = Mode::Research;
                return;
            }
            GameKey::Char('s') => {
                self.save_game();
                return;
            }
            GameKey::Char('q') => {
                self.game = None;
                self.status_line = None;
                self.mode = Mode::MainMenu;
                return;
            }
            GameKey::Char('?') => {
                self.mode = Mode::Help;
                return;
            }
            GameKey::Char('+') | GameKey::Char('=') => {
                self.zoom = (self.zoom + 1).min(MAX_ZOOM);
                return;
            }
            GameKey::Char('-') | GameKey::Char('_') => {
                self.zoom = self.zoom.saturating_sub(1).max(MIN_ZOOM);
                return;
            }
            _ => {}
        }

        let is_move_key = matches!(
            key,
            GameKey::Up
                | GameKey::Down
                | GameKey::Left
                | GameKey::Right
                | GameKey::Char('k')
                | GameKey::Char('j')
                | GameKey::Char('h')
                | GameKey::Char('l')
        );
        let acted = {
            let Some(game) = &mut self.game else { return };
            match key {
                GameKey::Up | GameKey::Char('k') => {
                    game.move_player(0, -1);
                    true
                }
                GameKey::Down | GameKey::Char('j') => {
                    game.move_player(0, 1);
                    true
                }
                GameKey::Left | GameKey::Char('h') => {
                    game.move_player(-1, 0);
                    true
                }
                GameKey::Right | GameKey::Char('l') => {
                    game.move_player(1, 0);
                    true
                }
                GameKey::Char('.') => {
                    game.wait();
                    true
                }
                GameKey::Char('e') => {
                    game.use_power_source();
                    true
                }
                GameKey::Char('r') => {
                    game.rest();
                    true
                }
                GameKey::Char('g') => {
                    game.forage();
                    true
                }
                _ => false,
            }
        };
        if !acted {
            return;
        }
        self.status_line = None;
        let entered_battle = self
            .game
            .as_ref()
            .map(|g| g.has_active_battle())
            .unwrap_or(false);
        if entered_battle {
            self.mode = Mode::Battle;
        }
        if is_move_key {
            self.pending_sounds.push(if entered_battle {
                SoundEvent::BattleStart
            } else {
                SoundEvent::Step
            });
        }
        self.check_game_over();
        if self.mode == Mode::GameOver {
            self.pending_sounds.push(SoundEvent::Defeat);
        }
    }

    /// Shared tail of every battle-turn key handler: queues the sound for
    /// the action just taken, then — once `check_game_over` has had a
    /// chance to move `self.mode` to `Mode::GameOver` — queues whichever
    /// outcome sound actually applies. A `Flee` action never gets a
    /// `Victory` sound layered on top of it even though it also ends the
    /// battle, since jacking out isn't a win.
    fn push_battle_outcome_sounds(&mut self, action_sound: SoundEvent, still_active: bool) {
        self.pending_sounds.push(action_sound);
        self.check_game_over();
        if self.mode == Mode::GameOver {
            self.pending_sounds.push(SoundEvent::Defeat);
        } else if !still_active && action_sound != SoundEvent::Flee {
            self.pending_sounds.push(SoundEvent::Victory);
        }
    }

    fn handle_battle_key(&mut self, key: GameKey) {
        // Both renderers label these `[A]ttack`/`[D]ecompile`/`[C]ommand
        // companion`/`[J]ack Out`, so a shifted keypress is the one the
        // prompt actually asks for and has to resolve the same as an
        // unshifted one.
        let key = match key {
            GameKey::Char(c) => GameKey::Char(c.to_ascii_lowercase()),
            other => other,
        };
        if key == GameKey::Char('c') {
            let Some(game) = &mut self.game else { return };
            let party = game.player_status().companions;
            match party.len() {
                0 => self.status_line = Some("You have no active companion.".to_string()),
                1 => {
                    let entity = party[0].entity;
                    game.battle_command_companion(entity);
                    let still_active = game.has_active_battle();
                    if !still_active {
                        self.mode = Mode::Playing;
                    }
                    self.push_battle_outcome_sounds(SoundEvent::Attack, still_active);
                }
                _ => self.mode = Mode::BattleCompanion,
            }
            return;
        }

        let (still_active, action_sound) = {
            let Some(game) = &mut self.game else { return };
            let sound = match key {
                GameKey::Char('a') => {
                    game.battle_attack();
                    SoundEvent::Attack
                }
                GameKey::Char('d') => {
                    game.battle_decompile();
                    SoundEvent::Attack
                }
                GameKey::Char('j') => {
                    game.battle_flee();
                    SoundEvent::Flee
                }
                _ => return,
            };
            (game.has_active_battle(), sound)
        };
        if !still_active {
            self.mode = Mode::Playing;
        }
        self.push_battle_outcome_sounds(action_sound, still_active);
    }

    /// Picks which party member acts this round when there's more than one
    /// active companion (a single companion is commanded directly from
    /// `handle_battle_key` with no extra step).
    fn handle_battle_companion_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Battle;
            return;
        }
        let Some(game) = &self.game else { return };
        let party = game.player_status().companions;
        let Some(idx) = self.selected_index(key, party.len()) else {
            return;
        };
        let entity = party[idx].entity;
        let Some(game) = &mut self.game else { return };
        game.battle_command_companion(entity);
        let still_active = game.has_active_battle();
        self.mode = if still_active {
            Mode::Battle
        } else {
            Mode::Playing
        };
        self.push_battle_outcome_sounds(SoundEvent::Attack, still_active);
    }

    fn handle_build_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &self.game else { return };
        let defs = game.buildable_structure_defs();
        if let Some(idx) = self.selected_index(key, defs.len()) {
            self.pending_structure = Some(defs[idx].id.clone());
            self.mode = Mode::BuildDirection;
        }
    }

    fn handle_craft_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let recipes = game.craft_recipes();
        if let Some(idx) = self.selected_index(key, recipes.len()) {
            self.pending_craft = Some(recipes[idx].result.clone());
            self.craft_quantity_input.clear();
            self.mode = Mode::CraftQuantity;
        }
    }

    /// Second page of the compile flow: asks how many units of
    /// `pending_craft` to make before actually calling `Game::craft`. `[F]`
    /// is a shortcut for 5 at once, `[M]` for the most affordable right now
    /// (see `Game::max_craftable`) — both bypass typing digits and Enter.
    fn handle_craft_quantity_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_craft = None;
                self.craft_quantity_input.clear();
                self.mode = Mode::Craft;
            }
            GameKey::Backspace => {
                self.craft_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.craft_quantity_input.len() < 4 => {
                self.craft_quantity_input.push(c);
            }
            GameKey::Char('f') | GameKey::Char('F') => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                self.craft_quantity_input.clear();
                self.commit_craft(result, 5);
            }
            GameKey::Char('m') | GameKey::Char('M') => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                self.craft_quantity_input.clear();
                let Some(game) = &self.game else {
                    self.mode = Mode::Playing;
                    return;
                };
                let max = game.max_craftable(&result);
                if max == 0 {
                    let name = game.item_name(&result).to_string();
                    self.status_line = Some(format!("Not enough resources to compile any {name}."));
                    self.mode = Mode::Playing;
                    return;
                }
                self.commit_craft(result, max);
            }
            GameKey::Enter => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                let quantity: u32 = if self.craft_quantity_input.is_empty() {
                    1
                } else {
                    self.craft_quantity_input.parse().unwrap_or(0)
                };
                self.craft_quantity_input.clear();
                self.commit_craft(result, quantity);
            }
            _ => {}
        }
    }

    /// Calls `Game::craft(result, quantity)` and returns to normal play,
    /// shared by the craft-quantity page's Enter, `[F]` (5), and `[M]` (max)
    /// paths. A quantity of 0 (e.g. Enter on an explicitly typed "0") is a
    /// silent no-op rather than a round-trip to the engine for an error.
    fn commit_craft(&mut self, result: ItemId, quantity: u32) {
        if quantity == 0 {
            self.mode = Mode::Playing;
            return;
        }
        if let Some(game) = &mut self.game {
            match game.craft(&result, quantity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
        self.mode = Mode::Playing;
    }

    fn handle_build_direction_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_structure = None;
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
        let Some(id) = self.pending_structure.take() else {
            self.mode = Mode::Playing;
            return;
        };
        if let Some(game) = &mut self.game {
            match game.place_structure(&id, dx, dy) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
        self.mode = Mode::Playing;
    }

    fn handle_cronjob_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let workers: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed)
            .collect();
        if let Some(idx) = self.selected_index(key, workers.len()) {
            self.pending_worker = Some(workers[idx].entity);
            self.mode = Mode::CronjobStructure;
        }
    }

    fn handle_cronjob_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_worker = None;
            self.mode = Mode::Playing;
            return;
        }
        let Some(worker) = self.pending_worker else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.can_work)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let Some(game) = &mut self.game else { return };
            match game.assign_cronjob(worker, structures[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.pending_worker = None;
            self.mode = Mode::Playing;
        }
    }

    fn handle_guard_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let workers: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed)
            .collect();
        if let Some(idx) = self.selected_index(key, workers.len()) {
            self.pending_worker = Some(workers[idx].entity);
            self.mode = Mode::GuardStructure;
        }
    }

    fn handle_guard_structure_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_worker = None;
            self.mode = Mode::Playing;
            return;
        }
        let Some(worker) = self.pending_worker else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_structure)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let Some(game) = &mut self.game else { return };
            match game.assign_guard(worker, structures[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.pending_worker = None;
            self.mode = Mode::Playing;
        }
    }

    fn handle_remove_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_structure)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            let picked_entity = structures[idx].entity;
            let picked_is_home = structures[idx].is_home;
            if picked_is_home {
                self.pending_remove_structure = Some(picked_entity);
                self.mode = Mode::RemoveConfirm;
                return;
            }
            let Some(game) = &mut self.game else { return };
            match game.remove_structure(picked_entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }

    fn handle_remove_confirm_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_remove_structure = None;
            self.mode = Mode::Playing;
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
            Some('y') => {
                if let Some(structure) = self.pending_remove_structure.take() {
                    let Some(game) = &mut self.game else { return };
                    match game.remove_structure(structure) {
                        Ok(()) => self.status_line = None,
                        Err(e) => self.status_line = Some(e),
                    }
                }
                self.mode = Mode::Playing;
            }
            Some('n') => {
                self.pending_remove_structure = None;
                self.mode = Mode::Playing;
            }
            _ => {}
        }
    }

    /// Lists every deployed symlink-capable structure (e.g. Home) anywhere
    /// on the map — not scan-radius-limited like the build/cronjob
    /// menus — and teleports the player to the picked one.
    fn handle_symlink_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let targets = game.symlink_targets();
        if let Some(idx) = self.selected_index(key, targets.len()) {
            let Some(game) = &mut self.game else { return };
            match game.use_symlink(targets[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.mode = Mode::Playing;
        }
    }

    /// Lists every tamed program you own, wherever it is — pressing a party
    /// member's number stands it down, pressing any other program's number
    /// adds it to the party (up to `MAX_PARTY_SIZE` at once).
    fn handle_companion_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let candidates = game.owned_pets();
        if let Some(idx) = self.selected_index(key, candidates.len()) {
            let candidate = &candidates[idx];
            let Some(game) = &mut self.game else { return };
            if candidate.is_companion {
                game.remove_companion(candidate.entity);
                self.status_line = None;
            } else {
                match game.add_companion(candidate.entity) {
                    Ok(()) => self.status_line = None,
                    Err(e) => self.status_line = Some(e),
                }
            }
        }
    }

    /// Picks the first of two tamed programs to fuse together.
    fn handle_fuse_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let candidates: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed)
            .collect();
        if let Some(idx) = self.selected_index(key, candidates.len()) {
            self.pending_fuse_first = Some(candidates[idx].entity);
            self.mode = Mode::FuseSecond;
        }
    }

    /// Picks the second program to fuse with the one from `handle_fuse_key`,
    /// then actually runs the fusion.
    fn handle_fuse_second_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_fuse_first = None;
            self.mode = Mode::Playing;
            return;
        }
        let Some(first) = self.pending_fuse_first else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let candidates: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed && e.entity != first)
            .collect();
        if let Some(idx) = self.selected_index(key, candidates.len()) {
            self.pending_fuse_second = Some(candidates[idx].entity);
            self.fuse_name_input.clear();
            self.mode = Mode::FuseName;
        }
    }

    /// Types a name (up to `feral_processes_engine::MAX_CUSTOM_NAME_LEN`
    /// characters) for the program that'll result from fusing
    /// `pending_fuse_first`/`pending_fuse_second`; Enter runs the fusion
    /// (blank keeps the default species name). Esc backs up one step to
    /// re-pick the second program, rather than aborting the whole fusion —
    /// the first pick is still good.
    fn handle_fuse_name_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_fuse_second = None;
                self.fuse_name_input.clear();
                self.mode = Mode::FuseSecond;
            }
            GameKey::Backspace => {
                self.fuse_name_input.pop();
            }
            GameKey::Char(c)
                if !c.is_control()
                    && self.fuse_name_input.chars().count()
                        < feral_processes_engine::MAX_CUSTOM_NAME_LEN =>
            {
                self.fuse_name_input.push(c);
            }
            GameKey::Enter => {
                let (Some(first), Some(second)) = (
                    self.pending_fuse_first.take(),
                    self.pending_fuse_second.take(),
                ) else {
                    self.mode = Mode::Playing;
                    return;
                };
                let name = (!self.fuse_name_input.is_empty()).then(|| self.fuse_name_input.clone());
                self.fuse_name_input.clear();
                let Some(game) = &mut self.game else { return };
                match game.fuse_companions(first, second, name) {
                    Ok(()) => self.status_line = None,
                    Err(e) => self.status_line = Some(e),
                }
                self.mode = Mode::Playing;
            }
            _ => {}
        }
    }

    /// Picks a nearby trading-post structure to open a trade session with.
    fn handle_trade_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.can_trade)
            .collect();
        if let Some(idx) = self.selected_index(key, structures.len()) {
            self.pending_trade_structure = Some(structures[idx].entity);
            self.mode = Mode::TradeAction;
        }
    }

    /// Picks a sell (from inventory) or buy (from the structure's trade
    /// list) line item — sell offers are numbered first, then buy offers.
    fn handle_trade_action_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_trade_structure = None;
            self.mode = Mode::Trade;
            return;
        }
        let Some(structure) = self.pending_trade_structure else {
            self.mode = Mode::Playing;
            return;
        };
        let Some(game) = &mut self.game else { return };
        let Some(trade) = game.trade_options(structure) else {
            self.mode = Mode::Playing;
            return;
        };
        let currency = game.currency();
        let sell_items: Vec<ItemId> = game
            .player_status()
            .inventory
            .iter()
            .map(|(item, _)| item.clone())
            .filter(|item| *item != currency)
            .collect();
        let buy_items: Vec<ItemId> = trade.buy.iter().map(|(item, _)| item.clone()).collect();
        let total = sell_items.len() + buy_items.len();
        if let Some(idx) = self.selected_index(key, total) {
            let choice = if idx < sell_items.len() {
                TradeChoice::Sell(sell_items[idx].clone())
            } else {
                TradeChoice::Buy(buy_items[idx - sell_items.len()].clone())
            };
            self.pending_trade_choice = Some(choice);
            self.trade_quantity_input.clear();
            self.mode = Mode::TradeQuantity;
        }
    }

    /// Types a quantity for the pending sell/buy line item; Enter commits it.
    fn handle_trade_quantity_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.pending_trade_choice = None;
                self.trade_quantity_input.clear();
                self.mode = Mode::TradeAction;
            }
            GameKey::Backspace => {
                self.trade_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.trade_quantity_input.len() < 4 => {
                self.trade_quantity_input.push(c);
            }
            GameKey::Enter => {
                let Some(choice) = self.pending_trade_choice.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                let Some(structure) = self.pending_trade_structure else {
                    self.mode = Mode::Playing;
                    return;
                };
                let quantity: u32 = if self.trade_quantity_input.is_empty() {
                    1
                } else {
                    self.trade_quantity_input.parse().unwrap_or(0)
                };
                self.trade_quantity_input.clear();
                if quantity == 0 {
                    self.pending_trade_structure = None;
                    self.mode = Mode::Playing;
                    return;
                }
                if let Some(game) = &mut self.game {
                    let result = match choice {
                        TradeChoice::Sell(item) => game.sell_item(structure, item, quantity),
                        TradeChoice::Buy(item) => game.buy_item(structure, item, quantity),
                    };
                    match result {
                        Ok(()) => self.status_line = None,
                        Err(e) => self.status_line = Some(e),
                    }
                }
                self.pending_trade_structure = None;
                self.mode = Mode::Playing;
            }
            _ => {}
        }
    }

    /// Picks a numbered perk to unlock; stays open so multiple can be
    /// unlocked in one visit if there are enough Perk Points.
    fn handle_perks_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let perks = feral_processes_engine::Perk::all();
        if let Some(idx) = self.selected_index(key, perks.len()) {
            let Some(game) = &mut self.game else { return };
            match game.unlock_perk(perks[idx]) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }

    /// Picks a numbered research node to unlock; stays open so several can
    /// be taken in one visit.
    fn handle_research_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        // Collecting the ids through `as_ref().map` (rather than a
        // `let Some(game) = &self.game` binding) ends the borrow here —
        // `selected_index` needs `&mut self`.
        let Some(ids) = self.game.as_ref().map(|g| {
            g.research_nodes()
                .into_iter()
                .map(|n| n.id)
                .collect::<Vec<_>>()
        }) else {
            return;
        };
        if let Some(idx) = self.selected_index(key, ids.len()) {
            let id = ids[idx].clone();
            let Some(game) = &mut self.game else { return };
            match game.unlock_research(&id) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }

    /// Picks a direction (arrows/hjkl) and inspects the first creature the
    /// engine finds stepping that way from the player, rather than picking
    /// from a numbered list of grid coordinates.
    fn handle_inspect_direction_key(&mut self, key: GameKey) {
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
                self.pending_inspect = Some(entity);
                self.status_line = None;
                self.mode = Mode::InspectDetail;
            }
            None => {
                self.status_line = Some("Nothing in that direction.".to_string());
                self.mode = Mode::Playing;
            }
        }
    }

    fn handle_inspect_detail_key(&mut self, _key: GameKey) {
        self.pending_inspect = None;
        self.mode = Mode::Playing;
    }

    /// Equipped slots are numbered 1-3 (Weapon/Armor/Module) and unequip
    /// immediately when pressed; unequipped inventory items start at 4 and
    /// open `Mode::InventoryItemAction` for the selected item.
    fn handle_inventory_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &self.game else { return };
        let inventory = game.player_status().inventory;
        let total = 3 + inventory.len();
        let Some(idx) = self.selected_index(key, total) else {
            return;
        };
        let slot = match idx {
            0 => Some(EquipmentSlot::Weapon),
            1 => Some(EquipmentSlot::Armor),
            2 => Some(EquipmentSlot::Module),
            _ => None,
        };
        if let Some(slot) = slot {
            let Some(game) = &mut self.game else { return };
            match game.unequip(slot) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            return;
        }
        if let Some((item, _)) = inventory.get(idx - 3) {
            self.pending_inventory_item = Some(item.clone());
            self.mode = Mode::InventoryItemAction;
        }
    }

    fn handle_inventory_item_action_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_inventory_item = None;
            self.mode = Mode::Inventory;
            return;
        }
        let Some(item) = self.pending_inventory_item.clone() else {
            self.mode = Mode::Inventory;
            return;
        };
        let actions: Vec<char> = {
            let Some(game) = &self.game else {
                self.mode = Mode::Inventory;
                return;
            };
            inventory_item_actions(game, &item)
                .into_iter()
                .map(|(k, _)| k)
                .collect()
        };
        let idx = self
            .selected_index(key, actions.len())
            .or_else(|| match key {
                GameKey::Char(c) => actions.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        if idx.map(|i| actions[i]) == Some('x') {
            self.pending_erase = Some(item);
            self.erase_quantity_input.clear();
            self.mode = Mode::EraseQuantity;
            self.pending_inventory_item = None;
            return;
        }
        if idx.map(|i| actions[i]) == Some('c') {
            let Some(game) = &mut self.game else { return };
            game.use_item(&item);
            self.status_line = None;
            self.pending_inventory_item = None;
            self.mode = Mode::Inventory;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let result = match idx.map(|i| actions[i]) {
            Some('e') => Some(game.equip(&item)),
            Some('u') => Some(game.fuse_item(&item)),
            _ => None,
        };
        let Some(result) = result else { return };
        match result {
            Ok(()) => self.status_line = None,
            Err(e) => self.status_line = Some(e),
        }
        self.pending_inventory_item = None;
        self.mode = Mode::Inventory;
    }

    /// Second page of the erase flow: how many units of `pending_erase` to
    /// destroy. `[A]` erases the whole stack, matching the pre-cap
    /// behavior. An empty input on Enter means 1.
    fn handle_erase_quantity_key(&mut self, key: GameKey) {
        let Some(item) = self.pending_erase.clone() else {
            self.mode = Mode::Inventory;
            return;
        };
        let stack_qty = self
            .game
            .as_ref()
            .map(|g| {
                g.player_status()
                    .inventory
                    .iter()
                    .find(|(i, _)| *i == item)
                    .map(|(_, q)| *q)
                    .unwrap_or(0)
            })
            .unwrap_or(0);
        match key {
            GameKey::Esc => {
                self.pending_erase = None;
                self.erase_quantity_input.clear();
                self.mode = Mode::Inventory;
            }
            GameKey::Backspace => {
                self.erase_quantity_input.pop();
            }
            GameKey::Char(c) if c.is_ascii_digit() && self.erase_quantity_input.len() < 4 => {
                self.erase_quantity_input.push(c);
            }
            GameKey::Char('a') | GameKey::Char('A') => {
                self.commit_erase(item, stack_qty);
            }
            GameKey::Enter => {
                let quantity: u32 = if self.erase_quantity_input.is_empty() {
                    1
                } else {
                    self.erase_quantity_input.parse().unwrap_or(0)
                };
                self.commit_erase(item, quantity);
            }
            _ => {}
        }
    }

    /// Calls `Game::erase_item` and returns to the inventory screen. A
    /// quantity of 0 is a silent no-op rather than a round-trip to the
    /// engine for an error, matching `commit_craft`.
    fn commit_erase(&mut self, item: ItemId, quantity: u32) {
        self.pending_erase = None;
        self.erase_quantity_input.clear();
        self.mode = Mode::Inventory;
        if quantity == 0 {
            return;
        }
        if let Some(game) = &mut self.game {
            match game.erase_item(&item, quantity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }

    fn handle_help_key(&mut self) {
        self.mode = Mode::Playing;
    }

    fn handle_game_over_key(&mut self) {
        self.game = None;
        self.status_line = None;
        self.mode = Mode::MainMenu;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app(seed: u32) -> App {
        let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let saves_dir =
            std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}_saves"));
        let history_path =
            std::env::temp_dir().join(format!("feral_processes_appcore_test_{seed}.log"));
        let mut app = App::new(assets_dir.clone(), saves_dir, history_path);
        app.game = Game::new(seed, DifficultyMode::Forgiving, &assets_dir).ok();
        app.mode = Mode::Playing;
        app
    }

    #[test]
    fn starting_a_new_game_creates_a_listed_save_that_can_be_loaded_and_deleted() {
        let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let saves_dir = std::env::temp_dir().join(format!(
            "feral_processes_appcore_test_savelist_{}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&saves_dir);
        std::fs::create_dir_all(&saves_dir).unwrap();
        let history_path = std::env::temp_dir().join(format!(
            "feral_processes_appcore_test_savelist_{}.log",
            std::process::id()
        ));
        let mut app = App::new(assets_dir, saves_dir.clone(), history_path);

        app.start_new_game(DifficultyMode::Forgiving);
        assert!(
            app.mode == Mode::Playing,
            "starting a new game should enter Playing"
        );
        let saves = app.list_saves();
        assert_eq!(
            saves.len(),
            1,
            "starting a new game should immediately create one listed save"
        );
        assert!(
            saves[0].summary.is_some(),
            "a freshly saved game should be readable back"
        );

        // Back to the main menu, then load that save from the list.
        app.game = None;
        app.mode = Mode::MainMenu;
        app.handle_key(GameKey::Char('l'));
        assert!(
            app.mode == Mode::LoadGame,
            "'l' should open the load list once a save exists"
        );
        app.handle_key(GameKey::Char('1'));
        assert!(
            app.mode == Mode::SaveAction,
            "picking a save should open the load/delete choice"
        );
        app.handle_key(GameKey::Char('l'));
        assert!(
            app.mode == Mode::Playing,
            "loading should return to Playing"
        );
        assert!(app.game.is_some(), "loading should populate the game");

        // Delete it.
        app.game = None;
        app.mode = Mode::MainMenu;
        app.handle_key(GameKey::Char('l'));
        app.handle_key(GameKey::Char('1'));
        app.handle_key(GameKey::Char('x'));
        assert!(
            app.list_saves().is_empty(),
            "deleting the only save should empty the list"
        );

        let _ = std::fs::remove_dir_all(&saves_dir);
    }

    fn structure_count(app: &mut App) -> usize {
        app.game
            .as_mut()
            .unwrap()
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_structure)
            .count()
    }

    /// Exercises the exact key sequence a player drives at the keyboard —
    /// `b` to open Build, a number to pick a structure, then a direction to
    /// place it — entirely through `App::handle_key`, to make sure the
    /// build/deploy flow (as opposed to `Game::place_structure` in
    /// isolation, which the engine's own tests already cover) still works
    /// end to end after the menu-navigation changes. Loops over every
    /// structure number and every direction (re-opening the build menu each
    /// time, exactly as a player retrying would) rather than assuming
    /// number "1" is affordable or a given direction is walkable — with
    /// starting resources, several of the ten structures are affordable, so
    /// this only fails if the *menu itself* is broken, not because of which
    /// particular structure a fresh session happens to put at each digit.
    #[test]
    fn t_opens_the_research_menu_and_esc_closes_it() {
        let mut app = test_app(501);
        app.handle_key(GameKey::Char('T'));
        assert!(matches!(app.mode, Mode::Research));
        app.handle_key(GameKey::Esc);
        assert!(matches!(app.mode, Mode::Playing));
    }

    #[test]
    fn picking_an_unaffordable_research_node_reports_why_and_stays_open() {
        let mut app = test_app(502);
        app.handle_key(GameKey::Char('T'));
        app.handle_key(GameKey::Char('1'));
        assert!(
            matches!(app.mode, Mode::Research),
            "the menu stays open so several nodes can be taken in one visit"
        );
        assert!(
            app.status_line
                .as_ref()
                .is_some_and(|s| s.contains("Research Data")),
            "got: {:?}",
            app.status_line
        );
    }

    #[test]
    fn build_menu_number_key_reaches_the_direction_picker_and_can_place_a_structure() {
        let mut app = test_app(101);
        assert!(app.game.is_some(), "test game should have loaded");
        assert!(app.mode == Mode::Playing);

        let structure_count_in_menu = app.game.as_mut().unwrap().buildable_structure_defs().len();
        let mut placed = false;
        // Navigate with Down + Enter rather than a digit key, both to
        // exercise the new arrow-navigation path and because a menu with
        // more than 9 rows can't be reached by a single digit at all.
        'outer: for n in 0..structure_count_in_menu {
            for dir in [GameKey::Up, GameKey::Down, GameKey::Left, GameKey::Right] {
                let before = structure_count(&mut app);

                app.handle_key(GameKey::Char('b'));
                assert!(app.mode == Mode::Build, "'b' should open the build menu");

                for _ in 0..n {
                    app.handle_key(GameKey::Down);
                }
                app.handle_key(GameKey::Enter);
                assert!(
                    app.mode == Mode::BuildDirection,
                    "picking structure {n} via Down+Enter should move to the direction picker"
                );

                app.handle_key(dir);
                assert!(
                    app.mode == Mode::Playing,
                    "the direction picker should return to Playing either way"
                );

                if structure_count(&mut app) > before {
                    placed = true;
                    break 'outer;
                }
            }
        }
        assert!(
            placed,
            "should have been able to place at least one of the {structure_count_in_menu} structures \
             in at least one of the four directions"
        );
    }

    /// Exercises the `R` demolish flow end to end through `App::handle_key`:
    /// picking Home moves to a confirmation step instead of demolishing
    /// immediately (unlike any other structure — see `Game::remove_structure`
    /// for why Home is special), `n` backs out leaving it standing, and `y`
    /// actually demolishes it.
    #[test]
    fn remove_key_on_home_requires_confirmation_before_demolishing() {
        let mut app = test_app(203);

        app.handle_key(GameKey::Char('b'));
        assert!(app.mode == Mode::Build, "'b' should open the build menu");
        app.handle_key(GameKey::Enter);
        assert!(
            app.mode == Mode::BuildDirection,
            "Home is the first entry in the build menu"
        );
        app.handle_key(GameKey::Up);
        assert!(app.mode == Mode::Playing);
        assert_eq!(structure_count(&mut app), 1, "Home should now be deployed");

        app.handle_key(GameKey::Char('R'));
        assert!(
            app.mode == Mode::Remove,
            "'R' should open the demolish menu"
        );
        app.handle_key(GameKey::Enter);
        assert!(
            app.mode == Mode::RemoveConfirm,
            "picking Home should require confirmation instead of demolishing immediately"
        );
        assert_eq!(
            structure_count(&mut app),
            1,
            "Home shouldn't be removed yet"
        );

        app.handle_key(GameKey::Char('n'));
        assert!(app.mode == Mode::Playing);
        assert_eq!(
            structure_count(&mut app),
            1,
            "declining the warning should leave Home in place"
        );

        app.handle_key(GameKey::Char('R'));
        app.handle_key(GameKey::Enter);
        assert!(app.mode == Mode::RemoveConfirm);
        app.handle_key(GameKey::Char('y'));
        assert!(app.mode == Mode::Playing);
        assert_eq!(
            structure_count(&mut app),
            0,
            "confirming should demolish Home"
        );
    }

    /// `SoundEvent`s are the seam frontends use to play movement/battle
    /// sound effects — this doesn't try to reach every variant (the
    /// engine's own battle tests already cover the mechanics that decide
    /// which one fires), just locks in that a movement key queues exactly
    /// one of `Step`/`BattleStart`, that a non-movement key queues neither,
    /// and that `take_sounds` actually drains the queue rather than
    /// leaking across keypresses.
    #[test]
    fn movement_keys_queue_exactly_one_step_or_battle_start_sound() {
        let mut app = test_app(202);
        assert!(
            app.take_sounds().is_empty(),
            "a fresh App should start with no queued sounds"
        );

        app.handle_key(GameKey::Char('.'));
        assert!(
            app.take_sounds().is_empty(),
            "waiting isn't a movement key and shouldn't queue a movement sound"
        );

        app.handle_key(GameKey::Right);
        let sounds = app.take_sounds();
        assert_eq!(
            sounds.len(),
            1,
            "a movement key should queue exactly one sound, got {sounds:?}"
        );
        assert!(
            matches!(sounds[0], SoundEvent::Step | SoundEvent::BattleStart),
            "a movement key should queue Step or BattleStart, got {:?}",
            sounds[0]
        );
        assert!(
            app.take_sounds().is_empty(),
            "take_sounds should drain the queue, not just peek it"
        );
    }

    /// The invariant every menu renderer leans on: the key a row advertises
    /// is the key that picks it. Twelve research nodes and ten deployable
    /// structures both run past the nine rows a digit can address, and rows
    /// beyond that used to be unreachable by shortcut entirely.
    #[test]
    fn every_row_shortcut_selects_the_row_it_labels() {
        let mut app = test_app(920);
        let len = 35;
        for idx in 0..len {
            let shortcut = menu_shortcut(idx);
            assert_eq!(
                app.selected_index(GameKey::Char(shortcut), len),
                Some(idx),
                "row {idx} is labelled [{shortcut}] but that key picks something else"
            );
        }
    }

    #[test]
    fn row_shortcuts_run_digits_first_then_letters() {
        assert_eq!(menu_shortcut(0), '1');
        assert_eq!(menu_shortcut(8), '9');
        assert_eq!(menu_shortcut(9), 'a');
        assert_eq!(menu_shortcut(11), 'c');
        assert_eq!(menu_shortcut(34), 'z');
        assert_eq!(
            menu_shortcut(35),
            '-',
            "past 'z' a row should advertise no key rather than a dead one"
        );
    }

    /// The main-menu, save, difficulty and demolish-confirm handlers all
    /// map letters to their own actions through `.or_else`, which only
    /// works while `selected_index` leaves letters alone. They're short
    /// menus, so the letter rows never come into play — but nothing else
    /// enforces that, so lock it in.
    #[test]
    fn letters_pick_no_row_in_a_menu_shorter_than_ten_rows() {
        let mut app = test_app(921);
        for len in 1..=DIGIT_ROWS {
            for c in ['a', 'l', 'n', 'q', 'x', 'y', 'f', 'm', 'p'] {
                assert_eq!(
                    app.selected_index(GameKey::Char(c), len),
                    None,
                    "[{c}] must stay free for a {len}-row menu's own shortcuts"
                );
            }
        }
    }

    /// Scans seeds until one puts a wild program next to the player, then
    /// bumps it to open a battle. Returns the app sitting in `Mode::Battle`
    /// with the entry sounds already drained, so a caller can attribute
    /// anything it observes afterwards to the key it pressed.
    fn battling_app() -> App {
        for seed in 0..200u32 {
            let mut app = test_app(seed);
            let game = app.game.as_mut().unwrap();
            let player = game.player_status().position;
            let target = game
                .view_entities(12, 12)
                .into_iter()
                .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
                .find(|e| (e.pos.0 - player.0).abs() + (e.pos.1 - player.1).abs() == 1);
            let Some(target) = target else { continue };
            app.handle_key(match (target.pos.0 - player.0, target.pos.1 - player.1) {
                (1, 0) => GameKey::Right,
                (-1, 0) => GameKey::Left,
                (0, 1) => GameKey::Down,
                _ => GameKey::Up,
            });
            if app.mode == Mode::Battle {
                let _ = app.take_sounds();
                return app;
            }
        }
        panic!("no seed under 200 put a wild program next to the player — encounter setup changed");
    }

    /// Both renderers advertise the battle actions as `[A]ttack`,
    /// `[D]ecompile`, `[C]ommand companion` and `[J]ack Out`, so a player
    /// reading the prompt has every reason to hold Shift. Case is
    /// normalized everywhere else a letter picks a menu row, and a battle
    /// turn is the one place where swallowing the keypress silently costs
    /// the player a round.
    ///
    /// Asserts only that the key was routed at all — which action each one
    /// resolves to is the engine's business, and depends on gear and party
    /// the seed happens to hand out.
    #[test]
    fn battle_action_keys_ignore_case() {
        for upper in ['A', 'D', 'C', 'J'] {
            let mut app = battling_app();
            app.handle_key(GameKey::Char(upper));
            let acted = !app.take_sounds().is_empty()
                || app.status_line.is_some()
                || app.mode != Mode::Battle;
            assert!(
                acted,
                "[{upper}] is what the battle prompt advertises, but Shift+{upper} was swallowed"
            );
        }
    }

    /// `update_realtime` is the hook a frontend's own loop calls every
    /// frame, independent of `handle_key`, so the world keeps advancing
    /// while the player is idle — but only in `Mode::Playing`. Backdates
    /// `last_realtime_tick` instead of actually sleeping so the test stays
    /// fast and deterministic.
    #[test]
    fn update_realtime_ticks_once_a_second_only_while_playing() {
        let mut app = test_app(303);
        let start_tick = app.game.as_ref().unwrap().current_tick();

        // Not enough wall-clock time has passed yet.
        app.last_realtime_tick = Instant::now();
        app.update_realtime();
        assert_eq!(
            app.game.as_ref().unwrap().current_tick(),
            start_tick,
            "update_realtime shouldn't tick before a full second has elapsed"
        );

        // A full second (backdated) should fire exactly one idle tick.
        app.last_realtime_tick = Instant::now() - Duration::from_secs(2);
        app.update_realtime();
        assert_eq!(
            app.game.as_ref().unwrap().current_tick(),
            start_tick + 1,
            "update_realtime should advance the world by one tick once a second has passed"
        );

        // Paused outside Playing (any menu, or battle via its own Mode) —
        // no tick, and the timer resets rather than banking elapsed time.
        app.mode = Mode::Inventory;
        app.last_realtime_tick = Instant::now() - Duration::from_secs(5);
        app.update_realtime();
        assert_eq!(
            app.game.as_ref().unwrap().current_tick(),
            start_tick + 1,
            "update_realtime shouldn't tick while paused on a non-Playing mode"
        );
    }

    #[test]
    fn erasing_asks_for_a_quantity_and_removes_exactly_that_many() {
        let mut app = test_app(900);
        let before = app
            .game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::from(ids::CORE_FRAGMENT))
            .map(|(_, q)| *q)
            .unwrap();

        app.pending_inventory_item = Some(ItemId::from(ids::CORE_FRAGMENT));
        app.mode = Mode::InventoryItemAction;
        app.handle_key(GameKey::Char('x'));
        assert_eq!(
            app.mode,
            Mode::EraseQuantity,
            "[X] should ask how many, not dump the whole stack"
        );

        app.handle_key(GameKey::Char('3'));
        app.handle_key(GameKey::Enter);

        let after = app
            .game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::from(ids::CORE_FRAGMENT))
            .map(|(_, q)| *q)
            .unwrap();
        assert_eq!(after, before - 3);
        assert_eq!(app.mode, Mode::Inventory);
    }

    #[test]
    fn erase_all_dumps_the_whole_stack() {
        let mut app = test_app(901);
        app.pending_inventory_item = Some(ItemId::from(ids::CORE_FRAGMENT));
        app.mode = Mode::InventoryItemAction;
        app.handle_key(GameKey::Char('x'));
        app.handle_key(GameKey::Char('a'));

        let held = app
            .game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == ItemId::from(ids::CORE_FRAGMENT))
            .map(|(_, q)| *q);
        assert_eq!(held, None, "[A] should clear the stack entirely");
    }

    #[test]
    fn escaping_the_erase_prompt_erases_nothing() {
        let mut app = test_app(902);
        let before = app.game.as_ref().unwrap().player_status().inventory;
        app.pending_inventory_item = Some(ItemId::from(ids::CORE_FRAGMENT));
        app.mode = Mode::InventoryItemAction;
        app.handle_key(GameKey::Char('x'));
        app.handle_key(GameKey::Esc);

        assert_eq!(app.mode, Mode::Inventory);
        assert_eq!(app.game.as_ref().unwrap().player_status().inventory, before);
    }

    #[test]
    fn every_equippable_item_offers_equip_fuse_and_erase() {
        let app = test_app(904);
        let game = app.game.as_ref().unwrap();
        for item in [
            ItemId::from(ids::OVERCLOCK_CORE),
            ItemId::from(ids::MONOFILAMENT_WHIP),
            ItemId::from(ids::FIREWALL_PLATING),
            ItemId::from(ids::ABLATIVE_PLATING),
            ItemId::from(ids::NEURAL_AMPLIFIER),
            ItemId::from(ids::CORTEX_HACK),
        ] {
            let keys: Vec<char> = inventory_item_actions(game, &item)
                .into_iter()
                .map(|(k, _)| k)
                .collect();
            assert_eq!(
                keys,
                vec!['e', 'u', 'x'],
                "{} should offer fuse regardless of how many copies are held",
                game.item_name(&item)
            );
        }
    }

    #[test]
    fn a_plain_resource_offers_only_erase() {
        let app = test_app(905);
        let game = app.game.as_ref().unwrap();
        let keys: Vec<char> = inventory_item_actions(game, &ItemId::from(ids::CORE_FRAGMENT))
            .into_iter()
            .map(|(k, _)| k)
            .collect();
        assert_eq!(keys, vec!['x']);
    }

    #[test]
    fn fusing_without_enough_copies_explains_why_instead_of_ignoring_the_key() {
        let mut app = test_app(903);
        app.pending_inventory_item = Some(ItemId::from(ids::OVERCLOCK_CORE));
        app.mode = Mode::InventoryItemAction;
        app.handle_key(GameKey::Char('u'));

        assert_eq!(
            app.status_line.as_deref(),
            Some("Need 2 Overclock Core to fuse (have 0)."),
            "[U] on a too-small stack must refuse out loud, not silently do nothing"
        );
    }
}
