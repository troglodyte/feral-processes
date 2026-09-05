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
use crate::tools::{ToolCategory, ToolDb, ToolDef, ToolId};
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

pub(super) fn minimal_active_battle(game: &Game) -> BattleState {
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

    // Pin the complete row map, not just the total and a `>=` on one row:
    // `rows.get(&rich_item).is_some_and(|&qty| qty >= RICH_IN_UNITS)` (the
    // shape this test shipped with) passes with zero contribution from
    // `rich_in` at all, because `scrapper`'s `rich_in` fallback
    // (`work_resource`) resolves to `core_fragment`, which `apportion`
    // already seats in the pool rows on its own — so it can't tell "the
    // bonus fired" from "the pool alone happened to clear the bar", and
    // it's blind to the bonus landing on the *wrong* row entirely. An exact
    // map pins both halves of the total and the apportionment split in one
    // assertion, and fails on a dropped addend, a misdirected addend, and a
    // total-preserving compensating error alike.
    //
    // Derivation (`salvage_clamp.ron`'s pool: `core_fragment` weight `1.0`,
    // `bytecode_block` weight `0.4`; `apportion` given `base_units = 2`):
    //   exact_core = 2 * 1.0 / 1.4 = 1.4286 -> floor 1, remainder 0.4286
    //   exact_byte = 2 * 0.4 / 1.4 = 0.5714 -> floor 0, remainder 0.5714
    //   spent = 1, left = 2 - 1 = 1; larger remainder (byte, 0.5714) claims
    //   the leftover unit -> core_fragment = 1, bytecode_block = 1
    // Then `rich_in` adds `RICH_IN_UNITS` (1) to `core_fragment` (the
    // merge case, since it's already a pool row): core_fragment = 1 + 1 =
    // 2, bytecode_block stays 1. Total 2 + 1 = 3, matching the sum above.
    //
    // Deliberately brittle against a future change to `salvage_clamp.ron`'s
    // pool: this is the phase's only economy gate, and a pool edit that
    // silently moves it — rather than forcing a look at this test — is
    // exactly what pinning the exact map is for.
    assert_eq!(
        game.rich_in(&median.species),
        Some(ItemId::from(crate::items::ids::CORE_FRAGMENT)),
        "test premise: scrapper's rich_in fallback must still resolve to core_fragment for the \
         derivation above to hold"
    );
    let expected_rows = std::collections::BTreeMap::from([
        (ItemId::from(crate::items::ids::CORE_FRAGMENT), 2u32),
        (ItemId::from(crate::items::ids::BYTECODE_BLOCK), 1u32),
    ]);
    assert_eq!(
        rows, expected_rows,
        "the median kill's granted rows must match the derived apportionment plus the rich_in \
         addend exactly, not just their sum: got {granted:?}"
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

// Task 7: the screen's two engine interfaces, `Game::downed_program_rows`
// and `Game::extraction_options`.

#[test]
fn downed_program_rows_reflect_the_store_in_order_with_species_names_resolved() {
    let mut game = Game::new(4490, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let held = vec![
        program(15, Rarity::Ordinary, 3),
        DownedProgram {
            species: "sentinel".to_string(),
            level: 12,
            rarity: Rarity::Silver,
            boss: true,
            condition: 60,
        },
    ];
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = held.clone();

    let rows = game.downed_program_rows();
    assert_eq!(
        rows.len(),
        held.len(),
        "one row per held program, in store order"
    );
    for (row, prog) in rows.iter().zip(held.iter()) {
        assert_eq!(row.level, prog.level);
        assert_eq!(row.rarity, prog.rarity);
        assert_eq!(row.condition, prog.condition);
        assert_eq!(row.boss, prog.boss);
        assert_eq!(
            row.grade,
            prog.grade(),
            "the row must carry DownedProgram::grade's own fold, not re-derive one"
        );
    }
    let sentinel_name = game
        .world
        .resource::<SpeciesDb>()
        .get("sentinel")
        .expect("sentinel is a shipped species")
        .name
        .clone();
    assert_eq!(
        rows[1].name, sentinel_name,
        "the row's name must resolve through SpeciesDb, not carry the raw id"
    );
    assert_ne!(
        rows[0].name, "",
        "scrapper must also resolve to a real display name"
    );
}

#[test]
fn downed_program_rows_is_empty_with_nothing_held() {
    let game = Game::new(4491, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.downed_program_rows().is_empty(),
        "a fresh run holds no downed programs"
    );
}

#[test]
fn extraction_options_lists_every_installed_tool_with_its_own_preview_yield() {
    let mut game = Game::new(4492, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let prog = program(70, Rarity::Gold, 20);
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![prog.clone()];

    let installed = game.installed_tools();
    assert!(
        !installed.is_empty(),
        "test premise: the starter tool must be installed"
    );
    let options = game.extraction_options(0);

    assert_eq!(
        options.len(),
        installed.len(),
        "one row per installed tool, in the same slot order"
    );
    for (tool, (id, yield_rows)) in installed.iter().zip(options.iter()) {
        assert_eq!(
            &tool.id, id,
            "extraction_options must walk installed_tools' own order"
        );
        assert_eq!(
            yield_rows,
            &game.extraction_yield(&prog, tool),
            "the preview must be extraction_yield's own answer, not a second copy of it"
        );
    }
}

#[test]
fn extraction_options_is_empty_for_an_index_the_store_does_not_hold() {
    let game = Game::new(4493, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(
        game.extraction_options(0).is_empty(),
        "an empty store names no program at index 0"
    );
}

#[test]
fn extraction_options_preview_matches_what_extract_program_actually_grants() {
    // The screen's whole safety property: a quoted figure and a granted one
    // cannot differ. `extraction_yield`'s own doc argues this follows from
    // determinism alone, but this test is what actually drives the preview
    // path (`extraction_options`) rather than the direct call every other
    // test in this file uses.
    let mut game = Game::new(4494, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let prog = program(70, Rarity::Gold, 20);
    game.world.get_mut::<DownedPrograms>(player).unwrap().0 = vec![prog.clone()];
    let tool_id = ToolId(tuning::STARTER_TOOL_ID.to_string());

    let options = game.extraction_options(0);
    let (_, preview) = options
        .iter()
        .find(|(id, _)| id == &tool_id)
        .expect("the starter tool must be among the offered options");
    let preview = totals(preview);

    let before = totals(&game.world.get::<Inventory>(player).unwrap().items);
    game.extract_program(0, &tool_id)
        .expect("nothing here refuses the extraction");
    let after = totals(&game.world.get::<Inventory>(player).unwrap().items);

    for (item, qty) in &preview {
        let prior = before.get(item).copied().unwrap_or(0);
        let now = after.get(item).copied().unwrap_or(0);
        assert_eq!(
            now - prior,
            *qty,
            "the previewed yield for {item:?} must equal what was actually granted"
        );
    }
}

// ---------------------------------------------------------------------------
// Phase 2, task 1: `ResearchDef::unlocks_tools` and `resources::KnownTools`.
// ---------------------------------------------------------------------------

/// Builds a standalone `ResearchDb` from `nodes`, each `(id, unlocked
/// tools)`, every node costing 1 Research Data with no prereqs. Loaded
/// against the real shipped structures/abilities/tools so the tool ids
/// themselves validate against real content, the same shape `research.rs`'s
/// own `load` test fixture takes. Swapped onto a `Game` wholesale — no
/// shipped research node names a tool yet (that is task 4's content), so
/// there is no other way to exercise `unlock_research`'s tool-teaching loop
/// in isolation.
fn research_db_with_tool_unlocks(tag: &str, nodes: &[(&str, &[&str])]) -> ResearchDb {
    let dir = std::env::temp_dir().join(format!(
        "feral_extraction_research_{}_{tag}",
        std::process::id()
    ));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (node_id, tools) in nodes {
        let names: Vec<String> = tools.iter().map(|t| format!("\"{t}\"")).collect();
        let body = format!(
            r#"(
                id: "{node_id}",
                name: "Test Node",
                description: "d",
                cost: 1,
                unlocks_tools: [{}],
            )"#,
            names.join(", ")
        );
        std::fs::write(dir.join(format!("{node_id}.ron")), body).unwrap();
    }
    let assets = test_assets_dir();
    let (structures, _) = StructureDb::load_dir(&assets.join("structures")).unwrap();
    let (abilities, _) = AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (tool_db, _) = ToolDb::load_dir(&assets.join("tools")).unwrap();
    let (db, warnings) = ResearchDb::load_dir(&dir, &structures, &abilities, &tool_db).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    assert!(
        warnings.is_empty(),
        "fixture nodes must load clean: {warnings:?}"
    );
    db
}

/// Every line the log holds, repeats expanded and filtered to `needle` —
/// `message_history` condenses an unbroken run into one row with a count,
/// so a bare entry count would read a line said twice as a line said once.
fn log_count(game: &Game, needle: &str) -> usize {
    game.message_history(500)
        .into_iter()
        .filter(|row| row.text.contains(needle))
        .map(|row| row.repeats.max(1))
        .sum()
}

#[test]
fn unlocking_a_node_teaches_its_tools_and_logs_once() {
    let mut game = Game::new(9001, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(research_db_with_tool_unlocks(
        "teach",
        &[("test_node", &["core_tap"])],
    ));
    grant_research_data(&mut game, 1);

    assert!(
        !game.knows_tool(&ToolId("core_tap".to_string())),
        "the fixture is vacuous unless the tool starts unknown"
    );

    game.unlock_research("test_node").unwrap();

    assert!(
        game.knows_tool(&ToolId("core_tap".to_string())),
        "unlocking the node must teach the tool it names"
    );
    assert_eq!(
        log_count(&game, "Core Tap"),
        1,
        "learning a tool must log exactly once"
    );
}

#[test]
fn a_tool_taught_by_two_nodes_logs_only_the_first_time() {
    // The ability arm's own rule (`unlock_research`'s fresh-insert check):
    // knowledge is a set, and re-teaching an already-known tool from a
    // second node must not repeat the log line. Both nodes ship in the one
    // `ResearchDb`, so the log is checked once at the end rather than after
    // each unlock — the point under test is the second unlock's silence.
    let mut game = Game::new(9002, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.insert_resource(research_db_with_tool_unlocks(
        "double",
        &[
            ("test_node_a", &["core_tap"]),
            ("test_node_b", &["core_tap"]),
        ],
    ));
    grant_research_data(&mut game, 2);

    game.unlock_research("test_node_a").unwrap();
    game.unlock_research("test_node_b").unwrap();

    assert!(game.knows_tool(&ToolId("core_tap".to_string())));
    assert_eq!(
        log_count(&game, "Core Tap"),
        1,
        "a tool already known must not log a second time"
    );
}

#[test]
fn known_tools_survive_a_save_load_round_trip() {
    // Save -> load, not a RON round trip: `SaveData::known_tools` is
    // `#[serde(default)]`, and only `Game::save`/`Game::load` exercise the
    // path that would silently drop a skipped field.
    let mut game = Game::new(9003, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world.resource_mut::<KnownTools>().0 = [
        ToolId("salvage_clamp".to_string()),
        ToolId("core_tap".to_string()),
    ]
    .into();

    let path = std::env::temp_dir().join(format!(
        "feral_known_tools_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        loaded.world.resource::<KnownTools>().0,
        [
            ToolId("salvage_clamp".to_string()),
            ToolId("core_tap".to_string())
        ]
        .into(),
        "a two-tool known set must come back exactly as saved"
    );
}

// ---------------------------------------------------------------------------
// Phase 2, task 2: `Game::forge_tool`.
// ---------------------------------------------------------------------------

const FORGE_TARGET_TOOL: &str = r#"(
    id: "forge_target",
    name: "Forge Target",
    description: "d",
    category: Materials,
    yields: [("core_fragment", 1.0)],
    tier: 1,
    ticks: 5,
    forge_cost: [("core_fragment", 5)],
)"#;

/// A fresh game with a custom `ToolDb` naming `forge_target` (5
/// `core_fragment` to forge), and `forge_target` marked known — the
/// starting point every forge test but the "unknown"/"unresearched" ones
/// wants. Tagged so parallel tests don't collide on the same temp dir.
fn game_ready_to_forge(seed: u32, tag: &str) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (tool_db, warnings) = load_tools(tag, &[("forge_target", FORGE_TARGET_TOOL)]);
    assert!(
        warnings.is_empty(),
        "fixture tool must load clean: {warnings:?}"
    );
    game.world.insert_resource(tool_db);
    game.world
        .resource_mut::<KnownTools>()
        .0
        .insert(ToolId("forge_target".to_string()));
    // The starter kit already carries a handful of `core_fragment` (the
    // fixture's own cost item), so a test asserting an exact spend or an
    // exact shortfall needs a known-empty pack to add onto.
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();
    game
}

fn carrier_count(game: &Game, tool: &ToolId) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&ItemId::tool(tool))
}

#[test]
fn forge_refuses_after_game_over_and_spends_nothing() {
    let mut game = game_ready_to_forge(9101, "game_over");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 10);
    game.world.resource_mut::<GameOver>().reason = Some("done".to_string());
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.forge_tool(&ToolId("forge_target".to_string()));

    assert!(result.is_err(), "a game-over run must refuse forging");
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent on a refusal"
    );
    assert_eq!(carrier_count(&game, &ToolId("forge_target".to_string())), 0);
}

#[test]
fn forge_refuses_during_an_active_battle_and_spends_nothing() {
    let mut game = game_ready_to_forge(9102, "battle");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 10);
    let battle = minimal_active_battle(&game);
    game.world.insert_resource(battle);
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.forge_tool(&ToolId("forge_target".to_string()));

    assert!(result.is_err(), "an active battle must refuse forging");
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent on a refusal"
    );
    assert_eq!(carrier_count(&game, &ToolId("forge_target".to_string())), 0);
}

#[test]
fn forge_refuses_an_unresolvable_tool_id_and_spends_nothing() {
    let mut game = game_ready_to_forge(9103, "unresolvable");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 10);
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.forge_tool(&ToolId("no_such_tool".to_string()));

    assert!(
        result.is_err(),
        "an id ToolDb cannot resolve must refuse forging"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent on a refusal"
    );
}

