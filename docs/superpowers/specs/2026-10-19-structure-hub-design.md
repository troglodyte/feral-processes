# Structure hub: `c` opens one screen per structure

**Status:** design, awaiting approval.

## Intent

Interacting with a structure through `c` is too many steps. Beside a machine
the first `c` asks *which side* (`Mode::InteractDirection`, a one-line prompt
listing nothing) and a second `c` opens the cargo; the rig's tool, the Power
Siphon and the Splice Rig each hide behind their own map letter (`F`, `P`,
`I`). The player wants one door: **`c` opens a hub for the structure beside
you, in the same frame `b` uses** — rail of pages on the left, the page on
the right — and everything you can do *with* that structure is a page in it.

## Goals

1. `c` beside one or more structures opens the **structure hub** directly. No
   direction prompt, no second `c`.
2. The hub shows **one structure at a time**. With several beside you it opens
   on the **first clockwise from north** (N, E, S, W — `(0,-1)`, `(1,0)`,
   `(0,1)`, `(-1,0)`; my reading of "clockwise", the user did not name the
   start). `Tab` steps to the next clockwise, wrapping, without leaving the
   hub; `Shift+Tab` back is the builder's call.
3. **Every interaction with a structure starts with `c`.** The pages:

   | Structure | Pages (rail order) |
   |---|---|
   | Depot | Inventory, Filter |
   | Rack (downed-program shelf) | Inventory (its shelf) |
   | Machine (has a recipe) | Inventory, Recipe |
   | Teardown Rig | Inventory (if it holds stock), Tool |
   | Power Siphon | Siphon |
   | Splice Rig | Splice |
   | Farm plot | Grow |
   | Rebuild site | Rebuild |
   | Index Terminal | Inventory (see below) |
   | Any structure *already in the cycle* | Info (last) |

   The list is driven by what the structure is, not by a per-kind table in
   app-core: a structure with a shelf gets Inventory, one with `strips` gets
   Tool, and so on, so a modded structure combining capabilities gets the
   union. Info is the existing `StructureManifest` view, read-only.
   **A structure with nothing but Info is not in the cycle** (the user's
   choice): a wall, a light or a Shield never opens the hub, never counts in
   `Tab` or the title's "n of m", and beside only such things `c` refuses as
   today.
4. **Inventory is that structure's own stock only** — the user's explicit
   choice. The pooled offer over every adjacent shelf goes away. Controls are
   today's basket (arrow toward the column it points at, Shift/Ctrl modifiers,
   one commit); the put budget is that one structure's room.
   - **Index Terminal** (my call): its own stock *is* the whole base's Depots,
     since reaching them is what the terminal is for. Its Inventory page is
     today's terminal-wide offer.
5. The hub **opens on the first page in rail order** (Inventory where there
   is one), cursor in the page, not on the rail — so a Depot's stock is one
   keypress. It does not restore a remembered page (`hub_last`) — my call;
   the structure differs from one visit to the next.
6. Opening the hub, switching structure and turning pages spend no turn, as
   opening the transfer screen does today.
7. **Removed:** `Mode::InteractDirection` and its prompt, the `c`-again-for-
   cargo rule, and the map keys `F` (rig tool), `P` (siphon) and `I` (splice
   rig). No shim, no alias.

## Non-goals (the user's decisions)

- Demolish (`d`) and Upgrade stay where they are.
- Work-it-yourself and a Bay's Breed stay where they are; the hub does not
  offer them.
- The `b` hub, its pages and the Inventory/Party hubs are unchanged except
  for whatever the frame needs to host a fourth hub.
- `x` inspect stays (it is ranged and also inspects programs).
- Trade (`t`, radius list) is not a structure-beside-you interaction; it
  stays.

## Anti-goals

- A hub that needs a page to be picked before anything is shown — the point
  is fewer presses.
