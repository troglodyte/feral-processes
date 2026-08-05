# feral-processes

![feral-processes gameplay screenshot](pics/gameplay.png)

A Neuromancer/Tron-flavored game blending Pokemon (tame and battle rogue
programs), Palworld (compiled programs work your base for you), and Dwarf
Fortress (procedural world, needs simulation, configurable permadeath).
Single-player, built in Rust, with a graphical frontend sitting on top of a
simulation that stays fully decoupled from presentation. A display is
required; there is no text mode.

This README is the overview. The full manual — every control, table, stat,
recipe, and species — lives in [docs/manual.md](docs/manual.md).

## Installing

You need the Rust toolchain (Cargo); if you don't have it, install it with
`curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh`. Then clone
the repo and `cargo build`. The binary resolves `assets/`, `saves/`,
`run_history.log` and `profile.ron` relative to the checkout at build time,
so the clone needs to stay put even if you `cargo install --path crates/launcher` to get
`feral-processes` on your `PATH`.

## Playing

Run `cargo run -p feral-processes` (or the release binary) and it launches
straight into the graphics window. Start a **New Game** from the main menu
and pick **Permadeath** (flatlining ends the run and appends a summary to
`run_history.log`) or **Forgiving** (flatlining costs Integrity and reboots
you at the nearest structure); either way you take a mild XP setback for
dying, or for a jack-out that actually gets you clear — escaping is a roll
weighed on your party's strength against the pack's, and crossing open
ground can get you ambushed. Each session gets its own save file under `saves/`,
autosaving every 50 game ticks, and `L` from the main menu lists every save
to load or delete. `A` from that menu opens your achievements, which are the
one thing that outlives a run. Press `?` in game for the complete control
list.

## The loop

Explore the Grid, fight or decompile the rogue programs you bump into, and
deploy structures to put your compiled programs to work gathering resources
for you. Kills, decompiles, and completed work cycles all grant XP, and
levelling grows stats and fully restores Integrity. Hostiles are colored by
an old-school "con" system scaled to your current power rather than a fixed
per-species color, so the same program reads Green early and Red again once
a deeper zone's stat doubling catches up to you. Programs spawn in packs
that fight as groups, and both group size and how many groups engage at once
scale with zone depth and distance from your base — a zone-1 fight beside
where you materialize is a genuine one-on-one, and the deep field can throw
four groups of a hundred at you.

## The Stack

Scattered across every zone are **links** (`>`) — holes down into a stack
of procedurally generated frames, seeded from the world seed and the depth
you have walked to. Step onto one and the view changes: the top-down
grid gives way to a **first-person corridor**, and `hjkl`/arrows stop being
compass directions and start being forward, back, turn left, turn right.
`<` and `>` follow whichever link is underfoot.

The party's map of the frame sits in the corner of the corridor the whole time
you are down there — `+` and `-` zoom it from the whole frame down to the
junction you are standing in — and `g` opens it full-screen with a legend, for
when the question is which wing you have not walked rather than which way you
are facing. Either way it shows what you have seen and nothing else: corridors
walked, links found, and the corridors something jumped you in. Cells you have
never had in view stay dark, and are drawn differently from rock you have seen,
because "I have not been here" and "there is nothing here" are the two things
worth telling apart.

A link opens onto a stack of fixed length rather than an endless descent.
How deep it runs is read off how far the walk to it was, so the same
distance that decides what lives down there decides how much of it there
is: the link inside your opening viewport is two frames, the ones out at
the edge of the sector are six. The bottom frame has no link down.

Corridors hang the occasional **doorway** (`+`) — walkable, but you cannot
see past one, so a corridor that ends in a door is a decision rather than
more corridor. The lair is walled off behind **sealed doors**, and opening
one burns an **Access Shard**, which is what caches are for. Once opened a
door stays open.

Three other things turn up in the floor. A **breakpoint** (`*`) is an
exposed debug port: walk onto it and the whole frame resolves at once, walls
and all, without your having walked a step of it — and it is the loudest
thing you can do, spiking Trace harder than anything else in the game. Each
one works once. A **fault** (`v`) is a hole: step on it and you drop a frame,
coming down far from that frame's way up, so a fall is a fast way down and a
slow way back. **Corruption** (drawn as purple ground rather than a glyph,
because it comes in stretches rather than single cells) is rotten substrate
that costs you Integrity every step you take through it — which is the first
time the maze has asked whether the short route is worth what it charges.

