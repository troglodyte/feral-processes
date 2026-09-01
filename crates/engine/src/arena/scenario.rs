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

use crate::abilities::AbilityId;
use crate::affixes::AffixId;
use crate::components::Rarity;
use crate::game::creation::CharacterChoice;
use crate::items::{GearCopy, ItemId};
use crate::classes::PlayerClass;
use crate::species::SpeciesId;
use crate::world::Biome;

/// A fight, authored rather than rolled.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct Scenario {
    pub player: PlayerSource,
    /// `Fresh` only — who the player was made as. A save or a template
    /// already carries its own answer.
    pub character: CharacterSpec,
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
    /// A context to roll, instead of naming `opponents`. Mutually exclusive
    /// with them — one scenario asks one question.
    pub encounter: Option<Encounter>,
    pub reps: u32,
    pub seed: u64,
}

/// Hand-written rather than derived so `reps` is 1 — a scenario that ran
/// zero fights and reported a clean sheet is the worst possible default.
impl Default for Scenario {
    fn default() -> Self {
        Self {
            player: PlayerSource::default(),
            character: CharacterSpec::default(),
            equip: Vec::new(),
            inventory: Vec::new(),
            party: Vec::new(),
            opponents: Vec::new(),
            encounter: None,
            reps: 1,
            seed: 0,
        }
    }
}

/// Where a fight is being had, for a scenario that wants the game's own roll
/// rather than an authored composition.
///
/// The zone is deliberately absent: it comes from the player row, and
/// `ZoneLevel` is one resource driving both gear scaling and enemy scaling —
/// a second zone on the same screen would be two answers to one question.
/// The biome is here because it alone decides the species pool, and the
/// arena's player stands on whatever tile `Game::new` dropped them on.
///
/// `depth` takes no serde default: a Stack encounter with no depth is a typo,
/// the same argument this file's header makes about an unnamed species id.
///
/// `Stack` and `Lair` are both underground at a named depth, and they are not
/// the same question. `Stack` is the corridor — what a walk between frames
/// costs, and never a boss, since `stack_encounter_pack` passes
/// `allow_boss: false`. `Lair` is the thing at the bottom, the only boss the
/// Stack fields and the game's only source of Portal Fragments. A depth curve
/// that leaves the corridor walkable can still leave the gate shut, so the
/// instrument has to be able to ask about the two separately.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum Encounter {
    Field { biome: Biome },
    Stack { biome: Biome, depth: u32 },
    Lair { biome: Biome, depth: u32 },
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

/// Who the player was made as, for a `Fresh` scenario — the class, the
/// creation stat pool and the starter routine, the three creation answers
/// that reach a fight.
///
/// The look is deliberately absent: a glyph and a swatch are what the map
/// draws, and nothing in a staged fight reads either.
///
/// **The headless bin can only see part of this.** `run_rep` plays
/// `PartyPlan::AllAttack` — the game's own `[A]` plan — which invokes no
/// routine, and a class is a spread of multipliers over *authored routine
/// power* alone (`Game::ability_affinity` is not on the ordinary swing's
/// path at all). So `class` and `routine` change nothing the bin reports;
/// they are for the played arena, `FERAL_DEV_ARENA=1 cargo run`. `stats`
/// lands in `Stats` and is visible to both.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CharacterSpec {
    pub class: Option<PlayerClass>,
    /// Units *bought* per axis, in `MainStat::all()` order — Atk, Def,
    /// Integrity, Decompiler — never points spent.
    /// `CharacterChoice::cost()` prices them, and `build_player` refuses a
    /// spend the pool cannot cover rather than applying none of it: fail
    /// closed is right inside a run and wrong in an instrument, where a
    /// silently ignored input reads as the axis being worthless.
    pub stats: [u32; 4],
    pub routine: Option<AbilityId>,
}

impl CharacterSpec {
    /// The choice this spec names. `EquipSpec::copy`'s precedent — one
    /// place a scenario's authored player becomes the engine's own type,
    /// so the bin and the in-game arena screen cannot disagree about what
    /// a file asked for.
    pub fn choice(&self) -> CharacterChoice {
        CharacterChoice {
            class: self.class,
            stats: self.stats,
            routine: self.routine.clone(),
            ..CharacterChoice::default()
        }
    }
}

