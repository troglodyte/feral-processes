# Plan: breeding visit and zone-2 Bloodlines gate

**Spec:** `docs/superpowers/specs/2026-11-05-breeding-visit-design.md`, which
is the source of truth for behaviour and copy.

**Branch/worktree:** `worktree-breeding-visit` at
`.claude/worktrees/breeding-visit`.

**Crates:** engine (most of the work), app-core (thin), gui (picker and sheet
copy), assets.

**Save:** additive `#[serde(default)]` fields only. The save is field-named
RON, so there is **no `SAVE_FORMAT_VERSION` bump** and the release is a patch.
The new fields still need a save→load test.

## Rules for every phase

- TDD: write a failing test first, and commit at each green step.
- After each phase:
  - `cargo fmt`;
  - `cargo clippy --workspace --all-targets`, with no warnings;
  - `cargo test -p <crate> <filter>` for that phase's crate.
- Read before touching code:
  - `.claude/rules/seams-base.md` (ProgramRole, drift, needs, both
    destruction paths);
  - `seams-research.md`;
  - `seams-memories.md`;
  - `seams-species.md`;
  - the matching traps in the `seams` skill.
- Unset `FERAL_DEV_NO_SIEGES` before running any test.
- Never `git checkout` or `stash` uncommitted work. Stage explicit paths. Never
  push.

## The model

These decisions are made here, so the phases below can rely on them.

- **The pending child is a slot state.** `breeding::Incubation` gains
  `#[serde(default)] pub visit: Option<VisitPause>`, with
  `VisitPause { pause_left: u64 }`.
  - `Some(..)` means the slot is reserved and the parents are on their way or
    paused at the bay.
  - `due` is meaningless while `visit` is `Some`. It is set to
    `now + INCUBATION_TICKS` when the visit completes, and `visit` becomes
    `None`.
  - `breed()` still rolls the child at confirm, so the RNG order is unchanged.
    It writes `visit: Some(VisitPause { pause_left: BREEDING_VISIT_PAUSE_TICKS })`.
- **The parent marker** is `components::BreedingVisit { bay: Entity,
  slot: usize }`, inserted on both parents at confirm.
  - Give it the same doc shape as `components::UnderStudy`.
  - When `breed()` inserts it, drop the parent's `Task`, following
    `pin_subject`'s unposting. Read how `pin_subject` does it and call the same
    helper.
- **The role** is a new `ProgramRole::Visiting`, read from the marker in
  `role_of`/`RoleMarkers`. It ranks just above `Staff`.
  - Party, Wielded, Sortie and Outpost still win over it. A parent pulled away
    therefore reads as something other than `Visiting` while it still carries
    the marker, and that mismatch is the cancel signal.
  - Being not-`Staff` takes the parent out of labour posting, and out of
    `needs_drain_system`, which filters `== Staff`. That gives the spec's
    "needs hold" with no needs-code change. Verify it with a test, not by
    reading the code.
  - Widen the three exhaustive matches the `UnderStudy` doc names:
    `roster_rank`, gui `role_heading`, and the rest-repair branch in
    `game/turn.rs`. Then grep `== ProgramRole::Staff` and `ProgramRole::` for
    readers that need an explicit decision. `add_companion` etc. are allowed;
    joining the party cancels the visit.
- **The walk** is a new arm in `drift_idle_staff`
  (`game/base/work_orders.rs`, beside the `UnderStudy` arm at about line 2748),
  gated the same way (`!sharing && on_floor`). It calls a new
  `Game::step_to_bay(worker) -> Result<(), NoPost>` in
  `game/base/breeding_visit.rs` (new file), modelled on
  `study.rs::step_to_study`.
  - The target is a free standing cell at one of the bay's four sides
    (`hauling::touch_for`), one per parent.
  - First check how `drift_idle_staff`'s worker list admits `UnderStudy` bodies
    that are not `Staff`, and admit `Visiting` the same way.
