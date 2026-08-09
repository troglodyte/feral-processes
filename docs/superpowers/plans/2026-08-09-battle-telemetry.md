# Battle Telemetry — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Record what happens inside a battle to `dev-logs/battles.jsonl`, so
a hand-played fight leaves an artifact a script can read instead of only a
memory.

**Architecture:** The engine builds `Serialize` records into a
`BattleTelemetry` resource at five seams that already exist as single
paths; app-core drains that resource and appends one JSON object per line.
The `PendingProfileWrites` shape — the engine performs no file IO.

**Tech Stack:** Rust 2024, `bevy_ecs` 0.19, `serde`. **`serde_json` is added
to `crates/app-core` only.**

**Spec:** `docs/superpowers/specs/2026-08-09-battle-telemetry-design.md`

## Global Constraints

- **`serde_json` goes in `crates/app-core`, never `crates/engine`.** The
  engine derives `Serialize` and hands over values; app-core is the only
  crate that turns one into a string. The engine's dependency list does not
  grow — `cargo check --workspace` at ~1s is the property being protected.
- **No save-format change.** `save::SAVE_FORMAT_VERSION` is not touched.
- **Off by default, and free when off.** `FERAL_DEV_LOG` is read **once**,
  through the existing `dev_console::dev_flag` predicate. Emission sites
  test one bool. `train` runs 1.9M fights per session — an env lookup or a
  record built-then-discarded per swing is a real cost there.
- **`dev_flag` is the one answer to "is a dev flag set".** Do not write a
  second one; its doc comment says why.
- **The telemetry drain must NOT sit behind `App::in_arena()`.** See Task 4.
- **Nothing reachable in a player's build.**
- **Run `cargo fmt` and `cargo clippy --workspace --all-targets` after every
  task.** Fix warnings rather than silencing them.
- **`cargo test --workspace` is the final gate**, not the tests you wrote.

## A note on the tree

Four of the five emission seams live in files another session was editing on
2026-08-09 (`combat_round.rs`, `combat_teardown.rs`, `resources.rs`,
`app/battle.rs`). Check `git status` before starting; if that work is still
in flight, do this in a worktree and merge after rather than interleaving.

---

## Task 1: The record types

Pure data with no `World` and no IO, so it is testable alone and the later
tasks can trust the shape.

**Files:**
- Create: `crates/engine/src/telemetry.rs`
- Modify: `crates/engine/src/lib.rs` (add `pub mod telemetry;`)

**Interfaces — Produces:**
```rust
/// Tagged so a reader can dispatch on one field and a line is
/// interpretable alone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "t", rename_all = "snake_case")]
pub enum Record {
    FightStart { fight: u64, seed: u64, zone: u32, depth: u32,
                 party: Vec<PartyMember>, enemies: Vec<EnemyGroupInfo> },
    Round      { fight: u64, round: u32, party_hp: Vec<i32>,
                 enemies: Vec<EnemyGroupHp> },
    EnemyChoice{ fight: u64, round: u32, group: usize, actor: String,
                 move_name: String, target_slot: usize, target: String,
                 target_hp_before: i32, target_max_hp: i32,
                 target_bracing: bool },
    PartyAction{ fight: u64, round: u32, slot: usize, actor: String,
                 kind: ActionKind, name: Option<String>,
                 target_slot: Option<usize> },
    FightEnd   { fight: u64, rounds: u32, won: bool,
                 player_hp_frac: f32, companions_downed: u32 },
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ActionKind { Attack, Special, Defend, Item, Flee }
```
Plus `PartyMember`, `EnemyGroupInfo`, `EnemyGroupHp` — small `Serialize`
structs; field names are in the spec's worked example and must match it.

`move_name` rather than `move`: `move` is a Rust keyword. Give it
`#[serde(rename = "move")]` so the JSON matches the spec.

