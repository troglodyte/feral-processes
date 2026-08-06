# Three More Hidden Keys — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Add three easter eggs in the shape of the wielded program (`W`) —
`Z` to listen in the Stack, `T` to taunt on the battle roster, `T` to throw a
consumable in the battle item picker — none of them named by any screen.

**Architecture:** Each egg is an uppercase key intercepted ahead of
`selected_index`'s digits-then-lowercase row scheme, on a screen that already
exists, calling one new `Game` method that returns `Result<(), String>` the
way `Game::adopt_orphan` does. No new `Mode`, no new saved state, no
`SAVE_FORMAT_VERSION` bump.

**Spec:** `docs/superpowers/specs/2026-08-06-easter-eggs-design.md` — read it
first. It carries the reasoning; this plan carries the file list.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine only), `ron` assets.

## Global Constraints

- **Nothing on any screen may name these keys.** Task 5 is what holds it;
  don't add help text in tasks 1-4.
- **Never draw from `resources::GameRng`** in any of this work. Both the
  crash-log line and the taunt line are chosen deterministically. A cosmetic
  draw shifts the shared stream and silently rewrites seeded combat tests.
- **New `.ron` schema fields are `#[serde(default)]`**, and the matching
  `assets/*/README.md` is updated in the same task.
- **A malformed `.ron` is skipped with a logged warning**, never a panic.
  Follow `AbilityDb::load_dir` (`crates/engine/src/abilities.rs:528`).
- **Tuning values go in `crates/engine/src/tuning.rs`** as documented `pub
  const`s in the right labelled section, never inline in a formula.
- **No version bump or `CHANGELOG.md` edit on this branch.** Both happen once
  at the merge — see "At the merge" at the end.
- Gates after every task: `cargo fmt`, `cargo clippy --workspace`, and the
  targeted test. `cargo test --workspace` before the branch is called done.

---

### Task 1: `Z` — listening for unspent features

The direction-finding half of the listen key. The crash-log half is Task 2.

**Files:**
- Modify: `crates/engine/src/tuning.rs` — add `TRACE_PER_LISTEN` to the
  "The Stack: Trace" section (around line 631), beside `TRACE_PER_SEAL`.
- Create: `crates/engine/src/game/listen.rs` — the whole reading.
- Modify: `crates/engine/src/game/mod.rs` — declare the module.
- Modify: `crates/app-core/src/app/playing.rs` — bind `Z` in
  `handle_stack_key`'s match (the block at ~line 182).
- Test: `crates/engine/src/tests/` — new `listen.rs`, declared in
  `crates/engine/src/tests/mod.rs`.
- Test: `crates/app-core/src/tests/` — extend the existing Stack-key tests.

**Interfaces:**
- Produces: `Game::listen(&mut self) -> Result<(), String>` — public, logs
  its reading with `MessageKind::Outcome`, advances the world by one turn,
  and raises Trace. Task 2 extends this same method; Task 3 and 4 do not
  touch it.
- Produces: `tuning::TRACE_PER_LISTEN: u32 = 3`.
- Consumes: `Game::stack_pos() -> Option<StackPos>`
  (`game/stack.rs`), `FrameMemory::cache_unopened / seal_open /
  orphan_present / lair_cleared` (`game/stack_features.rs`, all
  `pub(crate)`), `Game::raise_trace` (`game/trace.rs:88`).

**Behaviour to build:**

Refusal is `stack_pos()` returning `None` → `Err`, costing nothing. **Not
`require_surface`** — this reads `Locale::Stack`'s own coordinates, which is
the `Phase`/`Jump` case. Getting this wrong is the one architectural mistake
available here.

Scan the current frame for cells that are *unspent*: an unopened `Cache`, an
unburnt `Seal`, a present `Orphan`, an uncleared `Lair`. `Fault` and
`Corruption` are deliberately excluded — neither is used up, so neither has a
`FrameMemory` record and "unspent" is not a question about them.

Pick the nearest by Manhattan distance (the frame moves 4-way). Report it as
a bearing **relative to the party's facing**, not compass north, plus the
distance in steps. Name only the dominant relative axis, ties preferring
forward/back.

