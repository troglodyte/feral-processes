# Battle Impact Feedback Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: superpowers:subagent-driven-development (recommended) or superpowers:executing-plans. Checkbox steps.

**Goal:** Each narrated battle line carries the impact it had; the group battle screen draws floats, row flashes and a portrait reaction from that, replacing HP-diff inference.

**Spec:** `docs/superpowers/specs/2026-10-12-battle-impact-feedback-design.md`. Read it first; this plan does not restate its rationale.

**Phases:** three, one per crate, strictly in order (each consumes the last one's interface). One subagent per phase is the right grain, because each phase fits one context and its tasks share files.

## Global Constraints

- The renderer never touches `World`, and there is no `world()` accessor. Only `crates/gui/src/paint.rs` names the graphics library; `render/` draws through `Painter`, and panes take their origin from a `Rect`.
- No save change: `LogLine`, `BattleState` and `EnemyGroup` are not serialised. Confirm this with `rg -n "Serialize" crates/engine/src/battle.rs crates/engine/src/resources.rs` before Task 1. If any of them derives it, stop and report.
- **Line count is invariant.** No line is added or removed. Reveal pacing, `RosterFrame` counts, `condense` and sound cues must see exactly what they see today. The existing pacing and sound tests stay green with **no edits** beyond adding `impact: None` to literal `LogLine { .. }` constructions in tests.
- The tactical screen is out of scope. Impacts are released only under the same `self.mode.is_battle()` gate that sounds use.
- Named constants live in `fx.rs` beside `FLOAT_SECONDS`. There are no engine tuning changes, so `balance_sim` must not move.
- Read `.claude/rules/seams-*.md` for battle/log/HUD if not loaded; the `seams` skill has the traps.
- After every task: `cargo fmt` and `cargo clippy --workspace --all-targets` clean, then commit at green. Unset `FERAL_DEV_NO_SIEGES` before running tests.
- Subagents: never push; never `git checkout`, `git stash` or `git add -A`; stage explicit paths.

## Review Focus

1. **Kill swing:** the swing line carries `Damage`/`Crit` and the following kill line carries `Kill`. They are never merged, and no line is added.
2. **Front group dies mid-round:** the next group keeps its `GroupId`, and floats and ghosts follow the id, not the letter or index.
3. **Skip:** `skip_reveal` releases every skipped impact in order, while sound stays one loudest cue. `finish_reveal` on its own (auto-resolve `Finished`, `battle.rs:239`; arena) releases **none**, because the screen is jumping to results. Assert both.
4. **Party bar ghost key vs group id:** the two key ranges must not collide, since a counter has no upper bound (see Task 6).
5. **Miss/fumble** draw a mark and no flash. **Heal** flashes green. A **status-tick** line gets an impact, and a header or expiry line gets `None`.

---

## Phase 1: Engine (`crates/engine`)

### Task 1: `Impact` types and `GroupId`

**Files:** `resources.rs` (`LogLine` :443, `push_line` :533, `BattleState` :1452), `battle.rs` (`EnemyGroup` :435, plus wherever groups are formed), `views.rs` (`EnemyGroupView` :2060), and the crate's re-exports.

**Produces:**
- `Impact { target: ImpactTarget, kind: ImpactKind }`.
- `ImpactTarget { Party(usize), Group(GroupId) }`, deriving `Copy, Eq, Hash, Ord`.
- `ImpactKind { Damage(i32), Crit(i32), Heal(i32), Miss, Fumble, Kill }`.
- `pub struct GroupId(u32)`, deriving `Copy, Eq, Hash, Ord`.
- `LogLine.impact: Option<Impact>`.
- `EnemyGroup.id: GroupId`, assigned from a `BattleState` counter whenever a group is formed and never reused within a fight.
- `EnemyGroupView.id`.

Steps:
- [ ] Census every place a group is formed: `rg -n "EnemyGroup \{" crates/engine/src`. Each one takes an id from the counter, and the census goes in the commit message.
- [ ] Test (fails first): a fight with two groups where the front group is killed leaves the survivor with its original `GroupId`. Mutation-check it by re-deriving the id from the index and confirming the test goes red.
- [ ] Implement. Literal `LogLine { .. }` sites (`tests/message_log.rs`, `tests/level_up.rs`, others found by rg) gain `impact: None`. `push_line` gains no parameter; Task 2 adds a separate setter path.
- [ ] Run `cargo test -p feral-processes-engine`, then commit.

### Task 2: Set impacts at the logging sites

**Files:** `game/turn.rs` (`log_swing` :66 and its 8 callers), plus the heal, drain, status-tick and kill-line sites.

- [ ] Census the sites: `rg -n "log_swing\(|fn log\(|\.log\(" crates/engine/src/game crates/engine/src/battle.rs`. Classify each battle-narration line as one of: targeted, which gets an `Impact`; or untargeted, which gets `None` (round header, "stalls out", status expiry). List the classification in the commit.
- [ ] `log_swing` takes `Option<Impact>` alongside its `SwingOutcome`. Add one targeted logging helper for the non-swing sites so that every targeted line goes through the same path.
- [ ] Tests (one each, all failing first): party hit, enemy hit, crit, miss, fumble, heal, status-tick damage, and a killing swing whose swing line has `Damage`/`Crit` while the next line has `Kill`. Also: a header line has `None`, and the line count of a seeded round is unchanged from before (pin the number from the current build).
- [ ] Run `cargo test -p feral-processes-engine`. `balance_sim` must be unchanged. Commit.

## Phase 2: App-core (`crates/app-core`)

### Task 3: `pending_impacts`

**Files:** `lib.rs` (field beside `pending_sounds` :3250), `app/lifecycle.rs` (init :167; `take_sounds` :204 is the model), `app/input.rs` (`advance_reveal` :432, `skip_reveal` :770).

**Produces:** `App::take_pending_impacts(&mut self) -> Vec<Impact>`, which drains the queue.

- [ ] Tests: impacts arrive one per revealed line, in step with `advance_reveal`. `skip_reveal` releases every skipped line's impact in order. `finish_reveal` alone releases none. Nothing is released outside `is_battle()`. Mutation-check the skip test by pushing only the first impact and confirming it goes red.
- [ ] Implement. `advance_reveal` pushes `lines[i].impact` next to the existing sound push. `skip_reveal` collects `battle_log()[start..]`'s impacts **before** calling `finish_reveal`.
- [ ] Run `cargo test -p feral-processes-app-core` and confirm the pacing tests are unedited and green. Commit.

## Phase 3: GUI (`crates/gui`)

### Task 4: Fx state for impact floats and row flashes

**Files:** `fx.rs` (`spawn_float` :1685, floats retain :1072, `FLOAT_SECONDS` :232).

**Produces:**
- `Fx::impact(&mut self, impact: Impact, row: Rect)`, which spawns a styled float and, for hit, crit or heal, a flash.
- `Fx::row_flash(&self, target: ImpactTarget) -> Option<(Color, f32 alpha)>`.
- Constants `IMPACT_STAGGER_PX` and `ROW_FLASH_SECONDS` (≈0.3), plus the crit text scale.
- Float styles as in the spec: `-N` red; `CRIT -N` white and larger; `+N` green; `miss`/`fumble` dim; `KO`.

- [ ] Headless tests, following `fx.rs`'s existing pattern: two impacts on one row within a float's lifetime get distinct x positions, a flash fades to `None` after `ROW_FLASH_SECONDS`, and a miss produces no flash.
- [ ] Implement, run `cargo test -p feral-processes-gui fx` (check the gui crate's package name in its `Cargo.toml`), then commit.

### Task 5: Drive it from `render/battle.rs`

**Files:** `crates/gui/src/lib.rs` (beside `take_sounds` :897), `render/battle.rs` (group rows :343–422, party rows :527–590, `draw_battle_portrait` :880).

- [ ] Drain `take_pending_impacts()` once per frame and hand the result to the battle draw. Each impact resolves to its row `Rect` by `ImpactTarget`: `Party(slot)` by `p.slot`, `Group(id)` by `g.id`. A target with no row this frame (e.g. a group removed on `Kill`) falls back to its last known rect, kept in `Fx`.
- [ ] Delete the HP-diff `spawn_float` calls (:422, :587); the ghost band stays. Draw `row_flash` as a background tint under each row.
- [ ] Portrait: on `Damage`/`Crit`, a brief nudge plus flash; on `Kill`, a `KO` stamp. Use named constants.
- [ ] Test: a float lands on the row its `ImpactTarget` names after a group re-letters. Build the view with ids out of index order.
- [ ] Commit.

### Task 6: Re-key the ghost bar

- [ ] `fx.bar_ghost(idx as u64, …)` (:349) is keyed by `g.id`. Retire `PARTY_BAR_KEY_BASE`'s "can never meet" assumption. Prefer keying `bar_ghost` by `ImpactTarget` if all its callers are in `render/battle.rs` (check with rg). Otherwise encode the party/group split in disjoint halves of `u64`. Test that a group with id ≥ 1000 does not share a ghost with party slot 0.
- [ ] Commit.

### Task 7: Screen check and final gate

- [ ] Smoke screenshot: `DISPLAY=:0 FERAL_DEV_ARENA=1 cargo run -- --keys "R l Down Down Down Down Down Down Down Enter f" --screenshot out.png`. Read the PNG to confirm the battle screen still lays out. A capture cannot resolve a round, so the transient cues are proven by Task 4–6's headless tests, not by the screenshot.
- [ ] Run `cargo test --workspace` and `cargo clippy --workspace --all-targets`.
- [ ] Final whole-branch review (opus), with the diff given as a file and the Review Focus above as its checklist.
- [ ] CHANGELOG entry at deploy, not on the branch.
