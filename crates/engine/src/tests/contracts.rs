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

    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();
    let (structures, _) =
        crate::structures::StructureDb::load_dir(&assets.join("structures")).unwrap();
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
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
    let (items, _) = crate::items_db::ItemDb::load_dir(&assets.join("items"), &abilities).unwrap();

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
    let mut seen = [false; 7];
    for def in contracts.iter() {
        let slot = match &def.objective {
            Objective::Terminate { .. } => 0,
            Objective::Deliver { .. } => 1,
            Objective::Descend { .. } => 2,
            Objective::Breach { .. } => 3,
            Objective::Build { .. } => 4,
            Objective::Hold { .. } => 5,
            Objective::Perform { .. } => 6,
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

/// A run with onboarding already behind it, which is what every test in
/// this file below the chain's own section is about. A new run holds the
/// chain's first mission and has an empty board by design, so a test about
/// the board or about the cap has to start from here.
fn fresh() -> Game {
    let mut game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    skip_tutorial(&mut game);
    game
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
        starter: false,
        tutorial: None,
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
fn a_new_game_holds_nothing_it_did_not_sign_for() {
    // Not `fresh` — this is about what `Game::new` itself produces. A new
    // run holds exactly one thing, the chain's first onboarding mission, and
    // has signed for nothing.
    let game = Game::new(7, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let held = game.world.resource::<ActiveContracts>();
    assert_eq!(held.active.len(), 1);
    assert!(held.active[0].def.tutorial.is_some());
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
    // `fresh` files the onboarding chain as finished, so `done` carries it
    // too — what this is about is the one id the test put there.
    assert_eq!(
        held.done.last(),
        Some(&ContractId::from("already_finished"))
    );
}

#[test]
fn a_save_written_before_contracts_existed_still_loads() {
    // An install with no chain, so both fields serialise to a single `[]`
    // line each and the strip below is a line filter rather than a parser.
    let dir = scratch_assets_dir("legacy_contracts_save");
    copy_shipped_assets(&dir, &[]);
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
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

    let loaded = Game::load(&path, &dir).unwrap();
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
    assert_eq!(held.done.last(), Some(&ContractId::from("paid")));
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
    // `Structure` is the space tag — `view_entities` below refuses to
    // answer for one outside base space, so this fixture's raw spawns have
    // to land there too, not at the player's surface `Position`.
    stand_in_base(&mut game);
    let pos = game.base_pos().unwrap();
    deploy(&mut game, "contract_broker", pos.0 + 1, pos.1);
    deploy(&mut game, "mining_node", pos.0 + 2, pos.1);

    let views = game.view_entities(10, 10);
    let broker = views
        .iter()
        .find(|v| v.issues_contracts)
        .expect("the Broker reports the flag");
    assert_eq!(broker.pos, (pos.0 + 1, pos.1));
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

/// A Broker beside the player, standing on a base the player is also on —
/// which is what a run that has one actually looks like, since
/// A base with a Broker in it, and the party standing in there with it.
///
/// The structures are spawned directly rather than deployed through
/// `place_home`, which spends a `tick()` and five Core Fragments: a tick here
/// would move the shared `GameRng` for every seeded board below it. The
/// pocket is laid by the same call deploying a Home would have made — it is
/// the floor `Game::broker_reach` reads.
///
/// The Home itself still has to stand: a base with a Broker and no Home is
/// not a state the game can reach, and these fixtures have to survive their
/// own save round trip, which
/// `the_same_rolled_contract_comes_back_after_a_save_and_load` needs.
fn deploy_broker(game: &mut Game) {
    game.lay_starting_pocket();
    deploy(game, "home", 0, 0);
    deploy(game, "contract_broker", 1, 0);
    stand_in_base_at(game, 1, 1);
}

/// Out of the base entirely — on the open grid, which is where a party not in
/// base space is, and where no floor can answer for them.
fn stand_off_base(game: &mut Game) {
    game.world.insert_resource(Locale::Surface);
}

/// The far edge of the pocket: in the base, but well outside the arm's-length
/// reach the board used to be reachable from. This is the case the whole rule
/// exists for.
fn stand_across_the_base(game: &mut Game) {
    let edge = crate::tuning::STARTING_POCKET_RADIUS;
    stand_in_base_at(game, edge, 0);
    let broker_gap = edge - 1;
    assert!(
        broker_gap > 2,
        "the far edge has to be further from the Broker than arm's length, or this fixture proves nothing"
    );
}

/// Marks every shipped starter finished. A test about the rest of the board's
/// machinery is otherwise reading an onboarding board — an unfinished starter
/// outranks everything, rolled contracts included.
fn skip_the_starters(game: &mut Game) {
    let ids: Vec<ContractId> = game
        .world
        .resource::<ContractDb>()
        .iter()
        .filter(|def| def.starter)
        .map(|def| def.id.clone())
        .collect();
    game.world
        .resource_mut::<ActiveContracts>()
        .done
        .extend(ids);
}

fn board_ids(game: &mut Game) -> Vec<ContractId> {
    game.contract_board()
        .expect("a Broker is deployed")
        .into_iter()
        .map(|row| row.id)
        .collect()
}

// ---------------------------------------------------------------------------
// Starters
// ---------------------------------------------------------------------------

/// Six contracts, two of them starters, loaded over the shipped set — so
/// these are tests about the queue rather than about which three the roll
/// happened to pick out of the real catalogue.
fn board_with_starters(tag: &str) -> Game {
    let files: Vec<(String, String)> = (0..2)
        .map(|i| {
            (
                format!("s{i}.ron"),
                format!(
                    r#"(id: "s{i}", name: "S{i}", description: "d",
                        objective: Terminate(species: None, count: 3),
                        reward: [Xp(40)], starter: true)"#
                ),
            )
        })
        .chain((0..4).map(|i| {
            (
                format!("n{i}.ron"),
                format!(
                    r#"(id: "n{i}", name: "N{i}", description: "d",
                        objective: Terminate(species: None, count: 3),
                        reward: [Xp(40)])"#
                ),
            )
        }))
        .collect();
    let borrowed: Vec<(&str, &str)> = files
        .iter()
        .map(|(n, b)| (n.as_str(), b.as_str()))
        .collect();
    let (db, warnings) = load(tag, &borrowed);
    assert!(warnings.is_empty(), "{warnings:?}");

    let mut game = fresh();
    game.world.insert_resource(db);
    deploy_broker(&mut game);
    game
}

/// The whole of the onboarding: three slots drawn uniformly out of a pool of
/// fourteen made a new run's first contract a coin flip, and "deliver
/// twenty-five Core Fragments" was as likely to be it as anything.
#[test]
fn an_unfinished_starter_fills_the_board_before_anything_else() {
    let mut game = board_with_starters("starters_first");
    let ids = board_ids(&mut game);
    assert_eq!(ids.len(), 3, "the board fills its slots");
    assert_eq!(
        ids.iter().filter(|id| id.as_str().starts_with('s')).count(),
        2,
        "both starters are on the board, and the third slot falls through to \
         an ordinary contract rather than being left empty: {ids:?}"
    );
}

/// Onboarding is the first sector's business. A starter stays *offerable*
/// past it — nothing about it is unfinishable in zone 4 — but a Broker four
/// sectors out no longer leads with "go and kill three programs".
#[test]
fn a_starter_stops_jumping_the_queue_once_the_run_has_breached() {
    let mut game = board_with_starters("starters_past_zone_one");
    game.world.resource_mut::<ZoneLevel>().0 = 2;

    let ids = board_ids(&mut game);
    assert_eq!(ids.len(), 3);
    assert!(
        ids.iter().any(|id| id.as_str().starts_with('n')),
        "past the first sector the board draws uniformly again, so an \
         ordinary contract reaches it: {ids:?}"
    );
}

#[test]
fn the_board_returns_to_normal_once_the_starters_are_done() {
    let mut game = board_with_starters("starters_done");
    game.world.resource_mut::<ActiveContracts>().done =
        vec![ContractId::from("s0"), ContractId::from("s1")];

    let ids = board_ids(&mut game);
    assert_eq!(ids.len(), 3);
    assert!(
        ids.iter().all(|id| id.as_str().starts_with('n')),
        "a finished starter is out of the pool like any other one-shot: {ids:?}"
    );
}

/// The starters are what a run is handed before it knows what a Broker is
/// for, so each has to be finishable with what a fresh sector holds: no zone
/// gate, and no second helping.
#[test]
fn every_shipped_starter_is_a_one_shot_offered_from_the_first_sector() {
    let (contracts, _) = shipped_contracts();
    let starters: Vec<_> = contracts.iter().filter(|d| d.starter).collect();
    assert!(
        starters.len() >= 5,
        "the shipped set carries an onboarding arc, not one lonely job"
    );
    for def in starters {
        assert_eq!(
            def.min_zone, 0,
            "{} is a starter behind a zone gate, so a new run never sees it",
            def.id
        );
        assert!(
            !def.repeatable,
            "{} is a repeatable starter, so it holds a board slot for the \
             whole run rather than being the thing you did once",
            def.id
        );
    }
}

#[test]
fn a_new_runs_first_board_is_nothing_but_starters() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let ids = board_ids(&mut game);
    assert_eq!(ids.len(), 3);
    for id in &ids {
        let def = game
            .world
            .resource::<crate::contracts::ContractDb>()
            .get(id)
            .unwrap_or_else(|| panic!("{id} is rolled, so a template outranked a starter"));
        assert!(def.starter, "{id} is not a starter");
    }
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
    place_home(&mut game);
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
    skip_the_starters(&mut game);

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
    skip_the_starters(&mut game);
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
    place_home(&mut game);
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
    place_home(&mut game);
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
    place_home(&mut game);
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
    place_home(&mut game);
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

// ---------------------------------------------------------------------------
// Where the board may be read, and where it may be acted on
// ---------------------------------------------------------------------------

/// The board is a bulletin, not a conversation: it is derived from the world
/// seed, the sector and the epoch and makes no claim about where the party
/// is, so there is nothing for standing somewhere else to invalidate.
#[test]
fn the_board_is_readable_from_off_the_base() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let at_the_desk = board_ids(&mut game);
    assert!(!at_the_desk.is_empty(), "a zone-1 board fills its slots");

    stand_off_base(&mut game);
    assert_eq!(
        board_ids(&mut game),
        at_the_desk,
        "the offers are the sector's, not the tile's — walking away from the \
         base must not change what is on the board"
    );
}

/// The same reasoning underground, where it replaces the opposite rule: the
/// board used to be `None` down there because reach was measured from a
/// `Position` pinned to the surface entrance tile. Nothing measures from the
/// player's tile any more, so the reason is gone and the answer changes with
/// it. Taking one is still refused, which is what that guard was protecting.
#[test]
fn the_board_is_readable_underground_and_nothing_can_be_taken_off_it() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let on_the_surface = board_ids(&mut game);
    let id = on_the_surface
        .first()
        .cloned()
        .expect("a zone-1 board fills its slots");

    set_depth(&mut game, 2);
    assert_eq!(
        board_ids(&mut game),
        on_the_surface,
        "four frames down the sector is still offering what it was offering"
    );
    assert_eq!(
        game.accept_contract(&id),
        Err(ContractRefusal::NotAtBroker),
        "reading the board and standing at it are different questions"
    );
    assert!(
        game.world.resource::<ActiveContracts>().active.is_empty(),
        "a refused acceptance leaves the run exactly as it found it"
    );
}

/// The feature: the Broker's own tile stops mattering once you are home.
#[test]
fn a_contract_is_taken_from_anywhere_on_the_base() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let id = first_offer(&mut game);

    stand_across_the_base(&mut game);
    assert_eq!(
        game.accept_contract(&id),
        Ok(()),
        "standing on your own slab is standing at your Broker"
    );
    assert_eq!(game.world.resource::<ActiveContracts>().active.len(), 1);
}