The rotation is the one place a sign error is easy and invisible, so it is
worth spelling out. With `(dx, dy)` the target minus the party in frame
coordinates and `Dir::North` meaning `-y`:

```rust
let (fwd, right) = match facing {
    Dir::North => (-dy, dx),
    Dir::East => (dx, dy),
    Dir::South => (dy, -dx),
    Dir::West => (-dx, -dy),
};
```

Then `fwd > 0` is ahead, `right > 0` is to the right, and the dominant axis
is whichever has the larger absolute value.

**Trace and the turn are charged whether or not anything was found.** A swept
frame reporting silence is the information the turn bought; free-when-empty
turns `Z` into a zero-risk sweep to mash on every tile.

**Tests (engine):**
1. A frame with one unopened cache: the reading names its direction, and the
   distance matches the Manhattan distance. Build the frame through the
   normal `Game::enter_frame` path rather than hand-placing cells.
2. The *same* cache read from two different facings gives two different
   bearings. This is the test that catches a rotation sign error; assert the
   two specific bearings, not merely that they differ.
3. After emptying that cache, listening reports silence.
4. Trace rises by `TRACE_PER_LISTEN` in both case 1 and case 3 — the charge
   is unconditional.
5. Listening on the surface returns `Err` and leaves Trace and the turn
   counter untouched.

Fixtures are in `crates/engine/src/tests/support.rs` — look there before
writing a new one.

**Tests (app-core):** `Z` underground calls through and counts as an action
(the world advanced); `Z` on the surface map does nothing and costs no turn.

- [ ] **Step 1:** Write the five engine tests above. They will not compile —
      `Game::listen` does not exist yet.
- [ ] **Step 2:** `cargo test -p feral-processes-engine listen` — expect a
      compile failure naming `listen`.
- [ ] **Step 3:** Add `TRACE_PER_LISTEN`, then implement `Game::listen` in
      the new module.
- [ ] **Step 4:** `cargo test -p feral-processes-engine listen` — expect
      PASS.
- [ ] **Step 5:** Write the two app-core tests, watch them fail, bind `Z` in
      `handle_stack_key`, watch them pass. The arm sets `acted = true` on
      `Ok` and `refusal` on `Err`, exactly like the `'o'` arm above it.
- [ ] **Step 6:** `cargo fmt && cargo clippy --workspace`, then commit.

---

### Task 2: The crash log

The second reading of the same `Z`: standing on `Fault` or `Corruption`
reads that place's crash log instead of pointing at anything.

**Files:**
- Create: `crates/engine/src/crash_logs.rs` — `CrashLogDef` and `CrashLogDb`.
- Modify: `crates/engine/src/lib.rs` — declare the module.
- Modify: `crates/engine/src/game/lifecycle.rs:1018-1031` — load the new
  directory alongside the other seven databases, folding its warnings in.
- Modify: `crates/engine/src/game/listen.rs` — branch on the standing cell.
- Create: `assets/crash_logs/*.ron` — the shipped lines.
- Create: `assets/crash_logs/README.md` — the schema, in the shape of
  `assets/species/README.md`.
- Test: `crates/engine/src/tests/listen.rs` — extend.

**Interfaces:**
- Produces: `CrashLogDb::load_dir(dir: &Path) -> std::io::Result<(Self,
  Vec<String>)>` — same signature as `AbilityDb::load_dir`.
- Produces: `CrashLogDb::line_for(&self, zone: u32, depth: u32, cell: (i32,
  i32)) -> Option<&str>`.
- Consumes: Task 1's `Game::listen`.

**Schema:** one `.ron` file per entry, each an id and its lines:

```ron
(
    id: "orphaned_write",
    lines: [
        "...",
        "...",
    ],
)
```

**The ordering trap:** `std::fs::read_dir` returns entries in no defined
order, so the pooled line list **must be sorted by `id`** after loading. Without
that, the same cell reads a different line between runs and after a reload —
the same class of bug the `assembler_system` position sort exists to prevent,
and a test should assert the sort by loading a directory with at least two
files and checking a fixed cell reads a fixed line.

