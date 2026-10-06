# Floor items — plan

Spec: `docs/superpowers/specs/2026-10-05-floor-items-design.md`. Branch
`floor-items`. Read the spec once; this plan adds only what it left open:
re-located sites, decisions, task split, tests and gates.

**Execution:** five tasks, one sonnet subagent each, run serially (each uses
the last's types). Gates off per task; one opus whole-branch review at the
end. Each task: TDD (failing test first), commit per green step, `cargo fmt`,
`cargo clippy --workspace --all-targets`, then its listed tests. Full
`cargo test --workspace` at T2 end and T5 end only. Every dispatch: **never
push**; stage explicit paths only (`art/` is the user's untracked work —
never stage it); unset `FERAL_DEV_NO_SIEGES`. Engine paths below are under
`crates/engine/src/`; line numbers are at `9f70257b`.

## Decisions taken here

- **One core, two callers.** The drop is a free function
  `pub(crate) fn drop_load(world: &mut World, who: Entity)` in a new
  `game/base/floor.rs`; `Game::drop_load` wraps it. The stranded path runs
  inside `haul_step_system` (a system with `Commands`), so it calls
  `commands.queue(move |w: &mut World| drop_load(w, worker))` — confirm the
  method name against the workspace's `bevy_ecs` version (`queue`, older
  `add`). No marker component, no second system.
- **The table has seven paths, not six.** Re-located:

  | # | Path | Site | Now |
  |---|---|---|---|
  | 1 | Stranded past `STRANDED_SET_DOWN_TICKS` | `game/base/hauling.rs:1501–1537` | nearest-Depot set-down |
  | 2 | Post demolished | `game/base/building.rs:1326` | `remove::<(Task, Carrying)>` |
  | 3 | Post destroyed (`damage_structure`) | `game/base/upkeep.rs:1096` | same |
  | 4 | Siphon lock-in | `game/base/siphon.rs:70` | removed |
  | 5 | Study | `game/base/study.rs:315` | removed |
  | 6 | Reinforcement call-up | `game/reinforcement.rs:138` | removed |
  | 7 | Off-shift free (Downed) | `game/base/work_orders.rs:1489` | removed |
  | 7b | Re-matched to another job | `game/base/work_orders.rs:1503` | removed |

  The spec's "demolished" row conflated 2 and 3. 7b is new: a carrier stays
  in the pool (`is_on_shift`'s escape) so the matching can hand it another
  job, which destroys its load. Drop there too — one rule, no reasoning about
  reachability. Each site keeps its `Task` removal; only `Carrying` handling
  changes. Out of scope and untouched: `construction.rs:445,651` (builder),
  `combat_rewards.rs:1331` (besieger), the two delivery removals
  `hauling.rs:1286,1324`.
- **Path 1's `note_haul(…, "set_down", …)` record goes**; emit a
  `"drop"` record through the same `note_haul` only if a bench/telemetry test
  reads `set_down` (grep `"set_down"` first; delete the string if unread).
  `STRANDED_SET_DOWN_TICKS` keeps its name and value.
- **Pile lookup** is a linear query over `FloorPile` (piles are few, no index
  — no optimisation without evidence). `floor_pile_at(world, pos)` is the
  one finder.
- **Pickup errand reach:** `Errand::Pickup` destinations are pile entities,
  not structures, so `Errand::destination` / the walk must accept a
  non-`Structure` target. If the walk reads the target through the
  structures query, carry `at: Position` in the variant instead and walk to
  the tile. T3 decides by reading `haul_step_system` (`hauling.rs:974`); the
  variant still holds owned data only.
- **Examine:** `describe_base_rock` (`game/base_space.rs:317`) already names
  the first cell first (a finish). A pile on that cell is named before the
  finish, joined with "; ". No new app-core arm: app-core's `None =>` arm
  (`app-core/src/app/inspection.rs:104`) already shows the string.
- **View:** `Game::floor_piles(&mut self) -> Vec<FloorPileView { x, y }>`
  in `views.rs`, shaped like `marked_cells` (`base_space.rs:554`). Contents
  are not in the view; examine names them.

## T1 — data, drop core, save (engine)

Files: `components.rs` (`FloorPile`), new `game/base/floor.rs`
(`spawn_floor_pile`, `floor_pile_at`, `drop_load`, `take_from_pile`),
`game/base/mod.rs`, `save.rs` (`FloorPileSave`, `SaveData.floor_piles`
`#[serde(default)]`, add to the `Vec::new()` default at ~2266),
`game/lifecycle.rs` (write beside `StructureSave` ~2753, restore after
`restore_structures` ~1803), `game/inspection.rs:411`
(`stands_in_base_space` arm).

Interfaces: `take_from_pile(world, pile, item, qty) -> u32` takes up to
`qty`, despawns an emptied pile, returns 0 on a missing pile.

Then grep for queries over `&Position` with no type filter that a pile entity
would now match (occupancy, pathing blockers, targeting, siege) and add
`Without<FloorPile>` where it would misread one — list what was checked in
the commit message.

Tests (`tests/floor.rs` or beside `tests/hauling.rs`): drop makes a pile on
the carrier's tile and removes `Carrying`; two drops merge; drop with no
`Carrying` is a no-op; `take_from_pile` partial/whole/despawn/missing;
save→load keeps tile and contents (through `Game::save`/`load`, not RON
alone); an old save without the field loads.

## T2 — wire the seven paths (engine)

Files: the table's sites; `tests/hauling.rs` (the existing set-down tests
become drop tests).

