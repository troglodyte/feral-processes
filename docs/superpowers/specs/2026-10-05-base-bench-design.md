# Base bench — design

**Status:** spec, awaiting review.

## Intent

A headless instrument for the base: run a template for N ticks, report what
the economy, the staff and their memories did, and search asset values
against target ranges the way `tuner` searches species stats. It is the
"machine learning around the base" the user asked about, in the form that
fits this codebase: offline, deterministic, a proposal a person reads — never
a learned policy at runtime. Base scheduling stays priority-by-list-position
(`.claude/rules/seams-base.md`); nothing here changes game behaviour.

What the user said: do approach A (asset-only search) now; extend it to staff
and memories; record the deferred alternatives in memory. Answers to the
brainstorm questions: the search ships now, not later; templates are chosen
per run, `chains` first; exact production counts; CLI flags, not a scenario
file; staff/memory targets are set by the user **after** a baseline run;
three phases in one spec; `chains` is the only economy template for now.

## Today

- No tool runs a base for N ticks. `arena`, `train`, `tuner`, `savetool warp`
  and `balance_sim` are all about combat; nothing in `docs/measurements/` is
  about the base.
- `docs/base-economy-audit.html` derives the starve/clog problem by hand
  (extractor supply grows ~16× over a run, assembler demand stays flat).
- Production is already counted: every seam calls `base_ledger::emit`
  (`.claude/rules/seams-instrumentation.md`), which folds into `BaseLedger`
  (per item, always on) and builds telemetry `Record::Extract` /
  `Record::Assemble` (per machine: position, kind, tier, worker species).
  `enable_battle_telemetry` turns base records on too, despite its name.
- Staff behaviour is mostly data: `assets/needs/` (2 defs), amenity
  `services` on `defrag_bay.ron` / `sandbox.ron`, `assets/thoughts/` (5).
  Memories are data: `assets/memories/` (34 defs), `assets/interactions/`
  (10). Grievance rungs, tantrums, idle drift and the "established base"
  thresholds are `tuning.rs` constants and out of reach (see Not doing).
- No dev-save template has an amenity built.

## Shape — follows `arena`

**Engine `crates/engine/src/bench/`** does the measuring. `bench::run(save,
assets_dir, RunOptions { ticks, seed })` loads the game, enables telemetry,
calls `Game::wait()` `ticks` times, samples once per tick through the
**public `Game` API**, drains telemetry at the end and returns a
`BenchReport { economy, staff, memories }` (later sections `Option` until
their phase lands). Public API only, so it measures what the screens can
see; `Game.world` stays private.

**Launcher `bin/bench.rs`** is the CLI, in the launcher for the same reason
`arena` is: only the launcher's `dev_template` can turn a template name into
a save.

```sh
cargo run --release --bin bench -- run --template chains --ticks 5000 [--seed N] [--out report.ron]
cargo run --release --bin bench -- tune dev-tuning/economy.ron
```

`run` writes RON to `--out` and a short summary to stderr (stdout stays
clean for piping, as `arena` does).

## Measures

### Economy (phase 1)

- **Per machine:** ticks in each `MachineStatus` (sums to `ticks`), units
  made (from `Record::Extract` / `Assemble`), Running share.
- **Per line** (`line_reports`): end-of-line output per 1000 ticks, ticks in
  each line status.
- **Base-wide:** `labour_demand` wanted vs staffed, unworked by duty
  (mean over ticks); `BaseLedger` lifetime totals per item.

### Staff (phase 2)

Share of staff-ticks on shift (`program_errand_label` is `None`), morale
mean / min / share below the Sulking line, mean `need_strain`, tantrum count
(`message_log` lines of `MessageKind::Tantrum`), tick of the first fray.
New read-only `Game` methods for the two values not exposed today: each
need's numeric level and the grievance rung. Views, not a `World` accessor.

### Memories (phase 3)

Morale distribution (mean, spread, share downing tools), bond distribution
from `social`, and **which memory kinds fired** (count per kind from
`memory_report`) — the list that decides which memory defs can be knobs.

## Knobs

```ron
(file: "structures/assembly_bay.ron", field: "assembles.ticks_per_unit", min: 10, max: 40)
```

- Every knob has bounds — the reason is `tuner::objective::Bound`'s doc.
- Patching rewrites the number at the dotted field path as **text**, in a
  scratch copy of `assets/`, always from the pristine file, never from the
  previous candidate. Integer fields are rounded.
