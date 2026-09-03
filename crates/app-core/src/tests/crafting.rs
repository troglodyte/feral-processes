//! The Compile flow: picking a recipe, choosing a quantity, the
//! careful-compile toggle that decides what the batch is worth, and
//! `Mode::Compiling`, the screen that spends `Game::hand_craft_ticks`.

use feral_processes_engine::save;

use super::support::*;
use crate::*;

/// Drives `Mode::Compiling` to completion (or an abort) and back to
/// `Mode::Playing`, the way `advance_compile` is driven every frame by a
/// real frontend. One call with an oversized `dt` drains the whole batch —
/// `advance_compile`'s loop keeps spending ticks until it either runs out of
/// accumulated time or the batch reports `finished`, so a large enough `dt`
/// always reaches the latter first for anything a test fixture affords.
///
/// A batch is hundreds of ticks and `advance_compile` owes `after_tick` for
/// them, so the compile that ends can be the moment a queued notification
/// takes the screen — the low-Power notice most of all, since the ticks that
/// raise it are the ones this just spent. Dismissed here rather than asserted
/// against: which notice is waiting is not what any caller of this is about.
pub(crate) fn drain_compile(app: &mut App) {
    assert_eq!(app.mode, Mode::Compiling, "nothing is compiling to drain");
    app.advance_compile(3600.0);
    while app.mode == Mode::Notification {
        app.handle_key(GameKey::Esc);
    }
    assert_eq!(
        app.mode,
        Mode::Playing,
        "a drained compile should have returned to Mode::Playing"
    );
}

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
        drain_compile(&mut plain);
        drain_compile(&mut careful);
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
    // Two digits, but a small number: a hand-compile burns Power a tick and
    // `tuning::HAND_CRAFT_POWER_FLOOR` refuses a batch past the reserve, so
    // the thirteen this used to type is now a refusal rather than a batch.
    app.handle_key(GameKey::Char('0'));
    app.handle_key(GameKey::Char('5'));
    assert_eq!(app.craft_quantity(), 5, "digits still type a quantity");

    app.handle_key(GameKey::Right);
    assert_eq!(app.craft_quantity(), 6, "and an arrow steps what was typed");

    let before = held(&app, "ice_breaker");
    app.handle_key(GameKey::Enter);
    drain_compile(&mut app);
    assert_eq!(
        held(&app, "ice_breaker"),
        before + 6,
        "Enter compiles the quantity the arrows left"
    );
}

/// Committing a batch arms the loop and opens the screen; it does not drain
/// it in place the way `Game::craft` used to be called synchronously —
/// that's the whole of what `Mode::Compiling` changes about this flow.
#[test]
fn committing_a_craft_opens_the_compiling_screen_rather_than_finishing_at_once() {
    let mut app = stocked_app(710);
    open_compile_of(&mut app, "ice_breaker");
    let before = held(&app, "ice_breaker");

    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.mode,
        Mode::Compiling,
        "a committed batch should open the compiling screen"
    );
    assert!(
        app.game.as_ref().unwrap().hand_craft_in_progress(),
        "the engine should have a compile armed"
    );
    assert_eq!(
        held(&app, "ice_breaker"),
        before,
        "nothing is granted on the frame that opens the screen"
    );
}

/// The spec's answer: "bar fills, and any key aborts." No key is special —
/// there is nothing to page through on this screen, so even Esc just aborts
/// like every other key would.
#[test]
fn a_key_during_compiling_aborts_and_returns_to_playing() {
    let mut app = stocked_app(711);
    open_compile_of(&mut app, "ice_breaker");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Compiling);
    // A tick or two in, but nowhere near finished. Sized in whole seconds
    // because `COMPILE_TICKS_PER_SECOND` is the world's own rate: a tenth
    // of a second buys no tick at all, so the abort would be tested against
    // a batch that had not started.
    app.advance_compile(1.0);

    app.handle_key(GameKey::Char('q'));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "any key should abort back to the map"
    );
    assert!(
        !app.game.as_ref().unwrap().hand_craft_in_progress(),
        "an abort should clear the engine's in-flight compile"
    );
}

/// Advancing to completion is what `drain_compile` already exercises for
/// every other test in this file; this test is the dedicated one for the
/// outcome itself — the mode change and the fact that nothing is left
/// standing over the map as a refusal.
#[test]
fn advancing_to_completion_returns_to_playing_and_reports_the_outcome() {
    let mut app = stocked_app(712);
    open_compile_of(&mut app, "ice_breaker");
    let before = held(&app, "ice_breaker");
    app.handle_key(GameKey::Enter);
    let quantity = 1;

    drain_compile(&mut app);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        held(&app, "ice_breaker"),
        before + quantity,
        "the finished batch should have granted its result"
    );
    assert_eq!(app.status_line, None, "a finished compile is not a refusal");
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