Not everything down there wants a fight. Most frames leave one **orphaned
process** (`o`) running in a dead end the caches did not take — a program
with nothing left to serve, drawn from the same biome as everything else
below that link. Press `o` while standing on one and it joins your roster
for an **ICE Breaker**, with no capture roll to win and no fight to survive.
The catalyst is not what limits this: your roster is. A full stack offers
more programs than you have slots to hold them, so the deeper ones are
scenery unless you have built the space to take them home.

The deepest room of a stack is a **lair** (`&`), holding something drawn
from the link tile's biome. It is seeded off the stack, so it is the same
guardian every time you come back — and the fight is scaled by every frame
you descended to reach it, and by how loud you were on the way down.

Materializing in a sector runs a deep scan that logs how many links are
in it and the bearing of the nearest. One is always within sight of where
you arrive; the rest are a walk. Each opens onto its own stack — two
links are never two doors onto the same maze.

Corridors are not safe. Every step you take underground can draw an
intrusion, and the programs down there are drawn from the biome the link
opens in — descend under a Mainframe sector and Mainframe programs are what
live below it. Each frame down multiplies their stats, and since a kill pays
out the defeated program's Integrity, it multiplies the XP too. Descending
is the trade: harder fights, better returns, and a longer walk back.

The stack also notices what you take from it. **Trace** rises when you crack
a cache, burn a seal, jack into a breakpoint or kill something, and the HUD
reads it back as one of
four bands — **Quiet**, **Noticed**, **Traced**, **Hunted**. Each band draws
ambushes more often, makes what comes hit harder, and sends more of it at
once. Walking is free: the meter is about greed, not time, so mapping a
frame carefully costs you nothing and stripping it costs you plenty. Clear
a stack out floor by floor and you will meet its guardian at the top band,
having asked for it.

Trace resets when you surface, which is less of a let-off than it looks —
caches, seals, breakpoints, orphans and lairs are each one-shot per stack, so
climbing out to shed it means coming back to a stack with less left worth
taking.

Your base does not stop while you are down there. The player's position on
the zone map stays pinned to the link you entered by, so cronjobs keep
paying out, needs keep decaying, and a raid can land on your Home while you
are four frames below it. Anything that reaches into the zone map — deploying
structures, trading, resting — is refused underground; managing your party,
inventory, routines and perks is not.

The one exception is the symlink. `u` works at any depth: it hauls the party
up out of the stack and drops them at the structure they linked to, paying
the usual cost. It will not save you from a fight — like every symlink it is
refused mid-battle — and it abandons the descent, since walking back into the
link starts you at the top again. What it does not cost you is the mapping:
every frame you had walked is still drawn when you come back down.

## Building and cronjobs

There are no resource deposits to stumble onto — every workable node is
something you build, and deploying always costs materials. You start with a
handful of Core Fragments, Power Cells, ICE Breakers, and Power Outlets;
after that Core Fragments come from creature drops and eventually a Mining
Node. Build a Home first (nothing else can be deployed until it stands, and
everything else must sit within its 15-tile platform), then assign compiled
programs to structures as **cronjobs** — the Palworld-style "put a tamed
creature to work" mechanic. Production then runs tick by tick wherever you
are, at a rate that adds the structure's upgrade tier to your zone depth — so
upgrading what you have is worth more than rushing the next portal, and
neither lever runs away with the economy.

Output does not appear in your pocket. Every structure has its own **output
buffer**, and production goes there: a node runs until its buffer is full and
then **clogs**, producing nothing more until you come home. Press `c` to
collect from every structure orthogonally touching you. Since structures block
movement you always stand beside one, never on it — so standing in the crook
of an L empties three buildings and a sprawled-out line costs you trips.
Where you put things is a decision.

You can also work a node yourself (base menu → *Work a structure yourself*),
which is the same job for the
same payout — you just have to stand there and do it, and stepping away
breaks off the cycle. It is what you fall back on before you have programs
worth posting, not a replacement for them: a cronjob runs whether or not
you are there, and earns its worker XP besides. It fills the same buffer.

## Production chains

Machines feed each other by **touching**. A structure declaring `assembles`
pulls its ingredients out of the output buffers of the four structures
orthogonally adjacent to it — never diagonally — and builds one unit at a
time. So a chain is a physical line across your base, and a machine with two
ingredients needs both feeders beside it.

The shipped chains are three stages deep, running from three taps into four
terminals:

| stage | structure | builds | from |
|---|---|---|---|
| extract | Mining Node | Core Fragments | — |
| extract | Power Conduit | Power Cells | — |
| extract | Log Scraper | Raw Trace | — |
| refine | Refinery | Bytecode Blocks | Core Fragments |
| refine | Lathe | Blank Substrate | Core Fragments |
| refine | Winding Node | Charge Coils | Power Cells |
| refine | Transcriber | Logic Wafers | Raw Trace |
| assemble | Assembly Bay | Patch Routines | a Block and a Coil |
| assemble | Disk Press | Routine Disks | a Substrate and a Wafer |
| assemble | Armory | Hardened Shells | a Block and a Coil |
| assemble | Fabricator | Trace Sniffers | a Wafer and a Coil |

A machine's recipe is not written on the machine — it runs the *item's* own
crafting recipe, so a bench recipe and a machine recipe can never drift, and
any craftable item you mod in is automatable for free.

**Armour and modules are made out of those intermediates**, not out of raw
fragments, and the ingredient follows the stat: Logic Wafers buy Decompiler,
Bytecode Blocks buy Attack and bulk, Charge Coils buy Defense. The two
classes draw on different taps — armour off the Mining Node, modules off the
Log Scraper — while both want Coils, so the Winding Node ends up with three
machines pulling on it and layout starts to bite. The Armory and Fabricator
each automate one piece while staying the hand-craft bench for the rest of
their class. Scavenged gear is deliberately left on raw fragments, so a run
with no base standing can still equip.

The base menu's *Recipes* row draws that table for whatever assets are
actually loaded, one entry per conversion, each walked back to the raw
inputs it bottoms out in — so a Patch Routine reads as the four steps it
takes rather than as two ingredients you then have to go look up. Each step
names the structure that taps its raw ingredient, so a chain read top to
bottom is the build order:

```
Product: Routine Disk
  Mining Node (Core Fragment x4)      -> Lathe       -> Blank Substrate x1
  Log Scraper (Raw Trace x4)          -> Transcriber -> Logic Wafer x1
  Blank Substrate x1 + Logic Wafer x1 -> Disk Press  -> Routine Disk x1
```

An ingredient an earlier step already makes is named on its own, and a tap
quotes no yield because a node's payout scales with its tier and the zone.
Mods appear in the table without anyone editing this README. It is reference
data rather than a view of your base, so it reads the same four frames down
the Stack as it does standing in the base.

Every machine needs a program posted to it, assemblers included, so **roster
capacity is what buys chain length, not fragments**. The full five-machine
line needs five programs against a starting cap of three — a Data Cache (+5,
ten fragments) covers the line and leaves a party to adventure with. A
machine says so in the base log when it goes *starved*
(nothing feeding it), *clogged* (output full, come collect) or *idle* (no
program), once on the way in rather than every tick, and the base menu's *Structure
roster* shows every
buffer and every stall at a glance.

The Assembly Bay costs Bytecode Blocks to build, so the two-machine line you
can afford at the start is what pays for the rest of it.

## Research

The base menu's *Research* row opens the research tree. Deploy a Research Node, cronjob a program
onto it, and it banks **Research Data** separately from your carrying
capacity, up to 200 — which is also why it's the one output that doesn't
scale with zone depth. Nodes cost a flat amount, may require other nodes
first, and unlock permanently and one-way: the Compiler, Power Conduit,
iso Market, Shield, Patch Node, and the Fabricator/Armory benches,
plus six discounted equipment recipes. The tree is data, not code — every node is a
`.ron` file in `assets/research/`.

## Intrusions

Walking into a hostile program opens an intrusion: a party-versus-party
round battle where you pick an action for each party member and the whole
round resolves at once in freshly rolled initiative order. Enemies fight as
groups sorted by species, only a group's front member can be hit, and only
the front two groups can reach you at all — anything further back has to use
ranged moves or sit the round out, which is what keeps a deep-field fight
survivable and makes clearing front-to-back a real decision. You can Attack,
Defend, spend a Special — Decompile among them, the taming routine the
player starts with installed, spending a catalyst you hold — use an item, or
jack out; a lowercase key acts for the member currently choosing and its
uppercase counterpart acts for the whole party. Both sides are laid out as
stat tables around the combat log, including a live per-group decompile
chance so you can watch a group become worth taming. Bosses quote no chance
— they are encounters to beat, never programs to compile.

## Companions

