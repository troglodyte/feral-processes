use super::*;

/// Teardown parity: a tactical fight pays and closes through the same door a
/// group fight does — the win headline, the XP, and both fight resources
/// gone.
#[test]
fn killing_the_last_hostile_ends_the_fight_and_pays_for_it() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 1);
    let player = game.player_entity();
    let target = pack[0];
    let before = game.world.get::<Experience>(player).unwrap().xp;

    for _ in 0..64 {
        if game.world.get_resource::<TacticalBattle>().is_none() {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let at = game.world.resource::<TacticalBattle>().cell_of(target);
        let Some(at) = at else { break };
        if let Some(beside) = beside(&game, at) {
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, beside);
        }
        game.tactical_attack(target);
    }

    assert!(
        game.world.get_resource::<TacticalBattle>().is_none(),
        "the fight is still open"
    );
    assert!(
        game.world.get_resource::<BattleState>().is_none(),
        "a group fight was opened along the way"
    );
    assert!(
        game.world.get::<Stats>(target).is_none(),
        "the dead hostile is still standing"
    );
    let lines = log_texts(&game);
    assert!(
        lines.iter().any(|t| *t == "You won!"),
        "the win was never announced: {lines:#?}"
    );
    assert!(
        game.world.get::<Experience>(player).unwrap().xp > before,
        "the kill paid no experience"
    );
    let board = game.tactical_result_view().expect("a closing board");
    assert_eq!(
        board.fallen.len(),
        1,
        "the kill that ended the fight must still be marked on the closing board"
    );
}

/// A body killed on a battle map leaves a mark where it fell, for the rest
/// of the fight.
#[test]
fn a_body_killed_on_a_battle_map_leaves_a_mark_where_it_fell() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 2, 1);
    let player = game.player_entity();
    let target = pack[0];
    let mut last_seen = None;
    let species = game.world.get::<Creature>(target).unwrap().species.clone();
    let stats_before = *game.world.get::<Stats>(target).unwrap();

    for _ in 0..64 {
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let Some(at) = game.world.resource::<TacticalBattle>().cell_of(target) else {
            break;
        };
        last_seen = Some(at);
        if let Some(beside) = beside(&game, at) {
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, beside);
        }
        game.tactical_attack(target);
    }

    assert!(
        game.world.get::<Stats>(target).is_none(),
        "the target was never killed"
    );
    let view = game
        .tactical_view()
        .expect("the second hostile holds the fight open");
    assert_eq!(view.fallen.len(), 1);
    let mark = &view.fallen[0];
    assert_eq!(mark.cell, last_seen.expect("the target stood somewhere"));
    assert_eq!(mark.footprint, 1);
    let raise = mark
        .raise
        .as_ref()
        .expect("an ordinary hostile's death is raisable");
    assert_eq!(raise.species, species);
    assert_eq!(raise.stats.max_hp, stats_before.max_hp);
    assert_eq!(raise.stats.atk, stats_before.atk);
    assert_eq!(raise.stats.mitigation, stats_before.mitigation);
}

/// A boss's own fight is not meant to be re-fought off its corpse: it falls
/// with a plain, unraisable mark.
#[test]
fn a_boss_falls_with_no_snapshot_to_raise_it_from() {
    let mut game = game();
    let pack = tactical_fight(&mut game, 1, 1);
    let player = game.player_entity();
    let target = pack[0];
    game.world
        .entity_mut(target)
        .insert(crate::components::Boss);

    for _ in 0..64 {
        if game.world.get_resource::<TacticalBattle>().is_none() {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let Some(at) = game.world.resource::<TacticalBattle>().cell_of(target) else {
            break;
        };
        if let Some(beside) = beside(&game, at) {
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, beside);
        }
        game.tactical_attack(target);
    }

    let board = game
        .tactical_result_view()
        .expect("the last hostile died, closing the fight");
    assert_eq!(board.fallen.len(), 1, "the boss must still leave its mark");
    assert!(
        board.fallen[0].raise.is_none(),
        "a boss is not what Respawn is for"
    );
}

/// Respawn's `tactical_only` effect has nothing to resolve against in the
/// group model — `Game::ability_unavailable`'s first check, ahead of
/// cooldown and Power, exactly as it is for every other tactical-only
/// routine (`a_tamper_routine_is_refused_outside_a_battle_map`'s pattern).
#[test]
fn respawn_is_refused_outside_a_battle_map_and_spends_no_power() {
    let mut game = game();
    let player = game.player_entity();
    let enemies = battle_with_a_pack_of(&mut game, 1, 40);
    insert_battle(&mut game, player, enemies);
    let respawn = game
        .world
        .resource::<crate::abilities::AbilityDb>()
        .get("respawn")
        .cloned()
        .expect("respawn ships");
    let before = game
        .world
        .get::<crate::components::PowerReserve>(player)
        .map(|r| r.get());

    assert_eq!(
        game.ability_unavailable(player, &respawn),
        Some(crate::game::combat::RoutineRefusal::BattleMapOnly),
        "a tactical-only routine must be refused in a group fight"
    );
    assert_eq!(
        game.world
            .get::<crate::components::PowerReserve>(player)
            .map(|r| r.get()),
        before,
        "a refusal spends no Power"
    );
}

/// A tactical fight's telemetry says what the model actually is: one group
/// per hostile body, because groups dissolve on a battle map, and a party
/// list read off `Party` rather than off a battle line it does not have.
#[test]
fn a_tactical_fight_reports_a_group_per_body_and_a_matching_end() {
    let mut game = game();
    game.enable_telemetry();
    let pack = tactical_fight(&mut game, 2, 1);
    let player = game.player_entity();

    for _ in 0..64 {
        if game.world.get_resource::<TacticalBattle>().is_none() {
            break;
        }
        if !wait_for_turn(&mut game, player) {
            break;
        }
        let target = pack
            .iter()
            .copied()
            .find(|&e| game.world.resource::<TacticalBattle>().cell_of(e).is_some());
        let Some(target) = target else { break };
        let at = game
            .world
            .resource::<TacticalBattle>()
            .cell_of(target)
            .unwrap();
        if let Some(beside) = beside(&game, at) {
            game.world
                .resource_mut::<TacticalBattle>()
                .move_to(player, beside);
        }
        game.tactical_attack(target);
    }

    let records = game.take_telemetry();
    let start = records
        .iter()
        .find_map(|r| match r {
            crate::telemetry::Record::FightStart {
                fight,
                party,
                enemies,
                ..
            } => Some((*fight, party.len(), enemies.clone())),
            _ => None,
        })
        .expect("a tactical fight opened without a FightStart");
    assert_eq!(start.1, 1, "the party list read off a battle line it lacks");
    assert_eq!(start.2.len(), 2, "the pack was not one group per body");
    assert!(
        start.2.iter().all(|g| g.count == 1),
        "a group on a battle map holds more than one body"
    );

    let end = records
        .iter()
        .find_map(|r| match r {
            crate::telemetry::Record::FightEnd { fight, won, .. } => Some((*fight, *won)),
            _ => None,
        })
        .expect("the fight ended without a FightEnd");
    assert_eq!(end.0, start.0, "the end names a different fight");
    assert!(end.1, "clearing the board did not read as a win");
}

/// The gate roughly a hundred refusals ask through knows about both models.
#[test]
fn a_tactical_fight_counts_as_an_active_battle() {
    let mut game = game();
    assert!(!game.has_active_battle());
    tactical_fight(&mut game, 1, 10);
    assert!(
        game.has_active_battle(),
        "every screen that refuses mid-fight would have opened on a battle map"
    );
}
