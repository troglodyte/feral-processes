# Base bench: the memories baseline on `bench-economy` and `chains`

Follows the [staff baseline](2026-10-05-base-bench-staff-baseline.md). P3 of
the base bench adds a memories section to the report: morale bands, the morale
spread, bond bands, and a fired/formed count for every memory kind. This is
what it says on the two templates before any target range or knob is chosen.
Nothing here is tuned.

## The claim

Over 6,000 ticks, sieges off, seeds 1-5, 15 staff on both templates
(`staff_ticks` 90,000 everywhere, `stopped_at` None on all ten runs):

- **Only 15 of the 35 memory kinds fire on `bench-economy`, 14 on `chains`.**
  Every kind that needs a battle, a tantrum, a siege siphon, a departure, a
  roster overflow or a comfort floor is at zero, because the run contains
  none of those (see "Kinds that never fired").
- **Relationships are thin and nearly all Neutral.** 37.2 live directed pairs
  per run on `bench-economy` (2.5 per program), 98.3% Neutral, 1.7% Rival, no
  Friend, Close or Enemy on any seed. `chains` has 33.2 pairs, 92.4% Neutral,
  5.1% Rival, 2.5% Friend (three seeds), still no Close or Enemy. The bond
  ladder above Friend is out of reach in a siege-free 6,000 ticks.
- **The morale bands are the staff story again.** `bench-economy` is 79% `even`,
  12% `content`, 6% `devoted`, 2.7% `uneasy`, never `bitter`; `chains` is 69%
  `even`, 19% `bitter`, 11% `uneasy`, never `devoted`. The morale spread (staff-tick
  standard deviation) is 8.2 against 12.8.
- **`chains` frays every program by 6,000 ticks** (`frays` is 15 on all five
  seeds), which the 3,000-tick staff baseline (3-4 frays) did not reach.

## How to reproduce it

```sh
cargo build --release --bin bench
for t in bench-economy chains; do for s in 1 2 3 4 5; do
  ./target/release/bench run --template $t --ticks 6000 --seed $s --no-sieges \
    --order patch_routine:9999 --order bytecode_block:9999 --order ice_breaker:9999 \
    --out $t-$s.ron
done; done
```

Branch `memories-bench` at 8ecd4788, release. The reports were parsed with a
throwaway script (counts summed per kind, mean over seeds); per-1000 rates
are the five-seed mean count divided by 6. Counts are whole-base (all 15
staff together), not per program.

## The numbers

All new. Shares of `staff_ticks` unless noted. Bond shares are over the live
directed relationships at the last tick.

### Bands, spread, bonds

| measure | `bench-economy` seeds 1-5 | mean | `chains` seeds 1-5 | mean |
|---|---|---|---|---|
| band bitter | 0 / 0 / 0 / 0 / 0 | 0 | 0.097 / 0.110 / 0.343 / 0.284 / 0.097 | 0.186 |
| band uneasy | 0.016 / 0.022 / 0.026 / 0.026 / 0.046 | 0.027 | 0.090 / 0.089 / 0.091 / 0.133 / 0.154 | 0.112 |
| band even | 0.829 / 0.808 / 0.819 / 0.786 / 0.709 | 0.790 | 0.787 / 0.797 / 0.565 / 0.580 / 0.741 | 0.694 |
| band content | 0.123 / 0.087 / 0.111 / 0.142 / 0.142 | 0.121 | 0.025 / 0.0045 / 0.0019 / 0.0028 / 0.0076 | 0.0084 |
| band devoted | 0.032 / 0.084 / 0.044 / 0.046 / 0.104 | 0.062 | 0 / 0 / 0 / 0 / 0 | 0 |
| morale_spread | 6.80 / 8.50 / 7.53 / 8.03 / 9.94 | 8.16 | 10.08 / 9.91 / 16.33 / 16.29 / 11.18 | 12.76 |
| relationships | 36 / 39 / 37 / 39 / 35 | 37.2 | 42 / 35 / 25 / 33 / 31 | 33.2 |
| relationships per staff | 2.40 / 2.60 / 2.47 / 2.60 / 2.33 | 2.48 | 2.80 / 2.33 / 1.67 / 2.20 / 2.07 | 2.21 |
| bond Neutral | 0.972 / 1 / 1 / 1 / 0.943 | 0.983 | 0.976 / 0.857 / 0.880 / 0.970 / 0.935 | 0.924 |
| bond Rival | 0.028 / 0 / 0 / 0 / 0.057 | 0.017 | 0 / 0.114 / 0.080 / 0.030 / 0.032 | 0.051 |
| bond Friend | 0 / 0 / 0 / 0 / 0 | 0 | 0.024 / 0.029 / 0.040 / 0 / 0.032 | 0.025 |
| bond Close, Enemy | 0 | 0 | 0 | 0 |

