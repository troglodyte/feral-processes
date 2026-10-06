# Base rooms and doors — plan

Spec: `docs/superpowers/specs/2026-10-06-base-rooms-design.md`. Branch
`base-rooms`. Read the spec once. This plan adds only what the spec left
open: the sites, the decisions, the task split, the tests and the gates.

**Execution:** six tasks, run serially with one sonnet subagent each, since
each task uses the previous task's types. Per-task review gates are off, and
one opus whole-branch review runs at the end.

Each task follows TDD: write the failing test first, commit at every green
step, then run `cargo fmt`, `cargo clippy --workspace --all-targets` and the
tests the task lists. Run the full `cargo test --workspace` at the end of T2
and of T6 only.

Every dispatch prompt says:

- **Never push.**
- Never `git checkout`/`stash` anything.
- Stage explicit paths only. `art/` is the user's untracked work, so never
  stage it.
- Unset `FERAL_DEV_NO_SIEGES`.

Engine paths below are under `crates/engine/src/`. Line numbers are at
`9c3b20f3`.

## Decisions taken here

- **Role priority.** The `priority` values are quarters 30, dormitory 30,
  rec_room 20 and workshop 10. Quarters and dormitory can't both match,
  because their `min`/`max` ranges don't overlap. A room holding a bed and a
  lathe is therefore quarters.
- **Detection signature.** The spec named the parameters, and this plan
  pins the exact types:

  ```rust
  pub struct PlacedStructure<'a> { pub at: (i32, i32), pub def: &'a StructureDef }
  pub fn detect(grid: &BaseGrid, placed: &[PlacedStructure], rooms: &RoomDb,
                floors: &FloorDb, home: (i32, i32)) -> Rooms;
  ```

  `FloorDb` is added to the spec's parameter list because quality needs
  `comfort`, and `detect` grades every room as it builds it.
- **`Rooms` shape.** `Rooms` holds `rooms: Vec<Room>` and
  `by_cell: HashMap<(i32,i32), usize>`. Each `Room` holds `cells`, `role:
  Option<RoomId>`, `living: bool` and `band: RoomBand`.
  - The commons and roleless space are kept, with `role: None`.
  - Queries: `room_at(x, y) -> Option<&Room>` and `same_room(a, b) -> bool`.
  - A boundary cell (rock, wall, door) appears in no room.
- **One builder for the world.** `rooms::of_world(world: &World) -> Rooms`
  gathers grid, structures, Dbs and home, then calls `detect`. Systems and
  `Game` methods both call it, and no other code assembles `detect`'s
  inputs.
  - Read `BaseGrid` and the Dbs the way `needs_drain_system` does
    (`systems.rs:104`).
  - The home anchor comes from the `HOME_STRUCTURE_ID` lookup that
    `home_position` uses (`game/zone.rs:601`). If a system can't take
    `&World`, add a `SystemParam`-free variant that takes the same pieces,
    and keep exactly one function that builds `PlacedStructure`s.
- **Doors in pathing.** `blocked_tiles` (`game/base/hauling.rs:243`) has no
  Db, so door anchors are filtered at its only non-test caller,
  `Game::blocked_tiles` (`game/zone.rs:445`), by reading `def.door`.
  - Then grep for other code that treats a structure anchor as a wall
    (`Occupancy`, `walls`, `is_structure_at`-style checks).
  - Exempt doors wherever such code governs crew walking, and list what was
    checked in the commit message.
  - `base_step_blocked` (`game/base_space.rs:1032`) already passes a
    non-barrier anchor, so the player needs no change there. Add a test
    anyway.
- **Workshop speed: no fifth argument.** This is the design-patterns check
  the spec asked for.
  - `work_ticks_at_speed(base, speed, class_scale, build_quality)`
    (`systems.rs:329`) already multiplies in per-site factors.
  - T3 reads the formula first. If `build_quality` and a room scale combine
    the same way, rename the parameter to `site_scale` and have
    `Game::work_ticks_for` / `cycle_ticks_for` (`game/base/building.rs:1418,
    1455`) pass `build_quality * room_work_scale(band)`.
  - If they combine differently, introduce `struct SiteScales { build_quality,
    room }` instead. A fifth bare `f64` is never the answer.
  - All posting goes through those two functions, so the four `Task.required`
    sites need no change.
- **Amenity scale by position.** `Amenities::nearest`
  (`game/base/offshift.rs:120`) returns the site's `Position`. The refill
  loop (`systems.rs:137–151`) multiplies the rate by
  `room_service_scale(rooms.room_at(site).filter(has role).band)`. That loop
  is the only place this scale is applied.
