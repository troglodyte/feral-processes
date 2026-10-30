//! All drawing for the graphics frontend: one screen per `Mode`, laid out
//! in immediate mode against `Painter` (filled rects for bars and tiles,
//! drawn text for menus). Reads engine data through `App` and never touches
//! the ECS `World`.

use std::borrow::Cow;

use crate::fx::Fx;
use crate::paint::{Color, GRAY, Painter, Rect, TextRun, WHITE};
use crate::text::{Metrics, map_cell, ui_metrics};
use feral_processes_app_core::{
    App, ArenaRow, BattlePane, GearInspect, MENU_SCAN_RADIUS, Mode, Staffing, SwapChoice, SwapRow,
    TradeChoice, TransferEntry, base_popup_parent, equip_preview_tag, equip_swap_rows,
    inventory_item_actions, item_fusion_note, menu_shortcut, qty_column, stat_summary,
};
use feral_processes_engine::components::{GlyphColor, MachineStatus, Rarity, TaskKind};
use feral_processes_engine::items::{EquipmentSlot, GearCopy, ItemId, QualityBand, quality_band};
use feral_processes_engine::settlements::SettlementKey;
use feral_processes_engine::structures::StructureCategory;
use feral_processes_engine::tuning::{
    KERNEL_RING_MAX, MAX_COMPANION_REFACTORS, MAX_FUSIONS, MAX_PARTY_SIZE,
};
use feral_processes_engine::world::{Biome, Tile};
use feral_processes_engine::{
    Assignee, BrokerReach, ContractRow, CraftRecipe, EmulationOption, Entity, EntityView, Game,
    InventoryRow, LogEntry, MESSAGE_LOG_CAP, MemoryRow, MessageKind, OutpostMark, OutpostReport,
    PetInfo, PodState, ProgramSaleOption, RecipeChain, RecipeStep, ResearchState, SettlementView,
    ShelfFill, StockRow, StructureReport, morale_band,
};
use feral_processes_engine::{ResearchTree, RespecSubject};

mod alerts;
mod alt_labels;
mod alt_overlay;
mod arena;
mod bars;
mod base;
mod base_report;
mod battle;
mod breeding;
mod building;
mod canvas;
mod caravan;
mod compass;
mod contracts;
mod crafting;
mod creation;
mod depot_filter;
mod dev_pickers;
mod dispatch;
mod dossier;
mod ending;
mod excavate_brush;
mod extraction;
mod farm_recipe;
mod field;
mod frame_map;
mod gear_craft;
mod group_menu;
mod help;
mod history;
pub(crate) mod hud;
mod inventory;
mod level_up;
mod lighting;
mod list_filter;
mod machine_recipe;
mod manifest;
mod marks;
mod meta;
mod mod_copy;
mod notify;
mod outposts;
mod party;
mod perk_bought;
mod points;
mod popup;
mod progression;
mod put_down;
mod refit_copy;
mod research_graph;
mod rig_tool;
mod routine_craft;
mod routines;
mod score;
mod settlement;
mod settlement_board;
mod settlement_market;
mod shortcuts;
mod social;
mod splice_rig;
mod world_map;
pub(crate) use world_map::{WorldMapHit, world_map_hit};
// `pub(crate)` rather than private: `lib.rs::handle_sprite_pointer` needs
// `sprite_forge::SpriteHits` to name the type `sprite_forge::sprite_hits`
// hands back — every other module here stays private because nothing
// outside `render/` ever needed one of its types before the mouse did.
pub(crate) mod sprite_forge;
mod stack;
mod stack_light;
mod stack_market;
mod structure_manifest;
mod tactical;
mod talents;
mod talk;
mod terrain;
#[cfg(test)]
mod test_support;
mod tools;
mod trade;
mod transfer;
mod wildlife;

