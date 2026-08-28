# Paned Command HUD — Phase 2: the map pane and its border strips

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development
> (recommended) or superpowers:executing-plans to implement this plan task-by-task.
> Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Frame the map pane and mount its three strips — the `SECTOR MAP`
title, the threat readout, and the vitals strip — on its own borders, so the
player's vitals cost the pane zero body rows.

**Architecture:** Two new primitives and one consumer. `strip.rs` draws text
*over* a border run with a background quad behind it, and exists as one function
because the handoff's own warning names the bug it prevents. `bar.rs` renders a
meter as block glyphs in a `TextRun`, so a bar is just another run on the
baseline it shares with its label — this needs no new drawing call and rides
`Painter::ui_runs`. `map_frame.rs` composes them.

**Tech Stack:** Rust, `crates/gui` only. No engine or app-core change: the threat
readout's hostile count is a filter over the `EntityView`s the map already draws,
and `Game::raid_defense_active` already exists.

**Spec:** `docs/superpowers/specs/2026-08-27-paned-command-hud-design.md`
**Predecessor:** `docs/superpowers/plans/2026-08-27-paned-command-hud-phase-1.md`
(landed as v0.13.37)

## Global Constraints

Phase 1's constraints all still apply — read that plan's Global Constraints
section. In addition:

- **Draw order is the whole of `strip.rs`.** Border run, then background quad,
  then glyphs. Paint the pane's interior after the label and the label is cut in
  half; that is the handoff's own recorded failure.
- **The vitals strip does not fit at every window size and must not pretend to.**
  It is one row on a border with no wrap and no clip. Segments have a fixed
  priority and what does not fit is **dropped from the end**, measured.
- **The glyphs are present.** `█` `▉` `░` `▸` `✓` `→` `·` `─` `│` are all in
  `assets/fonts/DejaVuSansMono.ttf`, verified against its cmap. Task 2 pins that
  rather than leaving it as a claim.
- **`draw_surface_map`, `draw_stack` and `draw_map_inset` are not touched.** They
  already take a `Rect`; this phase changes what is drawn *around* that rect.

---

### Task 1: `hud::strip` — text mounted on a border

**Files:**
- Create: `crates/gui/src/render/hud/strip.rs`
- Modify: `crates/gui/src/render/hud/mod.rs` — add `pub(super) mod strip;`
- Test: inline `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `crate::paint::{Painter, Rect, TextRun}`, `hud::palette`.
- Produces:

```rust
/// Which end of a pane's border a strip is mounted on, and which end of that
/// border it starts from.
pub(in crate::render) enum Mount { TopLeft, TopRight, BottomLeft }

/// Draws `runs` over a border run, breaking the border behind them.
///
/// Returns the advance the strip consumed, so a caller mounting two strips on
/// one border can tell whether the second will clear the first.
pub(in crate::render) fn border_strip(
    pane: Rect,
    mount: Mount,
    runs: &[TextRun],
    painter: &Painter,
    m: &Metrics,
) -> f32;
```

**Order, and it is the point:** the caller has already drawn the pane's border
*and its interior fill*. `border_strip` then paints a `palette::STATUS_BG` quad
the measured width of `runs` plus a pad either side, and only then the glyphs.

**Inset** is `m.inset` from the named end. **Vertical placement** centres the
text on the border line: the handoff's `-9px` top and `-11px` bottom at a 15px
body font are `-0.6 * font_size` and `-0.73 * font_size`; express them that way
so they scale with `Metrics` rather than freezing at the reference size.

- [ ] **Step 1: Write the failing tests**

- `a_strip_paints_its_background_before_its_glyphs` — the load-bearing one. Draw
  a strip through `with_painter` and walk the returned `ClippedShape`s in paint
  order; assert the index of the first `Shape::Rect` covering the strip's box is
  **lower** than the index of the first `Shape::Text`. `crate::paint::painted_text`
  shows the existing shape-walking idiom; you will need the rect side too, and
  `crate::paint::painted_rect_widths` already exists.
- `a_strip_reports_what_it_consumed` — the returned advance equals
  `measure_ui_advance` of the joined run text plus the two pads. This is what a
  caller mounting `SECTOR MAP` and `THREAT` on one border needs in order to know
  they do not collide.
- `a_right_mounted_strip_ends_at_the_pane_inset` — a `TopRight` strip's right
  edge lands at `pane.x + pane.w - m.inset`, so it grows leftward as its text
  grows rather than overflowing the pane.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-gui hud::strip`

- [ ] **Step 3: Implement `border_strip`**

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Prove the order test is not vacuous**

Move the background quad below the glyph draw and confirm
`a_strip_paints_its_background_before_its_glyphs` fails. Restore. A strip test
that still passes with the quad removed is measuring nothing, and this is the
one bug the whole module exists to prevent.

- [ ] **Step 6: Commit**

`git commit -m "feat(hud): text mounted on a pane's border"`

---

### Task 2: `hud::bar` — a meter as block glyphs

