//! Shared, renderer-agnostic game-flow state machine.
//!
//! This crate owns `App`/`Mode` — what pressing a key does in a given
//! screen, save/load orchestration, autosave pacing — but knows nothing
//! about terminals or windows. The frontend (currently just
//! `feral-processes-gui`) translates its own input events into `GameKey` and
//! calls `App::handle_key`, then reads `App`'s public fields to render
//! however it likes.

mod app;

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use feral_processes_engine::battle::SpecialTargeting;
use feral_processes_engine::battle::{
    ActionKind, BattleAction, PartyCommandKind, SpecialTarget, TargetSpec,
};
use feral_processes_engine::items::{EquipmentSlot, ItemId};
use feral_processes_engine::tuning::{ITEM_FUSION_BONUS_PER_TIER, ITEM_FUSION_COST};
use feral_processes_engine::{
    DifficultyMode, Entity, Game, LogLine, MESSAGE_LOG_CAP, MessageSource, ProgramSaleOption,
    RoutineHolderView, SlotShift,
};

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
/// Whether any structure the player could trade with is close enough to
/// reach from a menu — the same scan `Mode::Trade`'s picker runs, so the
/// two can't disagree about whether selling is possible.
pub fn trader_in_range(game: &mut Game) -> bool {
    !traders_in_range(game).is_empty()
}

/// Every trading post the player could reach from here, in the order
/// `Mode::Trade`'s picker lists them. The quick-sell key needs to know
/// whether there is exactly one, not merely whether there is any — and
/// asking the same scan the picker runs is what keeps "the only trader in
/// range" meaning the trader the picker would have offered.
pub fn traders_in_range(game: &mut Game) -> Vec<Entity> {
    game.view_entities(MENU_SCAN_RADIUS, MENU_SCAN_RADIUS)
        .into_iter()
        .filter(|e| e.can_trade)
        .map(|e| e.entity)
        .collect()
}

/// `[S]ell` is the one action gated on the world rather than the item: it
/// is listed only when a trading post is inside `MENU_SCAN_RADIUS`, and
/// hidden otherwise. That is why this takes `&mut Game` — the scan runs
/// through `Game::view_entities`, and a bevy query needs `&mut World`.
pub fn inventory_item_actions(game: &mut Game, item: &ItemId) -> Vec<(char, String)> {
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
    if trader_in_range(game) {
        actions.push(('s', "[S]ell".to_string()));
    }
    actions.push(('d', "[D]escribe".to_string()));
    actions.push(('x', "[X] Erase".to_string()));
    actions
}

/// Formats the slot an equippable item would occupy plus its stat bonus as it
/// would be *if equipped right now* — gear scales with the current zone level
/// at the moment you equip it (see `Game::equip`), so this previews that same
/// number rather than a flat, unscaled base value. Empty string for a
/// non-equippable item.
///
/// Lives here rather than in either renderer because both draw the identical
/// tag, on both the inventory list and the item-action page.
pub fn equip_preview_tag(game: &Game, item: &ItemId, zone_level: u32, fusion_tier: u32) -> String {
    let Some((slot, base_mods)) = game.equipment_of(item) else {
        return String::new();
    };
    let mods = base_mods
        .scaled_for_level(zone_level)
        .fused_for_tier(fusion_tier);
    let mut parts = vec![slot.short_label().to_string()];
    if mods.atk != 0 {
        parts.push(format!("+{} ATK", mods.atk));
    }
    if mods.def != 0 {
        parts.push(format!("+{} DEF", mods.def));
    }
    if mods.decompiler != 0 {
        parts.push(format!("+{} DECOMP", mods.decompiler));
    }
    if fusion_tier > 0 {
        parts.push(format!("fusion T{fusion_tier}"));
    }
    format!(" ({})", parts.join(" "))
}

/// How many game ticks (see `Game::current_tick`) pass between autosaves —
/// paced against game time rather than wall-clock time, so it's the same
/// whether the player is acting quickly or sitting on a menu.
const AUTOSAVE_INTERVAL_TICKS: u64 = 50;