- **The per-tick step** is `Game::step_breeding_visits()`, called from
  `turn.rs` next to `hatch_incubations()` (about line 347), under the same
  `Busy` gate. It follows gather/decide/apply:
  - **Gather** (`&self`): a `Vec<VisitInput>` sorted by `(bay Entity, slot)`.
    Each entry holds the bay, the slot, the parents found with a matching
    marker, each parent's role, whether it carries `Downed`, whether it has
    arrived, and `pause_left`.
  - **Decide**: the pure `fn decide_visits(&[VisitInput]) -> Vec<VisitOutcome>`,
    where `VisitOutcome` is `Wait`, `Tick`, `Complete` or
    `Cancel(CancelReason)`. A visit cancels when:
    - it has fewer than 2 parents (despawned, released, fused, or an orphan
      after load);
    - a parent's role isn't `Visiting`;
    - a parent is `Downed`.

    `Tick` happens only when both parents have arrived.
  - **Apply** (`&mut self`):
    - `Complete` sets `due`, clears `visit`, removes both markers, inserts
      `BreedReadyAt` (moved here from `breed()`), writes `bred_with` to each
      parent about the other, and logs the "child starts" line.
    - `Cancel` goes through the one door,
      `cancel_breeding_visit(bay, slot, reason)`.
- **The one cancel door** is `Game::cancel_breeding_visit(bay, slot,
  CancelReason)`. It:
  - clears the slot;
  - removes the markers from whichever parents still carry them;
  - returns one `breeding_seed` to the player's pack (inventory insert);
  - writes one `log_base` line naming who and why.

  It is called from:
  - `apply`;
  - **both** destruction paths, `remove_structure` and `damage_structure`,
    before the bay entity goes;
  - load, when an orphan is found (via the first `step`, so there is no
    separate load path).
- **The cooldown** moves from `breed()` to completion: no cooldown on cancel.
- **New refusals** in `BreedRefusal`, checked by `breed_refusal`, which the
  picker already greys rows through:
  - `AwayFromBase`: Party, Wielded, Sortie or Outpost. Reason text "away from
    base".
  - `Held`: UnderStudy, Jailed, Siphoned or BaseAi. Reason text "held".
  - `OnVisit`: already `Visiting`. Reason text "with the bay".
  - `Recovering`: carries `Downed`. Reason text "recovering".

  Add all four to gui `render/breeding.rs`'s exhaustive list (about line 260).

## Phase 1: the gate and the memory (engine and assets)

Files:

- `assets/research/bloodlines.ron` (new):
  - `min_zone: 2`, `requires_subject: true`, `discoverable: true`;
  - `unlocks_structures: ["breeding_bay"]`, `requires: []`;
  - cost and materials in the range of the existing zone-2 nodes (`rg -l
    'min_zone: 2' assets/research`);
  - a plain-language description.
- `assets/memories/bred_with.ron` (new), modelled on `bonded_in_battle.ron`:
  - `subject: Program`;
  - name "Raised a child with";
  - add `spreads_as`/`known_for` only if the memories README or a census asks
    for them.
- `assets/research/README.md` and `assets/memories/README.md`, if they list
  entries.

Tests:

- **Update** `crates/engine/src/tests/research/gating.rs` at about line 201:
  - assert the bay is not in `buildable_structure_defs()` on a fresh game;
  - assert it is buildable once `bloodlines` is researched.
- **Update** `exactly_the_named_research_nodes_are_discoverable`
  (`tests/assets/structures_research.rs` at about line 582) to add
  `bloodlines`.
- Confirm that
  `every_discoverable_research_node_is_reachable_from_the_visible_tree` and
  the memory census pass.
