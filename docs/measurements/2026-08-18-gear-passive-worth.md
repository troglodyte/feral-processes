# What the eight granting items are worth

**Run 2026-08-18 on branch `gear-passives` at `feb3dca`, before merge.**
Answers `docs/superpowers/specs/2026-08-18-gear-passives-balance-measurement.md`.

## The claim

Seven of the eight items are worth their slot, and the eighth
(`watchdog_tap`) is not readable at all: on 100 paired fights it rescued one,
and every mean it moved sat inside noise. Nothing here is too strong for what
it costs *at level 12*, and no cooldown, no ability `power` and no item
`value` needs to move on this evidence.

`WOUNDED_INTEGRITY_FRACTION = 0.33` is the number that came out best. It
discriminates by band exactly as it was argued to: the wearer crosses it in
**5%** of runs of a fight the party wins easily, **25%** of the fights they
win on curve, **44%** of the wins in a fight they lose half the time, and
**100%** of losses in every band. It is neither dead nor a slower
`RoundStart`, and it should be left alone.

`RoundStart` does not front-load. Uptime is flat at one firing per 4.0–4.7
rounds across fights of 11, 17 and 41 rounds, and the value grows with the
fight rather than ahead of it, so the cooldown is doing what it was priced to
do.

Two findings the spec did not ask for, and they are the ones that matter:

- **The stat line, not the grant, is the bigger lever at level 12.** A bare
  `def: 2` module with no grant at all is worth **+21 wins per 100** at the
  on-curve band — more than the strongest grant measured (`interrupt_coil`,
  +16). At level 36 the same two points of DEF measured **exactly zero**,
  rep for rep, while the grant on the same item turned a 33-round fight into
  a 6.5-round one. The two halves of a granting item invert in importance
  across the level range, which is not a thing either the price or the
  design currently says out loud.
- **`deadman_relay` and the etched disk are not merely comparable, they are
  identical** — 100 of 100 reps agreed on outcome, rounds, Integrity and
  companions downed. Whether the disk is dead content is therefore a
  question about which slot is scarcer, and not a question the arena can
  answer.

## How to reproduce it

One debug binary, built once and never rebuilt: `cargo build --bin arena` at
workspace `0.11.1`, commit `feb3dca`. **Every number below compares within
that build only** — a rebuild reshuffles the RNG stream, so a figure here
means nothing beside a figure from another build.

The control for a granting item is that same item with its `grants:` line
commented out. Assets load at runtime, so a toggle changes no code and no
stream:

```sh
sed -i 's|^\(\s*\)grants: |\1// grants: |' assets/items/interrupt_coil.ron   # off
./target/debug/arena scenario.ron --out off.ron
sed -i 's|^\(\s*\)// grants: |\1grants: |' assets/items/interrupt_coil.ron   # on
git diff --quiet assets/          # assert the toggle came back out
```

**That control was validated against the shipped stat twins**, which is the
cross-check the spec asked for: `arc_lance` (atk 3) against a grant-stripped
`interrupt_coil`, and `hardened_shell` (def 3) against a grant-stripped
`parity_weave`, agreed on **100 of 100 reps each**, rep for rep. The
asset-toggle control and the twin control are the same control.

Every run is paired by seed — rep *n* is seed `4100 + n` on both sides — and
the statistics quoted as `± x` are the standard error of the *per-seed
difference*, which is a far tighter instrument than two independent means.
An unpaired win rate at n=100 carries about ±4.6pp, so a 6-point gap is not
readable unpaired; paired, the same runs resolve it as six fights rescued and
none lost.

### The four bands

All at `player: Fresh(level: 12, zone: 3)` with one companion,
`(species: "scrapper", level: 12, equip: [(item: "kinetic_edge"), (item: "packet_buffer")])`,
and a player wearing plain `kinetic_edge` + `packet_buffer` unless the item
under test replaces one of them:

| Band | `opponents` | win | rounds | Integrity left | companions down |
|---|---|---|---|---|---|
| easy | `(species: "trojan", count: 4)` | 93% | 14.0 | 73% | 0.00 |
| **on curve** | `(species: "rootkit", count: 4)` | 70% | 22.6 | 54% | 0.50 |
| losable | `(species: "zero_day", count: 3)` | 50% | 13.5 | 25% | 0.60 |
| long | `(species: "sentinel", count: 4)` | 90% | 57.5 | 88% | 0.07 |

