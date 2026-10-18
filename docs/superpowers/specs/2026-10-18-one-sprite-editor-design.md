# One sprite editor: the wizard draws with the Forge

**Date:** 2026-10-18
**Status:** draft for approval; see `INDEX.md` for whether it shipped.

## Intent

The game has two pixel editors.
The character wizard's Icon step opens an 8x8 editor on a fifteen-colour
palette, and the dev Sprite Forge (behind `FERAL_DEV_SPRITES`) opens a 16x16
editor on the 63-colour `SPRITE_PALETTE` with two animation frames, a frame
speed, a fill tool, a colour picker and a mouse.
Both are built on the same `CanvasEditor` mechanics, and the wizard's is the
poorer one in every respect a player would notice.

The user's words (2026-10-17): "have the create character sprite creator be
the same one as the dev one to create other sprites, we should only have
one, and it should be the more advanced one."

After this change there is one editor.
The player draws their avatar at the sprite's own 16x16 on the full sprite
palette, gives it a second frame and a speed if they like, and sees it blink
or wave on the map and the battle map like any other sprite.
The Forge keeps everything that is only a dev tool's business.

## Goals

- The wizard's Icon step opens the Forge's editor: 16x16, `SPRITE_PALETTE`,
  one or two frames, frame speed, fill, undo, colour pick, and the mouse.
- The drawn avatar animates everywhere the drawn icon is drawn today, at the
  speed the player set, and holds on frame 1 when animations are turned off,
  exactly as an asset sprite does.
- A player's existing 8x8 drawing survives: it opens in the new editor at
  16x16 in the nearest sprite-palette colours and can be edited further.
- The drawing stays an encoded string in `profile.ron` and in the save.
  No PNG is written anywhere for the avatar.
- One editor module holds the mechanics; each host is a few lines keyed on
  `Mode`.

## Non-goals

- Changing anything about asset sprites, the picker, `.off`, full-colour
  (`.colour.png`) saves, or how the Forge writes PNGs.
- Making the Forge itself available to players.
  `FERAL_DEV_SPRITES` still gates the picker and the PNG path.
- More than two frames, or a per-frame speed.
- A `SAVE_FORMAT_VERSION` bump.
  The save's `icon: Option<String>` field is unchanged; only the string's
  content is new, and old strings still decode.

## Anti-goals

- **Losing a drawing silently.** The whole reason the old palette was pinned
  at fifteen (`seam:icon-palette-is-fixed-at-fifteen-entries-by-the-save-format`)
  was that a one-hex-digit codec would turn a sixteenth colour into
  transparent without a failing test.
  The new codec must leave headroom and refuse rather than wrap.
- **A `SpriteHost` switch inside the editor.** The user chose the direct
  shape: the editor reports a key as handled or not and knows nothing about
  who opened it.
- **Dev chrome in the wizard.** No Save button, no subject hue, no picker.
- **Putting the player's hue back on the drawing.** The drawn icon is the one
  sprite drawn untinted (`.claude/rules/drawing-seam.md`); the wizard's
  preview and the map keep that rule.

## Decisions the user made

1. **Grid and palette:** 16x16 on the Forge's `SPRITE_PALETTE` (63 colours
   plus transparent). This needs a new codec version for `PlayerIcon`.
2. **Animation:** the avatar gets the Forge's two frames and its frame speed.
3. **Storage:** an encoded string in `profile.ron` and the save, as today.
   No PNG in `assets/` or the data directory.
4. **Shape:** direct. `SpriteEditor` keeps the drawing mechanics and the
   shared key table and reports each key as handled or unhandled; each host
   is a few lines in `App` keyed on `Mode`. Forge: `Esc` returns to the
   picker, `s` saves. Wizard: `Enter` keeps, `Esc` discards. The view carries
   what differs between hosts as plain fields.
5. **Old 8x8 drawings:** scale each cell to a 2x2 block and map each of the
   fifteen old colours to its nearest `SPRITE_PALETTE` entry through the
   existing `quantise`.

