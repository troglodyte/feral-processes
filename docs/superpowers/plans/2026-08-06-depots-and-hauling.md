# Depots and Hauling Programs Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A program posted to a machine walks its full output buffer to a depot and back, visibly, on the map.

**Architecture:** One new component (`Carrying`), one new system (`haul_step_system`), one new `MachineStatus` variant, one new `.ron` structure. All other state is *derived* from `Position` rather than stored — destination is "nearest depot if carrying, else my machine", and "at post" is orthogonal adjacency. The existing `MachineStatus::Clogged` branch in `task_progress_system` becomes the departure point.

**Tech Stack:** Rust 2024, standalone `bevy_ecs` 0.19 (engine has no full Bevy), `pathfinding` crate's `dijkstra_all`, RON assets, bincode saves.

**Spec:** `docs/superpowers/specs/2026-08-06-depots-and-hauling-design.md`

## Global Constraints

- **Branch:** work on `depots-and-hauling` (already created; the spec commit is on it).
- **Build/test:** `cargo test -p feral-processes-engine <name>` while iterating (~3s). `cargo test --workspace` is the final gate only. `cargo fmt` and `cargo clippy --workspace` after every task.
- **The renderer never touches the ECS `World`.** `Game`'s `world` field is private with no accessor. Anything gui needs comes through a view type in `crates/engine/src/views.rs`.
- **No hardcoded content.** The depot is a `.ron` file in `assets/structures/`. No Rust may name it except test fixtures.
- **Tuning values are `pub const` in `crates/engine/src/tuning.rs`**, never inline in a formula.
- **No flaky tests.** No `sleep()`, no wall-clock, no unseeded RNG. Bevy query iteration order is not stable — any test where two entities compete must spawn them in the *opposite* order to the one being asserted.
- **Comments explain *why*, never *what*.**
- **Commit at every green step.** Do not push; pushing needs an explicit ask.
- `ORTHOGONAL` is `crate::game::collect::ORTHOGONAL`, `pub(crate) const ORTHOGONAL: [(i32, i32); 4] = [(0, -1), (0, 1), (-1, 0), (1, 0)]`. Reuse it; do not write a second adjacency list.

---

## File Structure

**Create:**
- `crates/engine/src/game/hauling.rs` — `haul_step_system`, `nearest_depot`, `station_tile`, `at_station`. New module because `systems.rs` is already 1087 lines; `game/pursuit.rs` and `difficulty.rs` are the precedent for systems and pathing helpers living outside it.
- `crates/engine/src/tests/hauling.rs` — all tests for this feature.
- `assets/structures/depot.ron` — the depot.

**Modify:**
- `crates/engine/src/game/pursuit.rs` — extract `walk_field`; `pursuit_field` becomes a wrapper.
- `crates/engine/src/components.rs:341-395` — add `Carrying`, add `MachineStatus::Unstaffed`.
- `crates/engine/src/tuning.rs` — add `HAUL_CARRY_CAPACITY`.
- `crates/engine/src/systems.rs:284-300` (`set_machine_status` log arms) and `:396-418` (the loop head and clogged branch).
- `crates/engine/src/game/mod.rs` — declare `mod hauling;`.
- `crates/engine/src/game/lifecycle.rs:143-158` — register the system in the chain.
- `crates/engine/src/game/building.rs` (`remove_structure`, ~line 310) and `crates/engine/src/game/upkeep.rs` (`damage_structure`, ~line 250) — drop `Carrying` alongside `Task`.
- `crates/engine/src/save.rs:56-90` (`CreatureSave`) and `:314-316` (`SAVE_FORMAT_VERSION`).
- `crates/engine/src/game/lifecycle.rs:591-660` (save write) and `:415-440` (save restore).
- `crates/gui/src/render/base.rs:424-427` and `crates/gui/src/render/building.rs:473-475` — the two `MachineStatus` matches.
- `crates/engine/src/tests/mod.rs` — declare `mod hauling;`.
- `dev-saves/*.ron`, `CHANGELOG.md`, `Cargo.toml` (version), `CLAUDE.md` + `AGENTS.md`.

---

### Task 1: Extract `walk_field` from `pursuit_field`

Pure refactor, no behaviour change. `pursuit_field` excludes `Biome::Platform` — the base slab — because that rule keeps a nest swarm off the player's base. A hauler must walk *across* the slab, which is exactly the tile set that filter removes. Copying the Dijkstra walk is what CLAUDE.md forbids, so the walk gets a per-caller step predicate.

**Files:**
- Modify: `crates/engine/src/game/pursuit.rs`
- Test: `crates/engine/src/game/pursuit.rs` (its existing `#[cfg(test)] mod tests`, which already has `floor()` and `wall()` helpers)

**Interfaces:**
- Consumes: nothing.
- Produces:
  ```rust
  pub(crate) fn walk_field(
      map: &mut WorldMap,
      origin: (i32, i32),
      radius: i32,
      step_allowed: impl Fn(&Tile) -> bool,
  ) -> HashMap<(i32, i32), u32>
  ```
  `pursuit_field` keeps its exact current signature and becomes a one-line wrapper.

- [ ] **Step 1: Write the failing test**

Add to the existing `mod tests` in `crates/engine/src/game/pursuit.rs`. The `floor()`/`wall()` helpers are already there; add a `platform()` helper beside them.

