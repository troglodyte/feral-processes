//! Whose turn it is on a battle map, and what a turn is made of.
//!
//! One body acts at a time, in an order rolled once when the fight opens.
//! A turn is up to `Game::movement_allowance` cells of movement and then one
//! action, and the action ends the turn — so a body that swings first has
//! given up the rest of its walk, which is the whole of this model's
//! positioning pressure.

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::components::{Hostile, Position, Stats};
use crate::game::combat_teardown::FightVerdict;
use crate::resources::{GameClock, Party, ZoneLevel};
use crate::tactical::map::{BattleSpec, generate};
use crate::tactical::{TacticalBattle, deploy};
use crate::world::WorldMap;

/// What one press of a direction did.
///
/// Three answers rather than a `bool` because walking off the edge is not a
/// refused step and not an ordinary one either — see `Game::tactical_step`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    Moved,
    /// The body walked off the board and out of the fight.
    Departed,
    Refused,
}

impl Game {
    /// Opens a tactical fight around `pack`.
    ///
    /// `Game::begin_battle`'s counterpart, and deliberately not a caller of
    /// it: the two models disagree about what a fight *is*, and everything
    /// they share below the opening is shared by being one function already
    /// — `fight_rewards_mut`, `finish_hostile`, `finish_fight`.
    ///
    /// The board is generated from where the fight opened and the two sides
    /// are seated on the bearing the pack was actually found at, so walking
    /// into a pack from the side starts the party flanked.
    ///
    /// Nothing routes here yet. `start_battle` chooses the model in the
    /// phase that gives a tactical fight a screen to be fought on; until
    /// then this is reachable from tests alone.
    pub fn open_tactical_battle(&mut self, pack: Vec<Entity>) {
        let player = self.player_entity();
        let site = self
            .world
            .get::<Position>(player)
            .map(|p| (p.x, p.y))
            .unwrap_or((0, 0));
        let party: Vec<Entity> = std::iter::once(player)
            .chain(self.world.resource::<Party>().0.iter().copied())
            .filter(|&e| self.creature_alive(e))
            .collect();
        // Off the first member, which `gather_pack` guarantees is the body
        // the player actually bumped into — a centroid would answer a
        // different question, and the one being asked is which way the
        // player was facing trouble.
        let toward = pack
            .first()
            .and_then(|&e| self.world.get::<Position>(e))
            .map(|p| (p.x, p.y))
            .unwrap_or(site);
        let bearing = deploy::bearing(site, toward);

        let spec = BattleSpec {
            world_seed: self.world.resource::<WorldMap>().seed(),
            site,
            tick: self.world.resource::<GameClock>().tick,
            zone: self.world.resource::<ZoneLevel>().0,
            // The raw biome, not `environment_biome_at`: that gate answers
            // what the ground *does to you*, and a marsh fight should look
            // like a marsh in zone 1 too.
            biome: self
                .world
                .resource_mut::<WorldMap>()
                .tile(site.0, site.1)
                .biome,
            bodies: (party.len() + pack.len()) as u32,
        };
        let board = generate(spec);
        let plan = deploy::plan(&board, bearing, party.len() as u32, pack.len() as u32);

        let mut battle = TacticalBattle::open(spec, board);
        for (&body, &cell) in party.iter().zip(plan.party.iter()) {
            battle.place(body, cell);
        }
        for (&body, &cell) in pack.iter().zip(plan.wild.iter()) {
            battle.place(body, cell);
        }
        // Taken at the bell, before the first blow, for the reason
        // `BattleState::outmatched` gives: by the time a fight is won the
        // question is unanswerable.
        battle.outmatched =
            self.summed_power(pack.iter().copied()) > self.summed_power(party.iter().copied());

        let standing: Vec<Entity> = battle.bodies().map(|(entity, _)| entity).collect();
        battle.set_initiative(self.roll_turn_order(&standing));
        // Opened before the intercept line, so that line is the first thing
        // the battle pane shows — `begin_battle`'s ordering.
        self.world
            .resource_mut::<crate::resources::MessageLog>()
            .open_battle();
        self.world.insert_resource(battle);

        // After the resource is in place, deliberately: both telemetry
        // helpers read the fight back off it, so a record taken earlier
        // would describe a fight that does not exist yet — again
        // `begin_battle`'s ordering, and its reason.
        let fight = self.next_fight_id();
        self.record(|g| crate::telemetry::Record::FightStart {
            fight,
            seed: g.world.resource::<WorldMap>().seed() as u64,
            zone: g.world.resource::<ZoneLevel>().0,
            depth: 0,
            party: g.telemetry_party(),
            enemies: g.telemetry_enemy_groups(),
        });
        let line = self.intercept_line(pack.first().copied(), pack.len());
        self.log(line);
    }

