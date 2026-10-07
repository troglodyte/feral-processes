//! Reading and changing the player's Phase Keys.
//!
//! A key's effect is **derived on read** from `PhaseKeys::held` and the
//! `PhaseKeyDb`, through the implant readers in `game/implants.rs`
//! (`implant_stats`, `implant_hook_total`, the Dead Man's Switch check); only
//! the percent bonuses live here, applied by `phase_keys::apply_key_pct` at
//! each site that forms a final stat.

use crate::components::PhaseKeys;
use crate::phase_keys::{PhaseKeyDb, PhaseKeyDef, StatPct};
use crate::views::{HeldPhaseKey, PhaseKeySlot, PhaseKeysView};
use crate::*;

impl Game {
    /// The defs of the keys `entity` holds. Empty for any entity with no
    /// `PhaseKeys`, which is every creature but the player.
    pub(crate) fn key_defs(&self, entity: Entity) -> Vec<&PhaseKeyDef> {
        let Some(keys) = self.world.get::<PhaseKeys>(entity) else {
            return Vec::new();
        };
        let db = self.world.resource::<PhaseKeyDb>();
        keys.zones().filter_map(|zone| db.get(zone)).collect()
    }

    /// The percent bonuses of every key `entity` holds, summed.
    pub(crate) fn key_pct(&self, entity: Entity) -> StatPct {
        self.key_defs(entity)
            .into_iter()
            .fold(StatPct::default(), |acc, def| acc.sum(def.effect.stat_pct))
    }

    /// Marks `zone`'s key held and re-derives the player's stats. Returns
    /// whether it was newly gained. Nothing outside the key drop and the
    /// savetool warp may call this.
    #[cfg_attr(
        not(test),
        expect(dead_code, reason = "its first caller is the key drop")
    )]
    pub(crate) fn grant_phase_key(&mut self, zone: u32) -> bool {
        if !(1..=crate::tuning::PHASE_KEY_COUNT).contains(&zone) {
            return false;
        }
        let player = self.player_entity();
        let Some(mut keys) = self.world.get_mut::<PhaseKeys>(player) else {
            return false;
        };
        if keys.holds(zone) {
            return false;
        }
        keys.held |= 1 << (zone - 1);
        self.recompute_derived(player);
        true
    }

    /// The Phase Keys tab's whole picture: ten slots, each held or missing.
    pub fn phase_keys(&self) -> PhaseKeysView {
        let keys = self
            .world
            .get::<PhaseKeys>(self.player_entity())
            .copied()
            .unwrap_or_default();
        let db = self.world.resource::<PhaseKeyDb>();
        let slots = db
            .all()
            .iter()
            .map(|def| PhaseKeySlot {
                zone: def.zone,
                held: keys.holds(def.zone).then(|| HeldPhaseKey {
                    name: def.name.clone(),
                    flavour: def.flavour.clone(),
                    effect: def.effect.summary(),
                }),
            })
            .collect();
        PhaseKeysView {
            slots,
            held_count: keys.count(),
            story_complete: keys.story_complete,
        }
    }
}
