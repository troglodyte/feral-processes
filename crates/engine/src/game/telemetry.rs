//! The engine half of battle telemetry: a gate, a buffer, and a drain.
//!
//! `crate::telemetry` holds the record shapes; the five combat seams build
//! them through `Game::record`; app-core drains with
//! `Game::take_battle_telemetry` and is the only crate that turns one into
//! text. Nothing here touches the disk.

use crate::resources::BattleTelemetry;
use crate::telemetry::{ActionKind, EnemyGroupHp, EnemyGroupInfo, PartyMember, Record};
use crate::*;

impl Game {
    /// Starts collecting. Called once at startup by app-core when
    /// `FERAL_DEV_LOG` is set, never from inside the sim.
    pub fn enable_battle_telemetry(&mut self) {
        self.world.resource_mut::<BattleTelemetry>().on = true;
    }

    /// Hands over everything recorded since the last call, emptying the
    /// buffer. A drain rather than a read, matching
    /// `take_pending_profile_writes`: app-core appends what it is given, so
    /// a second read must not append the same records twice.
    pub fn take_battle_telemetry(&mut self) -> Vec<Record> {
        std::mem::take(&mut self.world.resource_mut::<BattleTelemetry>().records)
    }

    /// Records one event, building it only if telemetry is on.
    ///
    /// **The closure is the point.** An eager `record(Record::EnemyChoice
    /// { .. })` would build the struct — three `String` allocations — on
    /// every swing of every fight even when disabled, and `train` pays that
    /// 1.9M times a session. There is deliberately no eager variant, so a
    /// caller cannot get this wrong.
    ///
    /// **The `&Game` parameter is what makes the lazy form compile.** A
    /// `FnOnce() -> Record` would have to capture `&self` to read a target's
    /// `Stats`, while this holds `&mut self` — that does not borrow check.
    /// Reading `on` before taking the mutable borrow, and passing `self` in,
    /// is the shape that does.
    pub(crate) fn record(&mut self, f: impl FnOnce(&Game) -> Record) {
        if !self.world.resource::<BattleTelemetry>().on {
            return;
        }
        let record = f(self);
        self.world
            .resource_mut::<BattleTelemetry>()
            .records
            .push(record);
    }

    /// One `Record::BaseSnapshot`, at the top of every ledger window.
    ///
    /// **Called from `tick_inner` and gated on nothing but the clock.** Once
    /// per `base_ledger::BUCKET_TICKS`, which is what makes it cheap enough
    /// to leave armed for a whole session and what lines it up with the
    /// buckets the ledger is already keeping.
    ///
    /// Every count is built **inside** `Game::record`'s closure, which is
    /// the whole reason this is safe: the walk costs a pass over every
    /// structure and every owned program, and a disarmed run must not pay
    /// for it. Written eagerly it would be a full base census every
    /// thousand ticks of every ordinary player's game.
    pub(crate) fn note_base_snapshot(&mut self) {
        let tick = self.world.resource::<crate::resources::GameClock>().tick;
        if !tick.is_multiple_of(crate::base_ledger::BUCKET_TICKS) {
            return;
        }
        self.record(|g| {
            // `iter_entities` and not `World::query`, which wants `&mut
            // World` — the closure holds `&Game`, which is the shape that
            // makes the whole thing lazy in the first place.
            let mut machines = 0;
            let mut depots = 0;
            let mut posted = 0;
            let mut staff = 0;
            let db = g.world.resource::<crate::structures::StructureDb>();
            for entity in g.world.iter_entities() {
                if let Some(structure) = entity.get::<crate::components::Structure>()
                    && let Some(def) = db.get(&structure.kind)
                {
                    machines += u32::from(def.runs_a_job());
                    depots += u32::from(def.stores);
                }
                if entity
                    .get::<crate::components::Task>()
                    .is_some_and(|t| matches!(t.kind, crate::components::TaskKind::GatherResource))
                {
                    posted += 1;
                }
                if entity.get::<crate::components::Tamed>().is_some()
                    && g.program_role(entity.id()) == Some(crate::ProgramRole::Staff)
                {
                    staff += 1;
                }
            }
            let (draw, supply) = g.base_power();
            Record::BaseSnapshot {
                tick,
                zone: g.world.resource::<crate::resources::ZoneLevel>().0,
                staff,
                posted,
                machines,
                depots,
                supply,
                draw,
            }
        });
    }

    /// Reports one base event: the ledger fold and the log record, through
    /// `base_ledger::emit`'s one door.
    ///
    /// The `resource_scope` here is the whole of what this exists to hold in
    /// one place. `emit` wants both resources at once and a `&mut World`
    /// lends one at a time, so every site that reported from a `Game` was
    /// otherwise writing the same eight-line dance — five of them by the end
    /// of Phase 2, each free to get the borrow order subtly different.
    ///
    /// The record closure takes `(tick, zone)` because every base record
    /// carries them and no caller should be reading the two resources for
    /// itself.
    pub(crate) fn report_base(
        &mut self,
        event: crate::base_ledger::Event,
        record: impl FnOnce(u64, u32, &crate::base_ledger::Event) -> Record,
    ) {
        let tick = self.world.resource::<crate::resources::GameClock>().tick;
        let zone = self.world.resource::<crate::resources::ZoneLevel>().0;
        self.world.resource_scope(
            |world, mut ledger: bevy_ecs::prelude::Mut<crate::base_ledger::BaseLedger>| {
                let mut telemetry = world.resource_mut::<BattleTelemetry>();
                crate::base_ledger::emit(
                    &mut ledger,
                    &mut telemetry,
                    tick,
                    zone,
                    &event,
                    |event| record(tick, zone, event),
                );
            },
        );
    }

