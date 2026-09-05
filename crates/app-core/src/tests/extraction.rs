//! `Mode::DownedPrograms`: the pack's `D` key, and the two-phase screen it
//! opens — a list of held programs, then the tool-and-yield page for
//! whichever one is picked.

use feral_processes_engine::items::DownedProgram;
use feral_processes_engine::tools::ToolId;
use feral_processes_engine::tuning;

use super::support::*;
use crate::*;

fn program(species: &str, condition: u8, rarity: Rarity, level: u32) -> DownedProgram {
    DownedProgram {
        species: species.to_string(),
        level,
        rarity,
        boss: false,
        condition,
        carried: None,
    }
}

#[test]
fn pressing_uppercase_d_opens_the_downed_programs_screen_from_the_pack() {
    let mut app = test_app(9000);
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::Inventory);

    app.handle_key(GameKey::Char('D'));

    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(
        app.pending_downed_program_index.is_none(),
        "opening the screen lands on the list, not a program's tool page"
    );
}

/// `selected_index` reserves shifted letters for screen actions and lower
/// ones for rows — the trap this repo's own `S`/`U`/`I` bindings on this
/// same screen already guard against. A lowercase `d` binding here would
/// both open the screen and pick a row on the very keypress that opened it.
///
/// Asserting only that the mode isn't `DownedPrograms` would pass against
/// several wrong implementations on a short inventory, where `selected_
/// index` falls out on an out-of-range index regardless of case and does
/// nothing at all — `app_on_inventory_with_many_items` gives lowercase `d`
/// (`DIGIT_ROWS` + 3, the fourth letter row) a real row to land on, so
/// pressing it does what any other row-selecting letter on this screen
/// does: open `Mode::InventoryItemAction` for that item. That is the
/// evidence lowercase `d` is an ordinary row key here and nothing more.
#[test]
fn lowercase_d_selects_a_row_instead_of_opening_the_downed_programs_screen() {
    let mut app = app_on_inventory_with_many_items(9001);
    app.handle_key(GameKey::Char('i'));
    assert_eq!(app.mode, Mode::Inventory);
    let tenth_item = app.game.as_ref().unwrap().player_status().inventory[9]
        .copy
        .clone();

    app.handle_key(GameKey::Char('d'));

    assert_eq!(
        app.mode,
        Mode::InventoryItemAction,
        "lowercase d must pick the fourth letter row like any other row key, not open the \
         downed programs screen and not do nothing"
    );
    assert_eq!(
        app.pending_inventory_item,
        Some(tenth_item),
        "the row it picked must be the fourth-letter row (DIGIT_ROWS + 3), the tenth item"
    );
}

#[test]
fn esc_from_the_list_returns_to_inventory() {
    let mut app = test_app(9002);
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::DownedPrograms);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Inventory);
}

#[test]
fn an_out_of_range_row_is_ignored_on_an_empty_store() {
    let mut app = test_app(9003);
    assert!(
        app.game.as_mut().unwrap().downed_program_rows().is_empty(),
        "test premise: a fresh run holds nothing"
    );
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Char('D'));

    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::DownedPrograms, "the list page holds");
    assert!(
        app.pending_downed_program_index.is_none(),
        "there was no row 1 to pick"
    );
}

#[test]
fn picking_a_row_opens_the_tool_page_and_esc_backs_out_to_the_list() {
    let mut app =
        app_holding_downed_programs(9004, vec![program("scrapper", 70, Rarity::Gold, 20)]);
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::DownedPrograms);

    app.handle_key(GameKey::Char('1'));

    assert_eq!(
        app.pending_downed_program_index,
        Some(0),
        "the first row names index 0 in Game::downed_program_rows"
    );
    assert_eq!(app.mode, Mode::DownedPrograms, "still the same Mode");

    app.handle_key(GameKey::Esc);

    assert_eq!(
        app.mode,
        Mode::DownedPrograms,
        "Esc backs out one page, not out of the screen"
    );
    assert!(
        app.pending_downed_program_index.is_none(),
        "and forgets which program was picked, or the next visit reopens its tool page"
    );
}

#[test]
fn picking_a_tool_extracts_the_program_and_returns_to_the_list() {
    let mut app =
        app_holding_downed_programs(9005, vec![program("scrapper", 70, Rarity::Gold, 20)]);
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Char('D'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.pending_downed_program_index, Some(0));
    let game = app.game.as_ref().unwrap();
    assert_eq!(
        game.installed_tools()
            .iter()
            .map(|t| t.id.clone())
            .collect::<Vec<_>>(),
        vec![ToolId(tuning::STARTER_TOOL_ID.to_string())],
        "test premise: exactly the starter tool is installed"
    );

    app.handle_key(GameKey::Char('1'));

    assert_eq!(
        app.mode,
        Mode::DownedPrograms,
        "extracting returns to the list, not out of the screen"
    );
    assert!(
        app.pending_downed_program_index.is_none(),
        "the tool page for a now-extracted program is gone"
    );
    assert!(
        app.game.as_mut().unwrap().downed_program_rows().is_empty(),
        "the extracted program must be gone from the store"
    );
    assert_eq!(
        app.status_line, None,
        "a successful extraction is not a refusal"
    );
}
