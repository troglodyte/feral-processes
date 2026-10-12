# Tactical Enemy Personalities Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Hostiles on the tactical map choose swing/routine targets by scored features weighted per species personality, walk by per-profile cell weights, and each personality is trained to today's difficulty.

**Architecture:** A pure scoring module (`tactical/policy.rs`) replaces the hostile branch of `tactical_intent`'s fixed rules and `best_swing`'s lowest-HP rule with `argmax(weights · features)`. Profiles are data (`assets/behaviours/`, `BehaviourDb`), looked up via `SpeciesDef.behaviour`. The arena gains an auto-play party and a profile override; `train` gains a tactical mode that tunes free weights toward a recorded baseline using the existing CEM in `crates/launcher/src/cem.rs`.

**Tech Stack:** Rust, standalone `bevy_ecs`, serde + RON assets.

**Spec:** `docs/superpowers/specs/2026-10-12-tactical-enemy-personalities-design.md` — read it first; this plan argues from it. Per project rule, the plan carries files, interfaces, test intent and gates, not finished code.

## Global Constraints

- Tactical map only. Group battles, `combat_policy.rs`, `assets/policies/enemy_battle.ron`, party AI and `[A]`/`[R]` behaviour are untouched.
- Decision is deterministic argmax, no RNG draw; the only AI draw per turn stays the cell softmax. `decision_temperature` stays the one temperature door.
- `default` profile reproduces today exactly: existing tactical tests pass **unedited**, except adding `CellWeights::default()` to direct `cell_merit`/`cell_score` calls.
- New schema fields are `#[serde(default)]`; a malformed `.ron` is skipped with a logged warning (`*Db::load_dir` pattern); `default.ron` must exist.
- Personality is per species, never derived from class (seams-species).
- No save-format change. Release is a patch bump on `0.x` (deploy skill, not this plan).
- Read `.claude/rules/seams-tactical.md`, `seams-combat.md`, `seams-species.md`, `content-schema.md` before editing their files.
- Never `git checkout`/`stash` over uncommitted work; stage explicit paths; never push.
- Unset `FERAL_DEV_NO_SIEGES` before `cargo test`.
- A new `Resource` can shift query iteration order and RNG-seeded tests: if seed-luck tests move, read the code path; never retry until green.

## Review Focus

1. A modded species naming a profile that does not exist → plays as `default`, one warning, no panic (Task 2 test).
2. A profile with every weight zero or missing → ties resolve by the sorted candidate order (reading order), never a panic or a skipped turn (Task 3 test).
3. A hallucinating hostile seeing a decoy → `best_swing`'s decoy door still wins; the profile only ranks real targets (Task 3 test).
4. A 2x2 squad hostile under a non-default profile → walks by whole-footprint cell scores like today, just reweighted (Task 4 test).
5. An Injected body (`acts_for_hostiles` flipped) → keeps its species profile but targets the side it now acts for (Task 3 test).

---

### Task 1: `CellWeights` — move the walk constants behind a struct

**Files:**
- Modify: `crates/engine/src/tactical/ai.rs` (`cell_score` :226, `cell_merit` :284, `scored_cells` :1128, `walk_risk` :1225, `walk_to_best_cell` :1090, unit tests :1597+)
- Modify: `crates/engine/src/tactical/view.rs` (its one `cell_*` caller)
- Create: `crates/engine/src/tactical/policy.rs` (only `CellWeights` in this task); register in `tactical/mod.rs`

**Interfaces:**
- Produces: `#[derive(Clone, Copy, Debug, PartialEq, Deserialize)] #[serde(default)] pub struct CellWeights { pub reach: f32, pub closing: f32, pub cover: f32, pub cover_tiebreak: f32, pub crowding: f32, pub reaction: f32 }` with `Default` = `TACTICAL_AI_REACH_SCORE`, `_CLOSING_WEIGHT`, `_COVER_WEIGHT`, `_COVER_TIEBREAK`, `_CROWDING_WEIGHT`, `_REACTION_WEIGHT`. `cell_score(board, cells, band, targets, allies, w: &CellWeights)`, `cell_merit(board, cells, band, targets, w: &CellWeights)`; `scored_cells`/`walk_to_best_cell` take `&CellWeights`; `walk_risk(&self, actor, to, reaction: f32)`.

