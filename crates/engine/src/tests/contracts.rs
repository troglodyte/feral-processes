//! Contracts: the catalogue, the run state, the one system that advances
//! progress, and the board a Contract Broker derives.

use super::support::*;
use crate::contracts::{ContractDb, ContractId, Objective, Reward};

/// A temp directory of `.ron` files to load a `ContractDb` out of. Tagged as
/// well as pid-stamped because these run in parallel inside one process.
fn contract_dir(tag: &str, files: &[(&str, &str)]) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("fp_contracts_{}_{tag}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    dir
}

fn load(tag: &str, files: &[(&str, &str)]) -> (ContractDb, Vec<String>) {
    let dir = contract_dir(tag, files);
    let loaded = ContractDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

#[test]
fn every_objective_variant_parses_and_states_its_target() {
    let (db, warnings) = load(
        "variants",
        &[
            (
                "kill.ron",
                r#"(id: "kill", name: "Kill", description: "d",
                    objective: Kill(species: Some("drone"), count: 6),
                    reward: [Credits(40)])"#,
            ),
            (
                "kill_any.ron",
                r#"(id: "kill_any", name: "Any", description: "d",
                    objective: Kill(species: None, count: 3),
                    reward: [Xp(50)])"#,
            ),
            (
                "deliver.ron",
                r#"(id: "deliver", name: "Deliver", description: "d",
                    objective: Deliver(item: "core_fragment", count: 4),
                    reward: [Credits(20)])"#,
            ),
            (
                "descend.ron",
                r#"(id: "descend", name: "Descend", description: "d",
                    objective: Descend(depth: 3),
                    reward: [Xp(200)])"#,
            ),
            (
                "breach.ron",
                r#"(id: "breach", name: "Breach", description: "d",
                    objective: Breach(zone: 2),
                    reward: [Credits(60)])"#,
            ),
            (
                "build.ron",
                r#"(id: "build", name: "Build", description: "d",
                    objective: Build(structure: "mining_node"),
                    reward: [Item("core_fragment", 5)])"#,
            ),
        ],
    );

    assert!(warnings.is_empty(), "all six are well-formed: {warnings:?}");
    assert_eq!(db.iter().count(), 6);

    let target = |id: &str| db.get(&ContractId::from(id)).unwrap().objective.target();
    assert_eq!(target("kill"), 6, "a counting objective targets its count");
    assert_eq!(target("kill_any"), 3);
    assert_eq!(target("deliver"), 4);
    assert_eq!(
        target("descend"),
        1,
        "a state-shaped objective is one unit of progress, so every contract \
         completes through one `progress >= target()` rule"
    );
    assert_eq!(target("breach"), 1);
    assert_eq!(target("build"), 1);

    assert_eq!(
        db.get(&ContractId::from("kill")).unwrap().objective,
        Objective::Kill {
            species: Some("drone".to_string()),
            count: 6
        }
    );
    assert_eq!(
        db.get(&ContractId::from("build")).unwrap().reward,
        vec![Reward::Item(crate::items::ItemId::from("core_fragment"), 5)]
    );
}

#[test]
fn the_two_optional_fields_default_when_absent() {
    let (db, warnings) = load(
        "defaults",
        &[(
            "bare.ron",
            r#"(id: "bare", name: "Bare", description: "d",
                objective: Breach(zone: 2), reward: [Credits(10)])"#,
        )],
    );
    assert!(warnings.is_empty(), "{warnings:?}");
    let def = db.get(&ContractId::from("bare")).unwrap();
    assert_eq!(def.min_zone, 0, "absent and 0 mean the same thing");
    assert!(!def.repeatable);
}

#[test]
fn a_malformed_contract_file_is_skipped_not_fatal() {
    let (db, warnings) = load(
        "malformed",
        &[
            ("broken.ron", r#"(id: "x", objective: NotAnObjective)"#),
            (
                "ok.ron",
                r#"(id: "good", name: "Good", description: "d",
                    objective: Breach(zone: 2), reward: [Credits(10)])"#,
            ),
        ],
    );

    assert_eq!(warnings.len(), 1, "the junk file should warn: {warnings:?}");
    assert!(
        warnings[0].contains("broken.ron"),
        "the warning has to name the file so it can be fixed: {warnings:?}"
    );
    assert!(
        db.get(&ContractId::from("good")).is_some(),
        "a sibling of a broken file still loads"
    );
    assert_eq!(db.iter().count(), 1);
}

