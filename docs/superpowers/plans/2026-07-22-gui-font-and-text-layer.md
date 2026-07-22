# GUI Font and Text Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give the macroquad frontend real fonts — a pixel font for the map grid and a vector monospace for the UI — with sizes that scale to the window, emphasis carried by saturation instead of a double-draw hack, and the dead zoom-4 step fixed.

**Architecture:** A new `crates/gui/src/text.rs` owns the loaded fonts and all sizing/color math, mirroring how `fx.rs` was split out of `render.rs` — pure functions that unit-test, drawing wrappers that don't. `render.rs` keeps its layout logic but stops holding font constants. The engine, app-core, `fx.rs`, and the TUI are untouched.

**Tech Stack:** Rust 2024 edition, macroquad 0.4 (`load_ttf_font_from_bytes`, `draw_text_ex`, `TextParams`), fontdue 0.9 (test-only, already in `Cargo.lock` via macroquad).

**Spec:** `docs/superpowers/specs/2026-07-22-gui-font-and-text-layer-design.md`

## Global Constraints

- **Never run `git commit`.** It is in the `deny` list in `.claude/settings.local.json`. Every task ends with a **Checkpoint** step listing the files for the user to commit themselves. Do not attempt the commit and do not work around the denial.
- **The working tree already contains unrelated changes** to `crates/app-core/src/lib.rs`, `crates/gui/src/render.rs`, and `crates/tui/src/ui.rs` (an inventory fuse-action refactor). Do not revert, stage, or amend them. Checkpoint steps name only this plan's files.
- **`cargo test --workspace` is the final gate**, not just the tests you wrote. Run `cargo fmt` and `cargo clippy --workspace` after every task and fix warnings rather than silencing them.
- **Only `unscii-16.ttf`** may be vendored. `unscii-16-full.ttf` is **GPL** (it imports Unifont and Fixedsys Excelsior glyphs) and must never be added.
- **The GUI crate is `feral-processes-gui`.** Its tests run with `cargo test -p feral-processes-gui`.
- **Do not launch the GUI to verify drawing.** Verify by unit test, `cargo test --workspace`, and code reading.
- **No dead code between tasks.** Each task wires what it defines. Do not add `#[allow(dead_code)]` to bridge a gap.
- Comments explain *why*, never *what*. Named constants, never magic numbers.

## File Structure

| File | Responsibility |
|---|---|
| `assets/fonts/unscii-16.ttf` | Map glyph font (Public Domain / CC-0) |
| `assets/fonts/DejaVuSansMono.ttf` | UI regular (Bitstream Vera) |
| `assets/fonts/DejaVuSansMono-Bold.ttf` | UI bold (Bitstream Vera) |
| `assets/fonts/LICENSE-unscii` | unscii license statement + source URL |
| `assets/fonts/LICENSE-dejavu` | Bitstream Vera license text |
| `crates/gui/tests/font_rasterization.rs` | Guards that unscii rasterizes pixel-crisp at every zoom step |
| `crates/gui/src/text.rs` | `Fonts`, `Metrics`, `map_cell`, `ui_metrics`, `terrain_color` + their tests |
| `crates/gui/src/lib.rs` | Loads `Fonts` in `game_loop`, threads into `render::draw`, `draw_toast` |
| `crates/gui/src/render.rs` | Routes all text through `Fonts` + `Metrics`; deletes both double-draws |
| `crates/gui/Cargo.toml` | `fontdue` dev-dependency |

---

### Task 1: Vendor the fonts and prove unscii rasterizes crisp

This task exists to retire the plan's biggest risk before any code is built on top of it. unscii ships as *vectorized outlines of a bitmap* — HEX and PCF are its only true bitmap formats, and macroquad's loader needs outlines — so it is only pixel-crisp if the rasterizer happens to land on the pixel grid. macroquad rasterizes with fontdue (`macroquad-0.4.15/src/text.rs:103` calls `font.rasterize(character, size as f32)`), so this checks the identical code path headlessly.

**If the test in this task cannot be made to pass, stop and report.** Do not proceed to Task 2 and do not weaken the tolerance to force a pass. The fallback is choosing different ladder sizes, which changes Task 2's constants.

**Files:**
- Create: `assets/fonts/unscii-16.ttf`, `assets/fonts/DejaVuSansMono.ttf`, `assets/fonts/DejaVuSansMono-Bold.ttf`
- Create: `assets/fonts/LICENSE-unscii`, `assets/fonts/LICENSE-dejavu`
- Create: `crates/gui/tests/font_rasterization.rs`
- Modify: `crates/gui/Cargo.toml`

**Interfaces:**
- Consumes: nothing.
- Produces: the three `.ttf` files at the paths above, which Task 5 loads via `include_bytes!`. Confirms the size ladder `[16, 32, 48, 64]` that Task 2 hardcodes as `MAP_GLYPH_NATIVE * zoom`.

- [ ] **Step 1: Write the failing test**

Create `crates/gui/tests/font_rasterization.rs`:

```rust
//! Guards the one empirical assumption the map font rests on.
//!
//! unscii ships as vectorized outlines of a bitmap rather than a real
//! bitmap — HEX and PCF are its only true bitmap formats, and macroquad's
//! loader needs outlines — so it is pixel-crisp only if the rasterizer
//! lands on the pixel grid. macroquad rasterizes with fontdue, so testing
//! fontdue directly exercises the same path without needing a GL context
//! or a window.

use std::sync::LazyLock;

const UNSCII: &[u8] = include_bytes!("../../../assets/fonts/unscii-16.ttf");

/// The sizes `text::map_cell` draws map glyphs at: 1x-4x unscii-16's
/// native 16px cell.
const LADDER: [f32; 4] = [16.0, 32.0, 48.0, 64.0];

/// How far a coverage byte may sit from fully-off or fully-on before it
/// counts as antialiasing rather than a hard pixel edge.
const BLUR_TOLERANCE: u8 = 24;

/// Parsed once and reused: the sweep below calls this 380 times (4 sizes ×
/// 95 chars), and re-parsing the 280 KB font per call costs ~11.7s against
/// ~6ms of actual rasterization.
static FONT: LazyLock<fontdue::Font> = LazyLock::new(|| {
    fontdue::Font::from_bytes(UNSCII, fontdue::FontSettings::default())
        .expect("unscii-16.ttf must parse as a font")
});

fn blurry_pixels(font: &fontdue::Font, size: f32, ch: char) -> Vec<u8> {
    font.rasterize(ch, size)
        .1
        .into_iter()
        .filter(|&c| c > BLUR_TOLERANCE && c < 255 - BLUR_TOLERANCE)
        .collect()
}

#[test]
fn unscii_rasterizes_crisp_at_every_map_zoom_step() {
    // Every species and structure glyph under assets/ is printable ASCII,
    // so this sweep covers everything the map can ever draw.
    for size in LADDER {
        for ch in ' '..='~' {
            let blurry = blurry_pixels(&FONT, size, ch);
            assert!(
                blurry.is_empty(),
                "{ch:?} at {size}px has {} antialiased pixels (coverages {:?}) \
                 — the glyph is not landing on the pixel grid",
                blurry.len(),
                &blurry[..blurry.len().min(8)]
            );
        }
    }
}
```

Add the dev-dependency to `crates/gui/Cargo.toml`. Pin to the version already resolved in `Cargo.lock` so no new crate enters the tree:

```toml
[dev-dependencies]
fontdue = "0.9"
```

- [ ] **Step 2: Run the test to verify it fails**

Run: `cargo test -p feral-processes-gui --test font_rasterization`

Expected: compilation failure, `couldn't read .../assets/fonts/unscii-16.ttf: No such file or directory`. This red state confirms the test is wired to the path Task 5 will load from.

- [ ] **Step 3: Vendor the font files**

unscii-16 (needs network):

```bash
mkdir -p assets/fonts
curl -fsSL -o assets/fonts/unscii-16.ttf \
  https://raw.githubusercontent.com/viznut/unscii/master/fontfiles/unscii-16.ttf
```

DejaVu is already installed — copy rather than download:

```bash
cp /usr/share/fonts/truetype/dejavu/DejaVuSansMono.ttf assets/fonts/
cp /usr/share/fonts/truetype/dejavu/DejaVuSansMono-Bold.ttf assets/fonts/
```

Verify all three are real TrueType files and none is an HTML error page:

```bash
file assets/fonts/*.ttf
```

Expected: each line reports `TrueType Font data`. If `unscii-16.ttf` reports HTML or is under ~1 KB, the download failed — stop and report rather than proceeding.

- [ ] **Step 4: Capture the licenses**

Write `assets/fonts/LICENSE-unscii`:

```
unscii-16.ttf — UNSCII by Viznut
Source: https://github.com/viznut/unscii  (fontfiles/unscii-16.ttf)

Upstream licensing statement, quoted from the project README:

  "You can consider it Public Domain (or CC-0) except for the files
  derived from or containing parts of Roman Czyborra's Unifont project
  (unifont.hex, hex2bdf.pl, unscii-16-full.*) which fall under GPL."

Only the base unscii-16.ttf is vendored here. unscii-16-full.* is GPL and
is deliberately NOT included.
```

Copy the Bitstream Vera license text, which ships with the installed package:

```bash
cp /usr/share/doc/fonts-dejavu-core/copyright assets/fonts/LICENSE-dejavu
```

Confirm it contains the license body, not just a stub:

```bash
grep -c "Bitstream Vera" assets/fonts/LICENSE-dejavu
```

Expected: a non-zero count.

- [ ] **Step 5: Run the test to verify it passes**

Run: `cargo test -p feral-processes-gui --test font_rasterization`

Expected: PASS, `test unscii_rasterizes_crisp_at_every_map_zoom_step ... ok`.

If it fails, the failure message names the glyph, size, and actual coverage values. Record them and stop — do not raise `BLUR_TOLERANCE` to force a pass. The tolerance may only be adjusted after inspecting real output, and a size that cannot be made clean means the ladder in Task 2 must change instead.

- [ ] **Step 6: Verify nothing else regressed**

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings; all tests pass.

- [ ] **Step 7: Checkpoint**

Do not commit. Report to the user that these files are ready to commit:

```
assets/fonts/unscii-16.ttf
assets/fonts/DejaVuSansMono.ttf
assets/fonts/DejaVuSansMono-Bold.ttf
assets/fonts/LICENSE-unscii
assets/fonts/LICENSE-dejavu
crates/gui/tests/font_rasterization.rs
crates/gui/Cargo.toml
```

Suggested message: `assets: vendor unscii-16 and DejaVu Sans Mono for the GUI`

---

### Task 2: `map_cell` — the zoom ladder and the dead zoom-4 step

`MAX_ZOOM` is 4 (`crates/app-core/src/lib.rs:189`), but `render.rs:307` does `app.zoom.clamp(1, 8)` and `render.rs:313` does `tile_px = 20.0 * zoom.min(3.0)`. Zoom 4 therefore renders identically to zoom 3 — pressing `+` at zoom 3 mutates state and changes nothing. The `clamp(1, 8)` is also stale against `MAX_ZOOM`. This is fixed here because the map glyph size is derived from `tile_px`.

**Files:**
- Create: `crates/gui/src/text.rs`
- Modify: `crates/gui/src/lib.rs` (add `mod text;`)
- Modify: `crates/gui/src/render.rs:306-313` and `render.rs:358`

**Interfaces:**
- Consumes: `feral_processes_app_core::{MIN_ZOOM, MAX_ZOOM}` (both already `pub`, declared at `app-core/src/lib.rs:188-189`).
- Produces: `text::map_cell(zoom: u16) -> (f32, u16)` returning `(tile_px, glyph_px)`. Task 5 uses the `glyph_px` return to size map glyphs.

- [ ] **Step 1: Write the failing tests**

Create `crates/gui/src/text.rs`:

```rust
//! Fonts and text sizing for the graphics frontend.
//!
//! Split out of `render.rs` for the same reason `fx.rs` was: the sizing
//! and color math is pure and worth unit-testing, while the drawing that
//! consumes it can't be tested at all without a window.
//!
//! Two independent sizing rules live here. Map glyphs are sized by zoom
//! alone, in strict integer multiples of the pixel font's native cell.
//! UI text is sized continuously from the window height. They never mix.

use feral_processes_app_core::{MAX_ZOOM, MIN_ZOOM};

/// unscii-16's native cell height. Map glyphs are only ever drawn at
/// integer multiples of this, so the vectorized bitmap keeps landing on
/// the pixel grid instead of resampling into mush.
const MAP_GLYPH_NATIVE: u16 = 16;
/// Tile edge at zoom 1, leaving a native glyph a margin inside its cell.
const BASE_TILE_PX: f32 = 20.0;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_zoom_step_gets_its_own_tile_size() {
        // Zoom 4 used to render identically to zoom 3, so `+` at zoom 3
        // was a keypress that changed state and nothing else.
        let sizes: Vec<f32> = (MIN_ZOOM..=MAX_ZOOM).map(|z| map_cell(z).0).collect();
        for pair in sizes.windows(2) {
            assert!(
                pair[1] > pair[0],
                "each zoom step must grow the tile: {sizes:?}"
            );
        }
    }

    #[test]
    fn map_glyphs_always_land_on_an_integer_multiple_of_the_native_cell() {
        for z in MIN_ZOOM..=MAX_ZOOM {
            let (_, glyph) = map_cell(z);
            assert_eq!(
                glyph % MAP_GLYPH_NATIVE,
                0,
                "zoom {z} wants a {glyph}px glyph, which is off the pixel grid"
            );
        }
    }

    #[test]
    fn map_cell_clamps_zoom_outside_the_supported_range() {
        assert_eq!(map_cell(0), map_cell(MIN_ZOOM));
        assert_eq!(map_cell(99), map_cell(MAX_ZOOM));
    }

    #[test]
    fn zooms_one_through_three_keep_the_sizes_they_already_had() {
        assert_eq!(map_cell(1), (20.0, 16));
        assert_eq!(map_cell(2), (40.0, 32));
        assert_eq!(map_cell(3), (60.0, 48));
    }
}
```

Register the module in `crates/gui/src/lib.rs`, alphabetically among the existing `mod` lines (currently `mod fx; mod render; mod sounds;` at lines 8-10):

```rust
mod fx;
mod render;
mod sounds;
mod text;
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-gui map_cell`

Expected: FAIL to compile — `cannot find function 'map_cell' in this scope`, four times.

- [ ] **Step 3: Implement `map_cell`**

Add to `crates/gui/src/text.rs`, between the constants and the `tests` module:

```rust
/// Tile edge and glyph size in pixels for a zoom step.
///
/// Map sizing is driven by zoom alone and never by window size: a larger
/// window shows *more tiles at the same size*, which is what the TUI
/// already does and what reads correctly for a grid.
pub fn map_cell(zoom: u16) -> (f32, u16) {
    let z = zoom.clamp(MIN_ZOOM, MAX_ZOOM);
    (BASE_TILE_PX * z as f32, MAP_GLYPH_NATIVE * z)
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-gui map_cell`

Expected: PASS, four tests.

- [ ] **Step 5: Prove the bug still exists in render.rs**

Run: `grep -n "clamp(1, 8)\|zoom.min(3.0)\|tile_px \* 0.8" crates/gui/src/render.rs`

Expected output, confirming all three stale expressions are still present:

```
307:    let zoom = app.zoom.clamp(1, 8) as f32;
313:    let tile_px = 20.0 * zoom.min(3.0);
358:            let font_size = (tile_px * 0.8).max(14.0);
```

- [ ] **Step 6: Wire render.rs to `map_cell`**

In `crates/gui/src/render.rs`, replace lines 307 and 313. The current code reads:

```rust
fn draw_playing_base(app: &mut App, fx: &Fx) {
    let zoom = app.zoom.clamp(1, 8) as f32;
    let status_line = app.status_line.clone();
    let Some(game) = &mut app.game else { return };

    let map_w = screen_width() * 0.7;
    let map_h = screen_height() * 0.72;
    let tile_px = 20.0 * zoom.min(3.0);
```

Change it to:

```rust
fn draw_playing_base(app: &mut App, fx: &Fx) {
    let (tile_px, glyph_px) = crate::text::map_cell(app.zoom);
    let status_line = app.status_line.clone();
    let Some(game) = &mut app.game else { return };

    let map_w = screen_width() * 0.7;
    let map_h = screen_height() * 0.72;
```

Then at line 358, the current glyph sizing:

```rust
            let font_size = (tile_px * 0.8).max(14.0);
            let glyph = ch.to_string();
            let dims = measure_text(&glyph, None, font_size as u16, 1.0);
```

becomes:

```rust
            let glyph = ch.to_string();
            let dims = measure_text(&glyph, None, glyph_px, 1.0);
```

Every later use of `font_size` inside that loop (the two `draw_text` calls at lines 368 and 370) takes `glyph_px as f32` instead. Task 5 replaces those calls entirely; for now they just need to compile.

Add the import alongside the existing `use crate::fx::Fx;`:

```rust
use crate::text::map_cell;
```

and call it as `map_cell(app.zoom)` rather than the fully-qualified path.

- [ ] **Step 7: Verify the bug is gone and nothing regressed**

Run: `grep -n "clamp(1, 8)\|zoom.min(3.0)" crates/gui/src/render.rs`

Expected: no output.

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings; all tests pass.

- [ ] **Step 8: Checkpoint**

Do not commit. Report these files as ready:

```
crates/gui/src/text.rs
crates/gui/src/lib.rs
crates/gui/src/render.rs
```

Suggested message: `gui: derive map tile and glyph size from zoom, fixing the dead zoom-4 step`

---

### Task 3: `Metrics` — window-relative UI sizing

`FONT_SIZE` (`render.rs:20`) and `LINE_HEIGHT` (`render.rs:21`) are fixed at `24.0` and `30.0`, tuned against the 900px height in `window_conf()`. This task replaces both constants with values derived from the actual window height, and replaces the padding literals tuned alongside them.