```rust
fn platform() -> Tile {
    Tile {
        biome: Biome::Platform,
        walkable: true,
    }
}

/// The whole reason `walk_field` exists: two callers disagree about the
/// base slab, and only one of them may cross it.
#[test]
fn walk_field_crosses_a_platform_that_pursuit_field_refuses() {
    let mut map = WorldMap::new(7);
    for x in -2..=2 {
        for y in -2..=2 {
            map.set_override(x, y, floor());
        }
    }
    map.set_override(1, 0, platform());

    let pursued = pursuit_field(&mut map, (0, 0), 2);
    assert!(
        !pursued.contains_key(&(1, 0)),
        "pursuit must still refuse the base slab"
    );

    let walked = walk_field(&mut map, (0, 0), 2, |t| t.walkable);
    assert_eq!(
        walked.get(&(1, 0)),
        Some(&1),
        "a hauler must be able to cross the base slab"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-engine walk_field_crosses_a_platform`
Expected: FAIL to compile — `cannot find function 'walk_field' in this scope`.

- [ ] **Step 3: Write the minimal implementation**

In `crates/engine/src/game/pursuit.rs`:

- Rename the existing `pursuit_field` body to `walk_field`, adding the fourth parameter `step_allowed: impl Fn(&Tile) -> bool`.
- Inside the successor closure, replace the inlined `tile.walkable && tile.biome != Biome::Platform` check with `step_allowed(&tile)`. Keep the radius bound on the *successors* exactly as it is — the comment there explains it is what bounds an unbounded search on a lazily-generated infinite map, and that reasoning is unchanged.
- Keep the `map.tile(nx, ny)` call taking `&mut self` inside the closure. `dijkstra_all` takes `FnMut`, which is what permits the mutable borrow; the existing comment covers this and should move with the code.
- Redefine `pursuit_field` with its current signature and doc comment as:

  ```rust
  walk_field(map, origin, radius, |t| {
      t.walkable && t.biome != Biome::Platform
  })
  ```

  Move only the `Platform`-specific half of the doc comment onto `pursuit_field`; the "a tile absent from the map is unreachable" paragraph belongs on `walk_field`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine pursuit`
Expected: PASS, including every pre-existing test in that module unchanged.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p feral-processes-engine
git add crates/engine/src/game/pursuit.rs
git commit -m "refactor(pursuit): one walk, a step rule per caller"
```

---

### Task 2: `Carrying`, `HAUL_CARRY_CAPACITY`, and departure

The worker picks up a bounded load and loses its `Task` progress to nothing — `progress` stays held at `required` so the machine pays out the instant the worker is back.

