# Depot Deposit Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the player move plain cargo out of `Inventory` into an adjacent Depot's `Stock::output`, through a picker that mirrors the collect screen.

**Architecture:** A new engine module `game/base/deposit.rs` mirroring `collect.rs` function for function. In app-core the two pickers share one key table (`app/basket.rs`); the only difference between them — per-row ceilings versus one shared depot-room budget — is carried as an `Option<u32>` field rather than as a second copy of the handler. gui gets a `render/deposit.rs` mirroring `render/collect.rs`.

**Tech Stack:** Rust, `bevy_ecs` (engine only), RON assets. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-08-23-depot-deposit-design.md` — read it before Task 1. It carries the four settled choices and the argument for each; this plan does not repeat them.

## Global Constraints

- **No schema change and no save-format change.** `Stock`, `Inventory` and `StructureDef` are untouched. Do not bump `SAVE_FORMAT_VERSION`.
- **No version bump and no `CHANGELOG.md` section on this branch.** Both happen once, at the merge — commits on a branch stay unversioned.
- **Every new test is mutation-proved.** Delete or invert the fix, run the test, confirm it fails, restore, confirm it passes. Record the mutation in the commit body. A test that passes with the fix removed is not coverage.
- **Gates after every task:** `cargo fmt`, `cargo clippy --workspace` (fix warnings, never silence), and the task's own targeted `cargo test -p <crate> <name>`. `cargo test --workspace` is the final gate in Task 6 only.
- **Check `git branch --show-current` before every commit.** A concurrent session has fast-forwarded and deleted a branch mid-task in this repo before. Expected branch: `feat/depot-deposit`.
- **Never `git add -A`.** Stage explicit paths.
- **Comments explain why, never what.** Match the density and voice of the module you are editing — `collect.rs` and `hauling.rs` are the register to aim for.
- **Player-facing words follow the vocabulary**: the code's word is "deposit", the player's word is "put away". No occult naming.
- **Do not touch `docs/manual.md` or the root `README.md`.** Both are explicitly carved out of the documentation obligation. `CHANGELOG.md`, the `assets/*/README.md` schema docs and `docs/*-gen.py` still apply.
- **Do not write to `TODO.md`.** It is the user's own list; findings go to `docs/`, the changelog, or nowhere.

---

### Task 1: Engine — what may be deposited, and where

**Files:**
- Create: `crates/engine/src/game/base/deposit.rs`
- Modify: `crates/engine/src/game/base/mod.rs` — add `mod deposit;` in alphabetical position
- Create: `crates/engine/src/tests/deposit.rs`
- Modify: `crates/engine/src/tests/mod.rs` — add `mod deposit;` in alphabetical position

**Interfaces:**
- Consumes: `Game::adjacent_stock()` (`game/base/collect.rs`, `pub(crate)`), `Game::require_base()`, `Game::base_pos()`, `components::Inventory::items`, `components::Stock::output_room()`, `structures::StructureDef::stores`, `items_db::ItemDef::banked`.
- Produces, all on `impl Game`:
  - `pub(crate) fn adjacent_depots(&self) -> Vec<Entity>`
  - `pub fn depositable(&self) -> Vec<(ItemId, u32)>`
  - `pub fn deposit_room(&self) -> u32`

- [ ] **Step 1: Read the mirror**

Read `crates/engine/src/game/base/collect.rs` end to end. Every function in this task is a reflection of one in it, and the doc comments there state rules this module inherits — the `(x, y)` sort's reason, the guard set, why `&self` readers need no `require_surface`.

- [ ] **Step 2: Write the failing tests**

In `crates/engine/src/tests/deposit.rs`. Use `crates/engine/src/tests/support.rs` fixtures — read it before writing a new one. Test intents:

1. `depositable` lists the plain `Inventory` rows when the party stands beside a Depot.
2. It **excludes banked items** — stock the player with `research_data` (the only shipped banked item) and assert it is absent. This is the trap: `PlayerStatus::inventory` filters it and this list must too.
3. It excludes `GearCopies` entirely — a rare or fused copy in the pack is not offered.
4. Rows come back **sorted by `ItemId`**. `Inventory::items` is a `Vec` in insertion order, so add items in reverse-alphabetical order and assert alphabetical output. Without this the rows appear in pickup order.
5. It is **empty beside a machine that has a `Stock` but no `stores`** — a Mining Node. This is the whole difference from `collectable_adjacent`; without it the feature accepts any buffer.
6. It is empty with nothing adjacent, during a battle, on game over, and when `require_base` fails (on the surface, and underground).
7. `adjacent_depots` returns two adjacent Depots in `(x, y)` order, with the fixture **spawning them in the opposite order to their positions**. Copy `assembler_system`'s test trick — it is the only way the sort is load-bearing rather than incidental.
8. `deposit_room` sums `output_room()` across adjacent Depots, and drops as a Depot fills.

- [ ] **Step 3: Run them and watch them fail**

`cargo test -p feral-processes-engine deposit` — expect failures naming the missing functions.

- [ ] **Step 4: Implement**

Three functions. `adjacent_depots` is `adjacent_stock()` filtered by `StructureDef::stores`, preserving its order — filter, do not re-sort. `depositable` reads `Inventory`, applies the banked filter with the idiom `game/party.rs` already uses (`!db.get(item.as_str()).is_some_and(|d| d.banked)`), sorts by `ItemId`, and holds the same guards `collectable_adjacent` does — including returning empty when `adjacent_depots()` is empty. `deposit_room` sums `output_room()`.

- [ ] **Step 5: Run them and watch them pass, then prove them**

`cargo test -p feral-processes-engine deposit`. Then mutation-prove each: drop the banked filter (test 2 must fail), drop the `stores` filter (test 5 must fail), drop the sort (test 4 must fail), reverse the `adjacent_depots` order (test 7 must fail). Restore after each.

- [ ] **Step 6: Gates and commit**

`cargo fmt`, `cargo clippy --workspace`. Stage the four explicit paths; commit `feat(engine): what may be put into a Depot, and where` with the mutation table in the body.

---

### Task 2: Engine — the giving path

**Files:**
- Modify: `crates/engine/src/game/base/deposit.rs`
- Modify: `crates/engine/src/tests/deposit.rs`

**Interfaces:**
- Consumes: Task 1's three functions; `components::Inventory::take(item, qty) -> u32`; `components::Stock::output` and `output_room()`; `Game::log_base`; `Game::tick`; `Game::item_name`.
- Produces:
  - `pub fn deposit_items(&mut self, give: &[(ItemId, u32)]) -> Vec<(ItemId, u32)>`
  - `pub fn deposit_adjacent(&mut self) -> Vec<(ItemId, u32)>`

- [ ] **Step 1: Write the failing tests**

Test intents:

1. Deposited goods land in the Depot's `output`, and are then visible to `Game::base_holding` **and** to `Game::collectable_adjacent`. This is the point of the feature — assert it end to end, not just that the buffer changed.
2. An over-ask against the pack is clamped to what is held, and the return value reports what landed rather than what was asked for.
3. An over-ask against room is clamped to what fits. Assert `output_used() <= capacity` — `capacity` must never be exceeded.
4. Two adjacent Depots fill in `(x, y)` order: a basket larger than the first Depot's room spills into the second, and the fixture spawns them in the opposite order to their positions.
5. A full Depot takes nothing, returns empty, and spends **no** tick.
6. One tick per non-empty commit. An empty basket and an all-zero basket each spend none and say nothing.
7. Exactly one log line per commit, naming what actually landed.
8. `deposit_adjacent` with no adjacent Depot logs *"There is nowhere here to put anything."* and spends no tick.
9. `deposit_adjacent` beside a Depot with an empty pack logs *"You have nothing to put away."* and spends no tick.
10. Both refusals are **silent** during a battle, on game over, on the surface, and underground — nothing logged, no tick.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine deposit`.

- [ ] **Step 3: Implement `deposit_items`**

The one thing worth spelling out is the fill loop, because the two clamps compose and the order matters. Per requested `(item, qty)`, walk `adjacent_depots()` in order:

```rust
// Take from the pack FIRST and only what a Depot will actually accept, or a
// full base silently eats the player's cargo. `Inventory::take` already
// clamps to what is held and drops a slot that reaches zero, so its return
// value IS the pack-side clamp — there is no second check in front of it.
let room = stock.output_room();
if room == 0 {
    continue;
}
let moved = self.world.get_mut::<Inventory>(player).unwrap()
    .take(item.clone(), outstanding.min(room));
```

Then add `moved` to that Depot's `output` and to the running total. Never write past `capacity`: that is `hauling::deposit`'s rule and a full Depot is a decided failure mode, not an exception to one.

One `log_base` line and one `tick()` for the whole basket, both skipped when nothing landed. `MessageKind::Loot` is wrong here — nothing was looted — so use the plain `log_base`.

- [ ] **Step 4: Implement `deposit_adjacent`**

Everything `depositable()` offers, through `deposit_items`. Sequence the offer into a local first: `deposit_items(&self.depositable())` borrows `self` both ways at once. The two refusal sentences live **here and nowhere else** — app-core must never grow a copy. Guards come first and refuse silently.

- [ ] **Step 5: Run, pass, prove**

Mutation-prove: remove the `room` clamp (test 3 must fail on capacity), take from the pack before checking room (test 3 or 5 must fail — cargo vanishes into a full Depot), return the requested figure instead of what landed (test 2 must fail), move the tick above the empty check (test 6 must fail), swap the two refusal sentences (tests 8 and 9 must fail).

- [ ] **Step 6: Gates and commit**

`cargo fmt`, `cargo clippy --workspace`, `cargo test -p feral-processes-engine deposit`. Commit `feat(engine): put cargo into an adjacent Depot` with the mutation table.

---

### Task 3: app-core — one basket picker, two screens

**A pure refactor. No behaviour changes, no new mode.** Every existing collect test must stay green without being edited except for the field renames.

**Files:**
- Create: `crates/app-core/src/app/basket.rs`
- Modify: `crates/app-core/src/app/collect.rs` — shrinks to its two ends
- Modify: `crates/app-core/src/app/mod.rs` — add `mod basket;` in alphabetical position
- Modify: `crates/app-core/src/lib.rs` — field renames and the new field (near line 1556)
- Modify: `crates/app-core/src/app/input.rs:129,143` — the modifier fold and the dispatch arm
- Modify: `crates/gui/src/render/mod.rs:652-655` — the renamed fields
- Modify: `crates/app-core/src/tests/collect.rs` — field renames only

**Interfaces:**
- Consumes: `App::menu_selected`, `App::scroll(key, len)`, `App::mode`.
- Produces, on `impl App`:
  - fields `basket_rows: Vec<(ItemId, u32)>`, `basket_amounts: Vec<u32>`, `basket_room: Option<u32>` (replacing `collect_rows` and `collect_basket`)
  - `pub(crate) fn open_basket(&mut self, rows: Vec<(ItemId, u32)>, room: Option<u32>, mode: Mode)`
  - `pub(crate) fn handle_basket_key(&mut self, key: GameKey)`
  - `pub(crate) fn basket_available(&self, row: usize) -> u32`
  - `pub(crate) fn basket_request(&self) -> Vec<(ItemId, u32)>`
  - `pub(crate) fn leave_basket(&mut self)`

The decision behind this shape is recorded in the spec's "The extraction" section: two copies of the key table would drift, and the inverted Left/Right has exactly one test naming it as specification.

- [ ] **Step 1: Move the key table**

Move `handle_collect_key`, `edit_row`, `leave_collect` from `collect.rs` into `basket.rs` as `handle_basket_key`, `edit_row`, `leave_basket`. **Carry the doc comments across intact** — they hold the reasons for the inverted Left/Right, the `div_ceil` termination and the saturating digits. Do not paraphrase them.

`handle_basket_key`'s Enter arm branches on `self.mode` to call the owning screen's commit — two arms, explicit.

- [ ] **Step 2: Make the ceiling a value**

`edit_row` now takes its available from `basket_available(row)` rather than from the row's own quantity. That one function is the whole axis:

```rust
// `None` room is collect: each row's ceiling is its own shelf. `Some(r)` is
// deposit: one budget shared across every row, so filling one lowers the
// rest. Subtracting only the OTHER rows is what lets the highlighted row
// keep its own amount while it is being edited — counting itself would make
// every key a no-op the moment the basket reached the budget.
let taken: u32 = self.basket_amounts.iter().sum();
let others = taken.saturating_sub(self.basket_amounts.get(row).copied().unwrap_or(0));
let budget = self.basket_room.map_or(u32::MAX, |r| r.saturating_sub(others));
self.basket_rows.get(row).map_or(0, |(_, qty)| *qty).min(budget)
```

- [ ] **Step 3: Rewire collect**

`open_collect` becomes a call to `open_basket(offer, None, Mode::Collect)`. `commit_collect` stays in `collect.rs`, reading `basket_request()`. `leave_collect` is gone — `leave_basket` clears all three fields.

- [ ] **Step 4: Rename the fields everywhere**

`collect_rows` → `basket_rows`, `collect_basket` → `basket_amounts`, plus the new `basket_room`. Update the doc comments on the fields: they currently say "what the adjacent machines are offering", which is now one of two things. Keep the snapshot-not-re-derived argument — it applies to both screens.

- [ ] **Step 5: Run the existing suite**

`cargo test -p feral-processes-app-core collect` and `cargo test -p feral-processes-gui`. Every collect test must pass **unedited apart from the renames**. A test that needed its assertions changed means the refactor changed behaviour — stop and find out why.

- [ ] **Step 6: Prove the refactor is inert**

Run `cargo test -p feral-processes-app-core` and `cargo test -p feral-processes-engine`. Then confirm the axis is live: temporarily set `basket_room` to `Some(0)` in `open_collect` and check a collect test fails; restore.

- [ ] **Step 7: Gates and commit**

`cargo fmt`, `cargo clippy --workspace`. Commit `refactor(app-core): one basket picker behind both screens` — state in the body that no behaviour changed and the collect tests are unedited apart from renames.

---

### Task 4: app-core — `Mode::Deposit` and the `P` key

**Files:**
- Create: `crates/app-core/src/app/deposit.rs`
- Modify: `crates/app-core/src/app/mod.rs` — `mod deposit;`
- Modify: `crates/app-core/src/lib.rs` — `Mode::Deposit` variant, and the map-mode list near line 1219
- Modify: `crates/app-core/src/app/input.rs` — widen the modifier-fold condition, add the dispatch arm
- Modify: `crates/app-core/src/app/playing.rs` — the `P` key, in the surface-only block beside `c`
- Create: `crates/app-core/src/tests/deposit.rs`
- Modify: `crates/app-core/src/tests/mod.rs` — `mod deposit;`

**Interfaces:**
- Consumes: Task 2's `Game::depositable`, `Game::deposit_room`, `Game::deposit_items`, `Game::deposit_adjacent`; Task 3's `open_basket`, `basket_request`, `leave_basket`, `handle_basket_key`.
- Produces: `pub(crate) fn open_deposit(&mut self, offer: Vec<(ItemId, u32)>, room: u32)`, `fn commit_deposit(&mut self)`, `Mode::Deposit`.

- [ ] **Step 1: Write the failing tests**

In `crates/app-core/src/tests/deposit.rs`. Read `crates/app-core/src/tests/collect.rs` first — the fixtures and the key-driving idiom are there. Test intents:

1. **The shared budget.** With a Depot whose room is smaller than the pack, filling row 1 lowers row 2's ceiling, and **no sequence of keys can push the basket total past `deposit_room`**. Drive the real keys — this is a key-handling invariant, so pressing keys is the honest test. Include `A` (take-all) against a budget smaller than the pack.
2. Left adds and Right removes, saturating at both ends. The inversion is the specification.
3. `ShiftLeft` is idempotent under key repeat; `CtrlLeft` closes half the gap and terminates on a gap of one — pin it on a shelf of 8, where ceil and floor agree until the last press.
4. Digits accumulate as `n * 10 + d` and clamp at the budget rather than overflowing.
5. `P` beside a Depot with a non-empty pack opens `Mode::Deposit` and spends **no** tick — opening a screen is not an action.
6. `P` with nothing depositable routes back through `Game::deposit_adjacent`, so the engine speaks its own refusal; app-core sets no `status_line`. Assert on the **log**, not on a status line: a copy of an engine message here reads as the key doing nothing.
7. `P` is refused with no tick underground and on the open grid.
8. Commit sends exactly the non-zero rows and clears all three fields; a reopened screen shows no stale pack.
9. An all-zero basket never reaches the engine.
10. A modified arrow in any mode **other than** Collect and Deposit still folds to a bare arrow — pin `Mode::Playing` so `ShiftLeft` still walks west.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-app-core deposit`.

- [ ] **Step 3: Implement**

`open_deposit` calls `open_basket(offer, Some(room), Mode::Deposit)`. `commit_deposit` hands `basket_request()` to `Game::deposit_items` and then `leave_basket()` — no `status_line`, because the engine has logged it and the log pane is where the base reports what it received. An all-zero basket never reaches the engine, so the no-op stays true in one place rather than two.

The `P` handler mirrors `c` exactly: ask `depositable()`, and if empty call `deposit_adjacent()` straight back through the engine; otherwise hand the offer and the room out past the `self.game` borrow the way `c` hands out `opening`.

Widen `input.rs:129` to `_ if matches!(self.mode, Mode::Collect | Mode::Deposit) => key`. The existing comment there already anticipates this — *"A second screen wanting a modifier widens this one condition"* — so extend it rather than replacing it.

Classify `Mode::Deposit` in `Mode::is_battle` as `false`, beside `Mode::Collect`, carrying the same note: opened from the map only, so it never layers over a fight, and the engine refuses a deposit mid-battle anyway.

That match is **exhaustive on purpose** — the doc comment records that it began as an inline `matches!` and fell behind three times, each time silently wiping the battle ghost bars mid-animation. So the compiler, not a test, is what forces the classification: the new variant will not build until it is listed. Getting it wrong in the other direction (classifying it `true`) would keep the battle reveal paced while the player is standing at a Depot, which is the failure the seam names.

- [ ] **Step 4: Run, pass, prove**

Mutation-prove: subtract the whole basket instead of the other rows in `basket_available` (test 1's edit-in-place case must fail), drop `Mode::Deposit` from the fold condition (test 3 must fail — the modifiers go dead), set a `status_line` in the empty-pack branch (test 6 must fail), and drop the `open_deposit` room argument to `None` (test 1 must fail — the budget stops binding). `Mode::is_battle` needs no mutation: it is exhaustive, so omitting the variant does not compile.

- [ ] **Step 5: Gates and commit**

`cargo fmt`, `cargo clippy --workspace`, `cargo test -p feral-processes-app-core`. Commit `feat(app-core): the deposit picker` with the mutation table.

---

### Task 5: gui — the deposit screen

**Files:**
- Create: `crates/gui/src/render/deposit.rs`
- Modify: `crates/gui/src/render/mod.rs` — `mod deposit;`, the `use`, and a `Mode::Deposit` draw arm beside `Mode::Collect`'s
- Modify: `assets/structures/depot.ron` — the description
- Modify: `assets/help/20-controls.md` — the `P` key

**Interfaces:**
- Consumes: `App::basket_rows`, `App::basket_amounts`, `App::basket_room`, `App::menu_selected`; `super::popup::*`; `Painter`.
- Produces: `pub(super) fn draw_deposit(game, rows, amounts, room, selected, painter, m)`.

- [ ] **Step 1: Read the mirror**

Read `crates/gui/src/render/collect.rs` end to end, doc comments included. They state four rules this screen inherits: no shortcut lead (a digit is a quantity, so advertising `[1]` would be a menu lying about its own keys), figures in the **suffix column** rather than `format!`ed into the name, hint lines naming both arrows and both modifiers, and a width census but no height one.

- [ ] **Step 2: Write the failing width census**

Mirror `collect.rs`'s census over the widest shipped item name plus the deposit suffix at `u32::MAX`. Keep its two assertions — the `label > 0.0` guard is what stops the census silently measuring nothing.

The page needs **no height census**: the rows are `Row::Item` spans, so `popup_layout` keeps the selected row visible and a long pack scrolls. `draw_row` clips vertically only, which is exactly why width is the axis that fails in silence.

- [ ] **Step 3: Implement the screen**

Two differences from collect, both from the shared budget:

- A header line for remaining room. The ceiling is shared and otherwise invisible, and the player needs to see *why* a row stopped rising.
- The suffix reads `given / available`, where available is the live shared-budget figure and so moves as other rows fill. That movement is the feature.

Draw through `Painter` only, and take the origin from the caller's `Rect` — a literal `0.0` draws under the stock strip and no test sees it.

- [ ] **Step 4: Run and prove**

`cargo test -p feral-processes-gui`. Mutation-prove the census by widening the suffix format until it overflows, confirming the test fails, then restoring.

- [ ] **Step 5: The two asset edits**

`assets/structures/depot.ron`'s description currently reads *"Collect from it with c."* — it now states both halves. `assets/help/20-controls.md` gains the `P` key beside `c`. Both are player-facing: the code's word is deposit, the player's is "put away".

- [ ] **Step 6: Gates and commit**

`cargo fmt`, `cargo clippy --workspace`, `cargo test -p feral-processes-gui`. Note that an asset edit can move balance censuses — run `cargo test -p feral-processes-engine assets` too. Commit `feat(gui): the deposit screen`.

---

### Task 6: The seam documentation, and the full gate

**Files:**
- Modify: `CLAUDE.md` — the collect bullet under "The base"
- Modify: `AGENTS.md` — **a gitignored twin of CLAUDE.md with no tracking to catch drift.** Edit `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`.
- Modify: `docs/seams.md` — the matching entry under the same title

- [ ] **Step 1: Widen the collect seam**

CLAUDE.md's bullet begins *"A collect is one reach rule and one taking path"*. It becomes a collect-and-deposit statement. The rule and the trap, one or two lines — the argument goes in `docs/seams.md`, not here, because this file is loaded into context on every turn.

The trap worth naming: **a Depot's room is one budget shared across every row, where a shelf gives each row an independent ceiling** — the one place the mirror does not hold, and the reason the two pickers share a key table with the ceiling carried as a value.

Second trap: `depositable` must filter `ItemDef::banked` and must reject a `Stock` without `stores`. Both are silent when wrong.

- [ ] **Step 2: Write the argument in `docs/seams.md`**

Under the same title: why the deposit goes into `output` (it is what makes `base_holding` and `feeders_for` see it, which is the feature), why plain copies only (`Stock` keys by `ItemId`, so a rare copy would come back ordinary), and what was rejected — any-`Stock`, a third buffer, a Take/Put toggle inside one mode.

- [ ] **Step 3: Copy the twin**

`cp CLAUDE.md AGENTS.md`, then `git diff --stat` to confirm both moved.

- [ ] **Step 4: The full gate**

`cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt --check`. Then `git diff --quiet assets/` against `origin/main` for anything you did not intend — a timed-out loop has left a shipped asset mutated in this repo before.

Report the actual test count and the actual output. Do not claim green without pasting it.

- [ ] **Step 5: Commit**

Commit `docs(seams): the deposit half of the collect seam`. Then stop and report — the version bump, the `CHANGELOG.md` section and the tag all happen at the merge, not here.

---

## What this plan does not do

- **No playtest.** A green suite is not evidence of play, and the user cannot playtest remotely. Say so plainly when reporting completion rather than implying the feature has been exercised.
- **No gear deposits.** Explicitly out of scope; the spec records why.
- **No `CHANGELOG.md`, no version bump, no tag.** Those belong to the merge.
