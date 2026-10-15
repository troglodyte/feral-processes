# Inventory hub Implementation Plan

> **For agentic workers:** use superpowers:subagent-driven-development, with one subagent per phase. Steps use checkbox (`- [ ]`) syntax.

**Goal:** `i` opens a hub (page rail on the left, page on the right) whose pages are Items, Phase keys, Downed programs and Tools. Before that, the base report's frame and nav are made generic, with no visible change to `b`.

**Spec:** `docs/superpowers/specs/2026-10-15-inventory-hub-design.md`. Read it first. This plan does not restate it.

**Open decisions taken at their `b`-consistent default** (the user may still overturn them; each is a one-line change in `open_inventory_hub`):
- `i` opens with the cursor on the rail.
- Esc from a page goes to the rail, then out.

## Why phases

Each phase is one subagent with a clean context, and its handback is a short summary. Phase 1 leaves the tree green and behaviour unchanged, so it can land as its own commit even if later phases stall. Phases 2 and 3 split app-core and gui so neither subagent loads the other crate's files.

## Global constraints (every phase prompt carries these)

- No engine, schema or save change. `Mode`, `hub_on_rail` and the last-visited slots are session state.
- Only `crates/gui/src/paint.rs` names a graphics library (`.claude/rules/drawing-seam.md`). Read `.claude/rules/seams-screens.md` and `seams-hud.md` before editing.
- Follow the existing naming. Comments say why, not what. Delete what becomes unused (`InventoryTab`, `inventory_tab`, `tab_header`, the Tools `PARTY_ROWS` entry, the `menu_origin` two-doors special case for Downed). No shims or aliases.
- Uppercase letters are actions and lowercase letters are row selectors. All existing letter shortcuts keep working.
- Gates per phase: `cargo fmt`, `cargo clippy --workspace --all-targets` with no warnings, and the phase's crate tests. **Unset `FERAL_DEV_NO_SIEGES`** before testing. Don't pipe cargo output if the exit code matters.
- Git: work on branch `inventory-hub` in the primary checkout. Commit at each green step, staging explicit paths only (never `git add -A`). Never push, and never run `git checkout`, `stash` or `reset`.
- Docs: never touch the manual, README or TODO.md. CHANGELOG is written at release, not in this plan.

---

## Phase 1: generic hub, no visible change (app-core + gui)

**Files**
- `crates/app-core/src/lib.rs`: `BasePage` (~2719) becomes `HubPage`. Add `pub enum Hub { Base, Inventory }` with `title()` and `pages() -> &'static [HubPage]`. `BASE_REPORT_PAGES` stays as the base table. `base_page_of` (~2784) becomes `pub fn hub_of(mode) -> Option<(Hub, usize, usize)>`. `base_report_last` becomes `hub_last: [Option<Mode>; 2]` or a per-`Hub` accessor; pick the smaller diff.
- `crates/app-core/src/app/base_report.rs`: `handle_base_report_nav` becomes `handle_hub_nav`, reading `hub.pages()`. The BaseStaff exception stays. Rename the file to `hub.rs` only if the module-boundaries skill says to.
- `crates/app-core/src/app/input.rs` (~250, ~431): call sites move to `hub_of`, and the last-visited record is kept per hub.
- `crates/gui/src/render/base_report/frame.rs`: `paint_frame` and `draw_rail` take `(title, pages, flags: Option<&BaseReportFlags>)`. `frame.rs:353`, `mod.rs:54,81`, `render/mod.rs:1596` and `gui/src/lib.rs:899` move to `hub_of`.
- `crates/app-core/src/tests/support.rs` (~2762): `open_hub_screen` reads `hub_of` and presses the hub's own open key (`b` for now).

