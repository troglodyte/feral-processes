# Base Out Of Phase — Slice 1 Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Move the base off the zone surface and into its own space, entered
through a permanent anchor, with play feeling identical afterwards.

**Architecture:** A new sparse `BaseGrid` resource replaces `resources::Platform`
as the statement of the base's footprint. `resources::Locale` gains a third
variant, `Base { x, y }`, and the player's surface `Position` pins to the anchor
tile exactly as it pins to a Stack entrance today. Every `Structure` entity is in
base space by construction — there is one spawn site — so no new marker component
is needed. Deploying the Home lays a pre-cleared pocket instead of stamping a
slab of `Biome::Platform` tiles into `WorldMap`.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine, standalone), RON saves via serde,
`bevy` + `bevy_egui` (gui). Workspace: `crates/engine`, `crates/app-core`,
`crates/gui`, `crates/launcher`.

**Spec:** `docs/superpowers/specs/2026-08-19-base-out-of-phase-design.md` — read
it first. This plan implements **slice 1 only**; slices 2 and 3 are separate.

## Global Constraints

Copied from `CLAUDE.md` and the spec. Every task's requirements include these.

- **The engine's `Game` struct is the entire public API the renderer talks to.**
  `Game::world` is private with no accessor. Never add one. `crates/gui` now has
  `bevy_ecs` in its graph via `bevy`, so a `pub fn world_mut()` would compile and
  be immediately usable — the private field is the whole barrier.
- **`crates/gui/src/paint.rs` is the only file that may name a graphics library.**
  Everything in `crates/gui/src/render/` draws through `Painter`'s fourteen
  operations. This slice must not widen `Painter`.
- **Tuning values go in `crates/engine/src/tuning.rs`** as documented `pub const`,
  never inline in a formula, never duplicated from a `.ron`.
- **Content stays moddable.** New fields on `StructureDef`/`ItemDef`/etc. need
  `#[serde(default)]`; a malformed `.ron` is skipped with a logged warning, never
  a panic; update the matching `assets/*/README.md` in the same change.
- **No flaky tests.** No `sleep()`, no wall-clock, no unseeded RNG. Background
  systems (habitat spawning, nests) will interfere with naive assertions.
