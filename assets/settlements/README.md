# Settlements

One `.ron` file per town or city. Drop a file in, and it joins the pool
every region draws from — no Rust change, no registration.

Where a settlement *stands* is not authored and cannot be: it is derived
from the world seed and the region's coordinates
(`crates/engine/src/settlements/placement.rs`), so a settlement is a
property of the map rather than something spawned into it. What you author
is who it is when the party gets there.

**This directory is optional.** Delete it and every region derives to
nothing, which is the pre-settlement game — not an error, and not a flag.
A malformed file is skipped with a warning at startup; it never panics.

## Schema

```ron
(
    id: "lowport",
    name: "Lowport",
    blurb: "A handful of shelters bolted to a dead relay mast. They will \
            trade with anything that walks in and still has power to spare.",
    kind: Server,
    specialty: Materials,
    temperament: Open,
)
```

Every field is required. There are no defaults, on purpose: a settlement
missing its `specialty` or `temperament` is not a neutral settlement, it is
one whose behaviour has nothing to read, and a file that loads
half-authored is worse than one skipped loudly.

| Field | Meaning |
| --- | --- |
| `id` | Unique. The pool is indexed in id order, so this also decides tie-breaks between files. Must not be empty. |
| `name` | What the map and the settlement's own screen call it. Must not be blank. |
| `blurb` | One or two sentences, shown on the settlement screen. |
| `kind` | `Mainframe` (a city, drawn `M`) or `Server` (a town, drawn `s`). A mainframe carries more shelf rows and higher tiers. |
| `specialty` | `Gear`, `Materials`, `Routines` or `Programs`. Weights what its shelf offers. |
| `temperament` | `Open`, `Guarded` or `Mercantile`. How it prices and how quickly it warms to you. |

`kind`, `specialty` and `temperament` are closed sets. Each variant is a
hook into a particular formula, so a new one is a Rust change — a value no
formula knows about would author a town that reads as broken rather than as
neutral. Everything else about a settlement is this file.

## Names may repeat

The map is unbounded and the pool is finite, so two regions far enough
apart can both draw `Lowport`. That is deliberate: the alternative is
generated names, and a generated name is not a place. If it ever reads
badly, the fix is more files rather than a naming scheme.
