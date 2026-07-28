//! Routines: the slots abilities occupy, and how they get there.

use super::support::*;
use crate::components::Routines;
use crate::*;

/// The generic test species declares no abilities, so its kit is the
/// fallback — which must be a real installed routine, not an empty list
/// resolved at read time.
#[test]
fn a_tamed_program_with_no_species_kit_starts_with_the_fallback_installed() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    let installed = game
        .world
        .get::<Routines>(pet)
        .expect("a tamed program has routines");
    assert_eq!(
        installed.0,
        vec![crate::abilities::FALLBACK_ABILITY_ID.to_string()],
        "a species declaring no abilities implicitly installs the fallback"
    );
}

#[test]
fn a_species_kit_is_installed_at_tame_time_in_declared_order() {
    let (game, medic) = game_with_two_ability_companion();
    let installed = &game.world.get::<Routines>(medic).unwrap().0;
    assert_eq!(
        installed,
        &vec!["hot_patch".to_string()],
        "the level-1 unlock installed is TWO_ABILITY_SPECIES' declared id, \
         not just any single routine: {installed:?}"
    );
}

/// Regression for the shipped Scrapper: its only ability, `cascade_overflow`,
/// unlocks at level 3 and nothing unlocks at level 1, so a freshly tamed one
/// starts on the fallback. Before the eviction fix, the level-3 unlock found
/// that one slot "full" (of the fallback) and was logged as lost forever —
/// this pins the real fix down against the actual shipped asset rather than
/// a fixture, since no test fixture happened to reproduce the shape.
#[test]
fn a_scrapper_levelling_to_its_unlock_gets_cascade_overflow_instead_of_a_stuck_fallback() {
    let mut game = Game::new(61, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let scrapper = game
        .world
        .spawn((
            Creature {
                species: "scrapper".to_string(),
            },
            Position { x: 3, y: 3 },
            Stats {
                hp: 50,
                max_hp: 50,
                atk: 5,
                def: 2,
            },
            Tamed { owner: player },
            Experience::default(),
        ))
        .id();
    game.install_innate_routines(scrapper);
    assert_eq!(
        game.world.get::<Routines>(scrapper).unwrap().0,
        vec![crate::abilities::FALLBACK_ABILITY_ID.to_string()],
        "a level-1 Scrapper has no unlock yet, so it starts on the fallback"
    );

    set_level(&mut game, scrapper, 3);
    assert_eq!(
        game.world.get::<Routines>(scrapper).unwrap().0,
        vec!["cascade_overflow".to_string()],
        "reaching the level-3 unlock must evict the fallback, not lose the unlock"
    );
}

#[test]
fn a_level_up_that_reaches_an_unlock_installs_it_into_a_free_slot() {
    let (mut game, medic) = game_with_two_ability_companion();
    let before = game.world.get::<Routines>(medic).unwrap().0.len();
    // `TWO_ABILITY_SPECIES` gates `sandbox` at level 5, and level 5 is worth
    // two slots, so the unlock has somewhere to land.
    set_level(&mut game, medic, 5);
    let after = &game.world.get::<Routines>(medic).unwrap().0;
    assert_eq!(
        after.len(),
        before + 1,
        "reaching the second unlock should install it: {after:?}"
    );
}

/// Regression for the gap the re-review found in the C1 fix: eviction
/// matches on ability id alone, so it fires identically whether the matched
/// slot holds the auto-installed placeholder or a Priority Boost the player
/// chose to install by hand — and the latter used to be evicted with no log
/// line at all, unlike the neighbouring "it goes to cargo" branch.
#[test]
fn evicting_a_manually_installed_priority_boost_is_logged() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 4); // two slots: hot_patch, plus one free
    let boost_item = crate::abilities::routine_item_id(crate::abilities::FALLBACK_ABILITY_ID);
    set_inventory(&mut game, &[(boost_item.as_str(), 1)]);
    game.install_routine(medic, &boost_item).unwrap();
    assert_eq!(
        game.world.get::<Routines>(medic).unwrap().0,
        vec!["hot_patch".to_string(), "priority_boost".to_string()],
        "a deliberate install, not the tame-time fallback"
    );

    set_level(&mut game, medic, 5); // sandbox unlocks; both slots are full

    assert_eq!(
        game.world.get::<Routines>(medic).unwrap().0,
        vec!["hot_patch".to_string(), "sandbox".to_string()],
        "the unlock still evicts the id-matched slot, manual install or not"
    );
    assert!(
        game.message_log(10).iter().any(|(_, text)| {
            text.contains("swaps out")
                && text.contains("Priority Boost")
                && text.contains("Sandbox")
        }),
        "the eviction of a deliberately installed routine must be logged, \
         not just the auto-installed placeholder's: {:?}",
        game.message_log(10)
    );
}

