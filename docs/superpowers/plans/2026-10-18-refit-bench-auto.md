# Plan: the Refit Bench holds cores and mends on its own

Spec: `docs/superpowers/specs/2026-10-18-refit-bench-auto-design.md`. Read it
first; this plan carries files, interfaces, test intent and gates, not code.

Conventions for every task: TDD (failing test, then code), `cargo fmt`,
`cargo clippy --workspace --all-targets` clean, commit at green. Iterate with
`cargo test -p feral-processes-engine <name>`; the full gate is Phase 4.
`unset FERAL_DEV_NO_SIEGES` before any test run. Never `git checkout`
another agent's work; never push.

Anchors the explorer verified (line numbers are 0.19.9):
`StructureDef::supply` `structures.rs:547`, `supply_batch()` `:1037`;
`hauling::input_capacity` `hauling.rs:911`; `fuel_wants`
`work_orders.rs:1915`; `spend_supply` `game/base/stock.rs:355`;
`StructureReport.supply` `inspection.rs:1662`; `refit.rs` whole file;
`locate_in_slot`/`rekey_copy`/`swap_worn_copy` `affix_mods.rs:163-219`;
pack helpers `crafting.rs:676-722`; `Equipment` `components.rs:510`;
`Party` `resources.rs:1772`; `run_repair_bays` call `turn.rs:336`;
`Game::base_pos` `stack.rs:399`; `in_reach` `offshift.rs:558`;
`EntityView::recovering` `views.rs:1254` set in `inspection.rs:1351,1411,1516`;
`rising_mark` `gui/render/marks.rs:543`, `draw_rising_mark` from
`render/base.rs:1412`; palette `gui/render/hud/palette.rs`;
`give_to_adjacent` `deposit.rs:137`; `structure_unlocked` `catalog.rs:811`,
refusal in `building.rs:77`; ungated list `tests/research.rs:472`;
cue queues initialised at `lifecycle.rs:576-582` and `1591-1597`.

## Phase 1: the bill and the stock (engine + assets)

### T1. `supply_capacity` on `StructureDef`
- `structures.rs`: `pub supply_capacity: Option<u32>` with `#[serde(default)]`.
- `hauling::input_capacity` gains a def-aware caller: for a `supply`
  structure the cap is `def.supply_capacity.unwrap_or(supply_batch * INPUT_STOCK_BATCHES)`.
  Thread it through `input_room`, `intake_recipe`'s consumers and
  `StructureReport.supply` (`inspection.rs:1662`). Haul trigger in
  `fuel_wants` keeps `SUPPLY_BATCH`.
- `assets/structures/README.md`: document the field under `supply`.
- Tests (`tests/supply.rs`, fixture `base()`): a def with `supply_capacity:
  Some(20)` reports capacity 20; a crew fills it to 20 and never 21; a def
  without the field still reports `SUPPLY_BATCH * INPUT_STOCK_BATCHES`.
  Check `tests/inspection.rs:1754-1769` still passes.

### T2. One core, paid from the bench
- `assets/structures/refit_bench.ron`: `supply: Some("annealed_core")`,
  `supply_capacity: Some(20)`, new description.
- `assets/items/annealed_core.ron`: description gains the mending clause.
- `tuning.rs`: delete `REFIT_RECIPE_SHARE_PERCENT`, `REFIT_COILS`,
  `REFIT_DROP_FRAGMENTS_PER_RANK`; add `REFIT_CORES: u32 = 1`.
- `items.rs` `ids`: `ANNEALED_CORE`.
- `refit.rs`:
  - `refit_cost` returns `[(annealed_core, REFIT_CORES)]` for any gear copy,
    empty for non-gear.
  - `stocked_refit_bench(&self) -> Option<Entity>`: first bench in `(x, y)`
    order whose `Stock.input[annealed_core] >= REFIT_CORES`.
  - `mend_located(&mut self, at: WornAt, bench: Entity) -> GearCopy`: the one
    mend. `WornAt::Slot(wearer, slot, EquippedItem)` goes through
    `swap_worn_copy(wearer, slot, …)`; `WornAt::Pack(copy)` through
    `take_copies` + `add_copies`. Spends `REFIT_CORES` via
    `spend_supply(self, bench, ConsumeSource::Refit)`. No tick, no log.
  - `refit_copy` keeps its guards, locates with `locate_in_slot`, refuses
    "Load the Refit Bench with Annealed Cores first." when
    `stocked_refit_bench` is `None` (after the "Build a Refit Bench first."
    check), then calls `mend_located`, logs, ticks.
- `gui/render/refit_copy.rs`: a row "Refit Bench holds N Annealed Cores".
- Tests: rewrite `tests/wear.rs:507-600` bill assertions (craftable, drop,
  rare all cost one core); manual refit decrements the bench stock by 1;
  refused with a bench standing but empty; refused with no bench;
  `app-core/src/tests/refit_copy.rs` bill expectations; a gui row test if
  the file has a precedent.

### T3. The research gate
- `assets/research/program_refactoring.ron`: `min_zone: 1`,
  `unlocks_structures: ["refit_bench"]`, description mentions the bench.
- `tests/research.rs:444-480`: drop `refit_bench` from the ungated list and
  rewrite the comment. New test modelled on `tests/prison.rs:~864`:
  `structure_unlocked("refit_bench")` false, then true after
  `unlock_research_chain(game, "program_refactoring")`; `place_structure`
  refuses with "hasn't been researched yet."
