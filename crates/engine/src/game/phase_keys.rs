//! Reading and changing the player's Phase Keys.
//!
//! A key's effect is **derived on read** from `PhaseKeys::held` and the
//! `PhaseKeyDb`, through the implant readers in `game/implants.rs`
//! (`implant_stats`, `implant_hook_total`, the Dead Man's Switch check); only
//! the percent bonuses live here, applied by `phase_keys::apply_key_pct` at
//! each site that forms a final stat.

use crate::base_grid::BaseGrid;
use crate::components::PhaseKeys;
use crate::notifications::NotificationKind;
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

    /// Whether the party may breach out of `zone`: the one rule the portal
    /// build and the portal step both ask. Zones past the last key are
    /// ungated, and the refusal text is what both surfaces show.
    pub(crate) fn phase_key_gate(&self, zone: u32) -> Result<(), String> {
        if !(1..=crate::tuning::PHASE_KEY_COUNT).contains(&zone) {
            return Ok(());
        }
        let held = self
            .world
            .get::<PhaseKeys>(self.player_entity())
            .is_some_and(|keys| keys.holds(zone));
        match held {
            true => Ok(()),
            false => Err(format!(
                "Needs the Zone {zone} Phase Key. Guardians at the bottom of the Stack carry it."
            )),
        }
    }

    /// A lair guardian has just died: rolls this zone's key. Not eligible
    /// outside zones 1..=`PHASE_KEY_COUNT` or once the key is held. The
    /// roll is `phase_key_roll`, a pure hash, so the shared RNG is never
    /// touched and no seeded roll elsewhere moves.
    pub(crate) fn roll_phase_key(&mut self) {
        let zone = self.world.resource::<ZoneLevel>().0;
        if !(1..=crate::tuning::PHASE_KEY_COUNT).contains(&zone) {
            return;
        }
        let player = self.player_entity();
        let Some(keys) = self.world.get::<PhaseKeys>(player).copied() else {
            return;
        };
        if keys.holds(zone) {
            return;
        }
        let seed = self.world.resource::<BaseGrid>().seed();
        let guaranteed = keys.misses + 1 >= crate::tuning::PHASE_KEY_GUARANTEE_KILLS;
        if guaranteed || phase_key_roll(seed, zone, keys.misses) {
            if self.grant_phase_key(zone) {
                self.announce_phase_key(zone);
            }
        } else if let Some(mut keys) = self.world.get_mut::<PhaseKeys>(player) {
            keys.misses += 1;
        }
    }

    /// The modal screen, the log line and nothing else; the achievement is
    /// `achievement_system`'s to notice.
    fn announce_phase_key(&mut self, zone: u32) {
        let Some(def) = self.world.resource::<PhaseKeyDb>().get(zone).cloned() else {
            return;
        };
        let effect = def.effect.summary();
        self.log_kind(
            MessageKind::Outcome,
            format!("You recover the {}. {}", def.name, effect),
        );
        self.notify_filled(
            NotificationKind::PhaseKeyFound,
            &[("name", &def.name), ("flavour", &def.flavour)],
            Some(effect),
        );
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

/// Whether the `kill_index`th eligible guardian kill of `zone` drops its
/// key. A hash of its inputs and nothing else (splitmix64's finalizer), so it
/// is repeatable from a save and draws nothing from `GameRng`.
pub(crate) fn phase_key_roll(seed: u32, zone: u32, kill_index: u32) -> bool {
    let mut x = (u64::from(seed) << 32 | u64::from(zone)).wrapping_add(
        u64::from(kill_index)
            .wrapping_add(1)
            .wrapping_mul(0x9E37_79B9_7F4A_7C15),
    );
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^= x >> 31;
    let unit = (x >> 11) as f64 / (1u64 << 53) as f64;
    unit < crate::tuning::PHASE_KEY_DROP_CHANCE
}
