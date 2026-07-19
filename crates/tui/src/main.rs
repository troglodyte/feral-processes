mod ui;

use std::io;
use std::path::PathBuf;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use crossterm::event::{self, Event, KeyCode, KeyEventKind};

use feral_processes_engine::items::{EquipmentSlot, ItemId};
use feral_processes_engine::{DifficultyMode, Entity, Game};

/// Radius (in tiles) scanned for the build/work menus, independent of the
/// visible viewport size.
pub const MENU_SCAN_RADIUS: i32 = 40;

/// How many game ticks (see `Game::current_tick`) pass between autosaves —
/// paced against game time rather than wall-clock time, so it's the same
/// whether the player is acting quickly or sitting on a menu.
const AUTOSAVE_INTERVAL_TICKS: u64 = 50;

#[derive(Clone, Copy, PartialEq, Eq)]
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
    Symlink,
    InspectDirection,
    InspectDetail,
    Inventory,
    InventoryItemAction,
    Companion,
    Fuse,
    FuseSecond,
    Trade,
    TradeAction,
    TradeQuantity,
    Perks,
    Help,
    GameOver,
}

/// A line item picked in `Mode::TradeAction`, awaiting a quantity from
/// `Mode::TradeQuantity` before `Game::sell_item`/`Game::buy_item` is
/// actually called.
#[derive(Clone, Copy)]
pub enum TradeChoice {
    Sell(ItemId),
    Buy(ItemId),
}

