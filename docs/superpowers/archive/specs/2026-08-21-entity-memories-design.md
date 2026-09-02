# Entity memories

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

Owned programs accumulate memories — positive and negative, each about
*something*: another program, a species, a structure, a tile of the base,
or an activity. A memory fades unless the thing that caused it happens
again. This is groundwork: the substrate, a screen that shows it, and one
behaviour hook to prove the seam end to end. The content that makes
memories matter comes later.

## Why

The game has no model of a program's history with anything. A companion
that has been mauled by Zero-Days a dozen times is mechanically identical
to one that has never seen a fight; a body that has been stranded at the
same node all run treats that corner exactly as it treats any other. The
roster is a bag of stat blocks, and everything the player invests in it is
a number going up.

The reference points are Dwarf Fortress and RimWorld: a pawn's thoughts are
individually legible ("Was beaten badly, −8"), they decay, they compound
when reinforced, and they aggregate into a mood and into opinions of
specific other pawns. That texture is what makes those rosters read as
people rather than as inventory.

This spec builds the substrate for that and stops. Deliberately: the
interesting content questions — should a resentful program refuse a post,
should a bonded pair fight better together, should a scar cost accuracy —
each touch a load-bearing seam (`schedule_base_labour`'s no-sort rule,
`Stats`' gear-bonus guards, `balance_sim`'s regression gate). Answering
them costs more than building the thing they all need, and answering them
*before* the substrate exists means answering them blind.

## What is in scope

- A data-defined catalogue of memory kinds under `assets/memories/`.
- A per-program store, with intensity derived from the game clock.
- A stable per-program identity, so a memory can be about one specific
  program and survive a save/load.
- One door that writes memories, two that read them.
- Four shipped memory kinds and their four triggers.
- A screen listing a program's memories, headed by a derived Morale figure.
- One behaviour hook: an idle program avoids parking on a base tile it has
  a strong negative memory of.

## What is out of scope

- Memories on anything other than an owned program. Wild and hostile
  programs live minutes and despawn; the player is not an owned program.
- Any effect on `Stats`, damage, accuracy, or any other figure
  `balance_sim` models. This feature is invisible to the balance gate, and
  that is a property to preserve, not an oversight.
- Places outside the base. An owned program only persistently inhabits
  base space; surface and Stack places are a second variant plus a
  zone-local wipe, added when content asks for them.
- Refusing a post, preferring a machine, or any other change to
  `schedule_base_labour`.

## Design

### 1. The catalogue

`assets/memories/*.ron` is a content directory in the mould of
`assets/talents/` and `assets/nemesis/`: `MemoryDb::load_dir` walks it, and
a malformed file is skipped with a logged warning rather than aborting
startup, per file rather than per directory.

**An empty database is valid and inert.** Deleting `assets/memories/`
restores the pre-memory game exactly — `remember` becomes a no-op, the
screen draws nothing, the parking hook never fires. That is the same
supported way to play that deleting `assets/sectors/` or
`assets/policies/enemy_battle.ron` is, and it is the cheapest possible
proof that the feature is additive.

One def per memory kind:

```ron
(
    id: "stranded_at",
    name: "Left stranded here",
    blurb: "Nothing ever reaches this corner.",
    valence: -6.0,
    half_life: 3000,
    subject: BaseTile,
    strike_cap: 3,
)
```

- `valence` is signed and is the magnitude at one strike, undecayed.
- `half_life` is in ticks: how long until intensity halves.
- `subject` declares which `MemorySubject` kind this def expects. It is
  what the screen renders against and what `remember` validates.
- `strike_cap` is how far reinforcement compounds before it stops.

Every field added to `MemoryDef` later must be `#[serde(default)]`, per the
standing rule for `SpeciesDef`/`StructureDef`/`ItemDef`/`AbilityDef`, so a
mod's files keep parsing untouched.

`assets/memories/README.md` is the schema reference and ships with the
directory.

### 2. The record

