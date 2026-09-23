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

`0.12.0` and everything older, including the entries below `0.2.0` that
predate versioning, are in [CHANGELOG-ARCHIVE.md](CHANGELOG-ARCHIVE.md). They
were split off because GitHub stops rendering a Markdown file past about
512 KB, and this one had reached 669 KB.

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

## 0.13.99

**Caravan routes: load cargo at the Relay, send it to a town you know, and
the Credits come home.**

**A route can be left standing, reloading and departing again until you
sever it.**

**A Hostile town near the line your caravan walks takes a cut of it.**

**The Relay has a screen at last, and sorties finally have one too** —
`[S]` picks a squad for a sortie site, `[C]` builds a cargo manifest for a
town, `[X]` severs a standing route.

## 0.13.98

**Towns post their own work.** Every settlement now keeps a job board,
opened with `[J]` from its hub page, drawing on exactly the same contracts
the Broker does — no new schema, no new objective kinds, nothing to author.

**How many jobs a town posts is how much it likes you**, from one at Cold up
to four for an Allied town, and a Hostile one posts none at all. Courting a
town buys you work as well as a better price.

**A town leads its board with what it is good for** — a Materials town posts
the fetching, a Programs town the hunting — but it is a preference and not a
filter, so a board is never short of jobs it could have posted.

**A job is delivered where it was signed.** A town's contract is handed over
at that town's counter and the Broker will not take it, which is what makes
walking out to a settlement worth the trip.

**Finishing a town's job raises its regard for you, and handing one back
lowers it** — by less than finishing raises it, so a job you turn out not to
want is never worse than one you never took.

**Two towns in one sector post different work**, and a town's board turns
over more slowly than the Broker's, so what you saw on the way out is still
there when you arrive.

## 0.13.97

**Towns remember you now.** Every settlement carries a standing that runs
from Hostile through Neutral to Allied, shown on its hub page, and the game
says so the moment you cross from one band into the next.

**Trading at a town's counter earns its regard**, both directions counted —
what you spend and what it pays you are equally business it did — and small
baskets bank their leftover volume toward the next point rather than
rounding to nothing.

**So does clearing a nest or collapsing a stack near one.** A town hears
about what happens within sixty tiles of it and nothing beyond that.

**A town that has come to hate you shuts its counter.** The market screen
still opens and says so, rather than dropping you back to the map with no
explanation.

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
`standings` is additive behind `#[serde(default)]`, so a save from before
this release opens with every town at Neutral, which is where a new run
starts anyway.

## 0.13.96

**Existing saves load, and one resource is deliberately dropped from them.**
`save::SAVE_FORMAT_VERSION` stays at 32 — `settlements` is additive behind
`#[serde(default)]` and defaults to empty, so a save from before this
release simply materializes its towns as the party reaches them. The
exception is `StackMemory`: a frame's seed now folds in its tier, so a
memory written by an older build describes a maze this build no longer
generates. A save without the new `stack_memory_tiered` marker has its Stack
fog of war cleared on load rather than restored onto the wrong cells.

- **The world is persistent. A breach raises the tier; it does not rebuild
  the sector.** There is one map for the run. The party does not move, the
  ground does not change, the base stays where it stands, and every shelf,
  entrance and structure survives the crossing — what changes is what walks
  on it. A place can only be worth returning to if it is still there.
- **Towns stand in the world, and where each one stands is derived from the
  world seed rather than rolled.** Walk into one to enter it; it admits
  nobody, so the tile is a door rather than a floor. `x` toward one from a
  distance names it the same way.
- **A settlement has a name, a kind, a specialty and a temperament, and all
  four are content** — `assets/settlements/*.ron`, documented in that
  directory's README. Deleting the directory is a supported install and
  gives back a world with no towns in it.
- **`[M]` opens a town's market: one shelf, one basket, one commit.** Buying
  and selling settle in a single turn through the same core the caravan's
  own counter uses, so a basket can be funded by its own sales, and the
  buyback shelf a town keeps outlives anything built on its tile.
- **A town's temperament prices both directions, and Mercantile is not the
  average of the other two** — it competes on what it charges you and takes
  its margin on what it pays you.
- **`assets/sectors/` is retired.** Per-zone terrain deltas had nothing left
  to vary once a breach stopped carving new ground, so the biome palette is
  one fixed signal for the whole run.

## 0.13.95

**Existing saves load, with one deliberate exception.**
`save::SAVE_FORMAT_VERSION` stays at 32 — both `DownedPrograms` and `Tools`
are additive behind `#[serde(default)]`. `downed_programs` defaults to
empty, exactly the pre-extraction game. `tools` does not: a save written
before this release paid its material income through the kill drop this
release retires, so an absent `tools` key defaults to the starter tool
rather than an empty loadout — a migration for a save that predates the
concept entirely, not a re-grant into one that already made a choice.

- **A defeated wild program is left behind as a downed program you carry,
  not a pile of raw materials.** Species, level, rarity, boss flag and a
  rolled condition all travel with it in a new player store,
  `components::DownedPrograms`, capped at 10; a kill, a nest cache and a
  boss kill all leave one.
- **A carried tool strips a downed program down into what it drops.** The
  new `assets/tools/*.ron` catalogue ships two — `salvage_clamp` (the
  starter tool, forged into slot 1 at creation) and `core_tap` — each
  reaching a different category of the program for a different pool of
  items, at a ticked time cost.
- **`Mode::DownedPrograms`, opened from the pack, lists what you're
  carrying and previews exactly what each installed tool would give before
  you commit to one.** The previewed figure and the granted one can never
  differ — both read `Game::extraction_yield`, the one derivation.
- **The starter tool pays a median kill the same as the material drop it
  replaces.** `SpeciesDef::work_resource`'s old kill-time drop is retired;
  every shipped species keeps paying what it always did through the new
  `rich_in` field, which defaults to `work_resource` and needed no
  authoring pass.
- **Extraction is deterministic, not a dice roll.** The yield is a
  weighted apportionment of a fixed unit count, so it spends no RNG and a
  quoted preview always matches what you actually get.

## 0.13.94

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` is untouched —
nothing here reaches the save at all.

- **Powering down out in the field can be interrupted.** A charged rest rolls
  `REST_AMBUSH_CHANCE` once the charge has been taken; on a hit a pack engages,
  the outlet is spent and nothing is restored.
- **A free rest inside your base never rolls**, and it is safe by placement
  rather than by a locale check — the roll rides the branch that takes the
  charge, so base space cannot lapse into being rollable.
- **A jumped rest clears nothing**, which is the rule a refused rest already
  followed: the heal, the Power refill and the field buffs all sit below the
  roll.
- **A roll that hits but fields no pack lapses into an ordinary rest**, so a
  charge is never burnt for no fight at all.
- **`surface_ambush_pack` is an extraction out of `maybe_ambush`, not a copy.**
  A rest is the first roll site that cannot know its locale by construction, so
  the two pack builders are named as a pair and each states its placement rules
  once.
- **Three app-core fixtures no longer use a field rest as a one-line map
  action**, since `r` can now open a battle.
- **Sprite Forge's test scratch directory no longer races itself.** Keyed on
  the pid alone it was shared by all 21 sprite-forge tests at once, so a full
  workspace run failed intermittently on `AlreadyExists` — never when the file
  was run alone, which is the shape that reads as unrelated.

## 0.13.93

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` is untouched —
nothing here reaches the save at all.

- **A tile's con read is the glyph's own hue again, and the corner earmark is
  what the tiles that cannot spend that hue pay instead.** No mark small
  enough to share a cell with four other channels carries a scan as well as
  the ink in the middle of the cell does.
- **`ConRead` is the one place that is decided** — `Glyph(rung)`,
  `Earmark(rung)` or `None`, one value rather than two conditions agreeing at
  two draw sites, so "never both and never neither" is a property of the type
  rather than of a convention.
- **Two tiles cannot spend the hue: one drawn as a sprite, and a boss.** Art
  is authored near-white precisely so egui's tint multiplies through it, and a
  boss's magenta is the ink rather than a fifth rung.
- **The boss's corner mark goes with it**, its census retargeted from "not any
  other mark" to "not any con rung" — the reading a magenta glyph could
  actually be mistaken for.
- **`ConRead::of` takes the sprite call's own answer and never
  `sprite.is_some()`**, so a name the table has nothing under falls back to a
  glyph that can still carry the rung.

## 0.13.92

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` is untouched —
nothing here reaches the save at all, and the whole feature is inert without
both `FERAL_DEV_SPRITES` and a checkout behind the build.

- **A developer can draw a sprite in-game for any species, structure or map
  fixture, and the map shows it without a restart.** A main-menu row behind
  `FERAL_DEV_SPRITES` opens a picker of every subject the map can draw art
  for; the editor saves a 16x16 PNG into `assets/sprites/` under the name the
  loader already looks for, so no `.ron` file and no Rust changes.
- **Art is turned off by renaming it to `<name>.png.off`, never by deleting
  it.** `scan_sprite_dir` filters on the extension being exactly `png`, so a
  disabled sprite is already invisible to the loader and the glyph comes back
  untouched.
- **The editor's canvas mechanics are shared with the player's icon editor
  rather than copied.** `icon::Canvas` and app-core's `CanvasEditor` hold the
  cursor, the brush, the undo ring and the paint guard; both editors own one
  and keep their own sink.
- **`SPRITE_PALETTE` is a separate constant from `ICON_PALETTE`, and must
  stay that way.** The player icon's `v2` codec is 64 hex digits, so a
  sixteenth colour in `ICON_PALETTE` would encode as a digit meaning
  transparent and silently blank a drawing the player cannot get back.
- **The dev palette leads with a bright-biased value ramp** because the
  renderer applies a tile's colour as a multiplying tint, so near-white art
  inherits the species hue, `biome_tint` and the damage dimming for free.
- **A sprite can be painted with the mouse**, the first pointer input
  anywhere in the renderer, resolved to a cell before it reaches app-core and
  confined to this one screen.
- **A drag is one undo entry, and a click that changes nothing records
  none.**

## 0.13.91

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
both new fields are additive behind `#[serde(default)]`, so a save written
before this release loads with an empty receipt.

- **A perk screen and a companion's talent ladder can each be refunded whole
  for a flat Credit price.** `X` opens a confirmation; `y` clears that
  ledger, hands back every point spent, and takes the stats those purchases
  baked in back out.
- **A stat a purchase baked in now leaves a receipt, because neither grant is
  invertible.** `Perk::Buffer` and `TalentNode::Stat` read the value at
  purchase and floor at a whole point, so refunding without a record would let
  buy-wipe-rebuy compound maximum Integrity without limit.
- **A refund no longer makes the next banked-XP Perk Point cheap.**
  `convert_overflow_xp` prices each point off perks *ever* bought rather than
  off the list a respec empties, which is what keeps banked cap XP bounded.
- **Refunding Process Pool is refused while the roster is full**, rather than
  leaving more programs owned than there are slots to hold them.
- **A talent refund takes back the tree's routines and leaves the player's
  alone**, so a routine installed from a disk survives the wipe.

## 0.13.90

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release is test-only and stores nothing.

- **A screen that reads a modifier arrow must be named in `App::handle_key`'s
  fold, and a census now says so.** The fold rewrites
  `ShiftLeft`/`ShiftRight`/`CtrlLeft`/`CtrlRight` to bare arrows for every
  screen outside a four-mode allowlist, so a picker that grows the four arms
  without being listed gets them unreachable — the modifiers silently become
  plain steps and nothing fails to compile.
- **It reads source, because that direction is unreachable at runtime.** The
  fold precedes dispatch, so an unlisted screen's modifier arms are dead by
  construction and behave identically to bare `Left` whether the bug is
  present or not; comparing the two lists in source is the only place they
  meet.
- **`crates/app-core` is warning-clean.** `seed_profile` takes `&Path`, which
  was the crate's only clippy warning.

## 0.13.89

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing here is stored, and both new def fields are additive and
`#[serde(default)]`, so an existing `.ron` parses untouched.

- **A species or structure can name its own sprite.** `sprite:` in its `.ron`,
  defaulting to the def's own id, so dropping `assets/sprites/rootkit.png`
  into the tree is the whole of giving the Rootkit art.
- **The loader reads the directory instead of a list in Rust.** The table
  holds exactly what is on disk, so a name with no file behind it is now
  unreachable rather than a warning several frames late.
- **Nothing on screen changes yet.** All 47 shipped defs have no art and draw
  the glyph they always did; this is the code half of entity sprites, and the
  art is the next slice.

## 0.13.88

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the icon string gained a `v2` form, and a `v1` string written by 0.13.87
still decodes, folded onto the new grid.

- **The icon you draw is 8x8 now.** 256 cells was a lot to fill with the
  arrow keys, and the canvas is the one screen where the work is
  proportional to the grid.
- **The sprite is still 16x16**, so each drawn cell paints a 2x2 block — the
  format art will land on later is untouched, and under nearest sampling the
  result is identical to a native 8x8 texture either way.
- **A drawing made under 0.13.87 survives.** Each 2x2 block folds to the most
  frequent painted colour in it, so a silhouette comes through at half
  resolution rather than being thrown away.

## 0.13.87

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the two new fields are additive and `#[serde(default)]`, so a save written
before this release loads with no drawing and behaves exactly as it did.

- **You can draw your own icon now.** A 16x16 editor on the character-creation
  wizard's Icon step, from a fixed fifteen-colour palette — five steps of value
  and ten hues, because shading with value is what makes a figure read at this
  size.
- **`Tab` moves between the canvas and the palette**, so the arrows mean one
  thing at a time rather than changing meaning based on a mode you have to
  remember you are in.
- **What you draw follows you into the next run.** It is kept in the profile,
  which is what outlives a run, and copied into the save so that redrawing it
  later cannot repaint a character you already made.
- **A drawn icon is the one sprite in the game drawn untinted.** Every other
  sprite inherits a multiplying tint and must be authored near-white; the
  player's own tile inherits none of the hues that rule protects, so a drawing
  can carry its own colour there and nowhere else.

## 0.13.86

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing here is stored; the con read has always been derived at draw time.

- **A program's glyph tells you what it is again.** Its colour used to be
  replaced outright by the con read for anything hostile, so a tile could say
  what a program is or how dangerous it is and never both.
- **How dangerous something is, is a bar along the bottom of its tile now** —
  green through red, the mirror of the rare-tier bar along the top.
- **A boss wears its own corner mark**, so being a boss no longer costs it the
  con read.
- **A creature that is both a boss and a nemesis shows both.** The two used to
  compete for the same colour and the nemesis won outright, which meant one of
  the two facts about the most dangerous thing on the screen went undrawn.

## 0.13.85

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing here is stored; all three numbers are read at draw time.

- **The clouds move now.** They shipped at a wind slow enough that the
  leading edge spent three seconds crossing a tile, and since the edge is
  spread over several tiles by design, no single tile visibly changed inside
  the time anyone looks at one — so weather that was drawn every frame read
  as a still image. Doubled, in the direction it already had.
- **The map is darker, and the shadows are deeper.** A cloud rides the
  ground's shade and never the vignette, so a glyph under one is still drawn
  at full brightness; that is what makes a deeper shadow free, and it is why
  the vignette floors moved last and by the smallest step that reads.
- **The spawn ring is gone.** The magenta outline marked where you
  materialized on breaching a zone, which is a fact with nothing to spend it
  on once you have walked a few tiles away from it.

## 0.13.84

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the ledger is an additive `#[serde(default)]` field, so a save written before
it existed loads with an empty one, which is exactly what that run recorded.

- **The base tells you what it has made for you**, on a new "Base output" row
  in the base menu: one line per item, the sector's total beside the run's,
  and what a machine made beside what you made by hand.
- **Every extract, compile, stall, haul and unit that entered or left the run
  is recorded when `FERAL_DEV_LOG` is set**, through one door that also feeds
  the player's screen — so the page and the analysis a retune is done from
  cannot disagree about what happened.
- **`dev-logs/README.md` documents all eight base records**, with the `jq`
  recipes for the questions they were added to answer.

## 0.13.83

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **Compiling something by hand no longer runs the clock at thirty times
  speed.** The progress bar spent the world's ticks sixty a second against
  the two a second the world moves at when you are standing still, so a
  batch aged your base by minutes while you watched a few seconds of
  progress — needs draining, machines cycling, raid pressure building and
  wild programs spawning, all of it sprinting for as long as the screen was
  up. The bar now spends ticks at the rate everything else does.
- **Hand-compiling costs a tenth of the ticks it did.** The tick price and
  the bar's speed are the same number seen twice, so slowing the bar without
  cutting the price would have made one Hardened Shell two and a half
  minutes of watching it fill. It is fifteen seconds now, and the median
  recipe is five. What a bench still buys you is everything except speed: it
  works while you do something else, it costs you no Power of your own, and
  a developed one reaches quality your hands cannot.

## 0.13.82

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
how many times a body swings is worked out from its class and level when the
blow lands, so there is nothing new to store.

- **A Striker swings twice.** Your class said what you were good at and then
  had nothing to do with an ordinary attack: affinities only ever reached
  the routines you ran, so a Striker and a Medic threw the same punch. From
  level 8 a Striker takes a second swing every round — a full one, not a
  lesser follow-up — and so does any companion or wild program whose own
  affinities make it a Striker. Level 8 is past the first sector's ceiling,
  so it is something you cross into rather than start with, and the second
  swing aims at whatever is in front of you after the first one lands.
- **A Crash now costs a Striker the rest of the round.** Fumbling badly
  enough to crash cost you "your next action", which meant exactly one blow
  back when everyone threw one. For anyone with two, it takes both.

## 0.13.81

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the three new class variants append, so a save already naming one of the
five keeps the class it had.

- **Three classes no species can ever be.** A player's class was one of the
  same five roles a program can have, which meant every class had to be
  expressible as a spread of affinity multipliers — and so nothing that
  wasn't combat could be one. The player has their own class list now, and
  three classes on it that no species could hold: a **Decompiler**, whose
  every decompile attempt lands better and who trades damage for it; an
  **Invoker**, who carries two more routine slots than anyone else at every
  level, the level cap included; and a **Fabricator**, whose posted machines
  finish every work cycle a fifth sooner. The five original roles are
  untouched and still shared with the programs that have always had them.
  Each new class's effect is a named query in the engine rather than a field
  in its file, on the same seam a perk already uses — so the catalogue in
  `assets/classes/` stays a catalogue, and deleting it turns the affinity
  spread and the opening kit off without turning the effect off with them.

## 0.13.80

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
every new field is additive behind `#[serde(default)]`.

- **Compiling by hand costs the machine's own cycle, ten times over.** Every
  item the base's eleven assemblers make could be hand-compiled at the
  identical recipe cost, in a single tick, with no worker, no adjacency and
  no power — so the machines were a convenience rather than a route to
  anything. A hand-compile now spends ten times the cycle the machine that
  makes it would have taken: a Blank Substrate is 120 ticks against a
  Lathe's 12, a Hardened Shell 300 against an Armory's 30. The recipe is
  still yours; the shortcut is what got priced. A Compiling screen watches
  the ticks go by and any key stops it, keeping the units that finished and
  refunding the one in progress. Those are ordinary game ticks, so they cost
  Power like any other span of them — and a batch that would run your
  reserve out is refused whole rather than leaving you empty in the field.
- **The Zone Portal asks for a terminal product from every base chain, and
  the bill grows with the sector.** Breaching needed ten Portal Fragments
  and nothing else, which meant the entire production chain was optional and
  a run could reach the last sector having never built a machine. Opening
  the way now takes 24 Fragments and the output of the Power, armour and
  substrate chains besides; the Trace and Cache chains join at sector two
  and the refactoring chain at three, as their research comes into reach.
  Each line is ramped from the sector it was introduced in rather than from
  the first, so a demand that arrives late arrives at its authored price.
