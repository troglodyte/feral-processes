# What the base knows about itself: a social roadmap

**Status:** roadmap. Not implemented. Each sub-project below gets its own
design doc before it is built; this file holds the decisions they share and
the order they land in.

It began as a single design, "handles, bonds and rumours", out of
`2026-09-16-dwarf-fortress-social-survey.md`. A second pass against
`2026-09-16-rimworld-social-survey.md` kept its spine, replaced its rumour
pass, and grew it past one spec. What was dropped from the first version is
listed under "What was rejected".

## What it is for

The game has two of DF's three story-generating properties — the same event
lands differently per individual (`disposition.rs`), and the acting-out
ladder has no damper (`components::Grievance`). It has none of the third: a
memory is written only to whoever was there. And from RimWorld it has none of
the texture that makes a roster read as people: nobody talks, nobody is
anybody's friend, and nothing the base feels about a program can be seen.

The target loop, end to end:

> an insult → gossip → a reputation → a band → who avoids whom on the map →
> a sulk → sabotage, seen by a witness → gossip again

## Shared decisions

These hold across every sub-project.

**The hop cap is the chain length in the data.** A spreadable `MemoryDef`
names, in `spreads_as:`, the weaker def it propagates as. That def names
nothing, so it does not spread further. No hop counter, no propagation state,
no per-record field. A modder who wants three hops writes three defs.

**Opinion and morale are two reads of one store, with a per-def dial
between them.** `opinion_of(a, Program(b))` takes a record's full intensity;
`Game::morale` takes intensity × the def's `mood:` (default `1.0`, which is
today's behaviour). RimWorld's separate opinion and mood offsets, as one
multiplier. An insult can bite opinion hard and morale lightly; a brawl stays
at `1.0` so the tantrum spiral keeps its fuel.

**Nothing social draws `GameRng`.** Every choice — who talks, which
interaction, which template, whether a sulker spoils a unit — is a hash of
`(world seed, tick, participants)` reduced through `derive::index`, the way
static events and Stack descriptions are chosen. `Game::run_routes`' test is
the pattern: assert the stream is unmoved across a tick that does social
work.

**The empty catalogue stays a supported install.** Deleting
`assets/memories/` restores the game without that layer, never a broken one.
No system and no screen may be gated on a pool being non-empty.

**The social layer need not be moddable** (decided 2026-09-16). Interaction
defs and conversation templates may be Rust tables rather than asset
directories.

**A bond may steer where a body stands, never where it is posted** — with one
exception, and it is an existing door: a `Sulking` program already refuses a
post at a machine it resents through `Game::refuses_post`, and freezing-out
extends that door to a post beside a program it resents. For every program
that is not sulking the rule is absolute. `schedule_base_labour` gains no
memory term.

**Every derived value stays derived.** Band, reputation, `Sociability` and
situational thoughts are computed on read and saved nowhere. The handle is
derived too. The only new save field is the conversation records, additive
behind `#[serde(default)]`: **no `SAVE_FORMAT_VERSION` bump anywhere in this
roadmap.**

## The sub-projects

| | Sub-project | Needs | Size |
|---|---|---|---|
| **A** | Handles | — | small |
| **B** | Memory schema (`mood`, `stack_decay`) | — | small |
| **C** | Bonds: band, witnessing, loss, avoidance | A, B | medium |
| **D** | The SOCIAL tab | A, B, C | medium |
| **E** | Interactions and conversations | A–D | large |
| **F** | Situational thoughts | B | medium |
| **G** | Sulking behaviours | C, E, F | medium |

A and B land together as the first design. Each step is playable on its own:
A + C is already the base shunning a named troublemaker, and D makes it
visible.

### A and B. Handles and the memory schema

Designed in `2026-09-16-handles-and-memory-schema-design.md`. In short: a
handle is a hex code derived from `ProgramId` (stored nowhere), and
`MemoryDef` gains `mood:` and `stack_decay:`. **`spreads_as` lands with E and
`known_for` with D**, beside their only readers.

