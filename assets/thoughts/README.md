# Thoughts (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts. A malformed file is skipped with a
warning logged in-game rather than crashing startup.

**This directory may be deleted.** An empty catalogue is valid and inert: no
thought ever fires, every program's situational total is zero and Morale is
exactly the memory total it was before thoughts existed.

## What a thought is

A memory is something that happened; a thought is something that is true
*right now* about where a program is standing. It is worked out fresh every
tick, weighs on Morale while it holds and is gone the moment it stops. Active
thoughts show on the memories page as rows marked "now".

```ron
(
    trigger: BesideRival,
    name: "Beside a rival",
    blurb: "Hard to work with them standing right there.",
    intensity: -3.0,
)
```

| Field | Meaning |
|---|---|
| `trigger` | What makes the thought fire. One of the closed list below; all four fields are required. |
| `name` | What the row on the memories page leads with. |
| `blurb` | One line of flavour on the same row. |
| `intensity` | Signed. Positive lifts Morale, negative lowers it. Read through the program's disposition like a memory is. |

## Triggers

The list is closed: what fires a thought is code, and a mod can reword,
reweight or delete a thought but not invent a trigger.

| Trigger | Fires when |
|---|---|
| `BesideRival` | Another staff program stands on an adjacent tile (diagonals count) and this program's bond with it is Rival or Enemy. |
| `BesideFriend` | As above, and the bond is Friend or Close. |
| `Unpowered` | The program is posted to a machine the grid cannot power. |
| `MachineRunning` | The program is posted to a machine that is running and powered (a dark machine gives only `Unpowered`). |
| `NoAmenity` | The base has no amenity at all. |

Two files naming the same trigger: the first by file name wins and the second
is skipped with a warning.

## Caps

The situational total is clamped to plus or minus `tuning::SITUATION_MAX_TOTAL`
before it joins the memory total, so standing in a bad spot alone can never
start a program sulking, though it can tip one already soured by real
memories. Thoughts never change what a program thinks *of* another — bonds
read memories only.
