# Dev keypad cheats

**Date:** 2026-10-20

## Why

Testing advanced structures and zones means playing up to them and
gathering their materials first. Saves stop loading at every breaking
save-format bump (`save::SAVE_FORMAT_VERSION`, no migration), so that play
has to be repeated. These cheats let the developer reach a state in a few
key presses from any save or template, then `savetool capture` it if it is
worth keeping.

## Goals

Five cheats behind the existing dev keypad gate (`FERAL_DEV_CONSOLE`, or
the `FERAL_DEV` master):

1. **Free build**: a toggle row. While on, placing a structure costs no
   materials and the structure is complete as soon as it is placed.
2. **Give item**: a row opening an item picker. One press of a row adds 10
   of that item to the player's inventory.
3. **Warp to zone**: a row opening a zone picker that lists only the zones
   ahead of the current one. Picking one runs the real breach forward.
4. **Unlock research**: on the research screen, an uppercase action key
   (`U`, unless it is taken; the builder checks) unlocks the selected item
   immediately.
5. **Spawn companion**: a row that adds one random species, drawn from what
   can spawn on the player's current field tile, as a companion.

## Non-goals

- Gear in the item picker. Only plain items, the ones that live in
  `Inventory`. Weapons and armour (`GearCopies`, affixes) are out.
- Choosing a species or level for the spawned companion.
- Warping backwards. `warp_to_zone` is forward only and stays that way.
- A text console. The keypad is deliberately rows, not commands.
- Save migration and declarative setup scenarios. Both were considered and
  deferred.

## Anti-goals

- **A cheat must not leak into a save.** Free build is session-only: a
  `DevFreeBuild` resource that is never saved and never inserted by `new`
  or `load`, the same approach as `DevSieges` (`resources.rs:2091`). A
  captured save must not carry the toggle.
- **A cheat must not fake the game.** Each one calls the real game code
  (the real breach, the real research grant, the real adoption path), so
  what you test after cheating is what a player gets. Where a cheat needs
  part of an existing function, extract that part into a function both
  call, never a copy (CLAUDE.md "a call, not a copy").
- **No new RNG dependence on query order.** The companion draw uses the
  game's seeded RNG over a stably ordered candidate list.

## Decisions

### Engine

The existing convention: `pub fn dev_*` on `Game` with `#[doc(hidden)]`,
no `cfg`, gated only in app-core (see `dev_force_raid`,
`base/upkeep.rs:405`). The new methods:

- `dev_set_free_build(bool)` and `dev_free_build() -> bool`. These read
  and write `DevFreeBuild`; a missing resource means off.
- **Free build hooks** *(my call, from the review)*:
  - **The founding Home** already spawns at placement. Free build just
    makes its cost empty.
  - **Every other new build** skips the `BuildSite` and spawns the
    structure directly.
    - The worker-free tail of `raise_one_tick`'s `BuildGoal::New` branch
      (`base/construction.rs:541`) is extracted into a function. That tail
      is `spawn_structure`, `restore_machine_recipe`, the displace calls
      and `log_base`.
    - `raise_one_tick` keeps the worker-specific part (removing `Task`,
      `consume_site`) and then calls the extracted function. Placement
      calls it with no site.
  - **Upgrades:** `upgrade_cost` returns empty while the toggle is on, so
    the UI shows the upgrade as free. The upgrade completes at once
    through the same split of the `BuildGoal::Upgrade` branch.
  - **Ruin rebuilds** (`commit_rebuild_program`, `building.rs:547`) still
    consume the program they rebuild from, because that is the thing
    being rebuilt, not a material. Any material cost is empty, and the
    rebuild completes at once.
- `dev_give_item(id, qty) -> Result<String, String>` checks
  `ItemDb::get`, refuses gear and calls `Inventory::add` on the player.
- **Warp:** the picker calls `warp_to_zone(zone)` (`game/zone.rs:867`),
  either directly or through a thin `dev_` wrapper that refuses during a
  battle.
- `dev_unlock_research(id) -> Result<String, String>` grants the node
  whatever its prerequisites, cost, materials or required subject. Doing
  that inserts into `Research`, or into `KnownRoutines` for routine nodes,
  then logs and notifies as normal completion does. The grant half of
  `settle_research` (`unlocks.rs:1482`) is extracted, and both functions
  call it. If this node is the active research, active research is cleared
  the same way normal completion clears it. *My calls:*
  - The grant half is everything after the bill and subject checks:
    `spend_bill_from_base` and subject consumption are skipped.
  - A node that is already granted is refused with a message.
- `dev_spawn_companion() -> Result<String, String>`:
  - It gets the candidates from `habitat_pools` at the player's tile
    (`spawning.rs:1898`). It passes the same `depth` and `step_bonus` that
    the field spawner passes for that tile, and uses the first tuple
    element (the non-boss candidates).
  - It sorts the candidates by species id and draws one with the seeded
    RNG.
  - It adopts the species through `adopt_program`. The position,
    `stat_mult` and destination come from the existing `adopt_program`
    call site the builder finds closest to a real capture. Check
    `caravan.rs:1341` first, and name the chosen call site in the plan.
  - It refuses with a message when the tile has no candidates or is not
    walkable (`None`).

### App-core

- **New keypad rows:**
  - `DevAction` variants for free build, give item, warp to zone and spawn
    companion, added to `DEV_ROWS` (`dev_console.rs:63`).
  - The free-build row's label shows its current state (on or off).
- **Two new picker modes**, `Mode::DevItemPicker` and `Mode::DevZonePicker`,
  one module each. They follow the `Mode::SpritePicker` pattern:
  - `menu_origin` records where Esc returns to (the keypad).
  - Row selectors are digits 1-9, then lowercase; actions are uppercase.
  - Up/Down scrolls, since there are far more items than selectors.
- **Item picker rows:**
  - Every plain item, sorted by display name.
  - Each row shows the player's current count.
  - Picking a row gives 10 and leaves the picker open, so you can give
    again.
- **Zone picker** rows are the next 20 zones ahead of the current one, or
  up to the last zone if the game has a cap. Each warped zone runs a full
  `enter_next_zone`, which is why it's bounded *(my call)*. Picking one
  warps and closes the picker.
- **Research screen** (`progression.rs:94`, shared by `Research`,
  `RoutineResearch` and `AffixResearch`):
  - The `U` action is live only when the dev console is enabled.
  - Its hint is shown only then.
- Status text and refusals come from the `said` mechanism the keypad
  already uses.

### Gui

- A renderer for each picker under `crates/gui/src/render/`, drawn through
  `Painter` and `draw_popup`, wired into `render/mod.rs`.
- The research screen's hint line includes `U` only when the console is
  enabled.

## Constraints

- It spans the engine, app-core and gui, with no save-format change.
  `DevFreeBuild` is never serialized.
- Repo rules apply: TDD, `cargo clippy --workspace --all-targets`, and
  `cargo test --workspace` as the gate. Run `balance_sim` only if `tuning/`
  is touched; it shouldn't be.
- **Tests:**
  - An engine unit test for each `dev_*` method. Free build must show an
    empty cost and a complete structure. Research unlock must show a
    blocked node granted. The companion test must show a species from the
    tile's pool, with the same result for the same seed.
  - A save→load test proving free build does not survive.
  - App-core tests for each new row and picker, in the style of
    `tests/dev_console.rs`. Those include the existing guard that no two
    rows share an action.

## Left to the builder

- The companion's `adopt_program` call site to mirror (see Engine).
- The exact research key if `U` is taken.
- Picker paging details and the row layout.
- How an item is classed as plain (not gear).
