# GUI font and text layer

## Problem

The graphics frontend has never loaded a font. Every one of the 23
`draw_text` calls and both `measure_text` calls in `render.rs` passes
`None`, so the map grid, the status panel, every menu, and the message log
all render in macroquad's built-in default face.

Four things follow from that:

**Bold is a hack, twice.** macroquad has no bold weight for the default
font, so weight is faked by drawing the same glyph twice a pixel apart —
once for `MessageKind::LevelUp` in `draw_message_line` (`render.rs:47-49`),
once for structures and bosses in the tile loop (`render.rs:367-369`). Both
sites carry a comment admitting it.

**The GUI is stuck on ASCII.** `render.rs:1073` reads `"Inventory - Buffer"`
with a plain hyphen where the TUI at `ui.rs:1554` uses a real `—`. The
scroll indicators at `render.rs:206` and `render.rs:222` use `^` and `v`
where they want `↑` and `↓`. The default font is the reason.

**Nothing scales.** `FONT_SIZE` is a fixed `24.0` and `LINE_HEIGHT` a fixed
`30.0`, tuned against the 900px window height in `window_conf()`. On a
larger display the panel text shrinks relative to everything around it.

**Terrain and entities draw from the same palette at the same intensity.**
`biome_style` returns `BLUE`, `RED`, `CYAN`, `GREEN`, `GRAY`, `WHITE` — the
same constants `glyph_color` hands back for creatures and structures. The
tile-loop double-draw exists to compensate for that collision.

Separately, `MAX_ZOOM` is 4 (`app-core/src/lib.rs:189`), but the GUI does
`zoom.clamp(1, 8)` (`render.rs:307`) and then `tile_px = 20.0 * zoom.min(3.0)`
(`render.rs:313`). Zoom 4 renders identically to zoom 3 — pressing `+` at
zoom 3 mutates state and changes nothing on screen. The `clamp(1, 8)` is
stale against `MAX_ZOOM`. The TUI uses all four steps correctly
(`ui.rs:283`). This has to be fixed here because the map glyph size is
derived from `tile_px`.

## Scope

This is phase 1 of four. In scope: font loading, the sizing model, emphasis
by color separation, and the zoom fix.

Out of scope, in later phases: sub-tile camera lerp and tile depth
(phase 2), screen shake, particles, and world-space floats (phase 3), and
bringing any of this to the TUI (phase 4). Also out of scope: replacing
prose hyphens with em-dashes for TUI parity — cosmetic churn across strings
this phase has no other reason to touch. The two scroll indicators are the
exception, since those exact lines are being edited for metrics anyway.

The TUI is untouched. The engine is untouched.

## The two fonts

| Slot | Font | Cell | License |
|---|---|---|---|
| Map grid | unscii-16 | 8×16 | Public Domain / CC-0 |
| UI (panels, menus, log) | DejaVu Sans Mono, Regular + Bold | vector | Bitstream Vera |

**unscii-16** is picked for its native 16px cell, which lands exactly on
the existing 16/32/48/64 size ladder at 1x/2x/3x/4x, and for keeping 16px
of vertical resolution — species glyphs are case-sensitive (`S` and `s`,
`W` and `w`, `C` and `c` are all distinct species in `assets/species/`), so
the 8×8 variant's lower resolution would actively cost information.

Only the base `unscii-16.ttf` is used. `unscii-16-full.ttf` is **GPL**
(it imports glyphs from Unifont and Fixedsys Excelsior) and must not be
vendored.

**DejaVu Sans Mono** ships a real Bold weight, which is what retires the
`draw_message_line` double-draw. The Bitstream Vera license permits
redistribution provided the notice ships with it and the font is not sold
by itself — both trivially satisfied.

Font files live in `assets/fonts/` and are embedded with `include_bytes!`,
following the precedent `sounds.rs` documents: these are not moddable game
content, so they belong in the binary rather than being loaded from
`assets_dir` at runtime. License texts ship alongside them in the same
directory.

