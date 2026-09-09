//! The tactical battle model: a disposable grid a surface fight is fought
//! on.
//!
//! Opt-in, off by default, and the second of the game's two combat models —
//! see
//! `docs/superpowers/specs/2026-09-09-tactical-surface-battles-design.md`.
//! Nothing outside this module's own tests calls into it yet; the router
//! that chooses between the two models is a later phase.
//!
//! **A battle coordinate lives here and nowhere else.** No world `Position`
//! is ever written for a body standing on a battle map, the same way the
//! Stack keeps its coordinates in `resources::Locale` and base space keeps
//! its own in `Locale::Base`. This module does not import `Position`.

pub mod deploy;
pub mod map;
pub mod reach;

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
}

impl TacticalBattle {
    pub fn open(spec: BattleSpec, board: Board) -> Self {
        TacticalBattle {
            spec,
            board,
            bodies: Vec::new(),
            rewards: BattleRewards::default(),
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

    /// Takes a body off the board — killed, or walked off the edge.
    pub fn remove(&mut self, body: Entity) {
        self.bodies.retain(|(e, _)| *e != body);
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
