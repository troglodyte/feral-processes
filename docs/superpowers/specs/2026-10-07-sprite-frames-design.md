# Sprite frames: two-frame asset sprites

**Date:** 2026-10-07
**Source:** TODO #123
**Builds on:** `2026-09-04-dev-sprite-editor-design.md`,
`2026-10-07-sprite-forge-editing-design.md` (both shipped)

## Intent

An asset sprite can have a second frame, and the drawn sprite alternates
between the two at a rate set per sprite. That lets an antenna blink or an
arm move. The Sprite Forge gets a tab for frame 2, which starts as a copy of
frame 1.

Success: in the forge, open `construct`, press `[2]`, change a few pixels,
set the rate and save. Every construct on the map, battle screen and tactical
map then alternates frames, each creature out of step with the others, and
the sprite holds still on frame 1 when animations are turned off.

## Decisions (from brainstorming)

- **Asset sprites only.** The player's drawn icon (`PlayerIcon`, the `v2:`
  codec) does not change. No save-format or engine change.
- **The rate is per sprite**, not a global setting.
- **Sprites animate everywhere `Painter::sprite` draws.**
- **Frames respect `Fx::enabled`**, and each entity gets its own phase.
- **Storage is one sheet PNG plus an optional `.anim.ron`** (rejected
  alternatives: a sibling `.f2.png` file, which could drift from frame 1's
  colour variant or `.off` state; and a rate field on the defs, which
  changes the engine schema and lets the `depot_mk*` defs that share a
  sprite disagree).
- **Forge UX:** the `[1]`/`[2]` keys and Frame 1 / Frame 2 tab buttons
  switch frames. `Tab` keeps its job of switching canvas and palette focus.

## 1. Data: `assets/sprites/`

- A sprite PNG is **16x16** (one frame) or **32x16** (two frames side by
  side, frame 1 on the left). Any other size is skipped with a logged
  warning, and the entity draws its glyph as it would for a missing file.
- **`<key>.anim.ron`** is optional: `(frame_ms: 600)`. One file serves both
  `<key>.png` and `<key>.colour.png`.
  - When the file is missing, or fails to parse (logged warning), the rate
    is the named constant `DEFAULT_SPRITE_FRAME_MS`.
  - An `.anim.ron` beside a 16x16 sprite is ignored.
- The `.png.off` disable mechanism and the `.colour` variant work unchanged.
  The sheet is one file, so both frames always share one variant and one
  enabled state.
- Every shipped 16x16 sprite loads exactly as before.
- `assets/sprites/README.md` documents the sheet layout, `.anim.ron` and its
  default, in the same change.

## 2. Gui: `SpriteTable` and `Painter`

- A `SpriteTable` entry (`paint.rs`) becomes
  `{ texture, full_colour, frames: 1 | 2, frame_ms }`.
  - The texture is the whole sheet, uploaded once with
    `ImageSampler::nearest()` as today.
  - `Painter::sprite` draws the active frame by setting UVs to the left or
    right half, so there is still one texture per name and `"@drawn"` is
    untouched.
- **The frame is chosen by a pure function**, which lives in `paint.rs` or
  beside the fx phase helpers:
  `sprite_frame(now, phase_key, frames, frame_ms) -> usize`.
  - It returns 0 when `frames == 1`.
  - Otherwise it offsets time by `(phase_key % PHASE_KEYS)` steps, using the
    `staffed_bob_offset` phase model, and returns
    `floor(t / frame_ms) mod frames`.
- `Painter` carries `anim_now: f64`. `gui/src/lib.rs` sets it each frame
  from the same clock it hands `Fx::begin_frame`. When `Fx::enabled` is
  false, it sets `anim_now` to 0 and the phase is ignored, so every sprite
  shows frame 1.
- `Painter::sprite` gains a `phase_key: u64` argument.
  - The map (`render/base.rs`), battle (`render/battle.rs`) and tactical
    (`render/tactical.rs`) views pass the entity key their fx effects
    already use.
  - Notifications, floor finishes, the creation preview and the battle
    portrait pass 0.
