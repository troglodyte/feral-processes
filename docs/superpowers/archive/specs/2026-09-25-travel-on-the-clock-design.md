# Travel on the clock

**Status:** design approved in conversation 2026-09-25; spec awaiting review.

## Why

Every player step calls `Game::tick()` (`move_player`, `move_in_base`), and
gui's key repeat fires a held arrow every `REPEAT_INTERVAL` (0.09 s). Holding
a direction therefore runs the world at ~11 ticks/s on top of the idle
clock's 2 — faster than `WorldSpeed::Fastest` (8). The speed setting only
governs the world while the player stands still, and walking quietly
fast-forwards everything tuned against the clock: base production, the raid
clock, `WILD_SPAWN_CHANCE`, wild cooldowns.

The fix: **the player states where they want to go, and the clock walks
them there.** One tick, one step. Walking stops buying ticks, and the speed
setting governs the player exactly as it governs everything else.

## What the player sees

- **Arrow keys walk on the clock.** A press moves the player one cell on the
  next clock tick, not on the keypress. Holding an arrow is a steady walk at
  the world's rate — 2 cells/s at Normal, 4 at Fast, 8 at Fastest. Accepted
  as the feel; the first step after a tap can lag up to one tick (0.5 s at
  Normal).
- **Left-click on the map travels.** The player walks, one step per tick,
  to the clicked tile, **routing around hostiles** — unless the clicked tile
  holds the hostile, in which case the player follows it as it moves and the
  fight starts on contact.
- **A travel ends** on arrival, on any key, when the mode leaves `Playing`
  (a fight, a settlement visit, any popup), on game over, on a change of
  space (surface ↔ base), or when no route exists — then with a status line,
  "No clear way there."
- **Pause stays turn-based.** While paused, an arrow steps at once and
  spends one tick, as it does today (`acting_while_paused_still_spends_a_turn`
  holds). A travel set while paused waits for the clock.
- **Unchanged:** the Stack's first-person arrows, tactical battles, sieges,
  and every menu's arrow handling.

## Design

### The walk is app-core's, and the clock spends it

`App` gains `walk: Option<Walk>`:

```rust
enum Walk {
    Step(i32, i32),          // an arrow: a direction, never a route
    Travel(TravelGoal),      // a click
}
enum TravelGoal {
    Tile(i32, i32),          // in the space the click was made in
    Creature(Entity),        // re-resolved to its current tile every step
}
```

plus the space the travel was set in, so a change of space ends it.

`update_realtime` already spends the clock one tick at a time. Each tick
now spends the walk if there is one, and idles otherwise:

- `Step(dx, dy)` → `move_player(dx, dy)`, clear the walk.
- `Travel(goal)` → ask `Game::travel_step(goal)`; `Toward(dx, dy)` →
  `move_player`; `Last(dx, dy)` → `move_player` and clear;
  `NoRoute` / `Gone` → clear, with the status line for `NoRoute`.
- nothing → `idle_tick()`, as now.

`move_player` spends the tick itself, so the step **replaces** the idle
tick rather than adding one. The loop's existing re-check of `mode` after
every tick is what stops a travel that opened a fight or a visit. After a
`move_player` that left the player where they were (bounced off a wall that
appeared, refused), a travel ends rather than retrying forever.

**Rejected: an engine-side travel plan advanced in `tick_inner`.**
`tick_inner` runs under every verb that spends ticks — rest's loop,
hand-compiling, a relay trip — so the player would drift across the map
while resting.

### Input

- **Arrow on the map, clock running:** set `walk = Step(dx, dy)`, spend no
  tick in `handle_key`. A held key's repeats overwrite the pending `Step`,
  so at most one step is ever queued. An arrow replaces a travel.
