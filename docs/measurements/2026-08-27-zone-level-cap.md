# Fitting the zone level cap

## The claim

The level cap has to be **fitted to the geared clear curve, not to a design
intuition**, and the spec's proposed constants were unusable: `FLOOR = 6`,
`STEP = 5` caps zone 10 at 46 against a fully-geared clear requirement of
**77**, so every zone past 6 would have been a run that could not continue.
Re-derived by calling `balance_sim::min_level_to_clear_zone` live over zones
1-16 in both configurations, **`STEP = 11` is the smallest integer slope that
keeps every measured zone clearable**; 10 leaves zone 12 needing 113 against a
cap of 111.

The design goal the spec stated — the cap sits *under* the gear-free
requirement, so a zone cannot be cleared by levelling alone — **is not
satisfiable by any straight line over this range**, and that is a property of
the shipped curves rather than of the fit. Both clear curves pass near the
origin and then diverge (gear-free climbs about half again as fast), so a
slope steep enough for zone 16 necessarily overshoots the low zones. Under
the fitted constants the property holds from **zone 7 up**; zones 2-6 remain
clearable by levelling alone, by at most 6 levels of overshoot.

The party model had to change with it. `companion_level_for_player_level`
fielded companions at `1/sqrt(2)` of the player's level, which was right while
a companion had a ceiling of its own and is wrong now that it shares the
player's. Fitting against the old model gives `STEP = 13`; against the model
that ships, 11. **The same measurement, two answers, and the difference is
which party you assume** — which is the whole reason the model change came
before the fit rather than after it.

## How to reproduce it

Same build for every number below.

The curves come from a throwaway `#[test]` added to `balance_sim.rs`'s test
module, run with `--nocapture` and then removed. It is not shipped, because
`min_level_to_clear_zone` searching to level 400 across 16 zones twice is
slow and nothing needs it on every `cargo test`:

```rust
// inside `mod tests` in crates/engine/src/balance_sim.rs
#[test]
fn zzz_probe() {
    let (db, _) = SpeciesDb::load_dir(&species_assets_dir(), &shipped_abilities()).unwrap();
    let toughest = toughest_ordinary_species(&db);
    let party = median_ordinary_species(&db);
    let (weapon, armor) = best_gear_stats();
    for zone in 1..=16 {
        let g = min_level_to_clear_zone(toughest, party, zone, 400, BASE_PET_CAPACITY, true, (weapon, armor));
        let n = min_level_to_clear_zone(toughest, party, zone, 400, BASE_PET_CAPACITY, false, (weapon, armor));
        eprintln!("zone {zone} geared {g:?} nogear {n:?}");
    }
}
```

```sh
cargo test -p feral-processes-engine zzz_probe -- --nocapture
# and, for the old-model column, with companion_level_for_player_level
# temporarily returning ((l as f64) / SQRT_2).round().max(1.0) as u32
```

The arena runs:

```sh
cargo run --bin arena -- dev-arenas/full-group.ron
cargo run --bin arena -- dev-arenas/developed-companion.ron
cargo run --bin arena -- dev-arenas/stack-depth-5.ron
cargo run --bin arena -- dev-arenas/lair-on-curve.ron
```

## The numbers

Minimum player level to clear a full group of the toughest ordinary species
with `BASE_PET_CAPACITY` companions of the median species. **New.**

| Zone | Geared (old party model) | Geared (shipping model) | Gear-free (shipping model) | Cap at STEP 11 |
|---:|---:|---:|---:|---:|
| 1 | 1 | 1 | 1 | 6 |
| 2 | 5 | 5 | 6 | 12 |
| 3 | 14 | 12 | 17 | 23 |
| 4 | 24 | 19 | 28 | 34 |
| 5 | 34 | 26 | 40 | 45 |
| 6 | 44 | 36 | 55 | 56 |
| 7 | 57 | 44 | 69 | 67 |
| 8 | 69 | 56 | 86 | 78 |
| 9 | 86 | 68 | 109 | 89 |
| 10 | 100 | 77 | 134 | 100 |
| 11 | 121 | 100 | 147 | 111 |
| 12 | 144 | 113 | 176 | 122 |
| 13 | 154 | 121 | 189 | 133 |
| 14 | 164 | 130 | 202 | 144 |
| 15 | 175 | 138 | 215 | 155 |
| 16 | 186 | 148 | 228 | 166 |

