//! All drawing for the graphics frontend: one screen per `Mode`, laid out
//! in immediate mode against `Painter` (filled rects for bars and tiles,
//! drawn text for menus). Reads engine data through `App` and never touches
//! the ECS `World`.

use std::borrow::Cow;

use crate::fx::Fx;
use crate::paint::{Color, GRAY, Painter, Rect, TextRun, WHITE};
use crate::text::{Metrics, map_cell, ui_metrics};
use feral_processes_app_core::{
    App, ArenaRow, BattlePane, DevConsoleRow, GearInspect, GroupMenuRow, MENU_SCAN_RADIUS, Mode,
    Staffing, SwapChoice, SwapRow, TradeChoice, equip_preview_tag, equip_swap_rows,
    inventory_item_actions, item_fusion_note, menu_shortcut, qty_column, stat_summary,
};
use feral_processes_engine::components::{GlyphColor, MachineStatus, Rarity, TaskKind};
use feral_processes_engine::items::{EquipmentSlot, GearCopy, ItemId, QualityBand, quality_band};
use feral_processes_engine::structures::StructureCategory;
use feral_processes_engine::tuning::{
    KERNEL_RING_MAX, MAX_COMPANION_REFACTORS, MAX_FUSIONS, MAX_PARTY_SIZE,
};
use feral_processes_engine::world::{Biome, Tile};
use feral_processes_engine::{
    Assignee, BrokerReach, ContractRow, CraftRecipe, Entity, EntityView, Game, InventoryRow,
    LogEntry, MESSAGE_LOG_CAP, MemoryRow, MessageKind, PetInfo, ProgramSaleOption, RecipeChain,
    RecipeStep, ResearchState, StockRow, StructureReport,
};

mod arena;
mod bars;
mod base;
mod battle;
mod building;
mod canvas;
mod caravan;
mod contracts;
mod crafting;
mod creation;
mod field;
mod frame_map;
mod group_menu;
mod help;
pub(crate) mod hud;
mod icon_editor;
mod inventory;
mod manifest;
mod manifest_layout;
mod meta;
mod notify;
mod party;
mod popup;
mod progression;
mod routines;
// `pub(crate)` rather than private: `lib.rs::handle_sprite_pointer` needs
// `sprite_forge::HitRects` to name the type `sprite_editor_hit_rects` below
// hands back — every other module here stays private because nothing
// outside `render/` ever needed one of its types before the mouse did.
pub(crate) mod sprite_forge;
mod stack;
mod stack_market;
mod stock;
mod structure_manifest;
mod talents;
mod trade;
mod transfer;

use arena::{
    draw_arena_builder, draw_arena_load, draw_arena_pick, draw_arena_result, draw_arena_save,
};
use base::{draw_history, draw_playing_base};
// The one cue on the map a test outside `render` has to be able to name:
// arming the tools is `App`-level state and the fixture that reaches base
// space lives beside `App`.
#[cfg(test)]
pub(crate) use base::CUTTING_OUTLINE;
use battle::{
    draw_battle, draw_battle_ally_menu, draw_battle_item_menu, draw_battle_special_menu,
    draw_battle_target_menu,
};
use building::{
    draw_base_output, draw_base_staff, draw_build_direction, draw_build_menu, draw_remove_confirm,
    draw_remove_menu, draw_staffing_menu, draw_structure_menu, draw_structures, draw_upgrade_menu,
    draw_work_order_pick, draw_work_order_quantity, draw_work_orders,
};
use caravan::{CaravanBasket, draw_caravan};
use contracts::draw_contracts;
use crafting::{draw_compiling, draw_craft_menu, draw_craft_quantity, draw_recipes};
use field::{draw_field_routine, draw_field_routine_ally};
use frame_map::{draw_frame_map, draw_frame_map_cursor, draw_map_inset};
use group_menu::{draw_dev_console, draw_group_menu};
use help::{draw_help_index, draw_help_page};
use inventory::{
    draw_equip_swap, draw_erase_quantity, draw_gear_inspect, draw_inventory,
    draw_inventory_item_action, effect_lines,
};
use manifest::{ManifestNav, draw_manifest, draw_manifest_pick};
use meta::{
    draw_achievements, draw_game_over, draw_load_game, draw_main_menu, draw_quit_app_confirm,
    draw_quit_run_confirm, draw_save_action,
};
use party::{
    draw_companion_equip, draw_companion_memories, draw_companion_menu, draw_fuse_menu,
    draw_fuse_name_menu, draw_fuse_second_menu, draw_refactor, draw_refactor_item,
    draw_rename_menu,
};
use popup::{PopupSize, Row, counted_item_row, draw_popup, text_row};
use progression::{draw_perks_menu, draw_research_menu};
use routines::{
    draw_extract, draw_extract_confirm, draw_extract_pick, draw_routine_etch, draw_routine_install,
    draw_routine_target, draw_routines,
};
use sprite_forge::{draw_sprite_editor, draw_sprite_picker};
use stack_market::draw_stack_market;
use talents::{draw_develop, draw_develop_program};
use trade::{
    draw_trade_action_menu, draw_trade_menu, draw_trade_program_confirm, draw_trade_quantity_menu,
};
use transfer::draw_transfer;

/// The colour `draw` clears the window to before dispatching to a screen.
/// Named so `icon_editor.rs` can paint a transparent canvas pixel in the
/// window's own background rather than a hardcoded black the player could
/// mistake for a painted dark pixel.
const SCREEN_BG: Color = Color::new(0.02, 0.02, 0.03, 1.0);
const PANEL_BG: Color = Color::new(0.06, 0.07, 0.10, 0.95);
const BORDER: Color = Color::new(0.25, 0.65, 0.65, 1.0);
const TEXT: Color = Color::new(0.92, 0.92, 0.92, 1.0);
const TEXT_DIM: Color = Color::new(0.6, 0.6, 0.65, 1.0);
const SELECT_BG: Color = Color::new(0.25, 0.55, 0.55, 0.55);
const RED: Color = Color::new(0.9, 0.25, 0.25, 1.0);
const YELLOW: Color = Color::new(0.9, 0.8, 0.2, 1.0);
const BLUE: Color = Color::new(0.3, 0.55, 0.95, 1.0);
const CYAN: Color = Color::new(0.25, 0.85, 0.85, 1.0);
const MAGENTA: Color = Color::new(0.8, 0.35, 0.85, 1.0);
const GREEN: Color = Color::new(0.35, 0.85, 0.4, 1.0);
const ORANGE: Color = Color::new(0.95, 0.55, 0.15, 1.0);
/// The two rare-spawn tiers (`components::Rarity`), pinned by
/// `the_tier_colours_are_separable_from_their_neighbours`.
///
/// Both sit in crowded parts of the palette and the first draft of each was
/// too close to a neighbour to survive being drawn two pixels tall: silver
/// is cool and clearly blue-leaning rather than a near-neutral that reads
/// as dimmed `TEXT`, and gold is *paler* than `YELLOW` rather than warmer,
/// because warmer runs straight into `ORANGE` — and a hostile's glyph can
/// be drawn in either of those by `difficulty_color`, directly under the
/// bar.
const SILVER: Color = Color::new(0.72, 0.80, 0.92, 1.0);
const GOLD: Color = Color::new(1.0, 0.85, 0.40, 1.0);
/// The two rungs above gold. Platinum is a cool near-white with enough blue
/// to stay clear of `TEXT`, and prismatic a saturated violet — the only
/// corner of the palette left once silver has taken cool-blue, gold pale-
/// warm, `MAGENTA` the fused-and-finished slot and `CYAN` the fusable one.
/// Pinned by `the_tier_colours_are_separable_from_their_neighbours`.
const PLATINUM: Color = Color::new(0.62, 0.95, 0.90, 1.0);
const PRISMATIC: Color = Color::new(0.65, 0.45, 1.0, 1.0);
/// Thickness of the rare-tier bar the map draws along the top edge of a
/// creature's tile — see `draw_surface_map`. Matches the breach spawn
/// point's outline, the other overlay drawn over a glyph rather than
/// instead of it.
const RARITY_BAR_PX: f32 = 2.0;

/// How far toward grey a back-rank group's bar is pulled — enough to read
/// as "can't reach you" beside an engaged group without becoming
/// unreadable.
const BACK_RANK_DESATURATION: f32 = 0.55;

/// A program at or below `1 / CRITICAL_HP_DIVISOR` of its Integrity is
/// flagged as about to be lost. At 0 it is deleted for good, so the warning
/// has to arrive before the hit that gets it there rather than after.
///
/// A presentation threshold, not a difficulty knob — nothing in the sim
/// reads it — so it lives here with the colours rather than in the engine's
/// `tuning.rs`.
const CRITICAL_HP_DIVISOR: i32 = 3;

