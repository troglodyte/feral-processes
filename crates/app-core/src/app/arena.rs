//! The dev arena: authoring a battle scenario and playing it in the real
//! battle UI.
//!
//! Every screen in the family mutates one `ArenaSession` and they are
//! meaningless apart, so they share a module the way `app/trade.rs`'s do.
//!
//! What is being edited is a `feral_processes_engine::arena::Scenario` —
//! the same struct the `.ron` holds and the `arena` bin runs. There is no
//! parallel builder type, so a knob added to the schema cannot exist in one
//! tool and not the other.

use std::path::Path;

use feral_processes_engine::abilities::AbilityDb;
use feral_processes_engine::arena::{
    self, CompanionSpec, EquipSpec, InventorySpec, OpponentSpec, PlayerSource, RepRecord, Scenario,
    Watch,
};
use feral_processes_engine::items_db::ItemDb;
use feral_processes_engine::species::SpeciesDb;
use feral_processes_engine::tuning::{CREATURE_MAX_LEVEL, MAX_ENEMY_GROUPS, MAX_GROUP_SIZE};

use crate::*;

/// What the pickers choose from.
///
/// Loaded when the arena screen opens rather than in `App::new`, for two
/// reasons: the screen is reachable from the main menu where there is no
/// `Game` to ask for `species_defs()`, and a normal run must not pay for a
/// dev tool. It dies with the session.
///
/// Both dbs skip a malformed file with a warning rather than panicking —
/// the same warn-and-carry-on contract `App::new` already uses for
/// `AchievementDb` — so a missing directory reads as an empty picker.
pub(crate) struct ArenaCatalog {
    species: Vec<String>,
    /// Every item id, and whether it may be worn. Read off
    /// `ItemDef::equipment` directly: there is no `Game` here to ask
    /// `is_equippable`.
    items: Vec<(String, bool)>,
}

impl ArenaCatalog {
    fn load(assets_dir: &Path) -> Self {
        let (abilities, _) = AbilityDb::load_dir(&assets_dir.join("abilities")).unwrap_or_default();
        let (species_db, _) =
            SpeciesDb::load_dir(&assets_dir.join("species"), &abilities).unwrap_or_default();
        let (item_db, _) = ItemDb::load_dir(&assets_dir.join("items")).unwrap_or_default();

        let mut species: Vec<String> = species_db.all().map(|d| d.id.to_string()).collect();
        species.sort();
        let mut items: Vec<(String, bool)> = item_db
            .all()
            .map(|d| (d.id.as_str().to_string(), d.equipment.is_some()))
            .collect();
        items.sort();
        Self { species, items }
    }
}

/// Where a picked id goes. `Some(index)` replaces an existing row's id,
/// `None` appends a new one — one mode with four targets rather than four
/// near-identical handlers to keep in step, following `Mode::ManifestPick`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArenaPickKind {
    PartySpecies(Option<usize>),
    OpponentSpecies(Option<usize>),
    EquipItem(Option<usize>),
    InventoryItem(Option<usize>),
}

/// One row of the builder: what it draws as, and what it edits.
pub struct ArenaRow {
    pub label: String,
    pub kind: ArenaRowKind,
}

/// What a builder row edits. The index in a spec row is its position in the
/// scenario's own `Vec`, so Backspace and the picker both act on the row
/// under the highlight rather than on a row the renderer guessed at.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ArenaRowKind {
    PlayerSource,
    PlayerLevel,
    PlayerZone,
    Equip(usize),
    AddEquip,
    Inventory(usize),
    AddInventory,
    Party(usize),
    AddParty,
    Opponent(usize),
    AddOpponent,
    Reps,
    Seed,
}

