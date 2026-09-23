# Changelog: 0.4.0 to 0.12.0

Release notes for [feral-processes](README.md), `0.12.0` back to `0.4.0`. The versioning
policy is at the top of [CHANGELOG.md](CHANGELOG.md). Newer releases are in [CHANGELOG-0.13.md](CHANGELOG-0.13.md); older ones in [CHANGELOG-0.3-and-earlier.md](CHANGELOG-0.3-and-earlier.md).

## 0.12.0

**Saves written by 0.11.9 and earlier will not load.**
`save::SAVE_FORMAT_VERSION` moves from 30 to 31. `Stats::def` became
`Stats::mitigation` and changed *unit* — subtractive absorption became
percentage points — and a field whose meaning changes under a name it keeps
is the one case field-named RON cannot rescue: a v30 file would load `def: 6`
into a percentage slot and read as 6% mitigation rather than 6 points of soak.
It is refused by version instead. `FieldBuffKind::Def` folding into
`Mitigation` rides the same bump.

### Every attack rolls to hit

An attack is now resolved against the defender's **Evasion** rather than
landing automatically. Accuracy and Evasion are derived from a species'
`base_speed` plus its level plus gear — never stored, so they cannot drift —
and the odds are the scale-free ratio `accuracy / (accuracy + evasion)`,
clamped between 25% and 95%. A zone that scales everything therefore changes
no hit rate anywhere.

One roll decides the outcome across four bands: a **critical hit** doubles the
rolled portion of the damage (not the flat attack bonus, which would make
crits scale with every attack source in the game), a plain hit, a **fumble**,
or a miss.

### A four-rung fumble ladder

A fumbled swing lands on one of four rungs, chosen by how deep into the fumble
band the roll fell. **Exposed** cuts your Evasion until your next turn.
**Recoil** turns half your own damage back on you. **Opening** gives the
target a free swing. **Crash** costs you your next action. Rungs replace one
another rather than stacking — a cumulative top rung is a run-ender — and a
free swing that itself fumbles resolves as a plain miss rather than chaining.

`Exposed` is available to content immediately: any species move can inflict it
from its `.ron`, so a debuffer species needs no engine change.

### Weapons carry damage ranges

Gear authors a `damage: (min, max)` band, and **a weapon overrides a natural
attack rather than adding to it** — a companion still rolls a species move
each turn for its name and its status rider, but the weapon supplies the
numbers. Every shipped move and every damage ability gained a `spread` around
its authored power, so damage varies rather than being a single number.
Modded content needs no editing: an omitted `spread` is the deterministic
value it always was.

Weapons and armour now trade along two axes each. Shiv Routine, Kinetic Edge
and Black Ice Pick take a narrow band plus **accuracy**; Monofilament Whip and
Plasma Router take a wide band and none. Sandbox Liner, Scrap Ward and Static
Mesh trade most of their mitigation for **evasion**.

### Defense is now Mitigation, and it is a percentage

`DEF` becomes `MIT` on every screen: percentage points of damage reduction,
summed from your species, your gear and your buffs, and capped at 75% so
nothing reaches immunity. It is the one stat **levelling never raises** — a
percentage that grows per level approaches immunity, so a zone tier and a
level-up both leave it exactly as authored, and levelling buys evasion
instead. A Recompile Kernel no longer raises it by a zone tier either.

### Every con colour and every kill's XP in the game has moved

This is a consequence rather than a side effect, and it gets its own line.
`Stats::power` — the "how strong is this" scalar behind the difficulty colour
a program's glyph is drawn in, the price of a kill's XP, and what a trader
pays for a program — now prices mitigation as the effective HP it buys
(`max_hp / (1 - mitigation/100)`) instead of summing a percentage into a
total, which was meaningless. Nothing about the world changed; what you are
told about it did.

### Also

- A weapon's damage band shows wherever a gear stat already did — the
  inventory tag, the equipped panel, the swap picker and a program's manifest
  — through one formatter, so no two screens can disagree about it.
- The gear-swap picker wraps a long row onto a continuation line instead of
  running off the popup, the same way the inventory list already did.
- Materialising inside solid substrate is lethal again. It goes through a
  named `kill_outright` rather than a large damage figure, because mitigation
  was leaving the player standing on a single point of Integrity.
- A missed Drain restores nothing, and a missed swing lands no status rider.
- Battle log lines report the damage that actually landed rather than the
  damage that was rolled, which differ once mitigation is a percentage.

## 0.11.9

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
Both new save fields are additive on a field-named RON struct, so a save
written before this release loads as a program that has been developed
neither way.

### Companions can be developed past the level cap

A **Privilege Ring** drops from a lair guardian in the Stack, and from
nothing else. Spend it at the new **Develop a program** screen (party menu) to
open a **Kernel Ring** on one companion, raising *that companion's* level
ceiling by two — up to three rings, at one, two and three rings apiece, so a
fully developed program is six guardians' worth of descents.

A ring grants no stats, no level and no XP. It buys room; the fights still buy
the levels. A posted worker's cronjob XP is deliberately untouched, so a
developed program cannot be ground up at a Mining Node.

### Every level past the cap pays a talent point

Each level a companion earns above level 6 pays one point into its **class
talent tree**, spent on one of two choices in the next tier — six tiers deep,
one per level a fully ringed companion can earn. The four node kinds are a stat
percentage, a sharpened affinity for one ability category, a routine granted
outright, and one more routine slot. Points are derived from the level and the
list, so nothing can desync.

Fusing keeps the dominant parent's rings and talents — the parent whose species
and level the child takes — and loses the other's.

