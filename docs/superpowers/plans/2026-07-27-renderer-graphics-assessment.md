# Renderer & graphics: costed assessment

**2026-07-27.** Reference document, not a committed plan — nothing here has been
built. Written to answer "what would be the lift for switching to Bevy, so we
could implement actual graphics later on?", extended to cover bracket-lib.

## Context

The premise needs one correction before any estimate is meaningful:

**The engine is already Bevy.** `crates/engine/Cargo.toml` depends on
`bevy_ecs 0.19.0` — the same crate Bevy 0.19 ships — and uses it idiomatically
across 15 files: `World`, `Schedule`, `Query`/`Res`/`ResMut`, `Component`,
`Resource`, `SystemParam`, `resource_scope`. `Game` (`crates/engine/src/lib.rs:82`)
is literally `{ world: World, schedule: Schedule }`.

So there is no "switch the engine to Bevy" work item. The entire lift is the
**renderer**, and the second correction is that macroquad is not what's blocking
graphics:

**The renderer has never drawn a single texture.** Across all 4,410 lines of
`crates/gui`, there are **zero** uses of `Texture2D`, `draw_texture*`,
`load_material`, `set_camera`, or `render_target`. Everything on screen is
`draw_rectangle` (13 sites), `draw_rectangle_lines` (9), `draw_line` (1), and
`draw_text_ex` (via three `Fonts` wrappers). macroquad 0.4 supports sprites,
atlases, cameras, render targets and custom shader materials — none of it is
wired up. The tileset is blocked by absent code, not by the crate.

## What the codebase actually looks like

| | LOC | Notes |
|---|---|---|
| `crates/engine` | 26,303 | Pure `bevy_ecs`. No macroquad. Untouched by any tier below. |
| `crates/app-core` | 3,868 | Renderer-agnostic already: `GameKey`, `Mode`, `SoundEvent`. **Zero** macroquad references. |
| `crates/gui` | 4,410 | The only macroquad consumer. |
| `crates/launcher` | 60 | One `graphics_available()` preflight + `gui::run(App)`. |

Inside `crates/gui`:

- `render/` — 3,109 LOC across 15 files. Map/battle path is `base.rs` (314) +
  `battle.rs` (676); menu/popup layer is `popup.rs` (376) + 12 menu files + the
  `mod.rs` dispatcher (370).
- `text.rs` (455), `fx.rs` (414), `keys.rs` (173), `lib.rs` (201), `sounds.rs` (58).
- 48 unit tests: `text` 15, `fx` 13, `render/battle` 10, `keys` 5, `render/popup` 3,
  `render/mod` 2.

Two structural facts make this much cheaper to move than 4,410 LOC suggests:

1. **Layout is arithmetic, not text measurement.** `measure_ui`/`measure_map`
   have only **5 call sites** total. `ui_metrics()` (`text.rs:172`) derives every
   dimension from window height, and `PopupLayout` sizes boxes as a percentage of
   the window. That whole system is pure and backend-independent — it ports to
   anything that can draw a rect and a string at `(x, y)`.
2. **The backend surface is ~11 operations**: draw rect, draw rect outline, draw
   line, draw text (font/size/color), measure text, screen w/h, wall time, key
   down/pressed, char pressed, play sound. Only 7 of 19 gui files import
   macroquad directly; the rest inherit it through `use super::*`.

The map draw loop (`render/base.rs:37–136`) is per-tile: background rect →
centered glyph → optional outline → optional flash rect. Swapping
`fonts.map(&glyph, tx, ty, glyph_px, color)` for a tinted `draw_texture_ex` is a
three-line change in that loop.

---

## The three tiers, costed

### Tier 1 — Sprite tileset on macroquad
**~400–700 LOC. Days. Zero architectural risk.**

Add an atlas loader (`Texture2D` + `FilterMode::Nearest`, mirroring the existing
unscii crispness handling in `text.rs:38`), a `GlyphColor`/entity → atlas-index
map, and swap the glyph call in the `base.rs` tile loop for a tinted texture
draw. `terrain_color` and `fx::structure_condition` already produce the tint
values — sprites inherit the existing damage-dimming and desaturation for free.

Touches: `text.rs` (or a new `tiles.rs`), `render/base.rs`, `render/battle.rs`.
Everything else untouched. All 48 tests survive.

Gets you: the monochrome tech tileset, on screen, this week.
Does not get you: post-processing, particles, hot-reload.

### Tier 2 — Tileset + shader FX
**macroquad: ~800–1,400 LOC. Bevy: full-rewrite territory.**

