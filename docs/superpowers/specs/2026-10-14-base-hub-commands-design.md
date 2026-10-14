# Base hub commands: the base menu folds into the base report

**Status:** design approved in chat 2026-10-14; spec awaiting review.

## Goal

One window for the base. Today `b` opens a flat menu of fourteen commands
(plus a "Base report" row) and `B` opens the base report hub, a rail of
read-only pages. After this change `b` opens the hub, the commands live on
four new rail pages in it, and `B` and `Mode::BaseMenu` are gone.

Success: every screen the base menu reaches today is reachable from a hub
command page, under the same locality and availability rules, and Esc out of
any of them lands back on the page it was opened from.

## Decisions

| Question | Decision |
|---|---|
| Shape | Four grouped command pages on the rail, not one page, not scattered onto report pages |
| Key | `b` opens the hub (where it was last left, Overview the first time). `B` is retired — no alias, per `group_menu.rs`'s rule |
| Illegal rows | Hidden dynamically, as now. A page with every row hidden still sits on the rail and draws one line saying nothing here is available |
| Party menu | `p` unchanged; `PARTY_ROWS` and its handler stay |

## Pages

Rail order, the four command pages directly after Overview:

1. Overview
2. **Build** — Deploy a structure, Work a structure yourself, Upgrade a structure, Demolish a structure
3. **Research** — Research, Routine research, Affix research, Study a program, Hold a program
4. **Trade** — Contracts, Caravan, Dispatch
5. **Compile** — Compile an item, Recipes
6. Staff, Production, Stock, Work orders, Staffing, Build orders, Alerts (unchanged)

Each command page is one screen. Every `GroupEntry` moves verbatim — label,
target, `Locality`, `available` closure and its comment — from `BASE_ROWS`
into one of four tables. The "Base report" row is deleted: it has no menu
left to sit in.

## app-core

- `Mode`: add `BaseBuild`, `BaseResearch`, `BaseTrade`, `BaseCompile`
  (hub screens); delete `Mode::BaseMenu` and every arm naming it.
- `BASE_REPORT_PAGES` gains the four pages at the positions above.
- `group_menu.rs`: `BASE_ROWS` splits into `BUILD_ROWS`, `RESEARCH_ROWS`,
  `TRADE_ROWS`, `COMPILE_ROWS`. `base_menu_rows` is replaced by one public
  `command_rows(mode) -> Vec<GroupMenuRow>` that picks the table for a
  command-page mode and runs it through the existing `group_rows`. It stays
  the **only** source of a command page's rows: the handler, the renderer and
  the Overview count all call it (the `seams-screens` rule on group menus).
- Selecting a row: the dispatch currently in `handle_group_menu_key` (the
  `Perks` / `Manifest` / `Build` / `BaseOverview` special cases) is shared by
  the party menu and the command pages, so it is extracted into one function
  both call, not copied. `menu_origin` is set to the command page's mode.
- Return path: Esc from a command's screen goes through `close_screen` to
  `menu_origin`, the command page. The hub cursor must be **in the page**
  (`hub_on_rail = false`) on that return, not on the rail, so the highlight is
  on the row just used. Esc on the page itself goes to the rail (existing
  `handle_base_report_nav` behaviour).
- Completing an action still drops to the map and clears `menu_origin`
  (`App::handle_key`), unchanged.
- `base_report_last` records command pages like any other hub screen, so `b`
  reopens on the last command page used.
- Up/Down/Enter/row letters on a command page go to its row cursor via
  `selected_index`, as the base menu's did; the hub nav already lets those
  fall through when the cursor is in the page.
- Overview: a command page's line is its available-row count
  ("3 available") or "none here", read from `command_rows`.
- `playing.rs`: `b` calls `open_base_report`, placed where `B` is now (before
  the underground hand-off, so it opens from the Stack); the `B` arm is
  deleted. `inspection.rs`'s own `B` (breeding on a bay's sheet) is a
  different screen and unaffected.

## gui

- `render/base_report/`: one command-list screen drawn in the hub's content
  rect from `command_rows`, reusing the group menu's row drawing rather than a
  second copy of it. The empty-page line is drawn here.
- `render/mod.rs`: delete the `Mode::BaseMenu` arms; the base menu's refusal
  line moves with it onto the command screen.
- Overview drawing picks up the four new lines from the same table.
- Only `paint.rs` names a graphics library (drawing seam); nothing here
  changes that.

## Text

- `assets/help/20-controls.md`: the `b` and `B` lines become one `b` line
  describing the hub with its command pages.
- `assets/help/60-your-base.md`, `63-staff-morale.md`, `65-contracts.md`:
  "base menu" and "B" references point at `b` and the right page.
- `assets/contracts/tutorial_break_ground.ron`, `tutorial_arm_a_decompile.ron`,
  `tutorial_man_the_node.ron`: same rewrite. These already say "Tab to the
  structure roster", which 0.18.26 replaced with Right; fix that while there.
- `CHANGELOG.md` entry at release. Not the manual, not the README.

## Tests (app-core, TDD)

- `b` on the map, in base space and in the Stack opens the hub; `B` on the
  map does nothing.
- Each command page lists its rows in base space; off-base on the open grid,
  only the `Anywhere` rows, and Deploy while no Home stands (founding).
- Enter on a row opens its screen; Esc returns to the same command page with
  `hub_on_rail == false` and the row cursor where it was.
- An all-hidden command page draws/reports no rows and is still on the rail.
- Overview's count for a command page equals `command_rows(page).len()`.
- Existing tests that press `b` and pick a row (~50 across `group_menus`,
  `building`, `caravan`, `dispatch`, `research`, `contracts`, `notifications`,
  `dossier`, `breeding`, `readonly_screens`, `base_report`) move to a support
  helper that opens the hub on the right command page and selects the row by
  label. `support.rs`'s `'b' => base_menu_rows` is rewritten onto it.
- gui: a headless row-width check that the widest command label fits the
  content rect (`popup-row-width-is-testable-headlessly`).

No engine change, no schema change, no save change: `Mode` is not saved.
`base_report_last` is session state.

## Out of scope

- The party menu (`p`).
- Showing illegal rows greyed with a reason.
- Re-ordering or renaming the existing report pages.
