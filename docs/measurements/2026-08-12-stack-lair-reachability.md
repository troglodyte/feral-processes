# The Stack lair guardian was unreachable, and the curve was why

## The claim

A stack's lair guardian is the only boss the Stack fields and
`STACK_BOSS_PORTAL_FRAGMENT_DROP` is the game's only source of the breaching
currency, so it is the one fight a run cannot walk away from. It was
unwinnable past a depth that fell with the zone, and no amount of levelling
or gear fixed it — because enemy stats scaled geometrically (`x2` per zone,
`x1.35` per frame) against a player whose offence scales linearly
(`ATK_PER_LEVEL` is 1, gear adds flat points). Under
`battle::compute_damage`'s subtractive rule that does not read as difficulty:
once the guardian's DEF passes the party's ATK every swing lands on
`MIN_DAMAGE` and the fight stops responding to the player's stats at all.

Measured on a real stuck save — zone 1, **level 38**, five companions, and
`balance_sim` says zone 1 needs level 1 — depth 5 won **7.5%** of 40 reps and
depth 6 won **0%**. A zone-3 depth-5 lair was unwinnable at **level 90** in
the best gear the game ships.

Making the three curves linear (`ZONE_STAT_STEP`, `STACK_DEPTH_STAT_STEP`,
`GEAR_LEVEL_STEP`) takes every one of those to a finite, fundable level.
The same save now clears **every depth 1-6 at 100%**, and a zone-2 depth-5
lair went from needing level ~120 to needing level ~65.

The shape is the finding, not the numbers: geometric difficulty against
linear power always has a zone past which no reachable level clears it.
Linear against linear does not.

## How to reproduce it

The stuck run is committed as a template, so this needs no save of your own:

```sh
cargo run --bin arena -- dev-arenas/deep-lair.ron          # depth 5, 40 reps
```

`Encounter::Lair` is what makes this measurable at all — `Encounter::Stack`
rolls `stack_encounter_pack`, which passes `allow_boss: false`, so before it
existed the guardian could not be staged. Vary `depth` in the scenario for
the sweep; vary `player` for the on-curve rows:

```ron
player: Template("deep-lair"),                     // the stuck run
player: Fresh(level: 65, zone: 2),                 // an on-curve row
equip: [(item: "plasma_router"), (item: "ablative_plating"), (item: "oracle_core")],
party: [(species: "virus", level: 12), (species: "virus", level: 12), (species: "rootkit", level: 12)],
```

All rows are 40 reps at `seed: 900` (template) or `seed: 700` (`Fresh`).
Trace is 0 throughout — the party is placed at depth, not walked there.

The "before" column was produced by reverting the three formulae to their
geometric forms in `resources.rs`, `items.rs` and `game/stack.rs` and
rebuilding; there is no flag for it.

## The numbers

**The stuck save (zone 1, level 38, five companions), by depth.** New.

| depth | before — win rate | before — rounds | after — win rate | after — rounds |
|---|---|---|---|---|
| 1 | 100% | 2.0 | 100% | 2.0 |
| 2 | 100% | 3.3 | 100% | 3.3 |
| 3 | 100% | 8.5 | 100% | 7.0 |
| 4 | 100% | 24.6 | 100% | 12.2 |
| 5 | **7.5%** | 47.0 | **100%** | 21.3 |
| 6 | **0%** | 37.5 | **100%** | 22.6 |

Depths 1 and 2 are identical by construction: both curves are x1 and x1.35
there, so agreement is a correctness check on the change rather than a
result.

**Level needed to clear a depth-5 lair, by zone.** New. 70% win rate or
better, searched coarsely (50, 65, 80, 95, 110, 130).

| zone | before | after |
|---|---|---|
| 2 | ~120 | ~65 |
| 3 | unreachable at 90 with best gear | ~110 |
| 4 | unreachable | >130 |

**`balance_sim`'s own level curve, for context.** This reproduces what
`balance_sim` asserts on every `cargo test` and is repeated here only to
show the shape either side of the change — the live numbers are in
`MAX_GRIND_ONLY_ZONE_SWEPT`'s doc comment, which is the copy that cannot
drift.

| zones | 1 | 2 | 3 | 4 | 5 | 6 | 7 | 8 | 9 | 10 |
|---|---|---|---|---|---|---|---|---|---|---|
| before (geometric) | 1 | 15 | 30 | 63 | 131 | *unreachable by level 200* | | | | |
| after (linear) | 1 | 15 | 24 | 32 | 47 | 61 | 76 | 90 | 106 | 121 |

Zone 2 is 15 in both, for the same reason depths 1-2 agree above.

Also worth recording, because it cost time to establish: the doc comment
this replaced claimed the shipped curve was `1, 8, 29, 61, 138`. Measured on
the unchanged build it was `1, 15, 30, 63, 131`. The documented figure had
drifted and zone 2 was nearly twice what it said.

## What it does not say

- **The arena bin plays All-Attack and fires no companion Specials**
  (`dev-arenas/README.md` states this). Every win rate here is a *floor* on
  what the party can do. The gap is widest for the stuck save, whose lead
  companion carries `fork_bomb`.
- **Trace is 0 in every row.** In play a party arrives at the lair having
  looted its way down, and `TRACE_STAT_MULT` reaches 1.45. A Hunted party
  meets a guardian ~45% harder than anything measured here.
- **No attrition.** The party is placed at depth at full HP; a real descent
  spends resources on corridor ambushes first.
- **One save, one biome (Mainframe), one party shape.** Both shipped bosses
  live on all four Stack biomes, so the biome choice moves which of the two
  is drawn but not the roster it is drawn from.
- **It says nothing about whether a *boss* is proportionate to its zone.**
  A zone-2 lair wants level ~65 against `balance_sim`'s level ~15 for that
  zone's ordinary content. That gap is a property of boss base stats in
  `assets/species/`, not of the scaling curve, and this change does not
  address it. What changed is that the number is now finite.
- **`balance_sim` still has no Stack term at all.** It sweeps zones with
  surface group sizes and models no depth, no lair and no abilities. That is
  why this file exists rather than a test.

## What it means for the other instruments

Asked after the fact, and worth recording because the answer was not
obvious in either direction.

- **The trained enemy policy is unaffected in kind and affected in
  degree.** Every scale-sensitive feature is a ratio, so the weights are
  scale-invariant and `assets/policies/enemy_battle.ron` did not need
  regenerating. But the geometric curves used to drive those ratios into
  their clamps in deep fights, flattening the gradient exactly where the
  policy mattered; they now vary. Details and the consequences for
  `dev-logs/policy-sweep/` are in `assets/policies/README.md`.
- **The roster tuner's baseline moved and its targets did not.** Re-measured
  in `dev-tuning/NOTES.md`. The row that matters is `stack-depth-5`, which
  went 0% → 2% against a 55% target: this change fixed the *damage floor*
  half of that fight and not the *32-opponents-against-4* half, which is
  group size and lives in `tuning.rs` where the tuner cannot reach it.
- **Nothing in `balance_sim` needs new data** — it recomputes from the live
  constants on every `cargo test`, which is exactly why its curves belong
  there and not here.
