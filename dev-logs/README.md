# dev-logs

What happened inside a fight, as a file a script can read.

The game can be played, and it can be measured, but before this it could not
be both at once: `arena` measures a fight nobody plays, and a fight a person
plays leaves nothing behind but recall. This closes that — a hand-played
fight now writes `dev-logs/battles.jsonl`.

```sh
FERAL_DEV_LOG=1 cargo run                                # an ordinary run
FERAL_DEV_LOG=1 FERAL_DEV_ARENA=1 cargo run              # ...or an authored fight, [R] Arena
wc -l dev-logs/battles.jsonl
```

Unset — which is every ordinary run and every player's build — nothing is
collected, nothing is written, and no file is created. The flag is read
**once**, at startup, through the same `dev_flag` predicate `FERAL_DEV_ARENA`
and `FERAL_DEV_CONSOLE` use.

The log itself is gitignored — it is a measurement, not source; this README
is the schema and stays checked in. Nothing rotates the file, so a long
session appends to whatever is already there: `rm dev-logs/battles.jsonl`
before a run you mean to analyse on its own.

## The format

[JSON Lines](https://jsonlines.org): one JSON object per line, no wrapping
array. `t` is the tag every reader dispatches on, and `fight` is on every
record — a line is interpretable alone, and many fights in one session
separate cleanly.

```jsonc
{"t":"fight_start","fight":1,"seed":904,"zone":3,"depth":0,
 "party":[{"slot":0,"label":"You","species":null,"level":12,"max_hp":140},
          {"slot":1,"label":"Sentinel","species":"sentinel","level":12,"max_hp":180}],
 "enemies":[{"group":0,"species":"trojan","count":4}]}

{"t":"round","fight":1,"round":3,
 "party_hp":[120,180],
 "enemies":[{"group":0,"hp":[61,61,12]}]}

{"t":"enemy_choice","fight":1,"round":3,"group":1,"actor":"trojan",
 "move":"Payload Drop","target_slot":2,"target":"Sprite",
 "target_hp_before":44,"target_max_hp":90,"target_bracing":false}

{"t":"party_action","fight":1,"round":3,"slot":1,"actor":"Sentinel",
 "kind":"special","name":"redundancy_sync","target_slot":2}

{"t":"fight_end","fight":1,"rounds":10,"won":false,
 "player_hp_frac":0.0,"companions_downed":2}
```

### `fight_start`

Emitted by `Game::begin_battle`, once per fight, after `BattleState` exists.

| Field | Meaning |
|---|---|
| `fight` | Counts up within the process. Every later record in the fight carries it. |
| `seed` | The run's world seed (`WorldMap::seed`), not the fight's. |
| `zone` | `ZoneLevel` — the surface zone the run has breached to. |
| `depth` | Stack frame depth, `0` on the surface. |
| `party[]` | `slot` (0 is the player), `label`, `species` (`null` for the player), `level`, `max_hp`. |
| `enemies[]` | `group` index, `species`, `count` at the opening bell. |

### `round`

Emitted by `Game::battle_resolve_round` at the **top** of the round, before
anything resolves — a snapshot taken at the end would be the aftermath.

| Field | Meaning |
|---|---|
| `round` | 1-based, matching the planning screen's own header. |
| `party_hp[]` | One entry per slot, index-aligned with `fight_start.party`. A slot whose member is gone reads `0` rather than being dropped. |
| `enemies[]` | `group` index and the living members' `hp`, front first. |

### `enemy_choice`

Emitted by `Game::choose_wild_action_at` once per swing, **after** the move
and target are picked and **before** the caller applies damage. Both the
trained policy and the uniform baseline exit through it, so no swing can be
missed; a back group with nothing that reaches emits nothing, because no
swing happened.

| Field | Meaning |
|---|---|
| `group` | Which enemy group the attacker is in. `0` and `1` are engaged; further back needs a `ranged` move. |
| `actor` | The attacker's species id. |
| `move` | The move's display name, from its species `.ron`. |
| `target_slot` | Party slot hit. `0` is the player. |
| `target` | That member's display label. |
| `target_hp_before` | **HP before this hit lands.** The number the file exists for: focus fire is a distribution over it, and a per-round snapshot cannot show it, since four attackers all act inside one round. |
| `target_max_hp` | For turning the above into a fraction. |
| `target_bracing` | Whether the target was Defending — the aggro prior and the +6 DEF both follow from this. |

### `party_action`

Emitted by `Game::resolve_one_action` before the action resolves, and by
`Game::battle_flee` for the jack-out attempt. Reached only for a member that
actually acts: the dead and the stunned are skipped before it.

| Field | Meaning |
|---|---|
| `slot` | Party slot acting. `0` is the player. |
| `actor` | That member's label. |
| `kind` | `attack`, `special`, `defend`, `item` or `flee`. |
| `name` | The ability id for a `special`, the item id for an `item`, `null` otherwise. |
| `target_slot` | The party slot an ally-targeted `special` landed on. `null` otherwise. |

**Known gap:** an `attack`'s and an enemy-targeted `special`'s group index is
not recorded — `target_slot` is a *party* slot and there is no field for a
group. Party-side focus fire is therefore not answerable from this file;
enemy-side focus fire, which is what the schema was designed around, is.

### `fight_end`

Emitted by `Game::end_battle`, before the dead are reaped out of the party —
the last moment `companions_downed` can be counted at all.

| Field | Meaning |
|---|---|
| `rounds` | Rounds the fight lasted. |
| `won` | Read off the **enemies** being gone, never off the player's HP. A defeat is absorbed inside the round that lands it by `difficulty::death_handling_system`, which in Forgiving reboots the player — so their HP afterwards says nothing about the outcome. |
| `player_hp_frac` | Player HP as a fraction of max, at the end. Subject to the same caveat: a level-up full-heals in `progression::add_xp`, and the killing blow is usually the level. |
| `companions_downed` | Party members not alive when the fight ended. |

## Reading it

```sh
# every swing that landed on a bracing target
jq -c 'select(.t=="enemy_choice" and .target_bracing)' dev-logs/battles.jsonl

# what fraction of swings went at each party slot
jq -r 'select(.t=="enemy_choice") | .target_slot' dev-logs/battles.jsonl \
  | sort | uniq -c

# which routines a person actually spent
jq -r 'select(.t=="party_action" and .kind=="special") | .name' \
  dev-logs/battles.jsonl | sort | uniq -c
```

There is deliberately no analysis script yet: the shape of the analysis is
not knowable until there is data to look at.

## What it does not cover

- **Surface and economy events** — spawns, hauling, sweeps, upkeep. The base
  log pane and `balance_sim` already cover that ground, and the volume would
  bury the battle records this is for.
- **Diagnostics.** This answers "what happened in the fight", not "why did
  the code misbehave".
- **Two games at once.** One process, one file, append-only: two runs would
  interleave and their `fight` ids would collide.
- **Whether the fight was fun.** The playtest questions in
  `dev-arenas/policy-*.ron` still need a person.
