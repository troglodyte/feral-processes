//! Reading the player's installed implants.
//!
//! Every effect an implant has is read through these helpers at the one seam
//! that owns it (`docs/superpowers/plans/2026-10-06-player-implants.md`, P2),
//! and every helper goes through `ImplantDb::get`, so an id whose def is
//! missing contributes nothing.

use crate::implants::ImplantId;
use crate::implants::{
    ImplantDb, ImplantDef, ImplantDownside, ImplantHook, ImplantSignature, ImplantStats,
};
use crate::items::ItemId;
use crate::statuses::StatusId;
use crate::tuning::{
    DEAD_MANS_SWITCH_POWER, HUNGER_DECAY_PER_TICK, IMPLANT_DRAIN_PER_LOAD,
    IMPLANT_REMOVAL_FRAGMENTS_PER_LOAD, REJECTION_STATUS_DURATION, REJECTION_STATUSES,
};
use crate::views::{ImplantView, InstallableImplantRow, InstalledImplantRow};
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

    /// The player's installed Load against the cap for their level, as
    /// `(load, cap)`. The cheap read the HUD and the points screen take per
    /// frame, instead of building a whole `implant_view`.
    pub fn neural_load(&self) -> (u32, u32) {
        let level = self
            .world
            .get::<Experience>(self.player_entity())
            .map_or(1, |e| e.level);
        (self.implant_load(), crate::implants::load_cap(level))
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

    /// Whether a Splice Rig touches the party's cell — what the map key
    /// opens the screen on. `adjacent_teardown_rigs`' rule, minus the Vec:
    /// nothing here needs to tell two rigs apart.
    pub fn adjacent_splice_rig(&self) -> bool {
        let Some((px, py)) = self.base_pos() else {
            return false;
        };
        self.world.iter_entities().any(|e| {
            let (Some(s), Some(p)) = (e.get::<Structure>(), e.get::<Position>()) else {
                return false;
            };
            s.kind == SPLICE_RIG
                && crate::game::base::collect::ORTHOGONAL
                    .iter()
                    .any(|(dx, dy)| (p.x, p.y) == (px + dx, py + dy))
        })
    }

    /// Everything the Splice Rig screen shows: Load against the cap, what is
    /// built in (including ids whose def has gone missing, so they can be
    /// cut out), and the implant items in the pack.
    pub fn implant_view(&self) -> ImplantView {
        let player = self.player_entity();
        let level = self.world.get::<Experience>(player).map_or(1, |e| e.level);
        let load = self.implant_load();
        let cap = crate::implants::load_cap(level);
        let db = self.world.resource::<ImplantDb>();
        let installed_ids: Vec<ImplantId> = self
            .world
            .get::<crate::components::Implants>(player)
            .map(|held| held.installed.clone())
            .unwrap_or_default();
        let installed = installed_ids
            .into_iter()
            .map(|id| match db.get(&id) {
                Some(def) => InstalledImplantRow {
                    name: def.name.clone(),
                    known: true,
                    description: def.description.clone(),
                    load: def.load,
                    upkeep: upkeep_of(def.load),
                    downside: self.downside_text(def),
                    removal_fragments: removal_price(def.load),
                    id,
                },
                None => InstalledImplantRow {
                    name: id.as_str().to_string(),
                    known: false,
                    description: String::new(),
                    load: 0,
                    upkeep: 0.0,
                    downside: None,
                    removal_fragments: 0,
                    id,
                },
            })
            .collect();
        let inventory = self.world.get::<Inventory>(player);
        let mut installable: Vec<InstallableImplantRow> = self
            .world
            .resource::<crate::items_db::ItemDb>()
            .all()
            .filter_map(|item| {
                let def = db.get(item.implant.as_ref()?)?;
                let count = inventory.map_or(0, |inv| inv.count(&item.id));
                (count > 0).then(|| InstallableImplantRow {
                    item: item.id.clone(),
                    name: item.name.clone(),
                    description: def.description.clone(),
                    load: def.load,
                    upkeep: upkeep_of(def.load),
                    downside: self.downside_text(def),
                    count,
                })
            })
            .collect();
        installable.sort_by(|a, b| a.item.as_str().cmp(b.item.as_str()));
        ImplantView {
            load,
            cap,
            overload: crate::implants::overload(load, cap),
            fragments: inventory.map_or(0, |inv| inv.count(&ItemId::from(ids::CORE_FRAGMENT))),
            installed,
            installable,
        }
    }

    /// Builds `item`'s implant into the player, consuming the item. Every
    /// refusal sits above the payment, so a refused install spends nothing.
    /// Going past the Load cap is allowed (rejection is the price); callers
    /// that want to warn compare `implant_view` first.
    pub fn install_implant(&mut self, item: &ItemId) -> Result<(), String> {
        self.splicing_allowed()?;
        let implant = self
            .item_def(item)
            .and_then(|def| def.implant)
            .ok_or_else(|| format!("{} is not an implant.", self.item_name(item)))?;
        if self.world.resource::<ImplantDb>().get(&implant).is_none() {
            return Err(format!(
                "{} has no implant to splice.",
                self.item_name(item)
            ));
        }
        let player = self.player_entity();
        let held = self.world.get::<crate::components::Implants>(player);
        if held.is_some_and(|held| held.installed.contains(&implant)) {
            return Err(format!("{} is already installed.", self.item_name(item)));
        }
        self.pay_items(&[(item.clone(), 1)])?;
        if let Some(mut held) = self.world.get_mut::<crate::components::Implants>(player) {
            held.installed.push(implant);
        }
        self.recompute_derived(player);
        Ok(())
    }

    /// Cuts `implant` out and hands its item back, for
    /// `IMPLANT_REMOVAL_FRAGMENTS_PER_LOAD` `core_fragment`s per Load. An id
    /// whose def is missing has no Load, so it comes out free: charging for
    /// something the game can no longer price would strand it. An item that
    /// still names the id is handed back all the same; with no such item
    /// nothing is.
    pub fn remove_implant(&mut self, implant: &ImplantId) -> Result<(), String> {
        self.splicing_allowed()?;
        let player = self.player_entity();
        let held = self
            .world
            .get::<crate::components::Implants>(player)
            .is_some_and(|held| held.installed.contains(implant));
        if !held {
            return Err("That implant isn't installed.".into());
        }
        let load = self
            .world
            .resource::<ImplantDb>()
            .get(implant)
            .map_or(0, |def| def.load);
        let price = removal_price(load);
        if price > 0 {
            self.pay_items(&[(ItemId::from(ids::CORE_FRAGMENT), price)])?;
        }
        if let Some(mut held) = self.world.get_mut::<crate::components::Implants>(player) {
            held.installed.retain(|id| id != implant);
        }
        let carrier = self
            .world
            .resource::<crate::items_db::ItemDb>()
            .all()
            .find(|item| item.implant.as_ref() == Some(implant))
            .map(|item| item.id.clone());
        if let Some(item) = carrier {
            self.grant_loot(item, 1, crate::base_ledger::LootSource::Refund);
        }
        self.recompute_derived(player);
        // A removed Overclock Spine narrows the routine row.
        self.fit_routines_to_slots(player);
        Ok(())
    }

    /// The gate both doors share: no game over, no battle, a rig standing.
    /// Like the Mod Bench, one anywhere in the base is enough.
    fn splicing_allowed(&self) -> Result<(), String> {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return Err("Can't do that right now.".into());
        }
        if !self.has_structure(SPLICE_RIG) {
            return Err("Build a Splice Rig first.".into());
        }
        Ok(())
    }

    /// One line on what `def`'s downside does, for the screen.
    fn downside_text(&self, def: &ImplantDef) -> Option<String> {
        Some(match def.downside.as_ref()? {
            ImplantDownside::TraceRise(pct) => format!("Trace rises {pct}% faster."),
            ImplantDownside::BattleStartStatus(status, chance) => {
                let db = self.world.resource::<crate::statuses::StatusDb>();
                format!(
                    "{}% chance to start a fight {}.",
                    (chance * 100.0).round(),
                    db.name_of(status)
                )
            }
        })
    }
}

/// The structure install and remove require.
const SPLICE_RIG: &str = "splice_rig";

/// Extra Power per tick `load` adds, before perks: the share of
/// `HUNGER_DECAY_PER_TICK` that `drain_factor` multiplies in.
fn upkeep_of(load: u32) -> f32 {
    HUNGER_DECAY_PER_TICK * IMPLANT_DRAIN_PER_LOAD * load as f32
}

fn removal_price(load: u32) -> u32 {
    load * IMPLANT_REMOVAL_FRAGMENTS_PER_LOAD
}
