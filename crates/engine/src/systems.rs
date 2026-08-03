use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use rand::RngExt;

use crate::components::{
    Creature, Experience, FieldBuff, FieldBuffKind, MachineStatus, NEED_MAX, NEED_MIN, Needs, Nest,
    NestGuardian, Perks, Player, Position, Potential, Pursuing, ResourceNode, Stats, Stock,
    Structure, StructureTier, Tamed, Task, TaskKind, WanderAi, field_buff_power_of,
};
use crate::items::ItemId;
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
    KEEN_SCAVENGER_BONUS_PER_LEVEL, MINING_SUCCESS_BASE, MINING_SUCCESS_PER_LEVEL,
    NEST_TETHER_RADIUS, NODE_PAYOUT_ZONE_BONUS,
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

/// The per-creature state `wander_ai_system` walks. Aliased rather than
/// written inline because the tuple is long enough to trip clippy's
/// `type_complexity` lint — same reasoning as `CronjobWorker` below.
type Wanderer<'w> = (&'w mut Position, &'w mut WanderAi, Option<&'w NestGuardian>);

pub fn wander_ai_system(
    mut query: Query<Wanderer, (Without<Player>, Without<Pursuing>)>,
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
            let current = (pos.x - nest_pos.x).abs().max((pos.y - nest_pos.y).abs());
            // Refuse only a step that both leaves the tether *and* doesn't
            // close on the nest. A guardian dragged outside its radius — by
            // a chase, or by a test placing it there — would otherwise have
            // no legal move at all and stand frozen for the rest of the
            // run. `>=`, not `>`: a lateral step that holds distance
            // constant is still refused, so a displaced guardian makes
            // monotonic progress home rather than orbiting.
            if dist > NEST_TETHER_RADIUS && dist >= current {
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
///
/// `keen_scavenger_level` is the player's `Perk::KeenScavenger` level, which
/// adds `KEEN_SCAVENGER_BONUS_PER_LEVEL` on top of whatever the node itself
/// is worth. It is a parameter rather than a lookup because this runs inside
/// systems iterating worker programs, and the perk belongs to the player —
/// callers read it once, outside their loop.
pub(crate) fn mining_success_chance(level: u32, keen_scavenger_level: u32) -> f64 {
    (MINING_SUCCESS_BASE
        + level as f64 * MINING_SUCCESS_PER_LEVEL
        + keen_scavenger_level as f64 * KEEN_SCAVENGER_BONUS_PER_LEVEL)
        .min(1.0)
}

/// Whether the structure `node_entity` belongs to declares
/// `WorkDef::flat_payout`. Read per cycle off the `StructureDb` rather than
/// mirrored onto `ResourceNode` at build time: the component is rebuilt from
/// the def on load anyway (`Game::load`), and the one field that *is*
/// mirrored, `level`, already needed a re-derive there to stop a Mk3 coming
/// back extracting like a Mk1. A node whose kind isn't in the db — a
/// hand-spawned test fixture — takes the ordinary scaling curve.
fn node_is_flat_payout(node_entity: Option<&Structure>, structure_db: &StructureDb) -> bool {
    node_entity
        .and_then(|s| structure_db.get(&s.kind))
        .and_then(|d| d.work.as_ref())
        .is_some_and(|w| w.flat_payout)
}

/// One completed gather cycle against `node`: rolls the node's reliability
/// and, on success, reports what the cycle earned. `None` is a fizzle — the
/// cycle is spent and nothing produced.
///
/// A node has nothing to spend: it is a tap, not a reserve. What paces it is
/// the caller's output buffer, which is why this function no longer decides
/// whether there is anything left to give.
///
/// Shared by `task_progress_system` (a program running a cronjob) and
/// `player_gather_system` (the player working the node themselves) so the
/// two cannot drift apart on payout or on reliability. Both are the same
/// job; only who is standing there differs. Callers word their own log
/// line, which is the one thing that legitimately differs between them, and
/// each reads `keen_scavenger_level` off the player for itself — the perk is
/// the player's wherever the cycle is being run.
pub(crate) fn resolve_gather_cycle(
    node: &ResourceNode,
    tier: Option<&StructureTier>,
    zone: ZoneLevel,
    flat_payout: bool,
    keen_scavenger_level: u32,
    item_db: &ItemDb,
    rng: &mut GameRng,
) -> Option<(crate::items::ItemId, u32)> {
    if let Some(level) = node.level
        && !rng
            .0
            .random_bool(mining_success_chance(level, keen_scavenger_level))
    {
        return None;
    }
    let def = item_db.get(node.resource.as_str());
    // Read per cycle rather than baked in at deploy, so a base that travels
    // to a deeper zone immediately earns at the new rate.
    let payout = if flat_payout || def.and_then(|d| d.bank_limit).is_some() {
        1
    } else {
        node_payout(tier.map(|t| t.0).unwrap_or(1), zone)
    };
    Some((node.resource.clone(), payout))
}

/// The ingredient list a machine declaring `assembles` runs, which is the
/// assembled item's own `CraftableDef::cost` — there is no second recipe
/// format, so a machine's recipe and the bench recipe for the same item
/// cannot drift apart.
///
/// `None` for a structure that assembles nothing, and also for one naming an
/// item that isn't craftable (a typo, or a mod whose item file was removed).
/// Both are the same thing to a caller: nothing to run. The shipped-assets
/// test is what stops the second case reaching a player.
pub(crate) fn assembly_recipe<'a>(
    def: &crate::structures::StructureDef,
    items: &'a ItemDb,
) -> Option<&'a [(ItemId, u32)]> {
    let assembles = def.assembles.as_ref()?;
    let recipe = items.get(assembles.item.as_str())?.craftable.as_ref()?;
    Some(recipe.cost.as_slice())
}

