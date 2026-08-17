# Power Replaces Fatigue — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps use
> checkbox (`- [ ]`) syntax for tracking.

**Goal:** Delete the Fatigue meter and make Power the budget that routine
calls draw on alongside their cooldowns, with every companion holding its
own reserve.

**Architecture:** One component (`PowerReserve`, private float) replaces
`Needs`; one ability field (`AbilityDef::power_cost`) replaces two; one gate
(`Game::ability_unavailable`) refuses and one spender
(`Game::spend_power`) charges, both scaled by one knob. The 66 ability files
that already carry a cost keep their numbers under the new key.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine, standalone), RON assets, RON
save format.

**Spec:** `docs/superpowers/specs/2026-08-17-power-replaces-fatigue-design.md`
— read it before Task 1. This plan argues from it and does not restate its
reasoning.

## Global Constraints

- **Read `CLAUDE.md` first.** It is the project's standing rules and
  overrides anything here it contradicts.
- **TDD.** Failing reproducer first, every task. A test that passes with its
  fix reverted is a plan failure, not coverage — delete the fix and watch it
  fail before committing.
- **No finished code is prescribed below.** File lists, interfaces, test
  *intent* and gates are given; write the implementation yourself, and say
  so if a task's premise turns out to be wrong.
- **Moddability.** New `.ron`-visible fields are `#[serde(default)]`; a
  malformed file is skipped with a logged warning, never a panic. Update the
  matching `assets/*/README.md` in the same task that changes a schema.
- **`SAVE_FORMAT_VERSION` is bumped exactly once, in Task 2**, and Tasks 3–6
  ride that bump. Nothing is released between them, so a second bump would
  be noise.
- **Do not update `docs/manual.md` or the root `README.md`** — both are
  explicitly carved out of the documentation obligation. `CHANGELOG.md`,
  `assets/*/README.md`, `docs/seams.md` and `CLAUDE.md` still apply.
- **The workspace version bump and `CHANGELOG.md` section happen at the
  merge, not on the branch.** Do not bump `Cargo.toml` in any task here.
- **Never `git push`.** Commit freely; the release is the user's call.
- **Per-task gate:** `cargo test -p feral-processes-engine <filter>` plus
  `cargo fmt` and `cargo clippy --workspace`. **Final gate:**
  `cargo test --workspace`.
- Branch is `power-replaces-fatigue`, already created, spec already
  committed.

---

### Task 1: Close the underground Power-regen hole

Standalone bug fix, valuable on its own and a precondition for the whole
feature: `power_regen_system` reads the player's `Position`, which is pinned
to the surface entrance tile while underground, so a Stack link inside a
Recharger's radius regenerates Power the whole way down. Harmless until
routines cost Power; then it deletes the scarcity the design rests on.

**Files:**
- Modify: `crates/engine/src/systems.rs:940` (`power_regen_system`)
- Test: `crates/engine/src/systems.rs` tests module (the existing
  `power_regen_world` / `run_regen_once` helpers live there)

**Interfaces:**
- Consumes: `resources::Locale` (already a `Resource`), `Game::is_underground`
  logic — but the system is a bevy system, not a `Game`, so it reads
  `Res<Locale>` directly and matches the variant.
- Produces: nothing later tasks depend on.

- [ ] **Step 1: Write two failing tests, not one.** First: a player
  underground with a Recharger inside its radius of their (surface,
  entrance-tile) `Position` gains no Power on a tick. Second: the *same*
  fixture on the surface still gains Power. The second is not optional — a
  bare early return at the top of the system passes the first test alone,
  and this is exactly the vacuous-coverage failure `CLAUDE.md` records from
  2026-08-09.
- [ ] **Step 2: Run them.** Expect the underground one to fail (Power was
  granted) and the surface one to pass.
- [ ] **Step 3: Add the guard.** `Res<Locale>`, skip while underground.
  Preserve the existing `.chain()` ordering against `needs_tick_system` and
  the comment explaining why it is load-bearing.
- [ ] **Step 4: Run both.** Expect PASS.
- [ ] **Step 5: Revert the guard, confirm the underground test fails again,
  restore it.**
- [ ] **Step 6: Gate and commit.** `cargo test -p feral-processes-engine
  power_regen`, `cargo fmt`, `cargo clippy --workspace`.

