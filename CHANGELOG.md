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

**An entry is a claim a player can check, then the reason it matters.** The
bold sentence says what is different from the outside — what you can now do,
or what has stopped going wrong — in the words the game itself uses, and it
never names a type, a function or a field. One or two sentences follow it:
what was wrong before, and what is different now. Naming code in *those* is
fine where it is the clearest thing to say.

From `0.13.58` to `0.13.160` the bold claim was the whole entry, on the
argument that the explanation lives in the commit message and the memory
graph's seam arguments. It does — but the claim was the *conclusion* of that
explanation, and a conclusion with its premises removed is a riddle. Those
entries could only be read by someone who already knew the code, which is
nobody the changelog is for. The ten most recent sections were rewritten
under this rule on 2026-09-11; the rest stand as written.

Older releases are split across three more files, because GitHub stops
rendering a Markdown file somewhere between 238 KB and 296 KB and this one
had reached 669 KB:

- [CHANGELOG-0.13.md](CHANGELOG-0.13.md) — `0.13.99` back to `0.13.0`
- [CHANGELOG-0.4-to-0.12.md](CHANGELOG-0.4-to-0.12.md) — `0.12.0` back to
  `0.4.0`
- [CHANGELOG-0.3-and-earlier.md](CHANGELOG-0.3-and-earlier.md) — `0.3.4`
  back to the entries below `0.2.0`, which predate versioning and are kept
  as written, newest first, separated by a rule

When this file passes 200 KB, move its oldest releases into a new file of
the same kind and list it here.

## 0.13.243

**Towns and cities take up a square of the map sized by what they are.**
A town is 3×3; a city is 5×5, grows to 7×7 while it thrives and shrinks
to 3×3 when it starves. Every cell is the door and none lets you in, and
you can trade from anywhere beside it rather than one tile from its middle.

**Anything standing where a town grows is moved just outside it** — wild
programs, traps, nests, caravans, you, and your base's entrance. A Stack
entrance it swallows moves too and becomes a new maze, but never while
you are inside that Stack; it waits until you surface.

**Outposts can't be founded within 3 tiles of a town, and a big city's
patrols no longer stand on their own ground.** Nothing spawns inside a
town's square either.

## 0.13.242

**A long activity in the CREW pane is cut short with an ellipsis instead
of covering the program's Integrity.** The activity sits at the right end
of a crew row, so a long one — building or cutting something with a long
name — grew leftward over the INTEG figure. It now stops at twelve
characters and ends in `…` when anything was cut.

## 0.13.241

**Marking ground to dig out now asks for the tiles to floor it, and your
crew only cuts what it can floor.** Before, nothing ordered Blank Substrate
for a dig plan, so a crew could cut a hundred cells and lay no tiles at all
unless you remembered to order some yourself. Now a marked plan keeps its
own standing Blank Substrate order in your work order queue, sized to what
it will spend. It is filed at the bottom, and you can move it up. The crew
also waits to cut a wall until a tile is spare to floor it, and floors
cells already cut before it opens new ones.

## 0.13.240

**A program that couldn't find a way to somewhere to rest now tries again.**
A program in a bad enough mood walks off to unwind at an amenity. Before,
if the way there was blocked even once, it gave up until its mood recovered
on its own, which could leave a whole crew on strike long after the way was
clear. It now tries the walk again every so often, and you are told only
the first time it can't get there.

## 0.13.239

**Ground your crew digs out in the base now stays open for good.** Before,
a cut cell left without a tile turned back into rock after a while. That
could cut your programs off from their posts and from anywhere to rest,
and a crew that kept getting stranded soured into a strike nobody could
dig them out of. Your crew now cuts marked cells even when there is no
Blank Substrate in store. Laying the tile still needs one, and nothing can
be built on a cell until it is tiled.

## 0.13.238

**The depot has new art, drawn in its own colours.** Every depot tier now
shows an orange cabinet with a hazard stripe instead of the old sprite.
Map sprites are normally drawn white and tinted with their structure's
colour, which would have turned this art a murky green; a sprite file named
`<name>.colour.png` now keeps its own colours, and still darkens with the
map's shading and with damage. The naming rule is in
`assets/sprites/README.md`.

## 0.13.237

**Levelling up now opens a page showing what changed.** After the battle
results close — or on your next return to the map, when the level came from
outside a fight — a page reads back your stat gains, how you now stack up
against a typical program of the current sector (hit chance, damage per
swing, swings to win and its swings to down you), and the Perk Points and
Decompiler skill gained. Several levels at once show as one page. `P` goes
straight to Perks; `Esc` closes.

## 0.13.236

**A hostile's colour now reflects how hard it hits, not just how much it can
take.** The green-to-red tint compared a program's Integrity with yours and
all but ignored its attack, so one that hit twice as hard as you could read
Yellow beside one that could barely scratch you. It now weighs a whole
exchange — each side's damage against the other's Integrity — and counts the
programs fighting at your side. The XP a kill pays and the odds of a
decompile read the same measure; XP still measures against you alone, so a
companion never costs you any.

## 0.13.235

**A new round on the battle map is announced.** When the turn order comes
back round, `ROUND N` fades in over the top of the map, a short chime plays
and the log gets a `── round N ──` divider — before, the only sign was the
number changing in the corner of the turn strip. The banner never holds the
fight up: the next body acts underneath it.

**Hits, heals and interrupts on the battle map now show.** The red flash on
a body that is struck, the green `+` over one that is healed and the `!`
over one taking an opportunity swing were each wiped in the same frame they
appeared, so none of them had ever been visible in play. The effects layer
was told a battle-map fight was not a fight, and now asks whether one is
open.

## 0.13.234

**Every Depot now shows how full it is.** A bar along the top of the
Depot's tile fills as it takes stock — green while there is room, yellow
with a tenth or less left, red when it is full — the same three colours
the status bar's `[GRID]` figure uses.

## 0.13.233

**Stripping a downed program for materials now pays a varying amount.**
Each extraction rolls its unit count up to half again either side of what
it used to pay, so a typical kill through the starting tool pays 2 to 4
units instead of always 3 — the same on average. The extraction screen now
names the items a tool can draw out without quoting counts, and the
Teardown Rig rolls the same way. The Teardown perk and a species' own
specialty still add their flat bonus on top.

## 0.13.232

**The status bar's `[GRID]` figure now shows how close the base is to
running out of grid.** The figure is draw over supply. It is green with more
than 5 to spare, yellow at 5 or less (including exactly at capacity), and
red when the machines draw more than the base supplies — the state in which
some of them go dark. It used to be plain grey until the grid was already
short, and then amber, which read as a warning rather than a shortage.

## 0.13.231

**When a production chain or a research project is stuck because nothing
makes one of its ingredients, the message now names the machine to build.**
It used to say "Nothing is making Raw Trace within the Transcriber's reach —
it can only take what a neighbour has finished, or what a worker can fetch
off a Depot shelf", which reads as a placement puzzle when the real answer
is that the base has no Log Scraper. It now says "No Raw Trace — build a Log
Scraper to make it." When the machine does exist but stands out of reach
with no Depot to carry between them, the message names both machines and
both fixes: build one beside it, or build a Depot.

## 0.13.230

**You can found an outpost out on the surface with an Outpost Kit, post
programs to crew it, and watch it grow from raw materials to processed
and then complex ones.** Until now everything the base extracted came from
inside base space. An outpost is a foothold out in the zone: it grows while
it is crewed, stalls and then declines when it is not, and its tier sets what
it yields. The kit is unlocked by the new Outpost Founding research.

**A caravan route can now run to an outpost as well as a town.** It goes out
empty and hauls back up to a cartload of whatever the outpost has stocked. It
can be one-off or standing, and it is dispatched and cut from the same Relay
hub. You can also walk up to an outpost and carry its stock home yourself.

**Outposts can be raided when a GC Entropy Sweep hits, and a raided outpost
loses integrity and stock and can have a crew member downed.** A bigger crew
makes an outpost harder to hit, and a hostile town nearby makes it easier. An
outpost at zero integrity goes dark and sends its crew home until you repair
it. Zone 1 outposts are never raided, and the alert board says when an
outpost is stalling, declining, dark or raided.

## 0.13.229

**The green `+` over a program being mended at a Repair Bay now floats up
and fades out, and the next one starts from the patient.** Before, it
bounced up and down on the spot. Now a long repair reads as a stream of
pluses rising off the body, and two patients side by side float out of step.

## 0.13.228

**Press `N` to open an alert board listing what is blocking the base: stalled
machines, downed programs, sweeps and sieges, cut-off sites, and loads with
nowhere to go.** Before, each of these was a single line in the log that
scrolled away. Now each is kept on a board of up to 50 entries. A repeat
folds into one row counted `×N`, `x` or `d` dismisses the highlighted row,
and a badge on the status bar shows how many are unread. The board is saved
with the run.

## 0.13.227

**A Recharger Node that runs out of Power Cells now has one fetched from
any machine holding them, not only from a Depot.** Before, cells sitting in
a Power Conduit out of the Recharger's reach could never get to it once
every Depot was full: the Conduit's worker had nowhere to shelve them, and a
full Conduit makes nothing, so the grid stayed dark for good. Now a worker
walks to the Conduit and carries a cell straight across.

## 0.13.226

**Staff carrying goods to a Depot no longer get stuck at one they can't
stand next to when another Depot is free.** Before, a worker always picked
the nearest Depot, so one with its only open side blocked — often by an idle
program that couldn't move either — left the machine reading "cut off"
while a second Depot stood open nearby. Now the worker goes to the nearest
Depot it can actually walk to. A base whose only Depot is blocked still
reports the stall.

## 0.13.225

**The icon designer's palette now has a transparent swatch, first in the
row and drawn as a checker.** Before, only Backspace could erase a cell, and
nothing on screen said so; now you can pick "no colour" like any other and
paint holes with Space, or click it in the sprite editor. The editor still
opens on the first real colour, and saved icons are unchanged.

## 0.13.224

**You can now build a Wall: five Core Fragments, no research needed.** You
can't walk through one, and during a siege nothing can see past it, so a wall
is how you shape your base against raiders. Besiegers can still break a
wall down, and once it breaks it stops blocking sight. GC Entropy Sweeps
never target a wall, so a row of cheap walls can't soak up damage meant
for your machines. A siege that resolves while you are away still damages
walls, the way the raiders would have broken through them.

## 0.13.223

**From sector 2 on, the sector's wild programs besiege your base — they walk
in through the door, steal what they can carry, wreck what they cannot, and
leave when half of them are down.** Until now the only threat to the base
was the GC Entropy Sweep, which damages structures and never comes inside.
A siege builds on its own clock, roughly once an hour of play in sector 2
and sooner the deeper you are, and the log warns you when one is gathering
at the perimeter — about twelve minutes out in sector 2, and never less than
eight however deep you go.

**If you are home, you fight it on a battle map drawn from your base
itself.** The rooms and corridors you dug are the board, your machines are
obstacles the raiders can smash, and your staff defend where they stand
under their own control. A raider killed while carrying loot drops it back,
so the fight is about catching them before the door. It is always a battle
map, even with battle maps turned off. A siege in progress is kept in your
save.

**If you are away, it resolves without you**, set against your staff, your
defences and your turrets. A Turret is a new structure that fires on the
nearest besieger in range every round, and also soaks damage from a sweep.
The off-screen numbers are unmeasured: a well-staffed base can currently
beat a siege it never saw at no cost.

## 0.13.222

**A GC Entropy Sweep is a rare event you can hear coming now, instead of
something that happened every forty-odd seconds whether you noticed or
not.** A sweep used to be a 1.2% roll on every cycle, which meant one landed
about every 42 seconds of play — too often for any single one to matter, and
spread so evenly that no stretch of a run could feel safe or feel
threatened. Pressure builds toward one instead: in sector 2 a sweep comes
every fifteen to twenty-five minutes, and sooner the deeper you are.

**The base tells you one is forming, about four minutes out.** Sweep
telemetry thickens around the anchor, the line goes in the log, and the
alert stays up in the info column until the sweep arrives — long enough to
post staff at the machines you care about, patch what is already damaged, or
walk home from a shallow dive. That warning window shrinks with depth along
with the interval.

**Benching your crew no longer makes you un-raidable.** A base under the
defender floor was previously skipping the roll outright, so a base with
nobody on shift was never swept at all. The sweep now waits instead: the
pressure it built is still owed, and it arrives on the first cycle the base
can answer for itself.

