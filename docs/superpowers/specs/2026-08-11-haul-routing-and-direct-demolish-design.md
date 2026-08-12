# Hauler routing, stranded reporting, and direct demolish

2026-08-11. Three related changes to base logistics and the demolish flow.
Engine + app-core + gui, no save-format change. Built inline (TDD, commit per
green step) rather than through a plan document — see `CLAUDE.md`'s process
weight rule.

## 1. Haulers route around structures

Today `post_field` walks with `|t| t.walkable`, which is terrain only.
Structures do not make their tile unwalkable — the player is blocked from them
separately, by `find_blocking_structure_at` — so a posted program walks
straight over machines, depots and Home.

`walk_field`'s step rule widens from `Fn(&Tile) -> bool` to
`Fn(&Tile, (i32, i32)) -> bool`. `pursuit_field` ignores the coordinate; the
hauling caller uses it to refuse any tile holding a `Structure`. This is the
seam `pursuit.rs` already documents ("a third caller widens this predicate
rather than copying the walk"), used as intended. There is still one walk.

Three consequences that need handling rather than falling out:

- **`station_tile` takes the same filter.** It nominates the tile a worker
  stands on to work or deliver; unfiltered it can nominate a tile another
  structure occupies, and the worker would be asked to walk onto a building.
- **A worker may always step off its own tile.** `place_structure` checks
  terrain and other structures but not whether a program is standing there, so
  a structure can be built on top of a hauler mid-walk. `walk_field` filters
  successors, so that worker would otherwise be absent from its own field
  forever. The hauling predicate admits the worker's current tile
  unconditionally: you may step *off* a blocked tile, never onto one.
- **Some existing bases will strand workers, and that is correct.** A machine
  with structures on all four orthogonal neighbours cannot be collected from by
  the player either — you cannot stand on a building and `collect_adjacent` is
  orthogonal. The new rule makes haulers obey the geometry the player already
  obeys. Section 2 is what makes the state legible instead of silent.

## 2. Saying so when a post cannot be reached

### Machine status

`MachineStatus` gains `Stranded`, appended. It does not appear in `save.rs`, so
no `SAVE_FORMAT_VERSION` bump.

The writer split is the load-bearing part. `task_progress_system` is the only
system that sets a machine's status; `haul_step_system` is the only one that
computes routes. Giving the status two writers makes them ping-pong
`Unstaffed`↔`Stranded` every tick, and `set_machine_status` logs on every
transition — four lines a tick in the base pane.

So `haul_step_system` records the fact on the **worker**: a marker component,
inserted when `post_field` returns `None` and removed when it does not.
`task_progress_system`'s existing off-station branch reads the marker and
chooses `Stranded` over `Unstaffed`. One writer each.

`haul_step_system` runs last in the chain and `task_progress_system` first, so
the marker is read one tick after it is written. That one-tick lag on a status
label is accepted rather than reordering the chain, whose current order is
load-bearing for the clog/pickup handoff (`game/lifecycle.rs::build_schedule`).

The marker is derived every tick and is not saved. Both structure-destruction
paths (`remove_structure`, `damage_structure`) strip it alongside `Task` and
`Carrying` — the same obligation `Carrying` already carries.

### Assignment refusal

`can_reach_post` returns a bool, and `assign_cronjob` renders every refusal as
"too far away to post a program to". Once buildings can box a machine in, that
message is a lie. It becomes `post_reach`, reporting three states:

- reachable
- **boxed in** — `station_tile` found nothing: no walkable, unoccupied tile
  next to the structure at all
- **no route** — a station exists but the worker is not in its field: too far,
  or something is in the way

Each gets its own wording. The invariant `CLAUDE.md` records stays intact: what
`assign_cronjob` refuses is exactly what `haul_step_system` cannot deliver,
because both go through `post_field`.

## 3. `d` + direction to demolish

`d` on the map screen opens `Mode::RemoveDirection`, drawn with the existing
`draw_direction_prompt` that `x` already uses. A direction key (arrows/hjkl)
resolves the **adjacent tile only**; Esc cancels.

- Nothing there → `"Nothing to demolish that way."`, back to the map.
- Home → `Mode::RemoveConfirm`, the existing y/n screen and cascade warning.
- Anything else → `remove_structure` immediately.

The engine gains `Game::adjacent_structure(dx, dy) -> Option<EntityView>`,
built from the same `view_entities` the demolish menu reads so `is_home` cannot
drift between the two routes.

It returns `None` underground, for the reason `find_target_in_direction` does:
`Position` is pinned to the surface entrance tile while in the Stack, so
`d` + arrow four frames down would aim at the base overhead. App-core also
refuses the `d` keypress underground with a status line rather than a silent
nothing, matching the `surface_only` flag on the group menu's Demolish row.
`remove_structure`'s own `require_surface` remains the actual guard.

The existing menu route is untouched.

## Testing

Engine:

- a hauler's route never enters a structure tile, and it still arrives
- `station_tile` skips an occupied orthogonal neighbour
- a worker standing under a newly-built structure can step off it
- posting to a boxed-in machine is refused, with the boxed wording
- a posted worker with no route drives its machine to `Stranded`
- `adjacent_structure` finds the neighbour, and returns `None` underground

app-core:

- `d` + direction demolishes the adjacent structure
- `d` + direction at Home opens `RemoveConfirm` rather than demolishing
- `d` underground refuses with a status line
- nothing adjacent leaves a status line and returns to `Mode::Playing`

Each test is checked by removing the fix and watching it fail, per
`a-test-that-passes-with-the-fix-removed`. `cargo test --workspace` is the
final gate.

## Documentation

`CHANGELOG.md` section and a workspace version bump at merge, per the
release-per-change rule. `crates/gui`'s help screen gains the `d` key. The
manual and root README are carved out of the doc obligation.
