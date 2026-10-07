# Sprite Forge Editing Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Checkbox (`- [ ]`) steps.

**Goal:** The forge opens, edits and saves shipped `.colour` sprites through a 63-colour palette, and is usable by mouse (swatch grid, fill, eyedropper, buttons, picker clicks).

**Spec:** `docs/superpowers/specs/2026-10-07-sprite-forge-editing-design.md`. Branch `sprite-forge-editing`, base `54806296`. Line numbers are as of that commit; verify before editing.

Per repo CLAUDE.md: files, interfaces, test intent, gates — not finished code.

**Phases.** Phase 1 (T1–T4) is the load/save fix and stands alone: it ships even if Phase 2 waits. Phase 2 (T5–T8) is mouse and tools, all inside the forge. A fresh executor session per phase is fine; Phase 2 needs only this plan plus Phase 1's landed interfaces.

## Decisions made while planning

- **Palette (settles spec §1).** Nine families: yellow (50°) and lime (80°) merge. Shades are a per-family k-means over the shipped art's distinct opaque colours, sorted dark→light. Measured over every opaque pixel in `assets/sprites/`: **max snap 71.8, mean 13.06** (worst: `(0,255,204)` in `depot.colour.png`). Bound constant `SPRITE_SNAP_BOUND = 72.0`. A minimax fit got max 48.8, but its families had duplicates and grey entries, which breaks the spec's "family of six, dark to light" shape, so it was rejected. Entries 10–63, in order:
  ```
  // 0° red
  (0x45,0x0a,0x0a),(0x8d,0x1c,0x1c),(0xd8,0x25,0x25),(0xef,0x44,0x44),(0xf9,0x71,0x7a),(0xfc,0xa5,0xa7),
  // 20° orange/brown
  (0x1c,0x0f,0x05),(0x43,0x20,0x09),(0x80,0x35,0x12),(0x7a,0x51,0x28),(0xde,0x5b,0x0d),(0xa8,0x87,0x5a),
  // 50° yellow/lime
  (0x1a,0x2e,0x05),(0x45,0x6e,0x11),(0x6d,0x96,0x17),(0xf8,0xb8,0x13),(0xa3,0xe6,0x35),(0xd2,0xbf,0x85),
  // 140° green
  (0x05,0x2e,0x16),(0x14,0x53,0x2d),(0x15,0x76,0x3a),(0x16,0xa3,0x4a),(0x35,0xd1,0x6e),(0x86,0xef,0xac),
  // 160° emerald
  (0x02,0x2c,0x22),(0x05,0x6a,0x4d),(0x0a,0x95,0x7b),(0x11,0xb9,0x88),(0x30,0xd5,0xab),(0x84,0xee,0xce),
  // 190° cyan/teal
  (0x04,0x2f,0x2e),(0x12,0x58,0x53),(0x0f,0x76,0x6e),(0x07,0x90,0xb4),(0x27,0xc3,0xee),(0x67,0xe8,0xf9),
  // 230° blue
  (0x0c,0x11,0x25),(0x1c,0x22,0x48),(0x3d,0x4a,0x6e),(0x33,0x57,0xdb),(0x78,0xa0,0xea),(0xd3,0xdc,0xe7),
  // 280° violet
  (0x4a,0x04,0x4e),(0x79,0x1a,0x7f),(0xb2,0x21,0xc2),(0xba,0x50,0xf4),(0xd1,0x7f,0xfb),(0xf0,0xab,0xfc),
  // 340° rose
  (0x4c,0x05,0x19),(0x88,0x13,0x37),(0x9f,0x12,0x39),(0xbe,0x12,0x3c),(0xe1,0x1d,0x48),(0xf4,0x3f,0x5e),
  ```
  If T1's test measures something other than 71.8 / 13.06, the measuring code differs from the plan's (alpha ≥ 128, Euclidean RGB). Reconcile that before moving the bound.
- **Grid columns.** The swatch grid is 16 columns × 4 rows. `SpriteEditorView.palette_cols: u8` carries the column count, and so does `CanvasEditor`, so keyboard and mouse agree. The icon editor passes `palette_len + 1`, which keeps it at one row.
- **Double click** is a new `PointerPhase::DoubleClick`. The gui emits it from egui's `double_clicked()`, and it is honoured only on `PointerHit::Subject`.
- **The "hued" test** for the variant rule is any pixel index `> 9`, meaning past the grey ramp, and it is a pure fn in app-core.

## Global constraints

