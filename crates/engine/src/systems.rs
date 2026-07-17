use bevy_ecs::prelude::*;
use rand::RngExt;

use crate::components::{
    Experience, Inventory, Needs, PassiveProcessor, Player, Position, ResourceNode, Stats,
    Structure, Tamed, Task, WanderAi,
};
use crate::progression;
use crate::resources::{GameRng, MessageLog};
use crate::structures::StructureDb;
use crate::world::WorldMap;

/// XP a tamed creature earns for each completed gather cycle.
const WORK_XP_PER_CYCLE: u32 = 5;

const HUNGER_DECAY_PER_TICK: f32 = 0.15;
const FATIGUE_DECAY_PER_TICK: f32 = 0.08;

/// One tick of hunger/fatigue decay; pulled out of the system so the rates
/// are unit-testable without spinning up an ECS `World`.
pub fn decay_needs(hunger: f32, fatigue: f32) -> (f32, f32) {
    (
        (hunger - HUNGER_DECAY_PER_TICK).max(0.0),
        (fatigue - FATIGUE_DECAY_PER_TICK).max(0.0),
    )
}

pub fn needs_decay_system(mut query: Query<(&mut Needs, &mut Stats), With<Player>>, mut log: ResMut<MessageLog>) {
    for (mut needs, mut stats) in &mut query {
        let was_starving = needs.hunger <= 0.0;
        let (hunger, fatigue) = decay_needs(needs.hunger, needs.fatigue);
        needs.hunger = hunger;
        needs.fatigue = fatigue;
        if needs.hunger <= 0.0 {
            stats.hp -= 1;
            if !was_starving {
                log.push("Your power reserves are critical!");
            }
        }
    }
}

pub fn wander_ai_system(
    mut query: Query<(&mut Position, &mut WanderAi), Without<Player>>,
    mut world: ResMut<WorldMap>,
    mut rng: ResMut<GameRng>,
) {
    for (mut pos, mut ai) in &mut query {
        if ai.cooldown > 0 {
            ai.cooldown -= 1;
            continue;
        }
        ai.cooldown = rng.0.random_range(2..6);
        let dx = rng.0.random_range(-1..=1);
        let dy = rng.0.random_range(-1..=1);
        if dx == 0 && dy == 0 {
            continue;
        }
        let (nx, ny) = (pos.x + dx, pos.y + dy);
        if world.tile(nx, ny).walkable {
            pos.x = nx;
            pos.y = ny;
        }
    }
}

/// Generic job progression: any entity with a `Task` advances it once per
/// tick against its `target`; on completion the producing node hands a unit
/// of resource to the worker's owner. The same loop would drive future
/// colonist-style jobs, not just base-building work.
pub fn task_progress_system(
    mut tasks: Query<(&mut Task, &Tamed, &mut Experience, &mut Stats)>,
    mut nodes: Query<&mut ResourceNode>,
    mut inventories: Query<&mut Inventory>,
    mut log: ResMut<MessageLog>,
) {
    for (mut task, tamed, mut exp, mut stats) in &mut tasks {
        let Ok(mut node) = nodes.get_mut(task.target) else {
            continue;
        };
        if node.amount == 0 {
            continue;
        }
        task.progress += 1;
        if task.progress >= task.required {
            task.progress = 0;
            node.amount -= 1;
            if let Ok(mut inv) = inventories.get_mut(tamed.owner) {
                inv.add(node.resource, 1);
                let levels = progression::add_xp(&mut exp, &mut stats, WORK_XP_PER_CYCLE);
                let level_note = if levels > 0 {
                    format!(" It levels up to {}!", exp.level)
                } else {
                    String::new()
                };
                log.push(format!(
                    "Your subroutine extracted a {}.{level_note}",
                    node.resource.display_name()
                ));
            }
        }
    }
}

/// Proximity-based automation: a structure with a `passive_process` recipe
/// (see `StructureDef`) converts one item into another on its own whenever
/// the player is standing within range — no assigned worker needed. This is
/// the passive counterpart to `task_progress_system`'s active, creature-run
/// production.
pub fn passive_process_system(
    mut player: Query<(&Position, &mut Inventory), With<Player>>,
    mut structures: Query<(&Structure, &Position, &mut PassiveProcessor)>,
    structure_db: Res<StructureDb>,
    mut log: ResMut<MessageLog>,
) {
    for (player_pos, mut inventory) in &mut player {
        let player_pos = *player_pos;
        for (structure, pos, mut proc) in &mut structures {
            let Some(def) = structure_db.get(&structure.kind) else {
                continue;
            };
            let Some(recipe) = &def.passive_process else {
                continue;
            };
            if (pos.x - player_pos.x).abs() > recipe.radius || (pos.y - player_pos.y).abs() > recipe.radius {
                continue;
            }
            proc.progress += 1;
            if proc.progress < recipe.ticks_per_unit {
                continue;
            }
            proc.progress = 0;
            if inventory.take(recipe.consumes, 1) == 1 {
                inventory.add(recipe.produces, 1);
                log.push(format!(
                    "The {} processes a {} into a {}.",
                    def.name,
                    recipe.consumes.display_name(),
                    recipe.produces.display_name()
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn needs_decay_at_expected_rate() {
        let (hunger, fatigue) = decay_needs(100.0, 100.0);
        assert!((hunger - (100.0 - HUNGER_DECAY_PER_TICK)).abs() < f32::EPSILON);
        assert!((fatigue - (100.0 - FATIGUE_DECAY_PER_TICK)).abs() < f32::EPSILON);
    }

    #[test]
    fn needs_never_go_negative() {
        let (hunger, fatigue) = decay_needs(0.05, 0.02);
        assert_eq!(hunger, 0.0);
        assert_eq!(fatigue, 0.0);
    }
}
