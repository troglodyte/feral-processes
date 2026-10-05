# Base bench: a memories tune on `bench-economy`

Follows the [memories baseline](2026-10-05-base-bench-memories-baseline.md),
whose `bench-economy` rows hold the shipped values used here. This is what
`bench tune dev-tuning/memories.ron` proposes. Nothing was applied; `assets/`
is untouched and the decision is the user's.

## The claim

The search cuts the error from 0.1538 to 0.0121 on the search seeds and from
0.1536 to 0.0516 on the hold-out seeds (the tool's verdict: "holds up"). It
does this by lifting Rival and keeping `uneasy` and `devoted` in range; it does
**not** reach the Friend target (mean 0.040 on search, 0.026 on hold-out
against 0.05-0.20; shipped 0), so most of the remaining error is Friend. The
proposal also costs the staff: 5 points less time on shift, a bitter band that
did not exist, and deeper morale minima (see "Staff, alongside").

## How to reproduce it

```sh
cargo build --release --bin bench
bench tune dev-tuning/memories.ron --out /tmp/memories-out
```

Branch `memories-bench`, release, 16-core machine. `search_seed` 1337, search
seeds 1-3, hold-out 4-6, 6,000 ticks, sieges off, orders as in
`staff.ron`, 22 iterations x 24 candidates, 29 knobs: **17 m 06 s** wall (164
CPU-min). That is over the 15-minute aim; the machine was shared with other
sessions' builds and runs, so part of it may be contention, and I did not
re-time it on an idle machine. The tune is deterministic. The log ended
`proposing the best candidate seen`: the final mean re-scored 0.0252, the best
candidate seen (generation 20) 0.0121. Generation bests wandered 0.0121-0.0711
from generation 16 on rather than settling; generation means stayed at
0.09-0.16.

Per-seed numbers come from `bench run` for seeds 1-6 against the shipped
assets and against a scratch copy of the repo with the proposed `memories/` and
`interactions/` files in `assets/` (the real `assets/` was never edited; the
tool resolves `assets/` from its compile-time path, so the scratch copy is
built separately). The hold-out target means reproduce the tool's report to
three places.

## The numbers

Fitness is the objective's error (squared miss outside each range over the
range width, summed over targets, mean over seeds; lower is better).

| fitness (error) | shipped | proposed |
|---|---|---|
| search seeds 1-3 | 0.1538 | 0.0121 |
| hold-out seeds 4-6 | 0.1536 | 0.0516 |

Targets, mean over seeds (shipped / proposed). "Want" is the range.

| measure | want | search shipped | search proposed | hold-out shipped | hold-out proposed |
|---|---|---|---|---|---|
| memories.bond_share.Friend | 0.05-0.20 | 0 | **0.0396** | 0 | **0.0261** |
| memories.bond_share.Rival | 0.02-0.10 | 0.0093 | 0.0206 | 0.0190 | 0.0628 |
| memories.morale_band_share.uneasy | 0.02-0.10 | 0.0211 | 0.0503 | 0.0357 | 0.0430 |
| memories.morale_band_share.devoted | 0.02-0.10 | 0.0534 | 0.0745 | 0.0677 | 0.0622 |

Bold is outside its range. Per seed, proposed (shipped in brackets):

| seed | Friend | Rival | uneasy | devoted | bitter |
|---|---|---|---|---|---|
| 1 | 0.0227 (0) | 0.0227 (0.0278) | 0.0309 (0.0157) | 0.0679 (0.0321) | 0.0201 (0) |
| 2 | 0.0426 (0) | 0.0213 (0) | 0.0740 (0.0215) | 0.0590 (0.0841) | 0 (0) |
| 3 | 0.0536 (0) | 0.0179 (0) | 0.0459 (0.0261) | 0.0964 (0.0441) | 0 (0) |
| 4 | 0.0256 (0) | 0.0769 (0) | 0.0572 (0.0255) | 0.0461 (0.0462) | 0 (0) |
| 5 | 0.0526 (0) | 0.0263 (0.0571) | 0.0530 (0.0456) | 0.1104 (0.1039) | 0.0055 (0) |
| 6 | 0 (0) | 0.0851 (0) | 0.0187 (0.0359) | 0.0301 (0.0530) | 0 (0) |

