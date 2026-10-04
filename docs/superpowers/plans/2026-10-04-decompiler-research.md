# Plan: Decompiler research — range and area

Spec: `docs/superpowers/specs/2026-10-04-decompiler-research-design.md`.
Branch `decompiler-research`. All code is in `crates/engine` (plus
`crates/gui` only if the menu line needs drawing — check first; the engine's
`views.rs` likely already carries derived lines as strings).

TDD in every phase: failing test → code → green → `cargo fmt` + `cargo
clippy -p feral-processes-engine --all-targets` → commit. Iterate with
`cargo test -p feral-processes-engine <name>`. Never push.

Line numbers are from `52a8c368` and will drift; find by name.

## Phase 1 — schema, fold, loader (engine)

Files: `research.rs`, `game/unlocks.rs`, `tests/research.rs`,
`assets/research/README.md`.

- `research.rs`: `#[derive(Deserialize, Clone, Debug, Default, PartialEq)]
  pub struct DecompilerUpgrade { #[serde(default)] range: Option<u32>,
  #[serde(default)] radius: Option<u32> }`; `ResearchDef.decompiler:
  Option<DecompilerUpgrade>` with `#[serde(default)]`.
- `load_dir`: a node whose `decompiler` grants nothing (both `None` or
  both 0) is skipped with a warning; follow the existing `warnings.push`
  pattern.
- `game/unlocks.rs`: `pub struct DecompileReach { pub range: Option<u32>,
  pub radius: u32 }` and `Game::decompile_reach()`. It folds the researched
  base-tree nodes (via `is_researched`) by max per field.
- README: document the `decompiler:` field in the schema block. The menu
  line is derived, as the other lines are.

Tests: an empty set gives `{None, 0}`; two nodes fold by max; a malformed or
empty field warns and is skipped while its siblings still load.

## Phase 2 — group-model area (engine)

Files: `game/combat_rewards.rs` (`attempt_decompile`, `roll_decompile`),
a test module beside the existing decompile tests (find with `rg -l
attempt_decompile crates/engine/src/tests`).

- `attempt_decompile(group, player)`: when `decompile_reach().radius >= 1`,
  loop: front = `front_of_group(group)`; if there is none, or no
  `taming_catalyst()`, or `roster_room() == 0`, stop; otherwise run the
  existing single-program path (`decompile_body` + `remove_member`). On a
  failed roll the program stays at the front, so the loop needs a different
  stopping rule. **Iterate a snapshot of the group's members taken before
  the first roll**, not "the front until it's empty". Each program gets one
  roll. Re-check the battle-end path once, after the loop. With no research,
  the behaviour is byte-identical to today.
- Plan-time refusals (`ability_unavailable`) are unchanged: they need one
  catalyst and one roster slot.

Tests (seed the RNG and give a capture chance of 1.0 or 0.0 through an
existing test hook if one exists; otherwise check that catalysts were spent,
not who was captured):
- With radius research, N programs and ≥ N catalysts → N catalysts spent.
- With fewer catalysts → spending stops at 0, and nothing fizzles past it
  more than once.
- A full roster stops the loop.
- With no research → exactly 1 roll.
- A cloaked member is skipped.

## Phase 3 — battle maps (engine) + seam docs

Files: `game/unlocks.rs` (or `game/combat.rs`, next to `swing_range`),
`tactical/turn.rs`, `tactical/view.rs`, `tests/tactical.rs`,
`.claude/rules/seams-tactical.md`, the `seams` skill, and the
`seam:` graph entry.

- `Game::routine_tactical_range(&AbilityDef)` and
  `routine_tactical_shape(&AbilityDef)`. For any effect but `Decompile`,
  these return `def.tactical_range()`/`tactical_shape()`. For `Decompile`,
  range max = `max(def, reach.range)`, and shape = `Radius(reach.radius)` when
  `radius > 0`.
