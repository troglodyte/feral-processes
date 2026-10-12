use super::support::*;
use crate::behaviours::BehaviourDb;
use crate::*;

const DEFAULT: &str = r#"(id: "default", name: "Default", description: "d")"#;

/// A scratch `behaviours/` directory holding `files`.
fn dir_with(tag: &str, files: &[(&str, &str)]) -> ScratchAssets {
    let dir = scratch_assets_dir(tag);
    std::fs::create_dir_all(&*dir).unwrap();
    for (name, body) in files {
        std::fs::write(dir.join(name), body).unwrap();
    }
    dir
}

#[test]
fn a_malformed_profile_is_skipped_with_a_warning() {
    let dir = dir_with(
        "behaviours_malformed",
        &[("default.ron", DEFAULT), ("broken.ron", "(id: ")],
    );
    let (db, warnings) = BehaviourDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&*dir);
    assert_eq!(db.ids().collect::<Vec<_>>(), ["default"]);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("broken.ron"), "{warnings:?}");
}

#[test]
fn a_feature_both_pinned_and_free_warns_and_pinned_wins() {
    let both = r#"(id: "both", name: "B", description: "d",
        pinned: {"target_distance": 1.5}, weights: {"target_distance": -9.0, "would_kill": 2.0})"#;
    let dir = dir_with(
        "behaviours_both",
        &[("default.ron", DEFAULT), ("both.ron", both)],
    );
    let (db, warnings) = BehaviourDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&*dir);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    let def = db.get("both").unwrap();
    assert_eq!(def.pinned["target_distance"], 1.5);
    assert!(!def.weights.contains_key("target_distance"));
    assert_eq!(def.weights["would_kill"], 2.0);
}

#[test]
fn an_unknown_feature_warns_and_is_ignored() {
    let odd = r#"(id: "odd", name: "O", description: "d",
        weights: {"would_kill": 2.0, "moon_phase": 9.0})"#;
    let dir = dir_with(
        "behaviours_unknown_feature",
        &[("default.ron", DEFAULT), ("odd.ron", odd)],
    );
    let (db, warnings) = BehaviourDb::load_dir(&dir).unwrap();
    let _ = std::fs::remove_dir_all(&*dir);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("moon_phase"), "{warnings:?}");
    let w = crate::tactical::policy::TacticalWeights::of(db.get("odd").unwrap());
    let kill = crate::tactical::policy::TacticalFeature::WouldKill as usize;
    let mut f = [0.0; crate::tactical::policy::TacticalFeature::COUNT];
    f[kill] = 1.0;
    assert_eq!(crate::tactical::policy::score(&w, &f), 2.0);
}

#[test]
fn a_missing_default_profile_is_a_load_error() {
    let other = r#"(id: "brute", name: "B", description: "d")"#;
    let dir = dir_with("behaviours_no_default", &[("brute.ron", other)]);
    let result = BehaviourDb::load_dir(&dir);
    let _ = std::fs::remove_dir_all(&*dir);
    assert!(result.is_err());
}

fn crawler_naming(behaviour: Option<&str>, tag: &str) -> (ScratchAssets, String) {
    let shipped = std::fs::read_to_string(test_assets_dir().join("species/crawler.ron")).unwrap();
    let field = behaviour
        .map(|b| format!("behaviour: Some({b:?}),"))
        .unwrap_or_default();
    let body = shipped.replacen("id: \"crawler\",", &format!("id: \"crawler\", {field}"), 1);
    let dir = modded_assets_dir(tag, &[], &[], &[("crawler.ron", &body)], &[], &[]);
    (dir, body)
}

#[test]
fn a_species_naming_no_profile_plays_as_default_without_a_warning() {
    let (dir, _) = crawler_naming(None, "behaviour_none");
    let mut game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&*dir);
    let body = spawn_wild_without_routine(&mut game, "crawler", 4, 4);
    assert_eq!(game.behaviour_of(body).id, "default");
}

#[test]
fn a_species_naming_an_unknown_profile_plays_as_default_with_one_warning() {
    let (dir, _) = crawler_naming(Some("nonesuch"), "behaviour_unknown");
    let (db, _) = BehaviourDb::load_dir(&test_assets_dir().join("behaviours")).unwrap();
    let (abilities, _) = crate::abilities::AbilityDb::load_dir(&dir.join("abilities")).unwrap();
    let (species, _) = SpeciesDb::load_dir(&dir.join("species"), &abilities).unwrap();
    let warnings = db.unknown_species_references(&species);
    assert_eq!(warnings.len(), 1, "{warnings:?}");
    assert!(warnings[0].contains("nonesuch"));

    let mut game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&*dir);
    let body = spawn_wild_without_routine(&mut game, "crawler", 4, 4);
    assert_eq!(game.behaviour_of(body).id, "default");
}

#[test]
fn a_species_naming_a_known_profile_gets_it_and_an_override_beats_it() {
    let brute = r#"(id: "brute", name: "B", description: "d")"#;
    let (dir, _) = crawler_naming(Some("brute"), "behaviour_known");
    std::fs::create_dir_all(dir.join("behaviours")).unwrap();
    std::fs::write(dir.join("behaviours/brute.ron"), brute).unwrap();
    let mut game = Game::new(1, DifficultyMode::Forgiving, &dir).unwrap();
    let _ = std::fs::remove_dir_all(&*dir);
    let body = spawn_wild_without_routine(&mut game, "crawler", 4, 4);
    assert_eq!(game.behaviour_of(body).id, "brute");
    game.world
        .resource_mut::<crate::resources::Behaviours>()
        .0
        .set_override(Some("default".into()));
    assert_eq!(game.behaviour_of(body).id, "default");
}
