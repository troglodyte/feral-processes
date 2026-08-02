//! The turn loop: ticking, resting, waiting, and consuming items.

use super::support::*;
use crate::game::turn::forage_chance;
use crate::tuning::{KEEN_SCAVENGER_BONUS_PER_LEVEL, MAX_BUILD_DISTANCE_FROM_HOME, REST_TICKS};
use crate::*;

#[test]
fn player_status_power_matches_max_hp_plus_atk_plus_def() {
    let game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let status = game.player_status();
    assert_eq!(status.power, status.max_hp + status.atk + status.def);
}

/// The map's Integrity gauge and the battle screen's "You" bar are two
/// readouts of one number. Nothing may fork them — not the entity they
/// resolve, not a buff, not a stale view.
#[test]
fn battle_view_integrity_matches_the_map_status_panel() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = start_battle_with_a_wild_program(&mut game);
    let player = game.player_entity();
    assert_eq!(
        game.world.resource::<BattleState>().player,
        player,
        "the battle must be fought by the entity the map panel reads"
    );

    // Outlast the pack without killing it: a fight that ends mid-loop
    // would drop the battle view and stop comparing.
    {
        let mut w = game.world.get_mut::<Stats>(wild).unwrap();
        w.hp = 10_000;
        w.max_hp = 10_000;
        w.atk = 50;
    }
    {
        let mut p = game.world.get_mut::<Stats>(player).unwrap();
        p.hp = 5_000;
        p.max_hp = 5_000;
    }

    let start_hp = game.player_status().hp;
    for round in 0..10 {
        player_attacks(&mut game);
        let status = game.player_status();
        let view = game
            .battle_view()
            .unwrap_or_else(|| panic!("battle ended early at round {round}"));
        let player_row = &view.party[0];
        assert_eq!(player_row.hp, status.hp, "hp diverged at round {round}");
        assert_eq!(
            player_row.max_hp, status.max_hp,
            "max_hp diverged at round {round}"
        );
    }
    assert!(
        game.player_status().hp < start_hp,
        "the wild program never landed a hit, so the comparison proved nothing"
    );
}

#[test]
fn wait_advances_one_tick_without_moving() {
    let mut game = Game::new(16, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pos_before = *game.world.get::<Position>(player).unwrap();
    let tick_before = game.world.resource::<GameClock>().tick;

    game.wait();

    let pos_after = *game.world.get::<Position>(player).unwrap();
    let tick_after = game.world.resource::<GameClock>().tick;
    assert_eq!(pos_after, pos_before, "waiting shouldn't move the player");
    assert_eq!(
        tick_after,
        tick_before + 1,
        "waiting should advance exactly one tick"
    );
}

#[test]
fn current_tick_matches_the_internal_game_clock() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert_eq!(
        game.current_tick(),
        0,
        "a fresh game should start at tick 0"
    );

    game.wait();
    game.wait();

    assert_eq!(
        game.current_tick(),
        2,
        "current_tick should track GameClock exactly"
    );
}

#[test]
fn idle_tick_advances_the_clock_outside_battle_but_not_during_one() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    game.idle_tick();
    assert_eq!(
        game.current_tick(),
        1,
        "idle_tick should advance the clock with no battle active"
    );

    let player = game.player_entity();
    let wild = game.spawn_wild_creature("glitch", 5, 5).unwrap();
    insert_battle(&mut game, player, vec![wild]);
    game.idle_tick();
    assert_eq!(
        game.current_tick(),
        1,
        "idle_tick should be a no-op while a battle is active"
    );
}

#[test]
fn rest_fully_heals_and_restores_fatigue() {
    let mut game = Game::new(18, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.hp = 1;
    }
    {
        let mut needs = game.world.get_mut::<Needs>(player).unwrap();
        needs.fatigue = 10.0;
    }
    spawn_rest_structure_at_player(&mut game);

    game.rest();

    let stats = *game.world.get::<Stats>(player).unwrap();
    let needs = *game.world.get::<Needs>(player).unwrap();
    assert_eq!(stats.hp, stats.max_hp, "rest should fully heal Integrity");
    assert_eq!(needs.fatigue, 100.0, "rest should fully restore Fatigue");
}

