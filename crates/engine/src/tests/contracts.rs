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
                "terminate.ron",
                r#"(id: "terminate", name: "Terminate", description: "d",
                    objective: Terminate(species: Some("drone"), count: 6),
                    reward: [Credits(40)])"#,
            ),
            (
                "terminate_any.ron",
                r#"(id: "terminate_any", name: "Any", description: "d",
                    objective: Terminate(species: None, count: 3),
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
    assert_eq!(
        target("terminate"),
        6,
        "a counting objective targets its count"
    );
    assert_eq!(target("terminate_any"), 3);
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
        db.get(&ContractId::from("terminate")).unwrap().objective,
        Objective::Terminate {
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
            Objective::Terminate {
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
fn a_shipped_delivery_never_asks_for_the_bank() {
    let assets = test_assets_dir();
    let (contracts, _) = shipped_contracts();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items")).unwrap();

    for def in contracts.iter() {
        let Objective::Deliver { item, .. } = &def.objective else {
            continue;
        };
        assert!(
            !items.get(item.as_str()).is_some_and(|d| d.banked),
            "{} asks for {item}, which is banked. A bank shares `Inventory` \
             with cargo, so the hand-over would silently work — while \
             `PlayerStatus::inventory` omits the row, so the player is asked \
             for something no cargo screen will ever show them, and paying it \
             spends research progress rather than stock.",
            def.id
        );
    }
}

#[test]
fn every_objective_variant_ships_at_least_once() {
    let (contracts, _) = shipped_contracts();
    let mut seen = [false; 5];
    for def in contracts.iter() {
        let slot = match &def.objective {
            Objective::Terminate { .. } => 0,
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
            Objective::Terminate {
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
            Objective::Terminate {
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
            Objective::Terminate {
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
            Objective::Terminate {
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
            Objective::Terminate {
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

// ---------------------------------------------------------------------------
// The Contract Broker
// ---------------------------------------------------------------------------

/// Deploys a structure of `kind` at `(x, y)` without paying for it or asking
/// whether the base reaches — what is under test is the flag, not building.
fn deploy(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity {
    game.world
        .spawn((
            Structure {
                kind: kind.to_string(),
            },
            Position { x, y },
            Glyph {
                ch: '!',
                color: GlyphColor::Yellow,
            },
        ))
        .id()
}

#[test]
fn the_shipped_broker_is_the_one_structure_that_issues_contracts() {
    let game = fresh();
    let defs = game.structure_defs();
    let brokers: Vec<&str> = defs
        .iter()
        .filter(|d| d.issues_contracts)
        .map(|d| d.id.as_str())
        .collect();
    assert_eq!(
        brokers,
        vec!["contract_broker"],
        "exactly one shipped structure issues contracts"
    );
}

#[test]
fn a_deployed_broker_reports_the_flag_and_nothing_else_does() {
    let mut game = fresh();
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    deploy(&mut game, "contract_broker", pos.x + 1, pos.y);
    deploy(&mut game, "mining_node", pos.x + 2, pos.y);

    let views = game.view_entities(10, 10);
    let broker = views
        .iter()
        .find(|v| v.issues_contracts)
        .expect("the Broker reports the flag");
    assert_eq!(broker.pos, (pos.x + 1, pos.y));
    assert_eq!(
        views.iter().filter(|v| v.issues_contracts).count(),
        1,
        "a mining node is not a Broker"
    );
}

/// Contracts are what onboards a new run, so the Broker is deliberately
/// behind no research at all — a structure no research file names is
/// unlocked by default (`Game::structure_unlocked`). A node added later that
/// names it would re-gate it silently, which is what this catches.
#[test]
fn the_broker_is_buildable_from_turn_one() {
    let game = fresh();
    assert!(
        game.world
            .resource::<crate::research::ResearchDb>()
            .all()
            .all(|d| !d.unlocks_structures.iter().any(|s| s == "contract_broker")),
        "no research node may gate the Broker"
    );
    assert!(
        game.buildable_structure_defs()
            .iter()
            .any(|d| d.id == "contract_broker"),
        "the Broker must be in the build menu on turn one"
    );
}

// ---------------------------------------------------------------------------
// The derived board
// ---------------------------------------------------------------------------

/// A Broker one tile from the player, which is where a board is read from.
fn deploy_broker(game: &mut Game) {
    let pos = *game.world.get::<Position>(game.player_entity()).unwrap();
    deploy(game, "contract_broker", pos.x + 1, pos.y);
}

fn board_ids(game: &mut Game) -> Vec<ContractId> {
    game.contract_board()
        .expect("a Broker is deployed")
        .into_iter()
        .map(|row| row.id)
        .collect()
}

#[test]
fn there_is_no_board_without_a_broker() {
    let mut game = fresh();
    assert!(
        game.contract_board().is_none(),
        "one call answers both `is there a board` and `what is on it`, so no \
         screen asks those separately and then disagrees"
    );
    deploy_broker(&mut game);
    assert!(game.contract_board().is_some());
}

#[test]
fn the_same_board_comes_back_after_a_save_and_load() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let before = board_ids(&mut game);
    assert!(!before.is_empty(), "the shipped set fills a zone-1 board");

    let path =
        std::env::temp_dir().join(format!("feral_contract_board_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        board_ids(&mut loaded),
        before,
        "the whole reason the board is derived rather than saved: the player \
         is shown an offer before they accept it"
    );
}

#[test]
fn reading_the_board_spends_no_shared_rng() {
    let mut untouched = fresh();
    let mut read = fresh();
    deploy_broker(&mut read);

    let draw =
        |game: &mut Game| -> u32 { game.world.resource_mut::<GameRng>().0.random_range(0..1000) };

    assert_eq!(
        draw(&mut untouched),
        draw(&mut read),
        "same seed, same stream"
    );
    let _ = read.contract_board();
    assert_eq!(
        draw(&mut untouched),
        draw(&mut read),
        "opening a screen must not shift the run's RNG stream — the failure \
         this repo has been bitten by three times"
    );
}

#[test]
fn the_board_rotates_with_the_epoch() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let before = board_ids(&mut game);

    game.world.resource_mut::<GameClock>().tick += crate::tuning::CONTRACT_REFRESH_CYCLES as u64;
    let after = board_ids(&mut game);
    assert_ne!(
        before, after,
        "the offers stand for an epoch and then re-derive; a board that never \
         moved would read as static"
    );
}

#[test]
fn a_contract_above_the_current_zone_is_not_offered() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let zone = game.world.resource::<ZoneLevel>().0;
    let offered = board_ids(&mut game);
    for id in offered {
        let min_zone = game
            .world
            .resource::<crate::contracts::ContractDb>()
            .get(&id)
            .map(|d| d.min_zone)
            .unwrap_or(0);
        assert!(
            min_zone <= zone,
            "{id} needs zone {min_zone} and the run is in {zone}"
        );
    }
}

#[test]
fn an_active_or_finished_contract_is_not_offered_again() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let offered = board_ids(&mut game);
    let taken = offered.first().cloned().expect("the board has offers");

    // Through `repeatable` and `accept_contract` rather than a db lookup: the
    // first offer may be a rolled contract, which has no entry in the db at
    // all, and reaching for one is the exact bug the board-carries-the-def
    // shape exists to remove.
    let repeatable = game
        .world
        .resource::<crate::contracts::ContractDb>()
        .repeatable(&taken);
    assert_eq!(game.accept_contract(&taken), Ok(()));
    assert!(
        !board_ids(&mut game).contains(&taken),
        "a contract already in hand is not offered again"
    );

    game.world.resource_mut::<ActiveContracts>().active.clear();
    game.world
        .resource_mut::<ActiveContracts>()
        .done
        .push(taken.clone());
    let after_done = board_ids(&mut game);
    if repeatable {
        assert!(
            after_done.contains(&taken),
            "a repeatable contract comes back once it is finished"
        );
    } else {
        assert!(
            !after_done.contains(&taken),
            "a finished one-shot contract is done for the run"
        );
    }
}

#[test]
fn a_repeatable_contract_returns_to_the_board_and_a_one_shot_does_not() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let db = game.world.resource::<crate::contracts::ContractDb>();
    let repeatable = db
        .iter()
        .find(|d| d.repeatable && d.min_zone <= 1)
        .map(|d| d.id.clone())
        .expect("the shipped set has a repeatable contract available in zone 1");
    let one_shot = db
        .iter()
        .find(|d| !d.repeatable && d.min_zone <= 1)
        .map(|d| d.id.clone())
        .expect("and a one-shot one");

    game.world.resource_mut::<ActiveContracts>().done = vec![repeatable.clone(), one_shot.clone()];

    // Widen the pool to the whole catalogue so this is not a test about which
    // three the roll happened to pick.
    let offerable: Vec<_> = game
        .offerable_contracts()
        .into_iter()
        .map(|def| def.id)
        .collect();
    assert!(offerable.contains(&repeatable));
    assert!(!offerable.contains(&one_shot));
}

#[test]
fn active_contracts_read_anywhere_including_underground() {
    let mut game = fresh();
    give(
        &mut game,
        def(
            "held",
            Objective::Terminate {
                species: None,
                count: 4,
            },
            vec![Reward::Credits(10)],
        ),
        2,
    );
    set_depth(&mut game, 2);

    let rows = game.active_contracts();
    assert_eq!(rows.len(), 1);
    assert_eq!(rows[0].progress, 2);
    assert_eq!(rows[0].target, 4);
    assert!(!rows[0].objective_line.is_empty());
    assert!(!rows[0].reward_line.is_empty());
    assert!(
        game.contract_board().is_none(),
        "a Broker on the surface is not in reach from four frames down — the \
         player's Position is pinned to the entrance tile the whole time"
    );
}

// ---------------------------------------------------------------------------
// Accept, abandon, deliver
// ---------------------------------------------------------------------------

use crate::game::contracts::ContractRefusal;

fn first_offer(game: &mut Game) -> ContractId {
    board_ids(game)
        .into_iter()
        .next()
        .expect("the board has offers")
}

#[test]
fn accepting_puts_a_contract_in_hand_at_zero_progress() {
    let mut game = fresh();
    deploy_broker(&mut game);
    game.world.resource_mut::<GameClock>().tick = 90;
    let id = first_offer(&mut game);

    assert_eq!(game.accept_contract(&id), Ok(()));
    let held = game.world.resource::<ActiveContracts>();
    assert_eq!(held.active.len(), 1);
    assert_eq!(held.active[0].def.id, id);
    assert_eq!(held.active[0].progress, 0);
    assert_eq!(held.active[0].accepted_tick, 90);
}

#[test]
fn a_fourth_contract_is_refused_rather_than_silently_capped() {
    let mut game = fresh();
    deploy_broker(&mut game);
    for i in 0..crate::tuning::MAX_ACTIVE_CONTRACTS {
        give(
            &mut game,
            def(
                &format!("filler{i}"),
                Objective::Breach { zone: 99 },
                vec![Reward::Credits(1)],
            ),
            0,
        );
    }
    let id = first_offer(&mut game);
    assert_eq!(game.accept_contract(&id), Err(ContractRefusal::TooMany));
    assert_eq!(
        game.world.resource::<ActiveContracts>().active.len(),
        crate::tuning::MAX_ACTIVE_CONTRACTS
    );
}

#[test]
fn accepting_something_not_on_the_board_is_refused() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let offered = board_ids(&mut game);
    let elsewhere = game
        .world
        .resource::<crate::contracts::ContractDb>()
        .iter()
        .map(|d| d.id.clone())
        .find(|id| !offered.contains(id))
        .expect("the shipped set is larger than one board");

    assert_eq!(
        game.accept_contract(&elsewhere),
        Err(ContractRefusal::NotOffered)
    );
    assert_eq!(
        game.accept_contract(&ContractId::from("no_such_contract")),
        Err(ContractRefusal::NotOffered)
    );
}

#[test]
fn a_contract_already_in_hand_cannot_be_taken_twice() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let id = first_offer(&mut game);
    game.accept_contract(&id).unwrap();
    assert_eq!(
        game.accept_contract(&id),
        Err(ContractRefusal::AlreadyActive)
    );
}

#[test]
fn abandoning_drops_a_contract_and_loses_its_progress() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let id = first_offer(&mut game);
    game.accept_contract(&id).unwrap();
    game.world.resource_mut::<ActiveContracts>().active[0].progress = 2;

    assert!(game.abandon_contract(&id));
    assert!(game.world.resource::<ActiveContracts>().active.is_empty());
    assert!(
        !game.world.resource::<ActiveContracts>().done.contains(&id),
        "abandoning is not finishing — it must not file the contract as done"
    );
    assert!(!game.abandon_contract(&id), "nothing left to abandon");

    game.accept_contract(&id).unwrap();
    assert_eq!(
        game.world.resource::<ActiveContracts>().active[0].progress,
        0,
        "progress is lost, not banked"
    );
}

#[test]
fn a_finished_one_shot_cannot_be_accepted_again_and_a_repeatable_can() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let db = game.world.resource::<crate::contracts::ContractDb>();
    let one_shot = db
        .iter()
        .find(|d| !d.repeatable && d.min_zone <= 1)
        .map(|d| d.id.clone())
        .unwrap();
    let repeatable = db
        .iter()
        .find(|d| d.repeatable && d.min_zone <= 1)
        .map(|d| d.id.clone())
        .unwrap();
    game.world.resource_mut::<ActiveContracts>().done = vec![one_shot.clone(), repeatable.clone()];

    assert_eq!(
        game.accept_contract(&one_shot),
        Err(ContractRefusal::AlreadyDone)
    );
    // The repeatable one is refused only if this board did not roll it, which
    // is a different refusal and not the one under test.
    assert_ne!(
        game.accept_contract(&repeatable),
        Err(ContractRefusal::AlreadyDone),
        "a repeatable contract may be taken again once it is finished"
    );
}

fn deliver_fixture(game: &mut Game, count: u32) -> ContractId {
    let id = ContractId::from("quota");
    give(
        game,
        def(
            id.as_str(),
            Objective::Deliver {
                item: crate::items::ItemId::from("core_fragment"),
                count,
            },
            vec![Reward::Credits(5)],
        ),
        0,
    );
    id
}

#[test]
fn delivering_takes_exactly_what_is_needed_and_no_more() {
    let mut game = fresh();
    deploy_broker(&mut game);
    // The player starts with 5 Core Fragments; ask for 3.
    let held = carried(&game, "core_fragment");
    assert!(held >= 4, "the fixture needs spare stock, has {held}");
    let id = deliver_fixture(&mut game, 3);

    assert_eq!(game.deliver_to_contract(&id), Ok(3));
    assert_eq!(
        carried(&game, "core_fragment"),
        held - 3,
        "a contract must not eat cargo it did not ask for"
    );
    assert!(
        game.world.resource::<ActiveContracts>().active.is_empty(),
        "filling a Deliver objective completes it through the same door the \
         polled objectives use"
    );
    assert!(game.world.resource::<ActiveContracts>().done.contains(&id));
}

#[test]
fn a_partial_delivery_leaves_the_contract_open() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let held = carried(&game, "core_fragment");
    let id = deliver_fixture(&mut game, held + 4);

    assert_eq!(game.deliver_to_contract(&id), Ok(held));
    assert_eq!(carried(&game, "core_fragment"), 0);
    assert_eq!(progress_of(&game, "quota"), held);
}

#[test]
fn a_refused_delivery_leaves_cargo_untouched() {
    let mut game = fresh();
    let before = carried(&game, "core_fragment");
    let id = deliver_fixture(&mut game, 3);

    // No Broker in range: every refusal lands before anything leaves cargo,
    // the ordering `use_symlink` and `install_routine` follow.
    assert_eq!(
        game.deliver_to_contract(&id),
        Err(ContractRefusal::NotOffered)
    );
    assert_eq!(carried(&game, "core_fragment"), before);

    deploy_broker(&mut game);
    let mut inventory = game
        .world
        .get_mut::<Inventory>(game.player_entity())
        .unwrap();
    inventory.take(crate::items::ItemId::from("core_fragment"), before);
    assert_eq!(
        game.deliver_to_contract(&id),
        Err(ContractRefusal::NothingToDeliver)
    );
    assert_eq!(carried(&game, "core_fragment"), 0);
    assert_eq!(progress_of(&game, "quota"), 0);
}

#[test]
fn delivering_against_a_contract_that_is_not_held_is_refused() {
    let mut game = fresh();
    deploy_broker(&mut game);
    assert_eq!(
        game.deliver_to_contract(&ContractId::from("quota")),
        Err(ContractRefusal::NotOffered)
    );
}

// ---------------------------------------------------------------------------
// Templates, and the contracts they roll
// ---------------------------------------------------------------------------

use crate::contracts::{ContractTemplate, TemplateObjective, TemplatePools};
use rand::SeedableRng;
use rand::rngs::StdRng;

/// A contract directory with a `templates/` subdirectory beside the authored
/// files, which is where `ContractDb::load_dir` looks for them.
fn load_with_templates(
    tag: &str,
    contracts: &[(&str, &str)],
    templates: &[(&str, &str)],
) -> (ContractDb, Vec<String>) {
    let dir = contract_dir(tag, contracts);
    std::fs::create_dir_all(dir.join("templates")).unwrap();
    for (name, body) in templates {
        std::fs::write(dir.join("templates").join(name), body).unwrap();
    }
    let loaded = ContractDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&dir);
    loaded
}

/// Pools with one of everything, so a roll has exactly one valid answer and a
/// test never depends on which candidate a seed picked.
fn one_of_each(zone: u32) -> TemplatePools {
    TemplatePools {
        species: vec![("drone".to_string(), "Drone".to_string())],
        items: vec![(ItemId::from("core_fragment"), "Core Fragment".to_string())],
        structures: vec![("refinery".to_string(), "Refinery".to_string())],
        zone,
    }
}

fn template(id: &str, objective: TemplateObjective, reward: Vec<Reward>) -> ContractTemplate {
    ContractTemplate {
        id: ContractId::from(id),
        name: format!("Template {id}"),
        description: "d".to_string(),
        objective,
        reward,
        min_zone: 0,
        repeatable: false,
    }
}

fn rng() -> StdRng {
    StdRng::seed_from_u64(42)
}

#[test]
fn an_absent_templates_directory_is_silent_and_leaves_no_templates() {
    let (db, warnings) = load(
        "no_templates",
        &[(
            "one.ron",
            r#"(id: "one", name: "One", description: "d",
                objective: Breach(zone: 2), reward: [Credits(10)])"#,
        )],
    );
    assert!(warnings.is_empty());
    assert_eq!(
        db.templates().count(),
        0,
        "an install with no templates is the pre-template game, exactly as an \
         install with no contracts is the pre-contract one"
    );
}

#[test]
fn a_malformed_template_is_skipped_with_a_warning_rather_than_a_panic() {
    let (db, warnings) = load_with_templates(
        "bad_template",
        &[],
        &[
            ("broken.ron", "(this is not ron"),
            (
                "fine.ron",
                r#"(id: "hunt", name: "Hunt {target}", description: "d {count} {target}",
                    objective: Terminate(count: (4, 8)), reward: [Credits(5)])"#,
            ),
        ],
    );
    assert_eq!(warnings.len(), 1, "one bad file, one warning: {warnings:?}");
    assert_eq!(db.templates().count(), 1, "the good one still loads");
}

