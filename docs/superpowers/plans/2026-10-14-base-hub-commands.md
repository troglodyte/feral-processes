# Base hub commands Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** `b` opens the base report hub, which gains four command pages (Build, Research, Trade, Compile) holding every row the base menu has today; `B` and `Mode::BaseMenu` are deleted.

**Architecture:** The command pages are ordinary hub screens in `BASE_REPORT_PAGES`. Their rows are today's `GroupEntry` tables split four ways and read through the existing `group_rows`, so locality and availability rules are untouched. gui draws them in the hub content rect from the same call.

**Tech Stack:** Rust, app-core (`App`/`Mode`), gui (bevy + egui through `Painter`).

**Spec:** `docs/superpowers/specs/2026-10-14-base-hub-commands-design.md` — read it first; this plan argues from it.

## Global Constraints

- No engine change, no schema change, no save change (`Mode` is not serialized; `base_report_last` is session state).
- `command_rows` is the **only** source of a command page's rows — handler, renderer and Overview all call it (`.claude/rules/seams-screens.md`, group-menu rule).
- No alias: `B` is deleted, not rebound. `inspection.rs:129`'s `B` (breeding on a bay sheet) is a different screen — leave it.
- Every `GroupEntry` moves verbatim: label, target, `Locality`, `available` closure **and its comment**.
- Only `crates/gui/src/paint.rs` names a graphics library (`.claude/rules/drawing-seam.md`).
- Player copy: plain language, no tick vocabulary; UPPERCASE letters are actions, lowercase are row selectors (`b` is a map-level group key, which is already lowercase — unchanged convention).
- Docs: CHANGELOG at release only; never the manual, README or TODO.md.
- Gates after each task: `cargo fmt`, `cargo clippy --workspace --all-targets` (no warnings), `cargo test -p feral-processes-app-core` (Task 1/3) or `-p feral-processes-gui` (Task 2). Unset `FERAL_DEV_NO_SIEGES` before testing. Final gate: `cargo test --workspace`.
- Git: commit at each green step, explicit paths only (never `git add -A`), never push, never `git checkout`/`stash`/`reset` anything.

## Review Focus

1. **Esc back from a command's screen lands on the right row, cursor in the page.** `menu_selected` is shared and the child screen overwrites it; `input.rs:404` resets it to `opening_row()` on mode change. Expect: Deploy is row 0 on Build; pick Demolish (row 2), Esc → Build page, `hub_on_rail == false`, `menu_selected == 2`.
2. **Page turn onto a command page after a prior return doesn't inherit a stale row.** Expect: after (1), `]` away and `[` back → `menu_selected == 0`.
3. **`b` in the Stack** — opens the hub (the arm must sit before the underground hand-off where `B` is now, `playing.rs:332`); only `Anywhere` rows show.
4. **Founding:** fresh run, no Home, on the open grid — Build page offers exactly "Deploy a structure"; Enter opens `Mode::Build` with `build_category == None`.
5. **Row letters while the cursor is on the rail are swallowed**, not executed (`handle_base_report_nav` already does this — pin it for a command page, because a letter there would now open a screen, not just scroll).

---

### Task 1: app-core — command pages replace the base menu

**Files:**
- Modify: `crates/app-core/src/lib.rs` (`Mode` ~1607: delete `BaseMenu`, add four; `Mode` list ~2417; `BASE_REPORT_PAGES` ~2722)
- Modify: `crates/app-core/src/app/group_menu.rs` (split `BASE_ROWS`, `command_rows`, shared dispatch, module doc)
- Modify: `crates/app-core/src/app/base_report.rs` (`handle_command_page_key`)
- Modify: `crates/app-core/src/app/input.rs` (~264 dispatch arm; ~404 highlight reset → return row)
- Modify: `crates/app-core/src/app/playing.rs` (~186 delete `b` arm; ~332 `B` arm becomes `b`)
- Modify: `crates/app-core/src/app/lifecycle.rs` (init new field)
- Modify: `crates/app-core/src/app/building.rs:34,818` (doc comments naming `base_menu_rows`)
- Modify: `crates/gui/src/render/mod.rs` (~947, ~1159, ~1686: delete `Mode::BaseMenu` arms only, so the workspace compiles; command pages draw the hub fallback until Task 2)
- Modify: `crates/gui/src/render/group_menu.rs` (doc comment: it now draws `p` only)
- Test: `crates/app-core/src/tests/base_report.rs`, `crates/app-core/src/tests/group_menus.rs`, `crates/app-core/src/tests/support.rs`, and every test file pressing `b`: `caravan`, `notifications`, `building`, `readonly_screens`, `breeding`, `contracts`, `dossier`, `research`, `dispatch` (`rg -n "Char\('b'\)|Char\('B'\)|base_menu_rows|BaseMenu" crates/app-core/src/tests`)

