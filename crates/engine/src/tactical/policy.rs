//! Per-profile weights of the tactical AI.

use serde::{Deserialize, Serialize};

use crate::tuning::{
    TACTICAL_AI_CLOSING_WEIGHT, TACTICAL_AI_COVER_TIEBREAK, TACTICAL_AI_COVER_WEIGHT,
    TACTICAL_AI_CROWDING_WEIGHT, TACTICAL_AI_REACH_SCORE, TACTICAL_AI_REACTION_WEIGHT,
};

/// The weights `cell_merit`, `cell_score` and `walk_risk` read, so a
/// personality can walk differently. `Default` is the global tuning.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(default)]
pub struct CellWeights {
    pub reach: f32,
    pub closing: f32,
    pub cover: f32,
    pub cover_tiebreak: f32,
    pub crowding: f32,
    pub reaction: f32,
}

impl Default for CellWeights {
    fn default() -> Self {
        Self {
            reach: TACTICAL_AI_REACH_SCORE,
            closing: TACTICAL_AI_CLOSING_WEIGHT,
            cover: TACTICAL_AI_COVER_WEIGHT,
            cover_tiebreak: TACTICAL_AI_COVER_TIEBREAK,
            crowding: TACTICAL_AI_CROWDING_WEIGHT,
            reaction: TACTICAL_AI_REACTION_WEIGHT,
        }
    }
}
