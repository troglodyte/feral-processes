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
                EquipmentStats { atk: 3, ..EquipmentStats::default() },
            )),
            ItemId::MonofilamentWhip => Some((
                EquipmentSlot::Weapon,
                EquipmentStats { atk: 4, ..EquipmentStats::default() },
            )),
            ItemId::FirewallPlating => Some((
                EquipmentSlot::Armor,
                EquipmentStats { def: 3, ..EquipmentStats::default() },
            )),
            ItemId::AblativePlating => Some((
                EquipmentSlot::Armor,
                EquipmentStats { def: 4, ..EquipmentStats::default() },
            )),
            ItemId::NeuralAmplifier => Some((
                EquipmentSlot::Module,
                EquipmentStats { decompiler: 2, ..EquipmentStats::default() },
            )),
            ItemId::CortexHack => Some((
                EquipmentSlot::Module,
                EquipmentStats { decompiler: 3, ..EquipmentStats::default() },
            )),
            ItemId::CoreFragment | ItemId::PowerCell | ItemId::IceBreaker | ItemId::PortalFragment => {
                None
            }
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

/// Flat stat bonuses an equipped item grants while worn.
#[derive(Clone, Copy, Debug, Default)]
pub struct EquipmentStats {
    pub atk: i32,
    pub def: i32,
    pub decompiler: i32,
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
}