#[test]
fn an_empty_id_is_refused() {
    let (db, warnings) = load(
        "empty_id",
        &[(
            "nameless.ron",
            r#"(id: "", name: "N", description: "d",
                objective: Breach(zone: 2), reward: [Credits(10)])"#,
        )],
    );
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert_eq!(db.iter().count(), 0);
}

#[test]
fn a_duplicate_id_is_refused() {
    let (db, warnings) = load(
        "duplicate",
        &[
            (
                "a.ron",
                r#"(id: "twice", name: "A", description: "d",
                    objective: Breach(zone: 2), reward: [Credits(10)])"#,
            ),
            (
                "b.ron",
                r#"(id: "twice", name: "B", description: "d",
                    objective: Breach(zone: 3), reward: [Credits(10)])"#,
            ),
        ],
    );
    assert_eq!(
        warnings.len(),
        1,
        "the second file should warn: {warnings:?}"
    );
    assert_eq!(db.iter().count(), 1);
}

#[test]
fn a_contract_that_pays_nothing_is_refused() {
    let (db, warnings) = load(
        "free",
        &[
            (
                "empty.ron",
                r#"(id: "empty", name: "N", description: "d",
                    objective: Breach(zone: 2), reward: [])"#,
            ),
            (
                "credits.ron",
                r#"(id: "credits", name: "N", description: "d",
                    objective: Breach(zone: 2), reward: [Credits(0)])"#,
            ),
            (
                "item.ron",
                r#"(id: "item", name: "N", description: "d",
                    objective: Breach(zone: 2), reward: [Item("core_fragment", 0)])"#,
            ),
            (
                "xp.ron",
                r#"(id: "xp", name: "N", description: "d",
                    objective: Breach(zone: 2), reward: [Xp(0)])"#,
            ),
        ],
    );
    assert_eq!(
        warnings.len(),
        4,
        "a contract paying nothing is a mistake that reads as a working file: {warnings:?}"
    );
    assert_eq!(db.iter().count(), 0);
}

/// The shipped set, loaded from the real `assets/contracts/`.
fn shipped_contracts() -> (ContractDb, Vec<String>) {
    ContractDb::load_dir(&test_assets_dir().join("contracts")).unwrap()
}

#[test]
fn the_shipped_contracts_name_things_that_exist() {
    let assets = test_assets_dir();
    let (contracts, warnings) = shipped_contracts();
    assert!(
        warnings.is_empty(),
        "shipped contracts should all parse: {warnings:?}"
    );
    assert!(contracts.iter().count() >= 8, "the shipped set is authored");

    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items")).unwrap();
    let (structures, _) =
        crate::structures::StructureDb::load_dir(&assets.join("structures")).unwrap();
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (species, _) =
        crate::species::SpeciesDb::load_dir(&assets.join("species"), &abilities).unwrap();

    for def in contracts.iter() {
        assert!(
            !def.description.is_empty(),
            "{} needs a description — it is the only place a player is told what to do",
            def.id
        );
        match &def.objective {
            Objective::Deliver { item, .. } => assert!(
                items.get(item.as_str()).is_some(),
                "{} asks for an item that does not exist: {item}",
                def.id
            ),
            Objective::Kill {
                species: Some(id), ..
            } => assert!(
                species.get(id).is_some(),
                "{} names a species that does not exist: {id}",
                def.id
            ),
            Objective::Build { structure } => assert!(
                structures.get(structure).is_some(),
                "{} names a structure that does not exist: {structure}",
                def.id
            ),
            _ => {}
        }
        for reward in &def.reward {
            if let Reward::Item(item, _) = reward {
                assert!(
                    items.get(item.as_str()).is_some(),
                    "{} pays an item that does not exist: {item}",
                    def.id
                );
                assert_ne!(
                    item.as_str(),
                    crate::items::ids::PORTAL_FRAGMENT,
                    "{} pays Portal Fragments. `Reward::PortalFragments` does not exist on \
                     purpose; `Reward::Item` is the same thing through the back door, and \
                     breaching stays earned by fighting and descending.",
                    def.id
                );
            }
        }
    }
}