### Staff, alongside (same runs, same seeds)

So the memories numbers line up with P2. These are from the same ten
`bench` runs, not a second set.

| measure | `bench-economy` seeds 1-5 | mean | `chains` seeds 1-5 | mean |
|---|---|---|---|---|
| on_shift_share | 0.952 / 0.934 / 0.905 / 0.897 / 0.891 | 0.916 | 1.000 / 0.996 / 0.851 / 0.842 / 0.991 | 0.936 |
| morale_mean | 3.27 / 5.11 / 3.38 / 3.49 / 4.66 | 3.98 | -4.10 / -5.17 / -13.36 / -12.20 / -5.09 | -7.99 |
| morale_min | -14.99 / -13.99 / -15.85 / -14.46 / -19.20 | -15.70 | -47.96 / -56.17 / -54.65 / -54.17 / -58.39 | -54.27 |
| sulking_share | 0.023 / 0.032 / 0.052 / 0.052 / 0.071 | 0.046 | 0.218 / 0.242 / 0.459 / 0.447 / 0.262 | 0.325 |
| downed_tools share | 0 | 0 | 0 / 0.0037 / 0.150 / 0.160 / 0.0090 | 0.064 |
| coherence critical_share | 0.0012 / 0.0012 / 0.0049 / 0.0012 / 0.0011 | 0.0019 | 0.421 / 0.425 / 0.442 / 0.433 / 0.426 | 0.429 |
| frays | 0 / 0 / 0 / 0 / 0 | 0 | 15 / 15 / 15 / 15 / 15 | 15 |
| first_fray (`chains`) | none | | 1849 / 2228 / 2387 / 2559 / 2000 | 2205 |
| tantrums | 0 | 0 | 0 | 0 |

Against the staff baseline's 6,000-tick `bench-economy` table (taken at
`54eb6049`) the economy template reads a little worse now: sulking 0.046
against 0.0315, on shift 0.916 against 0.939. That is the squeeze-past fix
and later commits moving the trajectory, not this branch's work (this branch
changed no sim code); it is the row to compare against from here.

### Memory kinds that fired

Counts are seeds 1 / 2 / 3 / 4 / 5. "Fired" is every `Record::Remember`
(a new memory or a reinforcement); "formed" is only the `new: true` ones.
Rates are per 1,000 ticks, mean of five seeds, whole base.

`bench-economy`:

| kind | fired | fired per 1000 | formed | formed per 1000 |
|---|---|---|---|---|
| settled_in | 107 / 103 / 98 / 97 / 113 | 17.27 | 38 / 37 / 38 / 38 / 39 | 6.33 |
| stranded_at | 37 / 35 / 42 / 45 / 35 | 6.47 | 33 / 31 / 39 / 40 / 31 | 5.80 |
| unwound_at | 16 / 22 / 30 / 39 / 36 | 4.77 | 6 / 4 / 7 / 9 / 7 | 1.10 |
| chatted_with | 14 / 12 / 14 / 20 / 16 | 2.53 | 12 / 12 / 14 / 20 / 16 | 2.47 |
| idled_with | 7 / 7 / 18 / 8 / 13 | 1.77 | 7 / 7 / 16 / 8 / 13 | 1.70 |
| talked_shop_with | 10 / 20 / 10 / 8 / 0 | 1.60 | 10 / 20 / 10 / 8 / 0 | 1.60 |
| commiserated_with | 6 / 6 / 6 / 14 / 14 | 1.53 | 6 / 6 / 6 / 14 / 12 | 1.47 |
| slighted_by | 5 / 3 / 8 / 4 / 8 | 0.93 | 5 / 3 / 8 / 4 / 6 | 0.87 |
| complained_at_by | 7 / 6 / 3 / 5 / 5 | 0.87 | 7 / 5 / 2 / 5 / 5 | 0.80 |
| laughed_with | 0 / 6 / 2 / 10 / 8 | 0.87 | 0 / 6 / 2 / 10 / 6 | 0.80 |
| thanked_by | 4 / 3 / 4 / 4 / 2 | 0.57 | 4 / 3 / 4 / 4 / 2 | 0.57 |
| complimented_by | 2 / 5 / 2 / 2 / 1 | 0.40 | 2 / 4 / 2 / 2 / 1 | 0.37 |
| heard_well_of | 1 / 0 / 0 / 1 / 0 | 0.07 | 1 / 0 / 0 / 1 / 0 | 0.07 |
| insulted_by | 1 / 0 / 0 / 0 / 0 | 0.03 | 1 / 0 / 0 / 0 / 0 | 0.03 |
| swept_here | 0 / 0 / 1 / 0 / 0 | 0.03 | 0 / 0 / 1 / 0 / 0 | 0.03 |