- Only `crates/gui/src/paint.rs` names a graphics lib. `Game.world` gets no accessor. app-core never opens a file.
- `ICON_PALETTE` and its save format are untouched. There is no `SAVE_FORMAT_VERSION` change.
- The icon editor (`app-core/src/app/icon_editor.rs`, `gui/src/render/icon_editor.rs`) must behave and draw exactly as today. Every shared-code task re-runs its tests.
- `cargo fmt` and `cargo clippy --workspace --all-targets` must be clean per task. Commit per green step. Unset `FERAL_DEV_NO_SIEGES` before testing.
- Subagents: never push, never `git checkout`/`stash` over uncommitted work, and stage explicit paths only.

## Review focus

1. A shipped `x.colour.png` that is opened, saved and toggled never creates or touches `x.png`. Both-files precedence matches `SpriteTable::insert` (paint.rs:~183). → T3
2. The icon editor's palette navigation and swatch geometry are unchanged. → T5
3. A fill on an already-selected region records no undo entry. A fill drag fills once. → T6
4. A button cannot drift from its key, because the button calls the key handler. → T7
5. Preview tint and map tint are one fn, not a copy. → T4

---

## Phase 1 — open and save shipped art

### T1: Wider `SPRITE_PALETTE` (engine)

**Files:** `crates/engine/src/icon.rs:60` (array → `[_; 63]`, update the doc comment and the `1..=19` doc at :119); `crates/engine/src/tests/sprite_palette.rs`.

- [ ] Rename `all_nineteen_sprite_palette_entries_are_distinct` (:38) to `..._all_entries_...`. The existing round-trip tests then cover 63 entries for free.
- [ ] New test: the first nine entries equal the current grey ramp, in order. This pins near-white sprites.
- [ ] New test `every_shipped_sprite_snaps_within_the_bound`: decode each `assets/sprites/*.png` and assert the max nearest-palette distance is `< SPRITE_SNAP_BOUND`. Its doc comment records max 71.8 / mean 13.06. Check whether the engine has a PNG decoder dep first. If it has none, put this test in `crates/gui/src/sprites.rs` tests, which already decode. Do not add a dep to the engine for a test.
- [ ] Gate: `cargo test -p feral-processes-engine sprite_palette`. Run the icon editor tests too, since the editor reads `SPRITE_PALETTE.len()`.

### T2: Variant in app-core

**Files:** `crates/app-core/src/app/sprite_forge.rs`; `crates/app-core/src/tests/sprite_forge.rs`. The minimal gui call-site update goes in `crates/gui/src/sprites.rs` (scan_library wraps everything `full_colour: false` for now) so the workspace compiles.

**Interfaces:**
```rust
pub struct InstalledSprite { pub canvas: Canvas, pub full_colour: bool }
pub fn install_sprite_library(&mut self, enabled: HashMap<String, InstalledSprite>, disabled: HashMap<String, InstalledSprite>) // SF:264
pub enum SpriteOp { Save { canvas: Canvas, full_colour: bool }, Enable { full_colour: bool }, Disable { full_colour: bool } } // SF:194
pub struct SpriteEditorView { /* + */ pub full_colour: bool, pub palette_cols: u8 }
fn saves_as_full_colour(canvas: &Canvas) -> bool // any index > 9
```
- `SpriteEditor` (SF:143) gains `full_colour: Option<bool>`, where `None` means new art. On Save, use `full_colour.unwrap_or_else(|| saves_as_full_colour(&canvas))`. Picker `t` (SF:448) passes the subject's installed variant.
- [ ] Tests: hued new art saves `true`; grey-only new art saves `false`; loaded colour art stays `true` after grey-only edits; loaded plain art stays `false` after hued edits; Enable/Disable carry the installed variant.

### T3: gui loads, saves and toggles the variant file

**Files:** `crates/gui/src/sprites.rs` (`scan_library` :147, `apply_sprite_write` :370, invariant doc :359).

- [ ] `scan_library` strips `engine::FULL_COLOUR_SUFFIX` from the stem, for both `.png` and `.png.off`, and sets `full_colour`. On a key collision the colour file wins in each map.
- [ ] One `fn sprite_path(dir, name, full_colour, off: bool) -> PathBuf`. Every arm of `apply_sprite_write` derives its paths through it.
- [ ] Tests (tempdir; no shared temp path between tests): colour keying, including `.off`; colour preferred when both exist; Save, Enable and Disable on a colour sprite touch only `x.colour.png[.off]`; the never-both invariant holds per variant; plain-sprite behaviour is unchanged.

### T4: One tint fn, preview matches the map, README

