//! `GameKey`'s spelled form — what `--keys` on the launcher reads.

use crate::GameKey;

#[test]
fn a_named_key_parses_to_its_variant() {
    assert_eq!("Right".parse(), Ok(GameKey::Right));
    assert_eq!("ShiftLeft".parse(), Ok(GameKey::ShiftLeft));
    assert_eq!("Esc".parse(), Ok(GameKey::Esc));
}

#[test]
fn a_single_character_is_that_character() {
    assert_eq!("M".parse(), Ok(GameKey::Char('M')));
    assert_eq!(",".parse(), Ok(GameKey::Char(',')));
}

/// Tokens are split on whitespace, so the space bar needs a name.
#[test]
fn space_is_spelled_out() {
    assert_eq!("Space".parse(), Ok(GameKey::Char(' ')));
}

#[test]
fn an_unknown_name_is_refused_rather_than_dropped() {
    assert!("Rigth".parse::<GameKey>().is_err());
    assert!("Char".parse::<GameKey>().is_err());
}

#[test]
fn a_key_list_splits_on_whitespace() {
    assert_eq!(
        GameKey::parse_list("Right  Right\tM Enter"),
        Ok(vec![
            GameKey::Right,
            GameKey::Right,
            GameKey::Char('M'),
            GameKey::Enter
        ])
    );
    assert!(GameKey::parse_list("Right Nope").is_err());
}

/// **The census.** `GameKey::NAMED` is what the parser reads, so a variant
/// missing from it is a key `--keys` cannot press. The exhaustive match is
/// what fails to compile when a variant is added; bump the count with it.
#[test]
fn every_named_variant_is_in_the_table() {
    const NAMED_VARIANTS: usize = 16;
    let _exhaustive = |k: GameKey| match k {
        GameKey::Char(_) => (),
        GameKey::Up
        | GameKey::Down
        | GameKey::Left
        | GameKey::Right
        | GameKey::ShiftLeft
        | GameKey::ShiftRight
        | GameKey::CtrlLeft
        | GameKey::CtrlRight
        | GameKey::UpLeft
        | GameKey::UpRight
        | GameKey::DownLeft
        | GameKey::DownRight
        | GameKey::Enter
        | GameKey::Esc
        | GameKey::Backspace
        | GameKey::Tab => (),
    };
    assert_eq!(GameKey::NAMED.len(), NAMED_VARIANTS);
    for (i, key) in GameKey::NAMED.into_iter().enumerate() {
        assert!(!GameKey::NAMED[..i].contains(&key), "{key:?} listed twice");
        assert!(!matches!(key, GameKey::Char(_)));
        assert_eq!(format!("{key:?}").parse(), Ok(key));
    }
}
