//! Program extraction, phase 1: `items::DownedProgram`, its
//! `components::DownedPrograms` store, the `tools::ToolDb` catalogue, the
//! player's `components::Tools` slots with the starter grant, and the
//! extraction door itself — `Game::extraction_yield` and
//! `Game::extract_program`. This file is every phase-1 piece in isolation;
//! later phases append to it rather than starting a sibling file. See
//! `docs/superpowers/specs/2026-09-04-program-extraction-design.md`.

use super::support::*;
use crate::components::Tools;
use crate::items::DownedProgram;
use crate::tools::{ToolDb, ToolDef, ToolId};
use crate::*;

fn program(condition: u8, rarity: Rarity, level: u32) -> DownedProgram {
    DownedProgram {
        species: "scrapper".to_string(),
        level,
        rarity,
        boss: false,
        condition,
    }
}

#[test]
fn grade_rises_monotonically_with_each_axis_held_fixed() {
    // Condition, rarity and level fixed in turn while the other two hold
    // still — `DownedProgram::grade` must move the same direction as each
    // axis in isolation, or a yield formula built on it (a later phase)
    // could reward a worse-condition, lower-level or more-common program
    // over a better one.
    let by_condition: Vec<f32> = [10u8, 40, 70, 100]
        .into_iter()
        .map(|condition| program(condition, Rarity::Gold, 20).grade())
        .collect();
    assert!(
        by_condition.windows(2).all(|w| w[1] > w[0]),
        "grade must rise with condition alone: {by_condition:?}"
    );

    let by_rarity: Vec<f32> = Rarity::ALL
        .into_iter()
        .map(|rarity| program(70, rarity, 20).grade())
        .collect();
    assert!(
        by_rarity.windows(2).all(|w| w[1] > w[0]),
        "grade must rise with rarity's rung alone: {by_rarity:?}"
    );

    let by_level: Vec<f32> = [1u32, 10, 30, 80]
        .into_iter()
        .map(|level| program(70, Rarity::Gold, level).grade())
        .collect();
    assert!(
        by_level.windows(2).all(|w| w[1] > w[0]),
        "grade must rise with level alone: {by_level:?}"
    );
}