- **The base's power grid burns Power Cells to stay up.** A Recharger Node
  cost ten Core Fragments once and supplied the grid forever, which made the
  whole ledger decoration and the Line Driver pointless. A supplier now
  takes a Power Cell off an adjacent buffer every twenty ticks, and one that
  cannot pay goes quiet on both halves — the grid it feeds and the Power it
  trickles into you. The Home is exempt and stays free, so a cold base can
  still raise the Power Conduit that fuels everything else. The fuel is
  named in the structure's own file, so a mod can run its grid on something
  else.

## 0.13.79

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The recovery mark rides the program being mended, and it is green.** A
  Repair Bay wore a bouncing red `+` while it was healing somebody, which
  put the mark on the building rather than on the body it was about to
  give you back — and painted it in the colour this map otherwise spends
  only on a raid's flash and a structure taking a hit. It now bounces over
  the program itself, in the same green a climbing Integrity wears
  everywhere else.

## 0.13.78

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **Deploying a Home is free, and the anchor lands where you stand.** It
  cost five Core Fragments against a starting kit that carried five or six,
  which made founding a formality for a default character and a dead end for
  a bought one — the creation wizard's Kit step replaces the class kit, so a
  run that spent its allowance on gear could not open a base at all and
  nothing said why. And the door now settles on the tile you deployed from
  rather than back at the sector's arrival point: walk to ground you want to
  come back to, then press it. The Home itself still stands on base space's
  own origin, so the way out is where it always was. Founding while standing
  on a Stack link is refused — an anchor sharing that tile could never be
  stepped on to be entered.
- **Two onboarding notices.** The first time your Power reserve goes under
  half, the game says so and names the two ways out — the same threshold
  your attacks start weakening at, not a second number beside it. And the
  first time a program of yours is benched with no Repair Bay standing, it
  says what a benched program can and cannot do and what brings one back. A
  base that already has a Bay is told nothing: the program walks there
  itself.
- The character creator's second step is titled **Achievements**, and its
  footer says what it is.

## 0.13.77

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The field ramp tops out at 39 tiles instead of 135.** 128 tiles was
  picked off the world being unbounded rather than off the scale the map
  actually works at — everything the player reaches for sits inside a couple
  of dozen tiles, and the furthest the game scatters its own content is 40 —
  so the gradient read flat because nobody walked out of the first tenth of
  it. The cap now lands where the outermost Stack link already does: walk to
  the far links and you have seen the whole of it.

## 0.13.76

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
every new `PlayerSave` field is additive behind `#[serde(default)]`.

- **A run is bought as well as chosen.** A starting kit step sits between
  the class and the look, spending Credits off a shelf of what the run can
  afford — the class kit is now a floor rather than the whole of what you
  open with.
- **Four Perk Points are spent at creation, and the tutorial contract they
  replaced is gone.** Anything left unspent arrives with the run rather than
  being forfeited at the door.
- **A page before the class step names what earlier runs have already
  earned you**, so the profile's contribution is visible while you are still
  deciding what to build.
- **Twenty stat points, spent on top of the baseline rather than
  redistributed.** No step lets you leave an allowance unspent by accident,
  and every screen says how big its pool is.
- **The class picker says in words what each class trades**, and the
  manifest is headed by your name and your class.
- **The far field of a zone is the next zone's doorstep.** Zone 1 had no
  interior — every tile of it fielded one tier-0 pool at x1, which conned
  green against the bare baseline before creation spent a point. A surface
  spawn now ramps with distance from the opening ring's edge, capped at
  exactly one zone step, so a strong opening build has somewhere to walk
  without breaching.

## 0.13.75

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
every new `PlayerSave` field is additive behind `#[serde(default)]`.

- **A run opens on seven questions instead of two keystrokes.** Difficulty,
  class, look, stat points, a starter routine, a name and a summary, in one
  screen with one back button; `[R]` rolls whatever is left and jumps to the
  end.
- **A player has a class now, and it grants affinities and nothing else.**
  Five files in `assets/classes/`, each raising one category of routine and
  holding a different one down — so a class is a trade, and an empty
  directory is still a supported install that plays as the game did before.
- **A class brings its own opening kit**, replacing the four items every run
  used to start with.
- **Five points to spend on top of the stats you already had**, priced per
  axis: Integrity, Attack and Decompiler at one, Mitigation at three because
  levelling never raises it.
- **The free routine slot is filled at creation**, from an opt-in pool any
  ability file can join with `starter: true` — and the routine is *known*,
  not merely installed, so it can be etched onto a disk later.
- **You pick your own glyph and colour**, from six swatches held separable
  from every content hue by the palette census.
- **A run has a name, and the save list shows it** instead of a filename.
- **The summary says what your achievement record is about to pay** before
  you commit to the run, read off the same derivation that actually pays it.
- **`Mode::DifficultyPick` is gone**, folded into the wizard's first step.

## 0.13.74

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A notification waits for Esc, and no longer for any key at all.** The
  full-screen news took whatever you pressed next as a dismissal, so a
  keystroke aimed at the map blew past it unread — and a burst of three
  could vanish to three unrelated presses.

## 0.13.73

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A recharge repairs the programs standing with you, and nobody else.**
  Base staff mend at a Repair Bay and a squad away on a sortie lives on the
  provisions you paid for at dispatch — where a rest four frames down the
  Stack used to reach back and repair a base the party had not seen in an
  hour.

## 0.13.72

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A badly hurt staff program takes itself to a Repair Bay.** Anyone left
  under a fifth of their Integrity breaks off, walks there and comes back at
  full — where a Bay used to serve only a program killed in a Forgiving run,
  which made it an inert building under Permadeath.
- **A Repair Bay wears a bouncing red `+` while somebody is in it.** The
  building you paid for gave no sign it was doing its job, and two busy Bays
  bounce out of step.

## 0.13.71

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A worn piece keeps its affix on the manifest.** The name used to run off
  the end of its box, and the half you lost was always the tail — which is
  where the affix is printed.
- **EQUIPMENT is a full-width band on your own page.** A program's page keeps
  the columned box and elides a name that will not fit, from the middle,
  since gear carries meaning at both ends.
- **Each worn row is tagged `WEP`, `ARM` or `MOD`**, the vocabulary every
  other list that names gear already uses.
- **A negative affix reads `-30 DEF`**, not `+-30 DEF`.

## 0.13.70

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A finished contract is summed up in its own words.** The completion
  screen and the log line name the job, what it asked for and what it paid,
  where every contract in the game used to close on one identical sentence
  about the Broker settling up.
- **The onboarding chain closes on its own screen.** It counts the missions
  behind you, never credits a Broker who is not standing yet, and the last
  one is where you are told the board has opened.

## 0.13.69

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The way home is a routine you earn, not a key you always had.** Symlink
  is `AbilityEffect::Symlink` now — researched at Symbolic Links, etched,
  installed and run from the routine list — where `u` opened a one-row picker
  of teleport-capable structures on the first turn of every run.
- **A GC Entropy Sweep waits for the second sector.** The base you raise in
  sector 1 is never swept, so the first ground you lose is ground you had a
  breach to prepare for.
- **The replace picker rates the pieces it offers.** Every other list that
  names gear already carried the figure, and the one screen whose job is "is
  this better than what I have on" answered only in per-axis deltas.
- **The Coherence amenity is a Log Analyzer Bay.** "Defrag Bay" and "Repair
  Bay" read as the same building at a glance and are not.

## 0.13.68

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The transfer picker's two figures are where the units would land.** `you`
  and `container` move as the basket is edited — a take fills the pack and
  empties the shelf in front of you — replacing a `change` column beside two
  columns that reported a *ceiling* and so never moved at all.
- **Putting cargo in a Depot no longer reads as spending money.** The pack
  column was the Depot's shared room seen through one row, so filling any row
  lowered the figure printed on every other one, Credits included.
- **Credits are offered on neither side of a transfer.** A currency is what a
  transfer is priced in, never a thing that moves into a Depot.

## 0.13.67

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The arrows set a compile batch.** Right increases and Left decreases on
  the compile quantity page, Shift jumps to the most this batch can afford
  and Ctrl halves the gap to it — the gesture the depot picker and the
  caravan basket already use for a number, on the one quantity screen that
  was still digits and Enter.
- **The companion screen heads each role.** The roster is a run per role now
  — in your party, away on a sortie, base staff — where a dispatched program
  used to be listed among the staff it is no longer part of.

## 0.13.66

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **A base with nothing that services a need no longer earns a grudge for
  it.** The program is still told, in the same sentence as before; what it
  stops doing is holding the base to account for a building the player may
  not have researched, may not have the materials for, and has never been
  told they want.
- **A walled-off amenity still earns one**, which is the half of this with an
  errand attached — the base had an answer and could not deliver it.

## 0.13.65

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
`SaveData::game_over` is additive behind `#[serde(default)]`.

- **A Permadeath run that flatlines cannot be reloaded.** The slot is sealed
  with the run's own verdict, the load list shows it as FLATLINED, and
  `Game::load` refuses it — the mode used to leave the last autosave sitting
  there, at most fifty cycles before the death and with no record of it.
- **A machine already kept worked satisfies "Man the Node".** The mission
  could only be finished by turning the standing job off and on again, which
  pulls the body off the machine and is the opposite of what it asks for.
- **A GC Entropy Sweep now waits for a base of five**, up from three, which
  is past the roster a run opens with.
- **A program that has downed tools no longer counts as defending the base.**
  A base whose whole staff had walked off read as fully staffed and was swept
  until its machines were gone.
- **One bad memory can no longer stop a program working.** The line was set
  against a memory's valence alone and the worst single grudge in the game
  reaches nearly three times it, so a program worn thin on a base with
  nothing servicing the need downed tools for longer than the run that
  earned it. Downing tools takes a pattern now; one grudge still sulks.

## 0.13.64

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **Routine slots come in pairs now, doubling every kit at every level.** A
  companion still reaches its ceiling at level 6 and the player at 25, with
  twelve slots waiting there instead of six.
- **A new game opens with a free slot beside `decompile`**, where the
  player's one starting slot used to be spoken for.
- **The program page's ROUTINES box still draws six rows**, so a kit past
  that spends its last line on a "+N more" note — the manifest has the least
  clearance in the renderer and buys nothing new here.

## 0.13.63

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **`w` on a program's manifest sends the map's camera to it, and Esc hands
  it back** — the base runs at two ticks a second now and there was no way
  to watch any of it happen except by standing where it did.
- **What may be watched is whatever the sim actually walks**, which is
  `ProgramRole::Staff` less the guards: a party member, a wielded program, a
  squad away on a sortie and a guard all keep the tile they were last
  written at, and parking a camera on one claims a program is somewhere it
  isn't.
- **Deliberately not `position_is_honest`**, the neighbouring rule, which
  goes false for a worker standing at its own machine — the one moment the
  player most wanted to be looking.
- **The camera reads `Game::watch_position` every frame and lets go the
  moment it answers nothing**, so a program dissolved, dispatched, taken
  into the party or left behind in base space is one rule rather than a list
  of endings.
- **A step releases the camera and still steps**, because walking with the
  view somewhere else is walking blind and a swallowed movement key reads as
  the game having frozen.
- **The watch line takes the ground readout's mount rather than a border of
  its own**, since `map_pane`'s bottom border carries nothing by design and a
  strip there would re-lay the whole grid the moment `w` was pressed.

## 0.13.62

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The world runs at two ticks a real second, where it ran at one.** The
  base read as frozen — idle staff drifted a tile every six seconds and a
  dig swing took twelve — and `WORLD_SPEED_MULTIPLIER` in app-core is now
  the one knob for it.
- **Nothing is rebalanced by that**, because every rate in the game is
  priced in ticks: raids, need decay, caravans and production all keep
  their rates relative to each other, and what changes is how much world
  time a real second buys.
- **Wild programs on the map keep the wall-clock pace they had**, held
  there by two constants pinned to the new knob — the wander cooldown,
  hoisted out of `wander_ai_system` into `WANDER_COOLDOWN_MIN_TICKS` /
  `MAX_TICKS`, and a halved `WILD_SPAWN_CHANCE`.
- **A nest guardian is deliberately not held back**, so a pursuer and the
  player both move at two tiles a second and you still outrun a chase in a
  straight line without ever shaking it.
- **The spec for wanderers in the Stack is written and nothing is built.**

## 0.13.61

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The base anchor is drawn as a portal now, not a `#`.** Its glyph was
  chosen by elimination rather than by meaning — `>` and `<` were claimed by
  Stack links, Blue and Cyan were reserved — so it was the one fixture on
  the zone map that said nothing about itself.
- **`assets/sprites/anchor.png` is a ring with a bright core**, hard-edged
  rather than anti-aliased, and near-white so it inherits the anchor's tint
  the same way every sprite does.
- **The "Base Space" notice draws the same art and no longer names a
  character**, so the tutorial and the map agree on what to look for.
- **`EntityView::is_anchor` is what the renderer reads**, the anchor being
  the one map fixture that is neither a creature nor a `Structure`.

## 0.13.60

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **Cloud shadows drift across the zone map: soft patches of shade, off-axis
  and slow enough to be caught rather than watched.**
- **They are pure atmosphere and mean nothing.** A shadow crossing a biome
  under a Static event is not that event and does not mark it — Static claims
  a whole biome at a time and still reads out on the map pane's border.
- **Base space and the Stack have no sky, and neither takes a shadow.**
- **A cloud falls on bare ground only**, so a structure's damage wash and a
  glyph's own colour are never dimmed by something passing overhead.
- **The vignette goes on meaning the Power reserve alone**, which is why the
  shade rides the per-tile jitter instead.

## 0.13.59

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The surface has weather: four Static events, biome-wide and either on or
  off, that stack on whatever the ground was already doing.**
- **Which event is live is derived from the world seed, the sector, the biome
  and the clock epoch, so there is no save field, no roll to scum, and it
  rotates on its own.**
- **Static is the first thing to bias the ambush roll, so ground you have
  crossed safely all run can turn dangerous without the map changing.**
- **Packet Flood claims Open Grid, which has no standing condition, so the
  biome most of the map is made of is now safe *usually* rather than
  always.**
- **Ground effects finally say what they are doing: the crossing line names
  the condition, its description arrives the first time you meet it, and
  Static announces its arrival and its passing.**
- **The map pane's top-left border stops reading `SECTOR MAP` and reads the
  ground you are standing on instead.**
- **The first Static of a run takes the screen, on the tutorial half of
  `NotificationKind`.**
- **Ambient effects are a table in Rust, not a catalogue on disk:
  `assets/environment/` and `EnvironmentDb` are gone, and `GroundCondition`
  and `StaticEvent` are the census.** This is a mod-facing content directory
  removed — a third-party file dropped there is no longer loaded.
- **`Game::terrain_at` is the one door onto what a place does to you, and the
  zone-1 gate and the base-slab refusal have one definition between them.**

## 0.13.58

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

- **The surface map's vignette floor answers to the Power reserve, 0.75 at a
  full one down to 0.60 at an empty one.**
- **The Stack corridor says the same thing down the axis it has: the fog
  thickens as the reserve drains, and the cell the party stands in front of
  is identical at every reserve.**
- **Neither half dims what the player must not miss — the surface floor
  stops well short of illegible, and `MARK_FOG` takes no reserve term.**
- **No engine number moves; `view_cone` and `visible_rows` hand over the
  cells they always did, so this is a look and not a sight-range change.**
- **`hud::log_frame`'s `POWER_MAX` points at `components::POWER_MAX` rather
  than a hand-copied `100.0` whose own doc comment named it.**
- **Notification copy is a table in Rust, not a catalogue on disk:
  `assets/notifications/` and `NotificationDb` are gone, `NotificationKind`
  is the census and `NotificationKind::def` the copy.**

## 0.13.57

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32;
`PlayerSave::tutorial_seeded` is additive behind `#[serde(default)]`, which
is exactly what field-named RON retired migrations for. A run in progress
when this lands is never told to build a Home it built forty hours ago —
the whole chain is filed as finished at load.

Onboarding is a **chain of contracts** now: eleven missions, one live at a
time, handed to the player rather than offered, drawn green, and suppressing
the ordinary board until the last one is done.

- **The first hour was discoverable and none of it was offered.** A new run
  dropped you on a surface with five Core Fragments and no instruction.
  Everything it asks of you — raise a Home, examine a thing, decompile a
  program, stand up a Broker, work the transfer screen, queue a standing
  order, spend a perk point — you had to find. The `starter: true` queue was
  the first attempt and was not enough: it biased a board draw, it needed a
  Contract Broker you had not built, and it said nothing about order.

- **A tutorial mission is an ordinary contract.** `ContractDef` gains
  `tutorial: Option<u32>` — a *step*, spaced 10 apart so inserting one later
  renumbers nothing. Nothing downstream can tell one from a contract you
  signed for: the ordinary `contract_system` advances it and the ordinary
  `complete_contract` settles it. `ContractDb::tutorial_chain` is the one
  derivation of what the chain is, and the run's position in it is **the
  first mission not in `contracts_done`** — no cursor, no index, nothing
  that can disagree with the save about where you are.

- **`Game::ensure_tutorial_held` is the one writer**, and it bypasses
  `accept_contract` entirely. Three things follow as *omissions rather than
  checks*, which is the point of routing it that way: `MAX_ACTIVE_CONTRACTS`
  never sees it, so the cap keeps meaning what it meant; `broker_reach`
  never sees it, which is what lets the first five missions exist before a
  Broker does; and `offerable` never sees it, so no `min_zone` holds the
  chain up. It cannot be given back — an unbreakable chain with a give-back
  key is not a chain — and `[A]` refuses with a sentence on both the popup
  and the log rather than doing nothing.

- **Two new objectives, and six new verbs behind one of them.**
  `Objective::Hold` is *have this many in your pack at once*: state-shaped,
  latched, no Broker, and met four frames down — which is what lets the
  chain teach that fighting pays in stock before a Contract Broker is
  standing. `Objective::Perform { deed }` carries all six new verbs over a
  closed `Deed` enum, because a deed is an engine event a mod cannot emit,
  so a string would buy openness onto nothing while costing a mission that
  names a deed that does not exist and never finishes.

- **`already_met` takes one `ObjectiveState` instead of three positional
  arguments.** It has two readers that must not drift, so every objective
  added used to cost a signature change at both; the next one costs a field.
  The board still asks it at depth 0 with an empty pack, deliberately: the
  board is *the sector's*, and answered from where the party is standing a
  `Descend(1)` would drop out of the pool the moment you went one frame
  down — and since the draw uses `swap_remove`, a pool one entry shorter
  reshuffles every slot. A board that changed as you walked.

- **The first decompile cannot fail.** A run of bad rolls would end
  onboarding permanently, which is the one thing an unbreakable chain must
  not do. The catalyst is still spent — the lesson that decompiling is
  priced in catalysts is the half that stays — and the branch reads the live
  mission's *objective* rather than a flag, so it is content-driven and
  disarms itself the moment the chain moves on.

- **Every mission opens with a full-screen briefing** carrying the
  contract's own name, objective and description, and pointing at `[4]`.
  One templated `assets/notifications/onboarding_mission.ron` filled at fire
  time rather than eleven files repeating what the contract already says, so
  a twelfth mission still costs one asset file and no Rust.
  `Game::notify_filled` and `Game::notify_with_detail` are both thin calls
  onto one private `queue_notification`, so the latch and the resolved push
  still live in exactly one place.

- **A contract's description ran off the popup.** The screen pushed it
  unwrapped while every other prose surface in `render/` goes through
  `description_rows`, and the width census measured only the row above it —
  which is how eleven descriptions several times longer than any other
  contract's shipped against a green suite. `contract_rows` is split out of
  `draw_contracts` so the census now measures what the screen emits rather
  than what the helper would have returned.

