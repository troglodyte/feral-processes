# Staff Tantrums Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A base program whose mood has bottomed out rounds on a colleague in
reach and brawls with it for four to eight ticks; nobody dies, the loser is
swept into a Repair Bay by machinery that already exists, and both come away
with memories.

**Architecture:** A fourth rung on the existing `Grievance` morale ladder,
plus a transient `Brawls` resource advanced inside `schedule_base_labour`
between `update_disgruntled` and `admit_the_badly_hurt`. Two supporting
changes make the trigger reachable at all: `Game::fray`'s quiet branch gains a
blame-free memory so unmet needs actually move morale, and a
`base_is_established()` grace gate keeps a young base out of the whole
feature.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine is standalone `bevy_ecs`; gui is
full Bevy + `bevy_egui`). RON assets. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-09-08-staff-tantrums-design.md` — read
it first. This plan argues from it and does not repeat its reasoning.

## Global Constraints

- **Repo conventions are in `CLAUDE.md` and are not optional.** Read it. In
  particular: no hardcoded content in Rust that could be data; new tuning
  values go in `crates/engine/src/tuning.rs` with a doc comment stating the
  number's provenance; comments explain *why*, never *what*.
- **This plan deliberately contains no finished implementation code**, per
  `CLAUDE.md`'s process-weight rule. You get the file list, exact signatures,
  the intent of each test, and the gates. Write the code yourself.
- **TDD, every task.** Failing test first, watch it fail for the right
  reason, minimal implementation, watch it pass, commit.
- **A test that passes with the fix removed is not a test.** For every
  behavioural assertion, delete the implementation line it targets and
  confirm the test goes red before you commit. This repo has shipped vacuous
  tests before.
- **No new `SAVE_FORMAT_VERSION` bump.** Everything added here is either not
  saved at all or an appended enum variant in field-named RON.
- **Exhaustive matches stay exhaustive.** `EffectKind`, `MessageKind` and
  `Grievance` all have match sites that must not gain a `_ =>` arm — that is
  `render/stack.rs`'s `cell_mark` rule and it applies here.
- **Gate for every task:** `cargo test -p feral-processes-engine` plus the
  named tests. **Gate before the final commit:** `cargo test --workspace`,
  `cargo clippy --workspace`, `cargo fmt`.
- **Do not push.** Commit freely on the current branch (`claude/staff-tantrums`);
  landing is the user's call.

## File Structure

**Engine — new behaviour**

- `crates/engine/src/game/base/tantrum.rs` *(new)* — the whole feature: the
  brawl record's advance/close/open steps, victim selection, the non-lethal
  clamp. One file because these three steps share the `Brawls` resource and
  nothing else calls them.
- `crates/engine/src/game/base/mod.rs` — declare the new module.

**Engine — touched**

- `crates/engine/src/components.rs` — `Grievance::LashingOut` (appended).
- `crates/engine/src/resources.rs` — `Brawl`, `Brawls`, `EffectKind::Brawl`,
  `MessageKind::Tantrum`.
- `crates/engine/src/game/base/morale.rs` — `reached()` gains the top arm.
- `crates/engine/src/game/base/offshift.rs` — `fray`'s quiet branch writes
  `ran_down`; both branches gate on the grace predicate.
- `crates/engine/src/game/base/work_orders.rs` — the call site inside
  `schedule_base_labour`, and `base_is_established()`.
- `crates/engine/src/tuning.rs` — a `// Staff tantrums` subsection beside
  `// Acting out`.
- `crates/engine/src/lib.rs` or wherever resources are inserted — register
  `Brawls`.

**Assets**

- `assets/memories/ran_down.ron`, `vented.ron`, `turned_on_me.ron` *(new)*.

**gui**

- `crates/gui/src/fx.rs` — three exhaustive `EffectKind` matches, one
  `MessageKind` colour path, `observe_log`.
- `crates/gui/src/lib.rs` — play `SoundEvent::Hit` for drained `Brawl`
  effects.
- `crates/gui/src/render/mod.rs`, `crates/gui/src/render/hud/log_frame.rs` —
  `MessageKind::Tantrum` arms.

**Tests**

- `crates/engine/src/tests/tantrums.rs` *(new)* — declare it in the tests
  module.
