# Changelog

Release notes for [feral-processes](README.md).

Versions follow [semantic versioning](https://semver.org). While the project
is `0.x`, a **breaking** change bumps the minor version and a compatible one
bumps the patch. For a single-player game with no public API, "breaking"
means one thing above all: **a save-format bump**, where existing saves stop
loading (see `save::SAVE_FORMAT_VERSION`). Every crate in the workspace
shares one version, set in the root `Cargo.toml`.

At `1.x` and beyond the same definition of "breaking" moves up a level: a
save-format bump takes the **major**, new content or a new feature takes the
**minor**, and a fix or a balance retune takes the **patch**. The rule that
survives the boundary is the one that matters — what counts as breaking is
decided by what happens to a player's save, not by what happens to a type
signature.

**A release is cut per change that lands on `main`, not per batch.** A
feature or fix merged to `main` bumps the version, gets its own section
below, and is tagged; commits on a branch stay unversioned until they land.
This is a correction, adopted after `0.3.0` — everything from `v0.2.0` to
`v0.3.0` accumulated in a single `Unreleased` section over 2,200 lines long,
which is a changelog nobody can read and a version number that says nothing
about what is installed.

Entries below `0.2.0` predate versioning and are kept as written, newest
first, separated by a rule.

## 0.8.35

### Work orders: the base staffs itself

You used to staff the base one program at a time, forever. Open the base
menu, pick a program, pick a structure, repeat — and then repeat it again
every time a machine clogged, a worker was wanted elsewhere, or a chain
needed its upstream link fed before its downstream one could run. That is
not a decision, it is bookkeeping.

**Now you say what you want made.** "3 Routine Disks" — and the base works
out that this means Core Fragments through a Lathe into a Disk Press, and
staffs itself in that order with whoever you have given it.

An order is a **target level, not a production run**: three already sitting
in a Depot satisfy it on the spot. Completion is measured across Depots and
machine output buffers, because what the base *holds* is the question and
where it holds it is not.

**A work order stores an item and a quantity, and nothing else.** No
per-machine plan, no unit targets, no progress counters. Which machines a
line needs, in what order, and who is on each is recomputed from live world
state every tick — the same call this game already makes for the build
radius, a Broker's board, a Stack description and the wielded program's
bonus. The status screen runs the scheduler's own walk, so what you read is
what the base believes by construction rather than by a comment claiming
the two agree. Multiplying the recipe tree through at queue time would have
produced a tidier progress bar and a plan that is confidently wrong the
moment a machine is demolished.

An order is refused when it is placed if the line can never move, and the
refusal names the break: the machine you have not built, or the link with
no feeder beside it. Research is refused too, and not as a special case —
Research Data is banked as it is gathered, so it reaches no output buffer
and nothing can hold a stock of it. The Research Node is still staffed, as
a standing job.

### Base staff

Programs are now on one side or the other: in your party, or in the base.
The two are disjoint sets drawing on one roster, which makes explicit a
choice that used to be a side effect of a menu action — every program at a
machine is one absent from a fight.

Staff with nothing to do loiter on a ring around the Home instead of being
invisible, and `x` can name them, because what the map draws and what the
inspector names is one rule.

### Standing jobs

A machine can be told to keep running, and a structure to keep a guard,
whether or not an order asks for it. Standing jobs are filled only by a
body no order needs, and give that body up the moment one does — which is
how a Research Node gets worked at all, and the only way a guard post
survives the sweep that makes it worth having.

The toggle lives on the structure roster rather than in a menu of its own:
the question "should this machine always be running" belongs to the
machine.

### What was removed

"Assign a cronjob" and "Post a guard" are gone from the base menu, and the
engine methods behind them are gone from the engine. **Working a structure
yourself is untouched** — you are not staff, and the scheduler never moves
you.

An existing base keeps its workers. Any program holding a job when a save
loads is absorbed into the staff pool rather than being stood down, and
every new field is additive and defaulted, so **no save-format bump** and
nothing to migrate.

### Not gated by anything automatic

`balance_sim` is RNG-free and models no base at all — it cannot see a work
order, a posted program, or a production chain, and the arena models player
combat. Nothing here is covered by an automatic balance gate, and the
question it raises — whether automating staffing makes the base too
productive for the zone curve — is answerable only by playing it.

## 0.8.34

### Three bugs where an index stood in for the thing it named

All three were the same mistake wearing different clothes: a number that
identified something by *where it sat* rather than by *what it was*, and
then the thing moved.

**An attack aimed at one group landed on another.** A planned action stored
its target as a plain index into the enemy line, but a group that dies is
dropped from that line the moment it dies, sliding everything behind it down
one. Aim at the second group, watch a faster party member destroy the first,
and your strike landed on the third — whoever had moved up into the slot you
picked. The same fault at the tail end sent a plan aimed past the end of the
line onto the front group instead, which is where it was most likely to be
wasted on something already dying.

A plan names a group, not a slot. It now resolves against the line as it
stood when the plan was made, so the aim follows its group to wherever that
group is now standing, or the turn is spent and nothing is hit. Spending it
is deliberate: a heavy hit or a decompile going off on a target nobody
picked is worse than a wasted turn, and a decompile in particular could burn
an ICE Breaker on the wrong program entirely. The wielded program's proc
answers the same aim as the strike that triggered it, rather than the
shifted one.

**Bumping a pack dragged in any boss standing near it.** Assembling the
enemy side of a map fight sweeps in every hostile within a short radius of
whatever you walked into, and that sweep had no idea what a boss was. Bosses
roam the open map like anything else, so one that happened to be loitering
beside an ordinary cluster joined the fight — which is the opposite of what
being a boss means, since a boss is precisely the thing that arrives as its
own fight with its own escort. It is now left out of the sweep. Walking into
the boss itself still fights the boss, and the escort it brings with it is
untouched.

**Creatures spawned onto ground they could never leave.** A spawn checked
that its anchor tile was walkable and then scattered the rest of the group
around it without checking anything, while a creature's own movement is
stricter than plain walkability — it refuses rock, and it refuses your base
slab. A pack member or nest guardian scattered across a biome boundary
therefore had no legal move for the rest of the run, and a nest guardian's
tether meant nothing could ever push it off, either. Both now fall back to
the anchor tile when the scatter lands somewhere unusable.

That last one moves creatures around, which shifts the random stream a
little further downstream. Nothing about a save is affected, and existing
saves load unchanged.

## 0.8.33

### Ten more affixes, three of which charge you for the privilege

The affix pool shipped with eight entries, and two facts about it made a
find repeat sooner than it should have. Only four could land on a Module, so
the slot you keep for yourself was the one whose drops varied least. And
DECOMP had exactly one affix in the whole set, at two points — so a capture
build's only roll was an uncommon one, with no small common version beneath
it the way ATK and DEF both had.

Ten more, filling both:

| Affix | Grants | Slots |
|---|---|---|
| Tempered | +1 ATK | any |
| Patched | +1 DECOMP | Module, Armor |
| Shimmed | +2 DEF | Module |
| Rigged | +2 ATK | Module |
| of Deep Cache | +1 DEF +1 DECOMP | Module, Armor |
| of Sidechannel | +1 ATK +1 DECOMP | Module |
| Volatile | +2 ATK **-1 DEF** | Armor |
| of Deadlock | +2 DEF **-1 ATK** | Weapon |
| of Cold Boot | +1 ATK +2 DEF | Armor, Module |
| of Hot Swap | +2 ATK +2 DEF **-1 DECOMP** | any |

The last three are new in kind: an affix may now cost you something. With
only three stats in the game and a calibration ceiling of +3 on any one of
them, a drawback that merely undercut an existing affix would be strictly
worse than it and nobody would ever want it, however common it was. So each
of the three puts a stat on a slot no clean affix will — ATK on armour, DEF
on a weapon — and bills that slot's own axis for it. Nothing in the set
grants four points across two stats without one; `of Hot Swap` does, and
pays for it in a capture skill your companions do not have, so a copy of it
handed to a program costs nothing at all.

A drawback scales with the run, because the affix is folded into the base
before gear level is applied — a cost that quietly stopped mattering after a
breach would read as a free upgrade rather than as a choice. Fusion and rare
tiers deliberately do *not* deepen it: those are what you spend to improve a
copy, and spending to make your own gear worse on one axis is a trade nobody
would take.

Two smaller consequences. An affix that grants nothing positive is now
refused at load with a warning, the way one granting no stats at all always
was — a mod's pure penalty is a roll no player has a reason to keep. And
every slot is now asserted to have something to roll, since a slot with an
empty pool is one where drops stay exactly as interchangeable as they were
before affixes existed.

Saves are unaffected: a copy that never rolled one of these still names the
affix it has, or none.

## 0.8.32

### Four perks that leave the player's own body

The perk picker was lopsided. Seven of its twelve entries were your own stats
and your own casts, and the other five reached mining, hunger, decompiling and
crafting — so a perk was something you bought to hit harder, and never
something you bought to change how the run worked. Four more, one each into
the four systems the catalogue had never touched:

```
  [13] Obfuscation    3 pts   -10% to every Trace rise, floored
  [14] Process Pool   3 pts   +1 tamed program you may own
  [15] Teardown       4 pts   +1 work resource from every kill
  [16] Failover       2 pts   +1 Durability per repair interval
```

**Obfuscation** makes the Stack quieter: everything that raises Trace — a
kill, a cache, a seal, a breakpoint — costs 10% less per level. It can never
buy silence. Low Power Mode is allowed to stop hunger draining entirely
because a Recharger Node already deletes hunger as a cost, but Trace is the
only pressure the Stack applies, so however many levels are stacked a source
still costs something.

**Process Pool** raises how many programs you may own, through the same
capacity a Data Cache feeds — which means it is the slots a GC Entropy Sweep
cannot take back.

**Teardown** strips a defeated process to the frame: a kill drops 2-4 work
resources, and each level adds one to that. It is the steepest thing in the
catalogue relative to what it modifies, which is what the 4-point price is
for, and it is bounded by the fights you take rather than by a machine that
runs while you sleep.

**Failover** repairs your base with no Patch Node standing. A base without one
took sweep damage that was simply permanent; now it mends a point per repair
interval per level.

Perks are still half data: these four are named, described and priced in
`assets/perks/`, and how much each gives per level stays in `tuning.rs`, where
a mod cannot reach it.

### The perk chart said +3 where the game said +2

Fixed: `docs/perks.md` charted Attacker and Defender at +3 Attack and Defense
per level against a game that has granted 2 since the curves were linearised.
The page is a hand transcription rather than a parser, which is what its own
staleness warning is about — the warning now names this case.

## 0.8.31

### Fifteen more contracts

The authored set was eight, thin enough that a board of three slots repeated
itself inside an hour. It is now twenty-three, spread across every objective
shape and banded by sector so what a Broker offers moves with the run:

```
  Offered
  [1] Coil Order - Deliver 8 Charge Coil - pays 55 Credits, 140 XP
  [2] Guardian Bounty - Terminate 1 Wintermute - pays 130 Credits, 2 ICE Breaker, 340 XP
  [3] Deep Sounding - Stand 5 frames down a Stack - pays 95 Credits, 20 Core Fragment, 260 XP
```

Four of the fifteen are repeatable, so every band keeps something to grind
rather than emptying as its one-offs are finished. Each names something the
sector can actually supply: deliveries are bulk stock only, the builds are
either ungated or behind research a run that deep bought long ago, and the
deepest sounding stops at frame 5, since only a Stack link at the far edge of
a sector's scatter runs any deeper.

### The Research Backlog asked for the bank

Fixed: *Research Backlog* asked you to deliver ten **Research Data**, which is
banked progress rather than cargo. No cargo screen lists it — the research
tree is the one place the number appears — so the errand read as impossible
while quietly being payable out of research you had not spent yet. It now
asks for ten Logic Wafers, which is the same readings transcribed.

A banked item is no longer something a contract file can name by accident;
`assets/contracts/README.md` states the rule for mods, and the shipped set is
checked by a test.

## 0.8.30

### Rolled contracts

The Broker's board was eight authored contracts and nothing else, so a long
run saw the same offers come round again. It now also carries **templates**:
contracts with holes in them, filled in against whatever the sector can
actually supply.

```
  Offered
  [1] Hunt: Sub-Process - Terminate 9 Sub-Process - pays 63 Credits, 180 XP
  [2] Requisition: Blank Substrate - Deliver 14 Blank Substrate - pays 28 Credits, 70 XP
  [3] Sounding: Frame 4 - Stand 4 frames down a Stack - pays 45 Credits, 180 XP
```

A rolled contract is not a second kind of contract. It becomes exactly the
thing an authored file parses into, so it is accepted, tracked, finished and
paid by the same code — an authored contract is simply a template with no free
variables. Five ship, one per objective shape, and `assets/contracts/README.md`
documents the format for mods.

**A rolled contract is always finishable.** It draws its species from the
programs living on your base's doorstep, its delivery from stock this sector
can actually produce, and its structure from what you have unlocked and do not
already own. When a sector can supply nothing valid, the template rolls nothing
and the slot goes to something else — an empty slot beats an errand you cannot
run. Offers stay put across a save and load, like the rest of the board.

### Contracts you had already finished

Fixed: the board could offer a contract the run had **already met**, which paid
out in full the moment you accepted it. A base with a Refinery standing was
offered *Stand Up a Refinery* — 45 Credits, five Power Cells and 140 XP for
pressing a key — and a run in sector 3 was offered *Reach sector 3*. Both
shipped in 0.8.29.

### Deliveries are bulk goods only

A delivery is asked for by the score, so it now names only cheap, accumulable
stock rather than anything in the catalogue. Without this a Requisition could
ask for twenty etched Routine Disks, which is a run's worth of research stated
as an errand. Portal Fragments were never askable and still are not: they are
the breaching currency, and their only source is a boss at the bottom of a
stack.

### Terminate, not Kill

`Objective::Kill` is now `Objective::Terminate`, in the asset schema and in the
line you read on the contracts screen — "Terminate 6 Drone" rather than
"Defeat 6 Drone". A mod naming `Kill` needs the one-word edit.

This does **not** bump the save format. The variant is serialised inside a
contract you are holding, so the only save it could break is one carrying an
accepted contract — and contracts shipped one release ago behind a research
buy and a build, so no such save exists. Bumping would have refused every real
save to protect a file that cannot be written.

### For testers

`cargo run -- --template contracts` opens standing at a Broker, in the `chains`
world with its research unlocked and its cargo deep, so the loop can be played
without first buying Contract Brokerage and building the Broker.

## 0.8.29

### Contracts

The game had no statement of what to do next. Research is a shopping list,
achievements are cross-run and one-shot, and the zone ladder says only "go
deeper" — so a player who had stood their base up and beaten a stack had no
answer to "and now?" beyond doing it again one level down.

Contracts are that answer. Build a **Contract Broker** (glyph `!`, unlocked by
the new Contract Brokerage research node), stand beside it, and the sector
tells you what it is paying for: thin out the drones, deliver twenty-five Core
Fragments, stand three frames down a stack, breach to sector 3. Take up to
three at once. Each pays in full when you finish it.

```
  Held
  [1] Clear the Nursery - Defeat 6 Drone [4/6] - pays 40 Credits, 120 XP
  [2] Fragment Quota - Deliver 25 Core Fragment [0/25] - pays 35 Credits, 90 XP
  Offered
  [3] First Descent - Stand 3 frames down a Stack - pays 50 Credits, 200 XP
```

**What a Broker offers is derived, not stored.** It comes off the world seed,
the sector and the clock, so the same three offers come back after a save and
load, cannot be rerolled by reloading, and rotate on their own as the run goes
on. Contracts already taken or already finished drop off the board.

Four of the five objectives finish themselves wherever you are, including four
frames down — a contract you are holding is readable anywhere, even where
there is no Broker in reach. Deliveries are the exception: those items are
handed over at the Broker, and it takes only as many as the contract still
needs.

**This deliberately amends "progression is earned by fighting."** XP is a
legal contract reward on any objective, including delivery and construction,
so what advances you can be the thing the game asked for rather than whatever
was nearest. What survives unchanged is the narrower rule underneath it:
Portal Fragments are still earned only by fighting and descending. They are
not a contract reward, and there is no variant for them to be one through.

Contracts are data — `assets/contracts/*.ron`, one file per contract, with
`assets/contracts/README.md` as the schema. Delete the directory and the
board is empty, which is exactly the game as it was.

The reward figures are opening guesses. `balance_sim` cannot see a contract at
all, so nothing gates them but play.

## 0.8.28

### A beaten stack collapses behind you

Killing a stack's guardian used to leave everything exactly as it was: the
link still open, the frames still walkable, the lair a cleared room at the
bottom of a maze with nothing left in it. The deepest thing you can do in the
game ended with a long walk back up through rooms you had already emptied.

It now ends the stack. The floor gives way, the party is flung up through
collapsing frames onto open grid, and the link you walked in through caves in
behind you. The run's record of the place goes with it — there is no map to
come back to, because there is nowhere to come back to.

```
  The stack folds in on itself. You are flung up through collapsing
  frames and land hard on open grid.
  The ground answers somewhere else: a new link opens south-east at 7 tiles.
```

**The sector never loses a link.** The replacement opens on the nearest legal
ground before the old one comes down, so a zone always has a way back
underground — which matters more than it sounds, because a Stack boss is the
only thing in the game that pays a Portal Fragment, and a zone with no link
left is a run that can never breach again. The new stack's depth follows the
same rule every link does, read off how far out it lands, so finishing a deep
one nearby tends to hand you back a shallower one.

The trade is that a collapse forfeits whatever you left behind: an unopened
cache two frames up, a seal you never shouldered, an orphan in a dead end.
The lair sits on the bottom frame, so you have walked the whole stack by the
time you reach it — killing the guardian is now the deliberate end of that
stack rather than a step you can walk back from.

## 0.8.27

### One key fuses every matching pair in cargo

A long run leaves the buffer full of duplicate gear, and turning it into
fused copies meant opening each stack's action page and pressing `[U]` once
per pair — a dozen keypresses and a dozen turns for work with no decision in
it. `[U]` on the inventory screen now does the lot at once:

```
  You fuse 5 pairs:
    2 Ablative Plating -> tier 1
    2 Kinetic Edge -> tier 1
    1 Overclocked Arc Lance -> tier 2
```

It is **one pass, not a cascade**: four ordinary copies come out as two T1s
rather than one T2, and an odd copy is left ordinary. What you get is one
rung up the ladder for everything you had a pair of, which keeps the deeper
tiers a choice about where to spend the spares rather than something a
convenience key decides for you. A copy you are wearing still counts as one
of its pair, exactly as it does on the per-item page, so pressing this once
and pressing `[U]` down the list are the same fusions.

The whole batch costs **one turn**, not one per pair. Charging need decay,
sweep pressure and spawn rolls per fusion would make the shortcut cost more
than the typing it saves. A press with nothing to fuse is refused and spends
no turn at all.

## 0.8.26

### The run-a-routine picker says who needs it

Every field routine that lands on one ally is about a stat — Repair Loop
about Integrity, Overclock about Attack, Hardened Shell and Ablative Layer
about Defense. The screen that asked you to choose a body listed names and
levels and nothing else, so the one decision it exists for had to be made
from memory. Each row now carries the numbers the buff is about to touch:

```
  Run Overclock Single on whom?

  @ [a] You Lv12 - HP 88/120  ATK 24  DEF 18  PWR 130
  p [b] Kestrel Lv5 - HP 40/40  ATK 12  DEF 9  PWR 61
  p [c] Sable Lv7 - HP 12/58  ATK 15  DEF 11  PWR 84
          replaces Repair Loop Single HP+7/4t — 62t left
```

That last line is the other half. Running a routine on someone who already
has one of the same kind *replaces* it rather than stacking with it, and
nothing on the screen said so — so the tag names what is about to go, and
names the routine rather than saying "already running", because two
different routines can arm the same kind. It is deliberately narrower than
"what is running": a buff armed by a consumable survives the cast, so
listing it would be a lie about what you are about to lose.

## 0.8.25

### The extract picker names what each program is carrying

Breaking a program down destroys it for exactly one of its routines, and
until now the screen that asked you to choose said nothing about what was
inside any of them — the only way to find out was to open each program in
turn and back out again. Each row now carries its kit on the line beneath
it:

```
  s [a] Sprite Lv7
        Patch Routine (known), Static Burst
  g [b] Glitch Lv4 ++
        Priority Boost (known), Overclock (known)
```

The `(known)` tag is the part that decides it. Extraction *refuses* a
routine you already know, so a program whose whole kit is tagged is worth
nothing on the block — and that was invisible until you had walked two
screens in to find it.

The list is the same call the next page makes, so the two cannot describe a
program two ways, and the routines shed onto their own lines rather than
running off the popup's right edge. That wrapping is now one piece of code
shared with the fuse picker, which grew the same lines a release ago.

## 0.8.24

### The battle roster says what each member is wearing

The party's side of the intrusion screen gains a `GEAR` column, between
`POS` and `FATIGUE`, carrying the same `w|a|m` loadout cell the roster and
the status panel already show — a letter per filled equipment slot, a dot
for an empty one. A companion you kitted out and one you never got round to
are now told apart in the place it costs you to find out the hard way.

It is the engine's cell rather than one the renderer assembles, for the
reason the other two screens' is: a loadout that reads one way in a fight
and another on the sheet you set it from is worse than not showing it. And
it is fixed-width by construction, so `FATIGUE` and `ACTION` sit at the same
column whether a row is fully geared or completely bare.

## 0.8.23

### The etch screen is somewhere you can get to, and says what you already hold

Etching is where a blank Routine Disk becomes a routine you can install, and
it was buried: the only way in was *Install a routine* → pick a holder →
pick an **empty** slot → `e`. Every routine slot in the game starts full, so
a player who had never popped one out could not reach it at all. The party
menu now opens it directly.

It also answers the question it exists to ask. A blank is spent for good, so
each routine says how many finished disks of it are already in cargo — you
find out you have three before you burn a fourth, not after:

```
Burn which routine onto a blank? The blank is gone either way.
Blanks: 4

[1] Bastion Single v1.0            ×2 held
    +3 DEF to one ally for 3 rounds
[2] Overclock Single
    +4 ATK to one ally for 90 turns
```

**Fixed:** the last routine on that screen showed no description. The
popup's scrolling list ends at its last pickable row and pins whatever
follows to the bottom of the box, so the final description was drawn adrift
under the scroll indicator instead of under the routine it belonged to. The
slot panel, the install picker and the extraction picker all had it too, and
all four are fixed together.

## 0.8.22

### The fuse picker names what each program is carrying

Fusing two programs builds the result's routines fresh from its species, so
anything installed on either parent — researched, extracted, swapped in off
a disk — is gone. The game said so, but only on the last page of the flow,
after both picks were already made.

Both pickers now say it while the picks are still free. Each candidate
carries its routines on the line beneath its stats:

```
[a] Kestrel Lv6 - HP 22/28  ATK 8  DEF 5  PWR 19 (in party)
       Hyperthread Single v1.0, Sandbox
```

A full six-slot kit is wider than the popup, so a long list wraps onto
further lines rather than running off the right edge.

## 0.8.21

### A fight tells you what it paid at the end, not between the blows

Killing something used to interrupt the fight to tell you about it. Every
kill put a loot line and an XP line into the battle log, so a pack of five
scrolled the blow-by-blow past you behind ten lines of bookkeeping — and the
one thing you actually wanted, what the whole fight came to, was scattered
across all of them.

A fight now closes with the answer. One salvage tally, with copies of the
same thing counted together, and one experience line per fighter carrying
the fight's total and the stats it bought:

```
The rogue program crashes and deletes itself!
Salvage:
  1 Bastion Lattice of Static [ARM]
  1 Black ICE Pick [WEP]
You gain 1717 XP, reaching level 23.
  Max HP 318 → 354
  ATK 46 → 49
  DEF 42 → 45
  Perk Points 0 → 3
  Decompiler 9 → 12
```

Two things stay where they were. Nothing is *paid* any later than before —
a level still lands on the kill that earns it, and still heals you there, so
a fight plays out exactly as it did. And because a level heals you, reaching
one is still announced the moment it happens, briefly: your HP bar snapping
back to full mid-fight is something you need explained then, not afterwards.

Running from a fight pays the tally too. You keep what you killed before
you left.

An Overclocked copy is counted apart from an ordinary one rather than summed
into it, so the tally never tells you that you found two of a thing when one
of them was the good one.

## 0.8.20

### Staff a machine from the screen that told you it was idle

The structure roster now does something. Highlight a machine, press Enter,
and pick who works it — the roster is the only screen that shows the whole
base at once, and it has always drawn an unstaffed machine in yellow, so it
is where you find out that something is sitting idle. Until now the only
thing you could do about that was close it, open the base menu, pick a
program, and pick the machine again from a different list.

Picking drops you straight back on the roster, on the same row, with the
program you just posted listed under it. That is the point of doing it here:
the answer appears where you were already looking, and staffing three idle
machines is three presses of Enter rather than three round trips through a
menu.

The list leads with **Yourself**, which puts you on the machine by hand —
offered only when you are standing on one of the four tiles beside it, since
that is the only place working it yourself has ever been allowed. Everything
else on the list is a program you own, shown with the same level, power and
current errand the cronjob picker shows.

The old flows are untouched. "Assign a cronjob" still answers *where do I put
this program*, which is the right question when you have a program in mind;
the roster answers *who goes on this*, which is the right question when you
have just spotted an idle node. Posting a guard stays on its own screen, and
Enter on something that takes no worker — your Home, a Shield — says so
rather than doing nothing.

Underground the roster still reads exactly as before, but Enter does nothing:
while you are in the Stack the game holds your position at the entrance tile
you climbed in through, so a posting made from down there would measure its
walk from the wrong end of the map. The hint at the foot of the screen stops
offering the key rather than refusing it after the fact.

## 0.8.19

### A base you build outward

A base starts half the size it used to — a 9x9 slab of about 69 buildable
tiles rather than 15x15 — and the **Heap Pillar** grows it back, one ring at
a time. Build one and the edge creeps out a tile in every direction; build
another and it creeps again, up to five of them — a base ends at 19x19,
better than twice the ground it opens with. Five is a number in the
structure's own file rather than in the engine, so it is one edit to change
and any structure can declare a limit the same way. It costs 14 Core
Fragments, sits behind a new research node (Heap Allocation, 30 Research Data
off Power Grid), and cannot be demolished.

Like anything else, a Pillar is deployed onto one of the four tiles beside
you, so growing the base is a walk-and-build loop rather than a menu you
hold down.

The complaint this answers is feel rather than capacity. A real base does not
fill 213 tiles, so the old slab was the same size in the first minute of a
run as in the tenth hour and nothing you did ever made it bigger. Growing in
single-tile steps is the whole of the settlement reading: the edge moves
often, by a little, and you are the reason.

Irreversibility is the design rather than a limitation. A Pillar that could
come down would leave structures standing outside a shrinking slab, and there
is no good answer to that — so there is no shrinking slab. Demolishing your
Home still cascades over everything, Pillars included, and the base resets to
its starting size.

Growth claims the ground it takes, exactly as deploying a Home always has:
wild programs and nests in the new ring go, and so does a Stack link, with a
line in the log naming the tile. The one thing it will not bury is the
sector's **last** link — the Stack is the only place Portal Fragments come
from, so a base that swallowed the last way down would end the run's
breaches — and that refusal comes before it charges you.

Two knock-on fixes come with it, and one of them would have ended runs. A
zone's Stack links were drawn from boxes around your arrival point, and a
slab wide enough swallows those boxes whole — at which point the placement
loop spends its whole shared attempt budget failing to land the first link
and the sector gets **none at all**. No links means no Stack, no Stack means
no Portal Fragments, and the run can never breach again; it would have read
as a bad seed rather than as a bug. Every link is now drawn from a band
starting just outside the base, whatever size the base is — the on-ramp from
a narrow band and the other two from a wide one — which also repairs a
squeeze that existed at the old radius.

One thing a very large base does change: a program can only be posted to a
machine within sixty tiles of where you are standing. The walk a posted
program takes is a search whose cost grows with the square of its reach, and
past that point a single walking worker costs more per turn than the turn is
worth. The cronjob menu refuses the post rather than accepting one that
never arrives, and tells you to get closer. And stack depth is measured from the edge of
your base rather than from its centre, so growing does not quietly make every
descent in the sector deeper.

The opening nursery is unchanged at 7 tiles. It used to be defined *as* the
build radius, which would have shrunk it for the opening minutes and then
widened it every time you built a Pillar — a difficulty knob keyed to base
geometry, which is the thing removed when distance stopped scaling anything.
Home's rest radius and the Recharger Node's reach now cover a fully grown
base instead of a fixed 7 tiles.

Existing saves load untouched, and keep the base they already had. Nothing
about the footprint is stored — a base's width is rediscovered from the
structures a save already carries, under a rule that holds everywhere: the
slab always covers every structure standing on it. Without that, a base
built before this release would have kept its old floor while only the inner
9x9 accepted a building, which on a real save meant 156 tiles that looked
exactly like base and refused to be built on, with you standing on one of
them. The one wrinkle, and it costs real Core Fragments so it is worth stating
plainly: on a base built before this release, a Pillar buys **nothing** until
the bonuses have caught up with the width that base already had. A base
sitting at the old radius of 7 absorbs three of them before the fourth moves
anything. Only a pre-release save can be in that position, and a base built
from here grows on the first one.

## 0.8.18

### Rotten substrate can kill you now

A step onto corrupted ground in the Stack costs 10% of your maximum
Integrity, up from 3%. A patch is three cells, so walking one end to end is
about a third of the bar, and turning round and walking back out is most of
the rest.

At 3% a patch was a toll. It could not kill, so the only question it ever
asked was whether the way round was longer than a tenth of your health, and
the answer was almost always no — you paid it and forgot it. At 10% a party
that is already hurt can die on the third cell, which makes the detour a
decision you can get wrong. That is the whole reason the Stack has a second
kind of walkable floor.

Nothing else moved. Corruption still comes as two patches of three per
frame, sparse enough that most routes miss them; it still measures against
your maximum rather than your remaining Integrity, because Stack depth is
uncorrelated with your level and any flat figure is lethal at one end of a
run and free at the other; and it still goes through the one damage path in
the game, so a Mitigation field buff blunts it exactly as it always has.

## 0.8.17

### The roster's row shortcuts stopped standing programs down

`P` now adds the highlighted program to your party, or stands a member back
down. A row's number or letter only moves the highlight, and Enter — a key
this screen has never had a use for — does nothing at all.

Every other action on the roster already worked this way: `<` and `>` shift a
member along the battle line, `N` renames, `E` fits gear, all of them reading
the highlight. Party membership was the exception and the only destructive
one, so the screen answered a digit typed at the wrong moment by pulling a
program out of the party, with nothing to undo it but noticing. The help
lines name the new key, and `the_companion_screen_names_the_party_key` is
what keeps them naming it — with the row shortcuts inert, that line is the
only thing pointing at the one action this screen exists for.

## 0.8.16

### Counts read down a column instead of trailing the name

Every screen that prints a quantity now leads with it — the inventory list,
the base pane's cargo column, a trader's sell and buyback rows, and the Stack
market's — instead of appending `x3` after the item name. The column pads to
three digits, so the names form a straight left edge and a stack of 4 is
distinguishable from a stack of 40 at a glance rather than by reading to the
end of each row.

`qty_column` is the one definition of that column, and it lives in app-core
beside `equip_preview_tag`, which those same five screens already share. A
count that read one way on the screen you checked and another on the screen
you sell from is what costs a player a copy they meant to keep. It grows past
three digits rather than truncating: a wrong quantity is worse than a wide
row.

### A program's gear is readable from the list it's in

The roster and the status panel now carry a `w|a|m` cell per program — one
letter per filled equipment slot, a dot holding the place of an empty one.
Both screens get it from one function, so a loadout cannot read one way in
the panel and another in the roster you opened from it.

The cell is fixed width for the reason it exists at all. These lists include
programs you never open a gear screen for — a posted worker, a bench-warmer —
so "which of these is still bare" has to be answerable by scanning down a
column, and a cell that shrank when a slot was empty would leave nothing to
scan. It sits directly after the stats and ahead of every optional tag
(quality, fusion depth, the wield mark, the activity), because those come and
go per row and a column placed after one of them lines up only with the rows
that happen to carry it.

### Fixed

Nothing. Two overflow measurements came out of this work and neither is
fixed: the widest shipped inventory row and the widest roster row both run
off the right edge of their popup, by 68px and 393px. Both predate these
changes and both are recorded in `TODO.md` — the fix in each case is a
decision about which tag loses, not a shorter one.

## 0.8.15

### The research tree no longer finishes early

Twenty-one nodes, 561 Research Data, and nothing about any of it gated on
progress: every node was buyable from turn one if you were willing to wait,
and Research Data survives a breach. So the tree was paced by patience alone,
and it read as a checklist to clear rather than a set of decisions. You had
everything researched long before the run was over.

Raising the prices alone would have been the same checklist with a bigger
number on it. **Twelve nodes now declare the zone they open in** — six at
zone 2, six at zone 3 — and the ladder reprices from 561 to 1258.

Below its zone a node is **still listed**, still priced, still described,
and unbuyable at any balance; the row says what it is waiting on. That
visibility is the point rather than an oversight. A zone-3 tier sitting in
the menu while you are in zone 1 is the reason to go breach, and hiding it
would mean a player who never breached never learns the tier is there. It is
the same argument the upgrade menu already makes for a structure stalled at
its zone ceiling. A node held up by both a prerequisite and a breach says
both: *(needs Neural Interfacing, Zone 3)*.

The opening nine nodes are **nearly untouched** — 129 Research Data becomes
158. The complaint was that the tree finishes early, not that the first bench
arrives early, and a base that cannot stand up its first machine is a worse
opening rather than a slower one. The two dearest bands are where the change
lands: 163 becomes 350, and 269 becomes 750.

The tap is deliberately unchanged. A Research Node still pays a flat 1 per
cycle into a bank with no ceiling, which means the ladder is the only thing
bounding it — giving the tap a zone term would have paid out more at exactly
the moment the gate released more to buy, cancelling the pacing this exists
to create. What compounds instead is something already in the game: a
Research Node is capped at Mk1 in zone 1, Mk2 in zone 2, Mk3 in zone 3, and
its cycle succeeds 50% of the time at Mk1 against 90% at Mk5. The band you
can buy earliest is the band you earn slowest, and every breach speeds the
bank up at the same moment it releases more to spend it on.

The bands are content: `min_zone` in `assets/research/*.ron`, one optional
field that defaults to ungated. A mod's research file that never heard of it
keeps parsing and ships a fully open tree exactly as before, and retuning
which node sits in which band is a `.ron` edit with no code change. Two rules
are enforced against the loaded tree rather than left to care — a node may
not be gated below its own prerequisite, and nothing unlocking the Zone
Portal may be gated at all, since that is the structure you reach the next
zone *with*.

**Existing saves load untouched.** The gate is on buying, not on having, so
a save that already paid for a node keeps it whatever zone it is standing in.

## 0.8.14

### A breach now lands you somewhere

Breaching used to change numbers and nothing else. A zone-7 sector was
generated by exactly the same rules as zone 1 — the same six biomes at the
same five thresholds — so the deal on offer was: lose your stockpile, keep
your base, fight harder things, get Mk+1.

Every zone past the first is now a **sector** with a character of its own.
Cold Storage is frost-locked, Static Field spreading over ground that is
almost empty of it anywhere else. Fractured Allocation is more gap than
ground, awkward to cross and awkward to build in. Null Expanse is unmapped
pages to the horizon. A breach announces where you have landed, and the map
shifts its colours to match.

The mechanical half is one knob: a sector moves where the world generator's
biome boundaries fall, and everything else follows from that without a second
setting to disagree with it. What lives there follows, because the wild
roster is filtered by the biome of the tile a program spawns on. Where you
can build follows, because a hole in the map is a hole in the map. There is
deliberately no separate species-pool bias and no per-sector difficulty.

**Zone 1 is always neutral**, whatever is installed. The opening zone fields
only programs a fresh player can actually beat, and biasing its biome mix
would have moved that roster while looking like a cosmetic change.

Sectors are content, not code: `assets/sectors/*.ron`, one file per sector,
documented in that directory's `README.md`. A mod adds one by dropping in a
file, and deleting the directory restores the previous game exactly — the
same supported way to play that removing the affix pool or the trained enemy
policy already is. Two load-time checks refuse a file rather than shipping a
broken world: a sector that would leave too little standable ground to
materialize on, and a palette that would tell you a hole in the map is safe
to walk into.

Which sector a zone gets is derived from the world seed and the zone number,
both of which your save already holds — so this needed no save-format change,
and **existing saves load untouched**. One consequence worth knowing: in a
run that had already breached, ground you have not yet explored may generate
under the new thresholds, so a map can change shape at a boundary you have
not walked to. Everywhere you have been, and everything you have built, is
unaffected.

## 0.8.13

### Every screen now prices the affix it is naming

Gear picked up on the Grid can roll an affix — an Overdriven weapon, a
Reinforced plate, something of Static — worth a small flat bonus on top of
what the item already grants. That bonus is added before gear scaling rather
than after, so an affix grows with the run instead of dwindling into a
rounding error, which is the whole reason to be pleased about finding one.

The numbers on the screens were not being told any of this. The swap picker
would name a candidate "Overdriven Kinetic Edge" in one column and, in the
next column along, price it as a plain Kinetic Edge. Because the affix is
folded in before the level multiplier, the gap was not a fixed point or two —
it grew with how deep you were. At zone 3 that row promised +6 ATK where
putting it on actually granted 15. At zone 5 it read +10 against 25.

The same blind spot ran through the inventory tag, the delta column, the
`(Unequip)` row — which understated what taking a good weapon off would cost
you — and the equipped panel, which dropped the rare tier as well and could
report an Overclocked Overdriven Kinetic Edge worth 27 ATK as being worth 6.

Nothing about your character changed: the stats you were actually fighting
with were always correct, and no save is affected. What changed is that the
screens now agree with them, so a comparison between two pieces of gear is a
comparison you can trust — and an affixed find no longer looks like a
downgrade to the plain copy already on your back.

## 0.8.12

### A companion's gear, and the machine it is standing at, on its own sheet

The manifest is the page you open to find out what something *is*, and for a
program you own it had two holes in it.

Gear was the first. Any program you own has been able to wear a weapon,
armour and a module since 0.8.0, but the manifest's EQUIPMENT box was the
player's alone — the only way to see what a companion had on was the Program
Gear screen you equip from, which tells you what is in the slots and nothing
about what those slots are currently worth. A companion's page now carries
the same box the player's does, measured the same way: each item at the zone
level and fusion tier recorded when it went on, not at a fresh preview of
today's. A program wearing nothing shows no box, the same way an empty slot
has never been listed as "(none)".

The post was the second. A program on a cronjob did say so, as one entry in
the run of tags under its name — but a worker's tag is the bare structure
name, which beside "Lv 14" and "Excellent (91%)" reads as decoration rather
than as an assignment. The WORK box, which is already the box about what a
program is like to *post* somewhere, now states it outright: **Posted to
Mining Node** for a worker, **Guarding Shield Wall** for a guard, and no row
at all for one that is idle or in your party. The verb is the whole
difference between the two jobs, and it is the reason the row is not one
label for both.

Nothing about how gear or posts work has changed — this is what the screen
reports, not what the game does. Existing saves load untouched.

## 0.8.11

### Decompiling reads the gap between you and what you are pointing at

Wearing a program down before spending a catalyst on it has always been most
of the odds, and against anything near your own strength it still is. Against
trash it made no sense. A program far enough beneath you dies to a single
strike, so it can never be shown to you at low Integrity — and the attempt was
priced as though you had looked at the option and declined it. Decompiling
something harmless was therefore *worse* than decompiling something dangerous,
which is backwards, and it made the easiest programs on the Grid the most
annoying ones to collect.

The gap now counts. Once a target is far enough below you that it reads Green,
its remaining Integrity stops entering the odds at all: there is no softening
to reward, so you are no longer charged for skipping it. With the shipped ICE
Breaker that takes a healthy Green-con drone from roughly one attempt in eight
to closer to one in two. The relief fades in across the Yellow band rather
than switching on, so an even fight is very nearly the fight it was.

The same reading runs the other way. A program more than half again your
strength — the ones already drawn in Red — gets harder to decompile the
further above you it is, down to a floor of 60% of the odds at two and a half
times your power. Nothing becomes impossible; a long shot stays a long shot.

The number driving both is the one already on your screen. It is the same
comparison that paints a wild program Green, Yellow, Orange or Red, so the
color you are looking at and the odds you are quoted can no longer disagree
about which of the two of you is stronger.

One consequence worth knowing if you have bought into it: **Exploit Focus does
nothing against a target you badly outclass.** The perk cuts the penalty for
attempting a healthy program, and against a Green-con one that penalty is now
already gone. It is worth exactly what it always was everywhere else, which is
where it was always meant to matter.

## 0.8.10

### Examine looks down a line

Pressing `x` and picking a direction used to search a 90° wedge out to forty
tiles. Anything leaning your way counted — a thing forty tiles east *and*
forty north was "east" — and forty tiles is more than twice what the map pane
shows in either direction, so the inspector regularly opened a sheet for
something you could not see and had no way to find. It now looks down the row
or column you are facing, one tile wide, twelve tiles out. A creature one tile
off your row is missed, which is the price: you can step or turn, and what `x`
names is now what was in front of you.

Two things came out of the same rewrite. Two candidates the same distance away
could resolve differently between runs, because the scan asked for "whichever
came back first" from a source that does not promise an order — so the same
press could give two answers on two loads of the same save. And aiming at a
machine with a program posted to it would open the *program's* sheet, which
was odd on its own and stranger still because a program at its post is not
drawn on the map: the answer was something invisible standing one tile in
front of the thing you were pointing at. The inspector now names only what the
map draws, and a tile holding both a machine and a program names the machine.

That leaves a posted program with no sheet of its own, so its machine's sheet
now carries its level and health beside its cronjob progress. The `B` roster
shows the same, since both screens are built from one place.

Nests, Stack entrances and zone portals are still passed straight through —
they draw a glyph but are not yet things the inspector can name, so aiming at
one reports whatever lies beyond it.

## 0.8.9

### A dropped weapon can come up rare, and can carry a name

Gear that drops now rolls two things it never used to. The first is a rare
tier, off the same five-rung ladder a wild program rolls on — so an
Overclocked Arc Lance is exactly as rare as an Overclocked program, and the
word means one thing wherever you read it. The ladder itself grew two rungs
at the top, Unrolled and Bare-Metal, for gear and programs alike.

The second is an affix: a name fragment and an extra stat, giving you
"Overclocked Arc Lance of Static" — the colour you read off the row and the
words you read off the name, rolled independently. A tier is the chase, at
about one drop in thirty across the whole ladder. An affix is the variety,
at about one in five, which is what stops the other drops being the fourth
copy of something you already have. An affix's stats are added before the
gear scales, so a good one grows with the run rather than dwindling.

Only *found* gear rolls either. Crafting, buying and buying one back are
deliberately excluded: what you go looking for should beat what you can
shop for. A surface boss now pays at Optimized or better, since that fight's
whole job is to hand you something better than the ground you crossed to
reach it — where a Stack lair pays progression and a nest pays roster.

Affixes are content, not code. `assets/affixes/*.ron` ships eight, a mod
adds one by dropping in a file, and deleting the directory restores the
previous game exactly, down to the random number stream.

Existing saves load untouched. A copy you were already carrying reads as
ordinary and unaffixed, and gear you sold before this release still buys
back as the copy you sold.

## 0.8.8

### A key pressed while the map log scrolls in is no longer eaten

Battle results scroll in a line at a time after a fight, and a keypress
during that reveal skips to the end rather than acting — which is right, on
the battle screen. The reveal range was never closed, though, because the
results are still arriving when the fight ends. So `battle_log` went on
growing with ordinary map and base news for the rest of the run, and the map
believed it was still revealing something.

The result was a key silently spent for every line a running base logged.
Stand next to a working machine, press a direction, and the step would
sometimes just not happen — with nothing on screen to say why.

The reveal now applies only on the battle screen, gated in the one place
both the "is it revealing" check and the line count read. The map's log pane
stops holding lines back too, which was the same leak seen from the other
side.

## 0.8.7

### Somebody is selling things four frames down

A Stack frame can now stand a market on one of its junctions — not every
frame, and never twice — where somebody nobody asked about is trading out
of a folded tarp. Press `t` on the cell.

They are ephemeral in the way the word means. There is **no buyback**: a
surface Market keeps what you sell it and offers it back at double, and
this one keeps no record at all, so what leaves your pack is gone. And the
shelf does not refill — a row that has been bought stays bought for the
run, and a stall with nothing left on it packs up and reads as plain
corridor in both Stack views, exactly as an emptied cache does.

What is on the shelf is two routines, each at three sizes of bundle:
enough disks for **one program** at 150 Credits, for **your party** at 300,
or for **everything you own** at 1000. What is being sold is the *writing*,
not the routine — nothing here teaches you anything, so research and
extraction remain the only two ways to actually know one. The disks go
straight into cargo, so a party with every slot full can still buy, and who
ends up running them is a question you answer later at the routine panel.
Some markets also carry a program, priced off what it would spawn at as
this depth, compiled to your control on the spot.

Two things a trader down here will not sell. The 28 hunt-only routines
stay hunt-only — a shop selling those is the "buy it instead of hunting
for it" shortcut they exist to prevent — and no boss, for the reason an
orphaned process is never one either.

What is on a given shelf is a function of the stack, the entrance and the
depth, so it survives a save and a reload: you can look at a price, climb
out to go and earn it, and come back to the same stall.

### Some routines are not for sale, or for research, or for anyone

A routine now reaches a slot in two steps rather than one. Etching burns a
blank Routine Disk with something you know and hands you an **etched disk**;
installing spends that disk on a slot. Popping the routine back out still
returns nothing — the disk went at install. The routine panel's install page
lists the disks you are carrying, and `e` there opens the etch page.

That split is worth a paragraph only because of what it makes possible.
**Six routines exist that nobody can learn**, and therefore nobody can etch.
No research node teaches them, no species is born running one, and no shop
lists one at a bundle price. The only way to hold one is to hold the disk
somebody else already wrote — and there are exactly two of those.

Wintermute drops **Kernel Shear**, which tears a whole group open and leaves
every one of them bleeding; **Null Cache**, which drains a group and returns
every point of it to you rather than a third; and **Deadman**. The Overseer
drops **Hard Fault**, two rounds of nothing across every hostile on the
field; **Long Winter**, a quarter off all incoming damage for three hundred
turns; and **Watchdog**. Roughly one disk a kill, and a Stack trader deep
enough down will occasionally have one on the shelf for 1400 Credits — far
more than six copies of anything you could have made yourself.

Deadman and Watchdog are a new kind of thing. They sit in a slot, appear in
no menu, and never take a turn: Deadman goes off when one of yours goes
down, and Watchdog clears the condition off everybody the moment one sticks
to its holder. A deadman's switch fires precisely because nobody is holding
it any more.

Breaking a program down at a Compiler pops an exclusive routine's disk back
out intact instead of teaching it, which is the only way to move one — and
it still costs the whole program. There is never more than one copy.

Two smaller things fell out of it. `t` underground used to open the
*surface* trader picker, which scans from a position pinned to the entrance
tile — so it would cheerfully offer to trade with a base four frames
overhead; it now answers about the cell you are standing on. And the frame
map's glyph table is exhaustive, so the next cell kind added cannot ship
drawing as bare floor.

## 0.8.6

### The log pane's header lists every filter

The header read `LOG [Field]  F to filter`, which names the active setting
and nothing else — not what the other two are, not which way the key steps.
Seeing a base line under a header you believed said Field then leaves no way
to tell whether the filter is broken or the line is tagged wrong; the honest
reading, that you had cycled one press short, is the one the header gave you
no way to check. It also advertised `F`, which is bound to nothing: the key
is `f`, and reaching for shift did nothing at all.

It now reads `LOG  All · Field · Base   f to cycle   12 base hidden`, with
the active filter in bold green against the other two in dim grey. Two
colours on one line means the header is drawn as `Painter::ui_runs`
segments rather than a single string, so `log_pane_header` returns styled
pieces.

The options are listed in `LogFilter::ALL`, and a test pins that order to
the one `LogFilter::next` walks — a row of options the key steps through in
some other order would be worse than naming none of them.

Unchanged: the history screen (`L`) is still deliberately unfiltered, and
the counted "N base hidden" tail still only appears when the suppressed
channel has traffic in it.

## 0.8.5

### A compile row says what kind of thing it makes

The Compile screen listed each recipe by name, by what it grants and by what
it costs, and never said what the thing actually *is*. A Handshake Forge and
a Bytecode Block read the same way, so the only way to tell the piece of gear
from the machine feedstock was to already know — or to compile one and go
looking for it in cargo.

Every row now leads with the same kind tag the inventory and a trader's
shelf have always shown, and the list is grouped by kind, so the tag reads as
a heading for the run of rows beneath it: everything you spend, then
everything you wear, then everything you hoard. A modded item gets its tag
for free, since the tag comes off what its file already declares.

### Long rows stop running off the edge of a screen

The four recipes priced in Portal Fragments — Singularity Matrix, Nullsteel
Plate, Oracle Core and Phase Carapace — were too wide to fit the Compile
popup, and had been since they were added. The right-hand end of the
ingredient list was simply drawn past the border and off the panel, so the
recipes that cost the most were the ones you could not read the price of.

A row that no longer fits now wraps: the item keeps the line with its
shortcut on it, and its ingredients continue on the line beneath. The
Recipes screen wraps the same way if anything there ever grows that wide.

## 0.8.4

### The Recipes screen says why you would make the thing

The chains screen has always answered "how do I make this": what to build,
in what order, back to the raw salvage it bottoms out in. It never said a
word about what the thing at the end of the chain is *for*, so deciding
whether a two-machine line was worth standing up meant leaving the screen,
finding a copy of the product in cargo, and reading its description there —
or already knowing.

Each chain now opens on that description, above its steps. It is the same
prose the inventory's describe page shows, drawn from the item's own file,
so the two cannot end up describing one item differently and a modded item
gets its line for free.

## 0.8.3

### A door beside you looks like a door

Walking a Stack corridor, a door dead ahead draws brown with a `+` over it.
A door one step to your left drew as the same cyan wall as the rock it is
set into — so the only way to find a side passage was to walk into it.

The side wall now takes its colour from whatever is actually standing
there, the way the wall ahead of you already did. Sealed doors keep their
own red, so a way on you cannot open yet still reads as different from one
you can.

This is the half of the fault that survived the last attempt at it. That
one made a door legible *down* a corridor, by marking it with a glyph after
the distance fog was found to be eating the colour; this one is about the
door you are standing next to, where the neighbouring cell is off the edge
of the view entirely and its colour is the only thing left to see it by.

## 0.8.2

### The guardian at the bottom of a stack stops outclassing you

A lair guardian is the fight a run cannot walk away from — it is the only
thing in the game that pays a Portal Fragment, so a zone you cannot breach
is a run that has quietly ended. Measured on a party arriving in the shape
the game expects, a depth-2 lair in zone 3 was clearing about once in four.
It now clears a little over half the time. The change is one number, the
Overseer's attack; nothing else about the fight moved.

### The roster tuner can no longer propose a roster the game would reject

Developer tooling, and the rest of this release is all of that kind.

A species' stats are not authored freely — its growth band sets a budget,
its class sets what share of that budget it carries and how the three stats
divide it, and its speed sits in a band the class decides. That is what
makes "tanky for its tier" a readable thing rather than a number you have to
already know the ladder to interpret. The tuner moves exactly those fields
and had never been told any of it, so its first real search proposed
fourteen changes of which thirteen would have broken the shipped suite. Two
of the fourteen were never even reported, because the check that catches
them stopped at the first thing it found.

It now knows. A candidate that puts a species off its budget, off its
class's shares, outside its speed band, or on a growth multiplier between
the ladder's rungs is thrown out before any fight is run — and the rules are
the game's own, called rather than restated, so the two cannot drift apart.
Bosses stay exempt, which is deliberate: they sit outside the class system,
and a boss's attack was the one move the tool has ever found worth having.

Two further checks are too expensive to pay on every candidate — one runs a
level search, the other reads the whole roster's distribution — so they run
once on the winner and are **reported** rather than enforced. By then a
person is reading a diff and deciding, and a report that hides what it
checked is worse than one that admits what it could not.

The rejection count now says *which rule*, which turned out to matter
immediately: the first run under the new rules threw out forty candidates
against twenty-one fought and proposed nothing at all. That is the search's
move generator being wrong rather than the roster being right, and
`dev-tuning/NOTES.md` records it as the next piece of work instead of
letting a converged-looking run stand as evidence.

None of this touches a save, a species file, or anything a player sees.

## 0.8.1

### Difficulty rises in steps now, instead of doubling

A zone used to double every wild program's stats, and every frame you
descended into a stack multiplied them again. Your own side of the fight has
never worked that way — a level is worth one point of ATK, an item a few flat
points — so the two curves were never in a race you could win. Past a certain
depth the arithmetic stopped mattering: every blow you landed came out as 1,
and no amount of levelling, gear or roster changed it, because there was
nothing left for those numbers to do.

Both curves are now linear. A zone adds to enemy stats rather than doubling
them, a frame of Stack adds a little more, and your gear tracks the same
shape so it neither falls behind nor runs away. Deep is still hard — it is
just hard in a way you can answer.

What that means concretely, measured on a real stuck save at zone 1: the
guardian at the bottom of a five-frame stack went from a fight won 3 times in
40 to one won every time, and a six-frame stack from unwinnable at any level
to a twenty-round fight. Deeper in, a zone-three guardian was unbeatable at
level 90 in the best gear the game ships; it now wants about level 110. The
levels a zone asks of you go up by roughly the same amount each time rather
than doubling, which is the whole point — there is no longer a zone where the
game quietly stops being finishable.

Existing saves keep playing. If you are wearing something you equipped at
zone 2 or deeper, its bonus was banked at the old rate and stays that way
until you take it off, at which point you keep a few points you did not
strictly earn.

### Also

- **The arena can stage a lair guardian.** `Encounter::Lair` on the arena
  screen and in a scenario file fights the thing at the bottom of a stack,
  which nothing could reach before — corridor ambushes never roll a boss.
  `dev-saves/deep-lair.ron` is the run the numbers above came from.
- A Recompile Kernel's zone bump was doing nothing past the second tier. It
  applies properly now.

## 0.8.0

### Your programs can wear your gear

Every program you own now wears the same three slots you do — Weapon, Armor
and Module — out of the same cargo. Press `E` on the roster (`p`, then
Companions) and the program under the highlight gets a page of its own three
slots; picking one opens the replacement picker you already know, showing
what each swap would change for *that program*.

A copy is interchangeable. What comes off your back goes on a program's, and
comes back off again into your cargo — nothing is bound to a wearer and
nothing is consumed by being worn. That is the point: you have been hoarding
every second copy of every weapon you ever found, and the programs fighting
beside you have been going in bare while you outgrew them.

Gear always comes home. Sell a program, extract its routine, fuse it away,
lose it in a fight or lose it defending the base, and whatever it was wearing
is back in your cargo. A trader appraises the program and never the gear it
happens to be holding, so kitting one out before selling it neither raises
nor lowers what you are paid.

Two things worth knowing before you spend anything:

- A **Decompiler bonus does nothing on a program.** Only you ever attempt a
  capture. Ten shipped items carry the stat, they still equip, and the slot
  page says so at the top rather than leaving you to work it out.
- A refactor still upgrades the program and only the program. Whatever it is
  wearing is lifted off for the arithmetic and put straight back, so a
  Recompile Kernel cannot quietly bake a borrowed weapon into a program's
  permanent stat block.

### Saves are now plain text, and this should be the last time they break

Every save-format break this game has ever had — nine of them, v19 through
v28 — was one thing: a struct gained a field. None removed anything, none
changed what anything meant. They broke your saves anyway, because the file
was a positional binary encoding where an added field shifts everything
after it.

The save is now the same field-named text `savetool dump` has always
printed, with the version on the first line. A field added from here on
loads out of a file written before it existed, so **an additive change no
longer breaks your save at all**. What still can is a field removed, or one
whose meaning changes — and that needs a real migration whatever the file
looks like.

Two things follow. Saves are readable and editable in any text editor, no
`savetool` round trip required. And they are bigger: the measured save went
from 13 KB to 190 KB, which costs 1.46 ms to write and read back rather than
74 µs — far under a frame, so autosave is unaffected.

### Saves from 0.7.x will not load

The save format moves to v29 — v28 for the loadout, which is new state that
has to be written per creature, and v29 for the text format above.
**Existing saves stop loading** — the game says so plainly rather than
failing strangely.

If you want to carry a run across, dump it to RON with a 0.7.x build first,
then pack it with this one:

```sh
cargo run --bin savetool -- dump saves/<your-save>.bin save.ron   # on 0.7.x
cargo run --bin savetool -- pack save.ron saves/<your-save>.bin   # on 0.8.0
```

The dump has no loadout in it and every program comes through wearing
nothing, which is correct — none of them could have been wearing anything.
The `dev-saves/` templates need no migration at all for the same reason.

## 0.7.7

### A decompile that fails still gets you somewhere

Every attempt on a program's ICE now leaves the next one better off. A failed
decompile used to cost a catalyst and change nothing at all — the odds cell
read the same number afterwards as before, so a run of bad luck was pure loss
and the only thing that moved was your stock of ICE Breakers.

Each attempt against a given program is now worth +10% on the next attempt
against *that same program*, up to five attempts' worth — a hard ceiling of
1.5x. The failure line says which side of it you are on:

```
The program's ICE holds — decompile failed! Its defences fray a little.
The program's ICE holds — decompile failed! Its defences are as frayed as they will get.
```

The counter belongs to the fight, not to you and not to the species. Jack out
and come back and the program is met with its ICE intact, which is what keeps
this "you are wearing this one down" rather than a pity meter you could bank
against a later target by farming an easy one. The cap is the other half of
that: it multiplies alongside the skill and Capture Boost terms rather than
adding to the base, so persistence can never out-scale a species' own
`taming_difficulty` — a stubborn program stays a gamble however many
catalysts get fed to it.

The battle screen needed no changes: it already quotes live odds per group,
so the number simply climbs as you work, and it rewinds with the narration
along with everything else on the row.

## 0.7.6

### A level-up says what it gave

Levelling announced itself and stopped there — the stats it grew were left for
you to notice on the character screen later, if at all. The player, a party
member and a posted worker now each print a block of stat lines under the
announcement, one per stat that actually moved:

```
You gain 40 XP and reach level 5!
  Max HP 108 → 120
  ATK 14 → 15
  DEF 11 → 12
  Perk Points 3 → 4
  Decompiler 4 → 5
```

The Perk Point and the point of Decompiler skill a level pays were previously
announced nowhere at all, so a player could bank points for a run without
learning they had any. Those two rows are the player's alone; companions get
neither.

A stat that didn't move is left out rather than printed as unchanged, since
attack and defense both grow by one point a level and a low growth multiplier
rounds them away on a given level.

The lines carry `before → after` rather than a bare `+1 DEF` for a reason
worth recording: the history screen folds identical lines together, so two
programs each gaining a point of defense in the same fight would have
collapsed into one row and deleted the second one's line outright.

### A posted worker's level-up is its own line

It used to be glued to the end of the extraction line as a tail, which left it
drawn as loot rather than as a level-up, and dropped from the results a
finished intrusion keeps. It is now a base-log line in its own right, named by
the machine the program is posted to — so a base running several cronjobs says
which one grew.

## 0.7.5

### Haulers walk around the base instead of through it

A posted program's route was a walk over terrain, and a structure never makes
its tile unwalkable — the player is blocked from one separately. So a hauler
carrying a load to a depot walked straight over the machines its owner had to
walk around. It now refuses any tile a structure stands on, and the tile a
worker is sent to stand on while working or delivering is picked the same way.
One exception, and it is deliberate: a worker may always step *off* its own
tile, because deploying a structure never checks whether a program is standing
there and a worker built over would otherwise be stuck for the rest of the run.

Some tightly packed bases will find a machine that no longer has anywhere to
stand. That is not new — you could never have collected from such a machine
either, since you cannot stand on a building — it was simply invisible before.

### Machines say when their program cannot reach them

A new **cut off** state, distinct from a machine whose program is merely away:
it means no route exists at all, and waiting will not fix it. It draws red
rather than yellow, because it is asking you to go and clear a path.

Posting now tells *walled in* apart from *no route to it from here*, instead
of calling both "too far away" — which had become a lie about a machine you
were standing beside.

### Demolish with `d` and a direction

`d` on the map aims at one of the four neighbouring tiles and demolishes what
is there, without opening the base menu. Adjacent only: this key destroys what
it finds, so you have to be standing next to it. Removing your Home still asks
first, and the key is refused underground. The menu route is unchanged.

### Fixes

- A machine with nobody posted to it now reports **idle** and draws grey. It
  never did for an extractor: only assemblers were ever told they were idle,
  and the status defaults to *running*, so a freshly deployed Research Node
  drew green as though it were producing, and a machine whose program was
  killed or reassigned kept its old state for the rest of the run.
- A machine part-way through a long cycle now reads as running rather than
  falling back to that default.
- The manifest shows a rare spawn's **Optimized**/**Overclocked** flag. It was
  previously readable only as a bar on the map tile and a tag on the battle
  roster, with nowhere to go and check.

## 0.7.4

### The roster tuner lands

`cargo run --bin tuner` is a seeded hill-climb over the shipped species
roster: it perturbs `assets/species/` into a scratch install, fights the
result against authored arena scenarios, and scores the outcome against an
objective read as data from `dev-tuning/objective.ron`. It has been parked
on a branch since `0.5.10`; the code is unchanged apart from being brought
up to the current arena API, and the whole of it is developer tooling —
`assets/` is never written to, and the tool proposes rather than applies.

Two things about it are worth knowing before running one. Its constraint
layer refuses any candidate that would nerf a program the player actually
fields, so a search cannot buy an easier fight by weakening the party's own
roster. And its headline finding is recorded in `dev-tuning/NOTES.md`:
**Stack depth 5 is not a roster problem.** Zone and depth stat scaling
compound — 4x at zone 3 times 3.32x at depth 5 is 13.3x base stats — which
floors every party hit at `MIN_DAMAGE`, and no species file inside any sane
bound can undo a multiplier. The tool was built to answer that question and
the answer is that it should not try.

## 0.7.3

### Fix: the `chains` template's spare programs carry their species kit

The six spare programs `0.7.2` added were hand-authored holding a single
`priority_boost` between them, so they opened with nothing but the default
buff. A save's routine list is taken verbatim on load —
`install_innate_routines` runs when a program *comes into existence*, a
decompile or a fusion, and never on a load — so nothing filled the gap in.
At level 12 they hold six slots and now carry what their species grants:
`redundancy_sync`/`rollback_v1` for the Medics, `skim_group`/`skim_v3` for
the Leeches, `overclock_array`/`sandbox` for the Bastions, with
`priority_boost` kept beside them.

Their jobs at a post were never affected — a class's work at a structure is
passive and reads nothing from the routine slots.

## 0.7.2

### The `chains` dev template is now a factory sandbox

`cargo run -- --template chains` already stood up a three-stage production
chain, but it opened with eight of the twenty-one research nodes and fifteen
Core Fragments — enough to watch the chain it ships with, not enough to build
a second one. Testing the factory therefore still started by playing the
research tree. It now opens with:

- **Every research node unlocked**, so the whole build menu and every bench
  recipe is available. The eleven structures no research file names were
  already unlocked by default, so nothing is behind a gate any more.
- **Deep cargo**: 600 Core Fragments, 120 each of the four bench
  intermediates (Bytecode Block, Blank Substrate, Logic Wafer, Charge Coil)
  plus Annealed Cores and Raw Trace, 120 Power Cells, 100 Outlets, 20 Portal
  Fragments, 5000 Credits, 500 Research Data, 200 ICE Breakers, 50 Routine
  Disks — including the four intermediates that gate a bench's own
  `build_cost`.
- **Six unposted programs** standing on the player's tile — two each of the
  three classes that do something at a post, named `Spare Medic A/B`,
  `Spare Leech A/B` and `Spare Bastion A/B` — so the base jobs are testable
  without taming first. They are on top of the nine already running
  cronjobs; the five-slot party is unchanged.

Developer tooling only: no shipped asset, no engine code and no save format
changed, and the two gates that keep a template honest —
`every_checked_in_template_still_loads` and
`the_chains_template_starts_with_a_chain_that_actually_runs` — both still
pass against it.

## 0.7.1

### Three of the five classes now have a job at your base

A class has meant a stat shape and a kit — both about a fight. It now also
decides what a program does when you post it to a structure.

- A **Leech** draws an extra unit out of every successful gather cycle.
- A **Bastion**'s Defense counts twice against a GC Entropy Sweep on the
  structure it is guarding.
- A **Medic** repairs that structure by 2 Durability every 20 ticks, which
  is the first repair in the game that isn't a Patch Node — and the only
  one a base with no Patch Node has at all.

A **Striker** and a **Saboteur** do nothing at a post, and that is the
decision the feature is for. You have three pet slots: every program at a
machine is one absent from the party, so a base that runs itself is a party
that can't fight, and the classes that are worth posting are exactly the
ones you least want in the line. The manifest's WORK box now names each
program's job beside its Speed and Analysis, so the trade is legible before
you make it rather than after.

Two limits worth knowing. A Leech draws nothing extra from a Research Node
— banked resources pay a flat rate to everyone, which is what keeps the
research ladder priced — and a Medic only mends while it is *guarding*: put
it on a cronjob and it is extracting, not repairing.

### Every program now knows two moves, and which two says what it is

Until now, eight of the seventeen species taught a captured program
nothing at all — it fell back on `priority_boost` and stayed there for the
rest of the run, and the five that did teach something taught it in no
particular pattern. Every non-boss species now grants exactly two
abilities: a **class utility at level 2**, shared by all three members of
its class, and a **tier rung at level 6** that it holds alone.

So a program tells you what it is twice. The Drone, the Worm and the
Rootkit all open with Skim Group, which is what a Leech does; what
separates them is the second unlock, where the Drone learns Skim Single
v1.0 and the Rootkit v3.0. The same split the stats already make — the
tier sets the budget, the class spends it — now runs through the kit.

Fourteen new routines, in five families. Three of those families are new
(Segfault, Rollback, Skim) and one of them fills a real gap: **Drain was
previously findable only in the field**, so the Leech class had no move it
was allowed to know. Skim is a Leech's own drain, and the three hunt-only
Leech routines stay exactly where they were — the hunt-only pool is
unchanged at twenty-eight, and nothing that was a prize has become
standard issue.

**Nothing unlocks at level 1**, deliberately. `priority_boost` is what a
companion falls back on when its species has taught it nothing *yet*, and
extracting it from such a companion is the only way to get it — so a
species granting anything at level 1 would delete it from the game. It also
means a freshly tamed program reads as generic for a level before it reads
as its class.

Two routines you may already own display slightly differently: Bastion
Single is now **Bastion Single v1.0** and Bit Rot Single is **Bit Rot
Single v1.0**, because both are now the bottom rung of a ladder. Nothing
about either has changed and no save is affected — the names moved, the
ids did not.

`dev-arenas/class-mirror.ron` is the instrument for all of this, and it
exists because neither offline harness can be: `balance_sim` models no
abilities and the `arena` bin never fires a Special. It stages a Bastion,
a Medic and a Leech of one tier against a pack picked for the one thing a
kit needs, which is rounds to spend it in.

### Seventeen species stop being one axis wearing seventeen names

Every non-boss program now belongs to one of five classes — Striker,
Bastion, Medic, Saboteur, Leech — and none of them is a field in a file.
A class is the affinity axis a species raises, the shape of its stats and
the pace it moves at, all saying the same thing. A Crawler is now a wall
with 102 HP behind 9 DEF and almost no bite; a Scrapper of the same tier
is 80 HP, 12 ATK and 3 DEF. Before today those two were 75/8/4 and 98/9/5
— the same creature, one of them slightly larger.

What makes the role readable is that it is **independent of tier**. A
species' growth band sets a stat budget and its class decides both how
much of that budget it gets and how it spends it, so "low DEF for its
size" reads the same at tier 1 and tier 3. It could not before: a tier-3
striker out-tanks a tier-1 tank on raw HP, which made "tanky" a thing you
could only see if you already knew the ladder. Every raised affinity is
1.3 and every damped one 0.85, so the magnitudes say nothing and the axis
says everything; the manifest's AFFINITIES box, previously hidden for
eleven of seventeen species, now has something to show for all fifteen
ordinary programs.

Speed carries the class too, which since the last entry means it carries
into the base: Bastions and Leeches are slow at a machine and Saboteurs
and Medics quick. The Drone in particular drops from 13 to 8 — the
commonest early worker is now a slow one, and what it gets back for that
arrives in a later release.

**The Construct has moved down the ladder**, from the 1.5 growth band to
1.0, because the four species already carrying affinities plus the Virus'
pinned 1.5 fill that tier's five roles. It is a tier-1 wall now, 49/2/4,
and cheap to compile at 0.35; its Crash came down from power 13 to 8,
which was the highest move power of any ordinary program and made no
sense on an opening-ring statline. It does now spawn in the opening ring,
which is five species rather than four.

**The projected progression curve moved**, and is recorded rather than
retuned. `balance_sim`'s median party species is a Proxy instead of a
Scrapper, and the levels its sweeps project to clear each zone with a
full party are:

| zone | grind-only | geared | full roster |
|---|---|---|---|
| 2 | 19 → 15 | 14 → 10 | 11 → 8 |
| 3 | 40 → 30 | 29 → 26 | 26 → 22 |
| 4 | 77 → 63 | 55 → 61 | 49 → 55 |
| 5 | 149 → 131 | 100 → 127 | 96 → 117 |

The shallow zones got easier and the deep ones harder, both for the same
reason: the toughest ordinary program is a Sentinel, and a Sentinel is a
Bastion, so its ATK fell from 9 to 6 while its DEF held. The hardest
ordinary fight in the game is now a longer, safer one. The gate itself
asserts the curve's *shape* — monotonic, geometric, gear beating grind —
and all of that still holds.

### A species' stats now say something about work, not only about fighting

Two of a species' base stats picked up a second job today. `base_speed` —
until now read only to order turns in a fight — also sets a species' pace at
a machine: post a Sprite (`base_speed: 14`) to a Mining Node and its 10-tick
cycle becomes 8; post a Construct (`base_speed: 6`) and the same node takes
12. `base_int`, a stat with no meaning at all before this, is read as a
fourth term on the extraction roll, alongside the node's own tier and the
player's Keen Scavenger perk — a Cipher and a Construct posted to the same
Mk1 node now visibly disagree, 0.58 against 0.40. Working a node yourself is
untouched by either change: the player has no species, so both read at
exactly the roster's baseline, which is what keeps posting a sharp program
better than doing the job by hand, and a dull one worse.

The manifest has a new WORK box to say so — Speed and Analysis, the two
numbers that describe what a program is like to post somewhere, sit
together rather than getting buried in SPECIES or duplicated across two
places on the same screen. An assembler now runs at the rate baked in when
its program was posted, rather than at its structure's flat
`ticks_per_unit` regardless of who was staffing it — previously two very
different species posted to the same Assembly Bay ran identically. A
cronjob already posted before this update, including one loaded from an
older save, keeps its old rate until reassigned; nothing recalculates a
running job's pace mid-cycle.

Both effects are sized by their own tuning constant
(`MINING_SUCCESS_PER_INT`, `WORK_TICKS_PER_SPEED`), independent of the node
or machine's own numbers, so how much a species' choice matters can be
retuned without touching how much the machine itself matters.

## 0.7.0

**Breaking: existing saves will not load.** `SAVE_FORMAT_VERSION` goes 26 →
27 because every program now records the upgrades spent on it. To carry a
game across, dump it to RON *before* installing this version and pack it
back afterwards — RON is field-named, so the new fields fill themselves in:

```sh
cargo run --bin savetool -- dump saves/save.bin s.ron   # on the old build
cargo run --bin savetool -- pack s.ron saves/save.bin   # on the new one
```

### Refactoring: your programs can keep up now

A tamed program's stats were baked once, at capture, and never rescaled.
Enemies double every zone. So a Scrapper caught in the opening ring was
permanently anchored to zone-1 numbers, and the only answers were to throw it
away for a fresh catch or to fuse it — both of which are ways of *replacing*
a program rather than decisions about the one you have. Nothing on screen
even said it was happening.

There are two permanent, player-driven upgrade tracks now, both off a new
production line: Mining Node → **Annealing Node** → **Refactor Bench**, behind
one research node. Apply them from the party menu; it works underground, which
is where you are most likely to notice a companion falling behind.

- **Recompile Kernel** — rebuilds a program for the zone you are standing in,
  doubling its stats. Refused once it has caught up with you, which is what
  bounds it, and it costs no upgrade slot: nobody should have to burn a
  permanent slot merely staying current. The bench assembles these on a timer,
  because it is the thing you want again after every breach.
- **Six percentage buffs**, one per stat — `+5%` crafted at the bench, `+12%`
  off a boss or, rarely, off a mid-tier program. Each spends one of a
  program's five permanent upgrade slots, so which stat gets them is the
  choice.

Percentages rather than flat amounts, because a companion's numbers keep
growing and a `+15 HP` buff means nothing at 500 HP. They also commute with
the zone rebuild, so a buff bought in zone 1 is worth as much three breaches
later and there is no ordering to exploit. Small stats floor at `+1`: `+5%` of
a Drone's 3 ATK would otherwise round back to 3 and do nothing to exactly the
programs the feature exists to rescue.

**The manifest now tells you when a program is behind**, which it never did —
a zone tag reading "1" means nothing without the zone you are standing in
printed beside it.

**Traders pay for what a program is, not what you spent on it.** A trader
offers a tenth of a program's power, so upgrading one and selling it would
have turned printable salvage into Credits — the one currency that survives a
breach — at a rate that compounded with every rebuild. Bought tiers are
divided back out of the sale. Tiers *earned* by taming something deep are
untouched: beating it is what the game charges for that.

### Also

- Fusing two programs keeps the higher zone tier and the higher upgrade count
  rather than resetting both, which would otherwise have laundered a maxed-out
  program back into a fresh one.
- Item files may now declare an `upgrade` block; see
  `assets/items/README.md`. A negative percentage or one that declares no
  effect is refused at load with the rest of the malformed files.

## 0.6.0

**Breaking: existing saves will not load.** `SAVE_FORMAT_VERSION` goes 25 →
26 because every creature now records the rare tier it rolled. To carry a
game across, dump it to RON *before* installing this version and pack it
back afterwards — RON is field-named, so the new field fills itself in:

```sh
cargo run --bin savetool -- dump saves/save.bin s.ron   # on the old build
cargo run --bin savetool -- pack s.ron saves/save.bin   # on the new one
```

### Rare programs: Optimized and Overclocked

Wild programs used to vary along one invisible axis — a ±20% roll per stat,
readable only as a "quality" label on a screen you reach *after* catching
something. There was no moment on the map that said *that one, go get that
one*.

There is now. A wild spawn can come up **Optimized** (uncommon, 1.5x stats)
or **Overclocked** (rare, 1.8x). Both multiply on top of the existing roll,
so an Overclocked program lands between 1.44x and 2.16x an ordinary one of
its species — a real threat, and a real prize.

- **You can see one coming.** A rare program wears a silver or gold bar
  along the top of its tile. Its glyph still shows the difficulty colour,
  because how dangerous something is and how rare it is are two different
  things you need at once.
- **Catching one keeps it.** Decompiling never re-rolls stats, so a program
  you take stays Optimized or Overclocked for the rest of the run — and
  fusing two keeps the better of the pair rather than laundering it away.
- **It pays for itself.** A kill already pays the defeated program's max HP
  as XP, so a rare one is worth proportionally more.
- **Bosses and the opening ring are excluded.** A boss's stats are authored
  by hand, and the first ring around your landing site is guaranteed to be
  beatable by a fresh player. Neither gets a tier.

Both the spawn chances and the multipliers are tuning constants
(`SILVER_SPAWN_CHANCE`, `GOLD_SPAWN_CHANCE`, `SILVER_STAT_MULT`,
`GOLD_STAT_MULT`), so the rate and the reward move independently.

## 0.5.23

### The examine key no longer reads through shut doors

`0.5.22` shipped `x`-then-a-direction with a real defect: the ray it walks
to find something to describe never checked whether anything was in the
way. A door two cells ahead is opaque to the eye and to the map, but not to
`x` — so standing in a corridor and looking at a closed door reported the
unopened cache sitting behind it, seal intact. You could read the contents
of a room by looking at the door to it.

The ray now stops where sight stops, using the same occlusion rule the
first-person view and the frame map have always used. It stops *at* the
blocker rather than before it, so looking at a door still describes the
door — a wall in plain sight is a thing you can look at. Standing inside a
doorway still shows you the corridor beyond, which is the one case where a
sight-blocking cell is also one you can occupy.

Two smaller corrections to the same key. The cell immediately to your left
or right could not be examined at all — the ray skipped its whole nearest
rank rather than just the square underfoot — and nothing distinguished left
from right, so the two could have been swapped without any test noticing.

### Flavour text that described mechanics the game does not have

Three shipped fragments claimed rules that were never implemented. Rotten
substrate said standing on it "a while costs more"; the bleed fires once
when you arrive, so waiting is free. Corruption said it "spreads slower
than you walk", and a cleared lair was "already starting to rot back over";
corruption is placed when the frame is generated and never spreads at all.
All three now describe what actually happens.

### Fixes

- The description bank's schema doc contradicted itself on how long an
  underfoot line may be and on whether a fragment may run to two sentences,
  and did not mention that the frame-arrival subject is the one place a
  `{bearing}` token would reach the screen unexpanded. It is the prompt an
  author works from, so an error in it propagates into content.
- Engine tests no longer leak scratch directories under `/tmp` when a test
  panics; they now use the same cleanup guard the rest of the suite does.
  The failure mode this avoids is inode exhaustion, not disk space.

## 0.5.22

### Generated flavour prose for the Stack

The Stack's first-person view used to hand you one string per cell: a key
prompt like `"A link leads down  [>] descend"` and nothing else. Every cell
now composes real prose around that prompt from an offline-authored bank,
on three surfaces:

- **Underfoot.** The row under the view still carries its key prompt, but
  the description in front of it is drawn from the bank instead of a fixed
  literal.
- **The log.** Walking a notable cell into view — an unopened cache, a live
  breakpoint, a sealed door — writes one line about it, at most once per
  move, for the single most notable thing that just came into sight.
  Arriving on a new frame writes a one-line mood beat of its own, once per
  frame regardless of how many steps it takes to get there.
- **Examine.** Press `x` underground, then a direction, to read a full
  paragraph about whatever's ahead, to either side, or underfoot.

A given cell of a given stack always reads the same way — the door you
walked past once reads identically the second time — and a different stack,
or a different depth of the same one, reads differently. Nothing is stored
to make that true: every line is derived from the world seed, the entrance,
the depth and the cell, the same way the Stack's own layout already is.
**No save-format change** — an existing save loads with the feature already
working, nothing to migrate.

### The crash-log reader draws from the same bank

The `Z` key's crash-log reading used to draw from its own small, separate
pool of eight lines. That pool is gone; `Z` now composes its reading from
the shared corruption vocabulary instead. The words you read are unchanged
— every line carried over verbatim — only where they come from did.

### Fix: examining underground no longer answers with a surface creature

Pressing `x` while in the Stack used to run the surface creature-and-structure
scan, because the party's on-map position stays pinned to the entrance tile
the whole time underground — so it could name a creature standing near that
entrance as your target while you were several frames below it. Examining
underground now always describes the Stack cell you're facing instead (see
above); the surface scan refuses outright rather than answering with stale
ground data.