/// Whether a program is close enough to deletion to warrant the warning
/// colour. The single definition both the battle pane and the party menu
/// call, so the threshold cannot come to mean two different things on two
/// screens.
pub(super) fn hp_critical(hp: i32, max_hp: i32) -> bool {
    max_hp > 0 && hp * CRITICAL_HP_DIVISOR <= max_hp
}

/// The sprite editor's own two hit-test rects for this frame, recomputed
/// from the exact geometry `draw_sprite_editor` draws from — a pointer
/// resolved against these can never disagree with what's on screen. `None`
/// when no editor session is open, which is also when `lib.rs` has nothing
/// to read a pointer for.
pub(crate) fn sprite_editor_hit_rects(app: &App, painter: &Painter) -> Option<sprite_forge::HitRects> {
    let view = app.sprite_editor_view()?;
    let m = ui_metrics(painter.screen_h());
    Some(sprite_forge::hit_rects(
        painter,
        painter.screen_w(),
        &m,
        &view,
        app.zoom,
    ))
}

/// What colour a menu row draws in for something that has been fused —
/// cyan while it can still be an input to another fusion, magenta once it
/// is at `MAX_FUSIONS` and is a finished product. `None` leaves the row's
/// ordinary colour alone.
///
/// Programs and gear both call this, because both stop at the same
/// ceiling: `components::FusionCount` for a program, `GearCopies` (or a
/// worn copy's `EquippedItem::fusion_tier`) for a piece of gear — see
/// `Game::fuse_item`. One function rather than a
/// parallel pair, so the two cannot come to mean different things.
///
/// `Option` rather than a defaulted colour so a caller that already has a
/// colour rule composes with this instead of being overwritten — the party
/// screen's CRITICAL red wins over it, since critical is a state to act on
/// now and fusion depth is a permanent property to read at leisure.
pub(super) fn fusion_color(fusions: u32) -> Option<Color> {
    match fusions {
        0 => None,
        n if n >= MAX_FUSIONS => Some(MAGENTA),
        _ => Some(CYAN),
    }
}

/// What colour a rare tier draws in, or `None` for an ordinary one. The
/// variant names are colours even though a player only ever reads the
/// compiler vocabulary — see `Rarity::label`.
///
/// **The map's tile bar and every menu row call this same function**, so a
/// program cannot read as one colour on the grid and another on its own
/// row — the argument `fusion_color` above makes about programs and gear,
/// applied to the two places a tier is shown.
///
/// Exhaustive on purpose: a rung added to `Rarity` without a colour is a
/// compile error here rather than a tier that ships drawing as plain text,
/// which is the failure a `_ =>` arm would hide.
pub(super) fn rarity_color(rarity: Rarity) -> Option<Color> {
    match rarity {
        Rarity::Ordinary => None,
        Rarity::Silver => Some(SILVER),
        Rarity::Gold => Some(GOLD),
        Rarity::Platinum => Some(PLATINUM),
        Rarity::Prismatic => Some(PRISMATIC),
    }
}

/// The one colour rule for a menu row carrying both permanent tiers,
/// resolving the only case where two of them want the same channel.
///
/// **Fusion outranks rarity**, extending the chain `fusion_color`'s doc
/// starts to `CRITICAL > fusion > rarity > plain`. The same argument
/// applies one step down: on the fuse pickers, cyan-versus-magenta is the
/// read for *can this still be an input*, a question about an action
/// available right now, while a rare tier is never actionable — it is worth
/// knowing, not worth deciding on. A fused Overclocked program keeps the
/// tier in its name and gives up only the colour.
///
/// **Gear calls this too, as of 0.8.9.** It used to call `fusion_color`
/// directly, on the stated grounds that "gear has no tier" — a dropped
/// weapon now rolls exactly the same `Rarity` a wild program does, so that
/// sentence stopped being true and this is the function both use. Sharing it
/// is what stops a weapon and a program disagreeing about what Overclocked
/// looks like while sharing the word.
pub(super) fn tier_color(fusions: u32, rarity: Rarity) -> Option<Color> {
    fusion_color(fusions).or_else(|| rarity_color(rarity))
}

/// How a row's category tag is painted: the quality band of the copy it
/// stands for. A row naming something no copy exists of yet — a recipe's
/// result, a trader's stock — passes `None` and draws exactly as it always
/// did.
///
/// **The ramp is emphasis, not hue.** Only the two extremes spend a colour;
/// the middle two differ by weight alone, so an ordinary copy is never
/// painted an alarming colour and the eye is drawn only to what is worth
/// looking at. A green-to-red con ramp was rejected for that, and because
/// the row colour beside it is already spending two hues on fusion and
/// rarity.
///
/// **Emphasis is monotone.** Gold at normal weight above a bold band would
/// read as *less* emphatic than the rung below it, so the top band keeps the
/// weight the one under it earned and adds the colour.
///
/// **The as-designed band is literally no change** — default colour, default
/// weight. Every copy in every existing save is at `QUALITY_DEFAULT`, so
/// nothing already on screen is repainted by this.
///
/// The thresholds are the engine's (`items::quality_band`) and the palette is
/// this crate's: six sites draw a tag, and an engine-owned rule is what stops
/// them drifting, while a band carrying a weight as well as a hue is not
/// expressible as a `GlyphColor`.
///
/// **Known collision, accepted:** `GOLD` is also `rarity_color`'s colour for
/// `Rarity::Gold`, so an Overclocked exceptional copy shows a gold name and a
/// gold tag meaning two different things. They are different columns, and
/// each colour means exactly one thing in its own — the distinction
/// `Row::Item::icon` already draws.
pub(super) fn quality_tag_style(quality: Option<u8>) -> (Color, bool) {
    match quality.map(quality_band) {
        None | Some(QualityBand::AsDesigned) => (TEXT, false),
        Some(QualityBand::Under) => (GRAY, false),
        Some(QualityBand::Above) => (TEXT, true),
        Some(QualityBand::Exceptional) => (GOLD, true),
    }
}

/// How a rare tier reads where a full name will not fit — the battle
/// roster, whose `NAME_W` cell an "Overclocked Scrapper 2" overflows.
/// Bracketed to sit beside `[BOSS]`, and empty for an ordinary creature so
/// the overwhelming majority of rows gain nothing at all.
///
/// The words are the engine's (`Rarity::label`), not this renderer's: the
/// enum names colours and the player reads Optimized/Overclocked, and only
/// one place gets to make that translation.
pub(super) fn rarity_tag(rarity: Rarity) -> String {
    match rarity.label() {
        Some(tier) => format!(" [{}]", tier.to_uppercase()),
        None => String::new(),
    }
}

/// How a fusion depth reads in a menu row — nothing at all for something
/// never fused, a plain count while it still has fusions left, and an
/// explicit "maxed" note at `MAX_FUSIONS`. Beside `fusion_color` because
/// the two say the same thing in two channels and must agree on where the
/// ceiling is.
pub(super) fn fusion_tag(fusions: u32) -> String {
    match fusions {
        0 => String::new(),
        n if n >= MAX_FUSIONS => format!(" (fused {n}/{MAX_FUSIONS} - maxed)"),
        n => format!(" (fused {n}/{MAX_FUSIONS})"),
    }
}

/// The spent-upgrade-slots counterpart of `fusion_tag`, and deliberately the
/// same shape: both are permanent, both are capped, and a player reading a
/// row wants to know how much of each ceiling is gone.
pub(super) fn refactor_tag(refactors: u32) -> String {
    match refactors {
        0 => String::new(),
        n if n >= MAX_COMPANION_REFACTORS => {
            format!(" (upgraded {n}/{MAX_COMPANION_REFACTORS} - maxed)")
        }
        n => format!(" (upgraded {n}/{MAX_COMPANION_REFACTORS})"),
    }
}

/// The one place a `GlyphColor` becomes a drawable `Color`. Shared by the map
/// and by the manifest's header portrait — a second copy would be free to
/// drift, and a program would read as one colour on the grid and another on
/// its own sheet.
pub(super) fn glyph_color(c: GlyphColor) -> Color {
    hud::palette::glyph(c)
}

/// The colour the player's own glyph wears, off the **0-based** swatch
/// index the character-creation wizard wrote — `None`, and an index the
/// palette has nothing at, are both the `PLAYER` role colour, which is
/// what a save from before the wizard and a run that skipped the Look step
/// both carry.
///
/// The map and the wizard's preview cell both call this rather than each
/// spelling the fallback out, or the preview would go on promising a
/// colour the map had stopped drawing.
pub(super) fn player_look_color(colour: Option<u8>) -> Color {
    colour
        .and_then(|i| hud::palette::PLAYER_CHOICES.get(i as usize))
        .copied()
        .unwrap_or(hud::palette::PLAYER)
}

/// The sprite name to try for the player's own tile, or `None` for a look
/// that names none — the empty name is *no sprite*, not a lookup, and its
/// caller draws the glyph. Shared with `player_look_color` for its reason.
pub(super) fn player_sprite_name(sprite: &str) -> Option<&str> {
    (!sprite.is_empty()).then_some(sprite)
}

