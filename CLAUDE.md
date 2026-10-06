# feral-processes

A headless Rust ECS game sim with a graphical renderer. 4-crate Cargo
workspace.

**This file is loaded on every turn, so it holds rules, not arguments.** The
reasoning behind a load-bearing seam lives in the memory graph as
`seam:<slug>`; read it before changing a seam and write new reasoning there,
not here (the `seams` skill has the two calls).

```
crates/engine    (feral-processes-engine)   headless sim, standalone bevy_ecs
crates/app-core  (feral-processes-app-core) App/Mode input-and-flow state machine
crates/gui       (bevy + bevy_egui)         the renderer
crates/launcher  (feral-processes)          the binary (+ savetool, arena)
```

**Architectural rule:** the engine's `Game` struct (`crates/engine/src/lib.rs`)
is the entire API the renderer talks to via app-core; the renderer never
touches the ECS `World`. `Game.world` is private with no accessor — **never
add one**: `crates/gui` has the same `bevy_ecs` version via `bevy`, so a
`world_mut()` would be usable immediately with nothing to give pause. The
frontend is one big system rather than idiomatic Bevy components for this
reason (`crates/gui/src/lib.rs`). A graphical display is required; no text
mode, no headless play.

**The drawing seam:** `crates/gui/src/paint.rs` is the only file that names a
graphics library; `render/` draws only through its `Painter`. Full rule:
`.claude/rules/drawing-seam.md`.

## Load-bearing seams

One-sentence rules live in path-scoped `.claude/rules/*.md`, loaded when a
matching file is read. **Before changing code in a subsystem, read its rules
file if it has not loaded** (`ls .claude/rules/`). The trap behind each rule
is in the `seams` skill; the argument is `seam:<slug>` in the memory graph
(`memory_search(…, subsystem: "seams")` then `memory_get_entity`). A new seam
is three writes — graph, skill, rules file — in the order the skill
documents. Verify a rule against source before relying on it, and fix all
three places if it moved.

## Build & test

```sh
cargo test --workspace     # full gate
cargo run                  # the game
cargo clippy --workspace --all-targets   # --all-targets or test code is unlinted
cargo fmt

cargo run --bin savetool -- dump saves/save.bin s.ron   # edit a save; `pack` reverses
cargo run --bin savetool -- warp saves/save.bin 6       # runs the real breach
cargo run -- --template extraction                      # start from dev-saves/ (README lists them)
cargo run --bin savetool -- capture saves/save.bin <name>
cargo run --bin arena -- dev-arenas/opening-fight.ron [--out report.ron]
FERAL_DEV_ARENA=1 cargo run      # arena in the real battle UI; main menu [R]
cargo run -- --template stack --keys "Right Right" --screenshot out.png  # needs DISPLAY; Read the PNG
FERAL_DEV_REVEAL=1 cargo run -- --template stack   # whole Stack frame on the map
FERAL_DEV_NO_SIEGES=1 cargo run                      # the siege clock never runs
# Unset it before `cargo test`: an exported FERAL_DEV_NO_SIEGES=1 fails siege tests in the engine and launcher.
```

- Iterate with `cargo test -p feral-processes-engine <name>`.
- **Don't reach for a fresh `Game::new` when a `dev-saves/` template would
  do**; `capture` any state worth returning to.
