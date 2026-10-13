# Base report — one place to see how the base is doing

The base's state is spread over the base menu: Base output, Work orders,
Base staff, the Structure roster and the alert board (`N`) are each their
own screen. Staff wellbeing (mood, needs, grievances) is only visible one
program at a time on the manifest. Nothing says where the gaps are.

This change adds the **base report**: a hub in the redesigned manifest's
style (`crates/gui/src/render/manifest/`) with a page rail on the left and
the page on the right. It absorbs the existing base screens and adds pages
for staff health, machines, power, stock and build orders. Each rail entry
carries a gap flag, so the rail alone answers "where is the base short?"

It also gives amenities (structures with `services:`) a hard capacity of
two programs, so "amenities needed vs built" is a real number.

## Decisions

| Question | Decision |
|---|---|
| Shell | Shared frame over per-screen `Mode`s (not one mega-mode, not the info column). Existing editing handlers survive. |
| Navigation | Rail of pages; `[` / `]` previous/next page (wrapping); `Tab` cycles a page's screens; `Esc` leaves. |
| Why not digits for the rail | Alerts, Structures and Work orders already use `App::selected_index` (digits + lowercase) for row jumps. `[` `]` are unbound everywhere in app-core. |
| Landing page | Overview. |
| Map key | `B` opens the hub on Overview (free on the map; pairs with `b`). `N` opens it on Alerts. A "Base report" base-menu row also opens it. |
| Absorbed screens | Base output, Work orders, Base staff, Structure roster, Alerts move in; their base-menu rows are deleted (each in its phase). |
| Coverage model | Amenity capacity hard-coded at 2 (`tuning::AMENITY_CAPACITY`); needed = ⌈staff × off-shift share ÷ capacity⌉, min 1. |
| Capacity revisit | Deferred: a later session decides how many programs an amenity serves (data field? per structure? tier?). |
| Delivery | Four phases, each its own plan and release. |

## Page map

Rail order (fixed; `[`/`]` walk it):

| # | Page | Screens (`Tab`) | Mode per screen |
|---|---|---|---|
| 1 | Overview | — | `BaseOverview` (new) |
| 2 | Staff | Wellbeing · Coverage · Discontent | `StaffWellbeing`, `StaffCoverage`, `StaffDiscontent` (new) |
| 3 | Production | Output · Machines · Power | `BaseOutput` (existing), `BaseMachines`, `BasePower` (new) |
| 4 | Stock | — | `BaseStock` (new) |
| 5 | Work orders | — | `WorkOrders` (existing; `WorkOrderPick`/`WorkOrderQuantity` stay popups over it) |
| 6 | Staffing | Duties · Standing jobs | `BaseStaff`, `Structures` (existing; `StructureAssign` stays a popup) |
| 7 | Build orders | Builds · Digging · Floors | `BuildOrders`, `DigOrders`, `FloorOrders` (new) |
| 8 | Alerts | — | `Alerts` (existing) |

One table in app-core, `BASE_REPORT_PAGES: &[BasePage]` with
`BasePage { title, screens: &[Mode] }`, is the single source of the rail,
the `Tab` order and "is this mode in the hub". The frame derives the current
page and screen from the current `Mode` by lookup — no stored screen index.
`App` keeps only `base_report_last: Option<Mode>` so reopening with `B`
returns to where the player was (Overview on first open).

A page whose screens are absent in the current phase is simply not in the
table yet; the rail grows phase by phase.

## Frame and keys

- **Render:** new `crates/gui/src/render/base_report/` — `frame.rs` (rail,
  title with screen dots `● ○ ○`, keybar) and one file per new screen. It
  reuses `render::manifest::layout` geometry (`frame_rect`, `RailGeom`,
  `rail_window`) and the keybar helpers; if those are `pub(super)`, lift
  them to `pub(in crate::render)` rather than copying. Draws only through
  `Painter` (drawing seam).
