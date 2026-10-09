//! The player's derived figures: `progression::derive` applied to the
//! entity's attributes, written back through the one door.

use crate::components::{Attributes, Derived, HoldPoints, ProgramBase, StatPoints, Tamed};
use crate::progression::{SpendError, StatOwner};
use crate::resources::PendingProgramLevels;
use crate::*;

impl Game {
    /// `entity`'s maximum Power: its derived figure when it has one,
    /// `POWER_MAX` otherwise.
    pub fn max_power(&self, entity: Entity) -> f32 {
        crate::components::max_power_of(self.world.get::<Derived>(entity))
    }

    /// How many tiles of the surface the player sees: their derived
    /// Perception. Only the player's is read; companions don't scout.
    pub fn perception_radius(&self) -> f32 {
        self.world
            .get::<Derived>(self.player_entity())
            .map_or(crate::tuning::PERCEPTION_BASE_RADIUS, |d| d.perception)
    }

    /// Spends banked stat points on attributes, then recomputes. The whole
    /// spend is validated first and a refused one writes nothing. Never
    /// touches `BoughtStats`: that is the perk receipt, and a perk respec
    /// must not refund a level-up spend.
    pub fn spend_stat_points(
        &mut self,
        owner: StatOwner,
        spend: &[(crate::attributes::AttributeId, u32)],
    ) -> Result<(), SpendError> {
        let entity = match owner {
            StatOwner::Player => self.player_entity(),
            StatOwner::Program(program) => program,
        };
        // Only a seated program has a pool, so this is also the check that the
        // target is one.
        let banked = self
            .world
            .get::<StatPoints>(entity)
            .ok_or(SpendError::NoSuchTarget)?
            .0;
        let db = self.world.resource::<crate::attributes::AttributeDb>();
        let mut total = 0u32;
        for (id, points) in spend {
            let def = db.get(id).ok_or(SpendError::NoSuchTarget)?;
            if !def.buyable() {
                return Err(SpendError::NotBuyable);
            }
            total = total.saturating_add(*points);
        }
        if total > banked {
            return Err(SpendError::InsufficientPoints);
        }
        if self.world.get::<Attributes>(entity).is_none() {
            return Err(SpendError::NoSuchTarget);
        }
        self.apply_stat_spend(entity, spend);
        Ok(())
    }

    /// The write half of `spend_stat_points`: attributes, the banked pool,
    /// the recompute and the current-HP raise. No validation, so a caller
    /// that has not checked affordability gets a pool that floors at zero
    /// rather than an underflow - the preview relies on that.
    pub(crate) fn apply_stat_spend(
        &mut self,
        entity: Entity,
        spend: &[(crate::attributes::AttributeId, u32)],
    ) {
        let Some(mut attrs) = self.world.get::<Attributes>(entity).cloned() else {
            return;
        };
        for (id, points) in spend {
            let base = self
                .world
                .resource::<crate::attributes::AttributeDb>()
                .get(id)
                .map_or(0, |d| d.base);
            attrs.set(id, attrs.get(id).unwrap_or(base) + *points as i32);
        }
        let max_hp_before = self.world.get::<Stats>(entity).map_or(0, |s| s.max_hp);
        self.world.entity_mut(entity).insert(attrs);
        let total: u32 = spend.iter().fold(0, |n, (_, p)| n.saturating_add(*p));
        if let Some(mut points) = self.world.get_mut::<StatPoints>(entity) {
            points.0 = points.0.saturating_sub(total);
        }
        self.recompute_derived(entity);
        // Current HP rises by what the maximum rose by, so a level-up's full
        // heal is not undone by spending the points it paid, and a wounded
        // player is not healed by spending either.
        if let Some(mut stats) = self.world.get_mut::<Stats>(entity) {
            let raised = (stats.max_hp - max_hp_before).max(0);
            stats.hp = (stats.hp + raised).min(stats.max_hp);
        }
    }

    /// `(species growth multiplier, individual growth roll)` for `entity`,
    /// the two factors `progression::program_level_points` takes.
    pub(crate) fn program_growth(&self, entity: Entity) -> (f32, f32) {
        let species = self
            .world
            .get::<Creature>(entity)
            .and_then(|c| self.world.resource::<SpeciesDb>().get(&c.species))
            .map(|s| s.growth_multiplier)
            .unwrap_or(crate::tuning::BASELINE_GROWTH_MULTIPLIER);
        let roll = self
            .world
            .get::<Potential>(entity)
            .map_or(Potential::NEUTRAL.growth_roll, |p| p.growth_roll);
        (species, roll)
    }

