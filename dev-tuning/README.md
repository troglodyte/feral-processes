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

## What rejects a candidate outright

Not everything is a preference the score can trade against. A roster where
no species is `beatable_by_a_fresh_player` empties the opening ring — and
`habitat_pools` falls back to the biome's *unfiltered* roster when nothing
qualifies, so that failure looks intact from the outside. A roster where
*every* species is beatable has no upper half left. Both are thrown out
before any fight runs.

Boss-per-biome coverage is deliberately unchecked: the tuner never touches
`habitats` or `is_boss`, so it cannot break that census.

## Two things to know before trusting a number

**Specials never fire.** The headless arena plays the game's own All-Attack
every round, so no companion Special ever goes off and ability magnitudes go
entirely unmeasured. Every number is a *floor* on what a real party outputs.
Set targets knowing that, and play a proposal before applying it —
`FERAL_DEV_ARENA=1 cargo run`, `[R]`, `[L]` the scenario. A converged tuner
run is not evidence of play.

**The objective does not know which side you meant.** A target says "this
fight should be won 75% of the time"; it does not say whether to get there
by buffing the enemy or by nerfing the party. The first real run did both —
it raised `rootkit` (the opponent in `full-group.ron`) *and* dropped
`scrapper.base_def` to its floor, and Scrappers are the party in that
scenario. A stat lowered to satisfy one fight applies to that species
everywhere in the game. Read the Fields-moved list with that in mind; it is
exactly the kind of thing the human-review step exists to catch.

## Current state

Measured against the shipped roster:

| scenario | want | shipped |
|---|---|---|
| opening-fight | 92% | 100% |
| full-group | 75% | 100%, at 98.9% HP left |
| stack-depth-5 | 55% | 0%, in 2.5 rounds at 0% HP |

A geared zone-3 party clears a full enemy group having taken about 1%
damage, and is then erased at depth 5. `stack-depth-5.ron` was written to
retest the standing note that depth 5 may be unwinnable: that note blamed a
fixture with no gear, and this one has full gear and three Scrappers and
still loses 50 out of 50. Whether roster stats alone can fix it is not
settled — a 60-iteration random search made no progress there, which is
weak evidence either way.
