//! Per-profile weights of the tactical AI, and the scored choice a hostile
//! makes with them: swing or routine, and which body to swing at.
//!
//! **Deterministic argmax over features, no draw.** The only AI draw a turn
//! is still the cell softmax, so `tactical_forecast` keeps predicting the
//! turn it is asked about.

use bevy_ecs::prelude::Entity;
use serde::{Deserialize, Serialize};

use crate::Game;
use crate::abilities::AbilityDef;
use crate::battle;
use crate::behaviours::BehaviourDef;
use crate::components::{Hostile, Stats};
use crate::tactical::ai::Intent;
use crate::tactical::{TacticalBattle, footprint_cells_at, reach};
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

/// Everything a hostile's choice between candidates can weigh. Named for the
/// profile files; a later sub-project appends, and a profile that does not
/// name a feature reads it as zero.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TacticalFeature {
    TargetHpFrac,
    EstDamageFrac,
    WouldKill,
    TargetIsPlayer,
    TargetStunned,
    TargetDistance,
    IsRoutine,
    RoutineNetRecipients,
    SelfHpFrac,
    SelfXTargetDistance,
    TargetHp,
}

impl TacticalFeature {
    pub const COUNT: usize = 11;
    pub const ALL: [TacticalFeature; Self::COUNT] = [
        Self::TargetHpFrac,
        Self::EstDamageFrac,
        Self::WouldKill,
        Self::TargetIsPlayer,
        Self::TargetStunned,
        Self::TargetDistance,
        Self::IsRoutine,
        Self::RoutineNetRecipients,
        Self::SelfHpFrac,
        Self::SelfXTargetDistance,
        Self::TargetHp,
    ];

    pub fn name(self) -> &'static str {
        match self {
            Self::TargetHpFrac => "target_hp_frac",
            Self::EstDamageFrac => "est_damage_frac",
            Self::WouldKill => "would_kill",
            Self::TargetIsPlayer => "target_is_player",
            Self::TargetStunned => "target_stunned",
            Self::TargetDistance => "target_distance",
            Self::IsRoutine => "is_routine",
            Self::RoutineNetRecipients => "routine_net_recipients",
            Self::SelfHpFrac => "self_hp_frac",
            Self::SelfXTargetDistance => "self_x_target_distance",
            Self::TargetHp => "target_hp",
        }
    }

    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|f| f.name() == name)
    }
}

pub(crate) type Features = [f32; TacticalFeature::COUNT];

/// A profile's weights, one per feature.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct TacticalWeights([f32; TacticalFeature::COUNT]);

impl TacticalWeights {
    /// `pinned` and `weights` together; a name this build does not know is
    /// skipped (it warned at load). Pinned is applied last, so it wins a
    /// feature named in both.
    pub fn of(def: &BehaviourDef) -> Self {
        let mut w = [0.0; TacticalFeature::COUNT];
        for map in [&def.weights, &def.pinned] {
            for (name, &value) in map {
                if let Some(f) = TacticalFeature::from_name(name) {
                    w[f as usize] = value;
                }
            }
        }
        Self(w)
    }
}

pub(crate) fn score(w: &TacticalWeights, f: &Features) -> f32 {
    w.0.iter().zip(f).map(|(w, f)| w * f).sum()
}

/// The index of the best score, the *first* of equals. `policy::argmax_scored`
/// keeps the last, which is right for a cell draw and wrong here: candidates
/// are sorted in reading order and a tie must go to the first, as the
/// lowest-Integrity rule it replaces broke one.
fn first_best(scores: &[f32]) -> usize {
    let mut best = 0;
    for (i, s) in scores.iter().enumerate() {
        if s.total_cmp(&scores[best]).is_gt() {
            best = i;
        }
    }
    best
}

fn hp_frac(game: &Game, body: Entity) -> f32 {
    game.world
        .get::<Stats>(body)
        .filter(|s| s.max_hp > 0)
        .map_or(1.0, |s| s.hp.max(0) as f32 / s.max_hp as f32)
}

