# Alert Board Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A capped, saved, player-dismissed list of production blockers and program events, opened with `N`, with an unread badge on the status bar.

**Spec:** `docs/superpowers/specs/2026-09-23-alert-board-design.md`. Read it before Task 1. Its decisions table and the Sources table are what this plan implements, with the corrections below.

**Shape:** three phases, each one dispatch-sized. Phase 1 is the engine core and the save. Phase 2 hooks the sources and touches nothing from Phase 1 except by calling `alerts::post`. Phase 3 is app-core plus gui. Each phase ends green and committed, so a session can `/clear` between phases and start again from this file.

---

## Corrections to the spec (found by checking the source)

1. **Program downed is posted inside `Game::bench_or_dissolve` (`game/trade.rs:640`), not by its callers.** Two of the five callers write no downed line where they call it. The sortie's line is written ticks later in `return_sortie` (`game/sortie.rs:620`), and `combat_teardown.rs:436` writes none at all. `bench_or_dissolve` is the one place every downed program passes through, on both difficulty arms, so posting there is one door and misses no caller. Read the `ProgramId` at the **top** of the function: the Permadeath arm despawns the entity. The text is a sentence the engine builds from the name the function already gets (for example `"{name} is down."` on Forgiving and `"{name} was lost."` on Permadeath). It is not any caller's log line, because those lines differ from one caller to the next.
2. **Sweep hit and off-screen siege have no single log line to reuse.** `run_raid` (`game/base/upkeep.rs:684`) logs one of several lines depending on the branch. `resolve_siege_offscreen` (`game/siege/offscreen.rs:38`) logs nothing when the shortfall is 0. Each posts **once, at the top of the function**, with a fixed sentence. The off-screen one posts even when the shortfall is 0: a siege that was fully held off is still worth seeing on the board. Don't add log lines. The rule becomes: reuse the log line where one line exists, and use one fixed sentence where it doesn't. `open_siege` and `resolve_siege_offscreen` never both run for the same siege (`clock.rs:101` joins them with a short-circuit `||`).
3. **The depots-full latch is a field on `AlertBoard`, not a second resource.** Each new `Resource` shifts bevy's query iteration order, and this repo has recorded that as a cause of seeded tests moving. `AlertBoard { alerts: VecDeque<Alert>, depots_full: bool }`. Only `alerts` is saved, because the save writes `Vec<Alert>` and not the resource, so the latch still resets on load as the spec intends. **Budget one pass for seed-luck tests moving** when `AlertBoard` is inserted. See the memory `rng-stream-shift-exposes-seed-luck-tests`. Fix what moves by making each test not depend on luck, never by changing the seed.
4. **The popup already scrolls.** `popup_layout` (`crates/gui/src/render/popup.rs:540`) is the shared window arithmetic behind History and Help. Build the rows as `Row::Item` and call `draw_popup`. Write **no** new scroll function and no tests for one.
5. **`Dry` takes `OFFLINE`, not WARN/ATTENTION.** Call `marks::machine_color` (`render/marks.rs:556`) for stall rows rather than restating the mapping.
6. **Paths.** Hauling is `crates/engine/src/game/base/hauling.rs`, and siege is `crates/engine/src/game/siege/`.

## Global constraints

- `SaveData::alerts` is `#[serde(default)]`. Don't bump `SAVE_FORMAT_VERSION`, and don't recapture `dev-saves/`.
- `AlertKind` variant names are save format. Append new variants; never rename.
- `alerts::post` draws no `GameRng`.
- Every source keeps its existing log line exactly as it is. The board is an addition.
- Gates before every commit: `cargo fmt`, then `cargo clippy --workspace --all-targets`, then the task's own tests. Run `cargo test --workspace` at the end of each phase.
- Don't bump the version or add a CHANGELOG section on the branch.
- Deleted-fix check on every source test: remove the `post` call and watch the test fail.
- Never push. The user asks for pushes.

## File map

| File | Phase | Change |
|---|---|---|
| `crates/engine/src/alerts.rs` (new) | 1 | `AlertKind`, `Alert`, `AlertBoard`, `post`, `dismiss`, `cap` |
| `crates/engine/src/lib.rs` | 1 | `mod alerts` |
| `crates/engine/src/tuning.rs` | 1 | `ALERT_BOARD_CAP = 50` |
| `crates/engine/src/views.rs` | 1 | `AlertView` |
| `crates/engine/src/game/alerts.rs` (new) or an existing `game/` module | 1 | the four `Game` doors plus `post_alert` |
| `crates/engine/src/game/lifecycle.rs` | 1 | insert at `new` (~:470), save (~:2831), load (~:1354) |
| `crates/engine/src/save.rs` | 1 | `SaveData::alerts` |
| `crates/engine/src/systems.rs` | 2 | `set_machine_status` takes the board |
| `crates/engine/src/game/base/teardown.rs` | 2 | `set_rig_status` passes the board |
| `crates/engine/src/game/trade.rs` | 2 | post in `bench_or_dissolve` |
| `crates/engine/src/game/base/upkeep.rs` | 2 | sweep warned, sweep hit |
| `crates/engine/src/game/siege/{clock,mod,offscreen}.rs` | 2 | siege warned, begun ×2 |
| `crates/engine/src/game/base/work_orders.rs` | 2 | two cut-off announcers |
| `crates/engine/src/game/base/hauling.rs` | 2 | depots full plus the latch |
| `crates/engine/src/tests/alerts.rs` (new) | 1, 2 | engine tests |
| `crates/app-core/src/lib.rs` | 3 | `Mode::Alerts` |
| `crates/app-core/src/app/playing.rs` | 3 | `N` in the top match (:68) |
| `crates/app-core/src/app/menus.rs` (or new `app/alerts.rs`) | 3 | `handle_alerts_key` |
| `crates/gui/src/render/alerts.rs` (new) | 3 | popup |
| `crates/gui/src/render/mod.rs` | 3 | draw arm (:607) and `ALL_MODES` (:1513, 115 → 116) |
| `crates/gui/src/render/hud/status_bar.rs` | 3 | unread badge beside `draw_badge` (:154) |
| `assets/help/20-controls.md` | 3 | `N` under "Reading and housekeeping", near `L` and `f` |

