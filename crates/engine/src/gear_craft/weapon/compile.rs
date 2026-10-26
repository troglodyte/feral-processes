//! The one place weapon parts become numbers. Pure: a design and the
//! affix catalogue in, an `ItemDef` out.

use super::catalogue::{Frame, Proc, Tilt, WeaponPart};
use super::fit;
use crate::abilities::AbilityTarget;
use crate::affixes::AffixDb;
use crate::damage::DamageType;
use crate::items::{EquipmentSlot, EquipmentStats, ItemId, WeaponReach};
use crate::items_db::{ItemDef, WeaponExploit};
use crate::routine_craft::{AreaKind, StatusKind, Version};
use crate::species::MoveEffect;
use crate::tuning as t;

use super::super::{CompileError, GearDesign, GearPart};

/// Everything the parts do, folded before rounding.
struct Folded {
    range: u32,
    accuracy: i32,
    band_factor: f32,
    reach: Option<AreaKind>,
    damage_type: Option<DamageType>,
    rider: Option<StatusKind>,
    exploit: Option<StatusKind>,
    capture: bool,
    proc: Option<Proc>,
}

fn fold(frame: Frame, parts: &[WeaponPart]) -> Folded {
    let fig = t::GEAR_FRAME[frame.index()];
    let mut f = Folded {
        range: fig.range,
        accuracy: fig.accuracy,
        band_factor: 1.0,
        reach: None,
        damage_type: None,
        rider: None,
        exploit: None,
        capture: false,
        proc: None,
    };
    for part in parts {
        match *part {
            WeaponPart::Frame(..) => {}
            WeaponPart::Extend => {
                f.range += 1;
                f.accuracy += t::GEAR_EXTEND_ACCURACY;
            }
            WeaponPart::Reach(kind) => {
                f.reach = Some(kind);
                f.band_factor *= t::GEAR_REACH_BAND[reach_index(kind)];
            }
            WeaponPart::DamageType(ty) => {
                f.damage_type = Some(ty);
                f.band_factor *= t::GEAR_TYPE_BAND;
            }
            WeaponPart::Rider(status) => {
                f.rider = Some(status);
                f.accuracy += t::GEAR_RIDER_ACCURACY[status.index()];
                f.band_factor *= t::GEAR_RIDER_BAND[status.index()];
            }
            WeaponPart::Exploit(status) => {
                f.exploit = Some(status);
                f.accuracy += t::GEAR_EXPLOIT_ACCURACY[status.index()];
            }
            WeaponPart::Tilt(Tilt::Precise) => {
                f.accuracy += t::GEAR_TILT_PRECISE_ACCURACY;
                f.band_factor *= t::GEAR_TILT_PRECISE_BAND;
            }
            WeaponPart::Tilt(Tilt::Heavy) => {
                f.accuracy += t::GEAR_TILT_HEAVY_ACCURACY;
                f.band_factor *= t::GEAR_TILT_HEAVY_BAND;
            }
            WeaponPart::Capture => {
                f.capture = true;
                f.band_factor *= t::GEAR_CAPTURE_BAND;
            }
            WeaponPart::Proc(proc) => {
                f.proc = Some(proc);
                f.band_factor *= t::GEAR_PROC_BAND;
                f.accuracy += t::GEAR_PROC_ACCURACY;
            }
        }
    }
    f
}

fn reach_index(kind: AreaKind) -> usize {
    match kind {
        AreaKind::Group => 0,
        AreaKind::Everyone => 1,
    }
}

/// The item id a design's weapon is registered under.
pub fn item_id(design_id: u32) -> ItemId {
    ItemId(format!("crafted_gear/{design_id}"))
}

/// The band before it is rounded to whole points. The rounding is a display
/// fact; the no-dominance census compares this, because at a band of 3-5
/// two parts with different costs round to the same weapon.
pub(crate) fn exact_band(frame: Frame, version: Version, band_factor: f32) -> (f32, f32) {
    let fig = t::GEAR_FRAME[frame.index()];
    let scale = t::GEAR_VERSION_MAGNITUDE[version.index()] * band_factor;
    (fig.band.0 as f32 * scale, fig.band.1 as f32 * scale)
}

