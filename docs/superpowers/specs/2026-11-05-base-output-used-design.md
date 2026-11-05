# Base output: what used it

**Status:** Approved in chat, 2026-11-05.

## Intent

The base hub's Production → Output screen (`Mode::BaseOutput`) shows what the
base *made* per item — sector, machine, hand, run, sparkline — but nothing
about where those units *went*. The player wants to see demand next to
output: how much of each item was used, and what used it, so a chain that
makes 300 Raw Trace and burns 280 of them into Logic Gates reads as such.

## Decisions (the user's)

- **"Demand" means consumed totals**, from the ledger — not a live list of
  which structures/orders currently want the item.
- **The breakdown is an indented line under each item row**, not a new
  screen and not a selected-item detail block.

## Decisions (mine, shown to the user and accepted)

- A **`used` column** after `run`, the run (lifetime) total:
  `ItemTotals::consumed`. No sector figure for `used` — width.
- **One dim indented line under each item row** naming what used it,
  largest first, joined with ` · `, elided with `…` to fit the content rect:

  ```
    Raw Trace              140      312       0     312    268  ▂▄▆█▅▃▄▆
      ← into Logic Gate 180 · into Power Cell 70 · built 18
  ```

  - An assembler's inputs are attributed to the **product they went into**
    (`into <product name>`), because that is the structure/work-order chain
    the player reasons about.
  - Every other consumption is attributed to its `ConsumeSource`, worded
    for the player (fuel, built, base, craft, refit, install, breach,
    sabotage, defence, heal — the builder picks the exact words; "base" for
    the dig crew/sortie/research-bill bucket needs a plainer word).
  - **All sources are shown**, not only structure-driven ones.
  - An item with `consumed == 0` draws **no** indented line (no "← nothing").
- **`BASE_OUTPUT_MAX_ROWS` drops 5 → 3** (per section) if the doubled rows
  no longer fit; no scroll. `the_tallest_base_output_page_fits_its_content_rect`
  (`crates/gui/src/render/building/tests/base_output.rs`) is the gate and
  decides the final number — the largest that fits with every item carrying
  a usage line.
- **Items only ever consumed, never produced, stay off the page** (it lists
  `produced() > 0` today). Out of scope.

## The how

### Engine: the ledger learns why

Today `Event::Consume { item, qty }` drops its `ConsumeSource`; the source
reaches only the telemetry `Record`. `Event::Assemble` folds inputs into
`consumed` with no attribution.

- `Event::Consume` carries `source: ConsumeSource` (five emit sites:
  `game/telemetry.rs::note_consumed`, `game/base/stock.rs`,
  `game/base/construction.rs`, `game/base/research_supply.rs`,
  `systems.rs` fuel burn).
- `ItemTotals` gains a per-sink tally, `#[serde(default)]`, a `BTreeMap`
  (deterministic save encoding, `Stock`'s reason). The key is a new small
  enum, e.g. `UsedBy::{Assembled(ItemId), Source(ConsumeSource)}`, which
  needs `Ord` + serde; `ConsumeSource` gains `PartialOrd, Ord, Serialize,
  Deserialize`. Exact shape is the builder's.
- `fold` writes the tally in the same arms that bump `consumed`, so the
  breakdown sums to `consumed` for every event recorded after this ships.
- **Saves:** additive behind `serde(default)`, no `SAVE_FORMAT_VERSION`
  bump (seams-screens). An old save's `consumed` exceeds its tally sum;
  the page draws the tally only and does not invent an "earlier" bucket
  (builder may add one if trivial — note it in the plan).
- Buckets are untouched (no per-bucket breakdown).

### Engine: the view

`BaseOutputRow` gains `used: u32` and `used_by: Vec<(String, u32)>` —
labels resolved in the engine (product names via `ItemDb`, source words via
one function), sorted by qty desc then label, so gui only formats.
`Game::base_output_report` fills them.

### Gui

`base_output_rows` / `base_output_line` (`render/building/structure_sheet.rs`)
add the `used` column to header and line, and push the indented dim line
after each item with usage. Elision to the content rect width.

## Builder-check answers

- `used` is the whole `ItemTotals::consumed`, every sink — hand, sortie,
  heal and breach included. Every `ConsumeSource` variant gets a word.
- Each `ConsumeSource` keeps its own entry; no two share a word, so nothing
  merges. (My call.)
- The engine does not cap `used_by`; gui cuts the tail with `…` at the
  content-rect width and **never wraps** — exactly one indented line per
  item, which is what the row cap is sized against.
- Assembler inputs reach the ledger only through `Event::Assemble` (no
  `Consume` from the assembler path); a test pins "tally sums to
  `consumed`" so a double count fails loudly.
- The tallest-page fixture gives **every** item a usage line at the widest
  realistic breakdown, or it sizes the cap on the wrong data.
- An old save's `used` reading above its breakdown's sum is accepted as-is.

## Tests

- Ledger: a `Consume` with a source and an `Assemble` each land in the
  tally; tally sums to `consumed`; a save→load round trip keeps the tally
  (memory: a RON round trip cannot catch a skipped field); an old-format
  `ItemTotals` with no tally field still loads.
- Report: `base_output_report` orders `used_by` largest first and names an
  assembler's product.
- Gui: a row with usage draws the `used` figure and the indented line; a
  row without draws neither; the widest line fits; the tallest page fits.
- `balance_sim` untouched (no tuning change except possibly
  `BASE_OUTPUT_MAX_ROWS`, a layout constant) — run it anyway.

## Non-goals

- Live demand (which machines/orders currently want an item).
- Sector-scoped usage or a usage sparkline.
- Listing consume-only items.
- Telemetry changes beyond threading the source it already has.

## Docs

`CHANGELOG.md` entry at release. No asset schema change, no ADR (no `.ron`
field, no new pattern, no boundary move).