- `crates/engine/src/tests/disposition.rs` — the ladder tests extend here,
  beside the two that already guard `MORALE_DOWNS_TOOLS_AT`.
- `crates/engine/src/tests/assets.rs` — three `MEMORY_TRIGGERS` rows.

---

### Task 1: Needs actually move morale

The spec's decisions 1 and 2. Independent of everything else in this plan and
worth landing on its own: it closes a gap that exists today regardless of
tantrums.

**Files:**
- Create: `assets/memories/ran_down.ron`
- Modify: `crates/engine/src/game/base/offshift.rs` (`Game::fray`, ~line 402)
- Modify: `crates/engine/src/tests/assets.rs` (`MEMORY_TRIGGERS`, ~line 2354)
- Test: `crates/engine/src/tests/tantrums.rs` (create; declare in the tests
  module)

**Interfaces:**
- Consumes: `Game::remember(&mut self, who: Entity, def_id: &str, subject: MemorySubject) -> Remembered`; `Game::fray(&mut self, worker: Entity, need: &NeedId, unreachable: bool)`.
- Produces: memory id `"ran_down"`, `MemorySubjectKind::Nothing`.

- [ ] **Step 1: Write two failing tests, one per branch of `fray`.**

`fray_with_no_amenity_writes_ran_down` and
`fray_with_an_unreachable_amenity_still_writes_frayed_here`. Drive `fray`
directly with `unreachable: false` and `true`, then assert on the worker's
`Memories` store.

**Both tests are required and neither substitutes for the other.** A single
test over one branch passes against an implementation that writes the same
memory on both, which is exactly the bug worth guarding.

Note `fray` early-returns unless `store.latch(need)` returns true, so a
second call for the same need on the same worker is a no-op. One call per
worker per test.

- [ ] **Step 2: Run them and confirm they fail.**

`cargo test -p feral-processes-engine tantrums`

Expected: `fray_with_no_amenity_writes_ran_down` fails (no such memory);
the `frayed_here` one should already pass, which is fine — it is a
regression guard, not a driver.

- [ ] **Step 3: Author `assets/memories/ran_down.ron`.**

Fields exactly as the spec's Schema section gives them. Copy the house style
from `frayed_here.ron`: a leading `//` comment saying what the memory is and,
where there is a near neighbour, how it differs from it.

- [ ] **Step 4: Write the `ran_down` branch.**

`fray` currently writes `frayed_here` only under `if let (true, Some(at)) =
(unreachable, at)`. Add the `else` half. The doc comment above `fray` states
the withheld-blame rule and is now **wrong** — update it to say what is
actually true: the blame is withheld, the memory is not, which is what
`Nothing` as a subject buys.

- [ ] **Step 5: Add the `MEMORY_TRIGGERS` row.**

`("ran_down", K::Nothing)`, beside the existing `frayed_here` row, with a
comment naming `Game::fray` and which branch — matching the style of every
other row in that census.

- [ ] **Step 6: Run the tests plus the asset census.**

`cargo test -p feral-processes-engine tantrums`
`cargo test -p feral-processes-engine every_shipped_memory_def_is_reachable_from_a_trigger`

Expected: PASS.

- [ ] **Step 7: Verify the tests are not vacuous.**

Delete the `ran_down` write, re-run, confirm red. Restore.

- [ ] **Step 8: Commit.**

`feat(base): a need nothing answers is remembered`

---

### Task 2: The grace period

The spec's decision 3. Gates Task 1's memories, and later the tantrum itself.

**Files:**
- Modify: `crates/engine/src/tuning.rs`
- Modify: `crates/engine/src/game/base/work_orders.rs` (`base_is_established`)
- Modify: `crates/engine/src/game/base/offshift.rs` (`fray` gates on it)
- Test: `crates/engine/src/tests/tantrums.rs`

**Interfaces:**
- Consumes: `Game::base_staff(&self) -> Vec<Entity>`; `work_orders::structures_by_tile(game: &Game) -> HashMap<(i32,i32), Entity>` (~line 266) — or a direct `Structure` query, whichever reads cleaner.
- Produces: `pub(crate) fn base_is_established(&self) -> bool`.

- [ ] **Step 1: Write three failing tests.**

`a_young_base_earns_no_need_grudges` — below both thresholds, `fray` on
either branch writes nothing.

