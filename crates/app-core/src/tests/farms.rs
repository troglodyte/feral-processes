//! Farm plots from the player's side: `[P]` in the Excavation plan marks
//! them, `[c]` beside one picks what it grows.

use feral_processes_engine::tuning::STARTING_POCKET_HALF_WIDTH;

use super::support::*;
use crate::*;

fn app_at_the_frontier(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base_at(&mut app, STARTING_POCKET_HALF_WIDTH, 0);
    app
}

fn tick(app: &App) -> u64 {
    app.game.as_ref().unwrap().current_tick()
}

/// `m`, `P`, then a one-cell box on the cell west of the party.
fn mark_a_plot(app: &mut App) {
    for k in ['m', 'P', 'h', ' ', ' '] {
        app.handle_key(GameKey::Char(k));
    }
    app.handle_key(GameKey::Esc);
}

fn tiles(app: &App) -> Vec<feral_processes_engine::views::FarmTileView> {
    app.game.as_ref().unwrap().farm_tiles()
}

#[test]
fn p_toggles_the_plot_tool_and_a_box_marks_a_plot() {
    let mut app = app_at_the_frontier(6001);
    let marks_before = app.game.as_ref().unwrap().marked_cells();
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('P'));
    assert_eq!(
        app.excavate_brush_label().as_deref(),
        Some("Tool: farm plot [P]")
    );
    for k in ['h', ' ', 'h', ' '] {
        app.handle_key(GameKey::Char(k));
    }
    assert!(!tiles(&app).is_empty());
    assert_eq!(app.game.as_ref().unwrap().marked_cells(), marks_before);
    app.handle_key(GameKey::Char('P'));
    assert_ne!(
        app.excavate_brush_label().as_deref(),
        Some("Tool: farm plot [P]")
    );
}

#[test]
fn marking_a_plot_never_ticks_the_game() {
    let mut app = app_at_the_frontier(6002);
    let before = tick(&app);
    mark_a_plot(&mut app);
    assert!(!tiles(&app).is_empty());
    assert_eq!(tick(&app), before);
}

fn app_beside_a_plot(seed: u32) -> App {
    let mut app = app_at_the_frontier(seed);
    mark_a_plot(&mut app);
    assert_eq!(app.mode, Mode::Playing);
    app
}

#[test]
fn c_beside_a_plot_opens_the_recipe_picker_and_a_row_sows_it() {
    let mut app = app_beside_a_plot(6003);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::FarmRecipe);
    let before = tick(&app);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_plot.is_none());
    assert_eq!(
        tick(&app),
        before,
        "choosing a recipe is a plan, not a turn"
    );
    assert!(tiles(&app).iter().all(|t| t.crop.is_some()));
}

#[test]
fn esc_leaves_the_recipe_picker_without_sowing() {
    let mut app = app_beside_a_plot(6004);
    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_plot.is_none());
    assert!(tiles(&app).iter().all(|t| t.crop.is_none()));
}

#[test]
fn the_picker_opens_on_the_current_recipe() {
    let mut app = app_beside_a_plot(6005);
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.farm_recipe_row(), 0);
    let rows = app.farm_recipe_rows();
    assert!(!rows.is_empty());
    let last = rows.len() - 1;
    // Rows 1-9 are the digits; the picker is far shorter than that.
    assert!(last < 9);
    app.handle_key(GameKey::Char(char::from(b'1' + last as u8)));
    app.handle_key(GameKey::Char('c'));
    assert_eq!(app.mode, Mode::FarmRecipe);
    assert_eq!(app.farm_recipe_row(), last);
}
