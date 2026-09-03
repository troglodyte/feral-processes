# The player icon editor

**Date:** 2026-09-03
**Status:** implemented (v0.13.87), kept in `specs/` pending the usual
archive move
**Amended:** 2026-09-03 — the drawn grid halved to 8x8. See "The two grids"
below; every statement in this document reflects the amendment.

An 8x8 pixel editor the player draws their own map avatar in, opened from
the character-creation wizard's Icon step. A fixed palette, keyboard only,
persisted across runs.

TODO #60 asks for two things — a player-facing icon creator and a developer
tool for authoring structure and entity art. This spec is the first only.
The second is a different feature with a different consumer and is out of
scope; nothing here forecloses it.

## Why this is worth building

The sprite seam shipped with two images in `assets/sprites/` — `player.png`,
explicitly a generated placeholder, and `anchor.png`. The wizard's Icon step
offers five `(glyph, sprite)` pairs of which four name art that does not
exist, so today every one of them draws its glyph. The pipeline is finished
and has nothing to carry.

Letting the player draw their own is the one use of that pipeline that does
not wait on an artist.

## What was decided

**The editor is not a `Mode`.** `crates/app-core/src/app/creation.rs`'s
module doc states the rule: `Mode::CreateCharacter` carries a
`CreationStep` cursor "rather than being nine modes — `Mode::Transfer`'s
reason". The editor follows it as `icon_editor: Option<IconEditor>` on the
creation state, routed at the top of the Icon step's key handler. A new
`Mode` would cost every `Mode` census a row for a screen reachable from
exactly one other screen.

**The two grids: the player draws 8x8, the sprite stays 16x16.** The
editor shipped on a 16x16 canvas and was halved the same day. 256 cells is
more drawing than anyone wants to do with four arrow keys and a paint key,
and what reads at map zoom is a silhouette rather than a portrait. The
*sprite* did not move and must not: `assets/sprites/README.md` calls that
format non-negotiable, `text::map_cell`'s ladder is integer multiples of
it, and structure and entity art will land on it later. So each of the 64
drawn cells paints a 2x2 block of the 16x16 sprite — under
`ImageSampler::nearest()` that is pixel-identical to a native 8x8 texture,
and it leaves the sprite seam untouched. `ICON_GRID` is what the player
edits, `ICON_SIZE` is what the texture is, and `ICON_CELL_PIXELS =
ICON_SIZE / ICON_GRID` is the one place the relationship is stated; every
site that crosses between the two reads it rather than assuming a `2`.

**The palette is fixed at fifteen colours, and that is a ceiling, not a
placeholder.** Fifteen plus transparent is sixteen values, which is exactly
one hex digit per cell — so the palette size and the encoding are the same
decision, and halving the grid did not touch it: the palette bounds what
one *digit* can say, the grid bounds how many digits there are. A
per-drawing palette the player could edit was rejected: it is a second
editing mode on a screen that has no scroll, it doubles what the encoding
has to carry, and at this size a bounded palette is what the medium wants
anyway.

**The drawn icon is drawn untinted, at the vignette's value.**
`assets/sprites/README.md` requires authored art to be near-white because
egui's tint *multiplies* — a red sprite on a species authored green goes
black. A palette carrying hues is incompatible with that, so the drawn icon
opts out of the hue and keeps only the map's depth vignette. It is the one
sprite in the game drawn this way, and it is safe precisely because it is
the player's own tile: the hues a sprite normally inherits — species
colour, `biome_tint`, damage dimming — none of them reach the player's `@`,
which already draws at `player_look_color(colour) * vig` and nothing else.
That was verified in `render/base.rs`'s `is_player` arm, not assumed.

**The consequence is stated, not hidden.** A player who draws an icon no
longer sees their Colour choice on the map tile. It still governs the glyph
on every other surface, so the step keeps its place in the wizard and gains
one line of help text saying what it still does. The Icon step's preview
cell draws the art exactly as the map will, so the two screens cannot
disagree about what was chosen.

**The art is stored twice, and the second copy is the point.** The profile
holds the last thing drawn, so it carries into the next run. The save holds
what *this* character looks like. Profile-only storage means redrawing your
icon silently repaints every character already made; the save copy pins a
look to the character who chose it.

**One plain string, strictly decoded.** `achievements.rs` documents that
`Profile::load` discards the whole profile — achievements included — when
it cannot parse, which is why `seen_notifications` is a `Vec<String>` where
every other id in that file is a type. An icon inherits that rule exactly:
it is one string, and a string this build cannot read decodes to `None`
rather than costing the player everything they have earned.

## Non-goals

Companions, structures and any entity but the player. A developer tool that
writes into `assets/sprites/`. PNG import or export. Mouse input — there is
none anywhere in the gui and `GameKey` is the whole vocabulary. A
player-editable palette or any free-RGB picker. Flood fill, mirror or
symmetry, layers, animation, and any *sprite* size other than 16x16
(`assets/sprites/README.md`: the format is not negotiable — the drawn grid
is a separate number, see "The two grids").

## The type

`crates/engine/src/icon.rs`, new:

```rust
/// The colours a drawn icon may use, in palette order. Fifteen, so that a
/// cell and its transparent zero fit one hex digit.
pub const ICON_PALETTE: [Rgba; 15] = [ /* ... */ ];

/// The sprite's edge, in pixels. Not negotiable.
pub const ICON_SIZE: usize = 16;
/// The drawn grid's edge, in cells.
pub const ICON_GRID: usize = 8;
/// The one expression of the relationship between them.
pub const ICON_CELL_PIXELS: usize = ICON_SIZE / ICON_GRID;

pub struct PlayerIcon {
    /// Row-major from the top-left, matching the PNG layout. Each value is
    /// 0..=15: 0 is transparent, 1..=15 index `ICON_PALETTE`.
    cells: [u8; ICON_GRID * ICON_GRID],
}
```

The palette ships as five steps of value from near-black to white and ten
hues. The value ramp is the half that matters — shading with value is the
discipline `assets/sprites/README.md` already asks of every sprite, and it
is what makes a figure this small read at all.

Index 0 being transparent is not decoration: it is the erase colour, and it
is what lets the ground show through a drawn icon the way it shows through
a glyph.

**Encoding, `v2`:** `"v2:"` + 64 hex digits, one per drawn cell, row-major
from the top-left. 67 characters, one line, readable in a text editor.

Decoding is strict — an unknown prefix, a wrong length, or a non-hex digit
all yield `None`. There is no partial recovery: a half-decoded icon is a
corrupted avatar, and the fallback (the glyph) is already correct and
already tested.

**`v1` is still read, and folded rather than discarded.** A `"v1:"` payload
is 256 digits over the old 16x16 grid; each 2x2 block folds to one cell,
which takes **the most frequent non-transparent index in that block**, ties
broken in reading order, an all-transparent block staying transparent.
Sampling one corner of each block would be shorter and would delete every
one-pixel outline a player drew; the majority rule keeps the silhouette,
which is what survives the halving. A decoded `v1` is an ordinary
`PlayerIcon` and re-saves as `v2` — there is no second kind of icon
anywhere downstream.

## The editor

`IconEditor` lives in app-core and owns the editing state: the
`PlayerIcon`, a cursor, the selected colour, which of the two panels has
focus, and an undo stack of at most 32 whole `PlayerIcon` values (64 bytes
each, 2 KB — simpler than a diff and small enough that simple wins).

**The screen is two panels and `Tab` moves between them.** The arrows mean
one thing at a time — move the cursor on the canvas, or move along the
palette — rather than meaning different things depending on a mode the
player has to remember they are in. Which panel has focus is drawn, so the
answer is on screen rather than in the player's head.

| Key | Does |
|---|---|
| `Tab` | Move focus between the canvas and the palette |
| Arrows | Move the cursor, or move along the palette — whichever has focus |
| `Space` | Paint the cursor cell with the selected colour |
| `Backspace` | Erase the cursor cell (index 0) |
| `u` | Undo |
| `x` | Clear the canvas |
| `Enter` | Keep the drawing, return to the Icon step |
| `Esc` | Discard changes, return to the Icon step |

**`GameKey::Tab` is new.** `map_special_key` in `crates/gui/src/lib.rs`
falls through to `None`, so this is one variant and one mapping line, and
every existing key table ignores it. It is not added to `REPEATING_KEYS`:
focus is a toggle between two panels, and holding it would flicker.

**The editor screen draws its own canvas as rectangles, not as a texture.**
The grid needs per-cell rects anyway for the grid lines and the cursor, and
drawing it that way means no texture is uploaded while the player paints.
That is what keeps a texture from being minted per keystroke. The canvas
cell is `2.4` line-heights — double what it was at 16x16 — so the halved
grid keeps exactly the canvas size both layout censuses were verified
against at 1280x720, and the palette strip still fits beneath it.

## How it reaches the map

`views::PlayerLook` gains `icon: Option<PlayerIcon>` — about 300 bytes,
rebuilt per frame like the rest of the view.

The gui keeps the last icon it uploaded. Equal by value means no work.
Different means build a `bevy::Image` from the 16x16 RGBA buffer — each
drawn cell expanded to its `ICON_CELL_PIXELS` block, through
`PlayerIcon::pixel_rgba` — with `ImageSampler::nearest()`, register it through `EguiUserTextures`, and
insert it into `SpriteTable` under a reserved key. `nearest()` is not
optional: `sprites.rs` documents it as the whole reason pixel art stays
crisp at zoom, because bevy_egui binds the image's own sampler and bevy's
default is linear. The previous registration is removed when it is
replaced, or the table leaks a texture per redraw.

The player tile's fallback becomes three steps, and the order is the seam's
existing rule with one rung added on top:

1. a drawn icon, if the player has one — drawn at a neutral tint scaled by
   the vignette;
2. the named sprite from `PlayerLook::sprite`, tinted as today;
3. the glyph.

`Painter::sprite` is unchanged. It still reports whether it drew, and a
caller that gets `false` still falls back — which is what keeps
`assets/sprites/` optional and keeps a build with no art at all working
exactly as it does now.

## The wizard

