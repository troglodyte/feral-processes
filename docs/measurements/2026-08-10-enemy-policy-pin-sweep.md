# Enemy policy pin sweep

**Date:** 2026-08-10
**Instrument:** `train --log-dir`, then `analysis/policy_report.py`
**Data:** `dev-logs/policy-sweep/` — 118 MB of JSONL, gitignored, regenerable
from the commands below. The weights and `-report.ron` files beside it are
checked in.

## The claim

The three features pinned to zero in `assets/policies/enemy_battle.ron` are
a **design boundary that a free search will always cross**, not a tuning
accident of one 2026-08-09 run. Trained again from a different starting
point, an unpinned policy relearns "kill the player" (`target_is_player:
+6.50`) and downs **zero companions across 1,600 fights** where the uniform
baseline downs 267. Pinning the first two features does not stop it: the
search re-derives the same behaviour through `target_def_rel: -8.15`,
because what Defend grants is +6 DEF and "avoid high-DEF targets" is the
same sentence in other words.

The shipped three-pin config also turns out to preserve the most move
variety of any config measured, which nobody had checked before.

## How to reproduce it

```sh
cargo build --release --bin train
OUT=dev-logs/policy-sweep
COMMON="--scenarios dev-training --iters 30 --pop 40 --reps 200 --seed 1 --log-dir $OUT"

./target/release/train --out $OUT/unpinned.ron --label unpinned $COMMON
./target/release/train --out $OUT/pin2.ron --label pin2 $COMMON \
    --pin target_is_player,target_bracing
./target/release/train --out $OUT/pin3.ron --label pin3 $COMMON \
    --pin target_is_player,target_bracing,target_def_rel

cd analysis && .venv/bin/python policy_report.py --log-dir ../dev-logs/policy-sweep
```

About 8 minutes per config on 16 cores; 25 minutes for the sweep. **`--out`
must not point at `assets/policies/enemy_battle.ron`** — that is the file
the running game reads, and a sweep would overwrite the shipped weights with
a deliberately-rejected config.

Only the two evaluation passes either side of the search are logged. The
search itself is 1.9M fights of discarded candidates and would cost tens of
gigabytes; `evaluate_set` passes `None` there and says why.

## The numbers

### Companions downed, per 1,600 fights

The finding, and the one that needs no interpretation.

| config | baseline | trained |
|---|---|---|
| unpinned | 267 | **0** |
| pin2 | 267 | 285 |
| pin3 | 267 | 224 |

Share of enemy swings landing on a companion rather than the player, over
scenarios that field a party: baseline **66.7%**, unpinned **7.2%**, pin2
38.6%, pin3 41.8%.

### Learned weights — a replication

Signs and rough magnitudes match the 2026-08-09 report on an independent
run. This is replication, not new information, and it is the reason to
trust the entry above.

**Do not generalise it to the other fourteen features.** These five are, as
it turns out, the identifiable ones — see
[weight identifiability](2026-08-10-weight-identifiability.md), which
retrained the same `pin3` configuration at three seeds and found seven of
the sixteen free features flipping sign at indistinguishable fitness. That
this table replicates is real; it replicates because of *which* features are
in it, not because trained weights are generally reproducible.

| feature | unpinned | pin2 | pin3 | 2026-08-09 |
|---|---|---|---|---|
| `target_is_player` | **+6.50** | 0 pinned | 0 pinned | +7.42 (run 1) |
| `target_bracing` | −1.93 | 0 pinned | 0 pinned | −3.34 (run 1) |
| `target_def_rel` | −2.78 | **−8.15** | 0 pinned | −7.32 (run 2) |
| `target_hp_frac` | −7.38 | −7.98 | −9.63 | −10.86 (run 3) |
| `est_damage_frac` | +3.94 | +7.66 | +7.66 | +10.10 (run 3) |

Enemy win rate: baseline 0.254 → unpinned 0.627, pin2 0.566, pin3 0.551.
Pinning the third feature costs 1.5 points on top of the second's 6.1.