/// One visit to the arena: the scenario being edited, the fight it is
/// currently staging, and what that fight cost.
///
/// Held as `App::arena: Option<ArenaSession>`, and the presence of that
/// `Option` is what makes the session inert on disk — see `App::in_arena`.
pub(crate) struct ArenaSession {
    pub(crate) scenario: Scenario,
    /// The seed of the current or next fight. Separate from
    /// `scenario.seed`, which is where the *next visit* would start:
    /// reseeding on the result screen is the manual version of a rep, and
    /// must not rewrite the file the author is building.
    pub(crate) seed: u64,
    pub(crate) watch: Option<Watch>,
    pub(crate) outcome: Option<RepRecord>,
    /// What staging said the composition asks for past the zone's ceilings.
    /// Kept for the result screen — nothing is ever capped, and showing the
    /// ask is the only thing that makes that honest.
    pub(crate) warnings: Vec<String>,
    catalog: ArenaCatalog,
}

impl ArenaSession {
    fn new(assets_dir: &Path) -> Self {
        let scenario = Scenario::default();
        Self {
            seed: scenario.seed,
            scenario,
            watch: None,
            outcome: None,
            warnings: Vec::new(),
            catalog: ArenaCatalog::load(assets_dir),
        }
    }
}

/// How the game reaches the launcher's template library.
///
/// `dev_template` lives in the launcher crate — the `arena` bin lives there
/// for the same reason — and app-core cannot see it, so the launcher injects
/// what the builder needs. A plain `fn` rather than a boxed closure because
/// `dev_template::resolve` captures nothing. Absent, the `Template` player
/// source is simply not offered.
pub struct DevTemplates {
    pub names: Vec<String>,
    pub resolve: fn(&str) -> Result<PathBuf, String>,
}

/// A number nudged by `delta` and held inside `[min, max]`. Saturating on
/// both sides, so holding a key down cannot wrap a count to zero.
fn step(value: u32, delta: i32, min: u32, max: u32) -> u32 {
    value.saturating_add_signed(delta).clamp(min, max)
}

/// How a player source reads on the builder's first row. Also the identity
/// the source cycle compares by, so a `Save` — which is not in the cycle —
/// matches nothing and steps off onto `Fresh`.
fn player_source_label(source: &PlayerSource) -> String {
    match source {
        PlayerSource::Fresh { .. } => "Fresh".to_string(),
        PlayerSource::Save(path) => format!(
            "Save({})",
            path.file_name()
                .map(|n| n.to_string_lossy().into_owned())
                .unwrap_or_else(|| path.display().to_string())
        ),
        PlayerSource::Template(name) => format!("Template({name})"),
    }
}

/// Whether `FERAL_DEV_ARENA` is set — the switch that puts the arena row on
/// the main menu. Same predicate as the engine's `FERAL_DEV_REVEAL`
/// (`game/stack_view.rs`): present, non-empty and not `"0"`. Two answers to
/// "is a dev flag set" is exactly the drift this repo keeps catching.
///
/// Read once, in `App::new`, into a field — so a test can open the gate
/// without touching an environment the parallel suite shares.
pub(crate) fn dev_arena_enabled() -> bool {
    std::env::var_os("FERAL_DEV_ARENA").is_some_and(|v| !v.is_empty() && v != "0")
}

impl App {
    /// Installed by the launcher right after `App::new`, unconditionally:
    /// the gate decides visibility, and installing only when gated would
    /// make one flag mean two things.
    pub fn install_dev_templates(&mut self, templates: DevTemplates) {
        self.dev_templates = Some(templates);
    }

    /// Whether the main menu offers the arena at all.
    pub fn arena_enabled(&self) -> bool {
        self.arena_enabled
    }

    /// Whether an arena session is live — the one predicate every "an arena
    /// fight must not do this" rule reads. A named predicate rather than
    /// three ad-hoc `arena.is_some()` checks, because what has to stay true
    /// is a list and only a name makes the list checkable.
    pub(crate) fn in_arena(&self) -> bool {
        self.arena.is_some()
    }

