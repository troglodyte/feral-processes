# Alt shows what each machine makes

**Status:** Approved, 2026-10-28

## Why

In the base, the only way to find out what a machine is making is to walk to
it or open the structure roster. The user wants to hold a key and read the
answer off the map: just the item name, next to each machine, with no text
overlapping other text.

Alt already does this kind of reveal on the base map. Holding it tints each
room and writes the room's name, such as "Fine dormitory". Machine labels join
that same layer.

## What the player sees

- While Alt is held in base space, each producing structure gets a label that
  names the item it makes, such as "Copper wire". The label shows only the
  item's display name, with no counts, progress or rate.
- Room tints and room names still show, as they do today.
- Releasing Alt hides all of it. Nothing changes on the zone surface or the
  Stack.

### What gets a label

| Structure | Label |
| --- | --- |
| Configurable machine with a recipe set | that recipe's item |
| Configurable machine with no recipe set | none |
| Fixed extractor or assembler (`work` block) | its `produces` item |
| Farm plot with a crop sown | the crop's name, one label per plot rather than per cell |
| Unsown plot, Depot, Rack, quarters and everything else | none |

Every structure with a `work` block follows the same rule, with no
exclusions. That includes the research node, which produces the
`research_data` item and so is labelled "Research Data" (Claude's call). An
earlier draft excluded research stations, but that would have needed a
special case.

## Layout

All Alt text, meaning room names and machine labels, is placed by one layout
pass, so no two labels overlap. Text never clips or wraps (see
`gui-text-never-clips-or-wraps`).

**Machine labels**

1. First they try the four spots next to the machine's glyph tile, in this
   order: right, left, above, below. A label in one of these spots has no
   leader line.
2. If all four are blocked, they search outward ring by ring, up to about 6
   tiles from the machine, for the nearest free spot. A label placed this way
   gets a **leader line**: a thin line from the label's nearest edge to the
   machine's tile, in a dimmer version of the label's colour.
3. Among free spots at the same distance, the layout prefers one whose leader
   line crosses no other label's text and no other leader line. A leader line
   never crosses another label's text.
4. If nothing fits within that radius and inside the visible pane, the label
   is dropped. This should only happen in a very packed corner or at the edge
   of the pane.

**Room names**

- A room name may sit on any row of its room that is wide enough to hold it,
  trying the row nearest the room's centre first. Today it is fixed to
  `RoomView::label_at` and dropped if that row is too narrow.
- A room name never gets a leader line, because the tint already shows which
  room it belongs to.

**When labels collide**

The user's rule is that when two labels collide, both move so that both stay
visible. The layout only drops a label when no arrangement within the limits
above holds it.

**What counts as occupied** (Claude's call):

- Other labels' boxes are hard obstacles.
- Structure glyph tiles, and every footprint cell of a multi-tile structure,
  are soft obstacles. A label avoids covering them when any other spot is
  free. Footprint cells come from the existing
  `Game::view_structure_floor_at` grid, which is already read in the map
  draw, so `EntityView` gets no footprint field.
- Open floor, walls, crops, creatures, floor items, corner marks and the Alt
  "?" marker are free to cover.

**Look:**

- Labels use the room names' size (`ROOM_LABEL_SIZE`).
- Room names and machine labels both use the current room-name colour
  (`TEXT`).
- A dark backing box sits behind each machine label so it reads over any
  floor.

### Stability

The layout runs every frame while Alt is held, so the same base and camera
must always produce the same placement. Labels are placed in a fixed order:
room names first, then machine labels sorted by position (y, then x). Ties are
broken by the order listed above. Nothing depends on the order a query
iterates in.

While the camera eases toward its target, labels may shift by whole spots.
That is acceptable. The labels must not flicker between two spots on a
stationary camera.

## How it is built

**Engine.** A new `making: Option<String>` field on `EntityView` holds the
display name of what the structure makes. It comes from
`structures::Placed::production`, which is already the one door to "what does
this placed machine make", turned into a display name through `ItemDb` the
way `Placed::label` does it. It is `None` for every non-producer and for an
unset machine. `FarmTileView` gains a separate `crop_name: Option<String>`
field. The existing `crop` tuple stays as it is, so its other readers don't
change. The renderer groups farm cells by `plot` entity and anchors one label
per plot on that plot's first cell in (y, x) order. A plot is one label even
if its cells are not contiguous. The renderer never touches
`World`, per the architectural rule in `CLAUDE.md`.

**GUI.**

- The layout is a pure function. Its inputs are the label requests (text
  width and height, the anchor tile, room cells or "machine"), the occupied
  tiles and the pane bounds in tile units. It returns a placement per label:
  a position, an optional leader line, or dropped. It takes no `Painter`, so
  it can be tested directly.
- Text widths are measured through `Painter::measure_ui` before the call.
- The leader line draws with the existing `Painter::line`.
- Nothing names a graphics library outside `paint.rs` (the drawing seam).
- `render/base.rs` is about 7,500 lines, so the layout and its tests go in a
  new module beside it, such as `render/alt_labels.rs`, and do not grow
  `base.rs`. The existing room-label code (`room_row_cols`,
  `room_label_fits`, and the label loop in the map draw) moves into that
  layout.
- The Alt key handling is unchanged. `reveal` already reaches the base-map
  draw.

**Data.** No new `.ron` fields, no new asset kind and no save-format change.

## Tests

- **Engine:**
  - `making` is the set recipe's item name for a configurable machine.
  - `making` is the `work` output for a fixed extractor, including
    "Research Data" for the research node.
  - `making` is `None` for an unset machine and for a Depot.
  - The farm view carries the crop name.
- **Layout, pure:**
  - A lone machine gets the right-hand spot with no line.
  - With the right blocked, it gets the left.
  - With all four adjacent spots blocked, it gets a ringed spot and a leader
    line.
  - Two adjacent machines' labels never overlap.
  - A room name moves to another row instead of being dropped when a machine
    label claims its centre row.
  - No leader line crosses another label's box.
  - The same input always gives the same output.
  - A label with no free spot within the radius is dropped.
- **Draw:**
  - With `reveal` held over a base containing a set machine, the item name is
    painted.
  - Without `reveal`, it is not.
  - The draw test follows `the_map_draws_the_alt_marker_only_while_reveal_is_held`.
- **Screenshot check:** take one screenshot of a dev-save base with Alt held,
  using `FERAL_DEV_ALT=1` with `--screenshot`.

## Non-goals

- No toggle, and no new key binding.
- No labels on the zone surface or the Stack.
- No counts, progress, idle or clogged state in the label. The outline already
  shows status.
- No mouse hover.

## Left to the builder

- The exact ring-search order within a ring.
- The leader-line thickness and alpha.
- The backing box's padding.
- The label anchor point for a multi-tile structure. The glyph tile is the
  default.