On macroquad: `render_target()` for the scene, `load_material()` for a
CRT/scanline/bloom pass, hand-rolled particles. Supported, but you write the
post-processing stack yourself and macroquad's material API is thin.

On Bevy: `Material2d`, built-in bloom/tonemapping, `bevy_hanabi` for particles.
Genuinely better tooling — but you cannot take Bevy's renderer without taking
Bevy's `App`, its window/input plugins, and a UI toolkit decision, which drags
in the whole of Tier 3. There is no partial-Bevy option.

**This is the decision point.** If you want a shader stack, the honest choice is
"hand-roll it on macroquad" vs "pay for Tier 3".

### Tier 3 — Full Bevy frontend
**~3,500–4,400 LOC rewritten. Weeks. The engine still doesn't change.**

| Piece | Fate |
|---|---|
| `crates/engine` (26,303) | **Untouched.** Already `bevy_ecs 0.19`, version-matched to Bevy 0.19. |
| `crates/app-core` (3,868) | **Untouched.** Already renderer-agnostic. This is why the swap is tractable at all. |
| `keys.rs` (173) | Ports near-verbatim — `KeyRepeat::tick(now, held)` is pure; Bevy has no built-in key repeat either. |
| `sounds.rs` (58) | Trivial → `bevy_audio` or `bevy_kira_audio`. |
| `fx.rs` (414) | Timing math is pure and ports; the ~10 draw calls at the bottom get rewritten. |
| `text.rs` (455) | `Metrics`/`ui_metrics`/`terrain_color` port unchanged. `Fonts` is rewritten — Bevy has no `draw_text_ex`. |
| `lib.rs` (201) | Rewritten as a Bevy `App` with plugins + input systems. |
| `render/` (3,109) | **Rewritten.** This is the bulk. |
| 48 gui tests | Pure ones survive (`keys` 5, `fx` 13, most of `text` 15). Layout assertions in `render/*` (15) get rewritten or dropped if you move to flexbox. |

**UI toolkit fork inside Tier 3:**

- **`bevy_egui`** — keeps immediate mode, so `render/` ports ~60–70%
  mechanically (`draw_rectangle` → egui painter rect, `draw_text_ex` → galley).
  Bevy renders the map with sprites/shaders, egui draws the chrome. This is the
  common shipping shape for this genre and by far the cheaper path.
- **`bevy_ui`** — idiomatic and retained-mode, but flexbox replaces the hand-rolled
  `PopupLayout` scroll/header/footer logic, so `popup.rs` and all 12 menu files
  are from-scratch. Adds roughly 1,000+ LOC of rework over the egui path.

**Costs that don't show up as LOC:**

- Dependency count goes from 155 locked packages to roughly 400–500. Cold builds
  and CI go from under a minute to several. `cargo test --workspace` currently
  runs in ~3s; the engine/app-core tests stay fast (separate crates, no Bevy
  render deps), but the workspace build gate gets noticeably heavier. Given the
  workflow here — full suite as the final gate on every change — that's a real
  daily tax, not a one-off.
- **One tempting Bevy benefit is off the table by our own rules.** Version-matched
  ECS would let the renderer query the sim's `World` directly. CLAUDE.md
  explicitly forbids that: `Game` is the entire public API surface and the
  renderer never touches the ECS `World`. With the TUI gone, that rule is held by
  convention alone — a Bevy frontend that *could* reach into the world would put
  real pressure on it.

---

## Considered and rejected: bracket-lib (RLTK)

Closer shape-match than Bevy, and it loses on two independent grounds.

**What it gets right.** It is a glyph-cell console renderer, and the map *is* a
glyph grid — `render/base.rs:99–104` is `ctx.set(x, y, fg, bg, glyph)` in all but
name. Tilesets are its **native** mechanism: register a PNG glyph sheet as a font
and glyphs become tiles, with multiple consoles layered at different fonts. That
is a documented drop-in, cheaper than Tier 1. It also ships `bracket-pathfinding`
(A*, Dijkstra maps, FOV), and the engine has **zero** pathfinding or FOV code
today — additive capability, not overlap. (`bracket-noise` and `bracket-random`
*would* overlap `noise 0.9` in `world.rs` and `rand 0.10`; take sub-crates
individually, never the meta-crate.)

**Why it loses.**

1. **It's grid-locked, and this UI stopped being a grid.** `ui_metrics()`
   (`text.rs:172`) scales font size continuously from window height (16–40px);
   popups are percentage-of-window at float coordinates; `PopupLayout` does float
   row arithmetic. `fx.rs` is deliberately sub-cell — floats rise continuously
   (`fx.rs:266`, 24px over 0.6s), `flash_alpha` fades linearly, the ghost bar
   drains at 60 HP/sec, shield pulse oscillates at 0.5Hz. A character grid can't do
   smooth motion or partial-cell bar fills; the "fancy console" recovers
   fractional positioning but not the alpha work.