    /// Stages `session.scenario` at `session.seed` and opens `Mode::Battle`
    /// — the real battle UI, entire, which is how a companion Special ever
    /// fires in a measured fight. A staging error stays on the builder with
    /// the reason in the status line.
    pub(crate) fn start_arena_fight(&mut self) {
        let Some(session) = &self.arena else { return };
        let seed = session.seed;
        // A **clone**: a `Template` source is resolved to the save it
        // generates only for the fight. The session's own scenario keeps
        // saying `Template(name)`, or saving it back out would rewrite the
        // author's file into a path under `saves/`.
        let mut scenario = session.scenario.clone();
        if let PlayerSource::Template(name) = &scenario.player {
            let Some(templates) = &self.dev_templates else {
                self.status_line = Some(format!(
                    "template `{name}` needs the launcher's template library"
                ));
                return;
            };
            match (templates.resolve)(name) {
                Ok(path) => scenario.player = PlayerSource::Save(path),
                Err(e) => {
                    self.status_line = Some(e);
                    return;
                }
            }
        }

        match arena::stage(&scenario, &self.assets_dir, seed) {
            Ok(staged) => {
                if let Some(session) = &mut self.arena {
                    session.warnings = staged.warnings;
                    session.watch = Some(staged.watch);
                    session.outcome = None;
                }
                // Shown, never applied: nothing about the composition is
                // capped, and showing the ask is what makes that honest.
                self.status_line = self
                    .arena
                    .as_ref()
                    .filter(|s| !s.warnings.is_empty())
                    .map(|s| format!("warning: {}", s.warnings.join(" · ")));
                self.game = Some(staged.game);
                self.restart_reveal();
                self.mode = Mode::Battle;
            }
            Err(e) => self.status_line = Some(e),
        }
    }

    /// Feeds the round just resolved to the session's `Watch`. Called from
    /// `settle_after_round`, so every action that can end a battle reports
    /// through one hook rather than each remembering to.
    pub(crate) fn observe_arena_round(&mut self) {
        let App { arena, game, .. } = self;
        if let (Some(session), Some(game)) = (arena.as_mut(), game.as_ref())
            && let Some(watch) = &mut session.watch
        {
            watch.observe(game);
        }
    }

    /// Closes the fight and opens the result screen. Reached both from
    /// `settle_after_round` and from `check_game_over`, since a Permadeath
    /// player brought in on a `Save` source can end the run mid-round.
    pub(crate) fn finish_arena_fight(&mut self) {
        let App { arena, game, .. } = self;
        if let (Some(session), Some(game)) = (arena.as_mut(), game.as_ref())
            && let Some(watch) = &session.watch
        {
            session.outcome = Some(watch.finish(game));
        }
        // The result screen is not a log pane, and its transcript is
        // scrolled rather than paced — so release the narration rather than
        // spending the player's first key on skipping it.
        self.finish_reveal();
        self.mode = Mode::ArenaResult;
    }

    pub(crate) fn open_arena(&mut self) {
        self.status_line = None;
        self.arena = Some(ArenaSession::new(&self.assets_dir));
        self.mode = Mode::ArenaBuilder;
    }

    /// Esc drops the session rather than parking it: a scenario outliving
    /// its screen would be the one the next visit silently fought.
    fn leave_arena(&mut self) {
        self.arena = None;
        self.status_line = None;
        self.mode = Mode::MainMenu;
    }

