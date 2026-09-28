---
paths:
  - "**/memories*"
  - "**/needs*"
  - "**/disposition*"
  - "assets/memories/**"
---

# Load-bearing seams: What a program remembers

One sentence each, the rule alone. The trap is in the `seams` skill; the argument is `seam:<slug>` in the memory graph.

- **`Game::remember` is the one door a memory is written through**, and the
  four triggers are callers of it, not writers beside it.
- **Intensity is derived from `GameClock` on every read, never stored or
  ticked.** The decay is a magnitude scale and **never a sign flip**, or a
  grudge would decay into a fondness.
- **An empty catalogue is a supported install, and the property is held at
  both ends.** `remember` resolves the def *before* touching the store, and
  every reader skips what it cannot resolve — so deleting `assets/memories/`
  restores the pre-memory game rather than breaking one.
- **`MemorySubject::BaseTile` names the space, and that is why it is not
  called `Place`.** `note_strandings` writes the worker's **own `Position`**
  — base space for a posted program — and not its post, because a memory
  keyed to the machine's tile could never be read by `drift_idle_staff`.
- **The first of two hooks is `drift_idle_staff`'s last rejection, and it is
  not a score.** `opinion_of(worker, BaseTile) <
  MEMORY_AVOIDANCE_THRESHOLD`, beside the four tiles it already declines — a
  rejected candidate leaves the body standing where it was, so this opens no
  failure mode and needs no fallback.
- **The second hook is morale, and it is one addend in one formula.**
  `CycleModifiers::morale` into `systems::mining_success_chance`, priced and
  capped by `morale_shift`.
- **`memories::sum_intensity` is the fold, and `Game::morale` is a caller.**
  `party::role_of`'s reason: `task_progress_system` has no `Game` to ask,
  and two folds would eventually disagree about whether an unresolvable def
  counts — which is the property the whole empty-catalogue guarantee rests
  on.
- **A work memory is either an edge or a stretch, and `Game::note_postings`
  is the stretch half.**
- **A `Structure` memory names the kind, not the entity**, which is what
  lets it be written on the branch about to despawn the machine and what
  makes a rebuilt Lathe the same Lathe.
- **`MEMORY_TRIGGERS` in `tests/assets.rs` is the pairing census**, and a
  def shipped without a row in it fails the build.
- **The memories page is one derivation and has no scroll.** `R` from the
  roster — not `M`, which has been the manifest since long before this — and
  every figure comes from `Game::memory_report` / `Game::morale`.
- **`mood` splits one store into two reads, and `evict` reads neither.**
- **Slots are a grudge, not a door: `Game::roster_room` against
  `ROSTER_HARD_CAP` is the one refusal into the roster, and `pet_capacity`
  only decides who earns `unslotted`.**