2. **It is the frontend just deleted.** Commit `322b263` (2026-07-24) removed
   2,328 lines of ratatui renderer for being unmaintainable alongside the GUI.
   Adopting bracket-terminal means re-deriving the ~2,500-line menu/popup layer
   back onto character cells. Different library, same constraint.

**Maintenance.** Last crates.io release for both `bracket-lib` and
`bracket-terminal` is **0.8.7, 4 Oct 2022** — 3 years 9 months ago. Repo is alive
but not shipping: last push 5 Dec 2025, 1,686 stars, 108 open issues, not
archived. Pinning the renderer to a 2022 release under edition 2024 /
`bevy_ecs 0.19` / `rand 0.10` is a standing risk.

**Cherry-pick instead:** if smarter AI, line-of-sight or fog-of-war ever comes up,
add **`bracket-pathfinding` alone**. It has no renderer dependency and fills a
real gap. Skip `bracket-terminal`.

---

## Recommendation

**Do Tier 1 now. Don't decide on Bevy until Tier 1 is on screen.**

Tier 1 is close to free, it directly delivers the thing actually wanted (a tileset
instead of glyphs), and it is not wasted work under any later decision — the
atlas, the tile-index mapping, and the art itself all carry over to a Bevy
frontend unchanged. It also answers the Tier 2/3 question with evidence rather
than speculation: once sprites are rendering, we will know whether macroquad's
shader/material API is actually the wall, or whether it was never the constraint.

Committing to Bevy today means paying weeks of rewrite and a permanent build-time
tax to unblock effects we haven't yet established we need, on a frontend whose
menus work fine.

If Tier 1 lands and the shader stack is still wanted, take **Bevy + `bevy_egui`**,
not `bevy_ui` — it preserves the immediate-mode shape the existing 3,109 lines are
written in, and `Metrics`/`PopupLayout`/`terrain_color`/`fx` timing all survive
the move.

Ranked, with reasons:

1. **macroquad + sprite atlas** (Tier 1) — near-free, ships the tileset, wasted
   under no later choice.
2. **Bevy + `bevy_egui`** — only if a shader/particle stack turns out to be the
   real requirement. Weeks, plus a permanent build-time tax.
3. **bracket-lib** — no. Right tool for a game that is still a cell grid; this one
   deliberately isn't. Take `bracket-pathfinding` on its own if FOV comes up.

## Files for Tier 1

- `crates/gui/src/render/base.rs:37–136` — the tile loop; the three-line glyph→sprite swap.
- `crates/gui/src/render/battle.rs` — the second sprite consumer.
- `crates/gui/src/text.rs:31–50` — `Fonts::load`; the `FilterMode::Nearest` precedent a
  pixel-art atlas must follow, and the `include_bytes!` vs `assets_dir` decision.
  Per CLAUDE.md's moddability rule, a **tileset is content and belongs in
  `assets/`**, not embedded — unlike fonts and sound blips.
- `crates/engine/src/components.rs` — `GlyphColor`, the existing per-entity color the
  sprite tint reuses.
- `crates/gui/src/fx.rs` — `structure_condition` tinting, which sprites inherit unchanged.

Reuse rather than rewrite: `text::map_cell(zoom)` already returns the tile pixel
size, `text::terrain_color` the biome wash, `render::desaturate` the back-rank
grey. A sprite path needs none of those re-derived.

## Verification (for Tier 1, if and when it is picked up)

- `cargo test --workspace` — the gate. Tier 1 should leave all 48 gui tests and
  the engine/app-core suites passing untouched.
- New tests go in the pure layer only, following the existing convention that
  drawing isn't testable: assert the `GlyphColor` → atlas-index mapping is total,
  and that atlas UV rects land on whole-pixel boundaries at every zoom step in
  `MIN_ZOOM..=MAX_ZOOM` (the same class of assertion `text.rs`'s tests already
  make about the 16px glyph ladder).
- `cargo clippy --workspace` and `cargo fmt`.
- Final visual sign-off by eye — per standing policy, drawing changes are verified
  headlessly here, not by launching the GUI.

## Sources

- [bracket-lib on crates.io](https://crates.io/crates/bracket-lib)
- [bracket-terminal on crates.io](https://crates.io/crates/bracket-terminal)
- [amethyst/bracket-lib on GitHub](https://github.com/amethyst/bracket-lib)
