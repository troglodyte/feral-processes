//! The Compile flow: picking a recipe, choosing a quantity, and the
//! careful-compile toggle that decides what the batch is worth.

use super::support::*;
use crate::*;

/// An app with a base founded and enough material to compile a batch.
fn stocked_app(seed: u32) -> App {
    app_owning_a_program_and_a_compiler_with_cargo(seed, &[], &[("core_fragment", 200)])
}

/// Opens Compile and picks `item`'s row by name — the recipe list is sorted
/// by category, so a hardcoded digit picks whatever happens to sort first.
fn open_compile_of(app: &mut App, item: &str) {
    open_via_menu(app, 'b', "Compile an item");
    let idx = app
        .game
        .as_ref()
        .unwrap()
        .craft_recipes()
        .iter()
        .position(|r| r.result == ItemId::from(item))
        .unwrap_or_else(|| panic!("{item} should be compilable in this fixture"));
    app.handle_key(GameKey::Char(menu_shortcut(idx)));
    assert_eq!(app.mode, Mode::CraftQuantity, "a recipe should be pending");
}

/// Opens Compile and picks the first recipe, leaving the app on the
/// quantity page with something pending.
fn open_quantity_page(app: &mut App) {
    open_via_menu(app, 'b', "Compile an item");
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::CraftQuantity, "a recipe should be pending");
}

/// The toggle is the page's, and it does not outlive the page.
///
/// A flag that survived would be the worst kind of leak here: the next
/// compile would silently charge half again for a floor the player did not
/// ask for, on a screen that had gone back to saying nothing about it.
#[test]
fn careful_compiling_toggles_on_the_quantity_page_and_never_leaks() {
    let mut app = test_app(700);
    open_quantity_page(&mut app);
    assert!(!app.careful_craft, "a page opens uncareful");

    app.handle_key(GameKey::Char('c'));
    assert!(app.careful_craft, "[C] turns it on");
    app.handle_key(GameKey::Char('c'));
    assert!(!app.careful_craft, "and off again");

    app.handle_key(GameKey::Char('c'));
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Craft);
    open_quantity_page(&mut app);
    assert!(
        !app.careful_craft,
        "the next compile starts from the same place the last one did"
    );
}

/// Every one of the three ways to commit a batch passes the toggle, which
/// is the whole of the flag being worth having.
///
/// The observable is the pair (material spent, copies made), because the
/// three paths differ in which half moves: at a fixed quantity a careful
/// batch costs more for the same number of copies, while `[M]` spends the
/// same pile either way and comes back with fewer.
#[test]
fn every_commit_path_charges_the_careful_price() {
    fn ledger(app: &App, item: &ItemId) -> (u32, u32) {
        let status = app.game.as_ref().unwrap().player_status();
        let of = |id: &ItemId| -> u32 {
            status
                .inventory
                .iter()
                .filter(|row| &row.copy.item == id)
                .map(|row| row.qty)
                .sum()
        };
        (of(&ItemId::from("core_fragment")), of(item))
    }

    let edge = ItemId::from("kinetic_edge");
    for key in [GameKey::Enter, GameKey::Char('f'), GameKey::Char('m')] {
        let mut plain = stocked_app(702);
        let mut careful = stocked_app(702);
        open_compile_of(&mut plain, "kinetic_edge");
        open_compile_of(&mut careful, "kinetic_edge");
        careful.handle_key(GameKey::Char('c'));

        let before = ledger(&plain, &edge);
        plain.handle_key(key);
        careful.handle_key(key);
        let (plain_now, careful_now) = (ledger(&plain, &edge), ledger(&careful, &edge));

        assert!(
            plain_now.1 > before.1 && careful_now.1 > before.1,
            "{key:?} compiled nothing on one side ({plain_now:?} against \
             {careful_now:?}) — a quoted maximum the batch cannot afford is \
             a refusal, not a careful compile"
        );
        assert_ne!(
            plain_now, careful_now,
            "{key:?} spent the same and made the same, so the toggle never \
             reached the engine"
        );
    }
}