- [ ] Test intent: a new unit test shows `cell_score` with doubled `crowding` ranks a crowded cell lower than with `default()` (proves the field is read). Run, see it fail to compile, implement, see it pass.
- [ ] Every current call site passes `&CellWeights::default()` (behaviour unchanged). The existing ai.rs unit tests change only by that argument.
- [ ] Gate: `cargo test -p feral-processes-engine tactical` green; `cargo clippy --workspace --all-targets` clean; `cargo fmt`.
- [ ] Commit: "Tactical AI: cell weights behind CellWeights, defaults are today's constants".

### Task 2: `BehaviourDb`, profile schema, species field

**Files:**
- Create: `crates/engine/src/behaviours.rs` (register in `lib.rs`), `assets/behaviours/default.ron`, `assets/behaviours/README.md`
- Modify: `crates/engine/src/species.rs` (`SpeciesDef.behaviour`, next to `taunts`), `assets/species/README.md`
- Modify: `crates/engine/src/game/lifecycle.rs` (the asset bundle struct near :3770 that holds `policy`, and both `insert_resource` sites :534/:1516 — `Game::new` and `Game::load` both go through it)
- Modify: `crates/engine/src/resources.rs` (`pub struct Behaviours(pub BehaviourDb)`)
- Test: `crates/engine/tests/behaviours.rs` (new), `crates/engine/tests/assets.rs` (censuses)

**Interfaces:**
- Produces:
  - `pub enum Sign { Positive, Negative }`
  - `pub struct BehaviourDef { pub id: String, pub name: String, pub description: String, pub pinned: BTreeMap<String, f32>, pub signs: BTreeMap<String, Sign>, pub weights: BTreeMap<String, f32>, pub cells: CellWeights }` (serde; `pinned`/`signs`/`weights`/`cells` default)
  - `impl BehaviourDb { pub fn load_dir(dir: &Path) -> io::Result<(Self, Vec<String>)>; pub fn get(&self, id: &str) -> Option<&BehaviourDef>; pub fn default_profile(&self) -> &BehaviourDef; pub fn ids(&self) -> impl Iterator<Item=&str>; pub fn set_override(&mut self, id: Option<String>) }`
  - `pub fn write_file(path: &Path, def: &BehaviourDef) -> io::Result<()>` (whole-file re-serialise; used by Task 7)
  - `Game::behaviour_of(&self, body: Entity) -> &BehaviourDef` — override if set, else species `behaviour`, else `default`.
  - `SpeciesDef.behaviour: Option<String>`.
- `default.ron` here holds only `id/name/description` (weights arrive in Task 3).

- [ ] Tests first (`tests/behaviours.rs`): malformed file skipped with a warning; a feature in both `pinned` and `weights` warns and `pinned` wins; missing `default.ron` is a load error; species with unknown `behaviour` → `behaviour_of` returns `default` and one warning (Review Focus 1); species with no field → `default`.
- [ ] Census in `tests/assets.rs`: `default.ron` exists; every shipped species' `behaviour` names an existing profile.
- [ ] Implement; document the schema in `assets/behaviours/README.md` (fields, pinned/signs/weights split, "comments are not preserved by the trainer") and the field in `assets/species/README.md`.
- [ ] Gate: `cargo test --workspace` (new Resource → watch for seed-luck shifts, see Global Constraints), clippy, fmt.
- [ ] Commit: "Engine: behaviour profiles as data, species name their profile".

### Task 3: scored hostile decisions

**Files:**
- Modify: `crates/engine/src/tactical/policy.rs` (features, candidates, scoring)
- Modify: `crates/engine/src/tactical/ai.rs` — hostile branch of `tactical_intent` (:841), `best_swing` (:1538) for hostile actors
- Modify: `crates/engine/src/behaviours.rs` (resolve feature names → `TacticalFeature`; unknown name = warning at load)
- Modify: `assets/behaviours/default.ron` (weights that reproduce today)
- Test: `crates/engine/tests/tactical_policy.rs` (new)

