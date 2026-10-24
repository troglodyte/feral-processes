//! `compile`: the one place a build becomes numbers (ADR 0029).

use super::catalogue::{
    AreaKind, Core, CraftedRoutine, RangeKind, StatusKind, Subroutine, Version,
};
use crate::abilities::{
    AbilityDef, AbilityEffect, AbilityRange, AbilityShape, AbilityTarget, TamperKind, every_turn,
};
use crate::components::{BuffKind, FieldBuffKind};
use crate::damage::DamageType;
use crate::species::MoveEffect;
use crate::tuning::{self as t, CoreFigures};

/// A build taken apart by slot. Every field but `core` is optional because a
/// build may leave it out.
pub(super) struct Split {
    pub core: Core,
    pub version: Version,
    pub range: Option<RangeKind>,
    pub area: Option<AreaKind>,
    pub trickles: u32,
    pub damage_type: Option<DamageType>,
    pub status: Option<StatusKind>,
}

/// `None` when `parts` holds no Core. Does not check that the modifiers fit;
/// `validate_build` is the caller's job and `compile` the one reader.
pub(super) fn split(parts: &[Subroutine]) -> Option<Split> {
    let (core, version) = parts.iter().find_map(|p| match p {
        Subroutine::Core(core, version) => Some((*core, *version)),
        _ => None,
    })?;
    let mut split = Split {
        core,
        version,
        range: None,
        area: None,
        trickles: 0,
        damage_type: None,
        status: None,
    };
    for part in parts {
        match part {
            Subroutine::Core(..) => {}
            Subroutine::Range(kind) => split.range = Some(*kind),
            Subroutine::Area(kind) => split.area = Some(*kind),
            Subroutine::Trickle => split.trickles += 1,
            Subroutine::DamageType(ty) => split.damage_type = Some(*ty),
            Subroutine::Status(kind) => split.status = Some(*kind),
        }
    }
    Some(split)
}

/// The Core's v1.0 figures from `tuning/routine_craft.rs`.
pub(super) fn core_figures(core: Core) -> CoreFigures {
    match core {
        Core::Damage => t::CRAFT_DAMAGE,
        Core::Heal => t::CRAFT_HEAL,
        Core::Drain => t::CRAFT_DRAIN,
        Core::BoostAttack => t::CRAFT_BOOST_ATTACK,
        Core::BoostMitigation => t::CRAFT_BOOST_MITIGATION,
        Core::SapAttack => t::CRAFT_SAP_ATTACK,
        Core::Debuff => t::CRAFT_DEBUFF,
        Core::Cleanse => t::CRAFT_CLEANSE,
        Core::Cloak => t::CRAFT_CLOAK,
        Core::Fork => t::CRAFT_FORK,
        Core::Respawn => t::CRAFT_RESPAWN,
        Core::ColdSample => t::CRAFT_COLD_SAMPLE,
        Core::HeatInjection => t::CRAFT_HEAT_INJECTION,
        Core::Profile => t::CRAFT_PROFILE,
        Core::Inject => t::CRAFT_INJECT,
        Core::Hallucinate => t::CRAFT_HALLUCINATE,
        Core::Teleport => t::CRAFT_TELEPORT,
        Core::Reinforce => t::CRAFT_REINFORCE,
        Core::Emulate => t::CRAFT_EMULATE,
        Core::FieldRegen => t::CRAFT_FIELD_REGEN,
        Core::Recharge => t::CRAFT_RECHARGE,
        Core::FieldAttack => t::CRAFT_FIELD_ATTACK,
        Core::FieldMitigation => t::CRAFT_FIELD_MITIGATION,
        Core::CaptureBoost => t::CRAFT_CAPTURE_BOOST,
        Core::XpBoost => t::CRAFT_XP_BOOST,
        Core::EncounterDamp => t::CRAFT_ENCOUNTER_DAMP,
        Core::DropBoost => t::CRAFT_DROP_BOOST,
        Core::Phase => t::CRAFT_PHASE,
        Core::Jump => t::CRAFT_JUMP,
        Core::Symlink => t::CRAFT_SYMLINK,
    }
}