#[test]
fn rest_also_fully_heals_the_active_companion() {
    let mut game = Game::new(29, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    {
        let mut stats = game.world.get_mut::<Stats>(companion).unwrap();
        stats.hp = 1;
    }
    spawn_rest_structure_at_player(&mut game);

    game.rest();

    let stats = *game.world.get::<Stats>(companion).unwrap();
    assert_eq!(
        stats.hp, stats.max_hp,
        "rest should fully heal the active companion too"
    );
}

#[test]
fn rest_heals_every_party_member() {
    let mut game = Game::new(74, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let a = spawn_tamed(&mut game, 10, 3);
    let b = spawn_tamed(&mut game, 10, 3);
    game.add_companion(a).unwrap();
    game.add_companion(b).unwrap();
    for e in [a, b] {
        game.world.get_mut::<Stats>(e).unwrap().hp = 1;
    }
    spawn_rest_structure_at_player(&mut game);

    game.rest();

    assert_eq!(game.world.get::<Stats>(a).unwrap().hp, 10);
    assert_eq!(game.world.get::<Stats>(b).unwrap().hp, 10);
}

#[test]
fn a_new_game_starts_with_two_power_outlets() {
    let game = Game::new(701, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let held = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::OUTLET));
    assert_eq!(
        held, 2,
        "the bounded-income opening softener is two outlets, beside the \
         3 ICE Breakers / 3 Power Cells / 5 Core Fragments"
    );
}

#[test]
fn rest_is_refused_with_no_outlet_and_does_not_tick() {
    let mut game = Game::new(702, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    spawn_rest_structure_at_player(&mut game);
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::OUTLET));
        inv.take(ItemId::from(ids::OUTLET), held);
    }
    let before = game.current_tick();

    game.rest();

    assert_eq!(
        before,
        game.current_tick(),
        "a rest refused for lacking an outlet must not advance the clock — \
         the ticks are what the outlet buys"
    );
    assert!(
        game.message_log(5)
            .iter()
            .any(|(_, line)| line.to_lowercase().contains("outlet")),
        "the refusal should say why"
    );
}

#[test]
fn rest_spends_exactly_one_outlet_not_the_whole_stack() {
    let mut game = Game::new(703, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    spawn_rest_structure_at_player(&mut game);

    game.rest();

    let remaining = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::OUTLET));
    assert_eq!(
        remaining, 1,
        "a fresh game starts with 2 outlets; one rest should leave exactly 1, not 0"
    );
}

#[test]
fn rest_refused_by_game_over_consumes_no_outlet() {
    let mut game = Game::new(704, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    spawn_rest_structure_at_player(&mut game);
    game.world.resource_mut::<GameOver>().reason = Some("test".to_string());

    game.rest();

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        2,
        "a rest refused by the game-over gate must spend nothing"
    );
}

#[test]
fn rest_refused_by_active_battle_consumes_no_outlet() {
    let mut game = Game::new(705, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    spawn_rest_structure_at_player(&mut game);
    start_battle_with_a_wild_program(&mut game);

    game.rest();

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        2,
        "a rest refused by the active-battle gate must spend nothing"
    );
}

#[test]
fn rest_refused_by_no_nearby_rest_structure_consumes_no_outlet() {
    let mut game = Game::new(706, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    game.rest();

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::OUTLET)),
        2,
        "a rest refused for having no rest structure in range must spend nothing"
    );
}

const FREE_REST_PAD: &str = r#"(
    id: "free_rest_pad",
    name: "Free Rest Pad",
    description: "Test fixture: a rest structure whose RestDef carries no cost.",
    glyph: '#',
    color: White,
    build_cost: [],
    work: None,
    enables_rest: Some((radius: 7)),
)"#;