**The trees are moddable content.** `assets/talents/*.ron` ships one tree per
class plus a generic tree for a program with no readable class; a sixth class's
tree is a file, not a Rust change. See `assets/talents/README.md`.

### Also in this release

A field routine cast from a companion is charged to that companion's own Power
reserve rather than the player's — the fix that was sitting unreleased on the
branch this landed from.

### For anyone measuring

The arena's companion level clamp moved from `CREATURE_MAX_LEVEL` to the
absolute ring cap, because a scenario authors its own party and has no ring to
read. Five shipped `dev-arenas/` scenarios author `level: 12` and were silently
getting level 6 since the cap was halved; they now field what they say, so old
reports from them are not comparable to new ones.
`dev-arenas/developed-companion.ron` is the new scenario, and
`docs/measurements/2026-08-19-developed-companion-worth.md` is what it said.

## 0.11.8

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
A biome was renamed, which sounds like a save break and is not: the old name
is kept as an alias, so a save written before this release loads and reads
back as the new one.

**One mod-facing schema change.** A sector file's `static_temperature` key is
now `deadlock_temperature`, matching the biome it names. The old key still
works — a sector written before this release applies its threshold exactly as
it did — but new sectors should use the new name. See
`assets/sectors/README.md`.

### The ground has a name, and past the first sector it does something

Terrain has been scenery since the game started. It decided what spawned on
it and what colour it drew, and that was the whole of it — the player could
not even find out what they were walking on.

Every biome now has a name, and crossing from one into another says so. That
much happens from the first step of a new run, in every sector, including the
first.

Past sector 1, three of them also do something. Deadlock queues you: a step
onto it costs an extra tick, and the world keeps running through it — a
production cycle finishes, a need decays, a wandering program gets one move
closer. Null Sector and Mainframe cost Integrity instead, a small fraction of
your maximum on every step, so crossing one is a supply problem rather than a
countdown. Open Grid, which most of the map is made of, stays exactly as it
was. Sector 1 is neutral ground throughout; it is where a run learns the
game, and ground that bit there would be a tax on the tutorial rather than an
exception to it.

The bite goes through the same path a hit in a fight does, so mitigation
applies to it — Ablative Layer is worth something on a long crossing. Only
the player takes it; the party is never touched. Terrain never costs Power
and never raises Trace.

### Ambient effects are a content directory

`assets/environment/` is the new one, and it works the way the others do: one
`.ron` file per effect, a malformed one skipped with a warning rather than a
crash, and a schema reference in `assets/environment/README.md`. A file names
the biomes it claims and one of two effect shapes. **Deleting the directory
restores the pre-effects game exactly**, the same supported way deleting
`assets/sectors/` does.

Three refusals are enforced at load, each protecting something a file has no
business revoking: the base slab may not be claimed — it is the one safe
ground in the game — and both magnitudes are capped, against ground that
kills in two steps and against a step that reads as a hang.

### The cold biome is called Deadlock

Static Field is now Deadlock. The rename frees the word "static" for a
weather layer this is the first phase of, and the new name says what the
ground is: allocations that never resolved and are still waiting.

## 0.11.7

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
Nothing here touches what a save holds — one of these is a base-logic fix and
the other is a renderer change to what an existing screen prints.

### A work order sees the route its haulers already walk

Standing a Compiler and a Lathe on the slab could leave the Request row
missing from the base menu entirely, with nothing anywhere saying why. The
picker only offers an item some standing machine can actually make, and the
question it asked was whether a feeder for every ingredient stood
*orthogonally beside* the bench.

That was never the rule the base ran on. A worker fetches a missing
ingredient off a Depot shelf and walks it back, and the scheduler has staffed
a machine on the strength of that since Depots shipped. So a base with a
Mining Node, a Depot holding fragments, and two benches a couple of tiles
away was already able to run both of them — and was refused an order for what
they make, silently, because the machine that decides what to offer and the
machine that decides who to post were reading two different rules.

They read one rule now: a bench is fed by a neighbour, or, when a Depot is
standing, by any producer in the base. The walk that decides who to post
reaches through the same route, which is what keeps the order moving once the
shelf runs thin rather than stalling with nobody working anything. A base
with no Depot behaves exactly as it did, and the refusal still names the
ingredient nothing is making.

### An item says what it does, where you are looking at it

Seven of the shipped modules grant a passive routine while worn. Six items
permanently upgrade a companion. There are two consumables and a taming
catalyst. None of that appeared on any screen that *lists* items — an
inventory row said what the item would add to your stats and nothing else,
and the effect was visible only on the description page two keypresses
further in.

Every extra effect an item carries now prints on its own line under the item,
in the inventory, on a trader's three shelves, on a Stack market's sell list
and on the action screen you open a row into. A stat bonus stays where it
was, on the row itself, since it was never the thing that was missing.

## 0.11.6

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
Nothing here touches what a save holds — this is a renderer fix to how one
screen lays its text out.

### A research node's description no longer runs off the page

The Research and Perks pickers each print a description under every entry,
and each printed it as a single line. The prose the assets carry runs to
about 240 characters; the popup those screens open in holds about 114. So
most of most descriptions was drawn outside the box — and silently, because
a popup row is clamped vertically and nothing clamps it horizontally.

Descriptions now wrap, at the same width the Recipes screen already wraps a
product's prose to, so the two screens cannot disagree about how wide the
game's prose runs. A wrapped description stays attached to the entry it
belongs to and scrolls with it rather than being torn off and pinned to the
foot of the box.

