# The battle arena — design

**Date:** 2026-08-07
**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header.

## The problem

There is no way to run a battle without playing to it.

Tuning a fight today means one of two things. Either you start the game,
grind to the level and zone where the fight exists, find the species you
want to look at, and hope it spawns in a group of the size you were asking
about — or you reach for `balance_sim.rs`, which answers instantly and
answers a different question. The sim is a pure projection: no RNG, no
initiative, no abilities, no items, no status effects, mean move power in
place of a roll. Its own module doc says so, and the consequence is that
ability magnitudes are ungated by it, and always have been.

So the two tools available are "the real fight, at the cost of an hour of
play" and "an instant answer to a simplified fight". What is missing is the
real fight, on demand, repeatable, with the composition chosen rather than
rolled.

That gap is what makes tuning expensive. `tuning.rs` says difficulty is code
rather than data precisely so it can be reasoned about — but reasoning needs
measurements, and the only measurement currently cheap enough to take is the
one that models no abilities.

## What this is for, in order

Ranked deliberately; the ranking decides the tie-breaks below.

1. **Measuring a fight the game can really produce.** Real damage rolls,
   real initiative, real enemy AI, real status effects, real gear. If the
   arena and the game disagree about a number, the arena is wrong.
2. **Repeatability.** The same scenario file run after a `tuning.rs` edit
   must be comparable to the run before it. That means seeded, and it means
   the scenario is a file rather than a series of keystrokes.
3. **Sampling.** One fight is an anecdote. Fifty fights against the same
   pack is a win rate, and a win rate is what a difficulty knob moves.
4. **A transcript worth reading afterwards.** Not just an outcome — the
   round-by-round narration, in the game's own words, kept somewhere it can
   be post-processed later.

Nothing here produces a save. The arena loads state and throws it away; a
scenario cannot corrupt a real run because it never writes one back.

## Decisions

### The arena lives inside the engine crate

`Game::start_battle` is `pub(crate)` (`game/combat.rs:148`), and so are
`spawn_wild_creature_scaled` and `group_pack`. Nothing outside the engine can
construct an arbitrary fight, and the `world` field is private with no
accessor — which `CLAUDE.md` names as the compiler barrier that keeps the
renderer out of the ECS.

A new `pub mod arena` in `crates/engine/src/arena.rs` sits on the inside of
that barrier. It reaches `start_battle` and `world` because it is in the same
crate, and it therefore **adds no public `Game` method at all**. The rule the
project cares about is untouched: the renderer still cannot reach the
`World`, and still would not compile if it tried.

The alternatives were both worse. A bin under `crates/engine/src/bin/` is a
separate crate and hits exactly the same `pub(crate)` wall, so it would have
needed the public accessor anyway. Feature-gating a public seam
(`#[cfg(feature = "arena")]`) reads well until you notice cargo unifies
features within a build — the launcher would enable it, the game would get it,
and the gate would be decoration.

### The driver bin lives in the launcher, beside `savetool`

`crates/launcher/src/bin/arena.rs`, a third bin in the same package. Two
reasons. The launcher's `default-run` already resolves the ambiguity a second
bin creates, and that reasoning is documented in its `Cargo.toml`; adding a
third costs nothing new. And `dev_template` — `known`, `working_copy`,
`generate` — lives in the launcher's `[lib]`, so a bin there can let a
scenario name a template as well as a save path, which is the difference
between `Template("extraction")` and hunting for a `.bin`.

Cargo compiles per crate, not per bin, so this adds no build time beyond what
`savetool` already pays.

### The party plays the game's own All-Attack

Every round the arena calls `battle_plan_remaining(Attack { group: 0 })` and
then `battle_resolve_round()`, which is what `App::plan_every_slot`
(`app-core/src/app/battle.rs:154`) does when the player presses `[A]`. It is a
real in-game command, not a policy engine written for the tester — so the
arena cannot drift from the game by inventing decisions the game never makes.

The rejected alternative was a set of built-in policies (use-specials,
defend-under-30%). Each one is a hand-written AI that then has to be trusted
and maintained, and none of them is what the player does either. Scripted
per-round actions were rejected for now on the same grounds as YAGNI: they
desync the moment a fight runs longer than the script.

**This is the design's known blind spot, and it is stated rather than
hidden.** All-Attack fires no companion Specials, so ability magnitudes stay
unmeasured — the same gap `balance_sim` has. An arena number is a
floor-of-the-party's-output number. Scripted actions are the natural
follow-up if that gap needs closing; they are not in scope here.

### Opponents are spawned for real, and grouped by hand

Each opponent goes through `spawn_wild_creature_scaled` on the player's tile.
That is the game's own wild spawn: `ZoneLevel::stat_multiplier` applies, the
potential roll applies, wild routines are rolled, and the entity gets
`Hostile`, `WanderAi`, `ZonePortal` and `StatusEffects` exactly as a
map spawn would. A scenario names a species and a count; it does not name
stats, because stats are the thing under test.

