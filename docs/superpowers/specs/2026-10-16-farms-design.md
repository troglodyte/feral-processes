# Farms

**Status:** design, approved in conversation 2026-10-16; not yet planned.

## Intent

Dwarf Fortress-style farming on the base. The player marks a plot, gives it
a recipe, and base staff tend it without further orders: they prepare each
tile, it grows through visible stages, and when ripe staff harvest it and
leave the product on the floor. The plot then replants itself, forever,
until unmarked.

The first crop is the Power Cell — the same `power_cell` item the Power
Conduit makes, produced a second way. Lore: staff **dimple phase space** on
a tile, which starts a cell growing. Nothing is consumed; planting costs
only staff time.

Farms are deliberately unlimited. The Conduit may be retired later because
of them; that is not this change.

## Goals

- Mark plots with the existing box tool, inside the `[m]` Excavation plan.
- Set a plot's recipe by walking up to it and pressing `[c]`.
- Staff dimple, then (when ripe) harvest, each tile with no player order —
  harvest is generated automatically.
- Growth is visible: the crop's icon is drawn larger at each stage.
- Harvest sets 1 product item per tile down from that tile (`Game::set_down`:
  on the tile when free, else the nearest free floor, else the pack).
- Crops are data (`assets/crops/`), so mods can add them.
- Plots, recipes and per-tile growth survive save/load.

## Non-goals

- Hauling harvest to stockpiles. The end state here is a Power Cell on the
  floor; idle programs tidying it onto a stockpile zone (0.18.36) is fine,
  but not something this design relies on or changes.
- Removing or rebalancing the Power Conduit.
- Any cost, cap, input item, fertility, light, season or soil rule.
- Crops other than power cells shipping now (the schema allows them).

## Anti-goals

- A farm that needs player babysitting each cycle.
- Content hardcoded in Rust (crop item, timings, glyphs belong in the
  crop file; only scheduling constants go in `tuning.rs`).
- Farm work stealing staff from production, building or digging.

## Decided behaviour

**Marking.** In `Mode::Excavate` a toggle switches the box tool to "farm
plot" (alongside the existing `[F]` floor-finish brush; exact key is the
builder's, uppercase per the action-key convention). Same cursor, anchor,
commit and Esc behaviour as digs/floors (`app-core/src/app/excavate.rs`,
`Game::toggle_mark_box`). The anchor cell decides mark vs clear, as now.

- Eligible tiles: any open, walkable base floor tile with nothing standing
  on it. Ineligible tiles in a box are skipped, not refused wholesale.
- **One committed box is one plot.** It carries one recipe for all its
  tiles. (Overlapping/adjacent boxes: see builder's calls.)
- Unmarking tiles (clearing a box over them) removes them from their plot;
  anything growing there is lost.

**Recipe.** A new plot has no recipe and staff ignore it. Standing
orthogonally beside any tile of a plot, `[c]` (`Game::adjacent_interactions`,
`crates/engine/src/game/base/interact.rs`) offers the plot as an
interaction; choosing it opens a picker of crops from `assets/crops/`.
Picking sets the recipe for the whole plot. Changing the recipe on a
growing plot discards all growth; every tile returns to needing dimpling.

**Per-tile cycle.** `Fallow` (needs dimpling) → staff dimple (labour time)
→ `Growing` through N stages over the crop's grow time → `Ripe` (needs
harvesting) → staff harvest (labour time) → 1 product item set down from
the tile (`Game::set_down`, `game/base/floor.rs`) → back to
`Fallow`. Growth advances on its own; it does not need a worker present.

**Labour.** Dimple and harvest are new staff tasks, scheduled through
`Game::schedule_base_labour` (`game/base/work_orders.rs`) as a
`farm_wants` list appended **after** `dig_wants` — lowest priority, so it
never starves production (see `.claude/rules/seams-base.md` and the
`seams` skill before touching the scheduler). Execution modelled on
`run_dig_crew` (a `&mut Game` crew step). Unreachable tiles drop out like
unreachable dig wants. "Auto-generated work order" means a ripe tile
becomes a harvest want by itself — it does not need to appear in the
player's `WorkOrders` queue.