### Move variety — new

Share of each species' swings taken by its single most-used move, averaged
over the 16 species that appear. 1.0 would mean a species used exactly one
move all sweep.

| config | baseline | trained |
|---|---|---|
| unpinned | 0.583 | 0.926 |
| pin2 | 0.583 | 0.910 |
| pin3 | 0.583 | **0.844** |

Every trained policy collapses toward one move per species — the
2026-08-09 finding that effect-carrying moves are priced not to be worth
taking, seen from the other end. What is new is that **the shipped config
collapses least**. That is a point in favour of the pins that had not been
measured, and it is a side effect rather than a design goal: pinning the
targeting features leaves the search less able to build a single
kill-the-player line and more reliant on what each species actually has.

### Focus fire

Median HP fraction of the target at the moment it was chosen: baseline
0.949 → trained 0.81–0.85 across all three configs. Real, and more modest
than `target_hp_frac: -9.63` reads. `analysis/out/focus_fire_hist.png` has
the shape, which the mean flattens.

## What it does not say

- **Nothing about Defend.** `target_bracing` was `False` on all 279,824
  swings, because `arena::run_rep` plays the party as All-Attack and nobody
  braces. The most interesting question about the shipped policy is the one
  this data is structurally blind to. `analysis/policy_report.py::
  check_bracing` prints that as a line in every report rather than leaving
  it to be assumed, and it starts reporting a real count the moment the
  arena grows a party plan that braces.
- **Nothing about how a fight feels.** Win rates cannot say whether the
  trained enemy reads as smarter or merely harsher; `dev-arenas/policy-*.ron`
  exist for that and still want a person.
- **Nothing about the shipped weights specifically.** `pin3` here is a
  *fresh* three-pin training run, not the file in `assets/`. It lands in
  the same place, which is the point, but the numbers above are not a
  measurement of what players currently face.

## Open question

**The baseline moved and nobody knows why.** This sweep measured an
all-zero-weights enemy win rate of **0.254**; the 2026-08-09 report records
**0.324** for what should be the same thing — same scenarios, same
`--reps 200`, same `--seed 1`.

Determinism within the sweep is intact: all three configs produced
byte-identical baseline rows, so the harness is reproducible and every
comparison *inside* this document holds.

**The likely cause is that an arena number is not comparable across
builds**, which if true is the more useful finding of the two. Ruled out
first, by inspection:

- `dev-training/` and `assets/` are **unchanged** since `v0.5.13`, when the
  policy shipped. Same scenarios, same species, same items.
- 0.5.17's routine-availability gate lives entirely in
  `crates/app-core/src/app/battle.rs` — the *player's* picker.
  `arena::run_rep` never opens a picker; it calls `battle_plan_remaining` in
  the engine. It cannot reach these fights.
- The only other engine changes since `v0.5.13` are `combat_rewards` (drop
  tags — what a kill pays, not who wins) and a catalog lookup.

What *did* change is that **two new `Resource`s were registered**:
`BattleTimeline` in 0.5.14 and `BattleTelemetry` in 0.5.15. This repo has
already been bitten by that: registering a resource shifts bevy's query
iteration order, and a system that iterates a query and draws from
`GameRng` then consumes the stream in a different order. The fights at a
given seed are not weaker — they are **different fights**.

That reading is consistent with every observation and nothing contradicts
it, but it has not been proved. The decisive test is to check out `v0.5.13`,
run the baseline pass alone, and see whether 0.324 comes back; it needs a
separate worktree, since it means building an old tree.

Two consequences if it holds, and the second is the one worth carrying:

1. Nothing about the shipped game got easier. No player-facing regression.
2. **Arena win rates may only be compared within one build.** A cross-report
   delta is meaningless unless the resource set is identical, which no
   report currently records. Treat the numbers in any measurement here as
   valid against their own baseline and against nothing else.
