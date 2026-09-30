//! What a stat spend or a perk purchase would change in a fight, read off
//! the real computation rather than a delta added to a snapshot: a trial
//! applies the commit's own write core, reads the player back, and puts
//! every component the core wrote back where it was.
//!
//! The reason it is a trial and not arithmetic: attribute points do not
//! always reach combat (`Game::emulated_base` replaces ATK and Mitigation
//! under emulation), and a second model of how attributes and perks reach
//! `combatant_profile` would be a copy that drifts.

use crate::components::{Attributes, BoughtStats, Decompiler, Derived, PowerReserve, StatPoints};
use crate::game::level_up::{duel_comparison, stat_rows};
use crate::*;
use bevy_ecs::component::Component;

/// Every player component `apply_stat_spend`, `apply_perk_level` and
/// `recompute_derived` (with `apply_equipment_delta`) write, each as it was,
/// including absent. A component a future change writes from those paths
/// must be added here, and the no-leak test in `tests/spend_preview.rs`
/// fails until it is.
struct TrialState {
    attributes: Option<Attributes>,
    stat_points: Option<StatPoints>,
    perks: Option<Perks>,
    bought: Option<BoughtStats>,
    stats: Option<Stats>,
    decompiler: Option<Decompiler>,
    power: Option<PowerReserve>,
    derived: Option<Derived>,
}

fn put<C: Component>(world: &mut World, entity: Entity, value: Option<C>) {
    match value {
        Some(v) => {
            world.entity_mut(entity).insert(v);
        }
        None => {
            world.entity_mut(entity).remove::<C>();
        }
    }
}

impl TrialState {
    fn take(world: &World, e: Entity) -> Self {
        Self {
            attributes: world.get::<Attributes>(e).cloned(),
            stat_points: world.get::<StatPoints>(e).copied(),
            perks: world.get::<Perks>(e).cloned(),
            bought: world.get::<BoughtStats>(e).copied(),
            stats: world.get::<Stats>(e).copied(),
            decompiler: world.get::<Decompiler>(e).copied(),
            power: world.get::<PowerReserve>(e).copied(),
            derived: world.get::<Derived>(e).copied(),
        }
    }

    fn restore(self, world: &mut World, e: Entity) {
        put(world, e, self.attributes);
        put(world, e, self.stat_points);
        put(world, e, self.perks);
        put(world, e, self.bought);
        put(world, e, self.stats);
        put(world, e, self.decompiler);
        put(world, e, self.power);
        put(world, e, self.derived);
    }
}

impl Game {
    /// Runs `apply`, reads the result with `read`, and restores the player's
    /// components exactly as they were. No recompute on the way back: the
    /// clones *are* the recomputed state.
    fn trial<T>(&mut self, apply: impl FnOnce(&mut Game), read: impl FnOnce(&Game) -> T) -> T {
        let player = self.player_entity();
        let saved = TrialState::take(&self.world, player);
        apply(self);
        let out = read(self);
        saved.restore(&mut self.world, player);
        out
    }

    /// The player's comparison of `before` with what `apply` leaves behind,
    /// or `None` when there is no player body to read.
    fn preview_change(&mut self, apply: impl FnOnce(&mut Game, Entity)) -> Option<PerkPreview> {
        let player = self.player_entity();
        self.world.get::<Stats>(player)?;
        let before = self.snapshot_player(player);
        let after = self.trial(|g| apply(g, player), |g| g.snapshot_player(player));
        Some(self.compare(&before, &after))
    }

    fn compare(
        &self,
        before: &crate::resources::LevelSnapshot,
        after: &crate::resources::LevelSnapshot,
    ) -> PerkPreview {
        let (foe, foe_ehp) = self.typical_foe();
        let zone = self.world.resource::<ZoneLevel>().0;
        PerkPreview {
            stats: stat_rows(before, after),
            duel: duel_comparison(before, after, foe, foe_ehp, zone),
        }
    }

    /// The duel `spend` would leave the player with, against a typical
    /// program of the current zone. An unaffordable spend is still
    /// previewed: the Points screen clamps to the pool itself.
    pub fn preview_stat_spend(
        &mut self,
        spend: &[(crate::attributes::AttributeId, u32)],
    ) -> Option<DuelComparison> {
        self.preview_change(|g, player| g.apply_stat_spend(player, spend))
            .map(|p| p.duel)
    }

    /// What one more level of `perk` would change. Previewed whatever the
    /// Perk Points, so an unaffordable perk still reads what it does.
    pub fn preview_perk(&mut self, perk: Perk) -> Option<PerkPreview> {
        if self.is_game_over().is_some() {
            return None;
        }
        self.world.resource::<PerkDb>().get(perk)?;
        self.preview_change(|g, player| {
            g.apply_perk_level(player, perk);
        })
    }

    /// `unlock_perk`, read back: the perk bought, the level now held and
    /// what the purchase changed.
    pub fn buy_perk(&mut self, perk: Perk) -> Result<PerkReport, String> {
        let player = self.player_entity();
        let before = self.snapshot_player(player);
        self.unlock_perk(perk)?;
        let after = self.snapshot_player(player);
        let def = self
            .world
            .resource::<PerkDb>()
            .get(perk)
            .ok_or_else(|| "That perk isn't available.".to_string())?;
        Ok(PerkReport {
            name: def.name.clone(),
            level: self.world.get::<Perks>(player).map_or(0, |p| p.level(perk)),
            description: def.description.clone(),
            preview: self.compare(&before, &after),
        })
    }
}
