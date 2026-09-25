# Travel on the clock — plan

Spec: `docs/superpowers/specs/2026-09-25-travel-on-the-clock-design.md` (read it; this
file does not restate it). Branch `travel-on-the-clock`. No save change.

Three phases, one per crate, each ending green (`cargo test -p <crate>`, `cargo clippy
--workspace --all-targets`, `cargo fmt`) with a commit per green step. Each phase fits
one sonnet dispatch; phase 3 depends on 2's API, 2 on 1's. Per-task review gates off;
one opus whole-branch review at the end (see CLAUDE.md "Running subagents").

## Phase 1 — engine: `Game::travel_step`

Files: new `crates/engine/src/game/travel.rs` (+ `mod` in `game/mod.rs`),
`tuning.rs`, tests in `crates/engine/src/tests/` (fixtures: `tests/support.rs`).

Interface (pub, re-exported where `Game`'s other view types are):
- `TravelGoal { Tile(i32, i32), Creature(Entity) }`
- `TravelStep { Toward(i32, i32), Last(i32, i32), Arrived, NoRoute, Gone }`
- `Game::travel_step(&mut self, goal: TravelGoal) -> TravelStep` — writes nothing,
  draws no `GameRng`. `&mut` only because `WorldMap::tile` generates lazily.
- `tuning::TRAVEL_ROUTE_MARGIN` (start at 8; documented).

Rules:
- Current tile: `Position` on the surface, `base_pos()` in base space; underground →
  `NoRoute`. Creature goal: `world.get::<Stats>(e).is_none()` or in the other space
  (base-space body vs zone body — `stands_in_base_space`) → `Gone`.
- On goal → `Arrived`; goal Chebyshev-adjacent → `Last(dx, dy)`.
- Else `pursuit::walk_field(goal, dist + TRAVEL_ROUTE_MARGIN, cost)` rooted at the
  **goal**, then pick the player's neighbour with the lowest field value → `Toward`;
  none reachable → `NoRoute`. Break ties by `NEIGHBOURS` order (deterministic).
- `cost(cell)`: goal cell always `Some(1)`. Otherwise `Some(1)` iff the cell is one
  `move_player`/`move_in_base` would *step onto*: terrain walkable (`Tile::walkable` /
  `BaseGrid::walkable`) **and** no arm of the bump ladder fires there. The ladder's
  non-step arms (`turn.rs` ~700–800: wild creature, nest, settlement, outpost, trap;
  base: solid rock, bodies per `hauling::blocked_tiles`) must be **called, not
  restated** — extract one `pub(crate) fn Game::bump_at(x, y) -> bool` (surface) from
  the existing `find_*_at` calls and use it in the cost fn; don't restructure
  `move_player`'s arms. Base space: reuse `blocked_tiles`/`walks_the_base`, not a new
  body scan. A settlement/outpost must never be routed *through* (would open a visit).

Tests (spec §Testing/Engine, plus): route never passes a settlement tile; `Last` on a
hostile goal; `NoRoute` when boxed in; `Gone` after despawn; base route avoids rock and
bodies; RNG untouched (copy the Predation no-draw test's shape).

Seam writes: none new (a `walk_field` caller answering `Some(1)` is the existing rule).

## Phase 2 — app-core: the walk and the clock

Files: `crates/app-core/src/app/input.rs` (`update_realtime`),
`app/playing.rs` (`handle_playing_key`, arrow arms ~line 386), `lib.rs` (`App` field),
new `app/travel.rs` for `Walk` + `App::travel_to`, tests in `src/tests/`.

- `App::walk: Option<Walk>` (`Step(dx,dy)` | `Travel { goal, in_base: bool }`), not
  saved, cleared in whatever resets `App` state on load/new game.
- `pub fn App::travel_to(&mut self, x: i32, y: i32)`: only `Mode::Playing`, not
  underground; goal = `Creature(e)` if a hostile `EntityView` stands on `(x, y)`
  (use the same view list the map draws — `drawn_on_surface_map`'s rule), else
  `Tile`. Chosen over a `GameKey` payload: `handle_pointer` is the precedent for
  pointer input and `GameKey` is `Copy` key vocabulary.
- `handle_playing_key`: before the surface/base arrow arms — if **not paused**, an
  arrow/hjkl sets `walk = Some(Step)` and returns (no tick, no `after_world_action`).
  Paused keeps today's `stepped(...)` path untouched. Any non-move key clears a
  `Travel` first. Stack path (`handle_stack_key`) untouched.
- `update_realtime` loop body: take the walk; `Step` → `move_player`, clear;
  `Travel` → end if `in_base != game.in_base()`, else match `travel_step`:
  `Toward` → `move_player`, `Last` → `move_player` + clear, `Arrived`/`Gone` → clear,
  `NoRoute` → clear + `self.refuse("No clear way there.")` (check `refuse`'s signature).
  A `Toward` that left the player's tile unchanged → clear. No walk → `idle_tick()`.
  Keep the mode/battle/game-over re-check and the single `after_tick()`. Mode leaving
  `Playing` clears the walk (the early-return branch).
- Ground-bite cue: `move_player` returns the bite; route it to the same cue
  `after_world_action` picks today (read that fn; don't duplicate the pick).

Tests: spec §Testing/App-core. The regression test: hold (re-send) an arrow across
`update_realtime` calls totalling N s at Normal → exactly `N * ticks_per_second` ticks
and cells. Then fix existing map-walking tests (~254 arrow presses repo-wide, most are
menus and unaffected): add `tests::support` helper `walk(app, key)` = `handle_key` +
`update_realtime(1.0 / ticks_per_second)`; convert only the tests that fail. Also
confirm `acting_while_paused_still_spends_a_turn` passes unmodified.

## Phase 3 — gui: click to travel

Files: `crates/gui/src/render/base.rs` (beside `tile_origin_px` ~1619), `lib.rs`
(pointer read, beside `handle_sprite_pointer` ~193).

- Pure `tile_at_px(px, player, half, off, tile_px, pane) -> (i32, i32)`, the exact
  inverse of `tile_origin_px` (`floor`), in the same file so the two can't drift. Base
  space: confirm whether `draw_playing_base` uses the same origin/camera math there;
  if a second origin exists, invert that one too via the same pattern.
- The map pane rect, camera (`watching`, zoom) and `half`/`off` must come from the
  values the draw pass used this frame — stash them in a small `Resource` written by
  the draw, read by the pointer system; don't recompute layout.
- On primary *click* (press+release, not drag) inside the map pane, `Mode::Playing`,
  not underground: `app.travel_to(x, y)`. Ignore when egui wants the pointer.

Tests: `tile_at_px(tile_origin_px(t)) == t` at two window sizes with the status-bar row
claimed (non-zero `pane.y`) and a non-zero camera offset.

## Close-out (controller, not a dispatch)

- `cargo test --workspace`; opus whole-branch review with the spec as the rule.
- CHANGELOG entry at merge (deploy skill), no save bump. Schema READMEs unaffected.
- Seams: CLAUDE.md gains one line under *The ground*
  ("Walking is spent by the clock: `update_realtime` owns the step; `handle_key` never
  ticks on an unpaused arrow") — argument to graph, trap to the `seams` skill.
- Hand play to the user: feel at Normal, click-routing, following a hostile. Agents
  cannot playtest.
