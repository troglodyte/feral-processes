# 2026-08-19 — What the attack roll did to the shipped arenas

## The claim

Slice 1 of the combat model — attack rolls against a derived Evasion,
percentage-point Mitigation, rolled weapon bands, crits and a four-rung fumble
ladder — **lengthens fights without moving which ones are winnable.** Every
shipped `dev-arenas/` scenario keeps the verdict it had the day before the
branch: the same twelve wins, the same two losses, and `stack-depth-5` still at
**0% over 50 runs**. What changed is duration and margin. Depth 5 now takes
11.2 rounds to lose where it took 7.4, and the opening fight is the only
scenario in the set that reads as a fight at all — 98% wins with 58% of the
player's Integrity gone, against nine scenarios that finish at 92–100% HP.

The second half of that is the finding worth acting on, and it is not about
this slice: **nine of the fourteen authored scenarios are walkovers.** They
were written to exercise mechanisms rather than to be hard, so this is not a
regression — but it does mean the arena currently gates almost nothing about
difficulty, and a change that made the game much easier would show up in only
two of its fourteen scenarios.

## How to reproduce it

At `fec229d` (the last commit of the combat-model branch before the play pass):

```sh
cargo build --release --bin arena
for f in dev-arenas/*.ron; do
  echo "=== $(basename "$f" .ron)"
  ./target/release/arena "$f"
done
```

Each scenario carries its own seed and repetition count in its `.ron`; the
figures below are whatever the file asks for, unmodified. The five `policy-*`
scenarios and `class-mirror` are single-seed set-pieces rather than sweeps, so
they report one run and no rate.

The pre-branch column is quoted from
[2026-08-19 — The Stack's depth curve](2026-08-19-stack-depth-curve.md), taken
one day earlier on the same scenarios.

## The numbers

New, this build:

| scenario | wins | rounds (mean/median) | player HP left |
|---|---|---|---|
| `opening-fight` | 98% (49/50) | 8.6 / 8 | 58% |
| `gear-passives` | 100% (30/30) | 18.7 / 18 | 92% |
| `developed-companion` | 100% (50/50) | 7.6 / 7 | 98% |
| `full-group` | 100% (50/50) | 7.6 / 7 | 98% |
| `deep-lair` | 100% (40/40) | 6.9 / 7 | 100% |
| `geared-vs-boss` | 100% (20/20) | 3.8 / 4 | 99% |
| `lair-on-curve` | 100% (50/50) | 3.2 / 3 | 100% |
| `stack-depth-5` | **0%** (0/50) | 11.2 / 10 | 0% |

Single-seed set-pieces: `class-mirror` won in 21 rounds at full HP;
`policy-focus-fire` won in 25 at full HP; `policy-back-rank` (15),
`policy-deep-stack` (20), `policy-defend-taunt` (13) and `policy-full-kit` (5)
all lost. Those five are authored to be lost — they exist to watch the trained
policy play, not to be cleared.

Reproductions rather than discoveries — the two scenarios with a directly
comparable figure from the day before:

| scenario | pre-branch | this build | reading |
|---|---|---|---|
| `lair-on-curve` (depth 2) | 100%, 3.3 rounds | 100%, 3.2 rounds | unchanged |
| `stack-depth-5` | 0%, 7.4 rounds | 0%, 11.2 rounds | **still lost, ~50% longer** |

The depth-5 row is the one that says something. The prior measurement's whole
argument was that depth 5 fails on *volume* — 28 bodies against 4 — and that
levers which only reduce incoming damage lengthen the loss rather than
preventing it. An attack roll is exactly such a lever, and it behaved exactly
that way.

## What it does not say

- **Nothing here is a difficulty measurement.** `balance_sim` is what gates
  the progression curves and it is checked on every `cargo test`; these are
  authored set-pieces, and nine of them are walkovers by construction.
- **Round counts do not compare across builds, and only just compare here.**
  Changing the model reshuffles the `GameRng` stream as well as the
  arithmetic, so two runs of the same seed on two builds are different runs.
  The win *rates* are coarse enough to survive that; the round means are
  quoted as a direction, not a difference.
- **`assets/policies/enemy_battle.ron` is now stale.** Its weights were
  trained against a world where every swing landed and mitigation was
  subtractive. `bracing_still_draws_more_fire_under_the_shipped_weights` says
  the shipped policy still behaves, but nothing says it plays *well*.
  Retraining is deliberately deferred to slices 2 and 3.
- **`expected_damage` excludes the fumble ladder**, whose two damaging rungs
  land on the attacker. Every `balance_sim` projection therefore mildly
  overstates an attacker's net output — by roughly the fumble rate times the
  recoil fraction, so a couple of percent, but in a known direction.
- **No human has played any of this.** A green suite and a fourteen-scenario
  arena batch are not evidence of feel, and four of the knobs this slice
  introduced were set on judgement rather than measurement.

## Open questions

1. **Does a fight at even odds read as a fight or as a slog?** `HIT_CHANCE_MIN`
   0.25 / `HIT_CHANCE_MAX` 0.95 mean an even matchup trades misses half the
   time. `FUMBLE_CHANCE` and `CRIT_CHANCE` are the dials.
2. **Is `FUMBLE_RUNG_THRESHOLDS` weighted so Crash is a rare disaster rather
   than a coin flip that ends runs?** Currently 55/30/12/3 across
   Exposed/Recoil/Opening/Crash, within a 5% fumble band.
3. **Do `party_stat_bonus`/`wielded_stat_bonus` still make sense?** Each
   contributes a tenth of a companion's mitigation *percentage* to the player.
   The cap bounds the total, but a full party may now read as immune. This
   slice left both unchanged deliberately.
4. **Does the arena need harder scenarios?** Nine walkovers out of fourteen is
   the finding above, and it predates this branch.
