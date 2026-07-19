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
                                   //         Yellow, Blue, Magenta, Cyan, Brown,
                                   //         Orange
                                   // Only shown as-is for a tamed/companion
                                   // program. A *hostile* one is recolored on
                                   // the map by difficulty relative to the
                                   // player's current power (see
                                   // `difficulty_color` in lib.rs) — Green,
                                   // Yellow, Orange, or Red, or Magenta if
                                   // `is_boss` — so this field only matters
                                   // once it's compiled.
    base_hp: 60,
    base_atk: 6,
    base_def: 3,
    taming_difficulty: 0.4,       // 0.0 (trivial) .. 1.0 (very hard) to compile/tame
    habitats: [OpenGrid, Mainframe],
    // Biome options: DataVoid, StaticField, NullSector, Mainframe, OpenGrid, BlackIce
    // (DataVoid and BlackIce are unwalkable barrier terrain — don't list them as a habitat)
    moves: [
        (name: "Move Name", power: 8),
        (name: "Other Move", power: 5),

        // Optional per-move; omit `effect` entirely for a plain damage-only
        // move. If set, landing this move has a `chance` (0.0-1.0) to also
        // inflict a status condition on the target for `duration` battle
        // rounds, on top of its direct damage. A combatant can only carry one
        // status condition at a time — a fresh one overwrites whatever was
        // active. `kind: Bleed` deals `power` extra damage at the end of
        // every round it's active; `kind: Stun` causes the afflicted side to
        // lose their next action instead (`power` is required by the schema
        // but unused for Stun — just set it to 0).
        (name: "Corrupted Move", power: 6, effect: Some((
            kind: Bleed,       // or `Stun`
            chance: 0.4,
            duration: 3,
            power: 3,
        ))),
    ],
    work_resource: Some(CoreFragment),  // or `None` if it shouldn't be assignable to a cronjob
    // Item options: CoreFragment, PowerCell, IceBreaker, PortalFragment,
    //               OverclockCore, MonofilamentWhip (Weapon),
    //               FirewallPlating, AblativePlating (Armor),
    //               NeuralAmplifier, CortexHack (Module)

    // Optional; omit entirely for no chance of a gear drop. If set, defeating
    // or decompiling this species has a chance (0.0-1.0) to additionally
    // drop one piece of equipment, independent of `work_resource`.
    equipment_drop: Some((FirewallPlating, 0.3)),

    // Optional; can be left out entirely (defaults to false). If true, this
    // species is a boss: it's excluded from the normal per-tile habitat spawn
    // roll and spawns in its place only rarely (see `BOSS_SPAWN_CHANCE` in
    // the engine), rendered bold on the map and tagged "[BOSS]" in the
    // inspect/battle screens. Defeating one guarantees a cache of 3-6 Portal
    // Fragments instead of the flat drop chance every other species rolls.
    // There's no separate engine-side stat multiplier for a boss — make
    // `base_hp`/`base_atk`/`base_def` tough here directly (a boss's stats
    // still double per zone level like any other species, on top of this).
    is_boss: true,

    // Optional; can be left out entirely (defaults to `None`). A tamed
    // program no longer attacks directly when commanded in battle — it
    // grants the player a buff instead. With no `special_ability` set, that's
    // a generic rally (a temporary ATK boost). Setting this gives a tamed
    // member of this species its own unique action instead, triggered the
    // same way (commanding it in battle):
    //   Rally(power: 4, duration: 3)                   — boosts player ATK
    //   Shield(power: 4, duration: 3)                   — boosts player DEF
    //   Heal(power: 8)                                  — heals the player now
    //   Debuff(kind: Bleed, power: 3, duration: 3)      — afflicts the wild
    //                                                      program (kind is
    //                                                      `Bleed` or `Stun`,
    //                                                      same as a move's
    //                                                      `effect`)
    special_ability: Some(Heal(power: 8)),
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the species for readability, e.g. `wraith.ron`.
