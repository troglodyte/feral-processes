# `assets/sprites/`

One-cell sprites for the surface map. A sprite is drawn in place of an
entity's `glyph`, in the same cell, at the same size.

## Naming

A file's name — everything before `.png` — is the key it is looked up
under. `crates/gui/src/sprites.rs::scan_sprite_dir` scans this directory at
load and keys the table by file stem, so dropping a correctly-named file in
is the whole of shipping it; nothing outside this directory has to name the
file, and there is no list to edit anywhere else.

For a species or a structure, that key is the def's own `id` by
convention. `SpeciesDef::sprite_name()` and `StructureDef::sprite_name()`
both fall back to `id` unless the def's own optional `sprite:` field
overrides it (see the schema in `assets/species/README.md` and
`assets/structures/README.md`) — an escape hatch for when the filename you
want isn't the id, not the normal way a species or structure gets art. No
shipped def uses the override: name the file after the id and it is found
with no `.ron` change at all.

A file whose stem starts with `@` is never scanned, at any name — `@` is a
legal filename character on every platform this ships to, and `"@drawn"`
is the runtime-only key the player's own drawn icon registers under
(`sprites::DRAWN_ICON_KEY`). Without the filter, an `@drawn.png` dropped in
here would silently hijack the player's own drawing. A `sprite:` override
starting with `@` is not honoured either — `SpeciesDef::sprite_name()` and
`StructureDef::sprite_name()` fall back to the def's own id instead — so it
can never read the player's drawing off that slot: don't author one.

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

## Disabling a sprite

A file whose name ends `.png.off` — `<name>.png.off` — is a **disabled**
sprite: the same PNG, renamed rather than deleted, so the art survives
turning it off. `scan_sprite_dir` filters entries on the extension being
exactly `png`, so an `.off` file is already invisible to the loader with no
change to that scan at all — it never reaches the asset server, and the
entity it was drawn for falls back to its glyph exactly as if the file were
absent.

The dev-only sprite editor (`FERAL_DEV_SPRITES`, a checkout-only screen —
see `crates/gui/src/render/sprite_forge.rs`) is what writes `.png` files
into this directory and what renames them to and from `.png.off`; nothing
else in the game does either. It is invisible in an installed build and to
a player, so this directory's population is still meant to change only by
someone dropping in a file — a stray `.png.off` sitting next to an enabled
sprite is normal, not a leftover to clean up, and is exactly how a piece of
art gets shelved without losing it.

## A save quantises the file, irreversibly

The dev-only sprite editor reads any 16x16 PNG in this directory back onto
its own `SPRITE_PALETTE`, snapping every pixel to the nearest of that
palette's colours — so opening a piece of hand-authored art with a richer
palette and pressing `[s]` writes back a quantised copy, with no warning and
no backup kept anywhere. There is no undo for this once the file is
overwritten (the editor's own `[u]` only reaches back through the session's
own strokes). If you want to keep the original, copy it elsewhere first.

## Fallback

A missing sprite is not an error. `Painter::sprite` returns `false` when
it has nothing under that name and the caller draws the `glyph` instead,
so deleting this directory restores the glyph map exactly — the same
supported way deleting `assets/settlements/` restores a world with no
towns in it. That is what lets a modded species ship without a sprite rather
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
