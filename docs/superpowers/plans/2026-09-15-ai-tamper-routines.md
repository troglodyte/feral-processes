# Tamper Routines Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Five battle-map routines that tamper with the hostile AI's decision-maker
(its temperature, its forecast, its side, and what it sees), plus six flavour
routines on existing effects, two gear mods, and the research node that teaches
them.

**Architecture:** One new `AbilityEffect::Tamper { kind: TamperKind, duration }`.
It is resolved by its own branch in `run_tactical_routine`, the way `Decompile` and
`Summon` are. That branch writes a battle-scoped `components::Tampered` on each
recipient, and for a Hallucination it also writes `Decoy`s into `TacticalBattle`.
Four hooks read `Tampered`, each through one door:
- `Game::decision_temperature` feeds every tactical AI entry point.
- `tactical_sides` and `best_aim` go through one actor-relative allegiance, which Injected flips.
- Hallucinating replaces the target list with a decoy.
- A pure planner extracted from `run_tactical_beat` is called by both the turn and `Game::tactical_forecast`.

Entries age when **the tampered body's own turn is handed on**, in `hand_on_turn`,
and never in `tick_one_combatant`. The group model hides the effect behind
`AbilityEffect::tactical_only()`.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (standalone, engine only), bevy + bevy_egui
(gui, through `Painter`), RON assets.

**Spec:** [`docs/superpowers/specs/2026-09-15-ai-tamper-routines-design.md`](../specs/2026-09-15-ai-tamper-routines-design.md).
Read it before Task 1. The plan argues from it and does not repeat its reasons.

## Global Constraints

- **No `SAVE_FORMAT_VERSION` bump.**
  - `Tampered` has no serde derive and appears nowhere in `save.rs`.
  - `TacticalBattle` is never serialised.
  - `TamperKind` derives `Serialize`/`Deserialize` only because it is asset schema.
  - If you find yourself touching `save.rs`, stop.
- **No new `Resource`.** A new resource shifts bevy's query iteration order under
  unrelated seeded tests. Decoys live in `TacticalBattle`, which already exists.
- **Routine ids are frozen once shipped.** Use exactly the ids below. Display names
  follow `<Family> <Scope>` (`assets/abilities/README.md` § Naming).
- **Vocabulary.**
  - You *run* or *invoke* a routine. Never "cast" or "spell", including in log lines and comments. The spec says "cast" in places; don't copy it.
  - A fight speaks security (Integrity, not damage).
  - No player-facing "tick".
- **Plans hand over interfaces, not code.** Write bodies in the house style of the
  file you are editing, at its comment density. Comments explain *why*.
- **Gates, every task:**
  1. `cargo fmt`
  2. `cargo clippy --workspace --all-targets` (`--all-targets` is load-bearing)
  3. the named `cargo test -p <crate> <filter>`
  4. `cargo test -p feral-processes-engine tactical` for every task that touches `tactical/`
  - Run full `cargo test --workspace` at Task 11 only.
  - Cargo's exit code is lost through `| tail`, so check `PIPESTATUS` or don't pipe.
- **No flaky tests.** Seed every RNG. Place bodies with `TacticalBattle::move_to`
  rather than trusting a seeded deployment. Background habitat and nest systems
  interfere with naive assertions.
- **Mutation-check the tests marked (M).** Delete or neuter the behaviour, watch the
  test fail, then restore it. Say in the commit body that you did.
- **Commit per green task** on `feat/ai-tamper-routines`.
  - Stage explicit paths, never `git add -A`.
  - Never push, merge, tag or bump the version. Landing is a separate step.
- **Fixtures.**
  - Engine fixtures are in `crates/engine/src/tests/support.rs` (`rng_unadvanced_by` at ~2645).
  - Tactical ones are private to `crates/engine/src/tests/tactical.rs`: `tactical_fight` 135, `wait_for_turn` 182, `run_ai_rounds` 833, `only_routine` 620, `place_bodies`/`place_one`/`block_cell` ~2200, `ranged_fight` 2233, `fight_against` 2384, `open_ground` 3245, and `marooned` near 1038.
  - New tests go in a **new module `crates/engine/src/tests/tamper.rs`**, registered in the tests mod. Lift the fixtures you need to `pub(super)` rather than copying them.

## Decisions this plan takes that the spec left open

These are stated here so a reviewer can veto them in one place.

1. **Dropout ships as two rungs, `dropout` (Single) and `dropout_group` (Group).**
   - The spec names one group Stun.
   - `every_battle_ability_family_is_contiguous_from_single_upward` refuses a family that starts at Group.
   - Both `Hard Lock Group` and every other Stun family name are taken.
   - So the family gets its Single rung. That makes eleven routines, not ten.
2. **Tactical-only routines are exempt from the family-contiguity census.** Heat Injection Group
   and Hallucination Group have no Single rung.
   - The ladder rule exists for species kits and the hunt pool.
   - A tamper routine is research-taught and never enters either.
   - The exemption is one more `.filter` clause beside `Summon`'s, with its reason in the doc.
3. **Every decoy goes on a free cell.**
   - The spec puts the first decoy "on the aimed cell". The aimed cell is usually the hostile standing there, and a decoy under a body's own feet is struck at distance zero without a step.
   - Instead, decoys fill the walkable, unoccupied, decoy-free cells of `shape_cells(aim)`. They are ordered by Chebyshev distance from the aim, then `(y, x)`.
   - The aimed cell leads whenever it is free.
4. **The forecast covers a body that has not started its turn yet.**
   - That includes the acting body before its first beat: `!walk_planned() && !acted`.
   - It is omitted mid-turn. That is what makes "exact at the moment the turn begins" testable.
