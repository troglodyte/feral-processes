# Nemesis lines (mods)

A `.ron` file in this directory is picked up automatically the next time a
game session starts — no recompiling required. A missing or malformed file
leaves the bank empty rather than crashing startup.

## What a nemesis is

A program that has beaten the party or driven them off keeps a grudge count
(`components::Nemesis`) and, past its first, a promoted rarity tier. See
`docs/superpowers/specs/2026-08-17-nemesis-design.md` for the full feature.
This directory is its flavour bank:

- **`names.ron`** — the pool a nemesis's name is drawn from, written once
  into `components::CustomName` on its first grudge and never rewritten
  after (a second loss escalates the grudge, not the name).

Unlike `assets/descriptions/`, the bank is not keyed by a subject: there is
one shared pool for every species, not a per-species one. Only 4 of 17
shipped species author `SpeciesDef::taunts` today, so a per-species pool
would read generic for most of the roster while costing a schema change. A
species override on top of the shared pool is a clean later addition — see
the design doc's "The name and the taunt" — and does not invalidate
anything authored here.

## Schema

```ron
(
    lines: [
        "Segfault",
        "Deadlock",
    ],
)
```

### `names.ron`

A pool of short proper names. Keep each entry to `MAX_CUSTOM_NAME_LEN` (12
characters) — the chosen name is written through `components::CustomName::
sanitize`, which truncates rather than rejecting, so a longer entry loads
fine but renders cut off mid-word.

## Selection spends no RNG draw

Naming may not touch `resources::GameRng`, because `Game::mark_nemeses`
runs on **every** fight, arena scenarios included — a draw there would
shift the RNG stream for every scenario in `dev-arenas/`, and would not
survive a save/load either. The name is indexed by a value folded from the
creature's own identity (its species id and `Potential` rolls) and reduced
with `derive::index` — see `crates/engine/src/nemesis.rs` for the fold.
