//! Movement underground: the same four keys, steering a party that has a
//! facing.

use super::support::*;
use crate::*;

fn facing(app: &App) -> String {
    app.game
        .as_ref()
        .unwrap()
        .stack_view()
        .unwrap()
        .facing
        .to_string()
}

fn cell(app: &App) -> (i32, i32) {
    app.game.as_ref().unwrap().stack_view().unwrap().position
}

#[test]
fn the_fixture_actually_puts_the_party_underground() {
    let app = app_underground(303);
    let game = app.game.as_ref().unwrap();
    assert!(game.is_underground());
    assert!(game.stack_view().is_some());
}

/// `+` and `-` mean "zoom the map I am looking at", and underground that is
/// the corner inset rather than the zone map behind it. Separate fields
/// rather than one shared level: climbing out to find the surface tiles
/// resized by a dive spent reading the maze would read as a bug.
#[test]
fn plus_and_minus_zoom_the_stack_map_and_leave_the_surface_zoom_alone() {
    let mut app = app_underground(505);
    let surface_zoom = app.zoom;
    let before = app.stack_zoom;

    app.handle_key(GameKey::Char('+'));
    assert_eq!(app.stack_zoom, before + 1, "+ did not zoom the frame map");
    assert_eq!(app.zoom, surface_zoom, "+ underground resized the zone map");

    app.handle_key(GameKey::Char('-'));
    assert_eq!(app.stack_zoom, before);
    assert_eq!(app.zoom, surface_zoom);
}

/// And the other way about: the same keys on the surface must not quietly
/// re-frame the map waiting for the next descent.
#[test]
fn zooming_on_the_surface_leaves_the_stack_map_alone() {
    let mut app = test_app(606);
    let before = app.stack_zoom;
    app.handle_key(GameKey::Char('+'));
    assert!(app.zoom > MIN_ZOOM);
    assert_eq!(app.stack_zoom, before);
}

/// Both ends clamp, so holding either key parks at a level that still draws
/// rather than at a zero-cell window or a level nothing maps to.
#[test]
fn the_stack_map_zoom_clamps_at_both_ends() {
    let mut app = app_underground(707);
    for _ in 0..12 {
        app.handle_key(GameKey::Char('+'));
    }
    assert_eq!(app.stack_zoom, STACK_MAP_MAX_ZOOM);
    for _ in 0..12 {
        app.handle_key(GameKey::Char('-'));
    }
    assert_eq!(app.stack_zoom, STACK_MAP_MIN_ZOOM);
}

/// The defining difference from the surface: left and right turn the party
/// rather than strafing it sideways.
#[test]
fn left_and_right_turn_the_party_instead_of_moving_it() {
    let mut app = app_underground(303);
    let before = cell(&app);

    app.handle_key(GameKey::Right);
    assert_eq!(
        facing(&app),
        "E",
        "right should turn to face east from north"
    );
    assert_eq!(cell(&app), before, "turning must not move the party");

    app.handle_key(GameKey::Left);
    assert_eq!(facing(&app), "N");
    assert_eq!(cell(&app), before);

    app.handle_key(GameKey::Left);
    assert_eq!(facing(&app), "W");
}

#[test]
fn hjkl_steers_the_same_way_the_arrows_do() {
    let mut arrows = app_underground(404);
    let mut letters = app_underground(404);

    for (arrow, letter) in [
        (GameKey::Right, GameKey::Char('l')),
        (GameKey::Up, GameKey::Char('k')),
        (GameKey::Left, GameKey::Char('h')),
        (GameKey::Down, GameKey::Char('j')),
    ] {
        arrows.handle_key(arrow);
        letters.handle_key(letter);
        assert_eq!(facing(&arrows), facing(&letters));
        assert_eq!(cell(&arrows), cell(&letters));
    }
}