#[test]
fn taking_a_contract_off_the_base_is_refused_and_writes_nothing() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let id = first_offer(&mut game);

    stand_off_base(&mut game);
    assert_eq!(
        game.accept_contract(&id),
        Err(ContractRefusal::NotAtBroker),
        "a contract on the board a sector away is not a contract in hand"
    );
    assert!(game.world.resource::<ActiveContracts>().active.is_empty());
}

/// Distinct from `NotAtBroker` on purpose: the two leave the player different
/// errands, which is `NoPost::BoxedIn`'s reason for existing beside
/// `NoPost::NoRoute`. One says go home, the other says nobody is offering it.
#[test]
fn a_run_with_no_broker_is_refused_differently_from_one_away_from_its_broker() {
    let mut game = fresh();
    let id = ContractId::from("anything");
    assert_eq!(
        game.accept_contract(&id),
        Err(ContractRefusal::NotOffered),
        "with nothing built, there is no board to be away from"
    );
    assert!(game.contract_board().is_none());
}

#[test]
fn handing_over_cargo_off_the_base_keeps_the_cargo() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let before = carried(&game, "core_fragment");
    assert!(before >= 3, "the fixture needs stock, has {before}");
    let id = deliver_fixture(&mut game, 3);

    stand_off_base(&mut game);
    assert_eq!(
        game.deliver_to_contract(&id),
        Err(ContractRefusal::NotAtBroker)
    );
    assert_eq!(
        carried(&game, "core_fragment"),
        before,
        "every refusal lands before anything leaves cargo"
    );
    assert_eq!(progress_of(&game, "quota"), 0);
}

