# Floor Finishes Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A decorative finish (a shade plus optional carved sprite) that the dig crew paints onto laid base floor. It is moddable from `assets/floors/` and worth a comfort memory.

**Architecture:** `floors::FloorDb` is a new catalogue loaded beside `RockDb`. `BaseGrid` gains a sparse `finishes` map. A finish is a third kind of dig mark (`DigSite::finish`), worked by the existing dig crew. gui reads finishes through a new engine view and never touches `FloorDb` or `BaseGrid`.

**Tech Stack:** Rust, bevy_ecs 0.19, serde/RON, bevy + bevy_egui (gui).

**Spec:** `docs/superpowers/specs/2026-09-16-floor-finishes-design.md`. Read it before any task. This plan argues from it, and the section below lists where the plan departs from it.

## Where this plan corrects the spec

The corrections come from checking the spec against source on 2026-09-17. The plan wins wherever the two disagree.

1. **`Tile` stays untouched.** `world::Tile` is `Copy` and is saved through `tile_overrides`, so a `String` sprite key cannot ride on it. Instead the engine adds a parallel `Game::view_finishes_at(center, half_w, half_h) -> Vec<Vec<Option<FinishView>>>`, the same shape as `view_tiles_at`. gui calls it only in base space.
2. **`DigSite` loses `Copy`.** `FinishOrder::Apply(FloorId)` holds a `String`. Fix the call sites that copied it; do not index the id to keep `Copy`.
3. **`MemorySubject::BaseTile` is `BaseTile { x, y }`.** It is a struct variant, not `BaseTile(pos)`.
4. **The dry rule is not `build_is_workable`.** `dig_wants` gates tile jobs on `substrate_in_stock()`, which asks whether there is at least one. That becomes `substrate_available() -> u32`, a count across base stores and the pack. A tile job needs `>= 1` and an `Apply` job needs `>= FLOOR_FINISH_COST`.
5. **`MEMORY_TRIGGERS`** is in `crates/engine/src/tests/assets.rs`. There is no `crates/engine/tests/` directory.
6. **`MemoryDb` has no sorted `iter`.** It has an unsorted `all()`. `FloorDb::iter` sorts by id itself.
7. **The excavate header is new work.** `Mode::Excavate` draws no text today (`draw_excavation_plan`, `crates/gui/src/render/base.rs`). This plan adds a one-line brush label to that function, and it shows only when `FloorDb` is non-empty.
8. **The examine line is new work.** `Game::describe_base_rock` walks a ray and names only the first solid cell it hits. When the cell one step in the examined direction is finished floor, the reply names the finish first ("Cobalt Carpet underfoot; …") and then keeps whatever it said about rock.
9. **`BaseGrid::revert`** removes a cell, so it also clears any finish there. That makes "a finish implies floor" a property of the store while the game is running, not only at load.

## Global Constraints

- `FLOOR_FINISH_COST = 3` (`blank_substrate`), in `tuning.rs` next to `BASE_DIG_TICKS_PER_SWING`.
- `FINISH_EDGE_LEVEL = 0.5` and the edge width `r.w / 16`, in gui.
- The ten `FloorShade`s and their RGB values are exactly as in the spec's table.
- There is **no `SAVE_FORMAT_VERSION` bump**. Every new save field is `#[serde(default)]`.
- An absent `assets/floors/` is a supported install and gives today's game exactly. A malformed file is skipped with a warning and never panics.
- New actions in app-core are UPPERCASE (`F`), per the row-selector rule.
- `render/` draws only through `Painter`. No backend calls outside `paint.rs`.
- gui never reads `FloorDb` or `BaseGrid`, and `Game` gains no `world` accessor.
- Finish wants come **after** cut and tile wants in `dig_wants`' output.
- The comfort memory is written only through `Game::remember`.
- Every task ends with `cargo fmt`, `cargo clippy --workspace --all-targets` (no warnings) and the task's targeted tests. Run `cargo test --workspace` at the end of Tasks 4, 6 and 8.
- Commit per green step, on branch `feat/floor-finishes`. **Never push**, never `git add -A`, and never `git checkout`/`stash` over uncommitted work.