```rust
pub struct Memories(pub Vec<Memory>);

pub struct Memory {
    def: MemoryId,
    subject: MemorySubject,
    reinforced: u64,   // GameClock tick
    strikes: u32,
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

**Intensity is derived, never stored.**

```
valence * min(strikes, strike_cap) * 2^-((now - reinforced) / (half_life * MEMORY_HALF_LIFE_MULTIPLIER))
```

`resources::GameClock { tick: u64 }` already exists and is already saved,
so this costs no new clock and no decay system. Nothing ticks, nothing
oscillates, reinforcement is a single field write, and a stored weight
cannot drift out of step with the clock the way a per-tick decrement can.
It is the same instinct `Platform`'s radius, a program's role, a Broker's
board and a Stack description all follow.

**The variant is `BaseTile`, not `Place`.** Base space and surface space
are the same two integers meaning different things, and `docs/seams.md`
records what reading one as the other did — it put the base's roster on the
open grid. Naming the space in the type is what stops that recurring here.
When surface or Stack places are wanted, they arrive as their own variants,
and the surface one is zone-local and must be wiped by name in
`enter_next_zone` alongside `StackMemory`, `BuybackLedger` and
`PopulatedChunks`.

### 3. Identity

A memory about one specific program needs an identity the save can hold,
and `Entity` is written to the save nowhere — `save.rs` resolves everything
by position or by index, because entity ids are not stable across a round
trip.

So: `components::ProgramId(u32)`, minted from a saved
`resources::NextProgramId`, assigned at `Game::roster_parts()`.

`roster_parts` is the documented single barrier all four doors into the
roster pass through — `grant_starting_program`, a capture, `adopt_program`,
and `fuse_companions`, which hand-writes its own component list. Widening
its return tuple from three to four fails to compile at every call site,
which is the entire reason that function exists rather than four hand-built
tuples. It becomes `&mut self`, since minting advances the counter; all
five call sites already hold one.

Save changes, all additive behind `#[serde(default)]`:

- `CreatureSave::program_id: u32`
- `CreatureSave::memories: Vec<MemorySave>`
- the top-level save's `next_program_id: u32`

`MemorySave` is a named struct, never a positional tuple — a tuple is the
one shape field-named RON does not save you from, and the next property
added to a memory would cost a legacy field. It carries the def id, the
subject, `reinforced`, `strikes`, and — for a `Program` subject only — the
remembered display name, since the program it names may be gone by the time
the screen draws it.

Id `0` is the unassigned sentinel, so real ids start at 1. A save written
before this feature loads with every creature at 0; the load path mints a
fresh id for each and sets the counter past the highest it saw.

**No `SAVE_FORMAT_VERSION` bump.** The save is field-named RON, and an
additive field behind a default costs no version bump. Nothing is removed
and no field changes meaning under a name it keeps.

### 4. Formation — one door

`Game::remember(who, def_id, subject)` is the only place a memory is
written, on the model of `Game::apply_damage` being the only path that
damages a creature. A rule that must see every memory goes here.

- **A `who` with no `Memories` component is a no-op.** This is the same
  deliberate asymmetry `spend_power` uses for a missing `PowerReserve`: it
  is what keeps hostiles, structures and the player safe without a branch
  at every call site.
- A subject whose kind does not match the def's declared `subject` is
  **refused with a warning**. Fail fast; this is a programming error, not
  a game state.
- Forming versus reinforcing: an existing entry with the same
  `(def, subject)` takes `strikes += 1` (saturating at `strike_cap`) and
  `reinforced = now`. Otherwise a new entry is pushed.
- Eviction is lazy and happens here: entries whose derived intensity is
  under `MEMORY_FORGET_THRESHOLD` are dropped, then the weakest is dropped
  while the list is over `MEMORY_CAP_PER_PROGRAM`.
- **It draws no RNG at all.** No `GameRng`, no local `StdRng`. So no
  seeded test moves, no `dev-arenas/` report shifts, and none of the
  RNG-stream-shift diagnostics apply to anything this feature breaks.

**Formation logs nothing.** The screen is the surface. A line every time a
machine strands a body would flood the map's log pane and drag the fold,
filter and reveal seams into a feature that does not need them. Announcing
memories is a `MessageKind`/`MessageSource` decision to make deliberately
later, not to acquire by default.

### 5. The four shipped kinds and their triggers

| id | valence | subject | trigger |
|----|---------|---------|---------|
| `bonded_in_battle` | + | `Program` | surviving a won fight forms/reinforces a memory of each *other* surviving party member that is a program; the player carries no `ProgramId` and is skipped |
| `mauled_by` | − | `Species` | taking a single hit above `MEMORY_MAUL_FRACTION` of max HP, about the attacker's species |
| `stranded_at` | − | `BaseTile` | a worker marked `Stranded`, about the tile it was posted to |
| `hard_won` | + | `Nothing` | winning a fight whose hostiles' summed `Stats::power()` exceeded the party's at the opening round |