Only seeds 3 and 5 put Friend at or over 0.05; seed 6 has none. Live
relationships per run rise from 37 to 49 (search) and 41 (hold-out); Close and
Enemy stay 0 on every seed.

### Staff, alongside

Same runs, mean over seeds (shipped / proposed). No run stopped
(`stopped_at` None on all twelve); frays, tantrums and `downed_tools` are 0
both ways.

| measure | search shipped | search proposed | hold-out shipped | hold-out proposed |
|---|---|---|---|---|
| staff.on_shift_share | 0.930 | 0.878 | 0.904 | 0.876 |
| staff.sulking_share | 0.036 | 0.082 | 0.056 | 0.076 |
| staff.morale_mean | 3.92 | 3.76 | 3.72 | 3.63 |
| staff.morale_min | -14.9 | -22.5 | -16.9 | -20.5 |
| memories.morale_spread | 7.61 | 9.58 | 8.53 | 9.99 |

Seed 1's morale minimum is -30.0 (shipped -15.0). Mean morale does not move;
the distribution widens, which is what `uneasy`, `devoted` and `bitter`
are measuring.

### Knobs

Proposed against shipped, with the bound range. "At bound" means within 1% of
the range width of a limit. Twenty of the 29 are interior.

| file | field | shipped | proposed | range | at bound |
|---|---|---|---|---|---|
| memories/chatted_with.ron | valence | 1.5 | 1.941 | 0.75-2.25 | |
| memories/chatted_with.ron | half_life | 2000 | 1577 | 1000-6000 | |
| memories/talked_shop_with.ron | valence | 2 | 2.038 | 1-3 | |
| memories/talked_shop_with.ron | half_life | 2000 | 5226 | 1000-6000 | |
| memories/laughed_with.ron | valence | 2.5 | 3.75 | 1.25-3.75 | **max** |
| memories/laughed_with.ron | half_life | 2000 | 3755 | 1000-6000 | |
| memories/commiserated_with.ron | valence | 2 | 2.321 | 1-3 | |
| memories/commiserated_with.ron | half_life | 2000 | 1000 | 1000-6000 | **min** |
| memories/idled_with.ron | valence | 4 | 6 | 2-6 | **max** |
| memories/idled_with.ron | half_life | 4000 | 6000 | 1000-6000 | **max** |
| memories/thanked_by.ron | valence | 2 | 1.462 | 1-3 | |
| memories/thanked_by.ron | half_life | 2000 | 1217 | 1000-6000 | |
| memories/complimented_by.ron | valence | 3 | 3.129 | 1.5-4.5 | |
| memories/complimented_by.ron | half_life | 2000 | 3135 | 1000-6000 | |
| memories/slighted_by.ron | valence | -2.5 | -3.75 | -3.75 to -1.25 | **strongest** |
| memories/slighted_by.ron | half_life | 2000 | 2750 | 1000-6000 | |
| memories/complained_at_by.ron | valence | -1.5 | -1.556 | -2.25 to -0.75 | |
| memories/complained_at_by.ron | half_life | 2000 | 1810 | 1000-6000 | |
| memories/insulted_by.ron | valence | -4 | -5.645 | -6 to -2 | |
| memories/insulted_by.ron | half_life | 2000 | 3207 | 1000-6000 | |
| interactions/commiserate.ron | weight | 1 | 0.5 | 0.5-2 | **min** |
| interactions/complain.ron | weight | 1 | 0.5 | 0.5-2 | **min** |
| interactions/compliment.ron | weight | 1 | 0.860 | 0.5-2 | |
| interactions/insult.ron | weight | 0.3 | 0.244 | 0.15-0.6 | |
| interactions/joke.ron | weight | 1.5 | 0.75 | 0.75-3 | **min** |
| interactions/shop_talk.ron | weight | 2 | 1.371 | 1-4 | |
| interactions/slight.ron | weight | 0.6 | 0.934 | 0.3-1.2 | |
| interactions/small_talk.ron | weight | 3 | 1.752 | 1.5-6 | |
| interactions/thanks.ron | weight | 1 | 0.5 | 0.5-2 | **min** |

