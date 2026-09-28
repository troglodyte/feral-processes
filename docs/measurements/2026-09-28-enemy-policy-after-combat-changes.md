# 2026-09-28 — Does the enemy policy survive the combat changes?

## The claim

`assets/policies/enemy_battle.ron` (trained 2026-08-09/10) **still earns its
place** after everything combat has gained since: slice 1's hit rolls,
damage spread and percentage mitigation (08-19), the swing loop and
`attacks_per_round` (09-02), and the threat-ratio duel reading (09-24).
Against the uniform baseline it roughly doubles the enemy's win rate on
the training set and multiplies it several-fold on the held-back
`dev-arenas/policy-*` set-pieces. **No retrain is needed on this
evidence.** The features were kept current as combat changed —
`est_damage_frac` and `would_kill` read `battle::expected_damage` — so the
weights meet the numbers they were meant to meet.

A retrain would mostly learn from three scenarios: five of the eight
`dev-training/` scenarios have the enemy winning 0–3.5% even under
uniform play (see the table). Toughen some of those before the next
retrain rather than after.

## How to reproduce it

Built at `c0aa7170` on top of v0.13.246.

```sh
cargo build --release --bin train
./target/release/train --eval assets/policies/enemy_battle.ron \
    --scenarios dev-training --reps 200 --pop 16

mkdir -p /tmp/heldout
cp dev-arenas/{opening-fight,full-group,policy-back-rank,policy-deep-stack,policy-defend-taunt,policy-focus-fire,policy-full-kit}.ron /tmp/heldout/
./target/release/train --eval assets/policies/enemy_battle.ron \
    --scenarios /tmp/heldout --reps 200 --pop 16
```

`--eval` scores the baseline (all-zero weights) and the named file on the
same seeds (`--seed 1`, the default) and stops; `--pop` only sizes the
worker pool here. Under a minute per set on 16 cores.

## The numbers

Enemy win rate and mean player Integrity remaining, uniform baseline →
shipped policy. 200 reps per scenario; at that count sampling noise is
roughly ±3.5 points near 50%.

**Training set** (`dev-training/`): enemy win rate 0.153 → 0.294.

| Scenario | Enemy win | Player HP |
|---|---|---|
| 01-opening-solo | 0.005 → 0.005 | 0.688 → 0.667 |
| 02-early-pair | 0.000 → 0.000 | 0.931 → 0.936 |
| 03-midgame-group | 0.000 → 0.015 | 0.941 → 0.912 |
| 04-back-rank | 0.810 → 0.980 | 0.160 → 0.016 |
| 05-status-heavy | 0.035 → 0.460 | 0.619 → 0.443 |
| 06-geared-lategame | 0.000 → 0.000 | 0.940 → 0.967 |
| 07-rolled-field | 0.000 → 0.045 | 0.895 → 0.859 |
| 08-rolled-stack | 0.370 → 0.850 | 0.512 → 0.122 |

**Held-back set** (`dev-arenas/`, never trained on):

| Scenario | Enemy win | Player HP |
|---|---|---|
| full-group | 0.000 → 0.000 | 0.979 → 0.980 |
| opening-fight | 0.030 → 0.025 | 0.557 → 0.607 |
| policy-back-rank | 0.130 → 0.680 | 0.818 → 0.302 |
| policy-deep-stack | 0.665 → 0.910 | 0.290 → 0.087 |
| policy-defend-taunt | 0.005 → 0.215 | 0.931 → 0.736 |
| policy-focus-fire | 0.015 → 0.320 | 0.870 → 0.618 |
| policy-full-kit | 0.080 → 0.680 | 0.819 → 0.286 |

New: nothing had scored the shipped file since slice 1 landed. The
held-back gains reproduce the pattern the 2026-08-10 sweep reported, but
the absolute rates cannot be compared with it — see below.

## What it does not say

- **Group fights only.** A battle map's hostiles are decided by the
  hand-written `tactical/ai.rs`, which never reads these weights.
  Battle maps are off by default (`Profile::tactical_battles`), so the
  policy still covers the default path.
- **Not whether a retrain would do better.** It says the file works, not
  that it is the best one. Any retrain faces the same identifiability
  warning `assets/policies/README.md` gives.
- **Not comparable with earlier numbers.** Arena rates compare within one
  build; `dev-logs/policy-sweep/` predates slice 1.
- **`deep-lair` was left out:** it names a `dev-saves/` template, which
  only the `arena` bin can resolve.
- **All-Attack party only** (`--party-plan` default); a bracing party was
  not measured.
