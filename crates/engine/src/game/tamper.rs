//! `AbilityEffect::Tamper`'s other half: writing a `Tampered` entry and
//! ageing it away. `Game::run_tactical_routine` is the one caller of
//! `apply_tamper`; `tactical/turn.rs::hand_on_turn` is the one caller of
//! `age_tamper`.
//!
//! A Hallucination's decoys live here too: where they are seated, what a
//! body sees of them, the door that strikes one, and `settle_decoys`, which
//! clears what nobody can see any more. The decisions that read them are
//! `tactical/ai.rs`'s.

use crate::abilities::{TamperKind, TamperSlot};
use crate::components::{Glyph, GlyphColor};
use crate::resources::{BoltCue, BoltQueue};
use crate::tactical::{Decoy, TacticalBattle, opposes, reach};
use crate::*;

impl Game {
    /// Applies `ability`'s `Tamper` effect, run by `actor` and aimed at
    /// `aim`, to every recipient but the player.
    ///
    /// **The player is never a recipient.** `reach::recipients` reads no
    /// side, so a shaped tamper would otherwise catch the player exactly as
    /// full friendly fire catches a companion — dropped here rather than
    /// filtered by a caller, since every caller shares the one rule.
    ///
    /// **`Hallucinating` goes through `hallucinate`**, which seats the decoys
    /// first and writes the entry only on the side opposing `actor` — and on
    /// nobody at all when no decoy found a cell.
    ///
    /// `fresh` is `recipient == actor`: a companion's own radius can catch
    /// its caster, and `TamperEntry`'s first `age` has to know not to spend
    /// the turn that just applied it — Decision 5.
    pub(crate) fn apply_tamper(
        &mut self,
        actor: Entity,
        ability: &AbilityDef,
        kind: TamperKind,
        duration: u32,
        aim: (i32, i32),
    ) {
        let recipients = self.tamper_recipients(actor, ability, aim);

        if let TamperKind::Hallucinating { decoys } = kind {
            self.hallucinate(actor, ability, &recipients, decoys, duration, aim);
        } else {
            for recipient in recipients {
                self.write_tamper_entry(recipient, kind, duration, recipient == actor);
                self.log_tamper_take_hold(recipient, kind);
            }
        }

        if ability.effect.breaks_cloak() {
            self.break_cloak(actor);
        }
    }

    /// The bodies a tamper aimed at `aim` would write an entry on — every
    /// body `ability`'s shape covers, less the player.
    ///
    /// **A `&self` derivation rather than a step of `apply_tamper`**, because
    /// `tactical_use_routine` has to ask the same question before it spends
    /// anything: a Hallucination that covers nobody on the other side is
    /// refused there, and a refusal that asked a second copy of this could
    /// disagree with what the effect then did.
    pub(crate) fn tamper_recipients(
        &self,
        actor: Entity,
        ability: &AbilityDef,
        aim: (i32, i32),
    ) -> Vec<Entity> {
        let battle = self.world.resource::<TacticalBattle>();
        reach::recipients(battle, actor, aim, ability.tactical_shape())
            .into_iter()
            .filter(|&e| self.world.get::<Player>(e).is_none())
            .collect()
    }

    /// The cells a Hallucination run by `actor` and aimed at `aim` would seat
    /// its `count` decoys on — Decision 3.
    ///
    /// **Free cells only, nearest the aim first, then reading order.** The aim
    /// is usually a body, and a decoy under a body's own feet would be struck
    /// at distance zero without a step; so the aim leads only when it is
    /// free. **No `GameRng`** — a tactical fight's budget is one draw an AI
    /// turn, and this is not one.
    ///
    /// Pure over the board, which is what lets `tactical_use_routine` refuse
    /// an invocation that would seat nothing before it spends the Power, the
    /// cooldown and the turn on it.
    pub(crate) fn hallucination_cells(
        &self,
        actor: Entity,
        ability: &AbilityDef,
        aim: (i32, i32),
        count: u32,
    ) -> Vec<(i32, i32)> {
        let battle = self.world.resource::<TacticalBattle>();
        let Some(from) = battle.cell_of(actor) else {
            return Vec::new();
        };
        let mut cells: Vec<(i32, i32)> =
            reach::shape_cells(&battle.board, from, aim, ability.tactical_shape())
                .into_iter()
                .filter(|&c| {
                    battle.board.walkable(c.0, c.1)
                        && battle.occupant(c).is_none()
                        && !battle.decoys().iter().any(|d| d.cell == c)
                })
                .collect();
        cells.sort_by_key(|&(x, y)| (reach::distance((x, y), aim), y, x));
        cells.truncate(count as usize);
        cells
    }