## 0.5.21

### Rename a program

Press `N` on the companion roster to give a program your own name for it.
The page opens on the name it already has, so fixing a typo isn't a retype;
clearing the field puts its species name back. It works on any program you
own, wherever it is — in the party, posted to a cronjob, standing guard —
because a name changes nothing about where it is or what it's doing.

Names survive saving, and always did: `CustomName` has been complete since
fusion learned to name its result, and fusion was simply the only way to
reach it. **No save-format change** — an existing save loads unchanged, and
a program you already named by fusing keeps that name.

Renaming is refused during a battle. The reason is the log rather than the
roster: the battle screen replays *rendered* rows as its narration scrolls
in, so a name changed mid-fight would leave what you're reading and what
you're looking at disagreeing about who is being hit.

### The stun measurement was reported on a cherry-picked sample

`docs/measurements/2026-08-10-stun-move-levers.md` claimed the 2-turn stun
retrain made enemies vary their moves more. It measured only the three
species whose files were edited. Across the whole roster three improved and
**eight got worse**, including all four opening-ring programs a new player
meets. The entry now carries the full table and the correction; nothing in
the game changed, but the note that justified a shipped change did.

### Most trained weights turn out to mean nothing

Retraining the enemy policy three times, changing only the optimiser's seed,
**seven of the sixteen free features flip sign** — while the enemy win rate
lands within 2 points every time. Only `target_hp_frac` and
`est_damage_frac` are stable. Three quite different-looking policies play
about equally well, so a single weight is not evidence of anything and
several claims across the measurement notes were reading them as if it were.
Written up in `docs/measurements/2026-08-10-weight-identifiability.md`, with
corrections threaded back through the two entries that leaned on a
coefficient. The shipped policy is unchanged and still doubles the enemy win
rate; what changed is what we are entitled to say about why.

