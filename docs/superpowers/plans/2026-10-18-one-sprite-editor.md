# One Sprite Editor Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** The character wizard's Icon step opens the Sprite Forge's editor, and the player's avatar becomes a 16x16, two-frame sprite on the sprite palette stored as a `v3` string.

**Architecture:** The Forge's `SpriteEditor` mechanics move into their own app-core module and learn to report a key as handled or not; the Forge and the wizard each host it in a few lines keyed on `Mode`. `PlayerIcon` in the engine becomes one or two 16x16 `Canvas` frames plus a pace, with a `v3` codec that folds `v1`/`v2` forward. The gui draws both hosts through one function and uploads the avatar as a `16n x 16` sheet like any asset sprite.

**Tech Stack:** Rust workspace; `bevy_ecs` engine, app-core state machine, `bevy` + `bevy_egui` renderer. No new dependencies.

**Spec:** `docs/superpowers/specs/2026-10-18-one-sprite-editor-design.md`

## Global Constraints

- Work in worktree `.claude/worktrees/one-sprite-editor`, branch `worktree-one-sprite-editor`. Never `git checkout`, `git stash`, or push.
- Every commit compiles and passes `cargo test --workspace`; `cargo fmt` and `cargo clippy --workspace --all-targets` clean after every task.
- No `SAVE_FORMAT_VERSION` bump. `Profile::player_icon` and the save's `icon: Option<String>` stay `Option<String>`.
- Codec: `v3:<frame_ms>:<frame>[:<frame>]`, `frame_ms` decimal, each frame exactly 512 lowercase hex digits (256 cells, two digits each, row-major). Cell `0` transparent, `1..=SPRITE_PALETTE.len()` (63) a palette index. `decode` refuses anything malformed and never clamps or masks. `encode` always writes `v3`.
- `v1` and `v2` fold forward: `v2` scales each 8x8 cell to a 2x2 block; both remap old index `n` to `quantise((r, g, b, 255))` of the old colour `n-1`, old `0` stays `0`. One frame at `DEFAULT_SPRITE_FRAME_MS`.
- `ICON_GRID`, `ICON_CELL_PIXELS` and `PlayerIcon::pixel_rgba` are deleted. `ICON_PALETTE` becomes private. `ICON_SIZE` (16) stays.
- The drawn avatar is drawn untinted everywhere, including the wizard's editor preview.
- Dev-only things (picker, `.off`, PNG writes, subject hue, Save) stay behind `FERAL_DEV_SPRITES`.
- Player-facing copy is plain language; no code names, no tick vocabulary.
- Seam edits are three writes in the `seams` skill's order: memory graph, `.claude/skills/seams/references/hud.md`, `.claude/rules/*.md`.

## Review Focus

- A profile string with a prefix this binary does not know (a future `v4:`) must decode to `None` and the default avatar, never panic. Test in Task 2.
- A `v3` string with `frame_ms` `0` must be refused: the animation clock divides by the pace. Test in Task 2.
- Every legacy index `1..=15` must remap to an index in `1..=63`, so an old drawing never produces a cell `set` would refuse. Test in Task 2.
- A pointer event in `Mode::CreateCharacter` while the editor is closed must change nothing in the wizard's choice. Test in Task 3.
- A primary click on the wizard's Keep button must go through the wizard's own key handler and advance the step, exactly as `Enter` does. Test in Task 3.

---

### Task 1: Extract `SpriteEditor` into its own module

A behaviour-preserving refactor. The Forge works exactly as before; the editor no longer knows its subject.

**Files:**
- Create: `crates/app-core/src/app/sprite_editor.rs`
- Modify: `crates/app-core/src/app/sprite_forge.rs` (struct at 186-355, `EditorButton`/pointer types at 420-500, `handle_sprite_editor_key` 808-858, `handle_editor_pointer` 912-955, `sprite_editor_view` 788)
- Modify: `crates/app-core/src/app/mod.rs:27` area (add `pub(crate) mod sprite_editor;`), `crates/app-core/src/lib.rs:32` re-exports, the `App.sprite_editor` field
- Modify: `crates/gui/src/render/sprite_forge.rs:379-500` (`button_label`, `draw_button_bar`, `draw_sprite_editor`)
- Test: `crates/app-core/src/tests/sprite_forge.rs` (existing, must stay green), new tests in `sprite_editor.rs`