- **Rail entry:** page title plus `!` when its gap flag is set (same mark
  as the manifest's `needs_you`).
- **Existing screens inside the frame:** `draw_base_output`,
  `draw_work_orders`, `draw_base_staff`, `draw_structures` and the alerts
  draw are re-hosted in the frame's content rect instead of their own
  full-screen layout. Their content is unchanged except where a phase says.
- **Input:** a single `handle_base_report_nav(key) -> bool` runs first for
  any mode in `BASE_REPORT_PAGES`; it consumes `[`, `]`, `Tab` and `Esc`
  and otherwise falls through to the screen's existing handler. Each
  absorbed handler's own `Esc`/`Tab` arm is removed (Phase 3 checks
  `BaseStaff` and `Structures` for `Tab` before deleting).
- **Esc** leaves the hub to wherever it was opened from (`menu_origin`
  rule), never to the previous page.
- **Locality:** the hub is readable anywhere (like the roster today); the
  existing "refuse to pick a workable structure out of base" rule stays in
  the `Structures` handler.
- Every new `Mode` owes its rows in `ALL_MODES`, the draw match,
  `needs_status_banner` and the input dispatch (the `lib.rs` doc rule).

## Engine API

All derivations live in the engine; the renderer formats only.

- `Game::base_report_flags() -> BaseReportFlags` — one bool (or short
  reason string) per page. Overview, the rail `!` and nothing else read
  it. Built from the same functions the pages call (no second formula).
- `Game::staff_wellbeing() -> Vec<WellbeingRow>` — Staff-role owned
  programs: entity, name, morale sum + band, hp/max_hp, per-need
  `(id, level, critical, content)`, activity (on shift / seeing to `need`
  / waiting for a place / downed), grievance. Sorted worst morale first,
  ties by lowest need fraction. Built from `owned_pets`, `morale`,
  `need_levels`, `grievance`.
- `Game::need_coverage() -> Vec<CoverageRow>` — per need in `NeedDb`:
  built count, places (effective, summed), needed, programs below critical,
  programs below content, staff-time-lost %, and a list of boxed-in
  amenities `(label, pos, open_tiles)`.
- `needs::off_shift_share(def: &NeedDef, service: f32, working_share: f32)
  -> f32` — pure function; the formula below. Coverage and the gap flag
  both call it.
- `Game::discontent() -> Vec<DiscontentRow>` — programs with a grievance:
  name, rung, morale band, three most negative memories from
  `memory_report`.
- `Game::machine_report() -> Vec<MachineRow>` — non-`Running` structures
  grouped by `MachineStatus`, plus structures below full durability. From
  `structure_report`.
- `Game::power_report() -> PowerReport` — `base_power()` totals plus
  per-structure draw/supply from `StructureDb` defs over built structures,
  biggest first. `PowerLedger` stays `pub(crate)`.
- `Game::stock_report() -> Vec<StockReportRow>` — `base_stock()` joined to
  `work_order_report()` targets; rows short of a target first.
- `Game::floor_marks() -> Vec<FloorMark>` — new list over the finish marks
  (`base_space`); `build_order_report()` and `marked_cells()` already exist.

## Amenity capacity (Phase 2 sim change)

Today an amenity refills every program in reach each tick with no cap
(`needs_drain_system`, `systems.rs`), so one is always enough.

- `tuning::AMENITY_CAPACITY: usize = 2`, commented as a placeholder pending
  the capacity revisit.
- **Effective places** = min(`AMENITY_CAPACITY`, open tiles in reach). A
  tile in reach is open if walkable and holds no structure. One pure
  function, called by the sim and by `need_coverage`.
- **Who is served:** per amenity per tick, at most `places` programs in
  reach are refilled. Programs already being served keep their place
  (stored on the program as `ServicedAt(Entity)`, derived and not saved —
  re-claimed on load the first tick); free places go to the nearest
  waiting program, ties by entity order.
- **Routing:** an `OffShift(need)` program heads to the nearest amenity of
  that need with a free place; if none has one, to the nearest one, where
  it waits in reach (drain continues). The `pressing_need`, latch and
  `fray` rules are unchanged.
