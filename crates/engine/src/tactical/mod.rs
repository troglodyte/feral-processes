//! The tactical battle model: a disposable grid a surface fight is fought
//! on.
//!
//! Opt-in, off by default, and the second of the game's two combat models —
//! see
//! `docs/superpowers/archive/specs/2026-09-09-tactical-surface-battles-design.md`.
//! `Game::start_battle` is the router that chooses between the two models,
//! by inspecting the pack; `arena::stage` is the second chooser, and takes
//! the model its caller can drive.
//!
//! **A battle coordinate lives here and nowhere else.** No world `Position`
//! is ever written for a body standing on a battle map, the same way the
//! Stack keeps its coordinates in `resources::Locale` and base space keeps
//! its own in `Locale::Base`. This module does not import `Position`.

pub mod ai;
pub mod deploy;
pub mod map;
pub mod reach;
pub mod turn;
pub mod view;

use std::collections::HashMap;

use bevy_ecs::prelude::{Entity, Resource};

use crate::resources::BattleRewards;
use crate::tactical::map::{BattleSpec, Board};

/// A tactical fight's spatial state: the map it is fought on and where
/// every body stands.
///
/// **The one place a battle coordinate lives.** No world `Position` is
/// written for a body on a battle map, the same way the Stack keeps its
/// coordinates in `resources::Locale` and base space keeps its own there
/// too. A body's `Position` stays exactly where the fight opened.
///
/// Bare `#[derive(Resource)]` — no `Default`, no `Serialize` — matching
/// `BattleState` for the same reason: a fight is never saved, so nothing
/// here appears in `save.rs` and `SAVE_FORMAT_VERSION` does not move.
#[derive(Resource)]
pub struct TacticalBattle {
    pub spec: BattleSpec,
    pub board: Board,
    /// A `Vec` and not a `HashMap`. Iteration order has to be stable —
    /// bevy's own query order is not, and anything that walks a fight's
    /// bodies must not resolve differently between runs, which is `Stock`'s
    /// `BTreeMap` rule again — and a fight holds at most thirteen bodies,
    /// so a linear scan is the simpler thing and also the faster one.
    bodies: Vec<(Entity, (i32, i32))>,
    /// What this fight has paid out so far, held back until it ends.
    ///
    /// The second arm of `Game::fight_rewards_mut`, and a field rather than
    /// a resource of its own for the reason `BattleRewards`' own doc gives:
    /// a new `Resource` shifts bevy's query iteration order under unrelated
    /// tests, and a fight's payout has no business doing that.
    pub(crate) rewards: BattleRewards,
    /// Every body still in the fight, fastest first — rolled once when the
    /// fight opens rather than fresh each round, so a turn-order strip is
    /// stable enough to plan against.
    ///
    /// **Kept in step by deletion, not by re-sorting.** A body that dies or
    /// walks off the edge leaves this list in `remove`, which is the whole
    /// of "re-sorted as bodies die": the survivors' relative order was
    /// settled at the bell and nothing later may disturb it.
    initiative: Vec<Entity>,
    /// Which entry of `initiative` is acting. Never points past the end
    /// while any body remains — `wrap` is what holds that.
    turn: usize,
    /// What the acting body has spent on movement this turn, against
    /// `Game::movement_allowance`.
    spent: u32,
    /// Whether the acting body has taken its action. The action ends the
    /// turn, so this is only ever read between the action landing and the
    /// turn being handed on.
    acted: bool,
    /// How many times the order has come round, from 1. The results
    /// header's figure and the telemetry's alike.
    pub round: u32,
    /// How many decompiles this fight has thrown at each program, so a
    /// program's defences fray across a fight and no longer.
    ///
    /// `BattleState::decompile_attempts`' counterpart, and a field here for
    /// `rewards`' reason: a counter of its own would be a `Resource`, and a
    /// new one shifts bevy's query iteration order under unrelated tests.
    /// Read and written through `Game::decompile_attempts`/`_mut`, never
    /// directly, so a capture rolls against the same count on a battle map
    /// as it does in front of a group.
    pub(crate) decompile_attempts: HashMap<Entity, u32>,
    /// Whether the hostiles outweighed the party at the bell, by summed
    /// `Stats::power()` — `BattleState::outmatched`'s counterpart, and a
    /// snapshot for its reason: by the time a fight is won the question is
    /// unanswerable.
    pub(crate) outmatched: bool,
}