    /// Places the points a `Growth::ProgramPoints` level-up earned and
    /// returns `gain` with `max_hp`/`atk` filled in as the derived change, so
    /// the log sites that read them stay right. Unheld, the points go into
    /// `Attributes` and the figures recompute; held, they bank in
    /// `StatPoints`. Either way a level full-heals. A no-op for an unseated
    /// entity.
    pub(crate) fn apply_program_levels(
        &mut self,
        entity: Entity,
        mut gain: crate::progression::LevelGain,
    ) -> crate::progression::LevelGain {
        let Some(held) = self.world.get::<HoldPoints>(entity).map(|h| h.0) else {
            return gain;
        };
        if gain.levels == 0 {
            return gain;
        }
        let before = self.world.get::<Stats>(entity).copied();
        if held {
            let earned = gain.parity + gain.analysis;
            if let Some(mut points) = self.world.get_mut::<StatPoints>(entity) {
                points.0 += earned;
            }
            gain.stat_points += earned;
        } else {
            // The pool is empty of these points, so `apply_stat_spend`'s
            // decrement floors at zero and only the attributes and the
            // recompute do anything.
            self.apply_stat_spend(
                entity,
                &[
                    (crate::attributes::AttributeId::from("parity"), gain.parity),
                    (
                        crate::attributes::AttributeId::from("analysis"),
                        gain.analysis,
                    ),
                ],
            );
        }
        if let Some(mut stats) = self.world.get_mut::<Stats>(entity) {
            stats.hp = stats.max_hp;
            if let Some(before) = before {
                gain.max_hp = stats.max_hp - before.max_hp;
                gain.atk = stats.atk - before.atk;
            }
        }
        gain
    }

    /// Places the level-ups `task_progress_system` queued for seated
    /// programs, then writes the stat block it could not: only now is there a
    /// delta to show.
    pub(crate) fn drain_program_levels(&mut self) {
        let pending = std::mem::take(&mut self.world.resource_mut::<PendingProgramLevels>().0);
        for (entity, gain) in pending {
            let gain = self.apply_program_levels(entity, gain);
            let Some(stats) = self.world.get::<Stats>(entity).copied() else {
                continue;
            };
            for line in crate::progression::stat_block(&gain.stat_rows(&stats)) {
                self.log_base_kind(MessageKind::LevelUp, line);
            }
        }
    }

    /// Turns holding on or off for a seated program. Turning it off spends
    /// the bank at once in the growth split, so no points are stranded: as
    /// many whole levels' worth as it holds, then the remainder into Parity.
    pub fn set_hold_points(&mut self, entity: Entity, hold: bool) -> Result<(), SpendError> {
        if self.world.get::<ProgramBase>(entity).is_none() {
            return Err(SpendError::NoSuchTarget);
        }
        self.world.entity_mut(entity).insert(HoldPoints(hold));
        if hold {
            return Ok(());
        }
        let bank = self.world.get::<StatPoints>(entity).map_or(0, |p| p.0);
        if bank == 0 {
            return Ok(());
        }
        let (g, _) = self.program_growth(entity);
        let (parity, analysis) = crate::progression::program_level_points(g, 1.0);
        let per_level = parity + analysis;
        let levels = bank.checked_div(per_level).unwrap_or(0);
        let remainder = bank - levels * per_level;
        self.apply_stat_spend(
            entity,
            &[
                (
                    crate::attributes::AttributeId::from("parity"),
                    levels * parity + remainder,
                ),
                (
                    crate::attributes::AttributeId::from("analysis"),
                    levels * analysis,
                ),
            ],
        );
        Ok(())
    }

    /// How many seated programs hold banked points: what the Manifest
    /// attention row counts.
    pub(crate) fn programs_holding_points(&mut self) -> usize {
        let mut query = self.world.query::<(&HoldPoints, &StatPoints)>();
        query
            .iter(&self.world)
            .filter(|(hold, points)| hold.0 && points.0 > 0)
            .count()
    }

    /// Unspent points `owner` has banked, 0 for one that holds none.
    pub fn stat_points_of(&self, owner: StatOwner) -> u32 {
        let entity = match owner {
            StatOwner::Player => self.player_entity(),
            StatOwner::Program(program) => program,
        };
        self.world.get::<StatPoints>(entity).map_or(0, |p| p.0)
    }

    /// The catalogue as the Points screen previews against - a clone, so a
    /// frontend holds no borrow on `Game`, `attribute_defs`' reason.
    pub fn attribute_db(&self) -> crate::attributes::AttributeDb {
        self.world
            .resource::<crate::attributes::AttributeDb>()
            .clone()
    }

    /// `entity`'s attributes as they stand, the Points screen's "before".
    pub fn attributes_of(&self, entity: Entity) -> Attributes {
        self.world
            .get::<Attributes>(entity)
            .cloned()
            .unwrap_or_default()
    }

