# Situational thoughts

**Status:** design. Sub-project F of `2026-09-16-base-social-roadmap.md`.
The roadmap's shared decisions hold: nothing draws `GameRng`, every derived
value stays derived, an empty catalogue is a supported install, and there is
**no save change and no `SAVE_FORMAT_VERSION` bump**.

A–D are built. E (conversations) and G (sulking behaviours) are out of
scope; G's "slower beside a rival" is this sub-project's `BesideRival` term
reaching the existing morale addend, and needs nothing further from F.

## 1. Decisions and why

- **Assess once, carry the answer** (approach A). One system derives each
  staff program's situation and writes it to an unsaved `Situation`
  component; every morale reader folds that component. Rejected: a pure
  function over an inputs struct that `Game` and the bevy system each fill
  (moves the "two folds disagree" failure into input-gathering), and moving
  morale out of `task_progress_system` into a `Game` pass (restructures the
  gather loop for one feature). Precedent: `Stranded` (`components.rs`), a
  per-tick cache that is not saved because the walk that produced it runs
  again next tick.
- **Five terms, two of them positive.** A situation that can only subtract
  reads as a tax on building a base; a friend beside you and a machine that
  runs are the other half.
- **Thoughts reach morale and the Grievance ladder, capped.** The situational
  total is clamped to `±SITUATION_MAX_TOTAL` (5.0) before it joins the memory
  total. Against `MORALE_SULKS_AT` (−8), situation alone cannot start a sulk,
  but it tips a program already soured by real memories. Without the cap,
  base layout and power budget become Grievance triggers by themselves, and
  a sulker that refuses its post changes its own situation, recovers at
  `MORALE_RECOVERED_AT`, is re-posted and sulks again. The cap is a starting
  value to revisit after play, not a balanced one.
- **Thoughts never touch opinion.** `opinion_of`, `Game::bond` and every
  bond band read memories only. Otherwise "beside a rival" would deepen the
  rivalry that caused it, a loop with no event behind it.
- **"Beside" is an adjacent tile**: Chebyshev distance ≤ 1, excluding the
  program itself — the same adjacency `drift_idle_staff` scans.
- **Half data, like perks.** Wording and weight are a file each in
  `assets/thoughts/`; what fires a thought is a closed Rust enum. A modder
  can reword, reweight or delete a thought, not invent a trigger.

## 2. Data: `assets/thoughts/`

```rust
pub enum Trigger { BesideRival, BesideFriend, Unpowered, MachineRunning, NoAmenity }

pub struct ThoughtDef {
    pub trigger: Trigger,
    pub name: String,
    pub blurb: String,
    pub intensity: f32,   // signed; positive lifts morale
}
```

- `ThoughtDb::load_dir` follows the `*Db::load_dir` pattern: a malformed
  file is skipped with a logged warning. Two defs with the same trigger:
  the first by file name wins and the second is warned about, so a trigger
  never counts twice.
- A trigger with no def never fires. The empty catalogue gives every
  program a situational total of 0 and no rows.
- No `mood` field: `mood` splits a memory between opinion and morale, and a
  thought has only the morale read.
- Five shipped files, one per trigger. Starting intensities: rival −3,
  friend +2, unpowered −2, running +1, no amenity −2.
- `assets/thoughts/README.md` documents the schema and the closed trigger
  list.

## 3. Engine

**`crates/engine/src/situations.rs`** (new module):

- `Situation { thoughts: Vec<Trigger> }` — a `Component`, not
  saved. Holds triggers, not def ids, so a reworded asset needs no
  migration and the fold resolves through `ThoughtDb` on read.
- `assess_situation_system` — for each staff program in base space:
  - `BesideRival` / `BesideFriend`: any other staff program within
    Chebyshev 1 whose bond band (from the holder's view) `avoids()` /
    is Friend or Close. The band is `bonds::band` of the memory fold with a
    subject filter — the same `sum_intensity` call `Game::opinion_of` makes,
    so the system and `Game::bond` cannot disagree.
  - `Unpowered`: posted (`Task.target`) to a machine where
    `PowerGrid::is_dark` holds.
  - `MachineRunning`: posted to a machine whose `MachineStatus` is
    `Running`. `MachineStatus` is written later in the chain, so this reads
    the previous tick's status — the same one-tick lag `Stranded` accepts.
  - `NoAmenity`: `offshift::Amenities::build` over the base's structures
    finds nothing.
- `pub(crate) fn sum(situation, db, felt_as) -> f32` — each resolved
  trigger's intensity through `Disposition::felt`, summed, clamped to
  `±SITUATION_MAX_TOTAL`. A free function for `sum_intensity`'s reason.
- `pub(crate) fn scaled_rows(situation, db, felt_as) -> Vec<(…, f32)>` —
  each thought's share after the clamp (proportional scale), so rows on the
  memories page sum to the clamped total. `sum` is defined as the sum of
  `scaled_rows`, so the page and the figure are one derivation.

**Schedule** (`game/lifecycle.rs` `build_schedule`): the system goes
between `idle_machine_system` and `task_progress_system`. It is inserted,
not a reorder; the chain order stays load-bearing for the clog/pickup
handoff and is otherwise untouched. It needs `power_grid_system` to have
run, which it has.

**Morale** becomes memory fold + situational sum in exactly two places, both
reading the same `Situation`:

- `Game::morale` (`game/memories.rs`).
- `CycleModifiers::morale` in `task_progress_system` (the `CronjobWorker`
  query gains `Option<&Situation>`).

`morale_shift`'s ±`MEMORY_MORALE_MAX_SHIFT` still caps the total.
`player_gather_system` keeps `morale: 0.0`.

**Before the first tick.** `Game::load` and new-game construction run the
assessment once, through the same function the system calls, so `Game::morale` agrees with the
next tick's fold from the moment a save opens. A missing `Situation` folds
as empty, never as a panic.

**Constant** in `tuning.rs`: `SITUATION_MAX_TOTAL: f32 = 5.0`, with a const
assert that it is below `-MORALE_SULKS_AT`.

## 4. The memories page

- `Game::memory_report` appends one `MemoryRow` per active thought: `name`,
  `blurb`, `subject: None`, `intensity` from `scaled_rows`, `age: "now"`.
  Sorted with memories by |intensity|, so the header "Mood {band}
  ({strength})" — `Game::morale` — is still the sum of its rows.
- The page has no scroll: the existing row-height census gains the five
  thought rows in its worst case.
- `ManifestMood.sum` reads `Game::morale` and picks up thoughts for free.

## 5. Tests

- Each trigger: fires in its situation, and not in the nearest miss
  (diagonal adjacency fires, distance 2 does not; a neutral neighbour
  fires neither; a dark machine vs a powered one; `Starved` vs `Running`;
  one amenity present).
- Clamp: six negative points total to −5; `scaled_rows` sums to `sum`.
- One derivation: `Game::morale(w)` equals the `CycleModifiers::morale`
  `task_progress_system` builds for `w` on the same tick.
- Isolation: `opinion_of` and `Game::bond` are unchanged by an active
  `BesideRival`.
- The ladder: a program with no memories and every negative thought does
  not reach `Sulking`; one at −4 from memories does.
- Empty `assets/thoughts/`: morale equals the memory fold.
- Load: a save opened and read before any tick reports the situational term.
- Asset census: every `Trigger` variant has a shipped def.
- `balance_sim` does not model morale or base staff, so no curve moves;
  run it as the gate anyway.
