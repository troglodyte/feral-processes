//! The Mod Bench screens, walked by key from the item menu.

use super::support::*;
use crate::*;

fn plate() -> GearCopy {
    gear(&ItemId::from("nullsteel_plate"), 0)
}

fn open_item_menu(app: &mut App) {
    app.pending_inventory_item = Some(plate());
    app.mode = Mode::InventoryItemAction;
}

#[test]
fn modify_is_offered_for_weapons_and_armor_only() {
    let mut app = app_at_a_mod_bench(9300, 10);
    let game = app.game.as_mut().unwrap();
    let keys = |game: &mut Game, item: &str| -> Vec<char> {
        inventory_item_actions(game, &ItemId::from(item))
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    };
    assert!(keys(game, "nullsteel_plate").contains(&'m'));
    assert!(keys(game, "ragged_edge").contains(&'m'));
    assert!(!keys(game, "core_fragment").contains(&'m'));
}

#[test]
fn apply_then_strip_follows_the_rekeyed_copy() {
    let mut app = app_at_a_mod_bench(9301, 10);
    open_item_menu(&mut app);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(app.mode, Mode::ModCopy);
    assert_eq!(app.mod_copy, Some(plate()));

    // One empty slot: Enter opens the picker, the first row applies.
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::ModPickAffix);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::ModCopy, "{:?}", app.status_line);
    let fitted = app.mod_copy.clone().unwrap();
    assert_eq!(fitted.affixes, vec![AffixId::from("deflecting")]);
    assert!(app.status_line.as_deref().unwrap().contains("fit"));

    // `R` strips the highlighted filled row, and the screen follows the
    // copy back to its plain key.
    app.handle_key(GameKey::Char('R'));
    assert_eq!(app.mod_copy, Some(plate()));
    assert!(app.status_line.as_deref().unwrap().contains("strip"));

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.mod_copy, None);
}

#[test]
fn a_refused_apply_shows_the_refusal_line_and_stays_on_the_picker() {
    let mut app = app_at_a_mod_bench(9302, 0);
    open_item_menu(&mut app);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::ModPickAffix);
    assert!(app.status_line.is_some(), "a refusal sentence is shown");
    assert_eq!(app.mod_copy, Some(plate()), "nothing was fitted");
}

#[test]
fn enter_on_a_filled_slot_is_refused_not_opened() {
    let mut app = app_at_a_mod_bench(9303, 10);
    open_item_menu(&mut app);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::ModCopy);
    assert!(app.status_line.is_some());
}

#[test]
fn the_group_menu_row_opens_the_affix_research_view() {
    let mut app = app_at_a_mod_bench(9304, 0);
    open_via_menu(&mut app, 'b', "Affix research");
    assert_eq!(app.mode, Mode::AffixResearch);
    app.handle_key(GameKey::Esc);
    assert_ne!(app.mode, Mode::AffixResearch);
}

/// An over-cap copy (a drop can carry more affixes than its fusions buy
/// slots for) loses a row when stripped. The highlight must follow, or the
/// next `R` lands on a row that is gone and is refused as an empty slot.
#[test]
fn stripping_the_last_row_of_an_over_cap_copy_keeps_the_highlight_on_a_row() {
    let over_cap = GearCopy::with_affixes(
        ItemId::from("nullsteel_plate"),
        Rarity::Ordinary,
        0,
        ["deflecting", "hardened", "reinforced"]
            .iter()
            .map(|a| AffixId::from(*a))
            .collect(),
        feral_processes_engine::tuning::QUALITY_DEFAULT,
    );
    let carried = feral_processes_engine::save::GearCopySave {
        item: over_cap.item.clone(),
        rarity: over_cap.rarity,
        tier: over_cap.tier,
        affix: None,
        affixes: over_cap.affixes.clone(),
        quality: over_cap.quality,
    };
    let mut app = app_at_a_mod_bench_carrying(9304, 10, vec![(carried, 1)]);
    app.mod_copy = Some(over_cap);
    app.mode = Mode::ModCopy;

    app.handle_key(GameKey::Char('3'));
    app.handle_key(GameKey::Char('R'));
    assert_eq!(app.mod_copy.as_ref().unwrap().affixes.len(), 2);
    app.handle_key(GameKey::Char('R'));
    assert_eq!(
        app.mod_copy.as_ref().unwrap().affixes.len(),
        1,
        "the second R stripped the new last row: {:?}",
        app.status_line
    );
}
