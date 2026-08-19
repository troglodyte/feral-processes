# Companion progression: rings, levels past the cap, and talent trees

**Status:** Approved 2026-08-19. Not implemented. (Status headers in this
directory are written at approval time and go stale — see `INDEX.md`; answer
"did this ship" from `CHANGELOG.md`, never from here.)

## The problem

TODO #28, verbatim:

> companions also have perks, special unlocks and trees for entities, allow
> further level progression

The stated want, given in the brainstorm: **get the player invested in a
companion** — starting over with a fresh capture should be a hard choice, and
a developed program should have a reason to keep being developed.

Three symptoms were named: companions plateau, companions are
interchangeable, and the late-game roster is stale.

## What is not the problem

The obvious reading is that an old companion falls behind a fresh capture,
and that reading is wrong. It is worth writing down so nobody rebuilds a
solution that already exists.

A captured program takes `Experience::default()` — level 1 — but keeps the
`Stats` it spawned with, and those were scaled at capture time by
`Game::spawn_wild_creature_scaled`: `base × zone_mult × depth_mult ×
boss_mult × rarity_mult × potential`. So a fresh zone-5 capture does have
zone-5 bases *and* five unspent level-ups, where a program carried since zone
1 has zone-1 bases and has spent all six of its levels.

That gap already has an answer. **Recompile Kernel** (`assets/items/
recompile_kernel.ron`) is craftable from `annealed_core` ×3 at a Refactor
Bench, raises a program one zone tier through `game::refactor.rs`, and
refuses once the program has caught up with the player. It is renewable and
self-bounding. Catch-up is solved.

What is *not* solved is the two things actually asked for:

- **A maxed companion has nothing left to earn.** `CREATURE_MAX_LEVEL` is 6.
  Both species kit unlocks land at levels 2 and 4. `MAX_COMPANION_REFACTORS`
  is 5. After that the program is finished, permanently.
- **Nothing about it is the player's.** Every Scrapper unlocks
  `cascade_overflow` at 2 and `segfault_v2` at 4 — the kit table in
  `assets/species/README.md` is uniform by class and tier on purpose.
  `Potential` is the only divergence, and it is rolled at capture, never
  displayed, and never changes.

So the design target is not stats. It is **a ladder that keeps paying, and
choices no other program made.**

## Decisions taken, and why

### The power budget is a real increase, gated on a scarce item

Asked directly in the brainstorm; the answer was both "real increase, retune
zones" *and* "power, but scarce". Those compose rather than compete: the
increase is real and `balance_sim`'s curves are expected to move, and the
gate is a consumable the player had to go underground and fight for.

The alternative considered and rejected was specialisation at flat power — a
sidegrade menu. It was rejected because it does not produce investment: a
choice that costs nothing to unmake, and buys nothing a fresh capture
couldn't also buy, gives the player no reason to keep the program.

### The ring buys room; fights buy the points

The split is the whole of how this stays inside "progression is earned by
fighting". A Privilege Ring raises a companion's **ceiling** and pays nothing
by itself. Every level earned above the base cap pays **one Talent Point**,
and those levels come from `Game::award_party_xp` — combat and successful
decompiles.

`WORK_XP_LEVEL_CAP` (5) is untouched, so a posted cronjob still cannot grind
a program past level 5. Structure work remains steady low-effort income and
never a path to a developed companion.

### The gate is a boss drop, not a recipe

A Refactor Bench recipe would make rings renewable on demand, which is the
opposite of scarce. The drop rides the gate that already exists:
`Game::is_boss_creature` **and** underground — the same door that makes
Portal Fragments the only fight-and-descend currency. This widens that door
rather than opening a second one.

It stays *slowly* renewable, because `Game::collapse_stack` re-seeds a zone's
lairs on a new tile with an uncleared `FrameSpec`. That is the same bound
that already keeps a zone's fragment supply from stranding a run, and the
same argument applies here unchanged: a player who needs another ring can
always go and get one, but it costs a run.

Ring N costs N Privilege Rings, so the third is a real commitment rather than
a formality.

