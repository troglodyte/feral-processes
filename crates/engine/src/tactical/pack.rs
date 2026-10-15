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
use crate::tactical::map::BattleSpec;
use crate::tactical::reach::{self, line_of_sight};
use crate::tuning::{
    PACK_ROLL_SALT, PACK_TACTIC_ODDS, TACTICAL_COHESION_SLACK, TACTICAL_COHESION_WEIGHT,
    TACTICAL_HOLD_ALERT_RANGE, TACTICAL_HOLD_COVER_BONUS, TACTICAL_HOLD_ROUNDS,
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

/// Whether `body` is part of the pack the current tactic plays for.
fn in_pack(game: &Game, body: Entity) -> bool {
    game.acts_for_hostiles(body)
}

/// What `body` adds to a candidate cell's worth under the pack's tactic.
///
/// Zero for `Rush` and for any body outside the pack, so a party body driven
/// through `scored_cells` plans as it always has.
pub(crate) fn cell_terms(game: &Game, body: Entity, cell: (i32, i32)) -> f32 {
    if !in_pack(game, body) {
        return 0.0;
    }
    match effective(game) {
        PackTactic::Rush => 0.0,
        PackTactic::Together => cohesion(game, body, cell),
        PackTactic::Hold => hold_cover(game, body, cell),
    }
}

/// What a holding body gains for standing where its targets cannot shoot it.
fn hold_cover(game: &Game, body: Entity, cell: (i32, i32)) -> f32 {
    let battle = game.world.resource::<TacticalBattle>();
    let stands = crate::tactical::footprint_cells_at(cell, battle.footprint_of(body));
    let targets = game.tactical_sides(body).targets;
    TACTICAL_HOLD_COVER_BONUS * cover_share(&battle.board, &stands, &targets)
}

/// The tactic the pack is playing **this turn**: the stored one, unless a hold
/// has a reason to end.
///
/// What `closing_scale` and `cell_terms` read, so a forecast (which cannot
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

/// Whether any body outside the pack is within `TACTICAL_HOLD_ALERT_RANGE` of
/// a member and in its line of sight.
fn sighted(game: &Game, pack: &[Entity]) -> bool {
    let battle = game.world.resource::<TacticalBattle>();
    battle
        .bodies()
        .filter(|(body, _)| !pack.contains(body))
        .any(|(outsider, _)| {
            let theirs = battle.cells_of(outsider);
            pack.iter().any(|&member| {
                let ours = battle.cells_of(member);
                ours.iter().any(|&a| {
                    theirs.iter().any(|&b| {
                        reach::distance(a, b) <= TACTICAL_HOLD_ALERT_RANGE
                            && line_of_sight(&battle.board, a, b)
                    })
                })
            })
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

/// The slack-rope pull toward the packmates' centroid: free within
/// `TACTICAL_COHESION_SLACK` of it, a linear charge beyond.
fn cohesion(game: &Game, body: Entity, cell: (i32, i32)) -> f32 {
    let battle = game.world.resource::<TacticalBattle>();
    let mates: Vec<(i32, i32)> = members(game)
        .into_iter()
        .filter(|&mate| mate != body)
        .flat_map(|mate| battle.cells_of(mate))
        .collect();
    if mates.is_empty() {
        return 0.0;
    }
    let n = mates.len() as i32;
    let centroid = (
        (mates.iter().map(|c| c.0).sum::<i32>() as f32 / n as f32).round() as i32,
        (mates.iter().map(|c| c.1).sum::<i32>() as f32 / n as f32).round() as i32,
    );
    let stands = crate::tactical::footprint_cells_at(cell, battle.footprint_of(body));
    let d = reach::gap(&stands, &[centroid]);
    -TACTICAL_COHESION_WEIGHT * d.saturating_sub(TACTICAL_COHESION_SLACK) as f32
}

/// The multiplier on `body`'s closing term.
pub(crate) fn closing_scale(game: &Game, body: Entity) -> f32 {
    if !in_pack(game, body) {
        return 1.0;
    }
    match effective(game) {
        PackTactic::Rush | PackTactic::Together => 1.0,
        PackTactic::Hold => 0.0,
    }
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
