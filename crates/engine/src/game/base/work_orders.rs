//! Work orders: what the base should be holding, and who it stands where
//! to close the gap.
//!
//! **A work order stores an item and a quantity. Nothing else.** No
//! per-machine plan, no unit targets, no progress counters. Which machines
//! a line needs, in what order, and who is on each is recomputed from live
//! world state every time it is asked — the same call `Game::build_radius`,
//! `Game::contract_board`, `descriptions.rs`, `Game::wielded_program` and
//! the Stack's regenerated frames all make, and it buys the same things
//! here: the derivation cannot go stale, needs no save field beyond the
//! order itself, and costs no migration when the base it describes moves.
//!
//! The alternative — multiplying the recipe tree through at queue time into
//! fixed per-machine targets and counting payouts against them — produces a
//! plan that is confidently wrong the moment a machine is demolished,
//! upgraded, or fed from stock the plan did not know about. That is the
//! second copy that drifts, which `CLAUDE.md` records biting this repo four
//! times.
//!
//! An order is a **target level, not a production run**: "3 Routine Disks"
//! means *have three*, and three already sitting in a Depot satisfy it
//! immediately.

use serde::{Deserialize, Serialize};

use crate::base_grid::BaseGrid;
use crate::game::base::collect::ORTHOGONAL;
use crate::game::base::hauling;
use crate::items::ItemId;
use crate::systems::{assembly_recipe, produced_item};
use crate::*;

/// One queue entry: an item and how many of it the base should hold.
///
/// A **named struct, never a tuple.** RON parses a `(` in a struct position
/// as the start of named fields, so a `Vec<(ItemId, u32)>` can never be
/// widened and cannot be converted to a named struct with defaulted
/// trailing fields either. Two fields shipped in that shape already
/// (`PlayerSave::fused_gear`, `SaveData::buyback`) and both had to be
/// drained into named successors; a third is not being added.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct WorkOrder {
    pub item: ItemId,
    pub qty: u32,
}

/// **Every** deployed structure whose def puts `item` into an output
/// buffer, empty if nothing standing makes it.
///
/// All of them, not the first: a base that builds a second Mining Node
/// because the first one's output is being eaten by the assembler beside it
/// has doubled its own capacity, and an order that rooted its walk at one
/// arbitrary machine would leave the one just paid for standing empty
/// forever. The same plurality is what stops an unfed twin bench refusing a
/// line that is whole elsewhere in the base.
///
/// Sorted by tile and then entity, so two machines making the same thing
/// resolve the same way on every run — bevy's query iteration order is not
/// stable, and `assembler_system` sorts its own machines for exactly this
/// reason.
pub(crate) fn producers_of(game: &Game, item: &ItemId) -> Vec<Entity> {
    let db = game.world.resource::<StructureDb>();
    let mut found: Vec<(i32, i32, Entity)> = game
        .world
        .iter_entities()
        .filter_map(|e| {
            let kind = &e.get::<Structure>()?.kind;
            let pos = e.get::<Position>()?;
            let def = db.get(kind)?;
            (produced_item(def) == Some(item)).then_some((pos.x, pos.y, e.id()))
        })
        .collect();
    found.sort();
    found.into_iter().map(|(_, _, e)| e).collect()
}

/// Whether any structure the game ships or a mod supplies could produce
/// `item` at all, deployed or not. Separates "you have not built it yet"
/// from "nothing in this game makes that", which are different errands.
fn makeable_by(game: &Game, item: &ItemId) -> Option<StructureDef> {
    game.structure_defs()
        .into_iter()
        .find(|def| produced_item(def) == Some(item))
}

/// The one sentence naming why a line for `item` can never move, or `None`
/// if it is whole.
///
/// Walks the recipe tree from the deployed machine that makes `item`, and
/// for every assembler in it checks that some orthogonal neighbour is
/// producing each ingredient — a machine can only ever take what a
/// neighbour has *finished* (`components::Stock`'s `output` is public and
/// its `input` is not), so a link with no feeder beside it can never run
/// however much of the ingredient the base holds elsewhere.
///
/// The banked exclusion falls out of this rather than being special-cased:
/// a banked payout goes straight to the player's bank and reaches no
/// `output` (`systems::deliver_payout`), so nothing can hold a stock of it
/// and nothing can be fed from it. `research_data` is refused on those
/// terms, with its Research Node standing.
pub(crate) fn chain_break(game: &Game, item: &ItemId) -> Option<String> {
    let items = game.world.resource::<ItemDb>();
    if items.get(item.as_str()).is_none() {
        return Some("No such item.".into());
    }
    if items.get(item.as_str()).is_some_and(|d| d.banked) {
        let name = game.item_name(item);
        return Some(format!(
            "{name} is banked as it is gathered — the base never holds a stock of it to order \
             against."
        ));
    }
    let machines = producers_of(game, item);
    if machines.is_empty() {
        let name = game.item_name(item);
        return Some(match makeable_by(game, item) {
            Some(def) => format!(
                "No {} deployed — that is what makes a {name}.",
                def.name.clone()
            ),
            None => format!("Nothing the base can build makes a {name}."),
        });
    }
    // **One whole line is enough.** A second bench standing somewhere with
    // nothing beside it is a half-built plan, not a reason to refuse an
    // order the base can already fill — and since `wants` staffs every
    // producer, refusing here would hide a line the scheduler would have
    // worked. The first break is what is reported when none of them is
    // whole, so the sentence still names a real missing link.
    let by_tile = structures_by_tile(game);
    let mut first_break = None;
    for machine in machines {
        let mut seen = std::collections::HashSet::new();
        match break_at(game, &by_tile, machine, &mut seen) {
            None => return None,
            Some(reason) => first_break.get_or_insert(reason),
        };
    }
    first_break
}

/// Every deployed structure by the tile it stands on. Built once per walk
/// rather than scanned per neighbour, which is the shape `assembler_system`
/// already uses to answer the same adjacency question.
pub(crate) fn structures_by_tile(game: &Game) -> std::collections::HashMap<(i32, i32), Entity> {
    game.world
        .iter_entities()
        .filter(|e| e.contains::<Structure>())
        .filter_map(|e| e.get::<Position>().map(|p| ((p.x, p.y), e.id())))
        .collect()
}