---

### Task 2: Delete the Fatigue meter

Everything about Fatigue as a *resource*. `AbilityDef::fatigue_cost` is
deliberately left alone until Task 4 — this task removes the meter, not the
cost field, and the "grep for fatigue returns nothing" gate lands in Task 4
rather than here.

After this task the two Stack movement routines spend Power instead of
Fatigue, still reading `AbilityDef::fatigue_cost` for the amount. That is a
real gameplay change and the intended one.

**Files:**
- Modify: `crates/engine/src/components.rs` — `Needs::fatigue` and its
  `Default`; `FieldBuffKind::Coolant`; the `NEED_MAX`/`NEED_MIN` doc comment,
  which names both meters
- Modify: `crates/engine/src/systems.rs:36` — `tick_needs` loses its
  `fatigue` parameter and return element; `needs_tick_system` follows
- Modify: `crates/engine/src/tuning.rs` — delete `FATIGUE_REGEN_PER_TICK`;
  rewrite the "Needs & rest" section comment, which is now wrong in three
  places
- Modify: `crates/engine/src/save.rs` — delete `PlayerSave::fatigue`; **bump
  `SAVE_FORMAT_VERSION`** and follow that constant's own doc comment for how
- Modify: `crates/engine/src/items_db.rs` — `ConsumeDef::fatigue` and its
  `validate` arm (`consume.fatigue`). Dead schema: no shipped item sets it
- Modify: `crates/engine/src/game/turn.rs:519` (`consume_item`), `:668`
  (`rest`)
- Modify: `crates/engine/src/game/field.rs:52` — the `(cost, held, unit)`
  match collapses to one arm; `:403` `spend_fatigue` becomes a Power spend
- Modify: `crates/engine/src/game/combat_status.rs:339` — the `Coolant` arm
- Modify: `crates/engine/src/game/catalog.rs:288` — the `+{:.0} rest` line
- Modify: `crates/engine/src/game/inspection.rs:704`,
  `crates/engine/src/game/party.rs:85`,
  `crates/engine/src/game/lifecycle.rs:373` and `:1004`
- Modify: `crates/engine/src/difficulty.rs:45`, `:115`, `:178`
- Modify: `crates/engine/src/views.rs` — `PlayerStatus::fatigue` (:82),
  `PartySlotView::fatigue` (:751), `PlayerManifest::fatigue` (:1278)
- Modify: `crates/gui/src/render/battle.rs` — `fatigue_cell`, `FATIGUE_W`,
  `party_tail`, `the_fatigue_column_holds_its_place`,
  `a_companions_fatigue_cell_is_a_dash`. **Rename the column to Power and
  point it at `hunger`; do not delete it.** It is the companion Power
  display Task 5 fills in, already laid out and width-pinned
- Delete: `assets/abilities/coolant_flush.ron`
- Modify: `assets/research/field_ops.ron` — drop `coolant_flush` from
  `unlocks_abilities` and from the description string
- Modify: `assets/abilities/README.md`, `assets/items/README.md` — the
  `Coolant` kind and `consume.fatigue` are schema documentation
- Test: `crates/engine/src/tests/turn.rs`,
  `crates/engine/src/tests/combat_status.rs`,
  `crates/engine/src/tests/assets.rs`, `crates/app-core/src/tests/support.rs`

**Interfaces:**
- Consumes: Task 1's guard (untouched here).
- Produces: `Needs { hunger: f32 }` — a one-field struct, renamed in Task 3.
  `FieldBuffKind` with `Coolant` gone and `Trickle` unchanged.

- [ ] **Step 1: Write the failing tests.** (a) A save round trip with no
  `fatigue` field loads. (b) `Game::rest` restores Power to full — it
  currently only fills Fatigue, so this is a behaviour change worth pinning.
  (c) A `Trickle` field buff still restores Power per tick, proving the
  surviving kind was not damaged by removing its neighbour.
- [ ] **Step 2: Run them.** Expect compile failure or FAIL.
- [ ] **Step 3: Do the removal**, file list above. Let the compiler drive
  it — `Needs::fatigue` is a public field, so every reader surfaces.
