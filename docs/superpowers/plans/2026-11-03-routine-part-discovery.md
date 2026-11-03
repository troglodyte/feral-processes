# Routine part discovery: plan

**Design:** `docs/adr/0041-routine-parts-are-found-before-they-are-researched.md`.
Read it first, because it decides the behaviour and every "my call". This plan only says where the work goes.

**Worktree:** `.claude/worktrees/routine-part-discovery`, on branch `worktree-routine-part-discovery`.
One crate (`crates/engine`), with no save or schema change, so it is a single phase.

**Rules:**
- TDD: write each failing test first, then commit at each green step.
- Run `cargo fmt` and `cargo clippy --workspace --all-targets` before each commit.
- Never push. Never `git checkout`, `git stash` or `git add -A`; stage explicit paths.
- Read `.claude/rules/seams-items.md` (the `take_routine` and `extract_program` bullets) before touching extraction.
- Keep the RNG draw order unchanged wherever nothing is findable (ADR: no draw).
- Iterate with `cargo test -p feral-processes-engine <name>`. Tests that use research fixtures live in `src/tests/research/` (`discovery_and_study.rs` is the model).

## Steps

1. **Starting parts constant + hidden part nodes.**
   - In `src/tuning/routine_craft.rs`, add `pub const STARTING_PARTS: [Subroutine; 2]`: Heal v1.0 and BoostAttack v1.0. Fall back to a fn if a const won't type-check.
   - In `src/tuning/`, add `ROUTINE_PART_DISCOVERY_CHANCE: f64 = 1.0 / 3.0`, with a why-comment, next to `ROUTINE_TOOL_FIRST_UNKNOWN_WEIGHT`.
   - In `src/routine_tree.rs`, `part_node` takes `discoverable: bool`. It is true for every part except `STARTING_PARTS`, and false for capacity nodes.
   - `grant_starting_routine` (`src/game/routine_craft.rs:221`) also inserts Boost Attack v1.0's node id into `Research`. It builds nothing new.
   - Tests:
     - a new game has both starting nodes researched;
     - every other part node is `discoverable`;
     - capacity nodes are not.

2. **Visibility.** In `src/game/unlocks.rs`:
   - Add a pure-ish `fn part_known(&self, def) -> bool`: researched, or `!discoverable`, or in `DiscoveredResearch`.
   - `routine_node_visible` becomes zone gate && (`!discoverable` || discovered). Reuse `base_node_visible`'s discovered half by calling it, not copying it. `routine_node_reachable` stays researched || visible, so `listed_research` and `select_research` follow without edits.
   - Tests:
     - an undiscovered part is not listed and `select_research` refuses it with "Unknown research.";
     - once discovered it is listed;
     - a researched-but-undiscovered part (an old save) stays listed.
   - Expect existing routine-tree tests in `src/tests/research/` and app-core `tests/research.rs` to fail where they assume a full list. Fix each with `discover_research`, or with a fixture helper beside `discover_all_research` in `src/tests/support/research_fixtures.rs` (add a `discover_all_parts`, or extend that one). Don't weaken the assertions.

3. **Findability predicate (functional core).**
   - Add a pure `fn findable_parts(parts: &[Subroutine], known: impl Fn(&str) -> bool, eligible: impl Fn(&str) -> bool) -> Vec<Subroutine>`. Place it in `src/routine_tree.rs`, or a sibling if it grows.
     - Input: distinct parts in `Ord` order.
     - It keeps a part whose node is `discoverable`, not discovered, not researched and zone-allowed (`eligible`), and, for a Core v2/v3, whose rung below is `known`.
     - Findability is computed once, so nothing within one call chains.
   - Unit-test it directly with closures:
     - the v3-carrier with nothing known yields v2 (v1 Heal is starting);
     - it never skips a rung;
     - modifiers have no prerequisite;
     - duplicates collapse.

4. **`discover_research` takes a source.**
   - Add `enum DiscoverySource { Study, Extraction }`, which picks the log line (plain language). Both raise `ResearchDiscovered`.
   - Update all ~39 callers. Most are tests, so pass `Study`, or add a test helper if that reads better.
   - Test: an extraction find logs the extraction line and not "The study uncovers".

5. **Extraction door.** In `Game::take_routine` (`src/game/routines.rs:437`), after `grant_parts`:
   - Gather the findable parts via step 3. If there are none, return with no draw.
   - Otherwise draw one `GameRng` `random_bool(ROUTINE_PART_DISCOVERY_CHANCE)` per part in order, and on each hit call `discover_research(.., Extraction)`.
   - Keep decide pure: `fn(findable, &mut Rng) -> Vec<Subroutine>`.
   - Tests, through both doors (`extract_routine` on a tamed program at a bench, and `extract_program` with a `Routines` tool on a downed one):
     - with the chance forced to 1 via a seeded loop or a decide-step test, the parts are discovered;
     - an all-known routine leaves the `GameRng` state identical to a run without this feature: compare the next draw against a clone taken before;
     - a hit's node is listed afterwards.
   - Mutation-check: remove the roll, then confirm the tests fail.

6. **Study door.** In `eligible_discoveries`, the `ResearchTree::Routines => false` arm becomes `routine_tree_open && <part node of the pinned subject's routines and findable>`.
   - The subject comes from `pinned_subject()`, its routines from `Routines`, and its parts from `parts_of_routine`.
   - Routine nodes skip the base arms' `requires.all(is_researched)` filter and use step 3's predicate instead.
   - Pass the subject in, or filter in `settle_study`. The builder's call, but `eligible_discoveries` stays the one pool.
   - Tests:
     - a pinned program carrying an unfound part can discover it;
     - a part the subject doesn't carry never enters the pool;
     - with the routine tree closed, the pool equals today's, so seeded study tests don't shift.

7. **Censuses** (`src/tests/assets/structures_research.rs`):
   - `exactly_the_named_research_nodes_are_discoverable`: the routine set is "every part node but `STARTING_PARTS`".
   - `every_discoverable_research_node_is_reachable_from_the_visible_tree`: should still pass, since part prerequisites are other parts. Confirm, and change nothing if it does.
   - New `every_discoverable_part_is_carried_by_some_shipped_program`: union `decompose` over every shipped species' routines (the source `routine_craft/tests.rs::every_reachable_routine_decomposes_except_the_known_misfits` already walks). Every discoverable part node must be in it, or in a named exception list with a reason per entry. The list is the content finding: report it to the user, don't fix content.

8. **Docs.**
   - Add a `CHANGELOG.md` entry under the unreleased section, written for players.
   - Mark ADR 0041's Evidence current.
   - In the `seams` skill, the rules file and the graph, add a bullet only if a new load-bearing rule emerged. Candidate: "a routine part is found through `take_routine` and the study pool, and an all-known extraction draws nothing".

## Gates

- `cargo test --workspace`, with `FERAL_DEV_NO_SIEGES` unset.
- `cargo test -p feral-processes-engine balance_sim`. It is expected unchanged, because it models neither door.
- Clippy is clean.
- A seeded test that moved because extraction now draws counts as a stream shift, not a bug (`rng-stream-shift-exposes-seed-luck-tests`). Re-seed it only after confirming it was seed luck.
- Final whole-branch review on opus, given the diff as a file.
- Then deploy (merge → version bump → tag → push → clean up), on the user's word.
