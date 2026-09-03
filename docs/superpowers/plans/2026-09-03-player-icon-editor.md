# Player icon editor implementation plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Let the player draw their own 16x16 map avatar in the
character-creation wizard, from a fixed palette, and have it persist across
runs.

**Architecture:** A `PlayerIcon` (256 palette indices) and its string codec
live in the engine and ride the existing `PlayerIdentity` -> `PlayerLook`
path to the renderer. The editor is a sub-state of the wizard's Icon step in
app-core, not a `Mode`. The gui uploads the icon as a runtime egui texture
and draws it as the first rung of a three-step fallback at the player's
tile.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine), bevy 0.19 + bevy_egui 0.41
(gui), serde/RON.

**Spec:** `docs/superpowers/specs/2026-09-03-player-icon-editor-design.md` —
read it first; this plan argues from it and does not restate its reasoning.

## Global Constraints

- **This plan does not contain the implementation.** Per `CLAUDE.md`'s
  process-weight rule, code blocks here are reserved for the genuinely
  non-obvious — the encoding, the bevy/egui texture mechanics, one
  expression. Everything else is a file, an interface and the intent of a
  test. Write the code; don't re-emit this.
- **TDD, failing test first, at every task.** Commit per green step.
- **`cargo fmt` and `cargo clippy --workspace` after every change.** Fix
  warnings, never silence them.
- **`cargo test --workspace` is the final gate** (~4123 tests). Per-task
  gates are the targeted runs named in each task.
- **Sprites are 16x16, RGBA, nearest-sampled.** Non-negotiable — see
  `assets/sprites/README.md` and `crates/gui/src/sprites.rs`.
- **No mouse.** `GameKey` is the whole input vocabulary.
- **No `SAVE_FORMAT_VERSION` bump.** Every new persisted field is additive
  and `#[serde(default)]`.
- **Named constants, not magic numbers.** Gameplay/tuning values go in
  `crates/engine/src/tuning.rs`; layout values stay beside the render code
  that spends them, as the other render files do.
- **Do not `git add -A`** — another session may hold a worktree gitlink
  under `.claude/worktrees/`. Stage explicit paths.
- **Do not push.** Landing is the user's call.

---

### Task 1: `PlayerIcon` and its codec

**Files:**
- Create: `crates/engine/src/icon.rs` (tests inline, `#[cfg(test)] mod tests`)
- Modify: `crates/engine/src/lib.rs` (declare the module, re-export
  `PlayerIcon` beside the other engine types)

**Interfaces:**
- Consumes: nothing.
- Produces:
  - `pub const ICON_PALETTE: [(u8, u8, u8); 15]` — opaque RGB, palette
    order. Index 0 is *not* in it: 0 means transparent.
  - `pub struct PlayerIcon` — one private `[u8; 256]`, derives `Clone`,
    `PartialEq`, `Eq`, `Debug`, `Default` (default is all-transparent).
  - `pub fn PlayerIcon::get(&self, x: usize, y: usize) -> u8`
  - `pub fn PlayerIcon::set(&mut self, x: usize, y: usize, index: u8)` —
    ignores an out-of-range index rather than panicking; the editor is the
    only caller and cannot produce one, and a save cannot reach this path
    (`decode` refuses first).
  - `pub fn PlayerIcon::clear(&mut self)`
  - `pub fn PlayerIcon::is_blank(&self) -> bool`
  - `pub fn PlayerIcon::rgba(&self, x: usize, y: usize) -> (u8, u8, u8, u8)`
    — the one place index 0 becomes a transparent pixel, so the gui and any
    test agree about it.
  - `pub fn PlayerIcon::encode(&self) -> String`
  - `pub fn PlayerIcon::decode(s: &str) -> Option<PlayerIcon>`

**The encoding** (this is the non-obvious part, so it is spelled out):

```text
"v1:" followed by exactly 256 lowercase hex digits, row-major from the
top-left, one per pixel. '0' is transparent; '1'..='f' index ICON_PALETTE
at value-1. Total length 259.
```