**Raiders from a settlement you have wronged come far less often.** Their
rate was set at half the old ambient sweep's, and with sweeps now twenty
minutes apart it had quietly become fourteen times more frequent than the
event it was meant to be rarer than. It is half the sweep rate again.

## 0.13.221

**The pen a program under study stands in is drawn with four small red
squares now, one in each corner, instead of four red brackets.** The
brackets were right-angle outlines opening toward the middle of the tile,
and at the tighter zoom levels they read as apparatus around the body
rather than as a mark on the cell. The squares sit in exactly the same
four places, carry the same amount of red, and the body still rattles
inside them while a project is spending it.

## 0.13.220

**The key that opens your downed programs is named on the control bar now.**
`D` opens the store from the map and from the pack, and the bar along the
bottom of the log named neither — the only place it was written down was the
manual. It sits with the other screens, beside `b base` and `i pack`.

**One key fell off the end of the bar to pay for it.** The bar is one row
with no wrap, so it draws what fits and drops the rest: `x examine` is now
past the cut at 1280x720 and `e drain` at 1920x1080. Both are still in the
manual under `?`, which is where every key this bar cannot hold lives.

## 0.13.219

**You can now relocate yourself, or a body standing next to you, across a
battle map.** Teleport is a routine the research tree teaches in sector 2,
and it is the first one that aims twice: pick a body at arm's length — your
own is the common case — then pick a cell to send it to. How far it reaches
is half your level in cells, floored at one and with no ceiling, so it is a
short hop when you learn it and spans a board by the deep sectors.

**The destination has to be somewhere you can see, and somewhere the body
actually fits.** Both cursors outline the cells they will accept, drawn from
the same two questions the routine asks before it spends anything, so a cell
you are shown is never one that then refuses you. A squad needs its whole
block clear. Nothing lands on a body that is already standing there.

**It is an escape you pay for, not a free disengage.** Invoking beside a
hostile provokes a reaction like any other routine, and a reaction that cuts
it off keeps the Power and the cooldown — you are still standing where you
were. Throwing an adjacent hostile back is the other half of the routine, and
doing that breaks your own cloak where relocating yourself does not.

**Only you can run it.** A companion or a wild program holding it is refused
with a reason on the row rather than silently doing nothing, which is what a
mod dropping it into a species kit will see.

## 0.13.218

**Every program in the Grid now has five attributes you can read, and so do
you.** A program used to be its combat numbers and nothing else, which meant
two Scrappers caught in different places were the same program with different
rolls; there was no answer to "who is this one". Open a manifest and press D
for a dossier: Persistence, Entropy, Bandwidth, Footprint and Parity, each
with the old-school word it stands in for in parentheses after it, over a
build revision, a checksum and one line of where this thing came from.

**None of the five does anything yet, and the manual says so in as many
words.** They are there to be read. A number that promised an effect nothing
implements is a claim you would test and find false, so no gloss, no page and
no help line claims one — when a mechanic arrives it will be designed around
the number already on the sheet.

**A program's five are settled once, where it was found, and never move
again.** Two of one species caught on different tiles are different programs;
the same program reads the same after a save, a reload, a taming, a posting or
a breach. A fused program is a new program and is settled afresh, inheriting
neither parent's sheet. Yours come from the class you picked at creation, a
program's from its species — a Sentinel holds together where a Glitch does
not, a Sprite barely occupies the heap.

**Old saves need no migration and nothing stops loading.** A run in progress
gets its attributes the next time it loads, worked out from the same places a
fresh spawn would have used, so a program you have owned for hours reads as
what it always was rather than as something newly invented.

**Attributes are moddable like everything else.** `assets/attributes/` is five
`.ron` files; a sixth is a file drop, the directory can be deleted to get the
pre-attribute game back, and a species or class file may author its own base
for any of them.

## 0.13.217

**You can leave a honeypot on the ground and come back to a program caught in
it.** Everything you owned used to have to be beaten first, which meant a
fight you could lose for a roster slot you were only speculating on. Research
Deception, build a Decoy Bench and it compiles Honeypots out of ICE Breakers;
place one from your pack with P and a direction, and over the next few minutes
it catches a program of the kind that tile would have spawned and holds it
until you walk into it. What it catches is deliberately worse than what you
would have beaten — never a boss, never better than Optimized however deep you
are, in poorer condition than a fight would have left it, and never carrying
the routine that program was running — so it buys safety with time rather than
replacing the fight. They block you like anything else standing on a tile,
they survive a breach, and D with a direction breaks one down where it stands
and hands nothing back.

**D and a direction works out on the grid now, not only inside your base.** It
used to refuse outright anywhere else, for a reason that was true until this
release: every structure stands in base space, so out on the surface the four
directions pointed at nothing you owned. A honeypot is the first thing you own
that stands on the open grid, so the key opens in both places and the space
you are in decides what it means — your machines inside, your honeypots
outside.

## 0.13.216

Tooling only — nothing about a played game changes.

### The Sprite Forge opens from the map

The dev sprite editor's only door was the main menu, which is unreachable
once a run has started — so drawing a sprite and seeing it on the map meant
quitting to the menu, drawing, starting a new run, and walking back to
whatever the art was for. The dev keypad grows a row for it, and the keypad's
own key is bound above the hand-off to the Stack handler, so the forge now
opens on the open grid, in base space and down a Stack frame alike. Esc
leaves by the door it came through — the keypad for the new row, the main
menu for the old one — and the art is already live when you get back, since a
save reloads the sprite in the same frame it lands. The row is drawn even
with `FERAL_DEV_SPRITES` unset, per the keypad's rule that it hides none of
its rows, and refuses with a sentence rather than doing nothing, which is
what an unbound key looks like.

## 0.13.215

**Your downed programs open straight from the map, with D.** The store was
two screens deep — the pack, then D from inside it — for something that fills
up after most fights and is where a beaten program is actually spent. The
same key now opens it from the map, from your base and from down the Stack,
and Esc puts you back wherever you pressed it. It is still D inside the pack
too; one screen, two doors, one key.

**And the corner of the map says how many you are holding.** The box over the
map's top-left corner already read out what your base is researching and what
it is holding; the store is a third line under those — `held 3/10`. It costs
you nothing until the first body goes into it, and the figure turns amber at
ten, which is the point it starts turning away everything else you beat. A
full store was previously invisible: it refused the next program, said so
once in the log, and nothing on screen carried the fact afterwards.

## 0.13.214

Tooling only — nothing about a played game changes.

### The suite presses keys nobody chose

Every long test loop in the suite ticks inside one screen; nothing crossed
between them, so an arm reached only by an odd order of keypresses was
unreachable from all 6,242 tests. `soak::walk` drives a real `App` built from
each `dev-saves/` template through a pseudo-random stream of every key a
frontend can physically send, and asserts only that nothing panics. The
alphabet is uniform over the whole printable keyboard rather than weighted
towards keys the game binds, because an unlisted key is the one most likely
to reach an unguarded index, and the walk is deliberately ignorant of which
keys each mode reads — asking would be a second copy of `App::handle_key`'s
dispatch. `key_sequence` is split from the walk so that a seed replays a
failure on its own and a shorter walk is a prefix of a longer one.

It found no panic: 250,000 keypresses across the ten templates, and 10,000
in the suite itself at 1.3 seconds.

What the walk is *not* doing is on the record with it. A purely uniform walk
drowns in menus — returning to the map needs Esc at one key in 111, and a
nested popup needs several in a row — so ten templates spent 200 keys each
and advanced the world by 1 to 5 ticks between them, never reaching the
mid-run state a template exists for. `ESCAPE_AFTER` makes one stuck step in
twelve an Esc, which took them to 252 ticks across eight closing modes. Even
so, at 25,000 keys per template only 0.24% of presses advance the world, and
twenty-five times the keys bought 2.4 times the ticks: this is a fuzzer for
screen dispatch, not for a played game, and the per-template figures are
reported rather than asserted on because which template banks how many ticks
moves with any content change.

## 0.13.213

**Most of the research tree is hidden until your base finds it.** Twenty of
the twenty-seven nodes — everything past the handful that gets a base running
— no longer sit on the menu from turn one waiting to be afforded. They are off
the list and off the flow chart entirely, so what you are looking at is what
you could actually take, and the tree grows as the run does instead of being
a priced-out shopping list you read once on the first evening.

**A Research Station with nothing to work on now studies instead of standing
idle.** Its output used to land nowhere whenever no project was selected.
That output is now banked toward study attempts, and an attempt made while a
tamed program is pinned in the Station's pen uncovers a node — announced like
any other milestone, and researchable the moment it arrives, since a study
only ever turns up something whose prerequisites you already hold and whose
zone you have already reached. A study that finds nothing says so.

**The three zone-1 benches now cost a program.** Reactive Armor, Weapon
Fabrication and Routine Fabrication join every node gated at zone 2 and above
in requiring a subject pinned for study, so the chain that makes gear and
Routine Disks is bought with a tamed program rather than with time alone. The
five that get a base running from turn one — Automation, Power Grid, Isometric
Commerce, Teardown and Fortification — still ask for nothing but Research Data.

Saves written before this loads unchanged, with nothing discovered and nothing
banked: the hidden half of that run's tree hides itself and is studied back
out.

## 0.13.212

**A program pinned in a Research Station now jitters while its project is
actually running.** A settled subject wore four red brackets and then sat
perfectly still, which read the same whether the base was working on it or
merely holding it. It rattles in place now — the body alone, inside brackets
that stay put — so the pen says at a glance that something is being done to
the program standing in it.

**It goes still the moment the work does.** Only the one program the running
project is actually spending jitters, so a project that needs no subject, a
project parked waiting on a material it cannot pay for, and a second Station's
subject all leave their programs standing quietly.

## 0.13.211

**A program pinned for study is drawn again, and keeps its own glyph.** It had
been disappearing from the map the moment you selected it — the whole walk to
the Research Station and once standing in the pen — so the four red brackets
that mark a subject sat on bare floor and read as though they had replaced the
program. They are a mark on top of the glyph, not a substitute for it.

**And those brackets now appear only once the subject has settled into the
pen**, rather than the instant you pick one. A program on its way over is
simply one of your programs walking somewhere; it is under study when it
arrives, which is also the moment its project can proceed.

## 0.13.210

**Your programs stop loitering inside the Research Station.** An idle program
could wander into the Station's pen and sit there, and pinning a subject then
refused with "Something is already standing in the pen" while the Station's own
examine line still read Idle, nobody assigned — two true sentences that could
not be reconciled from the outside. A program may now walk across a building's
own floor but not stop on it, so the pen stays clear for the subject. A program
already parked on one steps off by itself, so a base that had got into this
state comes right without anything being rebuilt.

## 0.13.209

**Research now spends a program.** Every node from sector 2 up refuses to
start until you pin one of your tamed programs in the Research Station's pen,
and finishing the project spends it — the program comes back as a downed
record in your store, where extraction can take it apart. Research used to be
free base labour: one program posted on a node paid for the whole tree given
enough wall-clock, which made it a timer rather than a reason to go out and
fight for another program.

**The Research Node is now a Research Station, two cells to a side.** The
top-left cell still carries the glyph and still blocks the way; the other
three are the lab's own floor, and the corner diagonally opposite the glyph
is the pen the subject stands in. A Node already standing in an old save
keeps its single cell and is never refused after the fact — though if its
neighbours are built up, it has a pen nothing can reach, and the only thing
that says so is the refusal you get when you try to pin.

**Fusing two programs is something you research now.** Program Refactoring
unlocks it, and until then the roster's fuse option is hidden rather than
refusing. Fusion used to be available from the first minute of a run.

## 0.13.208

**Your programs no longer stand on top of each other.** Two bodies cannot
share a cell in the base now: a program walking anywhere routes around the
ones already standing there, and a crowd that has already piled up thins
itself out one step at a time. A base with a large roster used to draw a
dozen glyphs for a hundred programs, because the map has room for one glyph
to a cell and every walk stopped at the first tile that would do — which,
approached from the same side, was the same tile for everyone.

**A Repair Bay mends whoever can stand at it, and no more.** That is the
price of the rule above: seventy-one patients lying on one cell were all
touching the Bay, and once they have to spread out only four cells touch it.
A base with more patients than that builds more Bays, and a patient with
nowhere to stand goes back to milling with everyone else rather than holding
a queue — it stays a patient while it waits. A base with no Bay at all is
unchanged: a program benched there still lies where it fell.