`a_base_short_of_staff_earns_no_need_grudges` — `BASE_ESTABLISHED_STRUCTURES
+ 2` structures but `BASE_ESTABLISHED_STAFF - 1` staff.

`a_base_short_of_structures_earns_no_need_grudges` — the mirror.

**The last two are the point of this task.** A predicate wired `||` instead of
`&&` passes a test that only ever starves both halves at once, and `&&` is
what the spec calls for.

Plus `an_established_base_earns_need_grudges` at or above both, so the gate is
not simply always-off — a gate nothing can pass is a deleted feature.

- [ ] **Step 2: Run and confirm they fail.**

`cargo test -p feral-processes-engine tantrums`

- [ ] **Step 3: Add the two constants.**

`BASE_ESTABLISHED_STAFF: usize = 8` and `BASE_ESTABLISHED_STRUCTURES: usize =
8`, in a new `// Staff tantrums` light subsection beside `// Acting out`
(~line 4483). Follow that section's own comment style: every constant in this
file states its number's provenance. These are unmeasured and chosen as a
"the player has had a fair chance" line; say so, and say that a tick-based
grace was considered and rejected (spec decision 3).

- [ ] **Step 4: Write `base_is_established`.**

At-least on both counts, `&&`. Name it for what it means about the base, not
for tantrums — it gates the need-memories too.

- [ ] **Step 5: Gate both `fray` branches on it.**

The *log lines stay unconditional*. Only the memory writes are gated: the
player must still be told the need is unmet at a young base, because that
line is the errand.

- [ ] **Step 6: Run tests, verify non-vacuity, commit.**

Flip `&&` to `||` and confirm the two single-half tests go red. Restore.

`feat(base): needs count against a base only once it is established`

---

### Task 3: The fourth rung

The spec's decisions 4 and 5. No behaviour yet beyond the marker — the rung
is inert until Task 5.

**Files:**
- Modify: `crates/engine/src/components.rs` (`Grievance`, ~line 1643)
- Modify: `crates/engine/src/game/base/morale.rs` (`reached`, ~line 148)
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/disposition.rs`

**Interfaces:**
- Produces: `Grievance::LashingOut`; `MORALE_LASHES_OUT_AT: f32`.

- [ ] **Step 1: Write the failing tests.**

`the_ladder_climbs_in_order` already exists — extend it to the third rung.

`no_single_memory_can_make_a_program_lash_out` and
`two_bad_memories_can_still_make_a_program_lash_out` — model them directly on
the two neighbouring tests that guard `MORALE_DOWNS_TOOLS_AT`
(`disposition.rs` ~lines 329 and 355), including their use of
`worst_single_grudge(&game)` against the real `assets/memories/`. The felt
value of a memory is `valence * strike_cap * DISPOSITION_MEMORY_SWING`, not
its valence — that arithmetic is what the existing helper exists to get
right, so reuse it rather than restating it.

`reached_returns_lashing_out_past_its_line`.

- [ ] **Step 2: Run and confirm failure.**

`cargo test -p feral-processes-engine disposition`

- [ ] **Step 3: Append the variant.**

`LashingOut` after `DownedTools`. **Append, never insert** — `Ord` derives
from declaration order and is what `update_disgruntled`'s ratchet compares,
and `SaveData::disgruntled` encodes the variant name. Inserting reorders the
ladder silently.

- [ ] **Step 4: Add `MORALE_LASHES_OUT_AT`.**

`-75.0`, in the `// Staff tantrums` subsection from Task 2. The doc comment
must carry the argument from spec decision 5 — past what one memory can
reach, inside what two can — and name the two tests that hold it, matching
how `MORALE_DOWNS_TOOLS_AT`'s own comment is written.

- [ ] **Step 5: Add the arm to `reached`.**

Above the `DownedTools` arm, since the ladder is checked worst-first. The
exit side is untouched: still the single `MORALE_RECOVERED_AT` comparison in
`update_disgruntled`, so the ladder keeps one hysteresis gap.

- [ ] **Step 6: Fix every exhaustive match the new variant breaks.**

Compile and follow the errors. Do **not** silence any of them with a `_ =>`
arm.

- [ ] **Step 7: Run, verify non-vacuity, commit.**