    /// The one source of both the rows the handler dispatches against and
    /// the labels gui draws.
    ///
    /// **Rows are hidden dynamically**, which is the whole reason this is a
    /// function: `equip`, `inventory` and `party` are an *error* on a save
    /// or a template rather than being ignored, so those rows disappear
    /// when the source is not `Fresh`. A renderer that rebuilt the list
    /// from a static table would be right until the first hidden row and
    /// then draw a different row from the one under the highlight.
    pub fn arena_builder_rows(&self) -> Vec<ArenaRow> {
        let Some(session) = &self.arena else {
            return Vec::new();
        };
        let s = &session.scenario;
        let row = |label: String, kind: ArenaRowKind| ArenaRow { label, kind };
        let mut rows = vec![row(
            format!("Player: {}", player_source_label(&s.player)),
            ArenaRowKind::PlayerSource,
        )];

        if let PlayerSource::Fresh { level, zone } = &s.player {
            rows.push(row(format!("  Level: {level}"), ArenaRowKind::PlayerLevel));
            rows.push(row(format!("  Zone: {zone}"), ArenaRowKind::PlayerZone));
            for (i, e) in s.equip.iter().enumerate() {
                rows.push(row(
                    format!("  Worn: {} T{}", e.item.as_str(), e.tier),
                    ArenaRowKind::Equip(i),
                ));
            }
            rows.push(row("  + wear an item".into(), ArenaRowKind::AddEquip));
            for (i, r) in s.inventory.iter().enumerate() {
                rows.push(row(
                    format!("  Cargo: {} x{}", r.item.as_str(), r.qty),
                    ArenaRowKind::Inventory(i),
                ));
            }
            rows.push(row("  + carry an item".into(), ArenaRowKind::AddInventory));
            for (i, c) in s.party.iter().enumerate() {
                rows.push(row(
                    format!("  Party: {} Lv{}", c.species, c.level),
                    ArenaRowKind::Party(i),
                ));
            }
            rows.push(row("  + field a program".into(), ArenaRowKind::AddParty));
        }

        for (i, o) in s.opponents.iter().enumerate() {
            rows.push(row(
                format!("Against: {} x{}", o.species, o.count),
                ArenaRowKind::Opponent(i),
            ));
        }
        // Hidden at the ceiling rather than offered and refused:
        // `build_opponents` hard-errors past `MAX_ENEMY_GROUPS`, so a fifth
        // group is not a composition the game can represent at all.
        if s.opponents.len() < MAX_ENEMY_GROUPS {
            rows.push(row(
                "+ add an opponent group".into(),
                ArenaRowKind::AddOpponent,
            ));
        }
        rows.push(row(format!("Reps: {}", s.reps), ArenaRowKind::Reps));
        rows.push(row(format!("Seed: {}", s.seed), ArenaRowKind::Seed));
        rows
    }

    /// Which row the highlight is on, clamped to the list as it stands now
    /// — removing the last row must not leave the cursor past the end.
    fn arena_highlighted(&self) -> Option<ArenaRowKind> {
        let rows = self.arena_builder_rows();
        rows.get(self.menu_selected.min(rows.len().saturating_sub(1)))
            .map(|r| r.kind)
    }

