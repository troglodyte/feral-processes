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
| `AccuracyCut(percent)` | accuracy cut by percent x stacks |
| `AtkPercent(percent)` | attack scaled by percent x stacks |
| `MitigationPercent(percent)` | mitigation scaled by percent x stacks |
| `HealBlock` | heals restore nothing |

## Stacking

`Refresh` re-arming keeps the larger of the remaining rounds and the power.
`Stack(max: N)` adds a stack up to `N`.

A freshly applied status skips its first end-of-round tick, and so does a
`Refresh` re-apply. A `Stack` re-dose does not, so the stacks already held keep
ticking.

## Shipped

| id | tag | behaviour | stacking |
| --- | --- | --- | --- |
| `bleed` | BLD | damage per round | Refresh |
| `stun` | STN | skip the next action | Refresh |
| `exposed` | EXP | evasion cut by half | Refresh |
| `poison` | PSN | damage per round | Stack(max: 5) |
| `throttled` | THR | attack down 25% | Refresh |
| `locked` | LCK | no healing | Refresh |

## Built-in statuses

Some statuses are defined by the engine, not by a file here, because a damage
type's side effect arms them: `jammed` (JAM, accuracy cut by 30%, from Noise
hits) and `corrupted` (COR, 2 Integrity lost a round for 3 rounds, from
Radiation hits; healing still works). They exist whatever this directory holds. A file here using a built-in
id is ignored and a warning is logged. Armour resisting the type by 50% or
more stops the effect from landing.

Every shipped status must be armed by some move, ability or fumble rung; a
census test fails on one nothing uses.
