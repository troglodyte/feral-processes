//! Perk unlocks and the effects each perk level has.

use super::support::*;
use crate::tuning::{
    ATTACKER_BONUS_PER_LEVEL, BUFFER_MIN_BONUS_PER_LEVEL, DECOMPILER_SKILL_PER_LEVEL,
    DEFENDER_BONUS_PER_LEVEL, LEAN_COMPILER_DISCOUNT_PER_LEVEL,
};
use crate::*;

fn perk_cost(game: &Game, perk: Perk) -> u32 {
    game.perk_defs()
        .into_iter()
        .find(|d| d.id == perk)
        .unwrap_or_else(|| panic!("{perk:?} should be on offer"))
        .cost
}

#[test]
fn player_decompiler_skill_grows_on_level_up_and_survives_save_load() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();

    assert_eq!(
        game.player_status().decompiler,
        0,
        "should start with no decompiler skill"
    );

    game.award_player_xp(player, 20);
    assert_eq!(
        game.player_status().level,
        2,
        "20 xp should be enough to reach level 2"
    );
    assert_eq!(
        game.player_status().decompiler,
        DECOMPILER_SKILL_PER_LEVEL,
        "one level gained should grant one point of decompiler skill"
    );

    let path = std::env::temp_dir().join(format!(
        "feral_processes_decompiler_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.player_status().decompiler,
        DECOMPILER_SKILL_PER_LEVEL,
        "decompiler skill should survive a save/load round trip"
    );
}

