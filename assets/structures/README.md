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
    build_cost: [(CoreFragment, 3)],  // list of (item, quantity) pairs
    // Item options: CoreFragment, PowerCell, IceBreaker, PortalFragment,
    //               OverclockCore, MonofilamentWhip, FirewallPlating,
    //               AblativePlating, NeuralAmplifier, CortexHack

    // Omit (`None`) for a purely decorative/utility structure. Set `Some(...)`
    // to make it assignable to a tamed creature via the cronjob menu — it'll
    // produce one unit of `produces` every `ticks_per_unit` ticks. `capacity`
    // (optional, defaults to 5) caps how many units the node can hold before
    // it's mined down to empty; once empty it immediately refills to
    // `capacity` and the assigned creature keeps working — a worked node is
    // an infinite, bursty resource, never a one-time deposit.
    work: Some((produces: CoreFragment, ticks_per_unit: 5, capacity: 5)),

    // Optional; can be left out entirely (defaults to no passive processing).
    // If set, the structure automatically converts one `consumes` into one
    // `produces` every `ticks_per_unit` ticks whenever the player is standing
    // within `radius` tiles of it — no assigned worker needed, unlike `work`.
    passive_process: Some((
        consumes: CoreFragment,
        produces: PowerCell,
        ticks_per_unit: 15,
        radius: 2,
    )),

    // Optional; can be left out entirely (defaults to no symlink). If set,
    // this structure is a symlink target: the player can "use symlink" (`u`
    // in the TUI) to instantly teleport to it from anywhere on the map,
    // paying the listed item cost.
    teleport_cost: Some([(PowerCell, 4)]),

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
    // TUI) with it to sell any inventory item (except CoreFragment) for
    // `sell_rate` Core Fragments per unit, or buy any item listed in `buy`
    // for its Core Fragment cost.
    trade: Some((
        sell_rate: 1,
        buy: [(IceBreaker, 4), (PowerCell, 3)],
    )),

    // Optional; can be left out entirely (defaults to 30). How much damage
    // this structure can take from raids (see `Game::raid_check`) before
    // being destroyed. An assigned cronjob worker fights a raid off,
    // reducing the damage by its Defense stat; an unassigned structure
    // takes the raid's full damage. Damaged structures slowly regenerate
    // over time regardless.
    durability: 30,
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the structure for readability, e.g. `data_cache.ron`.