**Interfaces:**
- Produces, in `crates/app-core/src/app/sprite_editor.rs`:
  - `pub const MAX_SPRITE_FRAMES`, `DEFAULT_SPRITE_FRAME_MS`, `SPRITE_FRAME_MS_STEP/MIN/MAX` (moved verbatim from `sprite_forge.rs:38-48`; Task 2 moves the first two on again, to the engine, and leaves `pub use` here).
  - `pub enum SubjectTint`, `pub enum EditorButton`, `pub enum PointerHit`, `pub enum PointerButton`, `pub enum PointerPhase` (moved verbatim; `sprite_forge.rs` imports them).
  - `pub enum EditorButton` gains `Keep` (key `GameKey::Enter`) and `Discard` (key `GameKey::Esc`). `EditorButton::ALL` is renamed `FORGE_BUTTONS` (same eleven); `pub const WIZARD_BUTTONS: [EditorButton; 11] = [Keep, Discard, Undo, Clear, Brush, Tool, Frame1, Frame2, DeleteFrame, Slower, Faster]`.
  - `pub struct EditorChrome { pub title: String, pub buttons: Vec<EditorButton>, pub tint: Option<SubjectTint>, pub full_colour: bool }` — what differs by host.
  - `pub struct SpriteEditorView` (moved from `sprite_forge.rs:356`): field `subject: String` becomes `title: String`; new fields `buttons: Vec<EditorButton>`, `tint: Option<SubjectTint>`; the rest unchanged.
  - `pub(crate) enum SpriteKey { Handled, Unhandled }`.
  - `pub(crate) struct SpriteEditor` with fields `editor: CanvasEditor, frames: Vec<Canvas>, active_frame: usize, frame_ms: u32, history: VecDeque<(Vec<Canvas>, usize)>`. No `subject`, no `full_colour`.
  - `pub(crate) fn open(frames: Vec<Canvas>, frame_ms: u32) -> SpriteEditor` (empty `frames` opens one blank `Canvas::new(ICON_SIZE)`).
  - `pub(crate) fn handle_key(&mut self, key: GameKey) -> SpriteKey`: `g` brush toggle 1<->2, `f` tool, `1`/`2` frame, `D` delete second frame, `-`/`=` pace, `u` undo, `i` pick colour at cursor, else `CanvasEditor::handle_key` through `delegate`, mapping `CanvasKey::Handled/Unhandled` onto `SpriteKey`. `Enter`, `Esc`, `s` fall through to `CanvasEditor` and come back `Unhandled`.
  - `pub(crate) fn handle_pointer(&mut self, hit: PointerHit, button: PointerButton, phase: PointerPhase)`: the body of today's `handle_editor_pointer` from `sprite_editor.delegate(...)` down; `PointerHit::Button` and `Subject` are ignored here (the host handles buttons).
  - `pub(crate) fn frames(&self) -> Vec<Canvas>` (today's `current_frames`), `pub(crate) fn frame_ms(&self) -> u32`.
  - `pub(crate) fn view(&self, chrome: EditorChrome) -> SpriteEditorView`.
- Produces, in `sprite_forge.rs`: `pub(crate) struct ForgeSession { pub(crate) editor: SpriteEditor, pub(crate) subject: String, pub(crate) full_colour: Option<bool> }`; `App.sprite_editor: Option<ForgeSession>`. `saves_as_full_colour` and the pinned `full_colour` rule stay here, reading `editor.frames()`.
- Consumes: `CanvasEditor`, `CanvasKey`, `Tool` from `canvas_editor.rs` unchanged.

- [ ] **Step 1: Write the failing tests** in a `#[cfg(test)] mod tests` at the bottom of the new `sprite_editor.rs`

```rust
#[test]
fn enter_esc_and_s_come_back_unhandled() {
    let mut editor = SpriteEditor::open(Vec::new(), DEFAULT_SPRITE_FRAME_MS);
    for key in [GameKey::Enter, GameKey::Esc, GameKey::Char('s')] {
        assert_eq!(editor.handle_key(key), SpriteKey::Unhandled, "{key:?}");
    }
}

#[test]
fn the_forge_keys_are_handled_and_change_the_editor() {
    let mut editor = SpriteEditor::open(Vec::new(), DEFAULT_SPRITE_FRAME_MS);
    assert_eq!(editor.handle_key(GameKey::Char('2')), SpriteKey::Handled);
    assert_eq!(editor.frames().len(), 2);
    assert_eq!(editor.handle_key(GameKey::Char('=')), SpriteKey::Handled);
    assert_eq!(editor.frame_ms(), DEFAULT_SPRITE_FRAME_MS + SPRITE_FRAME_MS_STEP);
}

#[test]
fn the_view_carries_the_chrome_it_was_given() {
    let editor = SpriteEditor::open(Vec::new(), DEFAULT_SPRITE_FRAME_MS);
    let view = editor.view(EditorChrome {
        title: "x".into(),
        buttons: WIZARD_BUTTONS.to_vec(),
        tint: None,
        full_colour: false,
    });
    assert_eq!(view.title, "x");
    assert_eq!(view.buttons, WIZARD_BUTTONS.to_vec());
    assert_eq!(view.tint, None);
}

#[test]
fn keep_is_enter_and_discard_is_esc() {
    assert_eq!(EditorButton::Keep.key(), GameKey::Enter);
    assert_eq!(EditorButton::Discard.key(), GameKey::Esc);
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-app-core sprite_editor`
Expected: compile error, `sprite_editor` module not found.

- [ ] **Step 3: Create `sprite_editor.rs`** with the items in Interfaces, moving code out of `sprite_forge.rs` rather than copying it. `sprite_forge.rs` keeps `dev_sprite_forge_flag`, `SpriteArt`, `SpriteSubject`, `InstalledSprite`, `saves_as_full_colour`, `SpriteWrite`/`SpriteOp`, the picker, and `ForgeSession`.

- [ ] **Step 4: Rewrite the Forge host** in `sprite_forge.rs`: `handle_sprite_editor_key` matches `Esc` (close, `Mode::SpritePicker`), `Char('s')` (queue `SpriteOp::Save` from `session.editor.frames()` and `frame_ms()`), else `session.editor.handle_key(key)` and ignore the result. `handle_editor_pointer`: a `PointerHit::Button` on primary `Down` calls `handle_sprite_editor_key(pressed.key())`; anything else goes to `session.editor.handle_pointer`. `sprite_editor_view` builds `EditorChrome { title: session.subject.clone(), buttons: FORGE_BUTTONS.to_vec(), tint: Some(subject tint looked up as `draw_sprite_editor` does today at `render/sprite_forge.rs:450-459`, moved here), full_colour: session.full_colour() }`.

- [ ] **Step 5: Update the renderer** in `crates/gui/src/render/sprite_forge.rs`: `draw_sprite_editor` passes `view.tint.as_ref().map(subject_hue).unwrap_or(Color::new(1.0, 1.0, 1.0, 1.0))` as the hue; the header prints `view.title`; `draw_button_bar` and `hit_rects` iterate `view.buttons` instead of `EditorButton::ALL`; `button_label` gains `Keep => "[Enter] Keep"` and `Discard => "[Esc] Discard"`. Fix every import of the moved types.

- [ ] **Step 6: Run the gates**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt --check`
Expected: all green; `tests/sprite_forge.rs` unchanged and passing.

- [ ] **Step 7: Commit**

```bash
git add crates/app-core/src/app/sprite_editor.rs crates/app-core/src/app/sprite_forge.rs crates/app-core/src/app/mod.rs crates/app-core/src/lib.rs crates/gui/src/render/sprite_forge.rs
git commit -m "Sprite editor: the mechanics move into their own module and report a key as handled or not"
```

---

### Task 2: The `v3` avatar codec, and the avatar uploads as a sprite sheet

The engine's `PlayerIcon` becomes frames plus a pace. The old wizard editor and the gui are adapted just enough to compile and keep their tests meaningful; Task 3 deletes the old editor.

**Files:**
- Modify: `crates/engine/src/icon.rs` (constants 194-221, `PlayerIcon` 289-430, tests 480-699), `crates/engine/src/lib.rs:139-141` re-exports
- Modify: `crates/app-core/src/app/sprite_editor.rs` (constants become `pub use feral_processes_engine::{DEFAULT_SPRITE_FRAME_MS, MAX_SPRITE_FRAMES}`), `crates/app-core/src/app/icon_editor.rs` (compile fix only)
- Modify: `crates/gui/src/sprites.rs:255-312` (`sync_drawn_icon`) and its tests 715-897; `crates/gui/src/render/icon_editor.rs` (compile fix only: palette from `SPRITE_PALETTE`)
- Test: `crates/engine/src/icon.rs` tests, `crates/gui/src/sprites.rs` tests, `crates/app-core/src/tests/creation.rs:252` (`entering_the_icon_step_seeds_from_a_v1_profile`)

**Interfaces:**
- Produces, in `icon.rs`:
  - `pub const MAX_SPRITE_FRAMES: usize = 2;` and `pub const DEFAULT_SPRITE_FRAME_MS: u32 = 600;` (moved from app-core; re-exported from `engine/src/lib.rs`).
  - `pub struct PlayerIcon { frames: Vec<Canvas>, frame_ms: u32 }` with `Default` = one blank `Canvas::new(ICON_SIZE)` at `DEFAULT_SPRITE_FRAME_MS`.
  - `pub fn new(frames: Vec<Canvas>, frame_ms: u32) -> Option<PlayerIcon>`: `None` when `frames` is empty or longer than `MAX_SPRITE_FRAMES`, any canvas edge is not `ICON_SIZE`, any cell is above `SPRITE_PALETTE.len()`, or `frame_ms == 0`.
  - `pub fn frames(&self) -> &[Canvas]`, `pub fn frame_ms(&self) -> u32`, `pub fn is_blank(&self) -> bool` (every frame blank), `pub fn rgba(&self, frame: usize, x: usize, y: usize) -> (u8, u8, u8, u8)` as `sprite_rgba(self.frames[frame].get(x, y))`.
  - `pub fn encode(&self) -> String`, `pub fn decode(s: &str) -> Option<PlayerIcon>`.
  - Deleted: `get`, `set`, `clear`, `pixel_rgba`, `ICON_GRID`, `ICON_CELL_PIXELS`, the `pub` on `ICON_PALETTE` (renamed `LEGACY_ICON_PALETTE`, private).
  - Private: `fn legacy_index(old: u8) -> u8` (0 → 0, `n` → `quantise((r, g, b, 255))` of `LEGACY_ICON_PALETTE[n-1]`); `fn decode_v3`, `fn decode_v2` (8x8 → 2x2 blocks), `fn decode_v1` (16x16, remap only, replacing today's block-majority fold).
- Consumes: `SPRITE_PALETTE`, `quantise`, `sprite_rgba`, `Canvas` already in `icon.rs`.
- The gui's `Sprites::sync_drawn_icon` signature is unchanged; it uploads width `ICON_SIZE * icon.frames().len()`, height `ICON_SIZE`, frame `f` at x offset `f * ICON_SIZE`, and calls `table.insert(DRAWN_ICON_KEY, id, icon.frames().len(), icon.frame_ms())`.

- [ ] **Step 1: Write the failing engine tests**, replacing the `PlayerIcon` tests from `a_drawn_icon_round_trips_through_encode_and_decode_unchanged` through `a_drawn_cell_covers_its_whole_pixel_block` (the `Canvas` tests above them stay). A local helper `fn painted(frames: &[&[(usize, usize, u8)]], frame_ms: u32) -> PlayerIcon` builds an icon from cell lists.

```rust
#[test]
fn a_two_frame_icon_round_trips_through_encode_and_decode_unchanged() {
    let icon = painted(&[&[(0, 0, 1), (15, 15, 63)], &[(7, 7, 30)]], 450);
    let s = icon.encode();
    assert!(s.starts_with("v3:450:"));
    assert_eq!(s.len(), "v3:450:".len() + 512 + 1 + 512);
    assert_eq!(PlayerIcon::decode(&s), Some(icon));
}

#[test]
fn a_default_icon_encodes_to_one_frame_of_512_zeros_at_the_default_pace() {
    assert_eq!(PlayerIcon::default().encode(), format!("v3:{DEFAULT_SPRITE_FRAME_MS}:{}", "0".repeat(512)));
}

#[test]
fn decode_refuses_every_malformed_shape() {
    let frame = "0".repeat(512);
    for bad in [
        format!("v4:600:{frame}"),              // unknown future prefix
        format!("v3:600:"),                     // no frame
        format!("v3:600:{frame}:{frame}:{frame}"), // three frames
        format!("v3:600:{}", "0".repeat(511)),
        format!("v3:600:{}", "0".repeat(513)),
        format!("v3:600:{}g{}", "0".repeat(255), "0".repeat(256)),
        format!("v3:600:{}40{}", "0".repeat(254), "0".repeat(256)), // 0x40 = 64 > 63
        format!("v3:0:{frame}"),
        format!("v3:-5:{frame}"),
        format!("v3:six:{frame}"),
        format!("v3::{frame}"),
    ] {
        assert_eq!(PlayerIcon::decode(&bad), None, "{bad:.40}");
    }
}

#[test]
fn any_positive_pace_decodes_and_the_top_palette_index_round_trips() {
    let icon = painted(&[&[(3, 3, 63)]], u32::MAX);
    assert_eq!(PlayerIcon::decode(&icon.encode()), Some(icon));
}

#[test]
fn new_refuses_what_decode_refuses() {
    let c = Canvas::new(ICON_SIZE);
    assert!(PlayerIcon::new(vec![], 600).is_none());
    assert!(PlayerIcon::new(vec![c.clone(); 3], 600).is_none());
    assert!(PlayerIcon::new(vec![Canvas::new(8)], 600).is_none());
    assert!(PlayerIcon::new(vec![c.clone()], 0).is_none());
    let mut over = c.clone();
    over.set(0, 0, 64);
    assert!(PlayerIcon::new(vec![over], 600).is_none());
    assert!(PlayerIcon::new(vec![c], 600).is_some());
}

#[test]
fn a_v2_cell_becomes_a_2x2_block_in_the_remapped_colour() {
    let mut digits = "0".repeat(64).into_bytes();
    digits[8 * 1 + 2] = b'f'; // cell (2, 1), old index 15
    let icon = PlayerIcon::decode(&format!("v2:{}", String::from_utf8(digits).unwrap())).unwrap();
    assert_eq!(icon.frames().len(), 1);
    assert_eq!(icon.frame_ms(), DEFAULT_SPRITE_FRAME_MS);
    let want = legacy_index(15);
    assert!((1..=63).contains(&want));
    for (x, y) in [(4, 2), (5, 2), (4, 3), (5, 3)] {
        assert_eq!(icon.frames()[0].get(x, y), want, "({x},{y})");
    }
    assert_eq!(icon.frames()[0].get(6, 2), 0);
    assert!(icon.encode().starts_with("v3:"));
}

#[test]
fn a_v1_pixel_keeps_its_place_and_takes_the_remapped_colour() {
    let mut digits = "0".repeat(256).into_bytes();
    digits[16 * 9 + 13] = b'3';
    let icon = PlayerIcon::decode(&format!("v1:{}", String::from_utf8(digits).unwrap())).unwrap();
    assert_eq!(icon.frames()[0].get(13, 9), legacy_index(3));
    assert_eq!(icon.frames()[0].get(12, 9), 0);
}

#[test]
fn every_legacy_index_remaps_inside_the_sprite_palette() {
    assert_eq!(legacy_index(0), 0);
    for n in 1..=15u8 {
        assert!((1..=SPRITE_PALETTE.len() as u8).contains(&legacy_index(n)), "{n}");
    }
}

#[test]
fn rgba_is_transparent_at_index_zero_and_the_palette_colour_otherwise() {
    let icon = painted(&[&[(1, 1, 5)]], 600);
    assert_eq!(icon.rgba(0, 0, 0).3, 0);
    let (r, g, b) = SPRITE_PALETTE[4];
    assert_eq!(icon.rgba(0, 1, 1), (r, g, b, 255));
}

#[test]
fn is_blank_needs_every_frame_blank() {
    assert!(PlayerIcon::default().is_blank());
    assert!(!painted(&[&[], &[(0, 0, 1)]], 600).is_blank());
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine icon`
Expected: compile errors on `frames`, `new`, `legacy_index`, `DEFAULT_SPRITE_FRAME_MS`.

- [ ] **Step 3: Implement the new `PlayerIcon`** in `icon.rs` per Interfaces. `decode` splits on `:` into prefix, pace, frames; `decode_v3` parses the pace with `str::parse::<u32>` (which refuses `-5`, `six`, and empty) and rejects `0`, then each frame as 256 pairs of `char::to_digit(16)`, then goes through `PlayerIcon::new` so the one guard is shared. Delete `decode_v1`'s block-majority logic and its doc comment.

- [ ] **Step 4: Move the two constants** from `sprite_editor.rs` to `icon.rs`, re-export from `engine/src/lib.rs` (replace `ICON_CELL_PIXELS, ICON_GRID, ICON_PALETTE` in the `pub use icon::{...}` list), and make `sprite_editor.rs` `pub use feral_processes_engine::{DEFAULT_SPRITE_FRAME_MS, MAX_SPRITE_FRAMES};` so app-core and gui callers compile unchanged.

- [ ] **Step 5: Compile-fix the old wizard editor** (deleted in Task 3, so smallest edits only): in `app-core/src/app/icon_editor.rs`, `canvas_from_icon` returns `icon.frames()[0].clone()`, `icon_from_canvas` returns `PlayerIcon::new(vec![canvas.clone()], DEFAULT_SPRITE_FRAME_MS).expect("the editor paints palette indices on a 16x16 canvas")`, `open` uses `SPRITE_PALETTE.len()` and `ICON_SIZE`. In `gui/src/render/icon_editor.rs` read swatches from `SPRITE_PALETTE`. Update `a_v1_profile` expectations in `app-core/src/tests/creation.rs:252` to the lossless fold.

- [ ] **Step 6: Write the failing gui upload tests** in `crates/gui/src/sprites.rs`, replacing `a_drawn_cell_is_uploaded_as_its_whole_pixel_block` and `the_uploaded_pixels_are_the_icons_own_rgba`; keep the other five, adjusted to build icons through `PlayerIcon::new`.

```rust
#[test]
fn a_two_frame_icon_uploads_a_32x16_sheet_and_registers_its_pace() {
    // frame 0: cell (1, 0) = 5; frame 1: cell (2, 3) = 9; pace 450
    ... build icon, call sync_drawn_icon ...
    let image = images.get(&handle).unwrap();
    assert_eq!((image.width(), image.height()), (32, 16));
    assert_eq!(pixel(image, 1, 0), sprite_rgba(5));
    assert_eq!(pixel(image, 16 + 2, 3), sprite_rgba(9));
    assert_eq!(pixel(image, 2, 3), (0, 0, 0, 0));
    let entry = sprites.table().get(DRAWN_ICON_KEY).unwrap();
    assert_eq!((entry.frames, entry.frame_ms), (2, 450));
}

#[test]
fn a_one_frame_icon_uploads_a_16x16_sheet() {
    ... assert (16, 16) and entry.frames == 1 ...
}
```

Read how the existing tests at `sprites.rs:715-897` build `images`, `textures` and `sprites` and reuse those helpers; `sprites.table().get(DRAWN_ICON_KEY)` returns `Option<SpriteEntry>` (`paint.rs:201`) with `pub(crate) frames: usize` and `frame_ms: u32`.

- [ ] **Step 7: Run them to verify they fail**

Run: `cargo test -p feral-processes-gui sprites::`
Expected: width assertion fails (16, not 32) and the entry reports 1 frame at 600.

- [ ] **Step 8: Rewrite `sync_drawn_icon`'s upload** per Interfaces, through `icon.rgba(frame, x, y)`. Update the doc comments at `sprites.rs:250-262` (the 8x8/2x2 paragraph goes).

- [ ] **Step 9: Run the gates**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt --check`
Expected: green. If `rg -n 'ICON_GRID|ICON_CELL_PIXELS|pixel_rgba|ICON_PALETTE' crates` prints anything outside `icon.rs`, fix it.

- [ ] **Step 10: Commit**

```bash
git add crates/engine/src/icon.rs crates/engine/src/lib.rs crates/app-core/src/app/sprite_editor.rs crates/app-core/src/app/icon_editor.rs crates/app-core/src/tests/creation.rs crates/gui/src/sprites.rs crates/gui/src/render/icon_editor.rs
git commit -m "Avatar: a v3 codec of one or two 16x16 frames on the sprite palette, folding v1 and v2 forward, uploaded as a sprite sheet"
```

---

### Task 3: The wizard hosts the sprite editor

**Files:**
- Delete: `crates/app-core/src/app/icon_editor.rs`, `crates/app-core/src/tests/icon_editor.rs`, `crates/gui/src/render/icon_editor.rs`
- Modify: `crates/app-core/src/app/mod.rs:27`, `crates/app-core/src/tests/mod.rs:32`, `crates/app-core/src/lib.rs:24` and `:3612-3613`, `crates/app-core/src/app/creation.rs:396-416` and `:740-760`, `crates/app-core/src/app/sprite_forge.rs` (`handle_pointer` match, 871-880)
- Modify: `crates/gui/src/render/mod.rs:59` and `:648-654`, `crates/gui/src/render/sprite_forge.rs` (`sprite_hits` 727-750, `draw_sprite_editor_session` visibility), `crates/gui/src/lib.rs:211-275` and `:921` comments
- Test: `crates/app-core/src/tests/creation.rs:1364-1825`, `crates/gui/src/render/sprite_forge.rs` tests, `crates/gui/src/render/creation.rs` tests near 1111-1150

**Interfaces:**
- Consumes: `SpriteEditor`, `SpriteKey`, `EditorChrome`, `WIZARD_BUTTONS`, pointer types from Task 1; `PlayerIcon::new/frames/frame_ms/is_blank` from Task 2.
- Produces:
  - `App.creation_icon_editor: Option<SpriteEditor>` (field keeps its name).
  - `pub fn App::creation_sprite_view(&self) -> Option<SpriteEditorView>` in `creation.rs`, with `EditorChrome { title: <plain phrase, builder's words>, buttons: WIZARD_BUTTONS.to_vec(), tint: None, full_colour: false }`. Replaces `icon_editor_view`.
  - `fn App::handle_creation_pointer(&mut self, hit: PointerHit, button: PointerButton, phase: PointerPhase)` in `creation.rs`: a `PointerHit::Button` on primary `Down` calls `handle_creation_key(pressed.key())`; otherwise forwards to the open editor's `handle_pointer`; does nothing when the editor is closed.
  - `App::handle_pointer` gains `Mode::CreateCharacter => self.handle_creation_pointer(...)`.
  - In `handle_creation_key`'s editor branch: `Enter` → `PlayerIcon::new(editor.frames(), editor.frame_ms()).expect(...)`, `creation_choice.icon = (!icon.is_blank()).then_some(icon)`, drop the editor, `advance_creation()`; `Esc` → drop the editor; any other key → `editor.handle_key(key)` and stay. Opening (line 751) becomes `SpriteEditor::open(icon.frames().to_vec(), icon.frame_ms())` from `creation_choice.icon.clone().unwrap_or_default()`.
  - gui: `render::sprite_forge::draw_sprite_editor_session` becomes `pub(super)`; `render/mod.rs` `Mode::CreateCharacter` arm calls it with `app.creation_sprite_view()` and `app.zoom`; `sprite_hits` returns `SpriteHits::Editor` for `Mode::CreateCharacter` when `creation_sprite_view()` is `Some`.

- [ ] **Step 1: Write the failing app-core tests** in `tests/creation.rs`, keeping the existing Icon-step tests (they drive the wizard through keys and still describe the right behaviour) and adding:

```rust
#[test]
fn a_forge_key_in_the_wizard_editor_changes_the_drawing_and_keeps_the_screen_open() {
    let mut app = on_the_icon_step("x");
    open_the_editor(&mut app);
    app.handle_key(GameKey::Char('2'));
    app.handle_key(GameKey::Char('='));
    let view = app.creation_sprite_view().expect("still open");
    assert_eq!((view.frames, view.frame_ms), (2, DEFAULT_SPRITE_FRAME_MS + SPRITE_FRAME_MS_STEP));
    assert_eq!(view.buttons, WIZARD_BUTTONS.to_vec());
    assert_eq!(view.tint, None);
}

#[test]
fn enter_keeps_both_frames_and_the_pace() {
    let mut app = on_the_icon_step("x");
    open_the_editor(&mut app);
    app.handle_key(GameKey::Char(' '));      // paint frame 1 at the cursor
    app.handle_key(GameKey::Char('2'));
    app.handle_key(GameKey::Char('='));
    app.handle_key(GameKey::Enter);
    let icon = app.creation_choice().icon.as_ref().expect("a drawing");
    assert_eq!(icon.frames().len(), 2);
    assert_eq!(icon.frame_ms(), DEFAULT_SPRITE_FRAME_MS + SPRITE_FRAME_MS_STEP);
}

#[test]
fn s_in_the_wizard_editor_queues_no_sprite_write() {
    let mut app = on_the_icon_step("x");
    open_the_editor(&mut app);
    app.handle_key(GameKey::Char('s'));
    assert!(app.creation_sprite_view().is_some());
    assert!(app.take_sprite_writes().is_empty());
}

#[test]
fn a_click_on_keep_advances_the_wizard_like_enter() {
    let mut app = on_the_icon_step("x");
    open_the_editor(&mut app);
    app.handle_pointer(PointerHit::Cell(0, 0), PointerButton::Primary, PointerPhase::Down);
    app.handle_pointer(PointerHit::Cell(0, 0), PointerButton::Primary, PointerPhase::Up);
    app.handle_pointer(PointerHit::Button(EditorButton::Keep), PointerButton::Primary, PointerPhase::Down);
    assert!(app.creation_sprite_view().is_none());
    assert_eq!(app.creation_step(), CreationStep::Colour);
    assert!(app.creation_choice().icon.is_some());
}

#[test]
fn a_pointer_on_the_icon_step_with_the_editor_closed_changes_nothing() {
    let mut app = on_the_icon_step("x");
    let before = app.creation_choice().clone();
    app.handle_pointer(PointerHit::Cell(0, 0), PointerButton::Primary, PointerPhase::Down);
    app.handle_pointer(PointerHit::Button(EditorButton::Keep), PointerButton::Primary, PointerPhase::Down);
    assert_eq!(app.creation_choice(), &before);
    assert_eq!(app.creation_step(), CreationStep::Icon);
}
```

`creation_step()` and `creation_choice()` are the accessors the existing tests in this file already use; `take_sprite_writes` and `handle_pointer` are `pub` on `App`.

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-app-core creation`
Expected: compile errors on `creation_sprite_view` and the pointer in `CreateCharacter`.

- [ ] **Step 3: Replace the host** in `creation.rs` per Interfaces; delete `icon_editor.rs`, its `mod` line, its `pub use IconEditorView`, and `tests/icon_editor.rs` with its `mod` line. Add the `CreateCharacter` arm to `handle_pointer`.

- [ ] **Step 4: Write the failing gui tests** in `render/sprite_forge.rs`'s test module: `the_wizard_publishes_editor_hits_only_while_its_editor_is_open` (an `App` on the Icon step: `sprite_hits` is `None`; after opening the editor it is `Some(SpriteHits::Editor(_))`) and `the_wizard_preview_is_drawn_untinted` (draw the session with a wizard view whose frame has one lit cell of a saturated palette colour and assert the recorded preview pixel equals `canvas::palette_color` of that entry, the way `render/sprite_forge.rs:1158`'s test reads the painter).

- [ ] **Step 5: Run them to verify they fail**

Run: `cargo test -p feral-processes-gui sprite_forge`
Expected: `None` where `Some` is expected; the untinted test fails to compile until the view exists.

- [ ] **Step 6: Wire the renderer**: `render/mod.rs` arm, `sprite_hits` arm, `draw_sprite_editor_session` visibility; delete `render/icon_editor.rs` and its `mod`; the `TRANSPARENT`-style constant at `render/mod.rs:180` named for `icon_editor.rs` is renamed or moved to where `canvas.rs` uses it. Update the comments in `gui/src/lib.rs:211-245` and `:921` to say the pointer is read in `Mode::SpriteEditor`, `SpritePicker`, and `CreateCharacter` while its editor is open.

- [ ] **Step 7: Run the gates**

Run: `cargo test --workspace && cargo clippy --workspace --all-targets && cargo fmt --check`
Expected: green. `rg -n 'icon_editor|IconEditor' crates` prints nothing.

- [ ] **Step 8: Commit**

```bash
git add -A crates/app-core/src crates/gui/src
git commit -m "Wizard: the Icon step draws with the sprite editor — 16x16, the sprite palette, two frames and the mouse"
```

(`-A` scoped to the two source trees is what records the deletions; nothing else is running in this worktree.)

---

### Task 4: Help, seams and the index

**Files:**
- Modify: `assets/help/15-your-character.md:47-51`
- Modify: `.claude/skills/seams/references/hud.md:135-168`, `.claude/rules/seams-hud.md:42-44`, `.claude/rules/drawing-seam.md:36-52`
- Modify: `docs/superpowers/INDEX.md:467` (sprite-frames row)
- Memory graph: dissolve `seam:icon-palette-is-fixed-at-fifteen-entries-by-the-save-format`, add the new seam (via the `seams` skill)

**Interfaces:** none; documentation of Tasks 1-3.

- [ ] **Step 1: Rewrite the help paragraph** at `15-your-character.md:47-51`: the sixth row opens an editor to draw your own avatar; it is the size the map draws it; a wide palette; a second picture that alternates with the first, and a speed for the swap; a fill tool and a colour pick; the mouse paints too; Enter keeps, Esc throws it away; the swatch still colours your glyph elsewhere. Plain language, no key list the screen does not show.

- [ ] **Step 2: Invoke the `seams` skill** and follow its order: write the new seam entity `seam:avatar-codec-is-two-hex-digits-per-cell-so-the-palette-can-grow` to the graph with the trap from the spec (63 plus transparent is exactly one base64 character; the refusal in `decode` and `PlayerIcon::new` is what is load-bearing), mark the fifteen-entries seam dissolved with a relation to the new one; then rewrite the two bullets in `hud.md` (the 8x8/16x16 paragraph and the fifteen-entries bullet) and the one-liner in `seams-hud.md`; then rewrite the "player edits `ICON_GRID` (8)" sentence in `drawing-seam.md:48-52` to: the avatar is a sprite drawn at the sprite's size on the sprite palette, animated like one, and the save string is `v3` with `v1` and `v2` folded forward.

- [ ] **Step 3: Note the overturned claim** on `INDEX.md:467`: append to the sprite-frames row's third cell "; said the drawn icon never animates — overturned by `2026-10-18-one-sprite-editor-design`".

- [ ] **Step 4: Verify the docs name real code**

Run: `rg -n 'ICON_GRID|ICON_CELL_PIXELS|fifteen entries|8x8' .claude/rules .claude/skills/seams assets/help docs/superpowers/INDEX.md`
Expected: no hits except historical rows in `INDEX.md`.

- [ ] **Step 5: Commit**

```bash
git add assets/help/15-your-character.md .claude/skills/seams/references/hud.md .claude/rules/seams-hud.md .claude/rules/drawing-seam.md docs/superpowers/INDEX.md
git commit -m "Help and seam notes: the avatar is a sprite, and the codec has room for the palette to grow"
```

---

## Self-review notes

- Spec coverage: codec (T2), what dies (T2), editor module (T1), hosts (T1 Forge, T3 wizard), view and screen (T1, T3), mouse in the wizard (T3), GPU upload (T2), profile and save (unchanged fields; the save→load test the spec asks for is the existing `a_drawn_icon_reaches_the_profile_when_creation_finishes` plus `enter_keeps_both_frames_and_the_pace` in T3 — the builder adds a `Game` save→load assertion on `player_icon()` to the existing engine save round-trip test module if one covers the icon string; `rg -n 'icon' crates/engine/src/save.rs` finds it), help (T4), seams and INDEX (T4). Tests list: all named.
- Interfaces: `SpriteEditor::frames()` returns `Vec<Canvas>` (T1) and `PlayerIcon::new` takes `Vec<Canvas>` (T2); `PlayerIcon::frames()` returns `&[Canvas]` and `SpriteEditor::open` takes `Vec<Canvas>`, so the wizard opens with `.to_vec()` (T3). `SubjectTint` moves to `sprite_editor.rs` in T1 and the gui's `subject_hue` imports it from the crate root re-export.
- Green at every commit: T2 keeps the old editor compiling on purpose; T3 deletes it.