- Switch every reader that can see a decompile:
  - `turn.rs` range refusal (~1126), `aim_in_sight` (~1139), the Single
    check (~1182) and `run_tactical_routine`'s shape (~1447).
  - `view.rs` ~471/504/508.
  - Check `ai.rs` 98/1369. If a party body can never be AI-driven into a
    decompile, leave it and say why in the commit message.
  - Leave `tamper.rs`, `items_db.rs` and `combat_round.rs:1565` alone; they
    never see a decompile.
- Aim refusal (~1154): with radius > 0, it is legal when the blast contains
  ≥ 1 capturable hostile (`Hostile`, not `Summoned`, visible from the aim),
  rather than requiring one on the aim cell.
- Resolution (`run_tactical_routine` Decompile arm, ~1347). With radius,
  the candidates are capturable hostiles whose cells (`cells_of`) meet
  `shape_cells`. They are sorted by distance to the aim, then initiative
  order. For each one: stop on no catalyst or `roster_room() == 0`, then
  run the existing `decompile_squad`/`decompile_body` + `TacticalBattle::remove`.
  Squads count once. Stop if the fight ends mid-loop (`provoke`-style
  "still on the board" check). Update the comment at ~1342, which says area
  is "a different mechanic".
- Seam docs, in the order the `seams` skill documents: the graph, then the
  skill, then the rules line. "`AbilityDef::tactical_*` reconciles authored
  and derived; `Game::routine_tactical_*` is the one door that layers
  research on top, and every reader that can see a decompile calls it."

Tests:
- A decompile aimed at distance 4 is refused without research and accepted
  with range-4 research.
- The view highlight includes distance-4 cells with the research.
- With radius 1, a blast over 2 hostiles and 1 friendly spends 2 catalysts
  and leaves the friendly untouched.
- 1 catalyst means 1 roll.
- A full roster stops the loop.
- With radius research, an aim at an empty cell next to a hostile is legal.
- A squad in the blast counts once.

## Phase 4 — content, menu line, census, release prep

Files: four `assets/research/*.ron`, `views.rs` (research row lines),
`tests/assets.rs` (census), `CHANGELOG.md` (an Unreleased note only; the
version is bumped at landing).

| file | requires | min_zone | cost | materials | grants |
|---|---|---|---|---|---|
| `remote_decompile` | `routine_fabrication` | — (1) | 30 | bytecode_block 12, charge_coil 6 | range 4 |
| `long_range_decompile` | `remote_decompile` | 2 | 55 | bytecode_block 16, logic_wafer 8 | range 6 |
| `broadcast_decompile` | `long_range_decompile` | 3 | 120 | cache_grain 8, logic_wafer 12 | radius 1 |
| `wide_broadcast_decompile` | `broadcast_decompile` | 4 | 220 | cache_grain 14, charge_coil 16 | radius 2 |

All four set `requires_subject: true`. Descriptions say what each node is,
in plain language, with no "tick" wording; the derived line covers the
numbers.

- `views.rs`: one cyan line per node, built from `decompiler`: "Decompile
  reach: N" / "Decompile area: radius N". Test it beside the existing
  derived-line tests.
- Census in `tests/assets.rs`: every shipped `decompiler` node loads, the
  chain is reachable from `routine_fabrication`, and ranges and radii
  strictly increase along the chain.

## Gates (end of branch)

1. `cargo test --workspace` (use a file for the exit code, not a pipe).
2. `cargo clippy --workspace --all-targets`, `cargo fmt --check`.
3. `cargo test -p feral-processes-engine balance_sim`: expect no change.
4. `cargo run -- --template stack --screenshot` of the research menu showing a
   new node; Read the PNG.
5. A final whole-branch review on opus, with the diff passed as a file.

## Execution

Phases 1→2→3 are sequential (2 and 3 need `decompile_reach`); 4 needs 1.
For token efficiency:
- One sonnet subagent runs Phases 1+2. Then 3 and 4 run in parallel as two
  sonnet subagents. They touch disjoint files except `tests/`; stage
  explicit paths only.
- Per-phase review gates are off unless asked; the final opus review is
  mandatory.
