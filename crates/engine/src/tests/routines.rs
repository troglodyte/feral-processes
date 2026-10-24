//! Routines: the slots abilities occupy, and how they get there.

use super::support::*;
use crate::components::Routines;
use crate::items::DownedProgram;
use crate::*;

/// `Game::unseen_routine` — the Alt marker's own derivation (spec §4 "The
/// Alt marker", Task 10) — agrees with `routine_candidates`, the exact door
/// extraction reads, in both the flagged and the cleared case, and is false
/// on a tamed program regardless.
#[test]
fn unseen_routine_agrees_with_the_candidate_function_and_is_false_when_owned() {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let wild = spawn_wild_without_routine(&mut game, "scrapper", 3, 3);
    game.world
        .entity_mut(wild)
        .insert(Routines(vec!["hot_patch".to_string()]));
    let level = game.ability_user_level(wild);

    let program = DownedProgram {
        species: "scrapper".to_string(),
        level,
        rarity: Rarity::Ordinary,
        boss: false,
        condition: 100,
        carried: Some("hot_patch".to_string()),
    };
    let expected = !game.routine_candidates(&program).is_empty();
    assert!(expected, "the fixture must actually offer a candidate");
    assert_eq!(
        game.unseen_routine(wild),
        expected,
        "the marker must agree with the same candidate function extraction reads"
    );

    // A tamed program is never flagged.
    let pet = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(pet)
        .insert(Routines(vec!["hot_patch".to_string()]));
    assert!(
        !game.unseen_routine(pet),
        "an owned program is never flagged"
    );
}

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
    // The fallback survives beside it, and that is the slot count rather
    // than a change of heart about placeholders: the unlock has a free slot
    // to land in and has nothing to displace. The eviction branch it used to
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
    // The level is written straight in rather than levelled into, so the two
    // unlocks on the level-3 rung are still waiting when the kit is packed.
    game.world.get_mut::<Experience>(medic).unwrap().level = 3;
    install_routine_for_test(&mut game, medic, crate::abilities::FALLBACK_ABILITY_ID);
    assert_eq!(
        game.world.get::<Routines>(medic).unwrap().0,
        vec!["hot_patch".to_string(), "priority_boost".to_string()],
        "a deliberate install, not the tame-time fallback"
    );
    // One free slot against two unlocks: the first lands in it and the
    // second has to evict, which is the branch this test is about.
    fill_routine_slots(&mut game, medic, 1, &["sandbox", "cascade_overflow"]);

    game.install_unlocked_routines(medic, 2, 3);

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

/// The curve itself is pinned in `abilities.rs`; this shows the player and
/// a plain companion are both read off it, and that a companion's rarity,
/// quality and species class reach `Game::routine_slots`.
#[test]
fn game_routine_slots_reads_one_curve_plus_the_companion_s_bonuses() {
    use crate::components::{Potential, Rarity};
    let mut game = Game::new(11, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    let pet = spawn_tamed(&mut game, 10, 3);
    set_level(&mut game, pet, 6);
    set_level(&mut game, player, 6);
    // Pinned so the species' own class cannot add or take a slot here.
    let classless = game
        .world
        .resource::<crate::species::SpeciesDb>()
        .all()
        .find(|d| d.affinity_class().is_none() && !d.is_boss)
        .expect("a classless species ships")
        .id
        .clone();
    game.world.get_mut::<Creature>(pet).unwrap().species = classless;
    game.world
        .entity_mut(pet)
        .insert((Rarity::Ordinary, Potential::NEUTRAL));
    assert_eq!(
        game.routine_slots(player),
        crate::abilities::level_routine_slots(6)
    );
    assert_eq!(game.routine_slots(pet), game.routine_slots(player));

    game.world.entity_mut(pet).insert(Rarity::Gold);
    assert_eq!(
        game.routine_slots(pet),
        game.routine_slots(player) + 2,
        "Gold is two tiers above Ordinary"
    );

    let saboteur = game
        .world
        .resource::<crate::species::SpeciesDb>()
        .all()
        .find(|d| d.affinity_class() == Some(crate::species::AffinityClass::Saboteur))
        .expect("a Saboteur species ships")
        .id
        .clone();
    game.world.get_mut::<Creature>(pet).unwrap().species = saboteur;
    assert_eq!(
        game.routine_slots(pet),
        game.routine_slots(player) + 2 + crate::tuning::SABOTEUR_ROUTINE_SLOT_BONUS as usize,
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
    install_routine_for_test(&mut loaded, player, "priority_boost");
    assert!(
        loaded
            .actor_abilities(player)
            .iter()
            .any(|a| a.id == "priority_boost"),
        "the slot the zero_day vacated must accept a real routine"
    );

    let _ = std::fs::remove_dir_all(&dir);
}

/// Regression for M4: a save-loaded wild creature carries `Routines(vec![])`
/// too (`lifecycle.rs` inserts it in the common creature bundle before the
/// `if c.tamed` branch), so without an ownership check `install_built`
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
                species: game
                    .species_defs()
                    .into_iter()
                    .find(|d| !d.is_hybrid())
                    .unwrap()
                    .id,
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

    let err = game.install_built(wild, "crafted/1").unwrap_err();
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
        err.contains("Workbench"),
        "the refusal should name the bench: {err}"
    );

    stand_an_extraction_bench(&mut game, 30, 30);
    assert!(
        game.can_extract_routines(),
        "a bench 30 tiles away still counts — extraction has no proximity rule"
    );
}

