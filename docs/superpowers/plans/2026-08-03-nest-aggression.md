# Nest Aggression Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Attacking a nest provokes its guardians into pursuing the player across the zone map, and destroying one pays a loot cache.

**Architecture:** A `Pursuing` marker component on nest guardians, set by `Game::attack_nest`. A new `Game::nest_aggro_tick`, called from `tick_inner`, builds one bounded Dijkstra cost field from the player's tile per tick and steps every pursuer downhill; a pursuer that ends adjacent to the player starts an ordinary battle through `gather_pack`/`start_battle`. Nests, the guardian tether and live aggro become part of the save.

**Tech Stack:** Rust 2024, `bevy_ecs` 0.19 (standalone, engine crate only), `pathfinding` 4 (new), `bincode` 2 for the save.

**Spec:** `docs/superpowers/specs/2026-08-03-nest-aggression-design.md`. Read it first — it records *why* each of these decisions was made, and this plan does not repeat the reasoning.

## Global Constraints

- **All work is in `crates/engine`.** No other crate is touched. The renderer never sees any of this; nests already draw from `Glyph`.
- **Every new tuning value goes in `crates/engine/src/tuning.rs`**, in the existing nests section (near `NEST_RESPAWN_TICKS`, ~line 745), as a documented `pub const`. Never inline a number in a formula.
- **New test fixtures go in `crates/engine/src/tests/support.rs`.** Read that file before writing one — `spawn_tamed`, `spawn_wild_on_player_tile`, `insert_battle`, `flee_until_clear`, `set_level`, `test_assets_dir`, `resolve_round_with` already exist.
- **No flaky tests.** No `sleep`, no wall-clock, no unseeded RNG. Background systems (habitat spawning, nest respawns) run on every `tick` and will interfere with naive assertions — construct the state you are asserting on explicitly.
- **Gates after every task:** `cargo test --workspace`, `cargo clippy --workspace` (fix warnings, don't silence), `cargo fmt`.
- **Comment discipline:** comments explain *why* — a constraint, an invariant, a trap. Never *what*.
- Iterate with `cargo test -p feral-processes-engine <name>`; the engine suite is ~3s. Only run the full workspace suite at a task boundary.

## File Structure

| File | Responsibility in this change |
|---|---|
| `crates/engine/Cargo.toml` | Add `pathfinding = "4"`. |
| `crates/engine/src/components.rs` | New `Pursuing` marker, beside `NestGuardian` (~line 743). |
| `crates/engine/src/lib.rs` | Re-export `Pursuing` in the components `use` list (~line 44). |
| `crates/engine/src/tuning.rs` | Six new constants in the nests section. |
| `crates/engine/src/systems.rs` | Tether fix in `wander_ai_system`; exclude pursuers from wandering. |
| `crates/engine/src/game/pursuit.rs` | **New.** The bounded cost field — a free function, no `Game`. |
| `crates/engine/src/game/mod.rs` | Declare `mod pursuit;`. |
| `crates/engine/src/game/zone.rs` | `attack_nest` provokes and pays the cache; `despawn_nest` clears `Pursuing`. |
| `crates/engine/src/game/upkeep.rs` | `nest_respawn_tick` spawns a guardian aggroed at a besieged nest. |
| `crates/engine/src/game/turn.rs` | `nest_aggro_tick` — the pursuit step — and its call from `tick_inner`. |
| `crates/engine/src/game/combat_rewards.rs` | Add `Pursuing` to the tuple the tame path already strips. |
| `crates/engine/src/save.rs` | `NestSave`, two `CreatureSave` fields, `SAVE_FORMAT_VERSION` 18 → 19. |
| `crates/engine/src/game/lifecycle.rs` | Write and read nests; resolve the tether by tile. |
| `crates/engine/src/tests/spawning.rs` | Tether and respawn tests. |
| `crates/engine/src/tests/zone.rs` | Provocation, pursuit, contact, leash and cache tests. |
| `crates/engine/src/tests/support.rs` | New fixtures. |
| `docs/manual.md`, `CHANGELOG.md`, `README.md`, `CLAUDE.md` + `AGENTS.md` | Task 7. |

`pursuit.rs` is a new file rather than more lines in `turn.rs` because the cost field is a pure function of a map, an origin and a box — it takes no `Game`, touches no ECS, and is the one piece here that is worth testing without a world at all.

---

### Task 1: The tether no longer freezes a displaced guardian

This is a **pre-existing latent bug** that pursuit would make reachable. `wander_ai_system` refuses any step whose new Chebyshev distance from the nest exceeds `NEST_TETHER_RADIUS` (5). Nothing today can put a guardian outside that radius, so the check is total. Once a guardian can be dragged 15 tiles away by a chase and then leash off, *every* neighbouring tile is still beyond 5 — it has no legal move and stands frozen for the rest of the run.

Fix it first, on its own, with its own reproducer. It must land before or with the pursuit step, never after.

**Files:**
- Modify: `crates/engine/src/systems.rs:79-86` (the tether check inside `wander_ai_system`)
- Test: `crates/engine/src/tests/spawning.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: nothing new. Behaviour change only.

- [ ] **Step 1: Write the failing test**

In `crates/engine/src/tests/spawning.rs`, add `a_guardian_outside_its_tether_walks_back_toward_its_nest`.

Intent: spawn a nest, take one of its guardians, and place it far outside the tether by writing its `Position` directly (say nest + (12, 0)). Tick enough times that `WanderAi`'s 2–6 tick cooldown must have fired several times, then assert its Chebyshev distance from the nest is *strictly less* than where it started.

Two things this test must do to not be flaky:
- Seed the game and place the guardian on ground you know is walkable — check the tiles you are moving it across with `WorldMap::tile`, or place the nest somewhere the map is open. A guardian in a `DataVoid` pocket legitimately cannot move.
- Assert "closer than it started", not "back inside the tether". The walk is a random ±1 step on a cooldown, so the number of ticks needed to fully return is not deterministic. Closing *any* distance is the invariant; arriving is not.

- [ ] **Step 2: Run it and watch it fail**

`cargo test -p feral-processes-engine a_guardian_outside_its_tether -- --nocapture`

Expected: FAIL. The guardian's distance is unchanged — it never moved at all, because every candidate step was refused.

- [ ] **Step 3: Fix the check**

In `wander_ai_system`, the current condition refuses on `dist > NEST_TETHER_RADIUS` alone. Compare the *new* distance against the *current* one and refuse only a step that both leaves the tether and fails to close on the nest:

```rust
let dist = (nx - nest_pos.x).abs().max((ny - nest_pos.y).abs());
let current = (pos.x - nest_pos.x).abs().max((pos.y - nest_pos.y).abs());
// Refuse only a step that both leaves the tether *and* doesn't close on
// the nest. A guardian dragged outside its radius — by a chase, or by a
// test placing it there — would otherwise have no legal move at all and
// stand frozen for the rest of the run.
if dist > NEST_TETHER_RADIUS && dist >= current {
    continue;
}
```

Note `dist >= current`, not `>`: a lateral step that holds distance constant is still refused, so a displaced guardian makes monotonic progress home rather than orbiting.

- [ ] **Step 4: Run the test and the existing tether tests**

`cargo test -p feral-processes-engine tether`
Expected: PASS, including whatever already asserts a guardian stays within its tether — the fix must not let an *inside* guardian wander out.

- [ ] **Step 5: Full gates and commit**

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt
git add -A && git commit -m "fix(nests): a guardian dragged outside its tether can walk home"
```

---

### Task 2: The `Pursuing` marker, and what sets and clears it

Provocation only. Nothing moves yet — that is Task 4. This task is complete and reviewable on its own: after it, hitting a nest marks its guardians, and every path that removes a guardian from the world removes the marker with it.

**Files:**
- Modify: `crates/engine/src/components.rs` (beside `NestGuardian`, ~line 743)
- Modify: `crates/engine/src/lib.rs:44` (the components re-export list)
- Modify: `crates/engine/src/game/zone.rs` — `attack_nest` (~line 40), `despawn_nest` (~line 63)
- Modify: `crates/engine/src/game/upkeep.rs` — `nest_respawn_tick` (~line 70)
- Modify: `crates/engine/src/game/combat_rewards.rs:293-295` (the tame path)
- Modify: `crates/engine/src/systems.rs` — `wander_ai_system` query filter
- Test: `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Produces, for Tasks 4 and 6:
  - `pub struct Pursuing;` — a unit struct deriving `Component`, exported from `crate::components` and re-exported through `crate::*`.
  - `Game::provoke_nest(&mut self, nest: Entity)` — `pub(crate)`, sets `Pursuing` on every living guardian of `nest`.
  - `Game::nest_has_pursuers(&mut self, nest: Entity) -> bool` — `pub(crate)`, used by `nest_respawn_tick`.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/zone.rs`:

1. `attacking_a_nest_provokes_only_its_own_guardians` — spawn two nests far enough apart that their tethers cannot overlap, attack one (move the player onto its tile, or call `attack_nest` directly), assert every guardian of that nest has `Pursuing` and no guardian of the other does.
2. `destroying_a_nest_clears_pursuing` — provoke, then drive the nest's `Durability` to 0, then assert no surviving guardian has `Pursuing` *or* `NestGuardian`.
3. `a_guardian_respawned_at_a_besieged_nest_is_already_pursuing` — provoke a nest, push a `pending_respawns` entry, tick past `NEST_RESPAWN_TICKS`, assert the new guardian has `Pursuing`. Pair it with `a_guardian_respawned_at_a_calm_nest_is_not_pursuing`.
4. `a_pursuing_guardian_does_not_also_wander` — provoke, record positions, tick, assert positions are unchanged (nothing moves them yet in this task).
5. `decompiling_a_pursuing_guardian_strips_the_marker` — the tame path. Use the existing battle fixtures in `support.rs`; force the capture roll rather than hoping for it.

The two-nests fixture is worth adding to `support.rs` as e.g. `spawn_nest_at(&mut Game, &str, i32, i32) -> Entity` returning the nest entity, since Tasks 4, 5 and 6 all need it. `Game::spawn_nest` is `pub(crate)` and returns `()` — either widen it to return the `Entity` (preferred; nothing depends on the unit return) or have the fixture look the nest up with a `With<Nest>` query afterwards.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine nest`
Expected: FAIL to compile — `Pursuing` does not exist.

- [ ] **Step 3: Add the component**

In `components.rs`, beside `NestGuardian`. Doc comment records the two things a reader needs: that there is deliberately no target field (the player is the only thing anything in this game pursues), and that the chase ends spatially or with the nest, never on a timer — so a second pursuable target or a timed chase is the signal to revisit, not to bolt a field on.

Add `Pursuing` to the `use crate::components::{...}` list in `lib.rs:44` so `crate::*` carries it, matching how `NestGuardian` is exported.

- [ ] **Step 4: Wire the four setters and clearers**

- `attack_nest`: call `self.provoke_nest(nest)` at the top, on **every** hit, not just the first. A guardian that leashed off is re-provoked by the next swing. It must run before `attack_nest`'s caller ticks, which it does — `move_player` calls `attack_nest` then `tick`.
- `provoke_nest`: collect the guardian entities in an inner scope (the borrow of `self.world` from the query must end before the `entity_mut` loop — the same shape `despawn_nest` already uses two functions down), then insert `Pursuing` on each.
- `despawn_nest`: it already strips `NestGuardian` in a loop; make it `remove::<(NestGuardian, Pursuing)>()`. One place, covering both its callers.
- `nest_respawn_tick`: after `spawn_nest_guardian` returns, if `self.nest_has_pursuers(nest)` insert `Pursuing` on the new guardian. Note `spawn_nest_guardian` currently returns `()`; have it return `Option<Entity>` (it already has the entity in hand from `spawn_wild_creature`) rather than re-querying for the creature it just made.
- `combat_rewards.rs:293-295`: the tame path already does `remove::<(Hostile, WanderAi, NestGuardian)>()`. Add `Pursuing` to that tuple.

- [ ] **Step 5: Exclude pursuers from wandering**

`wander_ai_system`'s query filter is `Without<Player>`. Make it `(Without<Player>, Without<Pursuing>)`. A pursuer is driven by `nest_aggro_tick` in Task 4; letting both move it in one tick would double its speed and make `NEST_PURSUIT_STEPS_PER_TICK` a lie.

- [ ] **Step 6: Run the tests**

`cargo test -p feral-processes-engine nest`
Expected: PASS.

- [ ] **Step 7: Full gates and commit**

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt
git add -A && git commit -m "feat(nests): attacking a nest provokes its guardians"
```

---

### Task 3: The bounded pursuit cost field

A pure function, no `Game`, no ECS. This is the task that justifies the dependency, so its test has to be the one a greedy step-toward-the-player fails.

**Files:**
- Modify: `crates/engine/Cargo.toml`
- Create: `crates/engine/src/game/pursuit.rs`
- Modify: `crates/engine/src/game/mod.rs` (declare `mod pursuit;`)
- Modify: `crates/engine/src/tuning.rs`
- Test: inline `#[cfg(test)] mod tests` in `pursuit.rs` — it needs no `Game`, so it does not belong in `src/tests/`

**Interfaces:**
- Produces, for Task 4:
  ```rust
  pub(crate) fn pursuit_field(
      map: &mut WorldMap,
      origin: (i32, i32),
      radius: i32,
  ) -> std::collections::HashMap<(i32, i32), u32>
  ```
  Costs are Chebyshev step counts from `origin`. `origin` itself is present with cost 0. A tile absent from the map is unreachable, off-limits, or outside the box — callers must not distinguish.

- [ ] **Step 1: Add the dependency and the constants**

`crates/engine/Cargo.toml`, in dependency order (alphabetical — after `noise`, before `rand`):

```toml
pathfinding = "4"
```

Run `cargo build -p feral-processes-engine` once to fetch it and confirm it resolves before writing anything against it. This is the one cold-ish build in the plan; everything after is warm.

In `tuning.rs`, in the nests section after `NEST_DURABILITY`:

| Constant | Value | Doc must record |
|---|---|---|
| `NEST_PURSUIT_STEPS_PER_TICK: u32` | `1` | Tiles a pursuer covers per tick. `1` is player speed: you outrun a swarm in a straight line but never shake it, and it catches you the moment you stop to work, rest, or swing at the nest again. Above `1` they will reach you. |
| `NEST_AGGRO_LEASH_RADIUS: i32` | `15` | Chebyshev distance **from the nest** past which a pursuer gives up. Measured from the nest, not from where the chase began — so a nest near the base can put pursuers on the doorstep. |
| `NEST_PATH_SEARCH_MARGIN: i32` | `5` | Added to the leash radius to size the search box. |

The three cache constants land in Task 5; do not add them here.

- [ ] **Step 2: Write the failing test**

In `pursuit.rs`'s test module, `a_field_routes_around_a_concave_obstacle`.

Intent, and the reason this is the test that matters: build a bare `WorldMap`, then use `set_override` to carve a **cup** — three walls of unwalkable tile opening away from the origin — and stand a would-be pursuer inside the mouth of the cup with the origin behind its closed back. Assert that the neighbour of the pursuer with the lowest field cost is a step *sideways, out of the cup*, not the step that most reduces raw Chebyshev distance to the origin. A greedy chase walks into the back wall and stops; the field routes around.

Write a second test, `a_field_is_bounded_by_its_radius`: assert no key in the returned map lies outside the box, and that a walkable tile just outside it is absent. This is the test that stops an unbounded search generating chunks outward forever on an infinite map — it is not a nicety.

Third, `an_enclosed_origin_yields_a_field_of_just_itself`: ring the origin in unwalkable tile, assert the map has exactly one entry. Callers rely on absence meaning "no route".

- [ ] **Step 3: Run and watch it fail**

`cargo test -p feral-processes-engine pursuit`
Expected: FAIL to compile — `pursuit_field` does not exist.

- [ ] **Step 4: Implement**

Use `pathfinding::directed::dijkstra::dijkstra_all`, which returns `HashMap<N, (N, C)>` of node → (parent, cost). Three things about the shape are genuinely non-obvious and worth spelling out:

```rust
// `WorldMap::tile` takes `&mut self` — it generates chunks lazily — so the
// successor closure has to hold the map mutably. `dijkstra_all` takes
// `FnMut`, which permits exactly this; nothing else in the call borrows it.
let reached = dijkstra_all(&origin, |&(x, y)| {
    NEIGHBOURS
        .iter()
        .map(move |(dx, dy)| (x + dx, y + dy))
        .filter(|&(nx, ny)| {
            // Bounding the *successors* is what bounds the search. An
            // unbounded failed search on an infinite, lazily-generated map
            // generates chunks outward until it dies.
            (nx - origin.0).abs() <= radius
                && (ny - origin.1).abs() <= radius
                && {
                    let tile = map.tile(nx, ny);
                    // The base slab stays the one safe ground, as
                    // `maybe_ambush` and `stamp_platform` already establish.
                    // A leash measured from the nest cannot guarantee this:
                    // a nest can stand within leash range of the base.
                    tile.walkable && tile.biome != Biome::Platform
                }
        })
        .map(|n| (n, 1u32))
        .collect::<Vec<_>>()
});
```

Then flatten to node → cost and insert `origin => 0`, which `dijkstra_all` does not include.

`NEIGHBOURS` is the eight offsets, matching the game's 8-directional movement. `spawning.rs` and `turn.rs` each already spell this array out inline; a third copy here is fine under this repo's "three similar lines beat a speculative abstraction" rule — do not extract it.

Unit cost per step is correct: all eight directions cost the same because movement is Chebyshev. Do not weight diagonals.

- [ ] **Step 5: Run the tests**

`cargo test -p feral-processes-engine pursuit`
Expected: PASS, all three.

- [ ] **Step 6: Full gates and commit**

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt
git add -A && git commit -m "feat(nests): a bounded pursuit cost field, routed with pathfinding"
```

---

### Task 4: The pursuit step

Wires Tasks 2 and 3 together. After this the feature is playable.

**Files:**
- Modify: `crates/engine/src/game/turn.rs` — new `nest_aggro_tick`, called from `tick_inner` (~line 97)
- Test: `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Consumes: `Pursuing`, `Game::provoke_nest` (Task 2); `pursuit::pursuit_field` (Task 3); existing `Game::gather_pack(anchor: Entity) -> Vec<Entity>` and `Game::start_battle(pack: Vec<Entity>)` from `game/combat.rs`.
- Produces: `Game::nest_aggro_tick(&mut self)`, `pub(crate)`, called from `tick_inner`.

- [ ] **Step 1: Write the failing tests**

In `crates/engine/src/tests/zone.rs`:

1. `a_pursuer_closes_on_the_player_each_tick` — provoke, place a guardian a known open distance away, tick once, assert its Chebyshev distance to the player dropped by exactly `NEST_PURSUIT_STEPS_PER_TICK`.
2. `a_pursuer_that_reaches_the_player_starts_a_battle` — place a pursuer two tiles away, tick, assert `has_active_battle()`.
3. `the_battle_a_pursuer_starts_includes_its_packmates` — two pursuers converging; assert the battle's total member count across groups is greater than one. This is the "swarm" part of the feature and the thing a reviewer would otherwise have to take on faith.
4. `a_pursuer_beyond_the_leash_gives_up` — place a pursuer at `NEST_AGGRO_LEASH_RADIUS + 1` from its nest, tick, assert `Pursuing` is gone and it did not move toward the player.
5. `pursuers_never_step_onto_the_base_platform` — stamp a platform between a pursuer and the player, tick several times, assert no pursuer ever holds a `Position` whose tile is `Biome::Platform`.
6. `nest_aggro_tick_is_a_no_op_during_a_battle` — insert a battle with the existing `insert_battle` fixture, record pursuer positions, tick, assert unchanged.

Test 1 needs the ground between the two points to be walkable. Do not hope — either assert walkability as a precondition in the test, or `set_override` a walkable lane. A test that silently passes because the pursuer was blocked is worse than one that fails.

- [ ] **Step 2: Run and watch them fail**

`cargo test -p feral-processes-engine pursu`
Expected: FAIL — distances unchanged, no battle.

- [ ] **Step 3: Implement `nest_aggro_tick`**

Order inside the function, and every line of it is load-bearing:

1. Return if `is_game_over().is_some()` or `has_active_battle()`.
2. **Leash first.** For each `Pursuing` guardian, if its Chebyshev distance from its own nest's `Position` exceeds `NEST_AGGRO_LEASH_RADIUS`, remove `Pursuing`. Doing this before the field is built means a fully-leashed swarm costs one query and no search. A guardian whose nest entity no longer resolves also loses the marker — belt and braces; `despawn_nest` should have caught it.
3. Collect the surviving pursuers. Return if empty.
4. Build the field: `pursuit_field(map, player_tile, NEST_AGGRO_LEASH_RADIUS + NEST_PATH_SEARCH_MARGIN)`.

   **Centred on the player, not on the nest** — the spec says "around the nest", and one box around the player is the same bound with a simpler centre and holds however many nests are provoked at once. A pursuer farther from the player than that radius is simply absent from the field and skips its step, which is the documented outcome.
5. For each pursuer, in order:

```
if chebyshev(pursuer, player) <= 1 { engage(pursuer); return }
for _ in 0..NEST_PURSUIT_STEPS_PER_TICK {
    step to the neighbour with the strictly lowest field cost, if one exists
    if chebyshev(pursuer, player) <= 1 { break }
}
if chebyshev(pursuer, player) <= 1 { engage(pursuer); return }
```

The adjacency check **before** the step is what stops a pursuer walking onto the player's own tile: the player's tile has cost 0 and would always win the downhill comparison. Checking first means a step only ever happens at distance ≥ 2, where the best neighbour is at distance ≥ 1 and the player's tile is never a candidate.

`engage(pursuer)` is `let pack = self.gather_pack(pursuer); self.start_battle(pack);` then return from the whole function — nothing else moves once a fight has started.

A pursuer absent from the field, or with no neighbour of strictly lower cost, does not move. Both are legitimate: enclosed, out of range, or already as close as the terrain allows.

Borrow shape: collect `(Entity, Position)` pairs into a `Vec` in an inner scope before the movement loop, the way `despawn_nest` and `stamp_platform` already do. The field needs `&mut WorldMap` and the writes need `get_mut::<Position>` — do not try to hold both.

- [ ] **Step 4: Call it from `tick_inner`**

In `turn.rs:89-107`, immediately after `self.nest_respawn_tick();`. Order matters: a guardian respawned this tick at a besieged nest is already `Pursuing` (Task 2) and should get its step in the same tick it appeared.

- [ ] **Step 5: Run the tests**

`cargo test -p feral-processes-engine pursu && cargo test -p feral-processes-engine nest`
Expected: PASS.

Then run the whole engine suite and read the failures carefully. `nest_aggro_tick` runs on **every** tick in the game, so an unrelated test that spawns a nest and ticks may now find itself in a fight. A test that breaks this way is telling you the feature works; fix the test's setup, not the feature.

- [ ] **Step 6: Full gates and commit**

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt
git add -A && git commit -m "feat(nests): provoked guardians path to the player and engage"
```

---

### Task 5: The nest cache

**Files:**
- Modify: `crates/engine/src/game/zone.rs` — `attack_nest`
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Consumes: `Game::grant_loot(item: ItemId, qty: u32) -> u32` (`game/turn.rs:523` — returns how much actually landed, which can be less than asked for when the inventory is full); `Game::equipment_drops_for(&SpeciesDef) -> Vec<(ItemId, f32)>` (`game/combat_rewards.rs:24`); `Game::craft_currency() -> ItemId` (`game/catalog.rs:164`); `Game::item_name(&ItemId) -> &str` (`game/catalog.rs:41`).
- Produces: `Game::grant_nest_cache(&mut self, nest: Entity)`, `pub(crate)`.

- [ ] **Step 1: Add the constants**

In `tuning.rs`, in the nests section:

| Constant | Value | Doc must record |
|---|---|---|
| `NEST_CACHE_WORK_RESOURCE_MULT: u32` | `4` | Multiplier on an ordinary `WORK_RESOURCE_DROP` roll of the nest species' `work_resource`. |
| `NEST_CACHE_FRAGMENTS: RangeInclusive<u32>` | `2..=5` | Craft currency from a destroyed nest, before the zone bonus. Deliberately under `BOSS_PORTAL_FRAGMENT_DROP` (`3..=6`): a nest is sustained effort, a boss is a wall. |
| `NEST_CACHE_FRAGMENT_ZONE_BONUS: u32` | `1` | Added per zone below the current one, so a deeper nest — whose guardians already scale — stays worth clearing. Additive rather than multiplicative, matching `NODE_PAYOUT_ZONE_BONUS`; see that constant for why compounding broke the economy. |
| `NEST_CACHE_EQUIPMENT_ROLLS: u32` | `3` | Passes over the species' equipment drop table, each entry rolled at its own chance on each pass. Not a guarantee: a species whose table is empty, or whose chances are low, can still pay no gear at all. |

The spec said only "scaled by zone" without pinning how. This is that decision: additive, following the existing precedent. Record it in the spec's constants table too.

- [ ] **Step 2: Write the failing tests**

In `crates/engine/src/tests/zone.rs`:

1. `destroying_a_nest_grants_its_species_work_resource` — record inventory, drive `Durability` to 0 in one call, assert the species' `work_resource` count rose by at least `NEST_CACHE_WORK_RESOURCE_MULT * WORK_RESOURCE_DROP.start()`. Assert a lower bound, not an exact figure: the roll is a range and the inventory can be full.
2. `destroying_a_nest_grants_craft_currency` — same shape against `craft_currency()`.
3. `a_non_lethal_hit_on_a_nest_grants_nothing` — hit a full-durability nest once, assert the inventory is byte-identical.
4. `a_deeper_zone_pays_a_larger_nest_cache` — set `ZoneLevel` to 1 and to 4 with the same seed and species, assert the zone-4 currency grant exceeds the zone-1 one by at least `3 * NEST_CACHE_FRAGMENT_ZONE_BONUS`. Force the RNG or compare minimums; do not compare two unseeded rolls.
5. `destroying_a_nest_rolls_its_species_gear_table_repeatedly` — pick a species whose `equipment_drops_for` is non-empty and force the drop rolls to succeed (seed the RNG, or assert across enough seeded runs that the mean beats a single-pass expectation). Assert the destruction can yield more than one copy of a gear item, which is the only observable difference between `NEST_CACHE_EQUIPMENT_ROLLS = 3` and `= 1`. If forcing the roll turns out to be impractical without new plumbing, assert the weaker invariant — that a destroyed nest can drop gear at all — and say so in the test's doc comment rather than leaving a test that passes vacuously.
6. `the_cache_lines_are_loot_kind` — assert the log lines are `MessageKind::Loot`. They must survive `retain_outcomes_since_battle` and follow the player onto the map; `MessageKind::Info` would be pruned when the swarm fight ends, which is exactly when the player is reading.

- [ ] **Step 3: Run and watch them fail**

`cargo test -p feral-processes-engine cache`
Expected: FAIL — inventory unchanged on destruction.

- [ ] **Step 4: Implement**

`grant_nest_cache(nest)` reads `Nest::species`, clones the `SpeciesDef` out of `SpeciesDb` (clone, then drop the resource borrow — `award_loot` does exactly this at `combat_rewards.rs:62-67`; follow it), and grants the three parts. Model each on `award_loot`, including checking `landed > 0` before logging — a full inventory must not print a line claiming the player received something.

Call it from `attack_nest` inside the `if destroyed` branch, **before** `despawn_nest`, since it reads the nest's species off a component `despawn_nest` is about to delete. Order the log lines so the collapse line reads first and the cache follows it.

No XP. The guardians are the XP.

- [ ] **Step 5: Run the tests**

`cargo test -p feral-processes-engine cache`
Expected: PASS.

- [ ] **Step 6: Balance gate**

`cargo test -p feral-processes-engine balance_sim`
Expected: PASS, unchanged. `balance_sim` models no map, no pursuit and no nests, so nothing here should move a curve. **A curve that moves means you touched a shared constant by accident** — find it before continuing.

- [ ] **Step 7: Full gates and commit**

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt
git add -A && git commit -m "feat(nests): a destroyed nest pays a cache drawn from its species"
```

---

### Task 6: Persistence

Without this, save/reload deletes every nest in the zone — which both frees the player from any swarm and destroys a nest they were most of the way through, cache and all. A reward a reload can launder is a reward that teaches the player to reload.

**Files:**
- Modify: `crates/engine/src/save.rs` — `NestSave`, `SaveData::nests`, two `CreatureSave` fields, `SAVE_FORMAT_VERSION` 18 → 19 (line 237)
- Modify: `crates/engine/src/game/lifecycle.rs` — `load` (~line 129) and `save` (~line 457)
- Test: `crates/engine/src/tests/` — wherever the existing save round-trip tests live; find them with `rg -l "SAVE_FORMAT_VERSION|round_trip" crates/engine/src/tests/`

**Interfaces:**
- Produces:
  ```rust
  pub struct NestSave {
      pub species: SpeciesId,
      pub position: (i32, i32),
      pub durability: u32,
      pub pending_respawns: Vec<u32>,
  }
  ```
  plus `SaveData::nests: Vec<NestSave>`, `CreatureSave::nest_position: Option<(i32, i32)>` and `CreatureSave::pursuing: bool`.

- [ ] **Step 1: Write the failing tests**

1. `a_nest_survives_a_save_load_round_trip` — spawn a nest, damage it to a known non-full durability, push a `pending_respawns` entry, save, load, assert position, durability and `pending_respawns` all match.
2. `a_guardians_tether_survives_a_save_load_round_trip` — assert the reloaded guardian has `NestGuardian` pointing at the reloaded nest entity (compare via the nest's `Position`, not the raw `Entity` — ids are not stable across a round trip, which is the whole reason the link is keyed by tile).
3. `live_aggro_survives_a_save_load_round_trip` — provoke, save, load, assert `Pursuing` is still set.
4. `a_creature_whose_nest_is_missing_loads_as_an_ordinary_wild_program` — hand-build a `CreatureSave` with a `nest_position` matching no `NestSave`, assert it loads with `Hostile` and `WanderAi` and without `NestGuardian`, and that the load does not fail.

- [ ] **Step 2: Run and watch them fail**

`cargo test -p feral-processes-engine round_trip`
Expected: FAIL to compile — `NestSave` does not exist.

- [ ] **Step 3: Add the save types and bump the version**

`SAVE_FORMAT_VERSION` 18 → 19. Both `CreatureSave` fields are shape changes and bincode has no field-level compatibility here — `custom_name`'s doc comment at `save.rs:82-87` records exactly this precedent; follow its wording.

Doc-comment the `nest_position` field with *why it is a tile and not an `Entity`*: entity ids do not survive a round trip, and this follows `CronjobSave::target_position`, the existing precedent for the same problem. One nest per tile, so the key is unambiguous.

- [ ] **Step 4: Write nests out**

In `save` (`lifecycle.rs:457+`), beside the existing structure query, add a `(&Nest, &Position, &Durability)` query and collect `NestSave`s.

Extend the existing creature query tuple with `Option<&NestGuardian>` and `Option<&Pursuing>`. Fill `nest_position` by resolving the guardian's nest entity to its `Position` — the same `and_then(|t| self.world.get::<Position>(t.target)...)` shape the `cronjob` field already uses two lines up. Fill `pursuing` with `.is_some()`.

- [ ] **Step 5: Read nests back**

In `load` (`lifecycle.rs:129+`), spawn nests **before** the `for c in data.creatures` loop, building a `HashMap<(i32, i32), Entity>` as you go — mirroring the `structure_positions` map that already exists a little further down for exactly this reason.

Spawn each nest with the same component set `Game::spawn_nest` uses (`Nest`, `Position`, `Glyph` from the species' colour, `Durability`), reading `max_hp` from `NEST_DURABILITY` and `hp` from the save. Skip a `NestSave` whose species is no longer in `SpeciesDb`, matching how the creature and structure loops already skip unknown ids — a removed mod must not fail the load.

Then in the wild branch at `lifecycle.rs:343` (`entity.insert((Hostile, WanderAi::default()));`), look up `c.nest_position` in that map and insert `NestGuardian { nest }`, plus `Pursuing` if `c.pursuing`. A `nest_position` that resolves to nothing is dropped silently and the creature loads as an ordinary wild program — cheaper than failing the load over it, and covered by test 4.

- [ ] **Step 6: Run the tests**

`cargo test -p feral-processes-engine round_trip && cargo test -p feral-processes-engine save`
Expected: PASS.

- [ ] **Step 7: Check the savetool still round-trips**

The save format changed, so `savetool`'s RON dump/pack path must still work:

```bash
cargo run --bin savetool -- template
cargo run -- --template extraction   # then save in-game, or use an existing dev-save
cargo run --bin savetool -- dump saves/save.bin /tmp/claude-1000/-home-trog-code-feral-processes/3a4e7c5d-2f04-4a77-bd7f-3eae177bb91d/scratchpad/s.ron
cargo run --bin savetool -- pack /tmp/claude-1000/-home-trog-code-feral-processes/3a4e7c5d-2f04-4a77-bd7f-3eae177bb91d/scratchpad/s.ron /tmp/claude-1000/-home-trog-code-feral-processes/3a4e7c5d-2f04-4a77-bd7f-3eae177bb91d/scratchpad/save.bin
```

Expected: the dump contains a `nests: []` (or populated) field and the pack succeeds. **Every existing file in `dev-saves/` is now version 18 and will be rejected by a version-19 load.** Recapture any template you need, or note in the commit which ones are stale — do not silently leave a broken `--template`.

- [ ] **Step 8: Full gates and commit**

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt
git add -A && git commit -m "feat(save)!: nests, guardian tethers and live aggro are persisted"
```

---

### Task 7: Documentation

Not optional and not a footnote: `docs/manual.md` currently asserts things this branch makes **false**.

**Files:**
- Modify: `docs/manual.md` §Nests (~lines 1013-1035)
- Modify: `CHANGELOG.md`
- Modify: `README.md`
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`
- Modify: `docs/superpowers/specs/2026-08-03-nest-aggression-design.md`

- [ ] **Step 1: Find every claim this branch falsifies**

```bash
rg -n -i "nest" docs/manual.md README.md CHANGELOG.md
```

Known false after this work, at minimum:
- manual ~line 1026-1031: "That's a plain hit … Raids never target a nest; its Durability is only ever spent by you" — the first half survives, but the surrounding claim that a nest never retaliates does not.
- manual ~line 1035: "A nest is a deliberate risk/reward pocket" — now actually true, and should describe the cache rather than gesturing at one.

Do not stop at the grep. Read the surrounding paragraphs; the false claim is often the sentence *next to* the one containing the word.

- [ ] **Step 2: Rewrite the manual's Nests section**

Cover, in the manual's existing voice: hitting a nest provokes every guardian; they path to you and engage, folding in whatever else is standing nearby; they give up past a distance from the nest and walk home; they will not follow you onto the base platform; a besieged nest keeps producing pre-aggroed guardians; and destroying one pays a cache of the species' work resource, Portal Fragments scaled by zone, and rolls of its gear table.

- [ ] **Step 3: CHANGELOG**

The save-format bump above all — it is what "breaking" means for this project (see the workspace `[workspace.package]` comment). Then the feature and the tether fix, which is a user-visible bug fix in its own right.

- [ ] **Step 4: A load-bearing-seams entry in `CLAUDE.md`**

Two facts here cost real tool calls to rediscover and belong in the seams list:

- **`walkable()` alone does not decide where a pursuer may step** — `Biome::Platform` is excluded separately in `pursuit_field`, because the leash is measured from the nest and cannot keep a swarm off a base built near one. A second "walkable but off-limits" rule goes in that filter, not beside it.
- **A guardian's tether refuses a step only when it both leaves the radius and fails to close on the nest.** The simpler "outside the radius" check was total until a chase could displace a guardian, at which point it froze them permanently. Anything that can move a `NestGuardian` outside `NEST_TETHER_RADIUS` depends on this.

Then `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking to catch drift.

- [ ] **Step 5: Reconcile the spec**

The spec is the record of intent and two decisions moved during implementation. Amend it:
- The cost field is centred on the **player**, not on the nest (Task 4, step 3).
- `NEST_CACHE_FRAGMENT_ZONE_BONUS` pins "scaled by zone", which the spec left open (Task 5, step 1).

- [ ] **Step 6: Full gates and commit**

```bash
cargo test --workspace && cargo clippy --workspace && cargo fmt
git add -A && git commit -m "docs(nests): aggression, the cache and the save bump"
```

---

## After the plan

**The suite passing is not evidence the game is good.** Six new tuning constants ship here and `balance_sim` gates none of them — it models no map, no pursuit and no nests. Two are most likely to be wrong:

- `NEST_PURSUIT_STEPS_PER_TICK = 1` may make a swarm trivially kiteable, since it exactly matches player speed.
- A besieged nest producing pre-aggroed guardians means a patient player never runs out of enemies. That is the pressure clock by design, and it is also the most likely thing to feel unfair.

Launch it and play before calling this done:

```bash
cargo run -- --template extraction
```

Then walk until you find an `N` and hit it.
