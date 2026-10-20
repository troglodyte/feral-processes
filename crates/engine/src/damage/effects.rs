//! What a typed hit does beyond its damage. The decision is pure: the shell
//! (`Game::type_side_effect`) reads the target's resistance, calls
//! `side_effect`, and arms what comes back. Adding a type's effect is an arm
//! in `side_effect` plus its status in `statuses`.

use super::{DamageType, side_effect_blocked};
use crate::statuses::{StatusBehaviour, StatusDef, StatusId, StatusStacking};
use crate::tuning::{NOISE_JAM_ACCURACY_CUT, NOISE_JAM_ROUNDS, NOISE_JAMMED_STATUS};

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
        DamageType::Thermal
        | DamageType::Magnetic
        | DamageType::Voltage
        | DamageType::Radiation => None,
    }
}

/// The statuses the side effects arm, defined here so they exist whatever
/// `assets/statuses/` holds (`StatusDb::with_builtins`).
pub fn statuses() -> Vec<StatusDef> {
    vec![StatusDef {
        id: StatusId::from(NOISE_JAMMED_STATUS),
        name: "Jammed".into(),
        tag: "JAM".into(),
        stacking: StatusStacking::Refresh,
        behaviours: vec![StatusBehaviour::AccuracyCut(NOISE_JAM_ACCURACY_CUT)],
        inflict: "Targeting jammed on {target}!".into(),
        tick: String::new(),
        expire: "Targeting clears on {target}.".into(),
    }]
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
    fn a_type_with_no_effect_yet_does_nothing() {
        assert_eq!(side_effect(DamageType::Thermal, 0), None);
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
