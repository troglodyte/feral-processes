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
    let bodies = game.fork_programs(player, 1, 0);
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

/// The pool a fork draws its species from.
mod non_boss_pool {
    use super::*;
    use crate::species::SpeciesDb;

    fn db(game: &Game) -> &SpeciesDb {
        game.world.resource::<SpeciesDb>()
    }

    #[test]
    fn the_pool_holds_every_ordinary_species_and_no_apex_one() {
        let game = game(21);
        let db = db(&game);
        let ids = db.non_boss_ids();
        assert!(!ids.is_empty(), "the shipped catalogue is not empty");
        for id in &ids {
            assert!(
                !db.get(id).expect("an id the pool returned").is_boss,
                "{id} is apex and has no business being forked"
            );
        }
        let apex = db.all().filter(|s| s.is_boss).count();
        assert_eq!(
            ids.len(),
            db.all().count() - apex,
            "every non-apex species is in the pool"
        );
    }

    #[test]
    fn the_pool_is_sorted_and_stable() {
        let game = game(23);
        let first = db(&game).non_boss_ids();
        let second = db(&game).non_boss_ids();
        assert!(
            first.is_sorted(),
            "an unsorted pool is an RNG-stream shift between runs"
        );
        assert_eq!(first, second);
    }

    /// The one that would actually catch an unsorted pool. `is_sorted` alone
    /// passes against a `HashMap` that happens to iterate in order on this
    /// run; two `Game`s on one seed forking different species does not.
    #[test]
    fn one_seed_forks_one_species() {
        let mut a = game(29);
        let mut b = game(29);
        let a_species: Vec<String> = {
            let player = a.player_entity();
            a.fork_programs(player, 4, 0)
                .into_iter()
                .map(|e| a.world.get::<Creature>(e).unwrap().species.clone())
                .collect()
        };
        let b_species: Vec<String> = {
            let player = b.player_entity();
            b.fork_programs(player, 4, 0)
                .into_iter()
                .map(|e| b.world.get::<Creature>(e).unwrap().species.clone())
                .collect()
        };
        assert_eq!(a_species, b_species);
        assert!(
            a_species
                .iter()
                .collect::<std::collections::HashSet<_>>()
                .len()
                > 1,
            "four draws off a flat pool should not all land on one species: {a_species:?}"
        );
    }
}

/// What a fork is worth: the tier it rolls, the ceiling that holds it, and
/// the player's own level reaching it.
mod strength {
    use super::*;
    use crate::components::{Perks, Rarity};
    use crate::perks::Perk;

    fn scheduled(game: &mut Game, rank: usize) {
        let player = game.player_entity();
        game.world.entity_mut(player).insert(Perks {
            points: 0,
            unlocked: vec![Perk::Scheduler; rank],
        });
    }

    /// `count` forks rolled one at a time, each swept before the next, so
    /// the sample is of the roll and not of one invocation's worth of them.
    fn tiers(game: &mut Game, count: usize, penalty: u32) -> Vec<Rarity> {
        let player = game.player_entity();
        let mut seen = Vec::new();
        for _ in 0..count {
            for body in game.fork_programs(player, 1, penalty) {
                seen.push(game.world.get::<Rarity>(body).copied().unwrap_or_default());
                game.world.despawn(body);
            }
        }
        seen
    }

    /// A single fork passes against a broken window by luck; two hundred
    /// do not.
    #[test]
    fn an_unscheduled_fork_is_always_ordinary() {
        let mut game = game(31);
        scheduled(&mut game, 0);
        let rolled = tiers(&mut game, 200, 0);
        assert!(
            rolled.iter().all(|&r| r == Rarity::Ordinary),
            "the window is shut at rank 0: {rolled:?}"
        );
    }

    /// Both halves matter — the first proves the window opened, the second
    /// proves the ceiling holds it.
    #[test]
    fn one_rank_reaches_silver_and_stops_there() {
        let mut game = game(37);
        scheduled(&mut game, 1);
        let rolled = tiers(&mut game, 200, 0);
        assert!(
            rolled.contains(&Rarity::Silver),
            "a bought rank should be visible over 200 rolls: {rolled:?}"
        );
        assert!(
            rolled.iter().all(|&r| r <= Rarity::Silver),
            "rank 1's ceiling is Silver: {rolled:?}"
        );
    }

    #[test]
    fn a_penalty_lowers_the_ceiling_a_rung() {
        let mut game = game(41);
        scheduled(&mut game, 2);
        let plain = tiers(&mut game, 200, 0);
        assert!(
            plain.contains(&Rarity::Gold),
            "rank 2 unpenalised reaches Gold: {plain:?}"
        );

        let mut game = super::game(41);
        scheduled(&mut game, 2);
        let penalised = tiers(&mut game, 200, 1);
        assert!(
            penalised.iter().all(|&r| r <= Rarity::Silver),
            "a rarity_penalty of 1 is a rung off the ceiling: {penalised:?}"
        );
        assert!(
            penalised.contains(&Rarity::Silver),
            "and only off the ceiling — the window is still open: {penalised:?}"
        );
    }

