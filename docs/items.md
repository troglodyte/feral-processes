# Item catalogue

Every shipped item in feral-processes, charted from its own file in
`assets/items/`. Forty-six of them.

**These numbers are a transcription, not a read.** They were copied out of
`assets/items/*.ron` on 2026-08-05 and will drift the moment one of those
files is edited; regenerate the page rather than trusting it blind.

`ItemId` is a string newtype rather than an enum, so a new item never requires
touching Rust — shipped ones stay reachable from code through the `ids` module
for test setup and data-defined recipes, and nothing else needs them.

| | |
|---|---|
| items | 46 |
| equipment | 31 across 3 slots |
| craftable | 34 |
| need a bench | 25 |
| drop from a kill | 31 |
| species that drop anything | 17 |

## The value ladder

An item's `value` is the one place a price is decided — selling, buying back
and the trade screen all read it, and a trader's `sell_rate` is a multiplier
on it rather than a price of its own. So the ladder below is the economy.

```
THE VALUE LADDER

printable     1-2     5 items   a base can make it out of nothing
scavenged     3-8    14 items   salvage and intermediates
standard     12-16   11 items   the craftable working set
researched   20-60    6 items   needs a node and a bench
premium      80-120   9 items   portal_fragment gear

unpriced: Credits (TradeCurrency)
```

One item sits outside the ladder entirely, and correctly: **Credits** carry no
`value` at all, because they are what a price is denominated in rather than
something with a price. Core Fragments are the opposite case — a currency that
*is* priced, at the floor, because a base prints them.

The bands are a design statement, not an observation: worth comes from what a
base **cannot** manufacture. That is why the printable end is worth 1-2 and
the premium end is gated behind `portal_fragment`, which comes off exactly one
thing in the game: the guardian in a Stack lair, at `4..=8` per frame of
depth. Nothing on the surface pays it.

That makes the top of the ladder reachable two ways, and they are the two
kinds of boss. Craft it, spending fragments carried up out of a stack — or
take it off a boss killed on the surface, which pays gear instead of
fragments, drawn from a band of this ladder that climbs with the zone. A
zone-1 surface boss hands out the standard and researched tiers; by zone 4 it
is dropping the premium end whole.

Two bounds hold it in place, and the second is the one that is not obvious. A
craftable worth more than its ingredients is an infinite Credit loop — that
much is guessable, and build salvage is deliberately sellable while a Mining
Node produces it forever. But a `work.produces` structure makes its item out
of *nothing* on a timer, so **that item's value is really a Credit-per-tick
rate** the recipe ceiling cannot see. Both are asserted over the real assets
by `no_craftable_item_is_worth_more_than_its_ingredients` and
`every_base_produced_item_sits_at_the_floor_price`, so a retune that breaks
one fails rather than quietly minting money.

## Equipment

```
EQUIPMENT BY SLOT

Weapon (10)
  Black ICE Pick          90  ####################......  atk+3 decompiler+2
  Siege Compiler          90  ####################......  atk+3 def+2
  Plasma Router           80  #################.........  atk+4
  Monofilament Whip       60  #############.............  atk+4
  Overclock Core          22  #####.....................  atk+3
  Recursion Blade         14  ###.......................  atk+2 def+1
  Shim Blade              14  ###.......................  atk+2 decompiler+1
  Arc Lance               12  ###.......................  atk+3
  Kinetic Edge             7  ##........................  atk+2
  Shiv Routine             4  #.........................  atk+1

Armor (10)
  Nullsteel Plate         90  ####################......  def+3 decompiler+2
  Phase Carapace          90  ####################......  def+3 atk+2
  Bastion Lattice         80  #################.........  def+4
  Ablative Plating        25  #####.....................  def+4
  Firewall Plating        20  ####......................  def+3
  Null Weave              14  ###.......................  def+2 atk+1
  Static Mesh             14  ###.......................  def+2 decompiler+1
  Hardened Shell          12  ###.......................  def+3
  Packet Buffer            7  ##........................  def+2
  Scrap Ward               4  #.........................  def+1

Module (11)
  Singularity Matrix     120  ##########################  atk+3 def+3 decompiler+3
  Oracle Core             90  ####################......  decompiler+3 atk+2
  Kernel Key              80  #################.........  decompiler+4
  Cortex Hack             25  #####.....................  decompiler+3
  Neural Amplifier        20  ####......................  decompiler+2
  Sync Governor           16  ###.......................  atk+1 def+1 decompiler+1
  Entropy Damper          15  ###.......................  decompiler+2 def+1
  Logic Probe             15  ###.......................  decompiler+2 atk+1
  Trace Sniffer           13  ###.......................  decompiler+3
  Handshake Forge          8  ##........................  decompiler+2
  Probe Service            5  #.........................  decompiler+1
```

A worn item and a candidate to replace it are measured at two **different**
levels, and that is the point rather than a bug: gear locks in the zone level
it was equipped at and doubles per level, so the equip screen measures the
worn copy at its recorded level and every candidate at the current zone's.
Collapsing those to one level would hide the case the screen exists for — a
spare copy of the weapon already on your back is a real upgrade after a
breach.

## Recipes