/// The rounds a Debuff Core's status lasts with no Trickle.
pub(super) fn debuff_base(status: StatusKind) -> (i32, u32) {
    t::CRAFT_DEBUFF_STATUS[status.index()]
}

/// The field buff a field Core arms.
fn field_kind(core: Core) -> Option<FieldBuffKind> {
    match core {
        Core::FieldRegen => Some(FieldBuffKind::Regen),
        Core::Recharge => Some(FieldBuffKind::Trickle),
        Core::FieldAttack => Some(FieldBuffKind::Atk),
        Core::FieldMitigation => Some(FieldBuffKind::Mitigation),
        Core::CaptureBoost => Some(FieldBuffKind::CaptureBoost),
        Core::XpBoost => Some(FieldBuffKind::XpBoost),
        Core::EncounterDamp => Some(FieldBuffKind::EncounterDamp),
        Core::DropBoost => Some(FieldBuffKind::DropBoost),
        _ => None,
    }
}

/// The inverse of `field_kind`, for `decompose`.
pub(super) fn core_of_field(kind: FieldBuffKind) -> Core {
    match kind {
        FieldBuffKind::Regen => Core::FieldRegen,
        FieldBuffKind::Trickle => Core::Recharge,
        FieldBuffKind::Atk => Core::FieldAttack,
        FieldBuffKind::Mitigation => Core::FieldMitigation,
        FieldBuffKind::CaptureBoost => Core::CaptureBoost,
        FieldBuffKind::XpBoost => Core::XpBoost,
        FieldBuffKind::EncounterDamp => Core::EncounterDamp,
        FieldBuffKind::DropBoost => Core::DropBoost,
    }
}

/// Whether a field Core is timed (and so takes Trickle) rather than lasting
/// until the party rests.
pub(super) fn field_is_timed(core: Core) -> bool {
    matches!(core, Core::FieldRegen | Core::Recharge)
}

fn scaled(base: i32, factor: f32) -> i32 {
    (base as f32 * factor).round() as i32
}

/// Per-target magnitude factor of the Area part, 1.0 for none.
fn area_magnitude(area: Option<AreaKind>) -> f32 {
    match area {
        None => 1.0,
        Some(AreaKind::Group) => t::CRAFT_AREA_GROUP_MAGNITUDE,
        Some(AreaKind::Everyone) => t::CRAFT_AREA_EVERYONE_MAGNITUDE,
    }
}

fn target_of(s: &Split) -> AbilityTarget {
    let field_run_scoped =
        field_kind(s.core).is_some_and(|k| k.scope() == crate::components::FieldScope::Run);
    match s.core.facing() {
        _ if field_run_scoped => AbilityTarget::WholeParty,
        super::catalogue::Facing::Neither => AbilityTarget::WholeParty,
        _ if matches!(
            s.core,
            Core::Fork | Core::Respawn | Core::Reinforce | Core::Emulate
        ) =>
        {
            AbilityTarget::WholeParty
        }
        super::catalogue::Facing::Ally => match s.area {
            Some(_) => AbilityTarget::WholeParty,
            None => AbilityTarget::OneAlly,
        },
        super::catalogue::Facing::Enemy => match s.area {
            None => AbilityTarget::OneEnemyGroupFront,
            Some(AreaKind::Group) => AbilityTarget::WholeEnemyGroup,
            Some(AreaKind::Everyone) => AbilityTarget::AllEnemies,
        },
    }
}