5. **A self-applied entry skips its first ageing.**
   - A companion's radius tamper can catch the invoker. Aged at the end of that same turn, it would lose a turn the target never had.
   - `TamperEntry::fresh` is set when `recipient == actor` and consumed by the first hand-on instead of a decrement.
6. **Only a hallucinating actor destroys a decoy.** A body that cannot see a decoy cannot aim at it.
   This matters only once hostiles can hallucinate each other, which is out of scope.
7. **The strip's tags and forecast draw as a block of lines beneath the glyph strip.**
   - The strip is a horizontal row of two-cell glyph rungs with no room for text.
   - The block has one line per body that carries a tag, a forecast or is taken over, in initiative order.
   - Each line reads `<glyph> TAG TAG ▸ <routine name>`.
   - Width census: the widest possible line fits the map pane less two `strip_inset`s at 1280×720.
   - A taken-over companion's line carries `HIJACK`, and its strip glyph draws in `palette::WARN`.
8. **Prices** (the spec's "to be priced during the plan"). They are anchored on `deadlock` (6/cd 2),
   `hard_lock` (10/cd 4) and `race_condition` (13/cd 4). All tamper durations are the spec
   table's, not the RON example's.

   | id | name | target | shape / range | effect | cd | power |
   |---|---|---|---|---|---|---|
   | `cold_sample` | Cold Sample Single | OneEnemyGroupFront | Single / 0–5 | `Temperature(0.0)`, 3 | 3 | 6.0 |
   | `heat_injection` | Heat Injection Group | WholeEnemyGroup | Radius(1) / 0–4 | `Temperature(2.0)`, 2 | 4 | 12.0 |
   | `inference_probe` | Inference Probe Single | OneEnemyGroupFront | Single / 0–6 | `Profiled`, 3 | 3 | 5.0 |
   | `prompt_injection` | Prompt Injection Single | OneEnemyGroupFront | Single / 0–3 | `Injected`, 1 | 5 | 12.0 |
   | `hallucination` | Hallucination Group | WholeEnemyGroup | Radius(2) / 0–5 | `Hallucinating(decoys: 3)`, 3 | 5 | 14.0 |
   | `gradient_descent` | Gradient Descent Single | OneEnemyGroupFront | derived | `Damage(power: 7, spread: 2)`, accuracy as `segfault_v1` | 2 | 7.0 |
   | `backprop` | Backprop Single | OneEnemyGroupFront | derived | `Drain(power: 8, spread: 2, heal_fraction: 0.4)`, accuracy as `skim_v2` | 3 | 8.0 |
   | `dropout` | Dropout Single | OneEnemyGroupFront | derived | `Debuff(kind: Stun, power: 0, duration: 1)` | 3 | 7.0 |
   | `dropout_group` | Dropout Group | WholeEnemyGroup | derived | `Debuff(kind: Stun, power: 0, duration: 1)` | 5 | 13.0 |
   | `fine_tune` | Fine Tune Single | OneAlly | derived | `Buff(kind: Atk, power: 5, duration: 3)` | 2 | 7.0 |
   | `data_poisoning` | Data Poisoning Single | OneEnemyGroupFront | derived | `Debuff(kind: Bleed, power: 5, duration: 3)` | 3 | 8.0 |

   None of these carries `wild_weight`, because a research node may not grant a hunt-only routine.

---

### Task 1: The schema, and the effect hidden everywhere it cannot run

`Tamper` parses, validates and is excluded from every non-battle-map chooser. The
five tamper `.ron` files ship here so every test from now on uses real ids. Nobody
knows them yet, so nothing can run one.

**Files:**
- Modify: `crates/engine/src/abilities.rs`
  - `TamperKind` and the `Tamper` variant, beside `Summon` at ~511
  - `tactical_only()` beside `field_only()` at 535
  - arms in `affinity_kind` (551), `breaks_cloak` (592) and `effect_label` (~1354)
  - a `tamper_faults()` load check wired where `summon_target_mismatch` is (~1219)
- Modify: `crates/engine/src/game/combat_round.rs:~1623`, the `use_ability` match. `Tamper` joins `Decompile`/`Summon` as `unreachable!`, with a one-line reason: it is seated by `run_tactical_routine`.
- Modify: the routine choosers that must skip tactical-only effects:
  - `game/combat_enemy.rs:74` `wild_routine_ready`
  - `game/combat.rs` `battle_special_options` (**not** `tactical_routine_options`), so filter at that caller rather than inside `special_options_for`
  - `game/combat_round.rs:1227` `choose_summon_action`
  - `game/combat.rs:~1283` `wieldable_routines`
  - `sortie.rs:~660`
- Modify: `game/combat.rs:1326` `ability_unavailable`. A tactical-only routine outside a tactical fight is refused. Memory says these strings are row fragments, so write one that reads as a row suffix.
- Modify: `crates/engine/src/tests/assets.rs`
  - arms in the three exhaustive census copies (729, 779, 1001)
  - the contiguity exemption (Decision 2)
  - the stale "Nothing shipped authors a `shape:` yet" doc at ~870
- Create: `assets/abilities/{cold_sample,heat_injection,inference_probe,prompt_injection,hallucination}.ron`, per the Decision 8 table, each with an authored `shape:` and `range:`
- Test: `crates/engine/src/tests/tamper.rs` (new, registered)

**Interface:**
- `pub enum TamperKind { Temperature(f32), Profiled, Injected, Hallucinating { decoys: u32 } }`
  - derives `Clone, Copy, Debug, PartialEq, Serialize, Deserialize`
  - `pub fn slot(self) -> TamperSlot`
- `pub enum TamperSlot { Temperature, Profiled, Injected, Hallucinating }`
  - derives `Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord`
  - Heat and Cold share `Temperature`, which is what makes one replace the other.
- `AbilityEffect::Tamper { kind: TamperKind, duration: u32 }`
- `AbilityEffect::tactical_only(&self) -> bool` is an exhaustive match that answers true for `Tamper` only.
- `affinity_kind` → `None` ("a temperature is a temperature", per the spec). `breaks_cloak` → `true`.
- `tamper_faults` refuses:
  - `duration == 0`
  - a non-finite or negative temperature
  - `decoys == 0`
  - an ally-facing `target` (a tamper names the other side)
  - `Hallucinating` on a non-`Radius` shape

  A refused file is skipped with a warning, per the load pattern.

- [ ] **Step 1: Write the failing tests.** In `tamper.rs`:
  - `the_five_tamper_routines_load_with_the_kinds_they_author`: each id's effect matches the table.
  - `a_tamper_routine_never_runs_on_the_map`: `field_runnable()` is false, and `tactical_only()` is true for all five and false for `deadlock`.
  - `a_malformed_tamper_is_refused_at_load`: one case per `tamper_faults` refusal. Build `AbilityDef`s in the test and call the fault fn directly, the way the summon mismatch is tested (find it with `rg summon_target_mismatch crates/engine/src/tests`).
  - `no_hostile_readies_a_tamper_routine` (spec test 13): a hostile whose only routine is `cold_sample` gets `None` from `wild_routine_ready`, and the same body with `deadlock` gets `Some`.
  - `the_group_models_routine_rows_omit_tamper_routines` (spec 14): the player knows `cold_sample` and `deadlock`. `battle_special_options` names `deadlock` only, and `tactical_routine_options` names both.
  - `a_tamper_routine_is_refused_outside_a_battle_map`: `ability_unavailable` is `Some` in a group fight.
  - The `choose_summon_action`, `wieldable_routines` and sortie exclusions each get one assertion. A sortie reaching `use_ability` with a `Tamper` would hit the `unreachable!`, so that test is the one that matters.
- [ ] **Step 2: Run and watch them fail to compile.** Run `cargo test -p feral-processes-engine tamper`.
- [ ] **Step 3: Implement the interface and the filters, then author the five files.**
- [ ] **Step 4: Run the gates and the asset censuses.** Run `cargo test -p feral-processes-engine tamper` and `cargo test -p feral-processes-engine assets`.
  - Expect every census green.
  - If `every_everyone_scope_routine_pays_the_everyone_tier_price` or a power census objects, fix the file rather than the census.
- [ ] **Step 5: Commit.** Message: "Tamper effect: schema, load checks, and hidden from every non-battle-map chooser".

---

### Task 2: `Tampered`, applying it, and ageing it on the body's own turn

Running a tamper writes entries. The player is never a recipient. Entries age at
the tampered body's hand-on and are gone when the fight ends. No hook reads them yet.

**Files:**
- Modify: `crates/engine/src/components.rs`: `Tampered` and `TamperEntry`, beside `Cloaked` (1178)
- Modify: `crates/engine/src/tactical/turn.rs`
  - `run_tactical_routine` (581): a `Tamper` branch beside `Decompile`/`Summon`, calling `apply_tamper`
  - `tactical_use_routine` (449): the player refusal, beside the capture refusal at 487–500
  - `hand_on_turn` (681): age the actor's entries **first**, while it is alive
- Create: `crates/engine/src/game/tamper.rs` for `apply_tamper`, `age_tamper` and the log lines (register it in `game/mod.rs`)
- Modify: `crates/engine/src/game/combat_teardown.rs:164` `clear_battle_status_effects`: remove `Tampered` for player, hostiles and companions, the way `uncloak` removes `Cloaked`
- Test: `tamper.rs`

**Interface:**
- `#[derive(Component, Default, Debug, Clone)] pub struct Tampered { entries: BTreeMap<TamperSlot, TamperEntry> }`, with methods:
  - `apply(kind, duration, fresh)` replaces the slot's entry, which is refresh, never stack
  - `temperature() -> Option<f32>`
  - `has(TamperSlot) -> bool`
  - `remove(TamperSlot) -> Option<TamperEntry>`
  - `age() -> Vec<TamperKind>`, which returns the expired kinds and consumes `fresh` instead of decrementing
  - `slots() -> impl Iterator<Item = (TamperSlot, TamperKind)>`
  - `is_empty()`
  - Remove the component when it empties, so `get::<Tampered>().is_none()` means untampered.
- `pub struct TamperEntry { pub kind: TamperKind, pub remaining: u32, fresh: bool }`
- `Game::apply_tamper(&mut self, actor: Entity, ability: &AbilityDef, kind: TamperKind, duration: u32, aim: (i32, i32))`:
  1. Take `reach::recipients` and drop the player.
  2. For `Hallucinating`, stub for now: insert nothing. Task 6 fills it.
  3. Otherwise apply to every recipient with `fresh = recipient == actor`.
  4. Log one take-hold line per recipient.
  5. Call `break_cloak(actor)` when `ability.effect.breaks_cloak()`. `use_ability` does this after its loop, and this branch bypasses `use_ability`.
- `Game::age_tamper(&mut self, body: Entity)` logs one wear-off line per expired kind.
- Log lines use the label rule `tick_combatant_upkeep` uses (`entity_label` for hostiles, `creature_label` for companions):
  - take hold: "{label}'s sampler runs cold." / "…runs hot." (compare against `tuning::TACTICAL_AI_TEMPERATURE`), "{label}'s policy is profiled.", "{label} accepts an injected prompt.", "{label} starts seeing decoys."
  - wear off: "{label}'s sampler settles.", "{label}'s profile goes stale.", "{label} rejects the injection.", "{label} sees clearly again."
- Player refusal: a `Tamper` routine whose tactical shape is `Single` and whose aim's occupant carries `Player` is refused **before** cooldown or Power moves.
- **Check before relying on `hand_on_turn`:** grep every caller of `TacticalBattle::end_turn` and `Game::tactical_end_turn`, including a stunned body's skipped turn and `run_tactical_beat`'s empty-targets branch. Confirm each goes through `hand_on_turn`. Any path that advances the cursor without it would silently freeze an entry. Route it through `hand_on_turn`, or report it.

- [ ] **Step 1: Write the failing tests.**
  - `a_tamper_lands_on_every_body_in_its_radius_but_the_player` (spec 9, first half): Heat Injection aimed so the radius holds the player, a companion and two hostiles.
  - `a_single_tamper_at_the_player_is_refused_before_anything_is_spent` (spec 9): Power, cooldowns and `acted` are unchanged, and the turn is still the player's.
  - `reapplying_a_kind_refreshes_and_heat_replaces_cold`: two kinds coexist.
  - **(M)** `a_one_turn_injection_on_a_body_that_has_acted_is_live_on_its_next_turn` (spec 11):
    1. Let the hostile act this round.
    2. Have the player run `prompt_injection` on it.
    3. Advance with `tactical_end_turn` until the round wraps. Upkeep runs.
    4. At the start of the hostile's next turn it still has `Injected`.
    5. After that turn is handed on, the entry is gone.
    - Mutation: age in `tick_one_combatant` instead and watch it fail.
  - `a_self_applied_entry_is_not_aged_by_the_turn_that_applied_it` (Decision 5).
  - `tampered_is_gone_when_the_fight_ends` (spec 12, component half): win the fight through the normal door. No entity carries `Tampered`.
- [ ] **Step 2: Watch them fail.** Run `cargo test -p feral-processes-engine tamper`.
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the gates**, plus `cargo test -p feral-processes-engine tactical` and `cloak`.
- [ ] **Step 5: Commit.** Message: "Tampered: applied by a tamper routine, aged on the tampered body's own hand-on".

---

### Task 3: One temperature door

**Files:**
- Modify: `crates/engine/src/tactical/ai.rs`: add `decision_temperature` and replace the constant at `tactical_ai_turn` (176), `tactical_ai_beat` (244), `tactical_auto_beat` (269) and `tactical_drive_turn` (318). Remove the now-unused import at :35 if clippy says so.
- Test: `tamper.rs`

**Interface:** `pub(crate) fn decision_temperature(&self, body: Entity) -> f32`
- returns `Tampered::temperature()` when present
- otherwise `tuning::TACTICAL_AI_TEMPERATURE`
- `tactical_ai_turn_at(temperature)` stays as the test hook it is
- `ENEMY_POLICY_TEMPERATURE` is untouched

After this task, `rg TACTICAL_AI_TEMPERATURE crates/engine/src/tactical` should show only `decision_temperature` and the Task 2 log comparison.

- [ ] **Step 1: Write the failing tests.**
  - **(M)** `a_cold_sampled_hostile_draws_nothing_over_its_turn` (spec 1). Use the `marooned` board, where more than one candidate cell scores, and the two-games-one-draw comparison from `choosing_a_cell_at_zero_temperature_does_not_move_the_seeded_stream`. The untampered twin *does* draw.
  - **(M)** `every_tactical_ai_door_reads_the_temperature_door` (spec 2): the same Cold assertion through each of `tactical_ai_turn`, `tactical_ai_beat` (looped to `Acted`), `tactical_auto_beat` and `tactical_drive_turn`. Mutation: revert any one site to the constant and exactly that case fails.
  - `heat_leaves_the_draw_in_place`: a Heat-tampered body still draws. This guards against a door that zeroes every tampered body.
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the gates** plus `tactical`.
- [ ] **Step 5: Commit.** Message: "decision_temperature is the one door every tactical AI entry point reads".

---

### Task 4: Injected: one allegiance, flipped

**Files:**
- Modify: `crates/engine/src/tactical/ai.rs`
  - `tactical_sides` (469)
  - `best_aim` (593), which today decides "wanted" by `Hostile` alone, not actor-relative
  - `swing_at_best_neighbour` (648), only if it reads side
- Read, don't assume: `tactical/turn.rs:266` `tactical_attack` has no side filter today. Pin that with a test.
- Test: `tamper.rs`

**Interface:** `fn acts_for_hostiles(&self, actor: Entity) -> bool`
- `Hostile(actor) != Injected(actor)`
- `tactical_sides` classifies every other body as ally when `Hostile(other) == acts_for_hostiles(actor)`. The cloak partition and the never-empty rule apply to the resulting `targets` exactly as now.
- `best_aim`'s "wanted" becomes:
  - helpful: the actor, or a body with `Hostile == acts_for_hostiles(actor)`
  - otherwise: a body with `Hostile != acts_for_hostiles(actor)`
- Other bodies still see an injected hostile as their own. Only the injected body's view flips.

- [ ] **Step 1: Write the failing tests.**
  - **(M)** `an_injected_hostile_swings_at_a_packmate` (spec 5): two adjacent hostiles, the player out of reach, the first injected. After its turn, the packmate's Integrity fell, and the swing's `BoltCue.to` is the packmate's cell.
  - **(M)** `a_packmate_killed_by_an_injected_hostile_pays` (spec 5): set the packmate's Integrity to 1. After the kill, `BattleRewards` (or `Game::fight_rewards_mut`'s view of it) holds that body's XP and loot. Compare against the same kill made by the player.
  - **(M)** `an_injected_healer_aims_its_heal_at_the_party` (spec 6): a hostile with only a `Heal` routine, injected, a hurt companion in range. The companion's Integrity rose and no hostile's did.
  - `an_uninjected_packmate_still_treats_the_injected_one_as_its_own`.
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the gates** plus `tactical` and `cloak`. The cloak suite exercises `tactical_sides` hardest.
- [ ] **Step 5: Commit.** Message: "Injected flips the one allegiance tactical_sides and best_aim read".