- **Shared bond test.** Extract `bond_with` and the `walks_the_base` filter
  (`situations.rs:~233, 260–264`) into `fn bond_band(…) -> bonds::Band`.
  `BesideRival`/`BesideFriend` and the two roommate triggers all call it.
- **`Surroundings` gains `rooms: &Rooms`.** Its only non-test builder is
  `assess_situation_system` (`situations.rs:362`). Test builders pass
  `&Rooms::default()`.
- **Tint is data, as RGB.** This deviates from the spec:
  - `RoomDef.tint: (u8, u8, u8)` replaces the spec's palette-key string.
  - The GUI has no string-to-colour palette mapper (only `const`s in
    `render/mod.rs`). An RGB triple stays moddable with no new mapper and no
    unknown-key failure mode.
  - The GUI draws the tint at a fixed alpha, set as one named const.
- **The Alt screenshot needs a flag.** `--keys` can't hold a modifier
  (`GameKey::parse_list`, `app-core/src/lib.rs:951`).
  - Add `FERAL_DEV_ALT=1`, which forces `reveal = true` at
    `gui/src/lib.rs:763`.
  - Copy the `FERAL_DEV_REVEAL` predicate (`game/stack_view.rs:34`).
- **Tuning values.** All of these are in `tuning.rs` and adjustable.
  - Quality: `area = min(cells / ROOM_AREA_FULL(16), 1)`, `finish = share of
    comfort cells`, and `crowd = max(0, anchors/cells − ROOM_CROWD_FREE(0.25))`.
    - `score = 0.5·area + 0.5·finish − 1.0·crowd`.
    - Bands: Cramped < 0.25 ≤ Plain < 0.6 ≤ Fine < 0.85 ≤ Superb.
    - So a bare 16-cell room is Plain, half-finished is Fine and fully
      finished is Superb.
  - Service scale: 0.85 / 1.0 / 1.15 / 1.3.
  - Work-tick scale: 1.1 / 1.0 / 0.92 / 0.85.
  - Thought intensity, in the asset files: CrampedRoom −2, FineRoom +1.5,
    SuperbRoom +3, RoommateRival −3, RoommateFriend +2.

## T1 — data and detection (engine)

Files:

- `structures.rs:550`: add `door: bool` and `room_tags: Vec<String>`, both
  `#[serde(default)]`.
- New `assets/structures/door.ron`. Copy a cheap non-barrier structure's
  cost and size, with footprint 1×1.
- `room_tags`:
  - `bed` on `defrag_bay`.
  - `recreation` on `sandbox`.
  - `workshop` on the 18 `runs_a_job()` ids. Re-derive the list from
    `structures.rs:698` and don't trust the grep list: annealing_node,
    armory, assembly_bay, cache_tap, compiler, decoy_bench, disk_press,
    fabricator, lathe, log_scraper, mining_node, power_conduit,
    refactor_bench, refinery, research_node, teardown_rig, transcriber,
    winding_node.
  - Drop any id that isn't a workshop in play terms (`power_conduit`,
    `cache_tap`, `mining_node` are candidates) and say which ones in the
    commit.
- New `rooms.rs`: `RoomDef`, `TagCount`, `RoomDb::load_dir` (copy
  `FloorDb::load_dir`, `floors.rs:121`), `RoomBand`, `PlacedStructure`,
  `Rooms`, `Room`, `detect`, `quality`, `band_of`, `of_world`.
- `lib.rs:43`: add `pub mod rooms;`.
- Load `RoomDb` as a `Resource` beside where `StructureDb::load_dir` is
  inserted (grep its caller).
- New `assets/rooms/` with `quarters.ron`, `dormitory.ron`, `rec_room.ron`,
  `workshop.ron` and a `README.md`.
- `tuning.rs` constants. `assets/structures/README.md` documents both new
  fields.

Tests in `rooms.rs` `#[cfg(test)]`, on literal grids. Use the base_grid
test builders (`base_grid.rs:193, 248`). Every spec "Testing → detect" case:

- Role priority, including that a bed plus a lathe gives quarters.
- min/max: 1 bed gives quarters, 2 give a dormitory.
- Each band at each threshold, from both sides.
- Asset census in `tests/assets.rs`: every RoomDef parses, every `room_tags`
  value is some RoomDef's tag, and `door` is set only on `door.ron`.