## 0.11.5

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
Everything added here is asset data keyed by string — a new item, two new
structures, a research node — and a save stores those ids, not their
definitions. A patch for that reason: while the project is `0.x`, "breaking"
means a save that stops loading.

### A breach unlocks a material, not just a bigger number

Breaching used to change only the *rate* on the two raw materials: the Mining
Node has produced Core Fragments and nothing else since it shipped, and a
zone's contribution was a bonus per cycle. Cache Coherence, researchable once
you have breached, unlocks the Cache Tap — and Cache Grain is what the second
sector and everything past it is built out of.

It layers rather than replaces. Core Fragments remain the everyday currency
and stay extractable forever, because every recipe still denominated in them
would otherwise strand. Unlike the currencies, Cache Grain survives a breach:
a run arriving in a new sector holding none of it could not upgrade anything
until it had re-tapped.

Three things ask for it. The Line Driver is a new build that feeds the grid
harder than a Heap Pillar and claims no ground doing it, which is what keeps
a base growing once all five Pillars are standing. Every structure upgrade
past the first tier now wants it — free in zone 1, where the tier ceiling
already refuses every upgrade. And the six research nodes that hand over a
gear recipe now denominate those recipes in it, so zone-gated gear is made of
the zone's material.

### A companion's buff no longer runs off the panel

A routine running on a companion drew its holder's name on the same row as
the routine, and that row ran 360px past the edge of the map's status column
— silently, because rows there are clipped vertically and never
horizontally. The column holds 38 characters and the widest routine row
already spent all but four of them, so there was never room to shorten the
tag into. The holder now draws on a dimmed line of its own beneath the
routine, and the battle screen's copy of the panel is unchanged, since that
one measures itself and can simply grow.

A truncated list also counts routines again rather than lines: "+2 more" had
been about to mean two hidden rows, which is one hidden routine.

### One kind of attack

A wild program's turn has always branched two ways — cast a Special, or swing
a basic attack — and the two were different kinds of thing in the code for no
reason anyone had written down. A basic attack is now an ability like any
other, converted from the species file at load. Nothing about play changes:
every seeded fight in the suite plays out move for move as it did, and no
species file needed editing, mods included.

What this does not do is change how a basic attack's damage is worked out. It
stays flat authored power, where a Special scales with level and species
affinity — merging those would make every enemy swing scale too, which is a
difficulty change rather than a tidy-up, and one to argue on its own evidence.

## 0.11.4

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
`Platform::claimed` is additive behind `#[serde(default)]`: a save written
before ground claims existed loads with an empty claim set, which is what it
had. A patch rather than a minor for that reason — while the project is `0.x`,
"breaking" means a save that stops loading, and this has none.

### A base that grows a tile at a time

The Heap Block claims one tile of ground at the platform's edge instead of
standing a structure on it. Where the Heap Pillar is the tool for area — a
whole ring in every direction — the Block is the tool for shape, so a base can
be run out along a ridge or squared off around a Stack link rather than only
ever growing as a circle. It costs one Blank Substrate and sits behind Page
Allocation, a cheap early research.

A claim has to touch the base, and it **refuses** a Stack link, a nest or a
hostile rather than taking the ground out from under them the way a Pillar's
ring does. Paid-for ground travels with the base on a breach.

`StructureDef::claims_ground` is the data half, so a modder ships a
ground-claiming build with no Rust. The footprint half is stored rather than
derived, because a claim leaves no entity to count it back from — `covers`
stays the one statement of where the base is.

### The Pillar feeds the grid

A Heap Pillar now supplies grid energy while it stands, and costs six Blank
Substrates alongside its fragments. Growing the base is also how the base is
powered, and both growth tools now run off the Lathe a base already needed for
Routine Disks.

### Every log pane folds a repeated line into one row and a count

A round that wipes seven programs used to push "The rogue program crashes and
deletes itself!" seven times. The line names nobody, so seven copies say
exactly what one and a count say. The history screen already folded repeats;
the map pane and the battle pane now do too.

A wipe still takes one beat per kill, and what you watch is the count ticking
up — the fold sits on the rows about to be drawn, after the truncation, because
the battle reveal, the unread-line count and the battle roster replay all count
raw lines. Seven deaths would otherwise read as one.

## 0.11.3

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
`CreatureSave::boss` is additive behind `#[serde(default)]`, which under
field-named RON costs no bump: a file written before rolled bosses existed
loads with every creature un-bossed, which is what it was. A patch rather
than a minor for that reason — under the policy above, "breaking" while the
project is `0.x` means a save that stops loading, and this has none.

### Which species a zone or a depth may field

A species' danger band is now derived from its `growth_multiplier`, and each
band is eligible only inside a window of danger steps — the zone number on
the surface, the frame depth underground, the same scalar the group-size
curves already take. So a fresh run meets the gentle end of the ladder and
nothing else, the middle band takes over a few zones in, and the hardest band
arrives last and never leaves. It used to be possible for a level-1 player to
walk seven tiles off their landing site and meet the toughest program in the
game.

No asset changed. The band is derived rather than authored, for the reason a
species' class is: a rung is a fact about numbers the species already
carries, and a second authored field is a second thing that can disagree with
the first. A modded multiplier between rungs snaps to the nearest and is
never refused.

Where a biome has a hole in its ladder, the pool falls back to the nearest
band that biome does hold, so no biome is ever empty. That fires against the
real roster at both ends — Static Field ships no easiest-band species and
Open Grid no hardest-band one — and the honest fix for either is a species
file, not a wider window.

### Any species can spawn as a boss