- **Gates:** `cargo fmt`, `cargo clippy --workspace` (fix, don't silence),
  `cargo test --workspace` before calling anything done. Iterate with
  `cargo test -p feral-processes-engine <name>`.
- **Commits are free and expected at each green step. Do not push.** Do not
  bump the workspace version or write a `CHANGELOG.md` section — that happens
  once, at the merge, per the repo's release policy.
- **Do not write to `TODO.md`** — it is the user's own list.
- **`docs/manual.md` and the root `README.md` are carved out** of the doc
  obligation. `CHANGELOG.md`, `assets/*/README.md` and `docs/seams.md` are not.
- **This is a worktree.** Work only in
  `.claude/worktrees/base-out-of-phase`. Never `cd` to the primary checkout.
  Never bare `git stash` — the stash stack is shared across worktrees.
- **`SAVE_FORMAT_VERSION` is 31 and becomes 32 in Task 2.** One bump for the
  whole slice; later tasks must not bump it again.

## Known traps

Read these before starting. Each has cost this repo time before.

1. **A seeded test can pass under `--workspace` and fail under
   `-p feral-processes-engine`,** and vice versa — they are different builds and
   the RNG stream differs. If a seeded test fails, run both before theorising.
2. **Registering a new `Resource` shifts bevy's query iteration order.** A
   failure in an untouched subsystem right after Task 1 is a latent unsorted-query
   test, not a regression you introduced. Fix it by sorting the query, not by
   changing the seed.
3. **Mass `NotFound` failures on an assets path are stale build artifacts**, not
   bugs — this repo moved from `/home/trog/code/petmud`. Fix with
   `cargo clean -p feral-processes-engine -p feral-processes-app-core`. Never a
   full `cargo clean`; `target/` is ~4 GB.
4. **A RON round-trip test cannot catch a `#[serde(skip)]`.** Every new save
   field needs a real save→load test as well.
5. **`Biome::Platform` survives this slice as a *rendering* vocabulary word**
   even though nothing writes it into `WorldMap` any more. That looks like a
   contradiction with the spec's deletion list; it is not. The deletion is of the
   *writes*, not the variant.

## File structure

| File | Responsibility after slice 1 |
| --- | --- |
| `crates/engine/src/base_grid.rs` (new) | `BaseGrid`, `BaseCell`, and every predicate over them. Mirrors `world.rs`'s role for `WorldMap`. |
| `crates/engine/src/resources.rs` | `Locale` gains `Base`. `Platform` is deleted from here. |
| `crates/engine/src/components.rs` | `BaseAnchor` marker. |
| `crates/engine/src/game/base_space.rs` (new) | Entering, leaving, the pocket, and movement within base space. Keeps `game/stack.rs` as its shape reference. |
| `crates/engine/src/game/stack.rs` | `require_surface` splits here; `is_underground` unchanged. |
| `crates/engine/src/game/zone.rs` | `stamp_platform`/`clear_platform` deleted; `enter_next_zone` stops rebuilding the base. |
| `crates/engine/src/game/base/building.rs` | Footprint checks read `BaseGrid`. |
| `crates/engine/src/save.rs` | `base_grid` and `anchor` in, `claimed_tiles` out, version 32. |
| `crates/engine/src/views.rs` | `view_tiles` dispatches on locale. |
| `crates/gui/src/render/base.rs` | Two new biome colours; no other change. |
| `crates/app-core/src/app/playing.rs` | The key that enters and leaves. |

---

### Task 1: `BaseGrid` and its predicates

**Files:**
- Create: `crates/engine/src/base_grid.rs`
- Modify: `crates/engine/src/lib.rs` (declare the module; register the resource in `Game::new`)
- Test: `crates/engine/src/tests/base_grid.rs` (new), declared in `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Consumes: nothing.
- Produces, all used by Tasks 2–8:
  - `pub struct BaseGrid` — a `Resource`, holding a sparse map keyed `(i32, i32)`.
  - `pub enum BaseCell { Open { mined_at: u64 }, Floor }` — absent means Solid.
  - `pub fn is_floor(&self, x: i32, y: i32) -> bool`
  - `pub fn is_solid(&self, x: i32, y: i32) -> bool` — true when absent.
  - `pub fn cell(&self, x: i32, y: i32) -> Option<BaseCell>`
  - `pub fn walkable(&self, x: i32, y: i32) -> bool` — Open or Floor.
  - `pub(crate) fn lay_floor(&mut self, x: i32, y: i32)`
  - `pub(crate) fn open(&mut self, x: i32, y: i32, tick: u64)` — used by slice 2; add it now so the type is complete, and cover it by test here.
  - `pub fn floor_count(&self) -> usize` — what #35 will eventually tie capacity to, and the cheapest assertion for "the pocket was laid".

**Non-obvious constraint:** the map must have a deterministic iteration order
wherever it is *serialised or iterated for gameplay*, for the same reason
`Stock` keys by `ItemId` in a `BTreeMap` — a `HashMap` makes the save encoding
differ run to run. Use `BTreeMap<(i32, i32), BaseCell>`.

- [ ] **Step 1: Write the failing tests.** Intent, one test each: a fresh grid
  reports every coordinate solid and `floor_count() == 0`; `lay_floor` makes that
  coordinate `is_floor` and not `is_solid`; `open` makes it walkable but not
  floor; `lay_floor` over an `Open` cell replaces it rather than stacking; a grid
  built by inserting cells in scrambled order iterates in ascending key order.
- [ ] **Step 2: Run them and watch them fail** — `cargo test -p feral-processes-engine base_grid`. Expected: does not compile, `BaseGrid` undefined.
- [ ] **Step 3: Implement `base_grid.rs`.** Nothing here is subtle; keep it to the interface above.
- [ ] **Step 4: Register the resource** in `Game::new` alongside the other world resources.
- [ ] **Step 5: Run the engine suite** — `cargo test -p feral-processes-engine`. Read trap 2 above before diagnosing any failure outside `base_grid`.
- [ ] **Step 6: `cargo fmt` and `cargo clippy --workspace`, then commit.**

---

### Task 2: The save format moves to 32

**Files:**
- Modify: `crates/engine/src/save.rs` (`SAVE_FORMAT_VERSION` at line 659; `PlayerSave`; both load paths)
- Modify: `crates/engine/src/game/lifecycle.rs` (the save and load bodies)
- Test: `crates/engine/src/tests/base_grid.rs` — there is no dedicated save-test file; the convention here is that each feature tests its own save round trip in its own file

**Interfaces:**
- Consumes: `BaseGrid` from Task 1.
- Produces: a save that round-trips `BaseGrid`, and `PlayerSave::claimed_tiles` gone.

**Why the bump:** field-named RON makes *additive* change free, but
`claimed_tiles` is a field **removed**, which is exactly the case it does not
excuse. Old saves are refused by version, on the file's first line, by design —
no migration path.

- [ ] **Step 1: Write the failing tests.** Intent: a `Game` with a hand-laid
  `BaseGrid` saves and loads with the identical grid (a real save→load, not only
  the RON round trip — see trap 4); a save file's first line reads 32; a save
  written at 31 is refused rather than partially read.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Add `base_grid` to `PlayerSave`, remove `claimed_tiles`, bump the version to 32, and wire both load paths.**
- [ ] **Step 4: Run the tests, then the engine suite.** Expect existing save tests that mention `claimed_tiles` to fail — update them; that is the intended blast radius.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 3: `Locale::Base`, and splitting `require_surface`

**Files:**
- Modify: `crates/engine/src/resources.rs:1031` (the `Locale` enum)
- Modify: `crates/engine/src/game/stack.rs:317` (`require_surface`)
- Modify: the eleven guard sites listed below
- Modify: `crates/engine/src/tests/support.rs` (a fixture that puts a `Game` into `Locale::Base` directly — nothing can *walk* in until Task 5)
- Test: `crates/engine/src/tests/base_space.rs` (new)

**Interfaces:**
- Produces:
  - `Locale::Base { x: i32, y: i32 }`
  - `Game::in_base(&self) -> bool`
  - `Game::base_pos(&self) -> Option<(i32, i32)>` — mirrors `Game::stack_pos`, and is the refusal mechanism for anything base-only, exactly as `stack_pos` returning `None` is what makes `cast_field_routine`'s `Phase`/`Jump` Stack-only.
  - `Game::require_surface(&self) -> Result<(), String>` — narrowed to "on the surface proper".
  - `Game::require_base(&self) -> Result<(), String>` — new.
- Consumes: nothing from Tasks 1–2.

**This is the highest-risk task in the slice.** `require_surface` does not mean
what its name says: it means "not in the Stack", and today that is the same thing
as "on the surface". A third locale forces each site to declare which it meant,
and **a wrong answer is silent** — nothing fails to compile.

| Site | Becomes |
| --- | --- |
| `game/base/building.rs:15` | `require_base` |
| `game/base/building.rs:374` | `require_base` |
| `game/base/building.rs:457` | `require_base` |
| `game/base/building.rs:652` | `require_base` |
| `game/base/work_orders.rs:954` | `require_base` |
| `game/base/collect.rs:28` | `require_base` |
| `game/trade.rs:58` | `require_base` — the Black Market is a deployed `Structure` |
| `game/trade.rs:216` | `require_base` |
| `game/trade.rs:448` | `require_base` |
| `game/trade.rs:489` | `require_base` |
| `game/turn.rs:641` | `require_surface` (unchanged in meaning) |

Also re-read, though they never called `require_surface`: `nest_aggro_tick` and
`power_regen_system` both guard on `is_underground` today. `is_underground` must
stay **Stack-only** — but both now need to refuse in base space too, and for
`power_regen_system` that is load-bearing: a Recharger within radius of the
anchor would otherwise refill the party while they are out of phase.

- [ ] **Step 1: Write the failing tests.** Intent: one test per row of the table
  above asserting the action is refused in the *other* two locales and permitted
  in its own — table-driven is fine, but each row must actually exercise the real
  entry point, not a helper. Plus: `is_underground` is false in base space;
  `power_regen_system` does not restore Power in base space; `nest_aggro_tick`
  starts no fight in base space.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Add the variant, `in_base`, `base_pos`, `require_base`; narrow `require_surface`; re-read all eleven sites.**
- [ ] **Step 4: Run the tests, then the full engine suite.** `crates/engine/src/tests/stack.rs` has 67 `Locale` references and a non-exhaustive match there will now fail to compile — that is the point of using an enum.
- [ ] **Step 5: Write the `docs/seams.md` entry** for "`require_surface` means not-in-the-Stack", with this table, and add the one-or-two-line rule to `CLAUDE.md` under **The base**. Per trap: `CLAUDE.md` and `AGENTS.md` are gitignored twins — edit `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`.
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 4: The anchor

**Files:**
- Modify: `crates/engine/src/components.rs` (the `BaseAnchor` marker)
- Modify: `crates/engine/src/lib.rs` (`Game::new` spawns one)
- Modify: `crates/engine/src/game/zone.rs` (`enter_next_zone` re-places it — the full breach change is Task 8, this is only the spawn)
- Modify: `crates/engine/src/save.rs`, `crates/engine/src/game/lifecycle.rs` (persist its position)
- Test: `crates/engine/src/tests/base_space.rs`

**Interfaces:**
- Produces:
  - `components::BaseAnchor` — a marker.
  - `Game::anchor_position(&self) -> Option<(i32, i32)>`
- Consumes: `BaseGrid` (Task 1) is not needed yet; `Locale::Base` (Task 3) is not needed yet either. This task is independent of both and could run in parallel.

**Design notes that are not obvious:**
- The anchor is **not** a `Structure`. That is what keeps "every `Structure` is in
  base space" true without a marker component. Model it on `SurfaceLink`, which
  is already a non-`Structure` surface entity carrying a glyph.
- Unlike `SurfaceLink` it must **survive** `enter_next_zone`'s stale sweep, which
  today despawns `Or<(With<Hostile>, With<Nest>, With<SurfaceLink>)>`. It is not
  in that filter, so it survives by default — but it must then be *moved* to the
  new spawn point rather than left at its old coordinates.
- Persist its position in one `Option<(i32, i32)>` save field rather than deriving
  it from the zone spawn point on load. Derivation looks cheaper and is a trap:
  memory records that the zone spawn point is usually `(0,0)`, so a derivation
  bug would be invisible in most tests.
- It is indestructible: no `Durability`, so `run_raid`'s `With<Durability>` query
  cannot select it and no explicit exclusion is needed.

- [ ] **Step 1: Write the failing tests.** Intent: a new `Game` has exactly one
  `BaseAnchor` and it stands where the player starts; a save→load preserves its
  position; a forced raid never targets it; it is not counted by anything that
  counts deployed structures (`Game::structure_manifest` or whichever catalog
  call reports them — check `game/catalog.rs`).
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the engine suite.**
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 5: Walking in and out

**Files:**
- Create: `crates/engine/src/game/base_space.rs`
- Modify: `crates/engine/src/game/mod.rs` (declare it)
- Modify: `crates/engine/src/game/turn.rs` (movement dispatches on locale)
- Modify: `crates/app-core/src/app/playing.rs:242` (the key)
- Test: `crates/engine/src/tests/base_space.rs`

**Interfaces:**
- Consumes: `BaseGrid` (Task 1), `Locale::Base` + `base_pos` (Task 3), `BaseAnchor` + `anchor_position` (Task 4).
- Produces:
  - `Game::enter_base(&mut self) -> Result<(), String>` — refuses unless the player stands on the anchor and a Home exists.
  - `Game::leave_base(&mut self) -> Result<(), String>` — refuses unless standing on the base-space exit cell.
  - `Game::base_view(&self) -> Option<...>` is **not** part of this task; the renderer comes in Task 7.

**Design notes:**
- The player's surface `Position` **pins to the anchor tile** while
  `Locale::Base` is live — the Stack's trick exactly. Everything CLAUDE.md says
  about that applies unchanged: the test for whether a `Position` reader needs a
  guard is not "does it act" but "does it claim something about where the party
  is", and a read-only screen falls in the same hole.
- Movement in base space reads `BaseGrid::walkable`, never `WorldMap`. Solid is
  not walkable, so there is no "inside the rock" state to reach and no analogue of
  `die_in_the_rock` is needed.
- **Entry is refused while no Home exists.** A new run has no base at all
  (`game/base/building.rs:25` refuses every structure until a Home is deployed),
  so the anchor leads nowhere until Task 6 lays the pocket.
- Base space has its own origin. The exit cell is base-space `(0, 0)`, which is
  where the Home stands.
- Reuse the `>` / `<` keys rather than inventing new ones — they already mean
  "go in" and "go out" for the Stack, and `playing.rs` dispatches them by locale.

- [ ] **Step 1: Write the failing tests.** Intent: entering from the anchor sets
  `Locale::Base` and leaves the surface `Position` on the anchor tile; leaving
  from `(0,0)` restores `Locale::Surface` with the player still on the anchor
  tile; entering from anywhere but the anchor is refused; entering with no Home
  is refused with a distinct message; walking into a Solid cell is refused and
  costs no turn; walking onto a Floor cell moves the base-space coordinates and
  leaves the surface `Position` untouched.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Implement `base_space.rs` and the movement dispatch.**
- [ ] **Step 4: Wire the app-core key.** Add an app-core test that the key
  reaches `enter_base` — note that app-core battles are always one group and one
  slot, so anything richer belongs in the engine suite.
- [ ] **Step 5: Run `cargo test -p feral-processes-engine` and `cargo test -p feral-processes-app-core`.**
- [ ] **Step 6: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 6: The pocket, and the base moves in

**Files:**
- Modify: `crates/engine/src/game/base/building.rs` (`place_structure`'s footprint check, the deploy site that stamps today)
- Modify: `crates/engine/src/game/zone.rs:248` (`stamp_platform` → the pocket) and its `clear_platform` counterpart
- Modify: `crates/engine/src/game/base/hauling.rs`, `crates/engine/src/game/contracts.rs:592` (`broker_reach`)
- Modify: `crates/engine/src/tuning.rs` (`STARTING_POCKET_RADIUS`)
- Test: `crates/engine/src/tests/building.rs` (37 `Platform` references — expect wide churn), `crates/engine/src/tests/base_space.rs`

**Interfaces:**
- Consumes: everything from Tasks 1–5.
- Produces: `Game::lay_starting_pocket(&mut self)` — private; called from the Home deploy site, replacing `stamp_platform`.

**Design notes:**
- **One-for-one replacement.** `stamp_platform` is called when the Home is
  deployed; `lay_starting_pocket` is called from that same site. It lays a Floor
  disc of `STARTING_POCKET_RADIUS` (= today's `MAX_BUILD_DISTANCE_FROM_HOME`, 4)
  around base-space origin. It writes **no** `WorldMap` tiles.
- `place_structure`'s "is this on the slab" check becomes `BaseGrid::is_floor`.
  Everything else about deployment is unchanged.
- **`broker_reach` is a trap CLAUDE.md already flags.** `AtBroker` measures the
  *base*, via `Platform::covers`, never the distance to the Broker. It becomes
  "the player is in base space, standing on Floor". The base menu's row test must
  keep calling `broker_reach` and **not** `contract_board`, which rolls every
  template and walks the habitat ring before it can answer.
- `game/collect.rs::ORTHOGONAL` is the one reach rule both the player and the
  pull phase read. Do not add a second.

- [ ] **Step 1: Write the failing tests.** Intent: deploying a Home lays exactly
  `floor_count()` cells and writes no `Biome::Platform` tile into `WorldMap`;
  `place_structure` refuses a base-space cell that is not Floor; a machine on
  Floor deploys as it always did; `broker_reach` reports `AtBroker` on Floor,
  `OffBase` in base space off Floor, and `NoBroker` with no Broker standing;
  a fixture standing a Broker up must stand a **Home** up with it, or the pocket
  does not survive the save the test loads.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Implement.** `Platform` still exists at the end of this task and
  is simply unused for footprint decisions — deleting it is Task 7, so this task
  stays reviewable on its own.
- [ ] **Step 4: Run the engine suite.** `tests/building.rs` will need substantial
  updating; that is expected, not a signal something is wrong.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 7: Delete `Platform`, retire the Heaps, draw the new space

**Files:**
- Delete from `crates/engine/src/resources.rs`: `Platform`, `covers`, `in_shape`
- Delete from `crates/engine/src/game/zone.rs`: `stamp_platform`, `clear_platform`, `build_radius`, `claim_ground`
- Modify: `crates/engine/src/game/spawning.rs`, `crates/engine/src/game/catalog.rs`, `crates/engine/src/game/lifecycle.rs`, `crates/engine/src/game/environment.rs` (the remaining resource readers)
- Delete: `assets/structures/heap_pillar.ron`, `assets/structures/heap_block.ron`
- Modify: `assets/structures/README.md` (`build_radius_bonus` and `claims_ground` are gone)
- Modify: `crates/engine/src/views.rs` (`view_tiles` dispatches on locale)
- Modify: `crates/gui/src/render/base.rs` (two colours)
- Modify: `crates/engine/src/world.rs` (`Biome` gains two variants)

**Interfaces:**
- Produces:
  - `Biome::Entropy` — solid, unmined blackness.
  - `Biome::Excavated` — mined but not floored.
  - `view_tiles` returns synthesised base-space tiles when `Locale::Base` is live:
    Floor → `Biome::Platform`, Open → `Biome::Excavated`, absent → `Biome::Entropy`.

**Why the renderer barely changes:** `render/base.rs`'s `draw_surface_map` gets
its tiles from exactly one engine call, `game.view_tiles(hw, hh)`. Making that
call locale-aware means base space draws through the existing surface renderer,
and the gui change collapses to a palette. **Do not add a `Painter` operation
for this.**

**Also in this task:** `drawn_on_surface_map` must return nothing in base space,
or Examine will name things that are not drawn. That function is the one rule
`render/base.rs` and `Game::find_target_in_direction` both read; keep it that way.

- [ ] **Step 1: Write the failing tests.** Intent: `view_tiles` in base space
  returns `Biome::Platform` inside the pocket and `Biome::Entropy` outside it;
  `view_tiles` on the surface is unchanged; `drawn_on_surface_map` is empty in
  base space; no shipped structure asset declares `build_radius_bonus` or
  `claims_ground` (a census over the real assets, in `crates/engine/src/tests/assets.rs`
  alongside the existing ones); every `Biome` variant has a colour in the
  renderer — check whether `render/base.rs`'s colour table is already exhaustive
  and make it so if it is not, since CLAUDE.md records `cell_mark` shipping a
  `CellKind` invisible for exactly this reason.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Delete `Platform` and everything that read it. Retire the Heaps. Add the two biomes and the `view_tiles` dispatch. Add the two colours.**
- [ ] **Step 4: `cargo test --workspace`.** This is the task most likely to break
  something far away — `Biome::Platform` has fifteen non-test readers across nine
  files, and `species.rs` and `environment.rs` are among them.
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 8: The base stops travelling

**Files:**
- Modify: `crates/engine/src/game/zone.rs:482-535` (`enter_next_zone`)
- Test: `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Consumes: everything above.
- Produces: no new API. This task is a deletion.

**What goes:** the offset snapshot and rebuild at `zone.rs:499-535` — the block
that reads every `Structure`'s `Position` relative to the Home and re-lays the
base around the new spawn point. Structures are in base space now; a breach does
not touch it.

**What stays and must be verified, not assumed:** the stale-entity sweep still
despawns `Hostile`, `Nest` and `SurfaceLink`. Every zone-local resource is still
wiped **by name** — `StackMemory`, `BuybackLedger`, `PopulatedChunks`, the two
currencies. **`BaseGrid` must not be added to that list**, and that is the whole
point of this task: it inverts the rule the surrounding code teaches.

The anchor is re-placed at the new spawn point (Task 4 wired the spawn; this task
confirms it under a real breach).

- [ ] **Step 1: Write the failing tests.** Intent: across `enter_next_zone`, the
  `BaseGrid` is byte-identical and every structure keeps its base-space
  `Position`; the anchor moves to the new zone's spawn point; `StackMemory` and
  `BuybackLedger` are still wiped; Cache Grain still crosses the breach while the
  two currencies do not. Note the trap in memory: the zone spawn point is usually
  `(0,0)`, so asserting on it across a breach is vacuous — compare the wild
  population or the map seed instead.
- [ ] **Step 2: Run and watch them fail.**
- [ ] **Step 3: Delete the rebuild block.**
- [ ] **Step 4: `cargo test --workspace`.**
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

### Task 9: Templates, docs, and the gate

**Files:**
- Regenerate: `dev-saves/chains.ron`, `contracts.ron`, `deep-lair.ron`, `extraction.ron`, `rarity-preview.ron`, `stack.ron`
- Modify: `dev-saves/README.md` if any template's description no longer matches
- Modify: `docs/seams.md` and `CLAUDE.md` (+ `cp CLAUDE.md AGENTS.md`)
- Modify: `CHANGELOG.md` — **add the entry, do not bump the version**

**Interfaces:** none.

All six templates were captured at `SAVE_FORMAT_VERSION` 31 and will be refused
by version. Recapture, do not migrate. `dev-saves/README.md` says what each one
sets up; reproduce that state and `cargo run --bin savetool -- capture` it.

Seams to write, each under the same title in both files:

1. **`require_surface` means "not in the Stack".** The eleven-site table from
   Task 3, and why a wrong answer is silent.
2. **`BaseGrid` is the one base resource that is not zone-local.** It inverts the
   wipe-by-name rule its four neighbours follow.
3. **The base's footprint is `BaseGrid::is_floor` and nothing else.** Replaces the
   existing "The base's radius is derived, never stored" entry, which is now
   describing deleted code — delete it rather than leaving it.
4. **`Structure` is the space tag.** One spawn site, so no marker component; a
   second spawn site would silently break it.

Also correct, in `CLAUDE.md` under **The base**: the Heap Pillar/Heap Block entry
("The base grows on two axes, and only one is derivable"), the build-radius
covering term, and the slab-eats-the-on-ramp entry — all three describe code this
slice deletes. A rule with its subject deleted is worse than no rule.

- [ ] **Step 1: Recapture all six templates and confirm each loads.**
- [ ] **Step 2: Write the four seam entries in `docs/seams.md`, the short forms in `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`.**
- [ ] **Step 3: Delete the three now-false `CLAUDE.md` entries and their `docs/seams.md` arguments.**
- [ ] **Step 4: Add the `CHANGELOG.md` entry under the existing unreleased heading. Do not bump `Cargo.toml`.**
- [ ] **Step 5: `cargo fmt`, `cargo clippy --workspace`, `cargo test --workspace`.** This is the slice's final gate.
- [ ] **Step 6: Commit.**

---

## Playtest before calling slice 1 done

A green suite is not evidence of play, and this slice's entire claim is that
**nothing about play changed**. That claim is only testable by playing.

```sh
cargo run                       # a fresh run: build a Home, walk in, walk out
cargo run -- --template chains  # a running base, in base space
cargo run -- --template stack   # the Stack on-ramp still works beside the anchor
```

What to look for: the anchor is findable and readable on the surface map; base
space draws legibly and the pocket's edge is obvious; the base menu, work orders,
trade and the Broker all behave as they did; breaching leaves the base untouched.

## Not in this slice

- Mining, tiling and entropy (slice 2). `BaseGrid::open` exists and is tested but
  nothing calls it in gameplay.
- The dead `Biome::Platform` clauses in `Tile::open_to_hostiles` and
  `pursuit_field`, and `spawn_surface_links`' slab logic (slice 3). They compile
  and are unreachable once nothing writes `Biome::Platform` into `WorldMap`.
- Portals to older zones, and postable mining. Separate specs.