---

## Phase 1 — Engine core and save

### Task 1: `alerts.rs`, pure

**Interface:**
- `AlertKind { MachineStalled(MachineStatus), ProgramDowned, SweepIncoming, SweepHit, SiegeIncoming, SiegeBegun, SiteCutOff, DepotsFull }`. Derive `Clone, Debug, PartialEq, Eq, Serialize, Deserialize`, and check that `MachineStatus` derives all of these too. The collapse identity is `(kind, subject)` under full `==` on `kind`, so a different stall status is a different alert with no extra code.
- `Alert { kind, subject: String, text: String, count: u32, unread: bool }`. A field-named struct.
- `#[derive(Resource, Default)] AlertBoard { alerts: VecDeque<Alert>, depots_full: bool }`.
- `pub fn post(board: &mut AlertBoard, kind: AlertKind, subject: impl Into<String>, text: impl Into<String>)`. It follows the spec's three steps: collapse moves the entry to the front, `count += 1`, replaces `text` and sets `unread`; truncation drops from the back.
- `pub fn dismiss(board: &mut AlertBoard, index: usize)`. Out of range does nothing.
- `pub fn cap(board: &mut AlertBoard)`. `post` and load both use it.

- [ ] Tests first (inline `#[cfg(test)]` or `tests/alerts.rs`): collapse bumps `count`, marks the entry unread, moves it to the front and replaces `text`. The same subject with a different `MachineStatus` gives two entries. At `ALERT_BOARD_CAP + 1` the **oldest** is dropped. `dismiss` removes exactly one entry, and out of range does nothing.
- [ ] Implement, then run the gates and commit.

### Task 2: `Game` doors, the view, the save

- `views::AlertView { kind: AlertKind, text: String, count: u32, unread: bool }`. It has no `subject`.
- `Game::alerts(&self) -> Vec<AlertView>`, `unread_alerts(&self) -> usize`, `mark_alerts_read(&mut self)` and `dismiss_alert(&mut self, index: usize)`. Also `pub(crate) fn post_alert(&mut self, kind, subject, text)`, a one-line wrapper over `resource_mut`.
- Insert `AlertBoard::default()` in `Game::new`. Save `alerts: board.alerts.iter().cloned().collect()`. On load, rebuild the board from `data.alerts`, then call `cap`. The latch starts false.
- [ ] Tests: the doors (including that `mark_alerts_read` zeroes `unread_alerts`). A **save → load** test through the real `Game::save`/`Game::load` that keeps order, `count` and `unread`: a RON round trip alone passes against `#[serde(skip)]`. A hand-built `SaveData` with 60 alerts loads as 50.
- [ ] Implement. Run `cargo test --workspace`, and fix any seed-luck test the new resource moved (correction 3). Commit.

---

## Phase 2 — Sources

Build every test on `tests/support.rs` fixtures and assert on `Game::alerts()`. Each latched source gets a second assertion: it posts once, not twice.

### Task 3: machine stalls

