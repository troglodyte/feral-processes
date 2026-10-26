//! `decompose`: an authored weapon taken back apart into the parts that
//! build its nearest equivalent. Salvage hands these out instead of the
//! weapon.

use super::catalogue::{Frame, Proc, WeaponPart};
use crate::abilities::AbilityTarget;
use crate::items::EquipmentSlot;
use crate::items_db::ItemDef;
use crate::routine_craft::{AreaKind, Version};
use crate::tuning as t;

/// The Frame and version whose figures sit nearest a weapon's, among frames
/// that swing from the weapon's own range. Distance is band mean plus
/// accuracy, the two axes a frame's character is written in.
fn nearest_frame(range: u32, band_mean: f32, accuracy: i32) -> Option<(Frame, Version)> {
    let mut best: Option<(f32, Frame, Version)> = None;
    for frame in Frame::ALL {
        let fig = t::GEAR_FRAME[frame.index()];
        if fig.range != range {
            continue;
        }
        for version in Version::ALL {
            let m = t::GEAR_VERSION_MAGNITUDE[version.index()];
            let mean = (fig.band.0 + fig.band.1) as f32 * 0.5 * m;
            let distance = (mean - band_mean).abs() + (fig.accuracy - accuracy).abs() as f32;
            if best.is_none_or(|(d, ..)| distance < d) {
                best = Some((distance, frame, version));
            }
        }
    }
    best.map(|(_, frame, version)| (frame, version))
}

/// The parts that build `def`'s nearest equivalent, frame first, or `None`
/// for something that is not a weapon or has no frame at its range. Lossy on
/// purpose: mitigation maps to nothing (crafted weapons carry no armour),
/// and a `grants` that is not one of the three offered routines is dropped.
pub fn decompose(def: &ItemDef) -> Option<Vec<WeaponPart>> {
    let (EquipmentSlot::Weapon, stats) = def.equipment? else {
        return None;
    };
    let range = def.range.unwrap_or(t::TACTICAL_MELEE_RANGE);
    let mean = (stats.damage.min + stats.damage.max) as f32 * 0.5;
    let (frame, version) = nearest_frame(range, mean, stats.accuracy)?;
    let mut parts = vec![WeaponPart::Frame(frame, version)];
    if let Some(reach) = &def.reach {
        parts.push(WeaponPart::Reach(match reach.target {
            AbilityTarget::AllEnemies => AreaKind::Everyone,
            _ => AreaKind::Group,
        }));
    }
    parts.extend(stats.damage_type.map(WeaponPart::DamageType));
    if stats.decompiler > 0 {
        parts.push(WeaponPart::Capture);
    }
    parts.extend(
        def.grants
            .as_ref()
            .and_then(Proc::from_ability)
            .map(WeaponPart::Proc),
    );
    Some(parts)
}
