use crate::tuning::{
    GEAR_LEVEL_GROWTH, ITEM_FUSION_BONUS_PER_TIER, ITEM_FUSION_MIN_BONUS_PER_TIER,
};
use serde::{Deserialize, Serialize};

/// `#[serde(transparent)]` so an `ItemId` serializes as its bare inner string
/// rather than as a `ItemId("...")` tuple-struct — the RON asset files spell
/// item references as plain quoted strings (e.g. `work_resource: Some("power_cell")`),
/// and bincode saves encode it identically to a `String`.
/// `Ord` so `components::Stock` can key its buffers by item in a
/// `BTreeMap`: iteration order there feeds the production-chain pull phase
/// and the save encoding, both of which have to be identical run to run.
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
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
    pub const CREDITS: &str = "credits";
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
    /// Dormant. It was what a sealed Stack door cost until the seal became
    /// something the party simply shoulders open (`Game::force_seal`), and
    /// nothing spends one now — the id is kept, and the item still ships,
    /// against deciding what it is for instead.
    pub const ACCESS_SHARD: &str = "access_shard";
    /// Burnt to install a routine the player knows — see
    /// `Game::install_routine`. Named from Rust for the same reason
    /// `ACCESS_SHARD` is: what installing costs is engine content, not a
    /// data-driven requirement. The item, and the whole chain that makes it,
    /// are still ordinary `.ron` files.
    pub const ROUTINE_DISK: &str = "routine_disk";
    /// Named from Rust because the starting inventory is engine content
    /// (see `game/lifecycle.rs`) — same reason `CORE_FRAGMENT`, `POWER_CELL`
    /// and `ICE_BREAKER` are, on the same lines; the id itself lives in
    /// `assets/items/outlet.ron`.
    pub const OUTLET: &str = "outlet";
    /// The shipped production chain, named here only so the tests that walk
    /// it can spell it. Nothing in the engine references these: what each
    /// machine builds is authored in `assets/structures/*.ron`, and each
    /// recipe in the item's own file.
    pub const BYTECODE_BLOCK: &str = "bytecode_block";
    pub const CHARGE_COIL: &str = "charge_coil";
    pub const PATCH_ROUTINE: &str = "patch_routine";
}

/// What kind of thing an item is, for grouping the inventory and a trader's
/// list. Derived from the fields an `ItemDef` already declares — see
/// `ItemDef::category` — rather than authored, so a modded item is grouped
/// without its author adding a field, and the grouping cannot drift out of
/// step with the behaviour it describes.
///
/// Declaration order is display order: what you spend, then what you wear,
/// then what you hoard.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum ItemCategory {
    Consumable,
    Weapon,
    Armor,
    Module,
    Material,
    Currency,
}

impl ItemCategory {
    /// Compact form for a list row, matching `EquipmentSlot::short_label`'s
    /// case — the two sit in the same column on the inventory screen.
    pub fn short_label(self) -> &'static str {
        match self {
            ItemCategory::Consumable => "USE",
            ItemCategory::Weapon => "WEP",
            ItemCategory::Armor => "ARM",
            ItemCategory::Module => "MOD",
            ItemCategory::Material => "MAT",
            ItemCategory::Currency => "CUR",
        }
    }
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

    /// Compact form for space-constrained rows — see the inventory list's
    /// equip tag, where it sits beside `ATK`/`DEF`/`DECOMP` and so matches
    /// their case. `label` stays the name for headers and prose.
    pub fn short_label(self) -> &'static str {
        match self {
            EquipmentSlot::Weapon => "WEP",
            EquipmentSlot::Armor => "ARM",
            EquipmentSlot::Module => "MOD",
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
    ///
    /// A stat the item already has gains at least
    /// `ITEM_FUSION_MIN_BONUS_PER_TIER` per tier, whatever the percentage
    /// works out to. The percentage alone is worthless at the magnitudes
    /// equipment actually ships at — 4 × 1.1 rounds straight back to 4 —
    /// so the floor is what makes a fusion observable rather than a
    /// silent loss of two items. A stat sitting at zero stays at zero: the
    /// floor sharpens what an item does and does not hand it a new stat.
    pub fn fused_for_tier(self, tier: u32) -> EquipmentStats {
        let factor = 1.0 + ITEM_FUSION_BONUS_PER_TIER * tier as f64;
        let floor = ITEM_FUSION_MIN_BONUS_PER_TIER * tier as i32;
        let scale = |v: i32| {
            let scaled = (v as f64 * factor).round() as i32;
            if v > 0 { scaled.max(v + floor) } else { scaled }
        };
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
