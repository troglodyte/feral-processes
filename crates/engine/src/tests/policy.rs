//! The enemy battle policy: loading the weight asset, and the behaviour the
//! loaded weights buy.
//!
//! The arithmetic itself is tested in `policy.rs`'s own `mod tests` — this
//! module is about the parts that need a `Game`.

use super::support::*;
use crate::policy;
use crate::resources::EnemyPolicy;
use crate::*;

#[test]
fn an_absent_policy_file_loads_as_none_without_warning() {
    let dir = scratch_assets_dir("policy_absent");
    let (weights, warnings) = policy::load_file(&dir.join("enemy_battle.ron")).unwrap();
    assert!(weights.is_none());
    assert!(warnings.is_empty(), "an absent file is a valid state");
}

#[test]
fn a_malformed_policy_file_is_skipped_with_a_warning() {
    let dir = scratch_assets_dir("policy_malformed");
    std::fs::create_dir_all(&*dir).unwrap();
    let path = dir.join("enemy_battle.ron");
    std::fs::write(&path, "(features: [this is not ron").unwrap();

    let (weights, warnings) = policy::load_file(&path).unwrap();
    assert!(weights.is_none());
    assert_eq!(warnings.len(), 1, "{warnings:?}");
}

#[test]
fn a_game_starts_with_no_policy_file() {
    let game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(game.world.resource::<EnemyPolicy>().0.is_none());
}

// ─────────────────────────────────────────────────────────────────────────
// The decision seam.
// ─────────────────────────────────────────────────────────────────────────

fn weights(pairs: &[(&str, f32)]) -> policy::PolicyWeights {
    let owned: Vec<(String, f32)> = pairs.iter().map(|(n, v)| ((*n).into(), *v)).collect();
    let (w, warnings) = policy::PolicyWeights::from_pairs(&owned).unwrap();
    assert!(warnings.is_empty(), "test weights must name real features");
    w
}

fn install(game: &mut Game, w: policy::PolicyWeights) {
    game.world.insert_resource(EnemyPolicy(Some(w)));
}

/// A wild `scrapper` in a battle against the player and `companions` extra
/// party members, each spawned with the given `(hp, max_hp)`.
///
/// `scrapper` on purpose: it is one of the three shipped species with no
/// `ranged` move, which is what `a_back_group_still_only_uses_ranged_moves`
/// needs, and reusing it everywhere keeps one moveset in play across these
/// tests rather than whichever species `read_dir` happened to yield first.
fn battle_against_scrapper(seed: u32, companions: &[(i32, i32)]) -> (Game, Entity, Vec<Entity>) {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let members: Vec<Entity> = companions
        .iter()
        .map(|(hp, max_hp)| {
            let e = spawn_tamed(&mut game, *max_hp, 3);
            game.add_companion(e).unwrap();
            game.world.get_mut::<Stats>(e).unwrap().hp = *hp;
            e
        })
        .collect();

    let pos = *game.world.get::<Position>(player).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", pos.x, pos.y);
    insert_battle(&mut game, player, vec![wild]);
    (game, wild, members)
}

/// How often each entity is picked as the target over `draws` decisions,
/// with the group fixed at the front so reach never enters into it.
fn target_counts(
    game: &mut Game,
    wild: Entity,
    draws: usize,
) -> std::collections::HashMap<Entity, usize> {
    let player = game.player_entity();
    let mut counts = std::collections::HashMap::new();
    for _ in 0..draws {
        let (_, target) = game
            .choose_wild_action(wild, 0, player)
            .expect("a front-group scrapper always has a move that reaches");
        *counts.entry(target).or_insert(0) += 1;
    }
    counts
}

/// Plays a whole fight out on All-Attack and returns every line logged,
/// which is the finest-grained deterministic record of what happened —
/// each strike names its move, its target and its damage.
fn transcript(game: &mut Game) -> Vec<String> {
    game.world
        .resource_mut::<MessageLog>()
        .keep_battle_narration = true;
    for _ in 0..40 {
        if game.world.get_resource::<BattleState>().is_none() {
            break;
        }
        player_attacks(game);
    }
    game.world
        .resource::<MessageLog>()
        .lines
        .iter()
        .map(|l| l.text.clone())
        .collect()
}

/// The equivalence the `ln(slot_aggro_weight(..))` prior buys: installing
/// this feature untrained cannot move the game.
///
/// **Distributions, not transcripts.** The plan asked for an identical
/// battle log, which is a stronger claim than the design makes and than the
/// design wants: a trained policy scores `(move, target)` jointly and takes
/// *one* draw where the baseline takes two, so the RNG streams diverge on
/// the first swing however identical the odds are. What the spec actually
/// claims is that an all-zero vector reproduces today's *distribution*
/// exactly, and that is what is measured here.
///
/// The party is deliberately five deep so two members sit past
/// `FRONT_SLOTS`. Drop the prior and this fails loudly rather than subtly:
/// the front slots go from 3/11 each to 1/5 and the back two from 1/11 to
/// 1/5, which is the whole reason the prior is not a learned feature.
#[test]
fn an_all_zero_policy_reproduces_the_uniform_baseline() {
    const DRAWS: usize = 20_000;
    let party = [(60, 60), (60, 60), (60, 60), (60, 60)];

    let share = |zeroed: bool| {
        let (mut game, wild, members) = battle_against_scrapper(77, &party);
        let player = game.player_entity();
        if zeroed {
            install(&mut game, policy::PolicyWeights::default());
        }
        let mut counts = std::collections::HashMap::new();
        for _ in 0..DRAWS {
            let (mv, target) = game
                .choose_wild_action(wild, 0, player)
                .expect("a front-group scrapper always has a move that reaches");
            *counts.entry((mv.name, target)).or_insert(0usize) += 1;
        }
        let everyone: Vec<Entity> = std::iter::once(player).chain(members).collect();
        let mut out = Vec::new();
        for name in ["Ram", "Shock"] {
            for e in &everyone {
                let key = (name.to_string(), *e);
                out.push(*counts.get(&key).unwrap_or(&0) as f32 / DRAWS as f32);
            }
        }
        out
    };

    let baseline = share(false);
    let zeroed = share(true);
    for (i, (b, z)) in baseline.iter().zip(&zeroed).enumerate() {
        assert!(
            (b - z).abs() < 0.02,
            "pair {i}: baseline {b:.3} vs all-zero policy {z:.3} over {DRAWS} draws\n\
             baseline {baseline:?}\nzeroed   {zeroed:?}"
        );
    }
}