Nine of 29 knobs sit on a bound: two valences at max (`laughed_with`,
`idled_with`), one at strongest (`slighted_by`), `idled_with` and
`commiserated_with` half-lives, and four interaction weights at min.

### What was in the knob list

The 10 memory kinds (each `valence` and `half_life`) are the user's. Nine
interactions have a listener or speaker memory among them and were included
(`weight`): `small_talk` (chatted_with), `shop_talk` (talked_shop_with),
`joke` (laughed_with), `commiserate` (commiserated_with), `thanks`
(thanked_by), `compliment` (complimented_by), `slight` (slighted_by),
`complain` (complained_at_by), `insult` (insulted_by). **Excluded:** `gossip`,
whose `listener_memory: "chatted_with"` is ignored (the hearsay memory comes
from the speaker's own memory's `spreads_as`). `idled_with` has no
interaction; it is written from the off-shift idle pass, so only its two
memory knobs apply. Every field was present in its file; the load check
refused none, so no knob was dropped.

## Suspicious

- **The Friend target is not met on either seed set.** With valences at their
  ceiling (`laughed_with`, `idled_with`) the search got Friend to 0.04 and
  stopped. The bond thresholds are code (out of scope), so it may be out of
  reach inside these ranges in 6,000 ticks; a wider valence range, or a
  longer run, is the question for the user.
- **Hold-out is 0.0516, not near the search's 0.0121.** Most of the gap is
  Friend (seed 6 has none) and Rival (hold-out 0.063 is fine; search 0.0206
  is just inside 0.02).
- **Seed 1 is not like the others.** It is the only seed with a bitter band
  of note (0.0201) and a morale minimum of -30; the other seeds' bitter is 0
  to 0.0055. The search seed set contains it, so the 0.0121 may be a seed
  1-3 average that hides one program having a bad time.
- **The staff pay for it.** On shift falls from 0.930 to 0.878 (search) and
  0.904 to 0.876 (hold-out), sulking roughly doubles (0.036 to 0.082) though
  it is not a target, and morale minima deepen by 4-15. Frays, tantrums and
  `downed_tools` stay 0, so none of it is the staff-tune's "trouble"; whether
  it is a worse base is a design call.
- **Weights go down, relationships go up.** Four conversation weights are at
  their minimum (and `small_talk`, `joke`, `shop_talk` are down 40-50%), yet
  live pairs rise from 37 to 49. The valences and half-lives are carrying it
  (longer-lived, stronger memories form bonds from fewer conversations);
  the weights at their minimum mean the range may be limiting, but I did not
  test a wider one.
- **The search is noisy.** Generation bests from 16 on are 0.0122, 0.0509,
  0.0422, 0.0332, 0.0121, 0.0141, 0.0427; the winner was found at generation
  20 and not improved on. A different `search_seed` may land elsewhere; it was
  not tried.
- **`uneasy` and `devoted` were already near range.** Shipped `devoted` was
  inside 0.02-0.10 on both seed sets and `uneasy` was inside on hold-out;
  those targets are guards as much as goals.
- **`commiserated_with` half-life at 1000** is the shortest allowed; its
  memory fades twice as fast as shipped while its weight is also at minimum,
  so the search is switching that conversation nearly off.

## Open questions

- Whether to widen the valence ranges (or accept a Friend share near 0.03-0.04)
  rather than ask for 0.05.
- Whether the on-shift cost (about 5 points) and the bitter tail on seed 1
  are acceptable for the bond shares gained.
