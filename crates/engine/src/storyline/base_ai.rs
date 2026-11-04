//! The base AI arc: a staff program claims the Research Station, goes into a
//! fugue and fuses with it. Everything here is pure; no `World`, no RNG.

use serde::{Deserialize, Serialize};

use crate::components::ProgramId;
use crate::tuning::{
    BASE_AI_LEVEL_OFFSETS, BASE_AI_PRACTICE_BASE, BASE_AI_PRACTICE_QUAD, BASE_AI_START_SPEED,
    BASE_AI_TRAINING_COST_BASE, BASE_AI_TRAINING_COST_PER_LEVEL,
};

/// Programs and stations are saved by stable key, never by `Entity`.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub enum BaseAi {
    /// `announced` is true once the awakening notification has fired, so a
    /// claim that is abandoned and retried does not re-alert.
    Dormant {
        announced: bool,
    },
    Claiming {
        host: ProgramId,
        station: (i32, i32),
    },
    Fugue {
        host: ProgramId,
        station: (i32, i32),
        ticks_left: u32,
    },
    Fused(FusedAi),
}

impl Default for BaseAi {
    fn default() -> Self {
        BaseAi::Dormant { announced: false }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FusedAi {
    pub host: ProgramId,
    pub station: (i32, i32),
    pub level: u32,
    /// The receipt for the baked stat: what the arc added is the host's
    /// current Analysis minus this.
    pub analysis_at_fusion: i32,
    /// Research units per tick the host made on the ordinary formula.
    pub pace_at_fusion: f64,
    /// Practice toward the next Analysis point.
    pub practice: u32,
    /// Fractional research not yet credited.
    pub carry: f64,
    pub training: TrainingTarget,
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct TrainingTarget {
    pub selected: bool,
    pub progress: u32,
}

pub struct HostCandidate {
    pub id: ProgramId,
    pub analysis: i32,
    pub free: bool,
}

pub struct StationSpot {
    pub anchor: (i32, i32),
    pub distance: u32,
}

pub struct StepInput<'a> {
    pub researched: u32,
    pub trigger: u32,
    pub candidates: &'a [HostCandidate],
    pub stations: &'a [StationSpot],
    pub host_free: bool,
    pub host_arrived: bool,
    pub host_analysis: i32,
    pub host_pace: f64,
}

#[derive(Clone, Debug, PartialEq)]
pub enum StepOutcome {
    None,
    BeginClaim {
        host: ProgramId,
        station: (i32, i32),
        announce: bool,
    },
    AbandonClaim,
    /// The shell starts the fugue at `BASE_AI_FUGUE_TICKS`.
    StartFugue,
    TickFugue,
    Fuse(FusedAi),
}

/// Splitmix32-style mixer, local so the trigger never touches `GameRng`.
fn mix(mut x: u32) -> u32 {
    x = x.wrapping_add(0x9E37_79B9);
    x = (x ^ (x >> 16)).wrapping_mul(0x85EB_CA6B);
    x = (x ^ (x >> 13)).wrapping_mul(0xC2B2_AE35);
    x ^ (x >> 16)
}

/// How many finished research projects awaken the AI: 5 to 10, fixed for the
/// run by its seed.
pub fn trigger_count(seed: u32) -> u32 {
    5 + mix(seed ^ 0xBA5E_A1A1) % 6
}

/// The free program with the highest Analysis; a tie goes to the lower id.
pub fn pick_host(c: &[HostCandidate]) -> Option<ProgramId> {
    c.iter()
        .filter(|h| h.free)
        .max_by_key(|h| (h.analysis, std::cmp::Reverse(h.id)))
        .map(|h| h.id)
}

/// The nearest station; a tie goes to the lower anchor.
pub fn pick_station(s: &[StationSpot]) -> Option<(i32, i32)> {
    s.iter()
        .min_by_key(|s| (s.distance, s.anchor))
        .map(|s| s.anchor)
}

/// The Analysis at which the AI stalls at its current level; `i32::MAX` past
/// the table.
pub fn analysis_cap(f: &FusedAi) -> i32 {
    BASE_AI_LEVEL_OFFSETS
        .get(f.level as usize)
        .map_or(i32::MAX, |offset| f.analysis_at_fusion + offset)
}

/// Practice to earn the point after `points_gained` points.
pub fn practice_cost(points_gained: i32) -> u32 {
    let g = points_gained.max(0) as u32;
    BASE_AI_PRACTICE_BASE + BASE_AI_PRACTICE_QUAD * g * g
}

/// Research units per tick at `analysis`: the pace frozen at fusion, slowed to
/// the start speed, scaled by Analysis with no ceiling. The ordinary formula
/// saturates at once, so it cannot carry the AI's growth.
pub fn pace(f: &FusedAi, analysis: i32) -> f64 {
    let ratio = f64::from(analysis) / f64::from(f.analysis_at_fusion.max(1));
    f.pace_at_fusion * BASE_AI_START_SPEED * ratio
}

#[derive(Clone, Debug, PartialEq)]
pub struct PracticeOutcome {
    pub analysis: i32,
    pub practice: u32,
    pub stalled: bool,
}

/// Practise for `ticks`. Stops exactly at the cap and drops practice past it.
pub fn practise(f: &FusedAi, analysis: i32, ticks: u32) -> PracticeOutcome {
    let cap = analysis_cap(f);
    let mut analysis = analysis;
    let mut practice = f.practice;
    for _ in 0..ticks {
        if analysis >= cap {
            break;
        }
        practice += 1;
        let cost = practice_cost(analysis - f.analysis_at_fusion);
        if practice >= cost {
            practice -= cost;
            analysis += 1;
        }
    }
    let stalled = analysis >= cap;
    if stalled {
        practice = 0;
    }
    PracticeOutcome {
        analysis,
        practice,
        stalled,
    }
}

/// Research points "Train your AI" costs to leave `level`.
pub fn training_cost(level: u32) -> u32 {
    BASE_AI_TRAINING_COST_BASE + BASE_AI_TRAINING_COST_PER_LEVEL * level
}

/// Dormant → Claiming → Fugue → Fused, one tick at a time.
pub fn step(arc: &BaseAi, input: &StepInput) -> StepOutcome {
    match arc {
        BaseAi::Dormant { announced } => {
            if input.researched < input.trigger {
                return StepOutcome::None;
            }
            match (pick_host(input.candidates), pick_station(input.stations)) {
                (Some(host), Some(station)) => StepOutcome::BeginClaim {
                    host,
                    station,
                    announce: !announced,
                },
                _ => StepOutcome::None,
            }
        }
        BaseAi::Claiming { .. } => {
            if !input.host_free {
                StepOutcome::AbandonClaim
            } else if input.host_arrived {
                StepOutcome::StartFugue
            } else {
                StepOutcome::None
            }
        }
        BaseAi::Fugue {
            host,
            station,
            ticks_left,
        } => {
            if *ticks_left == 0 {
                StepOutcome::Fuse(FusedAi {
                    host: *host,
                    station: *station,
                    level: 0,
                    analysis_at_fusion: input.host_analysis,
                    pace_at_fusion: input.host_pace,
                    practice: 0,
                    carry: 0.0,
                    training: TrainingTarget::default(),
                })
            } else {
                StepOutcome::TickFugue
            }
        }
        BaseAi::Fused(_) => StepOutcome::None,
    }
}
