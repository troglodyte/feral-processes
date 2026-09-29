# Level-up stat allocation on derived attributes

**Status:** spec, awaiting review. Not built. This is project 1 of 3, and
the later two are recorded at the end of this file.

## Intent

The player's combat stats are **derived** from their attributes
(`assets/attributes/`). They are not stored. When the player levels up, they
choose which attributes grow, and then choose perks. Character creation
spends its pool on attributes too. Unspent points bank until the player
spends them at the next level-up or from the Perks screen.

This reverses decision 4 of `2026-09-21-program-attributes-design.md`, where
attributes stay clear of the combat stats. Attributes now *are* where combat
stats come from. That spec's "the numbers do nothing" rule ends for every
attribute that is given an effect here.

In this project only the player is derived. Companions keep stored `Stats`
until project 2. Every new piece is keyed by entity, never hardwired to the
player, so project 2 extends it rather than rewriting it.

## Decisions (from the brainstorm)

| Question | Decision |
|---|---|
| Derived or stored | **Recomputed** from one function on every change. It is written to be changed often: a new stat means one enum variant, and a new attribute means one file. |
| Old saves | **Break.** `SAVE_FORMAT_VERSION` 32 → 33, a major release. |
| Player attribute start | The class's authored values, **no spread**. With no class value, the catalogue `base` is used. |
| Class attributes | **Kept as authored**, so classes now start with different stats. There is no net-zero rule. The arena gate reports each class's start. |
| Intelligence | A new attribute **Analysis (Intelligence)**, which merges with species `base_int` in project 2. |
| Mining | Analysis feeds a separate, **capped** `Extraction` term. The player never out-mines the sharpest species. |
| Decompiler growth | The automatic `DECOMPILER_SKILL_PER_LEVEL` is **removed**. Decompiler comes from Analysis. |
| Persistence | Status resist is **built now**. |
| Stamina | Bandwidth sets **max Power**, which becomes per-entity in this project. |
| Flow | Level Up report → **Points** (the creation Points screen, generalised) → the in-run **Perks** screen. `Esc` at any step leaves, and unspent points stay banked. |
| Other entry point | Uppercase `S` on the Perks screen, plus a HUD attention row. |
| Stat respec | Not in scope. |

## Engine

### Attribute effects are data

`AttributeDef` gains `#[serde(default)] effects: Vec<AttributeEffect>`:

```ron
// assets/attributes/parity.ron
effects: [(stat: MaxHp, per_point: 6.0)],
```

`AttributeEffect { stat: DerivedStat, per_point: f32 }`.
`DerivedStat { MaxHp, Atk, Mitigation, Decompiler, MaxPower, StatusResist,
Extraction }` is a closed enum in Rust, because each variant needs a reader.
Attributes stay open and moddable.

**The one formula.** For each stat:
`value = base + Σ per_point × (attribute value − that attribute's catalogue base)`.
The sum is rounded once per stat, never per term, and then clamped to that
stat's range (below). A player sitting at every catalogue base therefore
derives exactly today's numbers. An attribute value the store doesn't hold
counts as its base, so it contributes nothing.

`progression::derive(base: &DerivedBase, attrs: &Attributes, db: &AttributeDb)
-> DerivedStats` is a pure function. Nothing else computes a derived stat:
the game, the Points preview and `balance_sim` all call it.

| Stat | Base (player) | Range | Where the result goes |
|---|---|---|---|
| MaxHp | `PLAYER_BASE_STATS.max_hp` (90) | ≥ 1 | `Stats::max_hp` |
| Atk | `PLAYER_BASE_STATS.atk` (6) | ≥ 1 | `Stats::atk` |
| Mitigation | `PLAYER_BASE_STATS.mitigation` (2) | ≥ 0 | `Stats::mitigation` |
| Decompiler | 0 | ≥ 0 | `Decompiler::skill` (the stored base; `player_decompiler_bonuses` still adds on top) |
| MaxPower | `POWER_MAX` (100) | ≥ `MIN_MAX_POWER` | the new `Derived` component |
| StatusResist (%) | 0 | −50..=75 | `Derived` |
| Extraction | 0.0 | 0..=`MINING_EXTRACTION_CAP` (0.10) | `Derived`, read by `mining_success_chance` in place of the player's `DEFAULT_BASE_INT` term |

