# Contracts (mods)

Drop a `.ron` file in this directory and it's picked up automatically the next
time a game session starts — no recompiling required. A malformed file is
skipped with a warning logged in-game rather than crashing startup.

Like `assets/achievements/`, this **is** a content directory: a new contract is
a new file, not a new enum variant. The five `objective` shapes and three
`reward` shapes below are the whole vocabulary, and every combination of them
already works.

## What a contract is

A contract is a named, finite objective with a payout. They are issued by a
**Contract Broker**, a structure you build; walk up to one and the contracts
screen lists what it is offering alongside what you already hold.

What a Broker offers is **derived, not stored**: a seeded roll off the world
seed, the sector and the current epoch (`tick / CONTRACT_REFRESH_CYCLES`). So
the same board comes back after a save and load, cannot be rerolled by
reloading, and rotates on its own as the run proceeds. A contract already
accepted or already finished is filtered out of the offers.

Accepting one copies the **whole definition** into your run. Editing or
deleting a file mid-run therefore cannot strand or silently rewrite a contract
you have already taken — it finishes and pays exactly as it read when you
accepted it.

You may hold `MAX_ACTIVE_CONTRACTS` at once (3 as shipped). Abandoning one
loses its progress; it is not banked.

## Schema

```ron
(
    // Unique across every file here. Also what the save records as done, so
    // renaming an id makes a finished contract look unfinished. Treat it as
    // permanent once anyone has played with it.
    id: "clear_the_nursery",

    // The row on the contracts screen.
    name: "Clear the Nursery",

    // The line under the name, and the only place a player is told what to
    // do. The engine never derives it from the objective, so a retuned count
    // leaves the wording stale until you edit it.
    description: "Drones have settled in the shallows around the base. Thin them out before they multiply.",

    // What finishes it. Exactly one of the five below.
    objective: Kill(species: Some("drone"), count: 6),

    // What it pays, in full, once. One or more of the three below.
    reward: [Credits(40), Xp(120)],

    // Optional. The lowest sector a board may *offer* this in. Absent and 0
    // mean the same thing. It gates the offer only — breaching mid-contract
    // never strands one you already hold.
    min_zone: 0,

    // Optional, default false. Whether finishing it puts it back on the
    // board.
    repeatable: true,
)
```

`id`, `name`, `description`, `objective` and `reward` are required.

### `objective`

| Written as | Finished when |
|---|---|
| `Kill(species: Some("drone"), count: 6)` | six of that species die |
| `Kill(species: None, count: 12)` | twelve of anything die |
| `Deliver(item: "core_fragment", count: 25)` | you hand that many over at a Broker |
| `Descend(depth: 3)` | the party stands in a Stack frame 3 or more levels down |
| `Breach(zone: 3)` | the run reaches sector 3 or deeper |
| `Build(structure: "refinery")` | one of those is deployed |

`Kill` names a species id from `assets/species/`; `Deliver` an item id from
`assets/items/`; `Build` a structure id from `assets/structures/`. An id
naming nothing is not refused at load — no other database is in hand there —
so it costs you an unfinishable contract rather than a warning. The shipped
set is checked by a test; a mod is not.

`Deliver` is the one objective that needs you to go somewhere: items are handed
over at the Broker, and it takes only as many as the contract still needs.
Everything else is measured wherever you are, including four frames down.

There is deliberately no `Tame` objective: a program joins the roster through
two doors rather than one, and neither is as cleanly funnelled as a kill.

### `reward`

| Written as | Pays |
|---|---|
| `Credits(40)` | 40 Credits |
| `Item("power_cell", 5)` | 5 of that item |
| `Xp(120)` | 120 XP to the player |

A contract may pay any mix, and the list is paid in full on completion.

**Gear rewards are always plain `Ordinary` copies.** A contract payout does not
roll rarity or an affix — that is reserved for gear you *find*, which is the
whole reason to go looking rather than shopping. Crafting and buying are
already excluded the same way.

**Portal Fragments are not a contract reward.** There is no
`Reward::PortalFragments` variant, and paying them through `Reward::Item` is
refused by the shipped-asset test. Fragments are the breaching currency and
stay earned by fighting and descending; a route from base production straight
to breaching would close the loop the game is built around. Contracts *do*
deliberately pay XP, including on delivery and construction objectives — that
much is an intended amendment, so what advances you is the thing the game asked
for rather than whatever was nearest.

A reward of `0` (`Credits(0)`, `Item(_, 0)`, `Xp(0)`), or an empty `reward`
list, is skipped with a warning, the same way a zero-paying achievement is: a
contract that pays nothing is a mistake that reads as a working file.

## Deleting a file

A contract with no file here stops being offered. It is not an error, and it is
not retroactive: one already accepted still finishes and still pays, because
the run holds its own copy. An id in the run's completed list is simply
ignored. Delete every file here and the board is empty, which is exactly what
the game looked like before contracts existed.
