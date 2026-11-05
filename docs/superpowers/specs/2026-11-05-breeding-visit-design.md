# Breeding visit and zone-2 gate — design

**Date:** 2026-11-05

## Intent

Breeding today is a menu action with no presence in the base. You confirm a
pair, a seed vanishes, and the child starts incubating in the bay at once.
Nothing happens on the floor, and the bay can be built from turn one. The user
wants two things:

1. **Breeding comes later in a run.** It sits behind a zone-2 research.
2. **Breeding is something the parents do.** After you pick them, both walk
   over to the Breeding Bay, spend a short while there, and then go back to
   work. Each one comes away with a happy memory of the other.

The walk is what makes the child: the child exists only once the visit
finishes.

## Goals

- A new research node with `min_zone: 2` and `requires_subject: true` that
  unlocks `breeding_bay`. The bay leaves the turn-one build list.
- Confirming a breeding spends the seed and reserves a bay slot. It then sends
  both parents to the bay.
- Both parents walk to the bay over the base floor, the same way staff walk to
  any post. When both have arrived, they pause together for a fixed time.
- When the pause ends:
  - the child's incubation is written into the reserved slot, and the normal
    `INCUBATION_TICKS` timer starts from that moment;
  - both parents gain the new memory of each other;
  - both parents return to the labour they had before, or whatever labour
    scheduling now gives them;
  - the existing `BreedReadyAt` cooldown starts.
- A visit in progress survives save and load.
- The copy matches the mechanics (see Copy below).

## Non-goals

- Staff labour or standing jobs for breeding. The player still starts every
  breeding from the menu.
- Any change to inheritance, hybrids, roll caps, incubation length, or seed
  drops.
- The Holding Cell warden bug. It gets its own session after this lands.

## Anti-goals

These would make the feature a failure even if it technically works:

- Parents that freeze, teleport, or get lost and never return to work.
- A seed spent with no child and no refund.
- A slot left reserved forever, so the bay looks full while nothing is
  incubating.
- A walk that only exists in tests. It must happen through the live game
  tick, and at least one test must drive it through `Game::tick`.
- Taking away a bay the player already built in an existing save.

## Decided by the user

- **Parents must be at the base.** A program that is not at the base, such as
  one in the player's party in a zone or one out on a sortie, cannot be picked
  as a parent. Its row is greyed with a reason in the same style as the other
  `BreedRefusal`s ("away from base" or similar plain words). Both the engine
  `breed_refusal` and the picker enforce this.
- **The child starts after the visit,** not when you confirm. The seed is
  spent when you confirm.
- **The memory is about the other parent.** Each parent forms a memory whose
  subject is the other program, like `bonded_in_battle` (`subject: Program`).
- **The gate is a new research node** with `min_zone: 2` and
  `requires_subject: true`, following the README rule that zone-2+ nodes need
  a study subject.
- **The node is hidden until discovered, after zone 2:** `discoverable: true`.
  Study rolls already consider only nodes inside the zone you have reached, so
  it can only be found once you are in zone 2.
  - Add it to the hand-written list in
    `exactly_the_named_research_nodes_are_discoverable`
    (`crates/engine/src/tests/assets/structures_research.rs`).
  - It must also pass
    `every_discoverable_research_node_is_reachable_from_the_visible_tree`.
- **Needs hold during a visit.** From confirm until the visit ends or is
  cancelled, a parent's needs do not drain. While a parent is on a visit:
  - a need never pulls it away;
  - it never goes `OffShift`;
  - it never writes `frayed_here` or `ran_down`.

  Read the needs rules in `seams-base.md` first. Hold the drain at its one
  source; don't add a check at each reader.

## Calls made by Claude (open to override)

- **Existing saves.** A bay already standing keeps working, and so does
  breeding into it, even before the new research is done. The gate only
  controls whether a *new* bay can be built. This follows the standing rule
  that we never rewrite a standing structure.
- **Interruptions cancel the visit.** If either parent stops being able to
  finish the visit, the visit is called off:
  - the seed goes back to the player's pack;
  - the reserved slot is freed;
  - no memory forms and no cooldown starts;
  - one base-log line says who and why.

  "Stops being able to finish" covers:
  - a parent leaves the base (joins the party, goes on a sortie);
  - a parent is downed or removed (released, fused, jailed, or killed in a
    raid);
  - the bay is demolished or destroyed.

  A battle starting elsewhere does not cancel the visit. It only pauses the
  visit, the same way base labour already pauses.
- **The pause lasts `BREEDING_VISIT_PAUSE_TICKS` = 60 ticks (30 seconds).**
  The constant lives in `tuning/sorties.rs` beside the other breeding
  constants. The builder may replace it with an existing break or idle
  duration if there is one of about that length.
- **Breeding from the base report.** The report breeds into the first bay
  with a free slot, as it does now. A reserved slot counts as taken.
