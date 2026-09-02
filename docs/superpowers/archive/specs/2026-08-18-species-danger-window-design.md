# The species danger window, and boss as a rolled variant

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

> `INDEX.md` warns that this header is the one line in a spec nobody ever
> revises. Answer "did this ship" from `CHANGELOG.md` and a grep, never from
> here.

Closes `TODO.md` #30, "lets move entity species into zones/stacks. see only
easiest ones on the surface, zone 1. stacks expose harder entities."

## The problem

A species' spawn eligibility is decided by biome and nothing else. Outside
the seven-tile opening ring, zone 1 fields the entire roster: a fresh player
can meet a Sentinel (stat total 154) on the same tile they could meet a
Glitch (45), and at 4% they can meet Wintermute (236). The roster is a
difficulty ladder — `growth_multiplier` bands 1.0, 1.25 and 1.5, five
species each, enforced by `species::stat_shape_faults` — but nothing in the
world reads the rung. So the ladder describes the roster without ordering
the game, and a zone is interchangeable with every other zone in what it can
put in front of you.

The second half of the same problem is bosses. There are two, both at
`growth_multiplier` 2.0, and they are the *only* things in the game that are
bosses. So a boss is a rare fixed encounter rather than a rare *kind* of
encounter, and the two of them list all four walkable biomes, which is why
one can turn up in zone 1.

## What this changes, in one sentence each

- A species is only eligible to spawn within a **window** of danger steps
  derived from its `growth_multiplier` band.
- **Any** species can spawn as a boss, rolled per individual; the two
  authored bosses become the apex band of the same ladder.

## Vocabulary

- **band** — a species' rung, derived from `growth_multiplier`. Bands 0, 1, 2
  for 1.0, 1.25, 1.5; **apex** for `is_boss`.
- **step** — `Game::danger_steps`, the existing escalation scalar.
- **window** — the range of steps a band is eligible in.
- **rolled boss** — an ordinary species spawned as a boss. Against an **apex**
  species, which is always one.

Player-facing, none of this has a name. A rolled boss draws exactly as a boss
draws today: magenta, tagged `[BOSS]`.

## One axis, and it is already built

`Game::danger_steps` (`game/spawning.rs`) is the step scalar: `zone - 1` on
the surface, `depth - 1` underground, depth *replacing* zone rather than
adding to it, clamped at `MAX_GROUP_SIZE_STEPS` (7). The group-size and
group-count curves already read it. The window reads the same number, so
there is no second difficulty axis to keep in step with the first — which is
the failure `Game::distance_from_danger_origin` was cut back to one consumer
to avoid.

## The band is derived, not authored

`SpeciesDef::danger_band()`, in `species.rs` beside `affinity_class`, and for
the same reason: a species' rung is a fact about numbers it already carries,
and a second authored field is a second thing that can disagree with the
first. `growth_multiplier` 1.0 → band 0, 1.25 → band 1, 1.5 → band 2,
`is_boss` → apex regardless of multiplier.

A modded value between rungs snaps to the nearest band. That is the same
concession `assets/species/README.md` already makes about the stat budget
being a step function: a mod is never refused, it just stops being readable
against the shipped ladder.

**No schema change.** `assets/species/README.md` gains a section documenting
that the band is derived and what it now controls; no `.ron` file is edited.

## The window

Three constants in `tuning.rs`, because how hard the game is, is not data:

```
TIER_ENTRY_STEPS  = 2    band b enters at step b * TIER_ENTRY_STEPS
TIER_WINDOW_STEPS = 3    and its last live step is entry + TIER_WINDOW_STEPS
APEX_ENTRY_STEP   = 4    apex enters here and, like the top band, never exits
```

| step | surface | Stack | bands live |
|---|---|---|---|
| 0–1 | zone 1–2 | depth 1–2 | band 0 |
| 2–3 | zone 3–4 | depth 3–4 | bands 0, 1 |
| 4–5 | zone 5–6 | depth 5–6 | bands 1, 2, apex |
| 6+ | zone 7+ | depth 7+ | band 2, apex |

**The top band never exits.** Band 2's window is open-ended, not `4..=7`. A
closed top empties the world at step 8, and steps are unbounded because zones
and depth are.

### The fallback, and why it is not optional

`SpeciesDb::windowed_matches(biome, step)` intersects the window with the
bands that biome actually holds. If the intersection is empty it falls back
to the band **nearest the window**, ties resolving upward. This mirrors the
opening ring's existing "the gentlest thing this biome has" fallback, pointed
the other way.

It is not defensive coding — it fires against the shipped roster, in two
known places:

| Biome | band 0 | band 1 | band 2 |
|---|---:|---:|---:|
| Mainframe | 3 | 2 | 3 |
| NullSector | 2 | 2 | 3 |
| OpenGrid | 4 | 3 | **0** |
| StaticField | **0** | 2 | 3 |

StaticField falls back at steps 0–1 and OpenGrid at steps 6+. The honest fix
for both is content — a band-2 OpenGrid species and a band-0 StaticField
species — and it is deliberately **not** in this change. Adding species is a
file each and changes the balance curves; do it as its own change with
`balance_sim` as the gate.

A census, `every_biome_fields_something_at_every_danger_step`, asserts the
fallback covers the real assets rather than trusting the table above.

### Where the window is applied

`Game::habitat_pools` gains a `depth: Option<u32>` parameter. It is handed in
rather than read off the party's locale, for the reason `SpawnEscalation`'s
doc comment already gives: ambient surface spawns and nest respawns keep
rolling on every tick while the party is underground, so anything read inside
the pool builder would size those from the party's depth. Four callers thread
it:

- `pick_habitat_species` — the surface spawner, `None`.
- the Stack encounter pool (`game/stack.rs`) — the frame's depth.
- `orphan_species` — the frame's depth. Spends a frame-seeded `StdRng`, never
  `GameRng`, and the window is a pure function of `(biome, step)`, so an
  orphan still survives a save/load unchanged.
- `pick_escort_species` — the depth its pack was spawned at.

The opening ring composes on top and stays second: window first, then
`beatable_by_a_fresh_player`. At step 0 the ring narrows a pool the window has
already cut to band 0, which is strictly more of what the ring was for.

## Boss becomes two facts instead of one

Today `SpeciesDef::is_boss` carries five consequences at once: spawns as its
own group, never tameable, never rolls a rare tier, magenta and `[BOSS]`, and
the boss payout. The split:

**`SpeciesDef::is_boss` becomes an apex marker.** It keeps meaning "always
spawns as a boss", and it gains the meaning "and is **not** engine-scaled,
because its stats are hand-authored". Overseer and Wintermute keep it and are
otherwise ordinary members of the ladder, entering at `APEX_ENTRY_STEP`.
`SpeciesDb::boss_habitat_matches` and the separate `boss_candidates` pool are
**deleted** — an apex species is drawn from the windowed pool like anything
else.

**`components::Boss` is a per-entity marker**, rolled onto any species at
spawn. `Game::is_boss_creature` becomes the one door: the species flag **or**
the component. It is already the door for the taming refusal
(`game/combat.rs`) and the pack sweep; two sites read `SpeciesDef::is_boss`
directly today and switch to it:

- `game/combat_rewards.rs:438`, the payout gate.
- `game/inspection.rs:822`, the view builder that feeds `views::EntityView`.

Because the renderer reads `views.is_boss` and never the species, **gui is
untouched** — the magenta glyph and the `[BOSS]` tag follow for free.

### What a rolled boss is worth

`BOSS_STAT_MULT = 1.75` in `tuning.rs`, applied at spawn the way
`Rarity::stat_mult` is. Apex species do not take it; their stats are authored.

Calibration: apex totals are 206 and 236 against a band-2 median of 140, so
~1.5x is "one band up". 1.75 puts a rolled boss above an Overclocked spawn
(`GOLD_STAT_MULT` 1.8 is close, but a boss rolls no rare tier on top), which
is what makes it read as a wall rather than as a shiny.

This number is **ungated by `balance_sim`** — it models no bosses, and
`toughest_ordinary_species` explicitly excludes them. `dev-arenas/` is the
instrument, and the figure should be re-run there before release.

### The spawn rate does not move, and neither does the fragment economy

`BOSS_SPAWN_CHANCE` stays 0.04 and stays the one boss rate. Only the pool it
draws from changes. Today it fires wherever a biome has a boss species and all
four biomes do, so 4% of surface spawns are already bosses; after this, 4% of
surface spawns are still bosses, drawn from the windowed pool and marked with
the component.

Underground is unchanged for a different reason: `Encounter::Stack` passes
`allow_boss: false`, so that roll never happens down there. The Stack's only
boss is a lair guardian, one per lair. Portal Fragment income is therefore
untouched by this change — worth stating plainly, because "every species can
be a boss now" reads like it should widen the game's only fragment source and
it does not.

### `pick_lair_species` gets simpler and closes a standing trap

Today it prefers an apex species for the entrance tile's biome and, where the
biome has none, falls back to the toughest ordinary species returning
`(species, false)` — a guardian that is not a boss and so **pays no
fragments**. That fallback is unreachable against the shipped assets only
because both apex species list all four biomes. `CLAUDE.md` records the
consequence as a live trap: "removing a habitat from the last boss covering
some terrain makes every stack under it unbreachable."

