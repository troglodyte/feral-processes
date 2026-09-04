//! Perk unlocks and the effects each perk level has.

use super::support::*;
use crate::abilities::AffinityKind;
use crate::tuning::{
    ATTACKER_BONUS_PER_LEVEL, BUFFER_MIN_BONUS_PER_LEVEL, DECOMPILER_SKILL_PER_LEVEL,
    DEFENDER_BONUS_PER_LEVEL, DIFFICULTY_EVEN_MAX, KEEN_SCAVENGER_BONUS_PER_LEVEL,
    LEAN_COMPILER_DISCOUNT_PER_LEVEL, OVERFLOW_XP_BASE, OVERFLOW_XP_STEP,
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
    // And again for the accuracy perk appended after that.
    assert_eq!(all[17], Perk::TargetLock);
    assert_eq!(all.len(), 18);
}

/// `Perk::TargetLock` reaches the roll, and reaches the player alone.
///
/// Asserted through `hit_chance` rather than through the accuracy number,
/// because a bonus that raised Accuracy without moving the odds would be
/// worth nothing and would pass a test that only read the input.
#[test]
fn target_lock_raises_the_players_odds_and_nobody_elses() {
    let mut game = Game::new(4190, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", 20, 20);
    let swing = crate::battle::Swing::plain(crate::battle::DamageRange::centred(10, 0));

    let odds = |g: &Game, attacker| {
        let a = g.combatant_profile(attacker, swing);
        let d = g.combatant_profile(
            if attacker == player { wild } else { player },
            crate::battle::Swing::default(),
        );
        crate::battle::hit_chance(a.accuracy, d.evasion)
    };
    let before_player = odds(&game, player);
    let before_wild = odds(&game, wild);

    for _ in 0..3 {
        game.world.get_mut::<Perks>(player).unwrap().points = 10;
        game.unlock_perk(Perk::TargetLock).unwrap();
    }

    assert!(
        odds(&game, player) > before_player,
        "three levels of Target Lock must move the player's odds: {} is not above {}",
        odds(&game, player),
        before_player
    );
    assert_eq!(
        odds(&game, wild),
        before_wild,
        "a player's perk must not aim a wild program's swing"
    );
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
    let levels = ((1.0 - crate::systems::mining_success_chance(1, 0, base_int, 0.0, 0.0))
        / KEEN_SCAVENGER_BONUS_PER_LEVEL)
        .ceil() as usize;
    let player = game.player_entity();
    game.world
        .get_mut::<Perks>(player)
        .unwrap()
        .unlocked
        .extend(std::iter::repeat_n(Perk::KeenScavenger, levels));
}

/// Takes `node` out of the GC Entropy Sweep's target pool, which filters on
/// `(With<Durability>, With<Structure>)`.
///
/// Both perk tests below tick sixty times with a base standing, so a sweep
/// can land on the very node they are measuring — and a damaged node rolls
/// differently, which reads as the perk failing to reach the roll. Whether a
/// sweep lands is a `GameRng` question, so the seed decided it and any change
/// anywhere that moves the stream flips it: this one surfaced when a resource
/// registered elsewhere shifted bevy's query iteration order. The fixtures in
/// `support.rs` built on `spawn_structure_at` are already immune for exactly
/// this reason (see that file's note); a node deployed properly is not.
fn keep_the_sweep_off(game: &mut Game, node: Entity) {
    game.world.entity_mut(node).remove::<Durability>();
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
    keep_the_sweep_off(&mut game, node);

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
    keep_the_sweep_off(&mut game, node);

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

// ---- Overflow XP ------------------------------------------------------
//
// XP earned at the level cap used to be discarded at `add_xp`'s first line.
// It accumulates in `Experience::xp` — already saved, already idle at the
// cap — and drains into Perk Points at a price that rises with the perks
// already held.

/// The accumulator half. Nothing is converted here; the XP simply stops
/// being thrown away.
#[test]
fn xp_earned_at_the_cap_accumulates_instead_of_being_discarded() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let player = game.player_entity();
    let cap = game.level_cap();
    set_level(&mut game, player, cap);
    let banked = game.world.get::<Experience>(player).unwrap().xp;

    game.award_player_xp(player, 500);

    assert!(
        game.world.get::<Experience>(player).unwrap().xp > banked,
        "XP earned at the cap must be kept, not dropped on the floor"
    );
    assert_eq!(
        game.world.get::<Experience>(player).unwrap().level,
        cap,
        "and it must still not buy a level"
    );
}

/// The conversion half.
#[test]
fn overflow_xp_converts_into_perk_points() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let player = game.player_entity();
    let cap = game.level_cap();
    set_level(&mut game, player, cap);
    game.world.get_mut::<Perks>(player).unwrap().points = 0;
    game.world.get_mut::<Experience>(player).unwrap().xp = 0;

    game.award_player_xp(player, OVERFLOW_XP_BASE * 3);

    assert!(
        game.world.get::<Perks>(player).unwrap().points > 0,
        "overflow at the cap has to buy something or the cap is just a wall"
    );
}

