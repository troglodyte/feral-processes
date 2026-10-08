# World map, surface fog, and pointing the compass from a map

**Date:** 2026-10-08

## Goal

A surface map the player opens to see the world they have explored, all at once:
- the places in it;
- how each town stands with them, and which way that town is heading;
- where their outposts and routes are;
- where trouble comes from.

The map serves three needs equally: deciding where to go next, feeling the
world change without the player, and choosing a destination.

The map **does not move the party**. Travel stays on the clock. The map's one
action is pointing the compass at a selected place.

## What the player said (decisions)

- **Diplomacy:** player-to-town standing only. There are no town-to-town
  relations and no factions.
- **Caravans:** only the player's own dispatched routes are drawn. No NPC
  traffic.
- **Growing or dwindling:** towns already grow and shrink, because footprint
  follows vitality. The map shows the *direction* of that change, which needs
  one stored snapshot per town.
- **Fog:** chunks are revealed where the party has been. The base area,
  outposts and route corridors are also revealed.
- **Rendering:** a chunk-scale view, one cell per 32×32 chunk, built by the
  engine in the shape of `Game::frame_map`.
- **Opening the map:** `g` opens it on the surface. Underground, `g` keeps
  opening the frame map.
- **Selecting a destination:** selecting a place and pressing `C` sets the
  compass. Outposts become compass targets. Nests stay map-only.

## Out of scope

- NPC caravans.
- Town-to-town relations and factions.
- Fast travel.
- Walk-to from the map.
- Towns dying or being founded.
- Free waypoints on arbitrary chunks.
- Zoom levels. One scale ships; more are possible later.
- Mouse input on the map.

## Engine

### Surface fog: `resources::ExploredChunks`

- **The resource:** `ExploredChunks(BTreeSet<(i32, i32)>)` holds chunk
  coordinates the party has been near. It is saved as a `#[serde(default)]`
  field on `SaveData`, with no `SAVE_FORMAT_VERSION` bump. An old save loads
  with an empty set.
- **What writes it:** the party's position marks every chunk within
  `WORLD_MAP_REVEAL_RADIUS_CHUNKS` (a new constant in `tuning.rs`).
  - It is written only on the surface. Base space and the Stack write nothing.
  - It is written where the party already moves on the surface. The plan names
    the hook; it must not be a per-tick scan over anything but the party.
- **Derived reveals are never stored.** The view computes these live:
  - the chunks around the base anchor;
  - the chunks around each outpost;
  - the chunks a dispatched route's corridor crosses.

  Storing them would duplicate state that `Outposts` and `Routes` already own.
  The cost: when an outpost is lost, its area stays revealed only where the
  party walked.

### Town trend: snapshot on the commerce epoch

- **The field:** `Relation` gains `commerce_at_epoch: i32`, marked
  `#[serde(default)]`.
- **Who writes it:** `settle_commerce_drift` (`game/settlement_growth.rs`),
  when it advances the epoch bookmark. It records the commerce held **before**
  paying that settlement's decay, so a town that has only decayed reads as
  falling.
  - Settling is lazy, so a town untouched for several epochs snapshots its
    value from the last time it was settled. It still reads as falling, which
    is the truth.
- **The pure function:** `settlements::growth::trend(now: i32, snapshot: i32)
  -> TownTrend { Rising, Flat, Falling }`. Its threshold, if one is needed, is
  a constant in `tuning.rs`.
- **Why a new enum:** this is its own enum, not `outposts::Trend`. That enum
  has a `Stale` state with outpost-only meaning.
- **Read-only view:** the view is `&self`, so it must not settle. It computes
  pending drift without writing. That calculation is a pure function shared
  with `settle_commerce_drift`; per CLAUDE.md, the two are not copies.
- **What the trend applies to:** Mainframes, alongside vitality. A Server
  shows its kind and "grows into a city" status instead, read from the
  existing due-date machinery. The plan confirms the exact wording against the
  no-tick-vocabulary rule.

### Compass: `CompassTarget::Outpost((i32, i32))`

- **The variant:** appended to the enum, so existing saves still parse.
- **The callers:** `Game::compass_targets` and the bearing resolution handle
  the new variant. The Compass screen lists outposts as a result.
- **When an outpost disappears:** a bearing on it resolves the way a
  collapsed `Link` already does. The plan reads that path and matches it.

### View: `Game::world_map(center_chunk, radius) -> Option<WorldMapView>`

