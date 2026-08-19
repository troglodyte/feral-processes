# The Stack's depth curve, and where it stops being winnable

## The claim

**Depth 5 is unwinnable and depth 4 is a pyrrhic slog, for any party the game
can currently field.** With a level-20 player in zone 3 and three geared
level-12 companions — the strongest party the game can produce, every Kernel
Ring open — an ordinary wandering pack wins 100% at depth 2 and 3, 78% at depth
4 (45.8 rounds, 2.4 of 3 companions lost), and **0% at depth 5**. Stacks run to
`STACK_FRAMES_MAX = 6`, so a third of the Stack's depth range is off the map.

It is not a party-strength problem. Doubling party power (the level-6 ceiling to
the level-12 one) bought 2.6 extra rounds at depth 5 and zero wins, and the
*lair* fight at that depth — nine bodies rather than twenty-eight — loses 50 of
50 too.

It is not one knob either. **Three curves move per frame descended**, and they
multiply:

| Term | Per frame | At depth 5 |
|---|---|---|
| Per-body stats (`STACK_DEPTH_STAT_STEP`) | +0.35, linear | 2.40x, on top of the zone curve |
| Bodies in the fight (`GROUP_SIZE_DISTANCE_GROWTH`) | **x2, geometric** | 16/group x up to 4 groups = 64 allowed, 28 rolled |
| Species pool (`TIER_ENTRY_STEPS`/`TIER_WINDOW_STEPS`) | one band per two steps | apex-adjacent (`zero_day`, `rootkit`) against a level-12 party |

The middle row is the interesting one: **group size is the one geometric
difficulty curve left in the game**, against `CLAUDE.md`'s stated correctness
property that every difficulty curve is linear. It is also completely ungated —
`balance_sim` has no Stack term at all, and its surface clearability test
projects *one* group at `zone_group_cap` rather than the four-group total, so
nothing automated can fail when a depth becomes unwinnable. That is why
`0.8.1`'s half-fix left this standing.

Neither single knob rescues depth 5 on its own. Cutting the pack to 8 bodies at
the shipped stat step reaches 4%; cutting the stat step to 0.15 (a 38% cut)
reaches 38% but produces 97-round fights. Cutting both moderately —
8 bodies and a 0.20 step — reaches **62%**, and leaves the on-curve depth-2 lair
untouched at 3.1 rounds against 3.3.

## How to reproduce it

Two of the four sweeps need a temporary patch, because the levers are `pub
const`s. Both patches were reverted after the run; re-apply them to re-measure,
and **`git diff --quiet crates/` before believing any number** afterwards.

```rust
// crates/engine/src/game/combat.rs, at the end of `group_pack`, after the
// existing `max_groups` truncation. A total-pack ceiling, which the shipped
// code has nowhere.
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

// crates/engine/src/stack.rs, in `depth_stat_multiplier`.
let step = std::env::var("FERAL_DEPTH_STEP").ok().and_then(|v| v.parse().ok())
    .unwrap_or(crate::tuning::STACK_DEPTH_STAT_STEP);
1.0 + step * (depth as f32 - 1.0)
```

Every sweep is 50 reps from `seed: 41`, biome `NullSector`, and one build per
table — arena numbers compare within one build only.