/// Wall-clock spacing between idle ticks (see `App::update_realtime`) —
/// the world keeps moving once a second even while the player just sits on
/// `Mode::Playing` and touches nothing.
const REALTIME_TICK_INTERVAL: Duration = Duration::from_secs(1);

/// How fast battle narration scrolls into the log pane, in lines per second.
///
/// Presentation rather than difficulty, which is why it lives here and not
/// in the engine's `tuning.rs`.
///
/// A typical round narrates four lines, so this is a round per second — a
/// line landing every quarter second, which reads as arriving rather than
/// as already being there. The first attempt at this was 12/sec, which put
/// the same round on screen in a third of a second and looked instant.
pub const REVEAL_LINES_PER_SECOND: f32 = 4.0;

/// How long a refusal ("that ability isn't ready") stays on screen before
/// clearing itself, in seconds.
///
/// It clears rather than sitting there because it is drawn over the action
/// bar on several screens — so a message about one rejected key would
/// otherwise hide the menu the player needs in order to press a different
/// one.
pub const STATUS_LINE_SECONDS: f32 = 4.0;

/// How much of the current battle's narration the player has been shown.
///
/// Transient presentation state, deliberately not saved: a loaded game
/// resumes with nothing pending.
#[derive(Default)]
struct BattleReveal {
    /// Lines released to the pane so far.
    revealed: usize,
    /// Sub-line carry, so a frame covering less than one line's worth of
    /// time isn't rounded away and lost.
    accumulated: f32,
    /// The `Game::battle_log_generation` this count belongs to. When the
    /// engine's generation moves on, the pane has a fresh range — a new
    /// round or a new battle — and the count restarts.
    generation: u64,
}

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
/// without audio is free to just drop what it drains.
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

/// Which of the log's two channels the map's pane is showing. View state, not
/// game state: not saved, and cycling it costs no turn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LogFilter {
    #[default]
    All,
    Field,
    Base,
}

impl LogFilter {
    /// The cycle the filter key walks. All → Field → Base → All, so one more
    /// press always gets you back to seeing everything.
    pub fn next(self) -> Self {
        match self {
            LogFilter::All => LogFilter::Field,
            LogFilter::Field => LogFilter::Base,
            LogFilter::Base => LogFilter::All,
        }
    }

    pub fn accepts(self, source: MessageSource) -> bool {
        match self {
            LogFilter::All => true,
            LogFilter::Field => source == MessageSource::Field,
            LogFilter::Base => source == MessageSource::Base,
        }
    }

    /// What the pane header calls the active filter.
    pub fn label(self) -> &'static str {
        match self {
            LogFilter::All => "All",
            LogFilter::Field => "Field",
            LogFilter::Base => "Base",
        }
    }

    /// Which channel this filter is suppressing, for the header's count of
    /// what you aren't seeing. `None` when nothing is hidden.
    pub fn hidden_channel(self) -> Option<&'static str> {
        match self {
            LogFilter::All => None,
            LogFilter::Field => Some("base"),
            LogFilter::Base => Some("field"),
        }
    }
}

/// Picks the map log pane's rows out of the retained log: drop the battle
/// results that have not scrolled in yet, apply the filter, then keep the
/// newest `capacity`.
///
/// The order is load-bearing. `hidden` counts *raw* tail lines (see
/// `App::hidden_log_lines`), so it has to come off before the filter thins the
/// list — chopping the same count out of a filtered list would eat lines that
/// had already been revealed. And the filter has to come off before the
/// capacity cut, or a screenful of base chatter would leave the field pane
/// blank while older field lines were still in reach.
///
/// A free function rather than a method so it can be tested against a
/// hand-built log; `App::visible_log` is the one caller that has a `Game`.
pub fn pane_rows(
    lines: &[LogLine],
    hidden: usize,
    filter: LogFilter,
    capacity: usize,
) -> Vec<LogLine> {
    let shown = lines.len().saturating_sub(hidden);
    let mut rows: Vec<LogLine> = lines[..shown]
        .iter()
        .filter(|l| filter.accepts(l.source))
        .cloned()
        .collect();
    if rows.len() > capacity {
        rows.drain(0..rows.len() - capacity);
    }
    rows
}