`cargo test -p feral-processes-engine disposition`

`feat(base): a fourth rung on the morale ladder`

---

### Task 4: The two tantrum memories

Split from Task 5 so the assets and the census land green on their own; Task
5 wires the writes.

**Files:**
- Create: `assets/memories/vented.ron`, `assets/memories/turned_on_me.ron`
- Modify: `crates/engine/src/tests/assets.rs` (`MEMORY_TRIGGERS`)

- [ ] **Step 1: Author both files** exactly as the spec's Schema section
  gives them, with house-style leading comments. `turned_on_me` is the first
  *negative* `Program`-subject memory the game ships — say so in its comment,
  since the two existing `Program` memories are both positive and a reader
  will assume the subject implies fondness.

- [ ] **Step 2: Add both `MEMORY_TRIGGERS` rows**, commented with
  `Game::close_brawl` as the writer (the function Task 5 creates).

- [ ] **Step 3: Run the census.**

`cargo test -p feral-processes-engine assets`

Expected: **FAIL** — the census asserts every shipped def has a real Rust
writer, and Task 5 has not written one yet.

- [ ] **Step 4: Do not commit yet.** This task's deliverable is only green
  once Task 5 lands. Carry it into Task 5's commit.

---

### Task 5: The brawl

The spec's decisions 6 through 11, and its Flow section.

**Files:**
- Create: `crates/engine/src/game/base/tantrum.rs`
- Modify: `crates/engine/src/game/base/mod.rs`
- Modify: `crates/engine/src/resources.rs` (`Brawl`, `Brawls`)
- Modify: `crates/engine/src/game/base/work_orders.rs` (call site ~line 872,
  between `update_disgruntled` and `admit_the_badly_hurt`)
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/tantrums.rs`

**Interfaces:**
- Consumes: `Game::apply_damage(&mut self, target: Entity, dmg: i32) -> i32`; `Game::opinion_of(&self, who: Entity, subject: &MemorySubject) -> f32`; `Game::creature_label(&self, e: Entity) -> String`; `Game::log_base_kind(&mut self, kind: MessageKind, s: impl Into<String>)`; `Game::push_effect(&mut self, entity: Entity, kind: EffectKind)`; `Game::base_is_established()` (Task 2); `Grievance::LashingOut` (Task 3).
- Produces: `Brawls`, `Brawl`, `Game::run_tantrums(&mut self, staff: &[Entity])`;
  `Game::close_brawl(&mut self, brawl: &Brawl)` — writes the result lines and
  both memories, and is the writer named in Task 4's census rows.

- [ ] **Step 1: Write the failing tests.** Named in the spec's Testing
  section; the intent of each, in dependency order:

  - `a_tantrum_never_kills` — a full brawl against a victim left on 1 HP,
    assert still alive afterwards. **This is the most important test in the
    plan.** `apply_damage` floors HP at 0 and reaching 0 *is* a kill; nothing
    inside it refuses a lethal blow.
  - `a_short_brawl_still_fills_the_bay` — two full-health staff, exactly
    `TANTRUM_TICKS_MIN` ticks, assert at least one carries `Downed`. Min and
    not max: the long case passes for free, and a test written against it
    goes green with a damage constant far too low to hold the guarantee.
  - `a_brawl_writes_both_memories` — aggressor has `vented`, victim has
    `turned_on_me`, and the victim's names the aggressor's `ProgramId`.
  - `a_program_with_nobody_in_reach_starts_no_brawl`.
  - `a_tantrum_draws_no_rng_when_nobody_is_lashing_out` — snapshot `GameRng`
    across a tick and assert it is unmoved. Model it on `run_routes`'
    predation test. Without this the feature can silently shift the seeded
    stream, which reads later as unrelated tests flaking.
  - `a_downed_program_is_neither_aggressor_nor_victim`.
  - `the_cooldown_holds`.
  - `a_brawl_reports_what_each_side_did` — the closing lines carry both
    damage totals, and all tantrum lines are `MessageKind::Tantrum` from
    `log_base_kind` (so `MessageSource::Base`).
  - `a_one_sided_brawl_omits_the_reply_line` — when the victim never landed a
    blow, the "fought back" line is absent rather than printed as `0 damage`.
    That happens whenever the bay takes the victim on the first exchange, so
    it is a reachable state and not a defensive branch.

  Note `message_history` condenses repeated lines, so a test that counts log
  entries cannot tell "said once" from "said eight times" — assert on the
  line text and on `repeats`, not on a count.
  - `no_tantrum_opens_below_the_grace_thresholds` — even for a program
    already on the rung.

- [ ] **Step 2: Run and confirm every one fails.**

- [ ] **Step 3: Add the remaining constants** to the `// Staff tantrums`
  subsection: `TANTRUM_CHANCE_PER_TICK`, `TANTRUM_REACH_TILES`,
  `TANTRUM_TICKS_MIN`, `TANTRUM_TICKS_MAX`, `TANTRUM_DAMAGE_FRACTION`,
  `TANTRUM_COOLDOWN_TICKS`. Values in the spec's Tuning table.
  `TANTRUM_DAMAGE_FRACTION`'s doc comment must record *why* it is a quarter
  rather than a small fraction — the short-end bound — and that mitigation
  makes the landed figure lower, so the number is pinned by
  `a_short_brawl_still_fills_the_bay` and not by arithmetic.

