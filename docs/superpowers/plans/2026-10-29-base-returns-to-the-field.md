# Plan: the base returns to the field

Spec: `docs/superpowers/specs/2026-10-29-base-returns-to-the-field-design.md`. ADR: `docs/adr/0032-base-returns-to-the-field.md`. Branch `base-in-field`.

**Gate 0: ADR 0032 is Proposed. No code until the user agrees it.** On their OK, set it to Agreed in the ADR and in `docs/adr/README.md`.

Each phase lands green and runs in its own context: start it from this file plus the spec, not from an earlier phase's history. Commit at each green step. Never `git checkout` or `git stash` uncommitted work, never push. Paths are under `crates/engine/src/` (written `E/`) unless they say otherwise. Line numbers come from a code survey on 2026-10-29, so treat them as hints and re-check them.

## Resolved open questions

1. **Mining reach.** A field blocker can be mined only when it is orthogonally next to laid floor, so the base digs outward. Crew: the cell needs a dig mark and a floor request (the floor request is allowed by decision 3's adjacency rule; decision 4 refuses it only until the cell is mined, so a floor request on an *adjacent marked* blocker is accepted and waits for the dig). Player bump with `MiningMode`: the same adjacency rule.
2. **What a dug cell becomes.** It gets a `Biome::Excavated` override (that variant already exists), and laying the queued floor then makes it base floor. No new biome. (The user didn't recognise the question as worded; this answer follows from answer 1.)
3. **Re-tier.** Stack links, settlements and outposts stay as they are. Nests spawn at the new tier.
4. **Sealed base.** If no walkable cell touches the edge, besiegers spawn at the nearest walkable cells outside the floor (a BFS from the edge, sorted by coordinate). Besiegers that dig are left for a later iteration.
5. **Pending floor counts as adjacency.** Yes, as the spec says.

## What the survey changed

- **Phase 1 is mostly done already.** `enter_next_zone` (`E/game/zone.rs:815`) keeps the map; the world is minted once in `Game::new`. What's missing is the "entered dungeons keep their tier" half. Today a breach wipes `StackMemory` (`resources.rs:2269`), and `FrameSpec` folds in the current tier, so every link re-carves at the new tier.
- **`balance_sim` doesn't read build costs.** The re-cost is checked against the class kits (`assets/classes/*.ron`; the smallest kit has `core_fragment` 4), against `tests/classes.rs` and `tests/kit.rs`, and against the `bench-economy` dev-save. `balance_sim` still runs as the regression gate.

## Phase 1: entered dungeons keep their tier (engine only)

Files: `E/game/zone.rs`, `E/resources.rs` (`StackMemory`/`FrameMemory`), `E/stack.rs` (`FrameSpec`), `E/game/stack.rs`, `E/save.rs`, `E/tests/zone.rs`, `E/tests/stack.rs`.

1. Test first, on populated ground (see the `populated-chunks-clear-is-half-a-re-tier` memory). After a breach:
   - the map and the tile overrides are unchanged
   - a link the player had entered re-carves at its old tier
   - a link they had not entered carves at the new tier
   - nests spawned after the breach use the new tier
2. Record a per-link `entered_tier: Option<u32>` when the player first enters a frame. Prefer a field on a per-link record that outlives the `StackMemory` wipe over keeping all of `StackMemory`. The `FrameSpec` tier input reads `entered_tier.unwrap_or(zone)`.
3. Save the new field, with a save→load test. The format bump itself waits for Phase 5.

Gate: `cargo test -p feral-processes-engine zone stack`, then clippy.

## Phase 2: field mining (engine only)

Files: `E/game/base_space.rs` (`strike_rock` 461, `bump_rock` 1137, `wall_at` 411, `describe_base_rock` 318), `E/rock.rs` (`kind_at` 215), `E/world.rs` (`set_override` 281), `E/game/inspection.rs` (`view_tiles_at` 70-110), `E/tuning/base.rs`, `E/tests/` (a new `field_mining.rs`).

1. **Rock kind from the biome.** `RockDb::kind_at` takes the field `Biome` in place of the `BaseGrid` seed. Proposed mapping: DataVoid → compacted, BlackIce → fused, Entropy → entropy. Check it against `assets/rock/*.ron` before settling. Update `assets/rock/README.md`.
2. **`strike_rock` on field coordinates.** It breaks a field blocker into a `Biome::Excavated` override through `WorldMap::set_override`. Overrides are already saved (`save.rs:1722`). Rename `BASE_MINE_FRAGMENT_CHANCE` to `ROCK_FRAGMENT_CHANCE` and keep its value and its RNG draw.
3. **The adjacency gate** (resolved question 1), as a pure `fn minable(cell, floor: &BaseGrid, map) -> bool`.

   Tests:
   - bumping a blocker next to floor mines it, and the override survives save→load
   - bumping a blocker that isn't next to floor is refused
   - the fragment drop is deterministic under a fixed seed
   - a blocker that is marked and floor-requested gets dug and then floored by the crew

   Until Phase 3, the floor still lives in base space, so this phase may land only the pure pieces (`kind_at` and `minable`) plus a field `strike_rock` behind its tests. Wire the bump and the crew in Phase 3.

Gate: `cargo test -p feral-processes-engine`, then clippy.

## Phase 3: the base moves onto the field (engine, app-core, gui)

This is the big one, about 70 files. Do it in the order below. The suite will be red between steps 3a and 3c, so commit at 3c and not before. If one context can't hold it, split it by test file across sonnet subagents (in a worktree, with the dispatch prompt forbidding checkout, stash and push).

Before starting, read `.claude/rules/seams-base.md` and `seams-sieges.md` and the `seams` skill. Run `memory_search(..., subsystem: "seams")` for the base and siege seams.

- **3a. The overlay.**
  - `BaseGrid` (`E/base_grid.rs:49`) keeps only `Floor` cells, keyed by field coordinates, plus `finishes` and `stockpiles`. Drop `Open` (mined ground now lives in the `WorldMap` override) and `seed`.
  - `BaseGrid::walkable(x,y)` becomes "the field tile is walkable".
  - `view_tiles_at` merges floor cells as `Biome::Platform` over the `WorldMap`, with no `base_pos()` branch.
- **3b. Delete base space.**
  - `enter_base`/`leave_base` (`base_space.rs:137/174`), `BASE_EXIT_CELL` and its 7 users
  - `lay_starting_pocket` and the pocket constants
  - `Locale::Base` (`resources.rs:1972`) and `in_base`, `base_pos`, `in_base_space` and `restore_locale` for that arm
  - `stands_in_base_space` (`inspection.rs:417`)
  - `require_base`/`require_surface` (`E/game/stack.rs:446/464`). Each of their roughly 35 call sites becomes "not in the Stack" or, where the base must exist, "anchor exists". Decide per site, and take the guidance from the `require-surface-used-to-mean-not-in-the-stack` seam.
  - `move_in_base` merges into `move_player`, with mining (Phase 2) on the bump.
  - Dig marks, `run_dig_crew`, `crew_lays_tile`, `floor_cell` and `place_wall_box` keep working on field coordinates; this changes their types, not their logic.
- **3c. Sieges** (decision 6 and resolved question 4).
  - Delete the `board::build` flood fill from `BASE_EXIT_CELL`.
  - Add a pure `fn siege_spawn_cells(floor, map) -> Vec<(i32,i32)>`: walkable cells orthogonally next to edge floor, falling back to a BFS for the nearest walkable cells, sorted by coordinate.
  - `spawn_siege_pack` draws from that list.
  - Remove `player_cell` (`save.rs:467`, `siege/persist.rs`) and the old-save migration test at `tests/siege.rs:3904`.
  - Keep `besieger_walk_toward`.
- **3d. Placement conflicts.**
  - Floor is refused on a Stack link, a settlement footprint or an outpost cell.
  - Settlement placement (`settlement_footprint.rs`, `standable_near`) skips base floor.
  - Delete `displace_anchor_at` (`settlement_footprint.rs:~360-388`), the `move_anchor_to` settlement caller.
  - The outpost minimum distance (`outposts.rs:46`), `raiding_towns` and `town_garrisons` (`upkeep.rs:330/367`) measure to the nearest floor cell, through one pure `fn distance_to_floor`.
- **3e. app-core and gui.**
  - Remove the enter/leave base flow (`app/playing.rs`, `app/lifecycle.rs`, `map_keybar.rs`, `travel.rs`, `group_menu.rs`).
  - The excavate, wall box, dig and floor order modes now target field cells.
  - `gui/src/render/terrain.rs:203-285` and `render/base.rs` draw one space.
- **3f. Seams.** Rewrite `seams-base.md` and `seams-sieges.md`, their `seam:` graph entries and the `seams` skill references, in the order the skill gives. Delete the anchor-pin trap.

Tests:
- Port `tests/base_space.rs` (166 tests), `building.rs`, `siege.rs`, `nemesis_siege.rs`, `hauling.rs`, `work_orders.rs`, `floor.rs` and `wall_box.rs` to field coordinates. Delete the tests whose subject is gone (enter/leave and the pin) instead of porting them.
- New tests:
  - siege spawn cells lie on the edge and are deterministic, and the BFS fallback works on a sealed base
  - floor is refused on a link, a settlement or an outpost
  - the outpost distance is measured to the footprint

Gate: `cargo test --workspace` (with `FERAL_DEV_NO_SIEGES` unset), `cargo clippy --workspace --all-targets`.

## Phase 4: founding and growth (engine and assets)

Files: `E/game/base/building.rs` (`place_structure` 85, `PlacementRefusal` 29, floor press 1180), `E/game/base/work_orders.rs` (`drop_dry_dig_wants` 2372), `E/tuning/base.rs`, `assets/structures/*.ron`, `assets/structures/README.md`, and tests in `building.rs` and `classes.rs`.

1. **Founding** (decision 2), test first.
   - `HOME_FOUNDING_COST` goes in `tuning/base.rs` and is paid in code. Don't put it in `home.ron`: that would be a new `.ron` field and need an ADR.
   - Founding floors the 8 ring cells. It is refused if any ring cell isn't walkable, and still refused on a link.
2. **Refusals.** Add `PlacementRefusal::NotAdjacent` and `NeedsMining`, each with a `message()`. A floor request on an adjacent blocker that carries a dig mark is accepted (resolved question 1).
3. **Re-cost.** Proposed numbers, to confirm with the user at the start of this phase:

   | What | Today | Proposed |
   |---|---|---|
   | `HOME_FOUNDING_COST` | free | 2 |
   | `FLOOR_TILE_COST` | 2 | 1 |
   | extractor | 12 | 6 |
   | depot | 12 | 8 |
   | research_node | 10 | 8 |
   | recharger_node | 10 | 8 |

   - The smallest kit (4) minus founding (2) leaves 2. The extractor then needs about 4 fights' worth of fragments, or mined rock (0.25 per cell).
   - Add a test that every class kit covers `HOME_FOUNDING_COST`.
   - Run `balance_sim`, and run `bench-economy` before and after.

Gate: `cargo test -p feral-processes-engine building classes kit balance_sim`, then the workspace gate.

## Phase 5: release

1. Bump `SAVE_FORMAT_VERSION` from 39 to 40 (`E/save.rs:2196`) and add a line to the history comment.
2. Check that every new field has a save→load test: the floor overlay, `entered_tier` and the overrides.
3. Recapture all 22 `dev-saves/` templates. Note it in `dev-saves/README.md`.
4. Take a `--screenshot` of a newly founded 3x3 base from a recaptured template and Read the PNG.
5. Add a `CHANGELOG.md` entry: a breaking minor release.
6. Run a final opus review of the whole branch diff, given as a file.

Deploy happens separately, through the `deploy` skill.

## Not in scope

Besiegers that dig, mining away from the base, a new "cleared" biome, and migrating old saves.