#[test]
fn forge_refuses_an_already_installed_tool_and_spends_nothing() {
    // The starter is installed into slot one at `Game::new` and known
    // unconditionally (Critical 1) — the natural case for "already
    // installed", since a second carrier of a tool already in a slot has
    // nowhere to go: the player is the only tool holder.
    let mut game = Game::new(9110, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 10);
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();
    let starter = ToolId(tuning::STARTER_TOOL_ID.to_string());

    let result = game.forge_tool(&starter);

    let err = result.expect_err("an already-installed tool must refuse forging");
    assert!(
        err.contains("already installed"),
        "got: {err}, expected the already-installed refusal"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent on a refusal"
    );
    assert_eq!(carrier_count(&game, &starter), 0);
}

#[test]
fn forge_refuses_an_unresearched_tool_and_spends_nothing() {
    // `core_tap` ships real content but nothing teaches it by default.
    // Unlike `core_tap`, the starter (`salvage_clamp`) cannot stand in for
    // "unresearched" any more: `knows_tool` answers true for
    // `STARTER_TOOL_ID` unconditionally (Critical 1's fix) — see
    // `pulling_the_starter_leaves_it_known_and_re_forgeable`.
    let mut game = Game::new(9104, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 10);
    assert!(
        !game.knows_tool(&ToolId("core_tap".to_string())),
        "test premise: core_tap must not be known by default"
    );
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.forge_tool(&ToolId("core_tap".to_string()));

    assert!(result.is_err(), "an unresearched tool must refuse forging");
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent on a refusal"
    );
    assert_eq!(carrier_count(&game, &ToolId("core_tap".to_string())), 0);
}