- [ ] **Step 4: Add `Brawl` and `Brawls` to `resources.rs`** and register
  `Brawls` wherever the other base resources are inserted. It is **not** a
  save field — spec decision 9. `Brawl` holds `Entity`, not `ProgramId`,
  precisely because it is not saved.

- [ ] **Step 5: Write `tantrum.rs`.** Three steps in the order the spec's
  Flow section gives, with the grace check first. Two things there are easy
  to get subtly wrong and are worth stating:

  The non-lethal clamp is applied to the **input**, before `apply_damage`,
  following `game/throw.rs:52`:

  ```rust
  let dmg = raw.min(hp - 1).max(0);
  ```

  And opening is deliberately the **last** of the three steps, so a brawl
  opened this tick throws its first blow next tick. That is what keeps the
  alert line ahead of any damage in the log and `ticks_left` an honest count.

- [ ] **Step 6: Wire the call site.** Inside `schedule_base_labour`, after
  `update_disgruntled` and **before** `admit_the_badly_hurt`. That ordering
  is what gets the Repair Bay for free — spec decision 8 — so add a comment
  saying so, in the style of the three gates already commented there.

  Note the existing `if staff.is_empty() { ... return; }` sits between
  `admit_the_badly_hurt` and `drift_idle_staff`; your call goes above it.

- [ ] **Step 7: Run the full engine suite.**

`cargo test -p feral-processes-engine`

The `MEMORY_TRIGGERS` census from Task 4 should now pass too.

- [ ] **Step 8: Verify non-vacuity of the two guarantees.**

Delete the `.min(hp - 1)` clamp → `a_tantrum_never_kills` must go red.
Halve `TANTRUM_DAMAGE_FRACTION` → `a_short_brawl_still_fills_the_bay` must go
red. Restore both.

- [ ] **Step 9: Commit**, including Task 4's assets and census rows.

`feat(base): a program with nothing left starts a fight`

---

### Task 6: The flash, the noise and the log

The spec's decisions 12 and 13.

**Files:**
- Modify: `crates/engine/src/resources.rs` (`EffectKind::Brawl`,
  `MessageKind::Tantrum`)
- Modify: `crates/gui/src/fx.rs` (~lines 350, 431, 438 and `observe_log` ~837)
- Modify: `crates/gui/src/lib.rs` (~line 578, the effect drain)
- Modify: `crates/gui/src/render/mod.rs` (~line 446)
- Modify: `crates/gui/src/render/hud/log_frame.rs` (~line 104)

- [ ] **Step 1: Write the failing gui tests.**

`a_brawl_flashes_the_same_red_as_a_hit` — `effect_color(EffectKind::Brawl) ==
palette::THREAT`, beside the existing assertions at `fx.rs:952`.

`a_tantrum_line_flashes_the_log_pane` — `observe_log` with a
`MessageKind::Tantrum` line starts the flash.

- [ ] **Step 2: Add both enum variants** in `resources.rs`.

- [ ] **Step 3: Fix every exhaustive match.** `EffectKind` has three in
`fx.rs` — `spark_burst` (~350), the flash-duration match (~431), and
`effect_color` (~438). `MessageKind` has the colour match in `render/mod.rs`
(~446) and the channel-label match in `log_frame.rs` (~104). Compile and
follow the errors; no `_ =>` arms.

