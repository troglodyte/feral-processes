# Battle props

Authored set pieces stamped onto tactical battle maps. Two kinds of file, one
`.ron` each. A malformed file, or a prefab that cannot be stamped, is skipped
with a logged warning; the rest load. An absent directory is the empty
catalogue (no props).

## `pieces/*.ron`: what a prop is

Every field but `id` is optional.

| Field | Default | Meaning |
|---|---|---|
| `id` | required | Unique name; prefab legends refer to it. |
| `blocks_move` | `false` | Cannot be walked onto. |
| `blocks_sight` | `false` | Stops lines and cones. |
| `cover` | `false` | Counts as partial cover for a shot passing it. |
| `hp` | `None` | `Some(n)` can be destroyed; `None` is indestructible. |
| `armour` | `0` | Subtracted from each hit (a hit always does at least 1). |
| `volatile` | `None` | `Some((radius: 1, damage: 12))` blasts on destruction. |
| `leaves` | `Floor` | `Rubble` or `Floor`: what is left on the cell. |
| `sprite` | `""` | Sprite key, by convention `prop_<id>`; `prop_<id>_cracked` is drawn below half hp when it exists. |
| `decoration` | `false` | Visual only: never blocks and cannot be damaged. |
| `move_cost` | `None` | Cost of crossing a walkable prop; `None` is the ground's own. |

A piece named `rubble` is required: destruction with `leaves: Rubble` places
it (shipped: `move_cost: Some(2)`, no cover, indestructible).

## `prefabs/*.ron`: where pieces go

```ron
(
    id: "rack_row",
    biomes: [Backplane, Deadlock],   // empty or absent = every biome
    weight: 4,                       // relative pick weight, default 1
    rows: ["RR.R", "..c."],
    legend: { 'R': "server_rack", 'c': "cable_run" },
)
```

- `rows` are equal-width strings. `.` leaves the noise underneath alone and
  may not be a legend key. Every other character must be in `legend`.
- A legend entry naming an unknown piece skips the whole prefab.
- A prefab may be stamped rotated by 0, 90, 180 or 270 degrees.
- Decoration pieces are also scattered singly over open ground; they need no
  prefab.
