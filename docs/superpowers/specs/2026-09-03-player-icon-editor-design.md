# The player icon editor

**Date:** 2026-09-03
**Status:** approved, not implemented

A 16x16 pixel editor the player draws their own map avatar in, opened from
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

**The palette is fixed at fifteen colours, and that is a ceiling, not a
placeholder.** Fifteen plus transparent is sixteen values, which is exactly
one hex digit per pixel — so the palette size and the encoding are the same
decision. A per-drawing palette the player could edit was rejected: it is a
second editing mode on a screen that has no scroll, it doubles what the
encoding has to carry, and at 16x16 a bounded palette is what the medium
wants anyway.

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
symmetry, layers, animation, canvas sizes other than 16x16
(`assets/sprites/README.md`: the format is not negotiable).

## The type

`crates/engine/src/icon.rs`, new:

```rust
/// The colours a drawn icon may use, in palette order. Fifteen, so that a
/// pixel and its transparent zero fit one hex digit.
pub const ICON_PALETTE: [Rgba; 15] = [ /* ... */ ];

pub struct PlayerIcon {
    /// Row-major from the top-left, matching the PNG layout. Each value is
    /// 0..=15: 0 is transparent, 1..=15 index `ICON_PALETTE`.
    pixels: [u8; 256],
}
```

The palette ships as five steps of value from near-black to white and ten
hues. The value ramp is the half that matters — shading with value is the
discipline `assets/sprites/README.md` already asks of every sprite, and it
is what makes a 16x16 figure read at all.

Index 0 being transparent is not decoration: it is the erase colour, and it
is what lets the ground show through a drawn icon the way it shows through
a glyph.

**Encoding, `v1`:** `"v1:"` + 256 hex digits, one per pixel. 259
characters, one line, readable in a text editor.

Decoding is strict — wrong prefix, wrong length, or a non-hex digit all
yield `None`. There is no partial recovery: a
half-decoded icon is a corrupted avatar, and the fallback (the glyph) is
already correct and already tested.

## The editor

`IconEditor` lives in app-core and owns the editing state: the
`PlayerIcon`, a cursor, the selected colour, which of the two panels has
focus, and an undo stack of at most 32 whole `PlayerIcon` values (256 bytes
each, 8 KB — simpler than a diff and small enough that simple wins).

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
A 16x16 grid at 24px needs per-cell rects anyway for the grid lines and the
cursor, and drawing it that way means no texture is uploaded while the
player paints. That is what keeps a texture from being minted per keystroke.

## How it reaches the map

`views::PlayerLook` gains `icon: Option<PlayerIcon>` — about 300 bytes,
rebuilt per frame like the rest of the view.

The gui keeps the last icon it uploaded. Equal by value means no work.
Different means build a `bevy::Image` from the 16x16 RGBA buffer with
`ImageSampler::nearest()`, register it through `EguiUserTextures`, and
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
- `v1` round-trips: a drawn icon encodes and decodes to itself.
- Every malformed form decodes to `None`: bad prefix, short, long,
  non-hex.
- `ICON_PALETTE` is fifteen long. The encoding is one hex digit per pixel,
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
- Taking a preset icon row clears the drawing.
- The wizard opens seeded from the profile's icon.

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