- **Needed:** with K = critical, C = content, d = drain, m =
  working_multiplier, r = per_tick × room scale, w = working share:
  - average drain d̄ = d·(1 + w·(m − 1))
  - T_work = (C − K) / d̄, T_serv = (C − K) / (r − d)
  - share f = T_serv / (T_work + T_serv) (walk time ignored — stated on
    screen as "approx.")
  - needed = max(1, ⌈staff × f ÷ AMENITY_CAPACITY⌉)
  - w is measured: the fraction of Staff programs currently holding a
    `Task`, floor 0.5 so an idle base doesn't under-report.
- No save-format change: `ServicedAt` is not serialized.

## Screens

| Screen | Shows | Gap flag (page `!`) |
|---|---|---|
| Overview | One line per page: worst gap, or "ok". Cursor + `Enter` jumps to that page. | — |
| Wellbeing | Name, mood band, HP bar, one bar per need, activity. | anyone below a need's critical, or downed |
| Coverage | `coherence  built 1 · places 2/2 · needed 2 ⚠  critical 1  lost ~7%`; boxed-in amenities listed under their need. | built < needed, or built = 0 |
| Discontent | Grievance rung, mood band, top 3 negative memories. | any grievance at `DownedTools` or worse |
| Output | Today's Base output report. | — |
| Machines | Non-running machines by status with reason; damaged structures. | any `Unpowered`/`Starved`/`Unstaffed`/`Stranded` |
| Power | Supply vs draw; per-structure list. | draw > supply |
| Stock | Base stock vs work-order targets, short rows first. | any target short with its order stalled |
| Work orders | Existing queue. | any order `Stalled` |
| Duties / Standing jobs | Existing work table / structure roster. | `unworked_total > 0` |
| Builds / Digging / Floors | Build orders with % and builder or "pending"; dig marks; floor marks. | a build `awaiting_program` or blocked |
| Alerts | Existing board. | unread alerts |

Every list screen scrolls by cursor (rows past the content rect are
windowed like `rail_window`); no screen relies on fitting.

## Phases

1. **Shell.** `BASE_REPORT_PAGES`, frame, `[`/`]`/`Tab`/`Esc` nav,
   `B` key and base-menu row, `base_report_flags` (Production + Alerts
   only), Overview, Output and Alerts moved in. Delete the "Base output"
   row. `N` opens the hub on Alerts.
2. **Staff.** Amenity capacity sim change (failing reproducer first; base
   bench P2 staff measures before/after), `need_coverage`,
   `staff_wellbeing`, `discontent`, the three Staff screens.
3. **Work orders + Staffing moved in.** Delete the "Work orders", "Base
   staff" and "Structure roster" rows. Strip absorbed `Esc`/`Tab` arms.
4. **Machines, Power, Stock, Build orders.** The remaining engine reports
   and screens, including `floor_marks`.

Each phase lands as its own release (`CHANGELOG.md` entry; patch digit —
no save breaks). The help pages for the moved screens are updated in the
phase that moves them.

## Testing

- **Engine:** unit tests per report on a `dev-saves/` template where one
  fits; `off_shift_share` and effective-places as pure-function tests.
  Capacity: a reproducer with three programs critical on one defrag bay
  — today all three refill; after, exactly two do and the third waits.
  Boxed-in bay with one open tile serves one.
- **app-core:** nav tests — `[`/`]` wrap the rail, `Tab` cycles screens,
  `Esc` returns to origin, a digit on Alerts still selects a row (not a
  page), `B` reopens the last page.
- **gui:** `ALL_MODES` census; a `--screenshot` per new screen checked by
  reading the PNG.
- `balance_sim` unaffected (no `tuning.rs` combat constant touched) — run
  it anyway in Phase 2 since `tuning.rs` changes.
- `cargo test --workspace` and `cargo clippy --workspace --all-targets`
  at every phase.

## Deferred

- **Amenity capacity revisit:** how many programs an amenity serves
  (per-structure `.ron` field, tier scaling, room bonus), replacing the
  hard-coded 2.
- Walk time in the needed formula.
- History/trends (morale over time, output rate graphs).
