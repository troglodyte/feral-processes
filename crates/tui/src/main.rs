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
    Help,
    GameOver,
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
    pending_inventory_item: Option<ItemId>,
    /// The recipe result picked in `Mode::Craft`, awaiting a quantity from
    /// `Mode::CraftQuantity` before `Game::craft` is actually called.
    pending_craft: Option<ItemId>,
    /// Digits typed so far on the craft-quantity page.
    craft_quantity_input: String,
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
            pending_inventory_item: None,
            pending_craft: None,
            craft_quantity_input: String::new(),
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
        let still_active = {
            let Some(game) = &mut self.game else { return };
            match code {
                KeyCode::Char('a') => game.battle_attack(),
                KeyCode::Char('d') => game.battle_decompile(),
                KeyCode::Char('c') => game.battle_companion_attack(),
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

    /// Lists nearby tamed programs; pressing the active companion's number
    /// stands it down, pressing any other tamed program's number makes it
    /// the new companion (swapping out whichever was active before).
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
                        game.clear_companion();
                    } else {
                        match game.set_companion(candidate.entity) {
                            Ok(()) => self.status_line = None,
                            Err(e) => self.status_line = Some(e),
                        }
                    }
                    self.mode = Mode::Playing;
                }
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
