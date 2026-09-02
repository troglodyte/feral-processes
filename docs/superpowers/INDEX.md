# Design specs: what shipped, and where its argument is

**Audited 2026-09-02** against the source tree and the release tags — not
against the specs' own headers, which had lied for weeks. This file is the
one-read answer to "did this ship, and where is its argument".

## The invariant

**`archive/specs/` is implemented. `specs/` is not.** Ninety-five specs are
archived and every one of them shipped; the eleven left in `specs/` are open,
parked, partial or superseded, and each says which in its own header. Sorting
the directory *is* the answer, so no sweep is needed next time.

Two independent checks agree on the ninety-five: a distinctive symbol from
each spec resolves in `crates/` or `assets/`, and the commit that added each
spec resolves to a release tag.

## What is open — the eleven in `specs/`

| Spec | State | Evidence |
| --- | --- | --- |
| `2026-09-02-combat-model-slice-2-design` | **built**, unplayed | a second swing for Strikers from level 8; `balance_sim` cannot gate it |
| `2026-08-31-stack-wanderers-design` | approved, **unbuilt** | `FrameWanderers` exists nowhere in `crates/` |
| `2026-08-24-rest-interruption-design` | never approved, **unbuilt** | `Game::rest_interrupted` does not exist |
| `2026-08-24-departure-memories-design` | brainstorm parked | no departure memory in `assets/memories/` |
| `2026-08-24-stack-depth-compounding-design` | question posed, no shape chosen | measurement only |
| `2026-08-17-zones-as-difficulty-parked` | parked | no shape chosen |
| `2026-08-17-item-synergy-burnout-parked` | parked | nothing stacks yet |
| `2026-08-18-gear-passives-balance-measurement` | **not run** | a measurement protocol, never executed; the design it measures shipped in `v0.11.2` |
| `2026-08-19-combat-model-ac-and-weapon-damage-design` | **partial** | slice 1 shipped; slices 2-4 deliberately deferred |
| `2026-08-13-creeping-base-footprint-design` | **superseded** | `build_radius_bonus` / `clear_platform` survive only in doc comments recording their retirement |
| `2026-08-22-collect-picker-design` | **superseded** | `collect_basket` absent; `Mode::Transfer` shipped instead |

`docs/content-gaps.md` holds built-but-unused engine mechanics, which is a
different question and not this file's job. **`TODO.md` no longer exists** —
it was deleted at `v0.12.0`; references to it here and in source comments are
historical, and git history is where its 62 lines live.

## Do not move these nine

Cited from source doc comments, so their paths are load-bearing:
`2026-07-31-the-stack`, `2026-08-03-nest-aggression`,
`2026-08-05-stack-movement-routines`, `2026-08-06-easter-eggs`,
`2026-08-09-battle-telemetry`, `2026-08-17-base-power-grid`,
`2026-08-19-base-out-of-phase`, `2026-08-27-paned-command-hud` and
`2026-09-01-character-creation`. Four more are cited from `CHANGELOG.md`,
`docs/seams.md` or `assets/nemesis/README.md`: `2026-08-17-nemesis`,
`2026-08-19-windows-and-macos-distribution`, `2026-08-21-item-quality` and
`2026-08-23-rock-kinds-and-mining-mode`.

## The plans have been deleted twice

Forty-six were deleted on 2026-08-13; forty-six more accumulated and
forty-three of those were deleted on 2026-09-02, leaving only the plans for
work that has not shipped. They are write-once scaffolding superseded by the
code they produced, nothing outside the directory cites one, and git history
holds them: `git log --diff-filter=D -- 'docs/superpowers/plans/*'` finds the
deletion and `git show <commit>^:<path>` reads any of them back.
`CLAUDE.md`'s **Process weight** section is the lesson that motivated it.

The forty-seventh file in the first batch was not a plan and moved to
`reports/` — see the footnote on `2026-07-21-visual-effects`.

## The specs

The fifty-nine archived on 2026-08-13. The gap this paragraph used to
record — `nemesis` and five siblings shipped but sitting outside the
table — was closed by the 2026-09-02 audit; they are in the second table
below.

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
| `2026-08-17-nemesis` | Nemesis: a lost fight gets a name, a rising grudge and a mark on the map | v0.9.3 |
| `2026-08-19-companion-progression-design` | Companion rings, levels past the cap, and per-class talent trees | v0.11.9 |
| `2026-08-19-combat-model-ac-and-weapon-damage-design` | Attack rolls, percentage-point Mitigation, weapon damage ranges, crits and a fumble ladder — **slice 1 of four; slices 2-4 deferred** | v0.12.0 |

