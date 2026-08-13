# Creeping Base Footprint Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Halve the starting base footprint and add a research-gated structure that creeps the platform edge outward one tile at a time.

**Architecture:** `MAX_BUILD_DISTANCE_FROM_HOME` stops being the footprint and becomes only its *starting* value. The live radius is derived from deployed structures (`Game::build_radius()`, the same shape as `Game::pet_capacity` over `pet_slot_bonus`) and cached on the `Platform` resource, which is where `Platform::covers` reads it. Because the radius is derived rather than stored, it rebuilds on load and travels through a breach for free, and costs no save-format change.

**Tech Stack:** Rust, `bevy_ecs` 0.19 standalone, RON assets via serde.

**Spec:** `docs/superpowers/specs/2026-08-13-creeping-base-footprint-design.md` — read it first. It carries the arguments; this plan carries the order.

## Global Constraints

- **No `SAVE_FORMAT_VERSION` bump.** `Platform` is not serialized. Any new `StructureDef` field is `#[serde(default)]`. If you find yourself needing to save the radius, stop — the design is wrong, not the save format.
- **`Platform::covers` stays the single statement of the footprint.** One function, three callers (`stamp_platform`, `clear_platform`, `place_structure`). Never inline the shape at a call site.
- **Content is data.** The structure and its research gate are `.ron` files. No new content hardcoded in Rust. Update `assets/structures/README.md` in the same change as the schema.
- **Tuning lives in `tuning.rs`** as documented `pub const`s, never inline in a formula.
- **No occult naming** in player-facing content.
- **Comment discipline:** comments explain *why*. Every doc comment this plan tells you to rewrite is being rewritten because it has become false — rewrite it, don't append to it.
- **Gates, every task:** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`. `tuning.rs` changes additionally run `cargo test -p feral-processes-engine balance_sim`.
- **Do not push, tag, bump the version, or merge.** Commit freely on the branch; every outward-facing action needs an explicit ask.

---

### Task 1: Halve the start, decouple the opening ring

Constants only. Lands first so every later task is written against the final geometry, and so the test fallout is isolated in one commit.

**Files:**
- Modify: `crates/engine/src/tuning.rs:1656` (`MAX_BUILD_DISTANCE_FROM_HOME`), `:154` (`OPENING_RING_TILES`)
- Test: existing suite; `crates/engine/src/tests/building.rs`, `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `MAX_BUILD_DISTANCE_FROM_HOME: i32 = 4`; `OPENING_RING_TILES: i32 = 7` (its own literal, no longer derived).

- [ ] **Step 1: Run the suite first and record the baseline.** `cargo test --workspace`. You need to know what was green before you change a difficulty constant.
- [ ] **Step 2: Change both constants.** `MAX_BUILD_DISTANCE_FROM_HOME` 7 → 4. `OPENING_RING_TILES` from `= MAX_BUILD_DISTANCE_FROM_HOME` to a literal `7`.
- [ ] **Step 3: Rewrite both doc comments.** `MAX_BUILD_DISTANCE_FROM_HOME`'s must now say it is the *starting* radius and point at `Game::build_radius` as the live one (forward reference is fine; Task 2 adds it). `OPENING_RING_TILES`'s currently argues "Set to `MAX_BUILD_DISTANCE_FROM_HOME` so the ring is exactly your base and its doorstep, and travels with the base for free" — that argument is now *wrong*, not merely stale: a derived ring would widen every time the player builds a Pillar, making the nursery a difficulty knob keyed to base geometry. Record that as the reason it was decoupled.
- [ ] **Step 4: Run the suite.** Expect failures in tests that assert absolute tile positions against the old 15×15 slab. Fix each by making it symbolic (`MAX_BUILD_DISTANCE_FROM_HOME + 1` rather than `8`), never by hardcoding the new number. A test that needs a *literal* is a test whose intent you have not understood yet — read it before touching it.
- [ ] **Step 5: Run `cargo test -p feral-processes-engine balance_sim`.** It should be untouched: the sim is RNG-free and models no spawn geometry. If a curve moved, stop and report — that means something reads the radius you have not found.
- [ ] **Step 6: Commit.** `tune(base): halve the starting build radius, decouple the opening ring`

