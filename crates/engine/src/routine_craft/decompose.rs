//! `decompose`: an authored routine taken back apart into the parts that
//! would build its nearest equivalent. Extraction and boss drops hand these
//! out instead of the routine itself.

use super::catalogue::{AreaKind, Core, Facing, RangeKind, StatusKind, Subroutine, Version, fits};
use super::compile::{core_figures, core_of_field, debuff_base, field_is_timed};
use crate::abilities::{AbilityDef, AbilityEffect, AbilityTarget, TamperKind};
use crate::components::BuffKind;
use crate::tuning as t;

/// The `N` of a trailing `vN.M` in a routine's name, if it ends in one.
fn named_version(name: &str) -> Option<Version> {
    let tail = name.split_whitespace().last()?.strip_prefix('v')?;
    let (major, minor) = tail.split_once('.')?;
    minor.parse::<u32>().ok()?;
    Version::from_number(major.parse().ok()?)
}

/// The Core `def`'s effect is, with the version its numbers imply. `None`
/// for what no Core stands for: a capture, a Mitigation sap.
fn core_and_version(def: &AbilityDef) -> Option<(Core, Version)> {
    let named = named_version(&def.name).unwrap_or(Version::V1);
    let (core, version) = match &def.effect {
        AbilityEffect::Damage { .. } => (Core::Damage, named),
        AbilityEffect::Heal { .. } | AbilityEffect::HealOverTime { .. } => (Core::Heal, named),
        AbilityEffect::Drain { .. } => (Core::Drain, named),
        AbilityEffect::Buff {
            kind: BuffKind::Atk,
            power,
            ..
        } if *power < 0 => (Core::SapAttack, named),
        AbilityEffect::Buff {
            kind: BuffKind::Atk,
            ..
        } => (Core::BoostAttack, named),
        AbilityEffect::Buff {
            kind: BuffKind::Mitigation,
            power,
            ..
        } if *power > 0 => (Core::BoostMitigation, named),
        AbilityEffect::Buff {
            kind: BuffKind::Mitigation,
            ..
        } => return None,
        AbilityEffect::Debuff { .. } => (Core::Debuff, Version::V1),
        AbilityEffect::Cleanse => (Core::Cleanse, Version::V1),
        AbilityEffect::Cloak { .. } => (Core::Cloak, Version::V1),
        AbilityEffect::Summon { count, extra, .. } => {
            let version = match (*count, *extra) {
                (2.., _) => Version::V3,
                (_, 1..) => Version::V2,
                _ => Version::V1,
            };
            (Core::Fork, version)
        }
        AbilityEffect::Reanimate { count } => (
            Core::Respawn,
            Version::from_number((*count).clamp(1, 3)).unwrap_or(Version::V1),
        ),
        AbilityEffect::Tamper { kind, .. } => (
            match kind {
                TamperKind::Temperature(temp) if *temp <= t::TACTICAL_AI_TEMPERATURE => {
                    Core::ColdSample
                }
                TamperKind::Temperature(_) => Core::HeatInjection,
                TamperKind::Profiled => Core::Profile,
                TamperKind::Injected => Core::Inject,
                TamperKind::Hallucinating { .. } => Core::Hallucinate,
            },
            Version::V1,
        ),
        AbilityEffect::Teleport => (Core::Teleport, Version::V1),
        AbilityEffect::Reinforce => (Core::Reinforce, Version::V1),
        AbilityEffect::Emulate { .. } => (Core::Emulate, Version::V1),
        AbilityEffect::Phase => (Core::Phase, Version::V1),
        AbilityEffect::Jump => (Core::Jump, Version::V1),
        AbilityEffect::Symlink => (Core::Symlink, Version::V1),
        AbilityEffect::FieldBuff { kind, .. } => (core_of_field(*kind), named),
        AbilityEffect::Decompile => return None,
    };
    Some((
        core,
        if core.versioned() {
            version
        } else {
            Version::V1
        },
    ))
}

