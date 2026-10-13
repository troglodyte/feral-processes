# Base Report Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Checkbox steps.

**Goal:** A base report hub (page rail + page) that absorbs the base screens, adds staff, machine, power, stock and build-order pages, and caps amenities at two programs.

**Spec:** `docs/superpowers/specs/2026-10-13-base-report-design.md`. Read it first. This plan does not restate its decisions, formulas or screen table.

**Phases:** four, each landing as its own patch release, strictly in order. **Dispatch one subagent per phase.** A phase fits one context and its tasks share files. A subagent reads only the Global Constraints, the Spec corrections and its own phase.

Anchors: `A=crates/app-core/src`, `G=crates/gui/src`, `E=crates/engine/src`. Line numbers date from 0.18.22, so re-check them before editing.

## Spec corrections (code differs from the spec)

- **`ALL_MODES`** is a test-only const in `G/render/mod.rs:1662` (`[Mode; 136]`). Bump the length when adding to it. A new `Mode` also owes rows in the draw match (`G/render/mod.rs` ~1190–1591), `needs_status_banner` (:566), the dispatch (`A/app/input.rs` ~280–368) and the long `Mode::` list at `A/lib.rs` ~2404–2504. Read that list's doc to learn what it gates, and add the new modes where their siblings are.
- **No absorbed handler has a `Tab` arm.** Phase 3's "check for `Tab`" is already answered. Only the `Esc` arms go, and each one is the first arm of its handler.
- **Absorbed draws are popups.** `draw_work_orders`, `draw_base_staff`, `draw_structures`, `draw_base_output` (`G/render/building.rs`) and `draw_alerts` (`G/render/alerts.rs:10`) all call `draw_popup(…, PopupSize::Large, …)` (`G/render/popup.rs:11`). Re-hosting one means splitting its row-building from the popup chrome, so the frame can draw the same rows into its content rect. Do not copy any of them.
- **`per_tick`** belongs to `ServiceDef` (`E/structures.rs:157`), not `NeedDef`. In the spec's formula, r = `ServiceDef.per_tick × room_service_scale`. d, K and C are `NeedDef.drain_per_tick`, `critical` and `content` (`E/needs.rs:66`).
- **Floor marks** are `DigSite` entities carrying a `FinishOrder` (`E/components.rs:2724`, `:2762`). `floor_marks()` queries them, and no list exists yet.
- **Manifest layout** helpers are already `pub(in crate::render)` (`G/render/manifest/layout.rs`). Only the keybar helpers (`manifest/keybar.rs:46–101`) need lifting.

## Global Constraints

- The renderer never touches `World` and gets no accessor. Only `G/paint.rs` names the graphics library, and `render/` draws through `Painter`. Read `.claude/rules/drawing-seam.md` and the base/HUD/screens `seams-*.md` rules if they have not loaded.
- All derivation happens in the engine and the renderer only formats. There must be no second formula: the gap flags call the same functions the pages call (CLAUDE.md "mirror = call").
- No save-format change. `ServicedAt` is not serialized.
- Named constants live in `tuning.rs`. Fail fast, using `Result`/`?` in the engine.
- TDD: a failing test first, with a mutation-check on the load-bearing ones. A green step gets `cargo fmt`, `cargo clippy --workspace --all-targets` clean and a commit.
- Unset `FERAL_DEV_NO_SIEGES` before running tests. The phase gate is `cargo test --workspace`.
- Subagents never push, never `git checkout`, `git stash` or `git add -A`, and stage explicit paths only.
- Help: update the `assets/help/` page that covers a moved screen in the phase that moves it (60/62/63 as candidates, so grep them). New keys go in `20-controls.md`.
- **Phase end:** the controller runs a whole-phase opus review against that phase's Review Focus. After fixes, land via the `deploy` skill (merge → next patch version → `CHANGELOG.md` → tag → push → cleanup) and add the phase to `docs/superpowers/INDEX.md`. Ask the user before landing.

---

## Phase 1: Shell (app-core + gui; engine flags only)

**Moves in:** Overview (new), Output, Alerts.