    /// What `entity` holds on top of its attributes' derivation - implants,
    /// the perk receipt, held keys and worn gear - which `recompute_derived`
    /// applies and the Points screen previews through, so the two cannot
    /// disagree.
    pub fn stat_bonus(&self, entity: Entity) -> crate::progression::StatBonus {
        crate::progression::StatBonus {
            implants: self.implant_stats(entity),
            bought: self
                .world
                .get::<BoughtStats>(entity)
                .copied()
                .unwrap_or_default(),
            gear: self.gear_bonus(entity),
            pct: self.key_pct(entity),
        }
    }

    /// The base `entity` derives from: its `ProgramBase` when seated, the
    /// player's otherwise. The one accessor, so no caller names
    /// `DerivedBase::player()` for a program.
    pub fn derived_base(&self, entity: Entity) -> crate::progression::DerivedBase {
        self.world
            .get::<ProgramBase>(entity)
            .map_or_else(crate::progression::DerivedBase::player, |base| base.0)
    }

    /// What `progression::derive` answers for `entity` right now.
    pub(crate) fn derived_stats(&self, entity: Entity) -> crate::progression::DerivedStats {
        crate::progression::derive(
            &self.derived_base(entity),
            self.world
                .get::<Attributes>(entity)
                .unwrap_or(&Attributes::default()),
            self.world.resource::<crate::attributes::AttributeDb>(),
        )
    }

    /// Turns a tamed program with baked `Stats` into a derived one, working
    /// backwards from its current figures: worn gear, the `BoughtStats`
    /// receipt and what its attributes contribute are taken off, and what is
    /// left is its `ProgramBase`. `Stats` is therefore the same before and
    /// after for any figure in range (`derive` floors hp and attack at 1, which
    /// a hand-built fixture can sit below), so current `hp` is untouched; only
    /// Power can move, through recompute's clamp to the new maximum.
    ///
    /// A no-op for an untamed entity or one already seated, so every door
    /// that makes a program calls it last without asking.
    pub(crate) fn seat_derived(&mut self, entity: Entity) {
        if self.world.get::<Tamed>(entity).is_none()
            || self.world.get::<ProgramBase>(entity).is_some()
        {
            return;
        }
        let Some(stats) = self.world.get::<Stats>(entity).copied() else {
            return;
        };
        let bought = self
            .world
            .get::<BoughtStats>(entity)
            .copied()
            .unwrap_or_default();
        let gear = self.gear_bonus(entity);
        let contribution = crate::progression::attribute_contribution(
            self.world
                .get::<Attributes>(entity)
                .unwrap_or(&Attributes::default()),
            self.world.resource::<crate::attributes::AttributeDb>(),
        );
        let base = crate::progression::DerivedBase::program(
            stats.max_hp - bought.max_hp,
            stats.atk - bought.atk - gear.atk,
            stats.mitigation - bought.mitigation - gear.mitigation,
            &contribution,
        );
        self.world.entity_mut(entity).insert((
            ProgramBase(base),
            Derived::default(),
            StatPoints(0),
            HoldPoints(false),
        ));
        self.recompute_derived(entity);
    }

    /// The only writer of derived values. Sets `Stats::{max_hp, atk,
    /// mitigation}` to the derived figure plus the `BoughtStats` receipt,
    /// `Decompiler::skill` and `Derived`, then puts worn gear back on top.
    /// The writes are absolute, so they replace whatever gear was baked in
    /// and nothing needs lifting first. `hp` and Power are clamped to their
    /// new maxima, never refilled.
    ///
    /// A no-op for an entity without `Derived`: companions keep stored
    /// stats, and the `BoughtStats` writers that serve them call this too.
    /// Emulation needs nothing here - it overlays `Stats` at read time
    /// (`Game::emulated_base`) rather than replacing them.
    pub(crate) fn recompute_derived(&mut self, entity: Entity) {
        if self.world.get::<Derived>(entity).is_none() {
            return;
        }
        let derived = self.stat_bonus(entity).apply(self.derived_stats(entity));
        if let Some(mut stats) = self.world.get_mut::<Stats>(entity) {
            stats.max_hp = derived.max_hp;
            stats.atk = derived.atk;
            stats.mitigation = derived.mitigation;
            stats.hp = stats.hp.min(stats.max_hp);
        }
        if let Some(mut decompiler) = self.world.get_mut::<Decompiler>(entity) {
            decompiler.skill = derived.decompiler;
        }
        if let Some(mut power) = self.world.get_mut::<PowerReserve>(entity) {
            power.restore(0.0, derived.max_power);
        }
        self.world.entity_mut(entity).insert(Derived {
            max_power: derived.max_power,
            status_resist: derived.status_resist,
            extraction: derived.extraction,
            crit: derived.crit,
            fumble: derived.fumble,
            perception: derived.perception,
        });
    }
}