fn area_of(core: Core, target: AbilityTarget) -> Option<AreaKind> {
    match (core.facing(), target) {
        (Facing::Enemy, AbilityTarget::WholeEnemyGroup) => Some(AreaKind::Group),
        (Facing::Enemy, AbilityTarget::AllEnemies) => Some(AreaKind::Everyone),
        (Facing::Ally, AbilityTarget::WholeParty) => Some(AreaKind::Group),
        _ => None,
    }
}

fn range_of(def: &AbilityDef) -> Option<RangeKind> {
    let reach = def
        .range
        .map(|r| r.max)
        .or_else(|| def.ranged.then_some(t::CRAFT_RANGE_NEAR))?;
    if reach > t::CRAFT_RANGE_NEAR {
        Some(RangeKind::Far)
    } else if reach > t::TACTICAL_MELEE_RANGE {
        Some(RangeKind::Near)
    } else {
        None
    }
}

/// How many Trickle parts the duration `def` carries implies.
fn trickles_of(def: &AbilityDef, core: Core, status: Option<StatusKind>) -> u32 {
    let over = |duration: u32, base: u32, per: u32| duration.saturating_sub(base).div_ceil(per);
    match &def.effect {
        AbilityEffect::HealOverTime { rounds, .. } => {
            rounds.div_ceil(t::CRAFT_HOT_ROUNDS_PER_TRICKLE)
        }
        AbilityEffect::Buff { duration, .. }
        | AbilityEffect::Cloak { duration }
        | AbilityEffect::Tamper { duration, .. } => over(
            *duration,
            core_figures(core).rounds,
            t::CRAFT_TRICKLE_ROUNDS,
        ),
        AbilityEffect::Debuff { duration, .. } => status.map_or(0, |s| {
            over(*duration, debuff_base(s).1, t::CRAFT_TRICKLE_ROUNDS)
        }),
        AbilityEffect::FieldBuff { duration, .. } if field_is_timed(core) => over(
            *duration,
            t::CRAFT_FIELD_DURATION,
            t::CRAFT_TRICKLE_FIELD_DURATION,
        ),
        _ => 0,
    }
}

/// The parts that build `def`'s nearest equivalent, or `None` when no Core
/// stands for it. Lossy on purpose: a modifier the Core cannot take is
/// dropped rather than refused (`fits` is the judge), and a passive's
/// trigger, a charge-up and the authored magnitude do not survive.
pub fn decompose(def: &AbilityDef) -> Option<Vec<Subroutine>> {
    let (core, version) = core_and_version(def)?;
    let status = match &def.effect {
        AbilityEffect::Damage {
            status: Some(rider),
            ..
        } => Some(StatusKind::from_id(&rider.kind)?),
        AbilityEffect::Debuff { kind, .. } => Some(StatusKind::from_id(kind)?),
        _ => None,
    };
    let damage_type = match &def.effect {
        AbilityEffect::Damage { damage_type, .. } | AbilityEffect::Drain { damage_type, .. } => {
            *damage_type
        }
        _ => None,
    };

    let mut parts = vec![Subroutine::Core(core, version)];
    let mut modifiers = Vec::new();
    modifiers.extend(range_of(def).map(Subroutine::Range));
    modifiers.extend(area_of(core, def.target).map(Subroutine::Area));
    modifiers.extend(damage_type.map(Subroutine::DamageType));
    modifiers.extend(status.map(Subroutine::Status));
    modifiers.retain(|part| fits(core, part).is_ok());

    let room = t::CRAFT_PART_CAP_MAX.saturating_sub(1 + modifiers.len());
    let trickles = if fits(core, &Subroutine::Trickle).is_ok() {
        (trickles_of(def, core, status) as usize).min(room)
    } else {
        0
    };
    parts.extend(
        modifiers
            .iter()
            .filter(|p| !matches!(p, Subroutine::Status(_) | Subroutine::DamageType(_)))
            .copied(),
    );
    parts.extend(std::iter::repeat_n(Subroutine::Trickle, trickles));
    parts.extend(
        modifiers
            .iter()
            .filter(|p| matches!(p, Subroutine::DamageType(_) | Subroutine::Status(_)))
            .copied(),
    );
    Some(parts)
}