---

### Task 2: `build_radius_bonus` and `Game::build_radius()`

Additive and inert — nothing reads the result yet, so the suite must stay green with no test edits outside this task's own.

**Files:**
- Modify: `crates/engine/src/structures.rs` (near `pet_slot_bonus`, ~`:278`), `crates/engine/src/tuning.rs`, `crates/engine/src/game/catalog.rs` (beside `pet_capacity`, `:414`)
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Consumes: Task 1's constants.
- Produces:
  - `StructureDef::build_radius_bonus: i32`, `#[serde(default)]`
  - `tuning::MAX_BUILD_RADIUS_TILES: i32 = 10`
  - `Game::build_radius(&mut self) -> i32` — `MAX_BUILD_DISTANCE_FROM_HOME` plus the summed bonus of every deployed structure, clamped to `MAX_BUILD_RADIUS_TILES`. `&mut self` because querying structures needs it, exactly like `pet_capacity`.

- [ ] **Step 1: Write the failing tests.** Three, and the third is the one that matters:
  - a bare `Game::new` reports `build_radius() == MAX_BUILD_DISTANCE_FROM_HOME`
  - spawning two structures whose defs each set `build_radius_bonus: 1` reports base + 2
  - enough bonus to exceed the cap reports exactly `MAX_BUILD_RADIUS_TILES` — write this with a modded def carrying a large bonus rather than by spawning ten structures, so it tests the clamp and not your patience
- [ ] **Step 2: Run them, confirm they fail to compile** (`no method named build_radius`). A test that fails for the wrong reason is not a red test.
- [ ] **Step 3: Add the field, the constant and the method.** The field's doc comment follows `pet_slot_bonus`'s exactly in shape, including the sentence explaining why `#[serde(default)]` means existing mods are unaffected. `MAX_BUILD_RADIUS_TILES`'s doc must say what the cap is *for* — the spec's argument is that the pre-2026-07-24 31×31 base was judged too big, and that a slab past this point swallows the Stack on-ramp draw box (Task 5).
- [ ] **Step 4: Run the tests, then the workspace suite.** Both green. Nothing else in the game reads the new field yet.
- [ ] **Step 5: Commit.** `feat(structures): a def field for extra build radius`

---

### Task 3: `Platform` carries the live radius

The type flip. Behaviour must be *identical* after this task — no shipped def sets a bonus yet, so `build_radius()` still returns the base value everywhere.