`is_boss` in `assets/species/*.ron` now marks an *apex* species: always a
boss, never scaled by the engine, and eligible only deep into a run. Boss-hood
itself became a per-individual roll available to every species — outside the
opening ring a spawn can come up a boss, and where no apex species is
eligible yet it is an ordinary one scaled up instead. Bosses now arrive early
and easy, late and hand-authored, rather than being two fixed programs you
either had met or hadn't.

Neither kind rolls a rare tier, since the boss multiplier is the whole of
what one is worth and a tier on top would be a second, invisible one. The
opening ring refuses a boss the same way it already refused a rare tier.

A lair guardian is drawn from the same window at its own depth and is always
a boss. That closes a standing trap: a guardian in a biome with no apex
species used to come back not-a-boss and pay no Portal Fragments, leaving a
stack unbreachable in everything but name.

## 0.11.2

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
`ItemDef::grants` is asset schema behind `#[serde(default)]`, and the passive
it names is **derived at fire time** off the wearer's `Equipment` rather than
written into `Routines` — so nothing about a granted routine reaches a file,
and taking the item off ends it by omission. `PopulatedChunks`, below, is an
additive save field behind the same default: an older save carries no marks
and stocks the ground around wherever it left the player.

### Gear that carries a routine

Every passive routine in the game was a boss drop, which made `exclusive` and
`triggers` look like the same idea. They never were. An item may now name a
routine it grants while worn, and eight scavenged pieces do — three weapons,
two pieces of armour, three modules — carrying six new passives between them.
A granted routine costs no turn and no routine slot; the item is what was
spent.

Two new triggers come with them, each with its one call site in
`battle_resolve_round`. `RoundStart` fires as the round opens, which is the
trigger that fires every round there is, so all three routines on it pay a
four-round cooldown. `AllyWounded` fires when a party member crosses
`tuning::WOUNDED_INTEGRITY_FRACTION` — a third of Integrity — **downward in
one round**, which is a crossing rather than a level: a fight that grinds
somebody down slowly never fires it, and a burst that drops them does.

`AllyDropped` stays reachable by exactly one item, deliberately. A dropped
companion is dissolved with no revive at any difficulty, so a routine paying
out there only pays a player who has already lost more than the payout is
worth.

Gear is wearable by any owned program, so a companion carries a granted
passive as readily as the player does — and unlike a Special, a passive costs
no turn, so it is the first companion-side ability effect that fires without
anybody choosing it. The item describe page names what a piece grants.

### What the eight items are worth

Measured before the merge rather than after, against 100 paired fights per
item, controlled by the item's own `grants` line and validated against the
shipped stat twins: `docs/measurements/2026-08-18-gear-passive-worth.md`.

Nothing in it moved a number, which is the result. `WOUNDED_INTEGRITY_FRACTION`
fires in 5% of the runs of a fight the party wins easily, 25% of the fights
they win on curve, and **100% of losses in every band** — neither dead nor a
slower `RoundStart`. Uptime on a four-round cooldown measured 0.230 firings a
round against a nominal 0.250, and it does not front-load.

Two things the run found that nobody had asked about. A bare `def: 2` module
with no grant at all is worth more at level 12 than the strongest grant
measured, and worth **exactly nothing** at level 36, where that same item's
grant ends fights — so the two halves of a granting item invert in importance
across the level range. And `deadman_relay` is not merely comparable to the
etched disk that carries the same routine, it is identical: 100 of 100 reps
agreed on outcome, rounds, Integrity and companions lost. Which of them is
worth carrying is a question about slots, not about effect.

**None of it was played.** `balance_sim` models no abilities and gates none
of this, so the arena and a session are the only instruments, and only the
first has been run. Whether three passive lines in a round read as your gear
working or as noise in the log is still open.

### The sector is populated where you go, not where you have been

Wild population was placed only relative to the player — a one-time seeded
disc around the arrival point, plus a 5%-per-tick roll within twelve tiles of
wherever they stood. The map is unbounded and generated a chunk at a time, so
no finite seed could cover it, and the roll could not fill space either:
walking costs a tick a tile, which buys about one spare roll per density box
against a target of twelve.

Measured before: zero to two programs per box across 240 tiles of walked
ground, and a clean halo around the base — 15 at the player, then 10, 6, 3, 1
at 25-tile intervals. After: 8, 13, 12, 12, 12, 15, 14, 14, 14, 13, 8, 12
across the same 300 tiles.

Population is a property of place now. `PopulatedChunks` records which world
chunks the sector has stocked and `ensure_local_population` stocks any chunk
within one of the player's, so ground arrives populated a chunk ahead of the
pane rather than filling in behind you. `WILD_CREATURE_CAP` stops being
decorative: its cull is shared by both placers and evicts whole chunks,
farthest first, taking the mark with the creatures.

### Two refusals in the Stack

Pressing `o` on an orphan with no ICE Breaker reported as "nothing happens".
The key was bound and the engine was refusing correctly; the refusal was
borrowed from a battle row, where a lowercase fragment sits under a greyed
option — alone on the status line for four seconds it reads as no response at
all. Both refusals are sentences now, and the underfoot row names the missing
item instead of advertising `[o] adopt` regardless.

A full roster was the same bug wearing the other obstacle: a party holding a
catalyst but no room still read `[o] adopt` and only learned otherwise by
pressing it. Both obstacles go through one `Game::adopt_block` that the key
refuses on and the row warns with, so the offer and the key cannot drift —
which is the invisible kind of drift, the row going on offering while the key
quietly refuses.

## 0.11.1

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 30.
The two new structure fields are asset schema behind `#[serde(default)]`, and
`MachineStatus` — which gains a variant here — is never serialised at all: it
carries no serde traits and appears nowhere in `save.rs`. A save written
before this release loads, and the first tick decides which of its machines
are dark.

