//! The three screens a tactical fight is fought through, and the loop that
//! paces the wild side.

use super::support::test_app;
use crate::{App, GameKey, Mode, TACTICAL_TURNS_PER_SECOND, TacticalIntent};

/// An app standing in a fight opened by walking into a lone wild program.
///
/// `battling_app_with`'s sweep, and deliberately the same one: the bump is
/// the real path into `Game::start_battle`, so what this opens is what
/// walking into a pack opens. `tactical` says which model to ask for.
fn fighting_app(tactical: bool) -> App {
    for seed in 0..200u32 {
        let mut app = test_app(seed);
        if tactical {
            app.profile.tactical_battles = true;
            let mut game = app.game.take().expect("the fixture has a game");
            game.install_profile(app.profile.clone());
            app.game = Some(game);
        }
        let game = app.game.as_mut().expect("the fixture has a game");
        let player = game.player_status().position;
        let target = game
            .view_entities(12, 12)
            .into_iter()
            .filter(|e| e.is_hostile && !e.is_tamed && !e.is_structure)
            .find(|e| (e.pos.0 - player.0).abs() + (e.pos.1 - player.1).abs() == 1);
        let Some(target) = target else { continue };
        app.handle_key(match (target.pos.0 - player.0, target.pos.1 - player.1) {
            (1, 0) => GameKey::Right,
            (-1, 0) => GameKey::Left,
            (0, 1) => GameKey::Down,
            _ => GameKey::Up,
        });
        let opened = if tactical {
            app.mode == Mode::TacticalBattle
        } else {
            app.mode == Mode::Battle
        };
        if opened {
            let _ = app.take_sounds();
            return app;
        }
    }
    panic!("no seed under 200 put a lone wild program next to the player");
}

fn fighting(_seed: u32) -> App {
    fighting_app(true)
}

#[test]
fn walking_into_a_pack_with_the_toggle_on_opens_the_battle_map() {
    let app = fighting(9101);
    assert_eq!(app.mode, Mode::TacticalBattle);
    assert!(
        app.game
            .as_ref()
            .expect("the fixture has a game")
            .in_tactical_battle()
    );
}

#[test]
fn the_toggle_off_still_opens_the_abstract_screen() {
    let app = fighting_app(false);
    assert_eq!(app.mode, Mode::Battle);
    assert!(
        !app.game
            .as_ref()
            .expect("the fixture has a game")
            .in_tactical_battle()
    );
}

/// A tactical fight narrates as it resolves, so none of the reveal's
/// machinery may hold its lines back — and a key pressed on the battle map
/// must act rather than being eaten by `handle_key`'s skip.
#[test]
fn a_tactical_fight_is_not_a_reveal() {
    let app = fighting(9103);
    assert!(!app.mode.is_battle(), "the reveal gate would pace the map");
    assert!(!app.is_revealing());
}

/// The cursor opens on the acting body's own cell, never on a hostile: a
/// `Radius` centred on the caster is a legal aim, and a cursor the player
/// did not put there is how a blast lands on the party.
#[test]
fn the_aim_cursor_opens_on_the_acting_body() {
    let mut app = fighting(9104);
    wait_for_the_player(&mut app);
    let acting = acting_cell(&mut app);

    app.handle_key(GameKey::Char('a'));

    assert_eq!(app.mode, Mode::TacticalAim);
    assert_eq!(app.tactical_cursor, Some(acting));
    assert_eq!(app.pending_tactical, Some(TacticalIntent::Swing));
}

#[test]
fn escape_takes_the_cursor_back_without_spending_the_action() {
    let mut app = fighting(9105);
    wait_for_the_player(&mut app);
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.mode, Mode::TacticalAim);

    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::TacticalBattle);
    assert_eq!(app.tactical_cursor, None);
    assert_eq!(app.pending_tactical, None);
    assert!(
        !app.game
            .as_mut()
            .expect("the fixture has a game")
            .tactical_view()
            .expect("the fight is open")
            .acted,
        "backing out of the cursor spent the turn"
    );
}

/// The cursor is held on the board, so a commit always names a cell the
/// engine can answer about.
#[test]
fn the_cursor_stays_on_the_board() {
    let mut app = fighting(9106);
    wait_for_the_player(&mut app);
    app.handle_key(GameKey::Char('a'));
    let side = app
        .game
        .as_mut()
        .expect("the fixture has a game")
        .tactical_view()
        .expect("the fight is open")
        .board
        .side;

    for _ in 0..(side * 2) {
        app.handle_key(GameKey::Left);
        app.handle_key(GameKey::Up);
    }
    assert_eq!(app.tactical_cursor, Some((0, 0)));

    for _ in 0..(side * 2) {
        app.handle_key(GameKey::Right);
        app.handle_key(GameKey::Down);
    }
    assert_eq!(app.tactical_cursor, Some((side - 1, side - 1)));
}

/// `[E]` is uppercase because lowercase letters are row selectors, and it
/// hands the turn on without spending the action.
#[test]
fn end_turn_hands_the_turn_on() {
    let mut app = fighting(9107);
    wait_for_the_player(&mut app);
    assert!(app.tactical_player_turn());

    app.handle_key(GameKey::Char('E'));

    assert!(
        !app.tactical_player_turn(),
        "[E] left the turn where it was"
    );
}