#[test]
fn downed_programs_survive_a_save_load_round_trip() {
    // Save -> load, not a RON round trip: `PlayerSave::downed_programs` is
    // `#[serde(default)]`, and a RON round trip can't catch a field that
    // silently defaults away — only `Game::save`/`Game::load` exercise the
    // path that would drop it.
    let mut game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let held = vec![
        program(15, Rarity::Ordinary, 3),
        program(88, Rarity::Prismatic, 47),
        DownedProgram {
            species: "sentinel".to_string(),
            level: 12,
            rarity: Rarity::Silver,
            boss: true,
            condition: 60,
        },
    ];
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = held.clone();

    let path = std::env::temp_dir().join(format!(
        "feral_downed_programs_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .world
        .get::<DownedPrograms>(loaded.player_entity())
        .unwrap();
    assert_eq!(
        restored.0, held,
        "three distinct downed programs must come back exactly as saved"
    );
}

#[test]
fn tool_slots_grow_one_a_step_from_a_full_base_slot() {
    // `tools::player_tool_slots` has its own unit test over the raw formula;
    // this is the shape assertion from the seat of a real save/`Game`
    // rather than the arithmetic — the controller's ruling was one slot at
    // base and one per step, deliberately not `abilities::
    // ROUTINE_SLOTS_PER_STEP`'s two, since a level-1 player holding a spare
    // slot beside the starter tool would have nothing yet to choose
    // between.
    assert_eq!(
        crate::tools::player_tool_slots(1),
        1,
        "the starter tool fills the only slot at level 1"
    );
    assert_eq!(
        crate::tools::player_tool_slots(tuning::TOOL_SLOT_PER_LEVEL),
        2,
        "one slot at the first step, not routines' two"
    );
    assert_eq!(
        crate::tools::player_tool_slots(9_999),
        tuning::TOOL_SLOT_CAP as usize,
        "never above the modest cap, however high level climbs"
    );
}

#[test]
fn a_new_game_starts_with_the_starter_tool_in_slot_one() {
    let game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let installed = game.installed_tools();
    assert_eq!(
        installed.len(),
        1,
        "exactly the base slot, filled — nothing else grants a tool yet"
    );
    assert_eq!(installed[0].id, ToolId(tuning::STARTER_TOOL_ID.to_string()));
}

#[test]
fn game_load_never_re_grants_the_starter_tool() {
    // The profile rule: pay at `Game::new`, never at `Game::load`. Emptying
    // the loadout by hand stands in for a future `uninstall_tool` (phase 2)
    // — what matters here is that `load` is not a second door the starter
    // grant can walk back through.
    let mut game = Game::new(4471, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Tools>(player).unwrap().0.clear();
    assert!(game.installed_tools().is_empty());

    let path =
        std::env::temp_dir().join(format!("feral_tools_no_regrant_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.installed_tools().is_empty(),
        "a load must never re-grant the starter tool into an emptied loadout"
    );
}

#[test]
fn a_tool_loadout_survives_a_save_load_round_trip() {
    // Save -> load, not a RON round trip — `PlayerSave::tools` is
    // `#[serde(default)]`, and only `Game::save`/`Game::load` exercise the
    // path that would silently drop a skipped field.
    let mut game = Game::new(2210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let held = vec![
        ToolId("salvage_clamp".to_string()),
        ToolId("core_tap".to_string()),
    ];
    game.world.get_mut::<Tools>(player).unwrap().0 = held.clone();

    let path =
        std::env::temp_dir().join(format!("feral_tools_roundtrip_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded.world.get::<Tools>(loaded.player_entity()).unwrap();
    assert_eq!(
        restored.0, held,
        "a two-tool loadout must come back in the same slot order"
    );
}

/// Writes `files` as `.ron` into a fresh temp dir and loads a `ToolDb` from
/// it. Duplicated rather than shared with `abilities`'s own version of this
/// helper: that one is private to its module's `#[cfg(test)]` block.
fn load_tools(tag: &str, files: &[(&str, &str)]) -> (ToolDb, Vec<String>) {
    let dir = std::env::temp_dir().join(format!("feral_tools_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(format!("{name}.ron")), body).unwrap();
    }
    let result = ToolDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    result
}

const VALID_TOOL: &str = r#"(
    id: "test_good_tool",
    name: "Test Good Tool",
    description: "d",
    category: Materials,
    yields: [("core_fragment", 1.0)],
    tier: 1,
    ticks: 5,
)"#;

/// `ToolDb::load_dir` is the second `load_dir` this phase writes, alongside
/// `items::DownedProgram`'s own store — see `AffixDb::load_dir`'s rule,
/// which `ToolDb` follows rather than `AbilityDb::load_dir`'s: abilities are
/// mandatory content and refuse a missing directory outright, but a tool
/// catalogue has no floor to enforce yet.
#[test]
fn an_absent_tools_directory_loads_silently_empty() {
    let dir = std::env::temp_dir().join(format!("feral_tools_absent_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    assert!(!dir.exists(), "the directory must genuinely not exist");

    let (db, warnings) = ToolDb::load_dir(&dir).unwrap();
    assert!(
        db.all().next().is_none(),
        "an absent directory must load no tools"
    );
    assert!(
        warnings.is_empty(),
        "an absent directory must warn about nothing: {warnings:?}"
    );
}

/// Mirrors `abilities`'s own malformed-file coverage: a file that fails to
/// parse is skipped with a warning naming it, and its well-formed neighbour
/// still loads — never a panic that would take the whole game down over one
/// bad mod file.
#[test]
fn a_malformed_tool_file_is_skipped_while_its_neighbour_still_loads() {
    let (db, warnings) = load_tools(
        "malformed",
        &[("good", VALID_TOOL), ("bad", "not valid ron at all {{{")],
    );
    assert!(
        db.get("test_good_tool").is_some(),
        "the well-formed neighbour must still load"
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(
        warnings[0].contains("bad"),
        "the warning should name the bad file: {}",
        warnings[0]
    );
}

// ---------------------------------------------------------------------------
// The condition roll — `DownedProgram::roll_condition`, section 1's formula.
// A kill leaving a program at all is `tests/combat_rewards.rs`'s territory;
// this is the pure formula in isolation, so the `FIGHT_CONDITION_WEIGHT`
// independence claim doesn't need a `Game` or a live kill to assert.
// ---------------------------------------------------------------------------

#[test]
fn condition_is_independent_of_overkill_while_the_fight_weight_is_zero() {
    assert_eq!(
        tuning::FIGHT_CONDITION_WEIGHT,
        0.0,
        "test premise: the fight axis ships at 0.0 — see tuning.rs's own doc for why"
    );
    let clean_kill = DownedProgram::roll_condition(Rarity::Gold, false, 0.0);
    let messy_kill = DownedProgram::roll_condition(Rarity::Gold, false, -0.97);
    assert_eq!(
        clean_kill, messy_kill,
        "two kills differing only in overkill must roll the same condition while the fight \
         axis is off"
    );
}

#[test]
fn condition_still_rises_with_rarity_and_a_boss_bonus() {
    // The positive control for the test above: the other two terms of the
    // same formula must still move, or "independent of overkill" would be
    // vacuously true because nothing in the formula does anything.
    let ordinary = DownedProgram::roll_condition(Rarity::Ordinary, false, 0.0);
    let gold = DownedProgram::roll_condition(Rarity::Gold, false, 0.0);
    assert!(
        gold > ordinary,
        "a rarer kill should roll a higher condition: {ordinary} vs {gold}"
    );

    let plain = DownedProgram::roll_condition(Rarity::Ordinary, false, 0.0);
    let boss = DownedProgram::roll_condition(Rarity::Ordinary, true, 0.0);
    assert!(
        boss > plain,
        "a boss kill should roll a higher condition: {plain} vs {boss}"
    );
}

// ---------------------------------------------------------------------------
// The extraction door — `Game::extraction_yield` and `Game::extract_program`,
// spec sections 3 and 4.
// ---------------------------------------------------------------------------

/// Sums a `Vec<(ItemId, u32)>` into a merged, orderless total per item — so
/// two rows naming the same item (a tool's own pool and its `rich_in`
/// bonus happening to coincide, `scrapper`'s own case below) compare equal
/// to one row carrying their sum.
fn totals(rows: &[(ItemId, u32)]) -> std::collections::BTreeMap<ItemId, u32> {
    let mut out = std::collections::BTreeMap::new();
    for (item, qty) in rows {
        *out.entry(item.clone()).or_insert(0) += qty;
    }
    out
}

fn starter_tool_def(game: &Game) -> ToolDef {
    game.world
        .resource::<ToolDb>()
        .get(tuning::STARTER_TOOL_ID)
        .unwrap()
        .clone()
}

#[test]
fn rich_in_falls_back_to_work_resource_for_every_shipped_species() {
    // No shipped species authors `rich_in` yet (spec decision 5: none had
    // to), so `Game::rich_in` must answer exactly what `work_resource`
    // already does, for every one of the 17 files.
    let game = Game::new(4475, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game.world.resource::<SpeciesDb>();
    let mut checked = 0;
    for def in species.all() {
        assert_eq!(
            game.rich_in(&def.id),
            def.work_resource.clone(),
            "species {:?} rich_in must fall back to its own work_resource",
            def.id
        );
        checked += 1;
    }
    assert!(checked > 0, "the walk covered no species at all");
}

#[test]
fn rich_in_overrides_work_resource_and_reaches_extraction_yields_output() {
    // Every shipped species leaves `rich_in` unset, so the fallback test
    // above can't tell an override branch that never ran from one that
    // isn't there at all — this fixture is the one place in the suite that
    // sets `rich_in` to something other than `work_resource`, so `def.
    // rich_in.clone().or_else(...)` in `Game::rich_in` has a test that
    // fails if it's collapsed to `work_resource` alone.
    let mut game = Game::new(4479, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let template = game
        .species_defs()
        .into_iter()
        .next()
        .expect("at least one shipped species");
    let overridden = SpeciesDef {
        id: "rich_in_override_species".to_string(),
        work_resource: Some(ItemId::from(crate::items::ids::CORE_FRAGMENT)),
        rich_in: Some(ItemId::from(crate::items::ids::RESEARCH_DATA)),
        ..template
    };
    game.world.resource_mut::<SpeciesDb>().insert(overridden);

    assert_eq!(
        game.rich_in(&"rich_in_override_species".to_string()),
        Some(ItemId::from(crate::items::ids::RESEARCH_DATA)),
        "Game::rich_in must answer the override, not the species' own work_resource"
    );

    let prog = DownedProgram {
        species: "rich_in_override_species".to_string(),
        level: 20,
        rarity: Rarity::Gold,
        condition: 70,
        boss: false,
    };
    let tool = starter_tool_def(&game);
    let granted = totals(&game.extraction_yield(&prog, &tool));

    assert_eq!(
        granted
            .get(&ItemId::from(crate::items::ids::RESEARCH_DATA))
            .copied(),
        Some(tuning::RICH_IN_UNITS),
        "the override's bonus must reach extraction_yield's output: {granted:?}"
    );

    // Baseline: the same fixture with no override at all — `research_data`
    // is not in the starter tool's own pool (`salvage_clamp` names only
    // `core_fragment` and `bytecode_block`), so its presence above can only
    // have come from `rich_in`, never from `apportion`'s weight split.
    let plain = SpeciesDef {
        id: "rich_in_plain_species".to_string(),
        work_resource: Some(ItemId::from(crate::items::ids::CORE_FRAGMENT)),
        rich_in: None,
        ..game
            .species_defs()
            .into_iter()
            .next()
            .expect("at least one shipped species")
    };
    game.world.resource_mut::<SpeciesDb>().insert(plain);
    let plain_prog = DownedProgram {
        species: "rich_in_plain_species".to_string(),
        ..prog
    };
    let plain_granted = totals(&game.extraction_yield(&plain_prog, &tool));
    assert!(
        !plain_granted.contains_key(&ItemId::from(crate::items::ids::RESEARCH_DATA)),
        "with rich_in unset, falling back to work_resource (core_fragment) must not somehow \
         still grant research_data — the override above must be what put it there: \
         {plain_granted:?}"
    );
}

#[test]
fn extraction_removes_the_program_and_grants_exactly_the_previewed_yield() {
    let mut game = Game::new(4476, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let prog = program(70, Rarity::Gold, 20);
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![prog.clone()];
    let tool_id = ToolId(tuning::STARTER_TOOL_ID.to_string());
    let tool_def = starter_tool_def(&game);

    let preview = game.extraction_yield(&prog, &tool_def);
    assert!(
        !preview.is_empty(),
        "test premise: a Gold, level-20 program run through the starter tool must yield \
         something, or this test proves nothing"
    );

    let before = totals(&game.world.get::<Inventory>(player).unwrap().items);
    game.extract_program(0, &tool_id)
        .expect("extraction must be allowed here — nothing refuses it");
    let after = totals(&game.world.get::<Inventory>(player).unwrap().items);

    assert!(
        game.world
            .get::<DownedPrograms>(player)
            .unwrap()
            .0
            .is_empty(),
        "the extracted program must be removed from the store"
    );

    // Signed, and over the union of both keysets: a plain `qty > prior`
    // filter (the shape this test shipped with) is blind to a decrease —
    // an extraction that *removed* an item as a side effect would pass it
    // silently, since only the increases it also grants would show up.
    let mut items: std::collections::BTreeSet<ItemId> = before.keys().cloned().collect();
    items.extend(after.keys().cloned());
    let mut delta: std::collections::BTreeMap<ItemId, i64> = std::collections::BTreeMap::new();
    for item in items {
        let prior = before.get(&item).copied().unwrap_or(0) as i64;
        let now = after.get(&item).copied().unwrap_or(0) as i64;
        if now != prior {
            delta.insert(item, now - prior);
        }
    }
    let expected: std::collections::BTreeMap<ItemId, i64> = totals(&preview)
        .into_iter()
        .map(|(item, qty)| (item, qty as i64))
        .collect();
    assert_eq!(
        delta, expected,
        "the inventory delta a real extraction grants must equal the previewed yield exactly, \
         with no unaccounted decrease anywhere"
    );
}

#[test]
fn a_higher_grade_program_yields_more_than_a_lower_one_all_else_equal() {
    let game = Game::new(4477, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tool = starter_tool_def(&game);

    let worst = program(10, Rarity::Ordinary, 1);
    let best = program(100, Rarity::Prismatic, 100);
    assert!(
        best.grade() > worst.grade(),
        "test premise: the two fixtures must actually differ in grade"
    );

    let low: u32 = totals(&game.extraction_yield(&worst, &tool)).values().sum();
    let high: u32 = totals(&game.extraction_yield(&best, &tool)).values().sum();
    assert!(
        high > low,
        "a higher-grade program must yield more total units through the same tool: {low} vs \
         {high}"
    );
}

#[test]
fn a_higher_tier_tool_yields_more_than_a_lower_one_on_the_same_program() {
    let game = Game::new(4478, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let prog = program(70, Rarity::Gold, 20);
    let low_tier = starter_tool_def(&game);
    let high_tier = game
        .world
        .resource::<ToolDb>()
        .get("core_tap")
        .unwrap()
        .clone();
    assert!(
        high_tier.tier > low_tier.tier,
        "test premise: core_tap must actually be a higher tier than the starter tool"
    );

    let low: u32 = totals(&game.extraction_yield(&prog, &low_tier))
        .values()
        .sum();
    let high: u32 = totals(&game.extraction_yield(&prog, &high_tier))
        .values()
        .sum();
    assert!(
        high > low,
        "a higher-tier tool must yield more total units on the same program: {low} vs {high}"
    );
}

#[test]
fn teardown_perk_adds_its_flat_bonus_to_the_unit_count() {
    let mut game = Game::new(4479, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let tool = starter_tool_def(&game);
    let prog = program(70, Rarity::Gold, 20);

    let without: u32 = totals(&game.extraction_yield(&prog, &tool)).values().sum();
    game.world
        .get_mut::<Perks>(player)
        .unwrap()
        .unlocked
        .push(Perk::Teardown);
    let with: u32 = totals(&game.extraction_yield(&prog, &tool)).values().sum();

    assert_eq!(
        with,
        without + tuning::TEARDOWN_SALVAGE_PER_LEVEL,
        "one level of Teardown must add exactly its flat bonus to the unit count"
    );
}

#[test]
fn extraction_yield_spends_no_gamerng_draw_even_with_teardown_bought() {
    // `Perk::Teardown` used to sit on top of `roll_work_resource_drop`'s own
    // draw as a flat addend, never a second roll — the property
    // `teardown_adds_flat_salvage_to_a_kill_without_rerolling` held before
    // Task 4 deleted that function. `extraction_yield` is where the perk's
    // term lives now, so this reasserts the same property there: calling it
    // — with the perk bought — must not move the shared `GameRng` stream at
    // all, `&self`'s own reason (the screen's preview calls this once per
    // installed tool with nothing spent).
    assert!(
        rng_unadvanced_by(4480, |game| {
            let player = game.player_entity();
            game.world
                .get_mut::<Perks>(player)
                .unwrap()
                .unlocked
                .push(Perk::Teardown);
            let tool = starter_tool_def(game);
            let prog = program(70, Rarity::Gold, 20);
            let _ = game.extraction_yield(&prog, &tool);
        }),
        "extraction_yield must not draw from the shared GameRng stream, salvage_bonus included"
    );
}

fn minimal_active_battle(game: &Game) -> BattleState {
    BattleState {
        player: game.player_entity(),
        round_targets: Vec::new(),
        groups: Vec::new(),
        round: 1,
        planned: vec![None],
        finished: false,
        player_won: false,
        decompile_attempts: std::collections::HashMap::new(),
        rewards: BattleRewards::default(),
        lair: None,
        outmatched: false,
    }
}

#[test]
fn extraction_refuses_after_game_over_and_spends_nothing() {
    let mut game = Game::new(4481, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![program(70, Rarity::Gold, 20)];
    game.world.resource_mut::<GameOver>().reason = Some("done".to_string());
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.extract_program(0, &ToolId(tuning::STARTER_TOOL_ID.to_string()));

    assert!(result.is_err(), "a game-over run must refuse extraction");
    assert_eq!(
        game.world.get::<DownedPrograms>(player).unwrap().0.len(),
        1,
        "the program must still be held"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent or granted on a refusal"
    );
}

#[test]
fn extraction_refuses_during_an_active_battle_and_spends_nothing() {
    let mut game = Game::new(4482, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![program(70, Rarity::Gold, 20)];
    let battle = minimal_active_battle(&game);
    game.world.insert_resource(battle);
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.extract_program(0, &ToolId(tuning::STARTER_TOOL_ID.to_string()));

    assert!(result.is_err(), "an active battle must refuse extraction");
    assert_eq!(
        game.world.get::<DownedPrograms>(player).unwrap().0.len(),
        1,
        "the program must still be held"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent or granted on a refusal"
    );
}

#[test]
fn extraction_refuses_an_out_of_range_index_and_spends_nothing() {
    let mut game = Game::new(4483, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![program(70, Rarity::Gold, 20)];
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.extract_program(5, &ToolId(tuning::STARTER_TOOL_ID.to_string()));

    assert!(
        result.is_err(),
        "an out-of-range index must refuse extraction"
    );
    assert_eq!(
        game.world.get::<DownedPrograms>(player).unwrap().0.len(),
        1,
        "the program must still be held"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent or granted on a refusal"
    );
}

#[test]
fn the_starter_tool_is_drop_neutral_for_a_median_kill() {
    // Spec decision 8, the phase's only economy gate: replacing the retired
    // `roll_work_resource_drop` roll (`WORK_RESOURCE_DROP`) with extraction
    // must not change what an ordinary kill actually pays a player holding
    // the starter tool. `extraction_yield` is deterministic — `apportion`'s
    // largest-remainder split spends no `GameRng` draw — so the figure here
    // is computed once from the real formula, never sampled. `Game::new`
    // grants the player no perks, so `extraction_yield`'s `salvage_bonus`
    // term (`Perk::Teardown`) is silently zero below — a profile that ever
    // starts a run with Teardown already bought would move this gate
    // without touching this file.
    let game = Game::new(4485, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let tool = starter_tool_def(&game);
    let median = program(tuning::CONDITION_BASE, Rarity::Ordinary, 1);

    let granted = game.extraction_yield(&median, &tool);
    let rows = totals(&granted);
    let total: u32 = rows.values().sum();

    // The retired roll's own mean, from its own constant — never restated
    // as a literal, so a future change to `WORK_RESOURCE_DROP`'s range (it
    // has zero readers left, but survives as this fit's reference point)
    // moves this test's target with it rather than silently drifting.
    let retired_mean = (*tuning::WORK_RESOURCE_DROP.start() as f32
        + *tuning::WORK_RESOURCE_DROP.end() as f32)
        / 2.0;

    // Exact equality, not `(total - retired_mean).abs() <= 1.0`: the loose
    // form is satisfied even with `rich_in`'s bonus dropped entirely (total
    // would be 2, delta 1.0, still "within one unit") — implemented but
    // undefended, one refactor from silently reverting to the lenient
    // reading. Exact equality is strictly *stronger* than "within one
    // unit", so it still satisfies the brief's own bound; it just can no
    // longer be satisfied by accident.
    assert_eq!(
        total,
        3,
        "a median kill (ordinary, CONDITION_BASE condition, level 1) through the starter tool \
         must pay exactly 3 units — within one unit of the retired WORK_RESOURCE_DROP roll's \
         mean ({retired_mean}) — got {total} (grade {}, rows {granted:?})",
        median.grade()
    );

    // Pin the `rich_in` half of that total on its own rather than only
    // through the sum above: `scrapper`'s `rich_in` fallback
    // (`work_resource`) happens to be an item the starter tool's own pool
    // already names, so a `total == 3` that came entirely from `apportion`
    // (no `rich_in` addend at all, paired with a pool reweighted to
    // compensate) would satisfy the assertion above without `rich_in`
    // having contributed anything. It must actually be in the rows.
    let rich_item = game
        .rich_in(&median.species)
        .expect("scrapper must resolve a rich_in item (falls back to work_resource)");
    assert!(
        rows.get(&rich_item)
            .is_some_and(|&qty| qty >= tuning::RICH_IN_UNITS),
        "the rich_in bonus ({} units of {rich_item:?}) must land in the granted rows, not just \
         be assumed from the total: {rows:?}",
        tuning::RICH_IN_UNITS
    );
}

#[test]
fn extraction_refuses_an_uninstalled_tool_and_spends_nothing() {
    let mut game = Game::new(4484, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![program(70, Rarity::Gold, 20)];
    // The new game's only installed tool is the starter (`salvage_clamp`);
    // `core_tap` exists in the catalogue but was never installed.
    assert!(
        game.installed_tools()
            .iter()
            .all(|t| t.id != ToolId("core_tap".to_string())),
        "test premise: core_tap must not be installed"
    );
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.extract_program(0, &ToolId("core_tap".to_string()));

    assert!(
        result.is_err(),
        "an uninstalled tool must refuse extraction"
    );
    assert_eq!(
        game.world.get::<DownedPrograms>(player).unwrap().0.len(),
        1,
        "the program must still be held"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent or granted on a refusal"
    );
}