### C. Bonds

- **The band** is a derived, exhaustive named query over
  `opinion_of(a, Program(b))` — `relations::band`'s pattern, with a friend and
  a rival rung at least. A consequence of a band is a named query on it,
  never a table of effects (`Standing::refuses_service`).
- **Avoidance.** `drift_idle_staff`'s last rejection gains a sibling: decline
  a tile adjacent to a program held below the rival threshold, with the same
  **signed** comparison so a fondness never triggers it. A rejected candidate
  leaves the body where it stands, so no fallback is needed. This is also the
  "walks away when it arrives" half of freezing-out.
- **Witnessing.** `tantrum.rs` writes `turned_on_me` on the victim alone; it
  widens to every staff body in reach, the `ProgramId` read hoisted above the
  branch the way `swept_here` hoists the structure kind. Firsthand, not
  hearsay — this is RimWorld's "witnessed", and what feeds the gossip.
- **Loss.** When a program leaves the roster, every program holding it at
  friend or rival band gets a memory about it: a grief def for a friend, a
  relief def for a rival. Base-wide and instant, not proximity. **Three
  doors, and fusion counts:** `bench_or_dissolve`, `dissolve_tamed_program`,
  and `fuse_companions`' own despawn of the consumed parent. The last
  hand-writes its removal and is the one that will be missed. The band is
  read *before* the program is removed.

### D. The SOCIAL tab

