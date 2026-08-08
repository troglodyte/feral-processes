//! What a fight is, as a file.
//!
//! Every field of `Scenario` is `#[serde(default)]`, so a scenario written
//! today keeps parsing after a knob is added — the same promise
//! `assets/*/README.md` makes to a modder, for the same reason.
//!
//! The spec rows are the deliberate exception: their *numbers* default, but
//! the id naming a species or an item does not. A row with no id has no
//! sensible zero — an unnamed item is not the empty item, it is a typo.

use serde::{Deserialize, Serialize};
use std::path::{Path, PathBuf};

use crate::items::ItemId;
use crate::species::SpeciesId;

/// A fight, authored rather than rolled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scenario {
    pub player: PlayerSource,
    /// `Fresh` only. Applied after the zone is set, since gear locks in the
    /// zone level it was equipped at.
    pub equip: Vec<EquipSpec>,
    /// `Fresh` only.
    pub inventory: Vec<InventorySpec>,
    /// `Fresh` only.
    pub party: Vec<CompanionSpec>,
    /// Order is formation: `ENGAGED_GROUPS` is 2, so entries past the second
    /// are out of melee reach.
    pub opponents: Vec<OpponentSpec>,
    pub reps: u32,
    pub seed: u64,
}

/// Hand-written rather than derived so `reps` is 1 — a scenario that ran
/// zero fights and reported a clean sheet is the worst possible default.
impl Default for Scenario {
    fn default() -> Self {
        Self {
            player: PlayerSource::default(),
            equip: Vec::new(),
            inventory: Vec::new(),
            party: Vec::new(),
            opponents: Vec::new(),
            reps: 1,
            seed: 0,
        }
    }
}

/// Where the player under test comes from.
///
/// A save or a template brings its whole run across — level, stats,
/// equipment and its fusion tiers, party, perks, zone. `Fresh` is the other
/// half, and the only one that accepts an authored loadout.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum PlayerSource {
    Fresh {
        #[serde(default = "one")]
        level: u32,
        #[serde(default = "one")]
        zone: u32,
    },
    Save(PathBuf),
    /// Resolved to a `Save` by the `arena` bin before the engine sees it —
    /// `dev_template` lives in the launcher, which the engine cannot see.
    Template(String),
}

fn one() -> u32 {
    1
}

impl Default for PlayerSource {
    fn default() -> Self {
        Self::Fresh { level: 1, zone: 1 }
    }
}

/// An item to put on the player. `tier` is the fusion tier of the copy —
/// gear fuses per physical copy, so a tier-blind scenario measures a
/// different weapon from the one it names.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EquipSpec {
    pub item: ItemId,
    #[serde(default)]
    pub tier: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct InventorySpec {
    pub item: ItemId,
    #[serde(default = "one")]
    pub qty: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CompanionSpec {
    pub species: SpeciesId,
    #[serde(default = "one")]
    pub level: u32,
}

/// One enemy group. There is deliberately no level here: a wild spawn
/// carries no `Experience`, so the zone is the strength dial and `count` is
/// the volume dial.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OpponentSpec {
    pub species: SpeciesId,
    #[serde(default = "one")]
    pub count: u32,
}

