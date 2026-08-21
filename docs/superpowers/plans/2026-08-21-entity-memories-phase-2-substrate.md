# Entity memories — Phase 2: Substrate

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Owned programs carry a store of memories that survives a
save/load, written through one door, read through two, with intensity
derived from the game clock and the catalogue defined in data.

**Architecture:** A content directory (`assets/memories/`) loaded by
`MemoryDb` on the `AffixDb`/`SectorDb` absent-is-silent pattern; a
`Memories` component minted at `roster_parts` beside Phase 1's
`ProgramId`; intensity computed from `GameClock` at read time and never
stored; `Game::remember` as the single writer, on the model of
`Game::apply_damage`.

**Tech Stack:** Rust, `bevy_ecs` 0.19, serde/RON assets and saves.

**Spec:** `docs/superpowers/specs/2026-08-21-entity-memories-design.md` —
read sections 1, 2, 4, 6 and 9 before starting. This plan implements those
and nothing else.

**Phase 1 is landed** (`6a1cd1c3`, `f336c89b`): `components::ProgramId`,
`resources::NextProgramId`, `roster_parts` minting, `CreatureSave::
program_id`, `SaveData::next_program_id`, and the legacy-load mint. Read
`crates/engine/src/game/spawning.rs:327` before Task 3 — you are widening
that same tuple again.

## Global Constraints

- **No `SAVE_FORMAT_VERSION` bump.** It is 32 and stays 32. The one new
  save field is additive behind `#[serde(default)]`, which the field-named
  RON format supports. Needing a bump means you changed a field's meaning
  rather than adding one.
- **No RNG, anywhere in this phase.** No `GameRng`, no local `StdRng`.
  `remember` in particular draws nothing — that is what keeps every seeded
  test and every `dev-arenas/` report where they are. If you find yourself
  reaching for a draw, stop.
- **Nothing this phase writes may reach `Stats`, damage, accuracy, or any
  figure `balance_sim` models.** The balance gate is meant to be blind to
  this feature. A moved curve means you touched something you shouldn't
  have.
- **An empty database is valid and inert.** Deleting `assets/memories/`
  must restore the pre-memory game exactly: `remember` becomes a no-op and
  both readers return zero. This is asserted, not assumed.
- **Owned programs only.** The store is minted at the roster barrier. The
  player, hostiles, wild programs and structures carry no `Memories` and
  `remember` on any of them is a silent no-op.
- **Formation logs nothing.** No `MessageLog` write anywhere in this
  phase. Announcing memories is a deliberate later decision.
- Follow the repo's comment discipline: comments explain *why*, never
  *what*.
- Gates while iterating: `cargo test -p feral-processes-engine <name>`.
  Before the phase is called done: `cargo fmt`, `cargo clippy --workspace`,
  `cargo test --workspace`.

**Evidence standard.** Every test in this plan must be mutation-proved:
delete the fix, run the test, watch it fail, restore the fix. Record the
mutation applied and the failure seen. A test that passes with its fix
removed is coverage-shaped and worse than nothing; this repo has shipped
vacuous ones twice.

**Known trap — a new `Resource` shifts query iteration order.** Registering
`MemoryDb` can redden an unrelated test in an untouched subsystem, because
bevy's query iteration order is not stable and some test implicitly leans
on it. That is a latent unsorted-query test, not a regression you
introduced. Fix that test's incidental coupling (sort, or assert on a set);
do not reseed it and do not revert the resource.

**Known trap — a single-crate run is a different build.** `-p
feral-processes-engine` and `--workspace` compile different crate sets and
so shift the RNG stream. A seeded test can fail in one and pass in the
other. Confirm any surprise under `--workspace` before treating it as real.

---

## Decisions this plan takes, and why

The spec leaves four things to the implementation. They are settled here so
they are not re-argued mid-task.

1. **`remember` returns an outcome enum, rather than printing a warning.**
   The spec asks for a subject-kind mismatch to be "refused with a
   warning". The engine has no runtime warning channel — `load_dir`
   warnings are returned `String`s surfaced once at startup, and the
   message log is player-facing text that section 4 forbids this feature
   from writing. So the refusal is *returned*:

   ```rust
   pub enum Remembered { Written, NoStore, UnknownDef, WrongSubject }
   ```

   Four observable outcomes, one per no-op the spec names, which is what
   makes the no-op rule testable without a `debug_assert!` panic in the
   middle of the test that asserts it. Callers may ignore it; it is not
   `#[must_use]`.

2. **The remembered name lives on `Memory`, not only on `MemorySave`.**
   The spec puts the display name of a `Program` subject in the save "since
   the program may be gone by the time the screen draws it". Resolved at
   *save* time that is unrecoverable — a program despawned before the next
   save takes its name with it, and the screen has nothing to draw between
   formation and the first save either. So `Memory` carries
   `subject_name: Option<String>`, captured by `remember` at formation and
   refreshed on reinforcement (a renamed program updates), and `MemorySave`
   mirrors the field rather than deriving it.

