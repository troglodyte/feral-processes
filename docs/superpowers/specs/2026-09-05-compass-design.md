# The compass

**Status:** approved, unimplemented

A selected destination and a bearing to it. The player opens a picker
(`u`), chooses one of the places the run already knows about — the home
base, a settlement, a Stack entrance — and a one-line strip on the zone
map's bottom border points at it for as long as the selection stands.

## The problem this closes

Settlement placement is correct and unfindable. `settlements::placement::
settlement_at` puts a town in 45% of regions, a region is 256 tiles across
(`SETTLEMENT_REGION_CHUNKS` x `CHUNK_SIZE`), and the map viewport is about
45 x 25 tiles. Measured over 400 seeds, the nearest town to spawn is a
median **143 tiles** away and a quarter of runs need 200+.

Nothing announces one. `spawn_settlement_at` logs nothing; there is no
marker, no direction cue, and the only signal is the glyph entering the
viewport.

The measured case is `save_1788625345.bin` (character `trog`, seed
1788625345, tick 1763). Six settlements stand recorded in
`resources::Settlements`, `standings` is empty, and the player's
`populated_chunks` reach y <= 191 — minus `POPULATION_CHUNK_MARGIN`, about
y <= 159 in actual footsteps. Lowport is at (-45, 219) and Kernel Reach at
(129, 225). **The run turned around roughly 60 tiles short of two towns
the engine had already recorded**, after ~68,000 tiles of ground.

So the fault is not placement and not discovery. It is that the engine
knows and never says.

## Decisions taken

Recorded so they are not relitigated. Each was chosen against a named
alternative.

1. **Two tiers of knowledge, and they are two fields.** A target the engine
   has recorded but the party has not reached gives a bearing and a generic
   label — "a settlement, south". Once reached it gives its name and a
   distance. *(See the amendment below: `distance` was moved out of the
   tier.)* Rejected: listing only visited places (fixes "I found Lowport
   once and cannot get back"; fixes nothing about the trog save, whose list
   would be empty), and listing everything in full (directly fixes the trog
   save, but is a scan with extra steps — the thing `Progression is earned
   by fighting` closed off alongside the Terminal, free rest and the
   Market's fragment listing).

   **Amended 2026-09-05, after the feature shipped in 0.13.103: the tier is
   one field, `label`, and `distance` is answered for every row.** The
   two-field version was wrong about which half is earned. A name is
   something arriving at a place teaches you; *how far away it is* is not —
   the engine derived that figure from coordinates it already had, and
   withholding it left the first tier unable to answer the only question the
   compass exists for, which is whether the party can afford the trip. A
   bearing with no figure is a direction to wander in, which is the fault
   the feature was built to close rather than a softer form of it. The
   rejected "listing everything in full" is still rejected and is still the
   line: a generic noun is not a scan, because it names nothing the run has
   not earned.
2. **Three target kinds: home, settlements, Stack entrances. Nests are
   out.** Nests are numerous, destructible and hostile; a "go here" list of
   them is a hunting list, and entries would vanish under the player.
   Rejected: including them for kill contracts, which can be revisited as
   its own change.
3. **The strip rides `map_pane`'s bottom border**, which CLAUDE.md names as
   the one border currently carrying nothing. Rejected: a row in the info
   column (the column does not scroll and `MAX_BAND_ROWS` already went
   4 -> 3 to pay for the need rows), and an arrow at the map's rim (no
   layout cost, but new drawing in `render/base.rs` and its own hue
   reservation).
4. **`map_pane` buys `strip_inset` unconditionally.** The rule is that a
   pane carrying a border strip starts its body at
   `render::hud::layout::strip_inset` and buys the height for it. Buying it
   only while a target is selected would resize the map the instant one is
   picked, which reads as a camera fault. A few pixels of map, always, in
   exchange for a stable viewport.

   **Decisions 3 and 4 amended together, 2026-09-05, one release after the
   feature shipped: the compass is a block *inside* the map pane's top-right
   corner, not a strip on any border.** Decision 3 read "the one border
   carrying nothing" as though the border were free. It is not: a strip's
   quad reaches into the pane, so the map has to buy a band it cannot draw
   tiles in — which is the whole of decision 4 — and that border also faces
   `log_pane`'s top border, where the vitals already reach up into the gap
   between them, so `pane_gap` had to grow to hold both. The rejected "arrow
   at the map's rim" was closer to right than it looked: an overlay inside
   the pane costs *no* layout at all, which makes decision 4's "a few pixels
   of map, always" unnecessary rather than merely cheap. The block draws the
   arrow, the name and the distance, and sits under the THREAT readout
   rather than beside it. `docs/seams.md` carries the argument in full.
5. **Hidden off the zone surface.** `Position` is pinned to the entrance
   tile in the Stack and to the anchor in base space, so a live bearing
   would be frozen while reading as live. `compass_targets` returns empty
   there and the strip is absent — the same shape `require_surface` already
   gives the actions that read the zone map. Rejected: showing it frozen,
   and showing it greyed with a note.
6. **The screen scrolls.** Towns accumulate across a run — the trog save
   holds six by tick 1763 — so a no-scroll page would need a row cap and
   would eventually hide a destination. `Mode::History` is the pattern;
   the memories page's no-scroll census is deliberately not followed.
