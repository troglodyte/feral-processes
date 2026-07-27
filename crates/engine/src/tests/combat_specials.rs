//! Companion special abilities: the menus that offer them and the buffs they apply.

use super::support::*;
use crate::tuning::COMPANION_COMMAND_FATIGUE_COST;
use crate::*;

#[test]
fn a_companions_special_rallies_the_player_instead_of_attacking() {
    let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 20);
    game.add_companion(companion).unwrap();

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
                atk: 1,
                def: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    companion_uses_special(
        &mut game,
        companion,
        0,
        battle::SpecialTarget::Ally { slot: 0 },
    );

    let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
    assert_eq!(
        wild_hp, 100,
        "commanding a companion should never damage the wild creature directly"
    );
    let buff = game.world.get::<CombatBuff>(player).unwrap().active;
    assert!(
        buff.is_some_and(|b| b.kind == BuffKind::Atk),
        "commanding a companion with no special ability should rally (ATK buff) the player"
    );
}

#[test]
fn commanding_a_companion_in_battle_costs_more_fatigue_than_a_stunned_one() {
    // Both paths advance the clock by one tick (a resolved round
    // always ticks at the end), so both pay the same small natural
    // fatigue decay regardless — comparing the two deltas rather than
    // asserting an absolute number isolates just the companion-command
    // cost from that shared per-tick decay.
    let active = fatigue_spent_commanding_companion(84, false);
    let stunned = fatigue_spent_commanding_companion(85, true);
    assert!(
        (active - stunned - COMPANION_COMMAND_FATIGUE_COST).abs() < 0.001,
        "commanding an active companion should cost exactly {COMPANION_COMMAND_FATIGUE_COST} \
         more fatigue than commanding a stunned one, which doesn't actually act: \
         active spent {active}, stunned spent {stunned}"
    );
}

#[test]
fn an_atk_buff_increases_damage_dealt_and_expires_after_its_duration() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<CombatBuff>(player).unwrap().active = Some(ActiveBuff {
        kind: BuffKind::Atk,
        remaining: 1,
        power: 50,
    });

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
                hp: 10_000,
                max_hp: 10_000,
                atk: 0,
                def: 0,
            },
            StatusEffects::default(),
        ))
        .id();
    insert_battle(&mut game, player, vec![wild]);

    player_attacks(&mut game);

    let wild_hp = game.world.get::<Stats>(wild).unwrap().hp;
    assert!(
        wild_hp < 10_000 - 50,
        "a +50 ATK buff should meaningfully increase damage dealt"
    );
    assert!(
        game.world
            .get::<CombatBuff>(player)
            .unwrap()
            .active
            .is_none(),
        "a 1-round buff should expire once the round it covered ticks down"
    );
}

#[test]
fn special_ability_heal_restores_player_hp_and_debuff_afflicts_the_wild_creature() {
    let mut game = Game::new(19, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 5;

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
                atk: 1,
                def: 0,
            },
            StatusEffects::default(),
        ))
        .id();

    let heal = ability(&game, "hot_patch");
    game.use_ability(&heal, player, "TestBot", &[player]);
    let hp = game.world.get::<Stats>(player).unwrap().hp;
    assert_eq!(
        hp, 13,
        "Heal should restore the player's HP by its power, capped at max_hp"
    );

    let debuff = ability(&game, "memory_leak");
    game.use_ability(&debuff, player, "TestBot", &[wild]);
    let active = game.world.get::<StatusEffects>(wild).unwrap().active;
    assert!(
        active.is_some_and(|a| a.kind == StatusKind::Bleed && a.power == 2 && a.remaining == 3),
        "Debuff should inflict the status condition memory_leak declares"
    );
}