**Files:**
- Create: `crates/gui/src/render/hud/bar.rs`
- Modify: `crates/gui/src/render/hud/mod.rs` — add `pub(super) mod bar;`
- Modify: `crates/gui/tests/font_rasterization.rs` — pin the glyph coverage
- Test: inline `#[cfg(test)] mod tests` in `bar.rs`

**Interfaces:**
- Consumes: `crate::paint::Color`.
- Produces:

```rust
/// The filled and empty halves of a meter, as two strings ready to become
/// `TextRun`s in the fill colour and `palette::BAR_TROUGH`.
pub(in crate::render) struct Bar { pub filled: String, pub empty: String }

pub(in crate::render) fn bar(value: f32, max: f32, width: usize) -> Bar;
```

Not a drawing function. It returns strings, so a bar is just another pair of runs
on the baseline it shares with its label and the existing `Painter::ui_runs` draws
it — there is no new drawing primitive here and none is wanted. The existing
`bars.rs::draw_bar` is a *stacked* label-over-track block and is the wrong shape
for a strip; leave it alone, it has four other callers.

**Rounding is down, and it is a rule.** A bar reads full only at max, so 509/510
shows 15 of 16 with the sixteenth in trough colour. That is the handoff's rule and
it is what makes "nearly full" and "full" distinguishable at a glance.

- [ ] **Step 1: Write the failing tests**

In `bar.rs`:

- `a_bar_reads_full_only_at_max` — `bar(509.0, 510.0, 16)` gives 15 filled;
  `bar(510.0, 510.0, 16)` gives 16. This is the rule, and the obvious
  `round()` implementation fails it.
- `a_bar_never_exceeds_its_width` — sweep value from `-10.0` to `2.0 * max` and
  assert `filled.chars().count() + empty.chars().count() == width` at every step.
  Catches an unclamped ratio, which on an over-max value writes a bar wider than
  its column and pushes everything after it off the border.
- `a_zero_max_bar_is_empty_not_a_panic` — `bar(5.0, 0.0, 16)` gives 0 filled.
  A structure with no durability and a level-0 XP target both reach this.

In `crates/gui/tests/font_rasterization.rs`, following that file's existing
`UI` fontdue handle:

- `the_ui_font_has_every_glyph_the_hud_draws` — assert a non-empty rasterization
  for each of `█ ▉ ░ ▸ ✓ → · ─ │`. The handoff says to verify these before
  starting and lists ASCII fallbacks if any are missing; they are all present, so
  what this test does is stop a font swap from silently turning the vitals strip
  into tofu boxes.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-gui hud::bar` and
`cargo test -p feral-processes-gui --test font_rasterization`

- [ ] **Step 3: Implement `bar`**

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Prove the rounding test is not vacuous**

Change the implementation to `round()` instead of `floor()` and confirm
`a_bar_reads_full_only_at_max` fails. Restore.

- [ ] **Step 6: Commit**

`git commit -m "feat(hud): a meter as block glyphs, rounding down"`

---

### Task 3: `hud::map_frame` — the frame and its three strips

**Files:**
- Create: `crates/gui/src/render/hud/map_frame.rs`
- Modify: `crates/gui/src/render/hud/mod.rs` — add `pub(super) mod map_frame;`
- Test: inline `#[cfg(test)] mod tests`

**Interfaces:**
- Consumes: `hud::{strip, bar, palette}`, `feral_processes_engine::PlayerStatus`.
- Produces:

```rust
pub(in crate::render) struct Vitals<'a> {
    pub status: &'a PlayerStatus,
    pub mining: bool,
}

pub(in crate::render) struct Threat { pub hostiles: usize, pub shielded: bool }

/// Frame, title, threat readout and vitals. Draws the border and the three
/// strips; the pane's *contents* are the caller's, drawn before this.
pub(in crate::render) fn draw_map_frame(
    pane: Rect,
    vitals: &Vitals,
    threat: Threat,
    painter: &Painter,
    m: &Metrics,
);
```

**The three strips.**

- **Title**, `TopLeft`: `SECTOR MAP` in `palette::PANE_TITLE`, bold.
- **Threat**, `TopRight`: `THREAT` in `palette::THREAT` when `hostiles > 0` and
  in `palette::LABEL` when not, then the count, then shields —
  `shields holding` in `palette::HEALTHY` or `no defence` in `palette::ATTENTION`.
  **There is no countdown.** Raids are a per-tick roll (`Game::raid_check`), so
  the handoff's `GC sweep in 3 ticks` is not derivable and is not drawn. Do not
  invent one.
- **Vitals**, `BottomLeft`: the segment list below.

**The vitals segments, in priority order.** The strip does not fit at every
window size and must drop from the end rather than overflow:

| # | Segment | Colour |
|---|---|---|
| 1 | `INTEG` + bar(16) + `hp/max_hp` | `HEALTHY` fill |
| 2 | `PWR` + bar(10) + `power` | `ATTENTION` fill |
| 3 | `L{level}` + bar(14) + `xp/xp_to_next` | `CH_GAIN` fill |
| 4 | `▸ {n} perk pts [k]` — **omitted entirely when zero** | `ATTENTION` |
| 5 | `MIT {n}%` | `BODY` |
| 6 | `ATK {n}` | `BODY` |
| 7 | `STR {n}` | `BODY` |
| 8 | `DEC {n}` | `BODY` |
| 9 | `mining on` / `mining off` | `HEALTHY` / `LABEL` |