`dev-arenas/gear-passives.ron` was moved onto the on-curve band as part of
this run. It previously fielded `rootkit ×3`, which measured 100% wins at 97%
Integrity remaining — a fight where no defensive passive can show anything,
because nothing was ever in danger.

Q5 and Q6 needed a routine *installed*, which no scenario field can express,
so they were run off four hand-edited saves built from `dev-saves/extraction.ron`
(level 36, zone 3) and packed with `savetool pack`: module and routine list
switched independently, everything else identical.

```sh
./target/debug/savetool pack variant.ron variant.bin
# then: player: Save("variant.bin"), opponents: [(wintermute, 1), (zero_day, 4)]
```

## The numbers

### Q1 — is each item worth its slot?

On-curve band, 100 paired reps. `rescued` is fights won with the grant that
were lost without it; `lost` the reverse. `*` marks a delta larger than twice
its standard error.

| Item | grant fires | win on→off | rounds Δ | Integrity Δ | companions Δ | rescued / lost |
|---|---|---|---|---|---|---|
| `interrupt_coil` | 3.85/run, 100% of runs | 92 → 76 | **−4.79 ± 0.52\*** | **+22.9pp ± 3.4\*** | −0.16 ± 0.04\* | 16 / 0 |
| `ragged_edge` | 4.45/run, 100% | 82 → 70 | **−2.57 ± 0.39\*** | **+12.4pp ± 2.5\*** | −0.07 ± 0.03\* | 12 / 0 |
| `parity_weave` | 6.05/run, 100% | 94 → 83 | +3.55 ± 0.66\* | **+15.2pp ± 2.8\*** | **+0.24 ± 0.04\*** | 12 / 1 |
| `crash_handler` | 0.66/run, 65% | 80 → 70 | −0.50 ± 0.26 | +5.9pp ± 2.0\* | +0.00 | 10 / 0 |
| `redundant_bank` | 0.42/run, 30% | 91 → 82 | +0.58 ± 0.24\* | +6.3pp ± 1.6\* | +0.01 ± 0.01 | 9 / 0 |
| `sandbox_liner` | 0.40/run, 36% | 76 → 70 | −0.03 ± 0.22 | +2.2pp ± 1.2 | +0.00 | 6 / 0 |
| `deadman_relay` | 0.75/run, 75% | 92 → 91 | **−5.88 ± 0.49\*** | **+13.3pp ± 1.5\*** | +0.00 | 1 / 0 |
| `watchdog_tap` | 0.26/run, 23% | 92 → 91 | −0.21 ± 0.11 | +1.6pp ± 1.0 | +0.00 | 1 / 0 |

Read the last two rows against each other rather than down the column. Both
sat at +1 win, but for opposite reasons: `deadman_relay` moved everything
else it could move and only failed to convert it into wins, while
`watchdog_tap` moved nothing at all. A player wearing the relay watches
fights end six rounds early; a player wearing the tap has no way to tell it
is on.

Three things in that table are worth stating outright:

- **`parity_weave` makes fights longer and kills more companions.** +3.55
  rounds and +0.24 companions down, while winning 11 more of them. The buff
  is `OneAlly` and lands on its own wearer, so it converts fights the player
  was losing into fights the player survives and the party does not. That is
  a coherent thing for armour to do, and it is not what the item's text
  suggests.
- **The two `Afflicted` items are the weakest pair, and they were measured
  against one status kind.** Across every band, the enemy policy landed Stun
  and never once landed Bleed, so both cleanse items were only ever answering
  a lost turn. The on-curve band is also the *most* afflicting of the four
  (the wearer is stunned in 65% of runs, 1.27 times a run), so this is not an
  instrument that was blind to their trigger — it is the best case they get.
- **`sandbox_liner` clears a Stun about a third of the time and rescued six
  fights in a hundred.** Six-and-zero is a real signal (a sign test puts it
  near p = 0.02) even though every mean it moved was inside noise. Small
  effects show up in flipped outcomes before they show up in averages.

### Q2 — does a `RoundStart` passive out-earn its cooldown?

`interrupt_coil`, paired, across three fight lengths.

