use crate::components::Rarity;
use crate::tuning::{
    GEAR_LEVEL_STEP, GEAR_RARITY_MIN_BONUS_PER_RUNG, ITEM_FUSION_BONUS_PER_TIER,
    ITEM_FUSION_MIN_BONUS_PER_TIER,
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

    /// The id of the etched Routine Disk holding `ability` — the item
    /// `ItemDb::synthesise_etched_disks` derives for it.
    ///
    /// One function rather than a `format!` at each site because this id is
    /// the seam between two databases: the ability half spells it when a
    /// boss drop or a market row is built, and the item half spells it when
    /// the disk is derived. A prefix typed twice is a disk nothing can ever
    /// install.
    ///
    /// The `etched_` prefix could in principle collide with a modder's own
    /// item — `ItemDb::synthesise_etched_disks` refuses to overwrite one and
    /// warns instead, which is the same call `load_dir` makes about a
    /// duplicated economy role.
    pub fn etched(ability: &str) -> Self {
        ItemId(format!("{ETCHED_DISK_PREFIX}{ability}"))
    }

    /// The ability burnt onto this disk, or `None` if this is not an etched
    /// disk at all. The inverse of `etched`, and the reason both live here:
    /// a round trip that does not close is a disk that installs the wrong
    /// routine.
    pub fn etched_ability(&self) -> Option<&str> {
        self.0.strip_prefix(ETCHED_DISK_PREFIX)
    }
}