- [ ] **Step 4: Handle the two asset edits and the two schema READMEs.**
  `coolant_flush` and `trickle_charge` are the same ability once Fatigue is
  gone; `coolant_flush` is the one that goes. Check `field_ops.ron`'s
  description prose, not just its `unlocks_abilities` list.
- [ ] **Step 5: Retune `trickle_charge`.** It is now the only in-Stack Power
  generator and nets +60 for 20 spent. Pick a number deliberately and record
  the reasoning in the `.ron` or the changelog; do not leave it at its
  Fatigue-era value by default. See the spec's note that this is the
  single highest-leverage number in the feature.
- [ ] **Step 6: Run the engine, app-core and gui suites.** Expect PASS.
- [ ] **Step 7: Gate and commit.** `cargo test --workspace` is warranted
  here despite the per-task rule — this task touches three crates and the
  save format.

---

### Task 3: `PowerReserve` holds its own clamp

Pure rename and encapsulation. No behaviour change; if a test's *expected
value* moves in this task, something is wrong.

`components.rs:122` documents "anything writing `hunger` or `fatigue` has to
clamp to it" as an invariant held by convention across ~10 sites. A private
field converts it into a barrier, the way `Game`'s private `world` field
holds the renderer rule.

**Files:**
- Modify: `crates/engine/src/components.rs` — `Needs` → `PowerReserve`;
  `NEED_MIN`/`NEED_MAX` → `POWER_MIN`/`POWER_MAX`, **staying in
  `components.rs`** rather than moving to `tuning.rs`, for the reason their
  doc comment already gives
- Modify: every `Needs` reader from Task 2's list
- Modify: `crates/engine/src/views.rs` — `hunger` → `power` on
  `PlayerStatus`, `PartySlotView`, `PlayerManifest`. These are read-only DTOs
  crossing to gui and stay plain `pub power: f32`
- Modify: `crates/engine/src/save.rs` — `PlayerSave::hunger` → `power`,
  riding Task 2's bump
- Modify: `crates/gui/src/render/bars.rs`, `manifest.rs`, `party.rs`,
  `field.rs`, `battle.rs` — field rename only
- Modify: `crates/app-core/src/` — wherever the renamed view fields are read
- Test: `crates/engine/src/components.rs` tests module (new, for the API)

**Interfaces:**
- Produces, and later tasks depend on exactly this surface:

```rust
pub struct PowerReserve(f32);          // field PRIVATE

impl PowerReserve {
    pub fn new(value: f32) -> Self;    // clamps
    pub fn get(&self) -> f32;
    pub fn holds(&self, cost: f32) -> bool;
    pub fn spend(&mut self, cost: f32);            // clamps at POWER_MIN
    pub fn restore(&mut self, amount: f32);        // clamps at POWER_MAX
    pub fn fill(&mut self);
    pub fn raise_to_at_least(&mut self, floor: f32);
}
```

`fill` exists for `Game::rest`, which sets outright rather than adding.
`raise_to_at_least` exists for `difficulty.rs`'s Forgiving reboot, the one
site that raises to a floor. Add nothing else — if a caller needs a ninth
operation, that is a signal to re-read the call site, not to widen the type.

- [ ] **Step 1: Write the failing tests** on `PowerReserve` directly:
  `spend` past zero floors at `POWER_MIN` and does not go negative;
  `restore` past full caps at `POWER_MAX`; `new` clamps a wild input both
  ways; `holds` is false at exactly one unit short and true at exactly the
  cost; `raise_to_at_least` never *lowers* a reserve already above the floor.
  That last one is the bug the Forgiving reboot would otherwise ship.
- [ ] **Step 2: Run them.** Expect compile failure.
- [ ] **Step 3: Add the type**, then let the compiler drive the rename. The
  private field will reject every existing write site; convert each to the
  matching method rather than adding an accessor.
- [ ] **Step 4: Run the workspace suite.** Expect PASS with no expected-value
  changes.
- [ ] **Step 5: Gate and commit.**

---

### Task 4: One cost field, one knob, and the asset flip

**Files:**
- Modify: `crates/engine/src/abilities.rs` — delete `AbilityDef::fatigue_cost`
  (:355) and `default_fatigue_cost` (:425); delete `power_cost` from the
  `AbilityEffect::FieldBuff` variant; add `AbilityDef::power_cost: f32` with
  `#[serde(default)]` defaulting to **0.0**; move the `validate` arm (:443)
  to the new field; delete
  `a_field_buff_leaving_fatigue_cost_at_its_default_is_silent` (:908) and the
  dead-fields exemption it guards; correct the stale doc comments at :258,
  :404, :553, :879
