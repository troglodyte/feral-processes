# The Stack, Phase 1 — Rename Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Rename the dungeon layer to the Stack — frames, links, slices —
changing no behaviour and no encoded save byte.

**Architecture:** Seven commits, split **by identifier group rather than by
crate**. A rename is only compilable when an identifier changes everywhere at
once, so each task crosses whatever crates it needs to and leaves the tree
green. Ordering is load-bearing in exactly one place, called out in Task 1.

**Tech Stack:** Rust, 4-crate Cargo workspace (`engine`, `app-core`, `gui`,
`launcher`), `bevy_ecs` in the engine, `bevy` + `bevy_egui` in gui.

Spec: `docs/superpowers/specs/2026-07-31-the-stack-design.md`

## Global Constraints

- **Behaviour must not change.** This phase is inert — no field, variant, or
  behaviour is added. `cargo test --workspace` must pass throughout, and the
  test count must never *fall* below the baseline. Confirm the baseline
  before Task 1 and record it in the ledger. Exactly three tests are added,
  all guards on the rename itself: one in Task 3 (the descend log wording)
  and two in Task 4 (binary save round-trip, RON template parse). Every
  other task holds the count exactly.
- **Do not bump `SAVE_FORMAT_VERSION`.** The on-disk save is bincode with
  `bincode::config::standard()`, which is positional — no field names, enum
  variants by index (`crates/engine/src/save.rs:201-206`). A rename moves no
  encoded byte.
- **Do not reorder any enum variant or struct field.** Variant *order* is the
  save format. Renaming is safe; reordering is not.
- **`dev-saves/*.ron` are RON, and RON deserializes by name.** Any `SaveData`
  field rename must update `dev-saves/extraction.ron` in the same commit or
  `--template extraction` breaks at load. This is not covered by the existing
  suite — the three `dev_template` tests in the launcher do not load it.
- `cargo fmt` and `cargo clippy --workspace` clean after every task. Fix
  warnings, don't silence them.
- Comments explain *why*. A rename that leaves a comment restating the new
  name is noise — delete rather than update those.
- CLAUDE.md and AGENTS.md are gitignored twins of the same document. Edit
  CLAUDE.md, then `cp CLAUDE.md AGENTS.md`.

---

## The mapping

Every rename in this phase. Later tasks reference this table by row rather
than repeating it.

### Types and variants

| now | becomes |
| --- | --- |
| `Locale::Dungeon` | `Locale::Stack` |
| `Locale::Stack { floors }` (field) | `Locale::Stack { frames }` |
| `DungeonMemory` | `StackMemory` |
| `LevelMemory` | `FrameMemory` |
| `LevelSpec` | `FrameSpec` |
| `DungeonLevel` | `Frame` |
| `DungeonPos` | `StackPos` |
| `DungeonView` | `StackView` |
| `DungeonCellView` | `StackCellView` |
| `DungeonMapView` | `FrameMapView` |
| `DungeonMapCell` | `FrameMapCell` |
| `DungeonMapMark` | `FrameMapMark` |
| `DungeonEntrance` | `SurfaceLink` |
| `DungeonSpawn` | `StackSpawn` |
| `CurrentDungeon` | `CurrentStack` |
| `CellKind::StairsUp` / `StairsDown` | `CellKind::LinkUp` / `LinkDown` |
| `FrameMapCell::StairsUp` / `StairsDown` | `FrameMapCell::LinkUp` / `LinkDown` |
| `StackCellView::StairsUp` / `StairsDown` | `StackCellView::LinkUp` / `LinkDown` |
| `Mode::DungeonMap` | `Mode::FrameMap` |

`Dir`, `CellKind::{Rock, Floor, Cache, Lair, Door, SealedDoor}` and
`Biome` are unchanged. `lair` keeps its name throughout — deliberate, per
the spec.

### Functions and fields