#[test]
fn every_objective_variant_ships_at_least_once() {
    let (contracts, _) = shipped_contracts();
    let mut seen = [false; 5];
    for def in contracts.iter() {
        let slot = match &def.objective {
            Objective::Kill { .. } => 0,
            Objective::Deliver { .. } => 1,
            Objective::Descend { .. } => 2,
            Objective::Breach { .. } => 3,
            Objective::Build { .. } => 4,
        };
        seen[slot] = true;
    }
    assert!(
        seen.iter().all(|&s| s),
        "every objective variant needs shipped content exercising it, or a code path \
         added for it is never walked in a real game: {seen:?}"
    );
}

// ---------------------------------------------------------------------------
// Run state and the save round trip
// ---------------------------------------------------------------------------

use crate::contracts::ContractDef;
use crate::resources::{ActiveContract, ActiveContracts};
use crate::*;

fn fresh() -> Game {
    Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap()
}

/// A def built by hand rather than looked up, so a retune of the shipped set
/// cannot move a test about the machinery.
fn def(id: &str, objective: Objective, reward: Vec<Reward>) -> ContractDef {
    ContractDef {
        id: ContractId::from(id),
        name: format!("Contract {id}"),
        description: "d".to_string(),
        objective,
        reward,
        min_zone: 0,
        repeatable: false,
    }
}

fn give(game: &mut Game, def: ContractDef, progress: u32) {
    let tick = game.current_tick();
    game.world
        .resource_mut::<ActiveContracts>()
        .active
        .push(ActiveContract {
            def,
            progress,
            accepted_tick: tick,
        });
}

#[test]
fn a_new_game_holds_no_contracts() {
    let game = fresh();
    let held = game.world.resource::<ActiveContracts>();
    assert!(held.active.is_empty());
    assert!(held.done.is_empty());
}

#[test]
fn the_shipped_contracts_are_reachable_through_a_loaded_game() {
    let game = fresh();
    assert!(
        game.world
            .resource::<crate::contracts::ContractDb>()
            .iter()
            .count()
            >= 8,
        "the asset directory has to be registered, or the board has nothing to draw from"
    );
}

#[test]
fn active_contracts_survive_a_save_and_load() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "hunt",
            Objective::Kill {
                species: Some("drone".to_string()),
                count: 6,
            },
            vec![Reward::Credits(40), Reward::Xp(120)],
        ),
        4,
    );
    give(
        &mut game,
        def(
            "haul",
            Objective::Deliver {
                item: crate::items::ItemId::from("core_fragment"),
                count: 25,
            },
            vec![Reward::Credits(35)],
        ),
        0,
    );
    game.world
        .resource_mut::<ActiveContracts>()
        .done
        .push(ContractId::from("already_finished"));
    let before = game.world.resource::<ActiveContracts>().active.clone();

    let path =
        std::env::temp_dir().join(format!("feral_contracts_save_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    let held = loaded.world.resource::<ActiveContracts>();
    assert_eq!(
        held.active, before,
        "progress, accepted_tick and the whole resolved def all travel"
    );
    assert_eq!(held.done, vec![ContractId::from("already_finished")]);
}

#[test]
fn a_save_written_before_contracts_existed_still_loads() {
    let mut game = fresh();
    let path =
        std::env::temp_dir().join(format!("feral_contracts_legacy_{}.bin", std::process::id()));
    game.save(&path).unwrap();

    // Strip the two fields back out, which is exactly the file a build
    // without them wrote. Since v29 the payload is field-named RON, so this
    // must load as empty vectors — no migration, and no version bump.
    let text = std::fs::read_to_string(&path).unwrap();
    let version = text.split_once('\n').unwrap().0.to_string();
    let stripped: String = text
        .lines()
        .filter(|line| {
            let t = line.trim();
            !t.starts_with("contracts:") && !t.starts_with("contracts_done:")
        })
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        stripped.lines().count() < text.lines().count(),
        "the fields have to be present to strip, or this test proves nothing"
    );
    std::fs::write(&path, stripped).unwrap();

    let loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);
    let held = loaded.world.resource::<ActiveContracts>();
    assert!(held.active.is_empty());
    assert!(held.done.is_empty());
    assert_eq!(
        version.trim().parse::<u32>().unwrap(),
        crate::save::SAVE_FORMAT_VERSION,
        "an additive field behind #[serde(default)] costs no version bump"
    );
}

