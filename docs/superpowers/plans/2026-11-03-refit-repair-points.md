# Refit Bench: repair points, slow mending, bench-only

Branch `worktree-refit-repair-points`. Crates: engine, app-core, gui. Save:
one additive `#[serde(default)]` field, no `SAVE_FORMAT_VERSION` bump (the
save is field-named RON, `save/mod.rs:814`).

## Why

User report (save `save_1793443404.bin`): the bench held 18 cores and never
touched their gear. Root cause: `worn_for_auto_refit` only serves copies
below `GEAR_WORN_ALERT_PERCENT` (20%); the worst copies sat at 4/15 = 27%.
That gate exists only because one core bought a whole repair however light
the wear (commit 96f69d22). User decisions (2026-11-03):

- 1 Annealed Core = **50 repair points**; 1 point = 1 unit of `wear`.
- Mending restores **1 point per tick** while the player stands in reach.
- The instant menu refit is **removed**; mending is bench-only.

Settled here: with per-point cost there is no waste, so the bench serves
**any** copy with `wear > 0`, and the alert-line gate goes.

## Design

- **Charge lives on the bench.** New component `RefitCharge(u32)` in
  `components/` (base-structure submodule), saved as
  `StructureSave::refit_charge: u32` (`#[serde(default)]`, written only for
  a bench). Restored on load; absent → 0.
- **Tuning** (`tuning/items.rs`): `REFIT_CORES` → `REFIT_POINTS_PER_CORE: u32
  = 50` and `REFIT_POINTS_PER_TICK: u8 = 1`. `REFIT_IDLE_TICKS` and
  `REFIT_REACH` stay.
- **A bench is serviceable** if `charge > 0 || stock cores >= 1`.
  `refit_benches_in_reach` returns `(Entity, charge, cores)`; serving picks
  the first serviceable one in `(x, y)` order (unchanged ordering rule).
- **One tick of service** (`mend_next_worn`, stays gather/apply over a pure
  step): if charge is 0, spend 1 core via `spend_supply(.., Refit)` and add
  50. Take the head of `worn_for_auto_refit` (same order: player, party in
  `Party` order, pack; most worn first) and reduce its wear by
  `min(REFIT_POINTS_PER_TICK, wear, charge)` through `rekey_copy`. Spend that
  many points. Log `"The Refit Bench mended {name}."` (with owner) **only
  when the copy reaches wear 0**.
- Pure helper for the arithmetic, tested directly:
  `fn refit_step(charge: u32, cores: u32, wear: u8) -> RefitStep { spend_core:
  bool, restored: u8, charge_after: u32 }`.
- **Out-of-cores warning**: same once-per-stand timing; fires when work is
  waiting and every bench in reach has `charge == 0 && cores == 0`.
- `RefitStand::mended` keeps its meaning: "this tick restored a point".
- **Removed:** `Game::refit_copy`, `refit_cost`, `stocked_refit_bench`,
  `refit_bench_cores`, `WornAt::Pack` paths only it used (keep what the bench
  uses), `REFIT_CORES`. Delete, no shims.
- **New read for the screen:** `Game::refit_bench_reserve() -> Option<(u32,
  u32)>`, the (charge, cores) of the bench that would serve: the first
  serviceable bench in reach, else the first bench in the base. `None` if
  no bench stands.

## Phase 1: engine (one context, TDD)

Files: `game/refit.rs`, `tuning/items.rs`, `components/<base submodule>`,
`save/mod.rs` (+ restore in `game/lifecycle/load.rs` or wherever
`StructureSave` rebuilds components; grep `stock_input`),
`tests/refit_auto.rs`, `tests/wear.rs`, `tests/save_roundtrip.rs`, and any
test calling `refit_copy` (`rg -l refit_copy crates/engine`).

Tests first, each red before its code:
1. `refit_step` table: empty charge + core → spends core, charge 49; charge
   with wear 1 → restores 1; charge 0 and no cores → restores 0.
2. Bench mends a 27%-condition copy (the reported state: wear 11 of 15). It
   fails today.
3. A copy at wear 4 takes 4 ticks after the idle wait to reach 0. It logs
   once, on the last tick.
4. One core covers 50 points: 18 copies' worth of wear across ticks spends
   exactly `ceil(total/50)` cores.
5. Charge survives save→load (real save path, not a RON round trip — see
   memory `ron-round-trip-cannot-catch-a-skipped-field`).
6. Rework the existing `refit_auto.rs` tests that assume "one copy per tick,
   full mend" and delete
   `a_copy_above_the_alert_line_is_left_alone_but_manual_refit_takes_it`.
   Keep the ones about reach, order, the wait, and the empty warning.

Gate: `cargo test -p feral-processes-engine refit wear save`. Then check
`balance_sim` (it should not move). The engine compiles alone, but the
workspace will not until phase 2. Commit.

## Phase 2: app-core + gui

- app-core `app/refit_copy.rs`: Esc → back; no Enter action. Keep
  `Mode::RefitCopy` as a read-only view (avoids the all-modes length merge
  trap). Check `inventory.rs:243,374` key hints and wording (`R` opens the
  view, not "refit").
- gui `render/refit_copy.rs`: rows = name, `Condition: {left}/{max}`, then
  `Refit Bench: {charge} repair points, {cores} Annealed Cores` or the
  no-bench line, then `"Stand by a Refit Bench to mend it."` when worn. Hints
  `["Esc back"]`. Update its two tests. Keep the 1280×720 width test.
- `rg -n "refit" crates/app-core/src/tests crates/gui/src` for the fixtures
  `app_at_a_refit_bench_wearing` / refusals tests that press Enter; rewrite
  or delete them.
- Text sweep: `rg -n -i "refit|annealed" assets/ crates/*/src --glob '!*.rs'`
  plus `assets/items/annealed_core.ron` and `assets/structures/README.md`
  §"The Refit Bench" (lines ~848-870: cost, `REFIT_CORES`, menu refit, alert
  line). Help markdown under `assets/` if it mentions the menu refit.

Gate: `cargo fmt`, `cargo clippy --workspace --all-targets`,
`cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES`). Commit.

## Phase 3: review + land

Opus whole-branch review (diff as a file). Then the `deploy` skill: patch
bump (no save break), CHANGELOG entry: "Refit Bench mends any worn gear
slowly while you stand by it; a core holds 50 repair points; menu refit
removed."