`decode` is strict and total: wrong prefix, any length but 259, or a
non-hex digit all return `None`. There is no partial recovery — the caller
falls back to the glyph, which is already correct and already tested.

**The palette**, in order, index 1..=15 — five steps of value then ten
hues. The value ramp is the half that does the work at this size;
`assets/sprites/README.md` explains why. Brown is in the list on purpose:
without it a 16x16 figure has no skin, leather or wood.

```text
 1 1c1c1c   2 4a4a4a   3 7d7d7d   4 b4b4b4   5 f2f2f2
 6 c0392b   7 d97b2b   8 e8c547   9 4f9d4f  10 3fa9a0
11 4bb3d9  12 3b6fd4  13 7a55c4  14 c04f9e  15 8a5a3c
```

- [ ] **Step 1: Write the failing tests.** Their intent, one test each:
  a drawn icon round-trips through `encode`/`decode` unchanged; a default
  icon encodes to `"v1:"` plus 256 zeros; each malformed form decodes to
  `None` (bad prefix, one char short, one char long, a `'g'` in the
  middle); `ICON_PALETTE.len() == 15`, with the message saying *why* — a
  sixteenth colour is unencodable and would be unreachable in a way nothing
  else reports; `rgba` returns alpha 0 for index 0 and alpha 255 otherwise.
- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine icon`
- [ ] **Step 3: Implement `icon.rs` and declare it in `lib.rs`.**
- [ ] **Step 4: Run them and watch them pass.** Same command. Then `cargo fmt` and `cargo clippy -p feral-processes-engine`.
- [ ] **Step 5: Commit.** `git add crates/engine/src/icon.rs crates/engine/src/lib.rs`

---

### Task 2: Persistence — the component, the save and the profile

**Files:**
- Modify: `crates/engine/src/components.rs` (`PlayerIdentity`, ~line 116)
- Modify: `crates/engine/src/save.rs` (`PlayerSave`, ~line 159 where
  `sprite` and `default_player_sprite` already are; and both the write and
  the read of the player's identity)
- Modify: `crates/engine/src/achievements.rs` (`Profile`, ~line 236 beside
  `seen_notifications`)
- Test: `crates/engine/src/tests/` — follow the existing module layout
  there; fixtures live in `crates/engine/src/tests/support.rs`, look there
  before writing a new one.

**Interfaces:**
- Consumes: `PlayerIcon`, `PlayerIcon::encode`, `PlayerIcon::decode` (Task 1).
- Produces:
  - `components::PlayerIdentity::icon: Option<PlayerIcon>` — the live value.
  - `save::PlayerSave::icon: Option<String>`, `#[serde(default)]` — what
    *this character* looks like.
  - `achievements::Profile::player_icon: Option<String>`,
    `#[serde(default)]` — the last icon drawn, cross-run.

Both persisted fields are the **encoded string**, not the struct. On
`Profile`, that is not a style choice: `Profile::load` discards the whole
profile when it cannot parse, so a form this build cannot read must be
inert rather than cost the player every achievement they have earned — the
reason already written out on `seen_notifications`. Decode failure yields
`None` and changes nothing else.

- [ ] **Step 1: Write the failing tests.** Their intent:
  - A profile file whose `player_icon` is garbage still loads its
    achievements. **Assert on the achievements**, not on the icon — the
    icon being `None` is also what an absent field gives, so a test that
    only checks the icon passes with the feature deleted.
  - A save round-trip through `Game::save` then `Game::load` preserves the
    player's icon. **Through the real files, not a RON round-trip** — a
    field that never reaches disk leaves a RON round-trip green (see
    `docs`/memory: "RON round-trip can't catch a skipped field").
  - A save written before this feature (no `icon` key) loads with `None`
    and no warning.
- [ ] **Step 2: Run them and watch them fail.** `cargo test -p feral-processes-engine icon`
- [ ] **Step 3: Add the three fields and wire save/load.**
- [ ] **Step 4: Run them and watch them pass**, then `cargo test -p feral-processes-engine save` to confirm nothing else in the save moved. `cargo fmt`, `cargo clippy -p feral-processes-engine`.
- [ ] **Step 5: Commit.**

