# The collect picker — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the wholesale `c` collect with a popup that takes only the
items and quantities the player asks for.

**Architecture:** One private tile-sorted neighbour scan in the engine feeds
two public verbs — a `&self` view of what is on offer and a `&mut self` verb
that takes a requested basket. `collect_adjacent` is reduced to a wrapper over
both, so take-all and take-some share one taking path. app-core holds the
pending basket as snapshot state behind a new `Mode`; gui draws it.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine only), bevy + `bevy_egui` (gui).

**Spec:** `docs/superpowers/specs/2026-08-22-collect-picker-design.md` — read
it first. This plan argues from it and does not restate its reasoning.

## Global Constraints

- **Do not write implementation code from this plan verbatim.** Per
  `CLAUDE.md`'s process-weight section, the plan gives file lists, exact
  interfaces, test intent and gates. Write the code yourself and push back if
  the plan looks wrong.
- **TDD, no exceptions.** Failing test first, watch it fail, minimal
  implementation, watch it pass, commit.
- **Every new test must be mutation-proved.** Delete or invert the fix, run
  the test, confirm it fails, restore. A test that passes with the fix removed
  is not coverage. Record the mutation used in the commit body.
- **Never `git push`.** Commit freely on the branch; the user asks for pushes.
- The branch is `collect-picker`. Check `git branch --show-current` before
  every commit — a concurrent session has merged and deleted a branch
  mid-task in this repo before.
- **No new tuning constants.** This change touches no balance knob.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix
  warnings rather than silencing them.
- Iterate with `cargo test -p feral-processes-engine <name>`. Save
  `cargo test --workspace` for phase boundaries.
- Never write to `TODO.md`, `docs/manual.md`, or the root `README.md`.

---

# Phase 1 — Engine: one reach rule, one taking path

Deliverable: `Game` can report what is collectable and take an exact basket,
and `collect_adjacent` is a wrapper over both with its behaviour unchanged.

All of Phase 1 lives in `crates/engine/src/game/base/collect.rs` and
`crates/engine/src/tests/collect.rs`.

**Existing fixtures you will use** (already in `tests/collect.rs`):
`stocked_structure(game, kind, x, y, &[(item_id, qty)]) -> Entity` and
`player_tile(game) -> Position`. Do not write new ones. Broader fixtures live
in `crates/engine/src/tests/support.rs` — look there before adding any.

### Task 1.1: The view

**Files:**
- Modify: `crates/engine/src/game/base/collect.rs`
- Test: `crates/engine/src/tests/collect.rs`

**Interfaces:**
- Produces:
  - `fn adjacent_stock(&self) -> Vec<Entity>` — private to the module. The
    four `ORTHOGONAL` tiles off `base_pos()`, filtered to entities with a
    `Stock`, **sorted by `(x, y)`**.
  - `pub fn collectable_adjacent(&self) -> Vec<(ItemId, u32)>` — items pooled
    across those entities, in `ItemId` order, zero-quantity entries dropped.
    `&self`: no tick, no log, no RNG.
- Consumes: `collect::ORTHOGONAL`, `Game::base_pos`, `Game::require_base`.

Both `collect_adjacent`'s existing neighbour scan and this one must become the
single `adjacent_stock` call in Task 1.3 — write it here so there is only ever
one.

`collectable_adjacent` holds the same four guards `collect_adjacent` holds
today and returns an empty `Vec` for each: `is_game_over().is_some()`,
`has_active_battle()`, `require_base().is_err()`, and `base_pos()` returning
`None`.

- [ ] **Step 1: Write the failing tests**

Four, all in `tests/collect.rs`:

1. Two structures on opposite orthogonal tiles both holding the same item are
   reported as **one** row with the summed quantity, and a third item held by
   only one of them appears as its own row. Assert the full `Vec` including
   order, so `ItemId` ordering is pinned rather than incidentally passing.
2. The scan is sorted: stand structures at tiles whose spawn order is the
   reverse of their `(x, y)` order, and assert `adjacent_stock` returns them
   in `(x, y)` order. Test the private fn directly — `tests/` is a child
   module of the crate, so it can see it.
3. A structure whose `output` is empty contributes no row, and a structure
   diagonally adjacent contributes nothing at all.
4. Each of the four guards returns empty. Write this as **one** test asserting
   all four, because the battle guard alone passes against a bare early
   return that swallows the others — the same trap
   `zone_one_takes_no_bite_but_still_names_the_ground` documents.

