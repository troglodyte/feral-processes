# Damage types and resistances — staged design

**Status:** approved 2026-10-20; Stage 1 shipped in `v0.19.20`. Supersedes
`2026-10-16-damage-types-parked.md`, which holds the brainstorm and the
directions rejected there.

## Intent

Every hit today is plain Integrity damage. The only variation comes from
statuses and the Tamper kinds. The goal is to give weapons and wild programs'
attacks a **damage type** and armour a **resistance** to each type, so gear
choice depends on what you're fighting. Each type also does something
**besides damage** (its side effect). The user liked that idea and asked to
keep it.

The types are real physical forces that wreck hardware or data, so they fit
the lore of programs living in a machine.

## Decided by the user

- **Five types plus untyped:** Thermal, Magnetic, Noise, Voltage and
  Radiation. Untyped is today's plain damage (blades, kinetic) and is
  unchanged. Corruption is not a type; it is Radiation's side effect. Shred is
  out. The electrical type is named **Voltage** (it was "Surge").
- **Resistance is a percentage that only reduces damage.** There are no
  weaknesses: a resistance never makes a type hit harder.
- **Resistance blocks the side effect at a threshold.** Damage scales with
  resistance normally. At or above a threshold, the side effect does not
  happen at all.
- **Where types come from:** weapons, and species moves, both from Stage 2.
  Abilities stay untyped until a later stage.
- **Where resistance comes from:** armour first; species natural resistances
  in a later stage.
- **Affixes may grant resistance, never a damage type.** A weapon's type comes
  only from its base item.
- **Voltage's arc ships last.** If the arc proves messy, Voltage first ships
  as just a big hit and gets the arc afterwards.
- **No asset files.** The type list, side effects and numbers are Rust. New
  *fields* on existing item and species `.ron` files are fine (those sets
  already live in `assets/`).
- **Balance is a separate effort after this one.** This work does not
  retune.

## Goals

- Damage types exist in the engine and the player can see them on gear and
  in the battle log.
- Armour resistance cuts typed damage and, above the threshold, cancels the
  side effect.
- Each type's side effect works on both the tactical battle map and the
  abstract battle model.
- Every stage ships as its own release and leaves the game playable.

## Non-goals

- Balance tuning. Picking resistance values to hold the level curves is the
  later balance effort's job.
