# feral-processes

A headless Rust ECS game sim with a graphical renderer. 4-crate Cargo
workspace.

**This file is loaded into context on every turn, so it holds rules and not
arguments.** The reasoning behind each load-bearing seam — the measurement,
the history, what was tried and rejected — lives in the **memory graph** as
`seam:<slug>`. Read the matching entry there before changing a seam, and write
any new reasoning there rather than here; the `seams` skill has the two calls.

The crates:

```
crates/engine    (feral-processes-engine)   headless sim, standalone bevy_ecs
crates/app-core  (feral-processes-app-core) App/Mode input-and-flow state machine
crates/gui       (bevy + bevy_egui)         the renderer
crates/launcher  (feral-processes)          the binary
```

**Architectural rule:** the engine's `Game` struct (`crates/engine/src/lib.rs`)
is the entire public API surface the renderer talks to via app-core. The
renderer never touches the ECS `World` directly. Keep it that way. A graphical
display is required; there is no text mode and no headless play.

What enforces the rule is `Game`'s `world` field being private with no
accessor — a compiler barrier from outside the crate. **What convention alone
has to hold is the accessor never being added**: `crates/gui` now has
`bevy_ecs 0.19` in its graph via `bevy`, the same version the engine uses, so
a `pub fn world_mut()` added in a weak moment would be immediately usable by
the renderer with no new dependency and no version mismatch to give anyone
pause. The second consumer that used to hold the rule (`crates/tui`) is gone.
Relatedly, the frontend is one big system rather than idiomatic Bevy
components — `crates/gui/src/lib.rs` documents why, and the reason is this
rule.

**The drawing seam:** `crates/gui/src/paint.rs` is the only file that names a
graphics library, and `render/` draws only through its `Painter`. The full
rule — pane origins, `clipped`, `sprite` and the player icon — is
`.claude/rules/drawing-seam.md`, loaded when a `crates/gui` file is read.

## Load-bearing seams

Facts that cost tool calls to rediscover every session, **one sentence each:
the rule alone.** They live in `.claude/rules/seams-<subsystem>.md`, each
path-scoped so it loads only when a matching file is read — reading a file
is what puts its subsystem's rules in context. **Before changing code in a
subsystem, read its rules file if it has not loaded**; the list is
`ls .claude/rules/`. They were moved out of this file on 2026-09-28 because
they were ~93 KB of the ~119 KB paid on every turn.

The trap each rule exists to close is in the **`seams` skill**
(`.claude/skills/seams/`). The argument behind a seam — the measurement, the
history, what was tried and rejected — is in the memory graph as
`seam:<slug>`, reached with `memory_search(…, subsystem: "seams")` then
`memory_get_entity`; read it before changing a seam itself.

**One sentence is a budget, not a style.** A new seam is three writes — the
argument to the graph, the trap to the skill, the rule to its rules file —
and the skill documents the order. A new subsystem is a new rules file with
a `paths:` list generous enough to load wherever its seams are touched.

Each was verified against the source, not remembered. Verify again before
relying on one, and correct all three places if it has moved.

## Build & test

```sh
cargo test --workspace     # 6246 tests
cargo run                  # the game; `default-run` in crates/launcher
cargo clippy --workspace --all-targets   # --all-targets or test code is unlinted
cargo fmt

# Edit a save for testing: dump to RON, edit, pack back. `warp` runs the
# real breach rather than editing the zone number.
cargo run --bin savetool -- dump saves/save.bin s.ron
cargo run --bin savetool -- pack s.ron saves/save.bin
cargo run --bin savetool -- warp saves/save.bin 6

# Start from a known world instead of playing up to one. `dev-saves/README.md`
# lists what each template sets up.
cargo run -- --template extraction
cargo run --bin savetool -- template                        # list
cargo run --bin savetool -- capture saves/save.bin <name>   # record a new one

# Run a battle offline instead of playing to it. `dev-arenas/README.md` is
# the schema; the shipped scenarios are worth re-running after a retune.
cargo run --bin arena -- dev-arenas/opening-fight.ron
cargo run --bin arena -- dev-arenas/full-group.ron --out report.ron

# ...or play the same scenario in the real battle UI, which is the only way
# a companion Special ever fires in an authored fight. Main menu, [R] Arena.
FERAL_DEV_ARENA=1 cargo run

# See a screen without a person at the desktop: load a template, press
# keys through App::handle_key, write a 1280x720 PNG and exit. It opens a
# real window for ~1s, so it needs DISPLAY or WAYLAND_DISPLAY. Look at the
# file with Read. Keys are GameKey names, `Space`, or one character.
cargo run -- --template stack --keys "Right Right" --screenshot out.png

# Draw the whole Stack frame on both maps instead of what has been walked,
# so testing a cell kind doesn't start by walking a maze to find one. Map
# only — see `dev-saves/README.md`.
FERAL_DEV_REVEAL=1 cargo run -- --template stack
```

