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
use crate::components::Stats;
use crate::derive::{FNV_BASIS, fold, index};
use crate::resources::Behaviours;
use crate::tactical::TacticalBattle;
use crate::tactical::ai::cover_share;
use crate::tactical::map::{BattleSpec, Board};
use crate::tactical::reach::{self, line_of_sight};
use crate::tuning::{
    PACK_ROLL_SALT, PACK_TACTIC_ODDS, TACTICAL_COHESION_FACTOR, TACTICAL_COHESION_SLACK,
    TACTICAL_HOLD_ALERT_RANGE, TACTICAL_HOLD_COVER_BONUS, TACTICAL_HOLD_ROUNDS,
    TACTICAL_HOLD_SIGHT_CHARGE,
};

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
    /// The round a hold began on, for its timeout.
    hold_start_round: u32,
    /// Every member's Integrity when the hold began, so "hit" is a
    /// comparison and `apply_damage` stays the only damage path with no hook
    /// of ours in it. A member that is no longer on the board is "gone".
    hold_hp: Vec<(Entity, i32)>,
}

impl Game {
    /// Seats `tactic` as the hostile side's plan.
    pub(crate) fn set_pack_tactic(&mut self, tactic: PackTactic) {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        let round = battle.round;
        let hold_hp = if tactic == PackTactic::Hold {
            members(self)
                .into_iter()
                .map(|member| (member, self.world.get::<Stats>(member).map_or(0, |s| s.hp)))
                .collect()
        } else {
            Vec::new()
        };
        self.world.resource_mut::<TacticalBattle>().pack = PackState {
            tactic,
            hold_start_round: round,
            hold_hp,
        };
    }
}

/// Every body the hostile side has on the board right now.
///
/// **`acts_for_hostiles`, the one door onto allegiance**: a body tampered
/// onto the party's side has left the pack and a hallucinating one is still in
/// it. A squad is one body, and positions are read through `cells_of`.
pub(crate) fn members(game: &Game) -> Vec<Entity> {
    let battle = game.world.resource::<TacticalBattle>();
    battle
        .bodies()
        .map(|(body, _)| body)
        .filter(|&body| game.acts_for_hostiles(body))
        .collect()
}

/// Everything a turn's planner needs to know about the pack, read once per
/// `scored_cells` call rather than once per candidate cell.
///
/// One path for the turn and the forecast: both build it through `plan`, so
/// both plan under the tactic this turn will actually play.
pub(crate) struct PackPlan {
    tactic: PackTactic,
    /// The mean of the packmates, one point per mate (a squad is one body).
    mates: Option<(i32, i32)>,
    targets: Vec<(i32, i32)>,
    footprint: u8,
    /// `TACTICAL_COHESION_FACTOR` times the body's own closing weight.
    cohesion: f32,
}

/// `body`'s plan under the pack's tactic, `closing` being its profile's closing
/// weight. A body outside the pack, and any
/// body under `Rush`, gets a plan that adds nothing and scales by one.
pub(crate) fn plan(game: &Game, body: Entity, closing: f32) -> PackPlan {
    let battle = game.world.resource::<TacticalBattle>();
    let tactic = if game.acts_for_hostiles(body) {
        effective(game)
    } else {
        PackTactic::Rush
    };
    let mut plan = PackPlan {
        tactic,
        mates: None,
        targets: Vec::new(),
        footprint: battle.footprint_of(body),
        cohesion: closing * TACTICAL_COHESION_FACTOR,
    };
    match tactic {
        PackTactic::Rush => {}
        PackTactic::Together => {
            let points: Vec<(f32, f32)> = members(game)
                .into_iter()
                .filter(|&mate| mate != body)
                .filter_map(|mate| {
                    let cells = battle.cells_of(mate);
                    let n = cells.len() as f32;
                    (n > 0.0).then(|| {
                        (
                            cells.iter().map(|c| c.0 as f32).sum::<f32>() / n,
                            cells.iter().map(|c| c.1 as f32).sum::<f32>() / n,
                        )
                    })
                })
                .collect();
            if !points.is_empty() {
                let n = points.len() as f32;
                plan.mates = Some((
                    (points.iter().map(|p| p.0).sum::<f32>() / n).round() as i32,
                    (points.iter().map(|p| p.1).sum::<f32>() / n).round() as i32,
                ));
            }
        }
        PackTactic::Hold => plan.targets = game.tactical_sides(body).targets,
    }
    plan
}

impl PackPlan {
    /// The multiplier on the body's closing term.
    pub(crate) fn closing_scale(&self) -> f32 {
        match self.tactic {
            PackTactic::Rush | PackTactic::Together => 1.0,
            PackTactic::Hold => 0.0,
        }
    }

