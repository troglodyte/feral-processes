//! The fight preview on the Points screen, the Perks menu and the purchase
//! page: what app-core owns is which routes refresh which figures, and that
//! a buy opens the page and leaves it with the highlight kept. The figures
//! themselves are the engine's (`tests::spend_preview` there).

use feral_processes_engine::save;

use super::support::*;
use crate::*;

/// A run holding `stat_points` and `perk_points`, through the save, the only
/// door app-core has onto them.
fn app_holding_points(seed: u32, stat_points: u32, perk_points: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("spend_preview_points", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player.stat_points = stat_points;
    data.player.perk_points = perk_points;
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

fn analysis_row(app: &App) -> usize {
    let want = feral_processes_engine::attributes::AttributeId::from("analysis");
    app.allocation_rows()
        .iter()
        .position(|row| matches!(row, CreationRow::Attribute { id, .. } if *id == want))
        .expect("an Analysis row")
}

fn perk_count(app: &App) -> usize {
    app.game.as_ref().unwrap().perk_defs().len()
}

fn assert_previews_filled(app: &App, route: &str) {
    assert_eq!(app.mode, Mode::Perks, "{route}");
    assert_eq!(app.perk_previews.len(), perk_count(app), "{route}");
    assert!(
        app.perk_previews.iter().any(Option::is_some),
        "{route}: no preview was computed"
    );
}

#[test]
fn the_points_duel_is_drawn_on_open_and_moves_on_a_spend_key() {
    let mut app = app_holding_points(9601, 3, 0);
    app.mode = Mode::Perks;
    app.handle_key(GameKey::Char('S'));
    assert_eq!(app.mode, Mode::AllocateStats, "{:?}", app.status_line);
    let open = app.allocation_duel.clone().expect("a duel on open");
    assert_eq!(open.per_swing.0, open.per_swing.1, "nothing spent yet");

    app.menu_selected = analysis_row(&app);
    app.handle_key(GameKey::ShiftRight);
    let spent = app.allocation_duel.clone().expect("a duel after a spend");
    assert_ne!(spent, open, "a spend key must refresh the figures");
    assert_eq!(spent.per_swing.0, open.per_swing.0, "before is unchanged");
    assert!(spent.per_swing.1 > spent.per_swing.0, "Analysis feeds ATK");
}

#[test]
fn a_point_moved_back_restores_the_open_figures() {
    let mut app = app_holding_points(9602, 3, 0);
    app.mode = Mode::Perks;
    app.handle_key(GameKey::Char('S'));
    let open = app.allocation_duel.clone();
    app.menu_selected = analysis_row(&app);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Left);
    assert_eq!(app.allocation_duel, open);
}

#[test]
fn every_route_into_the_perks_menu_fills_the_previews() {
    // Group menu.
    let mut app = app_holding_points(9603, 0, 1);
    open_via_menu(&mut app, 'p', "Perks");
    assert_previews_filled(&app, "group menu");

    // The level-up page's `P`.
    let mut app = app_holding_points(9604, 0, 1);
    app.mode = Mode::LevelUp;
    app.handle_key(GameKey::Char('P'));
    assert_previews_filled(&app, "level-up P");

    // The respec confirm, both answers.
    for key in [GameKey::Char('y'), GameKey::Char('n')] {
        let mut app = app_holding_points(9605, 0, 1);
        app.mode = Mode::RespecPerksConfirm;
        app.handle_key(key);
        assert_previews_filled(&app, "respec confirm");
    }

    // Leaving the Points screen, by Esc and by a commit.
    for key in [GameKey::Esc, GameKey::Enter] {
        let mut app = app_holding_points(9606, 2, 1);
        app.mode = Mode::Perks;
        app.handle_key(GameKey::Char('S'));
        app.handle_key(key);
        assert_previews_filled(&app, "leaving Points");
    }

    // The purchase page's return.
    let mut app = app_holding_points(9607, 0, 20);
    app.mode = Mode::Perks;
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::PerkBought, "{:?}", app.status_line);
    app.handle_key(GameKey::Enter);
    assert_previews_filled(&app, "purchase page");
}

#[test]
fn a_perk_key_opens_the_purchase_page_and_enter_returns_with_the_highlight() {
    let mut app = app_holding_points(9608, 0, 20);
    app.mode = Mode::Perks;
    app.menu_selected = 2;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::PerkBought, "{:?}", app.status_line);
    let report = app.pending_perk_report.clone().expect("a report");
    assert_eq!(report.level, 1);
    assert_eq!(app.menu_selected, 2, "the highlight rides through the page");

    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Perks);
    assert_eq!(app.menu_selected, 2);
    assert!(app.pending_perk_report.is_none());

    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::PerkBought);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Perks);
    assert_eq!(app.menu_selected, 2);
}

#[test]
fn a_refused_buy_stays_on_the_menu() {
    let mut app = app_holding_points(9609, 0, 0);
    app.mode = Mode::Perks;
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Perks);
    assert!(app.pending_perk_report.is_none());
    assert!(app.status_line.is_some(), "the refusal must say why");
}