    /// Driven by setting the level rather than by fighting: the term under
    /// test is `party_band_progress`, which reads `Experience::level`.
    #[test]
    fn a_fork_grows_with_the_players_own_level() {
        let hp_at = |level: u32| {
            let mut game = game(43);
            let player = game.player_entity();
            set_level(&mut game, player, level);
            let body = game.fork_programs(player, 1, 0)[0];
            game.world.get::<Stats>(body).unwrap().max_hp
        };
        assert!(
            hp_at(8) > hp_at(1),
            "the level term is live: {} at 1, {} at 8",
            hp_at(1),
            hp_at(8)
        );
    }
}

/// Seating a fork in the group model: `Party` and `planned` together, and
/// the three silent failures on either side of that.
mod group_model {
    use super::*;
    use crate::battle::{BattleAction, SpecialTarget};
    use crate::components::Summoned;
    use crate::resources::Party;

    fn log_texts(game: &Game) -> Vec<String> {
        game.message_log(crate::MESSAGE_LOG_CAP)
            .into_iter()
            .map(|l| l.text)
            .collect()
    }

    /// A fight with one hostile and one fork already seated, returning the
    /// hostile and the fork.
    fn fight_with_a_fork(game: &mut Game) -> (Entity, Entity) {
        let player = game.player_entity();
        let pos = *game.world.get::<Position>(player).unwrap();
        let wild = spawn_wild_without_routine(game, "scrapper", pos.x, pos.y);
        // Enough Integrity that the fight does not end before anyone acts.
        if let Some(mut stats) = game.world.get_mut::<Stats>(wild) {
            stats.max_hp = 4000;
            stats.hp = 4000;
        }
        insert_battle(game, player, vec![wild]);
        let body = game.fork_programs(player, 1, 0)[0];
        game.seat_summon_in_group(body);
        (wild, body)
    }

    /// Asserting only that it exists passes against the inert bug this seam
    /// is about — a body pushed to `Party` alone draws an initiative rung
    /// and then never acts, and nothing fails.
    #[test]
    fn a_seated_fork_draws_a_row_and_takes_a_turn() {
        let mut game = game(101);
        let (wild, body) = fight_with_a_fork(&mut game);

        let view = game.battle_view().expect("a fight is running");
        assert_eq!(
            view.party.len(),
            game.world.resource::<Party>().0.len() + 1,
            "battle_rows iterates planned, so a fork missing from it is invisible"
        );

        let before = game.world.get::<Stats>(wild).unwrap().hp;
        // The player braces, so anything that lands on the hostile this
        // round was the fork's doing.
        resolve_round_with(&mut game, BattleAction::Defend);
        let after = game.world.get::<Stats>(wild).unwrap().hp;
        assert!(
            after < before,
            "the fork should have acted: {before} -> {after}"
        );
        assert!(
            game.world.get::<Stats>(body).is_some(),
            "and still be standing"
        );
    }

    /// The other half of acting: a fork is a body the rest of the fight can
    /// see, on both sides.
    #[test]
    fn a_seated_fork_is_a_body_both_sides_can_reach() {
        let mut game = game(103);
        let (_, body) = fight_with_a_fork(&mut game);
        assert!(
            game.living_party().contains(&body),
            "living_party iterates planned"
        );
        let slot = game
            .party_slot_of(body)
            .expect("a seated fork occupies a party slot");
        assert!(
            game.ability_recipients(
                game.player_entity(),
                crate::abilities::AbilityTarget::WholeParty,
                &SpecialTarget::WholeParty
            )
            .contains(&body),
            "a WholeParty heal covers it; it is in slot {slot}"
        );
    }

    /// Re-invoking replaces the standing set. A replaced body is *killed*,
    /// never removed — removal shifts every slot behind it, which is the
    /// thing `BattleState::planned` indexing `Party` positionally forbids.
    #[test]
    fn re_invoking_replaces_the_standing_set_without_shrinking_the_party() {
        let mut game = game(107);
        let (_, first) = fight_with_a_fork(&mut game);
        let len_before = game.world.resource::<Party>().0.len();

        game.dissolve_summons();
        assert!(
            !game.creature_alive(first),
            "the replaced body is dead where it stands"
        );
        assert_eq!(
            game.world.resource::<Party>().0.len(),
            len_before,
            "nothing leaves Party mid-battle"
        );
        assert!(
            game.world.resource::<Party>().0.contains(&first),
            "and it is still referenced by the slot it held"
        );

        let player = game.player_entity();
        let second = game.fork_programs(player, 1, 0)[0];
        game.seat_summon_in_group(second);
        assert_eq!(game.world.resource::<Party>().0.len(), len_before + 1);
    }

