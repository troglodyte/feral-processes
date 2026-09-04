# Developer sprite editor — implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A dev-gated in-game screen for drawing a 16x16 sprite for any species, structure or map fixture, saving it into `assets/sprites/`, and toggling it back off without losing the art.

**Architecture:** The canvas *mechanics* are shared and the two editors are not. `engine::icon::Canvas` is a runtime-edged grid of palette indices; `PlayerIcon` becomes a wrapper around one at edge 8 and keeps its codec alone. `app-core::CanvasEditor` holds the cursor, brush, undo ring and shared key verbs, and both `IconEditor` (wizard sink) and the new `SpriteEditor` (PNG-cue sink) **own one as a field**. The gui shares one `draw_canvas` between the two screens, owns all PNG I/O, and resolves the mouse to a cell before app-core sees it.

**Tech Stack:** Rust, bevy_ecs 0.19, bevy 0.19 + bevy_egui 0.41, `image` 0.25.10 (png only).

**Spec:** [`docs/superpowers/specs/2026-09-04-dev-sprite-editor-design.md`](../specs/2026-09-04-dev-sprite-editor-design.md) — read it first; this plan argues from it and does not restate its reasoning.

**On this plan's shape:** it carries file lists, exact interfaces, test intent and gates, and *no implementation code*. That is CLAUDE.md's "Process weight" rule, which overrides the writing-plans skill's default: a subagent that has the repo and CLAUDE.md should not be handed finished code it will merely re-emit. Code blocks are reserved for the genuinely non-obvious, and nothing here needed one.

## Global Constraints

- **Read `CLAUDE.md` before touching a seam it names**, and invoke the `seams` skill for the HUD, saves and screens entries.
- **TDD, failing test first, one commit per green step.** `cargo fmt` and `cargo clippy --workspace` after every change; fix warnings rather than silencing them.
- **`cargo test --workspace` is the final gate.** Per-task, `cargo test -p <crate> <name>` is enough.
- **`ICON_PALETTE` stays at exactly 15 entries.** It is the player icon's save format — one hex digit, 0 = transparent. A sixteenth colour reads back as transparent, silently. `SPRITE_PALETTE` is a separate constant and the two never merge.
- **Sprites are 16x16 RGBA PNG.** Not negotiable — `assets/sprites/README.md` and `text::map_cell`'s integer zoom ladder.
- **No version bump and no `CHANGELOG.md` section on this branch.** Per CLAUDE.md the release happens once, at the merge.
- **The whole feature is invisible without `FERAL_DEV_SPRITES` *and* a checkout.** Acceptance criterion 1: with either absent, the game is exactly the game it is today.
- Player-facing text is not affected; the setting's no-occult-naming rule does not arise. This screen is a dev tool and is not documented in `assets/help/`.

---

### Task 1: `Canvas`, and `PlayerIcon` composed on it