- **`docs/measurements/` is what the instruments already said** — read it
  before running a sweep or arena batch; its `README.md` is the bar for
  adding one. Balance curves are excluded (`balance_sim.rs`'s job).
- Never delete `[profile.dev]` in the root `Cargo.toml` (debug playability;
  see `.claude/rules/launcher-and-build.md`).
- Mass `NotFound` test failures on an assets path = stale artifacts from the
  `petmud` rename: `cargo clean -p feral-processes-engine -p
  feral-processes-app-core`, never a full `cargo clean` (~3.5-minute cold
  rebuild).
- Disk: `git worktree remove` once a branch lands; sweep with `rm -rf
  target/debug/incremental`. Each worktree builds its own `target/`.
- Releases are manual per platform: `docs/releasing.md`.

## Moddability

The game must stay moddable: never hardcode content in Rust that can be data.
Species, structures, items, abilities, achievements, talent trees and help
pages are each a file in `assets/<kind>/`, schema in that directory's
`README.md`. Perks are half data and `tuning.rs` is deliberately code —
`.claude/rules/content-schema.md` has both, loaded with `assets/**`.

When a schema changes:

- New field on `SpeciesDef`/`StructureDef`/`ItemDef`/`AbilityDef` →
  `#[serde(default)]`, so existing and modded `.ron` files keep parsing.
- A malformed `.ron` is skipped with a logged warning, never a panic — follow
  the `*Db::load_dir` pattern.
- Update the matching `assets/*/README.md` in the same change.

## Code principles

- DRY but not prematurely; KISS/YAGNI; fail fast; follow existing naming.
- No backwards-compat cruft: no renamed-unused vars, `// removed` comments or
  shims. Delete what's unused.
- Comments explain *why*, never *what*.
- Don't assume — open the code and check.
- **A doc comment claiming to mirror other code must be a call, not a copy.**
  "Mirrors"/"same as" another formula → extract a pure function both call.
  Pattern: `battle::attackers_in_group`, `battle::slot_aggro_weight`,
  `battle::expected_damage`, `systems::node_payout`.
- Composition over inheritance; no global mutable state; named constants in
  `tuning.rs`; no optimization without evidence.

## Rust idioms

- `cargo fmt` + `cargo clippy --workspace --all-targets` after every change;
  fix warnings, don't silence them.
- `Result`/`?` over panics in engine code; `unwrap`/`expect` only in tests,
  true invariants, or startup config.
- Small functions over reflexive `.clone()`; explicit error types over
  `Box<dyn Error>`.

## Testing

- Sim logic gets unit tests; a bug fix starts with a failing reproducer.
- No flaky tests: no `sleep()`, no wall clock, no unseeded RNG; background
  systems (habitat spawning, nests) will interfere with naive assertions.
- `cargo test --workspace` is the final gate.
- **`balance_sim.rs` is the balance regression gate** — a deterministic
  battle simulator asserting level curves from live constants and real
  assets. Check `cargo test -p feral-processes-engine balance_sim` after any
  change to `tuning.rs`, a species or an item. A moved curve means
  progression changed.

## Process weight

Match process to blast radius:

- **One crate, no schema or save-format change, fits in one context** →
  brainstorm to a decision, then TDD inline, a commit per green step. No spec
  or plan file.
- **Two+ crates, a schema/save-format change, or wanted subagent isolation**
  → spec-and-plan pipeline. A plan carries file list, interfaces, test intent
  and gates — not finished code.

`docs/superpowers/INDEX.md` answers "what shipped, and where is its argument";
never answer from a spec's own `**Status:**` header. TDD and the full-suite
gate apply at every size.

## Running subagents

- Sonnet by default; opus for judgment (large reviews), not volume.
- Don't re-run the full suite to confirm a subagent's report; spot-check.
- Never brute-force a flake with repeated runs — read the code path.
- Per-task review gates are optional (ask the user first); the final
  whole-branch review is not.
- Give a diff as a file, never pasted into the prompt.

## Guardrails

- **Git:** commit freely at green states; branch first if on `main`. Push,
  force-push, `reset --hard` and amending pushed commits need an explicit ask.
- **One release per change landing on `main`:** bump the workspace version,
  add a `## X.Y.Z` section to `CHANGELOG.md`, annotated `vX.Y.Z` tag. Branch
  commits stay unversioned. `CHANGELOG.md`'s preamble decides the digit;
  "breaking" means a save stops loading (`save::SAVE_FORMAT_VERSION`). Push
  tags with `--follow-tags`.
- Surgical changes; no destructive shortcuts past an obstacle.
- Check what's at a path, branch or config before replacing it.
- Check nothing sensitive is staged.

## Working with the user

- State assumptions and decisions plainly.
- Ask a specific blocking question rather than guess; don't stall on
  decisions you can make.
- **Batch independent questions into one round trip** (overrides the
  brainstorming skill's one-question rule).
- Report what you verified, not what should work.