/// The recursive half of `chain_break`, walking *from the deployed
/// machine* rather than re-asking `producers_of` per ingredient — the
/// question is whether this machine's own neighbours can feed it, and a
/// producer standing somewhere else in the base is no answer to that.
///
/// `seen` is not an optimisation. Two machines that assemble each other's
/// ingredients are unreachable in the shipped assets but expressible by a
/// mod, and without it the walk would not terminate.
/// Whether the base has anywhere a worker can be sent to fetch from — any
/// deployed structure declaring `StructureDef::stores`.
///
/// Asked of the *base* rather than of a particular depot's distance,
/// for `Game::broker_reach`'s reason: a depot stands on laid floor by
/// construction, so which one it is says nothing the base does not.
fn shelf_is_standing(game: &Game) -> bool {
    let db = game.world.resource::<StructureDb>();
    game.world.iter_entities().any(|e| {
        e.get::<Structure>()
            .and_then(|s| db.get(&s.kind))
            .is_some_and(|d| d.stores)
    })
}

/// The one answer to "where can this machine's `ingredient` come from" —
/// every orthogonal neighbour producing it, or, when nothing beside it
/// does and a Depot is standing, every deployed producer of it wherever it
/// stands.
///
/// **The second arm is not a widening; it is the walk catching up with the
/// rule the runtime already uses.** `batch_within_reach` counts a Depot's
/// shelf through `depot_holding`, `can_progress` staffs a machine on the
/// strength of it and `hauling::Errand::Collect` walks a worker there. A
/// chain walk that asked only about neighbours was a fourth, narrower copy
/// of the reach rule, and it refused orders the base would have filled —
/// three machines in a row spaced two tiles apart, with the fragments they
/// all want sitting on the shelf between them.
///
/// **Structural, never a live stock count.** `chain_break` answers whether
/// a line can *ever* move; keyed to what a shelf happens to hold, the
/// picker would offer an item and then stop offering it as the shelf
/// drained. What varies with stock is `walk_feeders`' own shelf-before-bench
/// skip, which is a different question — who to post *now*.
fn feeders_for(
    game: &Game,
    by_tile: &std::collections::HashMap<(i32, i32), Entity>,
    pos: Position,
    ingredient: &ItemId,
) -> Vec<Entity> {
    let db = game.world.resource::<StructureDb>();
    let makes = |e: Entity| {
        game.world
            .get::<Structure>(e)
            .and_then(|s| db.get(&s.kind))
            .and_then(produced_item)
            == Some(ingredient)
    };
    let beside: Vec<Entity> = ORTHOGONAL
        .into_iter()
        .filter_map(|(dx, dy)| by_tile.get(&(pos.x + dx, pos.y + dy)).copied())
        .filter(|&e| makes(e))
        .collect();
    if !beside.is_empty() {
        return beside;
    }
    if !shelf_is_standing(game) {
        return Vec::new();
    }
    // Sorted by tile, so a base with two of the same producer resolves the
    // same way on every run — the reason `producers_of` sorts at all.
    producers_of(game, ingredient)
}

fn break_at(
    game: &Game,
    by_tile: &std::collections::HashMap<(i32, i32), Entity>,
    machine: Entity,
    seen: &mut std::collections::HashSet<Entity>,
) -> Option<String> {
    if !seen.insert(machine) {
        return None;
    }
    let db = game.world.resource::<StructureDb>();
    let items = game.world.resource::<ItemDb>();
    let kind = &game.world.get::<Structure>(machine)?.kind;
    let def = db.get(kind)?;
    let pos = *game.world.get::<Position>(machine)?;
    let Some(recipe) = assembly_recipe(def, items) else {
        // An extractor makes its product out of nothing on a timer, so the
        // walk terminates here rather than at a depth limit.
        return None;
    };
    let recipe: Vec<ItemId> = recipe.iter().map(|(item, _)| item.clone()).collect();
    for ingredient in recipe {
        let Some(&feeder) = feeders_for(game, by_tile, pos, &ingredient).first() else {
            let want = game.item_name(&ingredient);
            return Some(format!(
                "Nothing is making {want} within the {}'s reach — it can only take what a \
                 neighbour has finished, or what a worker can fetch off a Depot shelf.",
                def.name
            ));
        };
        if let Some(deeper) = break_at(game, by_tile, feeder, seen) {
            return Some(deeper);
        }
    }
    None
}

/// How many of `item` the base has **in store** — every Depot
/// (`StructureDef::stores`) and nothing else.
///
/// Deliberately narrower than `base_holding`, which counts machine output
/// buffers too. A buffer is the feed for whatever stands beside it; a Depot
/// is the shelf a worker can be sent to. Asking the wider question here
/// would have a bench count its own feeder's output twice — once as a
/// neighbour it can pull from, once as stores it could walk to — and skip
/// staffing that feeder on the strength of stock only the feeder itself
/// could ever have made.
pub(crate) fn depot_holding(game: &Game, item: &ItemId) -> u32 {
    let db = game.world.resource::<StructureDb>();
    game.world
        .iter_entities()
        .filter(|e| {
            e.get::<Structure>()
                .and_then(|s| db.get(&s.kind))
                .is_some_and(|d| d.stores)
        })
        .filter_map(|e| e.get::<Stock>())
        .map(|s| s.output.get(item).copied().unwrap_or(0))
        .sum()
}

/// Whether one batch of an ingredient is within a machine's reach, given
/// what its own input holds, what the machines touching it have finished,
/// and what the base has in store.
///
/// **The one statement of that rule.** `can_progress` asks it of a `&Game`
/// and `haul_step_system` asks it of its queries, and the two must not
/// drift: a scheduler that staffs a bench the walker will not fetch for
/// leaves a body standing at a starved machine forever, and the reverse
/// sends one on an errand nothing wanted.
///
/// Trivial arithmetic, and that is the point — it replaced a `beside.min(
/// want - held)` cap in `can_progress` that could never change the answer.
/// The cap bites only when `beside > want - held`, and since `want` is
/// `per_batch * INPUT_STOCK_BATCHES` that already implies
/// `held + beside > per_batch`, which is the true branch either way.
pub(crate) fn batch_within_reach(held: u32, beside: u32, in_store: u32, per_batch: u32) -> bool {
    held + beside + in_store >= per_batch
}

