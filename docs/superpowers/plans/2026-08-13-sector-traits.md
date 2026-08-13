# Sector Traits Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make each zone a visibly and mechanically different sector — its own
biome mix, roster and colour bands — driven entirely by data, at no
save-format cost.

**Architecture:** A `SectorDef` in `assets/sectors/*.ron` carries threshold
deltas for `WorldMap::classify` and two hues. Which def a zone gets is derived
from `(world seed, zone)` rather than stored. The biome mix shift does the
mechanical work (roster and buildable ground fall out of it, because
`habitat_pools` filters by tile biome and `Biome::walkable` gates placement);
the two hues do the visual work, applied in the renderer as an HSV hue swap
over the existing colour table so brightness relationships survive untouched.

**Tech Stack:** Rust, `bevy_ecs` (engine), `bevy` + `bevy_egui` (gui), `ron`
for assets, `noise::Perlin` for generation.

**Spec:** `docs/superpowers/specs/2026-08-13-sector-traits-design.md` — read it
first. This plan argues from it and does not restate its reasoning.

## Global Constraints

- **`CLAUDE.md` is the governing document.** Read it before Task 1. Every rule
  in it applies here, in particular Moddability, Code principles, Rust idioms
  and Testing.
- **No `SAVE_FORMAT_VERSION` bump.** The trait is derived from values already
  saved. If you find yourself adding a save field, stop and re-read the spec's
  "Which trait a zone gets".
- **Never draw from `resources::GameRng`** for anything in this feature. World
  generation must not; see `CLAUDE.md`'s entry on it.
- **`assets/sectors/` is a real content directory.** Malformed files are
  skipped with a logged warning, never a panic. Deleting the directory must
  restore today's game exactly.
- **The drawing seam holds.** `crates/gui/src/paint.rs` stays the only file
  naming a graphics library. Colour stays in `crates/gui`; the engine ships
  only the two authored hue numbers.
- **`biome_tint` stays exhaustive.** A new `Biome` must still fail to compile
  until someone decides which side of the walkability rule it falls on.
- **Biome textures are not touched.** `draw_traces`, `draw_dot`,
  `draw_broken_grid`, `draw_speckle`, `draw_slab`, `draw_depth` and
  `draw_shards` are each biome's identity — closer to a glyph than to
  decoration. Varying them per sector means relearning the map every breach.
- **Mid-run chunk drift is accepted, not a bug to fix.** An existing save's
  *unexplored* chunks regenerate under the new thresholds, so a map can change
  shape at a boundary the player has not walked to. Walked ground and
  everything stamped are unaffected (`WorldMap` caches generated chunks and
  saves its `overrides` overlay). The spec weighs this against a saved field
  and takes the drift. Do not add a field to avoid it.
- **TDD.** Failing test first, every task. `cargo fmt` and
  `cargo clippy --workspace` after every change.
- **Commits are free on this branch; pushing, tagging and version bumps are
  not.** Ask. The version bump happens once, at the merge.
- **Branch:** `sector-traits`, already created, spec already committed.

---

### Task 1: Thresholds become data

`WorldMap::classify` currently hardcodes five thresholds. Make them a value.

**Files:**
- Modify: `crates/engine/src/world.rs` — `classify` (~126-152), `WorldMap`
  struct (~90-112), and its `impl`.