impl Game {
    /// The features of `actor` swinging at `target` from `from`.
    ///
    /// **Cover is not read**: `defender_profile_against` asks where the
    /// attacker stands now, and a forecast asks from the cell the walk will
    /// end on, so reading it would let the forecast and the turn disagree.
    pub(crate) fn swing_features(
        &self,
        actor: Entity,
        from: (i32, i32),
        target: Entity,
    ) -> Features {
        let battle = self.world.resource::<TacticalBattle>();
        let swing = battle::Swing::plain(self.natural_range_of(actor));
        let expected = battle::expected_damage(
            self.combatant_profile(actor, swing),
            self.combatant_profile(target, swing),
        ) as f32;
        let target_hp = self
            .world
            .get::<Stats>(target)
            .map_or(1.0, |s| s.hp.max(1) as f32);
        let mine = footprint_cells_at(from, battle.footprint_of(actor));
        let distance =
            reach::gap(&mine, &battle.cells_of(target)) as f32 / battle.board.side.max(1) as f32;
        let self_frac = hp_frac(self, actor);
        let mut f = [0.0; TacticalFeature::COUNT];
        f[TacticalFeature::TargetHpFrac as usize] = hp_frac(self, target);
        f[TacticalFeature::TargetHp as usize] =
            self.world.get::<Stats>(target).map_or(0.0, |s| s.hp as f32);
        f[TacticalFeature::EstDamageFrac as usize] = expected / target_hp;
        f[TacticalFeature::WouldKill as usize] = f32::from(expected >= target_hp);
        f[TacticalFeature::TargetIsPlayer as usize] = f32::from(target == self.player_entity());
        f[TacticalFeature::TargetStunned as usize] = f32::from(self.is_stunned(target));
        f[TacticalFeature::TargetDistance as usize] = distance;
        f[TacticalFeature::SelfHpFrac as usize] = self_frac;
        f[TacticalFeature::SelfXTargetDistance as usize] = (1.0 - self_frac) * distance;
        f
    }

    fn routine_features(net_recipients: i32) -> Features {
        let mut f = [0.0; TacticalFeature::COUNT];
        f[TacticalFeature::IsRoutine as usize] = 1.0;
        f[TacticalFeature::RoutineNetRecipients as usize] = net_recipients as f32;
        f
    }

    /// Which of the bodies in `reachable` (reading order) `actor` swings at.
    pub(crate) fn rank_swing_targets(
        &self,
        actor: Entity,
        from: (i32, i32),
        reachable: &[Entity],
    ) -> Option<Entity> {
        let w = TacticalWeights::of(self.behaviour_of(actor));
        let scores: Vec<f32> = reachable
            .iter()
            .map(|&t| score(&w, &self.swing_features(actor, from, t)))
            .collect();
        reachable.get(first_best(&scores)).copied()
    }

    /// A hostile's routine-or-swing choice: its profile's argmax over a swing
    /// at each enemy body, in reading order, and each ready routine.
    ///
    /// The swing candidates are every body it fights rather than those in
    /// reach, because the walk has not happened yet; the walk is still
    /// scored for the band the winner names. With no ready routine there is
    /// nothing to choose.
    pub(super) fn choose_hostile_intent(&self, actor: Entity) -> Intent {
        let swing = Intent::Swing {
            range: self.swing_range(actor),
        };
        // Only the first ready routine is a candidate, as `wild_routine_ready`
        // always made it: a later one is never reached past a first that is
        // not worth choosing.
        let routines: Vec<AbilityDef> = self
            .wild_routine_ready(actor)
            .filter(|def| self.hostile_routine_worth_choosing(actor, def))
            .into_iter()
            .collect();
        if routines.is_empty() {
            return swing;
        }
        let battle = self.world.resource::<TacticalBattle>();
        let Some(from) = battle.cell_of(actor) else {
            return swing;
        };
        let w = TacticalWeights::of(self.behaviour_of(actor));
        let sides = self.tactical_sides(actor);
        let mut enemies: Vec<(i32, i32, Entity)> = sides
            .targets
            .iter()
            .filter_map(|&cell| battle.occupant(cell).map(|e| (cell.1, cell.0, e)))
            .collect();
        enemies.sort_by_key(|&(y, x, _)| (y, x));
        let mut scores: Vec<f32> = enemies
            .iter()
            .map(|&(_, _, e)| score(&w, &self.swing_features(actor, from, e)))
            .collect();
        let swings = scores.len();
        scores.extend(routines.iter().map(|def| {
            let net = self
                .best_aim_scored(actor, from, def, &sides.targets)
                .map_or(0, |(_, worth)| worth);
            score(&w, &Self::routine_features(net))
        }));
        match first_best(&scores).checked_sub(swings) {
            Some(i) => Intent::Routine(Box::new(routines[i].clone())),
            None => swing,
        }
    }

    /// How `actor` walks: its profile's cell weights if it decides by one,
    /// the global tuning otherwise.
    pub(crate) fn cell_weights_of(&self, actor: Entity) -> CellWeights {
        if self.decides_by_profile(actor) {
            self.behaviour_of(actor).cells
        } else {
            CellWeights::default()
        }
    }

    /// Whether `actor` decides by its species' profile: a `Hostile` body,
    /// injected or not. The party's bodies keep the old fixed rules.
    pub(crate) fn decides_by_profile(&self, actor: Entity) -> bool {
        self.world.get::<Hostile>(actor).is_some()
    }
}
