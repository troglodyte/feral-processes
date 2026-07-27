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