/// Whether staffing `machine` right now would actually move something.
///
/// Output has room, **and** for an assembler, one batch of every ingredient
/// is within reach: in its own input, in an orthogonally adjacent feeder's
/// output, or on a Depot shelf a worker can walk to. `batch_within_reach`
/// is that rule and `haul_step_system` reads the same one, so a machine the
/// scheduler staffs on the strength of stores is a machine the walker will
/// actually fetch for.
///
/// The second half is the load-bearing one. `assembler_system`'s pull phase
/// is *behind* the "is anyone posted here" gate, so a machine with nobody on
/// it never fills its own input — "the input is empty" is therefore not the
/// same question as "this machine has nothing to do", and reading it as one
/// would leave every empty bench permanently unstaffed.
///
/// This is also the predicate that *releases* a worker: a clogged machine
/// cannot progress, so it stops wanting a body, so the body goes somewhere
/// useful. That is how "work the deepest requirement until it is made, then
/// move on" falls out without the scheduler sequencing any phases.
pub(crate) fn can_progress(game: &Game, machine: Entity) -> bool {
    let Some(stock) = game.world.get::<Stock>(machine) else {
        return false;
    };
    if stock.output_room() == 0 {
        return false;
    }
    let db = game.world.resource::<StructureDb>();
    let items = game.world.resource::<ItemDb>();
    let Some(def) = game
        .world
        .get::<Structure>(machine)
        .and_then(|s| db.get(&s.kind))
    else {
        return false;
    };
    let Some(recipe) = assembly_recipe(def, items) else {
        // An extractor makes its product out of nothing on a timer, so room
        // in the output is the whole question for it.
        return produced_item(def).is_some();
    };
    let Some(pos) = game.world.get::<Position>(machine).copied() else {
        return false;
    };
    let by_tile = structures_by_tile(game);
    recipe.iter().all(|(item, per_batch)| {
        let held = stock.input.get(item).copied().unwrap_or(0);
        let beside: u32 = ORTHOGONAL
            .into_iter()
            .filter_map(|(dx, dy)| by_tile.get(&(pos.x + dx, pos.y + dy)).copied())
            .filter_map(|feeder| game.world.get::<Stock>(feeder))
            .map(|s| s.output.get(item).copied().unwrap_or(0))
            .sum();
        batch_within_reach(held, beside, depot_holding(game, item), *per_batch)
    })
}

/// Which machines this order needs a body on, deepest first.
///
/// Walks the recipe tree from the machine that makes the ordered item.
/// **A machine earns a place only if it `can_progress`** — staffing one that
/// would stand there doing nothing is what a scheduler must not do, and a
/// clogged machine dropping off this list is exactly how a body is released
/// downstream. The walk recurses *through* a machine either way, so an
/// upstream feeder that can work is found whether or not its consumer can.
///
/// The consequence worth knowing: on a base with every buffer empty this
/// returns the one extractor at the top of the line and nothing else,
/// because nothing else has anything to do yet. The full deepest-first line
/// appears as stock reaches each stage, which is also what lets two and
/// three staff spread down a chain without ever being posted somewhere
/// idle.
///
/// **A machine reached twice is kept once, at its deepest position** —
/// otherwise a shared feeder is staffed second on behalf of one branch while
/// the other still needs it first. No shipped assembler recipe has more than
/// one ingredient so the case is unreachable against the real assets, but
/// the engine's multi-input support is real and mods may ship one.
///
/// The sort is **total and stable** — `(depth, x, y, entity)` — for the
/// reason `assembler_system` sorts its machines: bevy's query iteration
/// order is not stable, and two machines at equal depth resolving
/// differently between runs is a flaky test and a base that behaves
/// differently after a reload.
pub(crate) fn wants(game: &Game, order: &WorkOrder) -> Vec<(Entity, u32)> {
    let mut deepest: std::collections::HashMap<Entity, u32> = std::collections::HashMap::new();
    let by_tile = structures_by_tile(game);
    // Every machine that makes the ordered item is a root of its own walk,
    // and `deepest` merges them: a feeder shared by two lines is kept once,
    // at the furthest depth either reached it. `seen` is path-local — lifted
    // again on the way out of `walk_wants` — so one set serves every root.
    let mut seen = std::collections::HashSet::new();
    for machine in producers_of(game, &order.item) {
        walk_wants(game, &by_tile, machine, 0, &mut deepest, &mut seen);
    }
    let mut list: Vec<(u32, Option<i32>, Option<i32>, Entity)> = deepest
        .into_iter()
        .map(|(e, depth)| {
            let pos = game.world.get::<Position>(e).map(|p| (p.x, p.y));
            (depth, pos.map(|p| p.0), pos.map(|p| p.1), e)
        })
        .collect();
    // Deepest first, then by tile, then by entity — a total order, so two
    // machines at equal depth never swap between runs.
    list.sort_by(|a, b| {
        b.0.cmp(&a.0)
            .then_with(|| (a.1, a.2, a.3).cmp(&(b.1, b.2, b.3)))
    });
    list.into_iter().map(|(d, _, _, e)| (e, d)).collect()
}

/// The recursive half of `wants`. `seen` guards a mod's mutually-feeding
/// pair, which would not terminate; `deepest` is what keeps a shared feeder
/// to one entry at the furthest depth it was reached.
fn walk_wants(
    game: &Game,
    by_tile: &std::collections::HashMap<(i32, i32), Entity>,
    machine: Entity,
    depth: u32,
    deepest: &mut std::collections::HashMap<Entity, u32>,
    seen: &mut std::collections::HashSet<Entity>,
) {
    // Guards a mod's mutually-feeding pair *along one path*, which is what
    // would not terminate. Lifted again on the way out, so a shared feeder
    // reached down a second, longer branch is still measured at that depth
    // — which is exactly the case the guard must not swallow.
    if !seen.insert(machine) {
        return;
    }
    walk_feeders(game, by_tile, machine, depth, deepest, seen);
    seen.remove(&machine);
}

