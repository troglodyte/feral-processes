# The Bard's Tale battle ledger

## Problem

The battle screen reads as a loose stack of rows rather than a stat table.
Both rosters put their numbers inline, in prose-ish parentheses, with each
row free to be a different length:

```
A  4 Null Daemons [BOSS] <engaged> (ATK 9 / DEF 4) [Bleeding (2)]
B  Warden Process <back> (ATK 14 / DEF 9)
```

Nothing lines up vertically, so comparing two groups' DEF means reading two
sentences instead of scanning a column. The reach rule (`<engaged>` /
`<back>`) and any status condition are punctuation-tagged fragments wedged
between the stats, at whatever horizontal position the name before them
happened to end.

The request was to make it feel more like *The Bard's Tale* (1985). Its
combat screen is a bordered stat table with a header row and hard columns —
name, class, AC, HP, status — which is exactly the affordance missing here.

## Approach

**Screen chrome and layout only.** Not the prompt wording, not the combat-log
voice, not the interaction model, and no engine changes: `BattleView` already
carries every field this needs. This is a rewrite of how `draw_battle` lays
its two rosters out, and nothing else.

Rejected along the way:

- **One line per row with an inline mini-bar.** Twice the density and closest
  to a terminal ledger, but `fx.rs`'s ghost drain band and damage-float
  anchoring are both keyed to the full-width bar geometry. Reworking the
  battle effects to buy density is a bad trade when the effects are the thing
  that makes a hit legible.
- **Header row only, leaving `draw_bar`'s appended `hp/max` where it is.**
  The smallest possible diff, but it strands HP in the last column instead of
  beside the name, so the columns stop matching the order that reads best.
- **A shared column-layout module.** There is one renderer now (see
  `2026-07-24-delete-the-tui-design.md`), so there is nothing to share with
  and nothing to drift against.

## Design

### The layout

The four stacked blocks stay exactly as they are — hostiles, log, party,
action bar — as does the two-visual-lines-per-row shape: a text line, then
the full-width gradient bar with its ghost band beneath it. What changes is
that the text line becomes a set of fixed-width columns, and each roster
block gains a header row above its first entry.

```
   GROUP             HP        ATK DEF RANGE   STATUS
A  4 Null Daemons   18/30        9   4 ENGAGED BLEED 2
███████████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░
B  Warden Process   44/44       14   9 BACK    OK
█████████████████████████████████████████████████████

   NAME              HP        ATK DEF POS   ACTION
>1 You              21/30       11   6 FRONT Attack A
██████████████████████████████████░░░░░░░░░░░░░░░░░░
```

### Columns

One row format, shared by both rosters so their columns line up with each
other as well as with their own headers:

```
{mark}{name:<NAME_W} {hp:<HP_W} {atk:>STAT_W} {def:>STAT_W} {reach:<REACH_W} {tail}
```

| Cell | Width | Hostiles | Party |
|---|---|---|---|
| `mark` | 3 | `A  ` — the group letter | `>1 ` / ` 1 ` — selection caret and slot number |
| `name` | 18 | `4 Null Daemons` (pluralised as today) | `You` or the companion's label |
| `hp` | 11 | `18/30`, one cell rather than two so the header can sit over it | same |
| `atk` / `def` | 3, right-aligned | `effective_atk`/`def` as today | same |
| `reach` | 7 | `ENGAGED` / `BACK` | `FRONT` / `BACK` |
| `tail` | rest of line | `STATUS`: `OK`, or the condition upper-cased | `ACTION`: the planned action, or `—` |

Header text is the column name in each cell, at the same size and x-origin
as the rows, so alignment is structural rather than eyeballed.

Numeric columns are right-aligned and text columns left-aligned — the point
of the exercise is that a column of DEF values can be compared by scanning
down it.

### What the columns say

`RANGE`/`POS` and `STATUS` become upper-case words in their own columns
instead of `<engaged>`-style tagged fragments. The renderer already authors
those two strings itself, so this is a presentation change with no engine
involvement.

