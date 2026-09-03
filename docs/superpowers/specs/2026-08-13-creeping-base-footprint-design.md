# Creeping base footprint — design

**Status:** **superseded, never implemented.** `build_radius_bonus` and `Game::clear_platform` survive only in doc comments recording their retirement; the base became a pocket instead — see `../archive/specs/2026-08-19-base-out-of-phase-design.md`. Audited 2026-09-02 against the source tree, not against this header.

## Problem

The base reads as a small stamp on the map rather than as a settlement.
`MAX_BUILD_DISTANCE_FROM_HOME` is 7, laying a 15x15 slab that is the same
size in the first minute of a run as in the tenth hour. Nothing the player
does makes it bigger, and nothing about it says "this grew".

The complaint is *feel*, not capacity. A real base does not fill 213 tiles.
So the goal is a footprint that visibly grows as the run does, not more room
for machines.

## Approach

Two halves, and each is inert without the other:

1. **Halve the starting footprint** — `MAX_BUILD_DISTANCE_FROM_HOME` 7 to 4,
   a 9x9 slab of ~69 buildable tiles. The opening base is genuinely cramped.
2. **A structure that creeps the edge out by one tile, all the way around.**
   Build a Heap Pillar and the slab grows from 9x9 to 11x11. Build another
   and it grows again. Irreversible, capped, and unlocked by research.

Growth in single-tile steps rather than one large jump is the whole of the
"settlement" reading: the edge moves often, by a little, and the player is
the reason.

### Rejected along the way

- **A 10x10 annex placed against the Home slab.** Considered and dropped.
  It makes the footprint a *set* of rectangles rather than a function of an
  offset, which kills `Platform::covers` as the single statement of the
  base's shape; it needs a direction picker (app-core and gui work) or an
  inferred direction that is ambiguous on a corner; and it makes the Stack
  link failure below *layout*-dependent, so it would fail on some runs and
  not others. A concentric creep keeps the footprint a predicate.
- **Upgrade tiers on one Pillar instead of several Pillars.** `UpgradeDef`
  is where this goes if it ever wants it. For now cost is the gate and a
  flat, summed def field matches `pet_slot_bonus` exactly.
- **Leaving the starting radius at 7.** Then the first Pillar takes the base
  past 15x15, which was judged too big once already
  (`../archive/specs/2026-07-24-battle-flow-and-base-radius-design.md`). Halving
  the start is what buys the growth room.

## The footprint becomes dynamic

`Platform::covers(dx, dy)` is a pure associated function today, and its doc
calls it "the one statement of the base's footprint" — `stamp_platform` lays
the floor, `clear_platform` takes it up, `place_structure` decides what may
stand on it, and a shape any of the three disagreed about puts a machine on
wild ground or leaves orphan floor behind a demolished Home.

That rule survives; only its signature changes. `Platform` gains a `radius`
field and `covers` takes `&self`. It stays one function with three callers.

**The radius is derived, never stored.** It is
`MAX_BUILD_DISTANCE_FROM_HOME` plus the sum of every deployed structure's
`build_radius_bonus`, clamped to `MAX_BUILD_RADIUS_TILES` — computed by a
new `Game::build_radius()` sitting beside `Game::pet_capacity`
(`game/catalog.rs:414`), which is the same shape over `pet_slot_bonus`.

It has to be *cached* on the `Platform` resource rather than queried on
demand, for the reason that resource exists at all: its own doc records that
`distance_from_danger_origin` takes `&self` while querying for structures
needs `&mut self`. The cache is written at exactly the three sites that
write `center` today and nowhere else:

| Site | Writes |
|---|---|
| `stamp_platform` (`game/zone.rs:316`) | `center` and `radius` |
| `clear_platform` (`game/zone.rs:340`) | both to `None`/base |
| load (`game/lifecycle.rs:687`) | both, from the restored Home and structures |

Derived-not-stored is what buys three properties for free, and each would
otherwise be work:

- **No `SAVE_FORMAT_VERSION` bump.** `Platform` is not serialized; the slab's
  tiles come back through `SaveData::tile_overrides` and only the centre is
  rediscovered. Pillars are structures, and structures are saved, so the
  radius is rediscovered the same way. Load restores structures before it
  sets the centre, so the ordering already works.