The party menu's *Companions* row lists every compiled program you own,
wherever it is; up to five can
be active party members. Party members fight beside you, gain half your XP,
passively add 10% of their own Attack and Defense to yours, and install
abilities as **routines** into level-derived slots as they level: one slot
per two companion levels, capped at six. A species' innate kit — data files
it claims by id and unlock level, priced by cooldown and Fatigue — is
pre-installed at tame or fuse time and topped up whenever a level-up reaches
a later unlock, and an innate routine can be popped back out and swapped for
a different one. The party menu's *Install a routine* row opens the routine panel (install,
swap, pop out), and *Extract a routine* at a Compiler standing anywhere on
the map salvages a single
routine out of any program you own — the program and its other routines are
destroyed. Not every routine comes from a kit or the research tree, though:
some exist only in the field. A wild program can spawn already carrying one
and will run it against you in battle — which is how you find out it has
it. Decompile the carrier and the routine comes over installed, ready to pop
out into whichever program you want running it instead; destroy the carrier
and the routine goes down with it. The player gets slots too, just slower:
one per ten of your own levels, same cap of six, starting with only
Decompile installed. Party order is the battle line, and `<`/`>` on that
screen move the highlighted member along it: the front slots draw more enemy
fire than the ones behind, so who leads is a decision, not the order you
happened to tame them in. A program
is either fighting or working a cronjob, never both. Every individual rolls
its own stats and growth rate within ±20% of the species baseline, surfaced
as a **Potential** tag, and tougher species grow faster per level. The party menu's *Fuse two programs* row
to fuse two programs into one stronger one — the result takes the
higher-level parent's species plus half the lower one's stats, and anything
can only be fused three times.

Some routines are field-only: instead of appearing in the Special menu in
battle, they cast straight from the map with `a`, for a Power cost and a
turn and no cooldown, arming a buff that keeps ticking through whatever
battle follows and survives a save — unlike the buff a companion's Special
arms mid-fight, which is wiped the instant that battle ends. A panel under
the map, and another in the battle roster, list every buff currently
running, however it was cast. Ten field routines ship: four land on one
ally you choose (a heal, a flat Attack or Defense bump, or a percentage of
damage reduction, each running for a set number of turns), and six always
land on the player regardless of who casts them, since they're pressure or
economy knobs the whole run feels rather than one combatant's stats —
Fatigue and Power regeneration, and percentage bonuses to capture odds, XP,
encounter chance, and drop rate. All ten come from research, off the same
`self_exec` root the other player routines hang from: Field Operations
compiles the three regeneration routines, then Adaptive Plating and Deep
Analysis branch off it for the combat trio and the four run-wide knobs.
Researching one drops it into cargo; installing it into a slot from the party menu is
a separate act, as with any other routine.

## Affinities

A species' `.ron` file can declare it's good at something: a per-category
multiplier on the magnitude of any ability that ends up running on it —
`damage`, `heal`, `buff`, `debuff`, and `drain`, one for each `AbilityEffect`
that carries a number. A Scrapper hits harder and heals worse than the
species baseline; a SubProcess is the other way around. `Cleanse` and
`Decompile` have no magnitude to scale, so no affinity touches them. You buy
the same five categories yourself as perks — Payload Tuning, Field Medic,
Overclocker, Corruption Vector, Siphon Protocol — each adding a flat
percentage per level, capped the same way a species' own affinity is
capped. The two never stack: a perk sharpens only the moves *you* cast, a
species affinity only the moves *its programs* cast, and nothing in the
game is both at once.

Affinity follows the ability into whatever slot it's installed in, not
just what the species was born with — a routine popped out at a Compiler
and plugged into a different program takes that program's affinity with
it. A species with a strong heal affinity and no innate heal isn't wasted;
it's a reason to move a researched or extracted heal routine onto it rather
than leave that program running whatever it started with.

## Items and equipment

The consumable economy is deliberately tight: Core Fragment is the universal
raw material, Power Cells and ICE Breakers are refined from it for one
purpose each, and Portal Fragments and Research Data are the two progression
currencies. Credits are money — a trader is the only thing that mints them
and the only thing that takes them. Everything you carry counts against a shared **Buffer** starting
at 30 units and growing with each deployed Data Cache. Press `i` for the
inventory screen, where you equip, unequip, consume, erase, and fuse items
across three slots (Weapon, Armor, Module). Picking one of those three slots
opens a replacement list for it: everything in cargo that fits, sorted by
what it would gain you, each row carrying both the bonus you'd get and the
change from what you're wearing. There are 31 pieces of gear:
six cheap ones behind both a research node and a bench, and 25 that declare
their own recipe, spanning a Scavenged tier you can make from turn one out
of raw fragments up to a Premium tier that wants Portal Fragments and
refined goods together. Between those, armour and modules are paid for in
what your production lines make; weapons stay on fragments. Gear
levels double the bonus per zone level reached, and both are locked in at
the moment you equip. Fusing feeds two copies of a piece in and gives one
back a tier stronger — worth +10% per tier, or at least a flat point on
every stat it already has, whichever is more. The floor matters more than
the percentage does: gear stats sit between 1 and 4, where 10% rounds away
to nothing.