3. **`MemorySubject` derives serde directly; there is no save-side
   mirror.** `save::CronjobKind` mirrors `components::TaskKind` on the
   stated grounds of keeping serde off the engine enum. A mirror here would
   be a second copy of a six-variant enum that a new variant must be added
   to twice, with nothing failing to compile if it isn't — and the whole
   point of the exhaustive-match rule on this enum is that a new variant
   *must* fail to compile. So `MemorySubject` derives `Serialize`/
   `Deserialize`, and `TaskKind` gains them for `Activity`'s sake. Both are
   fieldless-or-flat and the on-disk form is field-named RON, so variants
   encode by *name*: reordering them is not a save-format change, unlike
   `perks::Perk` under bincode.

4. **A memory naming an unknown def is kept, not dropped.** A mod file
   removed mid-run leaves entries whose `def` no longer resolves. Both
   readers skip them (contributing zero) and `remember` treats the id as
   `UnknownDef`. Keeping them means restoring the file restores the
   memories, and it is what makes the empty-database property fall out of
   the readers for free rather than needing a load-time purge.

**Deferred on purpose, do not build:** fusion inheriting the dominant
parent's memories (`fuse_companions` gets a fresh empty store from
`roster_parts`, which is a defensible default and a content question, not a
substrate one); `MEMORY_MAUL_FRACTION` (Phase 3 adds it with its trigger);
`MEMORY_AVOIDANCE_THRESHOLD` (Phase 5 adds it with its hook); the
`CLAUDE.md`/`AGENTS.md`/`docs/seams.md` entries (Phase 5, when the feature
is whole — a seam doc describing a half-built feature is a recorded trap in
this repo).

---

## File structure

| File | Responsibility in this phase |
|---|---|
| `crates/engine/src/memories.rs` | **create** — `MemoryId`, `MemorySubjectKind`, `MemoryDef`, `MemoryDb::load_dir` |
| `crates/engine/src/lib.rs` | declare `pub mod memories;` (alphabetical: after `items_db`, before `nemesis`) |
| `assets/memories/*.ron` | **create** — the four shipped defs |
| `assets/memories/README.md` | **create** — the schema reference |
| `crates/engine/src/components.rs` | `Memories`, `Memory`, `MemorySubject`, `Memory::intensity`; serde on `TaskKind` (`:702`) |
| `crates/engine/src/tuning.rs` | the labelled memory section, at the end of file |
| `crates/engine/src/game/memories.rs` | **create** — `remember`, `morale`, `opinion_of` |
| `crates/engine/src/game/mod.rs` | declare `pub(crate) mod memories;` (after `listen`, before `party`) |
| `crates/engine/src/game/spawning.rs` | `roster_parts` (`:327`) widens to five |
| `crates/engine/src/game/lifecycle.rs` | `AssetDbs` (`:1529`), `load_asset_dbs`, both constructors' destructure + `insert_resource` (`:74`, `:300`), the creature load loop (`:797`), the save query (`:1007`) and save site (`:1142`) |
| `crates/engine/src/save.rs` | `MemorySave`, `CreatureSave::memories`, `sample_creature` (`:1030`) |
| `crates/engine/src/tests/memories.rs` | this phase's tests, appended to Phase 1's seven |
| `crates/engine/src/tests/assets.rs` | the shipped-catalogue census |

---

## Task 1: The catalogue

**Files:**
- Create: `crates/engine/src/memories.rs`, `assets/memories/` (four `.ron`
  files and `README.md`)
- Modify: `crates/engine/src/lib.rs`, `crates/engine/src/game/lifecycle.rs`,
  `crates/engine/src/tests/assets.rs`

