# Outposts

**Status:** design approved in conversation 2026-09-23; spec awaiting review.
TODO list item 102 ("create outposts").

A one-tile fixture on the zone surface where the player posts tamed programs
to extract materials. It starts out yielding raw materials, and as it
**grows** — driven by crew, hauling and safety — it yields processed and then
complex ones. Output accumulates at the outpost and comes home by caravan
route or by hand. It can be raided.

This is concept **B** of four brainstormed. The other three — a forward camp
(rest/stash), a frontier post (reach), a Stack beachhead — are out of scope,
but the seams below are chosen so each lands as an addition to this one
rather than a second subsystem (§10).

---

## 1. Decisions and why

| Decision | Chosen | Rejected, and why |
|---|---|---|
| Physical form | Zone-surface fixture + a screen | A second base space: `BaseGrid`, `Locale::Base` and `stands_in_base_space` all assume there is one |
| Representation | A **record** in `resources::Outposts`, keyed by tile | A zone-surface entity with `Structure`: every reader that treats `Structure` as meaning base space would be wrong, and none would fail to compile |
| Site | Anywhere legal; the biome under the tile picks the yield | Derived deposits; cleared nests |
| Logistics | A caravan route with an outpost endpoint, plus take-by-hand | Teleporting to base stock (distance and safety would stop mattering) |
| Risk | Raidable | Safe havens |
| Building | Instant when the kit is used (v1) | Crew-built: builders would have to walk off the base grid. Follow-up |
| Away crew | No needs, no morale — `Sortie`'s omissions | A morale drain would be a second meter measuring the same neglect the growth bar already measures |

## 2. The record

```rust
// resources.rs
pub struct Outposts(pub BTreeMap<(i32, i32), Outpost>);   // key = zone-surface tile

pub struct Outpost {
    pub biome: Biome,          // read once at placement, never re-derived
    pub growth: u32,           // the one stored progress number
    pub integrity: u32,
    pub stock: BTreeMap<ItemId, u32>,
    pub stale_ticks: u32,      // ticks spent with full stock; drives Stale → Declining
    pub cycle_progress: u32,
}
```