/// Moves a machine to `next`, announcing it to the base feed only when the
/// state actually changes.
///
/// **Entering a state is news; staying in it is not.** A base with four
/// stalled machines would otherwise put four lines in the pane every tick it
/// stayed stalled, which is the fastest way to make the log useless. Shared
/// by every producer rather than each wording its own transition check, so
/// the "log once" property cannot hold in one system and lapse in another.
pub(crate) fn set_machine_status(
    status: &mut MachineStatus,
    next: MachineStatus,
    name: &str,
    log: &mut MessageLog,
) {
    if *status == next {
        return;
    }
    *status = next;
    log.push_base(match next {
        MachineStatus::Running => format!("The {name} resumes."),
        MachineStatus::Starved => format!("The {name} is starved — nothing is feeding it."),
        MachineStatus::Clogged => format!("The {name} is clogged — its output buffer is full."),
        MachineStatus::Idle => format!("The {name} sits idle — no program is assigned."),
    });
}

/// The producing side of a gather cycle, shared by the cronjob and
/// player-run systems. Aliased for the same `type_complexity` reason as
/// `CronjobWorker` below.
type WorkedNode = (
    &'static mut ResourceNode,
    Option<&'static StructureTier>,
    Option<&'static Structure>,
    &'static mut Stock,
    &'static mut MachineStatus,
);

/// The worker-side components `task_progress_system` reads per cronjob
/// assignment. Aliased rather than written inline because the tuple is long
/// enough to trip clippy's `type_complexity` lint.
/// `Tamed` moved out to a `With` filter when the payout stopped going to
/// `Tamed::owner` — it is a restriction on *which* workers run cronjobs, not
/// data the loop reads.
type CronjobWorker = (
    &'static mut Task,
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
    structures: Res<'w, StructureDb>,
    zone: Res<'w, ZoneLevel>,
}

