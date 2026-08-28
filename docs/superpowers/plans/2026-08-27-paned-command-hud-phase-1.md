# Paned Command HUD — Phase 1: geometry and the status bar

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Land the HUD's five-region geometry and the new status bar, with every
region live from this phase — no dead code and no doubled chrome.

**Architecture:** A new `crates/gui/src/render/hud/` module. `layout.rs` is pure
arithmetic over `(screen_w, screen_h, char_w)` and never sees a `Painter`, which
is what makes the geometry unit-testable headlessly. `palette.rs` holds the
handoff's 16 semantic roles. `status_bar.rs` absorbs today's stock strip as its
centre zone. `draw_playing_base` stops computing its own rects and reads them
from `hud::layout`; the **existing** status panel and log pane are repointed into
the new `info_column` and `log_pane` rects, so the screen is coherent at the end
of this phase and their *contents* are replaced in phases 3 and 5.

**Tech Stack:** Rust, `crates/gui` only. No engine or app-core change in this
phase. `bevy_egui` via the existing `Painter` seam.

**Spec:** `docs/superpowers/specs/2026-08-27-paned-command-hud-design.md`

## Global Constraints

- **`crates/gui/src/paint.rs` is the only file that names a graphics library.**
  Everything here draws through `Painter`. Do not add a backend call to
  `render/`.
- **The drawing seam takes an origin from its caller.** Panes take a `Rect`, never
  a bare width and height. A literal `0.0` for a y-origin draws under the status
  bar and no test sees it.
- **Widths are measured, never estimated.** `Painter::measure_ui_advance` is the
  one width question. The UI font is DejaVu Sans Mono, not the handoff's assumed
  0.6-advance face, so a character count is not a width.
- **What does not fit is counted, not clipped.** `Painter` clips rows vertically
  and never horizontally; an over-wide row is drawn off the panel in silence.
  This is `stock::fits`' existing rule and it is why this phase has a width
  census.
- **Colour roles, never raw indices.** `br yellow` means "the player must act" and
  `br red` means hostility or inbound harm. Neither is ever decorative.
- **Commits on a branch stay unversioned.** No `Cargo.toml` bump, no `CHANGELOG.md`
  section and no tag in this plan — those happen once, at the merge.
- Run `cargo fmt` and `cargo clippy --workspace` after every task; fix warnings
  rather than silencing them.

---

### Task 1: `hud::layout` — the five regions

**Files:**
- Create: `crates/gui/src/render/hud/mod.rs`
- Create: `crates/gui/src/render/hud/layout.rs`
- Modify: `crates/gui/src/render/mod.rs` — add `mod hud;` beside the existing
  `mod` list (alphabetical, after `help`)
- Test: `crates/gui/src/render/hud/layout.rs`, inline `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::paint::Rect`, `crate::text::Metrics`.
- Produces, and later tasks rely on these names exactly:

```rust
pub(super) struct HudRegions {
    pub status_bar: Rect,
    pub map_pane: Rect,
    pub log_pane: Rect,
    pub key_bar: Rect,
    pub info_column: Rect,
}

pub(super) fn regions(screen_w: f32, screen_h: f32, char_w: f32, m: &Metrics) -> HudRegions;
```

The arithmetic, ported from `design_handoff_hud/layout_reference.rs`. This is the
non-obvious part and is spelled out because getting it subtly wrong is invisible:

```
info_w  = (screen_w * 0.30).clamp(44.0 * char_w, 56.0 * char_w)
gutter  = char_w                      // 1 cell between the column and the left region
left_w  = screen_w - info_w - gutter
head_h  = m.line_height + m.inset     // == today's stock::strip_height
log_h   = m.line_height * 4.0 + m.inset * 2.0   // 4 text rows + 2 border rows
key_h   = m.line_height               // keybar, on the log's bottom border
map_h   = screen_h - head_h - log_h - key_h - m.gap
```

`key_bar` overlaps the bottom edge of `log_pane` deliberately — the keybar is
drawn *on* that border, not below it. `info_column` starts at `head_h` and its
height is `screen_h - head_h`: it reaches the bottom edge, and the log does not
pass under it.

- [ ] **Step 1: Write the failing tests**

Four tests, inline. Assert the two load-bearing *rules*, not transcribed pixel
values — a test that restates the arithmetic proves only that you can copy.

- `the_info_column_reaches_the_bottom_edge` — at 1280x720, 1440x810 and
  1920x1080, `info_column.y + info_column.h == screen_h` within `f32::EPSILON`
  scaled to the magnitude. Use `ui_metrics(screen_h)` for each `Metrics`.
