# Bounded income: rest costs a consumable, scan is deleted

Design, 2026-08-02.

## The problem

Nothing in the game limits how much a player can earn. Verified against the
source on 2026-08-02:

| Fact | Source |
| --- | --- |
| Scan (`g`) is 1 tick and pays ~0.6 Core Fragments in a rich biome | `game/turn.rs::forage`, `FORAGE_CHANCE_RICH` |
| It costs 0.15 Power, and a Power Cell — 2 fragments — restores 166 ticks of it | `HUNGER_DECAY_PER_TICK`, `assets/items/power_cell.ron` |
| So scanning returns ~50x the Power it burns | — |
| A Recharger Node pays +1.0 Power/tick in a 7-tile radius, no worker, no input, 10 fragments once | `assets/structures/recharger_node.ron`, `systems.rs:418` |
| Inside that radius Power *rises* 0.85/tick while scanning, so Power is not a cost at all | — |
| Rest runs 40 full ticks of cronjob payout, restores Fatigue and full HP for the player and every owned program, and repeats immediately | `game/turn.rs::rest`, `REST_TICKS` |
| A worked node refills to capacity at zero, so cronjobs never dry up | `systems.rs:208`, `systems.rs:289` |
| The only per-tick cost in the game is `RAID_CHANCE_PER_TICK` 0.012 — ~38% per rest — and repairs are paid in fragments | `tuning.rs:885` |

The Recharger Node is intended and stays. That settles the shape of the fix:
**Power cannot be the limiter**, so the limiter has to be the actions
themselves.

Why this matters beyond generosity: research, gear and structures all reduce
to keyboard time, and `balance_sim`'s level projections assume progression is
paced by what a player can earn.

## The intent

**The base is the farm.** Income should come from structures a player builds
and maintains — not from a key held down on one tile. Risk and infrastructure
pay; standing still does not.

## The design

### 1. Rest consumes a Power Outlet

A new craftable item, `assets/items/outlet.ron`, costing 5 Core Fragments.
`Game::rest` spends exactly one and refuses when the player has none.

This turns rest into an *investment* against 40 ticks of base output rather
than a free action:

| Structure | ticks/cycle | cycles per rest | yield per rest (zone 1, tier 1) |
| --- | --- | --- | --- |
| Mining Node | 10 | 4 | ~2 fragments (50% reliability) |
| Research Node | 14 | 2.8 | ~1.4 research data |
| Power Conduit | 6 | 6.6 | ~6 Power Cells |
| Compiler | 8 | 5 | ~5 ICE Breakers |

One Mining Node pays ~2 fragments against a 5-fragment rest — a net loss.
Break-even is roughly three worked nodes, which under the one-cronjob-per-
structure rule (`45870d3`) means three structures and three programs, not
three programs stacked on one node.

**Ordering is load-bearing.** The outlet is checked and spent only after the
existing gates pass — game-over, active battle, `require_surface`,
`nearby_rest_structure` — and before the 40-tick loop begins. A refused rest
must never consume one; a rest that *starts* has bought its ticks, so the
existing mid-loop `is_game_over` bail does not refund it.

**The seam:** `StructureDef::enables_rest` is already `Option<RestDef>`, so
the price becomes a new `#[serde(default)]` field on `RestDef` alongside
`radius`. `home.ron` becomes `enables_rest: Some((radius: 7, cost: [("outlet",
1)]))`. The cost sits with the structure that grants rest, a modder's
alternate rest structure can charge differently, and an existing `.ron` with
no `cost` still parses — as a free rest, which is today's behaviour.

Rejected: a fifth `EconomyRole` (`RestFuel`). The outlet is a consumable, not
a currency, and a role would fix one global rest price for every rest
structure.

### 2. Scan is deleted

`Game::forage`, `forage_chance`, the four `FORAGE_CHANCE_*` constants, and the
surface `g` binding in `app-core/src/app/playing.rs`. `g` keeps its Stack
meaning — the frame map — untouched.

Biomes do not become inert: habitat pools (`species.rs`) and walkability
(`world.rs`) are what carry them. `forage_chance` was a third consumer, not
the only one.

### 3. Keen Scavenger is repurposed, not deleted

Its entire effect is a forage bonus. `Perk`'s variant order is save format —
`PlayerSave::unlocked_perks` holds indices — so deleting the variant would
shift every later index and force a `SAVE_FORMAT_VERSION` bump for a change
that otherwise needs none.

Instead it boosts `mining_success_chance`, which is 50% at a level-1 node. The
flavour survives verbatim ("you read the terrain better"), the index does not
move, and the perk now backs the thing this change wants players investing in.
`KEEN_SCAVENGER_BONUS_PER_LEVEL` is re-documented as a mining constant and
`assets/perks/keen_scavenger.ron`'s description is rewritten.

### 4. The opening gets two free outlets

The player starts with 5 Core Fragments and Home costs exactly 5. The first
Mining Node costs 12 more, and with scan gone the only source is combat drops
at `WORK_RESOURCE_DROP` 1–2 per kill — roughly 8 kills before the base earns
anything. Meanwhile rest, still the only full heal, costs 5 fragments from
turn one.

Two Power Outlets go into the starting inventory
(`game/lifecycle.rs`, beside the 3 ICE Breakers / 3 Power Cells / 5 Core
Fragments) to cover getting established. This is the softener most likely to
need retuning after a playtest, and it is one line.

## Save format

**No bump.** Nothing new is persisted: the outlet is an ordinary inventory
item, `RestDef::cost` is asset data, and the perk keeps its index.

## Testing

- Rest refuses with no outlet, and the refusal does not tick the world.
- Rest consumes exactly one outlet on success.
- A rest refused by an *earlier* gate (underground, no Home nearby, in battle)
  consumes nothing — one test per gate ordering, since this is where a
  regression would silently tax the player.
- A `RestDef` with no `cost` field parses and rests for free, so existing and
  modded `.ron` files are unaffected.
- The outlet appears in `craft_recipes` and crafting it spends 5 fragments.
- `Perk::KeenScavenger` raises the mining roll and no longer names forage.
- Removal leaves no dangling references: `forage`, `forage_chance` and
  `FORAGE_CHANCE_*` are gone from engine, app-core and their tests.
- `cargo test -p feral-processes-engine balance_sim` — a moved curve is the
  signal, not a broken test. The sweep does not model rest or scan, so a
  *silent* suite here is expected and is not evidence the economy is unchanged.

## Docs to update in the same change

`assets/items/README.md` and `assets/structures/README.md` (the `RestDef`
field), `docs/manual.md` (the `g` key and how rest is paid for), the root
`README.md`, and `CHANGELOG.md`.

## What this does not fix

- **Nodes still refill at zero.** A worked node never runs dry. Left alone
  deliberately: the base is meant to be the farm, and a depleting base would
  fight that. Revisit only if rest-as-investment turns out to be too generous.
- **Rest is still the only full heal**, and now the only one that costs
  anything. If the opening reads as punishing, the outlet recipe or the
  starting count is the lever, not a second heal.
- Whether 5 fragments is the right price. It is arithmetic, not evidence —
  the same footing the Trace bands were on before they were played and moved
  40/100/180 → 25/70/140.
