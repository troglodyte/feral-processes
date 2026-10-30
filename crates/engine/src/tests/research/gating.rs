use super::*;

/// Everything compilable from turn one: the three consumable starters. The
/// Scavenged gear tier needs a Workbench. Anything else must be gated behind
/// research, a
/// bench, or both — so this set is pinned rather than counted.
#[test]
fn only_the_starters_need_no_research_or_bench() {
    let game = Game::new(80, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut results: Vec<String> = game
        .craft_recipes()
        .into_iter()
        .map(|r| r.result.as_str().to_string())
        .collect();
    results.sort();
    assert_eq!(
        results,
        ["ice_breaker", "outlet", "power_cell"],
        "nothing else is free"
    );
}

#[test]
fn a_researched_recipe_stays_hidden_until_its_bench_is_built() {
    let mut game = Game::new(81, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "overclock");

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(
        !results.contains(&ItemId::from(ids::OVERCLOCK_CORE)),
        "the blueprint alone isn't enough — you still need the Fabricator"
    );

    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "workbench", 0, 1).unwrap();

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(results.contains(&ItemId::from(ids::OVERCLOCK_CORE)));
}

#[test]
fn a_built_bench_alone_does_not_unlock_its_recipe() {
    let mut game = Game::new(82, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "weapon_bench");
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "workbench", 0, 1).unwrap();

    let results: Vec<ItemId> = game.craft_recipes().into_iter().map(|r| r.result).collect();
    assert!(
        !results.contains(&ItemId::from(ids::OVERCLOCK_CORE)),
        "the Fabricator is a bench now, not an unlock"
    );
}

/// The Standard/Premium gear tiers declare their own recipe with a
/// `requires_structure` bench and no research node of their own. Building
/// the bench is the whole unlock — but it is still a real gate.
#[test]
fn an_item_declared_recipe_stays_hidden_until_its_bench_is_built() {
    let mut game = Game::new(90, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    let probe = ItemId::from("logic_probe");

    assert!(
        !game.craft_recipes().iter().any(|r| r.result == probe),
        "a bench-gated item recipe must not be free from turn one"
    );

    // The Fabricator itself is research-gated; that gates the bench, not
    // the recipe, which has no research node of its own.
    unlock_research_chain(&mut game, "weapon_bench");
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "workbench", 0, 1).unwrap();

    assert!(
        game.craft_recipes().iter().any(|r| r.result == probe),
        "standing the bench should be enough — no research node names this recipe"
    );
}

