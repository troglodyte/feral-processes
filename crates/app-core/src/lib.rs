//! Shared, renderer-agnostic game-flow state machine.
//!
//! This crate owns `App`/`Mode` — what pressing a key does in a given
//! screen, save/load orchestration, autosave pacing — but knows nothing
//! about terminals or windows. The frontend (currently just
//! `feral-processes-gui`) translates its own input events into `GameKey` and
//! calls `App::handle_key`, then reads `App`'s public fields to render
//! however it likes.

mod app;

pub use app::arena::{ArenaRow, ArenaRowKind, DevTemplates};
pub use app::dev_console::{DEV_CONSOLE_KEY, DEV_CONSOLE_TICKS, DevAction, DevConsoleRow};
pub use app::group_menu::GroupMenuRow;

use app::arena::{ArenaPickKind, ArenaSession};

use std::path::PathBuf;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use feral_processes_engine::achievements::{AchievementDb, Profile};
use feral_processes_engine::battle::SpecialTargeting;
use feral_processes_engine::battle::{
    ActionKind, BattleAction, PartyCommandKind, SpecialTarget, TargetSpec,
};
use feral_processes_engine::items::{EquipmentSlot, EquipmentStats, ItemId};
use feral_processes_engine::tuning::{ITEM_FUSION_BONUS_PER_TIER, ITEM_FUSION_COST, MAX_FUSIONS};
use feral_processes_engine::{
    AchievementRow, BattleView, DifficultyMode, Entity, EntityView, FieldCastPick, FieldCastTarget,
    Game, LogLine, MESSAGE_LOG_CAP, MessageSource, ProgramSaleOption, RoutineHolderView, SlotShift,
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
    let summary = stat_summary(mods);
    if !summary.is_empty() {
        parts.push(summary);
    }
    if fusion_tier > 0 {
        let maxed = if fusion_tier >= MAX_FUSIONS {
            " - maxed"
        } else {
            ""
        };
        parts.push(format!("fusion {}{maxed}", item_fusion_note(fusion_tier)));
    }
    format!(" ({})", parts.join(" "))
}

/// An item's fusion depth as the compact note a column has room for —
/// `"T2/3"`, or empty for an unfused item. Gear shares `MAX_FUSIONS` with
/// programs (see `Game::fuse_item`), and this is the one place that
/// ceiling is spelled into a label: the equipped panel, the swap picker's
/// stat column and `equip_preview_tag` all read it, so a retune of the
/// constant cannot leave three literals disagreeing.
///
/// Deliberately no "maxed" wording — `SWAP_STATS_COLUMN` is 20 monospace
/// cells and `+2 ATK +1 DEF T3/3 maxed` is 24. The row colour carries that
/// in the two column sites; `equip_preview_tag` has the room and appends it.
pub fn item_fusion_note(tier: u32) -> String {
    if tier == 0 {
        String::new()
    } else {
        format!("T{tier}/{MAX_FUSIONS}")
    }
}

/// The three equipment stats as one line — `"+4 ATK"`, `"+2 ATK +1 DEF"`,
/// empty when every stat is zero. Signed throughout, so it reads a *change*
/// as naturally as a total: `"-2 ATK +3 DEF"`.
///
/// One formatter rather than three. The inventory list's equip tag, the
/// equipped panel and the swap picker's two stat columns all print exactly
/// this, over the same `scaled_for_level().fused_for_tier()` pair, and three
/// copies of it is how the column that nobody re-reads drifts.
pub fn stat_summary(mods: EquipmentStats) -> String {
    [
        (mods.atk, "ATK"),
        (mods.def, "DEF"),
        (mods.decompiler, "DECOMP"),
    ]
    .into_iter()
    .filter(|(value, _)| *value != 0)
    .map(|(value, name)| format!("{value:+} {name}"))
    .collect::<Vec<_>>()
    .join(" ")
}

/// What one row of the `Mode::EquipSwap` picker does when chosen.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SwapChoice {
    /// Wear this copy instead, sending whatever the slot holds back to
    /// cargo. The `u32` is the copy's fusion tier: two rows can name the
    /// same item and differ only in it.
    Equip(ItemId, u32),
    /// Empty the slot. Offered only when something is actually worn.
    Unequip,
}

