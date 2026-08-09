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

/// The shipped weights are wired all the way through: on disk, past
/// `load_asset_dbs`, into the resource `choose_wild_action` reads.
///
/// This replaces `a_game_starts_with_no_policy_file`, which asserted the
/// opposite and was correct right up until a trained file existed. Deleting
/// the file is still a supported way to play — `a_malformed_policy_file_is_
/// skipped_with_a_warning` and `an_absent_policy_file_loads_as_none_without_
/// warning` are what cover that, and they use scratch paths rather than the
/// real assets dir precisely so they keep meaning what they say.
#[test]
fn the_shipped_policy_file_loads_into_the_game() {
    let game = Game::new(1, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let policy = &game.world.resource::<EnemyPolicy>().0;
    let weights = policy
        .as_ref()
        .expect("assets/policies/enemy_battle.ron ships with the game");
    assert!(
        weights.to_pairs().iter().any(|(_, v)| *v != 0.0),
        "an all-zero shipped file is the baseline wearing a costume"
    );
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
        // Set explicitly in *both* arms. `test_assets_dir()` is the real
        // `assets/`, so once trained weights ship the "baseline" arm would
        // otherwise be the shipped policy measured against an all-zero one
        // — a different question, which this test would then answer while
        // still reading as the equivalence guard it is named for.
        game.world
            .insert_resource(EnemyPolicy(zeroed.then(policy::PolicyWeights::default)));
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

// ─────────────────────────────────────────────────────────────────────────
// Moddability, and the Defend guard.
// ─────────────────────────────────────────────────────────────────────────

/// The moddability claim walked rather than assumed: one weight vector
/// scores a species with a moveset no shipped one has. Seven moves is more
/// than any shipped species carries, so nothing about the scoring pass can
/// be quietly sized to the real assets.
#[test]
fn a_modded_species_with_more_moves_still_scores() {
    let body = r#"(
    id: "sevenfold",
    name: "Sevenfold",
    glyph: 's',
    color: Magenta,
    base_hp: 90,
    base_atk: 11,
    base_def: 4,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [
        (name: "One", power: 3),
        (name: "Two", power: 6, ranged: true),
        (name: "Three", power: 9, effect: Some((kind: Stun, chance: 0.3, duration: 1, power: 0))),
        (name: "Four", power: 12, effect: Some((kind: Bleed, chance: 0.4, duration: 2, power: 3))),
        (name: "Five", power: 15, ranged: true),
        (name: "Six", power: 18),
        (name: "Seven", power: 21, ranged: true),
    ],
    work_resource: None,
)"#;
    let dir = modded_assets_dir(
        "policy_seven",
        &[],
        &[],
        &[("sevenfold.ron", body)],
        &[],
        &[],
    );
    let mut game = Game::new(31, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 40, 3);
    game.add_companion(companion).unwrap();
    let pos = *game.world.get::<Position>(player).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "sevenfold", pos.x, pos.y);
    insert_battle(&mut game, player, vec![wild]);
    install(
        &mut game,
        weights(&[
            ("move_power_rel", 2.5),
            ("would_kill", 2.0),
            ("target_hp_frac", -1.0),
        ]),
    );

    let mut names = std::collections::HashSet::new();
    for _ in 0..600 {
        let (mv, target) = game
            .choose_wild_action(wild, 0, player)
            .expect("seven moves all reach from the front group");
        assert!(target == player || target == companion);
        names.insert(mv.name);
    }
    assert!(
        names.len() > 1,
        "a strong move_power_rel should still leave the weaker moves reachable, \
         not collapse to one: {names:?}"
    );
    assert!(
        names.contains("Seven"),
        "the biggest move should be among those chosen: {names:?}"
    );
}

/// The degenerate end of the same claim, and both zero-denominator guards
/// in one file: a species with a single move of power 0 and no Attack at
/// all. `move_power_rel` divides by the species' best power and
/// `target_def_rel` divides by the attacker's ATK, and both are zero here.
#[test]
fn a_modded_species_with_one_move_scores() {
    let body = r#"(
    id: "inert",
    name: "Inert",
    glyph: 'i',
    color: White,
    base_hp: 40,
    base_atk: 0,
    base_def: 0,
    taming_difficulty: 0.5,
    habitats: [OpenGrid],
    moves: [
        (name: "Nudge", power: 0),
    ],
    work_resource: None,
)"#;
    let dir = modded_assets_dir("policy_inert", &[], &[], &[("inert.ron", body)], &[], &[]);
    let mut game = Game::new(32, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 40, 3);
    game.add_companion(companion).unwrap();
    // DEF 0 against ATK 0 is the other zero denominator, and the two have
    // to be exercised together: `0 / 0` is the only combination that
    // produces a NaN rather than an infinity the clamp absorbs.
    game.world.get_mut::<Stats>(companion).unwrap().def = 0;
    let pos = *game.world.get::<Position>(player).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "inert", pos.x, pos.y);
    game.world.get_mut::<Stats>(wild).unwrap().atk = 0;
    insert_battle(&mut game, player, vec![wild]);
    install(
        &mut game,
        weights(&[("move_power_rel", 3.0), ("target_def_rel", -3.0)]),
    );

    // Two targets, and the assertion is that *both* get picked. A missing
    // guard does not panic — `0.0 / 0.0` is NaN, NaN poisons the score, and
    // `sample_scored` quietly falls back to argmax, which would hand every
    // single swing to the same entity forever. Asserting that one call
    // returns *something* would pass either way; this does not.
    let mut seen = std::collections::HashSet::new();
    for _ in 0..400 {
        let (mv, target) = game
            .choose_wild_action(wild, 0, player)
            .expect("one move is still a move");
        assert_eq!(mv.name, "Nudge");
        seen.insert(target);
    }
    assert_eq!(
        seen.len(),
        2,
        "both targets share a slot weight, so both must come up — one alone \
         means the scores went non-finite and sampling degenerated to argmax"
    );
}

