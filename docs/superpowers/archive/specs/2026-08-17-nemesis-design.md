# Nemesis

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

> `INDEX.md` warns that this header is the one line in a spec nobody ever
> revises. Answer "did this ship" from `CHANGELOG.md` and a grep, never from
> here.

Closes `TODO.md` #24, "nemesis".

## The problem

Every wild program is interchangeable. You fight one, you win or you don't,
and either way the encounter leaves nothing behind — a fight you lost is
indistinguishable a minute later from one that never happened. The zone has
no memory of you, and you have no reason to remember any particular thing
in it.

A nemesis is the game remembering one fight. The program that beat you, or
that you bailed out on, keeps standing there. It gets a name, it gets
stronger, it says something about it when you meet it again, and you can
pick it out of the map from across the sector.

## Vocabulary

Player-facing and code-facing alike: a **nemesis**. Plural **nemeses**.

- component `components::Nemesis`
- content `assets/nemesis/names.ron`, `assets/nemesis/taunts.ron`
- constant `tuning::MAX_NEMESES`

No new `Mode`, no new screen, no new menu row. A nemesis is a property of a
creature that already exists, drawn where that creature is already drawn.

## The trigger is one predicate, at the one place a fight can end

`Game::end_battle` (`game/combat_teardown.rs`) already computes the win as
`battle.groups.is_empty()`. That expression is read off the **opponents**,
never off the player, because a Forgiving defeat is absorbed mid-round by
`difficulty::death_handling_system` — which reboots the player, so their HP
afterwards says nothing about the outcome. The existing telemetry `won`
field is that same read.

The nemesis rule reuses it verbatim:

> **If the groups are non-empty when the battle tears down, every living
> hostile in the fight is marked.**

Both of the triggers this feature wants fall out of that one expression with
no branching. Being carried out (`tick_round_status_effects` ends the battle
when the player is not alive) and bailing out (the jack-out path in
`combat_teardown.rs`) are the same fact from the hostiles' side: the party
walked away from something still standing. There is deliberately no
`fled: bool` parameter and no per-call-site marking — the four `end_battle`
call sites are untouched, and a fifth cannot forget to mark.

A won fight leaves the groups empty and marks nobody, which is the whole of
the "you have to lose to earn one" rule.

### Where in `end_battle` this goes, and the limit that falls out

**After the `StackSpawn` stray sweep, and before `BattleState` is removed.**
That is a genuine window rather than a preference, and it is narrow:

- The sweep despawns every `Without<Tamed>` `StackSpawn` survivor, because a
  Stack pack that outlived the fight stands at surface coordinates around
  the link mouth and would be waiting there when the party climbs out.
  Marking above it would mark entities about to be despawned.
- `BattleState` is removed a few lines later, and it is the only record of
  *which* hostiles were in this fight. Marking below it would leave nothing
  to iterate but every hostile in the zone.

The `retain_outcomes_since_battle` prune sits between the two. The marking
pass must go above it if any of its lines should reach the map, or below it
if none should — the marking itself logs nothing, so this is free, but an
implementer adding a "you have made an enemy" line needs to know the prune
is there.

So a nemesis is **surface-only**, and this is a stated limitation rather
than an oversight. A lair guardian that beats you underground is despawned
before it could carry a mark. Placing the marking pass below the sweep makes
that a consequence of code order rather than a rule anyone has to remember
or a guard anyone can delete.

Making grudges work underground needs a *record*-based nemesis — species,
name and grudge count in a saved resource, respawned on demand — which was
considered and refused for this pass. It is separable and can be built later
without unpicking any of this.

### The arena

`arena::stage` runs real fights through the real `end_battle`, so arena
hostiles will be marked. This is harmless: the arena's `Game` is discarded,
touches no disk, and its creatures never reach a save. The marking pass must
therefore **spend no `GameRng` draw** (see the name derivation below), or
every arena scenario's stream would shift. With that held, an arena report
is unaffected by this feature.

## The mark

```rust
/// How many times this program has sent the party away from a fight.
#[derive(Component)]
pub struct Nemesis(pub u32);
```

Inserted on a hostile the first time it is marked, incremented on every
subsequent one.

Three things happen at each mark, in this order:

1. **Promote one rarity rung.**
2. **Recharge to the new `max_hp`.**
3. **Name it, if it has no name yet.**

### Promotion is the second and last place a rarity multiplier touches `Stats`

