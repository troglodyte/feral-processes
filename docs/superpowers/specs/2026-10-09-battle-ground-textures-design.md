# Battle-map ground textures

**TODO #123.** The tactical battle map draws every cell as one flat colour
picked by its kind. It reads as grey squares, the four kinds are told apart
by brightness alone, and a Backplane fight looks exactly like an OpenGrid
one. This change textures the ground: per kind, per biome, from PNGs a
player or a mod can redraw.

## Decisions

| Question | Decision |
|---|---|
| Goal | Atmosphere, legibility of the four kinds, and per-biome variety — all three. |
| Art source | 16x16 PNGs in `assets/sprites/`, starter set script-generated; any tile redrawable in the Sprite Forge. |
| Variation | Per kind, per biome, with a per-kind default set behind it. No random per-cell variants. |
| Grid | The 1px gap between cells stays. |
| Wiring | A file-name convention, not a new asset kind (no `.ron`, no `*Db`). |
| Colour | Near-white art, tinted by kind brightness times a muted per-biome hue. |

Rejected: a per-biome ground-set `.ron` (a new asset kind, README and
loader to hold what a naming rule already says); procedural patterns in
code (not moddable as files); full-colour tiles (compete with the
hue-coded washes and lose the free dimming of multiplied tints).

## The rule this must keep

`cell_color` in `crates/gui/src/render/tactical.rs`: **hue is spoken for by
what is standing on a cell; a kind may only spend brightness.** Washes are
alpha 0.13 precisely so the kind under them stays legible. Textures may add
pattern and a *muted* biome cast, never a hue strong enough to be mistaken
for a wash (Move, Cover, Danger, Aim, Hits, ChargeParty, ChargeHostile).

## Engine: the view carries the ground's biome

`TacticalBattle` already holds `spec: BattleSpec`, and `spec.biome` is the
raw biome the board was generated from (`tactical/turn.rs`, the field and
Stack fight path). `TacticalView` gains:

```rust
/// What the ground is made of, for the renderer's textures only. ...
pub ground: Biome,
```

set in the view builder (`tactical/view.rs`) from `battle.spec.biome`,
**except a siege board, which reads `Biome::Platform`**: `game/siege`
opens its fight with the biome of the surface tile under the base, but the
board itself is the base's own flood-filled floor, and should look like it.
The plan names the existing predicate that tells a siege fight apart; it
does not add a resource (a new `Resource` shifts bevy query order under
unrelated tests).

No save change. `TacticalBattle` is never saved; a siege restored by
`siege/persist.rs` reopens with its saved spec and is still a siege, so it
still reads `Platform`. `SAVE_FORMAT_VERSION` does not move.

## Renderer: drawing one cell

Order inside `draw_tactical_map`'s cell loop, unchanged except for step 2:

1. `painter.rect(… tile_px - 1 …, cell_color(kind))` — kept as the fallback
   and as the backdrop under any transparent pixel.
2. **New:** `painter.sprite(key, px, py, tile_px - 1, tint, …)` over the same
   square, so the grid gap survives. If `sprite` returns `false`, nothing
   else is drawn — step 1 already is the fallback.
3. Washes, hit flash, marks, bodies, forecast, cursor, legend — untouched.

Two pure functions in the gui crate, next to `cell_color`:

- `ground_sprite_keys(biome, kind) -> [String; 2]` —
  `ground_<biome>_<kind>` then `ground_<kind>`, snake-case
  (`ground_backplane_cover`, `ground_cover`). The draw tries them in order
  and stops at the first that draws.
- `ground_tint(biome, kind) -> Color` — `ground_level(kind)` (a brightness
  per kind, ordered Blocked < Open < Rough < Cover as `cell_color` is, but
  lifted so near-white art shows its pattern) multiplied by
  `biome_tint(biome)`.

`biome_tint` is an **exhaustive** `match` on `Biome` in `palette` — no `_`
arm, so a new biome fails to compile until it is given a cast. Unwalkable
biomes (DataVoid, BlackIce, Entropy) never host a fight; they get a neutral
tint rather than a panic.

Only `paint.rs` names a graphics library; this is all `Painter::sprite`
(drawing-seam rule). The sprite is a replacement for the flat fill, not a
glyph, so the "sprite replaces a glyph, never beside it" overdraw rule is
not in play: there is no glyph on a ground cell.

## Art

A committed, stdlib-only Python script (`zlib` + `struct`, no Pillow)
writes the starter PNGs into `assets/sprites/`: 16x16 RGBA, near-white,
nearest-sampled like every sprite.

- **4 defaults:** `ground_open`, `ground_rough`, `ground_cover`,
  `ground_blocked`.
- **20 biome tiles:** OpenGrid, Deadlock, NullSector, Backplane, Platform x
  the four kinds.

Shape language by kind, so kind reads from pattern as well as brightness:
Open is quiet ground; Rough is broken, cluttered ground; Cover is a raised
block with a lit top edge; Blocked is a void with a rim. Motif by biome:
OpenGrid a faint lattice, Deadlock interference and cracks, NullSector
speckled holes, Backplane circuit traces, Platform laid plates.

The script is the starting point, not the source of truth: once a tile is
redrawn in the Sprite Forge the PNG is what ships, and the script is not
re-run over it. The plan decides where the script lives.

`assets/sprites/README.md` documents the `ground_` keys and the fallback
order in the same change.

## Testing

Unit (gui crate, headless):

- Key order: a biome tile is tried before the default, for every
  `(biome, kind)`.
- Brightness order: for every playable biome, `ground_tint` keeps
  Blocked < Open < Rough < Cover by luminance.
- Muted: every `biome_tint` stays under a saturation bound, so no biome cast
  can be read as a wash hue.
- Draw: a cell whose key resolves draws the sprite over the fill; a cell
  with no sprite draws the fill alone (missing art = today's look).

Engine:

- A field fight's view carries the spec biome; a siege fight's view carries
  `Platform`, including after a siege save/load round trip.

Visual: screenshots through the arena (`--keys` cannot walk into a fight)
of at least Backplane, Deadlock and a siege board at zoom 1 and 2, Read and
checked for: the four kinds distinct at a glance, and the Move, Aim and
Danger washes still clearly legible over the textures.

## Scope

Three code crates (engine view, app-core only if it re-exports the view,
gui) plus assets and one script. No schema, no save-format change: a patch
release. Out of scope: per-cell random variants, animated ground,
surface-map or base-map textures, a legend for the tactical colours (bug
#13).