## Design

### The codec: `v3`

`PlayerIcon` (`crates/engine/src/icon.rs`) becomes one or two 16x16 `Canvas`
frames and a frame speed.

- `encode` writes `v3:<frame_ms>:<frame>[:<frame>]`.
  `frame_ms` is decimal.
  Each frame is 256 cells row-major, **two lowercase hex digits per cell**,
  so 512 characters.
  Cell value `0` is transparent and `1..=63` indexes `SPRITE_PALETTE`
  (`sprite_rgba` already maps exactly this way).
- `decode` returns `None` for anything malformed: a wrong prefix, zero or
  three or more frames, a frame not exactly 512 hex digits, a cell value
  above `SPRITE_PALETTE.len()`, a `frame_ms` that is not a positive integer.
  It never clamps or masks.
  A positive `frame_ms` of any size is accepted, the same rule
  `read_frame_ms` applies to an asset's `.anim.ron`; the Forge's step
  bounds are a UI concern and stay in app-core.
- `decode` folds `v1` and `v2` forward, never drops them.
  `v2` (8x8, one hex digit, old palette): each cell becomes a 2x2 block, and
  each old palette index maps through a private table to its nearest
  `SPRITE_PALETTE` index (old `0` stays `0`; old `n` becomes
  `quantise(ICON_PALETTE[n-1])`, which already returns index-plus-one).
  `v1` (16x16, one hex digit, old palette): the colour remap alone.
  Both yield one frame at `DEFAULT_SPRITE_FRAME_MS`.
  The remap table is built once from the old fifteen colours by calling
  `quantise((r, g, b, 255))` on each; it is a call, not a copied formula.
- `encode` always writes `v3`.
  A profile or save touched by the new binary is on the new format from
  then on; an old binary reading a `v3` string gets `None` and the default
  avatar, which is a degraded picture, not a failed load.

**Why two hex digits and not one base64 character.** 63 colours plus
transparent is 64 values, which fits one base64 character *exactly*.
That is the same shape as the trap this change dissolves: the day a 64th
colour is added, an exactly-fitting codec has nowhere to put it.
Two hex digits give a ceiling of 255, so `SPRITE_PALETTE` can grow without
a `v4`, and `PlayerIcon::set` refuses an index above the palette length
rather than wrapping.
The string is about 1.1 KB for two frames, which is nothing against the
save.

**What dies.** `ICON_GRID` and `ICON_CELL_PIXELS` are deleted along with
every use (`pixel_rgba`, the 2x2 block upload, the ratio paragraphs in the
seam docs).
`ICON_PALETTE` stops being `pub`; it becomes a private legacy table used
only by the `v1`/`v2` decoders and the remap.
`ICON_SIZE` (16) stays: it is the sprite format, not the icon's.
`PlayerIcon::set`'s bound becomes `SPRITE_PALETTE.len()`.
The pinned test
`the_palette_has_room_for_exactly_fifteen_colours_because_one_hex_digit_encodes_them`
is replaced by one asserting that every index up to `SPRITE_PALETTE.len()`
round-trips and that one past it is refused by `set` and by `decode`.

### The editor module

`SpriteEditor` moves out of `crates/app-core/src/app/sprite_forge.rs` into
its own module, `crates/app-core/src/app/sprite_editor.rs`, taking with it
the frame list, the active frame, `frame_ms`, the undo history, the
`MAX_SPRITE_FRAMES` / `DEFAULT_SPRITE_FRAME_MS` / `SPRITE_FRAME_MS_*`
constants, and the shared key table: `CanvasEditor`'s keys (arrows, `Space`,
`Backspace`, `Tab`, `x`) plus `g` brush, `f` tool, `1`/`2` frame, `D` delete
frame, `-`/`=` speed, `u` undo, `i` pick colour.

