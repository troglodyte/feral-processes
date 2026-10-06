# Floor items — design

**Status:** spec, awaiting review.

## Intent

The Dwarf Fortress version of an interrupted haul: a carrier that stops
holding its load puts it on the floor of its tile, and a later hauling errand
brings it home. It replaces the stranded set-down added in 0.15.8 (the load
teleported into the nearest accepting Depot) and ends five paths that destroy
a carried load outright.

What the user said (brainstorm, 2026-10-05):

- **Every** interruption drops the load, not only stranding.
- Floor items are **visible** (a glyph on the base map, named when the tile is
  examined); only haulers collect them — the player cannot pick one up.
- A floor item **never decays**.
- **Hauling only**: nothing but an interrupted carrier makes one. Demolition
  spill and death drops are out of scope.
- Approach A (a pile is an entity), with the folded decisions below.

## Today

A load is `components::Carrying { item: ItemId, qty: u32 }`. Items are
fungible counts; only gear has per-copy attributes, and gear never enters
`Stock` or a haul. Haul jobs are not stored: `hauling::haul_step_system`
derives an `Errand` every tick from the load and the post.

A carrier is never freed while it holds a load (`Game::is_on_shift`'s
`Carrying` escape, `game/base/morale.rs`). So a load leaves a carrier only by
delivery, or by:

| Path | Site | Today |
|---|---|---|
| Stranded past `STRANDED_SET_DOWN_TICKS` | `game/base/hauling.rs` (~1488–1503) | `deposit()` into the Chebyshev-nearest accepting Depot; kept if none accepts |
| The post is demolished | `game/base/upkeep.rs:1096` | `remove::<(Task, Carrying)>()` — destroyed |
| Siphon lock-in | `game/base/siphon.rs:70` | removed — destroyed |
| Study | `game/base/study.rs:315` | removed — destroyed |
| Reinforcement call-up | `game/reinforcement.rs:138` | removed — destroyed |
| Downed | `Downed`'s unconditional free (`morale.rs` ~283) | destroyed |

Line numbers are at `821ce769`; the plan re-locates each.

## Design

### 1. Data and save

- `components::FloorPile { items: BTreeMap<ItemId, u32> }` on an entity with a
  base-space `Position`. At most one pile per tile; `Game::floor_pile_at(pos)`
  finds it. No capacity — a drop must never fail. A pile is despawned the
  moment it is empty.
- `spawn_floor_pile` is the one place a pile's component list is written (the
  `spawn_structure` rule's reason: a hand-written copy drifts silently).
- `SaveData.floor_piles: Vec<FloorPileSave { pos, items: Vec<(ItemId, u32)> }>`
  with `#[serde(default)]`. Additive, so an old save loads with no piles and
  `SAVE_FORMAT_VERSION` (34) does not move. Restored beside
  `Game::restore_structures` in `game/lifecycle.rs`.
- No asset or schema change: a pile holds an item whose def already exists.

### 2. Dropping

- `Game::drop_load(who)` takes `who`'s `Carrying`, merges it into the pile on
  `who`'s tile (spawning one if needed) and removes `Carrying`. If `who` has no
  `Carrying` it does nothing.
- All six paths in the table call it in place of what they do today, then
  carry on as before. The stranded path keeps its timer; the nearest-Depot
  set-down and the keep-it branch are deleted.
- Dropping ends the hold, so the `is_on_shift` escape lets go and the carrier
  frees normally. The escape itself is unchanged.
- **New seam:** `drop_load` is the only way a `Carrying` ends other than
  delivery (`deposit`/`Load` at arrival). Written in the order the `seams`
  skill documents: graph, skill, `.claude/rules/seams-base.md`.
- **Out of scope, unchanged:** `CarryingProgram` (a downed program, not an
  item; it keeps `return_carried_program`), a builder's load (`put_back_load`
  already returns it), besieger cargo (`drop_besieger_cargo` returns stolen
  goods to their source).

### 3. Collecting

- New `Errand::Pickup { pile: Entity, item: ItemId, want: u32 }`, derived last
  in the `None =>` arm, after Tend, Collect and Load: a posted, on-shift
  hauler holding nothing, with no other errand, goes to the nearest reachable
  pile (`post_field` / `crew_reach`, as for stores).
- On arrival it takes up to `HAUL_CARRY_CAPACITY` of one item, the pile's
  lowest `ItemId` (deterministic). An emptied pile despawns.
- Delivery is the existing `Deposit` errand derived next tick; the existing
  fallback (the hauler's own machine when no depot has room) applies.
- A hauler cut off with a picked-up load is stranded like any other and drops
  it again on its new tile — one rule.
- No claims: two haulers may walk to one pile; the first takes, the second
  finds less or nothing and re-derives. Taking from an empty or vanished pile
  is a no-op (the plan confirms against `take_from`'s shape).

### 4. Display

- `stands_in_base_space` (`game/inspection.rs`) gets a `FloorPile` arm, so a
  pile never draws on a zone surface.
- `Game` exposes a view row (tile, glyph, colour) built in `views.rs`; the
  renderer never sees the ECS.
- `render/base.rs` draws one glyph per pile under programs and above the floor,
  through `Painter`. The glyph and colour are one named gui constant; a pile is
  the same glyph whatever it holds.
- Examining the tile names the contents ("a pile on the floor: 3 scrap,
  1 wire"); the plan finds the base-space examine path.

## Testing

- Engine unit tests, one per drop path (six): a carrier with a load put
  through the path leaves a pile on its tile holding the load, and no
  `Carrying`.
- Two drops on one tile merge into one pile.
- A posted hauler with a reachable pile picks it up and deposits it; an
  unreachable pile is ignored; an emptied pile despawns.
- Save→load test: drop, save, load, assert the pile's tile and contents (a RON
  round-trip cannot catch a field the save path skips).
- View/examine: a pile produces a view row and a tile description.
- Gates: `cargo test --workspace`, `cargo clippy --workspace --all-targets`.
  `balance_sim` should not move (no tuning, species or item change). Re-run the
  `bench-economy` baseline afterwards and record whether staff measures moved;
  the set-down change could shift them.

## Process

Two crates (engine, gui) and a save field: spec → plan → implementation, a
final opus review, one release on landing. `CHANGELOG.md` gets a line; the
manual and README do not.
