# Entity memories — Phase 4: The screen

> **For agentic workers:** REQUIRED SUB-SKILL: Use
> superpowers:subagent-driven-development (recommended) or
> superpowers:executing-plans to implement this plan task-by-task. Steps
> use checkbox (`- [ ]`) syntax for tracking.

**Goal:** What a program remembers becomes readable. One derivation in the
engine (`Game::memory_report`), one sub-page off the roster
(`Mode::CompanionMemories`), and the two censuses that keep it inside its
popup. `MemoryDef::name` and `MemoryDef::blurb` get their first readers,
and `Game::morale` loses its phase-2 `dead_code` attribute.

**Architecture:** The gear inspect page's shape exactly — a read-only page
whose every figure is one engine call, opened with a letter from the list
that names its subject, with no scroll and a census saying it fits.

**Tech Stack:** Rust, `bevy_ecs` 0.19, `bevy_egui` through `Painter`.

**Spec:** `docs/superpowers/specs/2026-08-21-entity-memories-design.md` —
read sections 6, 7 and the Testing section before starting. This plan
implements section 7 and the two readers section 6 names, and nothing else.

**Phases 1–3 are landed** (`6a1cd1c3` … `58ee3edc`). Read
`crates/engine/src/game/memories.rs` first: `memory_sum` is the fold this
phase's report has to agree with, and `evict` is the weight rule its row
order mirrors.

## Global Constraints

- **No `.ron` change, no schema change, no `SAVE_FORMAT_VERSION` bump.**
  Nothing here is authored and nothing here is saved. If you find yourself
  editing `MemoryDef` or `MemorySave`, stop.
- **No RNG, and nothing reaching `Stats`, damage or accuracy.** A moved
  `balance_sim` curve means you changed something you shouldn't have.
- **The engine owns every per-row transform.** A read-only screen's row
  count is owned by app-core and drawn by gui; a fold, a sort or a subject
  rendered in the renderer opens the screen on rows that are not drawn.
  This is why the subject is rendered in `memory_report` and not in
  `render/party.rs`.
- **The page writes nothing.** `memory_report` and `morale` are `&self` and
  neither evicts — a read-only screen that rewrote the roster it is drawing
  would make what a program remembers depend on whether anyone looked.
- **An empty database stays inert.** With `assets/memories/` deleted the
  page opens on a program with entries in its store and draws no rows,
  because every entry is unresolvable. Do not gate the page on the database
  being non-empty — that makes the property hold by accident.
- Comments explain *why*, never *what*.
- Gates while iterating: `cargo test -p feral-processes-engine <name>`,
  `cargo test -p feral-processes-app-core <name>`, `cargo test -p
  feral-processes-gui <name>`. Before the phase is done: `cargo fmt`,
  `cargo clippy --workspace`, `cargo test --workspace`.

**Evidence standard.** Every test is mutation-proved: delete the fix, run
the test, watch it fail, restore. Record the mutation and the failure. A
test that passes with its fix removed is coverage-shaped and worse than
nothing.

**Known trap — a single-crate run is a different build.** `-p` and
`--workspace` compile different crate sets and so shift the RNG stream.
Confirm any surprise under `--workspace` before treating it as real.

**Known trap — `draw_row` clips vertically and never horizontally.** A row
wider than the popup body is drawn off the panel in silence. Two censuses
below, one per axis, and both must measure real text through
`with_painter`.

---

## Decisions this plan takes, and why

1. **The key is `R`, not the spec's `M`.** `M` on `Mode::Companion` is
   already `open_companion_manifest`, landed well before the spec was
   written; the spec's section 7 is simply wrong about what is free. `R`
   reads, and it is uppercase, which is what keeps it clear of
   `menu_shortcut`'s digits-then-lowercase scheme however large the roster
   grows — the reason `W`, `N`, `E`, `P` and `M` are all uppercase. Nothing
   else on that screen binds it.

2. **The subject is rendered in the engine, as an exhaustive match.**
   `cell_mark`'s rule: a seventh `MemorySubject` variant must fail to
   compile rather than ship invisible. It also *cannot* live in gui — a
   `Species` subject needs `SpeciesDb`, a `Structure` subject `StructureDb`,
   and a `Program` subject the remembered name off the record. The renderer
   has none of them.

