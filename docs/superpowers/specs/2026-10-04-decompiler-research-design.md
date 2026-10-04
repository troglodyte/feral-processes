# Decompiler research — range and area upgrades

**Status:** draft for review

## Goal

Research nodes that extend the `decompile` routine: first how far it can be
aimed on a battle map, then an area that rolls a capture against several
programs at once. Data-driven, so a mod can add or retune tiers.

## Decisions (agreed in brainstorming)

- Applies to **both combat models**. Range is battle-map only (the group
  model has no distances); area works in both.
- **One taming catalyst per program rolled.** Area saves turns, not
  catalysts.
- A **decompile-specific** research field, not a general routine-upgrade
  mechanism.
- **Four tiers**, each requiring the previous. All four need a study
  subject, the same as every shipped node at zone 2 or above and the zone-1
  benches.

## Schema

New optional field on `ResearchDef` (`crates/engine/src/research.rs`),
`#[serde(default)]`:

```ron
decompiler: Some((range: 4)),     // or
decompiler: Some((radius: 1)),
```

`DecompilerUpgrade { range: Option<u32>, radius: Option<u32> }`. A node
authoring neither field, or a value of 0, is skipped at load with a logged
warning (the `*Db::load_dir` pattern) — it would grant nothing.
`assets/research/README.md` documents the field in the same change.

No save-format change: researched nodes are already saved in
`resources::Research`, and the reach is derived from them.

## The one door

`Game::decompile_reach() -> DecompileReach { range: Option<u32>, radius: u32 }`
folds every researched node's `decompiler` by **max** per field. Every
reader of decompile's reach goes through it; nothing re-folds the research
set.

### Battle maps

The seams-tactical rule says `AbilityDef::tactical_shape`/`tactical_range`
is the one place an authored figure and the derived one are reconciled.
That stays true. Research is layered on top in a new Game-level door,
`Game::routine_tactical_shape(&def)` / `routine_tactical_range(&def)`, which
returns the def's figures unchanged for anything but `Decompile`. For
`Decompile`:

- range max = `max(def range max, reach.range)`
- shape = `Radius(reach.radius)` when `radius > 0`, else the def's shape

Every current reader of `tactical_range`/`tactical_shape` that can see a
decompile — the range refusal (`tactical/turn.rs` ~1126), the aim
highlight (`tactical/view.rs` ~504), the AI readers (`tactical/ai.rs`) —
calls the Game door instead. This is a seam change: the rules file, the
`seams` skill and `seam:` graph entry are updated together.

**Area resolution** (`run_tactical_routine`, `Decompile` arm). With a
radius, the candidates are the bodies inside `shape_cells` that are
`Hostile`, not `Summoned`, and visible from the aim. They are ordered by
distance to the aim (nearest first, ties broken by initiative order so
it's deterministic). Each one in turn goes through the existing single-body
capture (`decompile_body` / `decompile_squad`), so **one catalyst per
roll**. It stops when catalysts or roster room run out. A squad counts as
one body, priced as its lead (the existing squad rule). Friendly bodies are
never candidates. The aim refusal stays "the aimed cell holds a capturable
hostile"; with a radius, an aim with any capturable hostile in the blast is
legal.

### Group model

Range has no effect. With `reach.radius >= 1`, `attempt_decompile` rolls
against every program in the target group, front first (skipping cloaked
programs, as `front_of_group` does), each through `roll_decompile`, so one
catalyst each. It stops on running out of catalysts or roster room. A
radius above 1 adds nothing beyond a whole group. The battle ends if the
group or the fight empties, as it does today.

### Menu text

The research menu derives one cyan line per node from the field, beside
the existing unlock lines: "Decompile reach: 4" / "Decompile area: radius
1". The routine's picker description is unchanged.

## Content

| id | name | requires | min_zone | grants |
|---|---|---|---|---|
| `remote_decompile` | Remote Decompile | `routine_fabrication` | 1 | range 4 |
| `long_range_decompile` | Long-Range Decompile | `remote_decompile` | 2 | range 6 |
| `broadcast_decompile` | Broadcast Decompile | `long_range_decompile` | 3 | radius 1 |
| `wide_broadcast_decompile` | Wide Broadcast | `broadcast_decompile` | 4 | radius 2 |

Costs and materials are set in the plan from nearby nodes at the same zone.
All four set the study flag.

## Balance

Area taming lets a player capture a group instead of fighting it, which
pulls against "progression is earned by fighting". The guards are one
catalyst per program and the zone-3 gate. Capture chance is unchanged per
roll. `balance_sim` does not model taming; run it anyway as the gate.

## Testing

- `decompile_reach` folds by max, and with nothing researched it is
  `{ None, 0 }`.
- A malformed or empty `decompiler` field is skipped with a warning; the
  rest of the directory loads.
- Battle map: decompile at distance 4 is refused without the research and
  allowed with it; the aim highlight covers the extended range.
- Battle map, area: N hostiles in the blast with ≥ N catalysts roll N times
  and spend N catalysts; with fewer catalysts it stops early; with a full
  roster it stops; a friendly body in the blast is untouched.
- Group model, area: rolls each program in the group, one catalyst each, and
  stops on catalysts and on roster room. Without research, it still rolls
  the front program only.
- Census: every shipped `decompiler` node loads and the chain is reachable.
- Full gate: `cargo test --workspace`, clippy `--all-targets`, `balance_sim`.

## Out of scope

- General routine upgrades for other abilities.
- Radius 2 reaching other groups in the group model.
- Lowering the capture chance for area rolls.