## Architecture

A new `crates/gui/src/text.rs`, mirroring how `fx.rs` was split out of
`render.rs`: sizing and color math as pure free functions that unit-test,
drawing wrappers that don't. `render.rs` is already 1892 lines and this
phase touches 24 `FONT_SIZE` references and 23 `LINE_HEIGHT` references
inside it; the new state does not belong there.

```rust
pub struct Fonts { map: Font, ui: Font, ui_bold: Font }

impl Fonts {
    pub fn load() -> Self { ... }
}
```

`load_ttf_font_from_bytes` is **synchronous** in macroquad 0.4 — unlike
`load_sound_from_bytes`, it is not awaited — but it calls
`get_quad_context()` internally, so it must still run after the window
exists. It is called at the top of `game_loop` alongside
`SoundBank::load().await`.

`load_ttf_font_from_bytes` applies the context's default filter mode, so
the map font needs an explicit `set_filter(FilterMode::Nearest)` after
loading or its vectorized bitmap outlines render blurry. The two UI faces
keep the default linear filter.

`render::draw(&mut app, &mut fx)` becomes
`render::draw(&mut app, &mut fx, &Fonts)`.

## Sizing model

The two fonts are sized by completely independent rules.

### Map: driven by zoom only, never by window size

A larger window shows *more tiles at the same size*, which is what the TUI
already does and what a roguelike should do.

```rust
/// unscii-16's native cell height. Map glyphs are only ever drawn at
/// integer multiples of this, so the vectorized bitmap stays on the pixel
/// grid instead of resampling into mush.
const MAP_GLYPH_NATIVE: u16 = 16;
/// Tile edge at zoom 1, leaving a native glyph a margin inside its cell.
const BASE_TILE_PX: f32 = 20.0;

/// Returns `(tile_px, glyph_px)` for a zoom step.
pub fn map_cell(zoom: u16) -> (f32, u16)
```

Clamped to `MIN_ZOOM..=MAX_ZOOM` rather than the stale literal `1..8`,
giving four distinct steps:

| Zoom | tile_px | glyph_px | multiple of native |
|---|---|---|---|
| 1 | 20 | 16 | 1x |
| 2 | 40 | 32 | 2x |
| 3 | 60 | 48 | 3x |
| 4 | 80 | 64 | 4x |

Zoom 4 becomes a real step. Zooms 1–3 reproduce today's sizes exactly.

### UI: continuous, derived from window height

```rust
/// The `window_conf()` height the current fixed sizes were tuned against.
const REFERENCE_HEIGHT: f32 = 900.0;
const BASE_UI_FONT: f32 = 24.0;
const MIN_UI_FONT: u16 = 16;
const MAX_UI_FONT: u16 = 40;
/// Preserves today's 30.0 / 24.0 relationship.
const LINE_HEIGHT_RATIO: f32 = 1.25;

pub struct Metrics {
    pub font_size: u16,
    pub line_height: f32,
    pub pad: f32,
    pub gap: f32,
}

impl Metrics {
    pub fn title(&self) -> u16;  // was FONT_SIZE + 4.0
    pub fn small(&self) -> u16;  // was FONT_SIZE - 3.0 and - 4.0
    pub fn label(&self) -> u16;  // was FONT_SIZE - 2.0
}
```

`small()` deliberately collapses two values that are currently distinct:
the keybind block uses `FONT_SIZE - 3.0` and the scroll indicators use
`FONT_SIZE - 4.0`. A one-pixel difference between two unrelated bits of
chrome is not a distinction worth carrying through a scaling system.

```rust

pub fn ui_metrics(window_height: f32) -> Metrics
```

`TextParams::font_size` is a `u16`, so sizes are inherently quantized to
whole pixels and the atlas accumulates at most a few dozen distinct entries
across a resize drag. No separate stepping scheme is needed.