/// Guards the wiring, not just the parsing: `structures::tests::a_rest_def_-
/// without_a_cost_field_defaults_to_a_free_rest` (Task 1) already proves an
/// old-format `RestDef` parses with an empty `cost`; this proves `Game::rest`
/// actually treats that empty `cost` as free rather than, say, silently
/// requiring some other implicit price.
#[test]
fn a_rest_structure_with_no_cost_field_still_rests_for_free() {
    let dir = assets_dir_with_extra_structure("free_rest_pad", "free_rest_pad.ron", FREE_REST_PAD);
    let mut game = Game::new(707, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let player = game.player_entity();
    {
        let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
        let held = inv.count(&ItemId::from(ids::OUTLET));
        inv.take(ItemId::from(ids::OUTLET), held);
    }
    let pos = *game.world.get::<Position>(player).unwrap();
    game.world.spawn((
        Structure {
            kind: "free_rest_pad".to_string(),
        },
        Position { x: pos.x, y: pos.y },
    ));
    game.world.get_mut::<Needs>(player).unwrap().fatigue = 10.0;

    game.rest();

    assert_eq!(
        game.world.get::<Needs>(player).unwrap().fatigue,
        100.0,
        "a rest structure whose def sets no cost should still rest for free, \
         with zero outlets held"
    );
}

#[test]
fn home_enables_rest_across_the_whole_base_footprint() {
    let game = Game::new(402, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.id == "home")
        .expect("home.ron should load");
    assert_eq!(
        def.enables_rest
            .as_ref()
            .expect("Home should be the rest gate")
            .radius,
        MAX_BUILD_DISTANCE_FROM_HOME,
        "Home's rest radius should cover exactly the base footprint"
    );
}

#[test]
fn rest_is_a_no_op_without_a_nearby_rest_structure() {
    let mut game = Game::new(401, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    {
        let mut needs = game.world.get_mut::<Needs>(player).unwrap();
        needs.fatigue = 10.0;
    }

    game.rest();

    let needs = *game.world.get::<Needs>(player).unwrap();
    assert_eq!(
        needs.fatigue, 10.0,
        "resting with no Home in range shouldn't restore anything"
    );
}

#[test]
fn tick_field_buffs_decrements_and_expires_after_the_exact_tick_count() {
    let mut game = Game::new(600, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Def,
            name: "Test Shield".to_string(),
            power: 2,
            remaining: 5,
            source: BuffSource::Routine,
        },
    );

    for _ in 0..4 {
        game.tick_field_buffs();
    }
    assert_eq!(
        game.world.get::<FieldBuff>(player).unwrap().active.len(),
        1,
        "a 5-tick buff should still be running after only 4 ticks"
    );

    game.tick_field_buffs();
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .is_empty(),
        "a 5-tick buff should be gone after the 5th tick"
    );
}

#[test]
fn tick_field_buffs_logs_the_armed_name_not_the_kind_on_expiry() {
    let mut game = Game::new(601, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::CaptureBoost,
            name: "Snare Protocol".to_string(),
            power: 10,
            remaining: 1,
            source: BuffSource::Consumable,
        },
    );

    game.tick_field_buffs();

    let log = game.message_log(10);
    assert!(
        log.iter().any(|(_, line)| line.contains("Snare Protocol")),
        "the expiry line should name the armed buff, not its kind: {log:?}"
    );
}

#[test]
fn tick_field_buffs_ages_buffs_on_party_members_too() {
    let mut game = Game::new(602, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Atk,
            name: "Overclock".to_string(),
            power: 3,
            remaining: 2,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();

    let remaining = game
        .world
        .get::<FieldBuff>(companion)
        .unwrap()
        .active
        .first()
        .unwrap()
        .remaining;
    assert_eq!(remaining, 1, "a companion's field buff should tick too");
}

/// The regression guard for the ordering constraint in `tick_inner`: a
/// field buff keeps aging through `rest` while a `Temporary` structure's
/// lifespan does not. Losing this distinction is exactly the kind of
/// "tidy up the call site" refactor that would silently make buffs
/// immortal through every night's rest.
#[test]
fn rest_ages_field_buffs_but_not_temporary_structures() {
    let mut game = Game::new(603, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    spawn_rest_structure_at_player(&mut game);
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Coolant,
            name: "Heat Sink".to_string(),
            power: 1,
            remaining: REST_TICKS + 5,
            source: BuffSource::Routine,
        },
    );
    let pos = *game.world.get::<Position>(player).unwrap();
    let structure = game
        .world
        .spawn((
            Structure {
                kind: "test_temp".to_string(),
            },
            Position {
                x: pos.x + 2,
                y: pos.y,
            },
            Temporary {
                ticks_remaining: 100,
            },
        ))
        .id();

    game.rest();

    let buff_remaining = game
        .world
        .get::<FieldBuff>(player)
        .unwrap()
        .active
        .first()
        .unwrap()
        .remaining;
    assert_eq!(
        buff_remaining, 5,
        "a field buff should lose exactly REST_TICKS while resting"
    );
    assert_eq!(
        game.world
            .get::<Temporary>(structure)
            .unwrap()
            .ticks_remaining,
        100,
        "a Temporary structure must not age while the player rests"
    );
}