- `the_log_never_passes_under_the_info_column` — at the same three sizes,
  `log_pane.x + log_pane.w <= info_column.x`.
- `the_info_column_stays_within_its_character_clamp` — sweep widths from 800.0 to
  3840.0 in steps of 40.0 and assert `44.0 * char_w <= info_w <= 56.0 * char_w`
  at every step. This is what catches the clamp being applied to the fraction
  instead of the product.
- `no_region_has_negative_extent` — same sweep, over both axes, for every one of
  the five rects. At 800x600 the subtractions are tightest and a negative height
  is a pane drawn inside-out.

For `char_w`, tests pass a literal (`9.0`) rather than measuring — `layout` takes
it as a parameter precisely so the geometry can be tested without a `Painter`.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-gui hud::layout`
Expected: does not compile — `regions` is not defined.

- [ ] **Step 3: Implement `regions`**

Write `HudRegions` and `regions` per the arithmetic above. `mod.rs` declares
`pub(super) mod layout;` and nothing else yet.

- [ ] **Step 4: Run them and watch them pass**

`cargo test -p feral-processes-gui hud::layout`

- [ ] **Step 5: Prove the tests are not vacuous**

Break `info_column`'s height to `screen_h - head_h - log_h` and confirm
`the_info_column_reaches_the_bottom_edge` fails. Restore. This repo has shipped
two tests that passed with the fix removed and read as coverage.

- [ ] **Step 6: Commit**

`git add crates/gui/src/render/hud/ crates/gui/src/render/mod.rs`
`git commit -m "feat(hud): the five HUD regions, derived once"`

---

### Task 2: `hud::palette` — the 16 roles

**Files:**
- Create: `crates/gui/src/render/hud/palette.rs`
- Modify: `crates/gui/src/render/hud/mod.rs` — add `pub(super) mod palette;`
- Test: inline `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::paint::Color`.
- Produces: named `pub(super) const` colours. Callers name a **role**, never an
  index. Chrome fills are separate consts in the same module.

```rust
// content roles — the handoff's ANSI 16, addressed by meaning
pub(super) const ATTENTION: Color;   // br yellow #e3b341 — "the player must act"
pub(super) const THREAT: Color;      // br red    #f26d6d — hostility, inbound harm
pub(super) const HEALTHY: Color;     // green     #4fa65b
pub(super) const PANE_TITLE: Color;  // br cyan   #56d4dd
pub(super) const EMPHASIS: Color;    // br white  #e8eef4 — keycap letters, emphasised values
pub(super) const BODY: Color;        // white     #a8b3bf
pub(super) const LABEL: Color;       // br black  #3a4550
pub(super) const PLAYER: Color;      // br cyan   #56d4dd
// log channels
pub(super) const CH_FIELD: Color;    // cyan      #3fa9b5
pub(super) const CH_GAIN: Color;     // br blue   #4a7fd0
pub(super) const CH_BASE: Color;     // br green  #7ee787
pub(super) const CH_DEFEND: Color;   // br red    #f26d6d
pub(super) const CH_IDLE: Color;     // br yellow #e3b341
// chrome
pub(super) const STATUS_BG: Color;   // #0b1117
pub(super) const PANE_BORDER: Color; // #1d2a36
pub(super) const BAR_TROUGH: Color;  // #1b2733
pub(super) const FIELD_LABEL: Color; // #5c6773
pub(super) const SECONDARY: Color;   // #8b97a5
pub(super) const FAINT: Color;       // #4a5563
pub(super) const KEYCAP_BG: Color;   // #20241a
pub(super) const ALERT_ROW_BG: Color;// #141410
pub(super) const KEYBAR_DIVIDER: Color; // #243040
pub(super) const DIVIDER: Color;     // #141e26
pub(super) const MAP_FLOOR: Color;   // #1c2c3a
```

Take the exact hex values from `design_handoff_hud/README.md`'s two colour
tables. `Color::new` takes 0.0–1.0 floats; `layout_reference.rs` already carries
the converted values for the ANSI 16 and the chrome greys — copy from there
rather than converting by hand.

- [ ] **Step 1: Write the failing test**

One test, `the_palette_matches_the_handoff`. Build a table of
`(const, expected_hex)` pairs for **every** const above and assert each channel
round-trips: `(c.r * 255.0).round() as u8` equals the hex's red byte, and the
same for green and blue. A palette has no behaviour of its own, so the only
thing worth testing is that no value was mistyped — and a single transposed
digit in `#3fa9b5` versus `#3fa8b5` is invisible on screen and permanent.

Write a small `fn hex(s: &str) -> (u8, u8, u8)` helper in the test module.

- [ ] **Step 2: Run it and watch it fail**