The font itself does **not** change in this task. That is deliberate: with the font held constant, a layout shift at the default 1440×900 window is unambiguously a consequence of this refactor rather than of new glyph metrics.

Three shifts are expected and accepted — a ratio system cannot reproduce every ad-hoc literal, and a special case for each is not worth the complexity: the status banner's left inset goes 12→10px, the keybind hints go 21→20px, and `keys_line_height` goes 26→24px, which puts the five-line keybind block 10px lower. Anything beyond those three is a bug.

**Files:**
- Modify: `crates/gui/src/text.rs`
- Modify: `crates/gui/src/render.rs` (all 24 `FONT_SIZE` and 23 `LINE_HEIGHT` references)

**Interfaces:**
- Consumes: `text::map_cell` from Task 2 (unchanged).
- Produces: `text::Metrics { font_size: u16, line_height: f32, pad: f32, inset: f32, gap: f32 }` with methods `title() -> u16`, `label() -> u16`, `small() -> u16`; and `text::ui_metrics(window_height: f32) -> Metrics`. Task 5 passes a `&Metrics` alongside `&Fonts` to every drawing helper.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/gui/src/text.rs`:

```rust
    #[test]
    fn ui_metrics_reproduces_todays_sizes_at_the_reference_window_height() {
        // The whole refactor rests on this: at the default window, every
        // number must come out exactly where it is hardcoded today, so a
        // layout shift at default size means the refactor is wrong.
        let m = ui_metrics(REFERENCE_HEIGHT);
        assert_eq!(m.font_size, 24);
        assert_eq!(m.line_height, 30.0);
        assert_eq!(m.pad, 16.0);
        assert_eq!(m.inset, 10.0);
        assert_eq!(m.gap, 6.0);
        assert_eq!(m.title(), 28);
        assert_eq!(m.label(), 22);
        assert_eq!(m.small(), 20);
    }

    #[test]
    fn ui_metrics_clamps_at_both_extremes() {
        assert_eq!(ui_metrics(1.0).font_size, MIN_UI_FONT);
        assert_eq!(ui_metrics(0.0).font_size, MIN_UI_FONT);
        assert_eq!(ui_metrics(100_000.0).font_size, MAX_UI_FONT);
    }

    #[test]
    fn ui_metrics_keeps_lines_taller_than_their_text_at_every_window_size() {
        for h in (100..4000).step_by(37) {
            let m = ui_metrics(h as f32);
            assert!(
                m.line_height > m.font_size as f32,
                "line height collapsed onto the font at window height {h}"
            );
        }
    }

    #[test]
    fn ui_metrics_keeps_the_size_ramp_ordered_including_at_the_clamps() {
        for h in (100..4000).step_by(37) {
            let m = ui_metrics(h as f32);
            assert!(m.small() < m.font_size, "small() inverted at height {h}");
            assert!(m.label() < m.font_size, "label() inverted at height {h}");
            assert!(m.font_size < m.title(), "title() inverted at height {h}");
        }
    }

    #[test]
    fn ui_metrics_scales_monotonically_between_the_clamps() {
        let mut previous = 0;
        for h in (100..4000).step_by(37) {
            let size = ui_metrics(h as f32).font_size;
            assert!(size >= previous, "font shrank as the window grew at {h}");
            previous = size;
        }
    }
```

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-gui ui_metrics`

Expected: FAIL to compile — `cannot find function 'ui_metrics' in this scope`, plus unresolved `REFERENCE_HEIGHT`, `MIN_UI_FONT`, `MAX_UI_FONT`.

- [ ] **Step 3: Implement `Metrics` and `ui_metrics`**

Add to `crates/gui/src/text.rs`, above the `tests` module:

```rust
/// The `window_conf()` height every hardcoded size in `render.rs` was
/// originally tuned against.
const REFERENCE_HEIGHT: f32 = 900.0;
const BASE_UI_FONT: f32 = 24.0;
const MIN_UI_FONT: u16 = 16;
const MAX_UI_FONT: u16 = 40;
/// Preserves the 30.0 / 24.0 relationship the fixed constants had.
const LINE_HEIGHT_RATIO: f32 = 1.25;
/// Ratios chosen to reproduce the existing literals exactly at
/// `BASE_UI_FONT`: 16.0, 10.0 and 6.0 respectively.
const PAD_RATIO: f32 = 2.0 / 3.0;
const INSET_RATIO: f32 = 5.0 / 12.0;
const GAP_RATIO: f32 = 0.25;

/// Every UI dimension that used to be a literal in `render.rs`, scaled to
/// the current window.
pub struct Metrics {
    pub font_size: u16,
    pub line_height: f32,
    /// Inset from a popup's edge to its content.
    pub pad: f32,
    /// Inset from a panel's edge to its content.
    pub inset: f32,
    /// Vertical breathing space between groups of rows.
    pub gap: f32,
}

impl Metrics {
    /// Popup titles.
    pub fn title(&self) -> u16 {
        self.font_size + 4
    }

    /// Bar labels.
    pub fn label(&self) -> u16 {
        self.font_size - 2
    }

    /// Keybind hints and scroll indicators. These are currently 3px and
    /// 4px below the body font respectively; a one-pixel difference
    /// between two unrelated bits of chrome isn't worth carrying through
    /// a scaling system, so both collapse to one size.
    pub fn small(&self) -> u16 {
        self.font_size - 4
    }
}

/// UI text scales continuously with window height, unlike map glyphs.
/// `TextParams::font_size` is a `u16`, so sizes are already quantized to
/// whole pixels and the font atlas gains at most a few dozen entries over
/// a resize drag — no separate stepping scheme is needed.
pub fn ui_metrics(window_height: f32) -> Metrics {
    let scaled = (BASE_UI_FONT * window_height / REFERENCE_HEIGHT).round();
    let font_size = (scaled as u16).clamp(MIN_UI_FONT, MAX_UI_FONT);
    let f = font_size as f32;
    Metrics {
        font_size,
        line_height: f * LINE_HEIGHT_RATIO,
        pad: f * PAD_RATIO,
        inset: f * INSET_RATIO,
        gap: f * GAP_RATIO,
    }
}
```