| now | becomes |
| --- | --- |
| `Game::dungeon_view` | `Game::stack_view` |
| `Game::dungeon_map` | `Game::frame_map` |
| `Game::dungeon_pos` | `Game::stack_pos` |
| `Game::enter_dungeon` | `Game::enter_stack` |
| `Game::clear_dungeon` | `Game::clear_stack` |
| `Game::dungeon_depth_multiplier` | `Game::stack_depth_multiplier` |
| `Game::find_dungeon_entrance_at` | `Game::find_surface_link_at` |
| `Game::spawn_dungeon_entrances` | `Game::spawn_surface_links` |
| `Game::restore_dungeon_entrances` | `Game::restore_surface_links` |
| `Game::level_spec` | `Game::frame_spec` |
| `Game::level_memory_mut` | `Game::frame_memory_mut` |
| `Game::breach_floors_at` | `Game::frames_at` |
| `breach_floors` (free fn) | `frames_for` |
| `DungeonLevel::stairs_down` (field) | `Frame::link_down` |
| `Game::stairs_available` | `Game::links_available` |
| `stair_mark` (`render/dungeon.rs`) | `link_mark` |
| `stand_on_stairs_down` (test helper) | `stand_on_link_down` |
| `SaveData::dungeon_entrances` | `SaveData::link_sites` |
| `SaveData::dungeon_memory` | `SaveData::stack_memory` |
| `App::handle_dungeon_map_key` | `App::handle_frame_map_key` |
| `draw_dungeon` | `draw_stack` |
| `draw_dungeon_map` | `draw_frame_map` |

`Game::is_underground` and `Game::require_surface` keep their names. Both
read correctly against the new vocabulary and both are named in CLAUDE.md's
load-bearing-seams section.

### Tuning constants

All `DUNGEON_*` become `STACK_*`, with three that also change their noun
because "level" and "floor" both meant *frame*:

| now | becomes |
| --- | --- |
| `DUNGEON_CACHES_PER_LEVEL` | `STACK_CACHES_PER_FRAME` |
| `DUNGEON_DOORS_PER_LEVEL` | `STACK_DOORS_PER_FRAME` |
| `DUNGEON_FLOORS_MIN` / `_MAX` | `STACK_FRAMES_MIN` / `_MAX` |
| `DUNGEON_TILES_PER_FLOOR` | `STACK_TILES_PER_FRAME` |
| `DUNGEON_ENTRANCES_PER_ZONE` | `STACK_LINKS_PER_ZONE` |
| `DUNGEON_ENTRANCE_SCATTER_TILES` | `STACK_LINK_SCATTER_TILES` |
| `DUNGEON_NEAREST_ENTRANCE_TILES` | `STACK_NEAREST_LINK_TILES` |
| `DUNGEON_MIN_ENTRANCE_TILES` | `STACK_MIN_LINK_TILES` |
| `DUNGEON_CACHE_CREDITS` | `STACK_CACHE_CREDITS` |
| `DUNGEON_CACHE_DEPTH_GROWTH` | `STACK_CACHE_DEPTH_GROWTH` |
| `DUNGEON_CACHE_FRAGMENT_CHANCE` | `STACK_CACHE_FRAGMENT_CHANCE` |
| `DUNGEON_DEPTH_STAT_GROWTH` | `STACK_DEPTH_STAT_GROWTH` |
| `DUNGEON_ENCOUNTER_CHANCE` | `STACK_ENCOUNTER_CHANCE` |
| `DUNGEON_VIEW_DEPTH` | `STACK_VIEW_DEPTH` |
| `DUNGEON_VIEW_HALF_WIDTH` | `STACK_VIEW_HALF_WIDTH` |

The `depth` field and `depth` parameters keep their name — "frame 3 of 5"
reads `depth: 3, frames: 5`.

### Files

| now | becomes |
| --- | --- |
| `crates/engine/src/dungeon.rs` | `crates/engine/src/stack.rs` |
| `crates/engine/src/game/dungeon.rs` | `crates/engine/src/game/stack.rs` |
| `crates/engine/src/game/dungeon_features.rs` | `crates/engine/src/game/stack_features.rs` |
| `crates/engine/src/game/dungeon_view.rs` | `crates/engine/src/game/stack_view.rs` |
| `crates/engine/src/tests/dungeon.rs` | `crates/engine/src/tests/stack.rs` |
| `crates/app-core/src/tests/dungeon.rs` | `crates/app-core/src/tests/stack.rs` |
| `crates/gui/src/render/dungeon.rs` | `crates/gui/src/render/stack.rs` |
| `crates/gui/src/render/dungeon_map.rs` | `crates/gui/src/render/frame_map.rs` |

