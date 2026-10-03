# Derived programs: every player-side program derives its stats

**Status:** approved design, not built. Project 2 of 3 in the series recorded
at the end of `2026-09-29-level-up-stat-allocation-design.md` (project 1
shipped in `v0.14.0`, project 3 in `v0.14.7`). This spec replaces that
file's project 2 sketch where they disagree; the differences are listed in
"Departures from the recorded sketch".

## Intent

Today only the player's combat stats come from `progression::derive`. Every
tamed program still carries baked `Stats`, grows automatically and ignores
its own `Attributes` beyond the dossier. After this project:

- two programs of the same species differ by their attributes, and those
  attributes act (crit, fumble, status resist, max Power, mining);
- the player can make build choices per program;
- every player-side stat has one writer, `Game::recompute_derived`, and
  every change made after seating is either part of the base or on a
  receipt.

"Player-side" means the player plus every entity with `Tamed`. Wild programs
keep baked `Stats` and are never derived.

## Decisions (from the brainstorm)

| Question | Decision |
|---|---|
| Who spends a program's level-up points | **Auto-spend unless held.** By default points are spent at once in the program's growth split. A program marked *hold points* banks them for the player. No program is weaker for being ignored. |
| Attributes at capture | **Keep the wild values** the program already carries (minted from `body_seed` at spawn), so what the dossier showed before capture is what you get. Fusion keeps minting from `program_seed`. |
| Balance curves | **Must hold.** At the default spend, HP, ATK and mitigation equal today's `stats_after_levels` exactly, asserted by a test. |
| Saves | **Break.** `SAVE_FORMAT_VERSION` 33 → 34, a major release. |
| What a program derives from | **A stored birth base** (`ProgramBase`), computed once at seating. Zone, depth, boss, rarity and `Potential` strength live in the base, not in attributes. |
| Secondary stats at capture | **They act.** Max Power, status resist, crit and fumble come from the program's attributes from the moment it is seated, so species identity matters. |
| Mining | **The program's own Analysis**, at the existing `MINING_SUCCESS_PER_INT` (0.02) rate. `SpeciesDef::base_int` is deleted. |
| Held points during the level-up flow | **Not inserted.** Held programs are spent from the Manifest; the player's LevelUp → Points → Perks flow is unchanged. |

### Departures from the recorded sketch

- *"On decompile, attributes are rolled from `program_seed`"*: wild
  programs already carry minted attributes and taming already keeps them.
  Nothing is rerolled.
- *"The capture zone's extra strength becomes attribute points"*: rejected.
  A fixed spend pattern cannot reproduce one multiplier applied to both HP
  and ATK, so the curves could not hold. It would also make attributes
  measure capture depth instead of character, and a rarity downgrade would
  have to take back points that may already be spent. The strength goes into
  `ProgramBase` instead.
- *"Companion level-ups bank points, and the player spends them"*: banking
  is opt-in per program (hold), because posted workers and sortie programs
  level too.

## Engine

### Seating

`Game::seat_derived(entity)` turns a tamed program with baked `Stats` into a
derived one. Every door keeps doing exactly what it does today and calls it
last:

- `decompile_body` and `decompile_squad` (`game/combat_rewards.rs`);
- `adopt_program` / `adopt_program_pinned` (`game/spawning.rs`) and so every
  caller: stack market, nest cache, caravan, settlement relations, orphan,
  `reinitialize_program` (extraction) and the arena's `spawn_companion`;
- `grant_starting_program` (`game/lifecycle.rs`);
- `fuse_companions` (`game/party.rs`), on the child.

It does not seat on load (see Save).

Seating works backwards from the program's current numbers, with gear lifted
(`apply_equipment_delta(-1)`, as recompute does):

```
contribution = what `attrs` add above their catalogue bases, per stat
base.max_hp  = Stats.max_hp     − BoughtStats.max_hp     − contribution.max_hp
base.atk     = Stats.atk        − BoughtStats.atk        − contribution.atk
base.mit     = Stats.mitigation − BoughtStats.mitigation − contribution.mitigation
```

The contribution is computed by calling `derive`, never by restating the
formula. The plan picks the exact shape, for example a
`progression::attribute_contribution(attrs, db) -> DerivedStats` that
`derive` itself also calls. The secondary fields of the base (max Power,
status resist, crit, fumble) are the same defaults the player's base uses,
so attributes move them from there.

`derive` rounds and clamps per stat, so seating has to subtract the
*unclamped, rounded* contribution. The plan must show a program whose
attributes would clamp a stat (for example mitigation near 0 with low
Footprint) still round-trips, or else clamp the base so that it does.

Seating inserts `ProgramBase`, `Derived`, `StatPoints(0)` and
`HoldPoints(false)`, then calls `recompute_derived`. **The invariant:**
`Stats` before seating equals `Stats` after it, at every door, so current
`hp` is untouched. Only `PowerReserve` can move, through recompute's
existing clamp to the program's new max Power.

