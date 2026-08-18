# 2026-08-18 — The wild population is a halo around the player, not a property of the sector

## The claim

The wild population still decays monotonically with distance from wherever
the player habitually stands, and past roughly 60 tiles the sector is
effectively empty. After 20,000 ticks of a player pottering about a base,
the density box centred on the base holds 15 wild programs against a target
of 12, and boxes at 25, 50, 75 and 100 tiles out hold 10, 6, 3 and 1. After
walking 300 tiles in a straight line, the boxes centred on the walked path
past 60 tiles hold **zero to two** against the same target of 12.

`0.5.12`'s fix — `WILD_LOCAL_DENSITY_TARGET` plus seeding out to
`INITIAL_SPAWN_SCATTER_TILES` — flattened the peak (the old measurement was
65 in one box) but did not change the shape. The halo is still there and the
far field is still empty.

This is not a mistuning. It is unreachable by construction: the map is an
unbounded lazily-chunked Perlin world, population is placed only relative to
the player, and the ambient roll is `WILD_SPAWN_CHANCE = 0.05` per tick
against a walking rate of one tile per tick. Traversing one 25x25 density
box costs 25 ticks and therefore buys ~1.25 spawn rolls, against a target of
12 — an order of magnitude short. Raising the chance to 1.0 would still
place only ~25 per box traversed while making a spawn fire on top of the
player every single tick. The one thing that ever fills space at the target
density is the one-time seed, which covers a 40-tile disc around the
arrival point.

## How to reproduce it

```sh
cargo test -p feral-processes-engine probe_wild_density -- --ignored --nocapture
```

The instrument is `crates/engine/src/tests/wild_density_probe.rs`, an
ignored print-only test. Seed 9001, `DifficultyMode::Forgiving`, the real
shipped assets. It runs four scenarios from the same seed:

- **entry** — `Game::new`, measured immediately.
- **camp** — 20,000 `tick()`s with the player frozen on the arrival tile.
- **travel** — 300 steps east, one tile per `tick()`, `Position` written
  directly so a walk into a creature does not become a battle.
- **mill** — 20,000 ticks with the player random-walking inside 8 tiles of
  the arrival tile, which is what tending a base actually looks like.

The player's `PowerReserve` is refilled every 100 ticks in all four, or
starvation ends the run and `tick_inner` returns early.

Counts are Chebyshev, radius `WILD_SPAWN_RADIUS_TILES` (12) — the same
25x25 box `local_hostile_count` uses, so a number here is directly
comparable to `WILD_LOCAL_DENSITY_TARGET`.

## The numbers

Density boxes along the +x axis, target 12 in every cell:

| scenario | at player | +25 | +50 | +75 | +100 | total live |
|---|---|---|---|---|---|---|
| entry | 9 | 12 (at +20) | 6 (at +40) | 0 (at +60) | 0 | 129 |
| camp, 20k ticks | 15 | — | — | — | — | 191 |
| mill, 20k ticks | 15 | 10 | 6 | 3 | 1 | 215 |
| travel, 300 tiles | — | 10 | 3 | 0 | 0 | 138 |

Travel, every box centred on the walked path:

| tiles out | 25 | 50 | 75 | 100 | 125 | 150 | 175 | 200 | 225 | 250 | 275 | 300 |
|---|---|---|---|---|---|---|---|---|---|---|---|---|
| in box | 10 | 3 | 0 | 0 | 1 | 0 | 0 | 0 | 2 | 2 | 1 | 0 |

New here: the travel row, and the mill gradient. Both are what the bug
report describes and neither had been measured.

Replicating what was already believed: the entry row reproduces the
`0.5.12` design — a 40-tile disc seeded at the target, hard edge just past
it. Ring totals at entry (23 inside 20 tiles, 96 in 20-40, 10 in 40-60) are
consistent with a *uniform* draw over the square, since Chebyshev ring
20-40 covers ~3x the tiles ring 0-20 does. There is no clustering bug in
the seeding itself.

Also confirmed, and worth recording because it rules out a suspect:
`WILD_CREATURE_CAP` (2000) and its farthest-from-player cull never fire.
Peak observed population across all four scenarios is 215.

## What it does not say

- **One seed.** 9001 only. The gradient is structural rather than seed
  luck — it falls straight out of the spawn radius and the tick rate — but
  the absolute counts will move with terrain, since unwalkable tiles and
  biomes with no habitat species both make a roll a miss.
- **Zone 1 only.** Group size grows with zone, so a deep sector places more
  creatures per successful roll and the far field will be less bare than
  this. The *shape* does not change; only the constant in front of it.
- **No nests, no raids, no player kills.** The probe never fights. Real
  play removes creatures near the base (you kill them) and adds them
  (nests respawn), and neither is modelled here. The near-base number is
  therefore the one to trust least.
- **Travel walks a straight line east over whatever terrain seed 9001
  puts there.** A route through unwalkable ground would read lower for
  reasons that are not the bug.
- **It says nothing about what density *should* be.** 12 per screen is the
  existing target and the probe measures against it; whether 12 is the
  right number is a play question this cannot answer.

## Open questions

- If population becomes a property of place, what happens to ground the
  player cleared and walked away from — does it stay cleared, and for how
  long? The probe cannot answer this because it never kills anything.
- `WILD_CREATURE_CAP`'s cull despawns the farthest hostiles from the
  player. In an unbounded world where population is placed per chunk,
  that cull and a per-chunk "already populated" mark disagree: walking
  back to a culled chunk would find it permanently empty unless the cull
  also clears the mark.
