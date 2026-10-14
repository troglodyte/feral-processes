# Fights Use Things Up Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Checkbox steps.

**Goal:** Healing spends `patch_routine`, defences spend `charge_coil`, and gear wears per fight and is repaired at a Refit Bench.

**Spec:** `docs/superpowers/specs/2026-10-14-fights-use-things-up-design.md`. Read it first. This plan does not restate it. Where the two disagree, this plan wins (see the next two sections).

**Phases:** two, matching the spec's two releases. **Phase 1 is one subagent** (Tasks 1–4). **Phase 2 is two subagents in sequence**: one for engine Tasks 5–7, then one for app-core/gui Task 8. The controller does docs, review and release at the end of each phase. Anchor: `E=crates/engine/src`, `A=crates/app-core/src`, `G=crates/gui/src`. Line numbers date from 0.18.26.

## Decisions taken here (veto at plan review)

1. **Coils per defence event, not per point.** The shipped turret authors `raid_defense: 1` and `turret.damage: 8`. Under the spec's "coils = points used", one off-screen siege would cost 9 coils per turret. Instead, each charged structure that contributed to a sweep or off-screen siege spends **1 coil per event**. A tactical turret still spends 1 coil per shot. One coil gates both of a turret's numbers.
2. **The wielded program does not wear gear.** It is not in `Party`, it does not fight, and its gear is inert while it is wielded. Gear wears on the player and on `Party` members only. Reinforcements and `Summoned` forks also do not wear.
3. **`unbreakable` is dropped (YAGNI).** Class kits carry no gear, and the easter egg has no item. Only `durability: Option<u8>` is added.
4. **Store wear taken, not condition.** `GearCopy.wear: u8` counts down from full: 0 is full and `#[serde(default)]` is free. `is_plain` gains `&& wear == 0` and needs no durability lookup. The UI shows `max - wear` as `n/max`.
5. **`MachineStatus::Uncharged`** is a new variant. `Dry` means "out of Power Cells", which reads wrongly for a defence.
6. **Uncraftable repair cost scales by rarity rank** (the drop tier). `ItemDef` has no tier field. The cost is `REFIT_DROP_FRAGMENTS_PER_RANK × (rank+1)` `core_fragment` plus `REFIT_DROP_COILS` coils.
7. **A supply's batch is a tuning constant.** `intake_recipe` returns `(supply, SUPPLY_BATCH)`, with `SUPPLY_BATCH` starting at 3. The existing `input_room` then holds `2 × SUPPLY_BATCH`. The spec's "topped up to capacity" is not how input room works.

## Spec corrections (code differs from the spec)

- **Rest:** the in-base path is the fall-through after `if !self.in_base()` (`E/game/turn.rs:1517`, restores at 1577–1642). It writes `stats.hp` directly. Keep that, and do not route it through `restore_hp`. Every owned program's **power** refill at 1636 stays as it is. The closing log line changes when coverage is partial.
- **No per-tick heal exists**, and no `recovery_system` exists (the doc comments at `E/structures.rs:180,389` are stale, so fix them). The Bay is `Game::run_repair_bays` (`E/game/base/repair.rs:105`, called from `turn.rs:329`). Put the trickle beside it as `Game::run_base_trickle`, because it needs `program_role`, which needs `&Game`.
- **Bay supply** lives in the Bay's `Stock.input`, and `spend_from_base` reads only outputs. Spend from the input directly. Track HP-per-patch with a new component `PatchCredit(u32)`: a supplied heal tick draws a patch when the credit is 0 and refills it to `REST_INTEGRITY_PER_PATCH`. `Bays::sites` (repair.rs:25) carries `(pos, rate, radius)`, so add the entity and the supplied rate.
- **`turret_defense` is a free fn** (`E/game/siege/turrets.rs:20`), not a def field. Readers:
  - `total_raid_defense` (`E/game/base/upkeep.rs:289`, which includes the garrison: **never coil-gate the garrison**)
  - `raid_defense_active` (:285)
  - `run_raid` (:824)
  - `run_town_raid` (:916)
  - `first_raid_defender` (~995)
  - `resolve_siege_offscreen_with` (`E/game/siege/offscreen.rs:45`)

  Gate in the per-structure sums. The garrison sum stays outside the gate.
- **`fire_one_turret`** (`turrets.rs:66`) has no log and no dry path. Spend the coil **after** a target is found, so empty rounds cost nothing. Log the dry line with `log_kind` once per battle per turret, not every round.
- **Defences have no `MachineStatus`.** It is inserted at two hand-mirrored sites (`E/game/base/building.rs:890` and `E/game/lifecycle.rs:1187`) only when `runs_a_job() || power_upkeep`. Extract `StructureDef::has_machine_status()` and call it from both sites.
- **Hauling:** generalise the `power_upkeep` special-cases rather than copy them. Add `StructureDef::spends_supply() -> Option<&ItemId>` (`power_upkeep` or `supply`) and have each special-case call it:
  - `hauling.rs:1162` and `:1235`
  - `work_orders.rs:1418` and `:1755` (`fuel_wants`)
  - `systems.rs:850`

  After the change `fuel_wants` serves both kinds, which gives one hauling path. Its position (`work_orders.rs:1022`, right after `build_wants`) already matches the spec's priority.
