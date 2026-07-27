//! The turn loop: ticking, resting, waiting, and consuming items.

use super::support::*;
use crate::game::turn::forage_chance;
use crate::tuning::{KEEN_SCAVENGER_BONUS_PER_LEVEL, MAX_BUILD_DISTANCE_FROM_HOME};
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
    // so `needs_decay_system` also shaves off one tick's worth of hunger
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
    // Arm an Atk buff directly (models what a prebattle_buff consumable does).
    game.world.get_mut::<CombatBuff>(player).unwrap().active = Some(ActiveBuff {
        kind: BuffKind::Atk,
        remaining: 3,
        power: 5,
    });

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
    // `needs_decay_system` also shaves off one tick's worth of hunger
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
    // Drain all three starting stacks (see `Game::new`: Ice Breaker,
    // Power Cell, Core Fragment) and rebuild the inventory with the
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