#[test]
fn an_authored_id_may_not_contain_the_rolled_separator() {
    let (db, warnings) = load(
        "sep",
        &[(
            "clash.ron",
            r#"(id: "hunt#drone-6", name: "Clash", description: "d",
                objective: Breach(zone: 2), reward: [Credits(10)])"#,
        )],
    );
    assert_eq!(db.iter().count(), 0, "refused rather than loaded");
    assert_eq!(warnings.len(), 1);
    assert!(
        warnings[0].contains('#'),
        "the warning has to name the character: {warnings:?}"
    );
}

#[test]
fn a_rolled_terminate_names_a_species_the_sector_actually_fields() {
    let t = template(
        "hunt",
        TemplateObjective::Terminate { count: (4, 8) },
        vec![Reward::Credits(5)],
    );
    let def = t
        .roll(&mut rng(), &one_of_each(1))
        .expect("one valid answer");
    match def.objective {
        Objective::Terminate { species, count } => {
            assert_eq!(species, Some("drone".to_string()));
            assert!(
                (4..=8).contains(&count),
                "the rolled count stays inside the authored range, got {count}"
            );
        }
        other => panic!("a Terminate template rolls a Terminate objective, got {other:?}"),
    }
}

#[test]
fn a_rolled_contract_reads_its_own_parameters_back_in_its_name_and_description() {
    let mut t = template(
        "hunt",
        TemplateObjective::Terminate { count: (6, 6) },
        vec![Reward::Credits(5)],
    );
    t.name = "Hunt: {target}".to_string();
    t.description = "Terminate {count} {target} out past the slab.".to_string();

    let def = t.roll(&mut rng(), &one_of_each(1)).unwrap();
    assert_eq!(def.name, "Hunt: Drone", "the display name, not the id");
    assert_eq!(
        def.description, "Terminate 6 Drone out past the slab.",
        "the description is the one field a template cannot derive, so it \
         authors the hole and the roll fills it"
    );
}

