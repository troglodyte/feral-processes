# Base staff interactions

A review of everything a program does once it works at your base: how it gets a job, what it needs, what it remembers, how it sulks and fights, how it gets hurt and mended, and where you can see each of these.
It describes the code on `feat/social-behaviours-survey` as of 2026-09-16, read from source rather than from the design notes.
Nothing here has been playtested.

Times are given in ticks and, where it helps, in real time.
While you stand still the world ticks twice a second, and every action you take spends ticks too, so "about 17 minutes" means 17 minutes of standing still.

## Who counts as staff

Every program you own has exactly one of four roles, and base staff is simply the one left over.
Nothing assigns it and no key toggles it.

| Role | What decides it | Needs drain? | Can be given base work? |
| --- | --- | --- | --- |
| Wielded | You hold it as your weapon | No | No |
| In party | It fights beside you | No | No |
| On sortie | It is away on an expedition from a Dispatch Relay | No | No |
| Staff | Everything else you own | Yes | Yes |

So a program you tame while your party is full, one you stand down from the party, and one a town gifts you all become staff the moment they join.
A program that has never set foot in the base (one beaten out on the grid) first appears on a ring three tiles out from the Home, then walks from there.

How many programs you may own is capped at three, plus five for each Data Cache.
That cap is the whole limit on how big your workforce can get.

Where it lives: `game/party.rs` (the role rule), `Game::base_staff`.

## How a program gets a job

You never post a program by hand.
You tell the base what you want, and every tick it works out who stands where.

### What the base wants, in priority order

Each tick the base builds one list of jobs that want a body, in this order:

1. Build and upgrade requests you have filed.
2. Fetching fuel for a Recharger Node that is running low.
3. The research project you picked.
4. Your work orders, top of the queue first.
5. Standing instructions you set on individual machines ("keep this worked", "keep this guarded").
6. Marked rock to cut and floor to lay.

If there are fewer bodies than jobs, the list is cut from the bottom.
Filing a build request on a short-handed base therefore takes someone off a machine until the structure is up, and digging only happens with spare hands.
The work order screen's header shows how many hands the base is short.

### Who fills a job

Programs already standing at a job that is still wanted stay put, so work in progress is not thrown away when a buffer changes by one unit.
A body carrying a load is never pulled off mid-delivery, and one standing at a full machine is kept on it when a Depot exists, because it is the only one who can carry the surplus away.
Everyone else is freed, and free programs are handed the remaining jobs.

A job is skipped when no free program can walk to it.
A build request or marked cell that nobody can reach is announced once ("...is cut off — no program can find a way to it") and set aside until a route opens.

Every question about walking is asked from where the program is actually standing, never from where you are.

### The kinds of work

- **Machine work.** The program walks to the machine and runs its cycle. If the base has a Depot, it carries up to 5 units per trip to a shelf when the machine fills up or when nothing next door will use the output.
- **Guarding.** Standing instructions only. The program stands at a structure to defend it from GC Entropy Sweeps.
- **Building.** The program fetches each material by hand from any shelf or machine output (and from your pack while you are in base space), sets it on the site, and raises the structure at 2 ticks per unit delivered.
- **Cutting and flooring.** The program swings at marked rock every 12 ticks, doing damage that scales with the program's strength but capped per rock kind, and fetches floor tiles the same way a builder fetches materials.

A few species classes do something extra at a post.
A Leech pulls one extra unit from each successful extraction cycle.
A Bastion counts its Defense twice when a sweep hits the structure it is at.
A Medic on guard duty restores 2 Durability to that structure each repair interval.

How good a program is at a machine comes from its species.
Each point of speed away from the default shortens or lengthens the cycle by 5%.
Each point of Analysis moves the chance an extraction cycle lands by 2%.

### Idle programs