#[test]
fn up_walks_forward_along_the_facing() {
    let mut app = app_underground(303);
    // Turn until there's somewhere to walk, so this isn't testing a wall.
    let mut moved = false;
    for _ in 0..4 {
        let before = cell(&app);
        app.handle_key(GameKey::Up);
        if cell(&app) != before {
            moved = true;
            break;
        }
        app.handle_key(GameKey::Right);
    }
    assert!(moved, "no open direction from the entry cell");
}

/// Down used to back the party up along its facing; it now turns them
/// clean around in place instead, which is what makes a dead end escapable
/// with one key rather than a turn-turn-turn-forward dance.
#[test]
fn down_turns_the_party_around_without_moving_it() {
    let mut app = app_underground(303);
    let before = cell(&app);
    assert_eq!(facing(&app), "N", "the fixture must start facing north");

    app.handle_key(GameKey::Down);

    assert_eq!(cell(&app), before, "turning around must not move the party");
    assert_eq!(facing(&app), "S", "down should turn the party clean around");

    app.handle_key(GameKey::Down);
    assert_eq!(cell(&app), before, "turning around must not move the party");
    assert_eq!(
        facing(&app),
        "N",
        "two about-faces should return to the start"
    );
}

#[test]
fn a_movement_key_underground_still_queues_a_step_sound() {
    let mut app = app_underground(303);
    let _ = app.take_sounds();
    app.handle_key(GameKey::Right);
    let sounds = app.take_sounds();
    assert_eq!(sounds.len(), 1, "got {sounds:?}");
    assert!(matches!(
        sounds[0],
        SoundEvent::Step | SoundEvent::BattleStart
    ));
}

#[test]
fn the_menu_keys_still_open_their_screens_underground() {
    // Party and inventory management is deliberately available down the
    // Stack — see `Game::require_surface`.
    for (key, expected) in [
        (GameKey::Char('i'), Mode::Inventory),
        (GameKey::Char('p'), Mode::PartyMenu),
        (GameKey::Char('?'), Mode::Help),
    ] {
        let mut app = app_underground(303);
        app.handle_key(key);
        assert_eq!(app.mode, expected, "{key:?} should still open its screen");
    }
    // And the party menu still reaches its screens from down there — the
    // rows are non-surface, so none of them are filtered out.
    let mut app = app_underground(303);
    open_via_menu(&mut app, 'p', "Install a routine");
    assert_eq!(app.mode, Mode::RoutineTarget);
}

#[test]
fn g_opens_the_map_underground_and_any_key_closes_it() {
    let mut app = app_underground(707);
    app.handle_key(GameKey::Char('g'));
    assert_eq!(app.mode, Mode::FrameMap);
    assert!(
        app.game.as_ref().unwrap().frame_map().is_some(),
        "the map screen must have a map to draw"
    );

    app.handle_key(GameKey::Char('z'));
    assert_eq!(app.mode, Mode::Playing);
}

/// Reading your own map is not an action. The Stack advancing a turn every
/// time you checked where you were would punish mapping, which is the one
/// thing this screen exists to make easier.
#[test]
fn opening_the_map_costs_no_time() {
    let mut app = app_underground(808);
    let before = app.game.as_ref().unwrap().player_status().position;
    let ticks = app.game.as_ref().unwrap().message_log(200).len();

    app.handle_key(GameKey::Char('g'));

    let game = app.game.as_ref().unwrap();
    assert_eq!(game.player_status().position, before);
    assert_eq!(
        game.message_log(200).len(),
        ticks,
        "the map advanced a turn"
    );
}

/// `g` used to scan the sector for salvage on the surface; that action was
/// deleted in the bounded-income pass. The key is still shared with the
/// Stack's map screen (see `g_opens_the_map_underground_and_any_key_closes_it`),
/// so above ground it now falls through to the no-op arm rather than doing
/// anything — it must neither change mode nor advance a turn.
#[test]
fn g_does_nothing_on_the_surface() {
    let mut app = test_app(909);
    let ticks = app.game.as_ref().unwrap().message_log(200).len();

    app.handle_key(GameKey::Char('g'));

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.game.as_ref().unwrap().message_log(200).len(),
        ticks,
        "g should be a no-op on the surface now that scanning is gone"
    );
}