impl Scenario {
    pub fn load(path: &Path) -> Result<Self, String> {
        // Both errors name the path: a bare "expected struct" with no
        // filename is what makes a typo expensive to find.
        let text = std::fs::read_to_string(path).map_err(|e| format!("{}: {e}", path.display()))?;
        Self::from_ron(&text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn from_ron(text: &str) -> Result<Self, String> {
        let scenario: Self = ron::from_str(text).map_err(|e| e.to_string())?;
        scenario.validate()?;
        Ok(scenario)
    }

    /// Writes this scenario to `path`, **overwriting whatever is there**.
    ///
    /// Overwriting is the behaviour rather than a hazard, following
    /// `dev_template::generate`: a scenario file exists so the same fight
    /// comes back, and a save that preserved the last version would leave
    /// the author guessing which copy the bin is about to run.
    ///
    /// Validated on the way out, so a file this writes is one `load` will
    /// take back — the round trip is what makes the two tools one library.
    pub fn save(&self, path: &Path) -> Result<(), String> {
        self.validate()?;
        let text = self.to_ron()?;
        std::fs::write(path, text).map_err(|e| format!("{}: {e}", path.display()))
    }

    pub fn to_ron(&self) -> Result<String, String> {
        ron::ser::to_string_pretty(self, ron::ser::PrettyConfig::default())
            .map_err(|e| e.to_string())
    }

    /// Everything checkable without a loaded `Game`. Species and item ids
    /// are deliberately *not* checked here — those need the databases, and
    /// so are `setup`'s job.
    fn validate(&self) -> Result<(), String> {
        if self.opponents.is_empty() {
            return Err("`opponents` is empty — a scenario with nobody to fight is a typo".into());
        }
        if !matches!(self.player, PlayerSource::Fresh { .. }) {
            for (field, populated) in [
                ("equip", !self.equip.is_empty()),
                ("inventory", !self.inventory.is_empty()),
                ("party", !self.party.is_empty()),
            ] {
                if populated {
                    return Err(format!(
                        "`{field}` applies only to a `Fresh` player — a save or template brings its own"
                    ));
                }
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_minimal_scenario_parses_at_the_documented_defaults() {
        let s = Scenario::from_ron(r#"( opponents: [(species: "glitch", count: 1)] )"#).unwrap();
        assert_eq!(s.reps, 1);
        assert_eq!(s.seed, 0);
        assert_eq!(s.player, PlayerSource::Fresh { level: 1, zone: 1 });
        assert!(s.equip.is_empty());
    }

    #[test]
    fn a_full_fresh_scenario_parses_with_every_field_populated() {
        let s = Scenario::from_ron(
            r#"(
                player: Fresh(level: 20, zone: 3),
                equip: [
                    (item: "monofilament_whip", tier: 0),
                    (item: "ablative_plating", tier: 2),
                ],
                inventory: [(item: "power_cell", qty: 5)],
                party: [
                    (species: "scrapper", level: 14),
                    (species: "scrapper", level: 14),
                ],
                opponents: [(species: "sub_process", count: 9)],
                reps: 50,
                seed: 7,
            )"#,
        )
        .unwrap();
        assert_eq!(s.player, PlayerSource::Fresh { level: 20, zone: 3 });
        assert_eq!(s.equip.len(), 2);
        assert_eq!(s.equip[1].tier, 2);
        assert_eq!(s.inventory[0].qty, 5);
        assert_eq!(s.party.len(), 2);
        assert_eq!(s.opponents[0].count, 9);
        assert_eq!(s.reps, 50);
        assert_eq!(s.seed, 7);
    }

    #[test]
    fn a_save_scenario_parses_and_holds_the_path() {
        let s = Scenario::from_ron(
            r#"(
                player: Save("saves/save.bin"),
                opponents: [(species: "sub_process", count: 9)],
            )"#,
        )
        .unwrap();
        assert_eq!(s.player, PlayerSource::Save("saves/save.bin".into()));
    }

    #[test]
    fn broken_ron_is_an_err_that_says_something() {
        let err = Scenario::from_ron("( opponents: [ ").unwrap_err();
        assert!(!err.is_empty());
    }

    #[test]
    fn equip_on_a_save_scenario_is_an_err_naming_the_field() {
        let err = Scenario::from_ron(
            r#"(
                player: Save("saves/save.bin"),
                equip: [(item: "monofilament_whip", tier: 0)],
                opponents: [(species: "glitch", count: 1)],
            )"#,
        )
        .unwrap_err();
        assert!(err.contains("equip"), "{err}");
    }

    #[test]
    fn inventory_and_party_on_a_save_scenario_are_likewise_an_err() {
        let with = |field: &str, rows: &str| {
            Scenario::from_ron(&format!(
                r#"(
                    player: Save("saves/save.bin"),
                    {field}: {rows},
                    opponents: [(species: "glitch", count: 1)],
                )"#
            ))
        };
        let inv = with("inventory", r#"[(item: "power_cell", qty: 1)]"#).unwrap_err();
        assert!(inv.contains("inventory"), "{inv}");
        let party = with("party", r#"[(species: "scrapper", level: 3)]"#).unwrap_err();
        assert!(party.contains("party"), "{party}");
        // A `Template` player is the same rule; it is not a second one.
        let tmpl = Scenario::from_ron(
            r#"(
                player: Template("extraction"),
                party: [(species: "scrapper", level: 3)],
                opponents: [(species: "glitch", count: 1)],
            )"#,
        )
        .unwrap_err();
        assert!(tmpl.contains("party"), "{tmpl}");
    }

    #[test]
    fn a_written_scenario_reads_back_as_itself() {
        // The property that makes the builder and the bin one library
        // rather than two: what the screen writes, `Scenario::load` takes.
        let s = Scenario::from_ron(
            r#"(
                player: Template("extraction"),
                opponents: [(species: "sub_process", count: 9), (species: "glitch", count: 2)],
                reps: 50,
                seed: 7,
            )"#,
        )
        .unwrap();
        let path = std::env::temp_dir().join("feral_processes_scenario_round_trip.ron");

        s.save(&path).unwrap();
        let back = Scenario::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(s, back);
    }

    #[test]
    fn a_scenario_with_no_opponents_is_not_written() {
        // Validated on the way out as well as in, or the screen could
        // produce a file only it can open.
        let path = std::env::temp_dir().join("feral_processes_scenario_invalid.ron");
        let err = Scenario::default().save(&path).unwrap_err();
        assert!(err.contains("opponents"), "{err}");
        assert!(!path.exists(), "an invalid scenario reached disk");
    }

    #[test]
    fn a_scenario_with_no_opponents_is_an_err() {
        let err = Scenario::from_ron("( opponents: [] )").unwrap_err();
        assert!(err.contains("opponents"), "{err}");
    }
}
