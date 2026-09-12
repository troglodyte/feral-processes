# Battle Summons Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Two routines that fork a temporary program fighting beside you for one
battle, in both combat models, plus a repeatable perk widening the rarity window
they roll on.

**Architecture:** A summon is a wild spawn on your side — `spawn_wild_creature_scaled`
with `Hostile`/`WanderAi` stripped, never through `roster_parts()`. It carries one
new marker (`Summoned`) and a `PowerReserve`. The group model seats it by appending
to `Party` **and** `planned`; the tactical model places it beside the invoker and
splices it into `initiative` behind the cursor. `finish_fight` sweeps it
unconditionally.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine crate only), RON assets.

**Spec:** [`docs/superpowers/specs/2026-09-12-battle-summons-design.md`](../specs/2026-09-12-battle-summons-design.md) — read it before Task 1; the plan argues from it.

## Global Constraints

- **Engine crate only.** `crates/engine`. No gui change is expected; Task 9 confirms
  rather than assumes it.
- **No `SAVE_FORMAT_VERSION` bump.** `Summoned` is battle-scoped, `BattleState` and
  `TacticalBattle` are never serialised, and a `Perk` variant appended at the end
  costs nothing. If you find yourself needing a bump, stop and re-read the spec.
- **`Perk` variant order is the save format** — bincode encodes positionally into
  `PlayerSave::unlocked_perks`. **Append only.**
- **Routine names:** `fork_program` (one body), `fork_cluster` (two or three).
  Perk name: **Scheduler**. Do not rename.
- **Vocabulary:** you *run* or *invoke* a routine; the noun is an *invocation*.
  Never "cast"/"spell". A fight's language is security, not swordplay — Integrity,
  not damage.
- **Plans hand over interfaces, not code.** Where this plan omits a body, write it
  in the house style of the file you are editing; match its comment density.
  Comments explain *why*, never *what*.
- **Gates, every task:** `cargo fmt`, then
  `cargo clippy --workspace --all-targets` (`--all-targets` is load-bearing —
  without it test modules go unlinted), then the named test command. Full
  `cargo test --workspace` at Task 9 only.
- **No flaky tests.** Seed every RNG you depend on. Background systems (habitat
  spawning, nests) will interfere with a naive assertion. `crates/engine/src/tests/support.rs`
  has the fixtures — read it before writing a new one.
- **A new `Resource` shifts bevy's query iteration order** under unrelated tests.
  This feature adds none; keep it that way.

---

### Task 1: The `Summoned` marker and the body that carries it

A summon can be created and is always cleaned up. No routine, no perk, no rarity
work yet — this task exists so that every later task has a body that cannot leak.

**Files:**
- Modify: `crates/engine/src/components.rs` — `Summoned` marker
- Modify: `crates/engine/src/game/spawning.rs` — `fork_programs`
- Modify: `crates/engine/src/game/combat_teardown.rs` — the sweep in `finish_fight`
- Modify: `crates/engine/src/tuning.rs` — `SUMMON_SENTINEL`
- Test: `crates/engine/src/tests/summons.rs` (new module; register it in the
  `tests` mod)