- `paint.rs` stays the only file that names a graphics library
  (`.claude/rules/drawing-seam.md`). The seam rule's "fifteen operations"
  wording gets a note that `sprite` picks a frame.

## 3. App-core: the Sprite Forge (`app/sprite_forge.rs`)

The forge's editor state holds a list of frames instead of a single canvas.

- **State:** `frames: Vec<Canvas>` (length 1 or 2), `active_frame: usize`,
  `frame_ms: u32`. The shared `CanvasEditor` keeps editing one canvas: the
  forge swaps the active frame in and out of it on a switch, so the player
  icon editor is unaffected.
- **Keys:**
  - `[1]` and `[2]` select a frame. The first `[2]` on a one-frame sprite
    creates frame 2 as a copy of frame 1.
  - `-` and `=` step `frame_ms` by `SPRITE_FRAME_MS_STEP` (100), clamped to
    `SPRITE_FRAME_MS_MIN..=SPRITE_FRAME_MS_MAX` (100..=2000).
  - Each new key must not collide with an existing forge or `CanvasEditor`
    key. The plan checks the key table first.
- **Buttons** (`EditorButton`): Frame 1, Frame 2, Delete frame 2, and
  speed −/+. Delete frame 2 is a button only, with no key, so it can't be
  hit by a stray keypress. It returns to a one-frame sprite on frame 1.
- **Undo:** each snapshot holds the whole frame set plus `active_frame`, so
  undoing a delete or the copy that created frame 2 works.
- **The save variant** (`saves_as_full_colour`) is decided across all frames.
  The existing pin-after-first-save behaviour still applies.
- **Load:** a 32x16 file splits into two frames, and `frame_ms` comes from
  `.anim.ron` or the default.
- **Save:** `SpriteOp::Save` carries `frames: Vec<Canvas>` and `frame_ms`.
  The gui's `sprites::drain_writes` writes a 16x16 or 32x16 PNG
  (`canvas_to_png` generalised to a frame list).
  - With two frames, it writes `<key>.anim.ron`.
  - With one frame, it removes any existing `<key>.anim.ron`, so deleting
    frame 2 and saving gives a clean 16x16 sprite.
  - The hot reload that already makes a save visible on the map without a
    restart re-reads the frame count and rate.
- **Preview:** the forge view (`render/sprite_forge.rs`) shows a small
  preview that plays the animation at `frame_ms`, and the active frame tab
  is highlighted.

## 4. Testing

- **App-core (`tests/`):**
  - `[2]` copies frame 1 exactly once.
  - Edits on frame 2 leave frame 1 alone.
  - Switching frames keeps each frame's pixels.
  - Delete frame 2 returns to one frame.
  - Undo restores a deleted frame 2.
  - The speed clamps at both ends.
  - The save op carries both frames and `frame_ms`.
  - The full-colour decision sees a colour index that exists only on
    frame 2.
- **Gui:**
  - A two-frame `canvas_to_png` → load round trip.
  - The 32x16 split.
  - `sprite_frame` with 1 frame, 2 frames, different `phase_key`s, and
    animations off.
  - A bad size (24x16) is rejected.
  - `.anim.ron` parsing, its fallback when missing or malformed, and its
    removal when a one-frame sprite is saved.
  - A UV assertion that frame 2 draws the right half.
- **Assets:** a test that every shipped sprite still loads, with
  `frames == 1` for each.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets` are
  the gate. `balance_sim` is not affected.

## Out of scope

- More than two frames. The sheet layout allows 3+ later; the loader rejects
  them now.
- Animating the player's drawn icon.
- Per-frame timing; one rate covers both frames.

## Release

Patch bump. No save-format change (`SAVE_FORMAT_VERSION` untouched).
`CHANGELOG.md` gets a `## X.Y.Z` entry when this lands.
