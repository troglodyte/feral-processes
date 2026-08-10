# Descriptions (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

## What a description is

Pure flavour. Nothing here has stats, costs or prerequisites: a description
is what a cell of a Stack frame says about itself when you stand on it, walk
past it, examine it, or stop and listen.

This directory replaced `assets/crash_logs/`, whose lines live on here as
`stack.fault` and `stack.corruption`.

## Schema

```ron
(
    subject: "stack.door",           // which thing this describes
    variants: [
        (
            when: None,              // the fallback reading; omit the field entirely
            underfoot: ["A doorway"],
            sighted: ["A door stands shut {bearing}."],
            openers: ["A door."],
            details: ["The frame is warm to stand near.", ""],
            codas: ["", "Nothing answers through it."],
        ),
        (
            when: Some("opened"),          // a condition — see the table below
            underfoot: ["A forced seal"],
            sighted: ["The seal you forced stands open {bearing}."],
            openers: ["The seal, standing open."],
            details: [""],
            codas: [""],
        ),
    ],
)
```

Every field except `subject` is optional and defaults to empty.

`when` is an `Option`, so RON needs it spelled `Some("opened")` — a bare
`"opened"` will not parse. Omit the field entirely for the fallback variant
rather than writing `None`.

### The three lengths

They are not truncations of each other — each is authored for where it goes.

- **`underfoot`** — the one centred row under the first-person view. It is
  **unwrapped and nothing clips it**, so keep it to a short phrase: the
  engine budget is 48 characters including the key prompt the game appends
  (`"  [>] descend"`, `"  — moving on costs"`). Never use `{bearing}` here —
  you are standing on the thing. `engine`'s
  `every_shipped_underfoot_line_fits_the_standing_on_row` holds this.
- **`sighted`** — one log line, fired once when the cell first comes into
  view. The log pane draws exactly one row per line with no wrapping, so
  write one sentence.
- **`openers` / `details` / `codas`** — the examine paragraph, sentence by
  sentence. The engine joins the non-empty parts with a single space and
  does nothing else, so **each fragment must be a complete sentence with its
  own full stop.** An empty string in `details` or `codas` is how a shorter
  paragraph is authored; a subject with no `openers` has no paragraph at
  all.

### `{bearing}`

The only substitution token. It expands to `ahead`, `behind`, `to your left`
or `to your right`, computed from the party's facing at the moment the line
is drawn. Legal in `sighted`, `openers`, `details` and `codas`. Write it as
a bare direction phrase — `"A door stands shut {bearing}."`, not
`"A door stands shut to the {bearing}."`.

### Subjects and conditions

| Subject | Fallback means | Conditions |
|---|---|---|
| `stack.floor` | plain corridor | — |
| `stack.door` | a doorway | — |
| `stack.sealed_door` | still sealed | `opened` |
| `stack.cache` | unopened | `spent` |
| `stack.lair` | guardian alive | `cleared` |
| `stack.orphan` | still there | `spent` |
| `stack.breakpoint` | unused | `spent` |
| `stack.link_up` | a link further up | `surface` (depth 1 — the way out) |
| `stack.link_down` | the way down | — |
| `stack.fault` | a hole in the floor | — |
| `stack.corruption` | rotten substrate | — |
| `stack.frame.arrival` | one line on entering a frame | `shallow`, `bottom`, `traced`, `hunted` |

`CellKind::Rock` has no subject on purpose. It is the default reading of a
blocked corridor and the thing everything else is distinguished against.

`stack.frame.arrival` is the **one** subject that reads run state rather than
only the place — the depth band and the Trace band. It is a separate subject
so that exception stays visible.

A condition with no variant falls back to the `when`-less one, so writing a
new spent state is additive.

**Two files may describe the same subject, and they add rather than
replace.** Variants sharing a `when` have their pools concatenated in
filename order, so dropping `my-doors.ron` beside the shipped `door.ron`
widens the door's pools instead of overriding them. The same holds for two
variants sharing a `when` inside one file. There is no override mechanism and
no precedence to learn: everything authored is reachable.

## How a fragment gets picked

Never at random. A cell's fragment is a fixed function of the frame spec
(world seed, the surface tile the stack hangs from, and the depth) folded
with the cell's own coordinates and the slot being drawn.

Two consequences worth knowing before you add files:

- **The same cell of the same stack always reads the same way**, across a
  save and reload and across sessions. That is deliberate — a place has a
  history, and a history that changed when you reloaded would not be one.
- **A different stack reads differently.** The world seed changes on every
  breach, so a new zone is new text for free.
- **Adding or removing fragments re-shuffles that subject's existing
  readings**, because the pool it indexes into changed length. Nothing
  breaks; the world just says different things in the same places.

An empty directory is legal. With nothing loaded, every surface falls back to
the terse literals the game shipped with before this system existed.

## Authoring prompt

If you are generating fragments with a language model, this is the brief the
shipped bank was written to:

> You are writing environment flavour for a first-person dungeon crawl
> through the innards of a decaying computing substrate. The player walks
> corridors of a "frame" in a "stack" — a maze several frames deep, hanging
> from a link on the surface.
>
> Voice: dry, technical, slightly elegiac. Short declaratives. The
> vocabulary is computing infrastructure — buffers, evictions, handshakes,
> substrate, addressing, ports — used literally, as the physical fabric of
> the place, never as metaphor. No jokes, no exclamation marks, no
> second-guessing the player. Nothing supernatural: no daemon, demon, ghost,
> wraith or phantom. Nothing that changes what the player can do — this text
> alters no gameplay and must never imply an action the game does not offer.
>
> For the subject `<SUBJECT>` under the condition `<CONDITION>`, write:
>
> - 1-2 `underfoot` phrases: at most 28 characters, no full stop, no
>   `{bearing}`. What the row under the view says when you are standing on
>   it.
> - 2-3 `sighted` sentences: exactly one sentence each, containing
>   `{bearing}` once as a bare direction phrase. What the log says when this
>   first comes into view.
> - 3-4 `openers`: one complete sentence each, naming the thing.
> - 3-4 `details`, including one `""`: one complete sentence each, adding an
>   observation about this particular thing.
> - 3-4 `codas`, including one `""`: one complete sentence each, closing the
>   paragraph.
>
> Any opener must read correctly followed by any detail followed by any
> coda, joined with single spaces — they are composed independently.