#[test]
fn a_rolled_build_never_names_a_structure_already_standing() {
    let t = template("commission", TemplateObjective::Build, vec![Reward::Xp(50)]);
    let mut pools = one_of_each(1);
    pools.structures.clear();
    assert!(
        t.roll(&mut rng(), &pools).is_none(),
        "with nothing left to build the template rolls nothing at all — a \
         Build of something already deployed completes the instant it is \
         accepted"
    );
}

#[test]
fn a_rolled_breach_always_targets_a_sector_deeper_than_this_one() {
    let t = template(
        "expansion",
        TemplateObjective::Breach { zone: (2, 6) },
        vec![Reward::Credits(100)],
    );
    for zone in 1..=6 {
        let rolled = t.roll(&mut rng(), &one_of_each(zone));
        match rolled {
            Some(def) => match def.objective {
                Objective::Breach { zone: want } => assert!(
                    want > zone,
                    "a Breach at or below the current sector completes on \
                     acceptance; rolled {want} in zone {zone}"
                ),
                other => panic!("expected a Breach, got {other:?}"),
            },
            // Zone 6 has nothing above it inside the authored range, and an
            // empty roll is the right answer rather than a clamped one.
            None => assert_eq!(zone, 6),
        }
    }
}

#[test]
fn a_rolled_descend_never_targets_the_surface() {
    let t = template(
        "sounding",
        TemplateObjective::Descend { depth: (0, 4) },
        vec![Reward::Xp(200)],
    );
    for seed in 0..32 {
        let def = t
            .roll(&mut StdRng::seed_from_u64(seed), &one_of_each(1))
            .unwrap();
        match def.objective {
            Objective::Descend { depth } => assert!(
                depth >= 1,
                "depth 0 is the surface, and `depth >= want` makes it finish \
                 on acceptance"
            ),
            other => panic!("expected a Descend, got {other:?}"),
        }
    }
}

