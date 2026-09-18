# Design specs: what shipped, and where its argument is

**Audited 2026-09-02, re-audited 2026-09-06** against the source tree and
the release tags — not against the specs' own headers, which had lied for
weeks and had rotted again by the second pass. This file is the one-read
answer to "did this ship, and where is its argument".

## The invariant

**`archive/specs/` is implemented. `specs/` is not — with three named
exceptions.** Every archived spec shipped; the ones left in `specs/` are open,
parked, partial or superseded, and each says which in its own header. Sorting
the directory *is* the answer, so no sweep is needed next time.

**The three exceptions are built and stay in `specs/` anyway**, because source
doc comments pin their paths and moving them would edit `crates/`:
`2026-09-04-program-extraction-design` (seventeen `//!` and `///` citations),
`2026-09-04-dev-sprite-editor-design` (`sprite_forge.rs` and a seam
argument in the memory graph) and `2026-09-16-routine-research-tree-design`
(`routine_tree.rs`, `tuning.rs` and its tests).

**The debt this section recorded is paid.** There was a third —
`2026-09-06-settlement-growth-design`, one `//!` line in
`tests/settlement_growth.rs` — created because the citation was noticed
during the landing itself, and editing a source file mid-deploy is how
unreviewed changes ride into a release. On 2026-09-09 it was archived with
its citation moved, as its own change rather than inside a deploy, which is
what the rule asked for all along. The same pass archived
`2026-09-09-tactical-surface-battles-design` and moved its two citations, so
no fourth exception was created.

The archive habit worth keeping is archiving **on landing**, not by a later
sweep — `2026-09-02-base-instrumentation-design` and the two settlement specs
were done that way. The 2026-09-06 pass had to move six specs that had gone
built without moving, which is what the habit avoids.

The checks: a distinctive symbol from
each spec resolves in `crates/` or `assets/`, and the commit that added each
spec resolves to a release tag.

## What is open — the specs still in `specs/`

| Spec | State | Evidence |
| --- | --- | --- |
| `2026-09-04-program-extraction-design` | **built**, unplayed; path-pinned | all five phases, 4 (§10) in `v0.13.114`; `StructureDef::strips` and `Game::run_teardown_rigs` resolve in `crates/engine` |
| `2026-09-04-dev-sprite-editor-design` | **built**, unplayed; path-pinned | `crates/app-core/src/app/sprite_forge.rs` |
| `2026-09-16-player-emulation-design` | approved, **unbuilt**; no plan yet; after #101 | `Game::kit_of` and `components::Emulation` exist nowhere |
| `2026-09-16-routine-research-tree-design` | **built**; path-pinned | `v0.13.196`; `ResearchTree`, `DiscoveredRoutines` and `crates/engine/src/routine_tree.rs` resolve in `crates/engine` |
| `2026-08-31-stack-wanderers-design` | approved, **unbuilt** | `FrameWanderers` exists nowhere in `crates/` |
| `2026-08-24-departure-memories-design` | brainstorm parked | no departure memory in `assets/memories/` |
| `2026-08-24-stack-depth-compounding-design` | question posed, no shape chosen | measurement only |
| `2026-08-17-zones-as-difficulty-parked` | parked | no shape chosen |
| `2026-08-17-item-synergy-burnout-parked` | parked | nothing stacks yet |
| `2026-08-18-gear-passives-balance-measurement` | **not run** | a measurement protocol, never executed; the design it measures shipped in `v0.11.2` |
| `2026-08-19-combat-model-ac-and-weapon-damage-design` | **partial** | slices 1 and 2 shipped (slice 2 has its own archived spec); slices 3-4 deliberately deferred |
| `2026-08-13-creeping-base-footprint-design` | **superseded** | `build_radius_bonus` / `clear_platform` survive only in doc comments recording their retirement |
| `2026-08-22-collect-picker-design` | **superseded** | `collect_basket` absent; `Mode::Transfer` shipped instead |

