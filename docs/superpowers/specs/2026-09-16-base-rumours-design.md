# What the base knows about itself: handles, bonds and rumours

**Status:** design. Not implemented.

Three changes that turn the memory system's existing program-to-program
opinions into something the player can read and the base can act on: every
owned program gets a **handle** so it is somebody, an opinion between two
programs gets a **band** so it has a word and a consequence, and a memory
**spreads** to programs that were not there.

The game already has two of Dwarf Fortress's three story-generating
properties — the same event lands differently per individual
(`disposition.rs`), and the acting-out ladder has no damper
(`components::Grievance`). It has none of the third: a memory is written only
to whoever was there. `docs/superpowers/specs/2026-09-16-dwarf-fortress-social-survey.md`
is the survey this came out of.

## The load-bearing decision

**The hop cap is the chain length in the data.**

A spreadable `MemoryDef` names, in a new `spreads_as:` field, the weaker def
it propagates as. That def names nothing, so it does not spread further. One
hop, expressed entirely in `assets/memories/`, with no hop counter, no
propagation state and no per-record field. A modder who wants three hops
writes three defs.

Two dampers come free with it and neither had to be designed:

- **`strike_cap` bounds the compounding.** A firsthand memory re-spreads every
  period for as long as it is held, and the hearsay it writes saturates at its
  own cap rather than climbing.
- **Eviction already favours firsthand.** `remember`'s tail eviction is lazy
  and compares **magnitude at both steps**, and a hearsay def is authored at a
  smaller magnitude than the def it copies — so when a store reaches
  `MEMORY_CAP_PER_PROGRAM` it drops what a program *heard* before what it
  *saw*. That is the property the census in "Censuses" exists to hold.

## Handles

**`components::Handle(String)`, minted once at `Game::roster_parts`.**

`creature_name` (`game/party.rs`) gains a middle rung: `CustomName`, then
`Handle`, then the species name. A player rename still wins, and nothing wild,
hostile or hand-built has a handle, which falls out of `roster_parts` being
the only mint — `ProgramId`'s rule, read the same way.

**The second mint site is `fuse_companions` and it will not fail to compile.**
There are four doors into the roster and `roster_parts` is the barrier for
three of them; fusion hand-writes its own component list, which is the
documented way a new component silently goes missing. A fused program with no
handle reads as fusion being broken, so fusion takes the dominant parent's
handle explicitly, beside the ring and the talents it already keeps.

**The pool is `assets/handles/`**, one `.ron` file of strings, loaded on
`MemoryDb`'s terms: an absent directory loads silently empty, and an empty
pool mints no handle, so deleting it restores the pre-handle game rather than
breaking one. No system and no screen may be gated on the pool being
non-empty.

**Save:** `CreatureSave` gains `handle: Option<String>` behind
`#[serde(default)]`. Additive, so **no `SAVE_FORMAT_VERSION` bump**. A save
written before this feature loads with `None`, and `Game::load` mints one the
way it already mints a `ProgramId` for the `0` sentinel. The handle is
**stored and not derived**: derived off `ProgramId` it would cost nothing to
save, but adding or reordering a name file would rename every companion the
player already knows.

**Width is the binding constraint, not taste.** The map's status column holds
38.5 monospace cells and the widest shipped buff row already spends all but
3.8 of them, so a handle that makes `creature_label` longer draws off the
panel in silence. The pool is capped by a character census, not by judgement,
and the vocabulary is process-and-security rather than fantasy —
`EASTER_EGGS.md`'s occult-naming ban applies to the pool like any other
content.

## The band

**A derived named query, `relations::band`'s pattern, over
`opinion_of(a, &MemorySubject::Program(b))`.**