#[test]
fn an_absent_contracts_directory_is_silent() {
    let dir = std::env::temp_dir().join(format!("fp_contracts_absent_{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    let (db, warnings) = ContractDb::load_dir(&dir).unwrap();
    assert_eq!(db.iter().count(), 0);
    assert!(
        warnings.is_empty(),
        "an install without contracts is the pre-contract game, not a fault: {warnings:?}"
    );
}

// ---------------------------------------------------------------------------
// The kill counter
// ---------------------------------------------------------------------------

/// `award_loot` directly rather than through a fight, for the reason
/// `killing_a_boss_records_its_species` does the same: `RunFeats` is a
/// per-tick drain queue, so a test that reads it after a resolved round sees
/// an empty queue whether the record was ever made or not. That the record
/// happens on a *real* kill is asserted in the `contract_system` tests, where
/// the observable is contract progress rather than the queue.
#[test]
fn every_kill_records_its_species() {
    let mut game = fresh();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let wild = game.spawn_wild_creature("drone", pos.x, pos.y).unwrap();
    game.award_loot(wild);

    assert_eq!(
        game.world.resource::<crate::resources::RunFeats>().kills,
        vec!["drone".to_string()],
        "the one door every kill in the game passes through"
    );
}

#[test]
fn a_boss_kill_lands_in_both_fields() {
    let mut game = fresh();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let boss = game.spawn_wild_creature("overseer", pos.x, pos.y).unwrap();
    game.award_loot(boss);

    let feats = game.world.resource::<crate::resources::RunFeats>();
    assert_eq!(feats.bosses_defeated, vec!["overseer".to_string()]);
    assert_eq!(
        feats.kills,
        vec!["overseer".to_string()],
        "a boss is also a kill. The two fields are separate and each has \
         exactly one drainer, which is what removes any ordering dependency \
         between achievement_system and contract_system — merging them would \
         silently make an unchained system order-sensitive."
    );
}

// ---------------------------------------------------------------------------
// contract_system — the one writer of progress
// ---------------------------------------------------------------------------

fn progress_of(game: &Game, id: &str) -> u32 {
    game.world
        .resource::<ActiveContracts>()
        .active
        .iter()
        .find(|c| c.def.id == ContractId::from(id))
        .expect("the contract is still active")
        .progress
}

fn kill(game: &mut Game, species: &str) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    let wild = game.spawn_wild_creature(species, pos.x, pos.y).unwrap();
    game.award_loot(wild);
    game.world.despawn(wild);
}

#[test]
fn a_named_kill_contract_advances_only_on_that_species() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "drones",
            Objective::Kill {
                species: Some("drone".to_string()),
                count: 3,
            },
            vec![Reward::Credits(10)],
        ),
        0,
    );

    kill(&mut game, "glitch");
    game.tick();
    assert_eq!(progress_of(&game, "drones"), 0, "a glitch is not a drone");

    kill(&mut game, "drone");
    game.tick();
    assert_eq!(progress_of(&game, "drones"), 1);
}

#[test]
fn an_unnamed_kill_contract_advances_on_anything() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "anything",
            Objective::Kill {
                species: None,
                count: 5,
            },
            vec![Reward::Credits(10)],
        ),
        0,
    );

    kill(&mut game, "glitch");
    kill(&mut game, "drone");
    game.tick();
    assert_eq!(progress_of(&game, "anything"), 2);
}