use arena::{
    draw_arena_builder, draw_arena_load, draw_arena_pick, draw_arena_result, draw_arena_save,
};
use base::draw_playing_base;
// The one cue on the map a test outside `render` has to be able to name:
// arming the tools is `App`-level state and the fixture that reaches base
// space lives beside `App`.
#[cfg(test)]
pub(crate) use base::CUTTING_OUTLINE;
// The click-to-travel geometry: `fx.rs` stores the frame's `MapClickLayout`
// and `lib.rs`'s pointer system inverts a click through `tile_at_px` — both
// outside `render`, so both need the path out.
pub(crate) use base::{MapClickLayout, tile_at_px};
use battle::{
    draw_battle, draw_battle_ally_menu, draw_battle_emulate_menu, draw_battle_item_menu,
    draw_battle_special_menu, draw_battle_target_menu,
};
use breeding::{draw_breed_confirm_menu, draw_breed_menu, draw_breed_second_menu};
use building::{
    build_commit, draw_build_direction, draw_build_menu, draw_build_program, draw_pin_prisoner,
    draw_pin_subject, draw_remove_confirm, draw_remove_menu, draw_siphon, draw_staffing_menu,
    draw_structure_menu, draw_work_order_pick, draw_work_order_quantity,
};
use caravan::{CaravanBasket, draw_caravan};
use contracts::draw_contracts;
use crafting::{draw_compiling, draw_craft_menu, draw_craft_quantity, draw_recipes};
use depot_filter::draw_depot_filter;
use dispatch::{draw_dispatch, draw_route_cargo, draw_sortie_squad};
use excavate_brush::draw_excavate_brush;
use extraction::draw_extraction_options;
use farm_recipe::draw_farm_recipe;
use field::{draw_field_routine, draw_field_routine_ally};
use frame_map::{draw_frame_map, draw_frame_map_cursor, draw_map_inset};
use group_menu::draw_dev_console;
use help::{draw_help_index, draw_help_page};
use history::draw_history;
use inventory::{
    SwapPicker, draw_equip_swap, draw_erase_quantity, draw_gear_inspect,
    draw_inventory_item_action, effect_lines,
};
use machine_recipe::{draw_machine_recipe, draw_machine_recipe_confirm};
use manifest::{ManifestNav, draw_dossier, draw_manifest};
use meta::{
    draw_achievements, draw_escaped, draw_game_menu_page, draw_game_over, draw_load_game,
    draw_main_menu, draw_quit_app_confirm, draw_save_action,
};
use mod_copy::{draw_mod_copy, draw_mod_pick_affix};
use outposts::{draw_outpost_post, draw_outpost_visit};
use party::{
    draw_companion_equip, draw_companion_memories, draw_fuse_menu, draw_fuse_name_menu,
    draw_fuse_second_menu, draw_refactor, draw_refactor_item, draw_rename_menu,
};
use popup::{DIRECTION_KEYS, PopupSize, Row, counted_item_row, draw_popup, key_hints, text_row};
use progression::{draw_research_menu, draw_respec_confirm};
use put_down::draw_put_down;
use refit_copy::draw_refit_copy;
use rig_tool::draw_rig_tool;
use routines::{
    draw_extract, draw_extract_confirm, draw_extract_pick, draw_routine_install,
    draw_routine_target, draw_routines,
};
use settlement::draw_settlement;
use settlement_market::{SettlementMarketBasket, draw_settlement_market};
use splice_rig::draw_splice_rig;
use sprite_forge::{draw_sprite_editor, draw_sprite_picker};
use stack_market::draw_stack_market;
use talents::{draw_develop, draw_develop_program};
use trade::{
    draw_trade_action_menu, draw_trade_menu, draw_trade_program_confirm, draw_trade_quantity_menu,
};
use transfer::draw_transfer;

/// The colour `draw` clears the window to before dispatching to a screen.
/// Named so the sprite editor and `canvas.rs` can paint a transparent canvas
/// pixel in the window's own background rather than a hardcoded black the player could
/// mistake for a painted dark pixel.
const SCREEN_BG: Color = Color::new(0.02, 0.02, 0.03, 1.0);
const PANEL_BG: Color = Color::new(0.06, 0.07, 0.10, 0.95);
/// Drawn behind a full-screen notice's panel. Light enough that the run shows
/// through around the notice, dark enough that the map does not compete with it.
const SCRIM: Color = Color::new(0.02, 0.02, 0.03, 0.55);
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
/// `tuning/`.
const CRITICAL_HP_DIVISOR: i32 = 3;