### The base power grid

A base had no reason not to sprawl. Every structure was bounded by its build
cost and by `MAX_BUILD_DISTANCE_FROM_HOME`, both paid once; a structure that
existed cost nothing to keep. So the optimal base was every machine you could
afford, and the only ongoing decision a base asked of you was where to post
programs.

Machines now draw on a grid, and the grid has to cover them. Every tick the
base sums what its structures supply against what its machines draw, and if
the draw is over, machines are cut in `(x, y)` order until the rest fit. A cut
machine reads **dark** — it makes no progress on a cronjob, an assembler
neither works nor pulls from its neighbours, and hand-working it yourself
yields nothing either. The base pane header reads `Grid  15 / 16`, red when
short.

The cut does not stop at the first machine that will not fit: a 3-draw machine
that cannot fit a 2-unit budget goes dark while a 1-draw machine behind it
still runs. Stopping early would darken an arbitrary tail. The order is
arbitrary but it never changes, so the far corner drops first and you can lay
out around it.

**A machine draws whether or not anyone is posted to it.** The building is
plugged in; the worker is a separate question. That makes an idle machine you
keep around "for later" a real expense — demolish it or power it — and it is
the only version you can plan against, since supply covers the base you built
rather than the base you happen to have staffed this minute.

The Recharger Node is what settles this. It was a one-time purchase that
deleted a need meter and then became furniture; there was never a reason to
build a second. It now supplies 4 to the grid, as Home does, and there is no
cap on how many you build.

**This is called the Grid, never Power.** `Power` already means a program's own
reserve, two panels away in the status column — the Recharger still trickles
that back, and the two are separate resources that happen to share a word.

**The numbers are unmeasured.** `balance_sim` models battles and has no
base-production term at all, so it gates none of this. Home supplies 4,
each Recharger 4; extractors draw 1, base assemblers 2, late assemblers 3.
They are a starting point for a session, not a tuned claim, and the first
thing to change if play says otherwise.

Three of the six `dev-saves/` templates stood more machines than a single
Recharger could carry and are given the Rechargers to match — `chains` and
`contracts` run 15 against 16, `deep-lair` 17 against 20. A new gate keeps the
next template from being captured short.

## 0.11.0

*`0.10.0` is skipped. A stray tag of that name was pushed onto a branch
commit that carried no release, and moving a published tag is worse than
spending a number.*

**Existing saves will not load.** `save::SAVE_FORMAT_VERSION` goes 29 → 30:
`PlayerSave::fatigue` is a field *removed*, which field-named RON does not
save you from, and `PlayerSave::hunger` is renamed to `power` in the same
bump. `CreatureSave::power` is additive and rides it. The six `dev-saves/`
templates are hand-edited to match.

### Power replaces Fatigue

The game had two need meters and only one of them was a mechanic.

**Power** drained at 0.15 a tick, was the only thing that could kill you by
attrition, and scaled your attack down below 50. **Fatigue** *refilled* at
0.08 a tick and was spent by exactly two things in the entire game — the
Stack's Phase and Jump. Battle Specials stopped charging it on 2026-08-08.
It was a meter, a save field, a `FieldBuffKind`, a `ConsumeDef` field, a
serde default and a row on the status bars, all in service of two routines.

Meanwhile the thing you actually spend all game — calling routines — was
priced only in cooldowns, which are a pacing device rather than a budget.
A cooldown says "not again yet". Nothing said "not any more".

Fatigue is gone. Power is now what every routine call draws on, alongside
the cooldown it already had, and **every companion holds its own reserve**.
A companion's Special is paid out of the companion's Power, so a party's
casting is something you manage rather than a free action that fires
whenever it is off cooldown. A companion at zero Power falls back to plain
attacks; nothing about an empty reserve costs it Integrity.

Scarcity stays soft on the surface and bites underground, and that is a
property of the code rather than a rule anyone maintains: a Recharger has a
radius, and a base is where Rechargers get built, so a Stack run carries
whatever the party walked in with. **That depended on a bug fix.**
`power_regen_system` reads the player's `Position`, which is pinned to the
surface entrance tile for the whole of a Stack run — so a link sited inside
a Recharger's radius refilled the party four frames down. Harmless while
nothing underground spent Power; it would have deleted the feature outright.

`Game::rest` refills Power, for you and for every program you own. That is
the sole refill, and it gives the party's casting budget the same
base-bound shape as everything else.

Two things did not change, deliberately. Hostiles get no reserve — their
policy weights were trained against today's action distribution, and a Power
constraint would cost a retrain nobody would see the benefit of. The wielded
program's 25% proc stays free, because that rate is already its whole price.

### No costs were authored for this

The numbers were already in the files. `AbilityDef::fatigue_cost` was
documented in three places as reaching only Phase and Jump — true about what
the engine read, and false about what the assets contained: 55 ability files
carried a cost nothing consumed, priced back when the field meant exactly
what it means again now, and 10 more carried one inside their `FieldBuff`
effect. The content pass was a key rename with the values untouched,
verified by diffing the sorted multiset of all 65 numbers rather than by eye.

The serde default moved from 5.0 to **0.0**. The old number was the price of
commanding a companion, a mechanic that stopped charging in 2026-08-08, and
it survived only because the field reached two routines; keeping it while
widening the field to every ability in the game would silently price every
ability a mod ships. The five uncosted files are untouched and stay free —
`priority_boost` most of all, since it is the fallback every companion has
when its species grants nothing.

