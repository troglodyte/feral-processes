//! `Mode::DownedPrograms`: the `D` key on the map and on Items, and the
//! two-phase screen it opens — a list of held programs, then the tool-and-yield page for
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
    open_hub_screen(&mut app, Mode::Inventory);
    assert_eq!(app.mode, Mode::Inventory);

    app.handle_key(GameKey::Char('D'));

    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(
        app.pending_downed_program_index.is_none(),
        "opening the screen lands on the list, not a program's tool page"
    );
}

/// Lowercase `d` opens nothing and picks nothing: the downed store is
/// `D`, because `d` is demolish on the map, and a list no longer reads
/// letters as rows. On a pack with a tenth item, where the old scheme would
/// have opened the item action for `d`, the screen simply stays put.
#[test]
fn lowercase_d_does_nothing_in_the_pack() {
    let mut app = app_on_inventory_with_many_items(9001);
    open_hub_screen(&mut app, Mode::Inventory);
    assert_eq!(app.mode, Mode::Inventory);
    let row = app.menu_selected;

    app.handle_key(GameKey::Char('d'));

    assert_eq!(
        app.mode,
        Mode::Inventory,
        "lowercase d must neither open the downed programs screen nor pick a row"
    );
    assert_eq!(app.menu_selected, row);
    assert_eq!(app.pending_inventory_item, None);
}

/// The store is a top-level screen, not a pocket of the pack: a body downs
/// in every fight and the tool page is where it is spent, so the map binds
/// it directly. `D` and not `d` for the pack binding's reason — the two
/// doors must be the same key, and `d` is demolish on the map.
#[test]
fn pressing_uppercase_d_on_the_map_opens_the_downed_programs_screen() {
    let mut app =
        app_holding_downed_programs(9010, vec![program("scrapper", 70, Rarity::Gold, 20)]);
    assert_eq!(app.mode, Mode::Playing, "test premise: standing on the map");

    app.handle_key(GameKey::Char('D'));

    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(
        app.pending_downed_program_index.is_none(),
        "opening the screen lands on the list, not a program's tool page"
    );
}

/// Bound in `handle_playing_key`'s top match, which runs before the
/// hand-off to `handle_stack_key` — the `1`/`2`/`3` rule. Extraction is
/// priced but never gated by locale (`extraction_yield` reads a bench tier
/// and nothing else), so a store reachable only on the surface would be the
/// keyboard contradicting the engine. A key that fell through to the Stack
/// handler instead would hit its `_ => {}` as a swallowed press with no
/// refusal and nothing in the log.
#[test]
fn the_store_opens_underground() {
    let mut app = app_underground(9011);
    assert!(
        app.game.as_ref().unwrap().is_underground(),
        "test premise: the party is down the Stack"
    );

    app.handle_key(GameKey::Char('D'));

    assert_eq!(app.mode, Mode::DownedPrograms);
}

#[test]
fn esc_from_the_map_route_steps_to_the_rail_and_then_out_to_the_map() {
    let mut app =
        app_holding_downed_programs(9012, vec![program("scrapper", 70, Rarity::Gold, 20)]);
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::DownedPrograms);

    app.handle_key(GameKey::Esc);
    assert!(app.hub_on_rail);
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
}

/// Both doors land on the same page, in it, so Esc means the same from
/// either: to the rail, then out to the map.
#[test]
fn esc_from_the_pack_route_steps_to_the_rail_and_then_out_to_the_map() {
    let mut app = test_app(9013);
    open_hub_screen(&mut app, Mode::Inventory);
    app.handle_key(GameKey::Char('D'));

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::DownedPrograms);
    assert!(app.hub_on_rail);

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn an_out_of_range_row_is_ignored_on_an_empty_store() {
    let mut app = test_app(9003);
    assert!(
        app.game.as_mut().unwrap().downed_program_rows().is_empty(),
        "test premise: a fresh run holds nothing"
    );
    open_hub_screen(&mut app, Mode::Inventory);
    app.handle_key(GameKey::Char('D'));

    pick_row(&mut app, 0);

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
    open_hub_screen(&mut app, Mode::Inventory);
    app.handle_key(GameKey::Char('D'));
    assert_eq!(app.mode, Mode::DownedPrograms);

    pick_row(&mut app, 0);

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
    open_hub_screen(&mut app, Mode::Inventory);
    app.handle_key(GameKey::Char('D'));
    pick_row(&mut app, 0);
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

    pick_row(&mut app, 0);

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
    open_hub_screen(&mut app, Mode::Inventory);
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

    pick_row(&mut app, 0);
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
    pick_row(&mut app, 0);

    assert_eq!(
        app.game.as_mut().unwrap().downed_program_rows().len(),
        2,
        "the picked row should spend exactly the one program it is on"
    );
    assert!(
        carried(&app) > before,
        "a hand extraction pays into the pack"
    );
}

// ---------------------------------------------------------------------------
// `Mode::RigTool`: the rig's own tool holder, a page of its hub.
// ---------------------------------------------------------------------------

/// `[c]` beside a rig opens its hub on the tool holder (the rig holds no
/// stock, so Inventory is not a page yet); `Esc` goes to the rail and `Esc`
/// again closes the hub.
#[test]
fn c_beside_a_rig_opens_the_tool_holder() {
    let mut app = app_beside_a_rig_holding(1);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    open_structure_page(&mut app);

    assert_eq!(app.mode, Mode::RigTool);
    assert!(
        app.rig_tool.is_some(),
        "the screen carries the rig it opened"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::RigTool, "Esc goes to the rail first");
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.rig_tool.is_none(), "and drops it on the way out");
}

/// The hub follows the party: out of the rig's reach it closes.
#[test]
fn the_tool_holder_closes_when_the_party_leaves_the_rig() {
    let mut app = app_beside_a_rig_holding(1);
    app.handle_key(GameKey::Esc);
    app.handle_key(GameKey::Esc);
    open_structure_page(&mut app);
    assert_eq!(app.mode, Mode::RigTool);

    stand_in_base_at(&mut app, 9, 9);
    app.handle_key(GameKey::Up);

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.rig_tool.is_none());
    assert!(app.structure_hub.is_none());
}