`MIN_UI_FONT` is 16, so `small()`'s `- 4` cannot underflow.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-gui ui_metrics`

Expected: PASS, five tests.

- [ ] **Step 5: Replace the constants in render.rs**

Delete `const FONT_SIZE: f32 = 24.0;` and `const LINE_HEIGHT: f32 = 30.0;` from `crates/gui/src/render.rs:20-21`.

Thread a `&Metrics` through every function that draws text. `draw` computes it once per frame from `screen_height()` and passes it down:

```rust
pub fn draw(app: &mut App, fx: &mut Fx) {
    let m = ui_metrics(screen_height());
    clear_background(Color::new(0.02, 0.02, 0.03, 1.0));
    match app.mode {
        Mode::MainMenu => draw_main_menu(app, &m),
        // ... every arm gains `&m`
    }
    if let Some(status) = &app.status_line
        && needs_status_banner(app.mode)
    {
        draw_status_banner(status, &m);
    }
}
```

Apply this substitution at every site. The mapping is mechanical — for
example `render.rs:246` currently reads:

```rust
        Row::Text(s) => {
            draw_text(s, x + 16.0, cy, FONT_SIZE, TEXT_DIM);
        }
```

and becomes:

```rust
        Row::Text(s) => {
            draw_text(s, x + m.pad, cy, m.font_size as f32, TEXT_DIM);
        }
```

The full mapping:

| Was | Becomes |
|---|---|
| `FONT_SIZE` | `m.font_size as f32` |
| `FONT_SIZE + 4.0` | `m.title() as f32` |
| `FONT_SIZE - 2.0` | `m.label() as f32` |
| `FONT_SIZE - 3.0`, `FONT_SIZE - 4.0` | `m.small() as f32` |
| `LINE_HEIGHT` | `m.line_height` |
| `LINE_HEIGHT - 4.0` (`render.rs:528`) | `m.line_height - m.gap` |
| popup content inset `x + 16.0` | `x + m.pad` |
| panel content inset `x + 10.0` | `x + m.inset` |
| banner inset `12.0` (`render.rs:75`) | `m.inset` |
| vertical group gaps `cy += 6.0` | `cy += m.gap` |

Functions needing the new parameter, with their current signatures:

- `draw_message_line(kind, text, x, y)` → add `m: &Metrics`
- `draw_status_banner(status)` → add `m: &Metrics`
- `draw_popup(title, size, rows)` → add `m: &Metrics`
- `draw_row(row, x, w, cy, max_y)` → add `m: &Metrics`
- `draw_playing_base(app, fx)` → add `m: &Metrics`
- `draw_status_panel(x, y, w, h, status)` → add `m: &Metrics`
- `draw_bar(x, y, w, label, value, max, color)` → add `m: &Metrics`
- `draw_ghost_band(x, y, w, value, ghost, max, color)` → add `m: &Metrics`
- `draw_mode_overlay(app)` → add `m: &Metrics`
- `draw_battle(app, fx)` → add `m: &Metrics`
- every `draw_*_menu`, `draw_*_prompt`, `draw_help`, `draw_main_menu`, `draw_load_game`, `draw_save_action`, `draw_difficulty_pick`, `draw_game_over`, `draw_inventory*`, `draw_inspect_detail`, `draw_craft_quantity`, `draw_erase_quantity` → add `m: &Metrics`

Leave the geometry that is **not** UI-font-relative alone:

- `BASE_TILE_PX` and the `20.0` inside `map_cell` — map geometry, owned by Task 2
- `map_w = screen_width() * 0.7`, `map_h = screen_height() * 0.72` — pane proportions
- `2.0` border thickness in every `draw_rectangle_lines` call
- every float inside a `Color::new(...)` — those are channel values
- popup size fractions `0.88 / 0.85 / 0.5`

The `20.0` in `map_cell` and the `20.0` UI insets are spelled identically but mean different things. Misclassifying map geometry as UI padding is the most likely way to get this task wrong — check each `20.0` against which function it sits in.

Add the import:

```rust
use crate::text::{Metrics, map_cell, ui_metrics};
```

Update the two `render::draw` callers: `crates/gui/src/lib.rs:147`.

- [ ] **Step 6: Verify**

Run: `grep -n "FONT_SIZE\|LINE_HEIGHT" crates/gui/src/render.rs`

Expected: no output.

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings; all tests pass.

- [ ] **Step 7: Checkpoint**

Do not commit. Report these files as ready:

```
crates/gui/src/text.rs
crates/gui/src/render.rs
crates/gui/src/lib.rs
```

Suggested message: `gui: scale UI text and padding to window height`

---

### Task 4: `terrain_color` — emphasis by saturation

Delete the tile-loop double-draw and separate terrain from entities by desaturating terrain instead.

**The discriminator is saturation, not brightness, and the reason matters.** Dimming terrain below the damaged-structure floor does not work: `fx::structure_condition` multiplies a structure's glyph color by a tint bottoming out at `MIN_TINT = 0.45`, and `GlyphColor::DarkGreen` is `(0.0, 0.4, 0.0)`. A damaged DarkGreen structure therefore sits at roughly 0.11 luminance — for terrain to be reliably dimmer it would have to be nearly black. Desaturation has no such failure mode: however dark or damaged an entity gets, it stays the only saturated glyph on the map.

**Files:**
- Modify: `crates/gui/src/text.rs`
- Modify: `crates/gui/src/render.rs:330`, `render.rs:337-352`, `render.rs:363-370`

**Interfaces:**
- Consumes: `text::Metrics` from Task 3 (unchanged).
- Produces: `text::terrain_color(Color) -> Color`. Nothing later depends on it.

- [ ] **Step 1: Write the failing tests**

Add to the `tests` module in `crates/gui/src/text.rs`:

```rust
    /// The map palette, copied from `render.rs`'s `biome_style` and
    /// `glyph_color`. Duplicated rather than imported because those are
    /// private to the renderer and this only needs representative values.
    const PALETTE: [Color; 8] = [
        Color::new(0.3, 0.55, 0.95, 1.0),  // BLUE
        Color::new(0.9, 0.25, 0.25, 1.0),  // RED
        Color::new(0.25, 0.85, 0.85, 1.0), // CYAN
        Color::new(0.35, 0.85, 0.4, 1.0),  // GREEN
        Color::new(0.95, 0.55, 0.15, 1.0), // ORANGE
        Color::new(0.8, 0.35, 0.85, 1.0),  // MAGENTA
        Color::new(0.0, 0.4, 0.0, 1.0),    // DarkGreen — the dark extreme
        Color::new(0.55, 0.27, 0.07, 1.0), // Brown
    ];

    fn luminance(c: Color) -> f32 {
        0.299 * c.r + 0.587 * c.g + 0.114 * c.b
    }

    /// Chroma: how far apart the channels are, standing in for saturation.
    fn spread(c: Color) -> f32 {
        c.r.max(c.g).max(c.b) - c.r.min(c.g).min(c.b)
    }

    #[test]
    fn terrain_color_desaturates_every_palette_color() {
        for c in PALETTE {
            assert!(
                spread(terrain_color(c)) < spread(c),
                "{c:?} came back no less saturated than it went in"
            );
        }
    }

    #[test]
    fn terrain_color_shrinks_saturation_by_exactly_the_configured_factor() {
        // Desaturating and dimming are both affine on each channel, so the
        // channel spread scales by exactly the product of the two factors.
        for c in PALETTE {
            let expected = spread(c) * TERRAIN_SATURATION * TERRAIN_BRIGHTNESS;
            assert!(
                (spread(terrain_color(c)) - expected).abs() < 1e-5,
                "{c:?}: expected spread {expected}, got {}",
                spread(terrain_color(c))
            );
        }
    }

    #[test]
    fn terrain_color_dims_luminance_by_exactly_the_brightness_factor() {
        // Luminance-weighted desaturation is luminance-preserving, so the
        // only thing left acting on luminance is the brightness factor.
        for c in PALETTE {
            let expected = luminance(c) * TERRAIN_BRIGHTNESS;
            assert!(
                (luminance(terrain_color(c)) - expected).abs() < 1e-5,
                "{c:?}: expected luminance {expected}, got {}",
                luminance(terrain_color(c))
            );
        }
    }

    #[test]
    fn terrain_color_leaves_a_grey_input_grey() {
        let grey = Color::new(0.5, 0.5, 0.5, 1.0);
        let out = terrain_color(grey);
        assert!(
            (out.r - out.g).abs() < 1e-6 && (out.g - out.b).abs() < 1e-6,
            "grey picked up a color cast: {out:?}"
        );
    }

    #[test]
    fn terrain_color_preserves_alpha() {
        for c in PALETTE {
            assert_eq!(terrain_color(c).a, c.a);
        }
    }
```

This is the task that first needs macroquad types in `text.rs` — Tasks 2 and 3 are pure arithmetic over `f32`/`u16`. Add to the top of the file:

```rust
use macroquad::prelude::*;
```

The same glob covers `Font`, `TextParams`, `TextDimensions` and `FilterMode` for Task 5, so no further imports are needed there.

- [ ] **Step 2: Run the tests to verify they fail**

Run: `cargo test -p feral-processes-gui terrain_color`

Expected: FAIL to compile — `cannot find function 'terrain_color' in this scope`, plus unresolved `TERRAIN_SATURATION` and `TERRAIN_BRIGHTNESS`, which Step 3 introduces alongside it.

- [ ] **Step 3: Implement `terrain_color`**

Add to `crates/gui/src/text.rs`:

```rust
/// Terrain keeps a quarter of its hue and the rest goes to grey, leaving
/// entity glyphs the only saturated thing on the map.
///
/// Brightness can't be the discriminator: `fx::structure_condition` dims a
/// damaged structure toward a floor of `MIN_TINT`, and `GlyphColor::DarkGreen`
/// is already `(0.0, 0.4, 0.0)`, so a damaged one sits near 0.11 luminance.
/// Terrain would have to be nearly black to stay reliably dimmer than that.
/// Saturation has no equivalent failure case.
const TERRAIN_SATURATION: f32 = 0.25;
const TERRAIN_BRIGHTNESS: f32 = 0.70;

/// Pushes a biome's glyph color back toward grey so entities read out of
/// the terrain without needing a faked bold weight.
pub fn terrain_color(c: Color) -> Color {
    let luminance = 0.299 * c.r + 0.587 * c.g + 0.114 * c.b;
    let toward_grey =
        |channel: f32| (luminance + (channel - luminance) * TERRAIN_SATURATION) * TERRAIN_BRIGHTNESS;
    Color::new(
        toward_grey(c.r),
        toward_grey(c.g),
        toward_grey(c.b),
        c.a,
    )
}
```

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-gui terrain_color`

Expected: PASS, five tests.

- [ ] **Step 5: Apply it in the tile loop and delete the double-draw**

In `crates/gui/src/render.rs`, the tile loop currently reads (lines 330-352, abridged):

```rust
            let (mut ch, mut color) = biome_style(tile.biome);
            let px = rx as f32 * tile_px;
            let py = ry as f32 * tile_px;
            let mut bold = false;
            let mut staffed = false;
            let mut shielded = false;
            let mut critical = false;
            for ev in &entities {
                // ...
                if erx == rx as i32 && ery == ry as i32 {
                    ch = ev.glyph;
                    color = glyph_color(ev.color);
                    bold = ev.is_structure || ev.is_boss;
                    staffed = ev.is_structure && ev.structure_worker.is_some();
                    (color, critical) = fx.structure_condition(ev.durability, color);
                    shielded = ev.is_structure;
                }
            }