After this it draws the toughest species from `windowed_matches` at the step
`Game::danger_steps` gives for `Some(pos.depth)` — asked of it, never
recomputed here — and returns `true` unconditionally. The trap is gone — there is
no biome without a guardian, because there is no species that cannot be one —
and guardians scale with depth for free. It keeps its frame-seeded `StdRng`
and `LAIR_SALT`.

### Everything else falls out

- `Game::roll_rarity` already refuses an apex species; it refuses a rolled
  boss too. Boss is decided before rarity, so no rare tier stacks on a boss.
- `spawn_pack(is_boss: true)` already spawns alone, with an escort past zone
  1 drawn from the same pool.
- Taming refusal, the `gather_pack` sweep exemption, the battle-screen tag and
  the inspect screen all go through `is_boss_creature` or `views.is_boss`.

## Save

`CreatureSave` gains `boss: bool` behind `#[serde(default)]`. Additive under
field-named RON, so **no `SAVE_FORMAT_VERSION` bump** — an old save loads with
every creature un-bossed, which is exactly what it was.

The load path must insert `components::Boss` when the field is true. A RON
round-trip test cannot catch a load path that drops it, so this needs a real
save → load → assert-the-component test.

## Testing

Unit, pure:

- `danger_band` over the real assets: 5/5/5/2 across bands 0/1/2/apex.
- the window schedule at steps 0 through 8, including that band 2 does not
  exit.
- `windowed_matches` fallback, both directions, on a hand-built db.

Census, over the real assets:

- `every_biome_fields_something_at_every_danger_step`.
- zone 1 spawns nothing above band 0, seeded, over enough rolls to be
  meaningful.
- zone 1 never spawns an apex species — the thing that reads wrong today.

Behaviour:

- a rolled boss is untameable, rolls no rare tier, spawns alone, and pays the
  boss reward.
- a rolled boss survives save → load with its component.
- a lair guardian is a boss at every depth, and pays fragments in a biome
  whose only apex species has been removed from the fixture — the trap, pinned.
- an orphan's species is unchanged across a save/load at depth.

Gates:

- `cargo test --workspace`.
- `cargo test -p feral-processes-engine balance_sim`. The curves may move.
  `toughest_ordinary_species` now over-states what zone 1 can field, which
  makes the gate **conservative rather than wrong**; leave it alone in this
  change and note it. Narrowing the gate to the window is its own decision and
  wants its own argument.
- `cargo run --bin arena -- dev-arenas/opening-fight.ron` and `full-group.ron`
  after `BOSS_STAT_MULT` is in, since nothing else measures it. Arena numbers
  compare within one build only — read deltas against a baseline captured on
  the same build, never absolutes against an older report.

## Blast radius

Engine only, plus assets docs and the two seam files.

| File | What |
|---|---|
| `crates/engine/src/species.rs` | `danger_band`, `windowed_matches`, delete `boss_habitat_matches` |
| `crates/engine/src/tuning.rs` | four constants |
| `crates/engine/src/components.rs` | `Boss` marker |
| `crates/engine/src/game/spawning.rs` | `habitat_pools` depth param, pool unification, `roll_rarity`, `BOSS_STAT_MULT` at spawn |
| `crates/engine/src/game/party.rs` | `is_boss_creature` reads the component too |
| `crates/engine/src/game/combat_rewards.rs` | payout gate through the door |
| `crates/engine/src/game/inspection.rs` | view builder through the door |
| `crates/engine/src/game/stack_features.rs` | `pick_lair_species` |
| `crates/engine/src/game/stack.rs`, `game/turn.rs` | thread `depth` |
| `crates/engine/src/save.rs` | `CreatureSave::boss` |
| `assets/species/README.md` | derived band, rolled boss, apex meaning |
| `CLAUDE.md`, `docs/seams.md` | the `is_boss` and `pick_lair_species` entries change meaning |
| `CHANGELOG.md` | one section at merge |

## Explicitly out of scope

- **New species.** A band-2 OpenGrid species and a band-0 StaticField species
  are the honest fix for the two fallback sites. Content, its own change, its
  own balance run.
- **The six band-0-only gear items** — `kinetic_edge`, `packet_buffer`,
  `probe_service`, `scrap_ward`, `shiv_routine`, `handshake_forge` — stop
  dropping once the window passes band 0. They are opening-grade gear and the
  player has moved past them; noted so it is not later diagnosed as a bug.
- **Narrowing `balance_sim` to the window.** See Gates above.
- **Any renaming.** A rolled boss is a boss on screen, with no second word for
  it.
