//! Field-for-field conversions between live gear and its save records, shared
//! by the save and load sides.

use crate::*;

/// The save's flat worn-item fields back into the live nested form, `None`
/// for an empty slot.
///
/// The save keeps equipment flat (see `save::EquippedItemSave`) precisely so
/// that adding a property to a worn copy stays an additive `#[serde(default)]`
/// field, which nesting `GearCopy` on disk would not be. These two functions
/// are the whole cost of that, and having them named is what stops the
/// conversion being open-coded at the four sites that need it — three player
/// slots and every program's loadout.
#[allow(clippy::too_many_arguments)]
pub(super) fn worn_from_save(
    item: Option<ItemId>,
    level: u32,
    fusion_tier: u32,
    rarity: Rarity,
    affix: Option<AffixId>,
    affixes: Vec<AffixId>,
    quality: u8,
    wear: u8,
) -> Option<EquippedItem> {
    Some(EquippedItem {
        copy: GearCopy {
            wear,
            ..GearCopy::with_affixes(
                item?,
                rarity,
                fusion_tier,
                save::affixes_from_save(affix, affixes),
                quality,
            )
        },
        level,
    })
}

/// The inverse of `worn_from_save`, for the write side. The legacy singular
/// affix field is left empty here and read on load only — see
/// `save::affixes_from_save`.
pub(super) fn worn_to_save(worn: &EquippedItem) -> save::EquippedItemSave {
    save::EquippedItemSave {
        item: worn.copy.item.clone(),
        level: worn.level,
        fusion_tier: worn.copy.tier,
        rarity: worn.copy.rarity,
        affix: None,
        affixes: worn.copy.affixes.clone(),
        quality: worn.copy.quality,
        wear: worn.copy.wear,
    }
}

/// A carried copy off disk, with the pre-stacking singular affix lifted.
/// Named beside `worn_from_save`/`worn_to_save` for that pair's stated
/// reason: the conversion is open-coded nowhere, so no site can drop the
/// legacy field and strip an affix off a reloaded save in silence.
pub(super) fn copy_from_save(saved: save::GearCopySave) -> GearCopy {
    GearCopy {
        wear: saved.wear,
        ..GearCopy::with_affixes(
            saved.item,
            saved.rarity,
            saved.tier,
            save::affixes_from_save(saved.affix, saved.affixes),
            saved.quality,
        )
    }
}

/// The inverse of `copy_from_save`, for the write side.
pub(super) fn copy_to_save(copy: &GearCopy) -> save::GearCopySave {
    save::GearCopySave {
        item: copy.item.clone(),
        rarity: copy.rarity,
        tier: copy.tier,
        affix: None,
        affixes: copy.affixes.clone(),
        quality: copy.quality,
        wear: copy.wear,
    }
}
