# 0026 No CI; releases are manual per platform

- Status: Accepted (standing)
- Decided by: user (CLAUDE.md)
- Date: 2026-09-28 (34f25937); recorded here 2026-10-21
- Pattern: —

## Context

Releases ship for several desktop platforms, each with its own packaging steps and a manual checklist that needs the real machine.
Cross-compiling from Linux to Windows is the less-travelled path for wgpu.

## Decision

There is no continuous integration and no cross-compiling.
Each platform's release is built and checked by hand on that platform, following `docs/releasing.md`.
`cargo test --workspace` run locally is the gate.
One release per change landing on `main`: bump the workspace version, add a `CHANGELOG.md` section, tag `vX.Y.Z`.

## Alternatives considered

- CI with `cargo-dist` or similar. A Windows machine is needed for the manual checklist regardless.
- Cross-compile from Linux. Nothing is installed for it, and the GNU ABI is the riskier path for wgpu.

## Consequences

The full suite is only as reliable as the habit of running it; flakes are read, never retried away.
A release is a deliberate act, which suits one release per landed change.

## Evidence

- `docs/releasing.md`.
- `CLAUDE.md` — Guardrails.
- [0011](0011-restructurings-considered-and-rejected.md) — "Continuous integration".