#[test]
fn taking_the_link_up_from_depth_one_surfaces() {
    let mut app = app_underground(505);
    // The fixture lands the party on the entry cell, which is the way out.
    app.handle_key(GameKey::Char('<'));
    let game = app.game.as_ref().unwrap();
    assert!(!game.is_underground());
    assert!(game.stack_view().is_none());
}

#[test]
fn surfacing_hands_movement_back_to_the_zone_map() {
    let mut app = app_underground(506);
    app.handle_key(GameKey::Char('<'));
    let before = app.game.as_ref().unwrap().player_status().position;

    // A step *or* a bump into a wild program: both are the zone map
    // answering, and which one the party gets is decided by whether the
    // seeded population happens to have put something on the next tile.
    // Requiring the step alone made this fail the day an upstream `GameRng`
    // draw moved that population — the party surfaced beside a hostile and
    // the first arrow key opened a fight instead. A key still routed to the
    // Stack would do neither: it would walk the frame, leaving the surface
    // `Position` pinned to the entrance and starting no surface battle.
    for key in [GameKey::Right, GameKey::Down, GameKey::Left, GameKey::Up] {
        app.handle_key(key);
        if app.game.as_ref().unwrap().player_status().position != before || app.mode == Mode::Battle
        {
            return;
        }
    }
    panic!(
        "no direction reached the zone map after surfacing (mode={:?})",
        app.mode
    );
}

/// Up and down are separate commands, not one key that guesses. Pressing
/// the wrong one on the entry cell must refuse rather than quietly do the
/// other thing.
#[test]
fn descending_from_the_entry_cell_refuses_instead_of_surfacing() {
    let mut app = app_underground(505);
    app.handle_key(GameKey::Char('>'));
    let game = app.game.as_ref().unwrap();
    assert!(
        game.is_underground(),
        "'>' on a way *up* must not take it — that is what '<' is for"
    );
    assert!(
        game.message_log(10)
            .iter()
            .any(|e| e.text.contains("no way down")),
        "the refusal should say why"
    );
}

#[test]
fn the_view_names_the_key_that_takes_the_link() {
    let app = app_underground(505);
    let view = app.game.as_ref().unwrap().stack_view().unwrap();
    let standing = view.standing_on.expect("the entry cell is the way out");
    assert!(
        standing.contains("[<]"),
        "the prompt must name the key, got: {standing}"
    );
}

#[test]
fn links_available_reports_only_what_the_cell_underfoot_offers() {
    let app = app_underground(505);
    let (down, up) = app.game.as_ref().unwrap().links_available();
    assert!(up, "the entry cell is a way up");
    assert!(!down, "and is not also a way down");
}

/// The whole chain: a step underground rolls an encounter, the engine starts
/// the battle, and `after_world_action` drops the app into `Mode::Battle`.
/// That transition is shared with the surface path rather than copied, and
/// this is what proves the Stack path actually reaches it.
#[test]
fn an_encounter_underground_opens_the_battle_screen() {
    let mut app = app_underground(606);
    for i in 0..600 {
        if app.mode == Mode::Battle {
            assert!(
                app.game.as_ref().unwrap().has_active_battle(),
                "Mode::Battle without a battle behind it"
            );
            assert!(
                app.game.as_ref().unwrap().is_underground(),
                "the fight should happen where the party is standing"
            );
            return;
        }
        app.handle_key(if i % 4 == 3 {
            GameKey::Right
        } else {
            GameKey::Up
        });
    }
    panic!("600 steps of corridor never opened a fight");
}

