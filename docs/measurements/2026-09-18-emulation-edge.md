# 2026-09-18 — Fitting `EMULATION_EDGE`

## The claim

`EMULATION_EDGE` (1.25) and `EMULATION_EDGE_PER_PERK_LEVEL` (0.1) — both
guesses when Task 2 wrote them — **stay unchanged**. Against a solo geared
level-20 player, an emulation of an ordinary (non-boss) species is clearly
worth taking: it roughly **doubles or more the win rate** of a fight that
is otherwise a coinflip (33.5% → 87-94%). But the strongest species reachable
— an apex boss, `is_boss: true` — already outclasses the player's own kit
**before `EMULATION_EDGE` is applied at all**, because a boss's
`growth_multiplier` (2.0) is double the player's own fixed growth rate
(`BASELINE_GROWTH_MULTIPLIER`, 1.0). Lowering `EMULATION_EDGE` from 1.25 to
1.05 — as low as the constant can go without breaking its own documented
invariant ("above 1.0, so an emulation always beats a wild program of the
same species and level") — barely moves the boss case (99.5% win / 52% HP
left → 99.0% / 46%) while roughly halving the benefit for every ordinary
species (scrapper 87.0% → 77.5%; the weakest species tested, sub_process,
drops from a real edge to statistical noise: 52.0% → 35.5%, against a 33.5%
control). **`EMULATION_EDGE` is not the lever that controls the boss case at
all** — the gap is set by `SpeciesDef::growth_multiplier` before the
multiplier ever touches it — so no value of this one global constant can
satisfy "worth taking" for the ordinary roster and "does not outclass" for
the strongest species at once. That is recorded here as an open risk rather
than something this task's scope (`tuning.rs` constants) can close.

`assets/abilities/emulate.ron`'s `research_zone: 2` is also unchanged: it
gates *when* Emulate can be learned, not the per-species power ratio this
measurement is about, and nothing here bears on it.

## How to reproduce it

Built at commit `630304b3` (todo #100 Task 7's `Scenario::emulate` field).
Two packs against the same solo geared level-20 player, zone 3, no party —
alone, so nothing but the player's own kit does the fighting:

- **on-curve**: 4 rootkits, at `zone_group_cap`'s own ceiling for zone 3 —
  the shape `opening-fight.ron` already established as "a real fight" (98%
  win, comfortable HP), used here to confirm an emulation does not make an
  already-easy fight suspicious.
- **contested**: 6 rootkits, past that ceiling — a coinflip on the player's
  own kit, which is what makes a kit swap's effect legible at all (a
  98%-90%-ish fight cannot show a 10-point win-rate move against sampling
  noise; a coinflip can).

```sh
cargo run --bin arena -- dev-arenas/emulation.ron
# 99.5% win, 52% HP left, mean 23.8 rounds — the shipped file, six rootkits,
# `emulate: Some("wintermute")`.
```