/// How many of `lines` the filter is holding back — the header's "there is
/// more you aren't seeing" figure. Zero under `LogFilter::All`, so the pane
/// says nothing when there is nothing to say.
pub fn filtered_out_count(lines: &[LogLine], filter: LogFilter) -> usize {
    lines.iter().filter(|l| !filter.accepts(l.source)).count()
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
    /// Picking an action for the party member in `Game::battle_active_slot`.
    /// The menu comes from `Game::battle_action_options`, so the action set
    /// lives in exactly one place and the two renderers cannot drift.
    Battle,
    /// Picking which enemy group the pending action targets. Entered from
    /// `Mode::Battle` when the chosen option has `TargetSpec::EnemyGroup`.
    BattleTarget,
    /// Picking which consumable the pending action spends. Entered from
    /// `Mode::Battle` when the chosen option has `TargetSpec::InventoryItem`.
    /// Distinct from `Mode::Inventory`: on the map an item is spent for
    /// free, in battle it costs that slot its round.
    BattleItem,
    /// Picking which of the acting member's special abilities to spend.
    /// Entered from `Mode::Battle` when the chosen option has
    /// `TargetSpec::SpecialAbility`, and followed by whichever picker that
    /// ability calls for — the ability and its target are separate choices,
    /// so they are separate steps.
    BattleSpecial,
    /// Picking which party member a buff or heal lands on. Entered from
    /// `Mode::BattleSpecial` when the chosen ability is `Ally`-targeted,
    /// where an enemy-targeted one goes to `Mode::BattleTarget` instead.
    BattleAlly,
    Build,
    BuildDirection,
    Craft,
    CraftQuantity,
    Cronjob,
    CronjobStructure,
    /// Picking a nearby structure for the *player* to work themselves rather
    /// than posting a program to it — see `Game::work_structure`. Same
    /// `can_work` list `CronjobStructure` offers.
    WorkStructure,
    Guard,
    GuardStructure,
    /// Lists nearby structures to demolish (see `App::pending_remove_structure`).
    /// Picking the Home moves to `Mode::RemoveConfirm` instead of demolishing
    /// immediately, since it cascades; anything else is removed right away.
    Remove,
    /// Warns that demolishing the Home destroys every other base structure
    /// too, before `Game::remove_structure` is actually called.
    RemoveConfirm,
    /// Lists nearby structures that declare an upgrade path (see
    /// `Game::upgrade_structure`); picking one advances it a tier. Anything
    /// un-upgradeable is filtered out rather than offered and then refused.
    Upgrade,
    Symlink,
    InspectDirection,
    /// The party's own map of the Stack frame they are standing in — see
    /// `Game::frame_map`. Underground only, opened with `g` — a no-op on the
    /// surface, where the two screens never both apply.
    FrameMap,
    /// The manifest — a full read-only stat sheet for the player, a program
    /// you own, or a wild one. `App::pending_manifest` is the subject.
    Manifest,
    /// Picking whose manifest to read — you, or any program you own.
    /// Reached with `d` from `Mode::Playing`.
    ManifestPick,
    Inventory,
    InventoryItemAction,
    /// The authored description of `pending_inventory_item`, read out of its
    /// `.ron` file. Reached with `d` from `Mode::InventoryItemAction`, and
    /// Esc steps back there rather than out to the inventory — it is a page
    /// about the item you already picked, not a separate errand.
    ItemDescribe,
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
    /// Picking whose routines to manage — you, or any program you own.
    /// Reached with `m` from `Mode::Playing`.
    RoutineTarget,
    /// The chosen member's slot list. A filled slot pops its routine back
    /// into cargo; an empty one opens `Mode::RoutineInstall`.
    Routines,
    /// Picking which loose routine to drop into the slot chosen in
    /// `Mode::Routines`.
    RoutineInstall,
    /// Picking which installed field routine to run — a `FieldBuff` ability
    /// on you or a program you own, cast outside battle rather than spent as
    /// a Special. Reached with `a` from `Mode::Playing`; rows come from
    /// `Game::field_routines`. A row with no ally target casts immediately
    /// and returns here to `Mode::Playing`; one that needs an ally instead
    /// goes to `Mode::FieldCastAlly`.
    FieldCast,
    /// Picking who a `OneAlly` field routine lands on. Entered from
    /// `Mode::FieldCast` only when the chosen row's
    /// `FieldRoutineView::needs_ally_target` is set — same split
    /// `Mode::BattleSpecial`/`Mode::BattleAlly` makes, for the same reason:
    /// the routine and its target are separate choices. Offers only the
    /// player and programs the player owns (`App::field_ally_options`),
    /// since `Game::cast_field_routine` checks a target is alive but not
    /// that the player owns it.
    FieldCastAlly,
    /// Picking which program to break down for a routine. Reached with `M`
    /// from `Mode::Playing`.
    Extract,
    /// Picking which of that program's routines to salvage.
    ExtractPick,
    /// Confirming the extraction. Programs take a confirmation for the same
    /// reason a sale does: it is irreversible, and every *other* routine on
    /// the program is lost with it — this screen is the only place that is
    /// said out loud.
    ExtractConfirm,
    Trade,
    TradeAction,
    TradeQuantity,
    /// Confirming the sale of the program picked in `Mode::TradeAction`.
    /// Programs take a confirmation where items don't: the sale is
    /// irreversible, and it silently cancels whatever the program was doing,
    /// which this screen is the only place to say out loud.
    TradeProgramConfirm,
    Perks,
    /// The research tree (see `Game::research_nodes`). Stays open after each
    /// unlock so several nodes can be taken in one visit.
    Research,
    /// The message log in full, scrolled with Up/Down — the map's pane shows
    /// only its last few lines. Read-only, and bounded by what the engine
    /// keeps: `MESSAGE_LOG_CAP` lines, minus the blow-by-blow that
    /// `MessageLog::retain_outcomes_since_battle` drops when a fight ends.
    History,
    /// Every structure in the zone and what is assigned to it — see
    /// `Game::structure_report`. Read-only: assigning and demolishing stay
    /// on their own screens.
    Structures,
    Help,
    GameOver,
    /// Confirming `q` from `Mode::Playing`, which abandons the run. Offers to
    /// save first: autosave only fires every `AUTOSAVE_INTERVAL_TICKS`, so
    /// leaving without one silently drops however many ticks have passed
    /// since — and remembering to press `s` first is not something a
    /// confirmation should require of the player.
    QuitRunConfirm,
    /// Confirming `q` from `Mode::MainMenu`, which ends the process. Nothing
    /// is in memory to lose here; the key simply sits between `n` and `l`.
    QuitAppConfirm,
}