#[test]
fn cargo_is_handed_over_from_anywhere_on_the_base() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let id = deliver_fixture(&mut game, 3);

    stand_across_the_base(&mut game);
    assert_eq!(game.deliver_to_contract(&id), Ok(3));
}

/// Giving one back is deliberately not gated. A contract abandoned in the
/// field is abandoned — walking home to resign is errand, not decision.
#[test]
fn a_contract_is_given_back_from_anywhere() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let id = first_offer(&mut game);
    game.accept_contract(&id).unwrap();

    stand_off_base(&mut game);
    assert!(game.abandon_contract(&id));
    assert!(game.world.resource::<ActiveContracts>().active.is_empty());
}

/// Three states, and the middle one is the whole point: a run can have a
/// Broker without the player being at it.
#[test]
fn broker_reach_reports_the_three_states() {
    let mut game = fresh();
    assert_eq!(game.broker_reach(), BrokerReach::NoBroker);

    deploy_broker(&mut game);
    assert_eq!(game.broker_reach(), BrokerReach::AtBroker);

    stand_across_the_base(&mut game);
    assert_eq!(
        game.broker_reach(),
        BrokerReach::AtBroker,
        "the far edge of the pocket is still the base"
    );

    stand_off_base(&mut game);
    assert_eq!(game.broker_reach(), BrokerReach::OffBase);
}