## T2 — doors in pathing, views (engine)

Files:

- `game/zone.rs:445`: filter door anchors.
- Any other wall-from-anchor site the grep finds.
- `views.rs`: `RoomView { role: String, band: RoomBand, cells, label_at }`
  and a `StructureReport.room: Option<String>` field, for example
  `"Fine dormitory"`.
- `Game::view_rooms(&mut self) -> Vec<RoomView>`, shaped like
  `floor_piles` (`game/base/floor.rs:84`). Only roles appear.
- `game/inspection.rs:1600`: fill the report field.

Tests (`tests/hauling.rs`):

- `crew_reach` passes a door and not a wall.
- `blocked_tiles` excludes a door anchor.
- `base_step_blocked` is false on a door.
- `view_rooms` for a walled room with a door and a bed.
- The report's room line, and its absence in the commons.

Mutation-check: remove the door filter, and the reach test must fail. Run
the full suite.

## T3 — amenity and workshop scales (engine)

Files: `systems.rs:137–151`, `systems.rs:329` (per the workshop decision),
`game/base/building.rs:1418–1460`.

Tests:

- A refill in a Superb room beats the same refill in the commons by the
  constant's ratio.
- `work_ticks_for` in a Fine workshop gives fewer ticks than in the commons,
  and the same structure in a quarters room gets no workshop scale.
- Mutation-check each one.
- Update the existing `systems.rs:2884–2934` tests only for the rename.

## T4 — situations (engine, gui)

Files:

- `situations.rs`: five `Trigger` variants, `ALL: [Trigger; 10]`, the test
  `index`, `bond_band` extraction, `Surroundings.rooms`, and the `assess`
  arms.
- Five `assets/thoughts/*.ron` files and `assets/thoughts/README.md`.
- `gui/src/render/party.rs:1380` reads `ALL`. Confirm that it compiles and
  that labels fit.

Triggers:

- A room thought fires only when the body's cell is in a room with a role
  and a non-Plain band.
- Roommate triggers fire when another Staff body is in the same `living`
  room, using `bond_band`.

Tests:

- Each band trigger fires in its band and not in Plain, the commons or
  roleless space.
- Rival fires in a dormitory and not in a workshop.
- The `BesideRival` tests stay green unchanged, which shows the extraction
  is behaviour-neutral.
- The census at `situations.rs:545` covers the new files.
- Mutation-check the band test and the roommate test.

## T5 — Alt overlay and inspect line (gui, app-core)

Read `.claude/rules/drawing-seam.md` first.

Files:

- `gui/src/render/base.rs:198`: in `draw_playing_base`, when `reveal` is
  set, tint each room cell with `RoomView.tint` at the named alpha, under
  structures and above the floor. Label each room once at `label_at`.
- Labels follow `gui-text-never-clips-or-wraps`. Drop a label that doesn't
  fit rather than clipping it.
- The report's room line goes in `render/building.rs:~1699`
  (`structure_headline` area).
- `gui/src/lib.rs:763`: add `FERAL_DEV_ALT`.

`RoomView` needs `tint`, so add it in T2 if T2 isn't done yet. Otherwise
add it here, in engine `views.rs`.

Screenshot:

1. Build a save from `bench-economy`: walls splitting off a room, a door,
   two `defrag_bay`, and a finish on half the cells.
2. `capture` it as `rooms`, and add a row to `dev-saves/README.md`.
3. Run `FERAL_DEV_ALT=1 cargo run -- --template rooms --screenshot out.png`
   and Read the PNG.

## T6 — docs, seam, gates (no new code)

- Help page `assets/help/` (next to `60-your-base.md`) covering rooms,
  doors, roles, bands and Alt.
- Seam, in the order the `seams` skill documents: graph
  `seam:rooms-derived` (rooms are derived per call, never a Resource, and a
  door is the only crew-passable anchor), then the skill, then
  `.claude/rules/seams-base.md`.
- A `CHANGELOG.md` unreleased line. No manual or README edits.
- A row in `docs/superpowers/INDEX.md`.
- `cargo test -p feral-processes-engine balance_sim` must not move.
- Full `cargo test --workspace` and clippy.

## Final

- Opus whole-branch review, with the diff passed as a file. Fixes get their
  own review.
- The user decides landing: merge, then version bump, then `## X.Y.Z`, then
  tag, then `--follow-tags` push.
- `SAVE_FORMAT_VERSION` stays 34, because the new fields are asset-side only.