3. **A `Program` subject renders `Memory::subject_name`, not a live
   lookup.** The name is captured at the write for exactly this reason: the
   program a memory is about can be destroyed, and the screen still has to
   say who it was. A live lookup would answer nothing for the case the
   field exists for, and would need the world at read time for the case it
   already covers.

4. **Rows are ordered by `|intensity|`, strongest first, stable.** It
   mirrors `evict`'s weight rule rather than describing it — magnitude and
   not signed value, so the deepest scar and the strongest bond sort
   together at the top and the row nearest eviction sits at the bottom. A
   signed sort would file every grudge below every fondness, which is not
   what the page is for. Stable, so ties keep insertion order and the page
   is reproducible run to run.

5. **An entry whose def no file defines is skipped**, contributing no row —
   `memory_sum`'s rule, and where the empty-database property comes from.
   The entry is *not* dropped from the store: restoring a removed mod file
   restores the memories that named it.

6. **`intensity` and `age` reach the row as numbers, and gui formats
   them.** `subject` is a `String` because rendering it needs the world;
   these two need nothing, and pre-rendering them would be inventing a
   second formatting seam for one screen. The engine-owns-transforms rule
   is about what changes a row *count*, which neither does.

7. **The page has no scroll**, the gear inspect page's call. `draw_popup`
   pages a `Row::Item` span and this page has none, so a row past the
   bottom is dropped in silence. `MEMORY_CAP_PER_PROGRAM` (12) is what
   bounds the height, and the fit census is what says the tallest possible
   page clears it. **Raising the cap past what fits requires giving the
   page a scroll first** — say so in the constant's doc comment.

---

## Task 1 — What the page is, in one engine call

**Files:** `crates/engine/src/views.rs`,
`crates/engine/src/game/memories.rs`, `crates/engine/src/tests/memories.rs`

**Interface:**

- `views::MemoryRow` — `name: String` (the def's), `blurb: String` (the
  def's), `subject: Option<String>` (`None` for `MemorySubject::Nothing`,
  which is about nothing in particular and has nothing to name),
  `intensity: f32` (signed, decayed, on the current tick), `age_ticks: u64`
  (`now - reinforced`, saturating).
- `Game::memory_report(&self, who: Entity) -> Vec<MemoryRow>` — `pub`.
- `Game::morale` becomes `pub` and loses its
  `#[cfg_attr(not(test), expect(dead_code, ...))]`. **`opinion_of` keeps
  its** — the hook that asks it is phase 5.
- A private subject renderer beside them, an **exhaustive** match on
  `MemorySubject`.

**Steps:**

- [ ] Write `MemoryRow` in `views.rs` with a doc comment carrying decisions
      3, 4 and 6 above.
- [ ] Write the subject renderer: `Nothing` → `None`; `Program` →
      `subject_name`, and where that is `None` a plain "a program that is
      gone" rather than an id (an id means nothing to a player); `Species`
      → the `SpeciesDb` display name, falling back to the raw id for a
      species a mod removed; `Structure` → the same shape off `StructureDb`;
      `BaseTile { x, y }` → a phrase naming it as **base** coordinates, not
      surface ones; `Activity` → the `TaskKind`'s label.
- [ ] Write `memory_report`: resolve each entry's def, skip what will not
      resolve, build the row, sort by `|intensity|` descending with a
      **stable** sort.
- [ ] Drop `morale`'s `dead_code` attribute and make both readers `pub`.

**Tests** (`crates/engine/src/tests/memories.rs`):

- [ ] A program holding two memories gets two rows, carrying the def's
      `name` and `blurb` — the first readers either field has ever had.
- [ ] Row order is by magnitude and not by sign: a strong grudge outranks a
      weak fondness. Build the two so a signed sort would invert them.
- [ ] Ties keep insertion order.
- [ ] `Nothing` renders no subject; a `Program` subject renders the
      remembered name; a `Program` subject whose program has been destroyed
      **still** renders it.
- [ ] A `Species` subject renders the species' display name and not its id.
- [ ] Age is the elapsed ticks since the last reinforcement, and
      reinforcing resets it to zero.
