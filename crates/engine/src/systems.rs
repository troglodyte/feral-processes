use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use rand::RngExt;

use crate::components::{
    Creature, Experience, FieldBuff, FieldBuffKind, Inventory, NEED_MAX, NEED_MIN, Needs, Nest,
    NestGuardian, PassiveProcessor, Perks, Player, Position, Potential, ResourceNode, Stats,
    Structure, StructureTier, Tamed, Task, TaskKind, WanderAi, field_buff_power_of,
};
use crate::items_db::ItemDb;
use crate::perks::Perk;
use crate::progression;
use crate::resources::{GameRng, MessageKind, MessageLog, ZoneLevel};
use crate::species::SpeciesDb;
use crate::structures::StructureDb;
use crate::tuning::{
    FATIGUE_REGEN_PER_TICK, HUNGER_DECAY_PER_TICK, WORK_XP_LEVEL_CAP, WORK_XP_PER_CYCLE,
};
use crate::tuning::{
    MINING_SUCCESS_BASE, MINING_SUCCESS_PER_LEVEL, NEST_TETHER_RADIUS, NODE_PAYOUT_ZONE_BONUS,
};
use crate::world::WorldMap;

/// One tick of the two needs; pulled out of the system so the rates are
/// unit-testable without spinning up an ECS `World`.
///
/// They move in opposite directions — hunger drains, fatigue refills — see
/// `tuning::FATIGUE_REGEN_PER_TICK`. `hunger_multiplier` scales only the
/// hunger rate (e.g. `Perk::LowPowerMode`'s per-level reduction); fatigue
/// is unaffected by it.
pub fn tick_needs(hunger: f32, fatigue: f32, hunger_multiplier: f32) -> (f32, f32) {
    (
        (hunger - HUNGER_DECAY_PER_TICK * hunger_multiplier).max(0.0),
        (fatigue + FATIGUE_REGEN_PER_TICK).min(NEED_MAX),
    )
}

