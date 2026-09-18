# 2026-09-18 — Fitting `EMULATION_EDGE`

**Rerun 2026-09-18 (final review, F10/U4)** against a realistic player —
Perk Points spent, a class, tier-appropriate gear, two levels, and the
ability's own real duration instead of a 9999-round stand-in. The rerun
overturns two things the first pass believed and confirms a third.

## The claim

`EMULATION_EDGE` (1.25) **stays**, but for a different reason than the first
pass gave: the constant now barely moves the outcome either way once a
realistic player's gear and Perk Points are in the mix (§"Does
`EMULATION_EDGE` still matter?" below), so there is no numeric argument for
moving it in either direction.

Three things changed since the first pass:

1. **The species gap collapses once gear and perks are counted.**
   Emulating the strongest *reachable* ordinary species (`zero_day`) no
   longer clearly beats a middling one (`scrapper`) — 63.0% vs 65.0% win
   rate at level 20, 55.0% vs 56.5% at level 30, both well inside 200 reps'
   ~3.5-point sampling noise. `Game::emulated_base` (F5, U2) adds worn gear
   and the `components::BoughtStats` receipt *after* the species figure and
   `EMULATION_EDGE` are applied, as flat bonuses — and a flat bonus is a
   bigger fraction of a weaker base than a stronger one, so it compresses
   exactly the spread the first pass measured. The weakest species tested,
   `sub_process`, moved from a real if modest edge (52.0% in the first pass)
   to barely distinguishable from the player's own kit (44.5% vs 39.0%
   control at level 20; 41.0% vs 38.0% at level 30).
2. **The real 10-round duration matters more than the species choice.**
   Staged for the ability's own real duration (F10-prep) instead of the old
   9999-round stand-in, `zero_day` at level 20 wins **63.0%** of a fight
   averaging 35 rounds. Staged to never lapse — the first pass's own method
   — the identical fight wins **92.0%**. The arena still cannot *invoke*
   Emulate (`PartyPlan::AllAttack` invokes no routine), so every number
   here is a floor: a played run that keeps recharging Power and spends the
   turn to re-invoke every cooldown would sit somewhere between these two
   figures, and this instrument cannot say where.
3. **The boss gap the first pass recorded was never only `growth_multiplier`.**
   See "Correcting the boss attribution" below — `wintermute` also starts
   with a far higher `base_mitigation` than any ordinary species, and
   mitigation never scales with level at all, so that whole axis of its
   edge was base stats from the first round. It no longer matters for
   *reachability* — U1 (final review) refuses an apex species' image at the
   extraction door — but the old doc's explanation was incomplete on its
   own terms, and this rerun corrects it since the finding is still true of
   the (now unlearnable) species.

## How to reproduce it

