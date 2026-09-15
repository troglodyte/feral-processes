# Tamper routines: attacking the enemy's decision-maker

**Status:** designed, not implemented

TODO #92 asked for "AI related skills, routines, tools, items". The shipped
vocabulary is systems and security — segfault, row hammer, kernel panic — and
the setting's AI half is barely spent. But the game already *has* an AI worth
attacking: every hostile on a battle map is driven by a scored policy with a
temperature, an action chosen before a position, and an argmax at zero.

So the core of this feature is five routines that tamper with that machinery
directly — how randomly a hostile decides, whether you can read what it will
do, which side it thinks it is on, and what it thinks it sees. Around them sit
five flavour routines and two gear mods on existing mechanics, and one research
node teaches the lot.

**Battle maps only.** None of this exists in the group model.

## The effect

One new `AbilityEffect` variant:

```ron
Tamper(kind: Temperature(0.0), duration: 2)      // Cold Sample
Tamper(kind: Temperature(2.0), duration: 2)      // Heat Injection
Tamper(kind: Profiled, duration: 3)              // Inference Probe
Tamper(kind: Injected, duration: 1)              // Prompt Injection
Tamper(kind: Hallucinating(decoys: 3), duration: 3)  // Hallucination
```

`TamperKind` is its own enum, and each kind carries only what it needs.

**Temperature is absolute, not a delta.** `Temperature(0.0)` means exactly the
argmax with no `GameRng` draw — which is what makes Cold Sample plus Inference
Probe a *guaranteed* forecast rather than a likely one. A delta clamped at zero
would get there too, but would make the authored number mean something
different on every base temperature.

**A tamper always lands.** No accuracy roll and no `chance`. The forecast is the
feature, and a Cold Sample that fumbles one time in five turns the guarantee into
a gamble. The price is the cooldown and the Power cost.

**Nothing in a tamper scales** with the invoker's level or affinity: a
temperature is a temperature, and `duration` is already unscaled on `Debuff`.

Why not a new `StatusKind` instead: `StatusEffects::active` is an
`Option<ActiveStatus>` with overwrite semantics, so a Cold Sample would wipe the
Bleed already on the target. The combos this feature exists for need two
effects to coexist. Why not four variants: four passes through every exhaustive
`AbilityEffect` match and census for nothing a player can see.

## Where it lives

**`components::Tampered`** holds at most one entry per kind — a small map keyed
by the kind's discriminant. Reapplying a kind the body already carries
**refreshes** the entry rather than stacking it; Heat and Cold are the same kind,
so each replaces the other. Different kinds coexist.

Like `StatusEffects`, `Cloaked` and `CombatBuff`, it has no serde derive and
appears nowhere in `save.rs` — a fight is never saved, so `SAVE_FORMAT_VERSION`
does not move. It must be cleared when the fight ends, the same place those are.

**Duration counts the tampered body's own turns**, decremented when that body's
turn is handed on — deliberately *not* in `tick_one_combatant`, where statuses,
buffs, cooldowns and the cloak all age once per round. Aged per round, a
`duration: 1` Prompt Injection cast mid-round on a body that has already acted
would expire at the round's upkeep before the target ever took a turn. This is
the one in-fight state that ages differently, and it is why.

**Decoys live in `TacticalBattle`**, as `decoys: Vec<Decoy>` — a cell, the
owning side, and the caster's glyph and colour. They are not entities and not
part of any `Tampered` entry, because they outlive being struck by one body and
are shared by every hallucinating body.

## Who can be tampered

- **Hostiles**, always.
- **The player, never.** Nothing drives the player's body. A shaped tamper's
  recipients skip the player, and a single-target tamper aimed at the player is
  refused at the door, the way a capture is.
- **Companions, through friendly fire.** `reach::recipients` never reads
  `Hostile`, so a Heat Injection's radius catches your own. That is intended —
  see *Taken over*.

**No AI ever chooses a tamper routine**, on either side: not `choose_wild_action`,
not `tactical_ai_beat`, not `tactical_auto_beat` answering for the party. A
companion runs one only when commanded. Giving tamper routines to hostile
species is deferred (see *Out of scope*), and when it comes, it needs real
scoring rather than "count opponents in reach".

## The four hooks

### Temperature

Both tactical AI entry points already take a temperature: `tactical_ai_turn`
passes `tuning::TACTICAL_AI_TEMPERATURE` to `tactical_ai_turn_at`, and the
constant is passed literally at a handful of call sites in `tactical/ai.rs`.
Replace each with one lookup — `Game::decision_temperature(body)` — that returns
the tamper's value when present and the tuning constant otherwise. One door, so
a sixth call site cannot read the constant directly and miss a Cold Sample.

`ENEMY_POLICY_TEMPERATURE` (group model) is untouched.

### Injected