**Interfaces:**
- Produces:
  - `Mode::{BaseBuild, BaseResearch, BaseTrade, BaseCompile}` — hub screens, pages 1–4 of `BASE_REPORT_PAGES` with titles `"Build"`, `"Research"`, `"Trade"`, `"Compile"`.
  - `pub fn App::command_rows(&mut self, page: Mode) -> Option<Vec<GroupMenuRow>>` — `None` when `page` is not a command page; otherwise the filtered rows of that page's table.
  - `GroupMenuRow { pub label, pub(crate) target }` — unchanged.
  - `App::command_return_row: Option<usize>` (private field) — set to the picked index when a command row opens its screen; taken when the mode lands back on a command page from that screen, and that value replaces `opening_row()`'s 0. Cleared wherever `menu_origin` is cleared.
  - Test helper `pub(crate) fn open_command(app: &mut App, label: &str)` in `tests/support.rs`: finds the command page whose `command_rows` contains `label` (panic listing all four pages' rows if none), `open_hub_screen(app, page)`, then selects the row by its `menu_shortcut`. `open_hub_screen` presses `b` instead of `B`. `open_via_menu` keeps only the `'p'` arm (rename nothing else).
- Row split (from `BASE_ROWS`, order within each table as listed):
  - `BUILD_ROWS`: Deploy a structure, Work a structure yourself, Upgrade a structure, Demolish a structure
  - `RESEARCH_ROWS`: Research, Routine research, Affix research, Study a program, Hold a program
  - `TRADE_ROWS`: Contracts, Caravan, Dispatch
  - `COMPILE_ROWS`: Compile an item, Recipes
  - "Base report" row: deleted.
- Selection dispatch: extract the `match rows[idx].target { Perks | Manifest | Build | BaseOverview | target }` block from `handle_group_menu_key` into `fn open_row_target(&mut self, target: Mode)`; the party menu and `handle_command_page_key` both call it. `BaseOverview` arm goes (no row targets it now).

- [ ] **Step 1: Write failing tests** in `tests/base_report.rs` (fixtures: a `dev-saves/` template with a Home where one serves — check `support.rs` for the existing base fixture used by `base_report.rs` before writing a new one):
  - `b` on the map opens the hub at Overview, `hub_on_rail == true`; on a second open, at the last hub screen.
  - `B` on the map leaves `Mode::Playing`.
  - `b` underground opens the hub; `command_rows(BaseBuild)` contains no `Base`-locality row (Review Focus 3).
  - In base space, each command page's labels equal its table's labels for a fixture where all are available (assert per page against a literal list).
  - Founding: no Home, open grid → `command_rows(BaseBuild)` labels == `["Deploy a structure"]`; Enter opens `Mode::Build`, `build_category == None` (Review Focus 4).
  - `command_rows(Mode::BaseStock)` is `None`.
  - Return path: Build page, select Demolish → `Mode::Remove`; Esc → `Mode::BaseBuild`, `hub_on_rail == false`, `menu_selected` == Demolish's index (Review Focus 1). Then `]`, `[` → `menu_selected == 0` (Review Focus 2).
  - On the rail of a command page, a row letter leaves the mode unchanged (Review Focus 5).
  - An all-hidden page (Trade with no Broker, Caravan or relay) is still reached by `]` and `command_rows` returns `Some(vec![])`.
- [ ] **Step 2:** Run `cargo test -p feral-processes-app-core base_report` — expect compile failure (no `command_rows`, no modes).
- [ ] **Step 3: Implement** per Interfaces. `handle_command_page_key`: `selected_index(key, rows.len())` → set `menu_origin = Some(self.mode)`, `command_return_row = Some(idx)`, `open_row_target(target)`. Add the four modes to `input.rs`'s dispatch (one arm `Mode::BaseBuild | … => self.handle_command_page_key(key)`). Rebind `b` at the `B` arm's position (move its comment, update it); delete the old `b` arm and the `B` arm. Delete `Mode::BaseMenu` and `handle_base_menu_key`, `base_menu_rows`. Update `group_menu.rs`'s module doc (the base half now lives in the hub) and `Locality` docs that say "base menu". Delete the gui `BaseMenu` arms.
- [ ] **Step 4:** Run the new tests — expect PASS.
- [ ] **Step 5: Migrate existing tests.** Replace each `open_via_menu(app, 'b', label)` with `open_command(app, label)`; each raw `Char('b')` + row pick likewise; each `Char('B')` that opens the report with `Char('b')` (not `inspection`'s bay-sheet `B` in `breeding.rs` — read each call's context). Delete tests whose only subject was the base menu popup itself in `group_menus.rs`, replacing what they asserted (row hiding, row order, locality) with the per-page assertions from Step 1 if not already covered. Do not weaken any assertion about a target screen.
- [ ] **Step 6:** `cargo test -p feral-processes-app-core` all green; `cargo build -p feral-processes-gui`; fmt; clippy.
- [ ] **Step 7: Commit** explicit paths: `Base hub: b opens the base report, whose Build/Research/Trade/Compile pages replace the base menu`.

### Task 2: gui — draw the command pages and their Overview lines

**Files:**
- Modify: `crates/gui/src/render/base_report/mod.rs` (`AppScreens`, `screen_rows`, `page_flag` neighbour)
- Create: `crates/gui/src/render/base_report/commands.rs` (command-page rows)
- Modify: `crates/gui/src/render/base_report/overview.rs` (command-page status text)
- Modify: `crates/gui/src/render/mod.rs` (~1587 build `AppScreens` with the new fields)
- Modify: `crates/gui/src/render/group_menu.rs` (extract the row-formatting it shares with the command page, if `draw_group_menu` builds rows inline)

**Interfaces:**
- Consumes: `App::command_rows(Mode) -> Option<Vec<GroupMenuRow>>`, `BASE_REPORT_PAGES`, `menu_shortcut`.
- Produces:
  - `AppScreens.commands: Option<&'a [GroupMenuRow]>` — the current screen's rows when it is a command page.
  - `AppScreens.command_counts: &'a [(usize, usize)]` — `(page index, rows.len())` for each command page, computed in `render/mod.rs` by calling `command_rows` on each page's first screen.
  - `fn group_row(i: usize, label: &str, selected: bool) -> Row` (in `group_menu.rs`, `pub(super)`) — the `[a] Label` row both the `p` popup and `commands::command_rows` call; no second copy of the format.
  - `commands::command_rows(rows: &[GroupMenuRow], selected: usize) -> Vec<Row>` — one row per command; empty slice → one dim text row `"Nothing here can be done from where you are."`.

- [ ] **Step 1: Write failing tests** (`#[cfg(test)]` in `commands.rs` and `overview.rs`, headless, following `base_report::fit`):
  - Empty rows → the single explanatory row.
  - Three rows → three item rows `[a]`/`[b]`/`[c]`, the selected one highlighted.
  - Width census: the widest label of all four tables as a row fits the hub content rect (follow `fit`'s existing census; labels as literals with a comment pointing at `group_menu.rs`'s tables — gui cannot build a `Game`).
  - Overview: a command page's line reads `"N available"` (dim), or `"none here"` (dim) at 0; report pages keep their flag/"ok" text.
- [ ] **Step 2:** `cargo test -p feral-processes-gui base_report` — expect FAIL.
- [ ] **Step 3: Implement.** `screen_rows` arm for the four modes reads `app_screens.commands`. Overview's status: a command page uses its count from `command_counts`, never yellow. Move the base menu's refusal line onto the command page (it already flows through `draw_base_report`'s `refusal`; verify a `refuse` from a command row shows there).
- [ ] **Step 4:** Tests pass; fmt; clippy.
- [ ] **Step 5: Screenshot check:** `cargo run -- --template stack --keys "b ] Right" --screenshot <scratchpad>/build.png` and a base template (`dev-saves/README.md` lists them) with `"b ]"`; Read both PNGs; confirm rows sit in the content rect and the rail shows twelve pages without clipping. If the rail overflows its height, stop and report — do not resize the frame unasked.
- [ ] **Step 6: Commit:** `Base hub: draw the command pages and their Overview counts`.

### Task 3: text — help pages and tutorials

**Files:**
- Modify: `assets/help/20-controls.md:23,61` — one `b` line: the base report, its rail, the four command pages and what each holds; delete the `B` line.
- Modify: `assets/help/60-your-base.md:82,149,166`, `assets/help/63-staff-morale.md:62`, `assets/help/65-contracts.md:4` — "base menu"/"B" → `b` and the named page.
- Modify: `assets/contracts/tutorial_break_ground.ron`, `tutorial_arm_a_decompile.ron`, `tutorial_man_the_node.ron` — same rewrite; "Tab to the structure roster" → "Right to the structure roster" (Tab was retired in 0.18.26).
- Leave `assets/help/46-breeding.md:12` (bay sheet `B`).

- [ ] **Step 1:** `rg -n -i "base menu|\bB\b for|press B|\(B\)|Tab to" assets` — list every hit; fix each.
- [ ] **Step 2:** If a help or contract test pins text (`rg -n "base menu" crates/*/src`), update it. `cargo test -p feral-processes-engine help contract` and `cargo test -p feral-processes-app-core` green.
- [ ] **Step 3: Commit:** `Help and tutorials: the base commands live in the b hub`.

### Final gate

- [ ] `cargo test --workspace` green; clippy clean.
- [ ] Whole-branch review (opus) against the spec and the Review Focus list; diff as a file.
- [ ] Release at landing via the `deploy` skill (CHANGELOG `## X.Y.Z`; minor-vs-patch per `CHANGELOG.md`'s preamble — a key change with no save break).