---

### Task 5: A companion taken over

**Files:**
- Modify: `crates/engine/src/tactical/ai.rs`: `tactical_ai_actor` (198) also accepts a taken-over body, and its doc comment says so. Fix the stale "gate is `Hostile`" docs on `tactical_auto_beat` and `tactical_drive_turn` while you are there. They already miss `Summoned`.
- Read: `crates/app-core/src/app/tactical.rs:223` `advance_tactical`. It already beats with `tactical_ai_beat` whenever `tactical_player_turn()` is false. Confirm no app-core change is needed; if one is, add it here.
- Test: `tamper.rs`, and one app-core test in `crates/app-core/src/tests/tactical.rs`

**Interface:** `pub(crate) fn taken_over(&self, body: Entity) -> bool`
- true when the body is not `Hostile`, not the player, and its `Tampered` has `Temperature` or `Injected`
- `tactical_awaits_input` falls out of `tactical_ai_actor` unchanged
- A taken-over companion's intent is still a swing: `run_tactical_beat` hands routines to `Hostile` actors only, and that stays.

- [ ] **Step 1: Write the failing tests.**
  - **(M)** `a_heat_caught_companion_stops_awaiting_input_and_returns_when_it_ends` (spec 10):
    1. `tactical_awaits_input()` is false on its turn.
    2. `tactical_ai_beat` drives it to `Acted`.
    3. After the entry expires on its own hand-on, its next turn awaits input again.
  - `a_profiled_companion_is_still_commanded` (spec 10).
  - `an_injected_companion_swings_at_the_party`.
  - app-core: `a_taken_over_companion_plays_without_a_key`. With `tactical_auto` off, `advance_tactical` with enough `dt` moves past the companion's turn.
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the gates** plus `tactical` for both engine and app-core (`cargo test -p feral-processes-app-core tactical`). If app-core `tests::creation` fails, it is the known unrelated flake; re-run that module and do not fix it here.
- [ ] **Step 5: Commit.** Message: "A companion under a Temperature or Injected entry is driven by the AI until it ends".