- Add `board: &mut AlertBoard` to `set_machine_status` (`systems.rs:702`). Posting inside the transition-only branch is the whole of the "only on entering" rule. Post only for the six stall states, not for `Running` or `Idle`. The subject is `format!("{}@{},{}", site.kind, x, y)` from `StallSite`, and the text is the log line the arm already builds. Build that line once and pass the same string to both the log and `post`.
- Callers: `burn_grid_upkeep` (exclusive, reaches the board through `&mut World`), `idle_machine_system`, `task_progress_system`, `player_gather_system`, `assembler_system` (each takes a new `mut board: ResMut<AlertBoard>`), and `Game::set_rig_status` (inside its `resource_scope`). Watch the bevy parameter limit on `task_progress_system`. If it is at 16, fold the board into an existing `SystemParam` bundle; don't split the system.
- [ ] Tests: a machine entering a stall posts once, and staying in it posts nothing more. Clogged followed by Unpowered gives two entries. Returning to `Running` leaves the alert where it is (spec: resolving doesn't remove).

### Task 4: program downed

- In `bench_or_dissolve`, read the `ProgramId` and the label **before** the difficulty branch, then post `ProgramDowned` with the `ProgramId` as the subject, on both arms (correction 1).
- [ ] Tests: a Forgiving bench and a Permadeath dissolve each post. Run at least one path end to end (the raid defender, or the sortie through `return_sortie`). Downing the same program twice collapses into `×2`.

### Task 5: sweep and siege

- `SweepIncoming` goes on the `warned` latch in `raid_check` (`upkeep.rs:540`) with the existing log text. `SweepHit` goes at the top of `run_raid`, once per sweep, with a fixed sentence.
- `SiegeIncoming` goes on the latch in `warn_of_approaching_siege` (`clock.rs:200`) with the existing text. `SiegeBegun` goes in `open_siege` next to its `"Besiegers pour in…"` line (`mod.rs:185`), and at the top of `resolve_siege_offscreen` with a fixed sentence.
- Zone-wide subjects use the kind's name, so a second sweep collapses into `×2`.
- [ ] Tests: each warning posts once across repeated checks. A sweep posts one `SweepHit`. `open_siege` posts `SiegeBegun`. An off-screen siege with a shortfall of 0 still posts. `dev_force_siege` is a convenient trigger.

### Task 6: sites cut off and depots full

- `announce_dig_cut_off` (`work_orders.rs:1433`) and `announce_cut_off` (`:1743`) post under their `announced_stuck` latch, with the site tile as the subject and the log line as the text.
- In `haul_step_system` (`game/base/hauling.rs:805`), take `mut board: ResMut<AlertBoard>`. In the all-depots-full branch (`:998`), post `DepotsFull` only when `!board.depots_full`, then set the latch. At the successful `deposit` (`:1064`), clear it when the load actually moved (`moved > 0`). The text is a fixed sentence.
- [ ] Tests: dig and build cut-offs post once each. Repeated failed hauls post once. After a successful deposit, the next failure posts again and collapses into `×2`.
- [ ] Phase gate: run `cargo test --workspace`, do the deleted-fix check on each source test, and commit.

---

## Phase 3 — App-core and GUI

### Task 7: app-core

- Add `Mode::Alerts`. In `handle_playing_key`'s top match (`playing.rs:68`, above the hand-off to `handle_stack_key`), `GameKey::Char('N')` sets the mode, sets `menu_selected = 0` and calls `mark_alerts_read`.
- `handle_alerts_key` follows the shape of `handle_history_key` (`menus.rs:249`). Esc calls `close_screen`. `x`/`d` are matched **before** `selected_index`: they dismiss `menu_selected`, then clamp it to `len.saturating_sub(1)`. Everything else goes through `selected_index(key, len)`.
- Add the mode to the dispatch in `handle_key`, however that routes a mode to its handler.
- [ ] Tests: `N` opens the board from the surface and from the Stack (use the `stack` template or its fixture) and marks everything read. `x` and `d` each dismiss the highlighted row, not the row their letter would select. Dismissing the last row clamps the selection. Esc closes. With the board open, a queued notification doesn't show until the board is closed.

### Task 8: GUI

- `render/alerts.rs` builds the rows as `Row::Item` (unread marker, text, `×N` when `count > 1`) and calls `draw_popup`, so `popup_layout` scrolls it (correction 4). An empty board shows "No alerts." The footer is `[↑↓] select  [x/d] dismiss  [Esc] close`. Colour: stall rows through `marks::machine_color`, sweep and siege rows in `THREAT`, the rest in the default row colour.
- Add the draw arm in `render/mod.rs:607`, and add `Mode::Alerts` to `ALL_MODES` (the array length goes to 116). It stays out of `needs_status_banner`, because it is a popup.
- Draw the unread badge next to the attention badge in `status_bar.rs`, only when `unread_alerts() > 0`, reusing `draw_badge`.
- Add a line for `N` to `assets/help/20-controls.md`.
- [ ] Tests: a width census like `no_manifest_pick_row_overflows_its_popup` (`render/manifest.rs:1295`) over the longest text each source can produce, with `×99` added. Build real posts through a `Game` where you can, so the census tests the engine's own sentences. The `ALL_MODES` census passes. If a gui test harness covers the status bar, test that the badge is absent at zero.
- [ ] Phase gate: `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test --workspace`. Commit.

### Task 9: seam and handoff

- Write the seam three times, in the `seams` skill's order: the argument to the memory graph, the trap to the skill's reference file, then one sentence in CLAUDE.md under "Instrumentation" or a new "Alerts" heading. The sentence: *"`alerts::post` is the one door onto the board, a free function because `set_machine_status` has no `Game`, and collapse identity is kind plus subject."* Put corrections 1 and 3 in the trap: a downed program is posted inside `bench_or_dissolve`, and the latch lives on the board.
- Move the spec to `docs/superpowers/archive/specs/` and add a row to `INDEX.md` at the merge, following the repo's existing convention.
- Report plainly that nothing here has been seen on a screen. Ask the user to play the board: open it, dismiss rows, and check the badge.