#[test]
fn tick_field_buffs_regen_heals_the_carrier_and_caps_at_max_hp() {
    let mut game = Game::new(610, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let max_hp = game.world.get::<Stats>(player).unwrap().max_hp;
    game.world.get_mut::<Stats>(player).unwrap().hp = max_hp - 10;
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Nanite Patch".to_string(),
            power: 4,
            remaining: 5,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        max_hp - 6,
        "Regen should heal by exactly its power on a tick that doesn't hit the cap"
    );

    // Two more ticks at +4 each would land at max_hp - 6 + 8 = max_hp + 2
    // if uncapped — proves the clamp, not just that healing happened.
    game.tick_field_buffs();
    game.tick_field_buffs();
    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        max_hp,
        "Regen must not heal past max_hp"
    );
}

#[test]
fn tick_field_buffs_regen_heals_a_companion_not_the_player() {
    let mut game = Game::new(611, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    game.world.get_mut::<Stats>(companion).unwrap().hp = 4;
    let player_hp_before = game.world.get::<Stats>(game.player_entity()).unwrap().hp;
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Self Repair".to_string(),
            power: 3,
            remaining: 5,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();

    assert_eq!(
        game.world.get::<Stats>(companion).unwrap().hp,
        7,
        "the companion carrying Regen should heal itself"
    );
    assert_eq!(
        game.world.get::<Stats>(game.player_entity()).unwrap().hp,
        player_hp_before,
        "Regen on a companion must not heal the player"
    );
}

/// `tick_field_buffs` runs on every `tick()`, including every battle round,
/// and `Party` deliberately keeps a dead member around until `end_battle`
/// reaps it — so a `Regen` with no floor check would heal a companion
/// killed mid-battle back to positive HP on the very next tick. This repo
/// shipped permadeath; an accidental auto-revive would silently undo it.
#[test]
fn tick_field_buffs_regen_does_not_revive_a_dead_companion() {
    let mut game = Game::new(614, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Self Repair".to_string(),
            power: 5,
            remaining: 10,
            source: BuffSource::Routine,
        },
    );
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.world.get_mut::<Stats>(companion).unwrap().hp = -3;

    game.tick_field_buffs();

    assert_eq!(
        game.world.get::<Stats>(companion).unwrap().hp,
        -3,
        "a dead companion's HP must not move on a Regen tick"
    );
    assert!(
        !game.creature_alive(companion),
        "the companion must still read as dead"
    );

    game.end_battle(player, None);
    assert!(
        game.world.get::<Stats>(companion).is_none(),
        "end_battle must still reap the dead companion; a running Regen must not save it"
    );
}

#[test]
fn a_full_tick_applies_coolant_and_trickle_on_top_of_that_ticks_decay() {
    let mut game = Game::new(612, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let (hunger_before, fatigue_before) = {
        let mut needs = game.world.get_mut::<Needs>(player).unwrap();
        needs.hunger = 90.0;
        needs.fatigue = 90.0;
        (needs.hunger, needs.fatigue)
    };
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Coolant,
            name: "Heat Sink".to_string(),
            power: 15,
            remaining: 5,
            source: BuffSource::Routine,
        },
    );
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Trickle,
            name: "Power Tap".to_string(),
            power: 15,
            remaining: 5,
            source: BuffSource::Routine,
        },
    );

    game.wait();

    // `needs_tick_system` runs inside the same tick's schedule, ahead of
    // `tick_field_buffs` (see `tick_inner`), so the restore lands on top of
    // whatever that tick's own movement was — hunger's drain, fatigue's
    // regen. Read through the live formula instead of restating its
    // constants.
    let (ticked_hunger, ticked_fatigue) =
        crate::systems::tick_needs(hunger_before, fatigue_before, 1.0);
    let needs = *game.world.get::<Needs>(player).unwrap();
    assert_eq!(
        needs.fatigue,
        (ticked_fatigue + 15.0).min(NEED_MAX),
        "Coolant should restore fatigue on top of the tick's own regen"
    );
    assert_eq!(
        needs.hunger,
        (ticked_hunger + 15.0).min(NEED_MAX),
        "Trickle should restore hunger the same way"
    );
    assert_eq!(
        needs.fatigue, NEED_MAX,
        "90 + 15 minus a hair of decay should still clamp to the cap"
    );
    assert_eq!(needs.hunger, NEED_MAX, "same clamp for hunger");
}

#[test]
fn tick_field_buffs_applies_a_buffs_last_tick_before_it_expires() {
    let mut game = Game::new(613, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let max_hp = game.world.get::<Stats>(player).unwrap().max_hp;
    game.world.get_mut::<Stats>(player).unwrap().hp = max_hp - 5;
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Regen,
            name: "Last Gasp".to_string(),
            power: 3,
            remaining: 1,
            source: BuffSource::Routine,
        },
    );

    game.tick_field_buffs();

    assert_eq!(
        game.world.get::<Stats>(player).unwrap().hp,
        max_hp - 2,
        "a buff with 1 tick left must still apply its effect before it expires"
    );
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .is_empty(),
        "the buff should be gone after its last tick"
    );
}

