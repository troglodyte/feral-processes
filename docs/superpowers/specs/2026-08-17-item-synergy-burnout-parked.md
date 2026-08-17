# Infinite synergy, item stacking and burn-out — parked brainstorm

**Status: PARKED after exploration on 2026-08-17. Not a design, not
approved, nothing implemented.** No approach was chosen. This file exists so
the findings below — which establish that the hazard named in TODO #23 does
not currently exist, and why — don't have to be rediscovered.

Read `INDEX.md`'s warning about `**Status:**` headers before trusting any
other spec's; this one is accurate as of the date above and will rot the same
way.

## The question asked

TODO #23, verbatim:

> infinite synergy and item stacking. using multiple items with side effects
> can stack even up to game breaking synergies. maybe burn out?

Answered in the same round: the axis worried about is **combat math**, and
the preferred governor shape is **diminishing returns plus backlash** past a
threshold.

## The headline finding: there is nothing to stack

Across the 54 files in `assets/items/`:

- All 31 equipment items and all 18 affixes are **flat stat lines only** —
  `atk` / `def` / `decompiler` (`items::EquipmentStats`,
  `crates/engine/src/items.rs:289`). No item in the game has a mechanical
  side effect on gear.
- Exactly **one** item has a side effect at all: `patch_routine`, a
  consumable arming Mitigation 10 for 120 ticks. The only other consumable,
  `power_cell`, restores Power.

So #23 is not a latent exploit to close. It is a note about content that does
not exist yet, and the interesting question is not "how do we stop the
runaway" but "what would have to open before a runaway were even reachable".

## Six independent closures, and each one is a different mechanism

The engine is already anti-synergy by construction. Every channel through
which effects could compound is closed, and no two are closed the same way —
which means none of them was closed *as* a stacking policy, and there is no
single seam to relax.

1. **Stat stacking is linear, and defence self-limits.**
   `battle::compute_damage` is `(move_power + atk - def).max(MIN_DAMAGE)`
   (`crates/engine/src/battle.rs:243`). Subtractive: +10 ATK is +10 damage at
   every magnitude, never more. And once `def >= move_power + atk`, each
   further point of DEF is worth exactly zero, because `MIN_DAMAGE` has taken
   over. N stat-granting items can never produce a superlinear result.

2. **Percentage mitigation sums additively into a floor.**
   `Game::mitigate_incoming_damage` (`game/combat_damage.rs:65`) reads
   `field_buff_power`, which **sums** every matching entry
   (`components::field_buff_power_of`), then applies
   `dmg * (1 - power/100)` and floors the result at 1. So sources add rather
   than compose multiplicatively, and past 100% summed the result saturates
   at `MIN_DAMAGE` rather than at zero. There is no cliff behind the cliff.

3. **Consumable buffs cannot coexist at all.**
   `Game::arm_field_buff` (`game/combat_status.rs:220`) drops *every*
   existing `BuffSource::Consumable` entry when a new one is armed — not
   same-kind, **all of them**. One consumable buff at a time, full stop. This
   is the closure that most directly forbids the thing #23 describes.

4. **Routine buffs displace same-kind.** The other arm of the same match
   retains all but the running `Routine` entry of that `kind`. `long_winter`
   (Mitigation 25) and `ablative_layer` (Mitigation 10) therefore replace each
   other rather than summing.

5. **Passive triggers are polled from state once per round, not driven by an
   event stream.** `battle_resolve_round` (`game/combat_round.rs:137-146`)
   calls `fire_passives` at one fixed point after every chosen action, asking
   "did anyone drop" and "who was newly afflicted" as *questions about current
   state*. A passive whose effect kills an ally cannot re-enter the trigger,
   because the trigger is not raised by the death — it is read once, later.
   Cooldowns then throttle re-firing across rounds. Trigger chain-reactions
   are structurally unreachable, not merely unlikely.

6. **Percentage buff kinds are excluded from caster scaling.**
   `FieldBuffKind::scales_with_caster` returns `false` for `Mitigation` and
   the four rate kinds, so an authored 10% cut stays 10% at every level and
   affinity rather than being multiplied into the ceiling.

**Ceiling actually reachable today, in the worst case:** Mitigation 25
(`long_winter`, Routine) + Mitigation 10 (`patch_routine`, Consumable) = 35%.
That is the entire stacking surface of the shipped game.

## The crux: burn-out is not a governor, it is a key

Because of closure 1, a diminishing-returns curve priced against *how much
bonus you are carrying* would tax the one axis that is already provably safe.
Stat stacks are linear; taxing them buys nothing and makes gear read worse.

The compounding risk in a game shaped like this one never lives in the stat
stack. It lives in **loops** — an effect whose output feeds its own input
(Drain healing off damage it caused, a trigger raising the condition that
fires it) — and in **multipliers of multipliers**, of which the engine
currently has none on the player side.