Exhaustive over its own enum so a new rung fails to compile
(`cell_mark`'s rule), derived on every read, stored nowhere and costing no
save field. `Standing::refuses_service` is the shape: a consequence of a band
is a named query on it, never a table of effects.

**Its surface is the memories page.** `R` from the roster already draws
`MemorySubject::Program` rows; with handles they finally name somebody, and
the band is the row's tag. No new screen, and the page still has no scroll.

**Its consequence is one line, in the one place the seam allows.**
`drift_idle_staff`'s last rejection already declines a tile a program holds a
grudge against; it gains a sibling that declines a tile **adjacent to a
program it holds a grudge against**, against the same
`MEMORY_AVOIDANCE_THRESHOLD` and with the same **signed** comparison, so a
fondness can never trigger an avoidance. A rejected candidate leaves the body
standing where it was, so this opens no failure mode and needs no fallback.

**Not `schedule_base_labour`.** That decides the whole assignment by priority
and then diffs it, with no sort and no score, and a memory term there would
sit in the path of both the anti-thrash rule and the
never-free-a-`Carrying`-holder rule. A bond may steer where a body *stands*.
It may never steer where a body is *posted*.

That single line is what the feature is for: the base physically shunning a
troublemaker, visible on the map without a word of UI.

## Witnessing

`game/base/tantrum.rs` writes `turned_on_me` on the victim alone. It widens to
every staff body within reach at the moment of the brawl, with the
`ProgramId` read hoisted above the branch the way `swept_here` hoists the
structure kind above the branch that despawns it.

This is the event a rumour network needs a supply of, and on its own it is
about five lines.

## Rumours

**`Game::spread_rumours`, on a period, modelled on `note_idling`.**

For each staff body, for each memory it holds whose def declares
`spreads_as`, write the named hearsay def **about the same subject** to every
staff body within `RUMOUR_REACH`, reusing `offshift::in_reach`.

**It draws no RNG.** `Game::remember` already draws none and writes no log
line, and a deterministic spread keeps the seeded stream unshifted — the
property `Game::run_routes` holds with a test asserting the tick draws nothing
when nothing preys. This one gets the same test. A rolled spread was the
obvious first shape and buys nothing here: the period already rate-limits it
and `strike_cap` already bounds it, so the roll would only make the feature
untestable.

**Its own period constant, not `MEMORY_POSTING_PERIOD`.** A rumour period
shorter than the posting period lets hearsay outpace the firsthand it copies,
which inverts the eviction order the whole design rests on.

**Shipped content: `turned_on_me` and `mauled_by` gain `spreads_as`.** Which
defs spread is a data decision and can change without touching Rust. The two
chosen are the ones where "somebody told me" is a sentence: what a colleague
did, and what a species did. A tile grudge somebody else holds is not.

The subject travels unchanged, so a bystander ends up holding a grudge against
the program that threw the tantrum — which is what feeds the band's avoidance,
and what closes the loop with the acting-out ladder that already exists.

## Censuses

- `MEMORY_TRIGGERS` in `tests/assets.rs` gains a row per hearsay def. There is
  no `trigger` field to derive it from; the catalogue is data and the triggers
  are Rust.
- **A new census on `spreads_as`**: it must resolve to a real def, carrying
  the **same `MemorySubjectKind`** and a **strictly smaller magnitude**. The
  second half is what eviction relies on to drop hearsay before firsthand. A
  `#[serde(default)]` field with no census is a field that ships authored
  nowhere, which this repo has a scar from (`AbilityDef::spread`).
- A handle-width census over the shipped pool.

## Scope and blast radius

Engine, and gui only where `creature_label` and the memories page already
draw. app-core is untouched. One additive save field, no format bump, no new
screen, no new `Mode`, no `ALL_MODES` row.

New tuning constants: `RUMOUR_PERIOD`, `RUMOUR_REACH`. The handle width cap is
a test census rather than a tuning constant — it is a layout and content
constraint, not a difficulty knob.

## What the tests have to prove

**Handles**

- Minted at every door into the roster, fusion included.
- Survive a save round trip, and a pre-handle save mints one on load. A RON
  round trip cannot catch a skipped field, so this needs a save→load test and
  not only the round trip.
- An empty `assets/handles/` leaves `creature_name` on the species name and
  breaks nothing.

**The band**

- Exhaustive, and a fondness never triggers the avoidance (the signed
  comparison).
- A body declines to idle beside a program it holds a grudge against, and the
  rejection leaves it standing rather than stranding it.
- The drift offers a different neighbour every beat, so the fixture must pick
  the tile at the far end of the fade, implant against it first, and **wind**
  the clock rather than ticking — and must stand its bodies clear of the Home
  and well inside the starting pocket or half the candidates are refused by
  the floor rule.

**Witnessing and rumours**

- Every body in reach of a brawl gets `turned_on_me`; a body out of reach
  gets nothing.
- A hearsay memory is written to bodies in reach and not to bodies out of it.
- **Hearsay does not spread further** — the def it would need names nothing.
- At `MEMORY_CAP_PER_PROGRAM`, hearsay is evicted before firsthand.
- The pass draws no `GameRng`: assert the stream is unmoved across a tick that
  spreads.
- With `assets/memories/` deleted, the pass writes nothing and nothing panics.

**Censuses**

- Every `spreads_as` resolves, matches its source's subject kind, and is
  strictly smaller in magnitude.
- Every hearsay def has a `MEMORY_TRIGGERS` row.
- Every shipped handle is within the width cap.

## What was rejected

- **A `secondhand` flag on `Memory`.** One new axis, valence scaled at read.
  Fewer files, but it is a save field, it is a second thing every reader must
  fold, and it stops `MEMORY_TRIGGERS` from being the whole pairing census.
- **Same def, fewer strikes.** Cheapest of all, and the screen could never
  tell the player what it heard from what it saw.
- **Deriving the handle from `ProgramId`.** Zero save cost, but a changed name
  pool renames every program the player knows.
- **A hop counter on the record.** Made redundant by the def chain.
- **A rolled spread.** See above.
- **Witnessing only, without propagation.** Bounded and cheap, and about a
  third of the payoff: the thing worth importing is precisely that an event
  reaches programs that were not there.
