# Where this got to — updated 2026-08-12

Handoff notes for the roster-tuner work. Read `README.md` first for what the
tool *is*; this is what happened and what to do next.

## State

`roster-tuner` merged at `4749e75`. `assets/` has still never been written
to — the tool has only ever produced proposals.

Check `git log` rather than trusting this line; it went stale once already,
claiming the branch was unmerged for three days after it landed.

## The headline finding: depth 5 is not a roster problem

This was the question the tool was built to answer, and the answer is that
the tool cannot fix it and shouldn't try.

**Zone and depth stat scaling compound.**

- `ZONE_STAT_GROWTH = 2`, applied as `2^(zone-1)` → zone 3 is **4x**
- `STACK_DEPTH_STAT_GROWTH = 1.35`, applied as `1.35^(depth-1)` → depth 5 is **3.32x**
- Together: **13.3x base stats**

For the rootkit actually fought at zone 3 / depth 5:

| stat | base | scaled |
|---|---|---|
| `base_def` | 10 | **133** |
| `base_hp` | 120 | **~1,596** |
| `base_atk` | 11 | **~146** |

The `base_atk` figure matches the observed transcript hits (151, 134, 198),
which is the check that the arithmetic above describes the real game rather
than a plausible model of it.

A level-20 geared player's ATK does not approach 133, so
`combat_damage.rs:71`'s `reduced.max(1)` floors **every** party hit at
`MIN_DAMAGE = 1`. Every party action in the transcript reads `for 1 damage`.

The fight therefore needs ~57,000 damage (36 enemies x ~1,596 HP) delivered
one point at a time. Not hard — arithmetically impossible. No value of
`base_hp` or `base_atk` inside any sane bound changes it, because the
problem is the multiplier and not the multiplicand.

**Second, independent fault: 36 enemies against 4.** The fought line reads
`rootkit x5, zero_day x11, sub_process x8, worm x12` — four groups, 36
members, versus the player and three Scrappers. This would still be lopsided
if the stat problem were fixed, and vice versa. Two faults, not one.

It compounds with depth in later zones: zone 5 depth 5 would be 16 x 3.32 =
**53x**.

Reproduce in about two seconds:

```sh
sed 's/reps: 50/reps: 1/' dev-arenas/stack-depth-5.ron > /tmp/d5.ron
cargo run --release --bin arena -- /tmp/d5.ron
```

### What is NOT the explanation

- **Not gear.** The standing note blamed a fixture with no gear.
  `dev-arenas/stack-depth-5.ron` has plasma_router, bastion_lattice,
  singularity_matrix and three level-12 Scrappers, and still loses 50/50 in
  a mean of 2.5 rounds at 0% HP. Do not re-suggest gear.
- **Not the roster.** See above.
- **Not Trace.** Trace was zero in every rep of both measurements.

### The decision this leaves open

Whether depth scaling should combine additively with zone rather than
multiplying, be capped in combination, or be left alone with player power
scaled up to meet it — that is a game-design call and was deliberately not
made. `tuning.rs` is deliberately code rather than data, and the tuner
cannot reach it by design.

## Baseline the tuner measured

As of **2026-08-09**, before difficulty scaling became linear:

```
opening-fight    100% win (want 92%),  47.8% HP left (want 62%)
full-group       100% win (want 75%),  98.9% HP left (want 45%)
stack-depth-5      0% win (want 55%),   0%   HP left (want 30%)
```

Re-measured on **v0.8.1**, after it:

```
opening-fight    100% win (want 92%),    65% HP left (want 62%)
full-group       100% win (want 75%),   100% HP left (want 45%)
stack-depth-5      2% win (want 55%),     2% HP left (want 30%)
```

The `full-group` row was its own finding and still is: a geared zone-3 party
clears a full enemy group having taken **about 1% damage**. Surface content
at that point is trivial and depth 5 is a wall. A cliff, not a curve.

