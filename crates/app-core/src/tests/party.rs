//! Reordering the battle line from the party screen.

use super::support::*;
use crate::*;

fn roster(app: &mut App) -> Vec<feral_processes_engine::Entity> {
    app.game
        .as_mut()
        .unwrap()
        .owned_pets()
        .iter()
        .map(|p| p.entity)
        .collect()
}

#[test]
fn the_angle_keys_reorder_the_party_and_the_highlight_follows_the_member() {
    let mut app = app_with_companions_in_the_party(760, 2);
    let before = roster(&mut app);

    open_via_menu(&mut app, 'p', "Companions");
    assert_eq!(app.mode, Mode::Companion);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1, "the second slot is highlighted");

    app.handle_key(GameKey::Char('<'));

    assert_eq!(
        roster(&mut app),
        vec![before[1], before[0]],
        "'<' takes the slot ahead"
    );
    assert_eq!(app.status_line, None, "the move should not be refused");
    assert_eq!(
        app.mode,
        Mode::Companion,
        "reordering leaves the screen open to keep shuffling"
    );
    assert_eq!(
        app.menu_selected, 0,
        "the highlight rides the member it moved, so a second press keeps \
         pushing the same program rather than the one it displaced"
    );

    app.handle_key(GameKey::Char('>'));
    assert_eq!(roster(&mut app), before, "'>' puts it back");
    assert_eq!(app.menu_selected, 1);
}

#[test]
fn the_lead_member_cannot_be_pushed_off_the_front_of_the_line() {
    let mut app = app_with_companions_in_the_party(761, 2);
    let before = roster(&mut app);

    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Char('<'));

    assert!(
        app.status_line.is_some(),
        "the refusal is reported rather than swallowed"
    );
    assert_eq!(roster(&mut app), before, "and the order is untouched");
    assert_eq!(app.menu_selected, 0, "the highlight stays put on a refusal");
}

/// The wielded row, if any — `PetInfo::wielded` is what the `(WEP)` tag is
/// drawn from, so app-core can assert the state without a renderer.
fn wielded_row(app: &mut App) -> Option<feral_processes_engine::Entity> {
    app.game
        .as_mut()
        .unwrap()
        .owned_pets()
        .into_iter()
        .find(|p| p.wielded)
        .map(|p| p.entity)
}

#[test]
fn the_hidden_key_wields_the_highlighted_program() {
    let mut app = app_with_companions_in_the_party(770, 2);
    let before = roster(&mut app);
    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);

    app.handle_key(GameKey::Char('W'));

    assert_eq!(app.status_line, None, "the wield should not be refused");
    assert_eq!(
        wielded_row(&mut app),
        Some(before[1]),
        "the highlighted program goes in hand"
    );
    assert_eq!(
        app.mode,
        Mode::Companion,
        "the screen stays open the way reordering does"
    );
}

#[test]
fn the_hidden_key_unwields_when_pressed_on_the_program_already_in_hand() {
    let mut app = app_with_companions_in_the_party(771, 2);
    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Char('W'));
    let wielded = wielded_row(&mut app).expect("the first press wields");

    // Wielding stands the program down from the party, so `owned_pets`
    // reorders — find the row it moved to rather than assuming it stayed.
    let row = app
        .game
        .as_mut()
        .unwrap()
        .owned_pets()
        .iter()
        .position(|p| p.entity == wielded)
        .unwrap();
    app.menu_selected = row;
    app.handle_key(GameKey::Char('W'));

    assert_eq!(app.status_line, None);
    assert_eq!(wielded_row(&mut app), None, "the second press puts it down");
}

#[test]
fn the_hidden_key_is_ignored_on_an_empty_roster() {
    let mut app = test_app(772);
    // Set directly: with no programs the party menu hides the row entirely,
    // so the screen is unreachable through it — which is the point. The key
    // still must not panic if the screen is somehow open on nothing.
    app.mode = Mode::Companion;

    app.handle_key(GameKey::Char('W'));

    assert_eq!(app.status_line, None, "no roster, nothing to say about it");
    assert_eq!(app.mode, Mode::Companion);
}

/// The display name of the highlighted roster row.
fn name_of(app: &mut App, row: usize) -> String {
    app.game.as_mut().unwrap().owned_pets()[row].name.clone()
}

fn type_name(app: &mut App, text: &str) {
    for c in text.chars() {
        app.handle_key(GameKey::Char(c));
    }
}

