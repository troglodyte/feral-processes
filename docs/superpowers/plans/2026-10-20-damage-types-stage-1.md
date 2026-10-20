# Damage types — Stage 1 plan (groundwork, no change in play)

Spec: `docs/superpowers/specs/2026-10-19-damage-types-design.md` (approved
2026-10-20). Branch `worktree-damage-types`. Engine crate only; no save-format
bump; no asset sets a type or resistance.

**The gate for every task:** `cargo test --workspace` and
`cargo test -p feral-processes-engine balance_sim` pass with **no edits to
existing assertions**. A moved number means the wiring is wrong. The only
allowed edits to existing tests are mechanical: a new `damage_type: None` field
in a literal, or a field access where a tuple used to be.

Paths below are under `crates/engine/src/`.

## Decisions taken here (the spec left them to the builder)

- **Resistance shape:** `pub struct Resistances { thermal, magnetic, noise,
  voltage, radiation: i32 }`, each field `#[serde(default)]`, derives
  `Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize`, with
  `get(DamageType) -> i32` and `Add`. `.ron` spelling:
  `resist: (thermal: 20)`. A struct is easier to write in `.ron` than an
  array indexed by type.
- **Constants:** `DAMAGE_RESIST_CAP = 75` and `SIDE_EFFECT_BLOCK_THRESHOLD =
  50` (percent). `VOLTAGE_ARC_SHARE` waits for Stage 3 (YAGNI).
- **The type rides on `Swing`.** Add `damage_type: Option<DamageType>` to
  `battle::Swing` (`battle.rs:72`). `Swing::plain` and `Swing::reaction` set
  it to `None`, and there is a `with_type` builder.
  `resolve_and_apply_attack` already takes a `Swing`, so its signature does
  not change.
- **Weapon vs. natural is decided once.** The place that already makes a worn
  weapon's band override the natural one (`attack_range`) also decides the
  type. A worn weapon gives its own type (`None` for a blade) even when the
  move has a type. With no weapon, the move's type is used. Return the band
  and the type together from that one function; do not write a second "same
  rule" helper.
- **No new random draws.** Paths that don't roll a move today keep not
  rolling one. The sortie swing (`game/sortie.rs:701`) uses the worn weapon's
  type, or `None`. The tactical reaction (`tactical/turn.rs:530`) already
  rolls a move for its name, so it uses that move's type.
- **Previews stay untyped in Stage 1:** `game/charge.rs:67`,
  `game/gear_power.rs:111` and `tactical/prop_fight.rs:248,330`.
- **`AttackOutcome` is unchanged** in Stage 1. Carrying the type and the
  amount resisted is part of Stage 2's display work.
- **Every non-attack `apply_damage` caller stays untyped,** with no edits:
  status ticks, recoil, throws, turrets, terrain, tantrum, upkeep, rewards and
  stack features.

## Task 1 — `damage/` module and tuning

Files: new `damage/mod.rs` (`pub mod damage;` in `lib.rs`, kept
alphabetical), new `tuning/damage.rs` (`mod damage; pub use damage::*;` in
`tuning/mod.rs`).

Interfaces:

```rust
pub enum DamageType { Thermal, Magnetic, Noise, Voltage, Radiation } // Copy, Eq, Hash, serde, ALL const
pub struct Resistances { .. }                                       // as above
pub fn resist_against(res: Resistances, ty: Option<DamageType>) -> i32 // 0 for None; clamped 0..=CAP
pub fn type_cut(dmg: f64, resist_pct: i32) -> f64                   // identity at 0
pub fn side_effect_blocked(resist_pct: i32) -> bool                 // >= THRESHOLD
```

Tests (in `damage/`):
- the cap clamps a sum over 100;
- `None` resists 0;
- `type_cut` is the identity at 0;
- the threshold fires at exactly the constant.

## Task 2 — the typed mitigation path

- `battle::damage_after_mitigation_typed(dmg, percent, type_resist,
  deflection)` runs general cut → `damage::type_cut` → round → minus
  deflection → floor 1. The early return checks `type_resist` as well.
  `damage_after_mitigation` becomes a call with `type_resist = 0`.