#[test]
fn a_template_with_nothing_valid_to_name_rolls_nothing() {
    let empty = TemplatePools {
        species: vec![],
        items: vec![],
        structures: vec![],
        zone: 1,
    };
    for objective in [
        TemplateObjective::Terminate { count: (4, 8) },
        TemplateObjective::Deliver { count: (4, 8) },
        TemplateObjective::Build,
    ] {
        let t = template("t", objective, vec![Reward::Credits(5)]);
        assert!(
            t.roll(&mut rng(), &empty).is_none(),
            "an unfinishable contract is worse than no contract"
        );
    }
}

#[test]
fn a_rolled_reward_scales_with_how_much_the_contract_asks_for() {
    let t = template(
        "quota",
        TemplateObjective::Deliver { count: (10, 10) },
        vec![Reward::Credits(3), Reward::Xp(4)],
    );
    let def = t.roll(&mut rng(), &one_of_each(1)).unwrap();
    assert_eq!(
        def.reward,
        vec![Reward::Credits(30), Reward::Xp(40)],
        "a template's reward is authored per unit of `objective.target()`, so \
         asking for ten pays ten times — one rule, and the same one that \
         already decides what `target()` means"
    );

    let flat = template(
        "sounding",
        TemplateObjective::Descend { depth: (3, 3) },
        vec![Reward::Credits(50)],
    );
    let def = flat.roll(&mut rng(), &one_of_each(1)).unwrap();
    assert_eq!(
        def.reward,
        vec![Reward::Credits(50)],
        "a state-shaped objective targets 1, so it pays the authored figure \
         flat — no separate rule for it"
    );
}

