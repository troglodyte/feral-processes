//! What a hostile does with its turn on a battle map.
//!
//! Entirely hand-written, and deliberately not `combat_policy.rs`: that
//! file's trained weights and its whole feature vector speak group indices
//! and aggro slots, which is a vocabulary a battle map does not have. What
//! replaces a slot here is where a body is standing, so the decision this
//! file makes is a decision about a cell.
//!
//! One turn is: decide what to do, pick the cell to do it from, walk there,
//! do it. The action is decided *first* because a routine's range is what a
//! cell is scored against — a body that picked its ground before it knew
//! whether it was closing or holding off would have to guess.
//!
//! **The walk is spent one cell at a time, through the door the player's own
//! arrow keys go through.** A turn is a run of `AiBeat`s — a step, then
//! another step, then the action — because a hostile that crossed six cells
//! between two frames read as a teleport, and a fight the player is watching
//! is the one thing that makes a path observably different from its
//! endpoint. `tactical_ai_turn` is still the whole turn: it is this loop,
//! run to the end.

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::abilities::{AbilityDef, AbilityId, AbilityRange, AbilityTarget, TamperSlot};
use crate::components::{Hostile, Stats, Tampered};
use crate::policy;
use crate::resources::GameRng;
use crate::tactical::map::Board;
use crate::tactical::reach::{distance, line_of_sight};
use crate::tactical::turn::StepOutcome;
use crate::tactical::{TacticalBattle, reach};
use crate::tuning::{
    ENEMY_ROUTINE_MIN_COOLDOWN, TACTICAL_AI_CLOSING_WEIGHT, TACTICAL_AI_CROWDING_WEIGHT,
    TACTICAL_AI_REACH_SCORE, TACTICAL_AI_TEMPERATURE, TACTICAL_FIELD_RADIUS,
};

/// What the acting body means to do this turn.
///
/// Decided before the walk, because `Intent::band` is the whole of what
/// tells closing apart from holding off.
enum Intent {
    /// The basic attack `tactical_attack` swings, carrying the range
    /// `Game::swing_range` answered for the body about to act.
    ///
    /// Carried on the variant rather than read inside `band()` because the
    /// band is decided **before the walk** and `band` takes no actor — and
    /// because a range read at swing time would let a body plan a standoff
    /// and then draw the melee half of its move pair.
    Swing {
        range: u32,
    },
    Routine(AbilityDef),
}

impl Intent {
    /// The spread of distances this intent wants to be at.
    fn band(&self) -> AbilityRange {
        match self {
            Intent::Swing { range } => AbilityRange {
                min: 0,
                max: *range,
            },
            Intent::Routine(def) => def.tactical_range(),
        }
    }

    /// Whether this lands on the invoker's own side.
    ///
    /// Read off the authored `target` rather than off the effect, so a mod
    /// that aims a novel effect at its own party is aimed correctly with no
    /// change here — and so this stays one expression rather than a second
    /// census of which effects are kind.
    fn helpful(&self) -> bool {
        match self {
            Intent::Swing { .. } => false,
            Intent::Routine(def) => matches!(
                def.target,
                AbilityTarget::OneAlly | AbilityTarget::WholeParty
            ),
        }
    }
}

/// What one beat of a body's turn spent.
///
/// Three answers rather than a `bool` because a driver pacing the fight owes
/// a different wait after each: a step is one cell of a walk and the next is
/// due soon, an action ends the turn and the next body is due after a beat
/// the player can read the blow in, and there was nothing to drive at all.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AiBeat {
    /// The body walked one cell. The rest of its turn is still owed.
    Stepped,
    /// The body spent its action — or passed — and the turn has been handed
    /// on.
    Acted,
    /// Nobody this file may drive is acting.
    Idle,
}

/// The two sides of a fight as the scoring reads them: where everyone the
/// acting body is fighting stands, and where everyone standing with it does.
#[derive(Default)]
struct Sides {
    targets: Vec<(i32, i32)>,
    allies: Vec<(i32, i32)>,
}

/// What a turn's action lands on, decided from a cell the body stands on —
/// or will stand on, once its walk is spent.
///
/// Three answers because there are three doors an action goes through: a
/// swing at a body, a strike at a decoy, and a routine aimed at a cell.
enum TurnTarget {
    Body(Entity),
    Decoy((i32, i32)),
    Aim((i32, i32)),
}

/// What a `Profiled` hostile will do with its next turn, published so the
/// player can play around it.
///
/// **A call into the planner the turn runs**, never a restatement — see
/// `Game::tactical_forecast`.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Forecast {
    pub action: ForecastAction,
    /// The cells the body will walk through, destination last. Empty above
    /// temperature zero, and when it will hold its ground.
    pub walk: Vec<(i32, i32)>,
    /// The cell its action will land on. `None` above temperature zero, and
    /// when nothing will be in reach from where the walk ends.
    pub target: Option<(i32, i32)>,
}

/// Which action a forecast names.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ForecastAction {
    Swing,
    Routine(AbilityId),
}

/// How far outside `band` a body at `from` is from `to`, in cells. Zero when
/// it is inside it.
///
/// **This is the term that tells closing from holding off**, and it is why
/// the score cannot simply read the distance to the nearest enemy: a body
/// standing inside its routine's minimum range wants to walk *away*, and a
/// distance term would march it further in.
fn shortfall(from: (i32, i32), to: (i32, i32), band: AbilityRange) -> u32 {
    let d = distance(from, to);
    if d < band.min {
        band.min - d
    } else {
        d.saturating_sub(band.max)
    }
}

