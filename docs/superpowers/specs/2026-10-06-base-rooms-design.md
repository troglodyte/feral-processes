# Base rooms and doors — design

**Date:** 2026-10-06 · **Branch:** `base-rooms` · **Status:** spec, not built

## Goal

Give the base RimWorld-style rooms: enclosed spaces detected from the
grid, given a role by what stands in them, and graded for quality. Rooms
add morale depth (thoughts about where a body is and who shares it) and
reward laying out a good base (amenities and workshops work better in
good rooms).

This is sub-project 1 of 3. The later two each get their own spec:

1. **Doors and detected rooms** (this spec).
2. **Painted zones.** A box-select tool, reusing `Mode::Excavate`'s
   anchor-and-box interaction, to override a room's role or mark an
   area by intent. Stored state, so a save-format change.
3. **Logistics on zones.** Stockpile zones tied into hauling and
   depots; restricted zones in pathing.

New needs such as food (dining halls) are out of scope.

Decisions taken in brainstorming:

- Rooms are auto-detected, and painting comes later as the hybrid half.
- A new door structure separates a room from a corridor. Single-cell
  chokepoints do not.
- v1 roles: Quarters, Dormitory, Rec room, Workshop.
- v1 effects: a situational room thought, roommate thoughts, an amenity
  rate bonus and a workshop speed bonus.
- Room thoughts are situational: present while the body stands in the
  room. They are not memories.
- Holding Alt, which already reveals undiscovered routines, also shows
  rooms. Inspecting a structure names its room. There's no new key.

## Room model

### Derived, never stored

Rooms are recomputed whenever they are needed, by a pure function. There
is no room cache and no new `Resource`, for two reasons:

- `seams-base`: "a base-space cell's kind is derived, never stored".
- `Amenities`, `Occupancy` and `Bays` are already rebuilt every tick,
  because a new `Resource` shifts bevy query iteration order engine-wide.

A base runs from 69 floor cells at the start to about 3,600 at the
`HAUL_WALK_MAX_TILES` ceiling, so a flood fill per tick is cheap. Don't
optimise this without a measurement.

```rust
// crates/engine/src/rooms.rs (new module)
pub fn detect(grid: &BaseGrid, structures: &[PlacedStructure], defs: &RoomDb, home: (i32, i32)) -> Rooms;
```

`PlacedStructure` is the minimum detection needs: anchor cell, def id,
`barrier`, `door` and `room_tags`. The plan decides the exact shape. The
point is that `detect` takes no `World`, so it can be unit-tested on
literal grids.

### Doors

- `assets/structures/door.ron` is a new structure. A new `StructureDef`
  field, `door: bool` with `#[serde(default)]`, marks it.
- A door is not a `barrier`, so `base_step_blocked` lets the player
  through.
- `hauling::blocked_tiles` currently walls every structure's anchor.
  It must skip door anchors, so the crew paths through a door and
  `crew_reach` and `post_field` see past it.
- Doors stand on Floor, like every structure (`building.rs` placement
  rule).

### Cells and boundaries

- A room is a 4-connected set of walkable cells (`BaseCell::Open` or
  `Floor`) with no boundary on it.
- A boundary is rock (any absent cell), a `barrier` structure's anchor
  (for example `wall.ron`), or a door's anchor.
- A non-barrier structure's anchor is part of the room it stands in,
  and only its anchor counts. Footprint cells beyond the anchor are
  ordinary floor.
- The region containing the Home's anchor is the **commons**
  and never gets a role.
- A region whose contents match no `RoomDef` is roleless space.
  Neither the commons nor roleless space produces thoughts or bonuses.

### Roles (data)

- `StructureDef` gains `room_tags: Vec<String>`, with
  `#[serde(default)]`. Initial tags:
  - `defrag_bay`: `bed`
  - `sandbox`: `recreation`
  - every structure where `runs_a_job()` is true: `workshop`
- `assets/rooms/*.ron` holds one `RoomDef` each, loaded by a
  `RoomDb::load_dir` that follows the `*Db::load_dir` pattern (a
  malformed file is skipped with a warning). Fields:
  - `id`, `name`
  - `priority: i32`: the highest-priority matching def wins.
  - `requires: Vec<TagCount { tag, min, max: Option<u32> }>`
  - `living: bool`: roommate thoughts apply.
  - `tint`: a palette key for the Alt overlay.