**Interfaces:**
- Produces: `memories::MemoryId` (a `#[serde(transparent)]` String newtype,
  `talents::TalentId`'s shape exactly — `Clone, Debug, PartialEq, Eq, Hash,
  PartialOrd, Ord, Serialize, Deserialize`, with `as_str`, `From<&str>` and
  `Display`); `memories::MemorySubjectKind`; `memories::MemoryDef`;
  `memories::MemoryDb` (`Resource, Default`) with `load_dir`, `get`, and an
  `all` iterator for the census.
- Consumes: nothing from an earlier task.

- [ ] **Step 1: Write the failing tests**

Two places.

In `memories.rs`'s own `#[cfg(test)] mod tests`, following `talents.rs`'s
worked example (build files into `support::scratch_assets_dir("memories")`,
then `load_dir`) — **not** `std::env::temp_dir` directly; engine fixtures
leaking into `/tmp` exhausted the tmpfs inode table once already:

1. `a_well_formed_def_loads_with_no_warnings`.
2. `a_malformed_file_is_skipped_and_warns_without_losing_its_neighbours` —
   two files, one broken; the good one loads, one warning is returned, the
   load does not fail.
3. `an_absent_directory_loads_an_empty_database_silently` — `load_dir` on a
   path that does not exist returns `Ok` with an empty db and **no**
   warning. This is the deleting-`assets/memories/` property at the load
   end.
4. `two_files_claiming_one_id_resolve_the_same_way_every_run` — the sort
   before the walk, `TalentDb::load_dir`'s reason.

In `crates/engine/src/tests/assets.rs`, a census over the shipped
directory, in the mould of the talent censuses at `:1503`:

5. `every_shipped_memory_def_is_well_formed` — ids unique, `valence`
   non-zero and finite, `half_life > 0`, `strike_cap >= 1`, `name` and
   `blurb` non-empty.

**Not** the spec's "every def's declared `subject` kind is reachable from a
real `remember` call site" — there are no call sites until Phase 3, which
is where that clause belongs. Do not write a vacuous version of it here.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine memories`
`cargo test -p feral-processes-engine assets::`

Expected: compile failure — `memories` does not exist. A valid red.

- [ ] **Step 3: Write `memories.rs`**

`MemoryDef`, deserialize-only is enough (`AbilityDef`'s shape, not
`SaveData`'s):

```rust
pub struct MemoryDef {
    pub id: MemoryId,
    pub name: String,
    pub blurb: String,
    pub valence: f32,
    pub half_life: u64,
    pub subject: MemorySubjectKind,
    pub strike_cap: u32,
}
```

Every field added to this struct *later* must be `#[serde(default)]`, per
the standing rule for `SpeciesDef`/`StructureDef`/`ItemDef`/`AbilityDef`.
The seven above are the initial schema and are required; say so in the doc
comment so nobody defaults them retroactively.

`MemorySubjectKind` is the authorable half of `MemorySubject`:
`Nothing, Program, Species, Structure, BaseTile, Activity`. It exists so
the def can *declare* what it is about without the record's payload; the
match between the two is `remember`'s to check (Task 3).

`MemoryDb::load_dir` follows `AffixDb::load_dir` (`affixes.rs:150`) for the
absent-directory rule and `TalentDb::load_dir` (`talents.rs:141`) for the
sorted walk and the per-file skip-and-warn. Both halves matter and they
come from different neighbours, so read both before writing this.

The module doc states the empty-database property in the terms `NemesisDb`
and `SectorDb` state theirs: deleting the directory is a supported way to
play, not an install fault. This is the opposite call from `TalentDb`,
whose `?` on a missing directory is deliberate — say which one you are and
why, or the next reader will "fix" it to match the wrong neighbour.

- [ ] **Step 4: Ship the four defs and the README**

`assets/memories/` gains one file per def. These are Phase 3's four
triggers' catalogue entries, shipped now because the census needs something
to census and because a def is content, not a hook:

| file | id | valence | half_life | subject | strike_cap |
|---|---|---|---|---|---|
| `bonded_in_battle.ron` | `bonded_in_battle` | `4.0` | `4000` | `Program` | `5` |
| `mauled_by.ron` | `mauled_by` | `-8.0` | `6000` | `Species` | `4` |
| `stranded_at.ron` | `stranded_at` | `-6.0` | `3000` | `BaseTile` | `3` |
| `hard_won.ron` | `hard_won` | `5.0` | `5000` | `Nothing` | `3` |

Write `name` and `blurb` as player-facing prose — the blurb is one line of
flavour, the name is what a screen row leads with. Mind the no-occult-
naming rule and the "GC Entropy Sweep" vocabulary note in `CLAUDE.md` when
wording them.

`assets/memories/README.md` is the schema reference, in the shape of
`assets/talents/README.md`: every field, its units (`half_life` is in
*ticks*), the `MemorySubjectKind` vocabulary, the skip-and-warn rule, and
an explicit statement that the directory may be deleted. Say that
`MemoryDef` has **no `trigger` field** and why — a modder will look for
one, and the spec's argument (a trigger vocabulary invented from four
samples is the speculative abstraction the code principles forbid) is what
answers them.

- [ ] **Step 5: Wire the db into both constructors**

`AssetDbs` gains a `memories` field (`lifecycle.rs:1529`);
`load_asset_dbs` loads it with the absent-is-silent comment its neighbours
carry; both `Game::new` (`:74`) and `Game::load` (`:300`) destructure it
and `insert_resource` it.

**Both doors, or neither.** `Game::load` rebuilds the db from the asset
directory, so a db inserted only in `new` is missing from every loaded
game — the same failure mode `load_asset_dbs`' own doc comment describes
for the economy-role check.