#[test]
fn companion_ability_label_shows_the_ability_name_or_the_fallback() {
    let mut game = Game::new(93, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let all_species = game.species_defs();
    let no_ability_species = all_species
        .iter()
        .find(|s| s.abilities.is_empty())
        .expect("at least one species with no declared abilities")
        .id
        .clone();

    let plain = game
        .world
        .spawn((
            Creature {
                species: no_ability_species,
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 30,
                def: 1,
            },
            Tamed { owner: player },
            Experience::default(),
        ))
        .id();
    game.install_innate_routines(plain);
    game.add_companion(plain).unwrap();
    let plain_ability = game.player_status().companions[0].ability.clone();
    assert_eq!(
        plain_ability, "Priority Boost",
        "a species declaring no abilities should show the fallback"
    );
}

#[test]
fn a_species_with_several_abilities_offers_each_one_in_menu_order() {
    let (mut game, medic) = game_with_two_ability_companion();
    // The medic's second ability is gated at level 5; this test is about
    // menu order, so unlock it rather than asserting on a one-row list.
    set_level(&mut game, medic, 5);
    let options = game.battle_special_options(1);
    assert_eq!(
        options.iter().map(|o| o.name.as_str()).collect::<Vec<_>>(),
        vec!["Hot Patch", "Sandbox"],
        "the picker should list the species' abilities in declaration order"
    );
    assert_eq!(
        options.iter().map(|o| o.index).collect::<Vec<_>>(),
        vec![0, 1],
        "index is what BattleAction::Special carries, so it must match position"
    );
    assert_eq!(options[0].detail, "Restore 8 Integrity to one ally");
}

#[test]
fn an_ability_above_the_companions_level_is_not_offered_yet() {
    let (game, medic) = game_with_two_ability_companion();
    assert_eq!(
        game.world.get::<Experience>(medic).unwrap().level,
        1,
        "a freshly tamed program starts at level 1"
    );

    let options = game.battle_special_options(1);
    assert_eq!(
        options.len(),
        1,
        "the level-5 ability must stay hidden until it is earned"
    );
    assert_eq!(options[0].name, "Hot Patch");
}

#[test]
fn a_companion_declaring_no_abilities_still_offers_exactly_the_fallback() {
    let mut game = Game::new(95, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    let options = game.battle_special_options(1);
    assert_eq!(
        options.len(),
        1,
        "the fallback is resolved into the list, so the menu is never empty"
    );
    assert_eq!(options[0].name, "Priority Boost");
}

#[test]
fn buffs_and_heals_aim_at_the_party_while_debuffs_aim_at_the_enemy() {
    use crate::abilities::AbilityTarget;
    use battle::SpecialTargeting;
    assert_eq!(AbilityTarget::OneAlly.targeting(), SpecialTargeting::Ally);
    assert_eq!(
        AbilityTarget::OneEnemyGroupFront.targeting(),
        SpecialTargeting::Enemy
    );
    assert_eq!(
        AbilityTarget::WholeEnemyGroup.targeting(),
        SpecialTargeting::Enemy
    );
    // The two sweeping shapes leave the player nothing to choose, so they
    // open no second picker at all.
    assert_eq!(
        AbilityTarget::WholeParty.targeting(),
        SpecialTargeting::None
    );
    assert_eq!(
        AbilityTarget::AllEnemies.targeting(),
        SpecialTargeting::None
    );
}

/// The whole point of aiming a buff: it has to land on a companion, not
/// just the player. Only the player is *spawned* holding a `CombatBuff`,
/// so this is the case that silently did nothing before `arm_buff`.
#[test]
fn a_buff_aimed_at_a_companion_actually_reaches_it() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 5);
    start_battle_with_a_wild_program(&mut game);
    let before = game.effective_def(medic);

    // Slot 1 is the medic itself; index 1 is its Sandbox.
    companion_uses_special(&mut game, medic, 1, battle::SpecialTarget::Ally { slot: 1 });

    assert!(
        matches!(
            game.world.get::<CombatBuff>(medic).and_then(|b| b.active),
            Some(ActiveBuff {
                kind: BuffKind::Def,
                ..
            })
        ),
        "a companion with no CombatBuff component must have one inserted, not be skipped"
    );
    assert!(
        game.effective_def(medic) > before,
        "the buff has to actually raise the companion's defense"
    );
}

/// A party-facing Special must not need a living enemy group to resolve,
/// since its target isn't a group at all.
#[test]
fn healing_an_ally_does_not_depend_on_a_valid_enemy_group() {
    let (mut game, _) = game_with_two_ability_companion();
    let player = game.player_entity();
    game.world.get_mut::<Stats>(player).unwrap().hp = 5;
    start_battle_with_a_wild_program(&mut game);

    game.battle_set_action(
        1,
        BattleAction::Special {
            ability: 0,
            target: battle::SpecialTarget::Ally { slot: 0 },
        },
    )
    .expect("an ally-targeted Special has no group to reject");

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        5,
        "planning alone shouldn't heal — that happens on resolve"
    );
}

#[test]
fn the_chosen_ability_index_decides_which_special_resolves() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 5);
    let player = game.player_entity();
    // Wounded, so a heal would be visible, but nowhere near death. At 1 HP
    // the wild's retaliation flatlines the player mid-round, and Forgiving
    // mode revives them at full HP — which reads exactly like the heal this
    // test exists to rule out. The health here only has to outlast one
    // round's damage, which `max_hp` guarantees it does.
    let max_hp = game.world.get::<Stats>(player).unwrap().max_hp;
    game.world.get_mut::<Stats>(player).unwrap().hp = max_hp / 2;
    let wounded = game.world.get::<Stats>(player).unwrap().hp;
    start_battle_with_a_wild_program(&mut game);

    // Index 1 is Sandbox, which buffs DEF and must not heal.
    companion_uses_special(&mut game, medic, 1, battle::SpecialTarget::Ally { slot: 0 });
    assert!(
        game.world.get::<Stats>(player).unwrap().hp <= wounded,
        "picking Sandbox must not run Hot Patch, the ability at index 0"
    );
    assert!(
        matches!(
            game.world.get::<CombatBuff>(player).and_then(|b| b.active),
            Some(ActiveBuff {
                kind: BuffKind::Def,
                ..
            })
        ),
        "picking Sandbox should raise DEF"
    );
}
