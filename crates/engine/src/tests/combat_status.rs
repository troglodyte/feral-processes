//! Status effects: how stun and bleed tick down, and when they clear.

use super::support::*;
use crate::tuning::WILD_ABILITY_CHANCE;
use crate::*;

#[test]
fn stunned_player_loses_their_turn_but_wild_still_retaliates_and_stun_clears() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Deliberately effect-free, so the wild creature's own retaliation
    // can't re-apply (and thus reset the clock on) the status this test
    // is tracking.
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
        .expect("at least one species with no status-effect moves should exist for this test");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 50,
                max_hp: 50,
                atk: 3,
                mitigation: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
        kind: StatusKind::Stun,
        remaining: 1,
        power: 0,
        landed_this_round: false,
    });

    let wild_hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    player_attacks(&mut game);
    let wild_hp_after = game.world.get::<Stats>(wild).unwrap().hp;

    assert_eq!(
        wild_hp_before, wild_hp_after,
        "a stunned player shouldn't deal any attack damage"
    );
    assert!(
        game.world
            .get::<StatusEffects>(player)
            .unwrap()
            .active
            .is_none(),
        "the stun should clear after its one round elapses"
    );
}

#[test]
fn bleed_status_deals_extra_damage_each_round_and_expires_after_its_duration() {
    let mut game = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Deliberately effect-free, so the wild creature's own retaliation
    // can't re-apply (and thus reset the clock on) the status this test
    // is tracking.
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
        .expect("at least one species with no status-effect moves should exist for this test");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 0,
                mitigation: 0,
            },
            StatusEffects {
                active: Some(ActiveStatus {
                    kind: StatusKind::Bleed,
                    remaining: 2,
                    power: 5,
                    landed_this_round: false,
                }),
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    // **The strike's own damage is measured, not predicted.** It rolls from
    // a band now, so the flat `power + atk` it used to be is no longer a
    // number this test can know in advance. What it *can* pin is the
    // difference the bleed makes: an identical forced swing against a clean
    // target on the same stream, subtracted off. Seeding both the same way
    // is what makes the two rolls identical, so the gap is the bleed alone.
    let clean_strike = {
        let mut control = Game::new(62, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let control_player = control.player_entity();
        let target = control
            .world
            .spawn((
                Creature {
                    species: species.id.clone(),
                },
                Hostile,
                Position { x: 5, y: 5 },
                Stats {
                    hp: 100,
                    max_hp: 100,
                    atk: 0,
                    mitigation: 0,
                },
                StatusEffects::default(),
            ))
            .id();
        insert_battle(&mut control, control_player, vec![target]);
        force_the_next_attack_to_land(&mut control);
        player_attacks(&mut control);
        100 - control.world.get::<Stats>(target).unwrap().hp
    };
    assert!(clean_strike > 0, "the control swing has to have landed");

    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    force_the_next_attack_to_land(&mut game);
    player_attacks(&mut game);
    let hp_after = game.world.get::<Stats>(wild).unwrap().hp;
    assert_eq!(
        hp_before - hp_after,
        clean_strike + 5,
        "wild should take its attack damage plus one round of bleed"
    );
    assert_eq!(
        game.world
            .get::<StatusEffects>(wild)
            .unwrap()
            .active
            .unwrap()
            .remaining,
        1
    );

    let hp_before2 = game.world.get::<Stats>(wild).unwrap().hp;
    force_the_next_attack_to_land(&mut game);
    player_attacks(&mut game);
    let hp_after2 = game.world.get::<Stats>(wild).unwrap().hp;
    assert_eq!(
        hp_before2 - hp_after2,
        clean_strike + 5,
        "the second bleed round should also tick"
    );
    assert!(
        game.world
            .get::<StatusEffects>(wild)
            .unwrap()
            .active
            .is_none(),
        "bleed should clear once its duration elapses"
    );
}

#[test]
fn status_effects_are_cleared_once_the_battle_ends() {
    let mut game = Game::new(63, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Deliberately effect-free, so the wild creature's own retaliation
    // can't re-apply (and thus reset the clock on) the status this test
    // is tracking.
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| !s.is_boss && s.moves.iter().all(|m| m.effect.is_none()))
        .expect("at least one species with no status-effect moves should exist for this test");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                mitigation: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
        kind: StatusKind::Bleed,
        remaining: 5,
        power: 1,
        landed_this_round: false,
    });

    // 1 HP wild creature dies to the player's first attack, ending the battle.
    player_attacks(&mut game);

    assert!(
        !game.has_active_battle(),
        "the wild creature's death should end the battle"
    );
    assert!(
        game.world
            .get::<StatusEffects>(player)
            .unwrap()
            .active
            .is_none(),
        "leftover status effects should be cleared once the battle ends, however it ends"
    );
}

/// The lines that are a battle's *results* have to be distinguishable from
/// the blow-by-blow, since only the results follow the player onto the map.
/// Loot and level-ups already carry their own kinds; the kill and the XP
/// award were plain `Info` and so were indistinguishable from narration.
#[test]
fn the_kill_line_and_xp_award_are_tagged_as_outcomes() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    start_battle_with_a_wild_program(&mut game);

    game.finish_member(0, 0, player);

    let tagged: Vec<String> = game
        .message_log(50)
        .into_iter()
        .filter(|e| e.kind == MessageKind::Outcome)
        .map(|e| e.text)
        .collect();

    assert!(
        tagged
            .iter()
            .any(|l| l.contains("crashes and deletes itself")),
        "the kill line was not tagged an outcome: {tagged:?}"
    );
    assert!(
        tagged.iter().any(|l| l.contains("XP")),
        "the XP award was not tagged an outcome: {tagged:?}"
    );
}