`chains`:

| kind | fired | fired per 1000 | formed | formed per 1000 |
|---|---|---|---|---|
| stranded_at | 82 / 84 / 108 / 111 / 87 | 15.73 | 19 / 27 / 47 / 45 / 20 | 5.27 |
| settled_in | 71 / 70 / 66 / 48 / 68 | 10.77 | 12 / 15 / 17 / 19 / 14 | 2.57 |
| chatted_with | 26 / 22 / 16 / 28 / 26 | 3.93 | 22 / 20 / 14 / 26 / 20 | 3.40 |
| talked_shop_with | 30 / 20 / 12 / 10 / 18 | 3.00 | 28 / 18 / 12 / 10 / 18 | 2.87 |
| commiserated_with | 14 / 14 / 24 / 14 / 18 | 2.80 | 6 / 4 / 10 / 12 / 10 | 1.40 |
| ran_down | 15 / 15 / 15 / 15 / 15 | 2.50 | 15 / 15 / 15 / 15 / 15 | 2.50 |
| complained_at_by | 5 / 10 / 14 / 19 / 9 | 1.90 | 4 / 7 / 11 / 15 / 9 | 1.53 |
| laughed_with | 16 / 10 / 4 / 16 / 8 | 1.80 | 14 / 8 / 2 / 12 / 6 | 1.40 |
| slighted_by | 3 / 6 / 9 / 7 / 9 | 1.13 | 3 / 6 / 8 / 6 / 7 | 1.00 |
| jammed_here | 4 / 4 / 5 / 13 / 7 | 1.10 | 1 / 1 / 2 / 3 / 1 | 0.27 |
| thanked_by | 3 / 9 / 7 / 5 / 5 | 0.97 | 2 / 6 / 5 / 3 / 4 | 0.67 |
| complimented_by | 3 / 4 / 5 / 4 / 6 | 0.73 | 1 / 2 / 3 / 3 / 3 | 0.40 |
| saw_sabotage | 0 / 15 / 0 / 0 / 0 | 0.50 | 0 / 5 / 0 / 0 / 0 | 0.17 |
| insulted_by | 1 / 1 / 3 / 2 / 0 | 0.23 | 1 / 1 / 3 / 2 / 0 | 0.23 |

`bench-economy` fires `idled_with`, `unwound_at`, `heard_well_of` and `swept_here`, which `chains` does not; `chains` fires `ran_down`, `jammed_here` and `saw_sabotage`, which `bench-economy` does not.

### Kinds that never fired (either template)

Read from where each is written in `crates/engine/src`.

