# Damage types — Stage 5 plan (typed abilities)

Spec: `docs/superpowers/specs/2026-10-19-damage-types-design.md`, § Where the
attacker's type comes from and § Stage 5. Stages 1–4 (`v0.19.20`–`v0.19.27`)
built the typed path, tagged gear and moves, added the side effects and
species resistances. This stage tags authored routines. Engine and assets
only; one `.ron` field with `#[serde(default)]`, no save-format change
(abilities are read from `AbilityDb`, never stored typed).

Paths are under `crates/engine/src/` unless stated. Read
`.claude/rules/seams-combat.md` and `seams-tactical.md` before starting.

## Already true (verified, do not rebuild)

- `AbilityEffect::Damage` has `damage_type` (`#[serde(default)]`), and the
  Special arm in `game/combat_round.rs` (~1780) puts it on the `Swing`. Charge
  releases (`fire_charge`) and triggers go through `use_ability`, so they
  inherit it on both battle models. `assets/abilities/README.md` already
  documents the field. **Tagging a `Damage` file is a data-only change.**
- `charge.rs:261` (`charge_ai_choice`'s forecast) stays untyped — the spec
  lets AI forecasts stay untyped.

## Decisions

- **Drain gets a type.** `damage_type: Option<DamageType>` on
  `AbilityEffect::Drain`, `#[serde(default)]`; the Drain arm in
  `combat_round.rs` (~1849, now `None`) passes it; `AbilityDef::attack_type`
  returns it for Drain too. The heal rides `dealt`, so a resisted drain
  heals less — intended, no special case.
- **Display.** `abilities::effect_label` (the one formatter for
  `routine_detail` and the creation wizard) shows the type after the band,
  "Damage 8–14 Thermal". Extract the band+type join from
  `Game::damage_label` (`game/crafting.rs`) into a pure fn in `damage/` that
  both call — not a second `format!`. Authored `description:` strings are
  not edited.
- **Sort (starting point; builder refines from each file's name and
  rider).** Tag only what reads as a type; the rest stay untyped.
  - Voltage: `interrupt_request`, `bus_fault`
  - Noise: `broadcast_storm`, `packet_shred`
  - Radiation (bit flips): `row_hammer_single`/`_group`/`_everyone`
  - Magnetic: `null_cache`
  - Thermal: a heat-themed damage routine if one exists, else none
  - Untyped: the `segfault_*`, `skim_*` lines, `haymaker`, `long_compile`,
    `kernel_*`, `gradient_descent`, `backprop`, the rest.
  - Avoid stacking a type whose side effect duplicates the file's own rider
    (Noise jams on a `stall` rider); pick another type or leave it untyped.
  - A whole-field routine typed Voltage arcs off every target: check one
    such fight in the arena before keeping it.
- **Balance:** no retuning. `balance_sim` models no abilities, so it should
  not move; if it does, the wiring is wrong.

## Tasks

1. **Drain type + label join** (TDD). Tests in `tests/damage_types.rs`: a
   typed Drain is cut by resistance and heals off the cut figure; an untyped
   Drain is unchanged; `effect_label` shows the type for typed Damage and
   Drain and is byte-identical for untyped; `damage_label` still passes.
   Fix every `AbilityEffect::Drain { .. }` constructor the compiler names.
2. **Tag assets** per the sort; update the README line "No shipped ability
   sets it yet" and add `damage_type` to the Drain entry. Census in
   `tests/assets.rs`: the tagged ids carry the expected type (only
   `Damage`/`Drain` have the field, so nothing else can).
3. **CHANGELOG** entry, plain language, per the release rules.

## Gates

`cargo fmt`, `cargo clippy --workspace --all-targets`,
`cargo test -p feral-processes-engine balance_sim` (unchanged),
`cargo test --workspace`. INDEX.md row: mark Stage 5 built at release.
