# Custom species (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all species files
    name: "Display Name",
    glyph: 'x',                   // single character shown on the map
    color: Cyan,                  // one of: White, Gray, Green, DarkGreen, Red,
                                   //         Yellow, Blue, Magenta, Cyan, Brown
    base_hp: 20,
    base_atk: 6,
    base_def: 3,
    taming_difficulty: 0.4,       // 0.0 (trivial) .. 1.0 (very hard) to compile/tame
    habitats: [OpenGrid, Mainframe],
    // Biome options: DataVoid, StaticField, NullSector, Mainframe, OpenGrid, BlackIce
    // (DataVoid and BlackIce are unwalkable barrier terrain — don't list them as a habitat)
    moves: [
        (name: "Move Name", power: 8),
        (name: "Other Move", power: 5),
    ],
    work_resource: Some(CoreFragment),  // or `None` if it shouldn't be assignable to a cronjob
    // Item options: CoreFragment, PowerCell, IceBreaker, OverclockCore,
    //               FirewallPlating, NeuralAmplifier

    // Optional; omit entirely for no chance of a gear drop. If set, defeating
    // or decompiling this species has a chance (0.0-1.0) to additionally
    // drop one piece of equipment, independent of `work_resource`.
    equipment_drop: Some((FirewallPlating, 0.3)),
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the species for readability, e.g. `wraith.ron`.
