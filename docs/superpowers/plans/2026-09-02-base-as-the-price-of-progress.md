# A working base is the price of progress — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: use `superpowers:subagent-driven-development`
> to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax.

**Goal:** Make the base structurally required for progression — hand-crafting
costs an order of magnitude more time than the machine that exists to do it,
the Zone Portal's bill demands a terminal product from every base chain and
grows with the sector, and the power grid burns Power Cells to stay up.

**Architecture:** Three independent engine changes plus one UI change. Each
lands behind an additive `#[serde(default)]` schema field or a new
`tuning.rs` constant, so no save-format bump. Tasks A, C and D share no files;
B consumes A's interface only.

**Tech Stack:** Rust, `bevy_ecs 0.19` (engine, standalone), `bevy` +
`bevy_egui` (gui), RON assets.

**Spec:** [`docs/superpowers/specs/2026-09-02-base-as-the-price-of-progress-design.md`](../specs/2026-09-02-base-as-the-price-of-progress-design.md)
— read it before starting a task; this plan argues from it.

## How this plan deviates from the writing-plans skill

`CLAUDE.md`'s **Process weight** section overrides the skill's "code blocks
required for code steps": *"don't write the implementation inside it. A
subagent that has the repo and this file needs the file list, the interface it
must produce, the intent of each test, and the gates to run — not finished
code it will merely re-emit. Reserve code blocks for the genuinely
non-obvious."* Code below appears only where the shape is not derivable from
the surrounding source: an ordering constraint, a formula, an existing
signature you must match.

## Global Constraints

**Every task's requirements implicitly include this section.**

- **TDD, always.** Failing reproducer first, watch it fail, minimal
  implementation, watch it pass, commit. Applies at every task size.
- **No save-format bump.** `save::SAVE_FORMAT_VERSION` is `32`
  (`crates/engine/src/save.rs:1163`) and stays there. The payload is
  field-named RON, so an additive field behind `#[serde(default)]` earns no
  bump. If you think you need one, stop and report.
- **Moddability.** New content goes in `assets/`; new *difficulty* goes in
  `crates/engine/src/tuning.rs` as a documented `pub const` in the right
  labelled section. Never hardcode content in Rust.
- **A schema change updates `assets/structures/README.md` in the same
  commit.** That file is the schema reference for anyone modding.
- **A malformed `.ron` is skipped with a logged warning**, never a panic.
- **Never add a `world()`/`world_mut()` accessor to `Game`.** `Game` is the
  entire public API the renderer sees.
- **A doc comment claiming to mirror another formula must be a call, not a
  copy.** If you write "mirrors" or "same as" about another module's
  arithmetic, extract a pure function both sides call.