#[test]
fn a_rolled_id_names_the_template_it_came_from_and_the_roll_that_made_it() {
    let t = template(
        "hunt",
        TemplateObjective::Terminate { count: (6, 6) },
        vec![Reward::Credits(5)],
    );
    let def = t.roll(&mut rng(), &one_of_each(1)).unwrap();
    assert_eq!(
        def.id,
        ContractId::from("hunt#drone-6"),
        "the same roll has to produce the same id, or the board would offer a \
         different contract after a reload"
    );
}

/// The shipped templates, loaded through a real game.
#[test]
fn the_shipped_templates_reach_a_loaded_game() {
    let game = fresh();
    let db = game.world.resource::<ContractDb>();
    let ids: Vec<&str> = db.templates().map(|t| t.id.as_str()).collect();
    assert_eq!(
        ids,
        vec!["commission", "expansion", "hunt", "requisition", "sounding"],
        "one template per objective shape, in the db's stable id order"
    );
}

#[test]
fn every_shipped_template_rolls_something_finishable_in_a_fresh_sector() {
    let mut game = fresh();
    place_home(&mut game, 0, 1);
    let pools = game.template_pools();
    assert!(
        !pools.species.is_empty(),
        "the base's doorstep has to field programs, or a Hunt can never roll — \
         the Home tile itself is Biome::Platform and fields none by design"
    );
    assert!(!pools.items.is_empty(), "and something to deliver");
    assert!(!pools.structures.is_empty(), "and something left to build");

    let templates: Vec<_> = game
        .world
        .resource::<ContractDb>()
        .templates()
        .cloned()
        .collect();
    for t in templates {
        // Zone 1, so `expansion` (min_zone 2) is not yet on offer; it is
        // still asked to roll, since a template that cannot roll at all is a
        // template that would never appear.
        let rolled = t.roll(&mut rng(), &pools);
        assert!(
            rolled.is_some(),
            "{} rolls nothing against a fresh sector",
            t.id
        );
    }
}

