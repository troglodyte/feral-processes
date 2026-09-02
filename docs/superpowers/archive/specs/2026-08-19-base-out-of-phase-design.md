# The base, out of phase

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

TODO #36. The base stops being a slab stamped onto the zone surface and
becomes a pocket dimension entered through a permanent door. Blackness by
default; you mine blocks out of it and lay tiles down, and entropy reclaims
anything you mined and did not floor.

## Why

The stated itch is a **new build loop**, and the reason to want one is that
the base should become somewhere you live rather than a footprint you
extend. #34 (a base roster with personalities, congregating around power
nodes, acting out when congested) and #35 (structures for entity wellbeing,
capacity bounded by floor space) both presuppose a base that *has* floor
space as a real, scarce, player-made quantity. Today it has a derived radius.
This spec is the groundwork those two need, not a feature that stands alone.

Bug #1 on the same list — a posted program walks to its assignment from the
player's tile rather than from where it was standing — is the same problem
seen from below. CLAUDE.md records the current behaviour as a seam: a tamed
program's `Position` is written at capture and never again, so the stale
value is the tile it was beaten on. There is no fix while base workers have
nowhere of their own to be. Base space gives them one. Out of scope here.

The secondary payoff is a large deletion. `stamp_platform` literally writes
`Biome::Platform` into `WorldMap`'s override overlay, and most of the base's
geometry code exists to manage the consequences.

## Settled decisions

Recorded so they are not relitigated.

1. **The base is a persistent hub.** One pocket dimension for the whole run.
   Breaching swaps the surface zone; the base is untouched. This is what
   makes "portals to older zones" coherent later, and it deletes
   `enter_next_zone`'s offset-snapshot-and-rebuild outright.
2. **Entropy reclaims mined-but-untiled space only.** Floor is permanent.
   Interior does not rot. There is no maintenance treadmill — the pressure
   is on the frontier, and only while you are ahead of your economy.
3. **Raids stay as they are.** Verified, and it costs nothing: `run_raid`
   queries `With<Durability>`, picks one at random and damages it. There is
   no position, no pathing, and no route for a raider to find. A sweep works
   identically in a pocket dimension.
4. **Growth stays economy-gated.** Mining yields nothing. Tiling costs
   `blank_substrate`, at Heap Block's current price, so the existing material
   sink is preserved exactly and the Lathe keeps its customer.
5. **Deploying the Home opens a small pre-cleared pocket** — floor laid,
   roughly today's starting slab. Corrected 2026-08-19 during planning: a new
   run has no base at all, because `game/base/building.rs:25` refuses every
   structure until a Home exists and the Home is player-built. So the pocket
   cannot exist at `Game::new`. Laying it when the Home is deployed is a
   one-for-one replacement for `stamp_platform`, called from the same site,
   and it keeps the opening playing exactly as it does now. Until then base
   space is solid and the anchor refuses entry for want of a base.
6. **The anchor is auto-placed at each zone's spawn point**, free and
   indestructible. You can always get home. A buildable anchor would let a
   run breach into a zone it cannot afford a door for and be cut off from its
   entire economy.
7. **Old saves are refused by version.** The `Platform` fields retire, so the
   format breaks regardless; bump `SAVE_FORMAT_VERSION` (31 → 32) and let the
   first line refuse them, as designed. The `dev-saves/` templates are
   recaptured instead of migrated.

## Architecture

### The grid

One new resource, sparse:

```
BaseGrid { tiles: HashMap<(i32, i32), BaseCell> }
enum BaseCell { Open { mined_at: u64 }, Floor }
```

Absent means Solid — the blackness. Three states, only two stored.

Base space has its own origin. A structure's `Position` is base-space, and
**`Structure` is already the space tag**: there is exactly one spawn site
(`game/lifecycle.rs:738`) plus the load path, both player-deploy, so every
`Structure` entity is in base space by construction. No new marker component
is needed to answer "which space is this entity in".

The exception is posted programs, whose `Position` is base-space, and party
companions, whose `Position` is noise that follows the player. That
asymmetry already exists; this change does not widen it.

### Entropy

One system over the `Open` cells. A cell older than
`BASE_ENTROPY_REFILL_TICKS` reverts to absent when it carries no Floor **and
nothing is standing on it**.