- [ ] **Step 6: Run the tests**

`cargo test -p feral-processes-engine memories`
`cargo test -p feral-processes-engine assets::`

Expected: PASS.

- [ ] **Step 7: Mutation-prove each of the five**

One at a time, restoring between. Suggested: make a malformed file abort
the whole load (test 2); make the absent directory return an `Err` (test
3); drop the `paths.sort()` (test 4 — if it stays green, the test is
reading map order rather than file order and needs rewriting, not
accepting); edit a shipped `.ron` to a `strike_cap` of 0 (test 5, and
restore it — an asset toggle left in place has bitten this repo, so
`git diff --quiet assets/` before you believe anything).

- [ ] **Step 8: Commit**

```bash
git add crates/engine/src/memories.rs crates/engine/src/lib.rs \
        crates/engine/src/game/lifecycle.rs crates/engine/src/tests/assets.rs \
        assets/memories
git commit -m "feat(memories): a catalogue of what a program can remember"
```

Stage explicit paths, never `git add -A` — another agent's worktree
gitlink under `.claude/worktrees/` gets swept up otherwise.

---

## Task 2: The record, and intensity derived from the clock

**Files:**
- Modify: `crates/engine/src/components.rs`, `crates/engine/src/tuning.rs`,
  `crates/engine/src/tests/memories.rs`

**Interfaces:**
- Consumes: `MemoryId`, `MemoryDef`, `MemorySubjectKind` from Task 1.
- Produces: `components::Memories(pub Vec<Memory>)` (`Component, Default`);
  `components::Memory`; `components::MemorySubject`;
  `Memory::intensity(&self, def: &MemoryDef, now: u64) -> f32`;
  `MemorySubject::kind(&self) -> MemorySubjectKind`; the tuning constants.

- [ ] **Step 1: Write the failing tests**

Appended to `crates/engine/src/tests/memories.rs`. These are pure
arithmetic on hand-built values — no `Game`, no world, no assets.

1. `intensity_halves_at_one_half_life_and_quarters_at_two` — exact, to
   `f32::EPSILON`-scale tolerance. This is the test that says the exponent
   is `(now - reinforced) / half_life` and not something adjacent to it.
2. `intensity_is_undecayed_at_the_moment_it_forms` — `now == reinforced`
   gives exactly `valence * strikes`.
3. `strikes_compound_intensity_up_to_the_cap_and_no_further` — three
   strikes against a `strike_cap` of 3 and against a `strike_cap` of 2 give
   different figures; four against a cap of 3 gives the same as three.
4. `a_negative_valence_stays_negative_however_it_decays` — decay is a
   magnitude scale, never a sign flip, and `morale` is a signed sum that
   depends on it.
5. `the_half_life_multiplier_scales_every_grudge_at_once` — at a
   multiplier of 2 the same elapsed ticks leave more intensity than at 1.
   Write it against `tuning::MEMORY_HALF_LIFE_MULTIPLIER` *relatively* —
   assert the ordering of two computed figures, not a hardcoded number, or
   the test pins the dial and the dial stops being a dial.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine memories`

Expected: compile failure on `Memory` / `MemorySubject`.

- [ ] **Step 3: Add the three types to `components.rs`**

```rust
pub struct Memory {
    pub def: MemoryId,
    pub subject: MemorySubject,
    pub subject_name: Option<String>,
    pub reinforced: u64,
    pub strikes: u32,
}

pub enum MemorySubject {
    Nothing,
    Program(ProgramId),
    Species(SpeciesId),
    Structure(StructureId),
    BaseTile { x: i32, y: i32 },
    Activity(TaskKind),
}
```

`MemorySubject` derives `Clone, Debug, PartialEq, Serialize, Deserialize`;
`TaskKind` (`:702`) gains `Serialize, Deserialize` so `Activity` can. See
decision 3 above for why there is no save-side mirror, and put that
argument on `MemorySubject`'s doc comment rather than in this plan alone.

**`BaseTile`, not `Place`, and the doc comment must say why.** Base space
and surface space are the same two integers meaning different things, and
`docs/seams.md` records what reading one as the other did — it put the
base's roster on the open grid. Naming the space in the type is what stops
that recurring. A surface variant, when content asks for one, is zone-local
and must be wiped by name in `enter_next_zone` alongside `StackMemory`,
`BuybackLedger` and `PopulatedChunks`; note that on the enum now, where the
person adding the variant will read it.

`MemorySubject::kind()` is an **exhaustive** match to `MemorySubjectKind` —
`cell_mark`'s rule: a seventh variant must fail to compile rather than ship
answering the wrong kind. No `_ =>` arm, ever.

`Memory::intensity`:

```
valence * min(strikes, strike_cap) as f32
        * 2f32.powf(-(elapsed as f32) / (half_life as f32 * MULT))
