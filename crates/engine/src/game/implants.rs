//! Reading the player's installed implants.
//!
//! Every effect an implant has is read through these helpers at the one seam
//! that owns it (`docs/superpowers/plans/2026-10-06-player-implants.md`, P2),
//! and every helper goes through `ImplantDb::get`, so an id whose def is
//! missing contributes nothing.

use crate::implants::{
    ImplantDb, ImplantDef, ImplantDownside, ImplantHook, ImplantSignature, ImplantStats,
};
use crate::statuses::StatusId;
use crate::tuning::{DEAD_MANS_SWITCH_POWER, REJECTION_STATUS_DURATION, REJECTION_STATUSES};
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

    /// Rolls the statuses a fight opens with: a rejection when the player is
    /// over the Load cap, and each installed `BattleStartStatus`. **Rolls only
    /// when something can land**, so a player with no implants (or none over
    /// the cap and none with a downside) draws nothing from `GameRng`.
    pub(crate) fn roll_implant_battle_start(&mut self) {
        let player = self.player_entity();
        let level = self
            .world
            .get::<Experience>(player)
            .map_or(1, |exp| exp.level);
        let over = crate::implants::overload(self.implant_load(), crate::implants::load_cap(level));
        let mut landed: Vec<StatusId> = Vec::new();
        if over > 0 {
            let pool: Vec<StatusId> = {
                let db = self.world.resource::<crate::statuses::StatusDb>();
                REJECTION_STATUSES
                    .iter()
                    .map(|id| StatusId::from(*id))
                    .filter(|id| db.contains(id))
                    .collect()
            };
            if !pool.is_empty() {
                let mut rng = self.world.resource_mut::<GameRng>();
                if rng.0.random_bool(crate::implants::rejection_chance(over)) {
                    landed.push(pool[rng.0.random_range(0..pool.len())].clone());
                }
            }
        }
        let downsides: Vec<(StatusId, f32)> = self
            .implant_defs(player)
            .into_iter()
            .filter_map(|def| match &def.downside {
                Some(ImplantDownside::BattleStartStatus(id, chance)) => Some((id.clone(), *chance)),
                _ => None,
            })
            .collect();
        for (id, chance) in downsides {
            let hit = self
                .world
                .resource_mut::<GameRng>()
                .0
                .random_bool(chance as f64);
            if hit {
                landed.push(id);
            }
        }
        for id in landed {
            self.arm_status(player, &id, REJECTION_STATUS_DURATION, 0);
            self.log_status_landing(&id, "You", None);
        }
    }

    /// The Dead Man's Switch: a hit that would take the player from above 0 to
    /// 0 leaves them at 1 instead, once per battle, for
    /// `DEAD_MANS_SWITCH_POWER`. Returns the damage that actually lands.
    ///
    /// Only `apply_damage` calls this, so `kill_outright` (dying inside rock)
    /// and every non-battle source still kill.
    pub(crate) fn dead_mans_switch(&mut self, target: Entity, dealt: i32) -> i32 {
        let player = self.player_entity();
        if target != player || !self.has_active_battle() {
            return dealt;
        }
        let hp = self.world.get::<Stats>(player).map_or(0, |s| s.hp);
        if hp <= 0 || dealt < hp {
            return dealt;
        }
        let installed = self
            .implant_defs(player)
            .iter()
            .any(|def| def.signature == Some(ImplantSignature::DeadMansSwitch));
        let funded = self
            .world
            .get::<PowerReserve>(player)
            .is_some_and(|power| power.holds(DEAD_MANS_SWITCH_POWER));
        if !installed || !funded || !self.claim_dead_mans_switch() {
            return dealt;
        }
        if let Some(mut power) = self.world.get_mut::<PowerReserve>(player) {
            power.spend(DEAD_MANS_SWITCH_POWER);
        }
        self.log_kind(
            MessageKind::Outcome,
            "Your Dead Man's Switch fires. You hold at 1 Integrity.",
        );
        hp - 1
    }

    /// Marks the switch spent on whichever battle model is open, and says
    /// whether it was still available.
    fn claim_dead_mans_switch(&mut self) -> bool {
        if let Some(mut battle) = self.world.get_resource_mut::<BattleState>() {
            return !std::mem::replace(&mut battle.dead_mans_switch_spent, true);
        }
        if let Some(mut battle) = self
            .world
            .get_resource_mut::<crate::tactical::TacticalBattle>()
        {
            return !std::mem::replace(&mut battle.dead_mans_switch_spent, true);
        }
        false
    }
}
