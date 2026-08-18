# Gear Passives: Balance Measurement

**Status:** not run. Written 2026-08-18, on branch `gear-passives`, before merge.

> A `**Status:**` header in this directory is written once and goes stale —
> fourteen shipped specs still claim they are unimplemented. Answer from
> `CHANGELOG.md` and `docs/measurements/`, never from this line.

**Goal:** decide whether the eight granting items are worth wearing, whether
any of them is obviously too strong, and whether `AllyWounded` fires at a
rate that makes it a distinct trigger rather than a slower `RoundStart`.

**Deliverable:** one file in `docs/measurements/`, named
`2026-08-18-gear-passive-worth.md`, following that directory's README —
claim, reproduction, numbers, blind spots. Not a spec update, and not a
`CHANGELOG` entry. If the numbers move a constant, that is a separate
commit with its own justification.

---

## Why the test suite does not answer this

`balance_sim.rs` is the balance regression gate for the rest of the game and
it **gates none of this**. It is a deterministic, RNG-free battle simulator
that models **no abilities at all** — no Specials, no passives, no cooldowns.
Every number below is invisible to it.

Running `cargo test -p feral-processes-engine balance_sim` after a retune here
is still correct, but it is checking for *collateral damage to the level
curves*, not validating the feature. Do not report it as evidence either way.

The engine suite is in the same position. `tests/gear_passives.rs` proves the
mechanism — a grant fires, a cooldown holds, two slots dedupe, a save round
trip keeps it — and says nothing about whether 5 power on a four-round
cooldown is worth a weapon slot.

So the instruments are `dev-arenas/` and a session, and nothing else.

## What is unmeasured

Six new abilities, all passive, none exclusive, all `power_cost: 0.0`:

| Ability | Trigger | Target | Effect | Cooldown |
|---|---|---|---|---|
| `interrupt_request` | `RoundStart` | OneEnemyGroupFront | Damage 5 | 4 |
| `clock_skew` | `RoundStart` | OneEnemyGroupFront | Debuff Bleed 2, 2 rounds | 4 |
| `parity_guard` | `RoundStart` | OneAlly (self) | Buff Def 3, 3 rounds | 4 |
| `quarantine` | `Afflicted` | OneAlly (self) | Cleanse | 4 |
| `core_dump` | `AllyWounded` | OneEnemyGroupFront | Damage 9 | 3 |
| `hot_spare` | `AllyWounded` | OneAlly (self) | Heal 8 | 3 |

Eight items carrying them, all drop-only with no recipe:

| Item | Slot | Stats | Grants |
|---|---|---|---|
| `interrupt_coil` | Weapon | atk 3 | `interrupt_request` |
| `ragged_edge` | Weapon | atk 2 | `clock_skew` |
| `crash_handler` | Weapon | atk 2 | `core_dump` |
| `parity_weave` | Armor | def 3 | `parity_guard` |
| `sandbox_liner` | Armor | def 2 | `quarantine` |
| `watchdog_tap` | Module | def 2 | `watchdog` (already shipped, exclusive) |
| `redundant_bank` | Module | def 1 | `hot_spare` |
| `deadman_relay` | Module | def 2 | `deadman` (already shipped, exclusive) |

Plus one constant: `tuning::WOUNDED_INTEGRITY_FRACTION = 0.33`, the line a
party member must cross **downward in one round** to fire `AllyWounded`. It
was chosen by argument, not measurement — half is where an ordinary exchange
puts somebody most fights, which would make the trigger indistinguishable
from `RoundStart`; a quarter is late enough that most answers no longer help.

## The method: one binary, one asset line

**The control for a granting item is the same item with its `grants` line
removed.** Delete the line, re-run, put it back. This is the only control
that is exactly stat-matched, and stat-matching is not optional: a `Damage`
passive casts as its wearer and scales with the wearer's ATK, so a control
whose stat line differs by a point folds the stat difference into the
measured delta. (This is not hypothetical — it is what made an early unit
test read 238 against 237 and look like a second passive firing.)

It is also the only control that does not rebuild. Assets load at runtime, so
toggling a `grants` line leaves the binary and the RNG stream untouched.
**Arena numbers compare within one build only** — a moved baseline across a
rebuild is a reshuffled RNG stream, not a difficulty change — so do not
compare any number here against a report from a different `cargo build`.

Where a stat-identical shipped twin exists, run it as a cross-check:
`interrupt_coil`↔`arc_lance`, `ragged_edge`/`crash_handler`↔`kinetic_edge`,
`parity_weave`↔`hardened_shell`, `sandbox_liner`↔`packet_buffer`. **No
shipped module has a pure-DEF stat line**, so the three module items have no
twin and the asset-toggle control is the only one available for them. Say so
in the blind spots.

### Scenario authoring traps

- **`equip` is top-level in a scenario file, never inside `Fresh(...)`.** A
  misplaced row is ignored in silence, and identical numbers across a sweep
  then read as a dead feature. This has happened in this repo before.
- A `Template(..)` or save-backed player takes **no** `equip`, `inventory` or
  `party` — those are `Fresh`-only, and naming one is an error, not a no-op.
- A companion takes the same `equip` rows the player does:
  `party: [(species: "scrapper", level: 12, equip: [(item: "parity_weave")])]`.
- Raise `reps` before trusting a gap. 20 reps is noise at this scale; use 50
  where a question is close.

## The questions

Answer these in order. Q3 is the one that can invalidate two items outright,
so if the budget runs short, do Q1 and Q3.

### Q1 — Is each item worth its slot?

Per item, paired against its `grants`-removed self. Report the delta in
rounds-to-win, player Integrity remaining, and companions down.

