# Statuses (mods)

Edit or add a `.ron` file in this directory and it is picked up the next time
a game session starts. A malformed file is skipped with a warning logged
in-game; two files claiming one id log a warning and the first (by file name)
wins. Deleting the directory is not an error: no status can be armed.

One file per status:

```ron
(
    id: "bleed",
    name: "Bleeding",          // party panel label
    tag: "BLD",                // battle map tag, 4 characters at most
    stacking: Refresh,         // or Stack(max: 5)
    behaviours: [DamagePerRound],
    inflict: "{target} starts leaking!",
    tick: "{target} leaks {n} Integrity.",   // optional, default ""
    expire: "{target}'s leak is plugged.",
)
```

`{target}` is the only placeholder in `inflict` and `expire`; `tick` also
takes `{n}`.

## Behaviours

A closed list; a status composes them, it cannot add new ones. Each acts per
stack.

| Behaviour | Effect |
| --- | --- |
| `DamagePerRound` | power x stacks damage at the end of the round |
| `SkipTurn` | the body loses its action |
| `EvasionCut(percent)` | evasion cut by percent x stacks |
| `AtkPercent(percent)` | attack scaled by percent x stacks |
| `MitigationPercent(percent)` | mitigation scaled by percent x stacks |
| `HealBlock` | heals restore nothing |

## Stacking

`Refresh` re-arming restarts the status. `Stack(max: N)` adds a stack up to
`N`.