### The tree is data, and it can honestly be data

`Perk` is deliberately code — `crates/engine/src/perks.rs` documents that
every variant is a hook into a different formula (the mining roll, the hunger
multiplier, `capture_chance`'s HP term, a direct `Stats` write) with no shared
shape to express as data, which is why `PerkDef` has no `effect` field.

A companion talent is different, because a companion's mechanical surface is
small enough to close the vocabulary at four node kinds. That makes the tree
a real content directory under the moddability rule, not a half-data
catalogue.

### The tree is a branching ladder, not a DAG

Each tier offers two choices and one point takes one of them; tiers are taken
in order. That is the honest minimum that makes two Scrappers diverge and
still fits one screen. Arbitrary prerequisites are an additive field later if
it reads thin in play — YAGNI until then.

### Talent points are derived, never stored

`earned = level.saturating_sub(CREATURE_MAX_LEVEL)`, `spent = talents.len()`.
There is no points field on the save and nothing that can desync, the same
reason a work order stores an item and a quantity and nothing else.

## Data: `assets/talents/*.ron`

One file per `AffinityClass`, plus `generic.ron` for a species whose
`SpeciesDef::affinity_class` is `None` — that returns `Option` and `None`
means *no base job* rather than a default class, so the fallback file is
required, not optional.

`TalentDb::load_dir` follows `PerkDb::load_dir` exactly: a malformed file is
skipped with a logged warning, never a panic that crashes startup. Every
field added later is `#[serde(default)]` so third-party files keep parsing.

`TalentId` is a string newtype like `ItemId`, so a mod adds a node by
dropping in a file.

### Node kinds

Four, and the width is deliberate — each one has an existing definition it
must *call* rather than copy.

| Node | Effect | Must share |
|---|---|---|
| `Stat(kind, percent)` | raises one of hp/atk/def by a percentage | `game::refactor::raised` — the per-step floor is load-bearing and a second copy of that arithmetic is the drift this repo has already been bitten by four times |
| `Affinity(kind, mult)` | multiplies the species' affinity for one `AffinityKind` | `AffinityClass::of_axis` |
| `Ability(id)` | grants a routine outright | the species-kit unlock path, so a granted routine behaves exactly like an unlocked one |
| `RoutineSlot` | +1 routine slot | `abilities::companion_routine_slots` |

`raised` is currently a private `fn` in `game/refactor.rs` and becomes
`pub(crate)` so the talent path can call it rather than restate it. Its floor
— never gaining less than a whole point — is why `Stat` must call it: `+5%` of a Drone's 3 ATK rounds straight back to 3, so without the
floor a percentage node would do nothing to exactly the weak programs a tree
exists to rescue, while charging them a permanent tier for the privilege.

### Authoring guidance

Author the `Stat` percentages **small** and weight each tree toward
`Ability` / `Affinity` / `RoutineSlot`. Two reasons, and the second is the
one that matters: a developed companion carries four multiplicative axes at
once — Recompile Kernel tiers, five refactor slots (~1.28× on power), ring
levels, and talents — and options compound far less dangerously than numbers
do. Weighting away from `Stat` also keeps more of the tree inside
`balance_sim`'s reach; see Testing.

## Engine changes

### Components

Two, both following the precedent set by `Refactors`, `FusionCount` and
`PurchasedTiers` that **absent means zero**:

- `KernelRing(u32)` — rings opened on this program.
- `Talents(Vec<TalentId>)` — nodes taken, in the order taken.

Because both default, neither joins `Game::roster_parts()`. That barrier
exists because four hand-written component tuples can each silently omit
something; a component whose absence is indistinguishable from its zero value
has nothing to omit.

### The cap

`progression::add_xp` already takes `level_cap: Option<u32>`, so the
mechanism needs no new parameter. What it needs is a definition of the
ceiling:

    CREATURE_MAX_LEVEL + ring * LEVELS_PER_RING

That belongs in **one function** — proposed `Game::companion_level_cap(entity)`
— because `CREATURE_MAX_LEVEL` is read as a ceiling at **four** production
sites, not one, and a per-entity value copied into four places is a seam
waiting to drift:

| Site | Today | After |
|---|---|---|
| `game/combat_rewards.rs:685` (`award_party_xp`) | `Some(CREATURE_MAX_LEVEL)` | the per-entity cap |
| `systems.rs:676` (cronjob work XP) | `Some(CREATURE_MAX_LEVEL)`, behind its own `exp.level < WORK_XP_LEVEL_CAP` guard | unchanged — see below |
| `arena/mod.rs:75` (staged companion growth) | `Some(CREATURE_MAX_LEVEL)` | the absolute maximum |
| `app-core/src/app/arena.rs:578` (arena level stepper clamp) | `CREATURE_MAX_LEVEL` | the absolute maximum |

The player keeps passing `None` and stays uncapped.

**The cronjob site does not change.** Its `WORK_XP_LEVEL_CAP` guard (5) sits
ahead of the ceiling and already stops it long before 6, so structure work
still cannot grind a developed program — which is the whole of how the ring
stays inside "progression is earned by fighting".

**The two arena sites take the absolute maximum**, `CREATURE_MAX_LEVEL +
KERNEL_RING_MAX * LEVELS_PER_RING`, rather than a per-entity value. An arena
scenario authors its own composition and has no `KernelRing` to read; letting
the stepper reach the true ceiling is what makes a staged fight able to field
a fully developed companion. That matters more here than usual: `Ability`,
`Affinity` and `RoutineSlot` nodes are invisible to `balance_sim`, so the
arena is the only instrument that can see them, and an arena clamped at 6
could not stage the fight the tree exists to change.

### New tuning constants

In `crates/engine/src/tuning.rs`, grouped and documented like everything else
there. Difficulty is code, not data.

- `KERNEL_RING_MAX` — proposed 3.
- `LEVELS_PER_RING` — proposed 2, giving a ceiling of 12 and six points.

Both are proposals for `balance_sim` to argue with, not settled numbers.

### Talents bake into `Stats`, and the list is the receipt

A `Stat` node is applied at purchase, exactly as a refactor is. `CreatureSave`
already writes `hp/max_hp/atk/def` directly, so a saved program's stats
already carry its talents.

**Load must not re-apply them.** This is the same rule refactors follow
today, and it is stated here because re-applying on load is the obvious
"fix" for a bug that does not exist, and would compound a program's stats on
every reload.

## Save format

`CreatureSave` gains two fields, both `#[serde(default)]`:

- `ring: u32`
- `talents: Vec<String>`

The save is field-named RON and this is purely additive, so it earns **no
`SAVE_FORMAT_VERSION` bump**. Nothing is removed and no existing field
changes meaning under a name it keeps.

A RON round-trip test cannot catch a field that fails to travel — that is
what `#[serde(skip)]` looks like from the round trip's side — so this needs
its own save → load → assert test, on both fields.

## Fusion is the trap

`Game::fuse_companions` is one of the four doors into the roster and the one
that assembles its **own** component list rather than going through
`roster_parts`. It already does its own `retain`/`despawn` and skips the
detachment logging that `dissolve_tamed_program` performs.

**Decision:** the surviving program keeps its own ring and talents; the
consumed program's are lost.

This needs an explicit test. Fusion is precisely the door where a new
component is silently dropped, and the symptom — a fused companion that has
lost its development — reads as "fusion is bad" rather than as a bug. The
repo has been bitten by this exact shape before, when three tests failed on
`spawn_tamed` rather than on the feature under test.

Note also that `fuse_companions` strips gear **before** its stats snapshot,
because no stats operation may run while a gear bonus is sitting in `Stats`.
A talent applied during fusion would inherit that constraint; the decision
above avoids the question entirely by never applying one there.

## Screen and flow

One new entry in `PARTY_ROWS` (`crates/app-core/src/app/group_menu.rs`):

- label **"Develop a program"**
- `surface_only: false` — like the Refactor row, this reaches no zone-map
  state through `Position`, so it works four frames down. The `surface_only`
  flag is a column in that table rather than a check inside each predicate
  precisely so it stays in step with `require_surface`'s caller list.
- `available`: any owned pet.

Two modes: pick the companion, then **one** screen carrying both verbs —
current ring, unspent points, the tier ladder, and *open the next ring* when
a Privilege Ring is held. One page rather than two, because opening a ring
and spending a point are the same decision loop and splitting them would make
the player back out to see what they just bought.

New `crates/gui/src/render/talents.rs`, drawing through `Painter` only. No
file in `render/` may name a graphics library.

The manifest gains a ring/talents section. `manifest_layout`'s fixtures must
match `sections_for`'s **emission order**, not merely its row count — a
drifted fixture has already hidden a live overflow behind a green suite once.

## Testing

### `balance_sim` will move, and that is the signal

The sweep fields a mid-grade party of three Scrappers. Extra companion levels
and any `Stat` talent move party strength directly, so **every zone
clearability curve is expected to move**. A curve that moves means
progression changed — read the direction and the magnitude; do not "fix" the
test.

### What `balance_sim` cannot see

It models **no abilities at all**. So `Ability`, `Affinity` and `RoutineSlot`
nodes ship with no regression gate whatsoever — the instruments for those are
`dev-arenas/` and a played session, the same position the Power economy's 66
costs are in. This is a known and accepted gap, not an oversight, and it is
the second reason the authoring guidance above matters.

### Asset censuses

In `crates/engine/src/tests/assets.rs`, over the real assets:

- every `AffinityClass` has a tree file, and `generic.ron` exists;
- every `Ability` node's id resolves in `AbilityDb`;
- every tree has exactly `KERNEL_RING_MAX * LEVELS_PER_RING` tiers, each
  with two choices;
- every `Stat` node's percentage is under a ceiling constant.

### Engine tests

- A ring raises the cap and `add_xp` pays past 6; without a ring it does not.
- A cronjob worker still stops at `WORK_XP_LEVEL_CAP` regardless of ring.
- Points earned equal levels above the base cap; spending decrements the
  unspent count and cannot go negative.
- A `Stat` node applies once, and survives a save → load without re-applying.
- Fusion keeps the survivor's ring and talents and drops the consumed one's.
- A Privilege Ring drops from an underground boss and not from a surface one.
- The arena's level stepper reaches `CREATURE_MAX_LEVEL + KERNEL_RING_MAX *
  LEVELS_PER_RING`, so a scenario can stage a fully developed companion.

Every test above must fail with its fix removed. Two tests written on
2026-08-09 in this repo passed against no fix at all and read as coverage.

## Documentation obligations

- `assets/talents/README.md` — the schema reference, in the same change. The
  moddability rule requires it whenever a field is added, removed, or changes
  meaning.
- `assets/items/README.md` — the Privilege Ring's shape if it needs one.
- `CHANGELOG.md` section and a workspace version bump at merge, not on the
  branch.
- A `docs/seams.md` entry plus its one-or-two-line summary in `CLAUDE.md`,
  for: the ring/points split, the derived-not-stored points rule, the
  bake-once-never-on-load rule, and fusion's decision.
- `docs/manual.md` and the root `README.md` are carved out and stay stale.

## Open questions

1. **Do the numbers hold?** `KERNEL_RING_MAX = 3` and `LEVELS_PER_RING = 2`
   are proposals. `balance_sim` decides, and the first implementation step
   after the mechanism works should be running it.
2. **Does a talented program sell for too much?** Talents raise
   `Stats::power()`, and `Game::program_payout` pays a fraction of it.
   `PurchasedTiers` exists because buying tiers and selling the program
   printed Credits — measured at 716 against 72 fragments' worth of kernels.
   Rings are boss drops rather than a renewable material chain, so this is
   not the same printing press, and the recommendation is to follow the
   refactor precedent and *not* divide talents back out. It should be a
   stated decision rather than an oversight, and it is worth a measurement
   once the tree is authored.
3. **Does a six-tier ladder read as a tree?** If it reads thin, prerequisites
   are an additive field. Decide from play, not from the spec.
