# Breeding — design

**Date:** 2026-10-06 · **Branch:** `breeding` · **Status:** spec, not built

## Goal

TODO #67. The player picks two owned programs and breeds them: a child
program incubates in a base structure and joins the roster when it is
ready. Breeding gives two things:

1. **Better programs over generations.** A child inherits the better of
   its parents' rolls and attributes, plus a small mutation, and each
   generation may climb a little past the wild ceiling, up to a hard cap.
2. **Hybrid species obtainable only by breeding.** A hybrid is an
   authored species file naming its two parent species.

Decisions taken in brainstorming (user's answers, not assumptions):

- Start fresh; the unbuilt 2026-08-13 design in the memory graph
  (`project:breeding-subsystem`) is input only.
- The player picks the pair; nothing breeds on its own.
- Parents are kept, each on a cooldown.
- Inheritance is best-of-both plus mutation, with a per-generation cap.
- Hybrids are authored in data. A cross-species pair with no authored
  hybrid yields one parent's species at random.
- Cost is one new item per breeding.
- The child incubates in a new structure, one child per slot, slots set
  in the structure's file. Incubation lasts about one sortie.
- The player-facing word is **Breed**.
- The "progression is earned by fighting" spine is deliberately not a
  constraint here: no fight-loot gate beyond the item's drop table, and
  no handicap on children beyond starting at level 1.

## Content (`assets/`)

### Item `breeding_seed`

New file `assets/items/breeding_seed.ron`, a consumable with no worn
stats. Its source is the existing `droppable: [(species, chance)]` list:
a few common non-boss species at 0.05–0.10 each (exact table picked in
the plan against `docs/measurements/` drop-rate notes). One is consumed
per breeding. Credit price follows the item README's price bands for
drop-only consumables.

### Structure `breeding_bay`

New file `assets/structures/breeding_bay.ron`, built from build materials
like any structure. New `StructureDef` field:

```rust
#[serde(default)]
pub incubation_slots: u32, // 0 for every structure but the bay; bay = 1
```

A built structure with `incubation_slots > 0` unlocks the Breed action.
`assets/structures/README.md` documents the field.

### Hybrid species

New optional `SpeciesDef` field:

```rust
#[serde(default)]
pub parents: Option<(String, String)>, // unordered pair of species ids
```

with `SpeciesDef::is_hybrid() -> bool`. A hybrid sets `habitats: []`
(never spawns wild) and is otherwise an ordinary species file: stats,
attributes, abilities, optional sprite (glyph fallback as today).

Loader rules, following the `*Db::load_dir` warn-and-skip pattern:

- a parent id that names no loaded species → skip the hybrid, warn;
- a parent that is itself a hybrid → allowed (hybrids are fertile);
- two hybrids for the same unordered pair → keep the first by id order,
  warn about the second;
- `parents: Some((x, x))` → skip, warn.

Ship one or two example hybrids so the path is exercised; names, glyphs
and stats are proposed in the plan for the user to approve. Update
`assets/species/README.md`.

### Hybrids must not leak

Every draw from `SpeciesDb::all()` that hands the player a program or a
fight must exclude hybrids. Known callers today:

- `game/stack_market.rs:229` — program offer (filters `!is_boss` only);
- `game/caravan.rs:330`;
- `game/contracts.rs:864`;
- `balance_sim::toughest_ordinary_species` and
  `balance_sim::median_ordinary_species` — would shift the balance
  curves.

The plan re-greps `SpeciesDb` usages at the time of writing; wild spawns
already go through habitat pools, which `habitats: []` keeps hybrids out
of. Dev tools (arena picker, sprite forge) keep listing hybrids.

## Engine (`crates/engine`)

### Species rule

Pure function, unit-tested in isolation:

```rust
pub fn child_species(a: &str, b: &str, db: &SpeciesDb, rng: &mut impl Rng) -> String
```

- `a == b` → that species;
- an authored hybrid for `{a, b}` → the hybrid;
- otherwise → `a` or `b`, uniformly.

### Inheritance

Pure function:

```rust
pub fn inherit(a: &ParentRolls, b: &ParentRolls, species: &SpeciesDef, rng: &mut impl Rng) -> ChildRolls
```

`ParentRolls`/`ChildRolls` carry `Potential`, `Attributes` and
`Generation`.

- **Generation:** `max(a.gen, b.gen) + 1`. Wild, tamed, bought and fused
  programs are generation 0 (an absent component reads as 0).
- **Potential:** each of the six rolls = `max(a, b) + U(-M, +M)` where
  `M = BREEDING_MUTATION` (~0.04), clamped to
  `[MIN_INDIVIDUAL_ROLL, min(MAX_INDIVIDUAL_ROLL + gen * BREEDING_GEN_STEP, BREEDING_ROLL_HARD_CAP)]`
  (~0.03 per generation, hard cap ~1.4).
- **Attributes:** for each attribute in the `AttributeDb` catalogue (the
  set `attributes::mint` iterates), `max(a, b)` plus a mutation in
  `{-1, 0, +1}`, clamped to the wild range widened at the top:
  `[base − spread, base + spread + min(gen, BREEDING_ATTRIBUTE_HARD_CAP)]`,
  where `base` is the child species' authored value (else the catalogue
  base) and `spread` the catalogue's, exactly as `mint` reads them.
  A parent's `Attributes` include stat points spent at level-up
  (`Game::spend_stat_points`), so the clamp is what stops trained points
  passing to the child wholesale.