- [ ] **Step 2: Run and watch them fail**

`cargo test -p feral-processes-engine collect` — expect failures on
unresolved `collectable_adjacent` / `adjacent_stock`.

- [ ] **Step 3: Implement**

Write `adjacent_stock` and `collectable_adjacent`. Pool into a
`BTreeMap<ItemId, u32>` — it is what `Stock::output` already is, so ordering
falls out rather than being sorted a second time.

Do **not** touch `collect_adjacent` yet; it keeps its own inline scan until
Task 1.3. Two scans coexisting for one task is deliberate — it keeps this
task's diff reviewable.

- [ ] **Step 4: Run and watch them pass**

`cargo test -p feral-processes-engine collect`

- [ ] **Step 5: Mutation-prove**

For each test: remove the sort and confirm test 2 fails; drop the pooling
(emit one row per structure) and confirm test 1 fails; remove each guard in
turn and confirm test 4 fails. Restore between each. Record the mutations in
the commit body.

- [ ] **Step 6: Commit**

```bash
git branch --show-current   # must print collect-picker
git add crates/engine/src/game/base/collect.rs crates/engine/src/tests/collect.rs
git commit
```

Subject: `feat(base): the base says what is on its shelves before you take it`

### Task 1.2: The taking path

**Files:**
- Modify: `crates/engine/src/game/base/collect.rs`
- Test: `crates/engine/src/tests/collect.rs`

**Interfaces:**
- Produces: `pub fn collect_items(&mut self, want: &[(ItemId, u32)]) -> Vec<(ItemId, u32)>`
  — draws each wanted item off the sorted neighbours, adds to the player's
  `Inventory`, returns what **actually** landed, in `ItemId` order.
- Consumes: `adjacent_stock` (Task 1.1),
  `crate::game::base::hauling::take_from(stock, item, qty) -> u32`.

Behaviour, each point a test below:

- Units leave a buffer **only** through `hauling::take_from`. Its doc comment
  already names it the one way, and a second remove-or-decrement is how a
  buffer ends up holding a zero entry every reader has to skip. Do not write
  `output.remove(&item)`.
- An over-ask is **clamped**, never refused.
- Ticks **once** for the whole basket, via `self.tick()`.
- Logs **one** `MessageKind::Loot` line, built the way the current
  `collect_adjacent` builds its summary (reuse that code — it moves here).
- An empty or all-zero request takes nothing, logs nothing, and does **not**
  tick. This path must not emit the "There is nothing to collect here."
  refusal either; that sentence belongs to `collect_adjacent` (Task 1.3) and
  must be stated once.
- Holds the same four guards as Task 1.1, returning empty.

- [ ] **Step 1: Write the failing tests**

Six:

1. Asking for less than a structure holds takes exactly that and leaves the
   remainder in `Stock::output`. Assert both the return value and the
   structure's buffer.
2. Asking for more than is held clamps: the return reports what came, the
   buffer is empty, and the player's `Inventory` holds the smaller figure.
3. **The sort test.** Two adjacent structures each holding the same item;
   request less than the pooled total but more than the first holds. Assert
   the *lower*-`(x, y)` structure is emptied and the *higher* one holds the
   remainder. This is the test the sort exists for — spawn them in reverse
   order so an unsorted implementation genuinely flips.
4. A basket naming two different items ticks exactly once and logs exactly
   one line. Compare `Game`'s tick counter before and after, and count log
   lines rather than reading their text.
5. An empty slice, and a slice of all-zero quantities, each take nothing, log
   nothing, and leave the tick counter unmoved.
6. The four guards, again as one test.

- [ ] **Step 2: Run and watch them fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run and watch them pass**

- [ ] **Step 5: Mutation-prove**

Swap `take_from` for a bare `remove` and confirm test 1 fails. Move
`self.tick()` inside the per-item loop and confirm test 4 fails. Remove the
all-zero early return and confirm test 5 fails. Remove the sort and confirm
test 3 fails. Restore between each.

- [ ] **Step 6: Commit**

Subject: `feat(base): collecting takes the amount it was asked for`

### Task 1.3: Take-all becomes a basket

**Files:**
- Modify: `crates/engine/src/game/base/collect.rs`
- Test: `crates/engine/src/tests/collect.rs` (existing tests only)