/// The base's floor is laid, not derived from a radius, so the rule has to
/// follow the floor — freezing the desk at `STARTING_POCKET_RADIUS` where the
/// question is about a base that *exists* is the mistake this guards. Slice 2
/// lets the player lay floor for a price; this lays it directly, because what
/// is under test is the desk following it and not what it costs.
#[test]
fn a_grown_base_carries_the_desk_out_to_its_new_edge() {
    let mut game = fresh();
    deploy_broker(&mut game);
    let grown = (crate::tuning::STARTING_POCKET_RADIUS + 3, 0);
    stand_in_base_at(&mut game, grown.0, grown.1);
    assert_eq!(
        game.broker_reach(),
        BrokerReach::OffBase,
        "that cell is unmined rock, well past the pocket the Home laid"
    );

    game.world
        .resource_mut::<crate::base_grid::BaseGrid>()
        .lay_floor(grown.0, grown.1);
    assert_eq!(
        game.broker_reach(),
        BrokerReach::AtBroker,
        "the same cell, once it is floor"
    );
}

/// The chain is every def carrying a step, in step order — not file order,
/// not id order. Written with the files deliberately out of order so a
/// `read_dir` that happened to return them sorted cannot pass this by luck.
#[test]
fn the_tutorial_chain_is_every_stepped_contract_in_step_order() {
    let (db, warnings) = load(
        "tutorial_chain_order",
        &[
            (
                "z_third.ron",
                r#"(id: "third", name: "Third", description: "d",
                    objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(30))"#,
            ),
            (
                "a_first.ron",
                r#"(id: "first", name: "First", description: "d",
                    objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#,
            ),
            (
                "m_plain.ron",
                r#"(id: "plain", name: "Plain", description: "d",
                    objective: Breach(zone: 2), reward: [Xp(1)])"#,
            ),
            (
                "b_second.ron",
                r#"(id: "second", name: "Second", description: "d",
                    objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(20))"#,
            ),
        ],
    );
    assert!(warnings.is_empty(), "all four are valid: {warnings:?}");
    let chain: Vec<&str> = db.tutorial_chain().iter().map(|d| d.id.as_str()).collect();
    assert_eq!(
        chain,
        vec!["first", "second", "third"],
        "the chain is step order, and a contract with no step is not in it"
    );
}

/// A directory with no stepped contract has no chain, which is the
/// pre-tutorial game and a supported install.
#[test]
fn a_directory_with_no_stepped_contract_has_no_chain() {
    let (db, _) = load(
        "tutorial_chain_empty",
        &[(
            "plain.ron",
            r#"(id: "plain", name: "Plain", description: "d",
                objective: Breach(zone: 2), reward: [Xp(1)])"#,
        )],
    );
    assert!(db.tutorial_chain().is_empty());
}

/// Two files claiming one step would run the chain in an order nobody
/// authored, and which of them won would depend on `read_dir`. The second is
/// skipped with a warning, exactly as a duplicate id is.
#[test]
fn a_duplicate_tutorial_step_is_refused() {
    let (db, warnings) = load(
        "tutorial_dup_step",
        &[
            (
                "a.ron",
                r#"(id: "a", name: "A", description: "d",
                    objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#,
            ),
            (
                "b.ron",
                r#"(id: "b", name: "B", description: "d",
                    objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#,
            ),
        ],
    );
    assert_eq!(db.tutorial_chain().len(), 1, "one of the two is kept");
    assert_eq!(
        warnings.len(),
        1,
        "and the other is warned about: {warnings:?}"
    );
    assert!(
        warnings[0].contains("step"),
        "the warning names what collided: {warnings:?}"
    );
}

/// `load_dir` sorts its entries, so two files claiming one step resolve the
/// same way every run. Without the sort the survivor above is whichever
/// `read_dir` happened to yield first, and the shipped chain would differ
/// between machines.
#[test]
fn a_duplicate_tutorial_step_resolves_the_same_way_every_run() {
    for i in 0..4 {
        let (db, _) = load(
            &format!("tutorial_dup_stable_{i}"),
            &[
                (
                    "zzz.ron",
                    r#"(id: "zzz", name: "Z", description: "d",
                        objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#,
                ),
                (
                    "aaa.ron",
                    r#"(id: "aaa", name: "A", description: "d",
                        objective: Breach(zone: 2), reward: [Xp(1)], tutorial: Some(10))"#,
                ),
            ],
        );
        assert_eq!(
            db.tutorial_chain()[0].id.as_str(),
            "aaa",
            "the file that sorts first is the one that loads"
        );
    }
}

/// A tutorial mission is never offered, so a `starter` flag on one is a
/// claim about a board slot it can never occupy.
#[test]
fn a_tutorial_mission_may_not_also_be_a_starter() {
    let (db, warnings) = load(
        "tutorial_and_starter",
        &[(
            "a.ron",
            r#"(id: "a", name: "A", description: "d", objective: Breach(zone: 2),
                reward: [Xp(1)], tutorial: Some(10), starter: true)"#,
        )],
    );
    assert_eq!(db.iter().count(), 0);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("starter"), "{warnings:?}");
}

