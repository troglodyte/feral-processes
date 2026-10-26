# Phase 1 — Pure core (engine, nothing wired)

Read first: plan index; spec "The parts", "How parts become numbers",
"Testing"; `.claude/rules/seams-items.md`, `seams-combat.md`;
`routine_craft/{catalogue,compile,decompose}.rs` as the model.

## Files

- New `crates/engine/src/gear_craft/`: `mod.rs` (`GearDesign { id, name,
  slot, parts }`, `GearPart { Weapon(WeaponPart), Affix(AffixId) }`,
  `compile(&GearDesign, &AffixDb) -> Result<ItemDef, _>` dispatching on
  `slot`), `weapon/catalogue.rs` (`WeaponPart`, `Frame` + version,
  `category()`, `all()`, `shelf()`), `weapon/compile.rs`,
  `weapon/decompose.rs` (`&ItemDef -> Vec<WeaponPart>`), `weapon/fit.rs`
  (one-per-category, cap, Rider+Exploit same status allowed), `tests.rs`.
- New `tuning/gear_craft.rs` (re-exported from `tuning/mod.rs`): frame bases ×
  version, modifier deltas, per-status rider table, exploit %, cap start/max,
  `GEAR_FRAME_CEILING`, `design_power` flat terms.
- `items_db.rs`: `ItemDef` gains `#[serde(skip)]` `on_hit_rider:
  Option<WeaponRider>` and `exploit: Option<WeaponExploit>`. Expose the
  authored-weapon validators as one fn `compile` output can be checked by
  (extract if they are inline in `load_dir`; don't copy them).
- `gear_power.rs` (or `gear_craft/power.rs`): `design_power(&ItemDef)` =
  `copy_power` of a fresh Ordinary tier-0 copy + flat rider/exploit/proc terms.

## Tasks

- [ ] 1. Types + catalogue; `WeaponPart::all()` census (every category
  represented, Stun absent from Rider, present in Exploit).
- [ ] 2. Fit rules + cap as a pure check returning a typed refusal.
- [ ] 3. `compile` per part family (frame, range, reach+recharge, type, tilt,
  capture, proc via `grants`, rider, exploit, affixes folded into
  `EquipmentStats`). One test per family.
- [ ] 4. `all_designs_up_to_cap()` enumerator; census: every one passes the
  authored-weapon validators. Check Radiation vs the "typed weapon needs a
  resisting armour" census — stop and ask if it fails.
- [ ] 5. `design_power`; ceiling census; no-dominance census (same frame,
  same size: neither set wins every axis); rider-weaker-than-`CRAFT_RIDER`
  table test.
- [ ] 6. `decompose` over all 15 authored weapons: each maps to a Frame;
  decompiler → Capture, `grants` → Proc, mitigation → nothing.

## Gate

`cargo test -p feral-processes-engine gear_craft`, then `balance_sim`, then
`cargo test --workspace`. Handback: the authored-weapon → parts table from
task 6, any validator/census that needed a spec number changed (with the
number), whether the validators had to be extracted.
