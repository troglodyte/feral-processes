//! Per-class talent trees, loaded from `assets/talents/`.
//!
//! A companion earns one talent point per level above `CREATURE_MAX_LEVEL`
//! (see `Game::talent_points`), and spends it on one of two choices in the
//! next untaken tier of its class's tree. The trees are **data**, unlike
//! `perks::Perk`: every node here is one of four shapes the engine already
//! knows how to apply, so a mod can ship a sixth class's tree by dropping in a
//! file. `assets/talents/README.md` is the schema reference.

use crate::abilities::{AbilityId, AffinityKind};
use crate::species::AffinityClass;
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// A talent node's id, unique across every tree — a string newtype for the
/// same reason `items::ItemId` is one: a mod's node cannot be an enum variant.
/// `transparent`, like `items::ItemId`: a node is referenced in a `.ron` file
/// as a plain quoted string rather than as `TalentId("...")`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TalentId(String);

impl TalentId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for TalentId {
    fn from(s: &str) -> Self {
        TalentId(s.to_string())
    }
}

impl std::fmt::Display for TalentId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Which of a creature's three stats a `TalentNode::Stat` raises.
///
/// Deliberately not `achievements::MainStat`, whose fourth axis is the
/// player-only `Decompiler` — a companion has no such stat, and a node naming
/// it would be authorable and silently do nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum TalentStat {
    Hp,
    Atk,
    Def,
}

/// What taking a node does. Four shapes, each one the engine already applies
/// somewhere else — which is the whole argument for the tree being data.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum TalentNode {
    /// Raises one stat by `percent`, baked into `Stats` at purchase through
    /// the same `refactor::raised` a percentage buff uses — including its
    /// never-less-than-a-point floor.
    Stat { stat: TalentStat, percent: f32 },
    /// Multiplies this companion's affinity for one ability category, clamped
    /// to `tuning::AFFINITY_MAX` the way the player's perk arm is.
    Affinity { kind: AffinityKind, mult: f32 },
    /// Grants a routine outright, installed exactly as a species-kit unlock
    /// is — same path, same slot competition.
    Ability { id: AbilityId },
    /// One more routine slot than this companion's level would give it.
    RoutineSlot,
}

impl TalentNode {
    /// A two-word tag naming the node's shape, for a menu row. Here rather
    /// than in a renderer for the reason `Game::copy_name` is one function:
    /// two screens building it themselves is two screens that can disagree.
    pub fn tag(&self) -> &'static str {
        match self {
            TalentNode::Stat { .. } => "stat",
            TalentNode::Affinity { .. } => "affinity",
            TalentNode::Ability { .. } => "routine",
            TalentNode::RoutineSlot => "slot",
        }
    }
}

/// One of the two choices in a tier.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TalentChoice {
    pub id: TalentId,
    pub name: String,
    pub description: String,
    pub node: TalentNode,
}

/// One rung of a tree: exactly `CHOICES_PER_TIER` choices, of which a
/// companion takes one.
/// `transparent` so a tier is authored as a bare list — `[choice, choice]`
/// rather than `([choice, choice])`, which is what ron demands of a newtype
/// struct otherwise and reads as a typo waiting to happen in every file.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TalentTier(pub Vec<TalentChoice>);

/// One class's ladder. `class: None` is the generic tree, which is what a
/// species raising no axis or more than one gets — see
/// `SpeciesDef::affinity_class`.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct TalentTree {
    #[serde(default)]
    pub class: Option<AffinityClass>,
    pub tiers: Vec<TalentTier>,
}

/// How many choices every tier offers. Two: a tier is a decision, and a
/// third option makes the ladder a list of everything with extra steps.
pub const CHOICES_PER_TIER: usize = 2;

/// How deep every tree is. One tier per level a fully ringed companion can
/// earn, so the last point spends the last rung.
pub fn tiers_per_tree() -> usize {
    (crate::tuning::KERNEL_RING_MAX * crate::tuning::LEVELS_PER_RING) as usize
}

#[derive(Resource, Default)]
pub struct TalentDb {
    by_class: HashMap<AffinityClass, TalentTree>,
    generic: Option<TalentTree>,
}