```

`elapsed` is `now.saturating_sub(self.reinforced)` — a memory reinforced at
a tick later than `now` is not reachable through `remember`, but a
hand-edited save can hold one and an underflow there is a panic in release
arithmetic, not a wrong number.

Document that intensity is **derived, never stored**, in the terms
`Platform`'s radius and a Broker's board are documented: nothing ticks,
nothing oscillates, reinforcement is a single field write, and a stored
weight cannot drift out of step with the clock the way a per-tick decrement
can.

- [ ] **Step 4: Add the tuning section**

At the end of `crates/engine/src/tuning.rs`, a labelled section in the
file's existing style:

- `MEMORY_HALF_LIFE_MULTIPLIER: f32 = 1.0` — the global stickiness dial.
  One number makes every grudge in the game longer or shorter. Neutral at
  1.0 so the authored `half_life`s mean what they say.
- `MEMORY_CAP_PER_PROGRAM: usize = 12` — **a layout constraint before it is
  a feel one.** `draw_popup` pages a `Row::Item` span and a page with none
  drops any row past the bottom in silence. A `PopupSize::Large` popup
  holds 23 rows at the tightest window the font ramp allows (600px: the
  font clamps at `MIN_UI_FONT` 16, so `line_height` 20 and `inset` 6.67
  against a body of `600 * 0.85`), and the page spends the rest on a title,
  a Morale header and their spacing. Phase 4's
  `the_tallest_memory_page_fits_its_popup` is what actually holds this;
  raising the cap past what fits requires giving the page a scroll first.
  Put that derivation in the doc comment — a bare `12` reads as taste.
- `MEMORY_FORGET_THRESHOLD: f32 = 0.5` — intensity **magnitude** below
  which an entry is dropped at the next formation. At the shipped
  half-lives this is a little under four half-lives from a single strike.

`MEMORY_MAUL_FRACTION` and `MEMORY_AVOIDANCE_THRESHOLD` are **not** added
here. They arrive with the code that reads them, in Phases 3 and 5.

- [ ] **Step 5: Run the tests**

`cargo test -p feral-processes-engine memories`

Expected: PASS.

- [ ] **Step 6: Mutation-prove each of the five**

Suggested: change the exponent's base to `e` (test 1); drop the
`min(strikes, strike_cap)` clamp (test 3); take `.abs()` of the result
(test 4); drop the multiplier from the denominator (test 5). Test 2 is
proved by test 1's mutation only if the mutation moves the zero point —
prove it separately by making `intensity` decay from tick 0 rather than
from `reinforced`.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/components.rs crates/engine/src/tuning.rs \
        crates/engine/src/tests/memories.rs
git commit -m "feat(memories): what a memory is, and how it fades"
```

---

## Task 3: The one door

**Files:**
- Create: `crates/engine/src/game/memories.rs`
- Modify: `crates/engine/src/game/mod.rs`,
  `crates/engine/src/game/spawning.rs:327`,
  `crates/engine/src/tests/memories.rs`

**Interfaces:**
- Consumes: everything from Tasks 1 and 2.
- Produces: `Game::remember(&mut self, who: Entity, def: &str, subject:
  MemorySubject) -> Remembered`; `game::memories::Remembered`;
  `roster_parts` returning a fifth element, `Memories::default()`.

- [ ] **Step 1: Write the failing tests**

Appended to `crates/engine/src/tests/memories.rs`. These need a `Game`; use
`support`'s existing fixtures and `spawn_tamed` rather than writing new
ones — check `crates/engine/src/tests/support.rs` before adding anything.

1. `a_first_remember_forms_one_entry_at_the_current_tick` — one entry,
   `strikes == 1`, `reinforced == game.current_tick()`.
2. `remembering_the_same_thing_again_reinforces_rather_than_forking` — tick
   the clock forward, `remember` the same `(def, subject)`, assert **one**
   entry with `strikes == 2` and the later `reinforced`.
3. `two_subjects_of_one_def_are_two_memories` — the same def about two
   different programs does *not* collapse. Identity is the pair, not the
   def.
4. `a_renamed_subject_reinforces_rather_than_forking` — the entry is keyed
   on `(def, subject)` and `subject_name` is refreshed, not compared. This
   is the trap decision 2 introduces: a name in the key would fork a
   program's history every time the player renames it.
5. `strikes_saturate_at_the_defs_cap` — `remember` past `strike_cap` leaves
   `strikes` at the cap.
6. `a_faded_entry_is_dropped_at_the_next_formation` — an entry under
   `MEMORY_FORGET_THRESHOLD` is gone after an unrelated `remember`.
   Eviction is lazy and happens here; nothing sweeps.
