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

// ---------------------------------------------------------------------------
// Phase 4: the two verbs that hand a program to an adjacent Teardown Rig —
// `L` from the list (every held program) and `Q` on a tool page (that one).
// ---------------------------------------------------------------------------

/// The three tests below share this: the party in base beside a rig, with
/// `n` held programs and the screen already open on the list page.
fn app_beside_a_rig_holding(n: u32) -> App {
    let held = (1..=n)
        .map(|level| program("scrapper", 70, Rarity::Ordinary, level))
        .collect();
    let mut app = app_beside_a_teardown_rig_holding(9100, held);
    app.handle_key(GameKey::Char('i'));
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::DownedPrograms);
    app
}
/// The rig is fed by the base now, so this screen has exactly one verb: a
/// row spends the highlighted program out of the player's own pack. `Q` and
/// `L` are unbound on both pages, and `App::selected_index` answering `None`
/// for anything not lowercase-or-a-digit is what keeps an uppercase key from
/// also picking a row.
#[test]
fn the_screen_is_the_players_own_hands_and_a_row_pays_the_pack() {
    let mut app = app_beside_a_rig_holding(3);

    app.handle_key(GameKey::Char('Q'));
    assert!(
        app.pending_downed_program_index.is_none(),
        "`Q` is unbound on the list page, and must not pick a row there"
    );

    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.pending_downed_program_index, Some(0));
    app.handle_key(GameKey::Char('L'));
    assert_eq!(
        app.pending_downed_program_index,
        Some(0),
        "`L` is unbound on the tool page, and must not pick a tool row there"
    );

    // A hand extraction pays the player through `grant_loot`, which is what
    // says the row spent the program into the pack rather than a machine.
    let carried = |app: &App| -> u32 {
        app.game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .map(|row| row.qty)
            .sum()
    };
    let before = carried(&app);
    app.handle_key(GameKey::Char('1'));

    assert_eq!(
        app.game.as_mut().unwrap().downed_program_rows().len(),
        2,
        "the digit row should spend exactly the one program it is on"
    );
    assert!(
        carried(&app) > before,
        "a hand extraction pays into the pack"
    );
}

// ---------------------------------------------------------------------------
// `Mode::RigTool`: the rig's own tool holder, `[F]` in base space.
// ---------------------------------------------------------------------------

/// `[F]` beside a rig opens the holder; `Esc` closes it. Not `c`, which
/// already opens the transfer picker at a rig to collect what it stripped,
/// and not `T`, which `crates/engine/EASTER_EGGS.md` reserves.
#[test]
fn f_beside_a_rig_opens_the_tool_holder() {
    let mut app = app_beside_a_rig_holding(1);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char('F'));

    assert_eq!(app.mode, Mode::RigTool);
    assert!(
        app.rig_tool.is_some(),
        "the screen carries the rig it opened"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.rig_tool.is_none(), "and drops it on the way out");
}

/// A key that reads as doing nothing is the failure being avoided, so the
/// refusal is spoken rather than swallowed.
#[test]
fn f_with_no_rig_beside_you_refuses_out_loud() {
    let mut app = app_beside_a_rig_holding(1);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    // Walk out of the rig's four orthogonal tiles without leaving base
    // space. Asserted rather than assumed: walking *into* the rig's cell
    // does not move the player, so a step in its direction would leave the
    // party adjacent and this test would pass for nothing.
    app.handle_key(GameKey::Char('j'));
    app.handle_key(GameKey::Char('j'));
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .adjacent_teardown_rigs()
            .is_empty(),
        "the fixture must actually get the party clear of the rig"
    );
    app.handle_key(GameKey::Char('F'));

    assert_eq!(app.mode, Mode::Playing, "no screen opens");
    let said = app
        .status_line
        .clone()
        .expect("the refusal is said out loud");
    assert!(
        said.contains("rig"),
        "and it is this key's own refusal, not another's: {said:?}"
    );
}
