# dev-arenas

Battle scenarios for the arena, which has two halves. The `arena` bin runs a
fight offline and **measures** it, so difficulty can be tuned by measurement
rather than by playing to the fight. The game's own arena screen **plays**
the same scenario, in the whole battle interface, so the half a measurement
cannot reach gets a person pressing the keys.

Both read and write the same file, so a fight found by feel is measured
without retyping and a loss seed from a report is watched by hand.

```sh
cargo run --bin arena -- dev-arenas/opening-fight.ron
cargo run --bin arena -- dev-arenas/full-group.ron --out report.ron
cargo run --bin arena -- templates          # what `player: Template(..)` may name

FERAL_DEV_ARENA=1 cargo run                 # ...then [R] Arena on the main menu
FERAL_DEV_LOG=1 FERAL_DEV_ARENA=1 cargo run # ...and leave a record of it behind
```

A fight you play by hand otherwise leaves nothing but recall — the arena
session writes no save, no profile and no run history by design. `FERAL_DEV_
LOG=1` is the deliberate exception: it appends what happened inside the
fight to `dev-logs/battles.jsonl`, one JSON object per swing, round and
decision. See `dev-logs/README.md` for the schema. Set it before playing a
scenario whose answer you want to keep.

## Playing one

`FERAL_DEV_ARENA=1` puts an **Arena** row on the main menu. Unset — which is
every ordinary run — nothing about the feature is reachable and none of it is
loaded.

The builder edits a scenario row by row: Up/Down move, Left/Right adjust the
number under the highlight, Enter opens a picker for a species, an item or a
biome, Backspace removes a row. `[L]` loads a scenario from this directory,
`[S]` writes one back to it, and `[F]` fights.

The `Encounter:` row cycles `Authored → Field → Stack → Lair` with
Left/Right. On `Authored` the `Against:` rows are the fight. On any of the
others they disappear — a file cannot hold both — and a `Biome:` row takes
their place, with a `Depth:` row beside it for `Stack` and `Lair`. Cycling back to `Authored` puts an
opponent row back, so every state the row can reach is one `[S]` will write.

A fight opens the real battle screen. **Specials fire, items are spent,
targets are chosen** — that is the whole point, and it is what the bin cannot
do. The result screen then reports won/lost, rounds, HP left, companions
down and the seed, over the round-by-round transcript, with `[R]` to refight
the same seed and `[N]` to step to the next one. `[N]` is the manual version
of `reps`: it is the same `seed + n` the bin walks, so a fight watched here
replays there.

The loop the two halves make: build by feel on the screen, save, run the file
for a win rate, pin a loss seed, and watch that one by hand.

An arena session **touches no disk** — no save, no `profile.ron`, no
`run_history.log`. A rung earned in an arena fight is not earned, and a lost
fight against a Permadeath save lands on the result screen rather than on
Game Over.

At `reps: 1` it prints the transcript round by round in the game's own
wording, then what was fought, then the outcome. Above 1 it prints the
aggregate: win rate, mean
and median rounds, mean player HP left, mean companions downed, and **the
seeds of the losses** — pin one of those as `seed` with `reps: 1` and that
fight replays alone.

Either way it writes a structured report (default `arena-report.ron`, or
`--out`) holding every rep's seed, outcome, rounds, HP fraction, companions
downed, composition and full transcript. Warnings go to stderr, so piping
stdout to a file keeps the data clean.

Nothing either half does is written back to a save. Both load state and
throw it away, which is what lets a scenario point at a real save without
risk.

## What the `arena` bin does not measure

This section is about the headless half specifically; it is the reason the
played half exists.

**The party plays the game's own All-Attack** — `[A]` — every round. That is
a real in-game command rather than a policy engine written for the tester,
so the arena cannot drift from the game by inventing decisions the game
never makes. It also means **no companion Specials ever fire**, so ability
magnitudes stay unmeasured and a number from the bin is a *floor* on the
party's output. This is the same gap `balance_sim` has, and it is stated
here so a reader of a report knows what it measured.

Playing the same scenario on the arena screen is what closes it — and it
closes it by having a person press the keys rather than by writing a second
policy that could drift. What the screen gives up in exchange is the sample:
it always fights once, which is why `reps` is preserved and editable there
but only ever acted on by the bin.

## Schema

Every field of the scenario is optional and defaults as below, so a file
written today keeps parsing after a knob is added. A malformed file, an
unknown species or item id, or an unknown template name is an error naming
what was wrong — a scenario is authored, not scavenged, so a typo stops the
run rather than quietly changing the fight.