/// The chain's position is derived from `done`, so a repeatable mission
/// would leave and re-enter it forever.
#[test]
fn a_tutorial_mission_may_not_be_repeatable() {
    let (db, warnings) = load(
        "tutorial_and_repeatable",
        &[(
            "a.ron",
            r#"(id: "a", name: "A", description: "d", objective: Breach(zone: 2),
                reward: [Xp(1)], tutorial: Some(10), repeatable: true)"#,
        )],
    );
    assert_eq!(db.iter().count(), 0);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("repeatable"), "{warnings:?}");
}

/// `Hold` is met by what is in the pack, needs no Broker, and is what lets
/// the chain teach "fighting pays in stock" before one is standing.
#[test]
fn hold_is_met_by_what_the_player_is_carrying() {
    let objective = Objective::Hold {
        item: ItemId::from(crate::items::ids::CORE_FRAGMENT),
        count: 12,
    };
    let mut state = crate::contracts::ObjectiveState {
        depth: 0,
        zone: 1,
        standing: Vec::new(),
        carried: vec![(ItemId::from(crate::items::ids::CORE_FRAGMENT), 11)],
    };
    assert!(!objective.already_met(&state), "eleven is not twelve");
    state.carried[0].1 = 12;
    assert!(objective.already_met(&state));
    state.carried[0].1 = 40;
    assert!(
        objective.already_met(&state),
        "more than asked still counts"
    );
}

/// State-shaped, so it completes through the one `progress >= target` rule
/// with a target of 1 — the same shape `Build` and `Descend` have.
#[test]
fn hold_is_a_latch_with_a_target_of_one() {
    let objective = Objective::Hold {
        item: ItemId::from(crate::items::ids::CORE_FRAGMENT),
        count: 12,
    };
    assert_eq!(objective.target(), 1);
}

/// Carrying nothing of the item at all is the common case and must not
/// panic or read as met.
#[test]
fn hold_is_not_met_by_an_empty_pack() {
    let objective = Objective::Hold {
        item: ItemId::from(crate::items::ids::CORE_FRAGMENT),
        count: 1,
    };
    let state = crate::contracts::ObjectiveState {
        depth: 0,
        zone: 1,
        standing: Vec::new(),
        carried: Vec::new(),
    };
    assert!(!objective.already_met(&state));
}

/// A held `Hold` finishes off the run's own inventory, through the ordinary
/// tick path and the ordinary completion path.
#[test]
fn a_held_hold_completes_from_the_players_pack() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    let item = ItemId::from(crate::items::ids::CORE_FRAGMENT);
    let player = game.player_entity();
    game.world
        .get_mut::<crate::components::Inventory>(player)
        .unwrap()
        .add(item.clone(), 100);
    give(
        &mut game,
        def(
            "hold_test",
            Objective::Hold { item, count: 12 },
            vec![Reward::Xp(1)],
        ),
        0,
    );
    game.tick();
    assert!(
        game.world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&ContractId::from("hold_test")),
        "a pack that already meets the objective finishes it on the next tick"
    );
}

/// A deed recorded this tick finishes a held `Perform`, through the same
/// system and the same completion path a kill goes through.
#[test]
fn a_deed_finishes_a_held_perform_contract() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    give(
        &mut game,
        def(
            "perform_test",
            Objective::Perform {
                deed: crate::contracts::Deed::Examined,
            },
            vec![Reward::Xp(1)],
        ),
        0,
    );
    game.tick();
    assert!(
        !game
            .world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&ContractId::from("perform_test")),
        "nothing has been done yet"
    );
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    assert!(
        game.world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&ContractId::from("perform_test")),
    );
}

/// A deed of the wrong kind advances nothing. Without this the six deeds
/// would be one deed with six names.
#[test]
fn a_deed_of_another_kind_advances_nothing() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    give(
        &mut game,
        def(
            "perform_test",
            Objective::Perform {
                deed: crate::contracts::Deed::PostedStaff,
            },
            vec![Reward::Xp(1)],
        ),
        0,
    );
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    assert!(
        !game
            .world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&ContractId::from("perform_test")),
        "examining is not posting staff"
    );
}

/// The queue is drained every tick by `contract_system` and by nothing else.
/// A deed left in it would finish a contract accepted long afterwards.
#[test]
fn a_deed_does_not_survive_the_tick_that_drained_it() {
    let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    assert!(
        game.world
            .resource::<crate::resources::RunFeats>()
            .deeds
            .is_empty(),
        "the queue is drained unconditionally"
    );
}

/// Every emit site, one test each. They assert on the queue rather than on a
/// finished contract so a failure names the site that stopped writing rather
/// than reading as the contract system being broken.
///
/// `Deed::Tamed` is not here: it is tested in `tests/taming.rs` beside the
/// forced first decompile, because the two are one behaviour from the
/// player's side.
mod deed_sites {
    use super::*;
    use crate::contracts::Deed;
    use crate::game::base::work_orders::WorkOrder;