`tuning::ROUTINE_POWER_COST_MULTIPLIER` scales the whole curve at once. The
shipped values' *ordering* is worth keeping and their *scale* is inherited
from a pool that refilled itself, so 1.0 is a starting point rather than a
measured answer. `balance_sim` models no abilities, so none of this is
gated by the balance suite — the arena and a session are the instruments.

### Trickle Charge, retuned

With one need there is one per-tick restore kind, so `Coolant` merges into
`Trickle` and `coolant_flush.ron` is deleted — the two were the same ability
once Fatigue was gone. That leaves Trickle Charge as the only source of
Power underground, and it was retuned rather than left at its Fatigue-era
numbers: 80 turns at a cost of 20 becomes 60 turns at 25. One cast buys back
about a quarter of a reserve and takes 60 underground turns to collect,
which is a real Trace and encounter cost — a sustain rather than a tap.

Its *scaling* was the larger find. Trickle ran its authored magnitude
through the caster's level, so `power: 1` is 7 a turn at the level cap:
Power pinned at full for the buff's whole duration, and an authored number
the level term swamps. It no longer scales, by the rule that arm of the
match already stated — Regen's ceiling is `max_hp` and grows with level,
Power's is a fixed 100 forever.

### Field routines run until you rest

Eight of the ten out-of-battle routines no longer count turns down. Overclock,
Hardened Shell, Ablative Layer, Long Winter, Deep Scan, Trace Analysis,
Salvage Routine and Stealth Protocol run until the party rests, and a
Forgiving reboot is the only other thing that ends one. That turns them from
something you cast just before a fight — timing an 80-turn window against a
walk you can't predict the length of — into an expedition loadout you buy at
base and carry until you come home.

Repair Loop and Trickle Charge keep their counters, and the line is a rule
rather than a list: `Regen` and `Trickle` are the only kinds with a per-tick
effect, so an until-rest one is unbounded healing or unbounded Power, and
Power underground is the whole of the Stack's scarcity. They are also the only
two that use `interval`, whose cadence is phased off the very counter an
until-rest buff no longer has.

An *item's* buff still keeps its own clock whatever kind it arms — a Patch
Routine is spent when you use it, where a routine can be run again on the next
charge. That distinction is why the rule reads the buff's source and not just
its kind: a consumable and a routine of one kind stack rather than displacing
each other, so a permanent item buff would have compounded under the routine's.

For modders, `duration` on a `FieldBuff` is now decided by `kind`, and both
mistakes are refused at load with a named reason rather than resolved
quietly: a duration on a kind that ignores it, and a missing one on a kind
that counts (which armed at zero and expired on the turn it was cast — silently
possible before). Buff rows show `rest` where they used to show a tick count.
No save-format change; a buff already running in an old save simply stops
ageing.

### Hardened Shell, for the whole party

Hardening the party meant casting Hardened Shell Single four times: four
turns, and 56 Power for a full roster. **Hardened Shell Party** is the same
+4 DEF on everyone off one cast for 32 Power, taught by a new **Mesh
Plating** node — zone 3, 120 Research Data, hanging off Adaptive Plating.

The price is the whole design, and it is bounded from both sides: above one
Single, so the narrow cast is still what you reach for when only one program
is going to get hit, and below covering the party a Single at a time, or
nobody would ever run the wide one — the only thing it would still buy is
turns, and turns are free. Both bounds are asserted against the shipped
assets as a *relationship* rather than as three numbers, so a Power retune
moves all of them freely and only an inversion fails.

Data only. `Long Winter Party` was already a whole-party field buff, so
nothing in the cast path changed — this is two `.ron` files, which is what
the moddability rule is for.

## 0.9.3

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 29
— `CreatureSave::nemesis_grudges` is additive and defaults to 0, so nothing
written before this release loads any differently.

### A program you lose to remembers you losing

Every wild program used to be interchangeable. Win or lose, the encounter
left nothing behind — a fight you bailed out on was indistinguishable a
minute later from one that never happened.

A creature that beats the party, or that the party jacks out on, is now
marked a **nemesis**. It gets a name on the spot, drawn from
`assets/nemesis/names.ron` and salted off its species and stat roll so two
nemeses of the same species read as different individuals. Meeting it again
opens the fight with a taunt of its own, from `assets/nemesis/taunts.ron`,
picked from a flat pool by folding the grudge count into the same seed —
so a longer history tends to land on a different line rather than climbing
some tone ladder. A grudge promotes the creature's `Rarity` a rung and fully
heals it, so a nemesis is not just a name — it is a program you are actively
losing ground to. The ladder is bounded by `Rarity::ALL` itself (2.15x
cumulative, decelerating), and a hard cap of ten held nemeses at once refuses
a new mark once full rather than bumping one out.

Both marks read anywhere the creature does: the map draws a corner mark
beside its rarity bar, and its glyph draws in a reserved colour regardless
of the usual con read — the one exception being a program that is also a
zone boss, where the nemesis mark wins. You already know what fighting this
one costs; that is the whole justification for spending the "can I win this
fight" read on its tile.

A breach clears every nemesis in the zone, the same as every other wild
hostile — there is no separate ledger for the feature to leave behind.
Rewards flow entirely through the promoted `Rarity` and the existing
challenge-scaled XP curve; nothing new was added to either.

### Contracts meet a new run where it starts

Contracts were meant to onboard a run and were sitting behind the tree they
were supposed to introduce: the Contract Broker wanted a 10 Research Data
node and 14 Core Fragments before it could be built. The Contract Brokerage
research node is deleted rather than zeroed — a structure no research file
names is unlocked by default — and the build drops to 5 Core Fragments, so
the Broker is buildable from turn one. An old save that had already
researched Contract Brokerage still loads; the id simply names nothing now.

