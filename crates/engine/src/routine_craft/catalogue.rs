//! The part catalogue: which Cores and modifiers exist, which modifier fits
//! which Core, and what a build may hold. Pure data and predicates; the
//! numbers a build becomes are `compile`'s.

use serde::{Deserialize, Serialize};

use crate::abilities::AbilityId;
use crate::damage::DamageType;
use crate::items::ItemId;
use crate::statuses::StatusId;
use crate::tuning::{CRAFT_MATERIAL_BYTECODE, CRAFT_MATERIAL_CELL, CRAFT_MATERIAL_FRAGMENT};

/// What a routine does. One per routine.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Core {
    Damage,
    Heal,
    Drain,
    BoostAttack,
    BoostMitigation,
    SapAttack,
    SapMitigation,
    Debuff,
    Cleanse,
    Cloak,
    Fork,
    Respawn,
    ColdSample,
    HeatInjection,
    Profile,
    Inject,
    Hallucinate,
    Teleport,
    Reinforce,
    Emulate,
    FieldRegen,
    Recharge,
    FieldAttack,
    FieldMitigation,
    CaptureBoost,
    XpBoost,
    EncounterDamp,
    DropBoost,
    Phase,
    Jump,
    Symlink,
}

/// Which side a Core's effect lands on, which is how an Area part reads.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    /// One ally or yourself; a Group is the whole party.
    Ally,
    /// One enemy; a Group is one group and Everyone every enemy.
    Enemy,
    /// Names no target at all.
    Neither,
}

