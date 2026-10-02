//! The player's derived figures: `progression::derive` applied to the
//! entity's attributes, written back through the one door.

use crate::components::{Attributes, Derived, HoldPoints, ProgramBase, StatPoints, Tamed};
use crate::progression::{SpendError, StatOwner};
use crate::*;

impl Game {
    /// `entity`'s maximum Power: its derived figure when it has one,
    /// `POWER_MAX` otherwise.
    pub fn max_power(&self, entity: Entity) -> f32 {
        crate::components::max_power_of(self.world.get::<Derived>(entity))
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
        let StatOwner::Player = owner;
        let entity = self.player_entity();
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

    /// What `entity` holds on top of its attributes' derivation - the
    /// perk receipt and worn gear - so the Points screen can preview the
    /// figures the HUD will show. Read off the live stats rather than summed
    /// again, so it cannot drift from `recompute_derived`. Only the stats
    /// gear or a perk can move are non-zero.
    pub fn stat_bonus(&self, entity: Entity) -> crate::progression::DerivedStats {
        let derived = self.derived_stats(entity);
        let stats = *self
            .world
            .get::<Stats>(entity)
            .expect("an entity with a stat bonus has Stats");
        let skill = self.world.get::<Decompiler>(entity).map_or(0, |d| d.skill);
        crate::progression::DerivedStats {
            max_hp: stats.max_hp - derived.max_hp,
            atk: stats.atk - derived.atk,
            mitigation: stats.mitigation - derived.mitigation,
            decompiler: skill - derived.decompiler,
            max_power: 0.0,
            status_resist: 0,
            extraction: 0.0,
            crit: 0.0,
            fumble: 0.0,
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
        let derived = self.derived_stats(entity);
        let receipt = self
            .world
            .get::<BoughtStats>(entity)
            .copied()
            .unwrap_or_default();
        let gear = self.gear_bonus(entity);
        if let Some(mut stats) = self.world.get_mut::<Stats>(entity) {
            stats.max_hp = derived.max_hp + receipt.max_hp;
            stats.atk = derived.atk + receipt.atk;
            stats.mitigation = derived.mitigation + receipt.mitigation;
            stats.hp = stats.hp.min(stats.max_hp);
        }
        if let Some(mut decompiler) = self.world.get_mut::<Decompiler>(entity) {
            decompiler.skill = derived.decompiler;
        }
        self.apply_equipment_delta(entity, gear, 1);
        if let Some(mut power) = self.world.get_mut::<PowerReserve>(entity) {
            power.restore(0.0, derived.max_power);
        }
        self.world.entity_mut(entity).insert(Derived {
            max_power: derived.max_power,
            status_resist: derived.status_resist,
            extraction: derived.extraction,
            crit: derived.crit,
            fumble: derived.fumble,
        });
    }
}
