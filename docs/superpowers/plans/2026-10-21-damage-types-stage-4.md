# Damage types — Stage 4 plan (species natural resistances)

Spec: `docs/superpowers/specs/2026-10-19-damage-types-design.md`, § Where the
fields live and § Stage 4. Stages 1–3 (`v0.19.20`–`v0.19.26`) built the typed
path, tagged gear and moves, and added the side effects. This stage gives
species their own resistance. Engine, gui and assets; one `.ron` field with
`#[serde(default)]`, no save-format change (species resistance is read from
`SpeciesDb` at use, never stored on an entity).

Paths are under `crates/engine/src/` unless stated. Read
`.claude/rules/seams-combat.md` before starting.

## Decisions

- **Shape.** `resist: damage::Resistances` on `SpeciesDef`, `#[serde(default)]`
  — the struct and `.ron` spelling armour already uses (`resist: (voltage:
  30)`). The spec's open "array or struct" question was settled by Stage 1.
- **One read.** `Game::type_resistances(entity) -> Resistances` is gear plus
  the kit's species: `Kit::Innate(def)` and `Kit::Emulated { def, .. }` add
  `def.resist`, `Kit::Unarmed` adds nothing. An emulating player borrows the
  species' resistance the way it borrows its mitigation and affinities. The
  two existing reads of `gear_bonus(..).resist` (`mitigate_with_resist`,
  `type_side_effect` in `game/combat_damage.rs`) call it instead, so the
  sum is taken before `resist_against` caps it, on both battle models, in
  sorties, arcs and ticks.
- **Values.** 20 for a species themed on a type (usually the type it deals),
  30 for the spec's hardened construct and for bosses. No species alone
  reaches `SIDE_EFFECT_BLOCK_THRESHOLD` (50): blocking a side effect still
  takes armour. A census test holds that.
- **Display.** `ProgramManifest::resistances: Vec<(DamageType, i32)>`,
  non-zero entries in `DamageType::ALL` order, rendered as a RESISTANCES
  ledger beside AFFINITIES and omitted when empty.
- AI forecasts and gear rating stay untyped (spec, § Left to the builder).

## Tasks

1. `SpeciesDef::resist` + `Game::type_resistances`; swap both reads.
   Tests (`tests/damage_species.rs`): a species resistance cuts a typed hit
   and is reported in `Resisted`; species plus armour sum before the cap; an
   untyped hit ignores it; species resistance plus armour reaching the
   threshold blocks a side effect; an emulating player borrows it.
2. Asset values + `assets/species/README.md`; census test in `tests/assets.rs`
   (construct resists Voltage; no species alone reaches the threshold).
3. Manifest field + gui RESISTANCES ledger; view test and a gui render test.

## Gates

`cargo fmt`, `cargo clippy --workspace --all-targets`,
`cargo test -p feral-processes-engine balance_sim`, `cargo test --workspace`.