Path 1: replace the Depot search and keep-it branch with the queued drop;
the `Stranded` timer and marker logic stay. Rewrite the doc comment above it
(the "set down, never destroyed" reasoning now points at the pile).

Tests: one per path (2–7b and 1): a carrier with a load put through the path
leaves a pile on its tile holding the load and no `Carrying`; the carrier
frees normally afterwards (path 1). Mutation-check: revert each call to the
old line, test must fail. Full suite at end.

## T3 — collecting (engine)

Files: `game/base/hauling.rs` (`Errand::Pickup`, derivation last in the
`None =>` arm after Tend/Collect/Load, arrival applies `take_from_pile` for
the pile's lowest `ItemId` up to `HAUL_CARRY_CAPACITY`, inserts `Carrying`),
reach through `post_field`/`crew_reach` (`hauling.rs:534,591`).

Tests: posted hauler with a reachable pile picks up and deposits it (pile
gone, Depot +qty); unreachable pile ignored; a hauler with any other errand
does not divert; two haulers to one pile — total conserved, no panic;
lowest `ItemId` first on a mixed pile. Use a `dev-saves/` template if one
fits (`chains`/`bench-economy`) rather than `Game::new`.

## T4 — display (engine, gui)

Files: `views.rs` (`FloorPileView`), `game/base/floor.rs` or
`game/inspection.rs` (`Game::floor_piles`), `game/base_space.rs`
(`describe_base_rock`), `crates/gui/src/render/base.rs` (draw under programs,
above floor, through `Painter` only; glyph + colour one named constant).
Read `.claude/rules/drawing-seam.md` first.

Tests: view row for a pile; none on a zone surface; examine text names
contents ("a pile on the floor: 3 scrap, 1 wire"), and with a finish on the
same cell names both. Screenshot check: `capture` a save holding a pile
(e.g. one built by a T2 test path via `savetool`), then
`cargo run -- --template <it> --screenshot out.png` and Read the PNG.

## T5 — seam, docs, measurement (no new code)

- New seam `drop_load` in the order the `seams` skill documents: graph
  (`seam:floor-pile-drop`), skill, `.claude/rules/seams-base.md`.
- `CHANGELOG.md` line under the unreleased section; no manual/README.
- `docs/superpowers/INDEX.md` row.
- `cargo test -p feral-processes-engine balance_sim` (must not move).
- Re-run the bench-economy baseline per
  `docs/measurements/2026-10-05-base-bench-economy-baseline.md` (same
  ticks/seeds, release); append whether staff measures moved and by how much.
- Full `cargo test --workspace`, clippy.

## Final

Opus whole-branch review (diff as a file). Fixes get their own review. Then
the user decides landing: merge → version bump → `## X.Y.Z` → tag →
`--follow-tags` push. `SAVE_FORMAT_VERSION` stays 34 (additive field).
