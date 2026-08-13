# Design specs: what shipped, and where its argument is

Every spec under `archive/specs/` is **implemented**. Audited 2026-08-13 by
checking each one against `CHANGELOG.md` and the source, not against its own
header — see the warning below. This file exists so that audit is one read
next time instead of a sixty-file sweep.

## Two things to know before reading a spec

**A spec's `**Status:**` line is stale.** It records what was true when the
design was approved and was almost never revised afterwards. Fourteen specs
still say "approved, not implemented" or "designed, not implemented" for
features that shipped and released — including `depots-and-hauling`,
`battle-arena`, `enemy-battle-policy`, `stack-descriptions` and
`companion-equipment`. Only two specs ever had the header corrected. The
`## Status` phase table in `2026-07-31-the-stack-design.md` is stale the same
way: it lists the derelict trader and the crash log as "sketched only", and
both shipped. Answer implementation questions from `CHANGELOG.md` and a grep,
never from the header.

**The plans are gone.** Forty-six implementation plans (1.9M, ~44,000
lines) were deleted on 2026-08-13; the forty-seventh file in that directory
was not a plan and moved to `reports/` (see the footnote on
`2026-07-21-visual-effects`). They were write-once scaffolding fully
superseded by the code they produced, nothing outside the directory cited
one, and git history holds them: `git log --diff-filter=D --
'docs/superpowers/plans/*'` finds the deletion, and `git show <commit>^:<path>`
reads any of them back. `CLAUDE.md`'s **Process weight** section is the
lesson that motivated it.

## Do not move these five again

Five specs are cited from source doc comments as the standing rationale for a
seam, so their paths are load-bearing:
`2026-07-31-the-stack` (`tuning.rs`, `game/trace.rs`),
`2026-08-03-nest-aggression` (`game/turn.rs`, `tests/zone.rs`),
`2026-08-05-stack-movement-routines` (`game/stack_movement.rs`),
`2026-08-06-easter-eggs` (`game/listen.rs`, `game/throw.rs`, `game/taunt.rs`,
`crates/engine/EASTER_EGGS.md`) and
`2026-08-09-battle-telemetry` (`telemetry.rs`, `crates/app-core/Cargo.toml`).

## The specs

"Release" is the earliest tag containing the commit that added the spec,
which is the release its branch landed in. It is exact from `v0.3.1`
onward. Everything at `v0.2.0` or `v0.3.0` predates the one-release-per-change
policy — those two tags are batch releases, so the column only says "before
the policy", not which change shipped it.