`cargo test -p feral-processes-gui hud::palette`
Expected: does not compile.

- [ ] **Step 3: Implement the palette**

- [ ] **Step 4: Run it and watch it pass**

- [ ] **Step 5: Commit**

`git commit -m "feat(hud): the handoff palette, addressed by role"`

---

### Task 3: `hud::status_bar` — three zones, absorbing the stock strip

**Files:**
- Create: `crates/gui/src/render/hud/status_bar.rs`
- Modify: `crates/gui/src/render/hud/mod.rs` — add `pub(super) mod status_bar;`
- Modify: `crates/gui/src/render/stock.rs` — make `pieces`, `line` and `fits`
  `pub(super)` so the status bar reuses them; **do not reimplement them**
- Test: inline `#[cfg(test)] mod tests` in `status_bar.rs`

**Interfaces:**
- Consumes: `hud::layout::HudRegions` (Task 1), `hud::palette` (Task 2),
  `stock::{pieces, line, fits}`, `feral_processes_engine::{StockRow, PlayerStatus}`.
- Produces:

```rust
pub(super) struct StatusBarState<'a> {
    pub zone: u32,
    pub position: (i32, i32),
    pub tick: u64,
    pub stock: &'a [StockRow],
}

pub(super) fn draw_status_bar(at: Rect, state: &StatusBarState, painter: &Painter, m: &Metrics);
```

**Layout.** Three zones across `at`:

- **Left** — `feral` in `EMPHASIS` immediately followed by `-processes` in
  `LABEL`, then `ZONE {n}`, `({x}, {y})`, `tick {n}`, separated by ` · ` in
  `FAINT`. Zone and tick values in `BODY`, their labels in `FIELD_LABEL`.
- **Centre** — the stock piles, drawn exactly as `stock::draw_stock_strip` draws
  them today, but into the width left over rather than the whole window.
- **Right** — reserved and **empty this phase**. The attention badge lands here in
  Phase 4. Do not draw a placeholder; an `ALL NOMINAL` that is not reading the
  attention model is a lie the next phase has to find and remove.

Background is `palette::STATUS_BG` across the full `at`, with a 2px
`palette::PANE_BORDER` rule along its bottom edge — the same shape
`draw_stock_strip` has today.

**The measurement rule.** Measure the left zone first, subtract it and the
reserved right zone from `at.w`, and pass the remainder to `stock::fits` as
`avail`. The centre zone is the only elastic one. Reserve the right zone as a
fixed fraction now (`at.w * 0.22`) so Phase 4 does not have to re-lay the bar out.

- [ ] **Step 1: Write the failing tests**

Three tests, using `with_painter` and `ui_metrics` — copy the fixture shape from
`stock.rs`'s existing test module, including its `fn stock(&[(&str, u32)])`
helper, which you should lift into the new module rather than reaching across.

- `the_status_bar_never_draws_wider_than_its_rect` — 60 piles with six-digit
  quantities and a zone/position/tick at their widest plausible values
  (`zone: 16`, `position: (-9999, -9999)`, `tick: 9_999_999`). Assert the measured
  advance of the left zone plus the measured advance of the drawn stock line is
  at most the bar's width minus the reserved right zone. This is the census: the
  bar is one row with no wrap and no clip.
- `the_left_zone_survives_a_crowded_base` — with those same 60 piles, assert
  `stock::fits` returns fewer piles than it would at full window width, i.e. the
  identity block actually took its space rather than being overdrawn.
