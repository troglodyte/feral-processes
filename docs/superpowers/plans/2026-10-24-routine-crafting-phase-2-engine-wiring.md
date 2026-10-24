# Phase 2 — Engine wiring (additive)

Read first: plan index; spec "Life of a built routine", "Where Subroutines
come from", "Research", "Save"; `.claude/rules/seams-items.md`,
`seams-research.md`, `content-schema.md`, `seams-combat.md`. Old etching /
known-routine paths stay in place this phase; phase 4 deletes them.

## Files (expected; grep before assuming)

- `abilities.rs`: `AbilityDb` gains a crafted tier + `set_crafted(&[CraftedRoutine])`;
  `get` answers both. Audit every iteration over `abilities` (authored-only
  vs both) and decide each explicitly.
- `items_db.rs`: synthesise one item per part (and Core version), beside
  `synthesise_etched_disks`. Parts take every economy role etched disks have:
  `tuning/production.rs`, `game/stack_market.rs`, depot/hauling, trade.
- `resources.rs` + `lib.rs`: player state — built routines, unslotted set,
  known Subroutines, part cap, id counter (`crafted/<n>`, never reused).
- New `game/routine_craft.rs`: `craft_subroutine`, `build_routine(name, parts)`,
  `install_built(routine, holder)`, `uninstall_built` (destroys). Each calls
  `set_crafted` in its apply step.
- Companion-leave sites destroy held built routines: `game/trade.rs`
  `sell_companion`, `game/party.rs` `fuse_companions`, release, Workbench
  teardown (`game/extraction.rs` / `game/routines.rs` `extract_routine`).
  Grep for every despawn of a companion; downed / posted / sortie keep them.
- `routine_tree.rs`: synthesise nodes from the catalogue (Core versions,
  modifiers, capacity per zone); cost shape from `routine_research_cost`.
  Visibility zone-gated only.
- `game/extraction.rs`, `combat_rewards.rs`, `combat_damage.rs`: Routines tool
  and teardown hand out `decompose(def)` items. Boss `boss_drop` → parts at
  today's odds (`items_db.rs`, `species.rs`).
- `game/creation.rs` / `lifecycle.rs`: new game holds Decompile + Patch; Heal
  v1.0 and Decompile prereqs known.
- `views/`: a `views/routine_craft.rs` for the three Workbench pages (build
  rows with fit/greyed reason, preview, parts used/missing; Subroutines list
  with held/cost/locked; Built list).
- `save.rs`: new fields (field-named RON, ADR 0023), `SAVE_FORMAT_VERSION`
  38→39; `set_crafted` runs on load before slots restore.

## Tasks (a commit per green step)

- [ ] 1. Crafted tier + `set_crafted`; a crafted id resolves through `get`
  and through `Game::actor_abilities`.
- [ ] 2. Part items synthesised; census: every catalogue part has an item.
- [ ] 3. Craft / build / install (player + companion) / uninstall-destroys.
  Cap enforced; Emulate untouched.
- [ ] 4. Companion leaving for good destroys; downed/posted/sortie keep.
  One test per leave path.
- [ ] 5. Research nodes + capacity raise cap. Check each part's material
  against the research-bill rule; misfit → stop and ask.
- [ ] 6. Extraction, teardown and boss drops yield parts, never a discovery.
  Extracted parts usable unresearched.
- [ ] 7. New game = Decompile + Patch.
- [ ] 8. Save v39: build → install (player and companion) → battle → save →
  **load** → still installed and resolvable. Uninstall removes it from save
  and crafted tier.
- [ ] 9. Views for the Workbench pages, unit-tested.

## Gate

`cargo test -p feral-processes-engine`, `balance_sim`, then
`cargo test --workspace` (app-core must still compile and pass — it still
uses the old API). Handback: the `abilities` iteration audit list, leave-site
list, any research-bill misfits.
