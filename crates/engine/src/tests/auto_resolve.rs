//! `Game::auto_resolve_battle`: playing whichever combat model is open out
//! to its end with no pacing, the way `[R]` will ask for.

use super::support::test_assets_dir;
use super::tactical::tactical_fight;
use crate::*;

/// A weak wild program dies to the player's first swing, so the whole loop
/// is one round: `Finished`, and nothing left open behind it.
#[test]
fn a_winnable_group_fight_resolves_and_closes() {
    let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    {
        let mut w = game.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 1;
        w.max_hp = 1;
    }
    game.start_battle(vec![wild]);

    assert_eq!(game.auto_resolve_battle(), AutoResolve::Finished);
    assert!(
        !game.has_active_battle(),
        "a won fight must not leave a battle open behind it"
    );
}

/// The tactical counterpart, and it must not wait for the player's own
/// turn first — the door works from whoever is holding the initiative
/// order when it is called, same as it would mid-fight from `[R]`.
#[test]
fn a_winnable_tactical_fight_resolves_and_closes() {
    let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pack = tactical_fight(&mut game, 1, 1);
    {
        let mut w = game.world.get_mut::<Stats>(pack[0]).unwrap();
        w.hp = 1;
        w.max_hp = 1;
    }

    assert_eq!(game.auto_resolve_battle(), AutoResolve::Finished);
    assert!(
        !game.has_active_battle(),
        "a won tactical fight must not leave a battle open behind it"
    );
}

/// The loss path has to stop the loop rather than spin it: a Permadeath
/// flatline ends the fight and sets `is_game_over`, and
/// `battle_resolve_round` (through `battle_auto_round`) does nothing
/// forever once that is set. This test returning at all is the "doesn't
/// spin" assertion — copies `tests::combat::a_round_that_kills_the_player_
/// ends_the_battle`'s setup, Permadeath rather than Forgiving because a
/// Forgiving flatline soft-reboots the player within the same tick.
#[test]
fn a_permadeath_loss_stops_with_the_game_over_set() {
    let mut game = Game::new(96, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("construct", 5, 5).unwrap();
    {
        let mut w = game.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 100_000;
        w.max_hp = 100_000;
        w.atk = 100_000;
    }
    game.start_battle(vec![wild]);

    assert_eq!(game.auto_resolve_battle(), AutoResolve::Finished);
    assert!(
        game.is_game_over().is_some(),
        "the setup should have flatlined the player outright"
    );
}

/// A fight nobody involved can end: both sides' HP dwarfs anything either
/// can deal in `AUTO_RESOLVE_ROUND_CAP` rounds (the player's own swing is
/// `PLAYER_UNARMED_DAMAGE`, floored well under 3,000 total across the cap),
/// and the wild side deals none at all. The cap has to be what stops the
/// loop, with the fight left open for the player to finish by hand.
#[test]
fn a_fight_nobody_can_end_stalls_at_the_cap_with_the_fight_open() {
    let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 10_000_000;
    game.world.get_mut::<Stats>(player).unwrap().max_hp = 10_000_000;
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    {
        let mut w = game.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 10_000_000;
        w.max_hp = 10_000_000;
        w.atk = 0;
    }
    game.start_battle(vec![wild]);
    let start = game.fight_round().expect("a fight just opened");

    assert_eq!(game.auto_resolve_battle(), AutoResolve::Stalled);
    assert!(
        game.has_active_battle(),
        "a stall must leave the fight open for the player to finish by hand"
    );
    assert_eq!(
        game.fight_round().expect("the stalled fight is still open") - start,
        tuning::AUTO_RESOLVE_ROUND_CAP,
        "the cap is what should have stopped the loop, not an early exit"
    );
}

/// The same stall, played out on a battle map instead of the group model.
#[test]
fn a_fight_nobody_can_end_stalls_at_the_cap_on_a_battle_map() {
    let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 10_000_000;
    game.world.get_mut::<Stats>(player).unwrap().max_hp = 10_000_000;
    let pack = tactical_fight(&mut game, 1, 10_000_000);
    {
        let mut w = game.world.get_mut::<Stats>(pack[0]).unwrap();
        w.max_hp = 10_000_000;
        w.atk = 0;
    }
    let start = game.fight_round().expect("a fight just opened");

    assert_eq!(game.auto_resolve_battle(), AutoResolve::Stalled);
    assert!(
        game.has_active_battle(),
        "a stall must leave the fight open for the player to finish by hand"
    );
    assert_eq!(
        game.fight_round().expect("the stalled fight is still open") - start,
        tuning::AUTO_RESOLVE_ROUND_CAP,
        "the cap is what should have stopped the loop, not an early exit"
    );
}
