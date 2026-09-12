# Battle summons: forked programs

**Status:** designed, not implemented

Two routines fork a temporary program that fights beside you for the length of
one battle and is gone when it ends. One fields a single body, one fields two
or three. A repeatable perk widens the rarity window they roll on, so an
unperked fork is reliably below average and a fully invested one is not.

Works in both combat models.

## What a summon is

**A wild spawn on your side, not a roster program.**

It comes out of `Game::spawn_wild_creature_scaled` — so it gets a species, a
stat block, a `Rarity` and its species' own moves with no new machinery — and
then has `Hostile` and `WanderAi` stripped, the way `Game::adopt_program`
strips them. It does **not** pass through `Game::roster_parts()`.

That omission is the whole containment story, and every consequence below is an
omission rather than a check:

| absent | what that buys |
|---|---|
| `Tamed` | invisible to `pet_count`/`pet_capacity`, `base_staff`, labour scheduling, `rest`, memories, needs — and to the save, which writes only `Tamed` programs |
| `Experience` | `award_companion_xp` already early-returns without it, so a summon earns nothing and needs no exclusion |
| `ProgramId`, `Memories`, `Disposition` | no orphaned memories in other programs, nothing to reconcile on load |

It gains exactly two things:

- **`components::Summoned`** — a battle-scoped marker. `Cloaked`'s precedent:
  no save field, **no `SAVE_FORMAT_VERSION` bump**.
- **`PowerReserve`** — because `ability_unavailable` reads the reserve off the
  entity in question, and without one a summon could never run the moves it was
  spawned to run.

Spawned at an off-map sentinel tile, `resolve_sortie_battle`'s precedent, so a
body that somehow outlives teardown is not standing in the zone. `Position` is
read nowhere for a body in a fight in either model.

## Seating it

The two models fail differently, and neither failure is loud.

### Group model

`BattleState::planned` is sized once at `begin_battle` as `Party.len() + 1`,
but `roll_initiative` reads `Party.0.len()` **live**. So pushing to `Party`
alone produces a body that draws an initiative rung and then never acts —
`plan.get(slot)` is `None` — while `living_party`, `battle_rows`,
`battle_active_slot` and hostile targeting all iterate `planned.len()` and
cannot see it. It would be invisible and inert, and nothing would fail.

**A summon is appended to `Party` and `planned` together.** Appending is safe:
the seam forbids *removal*, which shifts every slot behind it. `actor_entity`
maps `Actor::Party(i)` to `Party.0[i-1]`, and append leaves every existing
index where it was.

Companions have no AI in this model — every party slot is player-commanded, and
`arena::run` is the only thing that ever plans a party body by itself. So a
summon needs its action chosen: **`Game::choose_summon_action`**, a weighted
pick over its own moves aimed at the front of an engaged group.

Deliberately **not** `Game::choose_wild_action`. That is the trained policy,
and its features speak group indices and aggro slots from the hostile side;
pointing it at the wild side is `tactical_sides`' relative-to-the-actor problem
without `tactical_sides`' payoff, and `combat_policy.rs`'s weights are pinned
against a design boundary this would quietly cross.

### Tactical model

`TacticalBattle::place` already refuses occupied and unwalkable cells, and
`deploy::nearest_free` is exactly the breadth-first search for a free cell
beside the invoker. What does not exist is **insertion into `initiative`** —
nothing has ever added a body after `set_initiative`, and the field is private
with a read-only accessor.

The cursor names a body rather than a position, so insertion carries
`TacticalBattle::remove`'s trap mirrored: inserting *ahead* of the cursor
shifts everything down one and a body acts twice.

**Insert immediately after the cursor.** Always behind it, so the cursor never
moves, no turn is lost or doubled, and it reads correctly at the keyboard —
you call it, it acts next.

`tactical_ai_actor` gates on `Hostile`, with the explicit doc "every party body
is the player's to command". A summon is the first exception to that, so the
gate becomes `Hostile` **or** `Summoned`. Sidedness needs nothing:
`tactical_sides` is already relative to the actor, which is why
`tactical_drive_turn` works at all.

## Dissolution

A new invocation replaces whatever you currently have forked — both routines
field *one set*, and a set replaces a set.

This collides head-on with "nothing may leave `Party` mid-battle". The
resolution: **a replaced summon is killed, not removed.** It stays in its slot,
dead, and the existing deferred-reap discipline carries it to teardown —
exactly what already happens to a companion that dies mid-fight. No new removal
path, and no despawned entity left dangling in `Party`.

`Game::finish_fight` gains a sweep of `With<Summoned>` — unconditional, living
or not, `resolve_sortie_battle`'s rule that "nothing of the opposition may
outlive this call" — beside the `With<StackSpawn>, Without<Tamed>` sweep
already there. One sweep covers all five ways a fight can end.

**One exclusion must be explicit.** `finish_fight` calls `bench_or_dissolve` on
each dead `Party` member, and that function
(`crates/engine/src/game/trade.rs:640`) inserts `Downed` **unconditionally** —
it never asks whether the body is `Tamed`. Left alone, every dead summon
becomes a downed program in `DownedPrograms` on Forgiving and takes
`dissolve_tamed_program` on Permadeath. The skip is on `Summoned`.

