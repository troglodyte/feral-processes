//! A built routine's description, generated from the def its parts compile
//! to so the sentence and the numbers cannot disagree.

use super::catalogue::{StatusKind, Subroutine};
use super::compile::{Split, split};
use crate::abilities::{AbilityDef, AbilityEffect, AbilityTarget, TamperKind, range_label};
use crate::components::BuffKind;
use crate::components::FieldBuffKind;

/// The plain-language line a screen previews before a build is compiled.
/// Empty when `parts` holds no Core.
pub fn describe(parts: &[Subroutine]) -> String {
    let Some(s) = split(parts) else {
        return String::new();
    };
    let def = super::compile::compile(&super::catalogue::CraftedRoutine {
        id: String::new(),
        name: String::new(),
        parts: parts.to_vec(),
    });
    sentence(&def, &s)
}

fn recipient(target: AbilityTarget, ally_one: &str) -> String {
    match target {
        AbilityTarget::OneAlly => ally_one.to_string(),
        AbilityTarget::WholeParty => "the whole party".into(),
        AbilityTarget::OneEnemyGroupFront => "one target".into(),
        AbilityTarget::WholeEnemyGroup => "every member of one group".into(),
        AbilityTarget::AllEnemies => "every hostile program on the field".into(),
    }
}

fn band(power: i32, spread: i32) -> String {
    range_label(crate::battle::DamageRange::centred(power, spread))
}

/// The sentence for `def`, which `compile` built from `s`.
pub(super) fn sentence(def: &AbilityDef, s: &Split) -> String {
    let to = recipient(def.target, "one ally or yourself");
    let mut line = match &def.effect {
        AbilityEffect::Damage {
            power,
            spread,
            status,
            damage_type,
        } => {
            let kind = damage_type.map_or(String::new(), |t| format!(" {}", t.name()));
            let mut line = format!("Damage {}{kind} to {to}", band(*power, *spread));
            if let (Some(rider), Some(kind)) = (status, s.status) {
                line.push_str(&format!(
                    ", with a {:.0}% chance of {}",
                    rider.chance * 100.0,
                    kind.name()
                ));
            }
            line
        }
        AbilityEffect::Heal { power, spread } => {
            format!("Restore {} Integrity to {to}", band(*power, *spread))
        }
        AbilityEffect::HealOverTime {
            power,
            spread,
            rounds,
        } => format!(
            "Restore {} Integrity to {to} each round for {rounds} rounds",
            band(*power, *spread)
        ),
        AbilityEffect::Drain {
            power,
            spread,
            heal_fraction,
            damage_type,
        } => {
            let kind = damage_type.map_or(String::new(), |t| format!(" {}", t.name()));
            format!(
                "Damage {}{kind} to {to} and restore {:.0}% of it to yourself",
                band(*power, *spread),
                heal_fraction * 100.0
            )
        }
        AbilityEffect::Buff {
            kind,
            power,
            duration,
        } => match (kind, *power >= 0) {
            (BuffKind::Atk, true) => format!("+{power} ATK to {to} for {duration} rounds"),
            (BuffKind::Atk, false) => format!("{power} ATK to {to} for {duration} rounds"),
            (BuffKind::Mitigation, true) => {
                format!("-{power}% incoming damage to {to} for {duration} rounds")
            }
            (BuffKind::Mitigation, false) => {
                format!("+{}% damage taken by {to} for {duration} rounds", -power)
            }
        },
        AbilityEffect::Debuff { duration, .. } => {
            let what = s.status.map_or("a condition", StatusKind::name);
            format!("Inflict {what} on {to} for {duration} rounds")
        }
        AbilityEffect::Cleanse => format!("Clear status conditions from {to}"),
        AbilityEffect::Cloak { duration } => {
            format!("Hide {to} from targeting for {duration} rounds")
        }
        AbilityEffect::Summon { count, extra, .. } => match extra {
            0 => format!("Fork {count} temporary program into the fight"),
            _ => format!(
                "Fork {} to {} temporary programs into the fight",
                count,
                count + extra
            ),
        },
        AbilityEffect::Reanimate { count } => {
            format!("Raise up to {count} fallen bodies onto your side")
        }
        AbilityEffect::Tamper { kind, duration } => match kind {
            TamperKind::Temperature(temp) if *temp <= crate::tuning::TACTICAL_AI_TEMPERATURE => {
                format!("Pin {to}'s decisions at their best for {duration} rounds")
            }
            TamperKind::Temperature(_) => {
                format!("Randomise {to}'s decisions for {duration} rounds")
            }
            TamperKind::Profiled => format!("Publish {to}'s next move for {duration} rounds"),
            TamperKind::Injected => {
                format!("Turn {to} against its own side for {duration} rounds")
            }
            TamperKind::Hallucinating { decoys } => format!(
                "Seed {to} with {decoys} decoys it mistakes for real, for {duration} rounds"
            ),
        },
        AbilityEffect::Teleport => "Relocate yourself or a body at arm's length".into(),
        AbilityEffect::Reinforce => "Call a Drop Trooper into the fight".into(),
        AbilityEffect::Emulate { rounds } => {
            format!("Adopt a known image's kit for {rounds} rounds")
        }
        AbilityEffect::Phase => "Step through the wall in front of you".into(),
        AbilityEffect::Jump => "Jump to a cell you point at".into(),
        AbilityEffect::Symlink => "Return the party to your base".into(),
        AbilityEffect::FieldBuff {
            kind,
            power,
            duration,
            interval,
        } => field_sentence(*kind, *power, *duration, *interval, &to),
        AbilityEffect::Decompile => "Attempt a capture".into(),
    };
    if let Some(range) = def.range.filter(|_| s.range.is_some()) {
        line.push_str(&format!(" within {} cells", range.max));
    }
    line
}

fn field_sentence(
    kind: FieldBuffKind,
    power: i32,
    duration: u32,
    interval: u32,
    to: &str,
) -> String {
    let every = match interval {
        0 | 1 => "each turn".to_string(),
        n => format!("every {n}th turn"),
    };
    match kind {
        FieldBuffKind::Regen => {
            format!("+{power} Integrity to {to} {every}, for {duration} turns")
        }
        FieldBuffKind::Trickle => format!("+{power} Power to {to} {every}, for {duration} turns"),
        FieldBuffKind::Atk => format!("+{power} ATK to {to} until you rest"),
        FieldBuffKind::Mitigation => {
            format!("-{power}% incoming damage to {to} until you rest")
        }
        FieldBuffKind::CaptureBoost => format!("+{power}% capture chance until you rest"),
        FieldBuffKind::XpBoost => format!("+{power}% XP gained until you rest"),
        FieldBuffKind::EncounterDamp => format!("-{power}% wild encounter chance until you rest"),
        FieldBuffKind::DropBoost => format!("+{power}% drop chance until you rest"),
    }
}