/// An item to put on the player. `tier` is the fusion tier of the copy and
/// `rarity` its rare tier — gear varies per physical copy, so a scenario
/// blind to either measures a different weapon from the one it names.
///
/// Both default, so every scenario written before they existed still
/// describes exactly the copy it always did: ordinary and unfused.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct EquipSpec {
    pub item: ItemId,
    #[serde(default)]
    pub tier: u32,
    #[serde(default)]
    pub rarity: Rarity,
    /// Affixes by id, in any order — the spec canonicalises. Authorable for
    /// the same reason `rarity` is: an affix is rolled on a *drop*, and a
    /// staged fight drops nothing, so this is the only way to measure what
    /// one is worth. Repeat an id to stack it, which is what fusion does.
    #[serde(default)]
    pub affixes: Vec<AffixId>,
}

impl EquipSpec {
    /// The copy this spec names. `arena::stage` is the one way into a
    /// staged fight, so this is the one place a scenario's gear becomes a
    /// `GearCopy` — the headless bin and the in-game arena screen cannot
    /// disagree about what a scenario asked for.
    pub fn copy(&self) -> GearCopy {
        GearCopy::with_affixes(
            self.item.clone(),
            self.rarity,
            self.tier,
            self.affixes.clone(),
            // A scenario authors no quality, so its numbers stay comparable
            // to the reports its old runs produced.
            crate::tuning::QUALITY_DEFAULT,
        )
    }
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
    /// What this companion is wearing. Any program the player owns may wear
    /// gear, so a party authored without it is a *naked* party — weaker
    /// than one a run of the same shape would actually field, which biases
    /// every measurement taken against it in one direction.
    #[serde(default)]
    pub equip: Vec<EquipSpec>,
}

/// Hand-written for the same reason `Scenario`'s is: `level` has a meaning
/// at 1 and none at 0, and the derived zero would field a companion no
/// spawn path can produce.
impl Default for CompanionSpec {
    fn default() -> Self {
        Self {
            species: SpeciesId::default(),
            level: one(),
            equip: Vec::new(),
        }
    }
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
        if self.encounter.is_some() && !self.opponents.is_empty() {
            return Err(
                "`encounter` and `opponents` are mutually exclusive — a rolled context \
                 fields its own composition"
                    .into(),
            );
        }
        if self.encounter.is_none() && self.opponents.is_empty() {
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

    #[test]
    fn a_rolled_field_encounter_parses() {
        let s = Scenario::from_ron(
            r#"(
                player: Fresh(level: 3, zone: 1),
                encounter: Some(Field(biome: Mainframe)),
            )"#,
        )
        .unwrap();
        assert_eq!(
            s.encounter,
            Some(Encounter::Field {
                biome: Biome::Mainframe
            })
        );
        assert!(s.opponents.is_empty());
    }

    #[test]
    fn a_rolled_stack_encounter_carries_its_depth() {
        let s = Scenario::from_ron(
            r#"(
                encounter: Some(Stack(biome: OpenGrid, depth: 5)),
                reps: 50,
            )"#,
        )
        .unwrap();
        let path = std::env::temp_dir().join("feral_processes_scenario_rolled_round_trip.ron");

        s.save(&path).unwrap();
        let back = Scenario::load(&path).unwrap();
        let _ = std::fs::remove_file(&path);

        assert_eq!(
            back.encounter,
            Some(Encounter::Stack {
                biome: Biome::OpenGrid,
                depth: 5,
            })
        );
        assert_eq!(s, back);
    }

    #[test]
    fn an_encounter_beside_opponents_is_an_err_naming_both() {
        let err = Scenario::from_ron(
            r#"(
                encounter: Some(Field(biome: Mainframe)),
                opponents: [(species: "glitch", count: 1)],
            )"#,
        )
        .unwrap_err();
        assert!(err.contains("encounter"), "{err}");
        assert!(err.contains("opponents"), "{err}");
    }

    #[test]
    fn a_scenario_without_an_encounter_still_needs_opponents() {
        let err = Scenario::from_ron("( opponents: [] )").unwrap_err();
        assert!(err.contains("opponents"), "{err}");
        assert!(Scenario::default().encounter.is_none());
    }
}
