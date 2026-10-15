# Party hub: `p` opens a rail of pages like `b` and `i`

**Status:** design approved in chat 2026-10-15; spec awaiting review.

## Goal

`p` opens a third hub on the shared hub frame (`Hub`, `HubPage`, `hub_of`,
`handle_hub_nav`, `hub_last`), replacing the flat party menu. The roster
and Perks become pages drawn in the hub; the remaining party rows split
across two command pages, the way the base menu folded into `b`.

Success:

- Every screen reachable from `p` today is still reachable, under the same
  locality and availability rules, and every Esc lands somewhere the player
  has been (see **Reachability**).
- `b` and `i` behave exactly as they do today.

## Decisions

| Question | Decision |
|---|---|
| Pages | Companions (roster), Programs, Routines, Perks |
| Rail title | "PARTY" |
| Manifest | Stays its own full screen (it has its own roster rail). Reached from a Programs row and from `M` on the roster |
| Perks | A rail page drawn in the hub content rect |
| Party menu | `Mode::PartyMenu`, `PARTY_ROWS`, `party_menu_rows` and `handle_party_menu_key` are deleted |
| Letter shortcuts | All kept (roster `P` `N` `E` `W` `U` `M` `<` `>`; Perks `X` `S`); hub navigation is added alongside them |
| Sub-screens | Equip, memories, rename, manifest, dossier, fuse pickers, refactor, develop, routine install/etch/extract, PerkBought, AllocateStats, respec confirm stay as drawn today. They are screens a page opens, not hub pages |

## Pages

| Rail | Mode | Content |
|---|---|---|
| Companions | `Mode::Companion` | The roster |
| Programs | `Mode::PartyPrograms` (new) | Read a manifest, Fuse two programs, Refactor a program, Develop a program |
| Routines | `Mode::PartyRoutines` (new) | Install a routine, Etch a routine disk, Extract a routine |
| Perks | `Mode::Perks` | The perk list |

Each page has one screen. Every `GroupEntry` for a Programs or Routines row
moves verbatim from `PARTY_ROWS` — label, target, `Locality`, `available`
closure and its comment. The Companions and Perks rows are deleted: their
screens are pages now, and the Companions row's `Locality::Anywhere` comment
moves to the roster page's doc, since its argument (reading your own programs
must not vanish off-base) is now the reason the page is always on the rail.

## app-core

- `Hub::Party` joins `Hub::ALL`, titled "PARTY", with a `PARTY_PAGES` table
  and its own `hub_last` slot. The existing "no mode is in two hubs" test
  covers it.
- `open_party_hub` mirrors `open_inventory_hub`: the last-visited page, or
  Companions the first time, cursor on the rail. `playing.rs`'s `p` arm
  calls it.
- `group_menu.rs`: `PARTY_ROWS` splits into `PROGRAM_ROWS` and
  `ROUTINE_ROWS`, and `command_table` maps the two new modes to them, so
  `command_rows` stays the **only** source of a command page's rows
  (`seams-screens`). Selecting a row uses the existing shared dispatch
  (`open_row_target`) with `menu_origin` set to the command page, as the
  base command pages do.
- Hidden rows stay hidden. A command page with every row hidden still sits
  on the rail and draws one line saying nothing is available, as an empty
  base command page does. An empty roster draws one line saying there are no
  companions.
- `handle_companion_key` and `handle_perks_key` lose their Esc arms: hub nav
  takes Esc (in page → rail; on rail → `close_screen`). Neither handler
  binds Left, Right, `[` or `]`, so nothing else conflicts.
- **Perk previews.** `open_perks` is the one writer of `perk_previews`
  (`lib.rs` doc on the field). A rail turn sets `self.mode` directly, which
  would show a stale list. Whatever path lands on `Mode::Perks` — rail
  turn, `[`/`]`, `p` reopening there, LevelUp's `P`, the return from
  PerkBought, AllocateStats or the respec confirm — must run
  `refresh_perk_previews`. The plan picks the one place that guarantees it
  (an on-enter hook in the hub bookkeeping in `input.rs`, or routing hub
  turns through `open_row_target`), and keeps the "one writer" doc true.
- **Returns land in the page.** Every path that sets `Mode::Companion`
  (equip, memories, rename, `leave_manifest`'s `Roster` arm, the dossier
  through the manifest) lands on the Companions page with the cursor in the
  page and its row kept (`keeps_highlight` pairs unchanged). Every path back
  to `Mode::Perks` lands on the Perks page in page. Both hold because the
  cursor was in the page when the sub-screen opened and nothing between sets
  `hub_on_rail` — the inventory hub's mechanism; no second one is added.
