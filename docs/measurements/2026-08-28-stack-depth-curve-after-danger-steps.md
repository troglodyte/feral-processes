# The Stack's depth curve after the danger-steps sum, and the arena's stale ceiling

## The claim

**Three findings, two of which are new defects and one of which is an
instrument fault that has to be fixed before either can be tuned.**

1. **The ordinary Stack ambush is unwinnable from depth 3 down, not from
   depth 5.** At zone 3, against a party at the real zone-3 level cap, the
   shipped build wins **97.5%** at depth 2, **2.0%** at depth 3, **0.5%** at
   depth 4 and **0.0%** at depth 5 (200 reps each). The 2026-08-19 run
   recorded 100 / 100 / 78 / 0 for the same scenario family. What moved
   between them is the 2026-08-24 `danger_steps` sum: it added the zone step
   to the depth step, and `danger_steps` feeds the **group-size curve** as
   well as the species window that change was aimed at. The lair half of
   that change was measured at the time; the ordinary-ambush half was not.
2. **The missing bound is a ceiling on the whole pack.** `group_pack` caps
   members *per group* and groups *per fight*, so the product is what a
   fight may field — at zone 3 depth 5 that is 4 groups against a party of 4,
   rolling **33 bodies** on average. `stack_encounter_pack` fills it by
   construction: it takes one species pick and one full group roll **per
   group slot the curve allows**. The surface never does — a surface fight is
   whoever `gather_pack` found standing together, which measures **2.6
   bodies** at the same zone against the Stack's 16.2 at depth 2. One
   constant, a total-pack ceiling of 8, restores a monotone depth curve
   (100 / 93.5 / 67 / 39.5) touching nothing else.
3. **`arena_level_ceiling()` is stale, and it is why the shipped scenario
   looks harder than the game is.** It is `TALENT_START_LEVEL +
   KERNEL_RING_MAX * LEVELS_PER_RING` = **12**, an expression of the old
   ring-buys-levels rule that 0.13.44 replaced: a Kernel Ring now buys talent
   tiers, and the level ceiling is `zone_level_cap`, which is **23** at zone
   3. Every companion in every arena scenario has been clamped to 12 since
   that release. Raising the clamp to the zone cap moves the shipped
   `stack-depth-5.ron` family's depth-2 figure from 60% to 94% — so the
   instrument has been reporting a party half the level the game permits.

Finding 3 does **not** rescue the deep Stack: at the true cap, depths 3, 4
and 5 still win 2.0 / 0.5 / 0.0. Doubling party level buys depth 2 and
nothing below it, which reproduces the 2026-08-19 finding that this is not a
party-strength problem.

**A fourth, incidental:** the report's `companions down` column has read
`0.00` for every scenario since `2d780b0c` (2026-08-27) landed Forgiving
benching. `arena::watch::alive` is `hp > 0`, and a benched program sits at
**HP 1** carrying `Downed`, so a companion that died reads as alive. The
column is structurally zero, not observed zero, in every table below and in
anything measured since that commit.

## What shipped off it

All four findings were acted on the same day, on `fix/stack-pack-ceiling`:

- `tuning::MAX_PACK_BODIES = 8`, applied in `Game::group_pack` by trimming
  the largest group each pass. Bounds every fight in the game, surface
  included; the bodies it turns away stay standing and are met on the next
  bump. `docs/seams.md` carries the argument under "A fight is bounded by
  bodies, not just by groups".
- `arena::set_level` now stages a companion up to the **higher** of the
  scenario's zone cap and `arena_level_ceiling()` — keeping the zone-1
  property that figure was introduced for, and ending the upward clamp. The
  in-game arena screen's level dial follows the same rule.
- `dev-arenas/stack-depth-5.ron` authors `level: 23`, zone 3's own cap. Its
  numbers from before this date are not comparable to numbers after it.
- `arena::watch`'s `companions_downed` counts a benched program.

The tables below are the pre-fix build unless a row says otherwise, which is
what makes them the argument for the constant rather than a description of
the game as it stands. Re-measured after the change with the same scenarios
and 200 reps, the shipped curve is **100 / 93.5 / 67 / 39.5** across depths
2-5, reproducing the `cap 8` column exactly.

