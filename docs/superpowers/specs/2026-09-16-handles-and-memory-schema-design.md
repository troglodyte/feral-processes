# Handles and the memory schema

**Status:** design, approved in brainstorm 2026-09-16. Not implemented.
Sub-projects A and B of `2026-09-16-base-social-roadmap.md`, which this
revises in three places (listed at the end).

## What it is for

The social layer needs two things before anything social can happen. A
program needs a name that is its own and not its species', or no one can
talk about it. And a memory needs to count differently toward what a program
*thinks of* something and toward how it *feels*, and to stack the way
repeated slights stack, or every later sub-project has to hack around the
fold.

**What the player sees:** every owned program has a hex handle like
`0x435eaD`. **Nothing else about the game changes** — every shipped memory
def keeps today's numbers, so morale, mining odds and every seeded test that
does not print a name stay where they are.

## A. Handles

### The derivation

`handles::of(ProgramId) -> String` in a new `crates/engine/src/handles.rs`.

- A fixed permutation on 24 bits — an invertible multiply-and-xorshift with a
  constant salt — maps the id to six hex digits, printed after `0x`.
- A second hash of the id picks the case of each *letter* digit (`a`–`f`), so
  handles read `0x435eaD` rather than all one case.
- **Unique by construction.** The permutation is a bijection on 24 bits and
  `NextProgramId` mints sequentially, so no two ids below 2^24 share digits.
  Case is decoration on top of already-distinct digits and never makes two
  handles equal or unequal. An id at or past 2^24 wraps; that is 16.7M tamed
  programs in one run and is not guarded.
- **No RNG.** It is a pure function of the id.

### Derived, not stored

There is no `Handle` component, no `CreatureSave` field, no asset directory
and no mint at load. The roadmap stored the handle only because a changed
*word pool* would rename everyone; a hex format has no pool.

- A save written before handles gets them on load, because its programs'
  `ProgramId`s already exist (the `0`-sentinel mint in
  `game/lifecycle.rs` covers files older still).
- **Fusion needs no code.** `fuse_companions` already takes `roster_parts()`,
  so the child gets a fresh `ProgramId` — and so a fresh handle — beside the
  fresh `Disposition` it already gets.
- Wild, hostile and summoned bodies carry no `ProgramId` and so no handle,
  without a branch.

**The seam this creates: the salt and the permutation are save format in
all but name.** Changing either renames every program in every existing
save, and nothing fails to compile. A test pins several ids (including `0`,
`1` and a large one) to exact strings; its failure message says why.

**Not moddable, deliberately.** A handle is an identifier format, like the
`ProgramId` under it, not content.

### Where it shows

The ladder is `CustomName`, then handle, then species.

- **`Game::creature_name`** returns the handle for anything carrying a
  `ProgramId` and no `CustomName`. This is the short form.
- **`Game::creature_label`** puts the species after the name when the name is
  a handle: `Overclocked 0x435eaD Scrapper 3`. A `CustomName` keeps today's
  label (the player chose it; the species is on the manifest).
- **A surface with a fixed name cell takes the short form.** The party battle
  roster's `NAME_W` is 18 cells (`gui/src/render/battle.rs`) and already
  carries the tier as its own tag. The first task of the plan is a census of
  every `creature_name`/`creature_label` call site (about 110 across the
  three crates) sorting each into short or long, measured through
  `paint::with_painter` where the text is drawn into a bounded width, not
  counted in characters.
- **Width:** a handle is 8 characters, under `MAX_CUSTOM_NAME_LEN` (12), so
  any surface that fits a custom name fits a handle. The long label is wider
  than today's species label by 9 characters and is what the census checks.

### Test churn

Tests asserting a tamed program's display text by species name will break.
Where a test is really asserting *which species*, it switches to a species
read; where it is asserting the text, it takes `handles::of` of the
program's id rather than a literal hex string. No test hard-codes a handle
except the pinning test above.

