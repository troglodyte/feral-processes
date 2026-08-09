# Learned Enemy Battle Policy — Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Replace the two uniform rolls that pick a wild program's move and target with a policy trained offline against the arena harness and shipped as a frozen weight asset.

**Architecture:** Each candidate `(move, target)` pair is scored by one shared weight vector over ~19 normalised features, added to `ln(slot_aggro_weight(..))` as a fixed prior, then sampled through a temperature softmax on `GameRng`. Training is the cross-entropy method run over `arena::run`, in a new launcher bin. No new dependencies in any crate.

**Tech Stack:** Rust 2024, `bevy_ecs` 0.19, `ron`, `rand` — all already in the engine. Nothing added.

**Spec:** `docs/superpowers/specs/2026-08-09-enemy-battle-policy-design.md`

## Global Constraints

- **No new dependencies in any crate.** The engine depends on `bevy_ecs` alone and `cargo check --workspace` is ~1s. Keep it that way.
- **No save-format change.** Weights are an asset, not run state. `save::SAVE_FORMAT_VERSION` is not touched.
- **A malformed `.ron` is skipped with a logged warning, never a panic.** Follow `PerkDb::load_dir` (`crates/engine/src/perks.rs:161`).
- **Determinism.** Every roll goes through `GameRng`. No wall clock, no unseeded RNG, no `HashMap` iteration feeding a decision.
- **`battle::compute_damage` and `battle::slot_aggro_weight` are called, never reimplemented.** A doc comment claiming to mirror other code must be a call.
- **The fallback is today's behaviour.** No weights file, a malformed one, or an all-zero one must leave the game playing exactly as it does now.
- **Player-facing text says "sweep", not "raid".** Not expected to come up here, but it is a standing rule.
- **Run `cargo fmt` and `cargo clippy --workspace` after every task.** Fix warnings rather than silencing them.
- **`cargo test --workspace` is the final gate**, not the tests you wrote.

## Feature names (the contract every task shares)

These strings are the weight-file keys and the `Feature` enum's names. They must match exactly across Tasks 1, 3, 5 and 7.

| Group | Names |
|---|---|
| Move | `move_power_rel`, `move_ranged`, `move_has_effect`, `move_effect_stun`, `move_effect_bleed`, `move_effect_chance` |
| Target | `target_hp_frac`, `target_is_player`, `target_def_rel`, `target_stunned`, `target_bleeding`, `target_bracing`, `est_damage_frac`, `would_kill` |
| Attacker | `self_hp_frac`, `self_front_group` |
| Interaction | `effect_x_target_healthy`, `stun_x_not_stunned`, `bleed_x_not_bleeding` |

Nineteen features. No bias term — a constant cancels under a softmax over actions.

---

## Task 1: The policy math

Pure arithmetic with no `World` and no `Game`, so it is unit-testable on its own and the later tasks can trust it.

**Files:**
- Create: `crates/engine/src/policy.rs`
- Modify: `crates/engine/src/lib.rs` (add `pub mod policy;`)

**Interfaces — Produces:**
```rust
pub const FEATURE_COUNT: usize = 19;

/// Fixed order. `as usize` is the index into a `Features` array.
pub enum Feature { MovePowerRel, MoveRanged, /* ...19 total... */ }
impl Feature {
    pub fn name(self) -> &'static str;
    pub fn from_name(s: &str) -> Option<Self>;
    pub const ALL: [Feature; FEATURE_COUNT];
}

pub struct Features([f32; FEATURE_COUNT]);
impl Features {
    pub fn zeroed() -> Self;
    pub fn set(&mut self, f: Feature, v: f32);
}

#[derive(Clone, Default)]
pub struct PolicyWeights([f32; FEATURE_COUNT]);
impl PolicyWeights {
    /// `Err` on a non-finite weight — rejected at load, never per-round.
    pub fn from_pairs(pairs: &[(String, f32)]) -> Result<(Self, Vec<String>), String>;
    pub fn to_pairs(&self) -> Vec<(String, f32)>;
    pub fn score(&self, f: &Features) -> f32;
}

/// Index into `scores`, sampled at `temperature`. Subtracts the max before
/// `exp`. A temperature at or below 0 is argmax.
pub fn sample_scored<R: rand::Rng>(scores: &[f32], temperature: f32, rng: &mut R) -> usize;
```

