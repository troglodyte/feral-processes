//! Player implants, loaded from `assets/implants/`.
//!
//! An implant is something the player builds into their own avatar: always
//! on, paid for in Power while it runs, and paid for again to take out. Each
//! costs Neural Load against a cap that grows with level; running past the
//! cap is allowed and invites rejection at battle start.
//!
//! The effects are a closed vocabulary — absolute stat deltas, a short list of
//! hooks, one optional signature and one optional downside — that a `.ron`
//! file combines; the magnitudes are the file's and the formulas are here and
//! in `tuning.rs`. `assets/implants/README.md` is the schema reference.
//!
//! **An empty database is valid**, and a missing directory is not an error
//! (`StatusDb`'s rule). An id whose def is missing contributes nothing:
//! every reader goes through `ImplantDb::get` and skips a `None`.

use crate::components::Implants;
use crate::statuses::StatusId;
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::Path;

/// An implant's name in a `.ron` file or a save. `transparent`, like
/// `ItemId`, so both spell it as a bare quoted string.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ImplantId(pub String);

impl ImplantId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ImplantId {
    fn from(s: &str) -> Self {
        ImplantId(s.to_string())
    }
}

/// Absolute deltas over the axes `Game::recompute_derived` writes, plus the
/// accuracy/evasion pair gear carries. Negative is legal: it is how a stat
/// penalty is written.
#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct ImplantStats {
    #[serde(default)]
    pub max_hp: i32,
    #[serde(default)]
    pub atk: i32,
    #[serde(default)]
    pub mitigation: i32,
    #[serde(default)]
    pub max_power: f32,
    #[serde(default)]
    pub crit: f64,
    #[serde(default)]
    pub status_resist: i32,
    #[serde(default)]
    pub decompiler: i32,
    /// Read live, never baked into `Stats` — see `EquipmentStats::accuracy`.
    #[serde(default)]
    pub accuracy: i32,
    /// See `accuracy`.
    #[serde(default)]
    pub evasion: i32,
}

impl ImplantStats {
    pub(crate) fn is_finite(&self) -> bool {
        self.max_power.is_finite() && self.crit.is_finite()
    }
}

/// An effect that is not a stat, each read at one existing seam. Percentages
/// are whole percentage points.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub enum ImplantHook {
    /// Added to the decompile capture chance.
    CaptureOdds(i32),
    /// Added to the equipment drop chance. Applies only in the Stack.
    DropBoost(i32),
    /// Extra player routine slots, past the slot cap.
    RoutineSlots(u32),
    /// Trace rises this much more slowly.
    TraceDamp(i32),
    /// Added to the XP the player earns.
    XpBoost(i32),
}

/// Behaviour the hook list cannot express, one named query per variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ImplantSignature {
    /// Once per battle, a lethal hit leaves the player at 1 HP and costs
    /// `tuning::DEAD_MANS_SWITCH_POWER`.
    DeadMansSwitch,
}

/// What an implant costs beyond Load and upkeep. Optional per implant.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum ImplantDownside {
    /// Trace rises this many percent faster.
    TraceRise(i32),
    /// Rolled at battle start: arms the status with this 0.0-1.0 chance.
    BattleStartStatus(StatusId, f32),
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ImplantDef {
    pub id: ImplantId,
    pub name: String,
    #[serde(default)]
    pub description: String,
    /// Neural Load this implant counts against the cap.
    pub load: u32,
    #[serde(default)]
    pub stats: ImplantStats,
    #[serde(default)]
    pub hooks: Vec<ImplantHook>,
    #[serde(default)]
    pub signature: Option<ImplantSignature>,
    #[serde(default)]
    pub downside: Option<ImplantDownside>,
}

impl ImplantDef {
    /// Why this def must not load, or `None` if it is sound.
    fn problem(&self) -> Option<String> {
        if !self.stats.is_finite() {
            return Some("a stat is not a finite number".to_string());
        }
        match &self.downside {
            Some(ImplantDownside::BattleStartStatus(_, chance))
                if !(0.0..=1.0).contains(chance) =>
            {
                Some(format!(
                    "BattleStartStatus chance {chance} is outside 0.0-1.0"
                ))
            }
            _ => None,
        }
    }
}

/// Every implant the game knows, loaded from `assets/implants/`.
#[derive(Resource, Default, Clone)]
pub struct ImplantDb {
    defs: HashMap<ImplantId, ImplantDef>,
}

