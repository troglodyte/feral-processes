//! The inventory hub: `i` opens a rail of Items, Phase keys, Downed programs
//! and Tools, and every screen that used to hang off the pack still has its
//! door. Driven through `handle_key` throughout, so a lost door fails here.

use feral_processes_engine::items::ids;

use super::support::*;
use crate::*;
use feral_processes_engine::items::DownedProgram;

const RAIL: [Mode; 4] = [
    Mode::Inventory,
    Mode::PhaseKeys,
    Mode::DownedPrograms,
    Mode::Tools,
];

fn press(app: &mut App, keys: &[GameKey]) {
    for key in keys {
        app.handle_key(*key);
    }
}

/// `i`, then Right off the rail, onto the Items page's rows.
fn open_items_in_page(app: &mut App) {
    press(app, &[GameKey::Char('i'), GameKey::Right]);
    assert_eq!(app.mode, Mode::Inventory);
    assert!(!app.hub_on_rail);
}

/// Opens Items and puts the highlight on `item`'s pack row.
fn highlight_pack_row(app: &mut App, item: &str) {
    open_items_in_page(app);
    let row = app
        .game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .position(|r| r.copy.item.as_str() == item)
        .unwrap_or_else(|| panic!("{item} is in the pack"));
    // The three equipment slot rows come before the pack.
    app.menu_selected = 3 + row;
}

fn open_item_action(app: &mut App, item: &str) {
    highlight_pack_row(app, item);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::InventoryItemAction);
}

fn assert_on_items_in_page(app: &App) {
    assert_eq!(app.mode, Mode::Inventory);
    assert!(!app.hub_on_rail, "the cursor is still in the Items page");
}

#[test]
fn the_inventory_hub_lists_four_pages_in_rail_order() {
    assert_eq!(Hub::Inventory.title(), "INVENTORY");
    let titles: Vec<_> = Hub::Inventory.pages().iter().map(|p| p.title).collect();
    assert_eq!(titles, ["Items", "Phase keys", "Downed programs", "Tools"]);
    for (page, mode) in RAIL.into_iter().enumerate() {
        assert_eq!(hub_of(mode), Some((Hub::Inventory, page, 0)));
    }
}

#[test]
fn i_opens_on_items_with_the_cursor_on_the_rail() {
    let mut app = test_app(9500);
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::Inventory);
    assert!(app.hub_on_rail);
}

#[test]
fn rail_up_and_down_wrap_across_the_four_pages() {
    let mut app = test_app(9501);
    app.handle_key(GameKey::Char('i'));
    for expected in [RAIL[1], RAIL[2], RAIL[3], RAIL[0]] {
        app.handle_key(GameKey::Down);
        assert_eq!(app.mode, expected);
        assert!(app.hub_on_rail);
    }
    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::Tools, "Up from the first page wraps");
}

#[test]
fn brackets_turn_inventory_pages_from_the_rail_and_from_a_page() {
    let mut app = test_app(9502);
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::PhaseKeys);
    app.handle_key(GameKey::Char('['));
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::Tools);
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::Inventory);
}

#[test]
fn right_and_enter_move_into_the_page_and_esc_steps_out_by_halves() {
    for into in [GameKey::Right, GameKey::Enter] {
        let mut app = test_app(9503);
        app.handle_key(GameKey::Char('i'));
        app.handle_key(into);
        assert!(!app.hub_on_rail, "{into:?} enters the page");
        app.handle_key(GameKey::Esc);
        assert_eq!(app.mode, Mode::Inventory);
        assert!(app.hub_on_rail, "Esc in a page goes to the rail");
        app.handle_key(GameKey::Esc);
        assert_eq!(app.mode, Mode::Playing, "Esc on the rail leaves");
    }
}

#[test]
fn i_reopens_on_the_last_inventory_page_and_b_keeps_its_own_slot() {
    let mut app = test_app(9504);
    app.handle_key(GameKey::Char('i'));
    press(&mut app, &[GameKey::Down, GameKey::Down, GameKey::Esc]);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char('b'));
    assert_eq!(app.mode, Mode::BaseOverview, "b did not inherit i's page");
    app.handle_key(GameKey::Esc);

    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(app.hub_on_rail);
}

