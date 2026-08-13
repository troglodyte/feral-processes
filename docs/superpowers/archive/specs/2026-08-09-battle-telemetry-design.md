# Battle telemetry for dev builds — design

**Date:** 2026-08-09
**Status:** approved, not implemented
**Scope:** `crates/engine`, `crates/app-core` — two crates, so this earns the
full spec-and-plan pipeline per `CLAUDE.md`'s process-weight rule.
**Save format:** unchanged. Telemetry is dev output, not run state.

## The failure this exists to fix

On 2026-08-09 the trained enemy policy shipped, and the one question it
could not answer offline was whether a party using its **routines** — which
`arena::run_rep` never does, since it plays All-Attack — still loses to it.
The measured answer for `policy-full-kit` is that All-Attack loses 87.5% of
the time. A person played the same fight in the arena screen.

Nothing came back. An arena session writes nothing to disk by design, the
`MessageLog` died with the process, and the only artifact was recall. The
decision that fight was staged to inform — is the 0.324 → 0.611 headline
inflated by measuring a party that cannot heal — is still open.

That is the gap: **the game can be played, and it can be measured, but not
at the same time.**

## What exists today

| Instrument | What it holds | Why it does not cover this |
|---|---|---|
| `run_history.log` | One line per run, at game over | A whole run compressed to "flatlined at cycle 2500" |
| `MessageLog` | Prose lines, in memory | Capped, pruned by `retain_outcomes_since_battle`, gone at exit |
| `arena::Report` | Transcript + outcome per rep | Headless only; the interactive screen produces none |
| `balance_sim` | Projected curves | RNG-free, models no abilities, no Defend |

The arena bin already answers "what happens when nobody uses a routine".
Nothing answers "what happened when a person did".

## Decisions

| Question | Answer |
|---|---|
| What is recorded | Battles only. Every open question — enemy policy, Defend, focus fire, routines — lives inside a fight. |
| Who reads it | A script, first. One record per line, aggregated across fights. |
| Format | **JSON Lines.** `serde_json` is added to **app-core only**. |
| Where it is built | `crates/engine` — a resource the engine fills, drained by app-core. The `PendingProfileWrites` shape. |
| How it is enabled | `FERAL_DEV_LOG=1`, read once at startup like `FERAL_DEV_ARENA` and `FERAL_DEV_REVEAL`. Off by default and unreachable in a player's build. |
| Where it lands | `dev-logs/battles.jsonl`, gitignored, beside `dev-saves/`, `dev-arenas/` and `dev-training/`. |

### Why JSON rather than RON

The workspace has `ron` and `serde` but no `serde_json`, so line-delimited
RON would have cost nothing. JSON was chosen anyway because the consumer is
a *script*, and every off-the-shelf tool already reads JSONL where nothing
reads line-delimited RON.

The dependency lands in **`crates/app-core`, never `crates/engine`**. The
engine derives `Serialize` on the record types and hands them over as
values; app-core is the only crate that turns one into a string. The engine
depends on `bevy_ecs`, `bincode`, `noise`, `pathfinding`, `rand`, `ron` and
`serde` and that list does not grow here — `cargo check --workspace` at ~1s
is a property worth protecting, and app-core already sits above the engine
in the graph.

## Non-goals

- Surface and economy events — spawns, hauling, raids, upkeep. The base log
  pane and `balance_sim` already cover that ground, and the volume would
  bury the battle records this is for.
- Diagnostics or tracing. This answers "what happened in the fight", not
  "why did the code misbehave". A `tracing` layer is a different feature
  with a different consumer.
- Log rotation, size caps, or cleanup. It is a dev file; delete it.
- Anything reachable in a player's build.

## Architecture

Four pieces.

| Piece | Where | What it is |
|---|---|---|
| Record types | `crates/engine/src/telemetry.rs` | Plain `Serialize` structs. No `World`, no IO — unit-testable alone. |
| The buffer | `resources::BattleTelemetry` | `{ on: bool, records: Vec<Record> }`, default off. |
| Emission | four existing seams | Each already the single place its event happens. |
| The writer | `crates/app-core` | Drains and appends. The only place `serde_json` appears. |

### The five seams

Emission goes only where the codebase already declares a single path, so no
new "one true way" is invented and none can be bypassed:

| Seam | File | Emits |
|---|---|---|
| `Game::begin_battle` | `game/combat.rs` | `fight_start` |
| `Game::battle_resolve_round` | `game/combat_round.rs` | `round` |
| `Game::choose_wild_action` | `game/combat_policy.rs` | `enemy_choice` |
| `Game::resolve_one_action` | `game/combat_round.rs` | `party_action` |
| `Game::end_battle` | `game/combat_teardown.rs` | `fight_end` |

`choose_wild_action` is the load-bearing one. It is already documented as
the only place a wild program's move and target are decided, so a record
taken there cannot miss a swing — and it is the only point where the
target's HP **before** the hit is in hand, which is the number the whole
feature exists to collect.

### The records

One JSON object per line. Five kinds, discriminated by `t`:

```jsonc
{"t":"fight_start","fight":1,"seed":904,"zone":3,"depth":0,
 "party":[{"slot":0,"label":"You","species":null,"level":12,"max_hp":140},
          {"slot":1,"label":"Sentinel","species":"sentinel","level":12,"max_hp":180}],
 "enemies":[{"group":0,"species":"trojan","count":4}]}

{"t":"round","fight":1,"round":3,
 "party_hp":[120,180,44,90],
 "enemies":[{"group":0,"hp":[61,61,12]},{"group":1,"hp":[61,61,61,61]}]}

{"t":"enemy_choice","fight":1,"round":3,"group":1,"actor":"trojan",
 "move":"Payload Drop","target_slot":2,"target":"Sprite",
 "target_hp_before":44,"target_max_hp":90,"target_bracing":false}

{"t":"party_action","fight":1,"round":3,"slot":1,"actor":"Sentinel",
 "kind":"special","name":"redundancy_sync","target_slot":2}

{"t":"fight_end","fight":1,"rounds":10,"won":false,
 "player_hp_frac":0.0,"companions_downed":2}
```

`fight` counts up within a process, so many fights in one session separate
cleanly. Every record carries it, so a line is interpretable alone.

`target_hp_before` is the point. Focus fire *is* a distribution over that
number, and per-round snapshots cannot show it: four attackers all act
inside one round, and by the snapshot they have finished.

`party_action.kind` is one of `attack`, `special`, `defend`, `item`,
`flee` — the `BattleAction` variants, which is what makes "did the human
actually brace or heal" answerable at all.

### Enabling, and the cost when disabled

`FERAL_DEV_LOG` is read **once**, in the launcher, beside the existing
`FERAL_DEV_ARENA` and `FERAL_DEV_REVEAL` reads, and becomes
`BattleTelemetry::on`. Emission sites test that bool and return.

This matters more than it looks: `train` runs 1.9M fights per session and
`arena` runs tens of thousands. An env lookup per event would be a
per-swing syscall in the trainer's hot loop. One predictable branch is the
requirement, and the reason is written at the emission helper.

### The arena carve-out

`CLAUDE.md` records that an arena session touches no disk, held by three
tests — `an_arena_fight_writes_no_save`, `..._no_profile`,
`an_arena_loss_writes_no_run_history` — and by `App::in_arena()`
early-returning out of `after_tick`.

**Telemetry is deliberately a fourth thing, and it is allowed to write.**
The rule exists so a tester's fight cannot corrupt a save or pay out
profile rewards to a real player; a dev-only log under `dev-logs/` does
neither, and the arena is the single place this feature is most wanted.

So the telemetry drain must **not** sit behind `in_arena()`. That looks
like a violation of a documented invariant, which is exactly why it needs a
test of its own — `an_arena_fight_still_writes_telemetry` — beside the
three that say the opposite about everything else. Without it the next
person reads the invariant as absolute and "fixes" this.

### Error handling

Every failure degrades to "no telemetry", never to a broken run:

- `FERAL_DEV_LOG` unset → nothing collected, nothing written, no file.
- `dev-logs/` missing → created on first write.
- A failed write → reported once on the status line, collection continues,
  the run is unaffected. The same shape `flush_profile_writes` uses, and
  for the same reason: a dev log must never take a run down with it.
- A record that fails to serialize → skipped with a warning. Not reachable
  with the shipped types; the arm exists because a later field could make
  it so.

## Testing

Engine:

- `telemetry_records_round_trip` — a record serializes and parses back.
  Pure, no `World`.
- `telemetry_is_off_by_default` — a `Game` built normally collects nothing.
  This is the guard on the trainer: the cost of the feature when unused is
  zero records, and the test says so.
- `an_enemy_choice_records_the_targets_hp_before_the_hit` — the number the
  feature exists for, asserted against a target whose HP then changes.
- `every_enemy_swing_produces_one_record` — count over a multi-round fight,
  because a seam that silently misses swings produces a biased dataset,
  which is worse than no dataset.
- `a_party_special_records_its_ability_and_target` — the routines half.

app-core:

- `no_telemetry_file_is_written_when_disabled` — asserts on the *file*, the
  omission being invisible otherwise.
- `an_arena_fight_still_writes_telemetry` — the carve-out above, and the
  regression to head off is someone folding the drain back into
  `after_tick`.
- `a_failed_telemetry_write_does_not_end_the_run`.

Each must fail with the change removed.

## Known gaps

- **It records; it does not analyse.** Turning
  `dev-logs/battles.jsonl` into "the wounded companion took N% of swings"
  is a script that does not exist yet, and is deliberately out of scope —
  the shape of the analysis is not knowable until there is data to look at.
- **One process, one file, append-only.** Two games running at once
  interleave their records. `fight` ids would collide. Not worth solving
  until it happens.
- **It cannot say whether a fight was fun.** It answers what happened, not
  how it read, and the playtest questions in `dev-arenas/policy-*.ron`
  still need a person.

## Documentation obligations

- `dev-logs/README.md` — new: the record schema, one row per field, and the
  `FERAL_DEV_LOG` flag. This is the reference for anyone writing an
  analysis script.
- `.gitignore` — `dev-logs/`.
- `CHANGELOG.md` at the version this lands under.
- `CLAUDE.md` — the arena carve-out above belongs in the load-bearing seams
  list, since it is a stated exception to an invariant already recorded
  there.
- `dev-arenas/README.md` — a line pointing at the flag, since that is where
  someone about to hand-play will be reading.
- `docs/manual.md` and the root `README.md` are carved out and stay stale.
