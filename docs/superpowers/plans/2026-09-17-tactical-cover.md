# Tactical Cover Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** A boulder next to you on the attacker's side makes you harder to hit at range. Walk around it and the protection is gone, which is flanking for free.

**Architecture:** One pure predicate, `reach::cover_between`, is the only thing that decides cover. The evasion it buys enters at the one place both bodies are known — a new `Game::defender_profile_against`, which wraps the existing `combatant_profile` rather than copying it. The AI reads the same predicate through `cell_merit`, and the two telegraph marks are calls into it too, so the screen and the roll cannot disagree.

**Tech Stack:** Rust, bevy_ecs 0.19, bevy + bevy_egui (gui).

**Spec:** `docs/superpowers/specs/2026-09-16-tactical-cover-design.md`. Read it before any task. This plan argues from it; the section below lists where the two disagree, and the plan wins.

## Where this plan corrects the spec

Checked against source on 2026-09-17, after the reactions feature landed in `v0.13.200`.

1. **The board-generation follow-up is not needed, and the measurement is a test rather than a `docs/measurements/` file.** The spec says "measure first" and leaves raising cover density as a possible follow-up. It was measured (method and numbers below): between **12.7%** (OpenGrid) and **43.2%** (Backplane) of the shots that can actually be taken are at a defender with cover. That is plenty, so `terrain_weights` is not touched. The numbers are cheap to recompute from code, so per `docs/measurements/README.md`'s own rule — "if a claim can be a test, make it a test" — the floor becomes a census in Task 1 instead of a prose file. This also closes the repo's standing trap that a band-or-radius gate can ship green and unreachable.

   | biome | `Cover` weight | walkable cells with a cover neighbour | beyond-melee pairs **with line of sight** that have cover |
   |---|---:|---:|---:|
   | OpenGrid | 4 | 28.6% | 12.7% |
   | Deadlock | 7 | 46.5% | 22.3% |
   | NullSector | 8 | 46.3% | 22.8% |
   | Backplane | 20 | 75.6% | 43.2% |

   Method: 40 seeds x 3 body counts (all three board extents) per biome, full enumeration of ordered walkable pairs beyond `TACTICAL_MELEE_RANGE`, the spec's rule applied verbatim. Note Backplane has the *lowest* share of all pairs (7.3%) and the highest of sighted pairs — 20% cover blocks most long sightlines outright, so the shots that remain are mostly covered ones. Sighted pairs is the right denominator: a pair with no line of sight has no attack to modify.

2. **The `Swing` flag is `cover_ignored`, not `cover_applies`.** `battle::Swing` derives `Default`, and `Swing::free`'s own doc records that its polarity was chosen so a `Swing::default()` written later cannot switch the fumble ladder off. Same rule here: `cover_ignored: bool` defaulting to `false` means cover applies, which is the default the spec asks for. A `cover_applies` field would default to "ignored" and silently delete the feature at any construction site that uses `..Default::default()`.

3. **`defender_profile_against` wraps `combatant_profile`; it is not a second copy.** `combatant_profile(entity, swing)` (`game/combat_damage.rs:131`) is where the Exposed rung already reads, and it takes one entity by design. The new door calls it and then raises the evasion on the returned `battle::Combatant`. Nothing moves out of `combatant_profile`.

4. **The AI's cover term is pure and needs no `&self`.** `cell_merit(board, cell, band, targets)` (`tactical/ai.rs:188`) already carries everything `cover_between` needs. "Ranged" is `band.max > TACTICAL_MELEE_RANGE`, read off the `band` parameter it already has. This matters: `scored_cells` uses `cell_merit` as the **candidate filter** (`ai.rs:793-800`), so a term in merit is read by the filter as well as the draw — which is what makes a ranged body move to gain cover at all. The melee tie-break goes in `cell_score` beside crowding (`ai.rs:166`), the precedent the spec names.

