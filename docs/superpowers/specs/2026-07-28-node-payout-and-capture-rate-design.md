# Node payout and capture rate rebalance

**Date:** 2026-07-28
**Branch:** `balance/node-payout-and-capture-rate`
**Status:** Design, approved

## Problem

Two unrelated systems, one shared failure: a linear or exponential term
running away against a term that never grows, until the growing term is the
only one that matters.

**Structure income compounds twice.** `systems::node_payout` (`systems.rs:103`)
is `tier * zone.stat_multiplier()`. `ZoneLevel::stat_multiplier` is the
*enemy difficulty* curve — a geometric `ZONE_STAT_GROWTH` doubling per zone
(1, 2, 4, 8, 16) — and reusing it as the economy curve means a node's yield
doubles with depth *and* multiplies by upgrade tier. A Mk5 Mining Node pays
5 a cycle in zone 1 and 80 in zone 5. Every sink stayed flat: a node costs 12
Core Fragments, an upgrade 10 per tier, the market sells at 3–4. Cargo is
unbounded, so nothing sheds the surplus. Reported from play at zone 2–3: 4
Core Fragments a cycle from a single node and hundreds banked with no
shortage of anything.

Expressed as time-to-fund one Zone Portal while standing idle (1 tick = 1
real second, `app-core/src/lib.rs:119`):

| | Zone 1 | Zone 2 | Zone 3 | Zone 5 |
|---|---|---|---|---|
| Mk1 | 1600 | 1200 | 800 | 300 |
| Mk3 | 381 | 286 | 190 | 71 |
| Mk5 | 178 | 133 | 89 | 33 |

Zone 3 at Mk5 funds an entire breach in 89 seconds of doing nothing.

**Decompiler skill swamps the capture formula.** `taming::capture_chance`
(`taming.rs:12`) computes a multiplicative `base` from catalyst potency, the
target's remaining HP fraction and its species' `taming_difficulty`, then
*adds* a flat `decompiler_skill * DECOMPILER_SKILL_BONUS`. `base` cannot
exceed 0.33, because the only catalyst in the game is the ICE Breaker at
`taming_potency: 0.4`. `decompiler_skill` has no ceiling: +1 per player level
forever (`DECOMPILER_SKILL_PER_LEVEL`), +1 to +4 from each of fifteen gear
items, +1 per `Perk::ExploitFocus` level. At 0.02 a point, skill 40 is worth
+0.80 — more than twice the entire designed base.

The consequence is that past roughly skill 40 every attempt pins to the
`CAPTURE_CHANCE_MAX` clamp of 0.95. A fully-weakened Overseer needs skill 40
to get there; at full HP, 46. With gear and a few `ExploitFocus` levels that
is about player level 30. Species `taming_difficulty` — the moddable 0.15–0.95
lever that is supposed to distinguish a Drone from a boss — stops meaning
anything, and so does weakening a target before the attempt. Reported from
play: a 95% chance on a boss in zone 2.

Related: `capture_chance` takes no zone argument and has no boss concept. An
Overseer in zone 5 is exactly as tameable as the same species in zone 1.
That is left as-is by this spec (see Scope).

## Scope

1. `node_payout` stops multiplying tier by the enemy-difficulty curve and
   becomes additive, with its own constant in `tuning.rs`.
2. `capture_chance`'s skill term becomes multiplicative rather than additive.
3. The one `balance_sim` gate test whose asserted intent this inverts is
   rewritten to the intent that now holds.
4. Docs that state the old formulas are corrected.

Not in scope: changing any sink (build costs, upgrade costs, market prices,
the Zone Portal cost ramp); passive processing, power regen, forage rates or
combat drops; adding a zone or boss term to `capture_chance` (considered and
rejected below); any save-format change.

## Design

### Node payout

`crates/engine/src/systems.rs`:

```rust
pub(crate) fn node_payout(tier: u32, zone: ZoneLevel) -> u32 {
    tier + NODE_PAYOUT_ZONE_BONUS * zone.0.saturating_sub(1)
}
```

`crates/engine/src/tuning.rs` gains `NODE_PAYOUT_ZONE_BONUS: u32 = 1`,
documented as deliberately additive and deliberately not `ZONE_STAT_GROWTH`:
enemy difficulty and node income were sharing one curve, and multiplying an
upgrade tier by an exponential depth term is what put a Mk5 node in zone 5 at
80 a cycle. Additive means tier and depth stop compounding each other.

Yield per completed cycle:

|  | Zone 1 | Zone 2 | Zone 3 | Zone 5 |
|---|---|---|---|---|
| Mk1 | 1 | 2 | 3 | 5 |
| Mk3 | 3 | 4 | 5 | 7 |
| Mk5 | 5 | 6 | 7 | 9 |
| *Mk5 today* | *5* | *10* | *20* | *80* |

Untouched by this: the banked-item path in `task_progress_system`
(`systems.rs:183`) still pays exactly 1 for anything declaring a `bank_limit`,
so Research Data is unaffected. `mining_success_chance` is unchanged, so
upgrading still buys reliability (50% at Mk1 to 90% at Mk5) on top of the
payout step.

Nothing persists a payout, so no save migration is needed. An existing
save's stockpile is not reduced; the change only slows what arrives from
here.

### Capture chance

`crates/engine/src/taming.rs`:

```rust
let base = item_potency
    * (CAPTURE_POTENCY_CEILING - hp_fraction * CAPTURE_HP_PENALTY)
    * (1.0 - taming_difficulty * CAPTURE_DIFFICULTY_PENALTY);
(base * (1.0 + decompiler_skill as f32 * DECOMPILER_SKILL_BONUS))
    .clamp(CAPTURE_CHANCE_MIN, CAPTURE_CHANCE_MAX)
```

One operator. No new constant: `DECOMPILER_SKILL_BONUS` (0.02) is
reinterpreted from "percentage points added per point of skill" to "percent
added per point of skill", and its doc comment in `tuning.rs` must say so.

Because skill now scales the base instead of sitting beside it,
`taming_difficulty` and HP-weakening stay inside the thing being multiplied
and keep mattering at every skill level. Chance against a fully-weakened
target at skill 40:

| Species (difficulty) | Today | After |
|---|---|---|
| Drone (0.15) | 95% | 59% |
| Trojan (0.5) | 95% | 45% |
| Overseer (0.9, boss) | 95% | 30% |
| Wintermute (0.95, boss) | 95% | 28% |

Skill 0 is identical to today. Early game is close but slightly harsher — at
skill 5 a weakened Drone is 36% rather than 43%. A boss would need skill
around 235 to reach the 0.95 clamp, so the clamp stops being the practical
outcome without ever becoming unreachable in principle.

`CAPTURE_CHANCE_MIN` still floors every attempt at 5%, so no target is ever
hopeless.

### Rejected: a zone or boss term on capture

Considered, and rejected for this change. It would answer the reported
symptom ("a boss in zone 2") most directly, but it stacks a second cut on top
of the first and risks over-correcting into un-tameable before the first cut
has been played. Making `taming_difficulty` matter again is the fix with the
better shape: it is data a modder already controls, and 0.15–0.95 is exactly
the spread the field was written to express. If depth should defend a capture
after playtesting, that is a separate change with its own numbers.

## Balance gate

`balance_sim.rs` covers node income and does not cover capture, so the gate
moves in one place only.

`an_unupgraded_base_gains_ground_from_its_very_first_breach` **still passes**
on the numbers — at Mk1 the additive curve (1, 2, 3) still outruns the
Zone Portal's linear cost ramp (1, 1.5, 2 — `ZONE_PORTAL_COST_GROWTH_PERCENT`),
giving 1600 → 1200 → 1067 ticks across zones 1–3. Its doc comment cites
"exponential payout" and `2^(zone-1)` and must be rewritten even though no
assertion changes.

`base_income_outpaces_portal_cost_across_every_zone` **inverts**. At Mk3,
funding time now grows with depth instead of falling: 381 → 457 → 500 ticks
at zones 1, 3 and 6. This is the intended progression change, not a
regression — it is the scarcity the current curve destroyed. The test is
replaced by one asserting the property that now holds and is worth pinning:
funding time may grow with depth, but converges rather than exploding. It
stays under 1.5× the zone-1 figure out to zone 6 (the limit as depth → ∞ is
about 571 ticks against 381 at zone 1). Deeper is a longer grind, never a
wall. Rename to match, e.g.
`deeper_breaches_cost_more_time_but_stay_bounded`.

`ticks_to_afford_portal`'s own doc comment (`balance_sim.rs:190`) says the
economy "keeps pace with the doubling curve" and needs the same correction.

## Testing

New:

- A direct shape test on `node_payout` asserting tier and depth no longer
  compound — `node_payout(5, ZoneLevel(3)) == 7`, plus the Mk1 and Mk5 rows
  of the table above.
- A `capture_chance` test asserting the easy/boss spread survives high skill:
  at skill 40 a `taming_difficulty` of 0.15 must beat one of 0.9 by a wide
  margin, where today both clamp to 0.95.

Existing, needs rewriting:

- `crates/engine/src/tests/taming.rs:236` explicitly relies on maxed
  Decompiler skill pinning `capture_chance` to its 0.95 clamp. That premise
  is gone.
- `balance_sim.rs` as described above.

The four unit tests in `taming.rs` (`weaker_prey_is_easier_to_tame`,
`harder_species_resist_taming`, `higher_decompiler_skill_improves_odds`,
`chance_is_always_within_bounds`) all assert relative orderings that the
multiplicative form preserves, and should pass untouched. If any fails, the
change is wrong.

No engine test asserts a specific gathered quantity — the `GatherResource`
setups in `tests/building.rs`, `tests/zone.rs`, `tests/party.rs`,
`tests/raids.rs` and `tests/support.rs` assert task survival and raid
behaviour, not payout size.

## Documentation

Three places state a formula this change falsifies:

- `assets/structures/README.md:46-58` — "multiplied by the current zone
  level's stat multiplier (doubling per zone — zone 1 pays x1, zone 2 x2,
  zone 3 x4) and again by the structure's upgrade tier". This is the modder-
  facing schema reference and must describe the additive curve instead.
- `README.md:61-63` — "multiplied by both your zone level and the structure's
  upgrade tier".
- `README.md:148` — "each tier multiplying payout".

## Verification

```sh
cargo test -p feral-processes-engine balance_sim   # the signal lives here
cargo test -p feral-processes-engine taming
cargo test --workspace
cargo clippy --workspace
cargo fmt
```

Neither change can be verified by arithmetic alone — both are pacing changes
whose whole point is how they feel over a session. This needs play after it
is green.
