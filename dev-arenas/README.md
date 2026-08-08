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
```

## Playing one

`FERAL_DEV_ARENA=1` puts an **Arena** row on the main menu. Unset — which is
every ordinary run — nothing about the feature is reachable and none of it is
loaded.

The builder edits a scenario row by row: Up/Down move, Left/Right adjust the
number under the highlight, Enter opens a picker for a species or an item,
Backspace removes a row. `[L]` loads a scenario from this directory, `[S]`
writes one back to it, and `[F]` fights.

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
wording, then the outcome. Above 1 it prints the aggregate: win rate, mean
and median rounds, mean player HP left, mean companions downed, and **the
seeds of the losses** — pin one of those as `seed` with `reps: 1` and that
fight replays alone.

Either way it writes a structured report (default `arena-report.ron`, or
`--out`) holding every rep's seed, outcome, rounds, HP fraction, companions
downed and full transcript. Warnings go to stderr, so piping stdout to a
file keeps the data clean.

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
| `opponents` | — | Required, and must be non-empty |
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
scenario then names only the opponents, which is the "what would happen if I
hit this pack right now" question. `equip`, `inventory` and `party` are
therefore an **error** on a save or template rather than being ignored —
`Fresh` is where you pick items.

### `equip`, `inventory`, `party`

```ron
equip: [(item: "plasma_router", tier: 0)],   // tier defaults to 0
inventory: [(item: "power_cell", qty: 5)],   // qty defaults to 1
party: [(species: "scrapper", level: 12)],   // level defaults to 1
```

`tier` is the copy's fusion tier, and it is not decoration: gear fuses per
physical copy, so a tier-2 weapon is a different weapon from a tier-0 one of
the same name. `equip` is applied *after* the zone is set, because gear
locks in the zone level it was equipped at and doubles per level.

A companion's level tops out at `CREATURE_MAX_LEVEL`; asking for more gets
you the ceiling, because that is all play can reach.

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
  species.

The list is honoured verbatim, past what that zone could really field — "what
if zone 1 threw nine at me" is a legitimate tuning question, and explicit
authoring is the point of a tester. Exceeding the zone's ceiling prints a
warning naming the ask, the ceiling and the zone; it never silently caps.
`MAX_ENEMY_GROUPS` (4) and `MAX_GROUP_SIZE` (100) are the exception and are
a hard error, because past those the fight is not one the game can represent.

## The shipped scenarios

- **`opening-fight.ron`** — the fight the game actually opens on. A fresh
  level-1 player, nothing equipped, one program from the opening ring.
- **`full-group.ron`** — a geared level-20 party against a full zone-3
  group. The progression sweep's scenario, run for real rather than
  projected.
- **`geared-vs-boss.ron`** — the `extraction` template against a boss, and
  the worked example of the template path.

These are meant to be kept and re-run after a `tuning.rs` edit, not to
demonstrate syntax. Add one whenever you find a fight worth watching twice —
by hand here, or with `[S]` from the arena screen, which writes the same
format and overwrites a file of that name deliberately.
