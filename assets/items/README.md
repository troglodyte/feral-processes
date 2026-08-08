# Custom items (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup. That
includes a file whose numbers aren't finite: RON accepts bare `NaN` and
`inf` literals, and they'd otherwise slip past every clamp downstream, so
any non-finite `taming_potency`, `consume.power`, or `consume.fatigue`
disqualifies the whole file.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all item files
    name: "Display Name",

    // Optional; can be left out entirely (defaults to an empty string, which
    // the shipped-assets test refuses for anything in this repo). One line
    // on what the item is for, shown wherever it's listed. Authored rather
    // than derived, so a modder controls exactly how their item reads — but
    // that also means nothing checks it against the fields below, so if you
    // change `equipment`, `consume`, or another capability, update the text
    // to match by hand.
    description: "Restores 25 Power. The staple of staying on the Grid.",

    // Optional; can be left out entirely (defaults to false, ordinary cargo).
    // A banked item is a pool rather than something the player carries, and
    // that one flag carries every consequence:
    //
    //   - it never counts against the cargo the player is shown;
    //   - it never scales with the zone payout curve, unlike ordinary salvage
    //     from a work node;
    //   - a work node that produces it (`work.produces`) delivers it straight
    //     to the player instead of filling its own output buffer, so it is
    //     never collected by hand and can never feed a neighbouring machine.
    //     An `assembles` machine is NOT yet wired this way — a banked item as
    //     an assembler's product still fills that machine's buffer and still
    //     has to be collected;
    //   - it is not listed in the inventory and cannot be bought or sold.
    //
    // Research Data is the only shipped item that sets it. There is no
    // ceiling on a bank — leave the item out of `craftable` costs and
    // structure `build_cost`s, since a banked item can't be an ingredient.
    banked: true,

    // Optional; can be left out entirely (defaults to 1, the flat rate every
    // item traded at before this field existed — so a mod written against
    // the older schema keeps behaving exactly as it did). What one unit is
    // worth in trade currency, before a trader's own `sell_rate` multiplier
    // (see assets/structures/README.md). Selling pays it; the buyback shelf
    // charges twice it.
    //
    // Two rules keep the shipped ladder from turning a base into a Credit
    // press, and a mod that breaks either one prints money:
    //
    //   1. Don't price an item above the total value of its `craftable`
    //      ingredients. Build salvage is sellable and a Mining Node produces
    //      it forever, so a profitable recipe is an infinite Credit loop.
    //   2. Don't price an item a structure `produces` (see the structures
    //      schema) above the default. Such an item is made from nothing on a
    //      timer, so its value is really a Credit-per-tick rate — rule 1
    //      can't see that, because the recipe isn't what's being run.
    //
    // Both are asserted against the shipped assets by
    // `no_craftable_item_is_worth_more_than_its_ingredients` and
    // `every_base_produced_item_sits_at_the_floor_price`. Your own items
    // aren't covered by those tests, so the rules are yours to keep.
    //
    // The shipped ladder, for calibration: anything printable 1; scavenged
    // gear 3-8; standard gear 12-16; the drop-only researched pieces 20-60;
    // premium gear 80-120. Worth tracks what a base *can't* manufacture.
    //
    // That ladder is load-bearing beyond trade. A boss defeated on the
    // *surface* pays gear drawn from a band of it that climbs with the zone
    // — see `Game::surface_boss_loot` and the `SURFACE_BOSS_LOOT_*` values
    // in `crates/engine/src/tuning.rs`. So `value` on an equippable item
    // says two things at once: what a trader pays for it, and which zone a
    // boss starts handing it out in. An item priced off the ladder still
    // trades sensibly and still drops, but it drops at the wrong point in
    // the run. Only items with `equipment` set are eligible, which is why
    // the Access Shard's value of 12 doesn't put it in a zone-1 boss's
    // pool alongside the Hardened Shell.
    //
    // Standard and premium armour and modules sit on that same ladder but
    // are paid for in refined goods rather than raw fragments, so rule 1 is
    // checked against the intermediate's value and not a fragment count.
    // `standard_and_premium_gear_is_made_from_intermediates` holds that
    // policy, and `scavenged_gear_stays_benchless_and_fragment_only` holds
    // the other half — the cheap tier stays craftable with no base standing,
    // which is what a fresh run, or one just swept flat, equips out of.
    value: Some(90),

    // Optional; can be left out entirely (defaults to no economy role). If
    // set, this item is the game's singleton anchor for that role — engine
    // logic looks up "the item with role X" rather than naming an id, so
    // swapping which item is the currency is a data change, not a code
    // change. One of: `Currency`, `ResearchCurrency`, `CraftCurrency`,
    // `TradeCurrency`.
    //
    // `Currency` is the salvage the build economy runs on (build costs and
    // recipes); `TradeCurrency` is what traders pay and charge. They are
    // deliberately different items — no trader deals in `Currency`, and
    // `TradeCurrency` is the only one that survives a zone breach.
    //
    // Exactly one item across the whole loaded set must claim each of these
    // four roles or the game refuses to start (see `ItemDb::missing_roles`).
    // If two items claim the same role, the first one loaded keeps it and
    // the second is ignored with a warning — `.ron` files are read in
    // directory order, so don't rely on this to resolve a real conflict; fix
    // the duplicate instead.
    role: Some(Currency),

    // Optional; can be left out entirely (defaults to not equippable). If
    // set, this item can be worn in the given slot — one of `Weapon`,
    // `Armor`, `Module` — granting the paired stat bonus while equipped.
    // Stats are `atk`, `def`, `decompiler`, each optionally omitted (they
    // default to 0), and scale up with the wearer's gear level and with the
    // fusion tier of the *individual copy* worn — see
    // `EquipmentStats::scaled_for_level`/`fused_for_tier` and
    // `components::FusedGear`. Only equippable items can be fused.
    equipment: Some((Weapon, (atk: 4))),

    // Optional; can be left out entirely (defaults to not a catalyst). If
    // set, this item is a taming catalyst: a decompile attempt spends one
    // of it and it contributes this much to the taming roll — higher is
    // stronger. No item id is privileged here; a catalyst you drop in works
    // exactly like the shipped ICE Breaker does.
    //
    // Carrying several catalysts is fine. Each attempt resolves to the one
    // in inventory with the *highest* `taming_potency` and spends one of
    // that stack, so a stronger catalyst is never held back for a weaker
    // one. An exact tie resolves to whichever item id sorts first
    // alphabetically, so the same stack is always spent first.
    //
    // Carrying no catalyst at all makes decompiling unavailable: the action
    // is refused, and the odds readout in an intrusion and on the inspect
    // panel reads "needs a taming catalyst" rather than quoting a
    // percentage for an attempt that can't be made.
    taming_potency: Some(0.4),

    // Optional; can be left out entirely (defaults to no out-of-battle
    // effect). If set, this item can be used via `Game::use_item` outside
    // battle. All fields inside are optional (default 0/None) so one item
    // can restore several resources and/or arm a field buff:
    //   power  — restores this much Power
    //   fatigue — restores this much Fatigue (what the Stack's two movement
    //             routines are paid out of; it also regenerates on its own)
    //   heal   — restores this much HP
    //   prebattle_buff — arms a buff that keeps running on the map, through
    //     any battle that follows, and through a save — unlike a buff a
    //     companion's Special arms mid-fight, which is wiped the moment
    //     that battle ends. `kind` is one of `Regen`, `Coolant`, `Trickle`,
    //     `Def`, `Atk`, `Mitigation`, `CaptureBoost`, `XpBoost`,
    //     `EncounterDamp`, or `DropBoost`; `power` is its magnitude (flat
    //     for the stat kinds, percentage points for the rest); `ticks` is
    //     how many game ticks it lasts (ordinary turns, not battle rounds —
    //     it keeps counting down whether or not the player is in a fight).
    consume: Some((
        power: 25.0,
        fatigue: 10.0,
        heal: 5,
        prebattle_buff: Some((kind: Atk, power: 2, ticks: 30)),
    )),

    // `consume` is the *only* way `Game::use_item` spends an item, but it
    // isn't the only way an item gets spent at all — a mechanic elsewhere
    // can name an item id and take it directly. The Power Outlet is
    // craftable (see below) but has no `consume` block: it's spent by
    // `Game::rest`, priced on the rest-granting structure's own
    // `enables_rest.cost` (see assets/structures/README.md), not by the
    // player using it out of inventory.

    // Optional; can be left out entirely (defaults to not craftable). If
    // set, this item has a crafting recipe: `cost` is a list of (item id,
    // quantity) pairs the player must have in inventory to craft one unit.
    //
    // `requires_structure` is optional and defaults to none. Without it the
    // recipe is always available ("starter"), like the Power Cell and ICE
    // Breaker. Naming a structure id instead gates the recipe on one of
    // those being deployed — it only appears in the compile menu while the
    // bench stands, exactly like a researched recipe's own bench rule. The
    // bench is the entire unlock: an item-declared recipe needs no research
    // node of its own (though the bench itself may be research-gated to
    // build, which is what paces this).
    //
    // `cost` is ALSO what an automated machine runs. A structure whose
    // `assembles` field names this item (see assets/structures/README.md)
    // builds it from exactly this recipe — there is no second recipe format
    // anywhere in the game, so the bench and the machine can never drift
    // apart, and a multi-ingredient recipe you add here is automatable for
    // free. Naming that machine as `requires_structure` is the shipped
    // pattern: hand-crafting then reads as the manual fallback for a machine
    // you already own, rather than a way around building it.
    craftable: Some((
        cost: [("core_fragment", 12)],
        requires_structure: Some("fabricator"),
    )),

    // Optional; can be left out entirely (defaults to no drops). Species
    // that drop this item when defeated or decompiled, each with its own
    // 0.0-1.0 chance, rolled independently — so a kill can occasionally
    // yield two different pieces. Chances outside 0.0-1.0 are clamped; a
    // non-finite one disqualifies the whole file (see above).
    //
    // This is the inverse of a species file's `equipment_drop`, and the
    // preferred direction: one item names all of its sources, instead of
    // every species file naming the item. Both are still honoured and are
    // merged per kill — an item declared on both sides is rolled once, at
    // the better of the two chances.
    droppable: Some([("scrapper", 0.1), ("worm", 0.08)]),

    // Optional. Chance, 0.0-1.0, that a Stack cache holds one of these.
    // Caches sit in the dead ends of generated Stack frames; walking onto
    // one empties it, once, for good.
    //
    // Rolled once per cache for every item that declares this, so the
    // expected haul is the *sum* across the whole item set rather than a
    // pick from a list — adding a mod item with `cache_drop` makes caches
    // richer overall, it does not dilute what is already in them. Keep the
    // numbers low for that reason: the shipped set totals a little over one
    // item per cache.
    //
    // Currencies are handled separately and are not declared here — every
    // cache pays depth-scaled Credits and rolls for a Portal Fragment, from
    // constants in `tuning.rs`.
    cache_drop: Some(0.08),

)
```

There is **no per-ability item**. Routines used to be one item apiece, minted
at load; they are knowledge now (`resources::KnownRoutines`), and what a
player spends to write one into a slot is a blank **Routine Disk** —
`routine_disk`, an ordinary craftable item like any other in this directory.
See `../abilities/README.md`.

The filename doesn't matter to the loader (only the `id` field does), but
name it after the item for readability, e.g. `power_cell.ron`.

For the canonical list of shipped item ids and the rules governing the
four economy roles, see [Item ids](../../docs/manual.md#item-ids) and
[The four economy roles](../../docs/manual.md#the-four-economy-roles) in the
manual.