---

### Task 3: The icon reaches the view and the wizard's choice

**Files:**
- Modify: `crates/engine/src/views.rs` (`PlayerLook`, ~line 704)
- Modify: `crates/engine/src/game/inspection.rs` (~line 1017, where
  `PlayerLook` is built)
- Modify: `crates/engine/src/game/creation.rs` (`CharacterChoice`, and
  `apply_creation_identity`, which `apply_character_choice` already calls)
- Test: alongside the existing creation tests in
  `crates/engine/src/tests/creation.rs`

**Interfaces:**
- Consumes: Task 2's `PlayerIdentity::icon`.
- Produces:
  - `views::PlayerLook::icon: Option<PlayerIcon>` — populated from
    `PlayerIdentity`, `Some` only for the player, like the rest of
    `PlayerLook`.
  - `CharacterChoice::icon: Option<PlayerIcon>` — `None` in
    `CharacterChoice::default()`, which is what roughly 1,600 `Game::new`
    call sites construct and must keep producing today's player.

- [ ] **Step 1: Write the failing tests.** Their intent: a player created
  from a choice carrying an icon exposes it on `EntityView::look`; the
  default choice exposes `None`; the icon survives `apply_character_choice`
  onto the spawned player.
- [ ] **Step 2: Run and watch fail.**
- [ ] **Step 3: Add the field to `PlayerLook`, populate it, and carry it through `CharacterChoice`.**
- [ ] **Step 4: Run and watch pass**, then `cargo test -p feral-processes-engine creation`. `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 4: `GameKey::Tab`

**Files:**
- Modify: `crates/app-core/src/lib.rs` (`GameKey`, ~line 636)
- Modify: `crates/gui/src/lib.rs` (`map_special_key`, ~line 38; and the
  `untouched` list in the modifier test at ~line 1072)

**Interfaces:**
- Consumes: nothing.
- Produces: `GameKey::Tab`.

`map_special_key` ends in `_ => None`, so this is one variant and one
`KeyCode::Tab` arm, and every existing key table ignores it. **Do not add
it to `REPEATING_KEYS`** — focus is a toggle between two panels and holding
it would flicker.

- [ ] **Step 1: Write the failing tests.** Their intent: `KeyCode::Tab`
  maps to `GameKey::Tab`; a held modifier leaves `GameKey::Tab` unchanged
  (add it to the existing `untouched` list, which is where that property is
  already asserted); `Tab` is inert on `Mode::Playing` — it moves nothing
  and spends no tick.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-gui` and `cargo test -p feral-processes-app-core`
