# 0043 A hub rail can be keyed by something other than its mode

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-11-06
- Pattern: Strategy (extends `app::hub::HubKind`)
- Depends on: [0024](0024-code-is-organized-by-domain.md), [0014](0014-per-screen-state-and-one-screen-table.md)

## Context

The Sites hub ([spec](../superpowers/specs/2026-11-06-sites-hub-design.md)) wants a rail with one entry per known town, city and outpost: a glyph, the site's name, section headers, and a `!` per site.
Every town page has the same tabs, so every town's Status tab is the same `Mode`.

The hub framework cannot draw that today:

- `HubPage::title` is `&'static str`, so a site's name cannot be a rail entry.
- A rail row has no glyph and the rail has no section headers.
- `App::hub_for` finds the current page by searching the pages for the current `Mode` (`find_in`).
  With one `Mode` shared by many pages, every town resolves to the first town.

`StructureHubKind` meets "many instances" the other way round: its rail is one structure's pages, and `Tab` steps to the next structure.
The user chose the mockup's per-site rail over that shape and over a fixed rail with a site list.

## Decision

Add two methods with defaults to `HubKind`. Every existing hub keeps the defaults and is unchanged.

1. **`rail(&self, app) -> Vec<RailRow>`**, where `RailRow` holds an owned `title: String`, an optional `glyph: char`, and an optional `section: &'static str` drawn as a header above the row.
   By default it maps `pages()` to their static titles.
   gui's `HubFrame` and `frame::paint_frame` draw the rail from `rail()` instead of `HubPage::title`.
2. **`locate(&self, app, mode) -> Option<(page, screen)>`**: which page and screen `mode` is showing in this hub.
   By default it is today's `find_in` over `pages()`.
   `App::hub_for` calls it instead of `find_in`.

`pages()` keeps returning `&'static HubPage`.
The Sites hub returns the same static `TOWN_PAGE`, `OUTPOST_PAGE` or `CARAVANS_PAGE` once per site, and the per-site title comes from `rail()`.

The Sites hub holds which site is selected in its own app-core state (`App::sites_selected`).
It stores a stable site key (`SettlementKey` or outpost tile), never an index, because the rail grows as sites are found.
Its `locate` answers from that key, and its `turn` writes the key before setting the mode.
`handle_hub_nav` is unchanged: it already moves pages through `turn` and screens through `pages()[page].screens`.

## Alternatives considered

- **Owned `HubPage` and a stored page index for every hub.** It does the same job but rewrites all four existing hubs and their tests for a capability only one hub needs.
- **A bespoke Sites screen drawing its own rail.** It breaks the seam that every hub walks `App::handle_hub_nav` (`.claude/rules/seams-screens.md`).
- **The structure-hub shape** (rail = tabs, `Tab` cycles sites) or **a fixed rail with a site list.** Both need no framework change; the user chose the per-site rail instead.

## Consequences

- A hub can now have many pages that share screens.
  The Sites hub is the first; the structure hub could later list its structures the same way, but this record does not propose that.
- `hub_for` is answered through each kind's `locate`, so a hub whose `locate` is wrong mis-draws only itself.
  A census test asserts that every default-`locate` hub still resolves every `Mode` in its table to the page it did before.
- The rail's row width now comes from data (site names).
  The rail's 22-cell column needs elision plus a width census, per the rule that gui text never clips or wraps.

## Evidence

`app::hub::{HubPage, HubKind, find_in}`, `App::hub_for`, `App::handle_hub_nav`, `app::structure_hub::StructureHubKind`, `crates/gui/src/render/base_report/mod.rs` (`HubFrame::of`), `render/base_report/frame.rs` (`paint_frame`, `RAIL_CELLS`).
