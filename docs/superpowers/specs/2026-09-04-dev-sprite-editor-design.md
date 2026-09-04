# The developer sprite editor

**Date:** 2026-09-04
**Status:** approved, not implemented

A dev-only screen for drawing a one-cell sprite for any species, structure or
map fixture, saving it where the loader already looks, and turning it back off
without losing the art. Ships **invisible to a player and to an installed
build** — that is the acceptance criterion, not a caveat.

## Why now

Three things landed on 2026-09-03 and between them they leave exactly one gap.

`v0.13.89` made `assets/sprites/` a drop-in seam: `scan_sprite_dir` keys the
table by file stem, `SpeciesDef::sprite_name()` and
`StructureDef::sprite_name()` fall back to the def's own `id`, and a missing
file ends at the glyph. So **giving an entity art is already nothing but
putting a correctly-named PNG in a directory** — no `.ron` edit, no Rust
change, no save-format bump.

`v0.13.87` and `v0.13.88` built a working pixel editor — `IconEditor`, its key
table, its screen — but pointed it at exactly one target: the player's own
`@`, 8x8, encoded into `profile.ron` as a hex string.

What is missing is the join. There is a place to put art and a way to draw
art, and no way to draw art *for* that place. `docs/superpowers/reports/
2026-07-27-renderer-graphics-assessment.md`'s standing finding is that entity
sprites are the tractable half of the graphics question — 17 species and 30
structures as independent 16x16 images, no tiling, no autotile variants — and
the note in memory says the pipeline now exists and the work is parked on art.
This is the tool that unparks it.

It is a developer tool. It is not content, it is not moddable surface, and no
player ever sees it.

## What was decided

### It is a main-menu row, not a fifth binary

Behind `FERAL_DEV_SPRITES`, read through `dev_console::dev_flag` — the same
predicate `FERAL_DEV_ARENA` and `FERAL_DEV_REVEAL` use. Two answers to "is a
dev flag set" is drift this repo has already caught once, so there is one
predicate and only the flag name is new.

A standalone `spriteedit` bin was rejected on cost and on capability. Cost: it
would have to stand up its own bevy window, egui context and asset load, which
is the entire job `crates/gui` exists to do — the four existing dev bins are
headless for exactly this reason, and a fifth that needs a GPU is not the same
kind of thing. Capability: only the in-game route can show the sprite on the
real map, tinted by the real `difficulty_color` and `biome_tint`, which is the
question the tool is being built to answer.

### The row is gated on there being a checkout, not only on the flag

`paths::DevPaths` is already defined as "repo-only material, `None` in an
installed build". A `sprites: PathBuf` field joins `arenas` and `battle_log`,
and the launcher installs it into `App` the way it already installs
`DevTemplates`. **No path installed, no row** — so a shipped build cannot
offer a screen whose whole purpose is writing into a source tree it does not
have.

### The canvas is 16x16 always; coarseness is a brush, not a format

`assets/sprites/README.md` calls 16x16 non-negotiable, and it is right: it is
what makes `text::map_cell`'s integer zoom ladder pixel-exact. So the file is
always 16x16 and the editor's data is always 16x16.

"Choose the grid per sprite" is therefore **a brush footprint, not a second
canvas size**. `[g]` toggles the brush between 1x1 and 2x2; at 2x2 the cursor
steps two cells and snaps to an even grid, so the gesture is precisely the
8x8 player editor's, while the file, the save path, the load path and the
upload stay single. Nothing about which brush was used is recorded anywhere,
because nothing needs to know.

### The palette is new, and must not be an extension of `ICON_PALETTE`

`ICON_PALETTE` is exactly fifteen entries because `PlayerIcon`'s `v2` codec is
64 hex digits, and one hex digit holds 0-15 with 0 meaning transparent.
**Extending it is a save-format break**, silently — a sixteenth colour encodes
as a digit that means transparent on read. So the dev editor gets its own
`SPRITE_PALETTE` and the two never merge.

It is ordered **ramp first, hues after**: a nine-step value ramp biased bright,
then the ten hues `ICON_PALETTE` already carries. That ordering is the
near-white rule made into a default gesture rather than a paragraph of
documentation — the renderer hands egui the glyph's colour as a *multiplying*
tint, so ramp art inherits the species hue, `biome_tint` and the damage
dimming for free, while a saturated sprite on a differently-hued species goes
black. The hues stay reachable for the two tiles that inherit no hue at all:
the player's own icon and the anchor.

### Turning art off is a rename, not a delete

The picker's toggle renames `<name>.png` to `<name>.png.off` and back.
`scan_sprite_dir` filters on the extension being exactly `png`, so a `.off`
file is already invisible to it — the disable costs no loader change at all,
and the art survives it.

A hard delete is deliberately not offered. Nothing needs it, and a destructive
verb on a screen whose entire subject is unbacked-up work is a bad trade for a
keystroke.

### Three crates, three jobs — and the file format never reaches app-core

- **engine** owns colour semantics: `SPRITE_PALETTE`, and the quantiser that
  maps an arbitrary RGBA pixel to the nearest palette index (alpha below a
  threshold → 0, transparent).