- Modify: `crates/engine/src/tuning.rs` — delete
  `DEFAULT_ROUTINE_FATIGUE_COST` (:445); add `ROUTINE_POWER_COST_MULTIPLIER:
  f32 = 1.0`; correct the stale claim at :1483
- Modify: `crates/engine/src/game/combat.rs:850` — the doc comment states
  the field "is read only by the two Stack field routines", which stops being
  true here
- Modify: `crates/engine/src/game/field.rs` — `field_routines` reads
  `def.power_cost` for every row with one unit label, `"PWR"`; keep the
  existing ordering comment about stating the permanent objection ahead of
  the temporary one
- Modify: 55 files under `assets/abilities/` — key rename `fatigue_cost:` →
  `power_cost:`, **values unchanged**
- Modify: 11 files under `assets/abilities/` — hoist `power_cost` out of the
  `FieldBuff(…)` effect to the top level, **values unchanged**
- Modify: `assets/abilities/README.md`
- Test: `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Consumes: `PowerReserve` from Task 3.
- Produces: `AbilityDef::power_cost: f32`, and a single helper for the
  scaled price that both the gate and the spender call:

```rust
// Wherever it lands, this must be the ONE expression for a routine's price.
// Two call sites reading `def.power_cost * MULTIPLIER` independently is the
// drift `CLAUDE.md` warns about; the refusal and the charge must agree by
// construction.
pub(crate) fn routine_power_cost(def: &AbilityDef) -> f32;
```

The five files with no cost today — `deadlock`, `hot_patch`, `memory_leak`,
`priority_boost`, `sandbox` — are **not** edited. They inherit 0.0 and keep
today's behaviour. `priority_boost` staying free matters: it is the fallback
every companion has when its species grants nothing.

- [ ] **Step 1: Write the failing tests.** (a) An assets census over the real
  files: every `power_cost` is finite and non-negative — this replaces the
  `validate` coverage that would otherwise be lost with the rename. (b) A
  spot-check that three named abilities kept their exact pre-flip numbers,
  chosen from different value bands (there are 0.0, 4.0 and 20.0 cases). (c)
  `routine_power_cost` scales with the multiplier.
- [ ] **Step 2: Run them.** Expect compile failure.
- [ ] **Step 3: Change the Rust side**, including all six stale doc comments
  listed above. They are part of the deliverable, not tidying: two of them
  state as fact the thing this task falsifies.
- [ ] **Step 4: Flip the assets.** Mechanical, but verify the counts land at
  55 renamed and 11 hoisted with **zero** value changes — diff the numbers
  out of the before and after trees and compare the multisets, don't eyeball
  it.
- [ ] **Step 5: Run the tests.** Expect PASS.
- [ ] **Step 6: Grep gate.** `rg -i fatigue` across `crates/` and `assets/`
  must return nothing. This is the task where that becomes true.
- [ ] **Step 7: Gate and commit.**

---

### Task 5: Companion reserves, and the four doors

Give companions reserves *before* the gate is switched on in Task 6. Doing
it the other way round leaves an intermediate state where every companion
Special is refused.

**Files:**
- Modify: `crates/engine/src/game/spawning.rs:294` (`adopt_program`)
- Modify: `crates/engine/src/game/combat_rewards.rs:811` (a successful
  capture)
- Modify: `crates/engine/src/game/lifecycle.rs:1272`
  (`grant_starting_program`)
- Modify: `crates/engine/src/game/party.rs:831` (`fuse_companions` — spawns
  its own tuple and bypasses `adopt_program`; **this is the door that gets
  missed**)
- Modify: `crates/engine/src/game/lifecycle.rs:521`/`:611` (creature load
  loop) and `:897` (creature save write)
- Modify: `crates/engine/src/save.rs` — `CreatureSave` gains `power: f32`
  behind `#[serde(default = …)]` returning `POWER_MAX`, so companions in an
  existing save load charged rather than empty
- Modify: `crates/engine/src/game/turn.rs:668` (`rest`) — the loop that
  full-heals every owned program tops up its reserve too
