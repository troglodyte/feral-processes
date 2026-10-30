//! A Patch Node in a fought siege: it pulses a heal over everything its
//! circle covers, like a turret a structure acts once a round rather than
//! taking a turn. See ADR 0036.

use bevy_ecs::prelude::Entity;

use crate::game::base::defences::PatchReach;
use crate::resources::TacticalFxKind;
use crate::tactical::TacticalBattle;
use crate::tuning::{PATCH_SIEGE_HEAL, PATCH_SIEGE_PULSE_ROUNDS};
use crate::*;

impl Game {
    /// Heals every non-besieger body and board structure under a standing
    /// Patch Node, `PATCH_SIEGE_HEAL` per covering node, on every
    /// `PATCH_SIEGE_PULSE_ROUNDS`th round of a siege.
    ///
    /// Called once a round from `tactical_round_upkeep`. The nodes are the
    /// *board* bodies with a `repair`, so one destroyed mid-siege has left
    /// the board and never pulses again. The round number alone decides
    /// whether this is a pulse round: nothing is saved, and reloading on
    /// one cannot pulse twice because the upkeep runs only as the round
    /// wraps. No RNG.
    pub(crate) fn pulse_patch_nodes(&mut self) {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        if battle.siege_pack == 0 || !battle.round.is_multiple_of(PATCH_SIEGE_PULSE_ROUNDS) {
            return;
        }
        let origin = battle.siege_origin;
        let nodes: Vec<PatchReach> = self
            .patch_reaches()
            .into_iter()
            .filter(|r| !battle.cells_of(r.entity).is_empty())
            .collect();
        if nodes.is_empty() {
            return;
        }
        let mut targets: Vec<(Entity, u32)> = battle
            .bodies()
            .map(|(entity, _)| entity)
            // Deliberately the same rule as Shields' `siege_shields_over`.
            .filter(|&e| self.world.get::<Hostile>(e).is_none())
            .filter_map(|e| {
                let cells: Vec<(i32, i32)> = battle
                    .cells_of(e)
                    .into_iter()
                    .map(|(x, y)| (x + origin.0, y + origin.1))
                    .collect();
                let covering = nodes
                    .iter()
                    .filter(|n| crate::reach::covers_any(n.anchor, n.radius, &cells))
                    .count() as u32;
                (covering > 0).then_some((e, covering * PATCH_SIEGE_HEAL))
            })
            .collect();
        targets.sort_by_key(|&(e, _)| e);
        for (target, heal) in targets {
            self.heal_under_patch(target, heal);
        }
    }

    /// A structure on the board mends `Durability`; anything else goes
    /// through `restore_hp`, which cues the heal itself and respects a
    /// `HealBlock`.
    fn heal_under_patch(&mut self, target: Entity, heal: u32) {
        let is_structure = self.world.get::<Structure>(target).is_some();
        if is_structure && let Some(mut durability) = self.world.get_mut::<Durability>(target) {
            let gained = (durability.hp + heal).min(durability.max_hp) - durability.hp;
            durability.hp += gained;
            if gained > 0 {
                self.cue_tactical_fx(target, TacticalFxKind::Heal(gained as i32));
            }
            return;
        }
        self.restore_hp(target, heal as i32);
    }
}
