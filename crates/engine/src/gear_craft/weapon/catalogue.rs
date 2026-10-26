//! The weapon part catalogue: which parts exist and which category each
//! fills. Pure data and predicates; what fits what is `fit`'s and the
//! numbers a design becomes are `compile`'s.

use serde::{Deserialize, Serialize};

use crate::abilities::AbilityId;
use crate::damage::DamageType;
use crate::routine_craft::{AreaKind, StatusKind, Version};

/// The weapon's family and base numbers. Exactly one per design.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Frame {
    Edge,
    Lance,
    Whip,
    Launcher,
}

impl Frame {
    pub const ALL: [Frame; 4] = [Frame::Edge, Frame::Lance, Frame::Whip, Frame::Launcher];

    /// Position in the per-frame tuning tables.
    pub fn index(self) -> usize {
        match self {
            Frame::Edge => 0,
            Frame::Lance => 1,
            Frame::Whip => 2,
            Frame::Launcher => 3,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Frame::Edge => "Edge",
            Frame::Lance => "Lance",
            Frame::Whip => "Whip",
            Frame::Launcher => "Launcher",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Tilt {
    /// Accuracy for band.
    Precise,
    /// Band for accuracy.
    Heavy,
}

/// The routines an authored weapon grants today, the only ones a Proc part
/// may name (the user's decision).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Proc {
    ClockSkew,
    CoreDump,
    InterruptRequest,
}

impl Proc {
    pub const ALL: [Proc; 3] = [Proc::ClockSkew, Proc::CoreDump, Proc::InterruptRequest];

    pub fn index(self) -> usize {
        match self {
            Proc::ClockSkew => 0,
            Proc::CoreDump => 1,
            Proc::InterruptRequest => 2,
        }
    }

    pub fn ability(self) -> AbilityId {
        String::from(match self {
            Proc::ClockSkew => "clock_skew",
            Proc::CoreDump => "core_dump",
            Proc::InterruptRequest => "interrupt_request",
        })
    }

    pub fn from_ability(id: &AbilityId) -> Option<Proc> {
        Proc::ALL.into_iter().find(|p| p.ability() == *id)
    }
}

/// What a part fills. A design holds at most one part per category, except
/// affixes (which are not weapon parts and are bounded by the cap alone).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Category {
    Frame,
    Range,
    Reach,
    DamageType,
    Rider,
    Exploit,
    Tilt,
    Capture,
    Proc,
}

impl Category {
    /// Every category a modifier can fill, in the order a screen lists them.
    pub const MODIFIERS: [Category; 8] = [
        Category::Range,
        Category::Reach,
        Category::DamageType,
        Category::Rider,
        Category::Exploit,
        Category::Tilt,
        Category::Capture,
        Category::Proc,
    ];
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum WeaponPart {
    Frame(Frame, Version),
    /// +1 range.
    Extend,
    Reach(AreaKind),
    DamageType(DamageType),
    /// Setter: a chance to arm the status on a landed hit.
    Rider(StatusKind),
    /// Payoff: more damage against a target carrying the status.
    Exploit(StatusKind),
    Tilt(Tilt),
    /// Decompiler, as `shim_blade` and `black_ice_pick` carry.
    Capture,
    Proc(Proc),
}

impl WeaponPart {
    pub fn category(self) -> Category {
        match self {
            WeaponPart::Frame(..) => Category::Frame,
            WeaponPart::Extend => Category::Range,
            WeaponPart::Reach(_) => Category::Reach,
            WeaponPart::DamageType(_) => Category::DamageType,
            WeaponPart::Rider(_) => Category::Rider,
            WeaponPart::Exploit(_) => Category::Exploit,
            WeaponPart::Tilt(_) => Category::Tilt,
            WeaponPart::Capture => Category::Capture,
            WeaponPart::Proc(_) => Category::Proc,
        }
    }

    /// Every part there is, frames first. Stun is not offered as a rider
    /// (`stun-costs-no-turn-on-a-battle-map`) and is offered as an exploit.
    pub fn all() -> Vec<WeaponPart> {
        let mut parts = Vec::new();
        for frame in Frame::ALL {
            for version in Version::ALL {
                parts.push(WeaponPart::Frame(frame, version));
            }
        }
        parts.push(WeaponPart::Extend);
        parts.extend(
            [AreaKind::Group, AreaKind::Everyone]
                .into_iter()
                .map(WeaponPart::Reach),
        );
        parts.extend(DamageType::ALL.into_iter().map(WeaponPart::DamageType));
        parts.extend(
            StatusKind::ALL
                .into_iter()
                .filter(|s| *s != StatusKind::Stun)
                .map(WeaponPart::Rider),
        );
        parts.extend(StatusKind::ALL.into_iter().map(WeaponPart::Exploit));
        parts.extend(
            [Tilt::Precise, Tilt::Heavy]
                .into_iter()
                .map(WeaponPart::Tilt),
        );
        parts.push(WeaponPart::Capture);
        parts.extend(Proc::ALL.into_iter().map(WeaponPart::Proc));
        parts
    }

    /// The parts a market or caravan may stock. Payoffs are found or
    /// researched, and a granted routine comes only from salvaging a weapon
    /// that carried it; higher frame versions are research too.
    pub fn shelf() -> Vec<WeaponPart> {
        WeaponPart::all()
            .into_iter()
            .filter(|part| match part {
                WeaponPart::Frame(_, version) => *version == Version::V1,
                WeaponPart::Exploit(_) | WeaponPart::Proc(_) => false,
                _ => true,
            })
            .collect()
    }

    /// The parts of one category, in `all()` order.
    pub fn options(category: Category) -> Vec<WeaponPart> {
        WeaponPart::all()
            .into_iter()
            .filter(|p| p.category() == category)
            .collect()
    }
}
