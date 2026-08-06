# Banked Resources Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Make Research Data stop behaving like cargo — no collect trip, no
inventory row, not sellable — by turning `ItemDef::bank_limit` into
`ItemDef::banked` and giving that flag two new consequences.

**Architecture:** `banked` is a data flag on the item, not a Rust special
case, so a mod's second bank currency inherits every rule for free. Research
Data stays an `Inventory` entry keyed by `ItemId`, so `unlock_research` is
untouched and there is **no save-format change**. The cap is deleted, which
removes the only job that read the number and takes `add_capped`, `has_room`,
`check_room` and `grant_loot`'s overflow branch with it.

**Spec:** `docs/superpowers/specs/2026-08-05-banked-resources-design.md`

**Tech Stack:** Rust, `bevy_ecs` 0.19, RON assets.

## Global Constraints

- **No `SAVE_FORMAT_VERSION` bump.** Research Data remains an `Inventory`
  entry. If a task seems to need a bump, the design has been misread — stop
  and flag it.
- **`banked` is `#[serde(default)]`** (defaulting to `false`), per the
  schema rule in `CLAUDE.md` — an existing mod item file must keep parsing
  untouched.
- **A malformed `.ron` still warns and skips, never panics.** Don't disturb
  `ItemDb::load_dir`'s error handling.
- **Delete unreachable code rather than leaving it always-`Ok`.** No
  `// removed` comments, no shims (`CLAUDE.md`, "No backwards-compat cruft").
- **Comments explain why.** Several comments in the touched files are
  load-bearing arguments; where an argument stops being true, rewrite the
  argument — do not just reword the sentence.
- **Player-facing text says "sweep" not "raid" etc.** — not relevant here,
  but player-facing research text must not gain occult vocabulary.
- Gates after every task: `cargo test --workspace`, `cargo clippy
  --workspace`, `cargo fmt`.

---

### Task 1: `banked` replaces `bank_limit`, and the cap goes

The flag survives; the number does not. Nothing about *where* a payout lands
changes in this task — this is purely the field swap plus the removal of the
machinery that read its value.

**Files:**
- Modify: `crates/engine/src/items_db.rs` — `ItemDef::bank_limit:
  Option<u32>` → `banked: bool`
- Modify: `crates/engine/src/components.rs` — `cargo_used` reads `banked`;
  **delete** `add_capped` and `has_room`
- Modify: `crates/engine/src/systems.rs:187` — `resolve_gather_cycle`'s
  payout branch reads `banked`
- Modify: `crates/engine/src/game/catalog.rs` — **delete** `bank_limit_of`
  and `check_room`
- Modify: `crates/engine/src/game/turn.rs` — `grant_loot` loses its
  `added < qty` branch and the `"Research bank"`/`"Buffer"` label; its
  `add_capped` call becomes `add`
- Modify: `crates/engine/src/game/collect.rs:68` — `add_capped` → `add`;
  the doc comment's "what doesn't fit stays in the buffer" argument is now
  vacuous and must go
- Modify: `crates/engine/src/game/trade.rs` — remove 4 `check_room` calls
- Modify: `crates/engine/src/game/crafting.rs` — remove 2 `check_room` calls
- Modify: `crates/gui/src/render/progression.rs:51-54` — drop to
  `"Research Data: {held}"`
- Modify: `assets/items/research_data.ron` — `bank_limit: Some(200)` →
  `banked: true`; the description's "up to 200 of it rides free of your
  cargo cap" is now wrong
- Modify: `assets/items/README.md` — document `banked`, remove `bank_limit`

**Interfaces:**
- Consumes: nothing.
- Produces: `ItemDef::banked: bool`. `Inventory::add(item, qty)` is the only
  add path. `Inventory::cargo_used(&ItemDb) -> u32` unchanged in signature.

- [ ] **Step 1: Write the failing tests**

  Rework in `crates/engine/src/items_db.rs` tests: the shipped-assets
  assertion `bank_limit == Some(200)` becomes `banked == true`, and the
  "only Research Data is banked" census re-keys its filter to `banked`.
  That census is the guard that a second banked item does not silently
  widen the cargo buffer — keep its comment's argument intact.

  In `crates/engine/src/components.rs` tests: keep
  `cargo_used_ignores_banked_currency` (it is the surviving proof the flag
  still does job 1). **Delete** `add_capped_never_caps_ordinary_cargo`,
  `add_capped_measures_banked_currency_against_its_own_limit`,
  `add_capped_clamps_research_data_at_its_bank_limit`,
  `has_room_is_unbounded_for_cargo_but_bounded_for_banked`, and the
  `research_data_bank_limit` helper — all four test a cap that no longer
  exists.

  In `crates/engine/src/tests/building.rs`,
  `a_banked_resource_never_scales_with_zone_depth` keeps its assertion and
  gets a **new argument**. Its current comment reasons "scaling it would
  fill the bank in ~13 cycles", which is made entirely of the deleted cap.
  The reason the behaviour still holds: the research tree is a fixed ladder
  whose deepest node costs 45 (`cortex`), so a payout doubling per zone
  would collapse the tree rather than accelerate it.

  In `crates/engine/src/tests/trade.rs:146`, **delete** the test guarding
  `check_room`-before-despawn ordering in `sell_companion`. The ordering it
  protects ceases to exist with `check_room`, and a test asserting a
  vacuous property is worse than none.

