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
                mitigation: 2,
            },
            Tamed { owner: player },
            Experience::default(),
            PowerReserve::default(),
        ))
        .id();
    game.install_innate_routines(scrapper);
    assert_eq!(
        game.world.get::<Routines>(scrapper).unwrap().0,
        vec![crate::abilities::FALLBACK_ABILITY_ID.to_string()],
        "a level-1 Scrapper has no unlock yet, so it starts on the fallback"
    );

    set_level(&mut game, scrapper, 2);
    let routines = &game.world.get::<Routines>(scrapper).unwrap().0;
    assert!(
        routines.iter().any(|r| r == "cascade_overflow"),
        "reaching the unlock must never lose it: {routines:?}"
    );
    // The fallback survives beside it now, and that is the slot rate rather
    // than a change of heart about placeholders: a level-up brings a slot
    // (`COMPANION_ROUTINE_SLOT_PER_LEVEL` is 1), so the unlock has somewhere
    // to land and has nothing to displace. The eviction branch it used to
    // exercise is covered by `evicting_a_manually_installed_priority_boost_
    // is_logged`, against a species that unlocks two routines at once.
    assert_eq!(
        routines.len(),
        2,
        "both the unlock and the fallback fit: {routines:?}"
    );
}

#[test]
fn a_level_up_that_reaches_an_unlock_installs_it_into_a_free_slot() {
    let (mut game, medic) = game_with_two_ability_companion();
    let before = game.world.get::<Routines>(medic).unwrap().0.len();
    // `TWO_ABILITY_SPECIES` gates `sandbox` at level 5, and a slot arrives
    // with every level, so the unlock has somewhere to land.
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
/// slot holds the auto-installed placeholder or a Hyperthread Single v1.0 the player
/// chose to install by hand — and the latter used to be evicted with no log
/// line at all, unlike the neighbouring "it is lost" branch.
#[test]
fn evicting_a_manually_installed_priority_boost_is_logged() {
    // A species that unlocks two routines on one rung, because that is now
    // the only way to run out of slots: an ordinary level-up brings a slot
    // along with the unlock it grants.
    let (mut game, medic) = game_with_contending_unlocks_companion();
    set_level(&mut game, medic, 2); // two slots: hot_patch, plus one free
    install_routine_for_test(&mut game, medic, crate::abilities::FALLBACK_ABILITY_ID);
    assert_eq!(
        game.world.get::<Routines>(medic).unwrap().0,
        vec!["hot_patch".to_string(), "priority_boost".to_string()],
        "a deliberate install, not the tame-time fallback"
    );

    // Three slots, and hot_patch plus two unlocks to put in them.
    set_level(&mut game, medic, 3);

    let routines = &game.world.get::<Routines>(medic).unwrap().0;
    assert!(
        !routines.iter().any(|r| r == "priority_boost"),
        "the unlock still evicts the id-matched slot, manual install or not: {routines:?}"
    );
    assert!(
        routines.iter().any(|r| r == "sandbox") && routines.iter().any(|r| r == "cascade_overflow"),
        "and both unlocks landed: {routines:?}"
    );
    assert!(
        game.message_log(10).iter().any(|e| {
            e.text.contains("swaps out")
                && e.text.contains("Hyperthread Single v1.0")
                // Either unlock may be the one that claimed the slot: they
                // arrive on the same level-up, so which lands first is the
                // order they are declared in, not something worth pinning.
                && (e.text.contains("Bastion Single") || e.text.contains("Packet Shred Group"))
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
    install_routine_for_test(&mut game, pet, "sandbox");
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
/// unfiltered, that id would survive into `Routines` as a zero_day
/// `routine_view` renders `(empty)` but `installed.len()` still counts
/// against the slot cap — a slot the panel calls free that can never
/// actually be filled again.
#[test]
fn a_routine_naming_a_since_removed_ability_is_dropped_on_load_with_a_warning() {
    let dir = std::env::temp_dir().join(format!("feral_ghost_routine_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    // Through the shared copier, not a local loop: this used to carry its
    // own list of asset subdirectories, and a directory added to the real
    // asset set failed here as a mystery `NotFound` out of `Game::new`.
    copy_shipped_assets(&dir, &[]);
    let ghost_ability = r#"(
        id: "ghost_ability",
        name: "ZeroDay Ability",
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
    install_routine_for_test(&mut game, player, "ghost_ability");

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
        "the zero_day id must not survive the load"
    );
    assert!(
        loaded
            .message_log(20)
            .iter()
            .any(|e| e.text.contains("no longer available")),
        "the drop must be logged, not silent: {:?}",
        loaded.message_log(20)
    );

    // The freed slot must be genuinely usable, not still counted against the
    // cap by a zero_day entry the panel can no longer even show.
    teach_routine(&mut loaded, "priority_boost");
    give_disks(&mut loaded, 1);
    loaded.etch_disk("priority_boost").unwrap();
    loaded
        .install_disk(player, "priority_boost")
        .expect("the slot the zero_day vacated must accept a real routine");

    let _ = std::fs::remove_dir_all(&dir);
}

/// The etch picker has to say what is already in cargo, or a player burns a
/// blank on a routine they are carrying three disks of. The count comes off
/// `etched_disks_of` — the same figure `install_disk` refuses on — so the
/// screen and the refusal cannot disagree about how many you hold.
#[test]
fn the_etch_picker_counts_disks_already_held() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    teach_routine(&mut game, "sandbox");
    teach_routine(&mut game, "priority_boost");
    give_disks(&mut game, 3);
    game.etch_disk("sandbox").unwrap();
    game.etch_disk("sandbox").unwrap();

    let rows = game.etchable_routines();
    let held = |id: &str| {
        rows.iter()
            .find(|r| r.ability == id)
            .unwrap_or_else(|| panic!("{id} is offered"))
            .held
    };
    assert_eq!(held("sandbox"), 2, "two etched disks are two disks held");
    assert_eq!(
        held("priority_boost"),
        0,
        "a routine you know but hold no disk of reads zero, not one"
    );
}

#[test]
fn a_known_routine_is_offered_with_the_abilitys_own_text() {
    let mut game = Game::new(21, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let sandbox = ability(&game, "sandbox");
    teach_routine(&mut game, &sandbox.id);
    let row = game
        .etchable_routines()
        .into_iter()
        .find(|r| r.ability == sandbox.id)
        .expect("a routine the player knows is offered");
    assert_eq!(row.name, sandbox.name);
    assert_eq!(
        row.description, sandbox.description,
        "the picker reads the ability's own text, never a copy of it"
    );
}

#[test]
fn etching_burns_a_blank_and_installing_burns_the_etched_disk() {
    let mut game = Game::new(22, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 4); // two slots, one of them free
    teach_routine(&mut game, "sandbox");
    give_disks(&mut game, 3);

    game.etch_disk("sandbox").unwrap();
    assert_eq!(
        game.blank_disks_held(),
        2,
        "etching burns one blank and no more"
    );
    assert_eq!(
        game.etched_disks_of("sandbox"),
        1,
        "and the blank comes back as an etched disk, not as nothing"
    );

    game.install_disk(pet, "sandbox").unwrap();
    assert_eq!(
        game.etched_disks_of("sandbox"),
        0,
        "installing spends the etched disk"
    );
    assert_eq!(
        game.blank_disks_held(),
        2,
        "and touches no blank — that half was paid at the etch"
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
        game.etched_disks_of("sandbox"),
        0,
        "the disk is spent for good — uninstalling hands nothing back"
    );
    assert!(
        game.routine_view(pet)[slot].ability.is_none(),
        "the slot is free again"
    );
    assert!(
        game.knows_routine("sandbox"),
        "what the player keeps is the knowledge, which they never spent"
    );
}

#[test]
fn etch_is_refused_unknown_blankless_and_during_battle() {
    let mut game = Game::new(23, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();

    let err = game.etch_disk("sandbox").unwrap_err();
    assert!(err.contains("don't know"), "never researched it: {err}");

    teach_routine(&mut game, "sandbox");
    let err = game.etch_disk("sandbox").unwrap_err();
    assert!(err.contains("Routine Disk"), "no blank held: {err}");

    give_disks(&mut game, 1);
    start_battle_with_a_wild_program(&mut game);
    let err = game.etch_disk("sandbox").unwrap_err();
    assert!(err.contains("right now"), "mid-battle: {err}");
    assert_eq!(
        game.blank_disks_held(),
        1,
        "no refusal on any path may spend the blank"
    );
    assert_eq!(
        game.etched_disks_of("sandbox"),
        0,
        "and none of them may hand back a disk either"
    );
}

#[test]
fn install_is_refused_diskless_slotless_duplicated_and_during_battle() {
    let mut game = Game::new(24, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let pet = spawn_tamed(&mut game, 10, 3); // level 1: exactly one slot, already full

    let err = game.install_disk(pet, "sandbox").unwrap_err();
    assert!(
        err.contains("no free routine slot"),
        "slots full is checked before the disk, so a full slot never eats one: {err}"
    );

    set_level(&mut game, pet, 4); // a free slot, but no disk to fill it from
    let err = game.install_disk(pet, "sandbox").unwrap_err();
    assert!(
        err.contains("not carrying"),
        "the disk is what an install spends, and there isn't one: {err}"
    );

    give_etched_disks(&mut game, "sandbox", 2);
    game.install_disk(pet, "sandbox").unwrap();
    let err = game.install_disk(pet, "sandbox").unwrap_err();
    assert!(
        err.contains("already runs"),
        "a second copy of the same routine in a second slot is refused: {err}"
    );

    start_battle_with_a_wild_program(&mut game);
    let err = game.install_disk(pet, "sandbox").unwrap_err();
    assert!(err.contains("right now"), "mid-battle: {err}");
    assert_eq!(
        game.etched_disks_of("sandbox"),
        1,
        "no refusal on any path may spend the disk"
    );
}

#[test]
fn a_popped_innate_routine_replants_only_if_the_player_knows_it() {
    let (mut game, medic) = game_with_two_ability_companion();
    let popped = game.routine_view(medic)[0].ability.clone().unwrap();
    game.uninstall_routine(medic, 0).unwrap();
    assert!(
        game.routine_view(medic).iter().all(|s| s.ability.is_none()),
        "the innate slot should now be empty"
    );

    let host = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, host, 4);
    give_disks(&mut game, 1);
    // The refusal moved to the etch with the flow split: knowledge is what
    // writes a blank, and a slot only ever takes a disk somebody wrote.
    let err = game.etch_disk(&popped).unwrap_err();
    assert!(
        err.contains("don't know"),
        "an innate routine popped out is gone unless the player learned it: {err}"
    );

    teach_routine(&mut game, &popped);
    fit_routine(&mut game, host, &popped);
    assert!(
        game.routine_view(host)
            .iter()
            .any(|s| s.ability.as_deref() == Some(popped.as_str())),
        "a foreign species' routine installs fine once it is known"
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
                mitigation: 1,
            },
            Routines(vec![]),
        ))
        .id();

    give_etched_disks(&mut game, "sandbox", 1);
    let err = game.install_disk(wild, "sandbox").unwrap_err();
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
fn extracting_teaches_the_picked_routine_destroys_the_program_and_loses_the_rest() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 5); // both of its unlocks installed
    spawn_structure_at(&mut game, "compiler", 30, 30);

    let offered = game.extractable_routines(medic);
    assert_eq!(offered.len(), 2, "both installed routines are on offer");
    assert!(
        offered.iter().all(|r| !r.known),
        "neither is known yet, so neither row is marked"
    );
    let kept = offered[1].ability.clone();
    let lost = offered[0].ability.clone();

    game.extract_routine(medic, 1).unwrap();

    assert!(
        game.knows_routine(&kept),
        "the picked routine is learned, not stocked"
    );
    assert!(
        !game.knows_routine(&lost),
        "everything else on the program is lost with it"
    );
    assert_eq!(
        game.blank_disks_held(),
        0,
        "extraction teaches; it does not hand over a blank to write with"
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

/// Knowledge does not stack, so extracting a routine the player already
/// knows would destroy a program for nothing. Refused before the despawn,
/// not after.
#[test]
fn extracting_a_routine_you_already_know_is_refused_and_the_program_survives() {
    let mut game = Game::new(34, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    spawn_structure_at(&mut game, "compiler", 30, 30);
    let pet = spawn_tamed(&mut game, 10, 3);
    let installed = game.extractable_routines(pet)[0].ability.clone();
    teach_routine(&mut game, &installed);

    assert!(
        game.extractable_routines(pet)[0].known,
        "the picker must mark it before the player commits"
    );
    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(err.contains("already know"), "{err}");
    assert!(
        game.world.get::<Stats>(pet).is_some(),
        "the program must survive a refused extraction"
    );
}

#[test]
fn researching_a_node_teaches_the_routine_rather_than_installing_or_stocking_it() {
    let mut game = Game::new(41, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let node = game
        .research_nodes()
        .into_iter()
        .find(|n| !n.unlocks_abilities.is_empty())
        .expect("some shipped node grants an ability");
    let ability = node.unlocks_abilities[0].clone();

    unlock_research_chain(&mut game, &node.id);

    assert!(game.knows_routine(&ability), "the node teaches the routine");
    assert_eq!(
        game.blank_disks_held(),
        0,
        "knowledge is not a disk — the factory is what makes those"
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

/// Regression for I4, restated for the disk model: `decompile` is
/// one-of-a-kind — no species, research node, drop or listing grants it
/// again. Popping it out to free the one starting slot must therefore not
/// end taming for the save, which is why a new game already knows it.
#[test]
fn the_player_starts_knowing_decompile_so_popping_it_out_is_recoverable() {
    let mut game = Game::new(55, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert!(
        game.knows_routine(crate::abilities::DECOMPILE_ABILITY_ID),
        "decompile is knowledge the player starts with"
    );

    game.uninstall_routine(player, 0).unwrap();
    give_disks(&mut game, 1);
    game.etch_disk(crate::abilities::DECOMPILE_ABILITY_ID)
        .expect("decompile must be re-writable onto a fresh blank");
    game.install_disk(player, crate::abilities::DECOMPILE_ABILITY_ID)
        .expect("and the disk that comes out must go back into the slot");
    assert_eq!(
        game.routine_view(player)[0].ability.as_deref(),
        Some(crate::abilities::DECOMPILE_ABILITY_ID)
    );
}

/// The other half of the model: a `KnownRoutines` entry is world state, so
/// it has to survive the save like `researched` does.
#[test]
fn known_routines_survive_a_save_load_round_trip() {
    let mut game = Game::new(58, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    teach_routine(&mut game, "sandbox");
    let path =
        std::env::temp_dir().join(format!("feral_known_routines_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert!(
        loaded.knows_routine("sandbox"),
        "a learned routine is world state, not a held item"
    );
    assert!(
        loaded.knows_routine(crate::abilities::DECOMPILE_ABILITY_ID),
        "and so is the one the run started with"
    );
}

/// A species' kit is granted directly at spawn — the disk model prices what
/// the *player* installs, not what a program is born running.
#[test]
fn innate_routines_install_at_spawn_with_no_knowledge_and_no_disk() {
    let (game, medic) = game_with_two_ability_companion();
    let installed = &game.world.get::<Routines>(medic).unwrap().0;
    assert!(!installed.is_empty(), "the species kit is installed");
    assert_eq!(game.blank_disks_held(), 0, "and it cost no disk");
    assert!(
        installed.iter().all(|id| !game.knows_routine(id)),
        "nor does the player learn what their program runs"
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
    enlist(&mut game, companion);
    set_level(&mut game, companion, 4); // two slots, one free
    set_inventory(&mut game, &[(ids::ICE_BREAKER, 1)]);
    install_routine_for_test(&mut game, companion, crate::abilities::DECOMPILE_ABILITY_ID);

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
                mitigation: 0,
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
                mitigation: 0,
            },
        ))
        .id();
    game.world.insert_resource(BattleState {
        player,
        round_targets: vec![vec![e1, e2]],
        groups: vec![EnemyGroup {
            species,
            members: vec![e1, e2],
        }],
        round: 1,
        planned: vec![None, None],
        finished: false,
        player_won: false,
        decompile_attempts: std::collections::HashMap::new(),
        rewards: BattleRewards::default(),
        lair: None,
        outmatched: false,
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
    let species = generic_species();
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
    let species = generic_species();
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
/// lost, and logged rather than silently dropped.
#[test]
fn a_species_ability_displaced_by_a_carried_routine_is_logged_as_lost() {
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

    game.install_innate_routines(carrier);

    assert_eq!(
        game.world.get::<Routines>(carrier).unwrap().0,
        vec!["bastion".to_string()],
        "one slot at level 1, and the carried routine takes it"
    );
    let displaced_name = game.ability_display_name(&displaced);
    assert!(
        game.message_log(10)
            .iter()
            .any(|e| e.text.contains(&displaced_name) && e.text.contains("is lost")),
        "the displaced species ability is lost, and the player gets to read that: {:?}",
        game.message_log(10)
    );
    assert!(
        !game.knows_routine(&displaced),
        "a displaced routine is not a routine the player learned"
    );
}

/// A species unlock reaching a full kit is logged and lost — and,
/// critically, must never evict a carried routine to avoid that.
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
    let unlock_name = game.ability_display_name(&unlock.id);
    assert!(
        game.message_log(10)
            .iter()
            .any(|e| e.text.contains(&unlock_name) && e.text.contains("is lost")),
        "the unlock that found no room is lost, and said so: {:?}",
        game.message_log(10)
    );
}
