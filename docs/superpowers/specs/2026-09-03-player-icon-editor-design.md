# The player icon editor

**Date:** 2026-09-03
**Status:** approved, not implemented

A 16x16 pixel editor the player draws their own map avatar in, opened from
the character-creation wizard's Icon step. Free-RGB colour, keyboard only,
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

**The drawn icon is drawn untinted, at the vignette's value.**
`assets/sprites/README.md` requires authored art to be near-white because
egui's tint *multiplies* — a red sprite on a species authored green goes
black. Free RGB is incompatible with that, so the drawn icon opts out of
the hue and keeps only the map's depth vignette. It is the one sprite in
the game drawn this way, and it is safe precisely because it is the
player's own tile: the hues a sprite normally inherits — species colour,
`biome_tint`, damage dimming — none of them reach the player's `@`, which
already draws at `player_look_color(colour) * vig` and nothing else.

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
none anywhere in the gui and `GameKey` is the whole vocabulary. Flood fill,
mirror or symmetry, layers, animation, canvas sizes other than 16x16
(`assets/sprites/README.md`: the format is not negotiable).

## The type

`crates/engine/src/icon.rs`, new:

```rust
pub struct PlayerIcon {
    /// Swatches 1..=8. Index 0 is transparent and is not stored.
    palette: [Rgba; 8],
    /// Row-major from the top-left, matching the PNG layout. Each value is
    /// 0..=8: 0 is transparent, 1..=8 index `palette`.
    pixels: [u8; 256],
}
```

Nine values fit a nibble exactly, which is what makes the encoding one
character per pixel. Index 0 being transparent is not decoration: it is the
erase colour, and it is what lets the ground show through a drawn icon the
way it shows through a glyph.

**Encoding, `v1`:** `"v1:"` + eight swatches as `RRGGBBAA` hex + 256 hex
digits, one per pixel. 3 + 64 + 256 = 323 characters, one line, readable in
a text editor.

Decoding is strict — wrong prefix, wrong length, a non-hex digit, or a
pixel index above 8 all yield `None`. There is no partial recovery: a
half-decoded icon is a corrupted avatar, and the fallback (the glyph) is
already correct and already tested.

## The editor

`IconEditor` lives in app-core and owns the editing state: the
`PlayerIcon`, a cursor, the selected swatch, whether a swatch's channels
are open for editing, and an undo stack of at most 32 whole `PlayerIcon`
values (288 bytes each, ~9 KB — simpler than a diff and small enough that
simple wins).

| Key | Does |
|---|---|
| Arrows | Move the cursor |
| `Space` | Paint the cursor cell with the selected swatch |
| `Backspace` | Erase the cursor cell (index 0) |
| `1`-`8` | Select a swatch |
| `e` | Open/close the selected swatch's R/G/B channels |
| Arrows, while channels are open | Left/Right adjusts the highlighted channel, Up/Down picks one |
| `Shift`+arrow / `Ctrl`+arrow | Target and step on a channel, the pair `Mode::Transfer` already defines |
| `u` | Undo |
| `x` | Clear the canvas |
| `Enter` | Keep the drawing, return to the Icon step |
| `Esc` | Discard changes, return to the Icon step |

The default palette ships eight swatches spanning value and a few hues, so
the editor is usable on the first keystroke rather than opening on eight
identical blacks.

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
  non-hex, a pixel index of 9.
- A profile carrying an unreadable icon still loads its achievements. This
  is the property the plain-string decision exists for, and it is the one
  that is expensive to get wrong.
- A save round-trip through `Game::save` / `Game::load` — not a RON
  round-trip, which stays green for a field that never reaches disk.

**app-core**
- Paint, erase, swatch select, channel edit, clear.
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
profile seed), `lib.rs` (the view type the editor screen draws from).

**gui** — `render/creation.rs` (the editor screen; the preview cell already
exists), `render/base.rs` (the three-step fallback and the neutral tint),
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