## How to reproduce it

One build for every table; arena numbers compare within one build only.
Four of the six levers need a temporary patch, because they are `pub const`s
and one is a bound the shipped code does not have at all. **Every patch was
reverted and `git diff --quiet crates/` confirmed clean before this file was
written; re-apply them to re-measure and confirm the same way afterwards.**

```rust
// crates/engine/src/game/spawning.rs, in `danger_steps`, replacing the
// `depth_step` binding. Slows the depth term without touching the zone one.
let frame_step: u32 = std::env::var("FERAL_FRAME_STEP")
    .ok().and_then(|v| v.parse().ok()).unwrap_or(GROUP_SIZE_STEP_FRAMES);
let depth_step = depth.saturating_sub(1) / frame_step;

// crates/engine/src/game/spawning.rs, in `habitat_pools`, just after `step`
// is bound. Isolates the species window from the group-size curve, which
// `danger_steps` otherwise moves together.
let step = match std::env::var("FERAL_BAND_SUB").ok().and_then(|v| v.parse::<u32>().ok()) {
    Some(sub) => step.saturating_sub(sub),
    None => step,
};

// crates/engine/src/game/combat.rs, at the end of `group_pack`, after the
// existing `max_groups` truncation. The ceiling the shipped code has nowhere.
if let Ok(total_cap) = std::env::var("FERAL_PACK_CAP") {
    let total_cap: usize = total_cap.parse().unwrap();
    loop {
        let total: usize = groups.iter().map(|g| g.members.len()).sum();
        if total <= total_cap { break }
        let Some(biggest) = groups.iter_mut().max_by_key(|g| g.members.len()) else { break };
        biggest.members.pop();
        groups.retain(|g| !g.members.is_empty());
    }
}

// crates/engine/src/arena/mod.rs, in `set_level`, replacing the companion
// arm's `Some(arena_level_ceiling())`.
Some(std::env::var("FERAL_ARENA_CEILING").ok().and_then(|v| v.parse().ok())
    .unwrap_or_else(arena_level_ceiling))

// crates/engine/src/stack.rs, in `depth_stat_multiplier`.
let step = std::env::var("FERAL_DEPTH_STEP").ok().and_then(|v| v.parse().ok())
    .unwrap_or(crate::tuning::STACK_DEPTH_STAT_STEP);
1.0 + step * (depth as f32 - 1.0)
```

`FERAL_LINEAR_K` (a sixth lever, replacing `GROUP_SIZE_DISTANCE_GROWTH.pow`
with `1 + k * steps` in `max_group_size`) was swept and is reported below.

Every sweep runs from `dev-arenas/stack-depth-5.ron` with `depth:` edited,
`NullSector`, `seed: 41`, 50 reps unless a table says 200. Body counts are
the mean of `composition` across the report's reps, which the bin does not
print — read it out of `arena-report.ron`.

```sh
cargo build --bin arena
for d in 2 3 4 5; do
  sed "s/depth: 5/depth: $d/" dev-arenas/stack-depth-5.ron > /tmp/pack-d$d.ron
  ./target/debug/arena /tmp/pack-d$d.ron
done

# The instrument control. The scenario authored `level: 12` when this was
# run, and the control edits it to 23 — which does nothing at all without
# the ceiling patch. The file ships at 23 now, so reproducing the *pre-fix*
# tables means editing it back down.
FERAL_ARENA_CEILING=23 ./target/debug/arena /tmp/cap-d5.ron

# The three terms, isolated.
FERAL_PACK_CAP=8   ./target/debug/arena /tmp/pack-d5.ron   # volume
FERAL_BAND_SUB=3   ./target/debug/arena /tmp/pack-d5.ron   # species window
FERAL_DEPTH_STEP=0.05 ./target/debug/arena /tmp/pack-d5.ron # per-body stats

# The surface, for the asymmetry.
sed 's/Stack(biome: NullSector, depth: 5)/Field(biome: NullSector)/' \
  dev-arenas/stack-depth-5.ron > /tmp/field-z3.ron
./target/debug/arena /tmp/field-z3.ron
```

## The numbers

### The cliff, on shipped constants