**Cutting a Windows or macOS release** is manual by choice — no CI, no
`cargo-dist`, and cross-compiling from Linux is deliberately unsupported.
The per-platform checklists, the toolchain each needs, and the argument for
shipping a plain binary rather than a `.app` bundle are in
[`docs/releasing.md`](docs/releasing.md). The deliverable is always an
executable **plus a loose `assets/` tree**, never a single file — fonts and
sound cues are `include_bytes!`d but game content must stay droppable, which
is the moddability rule.

**`docs/measurements/` is what the instruments have already said.** One file
per question answered, each carrying the commands that produced it, the
numbers, and what the run was blind to. Read it before running a sweep or an
arena batch to answer something — the data behind these is hundreds of
megabytes and gitignored, so a number not written down there costs
CPU-hours and an afternoon to get back. Its `README.md` is the convention
for adding one, and the bar: something was run, the data is gone, and a
decision depends on it. Balance curves are excluded on purpose — those are
`balance_sim.rs`'s job, and a copy here would drift.

**Don't reach for a fresh `Game::new` when a `dev-saves/` template would
do.** Testing anything mid-run by hand — extraction, trade, a full party,
a deep zone — otherwise starts with an hour of play, and that cost is what
makes features ship unplaytested. `capture` any state worth returning to.

`savetool` and `arena` are the launcher crate's other two bins. They sit
there rather than in the engine so `default-run` can keep a bare `cargo run`
unambiguous — `default-members` in the workspace root would have done it
too, but would also have narrowed a bare `cargo test` to the launcher.
`arena` has a second, harder reason it cannot live in the engine: it
resolves `dev-saves/` template names, and `dev_template` is the launcher's.

The launcher's `[lib]` target exists solely so its three bins can share
`dev_template` and so that module can be unit-tested once. No game logic
lives there, and none should — the crate is still the binary.

**A debug build is a playable build, and `[profile.dev]` in the root
`Cargo.toml` is what makes that true.** Dependencies build at `opt-level =
3` and the four workspace crates at `1`. Without it the renderer's own
shape-building pass ran **51.4 ms a frame** against release's 2.0 ms at an
identical shape count — under 20 fps before bevy, wgpu or egui's
tessellator had done anything. Deleting the section is a 22x frame-cost
regression that no test catches and that reads as an animation bug. Numbers
and blind spots are in
`docs/measurements/2026-08-19-debug-build-frame-cost.md`.

**Warm builds are not the bottleneck.** `cargo check --workspace` with
nothing changed is ~1.8s; touch one file and the affected crate's test
binary rebuilds in ~2.5s. The engine suite runs in ~6.7s. Save/load was
never the play cost either — on the real 190 KB save a full round trip is
1.46 ms in release. The engine depends on `bevy_ecs` alone; the
557-dependency Bevy graph is `crates/gui`'s, and only a cold build or a
dependency change costs minutes — budget ~3.5 minutes for one and nothing
for anything else. Iterate with `cargo test -p feral-processes-engine
<name>`; there is no tooling problem here to solve with sccache or nextest.

If many tests fail at once with `NotFound` on an assets path, it's stale build
artifacts, not 150 bugs — this repo was formerly at `/home/trog/code/petmud`,
and test helpers bake the asset path in via `env!("CARGO_MANIFEST_DIR")`. Cargo
doesn't invalidate on a directory rename. Fix with
`cargo clean -p feral-processes-engine -p feral-processes-app-core` rather than
a full `cargo clean`, which costs a cold rebuild of the Bevy graph.

**Disk hygiene: remove a worktree when its branch lands, and sweep
`incremental/` rather than the whole tree.** Every agent worktree builds its
own complete `target/`, and an abandoned one keeps it; that plus months of
`incremental/` took `target/` to 474 GB on 2026-09-01 (521 GiB reclaimed — the
"~4 GB" this file used to claim had been wrong for a long time). So `git
worktree remove` once a branch is merged, and `rm -rf target/debug/incremental`
for the periodic sweep: it gives back the bulk and costs only a slower next
compile. A full `cargo clean` costs the ~3.5-minute cold build every time and
is not a per-effort ritual.

## Moddability

This game must always stay moddable. Never hardcode new game content in
Rust when it can be expressed as data instead — species, structures, items,
and abilities should stay extensible by dropping in a file, not by editing
engine code.