- **Comments explain why, never what.**
- **Gates before calling a task done:** `cargo test --workspace`,
  `cargo clippy --workspace` (fix warnings, don't silence), `cargo fmt`.
- **Do not push.** Commit freely on the branch; the orchestrator merges,
  versions and tags.
- **Do not bump the workspace version or edit `CHANGELOG.md`** — that happens
  once, at the merge.
- Branch: `feat/base-as-the-price-of-progress`.

### Reading the repo first

`CLAUDE.md` is loaded into your context every turn and states each
load-bearing seam in one sentence. Before touching a seam, invoke the
**`seams` skill** and read the reference file for the subsystem — the trap
behind the rule lives there, and `docs/seams.md` holds the full argument.
Tasks A and B touch items/crafting; C and D touch the base.

### Where the constants go

`crates/engine/src/tuning.rs` is sectioned by `// ---- Label ----` banners.
- Hand-craft constants → beside the quality/craft constants around line 1780
  (`QUALITY_BENCH_PER_TIER` at 1788, `QUALITY_CAREFUL_COST_PERCENT` at 1816).
- `POWER_UPKEEP_TICKS` → the **Production chains** section (banner at ~2201,
  where `DEFAULT_OUTPUT_CAPACITY` lives).

Each constant needs a doc comment in the module's voice: what it is, and *why
this number* — the argument, not a restatement.

### Test fixtures

Engine tests live at `crates/engine/src/tests/`, one file per topic, all
`use super::support::*`. **Read `crates/engine/src/tests/support.rs` before
hand-rolling any fixture.** Relevant helpers already exist, including:

```rust
pub(crate) fn stand_in_base(game: &mut Game);
pub(super) fn spawn_structure_at(game: &mut Game, kind: &str, x: i32, y: i32);
pub(super) fn spawn_machine_at(game: &mut Game, kind: &str, x: i32, y: i32) -> Entity;
pub(super) fn place_home(game: &mut Game);
pub(super) fn stand_ample_grid_supply(game: &mut Game);
pub(super) fn node_output(game: &Game, structure: Entity, item: &str) -> u32;
pub(super) fn set_inventory(game: &mut Game, stock: &[(&str, u32)]);
pub(super) fn give(game: &mut Game, item: &ItemId, qty: u32);
pub(super) fn held(game: &Game, item: &ItemId) -> u32;
pub(super) fn count_item(game: &Game, id: &str) -> u32;
pub(super) fn assets_dir_with_extra_structure(tag: &str, name: &str, body: &str) -> ScratchAssets;
```

Topic files: crafting → `tests/crafting.rs`; power → `tests/power.rs`;
structures → `tests/building.rs`; build requests → `tests/construction.rs`;
asset censuses → `tests/assets.rs`.

### Seam bookkeeping

A new load-bearing seam is **three writes, in this order** — the argument to
`docs/seams.md`, the trap to
`.claude/skills/seams/references/<subsystem>.md`, the one-sentence rule to
`CLAUDE.md`'s "Load-bearing seams" section. `CLAUDE.md` is loaded every turn,
so **one sentence there is a budget, not a style.**

`CLAUDE.md` is gitignored and has a twin: edit `CLAUDE.md`, then
`cp CLAUDE.md AGENTS.md`. Nothing tracks their drift.

Do the bookkeeping **in the task's final commit**, not as a follow-up.

---

## Task A: Hand-crafting takes real time

**Blocks Task B.** Independent of C and D.

**Files:**
- Modify: `crates/engine/src/tuning.rs`
- Modify: `crates/engine/src/game/crafting.rs` (`craft` at :232, `craft_cost`
  at :151, `player_craft_order` at :189, `CraftOrder` at :34)
- Modify: `crates/engine/src/resources.rs` — new `HandCraft`
- Test: `crates/engine/src/tests/crafting.rs`

**Interfaces — Produces (Task B consumes these exact names):**

```rust
// crates/engine/src/tuning.rs
pub const HAND_CRAFT_TICK_MULT: u32 = 10;
pub const HAND_CRAFT_DEFAULT_CYCLE: u32 = 10;

// impl Game
pub fn hand_craft_ticks(&self, item: &ItemId) -> u32;
pub fn begin_hand_craft(&mut self, result: &ItemId, quantity: u32, careful: bool) -> Result<(), String>;
pub fn advance_hand_craft(&mut self) -> Option<HandCraftProgress>;
pub fn abort_hand_craft(&mut self);
pub fn hand_craft_in_progress(&self) -> bool;

// crates/engine/src/resources.rs — Resource, NOT saved
pub struct HandCraft { /* item, remaining units, unit index, ticks spent in unit, careful */ }

pub struct HandCraftProgress {
    pub item: ItemId,
    pub unit: u32,        // 1-based, which unit of the batch
    pub units: u32,       // batch size
    pub ticks_done: u32,  // within the current unit
    pub ticks_total: u32, // == hand_craft_ticks(item)
    pub finished: bool,   // the whole batch is done; the resource is gone
}
```

`advance_hand_craft` returns `None` when nothing is in flight.

### The four non-obvious things

1. **`hand_craft_ticks` is the one door and the screen calls it.** The cycle
   it multiplies is, in order: the `ticks_per_unit` of the structure whose
   `assembles.item` is this item; else the `ticks_per_unit` of the structure
   whose `work.produces` is this item; else `HAND_CRAFT_DEFAULT_CYCLE`. There
   must be no second copy of this arithmetic — Task B displays the number by
   calling this, not by recomputing it.
2. **`Game::craft` is reimplemented on top of the loop**, keeping its
   signature `pub fn craft(&mut self, result: &ItemId, quantity: u32, careful:
   bool) -> Result<(), String>`: begin, then drain to completion. Every
   engine test and `crates/app-core/src/app/crafting.rs::commit_craft`
   already call it. `begin_hand_craft` holds the refusal list — in `craft`'s
   existing order: game over or active battle → `"Can't do that right now."`;
   `quantity == 0` → `"Compile at least 1."`; no recipe → `"{item} can't be
   compiled."`; inventory shortfall → the existing sentence. **Every refusal
   still lands before anything is spent.**
3. **Spend per unit, at the unit's start; roll quality and grant at the unit's
   end.** `advance_hand_craft` is the only code that spends or grants. An
   abort keeps completed units and refunds the in-flight one, so the only
   cost of an abort is time already spent. This mirrors the existing seam
   *materials are not spent until the structure is raised*, and it closes a
   real edge: a build crew can take from the player's pack while the party is
   in base space (`game/base/construction.rs`'s `Source::Pack`), so a craft
   that checked up front and spent at the end could find itself short.
4. **Break early exactly where drag terrain breaks.** `Game::move_player`
   (`crates/engine/src/game/turn.rs`, around 600-625) runs
   `for _ in 0..drag_ticks { … self.tick(); }` and breaks on game over or a
   battle starting. That is the precedent and the only one; match it. On an
   early break, treat it as an abort: refund the in-flight unit.

- [ ] **Step 1: Read** `crates/engine/src/game/crafting.rs` whole, plus
      `move_player`'s drag loop and `resources::RunFeats` (the precedent for
      an unsaved resource).

- [ ] **Step 2: Write the failing tests** in `tests/crafting.rs`. Eight
      behaviours, each its own test:
  1. `hand_craft_ticks("blank_substrate")` is `HAND_CRAFT_TICK_MULT × 12` —
     the Lathe's cycle. Assert against the Lathe's authored `ticks_per_unit`
     read from the db, not against a literal `120`, or the test pins content.
  2. `hand_craft_ticks("power_cell")` is `HAND_CRAFT_TICK_MULT × 6` — the
     Power Conduit's `work.ticks_per_unit`, proving the `work` fallback.
  3. An item no structure assembles or produces gets
     `HAND_CRAFT_TICK_MULT × HAND_CRAFT_DEFAULT_CYCLE`.
  4. `craft(item, 1, false)` advances `GameClock` by exactly
     `hand_craft_ticks(item)`.
  5. `craft(item, 3, false)` advances it by exactly `3 ×`.
  6. Each of the four refusals leaves the clock unmoved and the inventory
     untouched. **Assert per refusal** — a single test over one of four
     passes against three paths that never spend anyway.
  7. Aborting after the first unit of a batch of 3 completes: one item
     granted, the second unit's ingredients back in the pack, the third
     unit's never taken.
  8. A battle starting mid-batch ends the loop with the in-flight unit
     refunded. Drive this with an existing fixture that starts a fight from a
     tick rather than hand-inserting a `BattleState`.

- [ ] **Step 3: Run them and watch every one fail.**
      `cargo test -p feral-processes-engine crafting`
      A test that passes now is testing nothing.

- [ ] **Step 4: Add the two constants** with doc comments. For
      `HAND_CRAFT_TICK_MULT`, the argument is: the machine exists to do this,
      so hand-compiling has to be the expensive fallback rather than the
      free equivalent — 10× makes a Lathe's 12-tick substrate 120 ticks by
      hand, which is a real decision at the scale of a base's other cycles.

- [ ] **Step 5: Add `Game::hand_craft_ticks`** and its lookup.

- [ ] **Step 6: Add `resources::HandCraft` and `HandCraftProgress`,** then
      `begin_hand_craft` / `advance_hand_craft` / `abort_hand_craft` /
      `hand_craft_in_progress`. `HandCraft` is **not** registered in
      `save.rs`; say so in its doc comment and name `RunFeats` as the
      precedent.

- [ ] **Step 7: Reimplement `craft` on top of them.** Signature unchanged.

- [ ] **Step 8: Run the crafting tests; all eight pass.**

- [ ] **Step 9: Run `cargo test --workspace`.** **Expect fallout**, and it is
      the interesting part of this task: `craft` now advances the clock by
      the full duration instead of one tick, so tests that craft and then
      assert on something a background system touches will move.
      *(Outcome, recorded after the fact: exactly one test moved —
      `only_gear_spends_a_quality_roll`, whose doc had said in as many words
      that it worked "because `craft` ticks once whatever the batch size".)* Read each
      failure. Per the memory note on RNG-stream shifts, a seeded assertion
      that moves is usually incidental coupling in the fixture — re-ground it
      rather than re-seeding. Report anything you cannot explain instead of
      relaxing it.

- [ ] **Step 10: Seam bookkeeping.** One sentence for `CLAUDE.md`'s **Items,
      gear and economy** section, in the shape of *"Hand-compiling is priced
      at `hand_craft_ticks`, the machine's own cycle times a constant, and
      `Game::craft` is that loop drained to completion."* Trap for
      `references/items.md`: a second copy of the cycle lookup lets the
      screen quote a number the loop does not spend; and a refusal that
      lands after `begin` has armed the resource spends time before refusing.
      Argument for `docs/seams.md`: why slow rather than `machine_only`
      (every recipe stays reachable; the convenience is what is priced), and
      why per-unit spending rather than per-batch.

- [ ] **Step 11: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

## Task B: The compiling screen

**Depends on Task A only.** Do not start until A is committed.

**Files:**
- Modify: `crates/app-core/src/lib.rs` — `Mode` enum (:1097), `Mode::is_battle`
  (:1530)
- Modify: `crates/app-core/src/app/input.rs` — the `handle_key` dispatch
  (:190)
- Modify: `crates/app-core/src/app/crafting.rs` — `commit_craft`
- Modify: `crates/gui/src/render/mod.rs` — the `draw` dispatcher (:519),
  `ALL_MODES` (:1186), `needs_status_banner` (:482)
- Modify: `crates/gui/src/render/crafting.rs`
- Test: gui render tests in `crates/gui/src/render/`, app-core tests

**Interfaces — Consumes from Task A** (verified against `88110d57`, not
predicted):

```rust
// crates/engine/src/tuning.rs
pub const HAND_CRAFT_TICK_MULT: u32 = 10;
pub const HAND_CRAFT_DEFAULT_CYCLE: u32 = 10;

// impl Game
pub fn hand_craft_ticks(&self, item: &ItemId) -> u32;
pub fn hand_craft_in_progress(&self) -> bool;
pub fn begin_hand_craft(&mut self, result: &ItemId, quantity: u32, careful: bool) -> Result<(), String>;
pub fn advance_hand_craft(&mut self) -> Option<HandCraftProgress>;
pub fn abort_hand_craft(&mut self);

// re-exported at the crate root as feral_processes_engine::HandCraftProgress
pub struct HandCraftProgress {
    pub item: ItemId,
    pub unit: u32,        // 1-based
    pub units: u32,
    pub ticks_done: u32,
    pub ticks_total: u32,
    pub finished: bool,
}
```

Three things Task A settled that differ from this plan's original sketch:

1. **`advance_hand_craft` returns `Some(progress)` with `finished: true` on
   the call that ends the batch, then `None` on every call after** — the
   resource is gone by then. Stop on `p.finished`, not on the first `None`.
2. **On that finished report `ticks_total` is currently 0**, because no unit
   is in flight to size a bar against — so a bar would collapse on its last
   frame. **Task B is authorised to fix this in the engine**: make
   `close_hand_craft` carry the real total through. It is a two-line change
   in `crates/engine/src/game/crafting.rs` and it belongs to the screen that
   needs it.
3. `HandCraftProgress` carries no "how many came out" count. The batch is
   logged once on the way out with the true count, so B's existing
   `report` path needs no change.

**`resources::HandCraft` is inserted by `begin` and removed by `close`**, so a
run that never hand-compiles carries no resource at all. Do not "fix" this by
inserting it at the constructors — that is deliberate, and it is why adding
this feature shifted no query iteration order.

### The five non-obvious things

1. **`ALL_MODES` is a plain array, not an exhaustive match.**
   `crates/gui/src/render/mod.rs:1186` declares `const ALL_MODES: [Mode; 86]`.
   A forgotten variant **compiles fine** and is silently skipped by
   `every_screen_draws_a_refusal_exactly_once`. Add `Mode::Compiling` and
   bump the length to 87.
2. **`Mode::is_battle` (`crates/app-core/src/lib.rs:1530`) is exhaustive on
   purpose** — its doc says a new variant is a compile error until it is
   classified. Classify it (`false`).
3. **Reuse the existing bar.** `crates/gui/src/render/bars.rs:48`:
   ```rust
   pub(super) fn draw_bar(g: BarGeometry, label: &str, value: f32, max: f32,
                          style: BarStyle, painter: &Painter, m: &Metrics) -> f32
   ```
   with `BarStyle::plain(color)` — the pattern `render/manifest.rs`'s
   `Meter` rows already use. Do not invent a second progress bar, and do not
   name a graphics library: `crates/gui/src/paint.rs` is the only file that
   may, and `render/` draws through `Painter` alone.
4. **The screen must show the same number the engine spends** — call
   `Game::hand_craft_ticks`, never recompute `mult × cycle`.
5. **Any key aborts, and `Esc` is not special.** The spec's answer was "bar
   fills, and any key aborts". A key during `Mode::Compiling` calls
   `abort_hand_craft()` and returns to `Mode::Playing`.

- [ ] **Step 1: Read** `crates/app-core/src/app/crafting.rs` whole and
      `crates/gui/src/render/crafting.rs` whole. Note `commit_craft` is the
      one call site: it currently calls `game.craft(...)` and sets
      `Mode::Playing`.

- [ ] **Step 2: Write the failing tests.** Five behaviours:
  1. `ALL_MODES` contains `Mode::Compiling` (the census test already walks
     it — make it fail first by asserting the length).
  2. Committing a craft from `Mode::CraftQuantity` leaves the app in
     `Mode::Compiling` with a craft in flight, rather than back in
     `Mode::Playing`.
  3. A key press in `Mode::Compiling` aborts and returns to `Mode::Playing`.
  4. Advancing to completion returns to `Mode::Playing` and reports the
     outcome through the existing `report`/`refuse` path.
  5. The screen draws headlessly at 1280x720 without panicking and its rows
     fit — follow whatever the existing gui render tests do for a popup, and
     read the memory note: popup row width **is** testable headlessly via
     `paint::with_painter`, and `draw_row` clips vertically but never
     horizontally.

- [ ] **Step 3: Run them and watch them fail.**

- [ ] **Step 4: Add `Mode::Compiling`** and let the compiler find
      `is_battle` and the `handle_key` dispatch. Add `handle_compiling_key`.

- [ ] **Step 5: Rewrite `commit_craft`** to call `begin_hand_craft` and enter
      `Mode::Compiling` on success; on a refusal, report it and go to
      `Mode::Playing` exactly as today.

- [ ] **Step 6: Drive the loop.** The per-frame entry point is
      `App::update_realtime` (`crates/app-core/src/app/input.rs:619`), beside
      `advance_reveal(dt)` (:316) and `advance_status(dt)` (:389) — the
      existing per-frame drivers, and the pattern to follow. Drive
      `advance_hand_craft()` from there while `mode == Mode::Compiling`;
      do not add a second driver elsewhere. On `finished`, report and return
      to `Mode::Playing`.

      **Decide the pace deliberately and write the reason down.** One engine
      tick per frame makes a 300-tick Hardened Shell a five-second stare at
      60fps; a fixed wall-clock duration with the ticks spread across it is
      the alternative. Whichever you choose, the *ticks spent must be exactly*
      `hand_craft_ticks` — the pace is presentation, the cost is not.

- [ ] **Step 7: Draw the screen** in `render/crafting.rs` — a popup with the
      item name, `unit / units`, and `draw_bar` over
      `ticks_done / ticks_total`. Register it in the `draw` dispatcher
      (`render/mod.rs:519`) and in `ALL_MODES`. Check whether
      `needs_status_banner` needs it — it lists the five modes that draw no
      popup, so a popup screen should **not** be added there; confirm rather
      than assume.

- [ ] **Step 8: Run the tests; all five pass. Then `cargo test --workspace`.**

- [ ] **Step 9: Play it.** `cargo run` and compile something by hand. A green
      suite is not evidence the screen reads right. Report what you saw —
      whether the bar's pace is legible, and whether "any key aborts" is
      discoverable without a footer hint (add one if it is not).

- [ ] **Step 10: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

## Task C: The portal's bill grows with the sector

**Independent of every other task.**

**Files:**
- Modify: `crates/engine/src/structures.rs` — `StructureDef` (:224-446)
- Modify: `cratests/engine/src/game/catalog.rs` — `Game::structure_build_cost`
  (:803-820)
- Modify: `assets/structures/portal.ron`
- Modify: `assets/structures/README.md` (schema example: `build_cost` ~:17,
  `zone_portal` ~:245)
- Modify: `docs/seams.md`, `.claude/skills/seams/references/base.md`,
  `CLAUDE.md` (+ `cp CLAUDE.md AGENTS.md`)
- Test: `crates/engine/src/tests/building.rs` and
  `crates/engine/src/tests/construction.rs`

**Interfaces — Produces:** `StructureDef::zone_build_cost: Vec<(u32, ItemId,
u32)>`. `Game::structure_build_cost`'s signature is **unchanged** and takes a
def, not an id:

```rust
pub fn structure_build_cost(&self, def: &StructureDef) -> Vec<(ItemId, u32)>
```

No other task depends on either.

### The two non-obvious things

1. **Ramp from the zone the line was introduced in.** `zone_portal_cost`
   (`crates/engine/src/lib.rs:124`) ramps by `zone.saturating_sub(1)`. Price
   each line as:
   ```rust
   zone_portal_cost(base_qty, zone.saturating_sub(min_zone) + 1)
   ```
   with `build_cost` lines taking `min_zone = 1`. That reduces to exactly
   today's `zone_portal_cost(qty, zone)` for them — which is what makes this
   a no-op for the portal's existing line and for every other structure.
   Without it, a Cache Grain line authored for sector 2 arrives already
   inflated the first time it can legally be demanded.
2. **Keep the two early returns in order.** `structure_build_cost` returns
   empty for an unclaimed `first_free` structure, then returns `build_cost`
   unchanged for a non-`zone_portal` one. Append the qualifying
   `zone_build_cost` lines *before* the `zone_portal` branch so a non-portal
   structure that authors one still gets it — unramped.

- [ ] **Step 1: Read** `Game::structure_build_cost`, `zone_portal_cost` and
      its `ZONE_PORTAL_COST_GROWTH_PERCENT` doc, and `assets/structures/README.md`'s
      `zone_portal` section.

- [ ] **Step 2: Write the failing tests.** Five behaviours:
  1. At zone 1 the portal's bill is exactly its four `build_cost` lines at
     their authored quantities.
  2. At zone 2 it additionally holds `trace_sniffer` and `cache_grain` at
     their **authored base**, while the sector-1 lines are ramped one step.
  3. At zone 3 it holds `recompile_kernel` at base, the sector-2 lines at one
     step, and the sector-1 lines at two steps.
  4. A structure **without** `zone_portal: true` that authors a
     `zone_build_cost` line gets it appended **unramped** once
     `zone >= min_zone`. Build that def in the test with
     `assets_dir_with_extra_structure` — do not ship one.
  5. A `BuildSite` filed in one zone keeps the price it was filed at across a
     breach. This guarantee exists today; re-assert it because this change
     touches the pricing door.

- [ ] **Step 3: Run them and watch every one fail.**

- [ ] **Step 4: Add the field.** `#[serde(default)] pub zone_build_cost:
      Vec<(u32, ItemId, u32)>`, documented as `(min_zone, item, base_qty)`
      and with *why* it is a separate field: changing `build_cost`'s type
      would touch all 30 shipped files and every mod.

- [ ] **Step 5: Amend `structure_build_cost`.** It stays the one door.

- [ ] **Step 6: Run the tests; all five pass.**

- [ ] **Step 7: Author `portal.ron`.**

```ron
build_cost: [
    ("portal_fragment", 24),
    ("patch_routine",    4),
    ("hardened_shell",   3),
    ("routine_disk",     4),
],
zone_build_cost: [
    (2, "trace_sniffer",    2),
    (2, "cache_grain",     10),
    (3, "recompile_kernel", 3),
],
```

  **Rewrite the `description` too.** It currently reads "Deeper zones cost
  more Portal Fragments to open, and fragments come out of the Stack alone."
  That is now half the truth, and the player reads it on the build menu.

- [ ] **Step 8: Update `assets/structures/README.md`,** documenting
      `zone_build_cost` and the ramp-from-introduction rule beside the
      existing `build_cost` and `zone_portal` entries.

- [ ] **Step 9: `cargo test --workspace`.** Expect fallout in
      `crates/engine/src/tests/assets.rs` and in `tests/research.rs:820`
      (which asserts nothing unlocks the portal — it constrains the *gate*,
      not the *cost*, so it should still pass; if it fails, read why before
      touching it). Read each failure: a census failing because content
      changed gets updated; one failing because a *rule* changed needs the
      rule re-argued, not the assertion relaxed.

- [ ] **Step 10: Seam bookkeeping.** Rule for `CLAUDE.md`'s **The base**
      section, one sentence — shape: *"A zone-portal line is ramped from the
      zone it was introduced in, and `build_cost` is `min_zone: 1`."* Trap
      for `references/base.md`: a line authored for a later sector but ramped
      from zone 1 arrives pre-inflated, and nothing fails to compile.
      Argument for `docs/seams.md`: the composition is constrained by
      research gating — `cache_coherence` (40 RD) and `program_refactoring`
      (75 RD) are both zone 2 — which is the entire reason the field exists
      rather than a longer `build_cost`.

- [ ] **Step 11: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

## Task D: The grid burns Power Cells

**Independent of every other task.**

**Files:**
- Modify: `crates/engine/src/structures.rs` — `StructureDef`
- Modify: `crates/engine/src/components.rs` — new `PowerFuel`
- Modify: `crates/engine/src/game/base/power.rs` — `ledger` (:55),
  `PowerLedger` (:29)
- Modify: `crates/engine/src/systems.rs` — `power_grid_system` (:552)
- Modify: `crates/engine/src/game/base/building.rs` — `spawn_structure` (:246)
- Modify: `crates/engine/src/game/lifecycle.rs` — the structure load path
  (~:1186-1236) and the save path (~:1584-1611)
- Modify: `crates/engine/src/save.rs` — `StructureSave` (:776)
- Modify: `crates/engine/src/tuning.rs`
- Modify: `assets/structures/recharger_node.ron`, `line_driver.ron`,
  `assets/structures/README.md`
- Modify: `docs/seams.md`, `.claude/skills/seams/references/base.md`,
  `CLAUDE.md` (+ `cp CLAUDE.md AGENTS.md`)
- Test: `crates/engine/src/tests/power.rs`, plus a save→load test

**Interfaces — Produces:** `StructureDef::power_upkeep: bool`,
`components::PowerFuel { ticks_left: u32 }`, `tuning::POWER_UPKEEP_TICKS`.
No other task depends on them.

### The five non-obvious things

1. **The Home must never burn.** `home.ron` leaves `power_upkeep` unset, so
   its free 4 is the bootstrap: Home 4 covers a Power Conduit (draw 1) + a
   Mining Node (1) + a Lathe (2) exactly. If the Home burned, a base with no
   Power Cells could never make one.
2. **A Recharger Node has no `MachineStatus` today.** Both writers of a
   structure's component list gate it on `def.runs_a_job()`, which is
   `work.is_some() || assembles.is_some()` — and a Recharger has neither. So
   announcing `Starved` needs the component inserted for `power_upkeep`
   structures **in both places**: `Game::spawn_structure`
   (`game/base/building.rs:246`) and the load path in `game/lifecycle.rs`.
   `CLAUDE.md` names this exact trap — `spawn_structure` is the one place a
   component list is written, and the load path is the hand-written copy that
   drifts with nothing failing to compile.
3. **`set_machine_status` is the one place a stall is announced and it logs
   only on transition** (`crates/engine/src/systems.rs:511`). Call it; do not
   log from the new code. Its `Starved` line already reads *"The {name} is
   starved — nothing is feeding it."*, which is exactly right. **Do not add a
   `MachineStatus` variant** — the enum's matches are exhaustive by design.
4. **There is no shared adjacency-pull helper.** `assembler_system`
   (`systems.rs:1096`) inlines it: it builds a `by_tile: HashMap<(i32,i32),
   Entity>`, walks `crate::game::base::collect::ORTHOGONAL`, plans the takes,
   then applies them. `Game::take_from_adjacent` in
   `game/base/collect.rs:63` is the *player's* collect and works on `&mut
   Game`, so a bevy system cannot use it. **Extract a small helper** — "take
   up to `n` of one item from the orthogonal neighbours of a tile, in
   deterministic order" — and use it from the new upkeep code. Then try to
   use it in `assembler_system`'s inline block too. If the assembler's
   batch planning does not fit it cleanly, **stop and report rather than
   leaving two silent copies of the adjacency rule** — that is the drift
   `ORTHOGONAL`'s own doc comment exists to prevent.
5. **Order inside the schedule is load-bearing.** `power_grid_system` is an
   exclusive system (`world: &mut World`) and is first in a `.chain()` with
   `idle_machine_system` (`game/lifecycle.rs:346-376`); the comment says dark
   must be decided before anything reads or writes `MachineStatus` this tick,
   or a stall double-logs. Spend and refuel **inside `power_grid_system`,
   before it calls `ledger`** — do not add a new system.

- [ ] **Step 1: Read** `crates/engine/src/game/base/power.rs` whole,
      `power_grid_system` and `idle_machine_system`, `set_machine_status`,
      and `assembler_system`'s pull block (~:1109-1215).

- [ ] **Step 2: Write the failing tests** in `tests/power.rs`. Six
      behaviours:
  1. A Recharger Node orthogonally adjacent to a buffer stocked with Power
     Cells stays lit across `POWER_UPKEEP_TICKS * 2 + 1` ticks and has
     consumed exactly 2 cells.
  2. Beside an **empty** buffer it goes dark once its charge runs out:
     `ledger().supply` drops by exactly 4.
  3. That dry supplier is announced `Starved` **once**, not per tick. Per the
     memory note that `message_history` condenses repeats, sum `repeats` or
     the assertion is vacuous.
  4. The Home never consumes and never goes dark, on a base holding zero
     Power Cells anywhere.
  5. The existing `dark` cut still runs on the reduced supply — a machine
     that was lit goes dark when a supplier starves.
  6. Save → load preserves `PowerFuel::ticks_left`. A **save→load** test, not
     a RON round-trip: per the memory note, `#[serde(skip)]` and a missing
     write both leave a round-trip green.

- [ ] **Step 3: Run them and watch every one fail.**

- [ ] **Step 4: Add `tuning::POWER_UPKEEP_TICKS = 20`** in the Production
      chains section, documented with the rate argument: a Power Conduit at
      Mk1 in zone 1 yields one cell per 6 ticks (166 per 1,000 ticks); a
      burning supplier consumes 50 per 1,000; so one Conduit sustains three
      Rechargers while drawing 1 itself and occupying one posted program —
      and the Conduit is on the grid it feeds.

- [ ] **Step 5: Add `#[serde(default)] pub power_upkeep: bool`** to
      `StructureDef` and `components::PowerFuel { ticks_left: u32 }`.

- [ ] **Step 6: Insert `PowerFuel` and `MachineStatus`** for `power_upkeep`
      structures in **both** `spawn_structure` and the load path. New
      structures start at `POWER_UPKEEP_TICKS` (fuelled).

- [ ] **Step 7: Extract the adjacency-pull helper** (see non-obvious thing 4)
      and spend/refuel inside `power_grid_system` before it calls `ledger`.

- [ ] **Step 8: Make `ledger` count a fuelled supplier only.** A structure
      with `power_upkeep` and `ticks_left == 0` contributes 0 supply;
      anything without `power_upkeep` is unchanged. `ledger` has two callers
      — `power_grid_system` and `Game::base_power`, the latter read before
      any tick has run — so the cold state must read correctly.

- [ ] **Step 9: Persist it.** Add the field to `StructureSave` behind
      `#[serde(default = "default_power_fuel")]` returning
      `POWER_UPKEEP_TICKS`, so an existing save's suppliers load **fuelled
      rather than dry**. The pattern to copy is `save.rs:575`'s
      `#[serde(default = "default_worn_quality")]`. Write it on save and
      restore it on load — both paths, or the round trip lies.

- [ ] **Step 10: Run the tests; all six pass.**

- [ ] **Step 11: Author the assets.** `power_upkeep: true` on
      `recharger_node.ron` and `line_driver.ron`. **Rewrite the Recharger
      Node's `description`** — it ends "No worker, no input", which this
      change makes false, and it is what the player reads on the build menu.
      Document `power_upkeep` in `assets/structures/README.md` beside
      `power_supply` (~:226).

- [ ] **Step 12: Add a census** to `crates/engine/src/tests/assets.rs`
      pinning the new axis, in the voice of the neighbouring
      `every_shipped_machine_declares_a_power_draw` (:1445) and
      `no_shipped_structure_both_draws_and_supplies` (:1497): every shipped
      structure that declares `power_upkeep` also declares `power_supply`,
      and the Home declares neither upkeep nor a build cost. A
      `#[serde(default)]` field with no census is a field authored nowhere.

- [ ] **Step 13: `cargo test --workspace`.** Expect fallout wherever a test
      builds a Recharger Node and assumes steady supply — `support.rs`'s
      `stand_ample_grid_supply` is the first place to look. Read before
      editing: a test that now goes dark is telling you the feature works.

- [ ] **Step 14: Seam bookkeeping.** Rule for `CLAUDE.md`'s **The base**
      section — shape: *"A supplier that declares `power_upkeep` supplies
      nothing while dry, and the Home never declares it."* Trap for
      `references/base.md`: the Home burning makes a cold start
      unrecoverable; a supplier has no `MachineStatus` unless both writers
      insert one; a second `ORTHOGONAL` walk is how the pull rule drifts from
      the assembler's. Argument for `docs/seams.md`: the closed loop, the
      rate derivation, and why `Starved` rather than a new variant.

- [ ] **Step 15: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

## Task E: what a dry Recharger owes, and what it burns

**Added after Task D shipped**, from two questions D raised and the user
answered. Runs after B. Touches no file B touches.

**Files:**
- Modify: `crates/engine/src/structures.rs` — `StructureDef::power_upkeep`
- Modify: `crates/engine/src/systems.rs` — `power_regen_system`,
  `burn_grid_upkeep`
- Modify: `crates/engine/src/game/base/power.rs` — `ledger`
- Modify: `assets/structures/recharger_node.ron`, `line_driver.ron`,
  `assets/structures/README.md`
- Modify: `docs/seams.md`, `.claude/skills/seams/references/base.md`,
  `CLAUDE.md` (+ `cp CLAUDE.md AGENTS.md`)
- Test: `crates/engine/src/tests/power.rs`, `crates/engine/src/tests/assets.rs`

### E1 · The fuel is named in data

`power_upkeep: bool` becomes `#[serde(default)] pub power_upkeep:
Option<ItemId>`, authored `Some("power_cell")` on both suppliers. Task D
hardcoded `ids::POWER_CELL` and flagged it: *"`power_upkeep: Option<ItemId>`
would be strictly more moddable at zero extra cost, and I did not take that
decision unilaterally."*

`CLAUDE.md`'s moddability rule is the argument — content is never hardcoded in
Rust when it can be data. Cheapest now, while two shipped files and nothing
external read the field. **No save change**: `power_upkeep` lives on
`StructureDef` (a `.ron` def), not on `StructureSave`.

Everywhere that asks "does this burn?" becomes `power_upkeep.is_some()`;
everywhere that takes a cell takes `power_upkeep.as_ref()`'s item instead of
`ids::POWER_CELL`. The census in `tests/assets.rs` must additionally assert
that a declared fuel **resolves to a real item** — a typo'd id would otherwise
mean a supplier that can never be fed, silently.

### E2 · A dry Recharger stops trickling too

Today `power_regen_system` reads no `PowerFuel`, so a starved Recharger still
trickles Power into the party — Task D flagged this as an open design
question and reported the description accordingly. Gate it: **a supplier with
a fuel it cannot pay does nothing at all**, neither half.

The argument is a standing note in this repo: *Power is not a limiting
resource — the Recharger Node deletes hunger as a cost for 10 fragments.*
The personal trickle is the half the player actually feels, so fuelling the
Grid half alone leaves that untouched. This is what puts the cost back.

**Rewrite the Recharger's `description` a second time.** Task D's current text
is accurate about the Grid half specifically because the trickle was *not*
gated; once both halves are, the sentence should say the building stops.

- [ ] **Step 1: Read** `power_regen_system`, `burn_grid_upkeep` and `ledger`
      as Task D left them, plus `assets/structures/README.md`'s
      `power_upkeep` entry.

- [ ] **Step 2: Write the failing tests.** Four behaviours:
  1. A dry Recharger trickles **no** Power into a party standing in range —
     the player's `PowerReserve` is unchanged across a window.
  2. A fuelled one still trickles exactly as it did before this task. Pin the
     existing rate so E2 cannot silently retune it.
  3. A supplier whose `power_upkeep` names an item **no `ItemDb` entry
     resolves** never burns and never supplies — and the census below is what
     stops one shipping.
  4. The census: every shipped `power_upkeep` names an item that resolves.

- [ ] **Step 3: Run them and watch each fail.** Behaviour 2 will pass before
      the change; prove it real by mutation (break the regen rate and watch
      it fail), and say so in the report.

- [ ] **Step 4: Flip the field to `Option<ItemId>`** and update both suppliers
      and the README.

- [ ] **Step 5: Gate `power_regen_system`** on the same `PowerFuel` the ledger
      reads. Do not duplicate the "is it paying?" test — if Task D left it
      inline in two places, extract the predicate so E does not make a third.

- [ ] **Step 6: Rewrite the Recharger's description.**

- [ ] **Step 7: `cargo test --workspace`.** Expect fallout in the fixtures
      Task D already had to touch — `tests/support.rs`'s
      `stand_ample_grid_supply` and `dev-saves/chains.ron` — if either relies
      on a trickle that now stops. Read `dev-saves/README.md` before touching
      the template; Task D rebuilt its supplier bank and a Depot boxed in on
      all four sides is unreachable, which reports as `Stranded` on some
      *other* machine's worker.

- [ ] **Step 8: Seam bookkeeping**, amending rather than adding: Task D's
      rule in `CLAUDE.md` says the Grid half. Correct it to say the building.

- [ ] **Step 9: `cargo fmt`, `cargo clippy --workspace`, commit.**

---

## After all five

The orchestrator handles this; it is written down so nothing is dropped.

- [ ] Whole-branch review on opus, with the diff handed over **as a file**,
      never pasted into the prompt.
- [ ] `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt --check`
- [ ] `cargo test -p feral-processes-engine balance_sim` — it models no
      crafting, no portal and no power, so it gates none of this; run it to
      confirm the curves have **not** moved.
- [ ] Play it. Nothing in this change has been observed in a session, and
      `docs/measurements/` has no entry for base throughput. `cargo run --
      --template chains` opens on a running base.
- [ ] Version bump, `CHANGELOG.md` section, annotated tag — once, at the
      merge. Which digit moves is decided by `CHANGELOG.md`'s preamble:
      "breaking" means a player's save stops loading, and no save format
      changed here.
