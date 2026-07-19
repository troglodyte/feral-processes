use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum ItemId {
    CoreFragment,
    PowerCell,
    IceBreaker,
    OverclockCore,
    FirewallPlating,
    NeuralAmplifier,
    PortalFragment,
    MonofilamentWhip,
    AblativePlating,
    CortexHack,
}

impl ItemId {
    pub fn display_name(self) -> &'static str {
        match self {
            ItemId::CoreFragment => "Core Fragment",
            ItemId::PowerCell => "Power Cell",
            ItemId::IceBreaker => "ICE Breaker",
            ItemId::OverclockCore => "Overclock Core",
            ItemId::FirewallPlating => "Firewall Plating",
            ItemId::NeuralAmplifier => "Neural Amplifier",
            ItemId::PortalFragment => "Portal Fragment",
            ItemId::MonofilamentWhip => "Monofilament Whip",
            ItemId::AblativePlating => "Ablative Plating",
            ItemId::CortexHack => "Cortex Hack",
        }
    }

    /// `Some((slot, bonus))` for equippable items, `None` for plain
    /// resources. The single source of truth for what makes an item gear.
    pub fn equipment(self) -> Option<(EquipmentSlot, EquipmentStats)> {
        match self {
            ItemId::OverclockCore => Some((
                EquipmentSlot::Weapon,
                EquipmentStats {
                    atk: 3,
                    ..EquipmentStats::default()
                },
            )),
            ItemId::MonofilamentWhip => Some((
                EquipmentSlot::Weapon,
                EquipmentStats {
                    atk: 4,
                    ..EquipmentStats::default()
                },
            )),
            ItemId::FirewallPlating => Some((
                EquipmentSlot::Armor,
                EquipmentStats {
                    def: 3,
                    ..EquipmentStats::default()
                },
            )),
            ItemId::AblativePlating => Some((
                EquipmentSlot::Armor,
                EquipmentStats {
                    def: 4,
                    ..EquipmentStats::default()
                },
            )),
            ItemId::NeuralAmplifier => Some((
                EquipmentSlot::Module,
                EquipmentStats {
                    decompiler: 2,
                    ..EquipmentStats::default()
                },
            )),
            ItemId::CortexHack => Some((
                EquipmentSlot::Module,
                EquipmentStats {
                    decompiler: 3,
                    ..EquipmentStats::default()
                },
            )),
            ItemId::CoreFragment
            | ItemId::PowerCell
            | ItemId::IceBreaker
            | ItemId::PortalFragment => None,
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
}

/// Flat stat bonuses an equipped item grants while worn, at gear level 1
/// (base). See `GEAR_LEVEL_GROWTH`/`EquipmentStats::scaled_for_level` for
/// how a higher gear level scales these up.
#[derive(Clone, Copy, Debug, Default)]
pub struct EquipmentStats {
    pub atk: i32,
    pub def: i32,
    pub decompiler: i32,
}

/// Growth factor applied to an item's base `EquipmentStats` per gear level
/// above 1 — "150% more than the previous level" (level *N* = base *
/// `GEAR_LEVEL_GROWTH.powi(N - 1)`). Gear level is capped by
/// `resources::ZoneLevel`: reaching zone *N* is what "unlocks" level *N*
/// gear — see `Game::equip`.
pub const GEAR_LEVEL_GROWTH: f64 = 2.5;

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
    fn plain_resources_are_not_equippable() {
        assert!(ItemId::CoreFragment.equipment().is_none());
        assert!(ItemId::PowerCell.equipment().is_none());
        assert!(ItemId::IceBreaker.equipment().is_none());
    }

    #[test]
    fn each_equipment_item_maps_to_its_own_slot_and_bonus() {
        let (slot, mods) = ItemId::OverclockCore.equipment().unwrap();
        assert_eq!(slot, EquipmentSlot::Weapon);
        assert_eq!((mods.atk, mods.def, mods.decompiler), (3, 0, 0));

        let (slot, mods) = ItemId::FirewallPlating.equipment().unwrap();
        assert_eq!(slot, EquipmentSlot::Armor);
        assert_eq!((mods.atk, mods.def, mods.decompiler), (0, 3, 0));

        let (slot, mods) = ItemId::NeuralAmplifier.equipment().unwrap();
        assert_eq!(slot, EquipmentSlot::Module);
        assert_eq!((mods.atk, mods.def, mods.decompiler), (0, 0, 2));
    }

    #[test]
    fn each_new_equipment_item_shares_its_slot_with_an_existing_alternative() {
        let (slot, mods) = ItemId::MonofilamentWhip.equipment().unwrap();
        assert_eq!(slot, EquipmentSlot::Weapon);
        assert_eq!((mods.atk, mods.def, mods.decompiler), (4, 0, 0));

        let (slot, mods) = ItemId::AblativePlating.equipment().unwrap();
        assert_eq!(slot, EquipmentSlot::Armor);
        assert_eq!((mods.atk, mods.def, mods.decompiler), (0, 4, 0));

        let (slot, mods) = ItemId::CortexHack.equipment().unwrap();
        assert_eq!(slot, EquipmentSlot::Module);
        assert_eq!((mods.atk, mods.def, mods.decompiler), (0, 0, 3));
    }

    #[test]
    fn scaled_for_level_grows_150_percent_per_level_above_1() {
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
            10,
            "level 2 should be 2.5x base (4 * 2.5 = 10)"
        );
        assert_eq!(
            base.scaled_for_level(3).atk,
            25,
            "level 3 should be 2.5x level 2 (10 * 2.5 = 25)"
        );
        assert_eq!(
            base.scaled_for_level(0).atk,
            4,
            "level 0 should clamp to level 1's unscaled base"
        );
    }
}