- **New species** → add a `.ron` file to `assets/species/`. Schema is
  documented in `assets/species/README.md`.
- **New structures** → add a `.ron` file to `assets/structures/`. Schema is
  documented in `assets/structures/README.md`.
- **New items** → add a `.ron` file to `assets/items/`. Schema is
  documented in `assets/items/README.md`. `ItemId`
  (`crates/engine/src/items.rs`) is a string newtype, not an enum; shipped
  items are still reachable from Rust via the `ids` module in that same
  file (for test setup and data-defined recipes) but adding a new item
  never requires touching Rust.
- **New abilities** → add a `.ron` file to `assets/abilities/`. Schema is
  documented in `assets/abilities/README.md`. A species grants abilities by
  naming their ids with a level to unlock each at; `priority_boost` must
  exist, as it is the fallback for a companion whose species grants nothing.
- **New achievements** → add a `.ron` file to `assets/achievements/`. Schema
  is documented in `assets/achievements/README.md`. Unlike perks this is a
  real content directory — the four `Trigger` and three `Reward` shapes are
  the whole vocabulary and every combination already works. The *ceiling* on
  what the ladder may pay is not data: `tuning::MAX_PROFILE_*`, asserted over
  the real assets by `the_full_ladder_stays_under_its_ceiling`, because
  `balance_sim` models one run's curve and cannot see a cross-run profile.
- **New talent trees** → add a `.ron` file to `assets/talents/`. Schema is
  documented in `assets/talents/README.md`. A real content directory: the five
  `TalentNode` kinds are the whole vocabulary and a sixth class's tree is a
  file, not a Rust change. `Accuracy` is the one read on demand alongside
  `Affinity` — it has no `Stats` field to bake into — and the tier count is
  fixed, so a new node kind costs every shipped tree an existing choice.
  Exactly `KERNEL_RING_MAX * LEVELS_PER_RING` tiers of two choices each, or
  the tree is skipped with a warning; six censuses in `tests/assets.rs` hold
  the *shipped* trees to the design.
- **New help pages** → drop a `.md` file in `assets/help/`. Schema is
  documented in `assets/help/README.md`. Five block rules and no more; the
  filename is the ordering *and* the id a link points at, so there is no
  front matter and no second parser. A page is prose, which is why this one
  directory is markdown rather than RON.