## File map

| File | Responsibility |
|---|---|
| `crates/engine/src/floors.rs` (new) | `FloorId`, `FloorShade`, `FloorDef`, `FloorDb` |
| `crates/engine/src/base_grid.rs` | `finishes` store, its three methods, `revert` clearing |
| `crates/engine/src/components.rs` | `FinishOrder`, `DigSite::finish` |
| `crates/engine/src/save.rs` | `DigSiteSave::finish` |
| `crates/engine/src/game/lifecycle.rs` | load `FloorDb`, prune finishes on load, save/restore site finish |
| `crates/engine/src/game/base_space.rs` | brush through `toggle_mark_box`/`set_mark`, crew finish arm, `spend_substrate`, examine |
| `crates/engine/src/game/base/work_orders.rs` | finish wants, `substrate_available`, dry announcement wording |
| `crates/engine/src/game/memories.rs` + `game/turn.rs` | `note_comforts` and its call |
| `crates/engine/src/game/inspection.rs` | `FinishView`, `view_finishes_at` |
| `crates/app-core/src/app/excavate.rs`, `lib.rs`, `app/lifecycle.rs` | brush state, `[F]`, brush label |
| `crates/app-core/src/app/sprite_forge.rs` | floor subjects, `SubjectTint` |
| `crates/gui/src/render/terrain.rs`, `render/base.rs`, `render/sprite_forge.rs` | `shade_color`, census, `draw_finish`, header, editor hue |
| `assets/floors/` (new), `assets/sprites/`, `assets/memories/` | content and README |

---

### Task 1: The catalogue and its content

