//! Status effects: how stun and bleed tick down, and when they clear.

use super::support::*;
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
                def: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
        kind: StatusKind::Stun,
        remaining: 1,
        power: 0,
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
                def: 0,
            },
            StatusEffects {
                active: Some(ActiveStatus {
                    kind: StatusKind::Bleed,
                    remaining: 2,
                    power: 5,
                }),
            },
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    let player_atk = game.world.get::<Stats>(player).unwrap().atk;
    let expected_attack_dmg = battle::compute_damage(player_atk, 0, 5);

    let hp_before = game.world.get::<Stats>(wild).unwrap().hp;
    player_attacks(&mut game);
    let hp_after = game.world.get::<Stats>(wild).unwrap().hp;
    assert_eq!(
        hp_before - hp_after,
        expected_attack_dmg + 5,
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
    player_attacks(&mut game);
    let hp_after2 = game.world.get::<Stats>(wild).unwrap().hp;
    assert_eq!(
        hp_before2 - hp_after2,
        expected_attack_dmg + 5,
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
                def: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<StatusEffects>(player).unwrap().active = Some(ActiveStatus {
        kind: StatusKind::Bleed,
        remaining: 5,
        power: 1,
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
        .filter(|(kind, _)| *kind == MessageKind::Outcome)
        .map(|(_, line)| line)
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

    let battle: Vec<String> = game.battle_log().into_iter().map(|(_, l)| l).collect();

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

    let battle: Vec<String> = game.battle_log().into_iter().map(|(_, l)| l).collect();

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

    let after: Vec<String> = game.message_log(100).into_iter().map(|(_, l)| l).collect();
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
    let first = game.battle_log_id();
    game.log("first battle narration");
    game.end_battle(player, None);

    start_battle_with_a_wild_program(&mut game);

    assert_ne!(first, game.battle_log_id(), "the battle id did not advance");
    let battle: Vec<String> = game.battle_log().into_iter().map(|(_, l)| l).collect();
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

    let battle: Vec<String> = game.battle_log().into_iter().map(|(_, l)| l).collect();
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
            .any(|(_, l)| l.contains("intercepts your signal")),
        "the opening line should be in the pane before any round resolves"
    );

    resolve_round_with(&mut game, BattleAction::Defend);

    let pane: Vec<String> = game.battle_log().into_iter().map(|(_, l)| l).collect();
    assert!(
        !pane.iter().any(|l| l.contains("intercepts your signal")),
        "the opening line survived into the next round: {pane:?}"
    );
    assert!(
        pane.iter().any(|l| l.contains("round")),
        "the round's own narration is missing: {pane:?}"
    );
}