/// What a cell is worth to a body that intends `band` against `targets`,
/// with `allies` already on the board.
///
/// Pure, and takes cells rather than entities, so the three terms can be
/// read against each other without a fight to hold them.
fn cell_score(
    board: &Board,
    cell: (i32, i32),
    band: AbilityRange,
    targets: &[(i32, i32)],
    allies: &[(i32, i32)],
) -> f32 {
    let crowd = allies
        .iter()
        .filter(|&&a| distance(cell, a) <= TACTICAL_FIELD_RADIUS)
        .count();
    cell_merit(board, cell, band, targets) - TACTICAL_AI_CROWDING_WEIGHT * crowd as f32
}

/// The part of `cell_score` that is a reason to walk: the reach bonus and
/// the closing term, without crowding.
///
/// **Split out because crowding is a tie-break and never a reason on its
/// own.** A body only considers cells that beat where it already stands on
/// this figure — see `Game::walk_to_best_cell` — so a hostile that can
/// already act holds its ground, and crowding only chooses among cells a
/// body had some other reason to walk to.
fn cell_merit(board: &Board, cell: (i32, i32), band: AbilityRange, targets: &[(i32, i32)]) -> f32 {
    // Line of sight is asked only of a cell that is already in band, so a
    // cell behind cover scores as one that has closed but cannot fire —
    // better than standing further back, worse than stepping around.
    let hits = targets
        .iter()
        .any(|&t| shortfall(cell, t, band) == 0 && line_of_sight(board, cell, t));
    let gap = targets
        .iter()
        .map(|&t| shortfall(cell, t, band))
        .min()
        .unwrap_or(0);

    (if hits { TACTICAL_AI_REACH_SCORE } else { 0.0 }) - TACTICAL_AI_CLOSING_WEIGHT * gap as f32
}

impl Game {
    /// `TACTICAL_AI_TEMPERATURE`, or `body`'s own `Tampered` temperature when
    /// it carries one — a Cold Sample or Heat Injection overriding the
    /// tuning constant for exactly the body it landed on.
    ///
    /// **The one door.** Every production call site in this file reads this
    /// rather than the constant, so a later call site cannot miss a Cold
    /// Sample by reading around it. `tactical_ai_turn_at` stays the explicit
    /// test hook it always was — it takes a temperature as an argument
    /// rather than asking this.
    pub(crate) fn decision_temperature(&self, body: Entity) -> f32 {
        self.world
            .get::<Tampered>(body)
            .and_then(Tampered::temperature)
            .unwrap_or(TACTICAL_AI_TEMPERATURE)
    }

    /// Which side `actor`'s own decision-making treats as its own — not
    /// necessarily `Hostile(actor)`.
    ///
    /// **The one door onto allegiance in this file.** `tactical_sides` and
    /// `best_aim` both read this rather than `Hostile` directly, so an
    /// injected body's flipped reading lands in one place and a later tamper
    /// that also flips a side has the same door to go through.
    ///
    /// An injected hostile's policy runs as if it stood on the party's
    /// side: `Hostile(actor) != Injected(actor)` is true only when the two
    /// disagree, which is exactly the case a `prompt_injection` lands.
    fn acts_for_hostiles(&self, actor: Entity) -> bool {
        let hostile = self.world.get::<Hostile>(actor).is_some();
        let injected = self
            .world
            .get::<Tampered>(actor)
            .is_some_and(|tampered| tampered.has(TamperSlot::Injected));
        hostile != injected
    }

    /// Whether `body` is a companion the AI drives rather than the player —
    /// the design's "going mad": a `Temperature` or `Injected` entry takes it
    /// over for as long as the entry lasts, whether or not `App::tactical_auto`
    /// is on.
    ///
    /// Never the player and never `Hostile`, which is already the AI's every
    /// other way. `Profiled` and `Hallucinating` do not take a body over —
    /// only the two kinds that also bend a decision: which cell to draw and
    /// which side to swing at.
    pub(crate) fn taken_over(&self, body: Entity) -> bool {
        if self.world.get::<Hostile>(body).is_some() || body == self.player_entity() {
            return false;
        }
        self.world.get::<Tampered>(body).is_some_and(|tampered| {
            tampered.temperature().is_some() || tampered.has(TamperSlot::Injected)
        })
    }

    /// Runs the acting body's whole turn, and reports whether it did.
    ///
    /// `false` means the turn is not one this file drives — no fight open,
    /// nobody acting, or the body belongs to the player. A driver reads that
    /// as "wait for input".
    pub fn tactical_ai_turn(&mut self) -> bool {
        let Some(actor) = self.tactical_ai_actor() else {
            return false;
        };
        self.tactical_ai_turn_at(self.decision_temperature(actor))
    }

    /// The acting body, when it is this file's to drive.
    ///
    /// **One definition, two readers.** `tactical_ai_turn_at` spends the
    /// turn it names and `Game::tactical_awaits_input` is its complement —
    /// asked separately they would eventually disagree about a body that is
    /// neither the player nor a hostile, and the fight would either hang
    /// waiting for a key nobody may press or move a companion by itself.
    ///
    /// **Every party body is the player's to command with two exceptions**,
    /// so the gate is `Hostile`, `Summoned` or `taken_over` and not `Player`:
    /// a companion standing on a battle map waits for input exactly as the
    /// player does, unless a fork fielded it (it is the first party body
    /// that drives itself) or a Temperature or Injected entry has taken it
    /// over for the length of that entry.
    ///
    /// Read the fork exception as the feature rather than as a bug: it is
    /// fielded by a routine, not brought to the fight, and asking the player
    /// to command one would make `fork_cluster` three more turns of
    /// bookkeeping a round. The taken-over exception is the design's own —
    /// see `taken_over`'s doc. Sidedness needs nothing — `tactical_sides` is
    /// already relative to the actor, which is why `tactical_drive_turn`
    /// works at all.
    fn tactical_ai_actor(&self) -> Option<Entity> {
        let actor = self.world.get_resource::<TacticalBattle>()?.actor()?;
        (self.world.get::<Hostile>(actor).is_some()
            || self
                .world
                .get::<crate::components::Summoned>(actor)
                .is_some()
            || self.taken_over(actor))
        .then_some(actor)
    }