**Files:**
- Modify: `crates/engine/src/resources.rs:599-620` (`Platform`, `covers`), `crates/engine/src/game/zone.rs:243` (`stamp_platform`), `:328` (`clear_platform`), `crates/engine/src/game/building.rs:36` (`place_structure`), `crates/engine/src/game/lifecycle.rs:687` (load)
- Test: `crates/engine/src/tests/building.rs`, `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Consumes: `Game::build_radius()`.
- Produces: `Platform { center: Option<(i32, i32)>, radius: i32 }`; `Platform::covers(&self, dx: i32, dy: i32) -> bool`.

- [ ] **Step 1: Write the failing test** — a save/load round trip with a structure carrying a `build_radius_bonus` restores the same `Platform.radius`. This is the regression that proves the derived-not-stored claim, and it must pass at the current `SAVE_FORMAT_VERSION`.
- [ ] **Step 2: Run it, confirm it fails.**
- [ ] **Step 3: Add the field and flip the signature.** `radius` defaults to `MAX_BUILD_DISTANCE_FROM_HOME`.
- [ ] **Step 4: Write the radius at the three sites that already write `center`, and nowhere else.** That list is the invariant — if you find yourself writing it at a fourth, the design has drifted.

  `stamp_platform` has a borrow-ordering constraint. `build_radius()` takes `&mut self`, and the existing body then holds `resource_mut::<WorldMap>()` across the stamping loop. Compute first, hold second:

  ```rust
  let radius = self.build_radius();          // &mut self borrow ends here
  self.world.resource_mut::<Platform>().radius = radius;
  // ...existing map borrow and stamp loop, reading `radius` as a local
  ```

  The load path (`lifecycle.rs:687`) sets both `center` and `radius` — structures are already restored by that point, so `build_radius()` is correct there.
- [ ] **Step 5: Update the three `covers` callers** to go through the resource. `clear_platform` is deliberately *not* one of the "live radius" readers — see Task 4.
- [ ] **Step 6: Rewrite `covers`'s doc comment.** It currently says the shape is "deliberately a function of the offset rather than of the resource". That is now false in the letter and true in the spirit: it is a function *of the resource*, and it is still the one statement of the footprint with the same three callers for the same reason. Say both.
- [ ] **Step 7: Run the workspace suite.** Everything green with no edits to unrelated tests. If a test moved, the flip was not behaviour-preserving — find out why before proceeding.
- [ ] **Step 8: Commit.** `refactor(base): the platform footprint reads a live radius`

---

### Task 4: The downstream readers follow

**Files:**
- Modify: `crates/engine/src/game/spawning.rs:384` (`distance_from_danger_origin`), `:573` (`spawn_initial_creatures`), `crates/engine/src/game/zone.rs:334` (`clear_platform`), `crates/engine/src/game/hauling.rs:161`, `crates/engine/src/tuning.rs:1501` (`HAUL_WALK_RADIUS`)
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Consumes: `Platform.radius`, `Game::build_radius()`.
- Produces: `HAUL_WALK_RADIUS` deleted; the walk reach is `radius * 2` read live at both `walk_field` call sites.

- [ ] **Step 1: Write the failing test** — a program posted to a machine at the far edge of a *fully grown* base still reaches its station. Use `park_at_post` / `stand_player_at_post` from `crates/engine/src/tests/support.rs` rather than writing a new fixture; check what is there first. A hand-spawned work node needs `work_node_parts()` or it is silently skipped by `task_progress_system`'s query and reads as a payout curve that moved.
- [ ] **Step 2: Run it, confirm it fails** — the worker never arrives, because a walk of `4 * 2 = 8` cannot cross a radius-10 base.
- [ ] **Step 3: Point the two `spawning.rs` readers at `Platform.radius`.** Both currently branch on `center.is_some()` and subtract or add the constant; they keep that branch and change what they add.
- [ ] **Step 4: Delete `HAUL_WALK_RADIUS` and thread the live value.** `walk_field` already takes the reach as a parameter, so this is the two callers (`haul_step_system`, which can take `Res<Platform>`, and `assign_cronjob`, which has `&mut Game`). `crates/engine/src/tests/building.rs:4` imports the constant — those tests need the live value too, not a re-introduced local copy.
- [ ] **Step 5: `clear_platform` sweeps `MAX_BUILD_RADIUS_TILES`, not the live radius.** This is the one deliberate disagreement and it needs a comment saying so. Its existing doc already explains sweeping the full box rather than `covers`'s cut shape, because a save written before the corners were cut would otherwise keep orphan floor forever. Task 1 recreated exactly that situation for every existing save — a 15×15 slab in `tile_overrides` against a base radius of 4 — so the sweep must cover the largest slab that could ever have existed.
- [ ] **Step 6: Write the second failing test** — load a save whose `tile_overrides` carry a slab wider than the current radius, demolish the Home, and assert no `Biome::Platform` tile survives anywhere. Build the fixture by stamping at an inflated radius rather than by checking in a save file.
- [ ] **Step 7: Run both tests, then the workspace suite.**
- [ ] **Step 8: Commit.** `feat(base): hauling and spawn scatter follow the live footprint`

---

### Task 5: The Stack on-ramp, and depth measured from the edge

The safety-critical task. It lands *before* anything can grow, because it is what makes growth survivable.

**Files:**
- Modify: `crates/engine/src/game/stack.rs:140` (`spawn_surface_links`), `:102` (`frames_for`), `crates/engine/src/tuning.rs:691` (`STACK_NEAREST_LINK_TILES` doc)
- Test: `crates/engine/src/tests/` — the Stack test module; find where `spawn_surface_links` is currently covered rather than starting a new file

**Interfaces:**
- Consumes: `Platform.radius`.
- Produces: no signature change to `spawn_surface_links`. `frames_for(tile, spawn)` gains the radius — either as a third parameter or by taking the edge-adjusted distance; pick one and state it in the commit.

- [ ] **Step 1: Write the failing test** — with a base stamped at `MAX_BUILD_RADIUS_TILES`, a zone still receives all `STACK_LINKS_PER_ZONE` links. This must fail against the current code, and understanding *why* is the whole task: `reach` widens to `STACK_LINK_SCATTER_TILES` only once `placed > 0`, and the attempt budget is `count * 40` shared across all three links — so an on-ramp that can never land starves every link, not just the first.
- [ ] **Step 2: Run it, confirm it fails with zero links placed** — not one, not two. If it fails with two links you have built the fixture wrong.
- [ ] **Step 3: Draw the on-ramp from the ring outside the slab.** Replace the `placed == 0` box draw with a draw from Chebyshev `radius + 1 ..= radius + STACK_NEAREST_LINK_TILES`. Drawing a uniform point on a Chebyshev *ring band* is not a uniform box draw — pick the band offset then the position along it, or rejection-sample within the outer box against the inner bound. Whichever you choose, the existing `STACK_MIN_LINK_TILES` rejection stays: it is what keeps a link off the arrival tile when no base exists.

  This function seeds a local `StdRng` off the world seed and must keep doing so. Do not reach for `GameRng` — a level drawn from it regenerates differently after a save/load and strands the party inside rock.
- [ ] **Step 4: Run the test, confirm three links.** Then run the existing Stack tests: link placement is seeded, so any test asserting a specific link position will have moved. Re-baseline those deliberately and say so in the commit — a seeded position changing is expected here, a *count* changing is not.
- [ ] **Step 5: Rewrite `STACK_NEAREST_LINK_TILES`'s doc.** Its argument is the viewport — "the pane shows roughly ±16 by ±9 tiles, so anything past this is off screen when the player materializes". At a grown radius the base has eaten that viewport itself and the promise cannot be kept. The constant now means "on your doorstep", and `announce_surface_links` is what keeps the layer discoverable. Rewrite, don't append.
- [ ] **Step 6: Write the second failing test** — the nearest link opens a stack of the same depth at base radius and at the cap.
- [ ] **Step 7: Make `frames_for` measure from the slab edge.** Same correction `distance_from_danger_origin` already makes so the whole base counts as distance zero. Its doc claims it uses "the same distance-from-arrival that already scales wild program stats" — that claim only stays true if both subtract the radius, which is the point.
- [ ] **Step 8: Run both tests, then the workspace suite.**
- [ ] **Step 9: Commit.** `fix(stack): place the on-ramp outside the base slab`

---

### Task 6: The Heap Pillar

**Files:**
- Create: `assets/structures/heap_pillar.ron`, `assets/research/<gate>.ron`
- Modify: `crates/engine/src/game/building.rs` (`place_structure`, `remove_structure`)
- Test: `crates/engine/src/tests/building.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: the shipped structure; `place_structure` re-stamps and refuses on a link; `remove_structure` refuses a Pillar outside a Home cascade.

