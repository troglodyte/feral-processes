# Entity memories — Phase 1: Identity

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every owned program a stable `ProgramId` that survives a
save/load, minted at the single roster barrier, so a later phase can write
a memory that is about one specific program.

**Architecture:** A `u32` newtype component, minted from a saved counter
resource at `Game::roster_parts()` — the one barrier all four doors into
the roster pass through. Two additive `#[serde(default)]` save fields. A
legacy save carries id `0` for everyone and mints fresh ids on load.

**Tech Stack:** Rust, `bevy_ecs` 0.19, serde/RON saves.

**Spec:** `docs/superpowers/specs/2026-08-21-entity-memories-design.md` —
read section 3 (*Identity*) before starting. This plan implements that
section and nothing else.

## Global Constraints

- **No `SAVE_FORMAT_VERSION` bump.** Both new save fields are additive
  behind `#[serde(default)]`, which the field-named RON format supports.
  If you find yourself needing a bump, stop — you have changed a field's
  meaning rather than added one.
- **No new RNG draws.** Nothing in this phase may touch `GameRng` or a
  local `StdRng`. Minting is a counter.
- **Id `0` is the unassigned sentinel.** Real ids start at 1.
- **Only owned programs get an id.** Wild and hostile creatures never pass
  through `roster_parts`, so they keep `0` and carry no component.
- Follow the repo's comment discipline: comments explain *why*, never
  *what*.
- Gates for every task: `cargo test -p feral-processes-engine <name>`
  while iterating; `cargo test --workspace`, `cargo clippy --workspace`
  and `cargo fmt` before the phase is called done.

**Evidence standard.** Every test in this plan must be mutation-proved:
delete the fix, run the test, watch it fail, restore the fix. A test that
passes with its fix removed is coverage-shaped and worse than nothing.
Record the mutation you applied and the failure you saw. This repo has
shipped vacuous tests twice.

**Known trap — a new `Resource` shifts query iteration order.** Registering
`NextProgramId` can make an unrelated test in an untouched subsystem fail,
because bevy's query iteration order is not stable and some test is
implicitly relying on it. If that happens it is a latent unsorted-query
test, not a regression you introduced. Fix that test's incidental coupling
(sort, or assert on a set); do not reseed it and do not revert the
resource.

---

## File structure

| File | Responsibility in this phase |
|---|---|
| `crates/engine/src/components.rs` | `ProgramId(pub u32)` — the per-program identity |
| `crates/engine/src/resources.rs` | `NextProgramId(pub u32)` — the counter |
| `crates/engine/src/game/spawning.rs` | `roster_parts` widens to mint |
| `crates/engine/src/game/lifecycle.rs` | resource registration (`:114`), the creature save site (`:1069`), the creature load loop (`:661`), and `grant_starting_program` (`:1486`) |
| `crates/engine/src/game/combat_rewards.rs` | `attempt_decompile` call site (`:904`) |
| `crates/engine/src/game/party.rs` | `fuse_companions` call site (`:888`) |
| `crates/engine/src/save.rs` | `CreatureSave::program_id`, `SaveData::next_program_id` |
| `crates/engine/src/tests/support.rs` | `spawn_tamed` call site (`:1150`) |
| `crates/engine/src/tests/memories.rs` | **create** — this phase's tests |
| `crates/engine/src/tests/mod.rs` | declare `mod memories;` (alphabetical: after `listen`, before `message_log`) |

---

## Task 1: Mint an id at the roster barrier

**Files:**
- Modify: `crates/engine/src/components.rs`, `crates/engine/src/resources.rs`,
  `crates/engine/src/game/spawning.rs:321`,
  `crates/engine/src/game/lifecycle.rs:114`,
  and the four call sites listed in the file table
