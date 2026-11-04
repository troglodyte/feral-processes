# Field auto-recast: plan

**Spec:** `docs/superpowers/specs/2026-11-04-field-auto-recast-design.md`. It
holds every decision and the reasons for them; this plan does not repeat them.
**Branch / worktree:** `worktree-field-auto-recast` at
`.claude/worktrees/field-auto-recast`.
**Crates:** engine, app-core, gui. Save: additive `#[serde(default)]`, no
`SAVE_FORMAT_VERSION` bump.

Run TDD on every task: a failing test first, then a commit per green step.
After each task, run `cargo fmt` and `cargo clippy --workspace --all-targets`
clean. Per-task review gates are off. The final review is opus, over the
whole branch.

## Phases

The plan has three phases, arranged so that each one fits in one context.
Phase 1 is one subagent (engine only). Phase 2 is one subagent (app-core and
gui), and it starts only once Phase 1 is merged into the branch. Phase 3 is
the controller's job.

Every dispatch prompt must forbid `git checkout`/`stash`/`push` and
`git add -A`, and must tell the subagent to stage explicit paths only.

---

## Phase 1: engine

Read `.claude/rules/seams-combat.md` and `seams-screens.md` first. One combat
seam is bent in Task 5, so read `seam:` *field buff lifetime* in the graph
(`memory_search("field buff lifetime", subsystem: "seams")`) before you
start.

### Task 1: data. Eligibility predicate, buff origin, holder flag, save

- `abilities.rs`: add a predicate on `AbilityDef`, for example
  `auto_recastable()`. It is true when the def is `field_runnable()`, its
  effect is `FieldBuff { kind, .. }`, and `!kind.runs_until_rest()`. It is
  the one rule; the menu, the toggle and the recast all call it.
- `components/effects.rs` `ActiveFieldBuff`: add
  `#[serde(default)] pub ability: Option<AbilityId>`. Every struct-literal
  site needs it; there are about 54 hits for `ActiveFieldBuff {`. Item and
  consumable sites get `None`.
- `components/staff.rs`: add a holder component next to `Routines`, for
  example `AutoRecast(pub Vec<AbilityId>)`, with `Default`. Add it wherever
  `Routines` is spawned or carried:
  - `roster_parts()`
  - the player spawn
  - `fuse_companions`, which hand-writes its own component list. That is a
    seam trap. Copy the dominant parent's flags only for routines the fused
    body still holds; otherwise drop them.
- `save/mod.rs`: add
  `#[serde(default, skip_serializing_if = "Vec::is_empty")] auto_recast: Vec<AbilityId>`
  on `PlayerSave` (~178) and on `CreatureSave` (~644). Write it from the
  component and restore it on load. Update the struct literals in
  `save/tests.rs:192,302` and `gui/src/render/test_support.rs` where they
  are `PlayerSave`/`CreatureSave`.
- `game/routines.rs:234` `uninstall_routine`: remove the id from that
  holder's `AutoRecast`.
- **Tests:**
  - The predicate is true for a crafted Field Regen and a Recharge, and
    false for Field Mitigation, Heal and Jump. Build them through
    `routine_craft` and don't hand-write defs.
  - A **save→load** round trip keeps the player's and a companion's flags,
    and keeps a buff's `ability`. Memory:
    ron-round-trip-cannot-catch-a-skipped-field, so it must go through
    `Game::save`/`load`.
  - Uninstalling a routine clears its flag.
  - Fusion keeps or drops flags per the rule above.

### Task 2: refactor. Inner cast function, no behaviour change

- `game/field.rs:227` `run_field_routine`: split out
  `fn cast_field_routine(&mut self, holder: Entity, ability: &AbilityId, pick: FieldRoutineTarget) -> Result<FieldCast, CastError>`.
  - It finds the row in `field_routines()` by `(holder, ability)` and
    refuses on `unavailable`.
  - It then runs everything the current FieldBuff/Heal/Phase/Jump/Symlink
    arms do: `field_recipients`, magnitude, `spend_power`,
    `arm_field_buff`, with `ability: Some(id)` set on each armed buff.
  - **No `tick()` and no "runs" log.**
