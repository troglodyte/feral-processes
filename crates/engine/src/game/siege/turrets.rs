//! Turrets: a `StructureDef` field resolved at a point in a siege, holding
//! no initiative slot and no body — see `structures::TurretDef`.
//!
//! `Game::fire_turrets`, called once a round from `tactical::turn::
//! tactical_round_upkeep`. This file starts with the half a turret does
//! *outside* a siege too: contributing to the off-screen defence
//! `Game::resolve_siege_offscreen` (Task 4) prices a pack against.

use bevy_ecs::prelude::Entity;

use crate::structures::TurretDef;
use crate::tactical::TacticalBattle;
use crate::tactical::reach;
use crate::*;

/// The sum of `damage` over every deployed structure whose def declares a
/// turret — `Game::total_raid_defense`'s shape, and a free function rather
/// than a `Game` method because Task 4's shortfall formula and Task 15's
/// fire both read it as one term among several, not as a verb on `Game`.
pub(crate) fn turret_defense(game: &Game) -> u32 {
    let structure_db = game.world.resource::<StructureDb>();
    game.world
        .iter_entities()
        .filter_map(|e| e.get::<Structure>())
        .filter_map(|s| structure_db.get(&s.kind))
        .filter_map(|def| def.turret.as_ref())
        .map(|turret| turret.damage)
        .sum()
}

impl Game {
    /// `structure`'s `TurretDef`, resolved live by its `Structure::kind`
    /// against `StructureDb` — `ActiveContract`'s precedent for resolving a
    /// def by id rather than caching a copy, applied to the one other
    /// device standing in the world on its own initiative. `None` for a
    /// structure with no `turret:` field, or one already despawned.
    fn turret_def_of(&self, structure: Entity) -> Option<TurretDef> {
        let kind = self.world.get::<Structure>(structure)?.kind.clone();
        self.world.resource::<StructureDb>().get(&kind)?.turret
    }

    /// Every turret on the board fires once, at the nearest hostile in
    /// range with line of sight — a property of the structure, resolved
    /// here rather than given an initiative slot or a body.
    ///
    /// Called once a round, from `tactical::turn::tactical_round_upkeep`,
    /// ahead of that round's own upkeep and reap: a turret's kill is swept
    /// by the same reap pass an ordinary swing's would be.
    pub(crate) fn fire_turrets(&mut self) {
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        let turrets: Vec<Entity> = battle
            .bodies()
            .map(|(entity, _)| entity)
            .filter(|&entity| self.turret_def_of(entity).is_some())
            .collect();
        for turret in turrets {
            self.fire_one_turret(turret);
        }
    }

    /// `turret` fires at the nearest hostile in range with line of sight,
    /// through `Game::apply_damage` — the target is a creature, never a
    /// structure, so this is not a second `damage_structure` caller.
    fn fire_one_turret(&mut self, turret: Entity) {
        let Some(def) = self.turret_def_of(turret) else {
            return;
        };
        let Some(battle) = self.world.get_resource::<TacticalBattle>() else {
            return;
        };
        let turret_cells = battle.cells_of(turret);
        let target = battle
            .bodies()
            .filter(|&(entity, _)| entity != turret)
            .filter(|&(entity, _)| self.world.get::<Hostile>(entity).is_some())
            .filter_map(|(entity, _)| {
                let cells = battle.cells_of(entity);
                let dist = reach::gap(&turret_cells, &cells);
                (dist <= def.range
                    && turret_cells.iter().any(|&a| {
                        cells
                            .iter()
                            .any(|&b| reach::line_of_sight(&battle.board, a, b))
                    }))
                .then_some((dist, entity))
            })
            // Tie-broken by `Entity`, a total order over an otherwise equal
            // distance — `assembler_system`'s reason: a `HashMap`-free walk
            // is what keeps two equally near hostiles resolving the same
            // way in a seeded fight.
            .min_by_key(|&(dist, entity)| (dist, entity));
        let Some((_, target)) = target else {
            return;
        };
        self.apply_damage(target, def.damage as i32);
    }
}