```sh
# The cliff. No patch needed; shipped code.
for d in 2 3 4 5; do
  sed "s/depth: 5/depth: $d/" dev-arenas/stack-depth-5.ron > /tmp/pack-d$d.ron
  cargo run --bin arena -- /tmp/pack-d$d.ron
done

# Composition at equal depth: the lair instead of a wandering pack.
sed 's/Stack(biome: NullSector, depth: 5)/Lair(biome: NullSector, depth: 5)/' \
  dev-arenas/stack-depth-5.ron > /tmp/lair-d5.ron
cargo run --bin arena -- /tmp/lair-d5.ron

# Party strength: `developed-companion.ron`'s control trick, applied to depth 5.
sed 's/level: 12/level: 6/' dev-arenas/stack-depth-5.ron > /tmp/d5-ctl.ron
cargo run --bin arena -- /tmp/d5-ctl.ron

# The two levers, patches applied.
for step in 0.35 0.25 0.15 0.05; do
  FERAL_DEPTH_STEP=$step cargo run --bin arena -- dev-arenas/stack-depth-5.ron
done
for cap in 20 16 12 8; do
  FERAL_PACK_CAP=$cap cargo run --bin arena -- dev-arenas/stack-depth-5.ron
done

# The regression side: the depth step also moves the on-curve lair.
FERAL_DEPTH_STEP=0.25 cargo run --bin arena -- dev-arenas/lair-on-curve.ron
```

## The numbers

### The cliff, on shipped constants

Player level 20 zone 3, three `scrapper` companions at level 12 with `arc_lance`
and `hardened_shell`. Bodies and species are one rep's roll, quoted to show what
the pool does; they are not the ceiling.

| Depth | Stat mult | Bodies rolled | Species | Win rate | Rounds (mean/median) | Companions lost |
|---|---|---|---|---|---|---|
| 2 | 1.35x | 3 | glitch, sub_process | 100% | 2.2 / 2 | 0.00 |
| 3 | 1.70x | 5 | scrapper, sub_process, worm | 100% | 9.9 / 9 | 0.22 |
| 4 | 2.05x | 12 | scrapper, worm, sub_process | 78% | 45.8 / 39 | 2.40 |
| 5 | 2.40x | 28 | scrapper, zero_day, rootkit | **0%** | 7.4 / 6 | 2.42 |

Mean rounds peaking at depth 4 and collapsing at depth 5 is the signature of the
edge: depth 4 is a grind you usually survive, depth 5 kills you quickly.

### It is not party strength, and not the horde alone

| Fight at depth 5 | Win rate | Rounds |
|---|---|---|
| Party at level 6 (the pre-ring ceiling) | 0% | 4.8 / 4 |
| Party at level 12 (every ring open) | 0% | 7.4 / 6 |
| The **lair**: 1 overseer + 8 virus, same party | 0% | 18.8 / 13 |
| The lair at depth **2** (`lair-on-curve.ron`, player 24) | 100% | 3.3 / 3 |

### Lethality is the stat step

Shipped pack (28 bodies), depth 5:

| Step | Depth-5 mult | Win rate | Rounds (mean/median) |
|---|---|---|---|
| 0.35 (shipped) | 2.40x | 0% | 7.4 / 6 |
| 0.25 | 2.00x | 4% | 23.5 / 11 |
| 0.15 | 1.60x | 38% | 96.9 / 34 |
| 0.05 | 1.20x | 90% | 113.5 / 111 |

### Duration is the body count

Shipped stat step, depth 5:

| Total pack cap | Win rate | Rounds (mean/median) |
|---|---|---|
| none (shipped) | 0% | 7.4 / 6 |
| 20 | 0% | 8.4 / 7 |
| 16 | 0% | 9.5 / 8 |
| 12 | 0% | 12.5 / 11 |
| 8 | 4% | 18.3 / 12 |

### Together

| Cap | Step | Win rate | Rounds (mean/median) |
|---|---|---|---|
| 16 | 0.25 | 14% | 32.7 / 14 |
| 16 | 0.20 | 30% | 50.8 / 22 |
| 12 | 0.25 | 26% | 40.6 / 18 |
| 12 | 0.20 | 44% | 54.7 / 43 |
| 8 | 0.25 | 38% | 37.5 / 27 |
| 8 | 0.20 | **62%** | 39.8 / 39 |
| 6 | 0.25 | 58% | 32.7 / 30 |
| 6 | 0.35 | 12% | 18.3 / 15 |

