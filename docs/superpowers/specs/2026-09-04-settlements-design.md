# Settlements: a persistent world with mainframes and servers

## Context

Two things are being asked for, and one is a prerequisite for the other.

**The world resets.** `Game::enter_next_zone` (`crates/engine/src/game/zone.rs:330`)
despawns every hostile, nest and Stack entrance, generates a fresh `WorldMap`
from a new seed, replaces the resource outright, teleports the party and moves
the base anchor. There is no persistent per-zone state and no way back. A
brainstorm on 2026-08-17 parked the question of what to do about this
(`docs/superpowers/specs/2026-08-17-zones-as-difficulty-parked.md`); the recorded
complaint was motivational, not tuning — zones are interchangeable and breaching
is "an endless grind for no reason but to advance."

**There is nowhere to go.** The engine has no faction, no reputation, no NPC and
no named place other than a Stack entrance. A survey confirmed this from scratch:
the word "settlement" appears once in the entire engine, in a comment.

This plan makes the world persistent, then puts settlements on it — cities
(*mainframes*) and towns (*servers*) that trade, issue work, hold an opinion of
you, and anchor caravan routes back to your base. The zone rework is
infrastructure; the settlements are the content that justifies keeping a map you
can read.

## Decisions taken

Settled in brainstorming; recorded so they are not relitigated.

| Question | Decision |
|---|---|
| Zone breach | Raises a global tier. The map is **never** regenerated or wiped. |
| Where the tier lands | **Uniform** — everything newly spawned anywhere is at the new tier. |
| `assets/sectors/` | **Retired.** One map, one palette, one noise field per run. |
| Stack entrances | **Persist and re-tier**; collapsed ones stay collapsed. |
| The currency burn | **Deleted.** Nothing is destroyed on breach. |
| Biome name collision | `Biome::Mainframe` → `Biome::Backplane`, freeing *mainframe* for cities. |
| Settlement placement | Derived from the world seed per region; materialized as you cover ground. |
| Settlement identity | A module on the **perks precedent** — thin `.ron` catalogue for prose, behaviour in Rust. |
| Settlement interaction | A glyph on the map. You do **not** enter a sub-map. |
| Relations | Own module. Movers: contracts, trade volume, clearing nearby threats, gifts. |
| Hostility (now) | Refuse service; prey on routes. |
| Hostility (later) | Town-sourced raids; hostile patrols. **Structure the code for these now.** |
| City vs town | Mechanical scale **plus** a specialty (gear / materials / routines / programs). |
| Existing systems | **Coexist.** Your Broker still boards its own contracts; the roaming wagon still visits. |
| Routes | Both one-off dispatch **and** standing routes. |

Deferred by decision: settlement "aid" rewards (garrisons, gifted programs,
relay fast-travel), settlements growing from server to mainframe.

## Why the fragment burn can go

Portal Fragments drop only from bosses killed underground — that is the game's
single source. The burn existed so a stockpile could not fund breaches past
content it never engaged with, but the fight gate is already upstream of the
stockpile. Deleting the burn removes a rule, not a brake.

## What already exists

Surveyed 2026-09-04 against the source. **Do not re-derive this** — it cost a
full exploration pass. Verify a line before relying on it, but start here.

**Contracts.** `crates/engine/src/contracts.rs` (data) + `game/contracts.rs`
(the `&mut Game` doors). `Objective` (`contracts.rs:131`) is
`Terminate`/`Deliver`/`Descend`/`Breach`/`Build`/`Hold`/`Perform{Deed}`;
`Reward` (`contracts.rs:264`) is `Credits`/`Item`/`Xp` — no fragment variant, on
purpose. `ContractTemplate` (`contracts.rs:363`) is the rollable form; rolled ids
are `template_id#slug`. There is **no Broker struct** — a Broker is any
`Structure` whose `StructureDef::issues_contracts` (`structures.rs:349`) is true,
and nothing but the player's own build can issue a contract today. `BrokerReach`
is `NoBroker`/`OffBase`/`AtBroker` (`game/contracts.rs:376`), door
`Game::broker_reach` (`:809`). The board is derived, never stored —
`Game::board_defs` (`:451`) off a local `StdRng` seeded from world seed + sector
+ epoch (`board_seed`, `:495`). Four objective kinds are polled by
`contract_system` (`:46`); `Deliver` advances only via
`Game::deliver_to_contract` (`:1032`). `Game::settle_contracts` (`:200`) drains
into `Game::complete_contract` (`:220`).