**Interfaces:**
- Consumes: `CellWeights`, `BehaviourDef`, `Game::behaviour_of`.
- Produces:
  - `pub enum TacticalFeature { TargetHpFrac, EstDamageFrac, WouldKill, TargetIsPlayer, TargetStunned, TargetDistance, IsRoutine, RoutineNetRecipients, SelfHpFrac, SelfXTargetDistance }` with `name()`/`from_name()` (names as in the spec table) and `const COUNT`.
  - `pub struct TacticalWeights([f32; TacticalFeature::COUNT])` built from a `BehaviourDef` (pinned ∪ weights).
  - `pub(crate) enum Candidate { Swing { target: TurnTarget }, Routine(Box<AbilityDef>) }`
  - `pub(crate) fn score(w: &TacticalWeights, f: &[f32; COUNT]) -> f32`
  - `Game::hostile_features(&self, actor, &Candidate) -> [f32; COUNT]`, `Game::choose_hostile_intent(&self, actor) -> Intent` (routine-vs-swing), `Game::rank_swing_targets(&self, actor, from, range, targets) -> Option<TurnTarget>` (replaces the lowest-HP pick for hostiles; decoy branch first, unchanged).
  - Picking uses `policy::argmax_scored` over candidates in sorted order.
- `default.ron`: `is_routine` large enough to beat any swing, `target_hp_frac` negative; nothing else.

- [ ] Tests first (`tests/tactical_policy.rs`):
  - all-zero profile → ties go to reading order; no panic, turn taken (Review Focus 2);
  - a profile that rewards `would_kill` swings at the killable body over a lower-HP-fraction one;
  - a profile with negative `is_routine` swings instead of firing a ready routine;
  - decoy seen → decoy struck regardless of profile (Review Focus 3);
  - Injected body keeps its species profile and targets the side it acts for (Review Focus 5);
  - purity: for each shipped profile, `tactical_forecast`'s action equals the turn taken; same seed + profile replays identically.
  - unknown feature name in a profile → load warning, ignored;
  - census in `tests/assets.rs`: every shipped profile names only known features.
- [ ] Implement. Keep `hostile_routine_worth_choosing` as a candidate filter so a routine with no aim is not a candidate except where it admits it today.
- [ ] Refactor gate: `cargo test -p feral-processes-engine --test tactical` and all other existing tactical tests pass **unedited**; mutation-check one: flip `default`'s `target_hp_frac` sign → a lowest-HP test fails.
- [ ] Gate: `cargo test --workspace`, clippy, fmt.
- [ ] Commit: "Tactical AI: hostiles choose by profile-weighted scores; default is today".

### Task 4: personalities and per-profile walking