---

### Task 6: Hallucination: decoys

**Files:**
- Modify: `crates/engine/src/tactical/mod.rs`
  - `Decoy`, the `decoys: Vec<Decoy>` field, and its accessors
  - initialise the field at every `TacticalBattle` construction site
- Modify: `crates/engine/src/game/tamper.rs`
  - the `Hallucinating` arm of `apply_tamper`
  - `settle_decoys`
  - `tactical_strike_decoy`
- Modify: `crates/engine/src/tactical/ai.rs`
  - `tactical_sides`: a hallucinating actor's targets become the cell of its nearest opposing decoy, with its allies unchanged
  - `swing_at_best_neighbour`: a decoy target calls `tactical_strike_decoy`
  - `best_aim`: for a hallucinating actor, +1 per opposing decoy in `shape_cells(aim)`, −1 per own-side body, and party bodies count 0 because it doesn't see them
- Modify: `crates/engine/src/tactical/turn.rs`
  - `hand_on_turn` calls `settle_decoys` after ageing
  - `run_tactical_routine`'s ordinary branch: when the actor is hallucinating, compute `shape_cells` **before** `use_ability` (the actor may die in it) and remove opposing decoys in them afterwards, logging "{label}'s {routine name} passes through a decoy." once per routine
- Test: `tamper.rs`