7. **`u` opens it.** Free lowercase on the zone map is only `u`, `y` and
   `z`: `h/j/k/l` are movement, `w` is the wielded-program easter egg, and
   `a b c d e f g i m n o p q r s t v x` are taken. None is mnemonic.
8. **The feature surfaces what is recorded; it does not widen what gets
   recorded.** A town four regions out stays invisible. Changing
   `ensure_local_settlements`' 3x3 radius is a separate decision and is
   deliberately not folded in.

## The derivation

One door, `Game::compass_targets()`, in the shape `Game::attention` already
uses — a `Vec<Row>` derivation several surfaces read.

```rust
pub enum CompassTarget {          // save-stable; never an Entity
    Home,
    Town(SettlementKey),
    Link((i32, i32)),
}

pub struct CompassRow {
    pub target: CompassTarget,
    pub label: String,             // "Lowport", or "a settlement" unvisited
    pub bearing: &'static str,     // game::stack::bearing, already written
    pub distance: i32,             // answered for every row — see decision 1
    pub visited: bool,
}
```

`bearing(dx, dy)` (`game/stack.rs:63`) is reused as-is: eight-point, north
as `-y`, with a diagonal rule that refuses to call a near-45-degree bearing
due east. `announce_surface_links` (`stack.rs:256`) is the existing one-shot
precedent for the same sentence.

Decision 1 lands entirely in `label`, so the tiering is stated once in the
engine rather than by each surface.

`CompassTarget` keys by `SettlementKey` and by tile for `SettlementKey`'s
own stated reason: entity ids are not stable across a save.

Ordering is home, then settlements, then links, each group nearest-first —
`attention`'s rule, so "the first row" is stable across runs without a
second sort.

## What "visited" means

The only new bookkeeping, and it is one field.

- **Home** — always visited. It is the party's own.
- **Settlement** — a new `KnownSettlement::visited: bool` behind
  `#[serde(default)]`, written on the arm of `move_player` that already
  queues `resources::PendingVisit`. **Not** derived from
  `resources::Standings`: that resource's doc states it deliberately allows
  a standing with a town the party has not walked to, which routes need, so
  it is the wrong proxy.
- **Stack entrance** — no new field. `resources::FrameKey` is
  `((i32, i32), u32)` — entrance tile and depth — so a link is visited iff
  `resources::StackMemory` holds any key whose `.0` is that tile.
  Descending is what counts.

## The selection

`resources::CompassBearing(Option<CompassTarget>)`, serialized behind
`#[serde(default)]`.

A target that stops existing needs no cleanup hook: if the selection is not
among `compass_targets()`' rows — a link `Game::collapse_stack` took the
way out with — the derivation drops it and the strip goes blank.

Both save changes are additive behind `serde(default)`, so **no
`save::SAVE_FORMAT_VERSION` bump**.

## The screen

`Mode::Compass`. Rows are `compass_targets()`. Up/Down moves, Enter
selects, Esc closes, `X` clears the selection — uppercase, since lowercase
letters are row selectors.

Three censuses a new `Mode` must clear, none of which fail to compile:

- **`ALL_MODES`** in gui is hand-written and the draw match ends in
  `_ => {}`, so a missing entry ships a **blank screen** with a green suite.
- **`needs_status_banner`** in `render/mod.rs` (gui, not app-core).
- **`ALL_MODES`' length** is a semantic merge conflict: two branches each
  adding a variant merge cleanly in the entries and not in the count, and
  it breaks only on the merge commit.

## The block

A block in the map pane's top-right corner, under the THREAT readout
(amended — this was one line on `map_pane`'s bottom border for one
release; see decisions 3 and 4):

```
────────────────── THREAT clear · no defence ┐
                              ┌─────────────┐
                              │  ↓  Lowport │
                              │     219 tiles│
                              └─────────────┘
```

Absent when nothing is selected, and absent off the zone surface.

## Testing

Engine:

- Ordering is home, settlements, links, each nearest-first.
- An unvisited settlement yields a generic label and a figure; a visited
  one yields its name and the same figure.
- A selection naming a link that no longer exists is dropped.
- `compass_targets()` is empty in `Locale::Stack` and `Locale::Base`.
- **Seed 1788625345 is the regression fixture.** A fresh `Game::new` on it
  must list Lowport bearing south — `Game::new` already runs
  `ensure_local_settlements` over the 3x3 region block, which on that seed
  holds four towns at tick 0.
- Save: a RON round-trip **and** a save -> load test. A `#[serde(skip)]`
  slip leaves the round-trip green.

app-core: key handling, scrolling, and the selection surviving a mode
change.

gui: the `ALL_MODES` entry draws, the strip renders its two forms, and the
strip is absent off-surface. `map_pane`'s body height is unchanged between
"a target selected" and "none selected".

## What this does not do

- It does not widen `ensure_local_settlements`' radius, so a town four
  regions out stays invisible until walked near (decision 8).
- It does not route, pathfind or auto-travel. It gives a bearing and a
  number; the walking is the player's.
- It does not list nests (decision 2).
