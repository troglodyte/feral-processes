# Settlement aid: what a friendly town is worth

**Status:** built on `feat/settlement-aid`, unmerged and **unplayed**. See
"Amended in the build" below for the four places the design did not survive
contact.

The follow-on to `2026-09-04-settlements-design`, whose six numbered phases
all shipped by `v0.13.99`. It builds the item that spec deferred by
decision — *settlement "aid" rewards (garrisons, gifted programs, relay
fast-travel)* — and closes an asymmetry the ladder shipped with.

**The asymmetry.** `Standing` has five bands and four consequences, and every
one of them is a penalty or a gate. `Hostile` refuses service and preys on
routes; `Warm` unlocks standing routes. `Allied` differs from `Warm` in one
number, `job_slots`. The top of the ladder is a plateau, so the climb past
Warm buys almost nothing and the relationship stops being worth working.

## Decisions taken

Recorded so they are not relitigated. Each was chosen against a named
alternative.

1. **All three aid forms ship together.** Rejected: garrison alone. One
   consequence would not fix the plateau — the reason to reach Allied has to
   be visible from Warm.
2. **The garrison is passive; the gift and the travel are verbs.** A
   continuous benefit that needs asking for is friction with no decision in
   it, and a discrete grant that arrives unasked is a surprise the player
   cannot plan around. Rejected: one shape for all three.
3. **Aid is free while the band holds.** The cost was earning the band.
   Rejected: spending standing per grant — it makes every gift a decision to
   decline, and towns oscillate at the boundary. Rejected: a Credit price —
   it reads as one more shop rather than as a relationship paying off.
4. **The garrison is a magnitude, not a boolean, and it ramps from Warm.**
   `garrison_defense() -> u32`. Three Allied-only booleans would be three
   copies of one predicate; a ramp gives `Warm` a second thing and makes the
   query carry information the band alone does not.
5. **The gift is labour, not power.** It arrives as base staff at a
   multiplier derived from the town, not from the player. A free companion
   scaled to the zone is the exact shape closed off when scan, the Terminal,
   free rest and the Market's fragment listing were shut — *progression is
   earned by fighting*. Feeding `resources::LabourDemand`, which is the
   genuinely scarce thing, hands out no combat progression.
   **This is a tuning claim, not a structural guarantee.** `ProgramRole` is
   derived, so nothing prevents the player putting a gift in the party; what
   makes that a bad trade is the number, and the number is unmeasured.
6. **Travel spends the ticks the walk would have.** Chebyshev distance times
   a per-tile constant, drained through the same loop `Game::craft` uses, so
   upkeep, decay, needs and production all advance. What travel removes is
   the encounters, not the time. Rejected: instant travel, which makes
   distance from the anchor cost nothing while `Game::field_stat_mult` still
   treats it as a difficulty axis. Rejected: a flat charge, which makes the
   far side of the map exactly as cheap as the near side.
7. **No new `Mode` variant.** Both verbs are uppercase keys on screens that
   already exist. `ALL_MODES` is hand-written and ends its draw match in
   `_ => {}`, so a new variant ships as a blank screen, and its length is a
   semantic merge conflict between branches. Adding none avoids both.
8. **One additive save field, no `SAVE_FORMAT_VERSION` bump.**
   `Relation::last_gift_tick` behind `#[serde(default)]` — what field-named
   RON was adopted for.

## Correction to the parent spec

`2026-09-04-settlements-design.md`'s Phase 6 section says sorties have "no UI
at all… out of scope unless asked". **Amended 2026-09-05: false since
`v0.13.99`.** The Relay hub shipped `Mode::Dispatch` and `Mode::SortieSquad`
together — `Game::sortie_board` and `Game::dispatch_sortie` are both reached
from the hub, `draw_dispatch` and `draw_sortie_squad` are both in the draw
match, and `crates/app-core/src/tests/dispatch.rs` covers the squad picker's
open, toggle, dispatch and Esc paths. `INDEX.md` already recorded this
correctly; the spec body did not.

## What already exists

Verified against the source on 2026-09-05. Verify a line before relying on
it, but do not re-survey.