## The routines

A new effect variant:

```rust
Summon { count: u32, extra: u32, rarity_penalty: u32 }
```

The count rolls `count ..= count + extra`. Not `spread`, which is a *centred*
half-width on `Damage`/`Heal`/`Drain`; a body count has no reason to be
centred, and the shipped pair wants exactly 2..=3.

It follows **`Decompile`'s precedent exactly**: an `unreachable!` arm inside
`Game::use_ability`, and a branch at each model's Special site. Like a capture
it is not resolved over recipients and needs model-specific seating, which is
the same reason `Decompile` branches there.

- Shared half: **`Game::fork_programs(invoker, def) -> Vec<Entity>`** — dissolve
  the standing set, roll the count, roll the species, roll the tier, spawn,
  strip, insert, log. Returns the bodies.
- `Game::resolve_one_action`'s `BattleAction::Special` arm and
  `Game::run_tactical_routine` each seat what they are handed.

`target: WholeParty` — ally-facing, derived range 0..0, centred on the invoker
— with a **`summon_target_mismatch`** refusal added to `AbilityDb::load_dir`'s
eight, pinning the convention in the one place it is checked.

Deliberately **no new `AbilityTarget` variant**. The seam record on the two
combat models says a `SpecialTarget::Cell` variant would be "an invented answer
every abstract match in three crates would have to reject"; a `Summon` target
is the same shape, and `AbilityTarget`'s four methods are all exhaustive on
`cell_mark`'s rule.

Two asset files in `assets/abilities/`:

| file | count | extra | rarity_penalty | fields |
|---|---|---|---|---|
| `fork.ron` | 1 | 0 | 0 | one body |
| `fork_bomb.ron` | 2 | 1 | 1 | two or three, each a rung worse |

Both priced in Power — `every_runnable_routine_is_priced_in_power` has no
exceptions list — with `fork_bomb` heavily so. Both carry a cooldown.

## Species

You draw from **every non-boss species in the catalogue**, flat.

`SpeciesDb` today exposes only `get`, `windowed_matches` and
`windowed_boss_matches`, all biome- and step-gated. This needs one new method
returning every species whose `is_boss` is false, **sorted by id** —
`NeedDb::iter`'s rule, because an unsorted pool is an RNG-stream shift between
runs and the symptom surfaces as an intermittent failure somewhere unrelated.

## Strength

Three multipliers, all existing, composed **caller-side** so that
`a_spawns_stats_come_from_its_escalation_and_never_from_its_tile` stays true.

**"Your level"** is `Game::zone_curve_ratio(Game::party_band_progress() *
SUMMON_LEVEL_STAT_STEPS)`. `party_band_progress` is already the player's level
as a 0..1 fraction across the zone's level band — the game's own currency for
this, and expressed as a ratio on the zone ladder it is gated by `balance_sim`
for free, since `steps` of 1 at zone N is arithmetically zone N+1. Note it
reads the **player's** level specifically, so a companion-invoked fork scales
off the player too.

**"Below average"** is a flat `SUMMON_STAT_MULT` under 1.0 in `tuning.rs`.
`SETTLEMENT_GIFT_STAT_MULT: 0.6` is the precedent for handicapping a granted
program.

**The perk axis** is the rolled `Rarity::stat_mult()`. At 0.6 the whole ladder
is:

| tier | `stat_mult` | x `SUMMON_STAT_MULT` |
|---|---|---|
| Ordinary | 1.0 | 0.60 |
| Silver | 1.5 | 0.90 |
| Gold | 1.8 | 1.08 |
| Platinum | 2.0 | 1.20 |
| Prismatic | 2.15 | 1.29 |

Bounded even fully perked, and temporary.

### Setting the tier

Rarity is rolled and baked into `Stats` **inside** `spawn_wild_creature_scaled`,
and `Rarity`'s doc says nothing else may apply the multiplier — the one
exception being `Game::promote_rarity`
(`crates/engine/src/game/combat_teardown.rs:580`), which re-tiers an already
spawned body by the **ratio** between the old and new multipliers.

**Generalise `promote_rarity` into `Game::retier_rarity(entity, new)`**, with
`promote_rarity` kept as the one-rung-up caller the nemesis path already uses.
The `step` ratio is arithmetically symmetric, so it handles a downward move
with no new code.

Rejected: a seventh `rarity: Option<Rarity>` parameter on
`spawn_wild_creature_scaled`. It touches 13 call sites across six files, and
that function's parameter list is already the one `spawn_pack` grouped into
`SpawnEscalation` — growing it further is the wrong direction.

Known asymmetry, preserved rather than fixed: `promote_rarity` scales
`mitigation`, which `spawn_wild_creature_scaled` deliberately leaves unscaled.
That is the existing nemesis behaviour and `MAX_MITIGATION_PERCENT` bounds it;
changing it here would be an unrelated retune riding a feature.

## The perk

