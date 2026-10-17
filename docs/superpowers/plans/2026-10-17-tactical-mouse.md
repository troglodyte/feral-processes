# Tactical Mouse Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** On a battle map, a primary click walks the acting body to a reachable cell, marks then swings at a hostile, and commits the aim cursor; hovering previews the walk's path.

**Architecture:** The engine gains four doors over code it already has (plan a walk to a cell, preview that path, stop a walk, spend one cell of the party's walk). App-core gains one pointer entry per gesture, dispatched by `Mode` the way `handle_key` is, holding the target mark and the hovered cell. The gui's existing `handle_map_pointer` stops calling `travel_to` directly and reports the cell and button to app-core; the board publishes its `MapClickLayout` like the surface map does.

**Tech Stack:** Rust workspace; `bevy_ecs` in the engine; egui pointer input in `crates/gui`; `Painter` for all drawing.

**Spec:** `docs/superpowers/specs/2026-10-17-tactical-mouse-design.md`

## Global Constraints

- `Game.world` stays private; no accessor.
- `crates/gui` names no graphics library outside `paint.rs`; the two new marks draw through `Painter`.
- The renderer decides nothing: gui resolves a pixel to a cell and hands it to `App`.
- Every pointer gesture ends in a call the keyboard already makes (`Game::tactical_step`, `Game::tactical_attack`, `App::commit_tactical_aim`).
- No save-format change; no new serialised state.
- Refusals go through `App::refuse`, with exactly these sentences: `"Out of reach."` and `"Not from here."`.
- `cargo fmt`, `cargo clippy --workspace --all-targets` clean after each task; `cargo test --workspace` is the gate before the branch is finished.
- Never `git checkout` or `git stash` another agent's work; stage explicit paths; never push.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.

## Review Focus

1. A click during a live walk must stop it and do nothing else (Task 2 tests the key; Task 3 tests the click).
2. The pointer parked over the board must not fight the arrow keys in aim mode: the cursor moves only on a cell change (Task 3).
3. A marked hostile that dies or leaves under a keyboard action, or a mark left from an earlier body's turn, must never reach `tactical_attack` (Task 3, "mark dropped when its body is gone" and "a mark does not outlive the turn").
4. A click on the surface map must still travel exactly as before once the handler is split (Task 5, existing gui tests stay green unchanged).
5. The hover label must not appear over the board now that a layout is published there (Task 6 test).

---

### Task 1: Engine doors for a planned walk

**Files:**
- Modify: `crates/engine/src/tactical/turn.rs` (beside `tactical_step`)
- Modify: `crates/engine/src/tactical/ai.rs` (beside `tactical_auto_beat`; `step_along_walk` is at ~981)
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Consumes: `reach::movement_field`, `reach::path_to`, `TacticalBattle::commit_walk`, `TacticalBattle::walking`, `Game::tactical_awaits_input`, `Game::movement_allowance`, private `Game::step_along_walk(actor) -> Option<AiBeat>`.
- Produces:
  - `pub fn tactical_path_to(&self, to: (i32, i32)) -> Vec<(i32, i32)>` — the cells the acting body would walk, excluding its own; empty when there is no fight, no actor, or the field never reached `to`.
  - `pub fn tactical_walk_to(&mut self, to: (i32, i32)) -> bool` — commits `tactical_path_to(to)` through `commit_walk`; `false` and nothing committed when the path is empty or the actor is not the player's (`!tactical_awaits_input()`).
  - `pub fn tactical_stop_walk(&mut self)` — `commit_walk(Vec::new())`, "has arrived", the same thing `step_along_walk` does on a refusal; a no-op with no fight.
  - `pub fn tactical_walk_beat(&mut self) -> AiBeat` — `Idle` unless `tactical_awaits_input()` and `walking()`; otherwise `step_along_walk(actor).unwrap_or(AiBeat::Idle)`.

- [ ] **Step 1: Write the failing tests** in `crates/engine/src/tests/tactical.rs`, using the file's existing tactical fixture (the one its `tactical_step` tests open a fight with):

```rust
#[test]
fn tactical_path_to_a_reachable_cell_ends_there_and_starts_beside_the_actor() {
    // pick `to` from `game.tactical_view().reachable` at Chebyshev distance >= 2 from the actor
    // assert path.last() == Some(&to)
    // assert every consecutive pair is one king-move apart, the first one from the actor's cell
    // assert path.len() as u32 <= view.allowance
}

#[test]
fn tactical_path_to_an_unreachable_cell_is_empty() {
    // a cell off the board: (-1, -1)
}

#[test]
fn tactical_walk_to_commits_and_walk_beats_spend_it_one_cell_at_a_time() {
    // assert tactical_walk_to(to); assert tactical_walking()
    // loop: tactical_walk_beat() == Stepped while walking; count steps == path.len()
    // assert cell_of(actor) == to; assert !acted (view.acted false)
    // assert tactical_walk_beat() == Idle afterwards
}

#[test]
fn tactical_walk_to_refuses_for_an_unreachable_cell_and_on_the_wild_sides_turn() {
    // (-1,-1) -> false and !tactical_walking()
    // hand the turn to a hostile (tactical_end_turn or the fixture's own way), then a reachable
    // cell for that hostile -> false
}

#[test]
fn tactical_stop_walk_leaves_the_body_where_it_stands() {
    // walk_to, one walk_beat, stop_walk: !tactical_walking(), cell_of(actor) == path[0],
    // a following tactical_walk_beat() == Idle
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-engine tactical_path_to tactical_walk tactical_stop_walk`
Expected: compile errors naming the four missing methods.

- [ ] **Step 3: Implement the four doors** with the signatures above. `tactical_path_to` builds `movement_field(battle, actor, self.movement_allowance(actor))` and calls `path_to`; it reads, so `&self`. Doc comments say why each exists (the player's walk is the same run of `tactical_step`s a hostile's is) and point at the seam list in `.claude/rules/seams-tactical.md`.

- [ ] **Step 4: Run the tests to verify they pass**

Run: `cargo test -p feral-processes-engine tactical_path_to tactical_walk tactical_stop_walk`
Expected: PASS.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy -p feral-processes-engine --all-targets
git add crates/engine/src/tactical/turn.rs crates/engine/src/tactical/ai.rs crates/engine/src/tests/tactical.rs
git commit -m "Engine: a party body can plan, spend and stop a walk to a cell"
```

---

### Task 2: App-core paces the player's walk and any key stops it

**Files:**
- Modify: `crates/app-core/src/app/tactical.rs` (`advance_tactical` at ~393, `handle_tactical_key` at ~56)
- Modify: `crates/app-core/src/app/input.rs` (`handle_key`'s tactical arms at ~333 only if the stop must catch keys in `TacticalAim`; see Step 3)
- Test: `crates/app-core/src/tests/tactical.rs`

**Interfaces:**
- Consumes: Task 1's `Game::tactical_walk_to`, `tactical_walk_beat`, `tactical_stop_walk`, `tactical_walking`.
- Produces:
  - `pub fn tactical_walk_to(&mut self, cell: (i32, i32))` on `App` — on the player's turn in `Mode::TacticalBattle`: `tactical_walk_to(cell)`, refusing with `"Out of reach."` when the engine answers `false`; a click on the acting body's own cell is silently ignored (the engine's empty path would otherwise refuse).
  - `advance_tactical` spends beats while `Game::tactical_walking()` on the player's turn: the early return becomes `player_turn && !auto && !walking`, and the loop's door is `tactical_auto_beat` when auto, else `tactical_walk_beat` when the player's turn, else `tactical_ai_beat`.
  - `handle_tactical_key`: ahead of the auto-attack stop, if `tactical_player_turn() && game.tactical_walking()` then `tactical_stop_walk()` and return (swallowed).

- [ ] **Step 1: Write the failing tests**, using `fighting`, `wait_for_the_player`, `acting_cell` from the same file and `TACTICAL_STEPS_PER_SECOND`:

```rust
fn reachable_cell_two_away(app: &mut App) -> (i32, i32) { /* from tactical_view().reachable, Chebyshev >= 2 from acting_cell */ }

#[test]
fn walking_to_a_cell_moves_one_step_a_beat_and_spends_no_action() {
    // app.tactical_walk_to(to); let path_len = game.tactical_path_to(to).len() before the call
    // advance_tactical(1.0 / TACTICAL_STEPS_PER_SECOND) once: acting_cell moved one king-move
    // advance until arrived: acting_cell == to; view.acted == false; still the player's turn
}

#[test]
fn walking_to_an_unreachable_cell_refuses_aloud() {
    // (-1, -1): status_line == Some("Out of reach."); acting_cell unchanged
}

#[test]
fn a_key_during_the_walk_stops_it_and_is_swallowed() {
    // walk_to; one beat; handle_key(GameKey::Char('a'));
    // mode still TacticalBattle (the 'a' did not open the aim cursor); !tactical_walking()
    // a further beat moves nothing
}

#[test]
fn the_walk_pauses_the_carry_only_while_it_is_the_players_turn() {
    // on the wild side's turn, App::tactical_walk_to does nothing and status_line stays None
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-app-core walking_to a_key_during_the_walk the_walk_pauses`
Expected: compile error, `tactical_walk_to` not found on `App`.

- [ ] **Step 3: Implement.** Keep the stop in `handle_tactical_key` only; a walk is only ever committed from `Mode::TacticalBattle`, and every key in that mode passes through it. Update `advance_tactical`'s doc: the wait is now derived from "owes a beat", which the player's walk also does.

- [ ] **Step 4: Run the tactical suite**

Run: `cargo test -p feral-processes-app-core tactical`
Expected: PASS, including the existing pacing tests (`tactical_carry` held at zero on an idle player turn must still hold).

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy -p feral-processes-app-core --all-targets
git add crates/app-core/src/app/tactical.rs crates/app-core/src/tests/tactical.rs
git commit -m "Battle map: the player's walk is paced a cell a beat, and any key stops it"
```

---

### Task 3: App-core pointer entry — click dispatch by mode and the hovered cell

**Files:**
- Create: `crates/app-core/src/app/pointer.rs` (one `impl App` block; register `mod pointer;` where `mod hover;` is)
- Modify: `crates/app-core/src/lib.rs` (two fields on `App`, beside `tactical_cursor` at ~3139)
- Modify: `crates/app-core/src/app/travel.rs` (`travel_to` keeps its signature; it becomes the `Mode::Playing` arm)
- Test: `crates/app-core/src/tests/tactical.rs` and `crates/app-core/src/tests/hover.rs`

**Interfaces:**
- Consumes: `PointerButton` (already `pub` in `sprite_forge.rs` and re-exported), `App::travel_to`, Task 2's `App::tactical_walk_to`, `Game::tactical_occupant`, `Game::tactical_attack`, `App::commit_tactical_aim` (make it `pub(crate)` if it is not), `Game::tactical_path_to`.
- Produces, all on `App`:
  - `pub map_hover: Option<(i32, i32)>` — the board or map cell under the pointer this frame, `None` off the pane; set by gui every frame.
  - `pub struct TargetMark { pub actor: Entity, pub target: Entity }` and `pub tactical_target: Option<TargetMark>` — the marked hostile and which body's turn marked it.
  - `pub fn set_map_hover(&mut self, cell: Option<(i32, i32)>)` — stores it; in `Mode::TacticalAim`, when `cell` is `Some` and differs from the previous `map_hover`, sets `tactical_cursor` to it clamped to `0..side`.
  - `pub fn map_click(&mut self, cell: (i32, i32), button: PointerButton)` — `match self.mode`: `Playing` → primary only, `travel_to`; `TacticalBattle` → `tactical_board_click`; `TacticalAim` → primary: `tactical_cursor = Some(cell)` then `commit_tactical_aim(cell)`; secondary: the `Esc` arm of `handle_tactical_aim_key`; every other mode → nothing.
  - `fn tactical_board_click(&mut self, cell, button)` — private: stop a live walk and return if walking; on the wild side's turn or under `tactical_auto` do what a key does (auto off, swallowed) and return; secondary → clear the mark; primary: if `tactical_occupant(cell)` is a hostile body (`view.bodies` entry with `is_hostile`): equal to the mark → `tactical_attack`, refuse `"Not from here."` on `false`, clear the mark on `true`, then `after_tactical_action`; otherwise set the mark. If no hostile there: clear the mark, then `tactical_walk_to(cell)`.
  - `pub fn tactical_target_cell(&mut self) -> Option<(i32, i32)>` — the mark's cell read off the live view; drops the mark and answers `None` when `target` is not on the board or `actor` is no longer the acting body. The renderer draws off this and the click path validates through it, so no turn-change event is needed.
  - `pub fn tactical_hover_path(&mut self) -> Vec<(i32, i32)>` — `Mode::TacticalBattle`, player's turn, not walking, `map_hover` in `view.reachable` and not the actor's cell → `Game::tactical_path_to(cell)`; else empty.
  - `tactical_target` is also cleared in `settle_tactical_end`, where the other per-fight state is.

- [ ] **Step 1: Write the failing tests** in `tests/tactical.rs` (helpers: `fighting`, `wait_for_the_player`, `acting_cell`, `acting_entity`; add `hostile_body(app) -> (Entity, (i32,i32))` reading the first `is_hostile` body):

```rust
#[test]
fn a_click_on_a_hostile_marks_it_and_a_second_click_swings_or_refuses() {
    // map_click(hostile_cell, Primary): tactical_target.map(|m| m.target) == Some(hostile); view.acted == false
    // map_click again: if Chebyshev(acting, hostile) <= 1 then acted == true and target == None
    //                  else status_line == Some("Not from here.") and target still Some(hostile)
}

#[test]
fn a_click_elsewhere_clears_the_mark_and_walks() {
    // mark; map_click(reachable_cell_two_away, Primary): target == None; tactical_walking()
}

#[test]
fn a_secondary_click_and_esc_each_clear_the_mark() { /* two apps, one each */ }

#[test]
fn the_mark_is_dropped_when_its_body_is_gone() {
    // mark; game.finish_hostile / remove the hostile through the fight's own door (Esc-free):
    // simplest: aim-and-swing with the keys until it is gone, or use the engine test hook the
    // engine tests use to kill a body; then tactical_target_cell() == None and tactical_target == None
}

#[test]
fn a_mark_does_not_outlive_the_turn() {
    // mark; handle_key(GameKey::Char('E')) ends the turn; wait_for_the_player;
    // tactical_target_cell() == None and tactical_target == None
}

#[test]
fn a_click_on_the_wild_sides_turn_does_nothing() {
    // before wait_for_the_player, if it is not the player's turn: map_click(any, Primary) leaves
    // cell, mark and status_line unchanged
}

#[test]
fn hovering_a_reachable_cell_answers_the_path_and_nothing_else_does() {
    // set_map_hover(Some(two_away)): tactical_hover_path() non-empty, ends at the cell
    // set_map_hover(Some(acting_cell)): empty; set_map_hover(None): empty
}

#[test]
fn in_aim_mode_the_cursor_follows_a_cell_change_and_a_click_commits() {
    // handle_key('a'); set_map_hover(Some(c1)) where c1 != map_hover before: tactical_cursor == Some(c1)
    // handle_key(Right): cursor == c1 + (1,0); set_map_hover(Some(c1)) again: cursor unchanged (no change event)
    // set_map_hover(Some(c2)): cursor == c2
    // map_click(hostile_cell, Primary): mode == TacticalBattle, and acted == true when adjacent
    // else status_line == Some("Not from here.")
}

#[test]
fn in_aim_mode_a_secondary_click_is_esc() { /* mode back to TacticalBattle, nothing spent */ }
```

And in `tests/hover.rs` (or `tests/playing.rs`, wherever `travel_to` is tested):

```rust
#[test]
fn a_primary_map_click_in_playing_travels_and_a_secondary_does_not() {
    // map_click(tile, Primary) sets the same travel travel_to(tile) does; Secondary sets none
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-app-core map_click set_map_hover tactical_hover_path the_mark_is_dropped`
Expected: compile errors for the missing fields and methods.

- [ ] **Step 3: Implement `pointer.rs`** with the interfaces above. Module doc: the mouse's one entry into app-core for anything drawn in the map pane; gui has no `match app.mode`.

- [ ] **Step 4: Run the app-core suite**

Run: `cargo test -p feral-processes-app-core`
Expected: PASS.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy -p feral-processes-app-core --all-targets
git add crates/app-core/src/app/pointer.rs crates/app-core/src/app/mod.rs crates/app-core/src/lib.rs crates/app-core/src/tests/tactical.rs crates/app-core/src/tests/hover.rs
git commit -m "Battle map: a click walks, marks and swings, and the aim cursor follows the pointer"
```

---

### Task 4: The board publishes its layout and draws the path and the mark

**Files:**
- Modify: `crates/gui/src/render/tactical.rs` (`draw_tactical_map` at ~607, `Wash` at ~307, `board_washes`, the cursor outline at ~980)
- Modify: `crates/gui/src/render/base.rs` (the tactical branch at ~433; `tactical_preview` wiring at ~329)
- Test: `crates/gui/src/render/tactical.rs` tests module

**Interfaces:**
- Consumes: `MapClickLayout` and `tile_at_px` from `render/base.rs`, `Fx::set_map_click`, Task 3's `App::tactical_hover_path` and `App::tactical_target_cell`.
- Produces:
  - `pub(super) struct Overlay { cursor: Option<(i32,i32)>, preview: Vec<(i32,i32)>, placeable: Vec<(i32,i32)>, path: Vec<(i32,i32)>, target: Option<(i32,i32)> }` with `Default`, replacing `draw_tactical_map`'s three overlay parameters; the nine existing test call sites become `&Overlay::default()`.
  - `Wash::Path`, label `"path"`, colour `palette::PLAN` at the alpha `Wash::Aim` uses (so it reads as "move, chosen"); drawn after `Move`/`Cover`/`Danger` and before `Aim` in `board_washes`, cells taken from `overlay.path`.
  - The target outline: `rect_lines` at the acting outline's weight in `palette::EMPHASIS` over `target`'s footprint, bounds-checked with `on_pane`, drawn beside the cursor outline.
  - `draw_tactical_map` calls `fx.set_map_click(Some(MapClickLayout { pane, center, half: (half_w, half_h), off: (off_x, off_y), tile_px }))` once it has computed them.

- [ ] **Step 1: Write the failing tests** in `render/tactical.rs`'s tests, using its `fighting()`, `with_painter`, `pane()`:

```rust
#[test]
fn the_board_publishes_a_layout_that_inverts_a_bodys_pixel_to_its_cell() {
    // draw with Overlay::default(); let layout = fx.map_click().expect("published");
    // for each body: tile_origin_px(body.cell, layout.center, layout.half, layout.off, tile_px, pane)
    //   then tile_at_px(px + 1.0, py + 1.0, &layout) == body.cell
}

#[test]
fn a_hover_path_is_washed_and_named_in_the_legend() {
    // Overlay { path: vec![two cells], ..Default::default() }: returned washes contain Wash::Path
}

#[test]
fn a_marked_target_is_outlined() {
    // Overlay { target: Some(hostile.cell), .. }: one more rect_lines shape than the default draw,
    // at that cell's pixel origin
}
```

- [ ] **Step 2: Run them to verify they fail**

Run: `cargo test -p feral-processes-gui the_board_publishes a_hover_path a_marked_target`
Expected: compile errors (`Overlay`, `Wash::Path`).

- [ ] **Step 3: Implement.** In `base.rs`, build the `Overlay` from `app.tactical_cursor`, the two existing vectors, `app.tactical_hover_path()` and `app.tactical_target_cell()` before the `game` borrow, as `tactical_preview` already is. Update the wash doc comment on `board_washes` (it enumerates the washes) and the legend's row order.

- [ ] **Step 4: Run the gui suite**

Run: `cargo test -p feral-processes-gui`
Expected: PASS.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy -p feral-processes-gui --all-targets
git add crates/gui/src/render/tactical.rs crates/gui/src/render/base.rs
git commit -m "Battle map: the board publishes its click layout and draws the hover path and target mark"
```

---

### Task 5: The gui pointer handler reports cells instead of travelling

**Files:**
- Modify: `crates/gui/src/lib.rs` (`handle_map_pointer` at ~419; its tests at ~1331 onward)
- Test: `crates/gui/src/lib.rs` tests module

**Interfaces:**
- Consumes: Task 3's `App::map_click`, `App::set_map_hover`; `PointerButton`.
- Produces: `handle_map_pointer` unchanged in signature. Each frame it calls `app.set_map_hover(pane_tile(hover_pos))` (`None` when egui wants the pointer, when no layout is published, or off the pane). A primary release within `CLICK_DRAG_TOLERANCE_PX` of its press calls `app.map_click(tile, PointerButton::Primary)`; a secondary release under the same rule calls `app.map_click(tile, PointerButton::Secondary)`. The hover label path is untouched.

- [ ] **Step 1: Write the failing tests** beside `handle_map_pointer_answers_a_hover_after_a_rest_and_not_while_held`, using `app_on_the_map`, `test_layout`, `map_pointer_frame` (extend it with a `button: egui::PointerButton` parameter, defaulting the existing calls to `Primary`):

```rust
#[test]
fn a_frame_over_the_pane_sets_the_apps_hover_cell_and_off_it_clears_it() {
    // hover_frame at a pane pixel: app.map_hover == Some(tile_at_px(...)); at (-5,-5): None
}

#[test]
fn a_secondary_click_reaches_map_click_with_the_secondary_button() {
    // Playing mode: a secondary press+release sets no travel (`travel_goal()` is `None`)
    // — the only observable in Playing; the tactical dispatch is Task 3's to test
}
```

The existing click-to-travel tests stay as they are and must still pass.

- [ ] **Step 2: Run to verify they fail**

Run: `cargo test -p feral-processes-gui handle_map_pointer`
Expected: the two new tests fail (`map_hover` never set; secondary unhandled).

- [ ] **Step 3: Implement.** Track `down_pos` per button (`down: Option<(egui::PointerButton, egui::Pos2)>`); a second button pressed while one is held cancels the gesture. Update the function doc: it names a cell and a button and nothing more.

- [ ] **Step 4: Run the gui suite**

Run: `cargo test -p feral-processes-gui`
Expected: PASS.

- [ ] **Step 5: fmt, clippy, commit**

```bash
cargo fmt && cargo clippy -p feral-processes-gui --all-targets
git add crates/gui/src/lib.rs
git commit -m "Map pointer: report the cell and button to app-core instead of travelling"
```

---

### Task 6: No hover label over the board

**Files:**
- Test: `crates/app-core/src/tests/hover.rs`

**Interfaces:**
- Consumes: `App::hover_lines`; a tactical fixture (`fighting` lives in `tests/tactical.rs`; move it into `tests/support.rs` if `hover.rs` cannot reach it).

- [ ] **Step 1: Write the test**

```rust
#[test]
fn hover_lines_are_empty_on_a_battle_map() {
    // fighting app in Mode::TacticalBattle: hover_lines(x, y) for the acting body's cell is empty
}
```

- [ ] **Step 2: Run it**

Run: `cargo test -p feral-processes-app-core hover_lines_are_empty_on_a_battle_map`
Expected: PASS already (`surface_map_game` refuses outside `Mode::Playing`). If it fails, `surface_map_game` has changed and the gate is the fix, not a second one in gui.

- [ ] **Step 3: Commit**

```bash
git add crates/app-core/src/tests/hover.rs crates/app-core/src/tests/support.rs
git commit -m "Pin: the hover label stays off a battle map"
```

---

### Task 7: Help page and changelog

**Files:**
- Modify: `assets/help/28-battle-maps.md` ("On your turn" list and the cursor paragraph near the end)
- Modify: `CHANGELOG.md` (an unreleased entry under the next section the preamble's rule names; the release bump itself happens at landing)

- [ ] **Step 1: Add the mouse to the help page.** Under "On your turn", after the arrows row: a row for the mouse — click a blue cell to walk there (the path lights up as you hover; any key or click stops the walk), click a hostile to mark it and click it again to attack, right-click to clear the mark. In the cursor paragraph: the cursor follows the mouse over the board, a click is Enter and a right-click is Esc. Plain language, no "tick" or "frame" vocabulary, programs not monsters.

- [ ] **Step 2: Changelog line** describing the three gestures in one sentence.

- [ ] **Step 3: Run the help tests** (the help pages are parsed by tests)

Run: `cargo test -p feral-processes-engine help`
Expected: PASS.

- [ ] **Step 4: Commit**

```bash
git add assets/help/28-battle-maps.md CHANGELOG.md
git commit -m "Help: the mouse on a battle map"
```

---

### Task 8: Whole-workspace gate

- [ ] **Step 1:** `unset FERAL_DEV_NO_SIEGES; cargo fmt --check && cargo clippy --workspace --all-targets && cargo test --workspace`
Expected: all green. `balance_sim` is unaffected (no tuning, species or item change) but runs with the rest.

- [ ] **Step 2:** `cargo run --bin arena -- dev-arenas/opening-fight.ron` still stages (the arena drives `tactical_ai_beat`; the new `tactical_walk_beat` must not have changed `AiBeat` semantics).

- [ ] **Step 3:** Hand over for the whole-branch review (opus) per `CLAUDE.md`.