| Field | Default | Meaning |
|---|---|---|
| `player` | `Fresh(level: 1, zone: 1)` | Who is fighting — see below |
| `equip` | `[]` | `Fresh` only. Gear to wear |
| `inventory` | `[]` | `Fresh` only. Items in cargo |
| `party` | `[]` | `Fresh` only. Companions to field |
| `opponents` | — | The fight, authored. Required unless `encounter` is set |
| `encounter` | `None` | A context to roll instead of authoring one |
| `reps` | `1` | How many times to run it |
| `seed` | `0` | Rep *n* runs at `seed + n` |

### `player`

- `Fresh(level: N, zone: N)` — a new run, levelled and moved to that zone,
  then given whatever `equip` / `inventory` / `party` name. Both fields
  default to 1.
- `Save("saves/save.bin")` — a real save, loaded whole.
- `Template("extraction")` — a `dev-saves/` template, generated into a
  working copy and loaded. `cargo run --bin arena -- templates` lists them.

A save or a template brings its **entire** run across: level, stats,
equipment *and its fusion tiers*, party, perks, zone, Power and Fatigue. The
scenario then names only the fight — the opponents, or the context to roll
one from — which is the "what would happen if I hit this pack right now"
question. `equip`, `inventory` and `party` are
therefore an **error** on a save or template rather than being ignored —
`Fresh` is where you pick items.

### `equip`, `inventory`, `party`

```ron
equip: [(item: "plasma_router", tier: 0)],   // tier defaults to 0
inventory: [(item: "power_cell", qty: 5)],   // qty defaults to 1
party: [(species: "scrapper", level: 12)],   // level defaults to 1

// A copy can also carry a rare tier — Ordinary, Silver, Gold, Platinum
// or Prismatic. Defaults to Ordinary.
equip: [(item: "arc_lance", rarity: Gold)],

// ...and any number of affixes by id, in any order. Repeat one to stack
// it, which is what fusing two copies carrying the same affix does.
equip: [(item: "arc_lance", affixes: ["honed", "honed", "of_static"])],

// A companion takes the same `equip` rows the player does:
party: [(species: "scrapper", level: 12, equip: [(item: "arc_lance")])],
```

`tier`, `rarity` and `affixes` are what make one *copy* of an item different
from another, and none is decoration: gear fuses per physical copy and drops
at a rolled rare tier with a rolled affix, so a tier-2 Gold weapon with three
affixes is a different weapon from a plain one of the same name. All default,
so a scenario written before any of them existed still describes exactly the
copy it always did.

**None of the three can be reached by playing to it in an arena** — rarity and
affixes are rolled by `Game::grant_gear_drop` when gear *drops*, and a staged
fight drops nothing. So authoring them here is the only way to measure what
they are worth, which is the whole reason the fields exist.

`affixes` was a singular `affix: Option<AffixId>` until affixes stacked. It was
renamed outright rather than accepting both spellings: a scenario field that is
silently ignored reads as the feature doing nothing, which has cost a
measurement here before.

`equip` is applied *after* the zone is set, because gear locks in the zone
level it was equipped at and grows by `GEAR_LEVEL_STEP` per level.

**A party authored without `equip` is a naked party.** Any program the player
owns may wear gear, so a scenario whose companions carry none fields a weaker
party than a run of that shape actually would — and it is weaker in one
direction, so every number measured against it is soft in that direction too.
This is a second floor alongside the Specials gap above, and unlike that one
it is avoidable: gear the party to what the run you are modelling would have.

A companion's level tops out at `tuning::arena_level_ceiling()` — 12; asking
for more gets you that ceiling. It is deliberately the arena's *own* number
and not `Game::level_cap`, which is the zone-derived cap play runs under: a
scenario authors its own composition and staging it against the zone would
silently clamp every shipped file that asks for level 12.

### `opponents`

```ron
opponents: [
    (species: "sub_process", count: 9),
    (species: "glitch", count: 4),
]
```

Two properties are not obvious from the syntax:

- **Order is formation.** `ENGAGED_GROUPS` is 2, so only the first two
  entries are in melee range; a third or fourth acts only if its species has
  a ranged move, and sits inert otherwise. Reordering the list is a tuning
  lever, not a cosmetic choice. Two entries naming the *same* species stay
  two groups — that is how you put one pack in reach and another behind it.
- **There is no per-enemy level.** A wild spawn carries no `Experience`, so
  how hard one hits comes from the zone's stat multiplier and its potential
  roll. **The zone is the strength dial and `count` is the volume dial.** A
  scenario wanting a tougher individual raises the zone or names a tougher
  species. A rolled `encounter` has the same two dials plus one: the zone,
  and underground the depth, which raises stats and the group curve together.