/// A compile spends the world's ticks at the world's own pace.
///
/// `Mode::Compiling` is the only screen that spends ticks against `dt`
/// rather than against a keypress, so its rate is the one place the world
/// can be made to sprint while the player just watches. At the 60/sec it
/// shipped with, a batch ran the clock thirty times faster than standing
/// still did — needs decay, base production, raid pressure and wild spawns
/// all with it — and the only tell was that the bar looked good.
#[test]
fn a_compile_advances_the_world_at_the_idle_tick_rate() {
    let mut app = stocked_app(722);
    open_compile_of(&mut app, "ice_breaker");
    // A batch long enough that one second of it is nowhere near the end,
    // so what this measures is the pace and not a batch running out.
    app.handle_key(GameKey::Char('5'));
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Compiling, "the batch should have armed");
    let before = app.game.as_ref().unwrap().current_tick();

    app.advance_compile(1.0);

    assert_eq!(
        app.game.as_ref().unwrap().current_tick() - before,
        u64::from(WORLD_SPEED_MULTIPLIER),
        "one real second of compiling should move the clock exactly as far \
         as one real second of standing on the map touching nothing"
    );
}

/// A path that spends ticks owes `after_tick()`.
///
/// `handle_key`'s tail and `update_realtime` both end in it; `advance_compile`
/// spends ticks in a loop and did not, so a twelve-shell batch was 3,600
/// ticks with the autosave, the profile flush and the notification queue all
/// suspended for the duration. It self-heals on the next idle tick, so what
/// this pins is the window, not permanent loss — and the next tick-spending
/// path added here has something to fail against.
#[test]
fn a_compile_that_spends_ticks_autosaves_like_any_other_span_of_them() {
    let mut app = stocked_app(720);
    let path = scratch_path("compile_autosave", 720);
    let _ = std::fs::remove_file(&path);
    app.current_save_path = Some(path.clone());
    app.last_autosave_tick = app.game.as_ref().unwrap().current_tick();

    open_compile_of(&mut app, "ice_breaker");
    // Sized against the interval rather than trusting one unit to clear it:
    // `hand_craft_ticks` is a machine's cycle and a cheap recipe's is well
    // under fifty ticks, so a one-unit batch would pass this test without
    // ever reaching an autosave.
    let per_unit = app
        .game
        .as_ref()
        .unwrap()
        .hand_craft_ticks(&ItemId::from("ice_breaker"));
    let quantity = crate::AUTOSAVE_INTERVAL_TICKS as u32 / per_unit + 1;
    for c in quantity.to_string().chars() {
        app.handle_key(GameKey::Char(c));
    }
    assert!(
        quantity * per_unit > crate::AUTOSAVE_INTERVAL_TICKS as u32,
        "the batch has to outlast the autosave interval or this proves nothing"
    );
    app.handle_key(GameKey::Enter);
    drain_compile(&mut app);

    assert!(
        path.exists(),
        "a batch longer than the autosave interval left no autosave behind"
    );
    let _ = std::fs::remove_file(&path);
}

/// A run that ends on a compile tick reaches the game-over screen with the
/// same cue every other ending has.
///
/// `finish_compile` copied `after_world_action`'s two checks and not its
/// third line, so the defeat sound was silently dropped on exactly one of
/// the ways a run can end.
#[test]
fn a_run_that_ends_mid_compile_still_gets_its_defeat_cue() {
    let mut app = stocked_app(721);

    // Through the save, which is the only door app-core has onto the
    // engine's `World`: a run on the difficulty that ends rather than
    // reboots, with the player already at zero. Nothing has ticked yet, so
    // `death_handling_system` has not seen it and the compile still arms.
    let path = scratch_path("compile_gameover", 721);
    app.game.as_mut().unwrap().save(&path).unwrap();
    let mut data = save::load_from_file(&path).unwrap();
    data.difficulty = DifficultyMode::Permadeath;
    data.player.hp = 0;
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());

    open_compile_of(&mut app, "ice_breaker");
    app.handle_key(GameKey::Enter);
    assert_eq!(app.mode, Mode::Compiling, "the batch should have armed");
    let _ = app.take_sounds();

    app.advance_compile(3600.0);

    assert_eq!(
        app.mode,
        Mode::GameOver,
        "the first tick should end the run"
    );
    assert!(
        app.take_sounds().contains(&SoundEvent::Defeat),
        "a run that ends on a compile tick ends as silently as one that does not"
    );
    let _ = std::fs::remove_file(&path);
}
