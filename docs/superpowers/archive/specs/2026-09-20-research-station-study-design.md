# The Research Station and studying a program

**Date:** 2026-09-20
**Status:** implemented on `feat/research-station-study`, all twelve tasks plus
a review pass (C1-C2, I1-I3, M1-M8). Archived on landing 2026-09-21. Not yet
played at the keyboard. The seam arguments are in the memory graph as
`seam:structure-footprint` and `seam:under-study`.
**Todo:** no number; raised in conversation.

The Research Node grows from one cell to a 2x2 Research Station. Its anchor
cell stays the impassable block that carries the `R` glyph; the other three
are the lab's own walkable floor, and the cell diagonally opposite the
anchor is a **pen**. A tamed program pinned in that pen is *under study*,
and every research node from sector 2 up refuses to be selected until one
is. When the project completes the subject is spent: it becomes an
`items::DownedProgram` record in the player's store, where the existing
extraction loop can take it apart. One of those newly subject-gated nodes,
`program_refactoring`, is what unlocks fusing two programs — a capability
that is ungated today.

The point of the feature is that **research stops being free base labour**.
The graph records "progression is earned by fighting" as a design spine and
records research as an open violation of it: a posted program grinds
Research Data out of a node without the player doing anything. A study
demands a body, and a body has to be captured. That is the debt this pays.

## Decisions taken in brainstorming

| Question | Answer |
|---|---|
| One research building or two | One. The 2x2 Station replaces the 1x1 Node; basics need no subject |
| Which nodes demand a subject | Every `min_zone >= 2` node — 19 of the 27 shipped |
| Does which program matter | No. Any owned tamed program satisfies any study |
| What is left afterwards | An `items::DownedProgram` record, extractable at a Compiler |
| Who stands in the three open cells | One subject in the pen; the worker posts from outside the footprint |
| How "pinned" is drawn | Four red L-brackets on the tile-edge ring |
| Fusion's gate | The research alone — no structure, no locality, no grandfathering |
| Migration | Same structure id; the footprint is derived, so no save migration |
| Are subject-gated nodes hidden | No. They list as blocked with a reason (see §3) |

## 1. Architecture

### The two new authored fields

`StructureDef` (`crates/engine/src/structures.rs`) gains two fields, both
`#[serde(default)]` per the schema rule, so every existing and modded
`.ron` keeps parsing untouched:

- **`footprint: u8`**, default 1. The side of a square claim, **anchored
  top-left**. This is deliberately the same meaning and the same anchoring
  as `tuning::Formation::footprint` and `TacticalBattle::footprint_cells_at`,
  so there is one spelling of "a square footprint" in the repo rather than
  two that can drift.
- **`studies: bool`**, default false. This machine can hold a subject under
  study, and therefore has a pen.

`research_node.ron` keeps `id: "research_node"` and gains
`footprint: 2, studies: true`. Its name and description change; nothing
else about it does — it still declares
`work: (produces: "research_data", ticks_per_unit: 14, level: Some(1))`,
still upgrades to tier 5, still draws 1 Power, still costs 10 Core
Fragments, and is still unlocked from the start (no research node names it,
which is what `Game::structure_unlocked` reads as unlocked).

### The anchor cell is the block

This is the load-bearing decision of the whole section, and it is what makes
"same structure id" cost nothing.

The 2x2's **anchor** is the impassable block, and it carries the `R` glyph.
The other three cells are the station's own **walkable floor**. So a
legacy 1x1 Research Node standing in an existing save already occupies
exactly the cell it will keep occupying, already blocks exactly what it
blocked before, and already draws its glyph where it drew it. The three new
cells are purely additive. There is no load-time reconciliation, no write
to `BaseGrid`, no displacement of neighbours, and no `SAVE_FORMAT_VERSION`
bump.

The alternative — a 2x2 slab where all four cells block — would have needed
a migration rule for every Node in every existing save whose neighbours are
occupied, and would have been the one version of this feature that could
break a base the player already built.

### The footprint is a claim on placement and a derivation on read

It is **never stored**. Nothing in the save says which cells a structure
owns, and nothing in the world holds a second `Position`.

`Game::place_structure` (`crates/engine/src/game/base/building.rs:24`)
gains one refusal: every cell of the footprint must be floor, hold no
structure, hold no build site and carry no dig mark. Its three existing
refusals are all single-cell point queries against the anchor
(`building.rs:116`, `:121`, `:128`) and each widens to the footprint.

