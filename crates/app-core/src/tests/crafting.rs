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

/// The arrows are the quantity, the gesture the transfer picker and the
/// caravan already use for a number: Right increases, Left decreases.
///
/// The page opens on an empty buffer that *reads* as one, so the first
/// Right has to land on two — an arrow that stepped the parsed zero instead
/// would print the number the screen was already showing and read as a
/// dropped keypress.
#[test]
fn arrows_step_the_compile_quantity() {
    let mut app = test_app(700);
    open_quantity_page(&mut app);
    assert_eq!(app.craft_quantity(), 1, "a page opens on one");

    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Right);
    assert_eq!(app.craft_quantity(), 3, "Right increases");

    app.handle_key(GameKey::Left);
    assert_eq!(app.craft_quantity(), 2, "Left decreases");

    for _ in 0..5 {
        app.handle_key(GameKey::Left);
    }
    assert_eq!(
        app.craft_quantity(),
        0,
        "and stops at zero rather than wrapping"
    );
}

/// Shift is a *target* and Ctrl a *step*, `app/basket.rs`' split, and the
/// end they head for is the max this batch can afford.
///
/// Both have to survive `App::handle_key`'s modifier fold, which turns them
/// into bare arrows on every screen not named in it — miss the name and
/// this test sees a one-unit step where it asked for a jump.
#[test]
fn shift_and_ctrl_reach_the_max_affordable_compile() {
    let mut app = stocked_app(701);
    open_compile_of(&mut app, "ice_breaker");
    let max = app
        .game
        .as_ref()
        .unwrap()
        .max_craftable(&ItemId::from("ice_breaker"), false);
    assert!(max >= 4, "the fixture should afford a batch worth halving");

    app.handle_key(GameKey::ShiftRight);
    assert_eq!(app.craft_quantity(), max, "Shift lands on the end");

    app.handle_key(GameKey::ShiftLeft);
    assert_eq!(app.craft_quantity(), 0, "and on the other one");

    app.handle_key(GameKey::CtrlRight);
    assert_eq!(
        app.craft_quantity(),
        max.div_ceil(2),
        "Ctrl closes half the gap to it"
    );
    app.handle_key(GameKey::CtrlLeft);
    assert_eq!(
        app.craft_quantity(),
        max.div_ceil(2) / 2,
        "and half the gap back to zero"
    );
}

/// The number the arrows walk is the number Enter spends, and a typed
/// batch is still typed: one expression answers all three.
#[test]
fn an_arrow_and_a_typed_batch_are_the_same_quantity() {
    let mut app = stocked_app(703);
    open_compile_of(&mut app, "ice_breaker");
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.craft_quantity(), 12, "digits still type a quantity");

    app.handle_key(GameKey::Right);
    assert_eq!(
        app.craft_quantity(),
        13,
        "and an arrow steps what was typed"
    );

    let before = held(&app, "ice_breaker");
    app.handle_key(GameKey::Enter);
    assert_eq!(
        held(&app, "ice_breaker"),
        before + 13,
        "Enter compiles the quantity the arrows left"
    );
}

/// How many of `item` the player is holding, across both stores — a
/// compile rolls per unit, so a batch can land in either.
fn held(app: &App, item: &str) -> u32 {
    let id = ItemId::from(item);
    app.game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .filter(|row| row.copy.item == id)
        .map(|row| row.qty)
        .sum()
}