    /// `Tampered`'s own `arm_cloak` — inserts the component fresh if
    /// `recipient` carries none yet, otherwise mutates the one already
    /// there. `Tampered::apply` is what actually refreshes the slot.
    fn write_tamper_entry(
        &mut self,
        recipient: Entity,
        kind: TamperKind,
        duration: u32,
        fresh: bool,
    ) {
        match self.world.get_mut::<Tampered>(recipient) {
            Some(mut tampered) => tampered.apply(kind, duration, fresh),
            None => {
                let mut tampered = Tampered::default();
                tampered.apply(kind, duration, fresh);
                self.world.entity_mut(recipient).insert(tampered);
            }
        }
    }

    /// Ages `body`'s `Tampered` entries by one of its own turns, logging a
    /// wear-off line per kind that expired — `tactical/turn.rs::hand_on_turn`'s
    /// one caller, on the tampered body's own hand-on.
    pub(crate) fn age_tamper(&mut self, body: Entity) {
        let Some(mut tampered) = self.world.get_mut::<Tampered>(body) else {
            return;
        };
        let expired = tampered.age();
        if tampered.is_empty() {
            self.world.entity_mut(body).remove::<Tampered>();
        }
        for kind in expired {
            self.log_tamper_wear_off(body, kind);
        }
    }

    /// Seats `count` decoys of `actor`'s side over `hallucination_cells`,
    /// then writes the entry on every recipient standing on the other side —
    /// Decision 3.
    ///
    /// **The entry goes on the other side alone**, by literal `Hostile`
    /// through `tactical::opposes`: a companion caught in the party's own
    /// Hallucination ignores the party's decoys, so an entry on it would be a
    /// `HALL` tag that changes nothing.
    ///
    /// **Both empty cases are silent here, and the player's door refuses them
    /// before they can happen.** `tactical_use_routine` will not spend Power,
    /// a cooldown or a turn on an aim that seats no decoy or covers no
    /// opposing body. The early return and the loop that writes nothing stay
    /// anyway, because the enemy AI reaches `run_tactical_routine` through
    /// `wild_routine_ready`'s gate rather than through those refusals.
    fn hallucinate(
        &mut self,
        actor: Entity,
        ability: &AbilityDef,
        recipients: &[Entity],
        count: u32,
        duration: u32,
        aim: (i32, i32),
    ) {
        let owner_hostile = self.world.get::<Hostile>(actor).is_some();
        let cells = self.hallucination_cells(actor, ability, aim, count);
        if cells.is_empty() {
            return;
        }

        let (glyph, color) = self
            .world
            .get::<Glyph>(actor)
            .map(|g| (g.ch, g.color))
            .unwrap_or(('@', GlyphColor::White));
        let of_player = actor == self.player_entity();
        {
            let mut battle = self.world.resource_mut::<TacticalBattle>();
            for cell in cells {
                battle.place_decoy(Decoy {
                    cell,
                    owner_hostile,
                    glyph,
                    color,
                    of_player,
                });
            }
        }

        let kind = TamperKind::Hallucinating { decoys: count };
        for &recipient in recipients {
            if !opposes(
                owner_hostile,
                self.world.get::<Hostile>(recipient).is_some(),
            ) {
                continue;
            }
            self.write_tamper_entry(recipient, kind, duration, false);
            self.log_tamper_take_hold(recipient, kind);
        }
    }

