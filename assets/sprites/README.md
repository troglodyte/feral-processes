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
keeps everything that already colours a glyph — the species' own authored
hue, `biome_tint`, the damage dimming — working untouched, for free.

A sprite that carries its own colour fights all of them: a red sprite for
a species authored green goes black. Shade with **value**, not hue.

The con read is **not** one of them any more. It used to replace a
hostile's glyph colour outright, which made this rule far sharper — art had
to survive being multiplied by anything from green through red. It is a bar
along the bottom edge of the tile now, and a boss and a nemesis wear corner
marks, so all three reach a tile without touching a sprite's pixels.

### The one exception: a player-drawn icon

The player's own drawn icon (the pixel editor, `"@drawn"` in the sprite
table) is the sole sprite exempt from this rule, and it is drawn
untinted. It is also the one sprite nobody authors as a file: the player
edits an **8x8 grid** (`icon::ICON_GRID`) and each cell fills a 2x2 block
of the 16x16 texture the game uploads, which under nearest sampling is
pixel-identical to a native 8x8 one — so the format above is untouched by
it, and **an authored sprite is still 16x16**. It carries its own colour from a fixed fifteen-colour palette
the player chose on the wizard's Colour step, so it has no hue to protect
the way a near-white sprite does — this tile is the only one in the game
that inherits none of the hues the tint exists to preserve (no species
colour, no `biome_tint`, no damage dimming), so dropping the tint's hue
costs nothing but the Colour step's swatch on that one tile. That is what
makes the exception safe here and nowhere else: any *authored* sprite
still needs the near-white treatment above, because it does carry a hue
this tint would otherwise be protecting.

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
- `anchor.png` — the base anchor, the permanent door into base space. A
  ring with a bright core, hard-edged rather than anti-aliased: alpha
  blending on a 16px circle becomes 4x4 blocks of half-transparency at
  zoom 4, where a thresholded edge stays a clean step. Its glyph fallback
  is `#`, which was chosen by elimination rather than by meaning (see
  `Game::new`'s anchor spawn), so this is the sprite that carries the most
  — it is also what `NotificationKind::BaseFounding` draws, and that
  notice's prose deliberately no longer names a character.
