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
| `e` | Drain a Power Cell (restores Power) |
| `r` | Recharge overnight (restores Fatigue, costs Power) |
| `g` | Scan the sector for Power Cells |
| `c` | Compile an ICE Breaker (costs 3 Core Fragments) |
| `b` | Deploy a structure |
| `w` | Assign a compiled program to work a structure |
| `s` | Save |
| `q` | Quit |
| `+` / `-` | Zoom the grid in/out |
| `?` | In-game help / full control list |

**During an intrusion (battle):**

| Key | Action |
| --- | --- |
| `a` | Attack |
| `d` | Decrypt (attempt to compile/tame the program — needs an ICE Breaker) |
| `j` | Jack out (flee) |

### The loop

Explore the Grid, fight or decrypt rogue programs you run into, and deploy
structures (build menu) to put compiled programs to work gathering
resources for you. Defeating or decrypting a program grants XP; compiled
programs also gain XP from completed work cycles. Leveling up grows stats
and fully restores Integrity.

Starting resources (Core Fragments, Power Cells, ICE Breakers) bootstrap the
loop — after that, scan for more Power Cells, put a compiled program to
work a Mining Node for more Core Fragments, and compile more ICE Breakers
from those.

### Current roster

| Program | Difficulty | Habitat | Works for |
| --- | --- | --- | --- |
| Sprite (`s`) | Easy | OpenGrid, Mainframe | — |
| Scrapper (`x`) | Medium | OpenGrid, NullSector | Power Cells |
| Wraith (`w`) | Medium | StaticField | — |
| Virus (`v`) | Hard | NullSector, Mainframe | Core Fragments |
| Construct (`C`) | Hard | Mainframe | Core Fragments |

### Structures

| Structure | Cost | Purpose |
| --- | --- | --- |
| Terminal | 3 Core Fragments | Passively cooks a Core Fragment into a Power Cell every so often while you're standing nearby — no worker needed |
| Data Cache | 5 Core Fragments | Utility storage |
| Mining Node | 2 Core Fragments | Assign a compiled program to produce Core Fragments over time |

Mining Node uses **active** automation (an assigned worker produces over
time); Terminal uses **passive** automation (it processes on its own
whenever you're in range). Any structure can define either or both via its
`.ron` file — see [Modding](#modding).

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
