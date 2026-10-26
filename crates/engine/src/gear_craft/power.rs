//! `design_power`: what a design's weapon is worth, for the ceiling and
//! no-dominance tests.

use crate::game::gear_power::{ReferenceFoe, rate_against};
use crate::items::GearCopy;
use crate::items_db::ItemDef;
use crate::routine_craft::StatusKind;
use crate::tuning as t;

use super::weapon::Proc;

/// `Game::copy_power` of a fresh Ordinary tier-0 copy at the reference
/// zone, plus a tuned flat value per rider, exploit and proc. `copy_power`
/// cannot see those: the rider and exploit are runtime-only fields and a
/// proc is a routine, so without the flat terms the riskiest parts would
/// read as weaker than a plain weapon. `None` when the def pays on no
/// priced axis.
pub(crate) fn design_power(def: &ItemDef, foe: &ReferenceFoe) -> Option<i32> {
    Some(copy_part(def, foe)? + flat_terms(def))
}

/// The `copy_power` half of `design_power`.
pub(crate) fn copy_part(def: &ItemDef, foe: &ReferenceFoe) -> Option<i32> {
    let (_, stats) = def.worn_stats()?;
    let copy = GearCopy::plain(def.id.clone());
    let mods = stats.for_copy(t::POWER_REFERENCE_ZONE, &copy);
    Some(rate_against(mods, foe)?.total)
}

/// The flat half of `design_power`, on its own for the dominance test.
pub fn flat_terms(def: &ItemDef) -> i32 {
    let mut total = 0;
    if let Some(rider) = &def.on_hit_rider
        && let Some(status) = StatusKind::from_id(&rider.kind)
    {
        total += t::GEAR_RIDER_POWER[status.index()];
    }
    if let Some(exploit) = &def.exploit
        && let Some(status) = StatusKind::from_id(&exploit.status)
    {
        total += t::GEAR_EXPLOIT_POWER[status.index()];
    }
    if let Some(proc) = def.grants.as_ref().and_then(Proc::from_ability) {
        total += t::GEAR_PROC_POWER[proc.index()];
    }
    total
}
