# What a lair costs a party that is on its zone's curve

## The claim

Linear scaling (`0.8.1`) made every lair depth reachable *for the run it was
measured on* — zone 1, level 38, five companions, which cleared depths 1-6 at
100%. That run was massively over-levelled for its zone: `balance_sim` says
zone 1 wants level 1. The open question was what a lair costs a party that is
**on** its curve rather than 37 levels past it.

Measured at zone 3, where the curve wants level 24, with a party of three
level-12 Scrappers, the player in `plasma_router`/`bastion_lattice`/
`singularity_matrix` and each companion in `arc_lance`/`hardened_shell`:

| player level | depth 1 | depth 2 | depth 3 |
|---|---|---|---|
| 20 | 80% / 64% | 15% / 15% | 0% |
| **24** *(on curve)* | **95% / 87%** | **15% / 15%** | **0%** |
| 30 | 100% / 96% | 45% / 44% | 0% |
| 36 | 100% / 98% | 70% / 61% | 0% |
| 50 | — | — | 0% |
| 70 | — | — | 75% / 52% |
| 90 | — | — | 100% / 98% |

Win rate / mean player HP left, 20 reps per cell at `seed: 900`.

**The level a lair demands roughly doubles per frame at fixed zone**: about
24 at depth 1, 33 at depth 2, 65 at depth 3 — against a zone that itself
wants 24. Depth 3 is not a wall (level 70 clears it 75% of the time), so this
is a steep curve rather than the unreachable band that geometric scaling
produced. But it is steep enough that an on-curve party clears the shallowest
lair and bounces off the next one down.

**The other half of the cliff.** The same on-curve party takes **zero**
damage from the largest group zone 3 can field (`full-group.ron`, 4 rootkit,
100% win at 99.9% HP left over 200 reps). Surface content at zone 3 is a
victory lap and the second frame of a stack is a wall. That is the finding:
not that either number is wrong on its own, but that they are the same party
on the same afternoon.

**It bears on the Portal Fragment softlock.** `frames_for` sets a stack's
lair depth from the link's distance to the spawn point, so a player can be
handed a deep stack as their only remaining lair, and a lair guardian is the
game's only source of the breaching currency. Depth 3 at zone 3 wants roughly
triple the zone's own level.

## How to reproduce it

```sh
cargo build --release --bin arena
cargo run --release --bin arena -- dev-arenas/lair-on-curve.ron
```

Vary `player: Fresh(level: N, zone: 3)` and `Lair(biome: Mainframe, depth: N)`
in the scenario for the sweep. 20 reps per cell is enough to see the shape
and **not** enough to set a target from — see the sampling note below.

## The sampling trap this run walked into

The lair fight at depth 2 read **15%** over 20 reps at `seed: 900`, **50%**
over 20 reps at `seed: 1337`, and **28.5%** over 200 reps at either. The
20-rep readings are both wrong by more than the gap anyone would be tuning
against.

This had a live consequence: `dev-tuning/objective.ron` ran at `seeds: 20`,
so the tuner scored this fight at 50% against a target of 55% and would have
seen itself as nearly converged while sitting 26 points off. It is now 200.

The reason it went unnoticed for so long is worth keeping: the other two
targets sit at 100% win, and a saturated fight has almost no variance to
sample. The rows anyone would have sanity-checked were the rows that could
not show the problem.

## What this run was blind to

- **No companion Specials.** The headless arena plays All-Attack every
  round, so every figure here is a floor on what a real party outputs.
  Nothing in this sweep was played.
- **One biome.** `Mainframe` only, so it says nothing about which species
  guards a lair elsewhere. `pick_lair_species` falls back to the toughest
  ordinary program where a biome fields no boss, and that path is untested
  here.
- **One party shape.** Three Scrappers, one gear loadout. Party size is a
  lever this did not pull, and the `0.8.1` run that cleared depths 1-6
  fielded five companions.
- **Trace is 0 throughout** — the party is placed at depth, not walked
  there, so nothing here includes the group-size multiplier a real descent
  accumulates.