7. `over_the_cap_the_weakest_goes_and_the_strongest_survives` — form
   `MEMORY_CAP_PER_PROGRAM + 1` distinct memories and assert the list is at
   the cap and still holds the strongest.
8. `remember_is_a_no_op_on_a_hostile_a_structure_and_the_player` — three
   subjects, `Remembered::NoStore` each, no panic, no component appears.
   One test, all three: the hostile arm alone passes against a fix that
   only checks `Hostile`.
9. `a_subject_of_the_wrong_kind_is_refused` — a `Species` subject against a
   `BaseTile` def returns `Remembered::WrongSubject` and writes nothing.
10. `an_unknown_def_id_is_a_silent_no_op` — `Remembered::UnknownDef`,
    nothing written. This is the deleted-mod-file case and the empty-
    database property at the write end.
11. `remember_draws_no_rng` — snapshot the `GameRng` stream position,
    `remember` several times, assert the next draw is unchanged. Compare a
    draw before and after rather than reading internals.
12. `every_door_into_the_roster_hands_out_a_memory_store` — a program from
    `grant_starting_program`, one from `spawn_tamed`, and a fused one all
    carry `Memories`. `fuse_companions` hand-writes its own component list,
    so it is the door that can silently skip a widened tuple; Phase 1's
    `a_fused_program_takes_a_fresh_id` has the fixture to copy.

- [ ] **Step 2: Run them and watch them fail**

`cargo test -p feral-processes-engine memories`

Expected: compile failure on `remember`.

- [ ] **Step 3: Mint the store at the roster barrier**

`roster_parts` returns a fifth element, `Memories::default()`. The
compiler names all five call sites; that is the point of the shared tuple,
so do not hand-write a `Memories` at any of them.