- **LevelUp `P`** opens the Perks page with the cursor in the page
  (`hub_on_rail = false`), as `D` opens the Downed page. Esc goes to the
  rail, Esc again to the map.
- `ManifestOrigin::Menu` returns via `close_screen` to `menu_origin`, now the
  Programs page. Its doc and `leave_manifest`'s doc stop saying "party menu".
- `etch_return` keeps its job; its doc names the Routines page instead of
  `Mode::PartyMenu`.
- `Mode::PartyPrograms` and `Mode::PartyRoutines` join every list
  `Mode::PartyMenu` is in today (`lib.rs` classification, gui
  `render/mod.rs`), and `Mode::PartyMenu` leaves them.

## gui

- `render/mod.rs` dispatches the four party modes to the hub frame with the
  party hub's title and table; no flags.
- The two command pages reuse the base hub's command-list screen, which
  already draws from `command_rows` and draws the empty-page line.
- The roster (`draw_companion_menu`) and Perks (`draw_perks_menu`) are drawn
  into the hub content rect instead of their popup. They keep their row
  content; only the container changes.
- The drawing seam is unchanged: only `paint.rs` names a graphics library.

## Reachability

Every door into or out of a moved screen, before and after. The
implementation adds a test for each row marked **test**, driven through the
real key handlers rather than by setting `app.mode`.

| Screen | Door today | Door after |
|---|---|---|
| Roster | `p` → Companions | `p` opens the hub; Companions page — **test** |
| Manifest (menu) | `p` → Read a manifest | Programs → Read a manifest — **test** Esc lands on Programs, in page |
| Manifest (roster) | `M` on roster | unchanged — **test** Esc lands on Companions, in page, same row |
| Equip / memories / rename | `E` / `R` / `N` on roster | unchanged — **test** Esc lands on Companions, in page, same row |
| Fuse / FuseSecond / name | `p` → Fuse | Programs → Fuse — **test** Esc lands on Programs, in page |
| Refactor | `p` → Refactor | Programs → Refactor — **test** |
| Develop | `p` → Develop | Programs → Develop — **test** |
| Routine install | `p` → Install | Routines → Install — **test** Esc lands on Routines, in page |
| Routine etch | `p` → Etch; `[e]` during install | Routines → Etch; `[e]` unchanged — **test** both Esc paths |
| Extract | `p` → Extract | Routines → Extract — **test** |
| Perks | `p` → Perks; LevelUp `P`; AllocateStats return | Perks page; LevelUp `P` in page — **test** each, and that previews are fresh after a rail turn |
| PerkBought / respec confirm | from Perks | unchanged — **test** return lands on Perks, in page, row kept |
| Tools | already moved to `i` | unchanged |

Before deleting `Mode::PartyMenu` and `PARTY_ROWS`, the plan runs
`rg "PartyMenu|PARTY_ROWS|party_menu|party menu"` across `crates/` and
`assets/` and checks every hit against this table.

## Text

- `assets/help/20-controls.md`: the `p` line describes the hub and its
  pages, in the `i` line's form ("a rail of pages like the base report's …
  The rail and page keys work as they do under b").
- `assets/help/55-perks.md`, `50-routines.md`, `63-staff-morale.md`: "the
  party menu" / "the Perks row on the party menu" name the page instead.
- `CHANGELOG.md` entry at release. Not the manual, not the README.

## Tests

App-core, TDD:

- Hub nav on the party table: rail Up/Down wrap, `[`/`]`, Right or Enter
  into the page, Esc to the rail, Esc out to the map.
- Every Reachability row marked **test**.
- `p` reopens on the last-visited party page; `b` and `i` keep their own
  slots.
- A command page with every row hidden (e.g. one companion, no fusion
  research) still turns and Esc's normally.
- Existing party-menu tests (`group_menus.rs`, `menus.rs`, `refactor.rs`,
  `develop.rs`, `inventory_hub.rs`, `support.rs`) are ported to the helper
  that opens a hub page.
- Existing base report and inventory hub tests pass unedited.

gui:

- A headless row-width check that the widest roster row and widest Perks
  row fit the hub content rect, which is narrower than today's popups. If
  one does not fit, that is a design question brought back to review, not
  something to clip (`gui-text-never-clips-or-wraps`).
- Screenshots of each party page, and of `b` and `i` (unchanged).

No engine change, no schema change, no save change: `Mode` and hub state
are not saved.

## Out of scope

- The manifest's own frame and rail.
- A whole-party equipment page.
- Other popups (history, compass, trade, help, achievements, options).