- In `game/combat_damage.rs`, add `mitigate_incoming_damage_typed(target,
  dmg, ty)` and `apply_damage_typed(target, dmg, ty)`. The untyped versions
  delegate with `None`, and their signatures stay the same.
  `resolve_and_apply_attack` (`:313`) calls `apply_damage_typed(defender,
  rolled, swing.damage_type)`.
- `Game::resistances(target) -> Resistances` reads `gear_bonus(target).resist`.
  The cap is applied in `resist_against`, not here.

Tests:
- a grid over (dmg, percent, deflection) showing the typed version with
  resist 0 equals the untyped one;
- a typed hit with resistance lands less;
- rounding happens after both cuts (choose a case where rounding between the
  two cuts would give a different number).

## Task 3 — fields on gear, moves and abilities

- `EquipmentStats` (`items.rs:604`) gets `#[serde(default)] damage_type:
  Option<DamageType>` and `#[serde(default)] resist: Resistances`.
  - The four scalers (`scaled_for_level`, `fused_for_tier`, `for_rarity`,
    `for_quality`) pass both through unscaled.
  - `is_empty` counts both.
  - `has_upside` counts any positive resistance, so an affix that only grants
    resistance is valid.
- `copy_bonus` (`game/crafting.rs:815`) keeps the base's `damage_type` and
  adds affix resistances. `gear_bonus` (`:889`) adds resistances together and
  sets `damage_type: None`, because a total of types means nothing. The
  weapon's type is read from the weapon slot through Task 4's function.
- The affix loader (`affixes.rs:142`, `fault`) rejects an affix whose
  `stats.damage_type.is_some()`, using the existing message pattern.
- `MoveDef` (`species.rs:46`) gets `#[serde(default)] damage_type`.
  `AbilityEffect::Damage` (`abilities.rs:445`) gets `#[serde(default)]
  damage_type`, and `basic_attack_ability` (`species.rs:97`) copies the move's
  type across. Add `damage_type: None` to the struct literals; check whether
  the patterns at `abilities.rs:1206, 1783, 2034` are exhaustive.
- Update the READMEs:
  - `assets/items/README.md`: the two new fields, and add the missing
    `deflection` while there.
  - `assets/affixes/README.md`: the field count, and `damage_type` is
    rejected.
  - `assets/species/README.md`: the move's `damage_type`.
  - `assets/abilities/README.md`: `damage_type` is set only by species moves
    until Stage 5.

Tests:
- an affix with a `damage_type` is skipped with a warning (use a unique temp
  dir);
- `copy_bonus` adds an affix's resistance and does not scale it by rarity or
  quality;
- `gear_bonus` adds resistance across slots;
- a `.ron` move with `damage_type: Some(Voltage)` parses, and
  `basic_attack_ability` carries it.

## Task 4 — the attacker's type reaches the swing

- `swing_move_at` (`game/combat_round.rs:639`) returns `SwingMove { name,
  range, damage_type }` instead of a tuple. Unarmed returns `None`. Update the
  callers: `combat_round.rs:559`, `tactical/turn.rs:530,689`, `tests/kit.rs`
  and `tests/reactions.rs`.
- `attack_range` (or what replaces it) returns the band and the type. Update
  its callers.
- Each attack path that builds a `Swing` sets its type:
  - abstract round `:600`;
  - tactical `turn.rs:759`;
  - reaction `:532`;
  - enemy `combat_enemy.rs:284`;
  - sortie `sortie.rs:701`;
  - ability Damage `combat_round.rs:1758`, from the effect's type. Drain
    stays `None`.
- `Kit::Emulated` uses the rolled move's type through the same path. Check
  this; don't add a special case.

Tests (engine; put the typed defs on the test's own `ItemDb` and
`SpeciesDb`, not in `assets/`):
- a typed weapon against matching armour resistance lands less than against
  no armour (abstract round and tactical);
- an untyped blade worn by a species with typed moves stays untyped;
- an unarmed companion uses its move's type;
- a species-move ability carries its type through `use_ability`.

Mutation-check each test: revert the one wiring line it guards and confirm
the test fails.

## Finish

- `cargo fmt`, then `cargo clippy --workspace --all-targets` with no
  warnings, then the full gate.
- The final review is opus, over the whole branch diff written to a file.
- Release as a patch, per the `CHANGELOG.md` preamble: "Groundwork for damage
  types; no change in play". Use the deploy skill.
- Spec status becomes "Stage 1 shipped"; add a line to `INDEX.md`.
