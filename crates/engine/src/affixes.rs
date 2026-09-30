//! Rolled name-and-stat modifiers on a dropped piece of gear.
//!
//! An affix is the second thing a drop rolls, after its rare tier: it gives
//! the copy a generated name — "Overclocked Arc Lance **of Static**" — and a
//! small extra stat bonus. Between them the two axes are why two copies of
//! one item are worth comparing at all, which is the whole point of the
//! feature (see `items::GearCopy`).
//!
//! **Fully data-driven.** Everything an affix *is* lives in
//! `assets/affixes/*.ron`; nothing here names a shipped one. A mod adds an
//! affix by dropping in a file, and it is immediately in the roll for every
//! item whose slot it allows.

use std::collections::{HashMap, HashSet};
use std::path::Path;

use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};

use crate::items::{EquipmentSlot, EquipmentStats, ItemId};
use crate::items_db::ItemDb;

/// A string newtype for the same reason `ItemId` is one: an affix is
/// content, so a mod must be able to add one without touching Rust, and a
/// save has to be able to name one that no longer exists without failing to
/// parse. `Ord` so a roll can walk a sorted pool deterministically.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct AffixId(pub String);

impl AffixId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for AffixId {
    fn from(s: &str) -> Self {
        AffixId(s.to_string())
    }
}

/// One affix as authored. See `assets/affixes/README.md` for the schema.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AffixDef {
    pub id: AffixId,
    /// A word placed *before* the item name. Exactly one of `prefix` and
    /// `suffix` must be set — an affix with both would produce
    /// "Overclocked Honed Arc Lance of Static", which no column on any
    /// screen has room for, and one with neither would be a stat bonus the
    /// player cannot see the source of.
    #[serde(default)]
    pub prefix: Option<String>,
    /// A phrase placed *after* the item name, conventionally "of ...".
    #[serde(default)]
    pub suffix: Option<String>,
    /// What this affix adds on top of the item's own bonus, before any
    /// scaling. Added rather than multiplied, so an affix is worth the same
    /// on a cheap item as an expensive one — which is what lets a scavenged
    /// weapon with a good affix stay interesting after a breach.
    ///
    /// A component may be **negative**: a drawback beside a bonus is a
    /// shipped affix shape, and folding it in here is what makes the cost
    /// scale with the run rather than dwindling out of relevance. What
    /// `fault` refuses is an affix with no positive component at all.
    #[serde(default)]
    pub stats: EquipmentStats,
    /// Which slots this affix may land on, or every slot when omitted. An
    /// affix granting DECOMP on a weapon reads oddly; this is how an author
    /// says so.
    #[serde(default)]
    pub slots: Option<Vec<EquipmentSlot>>,
    /// Relative likelihood within the eligible pool — weight 12 is twice as
    /// likely as weight 6. Not a probability: whether a copy gets an affix
    /// *at all* is `GEAR_AFFIX_CHANCE`, decided before this pool is
    /// consulted, exactly as `WILD_ROUTINE_CHANCE` and an ability's
    /// `wild_weight` are two separate questions. Folding them into one would
    /// mean adding an affix changed how often affixes appear.
    #[serde(default = "default_weight")]
    pub weight: u32,
    /// Present on an affix that is **research-only**: it never drops and
    /// never appears on a caravan shelf, and is reached only by researching
    /// it and applying it at a bench. See `AffixResearch`.
    #[serde(default)]
    pub research: Option<AffixResearch>,
}

/// What it takes to research an affix and then to apply it to a copy. Its
/// presence is what makes an affix research-only — `AffixDb::pool_for`
/// filters on it, which is the one door both roll sites (a drop and a
/// caravan shelf) draw their pool through.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AffixResearch {
    /// Research Data, as `ResearchDef::cost`.
    pub cost: u32,
    #[serde(default)]
    pub materials: Vec<(ItemId, u32)>,
    #[serde(default)]
    pub min_zone: u32,
    /// Other research-only affixes that must be researched first.
    #[serde(default)]
    pub requires: Vec<AffixId>,
    /// Paid from the pack on every apply. Required and non-empty: an affix
    /// that applies for free would be a recipe with no price.
    pub apply_cost: Vec<(ItemId, u32)>,
}

fn default_weight() -> u32 {
    1
}

impl AffixDef {
    /// `item_name` wearing this affix. The engine owns this translation for
    /// the reason `Rarity::label` does: a renderer that built the name
    /// itself would be a second copy, and the two would drift on the day
    /// someone adds an affix shape.
    pub fn decorate(&self, item_name: &str) -> String {
        match (&self.prefix, &self.suffix) {
            (Some(prefix), _) => format!("{prefix} {item_name}"),
            (None, Some(suffix)) => format!("{item_name} {suffix}"),
            (None, None) => item_name.to_string(),
        }
    }