- A per-structure inventory that silently still pools (a Depot page showing
  its neighbour's stock reads as a bug).
- Two homes for one screen: the recipe/filter/tool/siphon/splice screens move
  *into* the hub; they are not duplicated beside it.

## Behaviour details

- **Beside** means the four orthogonal neighbours (`collect::ORTHOGONAL`),
  as every current `c`/`F`/`P`/`I` lookup already uses. A structure touching
  several of them (wide footprint) appears **once**, ordered by the first
  side clockwise from north it occupies (my call).
- The rig tool, siphon and splice screens are reachable today *only* by
  adjacency (the map keys), so embedding them in an adjacency hub loses no
  remote use.

- **Nothing beside you:** `c` refuses through `game.refuse_transfer()` as
  now. **Only pack downed programs, no neighbour** (`dir == (0,0)`): `c`
  opens what it opens today (my call — not a structure, so not a hub).
- **Hub title** names the structure and its position in the cycle, e.g.
  `Depot — east (2 of 3)`; exact wording is the builder's, within
  `gui-text-never-clips-or-wraps`.
- **Esc:** from a page → rail, from the rail → map, as `b` does. A structure
  screen's own Esc meaning (a confirm popup, a sub-list) still wins first.
- **Walking away / the structure going:** today the siphon and splice
  screens close themselves when no longer adjacent. The hub does the same
  for any structure: if the current one leaves the adjacent set, step to the
  next or close if none.
- **Tab or a page turn resets/leaves an uncommitted basket** the way Esc on
  the transfer screen does today (`App::leave_basket` discards it) — my call.
  `Tab` lands on the next structure's **first** page, per goal 5.
- **A Rack's Inventory page** keeps the pack side: downed programs you carry
  can be put on that rack, as the pooled screen allowed (my call).
- **A one-page structure still shows the rail**, for one layout; Esc goes
  page → rail → map (my call).
- **The outpost's transfer** (`TransferSource::Outpost`, from
  `Mode::OutpostVisit`) is not a structure and is unchanged.
- **Arrow keys conflict:** the `b` hub's in-page Left/Right steps a page's
  screens and Left off the first returns to the rail. Inventory spends
  Left/Right on moving stock. In the structure hub each page is one screen
  and owns its arrows; the rail is reached by Esc (and `[`/`]` turn pages
  from anywhere). The builder settles how `handle_hub_nav` expresses that —
  a per-hub rule, not a per-screen special case.
- `Tab` already means "next structure of this kind" on the recipe, depot
  filter and rig tool screens. Inside the hub `Tab` is "next structure" of
  *any* kind, and those per-screen Tab cycles are removed.

## Constraints

- Drawing seam: only `crates/gui/src/paint.rs` names a graphics library;
  the hub reuses `render/base_report/frame.rs`'s `paint_frame`.
- `Game` is the renderer's whole API; the per-structure offer and the
  structure-to-pages answer are `Game` methods (engine), not app-core
  reaching into components.
- Seams that change and must be rewritten in all three places (graph, skill,
  `.claude/rules/seams-base.md`):
  - "`[c]` is the interact dispatcher" — still true; `InteractionKind`
    grows (rig, siphon, splice) or becomes per-structure.
  - "Taking and putting are one screen, one basket and one commit —
    `Mode::Transfer`, opened with `c`" — now per structure.
  - "The put side is one budget…" — the budget is one structure's room.
  - "The rig's tool screen is `[F]`" — now a hub page.
- `HubPage.screens` is a static `&[Mode]`; the structure hub's pages are
  per-structure. How the frame takes a dynamic page list is the builder's
  call (run the `design-patterns` dialog).
- Docs in the same change: `assets/help/20-controls.md`, `60-your-base.md`,
  `62-production-lines.md`, `64-implants.md`, `75-extraction.md`, the
  `description` in `assets/structures/power_siphon.ron` and
  `teardown_rig.ron` (they say "press P"/"press F"), and `CHANGELOG.md`.
  Not the manual, not the README.
- No save-format change expected.

## Left to the builder

- Whether `InteractionKind` becomes a structure list with derived pages or
  grows variants.
- The per-structure transfer offer's shape. A machine's Inventory carries
  over today's take/put rules for a machine shelf unchanged, scoped to that
  one machine.
- Hub title text, rail labels, key-bar text.
- Test reshaping: `app-core/src/tests/interact.rs`, `transfer.rs`,
  `depot_filter.rs`, `machine_recipe.rs`, `extraction.rs`, `siphon.rs`,
  `splice_rig.rs` press the removed keys and the two-`c` flow.
