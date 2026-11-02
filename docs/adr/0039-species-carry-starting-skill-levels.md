# 0039 Species carry starting skill levels

- Status: Agreed
- Decided by: user, on Claude's proposal
- Date: 2026-11-02
- Pattern: —
- Depends on: [0038](0038-tamed-programs-carry-practised-skills.md), [0016](0016-shipped-assets-deny-unknown-fields.md), [0019](0019-content-defaults-to-rust.md)

## Context

[0038](0038-tamed-programs-carry-practised-skills.md) gives tamed programs work skills that start low.
The user chose to have a species start a few levels up in the work that suits it, in place of RimWorld-style passions or per-species learning rates.
The 19 species already carry per-species tuning in `assets/species/*.ron` (`base_speed`, the work profile behind `ProgramManifest::work_resource`).

## Decision

`SpeciesDef` gains `starting_skills`, `#[serde(default)]`, a list of `(Skill, level)` pairs.
A skill not listed starts at 0, and a species with no field starts at 0 everywhere.
The entries name `Skill` variants directly, because a shipped asset denies unknown fields (0016) and a typo should fail the census, not load as zero.
The levels must not exceed the skill cap. The loader clamps an out-of-range level and warns, following the `*Db::load_dir` skip-and-warn pattern.

The starting levels are seeded wherever `Skills` is created: `Game::roster_parts`, and the load path when a save has no `skills` field.

Content: each shipped species gets one or two entries in the work its existing profile points at, at a few levels.
The exact numbers are set in the plan and listed in `assets/species/README.md`.

## Alternatives considered

- **Per-species learning rates.** These are invisible until much later in play, and the curve becomes two-dimensional to tune.
- **A Rust table keyed by species id.** The rest of a species' tuning lives in its `.ron`. A table elsewhere could drift from a renamed species without failing.
- **Derive the start from the existing work profile.** That leaves no way to give a species a skill outside its profile, and hides the number from the asset that owns the species.

## Consequences

A species-specific edge from the first day: a species suited to digging out-digs a generalist until the generalist practises up.
`assets/species/README.md` documents the field in the same change.
A census test should check that every shipped species' `starting_skills` stays within the cap.

## Evidence

`species::SpeciesDef`, `species::SpeciesDb::load_dir`, `SpeciesDef::base_speed`, `views::manifest::ProgramManifest::work_resource`, `assets/species/README.md`.