`SpriteEditor::handle_key(key) -> SpriteKey::{Handled, Unhandled}`.
The editor never sees `Enter`, `Esc` or `s`; those return `Unhandled` and
the host decides.

The Forge's own things stay in `sprite_forge.rs`: the picker, subjects and
their tints, `full_colour`, `SpriteWrite`/`SpriteOp::Save`, the pending
writes queue, `.off`.
`handle_sprite_editor_key` shrinks to `Esc` → picker, `s` → queue a Save,
else delegate.

`crates/app-core/src/app/icon_editor.rs` is deleted.
The wizard's host lives in `creation.rs`, where
`handle_creation_icon_key` already intercepts the editor: an
`Option<SpriteEditor>` plus the `opened_with: PlayerIcon` it needs for
`Keep`/`Discard`, with the same meaning as today. `Enter` folds the editor's
frames and speed into a `PlayerIcon`, stores it as `opened_with` and
closes; `Esc` closes and leaves `opened_with` alone; everything else is
delegated and the screen stays open whether or not the editor handled it.
It is still not a `Mode`, for `creation.rs`'s existing reason.

### The view and the screen

`SpriteEditorView` already carries the canvas, palette, tool, frames,
active frame, `frame_ms` and per-frame cells for the animated preview.
It gains the fields that differ by host, set by the host when it builds
the view:

- `title: String`. Forge: the subject's label as today. Wizard: a plain
  phrase such as "Your avatar"; the builder picks the words, in the game's
  own voice, no code names.
- `buttons: Vec<EditorButton>`. Forge: today's eleven. Wizard: the same
  minus `Save`, plus a `Keep` button whose key is `Enter`; the `Esc`
  button's label is "Back" in the Forge and "Discard" in the wizard. How the
  label varies is the builder's call (a second variant or a label on the
  view both work); the row must stay data the renderer lays out, so the
  mouse hits it the same way in both hosts.
- `tint: Option<...>`. Forge: the subject's `SubjectTint` as today. Wizard:
  `None`, and the preview draws untinted. The existing untinted assertions
  (`the_look_preview_prefers_the_drawn_icon` in `render/creation.rs` and
  the one near the body draw in `render/tactical.rs`) keep the rule on the
  map; a new one holds it on the editor's preview.

One draw function, `render/sprite_forge.rs::draw_sprite_editor`, draws
both. The arm in `render/mod.rs::draw` for `Mode::CreateCharacter` that
today calls `icon_editor::draw_icon_editor` calls it instead, with the
wizard's view. `crates/gui/src/render/icon_editor.rs` is deleted.

`subject` and `full_colour` on the view are Forge-only; the wizard sets
`full_colour: false` (its canvas can only hold palette indices) and the
renderer draws no `.colour` indicator when there is nothing to indicate.

### The mouse in the wizard

Today `handle_sprite_pointer` (`crates/gui/src/lib.rs`) and
`App::handle_pointer` route only in `Mode::SpriteEditor` and
`Mode::SpritePicker`. Both gain `Mode::CreateCharacter` while the wizard's
editor is open, and `render::sprite_forge::sprite_hits` publishes the
editor's rects in that case. When the editor is not open in the wizard the
pointer is dropped as today. `needs_status_banner` already has a
`CreateCharacter` row; verify nothing new is owed.

### Putting the drawing on the GPU

`Sprites::sync_drawn_icon` (`crates/gui/src/sprites.rs`) uploads a
`16n x 16` texture, frames side by side, exactly the layout an asset
sprite's PNG has, built through `sprite_rgba(index)` per cell, and inserts
`DRAWN_ICON_KEY` with the icon's frame count and `frame_ms` instead of the
hard-coded `1, DEFAULT_SPRITE_FRAME_MS`. Nearest sampling stays. The
drawn-icon-changed check compares the whole `PlayerIcon`, frames and speed
included.

