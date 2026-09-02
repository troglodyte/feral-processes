use bevy_ecs::prelude::*;
use bevy_ecs::system::SystemParam;
use rand::RngExt;

use crate::components::{
    Carrying, Creature, Experience, FieldBuff, FieldBuffKind, Inventory, MachineStatus, Memories,
    Needs, Nest, NestGuardian, POWER_MIN, Perks, Player, Position, Potential, PowerFuel,
    PowerReserve, Pursuing, ResourceNode, Stats, Stock, Stranded, Structure, StructureTier, Tamed,
    Task, TaskKind, WanderAi, field_buff_power_of,
};
use crate::game::base::hauling::at_station;
use crate::items::ItemId;
use crate::items_db::ItemDb;
use crate::memories::MemoryDb;
use crate::needs::NeedDb;
use crate::perks::Perk;
use crate::progression::{self, LevelGain};
use crate::resources::{GameClock, GameRng, Locale, MessageKind, MessageLog, PowerGrid, ZoneLevel};
use crate::species::{AffinityClass, SpeciesDb};
use crate::structures::StructureDb;
use crate::tuning::{
    DEFAULT_BASE_INT, DEFAULT_BASE_SPEED, LEECH_YIELD_BONUS, MEMORY_MORALE_MAX_SHIFT,
    MEMORY_MORALE_PER_POINT, MINING_SUCCESS_BASE, MINING_SUCCESS_PER_INT, MINING_SUCCESS_PER_LEVEL,
    NEST_TETHER_RADIUS, NODE_PAYOUT_ZONE_BONUS, WANDER_COOLDOWN_MAX_TICKS,
    WANDER_COOLDOWN_MIN_TICKS, WORK_TICKS_PER_SPEED,
};
use crate::tuning::{
    HUNGER_DECAY_PER_TICK, NEED_STRAIN_MAX_SHIFT, NEED_STRAIN_PER_POINT, WORK_XP_LEVEL_CAP,
    WORK_XP_PER_CYCLE,
};
use crate::world::WorldMap;

/// How much Power one tick costs, before the floor — which is
/// `PowerReserve::spend`'s and not this function's, so there is one place a
/// reserve is clamped rather than one per writer.
///
/// Pulled out of the system so the rate is unit-testable without spinning up
/// an ECS `World`. `multiplier` scales it — e.g. `Perk::LowPowerMode`'s
/// per-level reduction.
pub fn power_drain_per_tick(multiplier: f32) -> f32 {
    HUNGER_DECAY_PER_TICK * multiplier
}

pub fn needs_tick_system(
    mut query: Query<(&mut PowerReserve, &mut Stats, Option<&Perks>), With<Player>>,
    mut log: ResMut<MessageLog>,
) {
    for (mut needs, mut stats, perks) in &mut query {
        let hunger_multiplier = crate::perks::power_drain_multiplier(perks);
        let was_starving = needs.get() <= POWER_MIN;
        needs.spend(power_drain_per_tick(hunger_multiplier));
        if needs.get() <= POWER_MIN {
            stats.hp -= 1;
            if !was_starving {
                log.push("Your power reserves are critical!");
            }
        }
    }
}

/// Every owned program's reserves fall a little, and faster while it works.
///
/// **Staff only** — a party member and the wielded program are not on shift,
/// which is the v1 scope. The query is deliberately wider than that rule and
/// narrows through `party::role_of`, `base_entropy_system`'s idiom: the
/// narrowing is the part that must not exist twice.
///
/// Seeding is the first thing it does, through `Needs::seed_missing`, and it
/// is the only seeding site in the game — one code path covers a freshly
/// spawned program, a program that predates a new def, and a save written
/// before this feature. With an **empty** `NeedDb` there is nothing to seed
/// and nothing to drain, and that falls out of the loop rather than out of a
/// branch: the pre-needs game, exactly.
/// **The restore is here rather than in a system of its own**, after the
/// drain: the two write the same component, and splitting them would be two
/// systems bevy has to be told about instead of one loop that cannot disagree
/// with itself. The index is rebuilt once a tick — `drift_idle_staff` builds
/// its own once a beat — because two cheap builds beat one stale cached copy,
/// and a cached one would be a new `Resource` and another iteration-order
/// shift.
/// What `needs_drain_system` reads off each candidate. Aliased for the same
/// `type_complexity` reason `Wanderer` and `CronjobWorker` are.
type Needful<'w> = (
    Entity,
    &'w mut Needs,
    Option<&'w Task>,
    &'w Tamed,
    &'w Position,
    Option<&'w crate::disposition::Disposition>,
);