impl Core {
    pub const ALL: [Core; 31] = [
        Core::Damage,
        Core::Heal,
        Core::Drain,
        Core::BoostAttack,
        Core::BoostMitigation,
        Core::SapAttack,
        Core::SapMitigation,
        Core::Debuff,
        Core::Cleanse,
        Core::Cloak,
        Core::Fork,
        Core::Respawn,
        Core::ColdSample,
        Core::HeatInjection,
        Core::Profile,
        Core::Inject,
        Core::Hallucinate,
        Core::Teleport,
        Core::Reinforce,
        Core::Emulate,
        Core::FieldRegen,
        Core::Recharge,
        Core::FieldAttack,
        Core::FieldMitigation,
        Core::CaptureBoost,
        Core::XpBoost,
        Core::EncounterDamp,
        Core::DropBoost,
        Core::Phase,
        Core::Jump,
        Core::Symlink,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Core::Damage => "Damage",
            Core::Heal => "Heal",
            Core::Drain => "Drain",
            Core::BoostAttack => "Boost Attack",
            Core::BoostMitigation => "Boost Mitigation",
            Core::SapAttack => "Sap Attack",
            Core::SapMitigation => "Sap Mitigation",
            Core::Debuff => "Debuff",
            Core::Cleanse => "Cleanse",
            Core::Cloak => "Cloak",
            Core::Fork => "Fork",
            Core::Respawn => "Respawn",
            Core::ColdSample => "Cold Sample",
            Core::HeatInjection => "Heat Injection",
            Core::Profile => "Profile",
            Core::Inject => "Inject",
            Core::Hallucinate => "Hallucinate",
            Core::Teleport => "Teleport",
            Core::Reinforce => "Reinforce",
            Core::Emulate => "Emulate",
            Core::FieldRegen => "Field Regen",
            Core::Recharge => "Recharge",
            Core::FieldAttack => "Field Attack",
            Core::FieldMitigation => "Field Mitigation",
            Core::CaptureBoost => "Capture Boost",
            Core::XpBoost => "XP Boost",
            Core::EncounterDamp => "Encounter Damp",
            Core::DropBoost => "Drop Boost",
            Core::Phase => "Phase",
            Core::Jump => "Jump",
            Core::Symlink => "Symlink",
        }
    }

    /// Whether this Core exists as v1.0, v2.0 and v3.0. The ones without a
    /// magnitude to grow do not.
    pub fn versioned(self) -> bool {
        match self {
            Core::Damage
            | Core::Heal
            | Core::Drain
            | Core::BoostAttack
            | Core::BoostMitigation
            | Core::SapAttack
            | Core::SapMitigation
            | Core::Fork
            | Core::Respawn
            | Core::FieldRegen
            | Core::Recharge
            | Core::FieldAttack
            | Core::FieldMitigation
            | Core::CaptureBoost
            | Core::XpBoost
            | Core::EncounterDamp
            | Core::DropBoost => true,
            Core::Debuff
            | Core::Cleanse
            | Core::Cloak
            | Core::ColdSample
            | Core::HeatInjection
            | Core::Profile
            | Core::Inject
            | Core::Hallucinate
            | Core::Teleport
            | Core::Reinforce
            | Core::Emulate
            | Core::Phase
            | Core::Jump
            | Core::Symlink => false,
        }
    }

    pub fn facing(self) -> Facing {
        match self {
            Core::Damage
            | Core::Drain
            | Core::SapAttack
            | Core::SapMitigation
            | Core::Debuff
            | Core::ColdSample
            | Core::HeatInjection
            | Core::Profile
            | Core::Inject
            | Core::Hallucinate => Facing::Enemy,
            Core::Heal
            | Core::BoostAttack
            | Core::BoostMitigation
            | Core::Cleanse
            | Core::Cloak
            | Core::Fork
            | Core::Respawn
            | Core::Reinforce
            | Core::Emulate
            | Core::FieldRegen
            | Core::Recharge
            | Core::FieldAttack
            | Core::FieldMitigation
            | Core::CaptureBoost
            | Core::XpBoost
            | Core::EncounterDamp
            | Core::DropBoost => Facing::Ally,
            Core::Teleport | Core::Phase | Core::Jump | Core::Symlink => Facing::Neither,
        }
    }

    /// Whether it can only be run outside a battle.
    pub fn is_field(self) -> bool {
        matches!(
            self,
            Core::FieldRegen
                | Core::Recharge
                | Core::FieldAttack
                | Core::FieldMitigation
                | Core::CaptureBoost
                | Core::XpBoost
                | Core::EncounterDamp
                | Core::DropBoost
                | Core::Phase
                | Core::Jump
                | Core::Symlink
        )
    }

    fn takes_range(self) -> bool {
        matches!(
            self,
            Core::Damage
                | Core::Heal
                | Core::Drain
                | Core::BoostAttack
                | Core::BoostMitigation
                | Core::SapAttack
                | Core::SapMitigation
                | Core::Debuff
                | Core::Cleanse
                | Core::ColdSample
                | Core::HeatInjection
                | Core::Profile
                | Core::Inject
                | Core::Hallucinate
        )
    }

    fn takes_trickle(self) -> bool {
        matches!(
            self,
            Core::Heal
                | Core::BoostAttack
                | Core::BoostMitigation
                | Core::SapAttack
                | Core::SapMitigation
                | Core::Debuff
                | Core::Cloak
                | Core::ColdSample
                | Core::HeatInjection
                | Core::Profile
                | Core::Inject
                | Core::Hallucinate
                | Core::FieldRegen
                | Core::Recharge
        )
    }

    /// The Area kinds this Core reads, empty when it takes none.
    pub fn areas(self) -> &'static [AreaKind] {
        match self {
            Core::Damage
            | Core::Drain
            | Core::SapAttack
            | Core::SapMitigation
            | Core::Debuff
            | Core::Hallucinate => &[AreaKind::Group, AreaKind::Everyone],
            Core::Heal
            | Core::BoostAttack
            | Core::BoostMitigation
            | Core::Cleanse
            | Core::Cloak
            | Core::FieldRegen
            | Core::Recharge => &[AreaKind::Group],
            _ => &[],
        }
    }

    /// What this Core cannot do without, if `parts` lacks it.
    fn missing(self, parts: &[Subroutine]) -> Option<&'static str> {
        let has_status = parts.iter().any(|p| matches!(p, Subroutine::Status(_)));
        let has_area = parts.iter().any(|p| matches!(p, Subroutine::Area(_)));
        match self {
            Core::Debuff if !has_status => Some("a Status"),
            Core::Hallucinate if !has_area => Some("an Area"),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum Version {
    V1,
    V2,
    V3,
}

impl Version {
    pub const ALL: [Version; 3] = [Version::V1, Version::V2, Version::V3];

    /// 1, 2 or 3: the figure a screen prints and the material count.
    pub fn number(self) -> u32 {
        self.index() as u32 + 1
    }

    /// Position in the per-version tuning tables.
    pub fn index(self) -> usize {
        match self {
            Version::V1 => 0,
            Version::V2 => 1,
            Version::V3 => 2,
        }
    }

    pub fn from_number(n: u32) -> Option<Version> {
        match n {
            1 => Some(Version::V1),
            2 => Some(Version::V2),
            3 => Some(Version::V3),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum RangeKind {
    Near,
    Far,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum AreaKind {
    Group,
    Everyone,
}

/// The six statuses a Status part can name; the order is the index into
/// `tuning::CRAFT_DEBUFF_STATUS` and `CRAFT_RIDER`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub enum StatusKind {
    Bleed,
    Stun,
    Exposed,
    Poison,
    Throttled,
    Locked,
}

impl StatusKind {
    pub const ALL: [StatusKind; 6] = [
        StatusKind::Bleed,
        StatusKind::Stun,
        StatusKind::Exposed,
        StatusKind::Poison,
        StatusKind::Throttled,
        StatusKind::Locked,
    ];

    pub fn index(self) -> usize {
        match self {
            StatusKind::Bleed => 0,
            StatusKind::Stun => 1,
            StatusKind::Exposed => 2,
            StatusKind::Poison => 3,
            StatusKind::Throttled => 4,
            StatusKind::Locked => 5,
        }
    }

    /// The `assets/statuses/` id this part inflicts.
    pub fn id(self) -> StatusId {
        StatusId::from(match self {
            StatusKind::Bleed => "bleed",
            StatusKind::Stun => "stun",
            StatusKind::Exposed => "exposed",
            StatusKind::Poison => "poison",
            StatusKind::Throttled => "throttled",
            StatusKind::Locked => "locked",
        })
    }

    pub fn from_id(id: &StatusId) -> Option<StatusKind> {
        StatusKind::ALL.into_iter().find(|k| k.id() == *id)
    }

    /// The word a description uses for the condition.
    pub fn name(self) -> &'static str {
        match self {
            StatusKind::Bleed => "bleeding",
            StatusKind::Stun => "a stall",
            StatusKind::Exposed => "exposure",
            StatusKind::Poison => "poison",
            StatusKind::Throttled => "throttling",
            StatusKind::Locked => "a repair lock",
        }
    }
}

/// One part. A Core carries its version; every modifier is unversioned.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Subroutine {
    Core(Core, Version),
    Range(RangeKind),
    Area(AreaKind),
    /// One round-adder; several stack.
    Trickle,
    DamageType(DamageType),
    Status(StatusKind),
}

impl Subroutine {
    /// What a list row calls it.
    pub fn label(&self) -> String {
        match self {
            Subroutine::Core(core, version) if core.versioned() => {
                format!("{} v{}.0", core.name(), version.number())
            }
            Subroutine::Core(core, _) => core.name().to_string(),
            Subroutine::Range(RangeKind::Near) => "Range: Near".into(),
            Subroutine::Range(RangeKind::Far) => "Range: Far".into(),
            Subroutine::Area(AreaKind::Group) => "Area: Group".into(),
            Subroutine::Area(AreaKind::Everyone) => "Area: Everyone".into(),
            Subroutine::Trickle => "Trickle".into(),
            Subroutine::DamageType(t) => format!("Type: {}", t.name()),
            Subroutine::Status(s) => format!("Status: {}", s.name()),
        }
    }

    fn slot(&self) -> &'static str {
        match self {
            Subroutine::Core(..) => "Core",
            Subroutine::Range(_) => "Range",
            Subroutine::Area(_) => "Area",
            Subroutine::Trickle => "Timing",
            Subroutine::DamageType(_) => "Damage type",
            Subroutine::Status(_) => "Status",
        }
    }
}

/// A routine the player built: the record the save keeps and `compile`
/// reads.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct CraftedRoutine {
    pub id: AbilityId,
    pub name: String,
    pub parts: Vec<Subroutine>,
}