**Interfaces:**
- Produces:
  - `components::Summoned` — a unit marker struct, `#[derive(Component)]`, **not**
    `Serialize`/`Deserialize`. Model it on `components::Cloaked`, whose doc explains
    why a battle-scoped marker stays off the save.
  - `Game::fork_programs(&mut self, invoker: Entity, count: u32) -> Vec<Entity>` —
    spawns `count` bodies at the sentinel tile, strips `Hostile` and `WanderAi`
    (`adopt_program`'s two removals), inserts `Summoned` and `PowerReserve`, logs
    one line per body through `creature_label`. Species is a single hard-coded id
    from the shipped catalogue for now; Task 3 replaces it with a real draw.
    Returns the spawned entities.
    **It does not seat them anywhere.**

**Steps:**

- [ ] **Step 1: Write the failing tests.** In the new `tests/summons.rs`:
  - a forked body exists, has `Summoned` and a `PowerReserve`, and carries
    **neither** `Tamed` nor `Experience` nor `Hostile` nor `WanderAi`;
  - `Game::pet_count` is unchanged across a fork (it counts `Tamed` under the
    player — the summon must be invisible to it);
  - `Game::program_role` answers `None` for a forked body;
  - after a fight is torn down through `finish_fight`, **no entity carrying
    `Summoned` is still alive**. Use the `world.get::<Stats>(e).is_none()` idiom
    for "this entity is gone" — do not reach for `World::get_entity`.
- [ ] **Step 2: Run them and watch them fail.**
      `cargo test -p feral-processes-engine summons`
      Expect: does not compile — `Summoned` undefined.
- [ ] **Step 3: Add `Summoned` to `components.rs`.** Doc comment says what it is and
      why it is not saved.
- [ ] **Step 4: Add `SUMMON_SENTINEL` to `tuning.rs`**, in a new labelled section.
      An off-map coordinate, `SORTIE_SENTINEL`'s precedent — find that constant and
      say in the doc comment that it is the same idea, so a reader knows the two are
      siblings rather than a copy.
- [ ] **Step 5: Write `fork_programs`.** Spawn through `spawn_wild_creature_scaled`
      at the sentinel with `depth_mult: 1.0` and `boss: false` (Task 5 replaces the
      multiplier). Strip, insert, log.
- [ ] **Step 6: Add the sweep to `finish_fight`.** Place it **beside** the existing
      `With<StackSpawn>, Without<Tamed>` sweep (`combat_teardown.rs`, look for the
      comment about strays). Unconditional — living or not — on
      `resolve_sortie_battle`'s stated rule that nothing of the opposition may
      outlive the call. Read that function first so the two read alike.
- [ ] **Step 7: Run the tests.** All four pass.
- [ ] **Step 8: Gates and commit.** `cargo fmt && cargo clippy --workspace --all-targets`.
      `feat(combat): a forked program exists and never outlives its fight`

---

### Task 2: `retier_rarity` — moving a spawned body's tier in both directions

**Files:**
- Modify: `crates/engine/src/game/combat_teardown.rs:580-599` (`promote_rarity`)
- Test: `crates/engine/src/tests/summons.rs`

**Interfaces:**
- Consumes: nothing from Task 1.
- Produces: `Game::retier_rarity(&mut self, entity: Entity, new: Rarity) -> Rarity`.
  `Game::promote_rarity(&mut self, entity: Entity) -> Rarity` stays, reduced to a
  caller that computes one rung up and delegates. Its signature and behaviour must
  not change — the nemesis path depends on it.

**Why this rather than a spawn parameter:** rarity is baked into `Stats` inside
`spawn_wild_creature_scaled`, and `Rarity`'s doc says nothing else may apply the
multiplier — `promote_rarity` being the one exception, because it re-scales by the
**ratio** between old and new. A seventh `rarity:` parameter on the spawner would
touch 13 call sites across six files and grow the very parameter list `spawn_pack`
already grouped into `SpawnEscalation`.

**Preserve the known asymmetry rather than fixing it:** `promote_rarity` scales
`mitigation`, which `spawn_wild_creature_scaled` deliberately leaves unscaled.
That is existing nemesis behaviour, `MAX_MITIGATION_PERCENT` bounds it, and
changing it here would be an unrelated retune riding a feature. Say so in the doc
comment so the next reader does not "fix" it.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `retier_rarity` upward from `Ordinary` to `Gold` produces the same `Stats` as
    two `promote_rarity` calls would;
  - **downward is the exact inverse of upward on the same body** — retier up to
    `Gold`, then back to `Ordinary`, and the stats match the original. (Round to
    whole points; `promote_rarity` rounds, so assert within one point per stat and
    say in a comment that the rounding is why.)
  - `promote_rarity` still moves exactly one rung and still stops at `Prismatic`.
- [ ] **Step 2: Run them; watch the first two fail to compile and the third pass.**
      `cargo test -p feral-processes-engine retier`
- [ ] **Step 3: Extract.** `retier_rarity` takes the target tier; `promote_rarity`
      computes `Rarity::ALL.get(old.rank() + 1).unwrap_or(old)` and delegates.
- [ ] **Step 4: Run the tests.** All pass.
- [ ] **Step 5: Gates and commit.**
      `refactor(combat): retier_rarity moves a tier in either direction`

---

### Task 3: A flat non-boss species pool

**Files:**
- Modify: `crates/engine/src/species.rs` (near `windowed_matches`, ~line 1107)
- Modify: `crates/engine/src/game/spawning.rs` — `fork_programs` draws from it
- Test: `crates/engine/src/tests/summons.rs`

**Interfaces:**
- Produces: `SpeciesDb::non_boss_ids(&self) -> Vec<&str>` — every species whose
  `is_boss` is false, **sorted by id**.

**Sorted is not tidiness.** `NeedDb::iter` is sorted for the same reason: an
unsorted pool is an RNG-stream shift between runs, and the symptom surfaces as an
intermittent failure in an unrelated seeded test. The repo has been bitten by this
(`species-habitat-lookup-unsorted-flake`).

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - `non_boss_ids` over the real asset tree contains no species whose `is_boss` is
    true, and is non-empty;
  - it is sorted — assert `is_sorted()`, not a hand-written expected list, which
    would need editing every time a species ships;
  - two calls return identical order.
  - **And a seeded behaviour test:** two `Game`s built on the same seed fork the
    same species. This is the one that would catch an unsorted pool; the
    `is_sorted` assertion alone passes against a `HashMap` that happens to iterate
    in order on this run.
- [ ] **Step 2: Run them; fail.** `cargo test -p feral-processes-engine non_boss`
- [ ] **Step 3: Implement `non_boss_ids`, and point `fork_programs` at it**,
      drawing the index from `resources::GameRng`.
- [ ] **Step 4: Run the tests.** All pass.
- [ ] **Step 5: Gates and commit.** `feat(species): a flat non-boss pool for forking`

---

### Task 4: The Scheduler perk

Independent of Tasks 1–3 — it can be built and reviewed alone.

**Files:**
- Modify: `crates/engine/src/perks.rs` — variant, two queries, census arm, `all()`
- Create: `assets/perks/scheduler.ron`
- Modify: `crates/engine/src/tuning.rs` — the per-rank window constant
- Test: `crates/engine/src/tests/perks.rs`

**Interfaces:**
- Produces:
  - `Perk::Scheduler`, **appended last**. `Perk::all() -> [Perk; 18]` becomes
    `[Perk; 19]`.
  - `perks::summon_rarity_window(perks: Option<&Perks>) -> f64`
  - `perks::summon_tier_ceiling(perks: Option<&Perks>) -> Rarity`

Both are the `Option<&Perks>` family (`roster_slot_bonus` at `perks.rs:276` is the
model) because the subject is always the player. Not the bare-level family — that
exists for formulas whose subject may be a program.

The two, given `n = level(perks, Perk::Scheduler)`:

```rust
// Zero at rank 0 — an unperked fork is ALWAYS Ordinary, and that is what
// "below average" means mechanically. The whole feature's balance rests on it.
window = rarity_mass() * SUMMON_RARITY_WINDOW_PER_RANK * n as f64

// Self-capping: Rarity::ALL is finite, so there is no rung above Prismatic to
// buy. That is what bounds a repeatable perk with no constant to forget.
ceiling = Rarity::ALL[min(n as usize, Rarity::ALL.len() - 1)]
```

`rarity_mass()` is `pub(crate)` in `game/spawning.rs` and its doc names this exact
use: *"a caller that wants a different rare rate can narrow the range it draws
from instead of authoring a second table"*. Do **not** author a second table.

**The census will refuse a perk with no query.** `every_perk_has_a_query_that_answers_what_it_is_worth`
(`perks.rs:802`) loops `Perk::all()` through `one_level_is_worth_something`, whose
match is exhaustive with no `_` arm — so this will not compile until you add the arm.
Name **`summon_rarity_window`** there: it is strictly greater at rank 1 than at
rank 0, which is what that helper asserts. `summon_tier_ceiling` is the second
query and is not the census's.

**Steps:**

- [ ] **Step 1: Write the failing tests** in `tests/perks.rs`, beside the existing
      per-perk floor tests:
  - window is exactly `0.0` at rank 0 and strictly positive at rank 1;
  - ceiling is `Ordinary` at rank 0, `Silver` at 1, `Gold` at 2;
  - ceiling **saturates at `Prismatic`** and does not panic at rank 10 — the
    index clamp is the thing being tested, and an off-by-one here is an index
    panic in a fight.
- [ ] **Step 2: Run them; fail to compile.** `cargo test -p feral-processes-engine perks`
- [ ] **Step 3: Add the variant, bump `all()`'s length, add both queries and the
      census arm.**
- [ ] **Step 4: Add `SUMMON_RARITY_WINDOW_PER_RANK` to `tuning.rs`.** Per-level perk
      *magnitudes* belong there, not in `perks.rs` — only cost lives in the
      catalogue.
- [ ] **Step 5: Write `assets/perks/scheduler.ron`.** Copy the shape of
      `assets/perks/attacker.ron` exactly: `id`, `name`, `description`, `cost`.
      The description must say what a rank buys in the player's words.
- [ ] **Step 6: Run the perk tests plus the asset censuses.**
      `cargo test -p feral-processes-engine perks` and
      `cargo test -p feral-processes-engine assets`
- [ ] **Step 7: Gates and commit.** `feat(perks): Scheduler widens a fork's rarity window`

---

### Task 5: What a forked body is worth

Composes Tasks 2, 3 and 4 into Task 1's spawner.

**Files:**
- Modify: `crates/engine/src/game/spawning.rs` — `fork_programs`
- Modify: `crates/engine/src/tuning.rs`
- Test: `crates/engine/src/tests/summons.rs`

**Interfaces:**
- Consumes: `Game::retier_rarity`, `SpeciesDb::non_boss_ids`,
  `perks::summon_rarity_window`, `perks::summon_tier_ceiling`.
- Produces: `Game::fork_programs(&mut self, invoker: Entity, count: u32, rarity_penalty: u32) -> Vec<Entity>`
  — the `rarity_penalty` parameter is added here.
- New in `tuning.rs`: `SUMMON_STAT_MULT: f32` (start at `0.6`),
  `SUMMON_LEVEL_STAT_STEPS: f32`.

Three multipliers, composed **caller-side** — this is what keeps
`a_spawns_stats_come_from_its_escalation_and_never_from_its_tile` true, and it is
why there is no `(x, y)`-derived term inside the spawner:

```rust
// "Your level", in the game's own currency. party_band_progress is the PLAYER's
// level as a 0..1 fraction of this zone's band — so a companion-invoked fork
// scales off the player too. Expressed as a ratio on the zone ladder, which is
// what leaves balance_sim gating it: `steps` of 1 at zone N is arithmetically
// zone N+1.
let level_mult = self.zone_curve_ratio(self.party_band_progress() * SUMMON_LEVEL_STAT_STEPS);
let depth_mult = level_mult * SUMMON_STAT_MULT;
```

Then per body: roll the tier from the window, clamp it to
`summon_tier_ceiling` lowered by `rarity_penalty` rungs, and `retier_rarity` to it.

**`SUMMON_STAT_MULT` at 0.6 is a guess, not a measurement** — borrowed from
`SETTLEMENT_GIFT_STAT_MULT`, the existing precedent for handicapping a granted
program. `balance_sim` gates none of this: it models no abilities and no perks.
Do **not** write a test asserting a balance figure; the instrument is
`dev-arenas/` and a played session. Say so in the commit message.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - **At perk rank 0 the tier is `Ordinary` over many rolls** — loop at least 200
    forks on a seeded `Game`, assert every one. A single fork passes against a
    broken window by luck.
  - At rank 1, over the same loop, at least one body reaches `Silver` and **none
    reaches `Gold`**. Both halves matter: the first proves the window opened, the
    second proves the ceiling holds.
  - `rarity_penalty: 1` at rank 2 caps at `Silver` where `rarity_penalty: 0` caps
    at `Gold`.
  - A fork's `max_hp` rises when the player levels within the zone band — the
    level term is live. Drive it by setting the player's `Experience::level`, not
    by fighting.
- [ ] **Step 2: Run them; fail.** `cargo test -p feral-processes-engine summons`
- [ ] **Step 3: Compose the multiplier and the tier roll in `fork_programs`.**
- [ ] **Step 4: Run the tests.** All pass.
- [ ] **Step 5: Run the balance gate to confirm nothing moved.**
      `cargo test -p feral-processes-engine balance_sim` — it should be untouched.
      If a curve moved, stop: something reached a shared spawn path it should not have.
- [ ] **Step 6: Gates and commit.**
      `feat(combat): a forked program's level, handicap and tier`

---

### Task 6: The `Summon` effect and the two routines

**Files:**
- Modify: `crates/engine/src/abilities.rs` — variant, `summon_target_mismatch`,
  wire into `AbilityDb::load_dir`'s refusal list (~line 1140)
- Modify: `crates/engine/src/game/combat_round.rs:1254` — `use_ability`'s match
- Create: `assets/abilities/fork_program.ron`, `assets/abilities/fork_cluster.ron`
- Modify: `assets/abilities/README.md` — the `Summon` effect's schema
- Test: `crates/engine/src/tests/combat_abilities.rs`, `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces: `AbilityEffect::Summon { count: u32, extra: u32, rarity_penalty: u32 }`.
  Count rolls `count ..= count + extra`.

**Not `spread`.** That field is a *centred* half-width on `Damage`/`Heal`/`Drain`.
A body count has no reason to be centred, and the shipped pair wants exactly 2..=3.

**`use_ability` gets an `unreachable!` arm, not an implementation.** This follows
`Decompile` exactly — look at its arm (`combat_round.rs:1472`) and match its shape
and the tone of its comment. Like a capture, a summon is not resolved over
recipients and needs model-specific seating, which is why it branches at each
model's Special site instead (Tasks 7 and 8).

**`target: WholeParty`**, pinned by a new `summon_target_mismatch` refusal beside
the eight already in `load_dir`. Do **not** add an `AbilityTarget` variant: its four
methods are all exhaustive on `cell_mark`'s rule, and the seam record on the two
combat models says an invented target variant is "an invented answer every abstract
match in three crates would have to reject".

Asset files:

| file | count | extra | rarity_penalty |
|---|---|---|---|
| `fork_program.ron` | 1 | 0 | 0 |
| `fork_cluster.ron` | 2 | 1 | 1 |

Both need a `power_cost` (`fork_cluster` substantially higher) and a `cooldown`.
`every_runnable_routine_is_priced_in_power` has **no exceptions list** — price
`fork_program` off its own rung, and never reach for an exception.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - a `.ron` file authoring `Summon` with a non-`WholeParty` target is **refused at
    load** — use `scratch_assets_dir` (`support.rs:492`) to author a bad file rather
    than editing a shipped one;
  - a malformed `Summon` file is **skipped with a warning, not a panic** — the
    moddability rule;
  - both shipped files parse, and both carry a non-zero `power_cost`;
  - **deleting both files leaves the game as it was** — the empty-catalogue property
    every data-driven subsystem here holds.
- [ ] **Step 2: Run them; fail.** `cargo test -p feral-processes-engine abilities`
- [ ] **Step 3: Add the variant and the load refusal.** The match in
      `AbilityEffect::field_only` and every other exhaustive match over the enum will
      now fail to compile — answer each one; do not add a `_` arm anywhere.
- [ ] **Step 4: Add the `unreachable!` arm to `use_ability`.**
- [ ] **Step 5: Write the two asset files and the README section.** The README is the
      schema reference for modders and must land in the same change as the field.
- [ ] **Step 6: Run the ability and asset suites.**
      `cargo test -p feral-processes-engine abilities assets`
- [ ] **Step 7: Gates and commit.** `feat(abilities): the Summon effect and two fork routines`

---

### Task 7: Seating a fork in the group model

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs` — the `BattleAction::Special`
  arm (~line 283), `choose_summon_action`
- Modify: `crates/engine/src/game/combat_teardown.rs` — the `bench_or_dissolve` skip
- Modify: `crates/engine/src/game/spawning.rs` — dissolve-the-standing-set
- Test: `crates/engine/src/tests/summons.rs`

**Interfaces:**
- Consumes: `Game::fork_programs(invoker, count, rarity_penalty) -> Vec<Entity>`.
- Produces:
  - `Game::seat_summon_in_group(&mut self, body: Entity)` — pushes to `Party`
    **and** `planned` together.
  - `Game::choose_summon_action(&mut self, body: Entity) -> BattleAction` — a
    weighted pick over the body's own moves, aimed at the front of an engaged group.
  - `Game::dissolve_summons(&mut self)` — kills, does not remove.

**Three traps, all silent.**

*Appending to `Party` alone ships an inert body.* `BattleState::planned` is sized
once at `begin_battle` as `Party.len() + 1`, but `roll_initiative` reads
`Party.0.len()` **live**. So a summon pushed to `Party` only draws an initiative
rung and then never acts (`plan.get(slot)` is `None`), while `living_party`,
`battle_rows`, `battle_active_slot` and hostile targeting all iterate
`planned.len()` and cannot see it. Nothing fails. Push to both.
Appending is safe — the seam forbids *removal*, which shifts every slot behind it;
`actor_entity` maps `Actor::Party(i)` to `Party.0[i-1]` and append leaves every
existing index alone.

*A replaced summon must be killed, not removed.* Removing from `Party` mid-battle
is exactly what the seam forbids. Kill it and let the existing deferred reap carry
it to teardown — the same thing that already happens to a companion that dies
mid-fight. Do not despawn it either: it is still referenced by `Party`.

*`bench_or_dissolve` inserts `Downed` unconditionally.* Read
`crates/engine/src/game/trade.rs:640` — it never asks whether the body is `Tamed`.
Left alone, every dead summon lands in `DownedPrograms` on Forgiving and takes
`dissolve_tamed_program` on Permadeath. Skip on `Summoned`.

**Use `choose_summon_action`, not `Game::choose_wild_action`.** The latter is the
trained policy; its features speak group indices and aggro slots from the hostile
side, and `combat_policy.rs`'s weights are pinned against a design boundary this
would cross.

**Steps:**

- [ ] **Step 1: Write the failing tests.**
  - a forked body **acts** — it appears in `battle_rows`, takes a turn, and is a
    legal target for hostiles and for a `WholeParty` heal. *Asserting only that it
    exists passes against the inert bug above, so assert on it acting.*
  - re-invoking replaces the standing set: the old bodies are dead and **do not act
    again**, and `Party`'s length does not shrink.
  - a dead summon is **not** in `DownedPrograms` on Forgiving, and is not dissolved
    as a tamed program on Permadeath. Two tests, one per `DifficultyMode`.
  - a summon earns no XP, and its kills still pay the player — `finish_hostile`
    keys the payout on the victim, not the killer, so this should pass without new
    code. If it does not, stop and re-read rather than adding a term.
- [ ] **Step 2: Run them; fail.** `cargo test -p feral-processes-engine summons`
- [ ] **Step 3: Write `seat_summon_in_group`, `choose_summon_action`,
      `dissolve_summons`, and branch the Special arm.**
- [ ] **Step 4: Add the `Summoned` skip to `finish_fight`'s `bench_or_dissolve` loop.**
- [ ] **Step 5: Run the tests.** All pass.
- [ ] **Step 6: Run the neighbouring suites** — a `Party` change reaches further than
      it looks: `cargo test -p feral-processes-engine combat party permadeath battle_timeline`
- [ ] **Step 7: Gates and commit.** `feat(combat): forks fight in the group model`

---

### Task 8: Seating a fork on the battle map

**Files:**
- Modify: `crates/engine/src/tactical/mod.rs` — initiative insertion
- Modify: `crates/engine/src/tactical/deploy.rs` — `nearest_free` visibility
- Modify: `crates/engine/src/tactical/turn.rs` — Special-site branch, placement
- Modify: `crates/engine/src/tactical/ai.rs` — `tactical_ai_actor`'s gate
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Produces: `TacticalBattle::insert_after_cursor(&mut self, body: Entity)`.
- `deploy::nearest_free(board: &Board, taken: &BTreeSet<(i32, i32)>, want: (i32, i32)) -> Option<(i32, i32)>`
  becomes `pub(crate)` — it is already exactly the breadth-first search needed.
  Build `taken` from `TacticalBattle::bodies()`.
- `Game::tactical_ai_actor` gate widens from `Hostile` to `Hostile` **or**
  `Summoned`.

**The insertion rule is the whole of this task.** `TacticalBattle`'s cursor names a
**body, not a position** — that is why `remove` (`tactical/mod.rs:180`) decrements
`turn` when it takes something out ahead of the cursor. Insertion carries the same
trap mirrored:

```rust
// Behind the cursor, always. Inserting AHEAD of it shifts every later entry
// down one and somebody acts twice; `remove`'s three cases are the same
// problem in the other direction. turn + 1 is the only index that is safe
// regardless of where the cursor sits, and it reads right at the keyboard:
// you call it, it acts next.
self.initiative.insert(self.turn + 1, body);
```

**The AI gate is the first exception to a stated rule.** `tactical_ai_actor`'s doc
says "every party body is the player's to command", and that is why
`tactical_drive_turn` exists as a separate door for the headless arena. A summon is
the first party body that drives itself — update that doc comment to say so, or the
next reader will read the gate as a bug. Sidedness needs **nothing**:
`tactical_sides` is already relative to the actor, which is why
`tactical_drive_turn` works at all.

**Steps:**

- [ ] **Step 1: Write the failing tests** in `tests/tactical.rs` (use the local
      `tactical_fight` / `tactical_pack` helpers at the top of that file):
  - a fork lands on a free walkable cell adjacent to the invoker, and never on an
    occupied one;
  - **nobody else loses or doubles a turn.** Record the order of acting bodies
    **by identity** across a full round after a mid-round fork, and compare against
    the pre-fork order with the summon spliced in. *A count of turns taken is
    conserved under a cursor shift and would pass against the bug* — this is the
    same trap the initiative seam already documents.
  - the summon takes its own turn without waiting for input —
    `Game::tactical_awaits_input` is `false` on its turn.
  - a fork on a board with no free adjacent cell is refused cleanly and spends
    nothing.
  - **no `Summoned` body survives either tactical ending** — a jack-out, and a
    walk off the board edge. Task 1 covered the group model's teardown; all five
    endings funnel through `finish_fight`, but the two that only a battle map can
    reach have never run the sweep, so assert on them here.
- [ ] **Step 2: Run them; fail.** `cargo test -p feral-processes-engine tactical`
- [ ] **Step 3: Add `insert_after_cursor`, make `nearest_free` `pub(crate)`, branch
      the tactical Special site, widen the AI gate and update its doc.**
- [ ] **Step 4: Run the tests.** All pass.
- [ ] **Step 5: Run the whole tactical suite plus the arena** — the AI gate is shared:
      `cargo test -p feral-processes-engine tactical arena`
- [ ] **Step 6: Gates and commit.** `feat(combat): forks fight on the battle map`

---

### Task 9: Confirm, document, record

**Files:**
- Modify: `CHANGELOG.md`, root `Cargo.toml` is **not** touched (the version bump
  happens at the merge, never on a branch)
- Modify: `CLAUDE.md` — the seam rules
- Modify: `.claude/skills/seams/references/combat.md` — the traps

**Steps:**

- [ ] **Step 1: Confirm the gui needs nothing.** The spec *expects* zero gui change —
      `battle_rows` builds party rows off `planned`, and the tactical map draws a
      non-`Hostile` body as an ally. **Confirm it, do not assume it:** check how
      `render/mod.rs::glyph_color` and the tactical body draw resolve a `Summoned`
      body, and check the turn-order strip renders a spliced entry. If something is
      missing, fix it here and say so.
- [ ] **Step 2: Full suite.** `cargo test --workspace`. This is the gate — passing
      only the tests you wrote is not evidence of correctness.
- [ ] **Step 3: `cargo clippy --workspace --all-targets`** clean.
- [ ] **Step 4: Write the CHANGELOG entry** under a new version section. Which digit
      moves is decided by `CHANGELOG.md`'s preamble; no save format broke, so this is
      not a breaking change.
- [ ] **Step 5: Write the seams, all three tiers, in order.** The `seams` skill
      documents the order and the exact calls. There are three new seams here:
      the containment-by-omission rule, the `Party`+`planned` pairing, and the
      insert-behind-the-cursor rule. Argument to the memory graph, trap to
      `references/combat.md`, **one sentence each** to `CLAUDE.md` — that budget is
      the point, the file is loaded every turn.
- [ ] **Step 6: Say plainly what has not been verified.** A green suite is not
      evidence of play. `SUMMON_STAT_MULT` is unmeasured, `balance_sim` gates none of
      this, and nothing here has been seen on a screen. Report that once, without
      hedging.
- [ ] **Step 7: Commit.** `docs: changelog and seams for battle summons`

---

## Notes for the executor

- **Do not push.** Commits are free; pushing needs an explicit ask from the user.
- **Don't reach for a fresh `Game::new` when a `dev-saves/` template would do** —
  `dev-arenas/` and `--template` exist so mid-run state costs nothing.
- If many tests fail at once with `NotFound` on an assets path, that is stale build
  artifacts from a directory rename, not real failures. Fix with
  `cargo clean -p feral-processes-engine -p feral-processes-app-core`, never a full
  `cargo clean` (which costs a ~3.5 minute cold rebuild of the Bevy graph).
- `cargo test -p feral-processes-engine` and `cargo test --workspace` are **different
  builds** and shift the RNG stream differently. A seeded test that passes under one
  and fails under the other is that, not a bug in your code — but confirm by reading
  the path, never by re-running until it goes green.
