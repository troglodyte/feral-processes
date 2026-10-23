//! Type-to-filter on the compile list, the put-down picker and the tactical
//! item picker. Each filters to the *last* row so the filtered position and
//! the source row differ.

use super::super::support::*;
use super::super::tactical::{fighting, wait_for_the_player};
use crate::*;
use feral_processes_engine::items::ItemId;

/// A row name cut to the leading run the filter can be typed from.
fn typeable(name: &str) -> String {
    name.to_lowercase()
        .chars()
        .take_while(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || *c == ' ')
        .collect()
}

fn stocked_app(seed: u32) -> App {
    app_owning_a_program_and_an_extractor_with_cargo(seed, &[], &[("core_fragment", 200)])
}

/// The Compile list, filtered to its last recipe's name.
fn filtered_craft(seed: u32) -> (App, ItemId, usize) {
    let mut app = stocked_app(seed);
    open_command(&mut app, "Compile an item");
    assert_eq!(app.mode, Mode::Craft);
    let game = app.game.as_ref().unwrap();
    let recipes = game.craft_recipes();
    let last = recipes.last().unwrap().result.clone();
    let name = typeable(game.item_name(&last));
    type_text(&mut app, &name);
    let game = app.game.as_ref().unwrap();
    let visible = craft_visible(game, app.filter_text());
    assert!(visible.len() < recipes.len(), "a real subset: {name:?}");
    let pos = visible
        .iter()
        .position(|&i| i == recipes.len() - 1)
        .unwrap();
    (app, last, pos)
}

#[test]
fn enter_picks_the_filtered_recipe_and_the_quantity_page_returns_to_it() {
    let (mut app, last, pos) = filtered_craft(1401);
    let filter = app.filter_text().to_owned();
    app.menu_selected = pos;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::CraftQuantity);
    assert_eq!(app.pending_craft, Some(last));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Craft);
    assert_eq!(app.filter_text(), filter);
    assert_eq!(app.menu_selected, pos);
}

#[test]
fn esc_clears_the_compile_filter_before_closing() {
    let (mut app, _, _) = filtered_craft(1402);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Craft);
    assert!(!app.filter_applied());
    app.handle_key(GameKey::Esc);
    assert_ne!(app.mode, Mode::Craft);
}

#[test]
fn a_compile_filter_with_no_rows_picks_nothing() {
    let mut app = stocked_app(1403);
    open_command(&mut app, "Compile an item");
    type_text(&mut app, "qqq");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Craft);
    assert!(app.pending_craft.is_none());
}

fn put_down_app(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base_at(&mut app, 3, 3);
    give_player_items(&mut app, &[("core_fragment", 2), ("power_cell", 2)]);
    app.handle_key(GameKey::Char('y'));
    assert_eq!(app.mode, Mode::PutDown);
    app
}

#[test]
fn put_down_sets_down_the_filtered_row() {
    let mut app = put_down_app(1411);
    let rows = app.put_down_rows();
    assert!(rows.len() >= 2);
    let last = rows.last().unwrap().clone();
    type_text(&mut app, &typeable(&last.label));
    let shown = app.put_down_rows();
    assert!(shown.len() < rows.len(), "a real subset");
    assert!(shown.iter().any(|r| r.label == last.label));
    app.menu_selected = shown.iter().position(|r| r.label == last.label).unwrap();
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Playing, "{:?}", app.status_line);
    let floor = app.game.as_ref().unwrap().floor_items();
    assert_eq!(floor.len(), 1);
    assert!(
        last.label.starts_with(&floor[0].name),
        "put down {:?}, picked {:?}",
        floor[0].name,
        last.label
    );
}

#[test]
fn esc_clears_the_put_down_filter_before_closing() {
    let mut app = put_down_app(1412);
    type_text(&mut app, "q");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::PutDown);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn the_tactical_item_picker_spends_the_filtered_row() {
    let mut app = fighting(1421);
    wait_for_the_player(&mut app);
    // Not `give_player_items`: its save round trip would end the fight.
    let game = app.game.as_mut().unwrap();
    for id in ["capacitor_array", "buffered_cell"] {
        game.dev_give_item(&ItemId::from(id), 3).unwrap();
    }
    let game = app.game.as_ref().unwrap();
    let items = game.battle_usable_items();
    assert!(items.len() >= 2, "{items:?}");
    let last = items.last().unwrap().clone();
    let name = typeable(game.item_name(&last));
    app.handle_key(GameKey::Char('U'));
    assert_eq!(app.mode, Mode::TacticalItem);
    type_text(&mut app, &name);
    let game = app.game.as_ref().unwrap();
    let visible = tactical_items_visible(game, app.filter_text());
    assert!(visible.len() < items.len(), "a real subset");
    assert_eq!(items[visible[0]], last);
    let held = |app: &App, id: &ItemId| -> u32 {
        app.game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|r| &r.copy.item == id)
            .map_or(0, |r| r.qty)
    };
    let before: Vec<u32> = items.iter().map(|i| held(&app, i)).collect();
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::TacticalBattle);
    for (i, id) in items.iter().enumerate() {
        let want = if *id == last {
            before[i] - 1
        } else {
            before[i]
        };
        assert_eq!(held(&app, id), want, "{id:?}");
    }
}

#[test]
fn esc_clears_the_item_filter_before_leaving() {
    let mut app = fighting(1422);
    wait_for_the_player(&mut app);
    app.handle_key(GameKey::Char('U'));
    type_text(&mut app, "q");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::TacticalItem);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::TacticalBattle);
}