One repeatable perk, appended to `Perk` (variant order **is** the save format —
bincode encodes positionally into `PlayerSave::unlocked_perks`), with
`Perk::all() -> [Perk; 18]` becoming `[Perk; 19]` and a catalogue file in
`assets/perks/`.

It moves the rarity roll's **window**, using the extension point `rarity_mass()`
documents for precisely this: *"Exposed so a caller that wants a different rare
rate can narrow the range it draws from instead of authoring a second table"* —
the caravan's standout rows already do it. No second table.

Each rank does two things:

- **Widens the window.** At rank 0 the window is **zero**, so an unperked fork
  is always Ordinary. That is what "below average" means mechanically, and it
  is the property the whole feature's balance rests on.
- **Raises a ceiling.** Rank 1 tops out at Silver, rank 2 at Gold, and so on;
  it **self-caps at Prismatic after four ranks**, because `Rarity::ALL` is
  finite.

That finiteness is the answer to the standing hazard that perks are uncapped
and repeatable: there is no rung above Prismatic to buy, so the ladder bounds
itself without a constant to forget.

`rarity_penalty` lowers the ceiling by its own number of rungs, so `fork_bomb`
is a rung behind `fork` at every rank — which means the quality-for-numbers
trade appears the moment the perk does, and not before.

**Two queries in `perks.rs`**, both `Option<&Perks>` family since the subject is
always the player: the window and the ceiling. The census
(`every_perk_has_a_query_that_answers_what_it_is_worth`, exhaustive with no
`_` arm) names the **window**, which is strictly greater at rank 1 than at
rank 0.

## Naming

`Rarity` is already `Ordinary → Silver → Gold → Platinum → Prismatic`, so the
silver/gold vocabulary needs nothing invented.

- Routines: **`fork`** and **`fork_bomb`**. Process-forking is the right
  register for this setting, and a fork bomb is precisely "spawn many processes
  at once".
- Perk: **Scheduler** — what decides the priority class a forked process gets.

Deliberately not "daemon", per the no-occult-naming rule.

## Scope and blast radius

Engine-only. `battle_rows` already builds party rows off `planned`, and the
tactical map already draws a non-`Hostile` body as an ally, so no gui change is
expected — to be confirmed rather than assumed during implementation.

No `SAVE_FORMAT_VERSION` bump: `BattleState` and `TacticalBattle` are never
serialised, `Summoned` is battle-scoped, and a new `Perk` variant appended at
the end costs nothing.

**Files touched:**

| file | change |
|---|---|
| `abilities.rs` | `AbilityEffect::Summon`, `summon_target_mismatch`, load validation |
| `components.rs` | `Summoned` marker |
| `game/spawning.rs` | `fork_programs`, escalation composition |
| `species.rs` | sorted non-boss species pool |
| `game/combat_round.rs` | `use_ability` `unreachable!` arm, Special-site branch, `choose_summon_action` |
| `game/combat_teardown.rs` | `retier_rarity`, `finish_fight` sweep, `bench_or_dissolve` skip |
| `tactical/mod.rs` | initiative insertion after the cursor |
| `tactical/turn.rs` | Special-site branch, placement beside the invoker |
| `tactical/ai.rs` | `tactical_ai_actor` gate widened to `Summoned` |
| `perks.rs` | `Perk` variant, two queries, census arm, `all()` length |
| `tuning.rs` | `SUMMON_STAT_MULT`, `SUMMON_LEVEL_STAT_STEPS`, window-per-rank |
| `assets/abilities/` | `fork.ron`, `fork_bomb.ron` |
| `assets/abilities/README.md` | the `Summon` effect's schema |
| `assets/perks/` | Scheduler catalogue entry |

## What the tests have to prove

Behaviour, not shape. The ones that carry the design:

1. A summon is **not** in the roster — `pet_count` unchanged, absent from the
   save's tamed programs, `program_role` answers `None`.
2. It **acts** in the group model — appended to `Party` *and* `planned`, draws
   a row, takes a turn, is a legal target for hostiles and for `WholeParty`.
   (Asserting only that it exists passes against the inert bug above.)
3. It acts in the tactical model, and **nobody else loses or doubles a turn** —
   assert on turn order by identity across a full round, not on a count of
   turns taken. A count is conserved under a cursor shift.
4. Every one of the five fight endings leaves **no `Summoned` entity alive**.
5. A dead summon does **not** land in `DownedPrograms` on Forgiving, and is not
   dissolved as a tamed program on Permadeath.
6. Re-invoking replaces the standing set and the replaced bodies do not act
   again.
7. At perk rank 0 the tier is **always** Ordinary over many rolls; rank 1
   reaches Silver and never Gold.
8. `fork_bomb` bodies are one rung below `fork` bodies at the same rank.
9. A summon earns no XP and its kills still pay the player — `finish_hostile`
   keys the payout on the victim, not the killer.
10. `retier_rarity` downward is the exact inverse of upward on the same body.
11. Deleting both `.ron` files leaves the game as it was — the empty-catalogue
    property every data-driven subsystem here holds.

`balance_sim` gates none of this: it models no abilities and no perks, so the
numbers are a `dev-arenas/` and a played-session question. Say so rather than
implying the suite proves the balance.