### `ProgramBase`

`ProgramBase(DerivedBase)` is a new, saved component holding the program's
base figures. `DerivedBase` is already the input type of `derive`.
`Game::derived_base(entity) -> DerivedBase` is the single accessor: it
returns `ProgramBase` for a program and `DerivedBase::player()` for the
player. `derived_stats` (`game/derived.rs`) and the app-core preview
(`stat_allocation.rs`) read it, replacing their hardcoded
`DerivedBase::player()`.

### Recompute

`recompute_derived` is unchanged except that it reads `derived_base(entity)`.
After seating it is the only writer of a program's `max_hp`, `atk` and
`mitigation`. It already does nothing for an entity without `Derived`, so
wild programs are unaffected.

### Level-ups

`Growth::Auto` stays for wild programs and anything unseated. A seated
program levels through a new `Growth::ProgramPoints { parity: u32, analysis:
u32 }`. Per level gained:

```
parity   = round(4 · g · roll)     // 6 HP each   → 24·g HP
analysis = round(2 · g · roll)     // 1 ATK each  → 2·g ATK
```

`g` is `SpeciesDef::growth_multiplier` and `roll` is `Potential::growth_roll`.
`progression::program_level_points(g, roll)` is the one function that
computes this, and `add_xp`, `balance_sim` and the tests all call it. The 4
and the 2 come from `canonical_spend`'s split, never restated. A
compile-time or unit assert ties them to `HP_PER_LEVEL`, `ATK_PER_LEVEL` and
the Parity and Analysis effects, as project 1 did.

At `roll = 1` and every shipped `g` (1.0, 1.25, 1.5, 2.0) the yield equals
today's `scaled_growth(HP_PER_LEVEL, g)` and `scaled_growth(ATK_PER_LEVEL,
g)` exactly, because `4·g` is a whole number for each. With any other
`roll`, HP moves in 6-HP steps instead of 1-HP steps. That is a small,
accepted shift in real games, and `balance_sim` doesn't model `roll`.

Unheld: the points are added to `Attributes` (Parity and Analysis) at once
and recompute runs. Held: they go into `StatPoints`. Either way the level
full-heals as today. The `LevelGain` a program reports names the points.

Mitigation still never grows from levels; it moves only through Footprint
spends.

### Spending

`StatOwner` gains `Program(Entity)`. `spend_stat_points` resolves the owner
to an entity and refuses, writing nothing, with `SpendError::NoSuchTarget`
when the entity is not seated. `apply_stat_spend` is already
entity-generic. A held program's spend raises its HP by what max HP rose
by, as the player's does.

`Game::set_hold_points(entity, bool)` toggles holding. Turning hold off
spends the banked points in the growth split at once, so no points are
stranded. The split is `program_level_points` scaled to the bank: the
largest whole number of levels' worth, then any remainder into Parity.

### Changes after seating

- **Refactor** (`refactor_companion`): refactors are permanent, so the
  delta `refactored()` writes is added to `ProgramBase` instead of `Stats`,
  then recompute runs. `refactored()` itself is unchanged, and it still
  reads the program's current stats with gear lifted, as today.
- **Talents** (`bake_talent_stat`): unchanged. They already write the
  `BoughtStats` receipt, and recompute adds it back.
- **Rarity** (`retier_rarity`, `promote_rarity`): their callers only reach
  wild programs (nemesis promotion on a `Hostile`, forks at spawn). A seated
  program can't be retiered: `retier_rarity` asserts no `ProgramBase`
  (a `debug_assert!`, with a test that exercises the guard).
- **Fusion:** today's arithmetic runs on each parent's `Stats` with gear
  *and* its `BoughtStats` taken out. The child carries the dominant
  parent's `BoughtStats` along with its talents. This fixes a latent bug:
  today a fused child's talent respec has no receipt to subtract. Then the
  child is seated.
- **Buffer perk:** the player's only, so it's unchanged.

### Secondary stats

Seated programs carry `Derived`, so the per-entity readers that already
exist pick up their values with no change: `combatant_profile`'s crit and
fumble, `arm_status`'s status resist and `max_power_of`. The power-regen
query (`systems.rs`, `With<Player>`) stays player-only unless the plan finds
a program `PowerReserve` that regenerates. If it does, it reads
`max_power_of` like the rest.

### Mining

`mining_success_chance` keeps its signature shape. A posted program passes
its own `Attributes` Analysis where it passed `SpeciesDef::base_int`, at the
same `MINING_SUCCESS_PER_INT`. Its `extraction` argument is `0.0`: the
Analysis effect's Extraction term is the player's alone, and a program
reading both would count Analysis twice. The player is unchanged.

`SpeciesDef::base_int` is deleted with its validation. Every shipped
species already authors an `analysis` value equal to its `base_int`; the
plan verifies that per file before deleting the field. `DEFAULT_BASE_INT`
becomes the Analysis catalogue base it already equals, or is deleted if no
reader survives.

### Attention

`Game::attention` adds one row when any held program has banked points:
*"N programs have points to spend"*, naming the Manifest.

## App-core

- `open_stat_allocation` takes a `StatOwner` instead of assuming the
  player, and reads `derived_base`, the owner's attributes and its points.
  `commit_allocation` passes that owner through. `AllocationOrigin` gains
  `Manifest`.
- **Manifest, Stats tab** (program subject only): uppercase **`H`** toggles
  hold points and uppercase **`S`** opens `Mode::AllocateStats` for that
  program. `S` is refused through `App::refuse` at 0 points. `Esc` or a
  commit returns to the Manifest on the same subject and tab.
- The LevelUp → Points → Perks flow is unchanged and remains the player's
  alone.

## GUI

- `render/points.rs` already draws a `StatAllocation`. Its title and footer
  name the owner (the program's name when it's a program).
- `render/manifest.rs` `program_sections`: attributes with values, banked
  points, a *holding points* marker, and the `H`/`S` hints. Draws through
  `Painter`.

## Balance and measurement

- **`balance_sim`:** `companion_stats` seats a base from
  `wild_stats_at_zone` at the species' authored attributes, then derives
  with `program_level_points(g, 1.0)` spent per level. A test asserts that
  HP, ATK and mitigation equal the old `stats_after_levels(…)` for every
  shipped species, every zone `balance_sim` models, and every level up to
  the cap.
- **Companion crit and fumble** now derive from species Entropy, in the
  game and in `balance_sim`, which has to model them to stay a simulator.
  Where a species' Entropy ≠ 10 its damage curve moves. **The moved curves
  are reported to the user before any `balance_sim` expectation is
  updated.**
- **Arena gate (before shipping):** at levels 10 and 20, for at least three
  species of different `g`, compare the default spend against all-Parity,
  all-Footprint and all-Bandwidth spends. Record the results in
  `docs/measurements/` per its README. **If one spend dominates, stop and
  bring the numbers to the user.** No per-point value changes without them.
- Mining: record the spread of a posted program's success chance across one
  species' attribute spread, as a single table in the same measurement
  file.

## Save

- `CreatureSave` gains `base` (the `ProgramBase`), `stat_points: u32` and
  `hold_points: bool`, all `#[serde(default)]`. For a seated program,
  `max_hp`, `atk` and `mitigation` are derived on load and not written,
  which is the plan's choice of skip-when-seated or removing them from a
  split save type. Wild creatures keep saving them.
