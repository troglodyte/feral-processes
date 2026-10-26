//! What a weapon part is called: the key its item id and research node are
//! built from, and the label a screen prints.

use super::catalogue::{Category, Proc, Tilt, WeaponPart};
use crate::routine_craft::AreaKind;
use crate::tuning as t;

impl Proc {
    fn key(self) -> &'static str {
        match self {
            Proc::ClockSkew => "clock_skew",
            Proc::CoreDump => "core_dump",
            Proc::InterruptRequest => "interrupt_request",
        }
    }

    fn label(self) -> &'static str {
        match self {
            Proc::ClockSkew => "Clock Skew",
            Proc::CoreDump => "Core Dump",
            Proc::InterruptRequest => "Interrupt Request",
        }
    }
}

fn capitalised(word: &str) -> String {
    let mut chars = word.chars();
    chars
        .next()
        .map(|c| c.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}

impl Category {
    /// The word a design row is headed with.
    pub fn label(self) -> &'static str {
        match self {
            Category::Frame => "Frame",
            Category::Range => "Range",
            Category::Reach => "Reach",
            Category::DamageType => "Damage type",
            Category::Rider => "Rider",
            Category::Exploit => "Exploit",
            Category::Tilt => "Tilt",
            Category::Capture => "Decompiler",
            Category::Proc => "Proc",
        }
    }
}

impl WeaponPart {
    /// Unique per part, and what the part's item id and research node end in.
    pub fn key(self) -> String {
        match self {
            WeaponPart::Frame(frame, version) => {
                format!("frame.{}.{}", frame.name().to_lowercase(), version.number())
            }
            WeaponPart::Extend => "extend".into(),
            WeaponPart::Reach(AreaKind::Group) => "reach.group".into(),
            WeaponPart::Reach(AreaKind::Everyone) => "reach.everyone".into(),
            WeaponPart::DamageType(ty) => format!("type.{}", ty.name().to_lowercase()),
            WeaponPart::Rider(status) => format!("rider.{}", status.id().0),
            WeaponPart::Exploit(status) => format!("exploit.{}", status.id().0),
            WeaponPart::Tilt(Tilt::Precise) => "tilt.precise".into(),
            WeaponPart::Tilt(Tilt::Heavy) => "tilt.heavy".into(),
            WeaponPart::Capture => "capture".into(),
            WeaponPart::Proc(proc) => format!("proc.{}", proc.key()),
        }
    }

    /// The part a key names — the inverse of `key`.
    pub fn from_key(key: &str) -> Option<WeaponPart> {
        WeaponPart::all().into_iter().find(|p| p.key() == key)
    }

    /// "Edge v2", "Rider: Bleed" — the words a list row carries.
    pub fn label(self) -> String {
        match self {
            WeaponPart::Frame(frame, version) => {
                format!("{} v{}", frame.name(), version.number())
            }
            WeaponPart::Extend => "Extend".into(),
            WeaponPart::Reach(AreaKind::Group) => "Reach: Group".into(),
            WeaponPart::Reach(AreaKind::Everyone) => "Reach: Everyone".into(),
            WeaponPart::DamageType(ty) => format!("Type: {}", ty.name()),
            WeaponPart::Rider(status) => format!("Rider: {}", capitalised(&status.id().0)),
            WeaponPart::Exploit(status) => format!("Exploit: {}", capitalised(&status.id().0)),
            WeaponPart::Tilt(Tilt::Precise) => "Tilt: Precise".into(),
            WeaponPart::Tilt(Tilt::Heavy) => "Tilt: Heavy".into(),
            WeaponPart::Capture => "Decompiler".into(),
            WeaponPart::Proc(proc) => format!("Proc: {}", proc.label()),
        }
    }

    /// The research node that lets the player craft this part.
    pub fn node_id(self) -> String {
        format!("{}{}", t::GEAR_PART_ID_PREFIX, self.key())
    }
}
