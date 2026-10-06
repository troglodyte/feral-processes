# Battle portrait — design

**Date:** 2026-10-06 · **Branch:** `battle-portrait`

## Goal

The group battle screen (every Stack fight) is all text. Add a single
Bard's Tale–style picture window showing one combatant's sprite: whoever
is acting as the round's narration reveals, and the first-in-line hostile
otherwise.

Decisions taken in brainstorming:

- One window, left of the log panel; log text flows to its right.
- Shows every actor in turn (hostile or party) during the reveal.
- Missing sprite: the player's drawn icon, else the entity's glyph drawn large.
- Still image: no dimming, flash or animation (effects can come later via
  `assets/effects/`).

## Engine (`crates/engine`)

**Who is shown is decided by the engine**, so it is unit-testable headless
and the renderer reads one field.

- New `PortraitView { sprite: Option<String>, glyph, color, name, is_player }`
  in `views.rs`, built from the same sources `EntityView` resolves sprite,
  glyph and colour from (call the shared resolver, don't copy it).
- `BattleView` gains `portrait: Option<PortraitView>`.
- `RosterFrame` gains `actor: Option<PortraitView>`, set when the frame for
  a swing line is recorded (party actors by slot, hostiles by entity — the
  initiative's own addressing). Frames already hold through unframed lines,
  so the actor does too.
- Selection, in `battle_view_at(revealed)` and live `battle_view`:
  1. the revealed frame's `actor`, if `Some`;
  2. else the front member of the first engaged group (`front_of_group`);
  3. else `None` (a won fight's empty hostile half — the pane clears, as the
     roster does).
- The battle-result view (`closing` roster) follows rule 2/3.
- No save-format change: `BattleTimeline` is transient and unsaved.

## GUI (`crates/gui/src/render/battle.rs`)

- The log panel's `Rect` is split: a square portrait pane on the left, side
  = the log height minus a caption row, floored to a multiple of 16 px
  (nearest-sampled pixels stay crisp); the log draws in the remainder.
- Skip the pane entirely (log keeps full width) when `portrait` is `None` or
  the remainder would be narrower than a minimum log width (named constant).
- Draw order: for `is_player`, `@drawn` untinted (`DRAWN_ICON_KEY`); else /
  then `sprite` with a neutral tint; if `Painter::sprite` returns `false`,
  draw `glyph` in `color` scaled to the pane. Gate on the return value, never
  on the name.
- Caption: `name` centred under the image.
- Drawn only through `Painter`; no new paint primitive is needed.

## Seam rule

`.claude/rules/drawing-seam.md` says a sprite substitutes for an entity's
glyph and is never drawn beside it. Add a scoped exception: the battle
portrait is a picture pane, not a glyph substitute; its only glyph is the
fallback, drawn only when the sprite call returns `false`. Update all three
places per the `seams` skill (graph `seam:drawing-seam`, skill, rules file).

## Testing

Engine:
- portrait follows the actor as `revealed` advances across a round with
  party and hostile swings;
- unframed lines hold the previous actor;
- planning / post-round falls back to the front of the first engaged group;
- `None` after a win;
- a party actor carries `is_player` correctly and its sprite name.

GUI (`with_sprites` / `painted_images`):
- sprite image painted and its glyph absent;
- glyph painted when the sprite is missing;
- player with a drawn icon paints `@drawn`;
- pane skipped on a narrow log and when `portrait` is `None`.

Visual check: `cargo run -- --template stack --keys … --screenshot` into a
fight, Read the PNG.

## Release

Patch bump, `CHANGELOG.md` entry. No asset schema change.