`tactical_sides` builds the per-turn `Sides { targets, allies }` every AI
decision is scored from. For an injected body, **swap them**. Its swing target,
its routine aim (`best_aim` already penalises its own side, so the penalty now
lands on the party), and its walk all follow with no further change. Its heal
routines land on the party too.

Two things have to hold that nothing in the swap guarantees:

- `Game::tactical_attack` must accept a hostile swinging at a hostile. If it
  filters on side, the filter moves behind the injection.
- **A hostile killed by an injected packmate pays** exactly what any hostile
  death pays, through `Game::finish_hostile`. The routine caused it.

### Hallucinating

Casting places `decoys` decoys owned by the caster's side: the first on the aimed
cell, the rest on the free cells of the routine's radius nearest the aim, ties
broken by cell order. **No `GameRng` draw** — a tactical fight's RNG budget is one
draw an AI turn, and this is not an AI turn.

The entry is inserted only on bodies of the side **opposing** the caster. A
companion caught in your own Hallucination is unaffected, because your
companions ignore your decoys.

For a hallucinating body, `tactical_sides` makes its **nearest opposing decoy
its only target**. It walks toward it and swings at it, or aims a routine at it.

- **An attack aimed at a decoy's cell destroys that decoy** and spends the
  turn. A swing needs a door that takes a cell rather than an entity; a routine
  resolves normally against any real bodies in its shape (full friendly fire)
  and also destroys the decoy.
- **Only the opposing side can target or destroy a decoy.** Your side's walk,
  swing and aim never see your own.
- Decoys do not block movement or sight.
- **A hallucinating body with no opposing decoys left sees clearly**, and its
  entry is removed then rather than waiting out its duration, so the strip
  stops saying `HALL`.
- Decoys are removed once no living body still carries a `Hallucinating` entry
  that could see them.

### Profiled

No AI change. A profiled hostile's **forecast** is published to the renderer.

The forecast must be a *call* into the same derivation the hostile's turn runs,
never a restatement — CLAUDE.md's "mirrors must be a call" rule. Today the
intent is a local `Intent` built fresh in `run_tactical_beat`, and the cell is
picked inside `walk_to_best_cell`. Extract a pure planning function that returns
the intent, and at temperature zero the destination and target, and have both
the turn and the forecast call it.

- **The action is always shown**: which routine, or a swing. The intent is
  decided before any draw, so it is honest at every temperature.
- **The destination and target are shown only at temperature zero.** Above zero
  the walk samples, and a drawn path that is wrong one time in three teaches the
  player the forecast lies.
- The forecast is computed from the board **as it stands**, so it updates as
  the party acts before the hostile's turn. It is exact at the moment the
  hostile's turn begins.

## Taken over

A companion carrying a `Temperature` or `Injected` entry is **driven by the AI
until the entry ends**, whether or not `App::tactical_auto` is on — the
"going mad" of other games.

- `Game::tactical_awaits_input` answers false for it, and the cursor skips its
  slot the way `slot_is_commanded` keeps it off a summon.
- It is driven by `tactical_auto_beat`, at `decision_temperature(body)` — so a
  Heat-caught companion plays erratically and an injected one swings at the
  party.
- `Profiled` never takes a companion over, and `Hallucinating` is never
  inserted on one.
- Control returns on the companion's first turn after the entry ends.

## Hidden in the group model

`AbilityEffect::tactical_only()` answers true for `Tamper`. The group model's
routine rows omit such routines, and `choose_wild_action` never considers them.
`field_runnable` is false. The five tier-A routines are ordinary effects and
appear in both models.

## What the player sees

- **Decoys**: `TacticalView` carries the decoy list; gui draws the caster's glyph
  at low alpha on each cell, through `Painter`, following the cloak fade.
- **Tamper tags**: `TurnRow` gains the active kinds, drawn as short tags —
  `HOT`, `COLD`, `PROF`, `INJ`, `HALL`. Temperature reads `COLD` at or below the
  tuning constant and `HOT` above. The strip does not widen or scroll, so the
  widest row with every tag is a width census.
- **Forecast**: a profiled hostile's strip row carries `▸ <routine name>` or
  `▸ swing`. At temperature zero the map also draws the walk to the destination
  and marks the target, in `BoltCue`'s line vocabulary and a palette role that
  is not `THREAT`.
- **Taken over**: the companion's strip row is marked as not under command.
- **Log lines**, in the security vocabulary, on a tamper taking hold, wearing
  off, and a decoy being destroyed: "Segfault Worm's sampler runs cold.", "The
  Worm's swing passes through a decoy."

The strip-row forecast is the first thing to confirm at the keyboard. It was
chosen because every corner and edge of a tile already carries a meaning, not
because it was seen to read well.

## Content

### Research: Model Inspection

One node, `model_inspection`, requiring `cortex` (zone 3, and the node that
already hands over the Routine Reader). It teaches all ten routines below and
unlocks the two gear recipes. Its material bill names only what its
prerequisites can make, which `tests/assets.rs` already asserts.

