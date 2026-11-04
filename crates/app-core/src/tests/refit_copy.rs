//! The Refit Bench view, walked by key from the item menu and a slot.

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
fn the_view_opens_from_the_item_menu_mends_nothing_and_backs_out() {
    let mut app = app_at_a_refit_bench(9401, WEAR, Vec::new());
    open_item_menu(&mut app);
    app.handle_key(GameKey::Char('r'));
    assert_eq!(app.mode, Mode::RefitCopy);
    assert_eq!(app.refit_copy, Some(worn_plate()));

    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::RefitCopy);
    assert_eq!(app.refit_copy, Some(worn_plate()), "Enter is not a refit");
    assert_eq!(app.status_line, None);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Inventory);
    assert_eq!(app.refit_copy, None);
}

/// Wear lands only on what is worn, so the equipment row has to reach the
/// view itself; unequipping first would re-lock the copy at the current
/// zone's level.
#[test]
fn a_worn_weapons_status_opens_from_its_slot_screen() {
    let item = "kinetic_edge";
    let mut app = app_at_a_refit_bench_wearing(9410, item, 3, WEAR, Vec::new());
    let game = app.game.as_ref().unwrap();
    let worn = game
        .worn(game.player_entity(), EquipmentSlot::Weapon)
        .unwrap();
    assert_eq!(worn.copy.wear, WEAR, "the fixture wears the weapon");

    open_hub_screen(&mut app, Mode::Inventory);
    app.menu_selected = 0;
    pick_row(&mut app, 0);
    assert_eq!(app.mode, Mode::EquipSwap);
    app.handle_key(GameKey::Char('R'));
    assert_eq!(app.mode, Mode::RefitCopy, "{:?}", app.status_line);
    assert_eq!(app.refit_copy, Some(worn.copy));
}
