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
                                    //         Yellow, Blue, Magenta, Cyan, Brown
    build_cost: [(CoreFragment, 3)],  // list of (item, quantity) pairs
    // Item options: CoreFragment, PowerCell, IceBreaker, OverclockCore,
    //               FirewallPlating, NeuralAmplifier

    // Omit (`None`) for a purely decorative/utility structure. Set `Some(...)`
    // to make it assignable to a tamed creature via the cronjob menu — it'll
    // produce one unit of `produces` every `ticks_per_unit` ticks.
    work: Some((produces: CoreFragment, ticks_per_unit: 5)),

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
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the structure for readability, e.g. `data_cache.ron`.