## B. The memory schema

Two new `MemoryDef` fields, both `#[serde(default)]`, both defaulting to
exactly today's behaviour. No shipped `.ron` file changes.

### `mood: f32`, default `1.0`

Opinion and morale become two reads of one store.

- `memories::sum_intensity` and `Game::memory_sum` take a
  `Read::{Opinion, Morale}`. **Opinion** counts a record's full felt
  intensity. **Morale** counts felt intensity × `def.mood`.
- `Game::morale` and `task_progress_system`'s `CycleModifiers::morale` read
  `Morale`. Both already call the free function, so they stay one fold.
- `Game::opinion_of` reads `Opinion`.
- **`evict` reads neither dial.** It weighs raw intensity, for the reason it
  ignores `Disposition`: what a program keeps is bookkeeping, not feeling.
- **The memories page (`R`) stays the morale page.** Its rows show the morale
  share, so they still sum to the figure the page heads with. Full opinion
  figures are the SOCIAL tab's (sub-project D).
- **Census:** `mood` is within `[0, 1]`. A `mood` of `0` is refused for a
  subject kind no opinion reader asks about — such a def is worth nothing
  anywhere. Today's readers ask about `Program` (`tantrum.rs`) and `BaseTile`
  (`morale.rs`'s drift rejection and `work_orders.rs`'s `refuses_post`), so
  `Nothing`, `Species`, `Structure` and `Activity` refuse it. The census names
  that list in one constant beside the test, so a new reader updates it.

### `stack_decay: f32`, default `1.0`

Each strike past the first is worth `stack_decay` times the one before.

- In `Memory::intensity_with`, the strikes term `min(strikes, strike_cap)`
  becomes `1 + r + r^2 + … + r^(n−1)` for `n = min(strikes, strike_cap)`.
- `r == 1.0` takes an explicit branch returning `n`, so the closed form
  `(1 − r^n) / (1 − r)` never divides zero by zero, and the default is
  bit-identical to today.
- Shipped at its neutral value, a test cannot tell a formula that honours it
  from one that ignores it — so, as with `stickiness`, the test varies it.
- **Census:** `stack_decay` is within `(0, 1]`.
- The intensity formula in `assets/memories/README.md` is updated, and both
  fields get rows in its table.

## Testing

- `handles::of`: pinned strings; distinct over the first 100,000 ids;
  exactly six hex digits after `0x`; case differs only on letter digits.
- A tamed program, a fused child and a program restored from a pre-handle
  save each read a handle; a wild creature and a summon read their species.
- `creature_label` carries the species after a handle and not after a
  `CustomName`.
- `mood`: a def at `0.5` halves its morale share and leaves `opinion_of`
  whole; `task_progress_system`'s morale matches `Game::morale` for the same
  body (the existing parity test, extended); `evict` ranks by raw intensity
  regardless of `mood`.
- `stack_decay`: `1.0` equals today's figure exactly; `0.5` at three strikes
  gives `1.75 ×` valence; the cap still bounds the sum.
- Asset censuses for both fields over the shipped defs.
- No `GameRng` draw and no `SAVE_FORMAT_VERSION` change; `balance_sim`
  unmoved.

## Revisions to the roadmap

1. **Handles are derived hex codes**, not a stored word pool. `assets/
   handles/`, `CreatureSave::handle` and the load-time mint are gone, and the
   "fusion is a second mint site" note was wrong — fusion already takes
   `roster_parts()`, and its child gets a fresh handle.
2. **`spreads_as` moves to E and `known_for` to D.** In B nothing would read
   either but its own census. The eviction-order census moves with
   `spreads_as`; the `known_for` width census needs D's page to measure.
3. **The social layer need not be moddable.** Where E and G would have added
   `assets/interactions/` and friends, a Rust table is acceptable. B's fields
   still go on `MemoryDef`, because memory defs already are data.