#[test]
fn capital_d_on_the_map_opens_downed_in_the_page_and_esc_esc_reaches_the_map() {
    let mut app = test_app(9505);
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(!app.hub_on_rail);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn capital_d_on_items_turns_to_downed_in_the_page_without_a_menu_origin() {
    let mut app = test_app(9506);
    open_items_in_page(&mut app);
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(!app.hub_on_rail);
    assert_eq!(app.menu_origin, None);
}

#[test]
fn esc_in_a_downed_drill_down_goes_back_to_the_list_not_the_rail() {
    let program = DownedProgram {
        species: "scrapper".to_string(),
        level: 20,
        rarity: Rarity::Gold,
        boss: false,
        condition: 70,
        carried: None,
    };
    let mut app = app_holding_downed_programs(9507, vec![program]);
    app.handle_key(GameKey::Char('D'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.pending_downed_program_index, Some(0));

    app.handle_key(GameKey::Esc);
    assert_eq!(app.pending_downed_program_index, None);
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(!app.hub_on_rail, "the list, not the rail");

    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail, "only Esc on the list reaches the rail");
}

fn one_downed_program() -> DownedProgram {
    DownedProgram {
        species: "scrapper".to_string(),
        level: 20,
        rarity: Rarity::Gold,
        boss: false,
        condition: 70,
        carried: None,
    }
}

#[test]
fn enter_opens_the_downed_drill_down_and_left_steps_back_to_the_list() {
    let mut app = app_holding_downed_programs(9510, vec![one_downed_program()]);
    app.handle_key(GameKey::Char('D'));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.pending_downed_program_index, Some(0));

    app.handle_key(GameKey::Left);
    assert_eq!(app.pending_downed_program_index, None);
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(!app.hub_on_rail, "the list, not the rail");
}

#[test]
fn a_page_turn_out_of_the_drill_down_drops_the_pick() {
    let mut app = app_holding_downed_programs(9511, vec![one_downed_program()]);
    press(&mut app, &[GameKey::Char('D'), GameKey::Enter]);
    assert_eq!(app.pending_downed_program_index, Some(0));

    press(&mut app, &[GameKey::Char(']'), GameKey::Char('[')]);
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert_eq!(
        app.pending_downed_program_index, None,
        "the list is showing"
    );
}

#[test]
fn leaving_the_hub_from_a_drill_down_does_not_keep_the_pick_for_the_return() {
    let mut app = app_holding_downed_programs(9512, vec![one_downed_program()]);
    press(
        &mut app,
        &[GameKey::Char('D'), GameKey::Enter, GameKey::Char(']')],
    );
    press(&mut app, &[GameKey::Esc, GameKey::Esc, GameKey::Char('i')]);
    app.handle_key(GameKey::Char('['));
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert_eq!(app.pending_downed_program_index, None);
}

#[test]
fn the_party_menu_no_longer_lists_tools_and_the_page_is_a_bracket_away() {
    let mut app = test_app(9508);
    app.handle_key(GameKey::Char('p'));
    assert!(
        party_rows(&mut app).iter().all(|r| r.label != "Tools"),
        "Tools moved to the inventory"
    );
    app.handle_key(GameKey::Esc);

    press(
        &mut app,
        &[GameKey::Char('i'), GameKey::Char(']'), GameKey::Char(']')],
    );
    app.handle_key(GameKey::Char(']'));
    assert_eq!(app.mode, Mode::Tools);
}

#[test]
fn the_tools_page_is_entered_on_the_rail_and_swallows_actions_there() {
    let mut app = test_app(9509);
    app.handle_key(GameKey::Char('i'));
    press(&mut app, &[GameKey::Up]);
    assert_eq!(app.mode, Mode::Tools);
    assert!(app.hub_on_rail);
    let status = app.status_line.clone();
    app.handle_key(GameKey::Char('F'));
    assert_eq!(app.status_line, status, "F is swallowed with no row chosen");
}

