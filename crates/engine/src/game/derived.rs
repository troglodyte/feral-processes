//! The player's derived figures: `progression::derive` applied to the
//! entity's attributes, written back through the one door.

use crate::components::{Attributes, Derived, StatPoints};
use crate::progression::{SpendError, StatOwner};
use crate::*;

impl Game {
    /// `entity`'s maximum Power: its derived figure when it has one,
    /// `POWER_MAX` otherwise.
    pub fn max_power(&self, entity: Entity) -> f32 {
        self.world
            .get::<Derived>(entity)
            .map_or(POWER_MAX, |d| d.max_power)
    }

    /// Spends banked stat points on attributes, then recomputes. The whole
    /// spend is validated first and a refused one writes nothing. Never
    /// touches `BoughtStats`: that is the perk receipt, and a perk respec
    /// must not refund a level-up spend.
    pub fn spend_stat_points(
        &mut self,
        owner: StatOwner,
        spend: &[(crate::attributes::AttributeId, u32)],
    ) -> Result<crate::progression::DerivedStats, SpendError> {
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
        let mut attrs = self
            .world
            .get::<Attributes>(entity)
            .cloned()
            .ok_or(SpendError::NoSuchTarget)?;
        for (id, points) in spend {
            let base = self
                .world
                .resource::<crate::attributes::AttributeDb>()
                .get(id)
                .map_or(0, |d| d.base);
            attrs.set(id, attrs.get(id).unwrap_or(base) + *points as i32);
        }
        self.world.entity_mut(entity).insert(attrs);
        self.world.get_mut::<StatPoints>(entity).unwrap().0 = banked - total;
        self.recompute_derived(entity);
        Ok(self.derived_stats(entity))
    }

    /// What `progression::derive` answers for `entity` right now.
    fn derived_stats(&self, entity: Entity) -> crate::progression::DerivedStats {
        crate::progression::derive(
            &crate::progression::DerivedBase::player(),
            self.world
                .get::<Attributes>(entity)
                .unwrap_or(&Attributes::default()),
            self.world.resource::<crate::attributes::AttributeDb>(),
        )
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
            power.clamp_to(derived.max_power);
        }
        self.world.entity_mut(entity).insert(Derived {
            max_power: derived.max_power,
            status_resist: derived.status_resist,
            extraction: derived.extraction,
        });
    }
}