Built at commit `c5eff0a5` (F10-prep's real-duration staging) plus
`ab250299` (the `character:` perk fields this rerun needed) on
`feat/player-emulation`. Two player builds, two levels apart, each fought
against a *coinflip* pack of rootkits — a pack sized so the player's own
kit alone wins close to half the time, which is what makes a kit swap's
effect legible against 200 reps of sampling noise (`sqrt(0.5 * 0.5 / 200)
≈ 3.5` percentage points).

**Level 20, zone 3** (`dev-arenas/emulation.ron`, shipped):

```ron
(
    player: Fresh(level: 20, zone: 3),
    character: (
        class: Some(Striker),
        perk_points: 40,
        perks: [(Attacker, 5), (Defender, 5), (Buffer, 4), (LowPowerMode, 4)],
    ),
    equip: [
        (item: "plasma_router", tier: 0),
        (item: "bastion_lattice", tier: 0),
        (item: "singularity_matrix", tier: 0),
    ],
    emulate: Some("zero_day"),           // omitted for "own kit"; swapped per row
    opponents: [(species: "rootkit", count: 13)],
    reps: 200,
    seed: 7,
)
```

```sh
cargo run --bin arena -- dev-arenas/emulation.ron
```

**Level 30, zone 4** (not shipped — a scratch file, reproduced from this
block; `character:` spends the level's full 60-point allowance in the same
proportions, and `equip:` moves to `tier: 1` as one fusion level a
level-30 player would plausibly have banked):

```ron
(
    player: Fresh(level: 30, zone: 4),
    character: (
        class: Some(Striker),
        perk_points: 60,
        perks: [(Attacker, 8), (Defender, 8), (Buffer, 6), (LowPowerMode, 5)],
    ),
    equip: [
        (item: "plasma_router", tier: 1),
        (item: "bastion_lattice", tier: 1),
        (item: "singularity_matrix", tier: 1),
    ],
    emulate: Some("zero_day"),
    opponents: [(species: "rootkit", count: 18)],
    reps: 200,
    seed: 7,
)
```

Both counts (13 at zone 3, 18 at zone 4) were found by sweeping the control
row (no `emulate:`) until its win rate landed near 50% — `zone_group_cap`'s
own ceiling (4 at zone 3, 8 at zone 4) is far below both, so every fight
here is already past what the game would ever field on its own, by design.

Species: `zero_day` (the strongest species an image can actually be
learned for — `base_atk: 16`, `growth_multiplier: 1.5`, both the ceiling
among ordinary species), `scrapper` (`base_atk: 12`, `growth_multiplier:
1.25`, the starter companion species), and `sub_process` (`base_atk: 4`,
`growth_multiplier` unset/1.0, the weakest species shipped) — the same
three species tiers the first pass used, so the two passes read against
the same roster.

The `EMULATION_EDGE = 1.05` row was measured by editing
`crates/engine/src/tuning.rs` to that value, rebuilding
(`cargo build --bin arena`), rerunning, then restoring the file — `git
diff crates/engine/src/tuning.rs` is empty on this branch, confirming the
shipped constant is what it was before this measurement.

The no-lapse comparison (finding 2) was measured by editing
`assets/abilities/emulate.ron`'s `rounds: 10` to `rounds: 9999`,
rebuilding, rerunning `zero_day` at level 20, then restoring the file —
`git diff assets/abilities/emulate.ron` is empty on this branch for the
same reason.

## The numbers

### Level 20, zone 3 — geared, perked, `EMULATION_EDGE = 1.25` (shipped)

On-curve pack (4 rootkits, `zone_group_cap`'s own ceiling):

| kit | win rate | mean rounds | mean HP left |
|---|---|---|---|
| own kit (no image) | 100.0% (200/200) | 12.3 | 82% |
| emulating `zero_day` | 100.0% (200/200) | 9.0 | 87% |

Contested pack (13 rootkits, a coinflip for this build's own kit):

| kit | win rate | mean rounds | mean HP left |
|---|---|---|---|
| own kit (no image) | 39.0% (78/200) | 35.4 | 8% |
| emulating `sub_process` | 44.5% (89/200) | 35.2 | 8% |
| emulating `scrapper` | 65.0% (130/200) | 34.9 | 15% |
| emulating `zero_day` | 63.0% (126/200) | 34.9 | 14% |

`scrapper` edging out `zero_day` (65.0% vs 63.0%) is sampling noise, not an
inversion — both sit at the same distance from the control within one
standard error of each other.

### Level 30, zone 4 — geared, perked, `EMULATION_EDGE = 1.25` (shipped)

Contested pack (18 rootkits):

| kit | win rate | mean rounds | mean HP left |
|---|---|---|---|
| own kit (no image) | 38.0% (76/200) | 40.3 | 20% |
| emulating `sub_process` | 41.0% (82/200) | 40.7 | 16% |
| emulating `scrapper` | 56.5% (113/200) | 40.8 | 28% |
| emulating `zero_day` | 55.0% (110/200) | 40.5 | 28% |

The same shape as level 20: a real but modest edge for the weakest species,
`scrapper` and `zero_day` statistically tied, both clearly ahead of the
control.

### Does `EMULATION_EDGE` still matter? (level 20, zone 3, contested pack)

| kit | `EDGE = 1.25` (shipped) | `EDGE = 1.05` |
|---|---|---|
| emulating `sub_process` | 44.5% (89/200) | 42.0% (84/200) |
| emulating `zero_day` | 63.0% (126/200) | 64.5% (129/200) |

Both moves are inside sampling noise — the opposite of the first pass,
which found `EDGE = 1.05` cut the ordinary roster's benefit roughly in
half. The reason is finding 1 above: most of what an ordinary emulation is
worth to *this* player now comes from `emulated_base`'s flat additions
(gear, `BoughtStats`), which `EMULATION_EDGE` does not touch at all —
`stats.atk`/`stats.mitigation` (the part `EDGE` scales) is a smaller share
of the total than it was for the ungeared, unperked player the first pass
measured. **`EMULATION_EDGE` stays at 1.25**: nothing here argues for
moving it, because moving it barely changes anything for the player this
measurement models.

### No-lapse comparison (level 20, zone 3, `zero_day`, contested pack)

| staged duration | win rate | mean rounds | mean HP left |
|---|---|---|---|
| real (`emulate.ron`'s `rounds: 10`) | 63.0% (126/200) | 34.9 | 14% |
| never lapses (`rounds: 9999`, the first pass's own method) | 92.0% (184/200) | 29.4 | 31% |

A 29-point swing, on the same build, same pack, same seed — see "What it
does not say" below.

## Correcting the boss attribution

The first pass wrote the boss gap off entirely to `growth_multiplier`
(`wintermute`'s 2.0 against every ordinary species' ceiling of 1.5). That
undercounted it. Comparing `wintermute` and `zero_day`'s own `SpeciesDef`
fields:

| | `base_atk` | `base_mitigation` | `growth_multiplier` |
|---|---|---|---|
| `zero_day` | 16 | 4 | 1.5 |
| `wintermute` | 19 | 17 | 2.0 |

`progression::emulated_stats` never scales mitigation by level at all
(`Stats::mitigation`'s own rule, `mitigation_is_unscaled_by_level_but_
takes_the_multiplier`) — every point of `wintermute`'s 17-vs-4
mitigation lead over `zero_day` is `base_mitigation` alone, `growth_
multiplier` never entering that axis at any level. And even on attack,
`emulated_stats(def, 1, 0)` (no growth applied yet) is already `base_atk *
EMULATION_EDGE`, so `wintermute`'s higher `base_atk` gives it a head start
before the level curve or `growth_multiplier` ever run. **The boss gap was
base stats as well as growth, on both axes and starting from level 1** —
not something this measurement retested with `wintermute` itself, since
U1 (final review, 2026-09-18) now refuses an apex species' image at the
extraction door and it can no longer be learned in play. The correction
stands because the first pass's *explanation* was incomplete on its own
terms, whether or not the species it was about is still reachable.

## What it does not say

- **The Power cost, the cooldown, and the action economy of *reaching* an
  emulation are still not modeled.** The arena has no way to *invoke*
  Emulate (`PartyPlan::AllAttack` invokes no routine), so every number here
  assumes the image is already up at round 1 for free and (bar the
  no-lapse row) drops for good once its 10 rounds run out — a played run
  could spend a turn and 20 Power to re-invoke on cooldown, which would
  sit somewhere between the "real" and "no-lapse" rows above. This is now
  the largest blind spot in this file, larger than the species choice —
  finding 2 puts a number on it for the first time.
- **The perk and gear spend is one plausible level-20/level-30 build, not
  the only one, and not the ceiling.** A different split of the perk
  budget, a Buffer-heavy or Low-Power-Mode-heavy build, or better gear
  (fused, rare, affixed — `dev-arenas/README.md`'s note on this applies
  here too) would all move the *absolute* win rates; whether it would
  re-open the gap between `scrapper` and `zero_day` that flat bonuses
  closed here is untested.
- **The pack sizes (13 and 18 rootkits) are specific to this build.** They
  were fit so *this player's own kit* is a coinflip; a different level,
  perk spend or gear loadout needs its own fit, the same way the first
  pass's six-rootkit pack stopped being a coinflip the moment perks and
  gear were added (see the "count was refit" note in `dev-arenas/
  emulation.ron` itself).
- **No party.** The player fights alone by construction, to isolate the
  kit swap from a companion's own output.
- **`class: Striker` changes nothing this bin reports** —
  `arena::scenario::CharacterSpec`'s own doc: a class is an affinity spread
  over *authored routine power*, and `PartyPlan::AllAttack` invokes no
  routine. It is here for realism alone.
- **`EMULATION_EDGE_PER_PERK_LEVEL` is untested here**, for the same reason
  the first pass left it untested: it only ever pushes every row further
  in the same direction, so it cannot narrow or widen the gaps this rerun
  is about.
- **`balance_sim` models no abilities**, which is the whole reason this is
  an arena question rather than a `balance_sim` assertion.
- **These numbers compare within this build only.** A later change to
  `battle::resolve_attack`, mitigation, `emulated_base`, or the rootkit
  species file reshuffles the `GameRng` stream as well as the fight, so
  re-tuning against a report from a different commit is invalid — rerun
  both sides fresh.

## Open questions

- **Is the action economy of re-invoking Emulate worth measuring for
  real?** The no-lapse comparison shows it is the single biggest lever in
  this file, bigger than which species is worn. Answering it needs the
  arena to be able to *invoke* a routine mid-fight, which nothing in
  `PartyPlan` does today — a real feature, not a numbers-only rerun.
- **Does a maximally-developed build (fused/affixed rare gear, a different
  perk split) re-open the gap between ordinary species that flat bonuses
  closed here?** Untested; the first pass's own "not the ceiling" caveat
  still applies, now to a higher floor.