impl Mode {
    /// Whether this screen belongs to an intrusion — the battle roster
    /// itself, or any of the pickers layered over it while it stays drawn
    /// underneath. Renderers use it to keep battle-only state alive across
    /// a popup: the GUI's HP ghost bars and pending damage floats are
    /// discarded the moment this reads false.
    ///
    /// Matched exhaustively on purpose. This began as an inline `matches!`
    /// in the GUI's frame loop and fell behind three times as battle
    /// pickers were added, each time silently wiping the ghost bars
    /// mid-animation. Listing every mode makes a new variant a compile
    /// error until it is classified, rather than a quiet `false`.
    pub fn is_battle(self) -> bool {
        match self {
            Mode::Battle
            | Mode::BattleTarget
            | Mode::BattleItem
            | Mode::BattleSpecial
            | Mode::BattleAlly => true,
            Mode::MainMenu
            | Mode::DifficultyPick
            | Mode::LoadGame
            | Mode::SaveAction
            | Mode::Playing
            | Mode::Build
            | Mode::BuildDirection
            | Mode::Craft
            | Mode::CraftQuantity
            | Mode::Cronjob
            | Mode::CronjobStructure
            | Mode::WorkStructure
            | Mode::Guard
            | Mode::GuardStructure
            | Mode::Remove
            | Mode::RemoveConfirm
            | Mode::Upgrade
            | Mode::Symlink
            | Mode::InspectDirection
            | Mode::Manifest
            | Mode::ManifestPick
            | Mode::Inventory
            | Mode::InventoryItemAction
            | Mode::ItemDescribe
            | Mode::EraseQuantity
            | Mode::Companion
            | Mode::Fuse
            | Mode::FuseSecond
            | Mode::FuseName
            | Mode::RoutineTarget
            | Mode::Routines
            | Mode::RoutineInstall
            | Mode::FieldCast
            | Mode::FieldCastAlly
            | Mode::Extract
            | Mode::ExtractPick
            | Mode::ExtractConfirm
            | Mode::Trade
            | Mode::TradeAction
            | Mode::TradeQuantity
            | Mode::TradeProgramConfirm
            | Mode::Perks
            | Mode::Research
            | Mode::History
            | Mode::Structures
            | Mode::Help
            | Mode::FrameMap
            | Mode::GameOver
            | Mode::QuitRunConfirm
            | Mode::QuitAppConfirm => false,
        }
    }
}