Every other row below is the same file with `emulate:` deleted (the
control), `emulate:`'s species swapped, or `opponents: [(species:
"rootkit", count: 4)]` for the on-curve pack — 200 reps, seed 7, in every
case:

```ron
(
    player: Fresh(level: 20, zone: 3),
    equip: [
        (item: "plasma_router", tier: 0),
        (item: "bastion_lattice", tier: 0),
        (item: "singularity_matrix", tier: 0),
    ],
    emulate: Some("<species>"),           // omitted for "own kit"
    opponents: [(species: "rootkit", count: <4 or 6>)],
    reps: 200,
    seed: 7,
)
```

The `EMULATION_EDGE = 1.05` rows were measured by editing
`crates/engine/src/tuning.rs` to that value, rebuilding
(`cargo build --bin arena`), rerunning the six-rootkit files, then reverting
the edit — `git diff crates/engine/src/tuning.rs` is empty on this branch,
confirming the shipped constant is what it was before this measurement.

Species and their relevant `SpeciesDef` fields, for reading the tables:

| species | `base_atk` | `base_mitigation` | `growth_multiplier` | note |
|---|---|---|---|---|
| `sub_process` | 4 | 3 | 1.0 (unset, default) | the weakest species shipped |
| `scrapper` | 12 | 3 | 1.25 | the starter companion species |
| `rootkit` | 12 | 3 | 1.5 | the pack's own species |
| `zero_day` | 16 | 4 | 1.5 | the strongest *ordinary* species |
| `wintermute` | 19 | 17 | 2.0 | `is_boss: true` — the strongest overall |

`scrapper` and `rootkit` end up with identical emulated attack at every
level despite different `growth_multiplier`s (1.25 vs 1.5): `progression::
scaled_growth` rounds `ATK_PER_LEVEL (2) * growth_multiplier` to the nearest
whole point, and `round(2 * 1.25) == round(2 * 1.5) == 3`. Their rows below
are identical, seed for seed — not a bug, a rounding coincidence at this
level.

## The numbers

### On-curve pack (4 rootkits, zone 3) — `EMULATION_EDGE = 1.25` (shipped)

| kit | win rate | mean rounds | mean HP left |
|---|---|---|---|
| own kit (no image) | 98.0% (196/200) | 26.3 | 41% |
| emulating `sub_process` | 100.0% (200/200) | 24.2 | 49% |
| emulating `scrapper` | 100.0% (200/200) | 19.2 | 61% |
| emulating `rootkit` | 100.0% (200/200) | 19.2 | 61% |
| emulating `zero_day` | 100.0% (200/200) | 18.8 | 62% |
| emulating `wintermute` | 100.0% (200/200) | 15.9 | 75% |

Every emulation improves on the player's own kit here, but the pack is
already close to a walkover on the player's own kit (98%), so a 100% ceiling
compresses every species into the same win-rate reading — HP left and
rounds are the only rows that separate them, and both move monotonically
with `growth_multiplier`.

### Contested pack (6 rootkits, past the zone's own ceiling) — `EMULATION_EDGE = 1.25` (shipped)

| kit | win rate | mean rounds | mean HP left |
|---|---|---|---|
| own kit (no image) | 33.5% (67/200) | 31.9 | 4% |
| emulating `sub_process` | 52.0% (104/200) | 32.1 | 9% |
| emulating `scrapper` | 87.0% (174/200) | 28.5 | 24% |
| emulating `rootkit` | 87.0% (174/200) | 28.5 | 24% |
| emulating `zero_day` | 94.0% (188/200) | 27.9 | 29% |
| emulating `wintermute` | 99.5% (199/200) | 23.8 | 52% |

This is the fight that shows what the feature is worth: every ordinary
species roughly doubles or more the win rate over the player's own kit, and
the ordering tracks `growth_multiplier` and `base_atk` exactly as
`progression::emulated_stats` says it should. `sub_process`'s edge is real
but small — it is barely stronger than the player's own kit *before* the
multiplier (its `growth_multiplier` of 1.0 matches the player's own, and its
`base_atk` of 4 is below the player's `PLAYER_BASE_STATS.atk` of 6), so
`EMULATION_EDGE` alone is carrying its whole case for "worth taking."

### Contested pack — `EMULATION_EDGE = 1.05` (tested, rejected)

| kit | win rate | mean rounds | mean HP left |
|---|---|---|---|
| own kit (no image, unaffected by this constant) | 33.5% (67/200) | 31.9 | 4% |
| emulating `sub_process` | 35.5% (71/200) | 32.3 | 5% |
| emulating `scrapper` | 77.5% (155/200) | 30.2 | 17% |
| emulating `rootkit` | 77.5% (155/200) | 30.2 | 17% |
| emulating `zero_day` | 78.0% (156/200) | 29.5 | 20% |
| emulating `wintermute` | 99.0% (198/200) | 26.5 | 46% |

The comparison this task turns on. Dropping `EMULATION_EDGE` almost to its
documented floor (1.0) barely touches `wintermute` — win rate moves one
point (99.5 → 99.0) and HP left drops 6 points (52% → 46%), both easily
inside what `growth_multiplier`'s own difference from an ordinary species
would produce with *no* edge at all. Every ordinary species loses far more:
`scrapper`/`rootkit` fall 9.5 points, `zero_day` 16 points, and
`sub_process` falls to a 2-point improvement over the control — inside the
noise a `sqrt(0.335 * 0.665 / 200) ≈ 3.3` percentage-point standard error on
200 reps already covers. **1.25 stays**, because 1.05 buys nothing at the
top and costs everything in the middle.

## What it does not say

- **Whether `wintermute` "should" be reachable at all is not this
  measurement's question.** It answers "if a player has this image, how
  strong is it", not "should extraction ever hand it out" — that is a
  content/economy question (how hard a boss is to down and to extract from)
  outside `tuning.rs`'s scope.
- **The player's own kit here is deliberately mid-tier, not the ceiling.**
  `equip`'s three items are tier-0, unfused, `Rarity::Ordinary` — the same
  loadout `full-group.ron` uses for a level-20 solo player. A maximally
  geared kit (rare tier, fused, affixed) would be stronger than what is
  measured as "own kit" here, which would *narrow* every gap in these
  tables, including the boss one. This instrument cannot speak to that
  ceiling; `dev-arenas/README.md`'s note on rarity and affixes applies here
  too.
- **No party.** The player fights alone by construction, to isolate the kit
  swap from a companion's own output — a party-bearing scenario would dilute
  every row in the same direction and by an amount this run cannot say.
- **`EMULATION_EDGE_PER_PERK_LEVEL` is untested here.** Buying
  `Perk::EmulationFidelity` only ever pushes every row further in the same
  direction (it is a positive addend on the same multiplier), so it cannot
  narrow the boss gap either — it was not measured because it cannot change
  this measurement's conclusion, only amplify it.
- **`EMULATION_ROUNDS` (the ability's real 10-round duration), its Power
  cost (20) and its 4-round cooldown are all bypassed.** `Scenario::emulate`
  inserts `components::Emulation` directly with `rounds_left: 9999`
  (`ARENA_EMULATION_ROUNDS`) so it cannot lapse mid-fight — the arena has no
  way to *invoke* Emulate at all (`PartyPlan::AllAttack` never invokes a
  routine), so this is the only way to stage the kit swap, and it measures
  the kit alone, never the action economy of reaching or holding it. A real
  fight that runs past 10 rounds pays to re-invoke; none of these numbers
  charge for that.
- **`balance_sim` models no abilities**, which is the whole reason this is
  an arena question rather than a `balance_sim` assertion (`CLAUDE.md`'s own
  rule for this seam).
- **These numbers compare within this build only.** A later change to
  `battle::resolve_attack`, mitigation, or the rootkit species file
  reshuffles the `GameRng` stream as well as the fight, so re-tuning against
  a report from a different commit is invalid — rerun both sides fresh.

## Open questions

- **Is a boss-tier emulation supposed to be this strong?** The measurement
  cannot answer "supposed to" — only that at `EMULATION_EDGE`'s documented
  floor (1.0) a boss species already doubles the player's own raw attack
  before any edge is applied, purely from `growth_multiplier`. If a future
  pass wants the strongest image capped closer to the player's own geared
  kit, the lever is not `tuning.rs`'s two emulation constants — it would
  need to read the emulated species itself (e.g. dampen `growth_multiplier`
  above some ceiling inside `emulated_stats`, or exclude `is_boss` species
  from `EmulationImages` at the extraction door) — a design decision, not a
  numbers-only retune.
