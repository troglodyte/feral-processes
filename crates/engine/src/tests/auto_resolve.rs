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

/// A stalled resolve owes the hook its rounds too — the `if` that calls it
/// does not special-case `Stalled` against `Finished`, so every round the
/// cap actually spent must reach it, exactly `AUTO_RESOLVE_ROUND_CAP` of
/// them. `App::auto_resolve` installs the same hook whichever `AutoResolve`
/// comes back (see app-core's `battle.rs`), so an arena session's `Watch`
/// stalling with the wrong round count would be this call site lying, and
/// there is no door onto a live `Stats` write from app-core's own tests to
/// build that fixture there — see the comment in `tests::arena` where one
/// was tried.
#[test]
fn a_stalled_resolve_calls_the_hook_for_every_round_it_fought() {
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

    let mut calls = 0u32;
    let outcome = game.auto_resolve_battle_with(|_| calls += 1);

    assert_eq!(outcome, AutoResolve::Stalled);
    assert!(game.has_active_battle());
    assert_eq!(
        calls,
        tuning::AUTO_RESOLVE_ROUND_CAP,
        "a stall must feed the hook every round the cap actually spent, not none of them"
    );
}

/// `App::auto_resolve` feeds an arena's `Watch` through `auto_resolve_
/// battle_with`'s hook — it has to see one call per round actually fought,
/// the same count `arena::run`'s own loop produces driving `battle_auto_
/// round` by hand, never one call for the whole fight (`I1`'s bug: a
/// `Watch` observed once after the loop read a seven-round fight as one
/// round). The seeded fight and the hand-driven one are the same fight —
/// same seed, same calls in the same order — so an equal count here is not
/// a coincidence a looser assertion could paper over.
#[test]
fn the_hook_fires_once_a_round_not_once_for_the_whole_fight() {
    let mut hooked = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = hooked.spawn_wild_creature("glitch", 5, 5).unwrap();
    {
        let mut w = hooked.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 50;
        w.max_hp = 50;
        w.atk = 0;
    }
    hooked.start_battle(vec![wild]);
    let mut calls = 0u32;
    let outcome = hooked.auto_resolve_battle_with(|_| calls += 1);
    assert_eq!(outcome, AutoResolve::Finished);
    assert!(!hooked.has_active_battle());

    // The same fight, driven by hand one round at a time — `run_rep`'s own
    // loop, without the hook, as the ground truth for how many rounds it
    // actually took.
    let mut by_hand = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = by_hand.spawn_wild_creature("glitch", 5, 5).unwrap();
    {
        let mut w = by_hand.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 50;
        w.max_hp = 50;
        w.atk = 0;
    }
    by_hand.start_battle(vec![wild]);
    let mut rounds = 0u32;
    while by_hand.battle_auto_round() {
        rounds += 1;
    }

    assert!(
        rounds > 1,
        "need a multi-round fight to tell counted-once from counted-right: {rounds}"
    );
    assert_eq!(
        calls, rounds,
        "the hook must fire once per round actually fought, not once for the whole fight"
    );
}

/// The tactical counterpart: `tactical_drive_turn` returns `true` once a
/// *turn*, not once a round, so a hook gated on the step's own return would
/// fire many times too often. Ground-truthed the same way, against `run_
/// tactical_rep`'s own round-change rule driven by hand.
#[test]
fn the_hook_fires_once_a_round_on_a_battle_map_too() {
    let mut hooked = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pack = tactical_fight(&mut hooked, 1, 50);
    {
        let mut w = hooked.world.get_mut::<Stats>(pack[0]).unwrap();
        w.atk = 0;
    }
    let mut calls = 0u32;
    let outcome = hooked.auto_resolve_battle_with(|_| calls += 1);
    assert_eq!(outcome, AutoResolve::Finished);
    assert!(!hooked.has_active_battle());

    let mut by_hand = Game::new(4, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pack = tactical_fight(&mut by_hand, 1, 50);
    {
        let mut w = by_hand.world.get_mut::<Stats>(pack[0]).unwrap();
        w.atk = 0;
    }
    let mut rounds = 0u32;
    loop {
        if !by_hand.has_active_battle() || by_hand.is_game_over().is_some() {
            break;
        }
        let round = by_hand.fight_round().unwrap_or(0);
        if !by_hand.tactical_drive_turn() {
            break;
        }
        if !by_hand.has_active_battle() || by_hand.fight_round().unwrap_or(0) != round {
            rounds += 1;
        }
    }

    assert!(
        rounds > 1,
        "need a multi-round fight to tell counted-once from counted-right: {rounds}"
    );
    assert_eq!(
        calls, rounds,
        "the hook must fire once per round actually fought, not once per turn"
    );
}

/// `Ruling 1`'s insurance, made testable by `M2`: a `GameOver` left behind
/// with a battle still open must not spin the loop. `battle_resolve_round`
/// no-ops forever once it is set, so before `battle_auto_round` reported
/// whether a round actually advanced, this state relied entirely on the
/// loop's own `is_game_over` stop check — which this test exercises, and
/// which a regression that removed `is_game_over` from *both* of this
/// function's checks would have turned into a hang rather than a clean
/// `Stalled`. (Verified by hand: with `battle_auto_round` reverted to
/// always returning `true`, deleting `is_game_over` from both checks spins
/// forever; with the fix in place, the same deletion answers `Stalled`
/// instead — this test is what would have caught that either way.)
#[test]
fn a_game_over_left_inside_an_open_battle_does_not_spin_the_loop() {
    let mut game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    game.start_battle(vec![wild]);
    game.world.resource_mut::<resources::GameOver>().reason = Some("test".to_string());

    assert_eq!(game.auto_resolve_battle(), AutoResolve::Finished);
    assert!(
        game.has_active_battle(),
        "is_game_over ends the loop, not the fight — the battle must still be open behind it"
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