- **gui** owns file I/O: decoding the sprites on disk, encoding a canvas back
  out, and re-scanning the written name into the `SpriteTable`. `image`
  `0.25.10` joins `crates/gui`'s dependencies with `default-features = false,
  features = ["png"]`. It is already in `Cargo.lock` at that exact version via
  bevy, so the dependency *graph* grows by nothing; bevy's own `png` feature
  is decode-only, which is why the encoder has to be named.
- **app-core** owns flow, and learns nothing about PNG or about pixels.

The material crosses those boundaries through the two seams already in use for
this shape. Inbound: `App::install_sprite_library(HashMap<String, Canvas>)`,
`install_dev_templates`' pattern — the frontend hands over decoded, quantised
canvases and app-core owns them from there. Outbound: `App::take_sprite_writes()`,
`take_sounds`/`take_transits`' pattern — app-core queues a `SpriteWrite { name,
canvas: Option<Canvas> }` cue and forgets it; the gui drains, writes or renames,
and refreshes both the table and the library at that one site, which is how the
map updates without a restart.

### The mouse resolves to a cell before it reaches app-core

There is no mouse handling anywhere in `crates/gui` today; this adds the first,
and confines it to one screen.

The gui already computes the canvas and swatch rects in order to draw them, so
it tests the pointer against those and emits **a cell or a swatch**, never a
pixel: `PointerHit::{Cell(u8, u8), Swatch(u8)}` with a
`PointerButton::{Primary, Secondary}`, through `App::handle_pointer`. This is
the same renderer-agnostic seam `GameKey` is, in the same direction, and it
keeps `paint.rs` the only file that names a graphics library. `GameKey` itself
gains nothing — its doc comment says it names physical gestures, and a click is
not a keystroke.

**A drag is one undo entry, not one per cell.** The gui reports the button
going down, each cell crossed while held, and the button coming up;
`CanvasEditor` snapshots on the down and not again until the up. Without this
the first thing anyone does — drag a line — costs sixteen undos to take back.
Painting a cell that already holds the selected index still records nothing at
all, which is the existing `paint` guard doing exactly the job it was written
for.

## The shape

The structural decision, taken through the `design-patterns` dialog: **share
the canvas mechanics, not the state machine.** Roughly seventy of
`icon_editor.rs`'s 226 lines are the cursor clamp, the paint guard, the
snapshot ring and the shared key verbs; the rest — the sink, the outcome, the
wizard's Esc interception — differs between the two editors and should.

```
engine::icon
  Canvas { edge: usize, cells: Vec<u8> }   get / set / clear / is_blank
  PlayerIcon { cells: Canvas }             edge 8; keeps encode/decode/rgba/
                                           pixel_rgba and the v1 fold, alone
  SPRITE_PALETTE, quantise()

app-core
  CanvasEditor { canvas, cursor, selected, focus, brush, history, stroke }
      the shared verbs and the shared half of the key table
  IconEditor    { editor: CanvasEditor, opened_with }        wizard sink
  SpriteEditor  { editor: CanvasEditor, subject }            PNG-cue sink

gui
  draw_canvas(rect, &CanvasView)           the grid, cursor and swatch row
  render/icon_editor.rs, render/sprites.rs each compose their own chrome
```

Both editors **own** a `CanvasEditor` as a field rather than being one, and
each takes the keys `CanvasEditor` reports as unhandled. A `Grid` trait over
the verbs was rejected: two implementors where one concrete type serves both is
indirection with no payoff, and this repo's own idiom says so — `CraftOrder`
stayed a struct until its second implementor was real. Const generics were
rejected for climbing out of the data and into the view types and the renderer.

`PlayerIcon`'s internals do open as a result, and its codec is load-bearing.
The change to it is mechanical, and **the existing icon tests are the gate on
it** — they must pass untouched.

## Screens

**`Mode::SpritePicker`** — every name the map can draw a sprite for: each
species def, each structure def, and the two names hardcoded in Rust
(`player` via the engine's `DEFAULT_PLAYER_SPRITE`, `anchor` at
`render/base.rs`). Each row carries the def's glyph in its own palette hue, the
id, and its art's state: none, on, or off. `Enter` edits, `[t]` toggles art on
or off, `Esc` returns to the menu.

**`Mode::SpriteEditor`** — the canvas, the swatch row, and a live preview cell
drawn the way the map draws it. Keys are the existing editor's, plus `[g]` for
the brush and `[s]` to save. The two new modes need their rows in the
renderer's `ALL_MODES`, in `needs_status_banner` and in the refusal census.

## Testing

- **engine** — every `SPRITE_PALETTE` entry quantises to itself; a
  below-threshold alpha quantises to 0; the palette is distinct; `ICON_PALETTE`
  is still fifteen entries, which is the guard on the save format.
- **app-core** — `CanvasEditor`: brush 2 writes four cells and snaps odd
  coordinates, a stroke is one undo entry, a repaint of the same index records
  nothing, both cursors clamp without wrapping. `SpriteEditor`: opening loads
  the installed canvas, saving queues one cue, toggling queues a rename cue,
  Esc discards. `IconEditor`: **its existing tests, unchanged**.
- **gui** — a canvas written and read back through a temp directory is the same
  canvas; the shipped-sprites-are-one-cell census still holds; the editor
  screen measures inside the width it claims, `render/icon_editor.rs`'s
  own `e47c66ea` fix being the precedent.

## Out of scope, deliberately

Terrain and tilesets (parked on autotiling, 112+ tiles — a different problem).
Item icons (the sprite seam is one map cell; items are not drawn on the map).
Animation or multiple frames. Editing the palette. Sprites under arbitrary
names — only names the map can actually draw. Mouse input anywhere else in the
game.

## Acceptance

1. With `FERAL_DEV_SPRITES` unset, or in a build with no checkout behind it,
   the game is exactly the game it is today: no row, no reachable mode, no
   behaviour change.
2. The player's icon editor behaves identically, on its existing tests.
3. Drawing a structure's sprite and saving puts it on the map without a
   restart; toggling it off brings the glyph back and keeps the art on disk.
