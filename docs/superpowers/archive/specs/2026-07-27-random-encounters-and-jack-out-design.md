# Random encounters and a fallible jack-out

**Date:** 2026-07-27
**Branch:** `feat/random-encounters-and-jack-out`
**Status:** Design, approved

## Problem

Two related gaps, both about the player having too much control over when
they fight.

**Every fight is opted into.** Wild programs are drawn on the map and only
engage when you deliberately walk into them (`Game::move_player`,
`turn.rs:131`). You can therefore cross any amount of open ground in perfect
safety by routing around the glyphs, and `difficulty_color`
(`inspection.rs:327`) hands you the threat read before you commit. Travel
carries no risk, so the map is a puzzle of avoidance rather than a place.

**Every fight is opted out of.** `Game::battle_flee`
(`combat_status.rs:8`) always succeeds. It rolls
`FLEE_COUNTERATTACK_CHANCE` for a parting counter-strike and docks a mild XP
setback, but the escape itself is guaranteed regardless of what you are
fighting. A hopeless fight costs 20% of in-level XP and nothing else, so
there is never a reason to weigh running against staying.

Fixing only the first would be worse than fixing neither: unavoidable
ambushes against a guaranteed escape hatch are just a tax on your XP bar.
The two go together.

## Scope

1. A per-step chance of being ambushed while travelling, spawning a
   biome-appropriate pack that engages immediately.
2. A jack-out attempt that can fail, keyed on the power ratio between your
   side and theirs, with a random element on every attempt.
3. Every knob for both in `crates/engine/src/tuning.rs`.

Not in scope: creatures that aggro or pursue you on the map (considered and
rejected in favour of ambushes), fleeing as a per-member battle action
rather than a party command, and any change to what a *won* battle awards.

## Existing architecture this builds on

Verified by reading, not assumed:

- `Stats::power()` (`components.rs:83`) is already the codebase's
  "how strong is this" scalar — `max_hp + atk + def`, unweighted. It backs
  `difficulty_color` and the trade-in price in `trade.rs:85`. The jack-out
  ratio reuses it rather than inventing a second strength metric.
- `Game::gather_pack` + `Game::start_battle` (`combat.rs:58`, `:137`) are
  the whole battle-entry path, already used by `move_player`. An ambush is
  a new *caller* of them, not new battle machinery.
- `Game::try_spawn_habitat_creature` (`spawning.rs:357`) already selects a
  biome-appropriate species, applies opening-ring gentling, rolls for a
  boss, rolls for a nest, picks a group size and scatters the members.
- `Game::all_wild_retaliate` is already the "every engaged group swings"
  primitive, used today for the parting counter-strike on a successful
  flee.
- `taming::capture_chance` is the house pattern for a clamped probability
  built from a ratio: a pure function with coefficients and hard
  `CAPTURE_CHANCE_MIN`/`MAX` bounds in `tuning.rs`. The jack-out chance
  follows its shape.
- `MIN_INDIVIDUAL_ROLL`/`MAX_INDIVIDUAL_ROLL` (`tuning.rs:271`) is the house
  pattern for a uniform random multiplier applied to an otherwise
  deterministic value. The jack-out luck roll follows its shape.
- `balance_sim.rs` models neither fleeing nor map movement — grepped, there
  are no matches for `flee` or `escape`. The balance curves should not move.

## Design

### 1. Ambush while travelling

**Where.** In `Game::move_player`, after a successful walk step, before the
closing `self.tick()`. The bump-into-creature, nest, portal and blocking
structure paths all `return` earlier and so never roll — an ambush is
something that happens while you are *walking*, not while you are already
starting a fight or changing zone.

**Rate.** One `random_bool(RANDOM_ENCOUNTER_CHANCE)` draw per step.

**Suppressed on `Biome::Platform`.** Your base floor stays safe. This is a
rule rather than a knob, so it is a code check, not a constant — the same
call `forage_chance` already makes about platform tiles.

**What spawns.** A biome-appropriate pack at a walkable tile adjacent to the
player — one of the eight neighbours, matching the game's 8-directional
movement and the Chebyshev distance `gather_pack` and the spawn culler
already measure in — gathered and engaged immediately via the existing
`gather_pack`/`start_battle` path. Two deliberate exclusions relative to an
ordinary habitat spawn:

- **No bosses.** A boss you cannot decline is a death sentence you did not
  opt into. Bosses stay something you find on the map and choose.
- **No nests.** A nest is a structure you attack (`attack_nest`), not a
  fight that jumps you. Spawning one as an ambush is a category error.

Opening-ring gentling (`in_opening_ring`) still applies, so a fresh player
is never ambushed by the worst thing in the biome.

If no walkable adjacent tile exists, or the biome has no eligible non-boss
species, the roll is spent and nothing happens. Failing quietly is correct
here: the alternative is hunting for somewhere to put a fight the player did
not ask for.

**`WILD_CREATURE_CAP` is not enforced on this path.** `maybe_spawn_wild_creature`
culls distant hostiles to make room before spawning; the ambush does not,
because the creatures it places are about to be fought and resolved rather
than left to roam. The cap exists to bound the population of *idle* wild
programs the player wandered away from, and an ambush pack is the opposite
of that. Worst case it overshoots the cap by one pack until the next spawn
roll culls.

**Refactor this requires.** `try_spawn_habitat_creature` currently welds
together species selection, the boss roll, the nest roll, group sizing and
scatter, and the ambush needs the first and last of those without the middle
two. Rather than copy the biome and opening-ring logic — which the repo has
been bitten by four times, per CLAUDE.md's rule on duplicated formulas —
split out two `pub(crate)` helpers that both callers use:

- `pick_habitat_species(x, y, allow_boss) -> Option<(String, bool)>` —
  candidates, opening-ring gentling, the boss roll, the pick. Returns the
  species id and whether it is a boss.
- `spawn_pack(species_id, is_boss, x, y) -> Vec<Entity>` — group size,
  swarm radius, the scatter loop. Returns the spawned entities.

`try_spawn_habitat_creature` becomes `pick_habitat_species(allow_boss:
true)` → nest roll → `spawn_pack`, preserving its `bool` return. The ambush
path is `pick_habitat_species(allow_boss: false)` → `spawn_pack` →
`gather_pack` → `start_battle`.

**RNG-order constraint.** `try_spawn_habitat_creature`'s comments are
explicit that its draw order is load-bearing for seeded tests — the nest
roll in particular is documented as only drawing when `can_nest` is true
precisely so it does not shift the sequence for the common case. The
extraction must leave the existing caller's draw sequence identical. The new
caller skipping the boss draw does not affect it, because that draw is
already conditional on `boss_candidates` being non-empty.

### 2. Fallible jack-out

A pure function in `battle.rs`, unit-testable without an RNG:

```rust
pub fn jack_out_chance(ours: i32, theirs: i32, luck: f64) -> f64
```

```
ratio  = ours / max(theirs, 1)
chance = clamp(JACK_OUT_BASE_CHANCE * ratio * luck,
               JACK_OUT_CHANCE_MIN, JACK_OUT_CHANCE_MAX)
```

`ours` is the sum of `Stats::power()` over the player and every living party
member; `theirs` is the sum over every living enemy in every group. Both use
`max_hp`, not current `hp`, so the odds are a property of the matchup rather
than of how the fight is going — the ratio you face when the ambush lands is
the ratio you keep. Killing an enemy improves it; losing a companion worsens
it.

`max(theirs, 1)` guards the divide; a battle with no living enemies should
have ended already, so this is defensive, not an expected path.

`luck` is the random element: a fresh uniform draw per attempt from
`JACK_OUT_LUCK_MIN..=JACK_OUT_LUCK_MAX`, passed in by the caller so the
function stays pure. A hopeless-looking escape sometimes works and a
favourable one sometimes does not, which keeps the decision to run from
being a lookup.

**On failure.** Log `"The exit route collapses — they're still on you!"`,
then `all_wild_retaliate` — a free volley from every engaged group. The
battle continues and one `tick()` passes. **No XP setback**: you only pay
for an escape you actually got. Spamming the attempt against a stronger pack
therefore bleeds HP without bleeding XP, and loses on its own terms.

**On success.** Unchanged from today: the `FLEE_COUNTERATTACK_CHANCE`
parting-shot roll, the XP setback via `apply_setback_xp_penalty`,
`end_battle`, `tick`.

**Signature change.** `battle_flee` returns `bool` — whether the escape
happened. `crates/app-core/src/app/battle.rs:107` calls it and then infers
the outcome from `still_active`; it must not play `SoundEvent::Flee` on a
failed attempt.