#[test]
fn tools_esc_goes_to_the_rail_then_the_map() {
    let mut app = test_app(9510);
    press(&mut app, &[GameKey::Char('i'), GameKey::Up, GameKey::Right]);
    assert_eq!(app.mode, Mode::Tools);
    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn phase_keys_is_a_read_only_page_whose_highlight_walks_ten_rows() {
    let mut app = test_app(9511);
    press(
        &mut app,
        &[GameKey::Char('i'), GameKey::Char(']'), GameKey::Right],
    );
    assert_eq!(app.mode, Mode::PhaseKeys);
    app.handle_key(GameKey::Up);
    assert_eq!(app.menu_selected, 9);
    let before = app.game.as_ref().unwrap().player_status().inventory.clone();
    for key in [
        GameKey::Char('S'),
        GameKey::Char('U'),
        GameKey::Char('I'),
        GameKey::Char('D'),
        GameKey::Char('4'),
        GameKey::Enter,
    ] {
        app.handle_key(key);
        assert_eq!(app.mode, Mode::PhaseKeys, "{key:?} left the keys page");
    }
    assert_eq!(app.game.as_ref().unwrap().player_status().inventory, before);
}

#[test]
fn esc_from_the_item_action_page_lands_on_items_in_the_page() {
    let mut app = app_at_a_mod_bench(9512, 10);
    open_item_action(&mut app, "nullsteel_plate");
    app.handle_key(GameKey::Esc);
    assert_on_items_in_page(&app);
}

#[test]
fn esc_from_the_mod_bench_lands_on_items_in_the_page() {
    let mut app = app_at_a_mod_bench(9513, 10);
    open_item_action(&mut app, "nullsteel_plate");
    app.handle_key(GameKey::Char('m'));
    assert_eq!(app.mode, Mode::ModCopy);
    app.handle_key(GameKey::Esc);
    assert_on_items_in_page(&app);
}

#[test]
fn esc_from_the_refit_bench_lands_on_items_in_the_page() {
    let mut app = app_at_a_refit_bench(9514, 4, Vec::new());
    open_item_action(&mut app, "nullsteel_plate");
    app.handle_key(GameKey::Char('r'));
    assert_eq!(app.mode, Mode::RefitCopy);
    app.handle_key(GameKey::Esc);
    assert_on_items_in_page(&app);
}

#[test]
fn esc_from_describe_opened_with_capital_i_lands_on_items_in_the_page() {
    let mut app = app_at_a_mod_bench(9515, 10);
    highlight_pack_row(&mut app, "nullsteel_plate");
    app.handle_key(GameKey::Char('I'));
    assert_eq!(app.mode, Mode::ItemDescribe);
    app.handle_key(GameKey::Esc);
    assert_on_items_in_page(&app);
}

#[test]
fn finishing_a_sale_lands_on_items_in_the_page() {
    let mut app = app_at_a_trading_post(9516, &[(ids::CORE_FRAGMENT, 5)]);
    open_item_action(&mut app, ids::CORE_FRAGMENT);
    app.handle_key(GameKey::Char('s'));
    assert_eq!(app.mode, Mode::TradeQuantity);
    app.handle_key(GameKey::Enter);
    assert_on_items_in_page(&app);
}

#[test]
fn esc_from_an_equip_swap_opened_on_items_lands_on_items_in_the_page() {
    let mut app = app_at_a_mod_bench(9517, 10);
    open_items_in_page(&mut app);
    // Row 1 is the armor slot, where the plate in the pack can be worn.
    app.menu_selected = 1;
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap);
    app.handle_key(GameKey::Esc);
    assert_on_items_in_page(&app);
}

#[test]
fn an_equip_swap_opened_from_a_companion_still_returns_to_the_companion() {
    let mut app = app_with_companions_and_cargo(9518, 1, &[("overclock_core", 1)]);
    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Char('E'));
    assert_eq!(app.mode, Mode::CompanionEquip);
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::EquipSwap);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::CompanionEquip);
}