So the honest reframing is: **the six closures block the hazard and the fun
by the same act.** Burn-out is not something you add before the content to
keep it safe. It is what you would trade for *opening one specific closure* —
the price of the loop, not a tax on the pile.

That inverts the sequencing implied by the TODO. Pick the closure first; the
governor is then designed against that closure's specific failure mode rather
than against stacking in the abstract.

## Three candidate shapes

Each names the closure it opens. None was chosen.

### A. Open closure 3 — let consumables coexist

The literal reading of #23. Delete the all-consumables `retain` and let
concurrent consumable buffs run together, each additional one worth a
diminishing fraction, with backlash past N concurrent.

- **Smallest opening in the file** — one `retain` predicate in
  `arm_field_buff`, which is already documented as the only writer of
  `FieldBuff::active`.
- Prototypable against the two consumables that already ship, before any new
  content is authored.
- Weakest payoff: with two consumables in the game there is nothing to
  combine yet, so this opens the door onto an empty room.

### B. Open a new axis — gear grants a passive routine while worn

`ItemDef::grants: Option<AbilityId>` behind `#[serde(default)]`, feeding the
existing `PassiveTrigger` / `AbilityEffect` vocabulary. Governor lives in
`fire_passives`: a per-battle firing count per holder, magnitude diminishing
with the count, backlash past a threshold.

- Most content-rich, and **free moddability** — the effect vocabulary already
  exists as data (9 `AbilityEffect` variants, 9 `FieldBuffKind`s, 2
  `PassiveTrigger`s), so this is a new field rather than a new language.
- `fire_passives` is the single funnel every triggered effect passes through,
  so the hook obeys CLAUDE.md's "a perk's hook belongs where its sources
  meet" rather than needing repeating.
- Costs a new `ItemDef` field, a decision about whether the firing count is
  saved, and `PassiveTrigger`'s vocabulary is only two variants wide — most
  interesting gear effects would want a third, and each new variant is a new
  call site in `combat_round`, refused at load if nothing fires it.

### C. Open closure 1 — a genuinely multiplicative effect kind

A `FieldBuffKind` that multiplies rather than adds, which is the only thing
in this engine that *can* compound. Burn-out is then the whole price of
admission.

- Delivers the "game-breaking synergy" feel #23 is actually gesturing at.
- Collides head-on with the standing invariant that **every difficulty curve
  in the game is linear, and that is a correctness property** — under the
  subtractive floor a multiplicative player curve racing a linear enemy curve
  has an end past which every enemy swing lands on `MIN_DAMAGE`. This is the
  same failure the linear-curve rule exists to prevent, arriving from the
  player's side.
- Highest risk, and `balance_sim` **cannot see it**: the sim models no
  abilities at all, so field buffs, their magnitudes and any burn-out curve
  are entirely ungated by the balance regression suite. This would ship on
  `dev-arenas/` and play alone.

## Open questions

1. **Where does the diminishing counter reset — per battle, per run, or per
   item?** This is the save-format question. Per-battle is free (battle-scoped
   components are wiped by `end_battle` and never persisted). Per-run needs a
   field and therefore an additive `#[serde(default)]` change at minimum.
   Per-item means the count rides `GearCopy`, which is saved, and interacts
   with the buyback shelf keying on the whole copy.

2. **What is backlash actually denominated in?** The obvious reuse — raising
   Trace — **does not work on the surface.** `Game::raise_trace`
   (`game/trace.rs:110`) returns silently unless `is_underground()`, so
   backlash-as-Trace would be a mechanic that exists in the Stack and
   evaporates on open grid. Either backlash is HP/status (which the damage
   floor already bounds), or it is a new meter, which is a new resource and a
   new save field.

3. **Does a diminishing multiplier on the player side interact badly with the
   subtractive floor?** It reduces a bonus rather than scaling an enemy, so
   probably not — but the interaction is unmodelled and unstated, and this
   repo has been bitten four times by exactly that kind of unverified
   "probably fine" about a curve.

## Notes in passing

- `components.rs:852` refers to "the cap on `Mitigation`". No such cap
  exists as a constant or a clamp — `mitigate_incoming_damage` does not bound
  `power` at all. What actually caps mitigation is `MIN_DAMAGE` taking over
  the result. The doc is describing an effect, not naming code; worth
  tightening if that file is touched, since a reader looking for the cap will
  not find one.

- **Perks are the live counter-example and are worth reading before designing
  any of this.** They are explicitly uncapped steady stacks, and `PerkDef`
  deliberately carries no `effect` field because a perk's effect is a hook
  into one particular formula with no shared shape. Any item-side-effect
  design that reaches for its own bespoke effect vocabulary is re-deriving
  that decision; shape B avoids it by borrowing `AbilityEffect` instead.