/// `ActiveFieldBuff`/`FieldBuffKind`/`BuffSource` all need to survive a
/// save/load round trip intact, on both the player and a party member — not
/// just the count, but every field, since a save that silently dropped
/// `power` or `source` would still pass a length check.
#[test]
fn field_buffs_survive_a_save_load_round_trip() {
    let mut game = Game::new(604, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 10, 3);
    game.add_companion(companion).unwrap();

    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::CaptureBoost,
            name: "Snare Protocol".to_string(),
            power: 15,
            remaining: 7,
            source: BuffSource::Consumable,
        },
    );
    game.arm_field_buff(
        companion,
        ActiveFieldBuff {
            kind: FieldBuffKind::Atk,
            name: "Overclock".to_string(),
            power: 4,
            remaining: 3,
            source: BuffSource::Routine,
        },
    );

    let path = std::env::temp_dir().join(format!(
        "feral_field_buffs_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let player_buff = loaded
        .world
        .get::<FieldBuff>(loaded.player_entity())
        .unwrap()
        .active
        .first()
        .cloned()
        .expect("the player's field buff should survive the round trip");
    assert_eq!(player_buff.kind, FieldBuffKind::CaptureBoost);
    assert_eq!(player_buff.name, "Snare Protocol");
    assert_eq!(player_buff.power, 15);
    assert_eq!(player_buff.remaining, 7);
    assert_eq!(player_buff.source, BuffSource::Consumable);

    let companion_buff = loaded
        .owned_pets()
        .first()
        .and_then(|p| loaded.world.get::<FieldBuff>(p.entity).cloned())
        .and_then(|f| f.active.first().cloned())
        .expect("the companion's field buff should survive the round trip");
    assert_eq!(companion_buff.kind, FieldBuffKind::Atk);
    assert_eq!(companion_buff.name, "Overclock");
    assert_eq!(companion_buff.power, 4);
    assert_eq!(companion_buff.remaining, 3);
    assert_eq!(companion_buff.source, BuffSource::Routine);
}

#[test]
fn forage_chance_applies_keen_scavenger_per_level_but_never_boosts_a_zero_chance_biome() {
    assert_eq!(forage_chance(Biome::OpenGrid, 0), 0.6);
    assert_eq!(
        forage_chance(Biome::OpenGrid, 1),
        0.6 + KEEN_SCAVENGER_BONUS_PER_LEVEL
    );
    assert_eq!(
        forage_chance(Biome::OpenGrid, 3),
        0.6 + KEEN_SCAVENGER_BONUS_PER_LEVEL * 3.0
    );
    assert_eq!(
        forage_chance(Biome::DataVoid, 1),
        0.0,
        "an unwalkable biome's 0% chance shouldn't be boosted into a nonzero one"
    );
    assert_eq!(
        forage_chance(Biome::Platform, 3),
        0.0,
        "a base platform is manufactured floor with nothing to scavenge, and no amount \
         of Keen Scavenger should turn a safe haven into a risk-free forage spot"
    );
}

#[test]
fn use_item_applies_a_power_restore_and_consumes_one() {
    let mut game = Game::new(500, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
    // The player already starts holding Power Cells (see `Game::new`);
    // drain the default stock first so the stack is exactly 2 below.
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let held = inv.count(&ItemId::from(ids::POWER_CELL));
    inv.take(ItemId::from(ids::POWER_CELL), held);
    inv.add(ItemId::from(ids::POWER_CELL), 2);

    game.use_item(&ItemId::from(ids::POWER_CELL));

    // `use_item` ends with `self.tick()` like every other player action,
    // so `needs_tick_system` also shaves off one tick's worth of hunger
    // (see `HUNGER_DECAY_PER_TICK` in systems.rs) on top of the +25
    // restore — same shared-decay caveat documented on
    // `commanding_a_companion_in_battle_costs_more_fatigue_than_a_stunned_one`.
    assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, 75.0 - 0.15);
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::POWER_CELL)),
        1
    );
}

