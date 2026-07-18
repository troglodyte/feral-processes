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

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    MainMenu,
    DifficultyPick,
    Playing,
    Battle,
    BattleCompanion,
    Build,
    BuildDirection,
    Craft,
    CraftQuantity,
    Cronjob,
    CronjobStructure,
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
    save_path: PathBuf,
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
}

impl App {
    fn new(assets_dir: PathBuf, save_path: PathBuf, history_path: PathBuf) -> Self {
        Self {
            mode: Mode::MainMenu,
            game: None,
            status_line: None,
            history_written: false,
            assets_dir,
            save_path,
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
        }
    }

    pub fn save_exists(&self) -> bool {
        self.save_path.exists()
    }

    fn start_new_game(&mut self, difficulty: DifficultyMode) {
        let seed = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs() as u32)
            .unwrap_or(1);
        match Game::new(seed, difficulty, &self.assets_dir) {
            Ok(game) => {
                self.game = Some(game);
                self.history_written = false;
                self.status_line = None;
                self.mode = Mode::Playing;
            }
            Err(e) => self.status_line = Some(format!("Failed to start game: {e}")),
        }
    }

    fn load_game(&mut self) {
        match Game::load(&self.save_path, &self.assets_dir) {
            Ok(game) => {
                self.game = Some(game);
                self.history_written = false;
                self.status_line = None;
                self.mode = Mode::Playing;
            }
            Err(e) => self.status_line = Some(format!("Failed to load game: {e}")),
        }
    }

    fn save_game(&mut self) {
        if let Some(game) = &mut self.game {
            match game.save(&self.save_path) {
                Ok(()) => self.status_line = Some("Game saved.".to_string()),
                Err(e) => self.status_line = Some(format!("Save failed: {e}")),
            }
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

    fn handle_key(&mut self, code: KeyCode) {
        match self.mode {
            Mode::MainMenu => self.handle_main_menu_key(code),
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
    }

    fn handle_main_menu_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('n') | KeyCode::Char('N') => {
                self.status_line = None;
                self.mode = Mode::DifficultyPick;
            }
            KeyCode::Char('l') | KeyCode::Char('L') if self.save_exists() => self.load_game(),
            KeyCode::Char('q') | KeyCode::Char('Q') => self.quit = true,
            _ => {}
        }
    }

    fn handle_difficulty_key(&mut self, code: KeyCode) {
        match code {
            KeyCode::Char('p') | KeyCode::Char('P') => self.start_new_game(DifficultyMode::Permadeath),
            KeyCode::Char('f') | KeyCode::Char('F') => self.start_new_game(DifficultyMode::Forgiving),
            KeyCode::Esc => self.mode = Mode::MainMenu,
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
        if self.game.as_ref().map(|g| g.has_active_battle()).unwrap_or(false) {
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
                    game.battle_companion_attack(entity);
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

    /// Picks which party member attacks this round when there's more than
    /// one active companion (a single companion is commanded directly from
    /// `handle_battle_key` with no extra step).
    fn handle_battle_companion_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Battle;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let party = game.player_status().companions;
        let KeyCode::Char(c) = code else { return };
        let Some(idx) = c.to_digit(10) else { return };
        let idx = idx as usize;
        if idx < 1 || idx > party.len() {
            return;
        }
        let entity = party[idx - 1].entity;
        game.battle_companion_attack(entity);
        let still_active = game.has_active_battle();
        self.mode = if still_active { Mode::Battle } else { Mode::Playing };
        self.check_game_over();
    }

    fn handle_build_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &self.game else { return };
        let defs = game.structure_defs();
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= defs.len() {
                    self.pending_structure = Some(defs[idx - 1].id.clone());
                    self.mode = Mode::BuildDirection;
                }
            }
        }
    }

    fn handle_craft_key(&mut self, code: KeyCode) {
        if code == KeyCode::Esc {
            self.mode = Mode::Playing;
            return;
        }
        let Some(game) = &mut self.game else { return };
        let recipes = game.craft_recipes();
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= recipes.len() {
                    self.pending_craft = Some(recipes[idx - 1].result);
                    self.craft_quantity_input.clear();
                    self.mode = Mode::CraftQuantity;
                }
            }
        }
    }

    /// Second page of the compile flow: asks how many units of
    /// `pending_craft` to make before actually calling `Game::craft`.
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
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if self.craft_quantity_input.len() < 4 {
                    self.craft_quantity_input.push(c);
                }
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
            _ => {}
        }
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
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= workers.len() {
                    self.pending_worker = Some(workers[idx - 1].entity);
                    self.mode = Mode::CronjobStructure;
                }
            }
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
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= structures.len() {
                    match game.assign_cronjob(worker, structures[idx - 1].entity) {
                        Ok(()) => self.status_line = None,
                        Err(e) => self.status_line = Some(e),
                    }
                    self.pending_worker = None;
                    self.mode = Mode::Playing;
                }
            }
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
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= targets.len() {
                    match game.use_symlink(targets[idx - 1].entity) {
                        Ok(()) => self.status_line = None,
                        Err(e) => self.status_line = Some(e),
                    }
                    self.mode = Mode::Playing;
                }
            }
        }
    }

    /// Lists nearby tamed programs; pressing a party member's number stands
    /// it down, pressing any other tamed program's number adds it to the
    /// party (up to `MAX_PARTY_SIZE` at once).
    fn handle_companion_key(&mut self, code: KeyCode) {
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
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= candidates.len() {
                    let candidate = &candidates[idx - 1];
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
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= candidates.len() {
                    self.pending_fuse_first = Some(candidates[idx - 1].entity);
                    self.mode = Mode::FuseSecond;
                }
            }
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
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= candidates.len() {
                    match game.fuse_companions(first, candidates[idx - 1].entity) {
                        Ok(()) => self.status_line = None,
                        Err(e) => self.status_line = Some(e),
                    }
                    self.pending_fuse_first = None;
                    self.mode = Mode::Playing;
                }
            }
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
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= structures.len() {
                    self.pending_trade_structure = Some(structures[idx - 1].entity);
                    self.mode = Mode::TradeAction;
                }
            }
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
        if let KeyCode::Char(c) = code {
            if let Some(idx) = c.to_digit(10) {
                let idx = idx as usize;
                if idx >= 1 && idx <= total {
                    let choice = if idx <= sell_items.len() {
                        TradeChoice::Sell(sell_items[idx - 1])
                    } else {
                        TradeChoice::Buy(buy_items[idx - 1 - sell_items.len()])
                    };
                    self.pending_trade_choice = Some(choice);
                    self.trade_quantity_input.clear();
                    self.mode = Mode::TradeQuantity;
                }
            }
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
            KeyCode::Char(c) if c.is_ascii_digit() => {
                if self.trade_quantity_input.len() < 4 {
                    self.trade_quantity_input.push(c);
                }
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
        let Some(game) = &mut self.game else { return };
        let KeyCode::Char(c) = code else { return };
        let Some(idx) = c.to_digit(10) else { return };
        let idx = idx as usize;
        let perks = feral_processes_engine::Perk::all();
        if idx >= 1 && idx <= perks.len() {
            match game.unlock_perk(perks[idx - 1]) {
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
        let Some(game) = &mut self.game else { return };
        let KeyCode::Char(c) = code else { return };
        let Some(idx) = c.to_digit(10) else { return };
        let slot = match idx {
            1 => Some(EquipmentSlot::Weapon),
            2 => Some(EquipmentSlot::Armor),
            3 => Some(EquipmentSlot::Module),
            _ => None,
        };
        if let Some(slot) = slot {
            match game.unequip(slot) {
                Ok(()) => self.status_line = None,
                Err(e) => self.status_line = Some(e),
            }
            return;
        }
        if idx >= 4 {
            let inventory = game.player_status().inventory;
            if let Some(&(item, _)) = inventory.get((idx - 4) as usize) {
                self.pending_inventory_item = Some(item);
                self.mode = Mode::InventoryItemAction;
            }
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
        let Some(game) = &mut self.game else { return };
        let stack_qty = game
            .player_status()
            .inventory
            .iter()
            .find(|(i, _)| *i == item)
            .map(|(_, q)| *q)
            .unwrap_or(0);
        let result = match code {
            KeyCode::Char('e') | KeyCode::Char('E') if item.equipment().is_some() => {
                Some(game.equip(item))
            }
            KeyCode::Char('d') | KeyCode::Char('D') => Some(game.drop_item(item, stack_qty)),
            KeyCode::Char('x') | KeyCode::Char('X') => Some(game.destroy_item(item, stack_qty)),
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
            if event::poll(Duration::from_millis(200))? {
                if let Event::Key(key) = event::read()? {
                    if key.kind == KeyEventKind::Press {
                        self.handle_key(key.code);
                    }
                }
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
    let save_path = repo_root.join("save.bin");
    let history_path = repo_root.join("run_history.log");

    let mut terminal = ratatui::init();
    let mut app = App::new(assets_dir, save_path, history_path);
    let result = app.run(&mut terminal);
    ratatui::restore();
    result
}
