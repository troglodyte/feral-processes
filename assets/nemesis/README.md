# Nemesis lines (mods)

Two `.ron` files in this directory, both picked up automatically the next
time a game session starts — no recompiling required. A missing or
malformed file leaves that one bank empty rather than crashing startup or
refusing the other bank.

## What a nemesis is

A program that has beaten the party or driven them off keeps a grudge count
(`components::Nemesis`) and, past its first, a promoted rarity tier. See
`docs/superpowers/archive/specs/2026-08-17-nemesis-design.md` for the full feature.
This directory is its two flavour banks:

- **`names.ron`** — the pool a nemesis's name is drawn from, written once
  into `components::CustomName` on its first grudge and never rewritten
  after (a second loss escalates the grudge, not the name).
- **`taunts.ron`** — what a nemesis says when a fight with it opens, logged
  once at the top of that battle.

Unlike `assets/descriptions/`, neither bank is keyed by a subject: there is
one shared pool for every species, not a per-species one. Only 4 of 17
shipped species author `SpeciesDef::taunts` today, so a per-species nemesis
pool would read generic for most of the roster while costing a schema
change. A species override on top of the shared pool is a clean later
addition — see the design doc's "The name and the taunt" — and does not
invalidate anything authored here.

## Schema

Both files share one shape: a flat list of lines, nothing else.

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

### `taunts.ron`

A pool of third-person verb phrases, each completing `"<name> "` — e.g.
`"has been waiting for a rematch since the last one."` becomes `"Segfault
has been waiting for a rematch since the last one."` in the log. Two rules,
both load-bearing:

- **Never assume how the party left last time.** A defeat and a jack-out
  both raise the same grudge (`Game::mark_nemeses`), so a line claiming
  "you ran" or "you fled" would be wrong half the time. Write about the
  program's own memory of the fight instead.
- **Logged as `MessageKind::Info`, on purpose.** `MessageLog::
  retain_outcomes_since_battle` keeps only `Outcome`, `Loot`, `LevelUp`,
  `Raid` and `Complete` when a battle ends, so the taunt is pruned before it
  can follow the player onto the map — it belongs to the fight it was said
  in, the same as a player-triggered taunt (`game/taunt.rs`).

## Selection spends no RNG draw

Neither pick may touch `resources::GameRng`, because `Game::mark_nemeses`
and `Game::begin_battle` both run on **every** fight, arena scenarios
included — a draw there would shift the RNG stream for every scenario in
`dev-arenas/`, and would not survive a save/load either. Both banks are
indexed by a value folded from the creature's own identity (its species id
and `Potential` rolls for a name, the grudge count folded on top for a
taunt) and reduced with `derive::index` — see `crates/engine/src/nemesis.rs`
for the fold.