    /// `standing` in descending initiative order, rolled once.
    ///
    /// Ties break on the order they were handed in — party before wild,
    /// each side in deployment order — because `sort_by_key` is stable, so
    /// a seeded run always produces the same order. `roll_initiative`'s
    /// rule, and the roll itself is that function's roll: both go through
    /// `Game::initiative_roll`.
    fn roll_turn_order(&mut self, standing: &[Entity]) -> Vec<Entity> {
        let mut rolled: Vec<(i32, Entity)> = standing
            .iter()
            .map(|&entity| (self.initiative_roll(entity), entity))
            .collect();
        rolled.sort_by_key(|&(initiative, _)| std::cmp::Reverse(initiative));
        rolled.into_iter().map(|(_, entity)| entity).collect()
    }

    /// Whose turn it is on the battle map, or `None` when there is no
    /// tactical fight open.
    pub fn tactical_actor(&self) -> Option<Entity> {
        self.world.get_resource::<TacticalBattle>()?.actor()
    }

    /// Moves the acting body one cell.
    ///
    /// **A step off the board is a departure, not a refusal.** Disengaging
    /// is how a body leaves a fight it does not want, and the only way to
    /// express it is to walk out — so the edge is not a wall, and the body
    /// leaves the board, the turn order and the fight together.
    ///
    /// Refused once the body has acted, because the action ends the turn.
    pub fn tactical_step(&mut self, dir: (i32, i32)) -> StepOutcome {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return StepOutcome::Refused;
        };
        let Some(actor) = battle.actor() else {
            return StepOutcome::Refused;
        };
        if battle.acted() {
            return StepOutcome::Refused;
        }
        let Some(from) = battle.cell_of(actor) else {
            return StepOutcome::Refused;
        };
        let to = (from.0 + dir.0, from.1 + dir.1);
        let spent = battle.spent();
        let cost = battle.board.cell(to.0, to.1).movement_cost();
        let inside = battle.board.in_bounds(to.0, to.1);

