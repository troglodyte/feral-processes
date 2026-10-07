# Siege Rebuild, the Interact Key, and Build Order — plan

Spec: `docs/superpowers/specs/2026-10-07-siege-rebuild-design.md`. The spec
holds the rules and the argument. This file gives only the order of work, the
files, and the gates. Branch: `siege-rebuild`.

**Every phase:**
- Use TDD and commit at each green step.
- Leave `cargo fmt` and `cargo clippy --workspace --all-targets` clean.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.
- Never push. Never `git checkout`/`stash`/`reset` over uncommitted work.
  Stage explicit paths only.
- Before editing, read `.claude/rules/seams-base.md` and `seams-sieges.md`.
  Phase 2 also reads the save rules.
- Phase gate: the crate tests named in the phase. Phase 4 runs the full
  `cargo test --workspace`.

The phases run in order. Phase 1 is independent of Phase 2, but both touch
`work_orders.rs`/`construction.rs`, so running them in parallel worktrees
would cause merge conflicts. Run them serially.

## Phase 1: Production level and build order (engine)

**Files:**
- `engine/src/structures.rs`: add a pure `structure_levels(defs) ->
  HashMap<StructureId, u32>` (spec §3 rules: `work` with no inputs = 0;
  otherwise 1 + max producer level over build cost + `assembles`/`strips`
  inputs; unproduced items contribute nothing; ignore own output in own
  cost; cycles share a level, either by a fixed point capped at the def
  count or by SCC condensation). `StructureDb` stores the result at load,
  with a `level(&id) -> u32` accessor.
- `engine/src/game/base/work_orders.rs` `build_wants` (:1895): sort by
  `(level, x, y)`, then keep only workable sites at the lowest workable
  level. Upgrade and recharge goals use the target structure's level.

**Tests:**
- unit tests on hand-built defs: raw producer, two-step chain, cycle,
  unproduced item, self-costing node;
- real assets: Mining Node < Assembly Bay;
- scheduler: miner and assembler pending → only the miner is worked; miner
  dry → the assembler is worked. Start from a `dev-saves/` template if one
  fits.

**Gate:** `cargo test -p feral-processes-engine` plus `balance_sim` (not
expected to move; if it does, stop and report).

## Phase 2: Ruins, rebuild sites, program commit, save (engine)

**Files:**
- `components.rs`: a `Ruins(Vec<Ruin { kind, x, y }>)` resource;
  `BuildSite::awaiting_program: bool` (:2794).
- `game/base/upkeep.rs` `damage_structure` (:1035): push a ruin on the
  durability-zero branch, before the despawn. Do not touch
  `remove_structure`.
- Filing step in the base tick, gated on "no siege running". Use the
  existing predicate if there is one; otherwise add it beside
  `game/siege/clock.rs`. Factor the site-filing half of `place_structure`
  (`building.rs:26`) into a helper that both call, so no program is
  committed. A blocked tile drops the ruin and logs it in the log's
  existing voice.
- `needs_program()` defs file with `program: None, awaiting_program: true`.
  `construction.rs` `build_is_workable` (:150) returns false for these, and
  the dry-announce latch skips them.
- `building.rs`: `pub fn commit_rebuild_program(site, program) -> Result<(),
  BuildError>` via `commit_for_build` (:382), and a read for awaiting sites
  that app-core/gui can use.
- `save.rs`: `SAVE_FORMAT_VERSION` 36 → 37 (also `game/lifecycle.rs` if it
  pins the version); `SaveData::ruins: Vec<RuinSave>`;
  `BuildSiteSave::awaiting_program`. Wire the save and load paths.

**Tests:** spec §Testing engine bullets 1–6 and 10. The save test must be a
real save→load, not a RON round trip.

**Gate:** `cargo test -p feral-processes-engine`.

## Phase 3: The interact key (engine read, app-core, gui)

**Files:**
- engine (`game/base/transfer.rs` or a new `game/base/interact.rs`):
  `Interaction`, `InteractionKind { Transfer, RebuildProgram(Entity) }`,
  and `Game::adjacent_interactions()`. `Transfer` is present when
  `transfer_offer`/`rack_offer`/`adjacent_depot_entities` is non-empty.
  Re-export from `lib.rs`.
- app-core `app/playing.rs` `[c]` arm (:548): 0 → `refuse_transfer()`;
  1 → act; several distinct directions → a direction prompt that reuses the
  build-direction input handling.
- app-core `lib.rs` `PendingBuild` (:2544): add a variant for committing to
  an existing site. `app/building.rs`'s program-pick path ends in
  `commit_rebuild_program` for that variant.
- gui `render/building.rs`: the picker footer and title name the rebuild
  (:584 helper). Draw a "needs a program" marker on awaiting sites wherever
  pending builds are drawn. Draw only through `Painter`.

**Tests:** spec §Testing app-core bullets: one Depot → Transfer unchanged;
one awaiting site → picker → commits; both → direction prompt routes each
side.

**Gate:** `cargo test -p feral-processes-engine -p feral-processes-app-core
-p feral-processes-gui` (use the real gui crate name).

## Phase 4: Docs, seams, final gate, review

- `CHANGELOG.md`: an unreleased note covering the breaking save, build
  order by level, and `[c]` as interact. The version bump happens at
  deploy, not here.
- `assets/structures/README.md`: only if level is surfaced to modders
  (it is derived, so probably a single line).
- Seams, in the order the `seams` skill documents: the `seam:` graph node,
  the skill entry, then `.claude/rules/seams-base.md` (`[c]` is the
  interact dispatcher; destruction records a ruin; build order is by
  level).
- Run the full `cargo test --workspace` and clippy.
- Final whole-branch review (opus), with the diff given as a file. Ask the
  reviewer to re-derive the levels of 2–3 real structures by hand.
