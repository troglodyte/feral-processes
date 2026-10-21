# 0016 Shipped asset kinds deny unknown fields

- Status: Agreed
- Decided by: user
- Date: 2026-10-21
- Pattern: —

## Context

`#[serde(deny_unknown_fields)]` is set on the save, research and achievements types only.
Every other asset kind ignores a field it does not know, so a typo in a `.ron` file — `spred:` for `spread:` — parses, the real field takes its default, and nothing says so.
The game then ships with a number nobody authored.

## Decision

Every shipped asset kind's definition types carry `#[serde(deny_unknown_fields)]`.
A file with an unknown field fails to parse, and its loader skips it with a logged warning, following the `*Db::load_dir` pattern; it never panics.
The shipped-assets test then catches the typo, because the file produces a warning.

This sits beside the `#[serde(default)]` rule in `CLAUDE.md`, not against it.
A missing field still takes its default, so older files keep parsing when a field is added.
An unknown field is rejected, because it is either a typo or a field this binary does not have.

## Alternatives considered

- Leave it lenient. Typos stay silent.
- Warn on unknown fields but still load the file. Needs a wrapping deserializer (such as the `serde_ignored` crate) in every loader, for a file that is wrong either way.
- Reject only in tests. The same typo would load in the shipped game with a different number than the test saw.

## Consequences

A typo'd field shows up at load as a skipped file and a failing shipped-assets test.
A `.ron` written for a newer schema is skipped by an older binary; moddability is not a goal ([0019](0019-content-defaults-to-rust.md)), so that is accepted.
Not built yet: it is in [backlog.md](backlog.md).

## Evidence

- `crates/engine/src/save.rs`, `research.rs`, `achievements.rs` — the existing `deny_unknown_fields`.
- `SpeciesDb::load_dir` (`crates/engine/src/species.rs`) — the skip-and-warn pattern.
- `every_shipped_asset_file_loads_without_a_warning` (`crates/engine/src/tests/assets.rs`).