The real work here is the pixel literals scattered through `render.rs` —
`10.0` appears 17 times, `20.0` 16, `16.0` 7, `6.0` 6, `14.0` 5. Those raw
counts are an upper bound, not a work list: they mix three unrelated kinds
of number, and separating them is itself part of the task.

- **UI-font-relative** (popup insets, panel padding, bar height, the `6.0`
  gaps) — these must scale, and become `Metrics` fields.
- **Map geometry** (`BASE_TILE_PX`, the `20.0` in `tile_px = 20.0 * zoom`) —
  these must *not* scale with the UI font; they belong to `map_cell`.
- **Colors and line widths** (`2.0` border thickness, channel values) —
  untouched.

Misclassifying the second group as the first is the most likely way to get
this wrong, since both are spelled `20.0`. `Metrics` is threaded through
`draw_popup`, `draw_row`, `draw_bar`, `draw_ghost_band`, and
`draw_status_panel`; the map path takes `map_cell` instead and never sees
`Metrics`.

## Emphasis by color separation

The tile-loop `bold` flag and both double-draw calls are deleted. Terrain
glyphs are pushed through `terrain_color()`; entity glyphs are left alone
at full saturation.

**The discriminator is saturation, not brightness.** This is a correction
to the obvious approach, and the reason is worth recording:

Dimming terrain below the damaged-structure floor does not work.
`fx::structure_condition` multiplies a structure's glyph color by a tint
bottoming out at `MIN_TINT = 0.45`, and `GlyphColor::DarkGreen` is
`(0.0, 0.4, 0.0)`. A damaged DarkGreen structure therefore renders at a
luminance of about 0.11. For terrain to be guaranteed dimmer than that it
would have to be nearly black — far too dim to see. The brightness
ordering cannot hold for the darkest palette entries, so it should not be
the mechanism.

Desaturating terrain toward grey does hold. However dark or damaged an
entity glyph gets, it stays the only *saturated* thing on the map, and
that reads independently of how bright it is.

```rust
/// Terrain keeps a quarter of its hue and the rest goes to grey. Entities
/// are the only saturated glyphs on the map, which is what separates them
/// — brightness can't, because a damaged `DarkGreen` structure is darker
/// than terrain can afford to be (see `fx::MIN_TINT`).
const TERRAIN_SATURATION: f32 = 0.25;
const TERRAIN_BRIGHTNESS: f32 = 0.70;

pub fn terrain_color(c: Color) -> Color
```

Implementation: compute luminance (`0.299r + 0.587g + 0.114b`), lerp each
channel from that luminance back toward its original by
`TERRAIN_SATURATION`, then scale by `TERRAIN_BRIGHTNESS`.

Tile *backgrounds* are untouched. They stay derived from the
pre-desaturation color at `* 0.18`, so biomes keep their identity and the
critical red wash still lands exactly where it does now. `fx.rs` needs no
change at all.

This also relieves a cell that is already carrying five cues — background
tint, critical red wash, magenta spawn outline, yellow staffed outline, and
blue shield pulse — by taking emphasis out of the outline and background
channels entirely and putting it in the glyph's saturation.

## Testing

Drawing stays untested, consistent with the rest of the renderer and with
what `fx.rs` established. The pure functions carry the tests:

`ui_metrics`
- at `REFERENCE_HEIGHT` reproduces today's values exactly: font 24, line
  height 30
- clamps hold at both extremes — a 200px window and a 4000px window both
  land inside `MIN_UI_FONT..=MAX_UI_FONT`
- `line_height > font_size` at every height in the range
- `small() < font_size < title()` at every height, including at both clamp
  boundaries where naive subtraction could invert or underflow

`map_cell`
- all four zoom steps return distinct `tile_px`, closing the bug where 3
  and 4 were identical
- every `glyph_px` is an exact integer multiple of `MAP_GLYPH_NATIVE`
- out-of-range input clamps to `MIN_ZOOM` / `MAX_ZOOM`
- zooms 1–3 still produce today's 20/40/60 and 16/32/48