/// The key reaches the engine. The refusal is the assertion rather than a
/// success, because the fixture lands on the entry cell rather than on the
/// frame's orphan and walking one down through the key handler would be a
/// maze solver in a keybinding test — what this has to prove is that `o`
/// gets past the mode block above and into `handle_stack_key` at all.
///
/// `t` would not: it is spent before the underground dispatch is reached —
/// on the stall underfoot, or on saying there isn't one — which the second
/// half asserts so the binding cannot quietly move onto a taken letter.
#[test]
fn o_reaches_adopt_orphan_underground() {
    let mut app = app_underground(505);
    app.handle_key(GameKey::Char('o'));

    assert_eq!(app.mode, Mode::Playing, "'o' must not open a screen");
    assert_eq!(
        app.status_line.as_deref(),
        Some("There's nothing like that here."),
        "the engine's refusal should reach the status line"
    );

    let mut app = app_underground(505);
    app.handle_key(GameKey::Char('t'));
    assert_eq!(
        app.status_line.as_deref(),
        Some("There's nobody selling anything here."),
        "'t' is trading wherever you are standing — the adopt key cannot be it"
    );
}

/// `Z` reaches `Game::listen` and counts as an action. Named by nothing on
/// screen — see `crates/engine/EASTER_EGGS.md` — so this test and the
/// surface half below are the only record that the key is bound at all.
#[test]
fn shift_z_listens_underground_and_costs_a_turn() {
    let mut app = app_underground(505);
    let before = app.game.as_ref().unwrap().current_tick();

    app.handle_key(GameKey::Char('Z'));

    assert_eq!(app.mode, Mode::Playing, "'Z' must not open a screen");
    let game = app.game.as_ref().unwrap();
    assert!(
        game.current_tick() > before,
        "listening should have advanced the world"
    );
    assert!(
        game.message_log(4)
            .iter()
            .any(|line| line.text.starts_with("You go still")),
        "the reading never reached the log"
    );
}

/// On open grid the same key is nothing at all: the engine refuses, so no
/// turn is spent and the surface handler has nothing to say about it.
#[test]
fn shift_z_on_the_surface_does_nothing() {
    let mut app = test_app(505);
    let before = app.game.as_ref().unwrap().current_tick();

    app.handle_key(GameKey::Char('Z'));

    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(
        app.game.as_ref().unwrap().current_tick(),
        before,
        "listening above ground spent a turn"
    );
}

/// Underground, `x` + a direction describes a cell of the frame instead of
/// scanning the surface the party's `Position` is still pinned to.
#[test]
fn x_underground_opens_a_cell_description() {
    let mut app = app_underground(606);
    app.handle_key(GameKey::Char('x'));
    assert_eq!(app.mode, Mode::InspectDirection);

    app.handle_key(GameKey::Up);
    assert_eq!(app.mode, Mode::CellDescribe);
    let text = app
        .pending_description
        .clone()
        .expect("the key always answers");
    assert!(!text.is_empty());
    // `{bearing}` never showing up here is not asserted: the shipped bank
    // never puts the token anywhere but its `sighted` pools, so a check
    // against this fixture's text would be true regardless of whether
    // `cell_paragraph` ever actually expanded it — a permanently-green
    // assertion that reads as coverage it isn't. The engine's own
    // `cell_paragraph_expands_bearing_even_in_a_field_the_shipped_bank_never_uses_it_in`
    // (`tests/descriptions.rs`) proves the substitution itself, with a
    // custom bank built to make the claim fallible.

    // A plain popup: any key leaves, and the text goes with it.
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
    assert!(app.pending_description.is_none());
}

/// The charge a rest is bought with, out of the player's pack.
fn held(app: &App, item: &str) -> u32 {
    app.game
        .as_ref()
        .unwrap()
        .player_status()
        .inventory
        .iter()
        .find(|r| r.copy.item.as_str() == item)
        .map(|r| r.qty)
        .unwrap_or(0)
}

