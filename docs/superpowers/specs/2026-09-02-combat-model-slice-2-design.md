# Combat model slice 2: a second swing, earned by class and level

**Status:** designed 2026-09-02, **not implemented**. Written against a code
survey, not against play — see Risks, and the standing note that slice 1 has
never been in front of a player either.
**Date:** 2026-09-02
**Slice:** 2 of 4 — see
`../archive/specs/2026-08-19-combat-model-ac-and-weapon-damage-design.md`
for slices 1, 3 and 4 and the argument for this ordering.
**Save format:** **no bump.** Every input is already saved.
**Crates:** `crates/engine` only.

## The problem

Slice 1 made a single swing interesting — an attack roll against Evasion,
four outcome bands, crit and a fumble ladder. It did not change how *many*
swings a fight is made of, so a fight's shape is still one actor, one blow,
every round, forever. Nothing a player earns changes that.

A related gap sharpens it. **The player's class does nothing for an ordinary
attack.** `PlayerClass` grants affinities, and affinity is read by
`ability_affinity` on the `Special` path only — `battle::expected_damage` has
no affinity term and `attack_range` feeds `resolve_attack` directly. So a
Striker, the class whose whole identity is hitting hardest, swings exactly
like a Medic. The class is legible on the character sheet and invisible in
the fight.

## What this builds

**A Striker gets a second attack per round from level 8.** One extra swing,
a full roll, resolved inside the same turn. Nothing else changes.

Striker is the grantor because it is the one class name that exists in **both**
`PlayerClass` (`classes.rs:86`) and `AffinityClass` (`species.rs:353`). The
player picks it at creation; a companion or wild program derives it from its
affinities through `SpeciesDef::affinity_class`. So the rule is written once
and lands symmetrically on all three — no new class, no asymmetric input, and
no species file edited.

Level 8 is the threshold because `zone_level_cap` floors zone 1 at 6
(`tuning.rs:309`) and only reaches 12 in zone 2 (`ZONE_LEVEL_CAP_STEP = 11`).
A second swing is therefore unreachable before the first breach and arrives
partway through the second sector — a milestone the player crosses rather
than a number they start with.

## Non-goals

Named so they are not smuggled in:

- **A third attack.** The cap is two, permanently. Every extra rung multiplies
  a curve that `balance_sim` fits by hand.
- **Reaching past the front member.** A second swing hits whatever
  `EnemyGroup::members[0]` promotes to, exactly as a first swing would. That
  is slice 3, which the slice 1 spec keeps deliberately last because it
  "rewrites what a swarm *is*".
- **Extra *actions*.** The second swing is a basic attack, never a Special. It
  spends no Power and arms no cooldown, so `routine_power_cost` and every
  authored `power_cost` keep meaning what they mean. Widening this to a second
  Special is a different feature with a different price list.
- **A talent or gear grant.** `assets/talents/` ships exactly
  `KERNEL_RING_MAX * LEVELS_PER_RING` tiers of two choices; a sixth
  `TalentNode` kind costs every shipped tree an existing choice. Gear would
  need a dual-wield concept that does not exist.
- **Retraining the enemy policy.** Required, but after this lands — the
  trainer must run against a round loop that already has the second swing.

## 1 · One rule, one function

```
battle::attacks_per_round(class: Option<AffinityClass>, level: u32) -> u32
```

Returns 2 for a Striker at or above `EXTRA_ATTACK_LEVEL`, otherwise 1.

It takes `AffinityClass`, not `PlayerClass`, because that is the vocabulary
both sides already share; the player's arm maps its `PlayerClass` down through
the five common variants and yields `None` for the three player-only ones
(Decompiler, Invoker, Fabricator), which grant no swing.

**Three callers and no fourth**, following `attackers_in_group`'s precedent —
the offline projection and the real round loop cannot drift because they are
the same call:

| Caller | File |
| --- | --- |
| the party's turn | `game/combat_round.rs::party_member_attacks` |
| the wild side's turn | `game/combat_enemy.rs::wild_retaliate` |
| the balance projection | `balance_sim.rs::simulate_roster_fight` |

**The naming trap this sits next to.** `battle::attackers_in_group` means *how
many bodies of a group may swing this round*. `attacks_per_round` means *how
many times one body swings*. They are adjacent in `battle.rs`, both read
`for _ in 0..n` at the call site, and applying the wrong one — or both — is
the single most likely way to build this incorrectly.

## 2 · The loop goes inside the turn

The swing loop lives **inside** `party_member_attacks` and `wild_retaliate`,
not in `Game::roll_initiative`.

Pushing a second `Actor` into the initiative list compiles just as well and is
wrong three ways: it rolls initiative twice for one body, it re-checks
`is_stunned` as though the second swing were a separate turn, and it
double-counts against `attackers_in_group`'s reach cap for wild groups. The
inner loop leaves initiative, `BattleState::planned` and all of app-core
untouched — `planned` still indexes `Party` positionally, one action per slot,
and the planning UI never learns this feature exists.

Between an actor's own swings, and only between them:

- **Re-resolve the target.** `finish_group_member` can empty the front or end
  the battle mid-loop, so swing 2 re-reads `front_of_group`/`retarget` exactly
  as a fresh call would. If the group is gone, the loop stops.
- **Re-check stun.** A Crash landed by the actor's *own* fumble on swing 1
  cancels its remaining swings this round. See §3.
