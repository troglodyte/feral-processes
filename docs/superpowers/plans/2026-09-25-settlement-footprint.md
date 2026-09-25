# Settlement footprints — plan

Spec: `docs/superpowers/specs/2026-09-25-settlement-footprint-design.md` (read it; this
file does not restate it). Branch `settlement-footprint`. Engine only, no save change,
no schema change.

Three phases, each ending green (`cargo test -p feral-processes-engine`, `cargo clippy
--workspace --all-targets`, `cargo fmt`) with a commit per green step. Each phase is one
sonnet dispatch, run serially (2 builds on 1's writer, 3 on 1's exclusion helper).
Per-task review gates off; one opus whole-branch review at the end. Full
`cargo test --workspace` once, after phase 3. Every new test is mutation-checked (delete
the line it guards, watch it fail, restore). Tests go in `crates/engine/src/tests/`
(fixtures in `tests/support.rs`; `tests/settlement_growth.rs` already drives growth).

Before each dispatch: invoke the `seams` skill for the subsystem (the ground; the Stack
for phase 2). Subagents must not push.

## Phase 1 — size, entities, the one writer

Files: `tuning.rs`, `components.rs`, `game/settlement_growth.rs`, `game/spawning.rs`
(`spawn_settlement_at` ~1324, `find_settlement_at` ~1251), `game/turn.rs:248`,
`game/settlement_patrol.rs` (`settlement_entity` ~132), `game/lifecycle.rs`
(`towns_by_tile` ~1731), `game/settlement_market.rs` (`settlement_reach`, last line),
`game/settlement_relations.rs` (`relay_landing` ~433), `game/outposts.rs`
(`found_outpost`).

- `tuning`: `SETTLEMENT_RADIUS_SERVER` / `_STARVED` / `_STEADY` / `_THRIVING` (1/1/2/3)
  and `SETTLEMENT_FOOTPRINT_MAX_RADIUS` as a `const` expression over them (not a
  restated 3).
- `components::SettlementCentre` marker.
- `Game::settlement_radius(key) -> Option<i32>`, exhaustive match on kind × vitality.
- `Game::footprint(key) -> impl Iterator<Item=(i32,i32)>` (or `Vec`) — the square, one
  derivation every reader below calls.
- `Game::sync_settlement_footprint(key)`: diff existing `Settlement { key }` entities
  against `footprint(key)`; spawn/despawn outer cells; repaint all glyphs; centre keeps
  `SettlementCentre`. Early return when cell count and centre glyph already match.
  Phase 1 leaves a `displace(cells)` call with an empty body — phase 2 fills it.
- `spawn_settlement_at` spawns the centre then calls sync; `announce_growth`'s repaint
  is replaced by a sync call; `turn.rs` calls sync for every known key after
  `settlement_growth_tick`.
- `settlement_entity` and `towns_by_tile` filter `With<SettlementCentre>` (towns_by_tile
  keys by tile, so an outer cell would shadow nothing — but filter anyway so it reads as
  one rule).
- `settlement_reach`: `<= radius + 1`. `relay_landing`: `ring_tiles` from band
  `radius + 1`. `found_outpost`: refuse within `SETTLEMENT_FOOTPRINT_MAX_RADIUS` of a
  known town tile — **before anything is spent** (per-refusal test).

Tests: spec §Testing rows Radius table, Materialization, Growth, Vitality, Outposts,
Trading reach, Patrols (shrink + save→load re-tether), Save/load.

## Phase 2 — displacement

Files: `game/settlement_growth.rs` (or a new `game/settlement_footprint.rs` if the
writer + displacement pass ~250 lines — ask the `design-patterns` question there, not
here), `game/stack.rs` (`collapse_stack` ~624, `ascend_to` ~1118),
`game/base_space.rs` (`move_anchor_to`), `game/caravan.rs`.

- One helper `Game::free_tile_outside(key) -> Option<(i32,i32)>`: `ring_tiles` outward
  from `radius + 1`, walkable, and none of wild creature / nest / surface link /
  settlement cell / trap / outpost. Built from `relay_landing`'s filter — extract that
  filter so both **call** it (CLAUDE.md "a mirror must be a call").
- `displace(cells)` handles each occupant per the spec table. Caravan rewrites
  `arrival_tile` too. Stack entrance: reuse `collapse_stack`'s despawn + `StackMemory`
  drop rather than copying it; log one line.
- **Deferral trap:** a Stack entrance skipped because the party is inside it sits under
  a cell that is no longer "newly covered", so sync's early return would strand it.
  Make the early return also require "no surface link inside the footprint" (≤49-cell
  check, cheap) so the next tick after surfacing relocates it. Test the surfacing half
  explicitly.
- Displacement draws no `GameRng` (world-gen rule); assert it.

Tests: spec §Testing Displacement, one per occupant, plus the deferral pair.

## Phase 3 — spawn exclusion and caravan spawn

Files: `game/spawning.rs` (`populate_chunk` ~1365, `spawn_wild_nearby` ~1672,
`scatter_open_tile` ~1929, `try_spawn_habitat_creature` ~1993), `game/stack.rs`
(`link_site_free` ~305), `game/settlement_patrol.rs` (`patrol_stand` ~301),
`game/caravan.rs` (`spawn_caravan` ~783, the walkable search).

- Each skips a tile where `find_settlement_at` answers. One predicate, called — don't
  inline the query six times.
- Watch the RNG stream: a skip that changes draw count shifts seeded tests
  (MEMORY: rng-stream-shift-exposes-seed-luck-tests). Probe before "fixing" one.

Tests: spec §Testing Spawning (seeded); caravan never spawns in a footprint.

## Close-out (inline, not dispatched)

- Seam, three writes (`seams` skill documents the order): graph argument, skill trap,
  CLAUDE.md rule. Also reword "the tile admits nobody" → "the footprint admits nobody"
  and the relay-landing "starts at band 1" rule.
- `CHANGELOG.md` entry text ready for the deploy (no bump on the branch).
- `cargo test --workspace`; opus whole-branch review with the diff as a file.
- Screenshot check (a template with a known city, `--screenshot`) — the footprint is the
  whole point and no test sees it drawn.