#[test]
fn n_renames_the_highlighted_program() {
    let mut app = app_with_companions_in_the_party(770, 2);
    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Down);

    app.handle_key(GameKey::Char('N'));
    assert_eq!(app.mode, Mode::RenamePet, "'N' opens the naming page");

    type_name(&mut app, "Hexed");
    app.handle_key(GameKey::Enter);

    assert_eq!(app.mode, Mode::Companion, "Enter returns to the roster");
    // `PetInfo::name` is zone-tagged ("Hexed 1"), so match the prefix — the
    // engine's own tests pin the exact stored name; what this one is for is
    // that the key reaches it at all.
    assert!(
        name_of(&mut app, 1).starts_with("Hexed"),
        "got {:?}",
        name_of(&mut app, 1)
    );
}

#[test]
fn esc_leaves_a_rename_without_changing_the_name() {
    let mut app = app_with_companions_in_the_party(771, 2);
    open_via_menu(&mut app, 'p', "Companions");
    let before = name_of(&mut app, 0);

    app.handle_key(GameKey::Char('N'));
    type_name(&mut app, "Hexed");
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Companion, "Esc backs into the roster");
    assert_eq!(name_of(&mut app, 0), before, "the typed name is discarded");
    assert!(
        app.rename_input.is_empty(),
        "the buffer must not survive into the next rename"
    );
}

#[test]
fn an_empty_rename_puts_the_species_name_back() {
    let mut app = app_with_companions_in_the_party(772, 2);
    open_via_menu(&mut app, 'p', "Companions");
    let species_name = name_of(&mut app, 0);

    app.handle_key(GameKey::Char('N'));
    type_name(&mut app, "Hexed");
    app.handle_key(GameKey::Enter);
    assert!(name_of(&mut app, 0).starts_with("Hexed"));

    app.handle_key(GameKey::Char('N'));
    assert_eq!(
        app.rename_input, "Hexed",
        "the page opens on the name it already has, so a correction is not a retype"
    );
    for _ in 0.."Hexed".len() {
        app.handle_key(GameKey::Backspace);
    }
    app.handle_key(GameKey::Enter);

    assert_eq!(
        name_of(&mut app, 0),
        species_name,
        "clearing the field is the way back to the species name"
    );
}

// --- Party membership ----------------------------------------------------

/// Whether `entity` is standing in the active party right now.
fn in_party(app: &mut App, entity: feral_processes_engine::Entity) -> bool {
    app.game
        .as_mut()
        .unwrap()
        .owned_pets()
        .into_iter()
        .find(|p| p.entity == entity)
        .expect("the program is still owned")
        .party_slot
        .is_some()
}

#[test]
fn p_stands_the_highlighted_member_down_and_puts_it_back() {
    let mut app = app_with_companions_in_the_party(780, 2);
    let before = roster(&mut app);
    open_via_menu(&mut app, 'p', "Companions");
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1);

    app.handle_key(GameKey::Char('P'));

    assert_eq!(
        app.status_line, None,
        "standing a member down is never refused"
    );
    assert!(
        !in_party(&mut app, before[1]),
        "the highlighted member steps out"
    );
    assert!(
        in_party(&mut app, before[0]),
        "and the other one is left alone"
    );
    assert_eq!(
        app.mode,
        Mode::Companion,
        "the screen stays open the way reordering does"
    );

    // Standing down reorders `owned_pets` — the party leads it — so find the
    // row it moved to rather than assuming the highlight still names it.
    let row = app
        .game
        .as_mut()
        .unwrap()
        .owned_pets()
        .iter()
        .position(|p| p.entity == before[1])
        .unwrap();
    app.menu_selected = row;
    app.handle_key(GameKey::Char('P'));

    assert_eq!(app.status_line, None, "and there is room to come back");
    assert!(
        in_party(&mut app, before[1]),
        "the second press puts it back"
    );
}

/// The whole of TODO 14: every action on this screen acts on the highlight,
/// so a row shortcut only moves it. Before this change a stray digit stood a
/// party member down on the spot.
#[test]
fn a_row_shortcut_only_moves_the_highlight() {
    let mut app = app_with_companions_in_the_party(781, 2);
    let before = roster(&mut app);
    open_via_menu(&mut app, 'p', "Companions");

    app.handle_key(GameKey::Char('2'));

    assert_eq!(app.menu_selected, 1, "the digit picks the row");
    assert!(
        in_party(&mut app, before[0]) && in_party(&mut app, before[1]),
        "and changes nothing about who is standing in the party"
    );
}

