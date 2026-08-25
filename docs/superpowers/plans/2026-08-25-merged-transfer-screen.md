# Merged transfer screen — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the `c` collect picker and the `P` deposit picker with one
`Mode::Transfer` screen whose per-item amount is signed — negative puts into an
adjacent Depot, positive takes off an adjacent `Stock`.

**Architecture:** The engine grows one new module, `game/base/transfer.rs`,
holding the union offer, the two refusal sentences and a single commit door.
The commit door does not reimplement either half: `collect_items` and
`deposit_items` are first split so their moving bodies become `pub(crate)`
movers that neither tick nor log, and `transfer_items` calls both — take
first, then give — logging once and ticking once. app-core keeps its one
shared key table in `app/basket.rs`, widened from `Vec<u32>` to `Vec<i64>`
with two ceiling functions instead of one. gui collapses two renderers into
`render/transfer.rs`. The old public doors are deleted last, once nothing
calls them.

**Tech Stack:** Rust, `bevy_ecs` (engine only), `bevy` + `bevy_egui` (gui
only). No new dependencies. No save-format change — nothing here is stored.

**Spec:** `docs/superpowers/specs/2026-08-25-merged-transfer-screen-design.md`
— read it before Task 1 and keep it open. Every "why" behind the decisions
below lives there, and several of them look arbitrary without it.

## Global Constraints

- **Read `CLAUDE.md` first.** It is the project's rule file and it overrides
  anything here that contradicts it.
- **TDD, always.** Failing test first, watch it fail, minimal implementation,
  watch it pass, commit. This holds at every task size.
- **A test that passes with the fix removed is not a test.** For each new
  behavioural test, delete or invert the implementation line it covers, watch
  the test go red, restore. Say so in the task report.
- **Gates after every task:** `cargo fmt`, then `cargo clippy --workspace`
  (fix warnings, never silence them), then the task's own
  `cargo test -p <crate> <filter>`. **`cargo test --workspace` is the final
  gate before the branch is called done**, not a per-task gate.
- **Single-crate test runs shift the RNG stream.** `cargo test -p
  feral-processes-engine` and `cargo test --workspace` are different builds; a
  seeded test can pass in one and fail in the other. If a seeded test in an
  untouched subsystem goes red, check that before assuming a regression.
- **Left puts in, Right takes out.** This inverts collect, knowingly. Do not
  "restore" it — the spec's Decisions section says why, and a test must say so
  in as many words.
- **Never `git add -A`.** Stage explicit paths. Another session may have a
  worktree gitlink under `.claude/worktrees/`.
- **Commit freely, push never.** Pushing needs an explicit ask from the user.
  Do not bump the workspace version and do not write a `CHANGELOG.md` section
  — both happen once, at the merge.
- **Player-facing copy:** no occult naming. "Raid" is the code's word, "GC
  Entropy Sweep" the player's; follow the player's word in new text.
- **Do not write to `TODO.md`.** Do not update `docs/manual.md` or the root
  `README.md` — both are carved out. `assets/*/README.md` schema docs are
  **not** carved out, but this change adds no schema field.

---

## File map

**engine**

| File | Responsibility after this change |
| --- | --- |
| `crates/engine/src/game/base/transfer.rs` | **New.** The union offer, the two refusals, the one commit door. |
| `crates/engine/src/game/base/collect.rs` | `adjacent_stock` (unchanged) and the take mover. Public collect doors deleted in Task 8. |
| `crates/engine/src/game/base/deposit.rs` | `adjacent_depots`, `deposit_room` (both unchanged) and the give mover. Public deposit doors deleted in Task 8. |
| `crates/engine/src/game/base/mod.rs` | One added `pub(crate) mod transfer;`. |
| `crates/engine/src/views.rs` | `TransferRow`. |

**app-core**

| File | Responsibility after this change |
| --- | --- |
| `crates/app-core/src/app/basket.rs` | The one key table, now signed, with two ceiling functions. |
| `crates/app-core/src/app/transfer.rs` | **New.** Opens the picker, commits the basket. Replaces `app/collect.rs` and `app/deposit.rs`, both **deleted**. |
| `crates/app-core/src/app/playing.rs` | The `c` arm rewritten; the `P` arm deleted. |
| `crates/app-core/src/app/input.rs` | The modifier fold and the mode dispatch name `Mode::Transfer`. |
| `crates/app-core/src/lib.rs` | `Mode::Transfer`; the three `basket_*` fields retyped. |
| `crates/app-core/src/tests/transfer.rs` | **New.** Replaces `tests/collect.rs` and `tests/deposit.rs`, both **deleted**. |
| `crates/app-core/src/tests/mod.rs` | `mod collect;` and `mod deposit;` become `mod transfer;`. |
| `crates/app-core/src/tests/support.rs` | `app_beside_depots` gains a `shelves` parameter. |