| Item | Value | Recipe | Bench |
|:---|---:|:---|:---|
| ICE Breaker | 1 | 3 core_fragment | hand |
| Power Cell | 1 | 2 core_fragment | hand |
| Scrap Ward | 4 | 4 core_fragment | hand |
| Shiv Routine | 4 | 4 core_fragment | hand |
| Power Outlet | 5 | 5 core_fragment | hand |
| Probe Service | 5 | 5 core_fragment | hand |
| Kinetic Edge | 7 | 7 core_fragment | hand |
| Packet Buffer | 7 | 7 core_fragment | hand |
| Handshake Forge | 8 | 8 core_fragment | hand |
| Hardened Shell | 12 | 3 bytecode_block | `armory` |
| Null Weave | 14 | 3 bytecode_block, 1 charge_coil | `armory` |
| Static Mesh | 14 | 2 bytecode_block, 1 charge_coil, 1 logic_wafer | `armory` |
| Bastion Lattice | 80 | 12 portal_fragment, 4 bytecode_block, 2 charge_coil | `armory` |
| Nullsteel Plate | 90 | 14 portal_fragment, 3 bytecode_block, 1 charge_coil, 2 logic_wafer | `armory` |
| Phase Carapace | 90 | 14 portal_fragment, 5 bytecode_block, 1 charge_coil | `armory` |
| Patch Routine | 7 | 3 charge_coil | `assembly_bay` |
| Routine Disk | 5 | 2 blank_substrate | `disk_press` |
| Arc Lance | 12 | 12 core_fragment | `fabricator` |
| Trace Sniffer | 13 | 5 logic_wafer | `fabricator` |
| Recursion Blade | 14 | 14 core_fragment | `fabricator` |
| Shim Blade | 14 | 14 core_fragment | `fabricator` |
| Entropy Damper | 15 | 2 logic_wafer, 3 charge_coil | `fabricator` |
| Logic Probe | 15 | 3 logic_wafer, 1 charge_coil, 1 bytecode_block | `fabricator` |
| Sync Governor | 16 | 2 logic_wafer, 2 charge_coil, 1 bytecode_block | `fabricator` |
| Kernel Key | 80 | 12 portal_fragment, 5 logic_wafer, 2 charge_coil | `fabricator` |
| Plasma Router | 80 | 16 portal_fragment | `fabricator` |
| Black ICE Pick | 90 | 18 portal_fragment | `fabricator` |
| Oracle Core | 90 | 14 portal_fragment, 4 logic_wafer, 1 charge_coil, 2 bytecode_block | `fabricator` |
| Siege Compiler | 90 | 18 portal_fragment | `fabricator` |
| Singularity Matrix | 120 | 20 portal_fragment, 3 logic_wafer, 3 charge_coil, 2 bytecode_block | `fabricator` |
| Blank Substrate | 3 | 4 core_fragment | `lathe` |
| Bytecode Block | 4 | 4 core_fragment | `refinery` |
| Logic Wafer | 3 | 4 raw_trace | `transcriber` |
| Charge Coil | 3 | 3 power_cell | `winding_node` |

25 of the 46 items name a bench, and the nine assembler
recipes in the game are exactly the products of the nine machines — because a
machine runs its product's own `craftable.cost`, there is no second recipe on
the structure that could drift from the bench recipe, and every craftable a
mod adds is automatable for free.

## Drops

```
WHO DROPS WHAT

proxy        Neural Amplifier 30%, Trace Sniffer 10%, Null Weave 9%, Entropy Damper 7%, Recursion Blade 7%
trojan       Overclock Core 20%, Recursion Blade 10%, Logic Probe 8%, Null Weave 7%, Sync Governor 6%
rootkit      Ablative Plating 30%, Bastion Lattice 6%, Black ICE Pick 6%, Oracle Core 5%
scrapper     Overclock Core 15%, Arc Lance 10%, Logic Probe 7%, Shim Blade 7%
worm         Shim Blade 9%, Arc Lance 8%, Static Mesh 7%, Sync Governor 7%
sentinel     Firewall Plating 35%, Bastion Lattice 8%, Hardened Shell 8%, Siege Compiler 6%
cipher       Cortex Hack 35%, Kernel Key 8%, Black ICE Pick 7%, Nullsteel Plate 6%
crawler      Firewall Plating 20%, Hardened Shell 10%, Entropy Damper 8%, Static Mesh 8%
virus        Neural Amplifier 25%, Kernel Key 6%, Phase Carapace 6%, Plasma Router 6%
zero_day     Neural Amplifier 30%, Phase Carapace 7%, Trace Sniffer 7%, Nullsteel Plate 6%
drone        Packet Buffer 10%, Handshake Forge 9%, Shiv Routine 8%
sub_process  Kinetic Edge 10%, Packet Buffer 8%, Handshake Forge 7%
glitch       Scrap Ward 10%, Kinetic Edge 8%, Probe Service 7%
overseer     Neural Amplifier 50%, Oracle Core 12%, Singularity Matrix 8%
construct    Overclock Core 30%, Plasma Router 8%, Siege Compiler 7%
sprite       Shiv Routine 10%, Probe Service 9%, Scrap Ward 8%
wintermute   Monofilament Whip 60%, Singularity Matrix 15%
```

A drop table is declared on the **item**, naming the species that pay it out,
rather than on the species naming its loot. That inversion is what lets a mod
add an item that drops from creatures it did not write, without editing
anyone else's species file.

---

Source of truth is `assets/items/`. A mod that drops a `.ron` file in that
directory joins the catalogue without a recompile, and will not appear above
until this page is regenerated -- edit the table at the top of
[`docs/items-gen.py`](items-gen.py) and run `python3 docs/items-gen.py` from
the repo root. The schema is documented in
[`assets/items/README.md`](../assets/items/README.md).
