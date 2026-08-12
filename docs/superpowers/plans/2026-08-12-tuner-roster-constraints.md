# Teaching the tuner the roster's derived stat budget

Written 2026-08-12, on branch `arena-companion-gear` at `54fbf9d`. Read
`dev-tuning/NOTES.md`'s "The first search against the re-argued objective"
section first — it is the evidence this plan is built on.

## Why

An ordinary species' stat block is **derived, not authored**:

```
total (base_hp + base_atk + base_def) == tier_budget(growth_multiplier) * class weight
```

exactly, with per-axis shares to ±1 and `base_speed` inside a per-class band.
`tier_budget` is a step function of `growth_multiplier` — 50 / 105 / 140.
`species.rs`'s `every_ordinary_species_stat_shape_agrees_with_its_affinity_class`
is what holds it, and the class is derived from the species' *affinities*, so
the census is a cross-check rather than a restatement of the file.

The tuner's six movable fields are precisely the inputs to that formula, and
`roster::perturb` moves them independently. So essentially every proposal it
makes for a non-boss species is invalid by construction. The first real
search proved it: 14 fields moved, four shipped censuses failed, and two
further violations went unreported because the shape census panics on its
first failure.

`constraints.rs` has never known any of this. It checks two beatability
bounds and nothing else.

The one move worth having (`overseer.base_atk 17 -> 11`) was legal, because
the shape census exempts `is_boss`. It is already applied at `54fbf9d`.

## The central design question, to settle before writing code

**Reject illegal candidates, or stop generating them?**

Rejection is what `constraints.rs` does today and it is cheap — it skips the
battles. But if the legal move set is a thin slice of the search space, a
rejecting search spends its whole iteration budget being turned away and
proposes nothing. Measured: the search ran 61 candidates with **0 rejected**,
so today's rejection rate tells us nothing about what it would be with these
rules added.

The alternative is a `perturb` that only offers legal moves — move a boss's
stats freely, move `taming_difficulty` freely, move `base_speed` within its
class band, and for an ordinary species' `base_hp`/`base_atk`/`base_def`/
`growth_multiplier` either leave them alone or move the *budget* and
redistribute by class share so the block stays derived.

Recommendation: implement the constraints first (they are the safety net and
are needed either way), measure the rejection rate over one search, and only
then decide whether `perturb` needs narrowing. Do not build both blind.

## Work

### 1. Extract the budget rules into shipping code — `crates/engine/src/species.rs`

`tier_budget`, `class_of` (returns the `ClassRow` tuple: up/down axis,
weight, hp/atk/def shares, slowest/fastest speed) and `pct` currently live in
`#[cfg(test)] mod tests`, around lines 781-890. They must become shipping
functions so the launcher can call them.

Follow the `Game::creature_class` precedent exactly: it was `#[cfg(test)]`
until the base jobs needed it, and **the censuses now look their row up from
the shipping function**. Do the same here — after the move, the census must
call the extracted function rather than keeping its own copy, or the census
stops being evidence about the game.

They need to be `pub` (not `pub(crate)`) because `crates/launcher` is a
separate crate. `class_of` currently panics on a species with no readable
class; decide whether the shipping form returns `Option` and let the census
keep the panic, or keep panicking and document it. Prefer `Option` — a mod
author's broken species should not abort the tuner.

### 2. Add the rejections — `crates/launcher/src/tuner/constraints.rs`

New `Rejection` variants, each calling the extracted functions rather than
restating any rule. The file's existing doc comment already states this
principle; keep it true.

- **Stat block not on budget** — for `!is_boss` species only. The exemption
  is not an optimisation; it is what the census does, and getting it wrong
  freezes the only species the last search found value in.
- **Axis share out of tolerance** — ±1, same as the census.
- **Speed outside the class band**.
- **Growth multiplier off the base-roster tier** — this is roster-level, not
  per-species (`base_roster_growth_multiplier_rises_with_difficulty_tier`),
  so `check` needs the whole candidate, which it already takes.

Leave `the_reach_rule_measurably_softens_a_full_pack` and
`extraction_aptitude_cuts_across_the_difficulty_ladder` **out** of `check`.
The first runs a level search per call and would be paid on every candidate;
the second is a roster-wide distribution property. Both are better as a
post-check on the winner only — see step 4.

### 3. Tests

In `constraints.rs`, matching the shape of the three already there (they
build a candidate by scaling the shipped roster, which is the right idiom):

- The shipped roster passes all the new rules. This is the one that catches
  an extraction that got the formula subtly wrong.
- A candidate with a boss's stats moved is **accepted** — the regression to
  head off is an over-eager rule freezing the only useful move.
- One candidate per new rejection, each moving exactly the field that rule
  is about.
- Use the real proposal as a fixture if convenient: proposed `rootkit`
  spends 169 points against a budget of 147, and proposed `sprite` spends 40
  where its new growth band demands 101. Both are in this plan's git history
  and in `dev-tuning/NOTES.md`.

Mutation-check each one: delete the rule and watch its test fail. Two tests
written on 2026-08-09 in this repo were vacuous and read as coverage.

### 4. Post-check on the winner — `crates/launcher/src/tuner/run.rs`

Run the two expensive censuses against the proposal once, after the search,
and print the result in `report.md` next to the holdout verdict. A proposal
that breaks one is not rejected — it is reported, because by then a human is
reading a diff and the "no silent caps" rule applies.

### 5. Docs

- `dev-tuning/README.md`'s "What rejects a candidate outright" section is
  currently two rules and must gain these.
- `dev-tuning/NOTES.md`'s defect list: strike the entry this closes.
- `crates/engine/src/species.rs` — the extracted functions need doc comments
  saying they are the one definition, read by both the census and the tuner.

## Gates

```sh
cargo test --workspace
cargo clippy --workspace
cargo fmt
cargo build --release --bin tuner
./target/release/tuner dev-tuning/objective.ron            # rejection count matters
./target/release/tuner dev-tuning/objective.ron --measure
```

The search reports `N fought, M rejected`. **If M is most of N, stop and
revisit the central design question above** rather than shipping a search
that cannot move.

Expect the shipped roster to measure `opening-fight` 99.0%, `full-group`
100%, `lair-on-curve` 56.0% — those are `54fbf9d`'s numbers. A change there
means something moved that this work should not have moved.

## Traps

- **Arena numbers compare within one build only.** Do not compare a number
  from this work against one in `NOTES.md` from a different build; compare
  deltas.
- **`Target::reps` is dead config** — `eval::measure` overwrites it with
  `Objective::seeds`. Do not tune it expecting an effect.
- **A 20-rep reading is noise on any fight not at 0% or 100%.** `seeds` is
  200 for this reason; leave it.
- **The frozen set still excludes `scrapper`**, so it is untouchable
  regardless of what this work does. That is a separate piece — see
  `NOTES.md`'s suggested order.
