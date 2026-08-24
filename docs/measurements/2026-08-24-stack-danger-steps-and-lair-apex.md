<title>Stack danger-steps sum and lair-apex ungating</title>

## The claim

Two changes on `fix/stack-bugs-batch` (commit `f0d09c93`) both raise Stack
difficulty and neither is visible to `balance_sim`, which has no Stack term
at all — the arena is the only instrument for either. Measured here:

1. **`Game::danger_steps` now sums the zone step and the depth step
   underground** instead of returning the depth step alone. At **zone 1**
   this changes nothing (`zone_step` is 0 at zone 1 regardless), so the
   zone-1 depth-2 lair's group shape is untouched by this change — only
   change 2 below moves it. At **zone 3** it does exactly what the commit
   intended: an ordinary Stack ambush at depth 1 goes from fielding **one**
   enemy group of size 1 to **up to three** groups summing to a dozen or so
   members. At **zone 3 depth 3** the same change also inflates a lair's
   *escort* group size roughly 4x (danger_steps 4 → group-size ceiling 16,
   vs. 2 → 4 the depth step alone would have given), which is most of why
   that fight got harder.
2. **`Game::pick_lair_species` now draws from the biome's apex pool
   ungated by `APEX_ENTRY_STEP`.** With the shipped asset set this makes
   the ordinary-species-plus-`BOSS_STAT_MULT` fallback **unreachable at any
   depth or zone**: `overseer` and `wintermute` both list every biome a lair
   can roll in (`OpenGrid`, `Mainframe`, `NullSector`, `Deadlock`), so
   `boss_habitat_matches` never comes back empty. Every lair guardian in the
   game, including the shallowest possible one, is now a hand-authored apex
   species.