**The settlement ladder is finished and has no open row.** The two settlement
specs shipped everything they scoped, and the three things they deferred *by
decision* have since resolved: town-sourced raids and hostile patrols were
claimed by `2026-09-06-town-raids-and-hostile-patrols-design` and both shipped
(`v0.13.113` and `v0.13.115`), and **a server growing into a mainframe is now
claimed by `2026-09-06-settlement-growth-design`** — the last settlement
deferral, shipped in `v0.13.117`. **Nothing on the settlements path is
unclaimed any more, and nothing on it is unbuilt.**

Program extraction is likewise finished: five phases, nothing unbuilt. What
both leave behind is tuning — thirteen guessed constants in the hostility
spec, seven in the aid spec, and a `FIGHT_CONDITION_WEIGHT` shipped at 0.0 in
extraction. None of them are answerable by any instrument in this repo.
`balance_sim` models no raids, no towns, no loot and no class.

`docs/content-gaps.md` holds built-but-unused engine mechanics, which is a
different question and not this file's job. **`TODO.md` no longer exists** —
it was deleted at `v0.12.0`; references to it here and in source comments are
historical, and git history is where its 62 lines live.

## Do not move these

Cited from source doc comments, so their paths are load-bearing. Two live in
`specs/` despite being built — `2026-09-04-program-extraction-design` and
`2026-09-04-dev-sprite-editor-design`, the invariant's two exceptions above.
The other eleven are already in `archive/specs/`:
`2026-07-31-the-stack`, `2026-08-03-nest-aggression`,
`2026-08-05-stack-movement-routines`, `2026-08-06-easter-eggs`,
`2026-08-09-battle-telemetry`, `2026-08-17-base-power-grid`,
`2026-08-19-base-out-of-phase`, `2026-08-27-paned-command-hud`,
`2026-09-01-character-creation`, `2026-09-06-settlement-growth-design` and
`2026-09-09-tactical-surface-battles-design`. Four more are cited from
`CHANGELOG.md`, a seam argument in the memory graph, or
`assets/nemesis/README.md`:
`2026-08-17-nemesis`,
`2026-08-19-windows-and-macos-distribution`, `2026-08-21-item-quality` and
`2026-08-23-rock-kinds-and-mining-mode`.

## The plans have been deleted three times

Forty-six were deleted on 2026-08-13; forty-six more accumulated and
forty-three of those were deleted on 2026-09-02; eighteen more accumulated and
all eighteen went on 2026-09-06, every one of them naming work that had
shipped or been superseded. A nineteenth was written the same day for
settlement growth and deleted at its landing, which is the convention working
as intended rather than a fourth purge: a plan lives exactly as long as the
work it directs. `plans/` is empty and the directory is gone until the next
plan is written. Eighteen more accumulated between 2026-09-07
and 2026-09-17 without being deleted at their landings, and all eighteen
went on 2026-09-18, each verified shipped against `crates/` and `assets/`. They are write-once scaffolding superseded by the
code they produced, nothing outside the directory cites one, and git history
holds them: `git log --diff-filter=D -- 'docs/superpowers/plans/*'` finds the
deletions and `git show <commit>^:<path>` reads any of them back.
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
| `2026-08-19-combat-model-ac-and-weapon-damage-design` | Attack rolls, percentage-point Mitigation, weapon damage ranges, crits and a fumble ladder — **slice 1 of four; slice 2 shipped separately, slices 3-4 deferred** | v0.12.0 |

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

## The specs archived on 2026-09-06

Six that had gone built without moving. "Release" is the earliest tag
containing the commit that added the spec, which for four of these is
`v0.13.100` — they were written on branches that merged there. Where the
implementation landed in a different release the row says so.

