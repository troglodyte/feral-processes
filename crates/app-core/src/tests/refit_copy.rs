//! The Refit Bench screen, walked by key from the item menu.

use super::support::*;
use crate::*;

const WEAR: u8 = 4;

fn worn_plate() -> GearCopy {
    GearCopy {
        wear: WEAR,
        ..gear(&ItemId::from("nullsteel_plate"), 0)
    }
}

fn open_item_menu(app: &mut App) {
    app.pending_inventory_item = Some(worn_plate());
    app.mode = Mode::InventoryItemAction;
}

/// A refit bench with the plate worn and the pack stocked to pay for it.
fn stocked_app(seed: u32) -> App {
    let probe = app_at_a_refit_bench(seed, WEAR, Vec::new());
    let bill = probe.game.as_ref().unwrap().refit_cost(&worn_plate());
    app_at_a_refit_bench(seed, WEAR, bill)
}

#[test]
fn refit_is_offered_for_gear_only() {
    let mut app = app_at_a_refit_bench(9400, WEAR, Vec::new());
    let game = app.game.as_mut().unwrap();
    let keys = |game: &mut Game, item: &str| -> Vec<char> {
        inventory_item_actions(game, &ItemId::from(item))
            .into_iter()
            .map(|(k, _)| k)
            .collect()
    };
    assert!(keys(game, "nullsteel_plate").contains(&'r'));
    assert!(!keys(game, "core_fragment").contains(&'r'));
}

#[test]
fn open_pick_refit_then_back_out() {
    let mut app = stocked_app(9401);
    open_item_menu(&mut app);
    app.handle_key(GameKey::Char('r'));
    assert_eq!(app.mode, Mode::RefitCopy);
    assert_eq!(app.refit_copy, Some(worn_plate()));

    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RefitCopy, "{:?}", app.status_line);
    let mended = app.refit_copy.clone().unwrap();
    assert_eq!(mended.wear, 0, "the screen follows the re-keyed copy");
    assert_eq!(app.status_line, None);
    let game = app.game.as_ref().unwrap();
    assert_eq!(game.condition_of(&mended).0, game.condition_of(&mended).1);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.refit_copy, None);
}

#[test]
fn a_short_purse_is_refused_with_the_engines_sentence_and_spends_nothing() {
    let mut app = app_at_a_refit_bench(9402, WEAR, Vec::new());
    open_item_menu(&mut app);
    app.handle_key(GameKey::Char('r'));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RefitCopy);
    let line = app.status_line.clone().expect("a refusal sentence");
    assert!(line.starts_with("Need "), "{line}");
    assert_eq!(app.refit_copy, Some(worn_plate()), "nothing was mended");
}

#[test]
fn a_copy_already_at_full_condition_is_refused() {
    let mut app = stocked_app(9403);
    open_item_menu(&mut app);
    app.handle_key(GameKey::Char('r'));
    app.handle_key(GameKey::Enter);
    app.handle_key(GameKey::Enter);
    assert!(app.status_line.is_some());
    assert_eq!(app.refit_copy.as_ref().unwrap().wear, 0);
}

#[test]
fn the_bench_being_absent_is_told_not_hidden() {
    let mut app = test_app(9404);
    app.refit_copy = Some(worn_plate());
    app.mode = Mode::RefitCopy;
    app.handle_key(GameKey::Enter);
    assert_eq!(
        app.status_line.as_deref(),
        Some("Build a Refit Bench first.")
    );
    assert!(!app.game.as_ref().unwrap().refit_bench_standing());
}

/// Wear lands only on what is worn, so the equipment row has to reach the
/// bench itself; unequipping first would re-lock the copy at the current
/// zone's level.
#[test]
fn a_worn_weapon_is_refitted_from_its_slot_screen_and_keeps_its_level() {
    const LEVEL: u32 = 3;
    let item = "kinetic_edge";
    let probe = app_at_a_refit_bench_wearing(9410, item, LEVEL, WEAR, Vec::new());
    let probe_game = probe.game.as_ref().unwrap();
    let worn = probe_game
        .worn(probe_game.player_entity(), EquipmentSlot::Weapon)
        .unwrap();
    assert_eq!(worn.copy.wear, WEAR, "the fixture wears the weapon");
    let bill = probe_game.refit_cost(&worn.copy);
    let mut app = app_at_a_refit_bench_wearing(9410, item, LEVEL, WEAR, bill);

    open_hub_screen(&mut app, Mode::Inventory);
    app.menu_selected = 0;
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::EquipSwap);
    app.handle_key(GameKey::Char('R'));
    assert_eq!(app.mode, Mode::RefitCopy, "{:?}", app.status_line);
    assert_eq!(app.refit_copy, Some(worn.copy.clone()));

    app.handle_key(GameKey::Enter);
    assert_eq!(app.status_line, None);
    let game = app.game.as_ref().unwrap();
    let after = game
        .worn(game.player_entity(), EquipmentSlot::Weapon)
        .expect("still equipped");
    assert_eq!(after.copy.wear, 0);
    assert_eq!(after.level, LEVEL, "refit does not re-lock the gear level");
    assert_eq!(app.refit_copy, Some(after.copy));
}
