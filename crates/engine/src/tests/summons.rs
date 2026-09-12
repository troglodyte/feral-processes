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

/// `retier_rarity` and `promote_rarity` are one formula with two doors, so
/// the tests that matter are the ones that pin them to each other.
mod retier {
    use super::*;
    use crate::components::Rarity;

    fn stats_of(game: &Game, body: Entity) -> Stats {
        *game.world.get::<Stats>(body).unwrap()
    }

    /// A body at a known tier with hand-set stats: every case here measures
    /// a *move* between tiers, and a wild spawn's own stats and tier are
    /// both rolled.
    fn a_body(game: &mut Game) -> Entity {
        let player = game.player_entity();
        let pos = *game.world.get::<Position>(player).unwrap();
        let body = spawn_wild_without_routine(game, "scrapper", pos.x, pos.y);
        game.world.entity_mut(body).insert((
            Rarity::Ordinary,
            Stats {
                hp: 40,
                max_hp: 40,
                atk: 12,
                mitigation: 10,
            },
        ));
        body
    }

    #[test]
    fn retiering_two_rungs_matches_two_promotions() {
        let mut game = game(3);
        let stepped = a_body(&mut game);
        game.promote_rarity(stepped);
        game.promote_rarity(stepped);

        let jumped = a_body(&mut game);
        game.retier_rarity(jumped, Rarity::Gold);

        assert_eq!(
            game.world.get::<Rarity>(jumped).copied(),
            Some(Rarity::Gold)
        );
        let a = stats_of(&game, stepped);
        let b = stats_of(&game, jumped);
        assert_eq!(
            (a.max_hp, a.atk, a.mitigation),
            (b.max_hp, b.atk, b.mitigation)
        );
    }

    #[test]
    fn retiering_down_undoes_retiering_up() {
        let mut game = game(5);
        let body = a_body(&mut game);
        let before = stats_of(&game, body);

        game.retier_rarity(body, Rarity::Gold);
        game.retier_rarity(body, Rarity::Ordinary);

        let after = stats_of(&game, body);
        // Within a point per stat: the step is applied by multiply-and-round
        // each way, so the round trip is exact only where the products land
        // on whole numbers.
        assert!(
            (after.max_hp - before.max_hp).abs() <= 1,
            "{before:?} -> {after:?}"
        );
        assert!(
            (after.atk - before.atk).abs() <= 1,
            "{before:?} -> {after:?}"
        );
        assert!(
            (after.mitigation - before.mitigation).abs() <= 1,
            "{before:?} -> {after:?}"
        );
        assert_eq!(
            game.world.get::<Rarity>(body).copied(),
            Some(Rarity::Ordinary)
        );
    }

    #[test]
    fn promote_rarity_still_moves_one_rung_and_stops_at_the_top() {
        let mut game = game(9);
        let body = a_body(&mut game);
        assert_eq!(game.promote_rarity(body), Rarity::Silver);
        assert_eq!(game.promote_rarity(body), Rarity::Gold);

        game.world.entity_mut(body).insert(Rarity::Prismatic);
        assert_eq!(
            game.promote_rarity(body),
            Rarity::Prismatic,
            "Rarity::ALL's own top is the ceiling"
        );
    }
}