**Interfaces:**
- Produces: `world::SectorShape`, a `Copy` struct with one field per
  `classify` threshold — the elevation floor (`DataVoid`), the elevation
  ceiling (`BlackIce`), the temperature floor (`StaticField`), and the
  temperature/moisture pair (`NullSector`) and moisture floor (`Mainframe`).
  `SectorShape::NEUTRAL` holds today's literals. `WorldMap::with_shape(seed,
  shape) -> WorldMap`; `WorldMap::new(seed)` keeps its signature and delegates
  with `NEUTRAL`. `WorldMap::shape(&self) -> SectorShape`.

- [ ] **Step 1: Write the failing tests**

Two, both in `world.rs`'s test module:
- A neutral shape produces byte-identical tiles to today across a sampled
  region. Capture the expected tiles from the current implementation *before*
  changing it, so this is a real pin rather than a tautology.
- A shape with a raised `StaticField` temperature floor yields strictly more
  `StaticField` tiles over the same region and seed than neutral does. This is
  the one that proves the knob is connected.

- [ ] **Step 2: Run them and watch them fail to compile** (`SectorShape`
      does not exist). Run: `cargo test -p feral-processes-engine world`

- [ ] **Step 3: Implement `SectorShape`, `NEUTRAL`, `with_shape`, and thread
      it through `classify`**

`WorldMap::new` must keep working unchanged — it has 13 call sites, almost all
tests, and churning them is noise that hides the real diff.

- [ ] **Step 4: Run the tests, then the whole engine suite**

Run: `cargo test -p feral-processes-engine`
The byte-identical test is the gate: generation must not have moved.

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit**

---

### Task 2: `SectorDef`, `SectorDb`, and the shipped sectors

**Files:**
- Create: `crates/engine/src/sectors.rs`
- Create: `assets/sectors/README.md`
- Create: at least two `assets/sectors/*.ron`
- Modify: `crates/engine/src/lib.rs` (module declaration)
- Modify: `crates/engine/src/game/lifecycle.rs` (~1181, ~1224) — load and
  register the db beside `AffixDb`
- Modify: `crates/engine/src/tuning.rs` — the walkable floor constant

**Interfaces:**
- Consumes: `world::SectorShape` from Task 1.
- Produces: `SectorDef { id, name, description, shape, palette }` where
  `shape` is a per-threshold delta applied to `NEUTRAL` and `palette` is two
  hues in degrees (ground, hazard). `SectorDef::shape() -> SectorShape`
  resolves the deltas. `SectorDb::load_dir(dir) -> io::Result<(Self,
  Vec<String>)>` returning warnings, exactly like `AffixDb::load_dir`
  (`crates/engine/src/affixes.rs:138`) — copy that shape, including how
  warnings are surfaced.

- [ ] **Step 1: Write the failing tests**

- A well-formed `.ron` loads and resolves its deltas onto `NEUTRAL`.
- A malformed `.ron` is skipped with a warning, and the other files in the
  directory still load. **No panic.**
- A ground hue outside the cool band is refused with a warning; likewise a
  hazard hue outside the warm band.
- A shape leaving less walkable ground than `tuning::MIN_SECTOR_WALKABLE_
  FRACTION` is refused with a warning. Sample over a fixed region and seed.
- An absent directory loads to an empty db with no error.
- **A census over the real `assets/sectors/`**: every shipped sector passes
  both validations.

> The census is the one that can go vacuous. A `for def in db.all()` loop over
> an empty directory passes while asserting nothing, which reads as coverage
> and is not. Assert a non-zero count first, and after implementing, delete a
> shipped `.ron` and confirm the census notices.

- [ ] **Step 2: Run and watch them fail.**
      Run: `cargo test -p feral-processes-engine sector`

- [ ] **Step 3: Implement `sectors.rs`, the two assets, and the README**

The walkable-floor check needs Task 1's `SectorShape` to build a throwaway
`WorldMap::with_shape` and sample it. Put the floor in `tuning.rs` under a
labelled section — it is a playability bound, not content, which is the same
argument `tuning.rs`'s header makes.

Author the two sectors so they are genuinely different from neutral and from
each other — a cold, Static-Field-dominant sector and a fractured one heavy on
`DataVoid` are a good pair, because they exercise opposite ends of the
walkable floor. `assets/sectors/README.md` documents the full schema and is
part of this task, not a follow-up: `CLAUDE.md` requires the schema doc to
land in the same change.

- [ ] **Step 4: Run the tests, then delete one shipped `.ron` and confirm the
      census fails.** Restore it. Then `cargo test -p feral-processes-engine`.

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit**

---

### Task 3: Which sector a zone gets

**Files:**
- Modify: `crates/engine/src/sectors.rs` — the derivation
- Test: `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Consumes: `SectorDb` from Task 2.
- Produces: a free function in `sectors.rs` taking the world seed, the zone
  number and the db, returning `Option<&SectorDef>` — `None` meaning neutral.
  It is the **only** place a sector is chosen; nothing else derives one.

- [ ] **Step 1: Write the failing tests**

- Zone 1 is always neutral, for many seeds. This is the opening-ring
  protection and it is not optional.
- The same `(seed, zone)` returns the same sector across repeated calls.
- Different zones off one seed do not all return the same sector (the
  anti-correlation trap `descriptions.rs` hit — see `CLAUDE.md`'s entry and
  the `description-selection-reads-high-bits` memory: reducing with `%` reads
  only the bits a multiply never disturbs).
- An empty db returns neutral for every zone.

- [ ] **Step 2: Run and watch them fail.**
      Run: `cargo test -p feral-processes-engine sector`

- [ ] **Step 3: Implement the derivation**

Follow `descriptions.rs`'s scheme rather than inventing one: an FNV-style fold
of seed and zone, reduced with Lemire's `(seed as u128 * len) >> 64`. **Not**
`% len` — the spec and `CLAUDE.md` both record why, and it is not a style
preference.

- [ ] **Step 4: Run the tests.** Run: `cargo test -p feral-processes-engine`

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit**

---

### Task 4: Wire the shape into generation

**Files:**
- Modify: `crates/engine/src/game/lifecycle.rs:85` (`Game::new`) and `:237`
  (`Game::load`)
- Modify: `crates/engine/src/game/zone.rs:507` (`enter_next_zone`)
- Test: `crates/engine/src/tests/zone.rs`

**Interfaces:**
- Consumes: the derivation from Task 3, `WorldMap::with_shape` from Task 1.

- [ ] **Step 1: Write the failing tests**

- **Save/load determinism**: generate a world, save, load, and assert the map
  is identical — through a real save file round trip, not a recomputation in
  the same process. `Game::load` reconstructing with a different shape would
  regenerate unwalked chunks differently and strand a party in rock; this is
  the same class of bug the Stack-frame RNG rule exists to prevent.
- Breaching to a zone with a Static-Field-heavy sector shifts the wild roster
  toward that biome's species. Do **not** assert on the spawn point — it is
  usually `(0,0)` and the assertion would be vacuous (see the
  `zone-spawn-point-is-usually-origin` memory); compare the population's
  species mix instead.
- Zone 1 generation is byte-identical to before this feature.
- **Absence is supported**: with `assets/sectors/` empty, generation at every
  zone is byte-identical to neutral. This is the same property affixes and the
  enemy policy have, and an omission is invisible without a test — see
  `CLAUDE.md` on `roll_affix` spending no RNG draw against an empty pool.

- [ ] **Step 2: Run and watch them fail.**
      Run: `cargo test -p feral-processes-engine zone`

- [ ] **Step 3: Wire all three construction sites**

All three must derive through Task 3's one function. A fourth site that builds
a `WorldMap` for real play goes through it too, never beside it.

- [ ] **Step 4: Run the balance gate and the suite**

Run: `cargo test -p feral-processes-engine balance_sim`
Then: `cargo test -p feral-processes-engine`

The roster mix moves with the biome mix, so `balance_sim` is exactly the right
instrument here. **A moved curve means progression changed — that is the
signal, not a broken test.** If a curve moves, stop and report the numbers
rather than adjusting the expectation.

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit**

---

### Task 5: The engine hands over two hues

**Files:**
- Modify: `crates/engine/src/game/inspection.rs` — beside `view_tiles` (~24)
- Test: `crates/engine/src/tests/inspection.rs`

**Interfaces:**
- Produces: `Game::sector_hues(&self) -> (f32, f32)` — the current sector's
  ground and hazard hues in degrees, falling back to the neutral pair when the
  sector is neutral or the db is empty. Task 6 consumes it by this name; keep
  them in step. The renderer already calls `game.view_tiles`
  and `game.zone_spawn_point` directly, so this needs no view-struct change
  and no app-core plumbing.

- [ ] **Step 1: Write the failing test** — zone 1 reports the neutral pair; a
      zone on a sector with authored hues reports that sector's.

- [ ] **Step 2: Run and watch it fail.**
      Run: `cargo test -p feral-processes-engine inspection`

- [ ] **Step 3: Implement the accessor.** Two floats. The engine ships no
      `Color` and no table — colour belongs to `crates/gui`.

- [ ] **Step 4: Run.** `cargo test -p feral-processes-engine`

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit**

---

### Task 6: The renderer applies the hues

**Files:**
- Modify: `crates/gui/src/render/base.rs` — `biome_tint` (~89-117), its call
  site (~598), and the test module (~1111-1131)

**Interfaces:**
- Consumes: `Game::sector_hues` from Task 5.
- Produces: `biome_tint` gains a hue-pair parameter and keeps its exhaustive
  match as the reference table.

- [ ] **Step 1: Write the failing tests**

- **The generalised rule.** Today `every_biomes_tint_says_whether_it_can_be_
  walked_on` sweeps `ALL_BIOMES` against `reads_as_hostile`. It becomes a
  sweep over *every hue pair in `assets/sectors/`* × `ALL_BIOMES`. This is the
  gate the whole palette design exists to satisfy, and the reason the
  transform is a band swap rather than a free palette.
- **Brightness relationships survive.** For any hue pair, the value ordering
  of the five walkable biomes is unchanged from neutral — Platform stays much
  the darkest. That property is what keeps a base screen readable, and it was
  arrived at twice by looking at the screen, so it deserves a pin.
- **Neutral hues reproduce today's table exactly.** Not approximately: assert
  equality against the current literals, or a rounding bug in the HSV round
  trip silently reskins the default game.

- [ ] **Step 2: Run and watch them fail.**
      Run: `cargo test -p feral-processes-gui biome`

- [ ] **Step 3: Implement the transform**

RGB → HSV, replace H with the ground hue when `biome.walkable()` and the
hazard hue otherwise, keep S and V, HSV → RGB. This is the one genuinely
non-obvious piece and the one place a code block is warranted:

```rust
// Keep S and V, replace H. The saturation/value spread is what separates
// the five walkable biomes from each other; hue is what separates walkable
// from not. Swapping only H moves the bands without disturbing either.
let (_, s, v) = rgb_to_hsv(base);
let h = if biome.walkable() { ground_hue } else { hazard_hue };
hsv_to_rgb(h, s, v)
```

Watch the round trip on the two hot colours: they have high S and V, and a
naive conversion that clamps or loses precision will move them enough to fail
the exact-equality test in Step 1.

- [ ] **Step 4: Run.** `cargo test -p feral-processes-gui`, then
      `cargo test --workspace`

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit**

---

### Task 7: Announce the sector, and document it

**Files:**
- Modify: `crates/engine/src/game/zone.rs` (~588, the breach log line)
- Modify: `CHANGELOG.md`
- Test: `crates/engine/src/tests/zone.rs`

- [ ] **Step 1: Write the failing test** — breaching into a named sector logs
      its name and description; breaching into a neutral one logs today's line
      unchanged.

- [ ] **Step 2: Run and watch it fail.**
      Run: `cargo test -p feral-processes-engine zone`

- [ ] **Step 3: Implement, and write the changelog entry**

New `## X.Y.Z` section. Which digit moves is decided by `CHANGELOG.md`'s own
preamble — read it, don't guess; "breaking" there means a save stops loading,
which this does not do. Do **not** bump the workspace version or tag: that
happens once, at the merge.

`docs/manual.md` and the root `README.md` are both carved out of the doc
obligation — leave them alone. `assets/sectors/README.md` landed in Task 2.

- [ ] **Step 4: Full gate.** `cargo test --workspace`

- [ ] **Step 5: `cargo fmt && cargo clippy --workspace`, then commit**

---

## Verification beyond the suite

A green suite is not evidence this looks right, and the visual half of this
feature has no automated judge. Before calling it done:

```sh
cargo run -- --template stack        # or any template; breach a few zones
```

Breach until you land in each shipped sector and confirm on screen that the
biomes remain distinguishable from one another, that the base slab still reads
as much the darkest thing under a full base, and that hazard ground still
reads as hazard at a glance. `dev-saves/README.md` lists what each template
sets up; `savetool warp` reaches a deep zone without playing to it.

Report what you actually saw. If it needs a hue adjusted, that is a data edit
in `assets/sectors/`, not a code change.