**Files:**
- Create: `crates/engine/src/floors.rs`, `assets/floors/README.md`, `assets/floors/cobalt_carpet.ron`, `assets/floors/moss_weave.ron`, `assets/floors/slate_inlay.ron`, `assets/memories/at_ease_on.ron`, and the three PNGs `assets/sprites/{cobalt_carpet,moss_weave,slate_inlay}.png`
- Modify: `crates/engine/src/lib.rs` (`pub mod floors;` between `environment` and `game`), `crates/engine/src/tuning.rs`, `crates/engine/src/game/lifecycle.rs` (`AssetDbs`, `load_asset_dbs`, the `Game::new`/`Game::load` destructure, `insert_resource`), `assets/memories/README.md` if it lists defs, `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces:
  - `pub struct FloorId(pub String)`, deriving `Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize`, with `From<&str>` and `as_str()`. Follow `ItemId`'s shape.
  - `pub enum FloorShade { Cobalt, Teal, Moss, Olive, Ochre, Umber, Wine, Plum, Violet, Slate }`, deriving `Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize`, plus `pub const ALL: [FloorShade; 10]`.
  - `pub struct FloorDef { id, name, description, shade, #[serde(default)] sprite: Option<String>, #[serde(default)] comfort: Option<String> }`, with `pub fn sprite_name(&self) -> &str`. Copy `SpeciesDef::sprite_name` exactly: an `@`-prefixed override is ignored.
  - `#[derive(Resource, Default, Clone)] pub struct FloorDb` with:
    - `load_dir(&Path) -> std::io::Result<(Self, Vec<String>)>`
    - `get(&FloorId) -> Option<&FloorDef>`
    - `iter() -> impl Iterator<Item = &FloorDef>`, sorted by id
    - `is_empty()`
  - `tuning::FLOOR_FINISH_COST: u32 = 3`.
  - `FloorDb` is inserted as a world resource in both `Game::new` and `Game::load`.
  - `pub fn floor_defs(&self) -> Vec<FloorDef>` on `Game`, a cloned sorted list for app-core.

- [ ] **Step 1: Write the failing tests.** Put them in `floors.rs`'s test module, following `MemoryDb`'s. They use a scratch directory and must clean it up; see the memory note on `/tmp` leaks and `tests/support.rs`. Assert:
  - an absent directory gives an empty db and no warnings;
  - a malformed file and a file with an unknown shade name are each skipped with a warning while the good file loads;
  - `iter` is id-sorted even when the files were written in reverse order;
  - `sprite_name` falls back to `id`, uses the override, and ignores `@x`.
- [ ] **Step 2:** Run `cargo test -p feral-processes-engine floors`. Expect FAIL (the module is missing).
- [ ] **Step 3:** Implement `floors.rs` by copying `MemoryDb::load_dir`'s NotFound/skip-and-warn shape. Wire it into `AssetDbs` beside `rock`. Add `Game::floor_defs`.
- [ ] **Step 4:** Add content.
  - Three `.ron` files:
    - `cobalt_carpet` (Cobalt, `comfort: Some("at_ease_on")`)
    - `moss_weave` (Moss, `comfort: Some("at_ease_on")`)
    - `slate_inlay` (Slate, no comfort)
  - `at_ease_on.ron`: `name: "At ease here"`, a one-line blurb, `valence: 3.0`, `half_life: 3000`, `subject: BaseTile`, `strike_cap: 3`. Copy `stranded_at.ron`'s field set exactly.
  - Three 16x16 PNGs, near-white on transparent, each a simple distinct pattern (a woven check, a diagonal, an inlaid border). Generate them with a throwaway script in the scratchpad, and open one with the Read tool to confirm it decodes.
  - `assets/floors/README.md` documents every field, the ten shades, the fixed cost, the absent-directory rule and the sprite fallback. Use `assets/memories/README.md` as the model.
- [ ] **Step 5: Asset census tests** in `tests/assets.rs`:
  - add `("at_ease_on", K::BaseTile)` to `MEMORY_TRIGGERS`;
  - add `every_shipped_floor_comfort_names_a_shipped_memory`, which loads both dbs and checks that each `Some` resolves to a `BaseTile` def;
  - add `shipped_floors_include_one_with_and_one_without_comfort`.
- [ ] **Step 6:** Run `cargo test -p feral-processes-engine floors assets`, then fmt and clippy. Expect PASS.
- [ ] **Step 7:** Commit: `Engine: FloorDb and three shipped finishes (todo #95)`.

---

### Task 2: The finishes store and its save

**Files:**
- Modify: `crates/engine/src/base_grid.rs`, `crates/engine/src/game/lifecycle.rs`
- Test: `base_grid.rs` test module; a save→load test in `crates/engine/src/tests/` beside the existing base-space save tests (find them with `rg -l 'base_grid' crates/engine/src/tests`)

**Interfaces:**
- Consumes: `FloorId`, `FloorDb` (Task 1).
- Produces:
  - `BaseGrid` gains `#[serde(default)] finishes: BTreeMap<(i32, i32), FloorId>`. It is private and keeps `PartialEq`/`Eq`.
  - `pub fn finish_at(&self, x: i32, y: i32) -> Option<&FloorId>`.
  - `pub(crate) fn set_finish(&mut self, x: i32, y: i32, id: FloorId) -> bool`. It returns false and does nothing unless `is_floor`.
  - `pub(crate) fn clear_finish(&mut self, x: i32, y: i32) -> bool`, which returns whether a finish was removed.
  - `pub(crate) fn prune_finishes(&mut self, db: &FloorDb) -> Vec<String>`. It drops unknown ids and non-floor cells and returns one warning line per drop.
  - `revert` also clears the finish on that cell.

- [ ] **Step 1: Failing unit tests.**
  - `set_finish` refuses an `Open` cell and a solid cell, and accepts `Floor`.
  - `clear_finish` leaves the cell `Floor`.
  - `revert` on a finished cell leaves no finish.
  - `prune_finishes` drops an unknown id and a non-floor entry and keeps a good one. Build the bad entries by writing the RON directly, since the setter refuses them.
- [ ] **Step 2:** Run them and watch them fail.
- [ ] **Step 3:** Implement. In `Game::load`, call `prune_finishes` against the loaded `FloorDb` right before `insert_resource(data.base_grid)`, and route its lines through the existing load-warning path. Find how `load_asset_dbs` warnings surface and use the same sink, logged once.
- [ ] **Step 4: Save→load test.** Do a real `Game::save` then `Game::load`, **not** a RON round trip. Lay a floor, finish it, save, load, and assert `finish_at`. Mark `finishes` as `#[serde(skip)]` temporarily, confirm the test fails, then restore it. A RON round trip cannot catch a skipped field.
- [ ] **Step 5:** Tests, fmt and clippy pass. Commit: `Engine: BaseGrid holds floor finishes, pruned on load (todo #95)`.

---

### Task 3: A finish is a dig mark

**Files:**
- Modify: `crates/engine/src/components.rs`, `crates/engine/src/save.rs`, `crates/engine/src/game/base_space.rs` (`toggle_mark_box`, `set_mark`), `crates/engine/src/game/lifecycle.rs` (`dig_site_saves_for`, `restore_dig_sites`), plus every caller of `toggle_mark_box`: `crates/app-core/src/app/excavate.rs:62` and the engine and app-core tests (`rg -n toggle_mark_box`)
- Test: `crates/engine/src/tests/base_space.rs`

**Interfaces:**
- Consumes: `FloorId`, `BaseGrid::finish_at`.
- Produces:
  - `#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)] pub enum FinishOrder { Apply(FloorId), Strip }` in `components.rs`.
  - `DigSite` gains `pub finish: Option<FinishOrder>` and **drops `Copy`**.
  - `DigSiteSave` gains `#[serde(default)] pub finish: Option<FinishOrder>`.
  - `pub fn toggle_mark_box(&mut self, a: (i32, i32), b: (i32, i32), brush: Option<&FinishOrder>)`. `None` is the plain brush.
  - `set_mark` takes the brush and decides per cell, exactly by the spec's table. Finish and strip marks spawn with `Durability { hp: 0, .. }`.
  - When the brush is marking and the cell already has a site of another kind, overwrite the site's `finish`; do not spawn a second site.
  - Clearing is unchanged: if the anchor is marked, every site in the box is cleared, whatever its kind.

- [ ] **Step 1: Failing tests.**
  - Write one assertion per cell of the spec's brush table: three brushes × five cell states, 15 cases. A table-driven test is fine as long as a failure message names the brush and the cell state.
  - Add a clear test: a finish-marked box is cleared by re-committing it with the plain brush.
- [ ] **Step 2:** Watch them fail. They should fail at compile time on the new parameter, and that counts.
- [ ] **Step 3:** Implement. Existing callers pass `None`. Where `DigSite` was copied, borrow or `.clone()`, following the borrow checker's grain.
- [ ] **Step 4: Save test.** Save and load a finish-marked site and a strip-marked site, then assert the `finish` fields.
- [ ] **Step 5:** Run `cargo test -p feral-processes-engine base_space` and `cargo test -p feral-processes-app-core excavate`, then fmt and clippy. Commit: `Engine: a finish or strip is a dig mark (todo #95)`.

---

### Task 4: The crew lays and strips finishes

**Files:**
- Modify: `crates/engine/src/game/base/work_orders.rs` (`dig_wants`, `substrate_in_stock` → `substrate_available`, `announce_dig_dry`), `crates/engine/src/game/base_space.rs` (`run_dig_crew`'s landing branch, `crew_lays_tile`, `spend_one_substrate` → `spend_substrate`)
- Test: `crates/engine/src/tests/base_space.rs` and/or `work_orders.rs` tests. Read `tests/support.rs` for the dig-crew fixtures first; `park_at_post()` may apply.

**Interfaces:**
- Consumes: `DigSite::finish`, `BaseGrid::{set_finish, clear_finish, finish_at, is_floor}`, `FLOOR_FINISH_COST`.
- Produces:
  - `fn substrate_available(&self) -> u32` is base holding plus pack count. It replaces the bool; the tile rule becomes `>= 1`.
  - `fn spend_substrate(&mut self, substrate: &ItemId, count: u32) -> bool` refuses whole when `substrate_available() < count`. Otherwise it spends from base stores first (`stock::spend_from_base`), then the pack, with the same ledger source as today. `crew_lays_tile` calls it with 1.
  - `dig_wants`:
    - An `Apply` site is a want only with `substrate_available() >= FLOOR_FINISH_COST`. Otherwise it calls `announce_dig_dry`, which is latched as today.
    - A `Strip` site is always a want.
    - The output is ordered cut/tile sites by (x, y), then finish/strip sites by (x, y).
  - `announce_dig_dry` takes the reason so the finish line reads naturally. Both wordings name the item; the tile wording is unchanged.
  - `run_dig_crew`'s landing branch for a site with `finish`:
    - `Apply(id)`: if the cell is not floor, or already wears `id`, despawn the site with no charge. Otherwise `spend_substrate(cost)`, then `set_finish`, then despawn the site and `log_base` a line. If the spend is refused, the site stays and the next `dig_wants` announces it dry.
    - `Strip`: `clear_finish`, despawn, log, no charge.
  - A site with a `finish` never reaches `strike_rock` or `crew_lays_tile`.

- [ ] **Step 1: Failing tests.** Each is its own test.
  - Apply spends exactly `FLOOR_FINISH_COST` and the cell wears the finish.
  - With `FLOOR_FINISH_COST - 1` in reach, nothing is spent and the site is not a want.
  - Replacing a finish spends the cost and refunds nothing.
  - Strip leaves `Floor`, removes the finish, and moves no stock.
  - A dry Apply announces **once** across several ticks. Sum `repeats` if you read `message_history`, since it condenses repeated lines.
  - A Strip site is a want with zero substrate.
  - A cell that stopped being floor under an Apply site despawns it uncharged. Use `BaseGrid::revert` to make one.
  - Ordering: with one tile site and one finish site and a single worker, the tile site is worked first.
  - `spend_substrate` split across base and pack spends the base first; below the count it moves nothing.
- [ ] **Step 2:** Watch them fail.
- [ ] **Step 3:** Implement.
- [ ] **Step 4:** Mutation check. Delete the `>= FLOOR_FINISH_COST` gate and confirm the dry test fails, then restore it. Swap the wants order and confirm the ordering test fails, then restore it.
- [ ] **Step 5:** `cargo test --workspace`, fmt and clippy all pass. Commit: `Engine: the dig crew lays and strips floor finishes (todo #95)`.

---

### Task 5: Comfort memories

**Files:**
- Modify: `crates/engine/src/game/memories.rs`, `crates/engine/src/game/turn.rs` (after `note_postings()` at about :296)
- Test: the memories tests module (`rg -l note_postings crates/engine/src/tests`)

**Interfaces:**
- Consumes: `BaseGrid::finish_at`, `FloorDb::get`, `Game::remember`, `MEMORY_POSTING_PERIOD`.
- Produces: `pub(crate) fn note_comforts(&mut self)`. It uses the same period gate as `note_postings`. For every program standing in base space (use whatever `note_postings` or `note_respites` uses to find base-space bodies), whose cell's finish resolves to a def with `comfort: Some(m)`, it calls `remember(who, &m, MemorySubject::BaseTile { x, y })`. The ids are collected first and the writes done after, so there is no borrow conflict.

- [ ] **Step 1: Failing tests.**
  - A program on a comfort finish, ticked to the period, has the `at_ease_on` memory.
  - On plain floor it has nothing.
  - On `slate_inlay` it has nothing.
  - With `comfort` naming an unknown memory id, nothing is written and nothing panics. Use a test `FloorDb` inserted into the world.
- [ ] **Step 2:** Watch them fail. **Step 3:** Implement. **Step 4:** Delete the call in `turn.rs` and confirm the first test fails, then restore it.
- [ ] **Step 5:** Tests, fmt and clippy pass. Commit: `Engine: a comfortable finish is remembered (todo #95)`.

---

### Task 6: The engine's view of a finish

**Files:**
- Modify: `crates/engine/src/game/inspection.rs`, `crates/engine/src/game/base_space.rs` (`describe_base_rock`), and `crates/engine/src/views.rs` if view structs live there (check where `Tile`-adjacent view types are declared)
- Test: the inspection tests

**Interfaces:**
- Consumes: `BaseGrid::finish_at`, `FloorDb::get`, `FloorShade`.
- Produces:
  - `#[derive(Clone, Debug, PartialEq)] pub struct FinishView { pub shade: FloorShade, pub sprite: String, pub name: String }`.
  - `pub fn view_finishes_at(&self, center: (i32, i32), half_w: i32, half_h: i32) -> Vec<Vec<Option<FinishView>>>`. Its indexing is identical to `view_tiles_at`; read that function and match its row/column order. It is all `None` outside base space. An id `FloorDb` does not resolve gives `None`.
  - `describe_base_rock` prefixes the finish's name when the cell one step along `(dx, dy)` is finished floor, as described in correction 8. When a finish is found but no rock is in range, it still returns `Some`.

- [ ] **Step 1: Failing tests.**
  - The grid shape matches `view_tiles_at` for the same arguments.
  - A finished cell appears at the same index `view_tiles_at` gives its `Platform` tile.
  - The view is all `None` off base space.
  - The examine line names the finish.
- [ ] **Step 2:** Fail. **Step 3:** Implement. **Step 4:** `cargo test --workspace`, fmt and clippy pass.
- [ ] **Step 5:** Commit: `Engine: finishes are visible through view_finishes_at and examine (todo #95)`.

---

### Task 7: app-core, the brush and the sprite editor

**Files:**
- Modify: `crates/app-core/src/lib.rs` (field beside `excavate_anchor`), `crates/app-core/src/app/lifecycle.rs` (initialise it), `crates/app-core/src/app/excavate.rs`, `crates/app-core/src/app/sprite_forge.rs`
- Test: `crates/app-core/src/tests/excavate.rs`, and the sprite forge tests

**Interfaces:**
- Consumes: `Game::floor_defs`, `FinishOrder`, and `toggle_mark_box(.., brush)`.
- Produces:
  - `pub excavate_brush: Option<FinishOrder>` on `App`. It resets to `None` when the mode opens.
  - In `handle_excavate_key`, `GameKey::Char('F')` cycles: `None` → `Apply(first id)` → … → `Apply(last)` → `Strip` → `None`. With empty `floor_defs` the key does nothing. The commit passes `self.excavate_brush.as_ref()`.
  - `pub fn excavate_brush_label(&self) -> Option<String>`. It returns `None` when `floor_defs` is empty; otherwise `"Brush: plain [F]"`, `"Brush: <name> [F]"` or `"Brush: strip [F]"`.
  - `pub enum SubjectTint { Glyph(Option<GlyphColor>), Shade(FloorShade) }` replaces `SpriteSubject::color` and the fourth field of `StaticSpriteSubject`.
  - `load_static_sprite_subjects` loads `FloorDb` from `assets_dir.join("floors")` using `unwrap_or_default`. It inserts each floor subject after the structures and before the player and anchor entries, keyed on `sprite_name()`, labelled with `name`, glyph `'_'`, tint `Shade(shade)`.

- [ ] **Step 1: Failing tests.**
  - The cycle order with the shipped floors.
  - Inert with an empty `FloorDb`. Build that app against a copy of the assets directory without `floors/`, or whatever fixture `tests/excavate.rs` already uses.
  - The brush reaches the engine: commit a box with a finish brush over laid floor, and a site with `Apply` exists.
  - The label text for each brush state, and `None` when the db is empty.
  - The sprite editor lists all three floor subjects with `Shade` tints, placed after structures.
- [ ] **Step 2:** Fail. **Step 3:** Implement. **Step 4:** Run `cargo test -p feral-processes-app-core`, fmt and clippy. The app-core `tests::creation` module is a known flake; if it fails, re-run that module and do not fix it here.
- [ ] **Step 5:** Commit: `App-core: [F] cycles the excavate brush; floors in the sprite editor (todo #95)`.

---

### Task 8: gui, drawing a finish

**Files:**
- Modify: `crates/gui/src/render/terrain.rs`, `crates/gui/src/render/base.rs`, `crates/gui/src/render/sprite_forge.rs`
- Test: those files' test modules. Use `paint::with_painter` for the draw assertions, following the existing sprite overdraw test (`rg -n 'with_painter' crates/gui/src/render/base.rs`).

**Interfaces:**
- Consumes: `Game::view_finishes_at`, `FinishView`, `App::excavate_brush_label`, `SubjectTint`.
- Produces:
  - `pub(crate) fn shade_color(shade: FloorShade) -> Color` in `terrain.rs`, an exhaustive match giving the spec's RGB values.
  - `const FINISH_EDGE_LEVEL: f32 = 0.5`.
  - `pub(super) fn draw_finish(painter: &Painter, r: Rect, finish: &FinishView, dim: f32)`. It draws three layers on `draw_slab`'s inset rect: the shade fill, the edge ring at `at_level(shade, FINISH_EDGE_LEVEL)` one `r.w / 16` wide, then `painter.sprite(&finish.sprite, …, tint = shade)`.
  - In `render/base.rs`'s tile loop, a `Platform` tile with `Some(finish)` calls `draw_finish` instead of `draw_biome`. It keeps the same `!occupied` gate and the same `dim` and critical-wash treatment that the plain floor gets. `view_finishes_at` is called once per frame, beside `view_tiles_at`.
  - `draw_excavation_plan` draws `excavate_brush_label()` as one line inside the map pane's top edge, when it is `Some`. Its origin comes from the pane rect; a literal `0.0` would draw under the status bar.
  - `subject_hue` takes `&SubjectTint`, and a `Shade` resolves through `shade_color`.

- [ ] **Step 1: Measure before asserting.** Write a throwaway test that prints, for every shade, its minimum Euclidean RGB distance to each reference:
  - `Entropy` brightened by 1.0..=4.0, in 0.1 steps, through `brighten`;
  - `Excavated` and `Platform`;
  - `THREAT`;
  - the damage wash, which is a plain slab after `structure_condition`'s critical red lift;
  - every other shade.

  **Stop and report if any shade is very close to a rock face.** From the table, `Umber` (0.24, 0.16, 0.10) looks within about 0.07 of a brightened `Entropy`. If it is, that is a spec defect to raise with the user. Do not lower the threshold until it passes.
- [ ] **Step 2: The census.** Add `FINISH_SHADE_MIN_SEPARATION`, set from the measurement and justified in a comment. Add a test asserting every pair clears it, whose failure message names the shade and the reference.
- [ ] **Step 3: Failing draw tests.**
  - A finished cell draws the fill, the darker edge and the sprite mesh.
  - With a sprite name the table lacks, it draws the fill and edge alone.
  - A plain `Platform` cell still draws the slab. This is a regression check.
  - The excavate header draws the label when `Some` and nothing when `None`.
- [ ] **Step 4:** Implement. **Step 5:** `cargo test --workspace`, fmt and clippy all pass.
- [ ] **Step 6:** Commit: `Gui: floor finishes are drawn, and the excavate header names the brush (todo #95)`.

---

### Task 9: The seam, three writes

**Files:**
- Modify: `CLAUDE.md` ("The base" section), `.claude/skills/seams/` (the base reference file), the memory graph (`seam:floor-finishes`), and `docs/superpowers/specs/2026-09-16-floor-finishes-design.md` (append a short "Corrected in the plan" note pointing here)

- [ ] **Step 1:** Read the `seams` skill for the order of the three writes.
- [ ] **Step 2:** Write the argument to the graph first:
  - why `Tile` was not widened;
  - why `FloorShade` is an enum;
  - why finish wants come last;
  - the separation measurement.
- [ ] **Step 3:** Add the trap to the skill: a finish on a non-floor cell; `DigSite` no longer being `Copy`; a finish want inserted above tile wants starving the floor.
- [ ] **Step 4:** Add the rule to CLAUDE.md as **one sentence**: "A finish is a third dig mark over laid floor, stored beside `BaseGrid`'s cells and never in them, and `view_finishes_at` is gui's only door to it."
- [ ] **Step 5:** Commit: `Docs: floor finishes seam (todo #95)`.

The CHANGELOG, version bump and tag happen at the merge, in the deploy skill. They do not happen on this branch.