## 0.5.20

Tooling and measurement only — nothing about a played game changes.

### The arena can exercise Defend

`arena::run_rep` played the party as All-Attack, so nobody ever braced and
no arena measurement could say anything about Defend at all.
`RunOptions::party` adds two plans: `BraceWhenHurt` (a member under half HP
Defends) and `BraceInRotation` (one slot per round, whatever anyone's
health). `--party-plan brace|rotate` reaches them from `train`.

All-Attack is still the default, so every number published before this keeps
meaning what it meant, and `the_default_party_plan_never_braces` holds it.

### The analysis refuses to present a confounded number

`analysis/policy_report.py` now checks each run for observables that move
together and warns above the first table. It was written after a run that
could not answer its own question — the brace rule fired on a threshold over
the policy's largest feature, so bracing and being wounded were one
variable. The check is grouped per run, because pooling a sweep hides the
confound behind the configs that never brace.

### What the instruments said

`docs/measurements/` gains the pin sweep, the stun-move levers, and the
bracing runs. The last of these confirms the three pinned policy features
are justified — and corrects the 2026-08-09 reason for one of them, which
read a weight that training could not have learned, since under All-Attack
`target_bracing` is constant and fitness is indifferent to it.

## 0.5.19

### Wild programs use their stun moves again

Three moves were effectively dead — cipher's Encrypt, crawler's Freeze and
rootkit's Privilege Escalation, each chosen on 1-2% of swings where the
program had a choice. All three are Stun, and each is priced below its
damage-only sibling, so skipping them was correct expected-damage play.

Each now stuns for **two turns instead of one**. Their power is untouched:
raising it to match the sibling was measured first and flips the problem
rather than fixing it, taking the stun move to ~96% and killing the *other*
move instead. Duration moves usage to 12-20% and leaves the enemy's win
rate where it was.

The retrained weights ship with it, and have to: stun duration is not one of
the policy's features, so the enemy cannot see the change directly. What it
changes is what training measures — longer stuns win more fights, so the
search stopped penalising the moves that carry them.

Measured in `docs/measurements/2026-08-10-stun-move-levers.md`, including
what the run could not establish.

## 0.5.18

### The trainer can now show its working

`train` records the two evaluation passes either side of its search —
the all-zero baseline and the trained result — as battle telemetry, one
file per config, pass and scenario. The 1.9M-fight search between them
stays unlogged on purpose: those are candidates that were thrown away,
and keeping them would cost tens of gigabytes to describe weight vectors
nothing ever used.

```sh
cargo run --release --bin train -- --label pin3 --log-dir dev-logs/policy-sweep …
```

Nothing about a played game changes. `arena::run` gained a `RunOptions`
argument that defaults to collecting nothing, so the headless arena bin
and the game's own arena screen behave exactly as before.

### Reading that telemetry, in Python

`analysis/` is the first Python in the repo with dependencies — pandas
and matplotlib, behind a venv and a `requirements.txt`. It answers who
the enemy swings at, how hurt the target is when chosen, and how much of
its moveset each species still uses. The training itself stays in Rust:
the objective function is the real game, so an optimiser over here would
have to call back into `arena::run` for every one of its evaluations.

### A place for what the instruments said

`docs/measurements/` — one file per question actually answered, each
carrying the commands that produced it and the blind spots it had. The
first entry finds that the three pinned policy features are a design
boundary a free search will always cross: trained unpinned, the enemy
downs **zero** companions across 1,600 fights where the baseline downs
267.

## 0.5.17

### A routine you can't run stops being pickable

Picking a routine the game had already greyed out — no ICE Breaker for a
Decompile, a cooldown still running, a full roster — opened the target
picker anyway. You chose the routine, chose who to point it at, and only
then found out it was never going to fire. The picker now refuses the
press on the reason already printed on the row: `Can't use Decompile — no
taming catalyst.`

Both other places a routine is chosen, the battle action menu and the
field cast list, already worked this way. The ability picker was the one
step of the three that skipped the check.

## 0.5.16

### A drop says what kind of thing it is

The battle log is where a player meets a dropped item for the first time,
and it named the item and nothing else. Every screen that *lists* an item —
the inventory, a trader's shelf — puts its category beside the name, so the
log was the one place you had to already know whether a "Hardened Shell"
was armour, a module or stock for the bench. Drop lines now carry the same
tag those screens do: `It also drops a Hardened Shell [ARM]!`

`Game::item_name_tagged` is the one formatter, reading the same
`ItemCategory::short_label` the columns do, so the two cannot drift. The
tag goes after the name rather than in front of it because a sentence has
no column to put it in.

## 0.5.15

### A played fight leaves something behind

The game could be played, and it could be measured, but not at the same
time. `arena` runs a fight offline and reports it; a fight a person plays
by hand produced nothing — the arena session writes no save and no profile
by design, the message log dies with the process, and the only artifact was
recall. So the question the trained enemy policy shipped with, whether a
party actually using its routines still loses to it, could be played out and
then not answered.

`FERAL_DEV_LOG=1` now records what happens inside a fight to
`dev-logs/battles.jsonl` — one JSON object per line, per swing, round and
decision. Five kinds: the fight opening with its party and pack, a snapshot
at the top of each round, every enemy's chosen move and target, every party
member's chosen action, and the outcome.

The number it exists for is `target_hp_before`, the target's HP *before* the
hit lands. Focus fire is a distribution over that number and a per-round
snapshot cannot show it: four attackers all act inside one round, and by the
snapshot they have finished. It is recorded at the single point where a wild
program's move and target are decided, so no swing can be missed.

`dev-logs/README.md` is the schema, one row per field. Unset — which is
every ordinary run and every player's build — nothing is collected, nothing
is written, and no file is created.

### An arena fight is allowed to write this one thing

An arena session deliberately touches no disk: no save, no profile, no run
history. That rule exists so a tester's fight cannot corrupt a save or pay
out profile rewards to a real player. A dev-only log does neither, and the
arena is the single place a recorded fight is most wanted — so telemetry is
a stated exception, with a test of its own sitting beside the three that
assert the opposite about everything else.

## 0.5.14

### Damage lands when the log says it does

A round used to resolve all at once. Every HP bar dropped to its
end-of-round value before the first line of narration was legible, so the
text was reporting a fight the screen had already finished — you read
"You unleash a data strike for 12 damage" against a bar that had absorbed
that hit and the three after it a second ago.

The roster now steps with the narration. A bar holds still until its own
line lands, then drops; a pack of three keeps reading as three until the
line announcing the kill, and only then does the next one step into the
front rank. Group letters, status tags, decompile odds and the planned
action all move at the same pace, so the whole roster is a picture of the
moment being described rather than of the moment after.

Nothing about the fight itself changed — the same round resolves in the
same order with the same outcome. What changed is that you can watch it.
The damage flashes and floating numbers came along for free: they were
already reading the difference between one frame's HP and the next, so
they now fire one hit at a time without having been touched.

### A fight ends on its own screen instead of dumping you on the map

Winning used to close the battle screen on the spot and leave the loot and
XP sliding past in the map's log pane, which is where a fight's results
were least likely to be read. The screen now stays up: same rosters, same
pane, with the results arriving at the same pace as the fight's narration
and the action bar replaced by `[any key] continue`.

It holds for every ending — a win, a jack-out, or a defeat you were
rebooted from. A key pressed while the results are still arriving releases
them rather than dismissing, so loot cannot be skipped past unseen. A run
that actually ended still goes straight to the game-over screen.

On a win the hostile pane empties out, header and all, because by then
there is genuinely nothing left in it. Jack out instead and the pack is
still listed, which is a better look at what you ran from than the map
ever gave you.

## 0.5.13

### Wild programs have learned to fight

Which move a wild program swings and who it swings at used to be two
uniform rolls. They are now one decision, scored against a policy trained
offline over 1.9 million arena fights and shipped as
`assets/policies/enemy_battle.ron`. Across the eight training scenarios the
other side's win rate goes from 32% to 61%.

What changed at the table: they finish what they started. A companion at a
sliver of health is the one that gets hit, a kill that is available gets
taken, and the move chosen is the one that does the most to *that* target
rather than whichever came up. In a group fight this is a different
opponent. One-on-one it is much the same, because there was never a choice
to make there.

The trained file is legible on purpose — it is a list of named weights, and
`assets/policies/README.md` reads the shipped one out loud. Delete the file
and the game plays exactly as it did before; that is a supported way to
play, not a broken install. A mod can ship its own, and a species with a
moveset nobody trained against is scored by the same weights, because the
policy reasons about what a move *is* rather than about which move it is.

### Bracing is a stronger draw

`DEFEND_AGGRO_WEIGHT` is raised from 4 to 7, and the reason is the feature
above. Defend's pull was tuned against an opponent picking at random, and
it does not survive one that thinks: reducing incoming damage is what
bracing does, so anything choosing by damage has a reason to hit somebody
else instead. At the old value bracing was quietly *counterproductive*
against a trained program. It now does what its description says again.

Two rounds of training were thrown away establishing that. Left to
themselves the weights learned to kill the player and ignore the party
entirely, and — when that was forbidden — to walk past whoever braced by
reading their Defence instead. Both routes are now closed off, a test
fails if either reopens, and
`docs/superpowers/reports/2026-08-09-enemy-policy-training.md` has the
whole account, including the mechanic this turned up that has *not* been
fixed: every species prices its status-effect move below its plain one, so
a program that thinks about damage will never use one.

### For anyone poking at the internals

- `train` is a third launcher binary, alongside `savetool` and `arena`. It
  runs a cross-entropy search over the arena harness; `dev-training/` holds
  the eight scenarios it learns from, and `dev-arenas/` is held back as the
  test set and never trained on.
- `tuning::ENEMY_POLICY_TEMPERATURE` is the one dial. Raise it to blunt the
  policy without retraining; 0 makes it play its best move every time.
- No save-format change. Weights are an asset, so retraining reaches a run
  already in progress.

## 0.5.12

### The sector is populated everywhere, not just where you have been standing

Wild programs gathered around wherever you spent your time. A base you had
been working at for a while would be ringed by them — sixty-five inside a
single screen on one save — while the ground an hour's walk out held almost
nothing. Travel far enough and the sector read as abandoned.

Both halves of that were one fault. Programs spawned near you and nothing
ever removed them, so the population was a record of where you had stood
rather than a property of the place. Resting is forty ticks in one spot,
and every machine you tend is more, so your base collected them at a rate
nothing else in the sector could match. Meanwhile the far ground had never
been seeded at all — a new sector's opening population was scattered within
a few steps of where you arrived — and walking costs a turn a tile, which
is faster than programs appear. You were outrunning them.

There is now a density the sector is held at: roughly a screenful of wild
programs around wherever you are. A new sector is seeded to that across the
whole area you might travel, out as far as its Stack links, and the ongoing
spawning tops it back up rather than adding without limit. Standing still
no longer accumulates a crowd, and arriving somewhere new no longer means
arriving somewhere empty.

### Wild programs stay off your base platform

The platform is meant to be the one safe ground, and three separate rules
in the game already said so — you cannot be ambushed on it, anything
standing there is cleared when you lay the floor, and a nest's swarm will
not chase you onto it. An ordinary wandering program was never told, and
could simply walk in. It took a crowded sector to make it common enough to
notice, which is why it surfaces now.

## 0.5.11

### Rooms in the Stack stop being corridors with the sides missing

Standing in an open frame and looking down it, the floor and ceiling ran
away ahead of you and everything to your left and right was flat black.
Corridors were fine — their walls filled that space — so the fault only
showed in the rooms and chambers that arrived in `0.5.9`, and it made a
hall read as a narrow passage someone had cut the sides out of.

The view was drawing one column of cells, straight ahead. The cells beside
you were being looked at for a single thing: whether to put a wall there.
When the answer was no, nothing was drawn at all, and the background
showed through.

The whole cross-section is drawn now, so the cells to either side are
floor, wall, doorway, cache or lair the same as the one ahead — including
their markers, so a cache off to your left is something you can spot
rather than something you find by walking into it. What is *behind* them
is unchanged: a passage running past the rock ahead of you shows, and one
running behind it still doesn't.

Two things remain that no cell accounts for — the far end of a corridor
that continues past what you can make out, and the outer edges of a hall
wider than your field of view. Both used to be the same flat black, which
read as a hole in the world. They are now the dark the light doesn't reach.

## 0.5.10

### The test suite stops filling up `/tmp`

A build died with `No space left on device` on a filesystem that was 15%
full. The number that had run out was not bytes but **inodes** — 1,048,576
of 1,048,576 — and the suite was producing them at 10,741 per run, which
exhausts the table in about 97 runs.