---

## Task 1: Free the word "frame" in the renderer

**Do this first.** `crates/gui/src/render/dungeon.rs` currently uses **frame**
to mean a corridor cross-section one cell away — `fn frame`, the module doc,
and the `SHRINK` comment. The rest of this plan makes "frame" mean a Stack
level. If any later task lands before this one, the same crate has two
meanings for the word and the confusion is permanent.

**Files:**
- Modify: `crates/gui/src/render/dungeon.rs` (module doc, `fn frame`, its
  two call sites in `draw_dungeon`, `SHRINK` and `CORRIDOR_HEIGHT` doc
  comments)

**Interfaces:**
- Consumes: nothing.
- Produces: `fn slice(depth: usize, w: f32, h: f32) -> (f32, f32, f32, f32)`,
  private, replacing `fn frame` with an identical signature and body.

- [ ] **Step 1: Rename `frame` to `slice`**

Private to the file, so no other crate is affected. Update the module doc's
description of the projection — it currently says "a stack of nested
frames", which after this phase reads as a claim about Stack levels and is
wrong twice over. "A stack of nested slices" is also wrong; say what it is:
successive cross-sections of the corridor.

- [ ] **Step 2: Verify**

Run: `cargo test -p feral-processes-gui`
Expected: PASS, count unchanged.

- [ ] **Step 3: Commit**

`refactor(gui): the corridor's cross-sections are slices, not frames`

---

## Task 2: `CellKind::StairsUp`/`StairsDown` → `LinkUp`/`LinkDown`

**Files:**
- Modify: `crates/engine/src/dungeon.rs`, `crates/engine/src/views.rs`,
  `crates/engine/src/game/dungeon.rs`,
  `crates/engine/src/game/dungeon_view.rs`,
  `crates/engine/src/tests/dungeon.rs`,
  `crates/gui/src/render/dungeon.rs`,
  `crates/gui/src/render/dungeon_map.rs`

**Interfaces:**
- Consumes: nothing.
- Produces: `CellKind::LinkUp`, `CellKind::LinkDown`, `FrameMapCell::LinkUp`,
  `FrameMapCell::LinkDown`. `FrameMapCell` is still named `DungeonMapCell`
  after this task — it is renamed in Task 4.

**Test intent:** `render/dungeon_map.rs` already has
`stairs_carry_the_same_glyphs_as_the_first_person_view`, which asserts
`>` and `<`. Rename it to `links_carry_the_same_glyphs_as_the_first_person_view`
and keep both assertions — the glyphs do **not** change, which is the point:
the surface entrance and the down-link were already both `>`, so this rename
makes the code agree with what was always drawn.

- [ ] **Step 1: Rename both variants in both enums, everywhere**
- [ ] **Step 2: Rename the glyph test as above**
- [ ] **Step 3: Run `cargo test --workspace`** — PASS, count unchanged
- [ ] **Step 4: Commit** — `refactor(engine): stairs are links`

---

## Task 3: The vocabulary of moving between frames

Everything about vertical movement: `floors` → `frames`, the remaining
`stairs` identifiers and prose, and all four player-visible strings that
name either.

**Scope note (added after Task 2).** Task 2 renamed the three *enum
variants* to `LinkUp`/`LinkDown`, which left the surrounding vocabulary
inconsistent — fields, methods, helpers, test names and player-facing
strings still say "stairs" while the variants they exercise say "link".
The original plan missed this entirely and claimed three player-visible
strings existed where there are five. Task 3 owns the whole sweep so no
half-renamed vocabulary survives the phase.