/// Whether a program is close enough to deletion to warrant the warning
/// colour. The single definition both the battle pane and the roster
/// call, so the threshold cannot come to mean two different things on two
/// screens.
pub(super) fn hp_critical(hp: i32, max_hp: i32) -> bool {
    max_hp > 0 && hp * CRITICAL_HP_DIVISOR <= max_hp
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

/// Whether the player's own drawing stands on their cell — the top rung of
/// the same ladder on the map and on the battle board, so the two cannot
/// disagree about when a player is wearing it. An emulating player draws
/// the image's sprite instead, never the drawing: the two would substitute
/// for the same glyph on the same cell.
pub(super) fn wears_drawing(
    look: Option<&feral_processes_engine::views::PlayerLook>,
    emulating: bool,
) -> bool {
    !emulating && look.is_some_and(|look| look.icon.is_some())
}

/// One row of the image picker (`Mode::BattleEmulate`/
/// `Mode::TacticalEmulate`, todo #100 Task 6), shared so the group model's
/// `draw_battle_emulate_menu` and the battle map's `draw_tactical_emulate`
/// cannot drift on it. `labelled` is the battle screen's `menu_shortcut(i)`
/// prefix; the tactical board picks by highlight alone and prints none.
pub(super) fn emulation_row_label(i: usize, option: &EmulationOption, labelled: bool) -> String {
    let body = format!(
        "{} — ATK {} MIT {}",
        option.name, option.atk, option.mitigation
    );
    if labelled {
        format!("[{}] {body}", menu_shortcut(i))
    } else {
        body
    }
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
        // A tantrum takes the alert hue a sweep does: both are the base
        // coming apart while the player is looking somewhere else.
        MessageKind::Raid | MessageKind::EnemySpecial | MessageKind::Tantrum => ORANGE,
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
        // The battle map and its cursor draw straight on the window with no
        // popup over them, `Mode::FieldRoutineCell`'s reason — so a refusal
        // has nowhere else to land. The routine picker is a popup and takes
        // its refusal inside, like every other list.
        Mode::TacticalBattle
            | Mode::TacticalAim
            | Mode::Battle
            | Mode::BattleResult
            | Mode::FrameMap
            | Mode::WorldMap
            | Mode::FieldRoutineCell
            | Mode::Notification
            | Mode::LevelUp
            | Mode::PerkBought
            | Mode::Ending
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

pub fn draw(app: &mut App, fx: &mut Fx, painter: &Painter, reveal: bool, shortcuts: bool) {
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
        Mode::GameplayOptions
        | Mode::AudioOptions
        | Mode::GraphicsOptions
        | Mode::SpriteOptions
        | Mode::ManualOptions
        | Mode::QuitRunConfirm => draw_game_menu_page(app, refusal, painter, &m),
        Mode::LoadGame => draw_load_game(app, refusal, painter, &m),
        Mode::SaveAction => draw_save_action(app, refusal, painter, &m),
        // The sprite editor hangs off the wizard's Icon step as `App` state
        // rather than a `Mode` of its own, so it is this arm's job to
        // notice it is open and draw over the wizard's own popup instead of
        // drawing it. The view carries no tint, so the avatar wears no hue.
        Mode::CreateCharacter => match app.creation_sprite_view() {
            Some(view) => sprite_forge::draw_sprite_editor_session(&view, app.zoom, painter, &m),
            None => creation::draw_create_character(app, refusal, painter, &m),
        },
        Mode::GameOver => draw_game_over(app, refusal, painter, &m),
        // Drawn over the map rather than over black: the run is still there
        // behind the notice, and it stays in view around the panel.
        // A `None` refusal because this screen is not a `draw_popup`, so it
        // has no status line to put one in
        // — `needs_status_banner` names it, so a refusal raised underneath
        // still reaches the strip along the bottom.
        Mode::Notification => {
            draw_playing_base(app, fx, None, painter, &m, reveal);
            match &app.pending_notification {
                Some(note) => notify::draw_notification(note, painter, &m),
                // The mode is only ever entered with a subject, so this is
                // unreachable — but a blank window would be a soft lock the
                // player cannot read their way out of, and the map is not.
                None => draw_mode_overlay(app, None, painter, &m),
            }
        }
        // `Mode::Notification`'s own arrangement, one vendor over: drawn
        // over the map rather than over black, no `draw_popup` (no scroll,
        // no refusal of its own), and `needs_status_banner` carries any
        // refusal raised underneath it.
        Mode::LevelUp => {
            draw_playing_base(app, fx, None, painter, &m, reveal);
            match &app.pending_level_up {
                Some(report) => level_up::draw_level_up(report, painter, &m),
                // Only ever entered with a report, `Mode::Notification`'s
                // own unreachable-in-practice fallback.
                None => draw_mode_overlay(app, None, painter, &m),
            }
        }
        // `Mode::LevelUp`'s arrangement exactly.
        Mode::PerkBought => {
            draw_playing_base(app, fx, None, painter, &m, reveal);
            match &app.pending_perk_report {
                Some(report) => perk_bought::draw_perk_bought(report, painter, &m),
                None => draw_mode_overlay(app, None, painter, &m),
            }
        }
        // `Mode::PerkBought`'s arrangement: over the map, no popup, and
        // `needs_status_banner` carries a refusal.
        Mode::Ending => {
            draw_playing_base(app, fx, None, painter, &m, reveal);
            ending::draw_ending(&app.ending_screens, app.ending_page, painter, &m);
        }
        Mode::Escaped => {
            draw_playing_base(app, fx, None, painter, &m, reveal);
            draw_escaped(app, refusal, painter, &m);
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
        Mode::BattleEmulate => {
            draw_battle(app, fx, painter, &m);
            draw_battle_emulate_menu(app, refusal, painter, &m);
        }
        Mode::Help => {
            draw_playing_base(app, fx, None, painter, &m, reveal);
            draw_help_index(app, refusal, painter, &m);
        }
        Mode::HelpPage => {
            draw_playing_base(app, fx, None, painter, &m, reveal);
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
                draw_playing_base(app, fx, None, painter, &m, reveal);
                draw_mode_overlay(app, None, painter, &m);
            }
        },
        // Full-pane like `Mode::FrameMap`: a chunk-scale map you have to peer
        // around a popup to read is not a map. A refusal (C on a nest) lands
        // on `needs_status_banner`'s strip.
        Mode::WorldMap => {
            let view = app.world_map();
            let pointing = app
                .game
                .as_mut()
                .and_then(|g| g.compass_bearing())
                .map(|r| r.target);
            match view {
                Some(view) => world_map::draw_world_map(
                    &view,
                    app.menu_selected,
                    app.world_map_list_focused,
                    pointing,
                    painter,
                    &m,
                ),
                None => {
                    draw_playing_base(app, fx, None, painter, &m, reveal);
                    draw_mode_overlay(app, None, painter, &m);
                }
            }
        }
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
                    draw_playing_base(app, fx, None, painter, &m, reveal);
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
        // Full-pane, like `Mode::FrameMap`, and neither draws a popup box to
        // put a refusal in, so both take `needs_status_banner`'s door
        // instead of a `refusal` argument here. Reached from the main menu
        // (`app.game` is `None`) *and*, since the dev keypad grew a row for
        // it, from the map mid-run — so neither arm may read `app.game`,
        // and neither does: the picker's rows come off the asset tree and
        // the editor's canvas off `App` itself.
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
            draw_playing_base(app, fx, on_the_map, painter, &m, reveal);
            draw_mode_overlay(app, refusal, painter, &m);
        }
    }
    if shortcuts && let Some(sheet) = app.shortcut_sheet() {
        shortcuts::draw_shortcuts(&sheet, painter, &m);
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
/// The base half is `Game::base_stock`, the same walk the stock block over
/// the map lists, so the two figures cannot disagree.
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

fn draw_mode_overlay(app: &mut App, refusal: Option<&str>, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    // Cloned off `app` before `app.game` is borrowed, `command_rows`' reason,
    // and only where it is read: a preview is a few numbers per perk.
    let perk_previews = match app.mode {
        Mode::Perks => app.perk_previews.clone(),
        _ => Vec::new(),
    };
    let graph_view = app.research_graph_view;
    let pending_manifest = app.pending_manifest;
    let manifest_tab = app.manifest_tab;
    let manifest_is_own = app.manifest_is_own();
    let level_up_reopenable = app.level_up_reopenable();
    let pending_field_routine = app.pending_field_routine;
    let pending_downed_program = app.pending_downed_program_index;
    let pending_structure = app.pending_structure.clone();
    let build_category = app.build_category;
    let pending_item = app.pending_inventory_item.clone();
    let mod_copy = app.mod_copy.clone();
    let refit_copy = app.refit_copy.clone();
    let pending_inspect = app.pending_inspect.clone();
    // Rows off `app` before `app.game` is borrowed, `command_rows`' reason.
    let allocation_rows = match app.mode {
        Mode::AllocateStats => points::allocate_stats_rows(app),
        _ => Vec::new(),
    };
    let allocation_title = match app.mode {
        Mode::AllocateStats => points::allocate_stats_title(app),
        _ => String::new(),
    };
    let staffing = match app.mode {
        Mode::StructureAssign => app.staffing(),
        _ => None,
    };
    // Through `App::contract_sections` and not off the engine directly, for
    // `command_rows`' reason one line up: the handler resolves a row number
    // against these two lists, so a renderer with its own copy would act on a
    // different contract from the one under the highlight.
    let (contract_active, contract_offers, contract_reach) = match app.mode {
        Mode::Contracts => {
            let (active, offers) = app.contract_sections();
            (active, offers, app.broker_reach())
        }
        _ => (Vec::new(), Vec::new(), BrokerReach::NoBroker),
    };
    // The town board's own pair, `contract_sections`' rule one vendor over.
    let (town_active, town_offers) = match app.mode {
        Mode::SettlementBoard => app.settlement_board_sections(),
        _ => (Vec::new(), Vec::new()),
    };
    let scanned = match app.mode {
        Mode::WorkStructure => app.workable_structures(),
        Mode::Remove => app.nearby_structures(),
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
        Mode::BaseStaff => Some(app.base_staff_table()),
        _ => None,
    };
    // `command_rows` reads the party's surroundings, so like the lists above
    // it is taken before `game` borrows the app.
    let commands = app.command_rows(app.mode);
    let command_counts: Vec<(usize, usize)> = match app.mode {
        Mode::BaseOverview => base_report::command_pages()
            .filter_map(|(page, mode)| Some((page, app.command_rows(mode)?.len())))
            .collect(),
        _ => Vec::new(),
    };
    let list_filter = app.filter_text().to_owned();
    let app_screens = base_report::AppScreens {
        list_filter: &list_filter,
        commands: commands.as_deref(),
        command_counts: &command_counts,
        work_orders: &work_orders,
        base_staff: base_staff.as_ref(),
        work_column: app.work_column,
        on_rail: app.hub_on_rail,
        perk_previews: &perk_previews,
    };
    // The row's two holdings and the amount the basket is asking for, taken
    // here because `game` below borrows `&mut app.game` and these read the
    // rest of the `App`. The screen projects the holdings against the amount
    // rather than drawing the two ceilings: `App::put_available` is what the
    // *keys* clamp against and never something the player reads.
    // Cloned out for `transfer_entries`' reason: the screen reads `App`,
    // and `game` below borrows `&mut app.game`. It is one Depot's rows, so
    // the clone is the item catalogue once per frame rather than per row.
    let depot_filter = match app.mode {
        Mode::DepotFilter => app.depot_filter.clone(),
        _ => None,
    };
    let siphon = app.siphon;
    let routine_draft = app.routine_draft.clone();
    let design_draft = app.design_draft.clone();
    let splice_rig = match app.mode {
        Mode::SpliceRig => app.splice_rig.clone(),
        _ => None,
    };
    let rig_tool = match app.mode {
        Mode::RigTool => app.rig_tool.clone(),
        _ => None,
    };
    let machine_recipe = match app.mode {
        Mode::MachineRecipe | Mode::MachineRecipeConfirm => app.machine_recipe.clone(),
        _ => None,
    };
    let excavate_brush = match app.mode {
        Mode::ExcavateBrush => app.excavate_brush_rows(),
        _ => Vec::new(),
    };
    let farm_recipe = match app.mode {
        Mode::FarmRecipe => app.farm_recipe_rows(),
        _ => Vec::new(),
    };
    let put_down = match app.mode {
        Mode::PutDown => app.put_down_rows(),
        _ => Vec::new(),
    };
    // The name is resolved here rather than in the renderer, because a
    // carrier row has no `ItemId` to look one up by: its label is the
    // engine's own sentence, and the table is a table of *names* either way.
    let transfer_entries: Vec<(String, i64, u32, u32)> = match app.mode {
        Mode::Transfer => app
            .basket_rows
            .iter()
            .enumerate()
            .filter_map(|(row, entry)| {
                let amount = app.basket_amounts.get(row).copied().unwrap_or(0);
                let game = app.game.as_ref()?;
                Some(match entry {
                    TransferEntry::Item(r) => (
                        game.item_name(&r.item).to_string(),
                        amount,
                        r.carried,
                        r.on_shelves,
                    ),
                    // A carrier's two columns are 1/0 or 0/1 — which side it
                    // is standing on — and `projected` moves it across from
                    // there exactly as it moves a unit of an item.
                    TransferEntry::Carrier(c) => (
                        c.label.clone(),
                        amount,
                        u32::from(!c.racked),
                        u32::from(c.racked),
                    ),
                })
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
    // The settlement's own shelf, and each row's basket cell beside it —
    // `caravan`'s shape exactly, one door over: `App::settlement_ceiling`
    // takes `&self`, so it has to run before `game` below takes `&mut
    // app.game`.
    let settlement_market = match app.mode {
        Mode::SettlementMarket => app.pending_settlement.and_then(|key| {
            app.game
                .as_mut()
                .and_then(|g| g.settlement_view(key))
                .map(|view| {
                    let cells = (0..view.offers.len() + view.sells.len())
                        .map(|row| {
                            (
                                app.settlement_amounts.get(row).copied().unwrap_or(0),
                                app.settlement_ceiling(&view, row),
                            )
                        })
                        .collect();
                    let purse = app.settlement_purse_after(&view);
                    SettlementMarketBasket { view, cells, purse }
                })
        }),
        _ => None,
    };
    // The hub's own two sections, `settlement_market`'s reason one rung up:
    // `App::dispatch_hub_sections`/`dispatch_trip_reports` take `&mut self`,
    // so they have to run before `game` below takes `&mut app.game`.
    let dispatch_sections = match app.mode {
        Mode::Dispatch => app.dispatch_hub_sections(),
        _ => None,
    };
    let dispatch_trips = match app.mode {
        Mode::Dispatch => app.dispatch_trip_reports(),
        _ => (Vec::new(), Vec::new()),
    };
    let sortie_squad_site = match app.mode {
        Mode::SortieSquad => app.sortie_squad_site(),
        _ => None,
    };
    let sortie_squad_candidates = match app.mode {
        Mode::SortieSquad => app.sortie_squad_candidates(),
        _ => Vec::new(),
    };
    let sortie_squad_cost = match app.mode {
        Mode::SortieSquad => app.sortie_squad_cost(),
        _ => Vec::new(),
    };
    let route_cargo_basket = match app.mode {
        Mode::RouteCargo => app.route_cargo_basket(),
        _ => None,
    };
    // What the program picker is about to spend a program on. Read here for
    // `scanned`'s reason — an upgrade's structure is named off
    // `App::upgradeable_structures`, which takes `&mut self` and so cannot
    // run once `game` below holds `&mut app.game`.
    let build_commit = match app.mode {
        Mode::BuildProgram => build_commit(app),
        _ => None,
    };
    // Read before `game` takes the whole of `app`, as the rows above are:
    // the figure is derived from more than one field, so the borrow cannot
    // be split at the call.
    let craft_quantity = app.craft_quantity();
    // `App::outpost_post_candidates` takes `&self`, `sortie_squad_candidates`'s
    // own reason for running before `game` below takes `&mut app.game`.
    let outpost_post_candidates = match app.mode {
        Mode::OutpostPost => app.outpost_post_candidates(),
        _ => Vec::new(),
    };
    // The frame the current mode sits in, and the one under a popup child,
    // taken off the `App` for the reason the rows above are.
    let hub_frame = base_report::HubFrame::of(app, app.mode);
    let popup_parent = base_popup_parent(app.mode).and_then(|p| base_report::HubFrame::of(app, p));
    let on_rail = app.hub_on_rail;
    let craft_at_bench = app.craft_at_bench();
    // Read ahead of `game` for the reason the rows above are. The two dev
    // pickers' rows are views off `&App`, taken only on their own modes.
    let dev_console = app.dev_console_enabled();
    let dev_item_rows = match app.mode {
        Mode::DevItemPicker => app.dev_item_rows(),
        _ => Vec::new(),
    };
    let dev_zone_rows = match app.mode {
        Mode::DevZonePicker => app.dev_zone_rows(),
        _ => Vec::new(),
    };
    let dev_row_labels: Vec<String> = match app.mode {
        Mode::DevConsole => App::dev_console_rows()
            .iter()
            .map(|r| app.dev_row_label(r))
            .collect(),
        _ => Vec::new(),
    };
    let Some(game) = &mut app.game else { return };
    // A hub page whose screen is also a popup: the same rows, in the hub's
    // frame instead. Every other hub screen is drawn by `draw_base_report`,
    // further down.
    let page_rows = hub_frame.as_ref().and_then(|_| match app.mode {
        Mode::Transfer => Some((
            transfer::body_rows(&transfer_entries, app.basket_room, selected),
            None,
        )),
        Mode::DepotFilter => Some((
            depot_filter::page_rows(depot_filter.as_ref(), selected),
            None,
        )),
        Mode::MachineRecipe => Some((
            machine_recipe::page_rows(machine_recipe.as_ref(), selected),
            None,
        )),
        Mode::RigTool => Some((rig_tool::page_rows(rig_tool.as_ref(), selected), None)),
        Mode::Siphon => Some((building::siphon_page_rows(game, siphon, selected), None)),
        Mode::Craft => Some((
            crafting::craft_page_rows(game, craft_at_bench, &list_filter, selected),
            None,
        )),
        Mode::RoutineBuild => Some((
            routine_craft::build_page_rows(game, &routine_draft, selected),
            None,
        )),
        Mode::BuiltRoutines => Some((routine_craft::built_page_rows(game, selected), None)),
        Mode::WeaponBuild => Some((
            gear_craft::weapon_build_page_rows(game, &design_draft, selected),
            None,
        )),
        Mode::WeaponSalvage => Some((gear_craft::salvage_page_rows(game, selected), None)),
        Mode::SpliceRig => Some((splice_rig::page_rows(splice_rig.as_ref(), selected), None)),
        Mode::FarmRecipe => Some((farm_recipe::body_rows(&farm_recipe, selected), None)),
        Mode::BuildProgram => Some(building::build_program_page(
            game,
            build_commit.as_ref(),
            selected,
        )),
        Mode::StructureManifest => Some((
            structure_manifest::structure_rows(game, app.pending_structure_manifest)
                .unwrap_or_else(|| {
                    vec![
                        text_row("That structure is gone."),
                        text_row(""),
                        text_row("Esc back"),
                    ]
                }),
            None,
        )),
        _ => None,
    });
    if let (Some(frame), Some((rows, nothing_qualifies))) = (&hub_frame, page_rows) {
        base_report::draw_hub_page(
            frame,
            app.mode,
            &rows,
            on_rail,
            refusal.or(nothing_qualifies.as_deref()),
            painter,
            m,
        );
        return;
    }
    match app.mode {
        Mode::DevConsole => draw_dev_console(&dev_row_labels, selected, refusal, painter, m),
        Mode::DevItemPicker => dev_pickers::draw_dev_item_picker(
            &dev_item_rows,
            &list_filter,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::DevZonePicker => {
            dev_pickers::draw_dev_zone_picker(&dev_zone_rows, selected, refusal, painter, m)
        }
        Mode::Build => draw_build_menu(game, build_category, selected, refusal, painter, m),
        Mode::BuildDirection => {
            draw_build_direction(game, pending_structure.as_deref(), refusal, painter, m)
        }
        Mode::BuildProgram => draw_build_program(game, build_commit, selected, refusal, painter, m),
        Mode::PinSubject => draw_pin_subject(game, selected, refusal, painter, m),
        Mode::PinPrisoner => draw_pin_prisoner(game, selected, refusal, painter, m),
        Mode::Siphon => draw_siphon(game, siphon, selected, refusal, painter, m),
        Mode::RoutineBuildPick => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            routine_craft::draw_pick(game, &routine_draft, selected, refusal, painter, m)
        }
        Mode::RoutineBuildName => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            routine_craft::draw_name(&routine_draft, refusal, painter, m)
        }
        Mode::WeaponBuildPick => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            gear_craft::draw_pick(
                game,
                &design_draft,
                &list_filter,
                selected,
                refusal,
                painter,
                m,
            )
        }
        Mode::WeaponBuildName => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            gear_craft::draw_name(&design_draft, refusal, painter, m)
        }
        Mode::Transfer => draw_transfer(
            &transfer_entries,
            app.basket_room,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::DepotFilter => {
            draw_depot_filter(depot_filter.as_ref(), selected, refusal, painter, m)
        }
        Mode::RigTool => draw_rig_tool(rig_tool.as_ref(), selected, refusal, painter, m),
        Mode::MachineRecipe => {
            draw_machine_recipe(machine_recipe.as_ref(), selected, refusal, painter, m)
        }
        Mode::MachineRecipeConfirm => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            draw_machine_recipe_confirm(machine_recipe.as_ref(), selected, refusal, painter, m)
        }
        Mode::ExcavateBrush => draw_excavate_brush(&excavate_brush, selected, refusal, painter, m),
        Mode::FarmRecipe => draw_farm_recipe(&farm_recipe, selected, refusal, painter, m),
        Mode::PutDown => draw_put_down(&put_down, &list_filter, selected, refusal, painter, m),
        Mode::SpliceRig => draw_splice_rig(splice_rig.as_ref(), selected, refusal, painter, m),
        Mode::Craft => draw_craft_menu(
            game,
            craft_at_bench,
            &list_filter,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::CraftQuantity => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            draw_craft_quantity(
                game,
                app.pending_craft.clone(),
                craft_quantity,
                app.careful_craft,
                refusal,
                painter,
                m,
            )
        }
        Mode::Compiling => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            draw_compiling(game, app.compile_progress.as_ref(), refusal, painter, m)
        }
        Mode::EraseQuantity => draw_erase_quantity(
            game,
            app.pending_erase.clone(),
            &app.erase_quantity_input,
            refusal,
            painter,
            m,
        ),
        Mode::WorkOrderPick => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            draw_work_order_pick(&orderable, selected, refusal, painter, m)
        }
        Mode::WorkOrderQuantity => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            draw_work_order_quantity(
                game,
                app.pending_order.clone(),
                &app.order_quantity_input,
                app.standing_order,
                refusal,
                painter,
                m,
            )
        }
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
        // Says the crew does the work, because the direction prompt is now
        // the whole of the screen the player decides on: the list this
        // replaced carried that sentence above its rows.
        Mode::UpgradeDirection => draw_direction_prompt(
            "Upgrade direction",
            "Upgrade which neighbour? Your crew fetches the parts and does \
             the work.",
            refusal,
            painter,
            m,
        ),
        Mode::InspectDirection => draw_direction_prompt(
            "Inspect direction",
            "Choose a direction to inspect",
            refusal,
            painter,
            m,
        ),
        // Names the cascade the way the picker's own header does: this route
        // reaches Home in one keypress, so the warning cannot wait for the
        // confirmation screen to be the first mention of it.
        Mode::RemoveDirection => draw_direction_prompt(
            "Demolish direction",
            "Demolish which neighbour? Removing Home destroys the whole base.",
            refusal,
            painter,
            m,
        ),
        Mode::TrapDirection => draw_direction_prompt(
            "Place direction",
            "Place the honeypot which way?",
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
                // Only advertise `W` when it will work, `cyclable`'s rule:
                // the roster reaches every program you own, including the
                // ones whose tile is the one they were beaten on.
                watchable: pending_manifest.is_some_and(|e| game.watch_position(e).is_some()),
                tab: manifest_tab,
                level_up: manifest_is_own.then_some(level_up_reopenable),
            };
            draw_manifest(game, pending_manifest, nav, refusal, painter, m)
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
        Mode::Settlement => draw_settlement(game, app.pending_settlement, refusal, painter, m),
        Mode::SettlementBoard => settlement_board::draw_settlement_board(
            game,
            app.pending_settlement,
            (&town_active, &town_offers),
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::SettlementMarket => draw_settlement_market(
            game,
            app.pending_settlement,
            settlement_market,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::OutpostVisit => {
            draw_outpost_visit(game, app.pending_outpost, selected, refusal, painter, m)
        }
        Mode::OutpostPost => {
            draw_outpost_post(&outpost_post_candidates, selected, refusal, painter, m)
        }
        Mode::CompanionEquip => draw_companion_equip(
            game,
            app.pending_equip_program,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::CompanionMemories => draw_companion_memories(
            game,
            app.pending_memory_program,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::Dossier => draw_dossier(game, app.pending_manifest, refusal, painter, m),
        Mode::EquipSwap => draw_equip_swap(
            game,
            SwapPicker {
                slot: app.pending_swap_slot,
                target: app.pending_swap_target,
                filter: &list_filter,
            },
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
        Mode::DownedPrograms if pending_downed_program.is_some() => draw_extraction_options(
            game,
            pending_downed_program.unwrap_or_default(),
            selected,
            refusal,
            painter,
            m,
        ),
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
        Mode::Breed => draw_breed_menu(game, selected, refusal, painter, m),
        Mode::BreedSecond => {
            draw_breed_second_menu(game, app.pending_breed_first, selected, refusal, painter, m)
        }
        Mode::BreedConfirm => draw_breed_confirm_menu(
            game,
            app.pending_breed_first,
            app.pending_breed_second,
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
        Mode::TacticalResult => {
            tactical::draw_tactical_result(&game.battle_outcomes(), refusal, painter, m)
        }
        Mode::TacticalItem => {
            tactical::draw_tactical_items(game, &list_filter, selected, refusal, painter, m)
        }
        Mode::TacticalRoutine => tactical::draw_tactical_routines(
            &game.tactical_routine_options(),
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::TacticalEmulate => tactical::draw_tactical_emulate(
            &game.emulation_options(),
            selected,
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
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::StackMarket => draw_stack_market(game, &list_filter, selected, refusal, painter, m),
        Mode::Caravan => draw_caravan(game, caravan, selected, refusal, painter, m),
        Mode::Dispatch => draw_dispatch(
            dispatch_sections
                .as_ref()
                .map(|(sites, destinations)| (sites.as_slice(), destinations.as_slice())),
            &dispatch_trips.0,
            &dispatch_trips.1,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::SortieSquad => draw_sortie_squad(
            sortie_squad_site.as_ref(),
            &sortie_squad_candidates,
            &sortie_squad_cost,
            selected,
            refusal,
            painter,
            m,
        ),
        Mode::RouteCargo => draw_route_cargo(route_cargo_basket, selected, refusal, painter, m),
        Mode::Trade => draw_trade_menu(game, selected, refusal, painter, m),
        Mode::TradeAction => draw_trade_action_menu(
            game,
            app.pending_trade_structure,
            &list_filter,
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
                selected,
                refusal,
                painter,
                m,
            )
        }
        Mode::AllocateStats => {
            points::draw_allocate_stats(&allocation_title, &allocation_rows, refusal, painter, m)
        }
        Mode::RespecPerksConfirm => {
            let quote = game.respec_quote(RespecSubject::Perks);
            draw_respec_confirm(&quote, "perk", selected, refusal, painter, m)
        }
        Mode::RespecTalentsConfirm => {
            if let Some(target) = app.pending_develop_target {
                let quote = game.respec_quote(RespecSubject::Talents(target));
                draw_respec_confirm(&quote, "talent", selected, refusal, painter, m);
            }
        }
        Mode::Research if graph_view => research_graph::draw_research_graph(
            game,
            ResearchTree::Base,
            selected,
            refusal,
            painter,
            m,
            dev_console,
        ),
        Mode::Research => draw_research_menu(
            game,
            ResearchTree::Base,
            selected,
            refusal,
            painter,
            m,
            dev_console,
        ),
        Mode::RoutineResearch if graph_view => research_graph::draw_research_graph(
            game,
            ResearchTree::Routines,
            selected,
            refusal,
            painter,
            m,
            dev_console,
        ),
        Mode::RoutineResearch => draw_research_menu(
            game,
            ResearchTree::Routines,
            selected,
            refusal,
            painter,
            m,
            dev_console,
        ),
        Mode::AffixResearch if graph_view => research_graph::draw_research_graph(
            game,
            ResearchTree::Affixes,
            selected,
            refusal,
            painter,
            m,
            dev_console,
        ),
        Mode::AffixResearch => draw_research_menu(
            game,
            ResearchTree::Affixes,
            selected,
            refusal,
            painter,
            m,
            dev_console,
        ),
        Mode::WeaponResearch if graph_view => research_graph::draw_research_graph(
            game,
            ResearchTree::Weapons,
            selected,
            refusal,
            painter,
            m,
            dev_console,
        ),
        Mode::WeaponResearch => draw_research_menu(
            game,
            ResearchTree::Weapons,
            selected,
            refusal,
            painter,
            m,
            dev_console,
        ),
        Mode::ModCopy => draw_mod_copy(game, mod_copy.clone(), selected, refusal, painter, m),
        Mode::ModPickAffix => {
            draw_mod_pick_affix(game, mod_copy.clone(), selected, refusal, painter, m)
        }
        Mode::RefitCopy => draw_refit_copy(game, refit_copy.clone(), refusal, painter, m),
        Mode::Contracts => draw_contracts(
            &contract_active,
            &contract_offers,
            contract_reach,
            selected,
            refusal,
            painter,
            m,
        ),
        _ if hub_frame.is_some() => {
            if let Some(frame) = &hub_frame {
                base_report::draw_base_report(
                    game,
                    frame,
                    selected,
                    &app_screens,
                    refusal,
                    painter,
                    m,
                )
            }
        }
        Mode::History => draw_history(game, selected, refusal, painter, m),
        Mode::Compass => compass::draw_compass(game, selected, refusal, painter, m),
        Mode::StructureAssign => {
            base_report::draw_popup_parent(popup_parent.as_ref(), painter, m);
            if let Some(staffing) = &staffing {
                draw_staffing_menu(staffing, selected, refusal, painter, m);
            }
        }
        Mode::Recipes => draw_recipes(game, selected, refusal, painter, m),
        Mode::BasinExitConfirm => ending::draw_basin_exit_confirm(selected, refusal, painter, m),
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
        &[
            text_row(body),
            text_row(""),
            text_row(key_hints(DIRECTION_KEYS)),
        ],
        refusal,
        painter,
        m,
    );
}

/// A program's current activity as a bracketed suffix — `" (in party)"`,
/// `" (Extractor)"`, `" (guarding Data Cache)"`, `" (idle)"`. The wording
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
        short_name: name.to_string(),
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
        assembly: None,
        extraction: None,
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
mod tests;
