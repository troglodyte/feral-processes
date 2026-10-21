# 0019 New content defaults to Rust

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: 2026-10-19 (077f90ae); widened to new fields 2026-10-21
- Pattern: —

## Context

Moddability is not a goal.
Data in `assets/` costs string ids, a loader, a schema README, census tests and the `#[serde(default)]` discipline.
For a handful of entries that cost buys nothing that an enum and `tuning/` constants do not.

The damage-types effort is the motivating case.
The parked brainstorm (`docs/superpowers/specs/2026-10-16-damage-types-parked.md`) took "moddable: the type list belongs in data" as a constraint.
The design spec (`2026-10-19-damage-types-design.md`) said "No asset files" but allowed new fields on existing item and species `.ron` files.
Stages 1, 2, 4 and 5 then added damage-type tags to items, species and abilities, with README edits and a census test (`every_damage_type_a_shipped_attack_carries_has_armour_resisting_it`), without weighing Rust instead.
The rule covered new directories but not new fields, so the choice was never made.

## Decision

A new content kind defaults to Rust: an enum, a `tuning/` module, a match.
A new `assets/<kind>/` is made only when it will hold many entries tuned often, or is not code (help markdown, sprites, sounds, effects).
**New fields on existing `.ron` files are also a data-vs-Rust decision**, and need an ADR before code, like a new directory.
The large sets already in `assets/` (items, abilities, contracts, research, structures, memories, species) stay data.
A kind holding one or two files may fold back into Rust when already being changed, not as a project.

## Alternatives considered

- Data by default, for modders. Moddability is not a goal; the cost is paid on every kind.
- Decide case by case with no default. That is how the damage-type fields landed without a decision.

## Consequences

Fewer loaders, READMEs and census tests.
A per-entry property of an existing data set (a species' resistance) may still belong in its `.ron`, but the ADR says why.
The damage-type fields already shipped are not reverted by this record.

## Evidence

- `CLAUDE.md` — Content: data or code.
- `crates/engine/src/damage/` — `DamageType`.
- `.claude/rules/content-schema.md` — per-kind traps.
