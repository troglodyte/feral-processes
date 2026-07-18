use serde::{Deserialize, Serialize};

/// A permanent passive unlock purchased with Perk Points (earned 1 per
/// player level-up — see `Game::award_player_xp`). Each perk can only be
/// unlocked once. Unlike species/structures/items, this is a small, fixed
/// set of player-only progression choices, not moddable content — see
/// `CLAUDE.md` for why some things stay code rather than data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Perk {
    /// +15 percentage points to `Game::forage`'s success chance.
    KeenScavenger,
    /// Power (hunger) drains at 70% of the normal rate.
    LowPowerMode,
    /// +5 effective Decompiler skill toward decompile-chance calculations,
    /// stacking with the real `Decompiler` stat from leveling/equipment.
    ExploitFocus,
    /// Compiling (`Game::craft`) costs 1 less of each required item, down
    /// to a minimum of 1 each.
    LeanCompiler,
}

impl Perk {
    pub fn all() -> [Perk; 4] {
        [Perk::KeenScavenger, Perk::LowPowerMode, Perk::ExploitFocus, Perk::LeanCompiler]
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Perk::KeenScavenger => "Keen Scavenger",
            Perk::LowPowerMode => "Low Power Mode",
            Perk::ExploitFocus => "Exploit Focus",
            Perk::LeanCompiler => "Lean Compiler",
        }
    }

    pub fn description(self) -> &'static str {
        match self {
            Perk::KeenScavenger => "+15% chance to find a Core Fragment when scanning (g)",
            Perk::LowPowerMode => "Power drains 30% slower",
            Perk::ExploitFocus => "+5 effective Decompiler skill toward decompile odds",
            Perk::LeanCompiler => "Compiling costs 1 less of each required item (min 1 each)",
        }
    }

    /// Perk Points required to unlock.
    pub fn cost(self) -> u32 {
        match self {
            Perk::KeenScavenger => 2,
            Perk::LowPowerMode => 2,
            Perk::ExploitFocus => 3,
            Perk::LeanCompiler => 3,
        }
    }
}