```

Change the initial biome color to be desaturated, and drop `bold` entirely:

```rust
            let (mut ch, biome_color) = biome_style(tile.biome);
            let mut color = terrain_color(biome_color);
            let px = rx as f32 * tile_px;
            let py = ry as f32 * tile_px;
            let mut staffed = false;
            let mut shielded = false;
            let mut critical = false;
            for ev in &entities {
                // ...
                if erx == rx as i32 && ery == ry as i32 {
                    ch = ev.glyph;
                    color = glyph_color(ev.color);
                    staffed = ev.is_structure && ev.structure_worker.is_some();
                    (color, critical) = fx.structure_condition(ev.durability, color);
                    shielded = ev.is_structure;
                }
            }
```

The tile background at line 353 must keep deriving from the **pre-desaturation** color so biomes keep their identity and the critical red wash lands where it does now:

```rust
            let mut bg = Color::new(
                biome_color.r * 0.18,
                biome_color.g * 0.18,
                biome_color.b * 0.18,
                1.0,
            );
```

Note this changes background behaviour for entity tiles, which previously derived their background from the *entity* color. Keep that: capture the color used for the background separately from the glyph color, so an entity tile still tints its background from the entity.

```rust
            let mut bg_source = biome_color;
```

set `bg_source = glyph_color(ev.color);` inside the entity branch, and derive `bg` from `bg_source`.

Then delete the faux-bold at lines 363-370. Current:

```rust
            // Same faux-bold trick `draw_message_line` uses — macroquad has
            // no bold font loaded, so weight is drawing the glyph twice a
            // pixel apart. Structures and bosses get it so they read out of
            // the terrain, matching the TUI's bold styling for them.
            if bold {
                draw_text(&glyph, tx + 1.0, ty, font_size, color);
            }
            draw_text(&glyph, tx, ty, font_size, color);
```

becomes:

```rust
            draw_text(&glyph, tx, ty, glyph_px as f32, color);
```

Add `terrain_color` to the `use crate::text::{...}` import.

- [ ] **Step 6: Verify**

Run: `grep -n "bold" crates/gui/src/render.rs`

Expected: only the `draw_message_line` occurrence around line 47 remains — that one is Task 5's to remove, since it needs the bold font file.

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings; all tests pass.

- [ ] **Step 7: Checkpoint**

Do not commit. Report these files as ready:

```
crates/gui/src/text.rs
crates/gui/src/render.rs
```

Suggested message: `gui: separate terrain from entities by saturation, dropping the tile double-draw`

---

### Task 5: `Fonts` — load the faces and route all text through them

The last task. Everything so far still draws with macroquad's default font; this replaces every `draw_text` with `draw_text_ex` carrying an explicit face, retires the second double-draw using a real bold weight, and swaps the two ASCII scroll indicators for real arrows.

There is no unit test here — loading a font requires a GL context and drawing can't be asserted on. Verification is the full suite still passing, clippy clean, and the greps in Step 5.

**Files:**
- Modify: `crates/gui/src/text.rs`
- Modify: `crates/gui/src/lib.rs`
- Modify: `crates/gui/src/render.rs`

**Interfaces:**
- Consumes: `text::{Metrics, ui_metrics, map_cell, terrain_color}` from Tasks 2-4; the three `.ttf` files from Task 1.
- Produces: `text::Fonts` with `Fonts::load() -> Fonts`, `ui(&self, text, x, y, size, color)`, `ui_bold(&self, ...)`, `map(&self, glyph, x, y, size, color)`, `measure_ui(&self, text, size) -> TextDimensions`, `measure_map(&self, glyph, size) -> TextDimensions`.

- [ ] **Step 1: Implement `Fonts`**

Add to `crates/gui/src/text.rs`:

```rust
/// The three faces the frontend draws with: a pixel font for the map grid
/// and a vector monospace, regular and bold, for everything else.
///
/// Embedded with `include_bytes!` rather than loaded from `assets_dir` for
/// the same reason the sound effects are (see `sounds.rs`): fonts aren't
/// moddable game content.
pub struct Fonts {
    map: Font,
    ui: Font,
    ui_bold: Font,
}

impl Fonts {
    /// Not async, unlike `SoundBank::load` — macroquad's font loader is
    /// synchronous — but it still reaches for the graphics context, so it
    /// has to run after the window exists.
    pub fn load() -> Self {
        let mut map = load_ttf_font_from_bytes(include_bytes!(
            "../../../assets/fonts/unscii-16.ttf"
        ))
        .expect("embedded unscii-16 is valid ttf");
        // unscii is vectorized outlines of a bitmap, so it only stays
        // crisp under nearest-neighbour sampling. The loader applies the
        // context default, which is linear.
        map.set_filter(FilterMode::Nearest);
        Self {
            map,
            ui: load_ttf_font_from_bytes(include_bytes!(
                "../../../assets/fonts/DejaVuSansMono.ttf"
            ))
            .expect("embedded DejaVu Sans Mono is valid ttf"),
            ui_bold: load_ttf_font_from_bytes(include_bytes!(
                "../../../assets/fonts/DejaVuSansMono-Bold.ttf"
            ))
            .expect("embedded DejaVu Sans Mono Bold is valid ttf"),
        }
    }

