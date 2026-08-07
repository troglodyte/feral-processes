//! One rep: reseed, fight, keep what was said.

use rand::SeedableRng;
use rand::rngs::StdRng;

use super::report::RepRecord;
use crate::battle::{BattleAction, EnemyGroup};
use crate::resources::GameRng;
use crate::*;

/// A fight that has not resolved in this many rounds is a stalemate, and is
/// recorded as one rather than hanging the tool. It catches nothing else:
/// the longest real fight the game can produce is orders of magnitude
/// shorter. Matches `balance_sim`'s `TURN_CAP`, which exists for the same
/// reason.
const ROUND_CAP: u32 = 2000;

/// Runs `groups` against the party in `game`, at `seed`.
///
/// The party plays the game's own All-Attack — `battle_plan_remaining`
/// followed by `battle_resolve_round`, which is what `App::plan_every_slot`
/// does when the player presses `[A]`. Not a policy engine written for the
/// tester, so the arena cannot drift from the game by inventing decisions
/// the game never makes.
///
/// `game` is consumed for one fight and must not be reused: it comes out
/// carrying this fight's dead companions, spent items and XP.
pub(crate) fn run_rep(game: &mut Game, groups: Vec<EnemyGroup>, seed: u64) -> RepRecord {
    // Per rep, not per run: twenty reps are then a sample rather than
    // twenty copies, and any one of them replays alone from its own seed.
    game.world
        .insert_resource(GameRng(StdRng::seed_from_u64(seed)));

    let player = game.player_entity();
    let party: Vec<Entity> = game.world.resource::<Party>().0.clone();
    // Won means the pack is wiped, so the opponents are what has to be
    // counted afterwards — and they have to be noted now, because
    // `BattleState` is gone by the time the answer is wanted.
    let opponents: Vec<Entity> = groups.iter().flat_map(|g| g.members.clone()).collect();
    game.begin_battle(groups);

    let mut transcript = Vec::new();
    let mut rounds = 0;
    while rounds < ROUND_CAP {
        // Not the player's HP: a Forgiving defeat is rebooted inside the
        // round that lands it, so by the time this could look the player is
        // alive again. `is_game_over` is the Permadeath half — a save
        // scenario can carry that mode in, and `battle_resolve_round`
        // returns early forever once it is set.
        if !game.has_active_battle() || game.is_game_over().is_some() {
            break;
        }
        // An `Err` here means the battle ended between the check above and
        // this call — the fight being over, not a bug to panic on.
        if game
            .battle_plan_remaining(BattleAction::Attack { group: 0 })
            .is_err()
        {
            break;
        }
        if !game.battle_round_ready() {
            break;
        }
        game.battle_resolve_round();
        rounds += 1;
        // After every round, never at the end: `end_battle` calls
        // `retain_outcomes_since_battle`, which deletes the blow-by-blow and
        // keeps only Outcome/Loot/LevelUp/Raid. `MESSAGE_LOG_CAP` is the
        // second reason — a long fight drops lines off the front before it
        // finishes.
        transcript.extend(game.battle_log().into_iter().map(|line| line.text));
    }

    // A stalemate leaves the pack standing, so it records as a loss with
    // `rounds == ROUND_CAP` — which is what says which it was.
    let won = opponents.iter().all(|&e| !alive(game, e));
    let (hp, max_hp) = game
        .world
        .get::<Stats>(player)
        .map(|s| (s.hp, s.max_hp))
        .unwrap_or((0, 1));
    RepRecord {
        seed,
        won,
        rounds,
        // Zero on a loss, matching `balance_sim`, and not merely a
        // convention: a Forgiving defeat reboots the player to a fraction of
        // max HP inside the losing round, so the number standing here
        // afterwards measures the reboot rather than the fight.
        player_hp_fraction: if won {
            (hp as f32 / max_hp.max(1) as f32).clamp(0.0, 1.0)
        } else {
            0.0
        },
        companions_downed: party.iter().filter(|&&e| !alive(game, e)).count() as u32,
        transcript,
    }
}

fn alive(game: &Game, entity: Entity) -> bool {
    game.world.get::<Stats>(entity).is_some_and(|s| s.hp > 0)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::scenario::{CompanionSpec, OpponentSpec, PlayerSource, Scenario};
    use crate::arena::setup::{build_opponents, build_player};
    use crate::tests::support::test_assets_dir;

    fn scenario(level: u32, zone: u32, party: &[(&str, u32)], against: &[(&str, u32)]) -> Scenario {
        Scenario {
            player: PlayerSource::Fresh { level, zone },
            party: party
                .iter()
                .map(|(species, level)| CompanionSpec {
                    species: (*species).into(),
                    level: *level,
                })
                .collect(),
            opponents: against
                .iter()
                .map(|(species, count)| OpponentSpec {
                    species: (*species).into(),
                    count: *count,
                })
                .collect(),
            ..Scenario::default()
        }
    }

    fn fight(s: &Scenario, seed: u64) -> RepRecord {
        let mut game = build_player(s, &test_assets_dir()).unwrap();
        let (groups, _) = build_opponents(&mut game, &s.opponents).unwrap();
        run_rep(&mut game, groups, seed)
    }

    #[test]
    fn the_same_seed_replays_the_same_fight() {
        let s = scenario(6, 2, &[("glitch", 4)], &[("sub_process", 3)]);
        assert_eq!(fight(&s, 12), fight(&s, 12));
    }

    #[test]
    fn two_seeds_diverge() {
        let s = scenario(6, 2, &[("glitch", 4)], &[("sub_process", 3)]);
        assert_ne!(
            fight(&s, 1).transcript,
            fight(&s, 999).transcript,
            "the per-rep reseed should be doing something"
        );
    }

    /// The regression this whole module exists for. `end_battle` prunes the
    /// log to Outcome/Loot/LevelUp/Raid, so an implementation that read the
    /// log once the fight was over passes every other test here and returns
    /// nothing but results.
    #[test]
    fn a_won_fights_transcript_survives_end_battle() {
        let s = scenario(20, 1, &[("glitch", 8)], &[("sprite", 1)]);
        let record = fight(&s, 3);
        assert!(
            record.won,
            "the fixture is meant to be a walkover: {record:?}"
        );
        assert!(
            record
                .transcript
                .iter()
                .any(|line| line.contains("── round 1 ──")),
            "round narration is dropped by `retain_outcomes_since_battle`: {:?}",
            record.transcript
        );
    }

    #[test]
    fn an_overwhelming_party_wins_in_a_few_rounds() {
        let record = fight(&scenario(20, 1, &[("glitch", 8)], &[("sprite", 1)]), 5);
        assert!(record.won);
        assert!(record.rounds > 0);
        assert!(record.rounds < 50, "{} rounds", record.rounds);
    }

    #[test]
    fn a_bare_level_one_player_does_not_beat_a_full_group_of_the_toughest_program() {
        // Named through `balance_sim` rather than spelled out, so a roster
        // retune cannot quietly turn this fixture into a win.
        let probe = build_player(&scenario(1, 1, &[], &[]), &test_assets_dir()).unwrap();
        let toughest =
            crate::balance_sim::toughest_ordinary_species(probe.world.resource::<SpeciesDb>())
                .id
                .clone();
        let record = fight(&scenario(1, 1, &[], &[(&toughest, 8)]), 7);
        assert!(!record.won, "{record:?}");
        assert_eq!(record.player_hp_fraction, 0.0);
    }
}
