//! Pack tactics: one plan for the whole hostile side of a battle-map fight,
//! rolled when the fight opens, that adds pack-aware terms on top of each
//! body's own personality.
//!
//! Everything here enters the decision through the planner the turn already
//! runs (`Game::scored_cells`' terms and closing scale), so a forecast follows
//! without a copy of its own. `Rush` adds nothing and scales closing by
//! exactly one, which is what keeps a fight that never rolled decide as it
//! always has.

use bevy_ecs::prelude::Entity;
use serde::{Deserialize, Serialize};

use crate::Game;
use crate::tactical::TacticalBattle;

/// What the hostile side is doing as a pack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum PackTactic {
    /// Every body for itself: today's behaviour, and what any fight that did
    /// not roll plays.
    #[default]
    Rush,
    /// Stay within a slack rope of the packmates, so the pack arrives
    /// roughly together.
    Together,
    /// Take cover and wait until sighted, hit, or out of patience.
    Hold,
}

/// The current tactic and what a transition out of it needs to read.
///
/// Never serialised: `TacticalBattle` is never saved.
#[derive(Clone, Debug, Default)]
pub struct PackState {
    pub tactic: PackTactic,
}

impl Game {
    /// Seats `tactic` as the hostile side's plan.
    pub(crate) fn set_pack_tactic(&mut self, tactic: PackTactic) {
        if let Some(mut battle) = self.world.get_resource_mut::<TacticalBattle>() {
            battle.pack = PackState { tactic };
        }
    }
}

/// What `body` adds to a candidate cell's worth under the pack's tactic.
pub(crate) fn cell_terms(_game: &Game, _body: Entity, _cell: (i32, i32)) -> f32 {
    0.0
}

/// The multiplier on `body`'s closing term.
pub(crate) fn closing_scale(_game: &Game, _body: Entity) -> f32 {
    1.0
}
