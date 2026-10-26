//! What a part costs to craft and what a frame costs to build. Pure tables
//! over `tuning`; `Game` pays them.

use super::catalogue::{Frame, WeaponPart};
use crate::items::ItemId;
use crate::routine_craft::Version;
use crate::tuning as t;

/// The material and count crafting `part` spends.
pub fn part_bill(part: WeaponPart) -> (ItemId, u32) {
    let count = match part {
        WeaponPart::Frame(_, version) => t::GEAR_FRAME_PART_COST[version.index()],
        _ => t::GEAR_MODIFIER_PART_COST,
    };
    (ItemId::from(t::GEAR_PART_MATERIAL), count)
}

/// What building a design on this frame spends beside its parts.
pub fn frame_bill(frame: Frame, version: Version) -> (ItemId, u32) {
    (
        ItemId::from(t::GEAR_FRAME_MATERIAL[frame.index()]),
        t::GEAR_FRAME_BILL[frame.index()][version.index()],
    )
}