**Deploying onto a cell a program is standing in is refused**, instead of
raising the building on top of it. The program you spend on the build is
exempt, since it is leaving anyway.

## 0.13.207

**Running out of roster slots no longer stops you capturing.** Decompiling,
adopting an orphan, buying a program and accepting a town's gift all go
through past your slots. What it costs instead is the base: once your base
is established, each program past your slots, newest first, keeps the
memory of having nowhere of its own to run, and left that way it ends up
lashing out at its colleagues, whose grudges then spread through the base.
A Data Cache or a level of Process Pool gives them room, and the memory
fades. There is still a ceiling, far above anything a base can hold
comfortably.

**The roster row on the attention list now reads "2 unslotted (5/3)"**, and
appears once you are past your slots rather than when you reach them.

**Refunding Process Pool is no longer refused** when it would leave more
programs than slots; the programs left over go unslotted.

## 0.13.206

**A base whose Recharger Nodes have all run dry now lights itself again.**
Before, with no Power Cell on any shelf, nobody was sent to make one: no
work order names Power Cells, so a Power Conduit could stand idle while the
grid sat at the Home's four for good. Now a program goes to a lit Conduit and
makes cells, its worker carries them to a Depot, and the Rechargers are fed
from there.

**When the grid is short, a Power Conduit is the last machine to go dark,
and a Mining Node the one before it.** Before, which machines were cut
depended only on where they stood, so a Compiler in the corner of a base
could take the Home's supply and leave the Conduit dark. The order is worked
out from what each machine makes, so a modded fuel maker ranks the same way
without being told to.

**Your crew holds off cutting a marked cell until the base has a Blank
Substrate to floor it with.** Before, a crew opened every marked cell and
floored only as many as it had Substrate for; the rest was bare ground that
knit back into rock, and every swing that opened it was owed again. Now each
cut claims its floor tile up front, and the crew says once when it is holding
off. Your own swings are not held.

## 0.13.205

**In a fight, you can emulate a species whose image you have learned, and
fight with its kit for ten rounds.** Emulate is a routine, unlocked by
research. Choosing it opens a picker of your learned images, each showing
the attack and mitigation you would fight with. While it holds, you swing
with that species' attacks, reach and affinities and run its routines.
You keep your own Integrity, gear, bought stat points and Decompile.
Changing back costs the turn and no Power. Only the player can emulate:
a companion given the Emulate disk is refused.

**A new extraction tool, the Image Extractor, teaches you a downed program's
image instead of drawing out items.** It unlocks alongside the Routine
Reader. An image you already hold is refused before the program is spent.
Apex (boss) species cannot be learned at all, because in the arena their
image outclassed every build it was tested against. A rolled boss of an
ordinary species still teaches its ordinary image.

**The Emulation Fidelity perk makes every emulation hit harder.** Each
level raises the emulated attack and mitigation. The figures in the image
picker already include it.

**The Emulate disk is not sold anywhere.** Installing a disk doesn't check
research, so a disk on a shelf would have skipped the research node. The
creation shelf, traders, settlements and the Stack market all leave it out.

**A new help page, Emulation, is linked from Extraction and Controls.** On
a battle map, Change Back is `V`; in a group fight it is a row on the
action list.

## 0.13.204

