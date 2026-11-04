//! What one weapon part does, in words, read off the same `PartDelta`
//! `compile` folds — so a research node and the weapon it builds cannot
//! quote different numbers.

use super::catalogue::{Tilt, WeaponPart};
use super::compile::{exact_band, part_delta, reach_index, rounded_band, scaled};
use crate::routine_craft::AreaKind;
use crate::tuning as t;

impl WeaponPart {
    /// "30% chance a hit leaves bleeding for 2 rounds. Accuracy −1,
    /// damage −15%." — the research menu's description of this part.
    pub fn effect(self) -> String {
        if let WeaponPart::Frame(frame, version) = self {
            let fig = t::GEAR_FRAME[frame.index()];
            let band = rounded_band(exact_band(frame, version, 1.0));
            let atk = scaled(fig.atk, t::GEAR_VERSION_MAGNITUDE[version.index()]);
            return format!(
                "The base of a design: hits for {}, {atk:+} ATK, accuracy {:+}, from {} cell{} away.",
                crate::damage::band_label(band, None),
                fig.accuracy,
                fig.range,
                if fig.range == 1 { "" } else { "s" },
            );
        }
        let d = part_delta(self);
        let what = match self {
            WeaponPart::Frame(..) => unreachable!("handled above"),
            WeaponPart::Extend => format!("Strikes from {} cell further away.", d.range),
            WeaponPart::Reach(kind) => {
                let rounds = t::GEAR_REACH_RECHARGE[reach_index(kind)];
                let whom = match kind {
                    AreaKind::Group => "every member of one group",
                    AreaKind::Everyone => "every enemy",
                };
                format!("Hits {whom}, resting {rounds} rounds between swings.")
            }
            WeaponPart::DamageType(ty) => format!("Deals {} damage.", ty.name()),
            WeaponPart::Rider(status) => {
                let (chance, rounds, _) = t::GEAR_RIDER[status.index()];
                format!(
                    "{:.0}% chance a hit leaves {} for {rounds} rounds.",
                    chance * 100.0,
                    status.name()
                )
            }
            WeaponPart::Exploit(status) => format!(
                "Hits {}% harder against a target with {}.",
                t::GEAR_EXPLOIT_PERCENT[status.index()],
                status.name()
            ),
            WeaponPart::Tilt(Tilt::Precise) => "Trades damage for accuracy.".to_string(),
            WeaponPart::Tilt(Tilt::Heavy) => "Trades accuracy for damage.".to_string(),
            WeaponPart::Capture => format!(
                "{:+} decompiler: taming a program is more likely to succeed.",
                d.decompiler
            ),
            WeaponPart::Proc(proc) => format!("Grants the {} routine.", proc.label()),
        };
        let mut costs = Vec::new();
        if d.accuracy != 0 {
            costs.push(format!("accuracy {:+}", d.accuracy));
        }
        let pct = ((d.band_factor - 1.0) * 100.0).round() as i32;
        if pct != 0 {
            costs.push(format!("damage {pct:+}%"));
        }
        if costs.is_empty() {
            return what;
        }
        let costs = costs.join(", ");
        let mut chars = costs.chars();
        let first = chars.next().map(|c| c.to_uppercase().collect::<String>());
        format!("{what} {}{}.", first.unwrap_or_default(), chars.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gear_craft::weapon::{Frame, Proc};
    use crate::routine_craft::{StatusKind, Version};

    #[test]
    fn every_part_says_something_of_its_own() {
        let mut seen = std::collections::BTreeSet::new();
        for part in WeaponPart::all() {
            let line = part.effect();
            assert!(!line.is_empty(), "{part:?} has no effect line");
            assert!(seen.insert(line.clone()), "{part:?} repeats {line:?}");
        }
    }

    #[test]
    fn a_frame_quotes_the_band_its_version_builds() {
        let line = WeaponPart::Frame(Frame::Lance, Version::V3).effect();
        let band = rounded_band(exact_band(Frame::Lance, Version::V3, 1.0));
        let label = crate::damage::band_label(band, None);
        assert!(line.contains(&label), "{line:?} lacks {label:?}");
        let range = t::GEAR_FRAME[Frame::Lance.index()].range;
        assert!(line.contains(&format!("{range} cells away")), "{line}");
    }

    #[test]
    fn a_modifier_quotes_its_own_costs() {
        let line = WeaponPart::Extend.effect();
        assert!(
            line.contains(&format!("Accuracy {:+}", t::GEAR_EXTEND_ACCURACY)),
            "{line}"
        );
        let heavy = WeaponPart::Tilt(Tilt::Heavy).effect();
        let pct = ((t::GEAR_TILT_HEAVY_BAND - 1.0) * 100.0).round() as i32;
        assert!(heavy.contains(&format!("damage {pct:+}%")), "{heavy}");
    }

    #[test]
    fn a_rider_quotes_its_chance_and_an_exploit_its_payoff() {
        let bleed = StatusKind::Bleed;
        let (chance, rounds, _) = t::GEAR_RIDER[bleed.index()];
        let rider = WeaponPart::Rider(bleed).effect();
        let want = format!("{:.0}% chance", chance * 100.0);
        assert!(rider.contains(&want), "{rider}");
        assert!(rider.contains(&format!("{rounds} rounds")), "{rider}");
        let exploit = WeaponPart::Exploit(StatusKind::Stun).effect();
        let pct = t::GEAR_EXPLOIT_PERCENT[StatusKind::Stun.index()];
        assert!(exploit.contains(&format!("{pct}% harder")), "{exploit}");
    }

    #[test]
    fn reach_and_proc_name_what_they_add() {
        let group = WeaponPart::Reach(AreaKind::Group).effect();
        assert!(group.contains("one group"), "{group}");
        let proc = WeaponPart::Proc(Proc::CoreDump).effect();
        assert!(proc.contains("Core Dump"), "{proc}");
    }
}
