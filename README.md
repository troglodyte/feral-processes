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
  restored) but you keep going.

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
| `p` | Pick a nearby compiled program as your active companion |
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
| **Level / XP** | Grows from defeating or decompiling rogue programs, or (for a compiled program) completing cronjob cycles. Each level-up grows Attack/Defense/max Integrity and fully heals. |
| **Attack** | How hard your hits land. Battle damage is roughly `move power + attacker's Attack − defender's Defense` (always at least 1). |
| **Defense** | How much incoming damage you shrug off — see the Attack formula above. |
| **Decompiler** | Player-only skill at cracking ICE. Grows by 1 every time you level up (starts at 0). Adds a flat bonus to your decompile odds — see Decompile chance below. Tamed programs never have this stat; only you attempt decompiles. |
| **Decompile chance** | Shown live during an intrusion. Your odds of successfully compiling (taming) the program *this attempt*, given its remaining HP fraction, its species' difficulty, and your Decompiler stat — weakening it first, and leveling up over time, both raise your odds. Shown even without an ICE Breaker in hand, so you can decide whether it's worth going to compile one. |

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
| Overclock Core | Weapon | +3 Attack | Fabricator cronjob; loot chance from Scrapper, Construct |
| Firewall Plating | Armor | +3 Defense | Armory cronjob; loot chance from Wraith, Sentinel |
| Neural Amplifier | Module | +2 Decompiler | Loot chance from Virus, Phantom |

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

### Companion

Press `p` to pick a nearby compiled program as your active companion — a
single tamed program that fights alongside you.

- Only one companion at a time; picking a different program swaps it in.
  Selecting the active companion's own number again stands it down.
- A companion is mutually exclusive with a cronjob: assigning it to work a
  structure (`w`) automatically stands it down as companion, and vice
  versa — a program is either working or fighting beside you, never both.
- During an intrusion, if you have an active companion the battle menu
  gains `[C]ommand companion`: it attacks using its own Attack stat
  *instead of* you acting that round — a turn-economy tradeoff, not a free
  extra hit.
- The wild program's retaliation has a 30% chance to target the active
  companion instead of you, using the companion's Defense stat. A companion
  knocked to 0 HP stands down automatically — it isn't lost, just no longer
  active; reselect it as companion (`p`) and recharge overnight (`r`) to
  heal it back up.
- The companion picker shows each candidate's status: `(active companion)`
  or `(on a cronjob)`, so you can see at a glance who's free to swap in.
- Recharging overnight (`r`) fully heals the active companion too, not
  just you.

### Current roster

| Program | Difficulty | Habitat | Works for |
| --- | --- | --- | --- |
| Sprite (`s`) | Easy | OpenGrid, Mainframe | — |
| Glitch (`g`) | Easy | OpenGrid, NullSector | Power Cells |
| Scrapper (`x`) | Medium | OpenGrid, NullSector | Power Cells |
| Wraith (`w`) | Medium | StaticField | — |
| Phantom (`p`) | Medium | Mainframe, StaticField | — |
| Virus (`v`) | Hard | NullSector, Mainframe | Core Fragments |
| Construct (`C`) | Hard | Mainframe | Core Fragments |
| Sentinel (`S`) | Hard | StaticField | — |

Scrapper, Wraith, Virus, Construct, Sentinel, and Phantom also each have a
chance to drop a piece of equipment on top of their listed resource — see
[Equipment](#equipment) for which item and odds.

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

Mining Node, Power Conduit, Compiler, Fabricator, and Armory use **active**
automation (an assigned cronjob produces over time); Terminal uses
**passive** automation (it processes on its own whenever you're in range).
Home is a **symlink target** — a third category, neither cronjob nor
passive: press `u`, pick it from the list of deployed symlink structures,
and pay the Power Cell cost to warp there instantly, no matter how far
away you are. Deploy more than one and `u` lists all of them.
Any structure can define either or both via its `.ron` file — see
[Modding](#modding).

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
