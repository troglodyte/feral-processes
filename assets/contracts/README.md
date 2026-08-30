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

Offers come from two places: the authored contracts in this directory, and
whatever `templates/` rolls for this sector. Both arrive on the board as the
same thing — see [Templates](#templates) below.

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
    objective: Terminate(species: Some("drone"), count: 6),

    // What it pays, in full, once. One or more of the three below.
    reward: [Credits(40), Xp(120)],

    // Optional. The lowest sector a board may *offer* this in. Absent and 0
    // mean the same thing. It gates the offer only — breaching mid-contract
    // never strands one you already hold.
    min_zone: 0,

    // Optional, default false. Whether finishing it puts it back on the
    // board.
    repeatable: true,

    // Optional, default false. One of the jobs a new run is onboarded with.
    // See "Starters" below.
    starter: false,

    // Optional. Which step of the onboarding chain this is. See "The
    // tutorial chain" below. Absent on an ordinary contract.
    tutorial: Some(60),
)
```

`id`, `name`, `description`, `objective` and `reward` are required.

### `objective`

| Written as | Finished when |
|---|---|
| `Terminate(species: Some("drone"), count: 6)` | six of that species die |
| `Terminate(species: None, count: 12)` | twelve of anything die |
| `Deliver(item: "core_fragment", count: 25)` | you hand that many over at a Broker |
| `Descend(depth: 3)` | the party stands in a Stack frame 3 or more levels down |
| `Breach(zone: 3)` | the run reaches sector 3 or deeper |
| `Build(structure: "refinery")` | one of those is deployed |
| `Hold(item: "core_fragment", count: 12)` | you have that many in your pack at once |
| `Perform(deed: Examined)` | you do that particular thing once |

`Terminate` names a species id from `assets/species/`; `Deliver` an item id from
`assets/items/`; `Build` a structure id from `assets/structures/`. An id
naming nothing is not refused at load — no other database is in hand there —
so it costs you an unfinishable contract rather than a warning. The shipped
set is checked by a test; a mod is not.

`Deliver` is the one objective that needs you to go somewhere: items are handed
over at the Broker, and it takes only as many as the contract still needs.
Everything else is measured wherever you are, including four frames down.

`Perform` names a **deed**, which is a fixed list rather than an id from an
asset directory: `Examined`, `Tamed`, `TookFromContainer`,
`QueuedStandingOrder`, `UnlockedPerk`, `PostedStaff`. These are things the
engine emits, so unlike a species or an item they cannot be added by a mod,
and a name outside the list is refused by `ron` at load rather than costing
you a contract that never finishes.

A deed carries no parameters. `QueuedStandingOrder` does not say which item
and `PostedStaff` does not say which machine — the contract's `description`
is where the player is told, and putting it in both places is a copy that
drifts.

`Hold` is not `Deliver`. Nothing is handed over and nothing is spent, so it
needs no Broker and is met wherever you are, four frames down included. It is
also **latched**: once you have held the count, spending it does not
un-finish the contract.

Don't name a **banked** item (`ItemDef::banked` — Research Data is the shipped
one). A bank shares `Inventory` with cargo, so the hand-over quietly works,
but the player is never shown the row: every cargo screen omits it, so they
are asked for something they cannot see themselves holding, and paying it
spends research progress rather than stock. The shipped set is checked by
`a_shipped_delivery_never_asks_for_the_bank`; a mod is not.

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

## Starters

A contract marked `starter: true` takes a board slot **ahead of everything
else** while it is unfinished. The board is three slots drawn uniformly out of
everything eligible, and a fresh sector has a dozen or more eligible contracts —
so without this a new player's first job is a coin flip, and "deliver
twenty-five Core Fragments" is as likely to be it as "kill three programs".
`min_zone: 0` cannot stand in for it: it says a contract *may* be offered, not
that it is offered first.

The queue belongs to the **first sector only**. Past zone 1 a starter is still
offered, just no longer ahead of anything — breaching is where onboarding ends,
and a Broker four sectors out leading with a beginner's errand reads as broken
rather than as helpful. Finishing one takes it off the board like any other
one-shot, so mark starters `repeatable: false` or they hold a slot for the whole
run.

Templates have no `starter` field, so a rolled contract is never one: the arc is
a written sequence, not whatever this sector happened to supply. The shipped set
ships seven, all `min_zone: 0` — a first kill, a first delivery, three
first structures, a first descent and the breach out of sector 1.

## The tutorial chain

A contract carrying a `tutorial` step is an **onboarding mission**. The chain
is every such contract in step order, and it behaves unlike everything else
here:

- It is **handed to the player, never offered**. It appears under *Held* at
  the start of a run, with no Broker standing and no key pressed. There is
  nothing to accept and nothing to decline.
- **One is live at a time.** Finishing one hands out the next in the same
  tick.
- It **does not count against `MAX_ACTIVE_CONTRACTS`**, and it cannot be
  given back with `[A]`.
- While the chain is unfinished the **ordinary board is empty**. When the
  last mission completes the board fills normally, starters first.
- It draws **green** on the contracts screen.

The number is a *step*, not an index. The shipped missions are spaced 10
apart so a mission inserted later never renumbers the others. Two files
claiming one step is refused at load, as is `tutorial` beside `starter` or
`repeatable`. `min_zone` on a tutorial mission is inert.

The chain runs on new games only. A save made before this existed has every
mission filed as finished at load.

**A mission must be finishable, or onboarding stops for the rest of the
run.** The shipped chain is held to that by three tests over the real assets
in `crates/engine/src/tests/assets.rs`: its build costs never outrun its
payouts, every `Deed` it names has an emit site, and every id it names
resolves. A mod is not checked, which is the line every content directory
here draws.

## Templates

A file in `templates/` is a contract with **holes in it**, rolled into a real
contract against whatever this sector can supply. Every board mixes rolled
offers in with the authored ones, and nothing downstream can tell them apart:
a rolled contract is accepted, progressed, finished and paid by exactly the
code an authored one is. An authored contract is simply a template with no
free variables.

The directory is optional. With no `templates/` at all you get the authored
set and nothing else, which is exactly what the game was before templates
existed.

```ron
(
    // A prefix, not an id. The contract this rolls gets an id built from it
    // and the roll: "hunt#drone-6". An authored contract may not contain '#'
    // for this reason.
    id: "hunt",

    // {target} becomes the rolled species, item or structure's display name.
    // {count} becomes the number rolled. Both are optional.
    name: "Hunt: {target}",
    description: "{count} {target} have been logged running loose within sight of your anchor. Shut them down.",

    // What varies. One of the five below.
    objective: Terminate(count: (5, 12)),

    // Paid PER UNIT of what the contract asks for, then multiplied up. See
    // below — this is the one field that does not mean what it means in an
    // authored file.
    reward: [Credits(7), Xp(20)],

    // Both optional, and both mean what they do on an authored contract.
    min_zone: 0,
    repeatable: true,
)
```

### `objective`

| Written as | Rolls |
|---|---|
| `Terminate(count: (5, 12))` | a species that lives near your base, and how many |
| `Deliver(count: (8, 20))` | an item this sector can supply, and how many |
| `Descend(depth: (2, 5))` | how far down a stack |
| `Breach(zone: (2, 6))` | which sector to reach |
| `Build` | a structure you have unlocked and do not already have |

Ranges are inclusive, `(low, high)`.

A template rolls **nothing at all** when the sector can supply nothing valid,
and its slot simply goes to something else. That is the whole point: a
contract naming a program that does not live here, or an item nothing here
produces, is unfinishable, and an empty slot is better. Two of the rules are
about contracts that would finish the *instant* you accept them — a rolled
`Breach` always names a sector deeper than the one you are in, and a rolled
`Descend` never names depth 0.

What a rolled `Deliver` may ask for is deliberately narrow: **bulk stock
only**. The item has to be a material (not something worn, drunk or spent as
currency) worth no more than `CONTRACT_MAX_DELIVER_VALUE`, because a delivery
is asked for by the score and twenty of anything past the scavenged band is a
run's worth of work stated as an errand. Portal Fragments are excluded by
that rule, which matters: they are the breaching currency and the only source
of them is a boss at the bottom of a stack.

Which programs count as living near your base is read from the Chebyshev ring
just outside your **anchor** — the door the base is reached through, and the
only thing it still has on the zone surface now that the base itself is out of
phase. The base's own floor is `Platform`, which nothing lives on and which is
in another space entirely. A run with no anchor is offered no `Terminate`, but
can still be asked to deliver or to build; neither is a question about the
ground.

### `reward` on a template

A template's reward is **per unit of what it asks for**, not the total. A
`Terminate(count: (5, 12))` paying `Xp(20)` pays 100 XP if it rolls 5 and 240
if it rolls 12. The three objectives that are not counted — `Descend`,
`Breach` and `Build` — ask for one thing, so they pay the figure as written.

This is the same `target()` rule the rest of the engine uses to decide when a
contract is finished, so there is no second notion of size to keep in step.

### What is refused at load

As well as anything `ron` itself rejects: an empty id, an id containing `#`, a
duplicate id, an empty `reward` or one paying `0`, a back-to-front range like
`(9, 4)`, and a `{target}` hole on a `Descend` or `Breach` — those roll no
species, item or structure, so there would be nothing to put in it and the
player would read the literal text.

## Deleting a file

A contract with no file here stops being offered. It is not an error, and it is
not retroactive: one already accepted still finishes and still pays, because
the run holds its own copy. An id in the run's completed list is simply
ignored. Delete every file here and the board is empty, which is exactly what
the game looked like before contracts existed.