**Interfaces:**
- Produces: `pub fn collect_adjacent(&mut self) -> Vec<(ItemId, u32)>`,
  signature unchanged, body reduced to
  `let all = self.collectable_adjacent(); self.collect_items(&all)` plus the
  empty-case refusal.
- The inline neighbour scan and the `output.remove` loop are **deleted**.

The refusal stays here and only here: when `collectable_adjacent()` is empty,
log `"There is nothing to collect here."` through `log_base`, return empty,
and do not tick. app-core will route the empty case back through this function
precisely so the sentence has one home (Task 2.1).

Sequence the view into a local before the `&mut self` call — `self.collect_items(&self.collectable_adjacent())` does not borrow-check.

- [ ] **Step 1: Run the existing tests, unchanged, and watch them pass**

`cargo test -p feral-processes-engine collect hauling base_space building work_orders`

They currently pass. This is the baseline: **write no new test for this
task.** The existing suite is the evidence the wrapper preserves behaviour,
and a new test written against the new body would only assert the new body.

- [ ] **Step 2: Rewrite the body**

- [ ] **Step 3: Run them again and watch them still pass**

Same command. Any failure here is a behaviour change, not a flaky test —
read it rather than adjusting the assertion.

- [ ] **Step 4: Mutation-prove the refusal**

Delete the empty-case branch (so the refusal never logs) and confirm the
existing test in `tests/collect.rs` that asserts a collect with nothing
adjacent fails. If no existing test covers it, **write one now** — that is a
real gap, not a task boundary.

- [ ] **Step 5: Phase gate**

`cargo test --workspace` and `cargo test -p feral-processes-engine balance_sim`.
Both must be green before Phase 2. `balance_sim` should be untouched; a moved
curve means something unintended happened.

- [ ] **Step 6: Commit**

Subject: `refactor(base): take-all is selecting everything, then committing`

---

# Phase 2 — app-core: the mode, the basket, the keys

Deliverable: the whole screen works headlessly — `c` opens it, keys edit a
basket, Enter commits exactly that basket. Nothing is drawn yet.

New module `crates/app-core/src/app/collect.rs`, following the shape of
`crates/app-core/src/app/crafting.rs` (a small module per screen). Tests go in
a new `crates/app-core/src/tests/collect.rs`; register both modules in their
parent `mod.rs`.

### Task 2.1: The mode, the state, and the key that opens it

**Files:**
- Create: `crates/app-core/src/app/collect.rs`
- Create: `crates/app-core/src/tests/collect.rs`
- Modify: `crates/app-core/src/lib.rs` — `Mode` enum near line 788, `App`
  fields near line 1530 (where `craft_quantity_input` lives)
- Modify: `crates/app-core/src/app/mod.rs`, `crates/app-core/src/tests/mod.rs`
  — declare the new modules
- Modify: `crates/app-core/src/app/input.rs:140` area — the `handle_key`
  dispatch match
- Modify: `crates/app-core/src/app/playing.rs:246-250` — the `c` arm

**Interfaces:**
- Produces:
  - `Mode::Collect` — a doc comment on the variant saying what the screen is
    and, specifically, **that it cannot use `selected_index`** because digits
    are quantities here rather than row picks. That is the non-obvious fact a
    later reader needs.
  - `App::collect_rows: Vec<(ItemId, u32)>` — pub, the snapshot of what is on
    offer, taken when the screen opens.
  - `App::collect_basket: Vec<u32>` — pub, same length as `collect_rows`, all
    zeroes on open.
  - `App::handle_collect_key(&mut self, key: GameKey)` — `pub(crate)`, stubbed
    this task to Esc-only; filled in by Tasks 2.2 and 2.3.
- Consumes: `Game::collectable_adjacent`, `Game::collect_adjacent` (Phase 1).

The `c` arm changes shape. It currently calls `game.collect_adjacent()` and
returns `true` (acted). Now:

- Ask `game.collectable_adjacent()`.
- **Empty** → call `game.collect_adjacent()` exactly as today and return
  `true`. The engine logs its own refusal and spends no turn. Do **not** set a
  `status_line` here: duplicating that sentence in app-core is the trap the
  spec calls out, and a `status_line` copy of an engine message reads as the
  key doing nothing.
- **Non-empty** → write `collect_rows`, write `collect_basket` as a zero `Vec`
  of the same length, set `self.mode = Mode::Collect`, and return `false` —
  opening a screen is not an action and must not tick.

Both new fields are cleared wherever the screen is left, in Task 2.3.

