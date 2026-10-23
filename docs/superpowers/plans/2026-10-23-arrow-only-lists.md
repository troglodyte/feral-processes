# Arrow-only List Selection Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Every pick list is driven by highlight + Enter. Positional row
labels (`menu_shortcut`: `1`–`9`, then `a`…) are gone everywhere except the
classic battle screen.

**Architecture:** `App::selected_index` (app-core `app/input.rs:113`) loses
its digit/letter branch. The classic battle screen gets its own resolver,
which handles digits and letters and then defers to `selected_index`.
Renderers stop printing labels, except on the battle screen. Tests move to
a `pick_row` helper *before* the keys are removed, so the suite stays green
at every commit.

**Tech Stack:** Rust workspace; `crates/app-core` (input), `crates/gui`
(bevy/egui renderers).

**Spec:** `docs/superpowers/specs/2026-10-23-arrow-only-lists-design.md` (read
it first, especially "Found by the inventory").

## Global Constraints

- Word letters stay: main menu `n/l/a/o/r/d/q`, confirm `y/n`, and every
  screen's own lowercase mnemonic (alerts `x/d`, routine install `e`,
  creation `n`, party `r`, sprite picker `t`, and the rest).
- UPPERCASE action keys stay.
- The classic battle screen keeps digit and letter row picks and labels,
  including the target screen's group letters (`app/battle.rs:299`). The
  tactical map (`app/tactical.rs`, `render/tactical.rs`) goes arrow-only.
- Lowercase letters do nothing inside a converted list.
- No dual path or flag that keeps the old scheme alive. `cargo fmt` and
  `cargo clippy --workspace --all-targets` must be clean at every commit.
- Do not touch `docs/manual.md` or `README.md`. Do update `assets/help/`.
- Stale "collides with `menu_shortcut`'s digits-then-lowercase scheme"
  comments (`app/party.rs:128,204,210,351`, `app/progression.rs:14,99`,
  `app/trade.rs:115`, `app/menus.rs:115,233`, `app/building.rs:976`,
  `lib.rs:161-171`, `app/world_map.rs:16,58`) are rewritten to their
  remaining *why*, or deleted, in the task that removes the scheme.
- Work in a worktree on branch `arrow-only-lists`. Never push, `git
  checkout` uncommitted work, `git stash` or `git add -A`; stage explicit
  paths.

## Review Focus

1. **A list with no visible highlight.** With the labels gone, it can't be
   used. Task 8 sweeps every list renderer, and each one either passes
   `selected` through or gets it added.
2. **A lowercase mnemonic that silently stops working, or a freed letter
   that now does something.** Task 4 adds a test per mnemonic screen that
   presses its letter and asserts the action, and asserts that a non-mnemonic
   letter leaves the mode and `menu_selected` unchanged.
3. **Irreversible confirms still open on "No"**
   (`opening_row`, `app/input.rs:196`). Task 4 adds one test per mode
   listed there: open it, press Enter, and assert the destructive action did
   not run.
4. **Battle digits still work after the removal.** Task 2's test presses a
   digit on each battle picker. Task 4 must not change it, and it is the
   mutation check for the battle resolver.
5. **The world map with an empty place list.** Tab to the list, then
   Up/Down/Enter/`C`, must not panic or move the pan centre (Task 6).

---

### Task 1: `pick_row` / `highlight_row` test helpers

**Files:**
- Modify: `crates/app-core/src/tests/support.rs` (helpers near `open_via_menu` at 2788 and `open_command` at 2828)
- Test: `crates/app-core/src/tests/menus.rs`

**Interfaces:**
- Produces: `pub(crate) fn highlight_row(app: &mut App, row: usize)`
  presses Up or Down from `app.menu_selected` until it equals `row`, and
  panics if `row` can't be reached within `row + 64` presses. Also
  `pub(crate) fn pick_row(app: &mut App, row: usize)`, which is
  `highlight_row` followed by `GameKey::Enter`.

- [ ] **Step 1: Write the failing test** `pick_row_picks_from_a_non_zero_opening_row`
  in `tests/menus.rs`. Open a screen whose `opening_row` is 1 (an
  `ExtractConfirm` from an existing fixture), call `pick_row(&mut app, 0)`,
  and assert the confirm resolved as Yes. Add `pick_row_reaches_row_twelve`,
  which opens a menu with ≥13 rows (help topics or inventory),
  `highlight_row(&mut app, 12)`, then `assert_eq!(app.menu_selected, 12)`.
- [ ] **Step 2:** `cargo test -p feral-processes-app-core pick_row` must FAIL to compile (no helper).
- [ ] **Step 3:** Implement both helpers. Switch `open_via_menu` and `open_command` from `Char(menu_shortcut(idx))` to `pick_row(app, idx)`.
- [ ] **Step 4:** `cargo test -p feral-processes-app-core` must PASS.
- [ ] **Step 5:** Commit with the message "Tests: pick rows by highlight + Enter".