- [ ] **Step 2: Run and watch them fail**

  `cargo test -p feral-processes-engine` — expect compile errors on the
  renamed field. That *is* the failure; a field rename fails at compile
  time, not at assertion time.

- [ ] **Step 3: Make the change**

  Work outward from `ItemDef`. The compiler names every site — there are
  four `.is_some()` readers and only one that used the value. Two of the
  readers (`cargo_used`, `resolve_gather_cycle`) keep their behaviour
  exactly; the third (`grant_loot`'s label) and the fourth (the cap trio)
  are deleted.

  The non-obvious part is `Inventory::add_capped`'s doc comment, which
  documents a real edge case that dies with it: "Holding more than a bank's
  ceiling is legal (a save predating the cap...)". With no ceiling, a legacy
  save holding 400 Research Data is simply correct and needs no clamp on
  load. Confirm `Game::load` does not clamp it (unlike `ItemFusions`, which
  does clamp to `MAX_FUSIONS` and must keep doing so).

- [ ] **Step 4: Run the tests**

  `cargo test --workspace`. Also `cargo test -p feral-processes-engine
  balance_sim` — `resolve_gather_cycle` is on the payout path, and the curve
  tests are the regression gate for it.

- [ ] **Step 5: Commit**

  `refactor(items): bank_limit becomes banked, and the cap goes`

---

### Task 2: A banked payout is delivered to the bank, not the buffer

**Files:**
- Modify: `crates/engine/src/systems.rs` — extract the shared delivery tail
  from `task_progress_system` (~line 372) and `player_gather_system`
  (~line 467); both currently repeat `payout.min(stock.output_room())` then
  write `stock.output`
- Test: `crates/engine/src/tests/research.rs`

**Interfaces:**
- Consumes: `ItemDef::banked` from Task 1.
- Produces: one `pub(crate)` delivery helper in `systems.rs` taking the
  resolved `(ItemId, u32)`, the machine's `&mut Stock`, the `&ItemDb`, and
  the player's `&mut Inventory`, returning the units that landed (for the
  `"You extract N"` log line the player-gather path prints). Name it for
  what it decides — where a payout lands — not for who calls it.

- [ ] **Step 1: Write the failing tests**

  Four, in `crates/engine/src/tests/research.rs`. Use
  `support::work_node_parts()` for any hand-spawned node — a node short of
  `Stock` or `MachineStatus` is silently skipped by the query and reads as
  a payout curve that moved.

  1. A staffed Research Node's payout lands in the player's `Inventory` and
     leaves the node's `Stock.output` **empty**. The existing
     `a_research_cronjob_banks_research_data_over_time` asserts the
     opposite (it checks `node_output` grows) and must be inverted, not
     added to.
  2. The same for the player-gather path, so the two delivery callers
     cannot diverge. This is the whole reason the tail is extracted; a test
     covering only one caller would not notice a copy.
  3. A **non**-banked node (a Mining Node producing Core Fragments) still
     fills its `Stock.output` and still requires `collect_adjacent`. This
     is the regression that matters — the change must be invisible to every
     other machine in the game.
  4. A banked payout still lands while `Locale::Stack` is live. Delivery
     touches `Inventory` and never `Position`, so this should pass on the
     first try; it is here to pin the property, since a later refactor
     reaching for the player's `Position` would break it silently.

- [ ] **Step 2: Run and watch them fail**

  `cargo test -p feral-processes-engine research`

- [ ] **Step 3: Implement**

  Extract the duplicated tail into one helper, then branch inside it on
  `banked`. Do **not** branch at the two call sites — the duplication is
  precisely where a banked rule would drift, and `player_gather_system`'s
  own doc already claims the two paths "produce identical output from the
  same node".

  Leave the `output_room() == 0` clog check where it is, above
  `resolve_gather_cycle`. It runs before the item is known, and for a
  banked producer `output_room()` is always non-zero because nothing ever
  accumulates there — so it passes harmlessly and needs no reordering.

  Delivery is **silent**: no log line per unit on the cronjob path. A
  Research Node pays every 14 ticks, and a line per payout would flood the
  base feed — the same argument `set_machine_status` makes about stalls.
  The player-gather path keeps its existing `"You extract N"` line, because
  that one is the player's own action and fires only when they ran the job.