- **Grep** the engine, app-core and gui test fixtures for setups that build a
  bay on a fresh game (`rg -n 'breeding_bay' crates/*/src/tests`). Make those
  fixtures research `bloodlines` first, through the existing test helper for
  granting research (find it, don't write a new one).

Gate: `cargo test -p feral-processes-engine research`, `-- breed` and
`-- memor`.

## Phase 2: the visit (engine)

Files:

- `breeding.rs` (`Incubation.visit`, `VisitPause`, the new `BreedRefusal`
  variants);
- `components/staff.rs` (`BreedingVisit`; make `Incubator::is_occupied` ignore
  pending slots so demolition isn't refused);
- `game/party.rs` (the role);
- `game/breeding.rs` (`breed()` changes, plus `hatch_incubations` skipping
  `visit.is_some()`);
- new `game/base/breeding_visit.rs` (`step_to_bay`, `step_breeding_visits`,
  `decide_visits`, `cancel_breeding_visit`);
- `game/base/mod.rs`;
- `game/base/work_orders.rs` (the drift arm);
- `turn.rs`;
- both destruction paths;
- `tuning/sorties.rs` (`BREEDING_VISIT_PAUSE_TICKS = 60`);
- `views/programs.rs` (the bay status line for a pending slot: "parents on
  their way" / "parents at the bay"; also check `IncubationView`);
- save:
  - `CreatureSave` gets `#[serde(default)] breeding_visit: Option<((i32, i32),
    usize)>`, holding the bay tile and the slot;
  - `save.rs` writes it the way `UnderStudy` does at about line 98;
  - `restore.rs` resolves it after structures, the way it does at about line
    589;
  - an unresolvable tile drops the marker silently, and the next step's
    orphan check refunds.

Tests (new `crates/engine/src/tests/breeding_visit.rs`; reuse the
`tests/breeding.rs` fixtures):

1. `decide_visits` unit tests: each cancel reason, waiting while not arrived,
   ticking when both have arrived, and completing at `pause_left == 0`.
2. **Through `Game::tick` only** (a hard requirement):
   - confirm, then tick until both parents stand beside the bay and the child
     is incubating with `due` set;
   - both parents get `bred_with` about each other;
   - both get the cooldown;
   - both are `Staff` again and get re-posted by labour scheduling.
3. A pending slot does not hatch, even past the original `due`.
4. Cancels, each through the tick:
   - a parent added to the party;
   - a parent dispatched on a sortie;
   - a parent `Downed`;
   - a parent despawned;
   - the bay demolished (`remove_structure` succeeds mid-visit);
   - the bay destroyed (`damage_structure`).

   Each one refunds the seed, frees the slot, leaves no markers, writes no
   memory and starts no cooldown.
5. **Needs hold**: a visiting parent's needs don't drain, and it never goes
   `OffShift` or writes `frayed_here` or `ran_down`, across a visit long enough
   to drain them otherwise.
6. A battle pauses the visit: `pause_left` doesn't move.
7. Refusals: each new `BreedRefusal` variant, and `breed()` refusing an
   on-visit parent.
8. **Save→load mid-walk and mid-pause**, through the real save path, not a
   RON round trip. The visit resumes and completes.
9. An orphan pending slot after load is refunded.
10. The existing `tests/breeding.rs` tests that expect instant incubation run
    the visit to completion first. Fix the expectations, not the rule.

Mutation-check tests 2, 4 and 5: remove the drift arm, the cancel call in
`damage_structure`, and the role variant, and confirm the tests fail. Restore
each change afterwards with Edit, not git.

Gate:

- `cargo test -p feral-processes-engine breed`;
- `cargo test -p feral-processes-engine save`;
- `cargo test -p feral-processes-engine balance_sim`, which should not move;
- clippy.

## Phase 3: menus, sheet and copy (app-core, gui, assets)

- **app-core** `app/breeding.rs`: confirming now starts a visit. Check that
  the existing app-core breeding tests still read right, and update the
  `confirming_breeds_and_consumes_the_seed` expectation.
- **gui** `render/breeding.rs`:
  - the second-parent popup hint becomes "Esc back" (about line 125);
  - the confirm page adds one line saying both parents will walk to the bay
    first, and the child starts once they're done;
  - add the four new refusals to the exhaustive list;
  - the bay sheet shows the pending-slot status from the view.

  If a renderer test covers the hint strings, update it, and check the popup
  width headlessly (memory `popup-row-width-is-testable-headlessly`).
- **gui** `role_heading` gets the `Visiting` heading ("At the bay").
- **Copy** (spec "Copy" section):
  - `assets/help/46-breeding.md`: the gate (discovered by study once you
    reach zone 2), base-only parents, the walk and pause, the child starting
    after, the memory, cancel and refund, seeds must be in your pack, and the
    roll ceiling caps out;
  - `breeding_bay.ron` and, if it is stale, `breeding_seed.ron`;
  - `rg -i 'breed' assets/` for any other stale lines;
  - split the log lines (visit starts / child starts).

  Plain language, no tick vocabulary.

Gate:

- `cargo test -p feral-processes-app-core breeding`;
- `cargo test -p feral-processes-gui breeding` (or the gui crate's real
  name);
- `cargo test --workspace`;
- clippy, fmt.

## Finish

- Final whole-branch review (opus). Give it the diff as a file, with the spec
  and this plan.
- Fix what it finds, then have a review of the fixes.
- Add `docs/superpowers/INDEX.md` rows.
- Seam: add one rules line to `seams-base.md` ("A breeding visit is a sixth
  held `ProgramRole`; `cancel_breeding_visit` is the one door, called from
  both destruction paths"), with the graph and skill entries per the `seams`
  skill.
- Landing (version bump, CHANGELOG, tag) is the deploy skill's job, done on
  the user's word.
- Note for later, out of scope: the "Any key back" hint on every structure's
  Info page in the `[c]` hub.