    /// Whether `body` carries a live `Hallucinating` entry.
    pub(crate) fn is_hallucinating(&self, body: Entity) -> bool {
        self.world
            .get::<Tampered>(body)
            .is_some_and(|t| t.has(TamperSlot::Hallucinating))
    }

    /// Whether `body` sees `decoy`: it is hallucinating, and the decoy was
    /// placed by the side it is not on.
    ///
    /// **The one predicate for what a body sees.** The walk, the swing, the
    /// aim and `settle_decoys` all read this, and it reads `Decoy::opposes`,
    /// so a party body can never be handed its own side's decoy by one of
    /// them reading the owner a different way.
    pub(crate) fn sees_decoy(&self, body: Entity, decoy: &Decoy) -> bool {
        self.is_hallucinating(body) && decoy.opposes(self.world.get::<Hostile>(body).is_some())
    }

    /// The decoy `body` sees nearest `from`, by the distance a swing is
    /// measured in (`reach::distance`, Chebyshev), ties to reading order — the
    /// cell a hallucinating body's `tactical_sides` makes its only target.
    ///
    /// **Measured from a cell rather than from the body's own**, because the
    /// nearest decoy changes along a walk: the turn asks from where the body
    /// stands on each beat, and a forecast has to ask from where the walk
    /// will end.
    pub(crate) fn nearest_seen_decoy(&self, body: Entity, from: (i32, i32)) -> Option<(i32, i32)> {
        let battle = self.world.get_resource::<TacticalBattle>()?;
        battle
            .decoys()
            .iter()
            .filter(|d| self.sees_decoy(body, d))
            .map(|d| d.cell)
            .min_by_key(|&(x, y)| (reach::distance(from, (x, y)), y, x))
    }

    /// Whether `body` sees a decoy on `cell`.
    pub(crate) fn sees_decoy_at(&self, body: Entity, cell: (i32, i32)) -> bool {
        self.world
            .get_resource::<TacticalBattle>()
            .is_some_and(|battle| {
                battle
                    .decoys()
                    .iter()
                    .any(|d| d.cell == cell && self.sees_decoy(body, d))
            })
    }

    /// The acting body swings at the decoy on `cell`, and reports whether it
    /// did.
    ///
    /// **A door that takes a cell rather than an entity**, because a decoy is
    /// not one — `tactical_attack`'s sibling, holding the same range and
    /// sight gates so a decoy is struck from exactly where a body on that
    /// cell could have been. Decoys never block sight, so the line is the
    /// board's alone.
    ///
    /// Every refusal lands before anything moves: no fight or actor, no
    /// actions left to spend, an actor that is not hallucinating (Decision 6
    /// — a body that cannot see a decoy cannot aim at one), no decoy it sees
    /// on `cell`, out of `swing_range`, or no line of sight.
    ///
    /// Nothing is damaged, so there is no reap; the hand-on settles the
    /// decoys, which is what ends the entry when this was the last one.
    pub(crate) fn tactical_strike_decoy(&mut self, cell: (i32, i32)) -> bool {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return false;
        };
        let Some(actor) = battle.actor() else {
            return false;
        };
        if battle.actions_left() == 0 || !self.sees_decoy_at(actor, cell) {
            return false;
        }
        let Some(from) = battle.cell_of(actor) else {
            return false;
        };
        if !reach::swing_reaches(&battle.board, from, cell, self.swing_range(actor)) {
            return false;
        }

