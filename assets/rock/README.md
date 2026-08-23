# Rock kinds

One `.ron` file per kind of base-space rock. A kind decides how hard a wall
is to cut, how few swings it can ever fall in, and how bright its exposed
face is drawn.

**Which kind a given cell is, is derived and never stored.** `BaseGrid` is a
sparse map in which a coordinate absent from it *is* solid rock, so there is
nowhere to write a kind without making the sparse map not sparse.
`RockDb::kind_at` folds base space's own seed with the *block* a coordinate
falls in and reduces the result to a kind by weight — so kinds come in
patches with an inside, adding a file widens the pool for the whole map, and
a wall you have never touched still knows what it is.

Drop a file in, restart, and it is in the world. Nothing else to edit.

## Schema

```ron
(
    id: "compacted",
    name: "Compacted Entropy",
    weight: 22,
    durability: 60,
    min_swings: 3,
    shade: 1.9,
)
```

| Field | Type | Meaning |
|---|---|---|
| `id` | string | Unique. Two files sharing one id keep whichever loads last. |
| `name` | string | What examining an exposed face calls it. |
| `weight` | u32 | Relative share of base space. Must be at least 1 — a kind nothing can roll is an authoring mistake, not a way to disable a file. |
| `durability` | u32 | Damage one cell absorbs before it opens. |
| `min_swings` | u32 | The fewest swings this kind can *ever* fall in. |
| `shade` | f32 | Brightness of an exposed face, `1.0`..`4.0`. |

## `min_swings` is the one that matters

A swinger's damage is their own — a stronger program cuts a wall in fewer
swings rather than faster ones — so durability alone does not bound anything:
a developed player hits hard enough to open any wall you can author in one
blow. `Game::strike_rock` therefore caps a single swing at
`durability / min_swings` (rounded up).

Set it to `1` and your kind is demolishable by an accidental keypress at a
high level, which is the bug this field exists to close. Every shipped kind
is at least `2`.

## `shade` is a brightness, not a colour

The map has one colour rule: **hue answers "can I walk here"**. Rock is a
hole in the map and shares its hue with the other holes, so a kind is told
apart by brightness instead — the same axis that already separates cut
ground from uncut. A file whose `shade` falls outside `1.0..4.0` is skipped
with a warning; below `1.0` an exposed face would be *harder* to see than the
wall around it.

The sector's own palette rotation applies on top, so a kind stays inside the
impassable band under every sector.

## Failure modes

A malformed file, or one failing any bound above, is skipped with a logged
warning — the rest of the directory still loads.

**An empty directory is supported**: base space becomes one uniform built-in
kind, which is the game as it was before kinds existed. It does *not* restore
one-swing walls — the built-in kind carries `min_swings: 2`, because the
swing floor is a bug fix rather than content.