impl TacticalBattle {
    pub fn open(spec: BattleSpec, board: Board) -> Self {
        TacticalBattle {
            spec,
            board,
            bodies: Vec::new(),
            rewards: BattleRewards::default(),
            decompile_attempts: HashMap::new(),
            initiative: Vec::new(),
            turn: 0,
            spent: 0,
            acted: false,
            round: 1,
            outmatched: false,
        }
    }

    /// Puts a body on a cell, or refuses.
    ///
    /// Refused when the cell cannot be stood on, when somebody is already
    /// there, or when this body is already on the board — the last so a
    /// double placement is a refusal rather than a second entry that
    /// `cell_of` would answer from and `occupant` would not.
    pub fn place(&mut self, body: Entity, cell: (i32, i32)) -> bool {
        if !self.board.walkable(cell.0, cell.1)
            || self.occupant(cell).is_some()
            || self.cell_of(body).is_some()
        {
            return false;
        }
        self.bodies.push((body, cell));
        true
    }

    pub fn cell_of(&self, body: Entity) -> Option<(i32, i32)> {
        self.bodies
            .iter()
            .find(|(e, _)| *e == body)
            .map(|(_, cell)| *cell)
    }

    pub fn occupant(&self, cell: (i32, i32)) -> Option<Entity> {
        self.bodies
            .iter()
            .find(|(_, at)| *at == cell)
            .map(|(e, _)| *e)
    }

    /// Moves a placed body, or refuses. Standing still is allowed.
    pub fn move_to(&mut self, body: Entity, cell: (i32, i32)) -> bool {
        if !self.board.walkable(cell.0, cell.1) {
            return false;
        }
        match self.occupant(cell) {
            Some(other) if other != body => return false,
            _ => {}
        }
        let Some(slot) = self.bodies.iter_mut().find(|(e, _)| *e == body) else {
            return false;
        };
        slot.1 = cell;
        true
    }

    /// Takes a body off the board — killed, or walked off the edge — and out
    /// of the turn order with it.
    ///
    /// **The cursor names a body, not a position.** Removing an entry ahead
    /// of the cursor shifts everything behind it down one, so the cursor
    /// follows; removing the *acting* body leaves the cursor already naming
    /// whoever stood behind it, which is a fresh turn and is reset as one.
    /// A body that dies on somebody else's turn costs the order nothing.
    pub fn remove(&mut self, body: Entity) {
        self.bodies.retain(|(e, _)| *e != body);
        let Some(idx) = self.initiative.iter().position(|&e| e == body) else {
            return;
        };
        self.initiative.remove(idx);
        if idx < self.turn {
            self.turn -= 1;
        } else if idx == self.turn {
            self.begin_turn();
        }
        self.wrap();
    }

    /// Seats the turn order, fastest first. Called once, when the fight
    /// opens.
    pub fn set_initiative(&mut self, order: Vec<Entity>) {
        self.initiative = order;
        self.turn = 0;
        self.begin_turn();
    }

    /// The turn order as it stands, fastest first.
    pub fn initiative(&self) -> &[Entity] {
        &self.initiative
    }

    /// Whose turn it is, or `None` when nobody is left to take one.
    pub fn actor(&self) -> Option<Entity> {
        self.initiative.get(self.turn).copied()
    }

    /// What the acting body has spent on movement so far this turn.
    pub fn spent(&self) -> u32 {
        self.spent
    }

    /// Whether the acting body has already taken its action.
    pub fn acted(&self) -> bool {
        self.acted
    }

    /// Charges `cost` against the acting body's movement.
    pub fn spend(&mut self, cost: u32) {
        self.spent += cost;
    }

    /// Records that the acting body has acted. The caller ends the turn —
    /// this only says the action landed, because a body killed by its own
    /// fumble leaves the order instead.
    pub fn mark_acted(&mut self) {
        self.acted = true;
    }

    /// Hands the turn to the next body in the order, starting a new round
    /// when it comes back round to the front.
    pub fn end_turn(&mut self) {
        self.turn += 1;
        self.wrap();
        self.begin_turn();
    }

