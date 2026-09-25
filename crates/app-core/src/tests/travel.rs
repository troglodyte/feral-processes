//! `App::travel_to` and `Walk::Travel`'s life on the clock — the click side
//! of `travel-on-the-clock`. The queued-arrow half (`Walk::Step`) lives in
//! `tests/playing.rs`, beside the idle clock it shares a loop body with.

use super::support::*;
use crate::*;
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save;

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

/// **The regression finding 3 exists for.** "A travel set while paused
/// waits for the clock" is the design's own line — `update_realtime` used
/// to fall through its single `mode != Playing || paused || no game` guard
/// and drop the walk on the paused branch too, exactly as it must on a
/// mode change.
///
/// Pause and unpause are set on `app.paused` directly rather than through
/// `GameKey::Char(' ')` — `handle_playing_key`'s own "any non-move key
/// clears a walk" rule (finding 6) would otherwise end the travel on the
/// *unpause* keypress itself, which is `update_realtime`'s own behaviour
/// to prove, not that separate rule's.
#[test]
fn a_travel_set_while_paused_waits_for_the_clock() {
    let mut app = test_app(2710);
    clear_the_area_around_player(&mut app);
    app.paused = true;
    let start = player_pos(&app);
    let start_tick = app.game.as_ref().unwrap().current_tick();

    app.travel_to(start.0 + 3, start.1);
    assert!(app.walk.is_some(), "test premise: a travel is pending");
    ticks_of(&mut app, 3);

    assert_eq!(
        player_pos(&app),
        start,
        "a paused travel must not move the party"
    );
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        start_tick,
        "a paused travel must not spend a tick"
    );
    assert!(
        app.walk.is_some(),
        "update_realtime must not drop a walk merely because the clock is paused"
    );

    app.paused = false;
    ticks_of(&mut app, 10);

    assert_eq!(
        player_pos(&app),
        (start.0 + 3, start.1),
        "unpausing must resume the travel that was waiting"
    );
}

/// `App::install_game` — the one door a new game and a load both share —
/// clears a pending walk: a route queued against one run means nothing
/// once the party underneath it is swapped for another.
#[test]
fn install_game_clears_a_pending_walk() {
    let mut source = test_app(2711);
    let path = scratch_path("install_clears_walk", 2711);
    source.game.as_mut().unwrap().save(&path).unwrap();

    let mut app = test_app(2711);
    clear_the_area_around_player(&mut app);
    let start = player_pos(&app);
    app.travel_to(start.0 + 3, start.1);
    assert!(app.walk.is_some(), "test premise: a travel is pending");

    app.load_game(path.clone());
    let _ = std::fs::remove_file(&path);

    assert_eq!(app.walk, None, "install_game must clear a pending walk");
}

/// A local helper for injecting a structure straight into a save, the way
/// `app_inside_a_small_base_with_programs` does for its Mining Node —
/// `save::StructureSave` has no `Default`, so every field needs a value,
/// and a wall and a portal both need the same harmless ones.
fn structure_save(kind: &str, x: i32, y: i32) -> save::StructureSave {
    save::StructureSave {
        kind: kind.to_string(),
        position: (x, y),
        durability: None,
        tier: None,
        stock_input: Vec::new(),
        stock_output: Vec::new(),
        standing_work: false,
        standing_guard: false,
        denied_items: Vec::new(),
        power_fuel: feral_processes_engine::tuning::POWER_UPKEEP_TICKS,
        build_quality: 1.0,
        racked: Vec::new(),
        hopper: Vec::new(),
        hopper_progress: 0,
        standing_tool: None,
    }
}

/// **The regression finding 8's "no-progress `Toward` clear" asks for.** A
/// base-space Portal is the one cell `Game::base_step_blocked` admits to a
/// route — `Game::move_in_base` breaches on it rather than refusing it —
/// while never actually landing the party there, since `enter_next_zone`
/// never touches `Locale::Base`. A `Toward` step onto it therefore ticks
/// without moving anybody, and `spend_walk_tick`'s own "did this step make
/// progress" check is what stops the very next tick asking `travel_step`
/// the same stalled question forever.
///
/// The corridor is walled on both long sides (`y = ±1`) rather than left
/// open, or the router — nothing in `Game::base_step_blocked` refuses a
/// Portal — would just as happily route *around* one it has no reason to
/// prefer; the walls are what force the one straight path across it.
#[test]
fn a_toward_step_that_makes_no_progress_clears_the_walk() {
    let mut app = test_app(2713);
    found_the_base(&mut app);
    let path = scratch_path("stalled_toward", 2713);
    app.game.as_mut().unwrap().save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    data.locale = Locale::Base { x: -4, y: 0 };
    for x in -4..=4 {
        data.structures.push(structure_save("wall", x, 1));
        data.structures.push(structure_save("wall", x, -1));
    }
    data.structures.push(structure_save("portal", 2, 0));
    save::save_to_file(&path, &data).unwrap();
    app.game = Some(Game::load(&path, &test_assets_dir()).unwrap());
    let _ = std::fs::remove_file(&path);

    app.travel_to(4, 0);
    ticks_of(&mut app, 8);

    assert_eq!(
        app.game.as_ref().unwrap().base_pos(),
        Some((1, 0)),
        "the party must have stalled one cell short of the portal, never having landed on it"
    );
    assert_eq!(
        app.walk, None,
        "a Toward step that made no progress must clear the walk"
    );
}
