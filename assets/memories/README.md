# Memories (mods)

Edit or add a `.ron` file in this directory and it's picked up automatically
the next time a game session starts — no recompiling required. A malformed
file is skipped with a warning logged in-game rather than crashing startup, so
a broken def costs the game that one memory kind and nothing else.

**This directory may be deleted.** An empty catalogue is valid and inert:
nothing can be remembered, every program's Morale reads zero, and the game is
exactly the pre-memory game. That is the same supported way to play that
deleting `assets/sectors/` or `assets/policies/enemy_battle.ron` is.

## What a memory is

An owned program accumulates memories of things that happened to it — one
entry per *kind of thing* per *subject*. A memory fades unless the thing
happens again, and reinforcement both resets its clock and compounds it up to
a cap. Wild and hostile programs carry none, and neither does the player.

Each file is one kind:

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

| Field | Meaning |
|---|---|
| `id` | Unique across the directory. It is what a save records, so renaming one drops whatever was remembered under the old name. |
| `name` | What a row on the memories screen leads with, in front of the subject. |
| `blurb` | One line of flavour on the same row, in the player's vocabulary. Said **once per kind** — see below. |
| `valence` | Signed. The intensity of one undecayed strike — negative is a grudge, positive a fondness. Zero means the memory is worth nothing, and a census refuses it. |
| `half_life` | In **ticks**: how long until intensity halves. |
| `subject` | Which kind of thing a record of this def is about (below). |
| `strike_cap` | How far reinforcement compounds before it stops. At least 1. |

All seven are required. Any field added in a later version will carry a
default, so a file written today keeps parsing untouched — but none of these
seven may be omitted.

Two files claiming the same `id` is not an error; the alphabetically last one
wins, which is deliberate (a mod's `zz_stranded_at.ron` overrides the shipped
def without deleting it).

## Intensity

Intensity is **derived from the game clock**, never stored:

```
valence * min(strikes, strike_cap) * 2^-(ticks since reinforced / half_life)
```

So nothing decays on a timer and a memory cannot drift out of step with the
clock. `tuning::MEMORY_HALF_LIFE_MULTIPLIER` scales every `half_life` in the
game at once — how sticky memory is in general is a difficulty decision and
lives in code, while how long *this* memory lasts relative to the others is a
content decision and lives here.

An entry that has faded below a threshold is dropped the next time the program
forms a memory, and a program holds a bounded number of them at once — when it
is over, the weakest goes.

## `name` and `blurb` on the screen

A program's memories screen (`R` on the roster) is one row per entry:

```
Mauled by — Zero-Day 3  (-8, recently)  One hit and there was almost nothing left of me.
```

The `blurb` is a property of the *kind*, so it is printed on the first row
naming that def and left off the repeats — a store easily holds three or four
entries of one def about different subjects, and the same sentence four times
down a page is worse than not printing it.

**Both fields are measured, not estimated.** The page does not scroll and
nothing clips a row horizontally, so a `name` or `blurb` long enough to run
past the right edge is simply lost, taking the strength and age figures with
it. A census measures the worst page the catalogue can build; a def authored
too long fails the build rather than shipping a row that runs off the box. If
you are writing a mod def, keep the blurb to roughly the length of the shipped
ones.

## `subject`

| Kind | A record of it is about |
|---|---|
| `Nothing` | nothing in particular — the event itself |
| `Program` | one specific owned program, by its stable id |
| `Species` | a species |
| `Structure` | a kind of structure |
| `BaseTile` | one tile of the base |
| `Activity` | a kind of work |

The declared kind is checked when a memory is written: a record whose subject
does not match its def's `subject` is refused and nothing is written. So a def
declaring `BaseTile` can only ever be about a tile.

**`BaseTile` is base space, not the surface.** Base space and surface space
are the same two integers meaning different things, and reading one as the
other has already put the base's roster on the open grid once. There is
deliberately no surface or Stack place variant yet; they arrive when content
asks for them.

## There is no `trigger` field

What *causes* a memory is Rust, not data. This is the same half-data seam
perks sit on: the catalogue crossed over, the hooks did not. A data trigger
vocabulary would have to be invented from a handful of samples, which is the
speculative abstraction this codebase's principles forbid — and unlike
`assets/achievements/`, where the four trigger shapes were known to be the
whole vocabulary, nobody yet knows what the whole vocabulary here is.

So a **new** memory kind is a file *plus* an engine hook that writes it, and a
mod editing the shipped files can change every number, name and blurb above
but not what makes a program remember.

## The shipped kinds

| id | | subject | formed by |
|---|---|---|---|
| `bonded_in_battle` | + | `Program` | surviving a won fight, about each other program that also survived |
| `hard_won` | + | `Nothing` | winning a fight the party was outmatched in |
| `mauled_by` | − | `Species` | taking a single hit worth a large share of maximum Integrity |
| `stranded_at` | − | `BaseTile` | being posted somewhere nothing can reach |
| `frayed_here` | − | `BaseTile` | running a need down with nothing in the base that could restore it, or nothing that could be walked to |
| `settled_in` | + | `Structure` | a stretch of service at a machine that is running |
| `jammed_here` | − | `Structure` | a stretch of service at a machine that is backed up |
| `cutting_rock` | − | `Activity` | a stretch of service on the dig crew |
| `swept_here` | − | `Structure` | a GC Entropy Sweep hitting the machine you are posted at |

They are chosen to cover both valences and every subject kind, not because
these nine are the interesting content. `Nothing`, `Program`, `Species` and
`BaseTile` are written by the fight-and-staffing triggers; `Structure`
and `Activity` by the four about a program's working life.

`frayed_here` shares `stranded_at`'s subject and sign and is deliberately a
separate kind: a hauler nothing can reach and a program worn down with
nowhere to go are different complaints, and the base tells you so in
different sentences. See `assets/needs/README.md`.

**The four work kinds divide on one axis, and it is not valence.**
`swept_here` is an **edge** — a sweep is an event, and it is remembered the
moment it lands. The other three are **stretches of service**: nothing
distinguishes the first tick at a machine from the thousandth, so they are
written on a period, and a memory reaching its `strike_cap` means a real
stretch of the run rather than a moment of it. That is why a `half_life`
authored for one of these is not comparable to one authored for `mauled_by`:
a stretch memory is topped up as long as the posting lasts, and starts
fading only once the body moves on.

A `Structure` memory names the machine's **kind**, not the machine, so it
outlives a machine that is destroyed and remembers a rebuilt one as the same
thing. `settled_in` and `jammed_here` share that subject and oppose in sign,
so a machine kind that mostly runs nets out to a mild fondness over a run and
one that spends its life clogged nets out to a grudge.

A digger is the one posting with no machine to remember instead — its `Task`
targets a dig site, which is not a structure — so cutting rock is remembered
as a kind of work and follows the program rather than the hole.
