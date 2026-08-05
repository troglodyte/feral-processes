//! The field-cast flow: `Mode::FieldCast` and `Mode::FieldCastAlly` — see
//! `Game::field_routines`/`Game::cast_field_routine`.

use super::support::*;
use crate::*;

#[test]
fn a_from_playing_opens_field_cast() {
    let mut app = test_app(70);
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.mode, Mode::FieldCast);
}

#[test]
fn a_whole_party_routine_casts_immediately_and_returns_to_playing() {
    let mut app = app_with_player_routines(71, &["trace_analysis"], 100.0);
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.mode, Mode::FieldCast);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.status_line, None,
        "cast was refused with: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::Playing);

    let buffs = app.game.as_mut().unwrap().active_buffs();
    assert!(
        buffs.iter().any(|b| b.name == "Trace Analysis Party"),
        "the buff should now be running"
    );
}

#[test]
fn a_one_ally_routine_opens_the_ally_picker_then_casts_on_the_pick() {
    let mut app = app_with_player_routines(72, &["hardened_shell"], 100.0);
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.mode, Mode::FieldCast);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode,
        Mode::FieldCastAlly,
        "a OneAlly routine needs a target before it can cast"
    );
    assert_eq!(app.pending_field_routine, Some(0));

    app.handle_key(GameKey::Char('1')); // "You" — the only holder, and the only ally
    assert_eq!(
        app.status_line, None,
        "cast was refused with: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.pending_field_routine, None);

    let buffs = app.game.as_mut().unwrap().active_buffs();
    assert!(
        buffs.iter().any(|b| b.name == "Hardened Shell Single"),
        "the buff should now be running on the player"
    );
}

#[test]
fn escape_backs_out_of_field_cast_without_casting_anything() {
    let mut app = app_with_player_routines(73, &["trace_analysis"], 100.0);
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.mode, Mode::FieldCast);
    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);

    let buffs = app.game.as_mut().unwrap().active_buffs();
    assert!(buffs.is_empty(), "escaping must not have cast anything");
}

#[test]
fn escape_backs_out_of_the_ally_picker_to_field_cast_without_casting_anything() {
    let mut app = app_with_player_routines(74, &["hardened_shell"], 100.0);
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::FieldCastAlly);

    app.handle_key(GameKey::Esc);
    assert_eq!(
        app.mode,
        Mode::FieldCast,
        "backs out one step, same as BattleAlly -> BattleSpecial"
    );
    assert_eq!(app.pending_field_routine, None);

    let buffs = app.game.as_mut().unwrap().active_buffs();
    assert!(buffs.is_empty(), "escaping must not have cast anything");

    app.handle_key(GameKey::Esc);
    assert_eq!(app.mode, Mode::Playing);
}

#[test]
fn an_unaffordable_routine_is_refused_and_does_not_change_mode() {
    // hardened_shell costs 14.0 Power; 5.0 Hunger can't cover it.
    let mut app = app_with_player_routines(75, &["hardened_shell"], 5.0);
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.mode, Mode::FieldCast);
    app.handle_key(GameKey::Char('1'));

    assert_eq!(
        app.mode,
        Mode::FieldCast,
        "a refused cast must not advance to the ally picker or back to Playing"
    );
    assert!(app.status_line.is_some(), "the refusal should be reported");

    let buffs = app.game.as_mut().unwrap().active_buffs();
    assert!(buffs.is_empty(), "nothing should have been armed");
}