- **It survives a zone transfer.** `enter_next_zone` repositions structures
  by offset around the new spawn point and *then* calls `stamp_platform`
  (`game/zone.rs:519-530`). The Pillars travel with the base — breaching
  despawns nothing — so the re-stamp is at the right size with no new code.
- **The no-Home fallback stays correct.** No Home means no slab means no
  Pillars; the next Home stamps at the base radius, because `stamp_platform`
  recomputes.

**Placing a Pillar re-stamps.** `place_structure` calls `stamp_platform` for
a Home today (`game/building.rs:126`); it gains a second condition for any
def with `build_radius_bonus > 0`. Re-laying the inner slab writes the same
overrides to the same tiles, so this is idempotent and the new ring gets
floor.

## Where the radius is read

Four consumers follow the live value, and each has a reason it must:

| Site | Why it cannot stay on the constant |
|---|---|
| `place_structure` (`building.rs:36`) | the ask |
| `spawn_initial_creatures` (`spawning.rs:573`) | scatter is widened past the slab because nothing spawns on platform floor; too narrow and a breached-into zone is born empty in its own ring |
| `distance_from_danger_origin` (`spawning.rs:384`) | it already means "distance to the edge of safe territory"; leaving it on the constant makes that sentence false the moment the edge moves |
| `clear_platform` (`zone.rs:334`) | sweeps a box, must sweep the box that is there |

`HAUL_WALK_RADIUS` (`tuning.rs:1501`) is `MAX_BUILD_DISTANCE_FROM_HOME * 2`
today, on the argument that two structures can sit at opposite corners. It
becomes `build_radius() * 2`, passed to `walk_field` — which already takes
it as a parameter (`hauling.rs:161`), so both callers (`haul_step_system`
and `assign_cronjob`) read the live value through `Res<Platform>`/`&mut
Game` respectively. Left as a constant, a fully grown base would refuse
postings across its own width, and `hauling::post_reach` is the single
predicate the cronjob menu and the assignment share — a posting the menu
accepts must be a posting that arrives.

`clear_platform` is the one exception: it sweeps a box sized by
`MAX_BUILD_RADIUS_TILES`, not the live radius. Its doc already explains why
it sweeps the full box rather than `covers`'s cut shape — a save written
before the corners were cut still has floor there and would otherwise keep
it forever. Halving the starting radius creates that situation again for
every existing save, which has a 15x15 slab in `tile_overrides` against a
new base radius of 4. Sweeping the maximum box costs nothing (clearing a
tile that was never stamped is a no-op) and covers both.

## The Stack link on-ramp

This is the part that fails, and it fails in a way that ends runs.

`Game::spawn_surface_links` (`game/stack.rs:140`) places the **first** link
of a zone by drawing uniformly from `[-STACK_NEAREST_LINK_TILES,
+STACK_NEAREST_LINK_TILES]` (8), rejecting Chebyshev distance under
`STACK_MIN_LINK_TILES` (5), and rejecting `Biome::Platform` outright at line
168. Two properties of that loop make a growing slab fatal:

- `reach` widens to `STACK_LINK_SCATTER_TILES` (40) only once `placed > 0`.
- The attempt budget is `count * 40` = 120, **shared across all three
  links**.

So if the on-ramp can never land, the loop spends every attempt at
`placed == 0` and the zone gets **zero links** — not just a missing first
one. `Platform::covers` swallows the entire draw box once the corner cut
stops reaching it: the largest `|dx| + |dy|` inside the box is 16, the cut
spares a tile only when `16 > 2R - 2`, so at **R >= 9 every tile in the box
is platform on every seed**, and at R = 8 only the four `(+-8, +-8)` corners
survive against those 120 shared attempts.

No links means no Stack, no Stack means no Portal Fragments, and
`award_loot` is the only thing in the game that pays one — so the run cannot
breach again. It reads to the player as a bad seed rather than as a bug.

Halving the start moves this failure from "immediately" to "after five
Pillars", which is worse rather than better: the player chooses it, feels
rewarded, and the consequence lands on the next breach.