The groups, however, are built directly from the scenario rather than by
handing the pack to `start_battle`. `group_pack` truncates to
`group_size_ceiling() × enemy_group_ceiling()`, both derived from the zone —
so a scenario asking for nine opponents at zone 1 would silently be given
one, and the tool would answer a question nobody asked.

**Decision: bypass the cap, and warn on stderr when the composition exceeds
what that zone could field.** Explicit authoring is the entire point of a
tester, and "what if zone 1 threw nine at me" is a legitimate tuning
question. But a silent cap is the failure mode `CLAUDE.md`'s "no silent caps"
rule names, so the warning is not optional — it names the ceiling, the ask,
and the zone.

### One rep, one seed

`GameRng` is reseeded to `scenario.seed + rep` at the start of every rep. Two
properties fall out. Reps genuinely differ, so fifty of them are a sample
rather than fifty copies. And the whole run reproduces from a single number
in the file, so a loss found at rep 37 can be replayed on its own by pinning
that seed.

This is also why the arena does not draw from a save's RNG stream: `CLAUDE.md`
records that `GameRng`'s stream position is not persisted, so a loaded save
has no reproducible position to inherit.

### The transcript is captured per round, not at the end

`end_battle` calls `MessageLog::retain_outcomes_since_battle`, which deletes
the blow-by-blow and keeps only `Outcome`, `Loot`, `LevelUp` and `Raid`. A
tester that read the log once the fight was over would get results and no
narration — which is precisely the thing being asked for.

So the arena calls `game.battle_log()` after each `battle_resolve_round()`
and accumulates. `MESSAGE_LOG_CAP` is a second reason to do it this way: a
long fight can drop lines off the front before it ends.

### Save import is wholesale; `Fresh` is where you pick items

Three player sources:

- `Save("saves/save.bin")` — `Game::load`, whole run state as it stands.
- `Template("extraction")` — `dev_template::generate` into a temp copy, then
  `Game::load`. The `dev-saves/` library already exists for exactly this
  ("don't reach for a fresh `Game::new` when a template would do").
- `Fresh(level: N, zone: N)` — `Game::new`, then an authored loadout.

A save or template brings across everything: level, stats, equipment *and its
`EquippedItem::fusion_tier`*, party, perks, zone, Power and Fatigue. The
scenario names only the opponents. This is the "what would happen if I hit
this pack right now" question, and overriding pieces of it would make the
answer mean something else.

`Fresh` is the other half — the "pick items" the tool exists for — and only
`Fresh` accepts `equip`, `inventory` and `party`. Applying those on top of a
save is deliberately not built: it doubles the schema's surface to answer a
question ("this save, but with different gear") that `Fresh` already answers
from a clean footing. The door is open if it turns out to be wanted.

### `set_level` and `spawn_tamed` move out of the test fixtures

`Fresh` needs to level a player and to stand up companions. Both already
exist in `crates/engine/src/tests/support.rs` — `set_level` (which correctly
calls `install_unlocked_routines`, so a raised level installs the kit a real
level-up would) and `spawn_tamed`. They are `#[cfg(test)] pub(super)`.

They move to a `pub(crate)` home both callers share, and `support.rs`
re-exports them. Copying them into `arena` instead would put two versions of
"what it means to be a level-N companion" in the tree, which is the exact
shape `CLAUDE.md`'s "a doc comment claiming to mirror other code must be a
call, not a copy" rule exists to prevent — and `install_innate_routines` is
already recorded as the step a duplicate dropped once.

## The scenario schema

RON, loaded from a path given on the command line. `dev-arenas/` is the
checked-in library, mirroring `dev-saves/`, with a `README.md` documenting
the schema — the same obligation `assets/*/README.md` carries.

```ron
(
    player: Template("extraction"),
    opponents: [
        (species: "sub_process", count: 9),
        (species: "glitch", count: 4),
    ],
    reps: 50,
    seed: 7,
)
```

A `Fresh` scenario, showing the fields only it accepts:

```ron
(
    player: Fresh(level: 20, zone: 3),
    equip: [
        (item: "monofilament_whip", tier: 0),
        (item: "ablative_plating", tier: 2),
    ],
    inventory: [(item: "power_cell", qty: 5)],
    party: [
        (species: "scrapper", level: 14),
        (species: "scrapper", level: 14),
    ],
    opponents: [(species: "sub_process", count: 9)],
    reps: 50,
    seed: 7,
)
```

### What `opponents` controls, and what it does not

The list is species and count, and both are honoured verbatim (that is what
the cap bypass above buys). Two properties of it are not obvious from the
syntax and belong in `dev-arenas/README.md` as well:

- **Order is formation.** `ENGAGED_GROUPS` is 2, so only the first two
  entries are in melee range. A third or fourth group acts only if its
  species has a ranged move, and sits inert otherwise. Reordering the list is
  therefore a tuning lever, not a cosmetic choice. `MAX_ENEMY_GROUPS` is 4
  and `MAX_GROUP_SIZE` is 100; past those the fight is not one the game can
  represent at all, so they are a hard error rather than a warning.