    fn begin_turn(&mut self) {
        self.spent = 0;
        self.acted = false;
    }

    /// Brings the cursor back inside the order, counting a round each time
    /// it comes round. An empty order parks it at zero rather than counting
    /// rounds against a fight nobody is left in.
    fn wrap(&mut self) {
        if self.initiative.is_empty() {
            self.turn = 0;
            return;
        }
        if self.turn >= self.initiative.len() {
            self.turn = 0;
            self.round += 1;
        }
    }

    /// Every body and where it stands, in placement order.
    pub fn bodies(&self) -> impl Iterator<Item = (Entity, (i32, i32))> + '_ {
        self.bodies.iter().copied()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tactical::map::{BattleSpec, generate};
    use crate::world::Biome;
    use bevy_ecs::world::World;

    fn fight() -> (TacticalBattle, Vec<Entity>) {
        let spec = BattleSpec {
            world_seed: 5,
            site: (0, 0),
            tick: 10,
            zone: 1,
            biome: Biome::OpenGrid,
            bodies: 4,
        };
        let board = generate(spec);
        let mut world = World::new();
        let bodies = (0..3).map(|_| world.spawn_empty().id()).collect();
        (TacticalBattle::open(spec, board), bodies)
    }

    /// The one place a battle coordinate lives.
    #[test]
    fn a_body_stands_where_it_was_placed() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        assert!(battle.place(bodies[0], cell));
        assert_eq!(battle.cell_of(bodies[0]), Some(cell));
        assert_eq!(battle.occupant(cell), Some(bodies[0]));
    }

    fn first_open(battle: &TacticalBattle) -> (i32, i32) {
        battle
            .board
            .cells()
            .find(|(_, k)| k.walkable())
            .map(|(c, _)| c)
            .expect("a board with no ground")
    }

    #[test]
    fn two_bodies_never_share_a_cell() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        assert!(battle.place(bodies[0], cell));
        assert!(!battle.place(bodies[1], cell), "the cell was taken twice");
        assert_eq!(battle.occupant(cell), Some(bodies[0]));
    }

    #[test]
    fn nobody_stands_on_ground_they_cannot_stand_on() {
        let (mut battle, bodies) = fight();
        let blocked = battle
            .board
            .cells()
            .find(|(_, k)| !k.walkable())
            .map(|(c, _)| c)
            .expect("a board with nothing on it");
        assert!(!battle.place(bodies[0], blocked));
        assert_eq!(battle.cell_of(bodies[0]), None);
    }

    #[test]
    fn a_body_placed_twice_is_refused_rather_than_duplicated() {
        let (mut battle, bodies) = fight();
        let first = first_open(&battle);
        assert!(battle.place(bodies[0], first));
        let elsewhere = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != first)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        assert!(
            !battle.place(bodies[0], elsewhere),
            "the body was placed twice"
        );
        assert_eq!(battle.bodies().count(), 1);
    }

    #[test]
    fn a_body_moves_and_leaves_its_cell_behind() {
        let (mut battle, bodies) = fight();
        let from = first_open(&battle);
        let to = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != from)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        battle.place(bodies[0], from);
        assert!(battle.move_to(bodies[0], to));
        assert_eq!(battle.cell_of(bodies[0]), Some(to));
        assert_eq!(battle.occupant(from), None);
    }

    #[test]
    fn a_body_cannot_move_onto_somebody_else() {
        let (mut battle, bodies) = fight();
        let a = first_open(&battle);
        let b = battle
            .board
            .cells()
            .filter(|(c, k)| k.walkable() && *c != a)
            .map(|(c, _)| c)
            .next()
            .expect("a board with one cell");
        battle.place(bodies[0], a);
        battle.place(bodies[1], b);
        assert!(!battle.move_to(bodies[0], b));
        assert_eq!(battle.cell_of(bodies[0]), Some(a));
    }

    /// A body that dies or walks off the edge leaves, and takes its cell
    /// with it.
    #[test]
    fn a_removed_body_frees_its_cell() {
        let (mut battle, bodies) = fight();
        let cell = first_open(&battle);
        battle.place(bodies[0], cell);
        battle.remove(bodies[0]);
        assert_eq!(battle.cell_of(bodies[0]), None);
        assert_eq!(battle.occupant(cell), None);
        assert_eq!(battle.bodies().count(), 0);
    }

    /// A fixture with all three bodies placed and seated in the order they
    /// were spawned.
    fn seated() -> (TacticalBattle, Vec<Entity>) {
        let (mut battle, bodies) = fight();
        let cells: Vec<(i32, i32)> = battle
            .board
            .cells()
            .filter(|(_, k)| k.walkable())
            .map(|(c, _)| c)
            .take(3)
            .collect();
        for (body, cell) in bodies.iter().zip(&cells) {
            battle.place(*body, *cell);
        }
        battle.set_initiative(bodies.clone());
        (battle, bodies)
    }

    #[test]
    fn the_order_is_walked_in_order_and_wraps_into_a_new_round() {
        let (mut battle, bodies) = seated();
        assert_eq!(battle.actor(), Some(bodies[0]));
        assert_eq!(battle.round, 1);
        battle.end_turn();
        assert_eq!(battle.actor(), Some(bodies[1]));
        assert_eq!(battle.round, 1, "a hand-off is not a round");
        battle.end_turn();
        battle.end_turn();
        assert_eq!(battle.actor(), Some(bodies[0]));
        assert_eq!(battle.round, 2);
    }

    /// The turn economy: a fresh turn owes nothing and has acted on nothing.
    #[test]
    fn a_new_turn_starts_with_nothing_spent_and_nothing_done() {
        let (mut battle, _) = seated();
        battle.spend(3);
        battle.mark_acted();
        assert_eq!(battle.spent(), 3);
        assert!(battle.acted());
        battle.end_turn();
        assert_eq!(battle.spent(), 0);
        assert!(!battle.acted());
    }

    /// A body dying on somebody else's turn costs the order nothing: the
    /// cursor names a body, not a position, so everyone behind the gap keeps
    /// their place.
    #[test]
    fn a_body_removed_ahead_of_the_cursor_leaves_the_acting_body_acting() {
        let (mut battle, bodies) = seated();
        battle.end_turn();
        battle.end_turn();
        assert_eq!(battle.actor(), Some(bodies[2]));
        battle.remove(bodies[0]);
        assert_eq!(battle.actor(), Some(bodies[2]), "the cursor slipped");
        assert_eq!(battle.initiative(), &[bodies[1], bodies[2]]);
    }

    #[test]
    fn removing_the_acting_body_hands_the_turn_to_whoever_stood_behind_it() {
        let (mut battle, bodies) = seated();
        battle.end_turn();
        battle.spend(2);
        assert_eq!(battle.actor(), Some(bodies[1]));
        battle.remove(bodies[1]);
        assert_eq!(battle.actor(), Some(bodies[2]));
        assert_eq!(battle.spent(), 0, "the dead body's spend carried over");
    }

    /// The last body in the order leaving wraps the cursor rather than
    /// stranding it past the end.
    #[test]
    fn removing_the_last_body_in_the_order_wraps_the_cursor() {
        let (mut battle, bodies) = seated();
        battle.end_turn();
        battle.end_turn();
        battle.remove(bodies[2]);
        assert_eq!(battle.actor(), Some(bodies[0]));
        assert_eq!(battle.round, 2);
    }

    #[test]
    fn an_emptied_order_has_nobody_acting() {
        let (mut battle, bodies) = seated();
        for body in bodies {
            battle.remove(body);
        }
        assert_eq!(battle.actor(), None);
    }

    /// Placement order is what `bodies` reports, every time. Bevy's own
    /// query order is not stable, so anything that walks a fight's bodies
    /// walks this instead.
    #[test]
    fn bodies_come_back_in_the_order_they_were_placed() {
        let (mut battle, bodies) = fight();
        let cells: Vec<(i32, i32)> = battle
            .board
            .cells()
            .filter(|(_, k)| k.walkable())
            .map(|(c, _)| c)
            .take(3)
            .collect();
        for (body, cell) in bodies.iter().zip(&cells) {
            battle.place(*body, *cell);
        }
        let order: Vec<Entity> = battle.bodies().map(|(e, _)| e).collect();
        assert_eq!(order, bodies);
    }
}