    fn deeds(game: &Game) -> Vec<Deed> {
        game.world
            .resource::<crate::resources::RunFeats>()
            .deeds
            .clone()
    }

    fn stocked(game: &mut Game, kind: &str, x: i32, y: i32, output: &[(&str, u32)]) -> Entity {
        let machine = spawn_machine_at(game, kind, x, y);
        let mut stock = game.world.get_mut::<Stock>(machine).unwrap();
        for (id, n) in output {
            stock.output.insert(ItemId::from(*id), *n);
        }
        machine
    }

    fn player_tile(game: &Game) -> Position {
        *game.world.get::<Position>(game.player_entity()).unwrap()
    }

    #[test]
    fn examining_something_writes_a_deed() {
        let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let start = player_tile(&game);
        clear_creatures_east_of_player(&mut game, start, 10);
        let species = game.species_defs().into_iter().next().unwrap();
        game.world.spawn((
            Creature {
                species: species.id.clone(),
            },
            Position {
                x: start.x + 3,
                y: start.y,
            },
            Stats {
                hp: 1,
                max_hp: 1,
                atk: 1,
                mitigation: 1,
            },
        ));
        assert!(
            game.find_target_in_direction(1, 0, 5).is_some(),
            "the fixture has to put something there"
        );
        assert!(deeds(&game).contains(&Deed::Examined));
    }