#[test]
fn use_item_clamps_power_at_full() {
    let mut game = Game::new(501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Needs>(player).unwrap().hunger = 90.0;
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::POWER_CELL), 1);

    game.use_item(&ItemId::from(ids::POWER_CELL));

    // 90 + 25 clamps to 100 before the trailing tick's decay shaves off
    // 0.15 (see the comment in the test above) — had the clamp not
    // engaged, this would read 114.85 instead.
    assert_eq!(
        game.world.get::<Needs>(player).unwrap().hunger,
        100.0 - 0.15
    );
}

#[test]
fn use_item_rejects_a_non_consumable() {
    let mut game = Game::new(502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // The player already starts holding Core Fragments (see
    // `Game::new`), so compare against a captured baseline rather than
    // an absolute count.
    let before = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::CORE_FRAGMENT));
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 3);

    game.use_item(&ItemId::from(ids::CORE_FRAGMENT));

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        before + 3,
        "a non-consumable must not be consumed"
    );
}

#[test]
fn use_item_on_an_empty_stack_is_a_no_op() {
    let mut game = Game::new(503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // The player starts holding Power Cells (see `Game::new`), so drain
    // the stack to actually exercise the empty-stack path.
    let held = game
        .world
        .get::<Inventory>(player)
        .unwrap()
        .count(&ItemId::from(ids::POWER_CELL));
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .take(ItemId::from(ids::POWER_CELL), held);
    let before = game.world.get::<Needs>(player).unwrap().hunger;

    game.use_item(&ItemId::from(ids::POWER_CELL));

    assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, before);
}

#[test]
fn a_prebattle_buff_armed_on_the_map_is_live_at_the_next_intrusion() {
    let mut game = Game::new(504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Arm an Atk buff directly — models a companion's Rally/Shield left
    // active going into a fight, `CombatBuff`'s own reason to exist. A
    // pre-battle consumable no longer arms this component; see
    // `arm_field_buff` below for what it arms instead.
    game.world.get_mut::<CombatBuff>(player).unwrap().active = Some(ActiveBuff {
        kind: BuffKind::Atk,
        remaining: 3,
        power: 5,
    });
    // And a field buff, modeling what a prebattle_buff consumable arms
    // now (see `Game::arm_field_buff`) — it must carry into the fight the
    // same way a `CombatBuff` does.
    game.arm_field_buff(
        player,
        ActiveFieldBuff {
            kind: FieldBuffKind::Atk,
            name: "Test Stim".to_string(),
            power: 5,
            remaining: 5,
            source: BuffSource::Consumable,
        },
    );

    let wild = spawn_wild_on_player_tile(&mut game);
    game.start_battle(vec![wild]);

    let buff = game.world.get::<CombatBuff>(player).unwrap().active;
    assert!(
        matches!(
            buff,
            Some(ActiveBuff {
                kind: BuffKind::Atk,
                power: 5,
                ..
            })
        ),
        "a buff armed before the fight must still be active when it starts"
    );
    assert_eq!(
        game.field_buff_power(player, FieldBuffKind::Atk),
        5,
        "a field buff armed before the fight must also still be active when it starts"
    );
}

/// One `.ron` item, shared by the two reproducers below, declaring a
/// `prebattle_buff` — no shipped item declares one, so a fixture is the
/// only way to drive `use_item`'s real code path.
const TEST_STIM_ITEM: &str = r#"(
    id: "test_stim",
    name: "Test Stim",
    consume: Some((
        prebattle_buff: Some((kind: Atk, power: 5, ticks: 5)),
    )),
)"#;

