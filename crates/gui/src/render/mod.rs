//! All drawing for the graphics frontend: one screen per `Mode`, laid out
//! with macroquad's immediate-mode primitives (filled rects for bars and
//! tiles, drawn text for menus). Reads engine data through `App` and never
//! touches the ECS `World`.

use macroquad::prelude::*;

use crate::fx::Fx;
use crate::text::{Fonts, Metrics, map_cell, terrain_color, ui_metrics};
use feral_processes_app_core::{
    App, MENU_SCAN_RADIUS, Mode, TradeChoice, equip_preview_tag, inventory_item_actions,
    menu_shortcut,
};
use feral_processes_engine::components::GlyphColor;
use feral_processes_engine::items::ItemId;
use feral_processes_engine::world::Biome;
use feral_processes_engine::{
    Entity, EntityView, Game, MAX_FUSIONS, MessageKind, PetInfo, ProgramSaleOption, ResearchState,
};

mod bars;
mod base;
mod battle;
mod building;
mod crafting;
mod inspection;
mod inventory;
mod meta;
mod party;
mod popup;
mod progression;
mod trade;

use base::draw_playing_base;
use battle::{
    draw_battle, draw_battle_ally_menu, draw_battle_item_menu, draw_battle_special_menu,
    draw_battle_target_menu,
};
use building::{
    draw_build_menu, draw_remove_confirm, draw_remove_menu, draw_structure_menu, draw_symlink_menu,
    draw_upgrade_menu, draw_worker_menu,
};
use crafting::{draw_craft_menu, draw_craft_quantity};
use inspection::draw_inspect_detail;
use inventory::{draw_erase_quantity, draw_inventory, draw_inventory_item_action};
use meta::{
    draw_difficulty_pick, draw_game_over, draw_help, draw_load_game, draw_main_menu,
    draw_save_action,
};
use party::{draw_companion_menu, draw_fuse_menu, draw_fuse_name_menu, draw_fuse_second_menu};
use popup::{PopupSize, draw_popup, text_row};
use progression::{draw_perks_menu, draw_research_menu};
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

/// How far toward grey a back-rank group's bar is pulled — enough to read
/// as "can't reach you" beside an engaged group without becoming
/// unreadable.
const BACK_RANK_DESATURATION: f32 = 0.55;

/// Pulls `color` toward its own grey, for drawing something that's present
/// but not currently in play.
fn desaturate(color: Color) -> Color {
    let grey = (color.r + color.g + color.b) / 3.0;
    let mix = |c: f32| c + (grey - c) * BACK_RANK_DESATURATION;
    Color::new(mix(color.r), mix(color.g), mix(color.b), color.a)
}

/// Display styling for a message-log line, chosen by the engine-supplied
/// `MessageKind` rather than by sniffing the text — low-priority chatter
/// stays dim, gains/damage that matter get a color.
fn draw_message_line(kind: MessageKind, text: &str, x: f32, y: f32, fonts: &Fonts, m: &Metrics) {
    let color = match kind {
        MessageKind::Info => TEXT_DIM,
        MessageKind::Loot => GREEN,
        MessageKind::LevelUp => GREEN,
        MessageKind::Raid => ORANGE,
        MessageKind::Round => TEXT_DIM,
    };
    if kind == MessageKind::LevelUp {
        fonts.ui_bold(text, x, y, m.font_size, color);
    } else {
        fonts.ui(text, x, y, m.font_size, color);
    }
}

/// Whether `mode` needs `App::status_line` redrawn on top of whatever it
/// just drew. `Playing` already shows it in the log pane, and the main-menu
/// and save popups carry it as a row inside the panel; every other mode
/// covers the log pane with a popup, which would otherwise bury the one
/// message explaining why a menu pick was refused.
fn needs_status_banner(mode: Mode) -> bool {
    !matches!(mode, Mode::Playing | Mode::MainMenu | Mode::SaveAction)
}

/// Draws `status` in a strip along the bottom edge, below every popup —
/// `draw_popup` caps a panel at 85% of the window height and centers it, so
/// the bottom 7.5% is always clear.
fn draw_status_banner(status: &str, fonts: &Fonts, m: &Metrics) {
    let dims = fonts.measure_ui(status, m.font_size);
    let baseline = screen_height() - m.pad;
    draw_rectangle(
        0.0,
        baseline - dims.height - m.pad / 2.0,
        screen_width(),
        dims.height + m.pad,
        PANEL_BG,
    );
    fonts.ui(status, m.inset, baseline, m.font_size, RED);
}

pub fn draw(app: &mut App, fx: &mut Fx, fonts: &Fonts) {
    let m = ui_metrics(screen_height());
    clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
    match app.mode {
        Mode::MainMenu => draw_main_menu(app, fonts, &m),
        Mode::LoadGame => draw_load_game(app, fonts, &m),
        Mode::SaveAction => draw_save_action(app, fonts, &m),
        Mode::DifficultyPick => draw_difficulty_pick(app.menu_selected, fonts, &m),
        Mode::GameOver => draw_game_over(app, fonts, &m),
        Mode::Battle => draw_battle(app, fx, fonts, &m),
        Mode::BattleTarget => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_target_menu(app, fonts, &m);
        }
        Mode::BattleItem => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_item_menu(app, fonts, &m);
        }
        Mode::BattleSpecial => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_special_menu(app, fonts, &m);
        }
        Mode::BattleAlly => {
            draw_battle(app, fx, fonts, &m);
            draw_battle_ally_menu(app, fonts, &m);
        }
        Mode::Help => {
            draw_playing_base(app, fx, fonts, &m);
            draw_help(fonts, &m);
        }
        _ => {
            draw_playing_base(app, fx, fonts, &m);
            draw_mode_overlay(app, fonts, &m);
        }
    }
    if let Some(status) = &app.status_line
        && needs_status_banner(app.mode)
    {
        draw_status_banner(status, fonts, &m);
    }
}

