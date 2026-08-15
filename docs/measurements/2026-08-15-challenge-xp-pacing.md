# 2026-08-15 — Challenge-scaled XP: what a level now costs

## The claim

Pricing a kill by challenge rather than by the victim's HP bar slows
on-curve levelling by roughly 2x and stops over-levelled farming almost
completely, without stalling a player who has nowhere harder to go. Measured
against the real roster: reaching level 5 in the opening zone costs **34
kills** where it used to cost about 6; grinding zone-1 drones from level 5 to
10 costs **208 kills**, which is the intended dead end; and the same stretch
costs **37 kills** four frames down a zone-1 Stack, which is the intended way
out of it. On-curve play at the level a zone actually demands settles at
about **4 kills a level**.

The second half of the change — halving the level count and doubling what a
level grants — is power-neutral and was confirmed to be so by two independent
instruments rather than by argument: `balance_sim`'s reach curve halved while
staying linear, and the ability-magnitude pin reproduced its existing band at
half the level with the band untouched.

## How to reproduce it

The pacing table came from a throwaway `#[test]` appended to
`crates/engine/src/tests/combat_rewards.rs`, not from a shipped test — it
measures rather than asserts, so it was deleted after the run. It builds a
`Game::new(5, DifficultyMode::Forgiving, ...)`, spawns one wild program via
`spawn_wild_on_player_tile`, overwrites its whole stat block to the figures
in the table below, then repeatedly calls

```rust
let earned = game.kill_xp(victim);
game.award_player_xp(player, earned);
```

recording the kill count as each level lands, and runs with
`cargo test --workspace scratch_measure_pacing -- --nocapture`.

Awarding directly rather than fighting is deliberate and is the main blind
spot — see below. The stat blocks are the shipped `base_hp`/`base_atk`/
`base_def` scaled by hand: `ZONE_STAT_STEP` is linear, so zone 3 is 3x, and
`STACK_DEPTH_STAT_STEP` is 0.35 a frame, so depth 4 is 2.4x.

The two curve figures came from `cargo test --workspace balance_sim` with a
temporary `eprintln!` of `required_levels` beside each
`assert_steps_stay_flat` call.

## The numbers

Kills to reach each level, from level 1, against one repeated opponent:

| Opponent | stats (hp/atk/def) | →L5 | →L10 | XP/kill at L10 |
|---|---|---|---|---|
| zone-1 drone | 48 / 4 / 1 | 34 | 242 | 12 |
| zone-1 sprite | 41 / 6 / 2 | 43 | 298 | 10 |
| zone-1 drone, depth 4 | 115 / 10 / 2 | 6 | 43 | 55 |
| zone-3 scrapper | 240 / 36 / 9 | 2 | 10 | 259 |

The zone-3 row is measured well below the level that zone is beatable at
(`balance_sim` says 12), so it reads as "fighting far above your weight pays
near the ceiling" rather than as a viable strategy — the party loses that
fight. At level 12, where the fight is real, the same scrapper sits at a
power ratio of 0.70, which is exactly `DIFFICULTY_EASY_MAX` and therefore a
factor of exactly 1.0: 240 XP against a 960-XP level, or 4 kills a level.

`balance_sim`'s minimum level to clear each zone, new against old — a
replication of a shape already asserted, not a discovery, and the assertions
are unchanged because they gate linearity rather than these figures:

| | z1 | z2 | z3 | z4 | z5 | z6 | z7 | z8 | z9 | z10 |
|---|---|---|---|---|---|---|---|---|---|---|
| gear-free, new | 1 | 8 | 12 | 16 | 19 | 25 | 31 | 37 | 42 | 49 |
| gear-free, old | 1 | 15 | 24 | 32 | 47 | 61 | 76 | 90 | 106 | 121 |
| geared, new | 1 | 5 | 8 | 14 | 18 | 24 | 29 | 35 | 41 | 46 |
| geared, old | 1 | 10 | 18 | 31 | 43 | 56 | 70 | 83 | 97 | 112 |

The step spread widened from 8–16 to 3–7, a ratio of 2.0x to 2.33x against
`LINEAR_STEP_GUARD_MULTIPLIER` of 3. That is the integer search quantising
into units twice as coarse, not a curve that compounds.

## What it does not say

- **It is not a fight.** XP is awarded directly, so nothing here measures
  whether the player can survive the opponent, how many turns it takes, or
  what a group of them costs. The zone-3 row in particular describes a fight
  the party loses. Real pacing is slower than every row above by whatever
  fraction of fights are fled or lost, and by the cost of finding opponents
  at all.
- **One opponent, repeated.** Real play mixes con-colours, and the factor is
  computed per kill, so a real run's rate lies somewhere between the rows.
- **No party.** `Game::kill_xp` divides by the player's power alone, so these
  numbers are unchanged by roster size — but a real party clears fights
  faster, which the kill counts here cannot express.
- **Nothing about whether the pace is right.** These are costs, not verdicts.
  `balance_sim` cannot judge them either: it models no XP at all, which is
  why the slowdown half of this change is ungated and answerable only by
  play.
- **Zone 1 has an intended answer and it is untested here.** The 208-kill
  wall assumes the player descends or breaches instead. Whether that reads as
  guidance or as a wall is a question for a session, not an instrument.