- Create: `crates/engine/src/tests/memories.rs`
- Modify: `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Produces: `components::ProgramId(pub u32)` — `Component, Clone, Copy,
  Debug, PartialEq, Eq, Hash`. `resources::NextProgramId(pub u32)` —
  `Resource`, no `Default` derive (see step 3). `Game::roster_parts(&mut
  self) -> (Tamed, Experience, PowerReserve, ProgramId)`.
- Consumes: nothing from an earlier task.

- [ ] **Step 1: Write the failing tests**

In the new `crates/engine/src/tests/memories.rs`, three tests. Write the
assertions; do not write the implementation.

1. `two_programs_through_different_doors_take_different_ids` — take a
   program through `grant_starting_program` (a fresh `Game::new` already
   has one) and another through the `spawn_tamed` fixture, and assert
   their `ProgramId`s differ and are both non-zero. This is the test that
   says minting is per-call rather than a constant.
2. `a_fused_program_takes_a_fresh_id` — fuse two companions and assert the
   result's id is neither parent's and is non-zero. `fuse_companions` is
   the door that hand-writes its own component list, so it is the one that
   can silently skip a widened tuple; the party tests already have a
   fusion fixture to copy the setup from.
3. `a_wild_program_carries_no_id` — spawn a wild creature and assert
   `world.get::<ProgramId>()` is `None`. This is what pins minting to the
   roster barrier rather than to creature spawning generally.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine memories`

Expected: compile failure — `ProgramId` does not exist. That is a valid
red for this step.

- [ ] **Step 3: Add the component and the counter**

`ProgramId` in `components.rs`, beside `Potential` and the other
per-creature identity components. Doc comment says what the spec's section
3 says: entity ids are not stable across a save round trip, so a memory
about one specific program needs this.

`NextProgramId` in `resources.rs`. **Do not derive `Default`** — a default
of `0` is the unassigned sentinel and would hand the first program the
sentinel value. Give it an explicit `NextProgramId::START` at 1, and
insert it in `Game::new` beside `GameClock` (`lifecycle.rs:114`).

- [ ] **Step 4: Widen `roster_parts` and fix every call site**

`roster_parts` becomes `&mut self` — it advances the counter. All five
callers already hold `&mut self`; each takes the shape `let parts =
self.roster_parts();` before a `world.spawn`, so the mutable borrow ends
before the spawn begins and no borrow-scoping work is needed.

The compiler will name all five sites. That is the point of the shared
tuple, so do not hand-write a `ProgramId` at any of them.

Extend the function's doc comment: it currently explains why a shared
constructor exists at all, and minting is now the strongest instance of
that argument — a door that skipped it produces a program that can never
be the subject of a memory, which reads as memories being broken.

- [ ] **Step 5: Run the tests and the neighbours**

`cargo test -p feral-processes-engine memories`
`cargo test -p feral-processes-engine party spawning`

Expected: PASS. If something unrelated goes red, re-read the resource
iteration-order trap above before touching anything.

- [ ] **Step 6: Mutation-prove each of the three tests**

For each: make `roster_parts` return a constant `ProgramId(1)` and confirm
tests 1 and 2 fail; add a `ProgramId` to the wild spawn path and confirm
test 3 fails. Restore after each. Record what you changed and what failed.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/components.rs crates/engine/src/resources.rs \
        crates/engine/src/game/spawning.rs crates/engine/src/game/lifecycle.rs \
        crates/engine/src/game/combat_rewards.rs crates/engine/src/game/party.rs \
        crates/engine/src/tests/memories.rs crates/engine/src/tests/mod.rs
git commit -m "feat(roster): every owned program carries an id of its own"
```

Stage explicit paths, never `git add -A` — another agent's worktree
gitlink under `.claude/worktrees/` gets swept up otherwise.

---

## Task 2: Save the id, and mint one for a legacy save

**Files:**
- Modify: `crates/engine/src/save.rs` (`CreatureSave` at `:110`, `SaveData`
  at `:489`)
- Modify: `crates/engine/src/game/lifecycle.rs` — the save site (`:1069`),
  the load loop (`:661`), and `save()` (`:944`)
- Modify: `crates/engine/src/tests/memories.rs`

**Interfaces:**
- Consumes: `ProgramId`, `NextProgramId`, `roster_parts` from Task 1.
- Produces: `CreatureSave::program_id: u32` and `SaveData::next_program_id:
  u32`, both `#[serde(default)]`.

- [ ] **Step 1: Write the failing tests**

