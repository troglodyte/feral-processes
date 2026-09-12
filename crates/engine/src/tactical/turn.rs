//! Whose turn it is on a battle map, and what a turn is made of.
//!
//! One body acts at a time, in an order rolled once when the fight opens.
//! A turn is up to `Game::movement_allowance` cells of movement and then one
//! action, and the action ends the turn — so a body that swings first has
//! given up the rest of its walk, which is the whole of this model's
//! positioning pressure.

use bevy_ecs::prelude::Entity;

use crate::Game;
use crate::abilities::{self, AbilityDef, AbilityEffect};
use crate::components::AbilityCooldowns;
use crate::components::{Hostile, Stats};
use crate::game::combat_teardown::FightVerdict;
use crate::resources::{GameClock, Party, ZoneLevel};
use crate::tactical::map::{BattleSpec, generate};
use crate::tactical::{TacticalBattle, deploy, reach};
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
        if actor == target || reach::distance(from, at) > self.swing_range(actor) {
            return false;
        }
        // **Unconditional, with no melee branch.** `line_of_sight` excludes
        // its endpoints, so for neighbours its loop is empty and this is
        // already a no-op — one rule, and no second place
        // `TACTICAL_MELEE_RANGE` has to be restated. Cover earns a second
        // job for free.
        {
            let battle = self.world.resource::<TacticalBattle>();
            if !reach::line_of_sight(&battle.board, from, at) {
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
        self.world.resource_mut::<TacticalBattle>().mark_acted();

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
    /// acting, and a body that has already taken its action. **No
    /// `is_stunned` gate**, unlike `battle_resolve_round`'s Defend loop —
    /// nothing in this model reads stun at all and a stunned body already
    /// takes a whole turn on a board, so gating the brace alone would make
    /// bracing the one thing a stunned body could not do.
    ///
    /// Reports whether the brace took. The action ends the turn.
    pub fn tactical_defend(&mut self) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let Some(actor) = battle.actor() else {
            return false;
        };
        if battle.acted() {
            return false;
        }

        let round_before = battle.round;
        self.begin_defend(actor);
        self.world.resource_mut::<TacticalBattle>().mark_acted();
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
    /// checked. Six of them: no fight, nobody acting, the body has already
    /// acted, no such routine, a routine that is not run in a fight at all
    /// (a passive, or a field-only effect — `battle_special_options`' own
    /// two exclusions), whatever `ability_unavailable` says, and an aim
    /// outside the routine's range.
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
        if battle.acted() {
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
        crate::tactical::deploy::nearest_free(&battle.board, &taken, from).is_some()
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
                crate::tactical::deploy::nearest_free(&battle.board, &taken, from)
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
            if let Some(target) = target
                && self.decompile_body(target, player)
            {
                self.world.resource_mut::<TacticalBattle>().remove(target);
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
        } else {
            let shape = ability.tactical_shape();
            let recipients =
                reach::recipients(self.world.resource::<TacticalBattle>(), actor, aim, shape);
            self.use_ability(ability, actor, &name, &recipients);
        }

        // A routine can drop a body anywhere on the board — that is what
        // friendly fire means — so the reap is over the whole roster rather
        // than over what was aimed at, exactly as it is after a swing.
        if self.world.get_resource::<TacticalBattle>().is_some() {
            self.world.resource_mut::<TacticalBattle>().mark_acted();
            let aimed = self.world.resource::<TacticalBattle>().occupant(aim);
            self.reap_tactical_dead(aimed);
        }
        self.hand_on_turn(actor, round_before);
    }

    /// Hands the turn on after `actor` has finished with it, and spends the
    /// round's upkeep when the order comes back round.
    ///
    /// **Only if `actor` is still the one acting.** A body that died to its
    /// own action — a fumble's recoil, a blast centred on its own cell —
    /// left the order inside the reap, and `TacticalBattle::remove` hands
    /// the turn on as it goes, because the cursor names a body rather than
    /// a position. Ending the turn again on top of that skips whoever was
    /// standing behind it: a companion who fumbles fatally costs the player
    /// their turn, with nothing on screen to say why.
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
    fn hand_on_turn(&mut self, actor: Entity, round_before: u32) {
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

    /// Ends the acting body's turn without spending its action.
    ///
    /// Through `hand_on_turn` like every other way a turn ends, so a passed
    /// turn buys the round's upkeep exactly as a spent one does.
    pub fn tactical_end_turn(&mut self) {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        let round_before = battle.round;
        if let Some(actor) = battle.actor() {
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
                self.finish_hostile(body, player);
            }
        }
        self.settle_tactical(wild);
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