5. **`expected_reaction_damage` calls the new door too.** `tactical/ai.rs:862` builds a defender profile for the reaction telegraph and the AI's `walk_risk`. Cover can never apply there — a reaction reaches `TACTICAL_MELEE_RANGE` and rule 1 refuses melee — so this is a no-op, and it is still a *call* rather than a site that quietly keeps the old behaviour if rule 1 ever changes.

6. **The group model's forecast is left alone.** `game/combat_policy.rs:285-287` keeps `combatant_profile`. There is no board there, so the new door would answer identically; leaving it makes "the group model is unchanged" an omission rather than a branch.

7. **The telegraph is two doors onto `cover_between`, not one query.** The spec says "one engine query" and means "the screen and the roll can't disagree". The two marks have different subjects — a *body* against one attacker, and a *destination* against every visible hostile — so one function cannot answer both without a mode flag. Both are thin calls into `cover_between`, which is what the spec is actually protecting.

8. **`TacticalView::frozen` must clear the new fields.** `tactical/view.rs:191` blanks the AI-only fields for the results board. A cover mark left standing there would draw over a finished fight.

9. **The top-right tile corner is free on the battle board.** `marks::nemesis_mark_rect` is never called from `render/tactical.rs`, even though the surface map spends that corner. Top-left is the con earmark, the top edge is the rarity bar and the bottom edge is the HP bar, so the shield mark takes top-right.

10. **The arena cannot author the board's biome, and this plan does not add that.** The board takes the biome of the tile the player stands on, and `dev-arenas/README.md:383-387` says overriding it is not worth a third parameter "until a question needs it". Cover *is* that question, but answering it is its own change. Task 7's tuning check is a seeded engine sweep over a hand-written board plus a before/after read of the existing `dev-arenas/tactical-full-group.ron`, and the plan records that the arena arm can only speak for one biome.

## Global Constraints

- **No save change.** No `SAVE_FORMAT_VERSION` bump, no new save field, no new `BattleCell` kind. The board keeps its four cell kinds.
- **Cover is not content.** No `assets/` directory, no `.ron`. It is a rule over terrain the generator already places, so its constants live in `tuning.rs` — `disposition.rs`'s argument, and `tuning.rs`'s own line that how hard the game is, is not moddable.
- **`reach::cover_between` is the only function that decides cover.** Every other reader — the roll, the AI, both telegraph marks — is a call to it. A second copy of the arc rule is the defect this whole design exists to prevent.
- **Tactical board only.** No `TacticalBattle` resource, or either body off the board, means no cover and no behaviour change. The group model, the arena's group arm and `balance_sim` are all untouched.
- `COVER_EVASION_PERCENT` goes in `tuning.rs`'s **Combat resolution** section beside `EXPOSED_EVASION_PERCENT` (`tuning.rs:4090`), because it is read by the same function. The AI weights go in the **Tactical battle grid** section beside `TACTICAL_AI_CROWDING_WEIGHT` (`tuning.rs:5114`).
- `render/` draws only through `Painter`; no graphics library is named outside `paint.rs`. gui never touches `World`, `TacticalBattle` or `Board` state beyond the `Board` already on `TacticalView`.
- Engine tests live in `crates/engine/src/tests/`; fixtures come from `tests/support.rs`. Hand-written boards use `Board::from_rows`.
- Every task ends with `cargo fmt` and `cargo clippy --workspace --all-targets` clean. Run `cargo test --workspace` at the end of Tasks 4 and 7; targeted `cargo test -p feral-processes-engine <name>` elsewhere.
- Commit per green step on branch `feat/tactical-cover`. **Never push.** Never `git add -A`, never `git checkout`/`stash` over uncommitted work.
- No version bump and no `CHANGELOG.md` entry on the branch — both happen once, at the merge.

## File map