    pub fn ui(&self, text: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font: Some(&self.ui),
                font_size: size,
                color,
                ..Default::default()
            },
        );
    }

    pub fn ui_bold(&self, text: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        draw_text_ex(
            text,
            x,
            y,
            TextParams {
                font: Some(&self.ui_bold),
                font_size: size,
                color,
                ..Default::default()
            },
        );
    }

    pub fn map(&self, glyph: impl AsRef<str>, x: f32, y: f32, size: u16, color: Color) {
        draw_text_ex(
            glyph,
            x,
            y,
            TextParams {
                font: Some(&self.map),
                font_size: size,
                color,
                ..Default::default()
            },
        );
    }

    pub fn measure_ui(&self, text: impl AsRef<str>, size: u16) -> TextDimensions {
        measure_text(text, Some(&self.ui), size, 1.0)
    }

    pub fn measure_map(&self, glyph: impl AsRef<str>, size: u16) -> TextDimensions {
        measure_text(glyph, Some(&self.map), size, 1.0)
    }
}
```

`impl AsRef<str>` rather than `&str` deliberately: it matches macroquad's own
`draw_text_ex` signature, and many call sites in `render.rs` pass an owned
`String` from `format!` (for example `render.rs:256`). Taking `&str` would
force a `&` at every one of those sites.

`FilterMode` needs no extra import — `macroquad::texture` re-exports it and
the prelude glob re-exports that in turn.

`expect` on embedded assets follows the existing precedent in `sounds.rs:25` and matches CLAUDE.md's carve-out for startup config that should abort anyway.

- [ ] **Step 2: Load it in the game loop**

In `crates/gui/src/lib.rs`, alongside the existing `SoundBank::load().await` at line 89:

```rust
async fn game_loop(mut app: App) {
    let sound_bank = SoundBank::load().await;
    let fonts = text::Fonts::load();
    let mut volume = DEFAULT_VOLUME;
```

Change the draw call at line 147:

```rust
        render::draw(&mut app, &mut fx, &fonts);
```

`draw_toast` at lines 62-75 uses `measure_text(..., None, ...)` and `draw_text`. Give it the fonts:

```rust
fn draw_toast(fonts: &text::Fonts, text: &str) {
    let font_size = 28;
    let dims = fonts.measure_ui(text, font_size);
    let x = (screen_width() - dims.width) / 2.0;
    let y = 44.0;
    draw_rectangle(
        x - 14.0,
        y - dims.height - 10.0,
        dims.width + 28.0,
        dims.height + 22.0,
        Color::new(0.06, 0.07, 0.10, 0.85),
    );
    fonts.ui(text, x, y, font_size, Color::new(0.92, 0.92, 0.92, 1.0));
}
```

and its call site at line 151 becomes `draw_toast(&fonts, text);`.

- [ ] **Step 3: Route render.rs through the fonts**

Add `fonts: &Fonts` as a parameter to `pub fn draw` and to every helper that already gained `m: &Metrics` in Task 3. Keep the parameter order consistent: `(..., fonts: &Fonts, m: &Metrics)`.

Replace every remaining `draw_text(...)` call. The transformation:

```rust
draw_text(text, x, y, size_f32, color)
```

becomes

```rust
fonts.ui(text, x, y, size_u16, color)
```

where `size_u16` is the `m.font_size` / `m.title()` / `m.label()` / `m.small()` value directly, without the `as f32` Task 3 introduced. The map glyph call in the tile loop uses `fonts.map(&glyph, tx, ty, glyph_px, color)` instead.

The two `measure_text(..., None, ...)` calls become `fonts.measure_ui(...)` at `render.rs:66` (status banner) and `fonts.measure_map(...)` at `render.rs:360` (glyph centering).

- [ ] **Step 4: Retire the last double-draw and fix the scroll indicators**

`draw_message_line` at `crates/gui/src/render.rs:40-51` currently reads:

```rust
fn draw_message_line(kind: MessageKind, text: &str, x: f32, y: f32) {
    let color = match kind {
        MessageKind::Info => TEXT_DIM,
        MessageKind::Loot => GREEN,
        MessageKind::LevelUp => GREEN,
        MessageKind::Raid => ORANGE,
    };
    if kind == MessageKind::LevelUp {
        draw_text(text, x + 1.0, y, FONT_SIZE, color);
    }
    draw_text(text, x, y, FONT_SIZE, color);
}
```

With a real bold weight it becomes:

```rust
/// Display styling for a message-log line, chosen by the engine-supplied
/// `MessageKind` rather than by sniffing the text — low-priority chatter
/// stays dim, gains and damage that matter get a color.
fn draw_message_line(fonts: &Fonts, m: &Metrics, kind: MessageKind, text: &str, x: f32, y: f32) {
    let color = match kind {
        MessageKind::Info => TEXT_DIM,
        MessageKind::Loot => GREEN,
        MessageKind::LevelUp => GREEN,
        MessageKind::Raid => ORANGE,
    };
    if kind == MessageKind::LevelUp {
        fonts.ui_bold(text, x, y, m.font_size, color);
    } else {
        fonts.ui(text, x, y, m.font_size, color);
    }
}
```

Note the doc comment loses its sentence about macroquad having no bold font — that constraint no longer exists.

The scroll indicators at `render.rs:206` and `render.rs:222` used ASCII because the default font had nothing better. DejaVu Sans Mono covers the arrows:

```rust
                format!("↑ {scroll_offset} more above")
```

```rust
                format!("↓ {below} more below")
```

- [ ] **Step 5: Verify**

Run: `grep -n "draw_text(\|measure_text(" crates/gui/src/render.rs crates/gui/src/lib.rs`

Expected: no output. Every call now goes through `Fonts`.

Run: `grep -n "faux-bold\|twice a\|no bold font" crates/gui/src/render.rs`

Expected: no output. Both hack sites and their explanatory comments are gone.

Run: `cargo fmt && cargo clippy --workspace && cargo test --workspace`

Expected: no warnings; all tests pass, including `unscii_rasterizes_crisp_at_every_map_zoom_step` from Task 1.

- [ ] **Step 6: Checkpoint**

Do not commit. Report these files as ready:

```
crates/gui/src/text.rs
crates/gui/src/lib.rs
crates/gui/src/render.rs
```

Suggested message: `gui: draw with unscii-16 on the map and DejaVu Sans Mono in the UI`

Then tell the user phase 1 is complete and that this is the natural point to launch the game themselves and look at it — the plan deliberately never does, and no automated check can confirm the result looks right.

---

## Out of Scope

Deferred to later phases, per the spec and `graphics-upgrade-four-phase-plan` in memory:

- **Phase 2 (map layer):** sub-tile camera lerp, per-tile shade variation, distance falloff, durability bars on damaged structures
- **Phase 3 (effects layer):** screen shake, destroy particles, world-space loot/XP floats, battle-screen rework
- **Phase 4 (TUI parity):** raid flash, damaged-structure dimming, log flash in `crates/tui/src/ui.rs`

Also deliberately not done here: replacing prose hyphens with em-dashes for TUI parity. That is cosmetic churn across strings this phase has no other reason to touch. The two scroll indicators are the exception because those exact lines are edited for metrics anyway.