**Files:**
- Modify: `crates/engine/src/components.rs` (after `Stock`'s impl block, ~line 365)
- Modify: `crates/engine/src/tuning.rs` (beside `DEFAULT_OUTPUT_CAPACITY`, ~line 975)
- Modify: `crates/engine/src/systems.rs:365-418`
- Create: `crates/engine/src/tests/hauling.rs`
- Modify: `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces:
  ```rust
  #[derive(Component, Clone, Debug)]
  pub struct Carrying {
      pub item: ItemId,
      pub qty: u32,
  }

  pub const HAUL_CARRY_CAPACITY: u32 = 5;
  ```
  `task_progress_system` gains a `mut commands: Commands` parameter (bringing it to 7, which is at but not over clippy's `too_many_arguments` threshold).

- [ ] **Step 1: Write the failing test**

Create `crates/engine/src/tests/hauling.rs`. Add `mod hauling;` to `crates/engine/src/tests/mod.rs` in alphabetical position (between `mod field;` and `mod inspection;`).

```rust
use crate::tests::support::*;
use crate::*;

/// A hand-spawned work node short of `Stock` or `MachineStatus` is skipped
/// by `task_progress_system`'s query and silently produces nothing, which
/// reads as a payout curve that moved rather than a broken fixture — hence
/// `work_node_parts()`.
fn clogged_node_with_worker(game: &mut Game) -> (Entity, Entity) {
    place_home(game, 2, 0);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    let worker = spawn_tamed(game, "drone");
    game.assign_cronjob(worker, node).unwrap();

    // Fill the output to the brim so the next completed cycle clogs.
    let cap = game.world.get::<Stock>(node).unwrap().capacity;
    let mut stock = game.world.get_mut::<Stock>(node).unwrap();
    stock.output.insert(ItemId::from(ids::CORE_FRAGMENT), cap);

    (node, worker)
}

#[test]
fn a_clogged_machine_sends_its_worker_off_with_a_bounded_load() {
    let mut game = Game::new(1);
    let (node, worker) = clogged_node_with_worker(&mut game);

    // Park the worker at its post so it produces rather than walking.
    let node_pos = *game.world.get::<Position>(node).unwrap();
    game.world.get_mut::<Position>(worker).unwrap().x = node_pos.x + 1;
    game.world.get_mut::<Position>(worker).unwrap().y = node_pos.y;

    for _ in 0..40 {
        game.tick();
        if game.world.get::<Carrying>(worker).is_some() {
            break;
        }
    }

    let carrying = game
        .world
        .get::<Carrying>(worker)
        .expect("a clogged machine's worker should pick up a load");
    assert_eq!(carrying.qty, tuning::HAUL_CARRY_CAPACITY);
    assert_eq!(carrying.item, ItemId::from(ids::CORE_FRAGMENT));

    let task = game.world.get::<Task>(worker).unwrap();
    assert_eq!(
        task.progress, task.required,
        "progress must stay held at required so the machine pays out the \
         tick the worker is back, not restart the cycle"
    );

    let left = game.world.get::<Stock>(node).unwrap().output_used();
    assert_eq!(
        left,
        game.world.get::<Stock>(node).unwrap().capacity - tuning::HAUL_CARRY_CAPACITY,
        "the cap is what leaves a buffer for a downstream neighbour to pull from"
    );
}
```

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-engine a_clogged_machine_sends_its_worker`
Expected: FAIL to compile — `cannot find type 'Carrying'`.

- [ ] **Step 3: Write the minimal implementation**

`crates/engine/src/tuning.rs`, beside `DEFAULT_OUTPUT_CAPACITY`:

```rust
/// How many units a posted program carries to a depot in one trip. The cap
/// is what makes `Carrying` a single `(item, qty)` pair rather than a map:
/// `Stock::output` is a `BTreeMap` and can hold more than one item id, so an
/// uncapped drain would have to carry all of them. It is also what leaves a
/// buffer behind for a downstream neighbour to keep pulling from across the
/// round trip.
pub const HAUL_CARRY_CAPACITY: u32 = 5;
```

`crates/engine/src/components.rs`, after `Stock`'s impl block. Add `Carrying` as above. Its doc comment must record *why* it is the only stored state: destination, "at post" and "in transit" are all derived from `Position`, so there is one source of truth and no state field to desync.

`crates/engine/src/systems.rs`:
- Add `mut commands: Commands` to `task_progress_system`'s parameters.
- The query needs the worker `Entity`, so change `CronjobWorker` to lead with `Entity` and destructure accordingly at the loop head (`for (worker, mut task, creature, potential, mut exp, mut stats) in &mut tasks`).
- In the clogged branch (currently `systems.rs:414-418`), after `set_machine_status(...)` and before `continue`: take the first key of `stock.output` in `BTreeMap` order, move `min(HAUL_CARRY_CAPACITY, qty)` out of it (removing the key entirely if it hits zero), and `commands.entity(worker).insert(Carrying { item, qty })`. Guard on the worker not already carrying — a worker that already has a load must not pick up a second.

The borrow to watch: `stock` is a `Mut<Stock>` from `nodes.get_mut(task.target)`. Read the first key into an owned `ItemId` before mutating the map, or the immutable borrow from `.keys().next()` will still be live.

Export `Carrying` from `crates/engine/src/lib.rs`'s component re-export list (line ~47, alongside `Task`, `TaskKind`).

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine hauling`
Expected: PASS.

Then: `cargo test -p feral-processes-engine chains` — the existing chain tests must be unaffected, because no depot exists yet and nothing consumes `Carrying`.
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy -p feral-processes-engine
git add crates/engine/src/components.rs crates/engine/src/tuning.rs \
        crates/engine/src/systems.rs crates/engine/src/lib.rs \
        crates/engine/src/tests/hauling.rs crates/engine/src/tests/mod.rs
git commit -m "feat(hauling): a clogged machine sends its worker off with a load"
```

---

### Task 3: `MachineStatus::Unstaffed` and production gating

This is the task that makes the walk cost something. Without it, hauling is free uptime — the outcome the spec ranks last.

**Files:**
- Modify: `crates/engine/src/components.rs:375-384`
- Modify: `crates/engine/src/systems.rs:294-300` and the loop head at `:396-410`
- Create: `crates/engine/src/game/hauling.rs`
- Modify: `crates/engine/src/game/mod.rs`
- Modify: `crates/gui/src/render/base.rs:424-427`, `crates/gui/src/render/building.rs:473-475`
- Test: `crates/engine/src/tests/hauling.rs`

**Interfaces:**
- Consumes: `Carrying`, `HAUL_CARRY_CAPACITY` (Task 2).
- Produces:
  ```rust
  // crates/engine/src/game/hauling.rs
  /// True when `worker` stands on one of the four tiles `structure` can be
  /// reached from. Uses `collect::ORTHOGONAL` — the one reach rule the game
  /// has — rather than a second adjacency list.
  pub(crate) fn at_station(worker: Position, structure: Position) -> bool
  ```
  `MachineStatus::Unstaffed` variant.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/hauling.rs`:

```rust
#[test]
fn a_worker_off_its_tile_produces_nothing_and_says_so() {
    let mut game = Game::new(2);
    place_home(&mut game, 2, 0);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    game.assign_cronjob(worker, node).unwrap();

    // Strand it well outside the four tiles the node can be worked from.
    {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = 40;
        pos.y = 40;
    }

    let before = game.world.get::<Task>(worker).unwrap().progress;
    for _ in 0..10 {
        game.tick();
    }

    assert_eq!(
        game.world.get::<Task>(worker).unwrap().progress,
        before,
        "production must not advance while the worker is away from its post"
    );
    assert_eq!(
        *game.world.get::<MachineStatus>(node).unwrap(),
        MachineStatus::Unstaffed,
    );
}

#[test]
fn unstaffed_wins_over_running() {
    let mut game = Game::new(3);
    place_home(&mut game, 2, 0);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    game.assign_cronjob(worker, node).unwrap();
    {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = 40;
        pos.y = 40;
    }
    // An empty output buffer would otherwise read as Running.
    game.world.get_mut::<Stock>(node).unwrap().output.clear();

    game.tick();

    assert_eq!(
        *game.world.get::<MachineStatus>(node).unwrap(),
        MachineStatus::Unstaffed,
        "a machine with nothing wrong but nobody there is not Running"
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine hauling`
Expected: FAIL to compile — `no variant named 'Unstaffed'`.

- [ ] **Step 3: Write the minimal implementation**

`crates/engine/src/components.rs` — add to `MachineStatus`, after `Clogged`:

```rust
/// A program is posted but is not standing at the machine — walking to its
/// post, carrying a load to a depot, or unable to reach the machine at all.
/// Distinct from `Idle`, which means no program is assigned.
Unstaffed,
```

Create `crates/engine/src/game/hauling.rs` with a module doc and `at_station`, implemented over `crate::game::collect::ORTHOGONAL`. Declare `mod hauling;` in `crates/engine/src/game/mod.rs`.

`crates/engine/src/systems.rs`:
- `set_machine_status`'s `match` is exhaustive; add the `Unstaffed` arm. The line must read as base news, not field news, and must follow the vocabulary rule — player-facing text says nothing about "tasks" or "entities": `format!("The {name} has no one at it — its program is away.")`.
- `task_progress_system` needs the worker's `Position` and the node's `Position`. Add `&'static Position` to `CronjobWorker` and to `WorkedNode`. At the top of the loop body, after resolving the node: if `!at_station(worker_pos, node_pos)`, `set_machine_status(..., MachineStatus::Unstaffed, ...)` and `continue` — *before* `task.progress += 1`, so nothing advances.

`crates/gui/src/render/base.rs:424-427` — add `MachineStatus::Unstaffed => YELLOW` (the same class of "attention, not failure" as `Starved`).

`crates/gui/src/render/building.rs:473-475` — `Unstaffed` gets `Some("no one at it — its program is away")`. Note this match currently groups `Running | Idle => None`; leave that grouping alone and add `Unstaffed` as its own arm.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine hauling`
Expected: PASS.

Run: `cargo test -p feral-processes-engine chains building collect`
Expected: PASS. If a chains test fails because its hand-placed worker was never near its machine, that is this feature working — move the fixture's worker adjacent to its node rather than weakening the gate.

Run: `cargo check -p feral-processes-gui`
Expected: clean; both `MachineStatus` matches now cover the new variant.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -A
git commit -m "feat(hauling): a machine with its program away produces nothing"
```

---

### Task 4: The depot asset and nearest-depot lookup

**Files:**
- Create: `assets/structures/depot.ron`
- Modify: `crates/engine/src/game/hauling.rs`
- Test: `crates/engine/src/tests/hauling.rs`

**Interfaces:**
- Consumes: `at_station` (Task 3).
- Produces:
  ```rust
  /// The depot a worker at `from` should deliver to: fewest Chebyshev tiles
  /// away, ties broken by the depot's `(x, y)`. Deliberately not `walk_field`
  /// path cost — that is a second field per worker per tick for a difference
  /// only a wall between two near-equidistant depots can produce.
  pub(crate) fn nearest_depot(
      depots: &[(Entity, Position)],
      from: Position,
  ) -> Option<(Entity, Position)>

  /// The tile a worker must stand on to work or deliver to `structure`: the
  /// walkable orthogonal neighbour nearest `from`, ties by `(x, y)`.
  /// `None` when the structure is walled in.
  pub(crate) fn station_tile(
      map: &mut WorldMap,
      structure: Position,
      from: Position,
  ) -> Option<Position>
  ```

A depot is identified by having a `Stock` and *not* running a job — `StructureDef::runs_a_job()` is false and it has no `ResourceNode`. Do not identify it by id string; that would hardcode content.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/hauling.rs`:

```rust
/// Bevy's query iteration order is not stable, so the two depots are spawned
/// in the *opposite* order to their positions. Spawned in position order this
/// would pass on iteration order alone, which is the bug the tie-break and
/// the distance sort exist to prevent.
#[test]
fn a_worker_delivers_to_the_nearer_of_two_depots() {
    let mut game = Game::new(4);
    place_home(&mut game, 0, 3);

    let far = game.place_structure("depot", 6, 0).unwrap();
    let near = game.place_structure("depot", 2, 0).unwrap();

    let from = Position { x: 0, y: 0 };
    let depots = vec![
        (far, *game.world.get::<Position>(far).unwrap()),
        (near, *game.world.get::<Position>(near).unwrap()),
    ];

    let (chosen, _) = game::hauling::nearest_depot(&depots, from).unwrap();
    assert_eq!(chosen, near);
}

#[test]
fn a_depot_is_not_offered_as_a_cronjob() {
    let mut game = Game::new(5);
    place_home(&mut game, 0, 3);
    let depot = game.place_structure("depot", 2, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");

    assert!(
        game.assign_cronjob(worker, depot).is_err(),
        "a depot is delivered to, not worked — accepts_a_program must \
         already refuse it with no new code"
    );
}
```

`game::hauling` must be reachable from the test module. `mod game;` is private but `crate::game::hauling` is visible from inside the crate, which is where the tests live.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine hauling`
Expected: FAIL — `nearest_depot` not found, and `place_structure("depot", ...)` errors with an unknown structure.

- [ ] **Step 3: Write the minimal implementation**

Create `assets/structures/depot.ron`. Match the field style of `assets/structures/data_cache.ron`:

```ron
(
    id: "depot",
    name: "Depot",
    description: "Holds what your programs bring in, so a full machine has somewhere to empty into.",
    glyph: '&',
    color: Cyan,
    build_cost: [("core_fragment", 12)],
    work: None,
    capacity: 100,
)
```

Check `assets/structures/README.md` for the exact spelling of every field and the permitted `color` values before writing it — a malformed `.ron` is skipped with a logged warning rather than a panic, so a typo here produces a missing structure, not an error. Confirm no other shipped structure already uses `&` as its glyph.

Add `nearest_depot` and `station_tile` to `crates/engine/src/game/hauling.rs`. `station_tile` iterates `ORTHOGONAL`, keeps the offsets whose tile is `walkable`, and picks by `(chebyshev_distance_to_from, x, y)`.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine hauling`
Expected: PASS.

Run: `cargo test -p feral-processes-engine assets`
Expected: PASS — there is an asset-loading test module that will parse the new file.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -A
git commit -m "feat(hauling): the depot, and the rule for which one is nearest"
```

---

### Task 5: `haul_step_system` — walking, arriving, depositing

The heart of the feature. Walking to a post falls out of this for free: a worker with no `Carrying` has its machine as its destination, so a freshly posted program walks to work with no separate code path.

**Files:**
- Modify: `crates/engine/src/game/hauling.rs`
- Modify: `crates/engine/src/game/lifecycle.rs:143-158`
- Test: `crates/engine/src/tests/hauling.rs`

**Interfaces:**
- Consumes: `Carrying`, `at_station`, `nearest_depot`, `station_tile`, `walk_field`.
- Produces: `pub(crate) fn haul_step_system(...)`, registered in `Game::build_schedule`.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/hauling.rs`:

```rust
#[test]
fn a_carried_load_ends_up_in_the_depot_and_in_your_cargo() {
    let mut game = Game::new(6);
    place_home(&mut game, 0, 4);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    let depot = game.place_structure("depot", 4, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    game.assign_cronjob(worker, node).unwrap();

    let cap = game.world.get::<Stock>(node).unwrap().capacity;
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), cap);

    for _ in 0..200 {
        game.tick();
        let delivered = game
            .world
            .get::<Stock>(depot)
            .unwrap()
            .output
            .get(&ItemId::from(ids::CORE_FRAGMENT))
            .copied()
            .unwrap_or(0);
        if delivered > 0 {
            break;
        }
    }

    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) >= tuning::HAUL_CARRY_CAPACITY,
        "the worker should have walked a load to the depot"
    );
    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "the load is dropped on arrival, which is what flips the destination back"
    );

    // Consolidation: the player collects from the depot with no new code.
    let depot_pos = *game.world.get::<Position>(depot).unwrap();
    {
        let mut pos = game.world.get_mut::<Position>(game.player_entity()).unwrap();
        pos.x = depot_pos.x - 1;
        pos.y = depot_pos.y;
    }
    let taken = game.collect_adjacent();
    assert!(
        taken.iter().any(|(id, n)| *id == ItemId::from(ids::CORE_FRAGMENT) && *n > 0),
        "collect_adjacent must work on a depot unchanged"
    );
}