- [ ] Intensity on a row equals `Memory::intensity` for that entry — the
      row is a projection of the one formula, not a second copy of it.
- [ ] A body with no `Memories` reports an empty vec rather than panicking,
      `morale`'s asymmetry.
- [ ] **The empty database**: with a `MemoryDb::default()` inserted, a
      program with a full store reports zero rows and `morale` reads 0.0.
- [ ] The report writes nothing: the store's length is unchanged after a
      call, including one holding a faded entry `evict` would have dropped.

**Verification:**

- [ ] `cargo test -p feral-processes-engine memories`
- [ ] Mutation table for every test above.

---

## Task 2 — The mode, and the key that opens it

**Files:** `crates/app-core/src/lib.rs`, `crates/app-core/src/app/party.rs`,
`crates/app-core/src/app/input.rs`, `crates/app-core/src/tests/party.rs`

**Interface:**

- `Mode::CompanionMemories`, documented on the variant in the house style —
  what it shows, what opened it, where Esc goes.
- `App::pending_memory_program: Option<Entity>` — its **own** field, not
  `pending_equip_program` reused. `GearInspect`'s rule: a page's subject
  inherited from another page's field is a distinct failure per axis.
- `App::open_companion_memories`, and `handle_companion_memories_key`.

**Steps:**

- [ ] Add the variant, the field, the `input.rs` dispatch arm, and the arm
      in the `Mode` list around `lib.rs:1204` (the compiler will not point
      at this one — check it by reading what the list is *for* and whether
      a page over the map belongs in it).
- [ ] `R` in `handle_companion_key`, handled **before** `selected_index`,
      the way `W`/`N`/`E`/`P`/`M` are.
- [ ] `open_companion_memories`: read the highlighted program off
      `owned_pets()`, return quietly if the row is past the end, set the
      subject, clear `status_line`. **It leaves `menu_selected` alone** —
      `open_companion_manifest`'s call, not `open_companion_equip`'s: this
      page indexes nothing with it and the parked row is what Esc comes
      back to.
- [ ] `handle_companion_memories_key`: Esc clears the subject and returns
      to `Mode::Companion`. Nothing else is bound — it is a page, not a
      menu.

**Tests** (`crates/app-core/src/tests/party.rs`):

- [ ] `R` on the roster opens the mode with the **highlighted** program as
      the subject, not the first one. (Field two programs and highlight the
      second, or the test passes against a hardcoded index.)
- [ ] Esc returns to `Mode::Companion` and clears `pending_memory_program`.
- [ ] Esc leaves the roster's highlight where it was.
- [ ] `R` with an empty roster changes no mode and panics on nothing.
- [ ] `R` does not disturb `pending_equip_program`, and opening the equip
      page does not disturb `pending_memory_program` — the two subjects are
      independent.

**Verification:**

- [ ] `cargo test -p feral-processes-app-core party`
- [ ] Mutation table.

---

## Task 3 — The page

**Files:** `crates/gui/src/render/party.rs`, `crates/gui/src/render/mod.rs`

**Interface:** `draw_companion_memories(game, program, painter, m)`, and a
row builder `memory_page_rows(game, program) -> Vec<Row>` **split out**, so
the two censuses in task 4 can measure the real page rather than a fixture
of it — `gear_inspect_rows` is the precedent and the reason.

**Steps:**

- [ ] Build the page: a title, the program's name, the **derived Morale
      figure** as the header (`Game::morale`), then one entry per row —
      the def's name, the subject where there is one, the intensity, the
      age. The blurb goes under it. Then the Esc footer.
- [ ] The gone-program guard `draw_companion_equip` already has: a subject
      that no longer names a live program draws a short popup saying so
      rather than an empty page.
- [ ] A program with an empty store gets a line saying so, not a blank box.
      This is also what the deleted-`assets/memories/` install draws.
- [ ] Dispatch the mode in `render/mod.rs`.
- [ ] Add the help line to `companion_help()` — the array grows to 6.
      **It must not contain a capital `W`**: the easter-egg census forbids
      the letter anywhere in those lines, and that constraint is working as
      designed, not an accident to route around. Follow the bare
      `R ...` style of the four lines above it.

