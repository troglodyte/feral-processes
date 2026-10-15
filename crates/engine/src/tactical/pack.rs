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
use crate::resources::Behaviours;
use crate::components::Stats;
use crate::derive::{FNV_BASIS, fold, index};
use crate::tactical::TacticalBattle;
use crate::tactical::map::BattleSpec;
use crate::tuning::{PACK_ROLL_SALT, PACK_TACTIC_ODDS};

/// What the hostile side is doing as a pack.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
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

impl PackTactic {
    /// Every tactic, in the column order of `tuning::PACK_TACTIC_ODDS`.
    pub const ALL: [PackTactic; 3] = [PackTactic::Rush, PackTactic::Together, PackTactic::Hold];

    /// Whether a pack of `members` bodies can play this at all. Checked
    /// before the draw, so an ineligible tactic has weight zero rather than
    /// being rerolled.
    fn eligible(self, members: usize) -> bool {
        match self {
            PackTactic::Rush | PackTactic::Hold => true,
            PackTactic::Together => members >= 2,
        }
    }
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

/// The odds row for behaviour `id`, falling back to `default`'s.
pub(crate) fn odds_row(id: &str) -> [u32; 3] {
    let row = |wanted: &str| {
        PACK_TACTIC_ODDS
            .iter()
            .find(|(key, _)| *key == wanted)
            .map(|(_, odds)| *odds)
    };
    row(id)
        .or_else(|| row("default"))
        .expect("PACK_TACTIC_ODDS has a `default` row")
}

/// The body a pack is profiled by: the highest max Integrity, ties to the
/// first in `members` (initiative order).
pub(crate) fn lead(game: &Game, members: &[Entity]) -> Option<Entity> {
    let mut best: Option<(i32, Entity)> = None;
    for &member in members {
        let max_hp = game.world.get::<Stats>(member).map_or(0, |s| s.max_hp);
        if best.is_none_or(|(top, _)| max_hp > top) {
            best = Some((max_hp, member));
        }
    }
    best.map(|(_, member)| member)
}

/// Rolls the tactic for a hostile side of `members` (initiative order).
///
/// **Draws nothing from `GameRng`**: the fold is `spec`'s own words and a
/// salt, so the game's random stream is where it would have been and no
/// seeded test shifts because a fight now rolls. An arena override wins
/// before any of it.
pub fn roll(spec: BattleSpec, members: &[Entity], game: &Game) -> PackTactic {
    if let Some(forced) = game.world.resource::<Behaviours>().0.forced_tactic() {
        return forced;
    }
    let Some(lead) = lead(game, members) else {
        return PackTactic::Rush;
    };
    let odds = odds_row(&game.behaviour_of(lead).id);
    let weights: Vec<u32> = PackTactic::ALL
        .iter()
        .zip(odds)
        .map(|(tactic, weight)| if tactic.eligible(members.len()) { weight } else { 0 })
        .collect();
    let total: u32 = weights.iter().sum();
    if total == 0 {
        return PackTactic::Rush;
    }
    let seed = fold(
        FNV_BASIS,
        &[
            spec.world_seed as u64,
            spec.site.0 as u32 as u64,
            spec.site.1 as u32 as u64,
            spec.tick,
            PACK_ROLL_SALT,
        ],
    );
    let mut pick = index(seed, total as usize) as u32;
    for (tactic, weight) in PackTactic::ALL.iter().zip(weights) {
        if pick < weight {
            return *tactic;
        }
        pick -= weight;
    }
    PackTactic::Rush
}

/// The log line that opens a fight played under `tactic`. `Rush` has none:
/// it is today's behaviour and needs no announcing.
pub(crate) fn opening_line(tactic: PackTactic) -> Option<&'static str> {
    match tactic {
        PackTactic::Rush => None,
        PackTactic::Together => Some("The pack advances together."),
        PackTactic::Hold => Some("The pack spreads out behind cover, waiting."),
    }
}