- [ ] **Step 3: Add the variant and the mapping.**
- [ ] **Step 4: Run and watch pass.** `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 5: The `IconEditor` — state and keys

**Files:**
- Create: `crates/app-core/src/app/icon_editor.rs`
- Modify: `crates/app-core/src/app.rs` or the module list wherever
  `app/creation.rs` is declared — follow what is there
- Modify: `crates/app-core/src/lib.rs` (the view type the gui draws from)
- Create: `crates/app-core/src/tests/icon_editor.rs`, declared beside the
  other test modules

**Interfaces:**
- Consumes: `PlayerIcon` and its methods (Task 1), `GameKey::Tab` (Task 4).
- Produces:
  - `pub(crate) struct IconEditor` — the working `PlayerIcon`, the icon it
    opened with (for `Esc`), a `(u8, u8)` cursor, the selected colour index
    `1..=15`, a `Focus`, and a bounded undo stack.
  - `pub enum IconFocus { Canvas, Palette }` (public — the gui draws which
    panel has focus).
  - `pub struct IconEditorView { pub pixels: [u8; 256], pub cursor: (u8, u8), pub selected: u8, pub focus: IconFocus }`
    and `pub fn App::icon_editor_view(&self) -> Option<IconEditorView>`.
  - `IconEditor::handle_key(&mut self, key: GameKey) -> IconEditorOutcome`,
    where the outcome is `Open`, `Keep(PlayerIcon)` or `Discard` — the two
    endings the wizard has to tell apart.
  - `pub const ICON_UNDO_DEPTH: usize = 32;` — beside the editor, not in
    `tuning.rs`: it is an editor affordance, not a difficulty knob.

**The key table** (from the spec; this is the contract the tests assert):

| Key | Does |
|---|---|
| `Tab` | Move focus between canvas and palette |
| Arrows | Move the cursor, or move along the palette — whichever has focus |
| `Space` (`GameKey::Char(' ')`) | Paint the cursor cell with the selected colour |
| `Backspace` | Erase the cursor cell (index 0) |
| `u` | Undo |
| `x` | Clear the canvas |
| `Enter` | Keep the drawing |
| `Esc` | Discard changes |

The cursor does not wrap; the palette selection does not wrap. Painting the
colour a cell already holds pushes no undo entry — otherwise holding
`Space` fills the stack with nothing and undo stops reaching real work.

- [ ] **Step 1: Write the failing tests.** One per row of that table, plus:
  - **Arrows act on the focused panel alone.** This is the test that would
    catch arrows painting while the palette has focus — the whole reason
    `Tab` exists.
  - Undo restores the previous canvas, and the stack is bounded at
    `ICON_UNDO_DEPTH` (drive more than 32 edits, undo 33 times, assert the
    oldest is not recoverable and nothing panics).
  - `Esc` yields `Discard` and the caller's icon is untouched; `Enter`
    yields `Keep` with what was drawn.
  - Repainting a cell with the colour it already holds adds no undo entry.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-app-core icon_editor`
- [ ] **Step 3: Implement the editor.**
- [ ] **Step 4: Run and watch pass.** `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 6: The wizard's sixth Icon row, and the profile seed

**Files:**
- Modify: `crates/app-core/src/lib.rs` (`CreationRow`, ~line 1061)
- Modify: `crates/app-core/src/app/creation.rs` (the Icon step's rows, its
  key table, and `enter_creation_step`)
- Modify: `crates/app-core/src/app/lifecycle.rs` (seed from the profile
  when the wizard opens; write the profile when creation finishes, through
  the existing `flush_profile_writes` path — do not write `profile.ron`
  by hand)
- Test: `crates/app-core/src/tests/creation.rs`

**Interfaces:**
- Consumes: `IconEditor`, `IconEditorOutcome` (Task 5);
  `Profile::player_icon` (Task 2); `CharacterChoice::icon` (Task 3).
- Produces:
  - `CreationRow::DrawnIcon { drawn: bool }` — the sixth row. A distinct
    variant, because `CreationRow::Icon` carries `(glyph, sprite)` and a
    drawing has neither. `drawn` is what lets the row read "Draw your
    own…" or "Your drawing" without the renderer deciding.
  - `App::icon_editor_view` is how the gui knows the editor is open.

Rules the tests pin down:
- Taking the drawn row opens the editor. Leaving it with `Enter` sets
  `CharacterChoice::icon`; with `Esc`, leaves it as it was.
- **Taking any of the five preset rows clears `CharacterChoice::icon`.**
  The two choices cannot both be live, and the drawn icon wins at the draw
  site — so a preset that did not clear it would look like the preset row
  doing nothing.
- The wizard **seeds** the editor from `Profile::player_icon` when the Icon
  step is entered, once — not in `App::creation_rows`, which is rebuilt
  every frame. `enter_creation_step` already carries this exact rule for
  the Points step's roll; follow it.
- Finishing creation writes the drawn icon to the profile.
- **No `Decided` flag.** `[r]` rerolls the kit and only the kit; there is
  nothing to protect the drawing from.
- **The Colour step gains one line of help text.** A player who has drawn
  an icon no longer sees their swatch on the map tile — it still governs
  the glyph on every other surface, and the step says so rather than
  quietly deciding nothing. Author it wherever that step's help line
  already lives (the row builder in `app/creation.rs`, or the screen's
  header in `crates/gui/src/render/creation.rs`) — follow what is there,
  and if the line is the engine's, it must be, since a read-only screen's
  rows are owned by app-core and merely drawn by gui.

- [ ] **Step 1: Write the failing tests.** Their intent: the Icon step
  offers six rows; taking the sixth opens the editor; `Enter` lands the
  drawing on the choice and `Esc` does not; taking a preset clears it;
  entering the step seeds from a profile that has an icon; finishing
  creation leaves that icon in the profile; a profile with no icon opens on
  a blank canvas; the Colour step's help line says what the swatch still
  governs when an icon is drawn.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-app-core creation`
