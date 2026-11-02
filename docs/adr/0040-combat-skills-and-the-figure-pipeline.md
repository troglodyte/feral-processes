# 0040 Combat skills, and a figure pipeline that programs' attributes can join later

- Status: Accepted
- Decided by: user, on Claude's proposal
- Date: 2026-11-02
- Pattern: Module per domain; Functional core, imperative shell
- Depends on: [0038](0038-tamed-programs-carry-practised-skills.md), [0039](0039-species-carry-starting-skill-levels.md), [0016](0016-shipped-assets-deny-unknown-fields.md), [0023](0023-saves-are-field-named-ron.md)
- Amends: [0038](0038-tamed-programs-carry-practised-skills.md) (the player now carries `Skills`)

## Context

0038 built practised skills for base work and planned for combat.
The user wants combat skills that start slightly below today's numbers and rise above them, for tamed programs and the player.
They also want each combat figure to come from an **attribute plus a skill**, with to-hit and damage on different ones.
Today no attribute feeds to-hit, and Analysis feeds ATK.
Only the player's attributes become combat stats (`progression::derive`). A program fights off stored stats, and its attributes are mostly unused.
The user wants that fixed in a later effort and wants this one built so that effort is small.

This record bends three standing positions:
- 0038's "the player never carries `Skills`";
- the seam "a class grants affinities and nothing else";
- Analysis feeding ATK.

Spec: [`docs/superpowers/specs/2026-11-02-combat-skills-design.md`](../superpowers/specs/2026-11-02-combat-skills-design.md).

## Decision

**A combat figure is a base figure scaled by a practised skill, and the two halves have one door each.**

- **The base figure** comes from `Game::base_figure(entity, DerivedStat)`, which does not branch on player versus program at its callers.
  - Today: the player arm is the existing `derive` output and the program arm is the stored stat. A program has no accuracy contribution, so that arm returns 0 for it.
  - The follow-up replaces only the program arm, with `derive` over a species-built `DerivedBase`.
- **The skill scale** is `skills::scale(level, spread) = 1 + spread·(level − 10)/10`, a pure function.
  - Work's existing linear scale becomes a call to it with `spread` 0.5, with unchanged numbers.
  - Combat calls it with `spread` 0.1.
  - What a scale is worth stays at each reading door.

**Four combat skills**, appended to `Skill`: Aim, Force, Defence, Routines. `Skill::WORK` and `Skill::COMBAT` split the families for their readers.

| Figure | Attribute | Skill | Door |
|---|---|---|---|
| to-hit | Analysis (new `DerivedStat::Accuracy`) | Aim | `Game::combatant_profile` |
| damage | Amplitude (new attribute, takes `Atk` from Analysis) | Force | new `Combatant::damage_scale`, applied in `resolve_attack_inner` and `expected_damage` |
| mitigation | Footprint | Defence | `Game::effective_mitigation`, (level − 10) points before status scaling |
| Power cost | none | Routines | `abilities::routine_power_cost(def, Option<level>)` |

**Practice:**
- Aim and Defence practise on every creature-versus-creature attack roll.
- Force practises on a landed one.
- Routines practises inside `Game::spend_power`.

**The player carries `Skills`,** inserted when the class is applied at creation and saved under `PlayerSave::skills`, `#[serde(default)]`. Work's `skill_scale.rs` treats the player as having none.

**A class carries `starting_skills`,** combat only. The class seam becomes "affinities and starting combat skills", rewritten in the rules file, the `seams` skill and the graph.

**Load overlays saved skill rows on the seed** (species or class), so a skill a save does not name loads at its seed.

**`balance_sim` stays skill-blind.** The attribute move reaches it through the player's derived stats, and that is gated: starting ATK must not move.

## Alternatives considered

- **One Attack skill for hit and damage.** The user split them so different bodies grow differently.
- **Entropy or a new attribute for to-hit, Analysis keeping ATK.** The user chose Analysis for to-hit and a new Amplitude for damage.
- **Derive programs' stats from attributes now.** That moves every program's numbers and refits `balance_sim`. It is deferred to its own effort, and this design keeps it to one arm of one function.
- **A separate `CombatSkills` component for the player.** That would mean two types, two save fields and two displays. The inert work tracks cost less.
- **Teaching `balance_sim` skills.** It would assert an assumed practice level nobody has measured.

## Consequences

- Every tamed program and the player fight slightly worse on a fresh run, and after an old save's first load, until they practise.
- `routine_power_cost` and `Combatant` change shape, so every caller and constructor is touched.
- The attribute catalogue grows to seven, and every class file authors Amplitude.
- The follow-up "programs' attributes count" is one arm of `Game::base_figure`, with its own spec and `balance_sim` refit.

## Evidence
