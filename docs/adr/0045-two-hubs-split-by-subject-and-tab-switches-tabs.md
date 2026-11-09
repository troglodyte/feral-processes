# 0045 Two hubs split by subject, and Tab switches tabs

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-11-09
- Pattern: Strategy (`app::hub::HubKind`), as reshaped by 0043
- Depends on: [0043](0043-a-hub-rail-is-runtime-data-and-its-page-is-held.md), [0014](0014-per-screen-state-and-one-screen-table.md)

## Context

There are four game hubs on one frame: the base report (`b`), the inventory (`i`), the structure hub (`c`) and the Sites hub (`w`), plus Esc's game menu.
Which hub holds a page has followed history rather than a rule:

- `b` mixes report pages with four **launcher pages** (Build, Research, Trade, Compile). These are the old base menu's verb lists, and each row opens some other screen. Contracts sits under Trade because it needed a bucket.
- One subject is split across rail entries: Staff / Duties / Staffing, and Build / Build orders.
- Contracts held from a town show on that town's Jobs tab in `w`, but the Broker's board is under `b` → Trade.
- `c` cycles adjacent structures with `Tab`, which is a page switch under a different key.
- `←/→` does three jobs (`App::handle_hub_nav`, `App::screen_claims`): it switches screens, steps back to the rail, and on some screens moves a cursor (Duties columns, level rows). On the Duties table the key's meaning changes partway across.
- A page's screens are anonymous dots (`frame::draw_title`), so a five-screen page cannot say which screen is showing.

## Decision

**Two hubs, split by subject: `b` is the operation, `i` is one thing. Arrows move the cursor, and `Tab` switches tabs, in every hub.**

- **The rail lists nouns, tabs are aspects of the highlighted noun, and actions are keys on a tab.** No launcher pages: a verb lives on the tab of the noun it acts on.
- **`b` holds everything not about one entity**, in sections: BASE (Overview, Build, Production, Stock, Staff, Research, Breed, Alerts, each with its former sibling pages as tabs), JOBS (Contracts), TOWNS, OUTPOSTS, TRAFFIC (Caravans). The Sites hub folds into it.
- **`i` holds everything about one entity**, in sections: YOU, PARTY (one entry per party member), PROGRAMS (base staff as one Roster entry, plus Downed), BESIDE YOU (one entry per adjacent structure, with the structure hub's pages as its tabs). The structure hub folds into it.
- **`b` opens anywhere** and `i` opens anywhere it does today. A page is readable anywhere, and an action refuses in its own door's words when out of reach. A subject is hidden only when it does not exist, never because of where the party stands.
- **`w` and `c` stay as shortcuts** that open `b` on the Sites section (or the site beside the party) and `i` on the structure beside the party.
- **The nav rule** (one rule for every hub, still `App::handle_hub_nav`):
  - `↑/↓` move on the rail or among the page's rows.
  - `→` or Enter on the rail enters the page.
  - Inside the page, `←/→` are the page's own: a column, a level, or nothing. `←` from the page's leftmost position returns to the rail.
  - `Tab` / `Shift+Tab` switch the highlighted entry's tabs, from the rail or the page.
  - `[ ]` step the rail from anywhere. `{ }` jump to the next or previous rail section.
  - Esc in the page returns to the rail, and Esc on the rail closes the hub.
- **Tabs are named.** The title row draws the page's tab names with the current one marked, and falls back to the dots only when the names do not fit.
- **Vocabulary:** a rail entry is a *page* in code and in the keybar's `[ ] page`; a page's sub-view is a *tab* in code and on screen. "Screen" retires as a name for it.

## Alternatives considered

- **Keep four hubs and move only Contracts.** Fixes one misplacement and keeps the rule that caused it.
- **`Tab` steps the rail, `←/→` switch tabs.** Keeps today's conflict on the Duties table and level rows. The user chose arrows for the cursor and `Tab` for tabs (2026-11-09).
- **`[ ]` switch tabs.** Frees `Tab`, but leaves the rail without a page-step key and makes `{ }` the only fast way along it.
- **Remove the rail cursor (arrows never reach the rail).** The user likes `←` stepping back to the rail.

## Consequences

- `b` loses its launcher pages, and every verb they listed needs a destination tab. The spec carries the table.
- The Sites spec's "opens only at base" rule (user's decision, 2026-11-06) is reversed for the merged hub (user confirmed, 2026-11-09). The doors keep their own reach checks, so nothing becomes doable from further away.
- `screen_claims` shrinks: `Left` no longer has to be claimed against a tab switch, only against the rail step.
- `GameKey` gains `ShiftTab`, and `{`/`}` become hub keys.
- `shared_screens_are_pinned` and `hub_census` change with the rails. The seams-screens hub rule gains the nav clauses. Update the rules file, the `seams` skill and the `seam:` entry.

## Evidence

`app::hub::{HubKind, HubPage, Hub}`, `App::{handle_hub_nav, screen_claims, open_hub, enter_hub_at}`, `app::base_report::BASE_REPORT_PAGES`, `app::group_menu::{TRADE_ROWS, command_rows}`, `app::sites_hub::{SitesHub, TOWN_TABS}`, `app::structure_hub`, `app::inventory_hub::INVENTORY_PAGES`, `render/base_report/frame.rs` (`draw_title`, `keybar_groups`), `tests/hub_census.rs`.