#[test]
fn pulling_the_starter_leaves_it_known_and_re_forgeable() {
    // Critical 1: a stored grant would make pulling the starter (its own
    // uninstall) permanently strand it, since nothing ever taught it in the
    // first place. `knows_tool` must answer true for `STARTER_TOOL_ID`
    // unconditionally instead, so the door back in never closes.
    let mut game = Game::new(9107, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let starter = ToolId(tuning::STARTER_TOOL_ID.to_string());
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 10);

    game.uninstall_tool(0)
        .expect("pulling the starter must succeed");

    assert!(
        game.knows_tool(&starter),
        "the starter's knowledge must survive being pulled"
    );
    assert!(
        game.tool_rows().iter().any(|r| r.id == starter),
        "the starter's row must survive being pulled"
    );

    game.forge_tool(&starter)
        .expect("the starter must still be forgeable once pulled");
    assert_eq!(carrier_count(&game, &starter), 1);
}

#[test]
fn forge_refuses_when_the_player_cannot_pay_and_spends_nothing() {
    let mut game = game_ready_to_forge(9105, "unpayable");
    let player = game.player_entity();
    // Short by one of the fixture's 5-unit cost.
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 4);
    let before = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.forge_tool(&ToolId("forge_target".to_string()));

    assert!(result.is_err(), "an unaffordable cost must refuse forging");
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before,
        "nothing must be spent on a refusal"
    );
    assert_eq!(carrier_count(&game, &ToolId("forge_target".to_string())), 0);
}

#[test]
fn forging_spends_exactly_the_cost_and_grants_one_carrier() {
    let mut game = game_ready_to_forge(9106, "success");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 12);

    game.forge_tool(&ToolId("forge_target".to_string()))
        .expect("nothing here refuses the forge");

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        7,
        "exactly the def's cost (5) must leave the inventory"
    );
    assert_eq!(
        carrier_count(&game, &ToolId("forge_target".to_string())),
        1,
        "exactly one carrier must be granted"
    );
}