    /// Mints the next fight id and makes it the one later records carry.
    /// Called by `begin_battle`; every record in the fight reads it back
    /// through `fight_id`.
    pub(crate) fn next_fight_id(&mut self) -> u64 {
        let mut telemetry = self.world.resource_mut::<BattleTelemetry>();
        telemetry.next_fight += 1;
        telemetry.fight = telemetry.next_fight;
        telemetry.fight
    }

    /// The fight the seams are currently tagging records with. Every record
    /// carries it, which is what makes a line in the file interpretable on
    /// its own.
    pub(crate) fn fight_id(&self) -> u64 {
        self.world.resource::<BattleTelemetry>().fight
    }

    /// The round the seams are currently tagging records with. Read off
    /// `BattleState` rather than threaded through as a parameter, because
    /// none of the seams takes one and inventing a second count is how the
    /// two drift.
    pub(crate) fn telemetry_round(&self) -> u32 {
        self.world
            .get_resource::<BattleState>()
            .map(|b| b.round)
            .unwrap_or(0)
    }

    /// Every party slot as the fight opened. `species` is `None` for the
    /// player, who carries no `Creature`.
    pub(crate) fn telemetry_party(&self) -> Vec<PartyMember> {
        let slots = self
            .world
            .get_resource::<BattleState>()
            .map(|b| b.planned.len())
            .unwrap_or(0);
        (0..slots)
            .filter_map(|slot| {
                let entity = self.actor_entity(battle::Actor::Party(slot))?;
                Some(PartyMember {
                    slot,
                    label: if slot == 0 {
                        "You".to_string()
                    } else {
                        self.creature_label(entity)
                    },
                    species: self
                        .world
                        .get::<Creature>(entity)
                        .map(|c| c.species.to_string()),
                    level: self.ability_user_level(entity),
                    max_hp: self
                        .world
                        .get::<Stats>(entity)
                        .map(|s| s.max_hp)
                        .unwrap_or(0),
                })
            })
            .collect()
    }

    pub(crate) fn telemetry_enemy_groups(&self) -> Vec<EnemyGroupInfo> {
        self.world
            .get_resource::<BattleState>()
            .map(|b| b.groups.as_slice())
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(group, g)| EnemyGroupInfo {
                group,
                species: g.species.to_string(),
                count: g.members.len(),
            })
            .collect()
    }

    /// Every party slot's HP, in slot order. A slot whose member is gone
    /// reads 0 rather than being dropped, so the vector stays index-aligned
    /// with `fight_start`'s party across the whole fight.
    pub(crate) fn telemetry_party_hp(&self) -> Vec<i32> {
        let slots = self
            .world
            .get_resource::<BattleState>()
            .map(|b| b.planned.len())
            .unwrap_or(0);
        (0..slots)
            .map(|slot| {
                self.actor_entity(battle::Actor::Party(slot))
                    .and_then(|e| self.world.get::<Stats>(e))
                    .map(|s| s.hp)
                    .unwrap_or(0)
            })
            .collect()
    }

    pub(crate) fn telemetry_enemy_hp(&self) -> Vec<EnemyGroupHp> {
        self.world
            .get_resource::<BattleState>()
            .map(|b| b.groups.clone())
            .unwrap_or_default()
            .iter()
            .enumerate()
            .map(|(group, g)| EnemyGroupHp {
                group,
                hp: g
                    .members
                    .iter()
                    .filter_map(|&e| self.world.get::<Stats>(e))
                    .map(|s| s.hp)
                    .collect(),
            })
            .collect()
    }

    /// How a planned action reads as a record. Derived from the
    /// `BattleAction` variant rather than from what resolution happened to
    /// do, which is what makes "did the human actually brace or heal"
    /// answerable — and a new variant has to be given an answer here before
    /// it compiles.
    ///
    /// The `Special` arm resolves its ability through the *same* fallback
    /// `resolve_one_action` uses, so the record can never name an ability
    /// other than the one that was actually spent.
    pub(crate) fn telemetry_action(
        &self,
        actor: Entity,
        action: &BattleAction,
    ) -> (ActionKind, Option<String>, Option<usize>) {
        match action {
            BattleAction::Attack { .. } => (ActionKind::Attack, None, None),
            BattleAction::Special { ability, target } => {
                let abilities = self.actor_abilities(actor);
                let name = abilities
                    .get(*ability)
                    .or_else(|| abilities.first())
                    .map(|a| a.id.to_string());
                let slot = match target {
                    battle::SpecialTarget::Ally { slot } => Some(*slot),
                    _ => None,
                };
                (ActionKind::Special, name, slot)
            }
            BattleAction::Defend => (ActionKind::Defend, None, None),
            BattleAction::UseItem { item } => (ActionKind::Item, Some(item.to_string()), None),
        }
    }

    /// How a party slot reads as an actor. Matches `battle_rows`' own
    /// naming so a record and the screen agree about who acted.
    pub(crate) fn telemetry_actor_label(&self, slot: usize, entity: Entity) -> String {
        if slot == 0 {
            "You".to_string()
        } else {
            self.creature_label(entity)
        }
    }
}