Four tests, appended to `crates/engine/src/tests/memories.rs`. Use
`support::scratch_assets_dir` for the save path — **not**
`std::env::temp_dir` directly. Engine fixtures leaking into `/tmp`
exhausted the tmpfs inode table once already; `scratch_assets_dir` is the
fixed pattern, and `tests/refactor.rs` around line 727 is a worked example.

1. `a_program_id_survives_a_save_and_load` — note a companion's id, save,
   load, assert the same id came back. **A RON round trip is not enough**
   and must not be the whole test: a `#[serde(skip)]` would leave a round
   trip green while the field never reaches the file. Go through
   `game.save(&path)` and `Game::load(&path, &assets)`.
2. `a_legacy_save_mints_an_id_for_every_owned_program` — save, then rewrite
   the file with every creature's `program_id` set to `0` and
   `next_program_id` absent, then load and assert every owned program has
   a distinct non-zero id. The savetool RON round trip (`dump` then `pack`)
   is the supported way to edit a save; in-test, deserialize `SaveData`,
   zero the fields, and re-serialize.
3. `an_id_already_in_the_file_is_never_minted_again` — a file mixing saved
   ids with zeros loads with the saved ids untouched and the minted ones
   distinct from them.
4. `the_counter_lands_above_every_id_seen` — after a load, minting one more
   program yields an id higher than any in the file. This is the test that
   catches a counter restored from `next_program_id` alone while the file
   held a higher id (a hand-edited or savetool-packed save).

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine memories`

Expected: compile failure on `program_id` / `next_program_id`.

- [ ] **Step 3: Add the two save fields**

Both `#[serde(default)]`. Doc-comment each with why it earned no
`SAVE_FORMAT_VERSION` bump — the neighbouring fields in `CreatureSave`
already model that comment style, and the file's own docs explain the
field-named-RON rule.

`save.rs`'s `sample_creature()` test fixture (`:983`) needs the new field
too.

- [ ] **Step 4: Write it at the save site**

At `lifecycle.rs:1069`, alongside `rarity` and `nemesis_grudges`:
`program_id: program_id.map(|p| p.0).unwrap_or(0)`, with `ProgramId` added
to that loop's query. `SaveData::next_program_id` is written in `save()`
from the resource.

- [ ] **Step 5: Mint on load, in the right order**

This is the one genuinely non-obvious part of the phase, because
`data.creatures` is moved by the `for c in data.creatures` loop at
`:661` — so the highest id in the file must be computed *before* the loop
begins:

```rust
let mut next = data
    .next_program_id
    .max(data.creatures.iter().map(|c| c.program_id).max().unwrap_or(0) + 1)
    .max(1);
```

Inside the loop, for a creature with `c.tamed` only: take `c.program_id`
when non-zero, otherwise take `next` and increment. Insert
`NextProgramId(next)` after the loop.

The `.max(1)` is not redundant with the sentinel: an empty roster gives
`unwrap_or(0) + 1 == 1` but a `next_program_id` of `0` from a legacy file
would otherwise win the `.max`.

- [ ] **Step 6: Run the tests**

`cargo test -p feral-processes-engine memories`

Expected: PASS.

- [ ] **Step 7: Mutation-prove each of the four**

Suggested mutations, one at a time, restoring between: drop
`#[serde(default)]`'s field from the save-site struct literal (test 1);
skip the mint for a zero id (test 2); mint unconditionally, ignoring
`c.program_id` (test 3); drop the `.max(highest seen)` term (test 4).
Record each.

- [ ] **Step 8: Full gate**

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

- [ ] **Step 9: Commit**

```bash
git add crates/engine/src/save.rs crates/engine/src/game/lifecycle.rs \
        crates/engine/src/tests/memories.rs
git commit -m "feat(save): a program's id survives the round trip"
```

---

## Phase 1 done when

- `cargo test --workspace` is green.
- `cargo clippy --workspace` is clean.
- Every one of the seven tests has a recorded mutation that made it fail.
- `SAVE_FORMAT_VERSION` is unchanged.
- Nothing reads `ProgramId` yet. That is expected — Phase 2's `remember`
  is its first reader. Do not invent one to make the field look used.

No `CHANGELOG.md` entry and no version bump: this is a branch commit, and
the release happens once at the merge.