**Verdict on the specific worry** (a fresh player with no party meeting
zone 1 depth 2's guardian): a **literally solo, ungeared, level-1 player is
crushed** — 0/300 wins at 4.2 rounds mean, on both biomes tested. But the
same fight against a player who has tamed even **two** level-appropriate
companions (still no crafted gear — a plausible state for a run that
hasn't found a bench yet) is **97% winnable** at 8.2 rounds mean, losing
essentially no companions. A same-build A/B (below) shows the solo-level-1
case was *already* nearly a guaranteed loss against the toughest **ordinary**
zone-1-window species alone (1% win, no escort, no depth multiplier) — so
the apex swap makes an already-unsurvivable solo encounter faster and more
certain, but does not appear to be what makes it unsurvivable in the first
place. **This reads as: the zone-1 depth-2 lair is fine for the party shape
the game actually expects a player to bring to their first stack, and was
never designed to be soloable by a bare level-1 with no party at all.**

The **zone-3 depth-3 compounded case is a real, measured jump** — 63% wins
at 35.3 rounds and 1.89 companions lost, against the same on-curve party
that clears zone-3 depth-2 (the shipped `lair-on-curve.ron`) at 100% in
11.1 rounds losing 0.16 companions in this same build. That is not a
walkover any more, though it is not a guaranteed loss either. Whether 63%
at depth 3 is the *intended* target is a design judgement this file does
not make — see "What it does not say."

## How to reproduce it

Built the `arena` bin in release for reps this size:

```sh
cargo build --release --bin arena
```

Smoke check (nothing broken):

```sh
./target/release/arena dev-arenas/opening-fight.ron
./target/release/arena dev-arenas/lair-on-curve.ron
./target/release/arena dev-arenas/stack-depth-5.ron
```

The eight scenarios below (all new, all in `dev-arenas/`):

```sh
./target/release/arena dev-arenas/measure-z1d2-lair-fresh-mainframe.ron
./target/release/arena dev-arenas/measure-z1d2-lair-fresh-opengrid.ron
./target/release/arena dev-arenas/measure-z1d2-lair-developed-mainframe.ron
./target/release/arena dev-arenas/measure-z1d2-lair-developed-opengrid.ron
./target/release/arena dev-arenas/measure-z3d3-lair-oncurve.ron
./target/release/arena dev-arenas/measure-z3d1-ambush-oncurve.ron
./target/release/arena dev-arenas/measure-z1-apex-raw.ron
./target/release/arena dev-arenas/measure-z1-ordinary-raw.ron
```

Reps: 300 for the four zone-1 lair scenarios, 200 for the raw apex/ordinary
pair, 100 for the two zone-3 scenarios (fewer because each rep there is a
much longer fight — 35 rounds mean at depth 3 — and 100 was already stable;
see the noise note below). Seeds are named in each file so any rep replays
alone with `reps: 1, seed: <seed+n>`.

## The numbers

### Zone 1, depth 2 lair — the on-ramp stack's bottom

| scenario | player | wins | rounds (mean/median) | player HP left | companions lost |
|---|---|---|---|---|---|
| `measure-z1d2-lair-fresh-mainframe` | L1, zone1, no gear, no party | **0/300 (0.0%)** | 4.2 / 4 | 0% | 0.00 |
| `measure-z1d2-lair-fresh-opengrid` | same | **0/300 (0.0%)** | 4.2 / 4 | 0% | 0.00 |
| `measure-z1d2-lair-developed-mainframe` | L8, 2 companions L8, no gear | **291/300 (97.0%)** | 8.2 / 8 | 79% | 0.06 |
| `measure-z1d2-lair-developed-opengrid` | same | **290/300 (96.7%)** | 8.3 / 8 | 77% | 0.05 |

Every rep's `composition` field confirms the mechanism: the guardian is
always `overseer` (Mainframe) or `wintermute` (OpenGrid) at count 1, plus
exactly one escort group of count 1 (an ordinary species drawn from the
habitat) — matching `max_enemy_groups(depth 2) = 2` and
`max_group_size = 1` at zone 1 (`zone_group_cap(1) == 1`), so change 1 is
inert here and every bit of the difficulty is change 2.

### Zone 1 raw A/B — apex vs. the toughest ordinary species, isolated

`opponents:` staging always spawns with `boss: false` and a `1.0` depth
multiplier (`arena::setup::build_opponents` →
`spawn_wild_creature_scaled`), so neither side of this pair includes the
depth multiplier a real underground fight applies, and neither can
reproduce the old `BOSS_STAT_MULT` elevation (that flag is never set from
authored `opponents`). Read this as "what the species themselves are worth
at zone 1, single individual, no escort, no depth bonus" — narrower than
the real fight, but a legitimate same-build delta.

| scenario | opponent | wins | rounds (mean/median) |
|---|---|---|---|
| `measure-z1-apex-raw` | `overseer` x1 | **0/200 (0.0%)** | 5.8 / 5 |
| `measure-z1-ordinary-raw` | `rootkit` x1 (steepest ordinary Mainframe/NullSector species at the zone-1 window, `growth_multiplier` 1.5) | **2/200 (1.0%)** | 8.7 / 8 |

Both are effectively a loss for a bare level-1 player — `rootkit` alone,
with no boss multiplier at all, already beats a solo level-1 96%+ of the
time. The apex swap turns "almost always fatal" into "always fatal, and
faster," rather than turning a winnable fight into an unwinnable one.

### Zone 3 — the compounded case, and change 1's headline case

Both scenarios reuse `lair-on-curve.ron`'s own on-curve party (level 24,
zone 3, geared player; three level-12 geared scrappers).

| scenario | fight | wins | rounds (mean/median) | player HP left | companions lost |
|---|---|---|---|---|---|
| `lair-on-curve.ron` (shipped, re-run this build) | Lair, Mainframe, depth 2 | 50/50 (100.0%) | 11.1 / 10 | 97% | 0.16 |
| `measure-z3d3-lair-oncurve` | Lair, Mainframe, **depth 3** | **63/100 (63.0%)** | 35.3 / 33 | 54% | 1.89 |
| `measure-z3d1-ambush-oncurve` | ordinary Stack ambush, Mainframe, **depth 1** | 100/100 (100.0%) | 8.5 / 8 | 97% | 0.02 |

`measure-z3d1-ambush-oncurve`'s `composition` field confirms change 1 does
what was intended: reps field 2–3 enemy groups (e.g. `sprite x3, drone x2,
proxy x2`; `drone x4, sprite x4`) totalling up to a dozen members, where
`danger_steps` used to return the depth step alone (0 at depth 1) and the
old code fielded **exactly one** group of one enemy.

`measure-z3d3-lair-oncurve`'s `composition` field shows the compounding
directly: the guardian (`wintermute`, since Mainframe) is joined by an
escort group of **7 to 13** members (`wintermute x1, trojan x7`;
`wintermute x1, proxy x10`; `wintermute x1, proxy x13`), against
`lair-on-curve.ron`'s depth-2 escort of count 1. `danger_steps` at zone 3
depth 3 is `zone_step(2) + depth_step(2) = 4`, giving a group-size ceiling
of `2^4 = 16`; the depth step alone (the pre-change formula) would have
given `2^2 = 4`. That fourfold escort-size jump, not the apex species per
se (the depth-2 lair already fields an apex under this build), is what
makes depth 3 a real fight instead of a walkover.

## What it does not say

- **No companion Special ever fires.** The bin plays the game's own
  All-Attack every round, so every number here is a floor on what the
  party's output could be — same gap `balance_sim` has, stated in
  `dev-arenas/README.md`.
- **Absolute numbers do not compare across builds.** `lair-on-curve.ron`
  measured 100% at 3.2 rounds on 2026-08-19 (`dev-arenas/README.md`'s own
  table) and 100% at 11.1 rounds in this run — the same scenario, the same
  file, unrelated to either change this file is about. Many commits sit
  between those two dates (the combat model, the party passive-bonus
  removal, gear quality, contracts, and others), so that 3.2 → 11.1 shift
  is **not** attributed to either change here and is not evidence either
  way. Every table above compares only numbers produced in this one build.
- **"Toughest ordinary species" is a judgement call, not a search.**
  `rootkit` was picked by eye (`growth_multiplier` 1.5, the steepest
  non-boss species habitat-matching Mainframe/NullSector at the zone-1
  window) rather than by enumerating every candidate `windowed_matches`
  would return; a tougher ordinary species might exist and would only
  strengthen the finding that a solo level-1 was already in trouble.
- **The "developed" loadout (level 8, two level-8 companions, no gear) is
  authored by judgement, not derived from play telemetry.** Nothing here
  measures how many players actually reach the first stack in that shape,
  weaker, or stronger — it is offered as a plausible floor, not a measured
  median.
- **`opponents:`-staged scenarios (the raw apex/ordinary pair) omit the
  underground depth multiplier and cannot reproduce `BOSS_STAT_MULT`.**
  Stated inline above; repeated here because it is the sharpest caveat in
  the file — those two numbers answer a narrower question than "what does
  the real lair fight do."
- **Whether 63% at zone-3 depth-3 is the right target is not decided
  here.** This file only establishes that the fight materially changed and
  by how much; `lair-on-curve.ron`'s own history (see `dev-arenas/README.md`)
  shows picking a target number is an open, separate design question this
  measurement does not resolve.
- **This is a small slice of the game's biomes and zones.** Only Mainframe
  and OpenGrid were tested at zone 1 (the two the raw A/B and the fresh/
  developed scenarios use); `NullSector` and `Deadlock` were not, though
  both also ship both apex species and so should behave the same way.