#[test]
fn the_battle_log_holds_only_the_current_battle() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.log("before the fight");
    start_battle_with_a_wild_program(&mut game);
    game.log("mid-battle narration");

    let battle: Vec<String> = game.battle_log().into_iter().map(|e| e.text).collect();

    assert!(
        battle.iter().any(|l| l == "mid-battle narration"),
        "the battle's own line is missing: {battle:?}"
    );
    assert!(
        !battle.iter().any(|l| l == "before the fight"),
        "a pre-battle line leaked into the battle log: {battle:?}"
    );
}

/// `MESSAGE_LOG_CAP` is 100, and the log drains its oldest lines past that.
/// A battle mark stored as a raw index into `lines` would be pointing at the
/// wrong entry by the time that happens — which is why it is a count of
/// lines ever pushed instead.
#[test]
fn the_mark_survives_a_log_that_overflows_its_cap() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    for i in 0..350 {
        game.log(format!("line {i}"));
    }

    let battle: Vec<String> = game.battle_log().into_iter().map(|e| e.text).collect();

    assert_eq!(
        battle.last().map(String::as_str),
        Some("line 349"),
        "the newest line is not last: {:?}",
        battle.last()
    );
    assert!(
        battle.len() <= 100,
        "the battle log outgrew the log it slices: {}",
        battle.len()
    );
}

#[test]
fn ending_a_battle_keeps_results_and_drops_narration() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    start_battle_with_a_wild_program(&mut game);
    game.log("A hostile swings and misses.");
    game.log_kind(MessageKind::Outcome, "You gain 12 XP.");
    game.log_kind(MessageKind::Raid, "A raid hits your base!");

    game.end_battle(player, None);

    let after: Vec<String> = game.message_log(100).into_iter().map(|e| e.text).collect();
    assert!(
        !after.iter().any(|l| l.contains("swings and misses")),
        "blow-by-blow survived the prune: {after:?}"
    );
    assert!(
        after.iter().any(|l| l.contains("You gain 12 XP")),
        "the result was pruned away: {after:?}"
    );
    assert!(
        after.iter().any(|l| l.contains("A raid hits your base")),
        "a raid alert is world news, not battle narration: {after:?}"
    );
}

#[test]
fn a_second_battle_starts_with_an_empty_pane() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    start_battle_with_a_wild_program(&mut game);
    let first = game.battle_log_generation();
    game.log("first battle narration");
    game.end_battle(player, None);

    start_battle_with_a_wild_program(&mut game);

    assert_ne!(
        first,
        game.battle_log_generation(),
        "the pane generation did not advance for the new battle"
    );
    let battle: Vec<String> = game.battle_log().into_iter().map(|e| e.text).collect();
    assert!(
        !battle.iter().any(|l| l == "first battle narration"),
        "the previous battle's narration is still in the pane: {battle:?}"
    );
}

/// Pruning removes lines from the middle of the log, which must not shift
/// where the battle mark points. The mark is a count of lines ever pushed
/// and the index is derived from how many have been dropped off the front —
/// so a prune that forgets to account for what it removed makes every later
/// slice reach back past the battle and swallow lines from before it.
#[test]
fn pruning_does_not_drag_the_battle_mark_backwards() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.log("well before the fight");
    game.log("also before the fight");
    start_battle_with_a_wild_program(&mut game);
    game.log("a hostile swings and misses");
    game.log("another miss");
    game.log_kind(MessageKind::Outcome, "You gain 12 XP.");

    game.end_battle(player, None);

    let battle: Vec<String> = game.battle_log().into_iter().map(|e| e.text).collect();
    assert!(
        !battle.iter().any(|l| l.contains("before the fight")),
        "the mark slid back past the battle after pruning: {battle:?}"
    );
    assert!(
        battle.iter().any(|l| l.contains("You gain 12 XP")),
        "the result went missing: {battle:?}"
    );
}

/// The pane shows one round at a time. Without this a six-round fight
/// leaves the player scanning a wall of text for the two lines that just
/// happened.
#[test]
fn resolving_a_round_clears_the_pane_of_the_previous_one() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    assert!(
        game.battle_log()
            .iter()
            .any(|e| e.text.contains("intercepts your signal")),
        "the opening line should be in the pane before any round resolves"
    );

    resolve_round_with(&mut game, BattleAction::Defend);

    let pane: Vec<String> = game.battle_log().into_iter().map(|e| e.text).collect();
    assert!(
        !pane.iter().any(|l| l.contains("intercepts your signal")),
        "the opening line survived into the next round: {pane:?}"
    );
    assert!(
        pane.iter().any(|l| l.contains("round")),
        "the round's own narration is missing: {pane:?}"
    );
}

