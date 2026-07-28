# feral-processes — full manual

The complete reference: every control, table, stat, recipe, and species.
The short overview is in [the README](../README.md).

![feral-processes gameplay screenshot](../pics/gameplay.png)

A Neuromancer/Tron-flavored game blending Pokemon (tame and battle rogue
programs), Palworld (compiled programs work your base for you), and Dwarf
Fortress (procedural world, needs simulation, configurable permadeath).

Single-player, built in Rust. The graphical (GUI) frontend, shown above,
sits on top of a simulation that stays fully decoupled from presentation
so a client/server split is possible later too. A graphical display is
required; there is no text mode.

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

Run the `feral-processes` binary (the `launcher` crate):

```sh
cargo run -p feral-processes
```

It launches straight into the graphics window. A display is required — with
no `DISPLAY` or `WAYLAND_DISPLAY` (e.g. over SSH) it exits with a message
rather than starting.

To skip `cargo run`'s overhead on every launch, either run the built binary
directly:

```sh
cargo build --release
./target/release/feral-processes
```

or install it onto your `PATH` so you can just type `feral-processes` from
anywhere:

```sh
cargo install --path crates/launcher
```

Either way, the binary still finds `assets/`, `saves/`, and
`run_history.log` in this repo checkout — those paths are resolved at
build time, not from the current directory — so the clone needs to stay
put, but you can run or reinstall the binary from anywhere. Rebuild
(`cargo build --release`) or reinstall (`cargo install --path
crates/launcher`) after pulling code changes to pick them up.

Either way, from the main menu, start a **New Game** and pick a difficulty:

- **Permadeath** — flatlining ends the run for good; a summary is appended
  to `run_history.log`.
