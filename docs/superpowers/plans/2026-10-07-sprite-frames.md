# Plan: two-frame asset sprites

**Spec:** `docs/superpowers/archive/specs/2026-10-07-sprite-frames-design.md`
**Branch:** `sprite-frames`, primary checkout (the user plays from here, so no
worktree).

There are two phases, and each fits one subagent. Phase A is the gui
renderer and loader. Phase B is the forge, in app-core plus its gui
read/write and view. Phase B depends on A's `SpriteFrames` type and its
`.anim.ron` helpers. TDD throughout, with a commit per green step. Every
dispatch forbids push, `git checkout`, `git stash` and `git add -A`.

## Shared definitions (Phase A creates these; B uses them)

All live in `crates/gui/src/sprites.rs` unless noted.

- `pub const DEFAULT_SPRITE_FRAME_MS: u32 = 600;`
- `pub const MAX_SPRITE_FRAMES: usize = 2;`
- `.anim.ron` schema: `#[derive(Deserialize, Serialize)] struct SpriteAnim
  { frame_ms: u32 }`.
  - `fn anim_path(dir, name) -> PathBuf` returns `<name>.anim.ron`. The
    name is the stem without `.colour`.
  - `pub fn read_frame_ms(dir, name) -> u32`. A missing file gives the
    default. A malformed file gives the default and logs a `warn!`.
- `fn frames_in(width, height) -> Option<usize>`: `Some(w / 16)` when
  `h == 16`, `w % 16 == 0` and `1 <= w/16 <= MAX`; otherwise `None`.
- `paint.rs`: `pub fn sprite_frame(now: f64, phase_key: u64, frames: usize,
  frame_ms: u32) -> usize`. This is pure. It uses fx's `PHASE_KEYS`, which
  becomes `pub(crate)`. The phase step is one frame_ms divided by
  `PHASE_KEYS`, so the 64 keys spread evenly across one frame.

---

## Phase A: renderer and loader (gui only)

**Files:** `gui/src/paint.rs`, `gui/src/sprites.rs`, `gui/src/fx.rs`
(visibility only), `gui/src/lib.rs`, `gui/src/render/{base,tactical,
battle,terrain,creation,notify}.rs`, `assets/sprites/README.md`,
`.claude/rules/drawing-seam.md`.

1. **`sprite_frame` (pure), test-first.**
   - 1 frame always gives 0.
   - 2 frames flip every `frame_ms`.
   - Two `phase_key`s give different frames at the same `now` for at least
     one sampled time.
2. **`SpriteTable` entries.**
   - The `(TextureId, bool)` entry becomes `struct SpriteEntry { texture,
     full_colour, frames: usize, frame_ms: u32 }`.
   - `insert` takes the frame count and `frame_ms`. Update `get`, `remove`
     and every caller.
3. **`Painter`.**
   - Add an `anim_now: Option<f64>` field, copied in `clipped()` (around
     line 356).
   - Add `pub fn set_anim_now(&mut self, now: Option<f64>)`.
   - `sprite(name, x, y, size, color, phase_key: u64)`: the UVs become
     `[f/frames, (f+1)/frames] x [0, 1]` with `f = sprite_frame(...)`.
   - Test: extend `painted_images` (paint.rs:614) or add a sibling that
     returns UVs.
     - A 2-frame entry draws `u ∈ [0.5, 1]` at a `now` that picks frame 2.
     - A 1-frame entry draws full UVs.