Extend the function's doc comment. Phase 1 already added the minting
argument there; the store is the same argument's other half — a door that
skipped it produces an owned program that can never *hold* a memory, and
the symptom (that one companion's screen is always empty) reads as memories
being broken rather than as a door short a component.

- [ ] **Step 4: Write `remember`**

`crates/engine/src/game/memories.rs`, declared in `game/mod.rs`.

Order inside the function, and it is load-bearing:

1. Resolve the def from `MemoryDb`. Absent → `UnknownDef`, and return
   **before** touching the store. This is what makes a deleted directory
   inert rather than merely quiet.
2. Check `subject.kind()` against `def.subject`. Mismatch →
   `WrongSubject`.
3. Take the `Memories` component. Absent → `NoStore`.
4. Resolve `subject_name`: for `MemorySubject::Program(id)`, find the
   living entity carrying that `ProgramId` and take `creature_label`
   (`game/party.rs:197`); `None` for every other variant and for a program
   that is already gone. Do this *before* the mutable borrow of the
   component, or you will fight the borrow checker — small functions are
   the fix here, not `.clone()`.
5. Find an existing entry with the same `(def, subject)`. Found →
   `strikes = (strikes + 1).min(def.strike_cap)`, `reinforced = now`,
   `subject_name` refreshed. Not found → push a new entry at
   `strikes: 1`.
6. Evict: drop every entry whose intensity **magnitude** is under
   `MEMORY_FORGET_THRESHOLD`, then while the list exceeds
   `MEMORY_CAP_PER_PROGRAM` drop the weakest by magnitude. Magnitude, not
   signed value, at both — a signed comparison evicts every grudge and
   keeps every fondness, which is not a memory system.
7. Return `Written`.

The entry just written must survive its own eviction pass: it is at full
undecayed intensity, so it does unless `valence * 1` is itself under the
threshold, which the census forbids by refusing a zero valence — but say so
in a comment, because it is the kind of thing a later threshold change
quietly breaks.

Document `remember` as **the one door**, in the terms `Game::apply_damage`
is documented: a rule that must see every memory goes here, and the missing-
component no-op is the same deliberate asymmetry `spend_power` uses for a
missing `PowerReserve` — it is what keeps hostiles, structures and the
player safe without a branch at every call site.

Say plainly, in the doc comment, that this function draws no RNG and writes
no log line, and why each matters.

- [ ] **Step 5: Run the tests**

`cargo test -p feral-processes-engine memories`
`cargo test -p feral-processes-engine party spawning`

Expected: PASS. If something unrelated goes red, re-read the two traps in
Global Constraints before touching anything.

- [ ] **Step 6: Mutation-prove all twelve**

One at a time, restoring between. Suggested: always push instead of
reinforcing (tests 2, 4); compare on `def` alone (test 3); include
`subject_name` in the key (test 4); drop the `.min(strike_cap)` (test 5);
skip the threshold sweep (test 6); evict on signed value rather than
magnitude (test 7 — verify it goes red, since a plan that says "magnitude"
and code that says `<` can both look right); insert `Memories` on any
entity lacking one (test 8); drop the kind check (test 9); default an
unknown def to the first in the db (test 10); add a `self.rng()` draw (test
11); hand-write a `Memories` at four of the five roster sites (test 12 —
the fifth omission is what it must catch).

Twelve mutations is the bulk of this phase's cost and it is the point: this
is the door every later phase writes through.

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/game/memories.rs crates/engine/src/game/mod.rs \
        crates/engine/src/game/spawning.rs crates/engine/src/tests/memories.rs
git commit -m "feat(memories): one door writes what a program remembers"
```

---

## Task 4: The two readers

**Files:**
- Modify: `crates/engine/src/game/memories.rs`,
  `crates/engine/src/tests/memories.rs`

**Interfaces:**
- Consumes: Task 3.
- Produces: `Game::morale(&self, who: Entity) -> f32`;
  `Game::opinion_of(&self, who: Entity, subject: &MemorySubject) -> f32`.

Both are `&self` — they derive, they do not evict. Eviction is `remember`'s
alone, or a read-only screen mutates the roster it is drawing.

- [ ] **Step 1: Write the failing tests**

1. `morale_sums_every_memorys_current_intensity` — one positive and one
   negative memory; the sum is signed and matches the two intensities
   computed by hand.
2. `morale_falls_as_a_good_memory_fades` — the same program, two clock
   readings, strictly lower. This is what says the readers derive rather
   than read a stored figure.
3. `opinion_of_counts_only_memories_about_that_subject` — three memories,
   two subjects; the restricted sum is not the total.
4. `opinion_of_an_unremembered_subject_is_zero` — not `NaN`, not a panic.
5. `a_program_with_no_memories_has_zero_morale` and a **body without the
   component at all** — the player and a hostile — also reads zero rather
   than panicking. One test, both cases.
6. `a_memory_naming_an_unknown_def_contributes_nothing` — decision 4's
   property, at the read end. Build the store with a def id no file
   defines.
7. `an_empty_database_leaves_every_reader_at_zero` — a `Game` built against
   an asset tree with `assets/memories/` omitted has memories that read
   zero. Build that tree from `support::copy_shipped_assets`
   (`tests/support.rs:389`) and **assert inside the test that the copied
   tree has no `memories` directory**, rather than relying on the fixture's
   subdirectory list to keep omitting it. That list is a hardcoded set of
   names; adding `"memories"` to it later would invert this test in
   silence, and the assertion is what turns that into a failure. Never
   mutate the real `assets/` — an asset toggle left in place has bitten
   this repo, and `git diff --quiet assets/` is the check before believing
   any number.

- [ ] **Step 2: Run them and watch them fail**

Expected: compile failure on `morale`.

- [ ] **Step 3: Write both readers**

`opinion_of` is `morale` restricted by subject; write it so that is
structurally true (one fold, a predicate parameter) rather than two folds
that can disagree about whether an unknown def counts. The repo has been
bitten four times by a doc comment claiming to mirror a formula it in fact
copied — if you write "same as `morale`" in a comment, that is the signal
to share the function instead.

Both skip a memory whose def does not resolve, which is where the empty-
database property comes from.

- [ ] **Step 4: Run, then mutation-prove all seven**

Suggested: sum magnitudes rather than signed values (test 1); read
`strikes * valence` without the decay term (test 2); drop the subject
filter (test 3); `unwrap()` the def lookup (tests 5, 6, 7).

- [ ] **Step 5: Commit**

```bash
git add crates/engine/src/game/memories.rs crates/engine/src/tests/memories.rs
git commit -m "feat(memories): morale, and what a program thinks of one thing"
```

---

## Task 5: The save round trip

**Files:**
- Modify: `crates/engine/src/save.rs`,
  `crates/engine/src/game/lifecycle.rs`,
  `crates/engine/src/tests/memories.rs`

**Interfaces:**
- Consumes: Tasks 2 and 3.
- Produces: `save::MemorySave`; `CreatureSave::memories: Vec<MemorySave>`,
  `#[serde(default)]`.

- [ ] **Step 1: Write the failing tests**

Use `support::scratch_assets_dir` for the save path, **not**
`std::env::temp_dir` directly. `tests/refactor.rs` around `:727` is a
worked example.

1. `a_programs_memories_survive_a_save_and_load` — form two memories of
   different subject kinds, save, load, assert def, subject, strikes and
   `reinforced` all came back and that `morale` reads the same figure.
   **A RON round trip is not enough and must not be the whole test**: a
   `#[serde(skip)]` leaves a round trip green while the field never reaches
   the file. Go through `game.save(&path)` and `Game::load(&path, &assets)`.
2. `a_remembered_name_survives_the_program_it_names` — form a
   `Program`-subject memory, destroy that program, save, load, assert
   `subject_name` is still there. This is decision 2's whole reason;
   without it the field is unfalsifiable.
3. `a_save_written_before_memories_existed_loads_with_an_empty_store` —
   deserialize `SaveData`, strip the `memories` keys, re-serialize, load.
   Every owned program gets an empty `Memories`, not a missing component:
   a loaded companion that cannot hold a memory is the legacy-load bug this
   catches.
4. `every_subject_kind_survives_the_round_trip` — one memory of each of the
   six variants through a real save/load. `Activity(TaskKind)` is the one
   most likely to break, since `TaskKind` gained its serde derives for this
   and nothing else uses them.

- [ ] **Step 2: Run them and watch them fail**

Expected: compile failure on `memories`.

- [ ] **Step 3: Add `MemorySave` and the field**

A **named struct, never a positional tuple** — a tuple is the one shape
field-named RON does not save you from, and the next property added to a
memory would cost a legacy field. Mirror `Memory`'s five fields.

`CreatureSave::memories: Vec<MemorySave>` behind `#[serde(default)]`.
Doc-comment it with why it earned no `SAVE_FORMAT_VERSION` bump — the
neighbouring fields model that comment style, and note that
`#[serde(default)]` here is doing real work rather than being decorative,
because the on-disk form is field-named RON.

`sample_creature()` (`:1030`) needs the new field.

- [ ] **Step 4: Write it and read it back**

Save site: add `Option<&Memories>` to the nested half of the creature query
(`lifecycle.rs:1007` — the nesting exists because bevy's query tuples top
out at 15 and the outer one is full; the inner is at 13, so this fits) and
write it beside `program_id` at `:1142`.

Load: insert `Memories` from `c.memories` inside the `if c.tamed` arm at
`:797`, beside `ProgramId`. Owned programs only — a wild creature's
`memories` is written empty and ignored, exactly as its `power` is.

- [ ] **Step 5: Run the tests**

`cargo test -p feral-processes-engine memories`
`cargo test -p feral-processes-engine save`

Expected: PASS. `a_save_survives_a_round_trip_through_ron_unchanged` must
stay green — if it reddens you changed a field's *shape*, not added one.

- [ ] **Step 6: Mutation-prove all four**

Suggested: `#[serde(skip)]` on the field (test 1 — and confirm the RON
round-trip test stays green under it, which is the point of going through
the file); drop `subject_name` at the save site (test 2); insert `Memories`
only when `c.memories` is non-empty (test 3); mirror `TaskKind` to a
save-side enum missing `Excavate` (test 4).

- [ ] **Step 7: Commit**

```bash
git add crates/engine/src/save.rs crates/engine/src/game/lifecycle.rs \
        crates/engine/src/tests/memories.rs
git commit -m "feat(save): a program's memories survive the round trip"
```

---

## Task 6: The full gate

- [ ] **Step 1: Run everything**

```bash
cargo fmt
cargo clippy --workspace
cargo test --workspace
cargo test -p feral-processes-engine balance_sim
```

`balance_sim` is called out separately because it is the one suite whose
*silence* is the evidence: this feature is meant to be invisible to it. A
moved curve here means something in this phase reached `Stats`, and the
fix is to find that, not to re-baseline the curve.

- [ ] **Step 2: Confirm the assets are untouched**

```bash
git status --porcelain assets/
```

Only the four new `.ron` files and the README. An asset toggle left in
place after a measurement has cost this repo a shipped item's `grants:`
block before.

- [ ] **Step 3: Confirm the version did not move**

`SAVE_FORMAT_VERSION` is 32. The workspace version in the root
`Cargo.toml` is unchanged and `CHANGELOG.md` has no new section — this is a
branch commit, and the release happens once at the merge.

---

## Phase 2 done when

- `cargo test --workspace` is green and `cargo clippy --workspace` is
  clean.
- Every new test has a recorded mutation that made it fail.
- `SAVE_FORMAT_VERSION` is unchanged at 32.
- `balance_sim`'s curves are untouched.
- Deleting `assets/memories/` leaves a game that starts, plays and saves —
  asserted by three tests (the absent directory, the empty database at the
  write end, the empty database at the read end) rather than checked by
  hand.
- **Nothing calls `remember` outside the tests.** That is expected: Phase
  3's four triggers are its first callers. Do not invent one to make the
  door look used, and do not wire a trigger early because it is a two-line
  change — Phase 3 is where those get their own reviewer boundary.
- No `CHANGELOG.md` entry, no version bump, and no `CLAUDE.md`/
  `AGENTS.md`/`docs/seams.md` edits. The seams entries land in Phase 5,
  when the feature is whole; a seam doc describing a half-built feature is
  a recorded trap in this repo.