| File | Responsibility |
|---|---|
| `crates/engine/src/tactical/reach.rs` | `cover_between` — the one rule |
| `crates/engine/src/tuning.rs` | `COVER_EVASION_PERCENT`, `TACTICAL_AI_COVER_WEIGHT`, `TACTICAL_AI_COVER_TIEBREAK` |
| `crates/engine/src/game/combat_damage.rs` | `defender_profile_against`, and `resolve_and_apply_attack`'s defender half |
| `crates/engine/src/battle.rs` | `Swing::cover_ignored` and its constructors |
| `crates/engine/src/game/combat_round.rs` | `use_ability` sets the flag from the routine's shape |
| `crates/engine/src/tactical/ai.rs` | the merit and tie-break terms; `expected_reaction_damage` calls the new door |
| `crates/engine/src/tactical/view.rs` | `TacticalBody::in_cover`, `TacticalView::covered`, and `frozen` clearing them |
| `crates/gui/src/render/tactical.rs` | the shield mark and the covered-destination overlay |
| `crates/engine/src/tests/tactical.rs` | `cover_between`, the reachability census, the AI terms, the telegraph agreement |
| `crates/engine/src/tests/combat_status.rs` | the evasion raise, beside the Exposed tests |
| `CLAUDE.md`, `.claude/skills/seams/references/combat.md`, the memory graph | the seam's three writes |

---

### Task 1: `cover_between`, and proof it is reachable

**Files:**
- Modify: `crates/engine/src/tactical/reach.rs`
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Produces: `pub fn cover_between(board: &Board, attacker: (i32, i32), defender: (i32, i32)) -> bool`, pure, in `reach.rs` beside `line_of_sight`. True only when all three hold: the two cells are more than `TACTICAL_MELEE_RANGE` apart (Chebyshev, via the existing `distance`); at least one of the defender's eight neighbours is `BattleCell::Cover` **and** the dot product of (that neighbour minus the defender) with (the attacker minus the defender) is **strictly positive**; and `line_of_sight(board, attacker, defender)` is true.

**Notes for the implementer:**
- Order the three conditions cheapest-first: the distance check, then the neighbour scan, then line of sight. Line of sight is the only one that walks cells.
- Strictly positive is the whole of the 90-degree rule. A boulder exactly abeam of the defender scores zero and must not count; a `>= 0` comparison is the bug this test suite is written to catch. This is deliberately the opposite choice from the cone's epsilon in `shape_cells`, which admits equality because an eight-way grid's diagonals sit exactly on the wedge — here equality means "beside you, not between you and the shot", and there is no grid artefact to rescue.
- Read cover off `Board::cell(x, y) == BattleCell::Cover`, not off `blocks_sight()`. They are the same predicate today and `blocks_sight` is the one that will grow a second member if a fifth cell kind ever lands.
- Off-board neighbours read as `Blocked` through `Board::cell`, so they are excluded without a bounds check.

- [ ] **Step 1: Write the failing tests.** Five, all on hand-written boards via `Board::from_rows` — follow `across_cover` (`tests/tactical.rs:3150`) for the fixture idiom, but these need no `Game` at all, so prefer bare `Board`s and direct calls.
  - a boulder in the arc between attacker and defender gives cover;
  - the same boulder on the far side of the defender does not;
  - a boulder exactly 90 degrees off the attacker's bearing does not (the dot product is zero) — place attacker and defender on a straight vertical, cover directly left of the defender;
  - two cells apart with cover, but adjacent, gives no cover (melee ignores it) — assert at exactly `TACTICAL_MELEE_RANGE`;
  - a defender the attacker cannot see has no cover, even with a boulder in the arc.
- [ ] **Step 2: Run them and watch them fail** with "cannot find function `cover_between`".

  `cargo test -p feral-processes-engine cover_between`
