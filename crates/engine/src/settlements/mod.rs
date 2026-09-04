//! Towns and cities: the content that makes a persistent map worth reading.
//!
//! Split on the perks precedent (`crates/engine/src/perks.rs`), and for the
//! same reason. A settlement's *prose* — its name, its blurb — plainly is
//! data, and a modder adding one should drop in a file. Its *behaviour* —
//! how a temperament moves a price, what a specialty puts on a shelf — is a
//! hook into a particular formula with no shared shape to express as data,
//! so it stays in Rust. `catalogue.rs` is the thin half; the behaviour
//! queries live beside the formulas they feed.
//!
//! **Where a settlement stands is derived, never stored** — `placement.rs`.
//! That is what makes one a property of the map rather than an event: there
//! is no spawn, no despawn, and nothing in the save that could disagree
//! with the ground. It is the same rule `rock::RockDb::kind_at` follows for
//! what a base-space coordinate is made of, and it only became possible
//! once a breach stopped rebuilding the world.

pub mod catalogue;
pub mod placement;

pub use catalogue::{SettlementDb, SettlementDef};
pub use placement::{SettlementKey, settlement_at};

use serde::{Deserialize, Serialize};

/// How big a settlement is, and how much it can do.
///
/// Two rather than a scale, because the difference is meant to be legible
/// at a glance on the map rather than compared: a `Server` is a stop, a
/// `Mainframe` is a destination.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum SettlementKind {
    /// A city. Carries more shelf rows and higher tiers than a server.
    Mainframe,
    /// A town.
    Server,
}

impl SettlementKind {
    /// The glyph the zone map draws.
    ///
    /// Case is the scale cue and is deliberate — the same shape, sized by
    /// what it is — so the two never need a legend.
    pub fn glyph(self) -> char {
        match self {
            SettlementKind::Mainframe => 'M',
            SettlementKind::Server => 's',
        }
    }

    pub fn label(self) -> &'static str {
        match self {
            SettlementKind::Mainframe => "Mainframe",
            SettlementKind::Server => "Server",
        }
    }
}

/// What a settlement is good for, which is what its shelf leans toward.
///
/// Exhaustive on purpose: a fifth specialty is a Rust change because every
/// one of these is a weighting hook, and a specialty no formula knows about
/// would author a town that reads as broken rather than as neutral.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Specialty {
    Gear,
    Materials,
    Routines,
    Programs,
}

impl Specialty {
    pub fn label(self) -> &'static str {
        match self {
            Specialty::Gear => "Gear",
            Specialty::Materials => "Materials",
            Specialty::Routines => "Routines",
            Specialty::Programs => "Programs",
        }
    }
}

/// How a settlement treats you before you have done anything.
///
/// Authored now and read by nothing yet: the hooks are prices (Phase 3) and
/// standing (Phase 4), and both want the whole catalogue to already carry
/// the field rather than every shipped town needing a re-author when they
/// land. `every_temperament_is_authored_somewhere` in `tests/assets.rs` is
/// what keeps an unreachable variant from shipping in the meantime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Temperament {
    /// Trades readily, asks little.
    Open,
    /// Drives a hard bargain; warms slowly.
    Guarded,
    /// Everything is business, including goodwill.
    Mercantile,
}

impl Temperament {
    pub fn label(self) -> &'static str {
        match self {
            Temperament::Open => "Open",
            Temperament::Guarded => "Guarded",
            Temperament::Mercantile => "Mercantile",
        }
    }

    /// What this temperament scales a buy price by — `Game::marked_unit_cost`'s
    /// `mult`, read by `Game::settlement_unit_cost`.
    ///
    /// Exhaustive on purpose, `Specialty`'s own reason: a fourth temperament
    /// with no price to read is one that reads as broken rather than as
    /// neutral. See `tuning::SETTLEMENT_OPEN_BUY_MULT` for the table and the
    /// argument for Mercantile's asymmetry.
    pub(crate) fn buy_mult(self) -> f32 {
        match self {
            Temperament::Open => crate::tuning::SETTLEMENT_OPEN_BUY_MULT,
            Temperament::Guarded => crate::tuning::SETTLEMENT_GUARDED_BUY_MULT,
            Temperament::Mercantile => crate::tuning::SETTLEMENT_MERCANTILE_BUY_MULT,
        }
    }

    /// What this temperament scales what a settlement pays *you* by — see
    /// `Game::settlement_sell_price` and `buy_mult`'s doc for why the two
    /// axes are six constants and not one.
    pub(crate) fn sell_mult(self) -> f32 {
        match self {
            Temperament::Open => crate::tuning::SETTLEMENT_OPEN_SELL_MULT,
            Temperament::Guarded => crate::tuning::SETTLEMENT_GUARDED_SELL_MULT,
            Temperament::Mercantile => crate::tuning::SETTLEMENT_MERCANTILE_SELL_MULT,
        }
    }
}