| Spec | What it designed | Release |
|---|---|---|
| `2026-07-21-inventory-capacity` | Inventory capacity | v0.2.0 |
| `2026-07-21-research-tree` | Research Tree | v0.2.0 |
| `2026-07-21-visual-effects` | Damage and shield visual effects | v0.2.0 |
| `2026-07-22-gui-font-and-text-layer` | GUI font and text layer | v0.2.0 |
| `2026-07-22-moddable-items` | Data-driven, moddable items (Phase 1) | v0.2.0 |
| `2026-07-22-recharger-power-regen` | Recharger Node power regeneration; Home as the rest gate | v0.2.0 |
| `2026-07-23-non-raidable-structures-and-slot-labels` | Non-raidable structures and inventory slot labels | v0.2.0 |
| `2026-07-23-soften-raids` | Soften raids | v0.2.0 |
| `2026-07-24-bards-tale-battle-ledger` | The Bard's Tale battle ledger | v0.3.0 |
| `2026-07-24-battle-flow-and-base-radius` | Battle flow and base radius | v0.3.0 |
| `2026-07-24-delete-the-tui` | Delete the TUI | v0.3.0 |
| `2026-07-24-party-roster-battles` | Party Roster Battles | v0.2.0 |
| `2026-07-24-sell-programs-to-a-trader` | Selling programs to a trader | v0.3.0 |
| `2026-07-24-travelling-base` | Travelling Base | v0.2.0 |
| `2026-07-25-abilities` | Abilities: data-driven multi-target combat actions | v0.3.0 |
| `2026-07-25-swarm-groups` | Swarm groups: enemy groups scale to 100 | v0.3.0 |
| `2026-07-25-zone-currency-reset` | Zone Currency Reset | v0.3.0 |
| `2026-07-26-player-ability-unlocks` | Player ability unlocks: research-granted routines | v0.3.0 |
| `2026-07-27-ability-routines` | Ability routines: extractable, slot-limited abilities | v0.3.0 |
| `2026-07-27-battle-log-reveal` | Paced battle narration and a results-only handoff | v0.3.0 |
| `2026-07-27-manifest-screen` | Manifest screen — full stat sheet for the player and any program | v0.3.0 |
| `2026-07-27-random-encounters-and-jack-out` | Random encounters and a fallible jack-out | v0.3.0 |
| `2026-07-28-node-payout-and-capture-rate` | Node payout and capture rate rebalance | v0.3.0 |
| `2026-07-28-program-permadeath` | Program permadeath | v0.3.0 |
| `2026-07-28-trader-buyback` | Trader buyback | v0.3.0 |
| `2026-07-28-trader-credits` | Trader Credits | v0.3.0 |
| `2026-07-28-wild-carried-routines` | Wild-carried routines, hostile specials, and level-scaled abilities | v0.3.0 |
| `2026-07-29-ability-affinities` | Ability affinities | v0.3.0 |
| `2026-07-30-condensed-message-log` | Condensed message log | v0.3.0 |
| `2026-07-30-field-routines` | Field routines | v0.3.0 |
| `2026-07-30-log-and-structure-screens` | Two read-only screens: message history and the structure roster | v0.3.0 |
| `2026-07-31-quick-trade-and-item-grouping` | Quick trade keys and item grouping | v0.3.0 |
| `2026-07-31-the-stack` | The Stack | v0.3.0 |
| `2026-08-02-bounded-income` | Bounded income: rest costs a consumable, scan is deleted | v0.3.0 |
| `2026-08-03-nest-aggression` | Nest aggression and the nest cache | v0.3.0 |
| `2026-08-03-production-chains` | Adjacency-fed production chains | v0.3.0 |
| `2026-08-04-achievements` | Achievements | v0.3.0 |
| `2026-08-04-menu-consolidation` | Menu consolidation | v0.3.0 |
| `2026-08-04-routine-disks` | Routine Disks | v0.3.0 |
| `2026-08-05-banked-resources` | Banked resources: research stops being a thing you carry | v0.3.0 |
| `2026-08-05-fusion-colour-and-gear-cap` | Fusion colour in menus, and a 3-fuse ceiling on gear | v0.3.0 |
| `2026-08-05-stack-movement-routines` | Stack movement routines | v0.3.0 |
| `2026-08-05-zone-gated-upgrades-and-gc-entropy` | Zone-gated upgrade tiers, and renaming raids to GC Entropy Sweeps | v0.3.0 |
| `2026-08-06-depots-and-hauling` | Depots and hauling programs | v0.3.0 |
| `2026-08-06-easter-eggs` | Three more hidden keys | v0.3.1 |
| `2026-08-06-wielded-program` | Wielding a program as your weapon | v0.3.0 |
| `2026-08-07-battle-arena` | The battle arena | v0.4.1 |
| `2026-08-07-per-copy-item-fusion` | Per-copy item fusion | v0.4.0 |
| `2026-08-08-arena-rolled-encounters` | Arena: rolled encounters | v0.5.1 |
| `2026-08-08-interactive-arena` | The interactive arena | v0.5.0 |
| `2026-08-09-battle-telemetry` | Battle telemetry for dev builds | v0.5.15 |
| `2026-08-09-enemy-battle-policy` | Learned enemy battle policy | v0.5.12 |
| `2026-08-10-shiny-variants` | Shiny variants: Optimized and Overclocked programs | v0.6.0 |
| `2026-08-10-species-classes` | Species classes: role as an axis independent of tier | v0.6.0 |
| `2026-08-10-stack-descriptions` | Generated flavour prose for the Stack | v0.5.23 |
| `2026-08-11-companion-refactoring` | Companion refactoring — permanent upgrades for tamed programs | v0.7.0 |
| `2026-08-11-haul-routing-and-direct-demolish` | Hauler routing, stranded reporting, and direct demolish | v0.7.5 |
| `2026-08-12-companion-equipment` | Companion equipment | v0.8.0 |
| `2026-08-12-exclusive-routines` | Disk-first routines and the exclusive pool | v0.8.7 |

## Four rows that need a footnote

- **`2026-07-21-inventory-capacity`** — built, then *deliberately reverted*.
  `BASE_INVENTORY_CAPACITY` is absent from the tree because commit `5b38c32`
  made the buffer unbounded again and repurposed the Data Cache to grant pet
  slots. Absent code here means removed, not never-built.
- **`2026-07-31-the-stack`** — five phases plus two later inhabitants, spread
  across many releases. `v0.3.0` is only phase 1, the rename. The ephemeral
  Stack market (`game/stack_market.rs`, `0.8.7`) is the "derelict trader" the
  spec defers, and the crash log was absorbed into `descriptions.rs` and is
  read by `Z` / `Game::listen`.
- **`2026-08-10-species-classes`** — eight phases; `v0.6.0` is the first.
- **`2026-07-21-visual-effects`** — the spec shipped (`resources::EffectQueue`,
  `crates/gui/src/fx.rs`). What is *not* built is the sprite tileset, which was
  never part of this spec: it is the open finding in
  `reports/2026-07-27-renderer-graphics-assessment.md`, and the blocker is
  112+ hand-drawn tiles rather than code. That file sat in `plans/` and was
  kept out of the deletion — it is a costed assessment with a live finding
  and a lesson worth re-reading, not scaffolding for a change that shipped.

## What is actually open

Not here. `TODO.md` holds the gameplay backlog, and `docs/content-gaps.md`
holds engine mechanics that are built and tested with no asset using them.