- [ ] **Step 3: Write `cover_between`.**
- [ ] **Step 4: Run them and watch them pass.**
- [ ] **Step 5: Write the reachability census.** One test, `cover_is_reachable_on_every_biome_a_fight_opens_on`, over `Biome::OpenGrid`, `Deadlock`, `NullSector` and `Backplane`. For each, generate boards across several seeds and all three body counts, enumerate ordered walkable pairs beyond melee range that have line of sight, and assert the share with cover is **at least 10%**. Doc-comment it with the four measured figures from this plan's corrections table and say what it is for: cover that no generated board ever produces is a feature that ships green and dead, which this repo has shipped before. Keep the sweep small enough to stay under a second — sample seeds rather than exhausting them.
- [ ] **Step 6: Run it, and prove it is not vacuous** by temporarily raising the floor above the OpenGrid figure and watching it fail. Restore the floor.
- [ ] **Step 7:** `cargo fmt`, `cargo clippy --workspace --all-targets`, then commit.

  `git commit -m "Engine: cover_between, the one rule for partial cover (todo #104)"`

---

### Task 2: The evasion it buys

**Files:**
- Modify: `crates/engine/src/tuning.rs`, `crates/engine/src/game/combat_damage.rs`
- Test: `crates/engine/src/tests/combat_status.rs`

**Interfaces:**
- Consumes: `reach::cover_between` from Task 1.
- Produces:
  - `pub const COVER_EVASION_PERCENT: i32` in `tuning.rs`'s Combat resolution section, immediately after `EXPOSED_EVASION_PERCENT` (`tuning.rs:4090`). **Start at 50** — the Exposed magnitude, as the spec asks — and leave the final value to Task 7.
  - `pub(crate) fn defender_profile_against(&self, attacker: Entity, defender: Entity, swing: battle::Swing) -> battle::Combatant` on `Game`, in `game/combat_damage.rs` beside `combatant_profile`.

**Notes for the implementer:**
- The body: call `self.combatant_profile(defender, swing)`, then if a `TacticalBattle` resource exists and `cell_of` answers for **both** entities and `reach::cover_between(&battle.board, attacker_cell, defender_cell)`, multiply the returned `evasion` up by `(100 + COVER_EVASION_PERCENT) / 100`. Otherwise return the profile unchanged.
- Mirror the Exposed rung's shape exactly (`combat_damage.rs:149-152`): a late multiplicative percentage on the evasion, applied after `evasion_of`. Raising rather than cutting is the only difference.
- Use `self.world.get_resource::<TacticalBattle>()`, not `resource::<_>()` — this runs in the group model too, where the resource is absent, and a panic there is the whole group model.
- Then change `resolve_and_apply_attack` (`combat_damage.rs:228-231`) to build its defender half through the new door, passing `attacker`. Keep `battle::Swing::plain(self.natural_range_of(defender))` exactly as it is — that argument is the riposte's own band and has nothing to do with cover.
- Also change `expected_reaction_damage` (`tactical/ai.rs:865-868`) to call the new door with `reactor` as the attacker. It is a no-op by rule 1; it is a call so that it stays correct if rule 1 ever moves.

- [ ] **Step 1: Write the failing tests**, beside the Exposed ones in `tests/combat_status.rs`.
  - `cover_raises_the_defenders_evasion`: a hand-written board, two bodies at range with a boulder in the arc, assert `defender_profile_against(attacker, defender, …).evasion` exceeds `combatant_profile(defender, …).evasion` by exactly the constant's ratio.
  - `moving_the_attacker_around_the_boulder_removes_the_cover`: same board, same defender, attacker moved to the far side; the two profiles are equal. This is the flanking property and is the test that matters most.
  - `cover_changes_nothing_in_the_group_model`: no `TacticalBattle`, the two profiles are equal.
  - `the_evasion_raise_reaches_the_roll`: drive `resolve_and_apply_attack` over a seeded sweep of a few thousand swings with and without the boulder, and assert the covered arm lands strictly fewer hits. Use the existing seeded-sweep idiom rather than a single roll; a single roll cannot see a probability.