    #[test]
    fn examining_nothing_writes_no_deed() {
        let mut game = Game::new(31, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let start = player_tile(&game);
        clear_creatures_along_ray(&mut game, start, 0, -1, 10);
        // An empty ray. The mission is to teach that `x` reports something,
        // so pointing it at blank ground must not complete it.
        assert!(game.find_target_in_direction(0, -1, 1).is_none());
        assert!(!deeds(&game).contains(&Deed::Examined));
    }

    /// `transfer_items` ticks, and the tick is what drains the queue — so
    /// these two read the contract rather than `RunFeats`. The deed is
    /// written *before* that tick deliberately, so the mission advances on
    /// the action rather than a tick behind it.
    fn holding_a_take_mission(seed: u32) -> Game {
        let mut game = Game::new(seed, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        give(
            &mut game,
            def(
                "take_test",
                Objective::Perform {
                    deed: Deed::TookFromContainer,
                },
                vec![Reward::Xp(1)],
            ),
            0,
        );
        game
    }

    fn finished_take_mission(game: &Game) -> bool {
        game.world
            .resource::<crate::resources::ActiveContracts>()
            .done
            .contains(&ContractId::from("take_test"))
    }

    /// Taking teaches pulling stock *out* of a machine, so only the take
    /// side writes.
    #[test]
    fn taking_from_a_container_writes_a_deed() {
        let mut game = holding_a_take_mission(22);
        // Base-space coordinates: `stand_in_base` puts the party at the
        // base's own origin, and `Position` is the surface tile.
        stand_in_base(&mut game);
        stocked(&mut game, "mining_node", 1, 0, &[(ids::CORE_FRAGMENT, 10)]);
        let (taken, _) = game.transfer_items(&[(ItemId::from(ids::CORE_FRAGMENT), 4)], &[]);
        assert!(!taken.is_empty(), "the fixture has to move something");
        assert!(finished_take_mission(&game));
    }

    /// The negative half, and the one that catches a `note_deed` written
    /// unconditionally at the top of `transfer_items`: a player who only put
    /// something in has not done what the mission asks.
    #[test]
    fn only_putting_into_a_container_writes_no_deed() {
        let mut game = holding_a_take_mission(23);
        stand_in_base(&mut game);
        stocked(&mut game, "depot", 1, 0, &[]);
        set_inventory(&mut game, &[(ids::CORE_FRAGMENT, 10)]);
        let (_, given) = game.transfer_items(&[], &[(ItemId::from(ids::CORE_FRAGMENT), 4)]);
        assert!(!given.is_empty(), "the fixture has to move something");
        assert!(!finished_take_mission(&game));
    }

    /// The mission asks for a *standing* order, which is the thing that
    /// keeps working without being asked again.
    #[test]
    fn a_standing_work_order_writes_a_deed() {
        let mut game = Game::new(24, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        stand_in_base(&mut game);
        spawn_machine_at(&mut game, "mining_node", 1, 0);
        game.queue_work_order(WorkOrder::level(ItemId::from(ids::CORE_FRAGMENT), 20))
            .unwrap();
        assert!(deeds(&game).contains(&Deed::QueuedStandingOrder));
    }

    /// And a one-off is not one. Without this the mission completes on the
    /// first order of any kind and the lesson never lands.
    #[test]
    fn a_one_off_work_order_writes_no_deed() {
        let mut game = Game::new(25, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        stand_in_base(&mut game);
        spawn_machine_at(&mut game, "mining_node", 1, 0);
        game.queue_work_order(WorkOrder::batch(ItemId::from(ids::CORE_FRAGMENT), 20))
            .unwrap();
        assert!(!deeds(&game).contains(&Deed::QueuedStandingOrder));
    }

    #[test]
    fn unlocking_a_perk_writes_a_deed() {
        let mut game = Game::new(26, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 20;
        let perk = game.perk_defs().first().expect("a shipped perk").id;
        game.unlock_perk(perk).unwrap();
        assert!(deeds(&game).contains(&Deed::UnlockedPerk));
    }

    /// A refusal spends nothing and must record nothing.
    #[test]
    fn a_refused_perk_writes_no_deed() {
        let mut game = Game::new(27, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        let player = game.player_entity();
        game.world.get_mut::<Perks>(player).unwrap().points = 0;
        let perk = game.perk_defs().first().expect("a shipped perk").id;
        assert!(game.unlock_perk(perk).is_err());
        assert!(!deeds(&game).contains(&Deed::UnlockedPerk));
    }

    /// `post_worker` directly rather than the `post_program` fixture, which
    /// ticks afterwards — and a tick is what drains the queue this asserts
    /// on.
    #[test]
    fn posting_a_worker_writes_a_deed() {
        let mut game = Game::new(28, DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
        stand_in_base(&mut game);
        let machine = spawn_machine_at(&mut game, "mining_node", 2, 0);
        let worker = spawn_tamed(&mut game, 10, 3);
        game.post_worker(worker, machine);
        assert!(deeds(&game).contains(&Deed::PostedStaff));
    }
}

// ---------------------------------------------------------------------------
// The onboarding chain
// ---------------------------------------------------------------------------

/// A new run has the chain's first mission in hand before anything is
/// ticked and with no Broker anywhere — which is the whole reason it is
/// handed out rather than offered.
#[test]
fn a_new_run_holds_the_first_mission_with_no_broker() {
    let dir = assets_with_fixture_chain("chain_first");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    let held: Vec<String> = game
        .active_contracts()
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    assert!(
        held.contains(&"fixture_step_1".to_string()),
        "the chain's first step is in hand at tick 0: {held:?}"
    );
    assert_eq!(
        game.broker_reach(),
        BrokerReach::NoBroker,
        "and no Broker is standing"
    );
}

/// Exactly one, never two. The property the whole feature rests on.
#[test]
fn exactly_one_mission_is_held_at_a_time() {
    let dir = assets_with_fixture_chain("chain_one");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    for _ in 0..3 {
        let count = game
            .active_contracts()
            .iter()
            .filter(|r| r.tutorial)
            .count();
        assert_eq!(count, 1, "one onboarding mission is live at a time");
        game.note_deed(crate::contracts::Deed::Examined);
        game.tick();
    }
}

/// Finishing one hands out the next in the same tick, so the player never
/// sees an empty slot.
#[test]
fn finishing_a_mission_hands_out_the_next_one() {
    let dir = assets_with_fixture_chain("chain_next");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    let held: Vec<String> = game
        .active_contracts()
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    assert!(held.contains(&"fixture_step_2".to_string()), "{held:?}");
    assert!(!held.contains(&"fixture_step_1".to_string()), "{held:?}");
}

/// When the last one is finished the chain is over and nothing is handed
/// out again.
#[test]
fn a_finished_chain_hands_out_nothing() {
    let dir = assets_with_fixture_chain("chain_end");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    for _ in 0..3 {
        game.note_deed(crate::contracts::Deed::Examined);
        game.tick();
    }
    assert!(!game.in_tutorial(), "three steps, three deeds, chain over");
    game.tick();
    assert_eq!(
        game.active_contracts()
            .iter()
            .filter(|r| r.tutorial)
            .count(),
        0
    );
}

/// The chain cannot be given back. An unbreakable chain with a give-back key
/// is not a chain.
#[test]
fn an_onboarding_mission_cannot_be_abandoned() {
    let dir = assets_with_fixture_chain("chain_abandon");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    assert!(!game.abandon_contract(&ContractId::from("fixture_step_1")));
    assert_eq!(
        game.active_contracts()
            .iter()
            .filter(|r| r.tutorial)
            .count(),
        1,
        "it is still in hand"
    );
}

/// The row says it is one, which is what the renderer colours on and what
/// app-core refuses on.
#[test]
fn an_onboarding_missions_row_is_flagged() {
    let dir = assets_with_fixture_chain("chain_flag");
    let game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    let row = game
        .active_contracts()
        .into_iter()
        .find(|r| r.id.as_str() == "fixture_step_1")
        .expect("held");
    assert!(row.tutorial);
}

/// An install with no chain is the pre-tutorial game exactly: nothing is
/// handed out and nothing is flagged.
#[test]
fn an_install_with_no_chain_hands_out_nothing() {
    // The shipped contracts with the eleven missions deleted — the claim is
    // that removing the chain gives the pre-chain game back, ordinary
    // contracts and open board included, and a directory with no contracts
    // at all could not say that.
    let dir = scratch_assets_dir("chain_absent");
    copy_shipped_assets(&dir, &[]);
    let contracts = dir.join("contracts");
    std::fs::create_dir_all(&contracts).unwrap();
    let shipped = test_assets_dir().join("contracts");
    let mut copied = 0;
    for entry in std::fs::read_dir(&shipped).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();
        if path.is_dir() || name.starts_with("tutorial_") {
            continue;
        }
        std::fs::copy(&path, contracts.join(&name)).unwrap();
        copied += 1;
    }
    assert!(
        copied > 0,
        "the install has to have ordinary contracts in it"
    );

    let mut game = Game::new(31, DifficultyMode::Forgiving, &dir).unwrap();
    assert!(!game.in_tutorial());
    assert_eq!(
        game.active_contracts()
            .iter()
            .filter(|r| r.tutorial)
            .count(),
        0
    );
    deploy_broker(&mut game);
    assert!(
        !game
            .contract_board()
            .expect("a Broker is standing")
            .is_empty(),
        "and the ordinary board is open, which is the pre-chain game"
    );
}

/// While the chain runs the board is empty — one mission at a time means
/// one, not one plus three the player cannot evaluate yet.
#[test]
fn the_board_is_empty_while_the_chain_runs() {
    let dir = assets_with_fixture_chain("board_suppressed");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    deploy_broker(&mut game);
    assert!(game.in_tutorial());
    assert_eq!(
        game.contract_board(),
        Some(Vec::new()),
        "a Broker is standing, so the board exists and is empty — not `None`, \
         which is the claim that no Broker is standing at all"
    );
}

/// And fills the moment the chain is finished.
#[test]
fn the_board_fills_when_the_chain_is_finished() {
    let dir = assets_with_fixture_chain("board_freed");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    deploy_broker(&mut game);
    skip_tutorial(&mut game);
    let board = game.contract_board().expect("a Broker is standing");
    assert!(!board.is_empty(), "the ordinary board is live again");
}

/// With no Broker the answer is still `None` and not an empty board. Two
/// readers depend on that difference.
#[test]
fn no_broker_still_answers_none_during_the_chain() {
    let dir = assets_with_fixture_chain("board_no_broker");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    assert_eq!(game.contract_board(), None);
}

fn save_path(tag: &str) -> std::path::PathBuf {
    std::env::temp_dir().join(format!(
        "feral_processes_contracts_{tag}_{}.bin",
        std::process::id()
    ))
}

/// Rewrites `path` as a build without `field` would have written it. The
/// save is field-named RON — which is the whole reason this is legal, and
/// a positional format would make this test impossible to write.
fn strip_field_from_save(path: &std::path::Path, field: &str) {
    let text = std::fs::read_to_string(path).unwrap();
    let key = format!("{field}:");
    assert!(
        text.contains(&key),
        "the current build has to be writing {field}, or this test proves nothing"
    );
    let older: String = text
        .lines()
        .filter(|line| !line.trim_start().starts_with(&key))
        .collect::<Vec<_>>()
        .join("\n");
    assert!(
        !older.contains(&key),
        "the key has to actually be gone for this to prove anything"
    );
    std::fs::write(path, older).unwrap();
}

/// A save written before this feature existed carries no flag, and a run
/// forty hours old must not be told to build a Home it built long ago. The
/// whole chain is filed as finished at load.
#[test]
fn a_save_from_before_the_chain_never_starts_it() {
    // Written by an install with no chain at all and loaded against one that
    // has it, which is exactly what shipping this feature does to a run in
    // progress. Building the fixture this way rather than deleting the field
    // out of a chain-aware save is what makes it faithful: a pre-chain save
    // cannot have a mission in hand either.
    let before = scratch_assets_dir("seed_old_save_before");
    copy_shipped_assets(&before, &[]);
    let mut game = Game::new(7, DifficultyMode::Forgiving, &before).unwrap();
    let path = save_path("old_save");
    game.save(&path).unwrap();
    strip_field_from_save(&path, "tutorial_seeded");

    let dir = assets_with_fixture_chain("seed_old_save_after");
    let loaded = Game::load(&path, &dir).unwrap();
    let _ = std::fs::remove_file(&path);
    assert!(!loaded.in_tutorial(), "the chain is filed as finished");
    assert_eq!(
        loaded
            .active_contracts()
            .iter()
            .filter(|r| r.tutorial)
            .count(),
        0,
        "and nothing is in hand"
    );
}

/// A save written *by* this build carries the flag and resumes the chain
/// exactly where it was — the position is derived from `done`, so there is
/// nothing else to restore.
#[test]
fn a_run_saved_mid_chain_resumes_on_the_same_step() {
    let dir = assets_with_fixture_chain("seed_mid_chain");
    let mut game = Game::new(7, DifficultyMode::Forgiving, &dir).unwrap();
    game.note_deed(crate::contracts::Deed::Examined);
    game.tick();
    let path = save_path("mid_chain");
    game.save(&path).unwrap();

    let loaded = Game::load(&path, &dir).unwrap();
    let _ = std::fs::remove_file(&path);
    let held: Vec<String> = loaded
        .active_contracts()
        .iter()
        .map(|r| r.id.to_string())
        .collect();
    assert!(held.contains(&"fixture_step_2".to_string()), "{held:?}");
    assert!(loaded.in_tutorial());
}