**gui**

| File | Responsibility after this change |
| --- | --- |
| `crates/gui/src/render/transfer.rs` | **New.** Replaces `render/collect.rs` and `render/deposit.rs`, both **deleted**. |
| `crates/gui/src/render/mod.rs` | `mod transfer;`, the dispatch arm, and `ALL_MODES`. |

**assets**

| File | Change |
| --- | --- |
| `assets/structures/depot.ron` | The `description` names one key. |
| `assets/help/20-controls.md` | Two key lines become one. |

---

## Task 1: Split the movers out of the two commit doors

A pure refactor with no behaviour change. The existing collect and deposit
suites passing unchanged **is** the proof — do not add tests here.

**Files:**
- Modify: `crates/engine/src/game/base/collect.rs`
- Modify: `crates/engine/src/game/base/deposit.rs`

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub(crate) fn take_from_adjacent(&mut self, want: &[(ItemId, u32)]) -> Vec<(ItemId, u32)>`
  - `pub(crate) fn give_to_adjacent(&mut self, give: &[(ItemId, u32)]) -> Vec<(ItemId, u32)>`

  Both return **what actually moved**, keyed and ordered as the existing
  `BTreeMap` accumulators already produce. Both **assume the guards have
  already been checked** — they hold none of their own. Neither ticks and
  neither logs. Say both facts in each doc comment; a later caller that
  forgets the guards is the failure mode this shape opens.

- [ ] **Step 1: Move `collect_items`' body into `take_from_adjacent`**

Everything from `let player = self.player_entity();` down to the point where
`taken` is complete moves across. `collect_items` keeps its three guards, calls
the mover, and keeps the `taken.is_empty()` early return, the summary
`format!`, the `log_base_kind(MessageKind::Loot, ...)` line and the
`self.tick()`.

- [ ] **Step 2: Do the same for `deposit_items` / `give_to_adjacent`**

Identical split. `deposit_items` keeps its guards, the `given.is_empty()`
early return, the `log_base` line and the `self.tick()`.

- [ ] **Step 3: Run the two existing suites**

```
cargo test -p feral-processes-engine collect
cargo test -p feral-processes-engine deposit
cargo test -p feral-processes-app-core collect
cargo test -p feral-processes-app-core deposit
```

Expected: all PASS, unchanged. A failure here means the split changed
behaviour — find it before continuing.

- [ ] **Step 4: `cargo fmt && cargo clippy --workspace`, then commit**

```bash
git add crates/engine/src/game/base/collect.rs crates/engine/src/game/base/deposit.rs
git commit -m "refactor(base): split the movers out of collect and deposit"
```

---

## Task 2: `TransferRow` and `Game::transfer_offer`

**Files:**
- Modify: `crates/engine/src/views.rs`
- Create: `crates/engine/src/game/base/transfer.rs`
- Modify: `crates/engine/src/game/base/mod.rs`
- Test: `crates/engine/src/tests/` — a new `transfer.rs` beside the existing
  `collect.rs`/`deposit.rs` suites, registered in that directory's `mod.rs`.

**Interfaces:**
- Consumes: `Game::adjacent_stock()`, `Game::adjacent_depots()`,
  `Game::is_banked()`, `Game::deposit_room()` — all already exist.
- Produces:
  - `pub struct TransferRow { pub item: ItemId, pub on_shelves: u32, pub in_pack: u32 }`
    in `views.rs`, deriving whatever the neighbouring view structs derive.
  - `pub fn transfer_offer(&self) -> Vec<TransferRow>` on `Game`.

**Behaviour to build:**

`&self`. No tick, no log, no RNG. Guards first, in `collectable_adjacent`'s
existing order — game over, active battle, `require_base` — each answering
with an empty `Vec`. No `require_surface`: `require_base` is the stronger
statement.

Rows are the union of the two existing offers, sorted by `ItemId`. Build them
in a `BTreeMap<ItemId, TransferRow>` so the sort is the map's rather than a
second explicit one.

- `on_shelves` — pooled non-zero `output` across `adjacent_stock()`, exactly
  as `collectable_adjacent` pools today.
- `in_pack` — the player's `Inventory` row for that item, **0 unless** the
  item is un-`banked` *and* `adjacent_depots()` is non-empty.

**Tests to write (intent — write them first, one at a time):**

1. An item on an adjacent shelf **and** in the pack is **one** row carrying
   both figures. This is the case the whole feature exists for.
2. Rows come back in `ItemId` order with items drawn from both sides.
3. A `banked` item in the pack gets `in_pack: 0`. It may still have
   `on_shelves` — a Research Node produces `research_data`, so a banked item
   on a shelf is a real take row.
4. Beside a `Stock` that does **not** `stores` (a Mining Node), every row has
   `in_pack: 0` while `on_shelves` is untouched.
5. Each of the three guards returns an empty offer.

- [ ] **Step 1: Write test 1, run it, watch it fail to compile**
- [ ] **Step 2: Add `TransferRow` and a stub `transfer_offer`; make test 1 pass**
- [ ] **Step 3: Tests 2–5, one at a time, red then green**
- [ ] **Step 4: Mutation-prove each of tests 1, 3 and 4** — delete the union,
      the `banked` filter and the `adjacent_depots().is_empty()` check in turn,
      watch the matching test go red, restore.
- [ ] **Step 5: Gates and commit**

```
cargo test -p feral-processes-engine transfer
cargo fmt && cargo clippy --workspace
git add crates/engine/src/views.rs crates/engine/src/game/base/transfer.rs \
        crates/engine/src/game/base/mod.rs crates/engine/src/tests/
