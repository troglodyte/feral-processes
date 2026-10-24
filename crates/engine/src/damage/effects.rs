//! What a typed hit does beyond its damage. The decision is pure: the shell
//! (`Game::type_side_effect`) reads the target's resistance, calls
//! `side_effect`, and arms what comes back. Adding a type's effect is an arm
//! in `side_effect` plus its status in `statuses`.

use super::{DamageType, side_effect_blocked};
use crate::statuses::{StatusBehaviour, StatusDef, StatusId, StatusStacking};
use crate::tuning::{
    NOISE_JAM_ACCURACY_CUT, NOISE_JAM_ROUNDS, NOISE_JAMMED_STATUS, RADIATION_CORRUPTED_STATUS,
    RADIATION_CORRUPTION_POWER, RADIATION_CORRUPTION_ROUNDS, THERMAL_HEAT_DAMAGE_TAKEN_PCT,
    THERMAL_HEAT_MAX_STACKS, THERMAL_HEAT_ROUNDS, THERMAL_HEAT_STATUS, VOLTAGE_ARC_SHARE_PCT,
};
use bevy_ecs::entity::Entity;

/// A side effect decided by `side_effect`. Concrete, not a general effect
/// type: each new kind of effect is a variant here.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SideEffect {
    /// Arm the built-in status `id` on the target.
    Arm {
        id: &'static str,
        rounds: u32,
        power: i32,
    },
    /// Part of the hit arcs to the target's neighbour (`arc_damage`).
    Arc,
}

/// The side effect of a landed hit of `ty` on a target resisting it by
/// `resist_pct`, or `None` when the type has none or the resistance blocks it.
pub fn side_effect(ty: DamageType, resist_pct: i32) -> Option<SideEffect> {
    if side_effect_blocked(resist_pct) {
        return None;
    }
    match ty {
        DamageType::Noise => Some(SideEffect::Arm {
            id: NOISE_JAMMED_STATUS,
            rounds: NOISE_JAM_ROUNDS,
            power: 0,
        }),
        DamageType::Radiation => Some(SideEffect::Arm {
            id: RADIATION_CORRUPTED_STATUS,
            rounds: RADIATION_CORRUPTION_ROUNDS,
            power: RADIATION_CORRUPTION_POWER,
        }),
        DamageType::Thermal => Some(SideEffect::Arm {
            id: THERMAL_HEAT_STATUS,
            rounds: THERMAL_HEAT_ROUNDS,
            power: 0,
        }),
        // Magnetic acts in mitigation (`ignores_deflection`), not as a status.
        DamageType::Magnetic => None,
        DamageType::Voltage => Some(SideEffect::Arc),
    }
}

/// What arcs to the neighbour of a Voltage hit that landed `landed`. Zero
/// means no arc.
pub fn arc_damage(landed: i32) -> i32 {
    landed * VOLTAGE_ARC_SHARE_PCT / 100
}

/// The body after `who` in `order`, wrapping round, or `None` when `who` is
/// alone in it or not in it. `order` is the caller's living bodies of one
/// side; the abstract models and a sortie all reduce to this.
pub fn next_in_order(order: &[Entity], who: Entity) -> Option<Entity> {
    let at = order.iter().position(|&e| e == who)?;
    let next = order[(at + 1) % order.len()];
    (next != who).then_some(next)
}

/// The cells in the ring around `footprint`: every cell touching it (corners
/// included) that is not part of it, sorted so a caller's pick is stable.
pub fn ring_around(footprint: &[(i32, i32)]) -> Vec<(i32, i32)> {
    let mut ring: Vec<(i32, i32)> = footprint
        .iter()
        .flat_map(|&(x, y)| (-1..=1).flat_map(move |dx| (-1..=1).map(move |dy| (x + dx, y + dy))))
        .filter(|cell| !footprint.contains(cell))
        .collect();
    ring.sort_unstable();
    ring.dedup();
    ring
}