50 reps. Companions clamped to 12 by `arena_level_ceiling()` whatever the
file says. **New** — the same scenario family measured 100 / 100 / 78 / 0 on
2026-08-19, before the `danger_steps` sum.

| Depth | Bodies (mean) | Win rate | Rounds (mean/median) | Player HP left |
|---|---:|---:|---:|---:|
| 2 | 16.2 | 60.0% | 27.7 / 25 | 46% |
| 3 | 29.7 | 0.0% | 11.2 / 10 | 0% |
| 4 | 35.6 | 0.0% | 8.4 / 8 | 0% |
| 5 | 33.3 | 0.0% | 7.5 / 7 | 0% |

Rounds *falling* as depth grows is the signature: depth 2 is a long fight
you usually win, depth 5 is over before the party acts.

### The same depths with the party at the real zone-3 cap

200 reps, `FERAL_ARENA_CEILING=23`. **New**, and the control that says this
is not a party-strength problem.

| Depth | Shipped | Total pack cap 8 |
|---|---:|---:|
| 2 | 97.5% (19.7 rounds) | 100% (9.5) |
| 3 | 2.0% (15.3) | 93.5% (20.5) |
| 4 | 0.5% (11.7) | 67.0% (26.3) |
| 5 | 0.0% (9.5) | 39.5% (30.7) |

### The Stack fills its ceiling; the surface never does

50 reps, shipped constants, same party. **New.** `Encounter::Field` against
`Encounter::Stack` at the same zone.

| Fight | Bodies | Win rate | Rounds |
|---|---:|---:|---:|
| Surface, zone 1 | 1.0 | 100% | 1.0 |
| Surface, zone 2 | 1.6 | 100% | 1.5 |
| Surface, zone 3 | 2.6 | 100% | 3.3 |
| Surface, zone 4 | 4.8 | 100% | 6.2 |
| Surface, zone 6 | 17.5 | 64% | 30.4 |
| **Stack, zone 3 depth 2** | **16.2** | 60% | 27.7 |

Zone 6's surface figure is measured with a zone-3 party and is quoted for
the body count, not as a verdict on zone 6.

### The three terms, isolated at depth 5

50 reps. **New**, and it answers the top open question left by
`2026-08-19-stack-depth-curve.md`: the species window is a **threshold, not
a gradient**.

| Lever | Bodies | Win rate | Rounds |
|---|---:|---:|---:|
| shipped | 33.3 | 0.0% | 7.5 |
| band −1 | 35.6 | 0.0% | 7.6 |
| band −2 | 35.6 | 0.0% | 7.6 |
| band −3 | 35.1 | 0.0% | 12.7 |
| depth stat step 0.05 (a 7x cut) | 33.3 | 0.0% | 14.4 |
| total pack cap 8 | 8.0 | 8.0% | 16.6 |
| total pack cap 8 **and** band −3 | 8.0 | **64.0%** | 23.8 |

Band −1 and −2 change nothing because the window still admits the tier-2
species; −3 drops below `TIER_ENTRY_STEPS * 2` and they leave the pool.
Neither volume nor band alone is enough at depth 5 and both together are —
the same shape as the 2026-08-08 two-fault diagnosis.

### Group-size curve shapes

50 reps, party clamped at 12. `FERAL_LINEAR_K` replaces the geometric
`2^steps` ceiling with `1 + k * steps`. **New.**

| Config | d2 | d3 | d4 | d5 |
|---|---:|---:|---:|---:|
| shipped (geometric) | 60% | 0% | 0% | 0% |
| linear k=1 | 100% | 24% | 4% | 10% |
| linear k=2 | 74% | 0% | 2% | 2% |
| linear k=3 | 38% | 0% | 0% | 0% |
| frame step 2 | 100% | 36% | 20% | 0% |
| frame step 3 | 100% | 100% | 20% | 10% |
| frame step 4 | 100% | 100% | 98% | 10% |
| frame step 3 + pack cap 8 | 100% | 100% | 90% | 56% |

Linear growth is not on its own a fix: even `k=1` fields 12.8 bodies at
depth 5 and wins 10%.

### What the candidate costs the shipped scenarios

