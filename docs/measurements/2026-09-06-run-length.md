# How many ticks a run actually reaches

**Date:** 2026-09-06
**Build:** `v0.13.116` (`85a39722`), before any settlement-growth code exists.
**Question:** an ambient world clock is about to be added — a `Server` grows
into a `Mainframe` at a seed-derived tick somewhere in
`[SETTLEMENT_GROWTH_DUE_MIN, SETTLEMENT_GROWTH_DUE_MAX]`. Those two numbers
can make the feature ship dead in either direction. Too small and every town
in the world is already grown by the time the player can reach one, so the
trade accelerant is decorative and growth is weightless. Too large and the
ambient half never fires inside a real session, so the only growth anyone
ever sees is growth they paid for. **What span can that clock occupy?**

## The claim

A mature run reaches **at least ~5,300–6,900 ticks**. That is a floor, not an
estimate. On that anchor, `SETTLEMENT_GROWTH_DUE_MIN = 3000` and
`SETTLEMENT_GROWTH_DUE_MAX = 12000`: roughly a **third** of a world's towns
are past their date on the clock alone in a mature run, and the rest grow
only if the player trades them forward.

## What was read

Every shipped dev-save template, which is every artifact in this repo that
records a tick a run actually reached:

```bash
grep -o 'tick: *[0-9]*' dev-saves/*.ron | sort -u
```

| Template | Tick |
| --- | --- |
| `dev-saves/contracts.ron` | 6944 |
| `dev-saves/deep-lair.ron` | 6363 |
| `dev-saves/settlements.ron` | 5422 |
| `dev-saves/extraction.ron` | 5350 |
| `dev-saves/chains.ron` | 5344 |
| `dev-saves/rarity-preview.ron` | 5344 |
| `dev-saves/stack.ron` | 5344 |

(`extraction.ron` and `settlements.ron` each also carry a `tick: 5000`, and
`settlements.ron` an empty one; those are nested fields, not the run clock.)

## Why this is a floor and not an estimate

**These templates were driven, not played.** Each is produced by an
engine-side test that raises a base, tames programs and tops up research
directly, then `savetool capture` names the result.
[`dev-saves/README.md`](../../dev-saves/README.md) says it of the `extraction`
capture in its own words: a save mutated by real doors, not a save produced by
playing. A
driver spends no ticks walking across a map, resting, reading a screen,
losing a fight and coming back, or standing in a town deciding what to buy.
A player spends most of a session on exactly those.

So 5,300–6,900 is **the tick count at which a run has the state a mature run
has**, reached by the shortest possible path to it. A real session that
arrives at the same state arrives later — how much later is unknown and is
the open question below. Anyone quoting these figures as "a run is about
6,000 ticks" is quoting them past their range.

## The cadence comparison

Growth has to feel like the world changing rather than like one more thing
that comes around. So it must be several multiples of the recurring cadences
the player already lives with:

| Constant | Ticks | Kind |
| --- | --- | --- |
| `SETTLEMENT_GIFT_COOLDOWN_TICKS` | 9000 | one-shot gate, per town |
| `SETTLEMENT_BOARD_ROTATION_TICKS` | 1800 | recurring rotation |
| `SORTIE_BOARD_ROTATION_TICKS` | 1200 | recurring rotation |
| `SETTLEMENT_MARKET_ROTATION_TICKS` | 900 | recurring rotation |
| `CARAVAN_VISIT_INTERVAL_TICKS` | 900 | recurring rotation |

**The longest tick-valued constant in `tuning.rs` is
`SETTLEMENT_GIFT_COOLDOWN_TICKS` at 9,000, but it is not the comparison that
matters here.** A gift cooldown is a gate on repeating one action, not a
cadence the world runs on; nothing rotates every 9,000 ticks. The class
growth could be mistaken for is the recurring one, and **the longest
recurring cadence shipped is `SETTLEMENT_BOARD_ROTATION_TICKS` at 1,800.**

That 9,000 is worth recording anyway, because it sets a scale: with
`_MIN = 3000` a player can watch a town grow before they can gift it twice.
That is acceptable — growth is the rarer event of the two in the upper half
of the span — but it is a consequence, not an accident, and a later reader
comparing 3,000 against 9,000 should find it already noticed here.

## The conclusion

**`SETTLEMENT_GROWTH_DUE_MIN = 3000`** — below the floor at which a run
matures, so the earliest-dated towns in a world grow inside a run that never
trades with them. Growth has to be reachable ambiently or the feature is
purely a trade reward. Still **1.7x** the longest recurring cadence, so it
cannot read as one more rotation.

**`SETTLEMENT_GROWTH_DUE_MAX = 12000`** — roughly **twice** the observed
maturity floor. Towns dated in the upper half of the span do *not* clear
their date within a typical run on the clock alone; they grow only if the
player trades them forward. A span every town cleared on time would make
`SETTLEMENT_COMMERCE_PULL_TICKS` decorative, which is the failure this number
exists to prevent.

Together: in a mature ~6,000-tick run, **roughly a third** of a world's towns
are past their date ambiently. That fraction is the claim a playtest should
challenge first.

## What it does not say

- **It does not say how long a session is.** It says how long a *driven* run
  to maturity is. The gap between those is the whole uncertainty here.
- **It measures nothing about growth itself.** No growth code exists yet.
  Whether an `s` turning into an `M` reads as an event at all is a question
  for a keyboard, and nobody in the session that wrote this can run the game.
- **It is a reading of artifacts, not an instrument run.** Strictly, this
  entry fails the second criterion in [the index](README.md) — the data *is*
  in the repo and the grep above re-runs in a second. It is here because the
  third criterion holds hard: two constants quote this file for their
  derivation, and that reasoning is not recoverable from the numbers alone.

## Open question

**How many ticks does a real play session reach?** One saved game from one
real session settles it. If sessions turn out to run 15,000+ ticks, `_MAX` is
too low and every town in the world grows on the clock, so the trade
accelerant never decides anything. If they run under 3,000, nothing ever
grows ambiently and half the feature is unreachable. Both failures look
identical from inside a green test suite.
