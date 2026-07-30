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
        buffs.iter().any(|b| b.name == "Trace Analysis"),
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
        buffs.iter().any(|b| b.name == "Hardened Shell"),
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
fn the_ally_picker_never_offers_an_unowned_entity() {
    let mut app = app_with_owned_and_wild_neighbors(76, &["hardened_shell"]);
    app.handle_key(GameKey::Char('a'));
    app.handle_key(GameKey::Char('1'));
    assert_eq!(app.mode, Mode::FieldCastAlly);

    // "You" and the one owned program — the wild neighbor must not be a
    // third row.
    let offered = app.field_ally_options();
    assert_eq!(offered.len(), 2);

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

    // A key past the end of the (owned-only) list does nothing — proof the
    // picker itself has no third, unowned row to select.
    app.handle_key(GameKey::Char('3'));
    assert_eq!(app.mode, Mode::FieldCastAlly);

    // The second row is the owned program, and casting on it works — proof
    // the picker's own rows are still live, not merely non-empty.
    app.handle_key(GameKey::Char('2'));
    assert_eq!(
        app.status_line, None,
        "cast was refused with: {:?}",
        app.status_line
    );
    assert_eq!(app.mode, Mode::Playing);
}
