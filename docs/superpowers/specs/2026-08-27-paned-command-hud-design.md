# The Paned Command HUD

**Status:** phases 1-5 shipped (v0.13.37, .38, .40, .41 and this one); phase
6 (the palette sweep across map glyph colours) is outstanding. Corrections
found while building are folded into the text below rather than appended.
**Date:** 2026-08-27

The main HUD's right-hand column is one undifferentiated text dump —
`render/base.rs::draw_status_panel`, ~250 lines, drawing bars, stats,
party, pets, buffs and the whole inventory into a single scrolling
run of rows with a four-line key block pinned under it. Nothing in it
is ranked, so an idle Mining Node and the player's decompiler stat
read with identical weight, and the one thing on the screen the player
can act on is the hardest thing on it to find.

This replaces that column, and the chrome around it, with the design
handed over in `Rust Bevy Base Building UI/design_handoff_hud/`
("direction 2a, Paned Command"). Read that README for pixel truth:
colours, copy and glyph choices are final there. This spec is what
changes **in this repo** to land it, and where the handoff and the
game disagree.

The organising idea is the **attention model**. One engine derivation
answers "what needs the player right now", and three surfaces — a
status-bar badge, the info column's tab markers, and the collapsed
bars of the closed tabs — are all readouts of that one call. A closed
pane can then never hide an actionable state, which is the whole
argument for making the column tabbed rather than longer.

## What the handoff gets wrong about this game

Four blocks in the reference render are, by its own admission,
plausible inventions. Verified against the source:

- **There is no raid countdown.** `Game::raid_check` is a per-tick
  `random_bool(RAID_CHANCE_PER_TICK)` roll. `GC sweep in 3 ticks` is
  not derivable and is not shown. Anything that "restores" it is
  inventing a schedule the simulation does not have.
- **There are no turrets.** But `Game::raid_defense_active` exists and
  its own doc comment calls itself "the seam frontends use to show the
  shield network as active", so `shields holding` is real and is the
  honest half of the `DEFENCE` block.
- **There is no pack capacity.** `components::Inventory` is an unbounded
  `Vec<(ItemId, u32)>`, and `PlayerStatus::inventory_used`'s own doc comment
  says "The Buffer is unbounded, so this is just how much is stored." A
  `pack full` row is not derivable. The container that *can* fill is the
  roster — `pet_count` against `pet_capacity` — and that is what the fourth
  attention row reports. Anyone "restoring" the pack row is inventing a
  limit the simulation does not have.
- **`PROGRAMS AVAILABLE` and `BUILD QUEUE` have real models** —
  `Game::labour_demand` and `Game::build_order_report` — which the
  handoff guessed at. Use the real ones.

Where this spec and the handoff README disagree, **this spec wins**;
where they agree, the README is the more detailed statement and is
the reference for exact copy and colour.

## Part 1 — Geometry

### `hud::layout` is pure

The handoff specifies a strict 160x38 character grid. This repo does
not have one: `Metrics` ramps UI text continuously off window height
and `map_cell` is an integer ladder off unscii's native cell, and
those two sizing rules deliberately never mix (`text.rs`). Adopting a
real cell grid would re-anchor all thirty-odd popup screens and
invalidate the popup-width censuses.

So the handoff's *arithmetic* is ported and its *grid* is not:

```
layout(screen_w, screen_h, char_w, m) -> HudRegions {
    status_bar, map_pane, log_pane, key_bar, info_column
}
```

`char_w` is a parameter, **not** read off `Metrics` — `Metrics` has no
character-width term and DejaVu Sans Mono's advance is not the
handoff's assumed 0.6 ratio. The caller passes
`painter.measure_ui_advance("M", m.font_size)`. That keeps `layout`
free of `Painter`, which is what makes the geometry unit-testable
headlessly, the same property the popup-width censuses already rely
on.

The five regions, and the two rules that are load-bearing rather than
decorative:

1. **The info column runs to the bottom edge.** The log does not pass
   under it.
2. **The log pane is only as wide as the map pane.** The screen's
   bottom-right corner belongs to the column.

`INFO_W` is `clamp(0.30 * W, 44ch, 56ch)`, so the column's tables
neither crush nor sprawl; everything else falls out of subtraction, as
in `layout_reference.rs`.

### The map pane keeps its own sizing

`map_cell`'s zoom ladder is untouched. The handoff exempts map tiles
from its own cell grid, so this is agreement, not compromise: a larger
window shows more tiles at the same size, which is what reads
correctly for a grid and is already this repo's rule.

`draw_surface_map`, `draw_stack`, `draw_map_inset` and `cell_mark` are
**not touched by this work**. Only the pane they draw into moves.

## Part 2 — The border strip

Text drawn *over* a pane's border run, with the frame background
painted behind it so the border reads as broken by a label. Five
instances: the map title, the threat readout, the vitals strip, the
log filters, and the keybar.

One function, `hud::strip::border_strip`:

```
border_strip(painter, at, runs, m)
//  1. background quad, measured width of runs + padding
//  2. glyphs
// the caller has already drawn the border run AND the pane interior
```

It is one function because the handoff's own warning names the bug:
paint the pane's interior after the label and the label is cut in
half. Written at five call sites that is a bug four of them can have
independently; written once it is a bug the ordering of one function
prevents.

Widths are **measured**, through `Painter::measure_ui_advance`, and
what does not fit is **counted, not clipped** — `stock::fits`' existing
rule, and for its reason: the map's status column has already shipped
a row drawn 360px off the panel in silence.

## Part 3 — The attention model

One new engine derivation, in `views.rs`:

```rust
pub enum AttentionKind { StructureDamaged, IdleStructures, PerkPoints, RosterFull }

pub struct AttentionRow {
    pub kind: AttentionKind,
    pub text: String,
    pub key: char,
    pub threat: bool,
}
// Game::attention(&mut self) -> Vec<AttentionRow>
```

Rows, **threat first and then in this order**:

| Condition | Text | Key | Threat |
|---|---|---|---|
| a structure below full `Durability` | `<name> damaged` | `b` | yes |
| idle workable structures | `N nodes without a program` | `b` | no |
| unspent perk points | `N perk points unspent` | `p` | no |
| roster at capacity | `roster full (n/n)` | `p` | no |

No countdown row and no pack row: see "What the handoff gets wrong".
`threat` selects br red over br yellow, per the handoff's colour
reservations, and it **sorts** as well as colouring — the badge shows the
leading row, and a raid eating the base reading second to an unspent perk
point is wrong on a HUD.

The keys are this game's, not the handoff's: `k` walks west and `m` opens
the Excavation plan. Perks live behind the party menu and both structure
rows behind the base menu, so a chip names the **top-level map key that
starts the journey**.

`kind` is carried so a frontend sorts a row into a pane by an exhaustive
match rather than off its prose or its keycap — `cell_mark`'s rule, and as
a `_ =>` arm a fifth condition ships with no marker on any tab.

**Three consumers, one call.** The status bar's badge, the info
column's tab `!`/`·` markers, and the two collapsed bars all read the
same `Vec`. None re-derives, which is the rule that makes "a closed
pane cannot hide an actionable state" true by construction rather than
by three sites agreeing.

`attention().is_empty()` is what drives `ALL NOMINAL` in green. The
calm state is a real state and is drawn, not an empty gap.

It lives in the engine and not in app-core or the renderer because it
is a claim about game state, and because a renderer-local version
would be three derivations that can disagree.

## Part 4 — The info column

Three tabs — BASE, CREW, PACK — one open, two collapsed to a single
live summary row each.