    /// The word this affix reads as on a copy's name — its prefix or suffix,
    /// the id only for an affix `fault` would have refused.
    pub fn label(&self) -> String {
        self.prefix
            .clone()
            .or_else(|| self.suffix.clone())
            .unwrap_or_else(|| self.id.as_str().to_string())
    }

    /// Whether this affix may land on `slot`.
    pub fn fits(&self, slot: EquipmentSlot) -> bool {
        self.slots.as_ref().is_none_or(|s| s.contains(&slot))
    }

    /// Why this file is unusable, or `None` if it is fine. Checked at load
    /// so a malformed affix is skipped with a warning rather than reaching
    /// a name-building call site that has no way to refuse.
    fn fault(&self) -> Option<&'static str> {
        if self.prefix.is_none() && self.suffix.is_none() {
            return Some("needs a prefix or a suffix, or its bonus has no visible source");
        }
        if self.prefix.is_some() && self.suffix.is_some() {
            return Some("sets both prefix and suffix; no screen has room for both");
        }
        // A research-only affix is never rolled, so its weight is moot; it
        // is warned about in `load_dir` rather than refused here.
        if self.weight == 0 && self.research.is_none() {
            return Some("has weight 0, so it could never be rolled");
        }
        if self.stats.is_empty() {
            return Some("grants no stats, so it would rename an item and nothing else");
        }
        // A penalty is legal *beside* a bonus — that trade is a shipped
        // affix shape. A penalty on its own is not: nothing weighs it, so
        // the copy that rolled it is one no player has a reason to equip,
        // and the roll that produced it was a wasted one.
        if !self.stats.has_upside() {
            return Some("grants no positive stat, so it is a cost with nothing to weigh it");
        }
        if self.slots.as_ref().is_some_and(|s| s.is_empty()) {
            return Some("allows no slots, so it could never be rolled");
        }
        None
    }

    /// Why this file's research block is unusable against the items that
    /// loaded, or `None`. Separate from `fault` because it needs `ItemDb`,
    /// and a bad item id would otherwise surface as a silent free apply or
    /// an unpayable research bill mid-play.
    fn research_fault(&self, items: &ItemDb) -> Option<String> {
        let research = self.research.as_ref()?;
        if research.apply_cost.is_empty() {
            return Some("has an empty apply_cost, so applying it would be free".into());
        }
        let unknown = |lines: &[(ItemId, u32)]| {
            lines
                .iter()
                .find(|(item, _)| items.get(item.as_str()).is_none())
                .map(|(item, _)| item.as_str().to_string())
        };
        if let Some(item) = unknown(&research.apply_cost) {
            return Some(format!("apply_cost names unknown item {item:?}"));
        }
        if let Some(item) = unknown(&research.materials) {
            return Some(format!("materials names unknown item {item:?}"));
        }
        None
    }
}

/// Every affix the game knows about, loaded from `assets/affixes/`.
///
/// **An empty database is valid and inert**: `Game::roll_affix` finds no
/// pool and spends no RNG draw, so deleting the directory restores the
/// pre-affix game exactly. That is the same supported-way-to-play property
/// `assets/policies/enemy_battle.ron` has.
#[derive(Resource, Default)]
pub struct AffixDb {
    defs: HashMap<AffixId, AffixDef>,
}