- Modify: `crates/engine/src/game/party.rs:85` — `PartySlotView::power`
  becomes `Some` for a companion
- Modify: `crates/gui/src/render/battle.rs` — the renamed Power column now
  renders numbers for companions; invert
  `a_companions_fatigue_cell_is_a_dash` rather than deleting it, keeping the
  dash case as the visible symptom of a reserve-less companion
- Test: `crates/engine/src/tests/party.rs`,
  `crates/engine/src/tests/spawning.rs`, save round-trip tests

**Interfaces:**
- Consumes: `PowerReserve` (Task 3).
- Produces, and this is the point of the task:

```rust
// The one constructor for "a program has joined the roster". All four doors
// call it. Nothing about `world.spawn` or `.insert` fails to compile when a
// component is missing from one of four hand-written tuples, so the shared
// constructor is the only barrier available — same role as the existing
// `work_node_parts()` fixture helper.
pub(crate) fn roster_parts(&self) -> (Tamed, Experience, PowerReserve);
```

`needs_tick_system` **stays `With<Player>`**. A companion's reserve never
drains passively; it only moves when the companion spends or something
restores it. That keeps the starvation branch and
`battle::power_attack_multiplier` player-only by construction rather than by
a guard the next author has to remember.

- [ ] **Step 1: Write the failing tests.** (a) Each of the four doors yields
  a companion holding a full reserve — four tests, not one, because they are
  four independent code paths and fusion is the one that will be missed. (b)
  A companion ticked many times has an unchanged reserve. (c) A companion at
  zero Power ticked many times loses no HP — permadeath makes an accidental
  attrition kill a real bug. (d) `rest` refills a drained companion. (e) A
  companion's reserve survives a save round trip, and a `CreatureSave`
  written without the field loads full.
- [ ] **Step 2: Run them.** Expect FAIL.
- [ ] **Step 3: Add `roster_parts` and route all four doors through it.**
  Resist inserting the component at each site; the shared constructor is the
  deliverable, not the component.
- [ ] **Step 4: Wire the save, the rest refill and the view field.**
- [ ] **Step 5: Run the tests.** Expect PASS.
- [ ] **Step 6: Delete-the-fix check on the fusion door specifically** —
  revert `fuse_companions` to its own tuple and confirm that test alone
  fails. That is the silent bug this task exists to prevent.
- [ ] **Step 7: Gate and commit.**

---

### Task 6: The gate and the spender

