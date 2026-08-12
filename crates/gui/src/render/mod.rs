//! All drawing for the graphics frontend: one screen per `Mode`, laid out
//! in immediate mode against `Painter` (filled rects for bars and tiles,
//! drawn text for menus). Reads engine data through `App` and never touches
//! the ECS `World`.

use crate::fx::Fx;
use crate::paint::{Color, GRAY, Painter, Rect, TextRun, WHITE};
use crate::text::{Metrics, map_cell, ui_metrics};
use feral_processes_app_core::{
    App, ArenaRow, DevConsoleRow, GroupMenuRow, LogFilter, MENU_SCAN_RADIUS, Mode, TradeChoice,
    equip_preview_tag, equip_swap_rows, inventory_item_actions, item_fusion_note, menu_shortcut,
    stat_summary,
};
use feral_processes_engine::components::{GlyphColor, MachineStatus, Rarity, TaskKind};
use feral_processes_engine::items::{EquipmentSlot, ItemId};
use feral_processes_engine::structures::StructureCategory;
use feral_processes_engine::tuning::{MAX_COMPANION_REFACTORS, MAX_FUSIONS, MAX_PARTY_SIZE};
use feral_processes_engine::world::{Biome, Tile};
use feral_processes_engine::{
    Assignee, CraftRecipe, Entity, EntityView, Game, InventoryRow, LogEntry, MESSAGE_LOG_CAP,
    MessageKind, PetInfo, ProgramSaleOption, RecipeChain, RecipeStep, ResearchState,
    StructureReport,
};

mod arena;
mod bars;
mod base;
mod battle;
mod building;
mod crafting;
mod field;
mod frame_map;
mod group_menu;
mod inventory;
mod manifest;
mod manifest_layout;
mod meta;
mod party;
mod popup;
mod progression;
mod routines;
mod stack;
mod structure_manifest;
mod trade;

use arena::{
    draw_arena_builder, draw_arena_load, draw_arena_pick, draw_arena_result, draw_arena_save,
};
use base::{draw_history, draw_playing_base};
use battle::{
    draw_battle, draw_battle_ally_menu, draw_battle_item_menu, draw_battle_special_menu,
    draw_battle_target_menu,
};
use building::{
    draw_build_direction, draw_build_menu, draw_remove_confirm, draw_remove_menu,
    draw_structure_menu, draw_structures, draw_symlink_menu, draw_upgrade_menu, draw_worker_menu,
};
use crafting::{draw_craft_menu, draw_craft_quantity, draw_recipes};
use field::{draw_field_cast, draw_field_cast_ally};
use frame_map::{draw_frame_map, draw_frame_map_cursor, draw_map_inset};
use group_menu::{draw_dev_console, draw_group_menu};
use inventory::{
    draw_equip_swap, draw_erase_quantity, draw_inventory, draw_inventory_item_action,
    draw_item_describe,
};
use manifest::{ManifestNav, draw_manifest, draw_manifest_pick};
use meta::{
    draw_achievements, draw_difficulty_pick, draw_game_over, draw_help, draw_load_game,
    draw_main_menu, draw_quit_app_confirm, draw_quit_run_confirm, draw_save_action,
};
use party::{
    draw_companion_equip, draw_companion_menu, draw_fuse_menu, draw_fuse_name_menu,
    draw_fuse_second_menu, draw_refactor, draw_refactor_item, draw_rename_menu,
};
use popup::{PopupSize, Row, counted_item_row, draw_popup, text_row};
use progression::{draw_perks_menu, draw_research_menu};
use routines::{
    draw_extract, draw_extract_confirm, draw_extract_pick, draw_routine_install,
    draw_routine_target, draw_routines,
};
use trade::{
    draw_trade_action_menu, draw_trade_menu, draw_trade_program_confirm, draw_trade_quantity_menu,
};

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