Every winnable configuration is a 30-to-40-round fight. That is arithmetic, not
tuning: damage is subtractive against depth-scaled HP, so a party of four
chewing through eight inflated bodies takes that long even when it wins.

### The regression side

`lair-on-curve.ron` — the designed set-piece at depth 2, which the stat step
also moves:

| Configuration | Win rate | Rounds |
|---|---|---|
| shipped | 100% | 3.3 / 3 |
| step 0.25 | 100% | 3.1 / 3 |
| step 0.25 + cap 8 | 100% | 3.1 / 3 |

Both candidate changes are invisible to it, which is the point: the fix wanted
here must not flatten the shallow end that already works.

## What it does not say

- **Nothing about the species-band axis**, which is the third term and was never
  swept. At depth 5 the window has walked to apex-adjacent species
  (`zero_day`, `rootkit`) while the party's ceiling is level 12 —
  `TIER_ENTRY_STEPS = 2`, `TIER_WINDOW_STEPS = 3`, `APEX_ENTRY_STEP = 4`, read at
  `Game::danger_steps`. Eight rootkits at 2.4x is not eight worms at 2.4x, so the
  pack-cap table above is *not* a clean measurement of volume; it is volume with
  the depth-5 pool held fixed. This is the cheapest untested lever and the top
  open question.
- **The pack-cap experiment is softer than the real fix would be.** It deletes
  the surplus outright. Shipped, `group_pack`'s surplus stays on the map and is
  met on the next bump, so a capped pack in play is two fights rather than one.
- **No Trace.** `Game::stack_depth_multiplier` is the depth term times
  `trace_stat_mult`; an arena session has no Trace, so every figure here is the
  quiet-party case. A noisy party at depth 5 faces up to 1.45x more than this.
- **No companion Specials.** The bin plays the game's own All-Attack every
  round, so every win rate is a *floor* on party output — the same gap
  `balance_sim` has, and the reason
  `docs/measurements/2026-08-19-developed-companion-worth.md` says three of the
  four talent node kinds are unmeasured.
- **One biome, one seed base, one party shape.** `NullSector`, `seed: 41`, three
  identical Scrappers. A mixed party, a different biome's pool, or a player
  using items were all out of scope.
- **The player is over-levelled for the zone**, deliberately (level 20 in zone
  3, matching the existing scenarios). The real curve for an on-curve player is
  worse than this, not better.
- **Nothing about whether depth 5 is *reached* often.** `frames_for` sets a
  stack's depth from its link's distance to the spawn point, so a player can be
  handed a 6-frame stack as their only remaining lair — but how often that
  happens per run is unmeasured.

## Open questions

1. **Sweep the band window.** If `TIER_ENTRY_STEPS` advanced per two frames
   underground rather than per one, depth 5 would field depth-3-band species at
   depth-5 stats. That is plausibly both winnable and short, which neither knob
   measured here manages on its own.
2. **Gate it before tuning it.** `balance_sim` has no Stack term, and its
   surface field projection (`full_field_at_zone`, four groups at
   `zone_group_cap`) is not what the clearability test uses — that uses one
   group. So the multi-group total is ungated on the surface too: zone 6 permits
   32/group x 4 groups, and `MAX_GROUP_SIZE = 100` x `MAX_ENEMY_GROUPS = 4`
   permits 400 bodies in principle. A depth-scaled field term would let a
   regression fail here rather than a session discovering it.
3. **What is the target?** A bottom-of-stack pack at 62% with a fully developed
   party may be right or may be generous; nothing in the design says. The
   depth-4 figure (78%, 45.8 rounds, 2.4 companions lost) is arguably the more
   honest problem — it is winnable and *miserable*.
4. **Is the long fight the real fault?** Every configuration that makes depth 5
   winnable produces 30-to-40 rounds. Deep packs may want to be fewer and
   meatier — which is what a lair already is — and that is a composition change
   rather than a retune, needing the played arena rather than the bin.