const DUPLICATE_COST_TOOL: &str = r#"(
    id: "duplicate_cost_tool",
    name: "Duplicate Cost Tool",
    description: "d",
    category: Materials,
    yields: [("core_fragment", 1.0)],
    tier: 1,
    ticks: 5,
    forge_cost: [("core_fragment", 3), ("core_fragment", 2)],
)"#;

/// A game with the fixture tool above known, and a cleared pack — so a
/// test can add exactly the `core_fragment` it wants to check against the
/// summed cost (3 + 2 = 5) rather than the two individual lines.
fn game_ready_to_forge_duplicate_cost(seed: u32, tag: &str) -> Game {
    let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let (tool_db, warnings) = load_tools(tag, &[("duplicate_cost_tool", DUPLICATE_COST_TOOL)]);
    assert!(
        warnings.is_empty(),
        "fixture tool must load clean: {warnings:?}"
    );
    game.world.insert_resource(tool_db);
    game.world
        .resource_mut::<KnownTools>()
        .0
        .insert(ToolId("duplicate_cost_tool".to_string()));
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .items
        .clear();
    game
}

#[test]
fn forge_refuses_when_a_duplicated_cost_line_is_short_of_the_summed_total() {
    // Minor 5: each cost line was checked independently against the
    // currently held count, so 3 `core_fragment` passed both a "need 3"
    // line and a "need 2" line — a total (5) it cannot actually pay.
    let mut game = game_ready_to_forge_duplicate_cost(9108, "dup_short");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 3);

    let result = game.forge_tool(&ToolId("duplicate_cost_tool".to_string()));

    assert!(
        result.is_err(),
        "3 core_fragment must not pay a 3+2=5 summed cost"
    );
    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        3,
        "nothing must be spent on a refusal"
    );
    assert_eq!(
        carrier_count(&game, &ToolId("duplicate_cost_tool".to_string())),
        0
    );
}

#[test]
fn forge_spends_the_summed_total_when_a_cost_line_is_duplicated() {
    let mut game = game_ready_to_forge_duplicate_cost(9109, "dup_paid");
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 5);

    game.forge_tool(&ToolId("duplicate_cost_tool".to_string()))
        .expect("5 core_fragment must pay a 3+2=5 summed cost");

    assert_eq!(
        game.world
            .get::<Inventory>(player)
            .unwrap()
            .count(&ItemId::from(ids::CORE_FRAGMENT)),
        0,
        "the summed cost (5) must leave the inventory, not just the larger line (3)"
    );
    assert_eq!(
        carrier_count(&game, &ToolId("duplicate_cost_tool".to_string())),
        1
    );
}

// ---------------------------------------------------------------------------
// Phase 2, task 3: `Game::install_tool` and `Game::uninstall_tool`.
// ---------------------------------------------------------------------------

fn hold_carrier(game: &mut Game, tool: &str, qty: u32) {
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::tool(&ToolId(tool.to_string())), qty);
}

#[test]
fn install_refuses_after_game_over_and_changes_nothing() {
    let mut game = Game::new(9201, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    hold_carrier(&mut game, "core_tap", 1);
    game.world.get_mut::<Experience>(player).unwrap().level = tuning::TOOL_SLOT_PER_LEVEL;
    game.world.resource_mut::<GameOver>().reason = Some("done".to_string());
    let before_tools = game.world.get::<Tools>(player).unwrap().0.clone();
    let before_inv = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.install_tool(&ToolId("core_tap".to_string()));

    assert!(result.is_err(), "a game-over run must refuse installing");
    assert_eq!(game.world.get::<Tools>(player).unwrap().0, before_tools);
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before_inv
    );
}

#[test]
fn install_refuses_during_an_active_battle_and_changes_nothing() {
    let mut game = Game::new(9202, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    hold_carrier(&mut game, "core_tap", 1);
    game.world.get_mut::<Experience>(player).unwrap().level = tuning::TOOL_SLOT_PER_LEVEL;
    let battle = minimal_active_battle(&game);
    game.world.insert_resource(battle);
    let before_tools = game.world.get::<Tools>(player).unwrap().0.clone();
    let before_inv = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.install_tool(&ToolId("core_tap".to_string()));

    assert!(result.is_err(), "an active battle must refuse installing");
    assert_eq!(game.world.get::<Tools>(player).unwrap().0, before_tools);
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before_inv
    );
}