/// What colour a menu row draws in for something that has been fused —
/// cyan while it can still be an input to another fusion, magenta once it
/// is at `MAX_FUSIONS` and is a finished product. `None` leaves the row's
/// ordinary colour alone.
///
/// Programs and gear both call this, because both stop at the same
/// ceiling: `components::FusionCount` for a program, `FusedGear` (or a
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

/// What colour a rare-spawn tier draws in, or `None` for an ordinary
/// creature. Silver and gold, which is what the tiers are named after
/// internally even though a player only ever reads Optimized/Overclocked.
///
/// **The map's tile bar and every menu row call this same function**, so a
/// program cannot read as one colour on the grid and another on its own
/// row — the argument `fusion_color` above makes about programs and gear,
/// applied to the two places a tier is shown.
pub(super) fn rarity_color(rarity: Rarity) -> Option<Color> {
    match rarity {
        Rarity::Ordinary => None,
        Rarity::Silver => Some(SILVER),
        Rarity::Gold => Some(GOLD),
    }
}

/// The one colour rule for a program's menu row, resolving the only case
/// where two permanent properties both want the same channel.
///
/// **Fusion outranks rarity**, extending the chain `fusion_color`'s doc
/// starts to `CRITICAL > fusion > rarity > plain`. The same argument
/// applies one step down: on the fuse pickers, cyan-versus-magenta is the
/// read for *can this still be an input*, a question about an action
/// available right now, while a rare tier is never actionable — it is worth
/// knowing, not worth deciding on. A fused Overclocked program keeps the
/// tier in its name and gives up only the colour.
///
/// Gear keeps calling `fusion_color` directly, because gear has no tier.
pub(super) fn program_color(fusions: u32, rarity: Rarity) -> Option<Color> {
    fusion_color(fusions).or_else(|| rarity_color(rarity))
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
    match c {
        GlyphColor::White => WHITE,
        GlyphColor::Gray => GRAY,
        GlyphColor::Green => GREEN,
        GlyphColor::DarkGreen => Color::new(0.0, 0.4, 0.0, 1.0),
        GlyphColor::Red => RED,
        GlyphColor::Yellow => YELLOW,
        GlyphColor::Blue => BLUE,
        GlyphColor::Magenta => MAGENTA,
        GlyphColor::Cyan => CYAN,
        GlyphColor::Brown => Color::new(0.55, 0.27, 0.07, 1.0),
        GlyphColor::Orange => Color::new(1.0, 0.55, 0.0, 1.0),
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
        MessageKind::LevelUp => GREEN,
        MessageKind::Raid | MessageKind::EnemySpecial => ORANGE,
        MessageKind::EnemyAttack => RED,
        // A result reads at full brightness: it is the line still on screen
        // once the fight is over and the map is back.
        MessageKind::Outcome => TEXT,
    }
}

/// Draws one log line in its kind's style.
///
/// A party member's hit is the one line styled unevenly: how hard it landed is
/// the part worth reading twice, so the number takes the emphasis and the
/// narration around it stays as quiet as any other chatter.
fn draw_message_line(
    kind: MessageKind,
    text: &str,
    x: f32,
    y: f32,
    painter: &Painter,
    m: &Metrics,
) {
    let color = message_color(kind);
    match kind {
        MessageKind::PartyDamage => {
            painter.ui_runs(&emphasize_numbers(text, color, WHITE), x, y, m.font_size)
        }
        MessageKind::LevelUp => painter.ui_bold(text, x, y, m.font_size, color),
        _ => painter.ui(text, x, y, m.font_size, color),
    }
}

