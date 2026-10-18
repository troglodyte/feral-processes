# Plan: held-output dot, and idle staff who clear a machine

Spec: `docs/superpowers/specs/2026-10-18-held-output-mark-and-idle-clearing-design.md`.
Branch `held-output-mark`, primary checkout. TDD per step, commit per green
step, mutation-check each engine test by hand (fix removed → test fails).
Gate per phase: `cargo fmt`, `cargo clippy --workspace --all-targets`, the
named tests. Final gate: `cargo test --workspace` (a `-p` run is
single-threaded; the workspace run is not). Rules already read:
`seams-base.md`, `seams-hud.md`, `seams-instrumentation.md`; read
`drawing-seam.md` before Phase 2.

Source names below were verified against the tree on 2026-10-18.

## Phase 1: the dot, engine side

Files: `crates/engine/src/views.rs` (~1277), `game/inspection.rs` (~1416,
~1518), `tests/inspection.rs`; `EntityView` literals in
`crates/gui/src/render/base.rs` (~2177) and `render/building.rs` (~2153).

1. `EntityView::holds_output: bool` beside `output_stranded`, doc per spec.
   Fill in `inspection.rs` from the same `Stock` read: `is_structure`, def
   does not `stores`, `Stock::output_used() > 0`. Both gui literals get
   `holds_output: false`.
2. Tests (`tests/inspection.rs`): one unit in `Stock::output` → true; same
   unit in `Stock::input` only → false; Depot holding goods → false; empty
   buffer → false.

## Phase 2: the dot, gui side

Files: `crates/gui/src/render/marks.rs` (constants ~61, `nemesis_mark_rect`
~110, `depot_fill_rect` ~444, `staffed_mark_rect` ~506), `render/base.rs`
(`draw_surface_map`, mark site ~959; mark tests ~5790-6072).

1. `const HELD_OUTPUT_MARK` in `marks.rs`, 0.14-0.16 of the tile (builder
   picks; must be `< IDENTITY_MARK`). `pub(super) fn held_output_rect(px,
   py, tile_px) -> Rect`: `x = px + IDENTITY_MARK_INSET`, `y = py +
   RARITY_BAR_PX + IDENTITY_MARK_INSET` (`RARITY_BAR_PX` is in
   `render/mod.rs:218`), square of `HELD_OUTPUT_MARK * tile_px`.
2. `draw_surface_map`: after `outline_open` and `draw_depot_fill`, gated on
   `structure.is_some_and(|ev| ev.holds_output)`, `painter.rect` in the
   cell's own hue, alpha 1.0, no bob or blink.
3. Tests (headless `with_painter`, as the depot-fill tests): `holds_output:
   true` paints exactly one rect in the top-left quadrant below
   `RARITY_BAR_PX` and left of the tile midpoint; `false` paints nothing
   there. Census: `held_output_rect` disjoint from `staffed_mark_rect` at
   every bob lift, `depot_fill_rect`, `progress_bar_rect` and the rarity
   bar; add beside `the_nemesis_mark_never_overlaps_the_rarity_bar`.
4. Real launch: `cargo run -- --template <base template> --screenshot
   out.png`, Read the PNG; switch to `poly` only if the square reads badly
   at the smallest `map_cell` rung.
5. `CHANGELOG.md` line for the dot (unversioned; bump is the landing step).

## Phase 3: idle clearing (engine, base labour seam)

Files: `crates/engine/src/game/base/tidy.rs` (`TidyBoard` 24, `tidy_board`
37, `may_tidy` 72, `tidy_step` 92, `set_down_posted_tidiers` 158),
`game/base/work_orders.rs` (`drift_idle_staff` 2483; tidy arm ~2664, board
built ~2552), `game/base/hauling.rs` (`take_haul_load` 35,
`depot_beside_accepting` 165 — currently private, make `pub(crate)`,
`nearest_depot` 201, `crew_reach` 616, `reaches` 644), `game/base/floor.rs`
(`spawn_floor_item` 25, `drop_load` 214), `game/base/depot_filter.rs`
(`depot_accepts` 62), `components.rs` (`Tidying` 722, `Carrying` 956),
`save.rs` (~762 `tidying`, ~2570), `tuning.rs` only if a magic number
appears. New module `game/base/clearing.rs` for the board and the step, in
`tidy.rs`'s shape. Tests: `tests/stockpiles.rs` (tidy tests live there),
`tests/save_roundtrip.rs`.

1. **Design-patterns dialog first** (required by the spec): the marker
   shape. Load is `Carrying` (the one load state); marker is a new
   component `Clearing { to: ClearTarget }` or similar, saved like
   `Tidying`. Constraints: `may_tidy`'s "holding a hauler's load is mid-trip
   for somebody else" reading survives; no second load state; every
   existing `drop_load` path covers it unchanged.
2. Reproducer first (fails before the change): machine with finished
   output, every Depot full, stockpile with free cells, one idle program →
   after enough beats the output is on stockpile tiles, buffer empty.
   Drive with `drift_idle_staff_for_test` (2762) as the tidy tests do.
3. `clearing_board`: sources = structures with `Stock`, def not `stores`,
   `output_used() > 0`, minus any with `depot_beside_accepting` with room;
   Depot targets = `stores` with `output_room() > 0`, per item through
   `depot_accepts`; stockpile cells = the free cells `tidy_board` already
   computes (share the computation, do not copy). `None` when there is no
   Depot and no stockpile cell. Share `may_tidy`'s gates with clearing by
   extracting, not copying.
4. `clear_step`: nearest reachable source (Chebyshev, ties `(x, y)`,
   `crew_reach`/`reaches`). Destination **before** pickup: `nearest_depot`
   with room accepting the item, else stockpile with a free reachable cell,
   else skip the source. Take `min(output, HAUL_CARRY_CAPACITY, room)` via
   `take_haul_load`; load is `Carrying` plus the marker. Deliver through
   the existing Depot deposit door, or `spawn_floor_item` one unit per free
   cell nearest the body, re-checking `free` on arrival. Destination gone
   mid-walk → fall back once to the other kind; both gone → `drop_load`
   where standing. One item kind per trip.
5. Wire into `drift_idle_staff` below every claiming errand and above the
   tidy arm (~2664); extend `set_down_posted_tidiers` to set a clearer's
   load down on post. Save: marker field behind `#[serde(default)]` next
   to `tidying` in `save.rs`, restored on load.
6. Tests, each mutation-checked: Depot with room beats stockpile; neither
   has room → `Stock::output` unchanged after many beats; machine beside
   an accepting Depot with room is not a source; clearing outranks tidy
   (loose floor item and a machine both waiting → body goes to the
   machine); posted mid-trip → load on the floor unit for unit; no Depot
   and no stockpile → no `Carrying` ever on an idle body; save-then-load
   with a clearer mid-trip (`tests/save_roundtrip.rs`). No `Record::Haul`:
   the instrumentation seam keys hauls to a post and a `Tend` writes
   nothing, and a clearing trip has no post.

## Phase 4: docs and seams

1. `assets/help/60-your-base.md` ~line 35: one plain sentence after the
   stockpile paragraph — idle staff move finished product out of a machine
   into a Depot or onto the stockpile when there is room.
2. Seam, three writes in the `seams` skill's order: memory graph
   `seam:<slug>` (the argument), the `seams` skill (the trap), one line in
   `.claude/rules/seams-base.md` (rank above tidy; destination before
   pickup; nothing picked up with nowhere to go).
3. `CHANGELOG.md` line for clearing. `docs/superpowers/INDEX.md` row at
   landing, not before.
