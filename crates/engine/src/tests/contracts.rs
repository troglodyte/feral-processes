//! Contracts: the catalogue, the run state, the one system that advances
//! progress, and the board a Contract Broker derives.

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
