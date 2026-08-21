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
| `blurb` | One line of flavour under it, in the player's vocabulary. |
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

They are chosen to cover both valences and every subject kind that has a
trigger — not because these four are the interesting content. `Structure` and
`Activity` ship as subject kinds with nothing writing them yet.