| Spec | What it designed | Release |
| --- | --- | --- |
| `2026-09-02-combat-model-slice-2-design` | A second swing for Strikers from level 8 | `v0.13.100` |
| `2026-09-03-player-icon-editor-design` | The player icon editor | `v0.13.100` (built `v0.13.87`) |
| `2026-09-03-sprites-from-the-defs-design` | A `sprite:` field on species and structures, and a directory scan | `v0.13.100` |
| `2026-09-04-perk-and-talent-respec-design` | Taking a perk or a talent back, for Credits | `v0.13.100` |
| `2026-09-05-compass-design` | A selected destination and a bearing to it | `v0.13.103` (distance `v0.13.104`) |
| `2026-09-06-town-raids-and-hostile-patrols-design` | Town-sourced raids (7a) and hostile patrols (7b) | `v0.13.113` (7b `v0.13.115`) |

`2026-08-24-rest-interruption-design` was archived earlier, on landing in
`v0.13.94`, but its row was left in the open table above until this pass —
which is the same rot from the other direction.

## The specs archived on 2026-09-09

Both were built and both are cited from source, so each moved **with its
citations** — which is the rule the invariant above states and the reason
neither became a standing exception.

| Spec | What it designed | Release |
| --- | --- | --- |
| `2026-09-06-settlement-growth-design` | A server growing into a mainframe | `v0.13.117` |
| `2026-09-09-tactical-surface-battles-design` | Opt-in tactical fights on a battle map, nine phases | `v0.13.139` (the tactical arena, phase 9, `v0.13.140`) |

## The spec archived on 2026-09-10

Archived **on landing**, which is the habit this file recommends — nothing in
`crates/` or `assets/` cites its path, so it moved on its own.

The release column is left to the merge on purpose: the version is decided
there (one release per change landing on `main`), and a tag written on a
branch is a claim that a rebase or a competing merge can invalidate.

| Spec | What it designed | Evidence |
| --- | --- | --- |
| `2026-09-10-invisibility-routine-design` | A body no picker may name until it acts | `components::Cloaked`, `Game::break_cloak` and `AbilityEffect::Cloak` resolve in `crates/engine`; `assets/abilities/detach.ron` |
| `2026-09-10-weapon-reach-design` | A weapon whose swing lands on more than one body | `items::WeaponReach`, `Game::swing_reach` and `components::ReachCharge` resolve in `crates/engine`; `assets/items/scatter_lance.ron`, `assets/items/broadcast_storm.ron`. Its one `///` citation moved with it, on the branch — no fourth exception |

## The spec archived on 2026-09-15

