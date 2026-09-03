# dev-logs

What happened inside a fight — and what the base made while nobody was
watching it — as a file a script can read.

The game can be played, and it can be measured, but before this it could not
be both at once: `arena` measures a fight nobody plays, and a fight a person
plays leaves nothing behind but recall. This closes that — a hand-played
fight now writes `dev-logs/battles.jsonl`.

The same file carries the **base records**, which answer the other half:
every extract, every compile, every stall and every unit that entered or left
the run. The filename is still `battles.jsonl` because it is one stream from
one process; splitting it would mean two files whose ticks have to be
reconciled by hand.

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
array. `t` is the tag every reader dispatches on, and a line is interpretable
alone.

**Two families, and which key a record is anchored by tells them apart.** A
battle record carries `fight`, which counts up within the process, so many
fights in one session separate cleanly. A **base** record carries `tick`
instead: it happens while no fight is open at all, and re-keying one onto a
fight would be inventing an association rather than recording one. That split
is held by an exhaustive match in `Record::set_fight`, so a new variant
cannot join either family by accident.

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

## The base records

Written whether or not a fight is open, by the seams named below. Every one
carries `tick`. All but `machine_stall` and `hand_craft` carry `zone` too —
the sector the run had breached to, which is what makes a rate attributable
when the base travels across a breach. Those two do not: a status is a fact
about a machine rather than about the sector, and a hand-compile is priced in
ticks, which is what its record is read for.

```jsonc
{"t":"base_snapshot","tick":3000,"zone":2,"staff":4,"posted":3,
 "machines":5,"depots":1,"supply":8,"draw":6}

{"t":"extract","tick":3041,"zone":2,"machine":[3,4],"kind":"mining_node",
 "tier":2,"worker_species":"sprite","item":"core_fragment",
 "rolled":3,"landed":3,"ok":true}

{"t":"assemble","tick":3052,"zone":2,"machine":[5,4],"kind":"fabricator",
 "item":"bytecode_block","inputs":[["blank_substrate",2]]}

{"t":"machine_stall","tick":3060,"machine":[5,4],"kind":"fabricator",
 "status":"starved"}

{"t":"hand_craft","tick":3120,"item":"charge_coil","qty":1,
 "careful":false,"bench":"armory","ticks_spent":90}

{"t":"acquire","tick":3200,"zone":2,"item":"core_fragment","qty":4,
 "source":"kill"}

{"t":"consume","tick":3240,"zone":2,"item":"power_cell","qty":1,
 "source":"fuel"}

{"t":"haul","tick":3260,"machine":[3,4],"kind":"mining_node",
 "errand":"deposit","item":"core_fragment","qty":5,"distance":3}
```

### `base_snapshot`

Emitted by `Game::note_base_snapshot` from `tick_inner`, once every
`base_ledger::BUCKET_TICKS` (1,000) and stamped with the tick that opens the
window — before the clock advances, so it heads the window whose events
follow it.

**Read this first.** Every other base record is a count; this is what to
divide by. Without it "412 Core Fragments" answers nothing, and B7 — is
roster size the throughput dial — is a question about rate per *posted
program*. It is also where ticks-per-sector comes from.

| Field | Meaning |
|---|---|
| `staff` | Programs the player owns that are base staff. Derived, never assigned: what is left once the party, the wielded program and any sortie are taken out. |
| `posted` | Bodies actually standing at a job this tick. |
| `machines` | Structures that run one — `StructureDef::runs_a_job`, i.e. a `work` or an `assembles`. |
| `depots` | Structures that store — `StructureDef::stores`, never an id, so a modded depot counts. |
| `supply` / `draw` | The power grid, off `Game::base_power`. A base running short is `draw > supply`. |

### `extract`

Emitted at both arms of `resolve_gather_cycle`, in `task_progress_system`
(a posted worker) and `player_gather_system` (the player cranking it
themselves).

| Field | Meaning |
|---|---|
| `machine` | The machine's own base-space tile, `[x, y]` — what tells one instance of a kind from another. |
| `kind` | Its `StructureDef` id. |
| `tier` | `StructureTier`, 1 if it has none. |
| `worker_species` | The posted program's species, whose `base_int` feeds the reliability roll. `null` for the player, who has no species and works a node at exactly the roster average. |
| `rolled` / `landed` | What the cycle produced against what reached the buffer. The difference is **the clog loss** — `deliver_payout` clamps against `output_room()`, and nothing else in the game records it. |
| `ok` | `false` is a fizzle: a cycle that produced nothing. The only empirical route to `systems::mining_success_chance`. |

### `assemble`

Emitted at `assembler_system`'s completion branch, with the input drain in
the same scope — so consumption and production are one record.

| Field | Meaning |
|---|---|
| `item` | The product. One record is one unit. |
| `inputs[]` | `[item, qty]` pairs drained to make it. |

### `machine_stall`