#[test]
fn game_routine_slots_dispatches_to_the_companion_or_player_curve_by_entity() {
    // The actual number-per-level curves are pinned in `abilities.rs`'
    // `companion_slots_grow_one_per_two_levels_up_to_the_cap` and
    // `player_slots_grow_one_per_ten_levels_so_the_first_free_one_lands_at_10`;
    // this only needs to show `Game::routine_slots` asks the right one of the
    // two depending on which entity it's asked about.
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 6);
    set_level(&mut game, player, 6);
    assert_eq!(
        game.routine_slots(pet),
        crate::abilities::companion_routine_slots(6),
    );
    assert_eq!(
        game.routine_slots(player),
        crate::abilities::player_routine_slots(6),
    );
    assert_ne!(
        crate::abilities::companion_routine_slots(6),
        crate::abilities::player_routine_slots(6),
        "the two curves must actually differ for this test to mean anything"
    );
}

#[test]
fn installed_routines_survive_a_save_load_round_trip() {
    let mut game = Game::new(13, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    // A generic-species program's kit is just the fallback, which a loader
    // that rebuilt `Routines` from `SpeciesDef` on load (rather than
    // actually persisting it) would reproduce too. Installing a foreign
    // routine here means the save format is the only thing that can be
    // carrying it — the same shape as the player-side twin of this test.
    set_level(&mut game, pet, 4); // two slots, one free
    let item = crate::abilities::routine_item_id("sandbox");
    set_inventory(&mut game, &[(item.as_str(), 1)]);
    game.install_routine(pet, &item).unwrap();
    let before = game.world.get::<Routines>(pet).unwrap().0.clone();
    assert_eq!(before.len(), 2, "fallback plus the foreign routine");
    let path = std::env::temp_dir().join(format!(
        "feral_routines_roundtrip_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let restored = loaded
        .owned_pets()
        .first()
        .map(|p| loaded.world.get::<Routines>(p.entity).unwrap().0.clone())
        .expect("the tamed program should come back");
    assert_eq!(restored, before, "a save must carry installed routines");
}

/// Regression for M14: a save can name an ability id the currently loaded
/// `AbilityDb` no longer has (the mod that added it was uninstalled). Left
/// unfiltered, that id would survive into `Routines` as a ghost
/// `routine_view` renders `(empty)` but `installed.len()` still counts
/// against the slot cap — a slot the panel calls free that can never
/// actually be filled again.
#[test]
fn a_routine_naming_a_since_removed_ability_is_dropped_on_load_with_a_warning() {
    let dir = std::env::temp_dir().join(format!("feral_ghost_routine_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    for sub in ["species", "structures", "research", "items", "abilities"] {
        let dst = dir.join(sub);
        std::fs::create_dir_all(&dst).unwrap();
        for entry in std::fs::read_dir(test_assets_dir().join(sub)).unwrap() {
            let entry = entry.unwrap();
            std::fs::copy(entry.path(), dst.join(entry.file_name())).unwrap();
        }
    }
    let ghost_ability = r#"(
        id: "ghost_ability",
        name: "Ghost Ability",
        description: "d",
        target: OneAlly,
        effect: Heal(power: 1),
    )"#;
    std::fs::write(
        dir.join("abilities").join("ghost_ability.ron"),
        ghost_ability,
    )
    .unwrap();

    let mut game = Game::new(57, DifficultyMode::Forgiving, &dir).unwrap();
    let player = game.player_entity();
    set_level(&mut game, player, 10); // a free slot alongside decompile
    let ghost_item = crate::abilities::routine_item_id("ghost_ability");
    set_inventory(&mut game, &[(ghost_item.as_str(), 1)]);
    game.install_routine(player, &ghost_item).unwrap();

    let path = std::env::temp_dir().join(format!(
        "feral_ghost_routine_save_{}.bin",
        std::process::id()
    ));
    game.save(&path).unwrap();

    // The mod is "uninstalled": the ability file is gone before the reload.
    std::fs::remove_file(dir.join("abilities").join("ghost_ability.ron")).unwrap();
    let mut loaded = Game::load(&path, &dir).unwrap();
    let _ = std::fs::remove_file(&path);

    let player = loaded.player_entity();
    assert!(
        loaded
            .actor_abilities(player)
            .iter()
            .all(|a| a.id != "ghost_ability"),
        "the ghost id must not survive the load"
    );
    assert!(
        loaded
            .message_log(20)
            .iter()
            .any(|(_, text)| text.contains("no longer available")),
        "the drop must be logged, not silent: {:?}",
        loaded.message_log(20)
    );

    // The freed slot must be genuinely usable, not still counted against the
    // cap by a ghost entry the panel can no longer even show.
    let boost_item = crate::abilities::routine_item_id("priority_boost");
    set_inventory(&mut loaded, &[(boost_item.as_str(), 1)]);
    loaded
        .install_routine(player, &boost_item)
        .expect("the slot the ghost vacated must accept a real routine");

    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn every_loaded_ability_gets_a_routine_item_carrying_its_own_text() {
    let game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for ability in game.ability_defs() {
        let item = crate::abilities::routine_item_id(&ability.id);
        let def = game
            .item_def(&item)
            .unwrap_or_else(|| panic!("{} should have a synthesized routine item", ability.id));
        assert_eq!(def.routine.as_deref(), Some(ability.id.as_str()));
        assert_eq!(
            def.description, ability.description,
            "a routine item reads its text from the ability, never a copy"
        );
        assert!(def.name.ends_with(" Routine"), "{}", def.name);
    }
}

#[test]
fn install_then_uninstall_returns_the_same_item() {
    let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 4); // two slots, one of them free
    let item = crate::abilities::routine_item_id("sandbox");
    set_inventory(&mut game, &[(item.as_str(), 1)]);

    game.install_routine(pet, &item).unwrap();
    assert_eq!(
        count_item(&game, item.as_str()),
        0,
        "installing spends the item"
    );
    assert!(
        game.routine_view(pet)
            .iter()
            .any(|s| s.ability.as_deref() == Some("sandbox")),
        "the routine should occupy a slot"
    );

    let slot = game
        .routine_view(pet)
        .into_iter()
        .position(|s| s.ability.as_deref() == Some("sandbox"))
        .unwrap();
    game.uninstall_routine(pet, slot).unwrap();
    assert_eq!(
        count_item(&game, item.as_str()),
        1,
        "uninstalling gives it back"
    );
}

#[test]
fn install_is_refused_with_no_free_slot_without_the_item_and_during_battle() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3); // level 1: exactly one slot, already full
    let item = crate::abilities::routine_item_id("sandbox");

    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("don't have"), "no copy held: {err}");

    set_inventory(&mut game, &[(item.as_str(), 1)]);
    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("no free routine slot"), "slots full: {err}");

    set_level(&mut game, pet, 4);
    start_battle_with_a_wild_program(&mut game);
    let err = game.install_routine(pet, &item).unwrap_err();
    assert!(err.contains("right now"), "mid-battle: {err}");
}

