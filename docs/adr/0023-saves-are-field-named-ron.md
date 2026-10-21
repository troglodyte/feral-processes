# 0023 Saves are field-named RON

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: in `CLAUDE.md` by 2026-09-08; recorded here 2026-10-21
- Pattern: Tolerant reader

## Context

A save written by one release has to load in the next.
Positional formats break on every added field and need migration code per version.

## Decision

The save is field-named RON (`save::SaveData`).
A new field is `#[serde(default)]`, which costs no version bump.
A field removed, or one whose meaning changes under a name it keeps, bumps `save::SAVE_FORMAT_VERSION`, and that bump is what "breaking" means for a release.
The version is the file's first line, so an unreadable file is refused by version.
Prefer a named struct to a positional tuple in any saved type.

## Alternatives considered

- A binary format with migrations. Each release carries migration code forever.
- Positional tuples in saved types. The one shape field-named RON does not protect; the next property costs a legacy field.

## Consequences

Most save changes are additive and free.
A skipped or defaulted field is invisible to a RON round-trip test, so a new save field needs a save-then-load test.
`CHANGELOG.md`'s versioning digit keys off this record's notion of breaking.

## Evidence

- `crates/engine/src/save.rs` — `SaveData`, `SAVE_FORMAT_VERSION`.
- `.claude/rules/seams-screens.md` — the save rule; trap in the `seams` skill, `references/screens.md`.
- Memory: `ron-round-trip-cannot-catch-a-skipped-field`.