`terrain_color`
- reduces saturation for every color in the palette — the channel spread
  `max - min` shrinks in all cases
- shrinks that spread by *exactly* `TERRAIN_SATURATION * TERRAIN_BRIGHTNESS`.
  Both operations are affine per channel, so the spread scales by exactly
  the product and the assertion can be an equality rather than a bound.
- dims luminance by *exactly* `TERRAIN_BRIGHTNESS`. Luminance-weighted
  desaturation is luminance-preserving, so nothing else acts on it.
- an already-grey input stays grey rather than picking up a cast
- preserves alpha

Note what is deliberately **not** asserted: that no output channel exceeds
its input. That is false, and assuming it would have produced a failing
test. Desaturation moves every channel *toward* luminance, so a channel
that starts below luminance gets pulled up — `RED`'s green channel goes
from 0.25 to about 0.28. Luminance is the quantity that only ever
decreases, which is why the test is written against luminance and not
against channels.

`cargo test --workspace` is the gate.

## Files

| File | Change |
|---|---|
| `assets/fonts/unscii-16.ttf` | new — map glyphs (Public Domain / CC-0) |
| `assets/fonts/DejaVuSansMono.ttf` | new — UI regular (Bitstream Vera) |
| `assets/fonts/DejaVuSansMono-Bold.ttf` | new — UI bold (Bitstream Vera) |
| `assets/fonts/LICENSE-unscii` | new — license text |
| `assets/fonts/LICENSE-dejavu` | new — license text |
| `crates/gui/tests/font_rasterization.rs` | new — guards unscii crispness |
| `crates/gui/Cargo.toml` | `fontdue` dev-dependency for that test |
| `crates/gui/src/text.rs` | new — `Fonts`, `Metrics`, pure fns, tests |
| `crates/gui/src/lib.rs` | load `Fonts` in `game_loop`, thread into `draw` |
| `crates/gui/src/render.rs` | route all text through `Fonts` + `Metrics`; delete both double-draws; `map_cell` for zoom; `↑`/`↓` scroll indicators |

No `.ron` schema changes, so `assets/species/README.md` and
`assets/structures/README.md` are untouched. `fx.rs`, the engine, app-core,
and the TUI are all untouched.

## Risks

**unscii's TTF is vectorized, not a true bitmap.** HEX and PCF are the only
real bitmap formats it ships, and macroquad's loader needs outlines, so TTF
is the only option. Whether it rasterizes pixel-crisp depends on fontdue
landing on the pixel grid at 16px; `FilterMode::Nearest` is necessary but
may not be sufficient.

This is checkable without a window. macroquad rasterizes through
`fontdue::Font::rasterize` (`macroquad-0.4.15/src/text.rs:103`), and
fontdue 0.9 is already in `Cargo.lock` as a macroquad dependency, so a
test can call the identical code path as a dev-dependency and assert that
every coverage byte comes back fully-on or fully-off rather than
antialiased. That test is the *first* implementation step, before any of
the metrics refactor is built on top of it.

If it comes out fuzzy, the fallback is nudging `BASE_TILE_PX` and
`MAP_GLYPH_NATIVE` to whatever size does land clean — which is why the size
ladder is expressed as named constants rather than inline arithmetic.

**The metrics refactor is wide.** Every `FONT_SIZE` and `LINE_HEIGHT`
reference (24 and 23 respectively) plus the padding literals around them
change meaning at once, and layout regressions are not caught by any test
in this repo. Mitigation: the reference-height test pins `ui_metrics` to
today's exact values, so at the default 1440×900 window most positions land
on the coordinates they do now.

Three sites do move, because a ratio system cannot reproduce every ad-hoc
literal and a special case for each is not worth the complexity: the status
banner's left inset goes 12→10px, the keybind hints go 21→20px, and
`keys_line_height` goes 26→24px, which puts the five-line keybind block
10px lower. That drift is accepted. Any *other* layout shift at default
size is a bug in the refactor.