Because the claim is only checked at placement, **a legacy Node whose three
new cells are occupied simply has a cramped footprint.** Nothing breaks and
nothing is refused retroactively. If the pen in particular is unusable, the
pin refusal in §2 is the surface that says so, and the fix is the player's:
demolish the neighbour, or cut the rock out.

### The readers that stop measuring from a bare anchor

This list *is* the work of this section, and it is the place the feature is
most likely to ship a silent defect. The tactical-squad seam records exactly
this failure: a squad was the first body in the game that was not one cell,
and two readers that measured from a bare anchor shipped on the branch
**invisible to 5964 passing tests**, because every fixture that could have
caught them was hand-built at footprint 1.

| Reader | Where | What changes |
|---|---|---|
| `Game::structure_tiles` | `game/zone.rs:274` | Yields every footprint cell, not one `Position` |
| `Game::blocked_tiles` | `game/zone.rs:285` | Structures contribute their **blocking** cells only |
| `hauling::blocked_tiles` | `game/base/hauling.rs:190` | Same; this signature *is* the one-cell assumption |
| `hauling::station_candidates` | `game/base/hauling.rs:245` | Faces of the whole footprint, not of the anchor |
| `Game::find_blocking_structure_at` | `game/base/building.rs` | Point-in-footprint, not point-equality |
| `Game::build_site_at` | `game/base/building.rs` | Same |
| `Game::place_structure` | `game/base/building.rs:24` | The footprint refusal above |

**`hauling::blocked_tiles`'s signature is the structural choice in this
feature.** It takes the structures and the bodies as two iterators of
`Position` — `collect::feeders_by_tile`'s argument, so `post_field` and
`crew_reach` cannot ask different questions about which cells are
crossable. One `Position` per structure is the one-cell assumption expressed
in a type, and that is the thing being changed. Run the `design-patterns`
dialog on this before writing it; it is the one decision in the feature that
should not be made alone.

Note the split in that table: **`structure_tiles` yields every footprint
cell while `blocked_tiles` yields only the blocking one.** They are already
documented as not interchangeable — `structure_tiles` is the narrower set
today and `hauling::has_station` is its only caller, on the argument that a
body standing on the only face of a marked cell is *proof* something can
stand there. After this change they diverge for a second reason, and both
rules have to be stated at both sites.

### The pen

**Derived, not authored:** the pen is the footprint cell diagonally
opposite the anchor. `Game::study_pen(structure) -> Option<(i32, i32)>`
answers `None` for a structure whose def does not declare `studies`, and is
the one door — the pin, the walk, the block check, the draw and the
consumption all call it rather than each re-deriving a corner.