- **Three censuses keep the chain finishable.** A mission you cannot finish
  ends onboarding for the rest of the run with no key to press, so the
  shipped set is held to: build costs never outrun payouts, every `Deed` has
  a `note_deed` caller outside the tests, and every id a mission names
  resolves. A fourth holds the briefing to the screen it is drawn on, filled
  — the plain height census would be measuring `{description}`, seventeen
  characters where a paragraph goes.

## 0.13.56

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
The info column opens on the pane the party's locale asks for, the depot
picker is a table whose arrows point at its own columns, and a settled
contract names its payout on the screen that announces it.

- **The info column always opened on BASE, wherever the party was.**
  Standing out on the surface — with nothing base-shaped in front of you —
  meant reading a pane about the base while the crew you were walking with
  sat one keypress away. The tab now follows the party across the base
  boundary: walking out selects CREW, walking back in selects BASE, and a
  zone breach lands on whichever the far side wants. `App::sync_info_tab_to_locale`
  is the one writer, called from `after_tick` so every verb able to move the
  party reaches it through one hook rather than each setting the tab itself,
  and it only rewrites the tab when the side actually changes — so a manual
  `1`/`2`/`3`/`4` press survives until the next crossing. A fresh or loaded
  run opens on the true tab rather than waiting for a crossing that, from
  its point of view, already happened. The Stack rides the same boolean:
  underground reads as outside, so descending selects CREW and surfacing
  reselects BASE.

- **The transfer picker packed three figures into one unlabelled suffix.**
  A row read `3 / -5 .. +128`, which said nothing about which number was
  your pack and which was the shelf you were standing next to — the screen
  the whole feature exists for, and it was the hardest one in the game to
  read. It is four named columns now, `item | change | you | container`,
  under a header. `you` and `container` are still `App::put_available` and
  `App::take_available` — the same live ceilings, so a `you` reading 0 while
  the pack holds units is still the screen saying the other rows have spent
  the Depot's room — but the `-`/`+` decoration is gone, because the heading
  now says which end it is. A row nobody has touched reads a bare `0` in the
  change column rather than `+0`: a direction with a quantity of nothing is
  not a direction.

- **The arrows now point at the columns they move stock toward.** With the
  container as the rightmost column, Left pulls units off it toward you and
  Right pushes them from you into it — the reverse of what shipped, which
  had been inverted by request back when the screen had no columns to be
  inverted against. Shift still takes a row to the end its arrow heads for
  and Ctrl still halves the gap to it. The sign convention underneath is
  untouched: negative still puts and positive still takes, so the basket,
  the clamp and `transfer_items` never learned about the change. The
  controls page was telling the player the old arrows and now tells them the
  new ones.

- **A closed contract never said what it paid.** The alert screen took over
  the display to announce the Broker had settled up, and the figure reached
  the player only as a log line they had to scroll back through afterwards.
  The payout now sits on that screen, in the notification's own colour so it
  reads as a figure rather than as more prose, and it is built from the same
  call the log line makes so the two cannot quote different numbers. What
  carries it is a detail on the queued notification rather than a field in
  the `.ron` — a payout is what the firing site knows and a content file
  cannot author — and the screen has no scroll, so the census that says the
  shipped catalogue fits now measures every notification as though it were
  the one carrying the longest payout the shipped contracts can word.

## 0.13.55

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
A held contract reaches the HUD, two keys stop being invisible, a skipped
battle reveal makes a sound again, and a dig plan the crew cannot reach
stops soaking the crew.

- **Contracts are a fourth info-column tab, on `4`.** A held contract had no
  HUD presence at all — no pane row, no attention kind, nothing in the
  status bar — so the only always-visible figure was a bare count on the
  manifest stat sheet, and reading `8/10` meant opening the base menu and
  walking into the Contracts screen. Two rows a contract now: the name with
  its progress, the objective under it. The tail takes the attention colour
  once progress reaches the target, since a contract you can walk in and
  hand over is the actionable state, and the pane reads from anywhere the
  Stack included. The key is bound beside the other three rather than
  behind the Stack's own handler, so it works underground.

- **The Contracts screen's footer was painted outside its own popup.** One
  122-character sentence measuring 1175px into 1105px of body at 1280x720 —
  and since nothing clips horizontally, its last seven characters were drawn
  over the map beyond the border and "Esc to close" lost its verb. It is two
  lines now, and the abandon hint leads the second: `[A]` had been sitting
  at character 74, in the third clause, dim, the only bracketed letter on a
  screen whose every other shortcut is a digit. On screen the whole time and
  still unfindable, which was the bug. The overflow census the other popup
  screens already carry now covers this one too.

- **The controls page names the side column's digits.** `1`-`3` were never
  documented and `4` would have joined them unmentioned. The tab row was the
  only place they appeared.

- **Skipping a battle reveal was silent.** Pressing the action key mid-fight
  to dump the rest of the round jumped the reveal to the end without walking
  the loop that plays the swing cues, so a fight fought at any speed made no
  sound at all. A skip now plays the *loudest* blow among the lines it
  dumps — one cue rather than one per line, or a wipe is six clips inside a
  frame, and loudest rather than first so a round that landed a crit sounds
  like a crit however many plain hits came with it.

- **A dig plan nobody could reach starved the one cell they could.** The
  scheduler already dropped the boxed-in interior of a marked block before
  budgeting bodies for it, but left the *other* refusal — a cell with a
  perfectly good face and no route to it — below the cut that trims wants to
  the number of programs. A run of those (a pocket entropy sealed off, or a
  plan drawn past the walk cap) sorts first in tile order, costs no body when
  its turn comes, and pushes the reachable rim off the end of the list: the
  crew stands idle in front of a plan, and nothing says why. Both refusals
  are now answered above the cut, sharing the block that already dropped
  unreachable build requests. Asked one walk per want that would have been a
  Dijkstra field per face across a hundred-cell plan every tick, so the
  field is built from the *body* instead, once, leaving each want a lookup —
  in a connected base, one walk for the whole scheduler.

## 0.13.54

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
One rendering fault, reported from play, and the general test that should
have caught it two releases ago.

- **The log pane's filter header was cut through the middle.** `0.13.53`
  moved the vitals onto the log pane's top border and the
  `LOG All · Field · Base` header into the pane's body beneath it — but a
  border strip paints an opaque background *centred on* the line it rides,
  so the vitals reached nine pixels down into the body and wiped the top of
  the header a moment after it was drawn. 4.32px of a 13px row, across its
  whole width. The body now starts below what its own strips paint, at both
  ends of the pane, from one expression rather than a figure per edge.

- **The pane was asking for fewer rows than it had room to draw.** A
  pre-existing miscount, unrelated to the above and in the direction that
  costs the player news: the collapsed pane requested three message lines
  against room for four, and the expanded one seven against eight. Both are
  now what the pane actually shows, asserted through a real draw rather than
  by arithmetic agreeing with itself.

- **The keybar end had the same latent fault and no symptom.** The body's
  floor sat inside the reach of the keybar's own background quad; nothing
  was clipped only because row baselines happened to miss that band at every
  supported window size. It is now held by the same expression as the top
  edge rather than by coincidence.

This is the third release to fix one bug — an opaque background painted over
text already drawn — and each of the first two was let through by a test
that checked one named rectangle instead of what was actually painted. So
the fix that matters here is the test: `draw_log_pane` is driven through a
real painter and **no fill painted after a piece of text may overlap that
text's ink**, for every text the pane draws, in both its states. It reads
ink rather than layout boxes, because the leading around a line is space
nothing draws into and measuring it would raise a false alarm on every row.

## 0.13.53

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32
and no save field is added — the one thing this release does store, a
notification's "once ever" latch, goes on the cross-run profile rather than
the save. A new feature and five bugs, all six of them things the map screen
was doing in front of the player.

- **Full-screen notifications.** `assets/notifications/` is a data
  catalogue in the shape `assets/needs/` established: an absent directory
  loads silently empty, a malformed file costs that one notification and
  never the startup, and deleting the directory restores the game exactly as
  it was. `Game::notify` is the one door — it resolves the def, checks the
  repeat latch and queues a resolved value — and the queue is drained only
  from `Mode::Playing`. That single equality is the whole timing rule, so a
  fight, any picker and text entry all keep a notification waiting rather
  than having it thrown over them, and a mode added later is covered without
  being named. Achievements are a second *source* and not a second door: the
  card is built from the achievement's own name and description, so a new
  achievement earns one with no notification work. Two repeat policies, once
  per session and once ever; the third — once per *run* — is rejected in the
  doc comment, because a session-only queue would make it mean the first.

- **The expanded log moved the map.** SPACE's four extra rows were paid for
  out of the map pane's height, so every press re-laid the whole grid out
  under the player — a key whose entire job is "show me more of what I have
  already read" moved the thing being looked at. The map is now derived from
  the *collapsed* log at every window size and the log's bottom edge is the
  fixed one, so the expanded pane grows upward as an overlay.

- **The info column overhung the log pane by a row.** It ran to the window's
  bottom edge while the log stopped one row short of it — the reserve the
  keybar's glyphs straddle — leaving the two panes' bottom edges out of line.
  The column now ends where the log does.

- **The vitals were being cut in half, and then hidden altogether.** A
  border strip centres its background on the line it rides and reaches past
  it on *both* sides, so the log pane's `LOG All · Field · Base` header —
  riding the log's top border — painted over the lower half of the
  `MIT ATK STR DEC` readout riding the map's bottom border, baseline
  included, every frame. Two strips were sharing one line. The vitals take
  the border, because they are the readout that must not be covered, and the
  filter header goes back to heading the log's body, top-aligned with the
  messages it names. Mounting the vitals on the log pane also means they
  travel with it, so expanding the log no longer hides them.

- **Long log lines drew across the info column.** Nothing clips a row
  horizontally, so a line longer than the pane did not stop at its right
  edge — it ran over the roster beside it, reading as a fault in that column
  rather than in the log. Lines wrap now, measured against the UI face's own
  advance rather than a fixed column count, and what will not fit is cut
  from the **oldest** end: the pane is handed its rows oldest first, so
  stopping at the bottom would have dropped the newest news.

- **A structure's description ran off the deploy screens.** Both the deploy
  menu and the direction prompt emitted an authored description as one
  unwrapped row, and the widest shipped one ran some 1715px past the popup's
  body before vanishing — 303 characters against a budget of about 114. Both
  wrap now, through the helper the perk and research pickers were already
  using rather than a second copy of it.

## 0.13.52

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32
and no save field is added. Four bugs, three of them things the game was
already doing wrong in front of the player and one of them a run-ender for a
base that had only just been founded.

- **An opening base is too thin to raid.** A GC Entropy Sweep is meant to be
  attrition the base absorbs, and a base with two bodies has nothing to
  absorb it with — early raids were taking the whole staff down and leaving
  every program `Downed`, with nobody standing to defend against the next
  one. `tuning::RAID_MIN_BASE_STAFF` (3) is the floor `Game::raid_check` now
  measures against, counted through the existing `Game::base_staff` and so
  through `party::role_of`, with `Downed` bodies excluded — a base already
  reduced to wreckage is exactly the state this closes. The gate sits
  **after** the RNG roll, `maybe_spawn_wild_creature`'s rule, so it shifts
  no seeded stream. Eight existing raid tests ran with **zero** staff and
  were silently gated out by it; they stand bodies up now rather than the
  gate being weakened to suit them.
- **A downed program says so.** One benched by a Forgiving death read as
  `idle` on the manifest and on the examine line, which is the same word a
  companion nobody has ever posted anywhere gets. `bench_or_dissolve` calls
  `detach_from_play` *before* inserting `Downed`, so the program has no
  party slot, no wield slot and no `Task` — every branch in
  `Game::program_activity` found nothing and fell through. One early return
  in that derivation, which both surfaces already share, so the fix lands on
  both at once and costs the renderer nothing. The word is `recovering`,
  which is what `run_repair_bays` already says when a Bay finishes the job.
- **The status bar was eating the map pane's title.** A border strip centres
  its background quad *on* the line it rides, reaching half its own height
  above it, and `map_pane.y` sat exactly on the status bar's bottom edge —
  so the bar's opaque fill, drawn after `draw_map_frame`, took the top of
  SECTOR MAP and the threat readout with it. The panes below the bar now
  start `TOP_STRIP_CLEARANCE_RATIO * m.small()` further down, built off
  `strip::PAD_RATIO` rather than a copied literal so the two cannot drift.
  The bar's own rect is unchanged. The test locates the **painted quad**
  through paint order rather than asserting a layout number, because a
  layout figure passes against the bug.
- **SPACE doubles the map's log pane** to eight rows and back, off
  `App::log_expanded` on `stack_zoom`'s precedent. Bound in
  `handle_playing_key`'s top match, which runs before the hand-off to
  `handle_stack_key`, so it works underground too rather than falling
  through that dispatch's `_ => {}` the way `r` once did. The pane's row
  count is already derived from its height, so the taller pane fills itself.
  Not advertised on the keybar — that strip has no slack at 1280x720 — so it
  is documented on the controls page instead.

## 0.13.51

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32
and no save field is added — but **this release changes how hard fights
are**, in one direction and mostly underground, so a run in progress will
notice.

Fights had two ceilings and neither bounded the other. `Game::group_pack`
caps a fight per group (`MAX_GROUP_SIZE`) and per group count
(`MAX_ENEMY_GROUPS`), so what one intrusion could actually field was those
two multiplied — four hundred programs in principle, and thirty-three
against a party of four at zone 3, five frames down.

Two things kept that hidden. A **surface** fight is whoever `gather_pack`
found standing together, which measures 2.6 bodies at zone 3 against a
ceiling of twelve, so the missing bound was also the bound nothing ever
reached. A **Stack** ambush is not: `stack_encounter_pack` takes one species
pick and one full group roll per group slot the curve allows, so it fills
the ceiling by construction. The same curve was producing a shallow-Stack
fight four times the size of the surface fight beside it.

`0.13.21`'s change to `Game::danger_steps` — the zone step added to the
depth step underground — is what turned that from large into fatal. It was
aimed at which species a frame may field, and `danger_steps` also feeds both
group-size curves. Its effect on lair fights was measured at the time; its
effect on the ordinary wandering pack was not, and the Stack has been
unwinnable from **depth 3 down** ever since: 2.0% at depth 3 and 0.0% at
depth 5 for a party at the zone level cap, over 200 staged fights each.

- **`tuning::MAX_PACK_BODIES` is the bound the other two never expressed.**
  Eight bodies across every group in the fight, trimmed off the largest group
  each pass — so a mixed pack keeps its variety and loses its depth, four
  species two deep rather than one species eight deep. The bodies it turns
  away **stay standing on the map** and are met on the next bump, exactly as
  the surplus past `MAX_ENEMY_GROUPS` already did. Nothing is despawned and
  no encounter is skipped.
- **What that does to the Stack:** depths 2 through 5 now run 100%, 93.5%,
  67% and 39.5% for a party at its zone's level cap — a curve that descends,
  where before it fell off a cliff between the second frame and the third.
- **Two fights outside the Stack move with it**, because the bound is on
  every fight rather than on the Stack's alone. A zone-3 depth-3 lair goes
  from 71% to 91%, and a **surface** pack at zone 6 — which could roll 17.5
  bodies — from 64% to 100%. The late-game surface has never had a real
  ceiling on how many programs may engage at once, and now does.
- **The arena could not field the party the game permits.** `set_level`
  clamped every scenario's companions to `arena_level_ceiling()`, which was
  the live ceiling until Kernel Rings stopped buying levels in `0.13.36`;
  since then it has sat *below* the zone cap from zone 2 on, silently
  clamping **upward** authoring. A zone-3 scenario asking for the level-23
  party that zone allows was staged at 12. The ceiling is now the higher of
  the two, which keeps the zone-1 property that figure was introduced for —
  five shipped scenarios author `level: 12` and zone 1's cap is below it.
- **A benched companion counts as one that went down.** `companions_downed`
  read `hp > 0`, and `0.13.36`'s Forgiving benching leaves a dead program on
  the roster at HP 1 — so the arena's "companions down" column has been
  structurally zero since it landed.

`docs/measurements/2026-08-28-stack-depth-curve-after-danger-steps.md` has
the sweep behind the constant, including the isolation of the three terms
that feed a deep-Stack fight — volume, the species window and the per-body
stat step — and the finding that the species window is a **threshold rather
than a gradient**: pulling it back one step or two does nothing at all, and
three steps is a cliff.

## 0.13.50

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32,
and this release adds no save field, changes no formula and moves no number.
`Perk`'s variant order is save format — bincode encodes an enum positionally,
so `PlayerSave::unlocked_perks` holds indices into it — and nothing here
touches that order.

An internal change with no gameplay effect, recorded because the seam it
moves is one somebody will have to reason about later.

Every perk's effect was a hook written at the formula that used it: the
mining roll, the hunger multiplier, a decompile's HP term, recipe costs, a
Trace rise, roster capacity, a kill's salvage, the base repair rate, a
compiled copy's quality floor, a direct `Stats` write. So the question "which
formula does this perk touch" was answerable only from the enum's doc
comments — prose, which cannot be kept in step with the code beside it.

`perks.rs` now holds one named query per perk and the call sites read them:
the module owns the answer, the caller owns the application, which is the
inversion `needs::strain` and `party::role_of` already use. Ten perk tuning
constants went dead at their old call sites in the process, which is the
evidence the hooks actually moved rather than being renamed in place.

- **The queries come in two families, and the difference is load-bearing.**
  Most take the player's `Perks` and name their own variant, so no call site
  says `Perk::` at all any more. `mining_roll_bonus` and
  `quality_floor_bonus` take a bare **level** instead, because
  `CycleModifiers` and `CraftOrder` carry that level to a formula whose
  subject may be a program rather than the player — a `Perks` argument there
  would quietly hand a program the player's investment, which is the exact
  failure `CraftOrder` was built to make impossible.
- **Three perks are applied at purchase rather than read at a formula**, so
  they get the only shape they can. `purchase_stat_gain` says what buying
  `Attacker`, `Defender` or `Buffer` grants — including `Buffer`'s
  percentage-of-current-max and its floor — while `unlock_perk` stays the one
  writer of `Stats`.
- **The floors moved with their perks.** Power may be switched off entirely
  by enough levels of Low Power Mode, because a Recharger Node already
  deletes that pressure; Trace never may, because it is the Stack's only
  escalation and descending has to keep costing something; and a recipe line
  never reaches zero, which is the infinite-Credit fault the caravan's price
  floor guards from the other side. Each of those used to live at the call
  site, where it read as an implementation detail rather than as the design.
- **The module is a census, and it is enforced.**
  `every_perk_has_a_query_that_answers_what_it_is_worth` is exhaustive on
  `Perk`, so a nineteenth variant fails to compile until it has a home there,
  and each arm asserts the query actually *moves* — a perk wired to a zero
  constant fails as loudly as one with no query at all.

## 0.13.49

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
The two new save fields are additive behind `#[serde(default)]`, and the one
a pre-existing file cannot carry is answered by deriving it rather than
defaulting it — see below.

Every program on your base roster is now somebody in particular, and none of
them will tell you who.

A program is born with one of five **dispositions**, derived from its own id
and then kept. Nothing draws it and no screen names it. You find out the way
you find out about people: one body is always the first to wander off to the
Sandbox, another grinds on long after everyone else has gone; one lets a bad
week go and another is still sore about a machine that jammed on it a
thousand ticks ago.

It moves two things and nothing else — how fast a program runs its reserves
down, and how hard what it remembers lands on it. `Languid` and `Dogged` are
the poles of the first, `Amiable` and `Abrasive` of the second, and `Steady`
is neutral on both. There is no work-rate dial: a Languid program does not
mine any slower per tick, it is simply off being seen to more of the time,
which is what "doesn't want to finish tasks" actually looks like from the
outside.

On top of that, a program can now **have had enough**. Morale — the signed
sum of everything it remembers — has two lines under it. Cross the first and
the program sulks: it still works, but not at a machine it holds a grudge
against, and if it is already standing at one it walks off and a willing body
takes the post on the same tick. Cross the second and it downs tools
altogether, leaves the labour pool, and the work order screen's shortfall
grows by one with the reason sitting on its manifest.