4. **`lib.rs`.** After the `\` toggle (around line 854), call
   `painter.set_anim_now(fe.fx.enabled.then_some(now))`. The field is
   `anim_now: Option<f64>`, and `None` means fx is off: frame 1, phase
   ignored. Don't use a `0.0` sentinel; review has already flagged one of
   those as a defect. Update the `for_frame` test caller at lib.rs:2083 if
   its signature changed.
5. **Call sites.**
   - `base.rs:1222` and `:1230` pass `ev.entity.to_bits()`, using the key
     the nearby fx calls use.
   - `tactical.rs:851` passes `body.entity.to_bits()`.
   - `battle.rs:866/870`, `terrain.rs:401`, `creation.rs:366/373` and
     `notify.rs:118` pass `0`.
6. **Loader.**
   - `register` (sprites.rs:537) gains `Res<Assets<Image>>`. On load it
     reads the image size and calls `frames_in`. On `None` it does
     `warn!` and skips the insert, so the glyph draws. Otherwise it
     inserts the frame count and `read_frame_ms(dir, stem)`.
   - Unit-test `frames_in`: 16x16→1, 32x16→2, 24x16→None, 48x16→None,
     16x32→None.
   - Unit-test `read_frame_ms` for a missing file, a valid file and a
     malformed file, using `codec_test_dir`.
   - Add a test that every shipped sprite in `shipped_sprites_dir()` is
     16x16, i.e. `frames_in == Some(1)`.
7. **Docs.**
   - `assets/sprites/README.md`: the sheet layout, `.anim.ron`, the
     default, and that the sheet shares one variant and one `.off` state.
   - `drawing-seam.md`: one clause saying `sprite` takes a `phase_key` and
     picks a frame.

**Gate A:**
- `cargo test -p feral-processes-gui`
- `cargo clippy --workspace --all-targets`
- `cargo fmt`
- The game launches. Screenshot a `--template stack` map to confirm 16x16
  art is unchanged.

---

## Phase B: the Sprite Forge (app-core + gui forge I/O and view)

**Files:** `app-core/src/app/sprite_forge.rs`,
`app-core/src/app/canvas_editor.rs` (one accessor),
`app-core/src/tests/sprite_forge.rs`, `gui/src/sprites.rs` (the scan and
write paths), `gui/src/render/sprite_forge.rs`.

1. **Types (app-core).**
   - `InstalledSprite { frames: Vec<Canvas>, frame_ms: u32, full_colour }`
     replaces `canvas`.
   - `SpriteOp::Save { frames: Vec<Canvas>, frame_ms: u32, full_colour }`.
   - `saves_as_full_colour(frames: &[Canvas])` is true if any frame needs
     colour.
   - App-core can't see gui's `DEFAULT_SPRITE_FRAME_MS`, so a fresh blank
     subject takes the `frame_ms` the gui supplies. Simplest: app-core
     defines `pub const DEFAULT_SPRITE_FRAME_MS` and gui imports it. Keep
     one definition.
   - Clamp constants live in sprite_forge.rs: `SPRITE_FRAME_MS_STEP = 100`,
     `MIN = 100`, `MAX = 2000`.
2. **`SpriteEditor` state.**
   - Add `frames: Vec<Canvas>`, `active_frame: usize`, `frame_ms: u32` and
     `history: VecDeque<(Vec<Canvas>, usize)>` (depth `ICON_UNDO_DEPTH`).
   - `CanvasEditor` holds the active frame. On a switch, the forge writes
     `editor.canvas()` back into `frames[active]`, then calls `set_canvas`
     with the new frame.
   - Add `pub(crate) fn undo_depth(&self) -> usize` to `CanvasEditor`.
3. **Keys** (in `handle_sprite_editor_key`, ahead of delegation):
   - `1` and `2` switch frames. The first `2` on a 1-frame sprite pushes a
     copy of frame 1 (recorded in the forge history).
   - `-` and `=` step and clamp `frame_ms`.
   - `D` deletes frame 2 (recorded) and returns to frame 1.
   - `u` is the forge's own undo, which restores frames and the active
     frame and then `set_canvas`.
   - **Delegation wrapper:** snapshot `(frames-with-current-canvas,
     active)`, delegate, and push the snapshot if `undo_depth` grew. Use
     it for both keys and `handle_pointer`.
4. **Buttons.** Add `EditorButton` variants `Frame1`, `Frame2`,
   `DeleteFrame`, `Slower` and `Faster`, mapped to `1 2 D - =`, and update
   `ALL`. `SpriteEditorView` gains `frames: Vec<CanvasCells>` (or the
   frame count plus the inactive frame's cells for the preview),
   `active_frame` and `frame_ms`.
5. **App-core tests** (`tests/sprite_forge.rs`, using the existing
   helpers):
   - `2` copies frame 1 once, and a second `2` doesn't recopy.
   - Painting on frame 2 leaves frame 1 alone.
   - `1` then `2` keeps both frames.
   - `D` gives one frame and active = 0.
   - `u` after `D` restores frame 2.
   - `u` after the copying `2` removes frame 2.
   - `-` and `=` clamp at both ends.
   - `s` queues one `Save` with both frames and `frame_ms`.
   - A colour index used only on frame 2 picks the colour variant, unless
     the variant was already pinned.
   - Each new button presses its key.
6. **Gui I/O (`sprites.rs`).**
   - `png_to_canvas` becomes `png_to_frames(path) -> Option<Vec<Canvas>>`,
     which splits a 32x16 image using `frames_in`. Update the existing
     refuse-wrong-size test so 32x16 is accepted and 24x16 refused.
   - `canvas_to_png` becomes `frames_to_png(&[Canvas], path)`, which writes
     a `16·n x 16` image.
   - `scan_library` fills `frames` and `frame_ms` (`read_frame_ms`).
   - The `apply_sprite_write` `Save` arm writes `.anim.ron` when there are
     2+ frames and removes it when there is 1.
   - Tests:
     - A two-frame round trip.
     - The written file is 32x16.
     - A one-frame save removes a stale `.anim.ron`.
     - Saving two frames writes a `frame_ms` that reads back.
7. **Forge view (`render/sprite_forge.rs`).**
   - The button bar gains frame tabs, with the active one highlighted, and
     a speed readout in ms. Never in ticks (memory:
     no-player-facing-tick-vocabulary).
   - The preview cell draws `frames[sprite_frame(now, 0, n, frame_ms)]`.
     The view needs `now`; take it from `Painter.anim_now`, and show frame 1 when it is `None`.
   - Extend the existing render tests: a 2-frame view draws both tab
     labels, and the preview draws the frame `sprite_frame` picks.
   - Check the popup and button-bar width against
     `gui-text-never-clips-or-wraps`.

**Gate B:**
- `cargo test --workspace`
- `cargo clippy --workspace --all-targets`
- `cargo fmt`
- A manual forge check with `FERAL_DEV_SPRITES=1`:
  1. Open `construct` and press `2`.
  2. Paint a pixel and press `=` twice.
  3. Press `s`.
  4. Confirm `assets/sprites/construct*.png` is 32x16 and
     `construct.anim.ron` exists.
  5. Confirm the map alternates.
  6. Revert the asset files after the check unless the user wants to keep
     them.

## Final

- An opus whole-branch review, with the diff given as a file. Before
  shipping, mutation-check `sprite_frame`'s phase handling and the
  `.anim.ron` removal.
- Land with the `skills:deploy` skill: patch bump plus a `CHANGELOG.md`
  entry.
- Add `docs/superpowers/INDEX.md` lines for the spec and the plan.
