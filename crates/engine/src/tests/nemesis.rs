//! `components::Nemesis` — the trigger only. `Game::mark_nemeses` is called
//! from exactly one place, the window in `Game::end_battle` between the
//! `StackSpawn` stray sweep and `BattleState`'s removal. Task 1 ships no
//! reader for the mark beyond the raw component, so these assert directly
//! on `world.get::<Nemesis>(e)` rather than through any public API.

use super::support::*;
use crate::tuning::MAX_NEMESES;
use crate::*;

/// A fight the player cannot win outright and cannot survive either: a
/// `construct` scaled so far past the player's own stats that one round
/// both fails to kill it and flatlines the player. Mirrors
/// `combat::a_round_that_kills_the_player_ends_the_battle`'s fixture, minus
/// the `Permadeath` gate that test needs and a nemesis loss doesn't — a
/// Forgiving reboot is what lets the same program be lost to twice in one
/// test.
fn spawn_overwhelming_wild(game: &mut Game) -> Entity {
    let wild = game
        .spawn_wild_creature("construct", 5, 5)
        .expect("construct ships with the game");
    let mut stats = game.world.get_mut::<Stats>(wild).unwrap();
    stats.hp = 100_000;
    stats.max_hp = 100_000;
    stats.atk = 100_000;
    wild
}

/// Every entity in `game` currently carrying `Nemesis` — the ledger the cap
/// counts against, and what "marks nobody" means when there's no single
/// surviving entity left to assert `None` on (a won fight despawns its only
/// hostile).
fn nemesis_holders(game: &mut Game) -> Vec<Entity> {
    game.world
        .query_filtered::<Entity, With<Nemesis>>()
        .iter(&game.world)
        .collect()
}

#[test]
fn a_won_fight_marks_nobody() {
    let mut game = Game::new(40, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 1;
    game.start_battle(vec![wild]);

    player_attacks(&mut game);

    assert!(!game.has_active_battle(), "the kill should end the fight");
    assert!(
        nemesis_holders(&mut game).is_empty(),
        "an emptied group has nothing left to mark"
    );
}

/// Like `support::flee_until_clear`, but reports which of its two possible
/// endings actually happened, rather than leaving a caller to assume one.
/// `flee_until_clear` returns on *either* a landed escape or a failed
/// attempt's counter-volley ending the fight in defeat first — both leave a
/// surviving hostile marked at grudge 1, so asserting only that the battle
/// is over afterward can't tell a jack-out from a Forgiving loss. Reporting
/// which one happened lets the test below assert on it directly: if the
/// defeat path is ever what actually ends the fight, that assertion fails
/// loudly instead of the test passing for the wrong reason.
///
/// A failed attempt is not free of risk here — `battle::compute_damage`
/// floors every landed hit at `tuning::MIN_DAMAGE`, so even this fixture's
/// zero-`atk` wild program (`spawn_wild_on_player_tile`) still deals 1
/// damage per hit; "deals no damage" would be the wrong reason to trust
/// this loop. What actually keeps 200 straight failures vanishingly
/// unlikely is `battle::jack_out_chance`'s own floor, `JACK_OUT_CHANCE_MIN`
/// — the same bound `support::flee_until_clear`'s doc argues from.
fn flee_until_it_lands(game: &mut Game) -> bool {
    for _ in 0..200 {
        if game.battle_flee() {
            return true;
        }
        if !game.has_active_battle() {
            return false;
        }
    }
    panic!("200 jack-out attempts all failed — the escape roll is broken");
}

#[test]
fn a_successful_jack_out_marks_the_surviving_hostile_at_grudge_1() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);

    assert!(
        flee_until_it_lands(&mut game),
        "the escape roll never landed in 200 tries — jack_out_chance floors \
         well above zero, so this means the roll itself is broken, not that \
         a legitimate defeat occurred"
    );

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(1),
        "the program the party bailed out on should be marked"
    );
}

#[test]
fn a_forgiving_defeat_marks_the_surviving_hostile_at_grudge_1() {
    let mut game = Game::new(42, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_overwhelming_wild(&mut game);
    game.start_battle(vec![wild]);

    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();

    assert!(
        !game.has_active_battle(),
        "the round should have flatlined the player and ended the fight"
    );
    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(1),
        "the program that beat the player should be marked"
    );
}

#[test]
fn a_second_loss_to_the_same_program_escalates_grudge_rather_than_re_marking() {
    let mut game = Game::new(43, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_overwhelming_wild(&mut game);

    game.start_battle(vec![wild]);
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();
    assert_eq!(game.world.get::<Nemesis>(wild).map(|n| n.0), Some(1));

    game.start_battle(vec![wild]);
    game.battle_set_action(0, BattleAction::Attack { group: 0 })
        .unwrap();
    game.battle_resolve_round();

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(2),
        "a second loss should raise the grudge count, not insert a second mark"
    );
    assert_eq!(
        nemesis_holders(&mut game),
        vec![wild],
        "only the one program should ever be marked here"
    );
}

#[test]
fn a_stack_fight_marks_nobody_because_the_strays_are_swept_first() {
    let mut game = Game::new(44, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    // What `rouse_lair` tags a Stack pack's members with — see
    // `game/stack.rs` around its battle setup. Tagging a normally spawned
    // pack directly exercises the same despawn path without walking a maze
    // to reach a real lair fight, which the task brief allows as a
    // stand-in.
    game.world.entity_mut(wild).insert(StackSpawn);

    flee_until_clear(&mut game);

    assert!(
        game.world.get_entity(wild).is_err(),
        "a StackSpawn stray should have been despawned on the way out"
    );
    assert!(
        nemesis_holders(&mut game).is_empty(),
        "nothing should be marked once the only survivor was swept first"
    );
}

#[test]
fn an_eleventh_distinct_program_is_not_marked() {
    let mut game = Game::new(45, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for _ in 0..MAX_NEMESES {
        game.world.spawn((Nemesis(1),));
    }
    let wild = start_battle_with_a_wild_program(&mut game);

    flee_until_clear(&mut game);

    assert!(
        game.world.get::<Nemesis>(wild).is_none(),
        "the cap should refuse a fresh mark once it's full"
    );
    assert_eq!(
        nemesis_holders(&mut game).len(),
        MAX_NEMESES,
        "the cap should not have grown"
    );
}

#[test]
fn an_already_marked_nemesis_still_escalates_while_the_cap_is_full() {
    let mut game = Game::new(46, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    game.world.entity_mut(wild).insert(Nemesis(1));
    for _ in 1..MAX_NEMESES {
        game.world.spawn((Nemesis(1),));
    }
    assert_eq!(nemesis_holders(&mut game).len(), MAX_NEMESES);

    flee_until_clear(&mut game);

    assert_eq!(
        game.world.get::<Nemesis>(wild).map(|n| n.0),
        Some(2),
        "an existing nemesis should escalate even with no room for a new one"
    );
}