impl ImplantDb {
    /// Loads every `*.ron` def in `dir`, in sorted file order so a duplicate
    /// id resolves the same way every run: the first file wins and the rest
    /// are warned about. A malformed file is skipped with a warning.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let mut db = ImplantDb::default();
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok((db, warnings)),
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            match ron::from_str::<ImplantDef>(&text) {
                Ok(def) if db.defs.contains_key(&def.id) => warnings.push(format!(
                    "skipped duplicate implant {:?} in {path:?}: an earlier file already defines it",
                    def.id.0
                )),
                Ok(def) => match def.problem() {
                    Some(why) => warnings.push(format!(
                        "skipped implant {:?} in {path:?}: {why}",
                        def.id.0
                    )),
                    None => {
                        db.defs.insert(def.id.clone(), def);
                    }
                },
                Err(e) => warnings.push(format!("skipped invalid implant file {path:?}: {e}")),
            }
        }
        Ok((db, warnings))
    }

    /// Adds or replaces a def, so a test can carry an implant no shipped file
    /// defines.
    #[cfg(test)]
    pub(crate) fn insert(&mut self, def: ImplantDef) {
        self.defs.insert(def.id.clone(), def);
    }

    pub fn get(&self, id: &ImplantId) -> Option<&ImplantDef> {
        self.defs.get(id)
    }

    pub fn len(&self) -> usize {
        self.defs.len()
    }

    pub fn is_empty(&self) -> bool {
        self.defs.is_empty()
    }

    /// Every id, sorted, for the census over shipped content.
    pub fn ids(&self) -> Vec<&ImplantId> {
        let mut ids: Vec<_> = self.defs.keys().collect();
        ids.sort();
        ids
    }
}

/// The Neural Load of everything installed. An id with no def counts nothing.
pub fn load_of(implants: &Implants, db: &ImplantDb) -> u32 {
    implants
        .installed
        .iter()
        .filter_map(|id| db.get(id))
        .map(|def| def.load)
        .sum()
}

/// How much Load a player of `level` can carry before overloading.
pub fn load_cap(level: u32) -> u32 {
    crate::tuning::IMPLANT_LOAD_BASE + level / crate::tuning::IMPLANT_LOAD_LEVELS_PER_POINT
}

/// Load past the cap, or 0.
pub fn overload(load: u32, cap: u32) -> u32 {
    load.saturating_sub(cap)
}

/// The multiplier on Power drain that `load` installed Load adds.
pub fn drain_factor(load: u32) -> f32 {
    1.0 + load as f32 * crate::tuning::IMPLANT_DRAIN_PER_LOAD
}

/// The whole multiplier on `HUNGER_DECAY_PER_TICK`: the perk reduction times
/// the implant factor, so `LowPowerMode` reduces implant upkeep too.
pub fn power_multiplier(perks: Option<&crate::components::Perks>, load: u32) -> f32 {
    crate::perks::power_drain_multiplier(perks) * drain_factor(load)
}

/// Chance a battle start arms a rejection status at `overload`: none at 0,
/// then linear up to `REJECTION_CHANCE_MAX`.
pub fn rejection_chance(overload: u32) -> f64 {
    (overload as f64 * crate::tuning::REJECTION_CHANCE_PER_LOAD)
        .min(crate::tuning::REJECTION_CHANCE_MAX)
}

/// `amount` of Trace after a net `net_pct` percent change (positive from
/// `TraceRise`, negative from `TraceDamp`), floored at 1 whenever there was
/// anything to scale, so a damp can slow Trace but never stop it. Applied
/// before `perks::trace_after_obfuscation`, whose own floor still holds.
pub fn trace_scaled(amount: u32, net_pct: i32) -> u32 {
    if amount == 0 || net_pct == 0 {
        return amount;
    }
    let scaled = (amount as f64 * (100.0 + net_pct as f64) / 100.0).round();
    (scaled.max(1.0)) as u32
}

#[cfg(test)]
mod tests {
    use super::*;

    fn def_text(id: &str) -> String {
        format!("(id: \"{id}\", name: \"Test\", load: 2, stats: (atk: 3))")
    }