`scratch_assets_dir` builds a test a private copy of the whole shipped
asset set, eight directories and ~190 files, and left deleting it to the
caller. Two shapes defeated that: a test that panics on a failed assert
never reaches its own cleanup line, and a helper returning a bare `Game`
had nowhere to put one. Neither is exotic — between them they had left
5,437 stale installs on the machine.

Cleanup is now an RAII guard, `ScratchAssets`, so it happens on the
unwinding path too. Its `Drop` is deliberately best-effort: turning a
failed removal into a second panic mid-unwind would abort the process and
bury the assertion that actually failed.

Measured over a full workspace run, before and after: 62 directories and
10,741 inodes become 9 and 34. The remainder is app-core's save fixtures
and the arena's, which are different helpers and are left for their own
change.

## 0.5.9

### The Stack is not all maze any more

Every frame was carved by the same maze generator, and the way down sat on
the single furthest cell from where you came in. Between them that made one
kind of trip: pick a direction, weave the whole map, find the opposite
corner. Doing it again three frames later was the same trip with different
walls.

A frame now rolls one of three shapes. The maze is still one of them,
unchanged. **Rooms** is rectangles joined by corridors, with more corridors
than it takes to connect them, so a wrong turn is usually a loop rather
than a walk back. **Chambers** is four open halls joined in a ring by
passages three cells wide — the one you cross without looking for a door.
Which you get is a property of the stack and the depth, so a particular
hole in the ground always opens onto the same thing, and climbing down a
level changes it.

The way down moved too, and on its own it is half of what made a frame feel
long. It is drawn from the far half of the frame now rather than from the
one cell furthest away: still a real walk, still never just around the
corner, but not the opposite corner every single time.

Caches and the orphaned program still live at the end of side passages —
which rooms and halls do not naturally have, so the generator now carves
them on purpose. That closed an old gap on the way past: the maze itself
ran short of them about one frame in four and quietly shipped two caches
instead of three.

A save made underground still loads. A frame is rebuilt from the world seed
rather than stored, so the walls around a saved party have genuinely moved —
if yours were standing where a wall now is, they come back at the way up
instead.

## 0.5.8

### The lair has no key

Reaching the guardian at the bottom of a stack meant carrying an Access
Shard, and shards came out of caches. So a descent could end at the door
you came for, with nothing to do about it but climb back out and hope the
next run's caches were kinder — a gate that decided whether the deepest
fight in the game happened at all, on a dice roll made three frames higher
up.

A sealed door is now a barrier rather than a lock. You shoulder it open by
walking into it, it stays open behind you, and what it costs is the noise:
the same Trace it always raised, now the whole of the price. Everything
else about it holds — it still walls the lair off, you still cannot see
past it, and phasing or jumping into the wing behind it is still refused,
because the lair is entered through its door.

The Access Shard itself is dormant rather than gone. Nothing asks for one
any more and caches have stopped stocking them, which makes a cache's haul
slightly thinner than it was; the shard keeps its name and its price, and
what it is for instead is an open question.

## 0.5.7

### A debug port might not give you the frame

Jacking into a breakpoint handed over the whole frame's map, every time, for
a walk and the loudest Trace raise in the game. The only question it asked
was whether the walk was shorter than mapping the frame on foot, and the
answer was always yes.

A jack-in now takes six times in ten. When it fails the port stutters and
resolves only the substrate you are standing in — a small patch around you,
but one that reads *through* walls, which the view down a corridor never
does, so a failed jack still tells you which way the junction you are on
actually goes.

Either way it is one try: the port burns out on the attempt, and the Trace
is charged for jacking in rather than for what came back. There is one port
per frame, so this is a decision about a walk you can see the length of, not
a lever to pull until it pays.

## 0.5.6

### Doors you can see coming

A door at the end of a Stack corridor was invisible until you were standing
in front of it. Doors are drawn as a face filling the passage, in their own
colour, and that colour was all that separated one from the rock beside it —
so the fog that makes the corridor recede also took the only thing saying
"door". Three cells out, both were near-black.

A door now carries the same orange `+` the frame map marks it with, and a
sealed one the same red, painted in the middle of its face. The marks that
were already there — links, caches, lairs, faults, corruption, orphans —
fade more slowly with distance than the walls do, so the layer that exists
to be spotted down a corridor stays legible at the far end of the view.

## 0.5.5

### A posted program starts from where you are

Posting a program to a structure could produce a cronjob that never ran. The
machine sat yellow, its working animation never played, and the log said
nothing beyond "Cronjob scheduled." — the program was standing somewhere it
could not walk back from, and never took a step for the rest of the run.

A program's position on the map is set when you beat it and never changes
again: it does not trail after you as you walk, which is why your companions
are not drawn on the map at all. So a program tamed far from home was still,
as far as the sim was concerned, standing out in the wild — and a machine
more than fourteen tiles from that spot was unreachable, whatever the
program's actual whereabouts.

A program now sets off from your own tile, because that is where you have
been carrying it. One consequence is worth knowing: the walk to a post is
bought by posting from a distance. Post while stood at the machine and the
program is already at its station; post from across the base and you will
watch it walk in. A structure too far away for the walk is now refused
outright, with a message, instead of accepting a cronjob that could never
start.

The cronjob and guard pickers were filtering the same stale positions, so a
program tamed more than forty tiles out was missing from both lists — and
because the base menu hides a row whose screen would be empty, a player whose
only program was tamed that far away lost the Cronjob row entirely. Both
pickers now offer every program you own, wherever you caught it.

## 0.5.4

### Structures throw sparks when a sweep hits them

A GC Entropy Sweep used to be a coloured wash over a tile and a line in the
log. Damage and destruction now throw debris: streaks flung outward from the
tile, decelerating and fading, twenty of them when a structure comes down and
fourteen when one merely takes a hit.

The two bursts are shaped differently on purpose. A structure coming down
throws wreckage over its neighbours — 3.2 tiles, over 0.7 seconds. A hit stays
inside its own tile, and that bound is not decoration: a sweep lands a hit on
every structure it damages, so debris that crossed tile edges would turn a
raid on a large base into a solid sheet rather than a series of impacts. With
reach spoken for, a hit's weight comes from its count and its lifetime.

Each spark's trail is tied to how fast it is actually travelling, so a burst
opens with long streaks and settles into short ones. A fixed length reads as a
ring of dashes sliding outward rather than as something thrown.

Nothing about this is engine state. The sparks are derived from the flash
records the renderer already kept, so they share a flash's colour, lifetime
and retirement for free, and the scatter is a hash of tile and spark index
with no time term — which is what structurally prevents a burst re-rolling its
own shape every frame.

### A dev console, behind `FERAL_DEV_CONSOLE`

Provoking a sweep in play means waiting on a 1.2%-per-tick roll, and wearing a
single structure down to nothing takes hundreds of them. That cost is why
visual work ships unplayed, so there is now a keypad for it: `` ` `` on the map
opens five rows — force a sweep, damage the nearest structure, destroy it,
spawn a wild encounter, or burn 25 cycles.

Gated by an environment variable, like `FERAL_DEV_ARENA` and
`FERAL_DEV_REVEAL` before it, and unreachable in a build a player runs.

Every row calls the same code the game calls. `raid_check` and
`maybe_spawn_wild_creature` were split into a roll and a body, and the console
fires the body — so what it puts on screen is evidence about the game rather
than about the console. The damage row takes a percentage of maximum
Durability and is held one short of lethal, because the point of it is to be
pressed repeatedly at the thing you are watching.

### The base slab's corners, on saves that predate them

`0.5.3` chamfered the base platform, and a save written before it kept its
square slab — the cut is applied when the floor is stamped, and loading
restores a zone map verbatim. No migration is needed and none was added:
breaching re-stamps the slab at the new spawn point through the same
predicate, so a legacy base is repaired by the next zone. That is now pinned
by a test, since it is the reason not to write the migration.

## 0.5.3

### The base slab has its corners cut

The platform stamped around a Home was a flat 15x15 square, and it read as
one — a box the game had put down, rather than something the player had
built. Its four corners are now chamfered: each loses the corner tile and
the two beside it.

- The cut is **footprint, not paint**. `Platform::covers` is the one
  statement of the shape, and the build check measures against the same
  predicate — so a tile with no floor under it has nothing standing on it
  either, and a machine cannot hang off a rounded corner onto wild ground.
  A build refused there reads "Too far from Home".
- It costs **12 of 225 buildable tiles**, and the depth is one constant.
  `tuning::PLATFORM_CORNER_CUT` is the chamfer in diagonal steps; `0`
  restores the square exactly.
- Taking the slab up still sweeps the whole build box rather than the cut
  shape, which is the one place the two deliberately disagree. Nothing else
  overrides terrain near a base, so clearing a tile that was never stamped
  costs nothing — while a save written before today still has floor at its
  corners and would otherwise keep it forever once the Home came down.
- The renderer needed no change at all. The slab is drawn from
  `Biome::Platform`, so a cut corner draws as the terrain it reverted to.

An existing save keeps its square slab until the next breach re-stamps one,
since the shape lives in the saved tile overrides rather than in the seed.

## 0.5.2

### Watch a program carry a load to the depot

Hauling has been simulated since depots landed and was never once visible.
A posted program walked to its machine, took a clogged buffer's output to
the nearest depot with room, put it down and walked back — every step of it
pathfound, saved, and drawn nowhere, because the map filtered every tamed
program off the screen. The base was a still life with numbers changing in
it.

- A posted worker is now **drawn while it is out on an errand**, and only
  then. At its post it sits under its machine's own glyph, so a base at rest
  still reads as buildings and motion is the one thing that draws the eye —
  a program appearing *is* the news that it has left to deliver.
- The bobbing mark goes **with it**. One sentence decides where the mark
  lives: on the program when the program is drawn, and on the structure when
  it isn't. So a machine wears it at rest exactly as before, the worker
  takes it along on the trip, and a guard — which is never drawn, since
  nothing walks one to its post — leaves it on the structure for good.
  Exactly one mark per posted program at every instant.
- A machine that is **full with nowhere to send its output** is a dead end:
  no errand starts unless a depot has room, so the worker never leaves and
  the buffer never drains. Its mark stops bobbing, turns orange and blinks
  slowly. Keyed on a depot having *room* rather than on one existing — a
  depot that has filled up is no better than none.
- Nothing tamed but a working program is ever drawn. Nothing walks a guard,
  an idle program or a party member, so each keeps whatever tile it was
  standing on when it took the job, and drawing it would put a glyph
  somewhere the program isn't.

No simulation changed. `at_station` and the `collect::ORTHOGONAL` list it
shares with the player's own collect are untouched — a worker still stands
beside its machine, it simply isn't drawn there. With no depot built there
is still no errand, so on a depot-less base nothing is ever drawn at all.

## 0.5.1

### Rolled encounters: fight what the zone would actually throw

The arena could only ever answer half the tuning question. `opponents`
names a composition — "what if zone 1 threw nine at me" — which is a fair
question and one the game itself would never ask. The other half is what the
game *does* throw, and finding that out meant playing to the fight.

- A scenario may now name a **context** instead of a composition:
  `encounter: Some(Field(biome: OpenGrid))` or
  `encounter: Some(Stack(biome: Mainframe, depth: 5))`. Staging then runs
  the game's own spawn machinery for that context and fights whatever comes
  out. Mutually exclusive with `opponents` — one scenario asks one question.
- The zone comes from the player row rather than the encounter, because
  `ZoneLevel` is one resource driving both gear and enemy scaling and a
  second zone would be two answers to one question. The biome is on the
  encounter, because it alone decides the species pool.
- A `Stack` encounter **descends for real**, through the same
  `Game::enter_frame` play uses, so the depth multiplier, the group curve
  and Trace all apply. Depth is not a stat multiplier bolted on — depth 5
  fields four groups where depth 1 fields one.
- `reps` gains a meaning it did not have: a rolled encounter rolls its own
  pack, so fifty reps **sample the distribution** a context fields rather
  than repeating one composition. Every rep therefore records the
  composition it fought — the bin prints it, the result screen draws it, and
  the report carries it.
- A rolled pack is capped by the zone's own ceilings, unlike an authored
  one, because it *is* the game's own fight. It warns about nothing:
  nothing was asked for past a ceiling, because nothing was asked for.
- On the arena screen, an `Encounter:` row cycles Authored / Field / Stack,
  with a biome picker and a depth. The biome list is built from the loaded
  roster — walkable, and lived in by something — so the picker cannot offer
  a biome the roll would refuse, and a mod adding the first StaticField
  resident gets it offered for free.

Three limits are stated in `dev-arenas/README.md` rather than left to be
found: zone 1's field roll is the opening ring and so cannot reach that
zone's ungentled roster, a field roll is one habitat spawn roll and so
fields one species group, and a Stack roll is an ambush and never a boss.

### Fixed

- Arena opponents were spawned before the rep's seed was installed, so every
  rep fielded the same potential rolls and only the battle varied with the
  seed. The seed now covers the composition and the fight together. A loss
  seed pinned from an older report no longer replays the same fight.

## 0.5.0

### The interactive arena: play the fight you were only measuring

`0.4.1` shipped a harness that runs a real fight offline and reports what it
cost. What it could not do was let anyone *watch* one. The party plays the
game's own All-Attack every round — deliberately, so the tester cannot
invent decisions the game never makes — but that means **no companion
Special ever fires**, and an arena number is a floor on the party's output
rather than a measurement of it. The second gap was authoring: a scenario is
a `.ron` file, so "what if it had one more of those, and I were wearing the
other weapon" meant editing text, saving, and re-running.

Both close here. `FERAL_DEV_ARENA=1` puts an **Arena** row on the main menu.
Behind it is the same `Scenario` the `arena` bin runs — not a parallel
builder type, so a knob added to the schema cannot exist in one tool and not
the other — edited row by row, and fought in the **whole battle interface**.
Specials fire, items are spent, targets are chosen, jacking out is on the
table, because a person is pressing the keys.

- **The builder.** Up/Down move, Left/Right adjust the number under the
  highlight, Enter opens a picker of species or items drawn from the asset
  directories, Backspace removes a row. The loadout rows disappear when the
  player source is not `Fresh`, because the engine treats an authored
  loadout beside a save as an error rather than ignoring it.
- **The result screen.** Won or lost, rounds, HP left, companions down, the
  seed, the staging warnings, and the scrollable round-by-round transcript.
  `[R]` refights the same seed and `[N]` steps to the next — the same
  `seed + n` the bin's reps walk, so a fight watched here replays there.
- **Round trips with the bin.** `[L]` loads a scenario from `dev-arenas/`
  and `[S]` writes one back, so a fight built by feel is measured fifty
  times without retyping, and a loss seed from a report is watched by hand.
- **Nothing is capped, and nothing is silent.** A composition past what the
  zone could really field is built as asked and warned about — on the status
  line as the fight opens, and again on the result screen. "What if zone 1
  threw nine at me" is the question the tool exists to answer.

Unset, the flag makes none of this reachable and none of it loaded. An arena
session also touches no disk at all: no save, no `profile.ron` — a rung
earned in a tester's fight would otherwise be paid out to every future new
game — and no `run_history.log`, so a lost fight against a Permadeath save
lands on the result screen rather than on Game Over.

Under it, the engine's staging and outcome-reading were split out of the
headless loop (`arena::stage`, `arena::Watch`), so the played fight and the
measured one are one code path and cannot disagree about the RNG stream or
about what a fight cost. Both shipped scenarios produce byte-identical
reports across that refactor.

No save-format change.

## 0.4.1

### The battle arena: run a real fight without playing to it

Tuning a fight used to mean one of two things — start the game and grind to
where the fight lives, or reach for `balance_sim`, which answers instantly
and answers a different question (no RNG, no initiative, no abilities, no
items, no status effects). The arena is the third option: the real fight, on
demand, repeatable, with the composition chosen rather than rolled.

- A scenario is a RON file in `dev-arenas/`. It names who is fighting — a
  fresh player with an authored loadout, a save, or a `dev-saves/` template —
  and who they are fighting, then how many seeded reps to run.
- `cargo run --bin arena -- dev-arenas/opening-fight.ron`. At `reps: 1` it
  prints the transcript round by round in the game's own wording; above 1 it
  prints win rate, mean and median rounds, mean HP left, companions downed,
  and **the seeds of the losses** — pin one and that fight replays alone.
  Either way it writes a structured report for working with later.
- Opponents are spawned for real, so the zone multiplier, the potential roll
  and wild routines all apply. The composition is honoured verbatim past
  what that zone could really field, because "what if zone 1 threw nine at
  me" is a legitimate question — with a warning naming the ask, the ceiling
  and the zone, never a silent cap.
- Three scenarios ship: the fight the game opens on, a geared zone-3 party
  against a full group, and a template player against a boss.
- Its blind spot is stated rather than hidden: the party plays the game's
  own All-Attack, which fires no companion Specials, so an arena number is a
  floor on the party's output. `dev-arenas/README.md` says so too.

Nothing the arena does is written back to a save, which is what lets it
point at a real one without risk. It is a measuring instrument, not an
assertion — `balance_sim` remains the balance regression gate.

### Fixed

- A test fixture could set a companion to a level play cannot reach, and
  gave it no stat growth for the levels it did set. `set_level` now awards XP
  through `progression::add_xp`, so the growth lands and `CREATURE_MAX_LEVEL`
  binds.

## 0.4.0

### Breaking: gear fuses per physical copy, not per item type

Existing saves will not load (`SAVE_FORMAT_VERSION` 24 -> 25). There is no
migration path — see that constant's docs.

- Fusing gear used to upgrade the item *type*, so every spare and every copy
  picked up afterwards equipped at the fused tier. It read as a display bug
  in the inventory screen and was an accurate report of the model. A fusion
  now consumes two copies at one tier and yields **one** stronger copy at the
  tier above; spares stay ordinary, the way a fused program already worked.
- The ladder's price in base copies rises accordingly: 2 for a T1, 4 for a
  T2, 8 for a T3. A T3 used to cost 6 and upgrade the whole stack.
- The inventory, trade and gear-swap screens list one row per `(item, tier)`,
  so `Arc Lance T1/3 x1` and `Arc Lance x3` are two rows. A worn copy still
  counts as one of the two a fusion needs and picks up the new tier live.
- A trader's buyback shelf remembers the tier it took, so buying back a
  mis-sold T3 returns a T3 rather than an ordinary copy. Unit prices are
  unchanged at every tier.
- Fused copies are not recipe or machine input, and never were: every recipe
  reads the ordinary cargo stack, which is now by definition the unfused one.

## 0.3.4

### Changed: the base floor is darker again

- `Biome::Platform` drops to roughly two-thirds of the `0.3.3` navy. Same
  colour, less of it — the floor sits further under the glyphs and
  machine-status outlines it is a backdrop for.

## 0.3.3

### Changed: the base floor is dark navy, not grey

- `Biome::Platform` goes back to the dark navy of `0.3.1`. The grey shipped in
  `0.3.2` was the right darkness but the wrong colour, which only became
  answerable once the floor had been seen on screen — `0.3.2` was cut against a
  build nobody had actually looked at.

## 0.3.2

### Changed: the base floor is dark grey

- `Biome::Platform`, the floor the player lays under a base, goes from deep
  navy to a neutral dark grey — the only achromatic terrain tint in the game.
  It is the only biome that covers whole screens, and the base is the one place
  a dozen glyphs and machine-status outlines have to read at once, so the floor
  now carries no hue to compete with them. The map's passability rule is
  unaffected: it turns on red-dominance, and grey is not red-dominant.
- The intermediate step is unrecorded above: this floor was bright cyan through
  `0.3.0` and was darkened to deep navy during `0.3.1` without a changelog
  entry. Noted here so the trail from cyan to grey is not missing a link.

## 0.3.1

### Added: two content surfaces for mods

- `assets/crash_logs/` — a new content directory, one `.ron` file per
  entry, with the lines pooled and ordered by id. See
  `assets/crash_logs/README.md`.
- `taunts` on species files — optional cosmetic lines a program says in a
  fight. `#[serde(default)]`, so every existing species file and every mod
  keeps parsing untouched. See `assets/species/README.md`.

No save-format change, so this is a patch: existing saves load unchanged.

## 0.3.0

### Breaking: save format 23 → 24

`CreatureSave` gained a field, so existing `.bin` saves stop loading.
Templates under `dev-saves/` are RON and were updated in place.

### Changed: the zone map is terrain now, not a wall of characters

Ground used to be a character per tile — `#` for Mainframe, `~` for the Data
Void, `^` for Black Ice — in seven different colours. It is drawn instead:
circuit traces, drifting specks, a lit slab under your base, shards over the
ice. Entities keep their glyphs, so the things that move read as things that
move rather than competing with the floor.

Colour now answers one question and pattern answers the other. Every biome
you can walk on is cool cyan-teal; the two you cannot — the Data Void and
Black Ice — are hot amber. They were previously just two more colours in a
palette of seven, which meant the map never actually told you where the
ground stopped. It does now: the walkable tile beside a hole lights the edge
it shares with it, so every impassable region is ringed like a shoreline.

### Added: your base has people in it now

A program posted to a machine used to be a name on a screen, frozen on the
tile you caught it on. It now stands at its machine, and when that machine's
output buffer fills it carries a load to the nearest Depot and walks back.
You can watch it happen.

Two consequences. A machine no longer stops dead when it fills — it sheds
five units at a time and keeps going, so how long your base runs unattended
is a question about where you put the Depot rather than a fixed number. And
a program takes a moment to reach its post when you assign it: it walks
there, and produces nothing until it arrives. A machine whose program is
away says so.

The Depot itself holds a hundred units and costs twelve Core Fragments. You
collect from it the way you collect from anything else — stand next to it.
Build a second one across a sprawled base and half your programs' walks get
shorter. Build none and nothing changes at all.

### Breaking: save format 22 → 23

`CreatureSave` gained a field, so existing `.bin` saves stop loading.
Templates under `dev-saves/` are RON and were updated in place.

### Added: a program can be more than a companion

There is something you can do with a tamed program besides fielding it or
putting it to work. It is not written down anywhere, and it is not going
to be — but a program used that way lends you its strength directly, and
sometimes acts on its own while you fight. Which program you pick decides
what that is worth.

It costs you the program's other roles while it lasts: it will not stand
in the battle line at the same time, and it takes the place of whatever
you were carrying. Both come back the moment you stop.

### Changed: the way to the next zone runs through the Stack

Portal Fragments now come from one place — the guardian in a Stack lair —
and the payout is `4..=8` per frame of depth, so the bottom of a stack is
worth the walk back up. Three faucets closed to make that true: the flat
35% roll every kill used to carry, the cache in a Stack wall, and the
surface boss. You breach by going down, or you don't breach.

Each of the three fights that survive now pays exactly one kind of thing:

- **A Stack lair boss pays progression** — fragments, and nothing else pays
  them.
- **A surface boss pays power** — high-end gear instead of fragments, drawn
  from a band of the item value ladder that climbs with the zone. A zone-1
  boss hands out standard and researched pieces; by zone 4 it is dropping
  the premium tier. The band is derived from the `value` and `equipment`
  fields items already have rather than a new one, so a modded item joins
  the pool by existing — see `assets/items/README.md`.
- **A nest pays roster** — Credits as a floor, and a coin flip on an
  orphaned program of the nest's own species, which joins you free. Clear
  the nest of the program you want. A full roster loses it and says so.

Nests no longer pay fragments at all, which is the same rule from the other
side: sustained surface work is worth doing, but it is not a route to the
next sector.

Saves load unchanged. Existing runs keep whatever fragments they had
banked.

### Breaking: field buffs can fire on a cadence — save format 21 → 22

`FieldBuff` abilities take an optional `interval` in their `.ron`: how many
turns pass between firings, defaulting to 1 (every turn, which is what they
all did before). It means something only to the three over-time kinds —
`Regen`, `Coolant`, `Trickle` — since the rest have no per-tick effect to
space out. See `assets/abilities/README.md`.

Repair Loop Single is the first ability to use it: **+2 Integrity every 4th
turn for 300 turns**, where it was +2 every turn for 100. Slightly more total
healing over three times the window, so it reads as a long convalescence
rather than a burst.

A running buff carries its own cadence, which is a new field on saved state
— every existing save stops loading. The checked-in `dev-saves/` templates
are field-named RON and survive.

### Added: entity menus show the icon of what they list

Every picker that lists a program or a structure — party, fuse, extract,
cronjob, guard, work, demolish, upgrade, symlink, routines, field cast,
manifest — now draws that entity's map glyph in its map colour beside the
row. The glyph's colour is independent of the row's, which on those screens
already means fusion tier, CRITICAL Integrity or idleness.

Those menus also arrive in a fixed order now: alphabetically, then by
position. They were being drawn in ECS iteration order, which is not stable,
so the same menu could list the same base differently between openings. The
party keeps slot order, since arranging the battle line is what that screen
is for.

### Added: deploying tells you what you're short of

A deploy refused for want of materials now says so in the base log, naming
every shortfall at once with what it needs and what you hold — the refusal
used to live only in the status line, which clears itself after four seconds.

The Deploy Direction screen also names the structure you're about to place,
with its description and its cost, instead of being an unlabelled compass.

### Changed: Attacker and Defender pay 3 a level

Both perks now grant +3 permanent Attack or Defense per level instead of +1,
for the same 2 Perk Points. They were the two flattest purchases in the
catalogue and, at 1 a level against a Perk Point earned per player level,
buying either read as doing nothing.

This moves them past the Payload Tuning and Siphon Protocol affinity perks
for the opening levels of a run — those multiply a magnitude that grows with
level, so they still overtake a flat +3 from around player level 3 and keep
widening. Their own rate is unchanged; what changed is that the flat perk is
now the better early buy rather than the worse one.

### Changed: Research Data is banked, not carried

Research Data is no longer something you pick up. A Research Node delivers
it straight to you the moment a cycle completes, so there is no walking over
to the node and no collect key; it has no inventory row; and no trader will
touch it. The only place it appears is the research screen, which is the
only place it does anything.

It was already half-abstract — it never counted against your cargo — and the
rest of it behaving like a crate of salvage was the part that never made
sense. Two things that were true and are no longer: it had a 200 ceiling,
which is gone entirely, and it could be sold for 1 Credit apiece, which made
a Research Node a slow money printer.

For mods, `bank_limit` on an item is replaced by `banked: true`. Everything
above follows from that one flag, so a mod's own banked currency behaves the
same way without asking for it — see `assets/items/README.md`. One gap worth
knowing: an `assembles` machine producing a banked item still fills its own
buffer and still has to be collected. Existing saves are unaffected.

### Changed: the staffed mark bobs

The green square saying "a program is posted here" now lifts a few pixels
and settles, about once a second, so a working base reads as busy rather
than as a field of static dots.

Each tile is offset in phase from its neighbour. In lockstep the whole base
blinks as one and looks like a screen artifact; out of step it looks like
separate workers. The phase is keyed to the world tile rather than the
screen, so a mark doesn't reshuffle its timing as the camera pans.

The lift is upward only. The mark's rest position is held off the tile's
bottom edge on purpose — `outline_open` draws a chained pair's shared wall
as an absence of line, and a mark flush into that corner reads as painting
one back in — so a down-swing would spend an inset that is load-bearing.
Effects-off holds it still, like every other ambient animation.

### Fixed: the Compile screen quoted a price Lean Compiler had already cut

A player who had bought Lean Compiler was still told an ICE Breaker needed
three Core Fragments while holding two — and the compile would have gone
through, because `Game::craft` charges the discounted price. The same screen
contradicted itself a line further down: "Max affordable right now" reads the
discounted path and said 1.

The discount used to be applied in `craft_cost`, which `craft` calls, while
the screen formatted the raw recipe out of `craft_recipes`. It is now applied
in `craft_recipes` itself, the one point every player-facing recipe passes
through, and `craft_cost` is a lookup into it. Quoting and charging can no
longer come from two places.

Nothing a structure consumes moved. A machine runs its product's authored
`craftable.cost` through `systems::assembly_recipe`, which reads `ItemDb`
directly, and the Recipes chains do the same — so the player's bench perk
stops at the player's bench, which
`lean_compiler_does_not_discount_what_a_structure_consumes` now holds.

### A structure with a program posted to it wears a green corner mark

"Someone is assigned here" used to be a yellow tile outline, and machines
took that outline over when they started wearing their state on it —
running, starved, clogged, idle. From then on a machine could never draw
the yellow at all, so the only surviving trace of an unstaffed one was the
grey of `Idle`: a colour meaning absence, on an axis already carrying three
other things. A base of machines gave no reading of which ones had a
program on them.

It is now a small green square in the tile's bottom-left corner, drawn for
every structure with a worker, machine or not. The outline says what the
machine is doing; the mark says whether anybody is on it. The mark is held
clear of the tile's edges on purpose — a chained pair's shared wall is
drawn as an *absence* of line, and a square flush into the corner would
read as painting one back in.

The two channels disagree in exactly one case, correctly: a guard counts as
assigned but runs no job, so a guarded machine draws grey and marked.

### Two routines that act on a Stack frame instead of reading it

Everything the player had for a frame either read the maze — the view cone,
the map, a breakpoint — or was inflicted by it: a fault, corrupted ground.
Nothing let them act on its shape. Two new field routines do, reached
through a new research node (**Address Translation**, 50 Research Data,
after Deep Analysis) and installed like any other: a Routine Disk and one of
your six slots.

| Routine | Costs | Does |
|---|---|---|
| Buffer Overrun Party | 12 Fatigue | Steps through exactly one wall ahead, landing on the open cell beyond |
| Wild Jump Party | 20 Fatigue | Moves the party to any cell of the frame you point at — and kills you if that cell is solid |

Buffer Overrun is deliberately one wall thick. Any deeper and a cast from
the frame edge cuts a diagonal across the whole maze; at one wall it opens
the room next door and nothing further. It is refused, spending nothing, if
the rock runs deeper than one cell or if the far side is off the frame.

Wild Jump is a `goto` to an address nobody validated. Aim it with a cursor
on the frame map: cells you have already walked are safe *because* you have
walked them, and the unlit part of the map is exactly the risk. Nothing
warns you beyond the map itself — that is the mechanic. Arrival hazards fire
on a jump exactly as on a step, so jump onto a fault and you fall through
it, and jump onto an uncleared lair and you have roused it.

Both refuse a landing behind an unopened seal, and refuse landing on the
seal itself — a refusal rather than a death, since the rule exists to keep
"earn your way to the guardian" intact rather than to punish a misclick.
Both are Stack-only and greyed with the reason on open grid, both raise
Trace on success, and neither ever appears in the battle Special menu or on
a wild program.

### Eight routines that fill the holes in the ability families

Charting the abilities made a gap visible that a directory listing never
could: the files are named for flavour, so nothing about `bus_fault` sitting
in `assets/abilities/` says it is the Pipeline Stall family reaching the
whole field with nothing between it and one target. Six families had holes
like that. Eight new routines close every one of them:

| Routine | Reaches | Does |
|---|---|---|
| Pipeline Stall Group | one group | 6 damage, 30% chance to lock up |
| Fork Bomb Everyone | the field | 8 damage, 20% chance to leave bleeding |
| Leech Everyone | the field | 4 damage, a fifth of it back to you |
| Etch Everyone | the field | −3 DEF for 3 rounds |
| Throttle Everyone | the field | −3 ATK for 3 rounds |
| Etch Single | one target | −5 DEF for 3 rounds |
| Throttle Single | one target | −5 ATK for 3 rounds |
| Flush Cache Single | one ally | clears its status condition |