- `CastError` is an enum that separates `TargetGone(String)` (any refusal
  from `field_recipients`) from `Refused(String)` (everything else).
  `Display` gives the sentence used today.
- `run_field_routine` keeps its battle and game-over guard and the index
  lookup. It calls the inner function, maps `Err` to `String` exactly as
  today, logs `"{holder} runs {name}."`, and calls `tick()`.
- **Gate:** the existing field-routine tests pass **unedited**, apart from
  the new `ability` field. `cargo test -p feral-processes-engine balance_sim`
  is unchanged. This path draws no RNG; confirm that by reading it.
- **Test:** a manual run arms the buff with `ability == Some(id)`.

### Task 3: the toggle API and the view

- `views/routines.rs:173` `FieldRoutineView`: add
  `pub auto_recast: Option<bool>`. It is `None` when the routine is
  ineligible and `Some(on)` otherwise. Set it only in
  `game/field.rs` `field_routines()`, and fix any test literals.
- `Game::toggle_field_auto_recast(&mut self, index: usize) -> Result<bool, String>`
  uses the same index space as `run_field_routine`, flips the row
  holder's flag, and returns the new state.
  - If the row is ineligible, return `Err("{name} doesn't run out on its own.")`.
  - If the index is bad, return `Err("No such routine.")`.
- **Tests:**
  - Toggle on, then off.
  - The view reflects the flag.
  - Two holders with the same routine are toggled independently.
  - An ineligible row refuses with that sentence.

### Task 4: recast when a buff expires

- `game/combat_status.rs:437` `tick_field_buffs`: the expiry pass collects
  `(recipient, buff.name, buff.ability)` instead of names alone, drops the
  borrow, and then:
  - **gather:** for each expired buff with `Some(ability)`, find the first
    holder in `routine_holders()` order that has `ability` installed and
    flagged, and record the def's `AbilityTarget`. Put the result in a
    plain input struct sorted by a stable key: the holder's position in
    `routine_holders()`, then the ability id.
  - **decide (pure, in a new `game/field_recast.rs` or similar):**
    `fn plan_recasts(input) -> Vec<Recast>`. It groups by `(holder, ability)`
    so a WholeParty routine recasts **once**. A OneAlly recast becomes
    `Ally(recipient)` and a WholeParty one becomes `None`. An expiry with no
    flagged holder becomes `Fade(name)`.
  - **apply:** call `cast_field_routine` for each recast.
    - `Ok`: log `"{holder} reruns {name}."`.
    - `Err(TargetGone(r))`: log `"{name} fades — couldn't rerun it: {r}."`
      and keep the flag.
    - `Err(Refused(r))`: log `"… {r}. Auto-recast is off."` and clear the
      flag.
    - `Fade`: log `"{name} fades."`, unchanged from today.
- **Check first** whether `tick_inner` runs while a battle is active. The
  spec makes the recast proceed regardless. Record which way it goes in
  the doc comment of `plan_recasts`.
- **Tests:**
  - The decide step directly: grouping, target mapping, and an unflagged
    expiry fades.
  - Through the game, from a `dev-saves/` template where one serves rather
    than `Game::new`:
    - A flagged Field Regen recasts on expiry and spends Power exactly
      once.
    - A WholeParty Recharge recasts once.
    - Short of Power, it fades, logs the line, and the flag goes off.
    - When the ally is benched, it fades, logs the line, and the flag
      stays on.
    - An unflagged routine still fades with today's line.
  - Mutation-check the Power-once assertion: remove the grouping and
    confirm the test fails (skill `mutation-testing`).

### Task 5: rest clears every routine buff (this bends a seam)

