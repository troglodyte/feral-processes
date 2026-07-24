# Soften raids

## Problem

Raids are not fun. Three specific complaints, in the player's words:
structures are lost outright, the chip damage can't be out-healed, and raids
fire too often. Guards taking damage is *not* a complaint — that part of the
mechanic reads fine.

The arithmetic backs all three up. Today (`crates/engine/src/lib.rs:209-247`,
`structures.rs:165`):

| Dial | Value | Consequence |
|---|---|---|
| `RAID_CHANCE_PER_TICK` | 0.02 | a raid every ~50 ticks |
| `RAID_DAMAGE` | 10 | vs. `default_durability` 30 → 3 hits and the structure is despawned |
| `STRUCTURE_REGEN_AMOUNT` / `_INTERVAL` | +2 / 20 ticks | ~100 ticks to undo one raid's damage |

A raid lands every ~50 ticks and heals back over ~100. Structures lose that
race by construction, and losing means a despawn plus the build cost plus a
dropped cronjob. `DifficultyMode` doesn't help — it only gates permadeath.

There *is* counterplay already: each Shield contributes `raid_defense: 4`
base-wide and they stack, so three Shields grant immunity. The baseline is
punishing enough that this reads as all-or-nothing rather than as a dial.

## Approach

Retune the constants. No new mechanics, no renderer changes, no save-format
changes — every dial here is a constant or a `.ron` value, so this stays
cheap to re-tune after play-testing.

Two approaches were considered and rejected:

- **Raids chip but never destroy** (floor `Durability` at 1). Removes the
  sting completely, but then `Durability`, Shields, and guards all stop
  meaning anything, and raids become log lines with nothing at stake — which
  makes the "too noisy" complaint worse, not better.
- **Scale raid intensity to `DifficultyMode`.** Adds a second meaning to an
  enum that currently only gates permadeath, forces a raid preference onto a
  choice made about death, and locks it in at run start.

## Changes

### Constants (`crates/engine/src/lib.rs`)

| Constant | From | To |
|---|---|---|
| `RAID_DAMAGE` | 10 | 4 |
| `RAID_CHANCE_PER_TICK` | 0.02 | 0.012 |
| `STRUCTURE_REGEN_AMOUNT` | 2 | 4 |

`RAID_DEFENDER_DAMAGE` (6) and `STRUCTURE_REGEN_INTERVAL` (20) are unchanged.

### Data and schema docs

`assets/structures/shield.ron`: `raid_defense: 4` → `2`. Without this, a
single Shield would fully absorb a 4-damage raid and immunity would become
all-or-nothing again at the very first Shield.

`assets/structures/README.md` (~line 102) cites the shipped Shield's
`raid_defense: 4` as its worked example, so it moves in the same commit. This
is required by CLAUDE.md: schema docs are the reference for anyone modding,
and they change alongside the values they document.

## Resulting behaviour

- A raid every ~83 ticks.
- 8 hits to destroy a fresh 30-durability structure (was 3).
- One raid hit fully regenerates in one 20-tick interval (was ~100 ticks).
- One Shield → 2 damage/raid → 15 hits to destroy.
- Two Shields → 0 damage → immune.

Destruction stops being attrition and becomes the outcome of the same
structure being picked repeatedly in a short window. The Shield progression
becomes a legible ramp instead of a cliff.

## Test impact

Most raid tests reference the constants symbolically (`lib.rs:10551`,
`10641`, `10987`) and adapt with no edit.

The exposure is four seed-hunting tests — `raid_check_can_damage_an_undefended_structure`
(`lib.rs:10437`), `raid_damage_message_is_tagged_message_kind_raid` (`10470`),
`deployed_shields_reduce_raid_damage_to_an_undefended_structure` (`10517`),
and `a_raid_fully_absorbed_by_the_shield_network_queues_a_deflected_effect`
(`10630`). Each sweeps 300 seeds calling `raid_check` **once** per seed and
panics if no raid ever fires.

Dropping the roll to 0.012 moves the odds of an all-miss sweep from
`0.98^300` ≈ 0.23% to `0.988^300` ≈ 2.7%. These are seeded and therefore
nominally deterministic, but unsorted habitat lookup can shift RNG
consumption between runs, so an all-miss sweep is a live flake rather than a
stable pass — and CLAUDE.md forbids flaky tests.

**Fix:** call `raid_check` up to 7 times per seed, breaking on the first
observed hit. Seven hits at 4 damage is 28, below the 30 durability floor, so
the structure can't be destroyed mid-sweep and invalidate the assertion. This
keeps the ~300 `Game::new` calls (the expensive part) while taking all-miss
odds to `0.988^2100` ≈ 1e-11.

## New coverage

- `RAID_DAMAGE` cannot one-shot a `default_durability` structure.
- A single deployed Shield leaves raid damage nonzero — a regression guard
  against `raid_defense` silently drifting back to total immunity.

## Out of scope

- `RAID_DEFENDER_DAMAGE` — guards are not the complaint.
- The destroy-at-zero rule in `damage_structure`.
- `raidable` / Home immunity (shipped in the preceding branch).
- `STRUCTURE_REGEN_INTERVAL`.

## Known side effect

`structure_regen` queries every `Durability` holder, not just raid targets,
so Nests (`NEST_DURABILITY` 60) also regenerate 4 per 20 ticks instead of 2.
Against `attack_nest`'s per-bump `effective_atk` damage that is noise, but it
is a real change to how nest clearing behaves and is accepted deliberately
rather than special-cased — a raid-only regen path would mean branching a
system that has no reason to know about raids.
