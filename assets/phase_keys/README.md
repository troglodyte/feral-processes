# Phase Keys

Edit or add a `.ron` file in this directory and it is picked up the next time
a game session starts. One key belongs to each zone 1 to 10: a Stack guardian
of that zone can drop it, and holding it grants a permanent benefit at once.

**The game always has exactly ten keys.** A malformed file is skipped with a
warning; a `zone` outside 1 to 10 is ignored with a warning; two files for one
zone keep the first by file name and warn. Any zone left empty gets a plain
"Phase Key N" with no effect (and a warning), so a mod can change what a key
is but can never make the story uncompletable. Deleting the directory is not
an error. The count, the drop rule, the portal gate and the exit requirement
are code (`tuning.rs`), not data.

## Schema

```ron
(
    zone: 3,                                  // 1 to 10, required
    name: "Seam Lens",                        // required
    flavour: "One line of story.",            // optional
    effect: (                                 // optional, every part optional
        stat_pct: (),                         // whole percent of a final stat
        stats: (accuracy: 5, crit: 0.03),     // flat points
        hooks: [],                            // ImplantHook list
        signature: None,                      // ImplantSignature
    ),
)
```

### `stat_pct`

Whole-percent bonuses on the magnitude stats `max_hp`, `atk`, `mitigation`,
`max_power` and `decompiler`. Applied to the player's stat after level,
attributes, perks and implants (worn gear is added on top, unscaled). Percents
from every held key sum before they apply, so 5% and 10% make 15%. A positive
percent adds at least 1. Negative percents are refused.

### `stats`

Flat deltas, the same fields as an implant's `stats` (`assets/implants/`):
use them for the chance stats `crit` (a fraction: 0.03 is 3%), `accuracy`,
`evasion` and `status_resist`.

### `hooks` and `signature`

Exactly the implant vocabulary (`CaptureOdds`, `DropBoost`, `RoutineSlots`,
`TraceDamp`, `XpBoost`; `DeadMansSwitch`). A signature that an installed
implant also has does not double: the Dead Man's Switch still fires once per
battle.

Keys cost no Neural Load and no Power upkeep. Their effects are derived from
which keys are held and are never baked into stored stats.
