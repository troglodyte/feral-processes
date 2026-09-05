//! `Mode::Tools`: the flat tool kit, reached from the party menu with
//! `App::party_menu_rows`'s "Tools" row.

use feral_processes_engine::items::ItemId;
use feral_processes_engine::save;
use feral_processes_engine::tools::ToolId;
use feral_processes_engine::tuning;

use super::support::*;
use crate::*;

/// An app with `tool` marked known, `cargo` sitting in the pack, and the
/// player at `level` — through a save/edit/load round trip, `stand_inside_
/// the_base`'s reason: `known_tools` and the pack are both engine `World`
/// state app-core has no other door onto.
fn app_with_known_tool(seed: u32, tool: &str, cargo: &[(&str, u32)], level: u32) -> App {
    let mut app = test_app(seed);
    let path = scratch_path("known_tool", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.known_tools = vec![ToolId(tool.to_string())];
    data.player.level = level;
    data.player
        .inventory
        .extend(cargo.iter().map(|(item, qty)| (ItemId::from(*item), *qty)));
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

/// The same fixture, plus one held carrier of `tool` — `app_with_known_
/// tool`'s reason, ported to a tool already forged.
fn app_holding_a_carrier(seed: u32, tool: &str, level: u32) -> App {
    let mut app = app_with_known_tool(seed, tool, &[], level);
    let path = scratch_path("held_carrier", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.player
        .inventory
        .push((ItemId::tool(&ToolId(tool.to_string())), 1));
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);
    app
}

#[test]
fn the_party_menu_opens_the_tools_screen_and_esc_backs_all_the_way_out() {
    let mut app = test_app(9400);
    open_via_menu(&mut app, 'p', "Tools");
    assert_eq!(app.mode, Mode::Tools);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::PartyMenu);
}

/// `App::party_menu_rows` is the only source of the row — there is no
/// direct key that opens `Mode::Tools`, `open_via_menu`'s own premise (it
/// looks the label up rather than pressing a hardcoded key).
#[test]
fn the_tools_row_is_offered_from_a_fresh_game() {
    let mut app = test_app(9401);
    let labels: Vec<_> = app.party_menu_rows().iter().map(|r| r.label).collect();
    assert!(
        labels.contains(&"Tools"),
        "the starter tool is installed from turn one, so the row must always be offered: \
         {labels:?}"
    );
}

/// An out-of-range digit does nothing — `selected_index`'s own floor,
/// re-asserted here because the row count is read fresh from the engine on
/// every keypress rather than cached (`handle_downed_programs_key`'s own
/// reason).
#[test]
fn an_out_of_range_row_is_ignored() {
    let mut app = test_app(9402);
    let len = app.game.as_ref().unwrap().tool_rows().len();
    assert_eq!(
        len, 1,
        "test premise: only the installed starter tool has a row"
    );
    open_via_menu(&mut app, 'p', "Tools");

    app.handle_key(GameKey::Char('2'));

    assert_eq!(app.mode, Mode::Tools, "the screen holds");
    assert_eq!(
        app.menu_selected, 0,
        "row 2 does not exist, so the highlight must not move"
    );
}

/// `F` forges a carrier of the highlighted, known-but-unforged tool — the
/// materials leave the pack and the row's own `carriers_held` goes to one,
/// both read back through `Game::tool_rows` rather than a second store.
#[test]
fn f_forges_a_carrier_of_the_highlighted_known_tool() {
    let mut app = app_with_known_tool(
        9403,
        "core_tap",
        &[("logic_wafer", 3), ("charge_coil", 1)],
        1,
    );
    open_via_menu(&mut app, 'p', "Tools");
    let rows = app.game.as_ref().unwrap().tool_rows();
    let core_tap_idx = rows
        .iter()
        .position(|r| r.id.as_str() == "core_tap")
        .expect("core_tap must have a row once known");
    app.menu_selected = core_tap_idx;

    app.handle_key(GameKey::Char('F'));

    assert_eq!(app.status_line, None, "a successful forge is not a refusal");
    let row = app
        .game
        .as_ref()
        .unwrap()
        .tool_rows()
        .into_iter()
        .find(|r| r.id.as_str() == "core_tap")
        .unwrap();
    assert_eq!(
        row.carriers_held, 1,
        "forging must grant exactly one carrier"
    );
    assert_eq!(
        app.game
            .as_ref()
            .unwrap()
            .player_status()
            .inventory
            .iter()
            .find(|r| r.copy.item.as_str() == "logic_wafer"),
        None,
        "the forge cost (3 logic_wafer) must be spent in full"
    );
}

/// `I` installs a held carrier into the player's next free slot.
#[test]
fn i_installs_a_held_carrier_into_a_free_slot() {
    // Level high enough for a second slot (`tuning::TOOL_SLOT_PER_LEVEL`),
    // since slot one is already the installed starter — see
    // `tools::player_tool_slots`.
    let mut app = app_holding_a_carrier(9404, "core_tap", tuning::TOOL_SLOT_PER_LEVEL);
    open_via_menu(&mut app, 'p', "Tools");
    let rows = app.game.as_ref().unwrap().tool_rows();
    let core_tap_idx = rows
        .iter()
        .position(|r| r.id.as_str() == "core_tap")
        .expect("core_tap must have a row while a carrier is held");
    assert_eq!(
        rows[core_tap_idx].slot, None,
        "test premise: not yet installed"
    );
    app.menu_selected = core_tap_idx;

    app.handle_key(GameKey::Char('I'));

    assert_eq!(
        app.status_line, None,
        "a successful install is not a refusal"
    );
    let row = app
        .game
        .as_ref()
        .unwrap()
        .tool_rows()
        .into_iter()
        .find(|r| r.id.as_str() == "core_tap")
        .unwrap();
    assert!(row.slot.is_some(), "the tool must now occupy a slot");
    assert_eq!(row.carriers_held, 0, "installing must burn the carrier");
}

/// `X` uninstalls the highlighted row's slot and hands nothing back —
/// `Game::uninstall_tool`'s own rule, re-asserted at the screen.
#[test]
fn x_uninstalls_the_highlighted_slot_and_grants_no_carrier_back() {
    let mut app = test_app(9405);
    open_via_menu(&mut app, 'p', "Tools");
    let starter = ToolId(tuning::STARTER_TOOL_ID.to_string());
    let starter_idx = app
        .game
        .as_ref()
        .unwrap()
        .tool_rows()
        .iter()
        .position(|r| r.id == starter)
        .expect("the starter tool must have a row");
    app.menu_selected = starter_idx;

    app.handle_key(GameKey::Char('X'));

    assert_eq!(
        app.status_line, None,
        "a successful uninstall is not a refusal"
    );
    assert!(
        app.game
            .as_ref()
            .unwrap()
            .tool_rows()
            .iter()
            .all(|r| r.id != starter),
        "the starter was never known, so pulling it drops its only row entirely"
    );
}

/// A refusal from any of the three verbs lands on the screen's own status
/// line exactly once, rather than layering a second copy — the trap the
/// spec's section 6 amendment warns `needs_status_banner` membership would
/// open (`Mode::Tools` must join `ALL_MODES` only).
#[test]
fn a_refusal_from_forging_lands_on_the_status_line_exactly_once() {
    let mut app = test_app(9406);
    open_via_menu(&mut app, 'p', "Tools");
    // The starter tool is installed but never known (task 1's own ruling),
    // so forging it refuses on "you haven't researched" — the highlighted
    // row from a fresh game is already this one.
    assert_eq!(app.menu_selected, 0);

    app.handle_key(GameKey::Char('F'));

    let status = app
        .status_line
        .clone()
        .expect("an unresearched tool must refuse forging");
    assert_eq!(
        status.matches("Salvage Clamp").count(),
        1,
        "the refusal must not be duplicated onto the line: {status:?}"
    );
}
