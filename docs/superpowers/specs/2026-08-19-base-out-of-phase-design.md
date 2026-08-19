# The base, out of phase

**Status:** approved 2026-08-19, unimplemented. (Spec `**Status:**` headers
in this repo go stale — answer from `CHANGELOG.md` and `rg`, never from this
line.)

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

- **Mine** — a directional action from the player's tile; adjacent Solid
  becomes Open. Costs a turn, yields nothing.
- **Tile** — Open becomes Floor, for `blank_substrate` ×1.
- **Build** — `place_structure`'s slab check becomes `BaseGrid::is_floor`.
  Nothing else about deployment changes.

One knob (`BASE_ENTROPY_REFILL_TICKS`) and one price. Mining is free but
slow, tiling is the money, and entropy punishes digging further ahead than
the economy can floor. Per CLAUDE.md, the knob belongs in `tuning.rs` and the
price belongs in the asset that charges it.

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

**2. Mining, tiling, entropy.** Two actions, one system, one price, one
timer.

Heap Pillar and Heap Block retire in slice **1**, not here — corrected
2026-08-19 during planning. Their entire mechanism is `build_radius_bonus`
and `claims_ground`, both of which die with `Platform` in slice 1; leaving
them shipped through slice 1 would leave two structures that cost materials
and do nothing.

**3. Surface cleanup.** The link spawner, `open_to_hostiles`, `pursuit_field`,
and the `tuning.rs` doc comments that describe a slab. Kept separate so slice
1 can land without touching the Stack's on-ramp.

Held back as their own specs: **portals to older zones** (only becomes
possible once the hub exists), and **postable mining**, which is #34/#35
groundwork rather than part of this.

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
  refuses a non-Floor cell.
- **Slice 3** — link placement no longer consults a slab, and a zone still
  yields its three links.

`balance_sim` has no base term and is not a gate for any of this. The
instruments that can see it are a session and the `dev-saves/` templates,
which is why recapturing them is part of slice 1 rather than a chore
afterwards.

## Open questions

1. **`BASE_ENTROPY_REFILL_TICKS` has no measured value.** It wants to be long
   enough that a normal dig-then-floor cycle never loses ground and short
   enough that over-digging is felt. Unmeasurable before slice 2 exists;
   pick a plausible value, play it, and record the result under
   `docs/measurements/`.
2. **How big is the starting pocket, exactly?** "Today's slab" is a circle of
   radius `MAX_BUILD_DISTANCE_FROM_HOME` = 4. Whether the pocket is that
   circle or a rectangle of similar area is a feel question for slice 1.
3. **Does base space need a bound?** Nothing here stops a player mining
   outward forever, and `HashMap` growth is the only cost. A bound may be
   wanted once #35 ties capacity to floor space — but adding one before that
   feature exists is a guess about a mechanic that has not been designed.