The program manifest gains two tabs, **STATS** (today's page, unchanged) and
**SOCIAL**.

- **The tabs cost no row.** They sit right-aligned on the header's name line.
  Measured 2026-09-16 on the fullest program page: 17.3px spare at 720px
  against a 20px line, so a strip above the page would push STATS off its
  bottom.
- **No new `Mode`.** `Mode::Manifest` is a unit variant whose subject lives
  in `App`; the open tab is an `App` field beside it, so `ALL_MODES` is
  untouched.
- **SOCIAL, top to bottom:** a relationships box (strongest opinions first,
  one row each with handle, band and figure, capped); a *known for* line or
  two; then a **double-wide conversation log** filling the rest of the frame.
  Empty until E lands.
- **Known for** is derived from what *other* programs hold about this one:
  sum every other program's `Program(this)` records by def, and the
  heaviest def carrying a `known_for` supplies the phrase. The same store
  opinion reads, from the subject's side — so gossip moves a reputation.
- **The page has no scroll,** so its height is a census through the
  renderer's own measurement, and its row width is measured through
  `paint::with_painter`, not counted in characters.

### E. Interactions and conversations

**One timed social pass, `INTERACTION_PERIOD`.** Two idle staff bodies in
reach; the speaker picks a def from `assets/interactions/` and it writes a
memory on the listener, and on the speaker if the def says so.

- **Weighting** is data: per-`Disposition` and per-band multipliers on each
  def, times the speaker's **`Sociability`** — a second minted axis
  (`Reserved`/`Sociable`/`Chatty`, names provisional) derived off
  `ProgramId` with its own salt and never saved. Sociability scales how often
  a program talks at all and how readily it gossips.
- **Gossip is one interaction.** The speaker passes on one of its
  `spreads_as` memories; the listener gets the hearsay def about the same
  subject. **Never to the subject about itself** — without that skip, a
  program is handed a grudge against itself. This replaces the first draft's
  `spread_rumours` pass outright.
- **The period must not outpace firsthand.** An interaction period shorter
  than the posting period lets hearsay overtake what it copies and inverts
  the eviction order B's census rests on.
- **Conversations are exchanges**, two to four lines of paired templates
  with slots for both handles and a topic drawn from a memory the speaker
  holds. A census caps the line count.
- **Stored as records, rendered on read.** A bounded per-program ring
  (about 32) of `(tick, interaction, template index, other ProgramId, topic)`,
  saved behind `#[serde(default)]`. No generated text is stored, and a
  rename reads through to old lines.
- **Its own log, visible nowhere else.** Not `MessageLog`, not a
  `MessageSource`: the history screen, the map pane and `condense` never see
  it. The only reader is the SOCIAL tab.
- **A speech mark on the base map** over the pair, a cue the engine queues
  and forgets — `TransitCue`'s pattern, drained by the renderer — and
  gated on base space like every other base-space draw.

### F. Situational thoughts

Morale terms derived from current state and never stored — "posted beside a
rival", "the base has no amenity", "working unpowered" — listed on the
memories page beside real memories.

**This touches a seam, and that is the design problem.**
`memories::sum_intensity` is a free function because `task_progress_system`
has no `Game` to call, and morale enters `mining_success_chance` through
`CycleModifiers::morale`. A situational term needs world state, so it has to
be computed where the world is visible and carried in, not recomputed inside
the bevy system — or the two folds disagree, which is the failure the free
function exists to prevent.

### G. Sulking behaviours

All three attach to the existing `Sulking` rung. `Grievance` is appended to,
never inserted into, so a passive-aggressive rung could only sit *above*
`LashingOut`, which reads backwards.

- **A slanted voice.** A sulker's interaction weights lean toward slights and
  negative gossip — the sulk shows in its conversations before it escalates.
- **Freezing out.** No `idled_with` edge is written between a pair below the
  rival band; C's avoidance already walks it away; and `refuses_post`
  declines a post beside a resented program, for sulkers only.
- **Petty sabotage, limited to doors that exist.**
  - *Slower beside a rival* is F's situational term through the existing
    morale addend — no new formula.
  - *Spoiled output*: on the derived hash, a sulker at a post destroys one
    unit of its machine's output `Stock`, through `base_ledger::emit` as a
    sink, with one base-log line. Every staff body in reach gets a firsthand
    witness memory about the saboteur carrying `known_for` and `spreads_as`.
  - Posting, carrying, power and machine status are untouched.

## Censuses the roadmap adds

- `MEMORY_TRIGGERS` gains a row per new def (hearsay, grief, relief,
  sabotage witness).
- `spreads_as`: resolves, same subject kind, no onward chain, maximum
  stacked magnitude below the source's single strike.
- `mood` and `stack_decay` within range; `known_for` within the SOCIAL
  page's width.
- Every interaction def: its template slots resolve, its exchange is within
  the line cap, its weights name real dispositions and bands.
- The long name label within its surfaces' widths (A).
- The SOCIAL page's height and row width, measured through the renderer.

## What was rejected

- **A separate `spread_rumours` pass** (the first draft). Two spread routes
  at two rates write the same hearsay twice and break the eviction order.
- **A `secondhand` flag on `Memory`.** A save field and a second axis every
  reader folds, and it stops `MEMORY_TRIGGERS` from being the whole pairing
  census.
- **Same def, fewer strikes.** The screen could never say what was heard
  from what was seen.
- **A stored handle from a word pool** (the first draft of A). Hex codes
  need no pool, so deriving from `ProgramId` renames no one.
- **A hop counter on the record.** The def chain already is one.
- **A rolled spread**, and `GameRng` anywhere in the social layer.
- **Opinion out of morale entirely** (RimWorld's split). A feud stops feeding
  the tantrum ladder; the dial keeps both.
- **Interactions in `MessageLog`** under their own `MessageSource`. The
  unfiltered history screen floods and `condense` fights varied text.
- **A band tag on the `R` memories page** and **a conversation band on the
  STATS page.** Measured: zero rows at 720px, two at 1080px.
- **A new `Grievance` rung** for passive aggression.
- **Abandoned hauls and skipped refuelling** as sabotage. The first breaks
  `Carrying`'s never-freed rule that both destruction paths rest on; the
  second special-cases `plan_adjacent_take`.
- **Reputation from a program's own record.** Tallies that do not exist, and
  not a reputation — that is what *others* think.
