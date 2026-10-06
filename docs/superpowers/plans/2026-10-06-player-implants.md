# Player implants — plan

Spec: `docs/superpowers/archive/specs/2026-10-05-player-implants-design.md`. Read it
once. This plan adds only the decisions, the corrections to the spec and the
file and test lists. Paths are relative to `crates/engine/src` unless they
name another crate.

**Execution:** five phases, one sonnet subagent each, run serially because
each uses the previous phase's types. Per-task review gates are off. One opus
whole-branch review runs at the end. Every task: TDD with the failing test
first, a commit per green step, `cargo fmt` and `cargo clippy --workspace
--all-targets`, then that phase's tests. Run the full `cargo test --workspace`
at the end of P1, P3 and P4. Every dispatch: **never push**, stage explicit
paths only, and never stage `art/` (the user's untracked work).

## Decisions (the spec's open questions, answered by the user 2026-10-06)

1. **Black Ledger's `DropBoost` applies only in the Stack.** The hook reads
   `DropBoost(pct)`, and the read site gates on `self.is_underground()`
   (`game/stack.rs:380`). There is no scope flag in data: every `DropBoost`
   applies only in the Stack, and the README says so.
2. **Implant upkeep is multiplied:** `LowPowerMode` reduces it too. Drain
   per tick = `HUNGER_DECAY_PER_TICK × perk_mult × implants::drain_factor(load)`,
   where `drain_factor(load) = 1 + load × IMPLANT_DRAIN_PER_LOAD`. Write one
   pure `implants::power_multiplier(perks, load)` (the perk multiplier times
   the factor). `systems.rs:52`, `game/crafting.rs:300` and
   `tests/kit.rs:254` all call it. `power_drain_per_tick`'s signature is
   unchanged.
3. **Items link to implants through `ItemDef::implant: Option<ImplantId>`**,
   with `#[serde(default)]`.
4. **The seam is written in P5,** after the code exists.

## Corrections to the spec (verified against the source 2026-10-06)

- **A. Accuracy and evasion are not baked into `Stats`.** They are read live
  from `gear_bonus(e)` at five sites: `game/combat_damage.rs:91` and `:137`,
  `game/inspection.rs:2253` and `:2262`, and `game/catalog.rs:544`. The
  comment at `game/crafting.rs:715` forbids baking them. Add
  `Game::hit_bonus(e) -> (accuracy, evasion)` (gear plus implants) and switch
  those five sites to it. All other gear fields stay on `gear_bonus`.
  `ImplantStats` has `max_hp, atk, mitigation, max_power, crit,
  status_resist, decompiler, accuracy, evasion`, all `i32` (`f32` where
  `DerivedStats` is a float) with `#[serde(default)]`.
- **B. Where `recompute_derived` takes the deltas.** In
  `recompute_derived` (`game/derived.rs:345`), add them to the local
  `derived` (a `DerivedStats`) right after `derived_stats(entity)` (`:349`).
  That way `Stats`, `Decompiler::skill`, the inserted `Derived` and
  `PowerReserve::restore` all see them. Do not add them at the
  `BoughtStats` lines.
- **C. The Splice Rig opens by adjacency, like the Teardown Rig.** That
  path is a map key → `App::open_rig_tool` (`app-core/src/app/rig_tool.rs:39`)
  → `Game::adjacent_teardown_rigs()` (`game/extraction.rs:1018`). The Mod
  Bench does not open this way. P4 picks an unused map key and greps the
  app-core keymap and the help pages before taking it.
- **D. Recipes live in the research node** (`unlocks_recipes`), gated by
  the node's `min_zone`. There is no `assets/recipes/`.
- **E. `needs_tick_system` is a bevy system.** It gains `Option<&Implants>`
  in its player query and `Res<ImplantDb>`. Load is computed through
  `implants::load_of(&Implants, &ImplantDb)` and is not cached.
- **F. `DeadMansSwitch` hooks `apply_damage`** (`game/combat_damage.rs:363`):
  the player only, only while `has_active_battle()`. It does not hook
  `lower_hp`, so `kill_outright` (dying inside rock) still kills. When a hit
  would take HP from above 0 to 0, clamp to 1 and spend
  `DEAD_MANS_SWITCH_POWER` from `PowerReserve`, if the player has it. The
  per-battle flag goes in `BattleState` (`resources.rs:1419`), which is not
  saved.
- **G. Removal is paid in `core_fragment`**, which is the zone-local currency
  and is lost at a breach. That is intended. The help page says so.

## Constraints every phase holds

- **The RNG stream must not shift for a player without implants.** Roll
  rejection and `BattleStartStatus` only when overloaded or when an
  installed implant has a `BattleStartStatus`. If the stream shifts, seed-luck
  tests start failing (see the memory note).
- **`balance_sim` stays unmoved.** Check `cargo test -p
  feral-processes-engine balance_sim` after P1 (new tuning constants) and
  after P2.
- **An `ImplantId` whose def is missing contributes nothing everywhere.**
  Every reader goes through `ImplantDb::get` and skips a `None`.
- **No `SAVE_FORMAT_VERSION` bump.** `PlayerSave::implants` is additive
  behind `#[serde(default)]`, the same reasoning as `sorties` (`save.rs:~161`).

## P1 — Content kind, state, save

Files:
- new `implants.rs`: `ImplantId` (transparent newtype, same shape as
  `ItemId` at `items.rs:24`), `ImplantDef`, `ImplantStats`, `ImplantHook`,
  `ImplantSignature`, `ImplantDownside`, `ImplantDb::load_dir` (copy
  `StatusDb::load_dir` at `statuses.rs:109`), and the pure functions:
  `load_of`, `load_cap(level)`, `overload(load, cap)`, `drain_factor`,
  `power_multiplier`, `rejection_chance(overload)`,
  `trace_scaled(amount, net_pct)`.
- `components.rs`: `Implants { installed: Vec<ImplantId> }`.
- `game/lifecycle.rs`: `Implants::default()` in the inner tuple of
  `spawn_player` (`:145`). Restore in `spawn_player_from_save` (`:284`).
  Add the DB to `AssetDbs` (`:3479`), `load_asset_dbs` (`:3516`) and both
  destructures (`:459` and `~:1372`).
- `save.rs`: `PlayerSave::implants`.
- `items_db.rs`: `ItemDef::implant`.
- `tuning.rs`: `IMPLANT_LOAD_BASE` (4), `IMPLANT_LOAD_LEVELS_PER_POINT`,
  `IMPLANT_DRAIN_PER_LOAD`, `REJECTION_CHANCE_PER_LOAD`,
  `REJECTION_CHANCE_MAX`, `REJECTION_STATUSES` (`throttled`, `exposed`,
  `stun`; all exist in `assets/statuses/`), `IMPLANT_REMOVAL_FRAGMENTS_PER_LOAD`
  and `DEAD_MANS_SWITCH_POWER`. Set `LEVELS_PER_POINT` so the cap reaches 16
  at the middle of the level band `balance_sim` models. Write that reasoning
  in the constant's doc comment.
- `assets/implants/` holds six `.ron` files and a `README.md`. Six item
  `.ron` files go in `assets/items/`, each with `implant: Some(..)` and
  `craftable` matching the existing gear. Update `assets/items/README.md`.

Tests:
- Unit tests for each pure function.
- `ImplantDb`: a malformed file is skipped with a warning; a missing
  directory gives an empty DB.
- Save → load round trip with implants, through the real save path, not just
  RON (see the memory note). An old save loads with none. A save naming a
  missing implant loads, keeps the id and contributes nothing.
- `tests/assets.rs`: six implants; every implant ↔ item link resolves both
  ways.

## P2 — Effects at their seams

Each row starts with a failing test that drives the real `Game` from a
`dev-saves/` template where one fits.

| Effect | Site | Test intent |
|---|---|---|
| `stats` | `recompute_derived`, the B position | Install, then remove, restores every field exactly, including `max_power` and `crit` |
| accuracy/evasion | `hit_bonus` at the five A sites | Dermal Lattice lowers evasion as seen by the combat profile and the manifest |
| drain | `power_multiplier` at all three callers | Drain scales with Load; `LowPowerMode` reduces it too |
| `CaptureOdds` | `player_decompiler_bonuses` (`game/unlocks.rs:100`), added to `capture_boost_pct` | Chance rises by exactly the hook's pct |
| `DropBoost` | `equipment_drops_for` (`game/combat_rewards.rs:100`), beside the field buff, gated on `is_underground` | Boosted in the Stack, unchanged on the surface |
| `XpBoost` | `combat_rewards.rs:1007` (player only, not `:1139`) | Unused by the first six; one test with a test def |
| `RoutineSlots` | `routine_slots` player arm (`game/combat.rs:1074`), past the clamp | +1 slot |
| `TraceRise`/`TraceDamp` | `raise_trace` (`game/trace.rs:97`): `trace_scaled` first, then `trace_after_obfuscation` | Rises faster; the obfuscation floor still holds |
| `BattleStartStatus`, rejection | `begin_battle` (`game/combat.rs:524`), after the intercept log; `arm_status` + `log_status_landing`; `GameRng` | Seeded: lands at chance 1 and never at 0; not overloaded means no roll and no RNG draw |
| `DeadMansSwitch` | `apply_damage`, per F | Survives once per battle at 1 HP and spends Power; the second lethal hit kills; `kill_outright` kills |

- `ImplantSignature` gets the exhaustive-match guard modelled on
  `perks.rs:847` (`one_level_is_worth_something`): a match with no wildcard
  arm, run by a test over every variant.
- If any hook needs the same check at two sites, stop and report; do not
  duplicate the check.

## P3 — Station, research, API

- `assets/structures/splice_rig.ron`, modelled on `mod_bench.ron`
  (`work: None`, small `power_draw`, fragment build cost).
- Research: `wetware_splicing` has `min_zone` 1 or 2, a low-tier `requires`,
  `unlocks_structures: ["splice_rig"]` and three recipes (Ripper Fibers,
  Ghost Handshake, Dermal Lattice). A second node (`min_zone: 3`, requires
  the first) unlocks the other three. Costs come from the player's own lines.
  Any node at zone 2 or above needs a `ZONE_MATERIALS` cost, and
  `requires_structure` should be the fabricator unless the source says
  otherwise.
- Censuses that move with this, all in `tests/assets.rs`: the gear-recipe
  count at `:2041` (`checked` goes up), `:5032`, the discoverable list at
  `:5384`, `:5447`, `:4931`, `:5157` and `:5257`. Update the counts and lists
  on purpose, not by loosening the assertions.
- New `game/implants.rs` exposes:
  - `implant_view() -> ImplantView` (installed rows, known or unknown; load,
    cap and overload; per-implant upkeep and downside text; installable items
    from cargo)
  - `install_implant(&ItemId) -> Result<(), String>`
  - `remove_implant(&ImplantId) -> Result<(), String>`
  - `adjacent_splice_rig() -> bool`

  Gate on `has_structure("splice_rig")`, refuse during a battle, and copy the
  Mod Bench refusal wording style. Removal costs `core_fragment` through
  `pay_items` (`game/crafting.rs:514`). Every refusal happens before
  anything is spent. Install and remove both call `recompute_derived`.
- Tests: the install/remove round trip (item consumed and returned, fragments
  spent, install materials not refunded), and one test per refusal (no rig,
  in battle, too few fragments, unknown id, item that is not an implant),
  each asserting the inventory is unchanged.

## P4 — app-core and gui

- app-core: `Mode::SpliceRig` with a handler at `app/splice_rig.rs`, opened by
  the map key chosen per C. It lists installed implants and installable
  items; uppercase actions Install and Remove (lowercase letters select
  rows, per the memory note). Installing past the cap needs a confirm that
  names the rejection risk. Add the mode to `all_modes` (see the memory
  notes).
- gui: `render/splice_rig.rs`, drawn through `Painter` only. It shows Load
  as used/cap with overload in the warning colour, plus upkeep and downside
  per row. `render/points.rs` gets a Load row. HUD: one glyph while
  overloaded, in `render/hud/status_bar.rs`.
- `assets/help/` gets an implants page, covering the fragment price and
  that it is lost at a breach.
- Tests: app-core flow (open, install, confirm on overload, remove). Use a
  headless row-width check if the screen has a popup.
- Take one `--screenshot` of the screen from a template with implants in
  cargo, and Read it.

## P5 — Docs, seam, landing prep

- Seam `implants`, using the `seams` skill's three writes in its order:
  `seam:implants` in the graph, the skill reference, then
  `.claude/rules/seams-implants.md`. Rule: implant effects are read only at
  P2's sites, through `ImplantDb`.
- `CHANGELOG.md` gets an unversioned entry; the version goes on at landing.
- Spec status becomes built. Update its `INDEX.md` row.
- Opus whole-branch review, with the diff given as a file. Then the full
  `cargo test --workspace`, with `FERAL_DEV_NO_SIEGES` unset.