/// Pulls `color` toward its own grey, for drawing something that's present
/// but not currently in play.
fn desaturate(color: Color) -> Color {
    let grey = (color.r + color.g + color.b) / 3.0;
    let mix = |c: f32| c + (grey - c) * BACK_RANK_DESATURATION;
    Color::new(mix(color.r), mix(color.g), mix(color.b), color.a)
}

/// Splits `text` so that every run of digits in it is drawn in `emphasis` and
/// everything else in `base`.
///
/// Which characters form a number is lexical, and stays that. The *decision*
/// to emphasise at all is still the engine's, taken from `MessageKind` by the
/// only caller — this does not sniff a line to work out what it is.
fn emphasize_numbers<'a>(text: &'a str, base: Color, emphasis: Color) -> Vec<TextRun<'a>> {
    let mut runs = Vec::new();
    let mut start = 0;
    let mut in_number = text.starts_with(|c: char| c.is_ascii_digit());
    let mut push = |piece: &'a str, bold: bool| {
        runs.push(TextRun {
            text: piece,
            bold,
            color: if bold { emphasis } else { base },
        });
    };
    for (i, c) in text.char_indices() {
        if c.is_ascii_digit() != in_number {
            push(&text[start..i], in_number);
            start = i;
            in_number = !in_number;
        }
    }
    if start < text.len() {
        push(&text[start..], in_number);
    }
    runs
}

/// The colour a log line of each kind is narrated in, chosen by the
/// engine-supplied `MessageKind` rather than by sniffing the text —
/// low-priority chatter stays dim, gains and either side's blows get a color.
///
/// For `PartyDamage` this is the colour of the *narration*; the number inside
/// it is picked out separately — see `draw_message_line`.
fn message_color(kind: MessageKind) -> Color {
    match kind {
        MessageKind::Info | MessageKind::Round | MessageKind::PartyDamage => TEXT_DIM,
        MessageKind::Loot => BLUE,
        // All three are something going right, and they are told apart by
        // weight and by source rather than by hue — a level is bold, a patch
        // is not, and a finished order is base news. `Complete` is the only
        // thing distinguishing that line from the filing and cancellation
        // lines it shares its wording with.
        MessageKind::LevelUp | MessageKind::Heal | MessageKind::Complete => GREEN,
        MessageKind::Raid | MessageKind::EnemySpecial => ORANGE,
        // Red is the "no" colour the status line has always used, and the
        // two never share a pane: a refusal is silent during a fight (see
        // `Game::note_refusal`), and `retain_outcomes_since_battle` drops
        // every `EnemyAttack` line when one ends. So red on the map pane
        // means a refusal and red in the battle pane means a blow landing.
        MessageKind::EnemyAttack | MessageKind::Refusal => RED,
        // A result reads at full brightness: it is the line still on screen
        // once the fight is over and the map is back.
        MessageKind::Outcome => TEXT,
    }
}

/// Whether this kind's line is read for a number rather than for its
/// wording, so the digits take the emphasis and the narration around them
/// stays as quiet as any other chatter.
///
/// Both are the party's own doing and both are read the same way: how hard a
/// blow landed, and how much Integrity came back. `Heal` covers a drain as
/// well as a patch — `use_ability` logs the party's drain under it, and that
/// line is the one with a figure on either side of it.
fn emphasizes_numbers(kind: MessageKind) -> bool {
    matches!(kind, MessageKind::PartyDamage | MessageKind::Heal)
}

/// What a log row reads as: the line itself plus the `×N` a folded row
/// carries — see `resources::condense`. The same suffix the history
/// screen's `counted_item_row` writes, so one repeated line reads the same
/// wherever it is drawn; a row standing for a single line carries nothing.
///
/// Separate from the drawing because the count is part of the sentence, and
/// anything that measures or wraps a row has to see it — the map's log pane
/// wraps this string, not `entry.text`.
fn message_text(entry: &LogEntry) -> Cow<'_, str> {
    if entry.repeats > 1 {
        Cow::Owned(format!("{} \u{d7}{}", entry.text, entry.repeats))
    } else {
        Cow::Borrowed(&entry.text)
    }
}

/// Draws one log-pane row in its kind's style.
fn draw_message_line(entry: &LogEntry, x: f32, y: f32, painter: &Painter, m: &Metrics) {
    draw_message_text(entry.kind, &message_text(entry), x, y, painter, m);
}

/// The same, for a caller that has already broken an entry into rows: the
/// map's log pane wraps a long line and draws each row through here, so the
/// three styling paths stay in one place rather than being restated beside
/// the wrap.
fn draw_message_text(
    kind: MessageKind,
    text: &str,
    x: f32,
    y: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let color = message_color(kind);
    match kind {
        k if emphasizes_numbers(k) => {
            painter.ui_runs(&emphasize_numbers(text, color, WHITE), x, y, m.font_size)
        }
        MessageKind::LevelUp => painter.ui_bold(text, x, y, m.font_size, color),
        _ => painter.ui(text, x, y, m.font_size, color),
    }
}

/// Whether `mode` needs `App::status_line` on a strip along the bottom
/// edge, because it draws nothing that can carry the message itself.
///
/// **This is now the exception rather than the rule.** Every mode that
/// draws a popup shows a refusal inside it, under the title, where the
/// player is already looking — see `draw_popup`. What is left is the
/// surfaces that are not popups: the map, which draws it over its log pane,
/// the two full-pane frame maps, and the two dev-only Sprite Forge screens
/// (`sprite_forge.rs`), none of which draw anything else at all.
///
/// `Battle` and `BattleResult` are not here: a refusal raised in a fight is
/// the only one the log never keeps (`Game::note_refusal`), so the strip is
/// the whole of how it reaches the player.
fn needs_status_banner(mode: Mode) -> bool {
    matches!(
        mode,
        Mode::Battle
            | Mode::BattleResult
            | Mode::FrameMap
            | Mode::FieldRoutineCell
            | Mode::Notification
            | Mode::SpritePicker
            | Mode::SpriteEditor
    )
}

/// Draws `status` in a strip along the bottom edge, below every popup —
/// `draw_popup` caps a panel at 85% of the window height and centers it, so
/// the bottom 7.5% is always clear.
fn draw_status_banner(status: &str, painter: &Painter, m: &Metrics) {
    let dims = painter.measure_ui(status, m.font_size);
    let baseline = painter.screen_h() - m.pad;
    painter.rect(
        0.0,
        baseline - dims.height - m.pad / 2.0,
        painter.screen_w(),
        dims.height + m.pad,
        PANEL_BG,
    );
    painter.ui(status, m.inset, baseline, m.font_size, RED);
}