/// Gear sources can be declared from either side. Both are honoured, an
/// item named twice is rolled once at the better chance, and the list is
/// ordered so a seeded run always spends its rolls the same way.
#[test]
fn equipment_drops_merge_both_declaration_sides_taking_the_better_chance() {
    let game = Game::new(91, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut scrapper = game
        .species_defs()
        .into_iter()
        .find(|s| s.id == "scrapper")
        .expect("scrapper ships");
    let arc_lance = ItemId::from("arc_lance");
    let chance_of = |drops: &[(ItemId, f32)], id: &ItemId| {
        drops
            .iter()
            .find(|(i, _)| i == id)
            .unwrap_or_else(|| panic!("{} should be droppable here", id.as_str()))
            .1
    };

    // Item side alone: arc_lance.ron names scrapper.
    let drops = game.equipment_drops_for(&scrapper);
    assert_eq!(chance_of(&drops, &arc_lance), 0.1);
    let mut sorted = drops.clone();
    sorted.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
    assert_eq!(drops, sorted, "roll order must be deterministic");

    // Declared on both sides: one entry, the better chance.
    scrapper.equipment_drop = Some((arc_lance.clone(), 0.5));
    let drops = game.equipment_drops_for(&scrapper);
    assert_eq!(
        drops.iter().filter(|(i, _)| *i == arc_lance).count(),
        1,
        "declared twice, rolled once"
    );
    assert_eq!(chance_of(&drops, &arc_lance), 0.5);

    // The weaker of the two loses, whichever side it came from.
    scrapper.equipment_drop = Some((arc_lance.clone(), 0.02));
    let drops = game.equipment_drops_for(&scrapper);
    assert_eq!(chance_of(&drops, &arc_lance), 0.1);
}

/// A species-side `equipment_drop` is legacy but still supported, so a
/// third-party species mod that predates item-side `droppable` keeps
/// dropping what it always did.
#[test]
fn a_species_side_equipment_drop_still_works_on_its_own() {
    let game = Game::new(92, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let mut sprite = game
        .species_defs()
        .into_iter()
        .find(|s| s.id == "sprite")
        .expect("sprite ships");
    // Nothing names power_cell in a `droppable`, so this can only arrive
    // from the species side.
    let power_cell = ItemId::from(ids::POWER_CELL);
    sprite.equipment_drop = Some((power_cell.clone(), 0.25));

    let drops = game.equipment_drops_for(&sprite);
    assert!(drops.contains(&(power_cell, 0.25)));
}

#[test]
fn a_researched_recipe_carries_the_cost_from_its_ron_file() {
    let mut game = Game::new(83, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    unlock_research_chain(&mut game, "overclock");
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    place_now(&mut game, "workbench", 0, 1).unwrap();

    assert_eq!(
        game.craft_cost(&ItemId::from(ids::OVERCLOCK_CORE), false),
        vec![
            (ItemId::from(ids::PORTAL_FRAGMENT), 6),
            (ItemId::from("cache_grain"), 2),
        ]
    );
}

#[test]
fn the_refit_bench_is_buildable_only_once_program_refactoring_is_researched() {
    let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    place_home(&mut game);
    stand_in_base(&mut game);
    game.world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 100);
    assert!(!game.structure_unlocked("refit_bench"));
    let refusal = place_now(&mut game, "refit_bench", 1, 0).unwrap_err();
    assert!(refusal.contains("hasn't been researched yet."), "{refusal}");

    unlock_research_chain(&mut game, "program_refactoring");

    assert!(game.structure_unlocked("refit_bench"));
    place_now(&mut game, "refit_bench", 1, 0).unwrap();
}

#[test]
fn a_structure_named_by_no_research_file_is_buildable_from_the_start() {
    let game = Game::new(70, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let ids: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    let mut sorted = ids.clone();
    sorted.sort();
    assert_eq!(
        sorted,
        vec![
            "assembler".to_string(),
            "breeding_bay".to_string(),
            "contract_broker".to_string(),
            "data_cache".to_string(),
            "defrag_bay".to_string(),
            "depot".to_string(),
            "door".to_string(),
            "extractor".to_string(),
            "factory".to_string(),
            "home".to_string(),
            "portal".to_string(),
            "printer".to_string(),
            "quarantine_rack".to_string(),
            "recharger_node".to_string(),
            // Ungated on purpose: recovering from a wipe is not a blueprint
            // you have to earn, and a Bay you cannot build in the zone where
            // the first wipe happens is a dead run rather than pressure.
            "repair_bay".to_string(),
            "research_node".to_string(),
            "sandbox".to_string(),
            // Ungated on purpose: shaping the base against a siege is not
            // something to earn.
            "wall".to_string(),
            "workbench".to_string(),
        ],
        "exactly the structures named by no research file start available"
    );
}

#[test]
fn a_research_gated_structure_is_hidden_from_the_build_menu_until_researched() {
    let mut game = Game::new(71, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let hidden: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    assert!(!hidden.contains(&"line_driver".to_string()));

    unlock_research_chain(&mut game, "cache_coherence");

    let shown: Vec<String> = game
        .buildable_structure_defs()
        .into_iter()
        .map(|d| d.id)
        .collect();
    assert!(shown.contains(&"line_driver".to_string()));
}

#[test]
fn placing_an_unresearched_structure_is_rejected_even_when_called_directly() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_in_base(&mut game);
    place_home(&mut game);
    let player = game.player_entity();
    game.world
        .get_mut::<Inventory>(player)
        .unwrap()
        .add(ItemId::from(ids::CORE_FRAGMENT), 200);
    let err = place_now(&mut game, "line_driver", 0, 1).unwrap_err();
    assert!(
        err.contains("researched"),
        "filtering the menu is not a gate: {err}"
    );
}

#[test]
fn nothing_is_researched_at_the_start_of_a_game() {
    let game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.is_researched("automation"));
    assert!(
        game.research_nodes(ResearchTree::Base)
            .iter()
            .all(|n| n.state != ResearchState::Unlocked),
        "a fresh game starts with an entirely locked tree"
    );
}