#[test]
fn unlock_perk_spends_points_and_can_be_bought_repeatedly() {
    let mut game = Game::new(110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 5;

    let cost = perk_cost(&game, Perk::KeenScavenger);
    game.unlock_perk(Perk::KeenScavenger).unwrap();

    let status = game.player_status();
    assert_eq!(status.perk_points, 5 - cost);
    assert_eq!(status.unlocked_perks, vec![Perk::KeenScavenger]);
    assert_eq!(game.player_perk_level(Perk::KeenScavenger), 1);

    game.unlock_perk(Perk::KeenScavenger).unwrap();
    assert_eq!(
        game.player_perk_level(Perk::KeenScavenger),
        2,
        "buying the same perk again should stack another level, not be rejected"
    );
    assert_eq!(status.perk_points - cost, game.player_status().perk_points);
}

#[test]
fn unlock_perk_rejects_without_enough_points() {
    let mut game = Game::new(111, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 0;

    assert!(game.unlock_perk(Perk::ExploitFocus).is_err());
    assert_eq!(game.player_perk_level(Perk::ExploitFocus), 0);
}

/// Spawns a wild program of the easiest species to decompile, at `hp` of
/// `max_hp` Integrity. The easiest species on purpose: a hard one at full
/// health sits close enough to `CAPTURE_CHANCE_MIN` that the clamp, not the
/// perk, would decide what these tests measure.
fn spawn_wild_at_hp(game: &mut Game, hp: i32, max_hp: i32) -> Entity {
    let species = game
        .species_defs()
        .into_iter()
        .min_by(|a, b| a.taming_difficulty.total_cmp(&b.taming_difficulty))
        .expect("at least one species");
    game.world
        .spawn((
            Creature {
                species: species.id.clone(),
            },
            Hostile,
            Position { x: 3, y: 3 },
            Stats {
                hp,
                max_hp,
                atk: 1,
                def: 1,
            },
        ))
        .id()
}

#[test]
fn exploit_focus_raises_decompile_odds_against_a_healthy_target_per_level() {
    let mut game = Game::new(112, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = spawn_wild_at_hp(&mut game, 10, 10);

    let before = program_manifest(&game, wild).decompile_chance;

    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    game.unlock_perk(Perk::ExploitFocus).unwrap();
    let after_one = program_manifest(&game, wild).decompile_chance;
    game.unlock_perk(Perk::ExploitFocus).unwrap();
    let after_two = program_manifest(&game, wild).decompile_chance;

    assert!(
        after_one > before,
        "Exploit Focus should raise the decompile chance shown for a full-Integrity target"
    );
    assert!(
        after_two > after_one,
        "a second level of Exploit Focus should raise it further still"
    );
}

/// What makes the perk worth its cost alongside the `Decompiler` stat that
/// levelling already grants for free: it buys attempts on programs you
/// haven't worn down, so its value falls away as the target does. A change
/// that turned it back into a flat bonus would fail here while still passing
/// the test above.
#[test]
fn exploit_focus_is_worth_far_less_against_an_already_drained_target() {
    let mut game = Game::new(119, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let healthy = spawn_wild_at_hp(&mut game, 10, 10);
    let drained = spawn_wild_at_hp(&mut game, 1, 10);

    let healthy_before = program_manifest(&game, healthy).decompile_chance.unwrap();
    let drained_before = program_manifest(&game, drained).decompile_chance.unwrap();

    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    game.unlock_perk(Perk::ExploitFocus).unwrap();

    let healthy_gain = program_manifest(&game, healthy).decompile_chance.unwrap() - healthy_before;
    let drained_gain = program_manifest(&game, drained).decompile_chance.unwrap() - drained_before;

    assert!(
        healthy_gain > drained_gain * 2.0,
        "the perk should pay off mainly on targets that still have their Integrity: \
         {healthy_gain} vs {drained_gain}"
    );
}

#[test]
fn lean_compiler_discounts_craft_cost_per_level_but_never_below_one_each() {
    let mut game = Game::new(113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let base_cost = game.craft_cost(&ItemId::from(ids::POWER_CELL));
    assert_eq!(
        base_cost,
        vec![(ItemId::from(ids::CORE_FRAGMENT), POWER_CELL_CORE_COST)]
    );

    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    game.unlock_perk(Perk::LeanCompiler).unwrap();
    let discounted = game.craft_cost(&ItemId::from(ids::POWER_CELL));
    assert_eq!(
        discounted,
        vec![(
            ItemId::from(ids::CORE_FRAGMENT),
            POWER_CELL_CORE_COST - LEAN_COMPILER_DISCOUNT_PER_LEVEL
        )]
    );

    for _ in 0..10 {
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        let _ = game.unlock_perk(Perk::LeanCompiler);
    }
    let floored = game.craft_cost(&ItemId::from(ids::POWER_CELL));
    assert_eq!(
        floored,
        vec![(ItemId::from(ids::CORE_FRAGMENT), 1)],
        "the discount should never drop the cost below 1"
    );
}

#[test]
fn perk_state_survives_save_and_load() {
    let assets = test_assets_dir();
    let mut game = Game::new(114, DifficultyMode::Forgiving, &assets).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    game.unlock_perk(Perk::LowPowerMode).unwrap();
    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    game.unlock_perk(Perk::LowPowerMode).unwrap();
    let points_after_unlock = game.player_status().perk_points;

    let path = std::env::temp_dir().join(format!(
        "feral_processes_perk_test_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &assets).unwrap();
    let _ = std::fs::remove_file(&path);

    let status = loaded.player_status();
    assert_eq!(status.perk_points, points_after_unlock);
    assert_eq!(
        status.unlocked_perks,
        vec![Perk::LowPowerMode, Perk::LowPowerMode]
    );
    assert_eq!(loaded.player_perk_level(Perk::LowPowerMode), 2);
}

#[test]
fn attacker_perk_adds_permanent_atk_per_level() {
    let mut game = Game::new(115, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    let base_atk = game.player_status().atk;

    game.unlock_perk(Perk::Attacker).unwrap();
    assert_eq!(
        game.player_status().atk,
        base_atk + ATTACKER_BONUS_PER_LEVEL
    );

    game.unlock_perk(Perk::Attacker).unwrap();
    assert_eq!(
        game.player_status().atk,
        base_atk + ATTACKER_BONUS_PER_LEVEL * 2
    );
}

#[test]
fn defender_perk_adds_permanent_def_per_level() {
    let mut game = Game::new(116, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    let base_def = game.player_status().def;

    game.unlock_perk(Perk::Defender).unwrap();
    assert_eq!(
        game.player_status().def,
        base_def + DEFENDER_BONUS_PER_LEVEL
    );
}

#[test]
fn buffer_perk_adds_percent_max_hp_per_level_floored_and_fully_heals() {
    let mut game = Game::new(117, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    let base_max_hp = game.player_status().max_hp;
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.hp = 1;
    }

    game.unlock_perk(Perk::Buffer).unwrap();
    let status = game.player_status();
    // 1% of the starting max HP rounds to well under the floor, so the
    // minimum bonus is what actually applies here.
    assert_eq!(status.max_hp, base_max_hp + BUFFER_MIN_BONUS_PER_LEVEL);
    assert_eq!(
        status.hp, status.max_hp,
        "buying Buffer should fully heal, like a level-up does"
    );
}

#[test]
fn buffer_perk_scales_past_the_floor_at_high_max_hp() {
    let mut game = Game::new(118, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    {
        let mut stats = game.world.get_mut::<Stats>(player).unwrap();
        stats.max_hp = 2000;
        stats.hp = 2000;
    }

    game.unlock_perk(Perk::Buffer).unwrap();
    let status = game.player_status();
    assert_eq!(
        status.max_hp, 2020,
        "1% of 2000 is 20, above the floor, so that's what should apply"
    );
}
