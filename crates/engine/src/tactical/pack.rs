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
    TACTICAL_FLANK_CROWD, TACTICAL_FLANK_PINCER, TACTICAL_FOCUS_BONUS, TACTICAL_FOCUS_CAP,
    TACTICAL_FORMATION_BACK_CLOSING, TACTICAL_FORMATION_DEPTH, TACTICAL_FORMATION_WEIGHT,
    TACTICAL_HOLD_ALERT_RANGE, TACTICAL_HOLD_COVER_BONUS, TACTICAL_HOLD_ROUNDS,
    TACTICAL_HOLD_SIGHT_CHARGE, TACTICAL_MELEE_RANGE,
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
    /// A line of the melee bodies advancing across the way to the enemy,
    /// the ranged ones behind it, until the front rank is gone.
    Formation,
    /// Swing at what a packmate is already on, from the side it is not.
    Flank,
}

impl PackTactic {
    /// Every tactic, in the column order of `tuning::PACK_TACTIC_ODDS`.
    pub const ALL: [PackTactic; 5] = [
        PackTactic::Rush,
        PackTactic::Together,
        PackTactic::Hold,
        PackTactic::Formation,
        PackTactic::Flank,
    ];

    /// Whether `members` can play this at all. Checked before the draw, so
    /// an ineligible tactic has weight zero rather than being rerolled.
    fn eligible(self, game: &Game, members: &[Entity]) -> bool {
        match self {
            PackTactic::Rush | PackTactic::Hold => true,
            PackTactic::Together | PackTactic::Flank => members.len() >= 2,
            PackTactic::Formation => {
                let back = members.iter().filter(|&&m| back_rank(game, m)).count();
                back > 0 && back < members.len()
            }
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
    footprint: u8,
    terms: Terms,
}

/// What each tactic's terms read, gathered once by `plan`.
/// A body's place in a formation.
#[derive(Clone, Copy)]
struct Slot {
    at: (i32, i32),
    /// The unit step toward the enemy.
    facing: (i32, i32),
}

impl Slot {
    /// How far `cell` is from the slot along `step`, in steps: positive in
    /// the step's direction. A diagonal unit step has squared length two.
    fn along(&self, cell: (i32, i32), step: (i32, i32)) -> f32 {
        let (dx, dy) = (cell.0 - self.at.0, cell.1 - self.at.1);
        let norm = step.0.pow(2) + step.1.pow(2);
        (dx * step.0 + dy * step.1) as f32 / norm as f32
    }
}

enum Terms {
    Rush,
    Together {
        /// The mean of the packmates, one point per mate (a squad is one
        /// body).
        mates: Option<(i32, i32)>,
        /// `TACTICAL_COHESION_FACTOR` times the body's own closing weight.
        cohesion: f32,
    },
    Hold {
        targets: Vec<(i32, i32)>,
    },
    Formation {
        back: bool,
        /// `None` for a front-rank body already in the fight, which has left
        /// the line, and for a pack with nobody to face.
        slot: Option<Slot>,
    },
    Flank {
        /// Every enemy body and how many packmates are within melee of it,
        /// already capped.
        focus: Vec<(Entity, u32)>,
        /// The body this one means to swing at, and the sector each packmate
        /// within melee of it holds.
        target: Option<((i32, i32), Vec<u8>)>,
    },
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
    let terms = match tactic {
        PackTactic::Rush => Terms::Rush,
        PackTactic::Together => {
            let points: Vec<(f32, f32)> = members(game)
                .into_iter()
                .filter(|&mate| mate != body)
                .filter_map(|mate| mean(&battle.cells_of(mate)))
                .collect();
            Terms::Together {
                mates: mean_of(&points).map(round),
                cohesion: closing * TACTICAL_COHESION_FACTOR,
            }
        }
        PackTactic::Hold => Terms::Hold {
            targets: game.tactical_sides(body).targets,
        },
        PackTactic::Formation => {
            let back = back_rank(game, body);
            let engaged = !back && beside_an_outsider(game, body);
            Terms::Formation {
                back,
                slot: if engaged {
                    None
                } else {
                    formation_slot(game, body)
                },
            }
        }
        PackTactic::Flank => flank_terms(game, body),
    };
    PackPlan {
        footprint: battle.footprint_of(body),
        terms,
    }
}

/// The mean of `cells`; `None` for no cells.
fn mean(cells: &[(i32, i32)]) -> Option<(f32, f32)> {
    let points: Vec<(f32, f32)> = cells.iter().map(|c| (c.0 as f32, c.1 as f32)).collect();
    mean_of(&points)
}

fn mean_of(points: &[(f32, f32)]) -> Option<(f32, f32)> {
    let n = points.len() as f32;
    (n > 0.0).then(|| {
        (
            points.iter().map(|p| p.0).sum::<f32>() / n,
            points.iter().map(|p| p.1).sum::<f32>() / n,
        )
    })
}

fn round(point: (f32, f32)) -> (i32, i32) {
    (point.0.round() as i32, point.1.round() as i32)
}

/// The mean of `cells`, rounded to a cell.
fn centre(cells: &[(i32, i32)]) -> Option<(i32, i32)> {
    mean(cells).map(round)
}

/// Whether `body` belongs in a formation's back rank: it can hit something
/// from beyond melee, by its swing or by a ready routine.
///
/// Derived each turn and never stored, so a body whose routine comes off
/// cooldown can change rank, and the roll's eligibility asks the same thing.
fn back_rank(game: &Game, body: Entity) -> bool {
    game.swing_range(body) > TACTICAL_MELEE_RANGE
        || game
            .ready_routines(body)
            .iter()
            .any(|def| game.routine_tactical_range(def).max > TACTICAL_MELEE_RANGE)
}

/// Every body on the board that is not in the pack.
fn outsiders(game: &Game) -> Vec<Entity> {
    let pack = members(game);
    game.world
        .resource::<TacticalBattle>()
        .bodies()
        .map(|(body, _)| body)
        .filter(|body| !pack.contains(body))
        .collect()
}

/// Whether a body outside the pack is within melee of `body`.
fn beside_an_outsider(game: &Game, body: Entity) -> bool {
    let battle = game.world.resource::<TacticalBattle>();
    let ours = battle.cells_of(body);
    outsiders(game)
        .into_iter()
        .any(|other| reach::gap(&ours, &battle.cells_of(other)) <= TACTICAL_MELEE_RANGE)
}

/// The compass sector, 0 (east) to 7 counter-clockwise in board terms, that
/// the step `(dx, dy)` points into; `None` for no step at all.
fn sector(dx: i32, dy: i32) -> Option<u8> {
    if dx == 0 && dy == 0 {
        return None;
    }
    let turns = (dy as f32).atan2(dx as f32) / std::f32::consts::FRAC_PI_4;
    Some((turns.round() as i32).rem_euclid(8) as u8)
}

/// The unit step pointing into compass `sector`.
fn sector_step(sector: u8) -> (i32, i32) {
    const STEPS: [(i32, i32); 8] = [
        (1, 0),
        (1, 1),
        (0, 1),
        (-1, 1),
        (-1, 0),
        (-1, -1),
        (0, -1),
        (1, -1),
    ];
    STEPS[sector as usize % 8]
}

/// Where along a line each body stands, in the order given (the first is the
/// toughest): the first in the middle, then alternating outward, each taking
/// as many cells as it is wide.
pub(crate) fn line_offsets(widths: &[u8]) -> Vec<i32> {
    let Some(&first) = widths.first() else {
        return Vec::new();
    };
    let mut offsets = vec![0];
    let mut right = first as i32;
    let mut left = 0;
    for (i, &w) in widths.iter().enumerate().skip(1) {
        if i % 2 == 1 {
            offsets.push(right);
            right += w as i32;
        } else {
            left -= w as i32;
            offsets.push(left);
        }
    }
    offsets
}

/// The cell `body` holds in the pack's formation, a pure function of where
/// the pack stands and what each member can reach.
///
/// The line faces from the pack's centre toward the nearest outsider,
/// snapped to a compass point; the front rank stands across it centred on its
/// own centre, the back rank `TACTICAL_FORMATION_DEPTH` behind. Each rank is
/// ordered toughest first, ties to initiative order.
fn formation_slot(game: &Game, body: Entity) -> Option<Slot> {
    let battle = game.world.resource::<TacticalBattle>();
    let order = battle.initiative();
    let mut pack = members(game);
    pack.sort_by_key(|m| order.iter().position(|o| o == m).unwrap_or(usize::MAX));
    let cells_of = |members: &[Entity]| -> Vec<(i32, i32)> {
        members.iter().flat_map(|&m| battle.cells_of(m)).collect()
    };
    let middle = centre(&cells_of(&pack))?;
    let facing_to = outsiders(game)
        .into_iter()
        .flat_map(|o| battle.cells_of(o))
        .min_by_key(|&c| (reach::distance(middle, c), c.1, c.0))?;
    let facing = sector_step(sector(facing_to.0 - middle.0, facing_to.1 - middle.1)?);
    let across = (-facing.1, facing.0);
    let max_hp = |m: Entity| game.world.get::<Stats>(m).map_or(0, |s| s.max_hp);
    let back = back_rank(game, body);
    let (backs, front): (Vec<Entity>, Vec<Entity>) =
        pack.iter().partition(|&&m| back_rank(game, m));
    let mut rank = if back { backs } else { front.clone() };
    // Stable, so equal toughness keeps initiative order.
    rank.sort_by_key(|&m| std::cmp::Reverse(max_hp(m)));
    let line = centre(&cells_of(&front))?;
    let at = if back {
        (
            line.0 - facing.0 * TACTICAL_FORMATION_DEPTH,
            line.1 - facing.1 * TACTICAL_FORMATION_DEPTH,
        )
    } else {
        line
    };
    let widths: Vec<u8> = rank.iter().map(|&m| battle.footprint_of(m)).collect();
    let i = rank.iter().position(|&m| m == body)?;
    let offset = line_offsets(&widths)[i];
    Some(Slot {
        at: (at.0 + across.0 * offset, at.1 + across.1 * offset),
        facing,
    })
}

/// How many other packmates stand within melee of each enemy body of
/// `body`'s, capped at `TACTICAL_FOCUS_CAP`.
fn focus_counts(game: &Game, body: Entity, enemies: &[Entity]) -> Vec<(Entity, u32)> {
    let battle = game.world.resource::<TacticalBattle>();
    let mates: Vec<Vec<(i32, i32)>> = members(game)
        .into_iter()
        .filter(|&mate| mate != body)
        .map(|mate| battle.cells_of(mate))
        .collect();
    enemies
        .iter()
        .map(|&enemy| {
            let theirs = battle.cells_of(enemy);
            let near = mates
                .iter()
                .filter(|cells| reach::gap(cells, &theirs) <= TACTICAL_MELEE_RANGE)
                .count() as u32;
            (enemy, near.min(TACTICAL_FOCUS_CAP))
        })
        .collect()
}

/// `Flank`'s terms for `body`: focus on every enemy, and the target it would
/// swing at from where it stands with that focus counted, with the sectors
/// its packmates already hold around it.
fn flank_terms(game: &Game, body: Entity) -> Terms {
    let battle = game.world.resource::<TacticalBattle>();
    let mut enemies: Vec<((i32, i32), Entity)> = game
        .tactical_sides(body)
        .targets
        .iter()
        .filter_map(|&cell| battle.occupant(cell).map(|e| (cell, e)))
        .collect();
    enemies.sort_by_key(|&(cell, _)| (cell.1, cell.0));
    enemies.dedup_by_key(|&mut (_, e)| e);
    let bodies: Vec<Entity> = enemies.iter().map(|&(_, e)| e).collect();
    let focus = focus_counts(game, body, &bodies);
    let target = battle.cell_of(body).and_then(|from| {
        let worth = |t: Entity| focus_bonus(&focus, t);
        let target = game.rank_swing_targets_by(body, from, &bodies, worth)?;
        let at = battle.cell_of(target)?;
        let theirs = battle.cells_of(target);
        let held = members(game)
            .into_iter()
            .filter(|&mate| mate != body)
            .filter(|&mate| reach::gap(&battle.cells_of(mate), &theirs) <= TACTICAL_MELEE_RANGE)
            .filter_map(|mate| battle.cell_of(mate))
            .filter_map(|cell| sector(cell.0 - at.0, cell.1 - at.1))
            .collect();
        Some((at, held))
    });
    Terms::Flank { focus, target }
}

fn focus_bonus(focus: &[(Entity, u32)], target: Entity) -> f32 {
    focus
        .iter()
        .find(|&&(e, _)| e == target)
        .map_or(0.0, |&(_, n)| TACTICAL_FOCUS_BONUS * n as f32)
}

impl PackPlan {
    /// The multiplier on the body's closing term.
    pub(crate) fn closing_scale(&self) -> f32 {
        match self.terms {
            Terms::Rush | Terms::Together { .. } | Terms::Flank { .. } => 1.0,
            Terms::Formation { back: false, .. } => 1.0,
            Terms::Formation { back: true, .. } => TACTICAL_FORMATION_BACK_CLOSING,
            Terms::Hold { .. } => 0.0,
        }
    }

    /// What a swing at `target` is worth on top of the profile's own score:
    /// `Flank`'s focus, and nothing under any other tactic.
    pub(crate) fn focus(&self, target: Entity) -> f32 {
        match &self.terms {
            Terms::Flank { focus, .. } => focus_bonus(focus, target),
            _ => 0.0,
        }
    }

    /// What standing at `cell` adds to its worth under the tactic.
    pub(crate) fn cell_terms(&self, board: &crate::tactical::map::Board, cell: (i32, i32)) -> f32 {
        let stands = crate::tactical::footprint_cells_at(cell, self.footprint);
        match &self.terms {
            Terms::Rush => 0.0,
            // The slack-rope pull toward the packmates: free within
            // `TACTICAL_COHESION_SLACK` of them, a linear charge beyond.
            Terms::Together { mates, cohesion } => mates.map_or(0.0, |centroid| {
                let d = reach::gap(&stands, &[centroid]);
                -cohesion * d.saturating_sub(TACTICAL_COHESION_SLACK) as f32
            }),
            // Cover-seeking must agree with the trigger: a cell the party
            // would see from is charged past anything cover pays, so the walk
            // to shelter never ends where the hold would then break.
            Terms::Hold { targets } => {
                let seen = targets.iter().any(|&t| in_view(board, &stands, &[t]));
                let charge = if seen {
                    TACTICAL_HOLD_SIGHT_CHARGE
                } else {
                    0.0
                };
                TACTICAL_HOLD_COVER_BONUS * cover_share(board, &stands, targets) - charge
            }
            // **Both ranks are charged on one axis only**, because each slot
            // is read from where the front rank stands now. The front rank
            // pays for straying along the line: a charge for the whole
            // distance would price every step forward at what closing pays,
            // and the line would never advance. The back rank pays only for
            // standing ahead of its slot: held to the slot outright, it
            // stood out of its own range while the front fought and the
            // pack lost most of its damage (the phase 2 measurement).
            Terms::Formation { slot, back } => slot.map_or(0.0, |slot| {
                let off = if *back {
                    slot.along(cell, slot.facing).max(0.0)
                } else {
                    let across = (-slot.facing.1, slot.facing.0);
                    slot.along(cell, across).abs()
                };
                -TACTICAL_FORMATION_WEIGHT * off
            }),
            Terms::Flank { target, .. } => target.as_ref().map_or(0.0, |(at, held)| {
                let Some(mine) = sector(cell.0 - at.0, cell.1 - at.1) else {
                    return 0.0;
                };
                let crowd = held.iter().filter(|&&s| s == mine).count() as f32;
                let pincer = held.contains(&((mine + 4) % 8));
                -TACTICAL_FLANK_CROWD * crowd + if pincer { TACTICAL_FLANK_PINCER } else { 0.0 }
            }),
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
    if breaking(game, state.tactic).is_some() {
        PackTactic::Rush
    } else {
        state.tactic
    }
}

/// The line to log if `tactic` has a reason to end now: a hold sighted, hit
/// or out of patience, or a formation with no front rank left.
fn breaking(game: &Game, tactic: PackTactic) -> Option<&'static str> {
    match tactic {
        PackTactic::Hold if hold_should_break(game) => Some("The pack breaks cover!"),
        PackTactic::Formation if members(game).iter().all(|&m| back_rank(game, m)) => {
            Some("The line breaks!")
        }
        _ => None,
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

/// Ends a hold or a formation if it has a reason to, returning the line to
/// log.
///
/// Called at the start of every hostile beat, before the intent is chosen.
/// The first trigger to fire ends the tactic for the whole pack, and a pack
/// already playing `Rush` answers `None`.
pub(crate) fn check_transition(game: &mut Game) -> Option<&'static str> {
    let battle = game.world.get_resource::<TacticalBattle>()?;
    let line = breaking(game, battle.pack.tactic)?;
    game.set_pack_tactic(PackTactic::Rush);
    Some(line)
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
pub(crate) fn odds_row(id: &str) -> [u32; 5] {
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
            if tactic.eligible(game, members) {
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
        PackTactic::Formation => Some("The pack forms a line, ranged programs behind."),
        PackTactic::Flank => Some("The pack fans out to surround you."),
    }
}
