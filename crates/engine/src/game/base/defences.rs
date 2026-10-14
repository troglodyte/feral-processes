//! Which defences are armed, and what a sweep or siege costs them.
//!
//! A defence that declares `StructureDef::supply` counts only while its
//! input holds one, and spends one per event it takes part in. The garrison
//! a friendly neighbour stations is not a structure and is never gated here.

use crate::base_ledger::ConsumeSource;
use crate::structures::StructureDef;
use crate::*;

/// One armed defence: the structure and the two numbers it adds.
pub(crate) struct ArmedDefence {
    pub entity: Entity,
    pub raid_defense: u32,
    pub turret_damage: u32,
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
                })
            })
            .collect();
        armed.sort_by_key(|d| d.entity);
        armed
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
            self.spend_one_supply(entity);
        }
    }

    /// Takes one unit of `entity`'s supply out of its input and writes it to
    /// the ledger. Returns whether it held one.
    pub(crate) fn spend_one_supply(&mut self, entity: Entity) -> bool {
        let Some(supply) = self
            .world
            .get::<Structure>(entity)
            .and_then(|s| self.world.resource::<StructureDb>().get(&s.kind))
            .and_then(|def| def.supply.clone())
        else {
            return false;
        };
        let Some(mut stock) = self.world.get_mut::<Stock>(entity) else {
            return false;
        };
        let held = stock.input.get(&supply).copied().unwrap_or(0);
        if held == 0 {
            return false;
        }
        if held == 1 {
            stock.input.remove(&supply);
        } else {
            stock.input.insert(supply.clone(), held - 1);
        }
        self.note_consumed(&supply, 1, ConsumeSource::Defence);
        true
    }
}