pub fn draw(app: &mut App, fx: &mut Fx, painter: &Painter) {
    let m = ui_metrics(painter.screen_h());
    painter.clear(SCREEN_BG);
    // Cloned rather than borrowed because most of the arms below want `app`
    // mutably, and threaded down as a parameter rather than read off `app`
    // where it is wanted: **this match is the one place that knows which
    // surface is on top**, and a refusal belongs on that one and no other.
    // Where an arm draws two things, the underlying one takes `None`.
    let held = app.status_line.clone();
    let refusal = held.as_deref();
    match app.mode {
        Mode::MainMenu => draw_main_menu(app, refusal, painter, &m),
        // Drawn over the menu it was opened from, so the row `q` was pressed
        // on stays visible behind the question.
        Mode::QuitAppConfirm => {
            draw_main_menu(app, None, painter, &m);
            draw_quit_app_confirm(app.menu_selected, refusal, painter, &m);
        }
        Mode::Achievements => draw_achievements(app, refusal, painter, &m),
        Mode::LoadGame => draw_load_game(app, refusal, painter, &m),
        Mode::SaveAction => draw_save_action(app, refusal, painter, &m),
        // The icon editor hangs off the wizard's Icon step as `App` state
        // rather than a `Mode` of its own (`app::icon_editor`'s reason), so
        // it is this arm's job to notice it is open and draw over the
        // wizard's own popup instead of drawing it.
        Mode::CreateCharacter => match app.icon_editor_view() {
            Some(view) => icon_editor::draw_icon_editor(&view, painter, &m),
            None => creation::draw_create_character(app, refusal, painter, &m),
        },
        Mode::GameOver => draw_game_over(app, refusal, painter, &m),
        // Drawn over the map rather than over black: the run is still there
        // behind the notice, and the scrim lets it show through faintly.
        // A `None` refusal because this screen draws no popup to put one in
        // — `needs_status_banner` names it, so a refusal raised underneath
        // still reaches the strip along the bottom.
        Mode::Notification => {
            draw_playing_base(app, fx, None, painter, &m);
            match &app.pending_notification {
                Some(note) => notify::draw_notification(note, painter, &m),
                // The mode is only ever entered with a subject, so this is
                // unreachable — but a blank window would be a soft lock the
                // player cannot read their way out of, and the map is not.
                None => draw_mode_overlay(app, None, painter, &m),
            }
        }
        Mode::Battle => draw_battle(app, fx, painter, &m),
        Mode::BattleTarget => {
            draw_battle(app, fx, painter, &m);
            draw_battle_target_menu(app, refusal, painter, &m);
        }
        Mode::BattleItem => {
            draw_battle(app, fx, painter, &m);
            draw_battle_item_menu(app, refusal, painter, &m);
        }
        Mode::BattleSpecial => {
            draw_battle(app, fx, painter, &m);
            draw_battle_special_menu(app, refusal, painter, &m);
        }
        Mode::BattleAlly => {
            draw_battle(app, fx, painter, &m);
            draw_battle_ally_menu(app, refusal, painter, &m);
        }
        Mode::Help => {
            draw_playing_base(app, fx, None, painter, &m);
            draw_help_index(app, refusal, painter, &m);
        }
        Mode::HelpPage => {
            draw_playing_base(app, fx, None, painter, &m);
            draw_help_page(app, refusal, painter, &m);
        }
        // Full-pane rather than a popup over the corridor: the whole point
        // is seeing the frame's shape at once, and a map you have to peer
        // around the first-person view to read is not that.
        Mode::FrameMap => match app.game.as_ref().and_then(|g| g.frame_map()) {
            Some(view) => {
                draw_frame_map(&view, painter, painter.screen_w(), painter.screen_h(), &m)
            }
            // Surfacing with the map open, which the engine allows: fall
            // back to the map screen rather than to a blank pane.
            // `None` on the way down: `needs_status_banner` answers per
            // mode, and this mode's other arm draws no popup at all — so the
            // banner is what says it here too, rather than the two arms of
            // one mode disagreeing about where a refusal appears.
            None => {
                draw_playing_base(app, fx, None, painter, &m);
                draw_mode_overlay(app, None, painter, &m);
            }
        },
        // Full-pane for the same reason `Mode::FrameMap` is, and doubly so:
        // picking a cell you have never walked to means seeing the whole
        // frame at once.
        Mode::FieldRoutineCell => {
            match (
                app.game.as_ref().and_then(|g| g.frame_map()),
                app.field_cursor,
            ) {
                (Some(view), Some(cursor)) => draw_frame_map_cursor(
                    &view,
                    cursor,
                    painter,
                    painter.screen_w(),
                    painter.screen_h(),
                    &m,
                ),
                // Surfacing mid-pick, the same fallback the map screen makes.
                // `None` for `Mode::FrameMap`'s reason, one arm up.
                _ => {
                    draw_playing_base(app, fx, None, painter, &m);
                    draw_mode_overlay(app, None, painter, &m);
                }
            }
        }
        // Their own arms rather than `draw_mode_overlay`'s, because the
        // arena hangs off the main menu: `app.game` is `None` on every one
        // of these, and the map underneath a mode overlay needs a run.
        Mode::ArenaBuilder => draw_arena_builder(
            &app.arena_builder_rows(),
            app.menu_selected,
            refusal,
            painter,
            &m,
        ),
        Mode::ArenaSave => {
            // The builder is underneath the name prompt; the prompt says it.
            draw_arena_builder(
                &app.arena_builder_rows(),
                app.menu_selected,
                None,
                painter,
                &m,
            );
            draw_arena_save(&app.arena_save_input, refusal, painter, &m);
        }
        Mode::ArenaPick => draw_arena_pick(
            &app.arena_pick_rows(),
            app.menu_selected,
            refusal,
            painter,
            &m,
        ),
        Mode::ArenaLoad => draw_arena_load(
            &app.arena_load_rows(),
            app.menu_selected,
            refusal,
            painter,
            &m,
        ),
        Mode::ArenaResult => draw_arena_result(
            app.arena_outcome(),
            app.arena_warnings(),
            app.arena_seed(),
            app.arena_transcript(),
            app.menu_selected,
            refusal,
            painter,
            &m,
        ),
        Mode::BattleResult => draw_battle(app, fx, painter, &m),
        // Full-pane, like `Mode::FrameMap`: both hang off the main menu
        // (`app.game` is `None` the whole time either is open) and neither
        // draws a popup box to put a refusal in, so both take `needs_status_
        // banner`'s door instead of a `refusal` argument here.
        Mode::SpritePicker => draw_sprite_picker(app, painter, &m),
        Mode::SpriteEditor => draw_sprite_editor(app, painter, &m),
        _ => {
            // The map is the one surface that is not a popup and still has
            // somewhere to put a refusal: its own log pane. So it takes the
            // message when it *is* the screen, and `None` when one of the
            // ~60 modes below is about to draw a popup over it.
            let on_the_map = if app.mode == Mode::Playing {
                refusal
            } else {
                None
            };
            draw_playing_base(app, fx, on_the_map, painter, &m);
            draw_mode_overlay(app, refusal, painter, &m);
        }
    }
    if let Some(status) = &app.status_line
        && needs_status_banner(app.mode)
    {
        draw_status_banner(status, painter, &m);
    }
}

/// Formats a `(item, quantity)` cost list, tagged `(have/need)`.
///
/// Counts the tier-0 row alone. A fused copy is not an ingredient — every
/// recipe reads `components::Inventory`, which is the tier-0 store — so
/// summing across tiers here would promise material the compile then
/// refuses to spend.
/// Each line of a bill of materials as `Name (have/need)`, counting the
/// player's own pack.
///
/// **The player-paid form**, and the one every verb that spends out of the
/// pack takes: compiling at a bench, upgrading, a symlink. `build_cost_
/// display` below is the other one, and which a screen wants is a question
/// about *whose store pays*, which is why they are two named functions
/// rather than one with a slice a caller can forget to fill.
fn cost_display(game: &Game, cost: &[(ItemId, u32)], inventory: &[InventoryRow]) -> Vec<String> {
    cost_rows(game, cost, |item| carried(inventory, item))
}

/// The build menu's form: "have" is the pack **and** the base's shelves,
/// because both are stores a builder fetches from — see
/// `game::base::construction::Source`.
///
/// Counting the pack alone made this column a claim about the player when
/// the question is about the base: a run with sixty Core Fragments banked in
/// a Depot and none in hand read `(0/18)` beside a structure the crew could
/// have started on the moment it was filed.
///
/// The base half is `Game::base_stock`, the same walk the stock strip across
/// the top of this very screen draws, so the two figures a player can see at
/// once cannot disagree.
fn build_cost_display(
    game: &Game,
    cost: &[(ItemId, u32)],
    inventory: &[InventoryRow],
    stock: &[StockRow],
) -> Vec<String> {
    cost_rows(game, cost, |item| {
        carried(inventory, item)
            + stock
                .iter()
                .find(|row| &row.item == item)
                .map(|row| row.qty)
                .unwrap_or(0)
    })
}

/// How many plain copies of `item` the pack holds. Tier 0 only: a fused copy
/// is not a material.
fn carried(inventory: &[InventoryRow], item: &ItemId) -> u32 {
    inventory
        .iter()
        .find(|row| &row.copy.item == item && row.copy.tier == 0)
        .map(|row| row.qty)
        .unwrap_or(0)
}

/// The shared formatter, so the two forms above differ only in what they
/// count and never in how a row reads.
fn cost_rows(game: &Game, cost: &[(ItemId, u32)], have: impl Fn(&ItemId) -> u32) -> Vec<String> {
    cost.iter()
        .map(|(item, qty)| format!("{} ({}/{qty})", game.item_name(item), have(item)))
        .collect()
}