#[test]
fn an_innate_routine_can_be_popped_out_and_plugged_into_another_program() {
    let (mut game, medic) = game_with_two_ability_companion();
    let popped = game.routine_view(medic)[0].ability.clone().unwrap();
    game.uninstall_routine(medic, 0).unwrap();
    assert!(
        game.routine_view(medic).iter().all(|s| s.ability.is_none()),
        "the innate slot should now be empty"
    );

    let host = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, host, 4);
    let item = crate::abilities::routine_item_id(&popped);
    game.install_routine(host, &item).unwrap();
    assert!(
        game.routine_view(host)
            .iter()
            .any(|s| s.ability.as_deref() == Some(popped.as_str())),
        "a foreign species' routine should install fine"
    );
}

/// Regression for M4: a save-loaded wild creature carries `Routines(vec![])`
/// too (`lifecycle.rs` inserts it in the common creature bundle before the
/// `if c.tamed` branch), so without an ownership check `install_routine`
/// and `uninstall_routine` would silently accept an entity no menu ever
/// offers — unlike `extract_routine` and `sell_companion`, which both
/// already refuse a program the player doesn't own.
#[test]
fn install_and_uninstall_routine_are_refused_for_a_program_you_dont_own() {
    let mut game = Game::new(56, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player_pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let wild = game
        .world
        .spawn((
            Creature {
                species: game.species_defs().into_iter().next().unwrap().id,
            },
            Hostile,
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Stats {
                hp: 10,
                max_hp: 10,
                atk: 0,
                def: 1,
            },
            Routines(vec![]),
        ))
        .id();

    let item = crate::abilities::routine_item_id("sandbox");
    set_inventory(&mut game, &[(item.as_str(), 1)]);
    let err = game.install_routine(wild, &item).unwrap_err();
    assert!(err.contains("control"), "{err}");

    let err = game.uninstall_routine(wild, 0).unwrap_err();
    assert!(err.contains("control"), "{err}");
}

#[test]
fn extraction_needs_a_bench_built_somewhere_but_not_nearby() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    assert!(!game.can_extract_routines(), "no bench, no extraction");

    let pet = spawn_tamed(&mut game, 10, 3);
    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(
        err.contains("Compiler"),
        "the refusal should name the bench: {err}"
    );

    spawn_structure_at(&mut game, "compiler", 30, 30);
    assert!(
        game.can_extract_routines(),
        "a bench 30 tiles away still counts — extraction has no proximity rule"
    );
}

