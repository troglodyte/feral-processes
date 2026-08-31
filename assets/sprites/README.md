# `assets/sprites/`

One-cell sprites for the surface map. A sprite is drawn in place of an
entity's `glyph`, in the same cell, at the same size.

## The format is not negotiable

**16x16 pixels, RGBA, PNG.** `text::map_cell` sizes a map glyph at
`16 x zoom` px with zoom clamped to 1..4, so a sprite is drawn at exactly
16, 32, 48 or 64 px — integer multiples of its authored size, sampled
nearest-neighbour. That is the same discipline `unscii-16` is held to by
`crates/gui/tests/font_rasterization.rs`, and it is what keeps pixel art
crisp instead of resampling into mush.

A sprite of any other size still draws, but it is scaled by a non-integer
factor at some zoom and will blur. `the_shipped_sprites_are_one_cell` in
`crates/gui/tests/sprites.rs` refuses one at load.

## Author them near-white

The renderer hands egui the same `Color` the glyph path would have used,
as a **tint**, and an egui tint *multiplies*. A white sprite therefore
keeps everything that already colours a glyph — `difficulty_color`, the
nemesis and boss overrides, `biome_tint`, the damage dimming — working
untouched, for free.

A sprite that carries its own colour fights all of them: a red sprite
under a green con-colour goes black. Shade with **value**, not hue.

## Fallback

A missing sprite is not an error. `Painter::sprite` returns `false` when
it has nothing under that name and the caller draws the `glyph` instead,
so deleting this directory restores the glyph map exactly — the same
supported way deleting `assets/sectors/` restores undifferentiated
zones. That is what lets a modded species ship without a sprite rather
than shipping invisible.

## What is here

- `player.png` — **placeholder**, generated rather than drawn. It exists
  to prove the texture pipeline end to end and is meant to be replaced by
  real art. It is deliberately not an `@`, so it is obvious at a glance
  whether the sprite or the glyph drew.