/// The body of `walk_wants`, split out solely so every early return leaves
/// the `seen` guard to one caller rather than each remembering to lift it.
fn walk_feeders(
    game: &Game,
    by_tile: &std::collections::HashMap<(i32, i32), Entity>,
    machine: Entity,
    depth: u32,
    deepest: &mut std::collections::HashMap<Entity, u32>,
    seen: &mut std::collections::HashSet<Entity>,
) {
    if can_progress(game, machine) {
        let entry = deepest.entry(machine).or_insert(depth);
        *entry = (*entry).max(depth);
    }
    let db = game.world.resource::<StructureDb>();
    let items = game.world.resource::<ItemDb>();
    let Some(def) = game
        .world
        .get::<Structure>(machine)
        .and_then(|s| db.get(&s.kind))
    else {
        return;
    };
    let Some(recipe) = assembly_recipe(def, items) else {
        // An extractor has no upstream, so the walk terminates here. A
        // clogged one simply did not earn its place above, and something
        // else has to drain it before it wants a body again.
        return;
    };
    let recipe: Vec<(ItemId, u32)> = recipe.to_vec();
    let Some(pos) = game.world.get::<Position>(machine).copied() else {
        return;
    };
    for (ingredient, per_batch) in recipe {
        // **The shelf comes before the bench.** With a batch already in
        // store there is nothing to make, so the feeder that would make it
        // does not want a body — and since the list is sorted deepest-first,
        // leaving it in is what would send the one spare program upstream to
        // hand-make what the base is already holding.
        //
        // A batch, not a single unit: a shelf too thin to run a cycle off is
        // no answer, and skipping the feeder on the strength of it would
        // stall the order with nobody working anything.
        if depot_holding(game, &ingredient) >= per_batch {
            continue;
        }
        for feeder in feeders_for(game, by_tile, pos, &ingredient) {
            walk_wants(game, by_tile, feeder, depth + 1, deepest, seen);
        }
    }
}

/// Where the idle staff member at `index` stands at `tick`, on the
/// Chebyshev ring of `IDLE_STAFF_RING_TILES` around `center`.
///
/// **A deterministic function of its three arguments and nothing else.**
/// No `GameRng`, no local `StdRng`, no clock. That is not fastidiousness:
/// `CLAUDE.md` records three separate occasions where a shifted RNG stream
/// silently rewrote the outcome of a seeded test in an unrelated file, and
/// world generation is barred from that stream outright for the same
/// reason. A milling draw taken every tick for every idle program would
/// shift it harder than anything currently in the game.
///
/// Staff are spread around the ring by index and stepped together on
/// `IDLE_STAFF_STEP_TICKS`, so two programs never share a tile and the pool
/// reads as waiting rather than frozen. `stack::ring_offset` is this repo's
/// single definition of a Chebyshev ring and is what is walked here rather
/// than a second copy of the maths.
pub(crate) fn park_tile(center: Position, index: usize, tick: u64) -> Position {
    let band = crate::tuning::IDLE_STAFF_RING_TILES.max(1);
    let perimeter = 8 * band;
    let spread = index as i64 * perimeter as i64 / 4;
    let step = (tick / crate::tuning::IDLE_STAFF_STEP_TICKS) as i64;
    let along = (spread + step + index as i64).rem_euclid(perimeter as i64) as i32;
    let (dx, dy) = crate::game::stack::ring_offset(band, along);
    Position {
        x: center.x + dx,
        y: center.y + dy,
    }
}

/// How many of `item` the **base** is holding — every Depot
/// (`StructureDef::stores`) and every machine output buffer.
///
/// Deliberately not the player's inventory: an order says what the base
/// should hold, and where it holds it does not matter. What you are carrying
/// is yours.
pub(crate) fn base_holding(game: &Game, item: &ItemId) -> u32 {
    game.world
        .iter_entities()
        .filter(|e| e.contains::<Structure>())
        .filter_map(|e| e.get::<Stock>())
        .map(|s| s.output.get(item).copied().unwrap_or(0))
        .sum()
}

/// Every item the queue has asked for, each order's own item together with
/// everything its recipe tree is made of.
///
/// The closure that answers "does the base want what this machine makes",
/// which is what tells a feed buffer's owner from a bystander standing beside
/// it — see `hauling::consumer_beside`.
///
/// Over **`ItemDef::craftable`, not over deployed machines**, and that is
/// what makes it usable from a bevy system: `haul_step_system` has
/// `WorkOrders` and `ItemDb` and no `Game`, so an entity walk like `wants`'
/// would have had to be copied there. It is also the more stable question of
/// the two — whether the base has been *asked* for something does not
/// flicker as machines pass in and out of `can_progress`.
///
/// **The whole queue, not the order being worked.** A line feeding order
/// three must not be taken apart while order one is worked, and the only
/// thing that would come of it is the same goods walked to a Depot and back.
pub(crate) fn queue_needs(
    orders: &[WorkOrder],
    items: &ItemDb,
) -> std::collections::HashSet<ItemId> {
    let mut needed = std::collections::HashSet::new();
    let mut frontier: Vec<ItemId> = orders.iter().map(|o| o.item.clone()).collect();
    // `needed` doubles as the seen set, so a mod's pair of items that list
    // each other as ingredients terminates rather than spinning.
    while let Some(item) = frontier.pop() {
        if !needed.insert(item.clone()) {
            continue;
        }
        let Some(recipe) = items.get(item.as_str()).and_then(|d| d.craftable.as_ref()) else {
            continue;
        };
        frontier.extend(recipe.cost.iter().map(|(id, _)| id.clone()));
    }
    needed
}