#[test]
fn extracting_yields_the_picked_routine_destroys_the_program_and_loses_the_rest() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 5); // both of its unlocks installed
    spawn_structure_at(&mut game, "compiler", 30, 30);

    let offered = game.extractable_routines(medic);
    assert_eq!(offered.len(), 2, "both installed routines are on offer");
    let kept = offered[1].id.clone();
    let lost = offered[0].id.clone();

    game.extract_routine(medic, 1).unwrap();

    assert_eq!(
        count_item(&game, crate::abilities::routine_item_id(&kept).as_str()),
        1,
        "the picked routine lands in inventory"
    );
    assert_eq!(
        count_item(&game, crate::abilities::routine_item_id(&lost).as_str()),
        0,
        "everything else on the program is lost with it"
    );
    assert!(
        game.owned_pets().iter().all(|p| p.entity != medic),
        "the program is consumed"
    );
}

#[test]
fn extraction_is_refused_for_a_program_you_dont_own_and_during_battle() {
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_structure_at(&mut game, "compiler", 30, 30);
    let wild = spawn_wild_on_player_tile(&mut game);
    let err = game.extract_routine(wild, 0).unwrap_err();
    assert!(err.contains("control"), "{err}");

    let pet = spawn_tamed(&mut game, 10, 3);
    start_battle_with_a_wild_program(&mut game);
    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(err.contains("right now"), "{err}");
}

