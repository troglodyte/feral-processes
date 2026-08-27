# Measurements

What the instruments said, and when. One file per question answered.

This is not a plans directory and not a specs directory.
`docs/superpowers/archive/specs/` holds the specs, and its own lesson —
recorded in `CLAUDE.md` under **Process weight** — is that ~58,000 lines of
write-once prose accumulated there and essentially none of it was ever read
twice. The 46 implementation plans were deleted outright on 2026-08-13 for
exactly that reason; git history is their archive. A measurement is
different in one specific way that earns it a home: **the data it came from
is usually gone.** Telemetry sweeps are hundreds of megabytes and
gitignored; a
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

- [2026-08-27 — Fitting the zone level cap](2026-08-27-zone-level-cap.md)
  — the spec's proposed constants would have made every zone past 6
  unclearable; `STEP = 11` is the smallest slope that does not, derived by
  calling `min_level_to_clear_zone` over zones 1-16. Also records that no
  straight line can sit under the gear-free curve at both ends of the range,
  and that `dev-arenas/developed-companion.ron` is byte-identical to its own
  control.
- [2026-08-24 — What stacked affixes are worth](2026-08-24-stacked-affix-power.md)
  — fusion now unions two copies' affixes, and four of one weapon affix moved
  an on-curve fight from a **60%** win rate to **90%**, a bigger swing than
  the whole Ordinary-to-Prismatic ladder buys. `balance_sim` models no fusion
  and sees none of it.
- [2026-08-19 — What an unoptimised dependency graph costs a frame](2026-08-19-debug-build-frame-cost.md)
  — `cargo run` was under 20 fps because the workspace had no `[profile.dev]`
  section, so bevy, wgpu and egui compiled unoptimised into the playable
  build. The renderer's shape pass alone measured **51.4 ms** a frame in
  debug against 2.0 ms in release, at an identical shape count; deps at
  `opt-level = 3` bring it to 2.3 ms. Found from a play report that the map's
  camera glide had gone jerky — the animation was intact, there were simply
  two frames per step to draw it in. Also retires the standing claim that the
  engine suite's ~24 s was an unavoidable debug artifact: it is 6.7 s.

- [2026-08-19 — What removing the passive party bonus cost the player](2026-08-19-party-passive-bonus-removal.md)
  — companions no longer lend the player a tenth of their ATK/DEF on top of
  acting. Across four party-bearing scenarios at 200 reps, fights got 0.3-0.4
  rounds longer and the player kept 1-3 points less Integrity; three were
  already walkovers and stayed 100%, and the one marginal scenario moved
  25.0% -> 21.0%, **within noise**. Carries the structural finding that
  `balance_sim` never modelled the term despite a doc comment claiming it, so
  the removal moved no curve at all — and the caveat that the deltas are two
  different RNG streams, not the same fights refought.

- [2026-08-19 — What the attack roll did to the shipped arenas](2026-08-19-combat-model-slice-1.md)
  — the combat model's first slice measured across all fourteen `dev-arenas/`
  scenarios. Every verdict holds: the same twelve wins, the same two losses,
  and `stack-depth-5` still 0% over 50 runs — but it now takes 11.2 rounds to
  lose where it took 7.4, which is the "levers that only cut incoming damage
  lengthen the loss" prediction landing. Carries the finding that **nine of
  fourteen scenarios are walkovers**, so the arena gates almost nothing about
  difficulty, and the four knobs this slice set on judgement rather than
  measurement.

- [2026-08-19 — The Stack's depth curve, and where it stops being winnable](2026-08-19-stack-depth-curve.md)
  — depth 2 and 3 are 100% wins, depth 4 is 78% over 46 rounds losing 2.4 of 3
  companions, and depth 5 is **0%** for the strongest party the game can field
  (and for the lair at that depth too). Three curves move per frame and
  multiply: +0.35 stats, **x2 bodies** — the one geometric difficulty curve left
  in the game — and a band of species. Carries the lever sweeps (stat step is
  lethality, body count is duration, 8 bodies at 0.20 reaches 62% without
  moving the depth-2 lair), the note that nothing automated gates any of it, and
  the untested third axis.

- [2026-08-19 — What a developed companion is worth](2026-08-19-developed-companion-worth.md)
  — the companion-progression branch measured before merge. Every Kernel Ring
  open roughly doubles a companion's power (177 → 345) and a fully spent
  generic tree adds 9% on top (345 → 376), so the ring is the power and the
  talents are the shape — which is why the sale needs no `PurchasedTiers`-shaped
  receipt. A fully ringed party clears a zone-3 group 18% faster and still loses
  **every** rep at depth 5. Carries the two things the bin cannot see (no
  Special ever fires, so three of four node kinds are unmeasured) and the note
  that the arena's companion clamp moved, so five existing scenarios now field
  the level-12 party they were authored for.

- [2026-08-18 — What the eight granting items are worth](2026-08-18-gear-passive-worth.md)
  — the gear-passive branch measured before merge. Seven of eight items earn
  their slot on curve and `watchdog_tap` rescued one fight in a hundred;
  `AllyWounded` fires in 5% of easy runs, 25% of on-curve wins and 100% of
  losses, so `WOUNDED_INTEGRITY_FRACTION` stays at 0.33; `RoundStart` uptime
  is flat at ~1-in-4.3 and does not front-load. Carries the two findings
  nobody asked for: a bare `def: 2` module beats every grant at level 12 and
  is worth *exactly nothing* at 36, and `deadman_relay` is identical to the
  etched disk rep for rep over 100 fights.

- [2026-08-18 — The wild population is a halo around the player](2026-08-18-wild-population-halo.md)
  — 15 in the box at the base, 10/6/3/1 at 25/50/75/100 tiles out, and
  zero to two per box along 300 tiles of walked ground against a target of
  12. Why `0.5.12`'s density target flattened the peak without changing the
  shape, and the arithmetic showing the ambient roll is an order of
  magnitude too slow to fill ground at walking speed. Carries the
  after-numbers for the per-chunk population that replaced it, and the
  simulation cost of holding three to five times as many creatures alive.

- [2026-08-15 — Challenge-scaled XP pacing](2026-08-15-challenge-xp-pacing.md)
  — what a level costs now that a kill is priced by difficulty: 34 kills to
  level 5 in the opening, 208 to grind zone 1 from 5 to 10, 37 to do it four
  frames down instead. Also the replication that showed the level-coarsening
  half was power-neutral.
- [2026-08-10 — Enemy policy pin sweep](2026-08-10-enemy-policy-pin-sweep.md)
  — the three pinned features are a real design boundary, not a tuning
  accident, and an unpinned policy downs zero companions in 1,600 fights.
- [2026-08-10 — Stun move levers](2026-08-10-stun-move-levers.md) — power is
  a switch and not a dial, so repricing an effect move cannot buy variety;
  duration can. Carries a correction: measured roster-wide, the retrain that
  shipped with it made most species *less* varied.
- [2026-08-10 — Weight identifiability](2026-08-10-weight-identifiability.md)
  — **read this before quoting any trained weight.** Retrained at three
  seeds, seven of the sixteen free features flip sign at indistinguishable
  fitness; only `target_hp_frac` and `est_damage_frac` are stable.
- [2026-08-10 — A party that braces](2026-08-10-a-party-that-braces.md) —
  Defend blunts a trained enemy and loses to a random one. The pins are
  justified — but the 2026-08-09 reason for pinning `target_bracing` was an
  unidentifiable weight, since All-Attack never braces.
