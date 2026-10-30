//! Which defences are armed, and what a sweep or siege costs them.
//!
//! A defence that declares `StructureDef::supply` counts only while its
//! input holds one, and spends one per event it takes part in. The garrison
//! a friendly neighbour stations is not a structure and is never gated here.

use crate::base_ledger::ConsumeSource;
use crate::structures::StructureDef;
use crate::*;

/// Where a circled defence stands and how far it reaches.
#[derive(Clone, Copy)]
pub(crate) struct ShieldReach {
    pub anchor: (i32, i32),
    pub radius: u32,
    /// What it takes off each hit it covers: the def's `raid_defense`.
    pub cut: u32,
}

/// One armed defence: the structure and the two numbers it adds. `shield` is
/// `None` for a base-wide defence.
pub(crate) struct ArmedDefence {
    pub entity: Entity,
    pub raid_defense: u32,
    pub turret_damage: u32,
    pub shield: Option<ShieldReach>,
}

impl Game {
    /// Whether `entity` holds the supply `def` spends. A def that names no
    /// supply is always armed, so a modded defence keeps its old behaviour.
    pub(crate) fn defence_is_armed(&self, entity: Entity, def: &StructureDef) -> bool {
        let Some(supply) = def.supply.as_ref() else {
            return true;
        };
        self.world
            .get::<Stock>(entity)
            .is_some_and(|stock| stock.input.get(supply).copied().unwrap_or(0) > 0)
    }