- **There is no per-enemy level.** A wild spawn carries no `Experience` —
  `spawn_wild_on_player_tile`'s own comment records that a wild pack member
  has neither `Tamed` nor `Experience`. How hard one hits comes from
  `ZoneLevel::stat_multiplier` and its potential roll, so the zone is the
  strength dial and `count` is the volume dial. A scenario that wants a
  tougher individual raises the zone or names a tougher species.

Rules, following the ones the asset loaders already follow:

- Every field is `#[serde(default)]`, so a scenario written today keeps
  parsing after a field is added. `reps` defaults to 1, `seed` to 0.
- A malformed scenario is a printed error and a non-zero exit code, never a
  panic — the `SpeciesDb::load_dir` pattern.
- An unknown species id, item id or template name is an error naming what was
  not found, not a silent skip. A scenario is authored, not scavenged; a typo
  should stop the run rather than quietly change the fight.
- `equip`, `inventory` and `party` on a `Save` or `Template` scenario are an
  error rather than being ignored, for the same reason.

## Output

**`reps: 1`** prints the transcript round by round, in the game's wording,
followed by the outcome line.

**`reps > 1`** prints an aggregate: win rate, mean and median rounds, mean
player HP fraction remaining, companions downed per rep, and **the seeds of
the losses**. The last of those is what makes the summary actionable — a bad
rep can be replayed alone by pinning its seed.

**Always** writes a structured report, default `arena-report.ron`, redirected
with `--out`. Per rep: seed, won/lost, rounds, player HP fraction, companions
downed, and the full transcript. RON because it is already a dependency and
already the repo's data format; this is the hook for working with the logs
later.

## Testing

Engine unit tests, in `arena`'s own `mod tests`:

- **Determinism.** The same scenario run twice produces an identical report.
  This is the property the whole tool rests on; without it a tuning
  comparison measures noise.
- **Per-rep divergence.** A scenario with `reps: 20` against a genuinely
  marginal pack does not produce twenty identical transcripts — the reseed is
  doing something.
- **A lopsided fight resolves the expected way,** in both directions: an
  overwhelming party wins, an overwhelming pack does not.
- **The transcript survives `end_battle`.** A won fight's report holds
  round narration, not just the results `retain_outcomes_since_battle` keeps.
  This is the regression that matters most, because the naive implementation
  passes every other test and returns an empty transcript.
- **Aggregation arithmetic** over a hand-built set of rep records.
- **Malformed RON is an `Err`,** as are an unknown species id and `equip` on
  a `Save` scenario.
- **The cap warning fires** when a composition exceeds the zone's ceiling,
  and the pack is still built at the size asked for.

Launcher bin tests follow `dev_template`'s three: argument parsing, and that
a missing scenario file is a clean error rather than a panic.

`cargo test --workspace` is the gate, per `CLAUDE.md`.

## What this deliberately does not do

- **No companion Specials**, per the All-Attack decision above. Stated in the
  `dev-arenas/README.md` too, so a reader of a report knows what it measured.
- **No saves out.** Nothing the arena does is written back. This is what lets
  it load a real save without risk.
- **No new tuning knobs.** The arena measures `tuning.rs`; it does not add to
  it.
- **No GUI.** A hidden debug screen would be higher fidelity to what you see
  on screen, but it cannot batch reps and it would touch `gui` and
  `app-core` — two more crates, for a tool whose output is a number.
- **No balance regression gate.** `balance_sim.rs` remains the gate; its
  tests are RNG-free by design and an RNG-sampled win rate is the wrong
  shape for a build to fail on. The arena is a measuring instrument, not an
  assertion.

## Files

| File | Change |
|---|---|
| `crates/engine/src/arena.rs` | new — scenario types, rep loop, report, tests |
| `crates/engine/src/lib.rs` | `pub mod arena;` |
| `crates/engine/src/tests/support.rs` | `set_level`/`spawn_tamed` move out; re-export |
| `crates/launcher/src/bin/arena.rs` | new — CLI, output formatting |
| `crates/launcher/Cargo.toml` | third `[[bin]]` |
| `dev-arenas/README.md` | new — schema reference |
| `dev-arenas/*.ron` | new — a starting scenario or two |
| `CHANGELOG.md` | a `## X.Y.Z` section at the merge |

Two crates, so this warrants the spec-and-plan pipeline rather than inline
TDD, per `CLAUDE.md`'s process-weight rule.

## Open questions

Neither blocks implementation; both are noted so the reviewer can redirect.

1. **The cap bypass.** Decided above as bypass-with-warning. Respecting the
   cap instead would make every arena run describe an encounter the game can
   really roll, at the cost of being unable to ask "what if".
2. **Where scenarios live.** `dev-arenas/` beside `dev-saves/` is the
   proposal. `dev-saves/arenas/` would keep the dev-tooling surface to one
   directory.