50 reps, party clamped at 12, `frame step 3 + pack cap 8`. **New**, and the
reason that pair is *not* the recommendation.

| Scenario | Shipped | Candidate |
|---|---:|---:|
| `opening-fight` | 98% / 7.2 rounds | 98% / 7.2 |
| `full-group` | 100% / 6.9 | 100% / 6.9 |
| `lair-on-curve` | 100% / 8.8 | 100% / 7.1 |
| `deep-lair` | 100% / 7.4 | 100% / 6.3 |
| `measure-z3d1-ambush-oncurve` | 100% / 7.3 | 100% / 6.9 |
| `measure-z3d3-lair-oncurve` | 71% / 22.5 | **100% / 8.8** |
| `measure-z1d2-lair-fresh-mainframe` | 0% / 3.4 | 0% / 4.6 |
| `measure-z1d2-lair-developed-mainframe` | 98% / 7.9 | 99.7% / 5.7 |

`GROUP_SIZE_STEP_FRAMES` is one knob over both the ambush pack and the lair
escort, so slowing it erases the zone-3 depth-3 lair jump that 2026-08-24
deliberately introduced (71% → 100%, 22.5 rounds → 8.8). A total pack
ceiling does not: a lair fields 8.7 bodies at that depth and a cap of 8
barely bites.

## What it does not say

- **Nothing here is a playtest.** 50- and 200-rep staged fights with no
  companion Special ever firing — the bin plays the game's own All-Attack
  every round, so every win rate is a **floor** on what a real party
  produces.
- **The noise floor at 50 reps is wide.** Re-authoring a level the arena
  then clamps away — a pure RNG-stream shift with no mechanical difference —
  moved depth 2 from 60% to 72%. Treat differences under ~15 points at 50
  reps as nothing. The 200-rep table is the one to quote.
- **`companions down` is zero everywhere and means nothing.** See the fourth
  finding above; it has been structurally zero since 2026-08-27.
- **One biome, one seed base, one party shape.** `NullSector`, `seed: 41`,
  three identical Scrappers, player level 20 in zone 3. A mixed party, a
  different pool, or a player using items were all out of scope.
- **No Trace.** An arena session has no Trace, so every figure is the
  quiet-party case; a noisy party at depth 5 faces up to 1.45x more.
- **The pack-cap experiment is softer than a shipped fix would be**, exactly
  as the 2026-08-19 file recorded: it deletes the surplus, where
  `group_pack`'s existing over-ceiling behaviour leaves it standing to be met
  on the next bump. A capped pack in play is two fights, not one.
- **Nothing about how often depth 3+ is reached**, or about zones other than
  3. `frames_for` sets a stack's depth from its link's distance to the spawn
  point, so a player can be handed a 6-frame stack as their only lair.

## Open questions

1. **What is the target?** Still open, and now shipped against: 100 / 93.5 /
   67 / 39.5 across depths 2-5 for a party at the zone cap is *a* curve, not
   a designed one, and the same question was left open on 2026-08-19 and
   2026-08-24. It was settled here by picking the constant that makes the
   curve descend; whether ~40% at the bottom of a stack is *felt* as a
   gamble worth taking is a play question, not a bin question.
2. **Should the ceiling be a total, or should the Stack stop filling its
   ceiling?** A `MAX_PACK_BODIES` in `group_pack` bounds every fight in the
   game including the surface's, which is arguably where it belongs — the
   surface is currently bounded only by how many bodies happened to wander
   together. Narrowing it to `stack_encounter_pack` instead leaves the
   surface's 4-group product ungated at high zones, where the zone-6 row
   above already measures 17.5 bodies.
3. **Gate it.** `balance_sim` still has no Stack term, and its surface
   clearability projection uses **one** group rather than the four-group
   product, so neither this regression nor the zone-6 surface figure can fail
   a test. That is why the 2026-08-24 change shipped with half of its effect
   unmeasured, and it is the durable fix — the retune is the symptom.
4. ~~**Is `arena_level_ceiling()` worth keeping at all?**~~ Answered by
   doing it: it survives as the **floor** under the arena's ceiling, because
   zone 1's cap is below it and five shipped scenarios author `level: 12`.
   The ceiling is the higher of the two.
