# Implants (mods)

Edit or add a `.ron` file in this directory and it is picked up the next time
a game session starts. A malformed file is skipped with a warning logged
in-game; two files claiming one id log a warning and the first (by file name)
wins. Deleting the directory is not an error: nothing can be installed.

An implant is something the player builds into their own avatar. It is always
on, drains Power while installed, and costs Neural Load against a cap that
grows with level. Going past the cap is allowed; while over it, each battle
start risks a rejection status. The Load cap, the upkeep per Load, the
rejection odds and the removal price are `tuning.rs` constants, not data.

## One implant is two files

The implant itself lives here. What the player crafts and carries is an item in
`assets/items/` whose `implant` field names this id. Every implant needs exactly
one such item, and every item naming an implant needs the implant: a census in
`tests/assets.rs` holds both.

## Schema

```ron
(
    id: "ripper_fibers",
    name: "Ripper Fibers",
    description: "One line on what it does.",   // optional
    load: 2,                                    // Neural Load, required
    stats: (atk: 3),                            // optional, every field defaults to 0
    hooks: [],                                  // optional, see below
    signature: None,                            // optional
    downside: None,                             // optional
)
```

### `stats`

Absolute deltas, negative allowed: `max_hp`, `atk`, `mitigation`, `max_power`,
`crit`, `status_resist`, `decompiler`, `accuracy`, `evasion`. `accuracy` and
`evasion` are read live in combat, like gear's.

### `hooks`

A closed list; a file combines them freely. Percentages are whole points.

| Hook | Effect |
| --- | --- |
| `CaptureOdds(pct)` | added to the decompile capture chance |
| `DropBoost(pct)` | added to the equipment drop chance, **in the Stack only** |
| `RoutineSlots(n)` | extra player routine slots, past the slot cap |
| `TraceDamp(pct)` | Trace rises this much more slowly |
| `XpBoost(pct)` | added to the XP the player earns |

### `signature`

One named behaviour the hooks cannot express. `DeadMansSwitch`: once per
battle, a lethal hit leaves the player at 1 HP and costs Power.

### `downside`

Optional, one of:

- `TraceRise(pct)`: Trace rises this much faster.
- `BattleStartStatus("status_id", chance)`: at battle start, arms the status
  with that 0.0-1.0 chance. The status must exist in `assets/statuses/`.

A file with a non-finite stat or a chance outside 0.0-1.0 is skipped.

## An id with no definition

A save may name an implant whose file has since been removed. The id is kept so
it can still be taken out, but it adds no Load, no upkeep and no effect.

## Shipped

| id | load | benefit | downside |
| --- | --- | --- | --- |
| `ripper_fibers` | 2 | +3 attack | none |
| `dermal_lattice` | 3 | +12 max HP, +4 mitigation | -3 evasion |
| `ghost_handshake` | 2 | +10% capture odds | Trace rises 25% faster |
| `black_ledger` | 2 | +25% equipment drops in the Stack | Trace rises 25% faster |
| `overclock_spine` | 4 | +1 routine slot | 20% to start a battle throttled |
| `dead_mans_switch` | 3 | survive one lethal hit per battle | none |