`Game::spawn_wild_creature_scaled` (`game/spawning.rs`) bakes
`Rarity::stat_mult` into `Stats` at spawn, and the `Rarity` component that
rides along is a **receipt** for a multiplier already spent. `Rarity`'s own
doc and `CreatureSave::rarity`'s are both emphatic that nothing downstream
may apply it a second time — a second application is invisible and compounds
on every reload.

Promotion is therefore not "apply the new tier". It is a new

```rust
pub(crate) fn promote_rarity(&mut self, entity: Entity) -> Rarity
```

which multiplies all four stats by the **ratio**
`new.stat_mult() / old.stat_mult()` and writes the new tier into the
receipt. `spawn_wild_creature_scaled`'s comment ("Rarity multiplies here and
exactly here") must be amended to name this as the one other site, or the
next reader will believe the invariant is stronger than it is.

**The ceiling is `Rarity::ALL`'s own top.** A `Prismatic` nemesis promotes to
nothing; only its grudge count rises. There is no `MAX_NEMESIS_RARITY`
constant, because a second ceiling could disagree with the ladder, and
because the ladder is already the bound the feature needs:

| step | multiplier | cumulative from Ordinary |
|---|---|---|
| Ordinary → Silver | ×1.50 | 1.50 |
| Silver → Gold | ×1.20 | 1.80 |
| Gold → Platinum | ×1.11 | 2.00 |
| Platinum → Prismatic | ×1.075 | 2.15 |

This is a **decelerating, bounded** ladder, not a compounding one. The seam
doc's rule — "every difficulty curve in the game is linear, and that is a
correctness property", because a geometric enemy curve racing a linear
player curve ends past `MIN_DAMAGE` — is not violated: a nemesis cannot
exceed 2.15× whatever it spawned at, no matter how many times it wins.

The first step is by far the largest, and that is the intended feel: the
thing that beat you is meaningfully worse news now.

**The escalation is safe to leave uncapped in this sense because a nemesis
cannot force the rematch.** `systems::wander_ai_system` moves hostiles and
never initiates combat, and `Pursuing` is only ever inserted alongside
`NestGuardian`. The one thing that *can* force an engagement is
`Game::nest_aggro_tick` — CLAUDE.md flags it as the first code to call
`start_battle` from inside `tick_inner` — which re-engages any `Pursuing`
guardian still adjacent to the player. A mark can land on exactly such a
guardian, promoted and healed to full in the same breath, and a Forgiving
defeat with no structure to warp to leaves the player standing right where
they fell — adjacent to it, on the very next tick. `mark_nemeses` closes that
off by dropping `Pursuing` from every hostile it marks, the same shake
`battle_flee` already performs on a successful jack-out. `NestGuardian` stays
untouched, so a cleared guardian keeps its tether and just resumes ordinary
wandering; the nest re-provokes it the next time `attack_nest` lands a hit.
That is what makes "engaging a nemesis is the player's decision, every time"
true rather than a caveat.

### The recharge

After promotion, `hp = max_hp`. It raises HP, so it goes nowhere near
`Game::apply_damage`'s rule that it is the only code path that lowers a
creature's HP.

### Gear cannot be sitting in these stats

The seam doc forbids any stats operation running while a gear bonus sits in
`Stats`, because scaling would weld the difference permanently into the base.
`spawn_wild_creature_scaled` inserts no `Equipment`, and `CreatureSave`'s
`equipment` field exists for programs the *player* owns. Promotion only ever
runs on a living `Hostile`, so the hazard is unreachable — and a test pins
that rather than leaving it to the reader.

## The cap

`tuning::MAX_NEMESES`, initially 10.

Counted by querying live `Nemesis` holders. **No resource, no bookkeeping,
no save field for the count** — the entities are the ledger.

At the cap, **no new marks are handed out, but an existing nemesis still
escalates.** That asymmetry is the point: it means there is never a
demotion path, and a demotion would mean *reversing* a stat multiply, which
is the one operation the rarity receipt is least able to survive.

The cap is far enough above the number a run realistically holds — nemeses
die when you kill them and are wiped wholesale by a breach — that it reads
as a runaway backstop rather than as a rule the player will feel.

## The name and the taunt

Both come from `assets/nemesis/`, following `assets/descriptions/`'s shape:
a directory of `.ron` line banks, a `README.md` documenting the schema, and a
loader that skips a malformed file with a logged warning rather than
panicking at startup — the pattern `SpeciesDb::load_dir` and friends already
set.

- `names.ron` — the pool a nemesis name is drawn from.
- `taunts.ron` — what a nemesis says when a fight with it opens.

One shared bank rather than per-species fields. Only 4 of 17 shipped species
author `SpeciesDef::taunts` today, so per-species nemesis lines would read
generic for most of the roster while costing a schema change. A species
override is a clean later addition if the shared voice turns out to be too
flat, and adding one does not invalidate anything here.