Emitted by `systems::set_machine_status`, which already speaks **only on
transition** — so these are edges, never a per-tick state dump. No `zone`:
a status is a fact about a machine, not about the sector.

| Field | Meaning |
|---|---|
| `status` | `running`, `starved`, `clogged`, `unstaffed`, `stranded`, `idle`, `unpowered`. Its own match, never a `Debug` derive. |

The signature to look for in **B4** is not the production rate: it is
transitions to `clogged` per 1,000 ticks rising with `zone` while extractor
output climbs.

### `hand_craft`

Emitted at `advance_hand_craft`'s completion, one record per unit.

| Field | Meaning |
|---|---|
| `careful` | Whether the careful surcharge was paid. |
| `bench` | The `StructureDef` id of the machine that exists to make this item, or `null`. Not a claim that a bench was used — this is the *hand* path. |
| `ticks_spent` | Against the machine cycle for the same item, this is the whole of **B2**. |

### `haul`

Emitted by `haul_step_system` when a leg actually moves goods. A `Tend` —
standing at the post with nothing to move — writes nothing, or a full base
would log a row per tick for re-deciding the same errand.

| Field | Meaning |
|---|---|
| `machine` | The worker's **post**, not where it is standing: by the time an errand acts the two are the same tile, and the post is what an analysis groups by. |
| `errand` | `deposit` (product out to a Depot), `load` (an ingredient into the machine's own hopper), `collect` (drawing one off a Depot). |
| `qty` | What actually moved, which is not what was wanted — a Depot that filled or emptied while the worker walked moves less. |
| `distance` | Chebyshev tiles from the post to the other end. |

**The corrected B3.** The published audit claimed a machine can only be fed
by its four orthogonal neighbours; it cannot — `Errand::Collect` walks to a
Depot, so any layout works and adjacency is a throughput *multiplier*. The
cost falls on the **extractor**, not the assembler: `task_progress_system`
gates on `at_station`, so a producer makes nothing while its worker walks,
while a consumer keeps working off its hopper. Plot `machine_stall` fractions
against this `distance` to price it.

### `acquire`

Emitted by `Game::grant_loot`, the one door all eighteen sources pass
through. **Never folded into the ledger** — see `base_ledger`'s doc for why a
kill's fragments must not read on the player's screen as a machine's work.

| Field | Meaning |
|---|---|
| `source` | `kill`, `rock` (cut out of base space), `cache`, `contract`, `trade`, `refund` (a demolition), `etch` (a disk the player made). |

**B5** is this against `extract`: what share of a sector's Core Fragments a
Mining Node is actually worth.

### `consume`

Units that left the run. Folded *and* recorded, unlike `acquire`: a sink is
the other half of the ledger's own arithmetic.

| Field | Meaning |
|---|---|
| `source` | `fuel` (a supplier burning Power Cells), `build` (spent at the tick the structure is raised, not when they left the shelf), `base` (the base spending its own shelves — the dig crew's tile, a sortie), `breach` (destroyed outright). |

`craft` covers a hand-compile's ingredients and the blank a routine is burnt
onto; `install` is a disk written into a slot, which refunds nothing and so
is a sink rather than a move.

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

# empirical mining_success_chance, against 0.4 + 0.1 * tier
jq -r 'select(.t=="extract") | "\(.kind) \(.tier) \(.ok)"' \
  dev-logs/battles.jsonl | sort | uniq -c

# what a clog ate, per item
jq -r 'select(.t=="extract" and .landed < .rolled)
       | "\(.item) \(.rolled - .landed)"' dev-logs/battles.jsonl \
  | awk '{n[$1]+=$2} END {for (i in n) print n[i], i}'

# B2: machine-made against hand-made, per item
jq -r 'select(.t=="assemble") | "machine \(.item)"' dev-logs/battles.jsonl \
  > /tmp/made
jq -r 'select(.t=="hand_craft") | "hand \(.item)"' dev-logs/battles.jsonl \
  >> /tmp/made
sort /tmp/made | uniq -c

# B5: where a sector's Core Fragments came from
jq -r 'select(.t=="acquire" and .item=="core_fragment") | .source' \
  dev-logs/battles.jsonl | sort | uniq -c

# ticks per sector, the number every other figure is divided by
jq -r 'select(.t=="base_snapshot") | "\(.zone) \(.tick)"' \
  dev-logs/battles.jsonl | awk '{if (!(0 in a)) a[0]=1; print}' | uniq -f1
```

There is deliberately no analysis script yet: the shape of the analysis is
not knowable until there is data to look at.

## What it does not cover

- **Spawns and sweeps.** Still uncovered, and deliberately: the base log pane
  and `balance_sim` cover that ground, and the volume would bury everything
  else here.
- **Diagnostics.** This answers "what happened in the fight", not "why did
  the code misbehave".
- **Two games at once.** One process, one file, append-only: two runs would
  interleave and their `fight` ids would collide.
- **Whether the fight was fun.** The playtest questions in
  `dev-arenas/policy-*.ron` still need a person.