**It is read-only.** Every verb stays where it is: the manifest (`M`),
the party menu (`p`), the pack (`i`), the base menu (`b`) and the
staffing screens keep every action, every refusal and every scroll
rule they have. A row the player can act on carries its keycap chip at
the right end of that same row, and the chip names the key that opens
the existing screen. Nothing moves out of a popup.

`App` gains exactly one field, `info_tab: InfoTab`, and the keys
`1`/`2`/`3`.

> **The trap.** Those keys have to work underground. `handle_stack_key`
> ends in `_ => {}`, so a key it never sees is a swallowed keypress with
> no refusal and nothing in the log — which is exactly how `r` (rest)
> shipped broken. **In the event one binding does it**:
> `handle_playing_key`'s top match runs *before* the hand-off to
> `handle_stack_key`, which is where `f` already sits, so a second arm
> down there would only be able to drift. What has to exist is the
> assertion, `the_digits_work_underground`, not the second binding.

### Pane contents

**BASE** — structure table (`structure_report`), `PRODUCTION`
(`base_stock` + `work_order_report`), `DEFENCE`, `BUILD QUEUE`
(`build_order_report`), `PROGRAMS AVAILABLE` (idle workable structures
+ `labour_demand`), footer.

`DEFENCE` is two rows: `SHIELDS holding | no defence` off
`raid_defense_active`, and the weakest structure's `Durability`.

**CREW** — party rows and pets (`player_status`, `owned_pets`),
`READINESS`, and the active field buffs (`active_buffs`).

**PACK** — the inventory list (`player_status.inventory`).

### The buff-tag ceiling moves with the buffs

`draw_status_buffs` currently passes `TagStyle::OwnLine` **because the
status column cannot widen** — the map's status column holds 38.5
monospace cells and the widest shipped buff row spends all but 3.8 of
them. The CREW tab is the same width class, so `OwnLine` stays, but
the ceiling it is measured against moves. The width census has to move
with it, or a companion's `(holder)` tag draws off the panel in
silence again.

### No scroll

The column does not scroll, the same as the gear inspect and memories
pages. That makes its height a **layout constraint**: a block that does
not fit has to be cut, not deferred to a scrollbar that does not exist.
Held by a census (Part 6).

## Part 5 — What moves where

`draw_status_panel` is deleted. Every row it drew has a named home:

| Today | New home |
|---|---|
| Stock strip (full-width row 0) | Status bar, centre zone; `stock::fits` overflow rule kept |
| Integrity / Power bars | Vitals strip, map pane bottom border |
| Level / XP / Perk Pts | Vitals strip |
| ATK / MIT / STR / DEC | Vitals strip |
| `Mining: on/off` | Vitals strip — the handoff's own `stats` block already carries `mining_on` |
| Zone / Position | Status bar, left zone |
| Party rows, Pets | CREW tab |
| Active buffs | CREW tab |
| Inventory list | PACK tab |
| Four-line key block | Keybar, one row |
| Refusal line | **Unchanged** — first line of the log pane |

Two rules survive the move verbatim:

- Position is `base_pos().unwrap_or(status.position)`. The anchor tile
  the party stepped through is not the number that means anything to
  someone walking around inside base space.
- The refusal stays in the log pane. `needs_status_banner` names the
  four screens that draw no popup; `Playing` is not one of them and
  nothing about that changes.

### The keybar

One row, grouped by verb, on the log pane's bottom border. The handoff
cut `t trade` and `s save` for width; they stay reachable from the
base/party menus and from `q`. **If the bar measures narrower than its
pane, `t` and `s` go back in before anything else** — the handoff's own
rule, and the census in Part 6 is what answers the question.

Everything else cut from the four-line block is in the manual (`?`),
which is the discoverable home for it.

## Part 6 — Testing

Geometry is pure, so it gets ordinary unit tests: `hud::layout` at
1280x720, 1440x810 and 1920x1080, asserting the two load-bearing rules
(the column reaches the bottom edge; the log never passes under it)
rather than transcribing pixel values.