They are chosen to exercise every subject kind that has a trigger and both
valences, not because these four are the interesting content. `Structure`
and `Activity` ship as variants with no trigger yet; that is deliberate —
they are the two subjects future content most obviously wants, and a
variant with no writer costs nothing while an enum that has to grow costs a
migration.

**The triggers are Rust, and the catalogue is data.** That puts memories on
the same half-data seam as `perks::Perk`: the catalogue crossed over,
the hooks did not. This is deliberate and should not be "fixed". A data
trigger vocabulary — the shape `assets/achievements/` has — earned its
place there because the four `Trigger` variants were known to be the whole
vocabulary. Inventing one from four samples is exactly the speculative
abstraction the code principles forbid. When content asks for a fifth and
a sixth trigger the shape will be visible; until then a new memory kind is
a `.ron` file plus a hook, and `MemoryDef` deliberately has no `trigger`
field.

### 6. Readers

- `Game::morale(entity) -> f32` — the sum of every memory's current
  intensity.
- `Game::opinion_of(entity, subject) -> f32` — the same sum restricted to
  memories about one subject.
- `views::MemoryRow` and `Game::memory_report(entity)` — what the screen
  draws.

Nothing else reads memories in this pass. The parking hook calls
`opinion_of`; the screen calls the other two.

### 7. The screen

`Mode::CompanionMemories`, reached with `M` from `Mode::Companion`. That is
exactly the precedent `Mode::CompanionEquip` sets with `E` from the same
screen: one program's detail page opens a sub-page for one axis of it.

The header is the derived Morale figure. Each row is the def's name, its
subject rendered, its current intensity, and its age.

**`MEMORY_CAP_PER_PROGRAM` is a layout constraint before it is a feel
one.** `draw_popup` pages a `Row::Item` span, and a page with no such span
drops any row past the bottom in silence — the trap
`the_tallest_gear_page_fits_its_popup` exists to catch on the gear inspect
page. The cap is set so the tallest possible page fits, and a mirror test,
`the_tallest_memory_page_fits_its_popup`, says so. Raising the cap past
what fits requires giving the page a scroll first.

Subject rendering is an exhaustive match on `MemorySubject`, `cell_mark`'s
rule: a new variant must fail to compile rather than shipping invisible. A
`Program` subject whose id no longer names a living program renders as the
program's remembered name — which means `MemorySave` carries the name
alongside the id, since the program may be gone.

### 8. The one hook

`park_idle_staff` (`game/base/work_orders.rs`) gains a third rejection
beside the two it already applies — a tile a `Structure` stands on, and a
tile `BaseGrid` says is not walkable:

```
opinion_of(worker, BaseTile { x, y }) < MEMORY_AVOIDANCE_THRESHOLD
```

A rejected candidate means the program holds its ground for that beat,
which is already that function's documented behaviour for the other two
rejections. So this opens no new failure mode and needs no new fallback.

It is `park_idle_staff` and not `schedule_base_labour` on purpose. The
scheduler is documented as deciding the whole assignment by priority and
then diffing it, with deliberately no sort and no score; a memory term
there is a score, and it would also put a memory in the path of the
anti-thrash rule and the never-free-a-`Carrying`-holder rule. "Doesn't want
to work" belongs there eventually, as content, once the substrate is proven
and the interaction can be designed rather than bolted on.

### 9. Tuning

`crates/engine/src/tuning.rs` gains a labelled section:

- `MEMORY_HALF_LIFE_MULTIPLIER: f32` — the global stickiness dial. One
  number makes every grudge in the game longer or shorter.
- `MEMORY_CAP_PER_PROGRAM: usize`
- `MEMORY_FORGET_THRESHOLD: f32`
- `MEMORY_AVOIDANCE_THRESHOLD: f32`
- `MEMORY_MAUL_FRACTION: f32`

Per-def half-lives stay in the `.ron`. A scar outlasting a bad shift is a
content decision about those two memories; how sticky memory is in general
is a tuning decision, and the split follows the standing rule that content
is data and how hard the game is, is not.

## Testing

`balance_sim` is untouched, and the spec says so rather than leaving it
inferred: nothing here reaches `Stats`, damage, or any figure the simulator
models. A curve that moves after this lands is not this feature.

