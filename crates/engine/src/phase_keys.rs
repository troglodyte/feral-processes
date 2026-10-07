//! Phase Keys, loaded from `assets/phase_keys/`.
//!
//! Ten keys, one per zone 1..=10, each a permanent benefit held from the
//! moment it is found. The slot count and the rules around it (drops, the
//! portal gate, the exit) are code; what a key *is* — name, flavour, effect —
//! is data. **The database always holds exactly one def per slot**: a missing,
//! malformed, out-of-range or duplicate file falls back to a plain "Phase Key
//! N" with no effect, so a mod can change a key but never make the game
//! uncompletable.
//!
//! The effect vocabulary is the implants' (`ImplantStats`, `ImplantHook`,
//! `ImplantSignature`) plus [`StatPct`], whole-percent bonuses on the
//! magnitude stats. Keys ride the implant readers in `game/implants.rs`; see
//! `assets/phase_keys/README.md` for the schema.

use crate::implants::{ImplantHook, ImplantSignature, ImplantStats};
use bevy_ecs::prelude::Resource;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Whole-percent bonuses on the player's final magnitude stats. Percents from
/// every held key sum before they apply.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct StatPct {
    #[serde(default)]
    pub max_hp: i32,
    #[serde(default)]
    pub atk: i32,
    #[serde(default)]
    pub mitigation: i32,
    #[serde(default)]
    pub max_power: i32,
    #[serde(default)]
    pub decompiler: i32,
}

impl StatPct {
    pub fn sum(self, other: StatPct) -> StatPct {
        StatPct {
            max_hp: self.max_hp + other.max_hp,
            atk: self.atk + other.atk,
            mitigation: self.mitigation + other.mitigation,
            max_power: self.max_power + other.max_power,
            decompiler: self.decompiler + other.decompiler,
        }
    }

    fn is_negative(&self) -> bool {
        [
            self.max_hp,
            self.atk,
            self.mitigation,
            self.max_power,
            self.decompiler,
        ]
        .iter()
        .any(|pct| *pct < 0)
    }
}

/// `value` raised by `pct` whole percent, by at least 1 whenever `pct` is
/// positive so a small stat still feels a held key. **The one formula**: every
/// site that forms a final stat calls this and none restates it.
pub fn apply_key_pct(value: i32, pct: i32) -> i32 {
    if pct <= 0 {
        return value;
    }
    value + (value * pct / 100).max(1)
}