### Task 1.1: Page table and nav (app-core)
- [ ] Add `BasePage { title: &'static str, screens: &'static [Mode] }` and `BASE_REPORT_PAGES` (Overview, Production[`BaseOutput`], Alerts) beside `Mode`. Add `Mode::BaseOverview`. Add `App::base_report_last: Option<Mode>`. Add a lookup `base_page_of(mode) -> Option<(page_idx, screen_idx)>`.
- [ ] `handle_base_report_nav(key) -> bool` runs at the top of dispatch for any hub mode. `[`/`]` move between pages with wrap, `Tab` cycles screens with wrap, and `Esc` returns to `menu_origin` or Playing. Each move stores `base_report_last`. Delete the `Esc` arms of `handle_base_output_key` (`A/app/menus.rs:416`) and the alerts handler (`menus.rs:282`).
- [ ] Openers: `B` on the map (`A/app/playing.rs`, near `'b'` :186) opens `base_report_last`, falling back to Overview. `N` (:336) keeps its mark-read behaviour and opens the hub on Alerts. Add a "Base report" row to `A/app/group_menu.rs` (Locality Anywhere) and delete the "Base output" row (:287). Every opener sets `menu_origin`. Before binding `B` and `[`/`]`, grep the other map-key files for them.
- [ ] Overview handler: a cursor over the pages, where `Enter` jumps to the page's first screen.
- [ ] Tests go in a new `A/tests/base_report.rs` registered in `tests/mod.rs` and modelled on `tests/alerts.rs`: `]` wraps last→first, `[` wraps first→last, `Esc` returns to origin from page 3, a digit on Alerts selects a row and does not change page, `B` reopens the last page, `N` lands on Alerts with alerts marked read, and the "Base output" row is gone.

### Task 1.2: Flags (engine)
- [ ] Add `views::BaseReportFlags` (one `Option<String>` reason per page; `None` = ok) and `Game::base_report_flags()`. In Phase 1 only Alerts is filled (unread alerts, from the same source as the `N` badge). Production stays `None` until Phase 4 (Output has no gap). Later phases add fields.
- [ ] Engine test: an unread alert sets the Alerts flag, and reading it clears the flag.

### Task 1.3: Frame and screens (gui)
- [ ] Create `G/render/base_report/{mod.rs, frame.rs, overview.rs}`. `frame.rs` draws the rail (title plus `!` in `hud::palette::ATTENTION` when flagged, the same mark as `manifest/rail.rs:128`), the title with screen dots, and a keybar (lift the keybar helpers). It reuses `frame_rect`, `RailGeom` and `rail_window`. It hands the content rect to the screen's draw.
- [ ] Split `draw_base_output` and `draw_alerts` into a row/body builder plus the popup caller, and have the hub call the body with the content rect. Delete the popup callers once nothing uses them.
- [ ] Add `ALL_MODES` and banner/draw rows for `BaseOverview`.
- [ ] Take a `--screenshot` of Overview, Output and Alerts (`--keys "B"` / `"B ]"` on a base template) and Read the PNGs.

**Review Focus:** `Esc` never steps back a page. Digits and lowercase still reach the screen handlers. No absorbed popup chrome remains. The rail is derived only from `BASE_REPORT_PAGES`.

---

## Phase 2: Staff (engine sim change + 3 screens)

### Task 2.1: Amenity capacity (engine, the risky step)
- [ ] Read `.claude/rules/` for needs/base and the `seams` skill for needs and labour, then read `docs/measurements/` for the base bench P2 staff numbers. Record before-numbers, using the same bench and conditions as the later after-run.
- [ ] Reproducer (fails first): three programs critical on one defrag bay all refill today. Afterwards exactly two refill and the third waits in reach while still draining. A second test: a bay boxed in with one open tile serves one program. Use a `dev-saves/` template with a staffed base if one fits (check `rooms`/`bench-economy`); otherwise build a minimal base.
- [ ] Add `tuning::AMENITY_CAPACITY = 2` (with a placeholder comment). Add a pure `offshift::effective_places(open_tiles) -> usize`, plus an open-tile counter that the sim and the report both call.
- [ ] In `needs_drain_system` (`E/systems.rs:129`, refill at :165–175), add a per-amenity place allocation: incumbents keep their places through `ServicedAt(Entity)` (not serialized; re-claimed on the first tick), and free places go to the nearest program, with ties broken by entity order. Only served programs refill.
- [ ] Routing: an `OffShift(need)` program goes to the nearest amenity with a free place, falling back to the nearest one. `pressing_need`, the latch and `fray` stay unchanged. Add a test with two bays, one full: the third program goes to the free one.
- [ ] Run `cargo test -p feral-processes-engine`, then `balance_sim`, then the bench after-numbers. Put the before/after in the commit message.