- [ ] **Step 1: Write four failing tests.**
  - placing a Pillar turns a tile at the old edge + 1 into `Biome::Platform`
  - a structure builds at the new edge and is refused one tile past it
  - a Pillar whose new ring contains a `SurfaceLink` is refused **and the player's inventory is unchanged** — the second half is the one that would pass against the bug, so it is the assertion that matters
  - `remove_structure` refuses a Pillar; demolishing the Home takes it in the cascade
- [ ] **Step 2: Run them, confirm all four fail.**
- [ ] **Step 3: Write the two assets.** `heap_pillar.ron`: name "Heap Pillar", glyph `I`, colour `Cyan`, `build_radius_bonus: 1`, `raidable: false`, `work: None`, and a `build_cost` sitting between the Data Cache's and the researched benches' — read three or four existing `assets/structures/*.ron` and price it against them rather than inventing a number. Remember it is bought repeatedly, six times to reach the cap. The research file lists `heap_pillar` in `unlocks_structures` and `requires` an existing node — read the current tree before picking, and follow the shape of `assets/research/fortification.ron`.
- [ ] **Step 4: Add the link refusal to `place_structure`, before the materials check.** It belongs with the other refusals, above the point where `Inventory::take` runs. Same ordering argument as `use_symlink` calling `clear_stack` only after every check passes, and `install_routine` taking the disk last: a refused action must not have spent anything. Scan only the *new* ring — the existing slab has no links in it by construction.
- [ ] **Step 5: Re-stamp on placement.** `place_structure` calls `stamp_platform` for a Home today; it gains a second condition for any def with `build_radius_bonus > 0`, stamping at the Home's position. Re-laying the inner slab rewrites the same overrides, so this is idempotent.
- [ ] **Step 6: Add the demolition refusal**, exempting the Home cascade — in `remove_structure` the cascade is the `is_home` branch that extends `targets`, so the refusal guards the single-target path only.
- [ ] **Step 7: Run the four tests, then the workspace suite.**
- [ ] **Step 8: Write the last two tests and confirm them green** — the widened slab survives a breach at the right size around the new spawn point, and a Pillar's radius survives a save/load. Both should already pass from Tasks 3 and 5; if either fails, something writes the radius outside the three sanctioned sites.
- [ ] **Step 9: Commit.** `feat(base): the Heap Pillar creeps the base edge outward`