**Tests** (in `policy.rs`'s own `mod tests`) — each asserts behaviour, not structure:

| Test | Asserts |
|---|---|
| `every_feature_name_round_trips` | `from_name(f.name()) == Some(f)` for all of `ALL`, and no two names collide. This is what stops a typo in Task 3 silently zeroing a feature. |
| `an_unknown_feature_name_is_warned_and_ignored` | `from_pairs` returns the other weights and one warning naming the unknown key. |
| `an_absent_feature_name_defaults_to_zero` | A file naming three features leaves the other sixteen at 0.0. |
| `a_non_finite_weight_is_rejected` | NaN and infinity each give `Err`. |
| `equal_scores_sample_uniformly` | Over many draws from a seeded RNG, three equal scores each land within a tolerance of a third. Use a fixed seed and a wide tolerance — this must not be flaky. |
| `a_large_score_does_not_overflow` | Scores of `[1e30, 0.0]` return index 0 rather than NaN. The max-subtraction. |
| `a_high_temperature_approaches_uniform` | Distinctly unequal scores at a high temperature sample close to uniform. |
| `a_zero_temperature_is_argmax` | Always the highest-scoring index. |

**Steps:**

- [ ] Write the eight tests above. Run them; expect failures for undefined items.
- [ ] Implement `Feature`, `Features`, `PolicyWeights`, `sample_scored`.
- [ ] `cargo test -p feral-processes-engine policy` — expect pass.
- [ ] `cargo fmt && cargo clippy --workspace`.
- [ ] Commit: `feat(policy): feature vector, weight scoring and temperature sampling`

---

## Task 2: Loading the weight file

Wires an optional asset into `Game` without changing any behaviour. Nothing ships in `assets/policies/` in this task, which is deliberate: the suite must stay green here so that any churn in Task 7 is provably caused by the weights and not the wiring.

**Files:**
- Modify: `crates/engine/src/policy.rs` (add `load_file`)
- Modify: `crates/engine/src/game/lifecycle.rs` (`AssetDbs` ~line 1009, `load_asset_dbs` ~line 1027, and both `Game::new` and `Game::load` resource insertion)
- Modify: `crates/engine/src/resources.rs` (the `EnemyPolicy` resource)
- Create: `assets/policies/README.md`

**Interfaces — Consumes:** `PolicyWeights` from Task 1.
**Interfaces — Produces:**
```rust
// policy.rs
/// A missing file is `Ok((None, vec![]))` — a valid state, not a warning.
/// A malformed one is `Ok((None, vec![warning]))`, matching PerkDb::load_dir.
pub fn load_file(path: &Path) -> std::io::Result<(Option<PolicyWeights>, Vec<String>)>;

// resources.rs
#[derive(Resource, Default)]
pub struct EnemyPolicy(pub Option<PolicyWeights>);
```

The file shape — this is the one part worth spelling out, because name-keying rather than position-keying is the whole point:

```ron
(
    features: [
        ("target_hp_frac", -1.84),
        ("would_kill",      2.31),
    ],
)
```

**Note on `load_dir` vs `load_file`.** Every other DB takes a directory and `std::fs::read_dir(dir)?` errors when it is absent. A policy is a singleton, so this reads one file at `assets_dir.join("policies/enemy_battle.ron")` and treats absence as success. Do not copy `read_dir` here.

**Tests** (`crates/engine/src/tests/` — follow the existing module layout):

| Test | Asserts |
|---|---|
| `an_absent_policy_file_loads_as_none_without_warning` | `load_file` on a nonexistent path gives `(None, [])`. |
| `a_malformed_policy_file_is_skipped_with_a_warning` | Garbage RON gives `(None, [one warning])` and no panic. |
| `a_game_starts_with_no_policy_file` | `Game::new` against the real `test_assets_dir()` succeeds and `EnemyPolicy.0.is_none()`. |

**Steps:**

- [ ] Write the three tests. Run; expect failure.
- [ ] Implement `load_file`; add `EnemyPolicy` to `resources.rs`.
- [ ] Add `policy` to `AssetDbs`, load it in `load_asset_dbs`, insert `EnemyPolicy` as a resource in both `Game::new` and `Game::load`. Both doors, per that function's own doc comment.
- [ ] Write `assets/policies/README.md`: the file shape, every one of the nineteen feature names with a one-line meaning, the name-keying rule, and that an absent file means the built-in uniform behaviour.
- [ ] `cargo test --workspace` — expect **fully green**. Any failure here is a wiring bug, not expected churn.
- [ ] `cargo fmt && cargo clippy --workspace`.
- [ ] Commit: `feat(policy): load an optional enemy policy asset`

---

## Task 3: Feature extraction and the one decision seam

The core task. Read the spec's "The scoring model" section before starting.

**Files:**
- Create: `crates/engine/src/game/combat_policy.rs`
- Modify: `crates/engine/src/game/mod.rs` (declare the module)
- Modify: `crates/engine/src/game/combat_enemy.rs:128-172` (replace the two inline rolls)
- Modify: `crates/engine/src/tuning.rs` (add `ENEMY_POLICY_TEMPERATURE`)

**Interfaces — Consumes:** `PolicyWeights`, `Features`, `Feature`, `sample_scored`, `EnemyPolicy`.
**Interfaces — Produces:**
```rust
impl Game {
    /// The one place a wild program's swing is decided. `None` means nothing
    /// reaches — the caller keeps its existing "circles beyond reach" line.
    pub(crate) fn choose_wild_action(
        &mut self,
        wild: Entity,
        group: usize,
        player: Entity,
    ) -> Option<(MoveDef, Entity)>;
}
```

**The scoring formula.** Spelled out because getting the prior wrong is invisible until it is far too late:

```
score(move, target) = (slot_aggro_weight(slot, bracing) as f32).ln()
                    + weights.score(&features(move, target, wild))
```

`exp(ln 3) == 3`, so an all-zero weight vector recovers today's exact distribution: uniform move choice crossed with slot-weighted targeting. Without the prior, all-zero weights would make every target equally likely — with a player and four companions the front three would drop from 27% to 20% and the back two rise from 9% to 20%, a real change caused by wiring alone.

**Feature definitions.** Every value normalised to roughly `[0, 1]` or `[-1, 1]` so the trainer's Gaussian is well-scaled:

- `move_power_rel` — this move's power over the largest power in *this species'* moveset (relative, so it holds for any modded roster; guard a zero denominator)
- `move_ranged`, `move_has_effect`, `move_effect_stun`, `move_effect_bleed` — 0.0 or 1.0
- `move_effect_chance` — the effect's own `.ron` `chance`, or 0.0
- `target_hp_frac` — `Stats::hp_fraction()`
- `target_is_player` — 0.0 or 1.0
- `target_def_rel` — `effective_def(target)` over `wild`'s `atk`, clamped to `[0, 2]` then halved into `[0, 1]`; guard a zero denominator
- `target_stunned`, `target_bleeding` — from the target's `Statuses`
- `target_bracing` — `is_defending(target)`
- `est_damage_frac` — `battle::compute_damage(wild_atk, effective_def(target), move.power)` over the target's current `hp`, clamped to 1.0. **Call `compute_damage`; do not reimplement it.**
- `would_kill` — 1.0 when that damage is at least the target's current `hp`
- `self_hp_frac` — the attacker's own `hp_fraction()`
- `self_front_group` — `group < ENGAGED_GROUPS`
- `effect_x_target_healthy` — `move_has_effect * target_hp_frac`
- `stun_x_not_stunned` — `move_effect_stun * (1.0 - target_stunned)`
- `bleed_x_not_bleeding` — `move_effect_bleed * (1.0 - target_bleeding)`

**Candidate enumeration** mirrors what `wild_retaliate` does today: moves filtered by `engaged || m.ranged`, crossed with the player and every living party member. Empty means `None`.

**Borrow scoping.** `GameRng` is a resource and the feature pass needs `&self` reads of `Stats`, `Statuses` and `SpeciesDb`. Build every score first, drop those borrows, then take the RNG in its own block — the same shape `wild_retaliate` already uses:

```rust
let idx = {
    let mut rng = self.world.resource_mut::<GameRng>();
    policy::sample_scored(&scores, ENEMY_POLICY_TEMPERATURE, &mut rng.0)
};
```

**`roll_enemy_target` stays.** The routine branch still calls it and so does the no-policy fallback. Do not delete or inline it.

**Tests:**

| Test | Asserts |
|---|---|
| `an_all_zero_policy_reproduces_the_uniform_baseline` | Two `Game`s at one seed — one with `EnemyPolicy(None)`, one with all-zero weights — produce an identical battle transcript. **This is an equivalence guard, not a fix-removal test:** it passes trivially with the policy absent, and the spec records that so nobody later reads it as coverage. |
| `a_policy_that_prefers_the_wounded_focus_fires` | Hand-authored weights with a strongly negative `target_hp_frac`. Over many seeded rounds against one wounded and one healthy companion, the wounded one is hit substantially more often than the aggro prior alone would give. Asserts the *outcome*, never that a function was called. |
| `a_policy_that_prefers_a_kill_takes_it` | Strongly positive `would_kill`; a target one hit from death is chosen over a healthy one in a higher-aggro slot. Proves the learned term can overcome the prior. |
| `a_high_temperature_approaches_the_uniform_baseline` | Non-trivial weights at a high temperature give a target distribution close to the no-policy one. |
| `the_same_seed_and_weights_replay_the_same_fight` | Two identical runs, identical transcripts. |
| `a_back_group_still_only_uses_ranged_moves` | The `ENGAGED_GROUPS` filter survives the rewrite — a group past the front with no ranged move still logs "circles beyond reach". |

**Steps:**

- [ ] Add `ENEMY_POLICY_TEMPERATURE: f32 = 1.0` to `tuning.rs` in a labelled section, documenting that 0 is argmax and a high value returns the uniform baseline, and that it is the only shipping control because `balance_sim` is blind to this change.
- [ ] Write all six tests. Run; expect failure.
- [ ] Implement `combat_policy.rs`: feature extraction, then `choose_wild_action` with the no-policy fallback returning today's rolls.
- [ ] Rewrite `wild_retaliate`'s move-and-target section to call `choose_wild_action`. Leave the routine branch, the `WILD_ABILITY_CHANCE` effect gate, the damage application and every log line exactly as they are.
- [ ] `cargo test --workspace` — expect **fully green**. No weights ship yet, so nothing may move.
- [ ] `cargo fmt && cargo clippy --workspace`.
- [ ] Commit: `feat(policy): score wild moves and targets through one seam`

---

## Task 4: Moddability and the Defend guard

**Files:**
- Modify: `crates/engine/src/tests/` (whichever module Task 3's tests landed in)

**Interfaces — Consumes:** everything from Task 3.

**Tests:**

| Test | Asserts |
|---|---|
| `a_modded_species_with_more_moves_still_scores` | A species `.ron` written into a scratch assets dir with seven moves — more than any shipped species — is scored and acts without panicking. Walks the moddability claim rather than assuming it. Use the existing scratch-assets fixture pattern; see `crates/engine/src/tests/support.rs` and note that scratch installs must clean themselves up. |
| `a_modded_species_with_one_move_scores` | The degenerate end: a single-move species still picks a target. Guards the `move_power_rel` zero-denominator. |
| `bracing_draws_more_fire_under_hand_authored_weights` | With a plausible non-zero weight set, a bracing front-slot member still draws more fire than the same member not bracing. This is the Defend guard in its testable form until real weights exist; Task 7 re-points it at the shipped file. |

**Steps:**

- [ ] Write the three tests. Run; expect failure or panic.
- [ ] Fix whatever they surface — most likely the two zero-denominator guards.
- [ ] `cargo test -p feral-processes-engine policy` then `cargo test --workspace`.
- [ ] `cargo fmt && cargo clippy --workspace`.
- [ ] Commit: `test(policy): modded rosters and the Defend guard`

---

## Task 5: The CEM optimiser

Pure optimisation, no game involved, so it is tested against a toy objective and cannot be wrong in a way the game hides.

**Files:**
- Create: `crates/launcher/src/cem.rs`
- Modify: `crates/launcher/src/lib.rs` (add `pub mod cem;`)
- Modify: `crates/launcher/Cargo.toml` — its comment currently says this crate "has no tests". Check whether `dev_template` already has some; correct the comment either way rather than leaving it stale.

**Interfaces — Produces:**
```rust
pub struct CemConfig {
    pub dims: usize,
    pub population: usize,
    pub elite_fraction: f32,
    pub iterations: usize,
    pub initial_std: f32,
    /// Added to the refitted variance each generation so the search cannot
    /// collapse to a point and stop exploring.
    pub std_floor: f32,
}

pub struct CemProgress { pub iteration: usize, pub best_fitness: f32, pub mean_fitness: f32 }

/// `fitness` is called once per candidate per generation. Higher is better.
/// `rng` makes the whole run reproducible from one seed.
pub fn optimise<F, R>(cfg: &CemConfig, rng: &mut R, fitness: F, on_progress: impl FnMut(CemProgress)) -> Vec<f32>
where F: Fn(&[f32]) -> f32, R: rand::Rng;
```

The update rule, since it is the whole algorithm: sample `population` vectors from `N(mean, std²)` per dimension, evaluate, sort by fitness, take the top `elite_fraction`, set `mean` and `std` to that elite set's per-dimension mean and standard deviation, add `std_floor` to `std`, repeat.

**Tests:**

| Test | Asserts |
|---|---|
| `cem_converges_on_a_quadratic` | Maximising `-(x-3)² - (y+1)²` lands within a small tolerance of `(3, -1)`. Proves the optimiser with no game in sight. |
| `the_same_seed_reproduces_a_run` | Two runs at one seed give an identical result vector. |
| `the_std_floor_keeps_the_search_alive` | With `std_floor` at 0 on a flat objective the spread collapses; with a floor it does not. This is the failure mode that makes a CEM run silently stop learning at generation 4. |
| `progress_is_reported_once_per_iteration` | The callback fires `iterations` times with ascending indices. |

**Steps:**

- [ ] Write the four tests. Run; expect failure.
- [ ] Implement `optimise`.
- [ ] `cargo test -p feral-processes cem`.
- [ ] `cargo fmt && cargo clippy --workspace`.
- [ ] Commit: `feat(train): cross-entropy method optimiser`

---

## Task 6: The trainer binary, and the throughput gate

**Files:**
- Create: `crates/launcher/src/bin/train.rs`
- Create: `dev-training/*.ron` (the scenario set) and `dev-training/README.md`
- Modify: `crates/launcher/Cargo.toml` (a third `[[bin]]`)
- Modify: `crates/engine/src/policy.rs` — a `write_file` counterpart to `load_file`

**Interfaces — Consumes:** `optimise`/`CemConfig` from Task 5; `arena::run`, `Scenario`, `Summary` from the engine; `PolicyWeights::from_pairs`/`to_pairs`.
**Interfaces — Produces:**
```rust
// policy.rs
pub fn write_file(path: &Path, w: &PolicyWeights) -> std::io::Result<()>;
```

**CLI:**
```
train --out <path> --scenarios <dir> [--iters 30] [--pop 40] [--reps 200]
      [--seed 1] [--report <path>]
```

**Fitness** for one candidate: install its weights, run every scenario in the set at a fixed per-generation seed offset, and score `enemy_win_rate + TIEBREAK * (1.0 - mean_player_hp_fraction)` with a small `TIEBREAK` (0.1 is a sane start). `Summary::win_rate` is the *player's*, so the enemy's is `1.0 - win_rate`. The tiebreak exists because at low win rates a pure win/loss signal has no gradient at all for the first few generations.

**Seeding:** every candidate within a generation runs the *same* seeds, so a comparison is signal rather than luck; the seed set is re-rolled between generations so the result does not overfit to one particular set of fights. Both halves matter.

**Installing candidate weights:** `arena::run` builds its own `Game` internally and there is no injection point. Resolve this by writing the candidate to a temporary file inside the scenario's assets dir before each evaluation. If that proves too slow, the alternative is a scoped engine change to let `stage` take pre-loaded weights — but do not build that until the measurement below says it is needed.

### The throughput gate — do this step first

- [ ] Time `arena::run` on `dev-arenas/opening-fight.ron` at `reps: 200`, release build. Record fights per second.
- [ ] Multiply out the default budget: `iters × pop × reps × scenarios`. At 30 × 40 × 200 that is 240,000 fights *per scenario*.
- [ ] **Decision gate.** `arena::stage` calls `setup::build_player` → `Game::new` → `load_asset_dbs`, which reads and parses roughly a hundred RON files **per fight**. If measured throughput puts a full run past about twenty minutes, do not proceed to a big run. Report the number and the options — smaller budget, fewer scenarios, or a scoped engine change to hoist asset loading — and let the human choose. Do not silently shrink the budget; a silent cap reads as "we trained properly" when we did not.

**Scenario set:** six to ten scenarios in `dev-training/`, spanning player levels 1/6/12/20, zones 1/3/5, Stack depths including 0, party sizes 0/2/4, and both authored `opponents` and rolled `encounter` scenarios. `dev-arenas/`'s three stay the held-back test set and are never trained on. `dev-training/README.md` says what each scenario is for and states that rule.

**Tests:** the binary itself is thin glue and gets no unit tests; `optimise` is covered in Task 5. Add one:

| Test | Asserts |
|---|---|
| `written_weights_load_back_identically` | `write_file` then `load_file` round-trips a weight vector. In `policy.rs`. |

**Steps:**

- [ ] Run the throughput gate above and report the number before writing anything else.
- [ ] Write and run the round-trip test; implement `write_file`.
- [ ] Write the scenario set and its README.
- [ ] Write `train.rs`: argument parsing, scenario loading, the fitness closure, the `optimise` call, progress printing per generation, and the final report.
- [ ] Run a **short** run (`--iters 3 --pop 8 --reps 20`) to prove the loop end to end. Do not train for real yet.
- [ ] `cargo fmt && cargo clippy --workspace`.
- [ ] Commit: `feat(train): CEM trainer over the arena harness`

---

## Task 7: Train, ship, triage

The only task that changes how the game plays. Expect the suite to move here.

**Files:**
- Create: `assets/policies/enemy_battle.ron` (the trained weights)
- Create: `docs/superpowers/reports/2026-08-09-enemy-policy-training.md`
- Modify: the Defend guard from Task 4, re-pointed at the shipped weights
- Modify: `CHANGELOG.md`, `CLAUDE.md`, `assets/policies/README.md`
- Modify: `TODO.md` (tick `[ ] machine learning for enemies`)

**Steps:**

- [ ] Run the real training at the budget the Task 6 gate settled on. Keep the report.
- [ ] Evaluate on the **held-back** `dev-arenas/` three: enemy win rate at all-zero weights versus at the trained weights. This number is the proof of concept's actual result.
- [ ] Write the training report: baseline versus trained win rate on both the training and held-back sets, per-scenario breakdown, and the highest-magnitude weights **by name** with a plain-English reading of each. Say explicitly whether the policy found anything a person would not have hand-written — a boring answer is an acceptable outcome and must be reported as one, not dressed up.
- [ ] Install the weights at `assets/policies/enemy_battle.ron`.
- [ ] `cargo test --workspace`. **Expect churn.** `test_assets_dir()` is the real `assets/`, so every seeded test containing a wild attack may now pick a different move or target.
- [ ] Triage each failure individually into one of two buckets, and write which in the commit message: **(a)** the test pinned an RNG accident and its assertion should be loosened to what it actually means; **(b)** the policy broke something real. Do not blanket-reseed to green — that is how a real regression ships.
- [ ] Re-point `bracing_draws_more_fire_under_hand_authored_weights` at the shipped file and rename it `bracing_still_draws_more_fire_under_the_shipped_weights`. This is the Defend census: a future retrain that cancels the aggro prior fails the suite instead of quietly devaluing a designed player action. `balance_sim` cannot cover it — its own docs say it passes `defending: false` throughout.
- [ ] `cargo test -p feral-processes-engine balance_sim` — expect **no movement**. It is RNG-free and models no abilities, so its curves should be untouched. If one moves, something other than the policy changed.
- [ ] Update `assets/policies/README.md` with the shipped weights as a worked example.
- [ ] Add a `CLAUDE.md` load-bearing-seam entry: `choose_wild_action` is the one place a wild program's swing is decided; the `ln(slot_aggro_weight(..))` prior is what makes an all-zero policy identical to today; `balance_sim` is blind to this and `ENEMY_POLICY_TEMPERATURE` is the only control. Then `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no tracking to catch drift.
- [ ] `CHANGELOG.md` section. `docs/manual.md` and the root `README.md` are carved out and stay stale.
- [ ] `cargo fmt && cargo clippy --workspace` and a final `cargo test --workspace`.
- [ ] Commit: `feat(policy): ship trained enemy battle weights`

---

## Not done by this plan

- Routine selection and timing, and the `WILD_ABILITY_CHANCE` effect gate
- Overworld `wander_ai_system` and `pursuit_field`
- Party-side or companion AI
- Training against anything other than All-Attack with no companion Specials
- The version bump, changelog section and tag, which happen at the merge to `main`, not on the branch

## Playtest before calling it done

A green suite is not evidence of play, and how a learned enemy *feels* is not measurable headlessly. Before this is finished:

```sh
FERAL_DEV_ARENA=1 cargo run    # main menu, [R] Arena
```

Play the same authored scenario with the weights file present and moved aside. The questions are whether fights read as smarter or merely as harsher, whether Defend still feels worth a round, and whether losing feels earned. Report what you actually played, not what the win rate implies.