/// The Defend census, run against **whatever weights are installed** rather
/// than against a hand-authored set. Bracing buys `DEFEND_AGGRO_WEIGHT` on
/// top of the slot weight, and that has to survive the learned term
/// multiplying it, or a designed player action quietly stops doing what its
/// description promises.
///
/// `balance_sim` cannot cover this: its own docs say it passes
/// `defending: false` throughout and models no Defend actions at all.
///
/// **This test has already done its job once, and the reason it exists is
/// worth keeping.** The first unconstrained training run learned
/// `target_bracing: -3.3` — avoid whoever braced — and the second, with
/// that feature pinned to zero, learned `target_def_rel: -7.3` and dodged
/// the brace *through the DEF it grants* instead. Bracing drew 10% of the
/// fire against 46% not bracing. Both routes had to be closed by pinning
/// before a policy could ship, and the trained file's own zeroes are the
/// record of it — see `docs/superpowers/reports/`.
///
/// The trap for a future retrain: any damage-maximising policy has a reason
/// to avoid the braced target, because reducing damage taken is what
/// bracing *is*. Defend's aggro bonus was tuned against an enemy that rolls
/// at random. A retrain that lets a DEF-reading feature back into the
/// search reopens this, and fails here rather than shipping.
#[test]
fn bracing_still_draws_more_fire_under_the_shipped_weights() {
    let share_of_first_companion = |bracing: bool| {
        let (mut game, wild, members) = battle_against_scrapper(64, &[(60, 60), (60, 60)]);
        if bracing {
            game.begin_defend(members[0]);
            assert!(game.is_defending(members[0]));
        }
        const DRAWS: usize = 4_000;
        let counts = target_counts(&mut game, wild, DRAWS);
        *counts.get(&members[0]).unwrap_or(&0) as f32 / DRAWS as f32
    };

    let braced = share_of_first_companion(true);
    let exposed = share_of_first_companion(false);
    assert!(
        braced > exposed + 0.02,
        "bracing drew {braced:.2} of the fire against {exposed:.2} not bracing — \
         Defend has to keep pulling aggro under the shipped policy"
    );
}

/// The shipped policy hardly ever picks an effect-carrying move, and this
/// records that as a measured fact rather than leaving it to surface as a
/// puzzling failure in the gate test above.
///
/// It is not a bug in the policy — it is the roster's own pricing read back
/// to us. Every shipped species prices its effect move *below* its
/// damage-only sibling (Worm: Replicate 5 with Bleed against Burrow Strike
/// 8; Cipher: Encrypt 6 with Stun against Cross-Reference 9), and
/// `WILD_ABILITY_CHANCE` then gates the effect down to roughly 6-10% of
/// swings. An expected-damage maximiser correctly never pays that. Only a
/// uniform roll ever did, which means the condition variety in wild fights
/// was a product of the enemy not thinking.
///
/// Two ways to make effect moves worth picking again, neither taken here
/// because both are balance changes to shipped content rather than to this
/// feature: price them at or above their sibling, or raise
/// `WILD_ABILITY_CHANCE` so the effect actually lands often enough to be
/// worth the lost damage. If either is done, this test is the one that
/// should start failing.
#[test]
fn a_trained_policy_rarely_picks_an_effect_carrying_move() {
    let mut effect_moves = 0;
    let mut total = 0;
    for seed in 0..300u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        let wild = start_battle_with_a_wild_program(&mut game);
        // A carrier spends its round on a routine and never reaches the
        // move choice, so it would silently shrink the sample.
        game.world.entity_mut(wild).insert(Routines::default());
        let species = game.world.get::<Creature>(wild).unwrap().species.clone();
        let has_effect_move = game
            .world
            .resource::<SpeciesDb>()
            .get(&species)
            .map(|s| s.moves.iter().any(|m| m.effect.is_some()))
            .unwrap_or(false);
        if !has_effect_move {
            continue;
        }
        let Some((mv, _)) = game.choose_wild_action(wild, 0, player) else {
            continue;
        };
        total += 1;
        if mv.effect.is_some() {
            effect_moves += 1;
        }
    }

    assert!(
        total > 50,
        "only {total} usable samples — the fixture stopped producing species with effect moves"
    );
    let rate = effect_moves as f32 / total as f32;
    assert!(
        rate < 0.15,
        "the shipped policy picked an effect move on {:.0}% of turns ({effect_moves} of \
         {total}). If effect moves have been repriced to be worth taking, that is good news \
         and this test is the one to update — read its doc comment first.",
        rate * 100.0
    );
}