**Which line:** derived from the place, never from `GameRng`, so it survives
a save/load — the `Game::orphan_species` rule. Index into the pooled lines
with a fixed mix of the four inputs:

```rust
let idx = (zone as i64 * 31 + depth as i64 * 17 + x as i64 * 7 + y as i64 * 3)
    .rem_euclid(lines.len() as i64) as usize;
```

`rem_euclid` rather than `%` because frame coordinates are `i32`.

**Tests:**
1. Listening on a `Corruption` cell logs a crash-log line rather than a
   bearing, and still charges `TRACE_PER_LISTEN` and a turn.
2. The same cell reads the same line after a save/load round trip. This is
   the test that would fail if anyone reached for `GameRng`.
3. Two different cells in the same frame can read different lines (the index
   actually varies with position).
4. A malformed `.ron` in the directory is skipped with a warning and the
   other files still load. Follow the existing malformed-asset tests for the
   other databases.
5. An empty crash-log directory leaves `Z` working — the cell falls back to
   the bearing reading rather than panicking on a modulo by zero.

- [ ] **Step 1:** Write tests 1-5. They will not compile.
- [ ] **Step 2:** `cargo test -p feral-processes-engine listen` — expect a
      compile failure.
- [ ] **Step 3:** Build `crash_logs.rs`, wire it into `lifecycle.rs`, write
      the shipped `.ron` files and the README, and add the branch to
      `Game::listen`.
- [ ] **Step 4:** `cargo test -p feral-processes-engine listen` — PASS.
- [ ] **Step 5:** `cargo fmt && cargo clippy --workspace`, then commit.

---

### Task 3: `T` — taunting

**Files:**
- Modify: `crates/engine/src/species.rs:166` — add the `taunts` field to
  `SpeciesDef`.
- Modify: `assets/species/README.md` — document it.
- Modify: `crates/engine/src/resources.rs` — add the taunt counter resource.
- Create: `crates/engine/src/game/taunt.rs`; declare it in `game/mod.rs`.
- Modify: some `assets/species/*.ron` — lines for a handful of the starter
  species (drone, glitch, sprite, sub_process at minimum).
- Modify: `crates/app-core/src/app/battle.rs:32` — intercept `T`.
- Test: `crates/engine/src/tests/` — new `taunt.rs`.
- Test: `crates/app-core/src/tests/battle.rs` — extend.

**Interfaces:**
- Produces: `Game::taunt(&mut self) -> Result<(), String>` — logs one line
  with `MessageKind::Info`, refuses when no battle is active. Costs no turn
  and resolves no round.
- Produces: `SpeciesDef::taunts: Vec<String>`, `#[serde(default)]`.
- Produces: `resources::TauntCount(pub u32)`.

**Behaviour to build:**

The speaker is the front living party companion; with an empty party the
player says it. The key must never silently do nothing, so a species with no
`taunts` falls back to a generic engine-side line.

`MessageKind::Info` is deliberate: `retain_outcomes_since_battle` keeps only
`Outcome`, `Loot`, `LevelUp` and `Raid`, so an `Info` taunt is pruned when
the battle ends and does not follow the player onto the map. That is the
right behaviour, not an oversight.

**Which line:** `TauntCount` increments per press and indexes the speaker's
lines with `rem_euclid`, so repeated presses cycle. **Do not use `GameRng`** —
a key a player might press twenty times in a fight is the worst possible
place to advance the shared stream.

`TauntCount` **must not be added to `save.rs`'s save struct.** Resources are
persisted by being explicit fields there (see `stack_memory` and the Trace
field around `save.rs:272-280`), so simply not adding it is what keeps it
transient. No `SAVE_FORMAT_VERSION` bump.

**The interception point** is `handle_battle_key` immediately after the
`let GameKey::Char(raw) = key else { return }` destructure at `battle.rs:32`
and **before** the party-command lookup, matching on `raw == 'T'` rather than
the folded `c`. Verified free: party commands are `A`/`D`/`j`
(`combat.rs:900`) and per-slot actions are `a`/`d`/`s`/`u` (`combat.rs:841`),
so neither `T` nor the `t` the fold would retry hits anything.