        let round_before = battle.round;
        let actor_hostile = self.world.get::<Hostile>(actor).is_some();
        let color = self
            .world
            .get::<Glyph>(actor)
            .map(|g| g.color)
            .unwrap_or(GlyphColor::White);
        self.world.resource_mut::<BoltQueue>().push(BoltCue {
            from,
            to: cell,
            color,
        });
        self.world
            .resource_mut::<TacticalBattle>()
            .take_decoy_at(cell, actor_hostile);
        let label = self.tamper_label(actor);
        self.log(format!("{label}'s swing passes through a decoy."));
        self.world.resource_mut::<TacticalBattle>().spend_action();
        self.hand_on_turn(actor, round_before);
        true
    }

    /// Takes every decoy `actor` sees on `cells` off the board, logging one
    /// line for the lot — what a routine run by a hallucinating body does to
    /// the decoys in its shape, on top of whatever it did to the real bodies
    /// there.
    ///
    /// `actor_hostile` is read by the caller before the routine resolved,
    /// since the routine may have killed its own invoker.
    pub(crate) fn pass_through_decoys(
        &mut self,
        actor_hostile: bool,
        cells: &[(i32, i32)],
        line: String,
    ) {
        let Some(mut battle) = self.world.get_resource_mut::<TacticalBattle>() else {
            return;
        };
        let before = battle.decoys().len();
        battle.retain_decoys(|d| !(d.opposes(actor_hostile) && cells.contains(&d.cell)));
        if battle.decoys().len() < before {
            self.log(line);
        }
    }

    /// Clears what nobody can see any more.
    ///
    /// 1. **A hallucinating body with no decoy it sees left sees clearly**,
    ///    and its entry goes now rather than at the end of its duration.
    /// 2. **Then every decoy no living hallucinating body sees is dropped**,
    ///    so a decoy never outlives the last body it was fooling.
    ///
    /// In that order, because the first can empty the set the second reads.
    /// Called from `hand_on_turn` after ageing — an entry that expired, or a
    /// decoy struck or passed through, is settled before the next body acts —
    /// and from `settle_tactical`, which every reap and every departure
    /// reaches, so a death in the round's upkeep settles with no hand-on
    /// after it.
    pub(crate) fn settle_decoys(&mut self) {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        let watchers: Vec<Entity> = battle
            .bodies()
            .map(|(body, _)| body)
            .filter(|&body| self.creature_alive(body) && self.is_hallucinating(body))
            .collect();

        let mut clear = Vec::new();
        let mut seeing = Vec::new();
        for body in watchers {
            match battle.decoys().iter().any(|d| self.sees_decoy(body, d)) {
                true => seeing.push(self.world.get::<Hostile>(body).is_some()),
                false => clear.push(body),
            }
        }
        for body in clear {
            let Some(mut tampered) = self.world.get_mut::<Tampered>(body) else {
                continue;
            };
            let Some(entry) = tampered.remove(TamperSlot::Hallucinating) else {
                continue;
            };
            if tampered.is_empty() {
                self.world.entity_mut(body).remove::<Tampered>();
            }
            self.log_tamper_wear_off(body, entry.kind);
        }

        self.world
            .resource_mut::<TacticalBattle>()
            .retain_decoys(|d| seeing.iter().any(|&hostile| d.opposes(hostile)));
    }

    /// `entity_label` for a hostile, `creature_label` for a companion —
    /// `tick_combatant_upkeep`'s own split. The player is never a subject
    /// here: `apply_tamper` drops it before either log fires.
    pub(crate) fn tamper_label(&self, body: Entity) -> String {
        if self.world.get::<Hostile>(body).is_some() {
            self.entity_label(body)
        } else {
            self.creature_label(body)
        }
    }

    fn log_tamper_take_hold(&mut self, recipient: Entity, kind: TamperKind) {
        let label = self.tamper_label(recipient);
        let line = match kind {
            TamperKind::Temperature(_) if kind.runs_cold() => {
                format!("{label}'s sampler runs cold.")
            }
            TamperKind::Temperature(_) => format!("{label}'s sampler runs hot."),
            TamperKind::Profiled => format!("{label}'s policy is profiled."),
            TamperKind::Injected => format!("{label} accepts an injected prompt."),
            TamperKind::Hallucinating { .. } => format!("{label} starts seeing decoys."),
        };
        self.log(line);
    }

    fn log_tamper_wear_off(&mut self, body: Entity, kind: TamperKind) {
        let label = self.tamper_label(body);
        let line = match kind {
            TamperKind::Temperature(_) => format!("{label}'s sampler settles."),
            TamperKind::Profiled => format!("{label}'s profile goes stale."),
            TamperKind::Injected => format!("{label} rejects the injection."),
            TamperKind::Hallucinating { .. } => format!("{label} sees clearly again."),
        };
        self.log(line);
    }
}
