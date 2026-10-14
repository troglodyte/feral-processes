# Fights use things up

**Status:** design approved in chat 2026-10-14; spec awaiting review.

Sub-project 1 of 3 for the base factory's missing recurring demand (graph:
`agenda:base-factory-gaps`). Sub-project 2 (settlements want things) and 3
(base upkeep, incl. the staff-upkeep must-have) get their own specs.

## Problem

The base's demand is almost all one-off: research bills, the portal, build
costs, seven delivery contracts. Once paid, machines clog —
`docs/measurements/2026-10-05-base-bench-chains-baseline.md` measured
86–96% of machine time clogged. `patch_routine` has no downstream consumer.

## Goal

Fighting draws on the factory every time: healing, gear upkeep and base
defence each spend factory output. Stakes are mostly an edge; healing is the
one soft must-have — never a hard stop.

Success: a base that fights regularly drains `patch_routine` and
`charge_coil` continuously, and a neglected factory is felt as slower
recovery, broken gear and unarmed defences.

## Non-goals

- New supply items or new assemblers — the sinks reuse `patch_routine` and
  `charge_coil`.
- A tactical role for the Shield (it is still absent from the tactical board).
- Wear in off-screen sieges (no one fights).
- Modelling wear or healing cost in `balance_sim`: it sets HP to max itself
  (`balance_sim.rs:1441`) and measures level curves, not the economy.
- Field rest: its Outlet cost and ambush roll are unchanged.

## 1. Healing spends `patch_routine`

**Rest in base** (`Game::rest`, the in-base branch of `turn.rs`):

- Power restores fully, free (power is not a limiting resource).
- Integrity costs one `patch_routine` per `REST_INTEGRITY_PER_PATCH`
  (tuning, start 40) points restored, summed across everyone the rest repairs
  (player + `InParty`/`Wielded` programs — the existing exhaustive match is
  unchanged).
- Patches come from base stock via `stock::spend_from_base` with a new
  `ConsumeSource` variant, so the ledger shows them.
- Short of patches, the party heals as far as the patches cover, worst-hurt
  first (lowest HP fraction first; ties by a stable order). Partial last
  patch rounds in the party's favour.
- Rest is never refused: `seams-combat` "priced by locale, never gated" holds.
  It still advances no clock.

**The free trickle:** a new per-tick system, while the player is in base
space and no battle is active, restores `BASE_TRICKLE_PER_TICK` Integrity to
the player and the programs a rest would repair. Tuned so a full recovery
from near-zero takes a few minutes of game time. It is the "never stuck"
floor, not a rival to paid rest.

**Repair Bay:** gets `supply: Some("patch_routine")` (see §3). With a patch
in its input it heals Downed bodies at `recovery.per_tick_supplied` (new
optional `RecoveryDef` field, `#[serde(default)]`), spending one patch per
`REST_INTEGRITY_PER_PATCH` healed. Empty, it heals at today's
`recovery.per_tick` (1) — the Bay's own trickle.

`patch_routine`'s pre-battle buff on direct use is unchanged.

## 2. Turrets and shields spend `charge_coil`

Both get `supply: Some("charge_coil")`; haulers keep them topped up to
`capacity` like fuel burners.

- **Tactical siege:** `fire_one_turret` (`siege/turrets.rs`) spends 1 coil per
  shot. Empty → no shot; a battle-log line says the turret is dry.
- **Raids and off-screen sieges:** a structure's `raid_defense` /
  `turret_defense` counts only while it holds ≥1 coil. When the defence
  actually reduces a sweep or siege, coils equal to the defence points used
  are spent from the charged structures, shields first. Applies to
  `run_raid`, `run_town_raid`'s base-side defence if it reads the same total,
  and `resolve_siege_offscreen_with`.
- An empty defence shows a machine status that raises the Production page's
  `!` (`base_pages::is_machine_gap`). Reuse `MachineStatus::Dry` only if its
  current meaning (a work node with nothing to tap) reads correctly for an
  uncharged defence; otherwise add `Uncharged`. The plan decides after
  reading `idle_machine_system`.

## 3. The `supply` field

New `StructureDef::supply: Option<ItemId>`, `#[serde(default)]`: an item this
structure spends by its own action and that haulers deliver to its input
buffer. `systems::intake_recipe` treats it like `power_upkeep` so the hauling
path is shared, not duplicated. Hauling priority: a new want right after
fuel suppliers in `schedule_base_labour`'s order. `assets/structures/README.md`
documents it.

`power_upkeep` stays as is (burned on a clock by `burn_grid_upkeep`);
`supply` is spent per action. Whether the two share one intake helper is a
plan-time detail; there must be one hauling path.

## 4. Gear wear

- **Storage:** `items::GearCopy` gains `condition: u8`, a fifth key axis
  beside `quality` (integer, `Eq`). Copies of differing condition no longer
  stack. `GearCopySave.condition` is `#[serde(default)]`, defaulting to
  "full"; full is resolved at load against the item's durability. Needs a
  real save→load test, not only a RON round trip.
- **Durability:** `GEAR_DURABILITY` in `tuning.rs` = 15 fights. Optional
  per-item override `durability: Option<u8>` on the equipment def,
  `#[serde(default)]`; `items/README.md` documents it. A per-item
  `unbreakable: true` exempts gear that must not wear (the plan audits class
  kits and the wielded-program easter egg).
- **When:** once per fight, win or lose, in `Game::finish_fight` (covers the
  abstract model, tactical fights and tactical sieges). Every equipped copy
  on each participant — player, `InParty`, `Wielded` — loses 1.
  `resolve_sortie_battle` calls the same `wear_gear` so sortie gear wears
  too.
- **Stats:** each step goes through `rekey_copy`'s −1/+1 pattern;
  `seams-items` "no stats operation while a gear bonus sits in `Stats`" holds.
- **Breaking:** at 0, `copy_bonus` returns zero; the copy stays equipped and
  inert. Log line "X's Y broke". Condition is shown as `n/max` wherever a
  copy is listed; broken copies are marked. An alert fires when any equipped
  copy falls below 20%.

## 5. The Refit Bench

New structure `assets/structures/refit_bench.ron`, ungated, cheap
`core_fragment` build cost. Player-operated and instant, Mod Bench pattern
(`affix_mods::locate_copy` / `rekey_copy`): pick a copy (carried or equipped),
pay, condition returns to full.

- **Cost, craftable gear:** `REPAIR_RECIPE_SHARE` (50%) of the recipe,
  rounded up per ingredient, plus 1 `charge_coil`.
- **Cost, uncraftable gear (drops):** tier-scaled `core_fragment` +
  `charge_coil` (tuning constants).
- Paid from base stock and pack, the same sources hand crafting uses.

## Testing

TDD per piece. Unit tests in the engine for: rest spends patches by HP
restored, partial-coverage worst-hurt-first, rest never refused; trickle
heals only in base and out of battle; Bay fast-with/slow-without; turret
spends per shot and skips when dry; raid defence counts only charged
structures and spends; wear per fight incl. sortie, break at 0 zeroes the
bonus with stats exact after equip/unequip; Refit restores condition and
charges recipe share; save→load keeps condition, old save loads at full.
`balance_sim` must not move. App-core/gui: Refit Bench mode and the
condition display.

## Build order

1a (healing + defences + `supply`) ships first; 1b (wear + Refit Bench, the
save change) second. Two releases.