/// `apply_key_pct` for the float Power maximum: the figure rounds to a whole
/// point only when a key actually scales it.
pub fn key_scaled_power(max_power: f32, pct: i32) -> f32 {
    if pct <= 0 {
        return max_power;
    }
    apply_key_pct(max_power.round() as i32, pct) as f32
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct KeyEffect {
    #[serde(default)]
    pub stat_pct: StatPct,
    #[serde(default)]
    pub stats: ImplantStats,
    #[serde(default)]
    pub hooks: Vec<ImplantHook>,
    #[serde(default)]
    pub signature: Option<ImplantSignature>,
}

impl KeyEffect {
    /// The effect as one line for the Phase Keys tab and the pickup alert.
    /// Empty for a key that does nothing.
    pub fn summary(&self) -> String {
        let mut parts: Vec<String> = Vec::new();
        let pct = &self.stat_pct;
        for (value, label) in [
            (pct.max_hp, "Integrity"),
            (pct.atk, "attack"),
            (pct.mitigation, "mitigation"),
            (pct.max_power, "max Power"),
            (pct.decompiler, "decompiler"),
        ] {
            if value != 0 {
                parts.push(format!("{value:+}% {label}"));
            }
        }
        let s = &self.stats;
        for (value, label) in [
            (s.max_hp, "Integrity"),
            (s.atk, "attack"),
            (s.mitigation, "mitigation"),
            (s.status_resist, "status resist"),
            (s.decompiler, "decompiler"),
            (s.accuracy, "accuracy"),
            (s.evasion, "evasion"),
        ] {
            if value != 0 {
                parts.push(format!("{value:+} {label}"));
            }
        }
        if s.max_power != 0.0 {
            parts.push(format!("{:+} max Power", s.max_power.round()));
        }
        if s.crit != 0.0 {
            parts.push(format!("{:+}% crit", (s.crit * 100.0).round()));
        }
        for hook in &self.hooks {
            parts.push(match hook {
                ImplantHook::CaptureOdds(pct) => format!("{pct:+}% decompile odds"),
                ImplantHook::DropBoost(pct) => format!("{pct:+}% Stack gear drops"),
                ImplantHook::RoutineSlots(n) => format!("{n:+} routine slot"),
                ImplantHook::TraceDamp(pct) => format!("Trace rises {pct}% slower"),
                ImplantHook::XpBoost(pct) => format!("{pct:+}% XP"),
            });
        }
        if let Some(ImplantSignature::DeadMansSwitch) = self.signature {
            parts.push("Dead Man's Switch".to_string());
        }
        parts.join(", ")
    }

    fn problem(&self) -> Option<String> {
        if !self.stats.is_finite() {
            return Some("a stat is not a finite number".to_string());
        }
        if self.stat_pct.is_negative() {
            return Some("a stat_pct is negative".to_string());
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PhaseKeyDef {
    pub zone: u32,
    pub name: String,
    #[serde(default)]
    pub flavour: String,
    #[serde(default)]
    pub effect: KeyEffect,
}

impl PhaseKeyDef {
    fn fallback(zone: u32) -> Self {
        PhaseKeyDef {
            zone,
            name: format!("Phase Key {zone}"),
            flavour: String::new(),
            effect: KeyEffect::default(),
        }
    }
}

/// Every key the game knows: exactly `PHASE_KEY_COUNT` defs, slot `zone - 1`.
#[derive(Resource, Clone)]
pub struct PhaseKeyDb {
    defs: Vec<PhaseKeyDef>,
    /// The ending the Basin Exit plays. It rides this resource instead of
    /// being one of its own because a resource registered at construction
    /// shifts every later `ComponentId` and with them query iteration order,
    /// which flips seed-luck tests nowhere near this feature. Set by
    /// `load_asset_dbs`.
    pub(crate) ending: crate::story::EndingText,
}

impl Default for PhaseKeyDb {
    fn default() -> Self {
        PhaseKeyDb {
            defs: (1..=crate::tuning::PHASE_KEY_COUNT)
                .map(PhaseKeyDef::fallback)
                .collect(),
            ending: crate::story::EndingText::default(),
        }
    }
}

impl PhaseKeyDb {
    /// Loads every `*.ron` def in `dir` in sorted file order, so a duplicate
    /// zone resolves the same way every run: the first file wins. Then every
    /// empty slot takes a fallback. A missing directory is silent (every
    /// other database's rule) and yields all fallbacks; a directory that
    /// exists but leaves a slot empty warns once per gap.
    pub fn load_dir(dir: &Path) -> std::io::Result<(Self, Vec<String>)> {
        let count = crate::tuning::PHASE_KEY_COUNT;
        let mut slots: Vec<Option<PhaseKeyDef>> = vec![None; count as usize];
        let mut warnings = Vec::new();
        let entries = match std::fs::read_dir(dir) {
            Ok(entries) => entries,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                return Ok((PhaseKeyDb::default(), warnings));
            }
            Err(e) => return Err(e),
        };
        let mut paths: Vec<_> = entries
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("ron"))
            .collect();
        paths.sort();
        for path in paths {
            let text = std::fs::read_to_string(&path)?;
            let def = match ron::from_str::<PhaseKeyDef>(&text) {
                Ok(def) => def,
                Err(e) => {
                    warnings.push(format!("skipped invalid phase key file {path:?}: {e}"));
                    continue;
                }
            };
            if !(1..=count).contains(&def.zone) {
                warnings.push(format!(
                    "skipped phase key {:?} in {path:?}: zone {} is outside 1..={count}",
                    def.name, def.zone
                ));
            } else if let Some(why) = def.effect.problem() {
                warnings.push(format!(
                    "skipped phase key {:?} in {path:?}: {why}",
                    def.name
                ));
            } else if slots[def.zone as usize - 1].is_some() {
                warnings.push(format!(
                    "skipped duplicate phase key for zone {} in {path:?}: an earlier file already defines it",
                    def.zone
                ));
            } else {
                let slot = def.zone as usize - 1;
                slots[slot] = Some(def);
            }
        }
        let defs = slots
            .into_iter()
            .zip(1..=count)
            .map(|(slot, zone)| {
                slot.unwrap_or_else(|| {
                    warnings.push(format!(
                        "no phase key defined for zone {zone}: using a plain Phase Key {zone} with no effect"
                    ));
                    PhaseKeyDef::fallback(zone)
                })
            })
            .collect();
        Ok((
            PhaseKeyDb {
                defs,
                ending: crate::story::EndingText::default(),
            },
            warnings,
        ))
    }

    /// The key for `zone`; `None` outside 1..=`PHASE_KEY_COUNT`.
    pub fn get(&self, zone: u32) -> Option<&PhaseKeyDef> {
        self.defs.get(zone.checked_sub(1)? as usize)
    }

    pub fn all(&self) -> &[PhaseKeyDef] {
        &self.defs
    }

    /// Replaces a slot, so a test can carry a key no shipped file defines.
    #[cfg(test)]
    pub(crate) fn set(&mut self, def: PhaseKeyDef) {
        let slot = def.zone as usize - 1;
        self.defs[slot] = def;
    }
}