- **Decay.** Intensity is exactly half at `half_life` ticks, exactly a
  quarter at two.
- **Reinforcement.** A second `remember` of the same `(def, subject)`
  raises strikes, resets the clock, and does not add a second entry;
  strikes saturate at `strike_cap`.
- **Eviction.** A faded entry is dropped at the next formation; over the
  cap, the weakest goes and the strongest survives.
- **The no-op.** `remember` on a hostile, a structure and the player each
  leave no trace and do not panic.
- **The kind mismatch.** A `Species` subject against a `BaseTile` def is
  refused and warns.
- **Save round trip.** A save→load test, not only a RON round trip: a RON
  round trip cannot catch a `#[serde(skip)]` and would stay green against
  a field that never reaches the file.
- **Legacy load.** A save with no `program_id` mints fresh ids on load,
  assigns no duplicates, and leaves the counter above every id seen.
- **The empty database.** `MemoryDb::default()` makes `remember` a no-op,
  the screen empty and the hook inert — the deleting-`assets/memories/`
  property, asserted.
- **Asset census** in `tests/assets.rs`: unique ids, signed non-zero
  valences, positive half-lives, `strike_cap >= 1`, and every def's
  declared `subject` kind reachable from a real `remember` call site.
- **The popup fit.** `the_tallest_memory_page_fits_its_popup`, mirroring
  the gear page's.
- **The hook.** A worker with a strong `stranded_at` memory does not park
  on that tile and does park on the next candidate; a worker with a weak
  one does park there.

Every new test is mutation-proved: the fix is deleted, the test is watched
to fail, the fix is restored. A test that passes with its fix removed is
coverage-shaped and worse than nothing.

Full gate: `cargo test --workspace`, `cargo clippy --workspace`,
`cargo fmt`.

## Documentation obligations

- `assets/memories/README.md` — the schema reference, shipped with the
  directory.
- `CHANGELOG.md` — its own version section at the merge.
- `CLAUDE.md` and `AGENTS.md` — a load-bearing seams entry for the one
  door, the derived intensity, and the `BaseTile` space tag. They are
  gitignored twins with no tracking to catch drift: edit `CLAUDE.md`, then
  copy it over `AGENTS.md`.
- `docs/seams.md` — the argument behind each of those rules, under the same
  titles.
- `docs/manual.md` and the root `README.md` are carved out and stay stale.

## Decisions taken, so they are not relitigated

1. **Owned programs only.** Wild and hostile programs live minutes and
   despawn; a memory on one is never read. The player carries none either —
   the player is not an owned program, and morale on the player is a
   different feature.
2. **Fades unless reinforced**, with the rate tunable. Not permanent
   scars, not plain decay.
3. **Intensity is derived from `GameClock`**, not decremented by a system.
4. **The catalogue is data; the four triggers are Rust.** Half-data, on
   `Perk`'s seam, until a trigger vocabulary is actually known.
5. **Formation logs nothing.** The screen is the surface.
6. **The one hook is `park_idle_staff`.** Quiet but real, and it touches
   no balance seam and no scheduler rule.

## Phases

Five phases, each with its own green gate and its own reviewer boundary.
They are **branch commits, not separate releases** — the version bump, the
`CHANGELOG.md` section and the tag happen once at the merge, per the
one-release-per-change rule. Phase 1 therefore lands a `ProgramId` that
nothing reads yet; that is a phase boundary, not a shipped dead field.

| Phase | Deliverable | Touches |
|---|---|---|
| 1 | **Identity** — `ProgramId`, `NextProgramId`, `roster_parts` widening, the two save fields, minting on a legacy load | engine, save |
| 2 | **Substrate** — `MemoryDb`, `assets/memories/` + README, `Memories`/`Memory`/`MemorySubject`, derived intensity, `remember`, `morale`, `opinion_of`, the tuning section | engine, assets |
| 3 | **Triggers** — the four shipped defs wired to their four hooks | engine |
| 4 | **Screen** — `Mode::CompanionMemories`, `views::MemoryRow`, the render page, the popup-fit test | engine, app-core, gui |
| 5 | **The hook** — `park_idle_staff` avoidance | engine |

Each phase is planned when it is reached rather than all five up front: a
plan exists to hand context to an executor, and four plans written against
a substrate that does not exist yet would be written twice.