**Read the two blocks together before pointing the tuner at anything.** The
`opening-fight` row is `Fresh(level: 1, zone: 1)`, where the zone multiplier
is x1 under either curve — so its 47.8% → 65% drift is *not* the scaling
change, it is everything else that landed between those dates, and it is the
reason a baseline needs re-measuring rather than reasoning about.

Re-argued and re-measured on **2026-08-12**, at 200 seeds rather than 20:

```
opening-fight    99.0% win (want 100%),  66.7% HP left (want 62%)
full-group        100% win (want 100%),  99.9% HP left (want 90%)
lair-on-curve    28.5% win (want  55%),  27.6% HP left (want 30%)
```

Four things changed at once and the block above is not comparable with the
two below:

- **The targets were re-argued.** Opening-fight and full-group became
  guards near what the game already does; the lair became the one lever.
- **`stack-depth-5` is no longer a target** — see its scenario comment.
- **Every target's party now wears gear**, which the arena could not
  express until 2026-08-12. It changed nothing at `full-group`, which was
  already a walkover, and it is why `lair-on-curve` is not comparable with
  any earlier Stack number.
- **`seeds` went 20 → 200.** This is the one that matters most for reading
  any older number here: at 20 seeds the lair fight read **50%** against a
  true rate of 28.5%. Both saturated targets were unaffected, because a
  fight at 100% has almost no variance to sample — so the error was
  invisible in exactly the rows anyone would have checked it against.

What the linear change did and did not do here:

- **`stack-depth-5` went 0% → 2%, and that is the honest size of it.** The
  2026-08-08 diagnosis of this fight named two causes — every point of
  player damage floored at `MIN_DAMAGE`, and 36 enemies against 4. Linear
  scaling fixed the first and cannot touch the second: this seed fields
  **32 opponents** (5 rootkit, 2 zero_day, 10 glitch, 15 scrapper), and
  group size comes from `zone_group_cap`, which was already linear at +9 a
  zone. A target of 55% is still ~53 points away and no stat proposal
  closes that gap — it is a **volume** question, and the knob is in
  `tuning.rs`, which the tuner cannot reach by design.
- **The targets themselves still encode pre-change intent.** They were
  authored against a game where a zone doubled. None of the three has been
  re-argued since, so a run today is optimising the roster toward numbers
  nobody has revisited. That is a game-design call, deliberately not made
  here — the same line this file already draws about `tuning.rs`.
- The tuner cannot undo the scaling change (it writes `assets/species/*.ron`
  only), but it **can** partly re-create the old difficulty inside those
  files, and nothing in a proposal's summary would say so. Re-argue the
  targets before the next search, not after.

## Side-blindness — FIXED 2026-08-09

A target says "this fight should be won 75% of the time". It did not say
whether to get there by buffing the enemy or nerfing the party. The first
real run did both: it raised `rootkit` (the opponent in `full-group.ron`)
*and* dropped `scrapper.base_def` from 5 to **0**, its bound floor — and
Scrappers are the *party* in that scenario. A stat lowered to satisfy one
fight applies to that species everywhere in the game.

Fixed by freezing every species the player fields. `sides::player_fielded`
derives that set from each target scenario's `party` list, and
`Workspace::baseline` leaves those species out of the candidate **entirely**
rather than filtering them later — so `perturb` cannot pick one, `apply`
cannot write one, and `summary` cannot list one. A second search operator
added later inherits all three for free, which a filter inside `perturb`
would not have given.

Two things worth knowing before extending it:

- **A save-backed target is refused, not silently unprotected.**
  `Scenario::party` is a `Fresh`-only field, so a `Save` or `Template`
  player brings a roster this code cannot see, and an empty party would be
  indistinguishable from "this fight has no companions". All three shipped
  targets are `Fresh`, so nothing is blocked today.
- **This does not make the objective two-sided.** Every species in the game
  can be tamed, so a scenario's `party` is only which ones that fight
  happens to field. The real fix is coverage: field a species on *both*
  sides of some pair of targets and lowering it costs a fight elsewhere.

