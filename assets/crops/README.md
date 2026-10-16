# Crops

One `.ron` file per crop: what a farm plot can grow, the item one harvest
yields, and how long it takes. Drop a file in, restart, and it is in the
world. Nothing else to edit.

## Schema

```ron
(
    id: "power_cell",
    name: "Power Cell",
    description: "Staff dimple phase space here, and a cell slowly grows.",
    produces: "power_cell",
    grow_ticks: 1200,
    stages: 4,
    glyph: "+",
    color: Yellow,
)
```

| Field | Type | Meaning |
|---|---|---|
| `id` | string | Save key. |
| `name` | string | The crop picker's row label. |
| `description` | string | One line of flavour. |
| `produces` | item id | The item one harvest yields. One harvest yields one item per tile. Must name a loaded item. |
| `grow_ticks` | integer, above zero | Ticks a sown tile takes to ripen. |
| `stages` | integer, above zero | How many looks the crop passes through between sown and ripe. |
| `glyph` | string, not empty | The character a plot tile draws. |
| `color` | glyph colour | The colour that character draws in (`Yellow`, `Cyan`, ...). |

## What is skipped

A file is skipped with a logged warning, and costs the install nothing else,
when it:

- is malformed, or names a colour that does not exist;
- has `grow_ticks: 0` or `stages: 0` (both divide in the stage maths);
- has an empty `glyph` (it would draw nothing).

A crop whose `produces` names no loaded item is dropped with a warning once
items have loaded.

Two files with the same `id` do not warn: the last one in path order wins.

## No cost field

There is deliberately no cost or input field: sowing is paid in labour, not
materials, so a crop cannot be priced against another resource. Tune a crop
through `grow_ticks`.