    pub(crate) fn handle_arena_builder_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.leave_arena();
                return;
            }
            // Its own key rather than Enter, which the rows need for
            // "edit this one" — one key meaning both editing and fighting
            // would be ambiguous on every row.
            GameKey::Char('f') => {
                self.start_arena_fight();
                return;
            }
            GameKey::Left => {
                self.adjust_arena_row(-1);
                return;
            }
            GameKey::Right => {
                self.adjust_arena_row(1);
                return;
            }
            GameKey::Backspace => {
                self.remove_arena_row();
                return;
            }
            GameKey::Char('l') => {
                self.mode = Mode::ArenaLoad;
                return;
            }
            GameKey::Char('s') => {
                self.arena_save_input.clear();
                self.mode = Mode::ArenaSave;
                return;
            }
            GameKey::Enter if self.open_arena_picker() => return,
            _ => {}
        }
        let rows = self.arena_builder_rows().len();
        self.scroll(key, rows);
    }

    /// Enter on a spec row or an `Add …`: opens the picker aimed at it.
    /// `false` for a row with no id to choose, which falls through to
    /// Up/Down so Enter is never a dead key on the wrong row.
    fn open_arena_picker(&mut self) -> bool {
        let Some(kind) = self.arena_highlighted() else {
            return false;
        };
        let pick = match kind {
            ArenaRowKind::Party(i) => ArenaPickKind::PartySpecies(Some(i)),
            ArenaRowKind::AddParty => ArenaPickKind::PartySpecies(None),
            ArenaRowKind::Opponent(i) => ArenaPickKind::OpponentSpecies(Some(i)),
            ArenaRowKind::AddOpponent => ArenaPickKind::OpponentSpecies(None),
            ArenaRowKind::Equip(i) => ArenaPickKind::EquipItem(Some(i)),
            ArenaRowKind::AddEquip => ArenaPickKind::EquipItem(None),
            ArenaRowKind::Inventory(i) => ArenaPickKind::InventoryItem(Some(i)),
            ArenaRowKind::AddInventory => ArenaPickKind::InventoryItem(None),
            _ => return false,
        };
        self.pending_arena_pick = Some(pick);
        self.mode = Mode::ArenaPick;
        true
    }

    /// The picker's rows — species or items, depending on what opened it.
    ///
    /// Ids rather than display names, deliberately: this is a dev tool and
    /// an id is what the `.ron` will hold, so what you picked and what the
    /// file says are the same string.
    pub fn arena_pick_rows(&self) -> Vec<String> {
        let (Some(session), Some(pick)) = (&self.arena, self.pending_arena_pick) else {
            return Vec::new();
        };
        match pick {
            ArenaPickKind::PartySpecies(_) | ArenaPickKind::OpponentSpecies(_) => {
                session.catalog.species.clone()
            }
            // Filtered by what the target can hold. There is no `Game` here
            // to ask `is_equippable`, so the catalogue carries the answer.
            ArenaPickKind::EquipItem(_) => session
                .catalog
                .items
                .iter()
                .filter(|(_, equippable)| *equippable)
                .map(|(id, _)| id.clone())
                .collect(),
            ArenaPickKind::InventoryItem(_) => session
                .catalog
                .items
                .iter()
                .map(|(id, _)| id.clone())
                .collect(),
        }
    }

    /// Nudges the number on the highlighted row, or cycles the player
    /// source.
    ///
    /// Everything is clamped to what the engine can represent, and
    /// deliberately **not** to the zone's ceilings: those warn rather than
    /// cap, and clamping them would delete the tool's main question.
    fn adjust_arena_row(&mut self, delta: i32) {
        let Some(kind) = self.arena_highlighted() else {
            return;
        };
        if kind == ArenaRowKind::PlayerSource {
            self.cycle_arena_player_source(delta);
            return;
        }
        let Some(session) = &mut self.arena else {
            return;
        };
        let s = &mut session.scenario;
        match kind {
            ArenaRowKind::PlayerLevel => {
                if let PlayerSource::Fresh { level, .. } = &mut s.player {
                    *level = step(*level, delta, 1, u32::MAX);
                }
            }
            ArenaRowKind::PlayerZone => {
                if let PlayerSource::Fresh { zone, .. } = &mut s.player {
                    *zone = step(*zone, delta, 1, u32::MAX);
                }
            }
            ArenaRowKind::Equip(i) => {
                if let Some(e) = s.equip.get_mut(i) {
                    e.tier = step(e.tier, delta, 0, MAX_FUSIONS);
                }
            }
            ArenaRowKind::Inventory(i) => {
                if let Some(r) = s.inventory.get_mut(i) {
                    r.qty = step(r.qty, delta, 1, u32::MAX);
                }
            }
            ArenaRowKind::Party(i) => {
                if let Some(c) = s.party.get_mut(i) {
                    c.level = step(c.level, delta, 1, CREATURE_MAX_LEVEL);
                }
            }
            ArenaRowKind::Opponent(i) => {
                if let Some(o) = s.opponents.get_mut(i) {
                    o.count = step(o.count, delta, 1, MAX_GROUP_SIZE);
                }
            }
            ArenaRowKind::Reps => s.reps = step(s.reps, delta, 1, u32::MAX),
            ArenaRowKind::Seed => s.seed = s.seed.saturating_add_signed(delta as i64),
            _ => {}
        }
        session.seed = session.scenario.seed;
    }

    /// Cycles `Fresh` and each installed template. `Save` is deliberately
    /// not in the cycle — it names a file, which is something you load
    /// rather than something you nudge — so a scenario that arrived holding
    /// one steps off it onto `Fresh`.
    fn cycle_arena_player_source(&mut self, delta: i32) {
        let mut options = vec![PlayerSource::Fresh { level: 1, zone: 1 }];
        if let Some(templates) = &self.dev_templates {
            options.extend(
                templates
                    .names
                    .iter()
                    .map(|n| PlayerSource::Template(n.clone())),
            );
        }
        let Some(session) = &mut self.arena else {
            return;
        };
        let here = options
            .iter()
            .position(|o| player_source_label(o) == player_source_label(&session.scenario.player));
        let next = match here {
            Some(i) => (i as i32 + delta).rem_euclid(options.len() as i32) as usize,
            None => 0,
        };
        session.scenario.player = options[next].clone();
        // The loadout is `Fresh`-only, and `Scenario`'s own validation
        // refuses one beside a save or template — so rows that have just
        // gone off screen have to leave the struct too, or a save would
        // write a file that will not load back.
        if !matches!(session.scenario.player, PlayerSource::Fresh { .. }) {
            session.scenario.equip.clear();
            session.scenario.inventory.clear();
            session.scenario.party.clear();
        }
    }

    /// Backspace on a spec row. The `Add …` rows and the numbers have
    /// nothing to remove.
    fn remove_arena_row(&mut self) {
        let Some(kind) = self.arena_highlighted() else {
            return;
        };
        let Some(session) = &mut self.arena else {
            return;
        };
        let s = &mut session.scenario;
        match kind {
            ArenaRowKind::Equip(i) if i < s.equip.len() => {
                s.equip.remove(i);
            }
            ArenaRowKind::Inventory(i) if i < s.inventory.len() => {
                s.inventory.remove(i);
            }
            ArenaRowKind::Party(i) if i < s.party.len() => {
                s.party.remove(i);
            }
            // The last opponent group stays: `Scenario::validate` refuses a
            // scenario with nobody to fight, so an empty list is a file that
            // cannot be saved and reloaded.
            ArenaRowKind::Opponent(i) if s.opponents.len() > 1 && i < s.opponents.len() => {
                s.opponents.remove(i);
            }
            _ => {}
        }
    }

    /// One picker, four targets. Esc adds nothing and goes back.
    ///
    /// A row *replaces* an existing id rather than rebuilding the spec —
    /// the count, quantity, level or tier beside it is the tuning dial, and
    /// losing it on an id change is the bug this shape exists to avoid.
    pub(crate) fn handle_arena_pick_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.pending_arena_pick = None;
            self.mode = Mode::ArenaBuilder;
            return;
        }
        let rows = self.arena_pick_rows();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let id = rows[idx].clone();
        let Some(pick) = self.pending_arena_pick.take() else {
            return;
        };
        if let Some(session) = &mut self.arena {
            let s = &mut session.scenario;
            match pick {
                ArenaPickKind::PartySpecies(Some(i)) if i < s.party.len() => {
                    s.party[i].species = id
                }
                ArenaPickKind::PartySpecies(_) => s.party.push(CompanionSpec {
                    species: id,
                    level: 1,
                }),
                ArenaPickKind::OpponentSpecies(Some(i)) if i < s.opponents.len() => {
                    s.opponents[i].species = id
                }
                ArenaPickKind::OpponentSpecies(_) => s.opponents.push(OpponentSpec {
                    species: id,
                    count: 1,
                }),
                ArenaPickKind::EquipItem(Some(i)) if i < s.equip.len() => {
                    s.equip[i].item = ItemId::from(id.as_str())
                }
                ArenaPickKind::EquipItem(_) => s.equip.push(EquipSpec {
                    item: ItemId::from(id.as_str()),
                    tier: 0,
                }),
                ArenaPickKind::InventoryItem(Some(i)) if i < s.inventory.len() => {
                    s.inventory[i].item = ItemId::from(id.as_str())
                }
                ArenaPickKind::InventoryItem(_) => s.inventory.push(InventorySpec {
                    item: ItemId::from(id.as_str()),
                    qty: 1,
                }),
            }
        }
        self.mode = Mode::ArenaBuilder;
    }

    /// Every `*.ron` in the arenas directory, by name, alphabetically. A
    /// missing directory reads as nothing to offer rather than an error —
    /// the same contract `App::list_saves` makes.
    ///
    /// The one source of both the row count this scrolls against and the
    /// rows gui draws.
    pub fn arena_load_rows(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.arenas_dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .flatten()
            .map(|e| e.path())
            .filter(|p| p.extension().is_some_and(|ext| ext == "ron"))
            .filter_map(|p| p.file_stem().map(|s| s.to_string_lossy().into_owned()))
            .collect();
        names.sort();
        names
    }

    pub(crate) fn handle_arena_load_key(&mut self, key: GameKey) {
        if key == GameKey::Esc {
            self.mode = Mode::ArenaBuilder;
            return;
        }
        let rows = self.arena_load_rows();
        let Some(idx) = self.selected_index(key, rows.len()) else {
            return;
        };
        let path = self.arenas_dir.join(format!("{}.ron", rows[idx]));
        match Scenario::load(&path) {
            Ok(scenario) => {
                if let Some(session) = &mut self.arena {
                    session.seed = scenario.seed;
                    session.scenario = scenario;
                    session.outcome = None;
                    session.watch = None;
                    session.warnings.clear();
                }
                self.status_line = None;
                self.mode = Mode::ArenaBuilder;
            }
            // `Scenario::load` already names the file it choked on, so this
            // stays on the picker rather than dropping the author back on a
            // builder that did not change.
            Err(e) => self.status_line = Some(e),
        }
    }

    /// Typing a filename for the open scenario. The text-input idiom
    /// `Mode::FuseName` uses, plus the one rule a filename needs that a
    /// program's name does not: it must be a name, never a path.
    pub(crate) fn handle_arena_save_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.arena_save_input.clear();
                self.status_line = None;
                self.mode = Mode::ArenaBuilder;
            }
            GameKey::Backspace => {
                self.arena_save_input.pop();
            }
            GameKey::Char(c) if !c.is_control() => self.arena_save_input.push(c),
            GameKey::Enter => self.save_arena_scenario(),
            _ => {}
        }
    }

    fn save_arena_scenario(&mut self) {
        let name = self.arena_save_input.clone();
        // The same rule `dev_template::source` applies, for the same
        // reason: there is no legitimate scenario whose name needs a
        // separator, and rejecting them is what keeps a filename inside
        // the directory it was typed for.
        if name.is_empty()
            || name == "."
            || name == ".."
            || name.contains('/')
            || name.contains('\\')
        {
            self.status_line = Some(format!("`{name}` is not a scenario name"));
            return;
        }
        let Some(session) = &self.arena else { return };
        let path = self.arenas_dir.join(format!("{name}.ron"));
        match session.scenario.save(&path) {
            Ok(()) => {
                self.arena_save_input.clear();
                self.status_line = Some(format!("Wrote {}", path.display()));
                self.mode = Mode::ArenaBuilder;
            }
            Err(e) => self.status_line = Some(e),
        }
    }

    /// What the fight cost, and the two ways to run it again.
    ///
    /// `[N]` is the manual version of `reps`: `arena::run` runs rep *n* at
    /// `scenario.seed + n`, so the same increment here is what lets a loss
    /// found by hand be replayed by the headless bin. It moves the
    /// session's seed and never `scenario.seed`, which is where the *next*
    /// visit starts and what a save writes out.
    ///
    /// Both refights go back through `start_arena_fight`, so there is one
    /// staging path and a refight starts from a whole party rather than
    /// from this fight's corpses.
    pub(crate) fn handle_arena_result_key(&mut self, key: GameKey) {
        match key {
            GameKey::Esc => {
                self.mode = Mode::ArenaBuilder;
                return;
            }
            GameKey::Char('r') => {
                self.start_arena_fight();
                return;
            }
            GameKey::Char('n') => {
                if let Some(session) = &mut self.arena {
                    session.seed = session.seed.wrapping_add(1);
                }
                self.start_arena_fight();
                return;
            }
            _ => {}
        }
        // Everything else scrolls the transcript, which is the screen.
        let rows = self.arena_transcript().len();
        self.scroll(key, rows);
    }

    /// The round-by-round narration the `Watch` collected, empty until a
    /// fight has finished. The one source of both the row count this
    /// scrolls against and the rows gui draws.
    pub fn arena_transcript(&self) -> &[String] {
        self.arena
            .as_ref()
            .and_then(|s| s.outcome.as_ref())
            .map(|r| r.transcript.as_slice())
            .unwrap_or_default()
    }
}