/// Which screen a trade was started from, and therefore which screen
/// finishing or abandoning it returns to.
///
/// A trade begun at the trader's list is one of a run of them and goes back
/// there (see `App::return_to_trade_list`); one begun with `[S]ell` in the
/// inventory has to go back to the inventory, because the trader's list is
/// a screen that player never opened.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum TradeOrigin {
    #[default]
    Trader,
    Inventory,
}

/// A line item picked in `Mode::TradeAction`, awaiting a quantity from
/// `Mode::TradeQuantity` before `Game::sell_item`/`Game::buy_item`/
/// `Game::buy_back` is actually called.
#[derive(Clone)]
pub enum TradeChoice {
    Sell(ItemId),
    Buy(ItemId),
    /// Something the player sold this trader, offered back at a markup —
    /// see `Game::buyback_options`.
    BuyBack(ItemId),
}

pub const MIN_ZOOM: u16 = 1;
pub const MAX_ZOOM: u16 = 4;

/// Zoom levels for the Stack's corner map. The minimum is the whole frame,
/// which is what the inset showed before there was a choice, so a player who
/// never presses `+` sees exactly what they saw before.
///
/// Presentation state, so it lives here beside the zone map's zoom rather
/// than in `tuning.rs` — what the renderer crops to is not difficulty. The
/// cell radius each level maps to is the renderer's business
/// (`render/frame_map.rs::window_radius`).
pub const STACK_MAP_MIN_ZOOM: u16 = 1;
pub const STACK_MAP_MAX_ZOOM: u16 = 4;

/// Builds the `BattleAction` an `ActionKind` becomes once the UI has
/// collected whatever its `TargetSpec` called for. One arm per kind, so a
/// new action is added here rather than by editing the key handlers.
/// `None` means the required target wasn't supplied.
/// Everything the pickers can have collected by the time an `ActionKind`
/// becomes a `BattleAction`. Bundled rather than passed as a row of
/// positional `Option`s, which is what it was becoming.
#[derive(Default, Clone)]
struct Collected {
    group: Option<usize>,
    item: Option<ItemId>,
    ability: Option<usize>,
    /// The party slot a buff or heal was aimed at.
    ally: Option<usize>,
}

