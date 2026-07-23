# Custom items (mods)

Drop a `.ron` file in this directory and it's picked up automatically the
next time a game session starts — no recompiling required. A malformed file
is skipped with a warning logged in-game rather than crashing startup.

## Schema

```ron
(
    id: "unique_snake_case_id",   // must be unique across all item files
    name: "Display Name",

    // Optional; can be left out entirely (defaults to no bank limit). If
    // set, this item is exempt from the shared inventory capacity and
    // capped only by this ceiling instead — Research Data does this so an
    // unrelated pile of cargo can't starve a Research Node's output. Leave
    // it out for ordinary cargo, which counts against
    // `Game::inventory_capacity`.
    bank_limit: Some(200),

    // Optional; can be left out entirely (defaults to no economy role). If
    // set, this item is the game's singleton anchor for that role — engine
    // logic looks up "the item with role X" rather than naming an id, so
    // swapping which item is the currency is a data change, not a code
    // change. One of: `Currency`, `ResearchCurrency`, `CraftCurrency`.
    //
    // Exactly one item across the whole loaded set must claim each of these
    // three roles or the game refuses to start (see `ItemDb::missing_roles`).
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

    // Optional; can be left out entirely (defaults to not usable for
    // taming). If set, using this item during a taming attempt contributes
    // this much to the taming roll — higher is stronger.
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
    // set, this item has an always-available ("starter") crafting recipe:
    // a list of (item id, quantity) pairs the player must have in
    // inventory to craft one unit of this item.
    craftable: Some((cost: [("core_fragment", 2)])),
)
```

The filename doesn't matter to the loader (only the `id` field does), but
name it after the item for readability, e.g. `power_cell.ron`.

## The shipped items

The eleven items the base game ships. Other schema docs point here for the
canonical id list, and mods that predate the data-driven item model named
these in PascalCase — the second column is what to replace those with.

| Old name | Id | What it is |
| --- | --- | --- |
| `CoreFragment` | `core_fragment` | `Currency` |
| `PowerCell` | `power_cell` | Consumable, restores Power |
| `IceBreaker` | `ice_breaker` | Taming catalyst |
| `PortalFragment` | `portal_fragment` | `CraftCurrency` |
| `ResearchData` | `research_data` | `ResearchCurrency`, banked |
| `OverclockCore` | `overclock_core` | Weapon |
| `MonofilamentWhip` | `monofilament_whip` | Weapon |
| `FirewallPlating` | `firewall_plating` | Armor |
| `AblativePlating` | `ablative_plating` | Armor |
| `NeuralAmplifier` | `neural_amplifier` | Module |
| `CortexHack` | `cortex_hack` | Module |

Nothing privileges these over an item you add — they're ordinary `.ron`
files in this directory, and any of them can be edited or removed (subject
to the role rule below).

## The three economy roles

The game needs exactly one item holding each of `Currency`,
`ResearchCurrency`, and `CraftCurrency` to start — these are the anchors
every trade, research spend, and zone-portal cost reads through instead of
naming a hardcoded item. Removing (or renaming without re-tagging) the item
that holds a role, with nothing else claiming it, leaves the economy
incomplete and the game won't start; see `ItemDb::missing_roles`.