/// A rest is **priced** by locale and never **gated** by it: underground it
/// burns a Power Outlet exactly as it does on the open grid, and
/// `Game::rest` takes neither `require_surface` nor `require_base`.
///
/// What was missing was the key. `handle_stack_key` bound `.` and `e` but
/// not `r`, so the press fell through its `_ => false` arm and was a dead
/// key nothing caught — not a refusal the player could read, but silence.
/// In play that reads as Power Outlets not working in the Stack, which is a
/// bug report about the item rather than about the binding.
#[test]
fn r_rests_underground_and_burns_an_outlet() {
    let mut app = app_underground(707);
    let before = held(&app, feral_processes_engine::items::ids::OUTLET);
    assert!(before > 0, "the fixture must start holding a rest charge");

    app.handle_key(GameKey::Char('r'));

    let game = app.game.as_ref().unwrap();
    assert_eq!(
        held(&app, feral_processes_engine::items::ids::OUTLET),
        before - 1,
        "`r` underground never reached Game::rest: {:?}",
        game.message_log(5)
    );
    let status = game.player_status();
    assert_eq!(
        status.hp, status.max_hp,
        "a successful rest fully heals the player"
    );
}

/// The other half: with no charge in the pack, `r` underground must refuse
/// exactly as it does on the open grid — burning nothing and never
/// advancing the world, since a refusal is not an action
/// (`App::after_world_action` only fires when `acted` is true).
#[test]
fn r_underground_with_no_charge_is_refused_and_spends_nothing() {
    let mut app = app_underground_with_no_rest_charge(808);
    assert_eq!(
        held(&app, feral_processes_engine::items::ids::OUTLET),
        0,
        "the fixture must hold no rest charge"
    );
    let (x_before, y_before) = cell(&app);
    let facing_before = facing(&app);
    let lines_before = app.game.as_ref().unwrap().message_log(50).len();

    app.handle_key(GameKey::Char('r'));

    let game = app.game.as_ref().unwrap();
    // The load-bearing assertion: a dead key that never reaches
    // `Game::rest` and a real refusal both leave the pack untouched and the
    // party where it stood, so those alone would pass whether `r` is bound
    // here or not. Only `Game::rest` writes this line — proving it is what
    // tells a genuine refusal apart from the key falling through
    // `handle_stack_key`'s `_ => false` arm in silence.
    let log = game.message_log(50);
    assert_eq!(
        log.len(),
        lines_before + 1,
        "`r` underground never reached Game::rest, so no refusal was logged: {log:?}"
    );
    assert!(
        log.last().unwrap().text.contains("power down"),
        "expected Game::rest's own refusal line, got: {log:?}"
    );
    assert_eq!(
        held(&app, feral_processes_engine::items::ids::OUTLET),
        0,
        "a refused rest must not spend a charge it never had"
    );
    assert_eq!(
        (x_before, y_before),
        cell(&app),
        "a refused rest is not an action and must not move the party"
    );
    assert_eq!(
        facing_before,
        facing(&app),
        "a refused rest must not advance the world at all"
    );
}

/// `e` (`Game::use_power_source`, a Power Cell — Power only, no heal, no
/// Integrity) was already bound underground before the `r` fix above. This
/// pins that it actually reaches the engine down there too, so the two
/// keys are not conflated again: `r` burns an Outlet and fully restores the
/// party, `e` burns a Power Cell and restores Power alone.
#[test]
fn e_restores_power_underground_and_burns_a_power_cell() {
    let mut app = app_underground(909);
    let before = held(&app, feral_processes_engine::items::ids::POWER_CELL);
    assert!(before > 0, "the fixture must start holding a Power Cell");

    app.handle_key(GameKey::Char('e'));

    let game = app.game.as_ref().unwrap();
    assert_eq!(
        held(&app, feral_processes_engine::items::ids::POWER_CELL),
        before - 1,
        "`e` underground never reached Game::use_power_source: {:?}",
        game.message_log(5)
    );
}
