//! Turrets: a `StructureDef` field resolved at a point in a siege, holding
//! no initiative slot and no body — see `structures::TurretDef`.
//!
//! Task 15 adds `Game::fire_turrets`, called once a round from the tactical
//! turn. This file starts with the half a turret does *outside* a siege
//! too: contributing to the off-screen defence `Game::resolve_siege_offscreen`
//! (Task 4) prices a pack against.

use crate::*;

/// The sum of `damage` over every deployed structure whose def declares a
/// turret — `Game::total_raid_defense`'s shape, and a free function rather
/// than a `Game` method because Task 4's shortfall formula and Task 15's
/// fire both read it as one term among several, not as a verb on `Game`.
///
/// `#[allow(dead_code)]`: only the test suite calls this between this task
/// and Task 4, which wires it into `Game::resolve_siege_offscreen`'s
/// shortfall — the allow is removed in that commit.
#[allow(dead_code)]
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