#[test]
fn a_rolled_contract_can_be_accepted() {
    let mut game = fresh();
    deploy_broker(&mut game);

    // Walk the epochs until a rolled contract surfaces on the board, rather
    // than depending on which three slots one seed happened to pick.
    let mut rolled = None;
    for _ in 0..40 {
        if let Some(id) = board_ids(&mut game)
            .into_iter()
            .find(|id| id.as_str().contains('#'))
        {
            rolled = Some(id);
            break;
        }
        for _ in 0..crate::tuning::CONTRACT_REFRESH_CYCLES {
            game.tick();
        }
    }
    let id = rolled.expect("the shipped templates reach a zone-1 board");

    assert_eq!(
        game.accept_contract(&id),
        Ok(()),
        "the regression this whole shape exists for: every step of the accept \
         path used to re-resolve the def out of ContractDb by id, which a \
         rolled contract has no entry in — so it was refused as NotOffered \
         while sitting visibly on the board"
    );
    let held = game.world.resource::<ActiveContracts>();
    assert_eq!(held.active.len(), 1);
    assert_eq!(held.active[0].def.id, id);
}

#[test]
fn the_same_rolled_contract_comes_back_after_a_save_and_load() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let before = board_ids(&mut game);
    assert!(
        before.iter().any(|id| id.as_str().contains('#')),
        "this test is only worth anything if a rolled contract is on the board"
    );

    let path = std::env::temp_dir().join(format!("feral_rolled_board_{}.bin", std::process::id()));
    game.save(&path).unwrap();
    let mut loaded = Game::load(&path, &test_assets_dir()).unwrap();
    let _ = std::fs::remove_file(&path);

    assert_eq!(
        board_ids(&mut loaded),
        before,
        "a rolled offer is shown before it is accepted, so it has to survive a \
         reload — which is why nothing about it is drawn from GameRng"
    );
}