**Caravan / trade.** `game/caravan.rs` (1467 lines) + `caravans.rs` (data).
A caravan is an entity with `components::Caravan` and `CaravanStage`
(`components.rs:2074`): `Approaching` (zone surface) → `Docking` → `Crossing`
(base space) → `Docked` → `Leaving`. Shelf derived per visit from
`BaseGrid::seed()` + `CARAVAN_SALT` (`Game::visit_seed`, `caravan.rs:70`); only
the journey is saved (`CaravanSave`, `save.rs:722`). Doors: `caravan_view`
(`:1032`), `caravan_shelf` (`:232`), `commit_caravan_basket` (`:1388`). Buyback
is a **separate** mechanism for stationary structures, keyed by
`ShelfKey = (StructureKind, (x,y))` (`game/trade.rs:101`). `TradeCurrency` is an
`EconomyRole` (`items_db.rs:16`) held by `credits`, distinct from `Currency`
(`core_fragment`).

**Sorties.** `sorties.rs` + `game/sortie.rs` (744 lines). Dispatch structure is
any `StructureDef::dispatches_sorties` (`structures.rs:362`). Board derived like
contracts (`sortie_board`, `:128`). One accept door `dispatch_sortie` (`:212`);
`step_sortie` (`:394`) ticks and calls `return_sortie` (`:606`) at completion;
members return to `Staff` **by omission**. `SortieSave` (`save.rs:220`) stores
the resolved def; membership rides `CreatureSave::sortie_index`.
**No UI exists** — no `Mode`, no key, no screen calls any of it.

**World map.** `world::WorldMap` (`world.rs:197`) — unbounded, two-tier,
`CHUNK_SIZE = 32` (`world.rs:7`), lazily classified per chunk by `classify`
(`:242`), plus sparse `overrides` for player changes. Only seed + overrides
persist. `Biome` (`world.rs:24`) derives `Serialize`/`Deserialize`, and
`world.rs:27` documents a prior variant rename kept loading by a serde alias at
no version cost — the precedent for the `Backplane` rename.
`SectorDef`/`SectorDb` (`sectors.rs:131`) only reskin noise thresholds and
palette; no content placement lives in a sector file. Zone fixtures are
`SurfaceLink` (`components.rs:2158`, just `Position` + `Glyph`) scattered by
`Game::spawn_surface_links` (`game/stack.rs:162`), plus the base anchor. Wild
population: `Game::ensure_local_population` (`game/spawning.rs:819`) stocks
chunks within `POPULATION_CHUNK_MARGIN` (`tuning.rs:1152`) not already in
`resources::PopulatedChunks` (`resources.rs:1541`, a `BTreeSet<(i32,i32)>`,
saved wholesale, reset at `zone.rs:447`).

**`StructureDef`** (`structures.rs:225`) is base-content-only — worker slots,
power, upkeep, trade counters, raid durability. It has **no position, footprint
or ownership field**, so it cannot describe a building inside someone else's
town. A settlement is a parallel concept, not a `StructureDef`.

**Save.** `SAVE_FORMAT_VERSION = 32` (`save.rs:1225`), version-history changelog
in the doc comments directly above it. Payload is field-named RON since v29, so
a field **added** behind `#[serde(default)]` costs no bump; only a field
**removed**, or one whose meaning changes under a name it keeps, earns one. The
convention for a new subsystem (`caravans`, `buyback_shelves`, `base_grid`): a
defaulted field on `SaveData` (`save.rs:883` onward), a `*Save` struct storing
the **whole resolved def**, drained and reassembled in `game/lifecycle.rs`.

**Absent, confirmed by grep**: faction, reputation, standing-as-a-noun, NPC,
diplomacy, settlement, town, city. Every "standing" hit is the verb, a standing
order, or `Objective::standing` (a deployed-structures list). Every "ally" hit is
combat targeting. The word "settlement" occurs once, in a `tuning.rs` comment.

## Phase 1 — The persistent world

Playable on its own. Nothing else here works without it.

**`crates/engine/src/game/zone.rs`** — `enter_next_zone` shrinks to roughly
"increment `ZoneLevel`, notify, reset `PopulatedChunks`". Remove: the
`Hostile`/`Nest`/`SurfaceLink` despawn sweep, `map_for_zone` + the reseed +
`insert_resource(new_map)`, `find_walkable_start` + `ZoneSpawnPoint` + the party
teleport, `move_anchor_to`, the `BuybackLedger` reset, the caravan despawn +
`CaravanMemory` reset, the economy-role currency wipe.

Two things that look like wipe code but are the mechanism and must stay:

- **The `PopulatedChunks` reset** (`zone.rs:447`). `PopulatedChunks` is a
  `BTreeSet<(i32,i32)>` of chunks already stocked; clearing it makes
  `Game::ensure_local_population` (`crates/engine/src/game/spawning.rs:819`)
  re-stock every nearby chunk at the new tier. This one line is what makes
  "the world hardens around you" visible.
