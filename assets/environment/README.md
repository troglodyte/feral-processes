# Environment effects

One `.ron` file per ambient effect. An ambient effect is what the ground
*does* to you for walking on it, keyed to the biome the world generator
already sorts terrain into. A file names one or more biomes and one effect,
and every tile of those biomes has it from then on.

Adding one never requires touching Rust. A malformed or unusable file is
skipped with a warning at startup, and the rest of the directory still loads.

**Deleting this directory restores the pre-environment game exactly.** Ground
goes back to being scenery everywhere, the way it was before ambient effects
existed. That is a supported way to play, the same as deleting
`assets/sectors/` or `assets/affixes/`.

**Zone 1 takes no ambient effects**, whatever this directory contains. The
opening zone is where a run learns the game, and ground that bites there is a
tax on the tutorial rather than an exception to it. Biome *names* are not
gated this way — the ground is named from the first step of a new run.

## Schema

```ron
(
    id: "standing_frost",
    name: "Standing Frost",
    description: "The floor pulls heat out of anything that stops on it.",
    biomes: [Deadlock],
    effect: Attrition(hp_percent: 0.02, min_damage: 1),
)
```

| Field | Meaning |
| --- | --- |
| `id` | Unique identifier. Also what a warning names when two files clash. |
| `name` | What the player calls this ground's condition. Distinct from the biome's own name — a biome is *what* the ground is, this is what it is *doing*. |
| `description` | One sentence, in the game's voice. |
| `biomes` | Which biomes this claims. One or more of `DataVoid`, `Deadlock`, `NullSector`, `Mainframe`, `OpenGrid`, `BlackIce`. |
| `effect` | One of the two shapes below. |

### `Attrition(hp_percent, min_damage)`

Takes a bite out of the player's Integrity on every step onto this ground:
`max(max_hp * hp_percent, min_damage)`. The party is never touched.

A fraction of maximum Integrity rather than a flat figure, because terrain is
uncorrelated with player level — any constant is lethal at level 1 and free by
mid-run. `min_damage` is the floor that keeps the effect from rounding away at
low levels.

The bite goes through the same damage path a fight does, so mitigation and
field buffs apply to it exactly as they would to a hit.

### `Drag(extra_ticks)`

The step costs `extra_ticks` more ticks than the one every step costs. Takes
no Integrity: the second shape exists so hostile ground is not all damage.

The world keeps running during those ticks — production advances, needs decay,
and a wandering program can walk into you. That is the cost.

## What a file may not do

Three refusals. Each is skipped with a warning naming the file and the reason,
and each protects something a file has no business revoking:

- **`Platform` may not be claimed.** The base slab is the one safe ground in
  the game: nothing spawns there and no ambush fires there. A base is also
  stamped over whatever terrain it lands on, so ground that bit there would
  make the safe floor depend on where you built.
- **`hp_percent` is capped** at `tuning::MAX_ENVIRONMENT_ATTRITION`. Terrain
  cannot be fled, refused or out-levelled, and a step is the cheapest action in
  the game — an authored `0.5` is death in two steps with no decision in
  between. `min_damage` may not be negative, which would heal.
- **`extra_ticks` is capped** at `tuning::MAX_ENVIRONMENT_DRAG_TICKS`. A tick
  runs the whole simulation; an authored `10_000` is a hang the player cannot
  tell from a crash.

Two files may not claim the same biome. The second one read is skipped with a
warning naming both, rather than resolved silently — files are read in sorted
order, but a game that quietly picked a winner would make a modder's install
differ from the one they tested for reasons nothing on screen explains.

Naming a biome nothing can stand on (`DataVoid`, `BlackIce`) is **not** an
error. Those are holes in the map; the effect is simply never reached, and a
mod naming every biome for convenience should not be refused for it.

Terrain never costs Power and never raises Trace. Both are resources the
player spends deliberately, and ground that drained them would price walking
in a currency the player is budgeting for something else.
