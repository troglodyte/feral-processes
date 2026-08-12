# dev-tuning

Tuning species stats by measurement rather than by feel.

```sh
cargo run --release --bin tuner -- dev-tuning/objective.ron --measure   # score the shipped roster
cargo run --release --bin tuner -- dev-tuning/objective.ron             # search, write a proposal
cargo run --release --bin tuner -- dev-tuning/objective.ron --out /tmp/p
```

`--release` matters: the search fights thousands of battles, and a debug
build turns ninety seconds into an afternoon.

## What it does

`objective.ron` names scenarios in `dev-arenas/` and says what each should
measure — a win rate and how much HP the player should have left. The tuner
hill-climbs species stats, fighting every candidate in the real arena, and
writes the best roster it found to `dev-tuning/out/`.

**The output is a proposal, never an edit.** Nothing writes into `assets/`.
Read `out/report.md`, `diff -r assets/species dev-tuning/out/species`, and
apply what you agree with by hand. `out/` is gitignored: a proposal is a
measurement, not source.

A run is reproducible. Same objective and same `search_seed` give the same
proposal, because the whole thing has to be reviewable and "run it again"
has to produce the same diff.

## What it may move

Six numeric fields per species: `base_hp`, `base_atk`, `base_def`,
`base_speed`, `taming_difficulty`, `growth_multiplier`. Not `habitats`, not
`is_boss`, not `moves` — those are what a species *is*, and only what it is
*worth* is a tuning question. Item values and `tuning.rs` are untouched, so
the craftable-price ceiling and the deliberate data/code line for difficulty
both stand.

Every field needs a `Bound`. An unbounded search proposes a 4000-HP drone
that satisfies one target by breaking something no target happened to
measure, so a missing bound is an error rather than an implicit free rein.

It may not move a species the player *fields*. Any species named in a
target scenario's `party` is left out of the candidate entirely, so the
search cannot reach it and the report lists it under Frozen — see the
two-sided note below. A scenario whose player comes from a `Save` or a
`Template` is refused for the same reason: `party` is a `Fresh`-only field,
so that save's companions would be invisible here and nerfable exactly as
before.

## What rejects a candidate outright

Not everything is a preference the score can trade against. A roster where
no species is `beatable_by_a_fresh_player` empties the opening ring — and
`habitat_pools` falls back to the biome's *unfiltered* roster when nothing
qualifies, so that failure looks intact from the outside. A roster where
*every* species is beatable has no upper half left. Both are thrown out
before any fight runs.

**An ordinary species' stat block is derived, not authored.** Its total is
its growth band's budget times its class's weight, *exactly*, with per-axis
shares to ±1 and a speed band per class — and the class comes from the
species' affinities, which the tuner cannot move. So four more rules apply,
all of them `species::stat_shape_faults` called rather than restated:

- the total is off its budget,
- an axis holds more than a point away from its class's share,
- `base_speed` is outside its class's band,
- `growth_multiplier` sits between the rungs of `GROWTH_TIERS`. The budget
  is a step function, so `1.238` derives a whole stat block from a number
  nobody chose.

**Bosses are exempt from all four**, which is what the shipped census does
and is not a detail: they sit outside the class system, and the one move the
first real search made that was worth having was a boss's ATK.

Boss-per-biome coverage is deliberately unchecked: the tuner never touches
`habitats` or `is_boss`, so it cannot break that census.

## What is reported rather than rejected

Two of the shipped censuses are too expensive or too roster-wide to pay on
every candidate, so they run once on the winner and land in `report.md`
under "Censuses too expensive to reject by":

- **the reach rule** (`balance_sim::reach_rule_verdict`) runs a level
  search per call;
- **the extraction-aptitude spread** (`species::extraction_aptitude_faults`)
  is a property of the whole roster's distribution rather than of any one
  move.

A proposal that breaks one is not thrown out — by the time these run a human
is reading a diff, and that person decides. The report prints the reach
rule's two levels rather than a verdict, because how close it came is the
part that informs the decision.

`report.md` also tallies rejections **by rule**. A bare count says a search
was turned away without saying from what, and the two readings need
different fixes: bounds set wrong is a config edit, while a legal move set
thinner than the search space is a reason to narrow `perturb` so it stops
proposing the illegal in the first place.

## Two things to know before trusting a number

**Specials never fire.** The headless arena plays the game's own All-Attack
every round, so no companion Special ever goes off and ability magnitudes go
entirely unmeasured. Every number is a *floor* on what a real party outputs.
Set targets knowing that, and play a proposal before applying it —
`FERAL_DEV_ARENA=1 cargo run`, `[R]`, `[L]` the scenario. A converged tuner
run is not evidence of play.

**A party with no gear is a second floor, and this one is avoidable.** Any
program the player owns may wear gear, so a scenario whose `party` rows name
no `equip` fields a weaker party than the run it is modelling — and the
search closes the gap by making enemies weaker than they should be. Every
target scenario gears its party as of 2026-08-12. A new one must too.

**The freeze is narrow, because the roster is two-sided.** A target says
"this fight should be won 75% of the time"; it does not say whether to get
there by buffing the enemy or by nerfing the party. The first real run did
both — it raised `rootkit` (the opponent in `full-group.ron`) *and* dropped
`scrapper.base_def` to its floor, and Scrappers are the party in that
scenario. A stat lowered to satisfy one fight applies to that species
everywhere in the game.

Every species the player fields in any target is now held at its shipped
numbers and listed in the report's Frozen section. That set is *derived*
from each scenario's `party`, not configured, so it cannot drift out of step
with the fights it protects. What it does not do is make the objective
two-sided: every species in this game can be tamed, so a scenario's `party`
is only which ones that authored fight happens to field. The fix that would
is coverage — put a species on *both* sides of some pair of targets and
lowering it costs a fight elsewhere, so the search self-corrects with no
frozen set at all.

## Current state

Measured against the shipped roster, 2026-08-12, 200 seeds
(`tuner dev-tuning/objective.ron --measure`):

| scenario | want | shipped | before the retune |
|---|---|---|---|
| opening-fight | 100% / 62% HP | 99.0% / 66.7% | 99.0% / 66.7% |
| full-group | 100% / 90% HP | 100% / 99.9% | 100% / 99.9% |
| lair-on-curve | 55% / 30% HP | 56.0% / 54.9% | 28.5% / 27.6% |

Two of the three are deliberately near what the game already does. The
opening ring is a nursery by design and surface content at zone 3 is
deliberately a victory lap, so those targets are *guards* — what they catch
is the surface getting harder, not the search failing to make it so.

The lair was the lever, and the first search against this objective moved it
from 28.5% to 56% — by lowering `overseer.base_atk` from 17 to 11 and
nothing else. That is now applied. **What is left of the error is almost
entirely the lair's HP band**: the fight is won at 54.9% health against a
want of 30%, so it is winnable but not yet the cost it was specced as.
Whether to chase that is a design call rather than a search problem.

The measured shape of the game behind those targets: an on-curve zone-3
party takes **zero** damage from the largest group zone 3 can field, and
before the retune cleared a depth-2 lair 28.5% of the time. A cliff, not a
curve.

The targets were re-argued on 2026-08-12; before that they still encoded a
game where a zone doubled enemy stats. `stack-depth-5.ron` was a target
until then and is not one now — see its own comment for why, and
`NOTES.md` for what it cost while it was.