**Files:**
- Modify: `crates/engine/src/icon.rs`
- Test: `crates/engine/src/tests/player_icon.rs` (existing — **must pass unchanged**)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub struct Canvas { edge: usize, cells: Vec<u8> }`, deriving `Clone, PartialEq, Eq, Debug`
  - `Canvas::new(edge: usize) -> Canvas` (all cells 0)
  - `Canvas::edge(&self) -> usize`
  - `Canvas::get(&self, x: usize, y: usize) -> u8` — 0 for any off-grid coordinate
  - `Canvas::set(&mut self, x: usize, y: usize, index: u8)` — off-grid dropped, not a panic
  - `Canvas::clear(&mut self)`, `Canvas::is_blank(&self) -> bool`
  - `PlayerIcon { cells: Canvas }` at edge `ICON_GRID`, with `get`/`set`/`clear`/`is_blank`/`rgba`/`pixel_rgba`/`encode`/`decode` unchanged in behaviour and signature

**Notes for the implementer:** `Canvas::set` must keep `PlayerIcon::set`'s palette-range guard *out* of it — the guard belongs to `PlayerIcon`, whose palette is `ICON_PALETTE`, because `Canvas` does not know which palette it is drawn from. Leave the guard where it is, in `PlayerIcon::set`, before delegating.

- [ ] **Step 1:** Write failing tests for `Canvas` alone: a fresh canvas of edge 16 is blank and `get` is 0 everywhere; `set` then `get` round-trips; an off-grid `set` is dropped and an off-grid `get` is 0; `clear` blanks a painted canvas; two canvases of the same edge and cells are `PartialEq`.
- [ ] **Step 2:** Run `cargo test -p feral-processes-engine canvas` — expect failure, `Canvas` not found.
- [ ] **Step 3:** Implement `Canvas`.
- [ ] **Step 4:** Run the same, expect pass. Commit.
- [ ] **Step 5:** Re-seat `PlayerIcon` on `Canvas` — its field becomes `cells: Canvas`, its methods delegate, `ICON_CELLS` remains the codec's length. Change no signature and no behaviour.
- [ ] **Step 6:** Run `cargo test -p feral-processes-engine player_icon` **and** `icon`. Every existing test must pass **with no edit to the test file** — that is the gate on this refactor. Then `cargo test -p feral-processes-app-core icon` for the wizard's own coverage. Commit.

---

### Task 2: `SPRITE_PALETTE` and the quantiser

**Files:**
- Modify: `crates/engine/src/icon.rs`
- Test: `crates/engine/src/tests/player_icon.rs` (add a sprite-palette section) or a new `crates/engine/src/tests/sprite_palette.rs` registered in `crates/engine/src/tests/mod.rs`

**Interfaces:**
- Consumes: Task 1's `Canvas`.
- Produces:
  - `pub const SPRITE_PALETTE: [(u8, u8, u8); 19]` — a nine-step value ramp **biased bright**, then `ICON_PALETTE`'s ten hues in their existing order
  - `pub const SPRITE_ALPHA_THRESHOLD: u8`
  - `pub fn quantise(pixel: (u8, u8, u8, u8)) -> u8` — index into `SPRITE_PALETTE` **plus one**, 0 for transparent, matching `rgba`'s convention that index 0 is transparent and index *n* is palette entry *n-1*
  - `pub fn sprite_rgba(index: u8) -> (u8, u8, u8, u8)` — `quantise`'s inverse for a valid index

**Notes for the implementer:** the ramp is biased bright because the renderer applies the glyph's colour as a **multiplying** tint (see `assets/sprites/README.md`) — a mid-grey ramp step multiplied by a dim species hue lands near black. Nearest-colour matching by squared euclidean distance in RGB is sufficient here; do not reach for a perceptual space.

- [ ] **Step 1:** Write failing tests: every `SPRITE_PALETTE` entry quantises to its own index (the round-trip that makes the editor's own files loss-free); an alpha below `SPRITE_ALPHA_THRESHOLD` quantises to 0 whatever the colour; all 19 entries are distinct; **`ICON_PALETTE.len() == 15`**, named so the failure says it is the save format.
- [ ] **Step 2:** Run `cargo test -p feral-processes-engine sprite_palette` — expect failure.
- [ ] **Step 3:** Implement the constant, the threshold, `quantise` and `sprite_rgba`.
- [ ] **Step 4:** Run, expect pass. `cargo fmt`, `cargo clippy --workspace`. Commit.

---

### Task 3: `CanvasEditor`, and `IconEditor` composed on it

**Files:**
- Create: `crates/app-core/src/app/canvas_editor.rs`, registered in `crates/app-core/src/app/mod.rs`
- Modify: `crates/app-core/src/app/icon_editor.rs`
- Test: `crates/app-core/src/tests/icon_editor.rs` (existing — **must pass unchanged**), plus a new `crates/app-core/src/tests/canvas_editor.rs` registered in `crates/app-core/src/tests/mod.rs`

**Interfaces:**
- Consumes: `engine::icon::Canvas`.
- Produces:
  - `pub(crate) struct CanvasEditor { canvas: Canvas, cursor: (u8, u8), selected: u8, focus: CanvasFocus, brush: u8, history: VecDeque<Canvas>, stroke: bool }`
  - `CanvasEditor::open(canvas: Canvas, palette_len: u8) -> CanvasEditor`
  - `CanvasEditor::view(&self) -> CanvasView` where `pub struct CanvasView { pub cells: Vec<u8>, pub edge: u8, pub cursor: (u8, u8), pub selected: u8, pub focus: CanvasFocus, pub brush: u8 }`
  - `CanvasEditor::handle_key(&mut self, key: GameKey) -> CanvasKey` where `pub(crate) enum CanvasKey { Handled, Unhandled }`
  - `CanvasEditor::canvas(&self) -> &Canvas`, `CanvasEditor::set_canvas(&mut self, canvas: Canvas)`
  - `CanvasEditor::set_brush(&mut self, brush: u8)` (1 or 2; anything else ignored)
  - `CanvasEditor::begin_stroke(&mut self)`, `CanvasEditor::end_stroke(&mut self)`
  - `CanvasEditor::paint_at(&mut self, x: u8, y: u8, index: u8)`, `CanvasEditor::pick_swatch(&mut self, index: u8)`
  - `pub enum CanvasFocus { Canvas, Palette }` — replaces `IconFocus`
  - `IconEditor { editor: CanvasEditor, opened_with: PlayerIcon }`
  - `IconEditorView { canvas: CanvasView }` — the view **does** change shape (its flat `cells`, `cursor`, `selected` and `focus` move inside the `CanvasView`), so `crates/gui/src/render/icon_editor.rs` is updated with it in this task. `IconEditor`'s own methods — `open`, `icon`, `handle_key`, and its `IconEditorOutcome` — keep their signatures.

**Notes for the implementer:** three behaviours are the substance of this task and each gets its own test.

1. **The brush is a footprint and a step.** At brush 2 the cursor moves two cells and its coordinates snap to even numbers, and a paint writes the whole 2x2 block anchored there. At brush 1 nothing changes from today.
2. **The no-op guard survives the brush.** A paint whose block already holds `index` in every cell records no undo snapshot and changes nothing — the existing `paint` guard, widened from one cell to the block.
3. **A stroke is one undo entry.** `begin_stroke` snapshots once; every `paint_at` until `end_stroke` records nothing further. Outside a stroke, each paint records as it does today.

`IconEditor` opens its `CanvasEditor` with brush 1 and `ICON_PALETTE.len()`, and keeps its own `Enter`/`Esc` handling — those are its outcome, not the canvas's. Route them by taking `CanvasKey::Unhandled` back from the shared table.

- [ ] **Step 1:** Write the failing `CanvasEditor` tests — the three behaviours above, plus: neither cursor wraps at either edge; the palette cursor walks on all four arrows and clamps at both ends; `undo` past an empty history is a no-op; the history is capped at `ICON_UNDO_DEPTH`.
- [ ] **Step 2:** Run `cargo test -p feral-processes-app-core canvas_editor` — expect failure.
- [ ] **Step 3:** Implement `CanvasEditor`, moving the verbs out of `icon_editor.rs` rather than copying them.
- [ ] **Step 4:** Run, expect pass. Commit.
- [ ] **Step 5:** Re-seat `IconEditor` on `CanvasEditor`.
- [ ] **Step 6:** Run `cargo test -p feral-processes-app-core icon_editor` and `creation`. **The existing test file must not be edited** — that is this task's gate. Then `cargo test -p feral-processes-gui` for the renderer's view coupling. Commit.

---

### Task 4: `Mode::SpritePicker` — the subject list and both gates

**Files:**
- Modify: `crates/app-core/src/lib.rs` (the `Mode` enum), `crates/app-core/src/app/menus.rs` (the main-menu row), `crates/app-core/src/app/lifecycle.rs` (the new field's init), `crates/app-core/src/app/input.rs` (mode dispatch)
- Create: `crates/app-core/src/app/sprite_forge.rs`, registered in `crates/app-core/src/app/mod.rs`
- Modify: `crates/launcher/src/paths.rs` (a `sprites: PathBuf` on `DevPaths`), `crates/launcher/src/main.rs` (the install call, beside `install_dev_templates`)
- Modify: `crates/gui/src/render/mod.rs` (`ALL_MODES` 88 → 89, and `needs_status_banner`)
- Test: `crates/app-core/src/tests/sprite_forge.rs`, registered in `crates/app-core/src/tests/mod.rs`

**Interfaces:**
- Consumes: `engine::icon::Canvas`.
- Produces:
  - `Mode::SpritePicker`
  - `App::install_sprite_dir(&mut self, dir: PathBuf)` — mirrors `install_dev_templates`; installed unconditionally by the launcher, so the flag alone decides visibility
  - `App::install_sprite_library(&mut self, enabled: HashMap<String, Canvas>, disabled: HashSet<String>)`
  - `App::sprite_forge_enabled(&self) -> bool` — `dev_console::dev_flag("FERAL_DEV_SPRITES")` **and** a sprite dir installed, read once into a field at `App::new` the way `arena_enabled` is
  - `App::sprite_subjects(&self) -> Vec<SpriteSubject>` where `pub struct SpriteSubject { pub name: String, pub label: String, pub glyph: char, pub art: SpriteArt }` and `pub enum SpriteArt { None, On, Off }`
  - `pub(crate) fn handle_sprite_picker_key(&mut self, key: GameKey)`

**Notes for the implementer:**

- The subject list is every species def, every structure def, and the two names hardcoded in Rust: `anchor` (`crates/gui/src/render/base.rs:1379`) and the engine's `DEFAULT_PLAYER_SPRITE`. Get the sprite *name* from `SpeciesDef::sprite_name()` / `StructureDef::sprite_name()` — not from the id — because the `sprite:` override is exactly what decides which file the loader will look for. Sort by name for a stable screen, and de-duplicate: two defs may legitimately share one image.
- `SpriteArt` comes from the installed library plus a second installed set of *disabled* names; app-core does no file I/O. Task 8 fills both.
- `FERAL_DEV_SPRITES` goes through `dev_console::dev_flag`. Do not write a second env-var predicate — CLAUDE.md names that as drift the repo has already caught.
- The main-menu row is a `'d'` option pushed in `handle_main_menu_key`'s `options` vec beside `'r'`, gated on `self.sprite_forge_enabled`. `crates/gui/src/render/meta.rs:8` passes menu state to `main_menu_options`; extend that call the way `arena_enabled` is passed.
- `ALL_MODES` is a fixed-size array; its length changes with the enum. `needs_status_banner` must answer for the new mode, and the refusal census in `render/mod.rs` will fail loudly until the screen draws one — that failure is expected here and is closed by Task 7.

- [ ] **Step 1:** Write failing tests — with the flag off the menu has no `'d'` row and `Mode::SpritePicker` is unreachable; with the flag on and a dir installed the row is present; `sprite_subjects` returns one entry per species def, per structure def, plus `player` and `anchor`, sorted and de-duplicated; a subject whose name is in the installed library reads `SpriteArt::On`, one in the disabled set reads `Off`, otherwise `None`; `Esc` returns to `Mode::MainMenu`.
- [ ] **Step 2:** Run `cargo test -p feral-processes-app-core sprite_forge` — expect failure.
- [ ] **Step 3:** Implement the mode, the field, the installs, the subject derivation and the key handler.
- [ ] **Step 4:** Add `sprites` to `DevPaths` and install it from `main.rs`.
- [ ] **Step 5:** Extend `ALL_MODES` and `needs_status_banner`. Run `cargo test -p feral-processes-app-core sprite_forge` (pass) and `cargo test -p feral-processes-gui` (the refusal census is expected to fail on the undrawn mode — record that in the commit message; Task 7 closes it).
- [ ] **Step 6:** `cargo fmt`, `cargo clippy --workspace`. Commit.

---

### Task 5: `Mode::SpriteEditor` — editing, the write cue, and the pointer seam

**Files:**
- Modify: `crates/app-core/src/lib.rs` (`Mode`, and the pointer types), `crates/app-core/src/app/sprite_forge.rs`, `crates/app-core/src/app/input.rs`
- Modify: `crates/gui/src/render/mod.rs` (`ALL_MODES` 89 → 90, `needs_status_banner`)
- Test: `crates/app-core/src/tests/sprite_forge.rs`

**Interfaces:**
- Consumes: Task 3's `CanvasEditor`, Task 4's `SpriteSubject` and installs.
- Produces:
  - `Mode::SpriteEditor`
  - `pub(crate) struct SpriteEditor { editor: CanvasEditor, subject: String }`
  - `App::sprite_editor_view(&self) -> Option<SpriteEditorView>` where `pub struct SpriteEditorView { pub canvas: CanvasView, pub subject: String, pub palette: &'static [(u8, u8, u8)] }`
  - `App::take_sprite_writes(&mut self) -> Vec<SpriteWrite>` where `pub struct SpriteWrite { pub name: String, pub op: SpriteOp }` and `pub enum SpriteOp { Save(Canvas), Enable, Disable }`
  - `pub enum PointerHit { Cell(u8, u8), Swatch(u8) }`, `pub enum PointerButton { Primary, Secondary }`, `pub enum PointerPhase { Down, Drag, Up }`
  - `App::handle_pointer(&mut self, hit: PointerHit, button: PointerButton, phase: PointerPhase)`

**Notes for the implementer:**

- `take_sprite_writes` is the `take_sounds` / `take_transits` seam: app-core queues and forgets, the frontend drains. **app-core never opens a file** and never learns what a PNG is.
- `handle_pointer` is routed **only** while `Mode::SpriteEditor`; every other mode drops it. `PointerPhase::Down` calls `begin_stroke`, `Up` calls `end_stroke`, `Drag` neither. `PointerButton::Secondary` paints index 0 (erase), which is `Backspace`'s meaning already.
- The editor's own keys on top of the shared table: `[g]` toggles brush 1↔2, `[s]` queues a `Save`, `Esc` leaves without queueing anything. The picker's `[t]` queues `Enable` or `Disable` depending on the subject's current `SpriteArt`.
- A blank canvas saved is still a save. Unlike the player's drawn icon — where `sync_drawn_icon` filters a blank so the `@` falls back to the glyph — a blank *file* is a legitimate thing to author here, and disabling is the verb for "no art". Do not copy that filter.

- [ ] **Step 1:** Write failing tests — opening a subject with art loads the installed canvas and one without art opens blank; `[g]` toggles the brush and the view reports it; `[s]` queues exactly one `SpriteOp::Save` carrying the edited canvas; `Esc` queues nothing and returns to `Mode::SpritePicker`; the picker's `[t]` queues `Disable` for an `On` subject and `Enable` for an `Off` one; `take_sprite_writes` drains, so a second call is empty; a pointer `Down`, three `Drag`s across new cells and an `Up` is **one** undo entry; a pointer event in any other mode changes nothing.
- [ ] **Step 2:** Run `cargo test -p feral-processes-app-core sprite_forge` — expect failure.
- [ ] **Step 3:** Implement `SpriteEditor`, the cue queue, the pointer entry point and both key tables.
- [ ] **Step 4:** Extend `ALL_MODES` and `needs_status_banner`. Run the app-core suite, expect pass. Commit.

---

### Task 6: `draw_canvas` — extracted, with the icon editor rewired

**Files:**
- Create: `crates/gui/src/render/canvas.rs`, registered in `crates/gui/src/render/mod.rs`
- Modify: `crates/gui/src/render/icon_editor.rs`
- Test: `crates/gui/tests/` — extend whichever file already measures the icon editor screen

**Interfaces:**
- Consumes: `CanvasView`, `CanvasFocus`.
- Produces: `pub(crate) fn draw_canvas(p: &Painter, rect: Rect, view: &CanvasView, palette: &[(u8, u8, u8)])` — draws the cell grid, the grid lines, the cursor (sized to the brush) and the swatch row, and nothing else. The caller owns every other pixel on the screen.

**Notes for the implementer:** this is a pure extraction and **the icon editor screen must not change by one pixel**. `render/icon_editor.rs` keeps its own chrome — title, help line, preview cell, the Colour-step note — and calls `draw_canvas` for the middle. The cursor rectangle is now brush-sized: at brush 1 that is one cell, which is exactly today's behaviour, so the icon editor's appearance is unaffected by the parameter existing.

Take the origin from the caller as a `Rect`. CLAUDE.md's drawing-seam rule: a literal `0.0` in a pane draws under the stock strip and no test sees it.

- [ ] **Step 1:** Write a failing test that `draw_canvas` at brush 2 draws a cursor twice the edge of the brush-1 cursor, measured through `paint::with_painter`.
- [ ] **Step 2:** Run it, expect failure.
- [ ] **Step 3:** Extract `draw_canvas` and rewire `render/icon_editor.rs` onto it.
- [ ] **Step 4:** Run the gui suite. The icon editor's existing width test (`e47c66ea`, "measure the editor screen at the width it claims") is the gate that the extraction changed nothing. Commit.

---

### Task 7: The two screens

**Files:**
- Create: `crates/gui/src/render/sprite_forge.rs`, registered in `crates/gui/src/render/mod.rs`
- Modify: `crates/gui/src/render/mod.rs` (the per-mode draw dispatch), `crates/gui/src/render/meta.rs` (the main-menu row)
- Test: `crates/gui/tests/`

**Interfaces:**
- Consumes: `App::sprite_subjects`, `App::sprite_editor_view`, Task 6's `draw_canvas`.
- Produces: the two draw functions, dispatched from `render/mod.rs`.

**Notes for the implementer:**

- The picker is a list: each row is the subject's glyph in its own palette hue (`hud::palette::glyph` — CLAUDE.md names it as the one table a content hue is drawn from; do not reach for a second), the name, and the art state. **The screen has no scroll**, so with 49 subjects the row height and column count are a layout constraint and need a test that the longest list fits — the memory-page precedent (`memory-page-has-no-scroll`).
- The editor screen is `draw_canvas` plus a live preview cell drawn the way the map draws it, at map zoom, so the tint is visible where it will actually land.
- Both screens must draw the refusal banner, which is what closes the census failure Task 4 recorded.

- [ ] **Step 1:** Write failing tests: the picker's rows fit the screen at the smallest supported window with the full 49-subject list; the editor screen measures inside the width it claims; both modes draw the refusal exactly once (the `render/mod.rs` census, now expected to pass).
- [ ] **Step 2:** Run, expect failure.
- [ ] **Step 3:** Draw both screens and dispatch them; add the `'d'` row to `render/meta.rs`.
- [ ] **Step 4:** Run `cargo test -p feral-processes-gui`, expect pass — **including the refusal census Task 4 left red.** Commit.

---

### Task 8: PNG I/O, the drain, and the live rescan

**Files:**
- Modify: `crates/gui/Cargo.toml` (the `image` dependency), `crates/gui/src/sprites.rs`, `crates/gui/src/lib.rs` (the new `PreUpdate` system)
- Test: `crates/gui/tests/sprites.rs`

**Interfaces:**
- Consumes: `App::take_sprite_writes`, `engine::icon::{quantise, sprite_rgba, Canvas}`.
- Produces:
  - `sprites::canvas_to_png(canvas: &Canvas, path: &Path) -> std::io::Result<()>`
  - `sprites::png_to_canvas(path: &Path) -> Option<Canvas>` — decode, quantise, `None` on any failure
  - `sprites::scan_library(dir: &Path) -> (HashMap<String, Canvas>, HashSet<String>)` — the enabled canvases and the disabled names
  - a `PreUpdate` system that drains the cues, performs the write or the rename, and re-registers the affected name

**Notes for the implementer:**

- `image = { version = "0.25.10", default-features = false, features = ["png"] }`. Already in `Cargo.lock` at that version via bevy, so the graph grows by nothing. Bevy's own `png` feature is decode-only, which is why the encoder must be named here.
- **Reuse the existing pending/register pipeline rather than writing a second upload path.** `sprites::load` pushes `(name, handle)` onto `Sprites::pending` and `sprites::register` moves them into the table; the drain system should push onto the same `pending` after writing the file. `ImageSampler::nearest()` at load is what keeps the art crisp — bevy_egui binds the image's own sampler and bevy's default is linear.
- Disabling renames `<name>.png` to `<name>.png.off` and takes the name back out of the table with `SpriteTable::remove`. `scan_sprite_dir` filters on the extension being exactly `png`, so a `.off` file is already invisible to it — **do not touch the scanner**.
- The drain runs **before** `sprites::register` in `PreUpdate` so a save lands in the table on the same frame.
- Install the library into `App` at startup and refresh it after every drain, so the picker's art column and the editor's opening canvas cannot go stale.

- [ ] **Step 1:** Write failing tests against a temp directory: a canvas written and read back is the same canvas (the round-trip that Task 2's quantiser makes loss-free); a written file is 16x16 RGBA; a `.png.off` file is absent from `scan_sprite_dir` and present in `scan_library`'s disabled set; `png_to_canvas` on a corrupt file is `None` rather than a panic. The existing `the_shipped_sprites_are_one_cell` census must still pass.
- [ ] **Step 2:** Run `cargo test -p feral-processes-gui sprites` — expect failure.
- [ ] **Step 3:** Add the dependency and implement the codec and `scan_library`.
- [ ] **Step 4:** Implement the drain system and register it in `PreUpdate` ahead of `sprites::register`; wire the library install.
- [ ] **Step 5:** Run, expect pass. `cargo clippy --workspace`. Commit.

---

### Task 9: The mouse

**Files:**
- Modify: `crates/gui/src/render/sprite_forge.rs`, `crates/gui/src/render/canvas.rs`, `crates/gui/src/lib.rs` (wherever input reaches `App`)

**Interfaces:**
- Consumes: `App::handle_pointer`, `PointerHit`, `PointerButton`, `PointerPhase`.
- Produces: no new public surface — the gui reads egui's pointer state and calls `handle_pointer`.

**Notes for the implementer:**

- **`crates/gui` has no mouse handling today; this is the first, and it must stay on this one screen.** Read the pointer through the egui context the frame already holds. `paint.rs` stays the only file that names a graphics library — do not add a backend call inside `render/`; hit-test against the `Rect`s `draw_canvas` was already given.
- Emit `PointerHit::Cell` for the canvas rect and `PointerHit::Swatch` for the swatch row, both resolved from the pointer position and the rect the caller passed in. **App-core must never receive a pixel.**
- Phase: send `Down` on the press, `Drag` while held and moving, `Up` on release — including an `Up` when the pointer leaves the rect while held, or a stroke never ends and the next click joins it.

- [ ] **Step 1:** Write a failing test that a pointer position inside the canvas rect resolves to the expected cell at each of the four corners and one interior point, and that a position outside it resolves to no hit. Test the resolver as a pure function of `(pos, rect, edge)` so it needs no window.
- [ ] **Step 2:** Run, expect failure.
- [ ] **Step 3:** Implement the resolver and wire the egui pointer to `handle_pointer`.
- [ ] **Step 4:** Run `cargo test -p feral-processes-gui`, expect pass. Commit.

---

### Task 10: The documentation the change owes

**Files:**
- Modify: `assets/sprites/README.md`
- Modify: `docs/seams.md`, `.claude/skills/seams/` (the relevant reference file), `CLAUDE.md` + `AGENTS.md`

**Notes for the implementer:**

- `assets/sprites/README.md` is a schema document and this change alters what the directory may contain: record that `<name>.png.off` is a disabled sprite, that it is invisible to the scanner by extension, and that the dev editor is what writes both.
- **One new seam earns the three writes** (argument to `docs/seams.md`, trap to the `seams` skill, one-sentence rule to `CLAUDE.md`): *`ICON_PALETTE` is fifteen entries because it is the player icon's save format, and `SPRITE_PALETTE` is separate for that reason.* A sixteenth colour would encode as a hex digit meaning transparent, silently, on a file the player cannot get back. Nothing in the compiler holds this.
- `CLAUDE.md` and `AGENTS.md` are gitignored twins — edit `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`.
- **No `CHANGELOG.md` section and no version bump on this branch** — the release happens once, at the merge.
- Do **not** touch `docs/manual.md` or the root `README.md`; both are carved out of the documentation obligation.

- [ ] **Step 1:** Update `assets/sprites/README.md`.
- [ ] **Step 2:** Write the seam's argument into `docs/seams.md`, its trap into the `seams` skill, its one sentence into `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md`.
- [ ] **Step 3:** Run `cargo test --workspace` — **the final gate, and the first time the whole suite is asked.** Commit.

---

## Verification

The three acceptance criteria from the spec, checked by hand at the end:

1. **`FERAL_DEV_SPRITES` unset → the game is unchanged.** No menu row, `Mode::SpritePicker` unreachable, no behaviour difference. The same with the flag set in a build whose `DevPaths` is `None`.
2. **The player's icon editor behaves identically**, on its existing tests, unedited.
3. **Draw a structure's sprite, save, and it is on the map without a restart; toggle it off and the glyph returns with the file still on disk as `.png.off`.**

Criterion 3 needs the game running, and agents in this repo have no display. **Hand it to the user** — do not put "launch the game" in a dispatch, and do not treat a green suite as evidence of play.