## The specs archived on 2026-09-02

The thirty-six that shipped between `v0.8.14` and `v0.13.81`. "Release" is
the earliest tag containing the commit that added the spec.

| Spec | What it designed | Release |
| --- | --- | --- |
| `2026-08-13-research-cost-and-zone-gate-design` | Research cost and zone gate | `v0.8.15` |
| `2026-08-13-sector-traits-design` | Sector Traits | `v0.8.14` |
| `2026-08-14-contracts-design` | Contracts | `v0.8.29` |
| `2026-08-14-work-orders-design` | Work Orders | `v0.8.36` |
| `2026-08-17-base-power-grid-design` | The base power grid | `v0.9.4` |
| `2026-08-17-nemesis-design` | Nemesis | `v0.9.3` |
| `2026-08-17-power-replaces-fatigue-design` | Power replaces Fatigue | `v0.9.4` |
| `2026-08-18-gear-passives-and-overclock-design` | Gear passives, and the Overclock axis they will carry | `v0.11.2` |
| `2026-08-18-species-danger-window-design` | The species danger window, and boss as a rolled variant | `v0.11.3` |
| `2026-08-19-base-out-of-phase-design` | The base, out of phase | `v0.13.0` |
| `2026-08-19-companion-progression-design` | Companion progression: rings, levels past the cap, and talent trees | `v0.11.9` |
| `2026-08-19-environment-effects-design` | Environment effects, phase 1: ground that does something | `v0.11.8` |
| `2026-08-19-windows-and-macos-distribution-design` | Windows and macOS distribution | `v0.13.14` |
| `2026-08-20-in-game-help-design` | In-game help | `v0.13.2` |
| `2026-08-21-entity-memories-design` | Entity memories | `v0.13.7` |
| `2026-08-21-item-quality-design` | Item quality | `v0.13.3` |
| `2026-08-21-work-order-queue-design` | The work order queue | `v0.13.5` |
| `2026-08-23-depot-deposit-design` | Putting items into a Depot | `v0.13.12` |
| `2026-08-23-morale-at-work-design` | Morale at work | `v0.13.17` |
| `2026-08-23-rock-kinds-and-mining-mode-design` | Rock kinds, a swing floor, and a mining toggle | `v0.13.15` |
| `2026-08-24-gear-affix-stacking-design` | Gear fusion across quality and affixes | `v0.13.20` |
| `2026-08-24-periodic-caravan-traders-design` | Periodic caravan traders | `v0.13.20` |
| `2026-08-25-merged-transfer-screen-design` | Merged transfer screen | `v0.13.23` |
| `2026-08-25-trade-screen-power-and-basket-design` | Item power, wagon grouping, and the caravan basket | `v0.13.27` |
| `2026-08-27-downed-programs-and-the-repair-bay-design` | Downed programs and the Repair Bay | `v0.13.36` |
| `2026-08-27-paned-command-hud-design` | The Paned Command HUD | `v0.13.37` |
| `2026-08-27-program-needs-design` | What a program needs | `v0.13.35` |
| `2026-08-27-upgrade-build-requests-design` | Upgrading as a build request | `v0.13.34` |
| `2026-08-27-zone-level-cap-design` | The zone level cap | `v0.13.36` |
| `2026-08-28-sorties-design` | Sorties | `v0.13.47` |
| `2026-08-29-notifications-design` | Full-screen notifications | `v0.13.53` |
| `2026-08-30-tutorial-contract-chain-design` | The tutorial contract chain | `v0.13.57` |
| `2026-08-31-static-weather-design` | Static: weather, and the environment comes home to Rust | `v0.13.59` |
| `2026-09-01-character-creation-design` | Character creation | `v0.13.75` |
| `2026-09-01-player-classes-design` | Player-only classes | `v0.13.81` |
| `2026-09-02-base-as-the-price-of-progress-design` | A working base is the price of progress | `v0.13.80` |

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

The eleven specs above, and `docs/content-gaps.md` for engine mechanics that are
built and tested with no asset using them. `TODO.md` was deleted at `v0.12.0`
and is not a backlog any more.