#[test]
fn progress_never_runs_past_the_target() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "two",
            Objective::Kill {
                species: None,
                count: 2,
            },
            vec![Reward::Credits(10)],
        ),
        0,
    );
    for _ in 0..4 {
        kill(&mut game, "drone");
    }
    game.tick();
    // The contract may have been settled and dropped by now; if it is still
    // held, it must not be showing 4 of 2.
    if let Some(held) = game
        .world
        .resource::<ActiveContracts>()
        .active
        .iter()
        .find(|c| c.def.id == ContractId::from("two"))
    {
        assert_eq!(held.progress, 2, "a bar cannot be more than full");
    }
}

#[test]
fn a_descend_contract_reads_the_locale_and_never_a_surface_position() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "deep",
            Objective::Descend { depth: 3 },
            vec![Reward::Xp(10)],
        ),
        0,
    );

    // The regression that matters: a party standing a long way from origin
    // on the *surface* has descended nowhere. `Position` is pinned to the
    // entrance tile underground, so a depth taken from it is a surface
    // coordinate — the trap `nest_aggro_tick` needs its guard for.
    {
        let player = game.player_entity();
        let mut pos = game.world.get_mut::<Position>(player).unwrap();
        pos.x = 60;
        pos.y = 60;
    }
    game.tick();
    assert_eq!(progress_of(&game, "deep"), 0);

    // Two frames down is still not three.
    set_depth(&mut game, 2);
    game.tick();
    assert_eq!(progress_of(&game, "deep"), 0);

    set_depth(&mut game, 3);
    game.tick();
    assert!(
        game.world
            .resource::<ActiveContracts>()
            .active
            .iter()
            .all(|c| c.def.id != ContractId::from("deep"))
            || progress_of(&game, "deep") == 1
    );
}

/// Rewrites `Locale` to a Stack frame at `depth` without walking one. What is
/// under test is what the objective *reads*, not how the party got there.
fn set_depth(game: &mut Game, depth: u32) {
    *game.world.resource_mut::<crate::resources::Locale>() = crate::resources::Locale::Stack {
        depth,
        frames: 5,
        x: 1,
        y: 1,
        facing: crate::stack::Dir::North,
        entrance: (0, 0),
    };
}

#[test]
fn a_breach_contract_reads_the_zone() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "outward",
            Objective::Breach { zone: 3 },
            vec![Reward::Xp(10)],
        ),
        0,
    );
    game.tick();
    assert_eq!(progress_of(&game, "outward"), 0);

    set_zone(&mut game, 3);
    game.tick();
    assert!(
        game.world
            .resource::<ActiveContracts>()
            .active
            .iter()
            .all(|c| c.def.id != ContractId::from("outward"))
            || progress_of(&game, "outward") == 1
    );
}

#[test]
fn a_build_contract_advances_only_once_one_is_standing() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "refine",
            Objective::Build {
                structure: "refinery".to_string(),
            },
            vec![Reward::Xp(10)],
        ),
        0,
    );
    game.tick();
    assert_eq!(progress_of(&game, "refine"), 0);

    // A different structure is not the one asked for.
    game.world.spawn((
        Structure {
            kind: "mining_node".to_string(),
        },
        Position { x: 3, y: 3 },
    ));
    game.tick();
    assert_eq!(progress_of(&game, "refine"), 0);

    game.world.spawn((
        Structure {
            kind: "refinery".to_string(),
        },
        Position { x: 4, y: 3 },
    ));
    game.tick();
    assert!(
        game.world
            .resource::<ActiveContracts>()
            .active
            .iter()
            .all(|c| c.def.id != ContractId::from("refine"))
            || progress_of(&game, "refine") == 1
    );
}

#[test]
fn a_deliver_contract_is_untouched_by_the_system() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "haul",
            Objective::Deliver {
                item: crate::items::ItemId::from("core_fragment"),
                count: 4,
            },
            vec![Reward::Credits(10)],
        ),
        0,
    );
    // The player starts holding Core Fragments, so a system that polled cargo
    // would advance this. Delivery is an act, not a state.
    for _ in 0..3 {
        game.tick();
    }
    assert_eq!(progress_of(&game, "haul"), 0);
}