**Tests (engine):**
1. A species with authored `taunts` speaks one of its own lines.
2. A species with none still produces a line (the generic fallback).
3. Taunting twice in a battle produces two different lines from a species
   with at least two.
4. Taunting does not advance `GameRng`: run a seeded fight to a fixed
   outcome, then run the identical seeded fight with a taunt in the middle,
   and assert the outcomes are identical.
5. Taunting outside a battle returns `Err`.
6. An existing species `.ron` with no `taunts` key still parses (the
   `#[serde(default)]` obligation).

**Tests (app-core):** `T` on the battle roster does not commit an action for
the active slot and does not resolve the round.

- [ ] **Step 1:** Write engine tests 1-6.
- [ ] **Step 2:** `cargo test -p feral-processes-engine taunt` — expect
      failure.
- [ ] **Step 3:** Add the field, the README entry, the resource, `Game::taunt`
      and the shipped lines.
- [ ] **Step 4:** `cargo test -p feral-processes-engine taunt` — PASS.
- [ ] **Step 5:** Write the app-core test, watch it fail, add the intercept,
      watch it pass.
- [ ] **Step 6:** `cargo fmt && cargo clippy --workspace`, then commit.

---

### Task 4: `T` — throwing

**Files:**
- Modify: `crates/engine/src/tuning.rs` — `THROWN_ITEM_DAMAGE`.
- Create: `crates/engine/src/game/throw.rs`; declare it in `game/mod.rs`.
- Modify: `crates/app-core/src/app/battle.rs:327` — intercept `T` in
  `handle_battle_item_key`.
- Test: `crates/engine/src/tests/` — new `throw.rs`.
- Test: `crates/app-core/src/tests/battle.rs` — extend.

**Interfaces:**
- Produces: `Game::throw_item(&mut self, item: &ItemId) -> Result<(),
  String>`.
- Produces: `tuning::THROWN_ITEM_DAMAGE: i32 = 1`.
- Consumes: `Game::battle_usable_items() -> Vec<ItemId>`
  (`combat.rs:724`), `Game::apply_damage` (`game/combat_damage.rs:22`,
  `pub(crate)`).

**Behaviour to build:**

Consumes one unit of `item` from the player's `Inventory`, applies damage to
the front living creature of the first living wild group, and logs a line
naming what bounced off. Refuses when no battle is active or the item is not
in cargo.

**Resolves immediately: no round cost, and no new `ActionKind`.** An action
kind would have to appear in `battle_action_options`, which is the list both
renderers build the prompt from — the secret would be printed on screen.
That is the whole reason this is not modelled as an action.

**A throw must not take a target below 1 HP.** `apply_damage` floors at 0 and
detects death, and a kill resolving from outside the round loop would end a
battle next to `BattleState::planned`'s positional indexing into `Party`.
Clamp at the call site — pass `min(THROWN_ITEM_DAMAGE, hp - 1).max(0)` — and
when that is 0, still consume the item and log the bounce. Mitigation inside
`apply_damage` only ever reduces the number further, so the clamp is
sufficient.

Damage goes through `apply_damage` rather than writing `Stats::hp`, because
that is the one path that lowers HP and anything watching damage must see
this too.

**Tests (engine):**
1. Throwing consumes exactly one unit and leaves the rest of the stack.
2. Throwing at a target above 1 HP lowers its HP.
3. Throwing at a target on exactly 1 HP leaves it on 1 HP and the battle
   running — `Game::has_active_battle()` is still true.
4. Throwing an item not in cargo returns `Err` and consumes nothing.
5. Throwing outside a battle returns `Err`.

**Tests (app-core):** `T` in the item picker throws the highlighted row
rather than falling through to the row shortcuts, and does not commit a
`UseItem` action.

- [ ] **Step 1:** Write engine tests 1-5.
- [ ] **Step 2:** `cargo test -p feral-processes-engine throw` — expect
      failure.
- [ ] **Step 3:** Add `THROWN_ITEM_DAMAGE` and `Game::throw_item`.
- [ ] **Step 4:** `cargo test -p feral-processes-engine throw` — PASS.
- [ ] **Step 5:** Write the app-core test, watch it fail, add the intercept
      ahead of the `selected_index` call at `battle.rs:338`, watch it pass.