**Files:**
- Modify: `crates/engine/src/resources.rs` (the `Locale::Dungeon` variant's
  `floors` field — **rename only, do not move it**),
  `crates/engine/src/game/dungeon.rs` (`breach_floors`, `breach_floors_at`,
  `stairs_available`, the descend/ascend log lines around 523 and 551, and
  the three "stairs" comments around 273, 510 and 546),
  `crates/engine/src/dungeon.rs` (`DungeonLevel::stairs_down` and its
  comments), `crates/engine/src/game/dungeon_view.rs` (**the two
  player-visible prompts at 213 and 215**),
  `crates/engine/src/views.rs` (`DungeonView::floors`,
  `DungeonMapView::floors`, the "standing_on" doc at 377), `crates/engine/src/tuning.rs`
  (`DUNGEON_FLOORS_MIN`/`MAX`, `DUNGEON_TILES_PER_FLOOR` — see the tuning
  table, plus the two "stairs" prose comments at 497 and 509),
  `crates/gui/src/render/dungeon_map.rs` (the heading string, the
  `cell_glyph` doc), `crates/gui/src/render/dungeon.rs` (`stair_mark`),
  `crates/engine/src/tests/dungeon.rs` and
  `crates/app-core/src/tests/dungeon.rs` (the helper, and the test names
  Task 2's review flagged as left mid-rename)

Take the four constants this task touches all the way to their final
`STACK_*` names rather than renaming only their noun — `tuning.rs` will
carry a mix of `STACK_*` and `DUNGEON_*` prefixes until Task 5, which is
expected and compiles fine. Renaming any identifier twice across two tasks
is the thing to avoid.

**Interfaces:**
- Consumes: nothing.
- Produces: `Locale::Dungeon { frames, .. }` (the variant itself is still
  `Dungeon` until Task 4), `Game::frames_at(tile) -> u32`
  (was `breach_floors_at`), free fn `frames_for(tile, spawn) -> u32` (was
  `breach_floors`), `Game::links_available() -> (bool, bool)` (was
  `stairs_available`), `DungeonLevel::link_down` (was `stairs_down`),
  `link_mark` (was `stair_mark`).

**The four player-visible strings.** Two log lines in `game/dungeon.rs`:
"You descend to dungeon level {} of {}." and "You climb back to dungeon
level {}." Two standing-on prompts in `game/dungeon_view.rs:213,215`:
"Stairs lead down  [>] descend" and "Stairs lead up  [<] climb". All four
become frame-and-link wording. Keep the `[>]` / `[<]` key hints exactly as
they are — those name keyboard keys, not glyphs, and the keys do not change.

**Test intent:** one new test only. Assert the descend log line names a
frame — e.g. contains `"frame 2 of"` after one `descend`. Use the fixtures
in `crates/engine/src/tests/support.rs`; check what is there before writing
a new one. The prompt strings are already covered by
`crates/app-core/src/tests/dungeon.rs` (`the_view_names_the_key_that_takes_the_stairs`),
which asserts on the key hint rather than the wording — update its name, and
check whether it asserts on wording that your change breaks.

- [ ] **Step 1: Write the failing test** for the descend log line's wording
- [ ] **Step 2: Run it** — FAIL on the old wording
- [ ] **Step 3: Rename `floors` → `frames`** across the field, the two free
      functions, the view structs and the four tuning constants
- [ ] **Step 4: Rename the remaining `stairs` identifiers** — the field, the
      method, `stair_mark`, the test helper, and the test names
- [ ] **Step 5: Reword all four player-visible strings**, and sweep the
      "stairs" prose comments in the files listed above
- [ ] **Step 6: Run `cargo test --workspace`** — PASS, count 1057 (+1)
- [ ] **Step 7: Verify** `rg -i 'stairs|floors' --type rust -g '!target' .`
      returns nothing outside genuinely unrelated prose, and report anything
      you deliberately left
- [ ] **Step 8: Commit** — `refactor(engine): frames and links, not floors and stairs`

---

## Task 4: `Dungeon*` types → `Stack*` / `Frame*`, and the save fields

The big one, and the one with the trap.

**Files:**
- Modify: every file in the "Types and variants" and "Functions and fields"
  tables above — `crates/engine/src/{resources,views,save,components,lib}.rs`,
  `crates/engine/src/game/{dungeon,dungeon_view,dungeon_features,lifecycle,zone,spawning,inspection,combat_teardown,mod}.rs`,
  `crates/app-core/src/{lib.rs,app/input.rs,app/playing.rs,app/menus.rs}`,
  `crates/gui/src/render/{mod,dungeon,dungeon_map,base,popup}.rs`,
  `crates/gui/src/paint.rs`, and both test modules
- **Modify: `dev-saves/extraction.ron`** — the `dungeon_entrances:` and
  `dungeon_memory:` keys

**Interfaces:**
- Consumes: Task 2's `CellKind::LinkUp`/`LinkDown`, Task 3's `frames` field.
- Produces: every name in the two tables above. `crates/engine/src/lib.rs`
  re-exports `SurfaceLink`, `StackSpawn`, `StackMemory`, `CurrentStack`,
  `StackView`, `StackCellView`, `FrameMapView`, `FrameMapCell`,
  `FrameMapMark` — gui imports these by name from the engine, so the
  re-export list at `lib.rs:42` and `lib.rs:56` must be updated in step with
  the definitions.

**The trap:** `SaveData::dungeon_entrances` and `SaveData::dungeon_memory`
appear **by name** in `dev-saves/extraction.ron`. The binary save is bincode
and positional, so it is unaffected — but RON is self-describing and the
template deserializes by field name. Renaming without updating the template
leaves `cargo run -- --template extraction` failing at load, and **no test
covers it**: the launcher's three `dev_template` tests do not load
`extraction.ron`.

**Test intent:** two tests, both new.

1. A round-trip test that an existing binary save still loads — the claim the
   no-version-bump decision rests on. Write a save with the pre-rename code
   path if one is not already committed as a fixture; otherwise assert
   `SAVE_FORMAT_VERSION` is unchanged at 15 and that a save written and read
   back preserves a `Locale::Stack` position.
2. A test that `dev-saves/extraction.ron` actually parses into `SaveData`.
   This gap is why the trap exists; close it rather than just avoiding it
   this once.

- [ ] **Step 1: Write the failing template-parse test** (`extraction.ron`
      deserializes into `SaveData`). It passes before the rename — run it
      first to confirm it is wired up, then it is the guard for step 3.
- [ ] **Step 2: Write the save round-trip test** for `Locale::Stack`
- [ ] **Step 3: Rename every type, field and function in the two tables**,
      updating `lib.rs`'s re-exports in the same pass
- [ ] **Step 4: Update `dev-saves/extraction.ron`'s two keys**
- [ ] **Step 5: Run `cargo test --workspace`** — PASS, count +2
- [ ] **Step 6: Manually verify** `cargo run -- --template extraction`
      reaches the game rather than failing at load. The template test covers
      parsing; this covers the launcher path around it.
- [ ] **Step 7: Commit** — `refactor(engine): the dungeon is the Stack`

---

## Task 5: `DUNGEON_*` → `STACK_*` tuning constants

Separate from Task 4 because `tuning.rs` is where difficulty lives and a
constants rename should be reviewable without the type churn around it.

**Files:**
- Modify: `crates/engine/src/tuning.rs` and every consumer —
  `crates/engine/src/{dungeon,views}.rs`,
  `crates/engine/src/game/{dungeon,dungeon_view,dungeon_features,zone,lifecycle}.rs`,
  `crates/engine/src/tests/dungeon.rs`

**Interfaces:**
- Consumes: Task 3 already renamed `DUNGEON_FLOORS_*` and
  `DUNGEON_TILES_PER_FLOOR`. Do not rename those twice.
- Produces: the remaining names in the tuning table above.

**Test intent:** none new. Values do not change, so `balance_sim` is the
guard — if a curve moves, a value was altered, not just renamed.

- [ ] **Step 1: Rename the constants and their doc comments.** Several docs
      cross-reference each other by name (`DUNGEON_ENTRANCE_SCATTER_TILES` is
      named inside `DUNGEON_TILES_PER_FLOOR`'s doc, and vice versa) — those
      references must move too or the section becomes self-contradictory.
- [ ] **Step 2: Run `cargo test -p feral-processes-engine balance_sim`** —
      PASS with curves unmoved. A moved curve means a value changed; revert
      and find it.
- [ ] **Step 3: Run `cargo test --workspace`** — PASS, count unchanged
- [ ] **Step 4: Commit** — `refactor(engine): STACK_* tuning constants`

---

## Task 6: File and module renames

Last of the code tasks, so every earlier diff is readable as a content change
rather than as a file move.

**Files:** the eight moves in the "Files" table, plus the `mod` declarations
in `crates/engine/src/lib.rs:6`, `crates/engine/src/game/mod.rs:18-20`,
`crates/gui/src/render/mod.rs:27-28,50`, and both `tests/mod.rs` files.

**Interfaces:**
- Consumes: everything from Tasks 1–5.
- Produces: `engine::stack` as the public module path (was `engine::dungeon`).
  Any `crate::dungeon::` path in the engine and any
  `feral_processes_engine::dungeon::` path elsewhere becomes `stack`.

- [ ] **Step 1: `git mv` each file** — use `git mv`, not create-and-delete,
      so the moves show as renames in review
- [ ] **Step 2: Update the `mod` declarations and every `use` path**
- [ ] **Step 3: Run `cargo test --workspace`** — PASS, count unchanged
- [ ] **Step 4: `cargo clippy --workspace` and `cargo fmt`**
- [ ] **Step 5: Commit** — `refactor: move dungeon modules to stack`

---

## Task 7: Documentation

**Files:**
- Modify: `README.md` (the "Dungeons" section at line 51 and every "breach"
  used for the hole rather than for zone travel), `docs/manual.md` (the
  "In a dungeon:" section at 137 and the surrounding passages at 161-188),
  `CHANGELOG.md` (a new entry — do not rewrite history), `CLAUDE.md`
  (the load-bearing-seams entries naming `resources::Locale`,
  `DungeonMemory`, `dungeon::generate`, `LevelSpec::rng_seed`,
  `Game::view_cone`, `game/dungeon_view.rs`, `game/dungeon_features.rs` —
  all of which move), then `cp CLAUDE.md AGENTS.md`
- Modify: `crates/gui/src/render/meta.rs:151` — the third and last
  player-visible string, in the help screen's key list

**Interfaces:** none.

**The distinction to hold throughout the docs:** "breach" now means zone
travel *only*. Every use of it for the hole in the ground becomes "link".
Both README and manual currently use it for both, which is the ambiguity
this rename exists to remove — a pass that renames the types but leaves the
docs saying "breach" for both has done the churn without the benefit.

- [ ] **Step 1: Rewrite the README and manual sections** in the new
      vocabulary, checking each "breach" for which meaning it carries
- [ ] **Step 2: Update CLAUDE.md's seams entries and `cp` to AGENTS.md**
- [ ] **Step 3: Update `render/meta.rs:151`** and its `cargo test -p
      feral-processes-gui` layout tests if the longer string reflows
- [ ] **Step 4: `rg -i 'dungeon' -g '!target' .`** — expect zero hits outside
      CHANGELOG history
- [ ] **Step 5: Run `cargo test --workspace`** — PASS, count unchanged
- [ ] **Step 6: Commit** — `docs: the dungeon is the Stack`

---

## Final gate

- [ ] `cargo test --workspace` — same count this phase started at, plus the
      three tests added in Tasks 3 and 4
- [ ] `cargo clippy --workspace` — clean
- [ ] `cargo fmt --check` — clean
- [ ] `cargo test -p feral-processes-engine balance_sim` — curves unmoved
- [ ] `cargo run -- --template extraction` — loads
- [ ] `SAVE_FORMAT_VERSION` still 15
- [ ] `rg -i dungeon -g '!target' .` — zero hits outside CHANGELOG history

## Not in this phase

Trace, the three new cell kinds, the inhabitants, and the corner map inset.
Each gets its own plan, written once its predecessor has landed — see the
spec's phase list. Nothing in this phase adds a field, a variant, or a
behaviour.
