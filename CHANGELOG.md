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