**Fix: the on-ramp draws from the ring just outside the slab.** Instead of a
box that the slab may have eaten, `placed == 0` draws from Chebyshev
`radius + 1 ..= radius + STACK_NEAREST_LINK_TILES`. This also repairs a
squeeze that exists today at radius 7, where only 64 of the box's 289 tiles
are valid.

`STACK_NEAREST_LINK_TILES`'s doc argues from the viewport — the pane shows
roughly +-16 by +-9, so the on-ramp is meant to be on screen when the player
materializes. Its meaning changes to "on your doorstep": at a grown radius
the base has eaten that viewport itself and the promise is unkeepable, while
`announce_surface_links` already tells the player where the nearest link
lies. The doc gets rewritten rather than left to become false.

`STACK_MIN_LINK_TILES` (5) keeps its own job — links off the arrival tile —
and is now subsumed by the ring's inner bound whenever a base exists.

## Stack depth measures from the slab edge

`frames_for` (`game/stack.rs:102`) buys a frame of depth per
`STACK_TILES_PER_FRAME` (8) tiles between the link and the zone's arrival
point. Pushing the on-ramp out to `radius + 1` therefore makes every stack
deeper as the base grows: at radius 10 the nearest link sits 11 tiles out
and opens a 3-frame stack where it opens a 2-frame one today.