/// The cargo row `Mode::InventoryItemAction` and `Mode::ItemDescribe` are
/// about, as the `(item, tier)` pair both their screens take.
fn draw_mode_overlay(app: &mut App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    let pending_manifest = app.pending_manifest;
    let manifest_origin = app.manifest_origin;
    let pending_field_routine = app.pending_field_routine;
    let pending_structure = app.pending_structure.clone();
    let pending_item = app.pending_inventory_item.clone();
    let pending_inspect = app.pending_inspect.clone();
    // Taken off `app` before `app.game` is borrowed below, and through the
    // same methods the handlers pick from. A renderer holding its own copy
    // of the filter would draw a list the handler doesn't index — which is
    // survivable while both are unconditional, and is not once the base
    // menu starts hiding rows.
    let group_rows = match app.mode {
        Mode::BaseMenu => app.base_menu_rows(),
        Mode::PartyMenu => app.party_menu_rows(),
        _ => Vec::new(),
    };
    let staffing = match app.mode {
        Mode::StructureAssign => app.staffing(),
        _ => None,
    };
    // Through `App::contract_sections` and not off the engine directly, for
    // `group_rows`' reason one line up: the handler resolves a row number
    // against these two lists, so a renderer with its own copy would act on a
    // different contract from the one under the highlight.
    let (contract_active, contract_offers, contract_reach) = match app.mode {
        Mode::Contracts => {
            let (active, offers) = app.contract_sections();
            (active, offers, app.broker_reach())
        }
        _ => (Vec::new(), Vec::new(), BrokerReach::NoBroker),
    };
    let scanned = match app.mode {
        Mode::WorkStructure => app.workable_structures(),
        Mode::Remove => app.nearby_structures(),
        Mode::Upgrade => app.upgradeable_structures(),
        _ => Vec::new(),
    };
    // Row counts are app-core's and rows are gui's, the way the history
    // screen and the structure roster already work: a renderer that rebuilt
    // these lists itself would be right until the first hidden row and then
    // draw a different one from the one under the highlight.
    let work_orders = match app.mode {
        Mode::WorkOrders => app.work_order_rows(),
        _ => Vec::new(),
    };
    let orderable = match app.mode {
        Mode::WorkOrderPick => app.orderable_items(),
        _ => Vec::new(),
    };
    let base_staff = match app.mode {
        Mode::BaseStaff => app.base_staff_rows(),
        _ => Vec::new(),
    };
    // The row's two holdings and the amount the basket is asking for, taken
    // here because `game` below borrows `&mut app.game` and these read the
    // rest of the `App`. The screen projects the holdings against the amount
    // rather than drawing the two ceilings: `App::put_available` is what the
    // *keys* clamp against and never something the player reads.
    let transfer_entries: Vec<(ItemId, i64, u32, u32)> = match app.mode {
        Mode::Transfer => app
            .basket_rows
            .iter()
            .enumerate()
            .map(|(row, r)| {
                let amount = app.basket_amounts.get(row).copied().unwrap_or(0);
                (r.item.clone(), amount, r.carried, r.on_shelves)
            })
            .collect(),
        _ => Vec::new(),
    };
    // The wagon's view, and each row's basket cell beside it. Taken here for
    // `transfer_entries`' reason — `App::caravan_ceiling` takes `&self`, so
    // it cannot run once `game` below holds `&mut app.game` — and computed
    // through app-core rather than here, so the figure the screen draws is
    // the figure the keys clamp against.
    let caravan = match app.mode {
        Mode::Caravan => app
            .game
            .as_mut()
            .and_then(|g| g.caravan_view())
            .map(|view| {
                let cells = (0..view.offers.len() + view.sells.len())
                    .map(|row| {
                        (
                            app.caravan_amounts.get(row).copied().unwrap_or(0),
                            app.caravan_ceiling(&view, row),
                        )
                    })
                    .collect();
                let purse = app.caravan_purse_after(&view);
                CaravanBasket { view, cells, purse }
            }),
        _ => None,
    };
    // Read before `game` takes the whole of `app`, as the rows above are:
    // the figure is derived from more than one field, so the borrow cannot
    // be split at the call.
    let craft_quantity = app.craft_quantity();
    let Some(game) = &mut app.game else { return };
    match app.mode {
        Mode::BaseMenu => draw_group_menu(&group_rows, "Base", selected, refusal, painter, m),
        Mode::PartyMenu => draw_group_menu(&group_rows, "Party", selected, refusal, painter, m),
        Mode::DevConsole => {
            draw_dev_console(App::dev_console_rows(), selected, refusal, painter, m)
        }
        Mode::Build => draw_build_menu(game, selected, refusal, painter, m),
        Mode::BuildDirection => {
            draw_build_direction(game, pending_structure.as_deref(), refusal, painter, m)
        }
        Mode::Transfer => draw_transfer(
            game,
            &transfer_entries,
            app.basket_room,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::Craft => draw_craft_menu(game, selected, refusal, painter, m),
        Mode::CraftQuantity => draw_craft_quantity(
            game,
            app.pending_craft.clone(),
            craft_quantity,
            app.careful_craft,
            refusal,
            painter,
            m,
        ),
        Mode::Compiling => draw_compiling(game, app.compile_progress.as_ref(), refusal, painter, m),
        Mode::EraseQuantity => draw_erase_quantity(
            game,
            app.pending_erase.clone(),
            &app.erase_quantity_input,
            refusal,
            painter,
            m,
        ),
        Mode::WorkOrders => draw_work_orders(
            &work_orders,
            game.labour_demand(),
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::WorkOrderPick => draw_work_order_pick(&orderable, selected, refusal, painter, m),
        Mode::WorkOrderQuantity => draw_work_order_quantity(
            game,
            app.pending_order.clone(),
            &app.order_quantity_input,
            app.standing_order,
            app.order_priority,
            refusal,
            painter,
            m,
        ),
        Mode::BaseStaff => draw_base_staff(game, &base_staff, selected, refusal, painter, m),
        Mode::WorkStructure => draw_structure_menu(
            &scanned,
            "Work",
            "Work which structure yourself?",
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::Remove => draw_remove_menu(&scanned, selected, refusal, painter, m),
        Mode::RemoveConfirm => draw_remove_confirm(selected, refusal, painter, m),
        Mode::Upgrade => draw_upgrade_menu(game, &scanned, selected, refusal, painter, m),
        Mode::InspectDirection => draw_direction_prompt(
            "Inspect Direction",
            "Choose a direction to inspect (arrows/hjkl), Esc to cancel",
            refusal,
            painter,
            m,
        ),
        // Names the cascade the way the picker's own header does: this route
        // reaches Home in one keypress, so the warning cannot wait for the
        // confirmation screen to be the first mention of it.
        Mode::RemoveDirection => draw_direction_prompt(
            "Demolish Direction",
            "Demolish which neighbour? Removing Home destroys the whole base. \
             (arrows/hjkl, Esc to cancel)",
            refusal,
            painter,
            m,
        ),
        Mode::Manifest => {
            // Only advertise ←/→ when they actually do something. A wild
            // program reached via `x` is not in the owned list, so cycling
            // from it is a no-op and the footer must not claim otherwise.
            let subjects = game.manifest_subjects();
            let nav = ManifestNav {
                cyclable: subjects.len() > 1
                    && pending_manifest.is_some_and(|e| subjects.contains(&e)),
                back_to_list: manifest_origin.returns_to_list(),
                // Only advertise `w` when it will work, `cyclable`'s rule:
                // the roster reaches every program you own, including the
                // ones whose tile is the one they were beaten on.
                watchable: pending_manifest.is_some_and(|e| game.watch_position(e).is_some()),
            };
            draw_manifest(game, pending_manifest, nav, refusal, painter, m)
        }
        Mode::ManifestPick => {
            let subjects = game.manifest_subjects();
            draw_manifest_pick(game, &subjects, selected, refusal, painter, m)
        }
        Mode::StructureManifest => structure_manifest::draw_structure_manifest(
            game,
            app.pending_structure_manifest,
            refusal,
            painter,
            m,
        ),
        Mode::CellDescribe => {
            stack::draw_cell_describe(app.pending_description.as_deref(), refusal, painter, m)
        }
        Mode::Inventory => draw_inventory(game, selected, refusal, painter, m),
        Mode::CompanionEquip => draw_companion_equip(
            game,
            app.pending_equip_program,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::CompanionMemories => {
            draw_companion_memories(game, app.pending_memory_program, refusal, painter, m)
        }
        Mode::EquipSwap => draw_equip_swap(
            game,
            app.pending_swap_slot,
            app.pending_swap_target,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::InventoryItemAction => {
            let zone = game.player_status().zone;
            draw_inventory_item_action(
                game,
                pending_item.clone(),
                zone,
                selected,
                refusal,
                painter,
                m,
            )
        }
        Mode::ItemDescribe => draw_gear_inspect(game, pending_inspect.clone(), refusal, painter, m),
        Mode::Companion => draw_companion_menu(game, selected, refusal, painter, m),
        Mode::Fuse => draw_fuse_menu(game, selected, refusal, painter, m),
        Mode::FuseSecond => {
            draw_fuse_second_menu(game, app.pending_fuse_first, selected, refusal, painter, m)
        }
        Mode::FuseName => draw_fuse_name_menu(
            game,
            app.pending_fuse_first,
            app.pending_fuse_second,
            &app.fuse_name_input,
            refusal,
            painter,
            m,
        ),
        Mode::RenamePet => draw_rename_menu(
            game,
            app.pending_rename,
            &app.rename_input,
            refusal,
            painter,
            m,
        ),
        Mode::FieldRoutine => draw_field_routine(game, selected, refusal, painter, m),
        Mode::FieldRoutineAlly => {
            draw_field_routine_ally(game, pending_field_routine, selected, refusal, painter, m)
        }
        Mode::RoutineTarget => draw_routine_target(game, selected, refusal, painter, m),
        Mode::Routines => draw_routines(
            game,
            app.pending_routine_holder,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::RoutineInstall => draw_routine_install(game, selected, refusal, painter, m),
        Mode::RoutineEtch => draw_routine_etch(game, selected, refusal, painter, m),
        Mode::Refactor => draw_refactor(game, selected, refusal, painter, m),
        Mode::Develop => draw_develop(game, selected, refusal, painter, m),
        Mode::DevelopProgram => draw_develop_program(
            game,
            app.pending_develop_target,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::RefactorItem => draw_refactor_item(
            game,
            app.pending_refactor_target,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::Extract => draw_extract(game, selected, refusal, painter, m),
        Mode::ExtractPick => draw_extract_pick(
            game,
            app.pending_extract_program,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::ExtractConfirm => draw_extract_confirm(
            game,
            app.pending_extract_program,
            app.pending_extract_index,
            refusal,
            painter,
            m,
        ),
        Mode::StackMarket => draw_stack_market(game, selected, refusal, painter, m),
        Mode::Caravan => draw_caravan(game, caravan, selected, refusal, painter, m),
        Mode::Trade => draw_trade_menu(game, selected, refusal, painter, m),
        Mode::TradeAction => draw_trade_action_menu(
            game,
            app.pending_trade_structure,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::TradeQuantity => draw_trade_quantity_menu(
            game,
            app.pending_trade_structure,
            app.pending_trade_choice.clone(),
            &app.trade_quantity_input,
            refusal,
            painter,
            m,
        ),
        Mode::TradeProgramConfirm => {
            let money = game.item_name(&game.trade_currency()).to_string();
            draw_trade_program_confirm(
                app.pending_trade_program.as_ref(),
                &money,
                refusal,
                painter,
                m,
            )
        }
        Mode::Perks => draw_perks_menu(game, selected, refusal, painter, m),
        Mode::Research => draw_research_menu(game, selected, refusal, painter, m),
        Mode::Contracts => draw_contracts(
            &contract_active,
            &contract_offers,
            contract_reach,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::History => draw_history(game, selected, refusal, painter, m),
        Mode::Structures => draw_structures(game, selected, refusal, painter, m),
        Mode::StructureAssign => {
            if let Some(staffing) = &staffing {
                draw_staffing_menu(staffing, selected, refusal, painter, m);
            }
        }
        Mode::Recipes => draw_recipes(game, selected, refusal, painter, m),
        Mode::BaseOutput => draw_base_output(game, refusal, painter, m),
        Mode::QuitRunConfirm => draw_quit_run_confirm(selected, refusal, painter, m),
        _ => {}
    }
}

fn draw_direction_prompt(
    title: &str,
    body: &str,
    refusal: Option<&str>,
    painter: &Painter,
    m: &Metrics,
) {
    draw_popup(
        title,
        PopupSize::Small,
        &[text_row(body)],
        refusal,
        painter,
        m,
    );
}

/// A program's current activity as a bracketed suffix — `" (in party)"`,
/// `" (Mining Node)"`, `" (guarding Data Cache)"`, `" (idle)"`. The wording
/// itself is `Game::program_activity`'s; every dialog that lists programs
/// appends it through here so they cannot drift apart.
fn activity_tag(activity: &str) -> String {
    format!(" ({activity})")
}

/// A stand-in program for the row builders that take one. Shared rather than
/// restated per screen: several of these tests measure a real row's *width*,
/// so a second fixture with a shorter name would quietly weaken whichever
/// census copied it.
#[cfg(test)]
pub(super) fn test_pet(name: &str, gear: &str) -> PetInfo {
    PetInfo {
        entity: Entity::PLACEHOLDER,
        glyph: 'p',
        color: GlyphColor::White,
        name: name.to_string(),
        level: 6,
        hp: 22,
        max_hp: 28,
        atk: 8,
        mitigation: 5,
        power: 19,
        party_slot: Some(0),
        role: feral_processes_app_core::ProgramRole::InParty,
        activity: "in party".to_string(),
        quality: None,
        fusions: 0,
        refactors: 0,
        ring: 0,
        talents: 0,
        rarity: Rarity::Ordinary,
        wielded: false,
        gear: gear.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every `Mode`, as the status-line census below drives them.
    const ALL_MODES: [Mode; 90] = [
        Mode::MainMenu,
        Mode::CreateCharacter,
        Mode::LoadGame,
        Mode::SaveAction,
        Mode::Playing,
        Mode::Transfer,
        Mode::BaseMenu,
        Mode::PartyMenu,
        Mode::Battle,
        Mode::BattleTarget,
        Mode::BattleItem,
        Mode::BattleSpecial,
        Mode::BattleAlly,
        Mode::BattleResult,
        Mode::Build,
        Mode::BuildDirection,
        Mode::DevConsole,
        Mode::Craft,
        Mode::CraftQuantity,
        Mode::Compiling,
        Mode::WorkOrders,
        Mode::WorkOrderPick,
        Mode::WorkOrderQuantity,
        Mode::BaseStaff,
        Mode::WorkStructure,
        Mode::Remove,
        Mode::RemoveConfirm,
        Mode::RemoveDirection,
        Mode::Upgrade,
        Mode::InspectDirection,
        Mode::FrameMap,
        Mode::Manifest,
        Mode::ManifestPick,
        Mode::StructureManifest,
        Mode::CellDescribe,
        Mode::Inventory,
        Mode::EquipSwap,
        Mode::InventoryItemAction,
        Mode::ItemDescribe,
        Mode::EraseQuantity,
        Mode::Companion,
        Mode::CompanionEquip,
        Mode::CompanionMemories,
        Mode::Fuse,
        Mode::FuseSecond,
        Mode::FuseName,
        Mode::RenamePet,
        Mode::RoutineTarget,
        Mode::Routines,
        Mode::RoutineInstall,
        Mode::RoutineEtch,
        Mode::FieldRoutine,
        Mode::FieldRoutineAlly,
        Mode::FieldRoutineCell,
        Mode::Excavate,
        Mode::Refactor,
        Mode::RefactorItem,
        Mode::Develop,
        Mode::DevelopProgram,
        Mode::Extract,
        Mode::ExtractPick,
        Mode::ExtractConfirm,
        Mode::Trade,
        Mode::TradeAction,
        Mode::TradeQuantity,
        Mode::StackMarket,
        Mode::Caravan,
        Mode::TradeProgramConfirm,
        Mode::Perks,
        Mode::Research,
        Mode::Contracts,
        Mode::History,
        Mode::Structures,
        Mode::StructureAssign,
        Mode::Recipes,
        Mode::BaseOutput,
        Mode::Achievements,
        Mode::Help,
        Mode::HelpPage,
        Mode::Notification,
        Mode::GameOver,
        Mode::QuitRunConfirm,
        Mode::QuitAppConfirm,
        Mode::ArenaBuilder,
        Mode::ArenaLoad,
        Mode::ArenaSave,
        Mode::ArenaPick,
        Mode::ArenaResult,
        // Both dev-only, both full-pane draws with their refusal on
        // `needs_status_banner`'s strip rather than in a popup — see
        // `sprite_forge.rs`.
        Mode::SpritePicker,
        Mode::SpriteEditor,
    ];

    const CENSUS_REFUSAL: &str = "Requires Zone 3 first.";

    fn census_app() -> feral_processes_app_core::App {
        let root = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
        let tmp = std::env::temp_dir().join(format!("fp_gui_census_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&tmp);
        std::fs::create_dir_all(&tmp).unwrap();
        let mut app = feral_processes_app_core::App::new(
            root.join("assets"),
            tmp.join("saves"),
            tmp.join("history.log"),
            tmp.join("profile.ron"),
            root.join("dev-arenas"),
            tmp.join("telemetry.jsonl"),
        );
        // [N] opens the wizard, and then every step is answered the
        // cheapest legal way — there is no key that skips it, and the two
        // steps that hand out an allowance will not be left until it is
        // spent.
        app.handle_key(feral_processes_app_core::GameKey::Char('n'));
        for step in feral_processes_app_core::CreationStep::ALL {
            match step {
                feral_processes_app_core::CreationStep::Difficulty => {
                    app.handle_key(feral_processes_app_core::GameKey::Char('f'))
                }
                feral_processes_app_core::CreationStep::Profile => {
                    app.handle_key(feral_processes_app_core::GameKey::Enter)
                }
                feral_processes_app_core::CreationStep::Class => {
                    app.handle_key(feral_processes_app_core::GameKey::Char('1'))
                }
                feral_processes_app_core::CreationStep::Kit
                | feral_processes_app_core::CreationStep::Points => {
                    for i in 0..app.creation_rows().len() {
                        app.menu_selected = i;
                        app.handle_key(feral_processes_app_core::GameKey::ShiftRight);
                    }
                    app.menu_selected = 0;
                    app.handle_key(feral_processes_app_core::GameKey::Enter);
                }
                feral_processes_app_core::CreationStep::Icon
                | feral_processes_app_core::CreationStep::Colour
                | feral_processes_app_core::CreationStep::Routine => {
                    app.handle_key(feral_processes_app_core::GameKey::Char('n'))
                }
                feral_processes_app_core::CreationStep::Perks => {
                    app.handle_key(feral_processes_app_core::GameKey::Enter)
                }

                feral_processes_app_core::CreationStep::Summary
                | feral_processes_app_core::CreationStep::Name => {
                    app.handle_key(feral_processes_app_core::GameKey::Enter)
                }
            }
        }
        app
    }

    /// The modes whose screen needs state a fresh run has not got — a
    /// pending trade, a program picked to fuse, a routine chosen to extract.
    /// Each draws nothing at all here, so the census can say where a refusal
    /// must *not* appear on them but not where it must. Their `draw_popup`
    /// calls are threaded the same way every other one is.
    const NEEDS_PENDING_STATE: [Mode; 21] = [
        Mode::BattleTarget,
        Mode::BattleSpecial,
        Mode::CraftQuantity,
        // Nothing is armed in the census fixture — `Mode::Compiling` is
        // only ever entered right after `begin_hand_craft` succeeds, and
        // this app never commits a compile — so `draw_compiling` draws
        // nothing, same as every other pending-state screen here.
        Mode::Compiling,
        Mode::EraseQuantity,
        Mode::FuseSecond,
        Mode::FuseName,
        Mode::RenamePet,
        Mode::Routines,
        Mode::FieldRoutineAlly,
        Mode::Excavate,
        Mode::RefactorItem,
        Mode::DevelopProgram,
        Mode::ExtractPick,
        Mode::ExtractConfirm,
        Mode::TradeAction,
        Mode::TradeQuantity,
        Mode::StackMarket,
        // The caravan screen needs a trader actually standing at the
        // counter, which a fresh run has not got — `StackMarket`'s case.
        // `a_caravan_page_says_a_refusal_exactly_once` below is what says
        // its `draw_popup` call is threaded, since this census can only say
        // where a refusal must *not* appear on it.
        Mode::Caravan,
        Mode::TradeProgramConfirm,
        Mode::StructureAssign,
    ];

    /// How many times `mode` paints `CENSUS_REFUSAL` with it set.
    fn refusals_drawn(app: &mut feral_processes_app_core::App, fx: &mut Fx, mode: Mode) -> usize {
        app.mode = mode;
        app.status_line = Some(CENSUS_REFUSAL.to_string());
        let (_, shapes) = crate::paint::with_painter(|p| draw(app, fx, p));
        crate::paint::painted_text(&shapes)
            .iter()
            .filter(|t| t.contains(CENSUS_REFUSAL))
            .count()
    }

    /// **The census this whole change rests on.** `draw_popup` takes its
    /// refusal as an argument, so a *new* call site cannot forget it — it
    /// will not compile without one — but nothing stops a caller passing
    /// `None`, and nothing stops two stacked popups both passing `Some`.
    /// This drives every `Mode` through `draw` and counts what was actually
    /// painted, which is the only way to tell either apart from a screen
    /// that simply looks right.
    ///
    /// Both halves matter and both have caught a real defect: `Playing`
    /// fell into the arm that hands the map `None` and showed the message
    /// nowhere, and `ArenaSave` drew it on the prompt *and* on the builder
    /// underneath it.
    #[test]
    fn every_screen_draws_a_refusal_exactly_once() {
        let mut app = census_app();
        let mut fx = Fx::new();
        for mode in ALL_MODES {
            let drawn = refusals_drawn(&mut app, &mut fx, mode);
            if NEEDS_PENDING_STATE.contains(&mode) {
                assert_eq!(drawn, 0, "{mode:?} draws nothing here, so it cannot say it");
            } else {
                assert_eq!(
                    drawn, 1,
                    "{mode:?} painted the refusal {drawn} times, not once"
                );
            }
        }
    }

    /// **Where** it lands, which is the whole point: under the title and
    /// above the first row, not on a strip along an edge the player is not
    /// looking at. `painted_text` comes back in paint order, so the row
    /// order on screen is the order in that list.
    #[test]
    fn a_refusal_sits_between_the_title_and_the_first_row() {
        let mut app = census_app();
        let mut fx = Fx::new();
        app.mode = Mode::Research;
        app.status_line = Some(CENSUS_REFUSAL.to_string());
        let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p));
        let drawn = crate::paint::painted_text(&shapes);

        let at = |want: &str| drawn.iter().position(|t| t.contains(want));
        let title = at("Research").expect("the popup drew its title");
        let refusal = at(CENSUS_REFUSAL).expect("the popup drew the refusal");
        let first_row = drawn
            .iter()
            .position(|t| t.starts_with("  ["))
            .expect("the popup drew a numbered option");

        assert!(title < refusal, "the refusal was drawn above the title");
        assert!(
            refusal < first_row,
            "the refusal was drawn below the first option instead of over it"
        );
    }

    /// Nothing is drawn where nothing was refused — the popup grows a line
    /// only when there is one, so a screen's layout is unchanged the rest of
    /// the time.
    #[test]
    fn a_screen_with_nothing_refused_draws_no_refusal() {
        let mut app = census_app();
        let mut fx = Fx::new();
        for mode in [Mode::Playing, Mode::Research, Mode::Inventory, Mode::Battle] {
            app.mode = mode;
            app.status_line = None;
            let (_, shapes) = crate::paint::with_painter(|p| draw(&mut app, &mut fx, p));
            assert!(
                !crate::paint::painted_text(&shapes)
                    .iter()
                    .any(|t| t.contains(CENSUS_REFUSAL)),
                "{mode:?} painted a refusal that was never raised"
            );
        }
    }

    #[test]
    fn hp_critical_triggers_at_exactly_a_third_and_not_a_point_above() {
        assert!(hp_critical(10, 30), "exactly a third is already critical");
        assert!(!hp_critical(11, 30), "a point above a third is not");
        assert!(hp_critical(0, 30), "a dead program reads as critical");
        assert!(!hp_critical(30, 30), "full health is never critical");
        assert!(
            !hp_critical(0, 0),
            "a program with no max HP is a malformed fixture, not a warning"
        );
    }

    /// Programs and gear read the same, because they share `MAX_FUSIONS` —
    /// cyan means fused and still usable as an input, magenta means at the
    /// ceiling. `None` rather than a default colour so a caller with a
    /// colour rule of its own (the party screen's CRITICAL red) composes
    /// with this instead of being overwritten by it.
    #[test]
    fn fusion_color_separates_a_fused_thing_from_a_maxed_one() {
        assert_eq!(fusion_color(0), None, "an unfused row is left plain");
        assert_eq!(fusion_color(1), Some(CYAN));
        assert_eq!(fusion_color(MAX_FUSIONS - 1), Some(CYAN));
        assert_eq!(fusion_color(MAX_FUSIONS), Some(MAGENTA));
        assert_eq!(
            fusion_color(MAX_FUSIONS + 1),
            Some(MAGENTA),
            "a legacy over-ceiling gear tier still reads as maxed"
        );
    }

    /// The ramp is emphasis and has to be monotone: a band that gave up the
    /// weight the one below it earned would read as *less* emphatic at the
    /// top of its own ladder. Only the two extremes spend a colour, and the
    /// as-designed rung is literally the default treatment — every copy in
    /// every existing save sits at `QUALITY_DEFAULT`, so this repaints
    /// nothing already on screen.
    #[test]
    fn the_quality_tag_ramp_gains_emphasis_and_never_gives_it_back() {
        use feral_processes_engine::tuning::{QUALITY_DEFAULT, QUALITY_MAX, QUALITY_MIN};

        let plain = quality_tag_style(None);
        assert_eq!(
            plain,
            (TEXT, false),
            "an untagged copy draws as it always did"
        );
        assert_eq!(
            quality_tag_style(Some(QUALITY_DEFAULT)),
            plain,
            "the as-designed band is no change at all"
        );

        let mut last = (GRAY, false);
        for quality in QUALITY_MIN..=QUALITY_MAX {
            let style = quality_tag_style(Some(quality));
            assert!(
                style.1 >= last.1,
                "emphasis went backwards at {quality}: {last:?} -> {style:?}"
            );
            last = style;
        }
        assert_eq!(quality_tag_style(Some(QUALITY_MIN)), (GRAY, false));
        assert_eq!(quality_tag_style(Some(QUALITY_MAX)), (GOLD, true));
        // The two middle bands differ by weight alone, which is the whole of
        // why an ordinary copy is never painted an alarming colour.
        assert_eq!(quality_tag_style(Some(110)).0, TEXT);
        assert!(quality_tag_style(Some(110)).1);
    }

    /// Two permanent properties, one channel. Fusion wins because it is the
    /// one that gates an action available now (can this still be a fusion
    /// input); a rare tier is only ever worth knowing.
    #[test]
    fn fusion_outranks_rarity_in_a_menu_row() {
        assert_eq!(
            tier_color(0, Rarity::Gold),
            Some(GOLD),
            "an unfused program shows its tier"
        );
        assert_eq!(
            tier_color(1, Rarity::Gold),
            Some(CYAN),
            "fusion depth is the actionable read and takes the channel"
        );
        assert_eq!(
            tier_color(MAX_FUSIONS, Rarity::Silver),
            Some(MAGENTA),
            "and still does at the ceiling"
        );
        assert_eq!(
            tier_color(0, Rarity::Ordinary),
            None,
            "an ordinary unfused program keeps the plain row colour"
        );
    }

    /// The tier bar is two pixels tall and is drawn directly above a glyph
    /// that `difficulty_color` may have painted green, yellow, orange, red
    /// or magenta — so each tier has to stay clear of all of them, of the
    /// neutrals it could read as a dimmed version of, and of the other tier.
    ///
    /// Asserting the separation rather than the literals, so a palette
    /// retune is free to move any of them. This caught both first drafts:
    /// gold was 0.23 from `YELLOW` and silver 0.22 from `TEXT`.
    #[test]
    fn the_tier_colours_are_separable_from_their_neighbours() {
        let dist = |a: Color, b: Color| (a.r - b.r).abs() + (a.g - b.g).abs() + (a.b - b.b).abs();
        // Everything a tier bar can end up sitting against: every hue the
        // glyph under it can be drawn in, the player's own role colour, and
        // the neutrals a tier could read as a dimmed version of.
        //
        // Walked through `glyph_color` rather than naming the constants a
        // creature *used* to be painted in — named, this went on measuring
        // `render/mod.rs`'s old palette after the map had moved to
        // `hud::palette`, which is the same failure it already caught once
        // in the tier list below.
        let neighbours: Vec<(String, Color)> = GlyphColor::ALL
            .into_iter()
            .map(|c| (format!("{c:?}"), glyph_color(c)))
            .chain([
                ("PLAYER".to_string(), hud::palette::PLAYER),
                ("TEXT".to_string(), TEXT),
                ("TEXT_DIM".to_string(), TEXT_DIM),
            ])
            .collect();
        // Walks `Rarity::ALL` through `rarity_color` rather than naming the
        // colour constants: named, this checked exactly the two rungs that
        // existed when it was written and went on passing when two more were
        // added, which is the whole failure it is supposed to prevent.
        let tiers: Vec<(Rarity, Color)> = Rarity::ALL
            .into_iter()
            .filter_map(|r| Some((r, rarity_color(r)?)))
            .collect();
        assert_eq!(
            tiers.len(),
            Rarity::ALL.len() - 1,
            "every rung above Ordinary needs a colour"
        );
        for (tier, colour) in &tiers {
            for (other_name, other) in &neighbours {
                assert!(
                    dist(*colour, *other) > 0.25,
                    "{tier:?} is only {:.2} from {other_name} — it would read \
                     as that colour in a two-pixel bar",
                    dist(*colour, *other)
                );
            }
        }
        for (i, (tier, colour)) in tiers.iter().enumerate() {
            for (other, other_colour) in &tiers[i + 1..] {
                assert!(
                    dist(*colour, *other_colour) > 0.25,
                    "{tier:?} and {other:?} are only {:.2} apart — two tiers \
                     that read alike are no ladder at all",
                    dist(*colour, *other_colour)
                );
            }
        }
    }

    #[test]
    fn only_a_rare_tier_gets_a_roster_tag() {
        assert_eq!(rarity_tag(Rarity::Ordinary), "");
        assert!(rarity_tag(Rarity::Gold).contains("OVERCLOCKED"));
        assert!(
            rarity_tag(Rarity::Silver).starts_with(' '),
            "the tag is appended after a name and separates itself"
        );
    }

    /// The strip is the fallback for the screens that are not popups and
    /// have no log pane of their own. `Battle` is the load-bearing one: a
    /// refusal raised in a fight is deliberately never logged, so the strip
    /// is the whole of how it reaches the player.
    #[test]
    fn only_the_screens_with_nowhere_else_to_put_it_use_the_banner() {
        for mode in [
            Mode::Battle,
            Mode::BattleResult,
            Mode::FrameMap,
            Mode::FieldRoutineCell,
            Mode::SpritePicker,
            Mode::SpriteEditor,
        ] {
            assert!(
                needs_status_banner(mode),
                "{mode:?} draws no popup, so its refusals need the banner"
            );
        }
    }

    /// The kinds a player is meant to tell apart at a glance mid-fight: what
    /// they gained, what the enemy did, which of the enemy's blows carried a
    /// condition, their own hit, Integrity coming back, and how the fight
    /// came out. Sharing a colour between any two of them defeats the point
    /// of the log being coloured at all — and all six can sit in the battle
    /// pane at once.
    #[test]
    fn the_log_colours_a_player_reads_mid_fight_are_all_distinct() {
        let kinds = [
            MessageKind::Loot,
            MessageKind::EnemyAttack,
            MessageKind::EnemySpecial,
            MessageKind::PartyDamage,
            MessageKind::Heal,
            MessageKind::Outcome,
        ];
        for (i, a) in kinds.iter().enumerate() {
            for b in &kinds[i + 1..] {
                assert_ne!(
                    message_color(*a),
                    message_color(*b),
                    "{a:?} and {b:?} narrate in the same colour"
                );
            }
        }
    }

    /// The party's own drain narrates under `Heal` — the Integrity coming
    /// back is the half a plain `Attack` cannot also produce — and that line
    /// carries a figure on either side of it. Losing the emphasis in the move
    /// would have taken the number-first reading off the one line that has
    /// two, so the styling follows the kind rather than staying behind on
    /// `PartyDamage` alone.
    #[test]
    fn the_lines_read_for_a_number_are_the_partys_own_blows_and_its_own_mending() {
        for kind in [MessageKind::PartyDamage, MessageKind::Heal] {
            assert!(
                emphasizes_numbers(kind),
                "{kind:?} is read for its figure, so its digits take the emphasis"
            );
        }
        for kind in [
            MessageKind::Info,
            MessageKind::Loot,
            MessageKind::LevelUp,
            MessageKind::Raid,
            MessageKind::Round,
            MessageKind::Outcome,
            MessageKind::EnemyAttack,
            MessageKind::EnemySpecial,
            MessageKind::Complete,
        ] {
            assert!(
                !emphasizes_numbers(kind),
                "{kind:?} is read for its wording; picking a digit out of it says nothing"
            );
        }
    }

    /// A message-log line always draws in full, whichever style it takes: the
    /// run split is a re-styling of the text, not a filter on it.
    #[test]
    fn splitting_a_line_into_runs_preserves_it_exactly() {
        for line in [
            "You unleash a data strike for 7 damage.",
            "Sparkgrub 12 executes Arc Bite for 103 damage.",
            "42",
            "no numbers at all",
            "",
            "Ünïcödé hits you for 5 damage.",
        ] {
            let joined: String = emphasize_numbers(line, TEXT_DIM, WHITE)
                .iter()
                .map(|r| r.text)
                .collect();
            assert_eq!(joined, line);
        }
    }

    /// Only the digits take the emphasis. Bolding the whole sentence is what
    /// this replaced, and bolding nothing would leave the number no easier to
    /// pick out than the flavour text around it.
    #[test]
    fn only_the_digits_of_a_damage_line_are_emphasized() {
        let runs = emphasize_numbers("You unleash a data strike for 7 damage.", TEXT_DIM, WHITE);
        for run in &runs {
            let all_digits = run.text.chars().all(|c| c.is_ascii_digit());
            assert_eq!(
                run.bold, all_digits,
                "{:?} came out bold={} — a run is either all digits or none",
                run.text, run.bold
            );
            assert_eq!(run.color, if run.bold { WHITE } else { TEXT_DIM });
        }
        let emphasized: String = runs
            .iter()
            .filter(|r| r.bold)
            .map(|r| r.text)
            .collect::<Vec<_>>()
            .join("|");
        assert_eq!(emphasized, "7");
    }

    /// Multi-digit numbers stay whole. Splitting per character would still
    /// look right, but only by accident of the two faces sharing an advance.
    #[test]
    fn a_multi_digit_number_is_one_run() {
        let runs = emphasize_numbers("Sparkgrub hits you for 103 damage.", TEXT_DIM, WHITE);
        let bold: Vec<&str> = runs.iter().filter(|r| r.bold).map(|r| r.text).collect();
        assert_eq!(bold, ["103"]);
    }

    #[test]
    fn modes_that_already_show_the_status_line_dont_double_up() {
        for mode in [
            Mode::Playing,
            Mode::MainMenu,
            Mode::SaveAction,
            Mode::QuitAppConfirm,
            Mode::Research,
            Mode::Inventory,
            Mode::Trade,
        ] {
            assert!(
                !needs_status_banner(mode),
                "{mode:?} already surfaces status_line itself"
            );
        }
    }
}
