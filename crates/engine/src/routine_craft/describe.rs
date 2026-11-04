//! A built routine's description, generated from the def its parts compile
//! to so the sentence and the numbers cannot disagree.

use super::catalogue::{AreaKind, RangeKind};
use super::catalogue::{StatusKind, Subroutine};
use super::compile::{Split, area_accuracy, area_magnitude, split};
use crate::abilities::{AbilityDef, AbilityEffect, AbilityTarget, TamperKind, range_label};
use crate::components::BuffKind;
use crate::components::FieldBuffKind;
use crate::tuning as t;

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
        AbilityEffect::Haste { version } => {
            let bonus = t::haste_move_bonus(*version);
            match *version >= t::HASTE_DOUBLE_SWING_VERSION {
                true => format!(
                    "Haste {to}: +{bonus} movement on the battle map for the rest of the fight, and every swing strikes twice"
                ),
                false => format!(
                    "Haste {to}: +{bonus} movement on the battle map for the rest of the fight"
                ),
            }
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

/// What one part does on its own — the routine research menu's line. A Core
/// is `describe` of the Core alone, so the row quotes the routine a bare
/// build of it compiles to; a modifier says what it adds to any Core.
pub fn part_effect(part: &Subroutine) -> String {
    match *part {
        Subroutine::Core(..) => describe(&[*part]),
        Subroutine::Range(kind) => {
            let cells = match kind {
                RangeKind::Near => t::CRAFT_RANGE_NEAR,
                RangeKind::Far => t::CRAFT_RANGE_FAR,
            };
            format!("Can be aimed from up to {cells} cells away on a battle map.")
        }
        Subroutine::Area(kind) => {
            let whom = match kind {
                AreaKind::Group => "a whole enemy group",
                AreaKind::Everyone => "every enemy",
            };
            let pct = (area_magnitude(Some(kind)) * 100.0).round() as i32;
            format!(
                "Reaches {whom} (or the whole party, for a helpful routine) at {pct}% strength. \
                 Accuracy {:+}, recharges {} slower.",
                area_accuracy(Some(kind)),
                rounds(t::CRAFT_AREA_COOLDOWN),
            )
        }
        Subroutine::Trickle => format!(
            "Makes a timed effect last {} longer, or spreads a heal over {}. Stacks; \
             recharges {} slower each.",
            rounds(t::CRAFT_TRICKLE_ROUNDS),
            rounds(t::CRAFT_HOT_ROUNDS_PER_TRICKLE),
            rounds(t::CRAFT_TRICKLE_COOLDOWN),
        ),
        Subroutine::DamageType(ty) => format!("The damage it deals is {}.", ty.name()),
        Subroutine::Status(kind) => {
            let (chance, duration, _) = t::CRAFT_RIDER[kind.index()];
            format!(
                "On Damage, a {:.0}% chance to leave {} for {}; on Debuff, inflicts it.",
                chance * 100.0,
                kind.name(),
                rounds(duration),
            )
        }
    }
}

fn rounds(n: u32) -> String {
    format!("{n} round{}", if n == 1 { "" } else { "s" })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::routine_craft::{Core, Version};

    #[test]
    fn a_core_reads_as_the_routine_it_builds_alone() {
        let core = Subroutine::Core(Core::Damage, Version::V2);
        assert_eq!(part_effect(&core), describe(&[core]));
    }

    #[test]
    fn every_part_says_something_of_its_own() {
        let mut seen = std::collections::BTreeSet::new();
        for part in Subroutine::all() {
            let line = part_effect(&part);
            assert!(!line.is_empty(), "{part:?} has no effect line");
            // Two unversioned Cores can describe the same bare routine;
            // modifiers must each be distinct.
            if !matches!(part, Subroutine::Core(..)) {
                assert!(seen.insert(line.clone()), "{part:?} repeats {line:?}");
            }
        }
    }

    #[test]
    fn a_modifier_quotes_its_figures() {
        let far = part_effect(&Subroutine::Range(RangeKind::Far));
        assert!(
            far.contains(&format!("{} cells", t::CRAFT_RANGE_FAR)),
            "{far}"
        );
        let group = part_effect(&Subroutine::Area(AreaKind::Group));
        let pct = (t::CRAFT_AREA_GROUP_MAGNITUDE * 100.0).round() as i32;
        assert!(group.contains(&format!("{pct}%")), "{group}");
        let bleed = part_effect(&Subroutine::Status(StatusKind::Bleed));
        let (chance, _, _) = t::CRAFT_RIDER[StatusKind::Bleed.index()];
        assert!(
            bleed.contains(&format!("{:.0}%", chance * 100.0)),
            "{bleed}"
        );
    }

    #[test]
    fn a_part_node_carries_its_effect() {
        for node in crate::routine_tree::synthesise_part_nodes() {
            let Some(key) = node.id.strip_prefix("part/") else {
                continue;
            };
            let part = Subroutine::from_key(key).unwrap();
            assert_eq!(node.description, part_effect(&part), "{}", node.id);
        }
    }
}
