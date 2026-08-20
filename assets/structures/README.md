# Custom structures (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all structure files
    name: "Display Name",
    glyph: '#',                   // single character shown on the map
    color: Magenta,                // one of: White, Gray, Green, DarkGreen, Red,
                                    //         Yellow, Blue, Magenta, Cyan, Brown,
                                    //         Orange
    build_cost: [("core_fragment", 3)],  // list of (item id, quantity) pairs
    // build_cost above, and every other item reference below (work.produces,
    // teleport_cost, trade.buy), all take
    // any item id from assets/items/*.ron — see assets/items/README.md for
    // the schema, and the top-level README's "Item ids" for the full set.

    // Optional; can be left out entirely (defaults to an empty string, which
    // the shipped-assets test refuses for anything in this repo). One line
    // on what the structure does, shown in the build menu. This used to be
    // derived automatically from the capability fields below (`work`,
    // `raid_defense`, and so on); it's authored text now,
    // so a modder controls exactly how their structure reads — but that also
    // means nothing checks it against those fields, so if you change a
    // structure's capabilities, update the description to match by hand.
    description: "Extracts Core Fragments while a program is posted to it. The cheapest thing you can deploy.",

    // Omit (`None`) for a purely decorative/utility structure. Set `Some(...)`
    // to make it assignable to a tamed creature via the cronjob menu.
    // `ticks_per_unit` is the machine's *baseline* rate — one unit of
    // `produces` every that many ticks — which the posted program's
    // `base_speed` (see `assets/species/README.md`) then scales faster or
    // slower. The player has no species and works a node at the baseline,
    // so `ticks_per_unit` is exactly what working it by hand costs. The
    // scaled rate is baked in the moment a program is posted, or the
    // player sets to work, not re-read every tick afterward — a cronjob
    // already running keeps its old rate, even across a save/load, until
    // it's reassigned.
    //
    // A node is a tap, not a reserve: there's no pool to mine down, and it
    // never runs dry. What paces it is the top-level `capacity` below — the
    // node fills its own output buffer and then *clogs*, producing nothing
    // more until the player walks over and collects (`c`). Production does
    // not go into the player's inventory; it goes into the structure.
    //
    // `level` (optional, defaults to `None`) makes each completed cycle a
    // gamble instead of a guaranteed yield: with it set, there's only a
    // level-based percentage chance the cycle actually pays out (a level-1
    // node succeeds about half the time), and a miss still costs the full
    // cycle. Higher levels succeed more reliably. Leave it out entirely for
    // a node that always yields on completion, same as before this field
    // existed.
    //
    // How much a completed cycle actually pays out is not one unit: it's the
    // structure's upgrade tier if it has one (see `upgrade` below), plus one
    // per zone level below the current one. A Mk1 node pays 1 in zone 1 and
    // 3 in zone 3; a Mk5 node pays 5 and 7. Depth and tier add rather than
    // multiply, so neither compounds the other. The zone part is read when
    // the cycle completes, not when the structure was deployed, so a base
    // carried into a deeper zone immediately earns at the deeper rate.
    //
    // There are two exceptions, and they're independent of each other.
    //
    // A `produces` item that declares `banked` (see
    // `assets/items/README.md`) — Research Data, for instance — always pays
    // exactly one unit per cycle. Nothing paces a bank, since it has no
    // ceiling and never clogs, so what keeps a flat 1 honest is demand:
    // the research tree is a fixed ladder, and a payout that doubled per
    // zone would collapse it rather than accelerate it.
    //
    // `flat_payout` (optional, defaults to `false`) opts *this node* out of
    // the same curve, whatever it produces. Set it for a node whose output is
    // consumed one at a time rather than in bulk: a taming catalyst is spent
    // one per decompile attempt, so a Mk5 in zone 5 paying nine a cycle
    // outruns the sink entirely. Leave it off for salvage.
    //
    // No shipped structure sets it — the Compiler used to, and now assembles
    // its catalysts out of Core Fragments instead. The field is here for
    // mods, and `flat_payout_takes_a_node_off_the_tier_and_depth_curve` is
    // what keeps it working.
    work: Some((produces: "core_fragment", ticks_per_unit: 5, level: Some(1))),

    // Optional; defaults to 20. How many units this structure's *output
    // buffer* holds before it clogs and stops producing. Every deployed
    // structure has one, whether or not it produces anything — the player
    // collects from a structure's output buffer by standing next to it, and
    // a neighbouring machine pulls its ingredients from the same place.
    // Counted in total across everything in the buffer, not per item.
    //
    // Top-level rather than inside `work` because a structure can have an
    // output buffer without being a work node.
    capacity: 20,

    // Optional; defaults to false. Marks this structure as a depot: somewhere
    // a posted program may empty a clogged machine's buffer into. When a
    // machine fills up, its own program takes five units out of it, walks to
    // the nearest structure with this set that has room, drops them there,
    // and walks back — so a base with a depot keeps producing while you are
    // away instead of stopping at the first full buffer.
    //
    // A flag rather than "has an output buffer and runs no job", because
    // every deployed structure has an output buffer — that rule would make a
    // Home and a Data Cache depots too. A depot never enters the cronjob
    // menu: it is delivered to, not worked.
    //
    // The `capacity` above is what makes one worth building; the shipped
    // Depot holds 100, five machines' worth of full buffers.
    stores: true,

    // Optional; can be left out entirely (defaults to no assembling). If
    // set, this structure automatically builds `item` out of ingredients it
    // pulls from the output buffers of the four structures orthogonally
    // touching it. Diagonals feed nothing. `ticks_per_unit` is this
    // machine's baseline rate the same way `work`'s is above — one unit
    // every that many ticks at the baseline, scaled by whatever program is
    // posted to it. Like `work`, it needs a program assigned to it via the
    // cronjob menu; unlike `work`, it consumes, which is what lets machines
    // form a chain across the base.
    //
    // There is no recipe here, and that's deliberate: the machine runs the
    // named item's OWN `craftable.cost` from assets/items/*.ron. So a recipe
    // is written once and can never drift between the crafting bench and the
    // machine, and any craftable item you add — including one with several
    // ingredients — is automatable for free.
    //
    // `item` must name an item that actually declares `craftable`. One that
    // doesn't builds a machine that can never run and says nothing about it.
    //
    // A machine can also be a crafting bench. The Armory and Fabricator each
    // assemble one item while every other recipe naming them stays
    // hand-compiled — so setting this does not cost a structure its bench
    // role. It does change its category: `category()` files a structure by
    // `assembles` when it is set, because that is what decides whether it
    // wants a program and feeders touching it.
    assembles: Some((item: "patch_routine", ticks_per_unit: 8)),

    // Optional; can be left out entirely (defaults to no regeneration).
    // If set, the structure restores `per_tick` Power to the player every
    // tick that they're standing within `radius` tiles of it — no assigned
    // worker and no input item, unlike `work`.
    // Stacks additively across every in-range structure that sets it, and
    // clamps at full Power. This is how the Recharger Node works:
    // `power_regen: Some((per_tick: 1.0, radius: 10))`, a radius chosen to
    // cover the whole of a base rather than a corner of it.
    power_regen: Some((
        per_tick: 1.0,
        radius: 10,
    )),

    // Optional; can be left out entirely (defaults to 0). What this
    // structure needs from the base's Grid to run, summed every tick
    // against every deployed structure's `power_supply`. A machine whose
    // draw doesn't fit the base's remaining supply goes unpowered and makes
    // no progress until the base has spare capacity again.
    //
    // The Grid is NOT the same resource as `power_regen` above: `power_regen`
    // restores the player's Power (their `PowerReserve` stat), while the Grid
    // is a base-wide capacity that only machines draw against. A structure
    // can set either, both, or neither — the Recharger Node sets both, at
    // different values, which is exactly why they're two fields instead of
    // one block.
    power_draw: 2,

    // Optional; can be left out entirely (defaults to 0). What this
    // structure contributes to the base's Grid — see `power_draw` above for
    // what draws against it. Home always supplies some, so a fresh base
    // isn't stuck at zero capacity before anything is built.
    power_supply: 4,

    // Optional; can be left out entirely (defaults to no symlink). If set,
    // this structure is a symlink target: the player can "use symlink" (`u`)
    // to instantly teleport to it from anywhere on the map, paying the
    // listed item cost.
    teleport_cost: Some([("power_cell", 4)]),

    // Optional; can be left out entirely (defaults to false). If true,
    // walking onto this structure breaches the player into the next zone
    // sector instead of blocking movement — see `Game::enter_next_zone`.
    // Wild programs in the new zone spawn with stats doubled per zone
    // level, and there's no portal back down. `build_cost` above is
    // treated as a *base rate* for a zone-portal structure: each quantity
    // grows by 50% of that rate per zone level, so a 10-fragment portal
    // costs 10 out of zone 1, 15 out of zone 2, 20 out of zone 3. No other
    // structure's cost changes with depth.
    // A zone-portal structure is consumed when the player steps onto it: it
    // does not travel to the next zone the way the rest of the base does
    // (see `enables_rest`/`Game::enter_next_zone`), so every breach costs a
    // fresh build.
    // Breaching also clears the player's Currency and CraftCurrency items
    // (the two economy roles in assets/items/*.ron): each zone funds its own
    // exit. ResearchCurrency is banked progress and is kept, as are gear
    // (fused copies included) and supplies.
    zone_portal: true,

    // Optional; can be left out entirely (defaults to false). If true, this
    // structure is a Contract Broker: standing near it opens the contracts
    // screen with what the sector is currently offering, and it is where a
    // `Deliver` contract's items are handed over. See
    // `assets/contracts/README.md` for what a contract is.
    //
    // A plain flag rather than a block, because a Broker has nothing to
    // configure: what it offers is derived from the world seed, the sector
    // and the clock, not authored on the building. Several Brokers therefore
    // all show the same board.
    issues_contracts: true,

    // Optional; can be left out entirely (defaults to no trading). If set,
    // this structure is a trading post: the player can "trade" (`t`) with
    // it to sell any inventory item (except the trade currency itself), or
    // buy any item listed in `buy` for its Credit cost.
    //
    // `sell_rate` is NOT the sale price — it is this trader's multiplier on
    // what an item is already worth (`value` in the item schema). A sale
    // pays `value * sell_rate` per unit, so 1 pays the going rate, 2 pays
    // double, and a trader that lowballs everything is a single number. Set
    // it to 1 unless this trader is meant to be better or worse than the
    // rest.
    //
    // It prices two things, not one. Whatever this trader buys goes onto a
    // buyback shelf the player can purchase back at twice what it paid — so
    // raising `sell_rate` makes selling here more rewarding *and* undoing a
    // sale more expensive. The multiplier is an engine constant
    // (`tuning::BUYBACK_PRICE_MULTIPLIER`), not a field here: what a trader
    // deals in is content, how steep the economy is isn't.
    //
    // `buy` costs, by contrast, are flat authored prices and ignore `value`
    // entirely — what a trader charges for its stock is its own business,
    // and deliberately isn't derived from what the thing is worth.
    //
    // A shelf belongs to the tile a trader stands on, not to the building,
    // so one destroyed by a raid and rebuilt on the same footprint reopens
    // with its stock intact — and two traders of the same kind in one zone
    // keep separate shelves. Every shelf is wiped when the player breaches
    // to the next zone, alongside their build salvage.
    //
    // `program_sell_divisor` is optional inside the trade block (defaults to
    // None — items only). Set it and this trader also buys the player's
    // tamed programs, paying `power / divisor` Credits rounded down, where
    // power is the program's max HP + Attack + Defense. The payout never
    // drops below 1. Selling despawns the program permanently and frees the
    // roster slot it occupied, so this is the player's way out of a full
    // roster; a sold program is destroyed, never shelved for buyback. A
    // divisor of 0 means the same as omitting it.
    trade: Some((
        sell_rate: 1,
        program_sell_divisor: Some(10),
        buy: [("ice_breaker", 5), ("power_cell", 3)],
    )),

    // Optional; can be left out entirely (defaults to 30). How much damage
    // this structure can take from GC Entropy Sweeps — the player-facing
    // name for what the code still calls a raid, `Game::raid_check` —
    // before
    // being destroyed. An assigned cronjob worker/guard fights a raid off,
    // reducing the damage by its Defense stat; an unassigned structure
    // takes the raid's full damage (less any raid_defense below).
    // Damage is permanent unless something with a `repair` field (see below)
    // is standing — structures never heal on their own. Ignored entirely
    // when `raidable: false` (see below).
    durability: 30,

    // Optional; can be left out entirely (defaults to true). Set to false
    // to make the structure impossible to raid: it's deployed with no
    // durability pool at all, so `Game::raid_check` can never select it,
    // it never takes damage, and no [HP x/y] is shown for it anywhere.
    // `durability` above is inert when this is false. This is how Home
    // works — losing the structure that gates every other build, anchors
    // symlinks, and can only exist once would strand the player rather
    // than cost them something.
    raidable: false,

    // Optional; can be left out entirely (defaults to 0). Flat raid-damage
    // reduction this structure contributes to *every* raid, against *any*
    // deployed structure, for as long as it's standing — not just itself,
    // and it stacks additively across every deployed structure that sets
    // this (e.g. several Shields). Applied before an assigned worker/guard's
    // own Defense-based mitigation, so the two stack. This is how the
    // Shield structure works: `raid_defense: 2` with no `work` recipe — one
    // Shield halves an ordinary raid, two absorb it entirely.
    raid_defense: 2,

    // Optional; can be left out entirely (defaults to 0). How many extra
    // tamed-program (pet) slots this structure grants while it's deployed.
    // The total pet limit is `3 + the sum of this across every deployed
    // structure`, so several of them stack. This is how the Data Cache works:
    // `pet_slot_bonus: 2` with no `work` recipe — each deployed cache lets you
    // own two more tamed programs (across party, cronjobs, and idle pets).
    pet_slot_bonus: 2,

    // Optional; can be left out entirely (defaults to 0, meaning no limit).
    // How many of this structure may stand at once — the build is refused
    // past it, before anything is spent. This is where a structure whose
    // *effect accumulates* is bounded, and the Line Driver is what it exists
    // for: `max_deployed: 3`, so a base's Grid supply grows three steps and
    // no further. Bounding it by whatever downstream constant the effect
    // happens to clamp against instead would put the limit somewhere no
    // player ever meets. Every other shipped structure leaves this at 0.
    max_deployed: 3,

    // Optional; can be left out entirely (defaults to no rest capability).
    // If set, `Game::rest` (recharge/overnight rest) is only allowed while
    // the player stands within `radius` tiles of this structure — resting
    // has no other way to happen. `cost` (optional inside the block,
    // defaults to an empty list) is spent per rest, checked and taken after
    // every other gate passes; an empty list means a free rest, same as
    // before this field existed. The price sits with the structure that
    // grants rest rather than as a single global rate, so a modded
    // alternate rest structure can charge differently, or nothing. This is
    // how Home works: `enables_rest: Some((radius: 10, cost: [("outlet", 1)]))`
    // — a radius covering the whole of a base rather than a corner of it,
    // priced at one Power Outlet.
    enables_rest: Some((radius: 10, cost: [("outlet", 1)])),

    // Optional; can be left out entirely (defaults to a permanent
    // structure). If set, this structure automatically collapses once
    // `max_ticks` ordinary game-clock ticks have passed since it was
    // deployed — no refund, it just disappears. Ticks spent inside a
    // `Game::rest` cycle don't count toward this, so a structure that also
    // sets `enables_rest` isn't worn down any faster by actually being
    // used to rest than by sitting there idle.
    temporary: Some((max_ticks: 20)),

    // Optional; can be left out entirely (defaults to repairing nothing).
    // If set, this structure restores `per_tier` Durability to *every*
    // deployed structure — itself included — every 20 ticks, multiplied by
    // its own upgrade tier. It stacks additively across every deployed
    // structure that sets this, the same way `raid_defense` does, so a
    // tier-2 repairer and a tier-3 repairer restore five between them.
    //
    // Pair it with `upgrade` or the tier is always 1 and `per_tier` is just
    // a flat rate. Nests carry Durability too, but a repairer never heals
    // one — only structures.
    //
    // This is how the Patch Node works: `repair: Some((per_tier: 1))` with
    // no `work` recipe. It is the only source of repair a *structure* can
    // declare — structures do not heal on their own at all — and the other
    // source is not a building: a Medic-class program posted to guard a
    // structure mends that one structure (see `assets/species/README.md`,
    // "Base jobs"). With neither standing, raid damage is permanent.
    repair: Some((per_tier: 1)),

    // Optional; can be left out entirely (defaults to un-upgradeable). If
    // set, the player can upgrade this structure (`U`) through
    // tiers, starting at Mk1 and stopping at `max_tier`. The cost to reach
    // tier N is each quantity in `cost` multiplied by N — so with the
    // 10 Core Fragments below, Mk1->Mk2 costs 20 and Mk2->Mk3 costs 30.
    //
    // `max_tier` is a *permanent* ceiling, and it is not the only one: the
    // player also cannot pass the zone they have breached to, so Mk*N*
    // needs zone *N*. Nothing at all upgrades in zone 1. Setting
    // `max_tier` above 5 is legal and simply means the ceiling stays the
    // zone's for longer.
    //
    // A structure's tier does two things at once. It multiplies its `work`
    // payout (on top of the zone multiplier — see `work` above), and it
    // becomes the node's effective `level`, which raises the odds a gather
    // cycle actually yields. That reliability saturates at level 6 (100%),
    // so tiers past that add payout only.
    upgrade: Some((max_tier: 5, cost: [("core_fragment", 10)])),

    // Optional; can be left out entirely (defaults to false). If true,
    // owning one of these anywhere lets the player extract a routine out of
    // a program they own, destroying it and losing every other routine it
    // carried — see `Game::extract_routine`. Checked by ownership
    // (`Game::has_structure`), not proximity, so a bench built anywhere on
    // the map counts. This is how the Compiler works.
    extracts_routines: true,
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the structure for readability, e.g. `data_cache.ron`.

## Research gating

A structure named in some research node's `unlocks_structures` can't be
built until that node is researched — see `assets/research/README.md`. A
structure named by **no** research file is buildable from turn one, which is
how the Home, Mining Node, Research Node, Recharger Node and Zone Portal
stay available at the start, and why a structure mod that ships no research
file keeps working unchanged.

The Research Node itself (`research_node.ron`) is the source of Research
Data: assign a tamed program to it via the cronjob menu, same as a Mining
Node.