**Tests** (in `telemetry.rs`'s own `mod tests`):

| Test | Asserts |
|---|---|
| `every_record_kind_round_trips` | One of each variant serializes and parses back equal, via `ron`. `Deserialize` exists *for this test* and for a future analysis script — say so in a comment, or someone will delete it as unused. |

Only one test here, and the reason is a constraint rather than thin
coverage: the properties actually worth pinning are **JSON** properties —
that the tag field is `t`, and that a record serializes to a single line
with no newline in it — and `serde_json` is banned from this crate. Those
assertions live in Task 4's app-core tests, against the real written file,
where `serde_json` legitimately is. Do **not** add `serde_json` to the
engine as a dev-dependency to make a test convenient here; the round trip
through `ron` proves the derives are wired, which is all this crate can
honestly claim.

**Steps:**

- [ ] Write `every_record_kind_round_trips` using `ron`. Run; expect failure.
- [ ] Implement the record types.
- [ ] `cargo test -p feral-processes-engine telemetry`.
- [ ] `cargo fmt && cargo clippy --workspace --all-targets`.
- [ ] Commit: `feat(telemetry): battle record types`

---

## Task 2: The buffer, the gate, and the drain

Wires an inert, empty collector into `Game`. Nothing emits yet, which is
deliberate: the suite must stay green here so any churn in Task 3 is
provably caused by emission and not by wiring.

**Files:**
- Modify: `crates/engine/src/resources.rs` (the `BattleTelemetry` resource)
- Modify: `crates/engine/src/game/lifecycle.rs` (insert it in **both**
  `Game::new` and `Game::load`)
- Create/modify: wherever `Game`'s telemetry helpers live — put them in
  `crates/engine/src/game/telemetry.rs` and declare it in `game/mod.rs`

**Interfaces — Consumes:** `Record` from Task 1.
**Interfaces — Produces:**
```rust
// resources.rs — not saved; dev output, not run state.
#[derive(Resource, Default)]
pub struct BattleTelemetry { pub on: bool, records: Vec<Record> }

impl Game {
    pub fn enable_battle_telemetry(&mut self);
    pub fn take_battle_telemetry(&mut self) -> Vec<Record>;
    pub(crate) fn next_fight_id(&mut self) -> u64;
    pub(crate) fn record(&mut self, f: impl FnOnce(&Game) -> Record);
}
```

**The one part worth spelling out**, because the obvious spelling does not
compile. `record` takes a closure so a disabled game never builds the
record — and that closure must be handed `&Game`, not capture it:

```rust
pub(crate) fn record(&mut self, f: impl FnOnce(&Game) -> Record) {
    if !self.world.resource::<BattleTelemetry>().on {
        return;
    }
    let record = f(self);
    self.world
        .resource_mut::<BattleTelemetry>()
        .records
        .push(record);
}
```

Two things are load-bearing here. **Laziness:** an eager
`record(Record::EnemyChoice { .. })` builds the struct — three `String`
allocations — on every swing of every fight even when disabled, which the
trainer pays 1.9M times over. **The `&Game` parameter:** a
`FnOnce() -> Record` closure would have to capture `&self` to read the
target's `Stats`, while `record` holds `&mut self` — that does not borrow
check. Passing `self` in, and reading `on` before taking the mutable
borrow, is what makes the lazy form legal. There is no eager variant, so a
caller cannot get this wrong.

`next_fight_id` counts up within the process so many fights in one session
separate cleanly. Store the counter on the resource.

**Tests** (`crates/engine/src/tests/telemetry.rs`, declared in `tests/mod.rs`):

| Test | Asserts |
|---|---|
| `telemetry_is_off_by_default` | A `Game::new` against the real assets has `on == false` and drains empty. This is the trainer's guard: the cost of the feature when unused is zero records. |
| `a_disabled_game_does_not_build_records` | Call `record` with a closure that panics; nothing panics. Proves the closure is not invoked when off — an eager version would pass a "drains empty" test while still paying the cost. |
| `taking_the_records_empties_the_buffer` | Two drains, second is empty. `take_*` is a drain, matching `take_pending_profile_writes`. |
| `fight_ids_increase` | Successive `next_fight_id` calls differ. |

**Steps:**

- [ ] Write the four tests. Run; expect failure.
- [ ] Implement the resource and the four methods.
- [ ] Insert the resource in `Game::new` **and** `Game::load` — both doors,
      per `load_asset_dbs`'s own doc comment about why both must be covered.
- [ ] `cargo test --workspace` — expect **fully green**. Nothing emits yet.
- [ ] `cargo fmt && cargo clippy --workspace --all-targets`.
- [ ] Commit: `feat(telemetry): an off-by-default battle record buffer`

---

## Task 3: Emission at the five seams

**Files:**
- Modify: `crates/engine/src/game/combat.rs` (`begin_battle`)
- Modify: `crates/engine/src/game/combat_round.rs`
  (`battle_resolve_round`, `resolve_one_action`)
- Modify: `crates/engine/src/game/combat_policy.rs` (`choose_wild_action`)
- Modify: `crates/engine/src/game/combat_teardown.rs` (`end_battle`)
- Modify: `crates/engine/src/tests/telemetry.rs`

**Interfaces — Consumes:** everything from Tasks 1 and 2.

**Where each record is taken, and the one ordering that matters:**

- `begin_battle` → `FightStart`. Mint the fight id here; every later record
  in the fight carries it. Store the live id on the resource.
- `battle_resolve_round` → `Round`, at the **top** of the round, before any
  action resolves. A snapshot taken at the end is a snapshot of the
  aftermath.
- `choose_wild_action` → `EnemyChoice`, **after** the pair is chosen and
  **before** the caller applies damage. This is the only point at which
  `target_hp_before` exists; taken later it is the HP after the hit, which
  silently inverts the meaning of the whole dataset. Note this function has
  an early `None` return for "nothing reaches" — that path emits nothing,
  which is correct: no swing happened.
- `resolve_one_action` → `PartyAction`, in each match arm, so `kind` comes
  from the `BattleAction` variant rather than being re-derived.
- `end_battle` → `FightEnd`.

**Tests:**

| Test | Asserts |
|---|---|
| `an_enemy_choice_records_the_targets_hp_before_the_hit` | Enable telemetry, run one wild retaliation against a target of known HP, assert the record's `target_hp_before` is the pre-hit value and that the target's HP is now lower. The number the feature exists for, and the one an ordering slip silently corrupts. |
| `every_enemy_swing_produces_one_record` | Over a multi-round fight, `EnemyChoice` records equal the number of enemy attack log lines. A seam that quietly misses swings yields a biased dataset, which is worse than none. |
| `a_party_special_records_its_ability_and_target` | Drive a companion Special via `companion_uses_special` (`tests/support.rs`); assert one `PartyAction` with `kind: Special` and the ability id. This is the routines half — the reason the feature exists. |
| `a_fight_emits_a_start_and_an_end_sharing_one_id` | And that a second fight gets a different id. |
| `a_back_group_that_cannot_reach_emits_no_choice` | The `None` path records nothing. Guards against an "every call emits" refactor inventing swings that never happened. |

**Steps:**

- [ ] Write the five tests. Run; expect failure.
- [ ] Add emission at the five seams.
- [ ] `cargo test --workspace` — expect **fully green**. Telemetry is off by
      default, so no existing test may move. If one does, the wiring is
      leaking rather than the feature working.
- [ ] `cargo fmt && cargo clippy --workspace --all-targets`.
- [ ] Commit: `feat(telemetry): record battle decisions at the five seams`

---

## Task 4: The writer, the arena carve-out, and the docs

**Files:**
- Modify: `crates/app-core/Cargo.toml` (`serde_json`)
- Modify: `crates/app-core/src/lib.rs` (`telemetry_enabled`, `telemetry_path`)
- Modify: `crates/app-core/src/app/lifecycle.rs` (`App::new`, the drain)
- Modify: `crates/launcher/src/main.rs` (pass `dev-logs/battles.jsonl`)
- Modify: `crates/app-core/src/tests/` (a `telemetry` module)
- Create: `dev-logs/README.md`
- Modify: `.gitignore`, `dev-arenas/README.md`, `CLAUDE.md` (then
  `cp CLAUDE.md AGENTS.md` — gitignored twins with no tracking to catch drift)

**Interfaces — Consumes:** `Game::take_battle_telemetry`,
`Game::enable_battle_telemetry`.
**Interfaces — Produces:**
```rust
impl App {
    fn flush_battle_telemetry(&mut self);
}
```

**The gate.** Read once in `App::new` into a `telemetry_enabled: bool`
field, via `crate::app::dev_console::dev_flag("FERAL_DEV_LOG")` — the one
predicate, whose doc comment already explains why a second answer is the
drift this repo keeps catching. Mirror `arena_enabled` exactly, including
its rationale: a field lets the parallel test suite open the gate without
touching a process-global environment. Call
`Game::enable_battle_telemetry` wherever a `Game` is installed, if the flag
is set.

**The carve-out, and the whole point of this task.** `App::after_tick`
early-returns on `in_arena()`, which is what makes an arena session inert on
disk — `an_arena_fight_writes_no_save`, `..._no_profile`,
`an_arena_loss_writes_no_run_history`. **The telemetry flush must not be
inside that guard.** That rule exists so a tester's fight cannot corrupt a
save or pay a real profile reward; a dev-only file under `dev-logs/` does
neither, and the arena is where this feature is most wanted. Put the flush
where it runs in both, carry a comment saying so, and let
`an_arena_fight_still_writes_telemetry` hold it — an omission is invisible
otherwise and the regression is someone folding it back in for tidiness.

Flush after each battle round and at battle end; appending is cheap and a
crash mid-session should not lose the fight that caused it.

**Error handling.** A failed write sets `status_line` once and the run
continues, the shape `flush_profile_writes` uses. Create `dev-logs/` on
first write.

**Tests:**

| Test | Asserts |
|---|---|
| `no_telemetry_file_is_written_when_disabled` | Asserts on the **file**. An omission is invisible otherwise, and this is what a player's build does. |
| `an_arena_fight_still_writes_telemetry` | The carve-out. Sits beside the three tests asserting the opposite about saves, profile and run history, and its comment should point at them. |
| `each_record_is_one_json_line` | Read the written file: line count equals record count, and each line parses with `serde_json` and carries a `t`. The wire-format assertions Task 1 could not make in the engine. |
| `a_failed_telemetry_write_does_not_end_the_run` | Point the path somewhere unwritable; the run continues and the status line reports it. |

**Steps:**

- [ ] Add `serde_json` to `crates/app-core/Cargo.toml` only. Confirm with
      `cargo tree -p feral-processes-engine | grep serde_json` returning
      nothing.
- [ ] Write the four tests. Run; expect failure.
- [ ] Implement the field, the flush and the launcher wiring.
- [ ] Write `dev-logs/README.md`: the flag, the file, and a row per field of
      every record kind — this is the reference for anyone writing an
      analysis script.
- [ ] Add `dev-logs/` to `.gitignore`; add a line to `dev-arenas/README.md`
      pointing at the flag, since that is what someone about to hand-play is
      reading.
- [ ] Add the `CLAUDE.md` load-bearing-seam entry for the arena carve-out —
      it is a stated exception to an invariant already recorded there — then
      `cp CLAUDE.md AGENTS.md`.
- [ ] `cargo test --workspace`; `cargo fmt && cargo clippy --workspace --all-targets`.
- [ ] Commit: `feat(telemetry): write battle records to dev-logs`

---

## Verify it actually answers the question

The feature is not done because the suite is green. Run the fight it was
built for:

```sh
FERAL_DEV_LOG=1 FERAL_DEV_ARENA=1 cargo run --release
# [R] Arena → [L] policy-full-kit → [F], and play it with routines
wc -l dev-logs/battles.jsonl
```

Then check the records answer the open question: how often was the Sprite
the `EnemyChoice` target, what was its `target_hp_before` each time, and did
a `PartyAction` with `redundancy_sync` change that. If the file cannot
answer it, the schema is wrong and that is worth finding now rather than
after ten sessions of collection.

## Not done by this plan

- The analysis script. Deliberate: the shape of the analysis is not knowable
  until there is data to look at.
- Surface and economy events, diagnostics or tracing, log rotation.
- Correcting the training report if the telemetry shows the policy's
  advantage is inflated — that is a finding, and it gets its own change.
- The version bump, changelog section and tag, which happen at the merge to
  `main`, not on the branch.