- `an_empty_base_still_names_itself` — with zero stock rows, the left zone still
  draws. Guards against an early return copied from `draw_stock_strip`, which
  returns after writing "Base stock: none" and would take the identity block with
  it.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-gui hud::status_bar`

- [ ] **Step 3: Implement `draw_status_bar`**

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Prove the census is not vacuous**

Widen the reserved right zone to `at.w * 0.9` and confirm
`the_status_bar_never_draws_wider_than_its_rect` still passes (it should — less
room, fewer piles) but `the_left_zone_survives_a_crowded_base` behaviour is
unchanged. Then remove the right-zone subtraction entirely from the `avail`
computation and confirm the first test **fails**. Restore.

- [ ] **Step 6: Commit**

`git commit -m "feat(hud): the status bar absorbs the stock strip"`

---

### Task 4: Wire `draw_playing_base` to the new geometry

**Files:**
- Modify: `crates/gui/src/render/base.rs:534-620` — `draw_playing_base`
- Modify: `crates/gui/src/render/base.rs:14-15` — `PANE_W` / `PANE_H`
- Modify: `crates/gui/src/render/stock.rs` — remove `draw_stock_strip` and
  `strip_height`; both callers move to `hud`
- Modify: `crates/gui/src/render/frame_map.rs:474-475` and
  `crates/gui/src/render/field.rs:602,604,667` — the three sites reading
  `PANE_W` / `PANE_H`
- Test: existing suite is the gate; no new test file

**Interfaces:**
- Consumes: `hud::layout::regions`, `hud::status_bar::draw_status_bar`.
- Produces: no new public surface.

**What changes.** `draw_playing_base` stops computing `map_w`, `map_h`, `log_h`
and `strip_h` by hand and calls `hud::layout::regions` once, passing
`painter.measure_ui_advance("M", m.font_size)` as `char_w`. Then:

- the status bar is drawn into `regions.status_bar` instead of
  `stock::draw_stock_strip`;
- `draw_surface_map` / `draw_stack` / `draw_map_inset` are handed
  `regions.map_pane` — **their internals are not touched**;
- the **existing** `draw_status_panel` is handed `regions.info_column`, which is
  narrower than today's panel and now reaches the bottom edge;
- the **existing** log pane drawing is handed `regions.log_pane`, which is now
  the map's width rather than the window's.

`PANE_W` and `PANE_H` are deleted. The three sites in `frame_map.rs` and
`field.rs` that read them are recomputing "how wide is the right-hand panel" by
hand; repoint them at `regions` so there is one answer to that question. Note
`field.rs` hardcodes `1440.0` as a window width in both places — replace with the
real `info_column.w`, which is the bug that literal already is.

> **Watch for:** `draw_status_panel` clips its inventory rows against a
> `keys_y` computed from a four-line key block pinned to the panel bottom. The
> panel is now taller (it reaches the bottom edge) and narrower. It will look
> wrong until Phase 5 replaces its contents; it must not *crash* or draw outside
> its rect. Confirm by eye with `cargo run -- --template chains`.

- [ ] **Step 1: Repoint `draw_playing_base`**

Compute `regions` once at the top, after the `game` borrow is taken. Replace the
four hand-computed dimensions and the `draw_stock_strip` call.

- [ ] **Step 2: Delete `PANE_W` / `PANE_H` and fix the three readers**

- [ ] **Step 3: Run the gui suite**

`cargo test -p feral-processes-gui`
Expected: PASS. Any failure here is a layout test that was pinned to the old
proportions — read it before changing it; if it was asserting a *rule* the rule
still holds and the test needs new numbers, and if it was asserting a *number*
it was never worth much.

- [ ] **Step 4: Run the full suite**

`cargo test --workspace`
Expected: PASS. This is the gate; passing only the tests you wrote is not
evidence of correctness.

- [ ] **Step 5: `cargo clippy --workspace` and `cargo fmt`**

- [ ] **Step 6: Prove the repointing with a test, not with your eyes**

There is no playtest available on this project, so the geometry has to be
asserted rather than looked at. Add one test in `base.rs`'s existing
`#[cfg(test)] mod tests`, `the_playing_screen_draws_inside_its_regions`: build
the regions at 1440x900 through `hud::layout::regions`, and assert the map pane's
right edge does not cross `info_column.x` and that `map_pane.y` is at or below
`status_bar`'s bottom edge. That is the "a literal 0.0 draws under the strip and
no test sees it" trap, which is a real one in this file.

- [ ] **Step 7: Commit**

`git commit -m "feat(hud): the HUD reads its geometry from one place"`

---

## What the later phases need from this one

Phase 2 (map pane frame, border strips, vitals) consumes `HudRegions::map_pane`
and adds `hud::strip::border_strip`. Phase 3 (log pane, channel gutter, keybar)
consumes `log_pane` and `key_bar`. Phase 4 (column shell, tabs,
`Game::attention`) consumes `info_column` and fills the status bar's reserved
right zone. Phase 5 replaces the old status panel's contents. Phase 6 sweeps the
palette across map glyph colours.

Each subsequent phase's plan is written when its predecessor lands, because its
interfaces are not knowable until then — writing all six now would be six
documents describing guesses about five phases that have not been built.

## Self-review notes

- Spec coverage for this phase: Part 1 (geometry) is Tasks 1 and 4; Part 7
  (palette) is Task 2, definition only, with the sweep deferred to Phase 6 as the
  spec's phasing says; Part 5's stock-strip row of the move table is Task 3.
- The spec's Part 8 claims phases 1–4 leave "both the new chrome and the old
  status panel on screen". Task 4 makes that more precise and better: the old
  panel is *repointed* into the new column rect rather than drawn beside it, so
  there is no doubled chrome at any point. Update the spec's Part 8 wording when
  this phase lands.