A granting item is **drop-only and priced above its stat line**, so it does
not have to beat the best bench gear — it has to beat the same stat line
plainly, by enough that a player picking it up feels the pickup. A delta
inside run-to-run noise means the numbers are too small to read, which is a
worse outcome than too strong: an effect nobody notices is a dead field.

`dev-arenas/gear-passives.ron` already exists and fields all three of the
first batch plus a companion. Extend or copy it; do not reuse its numbers
from the branch history, which were taken on a different build.

### Q2 — Does a `RoundStart` passive out-earn its cooldown?

Three of the six fire on `RoundStart`, which is the trigger that fires every
round there is. Each pays a four-round cooldown, so nominal uptime is one
round in four — but a fight that ends in five rounds gets two firings out of
a fight it would otherwise get one. Measure whether the value scales with
fight length or front-loads.

The lever if it front-loads is the cooldown, not the power: raising power
makes a short fight shorter still.

### Q3 — Does `AllyWounded` fire, and how often?

This decides whether `crash_handler` and `redundant_bank` are items at all.
Two failure modes, opposite directions:

- **Never fires.** A fight the party wins comfortably never crosses a third
  Integrity, so both items are dead weight in exactly the fights a player
  optimises for. If a curve-appropriate fight fires it in well under a
  quarter of runs, raise `WOUNDED_INTEGRITY_FRACTION`.
- **Fires every fight.** Then it is a `RoundStart` with extra steps and the
  crossing rule is buying nothing. If it fires in most runs of a routine
  fight, lower the constant.

Measure across at least three difficulty bands — a fight the party wins
easily, one on the curve, and one they lose sometimes — because the whole
point of the trigger is that it discriminates between them. `--out` the
reports and count from the loss seeds and Integrity-remaining distribution;
the summary line alone will not tell you how often the line was crossed.

Note this cannot be read off the arena's summary directly. Either instrument
it with `FERAL_DEV_LOG=1` and count the passive's log lines in
`dev-logs/battles.jsonl`, or infer it from the paired delta being zero.

### Q4 — Do three granting slots stack into something degenerate?

One item per slot is a legal loadout and nothing prevents it. Run all three
against the all-three-`grants`-removed control. Passives cost no turn, so
three of them is three free actions a round they all come off cooldown
together — check whether the cooldowns phase apart or synchronise.

### Q5 — Is `deadman_relay` too strong for a module slot?

`deadman` is Everyone-scope, power 14, and was previously reachable only by
beating Wintermute for the etched disk. The relay drops from Wintermute alone
and spends a module slot instead of a routine slot, so it is not a new source
so much as a second shape — but it is the most arguable thing on the branch.

The specific question is whether wearing the relay is strictly better than
installing the disk, since the disk costs a routine slot and the relay costs
a module slot with two DEF attached. If it is strictly better, the disk is
now dead content.

### Q6 — Is the cross-source double-fire proportionate?

A grant and an installed copy of the same routine **both** fire in one round,
sharing one cooldown entry. That is deliberate — it is what spending a
routine slot on what your gear already gives you buys — but it is the one
interaction that doubles a number. `watchdog_tap` plus an installed
`watchdog` is the reachable case. Check it is a reward and not a reason.

## What a session is for, and what it cannot be

`FERAL_DEV_ARENA=1 cargo run`, then `[R] Arena` on the main menu, is the only
way a **companion's** passive is ever seen firing in an authored fight, and
the companion case is what this feature adds for free. It is also the only
instrument that can answer the questions the arena's summary cannot:

- Does the "cuts in" line read as a thing your gear did, or as noise in the
  round log? Three granting items means up to three extra lines a round.
- Is `clock_skew`'s Bleed legible as coming from the weapon?
- Does `hot_spare` firing feel like a rescue or like a consolation?

`FERAL_DEV_LOG=1` alongside it appends every swing and decision to
`dev-logs/battles.jsonl`, which is how a fight found by feel becomes a number.
Set it before playing anything whose answer you want to keep.

An arena session writes no save, no profile and no run history by design.
That is three deliberate omissions, each with its own test asserting on the
file — the profile is the one that costs real money if it regresses.

## What would change as a result

In rough order of how likely each is to be the answer:

1. **A cooldown.** The cheapest lever and the one that does not touch damage
   curves. All six are 3 or 4.
2. **`WOUNDED_INTEGRITY_FRACTION`.** One constant, one line, and Q3 is the
   only thing that can justify moving it.
3. **A `power` figure** on one of the six abilities. Note `Damage` and `Heal`
   scale with level through `scaled_hp_power`, so a number that reads fine at
   level 12 may not at 40 — check both ends before moving one.
4. **An item's `value`.** All eight are priced above their stat line on the
   grounds that they carry an effect. If an effect turns out to be worth
   nothing, the price is a lie.
5. **Dropping `deadman_relay`.** Q5's failure case, and the cheapest fix for
   it is deletion rather than tuning.

None of these is a save-format change and none needs a `SAVE_FORMAT_VERSION`
bump. `ItemDef` and `AbilityDef` are asset data.

## Open, and not settled by this

The trigger set is now four, and `AllyDropped` is deliberately reachable by
exactly one item (`deadman_relay`) for a design reason recorded in
`assets/abilities/README.md` and pinned by
`gear_reaches_the_triggers_a_player_can_want_and_only_deadman_reaches_the_other`:
a dropped companion is dissolved and despawned with no revive at any
difficulty, so a routine paying out there only pays a player who has already
lost more than the payout is worth. Nothing in this measurement revisits
that; it is a design boundary, not a number.