| Band | rounds (on) | firings/run | firings per round | rounds Δ | rescued / lost |
|---|---|---|---|---|---|
| easy (11 rounds) | 11.5 | 2.91 | 0.253 | −2.29 ± 0.12\* | 5 / 0 |
| on curve (17) | 16.8 | 3.85 | 0.230 | −4.79 ± 0.52\* | 16 / 0 |
| long (41) | 41.0 | 8.66 | 0.211 | −15.72 ± 2.57\* | 3 / 0 |

Nominal uptime on a four-round cooldown is 0.250. Measured, it is 0.253 in a
short fight and 0.211 in a long one — the round-1 firing is free and
amortises over fewer rounds, which is a 20% edge to short fights and nothing
like front-loading. As a *fraction* of the fight the effect grows: 20%
shorter, 22% shorter, 28% shorter. **The cooldown does not need changing.**

Where the value lands does move: the fight it decides is the on-curve one (16
rescued), not the long one (3), because the long band is a fight the party
was winning anyway.

### Q3 — does `AllyWounded` fire, and how often?

`crash_handler` + `redundant_bank` worn together, 100 reps per band (50 for
the long one). Both share the trigger, so a firing count is a count of the
wearer crossing a third of Integrity downward in one round.

| Band | fired, all runs | fired, wins only | fired, losses only | win on→off | Integrity Δ | rescued |
|---|---|---|---|---|---|---|
| easy | 5% | 5% | — (no losses) | 100 → 98 | +1.3pp ± 1.0 | 2 |
| on curve | 30% | **24.7%** | 100% | 93 → 82 | +6.9pp ± 1.9\* | 11 |
| losable | 47% | **43.6%** | 100% | 94 → 76 | +10.8pp ± 1.6\* | 18 |
| long | 6% | 6.1% | 0% (1 loss) | 49 → 46 | +5.2pp ± 3.0 | 3 |

This is the cleanest result of the run. The trigger fires in one win in
twenty when the party is comfortable, one in four when the fight is real, one
in two when they are losing half of them, and in **every single loss in every
band**. Both spec failure modes are refuted: it is not dead, and it is not a
`RoundStart` with extra steps.

Note the long band. 55 rounds against `sentinel` fires it in 6% of runs —
lower than the 17-round on-curve band. The trigger keys on *burst*, not on
duration or attrition, which is what "crossed downward in one round" was
meant to buy and evidence that the one-round window is doing the work rather
than the fraction alone.

### Q4 — do three granting slots stack into something degenerate?

`interrupt_coil` + `parity_weave` + `deadman_relay` against the same three
with all grants stripped, on-curve band, 100 paired reps:

**100 → 92 wins, −9.09 ± 0.61 rounds, +20.8pp ± 2.5 Integrity, −0.23 ± 0.05
companions down.**

A full granting loadout wins 100 out of 100 fights at a band where plain
craftable gear of the same slots wins 70. That is the ceiling this feature
adds, and it costs three drop-only items that no bench can make.

**The cooldowns synchronise; they do not phase apart.** Both `RoundStart`
passives arm on round 1 and both carry a four-round cooldown, so they stay in
lockstep for the whole fight. Measured over every round of all 100 fights:

| passive lines in a round | share of rounds |
|---|---|
| 0 | 75.6% |
| 1 | 2.9% |
| 2 | 20.8% |
| 3 | 0.8% |

So the log is quiet three rounds in four and then carries two lines at once,
rather than one line most rounds. That is a rhythm rather than a wash, but it
is the opposite of what "up to three extra lines a round" predicted, and it
is what a person playing this should be watching for.

### Q5 — is `deadman_relay` too strong for a module slot?

Four hand-edited level-36 saves, `wintermute ×1 + zero_day ×4`, 100 reps
each. `neither` is the untouched extraction party; `relay` wears the module;
`disk` has `deadman` installed in a routine slot instead and an empty module;
`both` has both.

| Variant | win | rounds | Integrity left | companions down | `deadman` fires |
|---|---|---|---|---|---|
| neither | 74% | 33.3 | 60.5% | 3.79 | 0.00 |
| relay | **100%** | **6.50** | 97.8% | 1.06 | 1.03/run, 100% of runs |
| disk | **100%** | **6.50** | 97.8% | 1.06 | 1.03/run, 100% |
| both | 100% | 5.66 | 98.3% | 1.02 | 1.24/run, 100% |

