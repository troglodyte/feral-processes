use bevy_ecs::prelude::*;
use rand::RngExt;

use crate::NEST_TETHER_RADIUS;
use crate::components::{
    Creature, Experience, Inventory, Needs, Nest, NestGuardian, PassiveProcessor, Perks, Player,
    Position, Potential, ResourceNode, Stats, Structure, Tamed, Task, TaskKind, WanderAi,
};
use crate::items_db::ItemDb;
use crate::perks::Perk;
use crate::progression;
use crate::resources::{GameRng, MessageKind, MessageLog};
use crate::species::SpeciesDb;
use crate::structures::StructureDb;
use crate::world::WorldMap;

/// XP a tamed creature earns for each completed gather cycle.
const WORK_XP_PER_CYCLE: u32 = 5;

/// A cronjob worker stops earning XP from `task_progress_system` once it
/// reaches this level — structure work is meant to be a steady, low-effort
/// income, not a way to grind a pet's level without ever battling. Levels
/// above this only come from combat (`Game::award_player_xp` /
/// `award_party_xp`), up to the separate, higher absolute ceiling every
/// entity shares — see `progression::MAX_LEVEL`.
pub(crate) const WORK_XP_LEVEL_CAP: u32 = 10;

const HUNGER_DECAY_PER_TICK: f32 = 0.15;
const FATIGUE_DECAY_PER_TICK: f32 = 0.08;

/// One tick of hunger/fatigue decay; pulled out of the system so the rates
/// are unit-testable without spinning up an ECS `World`. `hunger_multiplier`
/// scales only the hunger rate (e.g. `Perk::LowPowerMode`'s per-level
/// reduction) — fatigue is unaffected.
pub fn decay_needs(hunger: f32, fatigue: f32, hunger_multiplier: f32) -> (f32, f32) {
    (
        (hunger - HUNGER_DECAY_PER_TICK * hunger_multiplier).max(0.0),
        (fatigue - FATIGUE_DECAY_PER_TICK).max(0.0),
    )
}