### Task 2: Battle-only row resolver

**Files:**
- Modify: `crates/app-core/src/app/battle.rs` (callers at :303, :379, :451, :492, :542)
- Modify: `crates/app-core/src/app/input.rs`
- Test: `crates/app-core/src/tests/battle.rs`

**Interfaces:**
- Produces: `pub(crate) fn battle_row(&mut self, key: GameKey, len: usize) -> Option<usize>`
  in `app/battle.rs`. It resolves digits (`1`→0, `0`→None) and lowercase
  letters (`DIGIT_ROWS + (c - 'a')`) exactly as `selected_index` does
  today, and passes every other key to `selected_index`. Move the
  digit/letter code out of `selected_index` into a private `fn
  shortcut_row(c: char, len: usize) -> Option<usize>` in `input.rs` that
  both call. `selected_index` stops calling it in Task 4; the move here is
  what makes the later removal a one-line diff.

- [ ] **Step 1:** Test `battle_pickers_take_digits_and_letters`: on each of
  special, ally, emulate and item, pressing `Char('2')` picks row 1. On a
  picker with ≥10 rows (or the target screen's group letters),
  `Char('a')` picks row 9 / group 0. Use the arena or battle fixture in
  `tests/battle.rs`.
- [ ] **Step 2:** Run it. It PASSES on today's code, which is intended: it pins behaviour that Task 4 must keep.
- [ ] **Step 3:** Add `battle_row` and `shortcut_row`, and route the five battle callers through `battle_row`. Tactical callers (`app/tactical.rs:239,259,301`) stay on `selected_index`.
- [ ] **Step 4:** `cargo test -p feral-processes-app-core battle` must PASS.
- [ ] **Step 5:** Commit with the message "Battle: own row resolver ahead of arrow-only lists".

### Task 3: Migrate non-battle tests to `pick_row`

**Files:**
- Modify: every `crates/app-core/src/tests/*.rs` that presses `Char('1'..'9')`
  or `Char(menu_shortcut(..))` *as a row pick* on a non-battle screen.
  Largest first: building, menus, routines, field, sprite_forge, inventory,
  transfer, breeding, base_report, quitting, develop.
- Skip: digits used as quantities (basket, crafting, trade, building,
  inventory quantity entry), pane/frame keys (`info_tab.rs`, the
  `playing.rs:399` and sprite editor `Char('1')`), and battle-screen tests.

**Interfaces:**
- Consumes: Task 1's helpers.

- [ ] **Step 1:** For each file, replace each row-pick press with `pick_row(&mut app, n)`. Where the test only moved the highlight (no pick expected), use `highlight_row`. Keep every assertion unchanged.
- [ ] **Step 2:** After each file, `cargo test -p feral-processes-app-core <file stem>` must PASS. Commit every few files with the message "Tests: <files> pick rows by highlight".
- [ ] **Step 3:** Leave the tests that *assert* the shortcut scheme (the list under "Task 4 Step 1") alone; Task 4 rewrites them.
- [ ] **Step 4:** Gate: `rg -n "Char\('[1-9]'\)|menu_shortcut" crates/app-core/src/tests` must list only battle tests, quantity/pane keys, and Task 4's shortcut-scheme tests.

### Task 4: Remove positional selectors from `selected_index`

**Files:**
- Modify: `crates/app-core/src/app/input.rs` (`selected_index`; `yes_no` digit guard can stay, with its comment rewritten)
- Modify: `crates/app-core/src/app/inspection.rs:333-342` (`manifest_nav_key`: drop the Char jump)
- Modify: the stale comments listed in Global Constraints
- Test: `crates/app-core/src/tests/menus.rs` and the files below

- [ ] **Step 1:** Rewrite the shortcut-scheme tests so they assert the new
  rule:
  - `tests/menus.rs` `every_row_shortcut_selects_the_row_it_labels`,
    `row_shortcuts_run_digits_first_then_letters`,
    `letters_pick_no_row_in_a_menu_shorter_than_ten_rows`,
    `zero_picks_no_row_rather_than_underflowing` and
    `uppercase_letters_pick_no_row` collapse into
    `digits_and_lowercase_letters_pick_no_row`. On a ≥12-row menu, each of
    `'0'..='9'` and `'a'..='z'` leaves the mode and `menu_selected`
    unchanged.
  - `a_digit_or_lowercase_letter_jumps_to_that_subject_and_keeps_the_face`
    (191) becomes `a_digit_or_letter_does_not_jump_the_manifest`.
  - Rewrite these to the arrow equivalent, or invert them: extraction.rs:52,
    dossier.rs:37, excavate.rs:381, party.rs:289, creation.rs:1551,
    research.rs:84, respec.rs:74 (keep "x never wipes"), alerts.rs:100,121
    (keep "x/d act, they are mnemonics"), and the neutral tests at
    base_report.rs 101/205/842, routines.rs 258, building.rs 898,
    breeding.rs 28, depot_filter.rs 89, level_up.rs 173, program_points.rs
    45, machine_recipe.rs 145 and readonly_screens.rs 151.
  - Leave outposts.rs, world_map.rs and dev_console.rs tests for Tasks 5–7.
  - Review Focus 2: add `mnemonic_letters_still_act` (one assertion per
    mnemonic screen in Global Constraints). Review Focus 3: add
    `irreversible_confirms_ignore_a_bare_enter` (one case per mode in
    `opening_row`'s confirm arm).
- [ ] **Step 2:** Run them. The rewritten negatives FAIL on today's code.
- [ ] **Step 3:** Remove the `shortcut_row` call from `selected_index`, drop the manifest jump, and rewrite the stale comments.
- [ ] **Step 4:** `cargo test -p feral-processes-app-core` must PASS, and Task 2's battle test must pass with no edits. Mutation check: re-add the `shortcut_row` call to `selected_index`, confirm `digits_and_lowercase_letters_pick_no_row` fails, then revert.
- [ ] **Step 5:** Commit with the message "Lists: digits and letters no longer pick rows".

### Task 5: Outposts onto the shared highlight

**Files:**
- Modify: `crates/app-core/src/app/outposts.rs:73-77,123-126`
- Modify: `crates/gui/src/render/outposts.rs` (or wherever OutpostVisit/OutpostPost draw, found with `rg -l Outpost crates/gui/src/render`)
- Test: `crates/app-core/src/tests/outposts.rs` (89, 126, 240)

- [ ] **Step 1:** Tests: `outpost_visit_arrows_move_the_crew_highlight`
  (Down then `U` recalls row 1) and `outpost_post_posts_the_highlighted_candidate_on_enter`
  (Down, Enter posts candidate 1). `outpost_letters_do_nothing`: `'a'`
  changes nothing. Rewrite the three existing letter tests to these.
- [ ] **Step 2:** Run them. They FAIL.
- [ ] **Step 3:** Replace the `c - 'a'` parsing with `selected_index`. The renderer passes `i == menu_selected` to its row helper and drops the letter labels.
- [ ] **Step 4:** `cargo test -p feral-processes-app-core outposts` and `cargo test -p feral-processes-gui outpost` must PASS.
- [ ] **Step 5:** Commit with the message "Outposts: crew rows by highlight".

### Task 6: World map Tab focus

**Files:**
- Modify: `crates/app-core/src/app/world_map.rs`, `crates/app-core/src/lib.rs` (new `App` field and `Mode::WorldMap` doc at 2209)
- Modify: `crates/gui/src/render/world_map.rs`
- Modify: `assets/help/20-controls.md:85` (the `g` paragraph)
- Test: `crates/app-core/src/tests/world_map.rs`

**Interfaces:**
- Produces: `pub world_map_list_focused: bool` on `App`, set to `false` in
  `open_world_map`. gui reads it.

- [ ] **Step 1:** Tests:
  - `world_map_opens_with_map_focus_and_arrows_pan`.
  - `tab_moves_focus_to_the_list_where_up_down_move_the_highlight_and_the_centre_stays`.
  - `tab_again_returns_arrows_to_panning`.
  - `c_points_at_the_highlighted_place_under_either_focus`.
  - `letters_pick_no_place`.
  - `an_empty_list_survives_tab_up_down_enter_c` (Review Focus 5).
  - Rewrite `a_letter_selects_a_row_and_arrows_do_not` (56) to the Tab
    version.
- [ ] **Step 2:** Run them. They FAIL.
- [ ] **Step 3:** Implement. With the list focused, Up/Down/PageUp/PageDown
  go through `selected_index` (Enter is inert, per the spec); with the map
  focused they pan. The renderer draws the list highlight only while
  `world_map_list_focused` is true, and adds a one-word focus cue in the
  list header. Check that the cue fits with the popup width test helpers
  (`gui-text-never-clips-or-wraps`). If the Stack frame map (`FrameMap`)
  has a picked list, apply the same rule there, or record that it has none.
- [ ] **Step 4:** Rewrite the help paragraph: the arrows pan; Tab switches to the place list, where Up/Down pick a place; then C, P and Esc as before.
- [ ] **Step 5:** `cargo test -p feral-processes-app-core world_map` and `cargo test -p feral-processes-gui world_map` must PASS. Commit with the message "World map: Tab switches between map and place list".

### Task 7: Dev pickers as highlighted lists

**Files:**
- Modify: `crates/app-core/src/app/dev_console.rs:34,390-423`, `app/dev_item_picker.rs`, `app/dev_zone_picker.rs`
- Modify: `crates/gui/src/render/dev_pickers.rs` (rows at 15-30; test helper at 84)
- Test: `crates/app-core/src/tests/dev_console.rs` (351)

**Interfaces:**
- Removes: `scroll_dev_picker`, `dev_picker_pick`, `dev_picker_page` and
  `DEV_PICKER_PAGE`.
- Produces: `dev_item_rows(&self) -> Vec<(String, u32)>` and the zone
  equivalent, which return *all* rows. The popup's own window follows
  `menu_selected`, as in every other list.

- [ ] **Step 1:** Tests:
  - `give_item_enter_gives_the_highlighted_item`: Down×30, then Enter gives
    10 of item 30.
  - `give_item_up_from_the_top_wraps_to_the_last_item`.
  - The zone-picker equivalent of Enter.
  - Replace `scrolling_exposes_the_rows_past_the_last_selector` (351) with
    `highlight_reaches_the_last_item`.
- [ ] **Step 2:** Run them. They FAIL.
- [ ] **Step 3:** Both handlers use `selected_index`; delete the paging helpers. The renderer passes `i == menu_selected`, drops labels and lets the popup window follow the highlight. Update the module header ("Neither has a highlight").
- [ ] **Step 4:** `cargo test -p feral-processes-app-core dev_` and `cargo test -p feral-processes-gui dev_pickers` must PASS. Commit with the message "Dev pickers: highlight + Enter".

### Task 8: Renderers drop labels (battle excepted)

**Files:**
- Modify: every `crates/gui/src/render/**` file that calls `menu_shortcut`
  (`rg -l menu_shortcut crates/gui/src`), except `render/battle.rs`.
  Includes `render/tactical.rs:1708,1733`, `render/mod.rs:442`
  (`emulation_row_label`), and the help-topics label `app/menus.rs:196`
  (`shortcut:` field).
- Modify: gui label/width tests: `render/popup.rs:1891,2211`,
  `render/building.rs:3237`, `render/progression.rs:891`, and any other
  test that breaks.
- Modify: `crates/app-core/src/lib.rs:161-180` doc comment on `menu_shortcut`
  (now "the battle screen's row labels").
- Modify: `assets/help/*.md` wherever it tells the player to press a row
  number or letter (`rg -n -i "1-9|a-z|number key|row letter" assets/help`).

**Interfaces:**
- Changes: `emulation_row_label(i: usize, option: &EmulationOption, labelled: bool) -> String`.
  Battle passes `true`, tactical `false`. If the help topic struct's
  `shortcut` field has no remaining reader, delete it.

- [ ] **Step 1:** Test `no_list_renderer_outside_battle_names_menu_shortcut`
  in `crates/gui/src/render/mod.rs` tests, as a source census:
  `include_str!` every render file except `battle.rs` and assert that none
  contains `menu_shortcut(`. `render/battle.rs:2124` must keep asserting its
  label.
- [ ] **Step 2:** Run it. It FAILS, listing the files.
- [ ] **Step 3:** Remove the `[x] ` prefix from each row and reflow the
  columns. Review Focus 1: for every list row, confirm `selected` is
  `i == app.menu_selected` and not a hard-coded `false`. Eyeball the sites
  the inventory flagged: settlement_market.rs:109, caravan.rs:120,205,
  stack_market.rs:43,77, extraction.rs:33,86, crafting.rs:82, field.rs:230
  and machine_recipe.rs:88. Fix the width tests to the new strings.
- [ ] **Step 4:** Screenshot check. Run `cargo run -- --template stack --keys "i" --screenshot /tmp/…/inv.png` (and one tactical shot through the arena, per memory `battle-screenshot-goes-through-the-arena`), Read the PNGs, and confirm that the highlight shows and no labels are drawn.
- [ ] **Step 5:** `cargo test -p feral-processes-gui` must PASS. Commit with the message "Renderers: no row labels outside the battle screen".

### Task 9: Gate and hand-off

- [ ] `cargo fmt`, then `cargo clippy --workspace --all-targets` must be clean.
- [ ] Run `cargo test --workspace` with `FERAL_DEV_NO_SIEGES` unset. It must PASS. `balance_sim` is unaffected (no tuning change), so don't run it separately.
- [ ] `rg -n "menu_shortcut|DIGIT_ROWS" crates` must hit only battle code, `lib.rs` and battle tests.
- [ ] Do a whole-branch review (opus). The diff goes to the reviewer as a file. Include the spec and this plan's Review Focus.
- [ ] Retire memory `lowercase-letters-are-row-selectors.md` and its MEMORY.md line, and replace it with "lists are highlight + Enter; only the classic battle screen keeps row digits".
- [ ] Add an entry to `docs/superpowers/INDEX.md`. The `CHANGELOG.md` entry is written at release (deploy), not on the branch.
