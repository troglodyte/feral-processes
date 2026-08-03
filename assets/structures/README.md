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
    // to make it assignable to a tamed creature via the cronjob menu — it'll
    // produce one unit of `produces` every `ticks_per_unit` ticks.
    //
    // A node is a tap, not a reserve: there's no pool to mine down, and it
    // never runs dry. What paces it is the top-level `capacity` below — the
    // node fills its own output buffer and then *clogs*, producing nothing
    // more until the player walks over and collects (`C`). Production does
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
    // A `produces` item that declares a `bank_limit` (see
    // `assets/items/README.md`) — Research Data, for instance — always pays
    // exactly one unit per cycle, because its cap is what paces it and a
    // scaling payout would just overflow it.
    //
    // `flat_payout` (optional, defaults to `false`) opts *this node* out of
    // the same curve, whatever it produces. Set it for a node whose output is
    // consumed one at a time rather than in bulk — the Compiler's ICE
    // Breakers are spent one per decompile attempt, so a Mk5 in zone 5 paying
    // nine a cycle outruns the sink entirely. Leave it off for salvage.
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

    // Optional; can be left out entirely (defaults to no regeneration).
    // If set, the structure restores `per_tick` Power to the player every
    // tick that they're standing within `radius` tiles of it — no assigned
    // worker and no input item, unlike `work`.
    // Stacks additively across every in-range structure that sets it, and
    // clamps at full Power. This is how the Recharger Node works:
    // `power_regen: Some((per_tick: 1.0, radius: 7))`, a radius chosen to
    // cover a whole base (structures must be built within 7 tiles of Home).
    power_regen: Some((
        per_tick: 1.0,
        radius: 7,
    )),

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
    // exit. ResearchCurrency is banked progress and is kept, as are gear,
    // supplies and fusion tiers.
    zone_portal: true,

    // Optional; can be left out entirely (defaults to no trading). If set,
    // this structure is a trading post: the player can "trade" (`t`) with
    // it to sell any inventory item (except the trade currency itself) for
    // `sell_rate` Credits per unit, or buy any item listed in `buy` for its
    // Credit cost.
    //
    // `sell_rate` prices two things, not one. Whatever this trader buys goes
    // onto a buyback shelf the player can purchase back at twice
    // `sell_rate` per unit — so raising it makes selling here more
    // rewarding *and* undoing a sale more expensive. The multiplier is an
    // engine constant (`tuning::BUYBACK_PRICE_MULTIPLIER`), not a field
    // here: what a trader deals in is content, how steep the economy is
    // isn't.
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
        buy: [("ice_breaker", 4), ("power_cell", 3)],
    )),

    // Optional; can be left out entirely (defaults to 30). How much damage
    // this structure can take from raids (see `Game::raid_check`) before
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

    // Optional; can be left out entirely (defaults to no rest capability).
    // If set, `Game::rest` (recharge/overnight rest) is only allowed while
    // the player stands within `radius` tiles of this structure — resting
    // has no other way to happen. `cost` (optional inside the block,
    // defaults to an empty list) is spent per rest, checked and taken after
    // every other gate passes; an empty list means a free rest, same as
    // before this field existed. The price sits with the structure that
    // grants rest rather than as a single global rate, so a modded
    // alternate rest structure can charge differently, or nothing. This is
    // how Home works: `enables_rest: Some((radius: 7, cost: [("outlet", 1)]))`
    // — a radius covering the whole base, priced at one Power Outlet.
    enables_rest: Some((radius: 7, cost: [("outlet", 1)])),

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
    // no `work` recipe. It is also the *only* source of repair in the game —
    // structures do not heal on their own at all, so raid damage is permanent
    // until something declaring this field is standing.
    repair: Some((per_tier: 1)),

    // Optional; can be left out entirely (defaults to un-upgradeable). If
    // set, the player can upgrade this structure (`U`) through
    // tiers, starting at Mk1 and stopping at `max_tier`. The cost to reach
    // tier N is each quantity in `cost` multiplied by N — so with the
    // 10 Core Fragments below, Mk1->Mk2 costs 20 and Mk2->Mk3 costs 30.
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
