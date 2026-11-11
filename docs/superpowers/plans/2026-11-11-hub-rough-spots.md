# Hub rough spots Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every page of `b`/`i` stays inside the hub frame (research tree included), Roster shows party + compact bench, towns carry a heading, and the key bar is the one place keys are explained.

**Architecture:** The research tree's draw takes the hub content `Rect` instead of the window; app-core stops the tree claiming keys while the rail has focus. Popup key footers become a `Row` variant the hub page skips. Town headings come from the engine's existing `stack::heading`.

**Tech Stack:** Rust workspace — `crates/engine`, `crates/app-core`, `crates/gui` (Painter seam).

**Spec:** `docs/superpowers/specs/2026-11-11-hub-rough-spots-design.md`

## Global Constraints

- `render/` draws only through `Painter`; panes take their origin from a `Rect`, never a literal `0.0` (`.claude/rules/drawing-seam.md`).
- Pointer targets are registered by the draw that places them; every `PointerHit` ends in a key the keyboard already presses (ADR 0046).
- `App::screen_claims` is the one place a page claims an arrow/Esc/brace (ADR 0045).
- No save-format change; no engine town renaming; key set unchanged.
- Key-bar words: `← list`, `Esc list` on a page; `Esc close` on the rail.
- Roster's first tab is named `Party`; page title stays `Roster`.
- Town rail label: `"<name> <heading label>"` using `Heading::label()` (e.g. `"Tally Yard north-east"`); no suffix at `Heading::Here` or with no anchor.
- Every commit: `cargo fmt`, `cargo clippy --workspace --all-targets` clean. No push. Never `git checkout`/`git stash` someone else's work; stage explicit paths.
- Branch commits stay unversioned (no CHANGELOG version section on the branch; the deploy adds it).

## Review Focus

1. **Smallest window (1280x720) with the tree in the page** — the tree pane is ~22 cells narrower than before; the detail panel must stay readable and `view_hint` must still count hidden columns. Test in Task 3.
2. **Rail resting on Research while a refusal line is up** — content rect shrinks by one line; the tree must not draw under the key bar. Test in Task 3.
3. **Clicking a tree cell** — must select it (first click) and work it (second click / double click), same as a row. Test in Task 3.
4. **A footer key with no key-bar entry** (e.g. Caravan's basket step keys, Breeding's pair keys) — removing the footer must not hide a key. Census in Task 4.
5. **Two towns with the same name in the same direction** — labels are still identical; acceptable (spec says direction, not uniqueness), but the rail must not panic or merge pages (pages are keyed by `RouteDestinationId`, not name). Test in Task 1.

---

### Task 1: Town rail labels carry a heading; the kind glyph leaves the rail

**Files:**
- Modify: `crates/engine/src/game/sites.rs:133-162` (`site_rail`)
- Modify: `crates/app-core/src/app/sites_hub.rs:58` (`site_pages`)
- Test: engine sites tests (beside existing `site_rail` tests; `rg -n "site_rail" crates/engine/src/tests`), app-core `sites_hub.rs` tests

**Interfaces:**
- Produces: `pub(crate) fn town_rail_label(name: &str, tile: (i32, i32), anchor: Option<(i32, i32)>) -> String` in `crates/engine/src/game/sites.rs` — pure.
- `SiteRailEntry.name` for a settlement now holds the label; `SiteRailEntry.glyph` unchanged (Status tab and other readers keep it).