### Tamper routines

Starting figures, to be priced against the neighbouring debuffs during the plan:

| id | name | shape | range | effect | cooldown |
|---|---|---|---|---|---|
| `cold_sample` | Cold Sample | Single | 5 | `Temperature(0.0)`, 3 turns | 3 |
| `heat_injection` | Heat Injection | Radius 1 | 4 | `Temperature(2.0)`, 2 turns | 4 |
| `inference_probe` | Inference Probe | Single | 6 | `Profiled`, 3 turns | 3 |
| `prompt_injection` | Prompt Injection | Single | 3 | `Injected`, 1 turn | 5 |
| `hallucination` | Hallucination | Radius 2 | 5 | `Hallucinating(decoys: 3)`, 3 turns | 5 |

All carry a `power_cost` and a `cooldown`, both census-required.
`breaks_cloak` is **true**: a tamper names the other side, which is the act a
cloak waits for, and `Debuff` already answers true.

### Flavour routines (existing effects)

| id | effect |
|---|---|
| `gradient_descent` | `Damage` |
| `backprop` | `Drain` |
| `dropout` | group `Debuff(kind: Stun)` |
| `fine_tune` | ally `Buff(kind: Atk)` |
| `data_poisoning` | `Debuff(kind: Bleed)` |

### Gear

Two mods, recipes at the Fabricator, costed in `cache_grain` because a
zone-gated gear recipe must name a zone material:

- **Adversarial Patch** — Evasion.
- **Attention Head** — Accuracy.

Both are held to the two price bounds and the item censuses like any other gear.

## Scope and blast radius

Three crates, one schema change (`AbilityEffect::Tamper`), no save-format change.

- **engine**: `abilities.rs` (variant, `TamperKind`, `breaks_cloak`,
  `tactical_only`, affinity category), `components.rs` (`Tampered`),
  `tactical/ai.rs` (temperature door, sides, extracted planning), `tactical/`
  turn and view (decoys, forecast, tags, the swing-at-a-cell door), the routine
  resolution for `Tamper`, `tactical_awaits_input`, group-model filters.
- **app-core**: the cursor and pacing of a taken-over companion, if
  `tactical_awaits_input` alone does not cover it.
- **gui**: decoys, strip tags, forecast row and path.
- **assets**: five tamper routines, five flavour routines, two items, one
  research node. `assets/abilities/README.md` documents `Tamper`;
  `assets/research/README.md` and `assets/items/README.md` only if a field
  changes meaning.
- `balance_sim` models no abilities and is not gated by any of this.

## Out of scope

- Tamper routines on hostile species, and the AI scoring that needs.
- A new AI-themed species (it belongs with the above).
- New materials, each of which needs a producing structure.
- Todo #8 (more machine learning) and #71 (recorded moves).
- Immunity for bosses or apex species — a playtest question, not a design one.

## What the tests have to prove

Engine, unless noted. The forecast and decoy tests must be checked by removing
the behaviour and watching them fail.

1. A body under `Temperature(0.0)` takes the argmax and draws nothing from
   `GameRng` over its turn; the same body without it draws.
2. Every tactical AI call site reads `decision_temperature` — a Heat-tampered
   body's choices differ from the constant's at a fixed seed.
3. **At temperature zero, a profiled hostile's forecast equals what its turn
   then does** — action, destination and target — across a seeded sweep of
   boards, not one fixture.
4. Above temperature zero the forecast carries the action and no destination.
5. An injected hostile's swing lands on a packmate; that packmate's death pays
   XP and loot.
6. An injected hostile with a heal routine aims it at the party.
7. Hallucination places the authored decoy count deterministically; a
   hallucinating hostile walks toward its nearest decoy; striking it destroys
   it and spends the turn; with none left the entry is removed.
8. A party body's walk, swing and aim ignore party-owned decoys, and a
   companion caught in a party Hallucination carries no entry.
9. A tamper's radius skips the player; a single-target tamper at the player is
   refused before Power or cooldown is spent.
10. A Heat-caught companion stops awaiting input, is driven by the AI, and
    returns to command when the entry ends; Profiled on a companion does not
    take it over.
11. A `duration: 1` Injection cast on a body that has already acted this round
    is still active on that body's next turn.
12. `Tampered` and decoys are gone when the fight ends.
13. No AI selects a tamper routine: `choose_wild_action`, `tactical_ai_beat`,
    `tactical_auto_beat`.
14. The group model's routine rows omit tamper routines.
15. Asset censuses: the new variant in the `breaks_cloak` census; every new
    routine has a cooldown, a Power cost and a battle-map shape; the research
    bill is makeable; the gear recipes name `cache_grain`.
16. gui: the widest `TurnRow` with all five tags and a forecast fits the strip.
