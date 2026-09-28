# Reinitialization Protocol — plan

Spec: `docs/superpowers/specs/2026-09-28-reinitialization-protocol-design.md`.
Branch: `reinitialization-protocol` (spec already committed there).
TDD throughout, a commit per green step. No save-format change.

Two phases, one sonnet dispatch each. Each ends green on
`cargo test -p <crate>` + `cargo clippy --workspace --all-targets` +
`cargo fmt`. After phase 2: `cargo test --workspace` once, then
`cargo test -p feral-processes-engine balance_sim` (new item). Final review:
opus, whole branch. Dispatches must forbid push.

## Anchors (verified 2026-09-28)

| What | Where |
|---|---|
| `DownedProgram { species, level, rarity, boss, condition, carried: Option<AbilityId> }` | `engine/src/items.rs:361` |
| `DownedPrograms(Vec<DownedProgram>)` (on the player) | `engine/src/components.rs:602` |
| `extract_program` (refusal order, index-into-store, log) | `engine/src/game/extraction.rs:802` |
| `spawn_wild_creature_scaled` (rolls potential → `roll_wild_routine` → `roll_rarity`, all `GameRng`) | `engine/src/game/spawning.rs:268`; callers: `arena/setup.rs:194`, `game/lifecycle.rs:3345`, `adopt_program`, tests |
| `adopt_program` (strips `Hostile`/`WanderAi`, `roster_parts`, `install_innate_routines`) | `spawning.rs:548`; callers `arena/mod.rs:121`, `stack_features.rs:468`, `caravan.rs:1339`, `zone.rs:356`, tests |
| `install_innate_routines` — treats pre-existing `Routines` as *carried*, kit fills around it | `engine/src/game/combat.rs:1119` |
| `arena::set_level` (cap = `level_cap().max(arena_level_ceiling())`; calls `install_unlocked_routines`) | `engine/src/arena/mod.rs:69`; re-exported `tests/support.rs:193` |
| `Game::level_cap` (zone cap) | `engine/src/game/party.rs:582` |
| `roster_room()` (= `ROSTER_HARD_CAP − pet_count`) | `engine/src/game/catalog.rs:884` |
| Spending one item: `Inventory::take(id, 1)`; counting: `inv.items` | `combat_rewards.rs:1331`, `unlocks.rs:102` |
| `Deed::Tamed` = "Decompile a wild program" | `engine/src/game/contracts.rs:1214` |
| `handle_downed_programs_key` (`None` list / `Some(i)` tool page; `report(outcome)`) | `app-core/src/app/extraction.rs:20` |
| `selected_index` — lowercase/digits only; uppercase free for actions | `app-core/src/app/input.rs:107` |
| Uppercase action precedent `GameKey::Char('A')` | `app-core/src/app/caravan.rs:178` |
| `extraction_options_rows` (per-record page) | `gui/src/render/extraction.rs:91` |
| Help pages | `assets/help/extraction.md`, `assets/help/supplies.md` |

## Decisions the spec left to the plan

- **Carried routine:** no separate install call. The pinned spawn puts
  `Routines(record.carried)` on the body; `install_innate_routines` already
  keeps pre-existing routines as the carried prize and fills the kit around
  them. `None` → empty `Routines`, identical to a non-carrier.
- **Contracts:** no `Deed` noted. `Deed::Tamed` reads "Decompile a wild
  program"; this isn't one.
- **Level cap:** clamp to `Game::level_cap()` (zone cap), not the arena
  ceiling — a resurrected program obeys the same cap as the rest of the
  roster.