/// Generic job progression: any entity with a `Task` advances it once per
/// tick against its `target`; on completion the producing node deposits its
/// payout into *its own* output buffer, not the worker owner's cargo. A node
/// that's been mined down to 0 refills to its `capacity` on the next tick
/// rather than stalling the cronjob forever. The same loop would drive
/// future colonist-style jobs, not just base-building work.
///
/// The buffer, not the deposit pool, is what paces a node: producing against
/// a full one is a *clog*, which costs neither the cycle nor the deposit, so
/// work resumes the moment the player collects. This is also the only reason
/// anything upstream in a production chain can ever back up — a node paying
/// straight into the player's pocket is an infinite source.
pub fn task_progress_system(
    mut tasks: Query<CronjobWorker, With<Tamed>>,
    mut nodes: Query<WorkedNode>,
    player: Query<(Option<&FieldBuff>, Option<&Perks>), With<Player>>,
    db: CronjobLookups,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
    let CronjobLookups {
        species: species_db,
        items: item_db,
        structures: structure_db,
        zone,
    } = db;
    // Both of these are the player's, not the worker's: `XpBoost` is
    // `FieldScope::Run`, so every worker's cronjob XP rides the same running
    // buff, and `KeenScavenger` is a perk only the player can buy. Read once,
    // outside the loop, since neither can vary per worker.
    let (xp_boost_pct, keen_scavenger_level) = player
        .iter()
        .next()
        .map(|(buff, perks)| {
            (
                buff.map(|b| field_buff_power_of(b, FieldBuffKind::XpBoost))
                    .unwrap_or(0),
                perks.map(|p| p.level(Perk::KeenScavenger)).unwrap_or(0),
            )
        })
        .unwrap_or((0, 0));
    for (mut task, creature, potential, mut exp, mut stats) in &mut tasks {
        if !matches!(task.kind, TaskKind::GatherResource) {
            continue;
        }
        let Ok((node, tier, structure, mut stock, mut status)) = nodes.get_mut(task.target) else {
            continue;
        };
        let machine_name = structure
            .and_then(|s| structure_db.get(&s.kind))
            .map(|d| d.name.as_str())
            .unwrap_or("machine");
        task.progress += 1;
        if task.progress < task.required {
            continue;
        }
        // Held at `required` rather than reset, so a cleared clog pays out on
        // the very next tick instead of restarting the cycle from zero — the
        // work was done, it just had nowhere to go.
        if stock.output_room() == 0 {
            task.progress = task.required;
            set_machine_status(&mut status, MachineStatus::Clogged, machine_name, &mut log);
            continue;
        }
        task.progress = 0;
        let Some((resource, payout)) = resolve_gather_cycle(
            &node,
            tier,
            *zone,
            node_is_flat_payout(structure, &structure_db),
            keen_scavenger_level,
            &item_db,
            &mut rng,
        ) else {
            log.push_base("Your subroutine's extraction attempt fails to compile.".to_string());
            continue;
        };
        let resource_name = item_db
            .get(resource.as_str())
            .map(|d| d.name.as_str())
            .unwrap_or(resource.as_str());
        // Clamped rather than refused: a payout that outgrows the room left
        // must not stall the cycle, and the node clogs on the next one
        // anyway, which is where the player is told about it.
        let landed = payout.min(stock.output_room());
        *stock.output.entry(resource.clone()).or_default() += landed;
        set_machine_status(&mut status, MachineStatus::Running, machine_name, &mut log);
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
        log.push_base_kind(
            MessageKind::Loot,
            format!("Your subroutine extracted {landed} {resource_name}.{level_note}"),
        );
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
///
/// The payout lands in the node's own buffer here too, not straight into the
/// player's cargo — the player is standing beside the node, so it is one `C`
/// away, and routing this path around the buffer would leave the deposit
/// pool as the only thing pacing it. That pool is gone.
pub fn player_gather_system(
    mut player: Query<(&mut Task, Option<&Perks>), With<Player>>,
    mut nodes: Query<WorkedNode>,
    item_db: Res<ItemDb>,
    structure_db: Res<StructureDb>,
    zone: Res<ZoneLevel>,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
    for (mut task, perks) in &mut player {
        if !matches!(task.kind, TaskKind::GatherResource) {
            continue;
        }
        let Ok((node, tier, structure, mut stock, mut status)) = nodes.get_mut(task.target) else {
            continue;
        };
        let machine_name = structure
            .and_then(|s| structure_db.get(&s.kind))
            .map(|d| d.name.as_str())
            .unwrap_or("machine");
        task.progress += 1;
        if task.progress < task.required {
            continue;
        }
        if stock.output_room() == 0 {
            task.progress = task.required;
            set_machine_status(&mut status, MachineStatus::Clogged, machine_name, &mut log);
            continue;
        }
        task.progress = 0;
        let keen_scavenger_level = perks.map(|p| p.level(Perk::KeenScavenger)).unwrap_or(0);
        let Some((resource, payout)) = resolve_gather_cycle(
            &node,
            tier,
            *zone,
            node_is_flat_payout(structure, &structure_db),
            keen_scavenger_level,
            &item_db,
            &mut rng,
        ) else {
            log.push_base("Your extraction attempt fails to compile.".to_string());
            continue;
        };
        let resource_name = item_db
            .get(resource.as_str())
            .map(|d| d.name.as_str())
            .unwrap_or(resource.as_str());
        let landed = payout.min(stock.output_room());
        *stock.output.entry(resource.clone()).or_default() += landed;
        set_machine_status(&mut status, MachineStatus::Running, machine_name, &mut log);
        log.push_base_kind(
            MessageKind::Loot,
            format!("You extract {landed} {resource_name}."),
        );
    }
}

/// One tick of every assembler: pull ingredients out of the neighbours, then
/// (Task 9) work them into product.
///
/// **Machines are visited in `(x, y)` order.** Bevy's query iteration order is
/// not stable, so two machines competing for one feeder's scarce output would
/// otherwise resolve differently between runs — a flaky-test source, and a
/// base that behaves differently after a reload.
///
/// A machine with no program assigned pulls nothing. Otherwise an unstaffed
/// machine would hoard from a shared feeder while producing nothing, starving
/// the line it sits beside for no gain.
pub fn assembler_system(
    structures: Query<(Entity, &Structure, &Position), With<Stock>>,
    mut stocks: Query<&mut Stock>,
    tasks: Query<&Task>,
    structure_db: Res<StructureDb>,
    item_db: Res<ItemDb>,
) {
    let by_tile: std::collections::HashMap<(i32, i32), Entity> =
        structures.iter().map(|(e, _, p)| ((p.x, p.y), e)).collect();

    let mut machines: Vec<(Entity, (i32, i32), &crate::structures::StructureDef)> = structures
        .iter()
        .filter_map(|(e, s, p)| {
            let def = structure_db.get(&s.kind)?;
            def.assembles.as_ref()?;
            Some((e, (p.x, p.y), def))
        })
        .collect();
    machines.sort_by_key(|(_, tile, _)| *tile);

    for (machine, (x, y), def) in machines {
        let Some(recipe) = assembly_recipe(def, &item_db) else {
            continue;
        };
        let staffed = tasks
            .iter()
            .any(|t| t.target == machine && matches!(t.kind, TaskKind::GatherResource));
        if !staffed {
            continue;
        }

        // Planned against a snapshot, then applied, because reading a
        // neighbour's `output` and writing this machine's `input` are the
        // same `Query<&mut Stock>`. Planned and applied *per machine* rather
        // than for the whole base at once, so a machine earlier in the sort
        // order really has taken its share before the next one looks.
        let plan: Vec<(Entity, ItemId, u32)> = {
            let Ok(mine) = stocks.get(machine) else {
                continue;
            };
            let mut plan = Vec::new();
            for (item, per_batch) in recipe {
                let cap = per_batch * crate::tuning::INPUT_STOCK_BATCHES;
                let mut want = cap.saturating_sub(mine.input.get(item).copied().unwrap_or(0));
                for (dx, dy) in crate::game::collect::ORTHOGONAL {
                    if want == 0 {
                        break;
                    }
                    let Some(&feeder) = by_tile.get(&(x + dx, y + dy)) else {
                        continue;
                    };
                    let available = stocks
                        .get(feeder)
                        .ok()
                        .and_then(|s| s.output.get(item).copied())
                        .unwrap_or(0);
                    let take = want.min(available);
                    if take == 0 {
                        continue;
                    }
                    plan.push((feeder, item.clone(), take));
                    want -= take;
                }
            }
            plan
        };

        for (feeder, item, qty) in plan {
            let taken = {
                let Ok(mut src) = stocks.get_mut(feeder) else {
                    continue;
                };
                match src.output.get_mut(&item) {
                    Some(have) => {
                        let taken = qty.min(*have);
                        *have -= taken;
                        if *have == 0 {
                            src.output.remove(&item);
                        }
                        taken
                    }
                    None => 0,
                }
            };
            if taken == 0 {
                continue;
            }
            *stocks
                .get_mut(machine)
                .expect("planned against this machine's own stock")
                .input
                .entry(item)
                .or_default() += taken;
        }
    }
}

/// Restores the player's Power once per tick for every in-range structure
/// whose def sets `power_regen` — no worker and no input item, unlike
/// `task_progress_system`.
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
        assert_eq!(hunger, 52.0, "radius is inclusive");
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

    /// A def that sets no `power_regen`, so the regen system has to leave the
    /// player's Power alone. Written to a scratch temp dir and loaded through
    /// `StructureDb::load_dir`, same fixture pattern `research.rs`'s tests
    /// use, since `StructureDb`'s fields are private outside its module.
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
            )"#,
        )])
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
        let level_1 = mining_success_chance(1, 0);
        let level_2 = mining_success_chance(2, 0);
        assert!(
            level_1 > 0.0 && level_1 < 1.0,
            "a basic level-1 node shouldn't be a sure thing"
        );
        assert!(
            level_2 > level_1,
            "a higher-level node should succeed more reliably"
        );
        assert_eq!(
            mining_success_chance(100, 0),
            1.0,
            "chance should never exceed a sure thing"
        );
    }

    #[test]
    fn keen_scavenger_adds_to_the_mining_roll_and_still_caps_at_one() {
        let plain = mining_success_chance(1, 0);
        let boosted = mining_success_chance(1, 3);
        assert!(
            (boosted - (plain + 3.0 * crate::tuning::KEEN_SCAVENGER_BONUS_PER_LEVEL)).abs()
                < f64::EPSILON,
            "each perk level should add exactly its tuning constant to the roll"
        );
        assert_eq!(
            mining_success_chance(1, 1000),
            1.0,
            "the perk must not push the roll past a sure thing either"
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
