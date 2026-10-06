//! Reading the player's installed implants.
//!
//! Every effect an implant has is read through these helpers at the one seam
//! that owns it (`docs/superpowers/plans/2026-10-06-player-implants.md`, P2),
//! and every helper goes through `ImplantDb::get`, so an id whose def is
//! missing contributes nothing.

use crate::implants::{ImplantDb, ImplantDef, ImplantDownside, ImplantHook, ImplantStats};
use crate::*;

impl Game {
    /// The defs of everything `entity` has installed. Empty for an entity with
    /// no `Implants`, which is every creature but the player.
    fn implant_defs(&self, entity: Entity) -> Vec<&ImplantDef> {
        let Some(held) = self.world.get::<crate::components::Implants>(entity) else {
            return Vec::new();
        };
        let db = self.world.resource::<ImplantDb>();
        held.installed.iter().filter_map(|id| db.get(id)).collect()
    }

    /// `entity`'s installed stat deltas, summed.
    pub(crate) fn implant_stats(&self, entity: Entity) -> ImplantStats {
        self.implant_defs(entity)
            .into_iter()
            .fold(ImplantStats::default(), |acc, def| ImplantStats {
                max_hp: acc.max_hp + def.stats.max_hp,
                atk: acc.atk + def.stats.atk,
                mitigation: acc.mitigation + def.stats.mitigation,
                max_power: acc.max_power + def.stats.max_power,
                crit: acc.crit + def.stats.crit,
                status_resist: acc.status_resist + def.stats.status_resist,
                decompiler: acc.decompiler + def.stats.decompiler,
                accuracy: acc.accuracy + def.stats.accuracy,
                evasion: acc.evasion + def.stats.evasion,
            })
    }

    /// Accuracy and evasion as `(accuracy, evasion)` — gear plus implants.
    /// Neither is baked into `Stats` (`gear_bonus`'s accuracy and evasion are
    /// read live, see `crafting.rs`), so every reader of the pair goes through
    /// this one rather than `gear_bonus`.
    pub(crate) fn hit_bonus(&self, entity: Entity) -> (i32, i32) {
        let gear = self.gear_bonus(entity);
        let implants = self.implant_stats(entity);
        (
            gear.accuracy + implants.accuracy,
            gear.evasion + implants.evasion,
        )
    }

    /// The player's installed hooks the caller picks a value out of, summed.
    pub(crate) fn implant_hook_total(&self, pick: impl Fn(&ImplantHook) -> Option<i32>) -> i32 {
        self.implant_defs(self.player_entity())
            .into_iter()
            .flat_map(|def| def.hooks.iter())
            .filter_map(pick)
            .sum()
    }

    /// The player's installed Neural Load.
    pub(crate) fn implant_load(&self) -> u32 {
        let player = self.player_entity();
        self.world
            .get::<crate::components::Implants>(player)
            .map_or(0, |held| {
                crate::implants::load_of(held, self.world.resource::<ImplantDb>())
            })
    }

    /// Net percent change on Trace the player's implants make: downsides
    /// raise it, `TraceDamp` hooks lower it.
    pub(crate) fn implant_trace_pct(&self) -> i32 {
        let rise: i32 = self
            .implant_defs(self.player_entity())
            .into_iter()
            .filter_map(|def| match def.downside {
                Some(ImplantDownside::TraceRise(pct)) => Some(pct),
                _ => None,
            })
            .sum();
        rise - self.implant_hook_total(|h| match h {
            ImplantHook::TraceDamp(pct) => Some(*pct),
            _ => None,
        })
    }
}