Labels in `palette::FIELD_LABEL`, values in `palette::BODY`, separators ` · ` in
`palette::FAINT`. Build the segments as `Vec<Vec<(String, Color, bool)>>`, measure
cumulatively, and take the longest prefix that fits `pane.w - m.inset * 2.0` —
`stock::fits`' shape, applied to segments instead of piles.

> **Why segment 4 is conditional and the rest are not.** A perk-points segment
> reading zero is chrome; the others reading zero are information. It also carries
> `palette::ATTENTION`, which is reserved for "the player must act", so drawing it
> at zero would be the reservation lapsing on its first use.

- [ ] **Step 1: Write the failing tests**

- `the_vitals_strip_never_draws_wider_than_its_pane` — at 1280x720, the smallest
  supported window, with every figure at its widest plausible value (hp 99999,
  level 99, xp 999999/999999, 99 perk points, stats in the hundreds). Assert the
  measured advance of the segments actually taken is at most
  `pane.w - m.inset * 2.0`, and assert that at this size **something was
  dropped** — if the fixture fits whole, it is not exercising the rule.
- `a_wide_window_keeps_every_vitals_segment` — at 1920x1080 the same fixture
  keeps all nine. Guards a too-eager drop rule, which the test above alone would
  not catch.
- `no_perk_segment_when_none_are_unspent` — with `perk_points: 0` the strip
  contains no `perk` text; with `perk_points: 1` it does.
- `the_threat_strip_reads_the_hostiles_and_the_shields` — four cases across
  `hostiles` zero/non-zero and `shielded` true/false, asserting the painted text
  through `crate::paint::painted_text`. Include the assertion that **no case
  contains the word `ticks`**, which is what stops the countdown being
  reintroduced by someone reading the handoff and not the spec.

- [ ] **Step 2: Run them and watch them fail**

- [ ] **Step 3: Implement `draw_map_frame`**

- [ ] **Step 4: Run them and watch them pass**

- [ ] **Step 5: Prove the drop rule is not vacuous**

Remove the width test from the segment fold so every segment is always taken, and
confirm `the_vitals_strip_never_draws_wider_than_its_pane` fails. Restore.

- [ ] **Step 6: Commit**

`git commit -m "feat(hud): the map pane wears its vitals on its borders"`

---

### Task 4: Wire the frame into `draw_playing_base`

**Files:**
- Modify: `crates/gui/src/render/base.rs` — `draw_playing_base`
- Test: existing suite is the gate, plus one addition below

**Interfaces:**
- Consumes: `hud::map_frame::{draw_map_frame, Vitals, Threat}`.
- Produces: no new public surface.

**What changes.** After the map's contents are drawn into `regions.map_pane`
(both the `stack_view` and `draw_surface_map` branches — the frame is shared, per
the spec's Stack decision), call `draw_map_frame`. The threat figures come from
the `EntityView`s the map already fetched: `hostiles` is
`views.iter().filter(|v| v.is_hostile).count()`, and `shielded` is
`game.raid_defense_active()`.

> **The trap.** `draw_map_frame` must be called **after** the pane's contents,
> not before. `border_strip` paints its own background so the border reads as
> broken by a label; drawn before the map, the map's own fill paints over the
> labels and cuts them in half. That is the handoff's recorded failure and Task
> 1's test only proves the ordering *inside* `border_strip`.

The old integrity and power bars in `draw_status_panel` now have a second home on
the vitals strip. **Leave them in the panel for now** — the panel is replaced
wholesale in Phase 5, and removing two rows from a screen that is about to be
deleted is churn. Note the duplication in the commit message.

- [ ] **Step 1: Wire it up**

- [ ] **Step 2: Add the ordering census**

In `base.rs`'s existing test module, `the_map_frame_draws_after_the_map`: draw
the playing screen through `with_painter` and assert the `SECTOR MAP` text shape
appears **later** in paint order than the map pane's own background rect. This is
the half Task 1 cannot see.

- [ ] **Step 3: Run the gui suite**

`cargo test -p feral-processes-gui`

- [ ] **Step 4: Run the full suite**

`cargo test --workspace` — the gate.

- [ ] **Step 5: `cargo clippy --workspace` and `cargo fmt`**

Warnings must be zero in files this phase touched. `palette.rs`'s
`#![allow(dead_code)]` is expected to still be suppressing entries for phases
3-6; that is by design and documented in the file. `PANE_TITLE` and `THREAT`
come off that list this phase.

- [ ] **Step 6: Commit**

`git commit -m "feat(hud): the map pane is framed"`

---

## What Phase 3 needs from this one

Phase 3 (log pane, channel gutter, filter strip, keybar) consumes
`hud::strip::border_strip` for the filter strip and the keybar, and
`HudRegions::{log_pane, key_bar}`. It is the first phase to mount **two** strips
on one pane and the first to use `Mount::BottomLeft` for something that must
measure against a competing strip — which is why `border_strip` returns its
consumed advance rather than nothing.