- **The frame-map cache clear.** Its original reason (a fresh link landing on a
  matching tile drawing the last sector's corridors) is gone, but uncollapsed
  links now *re-tier in place at the same tile*, so `StackMemory` and the cached
  frame map are stale for a frame that no longer exists. Same clear, new reason
   — update the doc comment or the next reader deletes it.

**Re-tiering Stack entrances.** Existing uncollapsed `SurfaceLink` entities stay
put; the frame generated behind one is regenerated at the current `ZoneLevel`.
`stack::generate` is already pure in `FrameSpec`, so this is a change to what
seeds a `FrameSpec`, not to generation.

**Retire sectors.** Delete `assets/sectors/`, `crates/engine/src/sectors.rs`
(`SectorDef`, `SectorDb`, `ShapeDelta`, `SectorPalette`, `map_for_zone`) and
every caller. `WorldMap::classify` reverts to its own thresholds — the doc
comment at `world.rs:179` records what zone 1's were, which is the target
behaviour. The breach line loses its sector name; settlements replace that
identity in Phase 2.

**Rename the biome.** `Biome::Mainframe` → `Biome::Backplane` with
`#[serde(alias = "Mainframe")]`. `world.rs:27` documents the identical earlier
rename (`StaticField`) and records that the alias is why it cost no
`SAVE_FORMAT_VERSION` bump. Copy that pattern exactly, including the comment.

**Save format.** Additive changes are free (field-named RON since v29). Check
specifically for *removed* fields — a removal is what earns a bump, and
`SAVE_FORMAT_VERSION` is currently 32 (`crates/engine/src/save.rs:1225`).
`ZoneSpawnPoint` and anything sector-derived are the candidates.

**Gates.** `cargo test --workspace`; then
`cargo test -p feral-processes-engine balance_sim` — `Game::level_cap` stays
zone-derived so pacing should hold, but a moved curve is the signal.

## Phase 2 — Settlements exist

**New module `crates/engine/src/settlements/`**, on the perks precedent: the
catalogue is data because prose plainly is; behaviour is Rust because a
temperament is a hook into particular formulas with no shared shape.

- `mod.rs` — `SettlementKind` (Mainframe/Server), `Specialty`
  (Gear/Materials/Routines/Programs), `Temperament`, `SettlementKey`.
- `catalogue.rs` — `SettlementDb` loading `assets/settlements/*.ron`: id, name,
  blurb, kind, specialty, temperament. Follows `PerkDb::load_dir` — malformed
  file skipped with a warning, never a panic; absent directory loads empty.
- `placement.rs` — the derivation.

**Placement is a property of the map, not an event.** An FNV-1a fold of
`(world seed, region coords)` reduced through the high-bit reducer used by
`rock::RockDb::kind_at` and `descriptions.rs` (**never `%`** — see the
description-selection trap) answers, for a region of N×N chunks: does it hold a
settlement, at which cell, and which catalogue entry. No spawn event, no
despawn; a town is simply *there* when you arrive.

**Materialization** mirrors `ensure_local_population` exactly: when a chunk
within margin of the player is reached, if its region's derivation yields a
settlement and one is not already spawned, spawn the entity and record it.

**`resources::Settlements`** — `HashMap<SettlementKey, KnownSettlement>` where
`SettlementKey` is region coords (stable, unlike `Entity`) and `KnownSettlement`
stores the **whole resolved catalogue entry**, following `ActiveContract`,
`SortieSave` and `CaravanSave`: a file edited or deleted mid-run must not strand
a relationship already earned. Saved behind `#[serde(default)]`.

**UI.** A glyph on the zone map (`M` / `s`), a hue in `hud::palette::glyph`, an
examine line, and `Mode::Settlement` — a hub opened by stepping onto the tile,
showing name, kind, specialty and blurb. Two traps: a new `Mode` variant does
**not** fail to compile and ships as a blank screen unless added to `ALL_MODES`
*and* the draw match; and lowercase letters are row selectors, so every new
screen action must be UPPERCASE.

**Names may repeat** across a truly unbounded map. Acceptable, or disambiguate
by region — decide at implementation, do not block on it.

## Phase 3 — The market

`Game::commit_caravan_basket` (`crates/engine/src/game/caravan.rs:1388`) is
documented as *the one commit door*. Do not add a second copy of it. **Extract
its core** — validate every line before anything is spent, sells land before
buys so a basket can fund itself, one tick for the whole commit — and have both
the caravan and the settlement market call it. Sharing the concept, not the
string, is the rule here.

A settlement's shelf is **derived per epoch** from `(world seed, settlement key,
epoch)`, the pattern `caravan_shelf` and `board_defs` already use, weighted by
the town's specialty and scaled by its kind (a mainframe carries more rows and
higher tiers than a server). Buyback uses the existing `ShelfKey`
(`crates/engine/src/game/trade.rs:101`) mechanism, keyed to the settlement.

## Phase 4 — Relations

`settlements/relations.rs`. A signed `standing` per `SettlementKey`, banded, with
**named queries** in the module rather than callers naming variants — the perks
module is the shape to copy, and its census test
(`every_perk_has_a_query_that_answers_what_it_is_worth`, exhaustive so a variant
with no query fails to compile) is the shape of the test to write.

Movers, each a call to one door `Game::adjust_standing`, never a write beside it:

| Event | Direction |
|---|---|
| Complete / abandon / expire a town's contract | up / down / down |
| Trade volume at their market | up, per N Credits transacted |
| Nest cleared or Stack collapsed within radius of a town | up |
| Gift of materials or Credits | up |

**Consequences, structured for extension.** Ship refuse-service (market and
board closed below a threshold) and route predation (Phase 6). Model the
consequence set as an enum or query table from the start so town-sourced raids
and hostile patrols are new arms, not a rewrite — that was an explicit ask.
Raids already exist as a system, so the later arm has a target.

## Phase 5 — Town job boards

Reuse `ContractDef`, `Objective` and `Reward` untouched — they are already data
enums and already moddable. A settlement board is `Game::board_defs`
(`crates/engine/src/game/contracts.rs:451`) re-derived from
`(world seed, settlement key, epoch)`, filtered by standing band and specialty.

Three real pieces of work:

1. `ActiveContract` gains `issuer: Option<SettlementKey>` behind
   `#[serde(default)]`, so completion knows whose standing to move.