- **Do not re-fire the wielded proc.** `proc_wielded_routine` fires once per
  turn, after the loop, gated on the player's slot as today. Firing it per
  swing would silently double the wielded build's value — the change to refuse.

## 3 · The fumble semantics this exposes

`EXPOSED_DURATION_ROUNDS = 1` carries this at `tuning.rs:3328`:

> a duration of 1 is exactly "until the fumbler's next turn", which is the
> rung's wording

That equivalence holds **only while one turn is one swing**. `CRASH_*` says it
costs the fumbler "their next action" for the same reason. Neither statement
survives an actor swinging twice in a round, and only the round reading is
implemented — `is_stunned` is checked once, at the top of the per-`Actor` loop.

This is latent today, not introduced here, and it must be settled explicitly
rather than discovered as a fumble that feels wrong:

- **Crash cancels the fumbler's remaining swings this round.** "Costs the
  fumbler their next action" is read as *action*, not *round*, and a swing is
  an action. So the stun check moves inside the swing loop for the actor's own
  swings, and keeps its existing position for the actor's next round.
- **Exposed keeps the round reading.** It is a debuff on the *victim*, and the
  victim's next turn is still a round away. `landed_this_round` already exempts
  the landing round, so nothing changes.

Both get a test naming the distinction, because the two rungs now resolve on
different clocks and the next reader will assume they match.

## Interface

| Symbol | Kind |
| --- | --- |
| `tuning::EXTRA_ATTACK_LEVEL: u32 = 8` | new `pub const` |
| `tuning::MAX_ATTACKS_PER_ROUND: u32 = 2` | new `pub const` |
| `battle::attacks_per_round(Option<AffinityClass>, u32) -> u32` | new, `pub(crate)` |
| `Game::attacks_for(Entity) -> u32` | new — resolves player vs companion vs wild to a class and level |

No new component, no new resource, no new save field, no `.ron` schema change.

## Save format

**No `SAVE_FORMAT_VERSION` bump.** The swing count is derived at battle time
from class and level, both already saved, exactly as slice 1 derived Accuracy
and Evasion rather than storing them. Nothing new is serialised and the figure
cannot drift from its inputs.

## Testing

TDD, failing reproducer first. Intents:

- `attacks_per_round` returns 2 for a Striker at exactly `EXTRA_ATTACK_LEVEL`,
  1 at one level below, and 1 for each of the other four affinity classes and
  the three player-only classes at any level.
- A level-8 Striker resolves two `SwingOutcome` lines in one round; a level-7
  Striker resolves one. Assert on the **log lines**, since that is what the
  player sees.
- The wielded proc fires **once** in a two-swing round. Delete the gate and
  this test must fail, or it is vacuous.
- A Crash fumbled on swing 1 cancels swing 2 — assert the second line is
  absent, not merely that damage is lower.
- A group emptied by swing 1 ends the loop rather than swinging into nothing.
- A wild Striker gets its second swing too — the rule is symmetric, and a test
  that only covers the player would pass against a player-only implementation.
- `balance_sim` compiles the same figure through the same call: a Striker
  roster's projected clear level moves, and moves in the sim exactly as it
  moves in a real fight.

**Gates:** `cargo test --workspace`, `cargo clippy --workspace`, `cargo fmt`,
and `cargo test -p feral-processes-engine balance_sim` — the last is not
optional here, because this is a damage-throughput change and the curve
movement *is* the signal.

## Risks

1. **This roughly doubles a Striker's damage from level 8, and the level cap's
   constants are a correctness bound, not a difficulty knob.** CLAUDE.md is
   explicit that the cap must sit at or above the geared clear requirement, and
   `ZONE_LEVEL_CAP_STEP = 11` was fitted against the current per-round output.
   The curves must be re-baselined and the cap re-derived, not just observed to
   have moved. This is the largest risk in the change.
2. **Slice 1 has never been played** — `docs/measurements/2026-08-19-combat-model-slice-1.md:97`
   says so outright. Every feel question this spec answers by reasoning (is a
   two-swing round a slog, is Crash more punishing when it eats two swings) is
   answerable only at a keyboard, and this stacks an unplayed feature on one.
3. **The trained enemy policy goes stale.** No new feature is needed in the
   schema — every feature is a ratio — but the optimal policy changes, because
   `target_hp_frac` and `would_kill` become jointly more valuable once a body
   can finish across two swings. The three pinned-to-zero features stay a
   design boundary a retrain may not reopen.
4. **Sortie pricing shifts silently.** `sortie_duration` reads the risk offset
   and has no term for squad strength, so a Striker squad that now clears
   faster is not priced differently. Accepted; noted so it is not read as a bug.
5. **`balance_sim` models no fumble ladder**, so its "mildly overstates net
   output" caveat gets proportionally worse as swings per fight rise. One line
   of doc comment, not a redesign.

## Work breakdown

Engine-only and it fits in one context, so per CLAUDE.md's process-weight rule
this needs **no plan document** — TDD inline, a commit per green step.

| | Task |
| --- | --- |
| **A** | `attacks_per_round`, the two constants, `Game::attacks_for`, and their tests |
| **B** | The inner swing loop in `party_member_attacks` and `wild_retaliate`, with retarget and the wielded-proc gate |
| **C** | The Crash-cancels-remaining-swings rule and the two fumble-clock tests |
| **D** | `balance_sim`'s two loops call the shared figure; re-baseline the curves and re-derive the cap |
