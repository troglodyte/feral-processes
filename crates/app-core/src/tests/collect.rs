//! The collect picker — `Mode::Collect` and the basket it builds.

use super::support::*;
use crate::*;

/// The item the fixtures stock. Anything with a name would do; naming it
/// once keeps the tests reading as being about quantities.
const ITEM: &str = "core_fragment";

fn item(id: &str) -> feral_processes_engine::items::ItemId {
    feral_processes_engine::items::ItemId::from(id)
}

/// `c` beside a stocked machine opens the window rather than emptying the
/// shelves. Every row starts at zero — nothing is selected by default, which
/// is the whole of the change.
#[test]
fn c_opens_the_collect_window_with_an_empty_basket() {
    let mut app = app_beside_stocked_machines(970, &[(ITEM, 6)]);

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.mode, Mode::Collect);
    let offer = app.game.as_ref().unwrap().collectable_adjacent();
    assert_eq!(app.collect_rows, offer);
    assert_eq!(
        app.collect_rows,
        vec![(item(ITEM), 12)],
        "pooled, both sides"
    );
    assert_eq!(app.collect_basket, vec![0]);
}

/// Opening a screen is not an action, and both halves of that are here.
///
/// The tick half is about the engine: nothing is taken, so nothing is
/// charged, and the commit is the first tick this whole flow spends. The
/// `status_line` half is about `acted`, which is what `after_world_action`
/// reads — returning `true` from a keypress that only opened a window wipes
/// whatever the last refusal was still explaining.
#[test]
fn opening_the_collect_window_spends_no_turn() {
    let mut app = app_beside_stocked_machines(971, &[(ITEM, 6)]);
    let before = app.game.as_ref().unwrap().current_tick();
    app.status_line = Some("something the player still needs to read".into());

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.game.as_ref().unwrap().current_tick(), before);
    assert!(
        app.status_line.is_some(),
        "opening a window is not an action, so it clears nothing"
    );
}

/// The refusal is unchanged and stays the engine's: with nothing on offer
/// `c` never opens the window, so there is no empty screen to back out of
/// and no second copy of the sentence in app-core.
#[test]
fn c_with_nothing_adjacent_opens_no_window() {
    let mut app = test_app(972);
    stand_in_base(&mut app);

    app.handle_key(GameKey::Char('c'));

    assert_eq!(app.mode, Mode::Playing);
    assert!(app.collect_rows.is_empty());
    assert!(app.collect_basket.is_empty());
}
