# Sprite Forge: editing shipped art

**Date:** 2026-10-07
**Builds on:** `2026-09-04-dev-sprite-editor-design.md` (the dev-only
`FERAL_DEV_SPRITES` picker and editor).

## Intent

The user wants to open a sprite that already ships — a construct, say —
change how it looks, and save it, all inside the Sprite Forge. They also want
the forge usable by mouse: picking subjects, sampling colours, filling
regions and pressing its actions as buttons.

Success: picking `construct` in the forge opens the art the map draws, the
colours survive the load closely enough to edit by eye, `[s]` (or the Save
button) writes the file the map actually reads, and the change is visible on
the map without a restart.

## What is broken today

- 18 of the 23 shipped sprites are `<key>.colour.png`. `sprites::scan_library`
  keys them by stem — `construct.colour` — so the picker reports `construct`
  as having no art and the editor opens blank.
- Saving writes `<key>.png` beside the colour file, and the loader prefers the
  colour file, so the edit never reaches the map.
- Loading quantises onto the 19-entry `SPRITE_PALETTE`: nine greys and ten
  icon hues. The creature sprites use 8–18 colours each, in hue ramps that
  palette cannot represent, so opening one wrecks it before any edit.

## Design

### 1. A wider `SPRITE_PALETTE` (engine, `icon.rs`)

`SPRITE_PALETTE` grows from 19 entries to 63:

- **Entries 1–9 (indices 0–8): the existing nine-step grey ramp, unchanged
  and in the same order.** Near-white sprites (`anchor`, `player`, the floor
  finishes) therefore quantise to exactly the pixels they do today.
- **Entries 10–63: nine hue families of six shades each**, dark to light
  within a family. The families are fitted to the hues the shipped art uses —
  a hue histogram of the 131 distinct opaque colours across
  `assets/sprites/` clusters at red (0°), orange/brown (20°), yellow (40–60°),
  lime (80°), green (140°), emerald (160°), cyan/teal (180–200°), blue
  (220–240°), violet (280°) and rose (340°). The plan settles the final nine
  (two neighbouring clusters merge) and the shade values.

The `ICON_PALETTE[5..15]` hues are dropped from `SPRITE_PALETTE`; the hue
families cover them. `ICON_PALETTE` itself is untouched — it is pinned at 15
by the player icon's save format, and this palette answers to no save format.

**No save-format change and no migration.** Sprites are stored as RGBA PNGs,
never as palette indices; an index is only meaningful inside one editing
session.

**Acceptance bound.** A test quantises every opaque pixel of every shipped
sprite and asserts the worst-case RGB distance stays under a named bound.
The plan measures the chosen palette and sets the bound just above it,
recording the mean and max in the test's doc comment, so a later palette
edit that makes shipped art snap worse fails loudly.

`quantise` and `sprite_rgba` keep their signatures and the `index 0 =
transparent, n = palette[n - 1]` convention. Indices stay `u8`.

### 2. Colour sprites load, save and toggle on their own file

**The variant is a fact about installed art, carried with it.**
`App::install_sprite_library` takes `InstalledSprite { canvas, full_colour }`
values instead of bare `Canvas`es, for both the enabled and disabled maps.

- `scan_library` strips `FULL_COLOUR_SUFFIX` (`engine::icon`, already shared
  with the asset census) and keys `<key>.colour.png` / `<key>.colour.png.off`
  under `<key>` with `full_colour: true`. When both `<key>.png` and
  `<key>.colour.png` exist the colour one wins — the loader's own precedence
  rule (`assets/sprites/README.md`), so the editor opens what the map draws.
- `SpriteEditor` remembers the variant it opened with.
- `SpriteOp::Save` carries `full_colour: bool` alongside the canvas, and
  `Enable`/`Disable` carry it too, so `apply_sprite_write` derives every path
  from `(name, full_colour)` through one function. app-core still never opens
  a file.
- **The variant rule lives in app-core, where it is testable without a
  filesystem.** Art that was loaded keeps its variant. Art with no file yet
  saves as `full_colour` exactly when the canvas uses any palette entry past
  the grey ramp — a hued sprite drawn as plain `.png` would be multiplied by
  the species tint and go muddy, which is the README's own rule. Editing a
  plain near-white sprite in hue does not switch its variant; the preview
  shows that result honestly (below).
- `apply_sprite_write`'s invariant — `<file>` and `<file>.off` never both
  exist — holds per variant file.

**The preview matches the map.** `SpriteEditorView` carries `full_colour`, and
`draw_preview_cell` tints a colour sprite the way the map does: grey at the
brightest channel of the subject's colour, not the subject's hue. That grey
conversion is inline in `Painter::sprite` (`paint.rs`) today; it is extracted
to a pure `colour_sprite_tint(Color) -> Color` that both `Painter::sprite` and
the preview call, so it isn't copied (the "mirrors" rule in `CLAUDE.md`).