#[test]
fn a_policy_that_prefers_the_wounded_focus_fires() {
    let (mut game, wild, members) = battle_against_scrapper(5, &[(6, 60), (60, 60)]);
    let (wounded, healthy) = (members[0], members[1]);
    install(&mut game, weights(&[("target_hp_frac", -3.0)]));

    const DRAWS: usize = 2_000;
    let counts = target_counts(&mut game, wild, DRAWS);
    let share = |e| *counts.get(&e).unwrap_or(&0) as f32 / DRAWS as f32;

    // All three sit in the front slots, so the aggro prior alone would give
    // a third each. Anything well past that came from the weight.
    assert!(
        share(wounded) > 0.6,
        "the wounded companion took {:.2} of {DRAWS} swings, the healthy one {:.2}",
        share(wounded),
        share(healthy)
    );
}

/// The learned term has to be able to *overcome* the prior, not merely
/// nudge it — a back-slot target draws a third of the fire a front one
/// does, and a kill has to be worth more than that.
#[test]
fn a_policy_that_prefers_a_kill_takes_it() {
    let (mut game, wild, members) =
        battle_against_scrapper(9, &[(60, 60), (60, 60), (60, 60), (1, 60)]);
    let nearly_dead = members[3];
    install(&mut game, weights(&[("would_kill", 5.0)]));

    const DRAWS: usize = 2_000;
    let counts = target_counts(&mut game, wild, DRAWS);
    let share = *counts.get(&nearly_dead).unwrap_or(&0) as f32 / DRAWS as f32;

    assert!(
        share > 0.7,
        "the finishable back-slot target took only {share:.2} of {DRAWS} swings"
    );
}

/// Temperature is the shipping dial-back, so it has to actually reach all
/// the way back to the baseline. Driven through the private
/// `choose_wild_action_at` because `ENEMY_POLICY_TEMPERATURE` is a `const`
/// and the constant is the thing under test.
#[test]
fn a_high_temperature_approaches_the_uniform_baseline() {
    let opinionated = weights(&[
        ("target_hp_frac", -3.0),
        ("would_kill", 5.0),
        ("move_power_rel", 2.0),
    ]);
    let (mut game, wild, members) = battle_against_scrapper(21, &[(6, 60), (60, 60)]);
    let player = game.player_entity();
    install(&mut game, opinionated);

    const DRAWS: usize = 4_000;
    let mut counts = std::collections::HashMap::new();
    for _ in 0..DRAWS {
        let (_, target) = game
            .choose_wild_action_at(wild, 0, player, 500.0)
            .expect("a front-group scrapper always has a move that reaches");
        *counts.entry(target).or_insert(0) += 1;
    }

    // Player and both companions are front slots: a third each.
    for e in [player, members[0], members[1]] {
        let share = *counts.get(&e).unwrap_or(&0) as f32 / DRAWS as f32;
        assert!(
            (share - 1.0 / 3.0).abs() < 0.05,
            "at a high temperature {e} took {share:.2}, not roughly a third"
        );
    }
}

#[test]
fn the_same_seed_and_weights_replay_the_same_fight() {
    let trained = weights(&[
        ("target_hp_frac", -1.8),
        ("would_kill", 2.3),
        ("est_damage_frac", 1.0),
        ("move_effect_stun", 0.7),
    ]);
    let run = |w: policy::PolicyWeights| {
        let (mut game, _, _) = battle_against_scrapper(404, &[(60, 60), (40, 40)]);
        install(&mut game, w);
        transcript(&mut game)
    };

    assert_eq!(run(trained.clone()), run(trained));
}

/// The `ENGAGED_GROUPS` reach filter has to survive the rewrite: a scrapper
/// has no `ranged` move, so from a back group there is nothing to score and
/// nothing to swing.
#[test]
fn a_back_group_still_only_uses_ranged_moves() {
    let (mut game, wild, _) = battle_against_scrapper(3, &[]);
    let player = game.player_entity();
    install(&mut game, weights(&[("move_power_rel", 2.0)]));

    assert!(
        game.choose_wild_action(wild, tuning::ENGAGED_GROUPS, player)
            .is_none(),
        "a melee-only species in a back group reaches nothing"
    );

    game.wild_retaliate(wild, tuning::ENGAGED_GROUPS, player);
    let last = game
        .world
        .resource::<MessageLog>()
        .lines
        .last()
        .unwrap()
        .text
        .clone();
    assert!(
        last.contains("circles beyond reach"),
        "the existing idle line should still be what happens: {last}"
    );
}