- [ ] **Step 1: failing tests**
  - `town_rail_label_appends_the_heading_from_the_base`: `town_rail_label("Tally Yard", (10, -10), Some((0, 0)))` == `"Tally Yard north-east"` (check `heading`'s y-axis convention in `stack.rs:119` and pick the tile so the expected heading is north-east under it).
  - `town_rail_label_is_bare_at_the_base_or_without_one`: `(0,0)` with anchor `(0,0)` → `"Tally Yard"`; anchor `None` → `"Tally Yard"`.
  - `same_named_towns_stay_separate_pages`: `site_rail()` on a game with two known settlements of one name in different directions yields two entries with distinct `site` and distinct `name`. Use the `settlements` dev-save template or the engine support fixture that places known settlements.
  - app-core: `town_pages_draw_no_kind_glyph`: `site_pages(...)` for a settlement entry has `glyph == None` (check `HubPage.glyph`'s type; if it's a `char`, use the type's "none" form). Outpost and Caravans entries unchanged.
- [ ] **Step 2:** run `cargo test -p feral-processes-engine town_rail_label` and the app-core test → FAIL.
- [ ] **Step 3:** implement `town_rail_label` over `stack::heading(dx, dy).label()`; call it in `site_rail` with `known.tile` and `self.anchor_position()`. Sort stays by def name (sort before labelling). In `site_pages`, drop the glyph for `SiteSection::Settlements`.
- [ ] **Step 4:** tests PASS; `rg -n '\.name' ` readers of `SiteRailEntry` to confirm no reader expected the bare name (a Status-tab heading that repeats the name is fine).
- [ ] **Step 5:** commit `Hub rough spots 1: towns on the rail carry their heading from the base, no kind glyph`.

### Task 2: The research tree claims nothing while the rail has focus

**Files:**
- Modify: `crates/app-core/src/app/hub.rs:529` (the `GameKey::Left if self.research_tree_shown()` arm)
- Test: app-core hub tests (`rg -n "research_tree_shown|graph_view" crates/app-core/src --type rust` for the existing ones)

**Interfaces:**
- Produces: `screen_claims` returns `false` for every key while `hub_on_rail`, research modes included.

- [ ] **Step 1: failing tests** (drive real key presses from `b`):
  - `resting_on_research_leaves_the_rail_keys_to_the_rail`: open `b`, `Down` to Research, assert `hub_on_rail`, press `Down` → rail moves to the next page (Alerts), `mode` no longer a research mode; press `Up` back, `Left` → still on rail, `Esc` → hub closes.
  - `right_enters_the_tree_and_left_from_its_edge_returns`: from rail on Research press `Right` → `!hub_on_rail`; press `Left` until the tree's left column; one more `Left` → `hub_on_rail`.
- [ ] **Step 2:** run → the first test FAILs (Left arm claims while on rail) or passes partially; record which.
- [ ] **Step 3:** guard the Left arm with `&& !self.hub_on_rail` (the Esc/brace arm already has it). Update its comment.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `Hub rough spots 2: the research tree takes no key while the rail has focus`.

### Task 3: The research tree draws inside the hub page

**Files:**
- Modify: `crates/gui/src/render/research_graph.rs:247` (`geometry`), `:409` (`draw_research_graph`), `:633-638` (hint/refusal baselines), its tests at `:1054-1158`
- Modify: `crates/gui/src/render/mod.rs:1347-1362` (hub `page_rows` research arm) and `:1782-1845` (`draw_mode_overlay` graph arms)
- Test: `research_graph.rs` tests module

**Interfaces:**
- `pub(super) fn geometry(area: Rect, graph: &ResearchGraph, m: &Metrics) -> GraphGeometry` — `area` replaces `screen_w, screen_h`; header/footer/panel split measured inside it.
- `pub(super) fn draw_research_graph(game: &mut Game, tree: ResearchTree, selected: usize, refusal: Option<&str>, area: Rect, targets: Option<&RowTargets>, painter: &Painter, m: &Metrics, dev_unlock: bool)` — no full-window backdrop; paints `area` only. Use whatever the hub frame lends rows for target registration (`frame.row_targets()`'s type); pass `None` when the rail has focus.
- Each drawn cell registers `PointerHit::Row(i)` with `i` = the index `selected` uses for that node. Verify against `App::handle_pointer_event` that `Row(i)` on a research mode sets `menu_selected = i` and a second hit is `Enter`; if research modes are excluded there, add them in app-core with a test.
- The tree's own key hint line (`[G] list · Arrows move · Tab tree · Enter work`) moves to `screen_keys(Mode::Research | RoutineResearch | AffixResearch | WeaponResearch)` in `base_report/frame.rs:460` as `G list`, `↑↓←→ move`, `Enter work`; `Tab` is already the hub's.

- [ ] **Step 1: failing tests**
  - `tree_draws_only_inside_its_area`: headless painter, `area = Rect::new(300., 60., 900., 560.)`; every recorded rect/text lies within `area` (reuse the existing painter-recording helper the `:1054` tests use).
  - `tree_keeps_the_cursor_in_view_in_a_narrow_area`: at the 1280x720 hub content width, select the right-most node; it is drawn fully inside `area` and `view_hint` reports hidden columns on the left.
  - `tree_registers_a_target_per_drawn_cell`: targets recorded == drawn cells, each `PointerHit::Row(i)` matching the node's selection index; none when targets is `None`.
  - Update the existing `:1054-1158` tests to pass a window-sized `Rect`; the `PANEL_FRACTION` width test at `:906` re-measured at the hub content width (if `Self-Execution` no longer fits, widen via the fraction or wrap — builder's call, spec "Left to the builder").
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** implement the signatures; in `mod.rs`'s hub path, for a research mode with `graph_view`, call `frame.paint(...)` for the content rect and then `draw_research_graph(..., content, ...)` instead of falling through to `draw_mode_overlay`. Delete the four `if graph_view` arms in `draw_mode_overlay` if no non-hub path reaches a research mode (`rg -n "Mode::Research\b" crates/app-core/src` — check every way in); otherwise they pass the window `Rect`. Replace the "draws over the hub" comment at `:1347`.
- [ ] **Step 4:** PASS; screenshot check: `${CARGO_TARGET_DIR:-target}/debug/feral-processes --template settlements --keys "b Down Down Down Down Down" --screenshot <scratch>/r.png` shows the rail plus the tree in the page; `... Right` shows the cursor in the tree.
- [ ] **Step 5:** commit `Hub rough spots 3: the research tree draws inside the hub page`.

### Task 4: Pages draw no key footer; the key bar carries their keys

**Files:**
- Modify: `crates/gui/src/render/popup.rs:11` (`Row`), `:383` (`push_key_footer`), `draw_popup`'s row draw
- Modify: `crates/gui/src/render/base_report/mod.rs:119` (`draw_hub_page`) / `draw_rows_in`
- Modify: `crates/gui/src/render/base_report/frame.rs:460` (`screen_keys`) as the census demands
- Test: `base_report/frame.rs` tests (beside the existing keybar censuses at `:637-706`)

**Interfaces:**
- Produces: `Row::Keys(String)` — a key-hint line. `push_key_footer` pushes the blank spacer and `Row::Keys(key_hints(chips))`. `draw_popup` draws it as `Text` was drawn; `draw_rows_in` called from the hub page skips it **and** the blank spacer row directly before it.
- Produces: `fn footer_keys(rows: &[Row]) -> Vec<String>` (test helper or `pub(super)`): the leading key token of each chip in every `Row::Keys`.

- [ ] **Step 1: failing tests**
  - `hub_pages_draw_no_key_footer`: for every `Mode` the hub `page_rows` match builds (mod.rs:~1250-1362 — list them in a const in the test, plus `Mode::Build` with and without a category open), build the rows from a `settlements` template game and assert the drawn rows (after `draw_rows_in`'s skip) contain no key-hint text — assert via a recording painter that no drawn text contains `KEY_HINT_SEP`.
  - `every_dropped_footer_key_is_on_the_key_bar`: for each of those modes, every token from `footer_keys(rows)` appears as a keycap in `keybar_groups(mode, …, on_rail = false, …)`. Tokens compare case-sensitively after mapping `Left/Right`→`←/→` style names the bar uses (state the mapping table in the test).
  - `popups_keep_their_footer`: `draw_build_menu` (popup path) still draws `Enter choose  ·  Esc back`.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** add the variant and the skip; add missing keys to `screen_keys` arms until the census passes. Do not change any key handler.
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `Hub rough spots 4: hub pages draw no key footer of their own; the key bar carries their keys`.

### Task 5: Key-bar words and Roster → Party tab

**Files:**
- Modify: `crates/gui/src/render/base_report/frame.rs:436-450` (`keybar_groups`), tests at `:637`, `:665`, `:706`
- Modify: `crates/app-core/src/app/inventory_hub.rs:19` (`ROSTER_TABS`)
- Test: same files

**Interfaces:**
- On the rail: last group `Esc close`. On a page: `← list`, `Esc list`.
- `ROSTER_TABS[0]` = `HubTab::new("Party", Mode::Companion)`.

- [ ] **Step 1: failing tests** — update the three keybar assertions to the new words (`"← list"`, `"Esc list"`, rail `"Esc close"`); add `roster_strip_does_not_echo_its_title`: the Roster page's tab names do not include its title.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** change the strings; `rg -n '"Roster"' crates assets/help` and fix any test, help markdown or census naming the tab (the page title stays "Roster").
- [ ] **Step 4:** PASS.
- [ ] **Step 5:** commit `Hub rough spots 5: the key bar calls the page list "list"; Roster's first tab is Party`.

### Task 6: Roster is the party plus a compact bench

**Files:**
- Modify: `crates/gui/src/render/party.rs:338` (`companion_page_rows`), `:417` (`roster_rows`)
- Test: `party.rs` tests (`:786-860` already build `pet(.., ProgramRole::Staff)` fixtures)

**Interfaces:**
- `companion_page_rows(pets, selected, columns)` keeps its signature. A `ProgramRole::Staff` pet renders as one `Row::Item` (no detail line) headed `Bench`, text `"<glyph> <name> Lv<level>  HP <hp>/<max>"`; every other role keeps today's rows. Row::Item ordinals are unchanged (one Item per pet, as today), so `P`/selection indices still match `owned_pets()`.

- [ ] **Step 1: failing tests**
  - `staff_render_as_one_bench_line_each`: pets `[party "aa", staff "cc", staff "dd"]` → headings `["In your party", "Bench"]`; the `cc`/`dd` items are each one row with no following detail row and contain no `Assembly:`/`Extraction:`/potential text.
  - `bench_selection_ordinals_match_owned_pets`: the `Row::Item` ordinal of `dd` equals its index in `pets`.
  - Update the existing `:799`/`:856` heading assertions from `"Base staff"` to `"Bench"`.
- [ ] **Step 2:** FAIL.
- [ ] **Step 3:** implement; rename `role_heading(ProgramRole::Staff)` to `"Bench"`.
- [ ] **Step 4:** PASS; `P` on a bench row still toggles party membership (existing app-core `companion_action` test, or add `p_on_a_bench_row_adds_it_to_the_party` driving keys).
- [ ] **Step 5:** commit `Hub rough spots 6: Roster shows the party in full and base staff as a one-line bench`.

### Task 7: Seam docs and final gate

**Files:**
- Modify: `.claude/rules/seams-screens.md` (hub bullet: the research tree draws in the page and claims nothing from the rail; key hints live only in the key bar — `Row::Keys`), `seams` skill hub/screens section (the trap: a popup row builder reused as a hub page carries its footer), memory graph `seam:` entity for the hub per the `seams` skill's two calls.
- Modify: `CHANGELOG.md` — none on the branch (deploy writes it); put the player-facing sentence in the final commit message instead.

- [ ] **Step 1:** docs edits as above.
- [ ] **Step 2:** `cargo fmt && cargo clippy --workspace --all-targets` clean; `cargo test --workspace` green (`set -o pipefail` if piped).
- [ ] **Step 3:** screenshots of `b` rail on Research, tree entered, a town list with headings, Roster, Build page — read each PNG.
- [ ] **Step 4:** commit `Hub rough spots 7: seam docs`.
- [ ] **Step 5:** one final opus review of the branch (diff as a file), one fix wave for Critical/Important, one scoped sonnet check, stop; Minors to the user.
