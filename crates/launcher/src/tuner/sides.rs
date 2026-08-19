//! Which species the *player* fields, and therefore which ones the search
//! may not move.
//!
//! A target says "this fight should be won 75% of the time". It does not say
//! whether to get there by making the opponent weaker or the party weaker,
//! and the search will take whichever is cheaper. The first real run took
//! the second: it raised `rootkit`, the opponent in `full-group.ron`, *and*
//! dropped `scrapper.base_mitigation` from 5 to its bound floor of 0 — and three
//! Scrappers are the party in that same scenario. Both moves lower the win
//! rate's error term. Only one of them is a balance change anybody wanted,
//! because a stat lowered to satisfy one fight applies to that species
//! everywhere in the game.
//!
//! The scenarios already carry the answer: every one names its `party` and
//! its `opponents` separately. So the frozen set is derived rather than
//! configured, and cannot drift out of step with the fights it protects.
//!
//! **This is a narrow fix for a two-sided roster.** Every species in this
//! game can be tamed, so "enemy species" and "companion species" are not
//! disjoint categories — a scenario's `party` list is only which ones that
//! authored fight happens to field. Freezing them stops the search
//! exploiting a species in the one role it was measured in; it does not make
//! the objective two-sided. The fix that would is coverage: field a species
//! on *both* sides of some pair of targets and lowering it costs a fight
//! elsewhere, so the search self-corrects with no frozen set at all.

use super::score::Target;
use feral_processes_engine::arena::{PlayerSource, Scenario};
use std::collections::BTreeSet;

/// Every species the player fields across all of `targets`.
///
/// Refuses a save-backed scenario rather than reporting a smaller set.
/// `Scenario::party` is `Fresh`-only — a `Save` or `Template` player brings
/// whatever roster the save holds, which is invisible from here — so a
/// silent empty answer would be indistinguishable from "this fight has no
/// companions" and would nerf them exactly as before the freeze existed.
pub fn player_fielded(targets: &[Target]) -> Result<BTreeSet<String>, String> {
    let mut frozen = BTreeSet::new();
    for target in targets {
        let scenario = Scenario::load(std::path::Path::new(&target.scenario))
            .map_err(|e| format!("{}: {e}", target.scenario))?;
        match &scenario.player {
            PlayerSource::Fresh { .. } => {
                frozen.extend(scenario.party.iter().map(|c| c.species.clone()));
            }
            PlayerSource::Save(_) | PlayerSource::Template(_) => {
                return Err(format!(
                    "{} draws its player from a save, whose party the tuner cannot read: \
                     `party` is a `Fresh`-only field. Every companion in that save would \
                     be treated as an opponent and could be nerfed to satisfy this \
                     target. Point the target at a `Fresh` scenario, or freeze that \
                     save's species by fielding them in one.",
                    target.scenario
                ));
            }
        }
    }
    Ok(frozen)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn repo_path(rel: &str) -> String {
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join(rel)
            .to_string_lossy()
            .into_owned()
    }

    fn target(scenario: &str) -> Target {
        Target {
            scenario: repo_path(scenario),
            reps: 1,
            want_win_rate: 0.5,
            want_hp_left: 0.5,
            weight: 1.0,
        }
    }

    #[test]
    fn a_species_the_player_fields_is_frozen() {
        // The exact bug: `full-group.ron` fields three Scrappers and the
        // first real run dropped `scrapper.base_mitigation` to 0 to satisfy it.
        let frozen = player_fielded(&[target("dev-arenas/full-group.ron")]).unwrap();
        assert!(frozen.contains("scrapper"), "got {frozen:?}");
    }

    #[test]
    fn a_species_only_ever_fought_is_left_movable() {
        // Freezing has to cost something, or it would be freezing everything.
        let frozen = player_fielded(&[target("dev-arenas/full-group.ron")]).unwrap();
        assert!(!frozen.contains("rootkit"), "got {frozen:?}");
    }

    #[test]
    fn a_scenario_with_no_companions_freezes_nothing() {
        let frozen = player_fielded(&[target("dev-arenas/opening-fight.ron")]).unwrap();
        assert!(frozen.is_empty(), "got {frozen:?}");
    }

    #[test]
    fn every_targets_party_is_collected_not_just_the_first() {
        let frozen = player_fielded(&[
            target("dev-arenas/opening-fight.ron"),
            target("dev-arenas/full-group.ron"),
        ])
        .unwrap();
        assert!(frozen.contains("scrapper"), "got {frozen:?}");
    }

    #[test]
    fn a_save_backed_target_is_refused_rather_than_silently_freezing_nothing() {
        // `party` is `Fresh`-only, so a save-backed scenario reports an
        // empty party and would leave every companion in it movable — the
        // original bug, wearing the freeze as cover.
        let mut t = target("dev-arenas/full-group.ron");
        let text = std::fs::read_to_string(&t.scenario).unwrap();
        let swapped = text.replace(
            "player: Fresh(level: 20, zone: 3),",
            r#"player: Save("saves/save.bin"),"#,
        );
        assert_ne!(swapped, text, "scenario text moved; fixture needs updating");

        let dir = std::env::temp_dir().join("feral_processes_tuner_sides_save");
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("save-backed.ron");
        std::fs::write(&path, swapped).unwrap();
        t.scenario = path.to_string_lossy().into_owned();

        let err = player_fielded(&[t]).unwrap_err();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(err.contains("save"), "got: {err}");
    }

    #[test]
    fn a_missing_scenario_is_an_error_rather_than_an_empty_set() {
        let err = player_fielded(&[target("dev-arenas/does-not-exist.ron")]).unwrap_err();
        assert!(err.contains("does-not-exist"), "got: {err}");
    }
}