- **Potential** rolls fresh (the record doesn't carry it); attributes mint as
  usual. Only rarity and routines are pinned.
- **Blocker strings** are row fragments (`ability_unavailable`'s style), shown
  after the action label.

## Phase 1 — engine + asset

Files: `assets/items/reinitialization_protocol.ron` (new),
`assets/items/README.md` (only if it lists items by role),
`engine/src/tuning.rs`, `engine/src/game/spawning.rs`,
`engine/src/arena/mod.rs`, `engine/src/game/party.rs` (level fn home),
`engine/src/game/extraction.rs`, `engine/src/tests/extraction.rs`,
`engine/src/tests/support.rs` (re-export follows the move).

1. **Asset.** `id: "reinitialization_protocol"`, name/description per spec,
   `value: Some(..)` in line with peers, `craftable: Some((cost:
   [("ice_breaker",1),("logic_wafer",2),("charge_coil",1),("cache_grain",1)],
   requires_structure: Some("fabricator")))`. No `taming_potency`,
   `droppable`, `cache_drop`. Test: loads from real assets; `taming_catalyst`
   never returns it (inventory holding only the protocol → `None`).
2. **`reinit_level(level: u32, condition: u8) -> u32`** pure fn (in
   `tuning.rs` beside `REINIT_LEVEL_FLOOR: f32 = 0.5`, or the extraction
   module if `tuning.rs` holds only constants there — follow neighbours).
   `max(1, round(level × lerp(FLOOR, 1.0, condition/100)))`. Tests: cond 100
   keeps, cond 0 halves (Lv10→5), Lv1 cond 0 → 1.
3. **Pinned spawn.** `pub(crate) struct SpawnPins { rarity: Option<Rarity>,
   routines: Option<Vec<AbilityId>> }`, `Default`. New
   `spawn_wild_creature_pinned(.., pins)`; `spawn_wild_creature_scaled`
   becomes a one-line delegate with `SpawnPins::default()`. A pinned field
   **skips its roll** (no `GameRng` draw); unpinned is byte-for-byte today's
   order, so existing callers' RNG streams don't move. Same for
   `adopt_program` → `adopt_program_pinned`. Tests: pinned rarity lands as the
   `Rarity` component and its `stat_mult` is in `Stats`; existing suite green
   untouched.
4. **Level move.** `arena::set_level` → `Game::raise_to_level(entity, level,
   creature_cap: u32)` (`pub(crate)`, `party.rs`). Arena's `set_level` becomes
   a call passing `level_cap().max(arena_level_ceiling())` (keep the wrapper
   so `tests/support.rs` and the ~15 test callers don't change). Body
   unchanged, including `install_unlocked_routines`.
5. **`reinitialize_blocker(index) -> Option<&'static str>`** (pub): in order
   game over / active battle, no such record, `boss`, no protocol held,
   `roster_room() == 0`. **`reinitialize_program(index) -> Result<(),
   String>`** (pub): blocker → `Err`; else `take` one protocol, remove the
   record, `adopt_program_pinned(species, player pos, 1.0, pins{record.rarity,
   record.carried})`, `raise_to_level(p, reinit_level(..), level_cap())`, one
   `MessageKind::Outcome` log line (label via `downed_program_label`). No
   ticks spent, no deed. If the spawn returns `None` (unknown species — mod
   removed it) refuse *before* spending: check `SpeciesDb` in the blocker.
6. **Tests** (`tests/extraction.rs`, from a real `Game` with a record pushed
   into `DownedPrograms` — reuse whatever fixture extraction tests use):
   success spends exactly one protocol, removes exactly that index (neighbours
   keep order), `pet_count` +1, body on player tile, `Tamed`; level =
   `reinit_level` (and clamped by `level_cap`); rarity = record's; carried
   routine present in `Routines`, and `None` carried → same `Routines` as a
   plain `adopt_program` at that level; refusals (boss, no protocol, roster
   at cap, bad index, unknown species) each leave store + inventory + roster
   unchanged; no `GameRng` draw for rarity/routine when pinned (compare RNG
   state against an unpinned spawn, or assert the pinned path never calls the
   roll). Mutation-check the refusal tests (commit fix first).

## Phase 2 — app-core + gui + help

Files: `app-core/src/app/extraction.rs`, `app-core` tests for it,
`gui/src/render/extraction.rs`, `assets/help/extraction.md`.

1. **Key.** In `handle_downed_programs_key`'s `Some(program_index)` arm,
   before `selected_index`: `GameKey::Char('R')` → call
   `reinitialize_program`, `report(outcome)`, return to list
   (`pending_downed_program_index = None; menu_selected = 0`) on success;
   on refusal stay on the page with the report. Must work even when
   `options` is empty (no tool installed) — move the empty-options early
   return below the `R` check.
2. **Row.** `extraction_options_rows`: after the tool rows, `[R]einitialize
   — resurrect downed program`, and when `reinitialize_blocker` is `Some(s)`
   append `  (s)` greyed. Follow the popup-row-width headless test pattern
   (`popup-row-width-is-testable-headlessly`) so the line fits the popup.
3. **Help.** One line in `assets/help/extraction.md` naming the protocol,
   where it's crafted, and that bosses can't be reinitialized.
4. **Tests:** app-core — `R` on a record page with a protocol → record gone,
   roster +1, back on list; `R` with no protocol → page kept, refusal
   reported, nothing spent; lowercase `r` still selects a row. gui — row
   builder shows the action and the blocker text.
5. Screenshot once (`--template` with a held record, or `capture` one) and
   Read the PNG — a green suite is not play.

CHANGELOG line is written at deploy, not on the branch.