/// Wild programs used to attempt their move's status effect every single
/// turn, so a species with a nasty stun was that stun on repeat. They now
/// reach for it only `WILD_ABILITY_CHANCE` of the time.
///
/// Samples many retaliations rather than asserting on one: the point is a
/// rate. Counts only the turns where the move actually used carries an
/// effect, so a damage-only move being picked is never mistaken for the
/// gate having fired.
///
/// **`EnemyPolicy` is cleared, and that is the test, not a convenience.**
/// The gate is a property of `wild_retaliate`, and measuring it needs the
/// move choice to be the uniform roll it was written against. Under the
/// shipped policy the sample collapses to *one* effect-move turn in 400,
/// which is too few to judge any rate at all — see
/// `a_trained_policy_rarely_picks_an_effect_carrying_move` below, which is
/// where that behaviour is asserted deliberately instead of showing up here
/// as an unexplained failure.
#[test]
fn wild_programs_only_sometimes_reach_for_their_status_effect() {
    let mut with_effect_move = 0;
    let mut landed = 0;
    for seed in 0..400u32 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        game.world
            .insert_resource(crate::resources::EnemyPolicy(None));
        let player = game.player_entity();
        let wild = start_battle_with_a_wild_program(&mut game);
        let species = game.world.get::<Creature>(wild).unwrap().species.clone();
        let moves = game
            .world
            .resource::<SpeciesDb>()
            .get(&species)
            .map(|s| s.moves.clone())
            .unwrap_or_default();

        let before = game.message_log(200).len();
        game.wild_retaliate(wild, 0, player);
        let after: Vec<String> = game
            .message_log(200)
            .into_iter()
            .skip(before)
            .map(|e| e.text)
            .collect();

        // Which move was used is only observable through the line naming it.
        let used_effect_move = moves
            .iter()
            .filter(|m| m.effect.is_some())
            .any(|m| after.iter().any(|l| l.contains(&m.name)));
        if !used_effect_move {
            continue;
        }
        with_effect_move += 1;
        if after
            .iter()
            .any(|l| l.contains("starts bleeding") || l.contains("locks up"))
        {
            landed += 1;
        }
    }

    assert!(
        with_effect_move > 30,
        "only {with_effect_move} turns used an effect-carrying move — too few to judge a rate"
    );
    // The gate composes with each move's own `effect.chance` (0.3-0.5 across
    // the shipped roster), so the landed rate sits *below* the gate itself
    // and can never exceed it.
    let rate = landed as f64 / with_effect_move as f64;
    assert!(
        rate < WILD_ABILITY_CHANCE,
        "status effects landed on {:.0}% of effect-move turns, above the {:.0}% gate \
         ({landed} of {with_effect_move})",
        rate * 100.0,
        WILD_ABILITY_CHANCE * 100.0
    );
    assert!(
        landed > 0,
        "no status effect ever landed — the gate is stuck shut"
    );
}