The regression is pinned by three tests, verified by removing the freeze and
watching them fail: `a_frozen_species_never_enters_the_candidate`,
`freezing_every_species_leaves_an_empty_candidate`, and
`the_search_never_proposes_a_change_to_a_species_the_player_fields`. A
fourth, asserting the *written* proposal was byte-identical, was written and
**deleted** — it passed with the freeze removed, because a short search
never happened to pick `scrapper` and patching writes identical values back
idempotently. Don't re-add that shape; it reads as coverage and is not.

## The first search against the re-argued objective — 2026-08-12

61 candidates, training error 0.2201 -> 0.0888, holdout 0.2738 -> 0.0761.
It moved 14 fields across 6 species and took the lair from 26% to 52%.

**One of those 14 was worth having and the other 13 were damage.** Applying
the whole proposal failed four censuses in the shipped suite:

| census | caused by |
|---|---|
| `base_roster_growth_multiplier_rises_with_difficulty_tier` | `sprite.growth_multiplier -> 1.238` |
| `extraction_aptitude_cuts_across_the_difficulty_ladder` | same field |
| `every_ordinary_species_stat_shape_agrees_with_its_affinity_class` | `drone.base_speed 8 -> 6` |
| `the_reach_rule_measurably_softens_a_full_pack` | `rootkit.base_hp +19`, `base_def +3` |

And two more violations went *unreported*, because the shape census panics
on the first failure: proposed `rootkit` spends 169 stat points where its
budget is 147, and proposed `sprite` spends 40 where its new growth band
demands 101.

**Why it can hardly avoid this.** Every ordinary species' stat block is
*derived*: `total == tier_budget(growth_multiplier) * class weight` exactly,
with per-axis shares to ±1 and a speed band per class. `tier_budget` is a
step function (50 / 105 / 140). The tuner's six movable fields are precisely
the inputs to that formula and it moves them independently, so essentially
any change it makes to an ordinary species' `base_hp`/`base_atk`/`base_def`/
`growth_multiplier` is invalid by construction. The censuses are in
`species.rs`; `constraints.rs` had never known about them. **Closed
2026-08-12** — see the next section for what teaching it cost and what it
revealed.

**The move that mattered was legal, and alone it beats the whole proposal.**
`overseer.base_atk 17 -> 11` is the lair guardian, and a boss — which the
shape census exempts. Applied by itself it takes the lair to **56.0%**
against a want of 55%, better than the full proposal's 52%, with the whole
workspace suite green. That is what is applied; the other five files were
reverted.

So the legal move set — bosses, `taming_difficulty`, speed within a class's
band — is not the crippling restriction it looks like. It is where the only
value in this search was.

## The constraints landed, and the search stopped moving — 2026-08-12

`constraints.rs` now knows the derived stat budget
(`species::stat_shape_faults`, shipping code the census asserts through), so
the 13 damaging moves in the run above are unreachable. The measurement that
matters is what that did to the search:

```
search done: 21 fought, 40 rejected, error 0.1019 -> 0.1019
```

**Two thirds of the iteration budget was spent being turned away, and the
proposal moved nothing at all.** This is exactly the failure the plan
predicted and the reason it said to measure before narrowing `perturb`: a
rejecting search whose legal move set is a thin slice of its search space
proposes nothing, and reports that as a converged run.

Read the two numbers apart, though, because they are not one finding:

- **40 rejected of 61** is a `perturb` problem. It picks one of six fields
  on one species uniformly, and four of the six are illegal to move
  independently on any of the 15 ordinary species. Narrowing it — a boss's
  stats free, `taming_difficulty` free, `base_speed` within its class band,
  and an ordinary block moved only by *changing the budget and
  redistributing by class share* — is the fix, and it is a new search
  operator rather than a tweak.
- **error 0.1019 -> 0.1019** is not necessarily the same problem. The
  baseline was 0.2201 in the run above and is 0.1019 now, because
  `overseer.base_atk 17 -> 11` was applied in between. The shipped roster is
  already close to the best the *legal* move set can reach, so a search that
  proposes nothing may be right. 21 legal candidates is too few to tell, and
  that is the point: the two questions cannot be separated until the search
  can actually spend its budget.