**Nothing changes in play.** This release prepares the code for player
emulation (todo #100), in which the player fights with a learned species'
own attacks, reach and affinities for a few rounds. The four places that
each worked out what a body fights with (its swing, its damage band, its
reach and its routine affinity) now ask one question, so emulation can
be added in one place.

## 0.13.203

**An opportunity attack on a battle map now shows a `!` over the body that
took it and plays a sound of its own.** Stepping out of a hostile's reach,
or running a routine beside one, provokes a free swing, but that swing used
to look and sound exactly like any other blow, so you couldn't tell that an
interrupt had happened. The yellow `!` pops over whoever reacted, and a
short double blip plays alongside the ordinary hit or miss.

**Compiling the most you can afford now tells you when Power is what's
short.** If the compile screen had nothing affordable, it always said "Not
enough resources", even with a full pack and an empty Power reserve. It now
gives the same answer compiling by hand gives, and names Power when that's
the problem.

**The base is no longer heard from out in the field.** Your crew cutting
rock, or two programs brawling at home, could be heard anywhere in the
world. Those sounds now play only while you're in the base, the same place
their flashes are drawn.

**The compiling progress box is now a small window instead of one that
covers most of the screen.** It holds two lines and a bar, but it used to
be as wide as the largest popups in the game.

## 0.13.202

**Five wild programs of the same kind now close ranks and fight you as one
body twice their reach, taking two swings a turn and wearing a `^` to say
what it is.** Meeting five of a kind on a battle map used to be five
separate turns against five separate health bars, which is slow to play and
easy to pick apart one body at a time. They now fold into a single squad
that fills a two-by-two block with one pooled Integrity, one place in the
turn order and two actions in it — and it weakens as you wear it down, so
the last of a squad swings for a fraction of what the first of it did. A
capture pulls one member out and leaves the rest standing, so a squad is
worth five attempts rather than one; beating one pays all five kills. Take
the fight somewhere else and the survivors walk away as individuals again.

## 0.13.201

**A boulder standing between you and whoever is shooting at you now makes
you harder to hit, and walking around it takes that protection away.** On a
battle map, a body with cover on the attacker's side of it dodges half again
as well — but only against shots from that side, so flanking is simply
stepping somewhere the rock no longer helps. Cover does nothing at arm's
length, nothing against a blast, and nothing where the shot could not be
taken at all. The hostiles read the same rule you do: a body that fights at
range will walk to a sheltered cell it can still fire from, while one that
fights up close will not stall behind a rock instead of closing. A body
sheltered from whoever is acting wears a mark in the corner of its tile, and
the cells that would shelter *you* are washed on the movement overlay before
you step.

## 0.13.200

**Walking away from something you are standing next to now costs you a free
hit, and so does running a routine under its nose.** Every body in a battle
map fight gets one of these a round, refunded when its own turn comes round,
so backing out of a melee is a decision rather than a free move. The cells
that will cost you are marked on the movement overlay before you step, and
each routine row says how many bodies it would provoke; a capture provokes
nobody. A routine cut off this way is spent — the Power and the cooldown are
gone and nothing lands. The free swing itself can never fumble, so one bad
roll cannot cascade into an exchange nobody can stop.

## 0.13.199

**Your dig crew can now finish a laid floor: pick a brush with [F] while
marking out a dig, and they paint the floor a colour and carve a pattern
into it.** Base floor was one flat grey wherever you laid it, so a room
you had built up looked exactly like the corridor you cut to reach it.
Each finished tile costs three Blank Substrate, a strip brush takes one
back off, and two of the three shipped finishes are comfortable enough
that a program standing on one remembers it fondly. Drop a file in
`assets/floors/` to add your own.

## 0.13.198

**Press R on your turn in any fight, on either battle screen, to play the
rest of it out at once and go straight to the results.** Your party fights
the way All attack ([A]) already makes it fight: plain attacks, no
routines, so skipping a fight costs fighting it worse than you would by
hand. A fight that cannot be won or lost within 200 rounds hands control
back to you with "Couldn't settle it — finish by hand."

## 0.13.197

**Sound no longer cuts out partway through a tactical battle, and each
blow on a battle map now sounds the moment it lands.** A tactical fight
used to be heard through the pacing that scrolls a normal battle's
narration, which lost its place once the message log reached its
100-line limit, so a long fight went silent until the next one began.
Battle-map blows now have a sound queue of their own; normal battles
sound exactly as before.

## 0.13.196

**Routines are now researched on their own screen, "Routine research" in
the base menu, one rung of a family at a time, and a family only appears
there once you have recovered one of its routines from a downed
program.** Nine base research nodes used to hand routines out wholesale;
they are gone, and recovering a routine no longer teaches it outright —
it discovers the family, which you then research. The screen opens once
Routine Fabrication is researched, and a routine you already know counts
as researched, so an existing save keeps everything it had.

**A program you don't own no longer shows you the names of routines you
have never seen — its sheet reads "??? — a routine you haven't seen" —
and holding Alt on the map marks the wild programs carrying one with a
`?`.** The extraction preview now counts the unfamiliar routines a
program would yield rather than naming them, and a tool refuses a program
with "nothing unfamiliar to recover" when there is nothing new to find.

**Routine research is priced by how far the routine reaches, its version
and the sector it unlocks in**, so an early single-target routine costs
about 15 and a sector-3 routine that hits everyone about 135 — close to
what the old nodes charged. The Patch family's later rungs now wait for
sector 2 alongside Hot Patch.

## 0.13.195

**Every program you own now goes by a handle of its own, like `0xed7631`,
instead of its species name, so two Wintermutes on the roster are no
longer the same word twice.** Wild programs and summons keep their
species name, and a name you give a program still wins over its handle.
The handle is worked out from the program itself rather than stored, so a
save from before this release shows handles the moment it loads. Logs and
popups print the handle with the tier, species and sector around it. The
battle roster moves the tier to a tag after the name so the handle is
never cut off, and the CREW pane shows the handle and sector alone.
Rosters now sort by species, then by when each program joined.

**Memory files can now say how much a repeated event adds each time, and
how much of a memory weighs on mood rather than on opinion.** The two new
fields, `stack_decay` and `mood`, default to what every shipped memory
already did, so no number in the game has changed. A memory file with
either field out of range is skipped with a warning, as any malformed file
is. `docs/base-staff-interactions.md` describes every way base staff
behave, in plain English, for review.

## 0.13.194

**Researching Model Inspection teaches eleven routines that attack a
hostile's judgement on the battle map: pin its choices to the obvious one,
make them erratic, read its next move off the turn strip, turn it against
its own pack, or fill the ground with decoys it chases instead of you.**
A fight on a battle map has always been decided by a scored policy that
picks an action, then a cell to act from, and nothing in the game reached
that machinery — every routine spent its effort on Integrity, Power or a
status. These do the opposite: Cold Sample takes the randomness out of a
hostile's choice, Inference Probe publishes what it has already decided,
and the two together make the next turn a thing you can read rather than
guess. Prompt Injection flips which side it believes it is on for a turn,
and whatever it kills pays out as any kill does. Hallucination seeds fakes
only that side can see, and it walks to them and swings through them until
they are gone. Heat Injection and Prompt Injection catch your own
companions too, and a caught companion plays itself until it wears off.
Six more routines and two modules, Adversarial Patch and Attention Head,
come from the same node. The routines are battle-map only, and every file
says so.

## 0.13.193

**On the Work Orders screen, `<` and `>` move the highlighted order up and
down the queue, and the queue is worked from the top down.** Before, the
only say over what the base worked first was the High/Normal/Low choice
made when an order was filed, and changing your mind meant cancelling and
refiling. That choice and its `[P]` key are gone: a new order joins the
bottom, a research project's material bill still files at the top, and an
order that cannot be worked yet hands its programs on to the next one down.
Saves with orders filed under the old choice load unchanged.

## 0.13.192

**The box in the map's top-left corner now shows the research project the
base is working, above BASE STOCK — its data earned against what it needs,
or the material it is stuck waiting for, or "none" when a Research Node
stands with nothing selected.** Which project was running, and whether it
had stalled, used to be visible only by opening the research screen from
the base menu. A project that has all its data but cannot pay its material
bill names that material instead of a figure that has stopped moving.

## 0.13.191

**Researching Memory Mapping unlocks the Index Terminal: stand beside one
and press c to take from or put into every Depot in the base, wherever it
stands.** The transfer window used to reach only the Depots touching you,
so a base with its storage spread out meant a walk to each shelf. Machine
shelves and Quarantine Racks still have to be walked to, and each Depot's
own filter still decides what it will take.

## 0.13.190

**When a research project finishes, a "Research Complete" notice names it,
describes it and lists what it unlocks — and every notice now opens in a
panel three quarters the size of the window, with the map still in view
around it.** A finished project used to announce itself only as a line in
the base log, easy to scroll past while the party was somewhere else. And
notices took over the whole window. Now the panel is the same size for
every notice, and a long unlock list or payout wraps inside it instead of
running off the edge.

## 0.13.189

**When a fight on a battle map ends, a "Fight Over" popup lists the outcome,
the salvage and the XP over the board the fight ended on, and any key
returns to the map.** A tactical fight used to finish by switching to the
full-screen battle page the other combat model uses, so the board vanished
the moment the last blow landed. Now the board stays where the fight left
it, with nobody left to act and the camera held still, and the results show
at once instead of scrolling in a line at a time.

## 0.13.188

**What the base is holding is now a list down the top-left corner of the map,
with each item's full name and amount.** It used to be a single row of
two-letter tags across the status bar, like `[CG] 12`, which you had to learn
before you could read it. The list uses small type, cuts long names short with
`…`, and counts anything past half the map's height as `+N more`. It shows on
the surface and in base space. In the Stack it is hidden, because the frame
map sits in that corner, and it is hidden on a battle map too. An open menu
now covers it, where the old status-bar row stayed visible.

## 0.13.187

**On a battle map, a body that takes a hit flashes red and throws off sparks,
and a body that is healed gets a green `+` that bounces over its head.** A
landed blow used to show only as a number going down in the log and on the
turn strip, so in a busy fight it was hard to see who had just been struck or
patched up. A hit now plays the same red flash and debris a GC Entropy Sweep
plays on a structure, on the tile of the body that was hit, and a heal shows a
`+` that bounces twice and fades. A miss, or a heal on a body already at full
Integrity, shows nothing.

## 0.13.186

**When you aim an area routine on a battle map, a yellow outline shows every
cell its centre can land on.** Before, you moved the aim cursor blind and only
found out a cell was out of range or out of sight when the routine was
refused. The outline is worked out from the same range and line-of-sight rules
that refuse a bad aim, so it cannot promise a cell the routine will reject.
While it is up, it takes the place of the blue movement outline.

## 0.13.185

**On a battle map, a program that can already hit its target stays where it
is.** Enemies, and your own party on auto-attack, used to take a step almost
every turn even when they were already in range and could see their target,
so a fight read as everyone shuffling about for no reason. A body now moves
only when it has to: its target is out of reach, or something is blocking its
line of sight.

## 0.13.184

**Research nodes and routines now say in plain words what they do.** The
research menu described its nodes in the setting's jargon — Capacitance was
"Bank a whole reserve in one cell", Address Translation "Write to the
substrate's own page tables" — and several routines read as flavour rather
than an effect, like Clock Skew's "the nearest hostile pays the drift". Every
research node and 25 routines were rewritten to say what you get: what a piece
of gear is for, when a passive fires, how long a leak or a stall lasts.

## 0.13.183

**Your Home no longer draws as a bare `H`: it wears the same portal art as
the base's doorway out on the zone map.** The Home is the base's way in and
out, and the game already calls the anchor outside "the home portal", but
inside the base it showed as a plain letter that looked like nothing else you
had seen. It now uses the anchor's `#` and its sprite, so the two sides of the
same doorway look alike. In the icon designer that art is now one row shared
by both, so editing it changes both.

## 0.13.182

**On a battle map you can tell which enemy is the silver or gold one.** A rare
program wears a coloured bar along the top of its tile on the surface map, but
the moment a fight went tactical that bar vanished, so a silver or gold spawn
looked exactly like the ordinary ones beside it. The battle board now draws
the same bar, and the difficulty earmark in the tile's corner drops below it
the way it already does on the surface map.

## 0.13.181

**A program mending beside a Repair Bay wears its green `+` above its head,
not over its face.** The mark was drawn at the same height as the program's
own letter, so the two painted on top of each other: what you saw was a bare
green `+` standing next to the Bay with nothing recognisable under it, which
read as a mark on the building. The `+` still bobs, but it now rests in the
top of the tile, clear of the program it belongs to.

## 0.13.180

**Cutting rock in your base makes a sound now, and a dig crew cutting makes
it too.** Mining was the one thing you could do down there that the game
never acknowledged: a swing into the entropy played the footstep cue, because
a bump into a wall reaches the world as a step like any other and nothing
downstream could tell the two apart. Swinging now plays a short, soft chip
instead, and the footstep is dropped for that press rather than layered under
it.

What the ear counts is a **tick of cutting**, not a swing that lands damage.
That distinction is the whole of why a crew is audible at all: a crew spends
twelve ticks per swing where you spend one, so a cue on the damage alone left
your programs chipping once every six seconds while the bar on the cell they
were working moved the whole time. Holding a direction into a wall drives the
world at about eleven ticks a second and sounds like it; a crew working at the
world's own pace sounds twice a second. The difference between the two is the
clock, not a choice about the sound.

A crew laying floor over a cell it already cut stays silent — that is the
other half of the same job, and it is not mining.

## 0.13.179

**A program's sheet no longer carries the "zone 1 — you're in 2" line under
its name.** The line was there to warn you that a companion had fallen behind
the sector you were standing in, but the number it led with was already the
digit on the program's own name — "Drone 1" — so most of the row restated what
a glance up had just told you, and what was left was a subtraction it made you
do yourself. It was rewritten first to name what would close the gap, which
fixed the repetition without making the row worth the space, and then removed.

Nothing about falling behind has changed: a program's tier is still on its
name, and a Recompile Kernel still brings it current.

## 0.13.178

**A machine, a build site and a wall being cut now show how far along the
work on them is.** The base could tell you a machine was running and that
somebody was posted to it, but never how close it was to finishing — so a
cycle three ticks from paying out and one that had just started looked
identical, and the only way to tell was to stand there and wait. Every cell
the crew is working now carries a small bar along its bottom edge: a
machine's production cycle filling and turning over, a request being raised,
a marked wall coming down.

The bar wears the colour that cell is already using, so it says how far
without saying anything new. A stalled machine's bar freezes in the same
yellow its outline is already showing, which reads as the cycle being stuck
rather than as a second thing to work out. A post with nobody on it has no
bar at all, and neither does a guard's — nothing about standing guard
advances.

## 0.13.177

**On a battle map a routine can no longer be thrown through a wall.** An
attack has always needed a clear line to what it is aimed at, but a routine
only checked the distance — and since every area routine is thrown up to six
squares, that meant any blast in the game landed in full through solid cover,
yours and theirs alike. A blast is now refused outright when you cannot see
the square you are throwing it at, and nothing is spent: no Power, no
cooldown, and the turn is still yours. What it covers once it lands is
measured from where it lands, so it no longer reaches around a corner either
— though anything standing *in* the cover is caught by it rather than
sheltered.

A wedge or a line is unaffected. Those are aimed as a direction rather than at
a square, and they already stopped at the first thing they could not see
through, so aiming one past a wall simply makes it shorter.

## 0.13.176

**You can hand a battle map over and watch it play out: press A and everyone
on your side closes and attacks on their own.** A fight you have already won
on paper still cost a keypress a body a round, with no way to say "just
finish it" — so the easy fights were the tiring ones. Your side now moves at
the pace the wild side moves at and runs no routines at all, only basic
attempts, so nothing is spent that you were saving. Any key at all takes
control back mid-fight, and it switches itself off as soon as the fight ends
— a hands-off fight is one you ask for each time, and a fight that opens
while you are not looking is never resolved for you.

## 0.13.175

**The view now sits still on the body that just swung for a quarter of a
second after the blow finishes, instead of a twentieth.** The wait before the
next body moves was the same wait a body takes between arriving somewhere and
striking — long enough to cover the blow and the slide to whoever is next,
with almost nothing left over, so the view started travelling the instant the
flash cleared. That hand-over is now its own length and a longer one; the
pause between arriving and striking is unchanged, and so is the speed a body
walks at.

## 0.13.174

**In a tactical fight the view now stays on whoever is swinging until the
blow has landed, then slides to the next body.** It used to jump the moment
an attack resolved, because the turn is handed on inside the same step that
resolves it — so the streak, the flash and the damage all played out on a
part of the board you were no longer looking at. The view now waits out the
blow before it moves, and moves by panning rather than cutting.

**The squares you can still step to are outlined.** They were already
tinted, faintly enough that you had to know where to look to see it; the
tint stays, because it has to leave the ground underneath readable, and the
edge of the reachable area is now drawn as a line you can find at a glance.
Squares you cannot enter — the one an enemy is standing on, a blocked
one — show as holes in it.

**Walking into an enemy attacks it with your weapon.** Pressing a direction
at something in the way did nothing at all before, so the last step of a
charge you had spent the whole turn making had to be finished by opening the
attack menu and aiming back at the square you were already facing. The bump
is an ordinary attack in every respect, which includes ending your turn.
Walking into one of your own is still refused — hitting your own side stays
something you have to aim at deliberately.

## 0.13.173

**Two new routines fork a temporary program that fights beside you for the
length of one battle.** Fork Program fields one body; Fork Cluster fields two
or three, each a tier below what the first would have managed. Both work in
a group fight and on a battle map, and both are priced steeply in Power with
a cooldown behind them — a whole extra combatant for the rest of a fight is
among the strongest things a routine can buy.

A forked program is not a companion. It draws its species from the whole
catalogue, takes its own stat block and moves with it, and fights on your
side without being commanded — but it never joins your roster, takes no
gear, earns no experience, costs you no roster slot, and is gone when the
battle ends however that battle ended. Its kills still pay you.

**A new repeatable perk, Scheduler, is what makes forking worth investing
in.** Unscheduled, every fork comes out Ordinary — reliably below what you
could field yourself. Each rank widens the tier a fork may roll and raises
its ceiling by one, topping out at Prismatic after four; Fork Cluster stays a
rung behind Fork Program at every rank, so the trade between numbers and
quality appears the moment you buy into it and not before.

Modders get a new `Summon` ability effect — see `assets/abilities/README.md`.

## 0.13.172

**Some weapons can now be swung from across the battle map, and every blow
is drawn travelling to what it hits.** On a battle map a swing has always
meant standing next to somebody, whatever the weapon and whatever the program
holding it. Seven weapons reach further now — the Arc Lance, Scatter Lance,
Interrupt Coil and Monofilament Whip two cells, the Plasma Router, Siege
Compiler and Broadcast Storm three — and a weapon's own page states the range
it swings from, beside the row saying what a wide swing lands on.

A wild program with a ranged move reaches two cells as well, and will now
hold that distance instead of walking into arm's length first. Cover stops a
swing at range exactly as it stops an aimed routine, so a body behind a block
is out of reach until one of you moves; standing next to somebody is
unaffected, because there is nothing between you. A weapon replaces whatever
its wielder could do bare-handed, so a program that shoots two cells swings
at arm's length while it is holding a blade.

Every basic attack now draws a streak from the swinger to each body it lands
on, at any distance — a wide swing fires one at every body it caught. At one
cell it is a short flick across a single square, which is the melee feedback
the map never had.

## 0.13.171

**Guard Page reads `[GU]` on the base stock strip now, and the manual calls
the Gear Puller by its name.** Renaming the Harness Puller two releases ago
left two loose ends behind it. A Gear Puller Carrier put on a Depot shelf
tagged `[GP]` — the same two letters Guard Page had carried all along — so
the strip showed two piles under one tag and said nothing about which of
them was filling. A tag is derived from an item's name, and a carrier takes
the name of the tool inside it, so the authored item is the only one of the
two that can be told to use different letters; Guard Page took `[GU]`.

The manual's extraction page had also gone on calling the tool a Harness
Puller throughout, which is a thing the game no longer has.

## 0.13.170

**Demolishing a building now hands back everything it was holding.** A Depot
came down with its whole shelf inside it and the units were simply gone — no
warning, no line in the log, and no confirmation step outside the one
demolishing a Home already had. The same went for the stock in a machine's
buffers and for the programs standing on a Quarantine Rack. Demolition
already gave back a share of the build cost, a rig's fitted tool, and a
program a hauler happened to be carrying at the time, so what the building
was actually holding was the one thing it destroyed.

All of it comes back to your pack now, the ingredients waiting in a machine's
input hopper included. A rack's programs come back the same way, and when
there is no room left for them you are told which ones could not be taken
rather than losing them quietly. Demolishing a Home still brings every other
structure down with it, and now returns all of their contents too.

**A GC Entropy Sweep still destroys what it takes down**, and that is
deliberate. Losing a building to a sweep is a loss; only a demolition you
chose hands the contents back, which is the rule the build-cost refund has
always followed.

## 0.13.169

**Wild programs scale with your party now, in how hard they hit and how many
of them turn up.** The field only ever read the sector you were in and how
far out you had walked, so a party that had levelled through a sector met the
same programs on the way back out that they had met on the way in, and a full
five-strong party met exactly the pack a lone runner did. What a spawn takes
now includes how far through the sector's own level band you have climbed and
how many companions are standing with you — and because the band resets when
you breach, the ground you have just arrived on still fights like the
newcomer you are.

Each program rolls its own share of that, so the field is a *spread* rather
than a mirror. The easy fights stay on the map and harder ones appear above
them, which is the difference between the world getting more dangerous and
the world simply refusing to let you get stronger. The ramp out from your
spawn point is unchanged and adds on top, as does the enemy-strength setting.

## 0.13.168

**The Teardown perk says what it actually does.** Its line in the picker read
"+1 salvage per kill", which stopped being true when a defeated program
started leaving a body to strip instead of dropping material on the spot. The
bonus has paid out at extraction ever since — a unit per level out of every
teardown, on whatever tool is fitted, at your own hands and at a Teardown Rig
alike — but nothing said so, so buying it for the kills was buying it for
something that no longer happens. The perk has also moved from Fieldcraft to
Workshop in the picker, beside Keen Scavenger and Lean Compiler, which is the
company it now keeps.

The manual's perk page said the same wrong thing and has been corrected, as
has the perk catalogue in `docs/`, which was additionally a release behind on
Lean Compiler's price.

**The Harness Puller is called the Gear Puller**, and several perk, tool and
sortie descriptions have been rewritten to read better.

## 0.13.167

**A contract now tells you how and where it is finished.** "Deliver 4 Core
Fragments" never said that a delivery is a keypress at the counter the job
was signed at, so compiling the items and putting them on a shelf did
nothing and looked like the job was broken; "Reach sector 3" named no gesture
a player could go and make, since *reach* is a verb the game does not use
anywhere else. Every job carries a third line now, under the flavour text,
saying how it is satisfied and where — and for a delivery that line goes
live when you reach the counter, naming the town or your own Broker and
counting what is in your pack. The two objective lines that used words the
game does not speak now read "Breach to sector 3" and "Carry 4 Core Fragment
at once".

Walking up to the counter with cargo a held job wants also raises a row in
the info column, so a delivery you could make right now reaches you without
opening anything. And the manual has a Broker page: there were fifteen help
pages and not one of them was about contracts, which is most of the reason
the screen was the only place the loop was ever explained.

**The board can ask for eight things it never could.** Clear nests, hold off
a GC Entropy Sweep without a scratch, bring a squad home from a sortie, trade
at a settlement, break down a downed program, beat a Stack's guardian, finish
a research project, and get a town onto good terms with you. Jobs used to
come in four shapes — kill some, fetch some, build one, go somewhere — so
most of what the game has was something the Broker could not name at all,
which is most of why a board read as filler. A deed job can also ask for
more than one of something now, so "clear three nests" is a job rather than
three separate ones.

Eight new contracts ship using them, and seven existing descriptions have
been rewritten: they used to end by restating the line directly above them,
which spent the one place a job can say *why* on saying *what* a second time.


## 0.13.166

**Breaching into sector 2 now warns you that sweeps start here.** A GC
Entropy Sweep cannot reach your base in the opening sector, and nothing said
so — the first thing most bases learned about the garbage collector was a
structure already losing Durability, with a Shield sixteen Core Fragments
away and nothing built toward it. The breach screen is followed by a second
one now, naming what a sweep does to a base and the two things that answer
it: a Shield, which soaks damage off every sweep against every structure you
own, and a Patch Node, which writes damaged structures back up across the
whole base on its own.

It fires on the breach that lands on the first swept sector and on no later
one, and it is read off the same setting the sweeps themselves are gated on,
so the two cannot come to disagree about which sector that is. Unlike the
other first-time notices it is not latched across runs: a new run starts in
the unswept sector again, so a second playthrough is warned again.


## 0.13.165

**Upgrading a structure is now done by pointing at it.** Pick "Upgrade a
structure" off the base menu and then press a direction — the machine
standing on that tile, north, south, east or west of you, goes in for its
next tier. It used to open a list of every upgradeable structure within forty
tiles, so ordering an upgrade on the Compiler you were standing in front of
meant finding its row among them. The menu row itself is unchanged: it is
offered wherever anything nearby can climb, so aiming at bare ground tells
you the tile was empty rather than doing nothing.

**The bill moved to the screen that knows which machine it is for.** The list
quoted each structure's materials down its rows; a prompt aimed at a tile
cannot, so the program picker that confirms the order now says what your crew
will fetch, counted against your pack and the base's shelves together. Aiming
at something that cannot climb — Home, or any structure with no upgrade path
— answers in the same words it always did, and spends nothing.


## 0.13.164

**Deploying a Shield, a Patch Node, a Repair Bay or a Relay no longer costs
you a program.** Filing a build order used to commit one tamed program to
almost everything you could put up, including the eleven structures no
program is ever posted to — so defending or repairing the base cost a body
that then had nothing to do inside it. Eleven structures are now free of it:
Shield, Patch Node, Repair Bay, Relay, Data Cache, Sandbox, Recharger Node,
Log Analyzer Bay, Line Driver, iso Market and Contract Broker. Materials, the
crew who raise them and the ticks they take are unchanged.

**A build costs a program exactly when the structure runs a job**, which is
what the nodes, the benches and the Teardown Rig do — the body is spent on
the machine a body will afterwards be posted to. Depots, the Quarantine Rack
and the Zone Portal were already free, and are now free for the same reason
rather than each for a reason of its own. Structure mods get this without
authoring anything: a decorative or defensive structure of your own costs no
program because it runs no job, and the flag that used to say so is gone.


## 0.13.163

**Every research node now says what it hands over before it says what it
is.** Both research views — the list on `T` and the flow chart on `G` —
draw one cyan `Unlocks:` line above each node's description, naming the
structures, items, routines and tools that node gives you. Working out
which node unlocks a particular bench used to mean reading thirty-four
paragraphs of prose, because the prose was the only thing that said so.

**That line is derived from what the node actually hands over, so it cannot
go stale.** A node repointed at a different structure re-words itself, and a
modded node gets its line without authoring one. Twenty-odd descriptions
were saying the same thing in a sentence of their own and now say what the
node *is* instead — which bench a recipe is compiled at, what a tool does,
and that a routine still has to be written onto a Routine Disk before you
can run it.


## 0.13.162

**`FERAL_DEV=1` adds a dev-console row that makes the world fight harder.**
The game only ever scaled a wild program one way — by zone, by how far from
home you were standing, and by Stack depth — so trying a run against a stiffer
world meant editing tuning constants and rebuilding. The console on `` ` ``
now carries a five-rung ladder, Standard through Critical, and a press
re-stocks the wild around you, so the change is felt where you are standing
rather than at the next spawn.

**A rung is worth half a zone, not a flat multiplier on stats.** A band means
"the ground fights like it is N zones deeper", and means the same thing at
zone 1 as at zone 9. A flat multiplier would not: every difficulty curve in
the game is linear, so x1.5 is half a zone at the start of a run and three
zones by the end of one.

**Nothing changes for a run on Standard.** The ladder has no rung below the
curve the game shipped with, Standard is exactly zero steps, and a save
written before this loads on it. `save::SAVE_FORMAT_VERSION` is untouched.


## 0.13.161

**The release notes are written for someone who does not have the source
open.** From `0.13.58` an entry was a single bold sentence and nothing else,
and those sentences named engine internals — a want, a bill, a seam — so a
reader outside the code could not tell what had actually changed. An entry
now says what is different from the outside, in the game's own words, and
then what was wrong before. The ten most recent sections are rewritten under
the new rule. Nothing in the game changed.

## 0.13.160

**A research project can finish on a small base again.** With one program and
one Research Node, the program stood at that node forever: once a project's
progress was full the node went on asking for a worker anyway, and that
request outranked the material orders the project had just filed — so the
materials were never built and the project could never complete. Abandoning
it, demolishing the node or finding a second program were the only ways out.
A node with nothing left to earn now lets its worker go.

**A project waiting on materials says so, and a full node stops repeating
itself.** A project short of something now names the item on the base panel
and points at `b`; before, a stalled project looked exactly like one quietly
working. A Research Node with nowhere to put what it extracted had been
logging "extracted 0 Research Data" every few seconds.

**The research flow chart says what the list says.** It was reading a stored
figure this release stopped keeping, so it showed neither a project's progress
nor the reason a node it was offering would be refused, and it printed its
"you can't take this" line under all thirty-four rows rather than the selected
one. Separately, the base menu had been rebuilding the whole research tree
every frame just to decide whether to offer the row.

## 0.13.159

**Research is something the base works on, not a bank you spend from.**
Picking a node makes it the run's one active project: every Research Node you
have deployed staffs itself and feeds it, the materials it needs are filed as
high-priority work orders and paid out of the base's own shelves, and picking
a node whose bill names something your base has no way to make is refused on
the spot. `A` abandons a project and keeps the progress it earned. Research
Data banked under the old system is written off when the save loads — the save
itself keeps loading, with no format bump.

## 0.13.158

**The research flow chart scrolls, so a tree bigger than the window can still
be read.** The view follows the cursor instead of trying to fit everything at
once, and each box is sized off the text in it rather than by dividing up the
pane.

**Every arrow between nodes gets its own lane.** The nine links feeding the
deepest tier were drawn on top of one another and read as a single thick bar
with stubs coming off it.

**A box at the edge of the view keeps all four sides.** The top row drew with
no top edge and the leftmost column with no left edge.

## 0.13.157

**The research flow chart is legible over the map.** It painted no backdrop of
its own, so the world showed through every box and line and nothing on the
tree could be made out.

## 0.13.156

**The research tree can be read as a flow chart.** `G` on the research screen
swaps the list for the tree drawn left to right: a box per node in the colour
its row already uses, elbowed arrows between them, and the prerequisites of
whatever is selected lit up. Arrow keys walk it, `Enter` buys, and the detail
panel down the right is built from the list's own rows, so the two views
cannot describe the same node differently.

**A research node that requires itself, round a loop, is dropped when the game
loads.** Both nodes in a loop like that can never be researched — the same
condition a node with a missing prerequisite was already dropped for.

## 0.13.155

**The deploy menu says how many of each structure you already have.** The
count sits between the name and the cost, and a row you have none of carries
nothing. It is the same figure the deploy limit is measured against, so a row
showing a count you have already filled is the one the deploy is about to
refuse.

## 0.13.154

**Two weapons hit more than one target.** The Scatter Lance sweeps a whole
enemy group and the Broadcast Storm sweeps every enemy on the field. You
attack the way you always attack; the swing is wide when the weapon's charge
is up.

**Both rate lower on the swap screen than the weapon they replace, and `[I]`
says why.** The rating prices a single swing, so a wide weapon reads weaker
there than it plays; the inspect page spells out what the swing lands on and
how often.

**On a battle map a wide swing catches your own side.** A companion standing
next to the enemy you aimed at is hit too, which is what makes a shape worth
aiming.

**Fixed: a round could end twice.** A swing that wiped out the last enemy
group and killed its own attacker left the end-of-round bookkeeping tearing
down a fight that was already over.

## 0.13.153

**A Teardown Rig holds its own tool.** Walk up to a rig in base space and
press `F` to fit a tool carrier out of your pack; `R` takes it back, and a rig
with nothing fitted does nothing. Before this, every rig stripped with
whatever tool sat in the player's own slots — so taking that tool out to use
elsewhere silently starved every rig in the base.

**Feeding a rig is the base's job now.** Put downed programs in a Quarantine
Rack and a posted worker carries them across, which leaves the store you open
with `D` as your own hands and nothing else.

## 0.13.152

**The Quarantine Rack stores downed programs.** A new base structure for the
carriers you have no room for, filled and emptied through the same `c`
transfer screen everything else moves through. A worker posted at a Teardown
Rig that has run dry walks to a rack within reach and brings one back.

## 0.13.151

**A fight reads as a break-in now, not a bar brawl.** The nouns were already
right — Integrity, routine, ICE, decompile, jack out, the Grid — but every
verb around them was ordinary melee: you tore a routine clean through for
damage, it glanced off, a body braced and recovered its guard. A swing is an
exploit attempt and Integrity is what refuses it, so an attempt lands, lands
unchecked, fumbles or is refused, and the number it moves is Integrity rather
than "damage".

**Bleed is a leak, Stun is a stall, and Exposed is unpatched.** The status
chips read `Leaking (3)` and `Stalled (1)`, and the sixteen ability
descriptions that named a status by its old word follow them. Modded ability
files need no editing: the words inside a `.ron` file were never the words on
screen.

**Two labels stopped disagreeing with the prose beside them.** The battle
table and the roster pane printed `HP` where every sentence in the game says
Integrity and the HUD strip already said `INTEG`, and one gear row on the
manifest said `DEF` where everything else says `MIT`.

**The manual's combat pages use the same words the fight does.** They were
still explaining a critical hit, a plain hit, a fumble and a miss. Mining
keeps its swing — hitting rock is what that word is for.

## 0.13.150

**A routine can take one body off the scheduler, and nothing can address it
until it acts.** Detach Single — taught by the new Process Detachment
research node — cloaks one ally: no target roll names it, no group hands it
forward as a front, and no swing on a battle map may aim at it. It is not
invulnerability, deliberately: an area routine whose shape covers its cell
still lands, the body is still a wall nothing may walk through, and the cloak
ends the moment it swings, runs anything aggressive, or takes a hit — but not
when it moves, braces, mends an ally, or is missed.

## 0.13.149

**A machine's description reads ingredient-first, and the research tree
states the conversion a node buys.** Four bench descriptions named their
product before their ingredients, the Transcriber carried a "from into"
typo, and the Armory named neither — a census now holds every assembling
structure to naming the recipe's ingredients and then its product, checked
against the recipe itself. In the research menu each node draws a cyan line
per conversion it unlocks, quantities included, derived from the recipe
rather than authored beside the description.

## 0.13.148

**The argument behind every load-bearing seam now lives in the memory graph,
and `docs/seams.md` is gone.** All 331 entries moved across as `seam:<slug>`
entities, verified byte-exact against the file before it was deleted; the
`seams` skill carries the two calls that reach one. The seventeen doc
comments that cited the file by name — sixteen in `crates/`, one in
`assets/species/README.md` — now name the seam they always meant instead.
No behaviour changes.

## 0.13.147

**A Power Cell has tiers now, and one line of them drips instead of
lumping.** Four consumables behind two research nodes: the Buffered Cell and
Capacitor Array restore more in a click and keep a supplier lit for however
many upkeep windows they declare, while the Sustain and Backfeed Cells
restore less and then feed Power back on a cadence — worth more in total than
a lump, but only to someone actually spending it. A supplier burns the
cheapest fuel it can reach, so the base keeps eating the staple it can make
in bulk and the dense cells stay worth carrying into the field.

## 0.13.146

**The reach wash is drawn for either side.** The tint showing where the body
whose turn it is may still step was gated on the turn being the player's, so
a fight showed where the party could go and never where the wild side could —
which is the half of the board a fight is planned against.

## 0.13.145

**A builder fetches the line it can get.** A build request whose bill led
with something the base could never supply held a program at the site
forever, delivered nothing under it and said nothing about any of it — which
on a Zone Portal, whose portal fragments only a Stack guardian drops, read as
the whole base having quietly stopped working.

## 0.13.144

**A hostile walks its approach one cell at a time.** A body's whole walk was
committed as a single placement, so one with room for six cells crossed all
six between two rendered frames and then swung — a teleport rather than an
approach, in the one combat model whose entire mechanic is where everybody is
standing. The cell it picks is still one decision; what is now spent a step
at a time is the path to it, through the same door the player's own arrow
keys go through. A body waits a readable beat before it sets off and after it
strikes, and covers ground at nearly four times that rate in between, so a
long approach reads as an approach without taking five seconds to watch.

**Every structure describes itself in plain English.** Thirty-six catalogue
entries rewritten from prose to what the thing does and how to use it.

**The zone portal is cheaper by two ingredients**, no longer asking for patch
routines or hardened shells on top of its portal fragments and routine disks.

**The Depot Mk3 is buildable again.** The rewrite above took its `glyph` line
with it, so the file stopped parsing and the structure dropped out of the
catalogue — never released in that state, and caught by the asset census.

## 0.13.143

**A body on a battle map can brace.** The board had attack, special and end
turn; the intrusion screen has had Defend on `d` since long before there was
a board, and a fight you can only spend a turn swinging in has no answer to a
turn you would rather survive. It is the group model's own `begin_defend`
rather than a second spelling of it, so the mitigation and the line stay one
definition — but only the mitigation crosses over: `DEFEND_AGGRO_WEIGHT`
weights an aggro slot and a board has none, so bracing is a survival play
here and not a tank one. It lasts the rest of the round, which makes it worth
most to whoever acts early and nothing at all to whoever acts last; the turn
strip is on screen, so that is a thing to read before spending the turn.

## 0.13.142

**A companion's turn on a battle map now reads as the player's.** The keybar
said "the wild side is moving" and drew no action keys and no movement wash
while the fight sat waiting for that companion to be moved — one predicate
too many, gated on the player's own body where the engine's is gated on
hostility.

## 0.13.141

**A battle map says whose turn it is.** An arrow hangs over the acting body's
head and bounces — blue for one of yours, red for one of theirs — because the
turn strip in the corner names the order and is the wrong place to look when
the question is where you are. It hangs above the tile rather than in it, so
it spends none of the three channels a tile already has: the con earmark's
corner, the HP bar's edge, and the glyph in the middle it points at. The
bounce is the raised cosine the base's staffed mark and build caret already
ride.

**A fight on a board takes the keys the abstract fight already taught.** `s`
opens the routine picker where `r` did, matching `[s]pecial` in the intrusion
screen's own action list, and the numpad steps the acting body all eight ways,
corners included.

**The aim cursor gets its own keybar.** It was showing the board's three
action keys, which do nothing while it is open, and neither of the two that
commit and cancel it.

**`assets/help/28-battle-maps.md` is the manual page for a board.** Nothing in
`assets/help/` had heard of a fight on a map.

## 0.13.140

**A staged fight can be fought on a battle map.** A `dev-arenas` scenario now
names which of the two combat models fights it, and the `arena` bin resolves a
tactical one headlessly — the only way to watch a fight on a map resolve
without a display, and the phase the tactical spec deferred until the mode
worked. `dev-arenas/tactical-full-group.ron` is `full-group.ron`'s fight on a
board, the same party and pack and seed, so the pair is the comparison: 4.0
rounds at 98% player HP against 7.3 at 93%, over 50 reps each.

Both sides are driven by the game's own AI, and a party body swings without
invoking — which is what `PartyPlan::AllAttack` already does in the group
model, so the two sets of numbers stay comparable. An `approach:` row seats
the pack on a bearing, since a staged fight spawns its opponents around the
player and so has none to read.

**A tactical scenario is bin-only.** The played arena drives rounds and a
battle map is driven a body at a time, so `[F]` on one answers with a line
naming the bin rather than opening a screen it cannot play.

## 0.13.139

**A fight can be fought on a map now, and the toggle is off by default.** The
game's first Options screen carries one switch; with it on, a pack met on the
open ground opens a grid where every body has a cell, a move allowance and a
line of sight, instead of the abstract group screen. Everything else — a nest,
a lair, a guardian or a patrol giving chase, the arena — stays abstract, and
which model a fight opens in is decided in one place.

The two models share their combat: the same ladder, bands, mitigation,
affinity, Power and cooldowns, through the same `use_ability`. What is new is
the space around them — a derived battle map with cover and rough ground, a
move-then-act turn order, routine shapes and ranges, full friendly fire, and a
hostile that decides what it will do before it decides where to stand.
Abilities that name no shape get one derived from what they target.

Off by default and opt-in on purpose: a grid fight spreads damage across every
body where the group model forces focus fire, so it hits harder and swings
wider than the same pack fought abstract, and `balance_sim` cannot see it.
`--template tactical` opens a save one keypress from a fight for comparing the
two.

## 0.13.138

**`render/base.rs` was three unrelated things wearing one name.** Its biome
and terrain painting primitives are now `render/terrain.rs`, the per-tile
marks a program or machine wears are `render/marks.rs`, and the message log
history screen — which had nothing to do with the base map and was simply
misfiled — is `render/history.rs`. What is left in `base.rs` is the three
functions that drive the map, down from roughly 1,940 lines of code to 1,198.

No behaviour change: every item moved verbatim, and the only edits are the
visibility keywords items needed once they crossed a module boundary. The
drawing seam is untouched — none of the three new files names a graphics
library, which stays `paint.rs`'s alone.

## 0.13.137

**`Game::load` and `Game::save` now name their sections instead of spelling
them out inline.** Loading a save was one 598-line function and writing one
was 391, so the two halves of a save section — the code that writes the
structures and the code that reads them back — sat hundreds of lines apart
with nothing naming either. Fifteen helpers now pair them off:
`spawn_player_from_save` against `player_save_for`, `restore_structures`
against `structure_saves_for`, and so on through nests, dig sites, build
sites, caravans, sorties, routes and cronjobs. `load` is down to 270 lines
and `save` to 134.

No behaviour change: the bodies moved verbatim, and the wall of
`insert_resource` calls stayed where it is, being length without complexity.

## 0.13.136

**A Zone Portal no longer costs a tamed program to build.** A Portal is
despawned the moment it is walked through, so the committed body died with
the doorway — and the build quality it bought is worth nothing on a structure
that runs no job.

The exemption is now a flag a structure sets for itself, `costs_no_program`,
rather than something read off `stores`: the shipped shelves and the Portal
declare it, and `stores` goes back to meaning only that a hauler may empty
into it.

## 0.13.135

**A Depot no longer costs a tamed program to build.** Every structure but
the Home committed one when the order was filed, which priced a shelf at a
body — so anything that declares `stores` is now exempt alongside the Home,
the whole researchable ladder from the first Depot to the Mk6 included. The
build is otherwise unchanged: it is still filed as a request, still paid for
in materials, and still raised by the crew.

The exemption is read off the structure's own `stores` flag rather than a
list of ids, so a mod's storage building is exempt for free.

## 0.13.134

**A test-only helper was being compiled into the game.** A scripted edit in
0.13.133 landed a new function between `#[cfg(test)]` and the one it
guarded, so `drift_idle_staff_for_test` shipped in the release binary and a
plain `cargo run` warned about it. No behaviour change.

## 0.13.133

**A program in a bad mood now takes itself somewhere to sit.** Instead of
wandering the base at random until its grudges faded, one far enough down
the ladder walks to the nearest amenity — a Sandbox, a Log Analyzer Bay,
anything with a `services:` list — stands there, and comes away fond of the
place, which is what carries its morale back up. A mild sulk leaves the
labour pool while it does, so building the first amenity is what turns a bad
mood from a posting restriction into an absence; a base with none keeps its
programs on the line, working through it at machines they do not resent.

Two older scheduling bugs turned up alongside it: a body that went off
shift, downed tools or broke off for repairs while the base's only
instruction was a *standing* job kept that posting for the rest of the run.

## 0.13.132

**Storage is now something a run earns rather than something it has** — the
Depot holds 50 instead of 200, and five research nodes stand up five more
rungs behind it, each holding twice the one below and gated a sector deeper.

## 0.13.131

**A program with nothing left now rounds on a colleague** — past refusing to
work there is a fourth rung, where a miserable base program picks whoever it
likes least within three tiles and brawls with them for four to eight beats.
Nobody dies; whoever comes out worst breaks off for repairs, the one that
started it comes away calmer, and the one that got hit remembers exactly who
did it. There is no key and no screen — the only way to see one is to run a
base badly enough that a program has nowhere else to go.

**An unmet need now moves a program's mood even when the base never had the
building** — the complaint blames nothing and names no tile, which is what
lets it count at all, so a base that never builds an amenity is no longer free
of the consequences of not building one.

**While a base is still getting started, needs do not count against it** — it
takes eight staff and eight structures before either grudge is written or any
tantrum can open. The lines are still said, because the line is the errand.

## 0.13.130

**A Depot can be told what it will accept** — `[F]` from the transfer screen opens one shelf's allow/deny list, an arrow per row, `[A]`/`[D]` for the whole catalogue and Tab between the Depots touching you, so two Depots with opposite lists sort the base between them; a refused load is carried to the next Depot instead, and back to the machine it came from if none will take it. A filter says only what may come **in** — what is already on the shelf stays there and can still be taken, and a refund the base is handing back is exempt, so a closed shelf can never destroy materials while you are out in the field.

## 0.13.129

**Research costs goods now, not just Research Data** — every node in the tree carries a material bill authored in its own `.ron`, ramping from raw Core Fragments at the roots to Trace Sniffers, Hardened Shell and Routine Disks at the deep end; the bill is paid from your pack, topped up off the shelves of anything you are standing beside, and refused whole rather than part-spent, so a Research Node running unattended is no longer the whole of the tree. A census holds every bill to what that node's own prerequisites can make, which is what found Weapon Fabrication unlocking a Fabricator it had no way to feed — it requires Routine Fabrication now.

## 0.13.128

**No routine is free to run any more** — Patch Single v1.0, Bastion Single v1.0, Bit Rot Single v1.0, Hyperthread Single v1.0 and Hard Lock Single v1.0 had inherited a Power cost of nothing when the field was renamed, and are now priced off their own ladders; Decompile pays a token 1, since it is the one routine with no cooldown to throttle it. Pricing Patch Single v1.0 is also what puts it on the map's routine list, where a heal has to be priced to be offered at all.

## 0.13.127

**Reaching a sector's level cap now announces itself** — the notice says the XP still banks into Perk Points, and that a breach is what lifts the ceiling, tiers up what your structures can become, and may open research closed to you here.

## 0.13.126

**A program now carries two build aptitudes, and the machine it is spent on runs at what it was worth.** Assembly decides how well it puts a bench or a rig together; Extraction decides how well it puts up a node. The one it does not have is the one that machine does not care about.

**A good builder leaves a machine faster than the def ships and a bad one leaves it slower** — both directions, and permanently: the figure is baked in the tick your crew finishes and never moves again, except on an upgrade, which replaces it with the new program's.

**The build picker quotes the machine you would get, in ticks, before you commit.** Each row names the aptitude that build reads, the rung the program sits on, and the cycle it would leave you — "Fabricator cycle 20 -> 18 ticks". A structure that runs no work cycle says so once, above the list, and stops pretending the choice matters.

**The picker is sorted best-first by the roll that build actually reads**, so the same roster comes out in a different order for a node than for a bench.

**Neither aptitude touches how a program fights, and the overall Potential figure still folds only the four combat rolls.** An Excellent fighter can be the worst builder you own, and the manifest and the roster now name both rungs so you can tell before you spend it. The program page paid for those two rows out of the MOVES box, which now shows one move and a "+1 more".

**A pre-feature save loads every program and every machine at neutral**, so nothing already built gets slower and no save needed a format bump.

## 0.13.125

**Deploying a structure now costs a tamed program, spent for good the moment your crew finishes it.** A picker closes out every build order and every upgrade, asking you to commit one program off your roster — deploying takes any of them, and upgrading a structure a tier takes one from at least that deep a zone, so a zone-5 program can raise anything while a zone-1 program only manages a fresh Mk1.

**Home is exempt, at every tier.** A fresh run owns no programs at all, so a founding that cost one would be unfoundable, and an upgrade to your own Home never asks either.

**The picker only offers what you can actually spend.** The program in your hand, one away on a sortie, one already down, and one carrying goods are all left off the list — and a build order can never take you to zero, so a base down to its last program has both build menus greyed and says which rule stopped it rather than refusing after you commit.

**Calling off an order gives the program back whole** — same name, level, gear and memories — and so does an upgrade order whose machine is destroyed out from under it, by a raid or by your own hand. Only a finished build spends the program for good; deconstructing what your crew already raised returns materials, never programs.

## 0.13.124

**The swap picker rates what you are already wearing.** Candidate weapons have carried a power rating since 0.13.69, but the heading naming the piece in the slot carried only a name and its stats — a column of figures with nothing to compare them against. The heading is now a row of the same shape as the rows below it, its rating in the same column.

**A long worn name no longer pushes its stats off the edge of that heading.** The line was drawn unwrapped, and the widest copy the shipped assets can build ran some thirty cells past the popup body.

## 0.13.123

**Decompile is welded into the slot it starts in.** It cannot be cleared to make room, it cannot be written onto a blank disk, and no program can be made to run it — the routine panel marks its row as fixed, so this is not something you find out by pressing it. It is how you take a program at all, and there is exactly one of it for the length of a run.

**A new run now knows no routine until it learns one.** Decompile used to sit in your known set purely so that popping it out could be undone; with the pop-out closed, the only thing that entry still did was offer the run's one unduplicable routine on the etch screen. The routine you pick at creation is the first thing you know, and the party menu's "Etch a routine disk" row stays hidden until you know something worth writing.

**A save made before this keeps whatever it has.** A run whose owner had already popped decompile out still has the empty slot, and any decompile disk left in cargo is now inert.

## 0.13.122

**A patch routine now runs out on the map, not only in a fight.** Anything that restores Integrity and costs Power — Patch Single v2.0 and v3.0, Patch Party v1.0 and v1.1, and all three Rollback tiers — is offered from the routine list under `a`, on top of still being a Special. It charges the same Power to whoever runs it, spends a turn, and repairs exactly what it would have repaired mid-fight, scaled to that program's own level and Healing affinity.

**It refuses when everyone it would land on is already whole**, before the Power is taken. In a fight a wasted turn is a real choice and the round advances anyway; out here declining costs nothing, so there is no reason to let you spend a reserve on nothing.

**Patch Single v1.0 stays a Special, because it is free.** A cooldown counts battle rounds and there are no rounds out on the map, so Power is the only thing pacing a field routine — and one that costs nothing has nothing pacing it at all. Powering down still mends the party completely; patching in the field is for being a long way out with no charge left to burn.

**Several reference pages were quoting numbers the game stopped using, and now they cannot.** The Zone Portal was listed at 10 Portal Fragments when it costs 24 and three crafted items besides; the Overseer's attack was overstated by half; every ordinary program's second routine was listed as unlocking at level 6 when it arrives at 4; and the Backplane was still going by its old name throughout the roster page, which quietly rendered its whole column as though nothing lived there. Six of the seven generated pages are now diffed against the assets they describe, field by field, and the check that does it ships alongside them.

## 0.13.121

**The base staff now keep a Recharger Node and a Line Driver fuelled.** A supplier with no Power Cell within reach is a want of its own, and a program walks one over from a shelf — where before, the only way a supplier got fed was you hand-stocking a Depot on the tile beside it and remembering to top it up.

**It is fetched before the node runs dry, not after.** The want opens the moment the last spare within reach is spent, which leaves a whole hundred-tick window for the trip, so the Grid should not go down at all while the base is holding cells.

**Feeding a supplier outranks every work order, and nothing else.** A pending build still comes first; on a base short of hands this means a machine stands idle while the lights stay on, which is the trade — a dry supplier takes the whole Grid down with it.

**A supplier spends the cell in its own hopper before the buffer beside it**, so a node a program has just stocked stops drawing down a shelf the rest of the base is spending from too.

## 0.13.120

**Ground that costs you Integrity now says so.** A step onto Null Sector or Backplane took a bite out of you in silence — the crossing line fires only when the biome *changes*, so every step after the first one inside a patch was damage with nothing to explain it, and a death by ground read as dying at random.

**An attriting step sounds like taking a hit** rather than like walking, which is the same cue a swing that lands in battle plays.

**A condition claims patches of a sector rather than the whole sector.** Null Sector and Backplane together are about three quarters of the walkable map, so a condition that claimed its biome outright made attrition the default state of the world: three steps in four cost Integrity, and death from full arrived in about thirty of them at any level. It is a fifth of the map now, in blotches large enough to see coming and walk around. An existing save picks this up the moment it loads — a condition is derived from the world seed and the zone, never stored.

## 0.13.119

**The status bar carries the base's grid at all times, as `[GRID] draw/supply`.** It turns amber the moment the base cannot cover its own draw, and it rides the fixed left block rather than the elastic centre, so a crowded base drops a stock pile to make room and never drops the grid.

**A supplier that has run out of Power Cells now says so in its own words.** "Out of fuel — no Power Cells beside it", where it used to borrow a machine's "starved — nothing is feeding it" and send you looking for an upstream node and a program that a Recharger Node does not have. It reads that way in the log, in the `B` roster and on the `i` sheet.

**A dry supplier turns red on the map.** The one machine state that wears one — it is the cause every dark machine on the base is an effect of, so it is the tile worth walking to.

**A dark machine no longer tells you to build a Recharger Node.** That is the wrong move on the base most likely to be reading it: one whose Rechargers are all standing there dry, where a fifth would go dry beside them.

**A Power Cell now runs a supplier for 100 ticks rather than 20.** Keeping the grid fed stops being a production rate you have to keep pace with; what it costs now is putting a stocked buffer where a supplier can actually reach it, which is an orthogonally adjacent tile and nothing further.

## 0.13.118

**`FERAL_DEV=1` opens every dev tool at once** — the console on `` ` ``, the
arena row and the sprite forge — and a single flag still works on its own, so
`FERAL_DEV=1 FERAL_DEV_ARENA=0` is how one is left out.

**The battle log and the map reveal stay outside the master switch**, because
`FERAL_DEV_LOG` writes a file nothing rotates and `FERAL_DEV_REVEAL` changes
what the map shows you while you play.


## 0.13.117

**A town can grow into a city while you are off doing something else.** Every
Server on the map is due to become a Mainframe somewhere between tick 3,000
and tick 12,000, on a date fixed by the world seed and the region it stands
in — so it is the same date every time you load, and two towns in one world
are not due together.

**Trading with a town brings that day forward.** Every 150 Credits moved
across its counter, in either direction, pulls its date 25 ticks earlier, up
to a cap of 2,500. The cap is deliberately less than the earliest possible
due date: no amount of money founds a city on its own, it only ever gets you
one sooner.

**A city you have stood in tells you when it happens**, once, and the map
starts drawing it `M` on the spot rather than waiting for the next load. A
place you have only heard of gets a line in the log and does not take the
screen.

**A city you neglect or anger thins out, and never stops being a city.** Its
shelf runs between the 6 rows a town draws and the 14 a busy city does; left
alone it drifts down a little every 600 ticks, and a city that is Hostile to
you drifts three times as fast. Nothing turns a Mainframe back into a
Server — the glyph, the name and the label are permanent.

**A city you have never traded with holds where it is.** Neglect is
something you have to have been present for, so the drift cannot take a city
below its starting size until you have done business there at least once.
Making one Hostile counts as having been present.

**A city's page now says how it is doing** — Starved, Steady or Thriving, one
row, and only for a city. A town has no such row, because a word that never
changes says nothing.

**Every city already standing in your save is now Steady, which is 10 shelf
rows and a 25% share of standout stock.** They previously drew 14 and 35%
flat; 14 and 35% is now what a city you keep in business works back up to.
That is the one balance change here that touches saves you already have, and
it moves in both directions from where you leave it.

**Older saves load unmodified.** Every new field is additive, so there is no
save-format bump and a save from `0.13.116` opens with every town at the
neutral middle, which is where a fresh world starts them anyway.

**Modders: `kind` in a settlement file is now the kind a place *starts* as.**
Authoring `Server` no longer means it stays one, and authoring `Mainframe`
means it begins at the middle of the range rather than at the top of it.

## 0.13.116

**A program's manifest now says what it feels and what it remembers.** Every
program on the roster — party members and base staff alike — heads a MEMORIES
box with its mood and names the two strongest things behind it, so the sheet
you open to find out what a program *is* finally answers that too.

**Mood reads as a word and a number, and both screens say it the same way.**
The `R` page's header changed from `Morale -14` to `Mood bitter (-14)`; the
bands are anchored to the point where morale stops shifting extraction, so a
retune of that term moves the words with it.

**A program you have not tamed still has no mood, and deleting
`assets/memories/` still leaves the game whole.** The box follows the memory
store rather than ownership, so a wild program drops it outright while an
owned one with no catalogue keeps it and says nothing has happened yet.

**The MOVES box gave up being full-width to pay for the new one.** A band and
two half-width boxes take the same room, so nothing was trimmed to make space
— every move, potential roll and decompile figure the sheet showed before is
still on it.

## 0.13.115

**A town that hates you now puts people on the ground between you and it.** A
settlement at `Hostile` standing near the party fields a patrol on its own
ground, up to three of them, and they are ordinary wildlife with a tether
rather than a species you have never seen.

**They notice you before you have to fight them.** A patrol gives chase when
you come within eight tiles, which is inside the range you can examine one
from — so you can see whose it is, and turn around, before it sees you.

**Repairing standing sends them home.** The moment a town stops being
Hostile its patrol stands down where it stands and goes back to wandering,
which is the way out of that band made visible.

**Killing one costs you with that town and with nobody else.** A full patrol
wiped in self-defence costs less than clearing one nest on that town's
doorstep pays back, so the ladder out of `Hostile` stays climbable while
they are in the way. Decompiling one costs nothing at all.

**They wear their town's colour in the corner of their tile**, in the one
corner nothing else on the map claims — so a patrol member still says what
it is, how dangerous it is and how rare it is at the same time.

**A patrol caught mid-chase is still chasing when you load.** Older saves
keep loading exactly as they did.

## 0.13.114

**A Teardown Rig strips downed programs while you are somewhere else.** Research
Teardown, build one, post a program to it and keep it on the grid.

**You load it from the store you already use.** `L` on the list hands it every
program you are holding, `Q` on a tool page hands it just that one, and you pick
the tool either way.

**What it leaves is ordinary materials in its output buffer**, so haulers, depots
and adjacent-take carry the yield with no rule of their own.

**Its hopper holds six against the ten your pack does**, and an over-ask takes
what fits and leaves the rest with you.

**A rig that cannot hold a whole body's worth holds the program instead**, and
reports as clogged rather than paying part of it.

**The Routine Reader and the Harness Puller are refused at the door**; a routine
and a gear copy are not materials a buffer can hold.

**A Teardown Rig is an extraction bench too**, so building and upgrading one pays
your own teardowns as well.

**Modders get `strips` on a structure def** — one number, how many programs the
hopper holds.

## 0.13.113

**A town that hates you now sends people for your stores.** A settlement at
`Hostile` standing, within half a region of your base, raids it on its own
initiative — the first raid in the game with somebody behind it, where a GC
Entropy Sweep is weather.

**Raiders take rather than break.** They carry off a share of the banked
salvage your base runs on and leave the machines standing, and the log line
names the town that sent them. Bounded at both ends, so a thin bank still
loses something and a rich one is bled rather than gutted.

**Everything that turns a sweep away turns raiders away too.** Structures
carrying raid defence and the garrison an allied neighbour stations both
count, against one number — so the hostile and friendly ends of the standing
ladder finally meet on the same axis. A maxed garrison alone softens a raid
and can never stop one; a shield network can.

**The town page says whether a hostile neighbour is near enough to trouble
you.** The distance is a per-run coin flip, and a page silent about it reads
as the band having no consequence at all.

**Towns are half as far apart.** A settlement region is 128 tiles across
rather than 256: measured over 2,000 worlds the median walk from your base to
the nearest town falls from 147 tiles to 71, and the worst quarter from 227
to 102. Every settlement radius is a fraction of the region, so they all
moved with it and the odds of a neighbour close enough to garrison are
unchanged.

**A first-town-raid notification, separate from the sweep's.** The sweep is
weather and this is a consequence; the same words would teach the wrong
lesson.

**None of this has been played.** It is green and unseen, and none of the
numbers behind it are tuned — no instrument in this repo models raids, towns
or loot.

## 0.13.112

**The manual has a page on taking a program apart.** What a kill leaves and
where it goes, the ten your store holds and what a full one turns away, the
five tools and what each of them reaches for, what the screen quotes before
you spend a body, and what a Compiler is worth standing against upgraded.

**Research Data never comes out of a downed program, through any tool.**
Research is earned by running a Research Node, and a body is not a shortcut
past it. Nothing that ships could have paid it out, so no run you have
played changes — the rule is there for what gets authored next, mods
included.

## 0.13.111

**The key bar at the foot of the screen now names `<` and `>`.** They were
the one crossing in the game with nothing on screen leading to them — the
Stack's link wears a `>` on its own cell, and your base's way out is a Home
that looks like every other structure.

**One label for every locale**, so it reads `< > ascend/descend`: `<` is up
and `>` is down whether you are stepping onto your base or into a stack.

**`hjkl move` and `. wait` come off the bar to pay for it.** The arrow keys
walk as well as `hjkl` does, so movement is the one verb you find without
being told, and `.` does nothing inside your base at all. Dropping them
bought room rather than costing it — `x examine` now survives at the
smallest supported window, where it used to fall off the end.

**None of this has been played.** It is green and unseen.

## 0.13.110

**A town can now actually turn on you.** `Hostile` sat fifty points below
neutral while the only thing in the game that lowers standing is abandoning
a job, at four points a time — thirteen abandonments at one town, undone by
any single trade or cleared nest. Nothing ever reached it, so a shut market
and a preyed-on caravan route were wired, tested and impossible to meet.

**The ladder is measured in deeds now, not in points.** Both ends sit five
deeds from neutral: five finished jobs to `Allied`, five abandoned ones to
`Hostile`. The old thresholds mirrored each other numerically while the
movers behind them did not, which is what made the bottom unreachable.

**Handing a job back still costs less than finishing one pays.** That
asymmetry was deliberate and is untouched — taking work you cannot finish
should not be worse than never reading the board.

**A town you have lost can be won back by deeds it did not ask for.** Going
`Hostile` closes the market and the board, so the jobs that took you there
are out of reach from inside it; clearing nests and collapsing Stacks near
the town is the way back up.

**A standing you have already earned may re-read.** The bands are derived on
every lookup rather than stored, so an existing save's towns re-band against
the new thresholds — no save-format change, and nothing stops loading.

**None of this has been played.** It is green and unseen.

## 0.13.109

**`.` no longer waits inside your base.** The key is bound on the zone
surface and in the Stack; out of phase it does nothing at all — no turn
spent, and no refusal either, because time in base space is time you spend
by walking it.

**The map's top border now names the anchor's two keys.** They were the one
crossing in the game with nothing on screen leading to them: the Stack's
link wears a `>` on its own cell, and base space's way out is a Home that
looks like every other structure.

**Out of phase the border reads PHASED OUT and names `>`** — for as long as
you are in there and wherever you have walked to, not only once you are
standing back on the way out.

**On the surface it reads ANCHOR and names `<`** whenever you stand on one
with a Home behind it. A dark anchor still says nothing, because there is
nothing on the other side to go to yet.

**None of this has been played.** It is green and unseen.

## 0.13.108

**A tool that pulls gear off a downed program instead of a routine.** The
Harness Puller — taught by Deep Analysis — rolls that program's own gear
drop chances and hands over whatever lands, through the same rare-tier door
a found copy walks through: a pulled item can be rare, affixed and rolled
for quality exactly like one that dropped at the kill.

**It is a second door, not a replacement.** Every existing way gear drops —
at the kill, from a nest cache, from a Stack feature cache, off the surface
boss — still pays exactly as before. Gear simply turns up more often
overall for any program you haul home and strip — and a fully upgraded
Compiler makes it dramatically more likely, not merely somewhat.

**A miss pays nothing.** No consolation scrap; the program is spent and the
time is spent whether anything comes off it or not.

**A better Compiler raises the odds.** Both the tool's own tier and the
extraction bench's tier push the chance up — a tier-1 tool worked at a
never-upgraded bench pays exactly the species' authored odds, and every
tier past that is what buys more.

**The extraction screen quotes the odds per item before you spend the
program**, the same figure the pull itself rolls against.

**None of this has been played.** It is green and unseen, like the phases
before it.
## 0.13.107

**The `extraction` template opens on the whole extraction kit**, which the
one it replaces could not: that capture predated the feature it is named for
and carried no `tools:` key at all, so it opened with the starter Salvage
Clamp alone and nothing researched.

**All four tools known, three installed and a forged carrier in the pack**,
with six downed programs from real kills spanning Ordinary to Prismatic plus
a boss — so the Tools screen, every yield preview and the extraction door
are one keypress from the opening frame rather than an hour of play away.

**It is held to that by a test**, `the_extraction_template_opens_on_the_whole
_kit`, the assertion the other three templates each already carry: a fixture
whose keys a format migration strips would still parse, still load, and still
open on nothing.

## 0.13.106

**Both settlement aid radii were dead by geometry, and are now fractions of region spacing rather than flat numbers.** Towns stand one per 256-tile region, so the garrison radius found a town near the anchor in 1.6% of worlds and route predation found one beside a trade lane in none of 2,000 — half a region and a quarter reach 39% and 18%.

**A route to your nearest market is safe by construction, and that is the design.** Its corridor is short and points away from every other town, so predation is a risk you take by hauling past somebody rather than by trading at all.

## 0.13.105

**The Compiler is where a program gets taken apart properly.** Standing one
anywhere makes an extraction quicker; every tier you upgrade it past the
first makes the same tool draw more out of the same program.

**A tool that reads routines instead of materials.** The Routine Reader —
taught by Cortex Hacking — reads a downed program for what it was running
and keeps one, favouring the earliest thing its species knows that you
don't. It teaches knowledge only: an exclusive routine still comes off a
program the one way it always has, by breaking down one you control.

**A downed program remembers what it was running.** A wild program carrying
a routine leaves that routine on the record, and the Reader offers it first.

**A sortie carries its kills home rather than teleporting them.** Programs
its squad downs ride back with it and arrive when it does.

**A trip can out-earn what you can carry, and the overflow is lost.** The
store refuses a delivery it has no room for and destroys nothing you were
already holding — but the programs it turned away are gone, and a long
sortie routinely downs more than the store's ten. Empty the store before
a squad comes home.

**None of this has been played.** It is green and unseen, like the two
phases before it.

## 0.13.104

**The compass gives a distance for every destination**, not only the ones
you have walked to. Reaching a place buys its name; how far off it is was
never something arriving taught you, and a bearing with no figure is a
direction to wander in rather than a trip you can plan.

**And it is a block in the map's top-right corner now**, under the threat
readout — an arrow, the name and the distance — instead of a line along the
map's bottom edge. The block sits over the map rather than beside it, so
picking a destination no longer costs the map any height.

## 0.13.103

**A destination and a bearing to it.** `u` opens a picker of the places the
run already knows about — the home base, every settlement recorded, every
Stack entrance — and the one you pick rides the zone map's bottom border
until you clear it.

**Two tiers, and the difference is what you have actually walked to.** A
place the engine has recorded gives a bearing and a generic noun: *a
settlement, south*. One the party has reached gives its name and a
distance: *Lowport, south, 219*.

**It surfaces what is recorded; it does not widen what gets recorded.** A
town four regions out stays invisible until the party walks near enough for
it to be materialized at all.

**Nests are not destinations**, and the compass is hidden underground and in
base space, where the party's `Position` is pinned and a live bearing would
be frozen while reading as live.

## 0.13.102

**Extraction tools are something you earn now.** Research teaches a tool,
you forge one from materials, and installing it burns the forged tool into a
slot. The Salvage Clamp you start with is no longer the only one you will
ever have.

**Two tools past the starter**, each behind the research node whose subject
it shares — a Component Stripper that pulls whole components out of a downed
process (Program Refactoring), and the Core Tap that draws the compiled core
out from under its shell (Deep Analysis).

**Tool slots grow with level**, to a ceiling of four.

**A new screen, `[T]` from the party menu.** One row per tool you know or
have installed, with the slot it sits in and how many you are carrying:
`[F]` forges, `[I]` installs, `[X]` pulls one back out.

**Pulling a tool does not hand the forged tool back** — what is in the slot
*is* the tool, the same rule an installed Routine Disk follows. You keep the
knowledge, so anything you have researched can be forged again, the starter
included.

## 0.13.101

**A `settlements` template opens the game standing on a town's doorstep**, so
the four standing bands, the aid page, dispatch and routes can be looked at
without walking 128 tiles to find a town first.

**Lowport is Allied, Tally Yard is Hostile and The Quiet Stack is Warm**, and
a Dispatch Relay stands beside the Home — raised by the crew, not written in.

**Both settlement specs are archived**, which is `INDEX.md`'s invariant; the
aid spec had been left listed as open and unmerged.

## 0.13.100

**An Allied town is worth something now, and the top of the standing ladder
stops being a plateau.** Three aid consequences, free while the band holds:

- **A friendly town keeps a detachment near your base**, softening GC
  Entropy Sweeps — ramping from Warm, and capped so that however many
  neighbours you win over, a sweep still lands for something.
- **An Allied town will spare you a program for the asking**, arriving at
  your anchor as base staff on a cooldown — labour for the crew, not a free
  companion, because progression here is still earned by fighting.
- **Its relay will carry you home, and carry you back out from yours** —
  `[T]` on the town page and on the Relay hub, for exactly the ticks the
  walk would have cost and none of the encounters.

**`[G]` asks a town for a program and `[T]` rides its relay**, both
uppercase, both on screens that already existed — no new screen ships with
this.

**The town page now says what the town is worth**, one line per aid it
actually offers, in words rather than in numbers.

**A save from 0.13.99 loads unchanged** — `save::SAVE_FORMAT_VERSION` stays
at 32, since everything new is additive behind `#[serde(default)]`.

Older releases: [CHANGELOG-0.13.md](CHANGELOG-0.13.md).