/// The exact band mean of a legal part list, for the census.
#[cfg(test)]
pub(crate) fn exact_band_of(parts: &[WeaponPart]) -> f32 {
    let (frame, version) = fit::frame_of(parts).unwrap();
    let (lo, hi) = exact_band(frame, version, fold(frame, parts).band_factor);
    lo + hi
}

fn scaled(base: i32, factor: f32) -> i32 {
    (base as f32 * factor).round() as i32
}

pub fn compile(design: &GearDesign, affixes: &AffixDb) -> Result<ItemDef, CompileError> {
    let mut parts = Vec::new();
    let mut affix_ids = Vec::new();
    for part in &design.parts {
        match part {
            GearPart::Weapon(w) => parts.push(*w),
            GearPart::Affix(id) => affix_ids.push(id),
        }
    }
    fit::check(&parts, affix_ids.len(), t::GEAR_PART_CAP_MAX).map_err(CompileError::Fit)?;
    let (frame, version) = fit::frame_of(&parts).map_err(CompileError::Fit)?;
    let f = fold(frame, &parts);

    let (exact_min, exact_max) = exact_band(frame, version, f.band_factor);
    let band_min = (exact_min.round() as i32).max(1);
    let band_max = (exact_max.round() as i32).max(band_min);
    let fig = t::GEAR_FRAME[frame.index()];
    let magnitude = t::GEAR_VERSION_MAGNITUDE[version.index()];
    let mut stats = EquipmentStats {
        atk: scaled(fig.atk, magnitude),
        decompiler: if f.capture {
            t::GEAR_CAPTURE_DECOMPILER
        } else {
            0
        },
        damage: crate::battle::DamageRange {
            min: band_min,
            max: band_max,
        },
        accuracy: f.accuracy,
        damage_type: f.damage_type,
        ..Default::default()
    };
    for id in affix_ids {
        let affix = affixes
            .get(id)
            .ok_or_else(|| CompileError::UnknownAffix(id.clone()))?;
        if !affix.fits(EquipmentSlot::Weapon) {
            return Err(CompileError::AffixDoesNotFit(id.clone()));
        }
        stats = stats.plus_affix(affix.stats);
    }

    Ok(ItemDef {
        id: item_id(design.id),
        name: design.name.clone(),
        description: describe(frame, &f),
        banked: false,
        value: None,
        role: None,
        equipment: Some((EquipmentSlot::Weapon, stats)),
        durability: None,
        taming_potency: None,
        consume: None,
        craftable: None,
        trap: None,
        droppable: None,
        cache_drop: None,
        grid_fuel: None,
        upgrade: None,
        grants: f.proc.map(|p| p.ability()),
        enables_rest: false,
        implant: None,
        reach: f.reach.map(|kind| WeaponReach {
            target: match kind {
                AreaKind::Group => AbilityTarget::WholeEnemyGroup,
                AreaKind::Everyone => AbilityTarget::AllEnemies,
            },
            shape: None,
            recharge: t::GEAR_REACH_RECHARGE[reach_index(kind)],
        }),
        range: (f.range > 1).then_some(f.range),
        abbrev: None,
        glyph: Some('\''),
        fx: Some(
            if frame == Frame::Edge {
                "slash"
            } else {
                "laser_pulse"
            }
            .into(),
        ),
        on_hit_rider: f.rider.map(|status| {
            let (chance, duration, power) = t::GEAR_RIDER[status.index()];
            MoveEffect {
                kind: status.id(),
                chance,
                duration,
                power,
            }
        }),
        exploit: f.exploit.map(|status| WeaponExploit {
            status: status.id(),
            percent: t::GEAR_EXPLOIT_PERCENT[status.index()],
        }),
    })
}

/// A plain-language line for what the design does beyond its frame.
fn describe(frame: Frame, f: &Folded) -> String {
    let mut line = format!(
        "A {} built at the Workbench, swung from {} cell{} away.",
        frame.name().to_lowercase(),
        f.range,
        if f.range == 1 { "" } else { "s" }
    );
    if let Some(kind) = f.reach {
        line.push_str(match kind {
            AreaKind::Group => " Sweeps a whole group, but needs time between swings.",
            AreaKind::Everyone => " Reaches every enemy, but needs time between swings.",
        });
    }
    if let Some(status) = f.rider {
        line.push_str(&format!(" May leave a target with {}.", status.name()));
    }
    if let Some(status) = f.exploit {
        line.push_str(&format!(" Hits harder against {}.", status.name()));
    }
    line
}