fn effect_of(s: &Split) -> AbilityEffect {
    let fig = core_figures(s.core);
    let v = s.version.index();
    let vmult = t::CRAFT_VERSION_MAGNITUDE[v];
    let mult = vmult * area_magnitude(s.area);
    let power = scaled(fig.magnitude, mult).max(1);
    let spread = scaled(fig.spread, mult);
    let rounds = fig.rounds + s.trickles * t::CRAFT_TRICKLE_ROUNDS;
    match s.core {
        Core::Damage => AbilityEffect::Damage {
            power,
            spread,
            status: s.status.map(|kind| {
                let (chance, duration, power) = t::CRAFT_RIDER[kind.index()];
                MoveEffect {
                    kind: kind.id(),
                    chance,
                    duration,
                    power,
                }
            }),
            damage_type: s.damage_type,
        },
        Core::Heal if s.trickles == 0 => AbilityEffect::Heal { power, spread },
        Core::Heal => AbilityEffect::HealOverTime {
            power: scaled(fig.magnitude, mult * t::CRAFT_HOT_SHARE_PER_ROUND).max(1),
            spread: scaled(fig.spread, mult * t::CRAFT_HOT_SHARE_PER_ROUND),
            rounds: s.trickles * t::CRAFT_HOT_ROUNDS_PER_TRICKLE,
        },
        Core::Drain => AbilityEffect::Drain {
            power,
            spread,
            heal_fraction: t::CRAFT_DRAIN_HEAL_FRACTION[v],
            damage_type: s.damage_type,
        },
        Core::BoostAttack => AbilityEffect::Buff {
            kind: BuffKind::Atk,
            power,
            duration: rounds,
        },
        Core::BoostMitigation => AbilityEffect::Buff {
            kind: BuffKind::Mitigation,
            power,
            duration: rounds,
        },
        Core::SapAttack => AbilityEffect::Buff {
            kind: BuffKind::Atk,
            power: -power,
            duration: rounds,
        },
        Core::Debuff => {
            let kind = s.status.unwrap_or(StatusKind::Stun);
            let (power, base_rounds) = debuff_base(kind);
            AbilityEffect::Debuff {
                kind: kind.id(),
                power,
                duration: base_rounds + s.trickles * t::CRAFT_TRICKLE_ROUNDS,
            }
        }
        Core::Cleanse => AbilityEffect::Cleanse,
        Core::Cloak => AbilityEffect::Cloak { duration: rounds },
        Core::Fork => {
            let (count, extra, rarity_penalty) = t::CRAFT_FORK_BODIES[v];
            AbilityEffect::Summon {
                count,
                extra,
                rarity_penalty,
            }
        }
        Core::Respawn => AbilityEffect::Reanimate {
            count: s.version.number(),
        },
        Core::ColdSample => AbilityEffect::Tamper {
            kind: TamperKind::Temperature(t::CRAFT_COLD_TEMPERATURE),
            duration: rounds,
        },
        Core::HeatInjection => AbilityEffect::Tamper {
            kind: TamperKind::Temperature(t::CRAFT_HEAT_TEMPERATURE),
            duration: rounds,
        },
        Core::Profile => AbilityEffect::Tamper {
            kind: TamperKind::Profiled,
            duration: rounds,
        },
        Core::Inject => AbilityEffect::Tamper {
            kind: TamperKind::Injected,
            duration: rounds,
        },
        Core::Hallucinate => AbilityEffect::Tamper {
            kind: TamperKind::Hallucinating {
                decoys: t::CRAFT_HALLUCINATE_DECOYS,
            },
            duration: rounds,
        },
        Core::Teleport => AbilityEffect::Teleport,
        Core::Reinforce => AbilityEffect::Reinforce,
        Core::Emulate => AbilityEffect::Emulate { rounds: fig.rounds },
        Core::Phase => AbilityEffect::Phase,
        Core::Jump => AbilityEffect::Jump,
        Core::Symlink => AbilityEffect::Symlink,
        Core::FieldRegen
        | Core::Recharge
        | Core::FieldAttack
        | Core::FieldMitigation
        | Core::CaptureBoost
        | Core::XpBoost
        | Core::EncounterDamp
        | Core::DropBoost => AbilityEffect::FieldBuff {
            kind: field_kind(s.core).expect("every field Core names a kind"),
            power: scaled(fig.magnitude, vmult).max(1),
            duration: match field_is_timed(s.core) {
                true => t::CRAFT_FIELD_DURATION + s.trickles * t::CRAFT_TRICKLE_FIELD_DURATION,
                false => 0,
            },
            interval: match s.core {
                Core::FieldRegen => t::CRAFT_FIELD_REGEN_INTERVAL,
                _ => every_turn(),
            },
        },
    }
}