`STATUS` reads `OK` when there is no condition, rather than being blank: a
blank cell in a ledger reads as missing data, where `OK` reads as a checked
box. The condition itself is `BattleGroupView::status_effect` upper-cased —
the engine owns its wording (`"Bleeding (2)"`), and the renderer is not
entitled to invent abbreviations for a vocabulary it does not define, so
`BLEEDING (2)` is what a long one becomes, truncated to fit like any other
cell.

`is_boss` keeps its `[BOSS]` marker, appended to the name cell before
padding, since magenta alone carries it today and the tag is what survives
truncation.

### Truncation

Every cell is padded to exactly its width, and truncated when it overruns —
a long species name has to lose its tail rather than push every column after
it to the right, which would defeat the entire change. Truncated cells end in
`…` (DejaVu Sans Mono has it; the map font is not involved in UI text).

This is the one place the design can visibly bite: `zone_tagged_name` can
make a name longer than 18 cells. That is the cost of columns, and it is
paid deliberately.

### Where the widths live

Constants in `crates/gui/src/render.rs`, beside `draw_battle`. There is one
renderer, so there is no second consumer to keep in sync and no reason to
lift them into `app-core`.

### `draw_bar` stops appending `hp/max`

`draw_bar` currently builds `format!("{label} {value:.0}/{max:.0}")`, which
forces HP to the end of the text and makes column order unachievable. It
will draw the label it is given, verbatim.

Three non-battle callers (the status panel's Integrity/Power/Fatigue bars,
`render.rs:543`/`556`/`569`) rely on the append and will format their own
labels. This makes the function honest about what it does rather than
hiding a formatting decision inside a drawing primitive.

## Verification

The GUI cannot be tested by drawing it, and this project's standing rule is
to verify drawing changes through unit tests rather than launching a window.
Three things are worth pinning, and all three are pure:

1. **The UI font is monospace.** The entire ledger rests on it. Asserted the
   way unscii's crispness already is — headlessly, through `fontdue` in
   `crates/gui/tests/font_rasterization.rs` — by rasterizing several
   characters of differing natural width and asserting one advance.
2. **Row and header formatting.** The row builder is a pure
   `&BattleView`-shaped-input → `String` function, so it can be asserted
   directly: every row the same display width as its header, columns landing
   at the same offsets, and an over-long name truncated rather than shifting
   the columns after it.
3. **Cell padding and truncation.** The `cell` helper: exact width in, exact
   width out; over-width in, exact width out ending in `…`.

Plus the standing gate: `cargo test --workspace` (440 at the time of
writing), `cargo clippy --workspace --all-targets` clean, `cargo fmt`.

## Consequences

**A long name gets clipped where today it pushed the line wider.** Deliberate
— see Truncation.

**Rows get slightly taller in text terms and identical in bar terms.** The
header line costs one `line_height` per roster block, twice per screen. The
log pane absorbs it, and it is already the flexible element between the two
rosters.

**The party block's small-window overrun is not addressed.** With four enemy
groups on a short window the party block can still bottom-anchor past the
action bar; the two header rows make that marginally more likely. It is a
pre-existing layout question — what gives way when nothing fits — and
belongs to whoever answers that, not to this change.

## Not doing

- Numbering the enemy groups. Bard's Tale used numbers, but
  `EnemyGroupView::letter` is engine-owned and the target picker keys off it,
  so renumbering is an engine change and out of scope.
- Padding the party block to `MAX_PARTY_SIZE + 1` rows. The roster is sized
  once at battle start and a downed companion now keeps its row at 0 HP (as
  of `4fd8f7e` — it did not before), so the screen does not reflow and the
  blank rows would buy nothing but cost the log up to five lines.
- The popup pickers (`Mode::BattleTarget`, `BattleSpecial`, `BattleAlly`,
  `BattleItem`). They are lists, not rosters, and were not part of the
  complaint.
- Prompt wording, log voice, interaction model. Explicitly out of scope.