### 3. Mouse and tools

All new actions go through the existing seam: the gui resolves a pointer to a
`PointerHit` and app-core never sees a pixel.

**Swatch grid.** 64 swatches (transparent plus 63) do not fit one row at
1280x720. `canvas.rs` gains a grid layout — `draw_swatch_grid(rect, cols, …)`
and a 2-D `swatch_at` — with the existing single row as the one-row case, so
the icon editor draws exactly as it does today. `CanvasEditor` learns the
palette's column count at `open`, and in palette focus Up/Down step one row;
at one row (the icon editor) Up/Down keep their current behaviour.

**Tool mode.** `CanvasEditor` gains `Tool::{Paint, Fill}`, toggled by `[f]`.
In Fill, `Space` at the cursor and a primary click on a cell flood-fill the
4-connected region of that cell's index with the selected swatch, one pixel
at a time regardless of brush, as one undo entry (the lazy stroke snapshot
already gives this). A drag in Fill mode fills once on `Down` and ignores the
rest of the gesture. The icon editor never offers `[f]` — it intercepts no
key for it — so its behaviour is unchanged.

**Eyedropper.** `PointerButton::Middle` on a cell selects that cell's index
as the swatch; the gui reports Alt+Primary as `Middle`. `[i]` does the same at
the keyboard cursor. Picking never records an undo entry.

**Buttons.** The editor draws a button bar: Save `[s]`, Undo `[u]`, Clear
`[x]`, Brush `[g]`, Paint/Fill `[f]`, Back `[Esc]`. The gui resolves a click
to `PointerHit::Button(EditorButton)`; app-core maps `EditorButton` to its
`GameKey` and calls `handle_sprite_editor_key` with it. **The key is the one
code path**, so a button cannot drift from its key. The bar's labels show the
key, the Paint/Fill button shows the current tool, and Brush shows the
current size.

**Picker.** Pointer routing, today confined to `Mode::SpriteEditor`, extends
to `Mode::SpritePicker`. The gui resolves a click to a subject index; a
single click moves `menu_selected` there, and a double click (egui's own
double-click detection) on the selected subject opens it — the same call
`Enter` makes. Every subject already fits at 1280x720
(`the_picker_shows_every_subject_with_no_scroll_at_1280x720`), so there is no
wheel scrolling.

**Keys.** `[f]` and `[i]` are lowercase, matching the editor's existing
`g`/`s`/`u`/`x`, rather than the uppercase-actions rule the rest of the game
follows. This screen has no row selectors for them to collide with.

## Out of scope

- Lossless editing of arbitrary hand-authored PNGs. A save still quantises
  irreversibly; the README's warning stays, re-worded for the wider palette.
- A per-sprite palette, a colour picker, or editing palette entries in-game.
- Mouse support on any screen other than the two forge screens.
- Brushes larger than 2, shapes, selection, copy/paste.

## Crates touched

- `engine` — `SPRITE_PALETTE`, its doc comment, palette tests.
- `app-core` — `InstalledSprite`, variant tracking, `SpriteOp` fields,
  `Tool`/fill/eyedropper in `CanvasEditor`, grid navigation,
  `EditorButton`, picker pointer handling.
- `gui` — `scan_library`, `apply_sprite_write` paths, swatch grid, button
  bar, picker hit rects, Alt/middle mapping, preview tint.
- `assets/sprites/README.md` — the editor now opens and saves `.colour`
  files; the quantise warning updated.

## Testing

TDD, a failing test first for each:

- `scan_library` keys `x.colour.png` and `x.colour.png.off` under `x` with
  `full_colour`, and prefers the colour file when both exist.
- `apply_sprite_write`: Save, Enable and Disable on a colour sprite touch
  only `<key>.colour.png[.off]`; the never-both invariant holds per variant.
- app-core: new art using a hued swatch saves `full_colour: true`, grey-only
  art saves `false`, loaded art keeps its variant.
- Round trip: every shipped sprite quantised, written and re-read gives the
  same canvas; the worst-case snap distance is under the named bound.
- Fill stays inside its region, ignores brush, and is one undo entry;
  a no-op fill records nothing.
- Eyedropper selects the cell's index and records no undo entry.
- Each `EditorButton` has the same effect as its key.
- Picker: a click selects, a double click opens the selected subject.
- Grid: `swatch_at` resolves every swatch centre and misses every gap; the
  icon editor's one-row geometry is unchanged.
- `the_editor_screen_fits_at_1280x720` and the picker fit test still pass.

Gate: `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
and a `--screenshot` of the editor open on `construct`.
