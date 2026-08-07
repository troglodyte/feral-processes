//! What a run of the arena produces.

use serde::Serialize;

use super::scenario::Scenario;

/// One fight, start to finish.
///
/// `transcript` is `Vec<String>` rather than `Vec<LogLine>` on purpose: the
/// report is for reading and post-processing, and `MessageKind` /
/// `MessageSource` are the log's internal vocabulary, not a file format.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct RepRecord {
    pub seed: u64,
    pub won: bool,
    pub rounds: u32,
    pub player_hp_fraction: f32,
    pub companions_downed: u32,
    pub transcript: Vec<String>,
}

/// Every rep, plus the scenario that produced them — a report is meant to
/// be readable a month later without the file that made it.
#[derive(Clone, Debug, Serialize)]
pub struct Report {
    pub scenario: Scenario,
    pub warnings: Vec<String>,
    pub reps: Vec<RepRecord>,
}