`report.md` now tallies rejections by rule, so the next run says which of
these it is without anyone re-deriving it.

## Two defects found on 2026-08-12, neither fixed

Both were found while re-arguing the targets and both are small; they are
recorded rather than fixed because neither is a tuning question.

- **`Target::reps` is dead config.** `eval::measure` overwrites
  `scenario.reps` with `Objective::seeds`, so the `reps:` on each target in
  `objective.ron` is read by nothing while looking exactly like the knob
  that controls sample size. It is required by the schema, so it cannot
  simply be deleted from the file without also dropping the field.
- **`balance_sim::full_group_at_zone` projects a fight the game cannot
  field.** It calls `zone_group_cap(zone)` — 19 at zone 3 — but a real
  fight is capped by `Game::max_group_size`, which is `2^danger_steps`
  clamped by that, and `danger_steps` on the surface is `zone - 1`. So zone
  3 fields at most **4**. Its doc justifies the wider figure as "what
  `Game::max_group_size` allows once distance growth is fully unlocked",
  which is stale: distance-driven scaling was removed on 2026-08-05. This
  is the fourth time `balance_sim` has drifted from the game it models.

## Suggested order

1. ~~Decide the depth-scaling question~~ — decided, and the curves went
   linear in `0.8.1`. What that did and did not fix is above.
2. ~~Fix the side-blindness~~ — done, see above.
3. ~~Land `fixture-cleanup`~~ — merged and released as `0.5.10`.
4. ~~Run a search against the re-argued objective~~ — run 2026-08-12. See
   the section below; the one field worth taking from it is applied.
5. ~~Teach `constraints.rs` the derived stat budget~~ — done 2026-08-12,
   and it is the safety net either way. It also answered the question it
   was told to measure: **`perturb` now needs narrowing.**
6. **Narrow `perturb` to the legal move set.** Two thirds of the last
   search's budget went on candidates it should never have generated. The
   shape is in the section above and in
   `docs/superpowers/plans/2026-08-12-tuner-roster-constraints.md`'s central
   design question — a boss's stats free, `taming_difficulty` free,
   `base_speed` within its class band, and an ordinary block moved only by
   changing the *budget* and redistributing by class share. Until this
   lands, "the search proposed nothing" is not evidence the roster is
   already right.
7. **Unfreeze `scrapper` via coverage.** It is the party in both party
   targets and therefore frozen out of the candidate entirely, which means
   the game's most-fielded species is the one the tool can never tune.
   `README.md`'s two-sided note has the argument; the change is a target
   that fields it as an *opponent*, plus a freeze rule that reads coverage
   rather than `party` alone.
8. **Phase B (burn / learned enemy tactics)** is still gated on the same
   thing it was gated on at the start: enemies carry at most one routine
   (`roll_wild_routine`, `game/spawning.rs:108`), so there is no per-turn
   decision for a policy to learn. Giving enemies real choices is a
   game-design change with no ML in it, and it has to come first.

## The /tmp inode leak (separate; landed in 0.5.10)

A build died mid-session with `No space left on device` on a tmpfs that was
15% full — it was **inodes**, not bytes: 1,048,576 of 1,048,576 used.

`scratch_assets_dir` (`crates/engine/src/tests/support.rs`) copies the whole
shipped asset set (~190 files) per test and left cleanup to the caller, so a
test that panics on a failed assert leaked the lot, and a helper returning a
bare `Game` had no way to clean up at all. Fixed with an RAII guard,
`ScratchAssets`, whose `Drop` is best-effort so a failed removal cannot
panic during unwinding and bury the assertion that actually failed.

Measured over a full workspace run, before and after: **62 directories and
10,741 inodes become 9 and 34** — about 97 runs to exhaustion, becoming
about 30,000. The earlier "53/run becomes 0/run" figure counted only the
engine's asset installs and was right about those; the residual 9 come from
app-core's save fixtures and the arena's, which are different helpers and
were left alone. Clear with `rm -rf /tmp/feral_*` if a build ever starts
failing for no visible reason.
