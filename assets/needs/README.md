# Needs (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts — no recompiling required. A malformed
file is skipped with a warning logged in-game rather than crashing startup, so
a broken def costs the game that one need and nothing else.

**This directory may be deleted.** An empty catalogue is valid and inert:
nothing is seeded, nothing drains, no program ever leaves a post and every
program's need strain reads zero — exactly the pre-needs game. That is the
same supported way to play that deleting `assets/memories/` or
`assets/environment/` is.

## What a need is

A program on base staff carries a **reserve** for each need, running
`0.0`..`100.0` and seeded full. It falls every tick, faster while the program
is working. When one falls below its `critical` the program leaves its post
and walks to a structure that services it; when that reserve reaches
`content` it goes back on shift. A drained reserve drags on the program's
work.

Party programs and the wielded program are not on shift and do not drain.

Each file is one need:

```ron
(
    id: "coherence",
    name: "Coherence",
    blurb: "A process that never yields comes apart at the edges.",
    servicing: "Defragmenting",
    drain_per_tick: 0.02,
    working_multiplier: 2.0,
    critical: 20.0,
    content: 60.0,
    morale_weight: -4.0,
)
```

| Field | Meaning |
|---|---|
| `id` | Unique across the directory. It is what a save records, so renaming one drops whatever a program had banked under the old name — which then seeds full again. |
| `name` | What the manifest row leads with. |
| `blurb` | One line of flavour under it, in the player's vocabulary. |
| `servicing` | The player's verb for the errand: what the program is *doing* while off shift servicing this need. It is what the manifest row and the examine line read. |
| `drain_per_tick` | How much the reserve falls per tick with the program idle. |
| `working_multiplier` | Multiplies `drain_per_tick` while the program holds a job. |
| `critical` | Below this the program leaves its post. |
| `content` | At this the program is done and goes back on shift. Must be above `critical`: the gap is the hysteresis, and without it a program flickers on and off its post every tick at the boundary. |
| `morale_weight` | Signed. The contribution at an **empty** reserve, scaled linearly to nothing at `content` — so a satisfied need is worth zero rather than worth a little. Negative is a drag. |

All nine are required. Any field added in a later version will carry a
default, so a file written today keeps parsing untouched — but none of these
nine may be omitted.

Two files claiming the same `id` is not an error; the alphabetically last one
wins, which is deliberate (a mod's `zz_coherence.ron` overrides the shipped
def without deleting it).

## Servicing it

A need is refilled by a **structure**, not by an item or a verb. A structure
declares what it services in its own `.ron` file:

```ron
services: [
    (need: "coherence", per_tick: 0.5, radius: 0),
],
```

See `assets/structures/README.md` for that half of the schema. A need with no
structure servicing it anywhere in the base is a need a program can only run
down: it says so once in the log and holds a grudge about the corner it was
standing in. A shipped need without a shipped amenity fails the build.

## The magnitudes

`drain_per_tick`, `working_multiplier` and the two thresholds are content and
live here. How far a drained reserve is allowed to drag on a program's work is
a difficulty decision and lives in `crates/engine/src/tuning.rs` as
`NEED_STRAIN_MAX_SHIFT` — the cap applies to every need in the game at once,
while `morale_weight` says how much *this* need matters relative to the
others.
