# Phase 1 — Catalogue, compile, decompose (engine, pure)

Read first: plan index; spec sections "The parts", "The part cap", "How parts
become numbers", "Decided by the user" 1–2; `.claude/rules/seams-combat.md`,
`seams-tactical.md`. Source: `abilities.rs` (`AbilityDef`, `AbilityEffect`,
`AbilityDb::load_dir` validators, `affinity_kind`), `statuses.rs`.

## Files

- New `crates/engine/src/routine_craft/{mod.rs, catalogue.rs, compile.rs,
  decompose.rs, describe.rs, tests.rs}` (layout is the builder's; keep the
  subsystem-directory shape of `tactical/`).
- New `crates/engine/src/tuning/routine_craft.rs`, re-exported from
  `tuning/mod.rs`.
- `abilities.rs`: extract the load validators into a pure
  `fn validate(&AbilityDef) -> Result<(), String>` that `load_dir` calls (so
  `compile`'s census calls the same code — "a call, not a copy").
- Heal over time: `abilities.rs` / battle resolution (`battle.rs`,
  `tactical/`), wherever the builder's chosen representation lands.

## Interfaces

```rust
pub enum Subroutine { Core(Core, Version), Range(RangeKind), Area(AreaKind),
                      Trickle, DamageType(DamageType), Status(StatusKind) }
pub struct CraftedRoutine { pub id: AbilityId, pub name: String, pub parts: Vec<Subroutine> }
pub fn fits(core: Core, part: &Subroutine) -> Result<(), &'static str>; // reason shown greyed
pub fn validate_build(parts: &[Subroutine], cap: usize) -> Result<(), BuildError>;
pub fn compile(r: &CraftedRoutine) -> AbilityDef;          // only parts→numbers site
pub fn decompose(def: &AbilityDef) -> Option<Vec<Subroutine>>;
pub fn material(part: &Subroutine) -> (ItemId, u32);      // 1 disk is implicit
pub fn describe(parts: &[Subroutine]) -> String;
```

Shapes are suggestions; names in the spec's tables are fixed.

## Tasks

- [ ] **1. Misfit census first.** Test that iterates every authored routine
  reachable by a wild program, species kit (`abilities:`/`moves:`), talent
  `Ability(id)`, item `grants`, and the seven boss exclusives, and asserts
  `decompose` returns `Some`. Write the catalogue enough to run it. **Any boss
  misfit → stop and report to the controller** (user's decision 2).
- [ ] **2. Catalogue + fit matrix + facing.** Table-driven tests from the
  spec's "What fits which Core" (one assertion per ✓/— cell is fine as a
  loop over a literal table in the test).
- [ ] **3. Tuning.** Base figures from shipped v1 files; derive Area factors,
  Trickle duration/cost, Range cost, Status rider chance from the shipped
  families. Each constant gets a one-line *why* naming the file it came from.
- [ ] **4. `compile`.** Version ×1.8/×2.8, Power +2/version, cooldown rule,
  Range→`ranged`, Area→`target`+`shape`, generated description, player name.
  Census: every Core × every allowed modifier multiset up to cap 6 passes
  `validate`. Spot tests: Patch (Heal v1.0, one ally) equals the spec's
  numbers; Damage v1.0 equals `segfault_v1` magnitude.
- [ ] **5. Heal over time in battle** (user decision 1). Works in the group
  fight and the battle map; Field Regen stays separate. Failing tests in
  both models first. Built only by `compile`; no authored file changes.
- [ ] **6. `decompose` ↔ `compile`.** Every decomposed routine compiles and
  validates (extends task 1's census).

## Gate

`cargo test -p feral-processes-engine routine_craft`, then
`balance_sim`, clippy, fmt. Handback: commits, constants chosen with sources,
any misfits raised, Heal-over-time representation chosen and why.