- `components/effects.rs:612` `drop_until_rest_buffs`: rename it to match
  its new scope, for example `drop_routine_buffs`, and filter on
  `b.source == BuffSource::Routine`, timed ones included. Consumable buffs
  stay. Update both callers:
  - `Game::drop_until_rest_buffs_on_party` (`combat_status.rs:402`; rename
    it too), called from `rest` at `turn.rs:1630`.
  - `difficulty.rs` `PartyFieldBuffs::drop_until_rest` (Forgiving reboot).
- Confirm that rest runs no tick loop after the drop (seams-combat: "no
  rest advances the clock"). If some `tick_inner` does run after it, there
  is now nothing timed left to expire, so no recast can fire. Assert that
  in a test.
- `runs_until_rest` keeps its meaning ("does not count down"). Fix doc
  comments that say timed buffs survive a rest (`effects.rs` ~540-570,
  `combat_status.rs` ~470, `abilities.rs` ~546).
- **Seam: write three places, in the order the `seams` skill documents.**
  1. The graph `seam:` entity for field-buff lifetime.
  2. The skill's trap text.
  3. The `.claude/rules/seams-combat.md` line "A field buff's lifetime is
     decided by its kind *and* its source…". Add that rest ends every
     routine buff, and that `runs_until_rest` decides only ageing.
- **Tests:**
  - A rest with a running Field Regen (timed) clears it and logs that it
    faded.
  - A consumable Regen survives a rest.
  - A Forgiving reboot clears timed routine buffs.
  - Update any existing test that asserted a timed buff survives a rest,
    and say so in the commit message.

**Phase 1 gate:** `cargo test --workspace` (unset `FERAL_DEV_NO_SIEGES`),
`balance_sim`, clippy and fmt. Commit.

---

## Phase 2: app-core and gui

Read `.claude/rules/seams-screens.md` first.

### Task 6: the key

- `app-core/src/app/field.rs:29` `handle_field_routine_key`: before
  `selected_index`, handle `GameKey::Char('A')` (uppercase, matching
  `party.rs:305` and `progression.rs:103`; memory:
  key-hint-case-is-the-handler-case).
  - Call `game.toggle_field_auto_recast(self.menu_selected.min(len - 1))`
    and stay in `Mode::FieldRoutine`.
  - `Ok(on)` → `self.report(...)` with `"Auto-recast on for {name}."` or
    `"Auto-recast off for {name}."`. Do the same as `toggle_party_member`
    (`party.rs:133`).
  - `Err` → `self.refuse(line)`.
  - Do nothing on an empty list.
- **Tests** (`app-core/src/tests/field.rs`):
  - A toggles the highlighted row and the mode stays.
  - An ineligible row refuses.
  - The list highlight doesn't move.

### Task 7: the row and the footer

- `gui/src/render/field.rs:227`: when `r.auto_recast == Some(true)`, the
  label becomes `"{name} [auto] — {cost} — …"`. There is no tag when it is
  off or ineligible, and the row count is unchanged.
- `field.rs:244`: footer chips `&["Enter run", "A auto", "Esc cancel"]`.
- **Test:** add a `gui/src/render/tests/` row check that the tag appears
  only when the flag is on. Check that the widest realistic crafted name
  plus `[auto]` still fits the popup (memory:
  popup-row-width-is-testable-headlessly, gui-text-never-clips-or-wraps).

**Phase 2 gate:** `cargo test --workspace`, clippy and fmt. Commit.

---

## Phase 3: controller

1. Spot-check one subagent claim per phase. Don't re-run the full suite to
   confirm a report.
2. Final review: one opus agent over `git diff origin/main...HEAD`, given
   as a file. Ask it to independently check:
   - that a manual cast and a recast cannot price differently;
   - that WholeParty recasts once;
   - the two failure outcomes;
   - that fusion and uninstall handle the flag;
   - that the seam update was written in all three places.
3. Fix the findings and send the fixes for their own review (memory:
   a-reviews-fix-needs-its-own-review).
4. Add `docs/superpowers/INDEX.md` entry (spec + plan). CHANGELOG at
   deploy (patch bump: no save break). Don't edit the manual or README.