pub fn needs_tick_system(
    mut query: Query<(&mut Needs, &mut Stats, Option<&Perks>), With<Player>>,
    mut log: ResMut<MessageLog>,
) {
    for (mut needs, mut stats, perks) in &mut query {
        let low_power_level = perks.map(|p| p.level(Perk::LowPowerMode)).unwrap_or(0);
        let hunger_multiplier = (1.0
            - crate::tuning::LOW_POWER_MODE_REDUCTION_PER_LEVEL * low_power_level as f32)
            .max(0.0);
        let was_starving = needs.hunger <= 0.0;
        let (hunger, fatigue) = tick_needs(needs.hunger, needs.fatigue, hunger_multiplier);
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

/// What one completed gather cycle yields from a non-banked node of `tier`
/// at `zone`: the tier itself, plus `NODE_PAYOUT_ZONE_BONUS` per zone below
/// the current one. Depth and upgrade tier add rather than multiply — see
/// that constant for why compounding them broke the economy.
///
/// Shared with `crate::balance_sim` so its base-economy projections and the
/// real payout cannot drift. Banked resources bypass this entirely for a
/// flat 1 (see `task_progress_system`): their bank limit is the pacing
/// mechanism, and a scaling payout would just overflow it every few cycles.
pub(crate) fn node_payout(tier: u32, zone: ZoneLevel) -> u32 {
    tier + NODE_PAYOUT_ZONE_BONUS * zone.0.saturating_sub(1)
}

/// Chance (0.0-1.0) a completed gather cycle against a leveled node (see
/// `ResourceNode::level`) actually yields, rather than fizzling out and
/// costing the cycle for nothing. Scales up with level so a node can be
/// made more reliable over time; a basic level-1 node succeeds only about
/// half the time.
pub(crate) fn mining_success_chance(level: u32) -> f64 {
    (MINING_SUCCESS_BASE + level as f64 * MINING_SUCCESS_PER_LEVEL).min(1.0)
}

/// One completed gather cycle against `node`: rolls the node's reliability
/// and, on success, spends a unit of its stock and reports what the cycle
/// earned. `None` is a fizzle — the cycle is spent and nothing produced.
///
/// Shared by `task_progress_system` (a program running a cronjob) and
/// `player_gather_system` (the player working the node themselves) so the
/// two cannot drift apart on payout or on reliability. Both are the same
/// job; only who is standing there differs. Callers word their own log
/// line, which is the one thing that legitimately differs between them.
pub(crate) fn resolve_gather_cycle(
    node: &mut ResourceNode,
    tier: Option<&StructureTier>,
    zone: ZoneLevel,
    item_db: &ItemDb,
    rng: &mut GameRng,
) -> Option<(crate::items::ItemId, u32)> {
    if let Some(level) = node.level
        && !rng.0.random_bool(mining_success_chance(level))
    {
        return None;
    }
    node.amount -= 1;
    let def = item_db.get(node.resource.as_str());
    // Read per cycle rather than baked in at deploy, so a base that travels
    // to a deeper zone immediately earns at the new rate.
    let payout = if def.and_then(|d| d.bank_limit).is_some() {
        1
    } else {
        node_payout(tier.map(|t| t.0).unwrap_or(1), zone)
    };
    Some((node.resource.clone(), payout))
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

/// The read-only lookups `task_progress_system` needs, bundled so bevy's
/// one-param-per-resource injection doesn't push the system past clippy's
/// argument-count threshold. Bundling beats an `#[allow]` here because the
/// grouping is real: all three are immutable reference data consulted while
/// resolving a completed gather cycle.
#[derive(SystemParam)]
pub struct CronjobLookups<'w> {
    species: Res<'w, SpeciesDb>,
    items: Res<'w, ItemDb>,
    zone: Res<'w, ZoneLevel>,
}

/// Generic job progression: any entity with a `Task` advances it once per
/// tick against its `target`; on completion the producing node hands its
/// payout to the worker's owner. A node that's been mined down to 0
/// refills to its `capacity` on the next tick rather than stalling the
/// cronjob forever. The same loop would drive future colonist-style jobs,
/// not just base-building work.
pub fn task_progress_system(
    mut tasks: Query<CronjobWorker>,
    mut nodes: Query<(&mut ResourceNode, Option<&StructureTier>)>,
    mut inventories: Query<&mut Inventory>,
    player_buff: Query<&FieldBuff, With<Player>>,
    db: CronjobLookups,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
    let CronjobLookups {
        species: species_db,
        items: item_db,
        zone,
    } = db;
    // `XpBoost` is `FieldScope::Run`: every worker's cronjob XP is boosted
    // by the same running buff on the player, not by anything the worker
    // itself carries. Read once, outside the loop, since it can't vary
    // per worker.
    let xp_boost_pct = player_buff
        .iter()
        .next()
        .map(|buff| field_buff_power_of(buff, FieldBuffKind::XpBoost))
        .unwrap_or(0);
    for (mut task, tamed, creature, potential, mut exp, mut stats) in &mut tasks {
        if !matches!(task.kind, TaskKind::GatherResource) {
            continue;
        }
        let Ok((mut node, tier)) = nodes.get_mut(task.target) else {
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
        let Some((resource, payout)) =
            resolve_gather_cycle(&mut node, tier, *zone, &item_db, &mut rng)
        else {
            log.push("Your subroutine's extraction attempt fails to compile.".to_string());
            continue;
        };
        if let Ok(mut inv) = inventories.get_mut(tamed.owner) {
            let resource_name = item_db
                .get(resource.as_str())
                .map(|d| d.name.as_str())
                .unwrap_or(resource.as_str());
            let landed = inv.add_capped(resource.clone(), payout, &item_db);
            if landed == 0 {
                log.push(format!(
                    "A cronjob yields {resource_name} but there's no room to store it."
                ));
            }
            let level_note = if exp.level < WORK_XP_LEVEL_CAP {
                let species_growth = species_db
                    .get(&creature.species)
                    .map(|s| s.growth_multiplier)
                    .unwrap_or(crate::tuning::BASELINE_GROWTH_MULTIPLIER);
                let individual_roll = potential
                    .map(|p| p.growth_roll)
                    .unwrap_or(Potential::NEUTRAL.growth_roll);
                let growth_multiplier = species_growth * individual_roll;
                let levels = progression::add_xp(
                    &mut exp,
                    &mut stats,
                    WORK_XP_PER_CYCLE,
                    growth_multiplier,
                    Some(crate::tuning::CREATURE_MAX_LEVEL),
                    xp_boost_pct,
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
                format!("Your subroutine extracted {landed} {resource_name}.{level_note}"),
            );
        }
    }
}

/// The player running a gather job themselves, rather than posting a
/// program to it — see `Game::work_structure`. The player carries the same
/// `Task` a worker does and earns through the same `resolve_gather_cycle`,
/// so the two produce identical output from the same node.
///
/// No XP is awarded, unlike a program's cronjob: a worker levels from its
/// job, and handing the player the same per-cycle XP would make a node an
/// XP faucet with no risk attached to it.
pub fn player_gather_system(
    mut player: Query<(&mut Task, &mut Inventory), With<Player>>,
    mut nodes: Query<(&mut ResourceNode, Option<&StructureTier>)>,
    item_db: Res<ItemDb>,
    zone: Res<ZoneLevel>,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
    for (mut task, mut inv) in &mut player {
        if !matches!(task.kind, TaskKind::GatherResource) {
            continue;
        }
        let Ok((mut node, tier)) = nodes.get_mut(task.target) else {
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
        let Some((resource, payout)) =
            resolve_gather_cycle(&mut node, tier, *zone, &item_db, &mut rng)
        else {
            log.push("Your extraction attempt fails to compile.".to_string());
            continue;
        };
        let resource_name = item_db
            .get(resource.as_str())
            .map(|d| d.name.as_str())
            .unwrap_or(resource.as_str());
        let landed = inv.add_capped(resource.clone(), payout, &item_db);
        if landed == 0 {
            log.push(format!(
                "You pull {resource_name} loose but there's no room to store it."
            ));
            continue;
        }
        log.push_kind(
            MessageKind::Loot,
            format!("You extract {landed} {resource_name}."),
        );
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
    item_db: Res<ItemDb>,
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
            // an award, so a full bank must refuse rather than consume the
            // input for an output that never lands. (Ordinary cargo is
            // unbounded and always has room; only a banked output can be
            // full.)
            if !inventory.has_room(&recipe.produces, 1, &item_db) {
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

/// Restores the player's Power once per tick for every in-range structure
/// whose def sets `power_regen` — no worker and no input item, unlike
/// `task_progress_system` and `passive_process_system`.
///
/// Chained ahead of `needs_tick_system` (see `Game::build_schedule`), and
/// that order is load-bearing: run the other way round, a player limping
/// into range at 0.1 Power is driven to 0 first, docked an Integrity point
/// and shown the "power reserves are critical!" warning on the very tick
/// the structure was about to cover them.
pub fn power_regen_system(
    mut player: Query<(&Position, &mut Needs), With<Player>>,
    structures: Query<(&Structure, &Position)>,
    structure_db: Res<StructureDb>,
) {
    for (player_pos, mut needs) in &mut player {
        let player_pos = *player_pos;
        for (structure, pos) in &structures {
            let Some(regen) = structure_db
                .get(&structure.kind)
                .and_then(|def| def.power_regen.as_ref())
            else {
                continue;
            };
            if (pos.x - player_pos.x).abs() > regen.radius
                || (pos.y - player_pos.y).abs() > regen.radius
            {
                continue;
            }
            // `per_tick` is mod-supplied, so it is clamped at both ends
            // rather than trusted. A negative value would drain Power past
            // 0 through a field named "regen", and NaN would pin it at the
            // ceiling forever — `f32::min` returns the non-NaN operand, so
            // a bare `.min(NEED_MAX)` silently yields NEED_MAX.
            if !regen.per_tick.is_finite() {
                continue;
            }
            needs.hunger = (needs.hunger + regen.per_tick.max(0.0)).clamp(NEED_MIN, NEED_MAX);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::items::{ItemId, ids};
    use crate::structures::StructureDb;
    use crate::tuning::PLAYER_BASE_STATS;
    use std::sync::atomic::{AtomicU32, Ordering};

    /// Pins the payout *shape*, not just its values: depth and upgrade tier
    /// each add, so neither compounds the other. The old
    /// `tier * stat_multiplier()` form put this Mk5-in-zone-3 cell at 20 and
    /// a Mk5 in zone 5 at 80.
    #[test]
    fn depth_and_tier_add_to_a_payout_rather_than_multiplying() {
        let row = |tier| {
            (1..=5)
                .map(|z| node_payout(tier, ZoneLevel(z)))
                .collect::<Vec<_>>()
        };

        assert_eq!(row(1), vec![1, 2, 3, 4, 5]);
        assert_eq!(row(3), vec![3, 4, 5, 6, 7]);
        assert_eq!(row(5), vec![5, 6, 7, 8, 9]);

        assert_eq!(
            node_payout(5, ZoneLevel(3)) - node_payout(1, ZoneLevel(3)),
            node_payout(5, ZoneLevel(1)) - node_payout(1, ZoneLevel(1)),
            "what four upgrade tiers are worth must not depend on depth"
        );
    }

    /// Writes `files` (filename, RON body) into a scratch dir and loads them
    /// through `StructureDb::load_dir` — `StructureDb`'s map is private
    /// outside its own module, so a fixture db has to come from disk. The
    /// counter disambiguates the directory per call: the pid alone repeats
    /// for every test in a run, so two tests loading fixtures in parallel
    /// would delete each other's directory mid-read.
    fn load_fixture_db(files: &[(&str, &str)]) -> StructureDb {
        static NEXT_DIR: AtomicU32 = AtomicU32::new(0);
        let dir = std::env::temp_dir().join(format!(
            "feral_structure_fixture_{}_{}",
            std::process::id(),
            NEXT_DIR.fetch_add(1, Ordering::Relaxed)
        ));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        for (name, body) in files {
            std::fs::write(dir.join(name), body).unwrap();
        }
        let (db, warnings) = StructureDb::load_dir(&dir).unwrap();
        let _ = std::fs::remove_dir_all(&dir);
        assert!(
            warnings.is_empty(),
            "fixture should parse cleanly: {warnings:?}"
        );
        db
    }

    /// A structure that only regenerates Power. `per_tick` and `radius` are
    /// deliberately unlike the shipped Recharger Node's (1.0 / 7) so a test
    /// asserting on them can't accidentally pass against real game data
    /// instead of this fixture.
    fn load_test_recharger() -> StructureDb {
        load_fixture_db(&[(
            "test_recharger.ron",
            r#"(
                id: "test_recharger",
                name: "Test Recharger",
                glyph: 'z',
                color: Orange,
                build_cost: [],
                work: None,
                power_regen: Some((
                    per_tick: 2.0,
                    radius: 3,
                )),
            )"#,
        )])
    }

    /// A player at the origin with `hunger` Power, plus one structure of
    /// `kind` at each of `structure_positions`.
    fn power_regen_world(
        db: StructureDb,
        kind: &str,
        hunger: f32,
        structure_positions: &[(i32, i32)],
    ) -> (World, Entity) {
        let mut world = World::new();
        world.insert_resource(db);
        world.insert_resource(MessageLog::default());
        let player = world
            .spawn((
                Player,
                Position { x: 0, y: 0 },
                Needs {
                    hunger,
                    fatigue: 100.0,
                },
                PLAYER_BASE_STATS,
            ))
            .id();
        for (x, y) in structure_positions {
            world.spawn((
                Structure {
                    kind: kind.to_string(),
                },
                Position { x: *x, y: *y },
            ));
        }
        (world, player)
    }

    /// Runs `power_regen_system` alone for one tick and returns the player's
    /// resulting Power.
    fn run_regen_once(db: StructureDb, kind: &str, hunger: f32, at: &[(i32, i32)]) -> f32 {
        let (mut world, player) = power_regen_world(db, kind, hunger, at);
        let mut schedule = Schedule::default();
        schedule.add_systems(power_regen_system);
        schedule.run(&mut world);
        world.get::<Needs>(player).unwrap().hunger
    }

    #[test]
    fn power_regen_restores_per_tick_while_in_range() {
        let hunger = run_regen_once(load_test_recharger(), "test_recharger", 50.0, &[(0, 0)]);
        assert_eq!(
            hunger, 52.0,
            "an in-range structure should add its per_tick"
        );
    }

    #[test]
    fn power_regen_clamps_at_full_power() {
        let hunger = run_regen_once(load_test_recharger(), "test_recharger", 99.0, &[(0, 0)]);
        assert_eq!(hunger, 100.0, "Power must never exceed the 0..=100 range");
    }

    /// `per_tick` comes from a mod file, so it is not trusted. A negative
    /// one would drain Power through a field named "regen", past 0 and out
    /// of the range `Needs` documents. NaN is worse: `f32::min` returns the
    /// non-NaN operand, so clamping with a bare `.min(NEED_MAX)` turns it
    /// into *permanently full* Power rather than rejecting it.
    #[test]
    fn power_regen_neither_drains_nor_pins_power_on_a_malformed_per_tick() {
        for (id, per_tick) in [("drainer", "-5.0"), ("nonsense", "NaN")] {
            let db = load_fixture_db(&[(
                &format!("{id}.ron"),
                &format!(
                    r#"(
                        id: "{id}",
                        name: "Malformed",
                        glyph: 'z',
                        color: Orange,
                        build_cost: [],
                        work: None,
                        power_regen: Some((
                            per_tick: {per_tick},
                            radius: 3,
                        )),
                    )"#
                ),
            )]);
            // Without this the NaN case could pass vacuously: a fixture RON
            // refuses to parse loads as an empty db, and a structure with no
            // def is skipped before `per_tick` is ever read.
            assert!(
                db.get(id).is_some_and(|d| d.power_regen.is_some()),
                "the {id} fixture has to actually load, or this proves nothing"
            );
            let hunger = run_regen_once(db, id, 50.0, &[(0, 0)]);
            assert_eq!(
                hunger, 50.0,
                "per_tick: {per_tick} must leave Power untouched, not move it"
            );
        }
    }

    #[test]
    fn power_regen_ignores_a_structure_past_its_radius_on_either_axis() {
        for at in [(4, 0), (0, 4), (-4, 0), (0, -4)] {
            let hunger = run_regen_once(load_test_recharger(), "test_recharger", 50.0, &[at]);
            assert_eq!(
                hunger, 50.0,
                "a structure at {at:?} is outside radius 3 and should do nothing"
            );
        }
    }

    #[test]
    fn power_regen_applies_at_exactly_the_radius_boundary() {
        let hunger = run_regen_once(load_test_recharger(), "test_recharger", 50.0, &[(3, 3)]);
        assert_eq!(
            hunger, 52.0,
            "radius is inclusive, matching passive_process"
        );
    }

    #[test]
    fn power_regen_stacks_across_in_range_structures() {
        let hunger = run_regen_once(
            load_test_recharger(),
            "test_recharger",
            50.0,
            &[(0, 0), (1, 1)],
        );
        assert_eq!(
            hunger, 54.0,
            "each in-range structure adds its own per_tick"
        );
    }

    #[test]
    fn a_structure_without_power_regen_does_not_restore_power() {
        let hunger = run_regen_once(load_test_capacitor(), "test_capacitor", 50.0, &[(0, 0)]);
        assert_eq!(
            hunger, 50.0,
            "a def that sets no power_regen must be inert here"
        );
    }

    #[test]
    fn power_regen_runs_before_decay_so_arriving_drained_costs_no_integrity() {
        let (mut world, player) =
            power_regen_world(load_test_recharger(), "test_recharger", 0.1, &[(0, 0)]);
        let mut schedule = Schedule::default();
        schedule.add_systems((power_regen_system, needs_tick_system).chain());
        schedule.run(&mut world);

        let stats = *world.get::<Stats>(player).unwrap();
        let needs = *world.get::<Needs>(player).unwrap();
        assert_eq!(
            stats.hp, stats.max_hp,
            "regen must cover the player before decay can starve them"
        );
        assert!(
            (needs.hunger - (0.1 + 2.0 - HUNGER_DECAY_PER_TICK)).abs() < 1e-5,
            "expected regen then decay, got {}",
            needs.hunger
        );
    }

    fn test_item_db() -> ItemDb {
        ItemDb::load_dir(
            &std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/items"),
        )
        .unwrap()
        .0
    }

    /// A conversion that consumes ordinary cargo and produces a *banked*
    /// currency. The Buffer is unbounded now, so a banked output is the only
    /// kind that can still be "full" — this is how we observe the guard that
    /// won't consume the input unless the output will land. Written to a
    /// scratch temp dir and loaded through `StructureDb::load_dir`, same
    /// fixture pattern `research.rs`'s tests use, since `StructureDb`'s
    /// fields are private outside its module.
    fn load_test_capacitor() -> StructureDb {
        load_fixture_db(&[(
            "test_capacitor.ron",
            r#"(
                id: "test_capacitor",
                name: "Test Capacitor",
                glyph: 'C',
                color: Cyan,
                build_cost: [],
                work: None,
                passive_process: Some((
                    consumes: "core_fragment",
                    produces: "research_data",
                    ticks_per_unit: 1,
                    radius: 5,
                )),
            )"#,
        )])
    }

    #[test]
    fn passive_process_does_not_consume_input_when_a_banked_output_is_full() {
        let structure_db = load_test_capacitor();
        let item_db = test_item_db();
        let limit = item_db
            .get(ids::RESEARCH_DATA)
            .and_then(|d| d.bank_limit)
            .expect("research_data ships with a bank limit");

        let mut world = World::new();
        world.insert_resource(structure_db);
        world.insert_resource(item_db);
        world.insert_resource(MessageLog::default());

        let mut inventory = Inventory::default();
        inventory.add(ItemId::from(ids::RESEARCH_DATA), limit); // bank already full
        inventory.add(ItemId::from(ids::CORE_FRAGMENT), 10);
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
            inv.count(&ItemId::from(ids::CORE_FRAGMENT)),
            10,
            "the input must not be consumed when the banked output has no room"
        );
        assert_eq!(
            inv.count(&ItemId::from(ids::RESEARCH_DATA)),
            limit,
            "the bank must not grow past its limit"
        );
    }

    #[test]
    fn hunger_drains_while_fatigue_recovers() {
        let (hunger, fatigue) = tick_needs(50.0, 50.0, 1.0);
        assert!((hunger - (50.0 - HUNGER_DECAY_PER_TICK)).abs() < f32::EPSILON);
        assert!(
            (fatigue - (50.0 + FATIGUE_REGEN_PER_TICK)).abs() < f32::EPSILON,
            "fatigue is the ability-energy pool, not a second starvation clock"
        );
    }

    #[test]
    fn hunger_never_goes_negative_and_fatigue_never_passes_full() {
        let (hunger, _) = tick_needs(0.05, 50.0, 1.0);
        assert_eq!(hunger, 0.0);
        let (_, fatigue) = tick_needs(50.0, NEED_MAX - 0.01, 1.0);
        assert_eq!(
            fatigue, NEED_MAX,
            "regen must clamp at full rather than overfilling the pool"
        );
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
        let (hunger, fatigue) = tick_needs(100.0, 50.0, 0.5);
        assert!((hunger - (100.0 - HUNGER_DECAY_PER_TICK * 0.5)).abs() < f32::EPSILON);
        assert!(
            (fatigue - (50.0 + FATIGUE_REGEN_PER_TICK)).abs() < f32::EPSILON,
            "fatigue regen shouldn't be affected by the hunger multiplier"
        );
    }
}