The bases and caps are named constants in `tuning.rs`. `Derived` is a new
component, not new `Stats` fields. `Stats` is summed by `power()`, scaled by
zone and read in hundreds of places (the attributes spec §4), so the
readers of `Stats` do not change.

### Shipped effects (starting values, tunable as data)

| Attribute | Effects |
|---|---|
| Parity (Vitality) | MaxHp +6 |
| Bandwidth (Stamina) | MaxPower +2 |
| Footprint (Size) | Mitigation +1, MaxHp +2 |
| **Analysis (Intelligence)**, a new file: base 10, spread 3 (species `base_int`'s scale) | Atk +1, Decompiler +1, Extraction +0.005 |
| Persistence (Willpower) | StatusResist +1 |
| Entropy (Luck) | none (project 3) |

An attribute with no `effects` is **not buyable**: the Points screen lists
only attributes that have at least one effect. That hides Entropy for now,
and a modded attribute appears once it is given an effect. The
`assets/attributes/README.md` rule "`meaning` must not promise an effect"
relaxes only for attributes that have effects. Each shipped file with
effects gains a `#[serde(default)] does: String` line saying what it does,
as that spec's decision 2 requires.

### Recompute

`Game::recompute_derived(entity)` is the only writer of derived values. It:

1. Lifts gear (`apply_equipment_delta(-1)`), the order `unbake_bought_stats`
   uses. No stat operation may run with gear baked in.
2. Sets `Stats::{max_hp, atk, mitigation}` = derived + `BoughtStats`, the
   perk receipt, which is unchanged. It sets `Decompiler::skill` and
   `Derived`.
3. Puts gear back and clamps `hp` to `max_hp` and `PowerReserve` to max
   Power. It never refills either.

   *As built:* `spend_stat_points`, not recompute, then raises current HP
   by what max HP rose by, so a level-up's full heal is not undone by
   spending the points it paid. Power gets no such rule.

It is called after creation, any attribute spend, a perk purchase or respec
that touches `BoughtStats`, and load. The plan has to find every
player-`Stats` writer: most of them either go through recompute or turn out
to be derived. The only other writers left are `hp`, gear and emulation,
which keeps replacing atk/mitigation through `emulated_base` as today.

Integrity no longer raises `hp` with `max_hp` at creation. Creation calls
recompute and then sets `hp = max_hp` once, so a run never starts damaged.

### Max Power is per-entity

`PowerReserve` readers that use `POWER_MAX` (about 70 sites in the engine
and gui) read `Game::max_power(entity)` instead. That is `Derived::max_power`
when present, otherwise `POWER_MAX`. Views carry the maximum, and the gui
bars read it from the view and never from the constant. Companions have no
`Derived` in this project, so they stay at 100.

### Status resist

`arm_status` applies `remaining = max(1, round(duration × (1 −
resist/100)))` for the target's `StatusResist`. Negative resist lengthens a
status. There is no RNG draw. The direct `StatusEffects` write at
`game/combat_round.rs:1763` goes through `arm_status` too, so the rule
cannot be skipped.

### Levelling

`add_xp` takes `Growth { Auto { multiplier: f32 }, Points }` instead of a
bare multiplier. `Auto` is unchanged, and companions, workers, sorties and
the arena pass it. `Points` grows no stats, full-heals, banks overflow as
today, and reports `LevelGain::stat_points = STAT_POINTS_PER_LEVEL × levels`.
`award_player_xp` passes `Points`, adds to the player's `StatPoints(u32)`
component, and no longer grants Decompiler.

`STAT_POINTS_PER_LEVEL = 6`. **Canonical spend:** 4 Parity + 2 Analysis per
level = +24 max HP, +2 ATK, +2 Decompiler. That equals today's
`HP_PER_LEVEL`, `ATK_PER_LEVEL` and `DECOMPILER_SKILL_PER_LEVEL` exactly. A
compile-time assert ties `STAT_POINTS_PER_LEVEL` to those constants, and
`progression::canonical_spend(levels)` derives the 4/2 split from them.

### Creation

`CREATION_STAT_POINTS` buys attribute points on top of the class's values,
priced by a per-attribute `CREATION_COST_*` table replaced by one
`CREATION_COST_PER_ATTRIBUTE_POINT = 1`. `CharacterChoice.stats` becomes
`BTreeMap<AttributeId, u32>` keyed by attribute, because the attribute set
is open. `apply_creation_stats` writes attributes, calls recompute, then
fills `hp`.

### Spending

`Game::spend_stat_points(owner: StatOwner, spend: &[(AttributeId, u32)]) ->
Result<DerivedStats, SpendError>`. `StatOwner { Player }` gets a companion
variant in project 2. The whole spend is validated first, and a refused
spend writes nothing. `SpendError::{InsufficientPoints, NoSuchTarget,
NotBuyable}` covers an unknown attribute or one with no effects. It then
raises the attributes and calls recompute. `BoughtStats` is never written:
it is the perk and talent receipt, and a perk respec must not refund
level-up spends.

### Save

The format version goes up (33). `PlayerSave` drops
`max_hp`/`atk`/`mitigation`/`decompiler`, which are derived on load, and
keeps `hp` and `power`. It gains `stat_points: u32`. The player's
`attributes` map is already saved, and load calls recompute. A save→load
test is required, not only a RON round-trip. The dev-saves templates are
regenerated.

### Attention

`Game::attention` reports unspent stat points the same way it reports Perk
Points, with key `'p'` and text naming `S`.

## App-core

### The Points screen is generalised, not duplicated

`StatAllocation { purpose: AllocationFor, pool: u32, rows: Vec<AttributeId> }`
with `AllocationFor { Creation, Owned(StatOwner) }`. Its methods take the
`spent` map: `rows`, `spend_on_row`, `remaining` and the preview. The
preview calls `derive` on attributes + spent, so the before→after values
come from the engine formula and are never copied. Creation passes
`&mut creation_choice.stats`, and `Mode::AllocateStats` passes its own.
`Mode` is `Copy` with unit variants only, so the allocation lives in
`App.stat_allocation: Option<StatAllocation>`.

`CreationRow::Stat` becomes `CreationRow::Attribute { id, name, spent,
value, effects: Vec<(DerivedStat, before, after)>, cost }`. The
"can't leave while points are affordable" refusal stays creation-only, and
the `Owned` purpose can always be left.

### Flow

- **`Mode::LevelUp`:** `Enter` → `AllocateStats`. `P` → Perks, and `Esc` →
  Playing, as today.
- **`Mode::AllocateStats`:** `Enter` commits through `spend_stat_points`,
  and `Esc` leaves without spending. Opened from LevelUp, it goes to Perks
  if Perk Points are unspent and to Playing otherwise. Opened from Perks,
  it goes back to Perks.
- **Perks screen `S`:** opens `AllocateStats`. It is refused through
  `App::refuse` at 0 points.

### Level Up report

The after-column shows the stat points and Perk Points earned, because
growth is deferred. The duel projections read current stats.

## GUI

`render/creation.rs`'s Points drawing takes a `StatAllocation` view, and
creation and `AllocateStats` both call it. Each row shows the attribute
(setting name + legacy name), its value and its effects' before→after. The
footer comes from `AllocationFor`. Power bars read max Power from views.
Everything draws through `Painter`.

## Balance and measurement

- **`balance_sim`:** the player path uses `PLAYER_BASE_STATS` +
  `canonical_spend`. A test asserts that this equals the old
  `stats_after_levels(PLAYER_BASE_STATS, n, BASELINE_GROWTH_MULTIPLIER)` for
  every n in 0..=cap, so the curves did not move. Companions keep
  `stats_after_levels`. The model is classless (catalogue bases), so class
  differences are outside it.
- **Arena gate (before shipping):** at levels 10, 15 and 20, compare the
  canonical spend with all-Footprint, all-Parity and all-Bandwidth spends,
  and compare each class's level-1 start. Record it in `docs/measurements/`
  per its README. **If one spend or class dominates, stop and bring the
  numbers to the user.** No per-point value changes without them.

## Testing

- Engine:
  - `derive` at catalogue bases equals `PLAYER_BASE_STATS`, and each effect
    moves only its own stats.
  - Rounding is per stat, and the range clamps hold.
  - An attribute with no effects is not buyable.
  - Recompute is correct with gear worn and does not double-count
    `BoughtStats`.
  - `Points` growth banks 6 per level and grows nothing, and `Auto` is
    unchanged.
  - The player gets no automatic Decompiler.
  - An overspend writes nothing.
  - Status resist shortens and lengthens durations, and the floor is 1.
  - Max Power follows Bandwidth, and `PowerReserve` clamps.
  - Extraction is capped.
  - Save→load keeps attributes, points and derived values.
  - `attention` flags unspent points.
- App-core:
  - The flow runs LevelUp → AllocateStats → Perks → Playing.
  - `Esc` keeps the points.
  - Creation's leave refusal holds, and the `Owned` purpose has none.
  - `S` is refused at 0.
  - Creation commits the expected attributes.
- Mutation-check each new test.
- Gates: `cargo test --workspace`, `balance_sim` and clippy
  `--all-targets`. Also a `--screenshot` of the level-up Points screen,
  because a green suite is not play.

## Release

A **major** bump, because saves break, with a `CHANGELOG.md` entry. The
same change updates `assets/attributes/README.md` (the effects schema,
`does`, the buyable rule) and `assets/classes/README.md` (class attributes
now matter).

## Later projects (not built here; recorded so they are not lost)

**Project 3: Entropy drives crit and fumble.** Crit and fumble already
exist, using the global `CRIT_CHANCE` (0.08) and `FUMBLE_CHANCE` (0.05) in
the single-draw `battle::resolve_attack_inner` bands. Add `DerivedStat`
Crit and Fumble, fed by Entropy, and pass them per attacker through
`Combatant` so `expected_damage` stays a call. There is no new RNG draw,
but band shifts still move seeded outcomes. `balance_sim` models crit but
not fumble today.

**Project 2: all player-side programs are derived.** Player-side means the
player plus every program with `Tamed`.
- **On decompile**, a program's attributes are rolled. The roll is seeded
  from `program_seed(ProgramId)` (fusion's precedent) and not `GameRng`,
  so no RNG stream shifts. Both `Potential` quality and `Rarity` nudge it.
- **The capture zone's extra strength** becomes attribute points,
  auto-spent in the species' pattern.
- **The doors:** everything goes through `roster_parts()`, except fusion
  (hand-written components) and load. The doors are group and tactical
  capture (`decompile_body` / `decompile_squad`), the seven `adopt_program`
  callers, `grant_starting_program`, `fuse_companions` and the arena.
- **Stats:** owned programs get `Derived` and move from baked `Stats` to
  recompute. That brings rarity, `Potential` rolls, refactor, fusion,
  talents and the Buffer perk under receipts.
- **Species:** `base_int` becomes each species' authored Analysis.
- **Companion level-ups** bank points, and the player spends them. That
  happens on the Points screen when the companion is in the party, and from
  the **Manifest screen** when it isn't. `StatOwner` gains a companion
  variant.