The binding constraints are zones 11 and 12: the cap clears the geared
requirement by 11 and 9 levels there, and by 18 or more everywhere else.
The overshoot against gear-free peaks at **6 levels**, at zones 2, 3 and 4 —
which is the figure `GRIND_TOLERANCE_LEVELS` carries in
`tests/level_up.rs`, measured rather than chosen.

Arena, on the post-change build. All four **reproduce** what was already
believed and none is a discovery:

| Scenario | Win rate | Rounds (mean) | Player HP left |
|---|---:|---:|---:|
| `full-group` | 100% (50/50) | 6.9 | 98% |
| `developed-companion` | 100% (50/50) | 6.9 | 98% |
| `lair-on-curve` | 100% (50/50) | 9.1 | 99% |
| `stack-depth-5` | 0% (0/50) | 7.5 | 0% |

`stack-depth-5` losing every rep is the volume fault recorded in
`2026-08-12-stack-lair-reachability.md` and is untouched here, as expected —
nothing in this feature makes the party stronger.

**`developed-companion.ron` is inert, and this run is what noticed.** Its
numbers are identical to `full-group.ron`'s because the two files are
byte-identical once comments are stripped: the file's comment says the two
"differ in exactly one thing", every ring open, but no ring is authored in its
data. It has been measuring the control against itself. That predates this
work — see `2026-08-19-developed-companion-worth.md`, whose own numbers came
from a build where the difference was applied by hand — and it matters more
now than it did: a ring no longer grants levels at all, so even authored, the
scenario would need spent *talents* to stage a developed companion, which
`arena` cannot currently express.

## What it does not say

- **Nothing here is a playtest.** These are RNG-free projections and 50-rep
  staged fights. Whether a cap the player meets is *felt* as pressure or as
  a wall is not something either instrument can answer.
- **The sim models no abilities and no Power decay**, so the geared column is
  a floor on what a zone asks for, not a description of it. A real party has
  Specials the projection cannot see and a Power economy it does not spend.
- **One party shape.** `BASE_PET_CAPACITY` companions of the *median* species
  against the *toughest* ordinary one. A player fielding worse gear than
  best-in-slot needs more level than the geared column says, and the cap does
  not move to accommodate them — the 9-to-11-level margin at zones 11-12 is
  the whole of their headroom.
- **It says nothing about zones past 16.** The slope clears the geared curve
  with room at 16 and the curve is close to linear by then, but that is an
  extrapolation and not a measurement.
- **The overflow XP price is unmeasured.** `OVERFLOW_XP_BASE` and
  `OVERFLOW_XP_STEP` are shaped so that points grow like the square root of
  XP — the test asserts the property — but what a Perk Point *is worth* per
  zone is not modelled by anything: `balance_sim` has no perk term at all.

## Open questions

- **Does the surface want retuning now that the party is stronger?** Wild
  programs scale by `ZoneLevel::stat_multiplier`, a zone term and not a level
  term, so the surface does not scale to meet a party that now caps at the
  player's level rather than 1/sqrt(2) of it. The geared column dropping by
  up to 31 levels (zone 12: 144 → 113) is that effect measured. **No retune
  was made here, deliberately** — this file is the evidence a later decision
  would rest on, and folding a `ZONE_STAT_STEP` change into the cap's own
  change would have been a difficulty change nobody asked for.
- **Should zones 2-6 be grind-clearable?** The fit forces it. If the answer
  is no, the cap has to stop being a straight line — a floor that holds
  longer, or a piecewise curve — and every linearity argument in
  `ZoneLevel::stat_multiplier`'s doc comment then has to be re-read, since it
  is about the *enemy* curve and not about a cap.
- **`developed-companion.ron` needs authoring or deleting.** As it stands it
  is a duplicate of `full-group.ron` that reads as a measurement.