- [ ] **Step 2: Run them and watch them fail.**

  `cargo test -p feral-processes-engine cover`
- [ ] **Step 3: Add the constant and the door, then wire the two call sites.**
- [ ] **Step 4: Run them and watch them pass.**
- [ ] **Step 5: Prove the roll test is not vacuous** — set `COVER_EVASION_PERCENT` to 0 and watch it fail. Restore it.
- [ ] **Step 6:** `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test -p feral-processes-engine combat_status`, then commit.

  `git commit -m "Engine: cover raises a defender's evasion, at the one place both bodies are known (todo #104)"`

---

### Task 3: Which attacks cover applies to

**Files:**
- Modify: `crates/engine/src/battle.rs`, `crates/engine/src/game/combat_round.rs`
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Consumes: `Game::defender_profile_against` from Task 2.
- Produces: `pub cover_ignored: bool` on `battle::Swing`, defaulting to `false` (cover applies). `Swing::plain` and `Swing::reaction` both set it `false`. `defender_profile_against` returns the plain profile when the swing carries it `true`.

**Notes for the implementer:**
- The polarity is the point — see correction 2. Add the field with a doc comment saying so, in the same voice as `free`'s.
- The only thing that sets it is `use_ability`, from the routine's `AbilityDef::tactical_shape()`: `AbilityShape::Radius` sets it `true`, every other shape leaves it `false`. A blast flushes bodies out of cover. `Single`, `Line` and `Cone` are all penalised.
- Read the shape through `tactical_shape()`, **never** `def.shape` directly — `shape:` is `#[serde(default)]` and nothing shipped authors it, so a direct read resolves the whole roster to nothing and the failure is silent.
- `use_ability`'s `Swing` construction sites are `game/combat_round.rs:1578` and `:1618`. Check both; one of them may be the wielded proc.
- Heals and buffs roll no hit, so they are unaffected by construction and need no branch.

- [ ] **Step 1: Write the failing tests** in `tests/tactical.rs`.
  - `a_blast_ignores_cover`: a `Radius` routine's swing against a covered defender profiles identically to an uncovered one.
  - `a_line_is_penalised_by_cover` and `a_single_is_penalised_by_cover`: the same setup with those shapes raises evasion.
  - `a_default_swing_still_honours_cover`: `Swing::default()` has `cover_ignored == false`. This is the polarity regression and should fail if the field is ever flipped.
- [ ] **Step 2: Run them and watch them fail.**

  `cargo test -p feral-processes-engine cover`
- [ ] **Step 3: Add the field, then set it in `use_ability`.**
- [ ] **Step 4: Run them and watch them pass.**
- [ ] **Step 5:** `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test -p feral-processes-engine tactical`, then commit.

  `git commit -m "Engine: a blast flushes a body out of cover; every other shape is refused by it (todo #104)"`

---

### Task 4: What the AI does about it

**Files:**
- Modify: `crates/engine/src/tuning.rs`, `crates/engine/src/tactical/ai.rs`
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Consumes: `reach::cover_between` from Task 1.
- Produces:
  - `pub const TACTICAL_AI_COVER_WEIGHT: f32` and `pub const TACTICAL_AI_COVER_TIEBREAK: f32` in `tuning.rs`'s Tactical battle grid section, after `TACTICAL_AI_CROWDING_WEIGHT` (`tuning.rs:5114`).
  - `cell_merit` gains a cover term for ranged bands; `cell_score` gains the melee tie-break.