/// The wild side is paced against `dt` and not against the frame — one turn
/// per beat, whatever the machine renders at.
#[test]
fn the_wild_side_is_paced_against_the_clock() {
    let mut app = fighting(9108);
    wait_for_the_player(&mut app);
    app.handle_key(GameKey::Char('E'));
    assert!(!app.tactical_player_turn(), "a wild body is up");

    let before = acting_entity(&mut app);
    // A frame far too short to owe a turn.
    app.advance_tactical(0.001);
    assert_eq!(
        acting_entity(&mut app),
        before,
        "a turn was spent inside a frame that had not paid for one"
    );

    app.advance_tactical(1.0 / TACTICAL_TURNS_PER_SECOND);
    assert_ne!(
        acting_entity(&mut app),
        before,
        "a full beat spent no turn at all"
    );
}

/// Nothing paces while the player is the one being waited on, and the carry
/// is held at zero rather than accumulated so the first wild body after a
/// player's turn waits a full beat.
#[test]
fn the_clock_does_not_run_on_the_players_own_turn() {
    let mut app = fighting(9109);
    wait_for_the_player(&mut app);

    app.advance_tactical(10.0);

    assert!(app.tactical_player_turn(), "the player's turn was spent");
    assert_eq!(app.tactical_carry, 0.0);
}

#[test]
fn nothing_paces_outside_a_tactical_screen() {
    let mut app = fighting(9110);
    app.mode = Mode::Playing;

    app.advance_tactical(10.0);

    assert_eq!(app.tactical_carry, 0.0);
}

/// Hands turns on until the player's comes round.
fn wait_for_the_player(app: &mut App) {
    for _ in 0..64 {
        if app.tactical_player_turn() {
            return;
        }
        let ran = app
            .game
            .as_mut()
            .expect("the fixture has a game")
            .tactical_ai_turn();
        assert!(ran, "nobody is acting and it is not the player");
    }
    panic!("the turn never came round to the player");
}

fn acting_entity(app: &mut App) -> Option<feral_processes_engine::Entity> {
    let view = app.game.as_mut()?.tactical_view()?;
    Some(view.order[view.active?].entity)
}

fn acting_cell(app: &mut App) -> (i32, i32) {
    let view = app
        .game
        .as_mut()
        .expect("the fixture has a game")
        .tactical_view()
        .expect("the fight is open");
    let acting = view.order[view.active.expect("somebody is acting")].entity;
    view.bodies
        .iter()
        .find(|b| b.entity == acting)
        .expect("the acting body is on the board")
        .cell
}

/// **The board's own reach already includes the diagonals** — `walk_field`
/// is Chebyshev, so `movement_field` offers them and the wild side's AI
/// walks them. Until the numpad landed, the player was the one body on the
/// board that could not, which is a manoeuvring advantage handed to the
/// hostiles for no reason anyone chose.
#[test]
fn a_diagonal_the_engine_calls_reachable_is_reachable_by_a_key() {
    let mut app = fighting(9120);
    wait_for_the_player(&mut app);
    let from = acting_cell(&mut app);
    let reachable = app
        .game
        .as_mut()
        .expect("the fixture has a game")
        .tactical_view()
        .expect("the fight is open")
        .reachable;

    let diagonals = [
        ((-1, -1), GameKey::UpLeft),
        ((1, -1), GameKey::UpRight),
        ((-1, 1), GameKey::DownLeft),
        ((1, 1), GameKey::DownRight),
    ];
    let (delta, key) = diagonals
        .into_iter()
        .find(|((dx, dy), _)| reachable.contains(&(from.0 + dx, from.1 + dy)))
        .expect("no diagonal neighbour of the acting body was reachable at all");

    app.handle_key(key);

    assert_eq!(
        acting_cell(&mut app),
        (from.0 + delta.0, from.1 + delta.1),
        "{key:?} did not take the body to the diagonal the engine offered it"
    );
}

/// The abstract fight has called this `[s]pecial` since long before there
/// was a board to fight on, and one fight model teaching a key the other
/// refuses is the whole of the cost.
#[test]
fn the_special_key_is_s_the_way_it_is_in_an_abstract_fight() {
    let mut app = fighting(9121);
    wait_for_the_player(&mut app);
    assert!(
        !app.tactical_routine_rows().is_empty(),
        "the acting body can run nothing, so this would pass on the refusal"
    );

    app.handle_key(GameKey::Char('s'));

    assert_eq!(app.mode, Mode::TacticalRoutine);
}

#[test]
fn r_is_not_a_second_way_into_the_picker() {
    let mut app = fighting(9122);
    wait_for_the_player(&mut app);

    app.handle_key(GameKey::Char('r'));

    assert_eq!(app.mode, Mode::TacticalBattle);
    assert_eq!(app.status_line, None);
}

/// The cursor takes the same eight directions the body walks: a numpad that
/// steers a body but not the cursor it aims with reads as one of the two
/// being broken.
#[test]
fn the_aim_cursor_moves_diagonally_too() {
    let mut app = fighting(9123);
    wait_for_the_player(&mut app);
    app.handle_key(GameKey::Char('a'));
    let (cx, cy) = app.tactical_cursor.expect("the cursor is open");

    app.handle_key(GameKey::DownRight);

    assert_eq!(
        app.tactical_cursor,
        Some((cx + 1, cy + 1)),
        "the cursor ignored a diagonal the body would have walked"
    );
}