Four censuses, in the style the repo already uses for popup width:

- `every_border_strip_fits_its_pane` — measured, at the smallest
  supported window.
- `the_tallest_column_pane_fits_its_column` — the column has no
  scroll, so a row past the bottom is dropped in silence. Same trap as
  `the_tallest_gear_page_fits_its_popup`.
- `the_keybar_fits_the_log_pane` — and reports its slack, which is what
  decides whether `t` and `s` come back.
- `attention_drives_all_three_markers` — badge, tab marker and
  collapsed bar agree, because they share one call.

**Delete the fix, watch it fail.** The draw-order census in particular:
a strip test that still passes with `border_strip`'s background quad
removed is measuring nothing. This repo has shipped two vacuous tests
that read as coverage.

## Part 7 — Palette

A new `hud::palette`: the handoff's 16 semantic roles and 14 chrome
fills. Routed through role names, never raw indices — the reservations
**are** the design: br yellow means "the player must act" and br red
means hostility or inbound harm, and neither is ever decorative.

Scope is the HUD and the map's entity glyph colours. The thirty-odd
popup screens keep today's colours. The popups draw *over* the HUD, so
the seam between the two palettes is not visible in play; unifying
them is a separate change and would move several colour censuses.

## Part 8 — Phasing

One release per phase, per the versioning rule. Phases 1-5 each leave
the game playable and independently revertible.

| # | Phase | Lands |
|---|---|---|
| 1 | `hud::layout`, `hud::palette`, status bar absorbing the stock strip | geometry + top row |
| 2 | Map pane frame, `strip.rs`, SECTOR MAP / THREAT / vitals | the signature move |
| 3 | Log pane frame, channel gutter, filter strip, keybar | old key block deleted |
| 4 | Column shell: tabs, `1`/`2`/`3`, collapsed bars, `Game::attention` | the attention model |
| 5 | BASE / CREW / PACK contents; `draw_status_panel` deleted | the column |
| 6 | Palette sweep across map glyph colours | colour |

There is no doubled-chrome period. Phase 1 makes the geometry live for
all five regions at once and **repoints** the existing status panel and
log pane into the new `info_column` and `log_pane` rects; phases 2-5
then replace those regions' contents one at a time. The old panel looks
cramped in its narrower column until phase 5, which is the visible cost,
and it is what buys each phase being revertible on its own.

## Out of scope

- The battle screen and the Stack corridor projection. The Stack gets
  the **new chrome** — it is one `draw_playing_base` and maintaining
  two HUDs is how a status change gets made once instead of twice —
  but the corridor drawing itself is untouched.
- The thirty-odd popup screens, their colours, and every verb they
  hold.
- A real character-cell grid. Considered and rejected in Part 1.
- Mouse and hover. The design is keyboard-only throughout and this
  game has no mouse input.

## Open questions, to be answered by building

- ~~Whether the BASE tab's five blocks fit a 44-cell column at 1280x720
  without a scroll.~~ **Answered in phase 5: yes, with room to spare.** The
  column body is 30.5 rows at that size against the handoff's 21 content
  rows and five dividers, so nothing was cut and the block list ships whole.
  What the census now measures is not silence but *rarity*: overflow is
  counted rather than dropped, so `the_tallest_column_pane_fits_its_column`
  asserts a base under load fits whole, or `+N more` becomes the HUD's
  normal state and the figure stops meaning anything.
- ~~Whether the keybar has slack for `t` and `s`.~~ **Answered in phase 3: no.**
  `the_keybar_fits_the_log_pane` measures twelve segments fitting at 1280x720
  and thirteen at 1920x1080 — the bar is near enough size-invariant, because
  `ui_metrics` ramps the face with the window. The measurement also moved the
  keybar's order off the handoff's: under that reading order `? help` and
  `q menu` fell past the cut, stranding every key the bar had to drop, so the
  order is priority and a named `ESSENTIAL` set says what may never go.
