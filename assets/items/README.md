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

    // Optional; can be left out entirely (defaults to no bank limit). The
    // ordinary Buffer (cargo) is unbounded; setting this makes an item a
    // banked currency instead, capped only by this ceiling — Research Data
    // does this so its own stockpile has a hard limit separate from cargo.
    // Leave it out for ordinary cargo, which is never capped.
    bank_limit: Some(200),

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
    // default to 0), and scale up with the wearer's gear level and any
    // fusion tier — see `EquipmentStats::scaled_for_level`/`fused_for_tier`.
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
    // can restore several resources and/or arm a pre-battle buff:
    //   power  — restores this much Power
    //   fatigue — restores this much (reduces Fatigue by this much)
    //   heal   — restores this much HP
    //   prebattle_buff — arms a buff that survives on the map and applies
    //     during the player's next intrusion (buffs only tick in battle);
    //     `kind` is one of `Atk`, `Def`, `power` is the flat bonus, and
    //     `rounds` is how many battle rounds it lasts.
    consume: Some((
        power: 25.0,
        fatigue: 10.0,
        heal: 5,
        prebattle_buff: Some((kind: Atk, power: 2, rounds: 3)),
    )),

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

    // Reserved for engine-synthesized items — leave this out. Every loaded
    // ability automatically gets a "<Ability Name> Routine" item (id
    // `routine_<ability_id>`) whose `routine` names that ability and whose
    // description is read live from the ability's own text, so it can never
    // drift. Authoring an item whose id collides with `routine_<ability>`
    // is refused with a warning (the authored file wins, but the ability
    // becomes unextractable) — don't claim that id namespace by hand.
    routine: None,
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the item for readability, e.g. `power_cell.ron`.

For the canonical list of shipped item ids and the rules governing the
four economy roles, see [Item ids](../../docs/manual.md#item-ids) and
[The four economy roles](../../docs/manual.md#the-four-economy-roles) in the
manual.