### Selection spends no RNG draw

Neither pick may touch `resources::GameRng`. Two reasons, and the second is
the load-bearing one:

- A draw does not survive a save/load and shifts every later roll in the run.
- A draw inside `end_battle` runs in **every arena fight**, shifting the
  stream for every scenario in `dev-arenas/`. Arena numbers already only
  compare within one build; there is no reason to spend that.

So selection folds the values it derives from and reduces with
`derive::index` — Lemire's `(seed as u128 * len) >> 64`, the shared reducer
that exists precisely so nobody writes `% pool.len()` again. That module's
doc carries the full argument: `%` on a two-entry pool reads nothing but the
seed's lowest bit, which the multiply-by-odd-prime every fold ends on
provably never disturbs.

**Reaching the high bits is the caller's problem**, and this spec is not
exempt. Fold the inputs a byte at a time, FNV-1a style, the way
`sectors::sector_seed` folds the zone number and for the same measured
reason — one XOR-then-multiply round carries a difference only about the
prime's width (~41 bits) upward, so a value folded in as the last word and
differing only in its low bits never reaches bit 63.

The seed is built from the creature's identity, not from the moment: its
`Potential` rolls and its species id. Two nemeses of the same species get
different names because their rolls differ; the same nemesis re-derived
gets the same name, though nothing depends on that — see below.

### The name lives in `CustomName`

The derived name is written to the existing `components::CustomName`, which
already saves, and which `Game::creature_label` /
`Game::entity_label` already prefer over the species name everywhere a
creature is named — map, examine, battle roster, log lines.

Two consequences, both wanted:

- A nemesis you eventually decompile joins your roster still wearing the
  name it earned.
- `Game::rename_companion` can overwrite it afterwards, which is correct: at
  that point it is yours.

`CustomName`'s doc comment currently says "the player's custom display
name". It needs widening to name this second author, or the next reader will
believe `rename_companion` and `fuse_companions` are the only writers.

Because the name is *stored*, its derivation does not have to be stable
across a `rand` upgrade or a bank edit — unlike a Stack description, which
is derived fresh on every read. The derivation is byte-stable anyway, since
it is arithmetic rather than an `StdRng` sequence, but nothing depends on
it and the spec does not promise it.

### The taunt

Fired from `Game::begin_battle` when any hostile in the opening groups
carries `Nemesis`, as `MessageKind::Info`.

`Info` is deliberate and matches `game/taunt.rs` exactly:
`MessageLog::retain_outcomes_since_battle` keeps only `Outcome`, `Loot`,
`LevelUp`, `Raid` and `Complete`, so the taunt is pruned when the fight ends
and trash talk does not follow the player onto the map. It belongs to the
fight it was said in.

Selection indexes the taunt bank by the grudge count folded together with
the name seed, so a nemesis with a longer history says different things
rather than repeating one line forever.

## The map

`views::EntityView` gains:

```rust
/// Whether this (creature) entity has beaten the party or driven them
/// off — see `components::Nemesis`.
pub nemesis: bool,
```

Two marks, both requested:

1. **A distinct tile mark**, drawn in `crates/gui/src/render/base.rs`
   alongside the rarity bar it already paints along the tile's top edge.
2. **A reserved glyph colour**, returned by
   `game::inspection::difficulty_color` for a nemesis regardless of power
   ratio — the same override shape it already has for a boss, which is
   always magenta.

The species **glyph character is unchanged**. It is the only thing on the
tile that says *what* the nemesis is, and a nemesis you cannot identify the
species of is worse to meet, not better.

The mark's exact form is left to implementation, under two constraints: it
must not collide with the rarity bar along the top edge, and it must not
collide with the outlines `base.rs` draws for a structure's production
links. It is a visual call best made with the map on screen, and the
playtest below exists partly to make it.

### What the recolour costs

`EntityView::rarity`'s doc records the existing rule: the map draws rarity
as a bar rather than by recolouring the glyph, because `color` is already
carrying `difficulty_color` for a hostile, and how dangerous something is
and how rare it is are two readings the glyph cannot both hold.

Overriding the colour for a nemesis spends that reading. A nemesis's tile
stops telling you whether you can take it. This is accepted deliberately:
you have fought this one, you know what it did to you, and it has since
gotten stronger — the con colour is the least informative thing its tile
could be saying. But it is a real loss and the doc comment must be amended
to say so, rather than left claiming a rule this breaks.