It comes back when its grudges have faded and better days have landed on top
— there is no amenity to walk to for this one, and nothing you can build that
fixes it directly. The gap between going out and coming back is deliberate:
one number would have a body dropping its tools and picking them up again on
alternate ticks.

The two features are the same feature. An Abrasive program's grudges are
scaled up, so it reaches the line on strictly less history than an Amiable
one — which is how a hidden temperament becomes something you can actually
see, through *when* somebody breaks rather than through a label.

An existing save gains all of this. A file written before dispositions
carries no disposition, and the load path derives one from each program's id
through the same formula a fresh program takes — so your base comes back with
the roster it would always have had, not a crew of identical neutral bodies.

**Both morale thresholds are unmeasured.** Morale has no natural scale and
the balance simulator models no base production at all, so the two numbers
were chosen against the shipped memory valences rather than against a played
run. They are the first thing to revisit.

## 0.13.48

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32,
and this release adds no save field at all — the walk below is drawn from a
queue the engine drains every frame and never writes to disk.

A squad you dispatch from the Relay used to vanish from the map on the frame
it left. That is exactly correct as simulation and reads as a bug, so the
bodies now walk: out from wherever each was standing, across the base pocket,
to the anchor, and out of phase. The return is the same thing backwards — in
through the anchor and back to the tile each body left from. A squad's
members set off one after another rather than together, so four programs read
as a file leaving.

- The walk is **purely cosmetic**, and that is the load-bearing decision.
  Nothing in the simulation stands on any of those cells: `dispatch_sortie`
  and `return_sortie` each queue one `resources::TransitCue` per body — a
  glyph and the cells it walks through — and forget it in the same call.
  `Game::take_transits` drains the queue, `take_effects`' counterpart, and a
  frontend that ignores it draws the game exactly as it did before.
  `ProgramRole::Sortie`'s five omissions are untouched, which is the whole
  point: giving an away program a live `Position` for a few ticks is the
  "which space is this?" bug class that role was built to stay out of.
- Departure and arrival are the **same cue with its ends swapped**, so there
  is no direction field to get wrong.
- The walk follows the dug ground rather than the straight line home.
  `base_space::transit_path` pathfinds on `BaseGrid::walkable` alone and
  admits no blocked set — the door is the cell the Home stands on, so a walk
  that refused occupied tiles could never reach its own destination. A body
  ghosting past a machine for a fraction of a second is invisible; one
  ghosting through rock reads as a rendering fault.
- The draw sits behind the gate a raid's flash already sits behind. A cue
  names base-space cells, so one drawn while the pane is showing the zone
  surface would file a squad of glyphs across whatever unrelated ground
  shares those numbers, the party's own tile among them.
- A body standing somewhere that is not walkable base space gets no walk at
  all rather than a straight line — the ordinary state of a program adopted
  on the surface that has not drifted into the base yet.

A cue is drained on the frame it is queued, so a return walk you are not home
to watch is dropped rather than saved for later. That matches how a raid's
flash already behaves, and it is what keeps the whole feature free.

## 0.13.47

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release adds two save fields, both additive, so a save written before it
loads with nobody away from the base.

The idle half of the roster gets a second front. A developed program you are
not fielding has done exactly one thing until now, which is work the base;
this is the engine for sending a squad of them out to fight somewhere instead.

**There is no screen for it yet.** Everything below is reachable from the
engine and covered by tests; the Relay's board and the dispatch picker are a
second piece of work. Nothing in a running game changes until those land.

### Added

- **Sorties.** A squad of base staff can be dispatched from a Dispatch Relay
  to a named site, where it fights a run of real battles off-screen over a
  number of ticks and comes home with what it could carry — or with fewer
  bodies than it left with. The fights use the same swing and routine
  resolution a fight in front of you does: the same hit ladder, the same
  damage bands, the same mitigation, the same Power costs.
- **A trip pays for itself four ways.** The bodies work no machine for the
  whole trip and the base reads as short-handed while they are gone;
  provisioning is charged from base stock at dispatch; the programs take real
  damage and can be lost; and a kill off-screen pays less than the same kill
  with you in the fight. Power does not recover in the field and there is no
  rest out there, so Specials taper across a long trip — which is what makes
  a twenty-fight site genuinely harder rather than merely longer.
- **The trip aborts on the first casualty.** Remaining battles are skipped,
  the survivors keep what they earned, and the return travel still runs —
  there is no teleport home. Under Forgiving the program that dropped comes
  back benched and walks itself to a Repair Bay; under Permadeath it is gone,
  but the abort caps the disaster at one.
- **`assets/sorties/` is a new content directory** — one `.ron` file per
  site, naming it, how dangerous it is relative to the sector you are in, and
  how many fights it takes. Four ship. Deleting the directory restores the
  pre-sortie game exactly, the same supported way deleting `assets/needs/`
  does. `assets/sorties/README.md` is the schema.
- **The Dispatch Relay and its research node**, both data. `StructureDef`
  gains `dispatches_sorties`, so a mod can add a second dispatch structure or
  move the gate without touching the engine.

### Changed

- **The Relay's board is derived, never stored**, the way a Broker's is: it
  is recomputed from the world seed, the sector and the clock, so reloading
  reproduces the same offers, there is nothing to re-roll, and it rotates on
  its own. Each offer's fight count is fixed the moment it appears, which is
  what lets the trip's length be quoted before you sign for it.

## 0.13.46

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release changes what a battle sounds like and what the dig crew does when
it runs out of substrate, and stores nothing.

A swing you can hear the shape of, and a dig crew that stops standing over a
cell it has nothing to floor with.

### Added

- **Hits, crits and misses sound different**, and each cue plays as that
  swing's own line is revealed in the log rather than all at once the moment
  the round resolves — so what you hear tracks what you are reading. A fumble
  sounds like a miss, since the line already narrates how badly it went.

### Fixed

- **A dig crew that runs out of Blank Substrate no longer strands the
  program.** A body posted to floor a cut cell stayed on it for the rest of
  the run, waiting on a material only a Lathe makes — which that same body
  would have had to go and run. A floor job with nothing to lay is set aside
  until the substrate exists, and the program goes back to useful work.
  Cutting is unaffected: a marked wall still comes down with an empty store.
- **A base that runs dry twice now says so twice.** The crew named an empty
  substrate store once per cell and then never again, so a shortage that came
  back was silent.
- **A base whose last job just became unworkable stands its worker down.** The
  quiet-base check could pass while a program still held a posting that had
  stopped existing, leaving it parked there.

## 0.13.45

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release changes what the game says when it refuses you and stores
nothing.

A rest that does not happen says why. `r` was reported dead in the Stack
again after it was bound there; it is not dead, but a rest that could not
happen accounted for itself nowhere the player was looking, which from
outside is the same event as a key that does nothing.

### Fixed

- **Every rest that is refused now says so**, on the status banner and in
  the log, on the surface and underground alike. Three of the four ways a
  rest could fail said nothing at all, and the fourth spoke through a log
  line the channel filter could hide.
- **An empty stack in the pack is no longer mistaken for a rest charge.** A
  slot that had been emptied still counted as something to power down with,
  so the refusal that should have named it was skipped and the rest failed
  silently one step later.
- **Every other refusal on the map and Stack screens reaches the log too.**
  Both screens were writing the banner directly, so a refused verb aged out
  of view after a few seconds and left nothing to scroll back to.

## 0.13.44

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release changes what colours the map draws in and stores nothing.

The marks and washes drawn over the map's glyphs are drawn from the HUD's
palette too. The tail of the previous release, which swept the glyphs and
left the overlays on top of them alone.

### Changed

- **An Excavation plan is blue.** Its washes, the box being previewed and the
  ring the party wears with cutting tools armed were an amber close enough to
  the machine-attention yellow that a cell you had marked and a machine that
  had stalled read as the same news. A plan is you having acted; the yellow is
  the base asking you to.
- **A stranded machine's mark is the attention colour**, the same one its
  glyph wears, and a staffed machine's is the green a running one has. What
  separates the two marks is still the blink and the colour together.
- **A raid's flash is the same red the rest of the screen uses for harm.**
  Every cue about something being attacked now comes from one place, so a
  retune moves all of them at once.

## 0.13.43

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release changes what colours the map draws in and stores nothing.

The map's glyphs are drawn from the HUD's palette. Phase 6, and the last of
the Paned Command HUD: the entity colours were the one part of the screen
still painted from the renderer's old table.

### Changed

- **Every entity on the map is drawn from the palette's sixteen entries.**
  The eleven colours species and structure files author resolve through one
  table, so a program's glyph is the same colour on the map and on every
  screen that shows it. Nothing about authoring a colour changed: a mod names
  the same eleven.
- **The player's `@` has a colour nothing else can take** — br cyan, the
  palette's player role, read off being the player rather than off the cyan
  the player happens to spawn with.
- **A stalled machine asks for attention in yellow instead of red.** A
  clogged, stranded or unpowered machine is the base asking you for
  something, and it now wears the same colour the status bar uses to say so.
  Red on this screen means hostility and inbound harm, and nothing else. A
  machine that is merely waiting — short of input, or with its program
  walking back — keeps the dimmer yellow it had, because "go and fix this"
  and "this is fine, give it a moment" are the whole information in that
  colour.
- **The danger ladder climbs in hue.** A creature's glyph still reads green
  through red by how badly it would beat you, with the middle rungs moved so
  that no two of them read as the same colour under the map's dimming.

## 0.13.42

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release draws the right-hand column differently and stores nothing new.

The column's three tabs get real contents, and the old status panel comes
out. Phase 5 of the Paned Command HUD, and the end of the ~250-line
undifferentiated text dump the whole design was aimed at.

### Added

- **BASE, CREW and PACK have bodies of their own.** BASE lists the base's
  structures with the program on each and what it is holding, then
  production, defence, the build queue and the programs the roster has
  spare; CREW lists the roster against its capacity and then every program
  with its level, Integrity and what it is doing, party members first,
  followed by the routines currently running; PACK lists what the party is
  carrying.
- **What does not fit is counted.** The column does not scroll, so a pane
  that runs past its body says `+N more` on the last row rather than
  drawing off the bottom edge in silence. The notice is itself a row and
  is budgeted for, so it cannot be the thing that overflows.
- **A collapsed tab says what its pane holds** — the base's structure
  count, the crew's headcount, the units in the pack — instead of
  `nominal`. A condition still outranks the summary, and both are built
  from the same data as the open pane's rows, so a bar and the pane it
  stands for cannot disagree.

### Changed

- **The four stats, the bars, the zone and the stock strip are no longer
  drawn twice.** The vitals strip has carried them since phase 2 and the
  status bar since phase 1, so what the old panel drew was a second copy.
  The rows that had nowhere else to be — the roster, the running routines
  and the pack — are what the three panes are.
- **A companion's buff row names its holder on its own line** in the
  column, as it already did, and the battle screen keeps naming it inline.
  The column is a fixed slice of the window and cannot widen for a tag.

### Removed

- **`draw_status_panel`.** The column it drew is now the info column's open
  tab, and every row it held is either in one of the three panes or was
  already on the status bar or the vitals strip.

## 0.13.41

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
which pane of the info column is open is view state, exactly as the log's
filter is, and nothing here is written to a save.

The info column wears three tabs and hides nothing behind them. Phase 4 of
the Paned Command HUD, and the one it is really about: a single derivation
answers "what needs the player right now", and every surface that could
report it reads that same answer.

### Added

- **A status-bar badge saying what needs you** — the most urgent condition,
  upper-cased, with the key that opens the screen it is acted on from. When
  nothing holds it reads `ALL NOMINAL` in green, because the calm state is a
  real state and is drawn rather than left as a gap.
- **The info column is tabbed: `1` BASE, `2` CREW, `3` PACK.** One pane is
  open and the other two collapse to a summary row each. The keys work
  underground as well as on the surface.
- **A closed pane can never hide something you need to act on.** Its tab
  wears a `!` and its collapsed row says what the condition is, in yellow
  for work and red for harm. The badge, the tab marks and the collapsed
  rows are three readouts of one derivation, so they cannot disagree about
  what is going on.
- **Four conditions are reported**: a structure below full Integrity, nodes
  standing without a program, unspent Perk Points, and a roster at capacity.
  A damaged structure sorts above the rest, since the badge shows only the
  leading one.

### Changed

- **The status column is now the open tab's body**, drawn inside the
  column's frame rather than drawing its own. What it holds has not moved
  yet — that is the next phase — so it reads as it did, in a slightly
  shorter box.

## 0.13.40

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this release draws the same screen differently and stores nothing new.

The log pane wears its filters and its keys on its borders. Phase 3 of the
Paned Command HUD: two rows come off the pane's body and onto its frame, and
a channel gutter goes down its left edge.

### Added

- **A channel gutter down the log.** Every line is tagged with where its news
  came from — `FIELD` or `BASE`, the two the `f` key already cycles — with a
  pickup marked `GAIN` and an inbound sweep marked `ALERT`, because those two
  are their own news whatever channel they arrived on.

### Changed

- **The log's filter row moved onto the pane's top border**, where it costs
  no body row. That is one more line of log at every window size, and the row
  now names `L history` alongside `f cycle`.
- **The four-line block of eighteen keys under the status column is one row
  on the log pane's bottom border.** It is ordered by priority and measured:
  what does not fit is dropped from the end rather than drawn off the panel
  in silence. Movement, `b`, `i`, `? help` and `q menu` are never dropped,
  `? help` in particular being where every key the bar had to cut still
  lives — all of them are in the manual's Controls page. `t trade` and
  `s save` do not fit at any supported window size and stay cut, as the
  design handoff had them.

### Fixed

- **A log line can no longer draw through the keybar.** With the filter row
  off the body the log runs a row deeper, toward the border the keys are
  mounted on, and the keybar paints last — so a collision would have shown as
  keys sitting on top of a half-covered line rather than as anything a reader
  would look for.

## 0.13.39

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the run's spent freebies are a new save field behind a `#[serde(default)]`,
so a save written before this loads with its free Broker still owed.

The run's first Contract Broker costs nothing. Contracts are what onboards a
run — the Broker has always been behind no research for that reason — and
its five Core Fragments were the last thing standing between a new player
and the board.

### Added

- **A Contract Broker is free to deploy until you have one.** The build menu
  quotes it as `free` and the deploy prompt says `Free to deploy`. The
  waiver is spent when the crew actually raises it, so filing a request and
  then cancelling does not burn it, and a second request filed alongside the
  free one is quoted in full.
- **Structures can be authored free-the-first-time**, a `first_free` flag in
  `assets/structures/*.ron`. One per run rather than one per zone: the base
  travels through a breach and so does whatever is standing in it. The
  shipped Broker is the only structure that sets it.

### Changed

- **A structure removed for a bill nobody paid still refunds the usual
  share** of what its file says it costs — one Core Fragment, once, for a
  demolished free Broker.

## 0.13.38

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Nothing here is stored — this is what the HUD draws, not what it keeps.

The map pane is framed, and it wears the player's vitals on its own borders
rather than spending map rows on them.

### Added

- **`SECTOR MAP` and a threat readout ride the map pane's top border.** The
  threat line says how many hostiles are on the map and whether anything in
  the base is contributing raid defence.
- **The vitals strip rides its bottom border** — Integrity, Power, level and
  XP as meters, then unspent perk points, mitigation, attack, strength,
  decompiler and whether mining is armed. On a narrow window the strip drops
  from the end rather than running off the pane, so the meters are always the
  part that survives.
- **Unspent perk points are called out in the attention colour**, and the row
  is absent entirely when there are none to spend.

### Changed

- **The frame is the same underground.** The Stack corridor draws into the
  same framed pane and carries the same vitals; only the map's contents
  change.

### Known rough edge

- Integrity and Power are drawn twice for now — once on the new vitals strip
  and once in the old right-hand panel. That panel is replaced outright in a
  coming release and the duplication goes with it.

## 0.13.37

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Nothing here is stored: the HUD work is layout and colour, and the two fixes
below change what is drawn rather than what is kept.

Groundwork for a redesigned HUD, plus two base-space drawing fixes that had
not yet been released.

### Added

- **The status bar.** One row across the top of every screen that draws the
  world behind it, carrying who and where you are — identity, zone, position
  and tick — alongside what the base is holding. It absorbs the stock strip
  rather than sitting above it, so the readout costs no extra row.

### Changed

- **The HUD's geometry is derived in one place.** The map pane, the log pane
  and the right-hand column are no longer three independent fractions of the
  window. They come from one calculation, which is what lets the column run
  to the bottom edge and the log stop at the column's left edge instead of
  passing underneath it.
- **The right-hand column is narrower and full height.** Its contents are
  unchanged for now and will look cramped; the panes that belong in it land
  over the next few releases.

### Fixed

- **A builder and a digger are drawn while they work.** A tamed program
  holding a build or dig post disappeared off the map for the whole job, so
  filing a build request made a program vanish and a structure appear a few
  hundred ticks later with nothing having visibly walked, fetched or built.
  Marking a wall did the same to whoever went to cut it. The "somebody is on
  this job" mark was missing from both ends of a build posting, and is back.
- **The build caret bounces in the middle of its slab** instead of sitting
  high in the tile. It was riding the staffed mark's upward-only curve, whose
  rest position is an inset off the tile's bottom edge rather than the centre
  of a slab.

## 0.13.36

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the one field this adds to a creature is additive behind `#[serde(default)]`,
and nothing else in either feature is stored at all. A save written before
this comes up with every program upright, and a program already developed
past its zone's new cap keeps every level and every stat it earned.

Two changes that meet in the middle. Losing a program in a fight is no longer
losing it, and how far anyone can develop is now a property of how far you
have got rather than of what you are.

### Added

- **A program that dies under Forgiving is benched, not destroyed.** It comes
  out of your party downed, keeps its place on the roster, and walks itself to
  a **Repair Bay** — a new passive building that writes it back to full
  Integrity a tick at a time and stands it off the bench when it is whole. No
  worker, no input, no research: 16 salvage and somewhere to put it.
- **Without a Bay standing, a downed program stays down.** It lies where it
  fell, off the labour pool, still holding its roster slot. Selling it or
  extracting a routine are what free the slot in the meantime, and the refusal
  says so rather than leaving you to guess.
- **XP earned at the level cap buys Perk Points** instead of vanishing. The
  price rises with every perk you already hold, so the exchange tapers rather
  than becoming a second grind; whatever is left unspent turns into real
  levels the moment a breach lifts the cap.

### Changed

- **One level cap over the whole party, and the zone sets it.** The player had
  no ceiling at all and a companion had a personal one; both now stop at the
  same number, which rises with every sector breached. A companion can stand
  level with you, which is what makes developing one worth the XP.
- **A Kernel Ring buys talent tiers rather than levels.** Every ring still
  opens two, three rings still buy exactly one full tree, and a program you
  built before this has precisely the points it had. What changed is what the
  ring is *for*: depth in one program's tree, not permission to be bigger than
  its roster-mates.
- **Who is in your party is decided at home.** Joining and standing down both
  need base space now. Wielding a program in the field is untouched.
- **A raid that kills a defender benches it too**, on the same terms as a
  fight.

## 0.13.35

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
both fields this adds to a creature are additive behind `#[serde(default)]`,
and a program in an older file simply comes up with full reserves.

Your programs want things now. Not much, and never as a new way to lose — a
body that has been at it too long walks off, sees to itself, and comes back.

### Added

- **Programs on the base staff carry reserves that fall on their own**, and
  faster while they work. Two ship: **Coherence**, which a process spends by
  running, and **Slack**, which it spends by running the same thing over and
  over. Both are `.ron` files in `assets/needs/`, so a third costs two files
  and no Rust — and deleting the directory restores the game exactly as it
  was.