---

### Task 7: Documentation

**Files:**
- Modify: `assets/structures/README.md`, `CHANGELOG.md`, `CLAUDE.md` (then `cp CLAUDE.md AGENTS.md`), `TODO.md`

- [ ] **Step 1: Document `build_radius_bonus` in `assets/structures/README.md`** — that file is the schema reference for anyone modding, and the rule is that it changes in the same commit as the field.
- [ ] **Step 2: Add the `CHANGELOG.md` section.** Do **not** bump the version — that happens once, at the merge. Which digit moves is decided by `CHANGELOG.md`'s own preamble; read it. No save stops loading here, so this is not breaking.
- [ ] **Step 3: Update `CLAUDE.md`'s load-bearing seams.** At least three entries are now false or incomplete: anything describing the build radius as a constant, the opening-ring entry (which now needs to say the ring was decoupled and why), and the Stack-link placement description. Add a new seam for the derived-not-stored radius and the three sites that write it — that is the fact the next session will otherwise rediscover by tool call. `CLAUDE.md` and `AGENTS.md` are gitignored twins with nothing tracking drift, so edit `CLAUDE.md` and `cp` it.
- [ ] **Step 4: Check `TODO.md`** for the known gap about nests, surface links and zone portals being invisible to the examine ray — Task 5 moves where links are placed but does not close that, so it should still read true.
- [ ] **Step 5: Grep for claims this falsifies.** `rg -n "15x15|31x31|build radius|MAX_BUILD_DISTANCE" docs/ *.md` and fix what is now wrong. `docs/manual.md` is explicitly carved out of the doc obligation — leave it stale. So is the root `README.md`.
- [ ] **Step 6: Full gates.** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`, `cargo test -p feral-processes-engine balance_sim`.
- [ ] **Step 7: Commit.** `docs: record the creeping base footprint`

---

## After the plan

**This has not been played.** The feature is entirely visual — the goal is that the base *reads* as a settlement that grew — and a green suite is no evidence of that at all. Before this is called done:

- `FERAL_DEV_REVEAL=1 cargo run -- --template <a mid-run template>` and look at the halved slab. 9×9 should read as cramped, not as broken.
- Build Pillars and watch the edge creep. One tile at a time may be too subtle to notice, or exactly right — that is the question the arena cannot answer.
- Breach with a fully grown base and confirm the zone has its links.

`cargo run --bin savetool -- template` lists what is available; capture a new one if none fits.
