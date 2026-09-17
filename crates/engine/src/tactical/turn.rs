//! Whose turn it is on a battle map, and what a turn is made of.
//!
//! One body acts at a time, in an order rolled once when the fight opens.
//! A turn is up to `Game::movement_allowance` cells of movement and then one
//! action, and the action ends the turn — so a body that swings first has
//! given up the rest of its walk, which is the whole of this model's
//! positioning pressure.

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::abilities::{self, AbilityDef, AbilityEffect, AbilityShape, TamperKind};
use crate::components::AbilityCooldowns;
use crate::components::{Hostile, Player, Squad, Stats};
use crate::game::combat_teardown::FightVerdict;
use crate::resources::{GameClock, Party, ZoneLevel};
use crate::tactical::map::{BattleSpec, generate};
use crate::tactical::{TacticalBattle, deploy, opposes, reach};
use crate::tuning::{FORMATIONS, TACTICAL_MELEE_RANGE};
use crate::world::WorldMap;

/// What one press of a direction did.
///
/// Four answers rather than a `bool` because three of them are not ordinary
/// steps: walking off the edge leaves the fight, walking into something
/// hostile swings at it, and neither is a refusal — see
/// `Game::tactical_step`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum StepOutcome {
    Moved,
    /// The body walked off the board and out of the fight.
    Departed,
    /// The cell held something hostile, so the step was spent as a swing.
    Struck,
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
    /// `Game::start_battle` routes here for a pack `fights_tactically`
    /// takes.
    pub fn open_tactical_battle(&mut self, pack: Vec<Entity>) {
        let site = self.tile_of(self.player_entity()).unwrap_or((0, 0));
        // Off the first member, which `gather_pack` guarantees is the body
        // the player actually bumped into — a centroid would answer a
        // different question, and the one being asked is which way the
        // player was facing trouble.
        let toward = pack.first().and_then(|&e| self.tile_of(e)).unwrap_or(site);
        let bearing = deploy::bearing(site, toward);
        self.open_tactical_battle_at(pack, bearing);
    }

    /// `open_tactical_battle` with the bearing supplied rather than read off
    /// the pack's own tile.
    ///
    /// `tactical_ai_turn_at`'s precedent, and its reason: a staged fight
    /// spawns its opponents around the player, so the tile the derivation
    /// reads answers nothing, and being flanked is exactly what an
    /// instrument wants to be able to ask about. The one caller past the
    /// door above is `arena::stage`.
    pub(crate) fn open_tactical_battle_at(&mut self, pack: Vec<Entity>, bearing: (i32, i32)) {
        let player = self.player_entity();
        let site = self.tile_of(player).unwrap_or((0, 0));
        let party: Vec<Entity> = std::iter::once(player)
            .chain(self.world.resource::<Party>().0.iter().copied())
            .filter(|&e| self.creature_alive(e))
            .collect();

        // Folded here, after `bearing` is already in hand — squads::plan's
        // own rule. `Game::gather_pack` answers a pack of one for an anchor
        // with no `Position`, and `open_tactical_battle`'s own bearing is
        // derived from `pack[0]`'s tile: folding one call earlier would let
        // a squad reach either as its anchor, both silent-degradation sites
        // `tactical::squads` warns about. `arena::stage` passes its bearing
        // explicitly and is safe by construction already.
        let pieces = crate::tactical::squads::plan(&pack, &self.world);
        let mut wild: Vec<Entity> = Vec::new();
        let mut wild_footprints: Vec<u8> = Vec::new();
        for piece in pieces {
            match piece {
                crate::tactical::squads::Piece::Single(entity) => {
                    wild.push(entity);
                    wild_footprints.push(1);
                }
                crate::tactical::squads::Piece::Squad { members, formation } => {
                    let squad = self.spawn_squad(&members, formation);
                    wild_footprints.push(FORMATIONS[formation].footprint);
                    wild.push(squad);
                }
            }
        }

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
            bodies: (party.len() + wild.len()) as u32,
        };
        let board = generate(spec);
        let plan = deploy::plan(&board, bearing, party.len() as u32, &wild_footprints);

        let mut battle = TacticalBattle::open(spec, board);
        for (&body, &cell) in party.iter().zip(plan.party.iter()) {
            battle.place(body, cell);
        }
        for ((&body, &cell), &footprint) in wild.iter().zip(plan.wild.iter()).zip(&wild_footprints)
        {
            battle.place(body, cell);
            if footprint > 1 {
                let actions = self.actions_per_turn(body);
                battle.set_shape(body, footprint, actions);
            }
        }
        // Taken at the bell, before the first blow, for the reason
        // `BattleState::outmatched` gives: by the time a fight is won the
        // question is unanswerable.
        battle.outmatched =
            self.summed_power(wild.iter().copied()) > self.summed_power(party.iter().copied());

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
        let line = self.intercept_line(wild.first().copied(), wild.len());
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

    /// How many actions `body` gets on its turn — a formation's `actions`
    /// for a `Squad`, one otherwise. The player's party always gets one,
    /// since a companion is never a formation row.
    ///
    /// `footprint_of`'s door on the `Game` side: `TacticalBattle` holds no
    /// `World` to look a `Squad` up with itself, so `open_tactical_battle_at`
    /// calls this once, at seat time, and hands the answer to
    /// `TacticalBattle::set_shape` for `begin_turn` to read back every
    /// round.
    pub fn actions_per_turn(&self, body: Entity) -> u8 {
        self.world
            .get::<Squad>(body)
            .and_then(|s| FORMATIONS.get(s.formation))
            .map_or(1, |f| f.actions)
    }

    /// Moves the acting body one cell.
    ///
    /// **A step off the board is a departure, not a refusal.** Disengaging
    /// is how a body leaves a fight it does not want, and the only way to
    /// express it is to walk out — so the edge is not a wall, and the body
    /// leaves the board, the turn order and the fight together.
    ///
    /// **Walking into something hostile is a swing.** `move_player`'s ladder
    /// one space over: an occupied cell answered `Refused`, so an arrow key
    /// pressed at the body a whole turn had been spent closing on did
    /// nothing at all. The swing is `Game::tactical_attack` and not a second
    /// spelling of one, so the range, the sight line, a reach weapon's
    /// sweep, the cloak refusal and the hand-on all come from there — which
    /// also means a bump **ends the turn**, because the action is what a
    /// swing costs.
    ///
    /// Two gates on it, and neither is a new predicate. `Hostile`, because
    /// friendly fire is legal through the aim cursor but an arrow key is not
    /// an aim, and a bump that hit whatever was in the way would make
    /// crossing your own line a coin flip. And `tactical_awaits_input`,
    /// which is false for exactly the bodies the AI's beat loop drives —
    /// this is the door that loop's walk goes through, so without it a
    /// hostile could spend its action part-way along a path it planned.
    ///
    /// Refused once the body has spent every action it has this turn.
    pub fn tactical_step(&mut self, dir: (i32, i32)) -> StepOutcome {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return StepOutcome::Refused;
        };
        let Some(actor) = battle.actor() else {
            return StepOutcome::Refused;
        };
        if battle.actions_left() == 0 {
            return StepOutcome::Refused;
        }
        let Some(from) = battle.cell_of(actor) else {
            return StepOutcome::Refused;
        };
        let to = (from.0 + dir.0, from.1 + dir.1);
        let spent = battle.spent();
        let cost = battle.board.cell(to.0, to.1).movement_cost();
        // A body departs if any footprint cell anchored at `to` leaves the
        // board — one cell today, without a `Squad`, so this is the same
        // check `in_bounds(to)` alone made.
        let footprint = battle.footprint_cells(actor, to);
        let inside = footprint.iter().all(|&(x, y)| battle.board.in_bounds(x, y));

        if !inside {
            self.depart_tactical(actor);
            return StepOutcome::Departed;
        }
        // Above the movement gates below it, deliberately: a swing is priced
        // in the action and not in steps, so a body that has walked its whole
        // allowance can still finish the approach it spent it on.
        let bumped = battle.occupant(to).filter(|&target| {
            self.world.get::<Hostile>(target).is_some() && self.tactical_awaits_input()
        });
        if let Some(target) = bumped {
            return match self.tactical_attack(target) {
                true => StepOutcome::Struck,
                // Out of reach, behind cover, or cloaked —
                // `tactical_attack`'s own refusals, and the cell is occupied
                // either way, so there is no step to fall through to.
                false => StepOutcome::Refused,
            };
        }
        let Some(cost) = cost else {
            return StepOutcome::Refused;
        };
        if spent + cost > self.movement_allowance(actor) {
            return StepOutcome::Refused;
        }
        // **Before the move is written**, so a mover that is put down or
        // stalled on its way out dies on the cell it tried to leave rather
        // than on the one it was reaching for. Below every refusal above it:
        // a step that was never going to happen provokes nobody.
        let reactors = self.step_reactors(actor, from, to);
        if !reactors.is_empty() && !self.provoke(actor, reactors) {
            return StepOutcome::Refused;
        }
        // The fight can close under a fatal reaction — a lone hostile put the
        // player down, or the player's last companion went with them.
        let Some(mut battle) = self.world.get_resource_mut::<TacticalBattle>() else {
            return StepOutcome::Refused;
        };
        if !battle.move_to(actor, to) {
            return StepOutcome::Refused;
        }
        battle.spend(cost);
        StepOutcome::Moved
    }

    /// Everyone who would react to `mover` stepping from `from` to `to`.
    ///
    /// **Leaving reach is the trigger, and a step that stays adjacent is
    /// not one.** The cell it starts on has to be inside the reactor's melee
    /// reach and the cell it ends on outside — so closing on a body, or
    /// circling it, is free, and only disengaging is paid for.
    pub(crate) fn step_reactors(
        &self,
        mover: Entity,
        from: (i32, i32),
        to: (i32, i32),
    ) -> Vec<Entity> {
        self.reactors(mover, from, |cells| {
            reach::gap(cells, &[from]) <= TACTICAL_MELEE_RANGE
                && reach::gap(cells, &[to]) > TACTICAL_MELEE_RANGE
        })
    }

    /// Everyone who would react to `mover` invoking a routine where it
    /// stands — every enemy already inside melee reach of it.
    pub(crate) fn invoke_reactors(&self, mover: Entity) -> Vec<Entity> {
        let Some(from) = self
            .world
            .get_resource::<TacticalBattle>()
            .and_then(|battle| battle.cell_of(mover))
        else {
            return Vec::new();
        };
        self.reactors(mover, from, |cells| {
            reach::gap(cells, &[from]) <= TACTICAL_MELEE_RANGE
        })
    }

    /// The half both triggers share: who is *able* to react to `mover` at
    /// all, in initiative order, out of the bodies whose footprint `trigger`
    /// accepts.
    ///
    /// Four conditions, and each is somebody else's rule rather than a new
    /// one. An enemy of the mover, read through `acts_for_hostiles` so an
    /// injected body reacts for the side it now believes it is on. Its
    /// reaction unspent. Line of sight to the mover, `tactical_attack`'s own
    /// gate — any cell of the reactor's footprint to `cell`, one cell today
    /// without a `Squad`. And the mover nameable at all — **a cloaked body
    /// provokes nobody**, which is not a special case here but the same
    /// filter that sits at the five doors that name a body.
    ///
    /// **Melee reach and not `Game::swing_range`.** A reaction from across
    /// the board is overwatch, which is a different feature; this is the one
    /// place in tactical code that reads `TACTICAL_MELEE_RANGE` rather than
    /// asking how far a body swings, and it is deliberate.
    fn reactors(
        &self,
        mover: Entity,
        cell: (i32, i32),
        trigger: impl Fn(&[(i32, i32)]) -> bool,
    ) -> Vec<Entity> {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return Vec::new();
        };
        if self.is_cloaked(mover) {
            return Vec::new();
        }
        let mover_side = self.acts_for_hostiles(mover);
        battle
            .initiative()
            .iter()
            .copied()
            .filter(|&body| body != mover)
            .filter(|&body| self.acts_for_hostiles(body) != mover_side)
            .filter(|&body| !battle.reaction_spent(body))
            .filter(|&body| self.creature_alive(body))
            .filter(|&body| {
                let cells = battle.cells_of(body);
                !cells.is_empty()
                    && trigger(&cells)
                    && cells
                        .iter()
                        .any(|&at| reach::line_of_sight(&battle.board, at, cell))
            })
            .collect()
    }

    /// Runs the reactions `reactors` take against `mover`, and reports
    /// whether `mover` may still do the thing that provoked them.
    ///
    /// **One door for both triggers.** A step and an invocation differ in
    /// who they provoke and in what a stopped mover means, never in what a
    /// reaction *is* — so the swing, the charge, the order, the streak and
    /// the stopping rule are written once.
    ///
    /// The swing is free (`Swing::reaction`), so it cannot fumble: a
    /// fumbled reaction could riposte, and a riposte is another swing that
    /// could provoke again.
    ///
    /// **The loop stops the moment the mover is down or stalled.** A body
    /// that has been stunned is not walking anywhere, and swinging at a
    /// corpse would charge the rest of the pack a reaction for nothing.
    pub(crate) fn provoke(&mut self, mover: Entity, reactors: Vec<Entity>) -> bool {
        for reactor in reactors {
            if !self.creature_alive(mover) || self.is_stunned(mover) {
                return false;
            }
            // Re-read each time round: an earlier reaction can kill a
            // reactor through a shared effect, and the fight can close
            // under the whole loop.
            let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
                return false;
            };
            let (Some(at), Some(to)) = (battle.cell_of(reactor), battle.cell_of(mover)) else {
                continue;
            };
            if !self.creature_alive(reactor) {
                continue;
            }
            self.world
                .resource_mut::<TacticalBattle>()
                .spend_reaction(reactor);
            let color = self
                .world
                .get::<crate::components::Glyph>(reactor)
                .map(|g| g.color)
                .unwrap_or(crate::components::GlyphColor::White);
            // `tactical_attack`'s rule: pushed before the blow lands, so a
            // body that dies to it still gets its streak drawn.
            self.world
                .resource_mut::<crate::resources::BoltQueue>()
                .push(crate::resources::BoltCue {
                    from: at,
                    to,
                    color,
                });
            let (move_name, _) = self.swing_move_at(reactor, Some(reach::distance(at, to)));
            let range = self.natural_range_of(reactor);
            let outcome = self.resolve_and_apply_attack(
                reactor,
                mover,
                crate::battle::Swing::reaction(range),
            );
            // **Through the model's own line builder**, so a reaction that
            // misses reads as a miss. A line written here would be a second
            // vocabulary for the same four outcomes, and the one that drifts
            // is the one that says a refused swing landed. The interrupt is
            // named in the move rather than in a lead of its own, because
            // both of that builder's forms — the player's and everybody
            // else's — put the move where the parenthetical still reads.
            let line = self.party_swing_line(reactor, &format!("{move_name} (interrupt)"), outcome);
            // Whose news the line is, which is what a kind means here — a
            // reaction is taken by both sides, so it cannot be one kind.
            let kind = match self.world.get::<Hostile>(reactor).is_some() {
                true => crate::resources::MessageKind::EnemyAttack,
                false => crate::resources::MessageKind::PartyDamage,
            };
            self.log_swing(kind, outcome, line);
        }
        // A reaction can put the mover down, and a body left standing on the
        // board at zero Integrity would keep its place in the order.
        self.reap_tactical_dead(Some(mover));
        // **Still on the board, not merely still alive.** A fatal reaction
        // closes the fight from inside the reap, and a Forgiving player is
        // rebooted by the tick that follows — so `creature_alive` alone
        // answers `true` for a body that has no cell, and the caller walks
        // on into a `TacticalBattle` that is no longer there.
        self.world
            .get_resource::<TacticalBattle>()
            .is_some_and(|battle| battle.cell_of(mover).is_some())
            && self.creature_alive(mover)
            && !self.is_stunned(mover)
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
        // A departing squad is disbanded here rather than caught by
        // `settle_tactical`'s own sweep: `remove` above has already taken it
        // out of `TacticalBattle::bodies`, so that sweep — which reads
        // `bodies()` for whatever squad the *player's* own departure or
        // defeat left standing — would never see it.
        if self.world.get::<Squad>(body).is_some() {
            self.disband_squad(body);
        }
        self.settle_tactical(None);
    }

    /// The acting body swings at `target`.
    ///
    /// Reaches as far as `Game::swing_range` says the swinger does, and
    /// needs line of sight to get there.
    ///
    /// **A cloaked target is refused**, alongside the range check and by
    /// the same `false` — one of the five doors that name a body. There is no
    /// never-empty rule here: this is a pick of one body rather than a pool
    /// to draw from, and a body that cannot be aimed at is exactly what the
    /// cloak is. An area routine covering its cell still lands
    /// (`reach::recipients` never learns the word), and a cloaked body is
    /// still a wall in `reach::movement_field`, so the cell it stands on is
    /// the tell.
    ///
    /// Reports whether the swing happened. It spends one action, which hands
    /// the turn on once none are left — unless it ended the fight.
    pub fn tactical_attack(&mut self, target: Entity) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let Some(actor) = battle.actor() else {
            return false;
        };
        if battle.actions_left() == 0 {
            return false;
        }
        let (Some(from), Some(at)) = (battle.cell_of(actor), battle.cell_of(target)) else {
            return false;
        };
        // `gap` rather than `distance`, and both bodies' whole footprints
        // rather than their anchors alone — one cell each without a
        // `Squad`, so an ordinary swing reads exactly the same range test
        // it always did.
        let actor_cells = battle.cells_of(actor);
        let target_cells = battle.cells_of(target);
        if actor == target || reach::gap(&actor_cells, &target_cells) > self.swing_range(actor) {
            return false;
        }
        // **Unconditional, with no melee branch.** `line_of_sight` excludes
        // its endpoints, so for neighbours its loop is empty and this is
        // already a no-op — one rule, and no second place
        // `TACTICAL_MELEE_RANGE` has to be restated. Cover earns a second
        // job for free.
        //
        // **Any cell of one footprint to any cell of the other**, rather
        // than anchor-to-anchor — the same single pair today.
        {
            let battle = self.world.resource::<TacticalBattle>();
            let sees = actor_cells.iter().any(|&a| {
                target_cells
                    .iter()
                    .any(|&t| reach::line_of_sight(&battle.board, a, t))
            });
            if !sees {
                return false;
            }
        }
        if self.is_cloaked(target) {
            return false;
        }

        let round_before = self.world.resource::<TacticalBattle>().round;
        let (move_name, natural) = self.swing_move_at(actor, Some(reach::distance(from, at)));
        let range = self.attack_range(actor, natural);

        // A reach weapon sweeps its shape, converted by this model's own
        // converter: `reach::recipients` reads the aim as a *destination*
        // for a blast and a *bearing* for a line or a cone, which is its
        // existing rule and needs no special handling here. The aim is the
        // cell of the body already being swung at at whatever distance, and
        // the range gate above is untouched — a reach is breadth and the
        // weapon's own `range` is distance.
        //
        // One swing a turn here, so there is no once-per-turn problem to
        // solve: `party_member_attacks`' `Option::take` has no counterpart.
        let wide = self.swing_reach(actor);
        let mut bodies = vec![target];
        if let Some(spec) = wide {
            self.arm_reach_charge(actor, spec.recharge);
            let swept = {
                let battle = self.world.resource::<TacticalBattle>();
                reach::recipients(battle, actor, at, spec.tactical_shape())
            };
            // **Nothing here reads `Hostile`.** Friendly fire is full and is
            // the whole reason a shape is worth aiming — a companion
            // standing beside the target is caught, and a side filter is
            // the trap this seam names.
            //
            // The swinger itself is dropped, and that is not a side filter:
            // `tactical_attack` already refuses `actor == target` at its own
            // door, so a body cannot swing at itself and its own cleave
            // cannot land on it either. Without it every wide swing would
            // hit the wielder, since `Radius { 1 }` around an adjacent cell
            // always covers the cell swung from.
            bodies.extend(
                swept
                    .into_iter()
                    .filter(|&body| body != target && body != actor),
            );
        }

        // The swinger's own hue, read once: a fumble's Recoil rung can kill
        // the body that swung, and a lookup inside the loop would then have
        // nothing to ask.
        let bolt_color = self
            .world
            .get::<crate::components::Glyph>(actor)
            .map(|g| g.color)
            .unwrap_or(crate::components::GlyphColor::White);
        for (index, body) in bodies.into_iter().enumerate() {
            // `party_member_swing`'s guard, and its reason: a fumble's
            // Recoil or Opening rung damages the swinger, so it really can
            // die on its own first body. The opening swing is unguarded, or
            // the narrow path stops behaving as it always has.
            if index > 0 && !self.creature_alive(actor) {
                break;
            }
            // **Before the blow lands**, so a body that dies to it still gets
            // its streak drawn — the cue names its cell, and `remove` takes
            // that cell with it.
            if let Some(to) = self.world.resource::<TacticalBattle>().cell_of(body) {
                self.world
                    .resource_mut::<crate::resources::BoltQueue>()
                    .push(crate::resources::BoltCue {
                        from,
                        to,
                        color: bolt_color,
                    });
            }
            let outcome =
                self.resolve_and_apply_attack(actor, body, crate::battle::Swing::plain(range));
            let line = self.party_swing_line(actor, &move_name, outcome);
            self.log_swing(crate::resources::MessageKind::PartyDamage, outcome, line);
        }
        self.world.resource_mut::<TacticalBattle>().spend_action();

        // Every body that fell, not just the target: a fumble's riposte can
        // put the swinger down, and a body left standing on the board at
        // zero HP would keep its place in the order.
        self.reap_tactical_dead(Some(target));
        self.hand_on_turn(actor, round_before);
        true
    }

    /// The acting body braces: `DEFEND_MITIGATION_BONUS` percentage points
    /// of mitigation for the rest of the round.
    ///
    /// `tactical_attack`'s third sibling, and the group model's own Defend
    /// rather than a second spelling of it — `Game::begin_defend` holds the
    /// number and the line, so a brace on a board and a brace in front of a
    /// group cannot come to disagree about either. The mitigation lands for
    /// free from there: `effective_mitigation` is read inside
    /// `Game::apply_damage`, the one door damage comes through.
    ///
    /// **Only the mitigation crosses over, not the aggro.**
    /// `DEFEND_AGGRO_WEIGHT` is a weight on an aggro *slot*, and a battle
    /// map has none — a hostile takes the wounded body in reach
    /// (`swing_at_best_neighbour`), so bracing is a survival play here and
    /// not a tank one.
    ///
    /// **The brace is worth what the order says it is worth.** It is armed
    /// for one round and `hand_on_turn`'s wrap is what ages it, so a body
    /// on the last rung braces against nobody: the wrap fires the moment it
    /// hands the turn on. Accepted rather than overlooked — the turn strip
    /// is on screen, so where a body sits in the order is something the
    /// player can read before spending the turn.
    ///
    /// Three refusals, all before anything is spent: no fight, nobody
    /// acting, and a body with no actions left to spend. **No `is_stunned`
    /// gate**, unlike `battle_resolve_round`'s Defend loop — nothing in this
    /// model reads stun at all and a stunned body already takes a whole turn
    /// on a board, so gating the brace alone would make bracing the one
    /// thing a stunned body could not do.
    ///
    /// Reports whether the brace took. It spends one action, which hands
    /// the turn on once none are left.
    pub fn tactical_defend(&mut self) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let Some(actor) = battle.actor() else {
            return false;
        };
        if battle.actions_left() == 0 {
            return false;
        }

        let round_before = battle.round;
        self.begin_defend(actor);
        self.world.resource_mut::<TacticalBattle>().spend_action();
        // No reap: bracing damages nobody, and the round upkeep
        // `hand_on_turn` may spend brings its own.
        self.hand_on_turn(actor, round_before);
        true
    }

    /// The acting body runs the routine at `index` in its own
    /// `Game::actor_abilities`, aimed at `aim`.
    ///
    /// `tactical_attack`'s counterpart, and the same index the group model's
    /// `BattleAction::Special` carries, so a screen offering the two models
    /// the same rows offers the same numbers.
    ///
    /// **Every refusal lands before anything is spent** — the Power, the
    /// cooldown and the turn alike — which is `commit_caravan_basket`'s rule
    /// and the reason the price is charged only once the aim has been
    /// checked. Six of them are the door's own: no fight, nobody acting, the
    /// body has already acted, no such routine, a routine that is not run in
    /// a fight at all (a passive, or a field-only effect —
    /// `battle_special_options`' own two exclusions), whatever
    /// `ability_unavailable` says, and an aim outside the routine's range or
    /// out of sight. The rest are an effect's own, each with its reason at
    /// the site: a capture aimed at anything but a hostile, a `Summon` with
    /// no room on the board, a `Single` tamper aimed at the player, and a
    /// Hallucination that would seat no decoy or cover nobody.
    ///
    /// Reports whether the routine ran. The action ends the turn, so one
    /// that runs hands the turn on — unless it ended the fight.
    pub fn tactical_use_routine(&mut self, index: usize, aim: (i32, i32)) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let Some(actor) = battle.actor() else {
            return false;
        };
        if battle.actions_left() == 0 {
            return false;
        }
        let Some(from) = battle.cell_of(actor) else {
            return false;
        };
        let Some(ability) = self.actor_abilities(actor).into_iter().nth(index) else {
            return false;
        };
        if ability.effect.field_only() || ability.is_passive() {
            return false;
        }
        if self.ability_unavailable(actor, &ability).is_some() {
            return false;
        }
        if !reach::in_range(from, aim, ability.tactical_range()) {
            return false;
        }
        // The other half of "may this be aimed there", and a refusal rather
        // than a fizzle: an unseen aim that merely covered nobody would still
        // spend the Power, the cooldown and the turn. `reach::aim_in_sight` is
        // which shapes read terrain at their aim — a `Line` and a `Cone` are
        // aimed as a direction and truncate themselves at cover, so this
        // refuses neither.
        if !reach::aim_in_sight(
            &self.world.resource::<TacticalBattle>().board,
            from,
            aim,
            ability.tactical_shape(),
        ) {
            return false;
        }
        // A capture is aimed at something hostile, and the refusal lands
        // here with the other five rather than inside the effect: aimed at
        // one of your own it would spend the catalyst and the turn, and on
        // a good roll hand the companion back through `roster_parts` — a
        // fresh `ProgramId`, level one, no memories, and kill XP paid to
        // the player for it. Aimed at empty ground it would spend both for
        // nothing at all.
        if matches!(ability.effect, AbilityEffect::Decompile)
            && self
                .world
                .resource::<TacticalBattle>()
                .occupant(aim)
                .is_none_or(|body| self.world.get::<Hostile>(body).is_none())
        {
            return false;
        }
        // The seventh refusal, and it lands here with the rest for the same
        // reason: a board with no free cell beside the invoker has nowhere
        // to put a body, and spending the Power, the cooldown and the turn
        // to seat nobody is exactly the wasted round the other six refuse.
        if matches!(ability.effect, AbilityEffect::Summon { .. }) && !self.board_has_room(actor) {
            return false;
        }
        // The eighth refusal, and a tamper's own: it names the other side,
        // and the player is never on it. A shaped tamper already drops the
        // player in `Game::apply_tamper`; a `Single` aim has nothing else to
        // drop it there, so it needs the refusal here or it would spend the
        // Power, the cooldown and the turn tampering with nobody.
        if matches!(ability.effect, AbilityEffect::Tamper { .. })
            && ability.tactical_shape() == AbilityShape::Single
            && self
                .world
                .resource::<TacticalBattle>()
                .occupant(aim)
                .is_some_and(|body| self.world.get::<Player>(body).is_some())
        {
            return false;
        }
        // The ninth and tenth, and a Hallucination's own — `board_has_room`'s
        // argument twice more. A radius with no free cell in it seats no
        // decoy at all, and a radius covering nobody on the other side seats
        // decoys that `settle_decoys` drops inside the very hand-on that
        // ended the turn, since no living body can see them. Either way 14
        // Power, a five-round cooldown and the turn buy nothing and say
        // nothing, which is exactly what the eight above refuse.
        //
        // Both questions are asked of the derivations the effect itself then
        // runs, rather than of a second copy of them.
        if let AbilityEffect::Tamper {
            kind: TamperKind::Hallucinating { decoys },
            ..
        } = &ability.effect
        {
            if self
                .hallucination_cells(actor, &ability, aim, *decoys)
                .is_empty()
            {
                return false;
            }
            let owner_hostile = self.world.get::<Hostile>(actor).is_some();
            if !self
                .tamper_recipients(actor, &ability, aim)
                .into_iter()
                .any(|body| opposes(owner_hostile, self.world.get::<Hostile>(body).is_some()))
            {
                return false;
            }
        }
        self.run_tactical_routine(actor, &ability, aim, 0);
        true
    }

    /// Whether a free walkable cell can still be found for a body seated
    /// beside `invoker` — `seat_summon_on_board`'s question, asked ahead of
    /// the spend rather than discovered inside it.
    fn board_has_room(&self, invoker: Entity) -> bool {
        let battle = self.world.resource::<TacticalBattle>();
        let Some(from) = battle.cell_of(invoker) else {
            return false;
        };
        let taken: std::collections::BTreeSet<(i32, i32)> =
            battle.bodies().map(|(_, cell)| cell).collect();
        crate::tactical::deploy::nearest_free(&battle.board, &taken, from, 1).is_some()
    }

    /// Places a forked body beside `invoker` and splices it into the turn
    /// order behind the cursor. Reports whether there was room.
    ///
    /// `deploy::nearest_free` is already exactly the breadth-first search
    /// for this — the deployment ranks find their cells with it — so the
    /// only thing this adds is building `taken` from the bodies already on
    /// the board rather than from a rank being laid out.
    ///
    /// Sidedness needs **nothing**: `tactical_sides` is relative to the
    /// actor, which is why `tactical_drive_turn` works at all. A body with
    /// no `Hostile` is on the player's side by omission, exactly as a
    /// companion is.
    pub(crate) fn seat_summon_on_board(&mut self, invoker: Entity, body: Entity) -> bool {
        let Some(at) = ({
            let battle = self.world.resource::<TacticalBattle>();
            battle.cell_of(invoker).and_then(|from| {
                let taken: std::collections::BTreeSet<(i32, i32)> =
                    battle.bodies().map(|(_, cell)| cell).collect();
                crate::tactical::deploy::nearest_free(&battle.board, &taken, from, 1)
            })
        }) else {
            return false;
        };
        let mut battle = self.world.resource_mut::<TacticalBattle>();
        if !battle.place(body, at) {
            return false;
        }
        battle.insert_after_cursor(body);
        true
    }

    /// Resolves a routine that has already been decided on and cleared: the
    /// price, the effect, the reap and the turn.
    ///
    /// **The effect is shared; the refusals are not** — `Game::take_routine`'s
    /// split, for the same reason. A hostile holds no `PowerReserve` by
    /// design, so `ability_unavailable` refuses it every priced routine there
    /// is, and every routine that can be *run* is priced; routing the enemy
    /// AI through the player's door would have given it a routine arm that
    /// compiles, tests green and can never fire. So the player's door keeps
    /// its six refusals and `tactical/ai.rs` brings `wild_routine_ready`'s
    /// gate instead, and the two meet here.
    ///
    /// `cooldown_floor` is the whole of the remaining difference, and it is
    /// `abilities::armed_cooldown`'s own parameter rather than a second
    /// spelling of it: the player's routines cool at their authored rate and
    /// a hostile's are floored at `ENEMY_ROUTINE_MIN_COOLDOWN`, exactly as
    /// `wild_retaliate` floors them in the group model.
    ///
    /// Takes the `AbilityDef` itself and not an index. `tactical_use_routine`
    /// indexes `actor_abilities`, which drops any id the `AbilityDb` cannot
    /// resolve — so that index is *not* a position in `Routines`, and a
    /// caller holding a def it found for itself must not have to invert one.
    ///
    /// **Invoking beside an enemy provokes, and the price is paid first.**
    /// The order is the charge, then the reactions, then the effect — so a
    /// routine that is cut off has still spent its Power and armed its
    /// cooldown. That is a **fizzle and not a refusal**: every refusal lands
    /// before anything is spent, above in `tactical_use_routine`, and this
    /// is the rest interrupt's shape instead — the turn is gone, nothing is
    /// handed back, and the line says so. `Decompile` is exempt, because a
    /// capture is the one action whose whole cost is already the catalyst.
    pub(crate) fn run_tactical_routine(
        &mut self,
        actor: Entity,
        ability: &AbilityDef,
        aim: (i32, i32),
        cooldown_floor: u32,
    ) {
        let round_before = self.world.resource::<TacticalBattle>().round;
        // Charged before the effect resolves, at the same moment and for the
        // same reason as the group model's own Special site: a killing blow
        // ends the fight below, and a cooldown armed afterwards would be
        // written onto an entity the teardown has already cleaned up.
        self.arm_tactical_cooldown(actor, ability, cooldown_floor);
        self.spend_power(actor, abilities::routine_power_cost(ability));

        // Below the charge and above the effect — the fizzle keeps what the
        // charge took. A capture is exempt.
        if !matches!(ability.effect, AbilityEffect::Decompile) {
            let reactors = self.invoke_reactors(actor);
            if !reactors.is_empty() && !self.provoke(actor, reactors) {
                let line = format!(
                    "{}'s {} is cut off.",
                    self.creature_label(actor),
                    ability.name
                );
                self.log(line);
                // The tail the effect's own branches share: the action was
                // taken, whatever it bought, and a reaction can have ended
                // the fight under it.
                if self.world.get_resource::<TacticalBattle>().is_some() {
                    self.world.resource_mut::<TacticalBattle>().spend_action();
                }
                self.hand_on_turn(actor, round_before);
                return;
            }
        }

        let name = self.creature_label(actor);
        // A capture is aimed at a body rather than resolved over an area:
        // `decompile_body` turns one program, and a blast that turned every
        // program it touched would be a different mechanic. The group model
        // reaches the same function through a group index — see
        // `Game::attempt_decompile`.
        if matches!(ability.effect, AbilityEffect::Decompile) {
            let player = self.player_entity();
            let target = self
                .world
                .resource::<TacticalBattle>()
                .occupant(aim)
                .filter(|&e| e != actor && self.world.get::<Hostile>(e).is_some());
            if let Some(target) = target {
                // A squad's capture pulls its lead out and keeps fighting —
                // the squad itself stays on the board (unless the capture's
                // own damage just killed it, which the ordinary reap below
                // still catches), so it is never removed here the way an
                // ordinary target is.
                if self.world.get::<Squad>(target).is_some() {
                    self.decompile_squad(target, player);
                } else if self.decompile_body(target, player) {
                    self.world.resource_mut::<TacticalBattle>().remove(target);
                }
            }
        } else if let AbilityEffect::Summon {
            count,
            extra,
            rarity_penalty,
        } = ability.effect
        {
            // A capture's branch for a capture's reason: this is not
            // resolved over `reach::recipients` either, and where the bodies
            // stand is this model's own answer. The group model's Special
            // site is the other half.
            // The same dissolve the group model's site runs: a set replaces
            // a set, and killing rather than removing leaves the reap below
            // to take the bodies off the board.
            self.dissolve_summons();
            let rolled = if extra == 0 {
                count
            } else {
                let mut rng = self.world.resource_mut::<crate::resources::GameRng>();
                rand::RngExt::random_range(&mut rng.0, count..=count + extra)
            };
            for body in self.fork_programs(actor, rolled, rarity_penalty) {
                if !self.seat_summon_on_board(actor, body) {
                    // Out of room part-way through a cluster. The bodies
                    // already seated stay; this one is swept with them at
                    // teardown, exactly as an unseated one would be.
                    break;
                }
            }
        } else if let AbilityEffect::Tamper { kind, duration } = ability.effect {
            // `Decompile`'s reason and `Summon`'s: seated by the one combat
            // model that can resolve it rather than through `use_ability`'s
            // recipient loop, which carries the `unreachable!` arm this
            // branch is what makes actually unreachable.
            self.apply_tamper(actor, ability, kind, duration, aim);
        } else {
            let shape = ability.tactical_shape();
            // Taken **before** the routine resolves: it can kill its own
            // invoker, and neither the cell it ran from nor the side it was on
            // can be asked of a body the reap has taken off the board.
            let passing = self.is_hallucinating(actor).then(|| {
                let battle = self.world.resource::<TacticalBattle>();
                let cells = battle
                    .cell_of(actor)
                    .map(|from| reach::shape_cells(&battle.board, from, aim, shape))
                    .unwrap_or_default();
                let line = format!(
                    "{}'s {} passes through a decoy.",
                    self.tamper_label(actor),
                    ability.name
                );
                (self.world.get::<Hostile>(actor).is_some(), cells, line)
            });
            let recipients =
                reach::recipients(self.world.resource::<TacticalBattle>(), actor, aim, shape);
            self.use_ability(ability, actor, &name, &recipients);
            if let Some((actor_hostile, cells, line)) = passing {
                self.pass_through_decoys(actor_hostile, &cells, line);
            }
        }

        // A routine can drop a body anywhere on the board — that is what
        // friendly fire means — so the reap is over the whole roster rather
        // than over what was aimed at, exactly as it is after a swing.
        if self.world.get_resource::<TacticalBattle>().is_some() {
            self.world.resource_mut::<TacticalBattle>().spend_action();
            let aimed = self.world.resource::<TacticalBattle>().occupant(aim);
            self.reap_tactical_dead(aimed);
        }
        self.hand_on_turn(actor, round_before);
    }

    /// Hands the turn on once `actor` has spent every action it has, and
    /// spends the round's upkeep when the order comes back round.
    ///
    /// **Another action still owed is not the turn ending.** Called once
    /// per action — the per-action `spend_action`/`hand_on_turn` pairs stay
    /// at each door rather than being hoisted to fire once after the whole
    /// turn — so a body with more than one action reads its own still-fresh
    /// `actions_left` here and gets nothing below: not the tamper age, not
    /// the decoy settle, not the round upkeep, all of which count a *turn*,
    /// not an action. Only its walk is cleared, so the next action plans a
    /// fresh one from wherever this one left the body standing.
    ///
    /// **The identity check has to run first, and stay first.** A body
    /// killed by its own action — a fumble's recoil, a blast centred on its
    /// own cell — left the order inside the reap that ran before this was
    /// called, and `TacticalBattle::remove` hands the turn on as it goes,
    /// because the cursor names a body rather than a position. Reading
    /// `actions_left` before checking `actor` is still `battle.actor()`
    /// would read the *next* body's fresh budget and mistake it for this
    /// one's still-open turn — silently skipping the round's upkeep
    /// whenever that death landed on the last rung of the order, and, were
    /// the reap ever deferred instead of run per action, leaving a dead
    /// body sitting as the acting one.
    ///
    /// **`round_before` is read by the caller, before it acts.** The order
    /// wraps in two places, not one: `end_turn` below, and
    /// `TacticalBattle::remove`, which calls `wrap()` itself — so a body
    /// that dies on the *last* rung starts the next round without
    /// `end_turn` being reached at all, and a wrap detected across this
    /// function alone would miss it and skip that round's upkeep.
    ///
    /// The upkeep is the one the group model's round spends in
    /// `battle_resolve_round`'s last two lines, at the same cadence — see
    /// `tactical_round_upkeep`.
    pub(crate) fn hand_on_turn(&mut self, actor: Entity, round_before: u32) {
        let continues = self
            .world
            .get_resource::<TacticalBattle>()
            .is_some_and(|battle| battle.actor() == Some(actor) && battle.actions_left() > 0);
        if continues {
            self.world.resource_mut::<TacticalBattle>().clear_walk();
            return;
        }

        // **First, and only while `actor` is alive.** Duration counts the
        // tampered body's own turns — `components::Tampered`'s reason for
        // ageing here rather than in `Game::tick_one_combatant` — so this is
        // the one place that turn is known to have happened. Guarded on
        // being alive so a body that died to its own action this turn (a
        // fumble's recoil, a blast on its own cell) doesn't age a turn it
        // no longer has a next one to reach.
        if self.creature_alive(actor) {
            self.age_tamper(actor);
        }
        // After the ageing, so a Hallucination that has just run out stops
        // holding its decoys up in the same hand-on — and after any strike or
        // routine this turn, so a body that took its last decoy sees clearly
        // before the next body acts rather than when its duration says.
        self.settle_decoys();
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        if battle.actor() == Some(actor) {
            self.world.resource_mut::<TacticalBattle>().end_turn();
        }
        if self.world.resource::<TacticalBattle>().round > round_before {
            self.tactical_round_upkeep();
        }
    }

    /// What a round costs, on a battle map exactly as in a group fight:
    /// cooldowns come down, status effects tick and expire, combat buffs
    /// age, whatever that killed leaves the board, and the world spends one
    /// tick.
    ///
    /// **`battle_resolve_round`'s tail, in the same order.** Without it
    /// every routine is once per fight — nothing decrements the cooldown
    /// its own refusal counts down in "rounds" — a `Stun` never wears off,
    /// `Bleed` never bites, every authored `duration` lasts the whole
    /// fight, and the world stands still for as long as the player is on
    /// the board.
    ///
    /// **The reap is between the upkeep and the tick, and that is not
    /// arrangement.** The upkeep can kill — a Bleed is damage — and
    /// `difficulty::death_handling_system` rides `Game::tick`, so ticking
    /// first reboots a Forgiving player *inside* a fight that is still
    /// open: they read as alive again, `settle_tactical` sees nothing
    /// wrong, and their world `Position` has been warped to the anchor
    /// while they stand on a board. The group model has this by
    /// construction — `tick_round_status_effects` reaps and tears down
    /// before `battle_resolve_round` reaches its tick — and this is the
    /// same sequence spelled out.
    ///
    /// The tick is skipped when the reap closed the fight, because
    /// `settle_tactical` spent the round's tick on the way out.
    fn tactical_round_upkeep(&mut self) {
        let player = self.player_entity();
        self.tick_combatant_upkeep(player);
        self.reap_tactical_dead(None);
        if self.world.get_resource::<TacticalBattle>().is_some() {
            self.tick();
        }
    }

    /// `Game::arm_cooldown` with a floor under what it writes.
    ///
    /// A `cooldown: 0` routine is armed by neither, so a hostile carrying one
    /// would otherwise run it every single turn of the fight.
    fn arm_tactical_cooldown(&mut self, actor: Entity, ability: &AbilityDef, floor: u32) {
        if floor == 0 {
            self.arm_cooldown(actor, ability);
            return;
        }
        let mut cooldowns = self
            .world
            .get::<AbilityCooldowns>(actor)
            .map(|c| c.0.clone())
            .unwrap_or_default();
        cooldowns.insert(
            ability.id.clone(),
            abilities::armed_cooldown(ability.cooldown, floor),
        );
        self.world
            .entity_mut(actor)
            .insert(AbilityCooldowns(cooldowns));
    }

    /// Ends the acting body's turn without spending any of its actions.
    ///
    /// **Forfeits whatever is left, rather than spending one.** `hand_on_
    /// turn`'s "another action is coming" gate reads `actions_left` alone
    /// once the identity check passes, so a pass that only spent one of two
    /// would read as a turn still open and hand nobody anything.
    ///
    /// Through `hand_on_turn` like every other way a turn ends, so a passed
    /// turn buys the round's upkeep exactly as a spent one does.
    pub fn tactical_end_turn(&mut self) {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        let round_before = battle.round;
        if let Some(actor) = battle.actor() {
            self.world
                .resource_mut::<TacticalBattle>()
                .forfeit_actions();
            self.hand_on_turn(actor, round_before);
        }
    }

    /// Clears the dead off the board and pays for them, then closes the
    /// fight if that ended it.
    ///
    /// `wild` names whoever the action was aimed at, which is what
    /// `finish_fight` wants cleared of combat-only effects even when the
    /// blow that ended the fight was struck at somebody else. `None` where
    /// the aim named an empty cell — a routine may be aimed at ground.
    fn reap_tactical_dead(&mut self, wild: Option<Entity>) {
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
                match self.world.get::<Squad>(body).map(|s| s.members.clone()) {
                    Some(members) => self.reap_squad(body, &members, player),
                    None => self.finish_hostile(body, player),
                }
            }
        }
        self.settle_tactical(wild);
    }

    /// A squad's death pays each of its still-living members' own kill —
    /// XP, loot, nest/patrol consequences alike, exactly the payout five
    /// separate kills would give — and only then despawns the squad shell
    /// itself, which draws no payout of its own.
    ///
    /// The squad's own overkill is read here, before the despawn takes its
    /// `Stats` away, and shared evenly across the members being paid —
    /// `Game::finish_hostile_with_overkill`'s reason: a member's own
    /// `Stats` never moved, so `overkill_term` read off one directly would
    /// always answer `0.0` regardless of how hard the squad's kill actually
    /// landed.
    fn reap_squad(&mut self, squad: Entity, members: &[Entity], player: Entity) {
        let overkill = self.overkill_term(squad) / (members.len().max(1) as f32);
        for &member in members {
            self.finish_hostile_with_overkill(member, player, overkill);
        }
        self.world.despawn(squad);
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
        // Every reap and every departure comes through here, including the
        // reap in the round's upkeep that no hand-on follows — so a decoy
        // never outlives the last body it was fooling by a turn.
        self.settle_decoys();
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
        let rounds = battle.round;
        let outmatched = battle.outmatched;
        // `battle`'s last read — freeing the borrow of `self.world` before
        // the squad sweep below needs `&mut self`. A squad still standing
        // when the fight ends for a reason other than its own death (the
        // player down, or gone) is the second of the three ways one
        // survives a fight, `depart_tactical`'s own direct call being the
        // first.
        self.disband_surviving_squads();
        let verdict = FightVerdict {
            won: hostiles == 0,
            rounds,
            outmatched,
            // A lair is roused in the Stack and the Stack stays abstract, so
            // a tactical fight never has one. Stated rather than omitted:
            // this is the field a fourth in-scope encounter kind would have
            // to answer.
            lair: None,
        };
        self.finish_fight(player, wild, verdict);
        // The round the fight died in still costs what a round costs, and a
        // defeat is absorbed by it: the group model runs
        // `battle_resolve_round`'s trailing tick after `end_battle` has
        // already torn the fight down, which is what puts
        // `difficulty::death_handling_system` inside the fight rather than
        // after it. A battle map that ends mid-round never reaches the wrap
        // that would otherwise owe this — so left out, the player walks off
        // the board at zero Integrity and reboots a moment later standing
        // on the map.
        self.tick();
        true
    }

    /// Closes a tactical fight the way it would close itself, for a caller
    /// that has decided it is over for a reason of its own.
    pub fn end_tactical_battle(&mut self, wild: Option<Entity>) {
        self.settle_tactical(wild);
    }
}