#[test]
fn enter_does_nothing_on_the_roster() {
    let mut app = app_with_companions_in_the_party(782, 2);
    let before = roster(&mut app);
    open_via_menu(&mut app, 'p', "Companions");

    app.handle_key(GameKey::Enter);

    assert_eq!(app.menu_selected, 0, "the highlight stays put");
    assert!(
        in_party(&mut app, before[0]) && in_party(&mut app, before[1]),
        "and the party is untouched"
    );
    assert_eq!(app.mode, Mode::Companion, "Enter is not a way out either");
    assert_eq!(app.status_line, None);
}

#[test]
fn p_is_ignored_on_an_empty_roster() {
    let mut app = test_app(783);
    // Set directly, for the reason `the_hidden_key_is_ignored_on_an_empty_
    // roster` gives: the party menu hides the row when there is nothing on it.
    app.mode = Mode::Companion;

    app.handle_key(GameKey::Char('P'));

    assert_eq!(app.status_line, None, "no roster, nothing to say about it");
    assert_eq!(app.mode, Mode::Companion);
}

// --- Companion equipment -------------------------------------------------

/// Opens the roster and puts the highlight on the first program.
fn open_roster(app: &mut App) {
    open_via_menu(app, 'p', "Companions");
    assert_eq!(app.mode, Mode::Companion);
}

fn companion_atk(app: &mut App) -> i32 {
    app.game.as_mut().unwrap().owned_pets()[0].atk
}

#[test]
fn e_on_the_roster_opens_the_highlighted_programs_slots_and_esc_backs_out() {
    let mut app = app_with_companions_and_cargo(770, 1, &[("overclock_core", 1)]);
    let program = roster(&mut app)[0];
    open_roster(&mut app);

    app.handle_key(GameKey::Char('E'));

    assert_eq!(app.mode, Mode::CompanionEquip);
    assert_eq!(
        app.pending_equip_program,
        Some(program),
        "the page is about the program under the highlight"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Companion, "Esc backs into the roster");
}

#[test]
fn picking_a_slot_opens_the_picker_for_the_program_and_esc_returns_to_its_slots() {
    let mut app = app_with_companions_and_cargo(771, 1, &[("overclock_core", 1)]);
    let program = roster(&mut app)[0];
    open_roster(&mut app);
    app.handle_key(GameKey::Char('E'));

    app.handle_key(GameKey::Char('1'));

    assert_eq!(app.mode, Mode::EquipSwap);
    assert_eq!(app.pending_swap_slot, Some(EquipmentSlot::Weapon));
    assert_eq!(
        app.pending_swap_target,
        Some(program),
        "the picker has to know whose slot it is filling"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::CompanionEquip,
        "Esc returns where the picker was opened from, not to the inventory"
    );
    assert_eq!(
        app.pending_swap_target, None,
        "every exit from the picker clears the target"
    );
}

#[test]
fn choosing_a_row_equips_the_program_and_not_the_player() {
    let mut app = app_with_companions_and_cargo(772, 1, &[("overclock_core", 1)]);
    let player_atk_before = app.game.as_ref().unwrap().player_status().atk;
    let companion_atk_before = companion_atk(&mut app);
    open_roster(&mut app);
    app.handle_key(GameKey::Char('E'));
    app.handle_key(GameKey::Char('1'));

    app.handle_key(GameKey::Char('1'));

    assert_eq!(
        companion_atk(&mut app),
        companion_atk_before + 3,
        "the weapon goes on the program"
    );
    assert_eq!(
        app.game.as_ref().unwrap().player_status().atk,
        player_atk_before,
        "and not on the player"
    );
    assert_eq!(
        app.mode,
        Mode::CompanionEquip,
        "back to the program's slots"
    );
    assert_eq!(app.pending_swap_target, None);
}

#[test]
fn the_pickers_rows_are_measured_against_the_programs_own_worn_copy() {
    // Two copies of one weapon: the program wears one, the player the other,
    // so the only thing that can make the two row sets differ is which
    // wearer the picker measured against.
    let mut app = app_with_companions_and_cargo(773, 1, &[("overclock_core", 3)]);
    let program = roster(&mut app)[0];
    let (game, player) = {
        let game = app.game.as_mut().unwrap();
        let player = game.player_entity();
        (game, player)
    };
    game.equip(program, &gear(&ItemId::from("overclock_core"), 0))
        .unwrap();

    let program_rows: Vec<String> =
        equip_swap_rows(app.game.as_ref().unwrap(), program, EquipmentSlot::Weapon)
            .into_iter()
            .map(|r| r.label)
            .collect();
    let player_rows: Vec<String> =
        equip_swap_rows(app.game.as_ref().unwrap(), player, EquipmentSlot::Weapon)
            .into_iter()
            .map(|r| r.label)
            .collect();

    assert_ne!(
        program_rows, player_rows,
        "a geared program and a bare player must not see the same rows"
    );
    assert!(
        program_rows.iter().any(|l| l.contains("Unequip")),
        "the program's picker offers to empty the slot it has filled: {program_rows:?}"
    );
    assert!(
        !player_rows.iter().any(|l| l.contains("Unequip")),
        "the player's does not, because the player is wearing nothing: {player_rows:?}"
    );
}