#[test]
fn researching_a_node_grants_routine_items_rather_than_the_ability_itself() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = game
        .research_nodes()
        .into_iter()
        .find(|n| !n.unlocks_abilities.is_empty())
        .expect("some shipped node grants an ability");
    let ability = node.unlocks_abilities[0].clone();

    unlock_research_chain(&mut game, &node.id);

    assert_eq!(
        count_item(&game, crate::abilities::routine_item_id(&ability).as_str()),
        1,
        "the routine arrives as an item, not as an installed ability"
    );
    // Not `actor_abilities(..).is_empty()`: the player always starts with
    // decompile installed now, so the general emptiness check would fail
    // for a reason unrelated to what this test is pinning down.
    assert!(
        game.actor_abilities(game.player_entity())
            .iter()
            .all(|a| a.id != ability),
        "researching does not install — that is a separate act"
    );
}

#[test]
fn a_new_game_starts_with_decompile_installed_in_the_players_only_slot() {
    let game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let slots = game.routine_view(game.player_entity());
    assert_eq!(slots.len(), 1, "level 1 gives the player exactly one slot");
    assert_eq!(
        slots[0].ability.as_deref(),
        Some(crate::abilities::DECOMPILE_ABILITY_ID)
    );
}

/// Regression for I4: `decompile` is one-of-a-kind and unrecoverable — no
/// species, research node, drop, recipe or market listing grants it again.
/// Popping it into cargo to make room must not put it next to a delete
/// button that ends taming for the save.
#[test]
fn a_loose_routine_cannot_be_erased_or_sold() {
    let mut game = Game::new(55, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let item = crate::abilities::routine_item_id(crate::abilities::DECOMPILE_ABILITY_ID);
    game.uninstall_routine(game.player_entity(), 0).unwrap();
    assert_eq!(count_item(&game, item.as_str()), 1, "popped into cargo");

    let err = game.erase_item(&item, 1).unwrap_err();
    assert!(err.contains("routine"), "{err}");
    assert_eq!(
        count_item(&game, item.as_str()),
        1,
        "erase must not spend it"
    );

    let def = game
        .structure_defs()
        .into_iter()
        .find(|d| d.trade.is_some())
        .expect("a trading structure should exist");
    let market = game
        .world
        .spawn((
            Structure {
                kind: def.id.clone(),
            },
            Position { x: 5, y: 5 },
        ))
        .id();
    let err = game.sell_item(market, item.clone(), 1).unwrap_err();
    assert!(err.contains("routine"), "{err}");
    assert_eq!(
        count_item(&game, item.as_str()),
        1,
        "sale must not spend it"
    );
}

#[test]
fn decompile_is_reached_through_special_not_its_own_command() {
    let mut game = Game::new(52, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    start_battle_with_a_wild_program(&mut game);
    let options = game.battle_action_options(0);
    assert!(
        options.iter().any(|o| o.kind == ActionKind::Special),
        "the player's Special row carries decompile"
    );
    assert!(
        game.battle_special_options(0)
            .iter()
            .any(|o| o.name.to_lowercase().contains("decompile")),
        "decompile is one of the abilities on offer"
    );
}

#[test]
fn decompile_greys_with_a_reason_rather_than_refunding_the_round() {
    let mut game = Game::new(53, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    set_inventory(&mut game, &[]); // no taming catalyst
    start_battle_with_a_wild_program(&mut game);
    let row = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("decompile is installed");
    assert_eq!(row.unavailable.as_deref(), Some("no taming catalyst"));

    let err = game
        .battle_set_action(
            0,
            BattleAction::Special {
                ability: row.index,
                target: battle::SpecialTarget::EnemyGroup { group: 0 },
            },
        )
        .unwrap_err();
    assert!(err.contains("no taming catalyst"), "{err}");
}

/// Regression for I1: `ability_unavailable` greys Decompile per slot
/// (`taming_catalyst().is_none()`), but the catalyst it checks is a
/// round-wide pool, not one reserved per planner. With exactly one ICE
/// Breaker held, both the player and a companion can plan Decompile in the
/// same round — the first to resolve spends the only copy, and the second
/// used to hit an `expect` instead of a refusal.
#[test]
fn a_second_decompiler_in_the_same_round_is_refused_rather_than_panicking() {
    let mut game = Game::new(72, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let companion = spawn_tamed(&mut game, 50, 5);
    game.add_companion(companion).unwrap();
    set_level(&mut game, companion, 4); // two slots, one free
    let decompile_item = crate::abilities::routine_item_id(crate::abilities::DECOMPILE_ABILITY_ID);
    set_inventory(
        &mut game,
        &[(decompile_item.as_str(), 1), (ids::ICE_BREAKER, 1)],
    );
    game.install_routine(companion, &decompile_item).unwrap();

    // Two members of one species, built by hand rather than through
    // `start_battle`: the pack ceiling at the player's own tile caps a
    // same-species group at one member, and this test needs a capture on
    // the front not to end the battle before the second slot's action runs.
    let species = game.species_defs().into_iter().next().unwrap().id;
    let player_pos = *game.world.get::<Position>(player).unwrap();
    let e1 = game
        .world
        .spawn((
            Creature {
                species: species.clone(),
            },
            Hostile,
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Stats {
                hp: 40,
                max_hp: 40,
                atk: 0,
                def: 0,
            },
        ))
        .id();
    let e2 = game
        .world
        .spawn((
            Creature {
                species: species.clone(),
            },
            Hostile,
            Position {
                x: player_pos.x,
                y: player_pos.y,
            },
            Stats {
                hp: 40,
                max_hp: 40,
                atk: 0,
                def: 0,
            },
        ))
        .id();
    game.world.insert_resource(BattleState {
        player,
        groups: vec![EnemyGroup {
            species,
            members: vec![e1, e2],
        }],
        round: 1,
        planned: vec![None, None],
        finished: false,
        player_won: false,
    });

    let player_decompile = game
        .battle_special_options(0)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the player has decompile installed")
        .index;
    let companion_decompile = game
        .battle_special_options(1)
        .into_iter()
        .find(|o| o.name.to_lowercase().contains("decompile"))
        .expect("the companion has decompile installed")
        .index;

    game.battle_set_action(
        0,
        BattleAction::Special {
            ability: player_decompile,
            target: battle::SpecialTarget::EnemyGroup { group: 0 },
        },
    )
    .unwrap();
    game.battle_set_action(
        1,
        BattleAction::Special {
            ability: companion_decompile,
            target: battle::SpecialTarget::EnemyGroup { group: 0 },
        },
    )
    .unwrap();

    // Must not panic: this is the exact shape that used to hit the `expect`
    // in `attempt_decompile`.
    game.battle_resolve_round();

    assert_eq!(
        count_item(&game, ids::ICE_BREAKER),
        0,
        "the one catalyst held was spent by whichever slot resolved first"
    );
}

#[test]
fn popping_decompile_out_leaves_the_player_with_no_special() {
    let mut game = Game::new(54, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.uninstall_routine(game.player_entity(), 0).unwrap();
    start_battle_with_a_wild_program(&mut game);
    assert!(
        game.battle_action_options(0)
            .iter()
            .all(|o| o.kind != ActionKind::Special),
        "giving up decompile really does cost you the command"
    );
}

/// The whole payoff: decompile a carrier and its routine comes with it.
#[test]
fn a_carried_routine_survives_capture() {
    let mut game = Game::new(5501, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = generic_species(&game);
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let carrier = game
        .spawn_wild_creature(&species.id, spawn.x + 2, spawn.y)
        .unwrap();
    game.world
        .entity_mut(carrier)
        .insert(Routines(vec!["kernel_panic".to_string()]));
    game.world.entity_mut(carrier).insert(Experience::default());

    game.install_innate_routines(carrier);

    assert!(
        game.world
            .get::<Routines>(carrier)
            .unwrap()
            .0
            .contains(&"kernel_panic".to_string()),
        "the carried routine is the prize — it must not be overwritten by the species kit"
    );
}

/// A carrier already holds something real, so it must not also be handed
/// the placeholder that exists for programs whose species grants nothing.
#[test]
fn a_carrier_of_an_ability_less_species_is_not_given_the_fallback() {
    let mut game = Game::new(5502, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = generic_species(&game);
    assert!(
        species.abilities.is_empty(),
        "fixture: this species grants nothing of its own"
    );
    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let carrier = game
        .spawn_wild_creature(&species.id, spawn.x + 2, spawn.y)
        .unwrap();
    game.world
        .entity_mut(carrier)
        .insert(Routines(vec!["cold_boot".to_string()]));
    game.world.entity_mut(carrier).insert(Experience::default());

    game.install_innate_routines(carrier);

    assert_eq!(
        game.world.get::<Routines>(carrier).unwrap().0,
        vec!["cold_boot".to_string()],
        "the fallback fills an empty kit, not a kit that already holds a prize"
    );
}

/// A level-1 program has one slot, and six shipped species grant an ability
/// at level 1. The carried routine wins the slot; the species ability is
/// minted into cargo rather than destroyed, so the player can swap.
#[test]
fn a_species_ability_displaced_by_a_carried_routine_goes_to_cargo() {
    let mut game = Game::new(5503, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| s.abilities.iter().any(|a| a.level <= 1))
        .expect("a shipped species grants an ability at level 1");
    let displaced = species
        .abilities
        .iter()
        .find(|a| a.level <= 1)
        .unwrap()
        .id
        .clone();

    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let carrier = game
        .spawn_wild_creature(&species.id, spawn.x + 2, spawn.y)
        .unwrap();
    game.world
        .entity_mut(carrier)
        .insert(Routines(vec!["bastion".to_string()]));
    game.world.entity_mut(carrier).insert(Experience::default());

    let player = game.player_entity();
    let item = crate::abilities::routine_item_id(&displaced);
    let before = game.world.get::<Inventory>(player).unwrap().count(&item);

    game.install_innate_routines(carrier);

    assert_eq!(
        game.world.get::<Routines>(carrier).unwrap().0,
        vec!["bastion".to_string()],
        "one slot at level 1, and the carried routine takes it"
    );
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().count(&item),
        before + 1,
        "the displaced species ability lands in cargo instead of being destroyed"
    );
}

/// A species unlock reaching a full kit used to be logged and lost. It now
/// goes to cargo, and — critically — must never evict a carried routine.
#[test]
fn a_level_up_unlock_never_evicts_a_carried_routine() {
    let mut game = Game::new(5504, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let species = game
        .species_defs()
        .into_iter()
        .find(|s| s.abilities.iter().any(|a| a.level > 1))
        .expect("a shipped species unlocks an ability above level 1");
    let unlock = species
        .abilities
        .iter()
        .find(|a| a.level > 1)
        .unwrap()
        .clone();

    let spawn = *game.world.resource::<ZoneSpawnPoint>();
    let pet = game
        .spawn_wild_creature(&species.id, spawn.x + 2, spawn.y)
        .unwrap();
    game.world.entity_mut(pet).insert(Experience::default());
    game.world
        .entity_mut(pet)
        .insert(Routines(vec!["cold_boot".to_string()]));

    // One slot, already holding the prize, and the unlock now lands.
    game.install_unlocked_routines(pet, 1, unlock.level);

    assert!(
        game.world
            .get::<Routines>(pet)
            .unwrap()
            .0
            .contains(&"cold_boot".to_string()),
        "a carried routine is not the fallback placeholder and must never be evicted"
    );
    let player = game.player_entity();
    let item = crate::abilities::routine_item_id(&unlock.id);
    assert_eq!(
        game.world.get::<Inventory>(player).unwrap().count(&item),
        1,
        "the unlock that found no room goes to cargo"
    );
}