- **Forgiving** — flatlining costs you (half Integrity, some Fatigue/Power
  restored) but you keep going, rebooting at the nearest deployed structure
  (or in place, if you haven't built anything yet).

Either way, flatlining also docks a mild (20%) chunk of your current
in-level XP — never a de-level, just a setback. Jacking out of a fight
(`j`) costs the same modest XP setback, so fleeing isn't entirely free
either, but it's a lot cheaper than dying.

**Jacking out is an attempt, not a guarantee.** Whether you get clear is a
roll weighed on the whole party's strength against the whole pack's: run
from an even fight and you usually make it, run from something far out of
your weight class and you usually don't. There's some luck in it either way,
so a bad matchup is never hopeless and a good one is never certain. Fail and
the exit route collapses — the attempt burns your round and every engaged
group gets a free swing at you. You are *not* docked XP for a failed
attempt; you only pay the setback for an escape you actually got. Trying
again costs Integrity, not progress.

Each game session gets its own save file under `saves/` in the repo root,
named from when it was started. Starting a new game (`N`) claims a fresh
file immediately; `s` saves it manually at any time, and it also autosaves
to that same file every 50 game ticks (paced against game time, not
wall-clock time, so it's the same whether you're playing fast or slow) —
silently, so it won't cover up a status message from whatever you just did,
unless the autosave itself fails.

`L` from the main menu opens a list of every save in `saves/`, each shown
with a short summary (level, zone, difficulty, tick). Pick one to choose
**Load** or **Delete**. Save files aren't compatible across updates that
change what gets stored — a save from a different build shows up as
"(incompatible save)" and can still be deleted, just not loaded.

### Controls

| Key | Action |
| --- | --- |
| `hjkl` / arrow keys | Move (bumping a rogue program starts an intrusion) |
| `.` | Wait in place (advances one tick) |
| `e` | Drain the first Power-restoring item in your inventory (a Power Cell, unless a mod adds another) |
| `r` | Recharge overnight (restores Fatigue and Integrity, costs Power) — requires standing within your base, near Home (see [Structures](#structures)) |
| `g` | Scan the sector for Core Fragments |
| `c` | Open the compile menu — an ICE Breaker (3 Core Fragments), a Power Cell (2), the six Scavenged-tier gear pieces, and every other recipe whose research and/or bench you have (see [Equipment](#equipment)). Then pick a quantity: type digits and Enter, or `[F]` for 5 at once, or `[M]` for the most you can currently afford |
| `b` | Deploy a structure |
| `w` | Assign a compiled program to a cronjob (work a structure) |
| `G` | Assign a compiled program to guard a structure against raids (any structure, not just a workable one — see [Base defense](#base-defense)) |
| `R` | Demolish a nearby structure, refunding 30% of its materials — demolishing Home destroys every other base structure too, after a confirmation warning (see [Structures](#structures)) |
| `U` | Upgrade a nearby structure one tier — each tier costs more and yields more (see [Structures](#structures)) |
| `u` | Use symlink: instantly teleport to a deployed symlink structure (e.g. Home), for its item cost |
| `i` | Inspect: pick a direction, open the manifest for the first program that way (no intrusion) |
| `d` | Manifest: a full read-only stat sheet for you or any program you own — integrity and XP meters, combat stats, potential rolls, routines, equipment, perks, species detail. `←`/`→` page between subjects, `Esc` goes back to the list (and again to the map) |
| `v` | Inventory/equipment: equip, unequip, consume, fuse, erase items |
| `T` | Research tree: spend Research Data to unlock structures and recipes — see [Research](#research) |
| `p` | Your pets: full stats (level, HP, Attack, Defense) for every compiled program you own, wherever it is — add/stand down party members (max 5) here too. Standing one down frees a battle slot, not a roster slot; to shed a program for good, sell it at a Market (`t`), fuse it (`f`), or extract a routine from it (`M`) — and losing one in a fight or a raid does the same, permanently |
| `m` | Routine panel: install a loose routine into a free slot, pop an installed one back out, or swap one for another — see [Routines](#routines) |
| `M` | Extraction: with a Compiler owned anywhere, break a program you own down into exactly one of its routines — destroys the program and everything else it carried. See [Routines](#routines) |
| `f` | Fuse two compiled programs you own into one stronger one — the whole roster is offered, wherever the programs are |
| `t` | Trade with a nearby iso Market: sell items or compiled programs for Credits, buy consumables with them |
| `x` | Perks: spend Perk Points on permanent passive unlocks |
| `s` | Save |
| `q` | Return to the main menu — asks first, and offers to save on the way out |
| `+` / `-` | Zoom the grid in/out |
| `[` / `]` | Volume down/up in 10% steps — see [Audio](#audio) |
| `\` | Toggle visual effects on/off |
| `?` | In-game help / full control list |

**In a dungeon:**

Walking onto a breach (`>`) on the zone map drops you into a first-person
dungeon level. Each sector holds three, and materializing in one runs a deep
scan that logs how many there are and the bearing of the nearest — one is
always within sight of your arrival point, the others are a walk. A
structure cannot be deployed on a breach tile. The movement keys change meaning — they steer a party that
has a facing rather than moving a token on a grid.

| Key | Action |
| --- | --- |
| `k` / `Up` | Step forward along your facing |
| `j` / `Down` | Back up, without turning round |
| `h` / `Left` | Turn 90° left, in place |
| `l` / `Right` | Turn 90° right, in place |
| `>` | Descend — take the stairs down under you |
| `<` | Climb — take the stairs up, or surface from depth 1 |
| `.` | Wait in place |
| `e` | Drain a Power-restoring item |

Every other map key still works: inventory, party, routines, fusion, perks,
manifest, research, save and help all open normally underground. The ones
that reach into the zone map — deploy, cronjob, guard, demolish, upgrade,
symlink, trade, rest, scan — refuse with a message, because while you are
underground your position on the zone map is pinned to the breach you came
in through.

Every step in a corridor can draw an intrusion. The programs you meet come
from the biome the breach opens in, and each level of depth multiplies their
stats — and so the XP they pay, since a kill awards the defeated program's
Integrity. Shoving at a wall is not travel and never rolls for a fight.
Jacking out ends the fight where you stand.

`g` opens the **map** — the same key that scans the ground on the surface,
since the two screens never both apply. It is drawn north-up and shows only
what the party has had in view: cells never seen stay dark, and are drawn
differently from rock that has been seen. Stairs are marked, so are the
corridors you were jumped in. Opening it costs no time.

Each breach keeps its own map, and each level of a shaft its own. The map
survives saving and loading — a level regenerates from its seed, but what
you have seen of it is history and is written to the save.

Every numbered or lettered menu (compile, deploy, cronjob, inventory,
party, fuse, trade, perks, and so on) can also be navigated with Up/Down
arrows and confirmed with Enter — that's on top of, not instead of, typing
a row's own number or letter directly.

**During an intrusion (battle):**

You pick an action for **each** party member in turn, then the whole round
resolves at once. The menu is generated by the engine rather than written
into the renderer, so a new action appears without the UI being touched:

| Key | Action |
| --- | --- |
| `a` | Attack — then pick which enemy group to hit |
| `d` | Defend — brace for the round: a Defense bonus, and you draw more of the incoming fire |
| `s` | Special — picks one of that member's installed **routines**, then a target if it needs one. The row is hidden entirely, not greyed, for a member with nothing installed. Costs you Fatigue — how much the routine decides — and may sit out a few rounds afterwards. See [Routines](#routines) |
| `u` | Use item (you only) — spend a consumable as that slot's action for the round |
| `j` | Jack out (flee) — an *attempt*, not a guarantee: the odds weigh your party's strength against the pack's. Success costs a mild XP setback, same as flatlining; failure burns the round and draws a free volley, but costs no XP. A party-level command, not a per-member action |
| `A` | All attack — every unplanned slot attacks. Asks which group only if more than one is left |
| `D` | All defend — every unplanned slot braces |

**Shift means "everyone".** A lowercase key acts for the member currently
choosing; its uppercase counterpart acts for the whole party at once, filling
every slot you haven't already decided for and resolving the round. `A` and
`D` never overwrite a choice you made deliberately.

Both sides are listed as a stat table with a column header, so a value can
be compared by scanning down its column:

```
   GROUP              HP        ATK DEF RANGE   STATUS        DECOMP
A  4 Null Daemons     18/30       9   4 ENGAGED BLEEDING (2)     62%
B  Warden Process     44/44      14   9 BACK    OK               18%

   NAME               HP        ATK DEF POS   ACTION
>1 You                21/30      11   6 FRONT Attack A
 2 Sparkgrub          18/18       7   3 FRONT Defend
```

`RANGE` is whether a hostile group can reach you at all (see above); `POS`
is your own member's rank. `>` marks the member currently choosing. Each row
also carries the HP bar the table's numbers summarise.

`DECOMP` is your live chance of compiling that group's front program if you
decompiled it right now — see [Decompile chance](#stats). It moves as you
wear the front program down, so you can watch a group become worth taming
instead of guessing. Groups differ, which is why it's a column and not one
figure: a battered Glitch and a fresh boss are not the same gamble. With no
taming catalyst in hand there is nothing to quote and the column reads `—`.

Enemy groups are addressed by letter (`A`, `B`, ...). `Esc` backs the
planning cursor up a slot if you mis-pick — it also clears the slots after
it, since you chose those in light of the choice you're taking back. An
action that can't be used right now is still listed, greyed with the reason
(`no taming catalyst`, `roster is full`), rather than silently vanishing.

### The loop

Explore the Grid, fight or decompile rogue programs you run into, and
deploy structures (build menu) to put compiled programs to work gathering
resources for you. Defeating or decompiling a program grants XP; compiled
programs also gain XP from completed work cycles. Leveling up grows stats
and fully restores Integrity.

Every hostile program on the map is colored by an old-school "con" system,
scaled to your *current* power (max Integrity + Attack + Defense) rather
than a fixed per-species color — the same program can read Green early on
and Red again in a deeper zone once stat doubling catches up to you:

| Color | Meaning |
| --- | --- |
| Green | Much weaker than you — easy |
| Yellow | Roughly an even match |
| Orange | Notably tougher than you |
| Red | Far stronger — dangerous |
| Purple (Magenta) | A boss, regardless of stats |

Tamed/companion programs and structures keep their own fixed colors — only
hostiles get this treatment.

**Packs.** A hostile program sometimes spawns with others clustered right
next to it, and bumping into any one of them pulls the whole cluster into
the same intrusion. You can only attack or decompile whichever one is
currently up front, and only the front few packmates retaliate each round
— not the whole group — so defeating or taming the front one still just
brings the next packmate up rather than ending the fight. How large a
group can get depends on both how deep the zone is and how far the
encounter is from your base: zone 1 caps every group at one program, and
every zone after that triples the ceiling (zone 2 → 3, zone 3 → 9, zone 4
→ 27, zone 5 → 81, zone 6 and deeper → 100). You only meet that ceiling
out in the field — a group doubles in size every 15 tiles from the edge of
your platform, so encounters near home stay small however deep you've
breached. A group that outgrows its ceiling doesn't get any bigger: the
extra members stay standing on the map, to be met on a later bump.

How many *groups* meet you at once rides the same 15-tile curve: one at
your own doorstep, a second a step out, a third the step after, up to the
ceiling of four. So an encounter beside your platform is a single program
however deep you've breached, and a zone-1 fight near where you materialize
is a genuine one-on-one — which is what a run needs while your party is
still empty. See [Zones and portals](#zones-and-portals) for the matching
distance scaling on individual stats.

**The opening ring.** Inside that first 15-tile step of a *zone-1* breach —
measured from where you materialize until you've built a Home, from the
platform edge after that — spawn rolls are also filtered by what a bare
level-1 player can actually beat one-on-one. Most of the roster can take
you apart before you have a single companion, and none of it should be the
first thing you meet.
Bosses never spawn there. Where a biome has nothing gentle to offer, the
ring fields the mildest thing it has rather than rolling freely. This is a
rule about what gets *born* there, not a safe zone: programs wander, so
something tougher can still walk in from further out.

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
   Core Fragments except the Zone Portal, which costs Portal Fragments).
   **Home** always comes first in the menu, followed by **Mining Node** then
   **Research Node**, then **Compiler** once it's researched — everything
   else after — and nothing else can be built until a Home is standing.
   The menu only lists structures you've actually unlocked: Home, Mining
   Node, Research Node, Recharger Node, Data Cache, and the Zone Portal are
   available from turn one, and the other seven each sit behind a research
   node (see [Research](#research)). Your base travels with you through a
   zone transition (see [Zones and portals](#zones-and-portals)), so you
   only ever need to place a Home once.
3. **Schedule a cronjob with `w`** — pick a compiled (tamed) program, then
   the structure to assign it to. This only works on structures with a
   `work` recipe (Mining Node, Power Conduit, Compiler); Fabricator, Armory,
   Terminal, and Data Cache aren't assignable this way — Fabricator and
   Armory unlock crafting instead (see below), and Terminal automates
   passively. Both pickers show status: the program picker flags
   `(active companion)` or `(on a cronjob: <structure>)`, and the structure
   picker flags `(assigned: <program>)`, so you can see who's already
   spoken for before reassigning them.
4. **Production runs automatically after that**, tick by tick, regardless of
   where you are or what you're doing:
   - Each tick, the assigned program's progress advances by 1.
   - Once progress reaches the structure's `ticks_per_unit` (Mining Node 10,
     Power Conduit 6, Compiler 8), a completed cycle drops its output
     straight into *your* inventory, progress resets, and the worker
     gains 5 flat XP (enough to level up mid-cycle sometimes) — **except** a
     Mining Node, which is gated behind a level-based percentage chance
     (a basic Mk1 node succeeds only about half the time) on top of
     its doubled `ticks_per_unit`, so it's meaningfully slower and less
     reliable than the other two. A missed attempt still resets the cycle,
     it just doesn't pay out.
   - **How much a cycle pays is not one unit.** It's multiplied by your
     current zone level the same way wild programs' stats are — zone 1 pays
     ×1, zone 2 ×2, zone 3 ×4, and so on — and again by the structure's
     upgrade tier (see [Structures](#structures)). A Mk3 Mining Node in
     zone 4 drops 24 Core Fragments a cycle where a fresh one in zone 1
     drops 1. That's what makes settling in and building up worth the time
     rather than rushing for the next portal. Research Data is the
     exception: it's capped at 200 in your Buffer, so it always pays
     exactly one unit a cycle regardless of depth. Cronjob XP stops
     entirely once a worker hits
     **level 10** — resources keep flowing, but leveling past that only
     comes from battling, not idle cronjob work, up to the level 12 cap
     tamed programs share (see the Stats table below — you yourself have no
     cap at all).
   - Every worked structure holds a stock capped by the `capacity` in its
     `.ron` file (5 by default; the Research Node's is 4). Each completed
     cycle draws one down; once mined to 0 it immediately refills back to
     capacity and the worker keeps going — a worked node is an infinite,
     bursty resource, never a one-time deposit you can exhaust.
   - Terminal works differently: it's **passive**, not cronjob-based — it
     auto-cooks a Core Fragment into a Power Cell every tick whenever
     you're standing within 2 tiles, no assignment needed.
5. **Cronjobs persist across save/load.** A program's assignment, its target
   structure, and its in-progress tick count are all saved — reload and it
   picks up right where it left off, no need to reassign it with `w`.

Once you have a Mining Node feeding a steady supply of Core Fragments, put a
second program on a **Research Node** — its Research Data is what unlocks
the Power Conduit (Power Cells) and Compiler (ICE Breakers) that round out
the consumable loop, along with almost everything else worth building — see
[Research](#research) for the tree. The strongest-per-Fragment gear sits at
the far end of it, though most of the gear catalog needs only a bench, not a
research node each — see [Equipment](#equipment).

### Research

Press `T` to open the research tree. **Research Data** is the currency:
deploy a Research Node (10 Core Fragments, available from the start) and
cronjob a compiled program onto it, exactly like a Mining Node — a cycle
takes 14 ticks and, like a Mining Node, is level-gated, so a basic Mk1
node pays out only about half the time. Research Data doesn't count against
your carrying capacity; it's banked separately, up to 200. That cap is also
why Research Data is the one output that **doesn't** scale with zone depth
— it always pays one unit a cycle, so upgrading a Research Node buys you
reliability rather than volume.

Each node costs a flat amount of Research Data and may require others
first. Unlocking is permanent and one-way — there's no refund and nothing
to un-research. The menu lists available nodes first, then locked ones
(tagged with what they still need), then what you've already taken, each
group cheapest-first; it stays open so you can spend several times in one
visit.

| Node | Cost | Requires | Unlocks |
| --- | --- | --- | --- |
| Automation | 8 | — | Compiler |
| Power Grid | 10 | — | Terminal, Power Conduit |
| Isometric Commerce | 12 | — | iso Market |
| Fortification | 15 | Power Grid | Shield |
| Weapon Fabrication | 18 | Automation | Fabricator |
| Reactive Armor | 18 | Automation | Armory |
| Overclock Cores | 22 | Weapon Fabrication | Overclock Core recipe (6 Portal Fragments) |
| Firewall Plating | 22 | Reactive Armor | Firewall Plating recipe (6) |
| Neural Interfacing | 25 | Weapon Fabrication | Neural Amplifier recipe (6) |
| Monofilament Edge | 40 | Overclock Cores | Monofilament Whip recipe (12) |
| Ablative Lattice | 40 | Firewall Plating | Ablative Plating recipe (12) |
| Cortex Hacking | 45 | Neural Interfacing | Cortex Hack recipe (12) |

A recipe node only unlocks the *blueprint*. The recipe shows up in the
compile menu (`c`) only while the bench it names is actually deployed — a
Fabricator for the four weapon/module recipes, an Armory for the two armor
ones — so a recipe node is worth taking only alongside its bench.

The tree is data, not code: every node is a `.ron` file in
`assets/research/`, and a structure named by no research file at all is
buildable from turn one. See `assets/research/README.md` for the schema.

### Intrusions

Walking into a hostile program opens an intrusion — a party-versus-party
round battle, not a duel with an audience.

**Some intrusions open themselves.** Every step across open ground carries a
small chance of an ambush: a pack you never saw drops in beside you and
engages on the spot, with no option to walk around it. Routing carefully
past the programs drawn on the map is still worth doing, but it no longer
makes travel free. Two things an ambush never fields are a boss and a nest —
those stay something you find and choose to take on. Your base platform is
exempt: nothing spawns on it and nothing jumps you there, so the ground
inside your build radius stays safe to cross.

**Enemies fight as groups.** Everything in the pack is sorted by species, so
three Glitches are one addressable unit — `A  3 Glitches` — rather than
three separate rows. Only a group's **front** member can be hit, and only
its HP is shown; kill it and the next member of that group steps up. Empty
a group entirely and it's gone from the list, which shifts everything
behind it forward. How many groups may engage at once depends on how far
from base the encounter is — one beside your platform, four once you're 45
tiles out; if a bigger cluster than that pulls together, the largest groups
fight and the rest stay standing on the map for you to meet separately.
Four groups at a hundred programs apiece is the hard ceiling on one
intrusion — four hundred programs, all told.

**Only the front two groups can reach you.** Anything further back has to
shoot: a group in the back rank can use only moves its species flags as
ranged, and if it has none it can do nothing at all while it's back there
(the log will say so). That, plus the fact that only some of a group can
swing at once — a hundred-strong swarm brings ten weapons to bear in a
round, not a hundred — is what keeps a deep-field intrusion survivable at
all. It's also what makes clearing front-to-back a real decision rather
than an obvious one, because wiping the front group *promotes* a back
group into melee range. Sometimes the right move is to leave a harmless
melee-only group standing at the front as a plug.

**Initiative is rolled fresh every round.** Every combatant on both sides
rolls `base_speed + d10` and they all act in one interleaved order, so a
fast species tends to strike first without ever being guaranteed to. Speed
is per-species data (see `assets/species/README.md`), spanning the Construct
at 6 to the Sprite at 14; you roll from 11.

The screen lists hostile groups on top and your party along the bottom, each
with HP, Attack and Defense, with the log between them — the two sides face
each other across the narration of what passed between them. Back-rank groups
are dimmed, and whichever creature you're addressing is bold: the party member
currently choosing, and the group highlighted in the target picker. You choose
an action for every party member and the round resolves in initiative order,
with the narration landing in that middle pane — rounds are separated there by
a dim `── round N ──` line, so you read what happened without leaving the
planning screen.

### Stats

Shown in the status panel (always) and the intrusion screen (in battle):

| Stat | What it means |
| --- | --- |
| **Integrity** | Your HP. Hits 0 and you flatline — final in Permadeath, a costly soft-reboot in Forgiving mode. Leveling up or recharging overnight (`r`) both fully restore it. |
| **Power** | Your hunger-equivalent. Drains over time; hits 0 and you start taking Integrity damage each tick. Below 50%, your Attack also starts weakening — a linear falloff to half strength at 0 Power, on top of (not instead of) the tick damage. Restored by draining a Power Cell (`e`), standing near a cooking Terminal, or passively anywhere in a base with a Recharger Node. |
| **Fatigue** | Drains over time; restored to full by recharging overnight (`r`). Directing a party member's Special in battle (`s`) also costs some of it — how much is the ability's own business, so a field-wide sweep bites far harder than an ordinary command. Run short and that ability is refused until you rest. Rest also advances a lot of game time, so use both deliberately. |
| **Level / XP** | Grows from defeating or decompiling rogue programs, or (for a compiled program) completing cronjob cycles. Each level-up grows Attack/Defense/max Integrity, fully heals, and grants 1 Perk Point — see [Perks](#perks). **You** have no level ceiling at all; **tamed programs** stop at level 12, and further XP from any source is simply ignored once one is maxed. |
| **Attack** | How hard your hits land. Battle damage is roughly `move power + attacker's Attack − defender's Defense` (always at least 1). The same formula covers every combatant: your own strike has a fixed move power, while a program — yours or wild — rolls one of its species' moves. |
| **Defense** | How much incoming damage you shrug off — see the Attack formula above. |
| **Decompiler** | Player-only skill at cracking ICE. Grows by 1 every time you level up (starts at 0). Adds a flat bonus to your decompile odds — see Decompile chance below. Tamed programs never have this stat; only you attempt decompiles. |
| **Decompile chance** | Shown live during an intrusion and on the manifest. Your odds of successfully compiling (taming) the program *this attempt*, given its remaining HP fraction, its species' difficulty, your Decompiler stat, and the potency of the taming catalyst the attempt would spend — weakening it first, leveling up over time, and carrying a stronger catalyst all raise your odds. With no catalyst in hand there's nothing to quote (you can't attempt at all), so the readout says "needs a taming catalyst" instead. |

### Perks

Every level-up grants 1 Perk Point (shown in the status panel and the `x`
menu). Spend them on permanent passive upgrades — unlike a one-time unlock,
each perk can be bought repeatedly, and each purchase stacks another level
on top of whatever you already have, at the same Perk Point cost every time:

| Perk | Cost/level | Effect per level |
| --- | --- | --- |
| Keen Scavenger | 2 | +1 percentage point to scan (`g`)'s success chance |
| Low Power Mode | 2 | Power drains 1 percentage point slower (floor: stops draining entirely) |
| Exploit Focus | 3 | +1 effective Decompiler skill toward decompile odds |
| Lean Compiler | 3 | Compiling (`c`) costs 1 less of each required item (min 1 each) |
| Attacker | 2 | +1 permanent Attack |
| Defender | 2 | +1 permanent Defense |
| Buffer | 3 | +1% permanent max Integrity per level, minimum +10 (fully heals on purchase) |

The `x` menu shows each perk's current level next to it. Unlike species,
structures, items, abilities, and research, perks are a small fixed set of
player-only progression choices rather than moddable content — adding one
means editing `crates/engine/src/perks.rs`, not dropping in a file.

### Items

| Item | Source | Used for |
| --- | --- | --- |
| Core Fragment | Starting inventory; scan (`g`); dropped by Virus/Construct; a Mining Node cronjob | Deploy structures (2–6 each); compile an ICE Breaker (3 each) or a Power Cell (2 each) |
| Power Cell | Starting inventory; compiled (`c`) from 2 Core Fragments; dropped by Scrapper/Glitch; cooked passively at a Terminal; a Power Conduit cronjob | Drain (`e`) to restore Power |
| ICE Breaker | Starting inventory; compiled (`c`) from 3 Core Fragments; a Compiler cronjob | The taming catalyst Decompile spends — a Special (`s`) in battle, not its own key |
| Portal Fragment | 35% drop from any defeated wild program; a guaranteed 3–6 cache from a boss; buyable at an iso Market (8 Credits) | Deploy a Zone Portal; pay for every equipment recipe |
| Research Data | A Research Node cronjob | Unlock research nodes (`T`) — see [Research](#research) |
| Credits | Selling items or programs at an iso Market — nothing else mints them | Buying at an iso Market. The only cache that survives a breach |

A deliberately tight core-consumable economy: Core Fragment is the
universal raw material — found by scanning (`g`) or harvested passively via
a Mining Node — and Power Cells and ICE Breakers are refined from it
(compiled with `c`, scavenged from creatures, or produced by a structure
cronjob) for one specific purpose each. Portal Fragments and Research Data
are the two progression currencies, spent on zones and gear and on the
research tree respectively. Credits are money and nothing else: a trader is
the only thing that pays them and the only thing that takes them, which is
what makes selling a doomed stockpile before a breach worth doing. Equipment (below) is a separate, non-consumable
item category. Items are data-driven `.ron` files under `assets/items/`,
same as species and structures — see `assets/items/README.md` for the
schema and [Item ids](#item-ids) for the canonical list.

**Carrying capacity.** Everything you carry counts against a shared cargo
limit — your **Buffer** — which starts at 30 units and grows by 10 for
every Data Cache you have deployed. Paying an input cost that would
overflow it (compiling, buying, unequipping into a full Buffer) is refused
outright rather than clamped, so nothing you already spent gets destroyed.
Research Data is the exception: it's banked separately against its own
200-unit ceiling and never competes with cargo, so a pile of loot can't
starve a Research Node's output.

### Equipment

Press `v` to open the inventory/equipment screen from anywhere while
playing. It shows your stats, your three equipment slots, and your
inventory, each item numbered for selection.

Every equippable item in that list is tagged with the **slot it would take**
and the bonus it would give at your current zone level — `(WEP +4 ATK)`,
`(ARM +4 DEF)`, `(MOD +3 DECOMP)` — so you can see at a glance what a piece
competes with before you equip it. Anything that isn't equipment carries no
tag at all.

There are **31 pieces of gear**, reachable by two different routes.

**Route 1 — the research tree.** Six pieces need *two* unlocks, not one: the
recipe's **research node** (see [Research](#research)) *and* the **bench** it
names, deployed. They're the cheapest strong gear in the game, and that
discount is what the research investment buys you.

| Item | Slot | Base bonus (level 1) | Compiled for | Also drops from |
| --- | --- | --- | --- | --- |
| Overclock Core | Weapon | +3 Attack | 6 Portal Fragments, at a Fabricator | Scrapper, Construct, Trojan |
| Monofilament Whip | Weapon | +4 Attack | 12 Portal Fragments, at a Fabricator | Wintermute (boss) |
| Firewall Plating | Armor | +3 Defense | 6 Portal Fragments, at an Armory | Wraith, Sentinel |
| Ablative Plating | Armor | +4 Defense | 12 Portal Fragments, at an Armory | Rootkit |
| Neural Amplifier | Module | +2 Decompiler | 6 Portal Fragments, at a Fabricator | Virus, Phantom, Ghost, Overseer |
| Cortex Hack | Module | +3 Decompiler | 12 Portal Fragments, at a Fabricator | Cipher |

**Route 2 — the open catalog.** The other 25 declare their own recipe in
their `.ron` file and need **no research node at all**. Where one names a
bench, standing that bench is the whole unlock — every recipe below appears
in the compile menu the moment its Fabricator or Armory is up. The trade is
price: they cost more raw material than the researched six do for comparable
power, so research buys a discount rather than exclusive access.

**Scavenged tier** — no bench, compilable from turn one. What you make
before you have a base worth the name.

| Item | Slot | Base bonus | Compiled for | Drops from |
| --- | --- | --- | --- | --- |
| Shiv Routine | Weapon | +1 Attack | 4 Core Fragments | Sprite, Drone |
| Kinetic Edge | Weapon | +2 Attack | 7 Core Fragments | SubProcess, Glitch |
| Scrap Ward | Armor | +1 Defense | 4 Core Fragments | Glitch, Sprite |
| Packet Buffer | Armor | +2 Defense | 7 Core Fragments | Drone, SubProcess |
| Probe Daemon | Module | +1 Decompiler | 5 Core Fragments | Sprite, Glitch |
| Handshake Forge | Module | +2 Decompiler | 8 Core Fragments | Drone, SubProcess |

**Standard tier** — needs the bench, paid in Core Fragments. Where hybrids
start: a piece that splits its budget across two stats gives up raw numbers
for covering a weakness.

| Item | Slot | Base bonus | Compiled for | Drops from |
| --- | --- | --- | --- | --- |
| Arc Lance | Weapon | +3 Attack | 12 Core Fragments, at a Fabricator | Scrapper, Worm |
| Recursion Blade | Weapon | +2 Attack, +1 Defense | 14 Core Fragments, at a Fabricator | Trojan, Phantom |
| Daemon Fang | Weapon | +2 Attack, +1 Decompiler | 14 Core Fragments, at a Fabricator | Worm, Scrapper |
| Hardened Shell | Armor | +3 Defense | 12 Core Fragments, at an Armory | Wraith, Sentinel |
| Null Weave | Armor | +2 Defense, +1 Attack | 14 Core Fragments, at an Armory | Phantom, Trojan |
| Static Mesh | Armor | +2 Defense, +1 Decompiler | 14 Core Fragments, at an Armory | Wraith, Worm |
| Trace Sniffer | Module | +3 Decompiler | 13 Core Fragments, at a Fabricator | Phantom, Ghost |
| Logic Probe | Module | +2 Decompiler, +1 Attack | 15 Core Fragments, at a Fabricator | Trojan, Scrapper |
| Entropy Damper | Module | +2 Decompiler, +1 Defense | 15 Core Fragments, at a Fabricator | Wraith, Phantom |
| Sync Governor | Module | +1 Attack, +1 Defense, +1 Decompiler | 16 Core Fragments, at a Fabricator | Worm, Trojan |

**Premium tier** — needs the bench, paid in Portal Fragments, so it competes
with Zone Portals and the researched six for the same currency. Drops come
off Hard species and bosses.

| Item | Slot | Base bonus | Compiled for | Drops from |
| --- | --- | --- | --- | --- |
| Plasma Router | Weapon | +4 Attack | 16 Portal Fragments, at a Fabricator | Construct, Virus |
| Black ICE Pick | Weapon | +3 Attack, +2 Decompiler | 18 Portal Fragments, at a Fabricator | Cipher, Rootkit |
| Siege Compiler | Weapon | +3 Attack, +2 Defense | 18 Portal Fragments, at a Fabricator | Construct, Sentinel |
| Bastion Lattice | Armor | +4 Defense | 16 Portal Fragments, at an Armory | Sentinel, Rootkit |
| Phase Carapace | Armor | +3 Defense, +2 Attack | 18 Portal Fragments, at an Armory | Ghost, Virus |
| Wraithsteel Plate | Armor | +3 Defense, +2 Decompiler | 18 Portal Fragments, at an Armory | Ghost, Cipher |
| Kernel Key | Module | +4 Decompiler | 16 Portal Fragments, at a Fabricator | Cipher, Virus |
| Oracle Core | Module | +3 Decompiler, +2 Attack | 18 Portal Fragments, at a Fabricator | Overseer (boss), Rootkit |
| Singularity Matrix | Module | +3 Attack, +3 Defense, +3 Decompiler | 24 Portal Fragments, at a Fabricator | Wintermute (boss), Overseer (boss) |

The Singularity Matrix is the only piece that pays into all three stats at
full value, and it's priced and gated to match — the two bosses are its only
drop sources.

A Fabricator or Armory runs no cronjob of its own. It just makes recipes
appear in the compile menu while it's standing: the ones you've researched,
plus every catalog recipe that names it. Note that the benches themselves are
research-gated *to build* (Weapon Fabrication, Reactive Armor), so the
Standard and Premium tiers still sit behind research — just not behind a
research node each.

**Gear levels.** Every piece of equipment has a level, starting at 1, and
each level above that **doubles** the bonus of the one before it (level 2
= 2× the base bonus, level 3 = 4×, and so on). Reaching zone *N* (see
[Zones and portals](#zones-and-portals)) is what unlocks level *N* gear:
whatever you equip *while* at zone level *N* gets that level's scaled
bonus. The level is locked in at the moment you equip an item — like a wild
program's zone-doubled stats, it doesn't retroactively get stronger if you
breach deeper afterward while still wearing it. Unequip and re-equip the
same item (or an identical one from inventory) to pick up a newly unlocked
level. The inventory/equipment screen (`v`) shows each equipped item's
level and its actual scaled bonus.

- **Equip**: select a numbered inventory item, then `[E]`. Equipping into an
  already-occupied slot swaps the old item back into your inventory — you
  can only ever have one item per slot.
- **Unequip**: press the number of an occupied slot (1 Weapon, 2 Armor, 3
  Module) directly from the main inventory screen.
- **Consume**: select a numbered inventory item, then `[C]`. Offered only for
  items that declare a `consume` block in their `.ron` file; it spends one
  and applies whatever mix of Power, Fatigue, Integrity, and pre-battle buff
  that item defines. The `e` key is the shortcut for the Power case.
- **Erase**: select a numbered inventory item, then `[X]`. Permanently
  removes it from your inventory — there's no way to get it back.
- An equipped item's (level-scaled) stat bonus is added the moment you
  equip it and removed the moment you unequip it — it shows up immediately
  in the status panel and the intrusion screen.

### Fusing items

Got a duplicate piece of gear? Select a numbered
inventory item with 2 or more copies, then `[U]` to fuse: it permanently
consumes 2 copies of that item and adds another +10% to *that item type's*
equipped bonus (stacking every time you fuse it again — tier 2 is +20%,
tier 3 is +30%, and so on), applied on top of gear-level scaling. Like gear
level, the fusion tier is locked in at the moment you equip the item —
fusing further afterward doesn't retroactively boost a copy you're already
wearing; re-equip to pick up the new tier. The inventory screen and item
action menu show each item's current fusion tier alongside its preview
bonus.

### Companions

Press `p` to open your pets screen: it lists **every** compiled program you
own — wherever it is, not just what's nearby — with its level, HP,
Attack, and Defense, so you can check on a cronjob worker off at some
distant structure without walking over to it. Up to **5** of them can also
be active party members, fighting alongside you at once.

- Selecting a tamed program not already in the party adds it (rejected if
  the party's already full — stand one down first). Selecting a party
  member's own number stands it down. The screen stays open so you can
  adjust multiple slots in one visit; `Esc` closes it.
- Every active party member also passively adds 10% of its own current
  Attack and Defense (minimum 1 each) to yours, stacking across the whole
  party — shown live in your own Attack/Defense numbers in the status panel
  and intrusion screen. It updates automatically as a companion levels up
  or is fused, and drops off the moment it's stood down or knocked out.
- A party member is mutually exclusive with a cronjob: assigning it to work
  a structure (`w`) automatically stands it down from the party, and vice
  versa — a program is either working or fighting beside you, never both.
- During an intrusion **every** party member acts each round, and you pick
  what each of them does — they're combatants, not buff dispensers. A member
  that Attacks rolls one of its species' own moves. Choosing its Special
  instead gives up that attack, and asks which ability to spend before asking
  who it lands on — the picker names the one you chose. Buffs and heals list
  **your own side**, so you can boost or patch up any party member, not just
  yourself; a debuff lists enemy groups instead. Some abilities need no
  target at all: a whole-party heal or a field-wide sweep commits the moment
  you pick it.
- **A companion's kit grows as it levels.** Its species names which abilities
  it gets and the level each unlocks at; each one installs itself as a
  **routine** into a level-derived slot, topped up automatically at every
  level-up that reaches a new unlock — see [Routines](#routines) for how
  many slots that buys and what else you can do with them. A species that
  declares none falls back to a single temporary Attack boost. Abilities are
  data — see `assets/abilities/README.md` for what one can do and
  `assets/species/README.md` for how a species claims them.
- **Powerful abilities are paced by cooldowns and Fatigue.** Each declares
  how many rounds it sits out afterwards and how much Fatigue commanding it
  costs you. One that's still cooling, or that you can't afford, is greyed in
  the picker with the reason and can't be chosen — it never silently eats the
  round. Cooldowns last the one intrusion.
- Enemy retaliation picks a target by weight rather than a flat chance.
  Ranks are **soft**: the first three slots — you and your first two
  companions — draw noticeably more fire than the ones behind them, but
  every member stays reachable, so a back slot is safer, never safe. A
  member holding Defend draws more still, which is what makes bracing a
  play for the whole party rather than a selfish one. A party member brought
  to 0 HP is **deleted for good** at the end of the fight, taking every
  routine installed on it — nothing drops, and there is no reviving it. The
  battle pane turns a member's bar red once it is down to a third of its
  Integrity; that is your warning to jack out.
- Party **order** matters and is saved with your game, since it's what
  decides who's in those front slots.
- The party/cronjob pickers show each candidate's status: `(in party)` or
  `(on a cronjob)`, so you can see at a glance who's free.
- Recharging overnight (`r`) fully heals every tamed program you own too,
  not just you — not just the active party, see [Base defense](#base-defense).
- Every active party member gains **half** as much XP as you do from a kill
  or a successful decompile, independently of who actually landed the blow
  — they can level up (growing their own stats and fully healing) right
  alongside you. A tamed program that's idle or on a cronjob doesn't earn
  battle XP this way; only cronjob work cycles grow it (see
  [Getting started](#getting-started-building-and-running-cronjobs)).
- **Tougher species grow faster.** Per-level stat growth for a tamed
  program scales with its species' tier — Easy species grow at the
  standard rate, Medium/Hard/boss species grow noticeably more per level
  (see `assets/species/README.md`'s `growth_multiplier` field) — so a
  higher-tier catch keeps pulling ahead of an easy one as both level up,
  on top of already having tougher base stats.
- **No two individuals of the same species are quite identical.** Every
  creature rolls its own HP/Attack/Defense independently within ±20% of
  the species/zone-scaled baseline when it's created, plus its own
  ±20% roll on top of its species' growth rate for how fast it levels —
  so two Scrappers can genuinely differ, not just look the same with the
  same number. Its overall roll shows up as a **Potential** tag (e.g.
  "Excellent (94%)") in the pets screen (`p`) and on the manifest (`d`),
  which also breaks the tier down into the four individual rolls behind it
  — Poor / Below Average / Average / Above Average / Excellent.
  Fusing two programs (below) averages their rolls into the result rather
  than rolling a fresh one.

### Routines

An ability only does anything once it's **installed** into a slot — owning
the routine item on its own isn't enough.

- **Slots grow with level.** A companion gets one slot per two levels it
  reaches, rounded down and never less than one (so a level-1 program still
  has somewhere to hold its kit), capped at six — reached at level 12. You
  get one slot per ten of your own levels, starting from one, same cap of
  six — so your first *free* slot doesn't open until level 10.
- **You start with Decompile pre-installed.** It's the one routine every
  new game begins with, occupying your only slot until level 10 — taming
  is always available from turn one, before you've researched anything.
- **A species' innate kit installs itself**, pre-installed the moment you
  tame or fuse a program and topped up automatically on any level-up that
  reaches a later unlock (see [Companions](#companions)). An innate
  routine isn't welded in, though: pop it back out and it becomes an
  ordinary inventory item again, free to plug into a different program.
- **Research grants the item, not the slot.** Unlocking a node that names
  an ability (see [Research](#research)) compiles its routine straight
  into your cargo — it still has to be installed before you can spend it
  in battle. Researching the same ability from two different nodes stacks
  two copies of the item rather than granting it once.
- Press `m` to open the routine panel: install a loose routine into a free
  slot, pop an installed one back out, or swap one for another — on
  yourself or any companion.
- **Extraction destroys the program.** Owning a Compiler is what's checked,
  not proximity — one built anywhere counts, wherever you're standing.
  Press `M` to break a program you own down into exactly one of its
  routines. The program and every other routine it carried are gone for
  good.

### Fusing programs

Press `f` to fuse two compiled programs into one — pick the first, then
the second (anyone but the first). Both are consumed. Both pages list
every program you own, wherever it is and whatever it's doing, the same
way the pets screen (`p`) does — a worker parked at a far-off node is
just as fusable as the one standing next to you.

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
- **A program can only be fused 3 times.** The result of a fusion is one
  fusion "deeper" than its deepest parent, and once a program reaches 3 it
  can't be fed into another fusion at all — the pets and Fuse screens tag
  it (e.g. `(fused 3/3 — maxed)`), and picking it anyway just tells you
  why it's off the table. Fusing a 2-deep program with a freshly caught
  one still gives a 3-deep result, so the cheapest way to reach the cap is
  a chain, not a tournament bracket.
- A fused program isn't placed in your party or on a cronjob automatically
  — add it with `p` or assign it with `w` like any other compiled program.

### Current roster

| Program | Difficulty | Habitat | Works for |
| --- | --- | --- | --- |
| Sprite (`s`) | Easy | OpenGrid, Mainframe | — |
| Glitch (`g`) | Easy | OpenGrid, NullSector | Power Cells |
| Drone (`o`) | Easy | OpenGrid, Mainframe | Core Fragments |
| SubProcess (`d`) | Easy/Medium | OpenGrid, NullSector | Power Cells |
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

**Every** species on this list has a chance to drop equipment on top of its
listed resource, and most drop several different pieces, each rolled
separately — a single kill can occasionally yield two. Which species drops
what runs roughly by difficulty: Easy programs shed Scavenged-tier gear,
Medium ones Standard tier, Hard ones and bosses Premium tier. See
[Equipment](#equipment) for the per-item sources and odds.

Some moves also have a chance to inflict a status condition alongside their
damage, shown bracketed on the intrusion screen (e.g. `[Bleeding (2)]`).
**Bleeding** deals extra damage at the end of every round it's active;
**Stunned** costs the afflicted side (you, your companion, or the wild
program) their next action. Only one condition is active at a time — a
fresh one overwrites whatever was there. Wraith's Freeze, Construct's
Lockdown, Sentinel's Lockout, Trojan's Backdoor Access, Rootkit's Privilege
Escalation, Cipher's Encrypt, the Overseer's Kernel Panic, and Wintermute's
Absolute Authority can stun; Wraith's, Virus's, SubProcess's Fork Bomb,
Worm's Replicate, Ghost's Haunt, the Overseer's Purge, and Wintermute's
Cascade Logic can cause bleeding.

### Bosses

Rare, much tougher programs — rendered **bold** on the map and tagged
`[BOSS]` on the manifest and battle screens. A boss takes a habitat's spawn slot
only occasionally, in place of an ordinary program for that biome.
Defeating one guarantees a cache of 3-6 Portal Fragments at once, instead
of the flat drop chance every other species rolls — a reliable way to fund
the next Zone Portal. The Overseer and Wintermute (above) are the two
bosses in the base roster; mods can add more via `is_boss: true` in a
species file (see `assets/species/README.md`).

### Nests

Some species build **nests** — an `N` on the map, in that species' color,
surrounded by a cluster of its own kind. A wild program of a nesting
species occasionally spawns one instead of spawning alone; Scrapper,
Wraith, Trojan, and Worm nest in the base roster (`can_nest: true` in a
species file).

- A fresh nest comes with **2–5 guardians**, which behave like ordinary
  hostiles except that they never wander more than 5 tiles from it.
- While the nest stands it keeps replacing them: every guardian you defeat
  or decompile is queued for respawn 10 ticks later, so grinding the
  guardians alone is an endless (if farmable) fight.
- **Walk into the nest to attack it.** That's a plain hit for your current
  Attack against its 60 Durability — no intrusion screen, no defense on its
  side, and no retaliation, so it's pure chip damage rather than a battle.
- Destroying it frees every surviving guardian (they scatter into ordinary
  wandering) and cancels any queued respawns.
- Raids never target a nest; its Durability is only ever spent by you.

A nest is a deliberate risk/reward pocket: a dense cluster of one species
worth farming for that species' loot, at the cost of being outnumbered
somewhere you can't retreat far from.

### Zones and portals

Every creature is tagged with the zone sector it was spawned in, shown
appended to its name (e.g. "Scrapper 2"). Defeating any wild program has a
chance to drop a Portal Fragment; deploy a Zone Portal structure (`b`) from
enough of them, then walk onto it to breach into the next zone.

- Each zone level **doubles** wild programs' stats compared to the last —
  zone 2 creatures hit twice as hard and survive twice as long as zone 1's,
  zone 3 quadruples it, and so on.
- Wandering away from your base adds its own scaling on top: every 15 tiles
  past the edge of your base platform adds another **25%** to wild stats,
  capping out at **3×** far enough out. Since the platform reaches 7 tiles
  from Home, the first step up lands **22 tiles from Home** — the whole
  base counts as safe territory, not just its centre. Before
  you've placed your first Home there's no platform, so it measures from
  where you breached in instead.
- Deploying a Zone Portal costs 10 Portal Fragments, **plus half that again
  for every zone below your current one** — 10 in zone 1, 15 in zone 2, 20
  in zone 3. Breaching deeper costs more raw material each time.
- **Your fragments and cores don't survive the breach.** Portal Fragments
  and Core Fragments are cleared as you step through (Credits are not), so every zone has to
  fund its own exit — you can't farm zone 1 rich and then chain-breach on
  the stockpile. Research Data is banked progress and is kept, as are your
  gear, your supplies, and your fusion tiers.
- **Your whole base travels with you.** Your active party, every deployed
  structure, and the platform floor under them all rematerialize around the
  new zone's entry point in exactly the layout they left in — damage, node
  stock and running cronjobs included (the currency in your own Buffer is
  the exception, above). Wild programs and nests are left behind, and
  **there's no portal back down**.
- **A Zone Portal is consumed when you step onto it.** It's the one
  structure that doesn't make the trip, so every breach costs a fresh
  build.
- A defeated boss's guaranteed fragment cache is the fastest way to afford
  the next portal without a long grind.

### Structures

| Structure | Cost | Unlocked by | Purpose |
| --- | --- | --- | --- |
| Home | 5 Core Fragments | — | Anchors your base platform (see below). `u` ("use symlink") instantly teleports you to it from anywhere on the map, for 4 Power Cells. Also lets you `r` (recharge/rest) anywhere in the base. Can't be raided — see [Base defense](#base-defense) |
| Mining Node | 12 Core Fragments | — | Cronjob a compiled program to it to produce Core Fragments over time (slower and level-gated — see [Getting started](#getting-started-building-and-running-cronjobs)). Upgradeable to Mk5 |
| Research Node | 10 Core Fragments | — | Cronjob a compiled program to it to produce Research Data over time (14 ticks a cycle, level-gated like a Mining Node) — see [Research](#research). Upgradeable to Mk5 |
| Recharger Node | 10 Core Fragments | — | Passively refills your Power anywhere within 7 tiles — the whole base |
| Data Cache | 10 Core Fragments | — | Raises your carrying capacity (Buffer) by 10 while deployed; stacks with every other one |
| Zone Portal | 10 Portal Fragments *(+50% of that per zone level)* | — | Walk onto it to breach into the next zone. Consumed on use, and your fragments and cores don't survive the trip — see [Zones and portals](#zones-and-portals) |
| Compiler | 16 Core Fragments | Automation | Cronjob a compiled program to it to produce ICE Breakers over time. Upgradeable to Mk5 |
| Terminal | 3 Core Fragments | Power Grid | Passively cooks a Core Fragment into a Power Cell every tick while you're standing within 2 tiles — no cronjob needed |
| Power Conduit | 14 Core Fragments | Power Grid | Cronjob a compiled program to it to produce Power Cells over time |
| iso Market | 16 Core Fragments | Isometric Commerce | `t` ("trade") to sell inventory items or compiled programs, or buy consumables, for Core Fragments — see [Trading](#trading) |
| Shield | 16 Core Fragments | Fortification | Passively reduces raid damage against **every** deployed structure by 4 — see [Base defense](#base-defense) |
| Fabricator | 18 Core Fragments | Weapon Fabrication | Not cronjob-workable — the bench for every researched weapon/module recipe, plus the 13 catalog recipes that name it (see [Equipment](#equipment)) |
| Armory | 18 Core Fragments | Reactive Armor | Not cronjob-workable — the bench for every researched armor recipe, plus the 6 catalog armor recipes that name it (see [Equipment](#equipment)) |

The "Unlocked by" column is the research node you must take before the
structure appears in the build menu at all (see [Research](#research)); a
dash means it's available from turn one. Home must be built before anything
else regardless — the build menu (`b`) always lists it first, followed by
Mining Node then Research Node then Compiler, with the rest after.

Only one Home can exist at a time, and every other structure must be
deployed within 7 tiles of it — a base clusters around its Home rather
than sprawling across the map. `R` demolishes a nearby structure and
refunds 30% of its materials; demolishing Home is a special case, since
without it nothing else could exist out of range anyway — it cascades to
demolish **every** other base structure too (each refunding its own 30%
share), so `R` warns you and asks to confirm before Home specifically goes
down. Remove Home to relocate the whole base, or to free up the 15-tile
radius for a fresh one elsewhere.

**Deploying a Home lays a platform.** Every tile within that 15-tile build
radius is flattened into base flooring, obliterating the terrain, nests and
rogue programs that were standing there. Nothing wild ever spawns on
platform flooring, so your base is a genuine safe haven — the only threat
that reaches it is a raid (see [Base defense](#base-defense)). There's
nothing to scavenge on it either: `g` always comes up empty on your own
floor. Demolishing Home tears the platform up again and the natural terrain
underneath comes back. The platform travels with you between zones, so a
base founded in zone 1 is the same base you're still standing in at zone 6
— see [Zones and portals](#zones-and-portals).

**Upgrade tiers.** Structures that produce something can be upgraded with
`U`, one tier at a time up to Mk5. Reaching tier N costs that structure's
upgrade price **times N**, so a Mining Node's Mk1→Mk2 costs 20 Core
Fragments and Mk2→Mk3 costs 30. A tier does two things at once: it
multiplies what a finished cronjob cycle pays out, and it makes cycles more
likely to succeed at all — a Mk1 node pays out on about **50%** of its
cycles, rising 10 points a tier to **90%** at Mk5.
Upgrades ride through a portal with the rest of the base, so they're the
main thing worth pouring materials into — an upgraded base is what keeps
your income ahead of each zone's rising portal cost.

Mining Node, Research Node, Power Conduit, and Compiler use **active**
automation (an assigned cronjob produces over time); Terminal uses
**passive** automation (it processes on its own whenever you're in range);
Fabricator and Armory use neither — they're benches, making already-
researched recipes compilable while they stand; Shield uses neither either
— it just sits there passively defending (see
[Base defense](#base-defense)).
Home is a **symlink target** — a third category, neither cronjob nor
passive: press `u`, pick it from the list of deployed symlink structures,
and pay the Power Cell cost to warp there instantly, no matter how far
away you are. Deploy more than one and `u` lists all of them.
Recharger Node is a **passive power source** — a fourth category: it
refills your Power every tick you're inside its 7-tile radius, with no
worker and no input item. Home doubles as the **rest gate**: `r` only
works within 7 tiles of it, which is exactly the base footprint.
Any structure can define any combination of these via its `.ron` file —
see [Modding](#modding).

### Base defense

Every deployed structure except Home has raid **Durability** (30 by
default), shown as `[HP x/y]` in the cronjob, symlink, and trade menus.
Home can't be raided at all — it has no Durability, shows no `[HP x/y]`,
and is never picked as a raid target. Losing the structure that gates every
other build, anchors your symlinks, and can only exist once would strand
you rather than cost you something. Each tick has a small chance of a raid
hitting a random one of your *other* deployed structures:

- If a compiled program is assigned to it — either cronjob-working it (`w`)
  or just posted to guard it (`G`) — it fights the raid off: the
  structure's damage is reduced by the defender's Defense stat, but the
  defender still takes a flat cost to its own HP for defending — win or
  lose. A defender brought to 0 HP is **destroyed** — and since programs
  have no passive healing, raid damage accumulates until you come home and
  recharge (`r`). A program left on a cronjob long enough will eventually
  be lost to raids while you are elsewhere.
- `G` (guard) works on **any** raidable structure, including ones with no
  cronjob recipe at all — Terminal, Data Cache, Fabricator, Armory, and so
  on. It's the only way to defend those. A structure already cronjob-worked
  is already defended by its worker; guard it separately only if you want a
  program standing there purely for defense, doing no production. Guarding
  Home is refused outright — there's no raid coming for it, so a program
  posted there would wait forever instead of doing something useful.
- Every deployed **Shield** shaves a flat amount (4) off *every* raid's
  damage, against *any* structure it hits — not just itself, and it stacks
  across however many Shields you've built. This is applied before a
  worker/guard's own Defense-based mitigation, so the two stack: a couple
  of Shields plus a guarded structure can fully no-sell a raid. A Shield
  has no cronjob recipe of its own — it just sits there defending, and it's
  a raid target like anything else, so it's not invulnerable.
- An unassigned (and unguarded) structure — after Shield reduction — takes
  whatever raid damage is left. At 0 Durability it's destroyed outright,
  and any cronjob/guard assignment pointed at it is dropped.
- Damaged structures slowly regenerate Durability over time regardless.
- Recharging overnight (`r`) fully heals **every** tamed program you own,
  not just your active party — including one left behind defending a
  raid while you were elsewhere.

Keeping your key structures staffed is the cheapest defense early on; a
Shield (or several) is the scalable version once you can afford one — an
idle Mining Node out on its own is the one most likely to get chipped away
without either.

### Trading

Press `t` to trade with a nearby iso Market (unlocked by the Isometric
Commerce research node). Pick the structure, then a line item: sell offers
(from your inventory) are numbered first, then buy offers, then any programs
the market will take. Items and buys go on to a quantity prompt; a program
goes straight to a confirmation.

- **Sell** any inventory item (except Core Fragments — trading them for
  more Core Fragments is a no-op the game refuses) for Core Fragments at
  the market's flat sell rate (1 each, for the base iso Market) — a
  floor value for excess loot that would otherwise just sit there.
- **Sell a compiled program** for a tenth of its power (max HP + Attack +
  Defense, rounded down, never less than 1). This is the only way to get rid
  of a program short of fusing it into another, so it's your way out of a
  full roster — standing one down with `p` frees a battle slot but not a
  roster slot. It's permanent: the program is erased, and the confirmation
  says so along with anything it was doing (a cronjob or guard post is
  cancelled by the sale). A level-1 Glitch is worth about 4 Core Fragments;
  a heavily levelled fusion, tens.
- **Buy** whatever the market lists — the base iso Market sells ICE
  Breakers (4 Core Fragments), Power Cells (3), and **Portal Fragments**
  (8), so a Core Fragment surplus (e.g. from a well-fed Mining Node) can
  fund zone progression even without much combat.
- A structure's trade terms are entirely data-driven (`trade` in its
  `.ron` file) — see [Modding](#modding).

## Modding

Species, structures, items, abilities, and research nodes are plain data
files under `assets/species/*.ron`, `assets/structures/*.ron`,
`assets/items/*.ron`, `assets/abilities/*.ron`, and `assets/research/*.ron` —
drop in a new `.ron` file and it's picked up automatically next run, no
recompiling needed. See the `README.md` in each of those directories for the
schema. A malformed file is skipped with an in-game warning rather than
crashing startup.

A new piece of equipment is a **single file** and no Rust at all: `equipment`
gives it a slot and any mix of Attack/Defense/Decompiler, `craftable` gives it
a recipe (optionally naming a bench that must be standing), and `droppable`
lists the species that drop it and at what odds. The 25-piece catalog in
[Equipment](#equipment) is written exactly this way — nothing about it is
privileged over gear you add yourself.

A new combat ability is likewise a single file: `target` picks who it lands
on (one ally, the whole party, one enemy group's front, a whole enemy group,
or every hostile on the field), `effect` picks what it does (damage with an
optional status rider, a heal, a stat buff, or a status debuff), and
`cooldown`/`fatigue_cost` price it. A species claims one by naming its id and
the level it unlocks at — see `assets/abilities/README.md`.

Structures are equally open-ended: a single `.ron` file decides whether a
structure is cronjob-workable, passively processing, a symlink target, a
rest gate, a power source, a trading post, temporary, and — via
`raidable: false` — whether
raids can target it at all. That last flag is the whole of what makes Home
safe; any structure you add can claim the same protection.

Perks are the one exception: they're a fixed, player-only set that lives in
Rust (see [Perks](#perks)).

### Item ids

The base game ships **36** items: the eleven below, plus the 25-piece gear
catalog in [Equipment](#equipment) (whose ids are the snake_case form of
their names — `arc_lance`, `singularity_matrix`, and so on).

Species, structure, and research files all reference items by id. The eleven
originals predate the data-driven item model and mods named them in
PascalCase back then — the second column is what to replace those with.

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
files in `assets/items/`, and any of them can be edited or removed (subject
to the role rule below).

### The four economy roles

The game needs exactly one item holding each of `Currency`,
`ResearchCurrency`, `CraftCurrency` and `TradeCurrency` to start — these are
the anchors every build cost, research spend, zone-portal cost and trade
reads through instead of naming a hardcoded item. `Currency` (Core Fragment)
is the salvage the build economy runs on; `TradeCurrency` (Credits) is what
traders deal in, and no trader touches the other three. Removing (or renaming without re-tagging) the item
that holds a role, with nothing else claiming it, leaves the economy
incomplete and the game won't start; see `ItemDb::missing_roles`.

## Audio

The GUI plays short sound effects for movement, starting an intrusion,
attacking, jacking out, winning, and flatlining, from `assets/sounds/`.
Master volume starts at 20% and is adjustable in-game with `[` and `]` in
10% steps (0–100%), showing a toast as it changes. `\` toggles visual
effects on and off the same way. Sound is a frontend concern the simulation
knows nothing about.

## Fonts

The GUI compiles two typefaces into the binary via `include_bytes!`: unscii
(the pixel font used for the map grid) and DejaVu Sans Mono (used for
everything else). unscii-16 is Public Domain / CC-0 (Viznut); DejaVu Sans
Mono is licensed under the Bitstream Vera license, which requires its
notice accompany all copies of the font. See `assets/fonts/LICENSE-unscii`
and `assets/fonts/LICENSE-dejavu` for the full notices.

## Tests

```sh
cargo test --workspace
```

## Changelog

Release notes have moved to [CHANGELOG.md](../CHANGELOG.md).