## Zones and portals

Every creature is tagged with the zone it spawned in, and each zone level
doubles wild stats over the last, with another 25% per 15 tiles you wander
past your platform edge (capping at 3×). Deploy a Zone Portal — 10 Portal
Fragments plus half that again per zone below your current one — and walk
onto it to breach deeper. Your Portal Fragments and Core Fragments are wiped
in the crossing, so every zone has to fund its own exit — sell what you can
for Credits first, since those do cross — but your gear,
supplies, banked Research Data, party, and entire base — structures,
platform, upgrades, running cronjobs and all — rematerialize around the new
entry point. The portal itself is consumed, and there is no way back down.

## Achievements

Achievements are the only progression that survives a run. Breaching into a
deeper sector, reaching a Stack frame far enough down, keeping a run alive
past a cycle count, and putting down a boss program each earn a rung; the
thirteen shipped ones stamp `profile.ron` at the repo root **the moment they
are earned**, so a permadeath run that ends badly still keeps what it proved.

The reward is paid at the **start of your next run**: a point into one of
Attack, Defense, Integrity or Decompiler, a Perk Point, or — for reaching the
eighth frame — a free program, already yours and waiting to be added to your
party. Nothing is paid when you *load* a save; those stats are already in it.

A fully-cleared profile is worth 7 stat points, 5 Perk Points and one
program: a bit over one extra level's worth spread across a whole run, enough
to flavour a new run rather than skip its opening. That ceiling is asserted
over the asset files, because the offline balance simulator models a single
run's curve and cannot see the profile at all.

Both difficulty modes earn. An entry records the mode it was first earned in,
and a later permadeath re-earn upgrades that flag. `A` on the main menu lists
every rung, earned or not — the point is showing you what is left. To wipe
your profile, delete `profile.ron`; there is no in-game reset.

## Structures and base defense

Structures are `.ron` files declaring any combination of roles: cronjob-
workable (Mining Node, Research Node, Power Conduit), assembling
from adjacent neighbours (Compiler, Refinery, Winding Node, Assembly Bay), a
symlink target you can `u` to from anywhere (Home),
a rest gate, a power source (Recharger Node), a trading post (iso Market),
a repairer (Patch Node), or a crafting bench that also assembles
(Fabricator, Armory). Producers
upgrade to Mk5 from the base menu, each tier adding to the payout and raising the
chance a cycle pays out at
all, and upgrades ride through portals with the rest of the base. Every structure
except Home has raid Durability and can be chipped away by random raids: a
cronjob worker or a program posted to guard fights the raid off at a
cost to its own HP, and every deployed Shield shaves flat damage off every
raid anywhere in the base.

Raid damage is permanent. Nothing heals on its own — a chipped structure
stays chipped until you build the thing that fixes it. That thing is the
**Patch Node**, unlocked by the same Fortification research as the Shield:
it recompiles every structure in the base, itself included, for 1 Durability
per upgrade tier every 20 ticks, and several of them stack. Shields stop raid
damage arriving; Patch Nodes undo what got through; until you have either,
attrition only runs one way.

## Reading back

Two screens exist only to be read. `L` opens the message log in full — the
pane under the map has room for a handful of lines, and this is the last 100,
scrolled with Up/Down. A line that repeats folds into one row with a dim `×N`
beside it, so a base full of cronjobs reads as what happened rather than as
forty copies of the same extraction. It is bounded on purpose: a finished
intrusion keeps its results and drops its blow-by-blow, and the screen says so
rather than implying a complete transcript.

`f` filters the pane under the map, cycling All → Field → Base → All. The base
talks constantly — every cronjob payout, every failed extraction, every raid —
and in six rows of pane that steadily pushes what happened to *you* off the
top. Field shows the world you are standing in; Base shows the one running
without you, and keeps running while you are four frames down the Stack. The
header names the active filter and counts what it is holding back, so a raid
landing while you are reading the other channel still says so. `L` is
unfiltered whatever the pane is set to — the history is the complete record.