/// One row of the gear-swap picker: what it does, and how it draws.
///
/// The handler dispatches `choice` and the renderer draws `label`, both out
/// of one `equip_swap_rows` call — the rows depend on what is in cargo right
/// now, so a renderer that rebuilt the list itself could put the highlight
/// on a different row from the one that fires.
pub struct SwapRow {
    pub choice: SwapChoice,
    pub label: String,
    /// The item's fusion depth, for the renderer's row colour. Carried
    /// rather than re-derived on the far side: this screen's rows are built
    /// here and only drawn there, and a renderer that looked the tier up
    /// itself could colour a row its own label contradicts. Zero on the
    /// unequip row, which stands for no item at all.
    pub fusion_tier: u32,
}

/// How wide the swap picker's name and stat columns are. Padding lives here
/// rather than in the renderer because the labels do — see `SwapRow`.
const SWAP_NAME_COLUMN: usize = 20;
const SWAP_STATS_COLUMN: usize = 20;

/// Every replacement for `slot` the player could put on right now, best
/// first, with the row that empties the slot last. One row per *copy*: a
/// fused Arc Lance and a plain one are two different pieces of gear and
/// each gets its own row, priced at its own tier.
///
/// Each candidate is previewed at the level it *would* equip at, since gear
/// takes the current zone level as it goes on (`Game::equip`), while the
/// worn item is measured at the level it actually remembers. Those two
/// scalings differ on purpose: gear doubles per zone level
/// (`GEAR_LEVEL_GROWTH`), so a spare copy of the weapon you are already
/// wearing is a real upgrade after a breach, and previewing it at the worn
/// copy's level would hide that.
///
/// The sort key sums the three stat deltas, which values a point of DECOMP
/// like a point of ATK. That is a display heuristic for "probably the one
/// you want" and not a claim about what those stats are worth, which is why
/// it is here rather than a weighting in `tuning.rs`.
pub fn equip_swap_rows(game: &Game, slot: EquipmentSlot) -> Vec<SwapRow> {
    let status = game.player_status();
    let worn = match slot {
        EquipmentSlot::Weapon => &status.weapon,
        EquipmentSlot::Armor => &status.armor,
        EquipmentSlot::Module => &status.module,
    };
    let worn_mods = worn
        .as_ref()
        .and_then(|e| {
            game.equipment_of(&e.item)
                .map(|(_, base)| base.scaled_for_level(e.level).fused_for_tier(e.fusion_tier))
        })
        .unwrap_or_default();

    let mut rows: Vec<(i32, String, SwapRow)> = status
        .inventory
        .iter()
        .filter_map(|row| {
            let (item_slot, base) = game.equipment_of(&row.item)?;
            (item_slot == slot).then_some((row, base))
        })
        .map(|(row, base)| {
            let tier = row.tier;
            let mods = base.scaled_for_level(status.zone).fused_for_tier(tier);
            let name = game.item_name(&row.item).to_string();
            let stats = match tier {
                0 => stat_summary(mods),
                tier => format!("{} {}", stat_summary(mods), item_fusion_note(tier)),
            };
            (
                delta_total(mods, worn_mods),
                name.clone(),
                SwapRow {
                    choice: SwapChoice::Equip(row.item.clone(), tier),
                    label: swap_label(&name, &stats, mods, worn_mods),
                    fusion_tier: tier,
                },
            )
        })
        .collect();
    // Descending by gain, then by name so the order never shifts between two
    // openings of the same screen.
    rows.sort_by(|a, b| b.0.cmp(&a.0).then_with(|| a.1.cmp(&b.1)));

    let mut rows: Vec<SwapRow> = rows.into_iter().map(|(_, _, row)| row).collect();
    if worn.is_some() {
        rows.push(SwapRow {
            choice: SwapChoice::Unequip,
            label: swap_label("(Unequip)", "", EquipmentStats::default(), worn_mods),
            fusion_tier: 0,
        });
    }
    rows
}

fn delta_total(mods: EquipmentStats, worn: EquipmentStats) -> i32 {
    (mods.atk - worn.atk) + (mods.def - worn.def) + (mods.decompiler - worn.decompiler)
}

