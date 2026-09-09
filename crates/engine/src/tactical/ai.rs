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

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::abilities::{AbilityDef, AbilityRange, AbilityTarget};
use crate::components::{Hostile, Stats};
use crate::policy;
use crate::resources::GameRng;
use crate::tactical::map::Board;
use crate::tactical::reach::{distance, line_of_sight};
use crate::tactical::{TacticalBattle, reach};
use crate::tuning::{
    ENEMY_ROUTINE_MIN_COOLDOWN, TACTICAL_AI_CLOSING_WEIGHT, TACTICAL_AI_CROWDING_WEIGHT,
    TACTICAL_AI_REACH_SCORE, TACTICAL_AI_TEMPERATURE, TACTICAL_FIELD_RADIUS, TACTICAL_MELEE_RANGE,
};

/// What the acting body means to do this turn.
///
/// Decided before the walk, because `Intent::band` is the whole of what
/// tells closing apart from holding off.
enum Intent {
    /// The basic attack `tactical_attack` swings, which is adjacent-only
    /// whatever the species' `ranged` flag says — that flag is the group
    /// model's answer to reach, and standing next to somebody is this one's.
    Swing,
    Routine(AbilityDef),
}

impl Intent {
    /// The spread of distances this intent wants to be at.
    fn band(&self) -> AbilityRange {
        match self {
            Intent::Swing => AbilityRange {
                min: 0,
                max: TACTICAL_MELEE_RANGE,
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
            Intent::Swing => false,
            Intent::Routine(def) => matches!(
                def.target,
                AbilityTarget::OneAlly | AbilityTarget::WholeParty
            ),
        }
    }
}

/// The two sides of a fight as the scoring reads them: where everyone the
/// acting body is fighting stands, and where everyone standing with it does.
#[derive(Default)]
struct Sides {
    targets: Vec<(i32, i32)>,
    allies: Vec<(i32, i32)>,
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
    let crowd = allies
        .iter()
        .filter(|&&a| distance(cell, a) <= TACTICAL_FIELD_RADIUS)
        .count();

    (if hits { TACTICAL_AI_REACH_SCORE } else { 0.0 })
        - TACTICAL_AI_CLOSING_WEIGHT * gap as f32
        - TACTICAL_AI_CROWDING_WEIGHT * crowd as f32
}

impl Game {
    /// Runs the acting body's whole turn, and reports whether it did.
    ///
    /// `false` means the turn is not one this file drives — no fight open,
    /// nobody acting, or the body belongs to the player. A driver reads that
    /// as "wait for input".
    pub fn tactical_ai_turn(&mut self) -> bool {
        self.tactical_ai_turn_at(TACTICAL_AI_TEMPERATURE)
    }

    /// The acting body, when it is this file's to drive.
    ///
    /// **One definition, two readers.** `tactical_ai_turn_at` spends the
    /// turn it names and `Game::tactical_awaits_input` is its complement —
    /// asked separately they would eventually disagree about a body that is
    /// neither the player nor a hostile, and the fight would either hang
    /// waiting for a key nobody may press or move a companion by itself.
    ///
    /// **Every party body is the player's to command**, so the gate is
    /// `Hostile` and not `Player`: a companion standing on a battle map
    /// waits for input exactly as the player does.
    fn tactical_ai_actor(&self) -> Option<Entity> {
        let actor = self.world.get_resource::<TacticalBattle>()?.actor()?;
        self.world.get::<Hostile>(actor).is_some().then_some(actor)
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

    /// Runs the acting body's turn **whichever side it is on**, and reports
    /// whether there was one.
    ///
    /// `tactical_ai_actor`'s gate is `Hostile` because every party body is
    /// the player's to command — so a fight with nobody at the keyboard
    /// cannot be resolved through the door above, which is the whole of why
    /// this one exists. **Its only caller is `arena::run`**: called from a
    /// real fight it would walk a companion by itself, which is exactly the
    /// failure that gate is there to prevent.
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
        self.run_tactical_turn(actor, TACTICAL_AI_TEMPERATURE);
        true
    }

