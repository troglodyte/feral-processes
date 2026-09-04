//! The settlement hub: opened by walking into a town's tile, or by `x`
//! toward one, and either way landing on `Mode::Settlement`.

use super::support::*;
use crate::*;

#[test]
fn bumping_a_settlement_opens_its_page_with_the_key_set() {
    let mut app = test_app(950);
    let (key, _) = place_settlement_east_of_player(&mut app);

    app.handle_key(GameKey::Right);

    assert_eq!(app.mode, Mode::Settlement);
    assert_eq!(app.pending_settlement, Some(key));
}

/// The tile does not admit you — `Game::move_player`'s settlement arm
/// returns before the walkable step below it ever runs — so the player must
/// still be standing where they started.
#[test]
fn bumping_a_settlement_does_not_move_the_player() {
    let mut app = test_app(951);
    place_settlement_east_of_player(&mut app);
    let before = app.game.as_ref().unwrap().player_status().position;

    app.handle_key(GameKey::Right);

    let after = app.game.as_ref().unwrap().player_status().position;
    assert_eq!(before, after, "a settlement admits nobody");
}

#[test]
fn esc_returns_to_playing_and_clears_the_pending_settlement() {
    let mut app = test_app(952);
    place_settlement_east_of_player(&mut app);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Settlement);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.pending_settlement, None);
}

/// The drain, asserted from the app's side. `Game::take_settlement_visit`
/// only ever answers `Some` once — `after_world_action` calls it on *every*
/// action that advanced the world, not only a bump — so a keypress spent on
/// something else entirely, after Esc already backed out of the page once,
/// must not reopen it. Against a plain getter (the bug this drain exists to
/// close) `PendingVisit` would still hold the first bump's key, and the wait
/// below — which never touches `find_settlement_at` at all — would read it
/// right back and reopen the page for an action that had nothing to do with
/// the settlement.
#[test]
fn an_unrelated_action_after_esc_does_not_reopen_the_page() {
    let mut app = test_app(953);
    place_settlement_east_of_player(&mut app);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Settlement);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char('.'));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "a wait queues no settlement visit, so nothing should have reopened the page"
    );
}

/// Fix for the HIGH-severity review finding: `Game::move_player`'s
/// settlement arm calls `self.tick()` before app-core ever sees the bump,
/// and `tick_inner` calls `nest_aggro_tick` (`turn.rs:236`), which can call
/// `start_battle` for a `Pursuing` guardian within chebyshev 1 of the
/// player (`turn.rs:382`). So a fight can begin *inside* the settlement
/// bump's own tick, and it must win the mode over `Mode::Settlement`.
#[test]
fn a_battle_starting_inside_the_settlement_bump_wins_the_mode() {
    let mut app = test_app(960);
    place_settlement_and_a_pursuing_guardian(&mut app);

    app.handle_key(GameKey::Right);

    assert_eq!(
        app.mode,
        Mode::Battle,
        "nest_aggro_tick can start a battle inside the settlement bump's own tick — the \
         battle must win"
    );
}

/// The cue still has to drain even though the battle won, or it would sit
/// in `PendingVisit` and reopen `Mode::Settlement` on some later, unrelated
/// action once the fight is over — the same hazard
/// `an_unrelated_action_after_esc_does_not_reopen_the_page` guards on the
/// Esc side.
#[test]
fn the_settlement_cue_drains_even_when_a_battle_wins_the_bump() {
    let mut app = test_app(961);
    place_settlement_and_a_pursuing_guardian(&mut app);
    app.handle_key(GameKey::Right);
    assert_eq!(app.mode, Mode::Battle);

    for _ in 0..60 {
        if !app.game.as_ref().is_some_and(|g| g.has_active_battle()) {
            break;
        }
        app.handle_key(GameKey::Char('a'));
        app.finish_reveal();
    }
    assert!(
        !app.game.as_ref().is_some_and(|g| g.has_active_battle()),
        "sixty rounds did not finish the fight — combat setup changed"
    );
    assert_eq!(app.mode, Mode::BattleResult);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    app.handle_key(GameKey::Char('.'));

    assert_eq!(
        app.mode,
        Mode::Playing,
        "a leaked settlement cue reopened the page on an unrelated action after the battle"
    );
}

/// `x` toward a settlement finds it through `find_target_in_direction` and
/// opens the same page a bump would, `InspectTarget::Settlement`'s reason
/// for existing rather than falling into `Mode::CellDescribe` the way a
/// caravan or a build site do.
#[test]
fn examining_a_settlement_opens_the_same_page() {
    let mut app = test_app(954);
    let (key, _) = place_settlement_east_of_player(&mut app);

    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::InspectDirection);
    app.handle_key(GameKey::Right);

    assert_eq!(app.mode, Mode::Settlement);
    assert_eq!(app.pending_settlement, Some(key));
}