**Steps**
- [ ] **Before any edit,** take baseline screenshots of the base report: `cargo run -- --template bench-economy --keys "b" --screenshot <scratch>/b-before.png`, plus one after `b Down Down Right`. Read the PNGs to confirm they show the hub. If the template has no Home, choose another from `dev-saves/README.md`.
- [ ] Rename and generalise as above. Add a test asserting that no `Mode` is in two hubs (iterate every hub's pages).
- [ ] Gate: the app-core and gui tests pass, and the existing base-report tests are **unedited except for renames**.
- [ ] Screenshot again with the same keys and compare with the baseline. The images must be identical; a differing pixel is a regression.
- [ ] Commit: "Generalise the base report frame and nav into a hub".

**Handback:** files touched, the shape of the new `Hub` API (signatures only), and the screenshot comparison result.

---

## Phase 2: inventory hub behaviour (app-core, TDD)

**Files**
- `crates/app-core/src/lib.rs`: add `Mode::PhaseKeys` and add it to every list `Mode::Inventory` is in (`rg "Mode::Inventory$|Mode::Inventory\b" crates`, including ~2483). Add the `INVENTORY_PAGES` table (Items `[Inventory]`, Phase keys `[PhaseKeys]`, Downed programs `[DownedPrograms]`, Tools `[Tools]`) and wire it to `Hub::Inventory` with title "INVENTORY". Delete `InventoryTab` and `App::inventory_tab`.
- `crates/app-core/src/app/playing.rs` (~192 `i`, ~213 `D`): `i` calls `open_inventory_hub()`, which opens the last-visited page or Items, cursor on rail. `D` sets `hub_on_rail = false`, as the `N` arm at ~341 does.
- `crates/app-core/src/app/inventory.rs`: delete the Tab arm (~45). `D` (~122) turns to the Downed page with the cursor in the page, and no longer sets `menu_origin`.
- `crates/app-core/src/app/extraction.rs`: Esc with `pending_downed_program_index == Some` steps back to the list. `handle_hub_nav` must let that Esc through, with a guard like BaseStaff's. Delete the two-doors comment.
- `crates/app-core/src/app/tools.rs`: delete the Esc arm, which the hub now owns, and update the module doc.
- `crates/app-core/src/app/group_menu.rs`: delete the Tools `GroupEntry` (~383-394) and fix the `close_screen` doc (~488) that names Inventory and Downed.
- Phase keys page handler: arrow and row keys do nothing, since it is read-only. Check `input.rs`'s dispatch has an arm for it.
- Tests: `crates/app-core/src/tests/{inventory,extraction,tools}.rs`, plus a new `inventory_hub.rs` or the nearest existing file, per module-boundaries.

**Steps (TDD: write each failing test first, then make it pass)**
- [ ] Hub nav on the inventory table: rail Up/Down wrap across the 4 pages, `[`/`]`, Right or Enter into the page, Esc in the page goes to the rail, Esc on the rail goes to `Playing`.
- [ ] Last-visited: `i` reopens on the last inventory page; `b` and `i` don't share a slot; the first `i` lands on Items, on the rail.
- [ ] Every row marked **test** in the spec's Reachability table, **driven through `handle_key`**, never by setting `app.mode` to reach the screen under test. The key cases:
  - `D` from the map lands on Downed, in the page, then Esc, Esc goes to `Playing`.
  - `D` on Items lands on Downed, in the page.
  - Esc in a Downed drill-down goes back to the list, not the rail.
  - `p` no longer offers Tools, and the Tools page is reachable by `]` from `i`.
  - Esc from item action, Mod Bench, Refit Bench, `I` describe, and finishing a sale each land on Items with `hub_on_rail == false`.
  - Equip swap opened from a companion still returns to `CompanionEquip`.
  - An empty Tools page still sits on the rail.
  Use the `extraction` and `bench-economy` templates or the existing fixtures in `tests/support.rs`.
- [ ] Port existing tests that set or toggle `inventory_tab`, or that open Tools through `p`, to `open_hub_screen`. Generalise that helper to take the hub's open key from `hub_of`.
- [ ] **Door audit:** `rg -n "Mode::(Inventory|PhaseKeys|DownedPrograms|Tools)\b|InventoryTab|inventory_tab" crates`. List every hit in the handback, each marked as matching the spec's table or fixed.
- [ ] Mutation-check two tests (skills:mutation-testing): remove the `hub_on_rail = false` in the `D` arm, and the Downed drill-down Esc guard. Each removal must turn a test red; revert after.
- [ ] Gate: `cargo test -p feral-processes-app-core` and clippy. gui will not compile until Phase 3 if it names `InventoryTab`, so keep the gui's `draw_inventory` signature building in this phase by passing nothing tab-shaped. If that is not small, do the minimal gui compile fix here and leave the drawing to Phase 3.
- [ ] Commit per green step.

**Handback:** a list of new tests, the door-audit table, and the mutation results.

---

## Phase 3: inventory hub drawing (gui)

**Files**
- `crates/gui/src/render/mod.rs` (~1336-1379, ~1596, ~1735): Inventory, PhaseKeys, DownedPrograms and Tools are drawn through the hub frame. Sub-screens (item action, equip swap, describe, erase, Mod Bench, Refit Bench, trade) keep their current popup drawing.
- `crates/gui/src/render/inventory.rs`: split `draw_inventory` into Items rows and Phase-keys rows, each drawn into the hub content rect via the frame's `draw_rows_in` path. Delete `tab_header`.
- `crates/gui/src/render/extraction.rs` and `crates/gui/src/render/tools.rs`: their row builders feed the content rect instead of a popup. Reuse the builder; don't copy it.
- If Phase 1 left the frame in `render/base_report/`, decide with module-boundaries whether it moves to `render/hub/`. Move it only if that keeps each file to one purpose.

**Steps**
- [ ] A headless row-width test (`popup-row-width-is-testable-headlessly`) checks that the widest Items row (an equipped stack with tag and quality figure), the widest Tools row and the widest Downed row fit the hub content rect. **If one does not fit, stop and hand back.** That is a design question for the user, not something to clip or wrap.
- [ ] Draw the four pages, plus the empty-Tools line.
- [ ] Screenshots, each Read to check it: `--template extraction --keys "i"`, `"i Down"`, `"i Down Down"`, `"i Down Down Down"`, `"D"`. Also check that `b` still matches the Phase 1 baseline.
- [ ] Gate: `cargo test -p feral-processes-gui` and clippy. Commit.

**Handback:** screenshot paths and the row-width margins.

---

## Phase 4: text, full gate, final review (controller)

- [ ] `assets/help/20-controls.md`: change the `i` line to the hub and its pages, make the `D` line say it opens the Downed page of the inventory, and remove `tools` from the `p` line's list. `assets/help/75-extraction.md:35`: "The Tools row on the party menu" becomes "The Tools page of your inventory (`i`)". Check that `tutorial_strip_it_down.ron` still reads correctly. It should need no edit.
- [ ] `cargo test --workspace`, with `FERAL_DEV_NO_SIEGES` unset.
- [ ] Final whole-branch review with **opus**, given the diff as a file. Review focus:
  1. Every door in the spec's Reachability table, re-derived from source rather than from the Phase 2 audit.
  2. `hub_on_rail` cannot be stale on any way into an inventory page: from the map, from a base page, or after `p` → companion → equip swap.
  3. `b` is unchanged, by app-core tests and the screenshot.
  4. Nothing is left that names `InventoryTab`, the Tools party row, or the Downed `menu_origin` special case.
- [ ] Fix the findings, and have the fixes reviewed too (a-reviews-fix-needs-its-own-review).
- [ ] Hand off to `skills:deploy` when the user says to land it.