// ---------------------------------------------------------------------------
// Completion and payout
// ---------------------------------------------------------------------------

fn carried(game: &Game, item: &str) -> u32 {
    game.world
        .get::<Inventory>(game.player_entity())
        .unwrap()
        .count(&crate::items::ItemId::from(item))
}

/// A contract already at its target, so the settle is what is under test
/// rather than the counting.
fn give_finished(game: &mut Game, id: &str, reward: Vec<Reward>) {
    give(
        game,
        def(
            id,
            Objective::Kill {
                species: None,
                count: 1,
            },
            reward,
        ),
        1,
    );
}

#[test]
fn a_finished_contract_pays_each_reward_once_and_is_filed_as_done() {
    let mut game = fresh();
    let credits_before = carried(&game, "credits");
    let cells_before = carried(&game, "power_cell");
    let xp_before = game
        .world
        .get::<Experience>(game.player_entity())
        .unwrap()
        .xp;

    give_finished(
        &mut game,
        "paid",
        vec![
            Reward::Credits(40),
            Reward::Item(crate::items::ItemId::from("power_cell"), 3),
            Reward::Xp(25),
        ],
    );
    game.tick();

    assert_eq!(carried(&game, "credits"), credits_before + 40);
    assert_eq!(carried(&game, "power_cell"), cells_before + 3);
    assert!(
        game.world
            .get::<Experience>(game.player_entity())
            .unwrap()
            .xp
            > xp_before
            || game
                .world
                .get::<Experience>(game.player_entity())
                .unwrap()
                .level
                > 1,
        "XP goes through award_player_xp, so a level-up full-heals as it does from a kill"
    );

    let held = game.world.resource::<ActiveContracts>();
    assert!(held.active.is_empty(), "a finished contract is not held");
    assert_eq!(held.done, vec![ContractId::from("paid")]);
}

#[test]
fn a_finished_contract_does_not_pay_twice() {
    let mut game = fresh();
    give_finished(&mut game, "once", vec![Reward::Credits(40)]);
    let before = carried(&game, "credits");
    for _ in 0..5 {
        game.tick();
    }
    assert_eq!(carried(&game, "credits"), before + 40);
}

#[test]
fn a_gear_reward_is_always_ordinary() {
    let mut game = fresh();
    give_finished(
        &mut game,
        "gear",
        vec![Reward::Item(crate::items::ItemId::from("kinetic_edge"), 1)],
    );
    game.tick();

    // The sibling of `crafted_gear_is_never_rare`. `Game::grant_gear_drop` is
    // the one door a copy above Ordinary enters the game by, and a contract
    // payout is closer to made gear than found gear — so the plain copy lands
    // in `Inventory`, and `GearCopies` (the special-copy store) stays empty.
    assert_eq!(carried(&game, "kinetic_edge"), 1);
    assert!(
        game.world
            .get::<GearCopies>(game.player_entity())
            .unwrap()
            .copies
            .is_empty(),
        "a contract must not mint a rare or fused copy"
    );
}

#[test]
fn a_completion_announced_mid_battle_survives_the_prune() {
    let mut game = fresh();
    let player = game.player_entity();
    let wild = spawn_wild_on_player_tile(&mut game);
    insert_battle(&mut game, player, vec![wild]);
    game.world.resource_mut::<MessageLog>().open_battle();

    give_finished(&mut game, "midfight", vec![Reward::Credits(5)]);
    game.tick();
    game.world
        .resource_mut::<MessageLog>()
        .retain_outcomes_since_battle();

    let lines = game.world.resource::<MessageLog>();
    assert!(
        lines
            .recent(50)
            .iter()
            .any(|line| line.text.contains("Contract midfight")),
        "a plain log() is Info and is deleted when the battle ends — the one \
         moment the player is least able to notice a payout arriving: {:?}",
        lines.recent(50).iter().map(|l| &l.text).collect::<Vec<_>>()
    );
}
