# What a program remembers

- **`Game::remember` is the one door a memory is written through**, and the
  four triggers are callers of it, not writers beside it. **A `who` with no
  `Memories` is a no-op** — the store is minted at `roster_parts` and nowhere
  else, so its absence *is* "not on the roster", which keeps hostiles,
  structures and the player safe without a branch at any call site. The
  player is neither a bond's holder nor its subject, and that falls out of
  `ProgramId` never being minted for them rather than out of a `Player`
  check. It draws **no RNG** and writes **no log line**: the screen is the
  surface. Refusals are *returned* (`NoStore`/`UnknownDef`/`WrongSubject`),
  since the engine has no runtime warning channel.
- **Intensity is derived from `GameClock` on every read, never stored or
  ticked.** The decay is a magnitude scale and **never a sign flip**, or a
  grudge would decay into a fondness. Eviction is lazy, lives only at the
  tail of `remember`, and compares **magnitude at both** steps: a signed
  comparison evicts every grudge and keeps every fondness, since the deepest
  scar is the smallest number in the store. An entry naming a def no file
  defines is **kept** but unscoreable, so it is the first thing the cap drops.
- **An empty catalogue is a supported install, and the property is held at
  both ends.** `remember` resolves the def *before* touching the store, and
  every reader skips what it cannot resolve — so deleting `assets/memories/`
  restores the pre-memory game rather than breaking one. Never gate a trigger
  or the page on the database being non-empty: that makes the property hold
  by accident at one site and lapse at another.
- **`MemorySubject::BaseTile` names the space, and that is why it is not
  called `Place`.** `note_strandings` writes the worker's **own `Position`**
  — base space for a posted program — and not its post, because a memory
  keyed to the machine's tile could never be read by `drift_idle_staff`. A
  *surface* variant would be zone-local and would have to be wiped by name in
  `enter_next_zone`; a base tile travels with the base.
- **The first of two hooks is `drift_idle_staff`'s last rejection, and it
  is not a score.** `opinion_of(worker, BaseTile) < MEMORY_AVOIDANCE_THRESHOLD`,
  beside the four tiles it already declines — a rejected candidate leaves the
  body standing where it was, so this opens no failure mode and needs no
  fallback. **Not `schedule_base_labour`**: that decides the whole assignment
  by priority and then diffs it, with no sort and no score, and a memory term
  there would sit in the path of the anti-thrash and
  never-free-a-`Carrying`-holder rules. `opinion_of` and never `morale` — the
  sum over everything keeps a program off *every* tile at once — and the
  comparison is **signed**, so a fondness can never trigger an avoidance.
  **Two ways this family hollows out.** A drift offers a different neighbour
  every beat, so a test that fades the grudge by advancing the clock measures
  a tile the memory was never about — pick the tile at the *far* end of the
  fade, implant against it first, and **wind** the clock rather than ticking.
  And a fixture must stand its bodies clear of the Home and well inside the
  starting pocket, or half the candidates are refused by the *floor* rule.
- **The second hook is morale, and it is one addend in one formula.**
  `CycleModifiers::morale` into `systems::mining_success_chance`, priced and
  capped by `morale_shift`. **Signed around a baseline of zero**, which is
  what buys three properties without a branch: a program with no memories,
  the player working a node themselves, and a deleted `assets/memories/` all
  contribute exactly nothing. The trap is that the term needs **its own cap**,
  `MEMORY_MORALE_MAX_SHIFT`, and the outer `clamp(0.0, 1.0)` is not it — that
  one exists because `GameRng::random_bool` panics, and it would silently
  swallow an uncapped overshoot where a test reading the finished chance
  could not see it. `morale` here and **not** `opinion_of`, the mirror of the
  parking hook's choice: this is a claim about the body, not about the
  machine. It reaches **extraction only**, and `balance_sim` models no base
  production at all, so nothing gates it numerically.
- **`memories::sum_intensity` is the fold, and `Game::morale` is a caller.**
  `party::role_of`'s reason: `task_progress_system` has no `Game` to ask, and
  two folds would eventually disagree about whether an unresolvable def
  counts — which is the property the whole empty-catalogue guarantee rests on.
- **A work memory is either an edge or a stretch, and `Game::note_postings`
  is the stretch half.** It runs beside `note_strandings`, after the schedule
  and before the clock moves, but **on a period rather than on an edge**: a
  stranding has `Stranded::since` to read, a posting has nothing telling the
  first tick at a machine from the thousandth. A per-tick write saturates
  `strike_cap` in three ticks and makes `strikes` mean nothing, and it makes
  `remember`'s tail eviction eager for working programs and lazy for idle
  ones. **Not on a completed cycle** either, which would make `strikes` a
  cycle count and the same service mean different things at a fast machine
  and a slow one. `swept_here` is the one edge, in `damage_structure`, on
  **both** branches — the kind and the worker list are hoisted above the
  branch, since the destroyed side despawns the structure the kind is read
  off.
- **A `Structure` memory names the kind, not the entity**, which is what lets
  it be written on the branch about to despawn the machine and what makes a
  rebuilt Lathe the same Lathe. `settled_in` and `jammed_here` share that
  subject and oppose in sign. A digger gets the `Activity` memory and **no**
  `Structure` one — a `DigSite` is the one `Task` target that is not a
  structure, so the arm skips structurally rather than by a check.
- **`MEMORY_TRIGGERS` in `tests/assets.rs` is the pairing census**, and a def
  shipped without a row in it fails the build. There is no `trigger` field to
  derive it from: the catalogue is data and the triggers are Rust.
- **The memories page is one derivation and has no scroll.** `R` from the
  roster — not `M`, which has been the manifest since long before this — and
  every figure comes from `Game::memory_report` / `Game::morale`. The report
  is `&self` and **evicts nothing**: a screen that rewrote the roster it draws
  would make what a program remembers depend on whether anyone looked. Rows
  sort by magnitude; the blurb is said once per kind; **age is in words,
  banded against the def's own half-life**, because nothing in the game has
  ever shown the player a tick. Two censuses hold it, so
  `MEMORY_CAP_PER_PROGRAM` is a **layout** constraint first — raising it past
  what fits means giving the page a scroll.