/// Bug 1: `clear_battle_status_effects` used to null the player's
/// `CombatBuff` unconditionally whenever a battle ended — correct for a
/// companion's Rally, but the pre-battle item buff was living in that same
/// component, so a 5-round stim was destroyed by a battle that ended after
/// 1 round, 4 rounds still on the clock. It now arms `FieldBuff`, which
/// battle end never touches.
#[test]
fn a_prebattle_buff_survives_the_battle_it_was_armed_for() {
    let dir = modded_assets_dir(
        "prebattle_stim_survives_battle",
        &[],
        &[("test_stim.ron", TEST_STIM_ITEM)],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(9101, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    let player = game.player_entity();
    let stim = ItemId::from("test_stim");
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(stim.clone(), 1);

    game.use_item(&stim);
    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .iter()
            .any(|b| b.kind == FieldBuffKind::Atk && b.power == 5),
        "arming a prebattle_buff item should land it on FieldBuff, not the \
         battle-scoped CombatBuff"
    );

    start_battle_with_a_wild_program(&mut game);
    game.end_battle(player, None);

    assert!(
        game.world
            .get::<FieldBuff>(player)
            .unwrap()
            .active
            .iter()
            .any(|b| b.kind == FieldBuffKind::Atk && b.power == 5),
        "a field buff must still be running once the battle it was armed for ends"
    );
}

/// Bug 2: before `FieldBuff` existed there was nowhere to put a map-armed
/// item buff in `PlayerSave`, so it vanished on the round trip. Arming it
/// now writes `FieldBuff`, which `PlayerSave` already persists.
#[test]
fn a_prebattle_buff_survives_a_save_load_round_trip() {
    let dir = modded_assets_dir(
        "prebattle_stim_survives_save",
        &[],
        &[("test_stim.ron", TEST_STIM_ITEM)],
        &[],
        &[],
        &[],
    );
    let mut game = Game::new(9102, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    let stim = ItemId::from("test_stim");
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(stim.clone(), 1);
    game.use_item(&stim);

    let path = std::env::temp_dir().join(format!(
        "feral_prebattle_buff_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &dir).unwrap();
    let _ = std::fs::remove_file(&path);
    let _ = std::fs::remove_dir_all(&dir);

    let buff = loaded
        .world
        .get::<FieldBuff>(loaded.player_entity())
        .unwrap()
        .active
        .first()
        .cloned();
    assert!(
        matches!(
            buff,
            Some(ActiveFieldBuff {
                kind: FieldBuffKind::Atk,
                power: 5,
                ..
            })
        ),
        "a prebattle buff must survive a save/load round trip: {buff:?}"
    );
}

#[test]
fn use_power_source_restores_power_and_consumes_one() {
    let mut game = Game::new(504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
    // The player already starts holding Power Cells (see `Game::new`);
    // drain the default stock first so the stack is exactly 2 below.
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let held = inv.count(&ItemId::from(ids::POWER_CELL));
    inv.take(ItemId::from(ids::POWER_CELL), held);
    inv.add(ItemId::from(ids::POWER_CELL), 2);

    game.use_power_source();

    // `use_power_source` dispatches to `use_item`, which ends with
    // `self.tick()` like every other player action, so
    // `needs_tick_system` also shaves off one tick's worth of hunger
    // (see `HUNGER_DECAY_PER_TICK` in systems.rs) on top of the +25
    // restore — same shared-decay caveat as `use_item_applies_a_power_
    // restore_and_consumes_one` above.
    assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, 75.0 - 0.15);
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::POWER_CELL)),
        1
    );
}

#[test]
fn use_power_source_with_nothing_to_recharge_from_is_a_no_op() {
    let mut game = Game::new(505, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    // Drain the default Power Cell stock (see `Game::new`) so no
    // power-restoring item remains; the Core Fragments the player also
    // starts with have no `consume` effect at all.
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let held = inv.count(&ItemId::from(ids::POWER_CELL));
    inv.take(ItemId::from(ids::POWER_CELL), held);
    let fragments_before = inv.count(&ItemId::from(ids::CORE_FRAGMENT));
    let hunger_before = game.world.get::<Needs>(player).unwrap().hunger;

    game.use_power_source();

    // No candidate item means no `use_item` dispatch, so unlike the
    // success path above there's no trailing `tick()` and hunger must
    // be untouched, not merely undecayed.
    assert_eq!(
        game.world.get::<Needs>(player).unwrap().hunger,
        hunger_before,
        "a failed recharge must not tick the game or touch Needs"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        fragments_before,
        "a failed recharge must not consume an unrelated item"
    );
    assert!(
        game.message_log(10)
            .iter()
            .any(|(_, line)| line == "You have nothing to recharge from."),
        "expected the no-power-source message, got: {:?}",
        game.message_log(10)
    );
}

#[test]
fn use_power_source_picks_the_power_item_over_an_earlier_non_power_item() {
    let mut game = Game::new(506, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Needs>(player).unwrap().hunger = 50.0;
    // Drain all four starting stacks (see `Game::new`: Ice Breaker, Power
    // Cell, Core Fragment, Power Outlet) and rebuild the inventory with the
    // non-power item (Core Fragment) added *first*, so it's ahead of
    // the Power Cell in `Inventory::items`. This pins selection to the
    // `ConsumeDef.power > 0.0` predicate rather than to iteration
    // order or to which `ItemId` happens to be checked first.
    let mut inv = game.world.get_mut::<Inventory>(player).unwrap();
    let ice_breaker_held = inv.count(&ItemId::from(ids::ICE_BREAKER));
    inv.take(ItemId::from(ids::ICE_BREAKER), ice_breaker_held);
    let power_held = inv.count(&ItemId::from(ids::POWER_CELL));
    inv.take(ItemId::from(ids::POWER_CELL), power_held);
    let fragments_held = inv.count(&ItemId::from(ids::CORE_FRAGMENT));
    inv.take(ItemId::from(ids::CORE_FRAGMENT), fragments_held);
    let outlets_held = inv.count(&ItemId::from(ids::OUTLET));
    inv.take(ItemId::from(ids::OUTLET), outlets_held);
    inv.add(ItemId::from(ids::CORE_FRAGMENT), 5);
    inv.add(ItemId::from(ids::POWER_CELL), 2);
    assert_eq!(
        inv.items[0].0,
        ItemId::from(ids::CORE_FRAGMENT),
        "test setup: the non-power item must be first in iteration order"
    );

    game.use_power_source();

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::POWER_CELL)),
        1,
        "the power-restoring item should have been the one consumed"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        5,
        "the earlier non-power item must be left untouched"
    );
    assert_eq!(game.world.get::<Needs>(player).unwrap().hunger, 75.0 - 0.15);
}