#[test]
fn a_posted_program_walks_to_its_machine_before_producing() {
    let mut game = Game::new(7);
    place_home(&mut game, 0, 4);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    {
        let mut pos = game.world.get_mut::<Position>(worker).unwrap();
        pos.x = 8;
        pos.y = 0;
    }
    game.assign_cronjob(worker, node).unwrap();

    let start = *game.world.get::<Position>(worker).unwrap();
    game.tick();
    let after = *game.world.get::<Position>(worker).unwrap();
    assert_ne!(
        (start.x, start.y),
        (after.x, after.y),
        "a program takes its post by walking to it"
    );

    let node_pos = *game.world.get::<Position>(node).unwrap();
    for _ in 0..40 {
        game.tick();
        let p = *game.world.get::<Position>(worker).unwrap();
        if game::hauling::at_station(p, node_pos) {
            break;
        }
    }
    let p = *game.world.get::<Position>(worker).unwrap();
    assert!(game::hauling::at_station(p, node_pos), "it should arrive");
    assert!(
        game.world.get::<Task>(worker).unwrap().progress > 0,
        "and start producing once it does"
    );
}

#[test]
fn clearing_a_full_buffer_takes_four_trips() {
    let mut game = Game::new(11);
    place_home(&mut game, 0, 4);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    let depot = game.place_structure("depot", 3, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    game.assign_cronjob(worker, node).unwrap();

    let cap = game.world.get::<Stock>(node).unwrap().capacity;
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), cap);

    for _ in 0..600 {
        game.tick();
        if node_output(&game, depot, ids::CORE_FRAGMENT) >= cap {
            break;
        }
    }

    assert!(
        node_output(&game, depot, ids::CORE_FRAGMENT) >= cap,
        "a {cap}-unit buffer moves {} units at a time, so it takes \
         {} round trips to shift — the cap is what makes the base's \
         motion continuous rather than one big haul",
        tuning::HAUL_CARRY_CAPACITY,
        cap / tuning::HAUL_CARRY_CAPACITY,
    );
}
```

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine hauling`
Expected: FAIL — the worker never moves; the depot stays empty and the loop times out.

