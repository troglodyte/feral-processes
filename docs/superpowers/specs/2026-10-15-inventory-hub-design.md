# Inventory hub: `i` opens a rail of pages like `b`

**Status:** design approved in chat 2026-10-15; spec awaiting review.

## Goal

`i` opens a hub in the base report's style: a rail of pages on the left and
the selected page on the right. The pack's two tabs, the downed-program store
and the tool kit become its pages. To make that possible, the base report's
frame and navigation are first made generic over a page table, with no
visible change to `b`.

The party menu (`p`) gets the same treatment in a later spec.

Success:

- `b` looks and behaves exactly as it does today.
- Every screen reachable today is still reachable, and every Esc lands
  somewhere the player has been (see **Reachability**).

## Decisions

| Question | Decision |
|---|---|
| Sequencing | Shared hub and `i` in this spec; `p` in its own spec afterwards |
| Pages | Items, Phase keys, Downed programs, Tools |
| Tools | Moves from the party menu to `i`; its `PARTY_ROWS` row is deleted |
| Downed programs | Becomes an `i` page; `D` on the map still opens it directly |
| Equipment page | Not added. Equipping stays on the Items page's worn rows. A whole-party equipment page would be a new screen, not a move |
| Manifest (`x`) | Unchanged, its own full screen |
| Letter shortcuts | All kept (`D`, `I`, `S`, the item action letters, Tools' `F`/`I`/`X`); hub navigation is added alongside them |
| Sub-screens | Item action, describe, erase, equip swap, Mod Bench, Refit Bench and the trade pickers stay as they are drawn today. They are screens a page opens, the way a base command page opens its screens, not hub pages |

## The shared hub (no visible change to `b`)

### app-core

- `BasePage` becomes `HubPage { title, screens }`. A `Hub` value carries a
  rail title, its `&'static [HubPage]` table and its last-visited slot.
  There are two hubs: `BASE_REPORT` (today's `BASE_REPORT_PAGES`, unchanged)
  and `INVENTORY`.
- `base_page_of(mode)` becomes `hub_of(mode) -> Option<(Hub, page, screen)>`.
  Every caller of `base_page_of` moves to it (input.rs, gui `render/mod.rs`,
  `base_report/`, `gui/src/lib.rs` volume keys, the test support helper).
  A mode belongs to at most one hub, and a test asserts that.
- `handle_base_report_nav` becomes `handle_hub_nav`, read off whichever hub
  the mode is in. The keys are unchanged: Up/Down on the rail, Right or Enter
  into the page, Left/Right across a page's screens, `[`/`]` turn pages, Esc
  in a page goes to the rail, and Esc on the rail runs `close_screen`. The
  BaseStaff column exception stays and is joined by the Downed exception
  below.
- `base_report_last` becomes one last-visited mode per hub. `input.rs`'s
  bookkeeping (recording the last page, restoring the parked row, clearing
  `menu_origin`) runs for any hub mode, not only base ones.
- `hub_on_rail` stays one field: only one hub is ever open.

### gui

- `frame.rs`'s `paint_frame` and `draw_rail` take the hub's title, page
  table and flags as parameters instead of hardcoding "BASE REPORT" and
  `BASE_REPORT_PAGES`. `BaseReportFlags` stays the base hub's flags; the
  inventory hub passes none.
- `render/mod.rs` dispatches any hub mode to the hub frame; the per-screen
  content is chosen by mode as `screen_rows` does now.
- The drawing seam is unchanged: only `paint.rs` names a graphics library.
- The frame and rail code may move from `render/base_report/` to a
  `render/hub/` module now that two hubs use it. The plan decides, following
  the `module-boundaries` skill.

## The inventory hub

| Rail | Mode | Today |
|---|---|---|
| Items | `Mode::Inventory` | Inventory popup, Items tab |
| Phase keys | `Mode::PhaseKeys` (new) | Inventory popup, Phase Keys tab |
| Downed programs | `Mode::DownedPrograms` | Its own popup |
| Tools | `Mode::Tools` | Its own popup, a row in `p` |

Each page has one screen. Rail title: "INVENTORY".

- **Opening.** `i` opens the hub on the page last visited (Items the first
  time) with the cursor on the rail, which is `b`'s rule. `InventoryTab`,
  `App::inventory_tab` and the Tab toggle on the pack are deleted, and
  `gui/render/inventory.rs`'s `tab_header` goes with them.
- **`D` on the map** opens the Downed page with the cursor in the page,
  which is `N`'s rule for Alerts. Esc goes to the rail, and Esc again to the
  map.
- **`D` on the Items page** turns to the Downed page with the cursor in the
  page. It no longer sets `menu_origin`, so the two-doors comment and special
  case in `handle_downed_programs_key` and `close_screen`'s doc go away.
- **Downed drill-down.** With a program picked
  (`pending_downed_program_index` is `Some`), Esc steps back to the list as
  it does today, and the hub nav lets it through. Only Esc on the list goes
  to the rail.
- **Tools.** `F`/`I`/`X` and the row cursor are unchanged. The handler's own
  Esc arm is removed, because the hub nav takes Esc. An empty tool list
  still sits on the rail and draws one line saying there are no tools, as an
  empty base command page does.
- **Returns to Items.** Every path that ends with
  `self.mode = Mode::Inventory` (equip swap with no target, Mod Bench, Refit
  Bench, `return_to_trade_list`, item action, erase) lands on the Items page
  with the cursor **in the page** and the highlight where it was. The plan
  reuses the mechanism the base command pages use for their returns
  (`command_return_row`, `hub_on_rail = false`) rather than adding a second
  one.
- `Mode::PhaseKeys` joins every list `Mode::Inventory` is in: the
  `lib.rs:~2483` classification and gui `render/mod.rs:~1735`.

## Reachability

Every door into or out of a moved screen, before and after. The
implementation adds a test for each row marked **test**.

| Screen | Door today | Door after |
|---|---|---|
| Inventory (Items) | `i` on the map | `i` opens the hub; Items page — **test** |
| Phase keys | Tab on Inventory | Phase keys page — **test** |
| Downed programs | `D` on the map; `D` on Inventory | `D` on the map; `D` on Items; Downed page — **test** each |
| Tools | `p` → Tools row | Tools page; `p` no longer lists it — **test** both |
| Item action | Enter on a cargo row | unchanged — **test** Esc lands on Items, in page |
| Equip swap | Enter on a worn row; `E` from a companion | unchanged — **test** Esc returns to Items in page, and to CompanionEquip |
| Item describe | `I` on Items; `[d]` on item action; other gear lists | unchanged — **test** Esc from `I` lands on Items, in page |
| Erase quantity | item action | unchanged |
| Mod Bench / Refit Bench | item action | unchanged — **test** Esc lands on Items, in page |
| Trade / TradeQuantity (sell) | `[S]ell` on item action | unchanged — **test** finishing a sale lands on Items, in page |
| Trap placement | item action | unchanged |
| Downed tool-and-yield page | Enter on a downed program | unchanged — **test** Esc returns to the list, not the rail |

The tests run through the real key handlers, not by setting `app.mode`, so
they would catch a door that was lost. Before deleting `InventoryTab` and the
Tools row, the plan runs
`rg "Mode::(Inventory|DownedPrograms|Tools)|InventoryTab"` and checks every
hit against this table.

## Text

- `assets/help/20-controls.md`: the `i` line describes the hub and its pages;
  the `D` line says it opens the Downed page; `tools` comes out of the `p`
  line's list.
- `assets/help/75-extraction.md`: "The Tools row on the party menu" becomes
  the Tools page of the inventory.
- `assets/contracts/tutorial_strip_it_down.ron`: `[D]` still works as
  written. Check it still reads correctly, and change nothing if it does.
- `CHANGELOG.md` entry at release. Not the manual, not the README.

## Tests

App-core, TDD:

- Hub nav on the inventory table: rail Up/Down wrap, `[`/`]`, Right or Enter
  into the page, Esc to the rail, Esc out to the map.
- Every row in the Reachability table marked **test**.
- `i` reopens on the last-visited inventory page; `b` still reopens on the
  last-visited base page, and the two do not share a slot.
- No mode is in two hubs.
- The existing base report tests pass unedited apart from the renames
  (`base_page_of` → `hub_of`). That is the "no visible change" gate on the
  app-core side.
- Existing inventory, downed-program and tools tests that open their screen
  by key are ported through the support helper that opens a hub page.

gui:

- A headless row-width check that the widest Items row (the swap and item
  rows already carry the `seams-items` width traps) fits the hub content
  rect, which is narrower than today's popup body. If it does not fit, that
  is a design question to bring back to review, not something to clip.
- Screenshots of `b` before and after the shared-hub step (they must
  match), and of each `i` page.

No engine change, no schema change, no save change: `Mode` and the hub state
are not saved.

## Out of scope

- The party menu (`p`) hub. That is the next spec.
- History, compass, trade, field routine, help, achievements and options
  stay popups.
- A whole-party equipment page.