**Tests** (`crates/gui/src/render/party.rs`'s test module):

- [ ] `the_companion_screen_names_the_memories_key` — the roster's help
      names `R`, mirroring the three key censuses beside it. Its doc
      comment must say *why this key has to be advertised*, as those three
      do.
- [ ] The page carries the morale figure, and a program whose memories net
      negative shows a different figure from one that nets positive. (A
      test that only checks a number is *present* passes against a
      hardcoded zero.)
- [ ] A row names its def and its subject; a `Nothing`-subject row names
      the def and nothing else.
- [ ] The blurb reaches the page.
- [ ] An empty store draws the says-so line and no entry rows.
- [ ] The existing easter-egg census still passes with six lines — it will,
      but it is the one that would go quietly if `companion_help`'s arity
      change were made by widening the type instead of adding a line.

**Verification:**

- [ ] `cargo test -p feral-processes-gui party`
- [ ] Mutation table.

---

## Task 4 — The two censuses, one per axis

**Files:** `crates/gui/src/render/party.rs`,
`crates/engine/src/tuning.rs`

**Steps:**

- [ ] `the_tallest_memory_page_fits_its_popup`, mirroring
      `the_tallest_gear_page_fits_its_popup` — **swept across window
      heights** (600..=2160 step 60), not measured at one, because
      `ui_metrics` clamps the font at both ends and the tightest window is
      the smallest one. The tallest page is a program holding
      `MEMORY_CAP_PER_PROGRAM` entries of the **widest** shipped def, built
      through `Game::remember` against the real catalogue rather than
      hand-assembled — a fixture measures the fixture.
- [ ] The horizontal census: no row on the tallest page overflows a
      `PopupSize::Large` body, measured with `with_painter` /
      `measure_ui_advance` the way
      `the_widest_shipped_routine_kit_fits_the_fuse_picker` does. The
      widest case is a `Program` subject carrying a renamed program at
      `MAX_CUSTOM_NAME_LEN` — use it, not a short fixture name.
- [ ] Both must fail if the cap is raised: assert against
      `MEMORY_CAP_PER_PROGRAM` rather than against a literal 12.
- [ ] Extend `MEMORY_CAP_PER_PROGRAM`'s doc comment in `tuning.rs`: it is a
      layout constraint before it is a feel one, and raising it past what
      fits requires giving the page a scroll first.

**Verification:**

- [ ] `cargo test -p feral-processes-gui memory`
- [ ] Mutation: raise the cap to a number that does not fit, watch both
      censuses fail, restore.

---

## Task 5 — Docs, and the whole gate

The spec's documentation obligations cover phases 1–4 and none of them has
been written yet; they land here rather than being carried to phase 5,
which appends one line to what this task writes.

**Steps:**

- [ ] `docs/seams.md` — the argument for each: the one door
      (`Game::remember`), intensity derived rather than ticked, the
      `BaseTile` space tag, and this page being one derivation with no
      scroll.
- [ ] `CLAUDE.md` — a **Memories** subsection under "Load-bearing seams",
      one or two lines each, in the rule-and-the-trap-it-closes voice.
      Then `cp CLAUDE.md AGENTS.md` — they are gitignored twins with no
      tracking to catch drift.
- [ ] `assets/memories/README.md` — check it against what the screen now
      draws. `name` and `blurb` have readers for the first time, and the
      README should say where each one lands.
- [ ] `CHANGELOG.md` is **not** touched: the version bump and its section
      happen once at the merge, per the one-release-per-change rule.
      `docs/manual.md` and the root `README.md` are carved out and stay
      stale.

**Verification:**

- [ ] `cargo fmt`
- [ ] `cargo clippy --workspace` — clean
- [ ] `cargo test --workspace` — green, and the count has grown by the
      tests this plan adds and by nothing else
- [ ] `cargo test -p feral-processes-engine balance_sim` — **unmoved**.
      Nothing here reaches a figure the simulator models; a moved curve
      means something was changed that shouldn't have been.
- [ ] The mutation table for tasks 1–4, collected in one place.