Every one is hunt-only, like every routine already in its family — the way
to get them is to meet a wild program running one and decompile it rather
than kill it. That takes the hunt-only set from twenty to twenty-eight.

The magnitudes are not new judgement calls. The shipped set already charges
a consistent price per scope — Single costs 8–10 Fatigue on a 2–3 round
cooldown, Group 11–13 on 3–4, Everyone 15–18 on 5 — and rider chances
already decay as the scope widens, 60% at Fork Bomb Single down to 35% at
Group. Each new routine reads off that ladder, sits below its own family's
next tier down on magnitude and above it on cost. Packet Shred Everyone
stays the one routine that gets better per point as it gets wider, which is
the whole reason it is a boss routine no player is taught.

Two saps that only ever hit a group now run the full width — a −5 DEF strip
on one target for 8 Fatigue against −3 across the field for 16 — and Flush
Cache stops being a party-only routine, so clearing the one member who is
actually bleeding costs 4 rather than 7.

`every_battle_ability_family_is_contiguous_from_single_upward` is what found
the Pipeline Stall hole and what will find the next one: a family occupies a
run of scopes starting at Single, with no gaps. Field routines are excluded,
since a scope ladder is not what that half of the set is organised by.
`every_everyone_scope_routine_pays_the_everyone_tier_price` pins the ladder
the magnitudes were derived against — nothing gated it before.

### A charted stat sheet for every content directory

`docs/roster.md` now has six siblings, one per remaining moddable directory
under `assets/`: items, abilities, structures, research, achievements and
perks. Each follows the roster's shape — a transcribed table at the top of a
`docs/*-gen.py`, ASCII charts generated from it, and prose that says what the
numbers mean rather than restating them.

What the charts surface is mostly relationships the individual files cannot
show. The research page draws the tech tree from its own `requires` edges and
prices each node by its whole chain, which is how the six end-of-branch nodes
turn out to carry more in prerequisites than the dearest single node costs.
The structures page derives the four production lines from which item feeds
which. The abilities page reads the naming scheme back out of the display
names — an id is flavour, a name is a spec — and finds twelve families, none
of them with a hole in it. The achievements page shows the ladder spending
its Perk Point ceiling exactly, 5 of 5, so a fourteenth rung paying points
has nowhere to go.

Two of those pages document a fact this work had to check rather than assume:
only two of the four end benches are built out of their own feeder's product,
not four, and `each_bench_is_built_out_of_what_its_own_feeder_makes` asserts
exactly that pair.

`assets/fonts/` and `assets/sounds/` are skipped — binaries with nothing to
chart.

### Eight pieces of content renamed out of the occult register

The Grid runs on malware and systems vocabulary, and eight things had drifted
away from it. Three species — **Ghost is now ZeroDay** (map glyph `z`),
**Wraith is now Crawler** (`r`), **Phantom is now Proxy**, which keeps its
`p` and whose Backdoor and Spoof moves already read that way. Three items:
**Daemon Fang → Shim Blade**, **Probe Daemon → Probe Service**, **Wraithsteel
Plate → Nullsteel Plate**. One routine: **Ghost Protocol Party → Stealth
Protocol Party**. One achievement: **"Ghost in the Wire" → "Something in the
Wire"**. ZeroDay's Haunt attack is now Fray, and its Static Wail is Static
Burst.

This renames ids and filenames, not just display names, following the
precedent set when the Daemon species became SubProcess. **Existing saves
lose these species, items and the routine on load** — their ids no longer
resolve. The `dev-saves/` templates were updated in place and still work.

One consequence worth recording, because it will recur: species are loaded in
sorted-id order, so renaming three of them moved every later draw from the
shared `GameRng` stream and changed the outcome of nine seeded tests. All
nine turned out to be latent fragility rather than the rename breaking
anything — a Stack ambush that now fires one step earlier and blocks a
fixture's walk, an opening-ring census that swept nest guardians it was
documented not to cover, a taming test counting refused attempts as charged
ones, and a nest test whose walkable strip was narrower than the tether
square a replacement guardian scatters across. Each was fixed at the cause,
so the next content change does not move them again.

### Fusion reads at a glance, and gear stops at three like everything else

Anything you have fused now draws in its own colour wherever a menu lists
it: cyan while it can still go into another fusion, magenta once it is at
3/3 and is a finished product. That covers the party screen, both fuse
pickers, the extract and cronjob pickers, the inventory and its equip
picker, and both of a trader's sell lists. The battle roster and the `B`
roster are deliberately left alone — their colours already mean HP state and
species.

Gear fusion is now capped at three, the same ceiling a program's lineage has
always had. It was previously unbounded, which made a stack of one item type
into an open-ended stat multiplier. Item rows name the ceiling the way a
program's do (`fusion T2/3`, `fusion T3/3 - maxed`), and a fusion refused for
being at the cap spends nothing.

A save that already holds an item above tier 3 has that ledger entry lowered
to 3 on load. A copy you are *wearing* keeps the bonus it was equipped with
until you take it off — its stats are already banked, and quietly restating
them would have left the difference welded into your base stats.

### Difficulty comes from the zone, not from how far you walked

Wild programs no longer get stronger the further you are from your base.
Distance from the danger origin used to scale their stats by up to 3x and
double group size every 15 tiles, on top of the zone's own doubling. Both
are gone. A zone now has one consistent difficulty, and the escalation you
feel comes from the commitments you make — funding a Portal, descending a
link — rather than from which direction you wandered.

This also fixes a leak into the Stack that was never intended. Every
underground spawn is placed at the surface tile you descended through, so
a link out at the edge of the sector multiplied *every fight in that whole
stack* by its own distance. Two stacks at the same depth could differ
threefold on nothing but where their entrances happened to sit.

Group size and group count still ride one shared curve, so they cannot
disagree about how dangerous a place is; it just reads the zone on the
surface and the frame depth in the Stack.

**The zone-1 opening ring survives**, now as an explicit radius —
`OPENING_RING_TILES`, the build radius — around your base rather than a
band derived from the old distance curves. It travels with your base and
still fields only programs a fresh player can actually beat.

### The field hits harder and bosses hit less hard

With distance gone, a zone's baseline is all it has, so the field roster's
base stats are up 25%. The four species the opening ring draws from — Scan
Drone, Glitch, Sprite and Sub-Process — deliberately sit it out: they are
the only four a bare level-1 player beats solo, and raising them would
quietly empty the ring.

Bosses move the other way. At Stack depth 2 an Overseer was 2.6x an
ordinary program's HP and 3.1x its attack, which made the gap between a
depth-2 fight and a depth-2 lair far larger than the depth between them.
Overseer and Wintermute now sit near 1.5x the toughest ordinary program,
and past zone 1 a boss arrives with an escort group of an ordinary species
from its own habitat. A boss is a harder *fight* now rather than a harder
single opponent. Zone 1 holds one group, so the opening zone's boss is
still met alone.

The grind-only level curve moves from 1/15/33/61/117 to 1/19/40/77/149
across zones 1-5. The shape is unchanged at roughly 2x per zone; the whole
curve sits higher.

### Upgrade tiers are gated by the zone you have breached to

A structure is deployed at Mk1 wherever you build it, and the zone you have
reached is what unlocks the tiers above: zone 2 frees Mk2, zone 3 frees Mk3,
up to the Mk5 every shipped producer caps at. Nothing upgrades at all in zone
1. Tiers already paid for are unaffected and ride through portals with the
rest of the base.

Previously a Mining Node could be walked to Mk5 without ever breaching, which
made the deepest producer in the game reachable from the opening zone. It now
follows the rule gear already followed — reaching zone *N* is what unlocks
level *N* gear — and the two ladders line up, both spanning 1..=5.

A structure sitting at its zone ceiling is still listed in the upgrade
picker rather than filtered out, and the row says which zone would free the
next tier. Hiding it would have taken the whole **Upgrade a structure** row
out of the base menu for all of zone 1, and a player who had never breached
would never have learned that upgrading exists.

This shifts the economy: Core Fragments in zone 1 now go only to building and
recipes, with the upgrade sink opening on the first breach. Nothing gates
that number — `balance_sim` models a run's battle curve, not its build
economy — so it is unmeasured, and unplayed.

### Raids are now GC Entropy Sweeps

A rename, player-facing text only. The Grid's periodic garbage collection
passing over anything you have left standing reads as what it is, rather than
as bandits. `{structure} takes N raid damage` also became `{structure} loses
N Durability to a GC Entropy Sweep` — the event is a noun phrase and does not
survive being pushed into an adjective slot.

No mechanical change. `MessageKind::Raid`, the `raid_defense` and `raidable`
structure fields and the tuning constants keep their names, so saves load and
existing mods parse untouched.

### The inspector reads structures, not just programs

`x` and a direction found the nearest *creature* that way and opened its
manifest; a Refinery standing right in front of you was invisible to it. It
now finds the nearest creature **or structure** and opens the right sheet for
whichever it is.

Nearest wins, with no priority by kind — both are gathered in one walk, which
is what makes that answerable at all. A creature exactly as far away as a
structure keeps the target, since it is the one that might have wandered off
by the time you look again.

The structure sheet is the `B` roster's row for that one machine — tier,
position, durability, who is posted to it, whether it is idle or starved, and
what is in both buffers. It is built by calling the roster's own line
builders, so the two screens cannot drift into disagreeing about the same
machine. Any key closes it.

**Structures are not offered while you are in the Stack.** Your `Position`
stays pinned to the surface entrance tile down there, so an unguarded scan
would report your base four frames overhead as lying off to your east.
Creatures are unaffected — that is already how the inspector behaves
underground.

### Production lines are lines again

A bench took two intermediates off two separate feeder legs, so anything the
factory made needed **two chains stood up and a corner to put the bench in**
before a single unit came out. Standing up a Fabricator meant a Log Scraper,
a Transcriber, a Power Conduit *and* a Winding Node — five machines, five
programs against a starting roster cap of three — and getting the geometry
right on top. That is a puzzle, not a decision.

Every shipped machine recipe is now **one ingredient**, and the four
intermediates match the four benches one-to-one:

```
Mining Node   -> Refinery     -> Armory        (Hardened Shell)
Mining Node   -> Lathe        -> Disk Press    (Routine Disk)
Log Scraper   -> Transcriber  -> Fabricator    (Trace Sniffer)
Power Conduit -> Winding Node -> Assembly Bay  (Patch Routine)
```

Three machines, three programs, a straight run of three tiles — so one line
is exactly what a fresh roster can staff, and which line you build first is
which thing you get first. The recipes:

| item | was | now |
|---|---|---|
| Hardened Shell | 2 Blocks + 2 Coils | 3 Bytecode Blocks |
| Routine Disk | Substrate + Wafer | 2 Blank Substrate |
| Trace Sniffer | 3 Wafers + 2 Coils | 5 Logic Wafers |
| Patch Routine | Block + Coil | 3 Charge Coils |

The Patch Routine swapping Blocks for Coils is the one that isn't just
arithmetic: without it the Winding Node loses every automated consumer and
becomes dead weight. With it, no intermediate is orphaned and all four raw
taps stay in play.

**Build costs follow the same rule.** The Assembly Bay cost Bytecode Blocks
while *running* on Charge Coils, so standing one up needed two unrelated
lines — exactly the tangle this removes. A bench is now bought with its own
feeder's product: the Assembly Bay costs Charge Coils, the Disk Press costs
Blank Substrate.

Researched armour and module recipes are untouched and still name several
ingredients — those are hand-crafts at a bench, with no adjacency and no
geometry, so they were never the un-fun part. The engine's multi-input
support is untouched too, and mods may still ship two-ingredient assemblers.

### The Recipes screen says which structure to build for each ingredient

A chain used to start at its raw materials with no hint where they came
from: a Routine Disk began at four Core Fragments and four Raw Trace, and
knowing that meant a Mining Node and a Log Scraper was something you brought
to the screen rather than something you got from it.

An ingredient no recipe makes now leads with the structure that taps it, so
a chain read top to bottom is the build order:

```
Product: Routine Disk
  Mining Node (Core Fragment x4)      -> Lathe       -> Blank Substrate x1
  Log Scraper (Raw Trace x4)          -> Transcriber -> Logic Wafer x1
  Blank Substrate x1 + Logic Wafer x1 -> Disk Press  -> Routine Disk x1
```

An ingredient an earlier step of the same chain produces is named on its
own — a Power Cell reads bare rather than naming the Power Conduit, because
the bench step that makes those ones is already a line above it, and a chain
claiming two sources for one item is worse than either. Ingredients nothing
produces, like scavenged Portal Fragments, likewise stand alone.

Products now carry the units one batch yields, which is what marks the
column as an item rather than another structure. Extractors quote none:
a node's payout scales with its upgrade tier and the zone, so there is no
one number to print.

### Dying in the Stack on Forgiving actually gets you out of it

A Forgiving reboot puts you at your nearest construction. Underground, it was
writing that construction's tile into your position while leaving you in the
maze — so you rebooted at home on paper and came to still four frames down,
with the entrance tile your position had been pinned to overwritten in the
process. The only way home was to walk the frames back up.

A reboot now surfaces the party first, then warps, and says so. With no base
standing you still surface, back onto the link you walked in through.

### Stuns and bleeds now last as long as they say

A status condition was being charged for the round it was applied in. End-of-
round upkeep ran on every round including that one, so the first tick of every
duration was spent within moments of the condition landing.

Stun was the casualty. Every stun in the game is `duration: 1` — the shipped
species moves, Deadlock, Pipeline Stall — so a stun was armed mid-round and
gone by the end of it. It cost the victim a turn only when the attacker also
happened to win initiative that same round; land it on something that had
already acted and it did nothing at all before shaking off. Hard Lock's
advertised two rounds gave one. Bleed had the quieter half of the same bug:
Memory Leak's "Bleed 2 per round for 3 rounds" dealt its first tick instantly
and showed two rounds on the roster.

Conditions now skip the upkeep for the round they land in, so `duration`
counts the rounds after it. A `duration: 1` stun reliably costs its victim the
next round's action; a three-round bleed ticks three times over the next three
rounds. Landing a stun on something that has not yet acted still stalls it
immediately, on top of the round it is owed — outrunning your target is
supposed to be worth something.

Both sides feel this. Wild carriers and stun-carrying species moves have been
getting the same nothing out of their stuns, and now they don't.

### The Compiler runs on Core Fragments

It used to print ICE Breakers out of nothing every 8 ticks. It now compiles
them out of Core Fragments pulled from whatever is touching it — a Mining
Node, in practice — the same way the Refinery and the Assembly Bay work. Its
recipe is the ICE Breaker's own three-Fragment recipe, so the bench and the
machine cannot drift apart; hand-crafting one is unchanged and still needs no
bench at all.

Two consequences worth knowing before you rebuild your base around it. A
Compiler needs a program posted to it *and* a feeder orthogonally adjacent,
so it can no longer be worked by hand. And a Compiler and a Refinery both
touching one Mining Node compete for its output — the lower-left machine
takes its share first — so a shared feeder now needs enough throughput for
both, or a node apiece.

At three Fragments per unit against a Mk1 node's one per ten ticks, the
Compiler is a fragment sink you feed from a stockpile rather than a line that
idles at full rate. Its `upgrade` block is retained but buys nothing: an
assembler's rate is fixed by its definition, and tier does not touch it.

### A Data Cache widens the roster by five

Two slots against a base of three was not enough to matter. A full Stack
descent offers four or five orphaned programs, and a roster of five refused
most of them; the five-machine production line wanted every slot a run had.
One cache — still ten Core Fragments — now takes you from three programs to
eight, which covers a whole descent or a whole factory floor and leaves a
party to adventure with.

### Lower-case map keys

Collecting from adjacent structures is now `c` rather than `C`, and the log
filter is `f` rather than `F`. Nothing else on the map asked for a modifier,
and the two that did were the two you press most often while walking. `L` for
the message history keeps its shift: `l` walks east.

### Swap gear from the slot it's in

Picking one of the three equipped rows on the inventory screen (`i`) used to
unequip on the spot, so changing a weapon meant unequipping it, hunting the
replacement down a cargo list of any length, and equipping that. It now opens
a replacement list for that slot: every piece of gear you carry that fits it,
best first, with the row that empties the slot last. Each row shows the bonus
you would get and, beside it, the change from what you are wearing.

A candidate is previewed at the level it *would* equip at, while the worn item
is measured at the level it remembers. That gap is the point — gear doubles per
zone level and locks in when you put it on, so a spare copy of the weapon
already on your back is a genuine upgrade after a breach, and the list now says
so out loud instead of leaving you to work it out.

No engine change: `Game::equip` already returned the outgoing item to cargo
itself, so a swap is one call rather than an unequip that could leave you
bare-handed if the equip behind it were refused — see
[Items and equipment](README.md#items-and-equipment).

### Achievements that outlive the run

Thirteen rungs across four axes — how deep a sector you breach into, how far
down the Stack you get, how many cycles you survive, and which boss programs
you put down. Earning one stamps `profile.ron` at the repo root immediately,
not at run end: a permadeath run that ends badly still keeps what it proved.

The reward is paid at the **start of the next run** — a point into one of
Attack, Defense, Integrity or Decompiler, a Perk Point, or a free program
already yours and waiting to be deployed. Loading a save pays nothing, since
that save's stats already hold what the profile bought when the run began.

`A` on the main menu lists every rung, earned or not, with what it pays and —
where earned — the cycle, the mode and which stat the roll landed on.

Achievements are data: `assets/achievements/*.ron`, one file per rung, so
adding one is a file drop. What is *not* data is the ceiling — at most 8 stat
points, 5 Perk Points and 1 starting program across the whole ladder, asserted
over the real asset files. `balance_sim` simulates one run's curve and cannot
see a cross-run profile, so that assertion is the only gate on how much the
profile is worth. The shipped ladder spends 7 / 5 / 1.

**No save is affected.** The profile is deliberately not part of `SaveData` —
a save is one run and this spans them — so `SAVE_FORMAT_VERSION` stays at 21,
and it will stay there for any field added to the profile later, since RON is
self-describing and every field is `#[serde(default)]`.

The three cycle thresholds (500 / 2000 / 5000) are arithmetic guesses. Nothing
in the repo says how long a run actually lasts and no test can; they want
checking against a real run.

### Every ability name says what it hits

Forty-one abilities had forty-one unrelated names, so nothing about
"Broadcast Storm" told you it hit the whole field while "Cascade Overflow"
hit one group. The picker shows the name before it shows anything else, and
that was the one thing it couldn't tell you.

Names are now `<Family> <Scope>`. The scope is **Single**, **Group** or
**Everyone** against enemies, and **Single** or **Party** on your own side.
The family names an effect rather than a file: `Packet Shred` is plain
damage, `Fork Bomb` is damage carrying a Bleed, `Pipeline Stall` carries a
Stun, `Patch` heals, `Hard Lock` stuns. So the group attack that bleeds is
Fork Bomb Group and the one that doesn't is Packet Shred Group v2.0.

Where two abilities in one family share a scope and differ only in size,
a version tag separates them — Patch Single v1.0, v2.0 and v3.0 restore 8,
25 and 50 Integrity. A major bump is a real step up; a minor one is the
same thing slightly bigger.

Field routines keep their own names and take the suffix, so Deep Scan is
Deep Scan Party. Creature *moves* are untouched — they have no targeting
scope for a suffix to describe, so the Overseer still opens with Kernel
Panic even though the ability of that name is now Packet Shred Single.

**No save is affected.** Ability ids, filenames and the `routine_<id>`
items minted from them are all unchanged; only the display string moved.
A mod that named its abilities the old way keeps working — the loader has
no opinion, and the two new tests only cover the shipped set.

### Armour and modules are made out of the factory

Standard and premium gear was crafted at a bench out of raw Core or Portal
Fragments, which meant the production chains and the gear economy never
touched. Thirteen recipes — six armour, seven modules — now spend what your
base refines instead. Weapons are deliberately untouched, and the Scavenged
tier still compiles from raw fragments with no base standing, because that
is what a fresh run, or one just raided flat, equips out of.

The ingredient follows the stat, so a recipe reads as what the piece does:
Logic Wafers buy Decompiler, Bytecode Blocks buy Attack and bulk, Charge
Coils buy Defense. Armour draws off the Mining Node and modules off the Log
Scraper, while both want Coils — so the Winding Node is now the first feeder
with three machines pulling on it, and where you put things starts to matter.

Premium gear trades roughly a fifth of its Portal Fragment price for refined
goods, so your base does some of the work boss drops used to.

The **Armory** and **Fabricator** each assemble one item on repeat — Hardened
Shells and Trace Sniffers — while staying the hand-craft bench for every
other recipe that names them. They take a program and want both feeders
touching them like any machine, which also moves them into the Assembler
group in the build menu.

Item values are unchanged, so the price ladder does not move and no balance
curve shifts. Saves are unaffected. An Armory or Fabricator standing in an
existing base will report itself *idle* once until you post a program to it.

### A routine is knowledge plus a disk you have to build

**Save format bumped to 21 — existing saves will not load.**

A routine was an item, minted per ability at startup and dropped free into
cargo by research. It is now two things: knowing it (permanent, from a
research node or from breaking a program down at a Compiler) and a blank
**Routine Disk** the base manufactures. Installing burns a disk; popping the
routine back out returns nothing.

Four new machines make the disks — Log Scraper → Transcriber and Mining Node
→ Lathe, both feeding a Disk Press — behind a new **Routine Fabrication**
node that **Self-Execution** now requires, so no node can teach a routine
you have no way to install. Species kits still install free at spawn, and a
new game already knows Decompile.

### Items are worth what they are worth

Every item sold for exactly 1 Credit. A trader's `sell_rate` *was* the
price, applied to whatever crossed the counter, so a Singularity Matrix — 24
Portal Fragments and the rarest thing in the game — fetched the same single
Credit as a Shiv Routine.

Items now carry a `value` in their `.ron` file, and `sell_rate` becomes the
trader's multiplier on it: what a thing is worth is a property of the thing,
what a trader pays is a property of the trader. Printable goods sit at 1,
scavenged gear at 3–8, standard gear at 12–16, the drop-only researched
pieces at 20–60, and premium gear at 80–120.

The ladder is bounded by two rules, both now tested against the shipped
assets, because breaking either turns a base into a Credit press: nothing is
worth more than the ingredients of its own recipe, and nothing a structure
produces on a timer is worth more than the floor. The second is the one that
isn't obvious — a Compiler makes ICE Breakers out of nothing every 8 ticks,
so pricing them at their recipe cost would have out-earned a Mining Node
nearly fourfold. Value comes from what a base *can't* manufacture.

Modded items without a `value` keep trading at 1, exactly as before.

### Power Cells stop piling up

Cells arrived from three directions: the Glitch and the Scrapper both
dropped them 2–4 at a time, and a Stack cache rolled for one at 0.4 — the
highest chance of any item, more than double the next non-catalyst. The
Scrapper now drops Core Fragments, leaving the Glitch as the only source
that walks around, and the cache chance falls to 0.15.

A cache's expected haul goes from 1.55 items to 1.30 — still the "a little
over one" the modding docs promise, and now a test rather than a promise.

The manual's roster table said the Sprite dropped nothing and the SubProcess
dropped Power Cells; both actually drop Core Fragments, and had done for
some time. Its column header also called that field "Works for", which is
not what it does — a tamed program's output comes from the structure it is
posted to, whichever program you assign.

### The map screen's keys are grouped behind `b`, `p` and `i`

**Breaking for muscle memory.** The map bound 27 keys. Seventeen of them now
sit behind three: `b` opens a base menu (deploy, compile, cronjob, work it
yourself, guard, upgrade, demolish, structure roster, research), `p` opens a
party menu (companions, manifests, fuse, install and extract routines,
perks), and `i` opens the pack directly — it is one screen, so it needs no
menu of its own.

Thirteen keys are retired outright and do nothing: `c` `w` `W` `G` `R` `U`
`B` `T` `f` `m` `M` `d` `v`. Keeping them as aliases would have meant the
flat surface never actually shrank, and the help screen would have had two
systems to document. Inspect-a-direction moves from `i` to `x` — perks
moving into the party menu freed the key a player would guess for it anyway.

Collect (`C`), trade (`t`), cast a field routine (`a`) and symlink (`u`)
stay flat despite being on-topic: they are pressed every few turns while
walking, and a menu is a keystroke tax on anything that frequent.

A menu lists only what you can do from where you stand. A row is shown when
its screen would have at least one row of its own — so with nothing
deployed there is no *Demolish*, and with one companion there is no *Fuse* —
and rows whose action needs open ground disappear while you are in the
Stack, where your `Position` is still pinned to the entrance tile above.

Esc from a screen a menu opened returns to that menu rather than to the map;
finishing a job still drops you back to the map.

### The base menu has a *Recipes* row

A read-only list of every conversion your base can run, each walked back to
the raw inputs it bottoms out in. A Patch Routine reads as the four steps it
actually takes — fragments into a Refinery, cells into a Winding Node, then
both into the Assembly Bay — rather than as two ingredients you then have to
go look up one at a time.

The list is built from the loaded assets rather than written down anywhere,
so a modded machine appears in it for free, and the recipe shown is the same
one the machine stages. A step with no structure beside it is one you
compile by hand.

Reference data rather than a view of your base, so unlike the rest of the
base menu it stays available in the Stack.

### The window opens fullscreen, and the UI text is smaller

Was a 1440x900 window. It now opens borderless fullscreen at whatever the
monitor is, and body text drops from 24px to 18px at the reference height —
so the extra room goes to map tiles and log rows rather than to a magnified
UI. The text change applies at every window size, not just large ones.

Nothing else moved: every size on screen is either a fraction of the window
or derived from the font, and the zone map has always shown *more tiles*
rather than bigger ones as the window grows. Zoom (`+`/`-`) remains the only
thing that changes tile size.

There is no key to leave fullscreen, and so no windowed size is carried
alongside it.

### Adjacency-fed production chains

**Save format 19 → 20.** Existing saves stop loading; the checked-in
`dev-saves/` templates are RON and survive.

Machines now have local storage and feed each other by *touching*. A
structure declaring `assembles` pulls ingredients out of the output buffers
of the four structures orthogonally adjacent to it — never diagonally — and
builds one unit at a time. A chain is a physical line across the base, a
machine with two ingredients needs both feeders beside it, and a machine
nobody visits fills up and stops.

A machine's recipe is the *item's* own `craftable.cost`. There is no second
recipe format, so a bench recipe and a machine recipe cannot drift apart, and
any craftable item a mod adds — including a multi-ingredient one — is
automatable for free.

**Extractors deposit into their own output buffer, not your inventory.** This
is the largest felt change: fragments stop appearing in your pocket while you
are away, and you come home and harvest with `C`, which empties every
structure orthogonally touching you. Structures block movement, so you always
stand beside one rather than on it — standing in the crook of an L empties
three buildings, a sprawled-out line costs you trips. It is also the only
thing that makes clogging real; a node paying straight into the player is an
infinite source and nothing upstream of it can ever back up.

Three stall states, each said once on the way in rather than every tick:
*starved* (input short), *clogged* (output full) and *idle* (no program). `B`
shows every buffer and every stall.

Every machine needs an assigned program, assemblers included, so **roster
capacity is what buys chain length, not fragments**. The five-machine line
needs five programs against a starting cap of three, which makes the Data
Cache the population building.

New content: Bytecode Block, Charge Coil and Patch Routine — the game's first
multi-ingredient recipe, and the first shipped item to arm a pre-battle buff
— produced by a new Refinery, Winding Node and Assembly Bay. The Assembly Bay
costs Bytecode Blocks to build, so the two-machine line a starting roster can
afford is what pays for the third stage.

Removed: `passive_process` (zero shipped users, superseded entirely by
`assembles`) and the node deposit pool — `ResourceNode::amount`/`capacity`,
`WorkDef::capacity` and `StructureSave::resource_amount`. A node is a tap
rather than a reserve now; the output buffer is what paces it.

New `dev-saves/chains.ron` stands the whole chain staffed and running, with
the player parked beside its Assembly Bay.

### The log pane filters by base or field

`F` on the map cycles the log pane through All → Field → Base → All. The base
talks constantly — cronjob payouts, failed extractions, construction, raid
damage — and in the six rows the pane has room for, that steadily pushed
combat and Stack lines off the top before they could be read.

Every log line now carries a `MessageSource` alongside its `MessageKind`. The
two are separate axes on purpose: kind is read by the colour table, by
`retain_outcomes_since_battle`'s prune, and by `condense`'s notion of line
identity, and a raid alert has to stay `MessageKind::Raid` for all three while
still being base news. `Field` is the default, so only the ~24 base-side log
calls moved. Power reserves stay field — a need follows you into the Stack.

The pane header names the active filter and counts what it is holding back, so
a raid landing while you are watching the other channel still announces
itself. The history screen (`L`) is deliberately unfiltered: it is the
complete record, and app-core's row count for it stays the one the renderer
draws.

No save-format bump — the message log was never persisted.

### Nests fight back, and destroying one pays

Bumps `SAVE_FORMAT_VERSION` 18 → 19 — existing saves need a new game.
`CreatureSave` gains `nest_position` and `pursuing`, and a new `NestSave`
records each nest's position, Durability and pending respawns.

- **Attacking a nest now provokes every one of its guardians.** They abandon
  their tether and path toward you around obstacles — a bounded cost field
  built once per tick from the player's tile (the `pathfinding` crate's first
  use in this engine) — then fold in whatever else is standing nearby the
  moment one makes contact, into an ordinary battle. A besieged nest keeps
  this going: a guardian that respawns while its nest already has pursuers
  joins the chase the moment it appears, so grinding one down without
  finishing it off no longer buys a permanent lull on its own — reaching
  your own base or successfully jacking out of the fight still does (see
  below on both).
- **A pursuer gives up 15 tiles (Chebyshev) from its own nest, more than 20
  from you, or once it simply has no route to you at all** — whichever it
  hits first — and walks itself home. **Reaching your base ends the chase
  outright, for the whole swarm at once**: no guardian will set foot on the
  platform, and standing inside it leaves every pursuer in the zone with no
  route to you, not just whichever one was closest, so going home disbands
  the chase rather than merely holding it at the door. New tuning:
  `NEST_AGGRO_LEASH_RADIUS`, `NEST_PURSUIT_STEPS_PER_TICK` (1, exactly player
  speed — outrunnable in a straight line, never shakeable),
  `NEST_PATH_SEARCH_MARGIN`.
- **Destroying a nest now pays a cache**: a multiple of its species'
  work-resource drop, Portal Fragments scaled by how far below your current
  zone it sits, and three rolls of its equipment table. No XP — the
  guardians already paid that on the way down, and a nest chipped but not
  finished pays nothing yet.
- **Persisted across save/load**: a nest's position, Durability and pending
  respawns; each guardian's tether and whether it's currently pursuing.
  Without this a reload would launder a half-destroyed nest and its cache,
  and quietly clear whatever swarm was chasing you.
- **Fixed: a guardian dragged outside its tether radius could freeze
  permanently.** `wander_ai_system` refused any step that left
  `NEST_TETHER_RADIUS`; once something could push a guardian past that
  radius — the chase above — every neighbouring tile still counted as
  "leaving" it, so the guardian had no legal move at all. It now refuses a
  step only when doing so both leaves the radius and fails to close the
  distance, so a displaced guardian makes its way home instead of standing
  frozen.