impl Game {
    /// One tick of base labour: complete what is done, drop what is no
    /// longer wanted, fill what is.
    ///
    /// Deliberately a `&mut Game` method called from `tick_inner` rather
    /// than a bevy system, because posting a program is `&mut Game` work —
    /// it logs, reads structure defs through `work_ticks_for`, and writes
    /// `Party`. It runs **immediately before** `schedule.run`, beside
    /// `maybe_spawn_wild_creature`, so a body assigned this tick progresses
    /// this tick rather than idling until the next one.
    ///
    /// The five steps and their order are load-bearing:
    ///
    /// 1. Take the front order. If base storage already holds its quantity
    ///    the order is complete — pop it, announce it, take the next.
    /// 2. Build its wants. If the walk yields nothing the order is
    ///    **stalled**, not complete: leave it in the queue and take the next
    ///    instead, so one dead order cannot freeze a base that could still
    ///    work the three behind it.
    /// 3. Leave in place any staff already posted where a want still exists.
    /// 4. Unpost any staff whose machine no longer wants a body.
    /// 5. Fill the remaining wants, deepest first, from **idle** staff only.
    ///
    /// Steps 3 and 5 together are the anti-thrash rule and are not
    /// optional. A scheduler that rebuilt every posting each tick would walk
    /// the whole roster across the base whenever a buffer changed by one
    /// unit — and restart every cronjob's progress from zero doing it.
    pub(crate) fn schedule_base_labour(&mut self) {
        if self.is_game_over().is_some() || self.has_active_battle() {
            return;
        }
        let mut wanted: Vec<(Entity, TaskKind)> = self
            .settle_orders()
            .into_iter()
            .map(|(machine, _)| (machine, TaskKind::GatherResource))
            .collect();
        // Standing jobs, appended after the worked order and therefore at
        // the lowest priority: a spare body fills one, a needed body does
        // not. A work job is still gated on `can_progress` — a standing
        // instruction says "keep this running", not "stand here regardless"
        // — while a guard job is not, because guarding produces nothing and
        // there is no cycle for a buffer to stall.
        for (structure, kind) in self.standing_wants() {
            if wanted.iter().any(|&(e, _)| e == structure) {
                continue;
            }
            if kind == TaskKind::GatherResource && !can_progress(self, structure) {
                continue;
            }
            wanted.push((structure, kind));
        }
        // Dig jobs, appended after the standing jobs and therefore last of
        // all — **the priority is the position in this list**, since
        // `truncate(staff.len())` below cuts from the end. A spare body
        // digs; a needed one does not, and a plan drawn across the base can
        // never stop it running. Anything inserted above these silently
        // starves production.
        for (site, kind) in self.dig_wants() {
            wanted.push((site, kind));
        }
        let staff = self.base_staff();
        if staff.is_empty() {
            // A valid, quiet state: orders queue and report normally and
            // nothing is posted. The status screen says the base has nobody
            // in it, which is a different errand from an order being stuck.
            return;
        }
        self.park_idle_staff(&staff);
        // Posts already covered by somebody the scheduler may not move —
        // the player's own `work_structure` task, or a program posted by
        // hand outside the pool. A post with a body on it does not need a
        // second, and this is the only body the scheduler treats as
        // permanent.
        let outsiders: Vec<(Entity, TaskKind)> = self
            .world
            .iter_entities()
            .filter(|e| !e.contains::<BaseStaff>())
            .filter_map(|e| e.get::<Task>())
            .map(|t| (t.target, t.kind))
            .collect();
        wanted.retain(|post| !outsiders.contains(post));

        // The staff are fewer than the posts most of the time, so the list
        // is cut to what can actually be filled — **in priority order**,
        // which is what makes an order outrank a standing job for a scarce
        // body. Deciding the whole assignment first and diffing against
        // what is posted is what lets the priority rule and the anti-thrash
        // rule both hold: filling greedily around the postings that already
        // exist would leave a body on a standing job while an order went
        // unworked, because the body was already somewhere "wanted".
        wanted.truncate(staff.len());

        // **The scheduler never takes a body off a post unless it has
        // somewhere better to put it.** With every wanted post already
        // filled there is no gain in moving anyone, and real harm in it: a
        // base whose queue has run dry — or one loaded from a save written
        // before work orders existed, whose workers were all posted by hand
        // — would otherwise be stood down wholesale on the first tick,
        // which is the exact regression `Game::load`'s absorption rule
        // exists to prevent.
        let posted: Vec<(Entity, TaskKind)> = staff
            .iter()
            .filter_map(|&e| self.world.get::<Task>(e))
            .map(|t| (t.target, t.kind))
            .collect();
        if wanted.iter().all(|post| posted.contains(post)) {
            return;
        }

        // Step 3 and step 4 in one pass: anyone already standing at a post
        // the assignment keeps stays exactly where they are (that is the
        // anti-thrash rule, and it is what stops a cronjob's progress being
        // restarted from zero every tick); everyone else is freed.
        let mut idle = Vec::new();
        let mut remaining = wanted.clone();
        for &worker in &staff {
            let held = self.world.get::<Task>(worker).map(|t| (t.target, t.kind));
            // **A body mid-delivery is never freed.** The same rule as the
            // one above, one case wider: a worker holding a load has
            // somewhere to be. Freeing it drops `Carrying` along with the
            // `Task`, and by then the units have already been taken *out* of
            // the machine's stock — so the goods are destroyed rather than
            // released. Rare while a worker only ever set off from a clogged
            // machine; routine now that one sets off every cycle.
            if self.world.get::<Carrying>(worker).is_some() {
                if let Some(index) = held.and_then(|post| remaining.iter().position(|&p| p == post))
                {
                    remaining.remove(index);
                }
                continue;
            }
            match held.and_then(|post| remaining.iter().position(|&p| p == post)) {
                Some(index) => {
                    remaining.remove(index);
                }
                None => {
                    self.world
                        .entity_mut(worker)
                        .remove::<Task>()
                        .remove::<Carrying>();
                    idle.push(worker);
                }
            }
        }
        idle.reverse();

        // Step 5, deepest first — and standing jobs last, since they were
        // appended after the order's wants and the list is not re-sorted.
        //
        // **Every question here is asked from the body's own tile**, never
        // from the player's. `park_idle_staff` above has just put each free
        // program on a real tile inside the base, and a program the
        // scheduler freed this tick is standing at the post it just left —
        // so the walk starts where the body is, and whether it arrives is a
        // question about the base rather than about where you happen to be
        // stood. Measured from the player it did both jobs wrong at once: a
        // loitering program teleported across the map onto you, and walking
        // out of the walk field stopped the base filling a single machine.
        let blocked = self.structure_tiles();
        let pocket_radius = self.world.resource::<BaseGrid>().radius();
        for (post, kind) in remaining {
            // Peeked rather than popped, so a skipped post costs no body.
            let Some(&worker) = idle.last() else {
                break;
            };
            let from = self
                .world
                .get::<Position>(worker)
                .copied()
                .unwrap_or(Position { x: 0, y: 0 });
            if kind == TaskKind::Excavate
                && !self.can_walk_to_dig(from, post, &blocked, pocket_radius)
            {
                // Both refusals are silent *here*: `can_walk_to_dig` owns
                // the announcement, because only it can tell the interior of
                // a marked block from a cell the player has walled off.
                continue;
            }
            if kind == TaskKind::GatherResource
                && !self.can_walk_to_post(from, post, &blocked, pocket_radius)
            {
                // A machine the base has been built around, or one with no
                // route from where the player is standing. **Skipped rather
                // than filled**: `hauling::post_reach` is the one predicate
                // for "is this a posting that arrives", and it used to be
                // `assign_cronjob`'s — a menu that offered a post the walker
                // could never complete. The scheduler inherits the question
                // rather than dropping it, because a body sent to an
                // unreachable machine is a body lost for the rest of the
                // run: it would sit `Stranded` forever while the order it
                // was meant to work went unstaffed. Left unfilled, the
                // machine reads as idle on the status screen and the body
                // goes to the next want instead.
                continue;
            }
            idle.pop();
            match kind {
                TaskKind::GatherResource => self.post_worker(worker, post),
                TaskKind::Guard => self.post_guard(worker, post),
                TaskKind::Excavate => self.post_digger(worker, post),
            }
        }
    }