    /// Every deployed defence that is armed right now, in entity order so a
    /// spend walks them the same way every run.
    pub(crate) fn armed_defences(&self) -> Vec<ArmedDefence> {
        let db = self.world.resource::<StructureDb>();
        let mut armed: Vec<ArmedDefence> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let def = db.get(&e.get::<Structure>()?.kind)?;
                (def.is_defence() && self.defence_is_armed(e.id(), def)).then(|| ArmedDefence {
                    entity: e.id(),
                    raid_defense: def.raid_defense,
                    turret_damage: def.turret.as_ref().map_or(0, |t| t.damage),
                    shield: self.shield_reach(e.id(), def),
                })
            })
            .collect();
        armed.sort_by_key(|d| d.entity);
        armed
    }

    /// The circle `entity` covers, or `None` for a defence with no
    /// `shield_radius` (base-wide) or one that has no `Position`.
    fn shield_reach(&self, entity: Entity, def: &StructureDef) -> Option<ShieldReach> {
        let radius_def = def.shield_radius.as_ref()?;
        let pos = self.world.get::<Position>(entity)?;
        let tier = self.world.get::<StructureTier>(entity).map_or(1, |t| t.0);
        Some(ShieldReach {
            anchor: (pos.x, pos.y),
            radius: crate::shields::radius_at(radius_def, tier),
            cut: def.raid_defense,
        })
    }

    /// The cells a structure stands on; empty when it has no `Position`.
    pub(crate) fn structure_cells(&self, e: Entity) -> Vec<(i32, i32)> {
        match self.world.get::<Position>(e) {
            Some(p) => {
                crate::tactical::footprint_cells_at((p.x, p.y), self.structure_footprint_of(e))
            }
            None => Vec::new(),
        }
    }

    /// Whether `d` protects a structure standing on `cells`. A base-wide
    /// defence always does.
    pub(crate) fn defence_covers(&self, d: &ArmedDefence, cells: &[(i32, i32)]) -> bool {
        d.shield
            .is_none_or(|s| crate::shields::covers_any(s.anchor, s.radius, cells))
    }

    /// Every deployed Shield, armed or not, sorted by anchor.
    pub fn view_shields(&self) -> Vec<ShieldView> {
        let db = self.world.resource::<StructureDb>();
        let mut shields: Vec<ShieldView> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let def = db.get(&e.get::<Structure>()?.kind)?;
                let reach = self.shield_reach(e.id(), def)?;
                let radius_def = def.shield_radius.as_ref()?;
                let tier = e.get::<StructureTier>().map_or(1, |t| t.0);
                let next_radius = def
                    .upgrade
                    .as_ref()
                    .filter(|u| tier < self.upgrade_ceiling(u))
                    .map(|_| crate::shields::radius_at(radius_def, tier + 1));
                Some(ShieldView {
                    entity: e.id(),
                    anchor: reach.anchor,
                    radius: reach.radius,
                    next_radius,
                    armed: self.defence_is_armed(e.id(), def),
                    raid_defense: def.raid_defense,
                })
            })
            .collect();
        shields.sort_by_key(|s| (s.anchor, s.entity));
        shields
    }

    /// The radius a Shield of `kind` would have at `tier`, for the deploy
    /// preview. `None` for a kind with no circle.
    pub fn shield_radius_for(&self, kind: &str, tier: u32) -> Option<u32> {
        let def = self.world.resource::<StructureDb>().get(kind)?;
        Some(crate::shields::radius_at(def.shield_radius.as_ref()?, tier))
    }

    /// Queues a flash of a Shield's circle: on the field, and on the siege
    /// board too (shifted by the siege's origin) while a siege is fought.
    pub(crate) fn cue_shield_flash(&mut self, anchor: (i32, i32), radius: u32) {
        let kind = crate::resources::EffectKind::Shielded { radius };
        self.world
            .resource_mut::<crate::resources::EffectQueue>()
            .push(anchor, kind);
        if !self.siege_running() {
            return;
        }
        let origin = self
            .world
            .resource::<crate::tactical::TacticalBattle>()
            .siege_origin;
        self.world
            .resource_mut::<crate::resources::TacticalFxQueue>()
            .push(
                (anchor.0 - origin.0, anchor.1 - origin.1),
                crate::resources::TacticalFxKind::Shielded { radius },
            );
    }

    /// The armed Shields whose circle covers `target` while a siege is
    /// fought; empty for any other fight and for a besieger.
    ///
    /// "Enemy hit" is implemented as "damage to a non-hostile body during a
    /// siege": every source of that damage on a siege board is the
    /// besiegers' (swings, routines, statuses they applied), and a tantrum
    /// never runs inside a battle.
    pub(crate) fn siege_shields_over(&self, target: Entity) -> Vec<ShieldReach> {
        let Some(battle) = self.world.get_resource::<crate::tactical::TacticalBattle>() else {
            return Vec::new();
        };
        if battle.siege_pack == 0 || self.world.get::<Hostile>(target).is_some() {
            return Vec::new();
        }
        let origin = battle.siege_origin;
        let mut cells: Vec<(i32, i32)> = battle
            .cells_of(target)
            .into_iter()
            .map(|(x, y)| (x + origin.0, y + origin.1))
            .collect();
        if cells.is_empty() {
            cells = self.structure_cells(target);
        }
        self.armed_defences()
            .into_iter()
            .filter_map(|d| d.shield)
            .filter(|s| crate::shields::covers_any(s.anchor, s.radius, &cells))
            .collect()
    }

    /// Records that these Shields softened a hit this siege (they spend a
    /// coil when it ends) and flashes each circle.
    pub(crate) fn note_shields_softened(&mut self, shields: &[ShieldReach]) {
        for s in shields {
            if let Some(mut battle) = self
                .world
                .get_resource_mut::<crate::tactical::TacticalBattle>()
            {
                battle.shields_softened.insert(s.anchor);
            }
            self.cue_shield_flash(s.anchor, s.radius);
        }
    }

    /// A hit on a siege-board structure, less what the Shields over it cut:
    /// never below 1 for a landed hit. Records and flashes each Shield.
    pub(crate) fn shielded_structure_damage(&mut self, structure: Entity, dmg: u32) -> u32 {
        let shields = self.siege_shields_over(structure);
        if shields.is_empty() || dmg == 0 {
            return dmg;
        }
        self.note_shields_softened(&shields);
        let cut: u32 = shields.iter().map(|s| s.cut).sum();
        dmg.saturating_sub(cut).max(1)
    }

    /// At a siege's end each Shield that softened a hit and still stands
    /// spends its one coil. A Shield destroyed since has no stock to spend
    /// from, so it is simply not found.
    pub(crate) fn spend_softening_shields(&mut self) {
        let Some(battle) = self.world.get_resource::<crate::tactical::TacticalBattle>() else {
            return;
        };
        if battle.siege_pack == 0 || battle.shields_softened.is_empty() {
            return;
        }
        let anchors = battle.shields_softened.clone();
        let db = self.world.resource::<StructureDb>();
        let mut payers: Vec<Entity> = self
            .world
            .iter_entities()
            .filter(|e| {
                let is_shield = e
                    .get::<Structure>()
                    .and_then(|s| db.get(&s.kind))
                    .is_some_and(|d| d.shield_radius.is_some());
                is_shield
                    && e.get::<Position>()
                        .is_some_and(|p| anchors.contains(&(p.x, p.y)))
            })
            .map(|e| e.id())
            .collect();
        payers.sort();
        for entity in payers {
            crate::game::base::stock::spend_supply(self, entity, ConsumeSource::Defence);
        }
    }

    /// What cuts a sweep against `target`: every armed defence that covers
    /// its footprint, plus the garrison. The one sum the sweep reads, so a
    /// Shield out of range adds nothing.
    pub(crate) fn sweep_defense_for(&self, target: Entity) -> u32 {
        let cells = self.structure_cells(target);
        let structures: u32 = self
            .armed_defences()
            .iter()
            .filter(|d| self.defence_covers(d, &cells))
            .map(|d| d.raid_defense)
            .sum();
        structures + self.garrison_defense()
    }

    /// The sweep defence the structures add, armed ones only.
    pub(crate) fn structure_raid_defense(&self) -> u32 {
        self.armed_defences().iter().map(|d| d.raid_defense).sum()
    }

    /// Takes one coil from each armed defence that `contributes` to the
    /// event, and returns how many paid. **One per event, not per point**: a
    /// turret authors `raid_defense: 1` and `damage: 8`, and charging the
    /// points would make one off-screen siege cost it nine.
    pub(crate) fn spend_defence_supply(&mut self, contributes: impl Fn(&ArmedDefence) -> bool) {
        let payers: Vec<Entity> = self
            .armed_defences()
            .iter()
            .filter(|d| contributes(d))
            .map(|d| d.entity)
            .collect();
        for entity in payers {
            crate::game::base::stock::spend_supply(self, entity, ConsumeSource::Defence);
        }
    }
}