const MIN_ZOOM: u16 = 1;
const MAX_ZOOM: u16 = 4;

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
    pending_save: Option<PathBuf>,
    history_path: PathBuf,
    pub quit: bool,
    pending_structure: Option<String>,
    pending_worker: Option<Entity>,
    pending_inspect: Option<Entity>,
    /// The first program picked in `Mode::Fuse`, awaiting a second from
    /// `Mode::FuseSecond` before `Game::fuse_companions` is actually called.
    pending_fuse_first: Option<Entity>,
    pending_inventory_item: Option<ItemId>,
    /// The recipe result picked in `Mode::Craft`, awaiting a quantity from
    /// `Mode::CraftQuantity` before `Game::craft` is actually called.
    pending_craft: Option<ItemId>,
    /// Digits typed so far on the craft-quantity page.
    craft_quantity_input: String,
    /// The trading post picked in `Mode::Trade`, awaiting a line-item pick
    /// from `Mode::TradeAction`.
    pending_trade_structure: Option<Entity>,
    /// The sell/buy line item picked in `Mode::TradeAction`, awaiting a
    /// quantity from `Mode::TradeQuantity` before `Game::sell_item`/
    /// `Game::buy_item` is actually called.
    pending_trade_choice: Option<TradeChoice>,
    /// Digits typed so far on the trade-quantity page.
    trade_quantity_input: String,
    /// How many screen characters render each world tile along each axis.
    pub zoom: u16,
    /// Which row is highlighted on the current numbered/lettered menu, for
    /// Up/Down-arrow-plus-Enter navigation (see `App::selected_index`) —
    /// on top of, not instead of, typing a row's own number/letter directly.
    /// Reset to 0 every time a menu mode is entered.
    pub menu_selected: usize,
    /// The game tick (see `Game::current_tick`) as of the last autosave —
    /// reset to the current tick whenever a game starts or loads, so a
    /// resumed session doesn't immediately autosave on its very first move.
    last_autosave_tick: u64,
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
    fn new(assets_dir: PathBuf, saves_dir: PathBuf, history_path: PathBuf) -> Self {
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
            pending_inspect: None,
            pending_fuse_first: None,
            pending_inventory_item: None,
            pending_craft: None,
            craft_quantity_input: String::new(),
            pending_trade_structure: None,
            pending_trade_choice: None,
            trade_quantity_input: String::new(),
            zoom: 2,
            menu_selected: 0,
            last_autosave_tick: 0,
        }
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
                let modified = e.metadata().and_then(|m| m.modified()).unwrap_or(std::time::UNIX_EPOCH);
                let name = path
                    .file_stem()
                    .map(|s| s.to_string_lossy().into_owned())
                    .unwrap_or_else(|| path.to_string_lossy().into_owned());
                let summary = feral_processes_engine::save::load_from_file(&path).ok().map(|data| {
                    format!(
                        "Lv{} · Zone {} · {:?} · tick {}",
                        data.player.level, data.zone, data.difficulty, data.tick
                    )
                });
                (modified, SaveEntry { path, name, summary })
            })
            .collect();
        saves.sort_by_key(|(modified, _)| std::cmp::Reverse(*modified));
        saves.into_iter().map(|(_, entry)| entry).collect()
    }

    /// A fresh, filesystem-safe save filename for a just-started game —
    /// unique enough for one-per-second play sessions, which is the only
    /// case that matters here.
    fn new_save_path(&self) -> PathBuf {
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map(|d| d.as_secs()).unwrap_or(0);
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
        let Some(path) = &self.current_save_path else { return };
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
        let Some(path) = self.current_save_path.clone() else { return };
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

    /// Shared Up/Down/Enter handling layered on top of every numbered menu's
    /// existing direct digit-key shortcuts (1-9) — this doesn't replace
    /// them, it's just another way to pick the same row. `len` is how many
    /// selectable rows the menu currently has. A typed digit 1-`len` resolves
    /// immediately to that 0-based index, same as before; Up/Down instead
    /// move `menu_selected` (wrapping) and return `None`; Enter resolves to
    /// whatever `menu_selected` currently highlights. Any other key, or an
    /// empty menu, returns `None`.
    fn selected_index(&mut self, code: KeyCode, len: usize) -> Option<usize> {
        if len == 0 {
            return None;
        }
        if let KeyCode::Char(c) = code {
            return c.to_digit(10).and_then(|d| {
                let d = d as usize;
                (d >= 1 && d <= len).then_some(d - 1)
            });
        }
        match code {
            KeyCode::Up => {
                self.menu_selected = (self.menu_selected + len - 1) % len;
                None
            }
            KeyCode::Down => {
                self.menu_selected = (self.menu_selected + 1) % len;
                None
            }
            KeyCode::Enter => Some(self.menu_selected.min(len - 1)),
            _ => None,
        }
    }

    fn handle_key(&mut self, code: KeyCode) {
        let mode_before = self.mode;
        match self.mode {
            Mode::MainMenu => self.handle_main_menu_key(code),
            Mode::LoadGame => self.handle_load_game_key(code),
            Mode::SaveAction => self.handle_save_action_key(code),
            Mode::DifficultyPick => self.handle_difficulty_key(code),
            Mode::Playing => self.handle_playing_key(code),
            Mode::Battle => self.handle_battle_key(code),
            Mode::BattleCompanion => self.handle_battle_companion_key(code),
            Mode::Build => self.handle_build_key(code),
            Mode::BuildDirection => self.handle_build_direction_key(code),
            Mode::Craft => self.handle_craft_key(code),
            Mode::CraftQuantity => self.handle_craft_quantity_key(code),
            Mode::Cronjob => self.handle_cronjob_key(code),
            Mode::CronjobStructure => self.handle_cronjob_structure_key(code),
            Mode::Guard => self.handle_guard_key(code),
            Mode::GuardStructure => self.handle_guard_structure_key(code),
            Mode::Symlink => self.handle_symlink_key(code),
            Mode::InspectDirection => self.handle_inspect_direction_key(code),
            Mode::InspectDetail => self.handle_inspect_detail_key(code),
            Mode::Inventory => self.handle_inventory_key(code),
            Mode::InventoryItemAction => self.handle_inventory_item_action_key(code),
            Mode::Companion => self.handle_companion_key(code),
            Mode::Fuse => self.handle_fuse_key(code),
            Mode::FuseSecond => self.handle_fuse_second_key(code),
            Mode::Trade => self.handle_trade_key(code),
            Mode::TradeAction => self.handle_trade_action_key(code),
            Mode::TradeQuantity => self.handle_trade_quantity_key(code),
            Mode::Perks => self.handle_perks_key(code),
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

    fn handle_main_menu_key(&mut self, code: KeyCode) {
        let mut options = vec!['n'];
        if !self.list_saves().is_empty() {
            options.push('l');
        }
        options.push('q');
        let idx = self
            .selected_index(code, options.len())
            .or_else(|| match code {
                KeyCode::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
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

    fn handle_load_game_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let saves = self.list_saves();
        if let Some(idx) = self.selected_index(code, saves.len()) {
            self.pending_save = Some(saves[idx].path.clone());
            self.mode = Mode::SaveAction;
        }
    }

    fn handle_save_action_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
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
            .selected_index(code, options.len())
            .or_else(|| match code {
                KeyCode::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
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

    fn handle_difficulty_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::MainMenu;
            return;
        }
        let options = ['p', 'f'];
        let idx = self
            .selected_index(code, options.len())
            .or_else(|| match code {
                KeyCode::Char(c) => options.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        match idx.map(|i| options[i]) {
            Some('p') => self.start_new_game(DifficultyMode::Permadeath),
            Some('f') => self.start_new_game(DifficultyMode::Forgiving),
            _ => {}
        }
    }

    fn handle_playing_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('b') => {
                self.mode = Mode::Build;
                return;
            }
            KeyCode::Char('c') => {
                self.mode = Mode::Craft;
                return;
            }
            KeyCode::Char('w') => {
                self.mode = Mode::Cronjob;
                return;
            }
            KeyCode::Char('G') => {
                self.mode = Mode::Guard;
                return;
            }
            KeyCode::Char('u') => {
                self.mode = Mode::Symlink;
                return;
            }
            KeyCode::Char('i') => {
                self.mode = Mode::InspectDirection;
                return;
            }
            KeyCode::Char('v') => {
                self.mode = Mode::Inventory;
                return;
            }
            KeyCode::Char('p') => {
                self.mode = Mode::Companion;
                return;
            }
            KeyCode::Char('f') => {
                self.mode = Mode::Fuse;
                return;
            }
            KeyCode::Char('t') => {
                self.mode = Mode::Trade;
                return;
            }
            KeyCode::Char('x') => {
                self.mode = Mode::Perks;
                return;
            }
            KeyCode::Char('s') => {
                self.save_game();
                return;
            }
            KeyCode::Char('q') => {
                self.game = None;
                self.status_line = None;
                self.mode = Mode::MainMenu;
                return;
            }
            KeyCode::Char('?') => {
                self.mode = Mode::Help;
                return;
            }
            KeyCode::Char('+') | KeyCode::Char('=') => {
                self.zoom = (self.zoom + 1).min(MAX_ZOOM);
                return;
            }
            KeyCode::Char('-') | KeyCode::Char('_') => {
                self.zoom = self.zoom.saturating_sub(1).max(MIN_ZOOM);
                return;
            }
            _ => {}
        }

        let acted = {
            let Some(game) = &mut self.game else { return };
            match code {
                KeyCode::Up | KeyCode::Char('k') => {
                    game.move_player(0, -1);
                    true
                }
                KeyCode::Down | KeyCode::Char('j') => {
                    game.move_player(0, 1);
                    true
                }
                KeyCode::Left | KeyCode::Char('h') => {
                    game.move_player(-1, 0);
                    true
                }
                KeyCode::Right | KeyCode::Char('l') => {
                    game.move_player(1, 0);
                    true
                }
                KeyCode::Char('.') => {
                    game.wait();
                    true
                }
                KeyCode::Char('e') => {
                    game.eat(ItemId::PowerCell);
                    true
                }
                KeyCode::Char('r') => {
                    game.rest();
                    true
                }
                KeyCode::Char('g') => {
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
        if self
            .game
            .as_ref()
            .map(|g| g.has_active_battle())
            .unwrap_or(false)
        {
            self.mode = Mode::Battle;
        }
        self.check_game_over();
    }

    fn handle_battle_key(&mut self, code: KeyCode) {
        if code == KeyCode::Char('c') {
            let Some(game) = &mut self.game else { return };
            let party = game.player_status().companions;
            match party.len() {
                0 => self.status_line = Some("You have no active companion.".to_string()),
                1 => {
                    let entity = party[0].entity;
                    game.battle_command_companion(entity);
                    if !game.has_active_battle() {
                        self.mode = Mode::Playing;
                    }
                    self.check_game_over();
                }
                _ => self.mode = Mode::BattleCompanion,
            }
            return;
        }

        let still_active = {
            let Some(game) = &mut self.game else { return };
            match code {
                KeyCode::Char('a') => game.battle_attack(),
                KeyCode::Char('d') => game.battle_decompile(),
                KeyCode::Char('j') => game.battle_flee(),
                _ => return,
            }
            game.has_active_battle()
        };
        if !still_active {
            self.mode = Mode::Playing;
        }
        self.check_game_over();
    }

    /// Picks which party member acts this round when there's more than one
    /// active companion (a single companion is commanded directly from
    /// `handle_battle_key` with no extra step).
    fn handle_battle_companion_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Battle;
            return;
        }
        let Some(game) = &self.game else { return };
        let party = game.player_status().companions;
        let Some(idx) = self.selected_index(code, party.len()) else {
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
        self.check_game_over();
    }

    fn handle_build_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &self.game else { return };
        let defs = game.structure_defs();
        if let Some(idx) = self.selected_index(code, defs.len()) {
            self.pending_structure = Some(defs[idx].id.clone());
            self.mode = Mode::BuildDirection;
        }
    }

    fn handle_craft_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let recipes = game.craft_recipes();
        if let Some(idx) = self.selected_index(code, recipes.len()) {
            self.pending_craft = Some(recipes[idx].result);
            self.craft_quantity_input.clear();
            self.mode = Mode::CraftQuantity;
        }
    }

    /// Second page of the compile flow: asks how many units of
    /// `pending_craft` to make before actually calling `Game::craft`. `[F]`
    /// is a shortcut for 5 at once, `[M]` for the most affordable right now
    /// (see `Game::max_craftable`) — both bypass typing digits and Enter.
    fn handle_craft_quantity_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.pending_craft = None;
                self.craft_quantity_input.clear();
                self.mode = Mode::Craft;
            }
            KeyCode::Backspace => {
                self.craft_quantity_input.pop();
            }
            KeyCode::Char(c) if c.is_ascii_digit() && self.craft_quantity_input.len() < 4 => {
                self.craft_quantity_input.push(c);
            }
            KeyCode::Char('f') | KeyCode::Char('F') => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                self.craft_quantity_input.clear();
                self.commit_craft(result, 5);
            }
            KeyCode::Char('m') | KeyCode::Char('M') => {
                let Some(result) = self.pending_craft.take() else {
                    self.mode = Mode::Playing;
                    return;
                };
                self.craft_quantity_input.clear();
                let max = self
                    .game
                    .as_ref()
                    .map(|g| g.max_craftable(result))
                    .unwrap_or(0);
                if max == 0 {
                    self.status_line = Some(format!(
                        "Not enough resources to compile any {}.",
                        result.display_name()
                    ));
                    self.mode = Mode::Playing;
                    return;
                }
                self.commit_craft(result, max);
            }
            KeyCode::Enter => {
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
            match game.craft(result, quantity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
        self.mode = Mode::Playing;
    }

    fn handle_build_direction_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.pending_structure = None;
            self.mode = Mode::Playing;
            return;
        }
        let dir = match code {
            KeyCode::Up | KeyCode::Char('k') => Some((0, -1)),
            KeyCode::Down | KeyCode::Char('j') => Some((0, 1)),
            KeyCode::Left | KeyCode::Char('h') => Some((-1, 0)),
            KeyCode::Right | KeyCode::Char('l') => Some((1, 0)),
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

    fn handle_cronjob_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let workers: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed)
            .collect();
        if let Some(idx) = self.selected_index(code, workers.len()) {
            self.pending_worker = Some(workers[idx].entity);
            self.mode = Mode::CronjobStructure;
        }
    }

    fn handle_cronjob_structure_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
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
        if let Some(idx) = self.selected_index(code, structures.len()) {
            let Some(game) = &mut self.game else { return };
            match game.assign_cronjob(worker, structures[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.pending_worker = None;
            self.mode = Mode::Playing;
        }
    }

    fn handle_guard_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let workers: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed)
            .collect();
        if let Some(idx) = self.selected_index(code, workers.len()) {
            self.pending_worker = Some(workers[idx].entity);
            self.mode = Mode::GuardStructure;
        }
    }

    fn handle_guard_structure_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
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
        if let Some(idx) = self.selected_index(code, structures.len()) {
            let Some(game) = &mut self.game else { return };
            match game.assign_guard(worker, structures[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.pending_worker = None;
            self.mode = Mode::Playing;
        }
    }

    /// Lists every deployed symlink-capable structure (e.g. Home) anywhere
    /// on the map — not scan-radius-limited like the build/cronjob
    /// menus — and teleports the player to the picked one.
    fn handle_symlink_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let targets = game.symlink_targets();
        if let Some(idx) = self.selected_index(code, targets.len()) {
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
    fn handle_companion_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let candidates = game.owned_pets();
        if let Some(idx) = self.selected_index(code, candidates.len()) {
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
    fn handle_fuse_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let candidates: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.is_tamed)
            .collect();
        if let Some(idx) = self.selected_index(code, candidates.len()) {
            self.pending_fuse_first = Some(candidates[idx].entity);
            self.mode = Mode::FuseSecond;
        }
    }

    /// Picks the second program to fuse with the one from `handle_fuse_key`,
    /// then actually runs the fusion.
    fn handle_fuse_second_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
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
        if let Some(idx) = self.selected_index(code, candidates.len()) {
            let Some(game) = &mut self.game else { return };
            match game.fuse_companions(first, candidates[idx].entity) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            self.pending_fuse_first = None;
            self.mode = Mode::Playing;
        }
    }

    /// Picks a nearby trading-post structure to open a trade session with.
    fn handle_trade_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let structures: Vec<_> = game
            .view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
            .into_iter()
            .filter(|e| e.can_trade)
            .collect();
        if let Some(idx) = self.selected_index(code, structures.len()) {
            self.pending_trade_structure = Some(structures[idx].entity);
            self.mode = Mode::TradeAction;
        }
    }

    /// Picks a sell (from inventory) or buy (from the structure's trade
    /// list) line item — sell offers are numbered first, then buy offers.
    fn handle_trade_action_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
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
        let sell_items: Vec<ItemId> = game
            .player_status()
            .inventory
            .iter()
            .map(|(item, _)| *item)
            .filter(|item| *item != ItemId::CoreFragment)
            .collect();
        let buy_items: Vec<ItemId> = trade.buy.iter().map(|(item, _)| *item).collect();
        let total = sell_items.len() + buy_items.len();
        if let Some(idx) = self.selected_index(code, total) {
            let choice = if idx < sell_items.len() {
                TradeChoice::Sell(sell_items[idx])
            } else {
                TradeChoice::Buy(buy_items[idx - sell_items.len()])
            };
            self.pending_trade_choice = Some(choice);
            self.trade_quantity_input.clear();
            self.mode = Mode::TradeQuantity;
        }
    }

    /// Types a quantity for the pending sell/buy line item; Enter commits it.
    fn handle_trade_quantity_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Esc => {
                self.pending_trade_choice = None;
                self.trade_quantity_input.clear();
                self.mode = Mode::TradeAction;
            }
            KeyCode::Backspace => {
                self.trade_quantity_input.pop();
            }
            KeyCode::Char(c) if c.is_ascii_digit() && self.trade_quantity_input.len() < 4 => {
                self.trade_quantity_input.push(c);
            }
            KeyCode::Enter => {
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
    fn handle_perks_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let perks = feral_processes_engine::Perk::all();
        if let Some(idx) = self.selected_index(code, perks.len()) {
            let Some(game) = &mut self.game else { return };
            match game.unlock_perk(perks[idx]) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
        }
    }

    /// Picks a direction (arrows/hjkl) and inspects the first creature the
    /// engine finds stepping that way from the player, rather than picking
    /// from a numbered list of grid coordinates.
    fn handle_inspect_direction_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let dir = match code {
            KeyCode::Up | KeyCode::Char('k') => Some((0, -1)),
            KeyCode::Down | KeyCode::Char('j') => Some((0, 1)),
            KeyCode::Left | KeyCode::Char('h') => Some((-1, 0)),
            KeyCode::Right | KeyCode::Char('l') => Some((1, 0)),
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

    fn handle_inspect_detail_key(&mut self, _code: KeyCode) {
        self.pending_inspect = None;
        self.mode = Mode::Playing;
    }

    /// Equipped slots are numbered 1-3 (Weapon/Armor/Module) and unequip
    /// immediately when pressed; unequipped inventory items start at 4 and
    /// open `Mode::InventoryItemAction` for the selected item.
    fn handle_inventory_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &self.game else { return };
        let inventory = game.player_status().inventory;
        let total = 3 + inventory.len();
        let Some(idx) = self.selected_index(code, total) else {
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
        if let Some(&(item, _)) = inventory.get(idx - 3) {
            self.pending_inventory_item = Some(item);
            self.mode = Mode::InventoryItemAction;
        }
    }

    fn handle_inventory_item_action_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.pending_inventory_item = None;
            self.mode = Mode::Inventory;
            return;
        }
        let Some(item) = self.pending_inventory_item else {
            self.mode = Mode::Inventory;
            return;
        };
        let mut actions = vec!['x'];
        if item.equipment().is_some() {
            actions.insert(0, 'e');
        }
        let idx = self
            .selected_index(code, actions.len())
            .or_else(|| match code {
                KeyCode::Char(c) => actions.iter().position(|&o| o == c.to_ascii_lowercase()),
                _ => None,
            });
        let Some(game) = &mut self.game else { return };
        let stack_qty = game
            .player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0);
        let result = match idx.map(|i| actions[i]) {
            Some('e') => Some(game.equip(item)),
            Some('x') => Some(game.erase_item(item, stack_qty)),
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

    fn handle_help_key(&mut self) {
        self.mode = Mode::Playing;
    }

    fn handle_game_over_key(&mut self) {
        self.game = None;
        self.status_line = None;
        self.mode = Mode::MainMenu;
    }

    fn run(&mut self, terminal: &mut ratatui::DefaultTerminal) -> io::Result<()> {
        while !self.quit {
            terminal.draw(|f| ui::render(f, self))?;
            if event::poll(Duration::from_millis(200))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                self.handle_key(key.code);
            }
        }
        Ok(())
    }
}