    fn load(scratch: &str, files: &[(&str, String)]) -> (ImplantDb, Vec<String>) {
        let dir = crate::tests::support::scratch_assets_dir(scratch);
        std::fs::create_dir_all(&*dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        ImplantDb::load_dir(&dir).unwrap()
    }

    fn db_with(defs: &[(&str, u32)]) -> ImplantDb {
        let mut db = ImplantDb::default();
        for (id, load) in defs {
            db.insert(ImplantDef {
                id: ImplantId::from(*id),
                name: (*id).to_string(),
                description: String::new(),
                load: *load,
                stats: ImplantStats::default(),
                hooks: Vec::new(),
                signature: None,
                downside: None,
            });
        }
        db
    }

    fn installed(ids: &[&str]) -> Implants {
        Implants {
            installed: ids.iter().map(|id| ImplantId::from(*id)).collect(),
        }
    }

    #[test]
    fn load_sums_installed_and_skips_an_id_with_no_def() {
        let db = db_with(&[("a", 2), ("b", 3)]);
        assert_eq!(load_of(&installed(&[]), &db), 0);
        assert_eq!(load_of(&installed(&["a", "b"]), &db), 5);
        assert_eq!(load_of(&installed(&["a", "gone"]), &db), 2);
    }

    #[test]
    fn the_cap_starts_at_base_and_reaches_sixteen_mid_band() {
        assert_eq!(load_cap(1), crate::tuning::IMPLANT_LOAD_BASE);
        assert!(load_cap(12) > load_cap(1));
        let reaches = (1..=100).find(|&l| load_cap(l) >= 16).unwrap();
        let (low, high) = (
            crate::tuning::zone_level_cap(5),
            crate::tuning::zone_level_cap(6),
        );
        assert!((low..=high).contains(&reaches), "reached at {reaches}");
    }

    #[test]
    fn overload_is_the_excess_and_never_negative() {
        assert_eq!(overload(5, 8), 0);
        assert_eq!(overload(8, 8), 0);
        assert_eq!(overload(11, 8), 3);
    }

    #[test]
    fn drain_factor_is_one_with_no_load_and_rises_with_it() {
        assert_eq!(drain_factor(0), 1.0);
        assert!(drain_factor(4) > drain_factor(2));
        let expected = 1.0 + 4.0 * crate::tuning::IMPLANT_DRAIN_PER_LOAD;
        assert!((drain_factor(4) - expected).abs() < 1e-6);
    }

    #[test]
    fn power_multiplier_is_the_perk_multiplier_times_the_factor() {
        let perks = crate::components::Perks {
            points: 0,
            unlocked: vec![crate::perks::Perk::LowPowerMode; 10],
        };
        assert_eq!(power_multiplier(None, 0), 1.0);
        let m = power_multiplier(Some(&perks), 6);
        let expected = crate::perks::power_drain_multiplier(Some(&perks)) * drain_factor(6);
        assert!((m - expected).abs() < 1e-6);
        assert!(m < power_multiplier(None, 6), "LowPowerMode reduces upkeep");
    }

    #[test]
    fn rejection_chance_is_zero_at_zero_rises_and_is_capped() {
        assert_eq!(rejection_chance(0), 0.0);
        assert!(rejection_chance(2) > rejection_chance(1));
        assert_eq!(rejection_chance(1000), crate::tuning::REJECTION_CHANCE_MAX);
    }

    #[test]
    fn trace_scaled_rises_and_damps_but_never_zeroes() {
        assert_eq!(trace_scaled(10, 0), 10);
        assert_eq!(trace_scaled(10, 50), 15);
        assert_eq!(trace_scaled(10, -30), 7);
        assert_eq!(
            trace_scaled(10, -100),
            1,
            "a damp slows Trace, never stops it"
        );
        assert_eq!(trace_scaled(0, 50), 0);
        assert_eq!(trace_scaled(1, -50), 1);
    }

    #[test]
    fn a_def_file_loads_with_every_optional_field_defaulted() {
        let (db, warnings) = load("implant_ok", &[("a.ron", def_text("a"))]);
        assert!(warnings.is_empty(), "{warnings:?}");
        let def = db.get(&ImplantId::from("a")).unwrap();
        assert_eq!(def.load, 2);
        assert_eq!(def.stats.atk, 3);
        assert_eq!(def.stats.max_hp, 0);
        assert!(def.hooks.is_empty() && def.signature.is_none() && def.downside.is_none());
    }

    #[test]
    fn a_malformed_file_is_skipped_with_a_warning() {
        let (db, warnings) = load(
            "implant_bad",
            &[("a.ron", def_text("a")), ("b.ron", "(id: ".to_string())],
        );
        assert_eq!(db.len(), 1);
        assert_eq!(warnings.len(), 1);
        assert!(warnings[0].contains("b.ron"), "{warnings:?}");
    }

    #[test]
    fn a_duplicate_id_keeps_the_first_file_and_warns() {
        let (db, warnings) = load(
            "implant_dup",
            &[("a.ron", def_text("x")), ("b.ron", def_text("x"))],
        );
        assert_eq!(db.len(), 1);
        assert!(warnings[0].contains("duplicate"), "{warnings:?}");
    }

    #[test]
    fn an_out_of_range_status_chance_is_skipped() {
        let text = "(id: \"x\", name: \"X\", load: 1, \
                    downside: Some(BattleStartStatus(\"stun\", 1.5)))";
        let (db, warnings) = load("implant_chance", &[("x.ron", text.to_string())]);
        assert!(db.is_empty());
        assert!(warnings[0].contains("chance"), "{warnings:?}");
    }

    #[test]
    fn a_missing_directory_gives_an_empty_db() {
        let dir = crate::tests::support::scratch_assets_dir("implant_none");
        let (db, warnings) = ImplantDb::load_dir(&dir.join("nope")).unwrap();
        assert!(db.is_empty() && warnings.is_empty());
    }
}
