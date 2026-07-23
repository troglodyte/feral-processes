# Custom structures (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all structure files
    name: "Display Name",
    glyph: '#',                   // single character shown on the map
    color: Magenta,                // one of: White, Gray, Green, DarkGreen, Red,
                                    //         Yellow, Blue, Magenta, Cyan, Brown,
                                    //         Orange
    build_cost: [("core_fragment", 3)],  // list of (item id, quantity) pairs
    // build_cost above, and every other item reference below (work.produces,
    // passive_process.consumes/produces, teleport_cost, trade.buy), all take
    // any item id from assets/items/*.ron — see assets/items/README.md for
    // the schema and the full set.

    // Omit (`None`) for a purely decorative/utility structure. Set `Some(...)`
    // to make it assignable to a tamed creature via the cronjob menu — it'll
    // produce one unit of `produces` every `ticks_per_unit` ticks. `capacity`
    // (optional, defaults to 5) caps how many units the node can hold before
    // it's mined down to empty; once empty it immediately refills to
    // `capacity` and the assigned creature keeps working — a worked node is
    // an infinite, bursty resource, never a one-time deposit.
    // `level` (optional, defaults to `None`) makes each completed cycle a
    // gamble instead of a guaranteed yield: with it set, there's only a
    // level-based percentage chance the cycle actually pays out (a level-1
    // node succeeds about half the time), and a miss still costs the full
    // cycle. Higher levels succeed more reliably. Leave it out entirely for
    // a node that always yields on completion, same as before this field
    // existed.
    work: Some((produces: "core_fragment", ticks_per_unit: 5, capacity: 5, level: Some(1))),

    // Optional; can be left out entirely (defaults to no passive processing).
    // If set, the structure automatically converts one `consumes` into one
    // `produces` every `ticks_per_unit` ticks whenever the player is standing
    // within `radius` tiles of it — no assigned worker needed, unlike `work`.
    passive_process: Some((
        consumes: "core_fragment",
        produces: "power_cell",
        ticks_per_unit: 15,
        radius: 2,
    )),

    // Optional; can be left out entirely (defaults to no symlink). If set,
    // this structure is a symlink target: the player can "use symlink" (`u`
    // in the TUI) to instantly teleport to it from anywhere on the map,
    // paying the listed item cost.
    teleport_cost: Some([("power_cell", 4)]),

    // Optional; can be left out entirely (defaults to false). If true,
    // walking onto this structure breaches the player into the next zone
    // sector instead of blocking movement — see `Game::enter_next_zone`.
    // Wild programs in the new zone spawn with stats doubled per zone
    // level, and there's no portal back down. `build_cost` above is
    // treated as a *per-zone-level* rate for a zone-portal structure: the
    // amount actually charged when deploying it is each quantity
    // multiplied by the current zone level (so building the portal out of
    // zone 2 costs twice as much as building it out of zone 1).
    zone_portal: true,

    // Optional; can be left out entirely (defaults to no trading). If set,
    // this structure is a trading post: the player can "trade" (`t` in the
    // TUI) with it to sell any inventory item (except Core Fragment) for
    // `sell_rate` Core Fragments per unit, or buy any item listed in `buy`
    // for its Core Fragment cost.
    trade: Some((
        sell_rate: 1,
        buy: [("ice_breaker", 4), ("power_cell", 3)],
    )),

    // Optional; can be left out entirely (defaults to 30). How much damage
    // this structure can take from raids (see `Game::raid_check`) before
    // being destroyed. An assigned cronjob worker/guard fights a raid off,
    // reducing the damage by its Defense stat; an unassigned structure
    // takes the raid's full damage (less any raid_defense below).
    // Damaged structures slowly regenerate over time regardless.
    durability: 30,

    // Optional; can be left out entirely (defaults to 0). Flat raid-damage
    // reduction this structure contributes to *every* raid, against *any*
    // deployed structure, for as long as it's standing — not just itself,
    // and it stacks additively across every deployed structure that sets
    // this (e.g. several Shields). Applied before an assigned worker/guard's
    // own Defense-based mitigation, so the two stack. This is how the
    // Shield structure works: `raid_defense: 4` with no `work` recipe.
    raid_defense: 4,

    // Optional; can be left out entirely (defaults to 0). How much this
    // structure raises the player's inventory capacity while it's deployed.
    // Capacity is `30 + the sum of this across every deployed structure`,
    // so several of them stack. This is how the Data Cache works:
    // `inventory_bonus: 10` with no `work` recipe. Research Data is exempt
    // from the capacity system entirely and has its own separate cap.
    inventory_bonus: 10,

    // Optional; can be left out entirely (defaults to no rest capability).
    // If set, `Game::rest` (recharge/overnight rest) is only allowed while
    // the player stands within `radius` tiles of this structure — resting
    // has no other way to happen. This is how the Recharger Node works:
    // `enables_rest: Some((radius: 2))`.
    enables_rest: Some((radius: 2)),

    // Optional; can be left out entirely (defaults to a permanent
    // structure). If set, this structure automatically collapses once
    // `max_ticks` ordinary game-clock ticks have passed since it was
    // deployed — no refund, it just disappears. Ticks spent inside a
    // `Game::rest` cycle don't count toward this, so a structure that also
    // sets `enables_rest` isn't worn down any faster by actually being
    // used to rest than by sitting there idle.
    temporary: Some((max_ticks: 20)),
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the structure for readability, e.g. `data_cache.ron`.

## Research gating

A structure named in some research node's `unlocks_structures` can't be
built until that node is researched — see `assets/research/README.md`. A
structure named by **no** research file is buildable from turn one, which is
how the Home, Mining Node, Research Node, Recharger Node and Zone Portal
stay available at the start, and why a structure mod that ships no research
file keeps working unchanged.

The Research Node itself (`research_node.ron`) is the source of Research
Data: assign a tamed program to it via the cronjob menu, same as a Mining
Node.
