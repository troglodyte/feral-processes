//! The siege clock — `resources::SiegePressure`'s accrual, its three holds,
//! the approach warning and its wall-clock floor, and (Task 5) the dev
//! console's two rows.
//!
//! `raids.rs`'s clock tests are the model throughout: a siege's meter is
//! `RaidPressure`'s shape, built the same way, for the reasons recorded
//! there.

use crate::tests::support::*;
use crate::tuning::{
    BASE_ESTABLISHED_STAFF, BASE_ESTABLISHED_STRUCTURES, SIEGE_MIN_ZONE,
    SIEGE_PRESSURE_JITTER_PERCENT, SIEGE_PRESSURE_PER_ZONE, SIEGE_PRESSURE_THRESHOLD,
    SIEGE_PRESSURE_WARN_PERCENT, SIEGE_WARN_FLOOR_TICKS,
};
use crate::*;

mod board_and_opening;
mod clock;
mod fixtures;
mod raiders;
mod reviews;
mod turrets_and_persist;

pub(crate) use fixtures::*;