- [ ] **Step 6:** `cargo fmt && cargo clippy --workspace`, then commit.

---

### Task 5: Keeping them hidden

The omission is invisible without something holding it — the next person to
write a help string has no way to know they are breaking a feature.

**Files:**
- Create: `crates/engine/EASTER_EGGS.md` — the crate root, **not** `src/`, so
  it is not mistaken for module documentation and no player-facing page links
  to it.
- Modify: `crates/gui/src/render/meta.rs` — add the assertion to the inline
  `#[cfg(test)]` module, following the pattern `render/party.rs` uses for
  `W`.
- Modify: `crates/engine/src/tests/` — the key-claim assertion.

**Interfaces:** none. This task adds no production code.

**`EASTER_EGGS.md` contents:** the four keys — `W` (companion screen, wield),
`Z` (Stack, listen), `T` (battle roster, taunt), `T` (battle item picker,
throw) — one line each on what they do, and the standing rule that no help
text may name them, with a pointer at the two tests below.

**Test 1 (gui):** `render/meta.rs::draw_help` is the *only* screen in the game
that lists key bindings — one `Vec` of rows covering the map, the Stack,
trading and battle. Assert that no row contains `W`, `T` or `Z` as a
standalone whitespace-delimited token. That is the binding idiom this screen
uses (`s save`, `L history`, `A all attack`), so it catches a real
documentation of the key while staying satisfiable: the rows are full of
those letters inside ordinary words, and of the lowercase `t` that
legitimately binds trade. Assert on tokens, never on substrings.

**Test 2 (engine):** no `ActionOption` from `battle_action_options` and no
`PartyCommand` from `battle_party_commands` has `key` equal to `W`, `T` or
`Z`. Nothing claims them today; this is what fails if a future battle action
does, which would print the letter in the prompt both renderers build from
that list.

- [ ] **Step 1:** Write both assertions and watch them pass against the
      current code — they are guards, not red-green, so confirm they *can*
      fail by temporarily adding `T taunt` to a `draw_help` row and seeing
      test 1 catch it. Remove the row afterwards.
- [ ] **Step 2:** Write `crates/engine/EASTER_EGGS.md`.
- [ ] **Step 3:** `cargo test --workspace` — the full-suite gate for the
      whole branch, not just this task.
- [ ] **Step 4:** `cargo fmt && cargo clippy --workspace`, then commit.

---

## Before calling it done

- `cargo test --workspace` green.
- `cargo test -p feral-processes-engine balance_sim` — `tuning.rs` gained two
  constants, so run the balance gate even though neither feeds a curve. A
  moved curve would mean something unintended got wired in.
- **Play it.** A green suite is not evidence that a hidden key feels like a
  discovery. `FERAL_DEV_REVEAL=1 cargo run -- --template stack` puts you in a
  frame with the whole map drawn, which is the fastest way to reach a
  `Corruption` cell and an unopened cache without walking a maze.

## At the merge

Per the release-per-change rule, and **only** at the merge — not on this
branch:

- Bump the workspace version in the root `Cargo.toml`. **Patch, not minor.**
  Read `CHANGELOG.md`'s preamble rather than trusting ordinary semver
  instinct: while the project is `0.x`, minor is reserved for a *breaking*
  change, which here means a `save::SAVE_FORMAT_VERSION` bump where existing
  saves stop loading. Nothing in this branch touches the save format, so a
  feature addition is a patch.
- Add the `## X.Y.Z` section to `CHANGELOG.md`. Judgement call worth making
  deliberately: the changelog is player-facing, and naming the keys there
  would undo Task 5. The precedent is stronger than "be vague" — the wielded
  program has **no `CHANGELOG.md` entry at all**; `rg -i wield CHANGELOG.md`
  returns nothing. Ask the user which they want here rather than picking:
  silence matches the precedent, an unkeyed description does not.
- Annotated `vX.Y.Z` tag. `git push` alone does not send tags; `--follow-tags`
  does.
