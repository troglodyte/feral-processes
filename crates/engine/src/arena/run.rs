//! One rep: play a staged fight out on its own.

use super::report::RepRecord;
use super::watch::Watch;
use crate::battle::BattleAction;
use crate::*;

/// A fight that has not resolved in this many rounds is a stalemate, and is
/// recorded as one rather than hanging the tool. It catches nothing else:
/// the longest real fight the game can produce is orders of magnitude
/// shorter. Matches `balance_sim`'s `TURN_CAP`, which exists for the same
/// reason.
const ROUND_CAP: u32 = 2000;

/// Plays the staged fight in `game` out to its end, auto-attacking.
///
/// The party plays the game's own All-Attack — `battle_plan_remaining`
/// followed by `battle_resolve_round`, which is what `App::plan_every_slot`
/// does when the player presses `[A]`. Not a policy engine written for the
/// tester, so the arena cannot drift from the game by inventing decisions
/// the game never makes.
///
/// `game` is consumed for one fight and must not be reused: it comes out
/// carrying this fight's dead companions, spent items and XP.
pub(crate) fn run_rep(game: &mut Game, watch: &mut Watch) -> RepRecord {
    while watch.rounds() < ROUND_CAP {
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
        watch.observe(game);
    }

    // A stalemate leaves the pack standing, so `finish` records it as a loss
    // with `rounds == ROUND_CAP` — which is what says which it was.
    watch.finish(game)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::arena::scenario::{CompanionSpec, OpponentSpec, PlayerSource, Scenario};
    use crate::arena::setup::build_player;
    use crate::arena::test_fight as fight;
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
