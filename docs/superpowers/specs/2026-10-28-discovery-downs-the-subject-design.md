# A discovery downs the studied program

## Intent

Study by discovery should cost something visible. Today a Research Station with a pinned program and no project rolls `settle_study` again and again against the same subject, for ever. The user wants each discovery to use up the subject. The program goes down, the station shows it with an `x`, and nothing more is found until a new program is pinned. Workers clear the downed program away without being told.

## Behaviour

1. **Downed on a discovery, never on a miss.** When `settle_study`'s roll hits and `discover_research` runs, the pinned subject goes through the conversion `settle_research` already uses for a subject-spending node: `downed_program_for_with_overkill(subject, 0.0)`, `hold_at_station`, `destroy_built_held_by`, despawn, `note_departure(…, Departure::LetGo)`, and the log line "{name} is spent in the study." "The study turns up nothing." changes nothing. Factor the conversion into one function that both callers use (CLAUDE.md: a call, not a copy).
2. **The `x`.** A Research Station whose `StudyYield` slot holds a program draws an `x` on the base map. Engine side, a view field says the slot is full. GUI side, the station's tile draws the glyph through `Painter` only (`.claude/rules/drawing-seam.md`). **The `x` shows only while the program waits to be carried away** (user's decision). On a staffed base that is a few beats. After that the station reads as "no subject pinned", because the subject is gone. No lasting "spent" flag, and no save change.
3. **Study waits for a new subject and an empty slot.** The subject is despawned, so `pinned_subject()` already returns `None` and study stops on its own. `settle_study` also gets a slot-full hold beside the `pinned_subject` check, for a player who re-pins before the carry finishes. A held bar spends no draw. Research projects that don't spend a subject are never blocked by the slot (user's decision). Subject-spending completions stay blocked by the slot-full gate (item 6), as they are today. `study_wants` raises no want while the slot is full.
4. **One drop chain, both paths.** `shelter_program` and the carry errand in `study_yield.rs` use this order:
   1. a Quarantine Rack with room (the first in `(x, y)` order, as now)
   2. a free stockpile cell. Reuse `Game::free_stockpile_cells` (`tidy.rs`, `(x, y)`-sorted, already skips structures, build sites and floor items), keep only cells within reach as racks are, and set the program down as `FloorThing::Program`. *My call.*
   3. the first empty floor cell next to the station

   The old "station slot" step goes away. Subject-spending research completions and discoveries share this chain (user's decision). "Any stockpile cell with room" means any cell. There is no new stockpile filter (user's decision).
5. **The errand still chooses its destination before the pickup** (the module's existing rule). Rungs 2 and 3 count as destinations. A carrier reaches a stockpile or bench-side cell and sets the program down there. *My calls:*
   - **Claims.** `RackBoard` grows a set of floor cells already claimed by a carrier in transit, the way `spare` counts rack room. A new pickup skips claimed cells.
   - **Re-derived each beat**, as now. If the target cell filled on the way, the carrier falls through to the next rung from where it stands.
   - **A carrier that can reach nothing** (`put_carried_program_back`, `rack_deliver`'s fallback) still returns the program to the station slot. The slot is a holding place, no longer a `shelter_program` destination.
   - `set_down`'s last resort, `stow_program`, stays as it is. In practice it can't happen while a bench-side cell is free.
6. **Completion gate for subject-spending nodes.** `study_yield_blocker` keeps only the slot-full arm ("crew to carry the last program away"). The rack-room arm goes, because the floor next to the bench is always a destination. Delete `free_rack_room` if nothing else uses it once that arm is gone. The pickup guard moves to the new destination search. *My call.*
7. **Update the `study_yield.rs` module doc** ("rack, station slot, floor") to match the new chain.

## Constraints

- No save-format change. `StudyYield` and floor programs are already saved, and stockpiles are zoned floor cells.
- RNG draw order in `settle_study` stays the same. The new hold comes before the attempt is spent, so a held bar spends no draw. Afterwards, check `balance_sim` (it should not move).
- Read `.claude/rules/seams-base.md` and `seams-research.md`, and the `seams` skill's base-labour traps, before changing the errand.
- Engine + gui: the spec-and-plan pipeline. TDD, starting with failing tests in `tests/study_yield.rs` and `tests/work_orders.rs`.
- Docs: a `CHANGELOG.md` entry at release. The research help page if it describes endless study (check `assets/help/`).

## Non-goals

- Changing discovery chance, attempt cost, or the eligible pool.
- Stockpile filters or capacity rules for programs.
- Changing what happens to a downed program after it is racked or set down.

## Left to the builder

- The view field's name and shape, and the exact glyph colour (follow how other base-map status marks are drawn).
- Where the shared conversion function lives (`study_yield.rs` or `unlocks.rs`).
- Whether rung 3 reuses `free_floor_cells(station_pos, 1)`. It does if that already returns adjacent cells in a stable order.