- **Two buildings answer them.** The **Defrag Bay** puts a program back in
  order; the **Sandbox** is scratch memory nobody is watching. Neither takes
  a worker or an input, and a program whose reserve has run critical walks to
  one on its own, stands there until it is whole, and goes back to work.
- **The manifest's WORK box says where a program stands**, in words rather
  than a number — steady, fraying, strained, critical — with what it is off
  doing beside the need it is doing it for. The roster's activity line says
  the same thing instead of calling a program on an errand "idle".
- **Programs that idle together think better of each other.** A new
  `idled_with` memory, written once when a program finishes at an amenity,
  naming whoever else was there.
- **A program with nowhere to go says so, once, and holds it against the
  corner it was standing in** — the new `frayed_here` memory. Nothing in the
  base servicing a need and nothing being able to *reach* what does are
  different complaints, and the base tells you so in different sentences.

### Changed

- **A program off shift is not counted as a body the base has.** The work
  order screen's shortfall grows while somebody is seeing to itself, which is
  the reading you want: the base is short of hands, and the manifest says
  why. A body already carrying a load finishes the delivery first.
- **A run-down program extracts less reliably.** One capped term in the same
  roll morale already rides, and it reaches extraction only. A program with
  what it needs contributes exactly nothing, so the shipped rates are
  untouched.
- **The manifest's MOVES box trims at three rows instead of four**, to pay
  for the need rows. Nothing shipped has more than two moves.

## 0.13.34

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
a build request is a new entity in the save and every field it added is
additive behind `#[serde(default)]`, which is exactly the case field-named
RON was adopted to make free.

Building is something the base does, not something you do. A deploy is a
request your crew fetches for and raises, and so is an upgrade.

### Added

- **Every structure but the Home is now filed rather than built.**
  `Game::place_structure` answers every refusal it always did — researched,
  in base space, standing on laid floor, cell free, under `max_deployed` —
  and then spawns a `components::BuildSite` on the cell carrying the resolved
  bill of materials. `schedule_base_labour` posts a body to it ahead of every
  work order, and that body walks to whichever shelf or pack holds what the
  bill calls for, carries `HAUL_CARRY_CAPACITY` at a time, sets the load down
  on the site and raises the structure over `BUILD_TICKS_PER_MATERIAL` ticks
  per unit of material.
- **The Home stays a player verb**, and that is not a special case to be
  tidied away later: founding is the one build with nobody to ask, since base
  space does not exist before a Home stands — no roster inside it, no shelf to
  fetch from, and `require_base` refusing entry for want of the Home you are
  building.
- **Nothing is charged at filing.** A request the base cannot afford yet is a
  legitimate thing to file — production catches up, and the crew starts the
  moment the last unit exists — so the old shortfall *refusal* at the menu
  became a shortfall *report* from the builder standing at the site, said once
  per drought rather than once ever.
- **`d` plus a direction calls off the request on that tile** when no
  structure stands there: the same gesture as demolishing, because it is the
  same question the player is asking of that cell. No confirmation step —
  nothing is destroyed, and the units already carried there go straight back
  onto a shelf.
- **The map paints a site as a dark slab with a bouncing caret**, and examine
  says what is going up, what is still to be fetched and who is on it.
  `views::BuildOrderRow` is the one derivation all three read, so the map, the
  examine line and the order list cannot report a percentage the crew
  disagrees with.
- **The build menu's `(have/need)` column counts the base's shelves as well
  as the pack**, since both are stores a builder fetches from.

### Changed

- **Upgrading a structure is a build request too.** It was the last structure
  cost paid out of the player's own pack — the complaint arrives the moment
  you stand on a full Depot beside a Mk1 Lathe and are told "Not enough Cache
  Grain", with the stock strip along the top of that very screen saying the
  base is holding it. `Game::upgrade_structure` keeps every refusal it had in
  the same order, drops the shortfall check and the charge, and files a site
  on the machine's own tile; the crew fetches the bill and the tier lands when
  the work is done. The upgrade menu now quotes the pack **and** the base's
  shelves, or it would price the job against a store the verb no longer reads.
- **The machine keeps running while its upgrade stands.** Standing it down for
  its own upgrade brings back the deadlock closed below, on a base that files
  three at once — the machines making the materials the requests are waiting
  on are the ones switched off. The site carries no glyph for the same reason:
  the machine is still there and still drawing that cell, so a build frame
  over it would be a lie about the tile.
- **One component covers both jobs.** `BuildSite::goal` is `New` or
  `Upgrade { to_tier }`, and exactly one step branches on it — completion.
  The crew, the walk, the scheduler wants, both announcement latches, the
  reachability check and the refund on cancel are shared. A site names a
  **tile** and never an entity, so a machine destroyed underneath its own
  upgrade leaves nothing dangling.
- **Materials are not spent until the structure is raised.** They leave their
  shelf when a builder picks them up and stand on the cell until the job
  finishes, which is what makes calling a request off a refund of goods that
  still exist rather than a rebate.

### Fixed

- **A request the base could not supply deadlocked it.** Build wants outrank
  production, so a one-program base posted its only body to a site with
  nothing to fetch, the body stood there, and the Mining Node that would make
  the very material the site was waiting for was never worked again. The crew
  said "nothing to raise it with" once and the base was finished for the run
  — reached by a player doing the supported thing, since filing a request the
  base cannot afford is the whole reason filing charges nothing. A dry site
  now drops out of the want list until a next unit exists; the flicker that
  admits is the behaviour you want, mine a unit, carry it over, go back to
  mining.
- **An unreachable request starved the base silently.** A walled-in site
  posted a body, lost it in the same tick when the walk failed, and was handed
  the same body again on the next — forever, while the production want the
  truncation cut to make room for it went unfilled and nothing was logged. The
  reachability check now sits *above* the cut, asks the staff, and says so
  once.
- **The dry report is said once per drought, not once per request.** The latch
  was documented as clearing when a source appears and never did, which for a
  build — waiting on a bill of several items over many trips — leaves a base
  that ran out early silent about running out later.
- **A pending upgrade no longer eats one of its kind's deployment slots**, and
  a machine destroyed by a sweep or demolished by the player takes its pending
  request with it, handing back whatever had already been carried there.

## 0.13.33

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

The base stock strip lists banked pools, and a machine holds its row at zero.

### Fixed

- **The stock strip never drew the base's banked pool.** `Game::base_stock`
  walked output buffers alone, and a banked item never reaches one —
  `deliver_payout` sends a Research Node's yield straight past the node's own
  `output` into the player's bank, which is the whole of what `ItemDef::banked`
  buys. So Research Data, the only banked product in the game, had no row on
  the strip at all, and `research_data.ron` had been carrying an `abbrev` of
  `R` for the strip's benefit the entire time it could not be drawn. It is
  folded in **by the flag and never by name**, and `stock::output_buffers` is
  deliberately *not* widened to reach it — a work order for a banked item is
  still refused on the grounds that no shelf holds it.

### Changed

- **A pile keeps its row while its buffer is empty.** The strip used to list
  only what the base was holding right now, so a tag appeared and vanished as
  haulers cleared shelves — the same reshuffling under the reader's eye that
  sorting by quantity would cause, on a readout whose whole job is being
  glanceable. A row now exists if the base holds any of an item **or** is set
  up to make it: `stock::producible` seeds a zero for each deployed
  structure's `work.produces` and its `assembles.item`. Both halves, because
  an assembler declares no `work` block at all, so a `produces`-only rule
  would leave every crafting machine in the base off the strip until its
  first unit landed.
- **Deliberately narrower than "any structure", and narrower than the recipe
  list.** A Depot makes nothing, and seeding off what a building could *hold*
  would put a row on a one-row readout for every item in the game. A
  researched bench recipe is compiled into the *player's* pack and never into
  a base buffer, so a row for one would be a zero that could never move — on
  the shipped tree that is invisible, since all six researched recipes name
  equipment and `ItemDef::category` already filters those off the strip, which
  is exactly why it is written down. A banked pool the player has none of
  takes the same rule applied to the one item with no buffer to stand in for
  it: not seeded, or every run would open on a row for a resource nothing in
  the base makes yet.

## 0.13.32

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Idle staff wander the base, and armed cutting tools say so.

### Changed

- **An idle program drifts around the base instead of orbiting the Home.**
  It used to be walked onto a fixed Chebyshev ring at a set distance from the
  Home, a tile every six ticks; `wander_step` now offers it one of the eight
  neighbours of the tile it is *standing on*, or a hold, on the same cadence.
  Relative rather than absolute is the whole difference — a program the
  scheduler has just freed strolls away from the post it left, rather than
  snapping to a tile computed from its index.
- **Laid floor is the leash, and there is no radius to tune.** Entropy
  reverts a mined cell nobody is standing on, so a wanderer that strolled
  into a fresh corridor could be sealed in behind it — unpostable for the
  rest of the run. Floor never reverts, so the paving the base has actually
  laid is exactly how far a body may roam. It still never steps onto a tile
  another idle body holds, or onto the party's own cell.
- The walk stays a pure, RNG-free function of its arguments, folded a byte at
  a time so the step counter reaches the high bits the reducer reads — folded
  whole, every program drifts in the same straight line forever.

### Added

- **The party's tile wears a ring while cutting tools are armed.** `n` arms
  the player's own bump into base-space rock, and the only trace of it was
  the log line at the moment it was toggled: walk away, come back, and there
  was nothing left to read the mode off. The ring is the excavation plan's
  yellow, because a mark and a swing are the same job.
- A ring rather than a colour on the `@` itself, so the sprite that stands in
  for that glyph carries the cue too. It is gated on base space and not on
  the flag alone — nothing disarms the tools on the way back out through the
  anchor, and out on the zone map there is no rock to cut.
- The controls page had never listed `n` at all. It does now, beside `m`.

## 0.13.31

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

F3 shows what a frame actually costs.

### Added

- **A frame-timing readout, on `F3`.** Four figures in the bottom-right over
  the last completed second: the frame count, the mean, the worst frame, and
  the draw pass. The renderer's shape-building pass has had a measured
  baseline since the debug-profile work, but the rest of a frame — bevy's
  schedule, egui's tessellator, the upload and the GPU — had never been
  measured at all. That gap is why the map going jerky read as an animation
  bug for months: the camera code was intact, there were simply no frames to
  draw it in.
- **The peak is the figure that earns its place.** Sixty frames with one at
  90 ms still averages under 18, so a mean alone reports a stuttering game as
  a smooth one. The draw figure is the same pass the bench measures, so the
  two are directly comparable, and the gap between it and the mean is the
  part nobody has seen.
- **The meter is fed every frame whether the readout is on or not**, or the
  first second after pressing `F3` reads as a cold start every time. A
  function key rather than a letter for the reason backslash is, and one
  better: letters reach the game as typed text, and `F3` produces none on any
  layout.
- No new `Painter` operation — `rect`, `ui` and `measure_ui` already existed,
  so the drawing seam is untouched.

## 0.13.30

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Four more ways to hit the whole field.

### Added

- **Seven new routines, all data.** The Everyone tier — the top of the scope
  ladder, where a routine reaches every hostile on the field at once — had
  ten entries and already covered the whole effect vocabulary, so what was
  missing was variety inside it rather than a mechanic. No engine or schema
  change: every one of these is a `.ron` file in `assets/abilities/`, and a
  mod can add an eighth the same way.
- **Skim Everyone** tops off the one family that had a Single and a Group
  rung and nothing above them. It lands exactly on the Everyone tier's floor
  on both axes — 15 Power, four rounds — which makes it the cheapest way in
  the game to touch the whole field, and is the whole of Skim's identity as
  Leech's cheap sibling at every rung.
- **Segfault grows a Group and an Everyone rung.** Its Single rungs sat
  within a point of Packet Shred's on every axis, so the promotion needed an
  identity of its own: the band widens with the scope. Segfault Everyone
  rolls 16–40 against Packet Shred Everyone's 19–31 — a better top end and a
  worse floor — paid for with a fifth round of cooldown, three more Power and
  all but one point of aim.
- **Row Hammer** is a new family, and it is built on the same axis from the
  other end: low damage, high accuracy, at all three rungs. A whiff across
  the whole field is the biggest single roll a fight ever asks for, so this
  is the answer to Packet Shred Everyone being a gamble — chip everything,
  every time.
- **Snoop Everyone** is exclusive, off the Overseer at 0.30. A field-wide
  drain at a rate the ordinary tier may not have: `cycle_harvest` is capped
  low precisely so that reaching wider stays a trade rather than a straight
  upgrade, and behind a boss that stops being a concern, because there is no
  cheaper rung of it to undercut.

### Changed

- **The hunt-only pool widens from 28 routines to 34.** The six new ordinary
  rungs are found on wild carriers, the path five of the ten existing
  Everyone routines already take. That takes the pool's authored weight from
  185 to 212, so each routine already in it now turns up about an eighth less
  often. The census pinning that count carries the figure and the reasoning
  now, since the pool's size is a design decision rather than an accident.

## 0.13.29

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

The Transfer picker's figures line up.

### Changed

- **The Transfer screen (`c`) draws its quantities in a column.** Each row
  carries the signed amount and both live ceilings, and those figures were
  staggering with the item names above them — `suffix_x` places a suffix one
  inset past the *advance* of the row's own label, so a short name pulled its
  figures left and a long one pushed them right. The screen read as a list
  where it is really a table.

  The name is now padded out to the widest one on screen and each figure is
  right-aligned in its own width. The UI face is monospace and a trailing
  space advances exactly as a glyph does, so equal-width labels put every
  suffix at the same x — `draw_row` and `popup.rs` are untouched, and the
  figures keep the dim annotation colour a suffix is drawn in.

  Widths are measured from the rows actually listed rather than fixed, so a
  shelf of short names draws a narrow table instead of a wide one full of
  empty space. A name longer than the column is not truncated: it pushes its
  own figures right and leaves the rest of the table alone, since losing
  characters off an item's name to keep a column straight is the worse of the
  two failures.

## 0.13.28

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

The map can draw a sprite where a glyph was.

### Added

- **A one-cell sprite may stand in for an entity's map glyph.** Drop a
  16x16 RGBA PNG into `assets/sprites/` and the map draws it in the cell
  the glyph would have occupied. This release ships the pipeline and one
  placeholder — the player's — rather than a set of art; per-species and
  per-structure sprites are a later change.

  **The size is not arbitrary.** Map glyphs are drawn at `16 x zoom` with
  zoom clamped to 1..4, so a 16px source lands on exactly 16, 32, 48 or
  64px — whole multiples, sampled nearest-neighbour, which is the same
  contract `unscii-16` is already held to and the reason the font
  rasterization test asserts zero antialiased pixels at each step. A
  sprite authored at any other size still draws; it just blurs at some
  zoom, silently and only on screen. A census refuses one.

  **A sprite substitutes for the glyph rather than drawing over it.** The
  overdraw bug looks pixel-perfect against opaque art and breaks the
  moment a sprite has any transparency, at which point the old character
  shows through — so the test asserts both the sprite drawn *and* the
  `@` absent.

  **Colour is a multiplying tint**, so art authored near-white inherits
  everything that already colours a glyph — the difficulty read, the boss
  and nemesis overrides, the biome tint, the damage dimming — with no
  second mechanism and no change to any of them. Art carrying its own hue
  fights all four.

  **The directory is optional by construction.** A missing directory, a
  missing file, or a name nothing is authored for all end at the glyph, so
  deleting `assets/sprites/` restores the previous map exactly — the same
  supported way deleting `assets/environment/` restores the pre-effects
  game. That is what will let a modded species ship without art rather
  than ship invisible.

### Fixed

- **The asset server now reads the path the launcher resolved**, instead of
  guessing one of its own. Bevy resolves its asset root against the build
  machine's manifest directory in a dev build and the executable's
  directory once installed; left alone it would have been a second site
  deciding a runtime path, which works where it was built and nowhere
  else. Nothing player-visible today, since nothing was loading an asset
  through it before this release.

- **`crates/engine/src/lib.rs` no longer carries an unresolved `git stash
  pop`.** 5,056 lines of a stale inline test block, conflict markers
  included, had been committed on top of the one-line module declaration;
  the workspace did not compile. Resolved to the upstream side.

## 0.13.27

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Gear says what it is worth, and the wagon is one basket.

### Added

- **Every piece of gear carries one combat rating.** Six stat axes and no
  scalar meant "is this better?" was a question the game never answered.
  `Game::copy_power` answers it in one figure, in a fixed-width column that
  runs down every list naming an item — cargo, a trader's shelf, a recipe's
  result, the equipment panel, the Stack market, the wagon.

  **It is absolute, not measured for whoever is holding it**, so one number
  means one thing on all six screens. Every copy is priced against a single
  reference wearer in `tuning.rs`, and that wearer is *derived* rather than
  invented: the zone is the midpoint of the range `balance_sim` sweeps, the
  level is what its geared sweep reports as the minimum to clear that zone,
  and the stats are `stats_after_levels` of `PLAYER_BASE_STATS` at that
  level. A reference far from where players actually stand would make every
  figure in the game wrong in the same direction.

  Four terms, and none of them restates a formula that already exists.
  Attack and mitigation go through `Stats::power`, which already prices
  mitigation as the effective HP it buys rather than summing a percentage
  into a total. The damage band is a **difference** against the band it
  replaces, because a weapon *overrides* the natural attack — so a weapon
  worse than bare fists rates negative, which is the whole reason the term
  is not a sum. Accuracy and evasion are **proportional**, priced through
  `battle::hit_chance` as the fraction they move the throughput they act on:
  a probability is not a quantity. A Decompiler module buys taming rather
  than combat and gets no term at all.

  So there are three cells and three meanings, and they do not overlap: a
  figure is a rating, an em dash is *no answer* (a module with no combat
  axis, a consumable), and a blank is a row that is not an item — a Routine
  Disk on the wagon, an empty slot. A dash on one of those would claim the
  disk had been rated and found wanting.

  The **swap picker's delta may disagree with the column, and that is
  correct**: gear locks in the level it was equipped at, so a worn piece and
  a candidate are scaled at two different levels. The column is a property
  of the copy; the delta is a property of the swap.

- **The inspect page breaks the rating down.** `[I]` on any piece now says
  what the figure came off — offense, survivability, accuracy, evasion —
  with the axes that contributed nothing left out. One line rather than one
  per axis, because that page has no scroll and had no rows to spare; it
  paid for the line out of the affix block, which already had a cap and
  degrades by counting what it cannot draw.

### Changed

- **The visiting caravan is one basket, committed by Enter.** Buying was a
  row at a time and selling opened a per-item quantity page, so a visit that
  cleared a stack of Core Fragments and picked up two things was a dozen
  keypresses and a dozen turns.

  Every row now carries an amount, edited with the arrows — Shift jumps to
  the end of a row, Ctrl halves the gap, `[A]` fills your cargo rows, `[N]`
  clears — and Enter commits the lot. A header line says what the basket
  leaves in your purse, so six rows are no longer set blind.
  `Mode::CaravanQuantity` is gone.

  **The commit sells before it buys**, which is what lets a basket be funded
  by its own sales — the entire reason the two sections are one basket
  rather than two screens. And **every refusal lands before anything is
  spent**: a caravan has no buyback, so a half-committed basket is the one
  bug a player cannot undo. The whole visit costs **one turn**, not one per
  line.

  On this screen Right increases and Left decreases. The transfer picker's
  inverted arrows are specified for a single row spanning both directions;
  here the sign is fixed by which section a row is in, so inverting would
  read as a slip.

- **The wagon's two lists are grouped by category.** The offers came off the
  roll shuffled by construction — a weapon, a program, a second weapon — and
  a deep shelf read as a heap. Both lists now run in one order under their
  own headings. The grouping is a property of the *view*: the shelf itself
  stays in deal order, because which equipment slot a wagon leads with
  rotates per visit and sorting the shelf would open every wagon with a
  weapon.

