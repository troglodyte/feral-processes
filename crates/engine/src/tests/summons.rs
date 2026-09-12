//! Forked programs: what a summon is, and what it is not.
//!
//! The containment story is a list of omissions — no `Tamed`, no
//! `Experience`, no `ProgramId` — so most of what these tests assert is a
//! component's *absence*. The one thing that is a check rather than an
//! omission is the teardown sweep, and it is the assertion that matters
//! most: a body that outlives its fight is standing in the zone.

use super::support::*;
use crate::components::{Experience, PowerReserve, Stats, Summoned, Tamed};
use crate::*;

fn game(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

fn fork_one(game: &mut Game) -> Entity {
    let player = game.player_entity();
    let bodies = game.fork_programs(player, 1);
    assert_eq!(bodies.len(), 1, "one body was asked for");
    bodies[0]
}

#[test]
fn a_forked_body_carries_a_marker_and_a_reserve_and_nothing_of_the_roster() {
    let mut game = game(7);
    let body = fork_one(&mut game);

    assert!(
        game.world.get::<Summoned>(body).is_some(),
        "a fork is marked as one"
    );
    assert!(
        game.world.get::<PowerReserve>(body).is_some(),
        "without a reserve it could never run the moves it was spawned to run"
    );
    assert!(
        game.world.get::<Tamed>(body).is_none(),
        "a fork never passes through roster_parts"
    );
    assert!(
        game.world.get::<Experience>(body).is_none(),
        "no Experience is what makes award_companion_xp skip it with no exclusion"
    );
    assert!(
        game.world.get::<components::Hostile>(body).is_none(),
        "it fights on your side"
    );
    assert!(
        game.world.get::<components::WanderAi>(body).is_none(),
        "it is not loose in the zone"
    );
}

#[test]
fn forking_does_not_change_the_roster_count() {
    let mut game = game(11);
    let before = game.pet_count();
    fork_one(&mut game);
    assert_eq!(
        game.pet_count(),
        before,
        "pet_count tallies Tamed under the player, and a fork is neither"
    );
}

#[test]
fn a_forked_body_has_no_program_role() {
    let mut game = game(13);
    let body = fork_one(&mut game);
    assert_eq!(
        game.program_role(body),
        None,
        "program_role reads Tamed first, so a fork is outside the four roles"
    );
}

#[test]
fn no_forked_body_outlives_the_fight_it_was_made_for() {
    let mut game = game(17);
    let player = game.player_entity();
    let pos = *game.world.get::<Position>(player).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", pos.x, pos.y);
    insert_battle(&mut game, player, vec![wild]);

    let body = fork_one(&mut game);
    assert!(game.world.get::<Stats>(body).is_some(), "it exists first");

    game.end_battle(player, Some(wild));

    assert!(
        game.world.get::<Stats>(body).is_none(),
        "finish_fight sweeps every Summoned body, living or not"
    );
}