- **Equipment def is a tuple** (`ItemDef.equipment: Option<(slot, stats)>`, `E/items_db.rs:148`), so `durability` is a top-level `ItemDef` field.
- **Four save routes carry a copy:**
  - `PlayerSave.{weapon,armor,module}_quality` (`E/save.rs:55/67/79`)
  - `EquippedItemSave` (:1101)
  - `GearCopySave` (:1141)
  - caravan and buyback (grep `GearCopySave`)

  `wear` joins each one with `#[serde(default)]`. This is an additive field, so there is **no `SAVE_FORMAT_VERSION` bump** (it stays 37).
- **`rekey_copy` drops new fields.** It rebuilds through `with_affixes(.., quality)` (`E/game/affix_mods.rs:171`), so condition would reset on every mod. Change it to take the new `GearCopy` whole. It and `locate_copy` (:148) are player-only and Weapon/Armor-only. Extract the −1/+1 swap as `fn swap_worn_copy(&mut self, wearer: Entity, slot, old: &EquippedItem, new: GearCopy)`, used by mods, wear and refit. Give the Refit a slot predicate that accepts Module.
- **Wear goes in `finish_fight`** (`E/game/combat_teardown.rs:370`) **before the dead-party loop**, because `bench_or_dissolve` strips gear. In `resolve_sortie_battle` (`E/game/sortie.rs:432`), wear `members` before `bench_or_dissolve`.
- **Hand crafting pays from the pack only** (`pay_items`). The Refit pays base-then-pack. Check every ingredient with `stores_hold` first, then spend each with `spend_from_stores` (`E/game/base_space.rs:~1028`), so the payment is all-or-nothing.
- **"Ungated"** means no `unlocks_structures` entry. Check how a free structure is offered in the build menu (structures.rs:916 group derivation) before adding one.

## Global constraints

- Read the matching `.claude/rules/seams-*.md` before editing a subsystem: base, combat, items, sieges, sorties, notifications, screens, hud, and `content-schema`. Load the `seams` skill when a rule reads as arbitrary.
- No `Game.world` accessor. New def fields get `#[serde(default)]`, and the matching `assets/*/README.md` is updated in the same commit.
- **Rest fixture fallout:** tests calling `.rest()` in base (e.g. `tests/turn.rs`, `respite.rs`) assumed a free full heal. Give them patches through a support helper (`stock_patches(game, n)`) rather than weakening their asserts. Shield and turret fixtures in `tests/{raids,siege,settlement_relations,nemesis_siege}.rs` and `assets.rs:3120` need coils the same way (`charge_defences(game)`).
- TDD: each spec test fails first. Mutation-check (`skills:mutation-testing`, fix committed) the rest's patch arithmetic, the dry-turret skip, the charged-only defence filter, and wear-before-dissolve. A green step gets `cargo fmt`, `cargo clippy --workspace --all-targets` and a commit.
- `balance_sim` must not move: `cargo test -p feral-processes-engine balance_sim` after every tuning change.
- Unset `FERAL_DEV_NO_SIEGES`. Prefer `dev-saves/` templates or `place_now` fixtures over a fresh `Game::new`.
- Subagents never push, never `git checkout`, `git stash` or `git add -A`, and stage explicit paths only.

---

## Phase 1: healing and defences (release 1, minor engine and gui)

One subagent. New test files are `E/tests/supply.rs` and `E/tests/healing.rs`.

### Task 1: The `supply` field and hauling (refactor first)
- [ ] Extract `has_machine_status()` and `spends_supply()`, and switch every site listed above to them. The suite stays green, and this is its own commit.
- [ ] Add `StructureDef::supply: Option<ItemId>`, the `intake_recipe` arm (decision 7) and the `assets/structures/README.md` entry. Author `supply: Some("charge_coil")` on `shield.ron` and `turret.ron`, and `supply: Some("patch_routine")` on `repair_bay.ron`.
- [ ] Tests: a hauler fills a shield's input to `2 × SUPPLY_BATCH` from a store, and a fuel burner still outranks it when both want.

### Task 2: `Uncharged` and charged-only defence
- [ ] Add `MachineStatus::Uncharged` to `ALL` and to every exhaustive match: `reason`, label, rank (`components.rs:1014–1133, ~3693`), `set_machine_status`/`is_blocker` (`systems.rs:925–960`), `is_machine_gap` and the order list (`E/game/base_pages.rs:11–27`), and the gui matches (grep `Unpowered` in `G/render/`).
- [ ] A writer, in the pass that already visits defences or in a small new one, sets `Uncharged` when a supply structure's input is 0 and `Running` otherwise, through `set_machine_status`.
- [ ] Gate the per-structure defence sums on holding ≥1 coil. Spend 1 coil per contributing structure per sweep (`run_raid` and `run_town_raid`) and per off-screen siege, shields first (decision 1).
- [ ] Tactical: `fire_one_turret` spends 1 coil per shot and skips with the dry line when empty.
- [ ] Tests: uncharged defences add nothing; a sweep spends; a siege spends; the garrison is unaffected; a turret shot spends and a dry turret skips; `Uncharged` raises `is_machine_gap`.