- [ ] **Step 3: Write the minimal implementation**

Add `haul_step_system` to `crates/engine/src/game/hauling.rs`. Shape:

- Queries: workers as `(Entity, &mut Position, &Task, Option<&Carrying>)` with `With<Tamed>`; structures as `(Entity, &Position, &Stock, Option<&ResourceNode>, Option<&Structure>)`; `ResMut<WorldMap>`; and the `StructureDb` to ask `runs_a_job()`.
- Skip any worker whose `Task.kind` is not `GatherResource` — a `Guard` does not haul, mirroring `task_progress_system`'s own first check.
- Destination structure: nearest depot if `Carrying` is present, else `task.target`.
- Compute `station_tile(map, structure_pos, worker_pos)`. `None` → the structure is walled in; leave the worker where it is (Task 3's gate already reports `Unstaffed`).
- If the worker already stands on the station tile *and* is carrying, deposit and remove `Carrying`. Do this before the movement step, so an arrival is resolved in the same tick it completes rather than a tick later.
- Otherwise build `walk_field(map, station_tile, MAX_BUILD_DISTANCE_FROM_HOME, |t| t.walkable)` and step the worker to the neighbouring tile with the lowest cost, ties by `(x, y)`. A worker with no reachable neighbour in the field does not move.

Two ordering constraints to hold, both worth a comment:
- The deposit writes `Stock`, and so do `task_progress_system` and `assembler_system`. Register `haul_step_system` **inside** the existing `.chain()` in `Game::build_schedule` (`crates/engine/src/game/lifecycle.rs:148-156`), after `assembler_system`. Bevy can see the `Stock` conflict but not the disjointness, and an arbitrary-but-fixed order is not the same as a stated one — extend the existing comment there to name the third member.
- Depositing must respect the depot's `output_room()`. An over-capacity write would make `capacity` a suggestion. A full depot is Task 6.

Radius: use `MAX_BUILD_DISTANCE_FROM_HOME` from `tuning.rs` — bounding the search is what stops an unbounded walk generating chunks forever on a lazily-generated infinite map, which is the same reason `pursuit_field` bounds its successors.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine hauling`
Expected: PASS.

Run: `cargo test -p feral-processes-engine`
Expected: PASS. Watch for chains/building fixtures whose workers were never adjacent to their machines — fix the fixture, never the gate.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -A
git commit -m "feat(hauling): programs walk their loads to a depot and back"
```

---

### Task 6: Failure modes

Each is decided rather than left to emerge. The `Carrying`-drop pair is the "destroying a structure has two paths" trap from CLAUDE.md: `remove_structure` and `damage_structure` each clear worker `Task`s inline, and both must also drop `Carrying` or a worker keeps a load with nowhere to put it forever.

**Files:**
- Modify: `crates/engine/src/game/hauling.rs`
- Modify: `crates/engine/src/game/building.rs` (~line 310, inside `remove_structure`'s worker loop)
- Modify: `crates/engine/src/game/upkeep.rs` (~line 250, inside `damage_structure`'s worker loop)
- Test: `crates/engine/src/tests/hauling.rs`

**Interfaces:**
- Consumes: everything from Tasks 2-5.
- Produces: no new public names.

- [ ] **Step 1: Write the failing tests**

Append to `crates/engine/src/tests/hauling.rs`:

```rust
#[test]
fn a_full_depot_sends_the_load_back_and_re_clogs_the_machine() {
    let mut game = Game::new(8);
    place_home(&mut game, 0, 4);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    let depot = game.place_structure("depot", 3, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    game.assign_cronjob(worker, node).unwrap();

    // Depot brim-full, so there is nowhere for the load to land.
    let depot_cap = game.world.get::<Stock>(depot).unwrap().capacity;
    game.world
        .get_mut::<Stock>(depot)
        .unwrap()
        .output
        .insert(ItemId::from(ids::LOG_FRAGMENT), depot_cap);

    let node_cap = game.world.get::<Stock>(node).unwrap().capacity;
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), node_cap);

    for _ in 0..300 {
        game.tick();
    }

    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "the load must go back into the machine rather than ride forever"
    );
    assert_eq!(
        node_output(&game, node, ids::CORE_FRAGMENT),
        node_cap,
        "the base stalls loudly instead of the goods vanishing"
    );
}

#[test]
fn demolishing_a_machine_takes_its_workers_load_with_it() {
    let mut game = Game::new(9);
    place_home(&mut game, 0, 4);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    game.place_structure("depot", 6, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    game.assign_cronjob(worker, node).unwrap();

    let cap = game.world.get::<Stock>(node).unwrap().capacity;
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), cap);

    for _ in 0..200 {
        game.tick();
        if game.world.get::<Carrying>(worker).is_some() {
            break;
        }
    }
    assert!(game.world.get::<Carrying>(worker).is_some(), "precondition");

    game.remove_structure(node).unwrap();

    assert!(game.world.get::<Task>(worker).is_none());
    assert!(
        game.world.get::<Carrying>(worker).is_none(),
        "a worker whose task is gone must not keep a load with nowhere to put it"
    );
}
```

```rust
#[test]
fn a_depot_demolished_mid_walk_re_targets_the_next_one() {
    let mut game = Game::new(12);
    place_home(&mut game, 0, 6);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    let near = game.place_structure("depot", 3, 0).unwrap();
    let far = game.place_structure("depot", 8, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    game.assign_cronjob(worker, node).unwrap();

    let cap = game.world.get::<Stock>(node).unwrap().capacity;
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), cap);

    for _ in 0..200 {
        game.tick();
        if game.world.get::<Carrying>(worker).is_some() {
            break;
        }
    }
    assert!(game.world.get::<Carrying>(worker).is_some(), "precondition");

    game.remove_structure(near).unwrap();

    for _ in 0..400 {
        game.tick();
        if node_output(&game, far, ids::CORE_FRAGMENT) > 0 {
            break;
        }
    }
    assert!(
        node_output(&game, far, ids::CORE_FRAGMENT) > 0,
        "a worker whose depot vanished mid-walk delivers to the next nearest"
    );
}
```

Check `ids::LOG_FRAGMENT` exists in `crates/engine/src/items.rs`'s `ids` module before using it; substitute any item id a Mining Node does not produce, since the point is only that the depot is full of *something else*.

Verify the exact signatures of `spawn_tamed`, `place_home`, `node_output` and `test_assets_dir` in `crates/engine/src/tests/support.rs` before writing these — they are the repo's fixtures and the plan assumes `spawn_tamed(&mut Game, &str) -> Entity`. Look there before writing any new fixture of your own.

- [ ] **Step 2: Run tests to verify they fail**

Run: `cargo test -p feral-processes-engine hauling`
Expected: FAIL — the worker keeps its load indefinitely in both cases.

- [ ] **Step 3: Write the minimal implementation**

In `haul_step_system`'s deposit step: if `output_room()` is smaller than the carried quantity, deliver what fits and keep the rest. If nothing fits at all and the worker is at the depot, re-target — try the next-nearest depot; when every depot is full or none remains, set the destination back to the machine and, on arrival there, pour `Carrying` back into the machine's `output` and remove the component. Re-clogging is the intended outcome and needs no special handling: `task_progress_system` will report it on the next cycle.

In `remove_structure` (`crates/engine/src/game/building.rs`) and `damage_structure` (`crates/engine/src/game/upkeep.rs`): both already collect a `workers: Vec<Entity>` and call `self.world.entity_mut(w).remove::<Task>()`. Change each to `.remove::<(Task, Carrying)>()`. `EntityWorldMut::remove` on a tuple is a no-op for components that aren't present, so a worker that wasn't carrying is unaffected.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine hauling`
Expected: PASS.

Run: `cargo test -p feral-processes-engine raids building`
Expected: PASS — both destruction paths are exercised there.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -A
git commit -m "feat(hauling): a load always has somewhere to go, or comes back"
```

---

### Task 7: Save format 23 → 24

**Files:**
- Modify: `crates/engine/src/save.rs:56-90`, `:310-316`
- Modify: `crates/engine/src/game/lifecycle.rs:591-660` (write), `:415-440` (restore)
- Modify: `dev-saves/chains.ron`, `dev-saves/extraction.ron`, `dev-saves/stack.ron`
- Modify: `Cargo.toml` (workspace version)
- Test: `crates/engine/src/tests/hauling.rs`

**Interfaces:**
- Consumes: `Carrying`.
- Produces: `CreatureSave.carrying: Option<(ItemId, u32)>`, `SAVE_FORMAT_VERSION = 24`.

- [ ] **Step 1: Write the failing test**

Append to `crates/engine/src/tests/hauling.rs`:

```rust
#[test]
fn a_carried_load_survives_a_save_and_load() {
    let mut game = Game::new(10);
    place_home(&mut game, 0, 4);
    let node = game.place_structure("mining_node", 1, 0).unwrap();
    game.place_structure("depot", 6, 0).unwrap();
    let worker = spawn_tamed(&mut game, "drone");
    game.assign_cronjob(worker, node).unwrap();

    let cap = game.world.get::<Stock>(node).unwrap().capacity;
    game.world
        .get_mut::<Stock>(node)
        .unwrap()
        .output
        .insert(ItemId::from(ids::CORE_FRAGMENT), cap);

    for _ in 0..200 {
        game.tick();
        if game.world.get::<Carrying>(worker).is_some() {
            break;
        }
    }
    let before = game.world.get::<Carrying>(worker).cloned().expect("precondition");

    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("save.bin");
    game.save(&path).unwrap();
    let loaded = Game::load(&path, test_assets_dir()).unwrap();

    let mut q = loaded.world.query::<(&Carrying, &Task)>();
    let (carrying, _) = q
        .iter(&loaded.world)
        .next()
        .expect("the load must come back with the worker");
    assert_eq!(carrying.item, before.item);
    assert_eq!(carrying.qty, before.qty);
}
```

Check how existing save round-trip tests build their temp path — look at `crates/engine/src/tests/` for the existing `game.save(...)` / `Game::load(...)` pattern and copy it exactly rather than introducing `tempfile` if the repo does it another way.

- [ ] **Step 2: Run test to verify it fails**

Run: `cargo test -p feral-processes-engine a_carried_load_survives`
Expected: FAIL — the loaded world has no `Carrying`.

- [ ] **Step 3: Write the minimal implementation**

`crates/engine/src/save.rs`:
- Add `pub carrying: Option<(ItemId, u32)>` to `CreatureSave`, at the **end** of the struct. bincode encodes positionally; appending is what makes the version bump the only breakage.
- Doc comment on the field: only meaningful when `tamed` is true, and the target depot is *not* stored — the destination is re-derived on load from position, which is the whole point of deriving state rather than storing it.
- Bump `SAVE_FORMAT_VERSION` to `24` and add the changelog line above it, matching the existing style: `/// 23 → 24: 'CreatureSave' gained 'carrying', for a program mid-delivery to a depot.`

`crates/engine/src/game/lifecycle.rs`:
- Save (line ~591): add `Option<&Carrying>` to the `creature_query` tuple, destructure it in the `for` binding, and write `carrying: carrying.map(|c| (c.item.clone(), c.qty))` in the `CreatureSave { ... }` literal at line ~651.
- Restore (line ~415): inside the `if c.tamed` branch, alongside the existing `cronjob` handling, insert `Carrying` when `c.carrying` is `Some`. It does not need the deferred `pending_cronjobs` treatment — unlike a cronjob target, a load names no entity.

`dev-saves/*.ron`: these are RON, so add `carrying: None,` to every creature entry. Verify by loading each afterwards:
`cargo run -- --template extraction`, `--template chains`, `--template stack`.

`Cargo.toml`: bump `version` from `0.2.0` to `0.3.0` — a save-format bump is a breaking change under this project's stated versioning.

- [ ] **Step 4: Run tests to verify they pass**

Run: `cargo test -p feral-processes-engine hauling`
Expected: PASS.

Run: `cargo test --workspace`
Expected: PASS.

- [ ] **Step 5: Commit**

```bash
cargo fmt && cargo clippy --workspace
git add -A
git commit -m "feat(hauling): persist a program mid-delivery, save format 23 -> 24"
```

---

### Task 8: Documentation

The repo's standing doc obligation, minus two carve-outs.

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`
- Check: `assets/structures/README.md`

**Do NOT modify** `docs/manual.md` (carved out 2026-08-04) or the root `README.md` (carved out 2026-08-05). Both carve-outs are standing until the user lifts them.

- [ ] **Step 1: `CHANGELOG.md`**

Under `## Unreleased`, add these two sections. Match the voice of the existing entries — they describe what a *player* experiences, not what the code does.

```markdown
### Breaking: save format 23 → 24

`CreatureSave` gained a field, so existing `.bin` saves stop loading.
Templates under `dev-saves/` are RON and were updated in place.

### Added: your base has people in it now

A program posted to a machine used to be a name on a screen. It now stands
at its machine, and when that machine's output buffer fills it carries a
load to the nearest Depot and walks back. You can watch it happen.

Two consequences. A machine no longer stops dead when it fills — it sheds
five units at a time and keeps going, so how long your base runs unattended
is now a question about where you put the Depot rather than a fixed number.
And a program takes a moment to reach its post when you assign it: it walks
there, and produces nothing until it arrives.

The Depot itself holds a hundred units and costs twelve Core Fragments. You
collect from it exactly the way you collect from anything else — stand next
to it. Build a second one across the base and half your programs' walks get
shorter.
```

- [ ] **Step 2: `assets/structures/README.md`**

No schema fields were added or changed, so no update is required. Confirm this by re-reading the field list — if `capacity` is documented only in the context of machines, add a sentence that a structure with no `work` or `assembles` still uses it, since the depot is now the case that makes that matter.

- [ ] **Step 3: `CLAUDE.md` load-bearing seams**

Add entries for the two non-obvious facts a future session would otherwise pay tool calls to rediscover:
- `walk_field` is one walk with a per-caller step rule, and `pursuit_field`'s `Platform` exclusion is why. A third caller widens the predicate rather than copying the Dijkstra.
- `Carrying` is the only stored hauling state; destination and "at post" are derived from `Position`, and the `HAUL_CARRY_CAPACITY` cap is what lets it be a single `(item, qty)` pair rather than a map. Both destruction paths must drop it.

Then `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking to catch drift.

- [ ] **Step 4: Commit**

```bash
git add -A
git commit -m "docs: record the depot, the shared walk, and the carry cap"
```

---

### Task 9: Final gate

- [ ] **Step 1: Full suite**

Run: `cargo test --workspace`
Expected: PASS. Baseline before this work was 1455 tests; the count should be higher, not lower.

- [ ] **Step 2: Lints**

Run: `cargo clippy --workspace` and `cargo fmt --check`
Expected: clean. Fix warnings rather than silencing them.

- [ ] **Step 3: Balance check**

Run: `cargo test -p feral-processes-engine balance_sim`
Expected: PASS unchanged. `balance_sim` models battle, not work, so this feature should not move a single curve. **A moved curve here means something unintended touched combat** — investigate rather than re-baselining.

- [ ] **Step 4: Play it**

A green suite is not evidence that a base *feels* alive. Run it and watch:

```sh
cargo run -- --template chains
```

Build a depot, post a program, and confirm three things by eye: the program walks to its post rather than teleporting; the walk-to-post delay reads as intent rather than as a bug; and a delivery round trip is legible on screen rather than a glyph twitching. Report what you actually saw.

- [ ] **Step 5: Report**

Summarise for the user: what landed, what the play session showed, and which numbers (`HAUL_CARRY_CAPACITY` 5, depot `capacity` 100, `build_cost` 12 Core Fragments) are unplayed judgement calls that may want retuning.

---

## Notes for the implementer

- **`Position` is pinned to the surface entrance tile while the party is underground.** This feature never reads the player's `Position`, so it is unaffected — but do not add a "haul only when the player is nearby" rule, which would silently mean "nearby the entrance tile" in the Stack.
- **The base ticks while the party is in the Stack**, so workers keep hauling down there. That is the intended bounded-uptime payoff, not a bug.
- **If many tests fail at once with `NotFound` on an asset path**, it is stale build artifacts from an old directory rename, not real failures. Fix with `cargo clean -p feral-processes-engine -p feral-processes-app-core` — not a full `cargo clean`, which throws away ~4 GB.
- **Warm builds are ~1s for `cargo check` and ~3s for the engine suite.** There is no tooling problem to solve here; iterate with targeted test names.