- **Not inherited:** the child starts at level 1, 0 XP, `Rarity::Ordinary`,
  the species' innate routines only, no gear, talents, kernel ring,
  bought stats, custom name, fusion count, refactors or purchased tiers.

All constants are named in `tuning.rs`.

### Starting a breeding

```rust
pub fn breed(&mut self, a: Entity, b: Entity, bay: Entity) -> Result<String, BreedRefusal>
```

Every refusal is checked before the first mutation (the
`fuse_companions` pattern). Refuses when:

- game over, or a battle is active;
- `a == b`, or either is not owned by the player;
- either parent's `BreedReadyAt` is in the future;
- `bay` is not a built structure with a free incubation slot;
- no `breeding_seed` in stock.

On success: consume one seed; set `BreedReadyAt(now + BREEDING_COOLDOWN_TICKS)`
on both parents (~1,200, one `SORTIE_BOARD_ROTATION_TICKS`); roll
`child_species` and `inherit` **now** and store an `Incubation` record in
the bay's slot, due at `now + INCUBATION_TICKS` (~1,200). Rolling at start
means a reload cannot re-roll the child.

Parents remain fully usable during incubation — they can fight, work,
post, be studied or be fused. Nothing references them after `breed`
returns, so no existing guard (party, fusion, outposts, siphon, study,
sorties) needs to learn about breeding.

`BreedRefusal` is an explicit error enum with a `reason()` row fragment
short enough for a one-line menu entry.

### Incubation record

```rust
pub struct Incubation {
    pub species: String,
    pub potential: Potential,
    pub attributes: BTreeMap<AttributeId, i32>,
    pub generation: u32,
    pub due: u64,
}
```

Stored per bay as a component (`Incubator { slots: Vec<Option<Incubation>> }`)
sized from `incubation_slots` when the bay is built. The child is a
record, not an entity, until it hatches.

### Hatching

A system checks every bay each tick. A due incubation hatches through a
new acquisition door that goes through `roster_parts` like the other four
(`spawning.rs:442`), with `install_innate_routines` and `seat_derived`,
stats built from species base × rolled potential (no zone, depth or
rarity multiplier), and the stored attributes and generation.

- If `roster_room() == 0`, the child stays in the bay and retries each
  tick; the bay's sheet says why.
- Otherwise it joins as any new program does; past `pet_capacity` the
  existing unslotted-morale rules apply.
- One log line: the child is ready, naming its species and generation.

### Demolishing a bay

Refused while any slot holds an incubation ("a program is incubating").

## Save format

New saved state: `Incubator` contents per structure, `BreedReadyAt` and
`Generation` per creature (`CreatureSave`). The save is bincode, so
`SAVE_FORMAT_VERSION` goes 34 → 35: a breaking release per
`CHANGELOG.md`'s preamble. Bred programs' attributes use the existing
per-creature `attributes` field.

## App-core and GUI

Mirrors fusion's `Mode::Fuse` / `FuseSecond` / `FuseName`
(`app-core/src/app/party.rs`, `gui/src/render/party.rs`).

- **Entry:** uppercase `B` on the Breeding Bay's structure sheet, shown
  when a slot is free.
- **`Mode::Breed`:** pick the first parent. Programs on cooldown are
  listed dimmed with the refusal fragment ("ready later").
- **`Mode::BreedSecond`:** pick the second parent; the page previews the
  outcome — species (or "one of X or Y"), generation, and the child's
  potential range per roll.
- **`Mode::BreedConfirm`:** shows the seed cost; confirm calls `breed`.
- **Bay structure sheet:** one line per slot — "Incubating: Trojan, gen 2
  — ready soon" or "Empty". Time uses vague words, never ticks.
- **Program sheet:** shows the generation when above 0.

Lowercase letters select rows; actions are uppercase. Every new string
fits its row (popup row width is testable headlessly).

## Testing

TDD throughout; each piece starts from a failing test.

- **Engine units:** `child_species` (three cases); `inherit` (best-of-both,
  mutation bounds, generation cap, hard cap, attribute caps); each `breed`
  refusal leaves the game unchanged (seed count, cooldowns, bay); hatch
  at `due`; held child when the roster is full; demolition refused while
  incubating; no hybrid from the market, caravan or contracts draws.
- **Assets:** census of the new `serde(default)` fields; malformed hybrid
  and duplicate pair are skipped with a warning; every shipped hybrid's
  parents exist.
- **Save:** full save → load mid-incubation; the hatched child has the
  rolls recorded before the save. `BreedReadyAt` and `Generation` survive
  a round trip.
- **App-core:** the three modes, including picking a parent on cooldown
  and backing out.
- **Balance:** `balance_sim` excludes hybrids, then
  `cargo test -p feral-processes-engine balance_sim` passes with no curve
  moved.

## Out of scope

- Breeding started by bonds or by programs on their own.
- Procedurally generated hybrids.
- Inheriting routines, rarity or level.
- A group-menu entry beside Fuse (can follow).