**Notes for the implementer:**
- In `cell_merit` (`ai.rs:188`): when `band.max > TACTICAL_MELEE_RANGE`, add `TACTICAL_AI_COVER_WEIGHT * share`, where `share` is the fraction of `targets` the candidate cell has cover against — `cover_between(board, target, cell)`, with the *target* as the attacker and the candidate cell as the defender. Only targets the cell can see should count, matching `hits`' own use of `line_of_sight`; `cover_between` asks sight itself, so the share's denominator is `targets.len()`.
- Keep it strictly below `TACTICAL_AI_REACH_SCORE` (40.0) and above `TACTICAL_AI_CLOSING_WEIGHT` (1.0). Full cover must not be worth walking out of reach for, and it must be worth more than one step of closing or nothing will ever take it. Start at **4.0** and say in the doc comment why it is bracketed rather than tuned alone — the three terms are read against each other.
- In `cell_score` (`ai.rs:166`): when `band.max <= TACTICAL_MELEE_RANGE`, add `TACTICAL_AI_COVER_TIEBREAK * share` — same share, and **smaller than `TACTICAL_AI_CROWDING_WEIGHT`** (0.5), so it breaks ties between cells that already reach and never outvotes crowding. Start at **0.25**.
- Do **not** put the melee term in `cell_merit`. `scored_cells` (`ai.rs:793`) uses merit as the candidate filter, and a melee body that can leave a closing cell for a covered one stalls behind boulders instead of closing, which is exactly the sidestep pathology the filter was added to fix.
- Both terms are pure — `cell_merit` and `cell_score` take no `&self` and must keep it that way.
- Neither function may spend a draw. The one draw a turn stays where it is.

- [ ] **Step 1: Write the failing tests** in `tests/tactical.rs`.
  - `a_ranged_hostile_takes_a_covered_cell_over_an_equal_uncovered_one`: a hand-written board where two cells reach the target equally and one has a boulder in the arc; drive one AI turn at temperature zero and assert the covered cell is chosen.
  - `a_melee_hostile_does_not_leave_a_closing_cell_for_cover`: a covered cell that is further from the target than the body's own; assert it closes instead.
  - `a_ranged_hostile_already_in_cover_holds_its_cell`: the staying-put rule still wins when there is nothing better.
- [ ] **Step 2: Run them and watch them fail.**

  `cargo test -p feral-processes-engine cover`
- [ ] **Step 3: Add the two constants and the two terms.**
- [ ] **Step 4: Run them and watch them pass.**
- [ ] **Step 5: Prove the first test is not vacuous** by setting `TACTICAL_AI_COVER_WEIGHT` to 0 and watching it fail. Restore it.
- [ ] **Step 6: Run the full suite** — `cargo test --workspace`. A changed AI term reshuffles seeded fights; if a seeded test elsewhere moves, read the code path before touching the test, and report any that moved rather than editing them silently.
- [ ] **Step 7:** `cargo fmt`, `cargo clippy --workspace --all-targets`, then commit.

  `git commit -m "Engine: a ranged body walks to cover, a melee one does not stall behind it (todo #104)"`

---

### Task 5: What the screen is told

**Files:**
- Modify: `crates/engine/src/tactical/view.rs`
- Test: `crates/engine/src/tests/tactical.rs`

**Interfaces:**
- Consumes: `reach::cover_between` from Task 1, `Game::defender_profile_against` from Task 2.
- Produces:
  - `pub in_cover: bool` on `TacticalBody` — whether this body has cover against the body whose turn it is, when that body is hostile. `false` on every other turn and on the results board.
  - `pub covered: Vec<(i32, i32)>` on `TacticalView` — the subset of `reachable` that has cover against at least one hostile the acting body can see.
  - `pub(crate) fn body_in_cover(&self, attacker: Entity, defender: Entity) -> bool` on `Game`, the door app-core calls while the player is aiming at a particular hostile.

**Notes for the implementer:**
- Build `covered` the way `provoking` is built (`view.rs:293-301`): filter `reachable`, one call per cell, in `tactical_view`. That is the pattern the spec points at and it keeps the screen and the roll on one function.
- `in_cover` is set in `body_view` (`view.rs:394`) against the acting body when it is hostile. During the player's own turn the mark is off; app-core turns it on for the aimed hostile through `body_in_cover` instead.
- **Clear both in `TacticalView::frozen` (`view.rs:191`)** — `in_cover` to `false` on every body, `covered` to empty — with the AI-only fields it already blanks.
- `body_in_cover` must agree with `defender_profile_against` by construction: both call `cover_between`, and neither reimplements the arc.