    /// Whether a program setting off from `from` could actually reach a post
    /// at `machine` — `hauling::post_reach`, the same question
    /// `haul_step_system` asks when it walks one.
    fn can_walk_to_post(
        &mut self,
        from: Position,
        machine: Entity,
        blocked: &std::collections::HashSet<(i32, i32)>,
        pocket_radius: i32,
    ) -> bool {
        self.post_route(from, machine, blocked, pocket_radius)
            .is_ok()
    }

    /// `can_walk_to_post`'s answer with its reason kept, which is what a dig
    /// site needs and a machine does not.
    ///
    /// A target with no `Position` reads as `BoxedIn`: nothing can stand
    /// beside a post that is not anywhere, and the caller skips it in
    /// silence either way.
    fn post_route(
        &mut self,
        from: Position,
        target: Entity,
        blocked: &std::collections::HashSet<(i32, i32)>,
        pocket_radius: i32,
    ) -> Result<(), hauling::NoPost> {
        let Some(to) = self.world.get::<Position>(target).copied() else {
            return Err(hauling::NoPost::BoxedIn);
        };
        let grid = self.world.resource::<BaseGrid>();
        hauling::post_reach(grid, from, to, blocked, pocket_radius)
    }

    /// Whether a body at `from` can be sent to `site`, and **the one place a
    /// stalled excavation is announced**.
    ///
    /// The two ways a dig site can refuse a body are not symmetrical, and
    /// that asymmetry is the whole of this function:
    ///
    /// - `BoxedIn` — nothing can stand beside the cell at all. That is the
    ///   ordinary *interior* of any block the player marked, and it resolves
    ///   itself as the shell comes down. **Silent**, or a nine-cell plan
    ///   would complain about its own middle every tick until it was dug.
    /// - `NoRoute` — a face exists and no body can reach it. Only the player
    ///   can fix that, so it **says so once** and then stops, which is
    ///   `systems::set_machine_status`' rule one subsystem over: entering a
    ///   state is news, staying in it is not. `DigSite::announced_stuck` is
    ///   the latch, and it clears the moment a route exists again — without
    ///   that the second stall would be silent forever.
    fn can_walk_to_dig(
        &mut self,
        from: Position,
        site: Entity,
        blocked: &std::collections::HashSet<(i32, i32)>,
        pocket_radius: i32,
    ) -> bool {
        let route = self.post_route(from, site, blocked, pocket_radius);
        let announced = self
            .world
            .get::<DigSite>(site)
            .is_some_and(|d| d.announced_stuck);
        match route {
            Ok(()) => {
                if announced && let Some(mut dig) = self.world.get_mut::<DigSite>(site) {
                    dig.announced_stuck = false;
                }
                true
            }
            Err(hauling::NoPost::BoxedIn) => false,
            Err(hauling::NoPost::NoRoute) => {
                if !announced {
                    if let Some(mut dig) = self.world.get_mut::<DigSite>(site) {
                        dig.announced_stuck = true;
                    }
                    let at = self.world.get::<Position>(site).copied();
                    if let Some(at) = at {
                        self.log_base(format!(
                            "The marked cell at {}, {} is cut off — no program can find a way to it.",
                            at.x, at.y
                        ));
                    }
                }
                false
            }
        }
    }

    /// Every marked dig site, in a stable tile order — the lowest-priority
    /// wants the base has.
    ///
    /// Structural, like `feeders_for` and unlike a stock count: what makes a
    /// site a want is that the player marked it, not whether the base can
    /// pay for the tile that finishes it. A want that flickered as the
    /// substrate shelf drained would walk bodies on and off the frontier for
    /// the rest of the run.
    ///
    /// One want per site whichever half of the verb it is on: a marked solid
    /// cell wants cutting, a marked `Open` one wants flooring, and the same
    /// body does both because the mark outlives the cut.
    fn dig_wants(&mut self) -> Vec<(Entity, TaskKind)> {
        let mut sites: Vec<(i32, i32, Entity)> = {
            let mut query = self.world.query::<(Entity, &DigSite, &Position)>();
            query
                .iter(&self.world)
                .filter(|(_, dig, _)| dig.marked)
                .map(|(e, _, p)| (p.x, p.y, e))
                .collect()
        };
        sites.sort_unstable();
        sites
            .into_iter()
            .map(|(_, _, e)| (e, TaskKind::Excavate))
            .collect()
    }