    /// What standing at `cell` adds to its worth under the tactic.
    pub(crate) fn cell_terms(&self, board: &crate::tactical::map::Board, cell: (i32, i32)) -> f32 {
        let stands = crate::tactical::footprint_cells_at(cell, self.footprint);
        match self.tactic {
            PackTactic::Rush => 0.0,
            // The slack-rope pull toward the packmates: free within
            // `TACTICAL_COHESION_SLACK` of them, a linear charge beyond.
            PackTactic::Together => self.mates.map_or(0.0, |centroid| {
                let d = reach::gap(&stands, &[centroid]);
                -self.cohesion * d.saturating_sub(TACTICAL_COHESION_SLACK) as f32
            }),
            // Cover-seeking must agree with the trigger: a cell the party
            // would see from is charged past anything cover pays, so the walk
            // to shelter never ends where the hold would then break.
            PackTactic::Hold => {
                let seen = self.targets.iter().any(|&t| in_view(board, &stands, &[t]));
                let charge = if seen {
                    TACTICAL_HOLD_SIGHT_CHARGE
                } else {
                    0.0
                };
                TACTICAL_HOLD_COVER_BONUS * cover_share(board, &stands, &self.targets) - charge
            }
        }
    }
}

/// The tactic the pack is playing **this turn**: the stored one, unless a hold
/// has a reason to end.
///
/// What `plan` reads, so a forecast (which cannot
/// write) and the turn that follows it plan under the same tactic.
fn effective(game: &Game) -> PackTactic {
    let state = &game.world.resource::<TacticalBattle>().pack;
    if state.tactic == PackTactic::Hold && hold_should_break(game) {
        PackTactic::Rush
    } else {
        state.tactic
    }
}

/// Whether a hold has been sighted, hit, or run out of patience.
fn hold_should_break(game: &Game) -> bool {
    let battle = game.world.resource::<TacticalBattle>();
    let state = &battle.pack;
    if battle.round >= state.hold_start_round + TACTICAL_HOLD_ROUNDS {
        return true;
    }
    let pack = members(game);
    let hit_or_gone = state.hold_hp.iter().any(|&(member, hp)| {
        !pack.contains(&member) || game.world.get::<Stats>(member).is_none_or(|s| s.hp < hp)
    });
    hit_or_gone || sighted(game, &pack)
}

/// Whether anything standing on `theirs` is within `TACTICAL_HOLD_ALERT_RANGE`
/// of `ours` and in sight of it.
///
/// **The one sighted test.** The hold's trigger and the cover-seeking term
/// both ask it, so a body never walks to a cell the trigger would then count
/// as seen.
fn in_view(board: &Board, ours: &[(i32, i32)], theirs: &[(i32, i32)]) -> bool {
    ours.iter().any(|&a| {
        theirs.iter().any(|&b| {
            reach::distance(a, b) <= TACTICAL_HOLD_ALERT_RANGE && line_of_sight(board, a, b)
        })
    })
}

/// Whether any body outside the pack is sighted by a member.
fn sighted(game: &Game, pack: &[Entity]) -> bool {
    let battle = game.world.resource::<TacticalBattle>();
    battle
        .bodies()
        .filter(|(body, _)| !pack.contains(body))
        .any(|(outsider, _)| {
            let theirs = battle.cells_of(outsider);
            pack.iter()
                .any(|&member| in_view(&battle.board, &battle.cells_of(member), &theirs))
        })
}

/// Ends the hold if it has a reason to, returning the line to log.
///
/// Called at the start of every hostile beat, before the intent is chosen.
/// The first trigger to fire ends the hold for the whole pack, and a pack no
/// longer holding answers `None`.
pub(crate) fn check_transition(game: &mut Game) -> Option<&'static str> {
    let battle = game.world.get_resource::<TacticalBattle>()?;
    if battle.pack.tactic != PackTactic::Hold || !hold_should_break(game) {
        return None;
    }
    game.set_pack_tactic(PackTactic::Rush);
    Some("The pack breaks cover!")
}

/// The fold the roll is drawn from: `spec`'s own words and a salt, so it is
/// independent of the board drawn from the same spec.
pub(crate) fn roll_seed(spec: BattleSpec) -> u64 {
    fold(
        FNV_BASIS,
        &[
            spec.world_seed as u64,
            spec.site.0 as u32 as u64,
            spec.site.1 as u32 as u64,
            spec.tick,
            PACK_ROLL_SALT,
        ],
    )
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
        .map(|(tactic, weight)| {
            if tactic.eligible(members.len()) {
                weight
            } else {
                0
            }
        })
        .collect();
    let total: u32 = weights.iter().sum();
    if total == 0 {
        return PackTactic::Rush;
    }
    let seed = roll_seed(spec);
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