/// Wipes every wild program off the map. The ambush tests use this to make
/// a battle unambiguous: with nothing left to walk into, the bump path in
/// `move_player` cannot fire, so any fight that opens came from the ambush
/// roll.
fn despawn_every_hostile(game: &mut Game) {
    let hostiles: Vec<Entity> = {
        let mut query = game.world.query_filtered::<Entity, With<Hostile>>();
        query.iter(&game.world).collect()
    };
    for entity in hostiles {
        game.world.despawn(entity);
    }
}

#[test]
fn an_ambush_engages_a_pack_immediately() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    despawn_every_hostile(&mut game);

    for _ in 0..2000 {
        game.maybe_ambush();
        if game.has_active_battle() {
            let enemies = game.all_living_enemies();
            assert!(
                !enemies.is_empty(),
                "an ambush that opens a battle must put something in it"
            );
            return;
        }
    }
    panic!("2000 ambush rolls never fired — RANDOM_ENCOUNTER_CHANCE may be broken");
}

/// Bosses are something you find and choose to fight. One that jumps you
/// with no chance to decline is a death sentence you never opted into.
#[test]
fn an_ambush_never_fields_a_boss() {
    let mut ambushes = 0;
    for seed in 0..40 {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        despawn_every_hostile(&mut game);
        for _ in 0..400 {
            game.maybe_ambush();
            if !game.has_active_battle() {
                continue;
            }
            ambushes += 1;
            for enemy in game.all_living_enemies() {
                let species = game.world.get::<Creature>(enemy).unwrap().species.clone();
                let def = game.world.resource::<SpeciesDb>().get(&species).cloned();
                assert!(
                    !def.expect("an ambushed program's species must be loaded")
                        .is_boss,
                    "an ambush must never field a boss"
                );
            }
            despawn_every_hostile(&mut game);
            game.world.remove_resource::<BattleState>();
        }
    }
    // Without this the whole sweep passes vacuously if ambushes stop
    // firing — the assertion above only runs inside a battle.
    assert!(
        ambushes > 100,
        "the sweep only fired {ambushes} ambushes; it is not exercising the boss check"
    );
}

/// The base platform is a manufactured floor, not terrain. Nothing spawns
/// on it and nothing should jump you on it — it is the one safe ground.
#[test]
fn no_ambush_fires_on_the_base_platform() {
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game, 0, 0);
    despawn_every_hostile(&mut game);

    for _ in 0..2000 {
        game.maybe_ambush();
        assert!(
            !game.has_active_battle(),
            "the base platform must never be ambushed"
        );
    }
}

/// The integration the ambush actually ships as: a walked step can open a
/// fight. Every hostile is cleared immediately before each move, so the
/// bump path in `move_player` has nothing to trigger on and any battle that
/// appears is attributable to the ambush roll alone.
#[test]
fn walking_open_ground_can_be_ambushed() {
    let mut game = Game::new(3, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    for step in 0..1000 {
        despawn_every_hostile(&mut game);
        let dx = if step % 2 == 0 { 1 } else { -1 };
        game.move_player(dx, 0);
        if game.has_active_battle() {
            return;
        }
        if game.is_game_over().is_some() {
            panic!("the player died before any ambush fired");
        }
    }
    panic!("1000 walked steps never produced an ambush");
}
