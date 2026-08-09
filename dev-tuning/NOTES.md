# Where this got to — 2026-08-08

Handoff notes for the roster-tuner work. Read `README.md` first for what the
tool *is*; this is what happened and what to do next.

## State

Three worktrees, all rebased onto `main` (which moved to 0.5.9 mid-session):

| worktree | branch | state |
|---|---|---|
| `feral-processes` | `main` | untouched by this work |
| `feral-processes-tuner` | `roster-tuner` | 7 commits, 1709 workspace tests green |
| `feral-processes-fixtures` | `fixture-cleanup` | 1 commit, 1212 engine tests green |

Nothing merged, nothing pushed, `assets/` never written to. Both branches
were left rebased and green, so they should merge forward cleanly unless
`main` has moved again.

## The headline finding: depth 5 is not a roster problem

This was the question the tool was built to answer, and the answer is that
the tool cannot fix it and shouldn't try.

**Zone and depth stat scaling compound.**

- `ZONE_STAT_GROWTH = 2`, applied as `2^(zone-1)` → zone 3 is **4x**
- `STACK_DEPTH_STAT_GROWTH = 1.35`, applied as `1.35^(depth-1)` → depth 5 is **3.32x**
- Together: **13.3x base stats**

For the rootkit actually fought at zone 3 / depth 5:

| stat | base | scaled |
|---|---|---|
| `base_def` | 10 | **133** |
| `base_hp` | 120 | **~1,596** |
| `base_atk` | 11 | **~146** |

The `base_atk` figure matches the observed transcript hits (151, 134, 198),
which is the check that the arithmetic above describes the real game rather
than a plausible model of it.

A level-20 geared player's ATK does not approach 133, so
`combat_damage.rs:71`'s `reduced.max(1)` floors **every** party hit at
`MIN_DAMAGE = 1`. Every party action in the transcript reads `for 1 damage`.

The fight therefore needs ~57,000 damage (36 enemies x ~1,596 HP) delivered
one point at a time. Not hard — arithmetically impossible. No value of
`base_hp` or `base_atk` inside any sane bound changes it, because the
problem is the multiplier and not the multiplicand.

**Second, independent fault: 36 enemies against 4.** The fought line reads
`rootkit x5, zero_day x11, sub_process x8, worm x12` — four groups, 36
members, versus the player and three Scrappers. This would still be lopsided
if the stat problem were fixed, and vice versa. Two faults, not one.

It compounds with depth in later zones: zone 5 depth 5 would be 16 x 3.32 =
**53x**.

Reproduce in about two seconds:

```sh
sed 's/reps: 50/reps: 1/' dev-arenas/stack-depth-5.ron > /tmp/d5.ron
cargo run --release --bin arena -- /tmp/d5.ron
```

### What is NOT the explanation

- **Not gear.** The standing note blamed a fixture with no gear.
  `dev-arenas/stack-depth-5.ron` has plasma_router, bastion_lattice,
  singularity_matrix and three level-12 Scrappers, and still loses 50/50 in
  a mean of 2.5 rounds at 0% HP. Do not re-suggest gear.
- **Not the roster.** See above.
- **Not Trace.** Trace was zero in every rep of both measurements.

### The decision this leaves open

Whether depth scaling should combine additively with zone rather than
multiplying, be capped in combination, or be left alone with player power
scaled up to meet it — that is a game-design call and was deliberately not
made. `tuning.rs` is deliberately code rather than data, and the tuner
cannot reach it by design.

## Baseline the tuner measured

```
opening-fight    100% win (want 92%),  47.8% HP left (want 62%)
full-group       100% win (want 75%),  98.9% HP left (want 45%)
stack-depth-5      0% win (want 55%),   0%   HP left (want 30%)
```

The `full-group` row is its own finding: a geared zone-3 party clears a full
enemy group having taken **about 1% damage**. Surface content at that point
is trivial and depth 5 is a wall. A cliff, not a curve.

## Known defect in the tuner: the objective is side-blind

**Fix this before acting on another proposal.**

A target says "this fight should be won 75% of the time". It does not say
whether to get there by buffing the enemy or nerfing the party. The first
real run did both: it raised `rootkit` (the opponent in `full-group.ron`)
*and* dropped `scrapper.base_def` from 5 to **0**, its bound floor — and
Scrappers are the *party* in that scenario. A stat lowered to satisfy one
fight applies to that species everywhere in the game.

Options, unchosen: per-role bounds, excluding any species named in a
scenario's `party` from movement, or a penalty term for moving a species the
player fields. The first is the most general and the third is the least
surprising.

## Suggested order tomorrow

1. **Decide the depth-scaling question** above. It is the biggest live
   balance problem in the game and everything else here is downstream of it.
   No code needed to decide — the arithmetic is all in this file.
2. **Fix the side-blindness** if the tuner is going to be used again.
3. **Land `fixture-cleanup`.** It is a real bug on `main` (see below) and is
   done and green.
4. **Phase B (burn / learned enemy tactics)** is still gated on the same
   thing it was gated on at the start: enemies carry at most one routine
   (`roll_wild_routine`, `game/spawning.rs:108`), so there is no per-turn
   decision for a policy to learn. Giving enemies real choices is a
   game-design change with no ML in it, and it has to come first.

## The /tmp inode leak (separate, fixed on a branch)

A build died mid-session with `No space left on device` on a tmpfs that was
15% full — it was **inodes**, not bytes: 1,048,576 of 1,048,576 used.

`scratch_assets_dir` (`crates/engine/src/tests/support.rs`) copies the whole
shipped asset set (~190 files) per test and left cleanup to the caller, so a
test that panics on a failed assert leaked the lot, and a helper returning a
bare `Game` had no way to clean up at all. Measured: **53 leaked installs
per suite run**, 5,437 found on the machine.

`fixture-cleanup` fixes it with an RAII guard — 53/run becomes 0/run,
verified by reverting the change, re-running and counting. **Until that
lands, every engine suite run on `main` leaks 53 directories.** Clear with
`rm -rf /tmp/feral_*` if a build starts failing for no visible reason.