- Load restores `ProgramBase` and recomputes. It never seats.
- `SAVE_FORMAT_VERSION` 33 → 34. Dev-saves templates are regenerated.
- A save→load test is required, not only a RON round-trip: a held program
  with banked points, a refactor, a talent and gear survives with identical
  `Stats`, `Derived` and points.

## Testing

Engine:
- The seating round-trip at every door: decompile body, decompile squad,
  `adopt_program` (and pinned), `grant_starting_program`, fusion, the arena
  companion. Each is tested with gear worn and with `BoughtStats` present
  where the door can have them.
- Seating with an attribute set that clamps a stat still round-trips.
- `program_level_points` equals `scaled_growth` for every shipped `g` at
  `roll = 1`. Unheld levels raise attributes; held levels bank.
- Turning hold off spends the bank, with nothing stranded.
- `spend_stat_points(Program)` refuses an unseated entity and an overspend,
  writing nothing.
- A refactor on a seated program changes `ProgramBase`, and recompute keeps
  it.
- `retier_rarity` on a seated program trips its guard.
- A fused child's talent respec refunds exactly (the reproducer for the
  fusion receipt bug; it fails on today's code).
- A high-Entropy program crits more, and a high-Persistence one has shorter
  statuses (through the existing readers).
- A posted program's mining reads its own Analysis, with no Extraction
  double count.
- `attention` flags held points.
- Save→load as above.

App-core:
- `H` toggles hold. `S` opens AllocateStats for the program and is refused
  at 0. `Esc` keeps the points and returns to the Manifest. A commit spends
  on the program, not the player.

Mutation-check each new test. Gates: `cargo test --workspace`, `balance_sim`
and clippy `--all-targets`. Also a `--screenshot` of a program's Points
screen.

## Release

A **major** bump, because saves break, with a `CHANGELOG.md` entry. The
same change updates `assets/species/README.md` (`base_int` removed; Analysis
drives mining) and `assets/attributes/README.md` (attributes now act on
tamed programs as well as the player).
