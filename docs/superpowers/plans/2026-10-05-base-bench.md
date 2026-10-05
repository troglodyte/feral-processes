# Base bench — plan

Spec: `docs/superpowers/specs/2026-10-05-base-bench-design.md`. Branch
`base-bench`. Read the spec once; this plan adds only the decisions, files
and tests it left open.

**Scope of this plan:** phase 1 (Economy) in full. Phases 2 and 3 are gated
on the user setting target ranges after a baseline, so each gets a short
section here and its own plan pass when its baseline is in.

**Execution:** P1 is four tasks, one sonnet subagent each, run serially (each
uses the last's types). Gates off per task; one opus whole-branch review at
the end of P1. Each task: TDD, commit per green step, `cargo fmt`, `cargo
clippy --workspace --all-targets`, then its listed tests. Full `cargo test
--workspace` at T1 end and T4 end only. Every dispatch: **never push**,
stage explicit paths only (`art/` is the user's untracked work — never stage
it).

## Decisions taken here (the spec's "Open" list)

- **Re-seeding: no new `Game` method.** `bench` lives in the engine, so it
  installs the seed exactly as `arena` does
  (`arena/mod.rs:318`, `insert_resource(GameRng(StdRng::seed_from_u64(seed)))`)
  right after `Game::load`. Sampling still goes only through public `Game`
  methods.
- **Telemetry rename: do it.** Callers are app-core `app/lifecycle.rs`,
  `arena/mod.rs`, `tests/telemetry.rs`, `tests/tactical.rs`. Rename the pair
  `enable_battle_telemetry`/`take_battle_telemetry` →
  `enable_telemetry`/`take_telemetry` (`game/telemetry.rs:15,23`). The
  `BattleTelemetry` resource keeps its name (renaming a Resource is churn
  with no reader).
- **Machine status ticks come from edges, not per-tick sampling.**
  `Record::MachineStall` is emitted on every transition
  (`telemetry.rs:94`). Initial status per machine from one
  `structure_report()` (`inspection.rs:1497`, `StructureReport.status`) at
  tick 0; fold edges to the end. Exact, and avoids a per-tick
  `structure_report` walk. Machine identity = base-space `pos`.
- **Units made and per-item totals fold from the drained records:**
  `Extract.landed` (when `ok`) and one per `Assemble`, keyed by `machine`
  and `item`. No `BaseLedger` accessor is added; `base_output_report` is
  capped for the screen and unfit here.
- **Per tick samples:** `line_reports()` (`lines.rs:283`, `LineReport.status`
  and the end-of-line member) and `labour_demand()` (`work_orders.rs:3037`).
  End-of-line output = units the line's last `members` entry made, from the
  records.
- **Cost of a run:** T4 times `run --template chains --ticks 5000` in
  release before writing budgets into `economy.ron`; budgets are chosen so a
  `tune` finishes in ≤ 15 min. Timing goes in the measurement doc.
- **Fitness:** new `range_error(value, min, max)` = 0 inside, else
  `((distance outside) / (max - min))²`; summed over targets × seeds,
  averaged over seeds. `cem::optimise` maximises, so fitness = `-error`. Not
  `tuner::score::target_error` (point targets); the spec's "shape of" means
  the squared, per-target-normalised form.
- **Knobs normalised:** CEM vector in [0,1], clamped, mapped linearly to
  `[min, max]`; integer fields rounded after mapping.

## P1 — Economy

### T1 — engine: `bench` core (crate: engine, app-core)

Files: new `engine/src/bench/{mod.rs,report.rs}`, `engine/src/lib.rs`
(`pub mod bench`), `game/telemetry.rs` + the four callers above (rename).

```rust
pub struct RunOptions { pub ticks: u64, pub seed: u64 }
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BenchReport { pub ticks: u64, pub seed: u64,
    pub economy: EconomyReport,
    pub staff: Option<()>, pub memories: Option<()> }   // typed in P2/P3
pub struct EconomyReport {
    pub machines: Vec<MachineReport>,      // sorted by pos
    pub lines: Vec<LineBench>,
    pub labour: LabourBench,               // mean wanted, mean staffed, mean unworked per duty
    pub items: BTreeMap<String, u64>,      // units made per item
}
pub struct MachineReport { pub pos: (i32,i32), pub kind: String,
    pub status_ticks: BTreeMap<String, u64>, pub units: u64, pub running_share: f32 }
pub struct LineBench { pub key: String, pub output_per_1000: f32,
    pub status_ticks: BTreeMap<String, u64> }
pub fn run(save: &Path, assets_dir: &Path, opts: RunOptions) -> Result<BenchReport, String>;
/// Named scalar view, used by the launcher's targets; unknown name → Err.
impl BenchReport { pub fn measure(&self, name: &str) -> Result<f64, String> }
pub const MEASURES: &[&str];   // every name `measure` accepts
```
Measure names (P1): `economy.running_share.<kind>` (mean over machines of
that kind), `economy.line_output_per_1000.<line key>`,
`economy.labour_unworked`, `economy.items.<item id>`. `measure` parses the
prefix; `MEASURES` lists the fixed prefixes for error messages.

Tests (`cargo test -p feral-processes-engine bench`):
- edge fold: synthetic initial statuses + `MachineStall` list → expected
  per-status counts, summing to `ticks`. (Templates resolve only in the
  launcher, so the real-`chains` checks live in T3.)
- `measure("economy.nope")` is `Err`; a known name returns the field.
- same seed twice → equal reports; two seeds → differ somewhere (on a
  `Game::new` save written to a temp path unique to the test).

### T2 — launcher: scratch assets + knobs (crate: launcher)

Files: new `launcher/src/scratch_assets.rs`; `tuner/eval.rs` (`Workspace`
:24 uses it, its `copy_tree` :209 deleted), `bin/train.rs` (`copy_tree`
:458 deleted, `AssetPool` :381 uses it); new `launcher/src/bench/{mod.rs,knob.rs}`.

```rust
// scratch_assets.rs — unique dir via pid + AtomicU64 counter (no clock), copy, remove on drop
pub struct ScratchAssets { /* dir */ }
impl ScratchAssets { pub fn new(from: &Path, tag: &str) -> Result<Self, String>; pub fn dir(&self) -> &Path }
// bench/knob.rs
#[derive(Deserialize)] pub struct Knob { pub file: String, pub field: String, pub min: f64, pub max: f64 }
pub fn patch(pristine: &str, field: &str, value: f64, integer: bool) -> Result<String, String>;
pub fn read_back(assets: &Path, knob: &Knob) -> Result<f64, String>;
```
- `patch` walks the dotted path through RON text (nested `name: (`…`)`
  scopes, then `leaf: <number>`). Zero matches or two matches → `Err`
  naming the path. `craftable.cost` amounts: path form
  `craftable.cost.<item id>` matching the `("item", N)` tuple.
- `read_back` loads `StructureDb::load_dir` from the scratch dir and reads
  the typed field (phase-1 fields only: `work.ticks_per_unit`,
  `assembles.ticks_per_unit`, `capacity`, `power_draw`,
  `craftable.cost.<id>`); any other path → `Err` ("not a phase-1 knob").
- Integer-ness comes from the field, not the file: every phase-1 field is
  integral.

Tests (`cargo test -p feral-processes <name>`): patch + read-back round trip
on a real structure file; no-match fails; double-match fails; 12.6 → 13;
pristine reused (patch twice from the same original, second doesn't see
first). Existing tuner and train tests still green unchanged.

### T3 — launcher: objective, search, bin (crate: launcher)

Files: `launcher/src/bench/{objective.rs,search.rs}`, new
`bin/bench.rs`, `launcher/Cargo.toml` (`[[bin]] bench`).

```rust
#[derive(Deserialize)] pub struct Target { pub measure: String, pub min: f64, pub max: f64 }
#[derive(Deserialize)] pub struct Objective { pub template: String, pub ticks: u64,
    pub seeds: Vec<u64>, pub holdout_seeds: Vec<u64>, pub iterations: usize,
    pub population: usize, pub search_seed: u64, pub targets: Vec<Target>, pub knobs: Vec<Knob> }
impl Objective { pub fn load(path: &Path) -> Result<Self, String> } // validates
pub fn range_error(value: f64, min: f64, max: f64) -> f64;
pub fn search(obj: &Objective, assets: &Path, save: &Path) -> Result<Proposal, String>;
```
- `load` errors: unknown measure prefix (against `bench::MEASURES`),
  `min >= max` on a target or knob, empty `seeds`/`holdout_seeds`, overlap
  between them. Missing `min`/`max` is a serde error already — test it.
- One `ScratchAssets` per CEM worker thread (pool as `train`'s
  `AssetPool`); each candidate patches all knobs from pristine, read-back
  verifies, then `bench::run` per seed.
- Before search: read back every knob once on the pristine tree (fail fast
  before any tick).
- Output `dev-tuning/out/bench-<objective stem>/`: `proposal.ron` (knob →
  old, new), `report.md` (error before/after on search seeds, hold-out error
  before/after, per-target values), patched files for `diff -r`. Never
  writes `assets/`.
- CLI: `run --template <name> --ticks N [--seed N] [--out path]` (RON to
  `--out`, else stdout; summary to stderr) and `tune <objective.ron> [--out
  dir]`. Template → save via `dev_template::resolve`. Arg parsing in the
  style of `bin/arena.rs:54` (slice match).

Tests: `bench::run` on `chains`, 300 ticks — every machine's
`status_ticks` sums to `ticks`, end-of-line output > 0; below/above grow, normalised by width;
`range_error` inside = 0, below/above grow, normalised by width;
objective load errors above; `parse_args` cases; a 2-iteration × 2-pop
search on `chains`, 200 ticks, one knob — produces a proposal whose values
lie within bounds (smoke, not convergence; `cem_converges_on_a_quadratic`
covers that).

### T4 — baseline, docs, release prep (no code)

1. `cargo build --release --bin bench`; time `run --template chains --ticks
   5000` (3 runs, report median).
2. Baseline `run` on 3 seeds; pick budgets; write `dev-tuning/economy.ron`
   with 2–4 targets read off the baseline's starve/clog shape (e.g.
   assembler running share, end-of-line output) and the four phase-1 knob
   kinds on `chains`' structures.