- **The memory.** A new `assets/memories/bred_with.ron`, modelled on
  `bonded_in_battle`:
  - `subject: Program`;
  - name "Raised a child with";
  - blurb in plain language, along the lines of "We made something together.";
  - positive valence about as strong as `bonded_in_battle`'s.

  Exact numbers, `spreads_as` and `known_for` are the builder's call. Copy the
  `bonded_in_battle` pattern unless a census test or the memories README says
  otherwise.
- **The research node.**
  - Id `bloodlines`, name "Bloodlines".
  - `requires: []`, and no prerequisite gated above zone 2.
  - Cost and materials picked from peer zone-2 nodes, so it sits in their
    range.
  - Description says what it is in plain words, with no "zone" or tick
    vocabulary.
- **The rest flag.** A parent on a visit is not idle and not available to
  labour scheduling. The builder decides whether this uses a new `Task`/state
  or an existing mechanism. Either way it is plain Rust state owned in one
  place, so it can be saved, and any per-tick decision follows the
  gather/decide/apply rule in CLAUDE.md.
- **Demolition during a visit.** A reserved slot does not count as occupied
  for `remove_structure`'s "a program is incubating" refusal. Demolishing the
  bay mid-visit cancels the visit through the refund path above. A destroyed
  bay works the same way.
- **The child matches the confirm-time preview.** At confirm, store what the
  child roll needs in the visit state: both parents' species, rolls and
  generation. Draw the child from that snapshot at the end of the visit. The
  snapshot is saved with the visit.
- **Visiting refuses double-booking.** A parent already on a visit refuses
  with its own reason, such as "already with the bay", and cannot be picked
  for a second pairing.

## Copy

Update all of these in the same change:

- `assets/help/46-breeding.md`:
  - the research gate, which is discovered by study once you reach zone 2;
  - the base-only rule;
  - the walk and pause;
  - the child starting after the visit;
  - the memory;
  - the cancellation rule, including the refund;
  - the seed must be **in your pack**. Base storage doesn't count, and this is
    already true but undocumented.
  - Also fix the "every generation raises the ceiling a roll can reach" line:
    rolls stop rising at a cap.
- The `breeding_bay.ron` description. Also the `breeding_seed.ron`
  description, if it repeats anything that changed.
- The new research node's description.
- `crates/gui/src/render/breeding.rs`:
  - the second-parent popup says "Esc back", not "Esc cancel", because Esc
    steps back to the first parent;
  - the confirm page tells the player the parents will walk over first.
- The confirm log line. It currently says the child "starts to incubate"
  immediately. Split it into a line for the visit starting and a line for the
  child starting at the end.
- `CHANGELOG.md` entry at release time.
- `assets/research/README.md` and `assets/memories/README.md`, if they list
  nodes or memories.
- `rg -i 'breed' assets/` for any other line that says the bay is available
  from the start or that breeding is instant.

Not in scope: the "Any key back" hint on every structure's Info page in the
`[c]` hub. It isn't specific to breeding, so it gets noted for later.

## Constraints

- Save format: a visit in progress is new persisted state, and a reserved
  slot may be too.
  - Add `#[serde(default)]` so old saves load.
  - Follow `CHANGELOG.md`'s preamble on whether this is a breaking bump. It
    should not be, because old saves have no visits.
  - Add a save→load test, not only a RON round trip (see memory
    `ron-round-trip-cannot-catch-a-skipped-field`).
- The RNG draw order inside `breed` moves. The child's species and roll draws
  now happen at the end of the visit, not on confirm. Draws must still not
  depend on query order: sort by a stable id. Then check
  `cargo test -p feral-processes-engine balance_sim` and the breeding tests.
  The breeding tests may need their expectations re-derived.
- Existing tests to update:
  - `crates/engine/src/tests/research/gating.rs:201-216` asserts the bay is
    buildable on a fresh game. Flip it to assert the gate.
  - Existing breeding tests that expect an instant incubation will need to
    run the visit to its end.
- Read `.claude/rules/seams-base.md`, `seams-species.md` and
  `seams-memories.md` (and the `seams` skill for the matching traps) before
  touching labour scheduling, species, or memories.
- `breed` lives in `crates/engine/src/game/breeding.rs`. Hatching runs from
  `turn.rs:347` via `hatch_incubations`. The visit's per-tick step belongs in
  the same tick region, and should be paused under the same `Busy` conditions
  as hatching.

## Left to the builder

- How the parents path to the bay and stand there: reuse the existing
  staff-movement machinery, without a parallel walker.
- Which tile each parent stands on: adjacent to the bay, the way a post
  works.
- What "at the base" means, using an existing predicate. If two candidates
  disagree, prefer the one labour scheduling already uses.
- Whether the reserved slot is a new `Incubation`-like enum variant or a
  separate reservation.