- [ ] **Step 3: Implement the row, the routing and the seed.**
- [ ] **Step 4: Run and watch pass**, then the whole app-core suite. `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 7: The editor screen

**Files:**
- Create: `crates/gui/src/render/icon_editor.rs`
- Modify: `crates/gui/src/render/mod.rs` (declare it; dispatch to it when
  `App::icon_editor_view` is `Some`, ahead of the wizard's own screen)

**Interfaces:**
- Consumes: `IconEditorView`, `IconFocus` (Task 5); `ICON_PALETTE` (Task 1).
- Produces: nothing other tasks read.

**The canvas is drawn as rectangles, not as a texture.** A 16x16 grid needs
per-cell rects anyway for the grid lines and the cursor, and drawing it
this way means no texture is uploaded while the player paints — which is
what stops a texture being minted per keystroke. Transparent cells draw the
screen's own background, not black, or the player cannot tell a hole from a
dark pixel.

Draw: a header, the canvas, the palette strip with the selection marked,
which panel has focus (the two panels' borders are the obvious channel),
and a help footer naming the eight keys. Everything goes through `Painter`
— `crates/gui/src/paint.rs` is the only file allowed to name a graphics
library.

- [ ] **Step 1: Write the failing test.** Its intent: the screen is
  drawable at **1280x720 with no scroll** — the canvas, the palette, the
  focus marks and the whole footer all inside the window. This repo has
  shipped three silent overflows past a green suite; the census is the only
  thing that catches them. Use the layout-test pattern the other render
  tests already use.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-gui icon`
- [ ] **Step 3: Implement the screen.**
- [ ] **Step 4: Run and watch pass.** `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 8: The runtime texture and the player's tile

**Files:**
- Modify: `crates/gui/src/sprites.rs` (build and register the texture)
- Modify: `crates/gui/src/render/base.rs` (~line 1356, the player tile's
  sprite lookup and its `color`)
- Modify: `crates/gui/src/render/creation.rs` (~line 346, the Icon step's
  preview cell)
- Test: `crates/gui/tests/sprites.rs` and the paint-level tests in
  `crates/gui/src/paint.rs`

**Interfaces:**
- Consumes: `PlayerLook::icon` (Task 3), `PlayerIcon::rgba` (Task 1).
- Produces:
  - `Sprites::sync_drawn_icon(&mut self, icon: Option<&PlayerIcon>, images: &mut Assets<Image>, textures: &mut EguiUserTextures)`
  - `pub const DRAWN_ICON_KEY: &str = "@drawn";` — the `SpriteTable` key the
    icon is registered under. The `@` is what keeps it unreachable from a
    filename and from any future `sprite:` field on a species.

**Building the texture** (bevy/egui mechanics, and the one place this is
worth spelling out — verify against bevy 0.19 / bevy_egui 0.41 as you go):

```rust
// 16*16*4 RGBA bytes from PlayerIcon::rgba, then:
let extent = Extent3d { width: 16, height: 16, depth_or_array_layers: 1 };
let mut image = Image::new(extent, TextureDimension::D2, bytes,
                           TextureFormat::Rgba8UnormSrgb, RenderAssetUsages::RENDER_WORLD);