A program with nothing to do mills about the base.
It only ever steps onto laid floor, so the paving is its leash and it can never wander into a corridor that might be sealed behind it.
It will not step onto a structure, onto your tile, onto a tile another idle program holds, or onto a tile it holds a grudge against (see [Opinions steer where a program walks](#opinions-steer-where-a-program-walks)).
It has no random element: the direction comes from the program's place in the list and the clock.

Where it lives: `game/base/work_orders.rs` (the scheduler and the wander), `game/base/hauling.rs`, `game/base/construction.rs`, `game/base/upkeep.rs` (class jobs).

## Needs

A staff program carries two reserves, each running from 0 to 100 and starting full.

| Need | While off shift it is… | Falls per tick (idle / working) | Leaves its post below | Returns to work at | Refilled by |
| --- | --- | --- | --- | --- | --- |
| Coherence | Defragmenting | 0.02 / 0.04 | 20 | 60 | Log Analyzer Bay, 0.6 a tick, standing on one of its four sides |
| Slack | Idling | 0.012 / 0.0168 | 25 | 70 | Sandbox, 0.5 a tick, standing on any of the eight tiles around it |

In play, a steady worker needs to defragment after about 2,000 ticks (roughly 17 minutes) and to idle after about 4,500 ticks (roughly 37 minutes).
A defragment takes around 70 ticks and an idle around 90, plus the walk.
Both amenities cost Core Fragments only, need no research and no program to build.

Party members, the wielded program and programs away on sorties do not drain at all; their reserves freeze until they are back on staff.

### Going off shift and back

When a reserve falls below its "leave" line, the program drops what it is doing (unless it is mid-delivery), walks to the nearest amenity that services that need, and stands there until the reserve reaches its "return" line.
The gap between the two lines is what stops a program flickering on and off its post every tick.
If two needs are low at once, it deals with the one furthest below its own line first.

Standing near an amenity refills a reserve whatever brought the program there.
A program posted to a machine right beside a Sandbox is topped up while it works and may never need to leave.

### When nothing answers a need

If the base has no amenity for a need at all, the program keeps working.
The log says so once: "*name* is out of Coherence and there's nothing in the base that restores it."

If an amenity exists but the program cannot walk to it, it gives up, goes back to work, and the log says: "*name* can't find a way to anything that would restore its Coherence."

Either way the program won't say it again for that need until the reserve has somehow climbed back above its line.

Once the base is **established** (at least 8 staff and at least 8 structures), running dry also leaves a memory.
A base with no amenity leaves a "Ran down" memory that blames nothing in particular.
A base with an amenity the program could not reach leaves "Worn thin here", a grudge against the tile it was standing on.
Before that point, needs never count against a young base.

### What a low reserve costs

A low reserve drags on how reliably the program extracts.
Each need drags by up to its weight when empty (Coherence 4 points, Slack 3), easing to nothing at its "return" line, and each point costs 1% on the extraction roll.
So a program with both reserves at zero lands about 7% fewer cycles, and one waiting at its leave lines about 4.5% fewer.
For scale, a Mk1 Mining Node lands half its cycles.

### Temperament

Every program is born with one of five hidden temperaments, fixed for life and never shown on any screen.
Two of them affect needs: a **Languid** program's reserves fall 30% faster (so it takes more breaks and reads as lazy), and a **Dogged** one's 30% slower.
The other two affect memories (next section), and **Steady** is neutral.

Where it lives: `assets/needs/`, `assets/structures/sandbox.ron` and `defrag_bay.ron`, `game/base/offshift.rs`, `systems.rs` (the drain), `disposition.rs`.

## Memories and opinions

Every owned program keeps a short list of memories: one entry per kind of thing per subject.
Wild programs, structures and you carry none.
Memories are never announced in the log; the program's pages are where they show.

### What gets remembered

| Memory | Feels | Strength per time | Stacks up to | Fades by half in | About | Formed when |
| --- | --- | --- | --- | --- | --- | --- |
| Fought beside | fond | +4 | 5 times | ~33 min | another program | you win a fight, for each other survivor |
| Won against the odds | fond | +5 | 3 | ~42 min | nothing | you win a fight you were outmatched in |
| Mauled by | grudge | −8 | 4 | ~50 min | a species | one landed hit takes more than 35% of its maximum Integrity |
| Idled with | fond | +4 | 4 | ~33 min | another program | it finishes refilling a need, for each other staff program standing at the same amenity |
| Settled in at | fond | +3 | 4 | ~25 min | a machine kind | every ~2 minutes it is posted to a machine that isn't backed up |
| Always jammed | grudge | −4 | 3 | ~25 min | a machine kind | every ~2 minutes it is posted to a backed-up machine |
| Cutting rock | grudge | −3 | 4 | ~33 min | digging | every ~2 minutes it is on the dig crew |
| Swept while working | grudge | −5 | 3 | ~29 min | a machine kind | a sweep damages the machine it is posted to |
| Left stranded here | grudge | −6 | 3 | ~25 min | a base tile | it can no longer walk to its post |
| Worn thin here | grudge | −7 | 3 | ~25 min | a base tile | it runs a need dry beside an amenity it cannot reach |
| Ran down | grudge | −6 | 4 | ~33 min | nothing | it runs a need dry in a base with no amenity for it |
| Unwound at | fond | +3 | 3 | ~21 min | an amenity kind | every ~2 minutes it stands at an amenity while in a bad mood |
| Let it out | fond | +5 | 3 | ~17 min | nothing | it comes out of a tantrum it started |
| Turned on me | grudge | −7 | 4 | ~42 min | another program | a colleague started a tantrum against it |

The "every ~2 minutes" memories come from a 250-tick beat shared by the whole base, so a program reaches a full stack after about 8 minutes at the same kind of job.
Memories about a machine name the machine's *kind*, so a rebuilt Lathe is the same Lathe to the program.

### How memories stack and fade

Each time the same thing happens again, the memory gains a strike (up to its stack limit) and its fade clock restarts.
A memory's strength is its per-time strength times its strikes, halved for every half-life since it last happened.
A fully stacked "Left stranded here" is worth −18 fresh, −9 after 25 minutes, and so on.
Fading only ever shrinks a memory toward zero; a grudge never turns into a fondness.

A program holds at most 12 memories.
When it forms a new one, anything that has faded below half a point is forgotten, and if it is still over 12 the weakest (by size, fond or not) goes.

Temperament changes how memories *land*, not what is kept.
An **Amiable** program feels its fond memories 40% more and its grudges 40% less; an **Abrasive** one the reverse.

### The two new dials, `stack_decay` and `mood`

A memory kind may now declare two optional settings.
Neither shipped memory sets either one, and at their defaults the game behaves exactly as described above.

- **Stack decay** (default 1) makes each extra strike worth less than the one before it. At 1 every strike is worth the same; at 0.5 the second strike is worth half the first, the third a quarter, and so on.
- **Mood** (default 1) is the share of a memory that counts toward the program's overall mood. The program's opinion of the specific subject still feels the whole memory. At 0 a memory steers behaviour (where it walks, whom it picks a fight with) without ever touching mood.

### Mood (morale)

A program's mood is the sum of everything it currently remembers.
The screens show it as a word plus the number:

| Mood | Word |
| --- | --- |
| +20 or more | devoted |
| +10 to +20 | content |
| between −10 and +10 | even |
| −10 to −20 | uneasy |
| −20 or less | bitter |

Mood moves how reliably the program extracts: half a percent per point, capped at 10% either way (reached at ±20).
It affects nothing else about the program's work directly; its real teeth are the bad-mood ladder below.

### Opinions steer where a program walks

A program's opinion of one particular thing is the sum of its memories about that thing alone.
Three behaviours read opinions rather than overall mood:

- An idle program will not step onto a base tile it holds −3 or worse against. One stranding keeps it off that tile for about one half-life.
- A sulking program (see below) will refuse a machine *kind* it holds −3 or worse against, in a base with no amenity.
- A program lashing out picks the colleague it likes least.

A fondness never triggers any of these.

Where it lives: `assets/memories/`, `game/memories.rs`, `memories.rs`, `components.rs` (the fade formula), `disposition.rs`.

## Bad moods

A program whose mood falls far enough starts acting out, on a three-rung ladder.
It climbs the ladder as its mood worsens but never steps back down a rung; it comes off the ladder entirely once its mood recovers to −6 or better.

| Rung | Starts at | What the program does |
| --- | --- | --- |
| Sulking | −8 | Takes a break at an amenity if the base has one. If not, keeps working but refuses machines it resents. |
| Downed tools | −50 | Stops taking jobs altogether, amenity or not. Takes a break if there is an amenity. |
| Lashing out | −75 | As downed tools, and may start a fight with a colleague. |

One bad memory is enough to make a program sulk.
Downing tools takes a pattern: the worst single memory the game ships (a fully stacked "Mauled by" felt by an Abrasive program) reaches about −45.
Lashing out takes at least two maxed grudges.

### Taking a break

Mood has no reserve to refill, so a break works through memory: while a program in a bad mood stands at an amenity, it gains an "Unwound at" fondness every ~2 minutes, worth up to +9.
Walking there earns nothing; only arriving counts.
That is usually enough to lift a sulker back over −6 in one or two beats, and nowhere near enough to rescue a program at −50 on its own.
A program that has downed tools mostly has to wait for its grudges to fade.

Any amenity will do, whichever need it services.
The program goes to the nearest one.
If it cannot walk to any, the log says "*name* can't find a way to anywhere in this base worth stopping at." and the program goes back into the pool of workers (still disgruntled) rather than standing stuck.
No grudge is added for that failure.

The practical effect is that building the base's first amenity turns a sulk from "won't work at the Lathe" into "off the line entirely until it feels better".

### Tantrums and brawls

Once the base is established (8 staff and 8 structures), a program on the lashing-out rung with a colleague within 3 tiles has a 2% chance each tick of starting a fight.
Because such a program is usually walking to or standing at an amenity, fights tend to break out there.

- The target is the colleague within reach that it likes least, then the nearest.
- The fight lasts 4 to 8 exchanges. Each exchange both sides swing once, each blow worth a quarter of the target's maximum Integrity before its Defense.
- **No blow can take a program below 1 Integrity.** A tantrum never kills directly.
- Afterwards the aggressor gains "Let it out" and the target gains "Turned on me" about the aggressor. The grudge outlasts the relief, so a base that does nothing about a bad mood gets worse.
- The aggressor cannot start another fight for 400 ticks (about 3 minutes).
- A fight ends early if either side is downed, leaves base staff, or is destroyed.

The log reads "*A* rounds on *B*.", then "*A* hurt *B* in a tantrum, causing N damage.", "*B* fought back, causing N damage." (left out if *B* never landed a blow), and "*A* came out of it calmer. *B* will not forget it."
These lines are orange and tagged ALERT in the log pane.
Each landed blow flashes red on the map and plays one hit sound.

A four-exchange fight is sized to leave the loser under 20% Integrity, which sends it to a Repair Bay (next section).

Where it lives: `game/base/morale.rs`, `game/base/tantrum.rs`, the "Acting out" and "Staff tantrums" sections of `tuning.rs`.

## Injury and recovery

### Breaking off for repairs

A staff program that falls below 20% of its maximum Integrity leaves its post and walks to the nearest Repair Bay — but only if a Bay exists.
The log says "*name* breaks off for repairs."
It stands on one of the Bay's four sides and regains 1 Integrity a tick.
It goes back to work only at full Integrity: "*name* is back on its feet."
A Bay has no capacity limit; everyone in reach mends at once.
Repair outranks a need or a break: a hurt program goes to the Bay first.

A green bouncing `+` is drawn over each program that is being mended.
The roster calls it "recovering".

With no Bay standing, a hurt staff program just keeps working at whatever Integrity it has.
Resting heals only your party and wielded program, so without a Bay nothing restores a staff program's Integrity short of moving it into the party.

### Being downed by a death

Under **Forgiving**, a program that dies (in one of your fights, or defending the base from a sweep) is not lost.
It drops out of the party and off its post, is left at 1 Integrity, and is marked as downed.
It still takes its roster slot, and it cannot rejoin the party ("That program is down and needs a Repair Bay...").
It walks itself to a Bay and mends like any other patient.
If no Bay exists, a notification explains that the program needs one, and until one is built the only ways to free the slot are selling the program or extracting a routine from it.

Under **Permadeath**, the program is destroyed.

A downed program cannot be sent on a sortie or spent on a build.

Where it lives: `game/base/repair.rs`, `Game::bench_or_dissolve` in `game/trade.rs`, `assets/structures/repair_bay.ron`.

## Programs spent on building

Raising or upgrading any structure that runs a job (an extractor, an assembler, a teardown rig) costs one tamed program, chosen from a picker when you file the request.
The program is gone from the roster from that moment.
Structures that run no job (amenities, the Repair Bay, the Dispatch Relay, Data Caches, shelves, Shields, the Zone Portal, the Home) cost no program.

- An upgrade to Mk N needs a program originally from zone N or deeper.
- You cannot spend your last program.
- A program away on a sortie, downed, or carrying a load cannot be spent.
- Calling off the request brings the program back exactly as it was, memories and all ("*name* comes back off the job.").

### Build quality

The program you spend leaves its mark on the machine.
Each program has two build rolls, separate from its four combat rolls: an **assembly roll** for machines that make things and an **extraction roll** for machines that pull from the ground.
The relevant roll, plus a small lift for rarer programs (3% per rarity rung), becomes the machine's build quality.

Build quality speeds up or slows down the machine's cycle for as long as it stands.
Across the full roll range, a machine runs somewhere between 10% slower and 10% faster than its shipped rate.
This multiplies with the speed of whoever is posted there later.
An upgrade replaces the machine's build quality with that of the program spent on the upgrade.

The program picker shows each candidate's relevant roll as a word (Poor to Excellent) and the machine's cycle before and after, in ticks.
Both build rolls also appear in the POTENTIAL box of the program's manifest.
They are deliberately left out of the overall quality percentage, so an excellent fighter can be a poor builder.

Where it lives: `game/base/building.rs` (`build_quality`), `game/base/construction.rs`, `game/party.rs` (`commit_program`), `systems.rs` (the cycle formula).

## Names and handles

Every owned program has a **handle**: `0x` followed by six hex digits in mixed case, such as `0x435eaD`.
It is worked out from the program's permanent id, so it never changes, is unique, and needs nothing saved.
Wild and summoned programs have no handle and go by their species name.

You can give a program your own name (up to 12 characters) with `N` on the roster.
A blank entry removes it and the program goes back to its handle.

| Form | What it looks like | Where it shows |
| --- | --- | --- |
| Long | rarity tier + handle + species + zone, e.g. `Overclocked 0x435eaD Scrapper 3`; a custom name drops the species | Log lines, the roster, the manifest header, most popups |
| Short | handle (or custom name) + zone, no tier or species | The CREW pane's unit column, the party battle roster, the subject of a memory about another program |

A memory about another program keeps that program's name as it was when the memory last happened, so it still reads correctly after the other program is gone.
The roster is sorted by role, then party slot, then species, then the order you acquired them — never by handle.

Where it lives: `handles.rs`, `Game::creature_label` and neighbours in `game/party.rs`.

## GC Entropy Sweeps

From zone 2 onward, each tick has a 1.2% chance of a sweep, provided the base has at least 5 staff who are neither downed nor have downed tools.
A sweep picks a random structure (never the Home).

- **Nobody posted there:** the structure loses 4 Durability, less the base's Shield defence. At zero it is destroyed.
- **Somebody posted there:** that program's Defense cuts the damage by that percentage (a Bastion's counts double). The program takes 6 damage whether or not the damage was fully turned aside. If the cut is total the log says "*name* fends off a GC Entropy Sweep on *structure* without a scratch!"
- **The defender dies:** "*name* is destroyed defending *structure*." It is then downed or destroyed according to difficulty.
- **Anyone posted to a structure that actually takes damage** gains "Swept while working" about that machine kind.
- **If the structure is destroyed,** everyone posted there loses their job and drops what they were carrying, and a program a rig was carrying is put back somewhere safe.

A defender that survives but falls under 20% Integrity breaks off for the Bay.
Posting a Medic on guard at a structure mends it between sweeps.

The first sweep opens a notification, and the breach into zone 2 warns that sweeps are coming.
Sweep lines are orange and tagged ALERT in the log.

Raiders from a Hostile town are a separate event: they take Credits from your stores and do not touch staff.

Where it lives: `Game::raid_check` and `Game::damage_structure` in `game/base/upkeep.rs`.

## Sorties

A Dispatch Relay (unlocked by research) sends staff programs away on expeditions.

- Only staff can go, and only if not downed and at half their Integrity or better.
- You cannot send the whole staff; at least one must stay.
- Provisions come out of the base's stores.
- While away, a program is not staff: it is not scheduled, does not wander, its needs freeze, and it does not count toward the five defenders sweeps need.
- The fights happen off-screen but by the real combat rules. A program can come home with "Mauled by" memories; there is no "Fought beside" for sortie fights. XP is 60% of normal, and each survivor recovers 15% of its Integrity after each fight.
- A casualty ends the trip. Under Forgiving the casualty comes home downed; under Permadeath it is gone ("*name* did not come back.").
- The squad is drawn walking out of the base's door when it leaves and back in when it returns. The log says "*names* ship out for *site*." and "*names* came back from *site* — N down, N XP."

Where it lives: `game/sortie.rs`.

## Where the player sees all this

| Surface | What it shows |
| --- | --- |
| Base Staff screen (base menu) | Every program you own, its role, and for staff what it is working on ("working the Mining Node", "cutting the entropy at 3, 4", "raising the Lathe", "idle"), plus speed, Analysis and class job |
| Roster (companion screen) | What each program is doing: "recovering", "equipped as weapon", "in party", "defragmenting", "idling", "taking a break", its post, or "idle". `N` renames, `M` opens the manifest, `R` opens memories |
| Manifest, POTENTIAL box | The four combat rolls and the two build rolls |
| Manifest, WORK box | Speed, Analysis, class job, current post, and each need as a word (steady, fraying, strained, critical) with the errand verb if it is off shift for it |
| Manifest, MEMORIES box | Mood word and number, then the strongest few memories |
| Memories page (`R`) | Mood header, then every memory, strongest first, with its subject, strength, how long ago in words ("just now", "recently", "a while ago", "long ago") and its flavour line |
| Work order screen | How many hands the base is short, which grows while programs are off shift, on a break, downed or on strike |
| Build program picker | Each candidate's relevant build roll and the machine's cycle before and after |
| Map | A small bobbing mark on whichever end of a job the worker is at (blinking when the worker is stranded); machine status colours; the green `+` over a mending program; red flashes and a hit sound for brawl blows; idle programs milling on floor; squads walking out and in |
| Log | Needs going unanswered, breaks that can't be reached, repairs starting and finishing, stranded and idle machines, tantrums and sweeps (orange, ALERT), build completions, sortie departures and returns |
| Notifications | First sweep, sweeps about to begin, a program downed with no Repair Bay |
| HUD attention badge | Damaged structures and machines with nobody posted; nothing about moods, needs or downed staff |
| Examine | A program's current errand, the same words the roster uses |

Temperament appears nowhere.

## Open questions and oddities

These came out of reading the code.
None were fixed, and the ones marked *unverified* were not checked with a test.

1. **Idle programs may stride rather than mill.** The comments say an idle program holds a tile for 6 ticks and then drifts one step. The wander runs every tick with a direction that only changes every 6 ticks, so it looks like a program walks up to 6 tiles in a straight line and then turns. *Unverified.*
2. **A program sent on a sortie may keep its job.** Nothing in the dispatch clears a posted program's assignment. If so, the scheduler treats that machine as still staffed while the program is away, a sweep there can pick the absent program as its defender (and deal it 6 damage), it keeps forming "Settled in at" memories, and the roster and Base Staff screen describe its old post (or "taking a break") instead of the sortie. *Unverified.*
3. **"Settled in at" is formed at any machine that isn't backed up**, including one that is starved, dark, out of fuel, or one the worker is stranded from. A permanently stranded worker gains more from the fondness (+12) than it loses from the stranding (−6).
4. **Two screens disagree about what a staff program is doing.** The Base Staff screen says "idle" for a program that is defragmenting, on a break or being repaired; the roster names the errand.
5. **"Recovering" is shown with no Repair Bay**, where nothing will ever heal the program.
6. **Staff never heal without a Bay.** A long brawl leaves both sides on 1 Integrity. The brawl itself is non-lethal, but in a base without a Bay those two stay at 1 until a sweep's 6 damage to a defender destroys or downs one of them.
7. **A defender takes 6 damage even when the Shield network had already cancelled the sweep**, and the defender is whoever is assigned to that structure, even if it is off carrying a load at the time.
8. **The mood words and the ladder don't line up.** A program sulks at −8 while the screens still say "even"; −20 and −50 (downed tools) both read "bitter"; the extraction effect stops growing at −20, well before downed tools. A memory note already records this as unmeasured.
9. **Every acting-out and tantrum threshold is unmeasured**, as the tuning comments say. Nobody has watched a base climb the ladder, and whether downing tools or lashing out is reachable in normal play is unknown.
10. **A need's "morale weight" never reaches mood.** It only drags extraction. The needs README says it is what an empty reserve is "worth to morale", which a designer could reasonably read as the mood the screens show.
11. **Needs barely move mood.** "Ran down" and "Worn thin here" are written once per program per need and not again until the reserve recovers, which in a base with no amenity it never does. So a neglected base costs each program one fading −6 per need: enough to sulk, never enough to down tools.
12. **The sulking refusal only exists in a base with zero amenities.** Once any amenity stands, sulkers leave for a break instead.
13. **Mood and needs only touch four machines.** They move the landing chance of the Mining Node, Research Node, Log Scraper and Cache Tap only. Assemblers, the Power Conduit, builders and diggers work the same in any mood. The 15% cap on need drag can never be reached with the shipped needs (the most is 7%).
14. **Digging is the one job that sours a program just by doing it.** A Steady digger reaches the sulking line after about three ~2-minute beats, an Abrasive one after two, and a dig job is never refused. With an amenity this can loop: dig, sulk, break, recover, dig again.
15. **Fond memories about colleagues are one entry per colleague**, so a sociable program at a busy amenity, or one from a full party, can stack many +16 to +20 entries up to the 12-memory limit. The positive side may dwarf every grudge; this is unmeasured.
16. **Breaks pay only on the shared 250-tick beat**, so a program that arrives just after a beat waits up to two minutes before its break counts.
17. **Bad-mood and off-shift markers stay on a program in the party**, frozen, and resume when it returns to staff.
18. **Some log lines can no longer appear.** "It stands down as your companion to run this cronjob" (and its guard, dig and build siblings) is only printed for a party member being posted, and the scheduler only ever posts staff.
19. **Rename wording.** Clearing a name logs "*name* goes back to being a plain 0x435eaD Scrapper 3", and code comments on the rename and fusion prompts still say a blank name restores the species name.
20. **Open fights and fight cooldowns are not saved**, so a reload silently ends any brawl in progress. The stranded marker isn't saved either, so reloading with a stranded worker may add a second "Left stranded here" strike. *Unverified.*
21. **Sweeps switch off when staff are away or on strike.** Sending programs on sorties, or a base where enough programs have downed tools, can drop below the five-defender floor and stop sweeps entirely. The code comments intend this for strikes; for sorties it is a side effect.
22. **Sweep frequency looks high.** A 1.2% chance per tick is roughly one sweep every 40 seconds of standing still once the base qualifies. Worth a sanity check against intent.
23. **No HUD signal for staff trouble.** A strike, a brawl or a program stuck without a Bay reaches the player only through the log and the program's own pages.
24. **Temperament is invisible**, by design. A player has no way to learn why one program keeps wandering off or holds grudges, except by comparing programs over time.