**Files:**
- Modify: `crates/engine/src/tactical/ai.rs` — hostile walk passes `&behaviour_of(actor).cells` and its `reaction`
- Create: `assets/behaviours/{brute,hunter,skirmisher,cautious}.ron` (hand-set weights, pinned + signs per the spec's characters)
- Modify: species files per the spec mapping: brute → rootkit, botnet, worm, scrapper; hunter → cipher, zero_day, trojan, polymorph; skirmisher → glitch, sprite; cautious → drone, construct
- Test: `crates/engine/tests/tactical_policy.rs`

**Interfaces:**
- Consumes: Tasks 1–3. Party bodies keep `CellWeights::default()`.

- [ ] Tests first, each on a hand-built board with a fixed seed at temperature 0, and each mutation-checked (zero the distinguishing weight or cell term → test fails):
  - brute walks past cover to the nearest body;
  - skirmisher picks the covered cell at range;
  - hunter swings at the wounded companion over the healthy player;
  - cautious at low HP picks a cell with no adjacent threat;
  - a 2x2 squad under `skirmisher` scores whole footprints (Review Focus 4).
- [ ] Implement; profiles documented in `assets/behaviours/README.md` (one line each).
- [ ] Gate: `cargo test --workspace`; `cargo test -p feral-processes-engine balance_sim` unchanged; clippy, fmt.
- [ ] Commit: "Content: brute, hunter, skirmisher and cautious personalities on their species".

### Task 5: arena auto-play party and profile override

**Files:**
- Modify: `crates/engine/src/arena/scenario.rs` (`Scenario`: new `party_turns: PartyTurnsSpec` and `behaviour: Option<String>`, both `#[serde(default)]`; note `party` is already the companions list)
- Modify: `crates/engine/src/arena/run.rs:100` (map spec → `PartyTurns`), arena setup (call `set_override` when `behaviour` set; unknown id → scenario error, not silent — see memory `arena-scenario-fields-fail-silently`)
- Modify: `dev-arenas/README.md`
- Modify: seam rule "a headless rep … the party swings without invoking" in all three places: `.claude/rules/seams-tactical.md`, the `seams` skill, graph `seam:` entity
- Test: arena tests in the engine (where `run_tactical_rep` is tested)

**Interfaces:**
- Produces: `pub enum PartyTurnsSpec { #[default] SwingOnly, AutoPlay }`. `AutoPlay` → `PartyTurns::Invoke`. Refused for `model: Group` (scenario error).

- [ ] Tests first: `AutoPlay` rep with a party routine fires it; `SwingOnly` default rep output unchanged for an existing scenario; `behaviour: "brute"` makes every hostile resolve `brute`; unknown `behaviour` id → error; `AutoPlay` on a group scenario → error.
- [ ] Implement, docs, seam rule in three places.
- [ ] Gate: `cargo test --workspace`, clippy, fmt.
- [ ] Commit: "Arena: auto-play party and behaviour override for tactical scenarios".

### Task 6: benchmark set and baseline

**Files:**
- Create: `dev-training/tactical/*.ron` (6–8 scenarios: solo early, pair, mid group, squad, geared late, covered board, ranged-heavy opponents, one with opponent routines), all `model: Tactical`, `party_turns: AutoPlay`
- Modify: `dev-training/README.md`
- Create: `docs/measurements/<run date>-tactical-personality-baseline.md` (follow `docs/measurements/README.md`'s bar)

- [ ] Read `docs/measurements/` tactical notes first (squads, reactions-arena-blind traps).
- [ ] Each scenario loads and runs one rep (`cargo run --release --bin arena -- <file>`); fix scenario errors.
- [ ] Baseline: `default` profile, ≥200 reps each, release build; record enemy win rate, mean player HP lost fraction, mean rounds, build commit.
- [ ] Commit: "Dev-training: tactical benchmark set and default baseline".

### Task 7: tactical training mode

**Files:**
- Modify: `crates/launcher/src/bin/train.rs` (new `--tactical --profile <id>` path; group path untouched), reuse `crates/launcher/src/cem.rs::optimise`
- Create (if train.rs grows past one concern): `crates/launcher/src/tactical_train.rs` for objective + free-weight mapping
- Test: unit tests beside the new code

**Interfaces:**
- Consumes: `BehaviourDef`, `behaviours::write_file`, `Scenario.behaviour`/`party_turns`, baseline numbers (read from a `--baseline <ron>` the Task 6 run writes via `--out`).
- Produces: `fn free_features(def) -> Vec<TacticalFeature>` (not pinned); `fn weights_from_candidate(def, &[f32]) -> BehaviourDef` (sign-clamped); `fn objective(set: &SetResult, base: &SetResult) -> f32` = Σ scenarios `(hp_lost − base)²·1.0 + (enemy_win − base)²·0.5 + (rounds/base − 1)²·0.1`.

- [ ] Tests first: pinned features never change across candidates; a Negative-signed feature never goes positive; objective is 0 at baseline and grows with distance on each term.
- [ ] Implement; `--eval` prints trained vs baseline per scenario.
- [ ] Run training for brute, hunter, skirmisher, cautious; write the weights back; one measurement note per profile (or one combined note) showing HP lost and enemy win rate within ±10% relative of baseline across the set — same build as the baseline. A profile that cannot reach the band: stop and report, do not loosen the band.
- [ ] Re-run Task 4's behaviour tests on trained weights; a test that now fails means training erased the character — report, do not edit the test.
- [ ] Gate: `cargo test --workspace`, `balance_sim` unchanged, clippy, fmt.
- [ ] Commit: "Train: tactical profiles tuned to the default baseline".

### Task 8: docs and final review

**Files:**
- Modify: `docs/superpowers/INDEX.md` (entry: spec, plan, measurements)
- Graph: a `seam:` entry for "personality is per species, profile lookup is `behaviour_of`, decision is argmax with no draw", plus `seams` skill and a rules line (three writes, skill's order)

- [ ] Final whole-branch review (opus), diff given as a file.
- [ ] CHANGELOG and version bump happen at deploy, not here.

## Execution notes

Tasks 1→4 are sequential (each consumes the last's interfaces). Task 5 depends on Task 2 only and may run after it. Tasks 6→7 need 4 and 5.