- `assets/research/README.md:~112-114`: "the three benches" becomes four,
  naming `program_refactoring`.
- `assets/structures/README.md:806-819`: rewrite the Refit Bench section
  (gated, stocked, one core, auto-mend summary).
- Run `cargo test -p feral-processes-engine assets research` and fix any
  census that pinned the old zone or the ungated bench.

## Phase 2: automatic mending (engine)

### T4. `run_refit_bench`
- `tuning.rs`: `REFIT_IDLE_TICKS: u32 = 2`, `REFIT_REACH: i32 = 2`, with
  doc comments saying why (the spec's Anti-goals).
- `resources.rs`: `RefitStand { cell: Option<(i32, i32)>, ticks: u32 }`,
  `Resource`, not saved; insert in both `lifecycle.rs` sites.
- `refit.rs`:
  - `worn_for_auto_refit(&self) -> Vec<WornAt>`: player slots, then `Party`
    slots, `EquipmentSlot::ALL` order, filtered to broken or at/below
    `GEAR_WORN_ALERT_PERCENT` remaining (reuse the alert's own predicate;
    extract it if inline); stable-sorted by `wear` descending; then pack
    `GearCopies` rows with the same filter and sort.
  - `refit_bench_serving(&self) -> Option<Entity>`: `Some(bench)` when in
    base space, no battle, `RefitStand.ticks > REFIT_IDLE_TICKS`, a stocked
    bench within Chebyshev `REFIT_REACH` of `base_pos()` (first in `(x, y)`),
    and `worn_for_auto_refit` non-empty. Pure read; the view uses it too.
  - `run_refit_bench(&mut self)`: update `RefitStand` from `base_pos()`
    (changed cell or not in base resets), then if `refit_bench_serving` is
    `Some`, `mend_located` the first `WornAt` and log. If ticks crossed the
    threshold this tick, work exists and no bench is stocked, log "The
    Refit Bench is out of Annealed Cores." once.
- `turn.rs:336`: call after `run_repair_bays`, before `run_base_trickle`.
- Tests (new `tests/refit_auto.rs`, fixture from `tests/wear.rs::refit_game`
  plus `spawn_structure_at` and a stocked `Stock.input`): ticks 1 and 2 no
  mend, tick 3 mends; a step resets; 3 tiles away never; empty bench never
  and logs once; order (player weapon before more-worn party armour, party
  before pack, most worn first within equipped); threshold (a copy above
  the line untouched); a mended pack copy becomes plain and lands in
  `Inventory`; one core spent per mend. Use `spawn_tamed_on_map` for party
  bodies so views exist.

### T5. `EntityView::refitting`
- `views.rs`: `pub refitting: bool` beside `recovering`, documented as
  derived from `Game::refit_bench_serving`.
- `inspection.rs`: set it for the player view only (`is_player`) at the
  three sites that set `recovering`.
- Test: true on a tick where `run_refit_bench` would mend, false after a
  step and false with an empty bench.

## Phase 3: renderer and hand loading

### T6. The purple mark
- `palette.rs`: `pub(crate) const MENDING: Color`, a purple passing
  `the_reserved_roles_are_separable` and
  `no_content_hue_takes_a_reserved_role_by_accident`; if none fits,
  `rising_mark` uses `palette::glyph(GlyphColor::Magenta)` and the spec's
  fallback is noted in the commit.
- `marks.rs::rising_mark`: `ev.refitting` yields `('+', MENDING)`;
  `recovering` keeps precedence if both are set.
- gui test beside the existing `rising_mark` tests.

### T7. Transfer loads the bench
- `deposit.rs::give_to_adjacent`: a `supply` structure beside the player
  accepts its supply item into `Stock.input` up to the def's capacity;
  Depots unchanged. Check `Game::transfer_items` (`transfer.rs:299`) and
  the Transfer view list the bench's row; label left to the builder.
- `work_orders.rs::fuel_wants`: Phase 1 found the `held < SUPPLY_BATCH`
  trigger tops an authored-capacity bench to only 5-7. For a structure
  with `supply_capacity` authored, also file the want while
  `capacity - held >= HAUL_CARRY_CAPACITY` (a full trip fits) and a Depot
  holds at least that much. Structures without the field keep the old
  trigger exactly, so turrets are untouched.
- Tests: engine, player beside a bench puts 20 cores in; the 21st is
  refused and stays in the pack; a non-supply item is refused. Hauling: a
  bench at 0 with a Depot holding 25 cores ends at 20 and the Depot at 5;
  a turret's trigger is unchanged (existing supply tests stay green).
  `app-core/src/tests/` Transfer test if a precedent fixture exists
  (`app_at_a_refit_bench*` in `support.rs:3604-3700`).

## Phase 4: copy and the gate

### T8. Docs and the full gate
- `CHANGELOG.md`: an entry under the unreleased heading in the file's
  voice (a gated, stocked bench; one core; auto-mend; purple `+`;
  Program Refactoring in zone 1).
- Save round-trip test: a bench with 7 cores loads with 7
  (`tests/save_roundtrip.rs`).
- `cargo test --workspace`, `cargo clippy --workspace --all-targets`,
  `cargo fmt --check`, `cargo test -p feral-processes-engine balance_sim`.
- Final whole-branch review (opus) against the spec's Anti-goals and the
  seams named in its Constraints; mutation-check the idle threshold and
  the ordering tests (remove the sort, the test must fail).