Because the table entry now says how many frames the drawing has, every
`Painter::sprite` call that draws the avatar animates it with no further
change, and the animations-off setting holds it on frame 1 like any sprite.

### Profile and save

`Profile::player_icon: Option<String>` and `save::...icon: Option<String>`
are unchanged as fields. `lifecycle.rs` still writes
`choice.icon.encode()` only when the player drew. A round-trip test
(encode, write RON, read RON, decode) covers a two-frame icon, and a
save-then-load test asserts the loaded icon equals the saved one, frames
and speed included (a RON round trip alone cannot catch a skipped field).

### Help

`assets/help/15-your-character.md` describes the Icon step and its editor.
Update it: the size, that there are more colours, the second frame and the
speed, the fill and pick tools, and that the mouse works. Plain language,
no key names that the screen does not itself show.

## Seams this change moves

Three places per seam, in the order the `seams` skill documents: graph,
skill reference, rules file.

- `seam:icon-palette-is-fixed-at-fifteen-entries-by-the-save-format` is
  **dissolved** and replaced by a new seam, suggested slug
  `avatar-codec-is-two-hex-digits-per-cell-so-the-palette-can-grow`. The
  trap it records: 63 plus transparent is exactly one base64 character,
  and a codec that fits exactly is the trap `v2` had; the refusal in
  `decode` and in `set` is what is load-bearing, not the digit count.
  Bullets in `.claude/skills/seams/references/hud.md` and
  `.claude/rules/seams-hud.md` are rewritten.
- The "player edits 8x8 and the sprite is 16x16" paragraphs in
  `.claude/rules/drawing-seam.md` and `hud.md` are rewritten: the avatar
  is a sprite, drawn at the sprite's size on the sprite's palette, and the
  save string is `v3` with `v1` and `v2` folded forward.
- The untinted-drawn-icon rule is unchanged and now also covers the
  editor's preview in the wizard.
- `docs/superpowers/archive/specs/2026-10-07-sprite-frames-design.md` said
  the drawn icon never animates. `INDEX.md`'s row for it gets a note that
  this spec overturned that.

## Tests

Delete `crates/app-core/src/tests/icon_editor.rs`. Rework the icon-step
tests in `crates/app-core/src/tests/creation.rs` (the `IconEditor`-shaped
ones around lines 1364-1825), the `PlayerIcon` tests in
`crates/engine/src/icon.rs`, and the drawn-icon upload tests in
`crates/gui/src/sprites.rs` (around lines 715-897). New tests, at least:

- engine: `v3` round trip for one and two frames; every malformed shape
  listed above refused; `v2` folds to a 2x2 scaled, colour-remapped single
  frame; `v1` folds with the remap alone; index `SPRITE_PALETTE.len()`
  round-trips and one above is refused by `set` and `decode`.
- app-core: the wizard's `Enter` keeps frames and speed; `Esc` restores the
  opened-with drawing; a Forge key (`2`, `=`) in the wizard changes the
  drawing and keeps the screen open; `s` in the wizard is a no-op and
  queues no sprite write; the mouse in `Mode::CreateCharacter` paints only
  while the editor is open.
- gui: the upload is `16n x 16` with `n` frames and the table entry carries
  the icon's `frame_ms`; the wizard's preview is untinted; the pointer
  layout is published in `Mode::CreateCharacter` only while the editor is
  open.

`cargo test --workspace` is the gate; `balance_sim` is unaffected.

## Left to the builder

- The exact wizard title and button labels.
- Whether the `Esc` button's label is a second `EditorButton` variant or a
  label on the view.
- Where in `creation.rs` the host's few lines sit, as long as
  `handle_creation_icon_key` stays the one door.
- The order of commits; the engine codec with its tests is the natural
  first green step, the module move the second.

## Process

Three crates and a stored-string format change: the spec-and-plan
pipeline. Worktree `.claude/worktrees/one-sprite-editor`, branch
`worktree-one-sprite-editor`, at `origin/main` 0.19.3.