fn action_from(kind: ActionKind, c: Collected) -> Option<BattleAction> {
    match kind {
        ActionKind::Attack => Some(BattleAction::Attack { group: c.group? }),
        ActionKind::Special => Some(BattleAction::Special {
            ability: c.ability?,
            // Whichever picker ran second supplies the target; an ability
            // that needs neither never becomes an action.
            target: match (c.ally, c.group) {
                (Some(slot), _) => SpecialTarget::Ally { slot },
                (None, Some(group)) => SpecialTarget::EnemyGroup { group },
                (None, None) => return None,
            },
        }),
        ActionKind::Defend => Some(BattleAction::Defend),
        ActionKind::UseItem => Some(BattleAction::UseItem { item: c.item? }),
    }
}

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
    /// Whose stat sheet `Mode::Manifest` is showing — the player, a program
    /// you own, or the wild one `Mode::InspectDirection` just found.
    pub pending_manifest: Option<Entity>,
    /// Whether `Mode::Manifest` was opened from `Mode::ManifestPick`, which
    /// is where Esc then goes back to. Reached from the map with `i` instead,
    /// there is no list to return to and Esc goes straight back to play.
    pub manifest_from_picker: bool,
    /// Which of the log's two channels the map's pane shows. Cycled with `F`;
    /// see `LogFilter`.
    pub log_filter: LogFilter,
    /// The first program picked in `Mode::Fuse`, awaiting a second from
    /// `Mode::FuseSecond` before `Game::fuse_companions` is actually called.
    pub pending_fuse_first: Option<Entity>,
    /// The second program picked in `Mode::FuseSecond`, awaiting a name
    /// from `Mode::FuseName` before `Game::fuse_companions` is actually
    /// called.
    pub pending_fuse_second: Option<Entity>,
    /// Characters typed so far on the fuse-naming page (see `Mode::FuseName`).
    pub fuse_name_input: String,
    /// The routine holder picked in `Mode::RoutineTarget` — the player or one
    /// of their programs — awaiting a slot pick from `Mode::Routines`.
    pub pending_routine_holder: Option<Entity>,
    /// The program picked in `Mode::Extract`, awaiting a routine pick from
    /// `Mode::ExtractPick`.
    pub pending_extract_program: Option<Entity>,
    /// The routine index picked in `Mode::ExtractPick`, awaiting confirmation
    /// from `Mode::ExtractConfirm` before `Game::extract_routine` is called.
    pub pending_extract_index: Option<usize>,
    /// The index into `Game::field_routines` picked in `Mode::FieldCast`,
    /// awaiting a target from `Mode::FieldCastAlly` before
    /// `Game::cast_field_routine` is called. `None` outside that wait — a
    /// routine needing no ally casts straight from `Mode::FieldCast` and
    /// never sets this.
    pub pending_field_routine: Option<usize>,
    /// The action kind picked in `Mode::Battle`, awaiting an enemy group
    /// from `Mode::BattleTarget` before it becomes a `BattleAction`.
    pub pending_battle_action: Option<ActionKind>,
    /// Set when `Mode::BattleTarget` was opened by the party-wide `[A]ll
    /// attack` rather than by one slot's Attack — the group picked then plans
    /// every open slot instead of just `battle_active_slot`.
    pub pending_party_attack: bool,
    /// The ability index picked in `Mode::BattleSpecial`, awaiting a group
    /// from `Mode::BattleTarget` before it becomes a `BattleAction::Special`.
    pub pending_special_ability: Option<usize>,
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
    /// Which screen the in-flight trade was started from — see
    /// `TradeOrigin`. Set when a trade begins, read when it ends.
    pub trade_origin: TradeOrigin,
    /// The program picked in `Mode::TradeAction`, awaiting confirmation in
    /// `Mode::TradeProgramConfirm`. Holds the whole priced row rather than
    /// just the entity, so the confirmation shows the payout and detach list
    /// the player was actually offered.
    pub pending_trade_program: Option<ProgramSaleOption>,
    /// Digits typed so far on the trade-quantity page.
    pub trade_quantity_input: String,
    /// How many screen characters render each world tile along each axis.
    pub zoom: u16,
    /// How close the Stack's corner map is drawn in: `STACK_MAP_MIN_ZOOM`
    /// shows the whole frame, higher levels a window around the party.
    ///
    /// Its own field rather than a second use of `zoom`, which is the zone
    /// map's tile size: the two are different scales with no sensible
    /// mapping between them, and sharing one would resize the surface after
    /// a dive spent reading the maze.
    pub stack_zoom: u16,
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
    /// Paces battle narration into the log pane — see `App::advance_reveal`.
    reveal: BattleReveal,
    /// Seconds `status_line` has been on screen — see `App::advance_status`.
    /// Reset by every key press, so the window belongs to the most recent
    /// message rather than the first one.
    status_age: f32,
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

#[cfg(test)]
mod tests;