- **Perks are half data, and the seam is deliberate.** `assets/perks/*.ron`
  is a *catalogue*: one file per `Perk` variant carrying its name,
  description and Perk Point cost, loaded by `PerkDb` and reached from the
  renderer through `Game::perk_defs`. The variants themselves stay in
  `crates/engine/src/perks.rs`, because a perk's effect is a hook into a
  particular formula with no shared shape to express as data. So a
  nineteenth perk is a new `Perk` variant plus a **named query in
  `perks.rs`**, called from the site that applies it; `PerkDef` deliberately
  has no `effect` field. **That module is the census** — one query per perk,
  and `every_perk_has_a_query_that_answers_what_it_is_worth` is exhaustive on
  `Perk` (`cell_mark`'s rule), so a variant with no query fails to compile.
  **The two signature families are not interchangeable**: most take the
  player's `Option<&Perks>` and name their own variant, so no call site says
  `Perk::` at all; `mining_roll_bonus` and `quality_floor_bonus` take a bare
  **level**, because `CycleModifiers` and `CraftOrder` carry it to a formula
  whose subject may be a program rather than the player — a `Perks` argument
  there quietly hands a program the player's investment. The three `StatGain`
  perks are in the module too: `purchase_stat_gain` says what buying one
  grants and `unlock_perk` stays the one writer of `Stats`. Per-level
  *magnitudes* stay in `tuning.rs` (below); only cost crossed over. **A
  perk's hook belongs where its sources meet**, not at each of them:
  `Obfuscation` is read inside `raise_trace` rather than at the six things
  that raise Trace. A hook that has to be repeated is the signal the perk is
  aimed at the wrong seam. **`Perk`'s variant order is save format** —
  bincode encodes enums positionally, so `PlayerSave::unlocked_perks` holds
  indices; append, don't reorder, or bump `SAVE_FORMAT_VERSION`.
- **Difficulty tuning** (`crates/engine/src/tuning.rs`) is deliberately code,
  not data. Content is moddable; how hard the game is, is not. Every knob the
  engine hardcodes — zone and Stack-depth scaling, XP curves and level caps, the
  damage and capture formulas, spawn and drop rates, raid pressure, need
  decay, perk magnitudes — is a documented `pub const` there, grouped into
  labelled sections. Put new tuning values in that file rather than inline in
  a formula, and don't duplicate `.ron` values into it.

Rules to follow whenever the schema changes:

- Adding a field to `SpeciesDef`, `StructureDef`, `ItemDef`, or `AbilityDef`?
  Mark it `#[serde(default)]` so existing `.ron` files — including anyone's
  custom mods — keep parsing without being touched.
- A malformed `.ron` file must be skipped with a logged warning, never a
  panic that crashes startup. Follow the existing pattern in
  `SpeciesDb::load_dir` / `StructureDb::load_dir` / `ItemDb::load_dir` /
  `AbilityDb::load_dir` / `PerkDb::load_dir`.
- Update the matching `assets/*/README.md` in the same change whenever a field
  is added, removed, or changes meaning — those docs are the schema reference
  for anyone modding the game.

## Code principles

- **DRY, but not prematurely.** Three similar lines beat a speculative
  abstraction.
- **KISS / YAGNI.** No half-finished implementations, no unused feature flags,
  no building for hypothetical requirements.
- **Fail fast.** Validate early, handle errors explicitly. Don't write error
  handling for scenarios that can't actually happen.
- **Consistency.** Follow existing naming and structure rather than
  introducing new conventions.
- **No backwards-compat cruft.** Don't rename unused variables to dodge a
  lint, leave `// removed` comments, or add shims for code you're free to just
  change. If something's unused, delete it.
- **Comment discipline.** Comments explain *why* — a non-obvious constraint, a
  workaround, a subtle invariant — never *what*. Well-named code already says
  what; a comment restating it is noise.
- **Don't assume.** If you think code works a certain way from memory, open it
  and check.
- **A doc comment claiming to mirror other code must be a call, not a copy.**
  If you write "mirrors", "shared with", "matches", or "same as" about another
  module's formula, extract that formula into a pure function both sides call.
  A comment cannot hold two copies in sync, and the copy that drifts is
  usually the one nobody runs. This has bitten this repo four times, all in
  `balance_sim.rs`. `battle::attackers_in_group`, `battle::slot_aggro_weight`,
  `battle::expected_damage` and `systems::node_payout` are the pattern to
  follow — `expected_damage` is the RNG-free mean of exactly what
  `resolve_attack` rolls, and the sim calls it rather than keeping a copy.
- Composition over inheritance. Avoid global mutable state and god objects.
  Named constants over magic numbers (see `crates/engine/src/tuning.rs`). No
  optimization ahead of evidence it's needed.

## Rust idioms

- Run `cargo fmt` and `cargo clippy --workspace --all-targets` after every
  change; fix warnings and deprecations rather than silencing them.
  **`--all-targets` is load-bearing** — a bare `cargo clippy --workspace`
  leaves every test module unlinted, which is how three warnings sat in
  `structures.rs` and `render/progression.rs` across two releases.
- Prefer `Result`/`?` propagation over panics in engine code. `unwrap()` /
  `expect()` are for tests, truly-infallible invariants, or startup config
  that should abort anyway.
- Work with the borrow checker's grain — small, focused functions are usually
  the fix for a fighting-the-borrow-checker moment, not reflexive `.clone()`.
- Keep error types explicit; avoid `Box<dyn Error>` catch-alls where callers
  need to branch on failure mode.

## Testing

- Business/sim logic gets unit tests. For a bug fix, write the failing
  reproducer first.
- **No flaky tests.** No `sleep()`, no wall-clock dependence, no reliance on
  RNG you didn't seed — background systems (habitat spawning, nests) can and
  will interfere with a naive assertion.
- **Full suite is the final gate.** Run `cargo test --workspace` before calling
  anything done. Passing only the tests you wrote is not evidence of
  correctness.
- **`balance_sim.rs` is the balance regression gate.** Despite the name it is
  not a constants table (those are in `tuning.rs`) — it's a deterministic,
  RNG-free battle simulator whose tests assert hardcoded empirical level
  curves computed from the live constants against the real `.ron` assets. Any
  change to `tuning.rs`, a species file, or an item file should be checked
  with `cargo test -p feral-processes-engine balance_sim`. A curve that moves
  means progression changed — that's the signal, not a broken test.

## Process weight

Match the process to the blast radius, the same way **Surgical changes**
below matches the diff to the request. Measured on the permadeath feature: a
6-file, ~280-line change carried a 178-line spec and an 874-line plan — 3.7x
the deliverable, in write-once prose, and essentially none of the ~58,000
lines that accumulated under `docs/superpowers/` was ever read twice.

The 46 implementation plans were deleted on 2026-08-13 — git history is their
archive. The 59 specs moved to `docs/superpowers/archive/specs/`, because
nine of them *are* cited from source doc comments as the rationale record for
a seam. `docs/superpowers/INDEX.md` is the one-file answer to "what shipped,
and where is its argument"; every spec in it is implemented, and a spec's own
`**Status:**` header is stale for 14 of them, so never answer from it.

- **One crate, no schema or save-format change, fits in one context** →
  brainstorm to a decision, then TDD inline with a commit per green step. No
  spec file, no plan file. A plan document exists to hand context to a
  subagent that lacks it; writing one for work you are about to do yourself
  in the same session is writing the feature twice.
- **Two or more crates, a schema or save-format change, or you genuinely
  want subagent isolation** → the full spec-and-plan pipeline. That is what
  it is for, and it earns its cost there.

When a plan *is* warranted, don't write the implementation inside it. A
subagent that has the repo and this file needs the file list, the interface
it must produce, the intent of each test, and the gates to run — not
finished code it will merely re-emit. Reserve code blocks for the genuinely
non-obvious: a borrow-scoping trick, an ordering constraint, a formula.

The size rule governs the pipeline, not the discipline. TDD, the failing
test first, and the full-suite gate apply at every size.

## Running subagents

Multi-agent workflows (superpowers SDD and friends) are expensive here — a
single Phase-1 refactor cost ~1.6M subagent tokens across 16 sequential
dispatches. The isolation is worth paying for on a cross-crate refactor;
it is not worth paying for by reflex. Rules learned the hard way:

- **Sonnet is the default. Opus is for judgment, not volume.** A large
  type-flip review or a whole-branch review earns opus. A mechanical fix
  pass, a cleanup sweep, or a re-review of a fix does not — those ran 100+
  tool uses on opus and shouldn't have.
- **Don't re-run the full suite to confirm what a subagent already
  reported.** Spot-check with a targeted `cargo test -p … <name>`; save
  `cargo test --workspace` for task boundaries. The full-suite gate above
  is about not shipping untested work, not about auditing every claim.
- **Never brute-force a flake with repeated runs.** Read the code path
  first — the one intermittent failure this repo produced was diagnosed
  from source in a dispatch that was already running, after 15 rebuilds
  found nothing.
- **Per-task review gates are optional; the final whole-branch review is
  not.** Dropping the per-task gate roughly halves the dispatch count, at
  the cost of defects surfacing at the end instead of immediately. Ask
  which tradeoff the user wants before starting, not after.
- **Give the diff as a file, never pasted into the prompt.** Everything
  pasted into a dispatch stays in context for the rest of the session.

## Guardrails

- **Git:** commit freely as work reaches a green, coherent state — a passing
  test, a finished task. Branch first if on `main`. Pushing still needs an
  explicit ask, and so do force-push, `reset --hard`, and amending pushed
  commits.
- **Versioning: one release per change that lands on `main`.** A feature or
  fix merged to `main` bumps the workspace version in the root `Cargo.toml`,
  gets its own `## X.Y.Z` section in `CHANGELOG.md` and an annotated `vX.Y.Z`
  tag. Commits *on a branch* stay unversioned — the bump happens once, at the
  merge, so a rebase or squash can't invalidate a version already tagged.
  Which digit moves is decided by `CHANGELOG.md`'s preamble, the one
  statement of the policy; the short of it is that "breaking" means **a
  player's save stops loading** (`save::SAVE_FORMAT_VERSION`), not a changed
  type signature. This replaced batching into `## Unreleased`, which ran to
  2,200 lines and two save-format breaks between `v0.2.0` and `v0.3.0` — a
  version number that said nothing about what is installed. Note the tag push
  is separate from the commit push: `--follow-tags` sends annotated tags, a
  bare `git push` does not, and a tag that only exists locally is not a
  release.
- **Surgical changes:** match blast radius to the request. A bug fix doesn't
  need drive-by refactors. Don't take destructive shortcuts past an obstacle —
  find the root cause.
- **Investigate before overwriting:** before creating or replacing a file at a
  conventional path, check what's already there. Same for unfamiliar branches,
  config, or in-progress state you didn't create.
- **Secrets:** before staging, check nothing sensitive is included.

## Working with the user

- State assumptions and decisions plainly; don't hedge with caveats.
- Ask a specific blocking question over guessing at something only the user
  knows — but don't stall on decisions you can reasonably make yourself.
- **Batch independent questions into one round trip.** Ask sequentially only
  when a question genuinely depends on an earlier answer. This overrides the
  brainstorming skill's one-question-per-message rule, which spends a full
  round trip per decision — the permadeath brainstorm took four where two
  would have done.
- Report what you actually verified (commands run, output seen), not what you
  expect should work.
