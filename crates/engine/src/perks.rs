use serde::{Deserialize, Serialize};

/// A permanent passive upgrade purchased with Perk Points (earned 1 per
/// player level-up — see `Game::award_player_xp`). Unlike a one-time
/// unlock, a perk can be bought repeatedly: each purchase adds another
/// level, and each level is worth exactly 1 point toward that perk's
/// bonus (see `components::Perks::level`) — a small, steady stack rather
/// than one big jump. Unlike species/structures/items, this is a small,
/// fixed set of player-only progression choices, not moddable content —
/// see `CLAUDE.md` for why some things stay code rather than data.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum Perk {
    /// +1 percentage point to `Game::forage`'s success chance per level.
    KeenScavenger,
    /// -1 percentage point off Power (hunger) drain per level, down to a
    /// floor of 0 (hunger stops draining at all).
    LowPowerMode,
    /// +1 effective Decompiler skill per level toward decompile-chance
    /// calculations, stacking with the real `Decompiler` stat from
    /// leveling/equipment.
    ExploitFocus,
    /// -1 to the cost of each item Compiling (`Game::craft`) requires per
    /// level, down to a minimum of 1 each.
    LeanCompiler,
}

impl Perk {
    pub fn all() -> [Perk; 4] {
        [
            Perk::KeenScavenger,
            Perk::LowPowerMode,
            Perk::ExploitFocus,
            Perk::LeanCompiler,
        ]
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
            Perk::KeenScavenger => {
                "+1% chance to find a Core Fragment when scanning (g), per level"
            }
            Perk::LowPowerMode => "Power drains 1% slower per level (floor: doesn't drain at all)",
            Perk::ExploitFocus => "+1 effective Decompiler skill toward decompile odds, per level",
            Perk::LeanCompiler => {
                "Compiling costs 1 less of each required item per level (min 1 each)"
            }
        }
    }

    /// Perk Points spent per level — the same cost every time, however
    /// many levels you already have.
    pub fn cost(self) -> u32 {
        match self {
            Perk::KeenScavenger => 2,
            Perk::LowPowerMode => 2,
            Perk::ExploitFocus => 3,
            Perk::LeanCompiler => 3,
        }
    }
}