Archived **on landing** (of the last plan task, on `feat/ai-tamper-routines`,
ahead of the branch's own merge) — nothing in `crates/` or `assets/` cites
its path, so it moved on its own.

| Spec | What it designed | Evidence |
| --- | --- | --- |
| `2026-09-15-ai-tamper-routines-design` | Five battle-map routines that tamper with the hostile AI's decision-maker (temperature, forecast, side, sight), plus flavour routines, gear and a research node | `AbilityEffect::Tamper`, `TamperKind`, `components::Tampered`, `Game::decision_temperature`, `Game::acts_for_hostiles`, `Game::taken_over`, `Decoy`/`Game::sees_decoy`/`tactical_strike_decoy`, `Game::tactical_forecast` all resolve in `crates/engine`; the tamper block and decoy drawing resolve in `crates/gui`; `assets/research/model_inspection.ron` and eleven routine `.ron` files in `assets/abilities/`. Not yet played at the keyboard — see the plan's closing note |
| `2026-09-16-tactical-cover-design` | A boulder on the attacker's side of a defender makes them harder to hit at range, and walking round it is flanking for free | `reach::cover_between`, `Game::defender_profile_against`, `Swing::cover_ignored`, `AbilityShape::ignores_cover`, `TACTICAL_AI_COVER_WEIGHT`/`_TIEBREAK`, `TacticalBody::in_cover`, `TacticalView::covered` and `Game::body_in_cover` all resolve in `crates/engine`; the shield mark and the covered-destination wash resolve in `crates/gui/src/render/tactical.rs`. No save change and no new cell kind. The board density that makes it reachable is a census, `cover_is_reachable_on_every_biome_a_fight_opens_on`, not a `docs/measurements/` file. Not yet played at the keyboard |
| `2026-09-16-tactical-squads-design` | Five wild programs of one species fold into one 2x2 body with a combined stat block, two actions a turn and a `^` mark, on battle maps alone | `tuning::FORMATIONS`, `tactical::squads::plan`, `components::Squad`, `Game::spawn_squad`/`disband_squad`/`decompile_squad`, `TacticalBattle::cells_of`/`footprint_of`/`set_shape`, `reach::gap` and `actions_left` all resolve in `crates/engine`; the footprint draw, the bottom-right squad mark and the footprint-centred camera resolve in `crates/gui`. No save change — a squad carries no world `Position` and no `Tamed`, the omission that keeps it out of `save.rs`, asserted by a real save/load test. `docs/measurements/2026-09-17-tactical-squads.md` is the arena pair, re-run after a review found the squad's movement and reach both defective; it supersedes a contaminated first run. `swing_share` stays at 0.5, below the parity its own doc describes, deliberately and with the evidence recorded. Not yet played at the keyboard |

## The specs archived on 2026-09-18

Fourteen built specs that had stayed in `specs/` past their landings — the
habit above lapsed for a fortnight. Each was checked by resolving its symbols
in the tree. One path citation moved with them: `tests/racks.rs`' `//!` line,
so no fourth exception. The quarantine rack's own `**Status:**` header still
said "not implemented" and was corrected in the move.

| Spec | What it designed | Evidence |
| --- | --- | --- |
| `2026-09-07-tamed-program-build-requirement-design` | A build that runs a job costs a tamed program | `StructureDef::costs_no_program`, `Mode::BuildProgram` |
| `2026-09-08-program-build-quality-design` | A structure remembers how well it was built | `components::BuildQuality`, `BUILD_QUALITY_PER_RARITY_RUNG` |
| `2026-09-08-research-materials-design` | A research node's material bill | `ResearchDef::materials` |
| `2026-09-08-staff-tantrums-design` | A disgruntled program acts out | `resources::Brawls`, `EffectKind::Brawl` |
| `2026-09-10-power-cell-ladder-design` | Power cells above the first | `assets/items/buffered_cell.ron`, `sustain_cell.ron`; `v0.13.147` |
| `2026-09-10-quarantine-rack-design` | A rack that holds downed programs | `structures::RackDef`, `components::Racked`, `assets/structures/quarantine_rack.ron` |
| `2026-09-10-research-graph-view-design` | The research tree as a flow chart | `views::ResearchGraph`, `Game::research_graph`, `render/research_graph.rs` |
| `2026-09-11-research-as-a-project-design` | Research is one project at a time, paid by work orders | `resources::ActiveResearch`, `WorkOrder::for_research` |
| `2026-09-12-battle-summons-design` | A routine that fields a body mid-fight | `components::Summoned`, `assets/abilities/fork_program.ron`; `v0.13.173` |
| `2026-09-12-weapon-range-and-bolts-design` | Ranged weapons and the bolt cue | `ItemDef::range`, `BoltCue`, `TACTICAL_RANGED_MOVE_RANGE`; `v0.13.172` |
| `2026-09-16-auto-resolve-combat-design` | `[R]` plays the rest of a fight out | `game/auto_resolve.rs`, `AUTO_RESOLVE_ROUND_CAP` |
| `2026-09-16-floor-finishes-design` | A finish laid over base floor | `crates/engine/src/floors.rs` (`FloorDb`, `FloorShade`), `assets/floors/` |
| `2026-09-16-handles-and-memory-schema-design` | A program's handle, and the memory schema under mood | `handles::of`, `memories::mood` |
| `2026-09-16-tactical-reactions-design` | A reaction swing when a body leaves reach | `Game::provoke`, `battle::resolve_free_attack` |

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

The specs above, and `docs/content-gaps.md` for engine mechanics that are
built and tested with no asset using them. `TODO.md` was deleted at `v0.12.0`
and is not a backlog any more.