/// `extract_routine` despawns the program it draws from
/// (`dissolve_tamed_program`), and a posted crew member despawned mid-shift
/// would strand its outpost counting a body that no longer exists — one of
/// the outposts plan's four explicit new refusals, since nothing else here
/// checked either role before it.
#[test]
fn extract_routine_refuses_a_posted_program() {
    let mut game = Game::new(20260925, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_an_extraction_bench(&mut game, 30, 30);
    let pet = spawn_tamed(&mut game, 10, 3);
    game.world
        .entity_mut(pet)
        .insert(components::PostedAt((7, 7)));

    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(err.contains("outpost"), "unexpected error: {err}");
    assert!(
        game.world.get_entity(pet).is_ok(),
        "a refused extraction must not despawn the program"
    );
}

#[test]
fn extracting_hands_over_the_picked_routines_parts_destroys_the_program_and_loses_the_rest() {
    let (mut game, medic) = game_with_two_ability_companion();
    set_level(&mut game, medic, 5); // both of its unlocks installed
    stand_an_extraction_bench(&mut game, 30, 30);

    let offered = game.extractable_routines(medic);
    assert_eq!(offered.len(), 2, "both installed routines are on offer");
    let kept = offered[1].ability.clone();
    let expected = crate::routine_craft::parts_used(&game.parts_of_routine(&kept));
    assert!(!expected.is_empty(), "the picked routine takes apart");

    game.extract_routine(medic, 1).unwrap();

    for (part, count) in &expected {
        assert!(
            game.parts_held(part) >= *count,
            "{} should have come out of {kept}",
            part.label()
        );
    }
    let total: u32 = crate::routine_craft::Subroutine::all()
        .iter()
        .map(|p| game.parts_held(p))
        .sum();
    assert_eq!(
        total,
        expected.iter().map(|(_, n)| n).sum::<u32>(),
        "everything else on the program is lost with it"
    );
    assert_eq!(game.blank_disks_held(), 0, "and hands over no blank");
    assert!(
        game.owned_pets().iter().all(|p| p.entity != medic),
        "the program is consumed"
    );
}

#[test]
fn extraction_is_refused_for_a_program_you_dont_own_and_during_battle() {
    let mut game = Game::new(33, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_an_extraction_bench(&mut game, 30, 30);
    let wild = spawn_wild_on_player_tile(&mut game);
    let err = game.extract_routine(wild, 0).unwrap_err();
    assert!(err.contains("control"), "{err}");

    let pet = spawn_tamed(&mut game, 10, 3);
    start_battle_with_a_wild_program(&mut game);
    let err = game.extract_routine(pet, 0).unwrap_err();
    assert!(err.contains("right now"), "{err}");
}

/// The tamed door's log line says a routine came apart.
#[test]
fn extracting_a_routine_logs_that_it_came_apart() {
    let mut game = Game::new(35, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    stand_an_extraction_bench(&mut game, 30, 30);
    let pet = spawn_tamed(&mut game, 10, 3);

    game.extract_routine(pet, 0).unwrap();

    let recent: Vec<String> = game
        .message_log(5)
        .iter()
        .map(|line| line.text.clone())
        .collect();
    assert!(
        recent
            .iter()
            .any(|line| line.contains("apart into its parts")),
        "the log should say what was recovered: {recent:?}"
    );
}

#[test]
fn a_new_game_starts_with_decompile_installed_in_the_players_first_slot() {
    let game = Game::new(51, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let slots = game.routine_view(game.player_entity());
    assert_eq!(
        slots.len(),
        crate::abilities::level_routine_slots(1),
        "the view is one row per slot the level buys"
    );
    assert_eq!(
        slots[0].ability.as_deref(),
        Some(crate::abilities::DECOMPILE_ABILITY_ID)
    );
    assert_eq!(
        slots[1].ability.as_deref(),
        Some("crafted/1"),
        "the built Patch is the second thing a new game installs"
    );
    assert!(
        slots[2..].iter().all(|s| s.ability.is_none()),
        "decompile and Patch are all a new game pre-installs"
    );
}

/// `decompile` is one-of-a-kind — no species, research node, drop or
/// listing grants it again — and the way that is kept true is that it never
/// leaves the slot it is born in. The pop-out used to be legal and was made
/// recoverable by pre-seeding `KnownRoutines`; both halves are gone.
#[test]
fn decompile_cannot_be_popped_out_of_the_players_slot() {
    let mut game = Game::new(55, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    assert_eq!(
        game.routine_view(player)[0].ability.as_deref(),
        Some(crate::abilities::DECOMPILE_ABILITY_ID),
        "slot 0 is where a new game puts it"
    );

    let refusal = game
        .uninstall_routine(player, 0)
        .expect_err("decompile is fixed — clearing its slot must be refused");
    assert!(
        refusal.contains(&game.ability_display_name(crate::abilities::DECOMPILE_ABILITY_ID)),
        "the refusal has to name the routine it is about; got {refusal:?}"
    );
    assert_eq!(
        game.routine_view(player)[0].ability.as_deref(),
        Some(crate::abilities::DECOMPILE_ABILITY_ID),
        "and the slot is untouched"
    );
}

/// A species' kit is granted directly at spawn — the disk model prices what
/// the *player* installs, not what a program is born running.
#[test]
fn innate_routines_install_at_spawn_with_no_disk() {
    let (game, medic) = game_with_two_ability_companion();
    let installed = &game.world.get::<Routines>(medic).unwrap().0;
    assert!(!installed.is_empty(), "the species kit is installed");
    assert_eq!(game.blank_disks_held(), 0, "and it cost no disk");
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
                image: None,
            },
        )
        .unwrap_err();
    assert!(err.contains("no taming catalyst"), "{err}");
}

/// Regression for I1: `ability_unavailable` greys Decompile per slot
/// (`taming_catalyst().is_none()`), but the catalyst it checks is a
/// round-wide pool, not one reserved per planner. With exactly one Breach
/// Kit held, both the player and a companion can plan Decompile in the
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
    // Written straight into the slot: `decompile` is fixed to the player,
    // so no door — etch, install, species kit — puts it on a program any
    // more. What is guarded here is the round-wide catalyst pool, which two
    // planners can still race for in a save written before that rule, and
    // which must refuse rather than panic whoever loses.
    game.write_routine(companion, crate::abilities::DECOMPILE_ABILITY_ID);

    // Two members of one species, built by hand rather than through
    // `start_battle`: the pack ceiling at the player's own tile caps a
    // same-species group at one member, and this test needs a capture on
    // the front not to end the battle before the second slot's action runs.
    let species = game
        .species_defs()
        .into_iter()
        .find(|d| !d.is_hybrid())
        .unwrap()
        .id;
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
            id: crate::battle::GroupId::new(0),
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
        dead_mans_switch_spent: false,
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
            image: None,
        },
    )
    .unwrap();
    game.battle_set_action(
        1,
        BattleAction::Special {
            ability: companion_decompile,
            target: battle::SpecialTarget::EnemyGroup { group: 0 },
            image: None,
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

/// An empty kit hides the Special row entirely. Decompile is welded into
/// slot 0, so this is no longer a state the player can reach by clearing it
/// — it is what a save written before that rule can still carry, and the
/// row has to stay hidden for it.
#[test]
fn an_empty_kit_leaves_the_player_with_no_special() {
    let mut game = Game::new(54, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let player = game.player_entity();
    clear_routines(&mut game, player);
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

/// Six shipped species grant an ability at level 1. A carrier whose kit is
/// already full keeps what it was carrying; the species ability is lost, and
/// logged rather than silently dropped.
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
    game.world.entity_mut(carrier).insert(Experience::default());
    // A carried kit with no room left, built from the catalogue rather than
    // from how many slots level 1 happens to buy.
    let carried = spare_routine_ids(&game, game.routine_slots(carrier), &[&displaced]);
    game.world
        .entity_mut(carrier)
        .insert(Routines(carried.clone()));

    game.install_innate_routines(carrier);

    assert_eq!(
        game.world.get::<Routines>(carrier).unwrap().0,
        carried,
        "the kit is full, and what it was carrying keeps every slot"
    );
    let displaced_name = game.ability_display_name(&displaced);
    assert!(
        game.message_log(10)
            .iter()
            .any(|e| e.text.contains(&displaced_name) && e.text.contains("is lost")),
        "the displaced species ability is lost, and the player gets to read that: {:?}",
        game.message_log(10)
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
    let mut carried = vec!["cold_boot".to_string()];
    carried.extend(spare_routine_ids(
        &game,
        game.routine_slots(pet) - 1,
        &["cold_boot", &unlock.id],
    ));
    game.world.entity_mut(pet).insert(Routines(carried));

    // A full kit holding the prize, and the unlock now lands.
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