#[test]
fn a_rolled_contract_inherits_its_templates_repeatability() {
    let game = fresh();
    let db = game.world.resource::<ContractDb>();
    assert!(
        db.repeatable(&ContractId::from("hunt#drone-6")),
        "hunt is repeatable, so every contract it rolls is"
    );
    assert!(
        !db.repeatable(&ContractId::from("commission#refinery")),
        "commission is not"
    );
    assert!(
        !db.repeatable(&ContractId::from("gone#drone-6")),
        "a template deleted mid-run leaves the run's own copy to finish it, \
         and nothing to put back on a board"
    );
}

#[test]
fn deleting_a_template_does_not_reshuffle_what_the_others_rolled() {
    let mut game = fresh();
    let pools = game.template_pools();
    let seed = 99u64;

    let templates: Vec<_> = game
        .world
        .resource::<ContractDb>()
        .templates()
        .cloned()
        .collect();
    let roll_all = |set: &[crate::contracts::ContractTemplate]| -> Vec<ContractId> {
        set.iter()
            .filter_map(|t| {
                let mut r = StdRng::seed_from_u64(crate::game::contracts::fold(
                    seed,
                    t.id.as_str().as_bytes(),
                ));
                t.roll(&mut r, &pools).map(|def| def.id)
            })
            .collect()
    };

    let all = roll_all(&templates);
    let without_first: Vec<_> = templates[1..].to_vec();
    let survivors: Vec<ContractId> = all
        .iter()
        .filter(|id| !id.as_str().starts_with(templates[0].id.as_str()))
        .cloned()
        .collect();
    assert_eq!(
        roll_all(&without_first),
        survivors,
        "each template rolls from its own salted stream, so adding or removing \
         a template file cannot silently rewrite what the others offered"
    );
}

