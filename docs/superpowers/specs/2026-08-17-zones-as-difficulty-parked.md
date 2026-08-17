# Taking the thought of zones away — parked brainstorm

**Status: PARKED mid-brainstorm on 2026-08-17. Not a design, not approved,
nothing implemented.** No approach was chosen. This file exists so the
findings below — which cost real tool calls and correct two stale seam docs —
don't have to be rediscovered.

Read `INDEX.md`'s warning about `**Status:**` headers before trusting any
other spec's; this one is accurate as of the date above and will rot the same
way.

## The question asked

> What if we took away the thought of zones, and had the enemies autoscale
> based on total party strength? Or maybe by stack level?

## What the itch actually is

Not a difficulty-curve complaint. Verbatim:

> Zones are interchangeable, it feels awkward to play, like an endless grind
> for no reason but to 'advance' to the next zone.

So the target is **motivation**, not tuning. Breaching pays the player in
nothing but permission to keep playing.

Preferred scale, from the same round: **distance from home *and* Stack
depth**. Both spatial. World shape and the treadmill question were both left
as "not sure yet".

## Findings that survive the pause

### `ZoneLevel` is one number wearing three hats

Separating them is the whole design problem. Autoscaling on party strength or
depth replaces hat 1 only; hats 2 and 3 are where the work is.

1. **The difficulty dial.** `ZoneLevel::stat_multiplier` (linear,
   `1 + ZONE_STAT_STEP * (z - 1)`) on every wild program's stats, and
   `zone_group_cap` on how many bodies a pack may field.
2. **The unlock ladder.** `min_zone` on 12 research nodes and 8 contracts;
   `min(def.max_tier, zone)` on structure upgrades; `GEAR_VALUE_PER_ZONE` on
   drop bands; `NODE_PAYOUT_ZONE_BONUS` on node payouts;
   `Trigger::ZoneReached` achievements; the `ZoneLevel <= 1` starter-contract
   gate; `ZoneLevel::raised_a_tier` for a Recompile Kernel's bump.
3. **Place identity.** `sectors::map_for_zone(seed, zone, &sector_db)`
   generates the world and its traits; `enter_next_zone` wipes hostiles,
   nests, `SurfaceLink`s, `BuybackLedger` and `StackMemory` **by name** and
   carries the base across on snapshotted offsets.

### The crux: hat 2 is the grind

Every gate in hat 2 keys on a number whose only meaning is *how many Portals
the player has funded*. Rekey them to a monotonic high-water mark of danger
actually beaten — working name **`Clearance`** — and "advancing" becomes
"I killed the thing" instead of "I saved up fragments".

Monotonicity is required, not optional: `tests/research.rs:772` asserts
`prereq.min_zone <= def.min_zone` across the whole research tree. A
high-water mark satisfies it, so `min_zone` → `min_clearance` is a rekey
rather than a redesign. Achievements, gear bands and payouts follow.

**This part is common to all three shapes below** and is separable — it could
ship alone and be played before the world shape is decided.

### `spawning.rs::danger_steps` is the seam to widen

`fn danger_steps(&self, depth: Option<u32>) -> u32` is already documented as
*"the one input both group curves take, so the two halves of the pack ceiling
cannot disagree about how dangerous a place is"*, and already switches on
surface-vs-Stack: the zone above ground, `depth` below.

Its `Option<u32>` parameter shape is **already the anti-leak fix** — callers
pass the escalation they are at rather than reading the party's locale,
specifically so ambient surface spawns and nest respawns (which keep rolling
every tick while the party is underground) aren't sized from the party's
depth. Any generalisation should keep that shape.

The stat multiplier does *not* currently read this seam; it reads
`ZoneLevel::stat_multiplier` directly. Making both read one place is part of
the job.

### Two stale seam docs — corrected here

**`Game::distance_stat_multiplier` does not exist.** Six doc comments
reference it as though it does: `tuning.rs:811`, `tuning.rs:1094`,
`resources.rs:797`, `resources.rs:1149`, `game/zone.rs:659`,
`game/stack.rs:735`. It was deleted on 2026-08-05 in commit `30608eb`
*"feat(spawning): difficulty comes from the zone, not from how far you
walked"*. **The game has one autoscale axis (depth), not two.** Fix these six
comments whenever this is picked up, or independently — they are wrong now.

`docs/seams.md:424` gives two reasons distance stopped scaling stats and
group size (it used to reach 3x). They are not equally binding:

- *"a zone had no consistent difficulty of its own"* — a bug **about the
  zone/distance conflict**. Delete zones and it evaporates. It is closer to
  the feature being asked for than to an obstacle.
- *"it leaked underground — every Stack spawn is placed at the surface
  entrance tile, so descending through a far-flung link scaled the whole
  frame by that link's distance"* — **real and independent.** A direct
  consequence of the load-bearing Stack decision that the party's `Position`
  stays pinned to the entrance tile while underground, so there is no
  underground tile to read a distance from. This is the engineering problem
  any distance-keyed curve has to answer. `danger_steps`' `Option<u32>` is
  the existing shape of that answer.

`seams.md` currently ends that entry with "A new difficulty knob keyed to
where the party is standing reintroduces both bugs." That is true of a naive
one and overstated as a general claim — only the second bug is intrinsic.

### The treadmill question answers itself

Party-strength scaling produces the Oblivion problem: getting stronger
changes nothing. *Geographic* scaling does not — threat is a property of
place, so power buys literal new territory. Choosing distance + depth as the
axes retires the question. **Autoscaling on total party strength was the
opening idea and is the one thing here worth not doing.**

## The three shapes, undecided

**A — One world, danger is a map you learn.** Delete `ZoneLevel`, no
breaching. Surface threat is a distance band from home; Stack threat is
entrance band + depth. Sector traits become regional traits inside one large
persistent map. Purest answer to the itch — a frontier to push, never a
counter to advance. *Costs the most:* Portal Fragments lose their entire
purpose, taking a currency, a structure, `collapse_stack`'s renewable-supply
argument and the "fragments are earned only by fighting and descending"
invariant with them. World generation must produce one large regionally
varied map instead of N sector maps. Save format break.

**B — Depth is the whole ladder, the surface is home.** Flat surface forever;
all escalation is the Stack. `Clearance` is the deepest frame cleared. Portal
Fragments repurpose from "next zone" to "deeper stratum", each stratum
visually distinct. Leans on the most-built subsystem. *Strands built
content:* a permanently safe surface deletes the point of wild population,
nests, raids and the opening ring, and sector traits (shipped 0.8.14, still
never seen on screen) become dead code.

**C — Sectors stay places; the number dies.** *Recommended at the time of
parking.* `ZoneLevel(u32)` stops being a sequence counter and becomes a
sector's own threat rating, generated from its traits — so the player
breaches into a mild sector or a savage one **and can see which before
going**. Distance-from-home and depth are the gradient *within* a sector,
both feeding the widened `danger_steps`. Breaching becomes a choice with a
legible payoff (that sector's traits, species, resources) rather than +1.
*Costs the least,* and is the only shape where existing work gets more
valuable rather than less: sector traits stop being decoration and become the
entire reason to pick a destination, and Portal Fragments keep their meaning
so `collapse_stack`'s renewability argument survives untouched.

Attacks the two words in the complaint directly: "interchangeable" is fixed
by sectors differing in trait *and* threat with the difference visible in
advance; "grind for no reason" is fixed by the `Clearance` rekey, which A and
B get too but pay for by discarding three built subsystems.

## Open questions at the pause

1. **Which shape** — A, B, C, or ship the `Clearance` rekey alone first and
   decide the world shape after playing it. Never answered.
2. **Access control.** The moment threat is a *choice* rather than a
   sequence, `balance_sim` loses its pacing guarantee — today it can assert
   "a player at zone N has roughly power P" because the sequence forces it.
   A level-3 party could walk into threat 5. Three candidate answers: leave
   it open and make the danger unmissable before committing (con colours
   already exist); soft-gate savage sectors behind `Clearance`, which
   re-introduces a mild ladder but keeps the guarantee; or price the trip by
   the threat gap rather than forbidding it. Never answered.
3. **Linearity.** `docs/seams.md:438` holds every difficulty curve to being
   linear as a *correctness* property, because `battle::compute_damage`'s
   subtractive floor means a geometric enemy curve racing the player's linear
   one has an end past which every swing lands on `MIN_DAMAGE`. So threat
   bands must be linear in **band index**, never in raw tile distance. Not a
   question so much as a constraint that must not be forgotten.

## Related existing todo

TODO.md item 12, "next zone unlocks are research -> upgrade base to zone 2",
is the same instinct arriving from the other direction — it also wants
advancement to be earned through a system rather than bought with fragments.
Whatever happens here should absorb or supersede it.