## 0.13.26

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

A companion can reach into the pack mid-fight.

### Fixed

- **A companion can spend its round on a consumable.** The `[u]se item` row
  was offered only to slot 0, and `Game::consume_item` hardcoded the player as
  the recipient — so a companion that had run its Power reserve dry had no way
  to refill it, while the pack sat full of Power Cells. That matters because a
  companion's Special is charged to its *own* reserve
  (`spend_power(entity, ..)`), not to the player's.

  The item row is now offered to every party slot, and `consume_item` takes a
  recipient. **The pack stays the player's and only the effect moves**:
  `Inventory` lives on the player alone and is the party's one shared kit, so
  a companion draws from the same stack the player would, but the Power
  restore, the heal, any armed field buff and the log line all land on
  whoever spent the round taking it. The reserve and stat writes became
  no-ops rather than unwraps, matching `spend_power`'s asymmetry.

  There is no ally-targeting picker: the player cannot hand a cell to a
  companion, the companion spends its own round on one. Both frontends were
  untouched — the picker already resolved against `battle_active_slot`, and
  its prompt already read "It costs this member their round."

## 0.13.25

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Routines miss less, and accuracy is something you can buy.

### Fixed

- **An even matchup no longer means half your swings whiff.**
  `battle::hit_chance` returned exactly 0.5 for two identical combatants, and
  that was the whole of "routines miss too often". Measured against the
  shipped roster with the real functions, a player carrying no accuracy gear
  sat between 0.44 and 0.64 for the first ten levels — and both apex species,
  the ones that guard a Stack lair, are the fastest things in the game and so
  the hardest to hit at every level.

  A routine and a basic attack have always shared one resolution path, so the
  rate was never routine-specific. What is specific is the *cost*: a basic
  attack shrugs a miss off, while a routine has already spent its Power and
  armed its cooldown by the time the roll happens. Thirteen of the twenty-five
  damaging routines are multi-target with an independent roll for each
  recipient, so a single sweep at 55% printed two or three "goes wide" lines
  in a row.

  The attacker's Accuracy is now multiplied by `ATTACKER_ACCURACY_ADVANTAGE`
  before the ratio, putting an even matchup at 0.583. A multiplier rather than
  a flat bonus, because the ratio form is scale-free and has to stay that way
  — a flat `+n` would wash out as levels grow. It is necessarily symmetric,
  since the function takes two numbers and cannot know which side is the
  player; hostiles come off the hit-chance floor a high-level player had
  pinned them to.

- **An affix that paid only Accuracy, only Evasion or only a damage band was
  refused at load as granting nothing.** Both emptiness checks in
  `AffixDef::fault` enumerated three of `EquipmentStats`' six fields, so the
  accuracy axis could only ever ride along on an ATK affix — which is part of
  why it stayed on three weapons for as long as it did. Both now destructure
  the whole struct, so a seventh stat is a compile error rather than a field
  silently uncounted.

### Added

- **Routines are aimed.** A new `accuracy` key on an ability file adds flat
  Accuracy to the roll that routine makes, and to nothing else. The shipped
  roster grades it by how narrow the routine is — 6 for a single target, 4
  for a whole group, 2 for every hostile on the field — so a sweep trades
  odds for reach rather than being strictly better than an aimed shot. Flat
  and unscaled on purpose: a hostile's Evasion grows with the zone while
  yours grows with your level, so aim is a problem early and solves itself
  late.

- **Target Lock**, an eighteenth perk. +2 Accuracy per level on every attack
  you make, for 3 Perk Points, under the Combat heading.

- **An Accuracy talent node**, the fifth kind a tree may offer, one per class
  ladder and one in the generic tree. Companions get the axis through their
  tree exactly as the player gets it through a perk, and the two never stack.

- **Two accuracy affixes**, `Zeroed` (Weapon) and `of Direct Access` (Weapon
  and Module) — the latter the only one reaching a Module, so the axis is
  buyable by a program already carrying the weapon it wants.

### Changed

- **Eleven of thirteen weapons now author Accuracy**, up from three. The two
  heaviest author none, so the axis is a trade rather than a free line on
  everything.

- **A zone-1 group is no longer a single program.** `zone_group_cap(1)` was 1,
  which made the balance suite's zone-1 fixture a five-against-one fight
  rather than the body ratio the rest of the curve is about.
  `ZONE_ONE_GROUP_CAP` is a floor under the curve, so zone 1 lifts and no
  later zone's step moves. It also ends the Trace group lever's zone-1
  inertness, which was always a consequence of the old cap rather than an
  intent.

### Documentation

- `docs/seams.md` records why the parity baseline is no longer 0.5, and gains
  an entry for the two accuracy doors — what an *entity* brings to every
  swing against what one *invocation* brings.
- `docs/items.md` had never shown Accuracy or Evasion at all: `shiv_routine`
  read "atk+1" while carrying +3 Accuracy. Ten rows corrected.

## 0.13.24

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Routines roll a band, and you run them.

### Changed

- **Every routine that moves Integrity now rolls a range instead of a fixed
  figure.** The mechanism had been in the engine for several releases —
  damage bands, the scaling that widens them with level and affinity, eight
  paragraphs of schema documentation — and not one of the 77 shipped
  abilities ever used it, so every Special dealt exactly the same number
  every time. All 34 damage, drain and healing routines now author a spread
  of about a quarter of their power, which is the ratio the roster's basic
  attacks have used all along. `Heal` gained the range it never had.

  The band is **centred on the old number**, so nothing got stronger or
  weaker: the average of every routine in the game is exactly what it was,
  and the balance suite passed the change without a single curve moving.
  What changed is that a big hit and a small one are now different rolls
  rather than the same roll twice.

- **The word "cast" is gone.** You *run* a routine, or invoke one; the thing
  you did is an *invocation*. "Cast" and "spell" are fantasy words and this
  game has none, so they are out of the manual, the ability descriptions,
  the perk pages and the log lines — and, unlike the 2026-08-05 Raid rename
  which stopped at the player-facing half, out of the code as well. No asset
  field was ever named `cast`, so nothing a mod author writes had to change.

  Ability descriptions that quoted a number now quote the band: Segfault
  Single v3.0 reads "Damage 13–21 to one target" where it read "Damage 17".

### Fixed

- The ability catalogue in `docs/abilities.md` had been six abilities short
  of the game for at least a release — Clock Skew, Core Dump, Hot Spare,
  Interrupt, Parity and Quarantine were never listed, while the page's own
  prose counted them. It also described two passives where eight ship. Both
  are corrected, and the page now shows what each routine rolls rather than
  its centre.

## 0.13.23

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

One key moves cargo both ways.

### Changed

- **Taking and putting are one screen.** `c` beside a machine or a Depot now
  opens a single window with a row per item, carrying what the shelves hold
  of it *and* what your pack could hand over. The amount on each row is
  signed: **Right takes off a shelf, Left puts your own cargo into a Depot**,
  and Enter moves the whole basket in one action and one turn. `P` is gone —
  an item that was on a shelf and in the pack at once used to have a row on
  each of two screens, with no way to see the other from either.
- **A transfer takes before it gives.** Emptying a full Depot and refilling it
  from your pack now lands in one commit; done the other way round the put
  would have been refused for want of room, silently.
- **The screen tells a full Depot apart from no Depot.** A Mining Node draws
  no room line at all, where a Depot with nothing left draws one reading 0.
  The put ends of every row read `-0` in that case while the take ends stay
  live, which is the state the report this came from could not explain.
- **`[A]` still means take everything**, and now overwrites a put you had
  set on a row with nothing on its shelf. Shift goes to the end of a row and
  Ctrl halves the gap, in whichever direction the plain arrow was heading.

### Fixed

- **A Depot at exactly its capacity no longer reads as a broken screen.** The
  old deposit picker showed rows whose ceiling was zero with nothing on screen
  saying why; the room line and the live per-row figures now say it outright.

## 0.13.22

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

A visiting caravan is worth walking over to.

### Changed

- **A caravan's shelf never lists the same thing twice.** Every category is
  drawn from without replacement now: a routine is its ability, a program its
  species, a stack of cargo its item, and gear the whole rolled copy — so two
  copies of one item are on the wagon together only when their rarity, affix
  or quality tell them apart. Before this a wagon could stand there with three
  stacks of Power Cells and two of the same disk.
- **Both traders carry fifty rows**, up from twelve and ten. A Salvage Convoy
  now sets out 23–34 distinct pieces of equipment, and about eleven of them
  are standout stock rather than three.
- **The Kennel Run deals more gear than routines.** There are sixteen
  non-boss species, so on a fifty-row shelf its program pool runs dry a third
  of the way down and everything after it comes out of the other two
  categories — which at its old weights made a program trader whose actual
  wagon was a rack of disks. It is `gear: 4` / `routines: 2` now, and stocks
  the whole roster besides.
- **`rows` is a ceiling rather than a count.** A category that runs dry stops
  being dealt from and the rest of the shelf fills out of the others, so how
  much of anything a wagon can hold is bounded by how many files are
  installed. A shelf deeper than every pool it draws from stops when the last
  one empties. `assets/caravans/README.md` documents both.

Rows past the thirty-fifth have no letter of their own — `menu_shortcut` runs
out at nine digits and twenty-six letters — so a deep shelf is walked with the
arrows below that point, as a long inventory already is.

## 0.13.21

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

The Stack answers four complaints.

### Fixed

- **Down turns you around instead of walking you backwards.** Underground,
  Down (or `j`) is now an about-face: the party pivots on the spot and costs
  a turn for it, exactly as turning left or right does. Backing up is gone
  entirely rather than moved to another key — you turn around and walk
  forward, which is what everyone was doing anyway.

- **You can power down in the Stack.** `r` was bound on the surface and
  simply absent underground, so the key did nothing at all: no rest, no
  refusal, nothing in the log to say why. Resting itself was never gated by
  where you are standing — free on your own slab, one Power Outlet anywhere
  else, the Stack included — so the whole of the bug was a missing key
  binding. `e` and a Power Cell always worked down there and still do; they
  are different things, and only `r` was broken.

- **Deep zones field deeper stacks.** How dangerous an underground fight is
  used to be read off the frame's depth alone, so the first frame of a stack
  was a single lone program whether you had just started the game or had
  breached your way to zone 9 to get there. Depth and zone now both count.
  A zone-3 stack fields the fights a zone-3 stack should, from the first
  frame down, and going deeper still escalates on top of that.

- **There is a boss at the bottom.** The thing guarding a lair was drawn
  from the same danger window everything else spawns from, and the hand-
  authored apex programs sit so far up that window that no stack short of
  six frames could reach one. Every shallower lair quietly served an
  ordinary program with a boss's stat line instead. The bottom of a stack
  now fields a real apex where its terrain has one, at any depth. Ambushes
  and wild spawns are unchanged: an apex you never saw coming is still the
  thing the window exists to prevent, and a lair is walked into on purpose.

## 0.13.20

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

Somebody comes to you for once.

### Added

- **Traders visit your base.** Stand an iso Market up and, every so often, a
  caravan walks in out of the sector, phases through the anchor, sets out its
  stock beside your counter for a while, and rolls back out again. You will
  see it coming across the map and standing in the base; `x` toward it says
  who it is. While it is docked, `b` → **Caravan** opens the wagon.

- **What is on the wagon depends on who turned up.** The Salvage Convoy runs
  mostly worn equipment; the Kennel Run walks programs on short leashes. A
  wagon carries gear with its own rarity and quality rolls, Routine Disks,
  programs, and stacks of the materials your base eats — everything you could
  get another way, for a markup, without the trip. Portal Fragments are not
  on it and never will be: breaching is still earned by fighting and
  descending.

- **They will take what you are carrying**, at the same rate your own counter
  pays. There is no buyback — a caravan does not come back for it — and `[S]`
  sells a whole stack at once.

- **A visit is a property of your base and cannot be rerolled.** When one is
  due, which trader it is, which way it walks in from and what it carries are
  all derived from the base's own seed, so a save and reload finds exactly
  the trader you left standing there, with exactly the rows you had not
  bought yet. Breaching leaves both behind.

- **New moddable content directory, `assets/caravans/`.** One `.ron` file per
  trader: its name, its line, its glyph, how deep its shelf is and how that
  shelf is split between gear, disks, programs and materials. Deleting the
  directory gives you the game exactly as it was before caravans existed.
  Schema in `assets/caravans/README.md`.

## 0.13.19

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

When the game says no, it now says so where you are looking.

### Fixed

- **A refusal is drawn inside the screen you typed into.** Pick a research
  node your zone is too low for and "Requires Zone 3 first." now sits under
  the popup's title, above the rows, in red. It used to be painted as a strip
  along the **bottom edge of the window** — on screen the whole time, and a
  full window's height away from the centred popup you were reading. Every
  screen with a popup does this: research, perks, building, crafting, trade,
  the roster, work orders, contracts, the lot. The four screens that draw no
  popup — the battle screen and the two full-pane frame maps — keep the strip,
  because they have nowhere else to put it.
- The popup **grows** by the line rather than covering a row with it, and the
  numbered options never renumber, so a refusal appearing cannot move the row
  a keypress was about to land on.

### Added

- **Refusals are kept in the message log.** They were never written there
  before, so a refusal you looked away from was gone for good after four
  seconds. `L` now has them, and the map's log pane shows them in red.
  Refusals raised *inside a fight* are deliberately still not logged: the
  battle pane is paced line by line as the round scrolls in, and a message
  from a submenu would arrive as narration and swallow a keypress on the way
  past.
- A confirmation is not a refusal — "Game saved." and a fuse's receipt stay
  out of that history, which is a record of the game saying no.

## 0.13.18

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

A hold order finally holds, and stripping a program takes one key.

### Added

- **`U` takes a program's whole loadout back off.** The roster's `E` opens one
  program's three slots and the picker's `(Unequip)` row empties one of them,
  so handing one program's gear to another was a keypress and a slot choice,
  three times over. `U` is that in one press, gear straight back into your
  cargo.

### Fixed

- **A satisfied hold order stands its line down.** A hold order reaching its
  level put the order to sleep but left the bodies standing on their machines,
  so the line kept running for the rest of the run — a base told to hold ten
  ICE Breakers had seventy-three on the shelf and was still making more.
  Programs now come off a machine nothing is asking for and go back to milling
  about. Reported from a live save: ticked three thousand times that base
  reached 222 against an order of 10, and now stops at exactly 10.
- The same sweep no longer takes a body off a **clogged** machine while a
  Depot is standing. That body is the only thing that can carry the clog away
  and let the machine run again, and freeing it left the machine full for the
  rest of the run.

## 0.13.17

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Nothing here is stored: the new memories go into a store every program
already carries, and the figure they move is derived on every read.

Programs remember their working life, and how a program feels about its work
finally does something.

### Added

- **A program remembers the machine it works and the work it does.** Until
  now the roster remembered fights and strandings and nothing else — a body
  could spend a whole run at one node and hold no opinion of it. Four new
  memory kinds cover a program's working life: settling in at a machine that
  runs, resenting one that is always backed up, the grind of cutting rock,
  and being caught at a machine when a GC Entropy Sweep comes through.
- **Morale changes how reliably a program extracts.** The memories screen has
  headed itself with a Morale figure since 0.13.7 and nothing read it. A
  contented program now works a node a shade more reliably and a miserable one
  fizzles cycles more often. It is a small effect with a hard ceiling, and it
  is **symmetric** — the positive memory kinds are worth something now, rather
  than every memory being a liability.
- Both effects are as quiet as the parking hook that came before them. Nothing
  is logged, no number appears on the base screens, and the memories page
  (`R` from the roster) stays the one place that explains why a program is
  working the way it is.

### Changed

- **`MemorySubject::Structure` and `::Activity` have writers.** Both shipped
  with the substrate in 0.13.7 as subject kinds nothing could ever be written
  about. All six are live now.
- A memory about a machine names the machine's **kind**, not that particular
  machine, so it survives the machine being destroyed and a rebuilt one is
  remembered as the same thing. Settling in and resenting share that subject
  and pull opposite ways, so a machine kind that mostly runs nets out to a
  mild fondness over a run and one that spends its life clogged nets out to a
  grudge.
- A digger remembers cutting rock as a *kind of work* and has no machine to
  remember instead — a dig site is not a structure — so the memory follows the
  program rather than the hole.
- `assets/memories/README.md` documents the four new kinds and the axis that
  divides them, which is not their sign: a sweep is an **edge**, remembered
  the moment it lands, while the other three are **stretches of service**
  written on a period. A half-life authored for a stretch is not comparable to
  one authored for `mauled_by`.
- **Deleting `assets/memories/` still gives back the pre-memory game
  exactly**, extraction rates included. That property now holds at a third
  site and cost no code to keep there.

## 0.13.16

The defensive figure reaches the screens you compare programs on, and the
game settles on one word for it.

### Added

- **Attack and mitigation on the roster and the manifest picker.** Both lists
  quoted HP and PWR and nothing about a fight: the roster is where you decide
  which program goes in the party and which one gets the gear, and the picker
  exists to choose between subjects. Rows now read `ATK 8  MIT 5%` between the
  two, in the words the fuse picker and the field-cast picker already use. The
  picker's own row for you is unchanged — it quotes no HP or PWR either, so a
  lone pair of combat figures would be the only numbers on it.
- **A width census for the manifest picker**, which had none. At the widest
  reachable name it now measures 1170px into a 1243px body, making it the
  tighter of the two lists; the roster's head sits at 986.

### Changed

- **One word and one unit for mitigation, everywhere.** The map's status
  column said `Defense 12` for the number the manifest sheet has always called
  `Mitigation 12%`. The rename did not fit where it stood — `Attack 1234
  Mitigation 75%  Strength 1234` runs 38px past a column that cannot grow
  horizontally and that nothing clips, so it would have been drawn off the
  panel in silence. The four figures regroup onto two lines instead, offense
  on one and mitigation with the decompiler on the other, rather than taking a
  fifth line out of the buff and inventory lists below.
- The same word across eight talent descriptions, two companion upgrade items,
  the Defender perk, the achievements screen, the buff panel's row name, the
  two battle log lines announcing a boost landing and fading, and five
  schema-doc lines.

### Fixed

- **Nine ability descriptions were quoting a third of what they do.** The
  combat model rewrite converted `Buff(kind: Def, power: 4)` into
  `Buff(kind: Mitigation, power: 12)` and left the prose behind, so `bastion`
  has read "+4 DEF" against an authored 12 ever since, `acid_wash` "-5 DEF"
  against -15, and `bastion_shield_v3` "by 7" against 20. They now say what
  they apply, in the form the two that were already correct use — `long_winter`
  and `ablative_layer` have read "-25% incoming damage" all along. No
  magnitude changed; only the claims about them.
- `no_shipped_description_calls_mitigation_defense` is the gate that was
  missing. A screen's wording is held by the test that renders it, but nothing
  compiles a `.ron` description, which is how a rename sweep comes to stop at
  the code. Lower-case "defense" is deliberately allowed: a research node
  describing "automated perimeter defense" is using the ordinary word.

## 0.13.15

Base-space rock stops being one flat number, and walking stops being a way
to demolish your own base.

### Fixed

- **A swing can never take a whole wall.** `Game::swing_damage` grows all run
  against a rock durability that did not, so past a level every cell fell to
  one bump and navigating a developed base took its corners out a keypress at
  a time. `Game::strike_rock` now caps one swing at `durability / min_swings`
  for the cell's kind. Level-independent on purpose: scaling durability with
  the player would make digging cost the same forever. Levelling still cuts
  the swing count down to the kind's floor, it just cannot reach one — and at
  ordinary rock's 24 with a floor of 2 the cap is 12, above a level-1 swing,
  so the opening game's dig rate is unchanged.

### Added