2. `Objective::Deliver` is the one objective not polled — it advances only
   through `Game::deliver_to_contract` at the Broker. A town-issued delivery
   needs the town as a second delivery point.
3. Completion routes through `Game::complete_contract` (`contracts.rs:220`) into
   `adjust_standing`.

The player's own Broker is untouched and keeps boarding its own contracts.

## Phase 6 — Caravan routes

**One-off dispatch** first, on the sortie shape: `Game::dispatch_sortie`
(`crates/engine/src/game/sortie.rs:212`) validates members, charges provisioning,
pushes a record into a resource and queues a `TransitCue`; `step_sortie`
(`sortie.rs:394`) ticks it and calls the return door at completion. A route is
that, with cargo instead of a squad and a settlement instead of a site.

**Standing routes** second, unlocked by a standing band plus a structure: a
persistent record that ticks goods out and proceeds in until severed. Hostile
towns along a route prey on it.

`RouteSave` stores the whole resolved settlement record, same rule as everywhere
else.

**Worth knowing:** sorties are engine-complete and have **no UI at all** — no
`Mode`, no key, no screen calls `dispatch_sortie` or `sortie_board`. The only
player-visible trace is the party screen's "Away on a sortie" label
(`crates/gui/src/render/party.rs:270`). Building the route screen puts most of a
sortie screen on the table for nearly free. Out of scope unless asked, but flag
it at Phase 6 rather than discovering it there.

## Before implementation

**Starting from a cleared session:** this file is the whole handoff. Read it,
then `CLAUDE.md`, then invoke the `seams` skill before touching the Stack, the
base, contracts, saves or the HUD — Phases 1, 3, 5 and 6 each do. The
"What already exists" section above replaces re-running an exploration pass;
verify a line before relying on it, but do not re-survey.

Per the brainstorming skill's architectural path and the repo's process rule
(two-plus crates and a possible save-format change ⇒ full pipeline), the first
action is to copy this plan into
`docs/superpowers/specs/2026-09-04-settlements-design.md`, commit it, and add it
to `docs/superpowers/INDEX.md`. Then branch and work the phases — one branch per
phase, each landing on `main` with its own version bump, `CHANGELOG.md` section
and annotated tag, per the repo's release-per-change rule.

Per-phase discipline, unchanged by size: branch first, TDD with a failing test
first, a commit per green step, `cargo fmt` and `cargo clippy --workspace` after
each change, and `cargo test --workspace` as the gate before any phase is called
done. Read the `seams` skill before touching the Stack, the base, contracts,
saves or the HUD — several phases here do.

## Verification

- **Phase 1**: a save carried across a breach keeps its `tile_overrides`, its
  Stack entrances and its wild population's coordinates; a pre-rename save loads
  with `Mainframe` tiles intact via the serde alias; `balance_sim` curves.
  `cargo run -- --template extraction` opens mid-run for a hand check.
- **Phases 2–6**: engine unit tests per phase; a `dev-saves/` template captured
  once settlements are placeable, so later phases do not start with an hour of
  play. `savetool capture` records it.
- **Play**: agents cannot run the game in this environment — no display. Every
  phase ships with a green suite and zero screen time, and the feel questions
  (does the world hardening read as intended, is a town worth walking to, do
  routes feel like income or like idle) are yours to answer at the keyboard.