### 3. Tuning constants

All six in `crates/engine/src/tuning.rs`, each with a doc comment
explaining its role, placed in the existing labelled sections.

| Constant | Value | Section |
| --- | --- | --- |
| `RANDOM_ENCOUNTER_CHANCE` | `0.02` | Spawning & encounters |
| `JACK_OUT_BASE_CHANCE` | `0.6` | Battle |
| `JACK_OUT_LUCK_MIN` | `0.8` | Battle |
| `JACK_OUT_LUCK_MAX` | `1.2` | Battle |
| `JACK_OUT_CHANCE_MIN` | `0.10` | Battle |
| `JACK_OUT_CHANCE_MAX` | `0.95` | Battle |

Rationale for the values, all of which are arithmetic-plausible only and
have not been played:

- `0.02` is roughly one ambush per fifty steps. The map is already full of
  creatures you can choose to fight, so ambushes are spice rather than the
  main course. `WILD_SPAWN_CHANCE` is `0.05` per *tick* for comparison, but
  that only places a creature somewhere nearby; this one starts a fight.
- `0.6` puts an even matchup at a 60% escape before luck, swinging 48–72%
  with it. Running from a fair fight usually works.
- `0.8`/`1.2` matches the existing `MIN`/`MAX_INDIVIDUAL_ROLL` spread.
- `0.10`/`0.95` mirrors `CAPTURE_CHANCE_MIN`/`MAX`'s "never hopeless, never
  certain" shape. The floor means an overwhelming ambush takes roughly ten
  attempts to escape, each costing a volley — survivable with consumables,
  not free. This is the value most likely to need retuning after play.

`FLEE_COUNTERATTACK_CHANCE` is unchanged and keeps its current meaning: the
parting shot on a *successful* escape.

## Testing

**Pure functions**, no RNG, no `Game`:

- `jack_out_chance` at parity (`ours == theirs`) returns
  `JACK_OUT_BASE_CHANCE` at neutral luck.
- An overwhelming enemy total clamps to `JACK_OUT_CHANCE_MIN`.
- An overwhelming friendly total clamps to `JACK_OUT_CHANCE_MAX`.
- Luck at its min and max shifts the result proportionally, and cannot push
  it outside the clamp.
- `theirs == 0` does not divide by zero.

**Ambush**, seeded:

- Walking on a `Platform` tile never ambushes, however many steps.
- An ambush produces an active battle whose enemies are all non-boss.
- No ambush roll fires on a step that bumped a creature, nest or portal —
  those paths return before the roll.
- No ambush when the game is over.

**Jack-out**, seeded:

- A failed attempt leaves the battle active, deals damage, and costs no XP.
- A successful attempt ends the battle and applies the XP setback.
- `battle_flee` returns `false` on failure and `true` on success.

**Existing tests that assume infallible fleeing** and must be updated rather
than deleted — each needs its roll forced to succeed, via a stacked party or
a chosen seed:

- `crates/engine/src/tests/combat.rs:8`
  (`battle_flee_applies_the_same_mild_xp_setback_as_a_death`)
- `crates/engine/src/tests/combat.rs:566`
- `crates/engine/src/tests/party.rs:411`

**Balance gate.** `cargo test -p feral-processes-engine balance_sim` must
still pass unmoved. `balance_sim` models neither fleeing nor map movement,
so these constants should not touch it; the run confirms that rather than
assuming it.

## Documentation obligations

- `docs/manual.md:77` and `docs/manual.md:143` both state that jacking out
  costs a mild XP setback, full stop. Both are falsified by this change and
  need the chance, its dependence on relative power, and the cost of a
  failed attempt.
- `docs/manual.md` needs travel to stop reading as safe — a line on ambushes
  and on the base platform being exempt.
- `README.md:31` mentions fleeing in passing; check whether it overclaims.

## Verification

The standard gate applies:

```sh
cargo test --workspace
```

Baseline on this branch before any change: **554 passed, 0 failed** (462
engine, 48 app-core, 42 gui, 2 launcher). Plus `cargo clippy --workspace`
clean and `cargo fmt`, per CLAUDE.md.

This change touches `crates/engine` and one line of `crates/app-core`;
`crates/gui` needs no change, since jacking out is already routed through
`PartyCommandKind::JackOut` and the flee sound is chosen in app-core.
