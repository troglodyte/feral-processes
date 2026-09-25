//! `App::travel_to` and `Walk::Travel`'s life on the clock — the click side
//! of `travel-on-the-clock`. The queued-arrow half (`Walk::Step`) lives in
//! `tests/playing.rs`, beside the idle clock it shares a loop body with.

use super::support::*;
use crate::*;

fn ticks_of(app: &mut App, n: u32) {
    let tick = 1.0 / app.world_speed.ticks_per_second();
    for _ in 0..n {
        app.update_realtime(tick);
    }
}

fn player_pos(app: &App) -> (i32, i32) {
    app.game.as_ref().unwrap().player_status().position
}

/// A click on open ground sets a `Tile` goal and spends nothing yet — the
/// clock, not `travel_to` itself, is what moves the party.
#[test]
fn travel_to_a_tile_queues_a_travel_and_moves_nothing_yet() {
    let mut app = test_app(2701);
    clear_the_area_around_player(&mut app);
    let start = player_pos(&app);
    let start_tick = app.game.as_ref().unwrap().current_tick();

    app.travel_to(start.0 + 3, start.1);

    assert_eq!(
        app.walk,
        Some(Walk::Travel {
            goal: TravelGoal::Tile(start.0 + 3, start.1),
            in_base: false,
        })
    );
    assert_eq!(player_pos(&app), start, "travel_to moved the player itself");
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        start_tick,
        "travel_to spent a tick itself"
    );
}

/// A click on a hostile's own tile resolves to a `Creature` goal rather
/// than a `Tile` one — `views::drawn_on_surface_map`'s rule, read the same
/// way the map itself is drawn.
#[test]
fn travel_to_a_hostiles_tile_queues_a_creature_goal() {
    let mut app = test_app(2702);
    let hostile = place_wild_program_east(&mut app, 4);
    let start = player_pos(&app);

    app.travel_to(start.0 + 4, start.1);

    assert_eq!(
        app.walk,
        Some(Walk::Travel {
            goal: TravelGoal::Creature(hostile),
            in_base: false,
        })
    );
}

/// `Mode::Playing` only — every picker, popup and fight already refuses
/// `handle_playing_key` itself; a click reaching `travel_to` from one of
/// those screens must refuse the same way rather than queuing a walk no
/// key on that screen could ever spend.
#[test]
fn travel_to_off_the_playing_screen_is_refused() {
    let mut app = test_app(2703);
    app.mode = Mode::Inventory;

    app.travel_to(5, 5);

    assert_eq!(app.walk, None);
}

/// Underground has no clicked tile to walk to — the Stack is first-person,
/// and `Game::travel_step` answers `NoRoute` unconditionally down there
/// anyway, so `travel_to` says so before a tick is ever spent finding it
/// out.
#[test]
fn travel_to_underground_is_refused() {
    let mut app = app_underground(2704);

    app.travel_to(5, 5);

    assert_eq!(app.walk, None);
}

/// A travel toward open ground walks there one cell a tick and clears
/// itself on arrival, with no "No clear way there" refusal along the way.
#[test]
fn a_travel_arrives_and_clears_itself() {
    let mut app = test_app(2705);
    clear_the_area_around_player(&mut app);
    let start = player_pos(&app);
    let target = (start.0 + 3, start.1);

    app.travel_to(target.0, target.1);
    ticks_of(&mut app, 10);

    assert_eq!(player_pos(&app), target, "the party never arrived");
    assert_eq!(app.walk, None, "an arrived travel must clear itself");
    assert_eq!(
        app.status_line, None,
        "an arriving travel must not refuse itself along the way"
    );
}

/// A travel toward a hostile's tile follows it there and ends the walk in
/// a fight rather than stalling beside it — `TravelStep::Last` is an
/// ordinary bump, and a bump on a hostile fights.
#[test]
fn a_travel_toward_a_hostile_ends_in_a_fight() {
    let mut app = test_app(2706);
    let start = player_pos(&app);
    place_wild_program_east(&mut app, 3);

    app.travel_to(start.0 + 3, start.1);
    ticks_of(&mut app, 10);

    assert!(
        app.game.as_ref().unwrap().has_active_battle(),
        "walking up to a hostile must fight it, the way bumping it does"
    );
    assert_eq!(
        app.walk, None,
        "a travel that opened a fight must clear itself"
    );
}

/// Any non-move key ends a pending travel before it is handled — the
/// player asked for something else, not another tick of the old route.
#[test]
fn a_travel_ends_on_a_key() {
    let mut app = test_app(2707);
    clear_the_area_around_player(&mut app);
    let start = player_pos(&app);
    app.travel_to(start.0 + 5, start.1);
    assert!(app.walk.is_some(), "test premise: a travel is pending");

    app.handle_key(GameKey::Char('.'));

    assert_eq!(app.walk, None, "waiting did not end the pending travel");
}

/// A route that doesn't exist ends the travel with the status line the
/// player reads, rather than being retried forever.
#[test]
fn a_travel_with_no_route_refuses_with_the_status_line() {
    let mut app = test_app(2708);
    let start = player_pos(&app);
    box_in_player_with_hostiles(&mut app);

    app.travel_to(start.0 + 20, start.1);
    ticks_of(&mut app, 1);

    assert_eq!(app.walk, None, "a NoRoute travel must clear itself");
    assert_eq!(
        app.status_line.as_deref(),
        Some("No clear way there."),
        "a NoRoute travel must report itself on the status line"
    );
}

/// A crossing from the surface into base space (or back) between the tick
/// a travel was set and the tick that would spend it ends the walk rather
/// than routing it through a locale the goal was never resolved against.
#[test]
fn a_travel_ends_on_a_change_of_space() {
    let mut app = test_app(2709);
    clear_the_area_around_player(&mut app);
    let start = player_pos(&app);
    app.travel_to(start.0 + 5, start.1);
    assert_eq!(
        app.walk,
        Some(Walk::Travel {
            goal: TravelGoal::Tile(start.0 + 5, start.1),
            in_base: false,
        })
    );

    // The same save-edit trick `stand_in_base` uses: the run crosses into
    // base space by a means other than the walk itself, between the click
    // and the tick that would have spent it.
    stand_in_base(&mut app);
    ticks_of(&mut app, 1);

    assert_eq!(
        app.walk, None,
        "a travel set on one side of the base boundary must not survive a crossing to the other"
    );
}
