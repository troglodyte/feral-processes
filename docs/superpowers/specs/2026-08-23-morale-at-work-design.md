# Morale at work

**Status:** approved, not implemented
**Date:** 2026-08-23

Programs form memories about the work they do and the machines they are
posted to, and morale — the signed sum of everything a program remembers —
quietly changes how reliably it extracts. One dial, four new memory kinds,
and the two subject kinds that shipped with nothing writing them get their
first writers.

## Why

`assets/memories/` shipped in 0.13.7 with the substrate complete and the
content deliberately withheld. What is live today: four memory kinds, four
triggers, a screen, and **one** behaviour hook — `park_idle_staff` will not
stand an idle body on a tile it holds a grudge against.

That leaves two gaps the substrate was built to close and does not.

**`Game::morale` has no reader outside the screen that prints it.** The
memories page heads itself with a figure that changes nothing. A player who
opens it learns that a program is unhappy and can do nothing with the fact,
which makes the whole page a readout of a simulation nobody is running.

**Two of the six `MemorySubject` kinds have no writer.**
`MemorySubject::Structure` and `MemorySubject::Activity` are supported end
to end — variants, display names, save encoding — and nothing in the game
writes either. The entity-memories spec shipped them that way on purpose
("`Structure` and `Activity` ship as subject kinds with nothing writing them
yet"), because the four shipped kinds were chosen to cover both valences and
every subject kind that *had* a trigger.

A program's working life is the obvious source of both. A body spends the
run at a machine, on a job, in a base that sweeps and jams — and remembers
none of it.

## What is in scope

- A `morale` term in `systems::mining_success_chance`, entering through a
  fourth field on `CycleModifiers`.
- Two `tuning.rs` constants: the coefficient, and a cap on the term.
- `Game::note_postings` — a periodic pass beside `note_strandings` that
  remembers what a posted program is doing and where.
- One edge trigger in `Game::damage_structure`.
- Four new `assets/memories/*.ron` defs.
- The two memories-page censuses re-run against an eight-kind catalogue.

## What is out of scope

- **Assemblers and diggers.** `mining_success_chance` is reached only by
  `resolve_gather_cycle`, so the dial moves extraction and nothing else.
  Widening to `assembler_system` or `run_dig_crew` is a second change,
  taken only if this one reads as anything.
- **Anything `opinion_of` drives.** The new memories are read through
  `morale`, in aggregate. A per-machine or per-job preference is a
  different feature and lands on `schedule_base_labour`'s no-sort rule,
  which the entity-memories spec ruled out and this one does not reopen.
- **Any log line.** `Game::remember` writes none by design and this adds
  none. The hook is as quiet as the parking hook, and the memories page
  stays the one surface that explains it.
- **Battle.** `bonded_in_battle` and `mauled_by` still change nothing about
  a fight. That is the other deferred hook and it is still deferred.
- **A save-format change.** Nothing new is stored. The new subjects already
  encode, and `SAVE_FORMAT_VERSION` does not move.

## Design

### 1. The dial

`CycleModifiers` gains a fourth field:

```rust
pub(crate) struct CycleModifiers {
    pub keen_scavenger_level: u32,
    pub base_int: i32,
    pub class: Option<AffinityClass>,
    pub morale: f32,
}
```

That struct's doc comment already says what it is for — "what the *worker*
brings to a gather cycle, as opposed to what the node is" — and already
argues why its fields stay separate rather than collapsing into one number:
they differ in whose they are and in what they touch. Morale is a fourth
thing the worker brings, belonging to whoever is standing at the machine,
deciding whether the cycle lands rather than what a landed cycle is worth.
It sits beside `base_int`, which is the field it behaves most like.

`mining_success_chance` gains a fourth term:

```rust
pub(crate) fn mining_success_chance(
    level: u32,
    keen_scavenger_level: u32,
    base_int: i32,
    morale: f32,
) -> f64
```

The term is `(morale * MEMORY_MORALE_PER_POINT).clamp(-MAX, MAX)`, added
alongside the others and inside the existing `clamp(0.0, 1.0)`.

**Signed and symmetric.** A program that remembers good things extracts more
reliably; one that remembers bad things fizzles more; the sign of the sum is
the sign of the term. This matches `base_int`'s term, which is also signed
around a baseline, and it is what gives the positive kinds something to do:
a catalogue where only grudges bite makes every memory a liability.

### 2. The two guardrails

**Zero at the baseline.** This is the same idiom `base_int` and
`work_ticks_at_speed` both use, and its doc comments already state the
reason: a term read as a deviation means a species file, a mod, or a save
that never heard of the field extracts at the rate it always did. Making it
absolute silently re-rates everything by wiring alone.

Here it buys three properties at once, all for free:

- A program with no memories contributes exactly `0.0`.
- **The player** working a node themselves contributes exactly `0.0` — the
  player has no `Memories`, and `player_gather_system` passes the baseline
  the same way it already passes `DEFAULT_BASE_INT` and `class: None`.
- **`assets/memories/` deleted** contributes exactly `0.0` at every worker,
  because `memory_sum` skips every entry whose def no file defines. That is
  the empty-catalogue property holding at a third site without a line of
  code spent on it.

**A cap on the term, not just on the result.** `morale` is a sum of up to
`MEMORY_CAP_PER_PROGRAM` entries and is unbounded above and below.
`mining_success_chance` already clamps its *result* to `0.0..=1.0` because
`GameRng::random_bool` panics outside it — that is a different job. Without
a cap on the contribution, a bad run drives a node's reliability to zero and
the base stops producing, which reads as the base being broken rather than
as a program being unhappy. `MEMORY_MORALE_MAX_SHIFT` is what keeps this a
texture rather than a difficulty knob keyed to run history.

Both constants go in `tuning.rs` beside the five `MEMORY_*` constants
already there. Neither is ever scaled by level, zone or depth, for
`effective_mitigation`'s reason: a term that grows with the player
approaches the cap and stops meaning anything.

### 3. Where memories about work come from

Four triggers. One is an edge; three ride a period.

| def | valence | subject | formed by |
|---|---|---|---|
| `settled_in` | + | `Structure` | holding a posting at a machine that is not jammed |
| `jammed_here` | − | `Structure` | holding a posting at a machine that is `Clogged` |
| `cutting_rock` | − | `Activity` | holding a `TaskKind::Excavate` posting |
| `swept_here` | − | `Structure` | a GC Entropy Sweep damaging the machine you are posted at |

**`MemorySubject::Structure` names the *kind*, not the entity.** It carries
a `StructureId`, which is `Structure::kind`. So a program comes to like or
dislike Lathes in general, not one particular Lathe, and a machine that is
destroyed and rebuilt is remembered as the same thing. That is the right
fiction for a base whose structures are demolished and re-deployed, and it
is what makes the memory outlive the entity without a buyback-shelf-style
`(kind, tile)` key.

**A digger gets the `Activity` memory and no `Structure` one.** A
`DigSite` is the one `Task` target that is not a structure — it is the
second non-`Structure` entity carrying a base-space `Position` — so there is
no `StructureId` to name. Cutting rock is remembered as a kind of work and
not as a place, which is also what makes the memory follow the program
rather than the hole.

**`settled_in` and `jammed_here` are the same subject with opposite signs**,
so a machine kind that mostly runs and occasionally jams nets out to a mild
fondness, and one that spends its life clogged nets out to a grudge. They
are two defs rather than one signed def because valence is a property of the
kind in this schema, and because a mod may want to retune them apart.

### 4. The period, and why this is not a per-tick write

`Game::note_postings` runs from `tick_inner` immediately after
`self.schedule.run(&mut self.world)`, beside `note_strandings`, and for that
call's stated reason: the base systems' commands have just flushed and the
clock has not yet moved.

It fires only when `tick % MEMORY_POSTING_PERIOD == 0`.

**The period is load-bearing and `note_strandings` is why.** That function's
doc comment already settles this argument:

> a per-tick write instead would saturate `strike_cap` in three ticks and
> hold the grudge at full intensity for as long as the route stayed broken,
> which makes `strikes` mean nothing

A posting is a *standing* state, not an edge — unlike a stranding, which
`Stranded::since` makes edge-readable, there is nothing to distinguish the
first tick at a machine from the thousandth. So the period is what stands in
for the edge: it is what makes `strikes` count stretches of service rather
than ticks, and it is what stops a body pinning every memory at
`strike_cap` and holding it there.

It also keeps eviction lazy. `remember` evicts at the tail of every write;
a per-tick writer would make eviction effectively eager for any program
holding a posting, so a posted program would forget faded things promptly
while an idle one kept them — a difference in what a program remembers based
on whether it happened to be working, which nothing in the design wants.

Derived from `GameClock`, so there is no counter, no field on `Task`, and
nothing to save. The pass reads:

- every entity holding a `Task`, collected before anything is written
  (`remember` takes `&mut self`, which is `note_strandings`' and
  `form_victory_memories`' constraint too);
- for a `Task` whose target carries a `Structure`: `settled_in` or
  `jammed_here`, chosen on the target's `MachineStatus`;
- for a `Task` of kind `Excavate`: `cutting_rock`.

**`swept_here` is the one edge**, and it needs no period because a raid is
an event. `Game::damage_structure` already collects the workers posted at
its target — today only on the destroyed branch, to clear their cronjobs.
The trigger wants both branches: being hit under you and being destroyed
under you are the same memory, and only the second is currently visible to
that function.

### 5. The catalogue

Four `.ron` files, taking the shipped catalogue from four kinds to eight.
Every field is one the schema already has; `assets/memories/README.md`'s
"shipped kinds" table gains four rows.

**The constraint on this section is layout, not mechanism.** The memories
page has no scroll and nothing clips a row horizontally, so a `name` or
`blurb` too long is lost off the edge, taking the strength and age figures
with it. Two censuses measure the worst page the catalogue can build:
`the_tallest_memory_page_fits_its_popup` and
`no_memory_row_overflows_its_popup`. Doubling the catalogue doubles the
number of distinct kinds one program can hold, so both get harder to pass —
which is the censuses working, but it means the copy is constrained rather
than free, and a def authored too long fails the build.

Those two censuses live in `crates/gui/src/render/party.rs`, so this change
is engine-only in code and reaches a gui test through content. That is worth
knowing before running `cargo test -p feral-processes-engine` and believing
it.

Half-lives should sit in the same band as `stranded_at`'s 3000: these are
impressions built over a run, not shocks.

## Testing

TDD, a commit per green step. `cargo test --workspace` is the final gate.

**The dial**

- `mining_success_chance` at `morale: 0.0` returns exactly what it returns
  today, for a spread of levels and `base_int`s. This is the whole
  no-regression argument and it is asserted against the function directly.
  Most of it is already written: the function has one production caller and
  ten existing unit tests in `systems.rs`, every one of which pins a
  hardcoded curve or an ordering. Passing `0.0` to all ten and watching them
  stay green **is** the evidence that nothing moved, so they are updated
  rather than replaced, and a new test covers only the morale axis itself.
- The term is symmetric: equal and opposite morale moves the chance equally
  and oppositely, until the cap.
- The cap holds. A deliberately miserable program — morale far past
  anything the shipped catalogue can reach — shifts the chance by exactly
  `MEMORY_MORALE_MAX_SHIFT` and no further, and the result stays inside
  `0.0..=1.0`.
- The player working a node passes `0.0`, asserted at the
  `player_gather_system` call site rather than by reasoning about it.
- **With `assets/memories/` unavailable, extraction is unchanged.** The
  empty-catalogue property, asserted the way the existing ones are.

**The triggers**

Each of the four gets a test, and each is **mutation-proved**: delete the
trigger, watch the test fail, restore. A test that passes with the fix
removed is coverage-shaped and this repo has shipped two of those.

- A program posted at a machine across a period holds a `settled_in` memory
  naming that machine's kind — and one posted for less than a period holds
  none, which is what proves the period is doing something.
- A program at a `Clogged` machine holds `jammed_here` and not `settled_in`.
- A program on a `TaskKind::Excavate` posting holds `cutting_rock` and
  **no** `Structure` memory, since a `DigSite` is not a structure.
- A sweep that damages but does not destroy a machine forms `swept_here`
  in the worker posted there. Damage-not-destroy specifically, because that
  is the branch `damage_structure` does not currently look at workers on.

**The catalogue**

- Both memories-page censuses re-run against eight kinds.
- The existing shipped-catalogue censuses (non-zero valence, `strike_cap`
  at least 1) cover the new files for free.

**Fixture traps**, from `docs/seams.md` and prior sessions: a test that
hand-spawns a work node needs `work_node_parts()`, and one that posts a
program needs `park_at_post()`. Both omissions read as the feature not
working rather than as the fixture being short something.

## Documentation obligations

- `assets/memories/README.md` — four rows on the shipped-kinds table. The
  schema itself does not change.
- `CHANGELOG.md` — a `## 0.13.x` section, and the version bump, at the
  merge rather than on the branch.
- `CLAUDE.md` and `docs/seams.md` — the "What a program remembers" entry
  currently says "**The one hook is `park_idle_staff`'s third rejection, and
  it is not a score.**" That becomes false the moment this lands. Both files
  need the second hook named, and the reasoning — the baseline-zero idiom,
  the capped term, the period — belongs in `docs/seams.md` rather than in
  `CLAUDE.md`.
- `docs/manual.md` and the root `README.md` are carved out by standing
  practice; leave both.

## Decisions taken, so they are not relitigated

**No drain queue, and no new `Resource`.** The first shape of this design
had `task_progress_system` and `set_machine_status` — both bevy code with
no `Game` — pushing onto a `RunFeats`-style per-tick queue drained through
`Game::remember`. The post-schedule pass makes the queue unnecessary:
`note_strandings` already establishes that a `&mut Game` pass right after
the schedule can read what the base systems just did. Not building it also
sidesteps a known trap — registering a new resource shifts bevy's query
iteration order and has surfaced latent unsorted-query failures here before.

**Not fired on cycle completion.** Reinforcing `settled_in` every time a
cycle lands would make `strikes` a cycle count, which saturates
`strike_cap` in seconds at a fast machine and never at a slow one — so the
same length of service would mean different things at different machines.
The period measures time served, which is what the memory claims to be
about.

**No new field on `Task`.** A "posted since" tick would make the stretch
directly measurable, and would be a saved field on a component that has
four bare fields today. The period gets the same behaviour derived, which
is `Platform`'s radius, a program's role, a Broker's board and a Stack
description all following the same instinct.

**Reliability rather than cycle length.** `work_ticks_for` reaches
extractors *and* assemblers in one place, which is broader — but it writes
`Task::required` once when the job is assigned, so the figure freezes at
post time and goes stale on a body that never moves. Morale is a slow
quantity and a posting can outlast several half-lives.

**Aggregate `morale`, not `opinion_of`.** A program's feeling about *this
machine* driving its work at *that machine* is the more obvious design and
is deliberately not this one: it is a per-machine preference, and the moment
anything acts on it, it wants to be in the posting decision, which is the
seam this feature is under orders to stay out of.

**Nothing gates this numerically.** `balance_sim` models no base production
— no `node_payout`, no `resolve_gather_cycle`, no cycle length — so the
balance regression suite is blind to this change in both directions. The
morale-zero tests and the cap are the evidence that the economy has not
moved; there is no curve to check. Said out loud because the suite passing
is not the same claim here that it is for a combat change.

## Phases

One phase. Engine-only in code, no schema change, no save-format change —
CLAUDE.md's process rule puts this at TDD inline with a commit per green
step. The spec exists because it was asked for, not because the size
demands the pipeline.

Order within it: the dial and its two guardrails first (the whole
no-regression argument is testable before any memory is ever formed), then
`swept_here` as the one edge trigger, then `note_postings` with its three,
then the catalogue and the censuses last — where a too-long blurb fails the
build against everything else already green.