- **Fixed: jacking out of a fight with a nest's guardians could put you
  right back into it before you got a single input.** `battle_flee`'s
  teardown moved no one and left every pursuer still marked, so the very
  next tick's pursuit step saw the same pack still adjacent and started a
  new battle on the spot — under permadeath, every attempt to leave cost the
  same XP setback for nothing. A step-away fix was tried and rejected: with
  `NEST_PURSUIT_STEPS_PER_TICK` exactly matching a one-tile move, the
  guardian's own ordinary step closed the gap straight back to adjacency in
  the same tick, on any open ground. A successful jack-out now clears
  `Pursuing` from every guardian that was actually in that fight instead —
  it shakes the pack rather than trying to outrun it. They aren't gone:
  `NestGuardian` survives, so they resume ordinary tethered wandering and
  the nest re-provokes them the next time you hit it. A guardian that
  wasn't part of that fight keeps chasing regardless, and a failed jack-out
  attempt shakes nobody.

### The zone group cap is a line, not a curve

- **`ZONE_GROUP_GROWTH` (geometric, x3) becomes `ZONE_GROUP_STEP` (additive,
  +9)**, so `zone_group_cap` runs 1, 10, 19, 28, 37 instead of 1, 3, 9, 27,
  81. Geometric growth from a base of 1 spent the whole playable range in
  single digits — zone 2 capped every group at 3, holding surface packs and
  Stack packs alike to three programs against a party of five whatever the
  distance and depth curves had earned — and then ran away past zone 4 into
  caps no encounter is designed around. Zone 1 stays solo, which
  `in_opening_ring` and the fresh-player species checks depend on.