fn main() -> io::Result<()> {
    let crate_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let repo_root = crate_dir
        .parent()
        .and_then(|p| p.parent())
        .unwrap_or(&crate_dir)
        .to_path_buf();
    let assets_dir = repo_root.join("assets");
    let saves_dir = repo_root.join("saves");
    std::fs::create_dir_all(&saves_dir)?;
    // One-time migration: earlier builds kept a single save at
    // `save.bin`. Move it into the new saves directory (under its old
    // name) so it still shows up in the load list instead of silently
    // disappearing — even if it turns out to be from an incompatible
    // save version, it's still visible there and deletable.
    let legacy_save = repo_root.join("save.bin");
    if legacy_save.exists() {
        let _ = std::fs::rename(&legacy_save, saves_dir.join("save.bin"));
    }
    let history_path = repo_root.join("run_history.log");

    let mut terminal = ratatui::init();
    let mut app = App::new(assets_dir, saves_dir, history_path);
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_app(seed: u32) -> App {
        let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let saves_dir = std::env::temp_dir().join(format!("feral_processes_tui_test_{seed}_saves"));
        let history_path =
            std::env::temp_dir().join(format!("feral_processes_tui_test_{seed}.log"));
        let mut app = App::new(assets_dir.clone(), saves_dir, history_path);
        app.game = Game::new(seed, DifficultyMode::Forgiving, &assets_dir).ok();
        app.mode = Mode::Playing;
        app
    }

    #[test]
    fn starting_a_new_game_creates_a_listed_save_that_can_be_loaded_and_deleted() {
        let assets_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let saves_dir = std::env::temp_dir()
            .join(format!("feral_processes_tui_test_savelist_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&saves_dir);
        std::fs::create_dir_all(&saves_dir).unwrap();
        let history_path = std::env::temp_dir()
            .join(format!("feral_processes_tui_test_savelist_{}.log", std::process::id()));
        let mut app = App::new(assets_dir, saves_dir.clone(), history_path);

        app.start_new_game(DifficultyMode::Forgiving);
        assert!(app.mode == Mode::Playing, "starting a new game should enter Playing");
        let saves = app.list_saves();
        assert_eq!(saves.len(), 1, "starting a new game should immediately create one listed save");
        assert!(saves[0].summary.is_some(), "a freshly saved game should be readable back");

        // Back to the main menu, then load that save from the list.
        app.game = None;
        app.mode = Mode::MainMenu;
        app.handle_key(KeyCode::Char('l'));
        assert!(app.mode == Mode::LoadGame, "'l' should open the load list once a save exists");
        app.handle_key(KeyCode::Char('1'));
        assert!(app.mode == Mode::SaveAction, "picking a save should open the load/delete choice");
        app.handle_key(KeyCode::Char('l'));
        assert!(app.mode == Mode::Playing, "loading should return to Playing");
        assert!(app.game.is_some(), "loading should populate the game");

        // Delete it.
        app.game = None;
        app.mode = Mode::MainMenu;
        app.handle_key(KeyCode::Char('l'));
        app.handle_key(KeyCode::Char('1'));
        app.handle_key(KeyCode::Char('x'));
        assert!(app.list_saves().is_empty(), "deleting the only save should empty the list");

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
    fn build_menu_number_key_reaches_the_direction_picker_and_can_place_a_structure() {
        let mut app = test_app(101);
        assert!(app.game.is_some(), "test game should have loaded");
        assert!(app.mode == Mode::Playing);

        let structure_count_in_menu = app.game.as_mut().unwrap().structure_defs().len();
        let mut placed = false;
        // Navigate with Down + Enter rather than a digit key, both to
        // exercise the new arrow-navigation path and because a menu with
        // more than 9 rows can't be reached by a single digit at all.
        'outer: for n in 0..structure_count_in_menu {
            for dir in [KeyCode::Up, KeyCode::Down, KeyCode::Left, KeyCode::Right] {
                let before = structure_count(&mut app);

                app.handle_key(KeyCode::Char('b'));
                assert!(app.mode == Mode::Build, "'b' should open the build menu");

                for _ in 0..n {
                    app.handle_key(KeyCode::Down);
                }
                app.handle_key(KeyCode::Enter);
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
}