#[test]
fn the_ally_picker_offers_only_the_player_and_the_active_party() {
    let mut app = app_with_owned_and_wild_neighbors(76, &["hardened_shell"]);
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::FieldCastAlly);

    // Just "You": the fixture's owned program is never added to the party
    // (`party_slot: None`), and a `Creature`-scoped buff only ever ticks on
    // the player and the party (`Game::tick_field_buffs`) — offering a
    // benched program here used to let a cast pay Power for a buff that
    // ticked nowhere, same bug as offering the wild neighbor would be.
    let offered = app.field_ally_options();
    assert_eq!(
        offered.len(),
        1,
        "only the player should be offered, found {} rows",
        offered.len()
    );
    assert_eq!(offered[0].name, "You");

    let owned = app
        .game
        .as_mut()
        .unwrap()
        .view_entities(50, 50)
        .into_iter()
        .find(|e| !e.is_player && e.is_tamed)
        .expect("the fixture placed one owned, non-party program");
    assert!(
        offered.iter().all(|o| o.entity != owned.entity),
        "an owned program that isn't in the active party must not be offered"
    );

    let wild = app
        .game
        .as_mut()
        .unwrap()
        .view_entities(50, 50)
        .into_iter()
        .find(|e| !e.is_player && !e.is_tamed)
        .expect("the fixture placed one wild neighbor");
    assert!(
        offered.iter().all(|o| o.entity != wild.entity),
        "the wild neighbor must never be offered as a cast target"
    );

    // A key past the end of the (player-only) list does nothing — proof the
    // picker has no second row to select.
    app.handle_key(GameKey::Char('2'));
    assert_eq!(app.mode, Mode::FieldCastAlly);

    // The one real row, "You", still casts fine.
    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.status_line, None,
        "cast was refused with: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::Playing);

    let buffs = app.game.as_mut().unwrap().active_buffs();
    assert!(
        buffs.iter().any(|b| b.name == "Hardened Shell Single"),
        "the buff should now be running on the player"
    );
}

/// The cell picker (`Mode::FieldCastCell`) opens only for a routine whose
/// `FieldRoutineView::second_pick` asks for one.
#[test]
fn only_a_cell_routine_opens_the_cell_picker() {
    let mut app = app_underground_with_routines(90, &["wild_jump"]);
    app.handle_key(GameKey::Char('a'));
    assert_eq!(app.mode, Mode::FieldCast);
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::FieldCastCell);
    assert_eq!(app.pending_field_routine, Some(0));
    assert_eq!(
        app.field_cursor,
        app.game.as_ref().unwrap().stack_pos_xy(),
        "the cursor opens on the party's own cell"
    );

    // Buffer Overrun needs no second pick and commits from the list.
    let mut app = app_underground_with_routines(91, &["buffer_overrun"]);
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.field_cursor, None);
}

/// An out-of-bounds coordinate is unreachable rather than lethal — the
/// engine refuses one anyway, but the player must never be able to aim at
/// one in the first place.
#[test]
fn the_cursor_clamps_to_the_frame_bounds() {
    let mut app = app_underground_with_routines(92, &["wild_jump"]);
    let (w, h) = app.game.as_ref().unwrap().frame_bounds().unwrap();
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));

    for _ in 0..(w + h + 10) {
        app.handle_key(GameKey::Left);
        app.handle_key(GameKey::Up);
    }
    assert_eq!(app.field_cursor, Some((0, 0)));

    for _ in 0..(w + h + 10) {
        app.handle_key(GameKey::Right);
        app.handle_key(GameKey::Down);
    }
    assert_eq!(app.field_cursor, Some((w - 1, h - 1)));
}

/// Esc backs out spending nothing, matching every other second pick.
#[test]
fn esc_backs_out_of_the_cell_picker_spending_nothing() {
    let mut app = app_underground_with_routines(93, &["wild_jump"]);
    let before = app.game.as_ref().unwrap().stack_pos_xy();
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Right);
    app.handle_key(GameKey::Esc);

    assert_eq!(app.mode, Mode::FieldCast, "Esc steps back one screen");
    assert_eq!(app.pending_field_routine, None);
    assert_eq!(app.field_cursor, None);
    assert_eq!(
        app.game.as_ref().unwrap().stack_pos_xy(),
        before,
        "backing out of the picker moved the party"
    );
}

/// Enter commits the jump the cursor is on. Aimed at the party's own cell,
/// which is walkable and reachable by definition, so this asserts the
/// *plumbing* rather than any of the engine's landing rules.
#[test]
fn enter_commits_the_jump_the_cursor_is_aimed_at() {
    let mut app = app_underground_with_routines(94, &["wild_jump"]);
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));
    app.handle_key(GameKey::Enter);

    assert_eq!(
        app.status_line, None,
        "the jump was refused with: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::Playing);
    assert_eq!(app.field_cursor, None);
    assert_eq!(app.pending_field_routine, None);
}

/// On open grid both movement routines are greyed with the reason the
/// engine wrote, and picking one says so rather than casting.
#[test]
fn the_movement_routines_are_greyed_on_the_surface() {
    let mut app = app_with_player_routines(95, &["wild_jump"], 100.0);
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(
        app.mode,
        Mode::FieldCast,
        "it must not open the cell picker"
    );
    assert_eq!(
        app.status_line.as_deref(),
        Some("Can't run Wild Jump Party — only in the Stack.")
    );
}