#[test]
fn install_refuses_an_unresolvable_tool_id_and_changes_nothing() {
    let mut game = Game::new(9203, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let before_tools = game.world.get::<Tools>(player).unwrap().0.clone();

    let result = game.install_tool(&ToolId("no_such_tool".to_string()));

    assert!(
        result.is_err(),
        "an id ToolDb cannot resolve must refuse installing"
    );
    assert_eq!(game.world.get::<Tools>(player).unwrap().0, before_tools);
}

#[test]
fn install_refuses_a_player_with_no_tools_component_and_spends_nothing() {
    // Minor 8: unreachable today (the player always spawns with `Tools`),
    // but the refusal must read the component before the spend rather than
    // discovering it is missing on the `unwrap()` that writes the slot.
    let mut game = Game::new(9212, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    hold_carrier(&mut game, "core_tap", 1);
    game.world.entity_mut(player).remove::<Tools>();
    let before_inv = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.install_tool(&ToolId("core_tap".to_string()));

    let err = result.expect_err("a player with no Tools component must refuse installing");
    assert_eq!(err, "That can't hold tools.");
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before_inv,
        "nothing must be spent on a refusal"
    );
}

#[test]
fn install_refuses_a_tool_already_installed_and_changes_nothing() {
    // The fresh game's starter tool is already in slot one — the "already
    // installed" refusal must fire even with no carrier held, so it is
    // checked before the carrier check.
    let mut game = Game::new(9204, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let before_tools = game.world.get::<Tools>(player).unwrap().0.clone();

    let result = game.install_tool(&ToolId(tuning::STARTER_TOOL_ID.to_string()));

    let err = result.expect_err("an already-installed tool must refuse a second install");
    assert!(
        err.contains("already installed"),
        "got: {err}, expected the already-installed refusal ahead of the carrier check"
    );
    assert_eq!(game.world.get::<Tools>(player).unwrap().0, before_tools);
}

#[test]
fn install_refuses_when_no_free_slot_and_changes_nothing() {
    // Level 1 has exactly one slot, already filled by the starter — a
    // carrier is held so the only refusal that can fire is the slot cap.
    let mut game = Game::new(9205, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    hold_carrier(&mut game, "core_tap", 1);
    let before_tools = game.world.get::<Tools>(player).unwrap().0.clone();
    let before_inv = game.world.get::<Inventory>(player).unwrap().items.clone();

    let result = game.install_tool(&ToolId("core_tap".to_string()));

    assert!(result.is_err(), "a full loadout must refuse installing");
    assert_eq!(game.world.get::<Tools>(player).unwrap().0, before_tools);
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().items,
        before_inv
    );
}

#[test]
fn install_refuses_when_no_carrier_is_held_and_changes_nothing() {
    // A level step opens a second slot, but nothing was forged into it.
    let mut game = Game::new(9206, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Experience>(player).unwrap().level = tuning::TOOL_SLOT_PER_LEVEL;
    let before_tools = game.world.get::<Tools>(player).unwrap().0.clone();

    let result = game.install_tool(&ToolId("core_tap".to_string()));

    assert!(
        result.is_err(),
        "installing with no carrier held must be refused"
    );
    assert_eq!(game.world.get::<Tools>(player).unwrap().0, before_tools);
}

#[test]
fn installing_a_tool_burns_its_carrier_and_fills_the_slot() {
    let mut game = Game::new(9207, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.get_mut::<Experience>(player).unwrap().level = tuning::TOOL_SLOT_PER_LEVEL;
    hold_carrier(&mut game, "core_tap", 1);

    game.install_tool(&ToolId("core_tap".to_string()))
        .expect("nothing here refuses the install");

    assert_eq!(
        game.world.get::<Tools>(player).unwrap().0,
        vec![
            ToolId(tuning::STARTER_TOOL_ID.to_string()),
            ToolId("core_tap".to_string())
        ],
        "the new tool must land in the next free slot, after the starter"
    );
    assert_eq!(
        carrier_count(&game, &ToolId("core_tap".to_string())),
        0,
        "the carrier is spent, not kept alongside the installed tool"
    );
}

#[test]
fn installing_a_second_tool_only_succeeds_once_a_level_step_opens_a_slot() {
    let mut game = Game::new(9208, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    hold_carrier(&mut game, "core_tap", 1);
    game.world.get_mut::<Experience>(player).unwrap().level = tuning::TOOL_SLOT_PER_LEVEL - 1;

    assert!(
        game.install_tool(&ToolId("core_tap".to_string())).is_err(),
        "one level short of the step, the loadout must still be full"
    );

    game.world.get_mut::<Experience>(player).unwrap().level = tuning::TOOL_SLOT_PER_LEVEL;

    assert!(
        game.install_tool(&ToolId("core_tap".to_string())).is_ok(),
        "at the step itself, the newly opened slot must accept the held carrier"
    );
}

#[test]
fn uninstall_refuses_after_game_over_and_changes_nothing() {
    let mut game = Game::new(9209, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    game.world.resource_mut::<GameOver>().reason = Some("done".to_string());
    let before_tools = game.world.get::<Tools>(player).unwrap().0.clone();

    let result = game.uninstall_tool(0);

    assert!(result.is_err(), "a game-over run must refuse uninstalling");
    assert_eq!(game.world.get::<Tools>(player).unwrap().0, before_tools);
}

#[test]
fn uninstall_refuses_an_out_of_range_slot_and_changes_nothing() {
    let mut game = Game::new(9210, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let before_tools = game.world.get::<Tools>(player).unwrap().0.clone();

    let result = game.uninstall_tool(5);

    assert!(result.is_err(), "an empty slot must refuse uninstalling");
    assert_eq!(game.world.get::<Tools>(player).unwrap().0, before_tools);
}

#[test]
fn uninstalling_frees_the_slot_and_grants_no_carrier_back() {
    let mut game = Game::new(9211, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert_eq!(
        game.world.get::<Tools>(player).unwrap().0,
        vec![ToolId(tuning::STARTER_TOOL_ID.to_string())],
        "test premise: the starter tool starts in the one slot"
    );

    game.uninstall_tool(0)
        .expect("nothing here refuses pulling an installed tool");

    assert!(
        game.world.get::<Tools>(player).unwrap().0.is_empty(),
        "the slot must be freed"
    );
    assert_eq!(
        carrier_count(&game, &ToolId(tuning::STARTER_TOOL_ID.to_string())),
        0,
        "what was in the slot IS the tool — uninstalling must not hand a carrier back"
    );
}

// ---------------------------------------------------------------------------
// Phase 2, task 5: `Game::tool_rows`.
// ---------------------------------------------------------------------------

#[test]
fn tool_rows_lists_the_installed_starter_tool() {
    // The starter is both installed (`Game::new`, into slot one) and known
    // (`knows_tool`'s unconditional `STARTER_TOOL_ID` answer, Critical 1's
    // fix) — either alone would already put a row on screen; this pins the
    // row's own figures regardless of which side of the union wins.
    let game = Game::new(9301, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let starter = ToolId(tuning::STARTER_TOOL_ID.to_string());
    assert!(
        game.knows_tool(&starter),
        "test premise: the starter is known unconditionally"
    );

    let rows = game.tool_rows();
    let row = rows
        .iter()
        .find(|r| r.id == starter)
        .expect("the installed starter tool must have a row");
    assert_eq!(row.slot, Some(0));
    assert_eq!(row.carriers_held, 0);
    assert_eq!(row.name, "Salvage Clamp");
    assert_eq!(row.tier, 1);
}

#[test]
fn tool_rows_lists_a_known_but_unforged_tool_with_no_slot_and_no_carrier() {
    let mut game = Game::new(9302, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let core_tap = ToolId("core_tap".to_string());
    game.world
        .resource_mut::<KnownTools>()
        .0
        .insert(core_tap.clone());

    let rows = game.tool_rows();
    let row = rows
        .iter()
        .find(|r| r.id == core_tap)
        .expect("a known tool must have a row even with nothing forged");
    assert_eq!(row.slot, None, "nothing has installed it yet");
    assert_eq!(row.carriers_held, 0, "nothing has been forged yet");
    assert_eq!(row.category, ToolCategory::Cores);
    assert_eq!(row.tier, 2);
}

#[test]
fn tool_rows_reports_a_held_carrier_that_is_not_yet_installed() {
    let mut game = Game::new(9303, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let core_tap = ToolId("core_tap".to_string());
    game.world
        .resource_mut::<KnownTools>()
        .0
        .insert(core_tap.clone());
    hold_carrier(&mut game, "core_tap", 3);

    let row = game
        .tool_rows()
        .into_iter()
        .find(|r| r.id == core_tap)
        .expect("a known tool with a held carrier must have a row");
    assert_eq!(
        row.slot, None,
        "holding a carrier is not the same as installing it"
    );
    assert_eq!(row.carriers_held, 3);
}

#[test]
fn tool_rows_is_sorted_by_id_for_a_deterministic_screen() {
    let mut game = Game::new(9304, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<KnownTools>()
        .0
        .insert(ToolId("core_tap".to_string()));

    let ids: Vec<String> = game
        .tool_rows()
        .into_iter()
        .map(|r| r.id.as_str().to_string())
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(
        ids, sorted,
        "the same set drawn twice must list in the same order"
    );
    assert_eq!(
        ids,
        vec!["core_tap".to_string(), "salvage_clamp".to_string()],
        "core_tap sorts before the installed starter, salvage_clamp"
    );
}

#[test]
fn tool_rows_drops_a_known_tool_id_the_catalogue_cannot_resolve() {
    // A mod's tool file removed since a save referenced it — `installed_tools`'
    // own tolerance (there is no per-tool fallback to show in its place),
    // ported to the known-but-unresolvable case.
    let mut game = Game::new(9305, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.world
        .resource_mut::<KnownTools>()
        .0
        .insert(ToolId("no_such_tool".to_string()));

    let rows = game.tool_rows();
    assert!(
        rows.iter().all(|r| r.id.as_str() != "no_such_tool"),
        "an id ToolDb cannot resolve must not produce a row"
    );
}

/// Minor 9: `TOOL_SLOT_CAP` only bounds what is *installed*; a modded
/// research tree can teach more tools than any shipped one does, so
/// `tool_rows` needs its own ceiling — the screen has no scroll.
#[test]
fn tool_rows_never_exceeds_max_tool_rows_however_many_are_known() {
    let mut game = Game::new(9306, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let extra = tuning::MAX_TOOL_ROWS + 5;
    let files: Vec<(String, String)> = (0..extra)
        .map(|i| {
            let id = format!("bound_tool_{i}");
            (
                id.clone(),
                format!(
                    r#"(
                        id: "{id}",
                        name: "Bound Tool {i}",
                        description: "d",
                        category: Materials,
                        yields: [("core_fragment", 1.0)],
                        tier: 1,
                        ticks: 1,
                    )"#
                ),
            )
        })
        .collect();
    let file_refs: Vec<(&str, &str)> = files
        .iter()
        .map(|(id, body)| (id.as_str(), body.as_str()))
        .collect();
    let (tool_db, warnings) = load_tools("row_bound", &file_refs);
    assert!(
        warnings.is_empty(),
        "fixture tools must load clean: {warnings:?}"
    );
    game.world.insert_resource(tool_db);
    let mut known = game.world.resource_mut::<KnownTools>();
    for (id, _) in &files {
        known.0.insert(ToolId(id.clone()));
    }

    let rows = game.tool_rows();

    assert_eq!(
        rows.len(),
        tuning::MAX_TOOL_ROWS,
        "{extra} known tools (plus the starter) must be trimmed to the row bound"
    );
}

// ---------------------------------------------------------------------------
// Phase 3: the extraction bench — `StructureDef::extracts_programs` and what
// its tier is worth to a yield. See the phase-3 plan's Task 1.
// ---------------------------------------------------------------------------

/// The plan's fixture game, so the phase-3 tests below all fit the same
/// world: a fresh run on shipped assets, where no bench stands and
/// `extraction_bench_tier` is therefore zero until one is built.
fn new_test_game() -> Game {
    Game::new(4490, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A downed program of `species` at `level`, median condition and ordinary
/// rarity — the two axes the bench tests hold fixed, since only the bench's
/// term is under test here.
fn test_program(species: &str, level: u32) -> DownedProgram {
    DownedProgram {
        species: species.to_string(),
        level,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: tuning::CONDITION_BASE,
    }
}

fn starter_tool(game: &Game) -> ToolDef {
    starter_tool_def(game)
}

fn give_downed_program(game: &mut Game, program: DownedProgram) {
    let player = game.player_entity();
    game.world
        .get_mut::<DownedPrograms>(player)
        .unwrap()
        .0
        .push(program);
}

/// What the player holds of each item a quoted yield names, before the act —
/// so a granted row can be compared against the quote item by item rather
/// than as a total that a compensating error could survive.
fn held_counts(game: &Game, quoted: &[(ItemId, u32)]) -> std::collections::BTreeMap<ItemId, u32> {
    quoted
        .iter()
        .map(|(item, _)| (item.clone(), held(game, item)))
        .collect()
}

/// Stands whichever structure `Game::extraction_bench_tier` looks for, at
/// `tier` (`None` for a structure that has never been upgraded, which is
/// what `build_structure` leaves behind — see `best_structure_tier`'s doc
/// on why a missing `StructureTier` reads as tier 1).
fn build_program_bench(game: &mut Game, tier: Option<u32>) {
    let bench = game
        .world
        .resource::<StructureDb>()
        .all()
        .find(|def| def.extracts_programs)
        .map(|def| def.id.clone())
        .expect("some shipped structure extracts programs");
    let entity = spawn_structure_at(game, &bench, 30, 30);
    if let Some(t) = tier {
        game.world.entity_mut(entity).insert(StructureTier(t));
    }
}

/// The whole of decision 2's first half: a Compiler that has never been
/// upgraded is worth nothing to a yield. Without this the fresh-bench case
/// silently pays `TOOL_TIER_SCALE_STEP`'s full step, which on the shipped
/// `0.5` is +50% of the entire material economy for a structure most bases
/// already have.
#[test]
fn a_fresh_bench_does_not_change_a_yield() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 5);
    let tool = starter_tool(&game);
    let before = game.extraction_yield(&program, &tool);

    build_program_bench(&mut game, None);

    assert_eq!(game.extraction_bench_tier(), 1, "the bench is standing");
    assert_eq!(
        game.extraction_yield(&program, &tool),
        before,
        "tier 1 is the identity — only upgrades sell yield"
    );
}

#[test]
fn an_upgraded_bench_raises_a_yield_by_the_shared_tier_curve() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 5);
    let tool = starter_tool(&game);
    let before: u32 = game
        .extraction_yield(&program, &tool)
        .iter()
        .map(|(_, qty)| qty)
        .sum();

    build_program_bench(&mut game, Some(5));

    let after: u32 = game
        .extraction_yield(&program, &tool)
        .iter()
        .map(|(_, qty)| qty)
        .sum();
    assert!(
        after > before,
        "tier 5 must pay more than no bench at all: {after} vs {before}"
    );
}

/// Decision 3: the tier is read inside `extraction_yield`, so the figure
/// the screen quotes through `extraction_options` and the figure
/// `extract_program` grants move together. A parameter is how those two
/// come apart.
#[test]
fn the_previewed_yield_tracks_the_bench_tier() {
    let mut game = new_test_game();
    give_downed_program(&mut game, test_program("scrapper", 5));
    build_program_bench(&mut game, Some(4));

    let previewed = game.extraction_options(0);
    let (tool_id, quoted) = previewed.first().cloned().expect("the starter tool");
    let held_before = held_counts(&game, &quoted);

    game.extract_program(0, &tool_id)
        .expect("the extraction runs");

    for (item, qty) in &quoted {
        assert_eq!(
            held(&game, item),
            held_before.get(item).copied().unwrap_or(0) + qty,
            "granted {item} does not match the quoted {qty}"
        );
    }
}

/// The tick half of the bench, `GameClock` being this build's tick counter —
/// `tests::wielded`'s own idiom for measuring what an act actually spends.
fn ticks_elapsed(game: &Game) -> u64 {
    game.world.resource::<crate::resources::GameClock>().tick
}

#[test]
fn a_standing_bench_makes_an_extraction_faster() {
    let mut game = new_test_game();
    let tool = starter_tool(&game);
    let bare = game.extraction_ticks(&tool);
    assert_eq!(bare, tool.ticks, "no bench is the tool's own figure");

    build_program_bench(&mut game, None);

    assert!(
        game.extraction_ticks(&tool) < bare,
        "a fresh bench must already be worth time: {} vs {bare}",
        game.extraction_ticks(&tool)
    );
}

#[test]
fn an_upgraded_bench_is_faster_still_and_never_free() {
    let mut game = new_test_game();
    let tool = starter_tool(&game);
    build_program_bench(&mut game, Some(1));
    let tier_one = game.extraction_ticks(&tool);
    build_program_bench(&mut game, Some(5));
    let tier_five = game.extraction_ticks(&tool);

    assert!(tier_five < tier_one, "{tier_five} vs {tier_one}");
    assert!(tier_five >= 1, "an extraction never costs zero time");
}

/// The tick cost is what the act actually spends, not just what a number
/// says. Without this the formula could be right and unwired.
#[test]
fn the_act_spends_the_bench_reduced_tick_cost() {
    let mut game = new_test_game();
    give_downed_program(&mut game, test_program("scrapper", 5));
    build_program_bench(&mut game, Some(5));
    let tool = starter_tool(&game);
    let expected = game.extraction_ticks(&tool);
    let before = ticks_elapsed(&game);

    game.extract_program(0, &tool.id)
        .expect("the extraction runs");

    assert_eq!(ticks_elapsed(&game) - before, expected);
}

// ---------------------------------------------------------------------------
// Phase 3: the routine branch — a `Routines` tool taking a routine out of a
// downed program, sharing `Game::take_routine` with the tamed-program door.
// See the phase-3 plan's Task 3.
// ---------------------------------------------------------------------------

/// The draw's fixture: one fresh run per seed, since `GameRng` is shared and
/// a second extraction in the same run would sample a stream the first one
/// already moved.
fn test_game_with_seed(seed: u32) -> Game {
    Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// Installs a `Routines`-category tool built here rather than found in the
/// catalogue: no shipped tool is in that category until task 4, deliberately,
/// so that no commit on this branch hands the player a tool whose branch is
/// not written yet. Task 4 replaces this with the shipped one.
fn install_routine_tool(game: &mut Game) {
    let def = ToolDef {
        id: ToolId("test_routine_reader".to_string()),
        name: "Test Routine Reader".to_string(),
        description: "A fixture for the routine branch.".to_string(),
        category: ToolCategory::Routines,
        yields: Vec::new(),
        tier: 1,
        ticks: 4,
        forge_cost: Vec::new(),
    };
    let id = def.id.clone();
    game.world.resource_mut::<ToolDb>().insert(def);
    let player = game.player_entity();
    game.world.get_mut::<Tools>(player).unwrap().0.push(id);
}

/// The installed `Routines` tool, found by its category — the
/// `build_program_bench` rule, so task 4 swapping the fixture for the shipped
/// Reader leaves every call site alone.
fn routine_tool_id(game: &Game) -> ToolId {
    game.installed_tools()
        .into_iter()
        .find(|def| def.category == ToolCategory::Routines)
        .map(|def| def.id)
        .expect("a Routines tool is installed")
}

/// A species whose whole kit is one exclusive routine, minted here because no
/// shipped species declares one — the exclusive pool is authored to reach the
/// player as an already-written disk off a boss
/// (`tests::exclusive_routines`), so the downed-program door has no shipped
/// case to fire on. A one-entry kit also makes the draw certain, which is
/// what lets the disk count be asserted at all.
fn species_declaring_an_exclusive_routine(game: &mut Game) -> (String, String) {
    let ability = game
        .world
        .resource::<AbilityDb>()
        .exclusive_pool()
        .first()
        .map(|def| def.id.clone())
        .expect("the shipped exclusive pool is empty");
    let mut def = game
        .world
        .resource::<SpeciesDb>()
        .get("scrapper")
        .expect("the scrapper ships")
        .clone();
    def.id = "exclusive_kit_fixture".to_string();
    def.abilities = vec![crate::species::SpeciesAbility {
        id: ability.clone(),
        level: 1,
    }];
    let species = def.id.clone();
    game.world.resource_mut::<SpeciesDb>().insert(def);
    (species, ability)
}

/// The pool is the species' own kit at the program's level, minus what the
/// player already knows. A level gate that did not apply would teach a
/// level-30 routine off a level-2 kill.
#[test]
fn the_routine_pool_is_the_species_kit_at_that_level() {
    let game = new_test_game();
    let low = game.routine_candidates(&test_program("scrapper", 1));
    let high = game.routine_candidates(&test_program("scrapper", 30));
    assert!(
        low.len() <= high.len(),
        "a higher-level program cannot offer fewer routines: {low:?} vs {high:?}"
    );
    for id in &low {
        assert!(high.contains(id), "{id} vanished at a higher level");
    }
}

#[test]
fn a_routine_already_known_leaves_the_pool() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 30);
    let first = game
        .routine_candidates(&program)
        .first()
        .cloned()
        .expect("the scrapper declares a routine");
    game.world
        .resource_mut::<KnownRoutines>()
        .0
        .insert(first.clone());

    assert!(
        !game.routine_candidates(&program).contains(&first),
        "a known routine is still being offered"
    );
}

/// The refusal, asserted the way every other refusal in this feature is:
/// nothing spent. A program consumed for a routine the player already had is
/// the exact waste `extract_routine`'s own "already known" check exists to
/// prevent.
#[test]
fn a_routine_tool_with_nothing_left_to_teach_refuses_and_spends_nothing() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 30);
    for id in game.routine_candidates(&program) {
        game.world.resource_mut::<KnownRoutines>().0.insert(id);
    }
    give_downed_program(&mut game, program);
    install_routine_tool(&mut game);
    let tool = routine_tool_id(&game);
    let ticks_before = ticks_elapsed(&game);

    let refusal = game.extract_program(0, &tool);

    assert!(refusal.is_err(), "it should have refused");
    assert_eq!(game.downed_program_rows().len(), 1, "the program was spent");
    assert_eq!(ticks_elapsed(&game), ticks_before, "time was spent");
}

#[test]
fn a_routine_tool_teaches_a_routine_and_consumes_the_program() {
    let mut game = new_test_game();
    let program = test_program("scrapper", 30);
    let pool = game.routine_candidates(&program);
    give_downed_program(&mut game, program);
    install_routine_tool(&mut game);
    let tool = routine_tool_id(&game);

    game.extract_program(0, &tool).expect("the extraction runs");

    assert!(
        game.downed_program_rows().is_empty(),
        "the program survived"
    );
    assert!(
        pool.iter().any(|id| game.knows_routine(id)),
        "nothing from the pool was learned"
    );
}

/// The invariant the whole exclusive pool rests on, restated for the new
/// door: an exclusive routine taken off a downed program leaves exactly one
/// copy in the run — the disk — and teaches nothing.
#[test]
fn an_exclusive_routine_from_a_downed_program_leaves_exactly_one_copy() {
    let mut game = new_test_game();
    let (species, ability) = species_declaring_an_exclusive_routine(&mut game);
    let program = test_program(&species, 30);
    give_downed_program(&mut game, program);
    install_routine_tool(&mut game);
    let tool = routine_tool_id(&game);

    game.extract_program(0, &tool).expect("the extraction runs");

    assert!(!game.knows_routine(&ability), "an exclusive was learned");
    assert_eq!(
        held(&game, &ItemId::etched(&ability)),
        1,
        "exactly one disk, no more and no fewer"
    );
}

/// The first entry is favoured, not guaranteed — decision 4. Run the draw
/// enough times to see both outcomes; a deterministic pick would fail this
/// and a uniform one would fail the skew assertion.
#[test]
fn the_draw_favours_the_first_candidate_without_forcing_it() {
    let mut counts: std::collections::BTreeMap<String, u32> = Default::default();
    let mut first_id = None;
    for seed in 0..200u32 {
        let mut game = test_game_with_seed(seed);
        let program = test_program("scrapper", 30);
        let pool = game.routine_candidates(&program);
        if pool.len() < 2 {
            return; // nothing to skew between; the pool census covers this
        }
        first_id.get_or_insert(pool[0].clone());
        give_downed_program(&mut game, program);
        install_routine_tool(&mut game);
        let tool = routine_tool_id(&game);
        game.extract_program(0, &tool).expect("the extraction runs");
        for id in pool {
            if game.knows_routine(&id) || held(&game, &ItemId::etched(&id)) > 0 {
                *counts.entry(id.to_string()).or_default() += 1;
            }
        }
    }
    let first = first_id.expect("a pool");
    let favoured = counts.get(first.as_str()).copied().unwrap_or(0);
    let rest: u32 = counts
        .iter()
        .filter(|(id, _)| id.as_str() != first.as_str())
        .map(|(_, n)| n)
        .sum();
    assert!(
        favoured > 0 && rest > 0,
        "the draw is deterministic: {counts:?}"
    );
    assert!(
        favoured * 2 > rest,
        "the first candidate is not favoured: {counts:?}"
    );
}
