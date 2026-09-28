---
paths:
  - "crates/gui/**"
---

# Load-bearing seams: The HUD

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **`Game::attention` is the one derivation of what needs the player, and
  three surfaces read the same call.**
- **`1`/`2`/`3` are bound once, in `handle_playing_key`'s top match**, which
  runs before the hand-off to `handle_stack_key` — the same place `f` sits.
- **`draw_info_column` owns the column's chrome and returns the open pane's
  body rect.** The column **does not scroll**, so that rect's height is a
  layout constraint.
- **A pane body is rows, and `hud::panes::fitting_rows` is the one place
  what does not fit is counted** — `strip::fitting`'s rule turned ninety
  degrees, written once rather than once per pane.
- **`PaneData` takes `roster`/`carrying` and not a `&PlayerStatus`**: those
  are every field of it the panes read, and `PlayerStatus` derives nothing,
  so a wider dependency is a census nobody writes.
- **The buff-tag ceiling moved with the buffs.** CREW calls `buff_entries`
  rather than restating a buff row, `TagStyle::OwnLine` because the column
  cannot widen.
- **A collapsed bar carries `panes::summary` when calm, and a condition
  outranks it.** Built from the same `PaneData` as the open pane's rows, so
  a bar and the pane it stands for cannot disagree.
- **A content hue is authored, and `hud::palette::glyph` is the one table it
  is drawn from** — `render/mod.rs::glyph_color` is a call to it, shared
  with the six popups that draw a program's glyph, since a program reading
  as one colour on the grid and another on its own sheet is the failure to
  avoid.
- **A tile's con read takes the glyph's own hue whenever it can, and
  `ConRead` is the one place that is decided** — the top-left earmark is
  the fallback for the two tiles whose ink is spoken for, a boss's magenta
  and a drawn sprite, gated on the sprite call's own answer and never on
  the sprite's name.
- **The player's `@` is a role and is read off `is_player`** — `PLAYER`, br
  cyan, which nothing else may take, and never off the `GlyphColor::Cyan`
  the player happens to spawn with.
- **`ICON_PALETTE` is exactly fifteen entries because it is the player
  icon's save format, and `SPRITE_PALETTE` is a separate constant for that
  reason.**
- **A machine's stall asks for attention and never reads as a threat.**
  `Clogged`/`Stranded`/`Unpowered` take `ATTENTION` — waiting fixes none of
  them, and it is the colour `Game::attention` already spends on them —
  while `Starved`/`Unstaffed` keep the dimmer `WARN`.
- **The map's overlays take palette roles too, and `fx.rs` is why the
  palette is `pub(crate)`** — a raid's flash is painted by the effects
  layer, and a structure taking a hit is what the `THREAT` reservation
  exists for.
- **A tile's bottom edge belongs to the progress bar, and the bottom-corner
  marks lift above it the way the top-corner ones drop below the rarity
  bar** — drawn after `outline_open`, and in the hue the cell already wears.
- **The expanded log pane is an overlay drawn last, and `map_pane` is
  derived from the *collapsed* log at every window size.**
- **One strip to a border, and the vitals get the contested one** —
  they ride `log_pane`'s top border, the filter header is the log body's
  first row again, and `map_pane`'s bottom border carries nothing.
- **A pane whose border carries a strip starts its body at
  `hud::layout::strip_inset` and buys the height for it**, because that
  strip's quad reaches as far *into* the pane as it does out of it.
- **The compass is a block *inside* the map pane, not a strip on its
  border**, so a selection costs no layout — and it starts at
  `strip_inset` because THREAT's quad hangs into the pane above it.