fn swap_label(name: &str, stats: &str, mods: EquipmentStats, worn: EquipmentStats) -> String {
    let delta = stat_summary(EquipmentStats {
        atk: mods.atk - worn.atk,
        def: mods.def - worn.def,
        decompiler: mods.decompiler - worn.decompiler,
    });
    let delta = if delta.is_empty() {
        "no change".to_string()
    } else {
        delta
    };
    format!("{name:<SWAP_NAME_COLUMN$} {stats:<SWAP_STATS_COLUMN$} {delta}")
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
    /// The base menu, opened with `b`. Lists every base errand that is
    /// currently possible and dispatches to its screen — see
    /// `App::base_menu_rows`.
    BaseMenu,
    /// The party menu, opened with `p`. Same shape as `Mode::BaseMenu`, for
    /// everything about the programs you own — see `App::party_menu_rows`.
    PartyMenu,
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
    /// The dev keypad, opened with `DEV_CONSOLE_KEY` when
    /// `FERAL_DEV_CONSOLE` is set. Never reachable in a player's build.
    DevConsole,
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
    /// A read-only detail sheet for one structure, opened when
    /// `Mode::InspectDirection` finds a structure nearer than any creature.
    /// `App::pending_structure_manifest` is the subject. Separate from `Mode::Manifest`
    /// rather than a subject variant of it because the two share almost no
    /// fields — a structure has no HP, level, XP or stats.
    StructureManifest,
    Inventory,
    /// Replacements for one equipment slot, reached by picking that slot on
    /// `Mode::Inventory`. Rows come from `equip_swap_rows`, so the picker
    /// lists exactly what fits and shows what each swap would change. The
    /// row that empties the slot lives in here too — unequipping is one of
    /// the things you might want that slot to become, not a separate errand.
    EquipSwap,
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
    /// Aiming an `AbilityEffect::Jump` at a cell of the frame the party is
    /// standing in. Entered from `Mode::FieldCast` when the chosen row's
    /// `FieldRoutineView::second_pick` is `FieldCastPick::Cell`.
    ///
    /// The cursor (`App::field_cursor`) starts on the party's own cell,
    /// walks with the same keys the map already walks with, and is clamped
    /// to the frame's bounds — an out-of-bounds coordinate is unreachable
    /// rather than lethal. Enter commits, Esc backs out spending nothing,
    /// matching every other second pick.
    FieldCastCell,
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
    /// Every conversion a structure runs, expanded back to raw inputs — see
    /// `Game::recipe_chains`. Read-only, and reference data rather than a
    /// view of the base, so it reads the same underground as it does on the
    /// surface.
    Recipes,
    /// The cross-run achievement profile — every authored rung, earned or
    /// not, with what it pays. Reached from the main menu rather than from a
    /// group menu: the profile is the one thing here that outlives a run, so
    /// it belongs beside New Game rather than inside one.
    Achievements,
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
    /// The dev arena's scenario editor, and the screen the whole family
    /// returns to. Reached from the main menu when `FERAL_DEV_ARENA` is set;
    /// Esc drops the session. Rows come from `App::arena_builder_rows`.
    ArenaBuilder,
    /// Picking a `dev-arenas/*.ron` to load into the builder.
    ArenaLoad,
    /// Typing a filename to write the built scenario out under — the same
    /// text-entry idiom `Mode::FuseName` uses.
    ArenaSave,
    /// One picker, four targets: a party species, an opponent species, an
    /// item to equip or an item for cargo. Which is `App::pending_arena_pick`,
    /// following `Mode::ManifestPick` rather than being four near-identical
    /// modes with four near-identical handlers to keep in step.
    ArenaPick,
    /// What the fight cost — see `arena::Watch`. `[R]` refights the same
    /// seed, `[N]` the next one, Esc returns to the builder.
    ArenaResult,
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
            | Mode::Achievements
            | Mode::DifficultyPick
            | Mode::LoadGame
            | Mode::SaveAction
            | Mode::Playing
            | Mode::BaseMenu
            | Mode::PartyMenu
            // Opened from the map only, so it never layers over a fight.
            | Mode::DevConsole
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
            | Mode::StructureManifest
            | Mode::Inventory
            | Mode::EquipSwap
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
            | Mode::FieldCastCell
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
            | Mode::Recipes
            | Mode::Help
            | Mode::FrameMap
            | Mode::GameOver
            | Mode::QuitRunConfirm
            | Mode::QuitAppConfirm
            // The arena's own screens are not battle screens; the fight it
            // stages runs in `Mode::Battle` like any other.
            | Mode::ArenaBuilder
            | Mode::ArenaLoad
            | Mode::ArenaSave
            | Mode::ArenaPick
            | Mode::ArenaResult => false,
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
/// The `u32` beside a sold or bought-back item is its fusion tier — the
/// player may hold several copies of one item at different tiers, and which
/// one they picked is not recoverable from the id (see
/// `components::FusedGear`). `Buy` carries none because a trader's stock is
/// always ordinary.
#[derive(Clone)]
pub enum TradeChoice {
    Sell(ItemId, u32),
    Buy(ItemId),
    /// Something the player sold this trader, offered back at a markup —
    /// see `Game::buyback_options`.
    BuyBack(ItemId, u32),
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
    /// Where `profile.ron` lives — beside `run_history.log` at the repo root,
    /// not in `saves/`. Both are run-spanning rather than part of one run,
    /// and `list_saves` filters on `.bin` so neither can show up in the save
    /// picker.
    profile_path: PathBuf,
    /// The cross-run profile as last read or written. Held rather than
    /// re-read per use because the achievements screen is reachable from the
    /// main menu, where there is no `Game` to ask.
    profile: Profile,
    /// The authored ladder, for the same reason: the screen lists every rung
    /// including the unearned ones, and with no run in progress the `Game`'s
    /// copy of this db does not exist.
    achievement_db: AchievementDb,
    pub quit: bool,
    /// Which structure *kind* `Mode::BuildDirection` is about to place. Public
    /// because that screen names it: the build menu's row is off screen by
    /// then, so a renderer without this can only draw an anonymous compass.
    pub pending_structure: Option<String>,
    pending_worker: Option<Entity>,
    /// Which group menu opened the screen that is up, if one did — where
    /// `App::close_screen` sends Esc. `None` for a screen reached straight
    /// from the map, and cleared the moment the map is reached again.
    menu_origin: Option<Mode>,
    /// The structure picked in `Mode::Remove`, awaiting confirmation from
    /// `Mode::RemoveConfirm` if it's the Home (see `Game::remove_structure`).
    pending_remove_structure: Option<Entity>,
    /// Whose stat sheet `Mode::Manifest` is showing — the player, a program
    /// you own, or the wild one `Mode::InspectDirection` just found.
    pub pending_manifest: Option<Entity>,
    /// Which structure `Mode::StructureManifest` is showing — whatever the
    /// inspector found in the direction you pointed. Not `pending_structure`:
    /// that one is a structure *kind* awaiting placement in `Mode::Build`.
    pub pending_structure_manifest: Option<Entity>,
    /// Whether `Mode::Manifest` was opened from `Mode::ManifestPick`, which
    /// is where Esc then goes back to. Reached from the map with `x` instead,
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
    /// Where the cell cursor is aimed in `Mode::FieldCastCell`, in frame
    /// coordinates. `None` outside that mode — a routine needing no cell
    /// never sets it, and Esc and a committed jump both clear it.
    pub field_cursor: Option<(i32, i32)>,
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
    /// The cargo row picked on `Mode::Inventory`, as `(item, fusion tier)`
    /// — a fused copy and its ordinary spares are separate rows and every
    /// action on one has to say which it meant.
    pub pending_inventory_item: Option<(ItemId, u32)>,
    /// The equipment slot picked on `Mode::Inventory`, awaiting a
    /// replacement (or an unequip) from `Mode::EquipSwap`.
    pub pending_swap_slot: Option<EquipmentSlot>,
    /// The inventory item picked for erasure, awaiting a quantity from
    /// `Mode::EraseQuantity`.
    pub pending_erase: Option<(ItemId, u32)>,
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
    /// Where `Mode::ArenaLoad` reads scenarios from and `Mode::ArenaSave`
    /// writes them to. A constructor parameter beside `saves_dir` rather
    /// than something derived here: `App` takes its paths from the
    /// launcher and resolves none itself, which is what keeps app-core
    /// testable against a temp directory.
    arenas_dir: PathBuf,
    /// The live arena visit, if the dev arena is open. Its presence is what
    /// makes the session inert on disk — see `App::in_arena`.
    arena: Option<ArenaSession>,
    /// Whether `FERAL_DEV_ARENA` was set when this `App` was built. Read
    /// once, in `App::new`, so the parallel test suite can open the gate on
    /// a field rather than in a process-global environment.
    arena_enabled: bool,
    dev_console: bool,
    /// Where a row picked in `Mode::ArenaPick` is going — see
    /// `ArenaPickKind`. `None` outside that mode.
    pending_arena_pick: Option<ArenaPickKind>,
    /// Characters typed so far on `Mode::ArenaSave`'s filename page.
    pub arena_save_input: String,
    /// The launcher's template library, injected because `dev_template`
    /// lives in a crate app-core cannot see. `None` for any frontend that
    /// does not install it, which simply does not offer the `Template`
    /// player source.
    dev_templates: Option<DevTemplates>,
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