The list is honoured verbatim, past what that zone could really field — "what
if zone 1 threw nine at me" is a legitimate tuning question, and explicit
authoring is the point of a tester. Exceeding the zone's ceiling prints a
warning naming the ask, the ceiling and the zone; it never silently caps.
`MAX_ENEMY_GROUPS` (4) and `MAX_GROUP_SIZE` (100) are the exception and are
a hard error, because past those the fight is not one the game can represent.

### `encounter`

The other half of the tuning question. `opponents` asks "what if zone 1 threw
nine at me" — something the game itself would never do. `encounter` asks what
the game *actually* throws, by running its own spawn machinery for a named
context and fighting whatever comes out.

```ron
(
  player: Fresh(level: 12, zone: 5),
  encounter: Some(Stack(biome: Mainframe, depth: 5)),
  reps: 50,
)

encounter: Some(Field(biome: OpenGrid)),   // the surface, same zone
encounter: Some(Lair(biome: Mainframe, depth: 4)),  // the guardian at the bottom
```

`Stack` and `Lair` are both underground at a named depth and they ask
different questions. `Stack` is the corridor — what a walk between frames
costs — and it is **never a boss**: `stack_encounter_pack` passes
`allow_boss: false`, so a lair guardian is unreachable from it. `Lair` is the
thing at the bottom of the stack, which is the only boss the Stack fields and
the game's only source of Portal Fragments. It runs `rouse_lair`'s own two
steps — `pick_lair_species` and `spawn_pack` — without the cell underfoot, the
`FrameMemory` record of a cleared lair, or the narration, since a scenario
asks for the guardian directly rather than walking into it.

Ask about them separately: a depth curve that leaves the corridor walkable can
still leave the gate shut, and the gate is the one on the critical path.

**Mutually exclusive with `opponents`** — one scenario asks one question, and
a file holding both is an error naming both.

**The zone is not in here.** It comes from the player row: `Fresh(zone: N)`,
or whatever a save or template brought with it. `ZoneLevel` is one resource
driving both gear scaling and enemy scaling, so a second zone here would be
two answers to one question. The biome *is* here, because it alone decides
the species pool and the arena's player stands on whatever tile the world
generator dropped them on.

`reps` changes meaning: a rolled encounter rolls its own pack, so fifty reps
**sample the distribution** that context fields rather than repeating one
composition fifty times. That is why every rep records what it fought — see
the `fought` line at `reps: 1`, and the `composition` field of every rep in
the report.

A `Stack` encounter descends for real, so the party is genuinely underground
and the depth stat multiplier and Trace apply as they do in play. Depth also
moves the group curve — one group on the first frame down, widening with
every frame — so depth 5 is not depth 1 scaled up. It is an *ambush*, never a
boss: a lair guardian is a different question from "what would I meet walking
a frame", and it is not reachable from here. A `Field` roll can field a boss,
because the surface roll can.

Three limits of this design, stated rather than left to be found:

- **Zone 1 field is the opening ring.** The arena's player stands at the
  danger origin, so zone 1 gentles the pool to what a fresh player can beat
  and clears bosses out of it. That is the honest answer to "what does zone 1
  throw at a new run" — and it means zone 1's *ungentled* roster is not
  reachable from here. Raise the zone for that.
- **A field roll is one habitat spawn roll**, so an ordinary one fields a
  single species group. In play, walking into a cluster can pull an adjacent
  second one into the same fight; reproducing that needs a populated zone.
  The Stack path has no such gap — `stack_encounter_pack` *is* the game's
  multi-group rule and it is called rather than reimplemented.
- **A biome nothing lives in cannot be picked.** The builder offers only
  biomes that are walkable and have at least one resident, which are the two
  clauses the spawn code itself gives up on. `Platform` is absent because no
  species lives on a base slab — that absence is the whole mechanism behind a
  base being a safe haven.

Unlike an authored composition, a rolled pack **is** capped by the zone's own
ceilings, because it is the game's own fight. It therefore warns about
nothing: nothing was asked for past a ceiling, because nothing was asked for.

## The shipped scenarios

- **`opening-fight.ron`** — the fight the game actually opens on. A fresh
  level-1 player, nothing equipped, one program from the opening ring.
- **`full-group.ron`** — a geared level-20 party against the largest group
  zone 3 can field. Note this is **4**, from `Game::max_group_size`, not the
  19 of `zone_group_cap` — so it is not the same fight `balance_sim`'s
  `full_group_at_zone` projects, despite both being called "a full group".