That is a difficulty change caused entirely by a cosmetic one, and depth is
already the live concern in this repo (`MEMORY.md`: "Stack depth 5 may be
unwinnable"). So `frames_for` measures from the **edge of safe territory**,
exactly the correction `distance_from_danger_origin` already makes so the
whole base counts as distance zero. Depth then stays where it is regardless
of base size, and the two distance measures agree instead of pulling apart —
which is the property `frames_for`'s own doc claims ("the same
distance-from-arrival that already scales wild program stats") and would
otherwise quietly lose.

## The opening ring is decoupled

`OPENING_RING_TILES` is `= MAX_BUILD_DISTANCE_FROM_HOME` today, and its doc
gives the reason: "so the ring is exactly your base and its doorstep, and
travels with the base for free once a Home is placed".

Both halves of this feature break that. Halving the start halves the
nursery, 7 tiles to 4, making the opening harder for a reason nobody chose.
Every Pillar afterwards *widens* the nursery, which is a difficulty knob
keyed to base geometry — the thing `CLAUDE.md` records as reintroducing the
distance-scaling bugs removed on 2026-08-05.

So it becomes its own literal `7`, with a doc recording that it used to
derive and why it stopped. The ring keeps the size it has today and stops
moving. Note what this is *not* gated by: `balance_sim` is RNG-free and
models no spawn positions, and
`the_shipped_roster_has_species_on_both_sides_of_the_opening_ring` is a
census of the roster rather than of the radius. Nothing would have caught
the shrink.

## Irreversibility

The player's decision: a Pillar cannot come down.

- `remove_structure` refuses any target whose def sets
  `build_radius_bonus > 0`, **except** when it is swept up in a Home
  cascade — there the whole base is coming down and the radius resets to
  nothing anyway.
- The shipped asset sets `raidable: false`, so it gets no `Durability` and a
  GC Entropy Sweep cannot destroy it. A mod that sets `raidable: true` can
  shrink its own base out from under its structures; that is the modder's
  problem, and no shipped path reaches it.

This is what removes the whole shrink question — no orphaned outer
structures, no partial `clear_platform`, no state the build rules say is
impossible.

**What the creep costs:** `stamp_platform` purges every hostile, nest and
Stack link standing inside the slab, so each Pillar destroys whatever is in
the one-tile ring it claims — including a nest with its cache and any orphan
in it. Consistent with what deploying a Home already does, and thin enough
at one tile to be a footnote rather than a weapon.

**The link refusal:** `place_structure` refuses a Pillar whose new ring
contains a `SurfaceLink`, **before** the materials check, alongside the other
refusals. Same ordering argument as `use_symlink` and `clear_stack`, or
`install_routine` and the disk: a refused action must not have spent
anything. Only the new ring needs scanning — the existing slab has no links
in it by construction.

## Content

- **`assets/structures/heap_pillar.ron`** — name "Heap Pillar", glyph `I`,
  colour `Cyan`, `build_radius_bonus: 1`, `raidable: false`, `work: None`.
  (`Page Pillar` is the alternate if the
  memory-allocation reading is too oblique; the constraint is the
  no-occult-naming rule and the existing Node/Cache/Press vocabulary.)
- **`assets/research/`** — a new gate listing `heap_pillar` in
  `unlocks_structures`. `structure_unlocked` (`game/catalog.rs:385`) returns
  true for anything no research names, so the gate is pure data and costs no
  Rust.
- **`assets/structures/README.md`** — `build_radius_bonus` documented, as
  the schema reference for mods.

## Tuning

| Constant | Value | Note |
|---|---|---|
| `MAX_BUILD_DISTANCE_FROM_HOME` | 7 -> **4** | 9x9, ~69 tiles. "Half" of 7 rounded up; 3 and 5 are the other readings and it is a one-line change |
| `MAX_BUILD_RADIUS_TILES` | **10** (new) | 21x21 fully grown: six Pillars, bigger than today's base, well under the 31x31 that was cut |
| `OPENING_RING_TILES` | **7**, decoupled | was `= MAX_BUILD_DISTANCE_FROM_HOME` |
| `PLATFORM_CORNER_CUT` | 2, unchanged | at radius 4 it trims 3 tiles per corner; the base reads rounded rather than stamped, which is the point |
| `HAUL_WALK_RADIUS` | deleted | becomes `build_radius() * 2` |

## Tests

Each written failing first.

- Placing a Pillar widens the slab: a tile at the old edge + 1 becomes
  `Biome::Platform`.
- A structure builds at the new edge and is refused one tile past it.
- The link refusal fires **and spends no materials** — assert the inventory
  is untouched, which is the half that would pass against the bug.
- A Pillar's radius survives a save/load round trip, and the save still
  loads at the current `SAVE_FORMAT_VERSION`.
- The widened slab survives a breach, at the right size, around the new
  spawn point.
- `spawn_surface_links` places all three links with a fully grown base —
  the direct regression for the softlock, and it must fail against a build
  with the old draw box.
- `frames_for` returns the same depth for the nearest link at base radius
  and at the cap.
- Demolishing a Pillar is refused; demolishing the Home takes it in the
  cascade and `clear_platform` leaves no orphan floor, including from a
  slab wider than the current radius.
- The radius clamps at `MAX_BUILD_RADIUS_TILES`.
- A posted program crosses a fully grown base — `post_reach` against the
  live walk radius rather than the old constant.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`,
and `cargo test -p feral-processes-engine balance_sim` because `tuning.rs`
changed — though note the sim sees none of the ring or spawn geometry, so a
green sim is not evidence about the opening.

## Files

| File | Change |
|---|---|
| `crates/engine/src/tuning.rs` | the table above |
| `crates/engine/src/structures.rs` | `build_radius_bonus: i32`, `#[serde(default)]` |
| `crates/engine/src/resources.rs` | `Platform.radius`; `covers` takes `&self` |
| `crates/engine/src/game/catalog.rs` | `Game::build_radius()` |
| `crates/engine/src/game/zone.rs` | stamp/clear on the live radius; clear sweeps the max box |
| `crates/engine/src/game/building.rs` | range check, link refusal, re-stamp, demolition refusal |
| `crates/engine/src/game/spawning.rs` | two readers |
| `crates/engine/src/game/hauling.rs` | walk radius from the live value |
| `crates/engine/src/game/lifecycle.rs` | load sets radius |
| `crates/engine/src/game/stack.rs` | on-ramp ring; `frames_for` origin |
| `assets/structures/heap_pillar.ron` | new |
| `assets/research/*.ron` | the gate |
| `assets/structures/README.md` | schema |
| `CHANGELOG.md` | entry; version bump at merge |

## Playtest

A green suite is not evidence this feels like anything. The whole point is
visual, so it wants real screen time before release: deploy a Home on the
halved slab, confirm 9x9 reads as cramped rather than broken, build Pillars
and watch the edge creep, and breach with a grown base to confirm the links
are there. `dev-saves/` templates are the way in rather than playing up to
it.