pub fn needs_decay_system(
    mut query: Query<(&mut Needs, &mut Stats, Option<&Perks>), With<Player>>,
    mut log: ResMut<MessageLog>,
) {
    for (mut needs, mut stats, perks) in &mut query {
        let low_power_level = perks.map(|p| p.level(Perk::LowPowerMode)).unwrap_or(0);
        let hunger_multiplier =
            (1.0 - crate::LOW_POWER_MODE_REDUCTION_PER_LEVEL * low_power_level as f32).max(0.0);
        let was_starving = needs.hunger <= 0.0;
        let (hunger, fatigue) = decay_needs(needs.hunger, needs.fatigue, hunger_multiplier);
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
    mut query: Query<(&mut Position, &mut WanderAi, Option<&NestGuardian>), Without<Player>>,
    nests: Query<&Position, (With<Nest>, Without<WanderAi>)>,
    mut world: ResMut<WorldMap>,
    mut rng: ResMut<GameRng>,
) {
    for (mut pos, mut ai, guardian) in &mut query {
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
        if let Some(guardian) = guardian
            && let Ok(nest_pos) = nests.get(guardian.nest)
        {
            let dist = (nx - nest_pos.x).abs().max((ny - nest_pos.y).abs());
            if dist > NEST_TETHER_RADIUS {
                continue;
            }
        }
        if world.tile(nx, ny).walkable {
            pos.x = nx;
            pos.y = ny;
        }
    }
}

/// Chance (0.0-1.0) a completed gather cycle against a leveled node (see
/// `ResourceNode::level`) actually yields, rather than fizzling out and
/// costing the cycle for nothing. Scales up with level so a node can be
/// made more reliable over time; a basic level-1 node succeeds only about
/// half the time.
fn mining_success_chance(level: u32) -> f64 {
    (0.4 + level as f64 * 0.1).min(1.0)
}

/// The worker-side components `task_progress_system` reads per cronjob
/// assignment. Aliased rather than written inline because the tuple is long
/// enough to trip clippy's `type_complexity` lint.
type CronjobWorker = (
    &'static mut Task,
    &'static Tamed,
    &'static Creature,
    Option<&'static Potential>,
    &'static mut Experience,
    &'static mut Stats,
);

/// Generic job progression: any entity with a `Task` advances it once per
/// tick against its `target`; on completion the producing node hands a unit
/// of resource to the worker's owner. A node that's been mined down to 0
/// refills to its `capacity` on the next tick rather than stalling the
/// cronjob forever. The same loop would drive future colonist-style jobs,
/// not just base-building work.
///
/// Bevy injects one system param per query/resource, so the count here
/// tracks the data the system touches, not incidental complexity worth
/// refactoring away. TODO: fold the structure query and `StructureDb` into
/// a `#[derive(SystemParam)]` bundle so this drops back under the lint
/// threshold without suppressing it — this is the only `#[allow]` in the
/// workspace and shouldn't set a precedent.
#[allow(clippy::too_many_arguments)]
pub fn task_progress_system(
    mut tasks: Query<CronjobWorker>,
    mut nodes: Query<&mut ResourceNode>,
    mut inventories: Query<&mut Inventory>,
    structures: Query<&Structure>,
    structure_db: Res<StructureDb>,
    species_db: Res<SpeciesDb>,
    item_db: Res<ItemDb>,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
    let capacity = crate::structures::inventory_capacity_for(
        structures.iter().map(|s| s.kind.as_str()),
        &structure_db,
    );
    for (mut task, tamed, creature, potential, mut exp, mut stats) in &mut tasks {
        if !matches!(task.kind, TaskKind::GatherResource) {
            continue;
        }
        let Ok(mut node) = nodes.get_mut(task.target) else {
            continue;
        };
        if node.amount == 0 {
            node.amount = node.capacity;
        }
        task.progress += 1;
        if task.progress < task.required {
            continue;
        }
        task.progress = 0;
        if let Some(level) = node.level
            && !rng.0.random_bool(mining_success_chance(level))
        {
            log.push("Your subroutine's extraction attempt fails to compile.".to_string());
            continue;
        }
        node.amount -= 1;
        if let Ok(mut inv) = inventories.get_mut(tamed.owner) {
            let resource_name = item_db
                .get(node.resource.as_str())
                .map(|d| d.name.as_str())
                .unwrap_or(node.resource.as_str());
            if inv.add_capped(node.resource.clone(), 1, capacity, &item_db) == 0 {
                log.push(format!(
                    "A cronjob yields {resource_name} but there's no room to store it."
                ));
            }
            let level_note = if exp.level < WORK_XP_LEVEL_CAP {
                let species_growth = species_db
                    .get(&creature.species)
                    .map(|s| s.growth_multiplier)
                    .unwrap_or(progression::BASELINE_GROWTH_MULTIPLIER);
                let individual_roll = potential
                    .map(|p| p.growth_roll)
                    .unwrap_or(Potential::NEUTRAL.growth_roll);
                let growth_multiplier = species_growth * individual_roll;
                let levels = progression::add_xp(
                    &mut exp,
                    &mut stats,
                    WORK_XP_PER_CYCLE,
                    growth_multiplier,
                    Some(progression::CREATURE_MAX_LEVEL),
                );
                if levels > 0 {
                    format!(" It levels up to {}!", exp.level)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            log.push_kind(
                MessageKind::Loot,
                format!("Your subroutine extracted a {resource_name}.{level_note}"),
            );
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
    all_structures: Query<&Structure>,
    structure_db: Res<StructureDb>,
    item_db: Res<ItemDb>,
    mut log: ResMut<MessageLog>,
) {
    let capacity = crate::structures::inventory_capacity_for(
        all_structures.iter().map(|s| s.kind.as_str()),
        &structure_db,
    );
    for (player_pos, mut inventory) in &mut player {
        let player_pos = *player_pos;
        for (structure, pos, mut proc) in &mut structures {
            let Some(def) = structure_db.get(&structure.kind) else {
                continue;
            };
            let Some(recipe) = &def.passive_process else {
                continue;
            };
            if (pos.x - player_pos.x).abs() > recipe.radius
                || (pos.y - player_pos.y).abs() > recipe.radius
            {
                continue;
            }
            proc.progress += 1;
            if proc.progress < recipe.ticks_per_unit {
                continue;
            }
            proc.progress = 0;
            // Check room before taking the input: this is a conversion, not
            // an award, so a full buffer must refuse rather than consume the
            // input for an output that never lands.
            if !inventory.has_room(recipe.produces.clone(), 1, capacity, &item_db) {
                continue;
            }
            if inventory.take(recipe.consumes.clone(), 1) == 1 {
                inventory.add(recipe.produces.clone(), 1);
                let consumes_name = item_db
                    .get(recipe.consumes.as_str())
                    .map(|d| d.name.as_str())
                    .unwrap_or(recipe.consumes.as_str());
                let produces_name = item_db
                    .get(recipe.produces.as_str())
                    .map(|d| d.name.as_str())
                    .unwrap_or(recipe.produces.as_str());
                log.push_kind(
                    MessageKind::Loot,
                    format!(
                        "The {} processes a {consumes_name} into a {produces_name}.",
                        def.name
                    ),
                );
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemId, ids};
    use crate::structures::{BASE_INVENTORY_CAPACITY, StructureDb};

    fn test_item_db() -> ItemDb {
        ItemDb::load_dir(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/items"),
        )
        .unwrap()
        .0
    }

    /// A conversion that consumes a banked currency (no cargo cost) and
    /// produces ordinary cargo — unlike any shipped recipe, this can
    /// actually grow cargo usage, so it's the only way to observe the
    /// buffer-overflow bug a net-zero recipe like the real Terminal can't
    /// expose. Written to a scratch temp dir and loaded through
    /// `StructureDb::load_dir`, same fixture pattern `research.rs`'s tests
    /// use, since `StructureDb`'s fields are private outside its module.
    fn load_test_capacitor() -> StructureDb {
        let dir =
            std::env::temp_dir().join(format!("feral_passive_process_test_{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(
            dir.join("test_capacitor.ron"),
            r#"(
                id: "test_capacitor",
                name: "Test Capacitor",
                glyph: 'C',
                color: Cyan,
                build_cost: [],
                work: None,
                passive_process: Some((
                    consumes: "research_data",
                    produces: "core_fragment",
                    ticks_per_unit: 1,
                    radius: 5,
                )),
            )"#,
        )
        .unwrap();
        let (db, warnings) = StructureDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            warnings.is_empty(),
            "fixture should parse cleanly: {warnings:?}"
        );
        db
    }

    #[test]
    fn passive_process_does_not_consume_input_when_output_has_no_room() {
        let structure_db = load_test_capacitor();
        let mut world = World::new();
        world.insert_resource(structure_db);
        world.insert_resource(test_item_db());
        world.insert_resource(MessageLog::default());

        let mut inventory = Inventory::default();
        inventory.add(ItemId::from(ids::RESEARCH_DATA), 5);
        inventory.add(ItemId::from(ids::CORE_FRAGMENT), BASE_INVENTORY_CAPACITY);
        world.spawn((Player, Position { x: 0, y: 0 }, inventory));
        world.spawn((
            Structure {
                kind: "test_capacitor".to_string(),
            },
            Position { x: 0, y: 0 },
            PassiveProcessor::default(),
        ));

        let mut schedule = Schedule::default();
        schedule.add_systems(passive_process_system);
        schedule.run(&mut world);

        let mut query = world.query::<&Inventory>();
        let inv = query.iter(&world).next().unwrap();
        assert_eq!(
            inv.count(ItemId::from(ids::RESEARCH_DATA)),
            5,
            "the input must not be consumed when the produced unit has no room"
        );
        assert_eq!(
            inv.count(ItemId::from(ids::CORE_FRAGMENT)),
            BASE_INVENTORY_CAPACITY,
            "cargo must not grow past capacity"
        );
    }

    #[test]
    fn needs_decay_at_expected_rate() {
        let (hunger, fatigue) = decay_needs(100.0, 100.0, 1.0);
        assert!((hunger - (100.0 - HUNGER_DECAY_PER_TICK)).abs() < f32::EPSILON);
        assert!((fatigue - (100.0 - FATIGUE_DECAY_PER_TICK)).abs() < f32::EPSILON);
    }

    #[test]
    fn needs_never_go_negative() {
        let (hunger, fatigue) = decay_needs(0.05, 0.02, 1.0);
        assert_eq!(hunger, 0.0);
        assert_eq!(fatigue, 0.0);
    }

    #[test]
    fn mining_success_chance_rises_with_level_and_caps_at_one() {
        let level_1 = mining_success_chance(1);
        let level_2 = mining_success_chance(2);
        assert!(
            level_1 > 0.0 && level_1 < 1.0,
            "a basic level-1 node shouldn't be a sure thing"
        );
        assert!(
            level_2 > level_1,
            "a higher-level node should succeed more reliably"
        );
        assert_eq!(
            mining_success_chance(100),
            1.0,
            "chance should never exceed a sure thing"
        );
    }

    #[test]
    fn hunger_multiplier_scales_only_the_hunger_rate() {
        let (hunger, fatigue) = decay_needs(100.0, 100.0, 0.5);
        assert!((hunger - (100.0 - HUNGER_DECAY_PER_TICK * 0.5)).abs() < f32::EPSILON);
        assert!(
            (fatigue - (100.0 - FATIGUE_DECAY_PER_TICK)).abs() < f32::EPSILON,
            "fatigue decay shouldn't be affected by the hunger multiplier"
        );
    }
}