**Files:** `crates/gui/src/paint.rs:430-436` → `pub fn colour_sprite_tint(c: Color) -> Color` (pure, `Color` is the seam's own type); `crates/gui/src/render/sprite_forge.rs` `draw_preview_cell` :440; `assets/sprites/README.md` (:64-75 the editor now opens and saves `.colour`; :114-122 the warning re-worded for 63 colours).

- [ ] `Painter::sprite` calls `colour_sprite_tint`. When `view.full_colour` is set, the preview tints each index with `colour_sprite_tint(hue)` instead of `hue`.
- [ ] Tests: `colour_sprite_tint` gives grey at the max channel and keeps alpha. Mutation-check that the preview path calls it.
- [ ] **Phase gate:** run `cargo test --workspace` and clippy, then `cargo run` with `FERAL_DEV_SPRITES=1 --keys … --screenshot` to open `construct` and Read the PNG. Commit.

## Phase 2 — mouse and tools

### T5: Swatch grid

**Files:** `crates/gui/src/render/canvas.rs` (`draw_swatch_row` :118 → `draw_swatch_grid(p, rect, cols, selected, palette)`; the row becomes the `cols = len+1` case). `swatch_at` (GSF:504) gains a `cols` param for 2-D. `crates/app-core/src/app/canvas_editor.rs`: `open(canvas, palette_len, palette_cols)` (CE:113), and `step` (CE:271) in Palette focus moves Up/Down by `cols` when rows > 1, clamped. `crates/gui/src/render/sprite_forge.rs` `editor_geometry` :235.

- [ ] Tests: `swatch_at` hits every centre and misses every gap at 16 cols; the icon editor's one-row geometry is byte-identical, asserted against the current rects; Up/Down at one row behave as today; Up/Down at 16 cols move a row and clamp at the edges; `the_editor_screen_fits_at_1280x720` (GSF:775) still passes.

### T6: Fill tool and eyedropper

**Files:** `canvas_editor.rs`; `sprite_forge.rs` (app-core: `PointerButton::Middle`, key routing); `crates/gui/src/lib.rs:231` `handle_sprite_pointer` (Alt+Primary or middle → `Middle`).

- `enum Tool { Paint, Fill }` lives on `CanvasEditor`. `[f]` toggles it, and the key is routed only from `handle_sprite_editor_key` (SF:504), never from the icon editor. Fill is a 4-connected flood of the clicked cell's index. It ignores the brush and is wrapped in `begin_stroke`/`end_stroke`, so the lazy snapshot makes it one undo entry. In Fill mode only `Down` acts.
- `[i]` and `Middle` set `selected` to the cell's index and push no history.
- [ ] Tests: the fill stays in its region (a ring around a hole); the brush is ignored; one undo restores; a no-op fill records nothing; Drag/Up after a fill Down do nothing; the eyedropper selects without history; the icon editor ignores `f` and `i`.

### T7: Button bar

**Files:** app-core `sprite_forge.rs` (`pub enum EditorButton { Save, Undo, Clear, Brush, Tool, Back }`, `PointerHit::Button(EditorButton)`, `fn key(self) -> GameKey`); gui `render/sprite_forge.rs` (draws the bar; `HitRects` :526 gains the button rects; labels show the key, the current tool and the brush size).

- `handle_pointer` on `Button(b)` with `Down` calls `handle_sprite_editor_key(b.key())`. There is no other path.
- [ ] Tests: for each button, the effect equals its key's effect on a cloned App; the bar fits at 1280x720; a click on each rect resolves to its button.

### T8: Picker by mouse

**Files:** app-core `sprite_forge.rs` (`handle_pointer` SF:547 also accepts `Mode::SpritePicker`; `PointerHit::Subject(usize)`; `PointerPhase::DoubleClick`); gui `render/sprite_forge.rs` (subject row rects from `picker_geometry` :69) and `lib.rs` (route pointer in the picker mode too).

- Down on a subject sets `menu_selected`. DoubleClick on the selected subject runs the same fn as `Enter`; extract it from SF:448 if it is inline. Out-of-range indices are ignored.
- [ ] Tests: a click selects; a double click opens the selected subject (the editor holds its art and variant); a double click on an unselected row only selects it; `the_picker_shows_every_subject_with_no_scroll_at_1280x720` (GSF:641) passes.
- [ ] **Final gate:** run `cargo test --workspace` and clippy, plus a `--screenshot` of the editor open on `construct` showing the grid and the bar (Read it). Add a CHANGELOG entry at landing, not on the branch. The whole-branch review is opus.