/// Formats a `(item, quantity)` cost list, tagged `(have/need)` — same
/// convention as `ui.rs::cost_display`.
fn cost_display(game: &Game, cost: &[(ItemId, u32)], inventory: &[(ItemId, u32)]) -> Vec<String> {
    cost.iter()
        .map(|(item, qty)| {
            let have = inventory
                .iter()
                .find(|(i, _)| i == item)
                .map(|(_, q)| *q)
                .unwrap_or(0);
            format!("{} ({have}/{qty})", game.item_name(item))
        })
        .collect()
}

fn draw_mode_overlay(app: &mut App, fonts: &Fonts, m: &Metrics) {
    let selected = app.menu_selected;
    let Some(game) = &mut app.game else { return };
    match app.mode {
        Mode::Build => draw_build_menu(game, selected, fonts, m),
        Mode::BuildDirection => draw_direction_prompt(
            "Deploy Direction",
            "Choose a direction to deploy (arrows/hjkl), Esc to cancel",
            fonts,
            m,
        ),
        Mode::Craft => draw_craft_menu(game, selected, fonts, m),
        Mode::CraftQuantity => draw_craft_quantity(
            game,
            app.pending_craft.clone(),
            &app.craft_quantity_input,
            fonts,
            m,
        ),
        Mode::EraseQuantity => draw_erase_quantity(
            game,
            app.pending_erase.clone(),
            &app.erase_quantity_input,
            fonts,
            m,
        ),
        Mode::Cronjob => draw_worker_menu(
            game,
            "Assign Cronjob",
            "Assign which program to a cronjob?",
            selected,
            fonts,
            m,
        ),
        Mode::CronjobStructure => draw_structure_menu(
            game,
            "Assign Cronjob",
            "Cronjob which structure?",
            true,
            selected,
            fonts,
            m,
        ),
        Mode::Guard => draw_worker_menu(
            game,
            "Assign Guard",
            "Assign which program to guard duty?",
            selected,
            fonts,
            m,
        ),
        Mode::GuardStructure => draw_structure_menu(
            game,
            "Assign Guard",
            "Guard which structure? Any structure qualifies.",
            false,
            selected,
            fonts,
            m,
        ),
        Mode::Remove => draw_remove_menu(game, selected, fonts, m),
        Mode::RemoveConfirm => draw_remove_confirm(selected, fonts, m),
        Mode::Upgrade => draw_upgrade_menu(game, selected, fonts, m),
        Mode::Symlink => draw_symlink_menu(game, selected, fonts, m),
        Mode::InspectDirection => draw_direction_prompt(
            "Inspect Direction",
            "Choose a direction to inspect (arrows/hjkl), Esc to cancel",
            fonts,
            m,
        ),
        Mode::InspectDetail => draw_inspect_detail(game, app.pending_inspect, fonts, m),
        Mode::Inventory => draw_inventory(game, selected, fonts, m),
        Mode::InventoryItemAction => {
            let zone = game.player_status().zone;
            let fusion_tier = app
                .pending_inventory_item
                .as_ref()
                .map(|item| game.item_fusion_tier(item))
                .unwrap_or(0);
            draw_inventory_item_action(
                game,
                app.pending_inventory_item.clone(),
                zone,
                fusion_tier,
                selected,
                fonts,
                m,
            )
        }
        Mode::Companion => draw_companion_menu(game, selected, fonts, m),
        Mode::Fuse => draw_fuse_menu(game, selected, fonts, m),
        Mode::FuseSecond => draw_fuse_second_menu(game, app.pending_fuse_first, selected, fonts, m),
        Mode::FuseName => draw_fuse_name_menu(
            game,
            app.pending_fuse_first,
            app.pending_fuse_second,
            &app.fuse_name_input,
            fonts,
            m,
        ),
        Mode::Trade => draw_trade_menu(game, selected, fonts, m),
        Mode::TradeAction => {
            draw_trade_action_menu(game, app.pending_trade_structure, selected, fonts, m)
        }
        Mode::TradeQuantity => draw_trade_quantity_menu(
            game,
            app.pending_trade_structure,
            app.pending_trade_choice.clone(),
            &app.trade_quantity_input,
            fonts,
            m,
        ),
        Mode::TradeProgramConfirm => {
            draw_trade_program_confirm(app.pending_trade_program.as_ref(), fonts, m)
        }
        Mode::Perks => draw_perks_menu(game, selected, fonts, m),
        Mode::Research => draw_research_menu(game, selected, fonts, m),
        _ => {}
    }
}

fn draw_direction_prompt(title: &str, body: &str, fonts: &Fonts, m: &Metrics) {
    draw_popup(title, PopupSize::Small, &[text_row(body)], fonts, m);
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
        ] {
            assert!(
                needs_status_banner(mode),
                "{mode:?} draws over the log pane, so its refusals need the banner"
            );
        }
    }

    #[test]
    fn modes_that_already_show_the_status_line_dont_double_up() {
        for mode in [Mode::Playing, Mode::MainMenu, Mode::SaveAction] {
            assert!(
                !needs_status_banner(mode),
                "{mode:?} already surfaces status_line itself"
            );
        }
    }
}