/// The statuses the side effects arm, defined here so they exist whatever
/// `assets/statuses/` holds (`StatusDb::with_builtins`).
pub fn statuses() -> Vec<StatusDef> {
    vec![
        StatusDef {
            id: StatusId::from(NOISE_JAMMED_STATUS),
            name: "Jammed".into(),
            tag: "JAM".into(),
            stacking: StatusStacking::Refresh,
            behaviours: vec![StatusBehaviour::AccuracyCut(NOISE_JAM_ACCURACY_CUT)],
            inflict: "Targeting jammed on {target}!".into(),
            tick: String::new(),
            expire: "Targeting clears on {target}.".into(),
        },
        // Healing still works, so no `HealBlock`: the corruption is a drain
        // to outlast, not a lock.
        StatusDef {
            id: StatusId::from(RADIATION_CORRUPTED_STATUS),
            name: "Corrupted".into(),
            tag: "COR".into(),
            stacking: StatusStacking::Refresh,
            behaviours: vec![StatusBehaviour::DamagePerRound],
            inflict: "Data starts corrupting on {target}!".into(),
            tick: "Corruption eats {n} Integrity from {target}.".into(),
            expire: "Corruption is purged from {target}.".into(),
        },
        StatusDef {
            id: StatusId::from(THERMAL_HEAT_STATUS),
            name: "Overheating".into(),
            tag: "HOT".into(),
            stacking: StatusStacking::Stack {
                max: THERMAL_HEAT_MAX_STACKS,
            },
            behaviours: vec![StatusBehaviour::DamageTakenPercent(
                THERMAL_HEAT_DAMAGE_TAKEN_PCT,
            )],
            inflict: "Overheating on {target}!".into(),
            tick: String::new(),
            expire: "Heat dissipates from {target}.".into(),
        },
        StatusDef {
            id: StatusId::from(crate::tuning::REGEN_STATUS),
            name: "Regenerating".into(),
            tag: "RGN".into(),
            stacking: StatusStacking::Refresh,
            behaviours: vec![StatusBehaviour::HealPerRound],
            inflict: "A repair routine takes hold on {target}.".into(),
            tick: "{target} mends {n} Integrity.".into(),
            expire: "The repair routine finishes on {target}.".into(),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tuning::SIDE_EFFECT_BLOCK_THRESHOLD;

    #[test]
    fn noise_arms_jammed_and_resistance_at_the_threshold_blocks_it() {
        assert_eq!(
            side_effect(DamageType::Noise, 0),
            Some(SideEffect::Arm {
                id: "jammed",
                rounds: NOISE_JAM_ROUNDS,
                power: 0
            })
        );
        assert!(side_effect(DamageType::Noise, SIDE_EFFECT_BLOCK_THRESHOLD - 1).is_some());
        assert_eq!(
            side_effect(DamageType::Noise, SIDE_EFFECT_BLOCK_THRESHOLD),
            None
        );
    }

    #[test]
    fn radiation_arms_corrupted_and_resistance_at_the_threshold_blocks_it() {
        assert_eq!(
            side_effect(DamageType::Radiation, 0),
            Some(SideEffect::Arm {
                id: "corrupted",
                rounds: RADIATION_CORRUPTION_ROUNDS,
                power: RADIATION_CORRUPTION_POWER
            })
        );
        assert!(side_effect(DamageType::Radiation, SIDE_EFFECT_BLOCK_THRESHOLD - 1).is_some());
        assert_eq!(
            side_effect(DamageType::Radiation, SIDE_EFFECT_BLOCK_THRESHOLD),
            None
        );
    }

    #[test]
    fn thermal_arms_heat_and_resistance_at_the_threshold_blocks_it() {
        assert_eq!(
            side_effect(DamageType::Thermal, 0),
            Some(SideEffect::Arm {
                id: "heat",
                rounds: THERMAL_HEAT_ROUNDS,
                power: 0
            })
        );
        assert!(side_effect(DamageType::Thermal, SIDE_EFFECT_BLOCK_THRESHOLD - 1).is_some());
        assert_eq!(
            side_effect(DamageType::Thermal, SIDE_EFFECT_BLOCK_THRESHOLD),
            None
        );
    }

    #[test]
    fn magnetic_has_no_side_effect_because_it_acts_in_mitigation() {
        assert_eq!(side_effect(DamageType::Magnetic, 0), None);
    }

    #[test]
    fn voltage_arcs_and_resistance_at_the_threshold_blocks_it() {
        assert_eq!(side_effect(DamageType::Voltage, 0), Some(SideEffect::Arc));
        assert!(side_effect(DamageType::Voltage, SIDE_EFFECT_BLOCK_THRESHOLD - 1).is_some());
        assert_eq!(
            side_effect(DamageType::Voltage, SIDE_EFFECT_BLOCK_THRESHOLD),
            None
        );
    }

    #[test]
    fn an_arc_is_half_the_landed_hit_rounded_down() {
        assert_eq!(arc_damage(20), 10);
        assert_eq!(arc_damage(7), 3);
        assert_eq!(arc_damage(1), 0);
        assert_eq!(arc_damage(0), 0);
    }

    #[test]
    fn the_next_body_wraps_and_a_lone_one_has_none() {
        let e = |i| Entity::from_raw_u32(i).unwrap();
        let order = [e(1), e(2), e(3)];
        assert_eq!(next_in_order(&order, e(1)), Some(e(2)));
        assert_eq!(next_in_order(&order, e(3)), Some(e(1)));
        assert_eq!(next_in_order(&[e(1)], e(1)), None);
        assert_eq!(next_in_order(&order, e(9)), None);
    }

    #[test]
    fn the_ring_around_one_cell_is_its_eight_neighbours() {
        let ring = ring_around(&[(5, 5)]);
        assert_eq!(ring.len(), 8);
        assert!(!ring.contains(&(5, 5)) && ring.contains(&(6, 6)));
        let two = ring_around(&[(5, 5), (6, 5)]);
        assert_eq!(two.len(), 10);
    }

    #[test]
    fn every_armed_status_is_a_built_in() {
        let ids: Vec<StatusId> = statuses().into_iter().map(|d| d.id).collect();
        for ty in DamageType::ALL {
            if let Some(SideEffect::Arm { id, .. }) = side_effect(ty, 0) {
                assert!(ids.contains(&StatusId::from(id)), "{id} is not built in");
            }
        }
    }

    #[test]
    fn no_shipped_status_file_uses_a_built_in_id() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/statuses");
        let (db, _) = crate::statuses::StatusDb::load_dir(&dir).unwrap();
        for def in statuses() {
            assert!(!db.contains(&def.id), "{:?} is shipped as a file", def.id.0);
        }
    }

    #[test]
    fn a_file_using_a_built_in_id_loses_with_a_warning() {
        let mut db = crate::statuses::StatusDb::default();
        let mut clash = statuses().remove(0);
        clash.name = "Mod".into();
        db.insert(clash);
        let (db, warnings) = db.with_builtins();
        assert_eq!(warnings.len(), 1, "{warnings:?}");
        assert_eq!(db.get(&StatusId::from("jammed")).unwrap().name, "Jammed");
    }
}
