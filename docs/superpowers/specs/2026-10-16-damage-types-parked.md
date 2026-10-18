# Damage types and resistances — parked brainstorm

**Status:** parked. Explored 2026-10-16, **not implemented**, no design
approved. Everything below is a direction, not a decision, unless marked as
the user's.

## The question asked

> Can we brainstorm some different damage types that maybe stay in lore?

Today there are no damage types: every hit is plain Integrity damage. The
only variety comes from statuses (`assets/statuses/`: bleed, poison, stun,
throttled, exposed, locked) and the Tamper kinds (Temperature, Profiled,
Injected, Hallucinating).

## What the user said

- Damage types should be about **weapon damage and resistances**.
- **Keep the side-effect idea** — each type also doing something besides
  damage — "I like that train of thought".
- Thermal, magnetic, noise and surge "kinda work".

## Rejected directions (and why)

- **Hacking vectors as types** (brute force, flood, injection, overflow,
  spoof, escalation) — the mappings to species and effects were a stretch
  conceptually.
- **The CIA triad** (Confidentiality / Integrity / Availability) — a weapon
  dealing "confidentiality damage" doesn't make sense; damage must take
  Integrity.
- **Shred** (fragmentation as a slashing analogue) — fragmentation doesn't
  destroy data in reality; it only filled a slot.
- **Corruption as a type** — it is a *result*, not a force; it sits a level
  below the others. It survives as Radiation's side effect.

## Current shape: physical forces that wreck a machine

Five typed forces plus untyped damage. Every type is a real physical force
that destroys hardware or data.

| Type | Real-world basis | Side effect (rider on a hit) |
|---|---|---|
| **Thermal** | overheating | builds heat on the target; hot targets take more (ties to the Temperature tamper) |
| **Magnetic** | degaussing, magnetic erasure | blunt erasure that ignores flat mitigation |
| **Noise** | signal interference | weak damage, throws off accuracy/evasion |
| **Surge** | voltage spike, EMP | big single hit, can arc to a neighbour |
| **Radiation** | cosmic-ray bit flips (real in servers and spacecraft) | corruption: lingering damage that blocks repair |
| *untyped* | kinetic | today's basic hits, unchanged |

Radiation as a replacement for Corruption, and dropping Shred, were Claude's
recommendation; the user had not confirmed them when the topic was parked.

## Played back, not yet confirmed

The last playback before parking. The user asked "what's wrong or missing"
had not been answered.

**Weapons deal one type**, alongside their existing damage band. Shipped
weapons sort by name (Claude's guess):

- Surge: `arc_lance`, `interrupt_coil`
- Thermal: `plasma_router`, `overclock_core`
- Noise: `broadcast_storm`, `scatter_lance`
- Magnetic: `siege_compiler`, null-themed gear
- Radiation: nothing yet — room for new gear
- Untyped/kinetic: the blades — `kinetic_edge`, `monofilament_whip`,
  `shiv_routine`, `shim_blade`, `ragged_edge`, `recursion_blade`

**Armour resists types**, keeping general mitigation and adding per-type
resistance. Shipped armour that already reads that way:

- Thermal: `ablative_plating`
- Noise: `static_mesh`
- Radiation: `parity_weave` (parity is real error correction)
- Magnetic: `null_weave` / `nullsteel_plate`
- Surge: `hardened_shell` or a grounding item — undecided

**Species carry innate resistances/weaknesses** (Claude's guess), e.g. a
hardened construct shrugs off Surge, a drone burns under Thermal.

**Resistance cuts the side effect too** (Claude's guess): static mesh stops
both Noise's damage and its accuracy penalty.

## Open questions for when this resumes

- Is the five-plus-untyped set right (Radiation in, Corruption and Shred
  out)?
- What does resistance do numerically — percentage cut, flat, immunity —
  and are there weaknesses (more than 100%)?
- Does resistance block the side effect, scale it, or leave it alone?
- Species resistances: per-species data in `assets/species/`, or derived
  from something already there?
- Do abilities carry a type as well as weapons? (Basic attacks come from
  `species::basic_attack_ability`.)
- `balance_sim.rs` will move — what curve change is acceptable?

## Constraints any design must meet

- Moddable: the type list and each type's rider belong in data, not Rust
  (new `assets/<kind>/` directory or fields on items/species with
  `#[serde(default)]`, README updated).
- Damage type and resistance on `ItemDef`/`SpeciesDef` are schema changes →
  spec-and-plan pipeline, not inline.
- Side effects should compose from the existing status behaviours where
  possible (`DamagePerRound`, `HealBlock`, `EvasionCut`, …) rather than
  adding a parallel mechanism.