fn power_cost(s: &Split) -> f32 {
    let fig = core_figures(s.core);
    let area = match s.area {
        None => 0.0,
        Some(AreaKind::Group) => t::CRAFT_AREA_GROUP_POWER,
        Some(AreaKind::Everyone) => t::CRAFT_AREA_EVERYONE_POWER,
    };
    let range = match s.range {
        None => 0.0,
        Some(RangeKind::Near) => t::CRAFT_RANGE_NEAR_POWER,
        Some(RangeKind::Far) => t::CRAFT_RANGE_FAR_POWER,
    };
    let rider = match (s.core, s.status) {
        (Core::Damage, Some(_)) => t::CRAFT_RIDER_POWER,
        _ => 0.0,
    };
    let damage_type = s.damage_type.map_or(0.0, |_| t::CRAFT_DAMAGE_TYPE_POWER);
    fig.power_cost
        + s.version.index() as f32 * fig.cost_per_version
        + area
        + range
        + rider
        + damage_type
        + s.trickles as f32 * t::CRAFT_TRICKLE_POWER
}

fn accuracy(s: &Split) -> i32 {
    let fig = core_figures(s.core);
    if fig.accuracy == 0 {
        return 0;
    }
    let area = match s.area {
        None => 0,
        Some(AreaKind::Group) => t::CRAFT_AREA_GROUP_ACCURACY,
        Some(AreaKind::Everyone) => t::CRAFT_AREA_EVERYONE_ACCURACY,
    };
    (fig.accuracy + area).max(0)
}

fn cooldown(s: &Split) -> u32 {
    let fig = core_figures(s.core);
    fig.cooldown
        + s.version.index() as u32 * fig.cooldown_per_version
        + s.area.map_or(0, |_| t::CRAFT_AREA_COOLDOWN)
        + s.trickles * t::CRAFT_TRICKLE_COOLDOWN
}

/// The def a built routine runs as. The only place parts become numbers.
///
/// **The build must have passed `validate_build`** — the screens and the
/// save loader both hold that, and a Core-less build is the one input with
/// no def to return.
pub fn compile(routine: &CraftedRoutine) -> AbilityDef {
    let s = split(&routine.parts).expect("a built routine has a Core: validate_build ran first");
    let target = target_of(&s);
    let effect = effect_of(&s);
    let field_only = effect.field_only();
    let reach = match (s.core, s.range) {
        (Core::Respawn, _) => Some(t::CRAFT_RESPAWN_REACH),
        (_, Some(RangeKind::Near)) => Some(t::CRAFT_RANGE_NEAR),
        (_, Some(RangeKind::Far)) => Some(t::CRAFT_RANGE_FAR),
        (_, None) => None,
    };
    let shape = match s.core {
        Core::Respawn => Some(AbilityShape::Radius {
            radius: t::CRAFT_RESPAWN_RADIUS,
        }),
        _ => s.area.map(|_| target.derived_shape()),
    };
    let mut def = AbilityDef {
        id: routine.id.clone(),
        name: routine.name.clone(),
        description: String::new(),
        target,
        effect,
        // A field routine has no cooldown; the loader warns on one.
        cooldown: if field_only { 0 } else { cooldown(&s) },
        power_cost: power_cost(&s),
        accuracy: accuracy(&s),
        wild_weight: 0,
        research_zone: 0,
        exclusive: false,
        starter: false,
        ranged: s.range.is_some(),
        boss_drop: None,
        triggers: None,
        shape,
        range: reach.map(|max| AbilityRange { min: 0, max }),
        fx: None,
        charge: None,
    };
    def.description = super::describe::sentence(&def, &s);
    def
}