`Brawl` takes `HIT_SPARKS`/`HIT_SPARK_REACH`, `HIT_FLASH_SECONDS` and
`FLASH_RED` — identical to `Hit` in all three. The variant exists to carry
*sound*, not a different look.

- [ ] **Step 4: Extend `observe_log`** to start the log flash on
`MessageKind::Tantrum` as well as `Raid`, and update its doc comment, which
currently says "a newly logged raid line".

- [ ] **Step 5: Play the sound.**

In `crates/gui/src/lib.rs`, the effects vector is drained at ~line 578 and
consumed by `fe.fx.begin_frame(...)` immediately after — so inspect it
*before* that call.

Two things to get right: `fe.app.take_sounds()` is drained earlier (~563), so
this cannot go through `pending_sounds` and must call `sounds.play(&mut
commands, SoundEvent::Hit, fe.volume)` directly. And **play at most one cue
per frame** however many `Brawl` effects are in the vector — several ticks
can land in one frame, and one cue per blow is a machine-gun.

Play it whether or not `Fx` is enabled: sound is not a visual effect.

- [ ] **Step 6: Run the gui tests.**

`cargo test -p feral-processes-gui`

- [ ] **Step 7: Commit.**

`feat(gui): a brawl flashes red and is heard`

---

### Task 7: Documentation and the full gate

**Files:**
- Modify: `CHANGELOG.md`
- Modify: `assets/memories/README.md` — only if a field's meaning changed. It
  should not have; check rather than assume.
- Modify: `CLAUDE.md` — the seam rules (see below).
- Modify: `docs/seams.md` and `.claude/skills/seams/` — the other two writes.

- [ ] **Step 1: Add the `CHANGELOG.md` section.** Read the file's preamble
  first — it is the one statement of the versioning policy, and "breaking"
  there means a player's save stops loading, which this is not.

  **Do not bump the workspace version.** Per `CLAUDE.md`, commits on a branch
  stay unversioned; the bump happens once, at the merge.

- [ ] **Step 2: Write the three seam records.** This feature adds seams and
  `CLAUDE.md` documents the order: the argument to `docs/seams.md`, the trap
  to the `seams` skill, the one-sentence rule to `CLAUDE.md`. Invoke the
  `seams` skill for the convention rather than guessing at it.

  The rules worth recording, one sentence each:
  - The non-lethal clamp is applied before `apply_damage`, never inside it,
    and it is the whole of "a tantrum never kills."
  - The tantrum step sits between `update_disgruntled` and
    `admit_the_badly_hurt`, and that ordering is what gets the Repair Bay
    with no new code.
  - `Grievance` is appended to, never inserted into — `Ord` is the ladder and
    the variant name is the save.
  - `EffectKind::Brawl` draws identically to `Hit` and exists only to carry
    sound.

- [ ] **Step 3: Do NOT touch `README.md` or `docs/manual.md`.** Both are
  explicitly carved out of the documentation obligation.

- [ ] **Step 4: Run the full gate.**

```sh
cargo fmt
cargo clippy --workspace
cargo test --workspace
```

All three must be clean. Fix warnings rather than silencing them.

- [ ] **Step 5: Run the balance gate.**

`cargo test -p feral-processes-engine balance_sim`

This feature touches no combat constant, so the curves must be **unmoved**. A
moved curve here means something leaked into a shared formula and is a
finding, not a test to update.

- [ ] **Step 6: Commit.**

`docs: the staff tantrum seams`

---

## What this plan does not do

Stated so nobody adds them mid-flight:

- No key, no screen, no notification, no `Game::attention` row (spec's Out of
  scope).
- No save field, and no `SAVE_FORMAT_VERSION` bump.
- No pathing — a program fights only who is already in reach.
- No change to raid audio. `EffectKind::Brawl` exists precisely so raids stay
  as silent as they are today.
- **No playtesting.** Nothing here can be verified on a screen from an agent
  session — there is no display. A green suite is not evidence that this
  feels right, and the constants in the Tuning table are all unmeasured. The
  first time anyone sees a tantrum will be the user playing it.
