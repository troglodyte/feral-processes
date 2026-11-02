# 0038 Tamed programs carry practised skills, in a subsystem that knows nothing about work

- Status: Proposed
- Decided by: Claude — proposed, awaiting the user
- Date: 2026-11-02
- Pattern: Module per domain; Functional core, imperative shell
- Depends on: [0020](0020-functional-core-imperative-shell.md), [0023](0023-saves-are-field-named-ron.md), [0024](0024-code-is-organized-by-domain.md)

## Context

A posted program's work speed today comes only from its species (`species_base_speed`), the room (`SiteScales::room`), build quality and the player's class (`classes::work_tick_scale`).
A program tamed yesterday works exactly as fast as one that has run the same machine all game.
The user wants RimWorld/Dwarf Fortress work skills: they start low, rise with practice and never decay, and only tamed programs have them.
They also want the machinery built so that combat and other areas can add skills later without it being rebuilt.

The only per-program progression today is `components::Experience` (the combat level, `progression::xp_for_level`).
Work already feeds it a little (`tuning::WORK_XP_PER_CYCLE`, capped at `WORK_XP_LEVEL_CAP`).
That stays as it is; skills are a separate axis.

## Decision

**A new subsystem directory, `crates/engine/src/skills/`.** It holds:

- `Skill`, a closed enum. For now it has only the work variants: `Extract`, `Assemble`, `Research`, `Build`, `Dig`, `Farm`.
- `SkillTrack { level, practice }`.
- `Skills`, the per-program component: one track per `Skill`.
- Pure functions:
  - `practise(track, amount, curve) -> PractiseOutcome { levels_gained }`, which applies the practice, rolls levels over and stops at the cap.
  - `practice_to_next(level, curve)`.

The curve's constants live in `tuning/skills.rs`.
The subsystem never names a structure, task or duty.
What a level is *worth* belongs to the area that reads it: the work speed scale is a function in the base labour code, not in `skills/`.
A combat skill later means a new `Skill` variant, a new `practise` call site and its own effect function. Nothing in `skills/` changes shape.

**Who has it.** `Skills` is inserted only by `Game::roster_parts`, which is the one bundle every tamed program gets.
An entity without `Skills` (wild, settlement, the player) is never practised, and a work site reads it at scale 1.0, which is today's speed.

**No decay.** Practice only accrues. There is no clock on idle skills.

**Save.** `CreatureSave` gains `skills` under `#[serde(default)]`, as a list of `{ skill: String, level, practice }`.
The skill is saved by name, following `Duty::name`/`Duty::from_name`, so an unknown name loads inert.
This needs no `SAVE_FORMAT_VERSION` bump (0023).
An absent field loads as the species' starting levels ([0039](0039-species-carry-starting-skill-levels.md)), the same as a fresh tame.

## Alternatives considered

- **Fold skills into `Experience`.** That ties combat level and work practice together and makes the existing level curve and its `balance_sim` gate carry base work.
- **A generic `SkillSet<K>` keyed by a type parameter per area.** That builds the abstraction before there is a second area to need it. One enum that grows a variant is enough until combat arrives.
- **Skills as data (`assets/skills/`).** Six entries that are not tuned often. 0019 defaults this to Rust.
- **Plain struct outside the ECS (0021).** Skills belong to many ticking entities (the staff) and live and die with them, which is the case 0021 leaves to ECS.

## Consequences

Every tamed program has a new component, and every work site multiplies its rate by a level-derived scale.
Level 0 is slower than today, so a new base, and every loaded save's crew, works slower until it practises.
Engine tests that count labour ticks against today's speed will move. The plan must decide per test whether to pin a skill level or update the number, and must never just loosen an assertion.
A future combat skill reuses `Skill`, `SkillTrack`, `practise` and the save shape.

## Evidence

`Game::roster_parts`, `components::Tamed`, `components::Experience`, `systems::work_ticks_at_speed`, `systems::SiteScales`, `Game::work_ticks_for`, `classes::work_tick_scale`, `tuning::WORK_XP_PER_CYCLE`, `save::CreatureSave`, `duties::Duty::from_name`.