**Visual.** Each growing tile draws the crop's icon at a size that grows
with stage (about four stages, same icon scaled — my call). Fallow plot
tiles get a quiet plot marking so the plot's extent is visible; ripe tiles
should read as ready. Engine exposes a view list (pattern: `floor_items`,
`marked_cells`); `render/base.rs` draws it through `Painter` only
(`.claude/rules/drawing-seam.md`).

## Data: `assets/crops/`

One `.ron` per crop, loaded by a `CropDb` following the `*Db::load_dir`
pattern (malformed file → logged warning, skipped). New
`assets/crops/README.md` documents the schema. Fields, roughly:

- `id`, `name`, short description for the picker
- `produces`: item id (validated against the item DB at load)
- `grow_ticks`: total growing time
- `stages`: number of visible stages (or a list of per-stage scale/glyph)
- `glyph`/sprite reference for the icon

New fields later get `#[serde(default)]`. First file:
`assets/crops/power_cell.ron` producing `power_cell`.

**Grow time (my call):** a background pace — far slower than the
Conduit's 6 ticks per cell; pick a value comparable to a few in-game days
of base time. No player-facing tick vocabulary anywhere.

## Save format

Plots (tiles, recipe) and per-tile state/progress are saved. New save
fields use `#[serde(default)]` so older saves load with no farms. Needs a
real save→load test (a RON round-trip cannot catch a skipped field). Bump
`SAVE_FORMAT_VERSION` only if a save would stop loading.

## Calls I made (for the user to check)

- Plot marking lives inside `[m]`, not a new map key (user chose this).
- Farm labour is lowest priority, below digging.
- ~4 stages, one icon drawn at growing sizes.
- Grow time is data, set slow; exact value tunable.
- Partially-invalid boxes mark the valid tiles only.

## Settled after the builder check (my calls)

- **Floor items never block a plot.** A floor item on a plot tile does not
  stop dimpling or growth. A tile holds one floor item (0.18.36), so a ripe
  tile already holding one sets its harvest down on the nearest free floor
  and leaves the item untouched. "Nothing standing on it" means no
  structure/build site — a floor item is not standing. On the map a floor
  item's glyph claims the cell from the crop's; the plot outline and ripe
  edge still draw.
- **Stockpile zones and plots are independent marks** and may overlap;
  neither box clears the other.
- **Clock:** growth runs on the same engine clock as dig/build crews —
  whenever base labour is simulated. No catch-up or offline growth beyond
  what that clock already does.
- **Staff:** farm wants go to the same pool as dig wants via the existing
  matching; being appended last means they only get workers no earlier want
  claimed. No role filter. Test: a base with production + dig + farm wants
  and too few workers leaves farm wants unserved.
- **Other marks:** plot tiles are open floor, so dig marks (walls) cannot
  overlap. A floor-finish mark may coexist with a plot tile; the two are
  independent. Building onto a plot tile removes that tile from its plot.
- **Missing crop on load** (mod removed/broken): the plot loads with no
  recipe (staff ignore it, `[c]` lets the player pick again) and a warning
  is logged. Never a load failure.
- **Icon scaling:** crop icon is a map glyph string drawn with
  `Painter::map(glyph, x, y, size, color)`, varying `size` per stage — no
  `paint.rs` change needed. Schema field is a glyph string (sprite support
  can come later with `#[serde(default)]`).
- **Mid-labour loss:** if a tile leaves its plot or the recipe changes
  while a worker is dimpling/harvesting it, that labour is dropped; the
  crew step must tolerate its target vanishing (as dig crews do when a
  mark is cleared).

## Left to the builder

- The plot-toggle key in Excavate mode and how the mode shows it's active.
- Component shape (a `FarmPlot` entity owning tiles vs per-tile component
  with a plot id), and whether to extend `DigSite` or keep separate.
- How adjacent/overlapping new boxes interact with existing plots (merge,
  split, or new plot claims tiles) — pick the simplest that keeps "one box
  = one plot" true for the common case.
- Dimple/harvest labour durations (named constants in `tuning.rs`).
- Whether a Farm page joins the base report's order screens.
- Notifications (none required).
- What happens if something is built onto a plot tile.