impl TalentDb {
    /// Loads every `*.ron` tree in `dir`. A malformed or ill-formed file is
    /// skipped with a returned warning rather than aborting the load, exactly
    /// as `PerkDb::load_dir` does — a mod's broken tree costs that class its
    /// ladder and nothing else.
    ///
    /// An absent directory is **not** silent here, unlike `AffixDb`: the
    /// shipped trees are what every level past the cap is spent on, and a
    /// missing directory is an install fault rather than a supported state.
    /// The caller surfaces the `io::Error` as a startup warning.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = TalentDb::default();
        let mut warnings = Vec::new();
        let mut paths: Vec<_> = std::fs::read_dir(dir)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        // Sorted, because two files claiming one class must resolve the same
        // way every run — bevy's iteration order is not the only unstable
        // order in this game.
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<TalentTree>(&text) {
                Ok(tree) => match tree_faults(&tree) {
                    Some(fault) => {
                        warnings.push(format!("skipped invalid talent file {path:?}: {fault}"))
                    }
                    None => match tree.class {
                        Some(class) => {
                            db.by_class.insert(class, tree);
                        }
                        None => db.generic = Some(tree),
                    },
                },
                Err(e) => warnings.push(format!("skipped invalid talent file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    /// The tree a companion of `class` spends its points in, falling back to
    /// the generic tree.
    ///
    /// It takes `Option<AffinityClass>` rather than `AffinityClass` because
    /// `SpeciesDef::affinity_class` returns one: `None` means *no base job*
    /// rather than a default class, and a boss or a mod raising two axes must
    /// land on the generic tree rather than silently acquiring a Medic's.
    pub fn get(&self, class: Option<AffinityClass>) -> Option<&TalentTree> {
        class
            .and_then(|c| self.by_class.get(&c))
            .or(self.generic.as_ref())
    }

    /// Every node in every loaded tree, for the censuses. Order is by class
    /// map iteration and so unstable; nothing may read it expecting an order.
    pub fn all_nodes(&self) -> impl Iterator<Item = &TalentChoice> {
        self.by_class
            .values()
            .chain(self.generic.as_ref())
            .flat_map(|t| t.tiers.iter())
            .flat_map(|tier| tier.0.iter())
    }

    /// Every loaded tree, for the censuses.
    pub fn trees(&self) -> impl Iterator<Item = &TalentTree> {
        self.by_class.values().chain(self.generic.as_ref())
    }
}

/// Why `tree` is unusable, or `None` if it is well-formed. The verdict rather
/// than the ingredients, like `species::stat_shape_faults` — and the whole
/// reason both halves are checked here is that a tree one tier short strands
/// the last talent point a fully ringed companion earns, with nothing on
/// screen to say so.
fn tree_faults(tree: &TalentTree) -> Option<String> {
    let want = tiers_per_tree();
    if tree.tiers.len() != want {
        return Some(format!(
            "a tree needs exactly {want} tiers (KERNEL_RING_MAX * LEVELS_PER_RING), got {}",
            tree.tiers.len()
        ));
    }
    for (i, tier) in tree.tiers.iter().enumerate() {
        if tier.0.len() != CHOICES_PER_TIER {
            return Some(format!(
                "tier {} needs exactly {CHOICES_PER_TIER} choices, got {}",
                i + 1,
                tier.0.len()
            ));
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A well-formed tree of `tiers` tiers, each of `choices` choices.
    fn tree_text(class: &str, tiers: usize, choices: usize) -> String {
        let mut body = String::new();
        for t in 0..tiers {
            body.push_str("        [\n");
            for c in 0..choices {
                body.push_str(&format!(
                    "            (id: \"n{t}_{c}\", name: \"N{t}{c}\", description: \"d\", \
                     node: Stat(stat: Atk, percent: 5.0)),\n"
                ));
            }
            body.push_str("        ],\n");
        }
        format!("(\n    class: {class},\n    tiers: [\n{body}    ],\n)\n")
    }

    fn load(files: &[(&str, String)]) -> (TalentDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir("talents");
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        TalentDb::load_dir(&dir).unwrap()
    }

    #[test]
    fn a_well_formed_tree_loads_with_no_warnings() {
        let (db, warnings) = load(&[(
            "striker.ron",
            tree_text("Some(Striker)", tiers_per_tree(), CHOICES_PER_TIER),
        )]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let tree = db.get(Some(AffinityClass::Striker)).expect("loaded");
        assert_eq!(tree.tiers.len(), tiers_per_tree());
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning_and_the_others_still_load() {
        let (db, warnings) = load(&[
            ("bad.ron", "(class: Some(Striker), tiers: [".to_string()),
            (
                "medic.ron",
                tree_text("Some(Medic)", tiers_per_tree(), CHOICES_PER_TIER),
            ),
        ]);
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert!(warnings[0].contains("bad.ron"));
        assert!(db.get(Some(AffinityClass::Medic)).is_some());
        assert!(
            db.get(Some(AffinityClass::Striker)).is_none(),
            "and the broken class has no tree at all, not a half-built one"
        );
    }

    #[test]
    fn a_tier_with_the_wrong_number_of_choices_is_skipped_and_named() {
        for choices in [1, 3] {
            let (db, warnings) = load(&[(
                "striker.ron",
                tree_text("Some(Striker)", tiers_per_tree(), choices),
            )]);
            assert!(db.get(Some(AffinityClass::Striker)).is_none());
            assert_eq!(warnings.len(), 1, "{warnings:?}");
            assert!(
                warnings[0].contains("tier 1"),
                "the warning has to name the tier, got: {warnings:?}"
            );
        }
    }

    #[test]
    fn a_tree_of_the_wrong_depth_is_skipped_with_a_warning() {
        let (db, warnings) = load(&[(
            "striker.ron",
            tree_text("Some(Striker)", tiers_per_tree() - 1, CHOICES_PER_TIER),
        )]);
        assert!(db.get(Some(AffinityClass::Striker)).is_none());
        assert!(
            warnings[0].contains(&tiers_per_tree().to_string()),
            "{warnings:?}"
        );
    }

    /// `None` means no base job, and must land on the generic tree rather than
    /// on nothing — a boss carries no affinities, and so does a mod raising
    /// two axes.
    #[test]
    fn a_class_with_no_file_falls_back_to_the_generic_tree() {
        let (db, warnings) = load(&[(
            "generic.ron",
            tree_text("None", tiers_per_tree(), CHOICES_PER_TIER),
        )]);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert!(db.get(None).is_some(), "None is the generic tree");
        assert!(
            db.get(Some(AffinityClass::Leech)).is_some(),
            "and so is a class with no file of its own"
        );
    }
}