- **Rock kinds, in `assets/rock/`.** One `.ron` per kind carrying a
  durability, a swing floor, a spawn weight and a brightness. Which kind a
  cell is, is *derived* from base space's own seed and the block the
  coordinate falls in — nothing is stored, so `BaseGrid` stays sparse and a
  wall nobody has touched still knows what it is. Kinds come in patches with
  an inside rather than as pepper, and an ore later is a file drop.
  `assets/rock/README.md` is the schema. An empty directory is supported and
  gives uniform rock — though not one-swing walls, since the swing floor is a
  fix and not content.
- **Mining is a tool you take out.** `n` in base space arms the player's own
  bump; disarmed, a step into rock is refused for free — no damage, no dig
  site, no turn. Off when a run starts, and off for an existing save, which
  never expressed a preference. A posted crew is unaffected: a mark is an
  instruction the base was already given, and putting your own tools away
  says nothing about it.
- **An exposed rock face shows what it is made of.** A wall with air
  orthogonally against it is drawn brighter for its kind and named by
  examine; rock behind a face stays anonymous. So exposing a face is the act
  of prospecting rather than reading a map of everything you will ever dig —
  cut a cell and its four neighbours light up, let entropy take it back and
  they go dark. Seeing a kind is a display rule only: a swing at unseen rock
  meets that kind's real durability.

### Notes

- The save gains a `mining` flag and base space gains a seed, both additive
  behind `#[serde(default)]`, so **existing saves load unchanged** and
  `SAVE_FORMAT_VERSION` does not move. A save from before this release lays
  its seams out from seed 0 — a valid layout, not a special case.
- The durability values, the swing floors and the vein block size are
  **unmeasured**, like every other knob in this slice. The design and what is
  open are in
  `docs/superpowers/archive/specs/2026-08-23-rock-kinds-and-mining-mode-design.md`.

## 0.13.14

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this moves where saves live, not what is in them.

### The game can be zipped up and played on another machine

Every runtime path used to derive from the absolute path of the machine that
compiled the binary, which is why `README.md` told players the clone had to
stay put. The game had no distributable build on any platform as a result.

Nothing else in the codebase was ever Linux-only — there is exactly one
`cfg(target_os)` in the whole tree, and the dependency graph resolves for
Windows and macOS without a system library either of them lacks. It was the
*distribution model* that was tied to a checkout, and only by accident.

`crates/launcher/src/paths.rs` is now the single answer to where the game
finds anything. It picks an installed layout when an `assets/` directory
sits beside the executable — or at `../Resources/assets`, which is where a
macOS `.app` bundle keeps them — and falls back to the repo otherwise.
Installed-ness is sniffed rather than flagged, because a build flag that can
be forgotten produces a zip that works only on the machine that made it, and
nobody finds out until a stranger unzips it.

The deliverable is an executable plus a loose `assets/` tree, never a single
file. Fonts and the sound cues are embedded, but species, items, structures,
abilities, talents, perks, achievements and help pages have to stay droppable
— that is the moddability rule, and it survives the move intact.

### Saves, your profile and the run history move to the OS data directory