#[test]
fn a_rolled_delivery_never_asks_for_the_breaching_currency() {
    let mut game = fresh();
    place_home(&mut game, 0, 1);
    let pools = game.template_pools();
    assert!(
        !pools
            .items
            .iter()
            .any(|(id, _)| id.as_str() == crate::items::ids::PORTAL_FRAGMENT),
        "Portal Fragments are the breaching currency and the only source of \
         them is a boss underground — a contract eating a stack's worth is a \
         run that can never breach again. Asserted as an outcome rather than \
         against the filter that currently produces it."
    );
}

#[test]
fn a_rolled_delivery_asks_only_for_bulk_stock() {
    let mut game = fresh();
    place_home(&mut game, 0, 1);
    let pools = game.template_pools();
    assert!(!pools.items.is_empty(), "there is stock to ask for");
    for (id, name) in &pools.items {
        let def = game
            .world
            .resource::<crate::items_db::ItemDb>()
            .get(id.as_str())
            .unwrap();
        assert_eq!(
            def.category(),
            crate::items::ItemCategory::Material,
            "{name} is not something a base hoards, and a Deliver reads plain \
             Inventory — which is by definition the plain-copy store"
        );
        assert!(
            game.item_value(id) <= crate::tuning::CONTRACT_MAX_DELIVER_VALUE,
            "{name} is worth {} — a delivery is asked for by the score, and \
             twenty of anything past the scavenged band is a run's worth of \
             work stated as an errand",
            game.item_value(id)
        );
    }
}

#[test]
fn the_catalogue_covers_the_widest_row_a_template_can_roll() {
    let game = fresh();
    let authored = game.world.resource::<ContractDb>().iter().count();
    let catalogue = game.contract_catalogue();

    assert!(
        catalogue.len() > authored,
        "the renderer's width census measures this, so it has to reach past \
         the authored set — otherwise a template able to roll a longer name \
         than any authored contract stops being covered at all"
    );
    assert!(
        catalogue.iter().any(|row| row.id.as_str().contains('#')),
        "and what it reaches is the rolled rows"
    );
}

#[test]
fn a_contract_the_run_has_already_done_is_never_offered() {
    let mut game = fresh();
    place_home(&mut game, 0, 1);
    deploy_broker(&mut game);

    // Both cases are real: the `contracts` dev-save offered *Stand Up a
    // Refinery* to a base with a Refinery already standing, and *Push the
    // Sector* (reach sector 3) to a run already in sector 3. Either paid out
    // in full for pressing a key.
    let refinery = def(
        "already_built",
        Objective::Build {
            structure: "refinery".to_string(),
        },
        vec![Reward::Credits(45)],
    );
    let breached = def(
        "already_breached",
        Objective::Breach { zone: 1 },
        vec![Reward::Credits(120)],
    );
    assert!(
        game.offerable_contracts_for_test(&refinery),
        "with no Refinery standing it is a real contract"
    );
    assert!(!game.offerable_contracts_for_test(&breached), "zone 1 >= 1");

    deploy(&mut game, "refinery", 3, 0);
    assert!(
        !game.offerable_contracts_for_test(&refinery),
        "a Build of something already deployed completes on acceptance"
    );
}

#[test]
fn no_shipped_contract_or_template_can_be_offered_already_finished() {
    let mut game = fresh();
    place_home(&mut game, 0, 1);
    deploy_broker(&mut game);
    // The state that actually reproduces it, and the one the `contracts`
    // dev-save is in: sector 3 with a Refinery standing, which pre-meets the
    // shipped `push_the_sector` and `stand_up_a_refinery` respectively. In a
    // fresh zone-1 game with an empty base neither can be met, and this
    // census passes against the bug.
    set_zone(&mut game, 3);
    deploy(&mut game, "refinery", 3, 0);

    for _ in 0..12 {
        let offers = board_ids(&mut game);
        for id in offers {
            assert_eq!(game.accept_contract(&id), Ok(()), "{id}");
            // A contract does not settle on acceptance — `contract_system`
            // raises the progress and `settle_contracts` pays, both inside a
            // tick. Without one here the census never reaches the failure it
            // is for, which is how it first passed against the bug.
            game.wait();
            let done = game.world.resource::<ActiveContracts>().done.clone();
            assert!(
                !done.contains(&id),
                "{id} finished on the tick after it was accepted — it was \
                 offered in a state that already met it"
            );
            game.abandon_contract(&id);
        }
        for _ in 0..crate::tuning::CONTRACT_REFRESH_CYCLES {
            game.tick();
        }
    }
}
