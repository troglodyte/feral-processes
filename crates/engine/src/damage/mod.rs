//! Damage types: which kind of harm a hit does, and how much of it a target
//! shrugs off. A hit with no type (`None`) is ordinary damage and resistance
//! never applies to it.

mod effects;
pub use effects::{SideEffect, arc_damage, next_in_order, ring_around, side_effect, statuses};

use serde::{Deserialize, Serialize};
use std::ops::Add;

use crate::tuning::{DAMAGE_RESIST_CAP, SIDE_EFFECT_BLOCK_THRESHOLD};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum DamageType {
    Thermal,
    Magnetic,
    Noise,
    Voltage,
    Radiation,
}

impl DamageType {
    pub const ALL: [DamageType; 5] = [
        DamageType::Thermal,
        DamageType::Magnetic,
        DamageType::Noise,
        DamageType::Voltage,
        DamageType::Radiation,
    ];

    /// The word every screen and log line uses for this type.
    pub fn name(self) -> &'static str {
        match self {
            DamageType::Thermal => "Thermal",
            DamageType::Magnetic => "Magnetic",
            DamageType::Noise => "Noise",
            DamageType::Voltage => "Voltage",
            DamageType::Radiation => "Radiation",
        }
    }
}

/// Percent resistance to each damage type. Summed across gear; the cap is
/// applied at use (`resist_against`), not here.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Resistances {
    #[serde(default)]
    pub thermal: i32,
    #[serde(default)]
    pub magnetic: i32,
    #[serde(default)]
    pub noise: i32,
    #[serde(default)]
    pub voltage: i32,
    #[serde(default)]
    pub radiation: i32,
}

impl Resistances {
    pub fn get(self, ty: DamageType) -> i32 {
        match ty {
            DamageType::Thermal => self.thermal,
            DamageType::Magnetic => self.magnetic,
            DamageType::Noise => self.noise,
            DamageType::Voltage => self.voltage,
            DamageType::Radiation => self.radiation,
        }
    }
    /// The types resisted at all, in `DamageType::ALL` order, so a screen
    /// can omit the rest. A negative is left out: `resist_against` floors it
    /// at 0, so it would be shown doing something it does not.
    pub fn resisted(self) -> Vec<(DamageType, i32)> {
        DamageType::ALL
            .into_iter()
            .map(|t| (t, self.get(t)))
            .filter(|&(_, pct)| pct > 0)
            .collect()
    }
    /// Every resistance multiplied by `n`: a stacked affix counts `n` times.
    pub fn times(self, n: i32) -> Resistances {
        Resistances {
            thermal: self.thermal * n,
            magnetic: self.magnetic * n,
            noise: self.noise * n,
            voltage: self.voltage * n,
            radiation: self.radiation * n,
        }
    }
}

impl Add for Resistances {
    type Output = Resistances;
    fn add(self, o: Resistances) -> Resistances {
        Resistances {
            thermal: self.thermal + o.thermal,
            magnetic: self.magnetic + o.magnetic,
            noise: self.noise + o.noise,
            voltage: self.voltage + o.voltage,
            radiation: self.radiation + o.radiation,
        }
    }
}

/// The effective resistance (percent) to a hit of type `ty`: 0 for an
/// untyped hit, otherwise the summed value clamped to `0..=DAMAGE_RESIST_CAP`.
pub fn resist_against(res: Resistances, ty: Option<DamageType>) -> i32 {
    ty.map_or(0, |t| res.get(t).clamp(0, DAMAGE_RESIST_CAP))
}

/// `dmg` after a type resistance. The identity at 0.
pub fn type_cut(dmg: f64, resist_pct: i32) -> f64 {
    if resist_pct == 0 {
        return dmg;
    }
    dmg * (1.0 - resist_pct as f64 / 100.0)
}

/// Whether a resistance is high enough to stop a type's side effect.
pub fn side_effect_blocked(resist_pct: i32) -> bool {
    resist_pct >= SIDE_EFFECT_BLOCK_THRESHOLD
}

/// Whether a hit of `ty` against a target resisting it by `resist_pct` skips
/// the target's flat deflection. Magnetic does, until resistance blocks the
/// effect like any other type's.
pub fn ignores_deflection(ty: DamageType, resist_pct: i32) -> bool {
    ty == DamageType::Magnetic && !side_effect_blocked(resist_pct)
}

/// How much of a hit a resistance stopped, and which type stopped it. Only
/// built when `amount > 0`, so an uncut hit has none.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Resisted {
    pub ty: DamageType,
    pub amount: i32,
}

/// The suffix every hit line appends: what resistance cut, or nothing. **The
/// one formatter**, so every path that words a hit words the cut the same.
pub fn resisted_note(resisted: Option<Resisted>) -> String {
    match resisted {
        Some(r) => format!(" ({} stopped by {} resist)", r.amount, r.ty.name()),
        None => String::new(),
    }
}

/// A damage band plus its type — `"5–10 Thermal"`, or exactly
/// `abilities::range_label` when untyped. **The one place a band and its type
/// are joined**, so gear, bodies and routines cannot word it differently.
pub fn band_label(range: crate::battle::DamageRange, ty: Option<DamageType>) -> String {
    let band = crate::abilities::range_label(range);
    match ty {
        Some(ty) => format!("{band} {}", ty.name()),
        None => band,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_resisted_note_names_the_type_and_the_amount() {
        let r = Resisted {
            ty: DamageType::Thermal,
            amount: 3,
        };
        assert_eq!(resisted_note(Some(r)), " (3 stopped by Thermal resist)");
    }

    #[test]
    fn no_resistance_adds_nothing_to_a_line() {
        assert_eq!(resisted_note(None), "");
    }

    #[test]
    fn the_cap_clamps_a_sum_over_100() {
        let res = Resistances {
            thermal: 60,
            ..Default::default()
        } + Resistances {
            thermal: 70,
            ..Default::default()
        };
        assert_eq!(res.thermal, 130);
        assert_eq!(
            resist_against(res, Some(DamageType::Thermal)),
            DAMAGE_RESIST_CAP
        );
    }

    #[test]
    fn an_untyped_hit_resists_nothing() {
        let res = Resistances {
            thermal: 50,
            ..Default::default()
        };
        assert_eq!(resist_against(res, None), 0);
    }

    #[test]
    fn type_cut_is_the_identity_at_zero() {
        for d in [0.0, 1.0, 7.5, 123.0] {
            assert_eq!(type_cut(d, 0), d);
        }
        assert_eq!(type_cut(10.0, 50), 5.0);
    }

    #[test]
    fn only_unresisted_magnetic_ignores_deflection() {
        assert!(ignores_deflection(DamageType::Magnetic, 0));
        assert!(ignores_deflection(
            DamageType::Magnetic,
            SIDE_EFFECT_BLOCK_THRESHOLD - 1
        ));
        assert!(!ignores_deflection(
            DamageType::Magnetic,
            SIDE_EFFECT_BLOCK_THRESHOLD
        ));
        for ty in DamageType::ALL {
            if ty != DamageType::Magnetic {
                assert!(!ignores_deflection(ty, 0));
            }
        }
    }

    #[test]
    fn the_threshold_fires_at_exactly_the_constant() {
        assert!(!side_effect_blocked(SIDE_EFFECT_BLOCK_THRESHOLD - 1));
        assert!(side_effect_blocked(SIDE_EFFECT_BLOCK_THRESHOLD));
    }
}