- [ ] **Step 1: Write the failing tests** in `tests/tactical.rs`.
  - `the_telegraph_agrees_with_the_roll`: over a hand-written board, for every pair of bodies, `body_in_cover(a, d)` is true exactly when `defender_profile_against(a, d, …).evasion` exceeds `combatant_profile(d, …).evasion`. This is the test the spec asks for by name and it is the one that must not be weakened.
  - `a_covered_destination_is_marked`: on a hostile's turn, a reachable cell with cover against a visible party body appears in `view.covered`, and one without does not.
  - `a_finished_fight_marks_nothing`: `frozen()` empties `covered` and clears every `in_cover`.
- [ ] **Step 2: Run them and watch them fail.**
- [ ] **Step 3: Add the two fields, the door, and the `frozen` clearing.**
- [ ] **Step 4: Run them and watch them pass.**
- [ ] **Step 5:** `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test -p feral-processes-engine tactical`, then commit.

  `git commit -m "Engine: the cover telegraph is a call into the rule the roll reads (todo #104)"`

---

### Task 6: Drawing it

**Files:**
- Modify: `crates/gui/src/render/tactical.rs`
- Test: `crates/gui/src/render/tactical.rs`'s own `mod tests`

**Interfaces:**
- Consumes: `TacticalBody::in_cover` and `TacticalView::covered` from Task 5.

**Notes for the implementer:**
- **The shield mark** goes in `draw_body` (`tactical.rs:533`), in the **top-right** corner — free on this board, since `nemesis_mark_rect` is never called here. Top-left is the con earmark and must not be disturbed; it already drops below the rarity bar, and the shield must do the same so the two corners read as a pair.
- **The destination overlay** goes in `draw_tactical_map`'s cell loop, in the same block as the reach and provoking washes (`tactical.rs:270-294`), reading `view.covered` through the existing `draw_cell_field` helper. Draw it **after** the reach wash and **before** the provoking wash: a cell can be both reachable and covered and both marks should read, but an inbound reaction is the louder news.
- **Pick the role carefully.** `THREAT` and `PLAN` are spoken for on this board, `AIM` is the placeable outline, `FORECAST` is the profiled hostile, `EMPHASIS` is the cursor, and `ATTENTION` is reserved and off-limits here. `HEALTHY` is the honest fit — cover is protection and nothing else on the board claims green. Check `palette.rs`'s separability tests before settling, and if `HEALTHY` collides, prefer a new role over overloading an existing one: a second meaning on one hue makes both unreadable.
- Use `Painter` only. No backend calls.
- Do not gate either draw on `view.covered` being non-empty in a way that changes layout — both are overlays and cost no space.

- [ ] **Step 1: Write the failing tests** in the file's own `mod tests`, following `every_body_on_the_board_is_drawn` (`tactical.rs:943`) for the `with_painter` idiom.
  - `a_body_in_cover_wears_a_mark`: a view with one body `in_cover` draws one more shape in the chosen role than the same view without it. Discriminate by shape and position the way `the_turn_arrow_names_the_side_whose_turn_it_is` does, not by colour alone.
  - `a_covered_destination_is_washed`: a view with a cell in `covered` draws a fill at that cell's origin.
  - `a_finished_board_draws_neither`: the frozen view draws neither mark.
- [ ] **Step 2: Run them and watch them fail.**

  `cargo test -p feral-processes-gui cover`
- [ ] **Step 3: Draw both marks.**
- [ ] **Step 4: Run them and watch them pass.**
- [ ] **Step 5:** `cargo fmt`, `cargo clippy --workspace --all-targets`, `cargo test -p feral-processes-gui`, then commit.

  `git commit -m "Gui: the shield mark and the covered-destination wash (todo #104)"`