- **Arrow on the map, paused:** today's immediate step, unchanged.
- **Any other key on the map:** clears a travel before it is handled.
- **Click:** gui resolves a left-click inside the map pane to a tile and
  hands app-core the tile (a `GameKey` payload or an `App::travel_to` call
  — whichever matches how input already flows into app-core; the plan
  picks). Only `Mode::Playing`, surface or base space; ignored underground,
  outside the pane, and in every other mode. If a hostile's `EntityView`
  stands on the tile, the goal is `Creature(entity)`; otherwise `Tile`.
  Choosing the goal from the drawn view keeps it to what the map shows —
  `views::drawn_on_surface_map`'s rule.

### The engine question: `Game::travel_step`

```rust
pub fn travel_step(&mut self, goal: TravelGoal) -> TravelStep
enum TravelStep { Toward(i32, i32), Last(i32, i32), Arrived, NoRoute, Gone }
```

A read of the world that writes nothing and draws no RNG.

- Resolves the goal to a tile in the current space (`Gone` if a creature
  goal no longer exists — `world.get::<Stats>(e).is_none()` — or is in the
  other space).
- **The last step is always an ordinary bump, and the walk ends with it.**
  When the goal is adjacent, answer `Last(dx, dy)`: `move_player` then does
  whatever it does there today — fight a hostile, queue a settlement visit,
  swing at rock when mining is armed, be refused by a wall — with nothing
  new. When the player is already on the goal, `Arrived`.
- Otherwise builds a route with **`walk_field`**, the one surface Dijkstra
  (its cost-function seam), from the goal back toward the player, and
  answers the neighbour of the player with the lowest cost. The step rule
  is **the same walkability `move_player` / `move_in_base` consult — called,
  not restated** — with every cell holding a hostile (wild program, nest,
  patrol) refused. The goal cell is exempt from both refusals.
- Search box: the Chebyshev distance to the goal plus a margin,
  `tuning::TRAVEL_ROUTE_MARGIN`, bounding the lazily generated zone map
  exactly as `walk_field`'s comment requires.

Base space uses the same function over `BaseGrid` walkability, with bodies
blocking per "one body to a cell"; the underground returns `NoRoute` and is
never asked.

### Saves

Nothing is saved. A walk is intent, not state; a reload drops it. No save
field, no `SAVE_FORMAT_VERSION` bump.

## Consequences worth naming

- **An arrow into a hostile lands next tick.** The hostile may step away or
  step into you first.
- **Walking no longer fast-forwards the world.** Anything that felt paced
  while walking — encounter arrivals, base output per trip — now runs at
  the clock's pace. `balance_sim` models none of it. This wants a
  playtest at Normal before anything is retuned.
- CLAUDE.md seams touched: "A path that spends ticks owes `after_tick()`"
  (update_realtime already ends in it) and `walk_field`'s cost-function
  rule (a new caller, `Some(1)` like the others).

## Testing

**Engine** (`travel_step`):
- routes around a hostile standing in the straight line;
- a hostile goal is routed *to*, and `Last` is answered beside it;
- a player boxed in by hostiles gets `NoRoute`;
- `Arrived` on the goal; `Gone` for a despawned creature;
- a base-space route avoids solid cells and bodies;
- no `GameRng` draw (the Predation test's shape).

**App-core:**
- an arrow moves the player on the next `update_realtime` tick and not in
  `handle_key`;
- holding an arrow for N seconds walks `ticks_per_second * N` cells and
  spends exactly the clock's ticks — **the regression this exists for**;
- paused arrow still steps immediately and spends one tick;
- a travel arrives; follows a moving hostile into a fight; ends on a key;
  ends on `NoRoute` with the status line; ends on a change of space;
- existing tests that walk the map with arrows use a helper that presses
  and then spends one tick.

**Gui:**
- a click maps back to the tile `tile_origin_px` drew there, at two window
  sizes and with the status bar's row claimed (the literal-`0.0` trap in
  the drawing seam).

## Out of scope

Stack movement, tactical and siege boards, a keyboard cursor for choosing a
destination, stopping when a new hostile comes into view, hover previews of
the route.
