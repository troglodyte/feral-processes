use serde::{Deserialize, Serialize};

/// `#[serde(transparent)]` so an `ItemId` serializes as its bare inner string
/// rather than as a `ItemId("...")` tuple-struct — the RON asset files spell
/// item references as plain quoted strings (e.g. `work_resource: Some("power_cell")`),
/// and bincode saves encode it identically to a `String`.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ItemId(pub String);

impl ItemId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl From<&str> for ItemId {
    fn from(s: &str) -> Self {
        ItemId(s.to_string())
    }
}

impl From<String> for ItemId {
    fn from(s: String) -> Self {
        ItemId(s)
    }
}

impl std::fmt::Display for ItemId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

/// Canonical ids of the shipped items. Used by test setup and data-defined
/// recipes for readability — never by engine *logic*, which goes through
/// economy roles and `ItemDef` fields.
pub mod ids {
    pub const CORE_FRAGMENT: &str = "core_fragment";
    pub const POWER_CELL: &str = "power_cell";
    pub const ICE_BREAKER: &str = "ice_breaker";
    pub const OVERCLOCK_CORE: &str = "overclock_core";
    pub const FIREWALL_PLATING: &str = "firewall_plating";
    pub const NEURAL_AMPLIFIER: &str = "neural_amplifier";
    pub const PORTAL_FRAGMENT: &str = "portal_fragment";
    pub const RESEARCH_DATA: &str = "research_data";
    pub const MONOFILAMENT_WHIP: &str = "monofilament_whip";
    pub const ABLATIVE_PLATING: &str = "ablative_plating";
    pub const CORTEX_HACK: &str = "cortex_hack";
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum EquipmentSlot {
    Weapon,
    Armor,
    Module,
}

impl EquipmentSlot {
    pub fn label(self) -> &'static str {
        match self {
            EquipmentSlot::Weapon => "Weapon",
            EquipmentSlot::Armor => "Armor",
            EquipmentSlot::Module => "Module",
        }
    }
}

/// Flat stat bonuses an equipped item grants while worn, at gear level 1
/// (base). See `GEAR_LEVEL_GROWTH`/`EquipmentStats::scaled_for_level` for
/// how a higher gear level scales these up.
#[derive(Clone, Copy, Debug, Default, Serialize, Deserialize)]
pub struct EquipmentStats {
    #[serde(default)]
    pub atk: i32,
    #[serde(default)]
    pub def: i32,
    #[serde(default)]
    pub decompiler: i32,
}

/// Growth factor applied to an item's base `EquipmentStats` per gear level
/// above 1 — doubles each level (level *N* = base *
/// `GEAR_LEVEL_GROWTH.powi(N - 1)`), matching `ZoneLevel::stat_multiplier`'s
/// own per-zone doubling so neither leveling nor gear dominates the other
/// outright — see `balance::best_case_gear_bonus`'s tests for the
/// simulation that surfaced the old 2.5x growth overtaking it. Gear level
/// is capped by `resources::ZoneLevel`: reaching zone *N* is what
/// "unlocks" level *N* gear — see `Game::equip`.
pub const GEAR_LEVEL_GROWTH: f64 = 2.0;

/// Bonus `Game::fuse_item` adds to an item type's equipped stats, per
/// fusion tier — additive, not compounding (tier 2 is +20%, not +21%).
pub const ITEM_FUSION_BONUS_PER_TIER: f64 = 0.10;

/// Copies of an item `Game::fuse_item` consumes from inventory per fusion.
pub const ITEM_FUSION_COST: u32 = 2;

impl EquipmentStats {
    /// This item's bonus scaled up for `level` (1 = base, no scaling).
    /// Each component is rounded independently to the nearest whole point.
    pub fn scaled_for_level(self, level: u32) -> EquipmentStats {
        let factor = GEAR_LEVEL_GROWTH.powi(level.max(1) as i32 - 1);
        let scale = |v: i32| (v as f64 * factor).round() as i32;
        EquipmentStats {
            atk: scale(self.atk),
            def: scale(self.def),
            decompiler: scale(self.decompiler),
        }
    }

    /// This item's bonus scaled up for `tier` fusions (0 = base, no
    /// scaling) — see `ITEM_FUSION_BONUS_PER_TIER`. Applied on top of
    /// `scaled_for_level`, not in place of it.
    pub fn fused_for_tier(self, tier: u32) -> EquipmentStats {
        let factor = 1.0 + ITEM_FUSION_BONUS_PER_TIER * tier as f64;
        let scale = |v: i32| (v as f64 * factor).round() as i32;
        EquipmentStats {
            atk: scale(self.atk),
            def: scale(self.def),
            decompiler: scale(self.decompiler),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scaled_for_level_grows_100_percent_per_level_above_1() {
        let base = EquipmentStats {
            atk: 4,
            def: 0,
            decompiler: 0,
        };
        assert_eq!(
            base.scaled_for_level(1).atk,
            4,
            "level 1 should be unscaled base"
        );
        assert_eq!(
            base.scaled_for_level(2).atk,
            8,
            "level 2 should be 2x base (4 * 2 = 8)"
        );
        assert_eq!(
            base.scaled_for_level(3).atk,
            16,
            "level 3 should be 2x level 2 (8 * 2 = 16)"
        );
        assert_eq!(
            base.scaled_for_level(0).atk,
            4,
            "level 0 should clamp to level 1's unscaled base"
        );
    }

    #[test]
    fn equipment_stats_round_trip_ron_with_omitted_zero_fields() {
        let full: EquipmentStats = ron::from_str("(atk: 3, def: 0, decompiler: 0)").unwrap();
        assert_eq!((full.atk, full.def, full.decompiler), (3, 0, 0));
        // Zero fields may be omitted thanks to per-field serde defaults.
        let partial: EquipmentStats = ron::from_str("(atk: 4)").unwrap();
        assert_eq!((partial.atk, partial.def, partial.decompiler), (4, 0, 0));
    }
}