    /// `bench_or_dissolve` never asks whether the body it is handed is
    /// `Tamed` — it detaches, benches or dissolves whatever it gets. Run on
    /// a fork it announces a downed program the player never had, and
    /// `detach_from_play` writes "leaves your battle party" about a body
    /// that was never theirs to lose.
    #[test]
    fn a_dead_fork_is_not_benched_as_a_downed_program() {
        let mut game = game(109);
        let (wild, body) = fight_with_a_fork(&mut game);
        let name = game.creature_label(body);
        if let Some(mut stats) = game.world.get_mut::<Stats>(body) {
            stats.hp = 0;
        }

        game.end_battle(game.player_entity(), Some(wild));

        let mut queued = Vec::new();
        while let Some(note) = game.take_notification() {
            queued.push(note.title);
        }
        assert!(
            !queued.iter().any(|t| t == "Program Down"),
            "a new game has no Repair Bay, so an unskipped bench would announce one: {queued:?}"
        );
        let leaving = format!("{name} leaves your battle party.");
        assert!(
            !log_texts(&game).contains(&leaving),
            "a fork ending is not a companion being lost: {:#?}",
            log_texts(&game)
        );
    }

    /// The Permadeath half of the same skip: `dissolve_tamed_program`
    /// announces the same detachment and then despawns the body out from
    /// under the `Party` slot still naming it.
    #[test]
    fn a_dead_fork_is_not_dissolved_as_a_tamed_program() {
        let mut game = Game::new(111, DifficultyMode::Permadeath, &test_assets_dir()).unwrap();
        let (wild, body) = fight_with_a_fork(&mut game);
        let name = game.creature_label(body);
        if let Some(mut stats) = game.world.get_mut::<Stats>(body) {
            stats.hp = 0;
        }

        game.end_battle(game.player_entity(), Some(wild));

        let leaving = format!("{name} leaves your battle party.");
        assert!(
            !log_texts(&game).contains(&leaving),
            "a fork is dissolved by the sweep, not detached as a companion: {:#?}",
            log_texts(&game)
        );
    }

    /// `finish_hostile` keys the payout on the victim rather than the
    /// killer, so this should pass with no new code — and a fork with no
    /// `Experience` is already invisible to `award_companion_xp`.
    #[test]
    fn a_fork_earns_nothing_and_its_kills_still_pay_the_player() {
        let mut game = game(113);
        let player = game.player_entity();
        let pos = *game.world.get::<Position>(player).unwrap();
        let wild = spawn_wild_without_routine(&mut game, "scrapper", pos.x, pos.y);
        if let Some(mut stats) = game.world.get_mut::<Stats>(wild) {
            stats.max_hp = 1;
            stats.hp = 1;
        }
        insert_battle(&mut game, player, vec![wild]);
        let body = game.fork_programs(player, 1, 0)[0];
        game.seat_summon_in_group(body);
        assert!(
            game.world
                .get::<crate::components::Experience>(body)
                .is_none(),
            "nothing to award to"
        );

        let xp_before = game
            .world
            .get::<crate::components::Experience>(player)
            .map(|e| e.xp)
            .unwrap_or(0);
        // The player braces, so the kill is the fork's.
        resolve_round_with(&mut game, BattleAction::Defend);
        assert!(!game.creature_alive(wild), "the fork finished it");
        let xp_after = game
            .world
            .get::<crate::components::Experience>(player)
            .map(|e| e.xp)
            .unwrap_or(0);
        assert!(
            xp_after > xp_before
                || game
                    .world
                    .get::<crate::components::Experience>(player)
                    .map(|e| e.level)
                    .unwrap_or(1)
                    > 1,
            "a kill pays the player whoever landed it: {xp_before} -> {xp_after}"
        );
    }

    /// The engine plans a fork's turn, so the player is never asked for one.
    #[test]
    fn a_fork_is_never_a_slot_the_player_is_asked_to_command() {
        let mut game = game(117);
        let (_, body) = fight_with_a_fork(&mut game);
        let slot = game.party_slot_of(body).expect("it holds a slot");
        assert_ne!(game.battle_active_slot(), Some(slot));
        let _ = game.battle_set_action(0, BattleAction::Defend);
        assert!(
            game.battle_round_ready(),
            "with the player's own slot planned the round is ready"
        );
        assert!(
            game.world.get::<Summoned>(body).is_some(),
            "and the fork is still the one that was not asked"
        );
    }
}