    /// Walks every staff member with no post to its parking tile.
    ///
    /// Runs before the assignment rather than after it, and that ordering
    /// is what makes a posting truthful: a posted program's `Position` is
    /// what `haul_step_system` walks from and what `post_reach` is asked
    /// about, and neither is overwritten any more, so the tile a program is
    /// standing on when it *gets* a post has to already be a real one.
    ///
    /// The ring respects the same two rules a posted worker's field does:
    /// carved out in `BaseGrid`, and never a tile a `Structure` stands on. A
    /// candidate that breaks either is simply not taken — the program holds
    /// its ground for that beat rather than being nudged somewhere the rule
    /// would have refused.
    fn park_idle_staff(&mut self, staff: &[Entity]) {
        let Some(home) = self.home_position() else {
            // No Home means no base to loiter at, and no origin for the ring
            // to be laid out around either.
            return;
        };
        let tick = self.world.resource::<GameClock>().tick;
        let occupied = structures_by_tile(self);
        for (index, &worker) in staff.iter().enumerate() {
            if self.world.get::<Task>(worker).is_some() {
                continue;
            }
            let tile = park_tile(home, index, tick);
            if occupied.contains_key(&(tile.x, tile.y)) {
                continue;
            }
            if !self.world.resource::<BaseGrid>().walkable(tile.x, tile.y) {
                continue;
            }
            if let Some(mut pos) = self.world.get_mut::<Position>(worker) {
                *pos = tile;
            }
        }
    }

    /// Sets or clears the standing instructions on `structure` — see
    /// `components::StandingJob`.
    ///
    /// A guard job on something that cannot be swept is refused, and by
    /// exactly the check `assign_guard` already carries rather than a
    /// restatement of it: the same question deserves the same sentence.
    pub fn set_standing_job(
        &mut self,
        structure: Entity,
        work: bool,
        guard: bool,
    ) -> Result<(), String> {
        let kind = self
            .world
            .get::<Structure>(structure)
            .ok_or_else(|| "That's not a structure.".to_string())?
            .kind
            .clone();
        if guard {
            let unraidable = self
                .world
                .resource::<StructureDb>()
                .get(&kind)
                .filter(|def| !def.raidable)
                .map(|def| def.name.clone());
            if let Some(name) = unraidable {
                return Err(format!("{name} can't be raided — it doesn't need a guard."));
            }
        }
        if work && !self.accepts_a_program(structure) {
            return Err("That structure can't be worked.".into());
        }
        let mut entity = self.world.entity_mut(structure);
        if work || guard {
            entity.insert(StandingJob { work, guard });
        } else {
            entity.remove::<StandingJob>();
        }
        Ok(())
    }

    /// What a base staff member is doing right now, for the staff screen —
    /// the machine it is on, what it is guarding, or that it is waiting.
    ///
    /// Derived from the live `Task` rather than tracked, like everything
    /// else here: the scheduler moves a body whenever the answer changes,
    /// so a stored one would be a tick behind at best.
    pub fn staff_activity(&self, worker: Entity) -> String {
        let Some(task) = self.world.get::<Task>(worker) else {
            return "idle".to_string();
        };
        let name = self
            .world
            .get::<Structure>(task.target)
            .and_then(|s| self.world.resource::<StructureDb>().get(&s.kind))
            .map(|def| def.name.clone())
            .unwrap_or_else(|| "something".to_string());
        match task.kind {
            TaskKind::GatherResource => format!("working the {name}"),
            TaskKind::Guard => format!("guarding the {name}"),
            // No `name`: a dig site is not a structure and has none. What it
            // has is a tile, and that is what tells one dig job from another.
            TaskKind::Excavate => self
                .world
                .get::<Position>(task.target)
                .map(|p| format!("cutting the entropy at {}, {}", p.x, p.y))
                .unwrap_or_else(|| "cutting the entropy".to_string()),
        }
    }

    pub fn standing_job(&self, structure: Entity) -> Option<(bool, bool)> {
        self.world
            .get::<StandingJob>(structure)
            .map(|j| (j.work, j.guard))
    }

    /// Every standing job in the base, in a stable tile order.
    ///
    /// Appended **after** whatever order is being worked and at the lowest
    /// priority, so a Research Node or a guarded Shield is filled only by a
    /// body no order needs — and gives that body up the moment one does.
    fn standing_wants(&self) -> Vec<(Entity, TaskKind)> {
        let mut jobs: Vec<(i32, i32, Entity, TaskKind)> = self
            .world
            .iter_entities()
            .filter_map(|e| {
                let job = e.get::<StandingJob>()?;
                let pos = e.get::<Position>()?;
                let kind = if job.work {
                    TaskKind::GatherResource
                } else if job.guard {
                    TaskKind::Guard
                } else {
                    return None;
                };
                Some((pos.x, pos.y, e.id(), kind))
            })
            .collect();
        jobs.sort_by_key(|&(x, y, e, _)| (x, y, e));
        jobs.into_iter().map(|(_, _, e, k)| (e, k)).collect()
    }

    /// Steps 1 and 2: pop every completed order off the front, skip a
    /// stalled one, and return the want list of the first order that has
    /// work in it.
    ///
    /// Returns empty when nothing is orderable — which is also what a base
    /// with an empty queue looks like, and the two are the same instruction
    /// to the caller.
    fn settle_orders(&mut self) -> Vec<(Entity, u32)> {
        let mut index = 0;
        loop {
            let Some(order) = self
                .world
                .resource::<resources::WorkOrders>()
                .0
                .get(index)
                .cloned()
            else {
                return Vec::new();
            };
            if base_holding(self, &order.item) >= order.qty {
                let name = self.item_name(&order.item).to_string();
                let qty = order.qty;
                self.world
                    .resource_mut::<resources::WorkOrders>()
                    .0
                    .remove(index);
                self.log_base_kind(
                    MessageKind::Complete,
                    format!("Work order complete: {qty} x {name}."),
                );
                continue;
            }
            let list = wants(self, &order);
            if list.is_empty() {
                // Stalled. It keeps its place in the queue so the status
                // screen can say which machine went missing; the player
                // cancels it or rebuilds.
                index += 1;
                continue;
            }
            return list;
        }
    }