git commit -m "feat(base): one offer spanning the shelves and the pack"
```

---

## Task 3: `Game::transfer_items`

**Files:**
- Modify: `crates/engine/src/game/base/transfer.rs`
- Test: `crates/engine/src/tests/transfer.rs`

**Interfaces:**
- Consumes: `take_from_adjacent`, `give_to_adjacent` (Task 1).
- Produces:
  `pub fn transfer_items(&mut self, take: &[(ItemId, u32)], give: &[(ItemId, u32)]) -> (Vec<(ItemId, u32)>, Vec<(ItemId, u32)>)`
  — what was taken, then what was given.

**Behaviour to build:**

Guards first, returning `(Vec::new(), Vec::new())` for each.

**Take before give.** This is the one ordering constraint in the task and it
is load-bearing: a rebalance that empties a full Depot and refills it from the
pack only lands both halves in this order. Doing it the other way is refused
for want of room, and the failure is silent — the give clamps to zero and
`give_to_adjacent` returns an empty list.

One `Loot` line for what came and one base line for what went, in that order,
each skipped when its half is empty. Then **one** `self.tick()`, and only if
either half moved anything. Reuse the two summary `format!`s the existing
wrappers build — extract a small private helper rather than writing a third
copy of the join.

An empty or all-zero request is a silent no-op: nothing moved, nothing said,
no turn spent.

**Tests to write:**

1. A basket with both halves ticks **exactly once** — assert against
   `current_tick()` before and after.
2. Both log lines are emitted, the `Loot` one first, and each is absent when
   its half is empty.
3. **Take runs before give.** Fixture: a Depot at exactly `capacity`, a pack
   holding something the Depot does not, and a basket that takes enough out to
   make room and puts that much in. Both halves must land. This test fails if
   the order is reversed, which is the point.
4. An all-zero basket spends no tick and logs nothing.
5. The clamps still hold: an over-ask on the take side is clamped, and a give
   larger than the room leaves the surplus **in the pack** rather than eaten.

- [ ] **Step 1: Test 3 first** — it is the one that pins the ordering, so write
      it before the implementation exists to be reasoned backwards from.
- [ ] **Step 2: Implement `transfer_items`; test 3 green**
- [ ] **Step 3: Tests 1, 2, 4, 5, one at a time, red then green**
- [ ] **Step 4: Mutation-prove test 3** — swap the two mover calls, watch it go
      red, restore. Mutation-prove test 1 by adding a second `tick()`.
- [ ] **Step 5: Gates and commit**

```
cargo test -p feral-processes-engine transfer
cargo fmt && cargo clippy --workspace
git commit -m "feat(base): one commit that takes and gives in one tick"
```

---

## Task 4: `Game::refuse_transfer`

**Files:**
- Modify: `crates/engine/src/game/base/transfer.rs`
- Test: `crates/engine/src/tests/transfer.rs`

**Interfaces:**
- Produces: `pub fn refuse_transfer(&mut self)`.

**Behaviour to build:**

Guards first and **silent** — an action taken during a battle or from the
surface is not the base telling you its shelves are bare. Then two sentences,
because they leave the player different errands:

- `adjacent_stock()` empty → `"There is nothing here to take from or put into."`
- otherwise → `"There is nothing to move here."`

Both through `log_base`, matching the three sentences being replaced. Do **not**
delete those three sentences yet; Task 8 does that once nothing calls them.

**Tests to write:**

1. No adjacent `Stock` at all → the first sentence, in the log.
2. An adjacent `Stock` with empty shelves and an empty pack → the second.
3. Each guard refuses **silently** — no log line at all. Assert on the log
   length, not just on the absence of a particular string.

- [ ] **Step 1: Tests 1–3, one at a time, red then green**
- [ ] **Step 2: Mutation-prove test 3** — drop one guard, watch it go red
- [ ] **Step 3: Gates and commit**

```
cargo test -p feral-processes-engine transfer
git commit -m "feat(base): two refusals for the transfer screen"
```

---

## Task 5: The signed basket, `Mode::Transfer`, and the key table

The largest task, and it cannot be split further: the `Mode` rename has to land
in app-core and gui together or nothing compiles.

**Files:**
- Modify: `crates/app-core/src/lib.rs`
- Modify: `crates/app-core/src/app/basket.rs`
- Create: `crates/app-core/src/app/transfer.rs`
- Delete: `crates/app-core/src/app/collect.rs`, `crates/app-core/src/app/deposit.rs`
- Modify: `crates/app-core/src/app/playing.rs`, `crates/app-core/src/app/input.rs`
- Create: `crates/app-core/src/tests/transfer.rs`
- Delete: `crates/app-core/src/tests/collect.rs`, `crates/app-core/src/tests/deposit.rs`
- Modify: `crates/app-core/src/tests/mod.rs`, `crates/app-core/src/tests/support.rs`
- Modify: `crates/gui/src/render/mod.rs` — `mod transfer;`, the dispatch arm,
  and `ALL_MODES` (which is `[Mode; 88]` today and becomes `[Mode; 87]`)
- Create: `crates/gui/src/render/transfer.rs` — a straightforward port for now;
  Task 6 makes it the specified screen
- Delete: `crates/gui/src/render/collect.rs`, `crates/gui/src/render/deposit.rs`

**Interfaces:**
- Consumes: `Game::transfer_offer`, `Game::transfer_items`,
  `Game::refuse_transfer`, `Game::deposit_room`.
- Produces:
  - `Mode::Transfer`, replacing `Mode::Collect` and `Mode::Deposit`.
  - `pub basket_rows: Vec<TransferRow>`
  - `pub basket_amounts: Vec<i64>`
  - `pub basket_room: Option<u32>` — **type unchanged, meaning changed.**
    `None` is "no Depot beside you", `Some(0)` is "a Depot with nothing left".
    Keeping these distinguishable is the whole reason the report this came from
    was confusing; do not collapse it to a plain `u32`.
  - `pub fn take_available(&self, row: usize) -> u32`
  - `pub fn put_available(&self, row: usize) -> u32`
  - `pub(crate) fn open_transfer(&mut self, rows: Vec<TransferRow>, room: Option<u32>)`
  - `pub(crate) fn commit_transfer(&mut self)`

**The three formulas that are worth spelling out.** Everything else in this
task follows the shape already in `basket.rs`.

The two ceilings. `take_available` is per row and static; `put_available` is
one shared budget and subtracts only the **other** rows, which is what lets the
highlighted row be lowered and raised while it is being edited:

```rust
pub fn put_available(&self, row: usize) -> u32 {
    let given: u32 = self.basket_amounts.iter()
        .enumerate()
        .filter(|(i, _)| *i != row)
        .map(|(_, n)| n.min(&0).unsigned_abs() as u32)
        .sum();
    let budget = self.basket_room.unwrap_or(0).saturating_sub(given);
    self.basket_rows.get(row).map_or(0, |r| r.in_pack).min(budget)
}
```

The Ctrl step, generalised so each modifier pair points at the end its
unmodified arrow heads for. `div_ceil` on the **magnitude** of the gap is what
makes it terminate — rounded down, a gap of one gives a step of zero and the
key goes dead with the row neither full nor empty:

```rust
fn half_way_to(n: i64, target: i64) -> i64 {
    let gap = target - n;
    n + gap.signum() * gap.unsigned_abs().div_ceil(2) as i64
}
```

The digit rule — magnitude accumulates in the row's current sign, and a row at
zero types a take. `saturating_*` because a held digit key must reach the clamp
rather than overflow:

```rust
let sign = if n < 0 { -1 } else { 1 };
let magnitude = n.unsigned_abs().saturating_mul(10).saturating_add(d as u64);
clamp_row(row, sign * magnitude.min(i64::MAX as u64) as i64)
```

**The rest of the key table**, built the same way `basket.rs` already builds
it, every write passing through one `clamp_row(row, want)` that clamps to
`-(put_available as i64) ..= take_available as i64`:

| Key | Effect |
| --- | --- |
| `Left` | `n - 1` |
| `Right` | `n + 1` |
| `Shift`+`Left` | `-put_available(row)` |
| `Shift`+`Right` | `+take_available(row)` |
| `Ctrl`+`Left` | `half_way_to(n, -put_available(row))` |
| `Ctrl`+`Right` | `half_way_to(n, take_available(row))` |
| digit | as above |
| `Backspace` | magnitude `/ 10`, sign kept |
| `[A]` | every row to `+take_available`, **row by row** |
| `[N]` | every row to `0` |
| `Enter` | commit, then leave |
| `Esc` | leave |

`[A]` writes the take ceiling over **every** row, clearing a give the player
had set on a row with nothing on the shelf. That is what "take everything"
means on one axis; it is a decision, not an oversight, and the test for it
should say so.

`[A]` fills row by row through `take_available` rather than zipping across —
the existing reason, preserved even though `[A]` no longer touches the shared
budget, because the loop is what stops the next person reintroducing a zip.

**`playing.rs`.** The `c` arm becomes: call `game.transfer_offer()`; if empty,
call `game.refuse_transfer()` and return `true`; otherwise hand the rows and
`game.deposit_room()` out past the `self.game` borrow — the existing `opening`
local does this and the `putting` local is deleted with the `P` arm. `room` is
`Some(deposit_room())` when `adjacent_depots()` is non-empty and `None`
otherwise; add a small `&self` engine call for that rather than inferring it
from a zero. **Opening a screen is not an action** — it must leave `acted`
false so no turn is spent and the last refusal is not cleared.

**`input.rs`.** Both the modifier-promotion condition (currently
`matches!(self.mode, Mode::Collect | Mode::Deposit)`) and the mode dispatch
name `Mode::Transfer`. The fold back to bare `Left`/`Right` for every other
mode stays exactly as it is — a modified arrow reaching a handler that ends in
`_ => {}` is a dead key nothing catches.

**`support.rs`.** `app_beside_depots(seed, depots, filled, pack)` gains a
`shelves: &[(&str, u32)]` parameter so a fixture can stand a non-`stores`
`Stock` beside the party and stock it. Update the existing call sites.

**Tests to write** (`tests/transfer.rs`, absorbing what `tests/collect.rs` and
`tests/deposit.rs` covered — read both before deleting them, and carry across
every case that still applies):

1. `c` beside a stocked `Stock` opens `Mode::Transfer` with every amount at 0.
2. Opening spends no turn and does not clear `status_line`.
3. `c` with nothing on either side refuses through the engine and opens nothing.
4. **`Left` puts in and `Right` takes out** — named and asserted in as many
   words, because this inverts collect and is the thing a later reader
   "restores".
5. `Shift` is a target and is idempotent under key repeat; `Ctrl` is a step
   that halves the gap and **terminates** — pin it on a gap of 1 at both ends.
6. The put budget is shared: filling one row lowers `put_available` on the
   others, while the highlighted row keeps its own amount while being edited.
7. **A Depot at exactly `capacity`** leaves every `put_available` at 0 while
   `take_available` is untouched. This is the reported case; the suite covers
   195/200 today and never 200/200.
8. `basket_room` is `None` beside a non-`stores` `Stock` and `Some(0)` beside a
   full Depot — the two states that must not collapse.
9. Digits type into the row's current sign, and into a take at zero;
   `Backspace` keeps the sign.
10. `[A]` sets every row to its take ceiling **including** a row the player had
    set to give.
11. `[N]` zeroes everything.
12. `Enter` commits both halves and leaves; an all-zero `Enter` still leaves
    and never reaches the engine.
13. Every exit path clears all three `basket_*` fields.

- [ ] **Step 1: Retype the fields and add `Mode::Transfer`**, deleting the two
      old variants. Nothing compiles yet; that is expected.
- [ ] **Step 2: Rewrite `basket.rs` signed**, with the two ceilings, the clamp
      helper and the key table above.
- [ ] **Step 3: `app/transfer.rs`; delete `app/collect.rs` and `app/deposit.rs`**
- [ ] **Step 4: `playing.rs` and `input.rs`**
- [ ] **Step 5: Port the gui renderer** into `render/transfer.rs` — a working
      port drawing a row per item with the current amount and both ceilings.
      Presentation is Task 6's job; getting it compiling and drawing is this
      one's. Update `render/mod.rs` and `ALL_MODES`; delete the two old files.
- [ ] **Step 6: `support.rs`, then write tests 1–13 one at a time, red then
      green.** Read `tests/collect.rs` and `tests/deposit.rs` before deleting
      them and carry across every case that still applies.
- [ ] **Step 7: Mutation-prove tests 4, 6, 7 and 10** — invert the arrows,
      make `put_available` count the highlighted row, hard-code a non-zero
      budget, and make `[A]` skip negative rows. Each must go red, then restore.
- [ ] **Step 8: Gates and commit**

```
cargo test -p feral-processes-app-core transfer
cargo test -p feral-processes-gui
cargo fmt && cargo clippy --workspace
git commit -m "feat(base): one screen for taking and putting"
```

---

## Task 6: The transfer screen as specified

**Files:**
- Modify: `crates/gui/src/render/transfer.rs`
- Modify: `crates/gui/src/render/mod.rs` if the dispatch needs the extra values

**Behaviour to build:**

Body rows, in order — the room line first and **omitted entirely** when
`basket_room` is `None`, because a Mining Node has no room to report and a
line reading 0 there claims the base is full when it has no shelf at all:

```
Depot room remaining: {remaining}
Up/Down pick a row; digits and Backspace type an amount
Left puts in, Right takes out; Shift for the end, Ctrl halves the gap
[A] take everything  [N] clear  Enter to transfer  Esc to leave
(blank)
```

`remaining` is `basket_room` less the sum of the negative amounts.

Each item row goes through `annotated_item_row` with a **suffix column**, never
`format!`ed into the name — six screens made that mistake with the category tag,
and measuring a row without its column makes `suffix_x` drop the suffix on the
row's own tail. The suffix is the signed amount and then the row's **live**
availables: the deposit screen already draws live figures, and a row reading
`-0` while the pack still holds units is the screen saying the other rows have
spent the room.

Compute the availables in `render/mod.rs` before `&mut app.game` is taken —
`take_available`/`put_available` borrow the whole `App`, exactly as
`basket_available` does today, and the existing `deposit_entries` local is the
pattern to follow.

**Tests to write:**

1. `no_transfer_row_overflows_its_popup` — the width census. The name comes
   from the real `ItemDb` and the figures are the widest a `u32` can print at
   **both** ends of the range: nothing bounds a modded Depot's `capacity`, and
   `draw_row` clips vertically only, so an over-wide row is drawn off the panel
   in silence. `no_deposit_row_overflows_its_popup` is the model.
2. The hint lines stay no wider than the widest item row the census measures.
3. The room line is absent when `basket_room` is `None` and present reading 0
   when it is `Some(0)`.
4. `every_screen_draws_a_refusal_exactly_once` still passes with
   `Mode::Transfer` in `ALL_MODES` — it drives every `Mode` through `draw` and
   counts what was painted, and it has caught both a screen showing a refusal
   nowhere and one showing it twice.

- [ ] **Step 1: Tests 1–3, one at a time, red then green**
- [ ] **Step 2: Run the whole gui suite** — `cargo test -p feral-processes-gui`
- [ ] **Step 3: Mutation-prove test 3** — draw the line unconditionally, watch
      it go red, restore
- [ ] **Step 4: Gates and commit**

```
cargo fmt && cargo clippy --workspace
git commit -m "feat(gui): draw the transfer screen"
```

---

## Task 7: Content and docs

**Files:**
- Modify: `assets/structures/depot.ron`
- Modify: `assets/help/20-controls.md`

**Behaviour to build:**

`depot.ron`'s `description` currently reads "Collect from it with c, or put
your own cargo away with P." It must name one key and one screen. Keep it a
sentence a player reads in a build menu, not a key reference.

`20-controls.md` lines 26–27 are two entries, one for `c` and one for `P`.
They become one entry for `c` that says both directions and names the arrow
mapping — a modifier is invisible until named, and the arrows here run against
every other Left/Right in the game.

**Watch for:** `assets/help/README.md` is the one asset directory whose schema
reference shares an extension with its content. `HelpDb::load_dir` skips that
name explicitly and the easter-egg census reads **parsed pages** rather than
raw files. Do not add a page; edit the existing one.

**Tests to run:** the asset censuses in `crates/engine/src/tests/assets.rs` and
the gui help suite. Nothing new to write — this is content, and the censuses
that guard it already exist.

- [ ] **Step 1: Edit both files**
- [ ] **Step 2: `cargo test -p feral-processes-engine assets` and
      `cargo test -p feral-processes-gui help`**
- [ ] **Step 3: `rg -n "\bP\b" assets/help/20-controls.md assets/structures/depot.ron`
      to confirm no stale key reference survives**
- [ ] **Step 4: Commit**

```bash
git add assets/structures/depot.ron assets/help/20-controls.md
git commit -m "content: one key for taking and putting"
```

---

## Task 8: Delete the doors nothing calls

Last, once every caller is on the new path. Per CLAUDE.md: no
backwards-compat cruft — if something's unused, delete it.

**Files:**
- Modify: `crates/engine/src/game/base/collect.rs`
- Modify: `crates/engine/src/game/base/deposit.rs`
- Modify: `crates/engine/src/tests/` — the collect and deposit suites

**To delete:**

| Symbol | Kept? |
| --- | --- |
| `Game::collectable_adjacent` | delete |
| `Game::collect_items` | delete |
| `Game::collect_adjacent` | delete |
| `Game::depositable` | delete |
| `Game::deposit_items` | delete |
| `Game::deposit_adjacent` | delete |
| `Game::adjacent_stock` | **keep** — `transfer_offer` calls it |
| `Game::adjacent_depots` | **keep** — `transfer_offer` calls it |
| `Game::deposit_room` | **keep** — the screen's header |
| `take_from_adjacent`, `give_to_adjacent` | **keep** |

The three refusal sentences go with their functions.

- [ ] **Step 1: `rg -n "collectable_adjacent|collect_items|collect_adjacent|depositable|deposit_items|deposit_adjacent" crates/`**
      — confirm every remaining hit is a test or a doc comment before deleting
      anything. A hit in `crates/gui` or `crates/launcher` means Task 5 missed a
      caller; fix that first.
- [ ] **Step 2: Move the still-meaningful cases from the engine's collect and
      deposit suites into `tests/transfer.rs`**, then delete what tested only a
      deleted door. Do not delete a test that is the only cover for a rule that
      survived — the `(x, y)` scan order and the two clamps both still apply,
      through the movers.
- [ ] **Step 3: Delete the six functions and their doc comments**
- [ ] **Step 4: `cargo test --workspace`** — the final gate. Expected: green,
      with the suite count down by however many tests Step 2 retired, and no
      dead-code warnings.
- [ ] **Step 5: Gates and commit**

```
cargo fmt && cargo clippy --workspace
git commit -m "refactor(base): retire the two single-direction doors"
```

---

## Done when

- [ ] `cargo test --workspace` green.
- [ ] `cargo clippy --workspace` clean.
- [ ] `rg -n "Mode::Collect|Mode::Deposit"` returns nothing.
- [ ] Every new behavioural test has been mutation-proved and the task report
      says so.
- [ ] The branch is **not** pushed, the workspace version is **not** bumped and
      `CHANGELOG.md` has **no** new section — all three happen at the merge.
- [ ] Loading `~/.local/share/feral-processes/saves/save_1787262086.bin` and
      standing at base cell `(-6, 1)` — beside the Depot at `(-6, 2)`, which is
      at 200/200 — shows a screen whose take ends are live and whose put ends
      all read `-0`, with the room header reading 0. That save is the report
      this change came from and is the one end-to-end check worth doing by hand.