image.sampler = ImageSampler::nearest();   // NOT optional — see below
let handle = images.add(image);
let id = textures.add_image(EguiTextureHandle::Strong(handle));
```

`ImageSampler::nearest()` is the whole reason pixel art stays crisp:
bevy_egui binds the image's *own* sampler and bevy's default is linear.
`sprites.rs` already documents this for the disk path; the runtime path
needs it just as much.

**Only upload when the value changed.** Keep the last `PlayerIcon`
uploaded and compare by value (`PartialEq` on 256 bytes — cheaper than
anything cleverer). When it does change, **remove the previous
registration** before adding the new one, or the table leaks a texture per
redraw.

**The player tile's fallback becomes three steps** — a drawn icon, then the
named sprite, then the glyph. The drawn icon is the one sprite in the game
drawn **untinted**: pass a neutral tint at the vignette's value rather than
`player_look_color(...) * vig`. Verified at `render/base.rs`'s `is_player`
arm: the player's tile colour is `player_look_color(colour) * vig` and
nothing else, so dropping the hue costs the Colour choice and nothing more.

Leave a comment at that site saying so. Adding the hue back reads as a bug
fix — every other sprite inherits its tint — and this is the seam a
reviewer is most likely to "correct".

- [ ] **Step 1: Write the failing tests.** Their intent:
  - A drawn icon is uploaded and drawn at the player's tile — assert the
    mesh **and the absent `@`**. Painting a sprite over a glyph that is
    still there looks perfect against opaque art and breaks the moment one
    pixel is transparent, which for a drawn icon is every icon. This is the
    sprite seam's existing overdraw rule; `paint.rs`'s `with_sprites` is
    the harness.
  - An unchanged icon uploads no second texture.
  - A player with **no** drawn icon still draws their named sprite, and a
    player with neither still draws their glyph — the two rungs that must
    not regress.
  - The Icon step's preview cell draws the drawing.
- [ ] **Step 2: Run and watch fail.** `cargo test -p feral-processes-gui`
- [ ] **Step 3: Implement the upload, the three-step fallback and the preview.**
- [ ] **Step 4: Run and watch pass.** `cargo fmt`, clippy.
- [ ] **Step 5: Commit.**

---

### Task 9: Documentation, the seam, and the full gate

**Files:**
- Modify: `assets/sprites/README.md`
- Modify: `docs/seams.md`
- Modify: `.claude/skills/seams/` — the reference file for the render/HUD
  area; that skill's own README documents the order
- Modify: `CLAUDE.md`, then `cp CLAUDE.md AGENTS.md` (they are gitignored
  twins with no tracking to catch drift)

**The seam, written in all three places:** *the player's drawn icon is the
one sprite drawn untinted, and the player tile's fallback is three-step.*
One sentence in `CLAUDE.md`, the trap in the skill, the argument in
`docs/seams.md` — the order the skill documents.

`assets/sprites/README.md` gains the exception to its near-white rule: a
player-drawn icon carries its own colour and is therefore drawn untinted,
and that is safe only because the player's tile inherits none of the hues
(species colour, `biome_tint`, damage dimming) that the rule protects.

Nothing is added to `docs/manual.md` or the root `README.md` — both are
carved out of the documentation obligation.

- [ ] **Step 1: Write the docs.**
- [ ] **Step 2: Run the full gate.** `cargo test --workspace`, then
  `cargo clippy --workspace` and `cargo fmt --check`. Report the actual
  counts and any failure text; do not claim green without the output.
- [ ] **Step 3: Commit.**
- [ ] **Step 4: Stop.** The version bump, the `CHANGELOG.md` section and
  the tag happen **once, at the merge** — not on the branch, so a rebase
  cannot invalidate a version already tagged. Landing is the user's call
  and needs an explicit ask.

---

## What this plan does not do

A green suite is not evidence of play, and no agent here can launch the
game — there is no display. Four things will only be answerable by playing:
whether `Tab` between two panels reads as obvious without a legend, whether
fifteen colours is too few, whether 16x16 with no fill tool is satisfying or
tedious, and whether losing the Colour choice on the map surprises anyone
who drew an icon. Those go to the user; they are not tasks.
