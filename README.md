# feral-processes

A Neuromancer/Tron-flavored ASCII game blending Pokemon (tame and battle
rogue programs), Palworld (compiled programs work your base for you), and
Dwarf Fortress (procedural world, needs simulation, configurable permadeath).

Single-player, built in Rust as a terminal (TUI) app. The simulation is kept
decoupled from rendering so a client/server split is possible later.

## Installing

You need the Rust toolchain (Cargo). If you don't have it:

```sh
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
```

Then clone this repo and build it:

```sh
git clone <this-repo-url> feral-processes
cd feral-processes
cargo build
```

## Playing

Run it from the `tui` crate:

```sh
cargo run -p feral-processes
```

(or, from `crates/tui`, just `cargo run`)

This launches a full-screen terminal UI. From the main menu, start a **New
Game** and pick a difficulty:

- **Permadeath** — flatlining ends the run for good; a summary is appended
  to `run_history.log`.
- **Forgiving** — flatlining costs you (half Integrity, some Fatigue/Power
  restored) but you keep going, rebooting at the nearest deployed structure
  (or in place, if you haven't built anything yet).

Progress saves to `save.bin` in the repo root (`s` to save, `L` from the
main menu to reload).

### Controls

| Key | Action |
| --- | --- |
| `hjkl` / arrow keys | Move (bumping a rogue program starts an intrusion) |
| `.` | Wait in place (advances one tick) |
| `e` | Drain a Power Cell (restores Power) |
| `r` | Recharge overnight (restores Fatigue and Integrity, costs Power) |
| `g` | Scan the sector for Core Fragments |
| `c` | Open the compile menu (compile an ICE Breaker — 3 Core Fragments — a Power Cell — 2 Core Fragments — and any future recipes) |
| `b` | Deploy a structure |
| `w` | Assign a compiled program to a cronjob (work a structure) |
| `u` | Use symlink: instantly teleport to a deployed symlink structure (e.g. Home), for its item cost |
| `i` | Inspect: pick a direction, see stats/moves/decompile odds for the first program that way (no intrusion) |
| `v` | Inventory/equipment: equip, unequip, drop, destroy items |
| `p` | Manage your party: add/stand down nearby compiled programs (max 3) |
| `f` | Fuse two nearby compiled programs into one stronger one |
| `t` | Trade with a nearby Black Market: sell items, buy consumables |
| `x` | Perks: spend Perk Points on permanent passive unlocks |
| `s` | Save |
| `q` | Return to the main menu (unsaved progress is lost — `s` first if you want to keep it) |
| `+` / `-` | Zoom the grid in/out |
| `?` | In-game help / full control list |

**During an intrusion (battle):**

| Key | Action |
| --- | --- |
| `a` | Attack |
| `d` | Decompile (attempt to compile/tame the program — needs an ICE Breaker) |
| `c` | Command your active companion to attack (only shown if you have one) |
| `j` | Jack out (flee) |

### The loop

Explore the Grid, fight or decompile rogue programs you run into, and
deploy structures (build menu) to put compiled programs to work gathering
resources for you. Defeating or decompiling a program grants XP; compiled
programs also gain XP from completed work cycles. Leveling up grows stats
and fully restores Integrity.

### Getting started: building and running cronjobs

There's no ore vein or resource deposit hiding out in the map to stumble
onto — every workable node is something *you* build. Deploying always costs
materials; there's no free placement.

A **cronjob** is a compiled program assigned to a structure to produce
resources for you over time — it's the game's Palworld-style "put a tamed
creature to work" mechanic.

1. **Gather starting materials.** You spawn with 5 Core Fragments, 3 Power
   Cells, and 3 ICE Breakers — enough to bootstrap. Beyond that:
   - `g` (scan) has a biome-dependent chance to find a **Core Fragment**
     (60% Mainframe/OpenGrid, 30% NullSector, 15% StaticField, 0% in the
     unwalkable DataVoid/BlackIce biomes). It never yields Power Cells
     directly — compile those with `c` instead (see [Items](#items)).
   - Defeating or decompiling a **Virus** or **Construct** drops a **Core
     Fragment**.
   - Once you have a Mining Node running (see below), it's the sustainable
     source of Core Fragments — everything before that comes from starting
     inventory, scanning, or creature loot.
2. **Deploy a structure with `b`.** Pick one from the menu, then a direction
   to place it on an adjacent walkable tile. It's rejected if the tile isn't
   walkable, is already occupied, or you don't have enough of the required
   item (see the [Structures](#structures) table for costs — all paid in
   Core Fragments right now).
3. **Schedule a cronjob with `w`** — pick a compiled (tamed) program, then
   the structure to assign it to. This only works on structures with a
   `work` recipe (Mining Node, Power Conduit, Compiler, Fabricator);
   Terminal and Data Cache aren't assignable this way. Both pickers show
   status: the program picker flags `(active companion)` or
   `(on a cronjob: <structure>)`, and the structure picker flags
   `(assigned: <program>)`, so you can see who's already spoken for
   before reassigning them.
4. **Production runs automatically after that**, tick by tick, regardless of
   where you are or what you're doing:
   - Each tick, the assigned program's progress advances by 1.
   - Once progress reaches the structure's `ticks_per_unit` (Mining Node 5,
     Power Conduit 6, Compiler 8, Fabricator 12), one unit of output drops
     straight into *your* inventory, progress resets, and the worker gains
     5 flat XP (enough to level up mid-cycle sometimes).
   - Every structure you build starts with a **fixed reserve of 20 units**.
     Each completed cycle consumes one; once it hits 0 the structure goes
     idle and the assigned worker just sits there until reassigned.
   - Terminal works differently: it's **passive**, not cronjob-based — it
     auto-cooks a Core Fragment into a Power Cell every 15 ticks whenever
     you're standing within 2 tiles, no assignment needed.
5. **Cronjobs persist across save/load.** A program's assignment, its target
   structure, and its in-progress tick count are all saved — reload and it
   picks up right where it left off, no need to reassign it with `w`.

Once you have a Mining Node feeding a steady supply of Core Fragments, feed
that into a Power Conduit (Power Cells), a Compiler (ICE Breakers), a
Fabricator (Overclock Cores), and/or an Armory (Firewall Plating — see
[Equipment](#equipment)) to round out the loop.

### Stats

Shown in the status panel (always) and the intrusion screen (in battle):

| Stat | What it means |
| --- | --- |
| **Integrity** | Your HP. Hits 0 and you flatline — final in Permadeath, a costly soft-reboot in Forgiving mode. Leveling up or recharging overnight (`r`) both fully restore it. |
| **Power** | Your hunger-equivalent. Drains over time; hits 0 and you start taking Integrity damage each tick. Restored by draining a Power Cell (`e`) or standing near a cooking Terminal. |
| **Fatigue** | Drains over time; restored to full by recharging overnight (`r`). Currently cosmetic — doesn't yet penalize anything on its own, but rest also advances a lot of game time, so use it deliberately. |
| **Level / XP** | Grows from defeating or decompiling rogue programs, or (for a compiled program) completing cronjob cycles. Each level-up grows Attack/Defense/max Integrity, fully heals, and grants 1 Perk Point — see [Perks](#perks). |
| **Attack** | How hard your hits land. Battle damage is roughly `move power + attacker's Attack − defender's Defense` (always at least 1). |
| **Defense** | How much incoming damage you shrug off — see the Attack formula above. |
| **Decompiler** | Player-only skill at cracking ICE. Grows by 1 every time you level up (starts at 0). Adds a flat bonus to your decompile odds — see Decompile chance below. Tamed programs never have this stat; only you attempt decompiles. |
| **Decompile chance** | Shown live during an intrusion. Your odds of successfully compiling (taming) the program *this attempt*, given its remaining HP fraction, its species' difficulty, and your Decompiler stat — weakening it first, and leveling up over time, both raise your odds. Shown even without an ICE Breaker in hand, so you can decide whether it's worth going to compile one. |

### Perks

Every level-up grants 1 Perk Point (shown in the status panel and the `x`
menu). Spend them on permanent passive unlocks — each perk unlocks once and
stacks with your regular stats:

| Perk | Cost | Effect |
| --- | --- | --- |
| Keen Scavenger | 2 | +15 percentage points to scan (`g`)'s success chance |
| Low Power Mode | 2 | Power drains 30% slower |
| Exploit Focus | 3 | +5 effective Decompiler skill toward decompile odds |
| Lean Compiler | 3 | Compiling (`c`) costs 1 less of each required item (min 1 each) |

Perks are a small, fixed set of player-only progression choices rather than
moddable content — see `CLAUDE.md` for the distinction.

### Items

| Item | Source | Used for |
| --- | --- | --- |
| Core Fragment | Starting inventory; scan (`g`); dropped by Virus/Construct; a Mining Node cronjob | Deploy structures (2–6 each); compile an ICE Breaker (3 each) or a Power Cell (2 each) |
| Power Cell | Starting inventory; compiled (`c`) from 2 Core Fragments; dropped by Scrapper/Glitch; cooked passively at a Terminal; a Power Conduit cronjob | Drain (`e`) to restore Power |
| ICE Breaker | Starting inventory; compiled (`c`) from 3 Core Fragments; a Compiler cronjob | Attempt to decompile a rogue program in battle (`d`) |

A deliberately tight core-consumable economy: Core Fragment is the
universal raw material — found by scanning (`g`) or harvested passively via
a Mining Node — and the other two are refined from it (compiled with `c`,
scavenged from creatures, or produced by a structure cronjob) for one
specific purpose each. Equipment (below) is a separate, non-consumable item
category. Items aren't yet data-driven the way species and structures are —
see `CLAUDE.md` for the moddability note on adding a new one.

### Equipment

Press `v` to open the inventory/equipment screen from anywhere while
playing. It shows your stats, your three equipment slots, and your
inventory, each item numbered for selection.

| Item | Slot | Bonus | Source |
| --- | --- | --- | --- |
| Overclock Core | Weapon | +3 Attack | Fabricator cronjob; loot chance from Scrapper, Construct, Trojan |
| Monofilament Whip | Weapon | +4 Attack | Loot chance from Wintermute (boss) |
| Firewall Plating | Armor | +3 Defense | Armory cronjob; loot chance from Wraith, Sentinel |
| Ablative Plating | Armor | +4 Defense | Loot chance from Rootkit |
| Neural Amplifier | Module | +2 Decompiler | Loot chance from Virus, Phantom, Ghost |
| Cortex Hack | Module | +3 Decompiler | Loot chance from Cipher |

Each slot now has two options — a common one from an ordinary program, and
a tougher, slightly stronger one from a harder species or boss.

- **Equip**: select a numbered inventory item, then `[E]`. Equipping into an
  already-occupied slot swaps the old item back into your inventory — you
  can only ever have one item per slot.
- **Unequip**: press the number of an occupied slot (1 Weapon, 2 Armor, 3
  Module) directly from the main inventory screen.
- **Drop** / **Destroy**: select a numbered inventory item, then `[D]` or
  `[X]`. Both permanently remove the item — they're functionally identical,
  just distinct log wording; there's no way to recover a dropped item from
  the world.
- An equipped item's stat bonus is added the moment you equip it and
  removed the moment you unequip it — it shows up immediately in the status
  panel and the intrusion screen.

### Companions

Press `p` to open your party screen: up to **3** nearby compiled programs
can fight alongside you at once.

- Selecting a tamed program not already in the party adds it (rejected if
  the party's already full — stand one down first). Selecting a party
  member's own number stands it down. The party screen stays open so you
  can adjust multiple slots in one visit; `Esc` closes it.
- A party member is mutually exclusive with a cronjob: assigning it to work
  a structure (`w`) automatically stands it down from the party, and vice
  versa — a program is either working or fighting beside you, never both.
- During an intrusion, if you have at least one active companion the battle
  menu gains `[C]ommand companion`: with exactly one, it attacks
  immediately; with more than one, you're asked which party member acts.
  Either way, it's *instead of* you acting that round — a turn-economy
  tradeoff, not a free extra hit, and only one companion can act per round
  even with a full party.
- The wild program's retaliation has a 30% chance to target the party
  instead of you (picking uniformly among current members if you have
  more than one), using that member's Defense stat. A party member knocked
  to 0 HP stands down automatically — it isn't lost, just no longer active;
  re-add it (`p`) and recharge overnight (`r`) to heal it back up.
- The party/cronjob pickers show each candidate's status: `(in party)` or
  `(on a cronjob)`, so you can see at a glance who's free.
- Recharging overnight (`r`) fully heals every tamed program you own too,
  not just you — not just the active party, see [Base defense](#base-defense).

### Fusing programs

Press `f` to fuse two nearby compiled programs into one — pick the first,
then the second (anyone but the first). Both are consumed.

- The result's species (and so its moves/work aptitude) matches whichever
  input was the **higher level**; ties favor the first program you picked.
  It comes out at that same level, with 0 XP.
- Each stat (Integrity/Attack/Defense) is computed as the higher input's
  value plus half the lower one's, so a fusion is always stronger than
  either parent alone without simply adding them together — chain-fusing
  can't runaway to absurd numbers.
- There's no separate item cost: losing two programs to gain one stronger
  one *is* the cost, which makes it a good way to turn duplicate catches
  into a single keeper.
- A fused program isn't placed in your party or on a cronjob automatically
  — add it with `p` or assign it with `w` like any other compiled program.

### Current roster

| Program | Difficulty | Habitat | Works for |
| --- | --- | --- | --- |
| Sprite (`s`) | Easy | OpenGrid, Mainframe | — |
| Glitch (`g`) | Easy | OpenGrid, NullSector | Power Cells |
| Drone (`o`) | Easy | OpenGrid, Mainframe | Core Fragments |
| Daemon (`d`) | Easy/Medium | OpenGrid, NullSector | Power Cells |
| Scrapper (`x`) | Medium | OpenGrid, NullSector | Power Cells |
| Wraith (`w`) | Medium | StaticField | — |
| Phantom (`p`) | Medium | Mainframe, StaticField | — |
| Trojan (`t`) | Medium | Mainframe, OpenGrid | — |
| Worm (`m`) | Medium | NullSector, OpenGrid | Core Fragments |
| Virus (`v`) | Hard | NullSector, Mainframe | Core Fragments |
| Construct (`C`) | Hard | Mainframe | Core Fragments |
| Sentinel (`S`) | Hard | StaticField | — |
| Rootkit (`k`) | Hard | Mainframe, NullSector | — |
| Ghost (`h`) | Hard | StaticField, NullSector | — |
| Cipher (`c`) | Hard | Mainframe, StaticField | — |
| Overseer (`B`) — **boss** | Very Hard | OpenGrid, Mainframe, NullSector, StaticField | — |
| Wintermute (`W`) — **boss** | Very Hard | OpenGrid, Mainframe, NullSector, StaticField | — |

Scrapper, Wraith, Virus, Construct, Sentinel, Phantom, Trojan, Rootkit,
Ghost, Cipher, and Wintermute also each have a chance to drop a piece of
equipment on top of their listed resource — see [Equipment](#equipment)
for which item and odds.

Some moves also have a chance to inflict a status condition alongside their
damage, shown bracketed on the intrusion screen (e.g. `[Bleeding (2)]`).
**Bleeding** deals extra damage at the end of every round it's active;
**Stunned** costs the afflicted side (you, your companion, or the wild
program) their next action. Only one condition is active at a time — a
fresh one overwrites whatever was there. Wraith's Freeze, Construct's
Lockdown, Sentinel's Lockout, Trojan's Backdoor Access, Rootkit's Privilege
Escalation, Cipher's Encrypt, and Wintermute's Absolute Authority can stun;
Wraith's, Virus's, Daemon's Fork Bomb, Worm's Replicate, Ghost's Haunt, the
Overseer's Corrupt/Purge, and Wintermute's Cascade Logic can cause bleeding.

### Bosses

Rare, much tougher programs — rendered **bold** on the map and tagged
`[BOSS]` on the inspect/battle screens. A boss takes a habitat's spawn slot
only occasionally, in place of an ordinary program for that biome.
Defeating one guarantees a cache of 3-6 Portal Fragments at once, instead
of the flat drop chance every other species rolls — a reliable way to fund
the next Zone Portal. The Overseer and Wintermute (above) are the two
bosses in the base roster; mods can add more via `is_boss: true` in a
species file (see `assets/species/README.md`).

### Zones and portals

Every creature is tagged with the zone sector it was spawned in, shown
appended to its name (e.g. "Scrapper 2"). Defeating any wild program has a
chance to drop a Portal Fragment; deploy a Zone Portal structure (`b`) from
enough of them, then walk onto it to breach into the next zone.

- Each zone level **doubles** wild programs' stats compared to the last —
  zone 2 creatures hit twice as hard and survive twice as long as zone 1's,
  zone 3 quadruples it, and so on.
- Deploying a Zone Portal costs 5 Portal Fragments **times your current
  zone level** — breaching deeper costs more raw material each time, so
  fragments gathered in zone 2 only ever fund the zone-3 portal.
- Your active party travels with you through a portal; deployed structures
  and wild programs are left behind, and **there's no portal back down**.
- A defeated boss's guaranteed fragment cache is the fastest way to afford
  the next portal without a long grind.

### Structures

| Structure | Cost | Purpose |
| --- | --- | --- |
| Terminal | 3 Core Fragments | Passively cooks a Core Fragment into a Power Cell every so often while you're standing nearby — no cronjob needed |
| Data Cache | 5 Core Fragments | Utility storage |
| Mining Node | 2 Core Fragments | Cronjob a compiled program to it to produce Core Fragments over time |
| Power Conduit | 4 Core Fragments | Cronjob a compiled program to it to produce Power Cells over time |
| Compiler | 6 Core Fragments | Cronjob a compiled program to it to produce ICE Breakers over time |
| Fabricator | 8 Core Fragments | Cronjob a compiled program to it to produce Overclock Cores (see [Equipment](#equipment)) over time |
| Armory | 8 Core Fragments | Cronjob a compiled program to it to produce Firewall Plating (see [Equipment](#equipment)) over time |
| Home | 5 Core Fragments | `u` ("use symlink") instantly teleports you to it from anywhere on the map, for 4 Power Cells |
| Zone Portal | 5 Portal Fragments *(× current zone level)* | Walk onto it to breach into the next zone — see [Zones and portals](#zones-and-portals) |
| Black Market | 6 Core Fragments | `t` ("trade") to sell inventory items or buy consumables for Core Fragments — see [Trading](#trading) |

Mining Node, Power Conduit, Compiler, Fabricator, and Armory use **active**
automation (an assigned cronjob produces over time); Terminal uses
**passive** automation (it processes on its own whenever you're in range).
Home is a **symlink target** — a third category, neither cronjob nor
passive: press `u`, pick it from the list of deployed symlink structures,
and pay the Power Cell cost to warp there instantly, no matter how far
away you are. Deploy more than one and `u` lists all of them.
Any structure can define either or both via its `.ron` file — see
[Modding](#modding).

### Base defense

Every deployed structure has raid **Durability** (30 by default), shown as
`[HP x/y]` in the cronjob, symlink, and trade menus. Each tick has a small
chance of a raid hitting a random deployed structure:

- If a compiled program is assigned to it (`w`), it fights the raid off:
  the structure's damage is reduced by the worker's Defense stat, but the
  worker still takes a flat cost to its own HP for defending — win or
  lose. A worker knocked to 0 HP stands down from the cronjob (like a
  knocked-out companion), but isn't destroyed.
- An unassigned structure takes the raid's full damage. At 0 Durability
  it's destroyed outright, and any cronjob pointed at it is dropped.
- Damaged structures slowly regenerate Durability over time regardless.
- Recharging overnight (`r`) fully heals **every** tamed program you own,
  not just your active party — including one left behind defending a
  raid while you were elsewhere.

Keeping your key structures staffed is the cheapest defense; an idle
Mining Node out on its own is the one most likely to get chipped away.

### Trading

Press `t` to trade with a nearby Black Market. Pick the structure, then a
line item: sell offers (from your inventory) are numbered first, then buy
offers, followed by a quantity prompt.

- **Sell** any inventory item (except Core Fragments — trading them for
  more Core Fragments is a no-op the game refuses) for Core Fragments at
  the market's flat sell rate (1 each, for the base Black Market) — a
  floor value for excess loot that would otherwise just sit there.
- **Buy** whatever the market lists — the base Black Market sells ICE
  Breakers (4 Core Fragments), Power Cells (3), and **Portal Fragments**
  (8), so a Core Fragment surplus (e.g. from a well-fed Mining Node) can
  fund zone progression even without much combat.
- A structure's trade terms are entirely data-driven (`trade` in its
  `.ron` file) — see [Modding](#modding).

## Modding

Species and structures are plain data files under `assets/species/*.ron`
and `assets/structures/*.ron` — drop in a new `.ron` file and it's picked up
automatically next run, no recompiling needed. See the `README.md` in each
of those directories for the schema. A malformed file is skipped with an
in-game warning rather than crashing startup.

## Tests

```sh
cargo test
```