- **`lair-on-curve.ron`** — a stack's lair guardian fought by a party
  exactly on its zone's curve. The fight a run cannot walk away from, since
  the guardian is the only source of the breaching currency.
  `docs/measurements/2026-08-12-lair-depth-on-curve.md` is the sweep behind
  its depth.
- **`stack-depth-5.ron`** — whatever depth 5 actually fields. Kept to play
  rather than to tune against; its own comment says why.
- **`deep-lair.ron`** — the stuck run, on a `Template`. The evidence for the
  `0.8.1` scaling change.
- **`geared-vs-boss.ron`** — the `extraction` template against a boss, and
  the worked example of the template path.

These are meant to be kept and re-run after a `tuning.rs` edit, not to
demonstrate syntax. Add one whenever you find a fight worth watching twice —
by hand here, or with `[S]` from the arena screen, which writes the same
format and overwrites a file of that name deliberately.

The list above is short of the directory: `class-mirror`, `developed-companion`,
`gear-passives`, `stack-depth-5` and the five `policy-*` files are also
shipped and are described by their own comments.

## Nine of the fourteen are walkovers, and only two of those are a problem

Measured 2026-08-19 across every shipped scenario
(`docs/measurements/2026-08-19-combat-model-slice-1.md`): nine finish at
92–100% Integrity remaining. **That is correct for most of them and wrong for
two**, and the difference is what a scenario is *for*.

**A scenario that isolates a mechanism should be a walkover.** It measures a
*delta* — does the passive fire, is a developed companion worth more than a
plain one, do two classes differ — and it wants everything else out of the
way. `gear-passives`, `developed-companion`, `class-mirror` and
`geared-vs-boss` are all this, and their win rates carry no information.
So do the five `policy-*` set-pieces, which are authored to be **lost**: they
exist to watch the trained policy play, and a win there would be the surprise.
`stack-depth-5` is the mirror image — 0% and left alone deliberately, see
`docs/measurements/2026-08-19-stack-depth-curve.md`.

**A scenario that gates difficulty should not be a walkover**, and two are:

| | authors | measured | reads as |
|---|---|---|---|
| `opening-fight` | level 1, zone 1, no gear | 98%, 58% HP left | a real fight |
| `full-group` | geared L20 + 3 L12, **4** rootkits | 100%, 98% HP left | no contest |
| `lair-on-curve` | geared L24 + 3 L12, lair at **depth 2** | 100%, 3.2 rounds | no contest |

`opening-fight` is the shape the other two should have.

**This predates the combat model.** `lair-on-curve` measured 100% at 3.3
rounds the day before that branch too, so the attack roll neither caused it
nor fixed it. Do not read the walkovers as a regression from anything.

### The levers, measured

Both are the scenario's own numbers, not `tuning.rs` — moving a tuning
constant to make one scenario bite would move the whole game.

`lair-on-curve` is not carried by the party. Dropping the player from level 24
to 13 (the level `balance_sim` now says clears zone 3 geared) changes 3.2
rounds to 3.4; stripping the player's gear entirely gives 3.5; removing the
whole party still wins 100% in 10.1. **The depth is the only lever that
matters**, and it is steep:

| depth | wins | rounds | companions lost |
|---|---|---|---|
| 2 (shipped) | 100% | 3.2 | 0.00 |
| 3 | 100% | 8.2 | 0.00 |
| 4 | 100% | 12.2 | 0.28 |
| 5 | 42% | 39.6 | 2.46 |

`full-group`'s lever is the count. Its README note above is the whole story —
it fields 4, from `Game::max_group_size`, where `zone_group_cap` allows 19:

| count | wins | rounds | companions lost |
|---|---|---|---|
| 4 (shipped) | 100% | 7.6 | 0.00 |
| 8 | 100% | 14.8 | 0.18 |
| 12 | 100% | 25.4 | 1.02 |
| 19 | 98% | 60.6 | 2.68 |

### What a retune has to decide first

Neither table says what the target *is*, and that is the open question rather
than an oversight. `opening-fight` sits at 98% and 58% HP left, which is the
only shipped example of a fight that reads as one — but it is the game's first
encounter, and a set-piece may want to be tighter than that. Pick the number
before moving either scenario, or the retune is just taste applied twice.

Two traps if you do move them. **Arena figures compare within one build only**
— a changed formula reshuffles the `GameRng` stream as well as the model, so
compare against a run of the same build, never against a report in a
measurement file. And a scenario is content: re-running the batch after a
`tuning.rs` edit is the point of the directory, so a retune that lands should
be re-measured into `docs/measurements/`, not left in a commit message.