- [ ] **Step 1: Write the failing tests**

Three:

1. Standing beside a structure with stock, `c` moves the app to
   `Mode::Collect`, `collect_rows` matches `Game::collectable_adjacent()`, and
   `collect_basket` is all zeroes of the same length.
2. Opening the screen spends **no** turn — compare the game's tick counter
   across the keypress.
3. The existing refusal is intact: `c` with nothing adjacent leaves the mode
   at `Mode::Playing`. The existing test at
   `crates/app-core/src/tests/playing.rs:106` already asserts this reaches
   `Game::collect_adjacent`; leave it untouched and assert the mode here.

- [ ] **Step 2: Run and watch them fail**

`cargo test -p feral-processes-app-core collect`

- [ ] **Step 3: Implement**

Add the variant, the two fields, the module, the dispatch arm, and the new `c`
arm. `handle_collect_key` handles `GameKey::Esc` only for now — set
`Mode::Playing`, clear both fields.

The `Mode` variant will make several matches non-exhaustive. They are
exhaustive on purpose; the compiler names every site. Add the arm at each
rather than reaching for `_ =>`.

- [ ] **Step 4: Run and watch them pass**

- [ ] **Step 5: Mutation-prove**

Make the `c` arm return `true` in the non-empty branch and confirm test 2
fails. Make the empty branch set the mode anyway and confirm test 3 fails.

- [ ] **Step 6: Commit**

Subject: `feat(ui): c opens a window instead of emptying the shelves`

### Task 2.2: Editing the basket

**Files:**
- Modify: `crates/app-core/src/app/collect.rs`
- Test: `crates/app-core/src/tests/collect.rs`

**Interfaces:**
- Produces: the full key table in `handle_collect_key`, except Enter (Task
  2.3).
- Consumes: `App::menu_selected`, `App::collect_rows`, `App::collect_basket`.

The table, from the spec:

| Key | Effect |
|-----|--------|
| `Up` / `Down` | move the row cursor, wrapping |
| digit `0`–`9` | append to the highlighted row's amount, clamped to that row's available |
| `Backspace` | drop the last digit of the highlighted row's amount |
| `Left` / `Right` | −1 / +1 on the highlighted row, saturating at 0 and at available |
| `[A]` | fill every row to its maximum |
| `[N]` | clear the basket to all zeroes |

Three things that are easy to get wrong and are each a test below:

- **The cursor must move through `App::menu_selected`**, not a new field.
  That is the field `render/mod.rs` hands every popup drawer and the field
  `popup_layout`'s scrolling follows, so using it is what gives this screen
  scrolling for free. Move it with `App::scroll(key, len)`, the existing
  helper — it is `selected_index` with the resolved row discarded, so Up/Down
  move and digits resolve to nothing you use.
- **Digits edit numerically, not through a string buffer.** `n = (n * 10 + d)`
  then clamp to the row's available. Use `saturating_mul`/`saturating_add`: a
  player holding a digit key must not overflow `u32` on the way to the clamp.
  Backspace is `n / 10`.
- **`[A]` and `[N]` are uppercase**, matching the reserved-uppercase
  convention (`[M]`, `[C]`, `[S]`). Lowercase `a`/`n` must do nothing.

- [ ] **Step 1: Write the failing tests**

Seven:

1. Up/Down move `menu_selected` and wrap at both ends.
2. A digit sets the highlighted row's amount and leaves every other row at
   zero. Assert the whole basket, not just the edited entry.
3. Typing digits that would exceed the row's available clamps to available —
   e.g. `5` then `0` against 12 available leaves 12, not 50 and not 5.
4. Backspace drops a digit; Backspace on 0 stays 0.
5. Left/Right step by one and saturate at 0 and at available.
6. `[A]` fills every row to its row's own available (not to a shared
   maximum); `[N]` returns every row to zero.
7. Lowercase `a` and `n` change nothing.

- [ ] **Step 2: Run and watch them fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run and watch them pass**

- [ ] **Step 5: Mutation-prove**

Remove the clamp and confirm test 3 fails. Change `[A]` to fill every row to
the *first* row's available and confirm test 6 fails — a shared maximum passes
any single-row fixture, so test 6 must use two rows with different
quantities. Swap `saturating_mul` for `*` and hold a digit key past ten
presses in a test: a debug build panics on overflow, which is the failure
the saturating form exists to prevent. Restore it.

- [ ] **Step 6: Commit**