The occupancy clause is not a nicety. Without it a cell can refill under the
player, which is the Stack's "party inside rock" state — and the Stack only
escapes that because `die_in_the_rock` damages and stops rather than writing
`Locale`. Skipping occupied cells closes the hole by construction, with no
new death path and no new exception for either `view_cone` consumer.

### `BaseGrid` is not zone-local

It is saved wholesale, because no seed can reproduce what the player dug, and
it is the one resource here that must **not** be wiped by name in
`enter_next_zone`. That inverts the rule CLAUDE.md states four times over
`StackMemory`, `BuybackLedger`, `PopulatedChunks` and the currencies. The
next person to add a resource in this area will pattern-match to the wipe
list, so this needs its own entry in `docs/seams.md`.

### Crossing, and the door

`resources::Locale` gains a third variant, `Base { x, y }`. The player's
surface `Position` pins to the anchor tile — the Stack's trick exactly, and
it is affordable only because the guard family that stops actions misreading
a pinned `Position` already exists.

The door is **not** a `Structure`, which is what keeps the tag above perfect.
It is a surface entity of its own kind, the way `SurfaceLink` already is —
and unlike `SurfaceLink` it survives `enter_next_zone` rather than being
despawned alongside the hostiles and nests.

### `require_surface` means "not in the Stack"

The load-bearing finding of this design. `Game::require_surface` does not
mean what its name says; today "not underground" and "on the surface proper"
are the same condition, so nothing has ever had to distinguish them. A third
locale forces every site to declare which it meant, and most of them turn out
to want *base* space:

| Site | Wants |
| --- | --- |
| `game/base/building.rs` ×4 | base |
| `game/base/work_orders.rs:954` | base |
| `game/base/collect.rs:28` | base |
| `game/trade.rs` ×4 | base — the Black Market is a deployed `Structure` |
| `game/turn.rs:641` | surface |

These are eleven independent re-readings, not a rename, and a wrong one is
silent. The test for whether a reader needs a guard is unchanged and stated
in CLAUDE.md: not "does it act" but "does it claim something about where the
party is".

### The loop

**Superseded 2026-08-20 by "Slice 2 in detail" below**, which is the
authoritative statement of the loop. Kept here as the shape slice 1 was built
against, and as the record of what was reconsidered:

- **Mine** — a directional action from the player's tile; adjacent Solid
  becomes Open. Costs a turn, yields nothing.
- **Tile** — Open becomes Floor, for `blank_substrate` ×1.
- **Build** — `place_structure`'s slab check becomes `BaseGrid::is_floor`.
  Nothing else about deployment changes.

Only the third survives unchanged, and it shipped in slice 1. Mining became a
thing you *hit*, with a trickle of Core Fragments and a crew you can post to
it; tiling stayed the money. One knob became three. Per CLAUDE.md, the knobs
belong in `tuning.rs` and the price belongs in the asset that charges it.

### Rendering

`render/base.rs` already draws a tile grid with entities on it, through
`Painter`. Base space is that same shape: a locale switch and a palette —
Solid a filled dark block, Open dark and hatched, Floor the colour the slab
already draws. `paint.rs` is untouched; the fourteen-operation seam is
exactly what should not need widening for this.

**No view cone and no fog.** The Stack has both because it is unknown; the
base is not. Stated as a deliberate omission so nobody adds sight rules to it
later for consistency.

## What retires

- `resources::Platform` entirely — `center`, `radius`, `claimed`, `covers`,
  `in_shape`
- `Game::build_radius`, with its cached-radius-and-exactly-three-writers
  arrangement. This is the reason TODO #21 passed on a build-radius perk; the
  obstacle stops existing.
- `stamp_platform`, `clear_platform`, `claim_ground`, and every
  `Biome::Platform` write into `WorldMap::overrides`
- `spawn_surface_links`' Chebyshev-ring-outside-the-slab walk, and with it
  the hazard where a wide slab consumed the shared attempt budget and yielded
  *zero* links for the whole zone
- `Tile::open_to_hostiles`' slab clause (`world.rs:102`), and `pursuit_field`'s
  separate `Biome::Platform` exclusion (`game/pursuit.rs:110`)