---

### Task 7: The tuning pass and the seam's three writes

**Files:**
- Modify: `crates/engine/src/tuning.rs` (final value only), `CLAUDE.md`, `.claude/skills/seams/references/combat.md`
- Move: `docs/superpowers/specs/2026-09-16-tactical-cover-design.md` → `docs/superpowers/archive/specs/`
- Modify: `docs/superpowers/INDEX.md`

**Notes for the implementer:**
- **The tuning read.** `COVER_EVASION_PERCENT` starts at 50. Measure what it is worth with a seeded engine sweep over a hand-written board — same-level attacker and defender, a few thousand swings with and without the boulder — and report the hit-rate delta. The target the spec sets is "a noticeable share of a typical same-level swing's hit chance". Then run `cargo run --bin arena -- dev-arenas/tactical-full-group.ron` before and after for a regression read. **State plainly in the commit message that the arena arm speaks for one biome only**: the board takes the biome of the tile the arena's player stands on and a scenario cannot author it, so that run says nothing about Backplane, where cover is three times as common.
- If the sweep says 50 is too strong or too weak, change the constant and rerun. Do not adjust the AI weights to compensate — they are bracketed against `TACTICAL_AI_REACH_SCORE` and `TACTICAL_AI_CLOSING_WEIGHT`, not against this.
- **The seam's three writes**, in this order, per the `seams` skill:
  1. The **argument** to the memory graph as `seam:<slug>` with `entityType: "seam"`, `subsystem: "seams"` — two observations, title first. It should carry: why the arc rule is a dot product and why strictly positive; why the effect enters at `defender_profile_against` rather than in `combatant_profile`; why `cover_ignored`'s polarity is what it is; why the AI's term splits across merit and score; and the measured density table with what it was blind to (one biome in the arena arm; nothing played).
  2. The **trap** to `.claude/skills/seams/references/combat.md`, as a bullet in the house style — bold rule sentence, then the trap.
  3. The **rule** to `CLAUDE.md` under `### Tactical battles`, as **exactly one sentence**.
- Archiving the spec on landing is the habit the INDEX asks for. Move it and add its row. Note that `INDEX.md` is stale for three other specs that shipped in `v0.13.198`–`v0.13.200` and for roughly a dozen older ones — **that sweep is out of scope for this branch** and should be its own change.
- No version bump, no `CHANGELOG.md` entry. Both happen at the merge.

- [ ] **Step 1: Run the seeded sweep** and record the hit-rate delta at `COVER_EVASION_PERCENT = 50`.
- [ ] **Step 2: Run the arena before/after** and record what moved.
- [ ] **Step 3: Set the final constant** if the sweep calls for it, and rerun the Task 2 and Task 4 tests.
- [ ] **Step 4: Run the full suite** — `cargo test --workspace`.
- [ ] **Step 5: Write the argument to the memory graph.**
- [ ] **Step 6: Write the trap to the seams reference and the one-sentence rule to `CLAUDE.md`.**
- [ ] **Step 7: Archive the spec and add its INDEX row.**
- [ ] **Step 8:** `cargo fmt`, `cargo clippy --workspace --all-targets`, then commit.

  `git commit -m "Docs: the cover seam, and what the arena could not see (todo #104)"`

---

## What this plan does not build

Straight from the spec's own exclusions, restated so nobody adds them under the heading of finishing the job: destructible cover, low cover as a fifth `BattleCell` kind, elevation, and any flanking bonus beyond the loss of cover. Two more from this plan: the arena gains no biome parameter, and `terrain_weights` is not retuned.

**The squads interaction is still open.** `docs/superpowers/specs/2026-09-16-tactical-squads-design.md` puts a body on a 2x2 block, and which of its cells takes the arc test is undecided. The spec says whichever of the two lands second answers it. Cover is landing first, so the question moves to the squads plan.
