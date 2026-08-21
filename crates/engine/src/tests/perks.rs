//! Perk unlocks and the effects each perk level has.

use super::support::*;
use crate::abilities::AffinityKind;
use crate::tuning::{
    ATTACKER_BONUS_PER_LEVEL, BUFFER_MIN_BONUS_PER_LEVEL, DECOMPILER_SKILL_PER_LEVEL,
    DEFENDER_BONUS_PER_LEVEL, DIFFICULTY_EVEN_MAX, KEEN_SCAVENGER_BONUS_PER_LEVEL,
    LEAN_COMPILER_DISCOUNT_PER_LEVEL,
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

    game.award_player_xp(player, crate::progression::xp_for_level(1));
    assert_eq!(
        game.player_status().level,
        2,
        "a level's worth of xp should be enough to reach level 2"
    );
    assert_eq!(
        game.player_status().decompiler,
        DECOMPILER_SKILL_PER_LEVEL,
        "one level gained should grant one level's decompiler skill"
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
///
/// Its Attack and Defense are padded to put `Stats::power` at
/// `DIFFICULTY_EVEN_MAX` of the player's, for the same kind of reason.
/// Exploit Focus and `taming::power_relief` subtract from the same
/// `CAPTURE_HP_PENALTY`, and relief waives all of it at a Green-con gap — so
/// an unpadded 12-power dummy leaves the perk nothing to reduce and turns
/// both tests below into measurements of the power gap. Only `hp`/`max_hp`
/// are the callers' own; the padding exists solely to keep the target a
/// threat, which is the regime the perk is for.
fn spawn_wild_at_hp(game: &mut Game, hp: i32, max_hp: i32) -> Entity {
    let species = game
        .species_defs()
        .into_iter()
        .min_by(|a, b| a.taming_difficulty.total_cmp(&b.taming_difficulty))
        .expect("at least one species");
    let even_match = (game.player_power() as f64 * DIFFICULTY_EVEN_MAX).ceil() as i32;
    // All of the padding goes into `atk`, and none into mitigation. Under
    // `Stats::power` mitigation is priced as the effective HP it buys rather
    // than summed in, so it is the wrong knob for "make this thing's power
    // reach N" — past `MAX_MITIGATION_PERCENT` it is clamped and the power
    // saturates short of the threshold, leaving the target inside the relief
    // ramp this fixture exists to clear. With mitigation at 0 the sum is
    // exactly `max_hp + atk`.
    let padding = (even_match - max_hp).max(1);
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
                atk: padding,
                mitigation: 0,
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
    let base_cost = game.craft_cost(&ItemId::from(ids::POWER_CELL), false);
    assert_eq!(
        base_cost,
        vec![(ItemId::from(ids::CORE_FRAGMENT), POWER_CELL_CORE_COST)]
    );

    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    game.unlock_perk(Perk::LeanCompiler).unwrap();
    let discounted = game.craft_cost(&ItemId::from(ids::POWER_CELL), false);
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
    let floored = game.craft_cost(&ItemId::from(ids::POWER_CELL), false);
    assert_eq!(
        floored,
        vec![(ItemId::from(ids::CORE_FRAGMENT), 1)],
        "the discount should never drop the cost below 1"
    );
}

/// The Compile screen quotes a `CraftRecipe::cost` and `Game::craft` charges
/// `craft_cost`, so those two agreeing is the whole of the screen not naming
/// a price the game doesn't want. They didn't: a player holding
/// `LeanCompiler` was quoted the undiscounted recipe — "Core Fragment (2/3)"
/// over a compile that would have gone through at 2 — while the same screen's
/// "Max affordable" line, which reads `craft_cost`, said 1.
#[test]
fn a_quoted_recipe_cost_is_the_price_actually_charged() {
    let mut game = Game::new(113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    game.unlock_perk(Perk::LeanCompiler).unwrap();

    let recipes = game.craft_recipes();
    assert!(!recipes.is_empty(), "the test assets declare recipes");
    for recipe in recipes {
        assert_eq!(
            recipe.cost,
            game.craft_cost(&recipe.result, false),
            "{:?} is quoted at a different price from the one it charges",
            recipe.result
        );
    }
}

/// `LeanCompiler` is the player's bench discount and stops at the player's
/// bench. A machine runs its product's authored `craftable.cost` through
/// `systems::assembly_recipe`, which reads `ItemDb` directly — so moving the
/// discount into `craft_recipes` must not reach a structure's consumption or
/// the Recipes chains that report it.
///
/// Compares the chains against themselves across the purchase rather than
/// against a fixture: what is being asserted is that nothing moved, and a
/// hardcoded quantity would go stale the first time an asset was retuned.
#[test]
fn lean_compiler_does_not_discount_what_a_structure_consumes() {
    let mut game = Game::new(113, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let quantities = |g: &Game| -> Vec<Vec<u32>> {
        g.recipe_chains()
            .iter()
            .map(|c| {
                c.steps
                    .iter()
                    .flat_map(|s| s.inputs.iter().map(|i| i.qty))
                    .collect()
            })
            .collect()
    };
    let before = quantities(&game);
    assert!(
        before.iter().any(|c| c.iter().any(|q| *q > 1)),
        "a chain has to quote a quantity above the discount floor for this to prove anything"
    );

    let player = game.player_entity();
    game.world.get_mut::<Perks>(player).unwrap().points = 10;
    game.unlock_perk(Perk::LeanCompiler).unwrap();

    assert_eq!(
        before,
        quantities(&game),
        "buying LeanCompiler changed what a structure consumes"
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
    let base_mitigation = game.player_status().mitigation;

    game.unlock_perk(Perk::Defender).unwrap();
    assert_eq!(
        game.player_status().mitigation,
        base_mitigation + DEFENDER_BONUS_PER_LEVEL
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

#[test]
fn the_original_seven_perks_keep_their_positions() {
    // Perk's variant order IS the save format: bincode encodes an enum
    // positionally and PlayerSave::unlocked_perks holds indices, so a
    // reordering would turn one player's Attacker levels into Defender
    // levels on load. The five affinity perks must be appended.
    let all = Perk::all();
    assert_eq!(all[0], Perk::KeenScavenger);
    assert_eq!(all[1], Perk::LowPowerMode);
    assert_eq!(all[2], Perk::ExploitFocus);
    assert_eq!(all[3], Perk::LeanCompiler);
    assert_eq!(all[4], Perk::Attacker);
    assert_eq!(all[5], Perk::Defender);
    assert_eq!(all[6], Perk::Buffer);
    // The five affinity perks are just as save-format-fixed as the
    // original seven once shipped: reordering *among themselves* would
    // pass a test that only pins indices 0-6 and checks len() == 12, while
    // silently turning one player's DamageAffinity levels into
    // HealAffinity levels on load.
    assert_eq!(all[7], Perk::DamageAffinity);
    assert_eq!(all[8], Perk::HealAffinity);
    assert_eq!(all[9], Perk::BuffAffinity);
    assert_eq!(all[10], Perk::DebuffAffinity);
    assert_eq!(all[11], Perk::DrainAffinity);
    // And the same again for the four subsystem perks appended after them.
    assert_eq!(all[12], Perk::Obfuscation);
    assert_eq!(all[13], Perk::ProcessPool);
    assert_eq!(all[14], Perk::Teardown);
    assert_eq!(all[15], Perk::Failover);
    // And again for the quality perk appended after those.
    assert_eq!(all[16], Perk::TightenTolerances);
    assert_eq!(all.len(), 17);
}

#[test]
fn every_affinity_kind_maps_to_a_perk_and_back() {
    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert_eq!(kind.perk().affinity_kind(), Some(kind));
    }
}

#[test]
fn a_non_affinity_perk_has_no_category() {
    assert_eq!(Perk::Attacker.affinity_kind(), None);
    assert_eq!(Perk::KeenScavenger.affinity_kind(), None);
}

#[test]
fn all_five_affinity_perks_are_on_offer_in_the_picker() {
    // Driven by PerkDb::catalogue, so this is really "all five .ron files
    // parse" — a file naming a variant the build lacks is rejected by RON
    // as an unknown variant and the perk silently stops being offered.
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let offered: Vec<Perk> = game.perk_defs().iter().map(|d| d.id).collect();
    for kind in [
        AffinityKind::Damage,
        AffinityKind::Heal,
        AffinityKind::Buff,
        AffinityKind::Debuff,
        AffinityKind::Drain,
    ] {
        assert!(
            offered.contains(&kind.perk()),
            "{:?} affinity perk is not on offer",
            kind
        );
    }
}

/// Stacks `Perk::KeenScavenger` high enough that a level-1 node's roll
/// clamps at 1.0, so "no cycle fizzles" is a property of the formula rather
/// than of the seed. Derived from the live curve so a retune of either
/// constant cannot quietly leave the tests below rolling.
///
/// `base_int` is whoever will actually be working the node, and it has to be
/// a parameter rather than the baseline: the roll it is capping now includes
/// the worker's own aptitude, so deriving against the player while a *dull*
/// program does the job under-buys and leaves cycles fizzling. The player
/// working the node passes `DEFAULT_BASE_INT`, being the baseline by
/// definition; a posted program passes its own species'.
fn buy_enough_keen_scavenger_to_cap_a_level_1_node(game: &mut Game, base_int: i32) {
    let levels = ((1.0 - crate::systems::mining_success_chance(1, 0, base_int))
        / KEEN_SCAVENGER_BONUS_PER_LEVEL)
        .ceil() as usize;
    let player = game.player_entity();
    game.world
        .get_mut::<Perks>(player)
        .unwrap()
        .unlocked
        .extend(std::iter::repeat_n(Perk::KeenScavenger, levels));
}

/// The perk belongs to the player, but the mining roll runs per gather
/// cycle — so what needs covering here is the wiring, not the formula
/// (`systems::keen_scavenger_adds_to_the_mining_roll_and_still_caps_at_one`
/// already pins that). Buying enough levels to cap a level-1 node's roll at
/// a certainty must mean no cycle fizzles; if the perk never reaches the
/// roll, a 50% node fizzles repeatedly across this many cycles.
///
/// The cronjob half of the same wiring is
/// `keen_scavenger_reaches_the_roll_a_cronjob_worker_runs` — two systems
/// read the perk by different routes, so one test cannot cover both.
#[test]
fn keen_scavenger_reaches_the_roll_when_you_work_a_node_yourself() {
    let mut game = Game::new(4210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    assert_eq!(
        game.world.get::<ResourceNode>(node).unwrap().level,
        Some(1),
        "a fresh node has to roll at all for the perk to be measurable against it"
    );

    buy_enough_keen_scavenger_to_cap_a_level_1_node(&mut game, crate::tuning::DEFAULT_BASE_INT);

    game.work_structure(node)
        .expect("a deployed node is workable");
    for _ in 0..60 {
        game.wait();
    }

    let log = game.message_log(MESSAGE_LOG_CAP);
    let extractions = log
        .iter()
        .filter(|e| e.text.starts_with("You extract"))
        .count();
    let fizzles = log
        .iter()
        .filter(|e| e.text.contains("fails to compile"))
        .count();
    assert!(
        extractions > 0,
        "the job should have completed cycles to measure: {log:?}"
    );
    assert_eq!(
        fizzles, 0,
        "a roll the perk has capped at a certainty must never fizzle: {log:?}"
    );
}

/// The other half of the same wiring: a cronjob's roll runs inside a system
/// iterating worker programs, and the perk is the player's, so
/// `task_progress_system` has to reach outside its loop for it. The perk is
/// bought by the owner and applies to the work their programs do.
#[test]
fn keen_scavenger_reaches_the_roll_a_cronjob_worker_runs() {
    let mut game = Game::new(4210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let node = deploy_upgradeable_node(&mut game);
    let worker = spawn_tamed(&mut game, 10, 3);
    game.assign_cronjob(worker, node).unwrap();
    park_at_post(&mut game, worker, node);
    // Read off the species actually posted rather than assumed: `spawn_tamed`
    // picks whichever species declares no abilities, and what that resolves
    // to is not this test's business — only that the cap is derived against
    // the aptitude doing the work.
    let worker_int = generic_species().base_int;
    buy_enough_keen_scavenger_to_cap_a_level_1_node(&mut game, worker_int);

    for _ in 0..60 {
        game.wait();
    }

    let log = game.message_log(MESSAGE_LOG_CAP);
    let extractions = log
        .iter()
        .filter(|e| e.text.starts_with("Your subroutine extracted"))
        .count();
    let fizzles = log
        .iter()
        .filter(|e| e.text.contains("fails to compile"))
        .count();
    assert!(
        extractions > 0,
        "the cronjob should have completed cycles to measure: {log:?}"
    );
    assert_eq!(
        fizzles, 0,
        "the perk has to reach the worker's roll too: {log:?}"
    );
}