pub fn needs_drain_system(
    mut programs: Query<Needful, Without<Player>>,
    sites: Query<(&Structure, &Position)>,
    structure_db: Res<StructureDb>,
    db: Res<NeedDb>,
    roles: crate::game::party::Roles,
) {
    let amenities = crate::game::base::offshift::Amenities::build(
        sites.iter().map(|(s, p)| (&s.kind, p)),
        &structure_db,
    );
    for (entity, mut needs, task, tamed, at, disposition) in &mut programs {
        if roles.of(entity, tamed.owner) != Some(crate::game::party::ProgramRole::Staff) {
            continue;
        }
        needs.seed_missing(&db);
        for def in db.iter() {
            let Some(current) = needs.get(&def.id) else {
                continue;
            };
            // The program's own temperament, and the whole of how a
            // `Languid` body reads as lazy in play: it crosses `critical`
            // sooner, goes off shift sooner, and spends more of the run
            // walking to an amenity. A program with no disposition at all —
            // a hand-built fixture — takes the neutral 1.0 without a branch.
            let temperament = disposition.copied().unwrap_or_default().need_drain();
            let rate = if task.is_some() {
                def.drain_per_tick * def.working_multiplier * temperament
            } else {
                def.drain_per_tick * temperament
            };
            needs.set(&def.id, current - rate);
        }
        // Standing in reach of an amenity refills, whatever brought the
        // program here — a body posted beside a Sandbox is being serviced
        // too, and a rule that only paid off-shift bodies would make that
        // read as the amenity being broken.
        for def in db.iter() {
            let Some((site, rate, radius)) = amenities.nearest(&def.id, *at) else {
                continue;
            };
            if !crate::game::base::offshift::in_reach(*at, site, radius) {
                continue;
            }
            let Some(current) = needs.get(&def.id) else {
                continue;
            };
            needs.set(&def.id, current + rate);
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
        ai.cooldown = rng
            .0
            .random_range(WANDER_COOLDOWN_MIN_TICKS..WANDER_COOLDOWN_MAX_TICKS);
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
        // `open_to_hostiles`, not `walkable`: the base slab is the one safe
        // ground, and a wandering program was the only mover in the game
        // that did not honour that — see `Tile::open_to_hostiles`.
        if world.tile(nx, ny).open_to_hostiles() {
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
/// flat 1 (see `resolve_gather_cycle`). Nothing paces them — a bank has no
/// ceiling and never clogs — so what keeps a flat 1 honest is demand: the
/// research tree is a fixed ladder whose deepest node costs 45, and a payout
/// doubling per zone would collapse it rather than accelerate it.
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
///
/// `base_int` is whoever is actually working the node — the posted program's
/// `SpeciesDef::base_int`, or `DEFAULT_BASE_INT` when the player is doing it
/// themselves. It enters as a **deviation from that baseline**, not as an
/// absolute: at the baseline the term is exactly zero, so a species file
/// that never heard of the field extracts at the rate it always did. Making
/// it absolute would silently re-rate every existing species and every mod
/// by wiring alone, which is a change nobody asked for and nobody would see.
///
/// `morale` is `Game::morale` for whoever is standing there — the signed sum
/// of everything that program remembers. It reads as a **deviation from
/// zero**, the same way `base_int` reads as one from `DEFAULT_BASE_INT`, and
/// that is what buys three properties at once and none of them by a branch:
/// a program with no memories contributes nothing, the player contributes
/// nothing because the player has no `Memories` at all, and an
/// `assets/memories/` that has been deleted contributes nothing because
/// `memory_sum` skips every entry whose def no file defines.
///
/// **Its own contribution is capped**, `MEMORY_MORALE_MAX_SHIFT`, which is
/// not the same job as the outer clamp: morale is an unbounded sum of up to
/// `MEMORY_CAP_PER_PROGRAM` entries, and uncapped it would drive a node to
/// never yielding, which reads as the base being broken rather than as a
/// program being unhappy.
///
/// Clamped at both ends because `GameRng::random_bool` panics outside
/// 0..=1 — the low end matters, since nothing stops a mod authoring a
/// deeply negative `base_int`.
pub(crate) fn mining_success_chance(
    level: u32,
    keen_scavenger_level: u32,
    base_int: i32,
    morale: f32,
    strain: f32,
) -> f64 {
    (MINING_SUCCESS_BASE
        + level as f64 * MINING_SUCCESS_PER_LEVEL
        + crate::perks::mining_roll_bonus(keen_scavenger_level)
        + (base_int - DEFAULT_BASE_INT) as f64 * MINING_SUCCESS_PER_INT
        + morale_shift(morale)
        + need_shift(strain))
    .clamp(0.0, 1.0)
}

/// What a program's need strain is worth to a gather cycle, priced and capped
/// in one place.
///
/// Split out beside `morale_shift` for `morale_shift`'s own stated reason: the
/// property that matters is that it saturates, and asserting that through
/// `mining_success_chance` alone cannot tell a working cap from the outer
/// `clamp(0.0, 1.0)` swallowing the overshoot.
pub(crate) fn need_shift(strain: f32) -> f64 {
    (strain as f64 * NEED_STRAIN_PER_POINT).clamp(-NEED_STRAIN_MAX_SHIFT, NEED_STRAIN_MAX_SHIFT)
}

/// What `morale` is worth to a gather cycle, priced and capped in one place.
///
/// Split out rather than inlined so the cap is a thing a test can reach
/// directly: the property that matters is that it saturates, and asserting
/// that through `mining_success_chance` alone cannot tell a working cap
/// apart from the outer `clamp(0.0, 1.0)` swallowing the overshoot.
pub(crate) fn morale_shift(morale: f32) -> f64 {
    (morale as f64 * MEMORY_MORALE_PER_POINT)
        .clamp(-MEMORY_MORALE_MAX_SHIFT, MEMORY_MORALE_MAX_SHIFT)
}

/// How many ticks one work cycle costs a worker of `speed` at a machine
/// whose def rates it at `base_ticks`.
///
/// `class_scale` is the player's own class term (`classes::work_tick_scale`,
/// 1.0 for everyone but a Fabricator), applied here rather than at
/// `Game::work_ticks_for` so there is still one formula — `views` documents
/// a machine's shipped `ticks_per_unit` against this function, and a scale
/// applied at the caller would make the displayed figure and the real one
/// two different numbers. It is a scale rather than a `PlayerClass` because
/// what this function knows about is speed; resolving a class into a number
/// is the caller's job.
///
/// Read as a **deviation from `DEFAULT_BASE_SPEED`**, exactly like
/// `base_int`'s term in `mining_success_chance` above. A species at the
/// baseline — and the player, who has no `Creature` and so takes the
/// baseline from `Game::species_base_speed` — gets `base_ticks` back
/// unchanged. That is what keeps a machine's shipped `ticks_per_unit`
/// meaning what it says, and what puts pressure on the posting in both
/// directions: a quick program beats working the node yourself, and a slow
/// one is worse than rolling your sleeves up.
pub(crate) fn work_ticks_at_speed(base_ticks: u32, speed: i32, class_scale: f64) -> u32 {
    let scale = 1.0 + (DEFAULT_BASE_SPEED - speed) as f64 * WORK_TICKS_PER_SPEED;
    // Floored at one cycle per tick however fast the species: a modded
    // `base_speed: 200` scales straight past zero into negative.
    (base_ticks as f64 * scale * class_scale).round().max(1.0) as u32
}

/// What the *worker* brings to a gather cycle, as opposed to what the node
/// is. Bundled rather than passed loose because `resolve_gather_cycle` is
/// otherwise past clippy's argument threshold, and this is the grouping that
/// is actually real — everything else that function takes describes the node
/// or the payout.
///
/// They stay three fields and not one number because they differ in whose
/// they are and in what they touch: the perk is the player's wherever the
/// cycle is being run, while the aptitude and the class belong to whoever is
/// standing at the machine — and the first two decide whether the cycle
/// lands at all, the third only what a landed cycle is worth.
///
/// The class is carried rather than a pre-computed yield bonus because the
/// bonus does not apply to a banked or `flat_payout` node, and that branch
/// lives inside `resolve_gather_cycle`. A caller handing over a finished
/// number would have to know about an exclusion it cannot see.
#[derive(Clone, Copy)]
pub(crate) struct CycleModifiers {
    pub keen_scavenger_level: u32,
    pub base_int: i32,
    /// `None` for the player working a node themselves, for a species the
    /// db has never heard of, and for anything outside the class system —
    /// all of which take the ordinary payout.
    pub class: Option<AffinityClass>,
    /// `Game::morale` for whoever is standing at the machine: the signed sum
    /// of everything that program remembers.
    ///
    /// Beside `base_int` rather than beside the perk, because it is the same
    /// shape of thing — it belongs to the body doing the work rather than to
    /// the player, and it decides whether the cycle lands rather than what a
    /// landed cycle is worth. **`0.0` is the baseline and not a missing
    /// value**: the player has no `Memories`, a fresh program remembers
    /// nothing, and a deleted `assets/memories/` makes every entry
    /// unresolvable — all three want the shipped rate back untouched, and all
    /// three get it without a branch here or at the call site.
    pub morale: f32,
    /// `needs::strain` for whoever is standing at the machine: the signed sum
    /// of how far its reserves have run down.
    ///
    /// Beside `morale` and for its reason — it belongs to the body doing the
    /// work, and **`0.0` is the baseline and not a missing value**: a program
    /// with full reserves, the player working a node themselves, and a deleted
    /// `assets/needs/` all contribute exactly nothing, without a branch here
    /// or at the call site.
    pub need_strain: f32,
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
///
/// `base_int` is the opposite: it belongs to whoever is standing there, so
/// the two callers genuinely differ on it rather than both reading the
/// player. That difference is the feature — see each call site.
pub(crate) fn resolve_gather_cycle(
    node: &ResourceNode,
    tier: Option<&StructureTier>,
    zone: ZoneLevel,
    flat_payout: bool,
    worker: CycleModifiers,
    item_db: &ItemDb,
    rng: &mut GameRng,
) -> Option<(crate::items::ItemId, u32)> {
    if let Some(level) = node.level
        && !rng.0.random_bool(mining_success_chance(
            level,
            worker.keen_scavenger_level,
            worker.base_int,
            worker.morale,
            worker.need_strain,
        ))
    {
        return None;
    }
    let def = item_db.get(node.resource.as_str());
    // Read per cycle rather than baked in at deploy, so a base that travels
    // to a deeper zone immediately earns at the new rate.
    //
    // A Leech's bonus rides the scaled branch and not the flat one: the flat
    // 1 is what keeps a banked resource honest (see `LEECH_YIELD_BONUS`), so
    // the class that draws more out of a tap draws nothing extra out of a
    // bank.
    let payout = if flat_payout || def.is_some_and(|d| d.banked) {
        1
    } else {
        node_payout(tier.map(|t| t.0).unwrap_or(1), zone)
            + match worker.class {
                Some(AffinityClass::Leech) => LEECH_YIELD_BONUS,
                _ => 0,
            }
    };
    Some((node.resource.clone(), payout))
}

/// Where a resolved payout lands, and the whole of the difference between a
/// banked resource and ordinary salvage. Returns the units that landed.
///
/// Ordinary salvage fills the machine's *own* output buffer, clamped to the
/// room left: a payout that outgrows the room must not stall the cycle, and
/// the node clogs on the next one anyway, which is where the player is told
/// about it. A banked item goes straight to the player's bank instead —
/// there is no ceiling on a bank, so it always lands in full.
///
/// Shared by `task_progress_system` and `player_gather_system` rather than
/// written out in each, because a banked item reaching an `output` even once
/// would put it in a chain's reach and back on the collect key. That is also
/// why `produced_item` can still claim to be the whole answer to "could this
/// structure feed a neighbour": a bank is not a buffer, so nothing can pull
/// from it.
///
/// `bank` is `None` only when no `Player` exists, which no live game reaches;
/// a banked payout then lands nowhere rather than falling back to the buffer,
/// since the buffer is exactly where it must never be.
///
/// Both callers still announce the cycle to the base feed afterwards, and a
/// banked resource is deliberately no exception. That line is the *only*
/// signal the player gets that a banked resource is accruing — it has no
/// inventory row and its total lives one keypress away on the research
/// screen — so silencing it would read as the node having stopped.
pub(crate) fn deliver_payout(
    resource: &ItemId,
    payout: u32,
    stock: &mut Stock,
    items: &ItemDb,
    bank: Option<&mut Inventory>,
) -> u32 {
    if items.get(resource.as_str()).is_some_and(|d| d.banked) {
        return match bank {
            Some(inventory) => {
                inventory.add(resource.clone(), payout);
                payout
            }
            None => 0,
        };
    }
    let landed = payout.min(stock.output_room());
    *stock.output.entry(resource.clone()).or_default() += landed;
    landed
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

/// What a structure puts into its *own* output buffer, or `None` for one
/// that produces nothing. An extractor's `work.produces` and an assembler's
/// `assembles.item` are the only two ways anything reaches an `output`, so
/// this is the whole answer to "could this structure feed a neighbour".
///
/// One narrowing: a structure producing a banked item reaches no `output` at
/// all (see `deliver_payout`), so it can feed nobody. A bank is not a buffer,
/// and nothing can pull from it.
pub(crate) fn produced_item(def: &crate::structures::StructureDef) -> Option<&ItemId> {
    if let Some(work) = &def.work {
        return Some(&work.produces);
    }
    def.assembles.as_ref().map(|a| &a.item)
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
        MachineStatus::Unstaffed => format!("The {name} has no one at it — its program is away."),
        MachineStatus::Stranded => {
            format!("The {name} is cut off — its program can't find a way to it.")
        }
        MachineStatus::Idle => format!("The {name} sits idle — no program is assigned."),
        // "grid", never "Power": `Power` is already the creature meter in the
        // status column (`PowerReserve`), and the two are different resources.
        MachineStatus::Unpowered => format!("The {name} is dark — the grid can't power it."),
    });
}

/// Recomputes the base's power ledger and parks it in `resources::PowerGrid`
/// for the rest of the tick.
///
/// **Runs first in the chained base group**, and that is the load-bearing
/// part of it. The alternative — a power system running *last*, overwriting
/// whatever the producing systems decided — would have `task_progress_system`
/// set `Running` and this one set `Unpowered` within the same tick, and
/// `set_machine_status` logs on every transition: two lines in the base feed
/// every tick, forever, for as long as the base is short. Deciding the
/// ledger before anything reads it is what keeps the log quiet.
///
/// An **exclusive** system because `game::base::power::ledger` takes a whole
/// `&World` — it counts every deployed `Structure`, which is not a query this
/// module gets to narrow — and a bevy system cannot hold `&World` and a
/// `ResMut` at the same time. The cost is a sync point at the head of a chain
/// that was already serial, so nothing is lost to it.
pub fn power_grid_system(world: &mut World) {
    burn_grid_upkeep(world);
    let grid = {
        let world: &World = world;
        let db = world.resource::<StructureDb>();
        crate::game::base::power::ledger(world, db)
    };
    world.insert_resource(PowerGrid {
        supply: grid.supply,
        draw: grid.draw,
        dark: grid.dark,
    });
}

/// Spends one tick of every burning supplier's charge and buys the next Power
/// Cell for any that just ran out — see `structures::StructureDef::power_upkeep`.
///
/// **Runs inside `power_grid_system`, ahead of the ledger**, rather than as a
/// system of its own. `ledger` counts a burner's `power_supply` only while its
/// `components::PowerFuel` has charge, so a refuel landing after the ledger
/// would darken the whole base for one tick out of every
/// `tuning::POWER_UPKEEP_TICKS` — and a separate system would have to be
/// wedged into a chain whose whole point is that dark is decided before
/// anything reads or writes a `MachineStatus`.
///
/// The order it visits burners in is `(tile, entity)`, `ledger`'s own and
/// `assembler_system`'s: two suppliers touching the same buffer with one cell
/// on it have to resolve the same way every run.
///
/// A supplier that cannot pay is announced through `set_machine_status`, the
/// one place a stall is announced and one that logs only on transition — so a
/// base left dry says so once rather than every tick. `Starved` is exactly
/// the existing meaning ("the input it needs is not there") and no variant is
/// added for this.
fn burn_grid_upkeep(world: &mut World) {
    let by_tile = crate::game::base::collect::feeders_by_tile(
        world
            .query_filtered::<(Entity, &Structure, &Position), With<Stock>>()
            .iter(world),
    );

    // Collected before anything is written, and carrying the def's name and
    // fuel item so the announcement and the pull below need no second lookup
    // while other borrows are open.
    let mut burners: Vec<(Entity, (i32, i32), String, ItemId)> = {
        let db = world.resource::<StructureDb>();
        let mut found: Vec<_> = world
            .iter_entities()
            .filter_map(|e| {
                let def = db.get(&e.get::<Structure>()?.kind)?;
                let cell = def.power_upkeep.clone()?;
                e.get::<PowerFuel>()?;
                let pos = e.get::<Position>()?;
                Some((e.id(), (pos.x, pos.y), def.name.clone(), cell))
            })
            .collect();
        found.sort_by_key(|(e, tile, _, _)| (*tile, *e));
        found
    };

    for (burner, tile, name, cell) in burners.drain(..) {
        let spent = {
            let mut fuel = world
                .get_mut::<PowerFuel>(burner)
                .expect("collected with the component");
            fuel.ticks_left = fuel.ticks_left.saturating_sub(1);
            fuel.ticks_left == 0
        };
        if !spent {
            continue;
        }
        let plan = crate::game::base::collect::plan_adjacent_take(tile, 1, &by_tile, |feeder| {
            world
                .get::<Stock>(feeder)
                .and_then(|s| s.output.get(&cell).copied())
                .unwrap_or(0)
        });
        let mut bought = 0;
        for (feeder, want) in plan {
            let Some(mut stock) = world.get_mut::<Stock>(feeder) else {
                continue;
            };
            bought += crate::game::base::hauling::take_from(&mut stock, &cell, want);
        }
        if bought > 0 {
            world
                .get_mut::<PowerFuel>(burner)
                .expect("collected with the component")
                .ticks_left = crate::tuning::POWER_UPKEEP_TICKS;
        }
        let next = if bought > 0 {
            MachineStatus::Running
        } else {
            MachineStatus::Starved
        };
        // Copied out, decided, and written back: `set_machine_status` needs
        // the status and the log at once, and an exclusive system cannot hold
        // a component borrow across a `resource_mut`.
        let Some(mut status) = world.get::<MachineStatus>(burner).copied() else {
            continue;
        };
        {
            let mut log = world.resource_mut::<MessageLog>();
            set_machine_status(&mut status, next, &name, &mut log);
        }
        if let Some(mut current) = world.get_mut::<MachineStatus>(burner) {
            *current = status;
        }
    }
}

/// Every machine nobody is posted to reports `Idle`.
///
/// One pass for every kind of machine, because "nobody is working this" is
/// one fact. It used to be a branch inside `assembler_system`, which visits
/// only structures declaring `assembles` — so an *extractor* with no program
/// was never visited by anything at all. `MachineStatus` defaults to
/// `Running`, and nothing else writes a status for a machine with no `Task`
/// pointed at it, so a freshly deployed Research Node drew green on the map
/// as though it were producing, and a machine whose worker was killed or
/// reassigned kept whatever status it had held at the time, forever.
///
/// `TaskKind::Guard` deliberately does not count: a guard defends a
/// structure without working it (see `Game::assign_guard`), so a machine
/// with only a guard on it really is idle.
///
/// Runs first in the chain (behind `power_grid_system`), as the baseline the
/// worked-machine systems refine. For `Idle` the order is for legibility
/// rather than to resolve a conflict — it can only ever touch a machine those
/// systems will not look at.
///
/// For `Unpowered` the order is load-bearing, and this is also the **single
/// writer** of that status. It already makes one pass over *every*
/// `Structure`, worked or not, which is why the precedence call lives here:
/// dark wins over all five other variants, and stating that once, in the one
/// place that can see both facts, beats spreading it across three systems
/// that would each have to agree. `task_progress_system`,
/// `assembler_system` and `player_gather_system` guard on the same
/// `PowerGrid` but write no status.
///
/// **Dark is checked before `worked`, deliberately.** A dark machine with a
/// program posted to it must still report `Unpowered`: the posting is exactly
/// one of the wasted moves the status exists to tell the player about, and an
/// early `continue` on `worked` would leave it reporting whatever it held
/// before the base went short.
pub fn idle_machine_system(
    mut machines: Query<(Entity, &Structure, &mut MachineStatus)>,
    tasks: Query<&Task>,
    structure_db: Res<StructureDb>,
    grid: Res<PowerGrid>,
    mut log: ResMut<MessageLog>,
) {
    for (machine, structure, mut status) in &mut machines {
        let def = structure_db.get(&structure.kind);
        let name = def.map(|d| d.name.as_str()).unwrap_or("machine");
        if grid.is_dark(machine) {
            set_machine_status(&mut status, MachineStatus::Unpowered, name, &mut log);
            continue;
        }
        // A structure that runs no job carries a status only because it can
        // stall some other way — a burning supplier's `Starved`. Nobody is
        // ever posted to one, so `Idle` here would be a permanent false
        // alarm that also stamped over what `power_grid_system` just wrote.
        if def.is_some_and(|d| !d.runs_a_job()) {
            continue;
        }
        let worked = tasks
            .iter()
            .any(|t| t.target == machine && matches!(t.kind, TaskKind::GatherResource));
        if worked {
            continue;
        }
        set_machine_status(&mut status, MachineStatus::Idle, name, &mut log);
    }
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
    &'static Position,
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
    &'static Position,
    Option<&'static Carrying>,
    Option<&'static Stranded>,
    Option<&'static Memories>,
    Option<&'static Needs>,
    Option<&'static crate::disposition::Disposition>,
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
    /// This tick's power ledger, read for the one-line dark guard below.
    /// Bundled here rather than added as a seventh system parameter for the
    /// reason the other three are: it is immutable reference data for the
    /// tick, and the parameter list is already at clippy's threshold.
    power: Res<'w, PowerGrid>,
    /// The two halves of pricing a worker's morale, which is derived from the
    /// clock on every read and never stored. Neither is a *new* resource —
    /// both are already registered and already saved — so this adds a read
    /// where there was none rather than shifting what the world holds.
    memories: Res<'w, MemoryDb>,
    clock: Res<'w, GameClock>,
    /// The other half of pricing what a worker brings to a cycle — see
    /// `CycleModifiers::need_strain`. Already a registered resource, so this
    /// adds a read where there was none rather than shifting what the world
    /// holds.
    needs: Res<'w, NeedDb>,
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
///
/// A banked resource (`ItemDef::banked`, see `deliver_payout`) is exactly
/// that infinite source, deliberately: it skips the buffer, so it never
/// clogs and nothing paces it. It is bounded by demand rather than by
/// storage — the research tree is a finite ladder, and once every node is
/// bought, more of it buys nothing. Don't reach for this shape for anything
/// a player spends forever.
/// What `task_progress_system` reads off the player: the two run-wide
/// modifiers its workers earn by, plus the bank a banked payout lands in.
/// Aliased for the same reason `Wanderer` is — the tuple trips clippy's
/// `type_complexity` lint written inline.
type CronjobPlayer<'w> = (Option<&'w FieldBuff>, Option<&'w Perks>, &'w mut Inventory);

pub fn task_progress_system(
    mut tasks: Query<CronjobWorker, With<Tamed>>,
    mut nodes: Query<WorkedNode>,
    mut player: Query<CronjobPlayer, With<Player>>,
    db: CronjobLookups,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
    let CronjobLookups {
        species: species_db,
        items: item_db,
        structures: structure_db,
        zone,
        power: grid,
        memories: memory_db,
        clock,
        needs: need_db,
    } = db;
    // Both of these are the player's, not the worker's: `XpBoost` is
    // `FieldScope::Run`, so every worker's cronjob XP rides the same running
    // buff, and `KeenScavenger` is a perk only the player can buy. Read once,
    // outside the loop, since neither can vary per worker.
    //
    // The bank comes out of the same single pass and is held across the loop
    // rather than re-fetched per worker: it is one entity's `Inventory`, and
    // nothing else in this loop touches it.
    let (xp_boost_pct, keen_scavenger_level, mut bank) = match player.iter_mut().next() {
        Some((buff, perks, inventory)) => (
            buff.map(|b| field_buff_power_of(b, FieldBuffKind::XpBoost))
                .unwrap_or(0),
            perks.map(|p| p.level(Perk::KeenScavenger)).unwrap_or(0),
            Some(inventory),
        ),
        None => (0, 0, None),
    };
    for (
        mut task,
        creature,
        potential,
        mut exp,
        mut stats,
        worker_pos,
        carrying,
        stranded,
        memories,
        worker_needs,
        disposition,
    ) in &mut tasks
    {
        if !matches!(task.kind, TaskKind::GatherResource) {
            continue;
        }
        // A dark machine makes no progress, and this system writes no status
        // saying so: `idle_machine_system` has already put `Unpowered` on it
        // this tick, and a second writer would ping-pong the label and log a
        // transition every tick the base stayed short.
        if grid.is_dark(task.target) {
            continue;
        }
        let Ok((node, tier, structure, mut stock, mut status, node_pos)) =
            nodes.get_mut(task.target)
        else {
            continue;
        };
        let machine_name = structure
            .and_then(|s| structure_db.get(&s.kind))
            .map(|d| d.name.as_str())
            .unwrap_or("machine");
        // The walk is only a cost because of this gate: a worker en route to
        // its post, off delivering, or standing at its machine still holding
        // a load produces nothing. `carrying` covers the arrival tick
        // specifically — a worker that produced there would refill the room
        // its own load needs and be left holding the remainder forever.
        if !at_station(*worker_pos, *node_pos) || carrying.is_some() {
            // `Stranded` is the more specific reading of the same situation:
            // nobody is at the machine, and no amount of waiting will change
            // that. The marker is `haul_step_system`'s, written a tick ago —
            // see `components::Stranded` for why the lag is accepted.
            let away = if stranded.is_some() {
                MachineStatus::Stranded
            } else {
                MachineStatus::Unstaffed
            };
            set_machine_status(&mut status, away, machine_name, &mut log);
            continue;
        }
        task.progress += 1;
        if task.progress < task.required {
            // A cycle part-way through is a working machine, and it has to
            // say so: this used to ride on `MachineStatus`'s `Running`
            // default, which held only because nothing announced a status
            // for a machine until its first payout. `idle_machine_system`
            // now writes that baseline, so a long cycle would otherwise read
            // as idle for every tick but the one it pays out on.
            set_machine_status(&mut status, MachineStatus::Running, machine_name, &mut log);
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
        // The posted program's own aptitude, which is the whole of what this
        // parameter is for: who you post to a node now changes how often it
        // fizzles. A species missing from the db is a hand-spawned test
        // fixture and takes the baseline, the same way `node_is_flat_payout`
        // already treats a structure kind the db has never heard of.
        let worker_def = species_db.get(&creature.species);
        let worker_int = worker_def.map(|d| d.base_int).unwrap_or(DEFAULT_BASE_INT);
        let Some((resource, payout)) = resolve_gather_cycle(
            &node,
            tier,
            *zone,
            node_is_flat_payout(structure, &structure_db),
            CycleModifiers {
                keen_scavenger_level,
                base_int: worker_int,
                class: worker_def.and_then(|d| d.affinity_class()),
                // The same fold `Game::morale` is, through the shared free
                // function rather than a copy of it: a worker with no
                // `Memories` sums nothing and reads exactly `0.0`, which is
                // the baseline and not a missing value.
                morale: memories
                    .map(|m| {
                        crate::memories::sum_intensity(
                            m,
                            &memory_db,
                            clock.tick,
                            disposition.copied().unwrap_or_default(),
                            |_| true,
                        )
                    })
                    .unwrap_or(0.0),
                // The same fold `Game::need_strain` is, through the shared
                // free function rather than a copy of it — `party::role_of`'s
                // reason, and the property the empty-catalogue guarantee
                // rests on.
                need_strain: worker_needs
                    .map(|n| crate::needs::strain(n, &need_db))
                    .unwrap_or(0.0),
            },
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
        let landed = deliver_payout(&resource, payout, &mut stock, &item_db, bank.as_deref_mut());
        set_machine_status(&mut status, MachineStatus::Running, machine_name, &mut log);
        let gain = if exp.level < WORK_XP_LEVEL_CAP {
            let species_growth = species_db
                .get(&creature.species)
                .map(|s| s.growth_multiplier)
                .unwrap_or(crate::tuning::BASELINE_GROWTH_MULTIPLIER);
            let individual_roll = potential
                .map(|p| p.growth_roll)
                .unwrap_or(Potential::NEUTRAL.growth_roll);
            let growth_multiplier = species_growth * individual_roll;
            progression::add_xp(
                &mut exp,
                &mut stats,
                WORK_XP_PER_CYCLE,
                growth_multiplier,
                // The zone cap, like every other XP site. `WORK_XP_LEVEL_CAP`
                // above is a *separate* and lower gate and is not folded into
                // it: that one is what stops a developed program being ground
                // up at a Mining Node, and unifying the two deletes the
                // property.
                Some(crate::tuning::zone_level_cap(zone.0)),
                xp_boost_pct,
            )
        } else {
            LevelGain::default()
        };
        log.push_base_kind(
            MessageKind::Loot,
            format!("Your subroutine extracted {landed} {resource_name}."),
        );
        // Its own `LevelUp` line rather than a tail on the payout above, so a
        // base level-up draws and prunes like the two field ones. Named by the
        // machine because this is a bevy system with no `Game` to ask
        // `creature_label`, and the base log identifies posted programs by
        // where they are posted anyway — see `set_machine_status`.
        if gain.levels > 0 {
            log.push_base_kind(
                MessageKind::LevelUp,
                format!(
                    "Your subroutine at the {machine_name} levels up to {}!",
                    exp.level
                ),
            );
            for line in progression::stat_block(&gain.stat_rows(&stats)) {
                log.push_base_kind(MessageKind::LevelUp, line);
            }
        }
    }
}

/// The read-only lookups `player_gather_system` consults. The third bundle of
/// this shape, after `CronjobLookups` and `AssemblerLookups`, and for the same
/// reason: bevy injects one parameter per resource, and adding `PowerGrid`
/// took this system to eight. A subset of `CronjobLookups` rather than a reuse
/// of it, because the player has no species and pulling `SpeciesDb` in here
/// would declare a dependency this system does not have.
#[derive(SystemParam)]
pub struct PlayerGatherLookups<'w> {
    items: Res<'w, ItemDb>,
    structures: Res<'w, StructureDb>,
    zone: Res<'w, ZoneLevel>,
    power: Res<'w, PowerGrid>,
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
/// player's cargo — the player is standing beside the node, so it is one `c`
/// away, and routing this path around the buffer would leave the deposit
/// pool as the only thing pacing it. That pool is gone.
///
/// **The third dark guard lives here**, and it is not optional even though
/// the design spec's chain diagram left this system unmarked: the spec's prose
/// says nothing the player can do makes a dark machine run, and cranking the
/// handle yourself is the largest hole there is in that. See the guard itself
/// for the second, independent reason.
pub fn player_gather_system(
    mut player: Query<(&mut Task, Option<&Perks>, &mut Inventory), With<Player>>,
    mut nodes: Query<WorkedNode>,
    db: PlayerGatherLookups,
    mut log: ResMut<MessageLog>,
    mut rng: ResMut<GameRng>,
) {
    let PlayerGatherLookups {
        items: item_db,
        structures: structure_db,
        zone,
        power: grid,
    } = db;
    for (mut task, perks, mut inventory) in &mut player {
        if !matches!(task.kind, TaskKind::GatherResource) {
            continue;
        }
        // The third of the three dark guards, in the same shape as the other
        // two and writing no status for the same reason.
        //
        // Two arguments, either one sufficient. It produces: this path calls
        // `deliver_payout`, so without the guard a player standing at a dark
        // node hand-cranks resources out of a machine the ledger has already
        // declared dark. And it *writes a status*: on its payout tick it sets
        // `Running` on a machine `idle_machine_system` set to `Unpowered`
        // earlier in the same tick, which is exactly the twice-per-transition
        // logging the design refused when it rejected a last-in-chain power
        // system — leaking back in through a third path.
        //
        // Ahead of `resolve_gather_cycle`, which draws from `GameRng`. Placing
        // it here means a dark node skips that draw, and skipping a draw
        // shifts the stream for everything behind it; that is safe only
        // because nothing goes dark until a structure authors a non-zero
        // `power_draw`. Moving this guard later to "protect the stream" would
        // trade a real production hole for a hypothetical one.
        if grid.is_dark(task.target) {
            continue;
        }
        // The node's `Position` goes unread here, and it takes both halves to
        // earn that: `work_structure` refuses a node the player is not at the
        // station of, and `move_player` drops the task the moment they step
        // away. Either half alone leaves a player working a node they are
        // nowhere near — which pays into a buffer a transfer cannot
        // reach.
        let Ok((node, tier, structure, mut stock, mut status, _)) = nodes.get_mut(task.target)
        else {
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
        // The player has no species and so no aptitude of their own; they
        // work a node at exactly the roster average, by decision rather than
        // for want of a value. That is what puts pressure on the posting in
        // both directions — a sharp program beats doing it yourself, and a
        // dull one is genuinely worse than rolling your sleeves up. Give the
        // player the top of the range instead and posting collapses back
        // into a pure time-saver, which is the thing this phase is undoing.
        let Some((resource, payout)) = resolve_gather_cycle(
            &node,
            tier,
            *zone,
            node_is_flat_payout(structure, &structure_db),
            CycleModifiers {
                keen_scavenger_level,
                base_int: DEFAULT_BASE_INT,
                // Same decision as the aptitude above: the player is in no
                // class, so working a node yourself pays the ordinary curve
                // and a Leech is a reason to hand the job over.
                class: None,
                // And the same again, but by construction rather than by
                // decision: the player has no `Memories` at all — the store is
                // minted at `Game::roster_parts` and nowhere else — so there is
                // no morale to read here and the baseline is the only honest
                // figure. A player who could be demoralised into mining badly
                // is a different game.
                morale: 0.0,
                need_strain: 0.0,
            },
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
        let landed = deliver_payout(
            &resource,
            payout,
            &mut stock,
            &item_db,
            Some(&mut inventory),
        );
        set_machine_status(&mut status, MachineStatus::Running, machine_name, &mut log);
        log.push_base_kind(
            MessageKind::Loot,
            format!("You extract {landed} {resource_name}."),
        );
    }
}

/// The read-only lookups `assembler_system` consults, bundled for exactly the
/// reason `CronjobLookups` is: bevy injects one parameter per resource, and
/// the third of them pushed the system past clippy's argument-count
/// threshold. The grouping is real — all three are immutable reference data
/// for the tick, two of them describing what a machine is and the third
/// whether it has the grid capacity to be it.
#[derive(SystemParam)]
pub struct AssemblerLookups<'w> {
    structures: Res<'w, StructureDb>,
    items: Res<'w, ItemDb>,
    power: Res<'w, PowerGrid>,
}

/// One tick of every assembler, in two phases: pull ingredients out of the
/// neighbours, then work a staged batch into one unit of product.
///
/// **Machines are visited in `(x, y)` order.** Bevy's query iteration order is
/// not stable, so two machines competing for one feeder's scarce output would
/// otherwise resolve differently between runs — a flaky-test source, and a
/// base that behaves differently after a reload.
///
/// Status resolves each tick to exactly one of `Idle` (no program) →
/// `Starved` (input short) → `Clogged` (output full) → `Running`, checked in
/// that order, and only a *change* is announced. Each stall returns before
/// touching anything: a clogged machine in particular must not spend the
/// batch it has no room to deliver.
///
/// A machine with no program assigned pulls nothing either. Otherwise an
/// unstaffed machine would hoard from a shared feeder while producing
/// nothing, starving the line it sits beside for no gain.
pub fn assembler_system(
    structures: Query<(Entity, &Structure, &Position), With<Stock>>,
    mut stocks: Query<&mut Stock>,
    mut statuses: Query<&mut MachineStatus>,
    mut tasks: Query<(Entity, &mut Task)>,
    db: AssemblerLookups,
    mut log: ResMut<MessageLog>,
) {
    let AssemblerLookups {
        structures: structure_db,
        items: item_db,
        power: grid,
    } = db;
    let by_tile = crate::game::base::collect::feeders_by_tile(structures.iter());

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
        // The second of the three guards, and it stops the machine before it
        // pulls from its neighbours as well as before it works: a dark
        // assembler must not drain a feeder's output into an input buffer it
        // will never consume. Writes no status, for the reason
        // `task_progress_system`'s copy doesn't.
        if grid.is_dark(machine) {
            continue;
        }
        let Some(recipe) = assembly_recipe(def, &item_db) else {
            continue;
        };
        let announce = |statuses: &mut Query<&mut MachineStatus>,
                        log: &mut MessageLog,
                        next: MachineStatus| {
            if let Ok(mut status) = statuses.get_mut(machine) {
                set_machine_status(&mut status, next, &def.name, log);
            }
        };

        // `Idle` is `idle_machine_system`'s to announce, for every kind of
        // machine rather than only the ones that assemble. This branch just
        // has nothing to pull.
        let Some(worker) = tasks
            .iter()
            .find(|(_, t)| t.target == machine && matches!(t.kind, TaskKind::GatherResource))
            .map(|(e, _)| e)
        else {
            continue;
        };

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
                let want = cap.saturating_sub(mine.input.get(item).copied().unwrap_or(0));
                let takes = crate::game::base::collect::plan_adjacent_take(
                    (x, y),
                    want,
                    &by_tile,
                    |feeder| {
                        stocks
                            .get(feeder)
                            .ok()
                            .and_then(|s| s.output.get(item).copied())
                            .unwrap_or(0)
                    },
                );
                plan.extend(
                    takes
                        .into_iter()
                        .map(|(feeder, take)| (feeder, item.clone(), take)),
                );
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

        // Phase 2: work. Every gate returns before spending anything, so a
        // stall costs the batch nothing and clears the moment its cause does.
        let (fed, roomy) = {
            let Ok(stock) = stocks.get(machine) else {
                continue;
            };
            let fed = recipe
                .iter()
                .all(|(item, need)| stock.input.get(item).copied().unwrap_or(0) >= *need);
            (fed, stock.output_room() > 0)
        };
        if !fed {
            announce(&mut statuses, &mut log, MachineStatus::Starved);
            continue;
        }
        if !roomy {
            announce(&mut statuses, &mut log, MachineStatus::Clogged);
            continue;
        }
        announce(&mut statuses, &mut log, MachineStatus::Running);

        let Ok((_, mut task)) = tasks.get_mut(worker) else {
            continue;
        };
        // The rate comes from the task, not from `def.assembles`: it was
        // baked at assignment out of the machine's `ticks_per_unit` and the
        // posted program's `base_speed` (`Game::work_ticks_for`). Safe
        // because `upgrade_structure` never touches `ticks_per_unit`, so
        // nothing changes a machine's rate after a program is on it, and
        // `displace_task_holder` allows only one `GatherResource` per
        // structure. The cost is that a cronjob in an old save keeps its
        // pre-`base_speed` rate until it is re-posted.
        //
        // `.max(1)` for the reason the def read had it: a zero would
        // produce on every tick forever.
        task.progress += 1;
        if task.progress < task.required.max(1) {
            continue;
        }
        task.progress = 0;

        let Ok(mut stock) = stocks.get_mut(machine) else {
            continue;
        };
        for (item, need) in recipe {
            match stock.input.get_mut(item) {
                Some(have) if *have > *need => *have -= need,
                _ => {
                    stock.input.remove(item);
                }
            }
        }
        let product = def
            .assembles
            .as_ref()
            .expect("filtered on `assembles` above")
            .item
            .clone();
        *stock.output.entry(product).or_default() += 1;
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
///
/// Permitted only in base space. This measures the space `Structure`
/// actually lives in — the same dispatch `Game::scan_center` makes, though
/// this is a bare system with no `Game` to call it on — and there is no
/// space but base space to measure it against, because there is no space
/// but base space a `Structure` can stand in
/// (`Game::place_structure` requires `require_base` for everything but the
/// founding Home).
///
/// Refusing the surface outright, rather than falling back to comparing the
/// raw `Position` there as a supposedly-harmless no-op, is load-bearing: it
/// is not a no-op. `scan_center`'s own doc names the reason — a base-space
/// coordinate and a surface coordinate read as the same numbers exactly
/// when the anchor happens to sit near base space's origin, which is where
/// `BASE_EXIT_CELL` puts a base's Home on *every* run, and the zone spawn
/// point (where the anchor stands) usually does too. `view_entities`
/// already hit this for real — a surface tile that numerically matched a
/// base-space Market's cell made `[S]ell` appear on the inventory screen —
/// and its fix was to drop `Structure` entirely off the surface, not to
/// leave the compare in. A Recharger's `power_regen.radius` is typically
/// double digits, so a base founded near spawn (the common case) and a
/// Recharger built near its Home left a player wandering the open grid
/// topped up by a machine that is not there. Refused underground for the
/// same reason `nest_aggro_tick` is: a reader of the player's `Position`
/// that never went through a locale guard but still claims something about
/// where the party is standing.
///
/// **Reaching the party in base space at all is a deliberate reversal, not
/// an oversight carried forward.** The old guard matched `Locale::Surface`
/// positively and refused base space outright, on the theory that a
/// Recharger near the anchor would otherwise top the party up while they
/// were out of phase for free. That reasoning no longer holds — see the
/// previous paragraph for what refusing base space actually bought instead
/// — so refusing it left the Recharger's whole purpose,
/// `recharger_node.ron`'s own "while you stand anywhere on your base",
/// unreachable in any real play session. Held by
/// `tests::base_space::a_recharger_regens_the_party_through_the_real_path_into_base_space`
/// and, for the surface arm, by
/// `tests::base_space::a_recharger_does_not_reach_the_party_genuinely_on_the_surface`.
pub fn power_regen_system(
    mut player: Query<&mut PowerReserve, With<Player>>,
    structures: Query<(&Structure, &Position, Option<&PowerFuel>)>,
    structure_db: Res<StructureDb>,
    locale: Res<Locale>,
) {
    let Locale::Base { x, y } = *locale else {
        return;
    };
    let scan_pos = Position { x, y };
    for mut needs in &mut player {
        for (structure, pos, fuel) in &structures {
            let Some(def) = structure_db.get(&structure.kind) else {
                continue;
            };
            let Some(regen) = def.power_regen.as_ref() else {
                continue;
            };
            if (pos.x - scan_pos.x).abs() > regen.radius
                || (pos.y - scan_pos.y).abs() > regen.radius
            {
                continue;
            }
            // A supplier that cannot pay its own upkeep does nothing at
            // all, not just the Grid half — `game::base::power::is_fuelled`
            // is the one predicate `ledger` asks too, so a dry Recharger
            // cannot silently disagree with itself about which half of it
            // is still running.
            if !crate::game::base::power::is_fuelled(def, fuel) {
                continue;
            }
            // `per_tick` is mod-supplied, so it is clamped at both ends
            // rather than trusted. A negative value would drain Power past
            // 0 through a field named "regen", and NaN would pin it at the
            // ceiling forever — `f32::min` returns the non-NaN operand, so
            // a bare `.min(POWER_MAX)` silently yields POWER_MAX.
            if !regen.per_tick.is_finite() {
                continue;
            }
            needs.restore(regen.per_tick.max(0.0));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::stack::Dir;
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

    /// The shipped item set, so the banked case below is asserted against
    /// the real `research_data` rather than a fixture that merely claims to
    /// resemble it — the whole point of that exclusion is a specific item.
    fn shipped_items() -> ItemDb {
        let assets = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let (abilities, _) =
            crate::abilities::AbilityDb::load_dir(&assets.join("abilities")).unwrap();
        ItemDb::load_dir(&assets.join("items"), &abilities)
            .unwrap()
            .0
    }

    /// What one cycle against an always-yielding node of `resource` pays a
    /// worker of `class`. `level: None` skips the reliability roll, so the
    /// RNG below is never drawn from and these are pure payout assertions.
    fn cycle_payout(resource: &str, class: Option<AffinityClass>, flat_payout: bool) -> u32 {
        let node = ResourceNode {
            resource: ItemId::from(resource),
            level: None,
        };
        let mut rng = GameRng(rand::SeedableRng::seed_from_u64(1));
        resolve_gather_cycle(
            &node,
            None,
            ZoneLevel(3),
            flat_payout,
            CycleModifiers {
                keen_scavenger_level: 0,
                base_int: DEFAULT_BASE_INT,
                class,
                morale: 0.0,
                need_strain: 0.0,
            },
            &shipped_items(),
            &mut rng,
        )
        .expect("a node with no level always yields")
        .1
    }

    /// The extraction half of the class base jobs: a Leech is worth a unit
    /// per cycle over anyone else at the same node.
    ///
    /// Asserted as a *difference from the ordinary payout* rather than
    /// against a hardcoded number, because `node_payout`'s curve is shared
    /// with `balance_sim` and is expected to move under a retune — what must
    /// not move is that the drain class is the one that gets more out of a
    /// tap.
    #[test]
    fn a_leech_draws_an_extra_unit_from_a_scaled_node() {
        let ordinary = cycle_payout(crate::items::ids::CORE_FRAGMENT, None, false);
        assert_eq!(
            ordinary,
            node_payout(1, ZoneLevel(3)),
            "a classless worker takes the ordinary curve"
        );
        assert_eq!(
            cycle_payout(
                crate::items::ids::CORE_FRAGMENT,
                Some(AffinityClass::Leech),
                false
            ),
            ordinary + LEECH_YIELD_BONUS,
        );
    }

    /// Every other class works a node at the ordinary rate. Striker stands
    /// for the four here: the job is Leech's alone, and a bonus that leaked
    /// to any class would make posting a decision about nothing again.
    #[test]
    fn a_striker_draws_no_more_than_a_classless_worker() {
        assert_eq!(
            cycle_payout(
                crate::items::ids::CORE_FRAGMENT,
                Some(AffinityClass::Striker),
                false
            ),
            cycle_payout(crate::items::ids::CORE_FRAGMENT, None, false),
        );
    }

    /// The exclusion that keeps the research ladder honest. `research_data`
    /// is banked, which pays a flat 1 however deep the zone and however
    /// upgraded the node — the only thing holding a bank's uncapped total in
    /// check is that flatness, against a research tree whose deepest node
    /// costs a fixed 45. A Leech doubling it is a different game.
    #[test]
    fn a_leech_draws_no_more_from_a_banked_node() {
        assert_eq!(
            cycle_payout(
                crate::items::ids::RESEARCH_DATA,
                Some(AffinityClass::Leech),
                false
            ),
            1,
        );
    }

    /// Same argument for a `flat_payout` node: it has opted out of the
    /// scaling curve entirely, and a bonus riding on top would put it back
    /// on a curve of its own.
    #[test]
    fn a_leech_draws_no_more_from_a_flat_payout_node() {
        assert_eq!(
            cycle_payout(
                crate::items::ids::CORE_FRAGMENT,
                Some(AffinityClass::Leech),
                true
            ),
            1,
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
        // Base space, not `Locale::default()` (`Surface`): every `Structure`
        // this file's fixtures spawn only ever exists in base space now, so
        // a world left on the surface has no structure in it to find,
        // whatever `structure_positions` says. `(0, 0)` matches the
        // player's own `Position` below purely so every existing
        // `structure_positions` offset in this module keeps meaning the
        // same thing it always did — the player's `Position` itself is not
        // what `power_regen_system` reads any more.
        world.insert_resource(Locale::Base { x: 0, y: 0 });
        let player = world
            .spawn((
                Player,
                Position { x: 0, y: 0 },
                PowerReserve::new(hunger),
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
        world.get::<PowerReserve>(player).unwrap().get()
    }

    /// `power_regen_system` reads `Locale::Base`'s own cell, never the
    /// player's `Position` — a `Structure` can now only ever stand in base
    /// space (`Game::place_structure` requires `require_base`), so
    /// comparing the pinned `Position` on either of the other two locales
    /// tops the party up on numbers that mean nothing where they actually
    /// are: the same trap `nest_aggro_tick` carries underground, and, on
    /// the surface, the exact bug `view_entities`'s own doc records already
    /// shipped once (a surface tile that numerically matched a base-space
    /// Market's cell made `[S]ell` appear on the inventory screen).
    ///
    /// All three locales live in one test on purpose: any one pair passes
    /// against a system that is simply broken in a different way — Base
    /// alone passes against "always regen", and Surface-or-Stack alone
    /// passes against "never regen".
    #[test]
    fn power_regen_reaches_only_base_space() {
        let mut power = [0.0f32; 3];
        for (i, locale) in [
            Locale::Base { x: 0, y: 0 },
            Locale::Surface,
            Locale::Stack {
                depth: 1,
                frames: 3,
                x: 4,
                y: 4,
                facing: Dir::North,
                entrance: (0, 0),
            },
        ]
        .into_iter()
        .enumerate()
        {
            let (mut world, player) =
                power_regen_world(load_test_recharger(), "test_recharger", 50.0, &[(0, 0)]);
            world.insert_resource(locale);
            let mut schedule = Schedule::default();
            schedule.add_systems(power_regen_system);
            schedule.run(&mut world);
            power[i] = world.get::<PowerReserve>(player).unwrap().get();
        }
        assert_eq!(
            power[0], 52.0,
            "the same fixture in base space must regenerate"
        );
        assert_eq!(
            power[1], 50.0,
            "a Recharger at base-space (0, 0) must not reach a party genuinely on the surface"
        );
        assert_eq!(
            power[2], 50.0,
            "and must not reach a party underground either"
        );
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
    /// of the range `PowerReserve` documents. NaN is worse: `f32::min` returns the
    /// non-NaN operand, so clamping with a bare `.min(POWER_MAX)` turns it
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
        let needs = *world.get::<PowerReserve>(player).unwrap();
        assert_eq!(
            stats.hp, stats.max_hp,
            "regen must cover the player before decay can starve them"
        );
        assert!(
            (needs.get() - (0.1 + 2.0 - HUNGER_DECAY_PER_TICK)).abs() < 1e-5,
            "expected regen then decay, got {}",
            needs.get()
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
    fn power_drains_by_its_rate() {
        let drain = power_drain_per_tick(1.0);
        assert!((drain - HUNGER_DECAY_PER_TICK).abs() < f32::EPSILON);
    }

    /// The floor is `PowerReserve::spend`'s, not the rate's — so this is
    /// asserted through the type rather than through the drain function.
    #[test]
    fn power_never_goes_negative() {
        let mut reserve = PowerReserve::new(0.05);
        reserve.spend(power_drain_per_tick(1.0));
        assert_eq!(reserve.get(), POWER_MIN);
    }

    /// The whole no-regression argument for the morale term, stated as a
    /// property rather than left implicit in the ten tests above that now
    /// pass `0.0`: a worker with nothing to remember is a worker the formula
    /// has never heard of.
    ///
    /// Swept across levels, perk levels and aptitudes, because the term is an
    /// addend and an addend that failed to be zero would show up at every
    /// combination rather than at one.
    #[test]
    fn a_worker_with_no_memories_extracts_at_the_shipped_rate() {
        for level in [1, 2, 5, 12] {
            for keen in [0, 1, 4] {
                for int_offset in [-4, 0, 3] {
                    let base_int = DEFAULT_BASE_INT + int_offset;
                    let with_morale = mining_success_chance(level, keen, base_int, 0.0, 0.0);
                    let without = (MINING_SUCCESS_BASE
                        + level as f64 * MINING_SUCCESS_PER_LEVEL
                        + crate::perks::mining_roll_bonus(keen)
                        + (base_int - DEFAULT_BASE_INT) as f64 * MINING_SUCCESS_PER_INT)
                        .clamp(0.0, 1.0);
                    assert_eq!(
                        with_morale, without,
                        "morale 0.0 moved the rate at level {level}, keen {keen}, int {base_int}"
                    );
                }
            }
        }
    }

    /// Signed and symmetric, which is what gives the positive memory kinds
    /// something to do rather than making every memory a liability.
    ///
    /// Asserted about equal *and opposite* morale rather than about a
    /// direction, since a term that only ever helped would pass a
    /// rises-with-morale test just as well.
    #[test]
    fn morale_moves_the_rate_both_ways_and_by_the_same_amount() {
        let baseline = mining_success_chance(4, 0, DEFAULT_BASE_INT, 0.0, 0.0);
        let content = mining_success_chance(4, 0, DEFAULT_BASE_INT, 8.0, 0.0);
        let miserable = mining_success_chance(4, 0, DEFAULT_BASE_INT, -8.0, 0.0);
        assert!(
            content > baseline,
            "a program that remembers good things should extract more reliably"
        );
        assert!(
            miserable < baseline,
            "a program that remembers bad things should fizzle more"
        );
        assert!(
            ((content - baseline) - (baseline - miserable)).abs() < 1e-9,
            "the term is an addend, so equal and opposite morale must move it equally"
        );
    }

    /// The guardrail that keeps this a texture rather than a difficulty knob.
    ///
    /// Asserted against `morale_shift` and **not** through
    /// `mining_success_chance`, which is the whole reason that function was
    /// split out: the outer `clamp(0.0, 1.0)` would swallow an uncapped
    /// overshoot at the low end and a test reading the finished chance could
    /// not tell a working cap from a missing one. `Game::morale` is an
    /// unbounded sum, so the figures here are deliberately past anything the
    /// shipped catalogue can build.
    #[test]
    fn the_morale_term_saturates_at_its_cap() {
        assert_eq!(morale_shift(10_000.0), MEMORY_MORALE_MAX_SHIFT);
        assert_eq!(morale_shift(-10_000.0), -MEMORY_MORALE_MAX_SHIFT);
        assert_eq!(morale_shift(0.0), 0.0);
        // And the finished chance stays inside what `random_bool` accepts,
        // which is the outer clamp's job and is asserted here so the two
        // cannot both be assumed.
        let floored = mining_success_chance(1, 0, DEFAULT_BASE_INT, -10_000.0, 0.0);
        assert!((0.0..=1.0).contains(&floored));
    }

    #[test]
    fn mining_success_chance_rises_with_level_and_caps_at_one() {
        let level_1 = mining_success_chance(1, 0, DEFAULT_BASE_INT, 0.0, 0.0);
        let level_2 = mining_success_chance(2, 0, DEFAULT_BASE_INT, 0.0, 0.0);
        assert!(
            level_1 > 0.0 && level_1 < 1.0,
            "a basic level-1 node shouldn't be a sure thing"
        );
        assert!(
            level_2 > level_1,
            "a higher-level node should succeed more reliably"
        );
        assert_eq!(
            mining_success_chance(100, 0, DEFAULT_BASE_INT, 0.0, 0.0),
            1.0,
            "chance should never exceed a sure thing"
        );
    }

    #[test]
    fn keen_scavenger_adds_to_the_mining_roll_and_still_caps_at_one() {
        let plain = mining_success_chance(1, 0, DEFAULT_BASE_INT, 0.0, 0.0);
        let boosted = mining_success_chance(1, 3, DEFAULT_BASE_INT, 0.0, 0.0);
        assert!(
            (boosted - (plain + crate::perks::mining_roll_bonus(3))).abs() < f64::EPSILON,
            "each perk level should add exactly its tuning constant to the roll"
        );
        assert_eq!(
            mining_success_chance(1, 1000, DEFAULT_BASE_INT, 0.0, 0.0),
            1.0,
            "the perk must not push the roll past a sure thing either"
        );
    }

    /// The whole point of reading `base_int` as a deviation: a species that
    /// declares nothing, a mod's species predating the field, and the player
    /// working the node themselves all sit at the baseline and must get the
    /// number the formula gave before the term existed. Asserted against the
    /// arithmetic on the constants rather than a literal, so a retune of
    /// `MINING_SUCCESS_BASE` doesn't turn this into a false alarm.
    #[test]
    fn a_baseline_worker_extracts_at_exactly_the_pre_int_rate() {
        for level in 1..=4 {
            let expected = crate::tuning::MINING_SUCCESS_BASE
                + level as f64 * crate::tuning::MINING_SUCCESS_PER_LEVEL;
            assert!(
                (mining_success_chance(level, 0, DEFAULT_BASE_INT, 0.0, 0.0) - expected).abs()
                    < f64::EPSILON,
                "a baseline worker must contribute exactly nothing at level {level}"
            );
        }
    }

    #[test]
    fn each_point_of_base_int_moves_the_roll_by_its_tuning_constant() {
        let baseline = mining_success_chance(1, 0, DEFAULT_BASE_INT, 0.0, 0.0);
        let sharp = mining_success_chance(1, 0, DEFAULT_BASE_INT + 4, 0.0, 0.0);
        let dull = mining_success_chance(1, 0, DEFAULT_BASE_INT - 4, 0.0, 0.0);
        assert!(
            (sharp - (baseline + 4.0 * crate::tuning::MINING_SUCCESS_PER_INT)).abs() < f64::EPSILON,
            "each point above the baseline should add exactly its tuning constant"
        );
        assert!(
            (dull - (baseline - 4.0 * crate::tuning::MINING_SUCCESS_PER_INT)).abs() < f64::EPSILON,
            "and each point below should subtract it — the pressure is two-sided"
        );
    }

    /// `rng.random_bool` panics outside 0..=1, so both ends are a crash and
    /// not merely a wrong number. The low end is the one that isn't obvious:
    /// nothing stops a mod authoring `base_int: -500`.
    #[test]
    fn base_int_cannot_push_the_roll_outside_a_probability() {
        assert_eq!(
            mining_success_chance(1, 0, 10_000, 0.0, 0.0),
            1.0,
            "aptitude must not push the roll past a sure thing"
        );
        assert_eq!(
            mining_success_chance(4, 0, -10_000, 0.0, 0.0),
            0.0,
            "nor below an impossibility, which would panic the roll"
        );
    }

    #[test]
    fn a_baseline_worker_works_at_exactly_the_machines_own_rate() {
        // The whole moddability contract: a species file with no `base_speed`,
        // and the player (who has no species at all), must reproduce the def's
        // number rather than merely land near it.
        for base in [1, 3, 6, 8, 10, 12, 20, 30] {
            assert_eq!(
                work_ticks_at_speed(base, DEFAULT_BASE_SPEED, 1.0),
                base,
                "a worker at the roster baseline must cost exactly the def's rate"
            );
        }
    }

    #[test]
    fn a_faster_species_needs_fewer_ticks_and_a_slower_one_more() {
        // The shipped extremes — construct 6, sprite 14 — against a Mining
        // Node's 10 and a Fabricator's 30.
        assert_eq!(work_ticks_at_speed(10, 14, 1.0), 8);
        assert_eq!(work_ticks_at_speed(10, 6, 1.0), 12);
        assert_eq!(work_ticks_at_speed(30, 14, 1.0), 24);
        assert_eq!(work_ticks_at_speed(30, 6, 1.0), 36);
    }

    #[test]
    fn an_absurd_modded_speed_still_costs_at_least_one_tick() {
        // A `base_speed: 200` mod scales the multiplier straight past zero and
        // negative. Without the floor that is a machine producing on every tick
        // forever, which is also what a `required: 0` would do.
        assert_eq!(work_ticks_at_speed(10, 200, 1.0), 1);
        assert_eq!(work_ticks_at_speed(1, 14, 1.0), 1);
    }

    #[test]
    fn the_hunger_multiplier_scales_the_drain() {
        let drain = power_drain_per_tick(0.5);
        assert!((drain - HUNGER_DECAY_PER_TICK * 0.5).abs() < f32::EPSILON);
    }
}
