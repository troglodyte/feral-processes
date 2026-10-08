---
paths:
  - "crates/gui/**"
  - "assets/sprites/**"
---

# The drawing seam

**The drawing seam:** `crates/gui/src/paint.rs` is the only file that names a
graphics library. The ~3,000 lines in `crates/gui/src/render/` draw through
`Painter` — fifteen operations, plus local `Color`/`Rect`/`TextDims`/`TextRun`
— and know nothing about the backend. That is what made the macroquad→Bevy swap
touch five files and no drawing code. Don't reintroduce direct backend calls
in `render/`. **The panes take their origin from the caller** — a `Rect`,
not a width and a height — because the status bar claims a row off the top
of the window. That is affordable only because each view states the origin
once: `stack::slice` for the whole corridor projection, `tile_origin_px` for
the surface map, `inset_rect` for the frame inset. A literal `0.0` in either
file draws under the strip and no test sees it. The fourteenth is `clipped`,
the only one about *not* drawing: the Stack corridor's lateral columns
overhang their pane by construction, and hand-clipping a trapezoid changes
the perspective it was drawn with. **The fifteenth is `sprite`**, the only
one that names a texture: a one-cell sprite **substitutes** for an entity's
glyph and never draws beside it, and a name the table has nothing under
returns `false` so the caller draws that glyph instead. `sprite` takes a
`phase_key` and picks a cell of a two-frame sheet from the painter's
animation clock, which is `None` (first cell) when effects are off. Three
things hold it up. `assets/sprites/` is optional by construction — a missing directory, file
or name all end at the glyph, so never gate the draw or the loader on it
being non-empty. `color` is a **multiplying tint**, so art is authored
near-white and inherits `difficulty_color`, `biome_tint` and the damage
dimming for free. And `sprite` takes a **top-left** and fills its square
while `map` takes a *baseline* centred on measured ink — reading the two as
one convention is a half-cell offset that reads as a camera fault. **The trap
is overdraw**: painting the sprite over a glyph that is still there looks
perfect against opaque art and breaks the moment one has any transparency, so
the test asserts the mesh *and* the absent `@`. Sprites are 16x16 because
`map_cell`'s ladder is integer multiples of unscii's cell, and
`ImageSampler::nearest()` at load is what makes that worth anything —
bevy_egui binds the image's own sampler and Bevy's default is linear.
**The player's drawn icon is the one sprite drawn untinted, and the
player tile's fallback is three-step**: the runtime-only `"@drawn"` key,
then `player.png` under `"player"`, then the `@` glyph. It is the
exception to "art is authored near-white" because it is the one tile
that inherits none of the hues that rule protects — no species colour,
no `biome_tint`, no damage dimming — so putting the hue back reads as a
bug fix and is not one. **The player edits `ICON_GRID` (8) and the
sprite stays `ICON_SIZE` (16)**, `ICON_CELL_PIXELS` the one expression
of the ratio: each drawn cell fills a 2x2 block of the upload, and the
save string is `"v2:"` plus 64 hex digits with `v1` folded, never
dropped.

**The battle portrait** (`render/battle.rs::draw_battle_portrait`) is the
one sprite drawn as a picture pane rather than in a glyph's place; its glyph
is only the `false`-return fallback (trap: `seams` skill, `hud.md`).