- [ ] **Step 4: Fix the two doc comments whose arguments are now false**

  Not cosmetic — both are load-bearing arguments a future reader would rely
  on:
  - `task_progress_system`: "a node paying straight into the player's
    pocket is an infinite source" was the reason the buffer paces
    everything. A banked node now *is* such a source, and that is fine
    because the research tree is finite — once every node is bought, more
    Research Data does nothing. Say that.
  - `player_gather_system`: "The payout lands in the node's own buffer here
    too... the player is standing beside the node, so it is one `c` away."
    That argument does not apply to something never collected.
  - `produced_item`'s "the whole answer to could this structure feed a
    neighbour" gets *truer* and should say so: a banked item never reaches
    an `output`, so a bank can never feed a chain.

- [ ] **Step 5: Run the tests and commit**

  `cargo test --workspace`, then
  `feat(research): bank a banked payout instead of buffering it`

---

### Task 3: Banked is invisible — not a row, not a good

**Files:**
- Modify: `crates/engine/src/game/party.rs:8` — `player_status()` filters
  banked items out of `PlayerStatus::inventory`
- Modify: `crates/engine/src/views.rs:60-65` — the field's doc comment
  currently explains why `inventory_used` won't match the sum of
  `inventory`; that reason changes
- Modify: `crates/engine/src/game/catalog.rs` — add the accessor for a
  banked balance
- Modify: `crates/engine/src/game/trade.rs` — `sell_item` refuses a banked
  item, beside the existing trade-currency refusal
- Modify: `crates/gui/src/render/progression.rs:44-50` — read `held` from
  the new accessor instead of from `player_status().inventory`
- Modify: `CHANGELOG.md`
- Test: `crates/engine/src/tests/trade.rs`,
  `crates/app-core/src/tests/inventory.rs`

**Interfaces:**
- Consumes: `ItemDef::banked` from Task 1.
- Produces: a `pub fn` on `Game` returning the player's held quantity of a
  given banked item as `u32`. The research screen is its only caller.

- [ ] **Step 1: Write the failing tests**

  1. Research Data is absent from `PlayerStatus::inventory` while a
     non-banked item in the same inventory is present.
  2. `Game::sell_item` refuses a banked item with an explicit message, and
     the player still holds it afterwards. Mirror the shape of the existing
     trade-currency refusal test.
  3. In `crates/app-core/src/tests/inventory.rs`: the inventory screen
     lists no row for Research Data even when the player holds some.
  4. The new accessor returns the held amount for a player holding Research
     Data — the research screen's number must survive the filter that hides
     the row.

- [ ] **Step 2: Run and watch them fail**

  `cargo test -p feral-processes-engine trade` and
  `cargo test -p feral-processes-app-core inventory`

- [ ] **Step 3: Implement**

  The filter belongs in `player_status()` — one place, and it makes every
  one of that field's twelve-plus consumers correct at once. Notably
  **`crates/app-core/src/app/trade.rs` needs no edit**: its sell list is
  built from `player_status().inventory`, so filtering at the source is
  what keeps the app-core list and the gui list indexed identically. That
  co-indexing is called out in a comment at `app/trade.rs:88-95` and
  breaking it would sell the row above or below the highlighted one.

  `sell_item`'s refusal is still required even though nothing can currently
  reach it through the menu — it is the engine-level rule, and the menu
  filter is a consequence of it rather than a substitute.

- [ ] **Step 4: Check `cost_display`'s callers**

  `render/crafting.rs` and `render/building.rs` pass `&status.inventory` to
  `cost_display` to show have/need for recipe ingredients. Confirm no
  shipped recipe or `build_cost` names a banked item — none should, since
  research is spent through `unlock_research` rather than a recipe. If one
  does, it would now display as 0 held. Record the finding either way; "a
  banked item cannot be a craft ingredient or a build cost" is a real
  consequence of this design and belongs in the commit message.

- [ ] **Step 5: Run the tests and commit**

  `cargo test --workspace`, then
  `feat(items): a banked item is neither an inventory row nor a good`

---

## Final gates

- [ ] `cargo test --workspace` — green
- [ ] `cargo test -p feral-processes-engine balance_sim` — the payout curve
      was touched; a moved curve is the signal, not a broken test
- [ ] `cargo clippy --workspace` — no new warnings
- [ ] `cargo fmt`
- [ ] `rg -n "bank_limit" crates/ assets/ docs/` returns nothing but this
      plan and the spec. Per the standing rename rule, also grep the **new**
      vocabulary (`banked`) to catch anything half-converted, and don't
      restrict to `--type rust` — `.ron` and README text are in scope.
- [ ] **Play it.** `cargo run -- --template extraction` is the closest
      shipped template to a working base. A green suite says nothing about
      whether a research economy with no collect trip, no inventory row and
      no per-unit log line still reads as something you are earning. This is
      the gate most likely to send the design back.

## Documentation obligations

- `assets/items/README.md` — Task 1 (schema change)
- `CHANGELOG.md` — Task 3
- **Not** `docs/manual.md`, **not** root `README.md` — both are under
  standing carve-outs.