The board itself was a coin flip. It draws three slots uniformly out of
everything eligible, and a zone-1 pool is nine authored contracts plus up to
five rolled, so "deliver twenty-five Core Fragments" was as likely to be a
new run's first job as anything. Seven **starter** contracts now lead the
queue in sector 1 — a first kill, a first delivery, a Mining Node, a Research
Node, a Recharger Node, one frame down a stack, and the breach into sector 2.
Past zone 1 a starter is still offerable, just no longer ahead of anything,
so a mid-run board is untouched and no seeded board moved.

### The board reads anywhere, and signs on the base

One call was answering two different questions — whether there was a board to
draw, and whether the player could act on it — measured as two tiles from the
Broker's own tile. So a player who had built the thing could not read their
own mission status from across their own base, and the board went dark four
frames down a Stack.

`Game::broker_reach` splits it into three states: no Broker, off the base, at
the Broker. The offers list wherever the party is standing, underground
included, because a board seeded off the sector and the epoch makes no claim
about where anyone is — there was never anything for distance to invalidate.
Accepting and delivering still require standing on the base, and the screen's
header says so rather than leaving you to press a key and read a refusal.

"At the Broker" now measures the base slab rather than the distance to the
Broker. A Broker is on the slab by construction, so its tile carries no
information the slab does not, and `CONTRACT_BOARD_RANGE_TILES` was deleted
rather than widened — a constant there would have frozen the desk at the
radius a base *starts* at, while a base's footprint is derived and grows.

### A staff row says what a program is worth at a post

The Base Staff screen's decision is who the scheduler may draw on, and the
row left you to judge by name. Each row now names the program's cycle speed,
extraction aptitude and base job, with its current activity on a continuation
line beneath. `Game::work_profile` answers all three off the same
`SpeciesDef`, so the walk from an entity to its def is written once; the
class label goes through the manifest's existing exhaustive mapping, so a
sixth class cannot ship without deciding what it does at a post.

### Fixed

- **The `0` key crashed any menu that was open.** The digit-to-row conversion
  guarded itself with `d >= 1` but wrote the subtraction inside `then_some`,
  which takes its argument by value — so `0usize - 1` was evaluated on every
  `0` keypress whether the guard held or not, panicking the menu and taking
  the renderer down with it. `checked_sub` makes the absence of a row `0`
  structural rather than a guard someone has to keep in step.
- **A feed buffer needs a neighbour with a reason to run.** A Lathe standing
  beside a Mining Node counted as an attached building on its recipe alone,
  so the node hoarded its whole twenty-unit buffer for a machine that pulls
  nothing while unstaffed. Attachment now means the base has a reason to run
  that assembler — the work-order queue naming what it makes, or a standing
  work job on it. With an order in for Core Fragments, the first fragment
  reached the Depot at tick 500 against the first cycle for the same node
  standing alone.
- **Teardown's description names the salvage it actually adds.** "Work
  resource" is the name of a `SpeciesDef` field, not a word the game shows
  anywhere else, and it said nothing about what a kill drops. It now names
  Core Fragments, and the line comes back under `ROW_WRAP_COLUMNS` — nothing
  clamps a popup row horizontally, and the old 172-column line ran well past
  the body.

## 0.9.2

**Existing saves load unchanged.** `save::SAVE_FORMAT_VERSION` stays at 29.
A save written before this release keeps its level and its banked XP; what
changes is the price of the next level.

### XP measures how hard the fight was, not how big the target was

Levelling was too fast, and the reason was that a kill paid out its victim's
HP bar. A bar is a property of the thing you hit, not of the fight — so the
cheapest way to level was to find the fattest thing you could already beat
without effort and keep beating it. Four fights three frames down a Stack
could hand over five levels.

A kill is now priced by challenge: the victim's bar, scaled by how it
compared to you. The scale is the same one the con colour has always used to
tell you whether something was safe, so what the screen calls a hard fight is
now literally what pays more, clamped between a quarter and double. Something
far beneath you still pays *something* — the floor is there so a player with
nowhere harder to go is slowed rather than stopped — and something far above
you cannot pay unboundedly, because the ceiling is what stops one lucky kill
from skipping a tier.

The denominator is your own power alone, not the party's. Counting companions
would dock you XP for recruiting them, which is the opposite of what the
roster is for.

Measured against the real roster: reaching level 5 in the opening zone costs
about 34 kills where it used to cost about 6. Grinding zone-1 drones from
level 5 to 10 costs 208 kills — the dead end, and deliberately so. The same
stretch costs 37 kills four frames down a zone-1 Stack, which is the way out
of it. Playing on curve, at the level a zone actually asks for, settles at
about 4 kills a level. The numbers and how to reproduce them are in
`docs/measurements/2026-08-15-challenge-xp-pacing.md`.

### Levels come at half the count and twice the size

Slowing XP alone would have made every level worth the same as before while
arriving half as often, which is just a longer game. So the ladder was
rebuilt around it: there are half as many levels to a run, and each one
grants twice as much. Every per-level constant carries that factor, and the
XP a level costs carries its square, so the *power* curve is unchanged —
you are as strong at a given point in the run as you were before, you simply
got there in fewer, larger steps.

Species ability unlock levels moved into the same currency, so a routine
still arrives at the same point in a program's growth rather than at half of
it. That is a data change across all seventeen species files; the schema note
in `assets/species/README.md` says what the levels now mean, and a mod's own
species files should be halved to match.

