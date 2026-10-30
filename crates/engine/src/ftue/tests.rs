use super::*;

fn set(keys: &[&str]) -> BTreeSet<String> {
    keys.iter().map(|k| k.to_string()).collect()
}

fn guidance(enabled: bool, never: &[&str]) -> FtueGuidance {
    FtueGuidance {
        enabled,
        never: never.iter().map(|k| k.to_string()).collect(),
    }
}

#[test]
fn a_fresh_key_with_guidance_on_shows() {
    assert_eq!(
        decide("event:first_raid", &set(&[]), &guidance(true, &[])),
        FtueDecision::Show
    );
}

#[test]
fn guidance_off_latches_without_showing() {
    assert_eq!(
        decide("event:first_raid", &set(&[]), &guidance(false, &[])),
        FtueDecision::LatchOnly
    );
}

#[test]
fn a_never_key_latches_without_showing() {
    let g = guidance(true, &["event:first_raid"]);
    assert_eq!(
        decide("event:first_raid", &set(&[]), &g),
        FtueDecision::LatchOnly
    );
    assert_eq!(
        decide("event:first_descent", &set(&[]), &g),
        FtueDecision::Show,
        "only the named message is opted out"
    );
}

#[test]
fn a_key_already_in_the_run_is_already_shown_whatever_the_profile_says() {
    let shown = set(&["event:first_raid"]);
    for g in [guidance(true, &[]), guidance(false, &["event:first_raid"])] {
        assert_eq!(
            decide("event:first_raid", &shown, &g),
            FtueDecision::AlreadyShown
        );
    }
}

#[test]
fn entry_keys_are_unique() {
    let mut keys: Vec<String> = ENTRIES.iter().map(|e| e.on.key()).collect();
    let total = keys.len();
    keys.sort();
    keys.dedup();
    assert_eq!(keys.len(), total, "two entries share a trigger");
}

#[test]
fn every_event_has_exactly_one_entry() {
    for event in FtueEvent::ALL {
        let n = ENTRIES
            .iter()
            .filter(|e| e.on == FtueTrigger::Event(event))
            .count();
        assert_eq!(n, 1, "{event:?}");
    }
}

#[test]
fn every_entry_has_words() {
    for entry in ENTRIES {
        assert!(
            !entry.title.is_empty() && !entry.body.is_empty(),
            "{:?}",
            entry.on
        );
    }
}

/// A typo in an asset id would fire never. Checked against the loaded
/// databases, so an unknown id fails `cargo test`.
#[test]
fn every_entry_names_a_real_asset() {
    use crate::Game;
    use crate::tests::support::test_assets_dir;
    let game = Game::new(7, crate::DifficultyMode::Forgiving, &test_assets_dir()).unwrap();
    for entry in ENTRIES {
        let (kind, found) = match entry.on {
            FtueTrigger::Research(id) => (
                "research",
                game.world
                    .resource::<crate::research::ResearchDb>()
                    .get(id)
                    .is_some(),
            ),
            FtueTrigger::Built(id) => (
                "structure",
                game.world
                    .resource::<crate::structures::StructureDb>()
                    .get(id)
                    .is_some(),
            ),
            FtueTrigger::Crafted(id) => (
                "item",
                game.world
                    .resource::<crate::items_db::ItemDb>()
                    .get(id)
                    .is_some(),
            ),
            FtueTrigger::Event(_) => continue,
        };
        assert!(found, "{} names no such {kind}", entry.on.key());
    }
}
