//! One rep: play a staged fight out on its own.

use super::PartyPlan;
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

/// Below this fraction of max HP, a `PartyPlan::BraceWhenHurt` member
/// Defends rather than attacking.
///
/// A fixed fraction rather than "would this kill me", which is the rule a
/// person actually plays: estimating incoming damage means a second copy of
/// the damage formula living in the tester, and `CLAUDE.md`'s rule about a
/// copy that drifts applies with force to the one nobody runs in the game.
/// Half health is the round number that produces bracing in the fights long
/// enough for Defend to matter and none of the ones that are over in three
/// rounds.
const BRACE_BELOW_HP_FRACTION: f32 = 0.5;

/// Plays the staged fight in `game` out to its end.
///
/// **`PartyPlan::AllAttack` is the game's own plan** —
/// `battle_plan_remaining` then `battle_resolve_round`, which is what
/// `App::plan_every_slot` does when the player presses `[A]`. On that path
/// the arena invents no decision the game would not make, which is the
/// guarantee every arena number published before 2026-08-10 rests on, and
/// `the_default_party_plan_never_braces` is what holds it now that a second
/// plan exists.
///
/// `BraceWhenHurt` sets Defend for the hurt slots and then falls through to
/// the *same* `battle_plan_remaining` for everyone else. Expressing only the
/// departures rather than planning every slot is deliberate: there stays one
/// mop-up path, and a slot the rule has no opinion about cannot accidentally
/// be given one.
///
/// `game` is consumed for one fight and must not be reused: it comes out
/// carrying this fight's dead companions, spent items and XP.
pub(crate) fn run_rep(game: &mut Game, watch: &mut Watch, plan: PartyPlan) -> RepRecord {
    while watch.rounds() < ROUND_CAP {
        // Not the player's HP: a Forgiving defeat is rebooted inside the
        // round that lands it, so by the time this could look the player is
        // alive again. `is_game_over` is the Permadeath half — a save
        // scenario can carry that mode in, and `battle_resolve_round`
        // returns early forever once it is set.
        if !game.has_active_battle() || game.is_game_over().is_some() {
            break;
        }
        for slot in bracing_slots(game, plan) {
            // Same reasoning as the `Err` below: a slot that has just
            // stopped being able to act is the fight moving on, not a bug.
            // `battle_plan_remaining` will skip it too.
            let _ = game.battle_set_action(slot, BattleAction::Defend);
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

/// The party slots that brace this round under `plan`, in slot order.
///
/// One function for all three plans so the "which slots depart from
/// All-Attack" question has a single answer; `AllAttack` returning empty is
/// what makes the caller unconditional.
///
/// Reads `Stats` and `BattleState` off the world directly, the way `Watch`
/// does — the arena lives inside the engine, so there is no reason to route
/// this through a view built for a renderer. A slot that cannot act is
/// skipped here rather than refused later, so the mop-up plan sees exactly
/// the slots this rule declined to touch.
fn bracing_slots(game: &Game, plan: PartyPlan) -> Vec<usize> {
    let Some(battle) = game.world.get_resource::<BattleState>() else {
        return Vec::new();
    };
    let slots = battle.planned.len();
    if slots == 0 {
        return Vec::new();
    }
    let round = battle.round as usize;
    let candidates: Vec<usize> = match plan {
        PartyPlan::AllAttack => return Vec::new(),
        PartyPlan::BraceWhenHurt => (0..slots)
            .filter(|&slot| {
                game.actor_entity(battle::Actor::Party(slot))
                    .and_then(|e| game.world.get::<Stats>(e))
                    .is_some_and(|s| {
                        s.max_hp > 0 && (s.hp as f32 / s.max_hp as f32) < BRACE_BELOW_HP_FRACTION
                    })
            })
            .collect(),
        // By round rather than by anything about the member, which is the
        // entire point: health and slot position both carry their own
        // signal, so a rule keyed on either is not measuring Defend.
        PartyPlan::BraceInRotation => vec![round % slots],
    };
    candidates
        .into_iter()
        .filter(|&slot| game.slot_can_act(slot))
        .collect()
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
                    ..Default::default()
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