- `enter_next_zone:499-535` — the offset snapshot and rebuild
- Heap Pillar and Heap Block, whose whole purpose was slab growth
- `save::PlayerSave::claimed_tiles`

`MAX_BUILD_DISTANCE_FROM_HOME` survives only if something still needs it;
it is the radius a base *starts* at, and the starting pocket is now the thing
that expresses that.

`OPENING_RING_TILES` and `distance_from_danger_origin` are untouched in
meaning — the origin becomes the anchor rather than the Home, and the ring
still gates nothing but `in_opening_ring`.

## Saves

- `SAVE_FORMAT_VERSION` 31 → 32.
- Root gains `base_grid`; `claimed_tiles` is removed. A field *removed* is
  precisely the case field-named RON does not excuse from a bump.
- Six `dev-saves/` templates recaptured: `chains`, `contracts`, `deep-lair`,
  `extraction`, `rarity-preview`, `stack`.
- Needs a real save→load test and not only the RON round trip — a round trip
  cannot catch a `#[serde(skip)]`.

## Slices

Each lands green and playable on its own.

**1. The space, and the base moves into it.** `BaseGrid`, `Locale::Base`, the
anchor entity, enter and exit, the renderer, the eleven-site `require_surface`
split, `Platform` retires, the save bump, and an opening onto a pre-cleared
pocket the size of today's slab. Deliberately *feels identical to play* — it
is a pure relocation, which is what makes it verifiable: anything that plays
differently after slice 1 is a bug in it.

**2. Growing the base.** Rock you hit, tiles you lay, a crew that digs what
you mark, and entropy on the frontier. **Redesigned 2026-08-20** — see
"Slice 2 in detail" below, which supersedes this line and records what it
replaced.

Heap Pillar and Heap Block retire in slice **1**, not here — corrected
2026-08-19 during planning. Their entire mechanism is `build_radius_bonus`
and `claims_ground`, both of which die with `Platform` in slice 1; leaving
them shipped through slice 1 would leave two structures that cost materials
and do nothing.

**3. Surface cleanup.** The link spawner, `open_to_hostiles`, `pursuit_field`,
and the `tuning.rs` doc comments that describe a slab. Kept separate so slice
1 can land without touching the Stack's on-ramp.

Held back as its own spec: **portals to older zones**, which only becomes
possible once the hub exists. **Postable mining** was held back here too, on
the grounds that it was #34/#35 groundwork rather than part of this; it moved
into slice 2 on 2026-08-20, because a base meant to become elaborate cannot
be dug a keypress at a time.

## Slice 2 in detail: growing the base

**Redesigned 2026-08-20.** Everything in this section supersedes the
one-line sketch above. What it replaces, recorded so the old shape is not
reintroduced by someone reading a stale summary: mining was a directional
action costing one turn a cell and yielding *nothing*, tiling was a second
action, and entropy was the only pressure.

### Why it was reopened

A frontier that is pure cost under a punishment clock collapses mining and
tiling into a single action with bookkeeping between them — you dig exactly
what you can floor this minute, because digging further is confiscated and
digging costs turns you get nothing for. And a dark room you pay by the tile
to enlarge is still a footprint you extend, which is the thing this spec's
own **Why** section says the base should stop being.

### Settled decisions

1. **Rock is hit, not walked through.** Stepping into a solid cell strikes
   it, in the same branch position `move_player` holds its nest branch. No
   new key and no direction prompt: the wall is a thing you attack, a couple
   of swings bring it down, and it reads exactly like wearing a nest down
   because it *is* that code path's shape. Damage is the weapon band's mean
   plus `effective_atk`, floored at 1, and **deterministic** for the reason
   `attack_nest` gives: identical swings have to stay identical, or wearing a
   wall down becomes a slot machine.
2. **Rock durability is never scaled by zone, depth or level.** The rock is
   the same rock all run, so the thing that changes is you. A wall that took
   three swings at level 1 takes one late, and that is the reward rather than
   a curve to tune.
