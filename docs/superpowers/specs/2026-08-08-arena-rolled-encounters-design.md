# Arena: rolled encounters

**Date:** 2026-08-08
**Status:** designed

## The problem

The arena authors a fight: you name a species and a count, and the tool
spawns exactly that. That is the right shape for "what if zone 1 threw nine
at me" — a question the game itself would never ask — but it cannot answer
the other half of the tuning question, which is *what the game actually
throws*. To find out today you play to the fight.

The ask: pick a context — "zone 1 field", "zone 1 stack, level 2", "zone 5
stack level 5" — and have the arena field whatever that context would
produce.

## What is being added

A scenario may name a **rolled encounter** instead of an authored opponent
list. Staging then runs the game's own spawn machinery for that context and
fights whatever comes out.

```ron
(
  player: Fresh(level: 12, zone: 5),
  encounter: Some(Stack(biome: Mainframe, depth: 5)),
  reps: 50,
)
```

Because a rep rolls its own pack, `reps` gains a meaning it did not have:
fifty reps of a rolled encounter sample the *distribution* of what that
context fields, not fifty copies of one composition.

### Schema

```rust
/// Where a fight is being had, for a scenario that wants the game's own
/// roll rather than an authored composition.
pub enum Encounter {
    Field { biome: Biome },
    Stack { biome: Biome, depth: u32 },
}
```

`Scenario::encounter: Option<Encounter>`, `#[serde(default)]` like every
other field, so every existing scenario — including the three shipped ones —
keeps parsing untouched.

**The zone is not here.** It comes from the player row: `Fresh { zone }`, or
whatever a save or template brought with it. `ZoneLevel` is one resource
driving both gear scaling and enemy scaling, and a second zone on the same
screen would be two answers to one question.

**The biome is here**, because it is the single thing that decides the
species pool and the arena's player stands on whatever tile `Game::new`
happened to drop them on. Without it, most of the game's encounter tables
are unreachable from the picker.

### Validation

`Scenario::validate` gains one rule: `encounter` set *and* `opponents`
non-empty is an error naming both fields. The existing "a scenario with
nobody to fight is a typo" error then applies only when `encounter` is
`None`.

A biome whose habitat pool is empty is an error naming the biome, raised at
staging where the species db is loaded — not in `validate`, which by
contract checks only what is checkable without a `Game`.

## How a roll is performed

A new `crates/engine/src/arena/encounter.rs` holding one function:

```rust
pub(crate) fn roll(game: &mut Game, encounter: &Encounter)
    -> Result<Vec<EnemyGroup>, String>
```

Three steps.

**1. Stamp the biome.** `WorldMap::set_override` on the player's tile, with
`walkable` taken from `Biome::walkable`. This is the same sparse overlay
`stamp_platform` writes through, so nothing new is introduced to the map —
and it is what makes every encounter table reachable instead of only the one
under the spawn point.

**2. Roll the pack.**

*Field* — `pick_habitat_species(x, y, allow_boss: true)` then
`spawn_pack(species, is_boss, x, y, SpawnEscalation::surface())`. Those are
the two halves of `try_spawn_habitat_creature`, minus its nest branch: a
nest is not a fight, and a roll that placed one would leave the arena with
nobody to fight. A boss brings its escort exactly as in play, because
`spawn_pack` is the function that decides that.

*Stack* — `descend_to(depth, frames, entrance)` with the player's tile as
the entrance, then `stack_encounter_pack()` verbatim. `frames` is
`frames_at(entrance).max(depth)`, so the frame is as authentic as the link
would make it while still reaching the depth asked for.

The descent is a real one: it goes through `Game::enter_frame`, which
`CLAUDE.md` records as the one way into a frame. `descend_to` widens from
private to `pub(crate)` for this; nothing else in the Stack machinery moves.
The party is genuinely underground for the fight, so the depth stat
multiplier, `StackSpawn` tagging, Trace accrual and a boss's Portal Fragment
drop all apply as they do in play.

**3. Group it.** `game.group_pack(pack)`.

A rolled pack **is** the game's own fight, so it takes the game's own
capping — the opposite of an authored composition, which
`build_opponents` deliberately leaves uncapped because explicit authoring is
the point of a tester. `stage` then hands the groups to the same
`begin_battle` tail it uses today, so the `Watch`, the transcript and the
outcome reading are one code path for both kinds of scenario.

A rolled encounter therefore produces **no warnings**: nothing was asked for
past a ceiling, because nothing was asked for.

## Three stated consequences

Each of these is a real limit of the design rather than a defect to fix
later, and each belongs in `dev-arenas/README.md`.

**Zone 1 field is the opening ring.** The arena's player stands at the
danger origin, so `in_opening_ring` gentles the pool to what a fresh player
can beat and clears bosses out of it. That is the honest answer to "what
does zone 1 field throw at a new run", and it is the fight
`opening-fight.ron` already pins. It also means zone 1's *ungentled* roster
is unreachable from this picker. An "out in the wild" flag that moved the
roll past `OPENING_RING_TILES` would open it, and is deliberately not in
this change.

