# dev-training

The scenario set the enemy battle policy is trained on. Same file format as
`dev-arenas/` — see that directory's README for the schema; nothing here
adds a field.

```sh
cargo run --release --bin train -- --out assets/policies/enemy_battle.ron \
    --scenarios dev-training/ --iters 30 --pop 40 --reps 200 --seed 1 \
    --report docs/superpowers/reports/training.ron
```

## The rule that matters

**`dev-arenas/` is the held-back test set and is never trained on.** Its
three scenarios are what the trained policy is *measured* on, and a number
measured on the set that was optimised against says only that the optimiser
worked. Adding a `dev-arenas/` scenario here, or pointing `--scenarios` at
that directory, silently turns the proof of concept's headline result into
a tautology.

## What each scenario is for

| File | Covers |
|---|---|
| `01-opening-solo.ron` | Level 1, zone 1, no companions. One target, so nothing to choose — it is here so that whatever is learned cannot make the opening unwinnable. |
| `02-early-pair.ron` | Level 6, one companion, two opponents. The smallest composition where "who do I hit" is a real question. |
| `03-midgame-group.ron` | Level 12, zone 3, party of four. The composition the balance sweep models. |
| `04-back-rank.ron` | Four enemy groups, two of them past `ENGAGED_GROUPS`. The only scenario that puts `move_ranged` and `self_front_group` under real pressure. |
| `05-status-heavy.ron` | Every opponent carries a status effect. Where the three interaction features earn their place or fail to. |
| `06-geared-lategame.ron` | Zone 5, level 20, fully equipped. Focusing fire is the only way the other side wins a round here. |
| `07-rolled-field.ron` | A **rolled** surface encounter rather than an authored one, so the policy meets compositions nobody chose for it. |
| `08-rolled-stack.ron` | A rolled Stack encounter at depth 3, where zone and depth scaling compound. |

Between them: player levels 1/6/12/20, zones 1/3/5, Stack depth 0 and 3,
party sizes 0/1/2/3, and both authored `opponents` and rolled `encounter`
compositions.

## Two things to know before editing this set

**Every scenario must use a `Fresh` player.** `equip`, `inventory` and
`party` are `Fresh`-only, and a `Save`/`Template` player would carry a party
the set cannot see or vary.

**Baseline player win rates should span the range.** Measured at all-zero
weights, this set runs from 0.22 to 1.00. A set where the player wins
everything gives the enemy no win-rate gradient at all and leaves the whole
fitness resting on the HP tiebreak; a set where the player loses everything
is the same problem from the other end.