### Task 3: Paid rest and the trickle
- [ ] Tuning: `REST_INTEGRITY_PER_PATCH = 40`, `BASE_TRICKLE_PER_TICK`.
- [ ] Add `ConsumeSource::Heal` with its `as_str` arm (grep the other exhaustive users).
- [ ] Rest: gather `(entity, hp, max)` for the player plus `repaired` roles. Spend `ceil(total_missing / 40)` patches, capped by stock. Heal worst-fraction first (ties by entity), with the last patch rounding in the party's favour. Rest is never refused. The log line distinguishes full from partial.
- [ ] `run_base_trickle` runs in base, outside battle, on the same role set.
- [ ] Tests: the spec's rest and trickle list.

### Task 4: The Repair Bay spends patches
- [ ] Add `RecoveryDef.per_tick_supplied: Option<i32>` (`#[serde(default)]`), author it on `repair_bay.ron`, and document it in the README. Add `PatchCredit` and thread the entity and supplied rate through `Bays`.
- [ ] Tests: the Bay heals fast with a patch and spends one per 40 HP, and heals at `per_tick` when empty.

**Controller, end of phase 1:** an opus whole-branch review (diff as a file), `cargo test --workspace`, then a CHANGELOG entry and a `deploy`. The `seams` skill decides whether this adds a seam (supply hauling probably does: graph, skill, rules file).

---

## Phase 2: gear wear and the Refit Bench (release 2)

### Subagent A (engine), Tasks 5–7. New test file `E/tests/wear.rs`.

#### Task 5: Storage and save
- [ ] Add `GearCopy.wear` (decision 4) and `is_plain`. Add `ItemDef.durability` with `GEAR_DURABILITY = 15`, and `Game::condition_of(copy) -> (n, max)`. Document both in the items README.
- [ ] Add `wear` to all four save routes.
- [ ] Generalise `rekey_copy` and extract `swap_worn_copy`. Existing mod tests stay green, and this is its own commit.
- [ ] Tests:
  - save→load keeps `wear` on all four routes. Template: `tests/equipment.rs:1523`.
  - a save with the field stripped loads at full.
  - copies of differing wear don't stack.

#### Task 6: Wear and break
- [ ] Add `Game::wear_gear(&[Entity])`, called in `finish_fight` before the dead loop (player plus `Party`) and in `resolve_sortie_battle` before casualties. Each step goes through `swap_worn_copy`.
- [ ] `copy_bonus` returns `Some(EquipmentStats::default())` at `wear >= max`. Log "X's Y broke".
- [ ] Add `AlertKind::GearWorn`, posted when an equipped copy falls below 20% (check the gui/views exhaustive matches).
- [ ] Tests:
  - wear per fight (abstract, tactical, sortie).
  - a killed party member's gear wears and is returned.
  - break zeroes the bonus.
  - `Stats` is exact after equip, wear, break and unequip.

#### Task 7: Refit engine
- [ ] Add `assets/structures/refit_bench.ron` (cheap `core_fragment`, no research) and `E/game/refit.rs`:
  - `refit_cost(copy)`: `div_ceil(2)` of the raw `craftable.cost` + 1 coil, or decision 6.
  - `refit_copy(copy)`: presence gate as `modding_allowed`, all-or-nothing base-then-pack pay, `swap_worn_copy` or carried rekey to `wear = 0`, then `log` + `tick`.
- [ ] Tests: refit restores and charges the recipe share; a drop is charged by rank; a short payer is refused with nothing spent; Module is accepted.

### Subagent B (app-core and gui), Task 8
- [ ] Add `Mode::RefitCopy`, modelled on `A/app/mod_copy.rs`. Entry is an UPPERCASE inventory action (lowercase letters are row selectors), on a key that is free in `A/app/inventory.rs`. Register it in every `Mode` list (`A/lib.rs:~1865, 2527`, `G/render/mod.rs:1568, ~1770, ~1887`).
- [ ] `copy_name` (`E/game/combat_rewards.rs:243`) appends `n/max` and a broken marker. Check that it fits `SWAP_NAME_COLUMN` with a headless row-width test.
- [ ] Add `G/render/refit_copy.rs`, drawing through `Painter` only.
- [ ] Tests: app-core mode flow (`A/tests/refit_copy.rs`), plus one `--screenshot` check of the refit screen.

**Controller, end of phase 2:** an opus whole-branch review, the full gate, `balance_sim` unchanged, CHANGELOG, `deploy`, and the seam writes for wear (`seams-items`).