Subject: `feat(ui): the collect window takes a quantity per row`

### Task 2.3: Commit and abandon

**Files:**
- Modify: `crates/app-core/src/app/collect.rs`
- Test: `crates/app-core/src/tests/collect.rs`

**Interfaces:**
- Produces: the `Enter` arm, and the shared teardown both `Enter` and `Esc`
  use.
- Consumes: `Game::collect_items` (Task 1.2).

`Enter` zips `collect_rows` with `collect_basket`, drops the zero entries, and
calls `game.collect_items(&pairs)`. Then clears both fields and returns to
`Mode::Playing`.

An all-zero basket must **not** reach the engine at all — return through the
same teardown `Esc` uses. Phase 1 already makes an all-zero request a no-op,
so calling through would be harmless today, but the screen should not depend
on that: two places would then both have to keep the no-op true.

Set no `status_line`. The engine has logged the haul, and the log pane is
where a haul is reported.

- [ ] **Step 1: Write the failing tests**

Five:

1. Enter with a basket takes exactly that basket: the player's `Inventory`
   gains those amounts, the structures keep the remainder, and the mode
   returns to `Mode::Playing`.
2. Enter spends exactly one turn, whatever the basket's size — assert with a
   two-item basket, since a one-item basket cannot tell one tick from
   per-item ticking.
3. Enter on an all-zero basket takes nothing, spends no turn, and returns to
   `Mode::Playing`.
4. Esc takes nothing and spends no turn.
5. Both exits clear `collect_rows` and `collect_basket`, so reopening the
   screen cannot show a stale shelf.

- [ ] **Step 2: Run and watch them fail**

- [ ] **Step 3: Implement**

- [ ] **Step 4: Run and watch them pass**

- [ ] **Step 5: Mutation-prove**

Skip the field clearing and confirm test 5 fails. Move `self.tick()`-equivalent
behaviour by calling `collect_items` per row instead of once and confirm test
2 fails. Test 3 cannot be mutation-proved from this side — Phase 1 already
makes an all-zero request a no-op, so dropping the zero filter here changes
nothing observable. That is expected: the filter is here so the screen does
not *depend* on the engine's defensiveness, and the engine's own all-zero test
(Task 1.2, test 5) is what proves the behaviour. Note this in the commit body
rather than inventing a test that passes either way.

- [ ] **Step 6: Phase gate**

`cargo test --workspace`. Green before Phase 3.

- [ ] **Step 7: Commit**

Subject: `feat(ui): the collect window commits its basket in one action`

---

# Phase 3 — gui: drawing the window

Deliverable: the screen is visible and its rows are proved to fit.

### Task 3.1: The screen

**Files:**
- Create: `crates/gui/src/render/collect.rs`
- Modify: `crates/gui/src/render/mod.rs` — declare the module, and add the
  `Mode::Collect` arm to the draw match (the `Mode::Craft` arms near line 650
  are the shape to follow)

**Interfaces:**
- Produces: `pub(crate) fn draw_collect(...)`. Take the signature shape from
  `draw_craft_menu` in `crates/gui/src/render/crafting.rs` — same painter and
  metrics parameters, plus `app.collect_rows`, `app.collect_basket` and
  `app.menu_selected`. Pass slices, not the `App`.
- Consumes: `Game::item_name` for the display name, `render/popup.rs`'s
  existing popup and row helpers.

One row per entry in `collect_rows`: the item's name, and a suffix column
reading `taken / available`.

Three rules that are not negotiable, each with a reason:

- **The suffix goes in the row's own suffix column**, not `format!`ed into the
  name string. `CLAUDE.md`'s items section records six screens that made that
  mistake: measuring a row without the column makes `suffix_x` drop its suffix
  onto the row's own tail, and a wrap then budgets for a row narrower than it
  draws.
- **Name nothing from a graphics library.** `crates/gui/src/paint.rs` is the
  only file that does. Draw through `Painter`.
- **Take the pane origin from the caller.** A literal `0.0` draws under the
  stock strip and no test sees it.

The page scrolls, because `menu_selected` drives it and `popup_layout` keeps
the selected row visible. So it needs no height census — but say so in a doc
comment, or the next person adds one by analogy with the memories page.

Also draw the key legend: the six keys from Task 2.2 plus Enter and Esc.

- [ ] **Step 1: Write the screen**

No test first here — this step is drawing, and its test is Task 3.2's census.
Do not skip 3.2.