`difficulty_color`'s boss override is the precedent for a non-power reading
winning the glyph, so this does not introduce a new kind of exception.

## Rewards

**None beyond what the promoted rarity already pays.** Loot and XP flow
through the existing curves — `progression::kill_xp` prices a kill by
challenge against the player's power alone, so a promoted nemesis is
automatically worth more, and `Rarity` already feeds the drop tables.

No achievement, no guaranteed drop, no new number. Every one of those would
be a magnitude `balance_sim` cannot see, bought for flavour the name and the
taunt already deliver.

## Save

One additive field on `save::CreatureSave`:

```rust
/// How many times this program has driven the party out of a fight — see
/// `components::Nemesis`. Absent on an ordinary program.
#[serde(default)]
pub nemesis_grudges: u32,
```

**No `SAVE_FORMAT_VERSION` bump.** The payload has been field-named RON
since version 29, and that entry says in terms that it should be the last
one an additive change needs: a field behind `#[serde(default)]` loads out
of a file written before it existed.

Nothing else is saved. The rarity promotion is already in the saved `Stats`
and the saved `rarity` receipt; the name is already in the saved
`custom_name`. The grudge count is the only genuinely new state, and it
exists so the taunt can escalate.

## What this deliberately does not do

Each of these falls out of the entity-bound choice rather than being an
independent decision, and each is separable if it turns out to be wanted:

- **A nemesis does not survive a breach.** `Game::enter_next_zone` despawns
  everything `Hostile`, and nothing is added to exempt them.
- **A nemesis does not exist underground** — see the stray sweep above.
- **A nemesis does not hunt you.** No `Pursuing`, no aggro radius, no
  tracking. The rematch is always the player's choice.
- **A nemesis is not announced when it spawns**, because it does not spawn.
  It is a program that was already there.
- **There is no nemesis screen, list or menu row.** The map, the examine
  line and the battle roster already name it.

## Testing

Engine unit tests, in a new `crates/engine/src/tests/nemesis.rs`. Fixtures
from `crates/engine/src/tests/support.rs` —
`start_battle_with_a_wild_program` is the shape most of these want.

**The trigger**

- A won fight marks nobody.
- A jack-out marks the surviving hostiles.
- A Forgiving defeat marks the surviving hostiles.
- A Stack fight marks nobody, because the strays are swept first.

**The promotion**

- An `Ordinary` nemesis lands on `Silver` with all four stats multiplied by
  exactly `SILVER_STAT_MULT`.
- A second grudge multiplies by the **ratio** to `Gold`, not by
  `GOLD_STAT_MULT` again. This is the compounding bug the ratio exists to
  prevent, and the test must fail if `promote_rarity` applies the tier
  rather than the step.
- A save/load round trip does not re-apply the multiplier — the mirror of
  `CreatureSave::rarity`'s existing guard.
- A `Prismatic` nemesis does not promote, and its grudge count still rises.
- HP is full after promotion, at the *new* `max_hp`.
- Promotion never runs on an entity carrying `Equipment`.

**The cap**

- The 11th distinct program to win is not marked.
- An already-marked nemesis still escalates while the cap is full.

**Name, taunt, bank**

- Marking spends no `GameRng` draw: the resource's state is byte-identical
  across a marking pass. (This is what protects every arena scenario, so it
  is not a nicety.)
- A name is derived and stored once; a second grudge does not rename.
- Two nemeses of the same species with different `Potential` rolls get
  different names.
- An empty or malformed bank file is skipped with a warning and does not
  panic — and a nemesis with no name available is still marked, promoted and
  recharged.

**The map**

- `EntityView::nemesis` is true for a marked hostile and false otherwise.
- `difficulty_color` returns the nemesis colour regardless of power ratio.
- A breach clears every nemesis.

**Gates**

`cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`. Plus
`cargo test -p feral-processes-engine balance_sim` because this moves enemy
stats — though `balance_sim` has no nemesis term and models one run's curve,
so it can only catch collateral damage, not this feature's own balance.

Note the trap: a seeded engine test can pass under
`-p feral-processes-engine` and fail under `--workspace`, because they are
different builds with different RNG streams. `--workspace` is the gate.

## Playtest

The green suite is not evidence this feels like anything. Two questions only
play can answer:

- **Is +50% on the first promotion the right size?** It is the largest step
  on the ladder and lands on a player who just lost.
- **Does the map read?** A nemesis mark plus a rarity bar plus a reserved
  glyph colour is three things on one tile, and the con colour is gone.

Capture a `dev-saves/` template with a live nemesis on the map once one
exists, so the second question does not cost a lost fight to ask again.
