//! `Mode::Excavate` — the Excavation plan: a cursor, a box, and the marks a
//! crew will one day work.
//!
//! The mode's whole claim is that it is a *mode* and not an action, so the
//! test that matters most here is the one asserting the clock never moves.

use super::support::*;
use crate::*;

/// The party on the pocket's eastern edge, one step from solid rock — the
/// place a plan is actually drawn from.
fn app_at_the_frontier(seed: u32) -> App {
    let mut app = test_app(seed);
    found_the_base(&mut app);
    stand_in_base_at(
        &mut app,
        feral_processes_engine::tuning::STARTING_POCKET_RADIUS,
        0,
    );
    app
}

fn marks(app: &mut App) -> Vec<(i32, i32)> {
    app.game
        .as_mut()
        .expect("a fixture with a game")
        .marked_cells()
}

fn tick(app: &App) -> u64 {
    app.game
        .as_ref()
        .expect("a fixture with a game")
        .current_tick()
}

#[test]
fn m_opens_excavation_plan_in_base_space_and_does_nothing_on_the_surface() {
    let mut app = app_at_the_frontier(4300);
    app.handle_key(GameKey::Char('m'));
    assert_eq!(
        app.mode,
        Mode::Excavate,
        "m must open the plan in base space"
    );

    let mut surface = test_app(4301);
    found_the_base(&mut surface);
    surface.handle_key(GameKey::Char('m'));
    assert_eq!(
        surface.mode,
        Mode::Playing,
        "there is nothing to excavate on the open grid"
    );
    assert!(
        surface.status_line.is_some(),
        "a refused key must say why rather than looking broken"
    );
}

#[test]
fn the_cursor_starts_on_the_party_and_moves_with_the_direction_keys() {
    let mut app = app_at_the_frontier(4302);
    let party = app.game.as_ref().unwrap().base_pos().unwrap();

    app.handle_key(GameKey::Char('m'));
    assert_eq!(
        app.excavate_cursor,
        Some(party),
        "the cursor must open on the one cell the player already knows"
    );

    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Down);
    assert_eq!(
        app.excavate_cursor,
        Some((party.0 + 1, party.1 + 1)),
        "the cursor did not walk with the keys the player walks with"
    );
}

#[test]
fn committing_a_box_reaches_toggle_mark_box() {
    let mut app = app_at_the_frontier(4303);
    let (px, py) = app.game.as_ref().unwrap().base_pos().unwrap();

    app.handle_key(GameKey::Char('m'));
    // Out onto solid rock, so the box is drawn over cells that can take a
    // mark at all.
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Char('j'));
    app.handle_key(GameKey::Char(' '));

    assert_eq!(
        marks(&mut app),
        vec![(px + 1, py), (px + 1, py + 1)],
        "the committed box did not reach the engine's marks"
    );
    assert_eq!(
        app.excavate_anchor, None,
        "a committed box leaves its anchor down"
    );
    assert_eq!(
        app.mode,
        Mode::Excavate,
        "committing must not leave the mode"
    );
}

/// The load-bearing property of the whole mode: planning a wing of the base
/// costs no game time, so entropy is not eating the frontier while you draw.
#[test]
fn excavation_plan_never_ticks_the_game() {
    let mut app = app_at_the_frontier(4304);
    let before = tick(&app);

    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Char('j'));
    app.handle_key(GameKey::Char(' '));
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        tick(&app),
        before,
        "drawing a plan spent game time — a mode is not an action"
    );
}

/// One press is never two undos: with an anchor down, Esc takes back the
/// anchor and leaves the player where they were drawing.
#[test]
fn esc_with_an_anchor_down_drops_the_anchor_and_stays_in_the_mode() {
    let mut app = app_at_the_frontier(4305);
    app.handle_key(GameKey::Char('m'));
    app.handle_key(GameKey::Char('l'));
    app.handle_key(GameKey::Char(' '));
    assert!(app.excavate_anchor.is_some(), "space must drop an anchor");

    app.handle_key(GameKey::Esc);

    assert_eq!(
        app.mode,
        Mode::Excavate,
        "Esc left the mode as well as the anchor"
    );
    assert_eq!(
        app.excavate_anchor, None,
        "Esc did not take the anchor back"
    );
    assert!(
        marks(&mut app).is_empty(),
        "a dropped anchor marked something"
    );

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing, "a second Esc leaves the mode");
    assert_eq!(app.excavate_cursor, None, "leaving must clear the cursor");
}