- [ ] **Step 2: Check it compiles and the suite is unmoved**

`cargo test -p feral-processes-gui` and `cargo clippy --workspace`.

- [ ] **Step 3: Commit**

Subject: `feat(ui): draw the collect window`

### Task 3.2: The width census

**Files:**
- Test: the gui crate's render tests, beside the existing popup-width censuses
  — find them with `rg -n "overflows_its_popup" crates/gui/`

**Interfaces:**
- Produces: a test named `no_collect_row_overflows_its_popup`.

`draw_row` clips vertically only, so an over-wide row is drawn off the panel
in silence. Row width **is** testable headlessly: `paint::with_painter`
measures real text. Follow whichever existing census the `rg` above finds, and
match its structure rather than inventing one.

Build the widest row the shipped assets can produce: the longest item name in
`assets/items/` paired with the widest plausible `taken / available` figure.
Derive the name from the real `ItemDb` rather than hardcoding a string — a new
item file must move this test, which is the whole point of a census.

**A width test that skips non-`Item` rows measures nothing here.** Check that
the rows you are measuring are the rows the screen actually builds; if the
census reads a different row type than `draw_collect` emits, it passes against
no fix at all.

- [ ] **Step 1: Write the failing test**

Temporarily widen the row (e.g. pad the suffix with 40 spaces) so the census
has something to fail against, and confirm it fails.

- [ ] **Step 2: Remove the padding and watch it pass**

- [ ] **Step 3: Phase gate**

`cargo test --workspace`.

- [ ] **Step 4: Commit**

Subject: `test(ui): no collect row overflows its popup`

---

# Phase 4 — Documentation

Deliverable: nothing in the repo still claims `c` takes everything.

### Task 4.1: The four documents

**Files:**
- Modify: `assets/help/20-controls.md:25` — currently
  `- c — collect from adjacent structures`
- Modify: `assets/help/60-your-base.md:54` — currently
  `- c collects from everything adjacent to you.`
- Modify: `docs/seams.md` — a new entry under **The base**
- Modify: `CLAUDE.md` — the matching one-liner under **The base**, beside the
  `Stock`/`ORTHOGONAL` line already there
- Modify: `CHANGELOG.md`

Both help lines are now false and are the player's only written account of the
key. Rewrite them to say `c` opens a window and that taking everything is
still one key inside it. Keep the file's existing voice — these are prose
pages, five block rules and no more; read `assets/help/README.md` before
editing.

The `docs/seams.md` entry carries the argument, `CLAUDE.md` carries the rule
and the trap. The rule: *one reach rule and one taking path — `ORTHOGONAL` via
`adjacent_stock`, and `hauling::take_from` for every unit that leaves a
buffer.* The trap: *the neighbour scan is sorted by `(x, y)` because a partial
take across two neighbours holding the same item must drain them in the same
order every run; take-all could not see this, which is why the code had no
sort for so long.*

`CHANGELOG.md`'s preamble is the one statement of the version policy — read
it and let it decide the digit. This is a feature and no save stops loading.

Note `CLAUDE.md` and `AGENTS.md` are gitignored twins with no tracking to
catch drift: edit `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`.

- [ ] **Step 1: Grep for anything else the change falsifies**

```bash
rg -in "collect" assets/help/ docs/ CHANGELOG.md README.md
```

Fix every claim that is now wrong. Do **not** touch `docs/manual.md`, the root
`README.md`, or `TODO.md` — all three are carved out of the doc obligation.

- [ ] **Step 2: Write the four edits**

- [ ] **Step 3: Verify the help pages still parse**

The help database skips a malformed page with a warning rather than failing,
so a broken page is silent. Run the help asset censuses:
`cargo test -p feral-processes-engine help`.

- [ ] **Step 4: Final gate**

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
cargo test -p feral-processes-engine balance_sim
git diff --quiet assets/ || echo "asset changes present — confirm they are the two help pages only"
```

- [ ] **Step 5: Commit**

Subject: `docs: c opens the collect window`

---

# Done

Report to the user with: the commands actually run and their output, the
mutation table (each new test, the mutation applied, that it failed without
the fix and passed with it), and anything left out and why.

**Do not push.** The user asks for pushes, and the release sequence — bump,
changelog section, tag, push — belongs to the merge, not to the branch.

**Do not offer a playtest.** The user works remotely and cannot run the game.
Say plainly that the suite is green and that no part of this has been seen on
a screen.