3. One `tune` run; keep its report.
4. `docs/measurements/2026-10-05-base-bench-chains-baseline.md` per
   `docs/measurements/README.md`: timing, baseline numbers, the tune's
   before/after/hold-out, and the spec's Known limit (one small base).
5. `dev-tuning/README.md`: a `bench` section (commands, output, proposal-not-
   edit). `CHANGELOG.md` entry text staged for the release; no version bump
   on the branch.
6. Memory graph: `decision:economy-tuning-search-scope` (the deferred C, per
   the spec's Not doing) if not already present.

Gate: `cargo test --workspace`, clippy, fmt. Then opus whole-branch review
(diff as a file), fix, land per `landing-work-is-merge-tag-push-cleanup`
(push only on the user's ask).

## P2 — Staff

Branch `staff-bench`. Same execution rules as P1 (serial sonnet tasks, gates
off, opus whole-branch review at the end, never push, explicit paths, never
stage `art/`).

### Decisions

- **No `staffed-base` capture.** `bench-economy` already is `chains` plus a
  Defrag Bay and a Sandbox, with 15 staff and 26 structures, so it is past
  `BASE_ESTABLISHED_STAFF`/`_STRUCTURES` (8/8). Baseline `bench-economy`
  (amenities) and `chains` (none: the fray contrast).
- **Tantrums and frays are telemetry records, not log scraping.**
  `message_log` is capped at 100, has no tick and is pruned after battle.
  Add `Record::Tantrum { tick, who }` (one per tantrum started, not per
  brawl line) and `Record::Fray { tick, who, need, unreachable }` (one per
  `fray` call), built where the log line is written. Not production, so
  they do not go through `base_ledger::emit`; they follow `MachineStall`'s
  pattern (telemetry-only, behind `enable_telemetry`).
- **Two views, no `World` accessor.** `Game::need_levels(who) ->
  Vec<NeedLevel { id: String, level: f32, critical: f32, content: f32 }>`
  (sorted by id, empty without `Needs`) and `Game::grievance(who) ->
  Option<&'static str>` (`"sulking"`/`"downed_tools"`/`"lashing_out"`, a
  `Grievance::as_str`). Both in `game/base/offshift.rs` beside `need_rows`.
- **Sampling is per staff-tick** over `base_staff()` re-read each tick (staff
  can leave or join mid-run).
- **Knobs ship with the measures, before the stop.** The stop is only for
  target ranges; knob plumbing does not depend on them.

### StaffReport (replaces `staff: Option<()>`; `#[serde(default)]` not needed — reports are not saves)

```rust
pub struct StaffReport {
    pub staff_ticks: u64,            // sum over ticks of base_staff().len()
    pub on_shift_share: f32,         // errand_label None / staff_ticks
    pub morale_mean: f32, pub morale_min: f32,
    pub sulking_share: f32,          // morale < MORALE_SULKS_AT
    pub need_strain_mean: f32,
    pub needs: BTreeMap<String, NeedBench>, // mean, min, critical_share (level < critical)
    pub rung_share: BTreeMap<String, f32>,  // "none" + each rung, over staff_ticks
    pub tantrums: u64, pub frays: u64,
    pub first_fray: Option<u64>,     // ticks from run start
    pub end_morale: Vec<f32>,        // per staff at the last tick, sorted
}
```

New `MEASURES`: `staff.on_shift_share`, `staff.morale_mean`,
`staff.morale_min`, `staff.sulking_share`, `staff.need_strain_mean`,
`staff.need_mean.<need>`, `staff.need_critical_share.<need>`,
`staff.rung_share.<rung|none>`, `staff.tantrums_per_1000`,
`staff.frays_per_1000`. Unknown need/rung id → `Err`, as for economy.

### T5 — engine: records + views (crate: engine)
`Record::Tantrum`, `Record::Fray`; `need_levels`, `grievance`. Tests: a
forced fray (no amenity, need set below critical) emits one `Fray` with
`unreachable: false`; a tantrum emits exactly one `Tantrum`; views read back
a set level and each rung; no telemetry → no records.

### T6 — engine: StaffReport (crate: engine)
Pure fold in `report.rs` (unit-tested without a `Game`: shares, mins,
first fray, per-1000 rates, `measure` arms); sampling in `play_with`. A run
test on `bench-economy` asserts `staff_ticks == 15 × ticks` and shares in
[0,1].

### T7 — launcher: knobs (crate: launcher)
`read_back` arms: `NeedDef` (`drain_per_tick`, `working_multiplier`,
`critical`, `content`, `morale_weight`), `ThoughtDef.intensity`,
`StructureDef` `services.<need>.per_tick` / `.radius` (a `locate` special
case like `craftable.cost.<item>`). Update `dev-tuning/README.md`. Tests:
each new arm patches and reads back; an ambiguous/missing path errors.

### T8 — baseline (no code)
Release build; `bench run` on `bench-economy` (its P1 orders, seeds 1–5,
3,000 ticks) and `chains` (same). Write
`docs/measurements/2026-10-05-base-bench-staff-baseline.md` with every
staff measure, `stopped_at`, first fray. **Stop: the user sets target
ranges and picks knobs.** Then T9 writes `dev-tuning/staff.ron`, runs
`tune`, and writes the proposal; CHANGELOG `## Unreleased` line.

**Risk to check at T8:** coherence drains 0.02/tick ×2.0 at work, so a full
reserve reaches critical (20) after ~2,000 working ticks, and a siege stops
`bench-economy`'s clock at ~3,800. If 3,000 ticks show no fray or rung
movement on `bench-economy`, report it rather than lengthen the run; the
siege fix (party outside base space) is a separate decision.

### After the baseline (user, 2026-10-05)

- **Siege dev switch (T9).** One engine switch, `Game::dev_set_sieges(bool)`
  (a non-saved Resource; `siege_check` does nothing while off). `bench
  run --no-sieges` and objective field `sieges: false` (`#[serde(default =
  true)]`); the game honours `FERAL_DEV_NO_SIEGES=1` where
  `FERAL_DEV_REVEAL` is read. README + `dev-tuning/README.md`.
- **Staff objective (T10)** on `bench-economy`, sieges off, 6,000 ticks
  (re-baseline shipped values at that length first; record it in the staff
  baseline doc). Targets, "occasional trouble":
  `staff.sulking_share` 0.05–0.15, `staff.need_critical_share.coherence`
  0.01–0.05, `staff.need_mean.slack` 60–85, `staff.frays_per_1000` 0–0.2,
  `staff.rung_share.downed_tools` 0–0.02.
  Knobs: coherence + slack `drain_per_tick`, `working_multiplier` (±50%),
  `critical` (10–30), `content` (50–80), `morale_weight` (±50%); Defrag Bay
  and Sandbox `services.<need>.per_tick` (±50%); the five thoughts'
  `intensity` (sign kept, magnitude 0.5–4). Proposal doc, CHANGELOG
  `## Unreleased`.

## P3 — Memories (outline; plan pass after baseline)

`MemoryReport` (morale distribution, bond distribution from `social`, fired
count per memory kind from `memory_report`). Baseline lists fired kinds →
**stop; user picks targets and knob list** → memory-def knobs restricted to
fired kinds.