| kind | where it is remembered | why it is zero here |
|---|---|---|
| `bonded_in_battle`, `hard_won` | `game/memories.rs` (`form_victory_memories`, after a won fight) | needs a battle; there is none in a base-only run |
| `mauled_by` | `game/memories.rs` (`note_maul`) | a landed hit in a battle |
| `lost_in_battle`, `let_go`, `became_part_of` | `bonds.rs` `Departure::grief_def`, via `note_departure` | a friend leaves (killed, sold or spent, fused); nobody leaves in a run |
| `rid_of` | `bonds.rs` `RELIEF_DEF`, same path | a rival or enemy leaves; same |
| `vented`, `turned_on_me`, `saw_turn_on` | `game/base/tantrum.rs` | a tantrum; both templates have 0 tantrums |
| `heard_ill_of` | `interactions` follow-up of `turned_on_me` and the gossip pass | there is no `turned_on_me` to pass on; gossip of a negative opinion did not happen in 30,000 ticks of either template |
| `siphoned` | `game/memories.rs` (`note_siphoned`) | a program must hold a `Siphoned` component, which is a siege effect; sieges are off |
| `unslotted` | `game/memories.rs` (`note_unslotted`) | roster size over capacity; neither base is over |
| `cutting_rock` | `game/memories.rs` (`note_postings`) | a worker on an Excavate task; no excavation is queued |
| `at_ease_on` | `game/memories.rs` (`note_comforts`) | a worker standing on a floor finish whose `comfort` names it (`cobalt_carpet`, `moss_weave`); neither template's floor has one |
| `frayed_here` | `game/base/offshift.rs` | needs a worker that cannot reach any restorer (`unreachable`); on `chains` the 15 frays all took the `ran_down` branch instead (no amenity at all) |
| `idled_with` (`chains` only) | `game/base/offshift.rs` | an off-shift idle pair meeting at an amenity; `chains` has none |
| `unwound_at` (`chains` only) | `game/memories.rs` (`note_respites`) | a respite at an amenity; `chains` has none |

## What moved and what did not

- **Moved, `bench-economy`:** morale spans five bands in sixths of the
  staff-ticks (`bitter` is the only one empty), the spread is 7-10, and 35-39
  relationships form per run, all by tick 6,000.
- **Moved, `chains`:** the no-amenity base is the bitter-band contrast: 19%
  `bitter` (10-34% by seed), never `devoted`, spread 10-16, and 15 frays a
  run, each leaving a `ran_down` memory.
- **Did not move:** every bond above Rival-or-Friend (Close and Enemy are 0 on
  all ten runs), the whole battle/tantrum/siege/departure family, and the
  comfort memory.
- **`settled_in` and `stranded_at` dominate** (17 and 6.5 per 1000 on
  `bench-economy`, 11 and 16 on `chains`) because they are posting-state
  writes, repeated every `MEMORY_POSTING_PERIOD`; most are reinforcements,
  not new memories (formed is 6.3 and 5.8 per 1000 on `bench-economy`).
  Conversation kinds are almost all new (fired is within 10% of formed).

## Suspicious

- **Zero Close or Enemy on every seed, no Friend on `bench-economy`.** A knob
  on bond-shaping memories has nothing to move against Close or Enemy within
  this window, and the Friend target is out of reach on `bench-economy`. The
  `BOND_*_AT` thresholds are code (out of scope), so only the memory valence
  knobs could change it.
- **`chains` frays are exactly 15 on every seed.** Not a bug read from code,
  but a ceiling: every program frays once by tick 6,000 and `ran_down` is
  15 (fired and formed) on every seed, so the fray count carries no spread on
  this template at 6,000 ticks. 3,000 ticks (3-4 frays) was the informative
  length.
- **`saw_sabotage` is 15 fired / 5 formed on seed 2 only**, so one sabotage
  event has all 15 programs witness it; it did not occur on the other four
  seeds. A single-seed event is not a rate.
- **`talked_shop_with` is 0 on `bench-economy` seed 5** while the other
  seeds make 8-20, and `heard_well_of` is 0-1; low counts, so per-seed
  differences here are mostly noise.
- **`relationships` is a snapshot at tick 6,000,** so a relationship dropped earlier is not counted.

## What it does not say

- **Two templates, five seeds, one set of orders, sieges off.** Nothing
  here says what battles, tantrums, siphons or departures do to memory; those
  kinds are unmeasured, not shown inert.
- **Per-kind rates are whole-base counts over 6,000 ticks;** at 0.03-0.07 per
  1000 (`insulted_by`, `swept_here`, `heard_well_of` on `bench-economy`) a kind
  fires 1-2 times in thirty thousand ticks and its rate is not distinguishable
  from zero.
- **Bonds are the last tick only,** per the plan, and the report cannot say
  how long a Rival or Friend held.
- **Per-program memory content is not reported.** `fired` counts a
  reinforcement the same as a first strike; `formed` is the share that was new.
- **No target range is proposed;** that is the next decision.

## Open questions

- Whether any bond above Neutral can be reached with sieges off on
  `bench-economy`, or whether the Friend/Close target ranges must be set from
  `chains` or a longer run.
- Whether a floor finish with a `comfort` belongs in `bench-economy`, so
  `at_ease_on` can be measured at all.
