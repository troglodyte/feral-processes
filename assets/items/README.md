# Custom items (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup. That
includes a file whose numbers aren't finite: RON accepts bare `NaN` and
`inf` literals, and they'd otherwise slip past every clamp downstream, so
any non-finite `taming_potency`, `consume.power`, or
`upgrade` percentage disqualifies the whole file.

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

    // Optional; almost always leave it out. The short tag this item is
    // listed under on the base stock strip, the one-row readout of what the
    // base's machines and depots are holding. Left out, it is derived from
    // `name`: the initials of its words, or the first two letters of a
    // one-word name, uppercased and capped at two characters — "Core
    // Fragment" becomes `CF`.
    //
    // Author one only to settle a collision. The strip carries the tag and
    // the quantity and nothing else, so two items tagging the same is a
    // readout that lies about which pile is filling. Only items that reach
    // a machine or depot buffer are listed, so a weapon or a consumable
    // never needs one. One or two characters; nothing refuses a modded
    // collision — the shipped set is held unique by its own census, and
    // yours is yours to settle.
    abbrev: Some("R"),

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
    // Stats are `atk`, `mitigation`, `decompiler`, `damage`, `accuracy` and
    // `evasion`, each optionally omitted (they default to 0), and all six
    // scale up with the wearer's gear level, with the fusion tier of the
    // *individual copy* worn, and with that copy's rare tier — see
    // `EquipmentStats::scaled_for_level`/`fused_for_tier`/`for_rarity` and
    // `components::GearCopies`. Only equippable items can be fused, and only
    // they roll a rare tier.
    //
    //   atk         flat damage added to every landed attack
    //   mitigation  **percentage points** of damage reduction. This was
    //               `def` and meant points of absorption; the name and the
    //               unit changed together. Everything that reduces damage is
    //               summed and then capped at 75, so a single piece near
    //               that ceiling wastes most of itself.
    //   damage      `(min: N, max: M)`, a weapon's damage band, rolled
    //               uniformly and inclusive at both ends. It **overrides**
    //               the wearer's natural attack rather than adding to it, so
    //               a Weapon that omits it silently disarms whoever wears
    //               it — the shipped censuses hold every Weapon to having
    //               one and everything else to having none. `atk` is added
    //               on top of whatever the band rolls.
    //   accuracy    raises the odds an attack lands, against the defender's
    //               evasion
    //   evasion     lowers the odds an attack lands on the wearer
    //
    // The two defensive axes are meant to be a real choice: heavy armour
    // buys `mitigation` and takes every hit smaller, light armour buys
    // `evasion` and takes fewer hits at full size. Weapons trade the same
    // way between a wide `damage` band and `accuracy`. Neither pairing is
    // enforced — it is simply what the shipped roster does, and what makes
    // the fields worth authoring.
    //
    // The wearer is the player *or* any program they own — one copy is
    // interchangeable, and every copy comes out of and returns to the
    // player's own cargo whoever is wearing it. One consequence worth
    // knowing before pricing a module: `decompiler` does nothing at all on
    // a program, since only the player ever attempts a capture. Such an
    // item is still worn, and still worth its `value`; it is simply inert
    // in that slot.
    equipment: Some((Weapon, (atk: 4, damage: (min: 7, max: 13)))),

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
    //   power  — restores this much Power (the one need; it drains on its
    //            own and is what every routine call is paid out of)
    //   heal   — restores this much HP
    //   prebattle_buff — arms a buff that keeps running on the map, through
    //     any battle that follows, and through a save — unlike a buff a
    //     companion's Special arms mid-fight, which is wiped the moment
    //     that battle ends. `kind` is one of `Regen`, `Trickle`,
    //     `Def`, `Atk`, `Mitigation`, `CaptureBoost`, `XpBoost`,
    //     `EncounterDamp`, or `DropBoost`; `power` is its magnitude (flat
    //     for the stat kinds, percentage points for the rest); `ticks` is
    //     how many game ticks it lasts (ordinary turns, not battle rounds —
    //     it keeps counting down whether or not the player is in a fight);
    //     `interval` is how many ticks pass between firings, defaulting to
    //     1 (every tick).
    //
    //     **An over-time kind needs an `interval` to be worth authoring.**
    //     `Regen` and `Trickle` fire on every tick they are eligible for, so
    //     `power: 3` with no interval is not a drip, it is a tap left
    //     running — twenty times the rate Power drains at. Set `interval` to
    //     the cadence you actually want and make `ticks` a whole multiple of
    //     it, or the last firing is silently short: the cadence is phased
    //     off the remaining count, not off a counter of its own.
    //
    //     **`ticks` always applies here, whatever `kind` you pick.** A
    //     *routine* arming most of these kinds runs until the party rests
    //     instead of counting down (see assets/abilities/README.md), but an
    //     item is spent when you use it and a routine can be run again on
    //     the next charge, so only the routine's half of that pair was ever
    //     meant to last a whole expedition. A consumable and a routine of the
    //     same kind stack rather than displacing each other, which is what
    //     would have made a permanent item buff compound with one.
    consume: Some((
        power: 25.0,
        heal: 5,
        prebattle_buff: Some((kind: Atk, power: 2, ticks: 30, interval: 1)),
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
    //
    // For an item with an `equipment` def, the named bench decides more than
    // whether the recipe is offered: each tier the player has upgraded it to
    // raises the quality floor every copy compiled there rolls off. Quality
    // is per-copy runtime state rather than an authored field, so there is
    // nothing to write here for it. A recipe naming no bench compiles at
    // the base floor, the same as one naming a bench nobody has upgraded.
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
    // numbers low for that reason: the shipped set totals about one item
    // per cache.
    //
    // Currencies are handled separately and are not declared here — every
    // cache pays depth-scaled Credits and rolls for a Portal Fragment, from
    // constants in `tuning.rs`.
    cache_drop: Some(0.08),

    // Optional; can be left out entirely (defaults to not being fuel). Makes
    // this item something a `power_upkeep` supplier will burn, and says how
    // many upkeep *windows* one unit buys. The staple Power Cell declares
    // `Some(1)`; a cell worth three windows declares `Some(3)`.
    //
    // Windows rather than ticks on purpose: how long one window lasts is a
    // difficulty knob and lives in `tuning.rs`, so an item file only ever
    // says how many of those a denser cell is worth. That keeps a mod from
    // retuning the base's fuel economy by editing an item.
    //
    // A supplier burns the *cheapest* fuel it can reach — lowest window
    // count first — so adding a denser tier never retires the one below it:
    // the base keeps eating the staple it can make in bulk, and the dense
    // one stays worth carrying into the field. A supplier is also gated on
    // its own declared fuel: if `power_upkeep` names an item that is not
    // grid fuel, that supplier burns nothing at all rather than falling back
    // to something that is.
    grid_fuel: Some(1),

    // Optional; can be left out entirely (defaults to no upgrade). What this
    // item does to one *tamed program* when applied from the party menu's
    // "Refactor a program" screen. It upgrades a companion permanently; it
    // never touches the player.
    //
    // The three percentages raise that stat by a percentage of its current
    // value, rounded, with a floor of +1 — so a +5% ATK buff still moves a
    // 3-ATK Drone, which is exactly the companion the feature exists to
    // rescue. Percentages rather than flat amounts because a companion's
    // numbers keep growing across breaches, and because they *commute* with
    // `zone_bump`: buying a buff now and bumping later lands on the same
    // stats as the reverse, so there is no ordering to exploit.
    //
    // `zone_bump: true` raises the program one zone tier, multiplying HP, ATK
    // and DEF by the game's per-zone growth. It is refused once the program
    // has caught up with the player's own zone, which is what bounds it.
    //
    // The two are independent and an item may declare both. An item with any
    // non-zero percentage spends one of the companion's bounded upgrade slots
    // (`MAX_COMPANION_REFACTORS` in `tuning.rs`); a pure `zone_bump` spends
    // none, because a player should never have to burn slots just staying
    // current with the zone they are standing in.
    //
    // Neither applies retroactively to a program's current HP as a heal — a
    // refactor raises current HP by exactly the amount it raised the maximum,
    // so it can't be used as a field patch mid-run.
    //
    // A `zone_bump` is *recorded* as well as applied, and traders divide
    // bought tiers back out of what they pay for a program (a tenth of its
    // power). So upgrading a program never raises its resale value: what a
    // trader pays for is what the program is, not what you spent on it.
    // Without that, a printable upgrade item plus a sale is a Credit press.
    // A percentage buff is not divided out — five slots is at most a 1.28x on
    // power, which never repays what it costs.
    //
    // Percentages must be zero or positive, and an `upgrade` declaring no
    // effect at all is refused with the rest of the malformed files: the
    // engine floors every gain at +1, so a negative percentage would become a
    // *raise* that also burned one of the five permanent slots.
    upgrade: Some((
        hp_percent: 5.0,
        atk_percent: 0.0,
        def_percent: 0.0,
        zone_bump: false,
    )),

    // Optional; can be left out entirely (defaults to no grant). A **passive
    // routine this item grants while it is worn**, by ability id — the same
    // ids `../abilities/` declares. The wearer fires it exactly as though it
    // were installed in one of their own slots, without spending one.
    //
    // Only useful on an item that also declares `equipment`, since nothing
    // else can be worn. Any owned program can wear gear, so a granted
    // passive is a companion's as readily as the player's.
    //
    // The named ability must exist and must be a **passive** — one declaring
    // `triggers`. A routine chosen on a turn has no trigger to fire on, so an
    // item naming one is skipped with a warning rather than worn as a
    // decoration that never runs.
    //
    // Nothing about a grant is saved. It is read off the worn item every time
    // its trigger comes round, so taking the item off ends it.
    grants: Some("watchdog"),

    // Optional; can be left out entirely (defaults to no reach). **How wide
    // one swing of this weapon is**, past the body it is aimed at. When the
    // charge is up, the swing lands on every body the target names — every
    // member of the group in front of a group, every body inside the shape
    // on a battle map. You attack the way you always attack; there is no
    // second key and no Power cost.
    //
    // `target` is the same vocabulary `../abilities/` uses, and only its two
    // plural enemy-facing values are legal: `WholeEnemyGroup` or
    // `AllEnemies`.
    //
    // `shape` is optional and overrides the geometry a battle map derives
    // from `target` — `WholeEnemyGroup` becomes a small blast and
    // `AllEnemies` a wider one. Author one only for real geometry: a `Line`
    // or a `Cone` is cast from the wielder toward the body being swung at,
    // where a `Radius` is centred on that body. Nothing shipped authors one.
    //
    // `recharge` is how many of the fight's own rounds pass before the swing
    // may be wide again. It is required, and shipped content never sets it
    // to 0 — a wide swing on every swing is a straight throughput
    // multiplier. `0` is legal and simply means "always ready".
    //
    // The reach is **breadth and never distance**: how far the weapon may be
    // swung from is `range:` below, and a reach weapon that authors no
    // `range` still swings at arm's length. On a battle map the sweep is
    // **full friendly fire** — a
    // companion standing beside the target is caught, and that is the price
    // of aiming a shape.
    //
    // Three faults are refused at load, and the file is skipped with a
    // warning like any other malformed one: a `reach` on an item that is not
    // a `Weapon`, a `target` that is ally-facing or names a single body
    // (`OneEnemyGroupFront`), and a `shape` of `Single`. The last two are
    // the same fault in two vocabularies — a reach that reaches nobody.
    //
    // Nothing about a reach is saved: it is read off the worn weapon's
    // definition every swing, so retuning this file reaches a run already in
    // progress.
    reach: Some((
        target: WholeEnemyGroup,
        recharge: 2,
    )),

    // Optional; can be left out entirely (defaults to arm's length). **How
    // far from the wielder this weapon may be swung**, in cells, on a
    // tactical battle map. Absent means one cell — an adjacent body, which
    // is what every weapon that does not author this reaches.
    //
    // `range:` is distance where `reach:` above is breadth, and the two are
    // independent: a single-target weapon may reach three cells, and a
    // sweeping one may swing only at arm's length. A swing fired from range
    // still sweeps — the shape is cast from the wielder toward the body
    // aimed at, whatever the distance between them.
    //
    // **Read on a battle map alone.** The group model has no geometry to
    // spend a distance on, so this changes nothing about a group fight; and
    // a swing at range needs line of sight, so cover stops one exactly as it
    // stops an aimed routine.
    //
    // A weapon's range **replaces** the wielder's own figure rather than
    // adding to it: a program whose species shoots two cells swings at arm's
    // length while it is holding a blade, because the weapon is what it is
    // swinging.
    //
    // Two faults are refused at load, and the file is skipped with a warning
    // like any other malformed one: a `range` on an item that is not a
    // `Weapon`, and a value outside `1..=3`. Refused rather than clamped — a
    // weapon quietly swinging shorter than its file says reads as a nerf
    // rather than a bad value.
    range: Some(2),

    // Optional; defaults to false. Marks this item as a **rest charge**: one
    // unit is spent by resting anywhere outside the player's base. Inside
    // the base a rest is free and spends nothing, so this is what makes
    // powering down possible out on the grid and down in the Stack.
    //
    // A flag rather than a price. A rest costs exactly one unit of the first
    // flagged item in the pack, so what a rest is worth is set through this
    // item's own `craftable.cost` rather than a quantity here. The shipped
    // rest charge is the Power Outlet (`outlet.ron`); an install can add
    // others, or ship none at all — with no flagged item anywhere in the
    // catalogue, resting simply stops working outside the base.
    enables_rest: true,

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

## Rare tiers are engine-rolled, not an item field

There is deliberately no `rarity` field, and adding one to a `.ron` file does
nothing. A rare tier belongs to a *copy*, not to an item: two Arc Lances can
differ, which is the whole point — see `items::GearCopy`.

The tier is rolled by `Game::grant_gear_drop` at the moment a copy drops, and
only for the three sources that drop gear: a defeated program's own drop
table, a surface boss's payout, and a Stack or nest cache. **Crafting and
buying never roll one**, so a made or purchased copy is always ordinary. That
asymmetry is the design rather than an oversight: found gear is meant to be
categorically better than made gear, which is what gives a player a reason to
go looking rather than shopping.

The chances and the multipliers live in `crates/engine/src/tuning.rs`
(`SILVER_SPAWN_CHANCE` and its siblings, `Rarity::stat_mult`,
`GEAR_RARITY_MIN_BONUS_PER_RUNG`) rather than here, for the same reason every
other difficulty knob does: content is moddable, how hard the game is, is not.
The one exception is a surface boss, which pays at or above
`SURFACE_BOSS_LOOT_RARITY_FLOOR` rather than rolling the bare ladder.

A mod that adds an equippable item gets all of this for free — its copies
roll the same tiers, scale by the same multipliers, and draw in the same
colours as a shipped one.