/// What `ItemId::etched` puts in front of an ability id. Not in `ids`
/// because it is not an id — it is the scheme every etched disk's id is
/// built from, and `ids` is a list of specific shipped items.
pub const ETCHED_DISK_PREFIX: &str = "etched_";

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
    /// Every slot, in the order the equipment panel and the companion slot
    /// page list them. One definition, so a fourth slot cannot reach some
    /// screens and miss others — or, worse, be missed by `Game::gear_bonus`
    /// and `Game::strip_gear`, where a skipped slot is a bonus welded into
    /// the wearer's base stats.
    pub const ALL: [EquipmentSlot; 3] = [
        EquipmentSlot::Weapon,
        EquipmentSlot::Armor,
        EquipmentSlot::Module,
    ];

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
/// (base). See `GEAR_LEVEL_STEP`/`EquipmentStats::scaled_for_level` for
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
        let factor = 1.0 + GEAR_LEVEL_STEP * (level.max(1) as f64 - 1.0);
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

    /// This item's bonus scaled up for the rare tier the *copy* rolled when
    /// it dropped (`Ordinary` = base, no scaling) — see `components::Rarity`
    /// and `Game::grant_gear_drop`. Applied on top of the other two, not in
    /// place of either.
    ///
    /// Deliberately the same shape as `fused_for_tier`, floor and all, and
    /// for the same reason: the percentage alone is worthless at the
    /// magnitudes equipment ships at, so `GEAR_RARITY_MIN_BONUS_PER_RUNG`
    /// is what makes a tier observable rather than a colour that changes no
    /// number. A stat sitting at zero stays at zero.
    ///
    /// The multiplier is shared with programs (`Rarity::stat_mult`) rather
    /// than being a second gear-only ladder, so one retune moves the word,
    /// the colour and both sets of numbers together.
    ///
    /// **Why this walks the rungs instead of applying one multiplier.** A
    /// creature's stats are in the tens or hundreds, where a 1.5x and a 1.8x
    /// are plainly different numbers. Gear ships at 1..=4 points, where they
    /// are frequently the *same* number: `round(3 * 1.5)` and
    /// `round(3 * 1.8)` are both 5, so a single-multiplier form makes an
    /// Overclocked weapon identical to an Optimized one on every base-3 stat
    /// in the game — a different colour, a different word, and no difference
    /// the player can act on. Walking the ladder and taking
    /// `max(multiplier, previous_rung + GEAR_RARITY_MIN_BONUS_PER_RUNG)`
    /// makes every rung strictly better than the one below it *by
    /// construction*, at every base value, which is the property
    /// `every_rare_tier_is_worth_more_than_the_one_below_it` pins.
    ///
    /// A stat sitting at zero stays at zero — a tier sharpens what an item
    /// does and does not hand it a stat it never had.
    pub fn for_rarity(self, rarity: Rarity) -> EquipmentStats {
        let scale = |base: i32| {
            if base <= 0 {
                return base;
            }
            (1..=rarity.rank()).fold(base, |value, rank| {
                let by_multiplier =
                    (base as f64 * Rarity::ALL[rank as usize].stat_mult() as f64).round() as i32;
                by_multiplier.max(value + GEAR_RARITY_MIN_BONUS_PER_RUNG)
            })
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
    fn scaled_for_level_adds_100_percent_of_base_per_level_above_1() {
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
            "level 2 should be base + 100% of base (4 * 2 = 8)"
        );
        assert_eq!(
            base.scaled_for_level(3).atk,
            12,
            "level 3 adds another 100% of base, not another 100% of level 2 \
             (4 * 3 = 12, not 16) — gear tracks a linear zone curve"
        );
        assert_eq!(
            base.scaled_for_level(0).atk,
            4,
            "level 0 should clamp to level 1's unscaled base"
        );
    }

    #[test]
    fn an_ordinary_tier_leaves_an_item_exactly_as_it_was() {
        let base = EquipmentStats {
            atk: 3,
            def: 1,
            decompiler: 2,
        };
        let same = base.for_rarity(Rarity::Ordinary);
        assert_eq!((same.atk, same.def, same.decompiler), (3, 1, 2));
    }

    /// Every rung has to move every stat the item actually has. The floor
    /// exists precisely because the percentage alone does not guarantee it
    /// at the magnitudes gear ships at (1..=4 a stat).
    #[test]
    fn every_rare_tier_is_worth_more_than_the_one_below_it() {
        // Every magnitude shipped gear actually uses. 3 is the one that
        // caught the original single-multiplier form: round(3 * 1.5) and
        // round(3 * 1.8) are both 5.
        for stat in [1, 2, 3, 4] {
            let base = EquipmentStats {
                atk: stat,
                ..Default::default()
            };
            for pair in Rarity::ALL.windows(2) {
                let lower = base.for_rarity(pair[0]).atk;
                let upper = base.for_rarity(pair[1]).atk;
                assert!(
                    upper > lower,
                    "at base {stat}, {:?} ({upper}) must beat {:?} ({lower})",
                    pair[1],
                    pair[0]
                );
            }
        }
    }

    /// A tier sharpens what an item does; it does not hand it a stat it
    /// never had. Same rule `fused_for_tier` follows, and the reason the
    /// floor is guarded on `v > 0`.
    #[test]
    fn a_rare_tier_never_invents_a_stat_the_item_lacks() {
        let weapon_only = EquipmentStats {
            atk: 3,
            ..Default::default()
        };
        for tier in Rarity::ALL {
            let scaled = weapon_only.for_rarity(tier);
            assert_eq!(scaled.def, 0, "{tier:?} invented a DEF bonus");
            assert_eq!(scaled.decompiler, 0, "{tier:?} invented a DECOMP bonus");
        }
    }

    /// **The three axes do not commute, so the order `Game::gear_bonus`
    /// applies them in is load-bearing rather than stylistic.**
    ///
    /// Two of them carry a per-step floor, and a floor is not commutative
    /// with a multiplier: applying rarity to a base 3 and then levelling
    /// gives a different number from levelling first and then applying
    /// rarity, because the floor is measured against whatever it is handed.
    /// That is fine — one order is canonical — but it means a call site that
    /// reorders the chain silently changes the stat, and an unequip that
    /// reordered it relative to its equip would leave the difference welded
    /// into the wearer's base `Stats` with no record of where it came from.
    ///
    /// This asserts the asymmetry exists so nobody "tidies" the chain on the
    /// assumption that it can't matter.
    #[test]
    fn the_gear_axes_do_not_commute_so_the_order_is_load_bearing() {
        let base = EquipmentStats {
            atk: 3,
            def: 2,
            decompiler: 0,
        };
        let canonical = base
            .scaled_for_level(3)
            .fused_for_tier(2)
            .for_rarity(Rarity::Gold);
        let reordered = base
            .for_rarity(Rarity::Gold)
            .scaled_for_level(3)
            .fused_for_tier(2);
        assert_ne!(
            (canonical.atk, canonical.def),
            (reordered.atk, reordered.def),
            "if these ever agree, this test has stopped protecting anything — \
             check whether a floor was removed from one of the three axes"
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
