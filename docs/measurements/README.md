# Measurements

What the instruments said, and when. One file per question answered.

This is not a plans directory and not a specs directory. `docs/superpowers/`
holds those, and its own lesson — recorded in `CLAUDE.md` under **Process
weight** — is that ~35,000 lines of write-once prose accumulated there and
essentially none of it was ever read twice. A measurement is different in
one specific way that earns it a home: **the data it came from is usually
gone.** Telemetry sweeps are hundreds of megabytes and gitignored; a
`Game::new` seed reproduces a world but not the run someone played. If the
number is not written down it has to be re-measured, and re-measuring costs
CPU-hours and a person's afternoon.

## What belongs here

A file earns a place if all three hold:

- **Something was actually run**, and the numbers are what it printed —
  not what a design expected it to print.
- **The data behind it is not in the repo**, so the claim cannot be checked
  by reading code.
- **A future decision depends on it.** A number nobody would act on is
  telemetry, not a finding.

Balance curves do *not* belong here: `balance_sim.rs` asserts those against
the live constants, so they are checked on every `cargo test` and a file
here would be a second copy that drifts. The rule is the same one `CLAUDE.md`
states about doc comments — if a claim can be a test, make it a test.

## How to write one

Name it `YYYY-MM-DD-<what-was-measured>.md`. Every entry carries four
sections, in this order, because a reader arriving cold needs them in this
order:

1. **The claim** — one paragraph, up front. What is now known.
2. **How to reproduce it** — the exact commands, seeds and budget. This is
   the part that rots least and matters most; a finding you cannot re-run
   is folklore.
3. **The numbers** — tables. Say which are new and which reproduce something
   already believed, because a replication and a discovery are different
   kinds of evidence.
4. **What it does not say** — the blind spots, stated rather than left to be
   inferred. Every instrument in this repo has them and the failure mode is
   always someone quoting a number past its range.

Add an open-questions section when the run raised something it could not
settle. Leave it in when it is still open; a stale "resolved" is worse than
an honest "unknown".

## Entries

- [2026-08-10 — Enemy policy pin sweep](2026-08-10-enemy-policy-pin-sweep.md)
  — the three pinned features are a real design boundary, not a tuning
  accident, and an unpinned policy downs zero companions in 1,600 fights.
- [2026-08-10 — Stun move levers](2026-08-10-stun-move-levers.md) — power is
  a switch and not a dial, so repricing an effect move cannot buy variety;
  duration can, and the asset edit is inert without a retrain.
- [2026-08-10 — A party that braces](2026-08-10-a-party-that-braces.md) —
  Defend blunts a trained enemy and loses to a random one. The pins are
  justified — but the 2026-08-09 reason for pinning `target_bracing` was an
  unidentifiable weight, since All-Attack never braces.