    /// Queues an order for `qty` of `item`, or names why the line for it can
    /// never move.
    ///
    /// Every refusal runs **before** anything is pushed — the same ordering
    /// argument `use_symlink` makes about `clear_stack` and
    /// `install_routine` makes about the disk. A refused order leaves the
    /// queue exactly as it was.
    pub fn queue_work_order(&mut self, item: ItemId, qty: u32) -> Result<(), String> {
        if qty == 0 {
            return Err("An order for nothing is not an order.".into());
        }
        // The same `Position` trap `find_target_in_direction` fell into: a
        // party underground has its `Position` pinned to the surface
        // entrance tile, so anything claiming something about where the base
        // is has to refuse rather than answer from a tile four frames up.
        // `require_base` rather than `require_surface` since the base went
        // out of phase — this reads which machines are standing, and they
        // stand in base space.
        self.require_base()?;
        if let Some(reason) = chain_break(self, &item) {
            return Err(reason);
        }
        let name = self.item_name(&item).to_string();
        self.world
            .resource_mut::<resources::WorkOrders>()
            .0
            .push(WorkOrder { item, qty });
        self.log_base(format!("Work order filed: {qty} x {name}."));
        Ok(())
    }

    /// Every item this base could actually be told to make, as
    /// `(id, display name)` in name order.
    ///
    /// The list a frontend offers, and it asks `chain_break` — the same
    /// question `queue_work_order` refuses on — so the picker cannot list a
    /// row the queue would then reject. That is the rule
    /// `App::base_menu_rows` holds for a menu whose rows are hidden
    /// dynamically, one level down.
    pub fn orderable_items(&self) -> Vec<(ItemId, String)> {
        let mut rows: Vec<(ItemId, String)> = self
            .item_defs()
            .into_iter()
            .filter(|def| chain_break(self, &def.id).is_none())
            .map(|def| (def.id.clone(), def.name.clone()))
            .collect();
        rows.sort_by(|a, b| a.1.cmp(&b.1));
        rows
    }

    /// Drops the order at `index`, shifting the ones behind it up.
    ///
    /// **Unwinds nothing, because nothing was wound.** There are no
    /// per-machine targets to roll back and no reserved stock to release —
    /// the next tick simply derives a different answer. That is the
    /// derived-never-stored decision paying out somewhere it was not
    /// designed for.
    pub fn cancel_work_order(&mut self, index: usize) -> Result<(), String> {
        let dropped = {
            let mut orders = self.world.resource_mut::<resources::WorkOrders>();
            if index >= orders.0.len() {
                return Err("No such work order.".into());
            }
            orders.0.remove(index)
        };
        let name = self.item_name(&dropped.item).to_string();
        self.log_base(format!("Work order cancelled: {name}."));
        Ok(())
    }

    pub fn work_orders(&self) -> &[WorkOrder] {
        &self.world.resource::<resources::WorkOrders>().0
    }

    /// What every queued order is waiting on, in queue order.
    ///
    /// Built **by calling `wants`**, not by walking the chain a second
    /// time. Per `CLAUDE.md`, a claim that two places use one rule has to
    /// be a call and not a comment — and the copy that drifts is the one
    /// nobody runs, which here would be the screen telling a player their
    /// base is doing something it is not.
    pub fn work_order_report(&self) -> Vec<views::WorkOrderReport> {
        let db = self.world.resource::<StructureDb>();
        let items = self.world.resource::<ItemDb>();
        self.work_orders()
            .iter()
            .map(|order| {
                let machines = wants(self, order);
                views::WorkOrderReport {
                    item: order.item.clone(),
                    label: self.item_name(&order.item).to_string(),
                    have: base_holding(self, &order.item),
                    target: order.qty,
                    stalled: machines.is_empty(),
                    blocked_by: machines
                        .is_empty()
                        .then(|| chain_break(self, &order.item))
                        .flatten(),
                    machines: machines
                        .into_iter()
                        .map(|(machine, depth)| views::WorkOrderMachine {
                            entity: machine,
                            label: self
                                .world
                                .get::<Structure>(machine)
                                .and_then(|s| db.get(&s.kind))
                                .map(|def| def.name.clone())
                                .unwrap_or_default(),
                            worker: self.posted_worker_name(machine),
                            short_of: self.machine_shortfall(machine, db, items),
                            depth,
                        })
                        .collect(),
                }
            })
            .collect()
    }

    /// The name of whoever holds a `GatherResource` post at `machine`.
    fn posted_worker_name(&self, machine: Entity) -> Option<String> {
        let holder = self
            .world
            .iter_entities()
            .find(|e| {
                e.get::<Task>()
                    .is_some_and(|t| t.target == machine && t.kind == TaskKind::GatherResource)
            })?
            .id();
        Some(self.creature_label(holder))
    }

    /// The first ingredient `machine` cannot assemble a batch from, named
    /// for the screen. `None` for an extractor, and for an assembler with
    /// everything it needs within reach.
    fn machine_shortfall(
        &self,
        machine: Entity,
        db: &StructureDb,
        items: &ItemDb,
    ) -> Option<String> {
        let def = self
            .world
            .get::<Structure>(machine)
            .and_then(|s| db.get(&s.kind))?;
        let recipe = assembly_recipe(def, items)?;
        let stock = self.world.get::<Stock>(machine)?;
        let pos = self.world.get::<Position>(machine).copied()?;
        let by_tile = structures_by_tile(self);
        recipe
            .iter()
            .find(|(item, per_batch)| {
                let held = stock.input.get(item).copied().unwrap_or(0);
                let beside: u32 = ORTHOGONAL
                    .into_iter()
                    .filter_map(|(dx, dy)| by_tile.get(&(pos.x + dx, pos.y + dy)).copied())
                    .filter_map(|feeder| self.world.get::<Stock>(feeder))
                    .map(|s| s.output.get(item).copied().unwrap_or(0))
                    .sum();
                held + beside < *per_batch
            })
            .map(|(item, _)| self.item_name(item).to_string())
    }
}