`%APPDATA%\feral-processes\` on Windows,
`~/Library/Application Support/feral-processes/` on macOS, and
`$XDG_DATA_HOME/feral-processes/` (or `~/.local/share/feral-processes/`) on
Linux.

This happens in every layout, a development build included. Writing beside
the executable would leave a copy unzipped under `Program Files` unable to
save at all, and the failure mode there is a game that appears to save and
silently doesn't. Making it conditional — data directory when installed,
repo when developing — would have been two code paths where one will do, and
would mean a development build could not reproduce a player's report about
where their saves went.

An existing checkout's `saves/`, `profile.ron` and `run_history.log` are
moved across once, automatically, on the next launch. It is a move rather
than a copy, because two save directories that drift apart is worse than one
that changed address, and it does nothing at all if the destination already
holds a save — so it cannot fire twice or land on top of something newer.
The old one-time migration of the pre-`saves/` `save.bin` is folded into it
rather than left running beside it.

### macOS is covered by the same module

The probe that finds an installed build's assets checks
`../Resources/assets` after checking beside the executable, which is where a
macOS `.app` bundle keeps them, and the data directory resolves to
`~/Library/Application Support/feral-processes/`. Neither needed
macOS-specific code — the engine and app-core were already portable, and
this is the whole of what the platform asked for.

The recommendation is to ship a plain binary plus `assets/` in a zip, the
same layout as Windows, rather than a `.app` bundle. A bundle costs no path
code but does cost a plist, an icon and a build step, and Gatekeeper is no
kinder to one. What a bundle would fix is that double-clicking a plain
binary in Finder opens a Terminal window behind the game — macOS's version
of the console that `windows_subsystem` suppresses, and the one place the
two platforms are not symmetric.

`packaging/macos-readme.txt` ships in that zip, covering where saves live,
how to clear the download quarantine flag Gatekeeper checks, and that a mod
is a file dropped into `assets/`.

### A release build on Windows opens no console window behind the game

Debug builds keep theirs, which is what anyone developing on Windows would
want. The cost is that a release build has nowhere to print a startup
failure, so the two a player can actually reach — a missing `assets/` folder
and a data directory that cannot be created — now write `startup-error.txt`
beside the executable as well as to stderr.

### Unverified on Windows and macOS

Neither runtime has been exercised: window creation, wgpu (DX12 and Metal),
audio, input, the console suppression, SmartScreen and Gatekeeper, or
whether `%APPDATA%` and `~/Library/Application Support` resolve as expected. Verification for this is manual by choice, and the
ten-step checklist lives in the spec
(`docs/superpowers/archive/specs/2026-08-19-windows-and-macos-distribution-design.md`).
The Linux suite being green is not evidence about any of it.

## 0.13.13

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing here is persisted.

### A defeated group no longer hands its turn to the pack behind it

Kill the last program in a group and the group behind it got to hit you
twice in the same round: once on its own initiative, and again on the dead
group's. The quieter half of the same fault ran the other way — a group
whose place in the order had shifted lost its round in silence, which reads
as a pack going passive the moment anything dies.

Initiative is rolled once at the top of a round, but a hostile was named in
that order by its *position* — which group, which member. A kill drops the
dead member, and drops the group entirely once that empties it, so every
position behind the casualty shifts down one and the order ends up pointing
at whoever moved into the gap. The order now names each hostile outright,
so what acts on a program's turn is that program or nothing.

A group promoted forward by a kill in front of it is also properly engaged
now, rather than swinging from the rank it held before anyone died.

The party side is untouched: a party slot stays a slot, because nobody
leaves the party mid-battle and the round's plan is written against those
slots. Aiming a strike at a group that falls before your turn still spends
the turn rather than redirecting it, exactly as it has since 0.2.

## 0.13.12

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing new is persisted. `Stock` and `Inventory` are untouched.

### Put your cargo into a Depot

A Depot was the only structure you could take *out* of and never put *into*.
`P` beside one now opens a window over your pack — a row per item, a quantity
per row, and Enter puts exactly that basket into the base's hands. It is the
collect window's mirror and shares its keys, `[A]`/`[N]` and the Shift and
Ctrl arrows included.

The goods go into the Depot's output buffer, which is the point rather than
an implementation detail: that is the same buffer `base_holding` sums and the
same one a work order's feeders draw from. A production line stalled for want
of an ingredient you were personally carrying had no fix before this.

What may go in is plain, ordinary cargo. Fused, rare and high-quality copies
stay on you — a Depot stores items by name and has nowhere to record what
made a copy special, so one put away would come back out ordinary. Banked
Research Data stays out for a different reason: a bank is not cargo.

A Depot's room is one budget shared across every row, so filling one row
lowers what the rest may reach, and the window says how much is left. An
over-ask is clamped rather than refused, and a Depot with no room left takes
nothing and costs no time.

### Also

The collect and deposit windows are now one key table rather than two copies
of one. Nothing about the collect window changed — but the inverted Left/Right
that window specifies, and the Ctrl step that halves the gap, now exist once
instead of twice, where they cannot drift apart.

## 0.13.11

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
`ItemDef::enables_rest` is an additive, defaulted asset field, and the two
retired structure fields were never written into a run.

### Resting is priced by where you are, and never costs time

Rest no longer needs a structure standing in reach, and **no rest advances
the clock**. Inside base space it is free: the walk home is the whole of the
price. Anywhere else — the open grid, or four frames down the Stack — it
spends one unit of an item whose def carries the new `enables_rest`, and the
Power Outlet is the one shipped item that does.

The two halves hold each other up. A base rest that ticked could be spammed
to farm production, raid pressure and need decay; a priced rest that ticked
was the game's only bulk source of time. Neither is true now, and
`Game::wait` is the only thing left that passes time without an action.

`RestDef`, `StructureDef::enables_rest`, `nearby_rest_structure`,
`rest_cost` and `REST_TICKS` are gone, none of them with a reader left. The
two help pages that called a recharge a full night now say what it is: free
at the base, an outlet in the field, instant either way.

### A marked room your crew would never touch

Marking a block of rock out in the open and leaving idle programs standing
in the base got you nothing at all — no swing, no walk, and no line in the
log saying why.

Every cell of such a block is boxed in but its rim, and a boxed-in cell is
refused *silently* on purpose: it is the ordinary interior of any plan and
it resolves itself as the shell in front of it comes down. But the refusal
happened after the crew's work had already been budgeted, so the interiors —
which sort first — spent the whole budget, and the rim was cut off the end
of the list before anyone looked at it. A thirty-six cell room with one
reachable cell and six spare programs sat untouched for the rest of a run.

A cell nothing can stand beside is no longer a job at all. The budget goes
to cells a program can be sent to, and each cell behind them becomes a job
the moment the rock in front of it opens — so a room now unpeels from its
face inward, with the crew fanning out as it widens. A marked cell that has
a face and no route to it is unchanged: that one is your errand, and it
still says so once.

## 0.13.10

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this is an input and text change and nothing about it is written into a run.

### The collect window's arrows are the other way round

Left now adds one to the highlighted row and Right removes one. The window
shipped a release ago with the conventional mapping and it read backwards in
play, so the polarity is reversed on this screen alone.

It is the one inverted Left/Right in the game — the manifest pager, the
arena row editor and every movement handler still step Right positive — so
the window now says which arrow does which instead of the old
"Left/Right set the amount", and the base's help page says it in prose too.
Nothing else about the screen moved: digits and Backspace still type an
amount, `[A]` and `[N]` still fill and clear every row, Enter still takes
exactly the basket and Esc still costs nothing.

### Shift and Ctrl on the collect window's arrows

The two modifiers are different verbs. **Shift is an end**: Shift+Left
fills the highlighted row to what is on the shelf, Shift+Right puts it back
to nothing. **Ctrl is a step that halves the gap** to whichever end it is
heading for — on a shelf of 7, Ctrl+Left walks 4, 6, 7, and Ctrl+Right
walks back down 3, 1, 0. Press it again and you get half of what is *left*,
not the same number twice.

The step rounds up, which is what makes it finish: rounded down, a gap of
one gives a step of nothing and the key would go dead with the row neither
full nor empty.

Both are per row, which is what separates them from `[A]` and `[N]` — those
are the same two ends across every row at once. Holding a key is safe either
way: Shift is already at its end, and Ctrl converges on one.

Every other screen is unchanged. A modifier reaches the collect picker and
nowhere else, so Shift with an arrow still walks, still pages a manifest and
still moves a building cursor exactly as the bare arrow does.

## 0.13.9

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
nothing about this is written into a run.

### `c` takes what you ask for

`c` beside your base used to empty every output buffer touching you into
your pack, wholesale. There is no deposit verb — nothing in the game puts
units back into a base buffer — so a misfired keypress beside a working
line was not a convenience cost but a permanent one: the ingredients a
chain was about to pull were now in your pack, and the only route back was
to make them again.

It now opens a window. One row per item on offer, pooled across every
machine touching you, and every row starts at **zero**. Up and Down pick a
row; digits, Backspace and Left/Right set an amount; Enter takes exactly
that basket, in one action and one turn. `[A]` fills every row to its
maximum and `[N]` clears them all, so taking everything is still two keys.
Esc takes nothing and costs nothing, the same way a collect that finds
nothing has always cost nothing.

An amount clamps as you type it: `50` against 12 on the shelf leaves the
row reading 12, rather than silently ignoring the second digit. An over-ask
at the moment you commit is clamped too — a raid or a hauler can empty a
shelf while the window is open, and a basket that has gone briefly
optimistic hands over what is there rather than refusing.

Underneath, taking everything is now literally selecting everything and
then committing: one reach rule, one taking path, and one place the
"nothing to collect here" refusal is spoken. Units leave a buffer through
`hauling::take_from` alone, where the old wholesale path removed the entry
by hand — correct only because it always took the entry whole.

### The dig crew pays for its tile out of the base's own stores

A crew that had cut a marked cell open floored it only if the *player* was
personally carrying a Blank Substrate — 12 on the base's shelves and 0 in
the pack left two marks standing over two finished cuts, and not a word
about why. The crew now draws from the base's output buffers, in tile
order and over the same set the stock strip counts, with the player's pack
as the fallback. `Game::lay_tile` is untouched: a player verb still pays
the way every player verb does.

The silence was half of it. A crew with nothing to lay now says so once,
beside the once-only complaint a boxed-in crew already made. Neither latch
is saved, so a reload says both again.

## 0.13.8

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
this is a rendering change and nothing about it is written into a run.

### The equipped panel says what your gear was compiled at

The `WEP` / `ARM` / `MOD` column carries a copy's quality in its emphasis —
dimmed under spec, bold above it, gold at the top — and the three rows
naming what you are *actually wearing* were the only gear rows in the game
without one. The screen showing your loadout could not say how well any of
it was made.

They carry it now, on the cargo screen and on a program's gear page alike.
The column replaces the spelled-out slot rather than sitting beside it:
`WEP` and `Weapon:` say the same thing, and printing both puts one column
on a row twice. An empty slot still names the slot it is empty of.

## 0.13.7

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
A save written before this release loads with every program given a fresh
identity and an empty memory store — the three new fields are additive on
field-named RON.

### What a program remembers

Programs on your roster now form memories of what happens to them, and hold
them. Four kinds ship:

- **Fought beside** — a bond with each program that survived a won fight
  alongside it.
- **Won against the odds** — a fight the party had no business walking away
  from.
- **Mauled by** — the species that took better than a third of a program's
  health in a single landed blow. The longest-lived of the four.
- **Left stranded here** — a grudge against one corner of the base, formed
  when a program is posted somewhere it turns out it cannot reach.

**A memory fades unless something reinforces it.** Intensity is worked out
from the clock every time it is read rather than being counted down, so
nothing ticks and nothing drifts; a repeat of the same event resets the
clock and deepens the mark, up to a limit each kind sets for itself. A
memory that has faded far enough is dropped the next time that program
forms one.

**`R` on a roster program opens what it holds** — a page headed by that
program's Morale, the signed sum of everything it currently remembers, with
each memory's strength and how long ago it was, in words rather than in
ticks. Rows arrive strongest first.

### A program will not be parked where it was left stranded

The one place a memory changes behaviour, and it is deliberately quiet: an
idle program loitering in your base will not be stood on a tile it holds a
grudge against, and takes a different spot on the ring instead. It costs the
program one beat of standing still — the same thing already happens when
the ring offers it a tile a machine is standing on.

The loop closes on itself. A program is parked somewhere, posted to a
machine, finds no route to it, and is marked stranded *where it is
standing*. That is the tile it remembers, and that is the tile it will not
be put back on.

Nothing else about staffing changed. The scheduler still decides the whole
assignment by priority and then diffs it, with no score anywhere in it.

### Memory kinds are a content directory

`assets/memories/` is a mod directory like species, items and abilities —
each kind is one file naming its valence, its half-life, what it can be
about, and how far it compounds. The schema is documented in
`assets/memories/README.md`. **Deleting the directory gives back the
pre-memory game exactly**, the same supported way deleting
`assets/environment/` or `assets/policies/` does: memories already formed
are kept but weigh nothing, the page draws no rows, and the parking hook
goes inert.

## 0.13.6

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Nothing here is stored — both features read figures the game already held.

### The player sheet states its to-hit pair

COMBAT grows to six rows on the player page — Damage, Attack, **Accuracy**,
**Evasion**, Mitigation, Power. The two new figures come from the same
`accuracy_of` / `evasion_of` calls `resolve_attack` consults, so the sheet
cannot quote you odds the fight then disagrees with.

Player-only, and that is a layout budget rather than a data gap: the
program page's worst case clears its footer by 17.3px against a 10px floor,
so one more row anywhere on it overflows at 1280x720. Both figures are
carried for both subjects, so buying that page room later is a layout
change and not a data change.

### A RUN box says what the run is holding

Credits, Portal Fragments, the difficulty mode, the cycle, and how many
contracts the run is signed to. Credits and Portal Fragments are banked
pools that deliberately sit outside your inventory, which is why the cargo
row has never said anything about either. Trace is absent on purpose —
it is underground-only and the Stack view already draws it.

### A stock strip says what the base is holding, on every screen

One row across the top of the window, on every screen that draws the world
behind it — including under a menu, since a popup is capped at 85% of the
window and leaves the top band clear. Each pile is a two-letter tag and a
quantity.

**It is a readout, not a second opinion.** `Game::base_stock` reads the same
output buffers the base's own holdings are summed from, so the strip cannot
drift from what the base actually has. Piles are ordered by item id rather
than by quantity — a strip that re-sorted as buffers filled would move
every tag under the eye of the player reading it — and the row is
*measured* rather than estimated, so the piles that fit are named and the
rest are counted in a `+N` tail instead of being drawn off the edge in
silence.

Tags are derived from an item's name, so a modded item gets one for free,
with an override for the one shipped collision: Research Data takes `[R]`
and the Research Disk keeps `[RD]`.

The panes below the strip now take their origin from the caller rather than
from the window. Both views already funnelled their geometry through one
converter each, so the offset is stated once and everything follows it.

## 0.13.5

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
An order filed before this release loads as a one-shot batch at the Normal
band, which is what it was.

### The base works every order at once, in priority order

The queue used to be a to-do list: the base worked the front order and
nothing else, so a base with more programs than that one line could use
parked the spares beside a machine that already had somebody on it. It is a
production policy now — every unsatisfied order is worked at the same time,
and the order nearest the top gets first refusal on every body.

Priority is still queue position, and nothing was added to make that work.
The wants come back in queue order and the scheduler already cut the list
from the end, the same mechanism that has always made dig jobs the lowest
priority. Two orders wanting the same machine are counted once, so the
higher one keeps it.

**Your staffed base is materially more productive as a result**, and nothing
in the suite can see that — `balance_sim` has no base term at all. It is a
pacing question for play.

### An order can be a level the base holds, not just a batch it makes

`[S]` on the quantity page files a **standing** order. A batch is finished
and removed; a level is held — when the shelf drains, the order re-arms and
the base makes more.

That closes a hole the old behaviour had no answer for: collecting from a
machine empties its whole output buffer, so walking past your own base
drained the very stock the order had just declared complete, and the order
was already gone. There is no hysteresis and no refill threshold, because
the drain is a burst rather than a trickle and there is nothing to
oscillate around.

A standing order says nothing when it tops itself up — "complete" is a lie
about something that is not complete — so **filing** one announces itself
instead.

The quantity page also now says that the target is what the *base* holds:
machine and depot buffers, not your pockets. `0/20` while carrying forty of
the thing is the figure working correctly.

### An order can be filed above or below the queue

`[P]` on the quantity page cycles a priority band — **High**, **Normal**,
**Low** — and it raises first, since raising is what the feature is for.
Before this the only control you had over the base's attention was cancel
and refile, which lands the order you care about at the *bottom*: strictly
worse than useless.

The band is an insert position rather than a second sort. An order lands
after the last order of equal or higher band and nothing reads the field
again, so ties still break by the order you filed them in and one band is
still a queue. Refiling an order now restores its band instead of dropping
it to the foot of the list.

### The queue screen says what the base is doing about each order

Every order carries a tag: **WORKING** (somebody is standing on its chain
right now), **QUEUED** (it wants machines and the base ran out of bodies
before it got here), **HOLDING** (a standing order at its level — the
feature working), or **STALLED** (the line broke).

The two that needed telling apart are HOLDING and STALLED. Both want
nobody, and one of them is a base that needs rebuilding. WORKING is read
off who is actually posted rather than off what the scheduler asked for,
because two machines in the want list never get a body — one the base has
been built around with no route to it, and one held by a program the
scheduler is not allowed to move — and calling those "working" sends you
off to watch a machine nobody will ever stand at.

### A broken line says so in the log

A stalled order was news only if you opened the queue screen on purpose.
It now logs **once**, on the way into the stall, and again if it breaks a
second time after you have repaired it. Not every tick: a line you have
already been told about is not news.

The log line is the headline alone — "Work order stalled: 30 x Routine
Disk." The sentence naming *which* machine went missing stays on the queue
screen, where it is wrapped; the commoner of its two shapes runs to 198
characters, and the map's log pane draws a line as exactly one unwrapped
row about 135 cells wide.

Reloading a save announces the stall again. The run that was told is over.

### The screen says how short of bodies the base is

A header above the key hints: how many posts the queue asked for against
how many programs the base has. The scheduler cuts its list to the bodies
it has and the posts past the end vanished in silence, so a three-machine
line with two programs said "no one" on the third machine and nothing
anywhere said you were one program short.

It answers "why is nothing happening" from the other side to the tags: a
tag says which order has the base's attention, this says whether the base
has anyone to give it. It is **silent when you have bodies to spare**,
because a line that shows on every visit is a line nobody reads by the
third one.

## 0.13.4

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.

### The perk screen has sections

Seventeen perks in one undifferentiated column made you read all of them to
find the three you cared about. They now sit under four headings —
**Combat**, **Affinities**, **Fieldcraft**, **Workshop** — in that order.
The shortcut keys still run straight down the list across the headings
rather than restarting under each one, so nothing about typing a key
changed except which letter lands where.

**The layout is a file.** `assets/perks/groups.ron` names each section,
what sits under it, and where it sits, all in one statement — rename a
heading, reorder the sections, move a perk between them, none of it needs a
recompile. The format is documented in `assets/perks/README.md`. It is one
statement rather than a `group:` field on each of the seventeen catalogue
entries because membership alone orders nothing: a per-perk label would
need a second rule for which heading came first, and two authored halves of
one layout drift apart the first time someone edits only one.

Three things it deliberately does not do. Deleting the file gives back the
flat, unheaded list exactly as it was. A perk no section names is still
offered, in a trailing unlabelled run at the foot of the screen — a typo
costs a heading, never a row you can spend points at. And a malformed
layout is skipped with a warning, costing the headings and none of the
perks.

### The results screen says you won

A win was the only ending in the game with no line of its own: the screen
went from the killing blow straight to `Salvage:`. It now heads the results
with **"You won!"**. A jack-out and a flatline are untouched, since both
already declare themselves one line higher.

The experience lines gained an `Experience:` header and the same indent the
salvage rows carry, so the two blocks read as a pair rather than as a list
and some loose numbers.

### The decisive round's blows outlive the fight

**Fixed:** the final round of a battle was narrated and then deleted before
the screen had revealed a single line of it, so the results appeared to
jump from the kill straight to the salvage. The round that decided the
fight is the one you most want to watch, and it was the one round you never
saw.

The narration is now pruned when you *leave* the results screen rather than
when the fight ends. Two consequences worth knowing: the closing roster now
stands beside the final blows instead of replacing them, and a companion
detaching after a loss scrolls past on the results screen rather than
vanishing with everything else.

## 0.13.3

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32.
Gear gained a quality figure, but it is an additive field with a named
default, so every copy in a file written before it existed loads at exactly
100% — the numbers that run already had.

### Gear is something you compile, not something you find

Every blade you compiled used to be byte-identical to the last, which put
the whole "is this one better than mine" question in the hands of the loot
table. A carried copy now records **how well that particular copy was
compiled**, as a percentage of what the item was designed to do, and it
shows in the name: `Overclocked Arc Lance of Static (115%)`. A copy at
exactly spec says nothing, the way an ordinary tier does not print a word.

**Your base is what makes good gear.** The floor a compile rolls from is
built out of things you own: the tier of the bench it is compiled at, a
perk, and whether you asked for a careful job. A fresh player's compile
lands in 80–100; a developed base's reaches 110–130. **A field drop rolls
from a poorer floor than any of that** — 70–90 — so the world is a lottery
ticket and the base is the gearing path. That is a real change to the early
game: your first compiles are *weaker* than they were, when every craft was
exactly 100.

**A batch is a spread, not five of one thing.** Each unit rolls its own
luck, so compiling five gives you five copies to compare and keep the best
of. A copy that rolls exactly 100 still stacks; the rest each take a row.

**`[C]` on the quantity page toggles a careful compile** — more quality for
more materials, priced at every figure that page quotes, including the
max-affordable line. The toggle clears when the page opens, so it can never
outlive the batch you asked for.

**Both compile benches can be upgraded now.** The Fabricator and the Armory
had no upgrade path, which meant the bench term did nothing on any shipped
recipe. Upgrading one buys better gear and nothing else.

**Tighten Tolerances** is the seventeenth perk: a higher floor on everything
you compile, read at the compile rather than banked at purchase, so gear
already in your buffer keeps what it was made at.

**The category tag on a list row carries the read.** `WEP`, `ARM`, `MOD` are
drawn in the copy's own emphasis — dimmed under spec, bold above it, gold at
the top — while the row's colour goes on meaning fusion and rarity. Only the
two extremes spend a colour, and a copy at spec is drawn exactly as it
always was.

### `[I]` inspects a piece of gear from anywhere it is named

Seven screens name gear and none of them could answer "what does that
actually grant". `[I]` now opens one page from all of them — the cargo list
and its action page, the swap picker, a program's slot page, a trader's
three shelves and a Stack stall.

The page draws the whole of a granted routine rather than its name: what
fires it, what it lands on, what it hits for **at the wearer's level**, its
cooldown and its price. Plus the stat block at the level the copy would go
on at, the accuracy it buys, the hit chance that works out to, and what it
compiled at. A piece carrying neither a damage band nor accuracy quotes no
hit chance at all, since that figure is the wearer's and printing it under
armour reads as a claim about the armour.

### The map draws one space, and it is the one you are standing in

Three reports, one cause: the base's coordinates and a zone's spawn point
are both usually (0, 0), so a tile in one freely aliased a tile in the
other.

- **Your `@` never moved inside the base.** It does now.
- **Stack entrances, nests and the anchor drew inside the base.** They
  carry a glyph and are neither structure nor program, so both existing
  gates looked straight past them.
- **Your base roster drew scattered across the open grid.** Idle staff are
  parked in base coordinates every tick, which is exactly what made them
  look like programs standing out in the sector.

**A GC Entropy Sweep's flash belongs to the base too.** A sweep landing
while you were out on the grid washed a tile of open ground and threw
debris across it — usually the tile you were standing on. The log line and
the pane's own flash already carry that news without claiming a tile.

### Eight more manual pages

Intrusions, Your companions, Routines and field buffs, Perks, Your base,
Supplies and salvage, The Stack, and Before you breach — all written
against the source rather than from memory. The Controls page also listed
two base rows that stopped existing when work orders replaced manual
posting, and omitted three party rows; both now match what the menus draw.

### Also

- **`<` and `>` at the anchor read as up and down now**, not in and out.
  `<` phases up into the base and `>` drops back to the grid — the mirror
  of what the Stack binds rather than a copy of it.
- The equip swap row sheds its stat column onto a continuation rather than
  running it off the edge of the popup. The quality figure costs seven
  cells, and the widest row had 3.7 to spare.

## 0.13.2

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the manual is content read off disk and nothing about it is written into a
run.

### A manual you read inside the game

`?` used to draw a card of key bindings that closed again on any key. It now
opens a manual: an index of topics, each one a page you can scroll, with
further reading at the bottom of each page that you follow by typing its
key. Four pages ship — Start here, Controls, Zones and breaching, and
Getting stronger — and the old key-bindings card is one of them rather than
a second help surface you have to know about separately.

**`?` no longer closes on any key.** That is a real change to an existing
reflex: the screen is navigable now, so Esc backs out of it one page at a
time, like every other screen. Esc from the index closes the manual.

The index is a menu and a page is a document, which is why they take
different keys. On the index, Up/Down and Enter or a row's own key pick a
topic. On a page, Up/Down scroll the prose, Enter does nothing, and a
further-reading row is followed by typing the key beside it. Esc pops one
level of wherever you have read to, so following three links and walking
back out lands you where you started.

### Pages are files you can edit

The manual is `assets/help/*.md` — ordinary markdown, five rules' worth of
it, documented in `assets/help/README.md`. Adding a topic is dropping a file
in that directory: no rebuild, no registration, and the filename is both the
ordering and the id a link points at. `[label](topic-id)` in a sentence
reads as the label and adds the further-reading row in one gesture, so a
cross-reference is written once, where it belongs.

A malformed page is skipped with a warning rather than refusing to start, as
every other asset directory does, and a link pointing at nothing is dropped
from the list instead of drawing a row that refuses when you pick it.

### A program you own and aren't fighting with is base staff

Landed on `main` after `0.13.1` was tagged and ships here. Base staff used
to be a marker you assigned by hand from the Base Staff screen. It is
derived now: a program on your roster that is not in your party and not
held as your weapon **is** base staff, with no verb to assign it and nothing
to forget to assign. The screen keeps its rows, its activity and its work
profiles, and loses only its write.

Two things follow from that. Posting a worker no longer pins it — the
poster is in the pool, so the scheduler may move it next tick — and your
base's output now scales with the size of your roster, bounded only by how
many programs you can hold.

Existing saves are unaffected: the old marker is still written and simply
read nowhere.

## 0.13.1

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 32 —
the walls you have started on and the plans you have drawn are a new,
additive save field, and a file written before it existed loads with no dig
sites, which is exactly what that run had.

### The base grows, and you cut it out yourself

0.13.0 moved the base into its own space and said growing it was out of
scope. This is that scope. The pocket you start with is a room in solid
entropy, and every cell past it is rock you can take down.

**Rock is hit, not walked through.** Step into a solid cell in base space and
you swing at it, the same way you wear a nest down. There is no new key and
no direction prompt — the wall is a thing you attack. Swings are
deterministic, so a wall never becomes a slot machine: it takes about three
hits at level 1 and one hit late in a run, and **that is the reward for
levelling rather than a curve that scales away from you**. Rock is the same
rock in every zone at every depth; what changes is you. A cell that opens
sometimes shakes a Core Fragment loose.

**A cut cell is not floor yet.** `v` lays a VectorStasis Tile on the cell
you're standing on for one Blank Substrate, and that is what makes ground
permanent and buildable. Bare cut ground is the frontier, and the frontier
does not keep: leave a cell open long enough and base space takes it back to
solid rock, at full thickness, so re-opening it costs the swings it cost the
first time. Laid tile is never reclaimed at any age, and neither is a cell
somebody is standing on — a base can be left alone while its owner is off in
a zone without coming home to a smaller one.

**`m` opens the Excavation plan**, a mode rather than an action: the cursor
costs no time and no tick, `space` drops an anchor, moving previews a
rectangle, and `space` again commits it. Marking and clearing are the same
verb, decided by the cell you anchored on — anchor a marked cell and the box
clears instead. A marked wall runs the whole way through on one mark: it gets
cut, the mark survives the cut, and then it gets floored.

**And you don't have to be there.** Post programs to your base and marked
cells are cut and floored while you are off in a sector. Dig jobs are the
**lowest** priority the base has, below work orders and standing jobs, so a
spare body digs and a needed one does not — marking a corridor can never
stop production. A marked cell your crew genuinely cannot reach says so
once, and then stays quiet; a marked cell walled in by the rest of your own
plan says nothing at all, because it opens itself as the shell comes down.

### Also

- Three contract and environment texts still described the base as a "slab"
  stamped onto the zone surface, which it stopped being in 0.13.0. A Hunt
  contract now speaks of what is loose within sight of your anchor.
- All four of the new tuning values — rock durability, the fragment chance,
  how long the frontier holds, and how fast a posted digger swings — are
  unmeasured starting values.
- GC Entropy Sweeps were landing on marked rock instead of your machines. A
  dig site carries a durability pool, which is what the raid picked its
  target by, so a large plan drew nearly every sweep away from the base —
  and a sweep that finished one off took the mark and every swing of
  progress with it while the wall stayed standing, so it healed to full.
  Sweeps target buildings again.
- A base staffer standing between postings could have the ground reclaimed
  out from under it and be sealed into solid rock permanently — unable to be
  posted, and unable to walk anywhere, for the rest of the run. The frontier
  now counts anybody standing on it, not just somebody mid-job.
- Clearing a plan the crew had already started on did not call them off: the
  site keeps its chip progress by design, and nothing downstream was reading
  the mark, so a digger finished a wall you had told it to leave.
- Every program dug at the player's rate. A crew program now swings its own
  species' attack band, so which program you put on a wall is worth
  something beyond its attack score.
- Clearing a mark off a cell the frontier had already reclaimed left an
  invisible record behind — drawn nowhere, wanted by nobody, and saved from
  then on.
- A marked cell the crew could reach could still be reported cut off. The
  base picked one face of a post up front — the nearest, ties to the lower
  x — and gave up if that one had no route, which for a cell on a rock spur
  is the side facing unbroken rock. It tries the other faces now, and
  because a dig site announces being stuck only once, the old behaviour
  skipped that cell for the rest of the run.
- Committing a large plan no longer costs more the more of the base you
  have already dug. The box was looked up tile by tile against every dig
  site standing, which is a full scan per cell — a maximum box ran 625 of
  them. It builds one lookup per press now.
- A wall you chipped and walked away from is called a Chipped Wall rather
  than a Marked Cell: most dig sites are not in any plan, and the manifest
  said they were.
- Pressing the tile key with nothing to press left the refusal in the base
  log as well as on screen, once per press. The base log records what the
  base did, not what it declined to.
- The fragment-payout bound is now stated per tick, against the Mining
  Node's own rate. As a per-cell comparison it could not fail: a cut cell
  pays at most one fragment against the four a Blank Substrate costs, so
  every legal value of the knob passed it. At the shipped numbers the real
  bound still has about fourteen times the slack it needs, so it is a
  backstop rather than a tight gate.

## 0.13.0

**Saves written by 0.12.0 and earlier will not load.**
`save::SAVE_FORMAT_VERSION` moves from 31 to 32. The base's own coordinate
space and everything standing in it are new save state, and there is no
sensible way to derive a pocket dimension's floor plan or a structure's
place in it from a v31 save that never recorded either.

### The base is out of phase

The base has left the zone surface for its own pocket-dimension coordinate
space, entered through a permanent, indestructible anchor (`#`, gray) that
now stands wherever your run starts, on the zone surface, alongside you.
Step onto it to phase in; step onto the same cell from inside to phase back
out. The anchor travels with you across a breach, appearing at the new
zone's spawn point, and cannot be destroyed or moved.

Deploying your first Home lays a pre-cleared, 69-cell pocket of floor around
the anchor's door rather than stamping platform tiles into the world you're
standing in — the base is *there*, not *here*, and every machine, trader and
posted program you build stands in that space from then on. Breaching no
longer touches the base at all: nothing despawns, nothing repositions,
nothing resets. It is exactly where you left it the next time you step
through the anchor, on any zone.

The Heap Pillar and Heap Block, the two structures that used to widen the
base's footprint on the surface, are retired along with the surface
footprint they widened. Growing the base is out of scope for this release.

### Two things that follow from being out of phase, and are not bugs

**A rest can no longer be interrupted.** Nothing on the surface can reach a
party that is off in its own coordinate space, so the mid-rest battle check
that used to be able to cut a rest short never fires there.

**The base is now completely safe from surface threats.** A wild program, a
raid, anything that hunts on the zone surface has no way to reach a base
that isn't on it. Both are the direct consequence of the relocation, not a
balance change.

### Also

- Two biomes were added for the new space, `Entropy` and `Excavated`, drawn
  through the existing surface renderer — nothing new to build for them on
  the graphics side.
- A Recharger Node now regens the party while they're genuinely standing in
  base space, and the structure roster's "Work it yourself" row now appears
  when you're really beside a machine in there — both were measuring the
  player's surface tile against a base-space structure's coordinates and
  came up wrong for the whole of the base.
## 0.12.1

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 31.

One release covering six changes, rather than the per-change releases the
preamble asks for. All six had already landed on `main` unversioned; this
section is written to match what the tag actually holds rather than to
pretend the rule was followed.

### Companions act instead of inflating your sheet

A companion used to feed a tenth of its ATK and mitigation into the player's
own stats, floored at one point each, *on top of* taking its own turn,
swinging and soaking hits. Recruiting was paying twice for one body. The
passive half is gone; the roster acts in its own right.

A **wielded** program keeps its bonus, and the difference is the argument for
it: wielding takes the program out of the party, so it never takes a turn and
its share of stats is the only thing it contributes.

### The map glides again

Reported from play: walking the map had gone jerky. The camera code was never
the problem — there were no frames to draw the glide in. The root
`Cargo.toml` carried no `[profile.dev]`, so bevy, wgpu and egui compiled
entirely unoptimised into the build `cargo run` produces, which is the build
the game is played from. The renderer's shape-building pass alone measured
**51.4 ms a frame** against release's 2.0 ms at an identical shape count —
under 20 fps before the tessellator had done anything.

Dependencies now build at `opt-level = 3` and the four workspace crates at
`1`. The debug draw pass lands at 2.3 ms. The price is one cold rebuild of
the 557-crate graph.

### A posted program walks from where it was standing

The base scheduler read *your* tile twice, and both readings went wrong once
idle staff started loitering. A program milling by Home teleported onto you
the instant it got a job and walked in from wherever you were; and walking
away from the base stopped it filling a single machine, so the pool stood
idle beside the order it was hired to work. A base that only runs while you
are stood in it is not a base.

Posting now writes no position at all, and every question is asked of the
body being sent.

### The research screen says what each row is waiting on

Every row in the research menu (`T`) is now coloured by what stands between
you and it. Green is a node on a recommended path you can buy right now,
plain white any other available one. The three you cannot take are quieter
and told apart by hue: amber for a node waiting on another node, blue for one
waiting on a breach, grey for what is already researched. A node held by both
walls draws blue, and the tag after its name still names both reasons.

Which nodes are recommended is **data**, not Rust: a research file sets
`recommended: true` on a destination and the whole chain leading to it
inherits the colour, so the green row is always one that can actually be
bought. The shipped tree points a new run at the Compiler and at
Fortification. See `assets/research/README.md`.

### Also in this release

The **Power Outlet** costs 5 Core Fragments again. A stray asset edit had
dropped it to 2 while leaving its value at 5 — an item worth more than its
recipe is an infinite Credit loop, and the price census caught it.

### For anyone measuring

The removed party stat bonus was never modelled by `balance_sim`, so no curve
in the suite moved — a doc comment there claiming it as one of three ways
party size compounds was a copy of the game's behaviour rather than a reading
of the module's, and now says two. Measured in the arena instead, 200 reps
across four party-bearing scenarios: fights run 0.3-0.4 rounds longer and the
player keeps 1-3 points less Integrity.

The debug profile also cuts the engine suite from 38.6 s to 6.7 s and
app-core's from 10.2 s to 1.7 s; CLAUDE.md's build section is rewritten
around the new figures, and its claim that the old ~24 s was an unavoidable
RON artifact is retired. Full numbers and blind spots for both are in
`docs/measurements/`.

Older releases: [CHANGELOG-ARCHIVE.md](CHANGELOG-ARCHIVE.md).