#[test]
fn a_slot_with_nothing_to_fit_it_reports_that_instead_of_an_empty_picker() {
    let mut app = app_with_companions_and_cargo(774, 1, &[("overclock_core", 1)]);
    open_roster(&mut app);
    app.handle_key(GameKey::Char('E'));

    // Row 2 is Armor, and the only gear in cargo is a weapon.
    app.handle_key(GameKey::Char('2'));

    assert_eq!(
        app.mode,
        Mode::CompanionEquip,
        "a dead-end picker should not open at all"
    );
    assert!(
        app.status_line
            .as_deref()
            .is_some_and(|s| s.contains("Armor")),
        "the refusal names the slot: {:?}",
        app.status_line
    );
}

/// `M` on the roster opens the sheet for the row under the highlight, and Esc
/// comes back to the roster with that row still highlighted — the roster is
/// where a player compares two programs, so reading one's numbers must not
/// cost the place they were reading from.
#[test]
fn m_reads_the_highlighted_programs_manifest_and_esc_returns_to_the_roster() {
    let mut app = app_with_companions_in_the_party(775, 2);
    let roster = roster(&mut app);
    open_roster(&mut app);
    app.handle_key(GameKey::Down);
    assert_eq!(app.menu_selected, 1, "the second program is highlighted");

    app.handle_key(GameKey::Char('M'));

    assert_eq!(app.mode, Mode::Manifest);
    assert_eq!(
        app.pending_manifest,
        Some(roster[1]),
        "the sheet is the highlighted program's, not the first row's"
    );

    app.handle_key(GameKey::Esc);

    assert_eq!(
        app.mode,
        Mode::Companion,
        "Esc backs into the roster rather than out to the map"
    );
    assert_eq!(
        app.menu_selected, 1,
        "and lands back on the row the sheet was opened from"
    );
}

/// Paging the sheet with ←/→ and then leaving lands the roster's highlight on
/// whoever was on screen, the same rule `Mode::ManifestPick` follows: the list
/// should agree with the sheet just left.
#[test]
fn paging_the_sheet_moves_the_rosters_highlight_to_match() {
    let mut app = app_with_companions_in_the_party(776, 2);
    let roster = roster(&mut app);
    open_roster(&mut app);
    app.handle_key(GameKey::Char('M'));
    assert_eq!(app.pending_manifest, Some(roster[0]));

    // The subjects are the player first, then the roster — so two steps
    // forward from the first program is the second one.
    let subjects = app.manifest_subjects();
    let from = subjects.iter().position(|&s| s == roster[0]).unwrap();
    let steps = (subjects.iter().position(|&s| s == roster[1]).unwrap() + subjects.len() - from)
        % subjects.len();
    for _ in 0..steps {
        app.handle_key(GameKey::Right);
    }
    assert_eq!(app.pending_manifest, Some(roster[1]));

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Companion);
    assert_eq!(
        app.menu_selected, 1,
        "the highlight follows the sheet, not the row M was pressed on"
    );
}

/// The roster has no row for the player, so a sheet paged onto them leaves the
/// highlight standing where it was rather than snapping to the top. The parked
/// row is what `keeps_highlight` carries across the side trip; without it this
/// returns to row 0 and the player loses their place by reading their own
/// stats.
#[test]
fn paging_onto_the_player_leaves_the_rosters_highlight_alone() {
    let mut app = app_with_companions_in_the_party(777, 2);
    open_roster(&mut app);
    app.handle_key(GameKey::Down);
    app.handle_key(GameKey::Char('M'));

    let player = app.game.as_mut().unwrap().manifest_subjects()[0];
    while app.pending_manifest != Some(player) {
        app.handle_key(GameKey::Right);
    }

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Companion);
    assert_eq!(
        app.menu_selected, 1,
        "the row M was pressed on is still highlighted"
    );
}