/// Whether `mode` needs `App::status_line` redrawn on top of whatever it
/// just drew. `Playing` already shows it in the log pane, and the main-menu
/// and save popups carry it as a row inside the panel; every other mode
/// covers the log pane with a popup, which would otherwise bury the one
/// message explaining why a menu pick was refused.
///
/// `QuitAppConfirm` is exempt because it draws the main menu underneath it,
/// which shows the line itself. `QuitRunConfirm` is not: the banner is how a
/// failed save-and-quit reaches the player, and it is the reason that screen
/// keeps them there instead of leaving anyway.
fn needs_status_banner(mode: Mode) -> bool {
    !matches!(
        mode,
        Mode::Playing | Mode::MainMenu | Mode::SaveAction | Mode::QuitAppConfirm
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
    painter.clear(Color::new(0.02, 0.02, 0.03, 1.0));
    match app.mode {
        Mode::MainMenu => draw_main_menu(app, painter, &m),
        // Drawn over the menu it was opened from, so the row `q` was pressed
        // on stays visible behind the question.
        Mode::QuitAppConfirm => {
            draw_main_menu(app, painter, &m);
            draw_quit_app_confirm(app.menu_selected, painter, &m);
        }
        Mode::Achievements => draw_achievements(app, painter, &m),
        Mode::LoadGame => draw_load_game(app, painter, &m),
        Mode::SaveAction => draw_save_action(app, painter, &m),
        Mode::DifficultyPick => draw_difficulty_pick(app.menu_selected, painter, &m),
        Mode::GameOver => draw_game_over(app, painter, &m),
        Mode::Battle => draw_battle(app, fx, painter, &m),
        Mode::BattleTarget => {
            draw_battle(app, fx, painter, &m);
            draw_battle_target_menu(app, painter, &m);
        }
        Mode::BattleItem => {
            draw_battle(app, fx, painter, &m);
            draw_battle_item_menu(app, painter, &m);
        }
        Mode::BattleSpecial => {
            draw_battle(app, fx, painter, &m);
            draw_battle_special_menu(app, painter, &m);
        }
        Mode::BattleAlly => {
            draw_battle(app, fx, painter, &m);
            draw_battle_ally_menu(app, painter, &m);
        }
        Mode::Help => {
            draw_playing_base(app, fx, painter, &m);
            draw_help(painter, &m);
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
            None => {
                draw_playing_base(app, fx, painter, &m);
                draw_mode_overlay(app, painter, &m);
            }
        },
        // Full-pane for the same reason `Mode::FrameMap` is, and doubly so:
        // picking a cell you have never walked to means seeing the whole
        // frame at once.
        Mode::FieldCastCell => {
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
                _ => {
                    draw_playing_base(app, fx, painter, &m);
                    draw_mode_overlay(app, painter, &m);
                }
            }
        }
        // Their own arms rather than `draw_mode_overlay`'s, because the
        // arena hangs off the main menu: `app.game` is `None` on every one
        // of these, and the map underneath a mode overlay needs a run.
        Mode::ArenaBuilder => {
            draw_arena_builder(&app.arena_builder_rows(), app.menu_selected, painter, &m)
        }
        Mode::ArenaSave => {
            draw_arena_builder(&app.arena_builder_rows(), app.menu_selected, painter, &m);
            draw_arena_save(&app.arena_save_input, painter, &m);
        }
        Mode::ArenaPick => draw_arena_pick(&app.arena_pick_rows(), app.menu_selected, painter, &m),
        Mode::ArenaLoad => draw_arena_load(&app.arena_load_rows(), app.menu_selected, painter, &m),
        Mode::ArenaResult => draw_arena_result(
            app.arena_outcome(),
            app.arena_warnings(),
            app.arena_seed(),
            app.arena_transcript(),
            app.menu_selected,
            painter,
            &m,
        ),
        Mode::BattleResult => draw_battle(app, fx, painter, &m),
        _ => {
            draw_playing_base(app, fx, painter, &m);
            draw_mode_overlay(app, painter, &m);
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
fn cost_display(game: &Game, cost: &[(ItemId, u32)], inventory: &[InventoryRow]) -> Vec<String> {
    cost.iter()
        .map(|(item, qty)| {
            let have = inventory
                .iter()
                .find(|row| &row.item == item && row.tier == 0)
                .map(|row| row.qty)
                .unwrap_or(0);
            format!("{} ({have}/{qty})", game.item_name(item))
        })
        .collect()
}

/// The cargo row `Mode::InventoryItemAction` and `Mode::ItemDescribe` are
/// about, as the `(item, tier)` pair both their screens take.
fn split_pending_item(pending: &Option<(ItemId, u32)>) -> (Option<ItemId>, u32) {
    match pending {
        Some((item, tier)) => (Some(item.clone()), *tier),
        None => (None, 0),
    }
}

fn draw_mode_overlay(app: &mut App, painter: &Painter, m: &Metrics) {
    let selected = app.menu_selected;
    let pending_manifest = app.pending_manifest;
    let manifest_from_picker = app.manifest_from_picker;
    let pending_field_routine = app.pending_field_routine;
    let pending_structure = app.pending_structure.clone();
    let pending_item = app.pending_inventory_item.clone();
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
    let scanned = match app.mode {
        Mode::Cronjob | Mode::Guard => app.nearby_programs(),
        Mode::CronjobStructure | Mode::WorkStructure => app.workable_structures(),
        Mode::GuardStructure | Mode::Remove => app.nearby_structures(),
        Mode::Upgrade => app.upgradeable_structures(),
        _ => Vec::new(),
    };
    let Some(game) = &mut app.game else { return };
    match app.mode {
        Mode::BaseMenu => draw_group_menu(&group_rows, "Base", selected, painter, m),
        Mode::PartyMenu => draw_group_menu(&group_rows, "Party", selected, painter, m),
        Mode::DevConsole => draw_dev_console(App::dev_console_rows(), selected, painter, m),
        Mode::Build => draw_build_menu(game, selected, painter, m),
        Mode::BuildDirection => {
            draw_build_direction(game, pending_structure.as_deref(), painter, m)
        }
        Mode::Craft => draw_craft_menu(game, selected, painter, m),
        Mode::CraftQuantity => draw_craft_quantity(
            game,
            app.pending_craft.clone(),
            &app.craft_quantity_input,
            painter,
            m,
        ),
        Mode::EraseQuantity => draw_erase_quantity(
            game,
            app.pending_erase.clone(),
            &app.erase_quantity_input,
            painter,
            m,
        ),
        Mode::Cronjob => draw_worker_menu(
            game,
            &scanned,
            "Assign Cronjob",
            "Assign which program to a cronjob?",
            selected,
            painter,
            m,
        ),
        Mode::CronjobStructure => draw_structure_menu(
            &scanned,
            "Assign Cronjob",
            "Cronjob which structure?",
            selected,
            painter,
            m,
        ),
        Mode::WorkStructure => draw_structure_menu(
            &scanned,
            "Work",
            "Work which structure yourself?",
            selected,
            painter,
            m,
        ),
        Mode::Guard => draw_worker_menu(
            game,
            &scanned,
            "Assign Guard",
            "Assign which program to guard duty?",
            selected,
            painter,
            m,
        ),
        Mode::GuardStructure => draw_structure_menu(
            &scanned,
            "Assign Guard",
            "Guard which structure? Any structure qualifies.",
            selected,
            painter,
            m,
        ),
        Mode::Remove => draw_remove_menu(&scanned, selected, painter, m),
        Mode::RemoveConfirm => draw_remove_confirm(selected, painter, m),
        Mode::Upgrade => draw_upgrade_menu(&scanned, selected, painter, m),
        Mode::Symlink => draw_symlink_menu(game, selected, painter, m),
        Mode::InspectDirection => draw_direction_prompt(
            "Inspect Direction",
            "Choose a direction to inspect (arrows/hjkl), Esc to cancel",
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
                from_picker: manifest_from_picker,
            };
            draw_manifest(game, pending_manifest, nav, painter, m)
        }
        Mode::ManifestPick => {
            let subjects = game.manifest_subjects();
            draw_manifest_pick(game, &subjects, selected, painter, m)
        }
        Mode::StructureManifest => structure_manifest::draw_structure_manifest(
            game,
            app.pending_structure_manifest,
            painter,
            m,
        ),
        Mode::CellDescribe => {
            stack::draw_cell_describe(app.pending_description.as_deref(), painter, m)
        }
        Mode::Inventory => draw_inventory(game, selected, painter, m),
        Mode::CompanionEquip => {
            draw_companion_equip(game, app.pending_equip_program, selected, painter, m)
        }
        Mode::EquipSwap => draw_equip_swap(
            game,
            app.pending_swap_slot,
            app.pending_swap_target,
            selected,
            painter,
            m,
        ),
        Mode::InventoryItemAction => {
            let zone = game.player_status().zone;
            let (item, fusion_tier) = split_pending_item(&pending_item);
            draw_inventory_item_action(game, item, zone, fusion_tier, selected, painter, m)
        }
        Mode::ItemDescribe => {
            let zone = game.player_status().zone;
            let (item, fusion_tier) = split_pending_item(&pending_item);
            draw_item_describe(game, item, zone, fusion_tier, painter, m)
        }
        Mode::Companion => draw_companion_menu(game, selected, painter, m),
        Mode::Fuse => draw_fuse_menu(game, selected, painter, m),
        Mode::FuseSecond => {
            draw_fuse_second_menu(game, app.pending_fuse_first, selected, painter, m)
        }
        Mode::FuseName => draw_fuse_name_menu(
            game,
            app.pending_fuse_first,
            app.pending_fuse_second,
            &app.fuse_name_input,
            painter,
            m,
        ),
        Mode::RenamePet => {
            draw_rename_menu(game, app.pending_rename, &app.rename_input, painter, m)
        }
        Mode::FieldCast => draw_field_cast(game, selected, painter, m),
        Mode::FieldCastAlly => {
            draw_field_cast_ally(game, pending_field_routine, selected, painter, m)
        }
        Mode::RoutineTarget => draw_routine_target(game, selected, painter, m),
        Mode::Routines => draw_routines(game, app.pending_routine_holder, selected, painter, m),
        Mode::RoutineInstall => draw_routine_install(game, selected, painter, m),
        Mode::Refactor => draw_refactor(game, selected, painter, m),
        Mode::RefactorItem => {
            draw_refactor_item(game, app.pending_refactor_target, selected, painter, m)
        }
        Mode::Extract => draw_extract(game, selected, painter, m),
        Mode::ExtractPick => {
            draw_extract_pick(game, app.pending_extract_program, selected, painter, m)
        }
        Mode::ExtractConfirm => draw_extract_confirm(
            game,
            app.pending_extract_program,
            app.pending_extract_index,
            painter,
            m,
        ),
        Mode::Trade => draw_trade_menu(game, selected, painter, m),
        Mode::TradeAction => {
            draw_trade_action_menu(game, app.pending_trade_structure, selected, painter, m)
        }
        Mode::TradeQuantity => draw_trade_quantity_menu(
            game,
            app.pending_trade_structure,
            app.pending_trade_choice.clone(),
            &app.trade_quantity_input,
            painter,
            m,
        ),
        Mode::TradeProgramConfirm => {
            let money = game.item_name(&game.trade_currency()).to_string();
            draw_trade_program_confirm(app.pending_trade_program.as_ref(), &money, painter, m)
        }
        Mode::Perks => draw_perks_menu(game, selected, painter, m),
        Mode::Research => draw_research_menu(game, selected, painter, m),
        Mode::History => draw_history(game, selected, painter, m),
        Mode::Structures => draw_structures(game, selected, painter, m),
        Mode::Recipes => draw_recipes(game, selected, painter, m),
        Mode::QuitRunConfirm => draw_quit_run_confirm(selected, painter, m),
        _ => {}
    }
}

fn draw_direction_prompt(title: &str, body: &str, painter: &Painter, m: &Metrics) {
    draw_popup(title, PopupSize::Small, &[text_row(body)], painter, m);
}

/// A program's current activity as a bracketed suffix — `" (in party)"`,
/// `" (Mining Node)"`, `" (guarding Data Cache)"`, `" (idle)"`. The wording
/// itself is `Game::program_activity`'s; every dialog that lists programs
/// appends it through here so they cannot drift apart.
fn activity_tag(activity: &str) -> String {
    format!(" ({activity})")
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// Two permanent properties, one channel. Fusion wins because it is the
    /// one that gates an action available now (can this still be a fusion
    /// input); a rare tier is only ever worth knowing.
    #[test]
    fn fusion_outranks_rarity_in_a_menu_row() {
        assert_eq!(
            program_color(0, Rarity::Gold),
            Some(GOLD),
            "an unfused program shows its tier"
        );
        assert_eq!(
            program_color(1, Rarity::Gold),
            Some(CYAN),
            "fusion depth is the actionable read and takes the channel"
        );
        assert_eq!(
            program_color(MAX_FUSIONS, Rarity::Silver),
            Some(MAGENTA),
            "and still does at the ceiling"
        );
        assert_eq!(
            program_color(0, Rarity::Ordinary),
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
        // Everything a tier bar can end up sitting against: the five
        // `difficulty_color` outputs, the neutrals, and the other tier.
        let neighbours = [
            ("GREEN", GREEN),
            ("YELLOW", YELLOW),
            ("ORANGE", ORANGE),
            ("RED", RED),
            ("MAGENTA", MAGENTA),
            ("CYAN", CYAN),
            ("BLUE", BLUE),
            ("TEXT", TEXT),
            ("TEXT_DIM", TEXT_DIM),
            ("GRAY", GRAY),
            ("WHITE", WHITE),
        ];
        for (name, tier) in [("SILVER", SILVER), ("GOLD", GOLD)] {
            for (other_name, other) in neighbours {
                assert!(
                    dist(tier, other) > 0.25,
                    "{name} is only {:.2} from {other_name} — it would read \
                     as that colour in a two-pixel bar",
                    dist(tier, other)
                );
            }
        }
        assert!(
            dist(SILVER, GOLD) > 0.25,
            "the two tiers must not read alike"
        );
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

    #[test]
    fn every_mode_that_covers_the_log_pane_gets_the_status_banner() {
        for mode in [
            Mode::Research,
            Mode::Build,
            Mode::Craft,
            Mode::Trade,
            Mode::Inventory,
            Mode::Battle,
            Mode::BattleTarget,
            Mode::BattleItem,
            Mode::Help,
            Mode::LoadGame,
            // Where "Save failed: ..." has to land, or save-and-quit refuses
            // to leave for a reason the player can't see.
            Mode::QuitRunConfirm,
        ] {
            assert!(
                needs_status_banner(mode),
                "{mode:?} draws over the log pane, so its refusals need the banner"
            );
        }
    }

    /// The kinds a player is meant to tell apart at a glance mid-fight: what
    /// they gained, what the enemy did, which of the enemy's blows carried a
    /// condition, their own hit, and how the fight came out. Sharing a colour
    /// between any two of them defeats the point of the log being coloured at
    /// all — and all five can sit in the battle pane at once.
    #[test]
    fn the_log_colours_a_player_reads_mid_fight_are_all_distinct() {
        let kinds = [
            MessageKind::Loot,
            MessageKind::EnemyAttack,
            MessageKind::EnemySpecial,
            MessageKind::PartyDamage,
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
        ] {
            assert!(
                !needs_status_banner(mode),
                "{mode:?} already surfaces status_line itself"
            );
        }
    }
}