/// Whether `part` can join a routine whose Core is `core`, or the reason it
/// cannot — worded for a greyed row.
pub fn fits(core: Core, part: &Subroutine) -> Result<(), &'static str> {
    match part {
        Subroutine::Core(..) => Err("a routine holds exactly one Core"),
        Subroutine::Range(_) => core
            .takes_range()
            .then_some(())
            .ok_or("this Core takes no Range"),
        Subroutine::Area(kind) => {
            if core.areas().contains(kind) {
                Ok(())
            } else if matches!(
                core,
                Core::ColdSample | Core::HeatInjection | Core::Profile | Core::Inject
            ) {
                Err("only Hallucinate takes an Area among the Tamper Cores")
            } else if core.facing() == Facing::Ally && !core.areas().is_empty() {
                Err("your own side has a Group, never an Everyone")
            } else {
                Err("this Core takes no Area")
            }
        }
        Subroutine::Trickle => core
            .takes_trickle()
            .then_some(())
            .ok_or("this Core has no duration to extend"),
        Subroutine::DamageType(_) => matches!(core, Core::Damage | Core::Drain)
            .then_some(())
            .ok_or("only Damage and Drain deal a type"),
        Subroutine::Status(_) => matches!(core, Core::Damage | Core::Debuff)
            .then_some(())
            .ok_or("only Damage and Debuff carry a Status"),
    }
}