- The trade is deliberate and it cuts both ways: the early zones gained
  range, zones 4+ lost their runaway (zone 5's cap falls 81 -> 37), and the
  hard `MAX_GROUP_SIZE` ceiling is now first reached at zone 12 rather than
  zone 6. This is the ceiling, not the roll — `spawn_pack` still draws
  uniformly in `1..=ceiling`, so a zone produces a wider *range* of fights
  rather than uniformly bigger ones.

### Depth is the Stack's distance

Engine-only, no save-format bump. Nothing new is persisted — depth already
lived in `Locale::Stack`, and both group curves are derived per fight.

- **Descending now widens the fight.** The surface escalates an encounter by
  distance from the danger origin; the Stack could not, because the party's
  `Position` is pinned to the entrance tile they walked in through, so
  `max_group_size` measured the base's own doorstep however far down they
  had gone. Every frame at every depth fielded one program in one group
  unless Trace had reached Hunted. `Game::danger_steps` is now the single
  input to both curves, taking frames descended underground
  (`GROUP_SIZE_STEP_FRAMES`, one frame per step) and tile distance on the
  surface.
- **A Stack ambush draws one species pick per group the depth allows**
  (`Game::stack_encounter_pack`). `group_pack` partitions by species, so a
  single draw was a single group however many the count permitted — raising
  the count alone would have been the same no-op that scaling only the spawn
  once was for `TRACE_GROUP_MULT`.
- **`SpawnEscalation` replaces `spawn_pack`'s loose `depth_mult`/`group_mult`
  pair** and carries depth alongside them. All three are properties of where
  the party is, and none may be read inside the spawn: ambient surface spawns
  and nest respawns keep rolling while the party is underground. Bundling
  gives that rule one home, and `SpawnEscalation::surface()` names the
  no-escalation case.
- Depth 1 is unchanged — it is the Stack's opening ring, and `in_opening_ring`
  and the fresh-player species checks still see a zone-1 surface fight as one
  program. `zone_group_cap` still binds: zone 2 caps every group at 3 whatever
  the depth.

### Bounded income: rest costs a consumable, scan is deleted

Engine-only, no save-format bump. The outlet is an ordinary inventory item,
`RestDef::cost` is asset data, and `Perk::KeenScavenger` keeps its save
index — nothing new is persisted.

- **Rest now spends a Power Outlet**, a new craftable item
  (`assets/items/outlet.ron`, 5 Core Fragments), and refuses with no outlet
  held. `Game::rest` checks and spends it after every existing gate
  (game-over, active battle, `require_surface`, `nearby_rest_structure`) and
  before the 40-tick loop, so a refused rest never consumes one and a rest
  that starts has already bought its ticks. The price lives on the
  structure that grants rest — `home.ron`'s `enables_rest` gained a `cost`
  field on `RestDef`, `#[serde(default)]` so an older `.ron` still parses,
  as a free rest, unchanged from before the field existed.
- **The scan action (`g` on the surface) is gone**, along with `Game::forage`,
  `forage_chance`, and the three `FORAGE_CHANCE_*` constants. `g` keeps its
  Stack meaning — the frame map — untouched; on the surface it is now a
  no-op.
- **`Perk::KeenScavenger` now boosts the mining roll instead of scan**,
  since its entire effect was a scan bonus and deleting the variant would
  have shifted every later save index. The flavour survives, the index
  doesn't move, and the perk now backs the thing this change wants players
  investing in: `mining_success_chance`, 50% at a level-1 node.
- **Two Power Outlets go into new-game starting inventory**, beside the 3
  ICE Breakers, 3 Power Cells and 5 Core Fragments, to cover getting
  established before the base earns anything of its own.
- **Portal Fragments are no longer sold at the iso Market.** Breaching is now
  earned by fighting — a 35% drop from any defeated program, a guaranteed 3–6
  from a boss, and a chance from a Stack cache. That listing was the only
  route from base production straight to progression, which is exactly what
  made settling in one zone and selling salvage a substitute for engaging
  with it. Credits surviving a breach no longer buys a way past content
  either, so a hole `enter_next_zone` previously priced rather than closed is
  now closed at the source.
- **`balance_sim::ticks_to_afford_portal` and its three tests are deleted.**
  It measured how long a worked node took to fund a breach *through that buy
  price*. With no buy price the question no longer exists, and a gate
  measuring a route the game doesn't have is worse than no gate. The engine
  suite drops from 1156 to 1153.
- **The Terminal is gone.** It converted a Core Fragment into a Power Cell
  every tick while you stood within 2 tiles, for a 3-Fragment build cost —
  the cheapest thing you could deploy, and a second free-Power source beside
  the Recharger Node. Power Grid research now unlocks the Power Conduit
  alone. It was the only shipped structure using `passive_process`, so that
  schema field and `passive_process_system` now serve mods only; both stay
  documented in `assets/structures/README.md`.
- **Deploying a Home now obliterates any Stack link under its platform**,
  the same way it already obliterates the hostiles and nests standing there.
  This is not a rare collision: `STACK_NEAREST_LINK_TILES` puts a zone's
  first link 5–8 tiles from where the player arrives, against a slab of
  `MAX_BUILD_DISTANCE_FROM_HOME` 7, so a Home near the arrival point swallowed
  one on a large fraction of seeds — leaving an entrance stranded inside the
  base, on floor nothing can spawn on. Breaching was never affected: there the
  platform is stamped before links are placed, and placement already skips
  platform tiles.
- **Core Fragments now drop from species a fresh player can actually beat.**
  Ranked by stat total, all three Power Cell droppers (`glitch`,
  `sub_process`, `scrapper`) sat in the gentle band the opening ring fields,
  while three of the four Fragment droppers (`worm`, `virus`, `construct`)
  sat behind fights a new player loses — so `drone` was effectively the only
  early Fragment source, and ten of seventeen species drop nothing at all.
  That was backwards: Fragments are what you build with, and Power Cells
  restore Power, which a Recharger Node gives away free. `sub_process` now
  drops Core Fragments instead of Power Cells, and `sprite` — which dropped
  nothing — now drops them too. `glitch` and `scrapper` keep Power Cells, so
  the early game still has a source before a Recharger is affordable.
- **A defeated program now drops 2–4 of its work resource, up from 1–2.**
  With scan gone, kills are the only source of Core Fragments outside a
  built base, and the first Mining Node — 12 fragments on top of Home's 5 —
  was about eight kills away while a single rest cost five. Only seven
  species carry a work resource (four Core Fragments, three Power Cells), so
  Power Cell drops double too; splitting the range per resource would be new
  asset data for a seven-species problem.
- Why: nothing limited how much a player could earn. Scanning was 1 tick for
  ~0.6 Core Fragments in a rich biome, returned roughly 50x the Power it
  burned, and inside a Recharger Node's radius Power *rose* while scanning —
  so Power was never a real cost, and research, gear and structures all
  reduced to keyboard time on one tile. The fix moves the limiter onto the
  actions themselves: **the base is the farm**. Rest is now an investment
  against 40 ticks of base output rather than a free action — at zone 1,
  tier 1, a Mining Node worked for a full rest cycle yields ~2 Core
  Fragments against a 5-fragment rest (a net loss), and break-even is
  roughly three worked nodes, which under the one-cronjob-per-structure rule
  means three structures and three programs, not three stacked on one node.
- This is unplayed balance — 5 fragments per rest and 2 starting outlets are
  arithmetic, not evidence, the same footing the Trace bands were on before
  playing moved them. The starting outlet count is the softener most likely
  to need retuning.

### One program per structure, per job

Engine-only, no save-format bump. A save that already has three programs on
one node keeps them until one is reassigned — the rule is enforced where a
job is handed out, not retroactively.

- **A structure runs one cronjob and holds one guard**, counted separately,
  so a worked Mining Node can still be defended. Assigning a second program
  to the same job stands the first one down and says so in the log.
- **Working a structure yourself holds the cronjob slot too.** `work_structure`
  puts the same `Task` on the player that a cronjob worker carries, so a
  player mining a node while a program worked it was the same double-payout
  in a different shape. Putting a cronjob on the node you're standing at now
  breaks your own concentration.
- Why this mattered: `task_progress_system` iterates *per worker* against the
  target node, and an emptied node refills to capacity, so three programs on
  one Mining Node was three times the income from one structure forever.
- Occupancy is read off the `Task` components rather than cached on the
  structure. `Task` is already the only record of who works what, and eight
  sites remove one — raid damage, demolition, sale, breach, party recall,
  fusion, rest and zone change — so a cached field would have had to be kept
  in step with all eight.

### Bosses are a wall, not a wait

Both boss `.ron` files, plus the comment on Decompile's boss refusal. No
save-format bump — a boss already in a save keeps the stats it spawned with.

- **Overseer drops from 1450 HP to 200, Wintermute from 1600 to 220.** They
  were 12–13x the toughest ordinary program's HP while their attack was only
  ~3x it and their defense ~1.5x, so a boss could not really kill you faster
  than a Sentinel could — you just had to hit it forty more times. That is a
  long fight, not a hard one.
- **A boss's real size is the multiplier stack, not its base.** Every wild
  spawn is scaled by zone x distance x Stack depth, so on frame 3 of a
  zone-3 stack a way-out tile multiplies base stats by roughly 11x. At 1450
  that was a **6,000 HP** program on screen; at 200 it is about 2,400,
  against 1,300 for a Sentinel standing next to it. The ratio is what was
  retuned; the absolute number follows the tile.
- Projected against `balance_sim`'s round loop, a *single* Overseer at 1450
  needed a level-15 party at zone 1, where a full zone-cap *group* of the
  toughest ordinary species needs level 1; at zone 5 neither boss was
  beatable at level 200. At 200 HP that zone-1 fight lands at **level 5 and
  9 rounds**, Wintermute at level 9 and 13.
- Bosses still cannot spawn in the opening ring — `beatable_by_a_fresh_player`
  remains false for both, asserted by
  `the_shipped_roster_has_species_on_both_sides_of_the_opening_ring`.
- Decompile still refuses a boss, but its comment no longer argues from
  `base_hp`, which has now moved twice. The durable reason is
  `growth_multiplier`: 2.0 on both bosses against 1.5 on every ordinary
  species, so a captured boss outgrows the roster it joins whatever it
  costs to bring down.
- `balance_sim` otherwise does not gate this and did not move:
  `toughest_ordinary_species` filters bosses out by design, so its curves are
  blind to boss stats. The numbers above came from a throwaway probe, not a
  committed test.

### The frame map, in the corner of the corridor

No save-format change: the renderer draws a view the engine already had.

- **The map of the frame you are in is always on screen**, in the top-left
  of the first-person corridor. Reading it no longer means leaving the view
  you are reading it *about*.
- **`g` still opens the full screen**, and the two answer different
  questions: the corner map is which way you are facing and where you have
  been, the full one is where the wing you have not walked is — three times
  the cell size, plus the legend that names every glyph. The corner map
  carries no text of its own for that reason.
- Both are drawn by one function, so a cell can never mean one thing on the
  small map and another on the large one.
- **`+` and `-` zoom the corner map**, from the whole frame down to a
  seven-cell window around the party — the same keys that size the surface
  map's tiles, since the two are never both on screen. They are separate
  settings, so a dive spent reading the maze up close does not resize the
  zone map you climb back out to. Against a wall the window slides to stay
  inside the frame instead of showing you the outside of it.
- Not playtested. How much of the corridor an always-on inset should cover
  is a matter of taste, and the size was picked by what stays legible at
  1280x720 rather than by playing with it.

### The orphaned process: a second way into the roster

**Save-format bump** — `SAVE_FORMAT_VERSION` is now 18 and existing saves
will not load. `dev-saves/` templates are unaffected; they are field-named
RON and keep parsing.

- **Orphaned processes** (`o`) — a program still running in a dead end with
  nothing left to serve, drawn from the same biome as everything else under
  that link. Press `o` while standing on one and it joins your roster for an
  ICE Breaker: no capture roll to win, no fight to survive, and no Trace,
  since taking one is a rescue rather than something you broke. Once per
  orphan; the dead end reads as empty afterwards.
- **The roster is the limit, not the catalysts.** A six-frame stack offers
  four or five programs against a base capacity of three, so a thorough
  descent gets refused near the bottom and the last orphans are scenery
  until you have built the space for them. Whether that reads as pressure
  toward capacity-granting structures or as a dead mechanic is the open
  question — the balance simulator models no roster and cannot answer it.
- **One per frame, in a dead end the caches did not take.** The placement
  pass runs after the caches and wants the same site type, so a frame needs
  four plain dead ends to field one and about a quarter of frames haven't
  got that many. "One per frame" is really three frames in four.
- **Which program it is survives a save and load.** The species is drawn
  from an RNG seeded off the frame itself rather than the run's shared
  stream, so you can see what is down there before deciding to pay for it.
  What it turns out to be *worth* is still rolled at adoption, like every
  other spawn in the game.
- Not playtested. The numbers above are arithmetic against a measured frame.

### Cell kinds: the Stack gets more than one kind of floor

**Save-format bump** — this shipped at `SAVE_FORMAT_VERSION` 17; the
orphaned process above took it to 18 before either was released.
`dev-saves/` templates are unaffected; they are field-named RON and keep
parsing.

- **Breakpoint** (`*`) — an exposed debug port on a junction. Walking onto
  one maps the entire frame at a stroke, walls included, and costs the
  single largest Trace spike in the game: two and a half caches' worth. One
  per frame, and each works once. The free map is meant to be a decision
  about the rest of the frame rather than a reflex.
- **Fault** (`v`) — a hole in the floor. Step on it and you drop a frame,
  landing in the far half of the one below rather than on its way up, so a
  fall is a quick way down and a long way back. Never generated on a bottom
  frame, which has nothing below it. Raises no Trace — falling is clumsy,
  not loud.
- **Corruption** — rotten substrate that costs Integrity every step through
  it, drawn as coloured ground rather than a glyph because it comes in
  stretches of three rather than single cells. That is the whole point: a
  lone bad cell is a toll you pay without thinking, where a stretch is
  something you can see and route around. The cost is a percentage of your
  maximum Integrity, so it stays meaningful at level 1 and at level 30 —
  Stack depth is uncorrelated with your level, and any flat number would be
  lethal at one end and free at the other.
- The frame map and the first-person corridor teach one vocabulary: the same
  glyphs and colours in both, with the map's legend extended to match.

### Trace: the Stack notices what you take

**Save-format bump** — this shipped at `SAVE_FORMAT_VERSION` 16; the cell
kinds above took it to 17 before either was released. `dev-saves/`
templates are unaffected; they are field-named RON and keep parsing.

- **A greed meter, not a timer.** Trace rises when you crack a cache, burn a
  seal or kill something underground. Walking is free — a meter driven by
  time or distance would tax exploration and punish the careful player,
  which is backwards for a maze whose per-frame map memory exists to reward
  learning it.
- **Four bands — Quiet, Noticed, Traced, Hunted** — shown in the Stack
  heading, with each crossing announced in the log. The band is all you see;
  there is no number, because it is there to tell you whether to press on
  rather than to be played up to a threshold.
- **Each band draws ambushes more often, scales what arrives, and sends more
  of it at once.** The stat scaling reaches the lair guardian too, so a
  party that stripped a stack on the way down meets its boss at the band
  they earned.
- **Trace clears on surfacing**, by either the link or a symlink. Not a free
  reset: caches, seals and lairs are one-shot per stack, so a trip up to
  shed a band means coming back to less worth taking.
- Every number is arithmetic against a measured frame and has not yet been
  playtested. The per-source ratios are grounded; where the band lines fall
  is the part that still needs playing.

### Trading in one keypress

- **`S` sells one, `B` buys one, straight off the highlighted row.** No
  quantity page, and the list stays put. A trade visit is normally a run of
  trades, so the four-screen round trip was being paid per item. `S` also
  works from the inventory screen when there is a trader in range; with two
  in range it asks which, since their buyback shelves are separate.
- **A program row is deliberately excluded.** `S` there opens the same
  confirmation Enter does — selling a levelled program is permanent, and a
  single keypress is exactly the mis-hit that confirmation exists to catch.
- **Inventory and trade lists are grouped by category** — consumables,
  weapons, armour, modules, routines, salvage, currency — with each row
  tagged `USE`/`WEP`/`ARM`/`MOD`/`RTN`/`MAT`/`CUR`. The category is derived
  from the fields an item already declares, so a modded item is grouped
  without its author adding anything.
- **Uppercase letters no longer pick menu rows.** They are reserved for
  screen actions now, matching what battle already did with `A` and `D`.
  Lowercase row shortcuts are unchanged.

### The dungeon is the Stack

- **Renamed the underground layer, top to bottom: dungeon → the Stack,
  level → frame, shaft → stack, entrance/breach → link.** Nothing about how
  it plays changed — this is a vocabulary pass, not a mechanics one. "Link"
  covers both scales deliberately: the hole on the zone map and the way
  between frames are the same kind of thing at different scales, and one
  word for both says so. The help screen, the deep-scan log line, the
  descend/climb/bottom-out messages, the map heading, the first-person
  prompts, the README and manual all speak the new vocabulary now. That
  makes three player-visible senses of "link," not two: `use_symlink`
  already used the word for the action that leaves the Stack ("The symlink
  hauls you up out of the stack and drops you at {name}"). In Unix register
  a symlink is a symbolic link, so the overlap reads as coherent rather
  than accidental — noted here so it doesn't look overlooked, not because
  it's being reopened.
- **"Breach" survives, narrowed to zone travel.** That word already meant
  three different things — the old dungeon entrance, stepping through a
  Zone Portal, and "ICE breached!" in combat — and renaming sense one is
  what frees the word to mean just the other two. "You breach the portal
  and materialize in a level {n} sector" and "ICE breached!" are unchanged.
- **Item and structure `.ron` ids and field names are untouched.**
  `access_shard` is still `access_shard` — only its human-readable
  `description` changed, along with `assets/items/README.md`'s
  `cache_drop` schema note. Ids are save-and-recipe keys mods depend on;
  they were never part of what this pass renamed.

### The party line is yours to arrange

- **`<` and `>` on the companions screen move a member along the battle
  line.** Slot order already decided who drew enemy fire — front slots are
  weighted heavier in the target roll — but the only way to change it was to
  stand a program down and re-add it, which appends to the back. `Game::
  move_party_member` swaps two adjacent slots and nothing else; no tick, so
  shuffling the roster stays free the way adding and standing down already
  were.
- **Refused during a battle.** `BattleState::planned` indexes the party
  positionally, so a mid-round swap would hand two slots each other's
  planned action.
- **The roster now leads with the party in slot order,** with each member's
  slot on its row. The sort lives in `Game::owned_pets` rather than in a
  renderer: app-core maps number keys by index while gui draws the rows, and
  sorting in one but not the other picks a different program than the one
  the player pressed.

### You can work a node yourself

- **`W` works a structure the way a program does.** The player takes the
  same `Task` a cronjob worker carries, advanced by the same tick and paid
  through the same `resolve_gather_cycle`, so the yield off a given node is
  identical whoever is standing at it. Extracted rather than reimplemented
  — a second payout formula for the player is precisely the drift this
  codebase has been bitten by before.
- **Moving breaks off the job.** There is no separate working mode: the job
  runs while the world ticks, and any attempt to move drops it — including
  one that walks into a fight or bounces off a wall, since either way you
  stopped working to do it.
- **No XP, unlike a cronjob.** A posted program levels from its work; the
  player doesn't, or a node becomes a risk-free XP faucet. The job is also
  not saved, so loading puts you beside the node rather than mid-cycle at
  it — that costs at most one cycle and no save-format bump.

### Fatigue recovers on its own

- **Fatigue now regenerates every tick instead of draining.** It was never a
  survival need — nothing starves you for running out of it, and its one
  job is paying for battle routines (`AbilityDef::fatigue_cost`). But the
  only full restore is resting, which refuses anywhere outside your base,
  so a long trip underground drained the pool with no way to refill it and
  quietly took your abilities away. Hunger is now the only clock that runs
  down, and the only one that can kill you.
- **Regen keeps running during a battle.** A routine's cost is a throttle
  on how often you can afford it in a fight, not a fixed budget for the
  whole fight. At `FATIGUE_REGEN_PER_TICK` a 5.0-cost routine is worth
  about 62 ticks of walking — arithmetic, not playtested.

### A save can be edited from the command line

- **`savetool` reads a save out as RON, takes it back, and can warp it
  forward.** Saves are bincode, which has no field names on disk and can't
  be hand-edited, so testing anything deep in a run meant playing there.
  `dump` renders one as text, `pack` re-encodes an edited one at the
  current save version, and `warp <n>` advances a save to a later zone by
  running the real breach — the base still travels and spawns still scale,
  rather than a zone number being overwritten in place.
- **`pack` always stamps the current save version**, so dumping before a
  format bump and packing after is the one way to carry a save across a
  version change. There is still no automatic migration, by design.
- **`dev-saves/` holds named worlds you can generate into.** A state you
  had to play up to becomes one you can regenerate in a second:
  `savetool capture <save.bin> <name>` records a save as a checked-in RON
  fixture, and `cargo run -- --template <name>` regenerates it and boots
  straight in. Shipped with `extraction` — nine tamed programs, four of
  them carrying 4–6 routines across every effect category, and a standing
  Compiler — because testing routine extraction otherwise starts with an
  hour of taming.
- **A template is copied, not played in place.** `--template` writes
  `saves/dev_<name>.bin` and opens that, overwriting it every run. The game
  autosaves, so opening the fixture directly would rewrite it, and it would
  decay into a record of the last session instead of a known world.
- **Templates are RON so they can outlive a save-format bump**, and
  `every_checked_in_template_still_loads` says whether they have. It
  generates each one through the real `Game::load` and compares the tamed
  count across it — a species id that has left `assets/species/` is skipped
  on load rather than rejected, so a gutted template would otherwise still
  open and simply be missing the programs it exists to provide.

### Field routines: buffs cast outside battle that keep running into one

- **An ability can now be field-only.** `AbilityEffect::FieldBuff` is the
  marker — there's no separate flag — and an ability carrying it never
  appears in the in-battle Special picker and is never used by a wild
  carrier. Instead it arms a buff from the map: press `a` to open the cast
  screen, spend Power and a turn, and the buff starts ticking. There is no
  cooldown. The buff keeps running through whatever battle follows and
  survives a save — unlike the buff a companion's Special arms mid-fight,
  which is still wiped the instant that battle ends. A panel under the map,
  and another in the battle roster, list every buff currently running,
  however it was cast.
- **Ten field routines ship, one per `FieldBuffKind`.** Four land on one
  ally you choose — Repair Loop (heal over time), Overclock (flat Attack),
  Hardened Shell (flat Defense), Ablative Layer (percent damage reduction)
  — and six always land on the player regardless of who casts them, since
  they're pressure or economy knobs the whole run feels rather than one
  combatant's stats: Coolant Flush (Fatigue over time), Trickle Charge
  (Power over time), Deep Scan (capture odds), Trace Analysis (XP), Ghost
  Protocol (encounter chance), Salvage Routine (drop chance). Scaling cuts
  across that scope split, not along it: five kinds — Repair Loop, Overclock,
  Hardened Shell, Coolant Flush and Trickle Charge — scale with the caster's
  level and affinity like any other ability; the other five — Ablative
  Layer and the four remaining percentage kinds — deliver their authored
  number unscaled, so a routine's percent bonus can't be pushed past its own
  ceiling by a high-level, high-affinity caster. See
  `assets/abilities/README.md`'s `FieldBuff` section for the full scope and
  targeting rules.
- **Three research nodes hand the ten over**, branching off `self_exec`
  where the other player routines already hang: Field Operations (16) for
  Repair Loop, Coolant Flush and Trickle Charge, then Adaptive Plating (32)
  for Hardened Shell, Overclock and Ablative Layer, and Deep Analysis (46)
  for Deep Scan, Trace Analysis, Ghost Protocol and Salvage Routine. As with
  every other researched routine, unlocking a node drops the item into cargo
  and installing it is a separate act. No species grants one and none has a
  wild-carrier weight, so a companion's own kit is unchanged.
  `every_shipped_field_routine_can_actually_be_obtained` pins this: a
  routine in no research node and no species kit fails the suite, because an
  ability file existing is not the same as a player ever seeing it.
- **An item's `prebattle_buff` now arms the same mechanism**, fixing two
  bugs it had carried since it shipped: the buff it armed was destroyed at
  the end of every battle despite the name, and it never survived a save.
  Both are now true of it, because it's the same `FieldBuff` a routine arms.
  See `assets/items/README.md` — the field now carries a `kind` and counts
  in ticks, not rounds.
- **The numbers are unplayed.** Every duration, magnitude and Power cost
  across the ten shipped routines is an arithmetic-plausible starting
  guess, not a tuned value — most visibly `trickle_charge`, which returns
  more Power over its run than it costs to cast, a choice made knowingly
  rather than caught late. None of this has been through an actual
  playthrough.
- **Save format bumped to v15.** Both `PlayerSave` and `CreatureSave` gained
  a `field_buffs` list. **A save written under v14 will not load** —
  bincode has no field-level migration path, so it's rejected up front with
  a clear message, the same policy as every prior bump.

### Raid damage is something you repair, not something you wait out

- **New structure: the Patch Node.** It repairs every deployed structure in
  the base — itself included — by 1 Durability per upgrade tier, every 20
  ticks. Several of them stack, the way Shields do, and it upgrades to Mk5
  for 6 per interval. Unlocked by **Fortification**, the same research node
  that unlocks the Shield, which now reads as the two halves of one answer:
  a Shield stops raid damage landing, a Patch Node undoes what lands anyway.
- **Raid damage is now permanent until you repair it.** Structures used to
  regenerate 4 Durability per interval for free — exactly `RAID_DAMAGE`, so
  one interval fully undid one raid and a base won the attrition race by
  doing nothing at all. Raids were an inconvenience with a timer on them.
  Free regen is gone entirely (`STRUCTURE_REGEN_AMOUNT` deleted rather than
  lowered), so every point of repair now comes from a deployed Patch Node.
  This is gentler than it sounds — an undefended structure still takes ~7
  raids to lose, and workers and guards already cut incoming damage — but
  the slope finally runs the other way if you ignore it.
- **Fixed: nests were quietly regenerating.** A nest carries Durability like
  a structure does, and the regen pass healed every Durability holder
  indiscriminately — so chipping a nest down with bump-attacks was racing its
  own healing, and walking away from a half-destroyed one gave the progress
  back. The manual already said a nest's Durability "is only ever spent by
  you"; the code just didn't agree. The pass is now `With<Structure>`, so
  neither the baseline trickle nor a Patch Node's repair reaches a nest.
- `StructureDef` gains an optional `repair: Some((per_tier: N))` field, so a
  mod can declare a repairer of its own without touching Rust — documented
  in `assets/structures/README.md`. Existing structure files parse unchanged.

### Fatigue is visible where it is spent

- **The battle roster has a `FATIGUE` column.** Fatigue prices every routine
  and refuses the ones you can't pay for, and the intrusion screen was the one
  place it wasn't shown — so "not enough Fatigue" quoted a figure nothing on
  screen named. It sits on your row alone: Fatigue is one pool, yours, spent
  whichever member runs the routine, so a companion's cell is a `—` rather
  than a fourth copy of your number.
- **The ability picker prices each routine** (`8 FTG`) next to what it does,
  and lists an unavailable one greyed with the engine's own reason — a
  cooldown, no catalyst, a full roster, or Fatigue you don't have. It
  previously showed descriptions alone, with neither the name nor the cost nor
  the reason.

### Two read-only screens

- **`L` shows the message log in full.** The pane under the map fits a few
  lines, so anything that scrolled past was gone. The new screen is the last
  `MESSAGE_LOG_CAP` (100) lines, oldest-first like the pane, each in its
  `MessageKind` colour, scrolled with Up/Down and opening on the newest line —
  the one that just left the pane. Its footer states its own three limits
  instead of leaving them to be discovered: the 100-line bound, the folding
  below, and that `retain_outcomes_since_battle` keeps a finished intrusion's
  results rather than its narration.
- **Repeated lines fold into one row with a dim `×N`.** Automation is
  repetitive by nature — a base with three cronjobs pushes a yield line per
  producer per cycle — and the screen was mostly forty copies of the same
  extraction with the raid you were looking for buried among them. An
  identical line now folds into a recent one even with a couple of lines
  between it, which is what catches several cronjobs interleaving their
  yields; the same line again much later stays its own row, so two starvation
  warnings still read as two events. The stored log is untouched, folding is
  the screen's alone — the pane under the map and a battle's narration still
  show every line.
- **`B` shows every structure and what is assigned to it.** Tier, tile,
  distance and raid Durability per structure, and *every* program posted to
  each one — a cronjob worker and a guard can share a structure, which the
  map's `structure_worker` label could never show, since it comes from a map
  keyed by the task's target and collapses the two. A workable structure with
  nobody on it is flagged idle. Backed by a new `Game::structure_report`,
  which is deliberately zone-wide where `view_entities` takes a radius: the
  base sits near its Home but the player wanders, and a roster that thinned
  out as they walked away would be worse than none.
- Neither screen takes an action or advances a tick.

### Balance

- **Routines now scale with the level of whoever runs them.** Ability
  `Damage` and `Drain` never did: `battle::compute_damage` is
  `power + ATK - DEF`, and the caster's ATK was held to carry the whole
  progression. It cannot — `ATK_PER_LEVEL` is 1 against `HP_PER_LEVEL`'s 12,
  and zone depth doubles Integrity again on top — so an authored power fell
  further behind its target every level. A level-10 player with the Damage
  affinity perk five deep, spending the heaviest single-target routine in
  the game against a 400-Integrity program, hit for **35**. The same cast
  now lands **147**.
- **There are two level curves now, not one, split by what the magnitude is
  measured in.** `Damage`, `Drain`, `Heal` and `Debuff` are HP figures and
  scale on the new, steeper `ABILITY_HP_SCALE_PER_LEVEL`; `Buff` and
  `FieldBuff` are stat points or percentages and stay on
  `ABILITY_STAT_SCALE_PER_LEVEL`, unchanged at its old rate. Putting a `Buff`
  on the HP curve would have turned a +3 attack routine into a tripling — the
  single-curve version of this change was not viable, which is why the split
  exists rather than one raised number.
- Heals move with damage: a Checksum Repair on a perked level-10 caster goes
  from 78 to 156, a Cold Boot from 156 to 313. Bleed, whose power is HP per
  round, moves with them for the same reason.
- Hostile carriers get the same curve, read off the zone rather than a level
  they don't have (`Game::ability_user_level`), so a wild routine at zone 4
  goes from 16 to 42. That is the cost of the change and it is deliberate:
  the player's own level outruns the zone number they are standing in.
- No authored `.ron` power changed. `balance_sim` models no abilities at all,
  so its curves do not move and never covered any of this — the new
  `a_perked_level_ten_kernel_panic_lands_in_the_intended_band` is the only
  regression gate on ability magnitudes.

### Fixed

- **A heal announced what it rolled, not what it landed.** "Medic patches
  you for 23 HP." was the scaled figure the ability produced, printed
  before the target's ceiling was applied — so a patch on someone three
  points down claimed twenty-three, and a patch on someone at full health
  claimed all of it while doing nothing. Both the `Heal` line and Drain's
  "restoring" figure now report HP actually restored, which is `0` when
  there was none to give back. `Game::restore_hp` is the single clamp both
  read from, the way `apply_damage` is for damage.
- **The party roster no longer prints ATK and DEF.** The row had grown long
  enough to crowd the quality, fusion and activity tags that only appear
  situationally; Integrity and Power carry the at-a-glance comparison. The
  fuse pickers still show the full stat line, since that is the screen where
  the numbers decide something.
- **A trade dropped you back on the map.** Every completed transaction —
  item sale, purchase, buyback, program sale — closed the trader's screen,
  so clearing a full pack a stack at a time meant walking `t` → trader →
  row → quantity again for every line. A finished trade now lands back on
  the trader's list, and only a death mid-visit takes the screen away. The
  list also shows your Credit balance now: the payout is announced in the
  log pane, which the trade popup covers.
- **The sell list offered the money and hid the salvage.** The screen drew
  your inventory minus Credits (a trader won't buy Credits for Credits),
  but the key handler behind it filtered out Core Fragments instead — so
  the two lists disagreed by a row whenever you carried both, and picking
  the top sell row offered the trader your Credits and got refused. Both
  sides now filter on the trade currency.
- **A battle refusal never faded.** Every message clears itself after four
  seconds, because it is drawn over the action bar it explains — but any key
  press restarted that window, on the theory that a message left standing
  deserved its full time. In a battle, where planning a round is several key
  presses and nothing clears a refusal on success the way the map menus do,
  that meant "Can't do that — 3 more rounds." stayed over the action bar for
  as long as the player kept playing. A message now keeps the window it was
  raised with, and only a *new* one gets a fresh window.
- **A held letter or digit drove the screen its own press opened.** Extracting
  a routine looked broken — nothing salvaged, the program still alive, and no
  refusal saying why. Letters and digits reach the game from
  `KeyboardInput::text`, which carries the OS auto-repeat (that is what makes
  `hjkl` walking work) and which `KeyRepeat::block_held` cannot reach. So a
  `2` held on the extraction picker chose the routine, opened the
  confirmation, and the repeat landed there — where every key but Enter backs
  out. The new `TextGate` applies `block_held`'s own rule to that path: one
  screen per press, until the key is released. The same hole dismissed the
  dungeon map and the help page instantly, and could carry a held key
  straight through `Mode::GameOver`.

### Affinities

- **A species can now be good at something, not just tougher or softer.**
  Five categories — `damage`, `heal`, `buff`, `debuff`, `drain`, one for
  every `AbilityEffect` that carries a magnitude — get a per-species
  multiplier declared in the species' `.ron` file, clamped to 0.5–2.0 at
  load. It applies to whatever is *installed* in a program's routine slot,
  not only to abilities its species natively grants, so a species with a
  strong heal affinity and no innate heal is now a reason to pop a
  researched or extracted heal routine onto it rather than another program.
  Six shipped species carry one strength and one weakness apiece —
  SubProcess leans heal over damage, Sentinel buff over damage, Cipher
  debuff over heal, Rootkit drain over buff, Scrapper damage over heal,
  Ghost damage over buff — and the rest of the roster, both bosses
  included, stays neutral across the board. `Cleanse` and `Decompile` have
  no magnitude, so no affinity ever touches them.
- **Five new perks buy the same specialization for the player**: Payload
  Tuning, Field Medic, Overclocker, Corruption Vector, and Siphon Protocol,
  2 Perk Points each, clamped at the same 2.0 ceiling a species' own
  affinity is. Field Medic, Overclocker and Corruption Vector are +5% per
  level; Payload Tuning and Siphon Protocol are +15%, because `Damage` and
  `Drain` skip the level-scaling the other three categories get for free —
  at the shared 5% rate they were a strictly worse Perk Point than the
  `Attacker`/`Defender` perks for nearly every shipped ability, and 15% is
  what makes them competitive up to their (lower, faster-reached) ceiling.
  A player perk only ever sharpens the *player's* own casts — a companion
  answers to its species instead — and the two sources can never stack: the
  player has no `Creature` component to hold a species affinity, and a
  companion has no `Perks` to hold a perk one.
- **The manifest screen shows a program's non-neutral affinities**, up to
  two, with any further ones collapsed into a "+N more" line rather than
  crowding the panel.
- **No save-format bump.** Species affinities are `.ron` data, reloaded
  fresh every start same as any other species field; the five new `Perk`
  variants were appended to the end of the enum rather than inserted, so
  every index an existing save already holds in `PlayerSave::unlocked_perks`
  still points at the same perk it always did.
- **The magnitudes are unplayed.** The two per-level rates, the 0.5–2.0
  clamp, and all twelve shipped species values are arithmetic-plausible and
  nothing more — none of it has been through an actual playthrough, and
  `balance_sim` structurally cannot vouch for it either, since that
  simulator models no abilities at all. A green suite here is evidence the
  arithmetic doesn't crash, not evidence the numbers are right.

### Dungeons

- **First-person dungeon levels.** Every zone is now seeded with a handful
  of **breaches** (`>`) standing on open ground. Walking onto one drops the
  party into a procedurally generated dungeon level, drawn as a receding
  first-person corridor rather than the top-down grid — the movement keys
  become forward, back, turn left and turn right, and `<`/`>` take the
  stairs. Levels are mazes with their dead ends partly braided back into
  loops, generated deterministically from the world seed and the depth
  walked to, so a level is the same every time you return to it and costs
  the save file nothing.
- **A map of what you have seen.** `g` — the same key that scans the ground
  on the surface — opens the party's map of the level they are standing in.
  It is drawn north-up and records only what has actually been in view:
  corridors walked, stairs found, and the corridors something jumped you in.
  Cells never seen stay dark and are drawn differently from rock that has
  been seen. Opening it costs no time. Each breach keeps its own map, and
  each level of a shaft its own; the map is written to the save, because a
  level regenerates from its seed but what you have seen of it is history.
- **Doors, and a vault at the bottom.** Levels now hang a few plain doorways
  in their corridors — walkable, but you cannot see past them, which turns a
  stretch of corridor into a decision. The lair is walled off behind **sealed
  doors** on every way in, and opening one burns an **Access Shard**: a new
  item found in caches. Once opened a door stays open, including across a
  save, so the way back out is free and a party that spent its last shard
  getting in is not stranded. A shaft is now a chain: caches pay for the
  shard, the shard buys the vault, the vault holds the guardian.
- **Something is holding the bottom.** The deepest room of a shaft is now a
  lair, marked `&`. Walking in starts the fight; what you face is drawn from
  the breach tile's biome, from the boss pool rather than the ordinary one,
  and is seeded off the level itself — so leaving and coming back cannot
  reroll a shaft's guardian into something easier. Biomes with no boss (no
  shipped Static Field species is one) field the toughest ordinary program
  they have instead, which at the bottom of a six-level shaft is no small
  thing. Jacking out leaves the lair held; killing what is in it clears the
  shaft for good, and that survives a save.
- **Caches.** Dead ends now hold something. `braid` deliberately leaves half
  of the maze's dead ends in place, and until now walking one was purely a
  waste of time; three cells per level hide a cache, marked `!` in the view
  and on the map. Walking onto one empties it, once and for good — the map
  stops advertising an emptied cache, so it answers "where is there still
  something" rather than "where was there once". Payout is depth-scaled
  Credits (the one currency that survives a breach), a chance at a portal
  fragment that grows with depth, and whatever the item set declares.
- **Items can declare themselves cache loot.** New optional `cache_drop`
  field on an item `.ron` — the chance a dungeon cache holds one. Rolled per
  cache per declaring item, so a mod adding items makes caches richer without
  touching engine code. See `assets/items/README.md`.
- **A breach is a shaft with a bottom.** Descent used to be unbounded — the
  depth counter simply went up, with the stat multiplier compounding the
  whole way and nothing down there. A breach now opens onto a shaft of fixed
  length, read off how far the walk to it was, so the same distance that
  scales what lives down there scales how much of it there is. The bottom
  level generates with no stairs down at all. Breaches also now carve their
  own dungeons: the entrance tile is part of the level seed, where before
  every hole in a sector opened onto the same maze at a given depth.
- **The surface keeps running while you are underground.** The player's
  position on the zone map stays pinned to the breach they entered by, so
  cronjobs keep paying out, needs keep decaying, and a raid can land on the
  base while the party is several levels down. Actions that reach into the
  zone map — deploying, cronjobs, guarding, demolishing, upgrading,
  trading, resting and scanning — are refused underground and say so. Party,
  inventory, routine, fusion and perk management are not: sorting your gear
  in a dungeon is a thing the genre expects.
- **The symlink is the way out.** `u` works at any depth, and is the only
  guarded action that changes locale rather than being refused by it: it
  surfaces the party and then teleports them to the structure they picked,
  for the usual cost. It cannot be used to flee a fight — symlinks are
  refused mid-battle wherever you are — and it gives up the descent, since
  the breach puts you back on level one. It does not give up the maps: those
  are keyed by breach and depth, so every level already walked comes back
  drawn.
- **Arriving in a sector scans for breaches.** The log reports how many are
  in it and which way the nearest lies, and one breach is always placed
  within sight of where you materialize — the other two are a walk. Without
  this the layer was effectively invisible: at the default zoom the map pane
  shows about ±16 by ±9 tiles, and three breaches scattered across a
  40-tile radius left most seeds with none on screen and nothing to suggest
  they existed.
- **Dungeons have teeth.** Every step underground rolls for an intrusion.
  The pack is drawn from the biome the breach opens in — a dungeon has no
  biome of its own, so it reads as the substrate beneath the ground above
  it — and each level of depth multiplies enemy stats, and with them the XP
  a kill pays. Shoving at a wall never draws a fight; only a step that
  covered ground does. Jacking out ends the fight where you stand, and the
  pack is cleaned up rather than left waiting at the breach mouth for when
  you climb out.
- **Save format bumped to v13**, so saves written by earlier builds no
  longer load. `SaveData` gained the locale (depth, cell, facing and
  entrance) and the zone's breach tiles; the level itself is not stored,
  regenerating from the seed.


### Economy

- **Traders deal in Credits, not Core Fragments.** Core Fragments were both
  the raw salvage every build cost and recipe consumes *and* the money a
  trader paid you, which made the iso Market a scrap dispenser rather than a
  merchant. Selling now pays **Credits**, a new item minted by nothing else,
  and buying spends them. Core Fragments stay exactly what they were for
  building, mining and scanning — and are now sellable, since they are
  ordinary goods to a trader.
- **Credits survive a zone breach.** Core Fragments and Portal Fragments are
  still wiped in the crossing, so a zone must still fund its own exit, but
  converting a doomed stockpile into money before you go now works and is
  the point of having a trader. Selling into a 1-Credit floor rate and
  buying back at 3–8 means the conversion is lossy; crafting from salvage is
  still cheaper than sell-then-buy, so the Market is for junk you cannot use
  and for value you want to keep, never for efficiency.
- `TradeCurrency` joins `Currency`, `ResearchCurrency` and `CraftCurrency` as
  a required economy role, so which item is money stays a data change rather
  than a code change. An item set that claims no `TradeCurrency` is refused
  at startup with the role named, as the other three already were. Prices and
  payouts now read the currency's name out of its `.ron` instead of saying
  "Core Fragments" in twelve hardcoded places — swapping the currency item in
  a mod no longer leaves the UI lying about it.
- No save-format change: `Inventory` is keyed by item id, so existing saves
  load with zero Credits, which is also where a new run starts.
- **Traders remember what you sold them, and will sell it back at double.**
  A sale used to be final, which made hoarding the safe play and selling a
  last resort. Everything you sell now goes onto that trader's buyback
  shelf, purchasable back at twice its `sell_rate` per unit. The shelf is
  finite — it holds only your own sales — and every round trip is a net
  loss, so it is a way to walk back a mistake, never a way to make money.
- A shelf belongs to the **tile** a trader stands on rather than to the
  building, so a Market levelled by a raid and rebuilt on the same footprint
  reopens with its stock intact, and two Markets in one zone keep separate
  shelves. Rebuild somewhere else and you have opened a new store; losing a
  trader that still holds stock now says what was on it and how to get it
  back. Shelves are wiped on a breach, alongside build salvage and breach
  keys — a shelf that crossed would be exactly the stockpile that wipe
  exists to strand.
- Selling a *program* still destroys it. It is not shelved, and cannot be
  bought back.
- **Save-format bump to 12.** `SaveData` carries every shelf, since a shelf
  outlives its building and can sit on a tile holding nothing at all.
  Existing saves stop loading.

### Moddability

- **Perk names, descriptions and prices are data.** `assets/perks/` now
  holds one `.ron` file per perk carrying its name, its picker line and its
  Perk Point cost, so retitling or re-pricing the menu is a file edit. It is
  a *catalogue* rather than a content directory: the twelve perks stay
  fixed, because a perk's effect is a hook into one particular formula — the
  scan roll, the hunger multiplier, the decompile HP term, recipe costs, a
  direct stat write — with no shape shared *across the full set* to put in
  a file, so a thirteenth perk still means Rust. (The five affinity perks
  added since are the one family that does share a shape among themselves —
  one generic `Perk::affinity_kind` hook, and one of two per-level rates
  depending on category, cover all five — but the magnitude is still a
  difficulty knob, not a per-perk `effect:` field, so the catalogue stays
  name/description/cost only.) Per-level magnitudes stay in `tuning.rs`
  with the rest of the
  difficulty knobs; only cost moved. Deleting a file removes that perk from
  the picker without touching levels a save already holds.

### Balance

- **Exploit Focus buys healthier targets, not better odds.** The perk used
  to grant +1 effective Decompiler skill per level, which was the same thing
  levelling up already hands you for free — so three levels' worth of Perk
  Points bought one level's worth of automatic growth, and the perk was
  strictly the worst purchase on the menu (about +0.5 percentage points of
  decompile chance for 3 points, against Attacker's +1 permanent Attack for
  2). It now shaves 3% per level off how much a target's *remaining
  Integrity* counts against the attempt. That is a different axis from the
  Decompiler stat: it is worth most against a program at full health and
  nothing at all against one you have already drained, so it buys you the
  option of cracking something early rather than a flat lift on every roll.
  Draining a target first is still worth up to 3.6x and no stack of the perk
  inverts that.
- **Bosses can no longer be decompiled.** A boss is built to be a wall —
  roughly thirteen times the Integrity of the toughest ordinary species —
  and capturing one moved that wall into your roster, where fusing two of
  them compounded it into something no encounter in the game could threaten,
  with two more fusions still to spare. Decompile is now refused the moment
  you aim it at a boss, so it costs you neither the round nor the catalyst,
  and the inspect and battle screens quote no odds where there is no attempt
  to make. Fusing a boss is gone with it, since fusion needs two compiled
  programs. Bosses keep their stats, their guaranteed Portal Fragment cache,
  and their place as the thing you beat rather than the thing you own. A save
  that already holds a captured boss keeps it.
- **A program brought to 0 HP is deleted for good.** It used to be knocked
  offline — dropped from your party at the end of the fight, then healed
  back to full for free the next time you recharged. Now it is destroyed,
  along with every routine installed on it, and nothing drops. This applies
  in Forgiving as well as Permadeath (that setting still governs only what
  happens when *you* flatline), and outside battle as well as in it: a
  cronjob worker that runs out of Integrity defending a structure is lost
  even though you were not there. Programs have no passive healing, so raid
  damage is attrition you have to manage by coming home to recharge. The
  battle pane and the party menu now flag any program at or below a third of
  its Integrity in red.
- **Structure income no longer doubles with every zone.** A worked node's
  payout used to be its upgrade tier multiplied by the zone's enemy-difficulty
  curve, so a Mk5 node paid 5 a cycle in zone 1 and 80 in zone 5 while build
  costs, upgrades and market prices all stayed flat — Core Fragments stopped
  being a constraint by about zone 2. Payout is now the tier *plus* one per
  zone below the current one: a Mk1 node pays 1 in zone 1 and 3 in zone 3, a
  Mk5 pays 5 and 7. Depth still pays, upgrading still pays, and neither
  compounds the other. Banked resources such as Research Data are unchanged
  at a flat 1 per cycle. Existing saves keep whatever they have banked; only
  the rate changes.
- **Decompiler skill can no longer be stacked past a species' resistance.**
  Skill used to add flat percentage points to a decompile chance, while the
  odds it was added to could never exceed 0.33 — so at about player level 30
  every attempt pinned to the 95% clamp, and a boss was no harder to take
  than a Drone. Skill now multiplies your odds instead, leaving the target's
  species and how far you have weakened it inside what gets multiplied. A
  fully-weakened Overseer at skill 40 sits near 30% rather than 95%; the same
  attempt against a Drone is near 59%. Low-skill play is essentially
  unchanged.

### Battle

- **Battle narration scrolls in instead of landing all at once.** A resolved
  round used to appear as a block of text; its lines now arrive at a steady
  pace you can read. Pressing any key skips straight to the end, and the
  action bar stays hidden until the narration finishes, so a key pressed
  mid-scroll never spends your round by accident.
- **Each battle's log pane starts empty.** The pane used to show the tail of
  one shared rolling log, so a fight opened on the end of the previous one.
- **Only a battle's results follow you back to the map.** The blow-by-blow
  stays in the battle it belongs to; what you read once the map is back is
  the kill, the XP, the loot, the level-ups and the decompile verdict — and
  those scroll in at the same pace. A raid that lands mid-fight is kept too,
  since that is world news rather than battle narration. One consequence
  worth knowing: the final round's blow-by-blow is dropped, so you see that
  you won and what you got, not the blow that won it.

### Renderer

- **The map glides under you instead of jumping a tile at a time.** There was
  no camera at all: the view was rebuilt around your integer position each
  frame, so a step moved the entire map by one whole tile in one frame. A
  camera now trails you and eases into place, and the grid is drawn shifted by
  whatever fraction of a tile it is currently behind. Because you are drawn at
  your own position rather than pinned to the centre, you visibly lead the
  camera while it catches up. It never falls more than a tile behind, which
  also means breaching a zone arrives as a cut rather than a long pan across
  terrain you are not in any more. Turning effects off makes the camera
  instant, the same way it makes an HP bar instant.
- **The ground has texture and the pane has depth.** Every tile of a biome
  used to draw at one identical colour, so a stretch of Open Grid read as a
  colour swatch. Each tile now varies slightly, by an amount fixed to its
  world coordinate — so the variation belongs to the ground and stays put as
  you walk over it. Tiles with something standing on them are left alone,
  since their colour is already carrying that structure's damage. Separately,
  the map dims toward the edges of its pane. That one is decoration rather
  than information: it stops well short of hiding anything, and a hostile at
  the edge of the view is as visible as it ever was.
- **Quitting asks first.** `q` in a run used to drop it on the spot, so one
  mistyped key cost every tick since the last autosave with nothing asked and
  nothing said. It now offers *save and quit*, *quit without saving*, or *keep
  playing* — and if that save fails it keeps you where you are with the error
  on screen rather than leaving anyway. `q` at the main menu, which ends the
  process, takes a plain yes/no; it sits one key from New Game and Load Game.
- **The log and the menus are colour-coded by what a line actually is.** Items
  you receive read blue instead of sharing green with a level-up. A hostile
  program's blow reads red, and orange when the move it used also inflicts a
  condition — at the time this shipped no hostile had a Special of its own,
  so a condition-bearing move was the whole of what made one special; a wild
  carrier running its own routine is now the exception. A party member's hit
  stays quiet except for the damage figure itself, which is picked
  out in bold white rather than the old all-or-nothing bolding of the whole
  sentence. On the research screen a node you have already unlocked greys out,
  so the list reads as what is left to buy rather than a flat menu. The Shield
  structure is blue on the map instead of red, which read as a threat.
- **A manifest screen (`d`) shows everything the sim knows about you or any
  program.** Integrity and XP meters (plus Power and Fatigue for you), combat
  stats, all four potential rolls behind the Excellent/Poor tag, installed
  routines, and — for a program — habitats, moves, work aptitude, growth and
  speed; for you, each equipped item with the bonus it is actually granting,
  and every perk with its level. `←`/`→` page between you and everything you
  own, and `Esc` steps back to the list rather than all the way out. Several of those numbers had no readout anywhere before: the
  individual rolls, growth, base speed, and a perk's purchased level.
  It replaces the old inspect popup rather than sitting beside it, so `i` +
  direction now opens the manifest for the program it finds.
- **The first frame no longer crashes on a missing font.** The window opened
  and the game died immediately with `FontFamily::Name("fp-ui") is not bound
  to any fonts`: the fonts were installed from inside the egui pass, but
  `set_fonts` only takes effect at the start of the *next* pass, so the first
  frame drew against families that did not exist yet.
- **The frontend runs on Bevy and egui instead of macroquad.** The engine was
  always built on `bevy_ecs`, so the sim and the renderer now share an ECS
  version — though they still meet only through `Game`, as before. Nothing
  about the screens changed: the same tiles, bars, popups and menus, drawn by
  the same code. Two player-visible differences, both improvements: typed keys
  now follow your keyboard layout instead of assuming QWERTY key positions,
  and overlapping sound cues play together rather than cutting each other off.
- **Drawing goes through one seam.** The ~3,000 lines that draw the screens no
  longer name a graphics library at all; they name a `Painter` with twelve
  operations on it, and the library sits behind that. That is what made the
  backend swap a change to one file rather than to every menu, and what would
  make the next one cheap too.
- Startup on a machine with no display still fails with a readable message
  rather than a backtrace.

### Encounters

- **Crossing open ground can get you ambushed.** Every walked step carries a
  small chance that a biome-appropriate pack drops in beside you and engages
  on the spot, with no option to route around it. Previously every fight was
  one you chose by walking into a program drawn on the map, which made travel
  free and the map a puzzle of avoidance. An ambush never fields a boss or a
  nest — those stay something you find and choose to take on — and never
  fires on your base platform, which remains safe ground. Tuned by
  `RANDOM_ENCOUNTER_CHANCE`.
- **Jacking out is an attempt, not a guarantee.** `j` used to end any fight
  unconditionally for a flat XP setback, so a hopeless battle cost 20% of
  in-level XP and nothing else. Whether you get clear is now a roll weighing
  your whole party's summed power against the whole pack's, times a fresh
  luck draw on every attempt — clamped so no escape is hopeless and none is
  certain. A failed attempt burns the round and draws a free volley from
  every engaged group, but costs **no** XP: you pay the setback only for an
  escape you actually got, so retrying bleeds Integrity rather than
  progression.

### Combat flow

- **The round-resolve page is gone.** A resolved round used to open a
  full-screen narration overlay that had to be dismissed before planning the
  next one. It never had anything to show — its log source was never written
  to — while the battle screen's own log pane was already carrying the real
  narration. Rounds now resolve straight back into planning, separated in the
  log by a dim `── round N ──` line.
- **`[A]ll attack` and `[D] all defend`** plan every open party slot in one
  keypress. All-attack asks which group only when more than one is still up.
  Neither overwrites a slot you already chose for.
- **Choosing Special now asks which ability, then who gets it.** Commanding a
  party member opens a picker of that member's abilities, and the screen that
  follows names your choice — `Pick a target (Hot Patch)` rather than a bare
  prompt. Backing out with Esc steps back one screen at a time. A planned
  Special also reads on the roster as the ability it will spend
  (`Hot Patch -> A`) instead of the generic word.
- **Buffs and heals can be aimed at any party member.** A buff or heal now
  lists your own side — you and every standing companion — instead
  of always landing on you. A debuff still picks an enemy group. Companions
  could never actually hold a buff before this: only the player is spawned
  with a buff slot, so one aimed elsewhere would have changed nothing.
- **Abilities are moddable data.** Drop a `.ron` file in `assets/abilities/`
  and it's picked up at startup, same as a species, structure or item — see
  `assets/abilities/README.md`. What used to be a fixed set of four things
  in Rust (rally, shield, heal, debuff) is now whatever the files say. A
  species names the ability ids it grants and the companion level each
  unlocks at, so a program's kit grows as it levels instead of being fixed
  when it's tamed. Ten abilities ship; seven species now declare kits, and
  the rest still fall back to Priority Boost.
- **Abilities can hit more than one target.** Three new shapes: every member
  of one enemy group (Cascade Overflow), every hostile program on the field
  (Broadcast Storm, Null Route), and the whole party at once (Redundancy
  Sync, Overclock Array). A sweeping ability skips the target picker
  entirely — there's nothing left to choose — and a party-facing one skips
  members who are already down rather than wasting the heal.
- **Rogue programs can now be destroyed from any rank.** Only a group's
  front member could ever take damage or die. Anything behind it was
  untouchable, which is what made area effects impossible; a back-rank
  program killed by one now drops out of its group and pays out its loot and
  XP exactly as a front kill does.
- **Abilities have cooldowns and their own Fatigue costs.** A sweep is a
  decision rather than a rotation: Broadcast Storm sits out four rounds and
  costs 15 Fatigue against the flat 5 an ordinary command charges. An
  ability that's cooling or that you can't afford is greyed in the picker
  with the reason, and can't be planned at all — no silently wasted round.
  Cooldowns last one intrusion and are never saved.
- **The trade screen shows equipment tags.** Sell and buy rows now carry the
  same `(WEP +3 ATK)` tag the inventory does, so you can tell a weapon from a
  module — and check a fusion tier — without backing out to compare. Sell
  rows show the tier of the copy you actually hold; buy rows show unfused
  stock.
- **The compile screen says what each item does.** Every recipe now carries a
  short gloss next to its name — `Power Cell (+25 power)`, `Arc Lance
  (+3 atk)` — so you can tell what you're building without leaving the menu.
  Read off each item's own definition rather than written per item, so a
  modded item gets one too.
- **The log moved between the two rosters.** Hostiles stay on top, your party
  is now along the bottom, and the narration sits between them, so the two
  sides face each other across the account of what passed between them.
- **The creature you're addressing is bold.** The party member currently
  choosing an action, and the group highlighted in the target picker, are
  drawn in bold on top of the existing highlight — those rows sit among others
  already coloured by faction and reach, and needed to win against that.
- **Both battle rosters are a stat table now.** Each side gets a column
  header and hard columns — `GROUP`/`NAME`, `HP`, `ATK`, `DEF`,
  `RANGE`/`POS`, `STATUS`/`ACTION` — with the numbers right-aligned, so you
  can compare two groups' DEF by scanning down a column instead of reading
  two sentences. Reach and status conditions move out of inline
  `<engaged>`-style tags into columns of their own, and a member with no
  condition reads `OK` rather than blank. The HP bars are unchanged.
- **Decompile odds are back on the battle screen, as a `DECOMP` column.** The
  hostile roster now carries each group's live compile chance beside the HP
  that drives it, so you can watch a group become worth taming as you wear its
  front program down. The engine had been computing per-group odds all along
  and the only place they surfaced was the target picker — you had to commit to
  Decompile before you could see whether it was worth trying, and during a
  companion's turn there was nowhere to see them at all. The column reads `—`
  when you hold no taming catalyst, since there is no attempt to quote odds
  for; the action bar still carries the reason. See
  [Decompile chance](README.md#stats).
- **Battle keys are lowercase, and Decompile moved to `c`.** Defend takes `d`,
  so the per-slot keys `a`/`d` and their party-wide counterparts `A`/`D` line
  up: shift means "everyone does this". Nothing sits one shift key away from a
  different action.

### Programs

- **You can sell a compiled program at an iso Market.** Until now there was
  no way to get rid of one: standing a program down with `p` frees a battle
  slot but leaves it owned and still counting against your roster cap, and
  fusing needs a second program you are also willing to lose. So a roster
  full of programs you had outgrown was a dead end, with decompiling refused
  outright at the cap. Selling pays a tenth of the program's power (max HP +
  Attack + Defense, rounded down, never less than 1) and frees the slot.
- **It is permanent, and it says so.** The sale erases the program, and the
  confirmation names anything the sale cancels — a party slot, a cronjob, a
  guard post — since it takes those down for you rather than refusing.
- **Modders:** whether a trader buys programs, and what it pays, is the new
  `program_sell_divisor` inside a structure's `trade` block. Omit it and the
  trader deals in items only, so existing structure files are unaffected. See
  `assets/structures/README.md`.
- **Fusing (`f`) now offers every program you own**, not just the ones
  standing within 40 tiles of you. Fusion itself never had a distance
  requirement — only the picker did, so a roster of workers left at far-off
  nodes reported "no compiled programs nearby" and there was nothing to fuse
  without walking the map to collect them first. Both pages now list the
  whole roster, the same way the pets screen does.

### Structures

- **The Recharger Node actually recharges you now.** Instead of gating rest,
  it passively restores 1 Power per tick anywhere within 7 tiles — the whole
  base footprint — with no assigned worker and no input item. Being home
  means never watching your reserves drain. Its cost rises from 5 to 10 Core
  Fragments to match. Power Cells and the Terminal are now expedition gear
  rather than daily upkeep.
- **Resting moved to Home.** `r` (recharge overnight) now works anywhere
  within 7 tiles of Home rather than within 2 tiles of a Recharger Node, so
  the base you already built is the place you rest. Existing saves need no
  migration and nobody is locked out — Home has always had to be built before
  anything else. A Recharger Node deployed under the old rules simply stops
  gating rest and starts regenerating Power, at the price you already paid —
  see [Structures](README.md#structures).
- **Modders get a `power_regen` field.** Any structure can restore Power in a
  radius by setting it; nothing in the engine names a structure id to do it.
  See `assets/structures/README.md`.

### Balance

- **Every difficulty knob is in one file now.** Tuning values were spread
  across three tiers with no boundary between them: 56 constants in the
  crate root interleaved with module declarations, 28 more scattered across
  seven other modules, and — worst — roughly thirty numbers that mattered
  most sitting anonymous inside formulas. The zone stat doubling, the
  steepest curve in the game, was an unnamed bit-shift. All of it is now
  `crates/engine/src/tuning.rs`, grouped into labelled sections with its
  documentation intact. No behaviour changed; the values are identical.
  Content stays data — species, items, structures, abilities and research
  are still `.ron` files and were not pulled into Rust.
- **`balance.rs` is `balance_sim.rs`.** It was never a table of balance
  constants; it is a 977-line offline battle simulator used as a
  regression-test harness, and the name sent anyone hunting for tuning
  values to the wrong file.
- Two formulas had been quietly duplicated rather than shared: the mining
  reliability curve was copied into the balance simulator whose own doc
  claimed it mirrored the real one, and a cronjob node's default worker
  capacity was repeated as a bare literal. Both now read one definition.
- **A pack is a swarm now, sized by depth and distance.** A wild group used
  to be a handful of programs however deep you were. Its ceiling is now the
  zone — 1 at zone 1, tripling each level (3 / 9 / 27 / 81) to a hard 100
  from zone 6 — and you only meet that ceiling by walking to it: a group
  doubles every 15 tiles from the edge of your platform. Four groups at a
  hundred apiece is the most one intrusion can hold. A swarm that size is
  an attrition problem rather than an instant wipe, because only some of a
  group can bring weapons to bear in a round — ten of a hundred, the square
  root of its size — and because anything past the front two groups has to
  shoot or idle. Members over a ceiling aren't deleted; they stay standing
  on the map and are met on the next bump.
- **How many groups meet you rides that same curve.** The *number* of
  groups used to jump straight to four anywhere on the map, while their
  *size* started at one and grew with distance — so the two halves of the
  same danger curve disagreed, most sharply right where you'd notice. It is
  now one group at your doorstep and one more per 15-tile step, up to four.
  An encounter beside your platform is a single program however deep you've
  breached.
- **A zone-1 opening is winnable.** Zone 1 caps a group at one member, but
  four groups could still engage — so a bump near where you materialize was
  routinely a four-on-one against a player who has no companions yet. Across
  25 generated worlds, 21 had a four-program fight inside 15 tiles of the
  spawn point. Offline projection scores that as a loss against every shipped
  species, and scores eleven of the fifteen ordinary species as a loss even
  one-on-one. So on top of the group-count fix, the first 15-tile step of a
  zone-1 breach now only spawns programs a bare level-1 player can actually
  beat solo, and never a boss. Where a biome has nothing gentle — no shipped
  StaticField species qualifies — it fields its mildest rather than its whole
  roster. Twenty of twenty fresh worlds now win their opening fight, taking
  4-8 rounds to do it.
- **Modders:** nothing new to author for any of this, but a species' base
  stats now decide whether it may open a run, not just how hard it hits. A
  new species that a level-1 player with 90 HP, 6 ATK and 2 DEF can't beat
  one-on-one simply won't spawn in that first step. It is still met
  everywhere else.
- **The base platform shrank from a 15-tile radius to 7.** The platform edge
  is also where the danger curve starts measuring, so hostiles now get tougher
  8 tiles nearer to home — the first stat-escalation step moves from 30 tiles
  out to 22.
- **Your fragments and cores no longer survive a zone breach.** Portal
  Fragments and Core Fragments are cleared as you step through the portal, so
  each zone has to fund its own exit instead of being chain-breached on a
  stockpile farmed somewhere safer. Research Data is banked progress and is
  kept, as are gear, supplies, fusion tiers, your party and your whole base.
  Modders: the wipe is keyed on the `Currency` and `CraftCurrency` economy
  roles, so a custom currency item resets too — see
  `assets/structures/README.md`.
- **A Zone Portal's cost ramp softened from ×zone to +50% per zone.** It cost
  10 Portal Fragments times your zone level; it now costs 10 plus half that
  again per zone — 10 / 15 / 20 / 25 / 30 through zone 5, where it used to
  reach 50. With currency no longer surviving the trip, the old ramp would
  have been a from-zero grind of ~143 kills for a zone-5 breach.

### Frontend

- **The text (TUI) frontend is gone.** It had not been user-selectable for
  some time — the launcher kept it only as a fallback for machines with no
  display and for GUI crashes — and maintaining a second renderer meant every
  screen change was made twice. The macroquad GUI is now the only frontend.
- **A graphical display is required.** Running with no `DISPLAY` or
  `WAYLAND_DISPLAY` now exits with `No display detected; feral-processes needs
  a graphical display.` instead of falling back to text. Playing over SSH or
  on a headless box is no longer possible.

### Routines

- **Abilities are installable routines now, occupying level-derived slots.**
  A companion gets one slot per two levels (capped at six, so six at level
  12); the player gets one per ten levels, starting with one so the first
  *free* slot lands at level 10. The `COMPANION_ROUTINE_SLOT_*` and
  `PLAYER_ROUTINE_SLOT_*` constants live in `crates/engine/src/tuning.rs`.
- **A species' innate kit installs itself.** It's pre-installed at tame or
  fuse time and topped up whenever a level-up reaches a later unlock. An
  innate routine can still be popped back out and plugged into a different
  program — nothing is permanently welded in.
- **Routine extraction.** With a Compiler standing anywhere on the map — no
  proximity requirement — break a program you own down into exactly one of
  its routines. The program and every other routine it carried are
  destroyed.
- **Decompile is an ability now, not its own battle command.** The player
  starts a new game with it pre-installed, reached through the Special menu
  like anything else. It greys out with a reason — no taming catalyst, or a
  full roster — instead of silently refusing the round.
- **Research hands over routine items, not the ability itself.** A
  researched unlock is compiled straight into cargo; installing it into a
  slot is a separate act from researching it, and two nodes naming the same
  ability now stack two copies of the item instead of granting it once.
- **Item and structure descriptions are authored in their `.ron` files.**
  `Game::structure_description`'s Rust derivation is gone, so a mod controls
  its own text end to end for both.
- **New map keys:** `m` opens the routine panel (install, swap, pop out);
  `M` opens extraction — needs a Compiler built somewhere, not nearby.
- **Save format is v11.** Older saves are rejected with a clear message, the
  same policy as every prior bump — this project does no save migration.
- **Wild programs can spawn already carrying a routine their species never
  grants, and will spend a round running it against you** — the first time a
  hostile's kit has ever been more than its species moveset. Decompiling the
  carrier hands the routine over already installed; destroying the carrier
  destroys the routine with it.
- **Twenty new routines exist that no species and no research node grants.**
  Finding a carrier in the field is the only way to get one.
- **Ability magnitudes now scale with the user's level**, so a heal or a
  buff picked up early doesn't fall behind the fights it's used in by the
  late game.
- **A routine that finds no free slot on capture now goes to cargo instead
  of being destroyed.** Popping a slot free later lets you install it same
  as any other routine item.
- **Every ability has a cooldown now, except Decompile** — it stays
  spammable, since it's the capture mechanic rather than a combat move.
  (Field routines, added later, ignore `cooldown` entirely: a field-only
  ability never reaches the Special picker, so there's nothing to throttle.)

## 0.2.0

### Breaking

- **Saves from earlier versions no longer load.** Party order is now
  persisted, because it decides which members stand in the front slots and
  draw more enemy fire — so it can't be rebuilt from creature-iteration
  order the way it used to be. That changes the shape of a saved program
  record, and the save format has no field-level compatibility.
  `SAVE_FORMAT_VERSION` is **v10**; a v9 save is rejected up front with a
  clear message rather than decoding into corruption.

### Party roster battles

- **Intrusions are now a party-versus-party round battle**, not a duel with
  an audience. The whole battle screen is replaced: hostile groups listed on
  top, your party listed below, both with HP and stats, and an action chosen
  for **every** party member before the round resolves.
- **Enemies fight as species groups.** A pack is sorted by species, so three
  Glitches are one addressable unit (`A  3 Glitches`) rather than three
  rows. Only a group's front member can be hit; empty a group and it drops
  off the list, promoting whatever stood behind it. At most four groups
  engage at once — a bigger cluster sends its four largest and leaves the
  rest on the map rather than despawning them.
- **Companions fight.** A party member that Attacks rolls one of its own
  species' moves and deals real damage. The old `[C]ommand companion`
  action is gone; its buff is now the **Special** action, one of the options
  that member can be given for the round, and still costs you Fatigue.
- **Only the front two enemy groups can reach you.** A group further back
  can use only moves its species flags `ranged`, and does nothing at all if
  it has none. This is the valve that keeps a twelve-program pack
  survivable, and it makes clearing front-to-back a real decision — wiping
  the front group promotes a back group into melee range.
- **Initiative is rolled every round.** Every combatant on both sides rolls
  `base_speed + d10` and acts in one interleaved order. `base_speed` is new
  per-species data, spanning 6 (Construct) to 14 (Sprite); the player rolls
  from 11. Both new species fields are optional, so existing mods keep
  loading untouched — see `assets/species/README.md`.
- **Defend** is a new action: a Defense bonus for the round, plus a much
  larger share of the incoming fire. It's applied before anyone acts, so
  bracing covers you against a faster enemy rather than being a coin flip on
  the initiative roll.
- **Soft ranks.** Enemy targeting is now weighted by party slot instead of a
  flat 30% chance to hit a companion. The first three slots draw noticeably
  more fire than the ones behind them, but every member stays reachable — a
  back slot is safer, never safe.
- **Bigger fights.** Party capacity goes 3 → 5, and a pack can reach 12
  programs (3 per zone level) instead of one more than the zone number.
- **The action menu is engine-generated.** Both frontends draw whatever the
  engine offers, including the reason an action is currently unusable, so
  the two can't drift and a new action needs no renderer change.

### Fixed

- **A seeded game is reproducible again.** The habitat spawn pools were
  returned in `HashMap` iteration order, which is randomized per process,
  and the per-tile spawn roll indexes into them — so two runs on the same
  seed picked different species and diverged from there. Only ever visible
  as intermittently-failing tests, never as a wrong-looking game, but it
  made any seeded test unreliable.

## Before versioning

- **Your base travels between zones**: breaching used to despawn every
  structure you'd deployed, which made staying anywhere long enough to
  build one a bad trade — the base was the only part of your progress that
  got deleted when you advanced, and the only one whose output never grew.
  Deploying a **Home** now lays a 15-tile **platform** under your base:
  flattened flooring that obliterates the terrain, nests and rogue programs
  in that radius, and that nothing wild will ever spawn on again. Step
  through a Zone Portal and the whole thing — platform, every structure on
  it, their damage, their stored resources, and their running cronjobs —
  rematerializes around the new sector's entry point in exactly the layout
  it left in. A base founded in zone 1 is the same base you're standing in
  at zone 6. Demolishing Home tears the platform up and the natural terrain
  comes back. See [Zones and portals](README.md#zones-and-portals).
- **A Zone Portal is consumed when you step onto it**: it's the one
  structure that doesn't make the trip. Without that, a portal carried
  forward with the rest of the base would make every breach after the first
  one free, and the 10-fragments-per-zone-level cost would stop meaning
  anything after zone 1.
- **Cronjob output scales with zone depth**: a finished cycle no longer
  drops exactly one unit. It's multiplied by your zone level on the same
  doubling curve wild programs' stats already used — zone 2 pays double
  zone 1, zone 3 quadruple — so a base keeps pace with the sector it's
  sitting in instead of falling further behind every time you breach. A Mk3
  Mining Node in zone 4 drops 24 Core Fragments a cycle where a fresh one
  in zone 1 drops 1. Research Data is the exception: it's banked against a
  200 cap, so it still pays one unit a cycle at any depth. See
  [Getting started](README.md#getting-started-building-and-running-cronjobs).
- **Structures can be upgraded (`U`)**: producing structures now have tiers,
  Mk1 through Mk5. Reaching tier N costs that structure's upgrade price
  times N, and a tier both multiplies what a cycle pays out and raises the
  odds the cycle pays out at all — a Mk1 Mining Node fizzles about half the
  time, a Mk5 about one time in ten. Upgrades ride through a portal with
  everything else, so they're the thing worth pouring materials into across
  a whole run. This is data, not Rust: any structure `.ron` file can
  declare an `upgrade` block, and every file that omits one stays
  un-upgradeable exactly as before, mods included — see
  `assets/structures/README.md` and [Structures](README.md#structures).
- **Danger now scales from your base's edge, not its centre**: wild stats
  still climb 25% every 15 tiles you wander out, but that count starts at
  the platform boundary instead of the tile you materialized on. Since the
  platform is itself 15 tiles across, the first step up moved from 15 tiles
  from Home to **30** — the whole base reads as safe territory rather than
  sitting exactly on the first escalation step. Pack sizes moved with it.
  Before your first Home there's no platform, so it measures from the entry
  point exactly as it always did.
- **Your base is a genuine safe haven**: because platform flooring lists no
  species as living there, nothing wild spawns inside your build radius at
  any zone depth. Raids are the only threat that still reaches it (see
  [Base defense](README.md#base-defense)). There's nothing to scavenge on
  it either — `g` always comes up empty on your own floor.
- **Save format bumped to v9.** Saves written by earlier versions no longer
  load.

---

- **Home can no longer be raided**: raids used to pick from every deployed
  structure, Home included, so a bad roll could destroy the one structure
  that gates every other build, anchors your symlinks, and can only exist
  once — stranding you rather than costing you something. Home is now a
  **non-raidable** structure: it has no Durability, shows no `[HP x/y]`
  anywhere, and is never selected as a raid target. Posting a guard on it
  (`G`) is refused outright now instead of silently wasting a program on a
  raid that will never come — see [Base defense](README.md#base-defense).
  This is data, not a special case in Rust: any structure `.ron` file can
  set `raidable: false` and get the same protection, and every file that
  omits the field stays raidable exactly as before, mods included — see
  `assets/structures/README.md`. You can still demolish Home yourself with
  `R`, cascade and all; non-attackable isn't indestructible.
- **The inventory screen now says which slot an item would take**: an
  equippable item's preview tag leads with its slot — `(WEP +4 ATK)`,
  `(ARM +4 DEF)`, `(MOD +3 DECOMP)` — instead of showing the bonus alone
  and leaving you to infer the slot from which stat moved. That inference
  was only ever reliable while every weapon happened to be pure Attack, and
  the 25-piece catalog's hybrids (a Recursion Blade is +2 Attack **and** +1
  Defense) broke it. Shows on both the inventory list and the item action
  menu, in both frontends — see [Equipment](README.md#equipment).
- **Taming catalysts are data, not one named item**: a decompile attempt now
  spends whichever item in your inventory declares the highest
  `taming_potency` (ties go to the first item id alphabetically), so a
  catalyst dropped in as a `.ron` file works exactly like the shipped ICE
  Breaker, and a stronger one is used in preference to it. No engine logic
  names the ICE Breaker any more — it's ordinary starting gear now, not a
  privileged item — see `assets/items/README.md`. The
  decompile-odds readout changed to match: it quotes the odds for the
  catalyst you'd actually spend, and with no catalyst in hand it reads
  "needs a taming catalyst" instead of a percentage for an attempt you
  can't make — see [Decompile chance](README.md#stats). For a player
  carrying only ICE Breakers, taming plays exactly as before.
- **Item files carrying `NaN` or infinity are refused**: RON accepts bare
  `NaN` and `inf` literals, and they used to survive every clamp downstream
  — a `NaN` `taming_potency` outranked every real catalyst, won the roll,
  then panicked the RNG. A non-finite `taming_potency`, `consume.power`, or
  `consume.fatigue` now disqualifies the whole file, which is skipped with a
  logged warning like any other malformed one. Relatedly, the
  duplicate-economy-role warning stopped Debug-printing ids at modders
  (`ItemId("core_fragment")` now reads `core_fragment`) — see
  `assets/items/README.md`.
- **README caught up with three systems it never documented**: the research
  tree, nests, and audio had all shipped without a word in the README, and
  the research tree in particular had silently invalidated its account of
  the opening — the build menu, the Structures table, and the Equipment
  recipes all described a game with no research gating. Now covered by new
  [Research](README.md#research), [Nests](README.md#nests), and
  [Audio](README.md#audio) sections, plus a rewrite of everything research
  touches. Also corrected: the Data Cache's cost and purpose, the market's
  name, the missing Research Node row, Portal Fragment and Research Data
  missing from the Items table, the carrying-capacity rules, `T`/`[`/`]`/`\`
  missing from the controls list, and a leftover claim that the player
  shares the level-12 cap. The backdated entries above cover the same
  ground from the release-notes side.

---

- **Items are now data-driven**: every item (Core Fragment, Power Cell, ICE
  Breaker, Overclock Core, Firewall Plating, Neural Amplifier, Portal
  Fragment, Research Data, Monofilament Whip, Ablative Plating, Cortex Hack)
  is now a `.ron` file under `assets/items/` instead of a hardcoded `ItemId`
  Rust enum variant, and `ItemId` itself is now a string newtype rather than
  an enum — drop a new item in as data, same as species and structures, no
  recompiling needed. This changes what a save stores, bumping the save
  format to **v8** (old saves need a new game). **Breaking for mods**: any
  species/structure/research file that named an item the old bare-variant
  way (e.g. `CoreFragment`) must switch to its quoted string id (e.g.
  `"core_fragment"`) — see `assets/items/README.md` for the schema and the
  full id mapping — see [Items](README.md#items) and
  [Modding](README.md#modding).
- **Crafting gained a data-declared starter-recipe path**: an item's own
  `.ron` file can now define its always-available "starter" recipe via a
  `craftable` field, rather than the two starter recipes (ICE Breaker,
  Power Cell) being hardcoded in Rust — see `assets/items/README.md`.
- **Consume action added to the inventory item menu**: `[C]onsume` now
  appears for any item that declares a `consume` block, applying whatever
  mix of Power/Fatigue/heal/pre-battle buff it defines. The `e` key changed
  to match: it now drains the first Power-restoring item found in inventory
  instead of being hardwired to Power Cells specifically. No player-facing
  mechanic changed — the 11 shipped items behave exactly as before — see
  [Items](README.md#items).

---

- **A research tree now gates most of the base**: press `T` to spend
  **Research Data** on a tree of 12 nodes. Research Data comes from a new
  **Research Node** structure (10 Core Fragments, buildable from the start)
  worked by a cronjob like a Mining Node, and it banks separately from your
  cargo rather than competing with it. Seven of the thirteen structures —
  Compiler, Terminal, Power Conduit, iso Market, Shield, Fabricator, Armory
  — plus every equipment recipe now sit behind a node, so the opening isn't
  "build whatever you can afford" any more: Home, Mining Node, Research
  Node, Recharger Node, Data Cache, and the Zone Portal are what you start
  with, and the rest is earned. Unlocking is permanent, and a recipe node
  grants only the blueprint — its bench still has to be deployed for the
  recipe to appear in the compile menu. The tree is data, not code: each
  node is a `.ron` file in `assets/research/`, and a structure named by no
  research file stays buildable from turn one, so existing structure mods
  keep working untouched — see [Research](README.md#research) and
  `assets/research/README.md`.
- **Carrying capacity is now a real constraint**: everything you carry
  counts against a shared cargo limit — your **Buffer** — starting at 30
  units. **Data Cache** is what raises it (+10 each, stacking, and its cost
  dropped from 15 Core Fragments to 10 to match its new job); it was
  previously just flavor. Paying an input cost that would overflow the
  Buffer — compiling, buying, unequipping — is refused outright rather than
  clamped, so nothing you already spent gets destroyed. Research Data is
  exempt, banked against its own 200-unit ceiling, so a pile of loot can't
  starve a Research Node's output — see [Items](README.md#items).
- **Programs can only be fused 3 times**: every fusion result is one level
  "deeper" than its deepest parent, and a program that's 3 fusions deep is
  a finished product — it can't be an input to another fusion. The pets
  (`p`), inspect (`i`), and Fuse (`f`) screens all show a program's fusion
  depth, flagging a maxed one. This bounds chain-fusing, which previously
  had no ceiling at all beyond your supply of duplicates. Persisted across
  save/load (bumps the save format to **v6** — old saves need a new game)
  — see [Fusing programs](README.md#fusing-programs).
- **The player's level cap is gone**: you now keep leveling forever
  (gaining stats and a Perk Point each time), while tamed programs still
  stop at level 12 as before. Long runs stay worth grinding instead of
  dead-ending at the shared ceiling — see the Stats table.

- **A busier, better-behaved wild population**: the world-wide cap on wild
  programs is up from 24 to 100, so an area you're exploring stays alive
  with things to fight and tame. To keep the simulation cheap, hitting the
  cap now culls the wild program *farthest* from you rather than blocking
  the spawn — a population you wandered away from quietly thins out
  instead of starving the area you're actually in. Programs near you are
  never culled. One caveat: a nest you've left far behind can lose a
  guardian this way, and it won't queue a respawn for it; walk back and
  the usual kill/tame/respawn cycle rebuilds the nest as normal.

---

- **Recharger Node is now a permanent structure**: it no longer collapses
  after 20 ticks — like every other structure, it just needs to be built
  within 15 tiles of your Home. Existing saves need no migration; a
  Recharger Node that was mid-countdown when this shipped simply stops
  decaying — see [Structures](README.md#structures).
- **The Black Market is now the iso Market**: renamed in the same
  structure-tuning pass (`fbd2bed`) that raised its cost. Nothing about how
  it trades changed — same flat sell rate, same stock — see
  [Trading](README.md#trading).
- **Wild creature nests**: Scrapper, Worm, Wraith, and Trojan can now
  spawn as a stationary Nest instead of an ordinary lone creature/pack —
  it keeps 2-5 guardians of its species tethered within 5 tiles, and any
  guardian that's killed or tamed is replaced 10 ticks later. Walk into
  the nest itself to attack it (it never attacks back); destroying it
  frees any surviving guardians to wander normally and stops further
  respawns. New species schema field: `can_nest` — see
  [Nests](README.md#nests) and `assets/species/README.md`.
- **Individual creatures now roll their own stat variance**: every
  creature independently rolls HP/Attack/Defense within ±20% of its
  species/zone-scaled baseline when it's created (wild spawn or fusion
  result), plus its own ±20% roll on its species' growth rate — so no two
  individuals of the same species are quite identical, and some out-level
  their littermates. Shown as a **Potential** tag (Poor through Excellent,
  with a percent) in the pets screen (`p`) and inspect screen (`i`).
  Fusing two programs averages their rolls into the result. Persisted
  across save/load (bumps the save format to v5 — old saves need a new
  game) — see [Companions](README.md#companions).
- **Level capped at 12 for everyone**: the player and every tamed program
  alike now stop leveling — and stop accumulating XP at all — once they
  hit level 12, regardless of source. This sits above the existing
  level-10 cronjob-work cap (work still stops paying XP at 10; battling
  can still carry a worker from 10 up to the new ceiling) — see the Stats
  table and [Getting started](README.md#getting-started-building-and-running-cronjobs).
- **Tamed programs grow faster the higher their species' tier**: a new
  per-species `growth_multiplier` (`assets/species/README.md`) scales a
  tamed program's per-level stat gains — Easy species stay at the
  standard rate, Medium is 1.25x, Hard is 1.5x, and both bosses are 2x —
  so a tougher catch keeps out-leveling an easy one, not just starting
  stronger. Player leveling is unaffected (the player has no species) —
  see [Companions](README.md#companions).
- **Resting now requires a Recharger Node**: `r` (recharge/rest) only
  works while you're within 2 tiles of a deployed Recharger Node (5 Core
  Fragments to build) — there's no other way to rest anymore. The node is
  also **temporary**: it collapses on its own after 20 ordinary
  game-clock ticks with no one resting near it, though actually resting
  near it doesn't burn down that clock any faster than leaving it idle
  would — see [Structures](README.md#structures).
- **GUI default volume lowered to 20%** (was 60%) — still adjustable with
  `[`/`]` in-game.
- **Command Companion picker condensed to a single line**: each entry now
  reads like `[1] Cipher (Rally Team)` — just the companion's name and a
  terse name for its ability — instead of a stats line (HP/ATK/PWR)
  followed by a separate line spelling out the exact numeric effect. Full
  stats are still one keypress away in the pets screen (`p`) — see
  [Companions](README.md#companions).
- **Buffer perk now scales with max Integrity**: each level adds 1% of
  your current max Integrity instead of a flat +10, with a +10 floor so
  it's never worse than before — a meaningful boost once your max HP has
  grown well past its starting value — see [Perks](README.md#perks).
- **README corrected to match the current structure-tuning pass**: the
  structure-cost table, Terminal's passive rate, and a couple of boss
  move/loot footnotes had drifted out of sync with an earlier balance
  commit (`fbd2bed`) that raised most build costs and sped up the
  Terminal. Data Cache, Mining Node, Power Conduit, Compiler, Fabricator,
  Armory, the market, and Shield all cost more Core Fragments than
  documented, and the Zone Portal costs 10 Portal Fragments per zone
  level, not 5 — see [Structures](README.md#structures) and
  [Zones and portals](README.md#zones-and-portals). The Terminal now cooks every
  tick, not every 15 — see [Getting started](README.md#getting-started-building-and-running-cronjobs).
  Overseer was also missing from the Neural Amplifier's loot sources and
  the stun move list, and credited with a "Corrupt" move it doesn't have
  — see [Current roster](README.md#current-roster) and [Equipment](README.md#equipment).
- **The world now ticks in real time while you're out and about**: once a
  second passes, one full game tick advances on its own — structures regen,
  wild programs can spawn, raids can roll — even if you're just standing
  still. This pauses the instant you open any menu (build, inventory,
  trade, ...) and never fires during a battle, so nothing sneaks up on you
  mid-dialog or mid-fight.
- **Structures must be built within 15 tiles of Home, and Home can be
  demolished**: only one Home can exist at a time, and every other
  structure now has to be deployed within 15 tiles of it. The new `R` key
  demolishes a nearby structure for a 30% material refund; demolishing
  Home cascades to demolish the whole base (with a confirmation warning
  first, since it's irreversible) — see [Structures](README.md#structures).
- **Message feed is color-coded by importance**: routine lines stay gray,
  loot/crafting gains are green, leveling up is bold green, and taking raid
  damage on a base structure is orange — in both the GUI and text UI.
- **Graphics is now the default frontend, no more startup prompt**: the
  launcher goes straight into the GUI instead of asking Graphics-or-Text;
  the `--gui`/`--tui`/`--ascii` flags are gone. The text UI still runs
  automatically if no display is available or the GUI crashes, but it's no
  longer user-selectable — see [Playing](README.md#playing).
- **Wild programs scale with distance from your zone's entry point**: on
  top of the existing per-zone doubling, wandering away from where you
  breached in adds up to another 3× to wild stats the farther out you go —
  see [Zones and portals](README.md#zones-and-portals).
- **Gear scaling brought down to 2× per level (was 2.5×)**: gear was
  overtaking zone scaling badly enough that a fully-geared level 1 player
  could trivialize zones 5+, while grinding without gear couldn't keep pace
  past zone 5 at all. 2× keeps gear and levels both mattering together —
  see [Equipment](README.md#equipment).
- **Wild programs can spawn in packs**: bump into one and any packmates
  spawned alongside it join the fight together. Pack size caps at your
  current zone level + 1 (zone 1 → 2, zone 2 → 3, ...), reached gradually
  the farther the encounter is from your zone's entry point — twice the
  distance it takes per-creature stat scaling to grow. Only the lead
  program can be attacked or decompiled at a time, but every packmate still
  alive retaliates each round, and defeating or taming the leader just
  brings the next one forward — see [The loop](README.md#the-loop).

---

- **Cronjob work now caps out at level 10**: a worker stops earning XP from
  structure work once it hits level 10 — resources keep coming, but further
  leveling requires battling.
- **Home is required to build anything else**: the build menu always lists
  Home first, then Mining Node, then Compiler, and nothing else can be
  deployed until a Home exists — see [Playing](README.md#playing). Since zone
  transitions leave structures behind, that means rebuilding a Home first
  in every new zone too.
- **Fuse and cronjob/guard/party pickers show pet status everywhere**: any
  menu listing your compiled programs now flags party membership and
  cronjob/guard assignment on every row, not just some of them.
- **Fuse duplicate equipment**: from the inventory item menu, `[U]` fuses 2
  copies of an item into a permanent +10% (stacking) bonus for that item
  type — see [Equipment](README.md#equipment).
- **Three new perks**: **Attacker** (+1 Attack/level), **Defender** (+1
  Defense/level), and **Buffer** (+10 max Integrity/level, fully healing on
  purchase) — see [Perks](README.md#perks).
- **Graphical frontend added**: a second, windowed UI alongside the
  original terminal one, with automatic fallback to the text UI if no
  display is available — see [Playing](README.md#playing). Menus scroll to keep
  your selection in view instead
  of clipping, size themselves to use most of the screen, and a structure
  with a cronjob worker assigned gets a yellow outline on the map.
- **Companions passively boost your stats**: every active party member adds
  10% (minimum 1) of its own current Attack and Defense to yours, stacking
  across the whole party and updating live as it levels — see
  [Companions](README.md#companions).
- **Low Power weakens your attacks**: below 50% Power your Attack falls off
  linearly, down to half strength at 0 — on top of, not instead of, the
  existing tick damage from fully running out. Commanding a companion in
  battle also now costs a flat chunk of Fatigue — see the Stats table and
  [Companions](README.md#companions).
- **Fuse now lets you name the result**: after picking both programs, type
  an optional name (12 characters max) for what they become — see
  [Companions](README.md#companions). The Fuse and cronjob/guard pickers also now
  show each candidate's full stats (or Power rating) instead of just a
  level.
- **Decompile odds lowered**: ICE Breaker potency and the Decompiler skill
  bonus were both tuned down — weakening a target first now matters a lot
  more than before, rather than skill alone making most attempts a sure
  thing.
- **Command Companion shows what it'll do**: the picker now lists each
  party member's actual ability (its species' special ability, or the
  computed default rally) instead of just its stats.
- Renamed the Daemon species to **SubProcess** throughout.
- **Battles lengthened**: tripled HP across the board — player starting/max
  HP (30 → 90), per-level HP growth (+4 → +12), and every species'
  `base_hp`. Attack/Defense and damage formulas are untouched, so fights
  just take longer, not deadlier.
- **Save system fixed and reworked**: saves now carry a format version and
  are rejected cleanly (instead of crashing) if incompatible. Save slots
  moved from a single `save.bin` to a `saves/` directory; `L` from the main
  menu lists every save with a summary and lets you Load or Delete each
  one. An existing `save.bin` is migrated into `saves/` automatically.
- **Gear now has levels**: every equipped item's bonus scales 150% per
  level above 1, unlocked by reaching the matching zone depth — see
  [Equipment](README.md#equipment).
- **Shield** structure added: passively reduces raid damage against every
  deployed structure, stacking across however many you build — see
  [Structures](README.md#structures) / [Base defense](README.md#base-defense).
- **Perks reworked**: no longer one-time unlocks — each perk can be bought
  repeatedly, with every level adding a flat +1 to its bonus at the same
  Perk Point cost — see [Perks](README.md#perks).
- **Guard assignment** (`G`): post a compiled program to defend any
  structure against raids, without needing a cronjob — see
  [Base defense](README.md#base-defense).
- **Gear crafting reworked**: the Fabricator/Armory no longer run a
  cronjob to grind out gear — building one unlocks compiling that gear
  (Overclock Core / Firewall Plating) for Portal Fragments instead — see
  [Equipment](README.md#equipment).
- **Companions buff instead of attacking**: commanding a companion in
  battle now grants the player a buff (a rally by default, or a species'
  own special ability) rather than dealing damage directly — see
  [Companions](README.md#companions).
- **Mining is harder**: Mining Node cronjobs take twice as long per cycle
  and gate the payout behind a level-based success chance instead of
  always yielding — see [Getting started](README.md#getting-started-building-and-running-cronjobs).
- **Power rating** added throughout the UI (status panel, pets screen,
  battle screen, inspect) — a rough overall-strength number (max
  HP + Attack + Defense) alongside the individual stats.
