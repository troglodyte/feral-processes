# 0043 A hub's rail is runtime data and its page is held, not derived

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user (the user chose this shape over a two-hook extension, 2026-11-06)
- Date: 2026-11-06
- Pattern: Strategy (`app::hub::HubKind`), reshaped
- Depends on: [0024](0024-code-is-organized-by-domain.md), [0014](0014-per-screen-state-and-one-screen-table.md)

## Context

The hub framework (`crates/app-core/src/app/hub.rs`, drawn by gui's `render/base_report/`) is the rail-and-pages UI behind the base report (`b`), the inventory (`i`), the structure hub (`c`) and the game menu (Esc).
It makes two assumptions:

- **The rail is code.** `HubPage { title: &'static str, screens: &'static [Mode] }`, so a rail entry cannot carry a runtime name, a glyph or a section header.
- **The page is derived from the mode.** `App::hub_for` finds the current page by searching the pages for the current `Mode` (`find_in`), so two pages cannot share a screen.

Both hold for the four hubs that exist.
Both break for the Sites hub ([spec](../superpowers/specs/2026-11-06-sites-hub-design.md)), which wants one rail entry per known town, city and outpost, all sharing the same tab screens.
They also explain the existing workarounds: the structure hub cycles structures with `Tab` instead of listing them, and the base report's Breed page is a conditional entry in a static table.

## Decision

**A rail is data the hub builds at runtime, and which page is showing is held in `App` state, for every hub.**

- **`HubPage` becomes owned rail data:** a stable `key`, a `title: String`, an optional `glyph`, an optional `section` header drawn above it, the `screens: &'static [Mode]` its tabs step through, and its `!` mark.
  The separate `HubKind::marks` folds into the page.
- **Page identity is a key, not an index or a mode.**
  `HubPageKey` is a small owned value the hub chooses: a fixed hub uses a name per page; the Sites hub uses the site's `SettlementKey` or outpost tile.
  A key survives the rail growing or shrinking, which an index does not.
- **`App` holds where the player is in a hub:** which hub is open, its page key, and (as now) whether the cursor is on the rail.
  `HubKind::turn` writes it, `App::open_hub` restores it, and leaving the hub clears it.
  `App::hub_last` remembers a page key per hub instead of a `Mode`.
- **The held state is checked, never trusted blind.**
  `App::hub_at` answers from the held state only while the current `Mode` is one of the held page's `screens`, and returns `None` otherwise.
  A screen that leaves the hub by setting `mode` directly therefore falls out of the hub, exactly as it does today.
  A key that has left the rail is forgotten on reopen, as a remembered screen is today.
- **`HubKind::pages(&App) -> Vec<HubPage>`** is the only rail source.
  `table()` and the free `hub_of(mode)` stay only if a caller still needs a lookup without an `App`; otherwise they are deleted rather than kept as a second path.
- **gui** draws the rail from the owned pages: glyph, title elided to the rail's width, section headers, and `!`.
  gui still names no hub.
- **`handle_hub_nav` keeps its rule** (rail Up/Down, Right/Enter into the screen, Left/Right between tabs, `[` `]`, Esc, `screen_claims`).
  It reads pages and the held key instead of searching by mode.

**Migration comes first, as its own step:** the four existing hubs move onto this model with no visible change, pinned by their existing tests plus a census that every hub's rail draws as it did before.
The Sites hub is built on top afterwards.

## Alternatives considered

- **Two hooks with defaults (`rail()`, `locate()`), existing hubs untouched.** Cheaper now, but it leaves two answers to "which page am I on": derived from the mode for four hubs, held for one.
  Every later dynamic hub writes its own workaround, and the two can drift.
  Turned down by the user in favour of one model.
- **A bespoke Sites screen drawing its own rail.** It breaks the seam that every hub walks `App::handle_hub_nav` (`.claude/rules/seams-screens.md`).
- **The structure-hub shape** (rail = tabs, `Tab` cycles sites) or **a fixed rail with a site list.** No framework change, but the user chose the per-site rail.

## Consequences

- Any hub can list many things that share screens.
  Likely later users, not proposed here: the structure hub listing every structure, the inventory listing companions, rails of sorties, nests or contracts.
- The hub-membership rule changes from "this mode is in a page's table" to "this hub is open, and this mode is one of its page's screens".
  `App::hub_for` for popups over a hub screen (`base_popup_parent`) moves to the held state. The plan must check each caller of `hub_for` / `hub_of`.
- Modes the structure hub borrows (`Transfer`, `BuildProgram`) count as in the hub only while it is open on a page offering them, which is the same answer as today, now held explicitly.
- A rail title can be any length: elision plus a width census, per the rule that gui text never clips or wraps.
- The seams-screens rule on hubs gains a clause: the page is held in `App`, keyed, and checked against the mode. Update the rules file, the `seams` skill and the `seam:` graph entry when this lands.

## Evidence

`app::hub::{HubPage, HubKind, Hub, hub_of, find_in}`, `App::{hub_at, hub_for, open_hub, handle_hub_nav, screen_claims, hub_last, hub_on_rail}`, `app::base_report::BASE_REPORT_PAGES`, `app::structure_hub::StructureHubKind`, `app::game_menu::GameMenu`, `app::inventory_hub`, `crates/gui/src/render/base_report/mod.rs` (`HubFrame::of`), `render/base_report/frame.rs` (`paint_frame`, `RAIL_CELLS`).
