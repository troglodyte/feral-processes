# Caravans

One `.ron` file per travelling trader. Drop a file in to add one; nothing in
Rust needs editing.

A caravan is not a structure and not a program. It walks in from the sector
on its own schedule, phases into base space through the anchor, stands beside
the iso Market for a while selling a shelf, and walks back out. It only ever
visits a base with an iso Market standing.

**An empty directory is supported.** With no files here nothing ever visits,
which is the game exactly as it was before caravans — the same rule
`assets/memories/` and `assets/nemesis/` follow. A malformed file is skipped
with a logged warning and costs you that one trader, never the startup.

## Schema

```ron
(
    id: "salvage_convoy",
    name: "Salvage Convoy",
    description: "Three carts of other people's gear ...",
    glyph: 'Ω',
    color: DarkGreen,
    rows: 12,
    weights: (
        gear: 6,
        routines: 2,
        programs: 0,
        materials: 2,
    ),
    bonus_share: 35,
    min_zone: 1,
    max_zone: 99,
)
```

| Field | Type | Meaning |
| --- | --- | --- |
| `id` | string | Unique. Two files claiming one id resolve by sorted filename, and the id is what the derived schedule picks by — so renaming one changes which trader a given seed sends. Must not be empty. |
| `name` | string | What the arrival line, the map label and the shelf header call it. |
| `description` | string | One line of flavour under the header, and what the examine ray reads out. The player's vocabulary, not the code's. |
| `glyph` | char | The mark it wears on the zone map and the base map. |
| `color` | enum | One of `White`, `Gray`, `Green`, `DarkGreen`, `Red`, `Yellow`, `Blue`, `Magenta`, `Cyan`, `Brown`, `Orange`. |
| `rows` | u32 | How many rows its shelf holds. Must be at least 1. This is the trader's own number and is deliberately **not** a `tuning.rs` constant — how much stock a particular trader carries is content. |
| `weights` | struct | Relative weights across the four row kinds; see below. At least one must be non-zero. |
| `bonus_share` | u32 | Percentage of this trader's **gear** rows that are standout stock; see below. Defaults to `0`, and must not exceed `100`. |
| `min_zone` | u32 | First sector this trader may visit in, inclusive. |
| `max_zone` | u32 | Last sector, inclusive. Must not be below `min_zone`. |

### `weights`

Four `u32`s — `gear`, `routines`, `programs`, `materials` — each defaulting
to `0`. They are *relative*, not counts: a `(gear: 6, materials: 2)` trader
fills roughly three quarters of its shelf with equipment whatever `rows`
says. That split is what makes two traders read as two different shelves
instead of two draws from one table.

- `gear` — a rolled equipment copy, with its own rarity, affix and quality.
- `routines` — a Routine Disk.
- `programs` — a tamed program, priced by its power.
- `materials` — a plain stack of a craftable or salvage item.

### `bonus_share`

Gear rows are dealt round-robin across the three equipment slots, so any
shelf with three or more of them stocks a weapon, a piece of armour and a
module — a trader is a shop that carries a range, not a rack of whatever the
item set has most files of.

`bonus_share` then decides how many of those rows are **standout stock**
rather than plain drop-rate rolls. A standout row is guaranteed an affix,
draws its rarity from a narrowed window so it is likelier to come up above
Ordinary, and rolls its quality off the item's *authored* figure instead of
below it. Everything else on the shelf rolls exactly as a drop does.

It is a share of the gear rows and not a count, so it keeps its meaning when
you change `rows` or reweight the shelf. It **rounds up**: any non-zero share
puts at least one standout row on a wagon that carries any gear at all.

```ron
rows: 12,
weights: (gear: 6, routines: 2, materials: 2),
bonus_share: 35,      // of ~9 gear rows, 4 are standout
```

`0` — or leaving the field out, which is what every file written before it
existed does — gives you a shelf of plain copies, exactly as before.

How *good* a standout row is stays in `crates/engine/src/tuning.rs`
(`CARAVAN_BONUS_RARITY_CHANCE`, `CARAVAN_BONUS_QUALITY_FLOOR`), for the
reason perk costs are data while perk effects are not: what a trader carries
is content, and how hard the game is, is not.

**Portal Fragments can never appear**, on any shelf, at any sector, however
the weights are set. Breaching is earned by fighting and descending; a census
over this directory holds that shut, the same way the contracts census keeps
`Reward::PortalFragments` off a contract.

## Choosing a `color`

Censused when this directory was added: `assets/structures/` uses `Blue`,
`Brown`, `Cyan`, `Gray`, `Green`, `Magenta`, `Orange`, `White` and `Yellow`,
and the `Glyph` writers in `crates/engine/src` (nests, the anchor, surface
links, zone portals) use `Blue`, `Cyan`, `Gray`, `Green`, `Magenta`,
`Orange`, `Red`, `White` and `Yellow`. `DarkGreen` is claimed by neither, so
both shipped traders wear it and a caravan standing on the base map cannot be
mistaken for a fixture. Wild programs are recoloured by difficulty on the map
and so never collide with it either.