    /// One body's whole turn: decide, walk, act, hand on.
    ///
    /// Shared by the two doors above so the fight a measurement watched is
    /// the fight a player would have watched.
    fn run_tactical_turn(&mut self, actor: Entity, temperature: f32) {
        let Sides { targets, allies } = self.tactical_sides(actor);
        if targets.is_empty() {
            self.tactical_end_turn();
            return;
        }

        // `wild_routine_ready` and not `ability_unavailable`: a hostile holds
        // no `PowerReserve` by design, so the player's gate refuses it every
        // priced routine there is. See `Game::run_tactical_routine`.
        //
        // A party body is offered none of it — see `tactical_drive_turn`,
        // the only way one reaches this at all.
        let intent = match self.wild_routine_ready(actor) {
            Some(def) if self.world.get::<Hostile>(actor).is_some() => Intent::Routine(def),
            _ => Intent::Swing,
        };
        self.walk_to_best_cell(actor, &intent, &targets, &allies, temperature);

        match intent {
            Intent::Routine(def) => self.run_tactical_intent(actor, def, &targets),
            Intent::Swing => self.swing_at_best_neighbour(actor, &targets),
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
    }

    /// Everyone `actor` is fighting, and everyone standing with it — cells
    /// rather than entities, because that is all the scoring reads.
    ///
    /// Sidedness is `Hostile` and nothing else, so the party's own bodies and
    /// the player are one list. The acting body is in neither.
    ///
    /// Read **relative to `actor`** rather than as "hostiles are the enemy":
    /// the arena drives both sides through this, and the absolute reading
    /// hands a party body its own side to swing at. For a hostile actor the
    /// two readings are the same list, which is why no seeded fight moved.
    fn tactical_sides(&self, actor: Entity) -> Sides {
        let battle = self.world.resource::<TacticalBattle>();
        let acting_side = self.world.get::<Hostile>(actor).is_some();
        let mut sides = Sides::default();
        for (body, cell) in battle.bodies() {
            if body == actor {
                continue;
            }
            if (self.world.get::<Hostile>(body).is_some()) == acting_side {
                sides.allies.push(cell);
            } else {
                sides.targets.push(cell);
            }
        }
        sides
    }

    /// Picks the cell `actor` will act from and puts it there.
    ///
    /// **The whole walk is committed as one placement** rather than as a
    /// run of `tactical_step`s. Nothing on this board reacts to a body
    /// mid-walk — there are no opportunity attacks and no cell that does
    /// anything on entry — so a path would be a sequence with no observable
    /// difference from its endpoint, and `movement_field` has already
    /// answered which endpoints are legal and what each costs.
    fn walk_to_best_cell(
        &mut self,
        actor: Entity,
        intent: &Intent,
        targets: &[(i32, i32)],
        allies: &[(i32, i32)],
        temperature: f32,
    ) {
        let allowance = self.movement_allowance(actor);
        let battle = self.world.resource::<TacticalBattle>();
        let field = reach::movement_field(battle, actor, allowance);
        if field.is_empty() {
            return;
        }
        let band = intent.band();
        // Sorted, because `movement_field` answers a `HashMap` and iteration
        // order over one is not stable between runs: two equally-scored
        // cells must not resolve differently in a seeded fight.
        let mut cells: Vec<((i32, i32), u32)> = field.into_iter().collect();
        cells.sort_by_key(|&((x, y), _)| (y, x));
        let scores: Vec<f32> = cells
            .iter()
            .map(|&(cell, _)| cell_score(&battle.board, cell, band, targets, allies))
            .collect();

        // One draw a turn, and none at all at temperature zero:
        // `sample_scored` returns the argmax before it touches the RNG, so
        // the branch that would have skipped it is the one this does not
        // need.
        let pick = {
            let mut rng = self.world.resource_mut::<GameRng>();
            policy::sample_scored(&scores, temperature, &mut rng.0)
        };
        let (cell, cost) = cells[pick];

        let mut battle = self.world.resource_mut::<TacticalBattle>();
        if battle.move_to(actor, cell) {
            battle.spend(cost);
        }
    }

    /// Aims the routine `actor` has already committed to and runs it.
    ///
    /// The aim is a plain argmax and spends no randomness: the cell is where
    /// this turn's uncertainty lives, and a second draw on top of it would
    /// make a hostile miss aims it had already walked into position for.
    fn run_tactical_intent(&mut self, actor: Entity, def: AbilityDef, targets: &[(i32, i32)]) {
        let Some(aim) = self.best_aim(actor, &def, targets) else {
            return;
        };
        self.run_tactical_routine(actor, &def, aim, ENEMY_ROUTINE_MIN_COOLDOWN);
    }

    /// The cell to aim `def` at from where `actor` now stands, or `None` when
    /// nothing it can reach is worth hitting.
    ///
    /// Scored on who the shape actually covers rather than on the aim point,
    /// which is what makes an area routine land on a cluster instead of on
    /// one body: **its own side counts against it**, because
    /// `reach::recipients` never reads `Hostile` and a blast aimed through a
    /// packmate lands on the packmate.
    fn best_aim(
        &self,
        actor: Entity,
        def: &AbilityDef,
        targets: &[(i32, i32)],
    ) -> Option<(i32, i32)> {
        let battle = self.world.resource::<TacticalBattle>();
        let from = battle.cell_of(actor)?;
        let band = def.tactical_range();
        let shape = def.tactical_shape();
        let helpful = Intent::Routine(def.clone()).helpful();
        let reach_max = i32::try_from(band.max).unwrap_or(0);

        let mut best: Option<((i32, i32), i32)> = None;
        for dy in -reach_max..=reach_max {
            for dx in -reach_max..=reach_max {
                let aim = (from.0 + dx, from.1 + dy);
                if !battle.board.in_bounds(aim.0, aim.1) || !reach::in_range(from, aim, band) {
                    continue;
                }
                let mut worth = 0;
                for body in reach::recipients(battle, actor, aim, shape) {
                    let wanted = if helpful {
                        body == actor || self.world.get::<Hostile>(body).is_some()
                    } else {
                        !targets.is_empty() && self.world.get::<Hostile>(body).is_none()
                    };
                    worth += if wanted { 1 } else { -1 };
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

    /// Swings at the adjacent target with the least Integrity left.
    ///
    /// Deliberately not `battle::slot_aggro_weight`: a slot is the group
    /// model's answer to who is exposed, and on a battle map being reachable
    /// at all is that answer — the body that walked into range chose to be
    /// there. What is left to decide is which of the bodies now in reach to
    /// finish, and the wounded one is worth more than a fresh one.
    fn swing_at_best_neighbour(&mut self, actor: Entity, targets: &[(i32, i32)]) {
        let battle = self.world.resource::<TacticalBattle>();
        let Some(from) = battle.cell_of(actor) else {
            return;
        };
        let mut reachable: Vec<(i32, i32, Entity)> = targets
            .iter()
            .filter(|&&cell| distance(from, cell) <= TACTICAL_MELEE_RANGE)
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
        if let Some(&(_, _, target)) = reachable.first() {
            self.tactical_attack(target);
        }
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