/// Why a set of parts is not a routine.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildError {
    NoCore,
    TwoCores,
    /// An unversioned Core given a version above v1.0.
    NotVersioned,
    TooManyParts {
        cap: usize,
    },
    /// A second Range, Area, Damage type or Status.
    Duplicate(&'static str),
    DoesNotFit(&'static str),
    Missing(&'static str),
}

impl std::fmt::Display for BuildError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BuildError::NoCore => f.write_str("a routine needs a Core"),
            BuildError::TwoCores => f.write_str("a routine holds exactly one Core"),
            BuildError::NotVersioned => f.write_str("this Core has only one version"),
            BuildError::TooManyParts { cap } => {
                write!(f, "a routine holds at most {cap} parts")
            }
            BuildError::Duplicate(slot) => write!(f, "a routine holds one {slot} part"),
            BuildError::DoesNotFit(reason) => f.write_str(reason),
            BuildError::Missing(what) => write!(f, "this Core needs {what}"),
        }
    }
}

/// Whether `parts` is a routine `compile` may be given, under part cap
/// `cap`. Order does not matter.
pub fn validate_build(parts: &[Subroutine], cap: usize) -> Result<(), BuildError> {
    let mut cores = parts.iter().filter_map(|p| match p {
        Subroutine::Core(core, version) => Some((*core, *version)),
        _ => None,
    });
    let (core, version) = cores.next().ok_or(BuildError::NoCore)?;
    if cores.next().is_some() {
        return Err(BuildError::TwoCores);
    }
    if !core.versioned() && version != Version::V1 {
        return Err(BuildError::NotVersioned);
    }
    if parts.len() > cap {
        return Err(BuildError::TooManyParts { cap });
    }
    for part in parts {
        if matches!(part, Subroutine::Core(..)) {
            continue;
        }
        fits(core, part).map_err(BuildError::DoesNotFit)?;
        if !matches!(part, Subroutine::Trickle) {
            let same = parts
                .iter()
                .filter(|p| std::mem::discriminant(*p) == std::mem::discriminant(part))
                .count();
            if same > 1 {
                return Err(BuildError::Duplicate(part.slot()));
            }
        }
    }
    match core.missing(parts) {
        Some(what) => Err(BuildError::Missing(what)),
        None => Ok(()),
    }
}

/// What crafting `part` costs besides the one Routine Disk: a material and
/// how many, the Core's version number or one for a modifier.
pub fn material(part: &Subroutine) -> (ItemId, u32) {
    let (item, count) = match part {
        Subroutine::Core(core, version) => {
            let item = match core {
                Core::Heal
                | Core::BoostAttack
                | Core::BoostMitigation
                | Core::Cleanse
                | Core::Cloak => CRAFT_MATERIAL_BYTECODE,
                Core::Damage
                | Core::Drain
                | Core::SapAttack
                | Core::SapMitigation
                | Core::Debuff => CRAFT_MATERIAL_FRAGMENT,
                Core::Fork
                | Core::Respawn
                | Core::ColdSample
                | Core::HeatInjection
                | Core::Profile
                | Core::Inject
                | Core::Hallucinate
                | Core::Teleport
                | Core::Reinforce
                | Core::Emulate
                | Core::FieldRegen
                | Core::Recharge
                | Core::FieldAttack
                | Core::FieldMitigation
                | Core::CaptureBoost
                | Core::XpBoost
                | Core::EncounterDamp
                | Core::DropBoost
                | Core::Phase
                | Core::Jump
                | Core::Symlink => CRAFT_MATERIAL_CELL,
            };
            (item, version.number())
        }
        Subroutine::Range(_) => (CRAFT_MATERIAL_CELL, 1),
        Subroutine::Area(_) | Subroutine::DamageType(_) => (CRAFT_MATERIAL_FRAGMENT, 1),
        Subroutine::Trickle | Subroutine::Status(_) => (CRAFT_MATERIAL_BYTECODE, 1),
    };
    (ItemId::from(item), count)
}