/// The discovery mechanism: you learn a program is a carrier by being hit
/// with what it carries.
#[test]
fn a_carrier_spends_its_round_on_its_routine_instead_of_a_move() {
    let mut game = Game::new(7701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world
        .entity_mut(enemies[0])
        .insert(Routines(vec!["hard_lock".to_string()]));
    game.world.get_mut::<Stats>(enemies[0]).unwrap().atk = 0;

    game.wild_retaliate(enemies[0], 0, player);

    assert!(
        matches!(
            game.world.get::<StatusEffects>(player).unwrap().active,
            Some(ActiveStatus {
                kind: StatusKind::Stun,
                ..
            })
        ),
        "Hard Lock stuns — a move could not have done this"
    );
}

#[test]
fn a_carriers_routine_goes_on_cooldown_and_comes_back() {
    let mut game = Game::new(7702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world
        .entity_mut(enemies[0])
        .insert(Routines(vec!["hard_lock".to_string()]));

    game.wild_retaliate(enemies[0], 0, player);
    assert!(
        game.wild_routine_ready(enemies[0]).is_none(),
        "it just fired — Hard Lock has a cooldown of 4"
    );

    for _ in 0..6 {
        game.tick_ability_cooldowns(enemies[0]);
    }
    assert!(
        game.wild_routine_ready(enemies[0]).is_some(),
        "and it comes back once the cooldown has ticked out"
    );
}

/// `cooldown` defaults to 0 and a carrier fires whenever it can, so a mod
/// ability declaring none must still not fire two rounds running.
///
/// Its predecessor stood `priority_boost` in for a cooldown-0 ability, but
/// this same branch bumped `priority_boost.ron` to `cooldown: 1` — so that
/// version passed for the wrong reason, and would have kept passing even
/// with `ENEMY_ROUTINE_MIN_COOLDOWN` deleted outright. This drops in a real
/// cooldown-0 ability instead, and asserts its premise before relying on it.
#[test]
fn a_cooldown_zero_routine_still_cannot_fire_two_rounds_running() {
    const ZERO_CD_ABILITY: &str = r#"(
        id: "test_zero_cd_strike",
        name: "Zero-CD Strike",
        description: "d",
        target: OneEnemyGroupFront,
        effect: Damage(power: 5),
    )"#;
    let dir = modded_assets_dir(
        "zero_cd_floor",
        &[],
        &[],
        &[],
        &[],
        &[("test_zero_cd_strike.ron", ZERO_CD_ABILITY)],
    );
    let mut game = Game::new(7703, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world
        .entity_mut(enemies[0])
        .insert(Routines(vec!["test_zero_cd_strike".to_string()]));
    assert_eq!(
        game.world
            .resource::<crate::abilities::AbilityDb>()
            .get("test_zero_cd_strike")
            .unwrap()
            .cooldown,
        0,
        "the ability under test must genuinely declare no cooldown, or this proves nothing"
    );

    game.wild_retaliate(enemies[0], 0, player);
    // One round-end tick, standing in for `tick_round_status_effects` — the
    // `+1` in `abilities::armed_cooldown` exists precisely so this tick
    // doesn't eat the round the routine just fired on. Checking readiness
    // *before* this tick can't tell floor 1 from floor 0: either arms a
    // cooldown that is still nonzero this same round. It's the round after
    // that distinguishes them — floor 0 would already read zero and fire
    // again, floor 1 still reads 1.
    game.tick_ability_cooldowns(enemies[0]);
    assert!(
        game.wild_routine_ready(enemies[0]).is_none(),
        "the enemy side floors the cooldown at ENEMY_ROUTINE_MIN_COOLDOWN, so a mod ability \
         declaring none still cannot fire two rounds running"
    );
}

/// A `FieldBuff` effect is field-only and has no in-battle resolution — see
/// the `unreachable!` arm `AbilityEffect::FieldBuff` hits in `use_ability`.
/// A carrier holding nothing else must fall back to an ordinary move rather
/// than picking a routine it can't run.
#[test]
fn a_carrier_whose_only_routine_is_field_only_has_nothing_ready() {
    let dir = super::support::modded_assets_dir(
        "field_only_carrier",
        &[],
        &[],
        &[],
        &[],
        &[("test_field_regen.ron", super::support::FIELD_ONLY_ABILITY)],
    );
    let mut game = Game::new(7704, DifficultyMode::Forgiving, &dir).unwrap();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.world
        .entity_mut(enemies[0])
        .insert(Routines(vec!["test_field_regen".to_string()]));

    assert!(
        game.wild_routine_ready(enemies[0]).is_none(),
        "a FieldBuff-only carrier has nothing it can spend a battle round on"
    );
}

/// A buff aimed at a hostile has to expire on schedule. While abilities
/// were party-only, `tick_combat_buff` was never called for a hostile — so
/// a mirrored buff or sap would have lasted the whole fight regardless of
/// its authored duration.
#[test]
fn a_buff_on_a_hostile_ticks_down_each_round() {
    let mut game = Game::new(7705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    game.arm_buff(
        enemies[0],
        ActiveBuff {
            kind: BuffKind::Atk,
            remaining: 2,
            power: 5,
        },
    );

    game.tick_round_status_effects(player);
    assert_eq!(
        game.world
            .get::<CombatBuff>(enemies[0])
            .unwrap()
            .active
            .unwrap()
            .remaining,
        1,
        "one round burned"
    );
    game.tick_round_status_effects(player);
    assert!(
        game.world
            .get::<CombatBuff>(enemies[0])
            .unwrap()
            .active
            .is_none(),
        "and it expires rather than lasting the whole fight"
    );
}

/// Hostiles that survive a jack-out stay on the map. A mirrored buff left
/// armed on one would be a permanent free stat that never ticks down.
#[test]
fn ending_a_battle_clears_every_hostiles_combat_state() {
    let mut game = Game::new(7704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 3, 200);
    for &e in &enemies {
        game.arm_buff(
            e,
            ActiveBuff {
                kind: BuffKind::Atk,
                remaining: 5,
                power: 9,
            },
        );
        game.world.get_mut::<StatusEffects>(e).unwrap().active = Some(ActiveStatus {
            kind: StatusKind::Bleed,
            remaining: 3,
            power: 2,
            landed_this_round: false,
        });
        game.world.entity_mut(e).insert(AbilityCooldowns(
            std::iter::once(("kernel_panic".to_string(), 3)).collect(),
        ));
    }

    game.end_battle(player, None);

    for &e in &enemies {
        assert!(
            game.world
                .get::<CombatBuff>(e)
                .is_none_or(|b| b.active.is_none()),
            "a buff left armed on a surviving hostile never ticks down — it is a free stat forever"
        );
        assert!(
            game.world
                .get::<StatusEffects>(e)
                .is_none_or(|s| s.active.is_none()),
            "and a bleed left running would tick outside any battle"
        );
        assert!(
            game.world
                .get::<AbilityCooldowns>(e)
                .is_none_or(|c| c.0.is_empty()),
            "and a routine's own cooldown would carry into the next fight — the one of the \
             three that only exists on a hostile because of this branch"
        );
    }
}

/// The death line has to be `Outcome`, not the default `Info` kind:
/// `MessageLog::retain_outcomes_since_battle` prunes everything else when
/// the battle ends, so an `Info` line would be announced mid-fight and then
/// silently vanish before the player reached the map.
#[test]
fn a_companion_brought_to_zero_announces_its_deletion_and_its_lost_routines() {
    let mut game = Game::new(4242, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(companion)
        .insert(Routines(vec!["priority_boost".to_string()]));
    game.add_companion(companion).unwrap();
    let name = game.creature_label(companion);

    game.apply_damage(companion, 10);

    let entry = game
        .message_log(20)
        .into_iter()
        .find(|e| e.text.contains(&name) && e.text.contains("deleted"))
        .expect("a companion reaching 0 HP must announce its deletion");
    assert_eq!(
        entry.kind,
        MessageKind::Outcome,
        "the death line must survive retain_outcomes_since_battle"
    );
    assert!(
        entry.text.contains("Hyperthread Single v1.0"),
        "the line must name the routines lost with it, got: {}",
        entry.text
    );
}

/// The guard is `Party` membership. A hostile reaching 0 is already handled
/// by `finish_member`, which logs its own kill line and awards loot — a
/// second announcement here would double-report every kill in the game.
#[test]
fn a_hostile_brought_to_zero_is_not_announced_by_the_companion_death_path() {
    let mut game = Game::new(4243, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_on_player_tile(&mut game);
    game.world.get_mut::<Stats>(wild).unwrap().hp = 5;

    game.apply_damage(wild, 5);

    assert!(
        !game
            .message_log(20)
            .iter()
            .any(|e| e.text.contains("deleted for good")),
        "only party members route through the companion death announcement"
    );
}

/// Damage that hurts without killing must stay silent, and a second hit on
/// an already-dead member must not announce twice — the transition is
/// `> 0` to `0`, not "is at 0".
#[test]
fn the_death_line_fires_once_on_the_transition_to_zero_and_never_above_it() {
    let mut game = Game::new(4244, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    game.apply_damage(companion, 4);
    assert!(
        !game
            .message_log(20)
            .iter()
            .any(|e| e.text.contains("deleted for good")),
        "a survivable hit must not announce a death"
    );

    game.apply_damage(companion, 6);
    game.apply_damage(companion, 6);
    let announcements = game
        .message_log(20)
        .iter()
        .filter(|e| e.text.contains("deleted for good"))
        .count();
    assert_eq!(
        announcements, 1,
        "hitting a corpse again must not re-announce its death"
    );
}

/// A running `Mitigation` routine cuts the hit by its percentage, computed
/// and rounded once against the raw damage.
#[test]
fn mitigation_field_buff_cuts_incoming_damage_by_its_percentage() {
    let mut game = Game::new(4246, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 100, 3);
    game.add_companion(companion).unwrap();
    game.arm_field_buff(companion, routine(FieldBuffKind::Mitigation, 25));

    game.apply_damage(companion, 20);

    assert_eq!(
        game.world.get::<Stats>(companion).unwrap().hp,
        85,
        "25% mitigation must cut a 20-point hit to 15, for 100 - 15 = 85 HP left"
    );
}

/// The floor is on mitigation's own effect, not damage in general: a hit
/// that would otherwise round to 0 must still land for 1, but mitigation
/// must never be the reason a hit that was already 0 becomes nonzero (that
/// case is covered by `apply_damage` flooring `dmg` at 0 well before this
/// path is reached).
#[test]
fn mitigation_never_reduces_a_landed_hit_below_one() {
    let mut game = Game::new(4247, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 100, 3);
    game.add_companion(companion).unwrap();
    // 3 * (1 - 0.95) = 0.15, which rounds to 0 — the case the floor exists for.
    game.arm_field_buff(companion, routine(FieldBuffKind::Mitigation, 95));

    game.apply_damage(companion, 3);

    assert_eq!(
        game.world.get::<Stats>(companion).unwrap().hp,
        99,
        "a chip hit must stay a hit even under heavy mitigation"
    );
}

/// No buff, no discount — proves the mitigation term is inert when absent
/// rather than, say, defaulting to some nonzero cut.
#[test]
fn no_mitigation_buff_leaves_damage_untouched() {
    let mut game = Game::new(4248, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 100, 3);
    game.add_companion(companion).unwrap();

    game.apply_damage(companion, 20);

    assert_eq!(game.world.get::<Stats>(companion).unwrap().hp, 80);
}

/// Mitigation is `FieldScope::Creature`, so it has to be read off the entity
/// taking the hit — a companion's own buff must not leak protection onto,
/// or borrow it from, anyone else in the party.
#[test]
fn a_companions_own_mitigation_protects_only_that_companion() {
    let mut game = Game::new(4249, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let shielded = spawn_tamed(&mut game, 100, 3);
    let unshielded = spawn_tamed(&mut game, 100, 3);
    game.add_companion(shielded).unwrap();
    game.add_companion(unshielded).unwrap();
    game.arm_field_buff(shielded, routine(FieldBuffKind::Mitigation, 25));

    game.apply_damage(shielded, 20);
    game.apply_damage(unshielded, 20);

    assert_eq!(
        game.world.get::<Stats>(shielded).unwrap().hp,
        85,
        "the shielded companion's own buff must cut its hit"
    );
    assert_eq!(
        game.world.get::<Stats>(unshielded).unwrap().hp,
        80,
        "a party-mate's buff must not protect a companion that doesn't carry it"
    );
}

/// The player is not a party member and must never be reaped by this path —
/// flatlining stays with `difficulty::death_handling_system`, which is what
/// `DifficultyMode` selects between. A player deleted from the world would
/// take the whole run with it.
#[test]
fn the_player_at_zero_hp_is_not_touched_by_the_program_death_path() {
    let mut game = Game::new(4245, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let hp = game.world.get::<Stats>(player).unwrap().hp;

    game.apply_damage(player, hp);

    assert!(
        game.world.get::<Stats>(player).is_some(),
        "the player entity survives 0 HP; only difficulty handling may act on it"
    );
    assert!(
        !game
            .message_log(20)
            .iter()
            .any(|e| e.text.contains("deleted for good")),
        "the player does not get a program death line"
    );
}

fn consumable(kind: FieldBuffKind, power: i32) -> ActiveFieldBuff {
    ActiveFieldBuff {
        kind,
        name: "test item".to_string(),
        power,
        remaining: 10,
        interval: 1,
        source: BuffSource::Consumable,
    }
}

fn routine(kind: FieldBuffKind, power: i32) -> ActiveFieldBuff {
    ActiveFieldBuff {
        kind,
        name: "test routine".to_string(),
        power,
        remaining: 10,
        interval: 1,
        source: BuffSource::Routine,
    }
}

#[test]
fn a_second_consumable_field_buff_displaces_the_first_even_of_a_different_kind() {
    let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    game.arm_field_buff(player, consumable(FieldBuffKind::Mitigation, 2));
    game.arm_field_buff(player, consumable(FieldBuffKind::Atk, 5));

    let active = &game.world.get::<FieldBuff>(player).unwrap().active;
    assert_eq!(
        active.len(),
        1,
        "a second consumable-armed buff must replace the first, not stack"
    );
    assert_eq!(active[0].kind, FieldBuffKind::Atk);
    assert_eq!(active[0].power, 5);
}

#[test]
fn a_second_routine_of_the_same_kind_displaces_only_that_kind() {
    let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    game.arm_field_buff(player, routine(FieldBuffKind::Mitigation, 2));
    game.arm_field_buff(player, routine(FieldBuffKind::Atk, 3));
    game.arm_field_buff(player, routine(FieldBuffKind::Mitigation, 9));

    let active = &game.world.get::<FieldBuff>(player).unwrap().active;
    assert_eq!(
        active.len(),
        2,
        "recasting one routine kind must not touch a different running routine"
    );
    assert_eq!(game.field_buff_power(player, FieldBuffKind::Mitigation), 9);
    assert_eq!(game.field_buff_power(player, FieldBuffKind::Atk), 3);
}

#[test]
fn a_routine_of_a_different_kind_coexists_with_the_first() {
    let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    game.arm_field_buff(player, routine(FieldBuffKind::Regen, 4));
    game.arm_field_buff(player, routine(FieldBuffKind::Mitigation, 10));

    let active = &game.world.get::<FieldBuff>(player).unwrap().active;
    assert_eq!(active.len(), 2, "distinct routine kinds must coexist");
}

#[test]
fn an_item_buff_and_a_routine_buff_coexist() {
    let mut game = Game::new(9004, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    game.arm_field_buff(player, consumable(FieldBuffKind::Mitigation, 2));
    game.arm_field_buff(player, routine(FieldBuffKind::Mitigation, 9));

    let active = &game.world.get::<FieldBuff>(player).unwrap().active;
    assert_eq!(
        active.len(),
        2,
        "a consumable buff and a routine buff of the same kind must not collide"
    );
}

#[test]
fn field_buff_power_is_zero_when_absent_and_the_stored_value_when_present() {
    let mut game = Game::new(9005, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    assert_eq!(
        game.field_buff_power(player, FieldBuffKind::Regen),
        0,
        "no FieldBuff has been armed yet"
    );

    game.arm_field_buff(player, routine(FieldBuffKind::Regen, 7));
    assert_eq!(game.field_buff_power(player, FieldBuffKind::Regen), 7);
    assert_eq!(
        game.field_buff_power(player, FieldBuffKind::Trickle),
        0,
        "a different kind stays absent"
    );
}

#[test]
fn field_buff_power_sums_a_consumable_and_a_routine_of_the_same_kind() {
    let mut game = Game::new(9007, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    game.arm_field_buff(player, consumable(FieldBuffKind::Mitigation, 2));
    game.arm_field_buff(player, routine(FieldBuffKind::Mitigation, 5));

    assert_eq!(
        game.field_buff_power(player, FieldBuffKind::Mitigation),
        7,
        "a consumable and a routine of the same kind coexist (arm_field_buff's \
         whole reason for two separate displacement rules), so a reader summing \
         only one of them would make that coexistence pointless"
    );
}

#[test]
fn arm_field_buff_inserts_the_component_on_demand_for_a_companion() {
    let mut game = Game::new(9006, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 20, 3);
    assert!(
        game.world.get::<FieldBuff>(companion).is_none(),
        "only the player is spawned holding a FieldBuff"
    );

    game.arm_field_buff(companion, routine(FieldBuffKind::Mitigation, 3));

    assert_eq!(
        game.field_buff_power(companion, FieldBuffKind::Mitigation),
        3
    );
}

/// The round a condition lands in is not one of the rounds it lasts.
///
/// A `duration: 1` stun — which is every stun the shipped roster carries —
/// used to be armed mid-round and then ticked away by that same round's
/// end-of-round upkeep, so the victim only ever lost a turn when the
/// attacker happened to out-roll it on initiative. Play read as "it shakes
/// off the stun before it ever costs it anything".
#[test]
fn a_stun_that_lands_this_round_costs_the_victim_their_next_turn() {
    let mut game = Game::new(7710, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 200);
    // Deadlock is the `duration: 1` stun, and a carrier fires its routine
    // rather than rolling a move — so the stun lands in round 1 for certain.
    game.world
        .entity_mut(enemies[0])
        .insert(Routines(vec!["deadlock".to_string()]));
    game.world.get_mut::<Stats>(enemies[0]).unwrap().atk = 0;
    // Round 2 it acts on a *move*, its routine having gone on cooldown, and
    // `battle_with_a_pack_of`'s default species is a Cipher — whose Encrypt
    // carries a `duration: 1` Stun of its own. A fresh stun landing there is
    // a legitimate event that says nothing about whether the first one
    // cleared, so the pack is a Scrapper instead: two moves, no effects.
    // Left as a Cipher this passed on `WILD_ABILITY_CHANCE` x that move's
    // own 0.35 missing, which is a 93% coin flip and not a mechanic.
    game.world.get_mut::<Creature>(enemies[0]).unwrap().species = "scrapper".to_string();

    player_attacks(&mut game);
    assert!(
        game.is_stunned(player),
        "the carrier's Deadlock should still be on the player going into round 2"
    );

    let before = game.world.get::<Stats>(enemies[0]).unwrap().hp;
    player_attacks(&mut game);
    let after = game.world.get::<Stats>(enemies[0]).unwrap().hp;

    assert_eq!(
        before, after,
        "a stunned player deals no damage — the stun cost them the round it was for"
    );
    assert!(
        !game.is_stunned(player),
        "and having cost them that round, it clears"
    );
}

/// The same off-by-one from the other side: `memory_leak` advertises "Bleed
/// 2 per round for 3 rounds", and its first tick used to land in the round
/// the bleed was applied, so the roster showed one fewer round than the
/// description promised.
#[test]
fn a_bleed_deals_its_damage_in_the_rounds_after_the_one_it_landed_in() {
    let mut game = Game::new(7711, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one species");
    let wild = game
        .world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 5, y: 5 },
            Stats {
                hp: 100,
                max_hp: 100,
                atk: 0,
                mitigation: 0,
            },
            StatusEffects::default(),
        ))
        .id();

    game.apply_status_effect(
        wild,
        &species::MoveEffect {
            kind: StatusKind::Bleed,
            chance: 1.0,
            duration: 2,
            power: 5,
        },
        "it",
        MessageKind::PartyDamage,
    );

    let mut hp_after_each_round = Vec::new();
    for _ in 0..3 {
        game.tick_status_effects(wild, "it");
        hp_after_each_round.push(game.world.get::<Stats>(wild).unwrap().hp);
    }

    assert_eq!(
        hp_after_each_round,
        vec![100, 95, 90],
        "the landing round costs nothing; the two rounds it was advertised for each bleed"
    );
    assert!(
        game.world
            .get::<StatusEffects>(wild)
            .unwrap()
            .active
            .is_none(),
        "and it clears once both of those rounds have passed"
    );
}

/// A local twin of `combat_targeting`'s helper — a routine-armed field buff
/// with a long enough clock that nothing ticks it away mid-test.
fn running_field_buff(kind: FieldBuffKind, power: i32) -> ActiveFieldBuff {
    ActiveFieldBuff {
        kind,
        name: "Test Field Buff".to_string(),
        power,
        remaining: 5,
        interval: 1,
        source: BuffSource::Routine,
    }
}

/// Innate mitigation, gear (already baked into `Stats` by
/// `apply_equipment_delta`) and a running field buff all count, and the sum
/// is capped. Delete the `.clamp(0, MAX_MITIGATION_PERCENT)` and this fails
/// at a stacked total.
#[test]
fn mitigation_sums_its_sources_and_stops_at_the_cap() {
    let mut game = Game::new(770, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    game.world.get_mut::<Stats>(pet).unwrap().mitigation = 60;
    game.arm_field_buff(pet, running_field_buff(FieldBuffKind::Mitigation, 40));
    assert_eq!(
        game.effective_mitigation(pet),
        crate::tuning::MAX_MITIGATION_PERCENT,
        "60 innate plus 40 buffed is 100, which the cap has to bring down"
    );
}

/// The three sources really are summed, not shadowed — a total under the cap
/// must be the arithmetic rather than whichever source happened to be
/// largest.
#[test]
fn mitigation_below_the_cap_is_the_sum_of_its_sources() {
    let mut game = Game::new(771, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    game.world.get_mut::<Stats>(pet).unwrap().mitigation = 9;
    game.arm_field_buff(pet, running_field_buff(FieldBuffKind::Mitigation, 13));
    assert_eq!(game.effective_mitigation(pet), 22);
}

/// A landed hit stays a hit under heavy mitigation, but a miss is not raised
/// to 1. This is `mitigate_incoming_damage`'s existing behaviour and it must
/// survive the rewrite onto `effective_mitigation`.
#[test]
fn heavy_mitigation_floors_a_landed_hit_at_one_and_leaves_a_miss_alone() {
    let mut game = Game::new(772, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 30, 5);
    game.world.get_mut::<Stats>(pet).unwrap().mitigation = crate::tuning::MAX_MITIGATION_PERCENT;

    let before = game.world.get::<Stats>(pet).unwrap().hp;
    game.apply_damage(pet, 2);
    assert_eq!(
        game.world.get::<Stats>(pet).unwrap().hp,
        before - 1,
        "a hit still lands"
    );

    let after_hit = game.world.get::<Stats>(pet).unwrap().hp;
    game.apply_damage(pet, 0);
    assert_eq!(
        game.world.get::<Stats>(pet).unwrap().hp,
        after_hit,
        "a miss costs nothing"
    );
}

/// Innate mitigation reaches the damage path at all. Before this task only a
/// running field buff did, so a species' authored toughness and every worn
/// piece of armour were invisible to `mitigate_incoming_damage`.
#[test]
fn innate_mitigation_cuts_incoming_damage() {
    let mut game = Game::new(773, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let soft = spawn_tamed(&mut game, 200, 5);
    let armoured = spawn_tamed(&mut game, 200, 5);
    game.world.get_mut::<Stats>(soft).unwrap().mitigation = 0;
    game.world.get_mut::<Stats>(armoured).unwrap().mitigation = 50;

    game.apply_damage(soft, 40);
    game.apply_damage(armoured, 40);

    let soft_lost = 200 - game.world.get::<Stats>(soft).unwrap().hp;
    let armoured_lost = 200 - game.world.get::<Stats>(armoured).unwrap().hp;
    assert_eq!(soft_lost, 40);
    assert_eq!(armoured_lost, 20, "half of it shrugged off");
}

// ------------------------------------------------------------ fumble ladder

/// Exposed cuts the fumbler's evasion until their next turn — which is what
/// makes rung 1 a cost rather than flavour. Delete the
/// `EXPOSED_EVASION_PERCENT` term in `combatant_profile` and this fails.
#[test]
fn exposed_cuts_the_fumblers_evasion() {
    let mut game = Game::new(780, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let victim = spawn_wild_without_routine(&mut game, "scrapper", 20, 20);
    let clean = game
        .combatant_profile(victim, battle::DamageRange::default())
        .evasion;
    game.arm_status(victim, StatusKind::Exposed, 1, 0);
    let exposed = game
        .combatant_profile(victim, battle::DamageRange::default())
        .evasion;
    assert!(exposed < clean, "{exposed} should be below {clean}");
}

/// Every rung of the ladder that deals damage goes through `apply_damage`,
/// which stays the only path that damages a creature — and it lands on the
/// *fumbler*, never on what they were swinging at.
#[test]
fn a_recoil_fumble_hurts_the_fumbler_and_not_the_target() {
    let mut game = Game::new(781, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let fumbler = spawn_wild_without_routine(&mut game, "scrapper", 20, 20);
    let target = game.player_entity();
    let fumbler_before = game.world.get::<Stats>(fumbler).unwrap().hp;
    let target_before = game.world.get::<Stats>(target).unwrap().hp;

    game.apply_fumble_rung(fumbler, target, battle::FumbleRung::Recoil { dmg: 4 });

    assert!(game.world.get::<Stats>(fumbler).unwrap().hp < fumbler_before);
    assert_eq!(game.world.get::<Stats>(target).unwrap().hp, target_before);
}

/// Rung 4 costs the fumbler their next action, through the machinery Stun
/// already has.
#[test]
fn a_crash_fumble_costs_the_fumbler_their_next_action() {
    let mut game = Game::new(782, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let fumbler = spawn_wild_without_routine(&mut game, "scrapper", 20, 20);
    let target = game.player_entity();
    game.apply_fumble_rung(fumbler, target, battle::FumbleRung::Crash);
    assert!(game.is_stunned(fumbler));
}

/// Rungs replace rather than stack — a cumulative top rung is a run-ender.
#[test]
fn a_second_fumble_replaces_the_first_rung() {
    let mut game = Game::new(783, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let fumbler = spawn_wild_without_routine(&mut game, "scrapper", 20, 20);
    let target = game.player_entity();
    game.apply_fumble_rung(fumbler, target, battle::FumbleRung::Exposed);
    game.apply_fumble_rung(fumbler, target, battle::FumbleRung::Crash);
    assert!(game.is_stunned(fumbler));
    assert_eq!(
        game.status_label(fumbler).as_deref(),
        Some("Stunned (1)"),
        "one status at a time — the second must clobber the first"
    );
}

/// Landing a fumble rung must spend no further `GameRng` draws — the roll
/// happened inside `resolve_attack`. A draw here would shift every seeded
/// run's stream by however many fumbles it happened to contain.
#[test]
fn landing_a_fumble_rung_spends_no_rng() {
    use rand::RngExt;
    let mut game = Game::new(784, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    // Spawned *before* the snapshot: `spawn_wild_creature` draws, which is
    // why `rng_unadvanced_by` cannot measure this one — its closure has to
    // contain the setup as well as the act.
    let fumbler = spawn_wild_without_routine(&mut game, "scrapper", 20, 20);
    let target = game.player_entity();

    // Pinned to a known seed and compared against a fresh stream on the same
    // one: `StdRng` is not `Clone`, so a snapshot has to be reconstructed
    // rather than copied.
    reseed_rng(&mut game, 991);
    for rung in [
        battle::FumbleRung::Recoil { dmg: 4 },
        battle::FumbleRung::Crash,
        battle::FumbleRung::Exposed,
        battle::FumbleRung::Opening { dmg: 3 },
    ] {
        game.apply_fumble_rung(fumbler, target, rung);
    }

    let mut untouched: rand::rngs::StdRng = rand::SeedableRng::seed_from_u64(991);
    let expected: u64 = untouched.random();
    let actual: u64 = game.world.resource_mut::<GameRng>().0.random();
    assert_eq!(
        actual, expected,
        "landing four rungs moved the stream — the rolls all happened inside \
         `resolve_attack`, and a draw here would shift every seeded run by \
         however many fumbles it contained"
    );
}