**Interface:**
- `pub(crate) struct Decoy { pub cell: (i32, i32), pub owner_hostile: bool, pub glyph: char, pub color: GlyphColor, pub of_player: bool }`
  - `of_player` exists because the player's `@` is drawn in the `PLAYER` role, not its `GlyphColor`
- `TacticalBattle` methods: `place_decoy(Decoy)`, `decoys() -> &[Decoy]`, `take_decoy_at(cell, owner_hostile) -> Option<Decoy>`, `retain_decoys(impl FnMut(&Decoy) -> bool)`
- Placement (Decision 3):
  - candidates are `reach::shape_cells(board, actor_cell, aim, shape)` filtered to walkable, unoccupied and decoy-free
  - sorted by `(chebyshev(aim), y, x)` and truncated to `decoys`
  - **no `GameRng`**
  - If no cell is free, nothing is inserted on anyone.
- The entry goes only on recipients with `Hostile != Hostile(actor)`.
- Nearest opposing decoy: by the distance `swing_range` uses (read it; don't guess Chebyshev vs Manhattan), then `(y, x)`.
- `pub(crate) fn tactical_strike_decoy(&mut self, cell: (i32, i32)) -> bool`
  - Refusals, all before any state moves: no fight or actor; already acted; actor not hallucinating; no opposing decoy there; beyond `swing_range`; no line of sight. Decoys never block sight.
  - On success:
    1. Push a `BoltCue`, the melee feedback rule.
    2. Remove the decoy.
    3. Log "{label}'s swing passes through a decoy."
    4. `mark_acted`.
    5. `hand_on_turn`.
- `fn settle_decoys(&mut self)`:
  1. Remove the `Hallucinating` entry, logging the wear-off line, from every body with no opposing decoy left.
  2. Then drop every decoy that no living body with a `Hallucinating` entry opposes.
- Decoys do not enter `reach::movement_field`, `line_of_sight` or `recipients`. Assert that rather than assume it.

- [ ] **Step 1: Write the failing tests** (spec 7, 8, 12).
  - `hallucination_places_its_decoys_on_the_nearest_free_cells`: exact cells on a fixed board with the aim cell occupied.
  - **(M)** `a_hallucinating_hostile_walks_toward_its_nearest_decoy`: its cell after the turn is strictly closer to the decoy and not closer to the player than a twin untampered run.
  - **(M)** `striking_a_decoy_destroys_it_and_spends_the_turn`: the hostile is adjacent to one decoy. After its turn the decoy is gone, nobody lost Integrity, and the turn moved on.
  - `a_routine_over_a_decoy_destroys_it_and_still_lands_on_real_bodies`.
  - **(M)** `with_no_decoys_left_the_entry_goes_at_once`: destroy the last decoy. The hostile's `Hallucinating` entry is gone before its `remaining` would have expired.
  - `a_companion_in_the_party_s_own_hallucination_carries_no_entry` (spec 8).
  - **(M)** `the_party_s_walk_swing_and_aim_ignore_its_own_decoys` (spec 8): an auto-driven companion with a party-owned decoy nearer than any hostile targets the hostile.
  - `decoys_block_neither_movement_nor_sight`.
  - `decoys_are_gone_when_the_fight_ends` (spec 12, the other half). `TacticalBattle` is removed. Also assert that `decoys()` empties once the last hallucinating body dies.
- [ ] **Step 2: Watch them fail.**
- [ ] **Step 3: Implement.**
- [ ] **Step 4: Run the gates** plus `tactical` and `cloak`.
- [ ] **Step 5: Commit.** Message: "Hallucination: decoys a hallucinating body targets, strikes through, and stops seeing".

---

### Task 7: The planner the turn and the forecast both call

Part A is a pure refactor that leaves the tactical suite green with no test edits.
Part B adds the forecast.

**Files:**
- Modify: `crates/engine/src/policy.rs`: extract `pub fn argmax_scored(scores: &[f32]) -> usize` from `sample_scored`'s closure. `sample_scored` calls it, which keeps the tie-breaking identical by construction.
- Modify: `crates/engine/src/tactical/ai.rs`
- Test: `tamper.rs`

**Interface (Part A: extraction, no behaviour change):**
- `fn tactical_intent(&self, actor: Entity) -> Intent`, lifted from `run_tactical_beat` :360
- `fn scored_cells(&self, actor: Entity, intent: &Intent, sides: &Sides) -> (Vec<(i32, i32)>, Vec<f32>)`: the candidate filter, `(y, x)` sort and `cell_score` from `walk_to_best_cell`
  - `walk_to_best_cell` becomes commit-empty → `scored_cells` → `sample_scored(decision_temperature)` → `path_to` → commit
- `enum TurnTarget { Body(Entity), Decoy((i32, i32)), Aim((i32, i32)) }`
- `fn chosen_target(&self, actor: Entity, from: (i32, i32), intent: &Intent, sides: &Sides) -> Option<TurnTarget>`
  - the swing selection from `swing_at_best_neighbour` and `best_aim`, both taking `from` instead of reading the actor's current cell
  - `swing_at_best_neighbour` and `run_tactical_intent` call it with the current cell and act on the result
- Ordering constraint: `walk_to_best_cell` commits `Vec::new()` **before** any early return (ai.rs:515). Keep that. Otherwise `a_spent_walk_acts_rather_than_planning_a_fresh_one` regresses into a re-plan per beat and an RNG draw per cell.

**Interface (Part B: forecast):**
- `pub(crate) struct Forecast { pub action: ForecastAction, pub walk: Vec<(i32, i32)>, pub target: Option<(i32, i32)> }`
- `pub(crate) enum ForecastAction { Swing, Routine(AbilityId) }`
- `pub(crate) fn tactical_forecast(&self, body: Entity) -> Option<Forecast>`
  - `None` unless there is a fight, the body is `Hostile` and `Profiled`, targets are non-empty, and the body is not mid-turn (Decision 4)
  - `action` always comes from `tactical_intent`
  - `walk`/`target` only when `decision_temperature(body) <= 0.0`: destination `= cells[argmax_scored(&scores)]`, `walk = reach::path_to(...)`, and `target` is the `chosen_target(from = destination)` cell
  - Otherwise both are empty/`None`.

- [ ] **Step 1 (A): Extract.** Run `cargo test -p feral-processes-engine tactical` and `cloak`. All green, with no test file edited.
- [ ] **Step 2 (A): Commit.** Message: "Extract the tactical planner: intent, scored cells and target are pure".
- [ ] **Step 3 (B): Write the failing tests.**
  - **(M)** `a_cold_profiled_hostile_does_what_its_forecast_said` (spec 3), a sweep over **24 seeded boards**:
    1. Build a local `StdRng::seed_from_u64(i)` and place 1–3 hostiles, the player and a companion on random walkable cells with `move_to`.
    2. Give a third of the hostiles `only_routine(deadlock)`, a Single Debuff that always lands, so the aim is observable as the Stun on its occupant.
    3. Tamper the subject with Cold + Profiled.
    4. `wait_for_turn(subject)`, which ends the other turns without acting, so the board doesn't move.
    5. Read `tactical_forecast(subject)`, then run `tactical_ai_turn`.
    6. Assert the final cell `== walk.last()` (or the start cell when the walk is empty).
    7. Assert the swing's `BoltCue.to == target`, or for a routine, that the body on `target` carries Stun.
    - Skip boards where targets are empty and count them. At least 18 of 24 must be compared, or the sweep proves nothing.
    - Mutation: make `tactical_forecast` use `sample_scored` at 0.5 with a cloned RNG, and watch it fail.
  - `above_zero_the_forecast_names_the_action_and_nothing_else` (spec 4): Profiled only. `action` is set, `walk` is empty, `target` is `None`.
  - `an_unprofiled_hostile_has_no_forecast`, and `a_hostile_mid_turn_has_no_forecast`.
- [ ] **Step 4 (B): Implement, then run the gates** plus `tactical`.
- [ ] **Step 5 (B): Commit.** Message: "The forecast is a call into the planner the turn runs".

---

### Task 8: The view carries tags, forecasts, hijacks and decoys

**Files:**
- Modify: `crates/engine/src/tactical/view.rs`
  - `TurnRow` (72): add `tags`, `forecast` and `taken_over`
  - `TacticalView` (83): add `decoys`
  - `turn_row` (313) and `tactical_view` (143)
  - `frozen` (110), if it copies fields explicitly
- Test: `tamper.rs`, or the view tests beside `crates/engine/src/tests/cloak.rs:796`

**Interface:**
- `pub enum TamperTag { Hot, Cold, Profiled, Injected, Hallucinating }`
  - `Temperature(t)` maps to `Cold` when `t <= tuning::TACTICAL_AI_TEMPERATURE`, else `Hot`
  - `TurnRow::tags: Vec<TamperTag>` in `TamperSlot` order
- `TurnRow::forecast: Option<ForecastView>`
  - `pub struct ForecastView { pub action: String, pub walk: Vec<(i32, i32)>, pub target: Option<(i32, i32)> }`
  - `action` is the routine's display name, or `"swing"`, resolved here in the engine
- `TurnRow::taken_over: bool`, from `Game::taken_over`
- `TacticalView::decoys: Vec<DecoyView>`, with `pub struct DecoyView { pub cell: (i32, i32), pub glyph: char, pub color: GlyphColor, pub of_player: bool }`

- [ ] **Step 1: Write the failing tests.**
  - `the_turn_row_carries_each_tamper_as_a_tag`: all five, with `Hot`/`Cold` split at the constant.
  - `a_profiled_hostile_s_row_carries_its_forecast`.
  - `a_hijacked_companion_s_row_says_so`.
  - `the_view_carries_the_decoys`.
- [ ] **Step 2: Watch them fail.** Then implement and fix every gui/app-core construction of `TurnRow`/`TacticalView` that the compiler names (test fixtures in gui included).
- [ ] **Step 3: Run the gates** plus `cargo check --workspace --all-targets`.
- [ ] **Step 4: Commit.** Message: "The tactical view carries tamper tags, forecasts, hijacks and decoys".

---

### Task 9: Drawing it

**Files:**
- Modify: `crates/gui/src/render/tactical.rs`
  - rename `CLOAKED_ALPHA` (63) to `FADED_ALPHA`, which both the cloak and decoys read. They mean the same thing: not really there.
  - `draw_tactical_map` (180): decoys drawn **before** bodies, the forecast walk and target mark drawn after bodies
  - `draw_turn_strip` (506): the tamper block beneath the strip, and the `HIJACK` glyph hue
- Possibly modify: `crates/gui/src/fx.rs` (`draw_bolts` at 943) if the forecast line reuses its line-plus-head drawing. **Call** it with a palette colour rather than copying its geometry. If its signature can't take a `Color`, extract the geometry into a function both call.
- Test: in-file tests beside `the_turn_strip_names_the_round` (1172) and `a_cloaked_body_draws_faded_and_an_uncloaked_one_does_not` (816)

**Rules:**
- Draw only through `Painter`. `render/` names no backend.
- Take the origin from the pane `Rect` and `strip_inset`, never a literal `0.0`.
- Decoy hue: `palette::glyph(color)`, or `palette::PLAYER` when `of_player`, times `FADED_ALPHA`, drawn with `painter.map`, never `sprite`.
- Forecast walk and target: `palette::PLAN`. Never `THREAT`, and not `AIM`, which is the player's cursor.
- Tag text is an exhaustive `match` on `TamperTag` → `"HOT" | "COLD" | "PROF" | "INJ" | "HALL"`, then `"HIJACK"` for `taken_over`, then `"▸ {action}"`.
  - Before using `▸`, check it has a glyph in DejaVu Sans Mono. The UI font is DejaVu and the map font is unscii; memory says this was got backwards twice.
- The block's lines take `palette::LABEL` and the strip's small metrics. Its height is bounded by the initiative length and it does not scroll.

- [ ] **Step 1: Write the failing tests.**
  - **(M)** `a_decoy_draws_the_invokers_glyph_faded`: use `paint::painted_map_glyphs` and check the alpha.
  - `the_forecast_draws_its_walk_and_marks_its_target`: count PLAN-coloured line shapes. There are none when `walk` is empty.
  - **(M)** `the_widest_tamper_line_fits_the_map_pane` (spec 16):
    - With `ui_metrics(900.0)` and `layout::regions` at 1280×720 (pattern: `render/field.rs:603`), measure a line with all five tags, `HIJACK`, and `▸` plus the **longest shipped routine display name**, read from the real `AbilityDb` rather than a literal.
    - Assert it is ≤ the map pane width − 2×`strip_inset`.
    - Mutation: add a sixth long tag and watch it fail.
  - `a_hijacked_companion_s_rung_is_drawn_in_warn`.
- [ ] **Step 2: Watch them fail.** Run `cargo test -p feral-processes-gui tactical` (check the package name in `crates/gui/Cargo.toml`).
- [ ] **Step 3: Implement, then run the gates.**
- [ ] **Step 4: Commit.** Message: "Draw decoys, tamper tags, forecasts and hijacked companions on the battle map".

---

### Task 10: Content: flavour routines, gear, and Model Inspection

**Files:**
- Create: `assets/abilities/{gradient_descent,backprop,dropout,dropout_group,fine_tune,data_poisoning}.ron`, per Decision 8. Copy `accuracy:` from the named neighbour file, and describe each in the same voice as `deadlock.ron`/`segfault_v1.ron`.
- Create: `assets/items/adversarial_patch.ron`
  - `equipment: Some((Module, (evasion: 3)))`, `value: Some(25)`, no `craftable`, no `droppable`
  - description: "Researched module. A pattern that makes a hostile's classifier look past you."
- Create: `assets/items/attention_head.ron`
  - `equipment: Some((Module, (accuracy: 3)))`, `value: Some(25)`
  - description: "Researched module. Holds a strike's focus where the model's was."
- Create: `assets/research/model_inspection.ron`
  - `name: "Model Inspection"`, `cost: 160`, `materials: [("trace_sniffer", 8), ("logic_wafer", 14)]`, `min_zone: 3`, `requires: ["cortex"]`
  - `unlocks_abilities:` all eleven ids
  - `unlocks_recipes:` the two items, each `cost: [("portal_fragment", 12), ("cache_grain", 3)]`, `requires_structure: Some("fabricator")`. This is `cortex_hack`'s bill, which already clears the price census at `value: 25`.
- Modify: `crates/engine/src/tests/assets.rs`
  - `every_zone_gated_gear_recipe_asks_for_a_zone_material`: `checked` 10 → 12, and update its message to "eight of gear"
  - any count census the full suite names (research node count, research graph layout, gear counts). Update the number *and* its prose.
- Modify: `assets/abilities/README.md`
  - a `Tamper` entry in the Schema effect list (after `Summon`, ~206): the four kinds, absolute temperature, no roll, no scaling, battle maps only, the player never a recipient, and duration counted in the tampered body's own turns
  - fix "Exactly one of ten" at 78 to the real count
  - under § Naming, one paragraph on the tactical-only contiguity exemption
- Modify: `docs/research-gen.py`: hand-transcribe the new node. It is a transcription, not a parser, so read how `cortex` is entered and match it. `docs/abilities-gen.py` is already missing Cloak and Summon; leave it and say so in the commit body.
- Test: `tamper.rs`

- [ ] **Step 1: Write the failing test.** `model_inspection_teaches_every_tamper_kind_and_both_gear_recipes` (spec 15): each of the four `TamperSlot`s is authored by a routine the node unlocks, and both item ids are recipe results.
- [ ] **Step 2: Watch it fail.** Author the files. Run `cargo test -p feral-processes-engine assets` and `tamper`.
- [ ] **Step 3: Run `balance_sim`.** `cargo test -p feral-processes-engine balance_sim` must be unmoved. It models no abilities; if a curve moved, something other than content changed, so stop and report.
- [ ] **Step 4: Commit.** Message: "Model Inspection: eleven routines and two modules".

---

### Task 11: Confirm, document, record

- [ ] **Step 1: Run the full gate.** Run `cargo test --workspace` and `cargo clippy --workspace --all-targets`. Report pass/fail counts from the actual output.
  - Known unrelated flakes: app-core `tests::creation`, and the posted-worker level-up test. Re-run the module once and name them if they appear. Anything else is yours.
- [ ] **Step 2: Grep for claims this falsifies.**
  - `rg -n "gate is .Hostile|TACTICAL_AI_TEMPERATURE" crates/` (docs still naming the constant as the thing read)
  - `rg -n "Nothing shipped authors a .shape"`
  - the README effect count
- [ ] **Step 3: Seams.** Invoke the `seams` skill and follow its three-write order (argument to the graph, trap to the skill reference, one-sentence rule to CLAUDE.md under § Tactical battles) for:
  - **`Game::decision_temperature` is the one door every tactical AI entry point reads**, and `tactical_ai_turn_at` is a test hook, not a fifth door.
  - **A tamper ages on the tampered body's own hand-on, never in `tick_one_combatant`.** A one-turn entry run on a body that has already acted would otherwise expire unseen.
  - **A profiled hostile's forecast is a call into the planner its turn runs**: `tactical_intent`, `scored_cells` and `chosen_target`, with `argmax_scored` shared with `sample_scored`.
  - **`AbilityEffect::tactical_only` is the group model's filter, applied at every chooser that is not a battle map**, and `use_ability`'s `Tamper` arm is `unreachable!` because of it.
- [ ] **Step 4: Record the spec.** Move the spec to `docs/superpowers/archive/specs/`, fix its `**Status:**` header, and add the row to `docs/superpowers/INDEX.md`. Update the plan link at the top of this file.
- [ ] **Step 5: Commit.** Message: "Tamper routines: seams, index, docs".
- [ ] **Step 6: Say plainly** that nothing here has been seen on a screen. The strip-row forecast and the tamper block are the first things to check at the keyboard (`FERAL_DEV_ARENA=1 cargo run`, or a `dev-saves/` template at zone 3 with Model Inspection researched).

## Notes for the executor

- **Order matters.** Tasks 4–6 each edit `tactical_sides`, and Task 7 moves that code. Do them in order and don't parallelise 4–7.
- **`best_aim` is not actor-relative today** (ai.rs:593 reads `Hostile`). Task 4 is where it stops being so. If any existing cloak or tactical test depended on the old spelling, that test found a real behaviour change, so read it before editing it.
- **Every refusal lands before anything is spent.** That applies to `tactical_use_routine`'s player refusal and to `tactical_strike_decoy`. Assert each refusal separately, since a single test over one path passes against the others.
- **Don't reach for `World::get_entity`.** "This entity is gone" is `world.get::<Stats>(e).is_none()`.
- **A seeded tactical test that goes red after Task 6 or 7** may be an RNG-stream shift rather than a bug. Memory has two entries on seed-luck tests. Probe for the cause before theorising.