Both halves of the claim were checked by instruments rather than argued:
`balance_sim`'s reach curve halved while staying linear, and the ability
magnitude pin reproduced its existing band at half the level with the band
itself untouched.

### Fixed

- **A level's XP threshold is derived on load, not trusted from the file.**
  It was already being written into the save and read back out, which meant a
  save written under the old curve would have carried the old threshold into
  the new one. Both load paths now compute it from the level, so the field is
  written and ignored — removing it is what would cost a save-format bump.

## 0.9.1

### Screens

- **The manifest goes back to full size.** 0.9.0 drew the sheet at
  two-thirds scale so it would read as a panel; it reads worse. The frame is
  back at 92% x 90% of the window and the page is back on the window's own
  metrics, so `manifest_layout` no longer scales anything or hands metrics
  back to the caller. `M` on the roster is untouched — that half of 0.9.0
  stays.

## 0.9.0

**Saves written by 0.8.x still load.** `save::SAVE_FORMAT_VERSION` is
unchanged at 29, so by the rule above this would ordinarily be a patch. The
minor is deliberate: seventeen changes landed on `main` between the 0.8.35
release and this one without being cut individually, and one number covering
the lot reads more honestly than back-dating sixteen sections. The
per-change rule stands; this is the correction, not a new practice.

### The Stack describes itself as you walk it

The description bank shipped with three lengths of prose per subject, and
the one-sentence length had a single reader: the line that fires the first
time a feature comes into view. So a plain corridor said **nothing at all**
— the floor and doorway prose was written, shipped, and unreachable — and
ground already on your map was silent forever after, which made walking back
through a frame a wordless trudge.

There are now two narration axes. **Discovery** is unchanged: the first
sight of something worth walking to, once ever. **Passage** is new, fires as
you arrive somewhere, and describes whatever your line of sight settles on
ahead — with no notion of *new*, so a corridor you have walked ten times
still has something to say about itself.

Both resolve through one pick, the same one the examine key uses, so the
corridor cannot announce a cache that `x` then declines to name.

**Which cells speak is a property of the place**, folded out of the frame
and the coordinates rather than rolled — so a corridor keeps its rhythm
across a save and reload and on every later walk, and opening a log line
costs nothing from the run's shared randomness. Roughly one cell in three,
which is a first guess at a feel question.

A **live Stack market** is now announced like any other find. It was the one
unspent feature missing from that ranking, so nothing ever mentioned a stall
and the examine ray looked straight through one.

### Work orders: the base fetches, delivers, and finishes

The base that started staffing itself in 0.8.35 was still hoarding and
still hand-making things it already owned.

**A worker fetches from a Depot before making the thing.** The scheduler
worked deepest-first, so a feeder outranked the bench it fed and your one
spare body went upstream to hand-make what was already sitting on a shelf.
A full batch on a shelf now settles it. One statement of "is a batch of this
in reach" answers for both the scheduler and the walker, so a bench that
gets staffed is a bench that gets fetched for.

**A machine with nothing downstream delivers as it produces.** Pickup used
to wait for a clog, so a lone extractor sat on twenty units the base could
not count. If no neighbour's recipe wants its output, the worker walks each
cycle's payout to the nearest Depot. Depot placement now paces a lone
extractor — deliberately, and the number most likely to want retuning after
play.

**A work order staffs every machine that makes the item, not one.** Deploy a
second Mining Node because the first one's output is eaten by the assembler
beside it, and it used to stand empty for the rest of the run. The mirror
failure went too: an unfed twin bench in a corner no longer refuses an order
the base can already fill through a line that is whole.

**Cancelling an order mid-walk no longer destroys the load** the worker was
carrying, and a **finished order reads green** in the base log instead of
sitting dim among the routine payouts.

### The Stack

- **An escort's death no longer collapses the stack.** Cut down one of a
  guardian's escort, jack out with the guardian untouched, and the lair was
  spent and the whole stack folded on the way out.
- **A guardian can no longer be walked off with.** Taming one recorded
  nothing, so the lair refilled on the next visit over a stack that could
  never be finished — and the guardian could be farmed indefinitely.

### Battle

- **The battle log can be walked back.** Narration that outgrew the pane was
  gone from every screen in the game — a round runs to 18 lines against a
  pane seating about 15, and the results page lands the salvage tally and
  every fighter's XP in the same space. Up and Down now walk the window on
  both the battle screen and the results page, with a hint naming what is
  out of sight.
- **The summary reports one decompile verdict, not one per catalyst.** Six
  catalysts fed to one program left six near-identical refusals on the
  results screen, only the newest of which still said anything true.

### Screens

- **The manifest reads as a panel** at two-thirds its old scale, and **`M` on
  the roster** opens the highlighted program's sheet — Esc comes back with
  that row still highlighted.
- **A roster row and an inventory row shed their tags** onto an indented
  continuation instead of running off the popup. The worst roster row ran
  382px past the edge, taking the activity and CRITICAL marks with it; the
  worst inventory row ran 68px over, taking the whole answer to what the item
  would do if you put it on. Ordinary rows are untouched.
- **The last perk's description is no longer torn off the list.** It was
  drawn detached at the foot of the box under a blank scroll indicator. The
  research picker had the identical defect and gets the identical fix.
- **An idle program stops claiming to be in your party** on the Base Staff
  screen. "Neither staff nor party" is the state a program is tamed into and
  the one it is stood down to — the common case read as the one it was least
  likely to be.
- **A party drain narrates green** like a patch, keeping its numbers. It is
  the one line in the game with a figure on either side of it.

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