`CREATION_ICONS` keeps its five pairs. The Icon step gains a sixth row that
is not a pair — a distinct `CreationRow` kind, because the existing one
carries `(glyph, sprite)` and the drawing has neither. Taking it opens the
editor; leaving the editor returns to the Icon step with that row selected.
Taking any of the five preset rows clears the drawing, so the two choices
cannot both be live.

No `Decided` flag is needed. `[r]` rerolls the kit and only the kit, and
that is documented at length in the same file — nothing else in the wizard
is rolled any more, so there is nothing to protect the drawing from.

The wizard seeds itself from `Profile::player_icon` when it opens, so the
last thing drawn is the starting point for the next character rather than a
blank grid.

## Storage

| Where | Field | Holds |
|---|---|---|
| `achievements::Profile` | `player_icon: Option<String>` | the last icon drawn, cross-run |
| `save::PlayerSave` | `icon: Option<String>` | what this character looks like |
| `components::PlayerIdentity` | `icon: Option<PlayerIcon>` | the live value |

Both persisted fields are additive and `#[serde(default)]`, so **no
`SAVE_FORMAT_VERSION` bump** — the save is field-named RON and that is
exactly what retired save migrations.

## Testing

**engine**
- `v2` round-trips: a drawn icon encodes and decodes to itself.
- Every malformed form decodes to `None`: bad prefix, short, long,
  non-hex — for `v2` and for `v1` on its own length.
- `v1` folds: a block holding three of one colour and one of another takes
  the majority (which is what fails against a decoder that samples the
  top-left of each block), a wholly transparent block stays transparent, a
  tie breaks in reading order, and a decoded `v1` re-encodes as `v2`.
- The two grids divide exactly, and one drawn cell covers its whole pixel
  block through `pixel_rgba`.
- `ICON_PALETTE` is fifteen long. The encoding is one hex digit per cell,
  so a sixteenth colour would be unencodable — and unreachable in a way
  nothing else would report.
- A profile carrying an unreadable icon still loads its achievements. This
  is the property the plain-string decision exists for, and it is the one
  that is expensive to get wrong.
- A save round-trip through `Game::save` / `Game::load` — not a RON
  round-trip, which stays green for a field that never reaches disk.

**app-core**
- Paint, erase, colour select, clear.
- `Tab` moves focus, and the arrows act on the focused panel alone — the
  test that would have caught arrows painting while the palette has focus.
- Undo restores the previous canvas and the stack is bounded at 32.
- `Esc` discards and `Enter` keeps.
- Taking a preset icon row clears the drawing on the *choice*, and does
  not erase the profile's stored one.
- The wizard opens seeded from the profile's icon, `v1` and `v2` alike —
  a `v1` profile seeds the folded 8x8 figure, which is the only place a
  player would ever see the fold.
- Keeping an all-transparent canvas is not a drawing. Decided at the
  `Keep` arm; the upload's filter stays as defence in depth.

**gui**
- The upload expands each drawn cell to its `ICON_CELL_PIXELS` block,
  asserted on the texture's actual bytes: a lit cell is exactly one 2x2
  square of opaque pixels at twice its coordinates, and its neighbours
  stay bare.
- `ImageSampler::nearest()` is on the uploaded image — the one line
  nothing on screen reports.
- The whole screen fits 1280x720 with no scroll, **measured at 1280**
  rather than at the painter's fixed 1440, and the palette strip fits
  under the canvas. Both verified by mutation, not by inspection.

**gui**
- A texture is built from raw pixels and drawn, asserting the mesh *and*
  the absent `@` — the overdraw rule from the sprite seam: painting a
  sprite over a glyph that is still there looks perfect against opaque art
  and breaks the moment one pixel is transparent, which for a drawn icon is
  every icon.
- An icon that has not changed uploads no second texture.
- The editor screen is drawable at 1280x720 with no scroll.

## Files

**engine** — `icon.rs` (new), `lib.rs` (module), `components.rs`
(`PlayerIdentity::icon`), `views.rs` (`PlayerLook::icon`),
`game/inspection.rs` (populate it), `game/creation.rs`
(`CharacterChoice::icon` through to the spawned player), `save.rs`,
`achievements.rs`.

**app-core** — `app/creation.rs` (the editor, its keys, the sixth row, the
profile seed), `lib.rs` (`GameKey::Tab`, and the view type the editor
screen draws from).

**gui** — `lib.rs` (`map_special_key` gains `KeyCode::Tab`),
`render/creation.rs` (the editor screen; the preview cell already exists),
`render/base.rs` (the three-step fallback and the neutral tint),
`sprites.rs` or `paint.rs` (registering a runtime texture).

**docs** — `assets/sprites/README.md` gains the one exception to its
near-white rule; `CHANGELOG.md`; and one seam recorded in all three places
CLAUDE.md requires: the argument to `docs/seams.md`, the trap to the
`seams` skill, the rule to CLAUDE.md.

**The seam:** *the player's drawn icon is the one sprite drawn untinted,
and the player tile's fallback is three-step.* The trap is that adding the
hue back looks like a bug fix — every other sprite in the game inherits its
tint, and a reviewer who knows that rule will read the neutral tint as an
omission.