The view is a read-only query. It returns `None` underground and in base
space. `WorldMapView` contains:

- **`cells`:** a square grid of `WorldMapCell::{Unknown,
  Explored(Biome)}`, one per chunk. The biome is the chunk's dominant surface
  biome, sampled cheaply. The sampling rule is decided in the plan, not by
  scanning 1024 tiles per cell.
- **`marks`:** each is `WorldMapMark { chunk, kind, label,
  target: Option<CompassTarget> }`. A mark appears only on a revealed chunk,
  whether that chunk was explored or revealed by a derived source. The kinds:

  | Kind | What it carries | Compass target |
  |---|---|---|
  | `Home` | — | `Home` |
  | `Town` | standing band, `SettlementKind`, vitality (Mainframe), `TownTrend` (Mainframe), and consequence flags read from the five named queries: raid source, patrols, preys on routes, refuses service | `Town(key)` |
  | `Outpost` | `outposts::Trend` | `Outpost(tile)` |
  | `StackLink` | — | `Link(tile)` |
  | `Nest` | — | none |

- **`routes`:** one polyline per dispatched route, with a per-segment flag
  where `settlements_near_route` finds a town that preys on routes.
- **`party`:** the party's own chunk.

Towns come from `resources::Settlements`, and the view never materializes
new ones. A town that exists but has never been met is not on the map.

## App-core

- **Mode:** a new `Mode::WorldMap` holds `center_chunk` and the selected
  index.
  - `g` in `Playing` opens it on the surface. Underground, `g` still opens
    `FrameMap`. The branch is the existing one at `app/playing.rs:670`.
  - Pressing `Esc` closes it.
- **Keys:**
  - Arrows pan by one chunk.
  - A key to re-centre on the party is chosen in the plan, from free
    lowercase-safe keys.
  - Lowercase letters select rows in the side list, following the rule that
    lowercase letters are row selectors. The list holds marks in view, nearest
    to the party first.
  - `C` sets `CompassBearing` to the selected mark's target through
    `Game::set_compass_bearing`.
- **Refusals:** a mark with no target (a nest) shows the action as
  unavailable. Its reason string follows the existing row-fragment
  convention for ability-unavailable strings.
- **No world access:** the mode reads only `Game::world_map` and the setter
  above.

## GUI

- **The file:** `render/world_map.rs`, drawing only through `Painter`, as
  the drawing-seam rule requires.
- **The map grid:**
  - Biome colour for explored cells; an unexplored look for `Unknown`.
  - A glyph per mark, with the town tinted by standing band.
  - Route lines, with predated segments marked.
  - The party marker.
  - The current compass target, highlighted.
- **The side list and detail panel:** the selected mark's standing,
  kind, vitality and trend (▲ ▬ ▼), consequence flags, and outpost trend.
- **The legend:** standing colours, trend arrows and mark glyphs.
- **Text:** follows the GUI rules that text never clips or wraps, and uses
  the map font.

## Content and docs

- **No new asset kinds.** Glyphs and colours follow existing render
  conventions.
- **`CHANGELOG.md`:** gets an entry when this lands.
- **Help:** a help page in `assets/help/` for the map key, if help pages
  list screens. The plan checks.

## Testing

**Engine:**
- Walking reveals chunks within the radius, and never in base space or the
  Stack.
- `ExploredChunks` survives save→load. This must be a real save→load, not a
  RON round-trip.
- Derived reveals (base, outposts, route corridor) appear without being
  stored.
- `trend` is unit-tested on its own.
- Trend across an epoch:
  - a decayed town reads `Falling`;
  - a town traded with above the decay reads `Rising`;
  - `commerce_at_epoch` survives save→load.
- The view's pending-drift calculation matches what `settle_commerce_drift`
  then writes. This is one function, asserted equal.
- Fog hides marks. A known town in an unrevealed chunk is absent.
- Route segments flag predation exactly where `settlements_near_route` says.
- A `CompassTarget::Outpost` bearing resolves, and survives save→load.

**App-core:**
- `g` opens `WorldMap` on the surface and `FrameMap` underground.
- `C` on a town sets `CompassBearing`; `C` on a nest refuses.
- Panning moves `center_chunk`.

**GUI:**
- A screenshot from a `dev-saves/` template with outposts and a route,
  read back by eye.

**Final gates:** `cargo test --workspace` and clippy `--all-targets`.