3. **There is one representation of rock-in-progress: the `DigSite`
   entity.** It carries `Durability` and whether it is marked, and it is
   spawned lazily — by your first swing at a wall, or by marking one. This is
   why `BaseCell` gains **no** `Rock` variant: absent from `BaseGrid` still
   means solid and untouched, and the module's stated invariant survives
   whole. The alternative considered and rejected was a `Rock { chipped }`
   cell variant, which would have needed a second, parallel representation
   the moment a crew had to be posted to one — `schedule_base_labour` works
   in `(Entity, TaskKind)` pairs and cannot address a coordinate.
4. **A mark is one verb, and what it means is derived from the cell under
   it.** Marked solid means cut it; marked `Open` means floor it. So a
   marked wall runs the whole way through — the crew cuts it, the mark
   survives the cut, the same crew floors it, and the mark clears — and
   "mark walls to be mined out and tiled" is the default path rather than a
   combination the player has to assemble. There is no second designation
   kind and no separate erase verb: a box whose **anchor cell** is already
   marked clears instead of marking.
5. **Digging pays a trickle and can never be income.** A cut cell rolls
   `BASE_MINE_FRAGMENT_CHANCE` for one Core Fragment. A floor tile costs a
   Blank Substrate, itself four Core Fragments at a Lathe, so a dug cell
   returns a fraction of what flooring it costs *by construction*. Raising
   the chance past that ratio turns the wall into a fragment tap that
   undercuts the Mining Node, which is the change to refuse.
6. **A crew complains only when the errand is the player's.** A marked cell
   with every neighbour still solid is the normal interior of any block you
   mark and resolves itself as the shell comes down — skipped **silently**.
   A marked cell with a standable neighbour and no route to it is stuck until
   you do something, and **says so once**. This is `hauling::post_reach`'s
   existing `BoxedIn`/`NoRoute` split, kept for the reason CLAUDE.md already
   gives for it: the two leave the player different errands. The
   announcement follows `set_machine_status` — one writer, logging **only on
   transition**, because entering a state is news and staying in it is not.
7. **Dig jobs are the lowest priority in `schedule_base_labour`**, below
   work orders and below standing jobs. A spare body digs; a needed one does
   not. Digging must never starve production, and the base must not stop
   running because you marked a corridor.
8. **Tiling costs one Blank Substrate**, unchanged from the original sketch,
   which is what preserves the existing material sink exactly and keeps the
   Lathe its customer. The laid form is a **VectorStasis Tile** — the
   substrate is raw stock in the store, the tile is what it becomes
   underfoot. `BaseCell::Floor` keeps its code name; this is the player's
   word for it, the same way "GC Entropy Sweep" is the player's word for a
   raid.
9. **Base space still has no bound.** Open question 3 stays open on purpose:
   a bound before #35 designs floor-space capacity is a guess about a
   mechanic that does not exist. The player-facing intent is that a base
   becomes elaborate, and the knobs below are what make its pace tunable.

### Build mode

`m` opens **Excavation plan**: a mode, not an action, so entering it and
moving the cursor cost no turns and no tick. The cursor starts on the
player's cell and moves on `hjkl`/arrows. `space` drops an anchor, moving
previews a rectangle, `space` again commits it; a single cell is a 1x1 box.
Whether the box marks or clears is decided by the anchor cell, per decision
4. `esc` leaves.

app-core owns the mode and the cursor position; gui draws the cursor, the
box preview and a tint on marked cells. **No new `Painter` operation** — this
is tint and glyph over the grid `render/base.rs` already draws, and the
fourteen-operation seam is exactly what should not need widening for it.

### The pieces

| Piece | Where |
| --- | --- |
| `DigSite` — `Durability` plus marked state, spawned lazily | engine, base space |
| `strike_rock` — the bump branch, damage mirroring `attack_nest` | `game/base_space.rs` |
| Break: cell becomes `Open { mined_at }`, roll for one Core Fragment | same |
| Lay a VectorStasis Tile: stand on `Open`, spend one Blank Substrate | `game/base/building.rs` |
| Dig jobs as `(Entity, TaskKind)` posts at the lowest priority | `schedule_base_labour` |
| A crew flooring a marked `Open` cell, paid from the store builds pay from | same |
| `base_entropy_system` — unoccupied `Open` past the knob reverts to solid | new system |
| Marks and chip progress, additive and `#[serde(default)]` | `save.rs` |