### Task 2.2: Staff reports (engine)
- [ ] Add a pure `needs::off_shift_share(def, service, working_share) -> f32` using the spec formula, with table tests (one hand-computed case, and w = 0.5 floor applied by the caller).
- [ ] Add `Game::need_coverage()`, `staff_wellbeing()` and `discontent()`, with view types in `views.rs` and fields per the spec. Build them from `owned_pets` (`E/game/party.rs:707`), `morale` (`E/game/memories.rs:729`), `need_levels` and `grievance` (`E/game/base/offshift.rs:201/225`) and `memory_report` (`memories.rs:787`). Add one test per report covering sort order and one gap case each.
- [ ] Add the Staff flag to `base_report_flags`. It is built by calling those reports.

### Task 2.3: Staff screens (app-core + gui)
- [ ] Add modes `StaffWellbeing`, `StaffCoverage` and `StaffDiscontent`, and a Staff page in `BASE_REPORT_PAGES`. Each screen gets a cursor-scroll handler. Wellbeing `Enter` → the program's manifest page is out of scope; leave it unbound.
- [ ] Add the draw files under `base_report/`, with lists windowed with `rail_window`. Coverage shows "approx." per the spec.
- [ ] App-core test: `Tab` cycles all three screens. Take screenshots of all three.
- [ ] Update `63-staff-morale.md` for capacity and the Staff page.

**Review Focus:** capacity holds across save→load (the `ServicedAt` re-claim). An incumbent is never evicted by a nearer arrival. Programs waiting in reach still drain. The coverage and flag share one formula. The bench before/after numbers come from matched conditions.

---

## Phase 3: Work orders + Staffing moved in (app-core + gui)

- [ ] Add a Work orders page (`WorkOrders`) and a Staffing page (`BaseStaff`, `Structures`) to `BASE_REPORT_PAGES`. `WorkOrderPick`, `WorkOrderQuantity` and `StructureAssign` stay popups drawn over their parent screen inside the frame. They are not in the table, so nav must not consume their keys. Test that `[` inside `WorkOrderPick` does not change page.
- [ ] Delete the `Esc` arms at `building.rs:886` and `:1033` and `menus.rs:350`. Child popups keep theirs, and their `Esc` returns to the parent screen.
- [ ] Delete the group-menu rows "Work orders" (:114), "Base staff" (:124) and "Structure roster" (~:154). Those two rows were `Locality::Base`. Find what that enforced and keep the same refusal in the screen's handler, with a test for it. The roster's "no workable pick out of base" rule stays.
- [ ] `input.rs:394–398` (the `work_column` reset on entering BaseStaff) and `input.rs:69` (`keeps_highlight` pair) must still hold when entering through `Tab`. Add a test for each.
- [ ] Split `draw_work_orders`, `draw_base_staff` and `draw_structures` into a body plus content rect, as in Phase 1. The prefetch match (`G/render/mod.rs` ~955–992) feeds the hub path too.
- [ ] Add the Work orders flag (any `Stalled`) and the Staffing flag (`unworked_total > 0`).
- [ ] Take screenshots and update the help pages (60/62).

**Review Focus:** Base-locality refusals survive the row deletion. The popup children's keys are untouched by nav. Nothing reaches the deleted menu targets.

---

## Phase 4: Machines, Power, Stock, Build orders

### Task 4.1: Engine reports
- [ ] Add `machine_report()` (from `structure_report`, `E/game/inspection.rs:1589`; re-read the full `MachineStatus` list at `E/components.rs:1014`), `power_report()` (`base_power` :1707 plus per-structure def draw/supply; `PowerLedger` stays `pub(crate)`), `stock_report()` (`base_stock` joined to `work_order_report` targets, short first) and `floor_marks()` (`DigSite` with `FinishOrder`). Each gets a test.
- [ ] Add the Production flag (Machines statuses, power draw > supply), the Stock flag and the Build orders flag (`awaiting_program`/blocked, from `build_order_report`, `inspection.rs:790`).

### Task 4.2: Screens
- [ ] Production gains `BaseMachines` and `BasePower`. Add a Stock page (`BaseStock`) and a Build orders page (`BuildOrders`, `DigOrders`, `FloorOrders`). Each gets a read-only cursor-scroll handler and a draw file. Add census rows.
- [ ] App-core nav test over the full 8-page rail. Take a screenshot per new screen.
- [ ] Update help (`60-your-base.md`).

**Review Focus:** power per-structure totals reconcile with `base_power()`. The floor marks exclude plain dig marks. Every flag calls its page's function.
