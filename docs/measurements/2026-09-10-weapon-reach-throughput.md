# 2026-09-10 — What a wide swing is worth against the single-target ladder

## The claim

The two shipped reach weapons are priced **below** the single-target ladder
on raw throughput and still clear a fight **faster** than it does, because
breadth beats a band once there is more than one body to spend it on. The
Scatter Lance takes 5.8 rounds where the Arc Lance takes 6.4 against one
four-deep group (**−9%**), and the Broadcast Storm takes 9.8 rounds where
the Plasma Router takes 10.8 against two three-deep groups (**−9%**), while
carrying 3–6/+2 against the Arc Lance's 5–10/+3 and 4–8/+3 against the
Plasma Router's 5–15/+4. Neither moves a `balance_sim` curve, and neither
can: the sim's fitted party carries no reach weapon and the sim models no
reach at all. The gap is written here because nothing in the suite measures
it.

The run also found a **crash**, which is the finding that mattered most —
see *What it does not say*.

## How to reproduce it

Four scenarios, differing in exactly one row (the player's weapon), 200 reps
each from seed 31. Two arms field one group so a `WholeEnemyGroup` sweep has
a group to sweep; two field two groups, which is the only shape that tells
`AllEnemies` from `WholeEnemyGroup` at all.

```sh
cargo run --bin arena -- dev-arenas/measure-reach-arc-lance.ron
cargo run --bin arena -- dev-arenas/measure-reach-scatter-lance.ron
cargo run --bin arena -- dev-arenas/measure-reach-plasma-router.ron
cargo run --bin arena -- dev-arenas/measure-reach-broadcast-storm.ron
```

Each prints a warning that the authored group size exceeds what zone 2 would
field — that is the arena authoring its own composition, which is what
`begin_battle` exists for, and it is deliberate here: a reach weapon is
measured against the pack it is for.

## The numbers

New. Nothing here reproduces anything previously believed, because the
mechanic did not exist before this branch.

**Standard tier, one group of four** (`rootkit` ×4, player level 12 zone 2,
two level-8 companions on Kinetic Edges):

| weapon | band | reach | win rate | rounds (mean/median) | player HP left |
| --- | --- | --- | ---: | ---: | ---: |
| Arc Lance | 5–10 / +3 ATK | — | 100.0% | 6.4 / 6 | 89% |
| Scatter Lance | 3–6 / +2 ATK | `WholeEnemyGroup`, recharge 2 | 100.0% | 5.8 / 6 | 90% |

**Premium tier, two groups of three** (`rootkit` ×3 + `cipher` ×3, same
party):

| weapon | band | reach | win rate | rounds (mean/median) | player HP left | companions down |
| --- | --- | --- | ---: | ---: | ---: | ---: |
| Plasma Router | 5–15 / +4 ATK | — | 89.5% | 10.8 / 10 | 61% | 0.25 |
| Broadcast Storm | 4–8 / +3 ATK | `AllEnemies`, recharge 5 | 90.0% | 9.8 / 9 | 61% | 0.22 |

Both single-group arms are walkovers, which is the standing problem with
this instrument — `2026-08-19-combat-model-slice-1.md` recorded that nine of
fourteen shipped scenarios are — so the standard-tier row is a duration
comparison and not a difficulty one. The two-group arms are the marginal
pair and carry the win rate worth reading: **89.5% → 90.0% is inside the
noise of 200 reps**, so the Storm buys speed and not survival.

## What it does not say

- **`balance_sim` models no reach at all**, so nothing in the suite gates
  any of this. The curves did not move, and a future retune of either
  weapon will not move them either — this file is the only record that the
  gap was ever measured.
- **The bin invokes nothing.** `arena::run` drives the party through
  `PartyPlan::AllAttack`, so no Special ever fires and no class ability is
  spent. That is what makes the two arms comparable and it is also why the
  numbers say nothing about a party that plays its kit.
- **Two RNG streams, not the same fights refought.** Changing the player's
  weapon changes the band rolled on every swing, so the streams diverge from
  the first blow. The deltas are distributions over 200 reps, not paired
  differences — `2026-08-19-party-passive-bonus-removal.md` carries the same
  caveat for the same reason.
- **Nothing here is a battle map.** The arena's group model is what these
  four files fight in; the tactical sweep's geometry, its friendly fire and
  its recharge are held by tests and have never been measured.
- **Nobody has played either weapon.** A green suite is not evidence of
  play, and these numbers are the bin's, not a session's.

## What it found that it was not looking for

The Broadcast Storm's second rep **panicked**: `end_battle` read a
`BattleState` that no longer existed. A wide swing can empty the last group
and kill its own swinger in one turn — one body's blow lands the killing
hit, a later body's fumble puts the swinger down on a Recoil rung — and
`battle_resolve_round` then spent the round's upkeep on a fight that had
already been torn down. A narrow swing cannot reach that state at all,
because the fumble that kills the swinger deals its defender nothing.

Fixed by returning early from `tick_round_status_effects` when no fight is
open, which is what `run_tactical_beat`'s own doc comment had already
identified as the hazard in that function. **The suite did not find this and
could not have**: it needs a fatal fumble on a body after the one that
emptied the roster, which is a stream to hunt for rather than a state to
construct. Two hundred reps found it twice.