Rendering needs nothing new for the three cell states: `Game::view_tiles`
already maps `Floor`, `Open` and absent onto `Biome::Platform`,
`Biome::Excavated` and `Biome::Entropy`, and the existing surface renderer
draws them. Only the mark tint and the cursor are new.

### The seam this widens

`DigSite` is a **non-`Structure` entity carrying a base-space `Position`**.
That widens the rule this spec states above — "`Structure` is already the
space tag" — whose only prior exception was posted programs. It needs its own
entry in `docs/seams.md`, because a `Position` read in the wrong coordinate
space is silent, and 0.13.0 shipped fixes for exactly that bug class
(`power_regen_system` and the structure roster's "Work it yourself" row).

### Tuning, all of it unmeasured

Starting values to play and then record under `docs/measurements/`, none of
them derived from anything:

- `BASE_ROCK_DURABILITY = 24` — a level-1 player swings for about 11
  (`PLAYER_UNARMED_DAMAGE` mean 5 plus `PLAYER_BASE_STATS::atk` 6), so about
  three hits early and one late.
- `BASE_MINE_FRAGMENT_CHANCE = 0.25`, bounded above by decision 5.
- `BASE_ENTROPY_REFILL_TICKS = 300`, the spec's original open question 1,
  still unmeasured and now measurable.

`balance_sim` has no base term and gates none of this. A session is the
instrument.

### Saves

Additive only: a saved `DigSite` list and nothing whose meaning changes.
`SAVE_FORMAT_VERSION` **stays at 32** — an additive change behind
`#[serde(default)]` costs no bump. A save-to-load test is still required and
not only the RON round trip, which cannot catch a `#[serde(skip)]`.

## Testing

Per slice, in the engine crate:

- **Slice 1** — the base survives a breach unchanged (structure count,
  layout, `BaseGrid` contents identical either side of `enter_next_zone`);
  entering and leaving through the anchor round-trips the player's
  `Position`; each of the eleven re-read guard sites refuses in the locale it
  should and permits in the one it should; a save→load round trip preserves
  `BaseGrid`; no `Biome::Platform` tile is written to `WorldMap` anywhere.
- **Slice 2** — an Open cell reverts after `BASE_ENTROPY_REFILL_TICKS`; an
  occupied Open cell does not; a Floor cell never does; tiling refuses
  without substrate and spends exactly one on success; `place_structure`
  refuses a non-Floor cell. Added 2026-08-20: a wall takes the swings
  `BASE_ROCK_DURABILITY` implies and opens on the last one; a marked wall is
  cut and *then* floored by a crew, with the mark surviving the cut and
  clearing on the floor; a marked cell whose neighbours are all solid is
  skipped **silently** while one with a standable neighbour and no route
  complains **once** and not again while it stays stuck; a dig job never
  displaces a body from a work order; a `DigSite` and its chip progress
  survive a save-to-load round trip.
- **Slice 3** — link placement no longer consults a slab, and a zone still
  yields its three links.

`balance_sim` has no base term and is not a gate for any of this. The
instruments that can see it are a session and the `dev-saves/` templates,
which is why recapturing them is part of slice 1 rather than a chore
afterwards.

## Open questions

1. **`BASE_ENTROPY_REFILL_TICKS` has no measured value**, and neither do
   `BASE_ROCK_DURABILITY` or `BASE_MINE_FRAGMENT_CHANCE`. The first wants to
   be long enough that a normal dig-then-floor cycle never loses ground and
   short enough that over-digging is felt. All three are unmeasurable before
   slice 2 is playable; the starting values are in "Tuning" above, and what
   they measure to belongs under `docs/measurements/`.
2. ~~**How big is the starting pocket, exactly?**~~ Closed by slice 1: it is
   the slab's own chamfered box at `STARTING_POCKET_RADIUS` = 4, 69 cells,
   `PLATFORM_CORNER_CUT` and all, so the opening plays as it did.
3. **Does base space need a bound?** Nothing here stops a player mining
   outward forever, and `HashMap` growth is the only cost. A bound may be
   wanted once #35 ties capacity to floor space — but adding one before that
   feature exists is a guess about a mechanic that has not been designed.