- A `BTreeMap` so iteration order is stable for the production roll and the
  save encoding (`Stock`'s precedent).
- **Crew is not on the record.** Membership rides
  `CreatureSave::outpost: Option<(i32, i32)>` on the creature, the way
  `sortie_index` does, and a runtime component `components::PostedAt(tile)`
  marks it. Stock capacity, crew cap and max integrity come from the def and
  tuning, so none of them is stored.
- Everything else is **derived on every read**: tier, trend, current yields,
  what the next tier needs.
- `tuning::MAX_OUTPOSTS` caps how many can stand at once.

## 3. Content: one def file

`assets/outposts/outpost.ron`, loaded by an `OutpostDb` that follows
`StructureDb::load_dir`'s pattern (a malformed file is skipped with a warning).
If it is absent, no outposts exist and the kit refuses with a message.

```ron
OutpostDef(
    name: "Outpost",
    glyph: '⌂',
    kit: "outpost_kit",
    tiers: [   // item ids illustrative; the content phase picks them
        ( yields: { DataVoid: ["raw_trace"], Deadlock: ["static_mesh"], ... } ),   // raw
        ( yields: { ... } ),                                                         // processed
        ( yields: { ... } ),                                                         // complex
    ],
    features: [],   // reserved — see §10
)
```

- **Which items is content, so it is data.** Naming item ids in Rust is the
  trap the `"relay"` / `dispatches_sorties` seam exists to close.
- **How fast, how many crew and what thresholds is tuning**, in a new
  `// Outposts` section of `tuning.rs`: `OUTPOST_TIER_GROWTH[3]`,
  `OUTPOST_TIER_CREW[3]`, `OUTPOST_GROWTH_PER_CREW`, `OUTPOST_DECAY_*`,
  `OUTPOST_STALE_GRACE_TICKS`, `OUTPOST_STOCK_CAP`, `OUTPOST_CREW_CAP`,
  `OUTPOST_MAX_INTEGRITY`, `OUTPOST_CYCLE_TICKS`.
- A biome with no entry at a tier falls back to that tier's first listed
  biome, so a new `Biome` variant yields something rather than nothing.
  Asserted by a census.
- **Censuses** in the engine's asset tests: every shipped biome has a row at
  tier 1; every named item exists; tier 3 names at least one zone material
  (`ZONE_MATERIALS`'s rule — a tier-3 row is where the zone ladder pays out).
- `assets/outposts/README.md` documents the schema.

## 4. Growth — the heart of it

**Tier** = the highest tier `t` with `growth >= OUTPOST_TIER_GROWTH[t]` **and**
`crew >= OUTPOST_TIER_CREW[t]`. Growth is kept when crew is short; output
falls to the tier the crew can hold. At that tier the yield is the tier's list
**unioned with every lower tier's**: a tier-2 outpost still yields raw
materials, and each cycle picks one entry from the union.

**Trend** is one pure function, `outposts::trend(&Outpost, crew, tier) ->
Trend`, derived from current conditions and never stored. The first match wins:

| Trend | Condition | Growth this period |
|---|---|---|
| `Declining` | `crew < OUTPOST_TIER_CREW[0]`, or integrity below `OUTPOST_DAMAGED_FRACTION`, or `stale_ticks > OUTPOST_STALE_GRACE_TICKS` | falls |
| `Stale` | stock full, still inside the grace period | unchanged |
| `Stable` | growth at the ceiling this crew can reach (the next tier's crew requirement is unmet, or max tier reached) | unchanged |
| `Growing` | otherwise | rises, scaled by crew above the minimum |

- One function answers the bar's colour, the screen's status line and the
  alert. The screen's reason string (`"Stale: stock full, no route"`,
  `"Stable: post 2 more for tier 3"`) is `Trend::reason(...)`, derived in the
  engine — gui builds no sentences.
- Growth moves slowly enough that tiers should not flicker. If playtesting
  shows they do, add an in/out gap like the needs system's hysteresis; not v1.
- `balance_sim` models none of this. The rates are an open tuning item (§11).

## 5. Production

`Game::run_outposts`, called from `tick_inner`:

1. Accumulate `stale_ticks` (or reset it) and apply the growth delta for
   `trend`.
2. Advance `cycle_progress`. At `OUTPOST_CYCLE_TICKS`, **each crew member**
   rolls a payout through `systems::mining_success_chance` with a
   `CycleModifiers` built for that program (class and base INT; `morale: 0`
   and `need_strain: 0` because away crew have neither). That is a **call**,
   not a copy of the formula.
3. On a success, pick an item from the current tier's union: one `GameRng`
   draw, and none when the union has one entry. Add 1 to stock unless it is
   full.

- Outposts tick in key `(x, y)` order (the traps precedent).
- Nothing spends `GameRng` while there is no outpost — asserted by a test,
  predation's pattern.
- Each unit reports through `base_ledger::emit` as an `Acquire` with a new
  provenance `Outpost`, so the base output page sees it without a second
  counter.

## 6. Crew — `ProgramRole::Outpost`

A sixth role, inserted after `Sortie` in `role_of`'s chain. Like `Sortie`,
its consequences are **omissions**: no body, out of the labour pool, no needs
decay, not drawn, not in the party, and a rest does not repair it.

- The three exhaustive matches (`roster_rank`, gui's `role_heading`, and the
  rest-repair arm in `game/turn.rs`) must each gain an arm.
- Every `==`-based reader has to be audited: fusion, party-add, wield,
  dispatch to a sortie, posting at base, extraction's ownership check. Each
  one must *refuse* an outpost crew member, not silently allow it. The plan
  lists them, and each gets a refusal test.
- **Posting**: `Game::post_to_outpost(tile, creature)`. It refuses unless you
  are standing at the outpost (visit open), the program is `Staff`, and the
  crew is below the cap. Every refusal lands before anything changes.
- **Recalling**: `Game::recall_from_outpost`. The program becomes `Staff`, and
  its base-space `Position` is set to the base entry tile, the arrival path a
  program downed in the Stack already uses.
- **Destroyed**: dissolving or benching a crew member clears `PostedAt`. The
  two destruction paths (`dissolve_tamed_program`, `fuse_companions`) are
  audited like the `CarryingProgram` precedent.

## 7. Getting the output home

**By route.** `Route` gains `outpost: Option<(i32, i32)>` (serde-default).
When it is `Some`:

- The outbound leg carries no cargo and sells nothing.
- On arrival it loads up to `ROUTE_OUTPOST_CARRY` units from the outpost's
  stock.
- The inbound leg rolls predation on the **goods**, with the same
  `settlements_near_route` segment geometry and the same
  `Standing::preys_on_routes`.
- It lands through `return_material`.
- A standing route reloads. If the outpost is gone, the route stalls with a
  log line.

Dispatch goes through the existing `Game::dispatch_reach` (Relay gated). The
cargo picker gains the outpost as a destination. It has no sale preview —
the route screen shows "hauls up to N".

An optional `destination` field was rejected for the save, because changing
the type of `Route::destination` is not additive. The code reads the pair
through one accessor, `Route::end() -> RouteEnd::{Settlement, Outpost}`,
which is the extension point for later endpoints.

**By hand.** From the outpost screen, `c` opens `Mode::Transfer` against the
outpost's stock, take side only (no put). This reuses the one basket and one
commit.

## 8. Raids

In `raid_check`'s sweep, after the base resolves, each outpost rolls
separately:

- **Chance** = `OUTPOST_RAID_BASE_CHANCE`, raised by `OUTPOST_HOSTILE_TOWN_BONUS`
  for each known town within `SETTLEMENT_RAID_RADIUS` whose
  `Standing::sends_raiders()`. It is reduced by crew:
  `OUTPOST_CREW_DEFENSE` each, capped below certainty.
- **Hit**:
  - Take `OUTPOST_RAID_DAMAGE` integrity.
  - Steal `OUTPOST_RAID_STEAL_FRACTION` of the stock.
  - Bench one crew member through `Game::bench_or_dissolve`, the one door
    that already handles the `DifficultyMode` branch.
  - Log a `MessageKind::Raid` line, and flash `THREAT` on the tile if it is
    drawn.
- **At zero integrity** the outpost goes **dark**: every crew member is
  recalled to staff, growth stops, and it stays on the map to be repaired
  (`R`, which pays materials from the pack, like a research bill). It is
  never removed by a raid.
- Outposts never enter `resolve_siege_offscreen` or `open_siege`. A siege
  stays a base event.

## 9. Placement, map and screen

**Placing.** Using the kit item on the zone surface calls
`Game::found_outpost(tile)`. It refuses when:

- the tile is not walkable;
- the biome is `Biome::Platform`;
- the player is underground or in base space;
- the tile is within `MAX_BUILD_DISTANCE_FROM_HOME` of the anchor;
- the tile is a settlement, nest or Stack link, or another outpost stands
  within `OUTPOST_MIN_SPACING`;
- `MAX_OUTPOSTS` already stand.

Every refusal lands before the kit is spent. The kit is a crafted item gated
by a research node (both content). Outposts persist across a breach, like
everything else on the surface, and a tier-3 zone material is read at the
current `ZoneLevel`.

**Entering.** `resources::PendingVisit` generalises from
`Option<SettlementKey>` to `Option<Visit>` with
`Visit::{Settlement(key), Outpost(tile)}`. The bump is a fifth arm of
`move_player`'s ladder: it queues the visit and leaves `Position` unchanged.
`take_visit` stays a drain. `Visit` is the second extension point (§10).

**Map.** The glyph comes from the def and is drawn through
`drawn_on_surface_map`, and examine names it.

- **Growth bar**: the tile's bottom-edge progress bar. It fills within the
  current tier, and its colour comes from the trend: its own hue / not
  animated / `WARN` / `ATTENTION`.
- **Tier pips**: 1–3 in the top-right corner.
- **Dark**: the glyph is dimmed and there is no bar.

**Screen**, `Mode::OutpostVisit`, added to `ALL_MODES` and `needs_status_banner`:

```
 ┌─ OUTPOST · Deadlock ──────────────────── Tier 2 · Processed ─┐
 │ Growth  [██████████████░░░░░░░░]  Growing  (+ with 3 crew)    │
 │ Integrity 84/100          Stock 37/60   Route: ⇄ running      │
 │ CREW (3/6)                        YIELDS NOW                  │
 │  a  Vexa-7    Scrapper  lv 12      Salvage Coil   tier 1      │
 │  b  Hollin    Leech     lv 9       Cache Grain    tier 2      │
 │  c  Ossi-3    Sentinel  lv 11      — tier 3 needs 5 crew      │
 │ [P] Post  [U] Recall  [R] Repair  [c] Take stock              │
 └───────────────────────────────────────────────────────────────┘
```

- Lowercase selects crew rows, and actions are uppercase. `c` is the one
  exception, and it is already the transfer key everywhere.
- Every figure comes from one derivation, `Game::outpost_report(tile)`.
- The screen has no scroll, so `OUTPOST_CREW_CAP` rows is a layout
  constraint, with a height census.

**Attention.** A move into `Stale`, `Declining` or `Dark`, or a raid hit,
posts through `alerts::post` keyed by (kind, outpost tile). It also counts
in `Game::attention`. It is announced only when the state changes
(`set_machine_status`'s rule).

## 10. Extension points (not built)

- **Forward camp (A)** and **frontier post (C)** are `features:` entries on
  the def, for example `Rest`, `Stash` or `Reach(radius)`, read by named
  queries at the seams they affect (rest pricing, the dispatch gate, the
  aid radius). The same record and the same screen, plus one more section.
- **Beachhead (D)** is a new `Visit` variant and a new `RouteEnd` variant.
- **Crew-built outposts**: the kit places a request that a crew walks out to
  finish. That needs a surface walk for base staff, which does not exist.

## 11. Open items

- All the rates in §4 and §8 are placeholders until playtesting. `balance_sim`
  gates none of them. Write the first playtest's numbers into
  `docs/measurements/`.
- Whether a Striker's second action or a Leech bonus should apply at an
  outpost. v1 passes `class` through `CycleModifiers` exactly as the base
  does, so whatever the base does, the outpost does.
- **§8's "flash `THREAT` on the tile if it is drawn" is not built.**
  `resources::EffectQueue`/`VisualEffect` is base-space by construction —
  `render/base.rs` and `crates/gui/src/lib.rs`'s `in_base` gate both read a
  `VisualEffect.pos` as a base-space cell, and an outpost's tile is a
  zone-surface coordinate in the same `(i32, i32)` shape. Pushing one there
  would flash the wrong tile (or the base's own tile by numeric accident)
  whenever the player happens to be in base space when a raid lands. A
  correct version needs its own zone-surface effect channel, `TransitCue`'s
  precedent one space over — not a shared queue disambiguated by a flag.
  Left undone; `raid_one_outpost` still logs and posts the alert, so the
  raid itself is fully reported, just not flashed on the map.

## 12. Save

Every addition is `#[serde(default)]`: `SaveData::outposts`,
`CreatureSave::outpost`, `Route::outpost`, and the `Visit` enum (saves hold
no pending visit). No `SAVE_FORMAT_VERSION` bump. A save→load test covers
each field, because a RON round trip cannot catch a skipped field.

## 13. Phasing

Each phase is a green, committable step:

1. **Record + growth + production + role + save** (engine only). Seeded
   tests for the trend table, each refusal, the no-draw-without-outposts
   rule, the save round trip and the role audit.
2. **Visit, screen, map marks, transfer take**: app-core + gui. ALL_MODES,
   height census, `Visit` drain test.
3. **Route endpoint**: goods leg, predation on goods, picker entry.
4. **Raids + alerts + attention.**
5. **Content**: the def file, the kit item, the research node, censuses,
   READMEs, CHANGELOG.

Four crates and a save change, so the full spec → plan pipeline applies.