The base menu's *Structure roster* opens a read-only list: every structure standing in the zone, its
tier, tile, distance and raid Durability, and every program posted to it —
both the cronjob worker and the guard, where the map's own labels can only
show one. A workable structure with nobody on it is called out as idle, which
is usually why you looked. Neither screen takes an action and neither passes
game time; assigning, demolishing and upgrading each have their own row in that same menu.

## Trading

Press `t` at a nearby iso Market to sell inventory items for **Credits**,
buy ICE Breakers or Power Cells with them, or sell a compiled program for a
tenth of its power. Portal Fragments are deliberately not for sale:
breaching is earned by fighting. Credits are the only currency a trader
deals in — Core Fragments are salvage, and a trader buys those off you like
anything else rather than paying in them.

Every item is worth what it is worth: anything your base can print on a
timer fetches 1, while premium gear fetches 80–120. Nothing you can
manufacture is ever worth more than the parts, so an automated base is a
supply line and not a mint.

A finished trade leaves you on the trader's list rather than back on the
map, since a visit is usually a run of them — the header carries your
Credit balance so a sale shows up somewhere the popup isn't covering.

A trader keeps what you sell it. Anything you hand over goes onto its
buyback shelf, and you can purchase it back at **double** what it paid — so
a sale you regret costs a fee to undo rather than being final. The shelf
belongs to the tile the trader stands on, not the building: raze it and
rebuild on the same footprint and the stock is still there, but rebuild
somewhere else and you have opened a new store. It is wiped when you breach,
along with your salvage.

Selling a *program* is the exception: it is destroyed, not shelved. That's
still the only way to free a roster slot short of fusing it, and it's
permanent — the confirmation says so, along with any cronjob or guard post
the sale cancels. A structure's trade terms are entirely data-driven.

## Modding

Species, structures, items, abilities, achievements and research nodes are
plain `.ron` files under `assets/*/` — drop one in and it's picked up next run, no
recompiling, with a malformed file skipped and warned about rather than
crashing startup. Each of those directories has a `README.md` documenting
its schema. A new piece of equipment or a new combat ability is a single
file and no Rust at all.

Perks are the one deliberate half-exception. `assets/perks/*.ron` sets what
each perk is called, how it reads and what it costs, but the set of twelve is
fixed and lives in `crates/engine/src/perks.rs` — a perk's effect is a hook
into one particular formula rather than a shape the engine can read from a
file, so there is no `effect:` field to write and a thirteenth perk means
Rust.

The economy needs exactly one item holding each of the `Currency`,
`ResearchCurrency`, and `CraftCurrency` roles or the game won't start.

## Tuning difficulty

Everything the engine hardcodes about how hard the game is lives in one
file: `crates/engine/src/tuning.rs`. Zone and distance scaling, XP curves
and level caps, the damage and capture formulas, spawn and drop rates, raid
pressure, need decay, perk magnitudes — each is a documented constant in a
labelled section. Change a number, run `cargo test --workspace`, play.

It is a Rust file rather than a `.ron`, so retuning means a rebuild (a few
seconds here). That is the one deliberate difference from modding: content
is data, difficulty is code. Values that *are* data — species stats, item
and craft costs, structure economy, research costs, ability magnitudes —
stay in `assets/*/` and are not duplicated in `tuning.rs`.

Offline balance projections live next door in `balance_sim.rs`, a
deterministic, RNG-free simulator that fights zone-scaled packs against the
real `.ron` assets and asserts the resulting level curves as regression
tests. A retune that breaks progression usually shows up there first.

## Audio and fonts

The GUI plays short sound effects from `assets/sounds/` for movement,
intrusions, attacks, jacking out, winning, and flatlining; master volume
starts at 20% and moves in 10% steps with `[` and `]`, and `\` toggles
visual effects. Sound is a frontend concern the simulation knows nothing
about. Two typefaces are compiled into the binary: unscii (the pixel font
for the map grid, Public Domain/CC-0) and DejaVu Sans Mono (everything else,
Bitstream Vera license, whose notice must accompany all copies) — see
`assets/fonts/LICENSE-unscii` and `assets/fonts/LICENSE-dejavu`.

## Tests

```sh
cargo test --workspace
```

## Changelog

Release notes are in [CHANGELOG.md](CHANGELOG.md).