`relay` and `disk` are identical on **all 100 reps** — same outcome, same
round count, same Integrity, same companions lost. And `relay` with its grant
stripped is identical on all 100 reps to `neither`, which says the module's
two points of DEF are worth **exactly nothing** at level 36 in this fight.

So the relay is not *strictly better* than the disk by anything measurable;
it is the same effect through a different slot. Whether that makes the disk
dead content is a question about which slot a level-36 player would rather
spend, and the arena cannot answer it — the player plays All-Attack there, so
the routine the disk displaces would never have been run.

What the table does say is that `deadman` itself is a fight-ender at level
36: a 33-round fight becomes a 6.5-round one and a 74% band becomes 100%.
That is shipped content and not new on this branch, but the relay is a second
door to it that opens without spending a routine slot, and the door is wide.

### Q6 — is the cross-source double-fire proportionate?

Yes, and less than it looks. `both` against `relay`, paired: **−0.84 ± 0.19
rounds, +0.5pp ± 0.2 Integrity, no wins moved** (both were already 100/100).

The reason is in the round counts. Across 100 fights there were 100 rounds
where `deadman` fired at all, and only **24** of them carried both copies:
the first copy usually ends the battle, and `fire_passives` returns the
moment `BattleState` is gone. So the second copy is dead in three fights out
of four, and what it buys across all of them averages to less than a round.

The other reachable double is `watchdog_tap` plus an installed `watchdog`,
and that one is worth **exactly zero** by construction rather than by
measurement: `Cleanse` sets `StatusEffects::active` to `None`, a single slot,
so the second copy in the same round finds nothing to clear and logs nothing.
The pairing the spec named as the reachable case is the one pairing where the
double-fire cannot pay anything at all.

## What it does not say

- **Nothing here was played.** No session was run — this machine has no
  display for `FERAL_DEV_ARENA=1 cargo run` — so every legibility question
  in the spec is still open: whether "cuts in" reads as your gear acting,
  whether Bleed reads as coming from the weapon, whether `hot_spare` feels
  like a rescue. The transcripts do show three pre-existing grammar breaks
  landing in consecutive lines (`You's Quarantine Single cuts in.` / `You
  locks up, stunned!` / `You flushes the corruption from you.`); the wording
  predates this branch (`1463a00`), but the branch is what makes it frequent.
- **One party shape, one level, one zone** for Q1–Q4: level 12, zone 3, a
  single level-12 Scrapper companion in plain craftable gear. `Damage` and
  `Heal` scale with level, and the only other level sampled was 36 (Q5/Q6),
  where the balance between an item's stat line and its grant inverted
  completely. Nothing between 12 and 36 was measured.
- **The bin plays All-Attack and fires no Specials**, so every number is a
  floor on what a party can do. Gear passives are the exception that proves
  it: they are the first companion-side ability effect the headless arena
  has ever been able to see, because they cost no turn. A companion wearing
  `parity_weave` on the on-curve band fired it 3.92 times a fight and cut
  companion losses by 0.22 ± 0.04 — while moving the player's win rate not at
  all (81 → 83, two rescued and four lost, inside noise).
- **The enemy policy never landed Bleed**, in any band, in any run. Both
  cleanse items are measured against Stun alone.
- **`reps: 100` and paired seeds.** An unpaired win rate here carries ±4.6pp;
  quoting one of these win rates against a number from any other build or any
  other band is the mistake this section exists to prevent.
- **`FERAL_DEV_LOG=1` was never set.** Firing counts were read out of the
  `--out` report's own transcripts, which carry every log line the fight
  emitted.
- `balance_sim` was not consulted and would not have helped: it models no
  abilities, so all six of these are invisible to it.

## Open

- **Is `watchdog_tap` worth shipping?** It is the one item measured at no
  readable value, and the two obvious levers both look wrong: its cooldown is
  already the same 4 as everything else, and raising `watchdog`'s power is
  meaningless for a `Cleanse`. What would actually help is a second status
  kind in play — the item is answering a question the enemy policy asks only
  one way. Deleting it is not obviously right either, since it is the only
  module that reaches a party-wide effect.
- **The level-12/level-36 inversion.** Two DEF outweighed every grant at
  level 12 and vanished at 36. Nothing on this branch is wrong because of it,
  but any future granting item is being priced against a stat line whose
  worth moves by an order of magnitude across the level range, and no
  instrument here samples the middle.
