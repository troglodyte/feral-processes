# Stack corridor lighting

**Date:** 2026-10-14 · **Part 2 of map lighting** (part 1: battle maps,
v0.18.28, `crates/gui/src/render/lighting.rs`)

## Intent

Every Stack frame is drawn in the same flat cyan today; the only shading is
`stack::shade`'s distance fog. Give each frame its own light, as part 1 did
for battle boards.

Settled with the user (part 1 answers carried over, part 2 answers below):

- Vary all four: light direction, colour cast, strength, shadows.
- Seeded per frame — stable across steps, turns and reloads.
- Visual only; no gameplay effect.
- The light comes from a fixed **world** direction per frame, so one side
  wall is lit and the other shaded, and turning swaps them — an orientation
  cue as well as a look.
- The seed crosses the boundary **raw**; the engine knows nothing about
  colour. (Rejected: engine computes the light — puts look decisions in the
  engine; gui-only fold of the cone — the cone changes every step, so the
  light would flicker.)
- Shadows are **floor shadows thrown by solid neighbours inside the cone**,
  not battle-style long shadows.
- **Corridor only**: the frame map and its inset stay unlit — the map is a
  reading, like the battle washes.

## Engine (`crates/engine`)

- `views::StackView` gains `light_seed: u64` — the frame's lighting seed.
- Derived in `game/stack_view.rs` as
  `self.frame_spec(pos.depth, pos.frames, pos.entrance).salted(&[LIGHT_SALT])`,
  a new named salt constant beside `stack_view`. This follows the Stack seam
  rule "World generation must not draw from `GameRng`" and the description
  scheme's one-fold rule; nothing is stored, nothing is saved.
- `StackView.facing` becomes `stack::Dir` (was `Dir::label()`'s
  `&'static str`). The renderer needs the real direction to light by world
  axes; its two readers (`render/stack.rs`'s heading and
  `app-core/src/tests/stack.rs`) call `.label()`. `FrameMapView.facing` is a
  separate field and stays a label. No shim, no second field.
- No save-format change, no schema change, no `tuning.rs` change.

## Gui (`crates/gui/src/render`)

### Sharing the seed→light step

`lighting.rs::light_from_seed` becomes `pub(super)`, and `Light`/`Lit` are
already `pub(super)`. The Stack draws its `Light` from that one function —
no second copy of the casts, strength range or Lemire pick. The battle-only
parts (`Lighting`, `levels`, `board_light`) stay as they are. `LEVEL_MIN`/
`LEVEL_MAX` are reused as the Stack's clamp band.

### Lighting the corridor (new `render/stack_light.rs`, called from `stack.rs`)

`stack.rs` is 1 038 lines; the lighting maths goes in its own file with its
own tests, and `stack.rs` only asks it for a `Lit` per surface.

World axes match the battle board: North is `(0, -1)`, y grows down, so
`Light::toward` is used unchanged. The party's basis is `facing.delta()`
(forward, `f`) and `facing.right_delta()` (right, `r`).

Each surface's level is `1 + span · strength · dot(n, toward)`, clamped to
`[LEVEL_MIN, LEVEL_MAX]`, where `n` is the unit world normal pointing out of
the surface into open space:

| Surface | Normal `n` |
|---|---|
| Left flank wall of an open cell | `+r` (it faces right, into the corridor) |
| Right flank wall | `-r` |
| Face ahead (rock, door, sealed door) | `-f` (faces the party) |

Floor and ceiling take a gentle gradient instead: `n` is replaced by the
cell's world offset from the party (`lateral·r + depth·f`) over the cone's
reach, at a smaller span, so the floor brightens toward the light's side.

**Floor shadow.** For an open cell, each orthogonal neighbour inside the
cone (left/right in the same row, ahead in the next row, behind in the
previous) that is `solid` and lies toward the light (`dot(dir, toward) > 0`)
throws a shadow of `SHADOW · strength · dot(dir, toward)` onto the half of
the floor quad nearest it; the deepest one wins per half. A neighbour outside
the cone throws none — the same rule `flank_colors` follows ("the cone is
what the party can see"). The shadow half is drawn as a second `poly` over
the lit floor.

**Composition.** `Lit { level, cast }.apply(dim(base, shade(depth, …)))` —
light then fog, both multiplicative. Constants (`WALL_SPAN`, `FLOOR_SPAN`,
`SHADOW`) are named in `stack_light.rs`; render tuning, not `tuning.rs`.

**Unlit:** `cell_mark` glyphs, the heading, `standing_on`, `VOID`, `UNLIT`
and the frame map / inset. These are what the player reads.

### Constraints the lit corridor must keep

- **`UNLIT` stays darker than the far wall** at the worst lit case:
  `WALL · shade(3) · LEVEL_MIN · darkest cast channel` ≈ (0.035, 0.10,
  0.099) against `UNLIT` (0.03, 0.07, 0.085). The existing
  `the_unlit_fill_is_lighter_than_void_and_darker_than_the_far_wall` is
  strengthened to that case.
- **A door still reads as a door**: lit `DOOR` and `SEALED` stay
  distinguishable from lit `WALL` at every level pair in the band (hue, not
  brightness, carries it — assert by dominant channel).
- `cell_mark` stays exhaustive (seam rule); this change does not touch it.

## Testing

Engine (`tests/stack.rs` or beside `stack_view`):
- `light_seed` is the same before and after a step, a turn, and a
  save→load (the ron round trip cannot catch a skipped field, so go through
  `Game::save`/`Game::load`).
- Two frames (different depth, and different entrance) get different seeds.

Gui (`stack_light.rs`):
- Facing North vs South swaps which flank wall is brighter; East/West
  likewise — for a light with non-zero component on that axis.
- A face ahead is brighter when the party faces away from the light.
- Every level lies in `[LEVEL_MIN, LEVEL_MAX]` across a sweep of seeds and
  facings.
- A solid neighbour toward the light shadows the floor half nearest it; one
  away from the light, or outside the cone, does not.
- Mutation check: zeroing `WALL_SPAN` or the shadow term fails a test.

Visual check: `cargo run -- --template stack --keys "Right Right"
--screenshot` at two depths, then Read the PNGs.

`balance_sim` is unaffected (no Stack term, no tuning change).

## Out of scope

Lighting the frame map; gameplay effects of light; animated light;
per-cell light sources (caches, links glowing).