**The band and its queries.** `crates/engine/src/settlements/relations.rs` —
`Relation { standing, trade_credits }`, `band()`, `clamp()`, and four named
queries on `Standing`: `label`, `job_slots`, `refuses_service`,
`preys_on_routes`, `allows_standing_route`. Each is an exhaustive match
(`cell_mark`'s rule) with its own census test walking all five bands. The
module doc states the extension shape outright: a new consequence is a *new
query answered by the same exhaustive match*, never a table of effects.
`resources::Standings` (`resources.rs:1590`) is
`BTreeMap<SettlementKey, Relation>`. `Game::adjust_standing` is the one
writer and holds the only clamp.

**Raids.** `Game::raid_check` (`game/base/upkeep.rs:320`) rolls
`RAID_CHANCE_PER_TICK`, then gates on `RAID_MIN_ZONE` and
`RAID_MIN_BASE_STAFF` *after* the roll so a miss leaves the RNG stream
untouched. `run_raid` (`:379`) picks a random `With<Durability>,
With<Structure>` target and applies
`RAID_DAMAGE.saturating_sub(self.total_raid_defense())`.
`total_raid_defense` (`:230`) is a bare sum of `StructureDef::raid_defense`
over deployed structures — **the whole hook**. `raid_defense_active`
(`:226`) is the existing frontend seam for "the shield network is up".

**Adoption.** `Game::adopt_program` (`game/spawning.rs:407`) is
`pub(crate)`, takes `(species_id, x, y, stat_mult)`, spawns scaled, strips
`Hostile`/`WanderAi`, inserts `roster_parts()` and installs innate routines.
It is one of the four doors into the roster and the only one that is not a
fight.

**The town screen.** `App::handle_settlement_key`
(`crates/app-core/src/app/inspection.rs:141`) handles `Esc`, `[M]` (market)
and `[J]` (board). Both letter keys first ask a **reach check** —
`Game::settlement_view(key).is_some()` — because `x` opens this page from
anywhere inside `EXAMINE_RANGE_TILES` while `Game::settlement_reach`
(`game/settlement_market.rs:69`) is Chebyshev 1. Uppercase throughout, for
`lowercase-letters-are-row-selectors`.

**The Relay hub.** `App::handle_dispatch_key`
(`crates/app-core/src/app/dispatch.rs:123`) handles `Esc`, `[S]`, `[C]`,
`[X]`, and falls through to `selected_index` for row selection. Rows are one
continuous numbering over sortie sites then route destinations, resolved by
`dispatch_row`. `Game::dispatch_reach` (`game/sortie.rs:76`) → `DispatchReach`
is the door both a squad and a route already leave through, gated on
`StructureDef::dispatches_sorties`.

**Geometry and placement.** `routes::settlements_near_route` is the
precedent for pure settlement geometry (it measures to a *segment*; this
spec measures to a point). `Game::standable_near`
(`game/spawning.rs:1013`) is a private `fn` and needs widening to
`pub(crate)`. A settlement **tile admits nobody** — the bump is the fourth
arm of `move_player`'s ladder, queues `resources::PendingVisit` and leaves
`Position` unchanged.

**Refusal shape.** `Game::commit_caravan_basket` is the rule every commit
door follows: every refusal lands before anything is spent, asserted **per
refusal**, because one test over one path passes against every other path
that never spends anyway.

## 1. The garrison

A new named query, exhaustive, its own census:

```rust
// crates/engine/src/settlements/relations.rs, on Standing
pub fn garrison_defense(self) -> u32
```

`Hostile`/`Cold`/`Neutral` answer 0; `Warm` answers
`SETTLEMENT_WARM_GARRISON`; `Allied` answers `SETTLEMENT_ALLIED_GARRISON`.
The ladder only ever climbs, which its census asserts alongside the
exhaustive walk — `every_standing_band_answers_how_many_jobs_it_posts` is the
shape.

It enters the game in exactly one place: a second term in
`Game::total_raid_defense`. Every settlement in `resources::Settlements`
whose resolved tile is within `SETTLEMENT_GARRISON_RADIUS` (Chebyshev, to the
base anchor) contributes its band's answer.

**The settlement contribution is clamped on its own**, to
`SETTLEMENT_GARRISON_MAX`, before being added to the structure sum. Without
it, five Allied neighbours drive `RAID_DAMAGE.saturating_sub(...)` to zero
and delete raids as a mechanic rather than softening them — and they do it
silently, since a raid that lands for nothing still logs.

A town whose tile has never been resolved does not garrison: `Settlements`
records the tile once a town is *found*, so aid follows discovery, which is
the same rule the market and the board already follow.

## 2. The gift

```rust
// crates/engine/src/game/settlement_relations.rs
pub fn request_program_gift(&mut self, key: SettlementKey) -> Result<(), String>
```

The one door, `commit_caravan_basket`'s order. Refusals, each landing before
anything is spent and each with its own test:

1. game over
2. a battle is running
3. no settlement under that key
4. out of reach — `settlement_view(key).is_none()`, the check `[M]` and `[J]`
   already make from this screen
5. the band does not gift — a new query, `Standing::gifts_programs()`,
   exhaustive, `Allied` alone
6. the cooldown has not elapsed

**The cooldown is the limiter, because the price is free.**
`Relation::last_gift_tick: Option<u64>` behind `#[serde(default)]`, compared
against `SETTLEMENT_GIFT_COOLDOWN_TICKS`. Additive, so no version bump; a
save→load round trip is what tests it, since a RON round trip alone cannot
catch a `#[serde(skip)]`.

**What arrives.** `adopt_program` at the base anchor, with a `stat_mult`
derived from the town's tier and *not* from the player's level or the zone —
decision 5. The species is drawn from the town's `Specialty`, seeded from
`(world seed, settlement key, gift count)` rather than `resources::GameRng`,
so the roll is reload-stable, cannot be save-scummed, and cannot shift the
seeded stream. It lands as base staff by omission, the way a returning
sortie member does: `ProgramRole` is derived, and a program that is not
fighting beside you, not wielded and not away **is** staff.

The line is logged through the base's own `MessageSource`, since the arrival
is base news and the player may be nowhere near.

## 3. The travel

Gated **once**: the destination town is Allied, and the base has a Relay.
Both halves are asked through machinery that exists —
`Standing::hosts_a_relay()` (a third new query, exhaustive, `Allied` alone)
and `Game::dispatch_reach`, the door a squad and a route already leave
through. One rule rather than a different gate per direction.

The name is about the *town's* half of the link and nothing else: it asks
whether this town will accept a relay to your base. The structure at your
end is `StructureDef::dispatches_sorties`, read through `dispatch_reach`,
and this query never speaks about it.

```rust
pub fn travel_to_settlement(&mut self, key: SettlementKey) -> Result<(), String>
pub fn travel_to_anchor(&mut self) -> Result<(), String>
```

Refusals, per-refusal tests again: game over, mid-battle, underground
(`require_surface`), no Relay, the town not Allied, the town's tile
unresolved, and **no standable neighbour at the far end**.

**Where you land.** Never on the settlement tile — it admits nobody.
Arrival is `standable_near` of the town's tile, widened to `pub(crate)`;
a town with no standable neighbour refuses rather than stranding the party
inside rock. Travelling home lands on the anchor, which `run_symlink`
already establishes as a valid landing.

**What it costs.** `SETTLEMENT_TRAVEL_TICKS_PER_TILE` times the Chebyshev
distance actually travelled, drained through the same tick loop
`Game::craft` uses. The engine door drains its own ticks; `after_tick()` is
app-core's (`app/lifecycle.rs:441`) and `handle_key`'s tail already pays it
for this keypress as it does for every other, so travel adds no fourth
tick-spending path to that list.

Arriving at a town queues `resources::PendingVisit`, so the town screen
opens on arrival exactly as walking into the tile does — one arrival
behaviour, not two.

## 4. Screens

No new `Mode`. Two new uppercase keys:

- **`[G]` on `Mode::Settlement`** — request a program. The reach check first,
  the same one `[M]` and `[J]` make, then the engine door; a refusal is one
  sentence through `App::refuse`, which is the one door for a refusal on two
  surfaces.
- **`[T]` on `Mode::Settlement`** — travel to the anchor.
- **`[T]` on `Mode::Dispatch`** — travel to the highlighted *destination*
  row. `dispatch_row` already resolves a row to `Site` or `Destination`, and
  `[T]` on a site refuses the way `[C]` on a site already does.

`[G]` and `[T]` are free on both screens (`Mode::Settlement` uses `M`/`J`;
the hub uses `S`/`C`/`X`).

The town screen should say what the town is currently worth — the band's
label is already there; the aid line is derived from the same three queries,
so a screen and a door cannot disagree about whether aid is available.

## 5. Testing

Censuses in `relations.rs`, each failing the build, each the shape the four
existing ones use — exhaustive walk over all five bands:

- every band answers `garrison_defense`, the ladder only climbs, and nothing
  below `Warm` contributes
- every band answers `gifts_programs`; `Allied` alone
- every band answers `hosts_a_relay`; `Allied` alone, and a band that hosts a
  relay never also `refuses_service`

Engine tests:

- an Allied town inside the radius softens a raid; the same town outside it
  does not
- the settlement contribution is clamped: many Allied neighbours cannot drive
  raid damage to zero
- a gift arrives as staff, through `roster_parts` — the roster's one barrier
- the gift's species is stable across a reload (derived, not `GameRng`)
- the cooldown refuses the second request and the tick budget releases it
- **every refusal spends nothing, asserted per refusal**, for all three doors
- travel lands on a walkable neighbour and never on the settlement tile
- travel from a town with no standable neighbour refuses and moves nobody
- travel spends the ticks it quoted
- a save→load round trip preserves `last_gift_tick`

app-core tests, `tests/dispatch.rs`'s shape:

- `[G]` out of reach refuses without calling the engine door
- `[T]` on a hub site row refuses; on a destination row it travels
- a refusal shows on `App::status_line` and in the log, once each

`balance_sim` gates none of this — it models no raids, no towns and no loot.

## Open, deliberately

- **Every constant here is a guess no instrument in this repo can check.**
  `SETTLEMENT_WARM_GARRISON`, `SETTLEMENT_ALLIED_GARRISON`,
  `SETTLEMENT_GARRISON_RADIUS`, `SETTLEMENT_GARRISON_MAX`,
  `SETTLEMENT_GIFT_COOLDOWN_TICKS`, the gift's stat multiplier and
  `SETTLEMENT_TRAVEL_TICKS_PER_TILE`. Seven figures, and the feel questions
  behind them — is a garrison noticeable, is the gift worth the walk, does
  travel make the map feel smaller — are answerable only at the keyboard.
- **Whether the gift should ever be a fighter.** Decision 5 ships the
  conservative number on purpose. Raising it is a retune, not a rewrite; the
  door and the derivation do not change.
- **Town-sourced raids and hostile patrols** stay deferred. They are the
  angry end of the same ladder and each is another named query on the same
  exhaustive match — this spec does not block them and does not build them.

## Amended in the build, 2026-09-05

Recorded here rather than left for a reader to discover by diffing. Each was
argued in a doc comment at the time; this is the same argument written back.

1. **`travel_to_anchor` takes a `SettlementKey`**, not no arguments. The gate
   is the *town's* willingness to hold the link, so the door has to know
   which town.
2. **The gate is two rules, not one.** §3 claimed a single rule for both
   directions. The Relay stands in base space, so `require_surface` would
   refuse the only place an outbound trip can start, and `dispatch_reach`
   answers `OffBase` from the town an inbound trip starts at. Outbound asks
   `DispatchReach::AtRelay`; inbound asks that a Relay stands and that the
   town is in reach.
3. **`[G]` and `[T]` make no app-core reach check**, where §4 said they
   would copy `[M]`/`[J]`'s. Those two open a *screen* whose view answers
   `None` from across the map; these call doors that refuse in their own
   words, so the reach rule lives in one place. The aid *lines* on the page
   are reach-gated to match, which the first build missed.
4. **`standable_near` was not widened.** `spawning::ring_tiles` was
   extracted instead, so the band-0 and band-1 searches share one definition
   of "nearest" rather than one calling the other with a flag.

## Found by review, and fixed

The whole-branch review caught four things the suite did not, all now fixed
with tests. Kept here because each is a trap this feature's shape invites:

- **The tick loop did not break on a fight**, unlike every other multi-tick
  loop in the engine, and **neither travel key called `after_world_action`**,
  so a battle opened mid-trip left the map drawn over it and the arrival cue
  queued for a later, unrelated action.
- **The garrison aid line was a copy** of the fold's radius check under a doc
  comment claiming it was a call.
- **The gift and relay lines ignored reach**, offering what the doors then
  refused.
- **`travel_to_anchor` had four refusals with no test**, which is exactly the
  hole the per-refusal rule exists to close: the two shipped tests passed
  against all six paths.

