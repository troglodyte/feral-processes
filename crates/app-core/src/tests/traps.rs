//! Placing a honeypot from the pack, and what `d` means on the surface now
//! that there is something ownable out there to aim it at.

use feral_processes_engine::items::ids;
use feral_processes_engine::save;

use super::support::*;
use crate::*;

fn honeypot() -> ItemId {
    ItemId::from("honeypot")
}

/// An app holding `qty` honeypots, standing on the surface.
///
/// Stocked by editing a save and reloading it, because app-core cannot
/// reach the `World` — that is the architectural rule, and a fixture that
/// could would be testing its own idea of a pack.
fn app_holding_honeypots(seed: u32, qty: u32) -> App {
    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = scratch_path("honeypots", seed);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.player.inventory.push((honeypot(), qty));
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

fn traps_standing(app: &mut App) -> usize {
    app.game.as_mut().unwrap().trap_count()
}

fn held(app: &App, item: &ItemId) -> u32 {
    app.game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .filter(|row| row.copy.item == *item)
        .map(|row| row.qty)
        .sum()
}

/// Opens the item action page on a honeypot and presses `[P]lace`.
fn aim_a_honeypot(app: &mut App) {
    app.pending_inventory_item = Some(gear(&honeypot(), 0));
    app.mode = Mode::InventoryItemAction;
    app.handle_key(GameKey::Char('p'));
}

#[test]
fn a_honeypot_row_offers_place_and_ordinary_cargo_does_not() {
    let mut app = app_holding_honeypots(950, 1);
    let game = app.game.as_mut().unwrap();

    let trap_actions = inventory_item_actions(game, &honeypot());
    assert!(
        trap_actions.iter().any(|(k, _)| *k == 'p'),
        "a placeable item offers [P]lace, got {trap_actions:?}"
    );
    let plain = inventory_item_actions(game, &ItemId::from(ids::ICE_BREAKER));
    assert!(
        !plain.iter().any(|(k, _)| *k == 'p'),
        "ordinary cargo does not, got {plain:?}"
    );
}

#[test]
fn place_opens_the_direction_prompt_and_holds_the_item() {
    let mut app = app_holding_honeypots(951, 1);
    app.pending_inventory_item = Some(gear(&honeypot(), 0));
    app.mode = Mode::InventoryItemAction;

    app.handle_key(GameKey::Char('p'));

    assert_eq!(app.mode, Mode::TrapDirection);
    assert_eq!(app.pending_trap, Some(honeypot()));
}

#[test]
fn a_direction_places_one_and_returns_to_the_map() {
    let mut app = app_holding_honeypots(952, 2);
    aim_a_honeypot(&mut app);

    app.handle_key(GameKey::Right);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.pending_trap, None, "the commit clears the held item");
    assert_eq!(held(&app, &honeypot()), 1, "one unit is spent");
    assert_eq!(traps_standing(&mut app), 1);
}

#[test]
fn esc_from_the_direction_prompt_places_nothing() {
    let mut app = app_holding_honeypots(953, 1);
    aim_a_honeypot(&mut app);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.pending_trap, None, "and nothing is left held");
    assert_eq!(held(&app, &honeypot()), 1);
    assert_eq!(traps_standing(&mut app), 0);
}

#[test]
fn a_refused_placement_says_so_and_places_nothing() {
    let mut app = app_holding_honeypots(954, 2);
    aim_a_honeypot(&mut app);
    app.handle_key(GameKey::Right);
    assert_eq!(traps_standing(&mut app), 1);
    let held_before = held(&app, &honeypot());

    // One trap to a tile: the second placement at the same tile is refused.
    aim_a_honeypot(&mut app);
    app.handle_key(GameKey::Right);

    assert_eq!(app.mode, Mode::Playing);
    assert!(
        app.status_line.is_some(),
        "the refusal has to reach the player"
    );
    assert_eq!(held(&app, &honeypot()), held_before, "and spends nothing");
    assert_eq!(traps_standing(&mut app), 1);
}

/// `d` used to be refused outside base space. It opens the prompt in both
/// spaces now, so the assertion is on the mode: the old behaviour was a
/// refusal and the new one is a screen.
#[test]
fn d_on_the_surface_opens_the_direction_prompt() {
    let mut app = test_app(955);
    assert!(
        !app.game.as_ref().unwrap().in_base(),
        "the fixture stands on the open grid"
    );

    app.handle_key(GameKey::Char('d'));

    assert_eq!(app.mode, Mode::RemoveDirection);
}

#[test]
fn d_on_the_surface_destroys_a_honeypot_and_refuses_empty_ground() {
    let mut app = app_holding_honeypots(956, 1);
    aim_a_honeypot(&mut app);
    app.handle_key(GameKey::Right);
    assert_eq!(traps_standing(&mut app), 1);

    // Pointed the wrong way first: empty ground refuses and destroys
    // nothing.
    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Left);
    assert_eq!(traps_standing(&mut app), 1, "nothing that way");
    assert!(app.status_line.is_some());

    app.handle_key(GameKey::Char('d'));
    app.handle_key(GameKey::Right);
    assert_eq!(traps_standing(&mut app), 0, "and this way it goes");
}

/// The regression this task is most likely to ship: a base-space query asked
/// about a surface tile answers by numeric coincidence, and the coincidence
/// is the *common* case — `find_walkable_start` returns `(0, 0)` whenever it
/// can, so the anchor, the zone spawn point and base space's origin all
/// carry the same numbers. Nothing else in the suite would catch this.
#[test]
fn d_on_the_surface_cannot_demolish_a_structure_standing_in_base_space() {
    let mut app = test_app(957);
    found_the_base(&mut app);
    let structures = |app: &mut App| app.game.as_mut().unwrap().structure_report().len();
    let before = structures(&mut app);
    assert!(before > 0, "the base has to be standing or this is vacuous");
    assert!(
        !app.game.as_ref().unwrap().in_base(),
        "founding leaves the party on the open grid"
    );

    for dir in [GameKey::Up, GameKey::Down, GameKey::Left, GameKey::Right] {
        app.handle_key(GameKey::Char('d'));
        app.handle_key(dir);
    }

    assert_eq!(
        structures(&mut app),
        before,
        "a surface demolish must never reach base space"
    );
}
