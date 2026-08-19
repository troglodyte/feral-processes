# Sectors

One `.ron` file per sector. A sector is what makes a zone somewhere rather
than a difficulty number: it shifts where the world generator's biome
boundaries fall, and its look, its wild roster and where a base can be built
all fall out of that one change.

Adding a sector never requires touching Rust. A malformed or unusable file is
skipped with a warning at startup, and the rest of the directory still loads.

**Deleting this directory restores the pre-sector game exactly.** Every zone
then generates at the neutral shape with the shipped colour table, the way it
did before sectors existed. That is a supported way to play, the same as
deleting `assets/affixes/` or `assets/policies/enemy_battle.ron`.

## Which zone gets which sector

You don't choose. A zone's sector is *derived* from the world seed and the
zone number — both already in the save — so it survives a reload and a
different world reads differently for free. Nothing about a sector is stored
in a save file.

**Zone 1 is always neutral**, whatever this directory contains. That is not
politeness: the opening zone's roster is chosen from the species a fresh
player can actually beat, and biasing zone 1's biome mix would move that
roster while looking like a cosmetic change.

## Schema

```ron
(
    id: "cold_storage",
    name: "Cold Storage",
    description: "Long-idle allocations, frost-locked and slow to answer.",
    shape: (
        deadlock_temperature: 1.15,
    ),
    palette: (
        ground_hue: 200.0,
        hazard_hue: 12.0,
    ),
)
```

| Field | Required | Meaning |
| --- | --- | --- |
| `id` | yes | Unique key. A second file with the same id replaces the first. |
| `name` | yes | Player-facing. Announced on the breach line. |
| `description` | yes | Player-facing. One sentence, announced beside the name. |
| `shape` | no | Threshold deltas; see below. Omitted means neutral terrain. |
| `palette` | no | Two hues; see below. Omitted means the shipped colours. |

### `shape`

The world generator sorts three noise fields — elevation `e`, temperature `t`
and moisture `m` — into six biomes at five thresholds, in this order:

```
e < void_elevation                          -> Data Void      (a hole)
e > black_ice_elevation                     -> Black Ice      (a hole)
t < deadlock_temperature                      -> Deadlock
t > null_temperature && m < null_moisture   -> Null Sector
m > mainframe_moisture                      -> Mainframe
otherwise                                   -> Open Grid
```

Neutral values are `void_elevation: -0.3`, `black_ice_elevation: 0.55`,
`deadlock_temperature: -0.3`, `null_temperature: 0.3`, `null_moisture: -0.1`,
`mainframe_moisture: 0.15`.

**Every field in `shape` is a delta added to the neutral value, not a
replacement**, and every one defaults to `0.0`. So a whole sector is usually
one line, and a threshold you say nothing about stays exactly where it is.

Two things follow from the ordering above that are easy to get wrong. The two
elevation tests run *first*, so a hole wins over any biome the temperature or
moisture tests would have produced on that tile — which is why the two
elevation deltas are the only ones that change how much ground there is.
And Null Sector is the one biome gated on a *pair* of fields, so shifting it
usually means moving `null_temperature` and `null_moisture` together (see
`arid.ron`).

Temperature is also latitude-dependent and sits near `1.0` around the origin,
which is why `cold_storage.ron` has to raise the Deadlock floor by more
than a whole unit to reach it.

#### A sector must leave ground to stand on

A shape that turns most of the map into Data Void and Black Ice is not merely
ugly — it is a stranded run. The party has to materialize somewhere on breach,
and every spawn, structure and Stack entrance refuses an unwalkable tile.

So a sector is sampled at load and **refused if it leaves less than
`MIN_SECTOR_WALKABLE_FRACTION` of the ground standable** (see
`crates/engine/src/tuning.rs`). The sample is taken across three fixed seeds
around the origin and the *worst* of the three is the verdict, because a shape
that leaves ground in one world is not evidence it leaves ground in every
world. A neutral sector scores about `0.61`; `fractured.ron` is the shipped
sector closest to the floor, at about `0.49`.

### `palette`

Two hues in degrees. `ground_hue` colours every biome that can be walked on;
`hazard_hue` colours the two that are holes in the map.

The map has one load-bearing promise: **hue answers "can I cross this",
pattern answers "what is it"**. So a palette moves the two *bands* and cannot
move biomes around inside them. Each biome keeps its own offset from its
band's anchor, along with its saturation and brightness — which is what keeps
the five walkable biomes distinguishable from each other, and what keeps the
base platform much the darkest thing on a base screen.

| Field | Neutral | Allowed |
| --- | --- | --- |
| `ground_hue` | `180.0` | `150.0`–`240.0` (the cool band) |
| `hazard_hue` | `20.0` | `0.0`–`40.0` (the warm band) |

**A hue outside its band is refused with a warning**, and the bounds are not
a matter of taste: they are how far the band can swing before a walkable
biome starts reading as hostile once the per-biome spread is added back on.
A test sweeps the whole of both ranges against every biome, so widening them
fails there rather than shipping a map that tells the player they may walk
into the void.

Biome *textures* are deliberately not authorable. Each biome's pattern is its
identity — closer to a glyph than to decoration — and varying them per sector
would mean relearning the map on every breach.

## Shipped sectors

| File | What it does |
| --- | --- |
| `cold_storage.ron` | Deadlock over most of the ground; holes unmoved. |
| `fractured.ron` | Both elevation thresholds close in; about half the map is holes. |
| `arid.ron` | Null Sector over most of the ground; as walkable as neutral. |