        if !inside {
            self.depart_tactical(actor);
            return StepOutcome::Departed;
        }
        let Some(cost) = cost else {
            return StepOutcome::Refused;
        };
        if spent + cost > self.movement_allowance(actor) {
            return StepOutcome::Refused;
        }
        let mut battle = self.world.resource_mut::<TacticalBattle>();
        if !battle.move_to(actor, to) {
            return StepOutcome::Refused;
        }
        battle.spend(cost);
        StepOutcome::Moved
    }

    /// Takes a body out of the fight without killing it: off the board, out
    /// of the order, and — for a hostile — out of the party's way.
    ///
    /// The body keeps its world `Position`, which never moved. A tactical
    /// fight writes none, so a body that walks out of one is standing
    /// exactly where the fight opened.
    fn depart_tactical(&mut self, body: Entity) {
        let line = if body == self.player_entity() {
            "You break off and slip away.".to_string()
        } else {
            format!("{} breaks off.", self.creature_label(body))
        };
        self.log(line);
        self.world.resource_mut::<TacticalBattle>().remove(body);
        self.settle_tactical(None);
    }

    /// The acting body swings at `target`.
    ///
    /// Melee and adjacent only. Shapes, ranges and the routines that use
    /// them are the next phase; this is the `Single` case they generalise,
    /// and it is what lets a fight be fought to its end.
    ///
    /// Reports whether the swing happened. The action ends the turn, so a
    /// swing that lands hands the turn on — unless it ended the fight.
    pub fn tactical_attack(&mut self, target: Entity) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let Some(actor) = battle.actor() else {
            return false;
        };
        if battle.acted() {
            return false;
        }
        let (Some(from), Some(at)) = (battle.cell_of(actor), battle.cell_of(target)) else {
            return false;
        };
        if actor == target || (from.0 - at.0).abs() > 1 || (from.1 - at.1).abs() > 1 {
            return false;
        }

        let (move_name, natural) = self.swing_move(actor);
        let range = self.attack_range(actor, natural);
        let outcome =
            self.resolve_and_apply_attack(actor, target, crate::battle::Swing::plain(range));
        let line = self.party_swing_line(actor, &move_name, outcome);
        self.log_swing(crate::resources::MessageKind::PartyDamage, outcome, line);
        self.world.resource_mut::<TacticalBattle>().mark_acted();

        // Every body that fell, not just the target: a fumble's riposte can
        // put the swinger down, and a body left standing on the board at
        // zero HP would keep its place in the order.
        self.reap_tactical_dead(target);
        if self.world.get_resource::<TacticalBattle>().is_some() {
            self.world.resource_mut::<TacticalBattle>().end_turn();
        }
        true
    }

    /// Ends the acting body's turn without spending its action.
    pub fn tactical_end_turn(&mut self) {
        if self.world.get_resource::<TacticalBattle>().is_some() {
            self.world.resource_mut::<TacticalBattle>().end_turn();
        }
    }

    /// Clears the dead off the board and pays for them, then closes the
    /// fight if that ended it.
    ///
    /// `wild` names whoever the swing was aimed at, which is what
    /// `finish_fight` wants cleared of combat-only effects even when the
    /// blow that ended the fight was struck at somebody else.
    fn reap_tactical_dead(&mut self, wild: Entity) {
        let player = self.player_entity();
        let fallen: Vec<Entity> = self
            .world
            .resource::<TacticalBattle>()
            .bodies()
            .map(|(entity, _)| entity)
            .filter(|&e| !self.creature_alive(e))
            .collect();
        for body in fallen {
            self.world.resource_mut::<TacticalBattle>().remove(body);
            // A fallen companion is reaped at teardown, not here — the same
            // deferral the abstract model makes, and `bench_or_dissolve` is
            // what a Forgiving death owes it.
            if self.world.get::<Hostile>(body).is_some() {
                self.finish_hostile(body, player);
            }
        }
        self.settle_tactical(Some(wild));
    }

    /// Ends the fight if it is over, and reports whether it did.
    ///
    /// Three ways out, and only one of them is a win: the board is clear of
    /// hostiles, the player is down, or the player has walked off it.
    ///
    /// **A pack that all broke off counts as won** — the field is the
    /// party's and there is nothing left to fight, which is the same answer
    /// an emptied roster gives. **The player walking out is the jack-out**,
    /// and it is theirs alone to make: a companion can break off and leave
    /// the party fighting on, but the player is the one holding the fight
    /// open, so their leaving closes it exactly as `battle_flee` does.
    fn settle_tactical(&mut self, wild: Option<Entity>) -> bool {
        let player = self.player_entity();
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let hostiles = battle
            .bodies()
            .filter(|&(entity, _)| self.world.get::<Hostile>(entity).is_some())
            .count();
        let gone = battle.cell_of(player).is_none();
        let down = self
            .world
            .get::<Stats>(player)
            .is_none_or(|stats| stats.hp <= 0);
        if hostiles > 0 && !down && !gone {
            return false;
        }
        let verdict = FightVerdict {
            won: hostiles == 0,
            rounds: battle.round,
            outmatched: battle.outmatched,
            // A lair is roused in the Stack and the Stack stays abstract, so
            // a tactical fight never has one. Stated rather than omitted:
            // this is the field a fourth in-scope encounter kind would have
            // to answer.
            lair: None,
        };
        self.finish_fight(player, wild, verdict);
        true
    }

    /// Closes a tactical fight the way it would close itself, for a caller
    /// that has decided it is over for a reason of its own.
    pub fn end_tactical_battle(&mut self, wild: Option<Entity>) {
        self.settle_tactical(wild);
    }
}