**A field roll is one habitat spawn roll**, so an ordinary one fields a
single species group. In play, `gather_pack` can pull an adjacent second
cluster into the same fight — reproducing that needs a populated zone, and
inventing a "one species pick per group the curve allows" rule for the
surface would be copying the Stack's rule onto ground that does not have
it. The Stack path has no such gap: `stack_encounter_pack` *is* that rule,
and it is called rather than reimplemented.

**A biome with no habitat species cannot be picked.** The picker offers only
biomes that are `walkable()` and have at least one `SpeciesDb::
habitat_matches` — the same two clauses `habitat_pools` itself early-returns
on. Reading it off the live species db rather than hardcoding a list means a
mod that adds the first StaticField resident gets it offered for free, and
`Platform` stays absent for the reason the `Biome` enum documents: no
shipped species lives on a base slab.

## The seeding change this forces

`stage` currently installs `GameRng(seed)` **after** building the opponents.
Authored opponents are therefore spawned from `Game::new(0)`'s stream: their
potential rolls and wild routines are identical in every rep, and only the
battle varies with the seed.

A rolled encounter cannot live with that — the composition *is* the thing
being sampled. The seed install moves to immediately after `build_player`,
uniformly for both kinds of scenario, so a rep samples the composition and
the fight together.

**This shifts existing arena numbers once.** A loss seed pinned from an old
report will not replay the same fight. The alternative — two seeding orders
inside one function, chosen by which kind of scenario it is — is the kind of
split that later drifts into the two halves disagreeing about the RNG
stream, which is exactly what `staging_then_running_matches_run_at_the_same_
seed` exists to prevent. One order, stated.

## What changes on screen

**Builder** (`crates/app-core/src/app/arena.rs`). A new
`Encounter: Authored / Field / Stack` row, cycled with Left/Right like the
player source. When it is not `Authored`:

- a `Biome:` row, Enter opening the picker (a fifth `ArenaPickKind`);
- for `Stack`, a `Depth:` row nudged with Left/Right, minimum 1;
- the `Against:` rows and `+ add an opponent group` hide, exactly as the
  loadout rows already hide for a non-`Fresh` player — and for the same
  reason, that `validate` refuses a file holding both.

Switching to a rolled encounter clears `opponents`, following
`cycle_arena_player_source`'s precedent of clearing rows that have just gone
off screen. Switching back seeds one opponent row — the catalogue's first
species at a count of 1, the same thing the picker would append — so every
state the cycle can reach is a state that can be saved and reloaded.

**Report and result screen.** `RepRecord` gains
`composition: Vec<(String, u32)>`, derived from the staged groups. With a
rolled encounter every rep fights something different, and the transcript
names only the front group — without this the report cannot be read at all.
`render/arena.rs` draws it above the transcript; the `arena` bin prints it
per rep at `reps: 1` and leaves the aggregate alone.

## Testing

Engine:

- a scenario with `encounter` parses, round-trips through `save`/`load`, and
  defaults to `None` on a file that omits it;
- `encounter` beside a non-empty `opponents` is an error naming both;
- a `Field` roll at a named biome fields only species that list that biome
  as a habitat, asserted over the real assets;
- a `Stack` roll leaves the party underground at the depth asked for and
  every member carries `StackSpawn`;
- **depth scales stats**: the same biome and the same seed at depth 1 and
  depth 5 pick the same species (one RNG stream, same draws) and the deeper
  pack has strictly more total HP. Deterministic rather than sampled, so it
  cannot flake;
- a rolled encounter reports no warnings, and no group exceeds
  `group_size_ceiling`;
- zone 1 field fields only species `beatable_by_a_fresh_player` — the
  opening-ring consequence, pinned rather than left to be rediscovered;
- an empty-pool biome is an error naming it;
- `staging_then_running_matches_run_at_the_same_seed` holds for a rolled
  scenario as well as an authored one.

App-core:

- the `Against:` rows disappear when the encounter is rolled and come back —
  with a row present — when it is cycled back to `Authored`;
- the biome picker offers only fieldable biomes;
- `Depth` appears for `Stack` and not for `Field`.

Gates: `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`.
`balance_sim` is untouched by this change and is not a gate for it — no
tuning constant moves.

## Out of scope

- The "out in the wild" flag for zone 1 discussed above.
- A Trace band knob. Trace is zero in a fresh arena game and a save brings
  its own; exposing it is a separate lever with its own screen row.
- Rolling a *lair guardian* rather than an ambush. `rouse_lair` is the third
  underground spawn site and a legitimate future variant, but it is a
  different question from "what would I encounter walking a frame".
- Any change to `balance_sim`, `tuning.rs`, or the shipped scenarios.