The remaining two cells have **no mechanical job**, deliberately. That is
the price of the building: four cells of base space, where space is a real
cost because one body occupies one cell and everything else has to be cut
out of rock. They stay walkable so the lab can never box anything in — the
Depot trap in the base seams ("a Depot with four occupied orthogonal tiles
is a Depot nothing can reach") is what that guards against.

Multiple Stations are allowed and each has its own pen. Where a rule needs
to pick one — `research_block`'s "is any subject pinned", and
`settle_research`'s "which subject is spent" — the choice is the
**`(x, y)`-sorted first**, which is `assembler_system`'s rule: bevy's query
iteration order is not stable, and two stations competing would otherwise
resolve differently between runs and encode differently in the save.

## 2. Being under study

### A fifth `ProgramRole`

`ProgramRole` (`crates/engine/src/game/party.rs:20`) gains **`UnderStudy`**,
sitting **between `Sortie` and `Staff`**. `Staff` stays what is left over,
which is the rule the enum already holds.

This is `Sortie`'s precedent, taken for `Sortie`'s reason: a sortie's
consequences are **omissions rather than checks**, and the same is wanted
here. A pinned program must not be handed a job by
`schedule_base_labour`, must not wander, must not be recalled into the
party, must not be one of the two bodies fusion consumes, and must not be
healed by a rest. Every one of those is a reader that matches exhaustively
on `ProgramRole`, so adding the variant makes each of them **fail to
compile until it decides** — which a marker component tested ad hoc at each
site would not.

`Game::program_role` delegates to the free function `role_of` so a bevy
system with no `Game` can ask; `UnderStudy` is derived there like every
other role.

### The authoritative state, and what is derived off it

`components::UnderStudy { station: Entity }` is the only stored fact.

**Arrival is derived**, not stored: the subject is *in* its pen when its
`Position` equals `Game::study_pen(station)`. A stored `arrived: bool`
would be a second writer of a fact the body's own coordinates already
answer, and the repo's own rule is that a figure like this is a call rather
than a field.

### The subject walks to the pen

`drift_idle_staff` gains an `UnderStudy` arm **above** its `Downed` arm,
gated on laid floor. That is the same shape and the same gate the `Downed`
arm already has: a program downed in the Stack has to arrive through
`entry_tile` rather than teleport, and a pinned one is no different.

Pinning writes **no `Position`**, keeping `post_worker`'s rule that a body
sets off from the tile it is standing on.

Two consequences:

- `party::walks_the_base` widens to include a body under study. It occupies
  ground — it is standing in the pen — and one-body-to-a-cell has to hold
  there or two bodies share the pen.
- `drift_idle_staff` is handed the staff list *plus* the bodies under study.
  A pinned program is no longer `Staff` by derivation, so it would otherwise
  fall out of the only pass that walks anything, and would stand still
  forever wherever it was when it was pinned.

The two arms are disjoint in practice — a `Downed` program is benched and
cannot be pinned — but the order is stated so the disjointness is a claim
somebody can test rather than an accident of which branch came first.

### Pinning and unpinning

`Game::pin_subject(program, station)` and `Game::unpin_subject(program)`
are the two doors, and **every refusal lands before anything is written**,
asserted per refusal — the `select_research` rule, for its reason: a single
test over one of several refusals passes against all the ones that never
write anyway.

`pin_subject` refuses: game over or an active battle; the program is not a
tamed program the player owns; it is already under study; its current role
is not `Staff` (a partied, wielded or away-on-sortie program has to come
home first, and saying so is better than silently recalling it); the
structure does not declare `studies`; the pen is not floor or already holds
a body; and the program cannot route to the pen.

`unpin_subject` refuses while the active project requires a subject,
naming the project and saying that abandoning it is how you change your
mind. That is verbatim the shape of `select_research`'s "a project is
already active" refusal, and it is what stops a player pulling the subject
out from under a project that is half paid for.

### The UI

One new group-menu row and one new `Mode`:

- The row sits with the base entries in
  `crates/app-core/src/app/group_menu.rs`, labelled for study, with a
  `Locality` that requires base space and an `available:` that requires a
  `studies` structure standing.
- `Mode::PinSubject` is a program picker over `Game::base_staff`, the same
  shape the build flow's program picker already has.

Two traps this has to clear, both recorded:

- **`ALL_MODES` does not fail to compile.** The list is hand-written and the
  draw match ends in `_ => {}`, so a new `Mode` ships as a *blank screen*.
  The new variant goes in `ALL_MODES`, in the draw match, and in
  `needs_status_banner`.
- **Lowercase letters are row selectors.** Any key this screen binds beyond
  row selection and Enter must be uppercase, or one keypress both picks a
  row and fires it.

Unpinning is the same row, which reads as unpin when a subject is already
pinned — one row, not two, because "there is at most one subject per
station" is the rule and a second row would be a second place to state it.

## 3. The subject gate on research

### The authored field

`ResearchDef` (`crates/engine/src/research.rs:41`) gains
**`requires_subject: bool`**, `#[serde(default)]`. The 19 files with
`min_zone >= 2` each get the line. The 8 ungated nodes do not:

`automation`, `power_grid`, `commerce`, `teardown`, `fortification`,
`armor_bench`, `routine_fabrication`, `weapon_bench`.

Those eight are what gets a base running — the Compiler, the Power Conduit
that makes Power Cells, the Shield and Patch Node, the Armory, and the
Log Scraper / Lathe / Transcriber / Disk Press chain that
`routine_fabrication` opens along with the routine tree. A new run reaches a
working, defended, producing base with nobody pinned. Everything from
sector 2 up costs a body.

Synthesised routine nodes are **not** subject-gated. They are minted by
`routine_tree::synthesise_nodes` with `requires_subject` at its default,
and the routine tree has its own economy already — you discover a family by
extracting a rung. Two gates on one tree would be one too many.

### Where the gate is checked

Inside **`Game::research_block`** (`game/unlocks.rs:727`), as a third block
reason beside "no Research Node standing" and the per-material
`chain_break` line. Not as a new arm bolted onto `select_research`.

That placement is the seam's own rule: `research_block` is what fills
`ResearchStatus::blocked_by`, so **the row the screen marks blocked and the
refusal the player is handed cannot disagree**. A check added only to
`select_research` would leave the menu offering a row it then refuses.

The condition is "the `(x, y)`-first `studies` structure has a body in its
pen". For a subject-gated node this subsumes the existing "is a Research
Node standing" test, since a station with a pen is a station.

`research_block_memo` memoises per item per screen pass (measured 3.7 ms →
0.6 ms on a 515-entity base); the subject test is per *node*, not per item,
so it goes outside that memo rather than into it.

### Blocked, not hidden

**The subject-gated nodes are not concealed**, and this is the one place the
design departs from the brainstorming ask, which was for progressive
disclosure in the routine tree's style.

The routine tree hides a rung until its family is *discovered*, and that
works because there is something to discover — a routine something in the
field is carrying. A subject-gated node has no such thing: any program
satisfies any study, so there is nothing per-node to find. What is left of
"disclosure" would be hiding a node until its prerequisite is done, and the
Base tree lists everything unconditionally on purpose — the graph screen's
whole value is showing the path before you can walk it, and
`ResearchStatus::Locked` already exists to say "not yet, and here is why".
Hiding nineteen mid-game nodes would spend that to buy mystery about a tree
the player is trying to plan against.

So the disclosure a player gets is the block reason on the row, filled by
`research_block` and therefore identical to the sentence a refusal would
hand them. If real concealment is wanted later it is a change to how the
Base tree reads, with its own argument, and not a free addition to this one.

### Pin first, then select

There is deliberately **no "a project is selected but has no subject"
state.** A subject-gated project cannot be selected until a subject is
pinned, which means `MachineStatus` gains nothing and its four reachable
values stay `Idle` / `Unstaffed` / `Stranded` / `Running`.

That is the reasoning the seam already recorded when it rejected a
`MachineStatus::NoProject`: a missing precondition should make the want
unraisable rather than mint a new status that every census has to learn and
that a second writer could then contradict.

### Losing the station under a running project

Both structure-destruction doors have to release the subject back to `Staff`
and abandon the project: `damage_structure`'s destroyed branch and
`remove_structure`. This is `clear_pending_build_at`'s rule with a second
subject — the door left out silently strands a program in a role nothing can
get it out of, and **nothing fails to compile.**

Abandoning goes through the existing `Game::abandon_research`, so
`withdraw_research_orders` runs and the material orders leave the queue.

## 4. Completion

### The subject is a third term in one `&&`

`Game::settle_research` (`game/unlocks.rs:1004`) gates today on
`progress >= def.cost && spend_bill_from_base(...)`. The subject joins that
same expression.

The short-circuit is the load-bearing half. `a_full_bill_alone_does_not
_complete_a_project` exists because spending the materials on a
half-researched project was the failure; consuming the *subject* on one
would be the same failure with a worse loss, since a program is not
refundable and a material still on a shelf is.

### The conversion

`downed_program_for_with_overkill(subject, 0.0)` → `push_downed_program` →
despawn. Both existing doors, no new conversion function.

This works unchanged because `downed_program_for_with_overkill`
(`game/combat_rewards.rs:639`) reads only generic components — `Creature`,
`is_boss_creature`, `rarity_of`, `ability_user_level` and the live
`Routines` — every one of which a tamed program has. Two properties fall out
of it and both are wanted:

- **`ability_user_level` reads the subject's real `Experience`**, where a
  wild kill has none and falls back to `ZoneLevel`. A studied companion's
  record states the level it actually reached.
- **`0.0` is `overkill_term`'s own identity value**, so the `condition` roll
  is best-case. A controlled dissection yields better material than
  overkilling something in the field, which is the right way round and is
  free.
- **`carried_routine` reads the live `Routines` minus the species kit**, so
  a routine the player installed on that program from a disk comes back out
  in its record. Study feeds `Game::extract_program`, and the loop closes.

### A full store blocks completion

`push_downed_program` **refuses** at `tuning::MAX_DOWNED_PROGRAMS` and logs
rather than discarding. That refusal has to block completion and be reported
the way a material shortfall is through
`Game::research_material_shortfall`.

A study that quietly ate the body *and* the knowledge because a store was
full is the failure to head off here, and it is the one the naive ordering
produces: consume, then push, then discover the push failed.

## 5. Fusion, and the capability unlock

### The field

`ResearchDef` gains **`unlocks_fusion: bool`**, `#[serde(default)]`. It is
`opens_routine_tree`'s exact shape, **including its lenient rule: if no
loaded node carries the flag, fusion is open from the start.** A mod that
deletes or replaces the research tree is not stranded without a capability
the base game had.

This keeps the capability's *owner* in data while Rust names only the
capability, which is the split `perks.rs` already documents: a perk's
catalogue is data, its effect is a named query in Rust because there is no
shared shape to express as data. `ResearchDef` deliberately gets a bool per
capability rather than an `unlocks_actions: Vec<String>` registry, for the
same reason `PerkDef` has no `effect` field.

### The node

The flag goes on **`program_refactoring`** — sector 2, cost 75, requires
`automation`, unlocks the Annealing Node, the Refactor Bench and the
Component Stripper. Its shipped description is already:

> Upgrade a tamed program instead of replacing it, or take one apart for
> useful parts instead of scrap.

It is the node about surgery on tamed programs. This is the node the feature
was waiting for, not a new one invented to hold a flag.

### What it costs elsewhere

`Game::fuse_companions` (`game/party.rs:1169`) gains one refusal, placed
with its existing ones, all of which land before anything is consumed. The
`Locality::Anywhere` menu row in `group_menu.rs:303` gains one term to its
`available:` closure beside the existing `owned_pets().len() >= 2`.

Fusion keeps `Locality::Anywhere` and needs no structure standing — the
research alone. **Existing saves lose fusion until they research it**, with
no grandfathering, which is the decision taken.

## 6. Drawing

### The station's floor

The three non-anchor footprint cells take a dark yellow/brown fill, and
**draw no outline.** That is not cosmetic: `marks::outline_open`
(`crates/gui/src/render/marks.rs:451`) already owns the tile-edge ring,
drawing a machine's walls as 2px lines flush at the tile's edges, and the
existing corner marks carry insets *specifically* so nothing reads as
painting one of those absent lines back in. The floor cells keeping the ring
clear is what leaves it available for the next bullet.

Drawn only on cells the station actually owns — which, because the footprint
is derived per read, excludes a legacy Node's cells that turned out to hold
something else.

### The pin mark

**Four red L-brackets on the tile-edge ring**, clear of `RARITY_BAR_PX` at
the top and `PROGRESS_BAR_PX` at the bottom, drawn `* vig` like every other
mark so an edge-of-light tile does not leave them burning. Geometry is a
free function in `marks.rs` returning rects, matching every mark in that
file, so it is unit-testable without a `Painter`.

**The top-left bracket yields to the Alt `?` marker.** That marker already
"borrows the top-left" while `reveal` is held, through `base.rs`'s
`corner_marker` gate. This extends one existing rule rather than inventing
an arbitration.

### The corner census is a proof, not a suppression

All four tile corners are spoken for — top-left is the con earmark (and the
Alt marker), top-right the nemesis mark, bottom-left the "someone is on
this job" mark, bottom-right the patrol mark — and `marks.rs:112-118` and
`:186-195` document that census as load-bearing.

The brackets do not collide with any of them, because they are on the ring
and the corner marks are inset off it. But the stronger statement is worth
asserting: **a body under study can wear none of those four marks.**

- The con earmark: `ConRead::of` answers `None` for anything non-hostile,
  and a pinned program is the player's own.
- The nemesis mark and the patrol mark: both wild-only.
- The staffed mark: `wears_job_mark` is about a posted program, and
  `UnderStudy` is not `Staff`, so a pinned program is never posted.

A census test asserts all four. They are unreachable today, so the test is a
proof — and it fails loudly the day somebody makes con reads apply to owned
programs, which is exactly when the ring would need re-examining.

## 7. Saves

`CreatureSave` gains **`study_station: Option<(i32, i32)>`** behind
`#[serde(default)]`.

- **The station's tile, not its `Entity`.** Entity ids are not stable across
  a save — `party_slot`'s precedent, and `SortieSave` carries no member list
  for the same reason.
- **Resolved after structures restore**, through the `pending_cronjobs`
  deferral. This is the town-patrol tether's precedent exactly: it saves by
  the town's tile and is resolved after `restore_settlements`.
- A tile that resolves to no `studies` structure on load leaves the program
  as ordinary `Staff`. That is the same lenient direction `ResearchDb`'s
  loader takes with an unresolvable id, and it is what keeps a save loadable
  after a modder deletes the station.

**Additive behind a default, so no `SAVE_FORMAT_VERSION` bump** — the rule
`PlayerSave::bought_stats` and `tutorial_seeded` already state.

It needs a **save→load test and not only a RON round trip**: a round trip
cannot catch a field that is not really persisting, which is why
`WorkOrder::for_research` carries `#[serde(default)]` rather than
`#[serde(skip)]` and is asserted the same way.

## 8. Moddability and docs

Per the schema rule, in the same change:

- `assets/structures/README.md` documents `footprint` and `studies`,
  including that the anchor is the top-left cell and the impassable one,
  that the other cells are the structure's own walkable floor, and that the
  pen is the cell diagonally opposite the anchor.
- `assets/research/README.md` documents `requires_subject` and
  `unlocks_fusion`, including the lenient rule that no node carrying
  `unlocks_fusion` means fusion is open from the start.
- `CHANGELOG.md` gets its section at the merge, not on the branch — the
  version bump happens once, at the merge, so a rebase cannot invalidate a
  tag.
- `docs/manual.md` and the root `README.md` are both carved out of the doc
  obligation and stay stale.

Nothing in this feature hardcodes content in Rust. The footprint, which
structure studies, which nodes need a subject and which node unlocks fusion
are all authored. Rust names the *capability* (fusion) and the *geometry
rule* (top-left anchor, diagonal pen), neither of which is content.

## 9. Testing

Engine fixtures live in `crates/engine/src/tests/support.rs` — look there
before writing a new one. `spawn_structure_at` bare-spawns a `Structure`
with no `PowerFuel` and is for what a standing structure *enables*, not for
the build rules, so a footprint test that needs the placement refusals has
to go through `place_structure`.

**The footprint fixtures are the ones that matter most**, because the squad
seam's failure was that every fixture was hand-built at footprint 1 and so
every anchor-measuring reader stayed green. So:

- A station placed with an occupied cell in its footprint is refused — one
  case per cell, and one case per blocker kind (structure, build site, dig
  mark, rock).
- A hauler's walk refuses to cross the anchor and **does** cross the floor
  cells.
- `station_candidates` offers faces of the whole footprint.
- `has_station` still answers for a marked cell whose only face holds the
  digger standing at it — the dig-crew deadlock the narrower set exists to
  avoid.
- A legacy 1x1 Node loaded from a save whose neighbours are occupied stands,
  blocks only its anchor, and refuses the pin with the pen's own sentence.

**Role and walk:**

- A pinned program is offered no job by `schedule_base_labour`, does not
  wander off the pen, is not recalled by a party add, is not a fusion
  candidate, and is not healed by a rest — one test each, because these are
  the omissions the fifth `ProgramRole` variant buys.
- A pinned program walks from where it was standing to the pen and stops.
- Two programs cannot share the pen.

**Research:**

- `select_research` refuses every subject-gated node with nobody pinned,
  and the refusal's sentence equals `research_block`'s `blocked_by` for the
  same node — asserted by comparing against a live call, not hardcoded
  prose, which is the drift this repo keeps recording.
- **A reachability test**, not only refusal tests: pinning a program makes
  the same selection succeed. The seam records that a refusal can ship
  *permanent* with every refusal test still green.
- Each of the eight ungated nodes is selectable with nobody pinned.
- Unpinning is refused while a subject-gated project is active.
- Demolishing the station releases the subject and abandons the project —
  once per destruction door.

**Completion:**

- Full progress with a full material bill and no subject does not complete.
- Full progress with a subject completes, the subject leaves the world, and
  exactly one `DownedProgram` appears whose species and level match it.
- A routine installed from a disk comes back as `carried`.
- A full `DownedPrograms` store blocks completion, reports, and leaves both
  the subject and the progress intact.

**Censuses in `tests/assets.rs`:**

- Every shipped node with `min_zone >= 2` declares `requires_subject`, and
  none of the eight ungated ones does.
- No subject-gated node is a prerequisite of a node that is not.
- Exactly one shipped node carries `unlocks_fusion`.
- `research_materials_are_reachable_from_their_own_prerequisites` still
  holds unchanged.
- Exactly one shipped structure declares `studies`, and every structure
  declaring `studies` declares `footprint >= 2`.

**Rendering:** the bracket geometry clears the rarity bar and the progress
bar; the top-left bracket is absent while the Alt marker draws; the four
corner marks are unreachable for a body under study.

**Gates:** `cargo fmt`, `cargo clippy --workspace --all-targets`
(`--all-targets` is load-bearing — a bare run leaves every test module
unlinted), and `cargo test --workspace` as the final gate. Also
`cargo test -p feral-processes-engine balance_sim`, which will report
nothing, for the reason in Risks.

## 10. Phasing

Each phase ends green and commits.

1. **Footprint.** The two `StructureDef` fields, `Game::study_pen`, the
   placement refusal, and every reader in §1's table. `research_node.ron`
   goes to `footprint: 2, studies: true`. No study mechanics yet — this
   phase is purely "a structure can be 2x2 and one cell of it blocks".
   The `design-patterns` dialog on `hauling::blocked_tiles`' signature
   happens here, before the code.
2. **The role and the walk.** `ProgramRole::UnderStudy`,
   `components::UnderStudy`, `party::walks_the_base`, the `drift_idle_staff`
   arm, `pin_subject` / `unpin_subject` and their refusals, and the save
   field. Still no research gate — a program can be pinned and it does
   nothing.
3. **The gate and completion.** `requires_subject` on the def and on the 19
   files, the `research_block` reason, the third `&&` term in
   `settle_research`, the conversion, the full-store stall, and the release
   on both destruction doors.
4. **Fusion.** `unlocks_fusion`, the flag on `program_refactoring`, the
   refusal in `fuse_companions`, the menu term.
5. **Drawing.** The floor fill, the brackets, the Alt yield, the census.
6. **UI.** The menu row, `Mode::PinSubject`, `ALL_MODES`, the draw match,
   `needs_status_banner`.
7. **Docs and censuses.** The two READMEs and the `tests/assets.rs` rows.

Phases 1 and 2 are independently useful and independently reviewable, and
phase 1 is the one carrying the risk. A `dev-saves/` template capturing a
base with a Station, a pinned subject and an active subject-gated project
is worth capturing at the end of phase 3 — testing this by hand otherwise
starts with an hour of play, and that cost is what makes features ship
unplaytested.

## Risks and what the tests cannot see

**Nobody can play this.** `cargo run` needs a display and there is none in
an agent session, so every claim in this spec is a claim about source and
tests. A green suite is not evidence of play. The things a test can confirm
the geometry of while they still look wrong are the dark yellow/brown floor
against the rock palette, the brackets' weight against the rarity bar, and
whether a 2x2 building reads as one thing or as a building next to some
floor.

**Balance moves and `balance_sim` will report nothing.** Every sector-2+
node just got strictly more expensive by one program, and `balance_sim`
models no research at all — no active project, no Research Data, no
materials bill. So the regression gate is silent here by construction, and
whether `cost` and `materials` should come down to pay for the subject is a
question for after the feature has been played. This spec changes no tuning
value.

**Nineteen bodies is an unmeasured price.** Reaching sector 6 now means
pinning and spending nineteen programs. Captures, gifts, adoptions and
Stack orphans all supply them and the roster cap is 200, so there is no
deadlock — `decompile` is welded into the player's slot 0 and is always
available, so a capture is always reachable. But whether the *pace* is right
is exactly the thing only play will say.

**The corner-mark proof is a proof about today.** It holds because con reads
are hostile-only and the staffed mark is posted-only. Both are properties of
other subsystems, and the census test is what turns a coincidence into a
constraint somebody will notice breaking.

**A cramped legacy footprint is a state nobody will think about again.** A
Research Node in an old save whose neighbours are occupied will stand
forever with a pen it cannot use, and the only surface saying so is a pin
refusal the player sees only when they try. That is the deliberate price of
taking "same structure id" and no migration; the alternative was a
migration that could break a base the player built.