    /// What `body` will do with its next turn, when it is a `Profiled`
    /// hostile that has not started one — `None` otherwise, and `None` with
    /// nothing to fight.
    ///
    /// **A call into the planner the turn runs, never a restatement**:
    /// `tactical_intent`, `scored_cells` and `chosen_target` are what
    /// `run_tactical_beat` spends, and `argmax_scored` is the index
    /// `sample_scored` answers at temperature zero. A forecast that kept its
    /// own copy would be right until the AI was retuned, and the copy that
    /// drifts is the one the player trusts.
    ///
    /// **The action is always named; the walk and target only at temperature
    /// zero.** The intent is decided before any draw, so it is honest at every
    /// temperature. Above zero the cell is a draw, and a drawn path that is
    /// wrong one time in three teaches the player the forecast lies.
    ///
    /// **Withdrawn mid-turn**, because a body part-way through a walk plans
    /// nothing more — what it will do is already on the board. Read off the
    /// board as it stands, so it is exact at the moment the turn begins and
    /// moves as the party acts before then. Read-only: no draw, no write.
    #[cfg_attr(
        not(test),
        expect(
            dead_code,
            reason = "the tactical view reads the forecast from Task 8 on"
        )
    )]
    pub(crate) fn tactical_forecast(&self, body: Entity) -> Option<Forecast> {
        let battle = self.world.get_resource::<TacticalBattle>()?;
        let profiled = self
            .world
            .get::<Tampered>(body)
            .is_some_and(|tampered| tampered.has(TamperSlot::Profiled));
        if self.world.get::<Hostile>(body).is_none() || !profiled {
            return None;
        }
        if battle.actor() == Some(body) && (battle.walk_planned() || battle.acted()) {
            return None;
        }
        let from = battle.cell_of(body)?;
        let sides = self.tactical_sides(body);
        if sides.targets.is_empty() {
            return None;
        }
        let intent = self.tactical_intent(body);
        let action = match &intent {
            Intent::Swing { .. } => ForecastAction::Swing,
            Intent::Routine(def) => ForecastAction::Routine(def.id.clone()),
        };
        if self.decision_temperature(body) > 0.0 {
            return Some(Forecast {
                action,
                walk: Vec::new(),
                target: None,
            });
        }

        let (cells, scores) = self.scored_cells(body, &intent, &sides);
        let walk = if cells.is_empty() {
            Vec::new()
        } else {
            self.path_for(body, cells[policy::argmax_scored(&scores)])
        };
        // From where the walk ends rather than from the chosen cell: an empty
        // path is a body that acts from where it stands, whatever it picked.
        let destination = walk.last().copied().unwrap_or(from);
        let target = self
            .chosen_target(body, destination, &intent, &sides)
            .and_then(|target| match target {
                TurnTarget::Body(entity) => battle.cell_of(entity),
                TurnTarget::Decoy(cell) | TurnTarget::Aim(cell) => Some(cell),
            });
        Some(Forecast {
            action,
            walk,
            target,
        })
    }

    /// Whether the fight is waiting on a key rather than on the AI.
    ///
    /// `tactical_ai_actor`'s complement, and `false` with no fight open or
    /// nobody acting — there is nothing to wait for either way.
    pub fn tactical_awaits_input(&self) -> bool {
        self.tactical_actor().is_some() && self.tactical_ai_actor().is_none()
    }

    /// `tactical_ai_turn` with the softmax temperature supplied rather than
    /// read from `tuning`. `choose_wild_action_at`'s reason: a dial-back
    /// nobody can vary is a dial-back nobody has checked works — and at zero
    /// this spends no `GameRng` at all, which is what lets a test pin the
    /// choice without moving the seeded stream.
    pub(crate) fn tactical_ai_turn_at(&mut self, temperature: f32) -> bool {
        let Some(actor) = self.tactical_ai_actor() else {
            return false;
        };
        self.run_tactical_turn(actor, temperature);
        true
    }

    /// Spends one beat of the acting hostile's turn: one cell of its walk,
    /// or the action that ends it.
    ///
    /// **`tactical_ai_turn`'s fine grain, and the coarse one is written in
    /// terms of it** rather than beside it — a fight watched a beat at a time
    /// and the same fight resolved in one call have to reach the same board,
    /// and two implementations of a turn is how they would come not to.
    ///
    /// A driver that wants the walk drawn calls this and paces itself off the
    /// answer; a driver that only wants the fight resolved calls
    /// `tactical_ai_turn`.
    pub fn tactical_ai_beat(&mut self) -> AiBeat {
        let Some(actor) = self.tactical_ai_actor() else {
            return AiBeat::Idle;
        };
        self.run_tactical_beat(actor, self.decision_temperature(actor))
    }

    /// Spends one beat of the acting body's turn **whichever side it is on**.
    ///
    /// `tactical_ai_beat`'s door with its `Hostile`/`Summoned`/taken-over
    /// gate lifted entirely, and the third onto one `run_tactical_beat` —
    /// the fight the player watches resolve itself has to be the fight they
    /// would have fought by hand. What it exists for is auto-attack: a
    /// player who has asked for one owes the party's turns to somebody, and
    /// this is who.
    ///
    /// **It does not make a party body the AI's.** `tactical_awaits_input`
    /// still answers `true` for one, which is what keeps "every party body is
    /// the player's to command" true of the engine — the decision to answer
    /// for it is app-core's, taken a key at a time, and revoked the same way.
    ///
    /// A party body driven here **swings and never invokes**, and that is
    /// `run_tactical_beat`'s own gate rather than a second rule: the routine
    /// branch is `Hostile`-only because `run_tactical_routine` charges Power
    /// through a door that never asks `ability_unavailable`, so the
    /// alternative is a party that invokes whatever it carries for free.
    pub fn tactical_auto_beat(&mut self) -> AiBeat {
        let Some(actor) = self.tactical_actor() else {
            return AiBeat::Idle;
        };
        self.run_tactical_beat(actor, self.decision_temperature(actor))
    }

    /// Whether the acting body is part-way through a walk it has committed
    /// to, and so owes a step rather than a turn.
    ///
    /// `false` with no fight open, and `false` on a turn nothing has planned
    /// yet — a body about to set off is not walking, which is what buys the
    /// beat of anticipation before it does.
    pub fn tactical_walking(&self) -> bool {
        self.world
            .get_resource::<TacticalBattle>()
            .is_some_and(|battle| battle.walking())
    }

    /// Whether the acting body has not yet spent anything on its turn, so
    /// what a driver owes it is the *hand-over* wait rather than the pause
    /// between arriving and striking.
    ///
    /// `tactical_walking`'s sibling and derived the same way, off the fight
    /// rather than remembered: a body that has planned a walk carries a
    /// `Some` — `Some(vec![])` once it has arrived — and a body that has
    /// swung carries `acted`, so all three states are told apart without a
    /// driver having to hold what the last beat did.
    pub fn tactical_turn_opening(&self) -> bool {
        self.world
            .get_resource::<TacticalBattle>()
            .is_some_and(|battle| !battle.walk_planned() && battle.spent() == 0 && !battle.acted())
    }

    /// Runs the acting body's turn **whichever side it is on**, and reports
    /// whether there was one.
    ///
    /// `tactical_ai_actor`'s gate is `Hostile`, `Summoned` or taken-over
    /// because every other party body is the player's to command — so a
    /// fight with nobody at the keyboard cannot be resolved through the door
    /// above, which is the whole of why this one exists. **Its only caller
    /// is `arena::run`**: called from a real fight it would walk a
    /// companion by itself, which is exactly the failure that gate is
    /// there to prevent.
    ///
    /// A party body **swings and never invokes**, which is not a policy
    /// invented for the tester: `PartyPlan::AllAttack` is the group model's
    /// own arena plan and it invokes no routine either, so a number taken
    /// on a battle map stays comparable with the one taken in front of a
    /// group. `run_tactical_turn` is where that lands.
    pub(crate) fn tactical_drive_turn(&mut self) -> bool {
        let Some(actor) = self.tactical_actor() else {
            return false;
        };
        self.run_tactical_turn(actor, self.decision_temperature(actor));
        true
    }

    /// One body's whole turn: decide, walk, act, hand on.
    ///
    /// Shared by the two doors above so the fight a measurement watched is
    /// the fight a player would have watched — and **the beat loop rather
    /// than a second spelling of a turn**, so that holds for a fight paced
    /// in front of the player too.
    fn run_tactical_turn(&mut self, actor: Entity, temperature: f32) {
        while self.run_tactical_beat(actor, temperature) == AiBeat::Stepped {}
    }

    /// One beat of `actor`'s turn: the next cell of its walk, or the action
    /// that ends the turn.
    ///
    /// The walk is planned on the beat that takes its first step and read
    /// back off `TacticalBattle` by every beat after it, which is what holds
    /// this to **one `GameRng` draw a turn** rather than one a cell.
    fn run_tactical_beat(&mut self, actor: Entity, temperature: f32) -> AiBeat {
        let sides = self.tactical_sides(actor);
        if sides.targets.is_empty() {
            self.tactical_end_turn();
            return AiBeat::Acted;
        }
        if self.step_along_walk(actor) {
            return AiBeat::Stepped;
        }

        // Asked again on the beat that acts rather than carried across the
        // walk: it reads cooldowns and a routine list, neither of which a
        // walk moves, so the answer is the one the walk was scored against
        // and storing an `AbilityDef` on the fight would be a second copy of
        // it.
        let intent = self.tactical_intent(actor);
        if !self.world.resource::<TacticalBattle>().walk_planned() {
            self.walk_to_best_cell(actor, &intent, &sides, temperature);
            if self.step_along_walk(actor) {
                return AiBeat::Stepped;
            }
        }

        match intent {
            Intent::Routine(_) => self.run_tactical_intent(actor, &intent, &sides),
            Intent::Swing { .. } => self.swing_at_best_neighbour(actor, &intent, &sides),
        }
        // **Only if the action did not already hand it on.** The action ends
        // the turn, so `tactical_attack` and `tactical_use_routine` both end
        // it themselves; ending it again here spends two rungs of the order
        // and skips whoever came next, which against a lone hostile is a
        // fight the player never gets a turn in. A body that found nothing
        // to swing at ended none, and still owes one.
        //
        // Asked as "is this body still up" rather than tracked as a flag: a
        // fight that ended inside the action took the resource with it, and
        // that is the same question with the same answer.
        let still_up = self
            .world
            .get_resource::<TacticalBattle>()
            .and_then(|b| b.actor())
            == Some(actor);
        if still_up {
            self.tactical_end_turn();
        }
        AiBeat::Acted
    }

    /// What `actor` means to do with its turn: its ready routine, or a swing.
    ///
    /// Pure, and decided before any draw — which is what lets a forecast
    /// name the action honestly at every temperature.
    fn tactical_intent(&self, actor: Entity) -> Intent {
        // `wild_routine_ready` and not `ability_unavailable`: a hostile holds
        // no `PowerReserve` by design, so the player's gate refuses it every
        // priced routine there is. See `Game::run_tactical_routine`.
        //
        // A party body is offered none of it — see `tactical_drive_turn`,
        // the only way one reaches this at all.
        match self.wild_routine_ready(actor) {
            Some(def) if self.world.get::<Hostile>(actor).is_some() => Intent::Routine(def),
            _ => Intent::Swing {
                range: self.swing_range(actor),
            },
        }
    }

    /// Takes the next cell off `actor`'s committed walk and steps it there,
    /// reporting whether there was one.
    ///
    /// **Through `Game::tactical_step`, the door the player's own arrow keys
    /// go through**, so a hostile's step is priced, bounded and refused by
    /// exactly the code a companion's is — the alternative is a second
    /// implementation of what a step costs, and the one that drifts is the
    /// one nobody plays.
    ///
    /// A refused step abandons the rest of the walk rather than retrying it:
    /// the path was legal when it was planned and nothing on this board moves
    /// between beats, so a refusal means the plan is wrong about the world
    /// and the body is better off acting from where it stands than standing
    /// still forever.
    fn step_along_walk(&mut self, actor: Entity) -> bool {
        let battle = self.world.resource::<TacticalBattle>();
        let Some(from) = battle.cell_of(actor) else {
            return false;
        };
        let Some(next) = self.world.resource_mut::<TacticalBattle>().take_walk_step() else {
            return false;
        };
        let dir = (next.0 - from.0, next.1 - from.1);
        match self.tactical_step(dir) {
            StepOutcome::Moved => true,
            // `Struck` is unreachable from here — `reach::movement_field`
            // treats every body as a wall, so a committed path never names an
            // occupied cell, and nothing on this board moves between one
            // body's beats — and is grouped with the refusals rather than
            // given an arm of its own: if it ever did fire, the action is
            // spent and the rest of the walk is owed to nobody.
            //
            // **Not `tactical_awaits_input`, which used to be the reason.**
            // That is false for every body the *AI* door drives and true for
            // every body `tactical_auto_beat` drives, so auto-attack would
            // have left the claim resting on a predicate that no longer says
            // it.
            //
            // `get_resource_mut`, because a departure closes the fight and
            // takes the resource with it.
            StepOutcome::Departed | StepOutcome::Refused | StepOutcome::Struck => {
                if let Some(mut battle) = self.world.get_resource_mut::<TacticalBattle>() {
                    battle.commit_walk(Vec::new());
                }
                false
            }
        }
    }

    /// Everyone `actor` is fighting, and everyone standing with it — cells
    /// rather than entities, because that is all the scoring reads.
    ///
    /// Sidedness is `acts_for_hostiles`, not a bare `Hostile` read, so the
    /// party's own bodies and the player are one list — for an untampered
    /// actor the two are the same question.
    ///
    /// Read **relative to `actor`** rather than as "hostiles are the enemy":
    /// the arena drives both sides through this, and the absolute reading
    /// hands a party body its own side to swing at. For a hostile actor the
    /// two readings are the same list, which is why no seeded fight moved.
    ///
    /// **An injected body's own side is swapped by that same relativity.**
    /// `acts_for_hostiles` answers `false` for one, so its packmates land in
    /// `targets` and the party lands in `allies` with no branch here at
    /// all — the flip is entirely `acting_side`'s, and every other body
    /// still reads `Hostile` as it always has, which is what keeps an
    /// uninjected packmate treating the injected one as its own.
    ///
    /// **A cloaked body leaves `targets`**, which is what keeps it out of
    /// `best_aim`'s scoring and out of `swing_at_best_neighbour` — the fifth
    /// door that names a body. `allies` is untouched: a cloak hides a body
    /// from being aimed at, not from being stood beside, and the crowding
    /// term is about where there is room to stand.
    ///
    /// **The never-empty rule.** With every hostile-side body cloaked the
    /// filter is skipped, because an empty `targets` list is what
    /// `Intent::choose` reads as "nothing to fight" — a whole side that
    /// stopped closing, stopped swinging and stood still until the caps
    /// expired.
    fn tactical_sides(&self, actor: Entity) -> Sides {
        let battle = self.world.resource::<TacticalBattle>();
        let acting_side = self.acts_for_hostiles(actor);
        let mut sides = Sides::default();
        let mut hidden: Vec<(i32, i32)> = Vec::new();
        for (body, cell) in battle.bodies() {
            if body == actor {
                continue;
            }
            if (self.world.get::<Hostile>(body).is_some()) == acting_side {
                sides.allies.push(cell);
            } else if self.is_cloaked(body) {
                hidden.push(cell);
            } else {
                sides.targets.push(cell);
            }
        }
        if sides.targets.is_empty() {
            sides.targets = hidden;
        }
        // **A hallucinating body fights the decoy it sees nearest, and only
        // that.** It walks at it, swings at it or aims at it through the same
        // scoring a body gets — `walk_to_best_cell` closes on `targets`
        // whatever stands there. `allies` is untouched: the decoy changes
        // what it is fighting, not where there is room to stand.
        //
        // With no decoy it sees, `settle_decoys` has already taken the entry,
        // so this falls through to the real sides — never an empty list,
        // which `run_tactical_beat` reads as nothing to fight.
        if let Some(decoy) = self.nearest_seen_decoy(actor) {
            sides.targets = vec![decoy];
        }
        sides
    }

    /// Picks the cell `actor` will act from and commits the walk to it.
    ///
    /// **The cell is chosen once and the path to it is spent one step at a
    /// time**, through `Game::tactical_step`. The choice is still a single
    /// decision — nothing on this board reacts to a body mid-walk, so there
    /// is nothing to reconsider between cells — but the steps are real,
    /// because a fight is watched and a body crossing six cells in one frame
    /// reads as a teleport rather than as an approach.
    ///
    /// The path is descended from the field `movement_field` already
    /// answered, so which cells are legal and what each costs is settled in
    /// one place.
    fn walk_to_best_cell(
        &mut self,
        actor: Entity,
        intent: &Intent,
        sides: &Sides,
        temperature: f32,
    ) {
        // Committed on every path out, empty ones included: an unplanned walk
        // is re-planned by the next beat, and a body that found nowhere worth
        // going would draw again every beat for the rest of the turn.
        self.world
            .resource_mut::<TacticalBattle>()
            .commit_walk(Vec::new());
        let (cells, scores) = self.scored_cells(actor, intent, sides);
        if cells.is_empty() {
            return;
        }

        // One draw a turn, and none at all at temperature zero:
        // `sample_scored` returns the argmax before it touches the RNG, so
        // the branch that would have skipped it is the one this does not
        // need.
        let pick = {
            let mut rng = self.world.resource_mut::<GameRng>();
            policy::sample_scored(&scores, temperature, &mut rng.0)
        };
        let path = self.path_for(actor, cells[pick]);
        self.world
            .resource_mut::<TacticalBattle>()
            .commit_walk(path);
    }

    /// The cells `actor` would choose among this turn, and what each is
    /// worth — empty when it stands nowhere or can reach nowhere.
    ///
    /// **Pure, so the turn's draw and a forecast's argmax are taken over one
    /// list.** Sorted before it is scored, so an index into the scores names
    /// the same cell whoever reads it.
    fn scored_cells(
        &self,
        actor: Entity,
        intent: &Intent,
        sides: &Sides,
    ) -> (Vec<(i32, i32)>, Vec<f32>) {
        let allowance = self.movement_allowance(actor);
        let battle = self.world.resource::<TacticalBattle>();
        let field = reach::movement_field(battle, actor, allowance);
        let Some(from) = battle.cell_of(actor) else {
            return (Vec::new(), Vec::new());
        };
        if field.is_empty() {
            return (Vec::new(), Vec::new());
        }
        let band = intent.band();
        // **Staying put is the default.** The candidates are the cells that
        // are strictly better than this one on reach or closing, and the
        // cell it stands on only when there are none. Offered every cell
        // instead, a body already in reach drew among all the cells that
        // reached as well as its own did — softmax over a tie is a uniform
        // draw — and sidestepped nearly every turn for no reason a player
        // could see.
        //
        // The hold still goes through the draw, so a turn spends one draw
        // whichever way it goes and holding does not reshuffle every roll
        // after it.
        let standing = cell_merit(&battle.board, from, band, &sides.targets);
        let mut cells: Vec<(i32, i32)> = field
            .keys()
            .copied()
            .filter(|&cell| cell_merit(&battle.board, cell, band, &sides.targets) > standing)
            .collect();
        if cells.is_empty() {
            cells.push(from);
        }
        // Sorted, because `movement_field` answers a `HashMap` and iteration
        // order over one is not stable between runs: two equally-scored
        // cells must not resolve differently in a seeded fight.
        cells.sort_by_key(|&(x, y)| (y, x));
        let scores: Vec<f32> = cells
            .iter()
            .map(|&cell| cell_score(&battle.board, cell, band, &sides.targets, &sides.allies))
            .collect();
        (cells, scores)
    }

    /// The path `actor` walks to reach `to`, descended from the field
    /// `movement_field` already answered, so which cells are legal and what
    /// each costs is settled in one place.
    fn path_for(&self, actor: Entity, to: (i32, i32)) -> Vec<(i32, i32)> {
        let allowance = self.movement_allowance(actor);
        let battle = self.world.resource::<TacticalBattle>();
        let Some(from) = battle.cell_of(actor) else {
            return Vec::new();
        };
        let field = reach::movement_field(battle, actor, allowance);
        reach::path_to(&battle.board, &field, from, to)
    }

    /// Aims the routine `actor` has already committed to and runs it.
    ///
    /// The aim is a plain argmax and spends no randomness: the cell is where
    /// this turn's uncertainty lives, and a second draw on top of it would
    /// make a hostile miss aims it had already walked into position for.
    fn run_tactical_intent(&mut self, actor: Entity, intent: &Intent, sides: &Sides) {
        let Intent::Routine(def) = intent else {
            return;
        };
        let Some(TurnTarget::Aim(aim)) = self.target_from_here(actor, intent, sides) else {
            return;
        };
        self.run_tactical_routine(actor, def, aim, ENEMY_ROUTINE_MIN_COOLDOWN);
    }

    /// `chosen_target` asked from the cell `actor` stands on now — what the
    /// acting beat reads, once the walk is spent.
    fn target_from_here(
        &self,
        actor: Entity,
        intent: &Intent,
        sides: &Sides,
    ) -> Option<TurnTarget> {
        let from = self.world.resource::<TacticalBattle>().cell_of(actor)?;
        self.chosen_target(actor, from, intent, sides)
    }

    /// What `actor`'s action would land on if it acted from `from`, or
    /// `None` when nothing it could reach from there is worth it.
    ///
    /// **Takes the cell rather than reading the body's own**, so the turn
    /// asks it from where the walk ended and a forecast asks it from where
    /// the walk will end — one derivation, two moments.
    fn chosen_target(
        &self,
        actor: Entity,
        from: (i32, i32),
        intent: &Intent,
        sides: &Sides,
    ) -> Option<TurnTarget> {
        match intent {
            Intent::Routine(def) => self
                .best_aim(actor, from, def, &sides.targets)
                .map(TurnTarget::Aim),
            Intent::Swing { range } => self.best_swing(actor, from, *range, &sides.targets),
        }
    }

    /// The cell to aim `def` at from `from`, or `None` when
    /// nothing it can reach is worth hitting.
    ///
    /// Scored on who the shape actually covers rather than on the aim point,
    /// which is what makes an area routine land on a cluster instead of on
    /// one body: **its own side counts against it**, because
    /// `reach::recipients` never reads `Hostile` and a blast aimed through a
    /// packmate lands on the packmate.
    ///
    /// **"Wanted" is read through `acts_for_hostiles`, actor-relative like
    /// `tactical_sides`.** A helpful routine wants the actor itself or a body
    /// on the actor's own side; an aggressive one wants a body on the other
    /// side. For an injected actor `acts_for_hostiles` answers `false`, so a
    /// Heal lands on the party it now calls its own and a Damage penalises a
    /// packmate exactly the way it used to penalise a companion.
    ///
    /// The aggressive branch's `!targets.is_empty()` guard is unreachable in
    /// practice: `run_tactical_beat` already returns before this is called
    /// once `tactical_sides` answers an empty `targets`.
    ///
    /// **A hallucinating body aims at its decoys instead.** An aggressive
    /// routine scores +1 for every decoy it sees in the shape, −1 for every
    /// body on its own side, and nothing for the other side's bodies, which
    /// it cannot see for the decoys — so the blast goes where the fakes are
    /// and still refuses to go through a packmate. A helpful routine is aimed
    /// exactly as it always was: the decoys are something to fight, not
    /// something to mend.
    fn best_aim(
        &self,
        actor: Entity,
        from: (i32, i32),
        def: &AbilityDef,
        targets: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        let battle = self.world.resource::<TacticalBattle>();
        let band = def.tactical_range();
        let shape = def.tactical_shape();
        let helpful = Intent::Routine(def.clone()).helpful();
        let reach_max = i32::try_from(band.max).unwrap_or(0);
        let acting_side = self.acts_for_hostiles(actor);
        let hallucinating = !helpful && self.is_hallucinating(actor);

        let mut best: Option<((i32, i32), i32)> = None;
        for dy in -reach_max..=reach_max {
            for dx in -reach_max..=reach_max {
                let aim = (from.0 + dx, from.1 + dy);
                if !battle.board.in_bounds(aim.0, aim.1)
                    || !reach::in_range(from, aim, band)
                    // The same gate the player's own door applies, asked here
                    // rather than inside the effect for the same reason the
                    // range is: an aim it may not take is not a candidate.
                    || !reach::aim_in_sight(&battle.board, from, aim, shape)
                {
                    continue;
                }
                let mut worth = 0;
                if hallucinating {
                    worth += reach::shape_cells(&battle.board, from, aim, shape)
                        .into_iter()
                        .filter(|&cell| self.sees_decoy_at(actor, cell))
                        .count() as i32;
                }
                for body in reach::recipients_from(battle, actor, from, aim, shape) {
                    let body_is_hostile = self.world.get::<Hostile>(body).is_some();
                    worth += if helpful {
                        if body == actor || body_is_hostile == acting_side {
                            1
                        } else {
                            -1
                        }
                    } else if body_is_hostile == acting_side {
                        -1
                    } else if hallucinating {
                        // The other side is hidden behind the decoys, so a
                        // body standing among them is neither a reason to aim
                        // there nor one not to.
                        0
                    } else if !targets.is_empty() {
                        1
                    } else {
                        -1
                    };
                }
                if worth <= 0 {
                    continue;
                }
                // Strictly greater, over cells walked in (y, x) order, so the
                // tie-break is the board's own reading order and not a hash.
                if best.is_none_or(|(_, seen)| worth > seen) {
                    best = Some((aim, worth));
                }
            }
        }
        best.map(|(aim, _)| aim)
    }

    /// Swings at the reachable target with the least Integrity left.
    ///
    /// Deliberately not `battle::slot_aggro_weight`: a slot is the group
    /// model's answer to who is exposed, and on a battle map being reachable
    /// at all is that answer — the body that walked into range chose to be
    /// there. What is left to decide is which of the bodies now in reach to
    /// finish, and the wounded one is worth more than a fresh one.
    fn swing_at_best_neighbour(&mut self, actor: Entity, intent: &Intent, sides: &Sides) {
        // A strike out of reach is simply refused, leaving the turn to be
        // ended by `run_tactical_beat` like any swing that found nothing.
        match self.target_from_here(actor, intent, sides) {
            Some(TurnTarget::Decoy(cell)) => {
                self.tactical_strike_decoy(cell);
            }
            Some(TurnTarget::Body(target)) => {
                self.tactical_attack(target);
            }
            Some(TurnTarget::Aim(_)) | None => {}
        }
    }

    /// The swing `swing_at_best_neighbour` takes from `from`, at `range`.
    fn best_swing(
        &self,
        actor: Entity,
        from: (i32, i32),
        range: u32,
        targets: &[(i32, i32)],
    ) -> Option<TurnTarget> {
        // A target this body sees a decoy on is struck through the door that
        // takes a cell — for a hallucinating body that is its only target.
        if let Some(&cell) = targets
            .iter()
            .find(|&&cell| self.sees_decoy_at(actor, cell))
        {
            return Some(TurnTarget::Decoy(cell));
        }
        let battle = self.world.resource::<TacticalBattle>();
        let mut reachable: Vec<(i32, i32, Entity)> = targets
            .iter()
            .filter(|&&cell| distance(from, cell) <= range)
            // Asked here as well as at the gate, so a body does not spend its
            // turn swinging at something it cannot see and calling that its
            // action.
            .filter(|&&cell| line_of_sight(&battle.board, from, cell))
            .filter_map(|&cell| battle.occupant(cell).map(|e| (cell.1, cell.0, e)))
            .collect();
        // By Integrity, then by the board's reading order, so a tie is broken
        // the same way twice in a seeded fight.
        reachable.sort_by_key(|&(y, x, e)| {
            (
                self.world.get::<Stats>(e).map(|s| s.hp).unwrap_or(i32::MAX),
                y,
                x,
            )
        });
        reachable
            .first()
            .map(|&(_, _, target)| TurnTarget::Body(target))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn open(side: usize) -> Board {
        let row = ".".repeat(side);
        let rows: Vec<&str> = (0..side).map(|_| row.as_str()).collect();
        Board::from_rows(&rows)
    }

    const MELEE: AbilityRange = AbilityRange { min: 0, max: 1 };
    const STANDOFF: AbilityRange = AbilityRange { min: 3, max: 6 };

    /// The term the whole standoff case rests on. A distance-to-target score
    /// would read "one cell away" as nearly perfect for a routine that
    /// cannot be fired inside three, and march the body further in.
    #[test]
    fn a_body_inside_its_minimum_range_is_as_short_as_one_outside_its_maximum() {
        assert_eq!(shortfall((0, 0), (1, 0), STANDOFF), 2, "one cell in");
        assert_eq!(shortfall((0, 0), (4, 0), STANDOFF), 0, "inside the band");
        assert_eq!(shortfall((0, 0), (9, 0), STANDOFF), 3, "three cells out");
        assert_eq!(shortfall((0, 0), (1, 0), MELEE), 0, "adjacent is melee");
    }

    /// A reaching body's band is its own range, not the melee constant — the
    /// whole of what makes it hold off rather than walk into arm's length.
    #[test]
    fn a_reaching_bodys_band_is_its_own_range() {
        let swing = Intent::Swing { range: 2 };
        assert_eq!(swing.band(), AbilityRange { min: 0, max: 2 });
        assert_eq!(
            shortfall((0, 0), (2, 0), swing.band()),
            0,
            "two cells is inside a range-2 band"
        );
        assert_eq!(
            shortfall((0, 0), (4, 0), swing.band()),
            2,
            "four cells is two short of the band"
        );
    }

    /// A standoff routine's carrier prefers the cell that holds the band over
    /// the cell that closes on the target, which is the difference between
    /// this and "walk at the player".
    #[test]
    fn a_standoff_carrier_scores_its_band_above_the_target_s_doorstep() {
        let board = open(12);
        let target = [(8, 4)];
        let held = cell_score(&board, (4, 4), STANDOFF, &target, &[]);
        let closed = cell_score(&board, (7, 4), STANDOFF, &target, &[]);
        assert!(
            held > closed,
            "holding the band scored {held}, the doorstep {closed}"
        );
    }

    /// The reach bonus has to outrank closing across the widest board there
    /// is, or a body walks past the swing it came for toward a target it
    /// merely likes the distance of.
    #[test]
    fn a_cell_it_can_hit_from_beats_every_cell_it_cannot() {
        let board = open(crate::tuning::TACTICAL_BOARD_LARGE as usize);
        let far = crate::tuning::TACTICAL_BOARD_LARGE - 1;
        let targets = [(1, 1)];
        let hitting = cell_score(&board, (2, 1), MELEE, &targets, &[]);
        let nearest_miss = cell_score(&board, (3, 1), MELEE, &targets, &[]);
        let across_the_board = cell_score(&board, (far, far), MELEE, &targets, &[]);
        assert!(hitting > nearest_miss, "{hitting} vs {nearest_miss}");
        assert!(
            nearest_miss > across_the_board,
            "closing must still order the cells it cannot hit from"
        );
        assert!(
            hitting > across_the_board + TACTICAL_AI_CLOSING_WEIGHT * far as f32,
            "the reach bonus must outrank the whole board's width"
        );
    }

    /// Crowding is a tie-break and nothing more: it separates two cells that
    /// both hit, and it must never outweigh hitting at all.
    #[test]
    fn crowding_separates_two_cells_that_both_reach_and_never_outranks_reaching() {
        let board = open(12);
        let targets = [(6, 6)];
        let packed = cell_score(&board, (5, 6), MELEE, &targets, &[(4, 6), (5, 5), (4, 5)]);
        let clear = cell_score(&board, (7, 6), MELEE, &targets, &[(4, 6), (5, 5), (4, 5)]);
        assert!(clear > packed, "clear {clear}, packed {packed}");
        let missing = cell_score(&board, (9, 6), MELEE, &targets, &[]);
        assert!(
            packed > missing,
            "a crowded cell that hits still beats a clear one that does not"
        );
    }

    /// Cover is read only of a cell already in band, so a body behind it is
    /// ordered between "has closed" and "can fire".
    #[test]
    fn a_cell_in_band_but_behind_cover_does_not_count_as_reaching() {
        let board = Board::from_rows(&["......", "......", "..#...", "......", "......", "......"]);
        let targets = [(2, 1)];
        let blocked = cell_score(&board, (2, 3), MELEE, &targets, &[]);
        let clear = cell_score(&board, (1, 1), MELEE, &targets, &[]);
        assert!(clear > blocked, "clear {clear}, blocked {blocked}");
    }
}