- **Fail fast:** after patching, the tool loads the asset dir and reads the
  value back through the typed def; a field path that matched nothing, or
  matched twice, is an error before any tick runs.
- Scratch-assets setup (copy tree, unique dir, cleanup on drop) is pulled out
  of `tuner::eval::Workspace` and `train.rs`'s `copy_tree` into one shared
  module that all three call.
- Phase 1 knobs: structure defs only (`work.ticks_per_unit`,
  `assembles.ticks_per_unit`, `capacity`, `power_draw`) and `craftable.cost`
  amounts. Phase 2: needs, amenity `services`, thought intensities. Phase 3:
  memory defs, restricted to kinds the baseline saw fire.

## Search

- `cem.rs` unchanged, over the knob vector normalised to [0, 1].
- **Fitness** is the summed distance outside target ranges, shape of
  `tuner::score::target_error`: zero inside a range, growing outside it,
  each target normalised by its range width so units don't dominate.
- **Objective file** `dev-tuning/<name>.ron`: `template`, `ticks`, `seeds`,
  `holdout_seeds`, `iterations`, `population`, `search_seed`, `targets`
  (`(measure: "economy.assembler_running_share", min: 0.6, max: 1.0)`),
  `knobs`. An unknown measure name is a load error, not a silent zero.
- **Output** is a proposal in `dev-tuning/out/`, like `tuner`: new values,
  error before/after, hold-out error. It never edits `assets/`.
- Search seeds vs hold-out seeds as in `tuner`: candidates compared on one
  pinned seed set, the final proposal scored on seeds the search never saw.

## Phases

Each lands and releases on its own.

1. **Economy.** Bench core, economy measures, knobs, shared scratch-assets
   module, CEM search, starter `dev-tuning/economy.ron`. Ends with a baseline
   `chains` run written up as a `docs/measurements/` entry.
2. **Staff.** Staff measures and the two new read methods; capture a
   `staffed-base` template (`chains` plus a Defrag Bay and a Sandbox, run
   long enough to be established). Baseline on `chains` (the neglected base)
   and `staffed-base` → **the user sets the target ranges** → staff knobs on.
3. **Memories.** Memory measures; baseline records which kinds fire → the
   user picks targets and the short knob list → search on.

## Testing

- Knob patch + read-back; a path that matches nothing fails; one that matches
  twice fails; integer rounding.
- Target error: inside range is zero, outside grows, normalised.
- Report folding: per-machine status counts sum to `ticks`.
- A short `chains` run (a few hundred ticks) produces nonzero end-of-line
  output — same layout guarantee `the_chains_template_starts_with_a_chain_that_actually_runs` relies on.
- Objective load: unknown measure name and missing bounds are errors.
- CEM itself is already proven by `cem_converges_on_a_quadratic`.

## Open, settled in the plan

- **Re-seeding.** A hold-out needs different seeds for one template. Check
  how `arena` varies seeds per rep and reuse it; if `Game` exposes no
  re-seed, adding one is a phase-1 engine change.
- **Cost of a run.** Time 5000 ticks of `chains` in release before choosing
  search budgets; CEM multiplies runs by population × iterations × seeds.
- **Telemetry rename.** `enable_battle_telemetry` → `enable_telemetry` if
  callers allow; otherwise leave it and note why.

## Not doing

- **Learned scheduling or worker policies.** Posting stays list-position
  priority; a learned policy would make it unpredictable to the player.
- **B — economy constants into an asset.** Breaks `content-schema.md`'s
  "content is moddable, difficulty is not", and is a schema change.
- **C — runtime-overridable `tuning.rs`.** Deferred, not rejected: a
  `Tuning` Resource defaulting from the existing consts, never saved or
  loaded from assets, set only by dev bins, migrated section by section.
  Revisit when the asset-only search is measured to hit a wall on a code
  constant. Recorded in the memory graph as
  `decision:economy-tuning-search-scope`. The `Knob` type is where it would
  plug in as a second knob kind.
- **Placing structures from the search.** Knobs change values, not layout;
  layouts come from templates.

## Known limit

With `chains` as the only economy template, a phase-1 proposal is tuned to
one small two-line base: a demonstration of the instrument, not a balance
change. A captured late-game factory becomes the hold-out template when one
exists.