**Files:**
- Modify: `crates/engine/src/game/combat.rs:852` (`ability_unavailable`) —
  add the reserve check; rewrite the doc comment, which currently states the
  opposite position in as many words ("A need is deliberately not among
  them")
- Modify: `crates/engine/src/game/combat_round.rs:247` — the
  `BattleAction::Special` resolution site, beside the existing "Paid before
  the effect resolves" comment where the cooldown is armed. **The charge goes
  here, not in `use_ability`**: `use_ability` is also the path
  `proc_wielded_routine` and hostile casts take, and the spec keeps both free
- Modify: `crates/engine/src/game/field.rs` — the two movement casts and the
  `FieldBuff` cast route through the shared spender
- Create/modify: `Game::spend_power(entity, cost)`, replacing Task 2's
  interim spend and the inline write at `field.rs:310`
- Test: `crates/engine/src/tests/combat_abilities.rs`,
  `crates/engine/src/tests/combat_specials.rs`,
  `crates/engine/src/tests/wielded.rs`

**Interfaces:**
- Consumes: `routine_power_cost` (Task 4), `PowerReserve` (Task 3), companion
  reserves (Task 5).
- Produces: `Game::spend_power(&mut self, entity: Entity, cost: f32)` — the
  one write path. A missing `PowerReserve` is a no-op, which is what makes
  hostiles safe without a branch.

**A missing reserve refuses, never permits.** `ability_unavailable` reads
`Option<&PowerReserve>` and a `None` returns a reason. Between a companion
that cannot cast and one with infinite Power, the former is the failure that
gets reported.

- [ ] **Step 1: Write the failing tests.** (a) A companion with an empty
  reserve has its Special greyed *with a reason* by
  `battle_special_options`, **and** `battle_set_action` refuses the same
  plan — both halves, since the seam's whole purpose is that they cannot
  disagree. (b) The caster pays: a companion casting draws down the
  *companion's* reserve and leaves the player's untouched. This is the
  assertion that "every companion tracks their power level" actually
  shipped. (c) A hostile with no reserve casts normally. (d)
  `proc_wielded_routine` charges nothing. (e) A lethal Wild Jump still
  charges — the routine ran, and what it found at the address is not
  refundable.
- [ ] **Step 2: Run them.** Expect FAIL.
- [ ] **Step 3: Implement the gate and the spender.**
- [ ] **Step 4: Run the tests.** Expect PASS.
- [ ] **Step 5: Delete-the-fix check** on (b) — the most load-bearing
  assertion in the feature.
- [ ] **Step 6: Gate and commit.**

---

### Task 7: Seams, changelog, and the whole-suite gate

The documentation obligation is part of the work, not a follow-up.
`CLAUDE.md` is loaded into context every turn and a stale seam entry in it
is worse than none.

**Files:**
- Modify: `docs/seams.md` — the reasoning, under titles matching CLAUDE.md's
- Modify: `CLAUDE.md` — then `cp CLAUDE.md AGENTS.md`; they are gitignored
  twins with no tracking to catch drift
- Modify: `CHANGELOG.md` — an entry, but **no version bump and no `## X.Y.Z`
  heading**; those happen at the merge

Seams to add or correct:

- **New:** there are four doors into the roster and `roster_parts()` is the
  only barrier — fusion spawns its own tuple.
- **New:** `PowerReserve`'s float is private; the clamp is the type's, not
  the caller's.
- **New:** `ability_unavailable` is the one gate and `spend_power` the one
  charge, both priced through `routine_power_cost`. The charge is at the
  `BattleAction::Special` site, not in `use_ability`, because the wielded
  proc and hostile casts share that function and stay free.
- **New:** `power_regen_system` needs the underground guard — a third entry
  in the same family as `nest_aggro_tick`.
- **Correct:** the "Needs & rest" tuning section, and every comment claiming
  `fatigue_cost` reaches only `Phase` and `Jump`. That was true of what the
  engine read and false about what the assets contained — 55 files carried a
  value nothing consumed, which is what made this change look like 71 files
  of authoring work when it was a key rename.
- **Correct:** `balance_sim` gates none of this. The suite proves the
  mechanism — costs are charged, the right entity pays, an empty reserve
  refuses — and no number in it.

- [ ] **Step 1: `cargo test --workspace`.** Every test, not a filter.
- [ ] **Step 2: `cargo clippy --workspace`** and `cargo fmt`. Fix warnings
  rather than silencing them.
- [ ] **Step 3: Write the seam entries and the changelog entry.**
- [ ] **Step 4: `cp CLAUDE.md AGENTS.md`.**
- [ ] **Step 5: Commit.**
- [ ] **Step 6: Report to the user that the feature is built and unplayed**,
  and offer to launch it. A green suite is not evidence of play, and this
  change's numbers are explicitly ungated. Suggest `FERAL_DEV_ARENA=1 cargo
  run` for the battle side and `cargo run -- --template stack` for the
  underground scarcity, which is the half no test can see.

---

## Self-Review

**Spec coverage.** Every spec section maps to a task: the regen guard → 1;
Fatigue deletion, the `Coolant`/`coolant_flush` merge, `ConsumeDef::fatigue`,
`trickle_charge`'s retune → 2; `PowerReserve` and its clamp → 3; the unified
field, the multiplier, the 66-file flip, the five untouched files, the
`proc_wielded_routine` carve-out → 4; the four doors, `roster_parts`, the
save field, rest as the refill, `With<Player>` → 5; the gate, the caster
pays, missing-reserve-refuses → 6; vocabulary and the renderer column are
folded into 2 and 3 where their edits land.

Explicitly out of scope in the spec and absent here by design: in-battle
targeting of consumables, hostile reserves, the base power grid, and scaling
a reserve's maximum with level.

**Ordering.** Task 5 precedes Task 6 deliberately. Reversed, every companion
Special would be refused between them.

**Save format.** One bump, in Task 2. Task 3's `hunger` → `power` rename and
Task 5's additive `CreatureSave::power` both ride it.