/// **The price rises with the perks already bought**, which is what makes
/// the exchange sublinear rather than an unbounded flat-rate power source.
///
/// The count is `BoughtStats::ever_bought` rather than the length of
/// `Perks::unlocked`, because `Game::respec_perks` empties that list and a
/// price read off it would reset to the opening rate on every wipe.
#[test]
fn the_overflow_price_rises_with_perks_bought() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let player = game.player_entity();
    let cap = game.level_cap();
    set_level(&mut game, player, cap);

    let paid_when_poor = {
        game.world.get_mut::<Perks>(player).unwrap().points = 0;
        game.world.get_mut::<Experience>(player).unwrap().xp = OVERFLOW_XP_BASE * 20;
        game.convert_overflow_xp()
    };

    // Same bank, but with a stack of perks already bought. Both halves are
    // written: the list is what the player is holding, the receipt is what
    // they have ever bought, and only the second prices this exchange.
    game.world.get_mut::<Perks>(player).unwrap().unlocked = vec![Perk::Attacker; 8];
    game.world.entity_mut(player).insert(BoughtStats {
        ever_bought: 8,
        ..Default::default()
    });
    let paid_when_rich = {
        game.world.get_mut::<Perks>(player).unwrap().points = 0;
        game.world.get_mut::<Experience>(player).unwrap().xp = OVERFLOW_XP_BASE * 20;
        game.convert_overflow_xp()
    };

    assert!(
        paid_when_rich < paid_when_poor,
        "the same XP must buy fewer points once perks are held: \
         {paid_when_rich} against {paid_when_poor}"
    );
}

/// **Points earned grow sublinearly in XP spent**, asserted as the property
/// rather than against a magic number. A linear cost makes points grow like
/// the square root of XP, which loses the race against a linear zone curve
/// forever — and that race is the whole feature.
#[test]
fn points_earned_are_sublinear_in_xp_spent() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let player = game.player_entity();
    let cap = game.level_cap();
    set_level(&mut game, player, cap);

    // Feed the same bank in one go against feeding it in halves, with the
    // perks bought along the way — twice the XP must buy strictly less than
    // twice the points.
    let earn = |game: &mut Game, xp: u32| -> u32 {
        game.world.get_mut::<Perks>(player).unwrap().points = 0;
        game.world
            .get_mut::<Perks>(player)
            .unwrap()
            .unlocked
            .clear();
        game.world.get_mut::<Experience>(player).unwrap().xp = xp;
        let mut total = 0;
        // Each point minted is a perk level the player can afford, so the
        // price walks up as they spend it — mimic that by banking the perks.
        loop {
            let minted = game.convert_overflow_xp();
            if minted == 0 {
                break;
            }
            total += minted;
            let mut perks = game.world.get_mut::<Perks>(player).unwrap();
            for _ in 0..minted {
                perks.unlocked.push(Perk::Attacker);
            }
        }
        total
    };

    let single = earn(&mut game, OVERFLOW_XP_BASE * 40);
    let double = earn(&mut game, OVERFLOW_XP_BASE * 80);
    assert!(
        double < single * 2,
        "twice the XP must buy less than twice the points: {double} against {single}"
    );
    assert!(double > single, "but it must still buy more");
}

/// Unconverted overflow becomes real levels the moment a breach lifts the
/// cap — banking and taxing are the same accumulator, which is why this
/// needed no new save field.
///
/// The overflow is **earned**, not hand-written into `Experience::xp`: with
/// the pile written in by the fixture this test passed with the banking
/// removed entirely, because the fixture was supplying what the code was
/// meant to. A deep stack of perks is what keeps it unconverted — the price
/// of the next point is then well past what the award pays, which is the
/// sublinear price doing its job.
#[test]
fn unconverted_overflow_becomes_levels_when_a_breach_lifts_the_cap() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let player = game.player_entity();
    let cap = game.level_cap();
    set_level(&mut game, player, cap);
    game.world.get_mut::<Experience>(player).unwrap().xp = 0;
    // Enough perks held that a point costs more than the award below pays,
    // so every bit of it stays banked.
    game.world.get_mut::<Perks>(player).unwrap().unlocked = vec![Perk::Attacker; 200];
    let award = 20_000;
    assert!(
        award < OVERFLOW_XP_BASE + OVERFLOW_XP_STEP * 200,
        "the fixture must not be able to afford a point, or nothing stays banked"
    );

    game.award_player_xp(player, award);
    assert_eq!(
        game.world.get::<Experience>(player).unwrap().level,
        cap,
        "still capped, so none of that bought a level"
    );

    game.world.insert_resource(ZoneLevel(4));
    game.award_player_xp(player, 1);

    assert!(
        game.world.get::<Experience>(player).unwrap().level > cap,
        "XP banked at the old cap must spend itself the moment the cap lifts"
    );
}

/// A companion has no Perk Points, so its overflow is simply not spent —
/// the behaviour every creature has today. Pinned so it is not read as an
/// oversight later.
#[test]
fn a_capped_companions_overflow_is_not_spent_and_does_not_panic() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(ZoneLevel(2));
    let companion = spawn_tamed(&mut game, 10, 3);
    enlist(&mut game, companion);
    let cap = game.level_cap();
    set_level(&mut game, companion, cap);
    let player_points = game
        .world
        .get::<Perks>(game.player_entity())
        .unwrap()
        .points;

    game.award_party_xp(100_000);

    assert_eq!(
        game.world.get::<Experience>(companion).unwrap().level,
        cap,
        "a capped companion stays capped"
    );
    assert_eq!(
        game.world
            .get::<Perks>(game.player_entity())
            .unwrap()
            .points,
        player_points,
        "and its overflow does not leak into the player's Perk Points"
    );
}