- v1 files:
  - `quarters`: exactly 1 `bed`, living.
  - `dormitory`: 2 or more `bed`, living.
  - `rec_room`: 1 or more `recreation`.
  - `workshop`: 1 or more `workshop`.
  - Priority decides which wins when a room holds tags for several
    roles. Proposed order: quarters/dormitory, then rec_room, then
    workshop. The plan confirms it.

### Quality

`rooms::quality(room) -> f32` is a pure function of three inputs:

- area (cell count)
- the share of cells with a comfort-bearing floor finish
  (`FloorDef::comfort.is_some()`)
- crowding (structure anchors per cell)

The score maps to `RoomBand { Cramped, Plain, Fine, Superb }`. All weights
and band thresholds are named constants in `tuning.rs`.

## Effects

Each effect reads the band of the room it is in. Each has its own
constants in `tuning.rs`, and every numeric value here is a placeholder
for the plan to set.

### Room thoughts

- New `situations::Trigger` variants: `CrampedRoom`, `FineRoom`,
  `SuperbRoom`.
- One `assets/thoughts/*.ron` each, so intensity is data.
- Active while a body's position is inside a room with a role in that
  band. `Plain` gives nothing.
- `Surroundings` gains the detected rooms. `assess` stays pure.
- Extend `Trigger::ALL` and the test-only `index`.

### Roommate thoughts

- New triggers `RoommateRival` and `RoommateFriend`.
- They fire when the body and another program both stand in the same
  `living` room. The rival/friend test reuses the opinion and band
  logic behind `BesideRival` and `BesideFriend`, extracted into a
  shared function rather than copied (the "mirrors must be a call"
  rule).
- They may co-fire with `BesideRival`. `SITUATION_MAX_TOTAL` already
  caps the sum.

### Amenity rate

- In `needs_drain_system` (`systems.rs`, the refill loop),
  `rate × room_service_scale(band)` applies when the serving amenity's
  anchor is inside a room with a role.
- Placeholders: Cramped ×0.85, Plain ×1.0, Fine ×1.15, Superb ×1.3.

### Workshop speed

- `work_ticks_at_speed` gains a workshop scale next to `build_quality`.
  It applies when the structure's room role is `workshop`.
- `Task.required` is fixed when the job is posted. A room change
  therefore affects the next posted job, not one already running.
  That is accepted.
- The function's argument list is growing. The plan runs the
  design-patterns check on that signature before adding a fifth
  argument.

## Visibility

- `Game::view_rooms() -> Vec<RoomView>`, where `RoomView` holds the
  role name, band, cells and a label anchor cell. The renderer never
  sees the `World`.
- **Hold Alt** (`ALT_KEYS`, gui-only):
  - `draw_playing_base` tints each room cell by its role's palette key
    and labels each room once, for example "Fine dormitory".
  - Drawing goes through `Painter` only (the drawing seam).
  - Labels obey "gui text never clips or wraps".
- **Inspect:** `StructureReport` gains the room line, for example
  "In: Fine dormitory". It is omitted in the commons and in roleless
  space.

## Schema, saves, docs

- New fields `door` and `room_tags` are `#[serde(default)]`. The new
  `assets/rooms/` directory gets a `README.md`. Update
  `assets/structures/README.md`, the thoughts README for the five
  triggers, and add a help page for rooms and doors.
- `SAVE_FORMAT_VERSION` is unchanged. Rooms are derived, doors are
  ordinary structures, and the new def fields are asset-side.
- Add a CHANGELOG entry at landing.

## Testing

- `rooms::detect` on literal grids:
  - Rock encloses a room.
  - A door splits a corridor from a room.
  - A wall splits two rooms.
  - The commons is excluded.
  - Open cells count.
  - A non-door, non-barrier anchor stays inside its room.
- Role matching (priority, min/max) and quality bands, at each
  threshold.
- Pathing: the crew reaches through a door. A wall still blocks.
  `blocked_tiles` excludes door anchors.
- One test per effect, each mutation-checked:
  - The trigger fires in a band and not outside it.
  - Roommate rival fires in a dorm, not in a workshop.
  - The amenity rate is scaled.
  - Workshop ticks are scaled.
- Asset census: every `door`/`room_tags` value is read, and every
  RoomDef parses.
- `cargo test -p feral-processes-engine balance_sim` after the tuning
  constants land.
- Screenshot from a captured dev-save with a door and rooms, Alt held.
  Whether `--keys` can hold Alt is checked in the plan. If it can't,
  the plan adds a dev env flag rather than skipping the screenshot.
