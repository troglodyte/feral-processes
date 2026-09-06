# Town-sourced raids and hostile patrols

The angry end of the settlement ladder. Both were named as deferred by
`2026-09-04-settlements-design.md` ("Hostility (later) | Town-sourced raids;
hostile patrols. **Structure the code for these now.**") and again by
`2026-09-05-settlement-aid-design.md` ("each is another named query on the same
exhaustive match — this spec does not block them and does not build them").

This builds them. **Two branches, two releases** — Phase 7a is raids, Phase 7b
is patrols, and each lands playable on its own.

## Context

The settlement ladder currently runs from `Hostile` to `Allied` and only the
friendly half has weight. `Warm` and `Allied` buy a garrison, a program gift, a
relay and a standing route. `Hostile` buys two refusals — `refuses_service`
closes the market and the board, `preys_on_routes` taxes a caravan — and
nothing that comes *at* the player. A band you can reach (as of `92f870f4`) and
that then does nothing to you is a threat in name.

The other half of this is that raids already exist and have no author. `run_raid`
writes "a GC Entropy Sweep", an ambient weather event with nobody behind it. A
town that hates you is the first thing in the game that could be behind one.

## Decisions taken

Settled in brainstorming on 2026-09-06; recorded so they are not relitigated.

| Question | Decision |
|---|---|
| What a town raid is | A **distinct event with its own rules**, not extra pressure on the sweep. |
| What it takes | Banked **build currency** from the base's stores, not structure durability. |
| What turns it away | `Game::total_raid_defense` — the same number the sweep already reads. |
| Zone / staff gates | **Neither applies.** Being Hostile with a neighbour is earned, not depth-scaled, and nobody has to be on shift for a store to be robbed. |
| What a patrol is | **Tethered pursuers**, the nest-aggro shape — they hold their town's ground and chase you off it. |
| The tether | A **second component**, `TownPatrol { town }` beside `NestGuardian { nest }`. Not a unified `Tethered`. |
| What provokes a patrol | **Proximity**, not an attack on the anchor. |
| Killing a patrol member | **−1 standing**, charged to that town alone. |
| Patrol species | The habitat pool at the town's tile. **No new asset data.** |
| Frontend reach | Engine + town-page lines + a map mark. |

Deferred by decision: patrols that leave their town's ground to intercept a
caravan or besiege the base (route predation already exists and is `Phase 6`'s;
a patrol that walks to your base is a third mechanic, not this one).

## What already exists

Surveyed 2026-09-06 against the source. **Do not re-derive this.** Verify a
line before relying on it; do not re-run the survey.

**The ladder.** `Standing` (`crates/engine/src/settlements/relations.rs:71`) is
five bands with six named queries on it — `job_slots`, `garrison_defense`,
`gifts_programs`, `hosts_a_relay`, `refuses_service`, `preys_on_routes`,
`allows_standing_route`. Every one is an **exhaustive match** (a sixth band
fails to compile) with a census test that walks the list to prove each answer
is reachable rather than merely written. `refuses_service`'s doc comment is the
one every other query's cites. This is the extension shape both features use.

**The one writer.** `Game::adjust_standing`
(`crates/engine/src/game/settlement_relations.rs:39`) holds the only clamp and
speaks only on a band transition. `credit_nearby_settlements` (`:88`) moves
**every** known town within `SETTLEMENT_NOTICE_RADIUS` of a tile — that is the
nest/Stack mover and it is *not* the shape the patrol-kill penalty wants, which
goes to one town by key.

**Raids.** `Game::raid_check`
(`crates/engine/src/game/base/upkeep.rs:381`) rolls `RAID_CHANCE_PER_TICK`,
then gates on `RAID_MIN_ZONE` and `RAID_MIN_BASE_STAFF` *after* the roll so a
miss leaves the RNG stream untouched. `run_raid` (`:440`) picks a random
`(With<Durability>, With<Structure>)` target and applies
`RAID_DAMAGE.saturating_sub(self.total_raid_defense())`, and its zero case is a
shipped outcome with its own line and `EffectKind::Deflected`.
`total_raid_defense` (`:230`) is the structure sum plus `garrison_defense`
(`:258`), which folds `Standing::garrison_defense` over every known town within
`SETTLEMENT_GARRISON_RADIUS` of the anchor and clamps that half alone to
`SETTLEMENT_GARRISON_MAX`. `tick_inner` calls `raid_check` at
`crates/engine/src/game/turn.rs:236`.

**A compile-time invariant to mirror.** `relations.rs:86` asserts
`SETTLEMENT_GARRISON_MAX < RAID_DAMAGE` as a `const _`, not a test, so that
"a garrison alone can never zero a sweep" fails the *build* if retuned away.

**Nest aggression — the whole patrol machine, already written.**
`NestGuardian { nest: Entity }` (`crates/engine/src/components.rs:2267`) and
`Pursuing` (`:2279`). `Game::nest_aggro_tick` (`crates/engine/src/game/turn.rs:325`)
leashes first, builds **one** `pursuit_field` centred on the player at
`NEST_AGGRO_LEASH_RADIUS + NEST_PATH_SEARCH_MARGIN`, checks adjacency *before*
every step, drops any pursuer absent from the field, steps
`NEST_PURSUIT_STEPS_PER_TICK`, and calls `gather_pack` + `start_battle` for the
first to arrive. It is a no-op during battle, after game over, underground, or
in base space — and the doc comment explains why the last two are two questions
rather than one. `systems::wander_ai_system` excludes anything `Pursuing` and
reads `Option<&NestGuardian>` for its tether radius (`systems.rs:149`).

**Where the tether is already load-bearing:** `zone.rs:214` (despawn strips it),
`:236` (provoke), `:255` (`nest_has_pursuers`), `combat_round.rs:807` and
`combat_rewards.rs:1119` (victim's nest on kill/tame; `:1122` strips
`(Hostile, WanderAi, NestGuardian, Pursuing)` on a tame),
`combat_teardown.rs:97`/`:409`, `lifecycle.rs:1196` (spawn) and `:1643`
(save/load resolves by **position**, not entity id).

**Saves.** `CreatureSave.nest_position: Option<(i32, i32)>`
(`crates/engine/src/save.rs:464`) and `.pursuing: bool` (`:470`) — the
precedent, and its doc states the reason: entity ids are not stable across a
round trip, one nest per tile so a position key is unambiguous.

**Spawning.** `spawning::ring_tiles(from, min_band, max_band)`
(`crates/engine/src/game/spawning.rs:1617`) walks square rings nearest-first;
it was extracted precisely so band-0 and band-1 searches share one definition
of "nearest". `Game::pick_habitat_species(x, y, depth, allow_boss)` (`:1255`)
answers what a spawn at a tile should field, split out of
`try_spawn_habitat_creature` so a caller can reuse the biome and opening-ring
rules **without** copying them. `spawn_group` (`:1503`) places a pack.
A settlement is an entity with a `Position` (`spawn_settlement_at`, `:1020`)
and its **tile admits nobody** — the fourth arm of `move_player`'s ladder.

**The map channels.** `EntityView` (`crates/engine/src/views.rs:749`) already
carries three separate readings — authored `color` (what it is), `difficulty`
(how dangerous, drawn as a bottom bar), `rarity` (a top bar) — plus
`nemesis: bool` (`:819`), which is the precedent for "a boolean the map draws
as identity". Its doc says identity rides corner marks.

**The town page.** `Game::settlement_aid_lines`
(`crates/engine/src/game/settlement_relations.rs:477`) writes the friendly
lines; `AID_LINES` (`:531`) is the census both the engine and the gui layout
gates read, because **the page has no scroll** and a gui test cannot build a
`Game` to ask. A line written and missing from the census is a line nothing
measures, and `draw_row` clips vertically but never horizontally.

**Inventory.** `Inventory::take(item, qty) -> u32` (`components.rs:607`)
returns how many were actually removed. `Game::currency()`
(`crates/engine/src/game/catalog.rs:659`) is the **build** currency;
`trade_currency()` (`:686`) is what traders deal in and is a different thing —
see the four economy roles.

---

# Phase 7a — Town-sourced raids

## 1. The query

```rust
// crates/engine/src/settlements/relations.rs, on Standing
pub fn sends_raiders(self) -> bool
```

`Hostile` alone; every other band `false`. Exhaustive, `refuses_service`'s
reason, with its own census walk beside the existing ones.

`preys_on_routes` is the sibling to copy rather than `garrison_defense`: this
is a bottom-of-the-ladder consequence with no ramp, and a magnitude here would
be a second spelling of "is this town Hostile" — the *frequency* and the *haul*
are tuning constants, not band answers, because no middle band ever sends a
raider at all.

## 2. The check

```rust
// crates/engine/src/game/base/upkeep.rs
pub(crate) fn town_raid_check(&mut self)
```

Called from `tick_inner` immediately after `raid_check`, so the two raid
sources resolve in a fixed order and a tick that produces both reads as two
events rather than an interleaving.

**Roll first, gate after** — `maybe_spawn_wild_creature`'s and `raid_check`'s
shared discipline, and the reason is the same: a miss must leave the RNG
stream untouched, so a world with no angry neighbour costs exactly one draw
per tick and the seeded spawn tests are unmoved.

1. Roll `SETTLEMENT_RAID_CHANCE_PER_TICK`. Miss ⇒ return.
2. Collect every known town whose band `sends_raiders()` and whose resolved
   tile is within `SETTLEMENT_RAID_RADIUS` (Chebyshev) of the anchor. Empty
   ⇒ return.
3. One draw picks which town, when several qualify. The town's key is carried
   through everything below — this event has an author, which is the whole
   point of it not being a sweep.

**Neither `RAID_MIN_ZONE` nor `RAID_MIN_BASE_STAFF` applies**, and both
omissions are deliberate. `RAID_MIN_ZONE` exists so a player who has not
engaged the game is not swept; reaching `Hostile` with a town near the anchor
*is* engagement, and gating it on depth would make the consequence of a choice
wait on an unrelated axis. `RAID_MIN_BASE_STAFF` protects a base already
reduced to wreckage from being ground down further — but this raid does not
break anything, so there is no attrition spiral for it to prevent, and a base
with no staff is exactly the one whose stores are easiest to walk off with.

**A town whose tile has never been resolved does not raid**, the same rule
`town_garrisons` follows: `Settlements` records the tile once a town is
*found*, so hostility, like aid, follows discovery. A place the party has
never met does not know where they live.

## 3. What it takes

```rust
fn run_town_raid(&mut self, key: SettlementKey)
```

The haul is a share of the party's banked **build currency**
(`Game::currency()`), taken through `Inventory::take`:

```
percent = SETTLEMENT_RAID_HAUL_PERCENT
            .saturating_sub(total_raid_defense() * SETTLEMENT_RAID_DEFENSE_PER_POINT)
if percent == 0 { deflect; return }          // §4's outcome, before any bound
haul    = (banked * percent / 100).max(SETTLEMENT_RAID_HAUL_FLOOR)
            .min(SETTLEMENT_RAID_HAUL_CAP)
```

**The zero check comes before the floor, and the order is load-bearing.** The
floor exists so a percentage of a *small bank* does not round to nothing; if it
ran after a defense that had driven the share to zero it would hand the raiders
one unit anyway and delete the deflect outcome — the shield network would stop
working and the log would still say it had. Two different zeroes, and only one
of them is the floor's business.

Three bounds, each closing a different failure:

- **`SETTLEMENT_RAID_HAUL_FLOOR`** because a percentage of a small bank rounds
  to zero, and a raid that takes nothing while still logging is the mechanic
  deleted rather than softened — the same failure `SETTLEMENT_GARRISON_MAX`
  exists to prevent on the friendly side.
- **`SETTLEMENT_RAID_HAUL_CAP`** because a percentage of a large bank scales
  without limit; a rich base should be bled, not gutted, and an uncapped share
  makes banking itself the punished behaviour.
- **`Inventory::take`'s own return** is the third: it takes what is there and
  says how much, so an empty store is a raid that finds nothing and says so,
  not a negative.

**Build currency and not Credits.** The four economy roles are separate on
purpose and only Credits cross a breach. Raiders hitting the *base's* stores
take what the *base* runs on — it costs construction, which is a real setback
that does not touch progression, and progression is earned by fighting.

## 4. What turns it away

`Game::total_raid_defense()` — the number the sweep already reads, called
once. Each point cuts `SETTLEMENT_RAID_DEFENSE_PER_POINT` percentage points
off the share. Enough defense drives the percent to zero and the raid is
**turned away**, with `EffectKind::Deflected` at the anchor and its own line —
the outcome `run_raid` already ships for a shield network.

Because `total_raid_defense` folds the garrison in, this is where the two
settlement halves meet on one number: **hostile neighbours raid your stores
and allied neighbours are what stands in the way.** That is the feature.

**Mirror the friendly side's compile-time invariant**, in the same file and
the same form:

```rust
const _: () = assert!(
    SETTLEMENT_GARRISON_MAX * SETTLEMENT_RAID_DEFENSE_PER_POINT
        < SETTLEMENT_RAID_HAUL_PERCENT
);
```

A garrison **alone** can never zero a town raid, however many Allied
neighbours the party collects — the same claim `relations.rs:86` makes about
the sweep, and for the same reason. Structures may still zero it, because that
is a thing the player built.

## 5. The line and the notification

`log_base_kind(MessageKind::Raid, …)`, and every line **names the town** via
`Game::settlement_name` — which falls back to a generic rather than refusing
to speak, since a key with no materialized town is reachable. Three outcomes,
three lines: hauled, turned away, found nothing.

`MessageKind::Raid` rather than a new kind: the retention rules
(`retain_outcomes_since_battle`) already treat a raid as base news that
survives a battle, and a town raid is a raid.

A new `NotificationKind::FirstTownRaid` on `FirstRaid`'s precedent —
`Repeat::OnceEver`, its own def and its own tutorial key. It is a different
lesson from the sweep ("something you did caused this") and firing `FirstRaid`
again would say the wrong one. The notification census is exhaustive; a
variant with no def fails it.

## 6. The town page

Two Hostile lines join `settlement_aid_lines` and the `AID_LINES` census —
one for raiding, one for whether *this* town is near enough to the anchor to
do it, since the radius is a per-run coin flip and a page that says "raiders
come from here" about a town forty regions away is a lie.

The census is not optional bookkeeping: the page has no scroll, the gui width
gate measures the worst case from `AID_LINES`, and a line missing from it is
drawn past the right edge with nothing to catch it.

## 7. Tuning

Every figure here is a guess, and the "Open, deliberately" section says so.

| Constant | Value | Why that shape |
|---|---|---|
| `SETTLEMENT_RAID_CHANCE_PER_TICK` | `0.006` | Half `RAID_CHANCE_PER_TICK`. A Hostile neighbour raises total raid pressure by half again, not double. |
| `SETTLEMENT_RAID_RADIUS` | `REGION_TILES / 2` | Deliberately equal to `SETTLEMENT_GARRISON_RADIUS` **today**, but its own constant: retuning the friendly reach must not silently retune the hostile one. Same per-run coin flip, and the same 39% figure the aid measurement recorded. |
| `SETTLEMENT_RAID_HAUL_PERCENT` | `10` | A tenth of the store. |
| `SETTLEMENT_RAID_DEFENSE_PER_POINT` | `2` | Max garrison (3) cuts 6 of 10 — real relief, never immunity. Two Shields plus a garrison zeroes it. |
| `SETTLEMENT_RAID_HAUL_FLOOR` | `1` | See §3. |
| `SETTLEMENT_RAID_HAUL_CAP` | `40` | See §3. |

## 8. Tests

Per rule, and each one written so that deleting the rule fails it:

- The census walk for `sends_raiders`, beside the existing five.
- The compile-time assert (it is a `const _`; its failure is a build failure).
- Roll-miss leaves the RNG stream untouched — the seam every raid gate
  already respects, asserted by drawing after a missed tick.
- A Neutral neighbour never raids; a Hostile one out of radius never raids;
  a Hostile one with an unresolved tile never raids. **Three tests, not one**
  — a single test over one path passes against every path that was never
  going to fire anyway.
- The haul lands, is capped, is floored, and is refused when the store is
  empty. Four assertions, per-outcome.
- Defense reduces the haul; enough defense deflects it; a **garrison alone
  never** deflects it (the runtime half of the compile-time claim, so the
  invariant is stated where a reader of either file finds it).
- The line names the town.

---

# Phase 7b — Hostile patrols

## 1. The query

```rust
// crates/engine/src/settlements/relations.rs, on Standing
pub fn fields_patrols(self) -> bool
```

`Hostile` alone. Exhaustive, its own census walk. `sends_raiders`' sibling and
`preys_on_routes`' shape, for the same reason: no band in the middle fields
half a patrol.

## 2. The tether — the design-patterns dialog, recorded

**The axis of change:** what an armed pursuer is tethered to. Today a nest;
with this, a town. Two implementors, both real, both in this change — that is
the trigger, not speculation about a third.

**The direct version, and what ships:** a second component

```rust
#[derive(Component, Clone, Copy, Debug)]
pub struct TownPatrol { pub town: Entity }
```

beside `NestGuardian`. `nest_aggro_tick` grows a **two-query collection step**
that builds one `Vec<(pursuer, anchor, leash)>`; everything below it — the
leash filter, the single `pursuit_field`, the adjacency check, the step loop,
`gather_pack`, `start_battle` — is unchanged and still written once. The
function is renamed to say what it now does (`pursuit_tick`), and
`wander_ai_system`'s tether radius reads `Option<&TownPatrol>` alongside
`Option<&NestGuardian>`.

**The alternative that was rejected:** unifying into
`Tethered { anchor, kind }`. It buys one query and one save field. It costs a
save-format field whose *meaning* is rewritten rather than added, edits across
~19 files, and a `kind` match at every nest-specific site anyway —
`despawn_nest` strips tethers, `nest_has_pursuers` asks a nest question,
`combat_rewards` and `combat_round` look up the victim's nest. The branching
moves; it does not leave. And the two differ in every respect except the tick:
what provokes them, what frees them, what a save keys them by, and what
happens when the anchor dies — **a town cannot be destroyed**, so a patrol's
leash never resolves to nothing, which is a case `nest_aggro_tick`'s belt-and-
braces check exists for and a patrol does not need.

**The coupling this creates, written down because it is the trap:** the shared
tick builds **one** field, sized `NEST_AGGRO_LEASH_RADIUS + NEST_PATH_SEARCH_MARGIN`.
With two leashes it must be sized off the **maximum** of them, or raising
`SETTLEMENT_PATROL_LEASH_RADIUS` past the nest's silently produces patrols that
read as absent from the field and give up on the spot — a mechanic that
disappears with no error. The two constants are equal today; the `max` is what
keeps that a coincidence rather than a load-bearing one.

## 3. Spawning

```rust
fn maybe_field_patrol(&mut self)
```

Roll first, gate after, once per tick. The gates: the party is on the surface,
some known town within `SETTLEMENT_PATROL_RANGE` of the party
`fields_patrols()`, and that town's living patrol is under
`SETTLEMENT_PATROL_SIZE`.

Members are placed on `ring_tiles(town_tile, SETTLEMENT_PATROL_RING_MIN,
SETTLEMENT_PATROL_RING_MAX)` — **band 0 is excluded because a settlement tile
admits nobody**, which is `move_player`'s fourth arm and the same reason a
relay landing searches from band 1. Species come from `pick_habitat_species`
at the chosen tile, `allow_boss: false` — a patrol is an ordinary-encounter
mechanic and a boss standing in one is a different fight. **No new asset
data**: a patrol reads as the town's from the map mark and its label, not from
a species nobody else fields.

Members spawn `Hostile` + `WanderAi` + `TownPatrol { town }`, exactly what a
nest guardian is minus the nest.

## 4. Provocation

Proximity, not an attack — the one behavioural difference from a nest, and the
reason it needs its own step rather than reusing `provoke_nest`. A patrol
member whose town `fields_patrols()` and who is within
`SETTLEMENT_PATROL_AGGRO_RADIUS` (Chebyshev) of the player gains `Pursuing`.

`SETTLEMENT_PATROL_AGGRO_RADIUS` sits **inside `EXAMINE_RANGE_TILES`** on
purpose: the player can see and identify a patrol before it notices them.
A threat that is only ever discovered by being in a fight is not a threat the
player can play around.

## 5. Standing down

The band is re-read every tick, so **the moment a town stops being Hostile its
patrol stands down**: `Pursuing` is dropped, `TownPatrol` is stripped, and the
members revert to ordinary untethered wildlife — the same disposition
`NestGuardian`'s doc describes when a nest is destroyed.

This is the way back out of `Hostile` made visible, and it is the reason the
band re-read is per-tick rather than cached at spawn. A player who repairs
standing must *see* the consequence lift.

## 6. Killing a patrol member

`Game::adjust_standing(town, SETTLEMENT_PATROL_KILL_STANDING)` — **that town
alone, by key**, not `credit_nearby_settlements`. Killing a town's guards is
news to that town; the neighbours have no view on it, and routing it through
the radius mover would spread a penalty the player cannot see the source of.

Fires from the kill path where `combat_rewards` already reads the victim's
`NestGuardian` (`:1119`) — the same place, one component over.

**`SETTLEMENT_PATROL_KILL_STANDING = -1`, and the magnitude is the whole
safety argument.** The movers pay `+10` a contract, `+8` a Stack collapse,
`+4` a nest cleared, `+1` per `SETTLEMENT_TRADE_CREDITS_PER_POINT` traded. A
full `SETTLEMENT_PATROL_SIZE` patrol wiped costs `-3` — less than clearing one
nest on that town's doorstep. That inequality is what keeps the ladder out of
`Hostile` climbable while patrols are actively in the way, and it is the risk
this decision was taken with open eyes about: at `-4` or worse, self-defence
outruns every mover that is still available to a Hostile player and the band
becomes a trap. **A test asserts the inequality directly** against the mover
constants, so a retune of either side fails the suite rather than quietly
sealing the exit.

A tamed patrol member costs nothing: `combat_rewards:1122` already strips
`(Hostile, WanderAi, NestGuardian, Pursuing)` on a tame, and `TownPatrol`
joins that tuple. Taking a program into the roster is not killing it.

## 7. Saves

```rust
#[serde(default)]
pub patrol_position: Option<(i32, i32)>,
```

on `CreatureSave`, resolved by the **town's tile**, not its entity id — the
reason `nest_position` gives, and one settlement per tile makes the key
unambiguous. Additive behind a default, so no `SAVE_FORMAT_VERSION` bump.

`pursuing: bool` is reused as-is; its doc says "meaningless unless
`nest_position` is also `Some`" and becomes "unless one of the two tethers is".

**A RON round trip cannot catch a skipped field** — this needs a real
save→load test that spawns a patrol, saves, loads, and asserts the member is
still tethered to its town and still pursuing.

## 8. The map mark

`EntityView.patrol: Option<String>` — the town's name, `nemesis: bool`'s
precedent, drawn as a corner mark. `Some` names the owner so the examine label
can say whose it is; a boolean would draw the mark without answering the
question the mark raises.

It is a **fourth channel and does not touch the other three**: authored colour
still says what the program is, the difficulty bar still says how dangerous,
the rarity bar still says how rare. The seam the HUD reference states is that
these are separate readings and collapsing two of them into one hue is the
regression — a patrol mark must not spend the identity hue.

## 9. Tuning

| Constant | Value | Why that shape |
|---|---|---|
| `SETTLEMENT_PATROL_SIZE` | `3` | Between `NEST_GUARDIAN_MIN` (2) and `NEST_GUARDIAN_MAX` (5). |
| `SETTLEMENT_PATROL_RING_MIN` / `_MAX` | `2` / `6` | Off the town's own tile, close enough to read as its ground. |
| `SETTLEMENT_PATROL_LEASH_RADIUS` | `15` | Equal to `NEST_AGGRO_LEASH_RADIUS` today; see §2's `max` coupling. |
| `SETTLEMENT_PATROL_AGGRO_RADIUS` | `8` | Inside `EXAMINE_RANGE_TILES` (12), so you see them first. |
| `SETTLEMENT_PATROL_RANGE` | `REGION_TILES / 2` | How near the party must be for a town to bother fielding one. |
| `SETTLEMENT_PATROL_RESPAWN_TICKS` | `60` | Far slower than `NEST_RESPAWN_TICKS` (10) — wiping a patrol should buy real time. |
| `SETTLEMENT_PATROL_KILL_STANDING` | `-1` | See §6. This one is not a free retune. |

## 10. Tests

- The census walk for `fields_patrols`.
- The mover inequality of §6, asserted against `SETTLEMENT_NEST_CLEARED_STANDING`.
- A patrol spawns only for a Hostile town, only in range, only up to
  `SETTLEMENT_PATROL_SIZE`, and never on the settlement's own tile. Four
  tests — see Phase 7a §8's reason for not folding them into one.
- Proximity provokes; distance does not; the leash releases.
- **A patrol and a nest guardian pursue in the same tick** — the shared field,
  and the test that would fail if the two-arm collection dropped an arm.
- Repairing standing out of `Hostile` stands the patrol down.
- A killed member moves that town's standing and **no other town's**.
- A tamed member moves nobody's.
- Save → load → still tethered, still pursuing.

---

## Open, deliberately

- **Every constant in both phases is a guess no instrument in this repo can
  check.** `balance_sim` models no raids, no towns and no loot; the aid
  spec recorded the same thing about its seven figures. Thirteen more here.
  Is a haul of 40 a setback or an inconvenience? Does a three-member patrol
  read as a threat or as scenery? Both are answerable only at the keyboard.
- **Whether the two radii should ever diverge.** `SETTLEMENT_RAID_RADIUS` and
  `SETTLEMENT_GARRISON_RADIUS` are the same expression today, and the aid
  measurement (`docs/measurements/2026-09-05-settlement-aid-reach.md`) says
  that reaches 39% of worlds. If hostility should be more common than aid —
  a defensible design position — it is a one-constant change and this spec
  does not take it.
- **Whether a patrol should ever leave its town's ground.** Deferred above.
  Route predation already exists; a patrol that marches on the base is a
  third mechanic and would want its own brainstorm.
- **Nothing here will have been played.** The settlements path has landed six
  phases and a green suite is not evidence of play.

## Before implementation

Per phase, in order:

1. Branch. One branch per phase, per the repo's release-per-change rule.
2. **Read the `seams` reference files for what that phase touches** — this
   spec deliberately did not read them at design time, and both phases touch
   more than one. Phase 7a: `references/base.md` (raids, upkeep) and
   `references/screens.md` (the log, the town page) and
   `references/notifications.md`. Phase 7b: `references/combat.md` (spawning,
   teardown, kill rewards), `references/screens.md` (saves) and
   `references/hud.md` (the fourth channel).
3. TDD, failing test first.
4. Version bump, `CHANGELOG.md` section, annotated tag at the merge.
