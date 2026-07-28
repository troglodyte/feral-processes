//! Movement underground: the same four keys, steering a party that has a
//! facing.

use feral_processes_engine::dungeon::{Dir, generate};
use feral_processes_engine::resources::Locale;
use feral_processes_engine::save;

use super::support::*;
use crate::*;

/// An `App` standing on the entry cell of dungeon level 1.
///
/// Built by editing a save and reloading it, the same trick
/// `app_owning_distant_programs` uses: the engine deliberately exposes no
/// way to drop the player into a dungeon from outside the crate, since on a
/// real run that only ever happens by walking onto an entrance.
fn app_underground(seed: u32) -> App {
    // Counted rather than keyed on the seed alone: tests run in parallel and
    // several share a seed, so a seed-named scratch file has two of them
    // reading and deleting the same path.
    static NEXT: std::sync::atomic::AtomicU32 = std::sync::atomic::AtomicU32::new(0);
    let unique = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

    let assets_dir = test_assets_dir();
    let mut app = test_app(seed);
    let path = std::env::temp_dir().join(format!(
        "feral_processes_appcore_dungeon_{seed}_{unique}.sav"
    ));
    let game = app.game.as_mut().unwrap();
    game.save(&path).unwrap();

    let mut data = save::load_from_file(&path).unwrap();
    let entry = generate(data.seed, 1).entry;
    data.locale = Locale::Dungeon {
        depth: 1,
        x: entry.0,
        y: entry.1,
        facing: Dir::North,
        entrance: data.player.position,
    };
    save::save_to_file(&path, &data).unwrap();

    app.game = Some(Game::load(&path, &assets_dir).unwrap());
    let _ = std::fs::remove_file(&path);
    app.mode = Mode::Playing;
    app
}

fn facing(app: &App) -> String {
    app.game
        .as_ref()
        .unwrap()
        .dungeon_view()
        .unwrap()
        .facing
        .to_string()
}

fn cell(app: &App) -> (i32, i32) {
    app.game.as_ref().unwrap().dungeon_view().unwrap().position
}

#[test]
fn the_fixture_actually_puts_the_party_underground() {
    let app = app_underground(303);
    let game = app.game.as_ref().unwrap();
    assert!(game.is_underground());
    assert!(game.dungeon_view().is_some());
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
fn forward_walks_along_the_facing_and_back_retreats_along_it() {
    let mut app = app_underground(303);
    // Turn until there's somewhere to walk, so this isn't testing a wall.
    let mut moved = false;
    for _ in 0..4 {
        let before = cell(&app);
        app.handle_key(GameKey::Up);
        if cell(&app) != before {
            let advanced = cell(&app);
            app.handle_key(GameKey::Down);
            assert_eq!(cell(&app), before, "back should undo forward");
            assert_ne!(advanced, before);
            moved = true;
            break;
        }
        app.handle_key(GameKey::Right);
    }
    assert!(moved, "no open direction from the entry cell");
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
    // Party and inventory management is deliberately available down a
    // dungeon — see `Game::require_surface`.
    for (key, expected) in [
        (GameKey::Char('v'), Mode::Inventory),
        (GameKey::Char('p'), Mode::Companion),
        (GameKey::Char('m'), Mode::RoutineTarget),
        (GameKey::Char('?'), Mode::Help),
    ] {
        let mut app = app_underground(303);
        app.handle_key(key);
        assert_eq!(app.mode, expected, "{key:?} should still open its screen");
    }
}

#[test]
fn taking_the_stairs_up_from_depth_one_surfaces() {
    let mut app = app_underground(505);
    // The fixture lands the party on the entry cell, which is the way out.
    app.handle_key(GameKey::Char('<'));
    let game = app.game.as_ref().unwrap();
    assert!(!game.is_underground());
    assert!(game.dungeon_view().is_none());
}

#[test]
fn surfacing_hands_movement_back_to_the_zone_map() {
    let mut app = app_underground(505);
    app.handle_key(GameKey::Char('<'));
    let before = app.game.as_ref().unwrap().player_status().position;

    for key in [GameKey::Right, GameKey::Down, GameKey::Left, GameKey::Up] {
        app.handle_key(key);
        if app.game.as_ref().unwrap().player_status().position != before {
            return;
        }
    }
    panic!("no direction moved the player after surfacing");
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
            .any(|(_, l)| l.contains("no way down")),
        "the refusal should say why"
    );
}

#[test]
fn the_view_names_the_key_that_takes_the_stairs() {
    let app = app_underground(505);
    let view = app.game.as_ref().unwrap().dungeon_view().unwrap();
    let standing = view.standing_on.expect("the entry cell is the way out");
    assert!(
        standing.contains("[<]"),
        "the prompt must name the key, got: {standing}"
    );
}

#[test]
fn stairs_available_reports_only_what_the_cell_underfoot_offers() {
    let app = app_underground(505);
    let (down, up) = app.game.as_ref().unwrap().stairs_available();
    assert!(up, "the entry cell is a way up");
    assert!(!down, "and is not also a way down");
}