impl AffixDb {
    /// Follows every other `load_dir` in the crate: a malformed or
    /// unusable file is skipped with a warning the game logs at startup,
    /// never a panic that stops a player reaching the main menu over
    /// somebody else's mod.
    ///
    /// Takes `ItemDb` to check the item ids a research block names, which is
    /// why it loads after items. A research-only affix whose `requires` names
    /// an affix that is unknown, not research-only, or itself refused is
    /// refused too, to a fixpoint — `ResearchDb::load_dir`'s cascade.
    pub fn load_dir(dir: &Path, items: &ItemDb) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = AffixDb::default();
        let mut warnings = Vec::new();
        // A missing directory is not an error: affixes are optional content
        // and an install without them is the pre-affix game.
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        for entry in entries {
            let path = entry?.path();
            if path.extension().and_then(|e| e.to_str()) != Some("ron") {
                continue;
            }
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<AffixDef>(&text) {
                Ok(def) => match def
                    .fault()
                    .map(String::from)
                    .or_else(|| def.research_fault(items))
                {
                    Some(why) => {
                        warnings.push(format!("skipped invalid affix file {path:?}: {why}"))
                    }
                    None => {
                        if def.research.is_some() && def.weight != default_weight() {
                            warnings.push(format!(
                                "affix file {path:?} sets weight on a research-only affix, \
                                 which is never rolled"
                            ));
                        }
                        db.defs.insert(def.id.clone(), def);
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid affix file {path:?}: {e}")),
            }
        }
        db.drop_unmet_requirements(&mut warnings);
        Ok((db, warnings))
    }

    /// Removes every research-only affix whose prerequisite is missing or
    /// not itself research-only, repeating until a pass removes nothing so
    /// that dropping one takes its dependants with it. Sorted so the warning
    /// order does not depend on the `HashMap`.
    fn drop_unmet_requirements(&mut self, warnings: &mut Vec<String>) {
        loop {
            let standing: HashSet<&AffixId> = self
                .defs
                .values()
                .filter(|d| d.research.is_some())
                .map(|d| &d.id)
                .collect();
            let mut doomed: Vec<(AffixId, AffixId)> = self
                .defs
                .values()
                .filter_map(|d| {
                    let missing = d
                        .research
                        .as_ref()?
                        .requires
                        .iter()
                        .find(|r| !standing.contains(r))?;
                    Some((d.id.clone(), missing.clone()))
                })
                .collect();
            if doomed.is_empty() {
                return;
            }
            doomed.sort();
            for (id, missing) in doomed {
                self.defs.remove(&id);
                warnings.push(format!(
                    "skipped affix {:?}: it requires {:?}, which is not a loaded research-only affix",
                    id.as_str(),
                    missing.as_str()
                ));
            }
        }
    }

    pub fn get(&self, id: &AffixId) -> Option<&AffixDef> {
        self.defs.get(id)
    }

    /// Every affix that may *roll* on `slot`, sorted by id — research-only
    /// affixes are excluded, and both roll sites (a drop and a caravan shelf)
    /// draw through here, so this is the one place that rule lives.
    ///
    /// Sorted for the reason `equipment_drops_for` and the cache table are:
    /// a seeded run has to consume its rolls in the same order however the
    /// files happen to come off the disk.
    pub fn pool_for(&self, slot: EquipmentSlot) -> Vec<&AffixDef> {
        let mut pool: Vec<&AffixDef> = self
            .defs
            .values()
            .filter(|d| d.research.is_none() && d.fits(slot))
            .collect();
        pool.sort_by(|a, b| a.id.as_str().cmp(b.id.as_str()));
        pool
    }

    pub fn all(&self) -> impl Iterator<Item = &AffixDef> {
        self.defs.values()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def(id: &str, slots: Option<Vec<EquipmentSlot>>) -> AffixDef {
        AffixDef {
            id: AffixId(id.into()),
            prefix: None,
            suffix: None,
            stats: EquipmentStats {
                atk: 1,
                ..Default::default()
            },
            slots,
            weight: 1,
            research: None,
        }
    }

    #[test]
    fn decorate_puts_a_prefix_before_and_a_suffix_after() {
        let with_prefix = AffixDef {
            prefix: Some("Honed".into()),
            ..def("p", None)
        };
        assert_eq!(with_prefix.decorate("Arc Lance"), "Honed Arc Lance");

        let with_suffix = AffixDef {
            suffix: Some("of Static".into()),
            ..def("s", None)
        };
        assert_eq!(with_suffix.decorate("Arc Lance"), "Arc Lance of Static");
    }

    #[test]
    fn an_affix_with_no_slot_list_fits_every_slot() {
        let no_slots = def("ns", None);
        assert!(no_slots.fits(EquipmentSlot::Weapon));
        assert!(no_slots.fits(EquipmentSlot::Armor));
        assert!(no_slots.fits(EquipmentSlot::Module));

        let weapon_only = def("w", Some(vec![EquipmentSlot::Weapon]));
        assert!(weapon_only.fits(EquipmentSlot::Weapon));
        assert!(!weapon_only.fits(EquipmentSlot::Armor));
    }

    #[test]
    fn pool_for_filters_by_slot_and_sorts_by_id() {
        let defs = HashMap::from([
            (
                "zeta".into(),
                def("zeta", Some(vec![EquipmentSlot::Weapon])),
            ),
            ("mid".into(), def("mid", Some(vec![EquipmentSlot::Armor]))),
            ("alpha".into(), def("alpha", None)),
            // Five fit, so a HashMap's order passes an unsorted pool 1 run in 120, not 1 in 2.
            ("echo".into(), def("echo", None)),
            (
                "bravo".into(),
                def("bravo", Some(vec![EquipmentSlot::Weapon])),
            ),
            ("delta".into(), def("delta", None)),
        ]);
        let db = AffixDb { defs };

        let pool: Vec<&str> = db
            .pool_for(EquipmentSlot::Weapon)
            .iter()
            .map(|a| a.id.as_str())
            .collect();
        assert_eq!(pool, vec!["alpha", "bravo", "delta", "echo", "zeta"]);
    }
}