- Weaknesses or vulnerabilities.
- Affixes that grant a damage type (the user's call). Affix stats are an
  `EquipmentStats`, so the new field would otherwise be authorable on them: a
  load-time check rejects an affix with a `damage_type`, following the
  affix loader's existing fault pattern, and the affix README says so.
- Typing non-attack damage: status ticks, fumble recoil, turrets, throws,
  props, terrain, siege, tantrums and squad damage stay untyped.
- Modding support.

## Anti-goals

- **Changing untyped fights.** A fight with no typed attacker must play
  exactly as it does today, including its seeded random draws. Stage 1 proves
  this; later stages must not break it.
- **A parallel status system.** Side effects that act over time use the
  existing status machinery (`StatusBehaviour`, `arm_status`), not a second
  one.

## Constraints

- **Code layout** (CLAUDE.md): a new engine subsystem is a directory, so this
  is `crates/engine/src/damage/` (the type enum, resistances, pure functions
  and tests). Constants go in `tuning/damage.rs`. Plain Rust structs and pure
  functions, not new ECS components.
- **RNG:** side effects draw no random numbers (see Stage 3), so typed hits
  shift no streams. If a builder finds a side effect that needs a draw, it
  happens only on a typed hit, never in a loop over query order.
- **Saves:** item stats are read from `ItemDb` at load, not stored per copy
  (`GearCopy` holds id, rarity, tier, affixes, quality and wear). New
  `#[serde(default)]` fields on `ItemDef`, `EquipmentStats`, `MoveDef` and
  `SpeciesDef` therefore need **no save-format bump**. Statuses armed by side
  effects are saved like any other status. The builder confirms that path in
  Stage 3.
- **Schema:** every new field on a `.ron` def is `#[serde(default)]` (absent
  means untyped, or no resistance), and the matching `assets/*/README.md` is
  updated in the same change.
- **Tests that call `apply_damage` and `mitigate_incoming_damage` directly**
  (for example `crates/engine/src/tests/affixes.rs` and `tests/wielded.rs`)
  must not need
  edits. Add typed variants; the existing functions delegate to them as
  untyped.
- **Releases:** one release per stage that lands on `main`, per CLAUDE.md.

## The model

### Types and resistances

`DamageType` is a closed Rust enum: `Thermal`, `Magnetic`, `Noise`,
`Voltage`, `Radiation`. *Untyped* is the absence of a type
(`Option<DamageType>` = `None`), not a sixth variant.

A **resistance set** holds one percentage per type. Sources add together and
the total is capped at `DAMAGE_RESIST_CAP` (below 100) in `tuning/damage.rs`.

### Where it applies in a hit

Today `battle::damage_after_mitigation` does: general percent cut → round →
minus flat deflection → floor of 1 on a landed hit.

With a type, the per-type percent cut comes **straight after the general
percent cut, before deflection**:

```
general percent cut → type percent cut → round → minus deflection → floor 1
```

Untyped hits skip the type step, so their numbers don't change. The type step
is a pure function in `damage/`, called from the mitigation path. The current
path is `Game::mitigate_incoming_damage` (`game/combat_damage.rs`) into
`battle::damage_after_mitigation` (`battle.rs`).

### The threshold

If a target's resistance to the hit's type is at or above
`SIDE_EFFECT_BLOCK_THRESHOLD` (`tuning/damage.rs`), the side effect does not
happen. That is one constant shared by all five types (Claude's call).

### Where the attacker's type comes from

`Game::resolve_and_apply_attack` (`game/combat_damage.rs`) is the one place
where both attacker and defender are known. Every real attack goes through
it: party basic attacks, ability Damage and Drain, wild attacks, tactical
attacks and reactions, and sortie attacks.

One rule for every swinger (Claude's call): **the type follows the band the
swing uses.**

- **Basic attack with a worn weapon, party or wild:** the weapon's type.
  `attack_range` already makes a worn weapon's band override the natural one,
  and that applies to geared wild programs too (`game/combat_enemy.rs`).
- **Basic attack with no weapon, by a companion or a wild program:** the type
  of the species move rolled for this swing. Party swings roll it through
  `swing_move` / `swing_move_at`, which must return the move's type alongside
  its name and band. That way the move named in the log and the type always
  agree. The same applies to the sortie fallback swing and tactical reactions
  wherever they pick a move.
- **The player unarmed** (`Kit::Unarmed`, the data strike): untyped.
- **The player emulating a species** (`Kit::Emulated`): the rolled move's
  type, like a companion.
- **The defender's opening riposte** (`FumbleRung::Opening`): untyped, like
  the other fumble rungs.
- **Ability / special Damage effect:** the type on the ability def. Species
  moves turned into abilities by `basic_attack_ability` carry their move's
  type. Abilities from `assets/abilities/` stay untyped until Stage 5. A
  special does not take the weapon's type, matching the existing rule that a
  special uses its own band and not the weapon's.

### Where the fields live

- **Weapon type:** `damage_type: Option<DamageType>` on `EquipmentStats`
  (`items.rs`), next to the `damage` band. That is Claude's call; the builder
  may put it on `ItemDef` instead if `EquipmentStats` fights it.
- **Armour resistance:** a resistance set on `EquipmentStats`, so it adds up
  across slots in the `gear_bonus` fold like `mitigation` and `deflection`
  do. Affix resistances add in through `copy_bonus`, which already adds an
  affix's `EquipmentStats` field by field. Claude's call: resistances are
  **not** scaled by rarity, tier or
  quality. They are taken as listed, and the scaling functions leave them
  alone. Scaling is for the balance effort.
- **Move type:** `damage_type: Option<DamageType>` on `MoveDef`
  (`species.rs`), carried through `basic_attack_ability` onto
  `AbilityEffect::Damage`.
- **Species resistance (Stage 4):** a resistance set on `SpeciesDef`, added
  to armour before the cap.

## Side effects

General rules (Claude's calls):

- A side effect fires only on a landed hit (landed damage > 0) of that type,
  and only when the target is below the block threshold.
- **No chance roll.** A landed, unblocked typed hit always applies its side
  effect. That keeps random draws unchanged and makes the behaviour
  readable.
- Side effects that last over time are statuses. Their `StatusDef`s are
  built in Rust in `damage/` and registered alongside the asset-loaded
  statuses, so `arm_status`, the round ticks, the party-panel tag and saves
  all work unchanged. This is how "no asset files" is met for statuses.
- New `StatusBehaviour` variants are allowed where no existing one fits.
  They are added to the closed enum and its README.

| Type | Side effect | Built from |
|---|---|---|
| **Noise** | jams the target: its accuracy drops for a few rounds | a status; a new accuracy-cut behaviour if `EvasionCut` / `AtkPercent` can't express it |
| **Radiation** | corruption: damage each round while it lasts; healing and repair still work (the user's call) | a status with `DamagePerRound` (exists) |
| **Magnetic** | erasure: the hit ignores flat deflection | no status; skips the deflection step in mitigation when the target is below the threshold |
| **Thermal** | heat: stacks on the target, and a hot target takes more damage from every source | a stacking status with a new "damage taken +%" behaviour; may later tie to the Temperature tamper (not in scope) |
| **Voltage** | arc: a share of the landed damage jumps to one adjacent enemy | no status; see below |

**Voltage arc.** After a landed, unblocked Voltage hit, a fixed share
(`VOLTAGE_ARC_SHARE` in `tuning/damage.rs`) of the landed damage is applied as
Voltage damage to one other enemy:

- On the tactical map: a body occupying a cell next to the target's
  footprint, using `tactical::reach::recipients_in_cells`.
- In the abstract model, against an enemy: another member of the target's
  enemy group.
- In the abstract model, against the party: the next occupied party slot in
  slot order.
- In a sortie fight: the next target in the `targets` order.

Whichever side is hit, the arc only ever jumps to the target's own side. The
neighbour is picked deterministically, with no draw: the first in that order,
or on the map the lowest stable id. If there is no neighbour, there is no
arc. The arc is applied through the typed damage path directly,
**not** a second `resolve_and_apply_attack`, so it can't miss, crit, fumble
or recoil. It does not chain further. The arc target's own Voltage
resistance cuts it as usual.

## Display

- **Gear detail** (`Game::gear_detail` / `stat_summary` in
  `game/catalog.rs`, `GearDetailView` in `views/player.rs`): a weapon shows
  its type next to its damage, for example "4-7 Thermal DMG". Armour lists
  its resistances, for example "Thermal resist 20%". `views/manifest.rs`'s
  damage label gets the same treatment.
- **Battle log:** a hit that resistance cut says so. `AttackOutcome::Hit` /
  `Crit` carry only the final `dmg` today, so the outcome must also carry the
  type and how much was resisted. That is how `party_swing_line`, the wild
  lines and the ability "lands on X" line learn it. The wording is the
  builder's, in plain language and with no tick vocabulary.
- **Side effects** log through the existing status inflict/tick/expire lines,
  plus one line for the arc and one for Magnetic ignoring deflection only if
  the builder finds it unclear without one.

## Stages

Each stage is its own plan, branch and release. Every stage keeps
`cargo test --workspace` green.

### Stage 1 — groundwork (no change in play)

- `damage/` module: `DamageType`, the resistance set and the pure
  type-cut and threshold functions, with unit tests.
- `tuning/damage.rs` constants.
- The new fields on `EquipmentStats` and `MoveDef` (and through to
  `AbilityEffect::Damage`), all defaulting to untyped or no resistance.
- The type is passed from `resolve_and_apply_attack` into a typed damage
  and mitigation path. The old untyped functions delegate to it.
- `gear_bonus` and `copy_bonus` add resistances together; the affix loader
  rejects an affix carrying a `damage_type`.
- **Gate:** no asset sets a type yet, so `balance_sim` and every existing
  test pass **unchanged**. A moved number means the wiring is wrong.

### Stage 2 — typed gear and moves, visible

- Tag weapons, species moves and armour in their `.ron` files. Starting
  sort, from the parked spec (the builder refines it):
  - Voltage: `arc_lance`, `interrupt_coil`
  - Thermal: `plasma_router`, `overclock_core`
  - Noise: `broadcast_storm`, `scatter_lance`
  - Magnetic: `siege_compiler`, null-themed gear
  - Radiation: nothing yet
  - Untyped: the blades
  - Armour: `ablative_plating` → Thermal, `static_mesh` → Noise,
    `parity_weave` → Radiation, `null_weave` / `nullsteel_plate` →
    Magnetic; Voltage armour is the builder's pick.
  - Species moves are tagged by what the species is.
- Gear detail and battle-log display.
- Update the schema READMEs.
- **Balance:** per the user, no retuning. `balance_sim` does not model types
  today (it uses `MONOFILAMENT_WHIP`, an untyped blade, and no per-type
  resistance), so its curves may not move at all. If any asserted number does
  move, update it and record the before and after in the release's
  `CHANGELOG.md` entry for the balance effort.

### Stage 3 — side effects, one release per type

In order: **Noise, Radiation, Magnetic, Thermal, Voltage.** Each release adds
one type's side effect, its threshold check and its log lines, with tests on
both battle models. Voltage may ship first as a big hit (its weapons'
damage bands, no arc) and get the arc in a later release.

### Stage 4 — species natural resistances

The resistance set on `SpeciesDef`, added to armour before the cap, and
filled in for species where it makes sense (a hardened construct resists
Voltage). Shown wherever species stats are shown.

### Stage 5 — typed abilities

A type on abilities in `assets/abilities/`, defaulting to untyped. The
abilities that read as a type get tagged.

## Left to the builder

- Exact constant values (`DAMAGE_RESIST_CAP`, `SIDE_EFFECT_BLOCK_THRESHOLD`,
  `VOLTAGE_ARC_SHARE`, status durations and strengths). Sensible, not tuned.
- Whether the AI forecasts and gear rating (`tactical/prop_fight.rs`,
  `game/charge.rs`, `game/gear_power.rs`) learn about types, in Stage 2 or
  later. Leaving them untyped is acceptable; the balance effort may revisit
  it.
- Log and gear-detail wording, within the plain-language rule.
- The exact Stage 2 sort of weapons, armour and species moves.
- The data shape of the resistance set (a fixed array indexed by type, or a
  struct with one field per type) and its `.ron` spelling.
