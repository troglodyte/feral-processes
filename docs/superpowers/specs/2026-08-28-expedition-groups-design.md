# Expedition groups

**Status:** approved, not implemented
**Date:** 2026-08-28

Base staff that are not in the party do exactly one thing today: work the
base. A developed program you are not fielding is a number in the roster.
This makes the idle half of the roster a *second front* — you form a squad,
send it to a named site, and it fights its way through a run of battles and
comes home with what it could carry, or with fewer bodies than it left with.

The whole feature is a bet on a roster you built, resolved with no player
input, and every part of the design below exists to stop that being a free
faucet.

## The thesis

Progression is earned by fighting. An expedition pays out without the player
in the fight, so it has to pay for itself four ways at once, and the design
was chosen with all four live:

1. **Bodies.** An expedition program is not `Staff`, so it works no machine,
   hauls nothing and digs nothing for the whole trip.
2. **Materials.** Provisioning is charged from base stock at dispatch.
3. **Real risk.** The programs take real damage from real swings and can be
   lost.
4. **Capped yield.** It pays less than fighting the same programs yourself,
   and that cap is *earned* rather than tuned — see "Why the yield is lower"
   below.

## Naming

`expedition` is already used in prose in this repo to mean *the player's own
outing from base* — `difficulty.rs:140` and `:281` ("a reboot ends the
expedition"), `components.rs:1138`, and the Repair Bay spec's "the expedition
is the unit of risk". It is not an identifier anywhere, so this is a prose
collision rather than a compile-time one, and it is weaker than the
`run`/`runner` collision `CLAUDE.md` records.

It is still worth settling before implementation, because this feature would
make the word ambiguous in exactly the documents that currently use it
loosely. **Recommendation: `Sortie`**, which has no existing meaning here and
fits the register. This spec is written in `expedition` because that is the
word the request used; a rename is a find-and-replace on a spec, and a
nightmare on a shipped feature.

## Part 1 — The role

`ProgramRole` gains a fourth variant:

```
Wielded, InParty, Expedition, Staff
```

ordered after `InParty` and ahead of the `Staff` fallback, keeping the rule
that `Staff` is *what is left over* rather than something assigned.

`party::role_of` is the one rule and gains a parameter for the in-flight
roster, alongside `party` and `wielded`. **This deliberately fails to compile
at both appliers** — `Game::program_role` and `base_entropy_system` — which
is the forcing function that stops a second copy of the rule existing. That
is the same argument the function's own doc comment already makes.

Almost everything the feature needs then falls out as an *omission* rather
than a check:

| Consequence | Mechanism |
|---|---|
| Never posted to a machine | `schedule_base_labour` filters on staff |
| Never wanders the base | `drift_idle_staff` keeps the staff list |
| Rock cannot reseal on them | `base_entropy_system` narrows through `role_of` |
| Gone from the map and the examine ray | `position_is_honest` is `wears_job_mark` plus *idle base staff*, and an away program is neither, so `drawn_on_surface_map` returns false |
| Still counts against `pet_capacity` | They are still on the roster — matching a `Downed` program keeping its slot |
| Base reads as short-handed | `LabourDemand`'s shortfall grows while they are away, and the work-order header already draws it |

The last row is the "bodies" cost made visible, and it costs no code.

**Needs keep draining while a program is away and nothing services them.**
This is deliberate and also free: `step_off_shift` is reached only from
`drift_idle_staff`, which no longer sees them, so an off-shift expedition
member simply never walks anywhere. A long trip returns frayed programs that
then want an amenity. No exclusion is needed and none should be added.

## Part 2 — The Relay and the research gate

Dispatching requires a **Relay** structure standing in the base, itself
behind a research node. Both halves are pure data:

- `assets/structures/relay.ron` — a normal `StructureDef`.
- `assets/research/<node>.ron` — carrying `cost`, `min_zone`, `requires` and
  `unlocks_structures: ["relay"]`.

`ResearchDef::unlocks_structures` already exists (`research.rs:57`), so the
gate costs **zero Rust**. This is the moddability rule working as intended:
a mod can add a second dispatch structure, or move the gate, without touching
the engine.

### Reach

`Game::expedition_reach` mirrors `Game::broker_reach`, returning three states:

```
NoRelay | OffBase | AtRelay
```

Three states rather than two booleans for `NoPost::BoxedIn`'s reason: "no
Relay built" and "not standing in base" leave the player different errands,
and a screen that cannot tell them apart says the wrong sentence.

**The reach test is `broker_reach`'s, verified live rather than remembered:**
`base_pos()` for the party's base-space coordinates, then
`BaseGrid::is_floor(x, y)`. It is emphatically **not** `Platform::covers` —
`resources::Platform` no longer exists, and `CLAUDE.md`'s base section is
stale on this point. It does not measure the distance to the Relay: a Relay
stands on laid floor by construction, so its tile says nothing the base does
not.

The base menu's row test calls `expedition_reach`, **not** `expedition_board`
— the board rolls a full set of sites before it can answer, which is the
exact trap `CLAUDE.md` records for the Broker's own row test.

## Part 3 — The board

`Game::expedition_board` is **derived, never stored** — the Broker board's
rule, and for the Broker board's reasons.

It is recomputed on every read from three values already in the save: the
world seed, `ZoneLevel`, and `clock / EXPEDITION_BOARD_ROTATION_TICKS`,
folded with `EXPEDITION_SALT`. Consequences:

- **No save field.** Nothing about the board is written.
- **No save-scumming.** Reloading reproduces the identical board, because the
  inputs are identical. There is no stored roll to reroll.
- **It rotates on its own** as the epoch advances.
- **No `GameRng` draw.** A draw here would not survive a reload and would
  shift every later roll in the run — `stack::generate`'s rule.

Sites come from `assets/expeditions/*.ron`, each authoring a name, a blurb,
a **risk offset** (steps above the zone baseline) and a **battle-count range**. The board rolls three per
epoch, and rolls each site's battle count within its range off the same
board seed — so the count is fixed the moment the offer appears, not at
dispatch, and the screen can quote it.

**An absent `assets/expeditions/` directory loads silently empty**, which
means no board and no feature — `assets/needs/` and `assets/memories/`'
property, and the same rule applies: never gate a system or a screen on the
directory being non-empty. Deleting it restores the pre-expedition game.

## Part 4 — Dispatch

`Game::dispatch_expedition(row, members)`.

**Every refusal lands before anything is spent** — `commit_caravan_basket`'s
rule. In order:

1. `expedition_reach() != AtRelay`.
2. A named member is not `ProgramRole::Staff` — party and wielded programs are
   refused, so the player unparties first. This is deliberate: seconding a
   party member should be an explicit act, not a side effect of a dispatch
   screen.
3. A named member is `Downed`.
4. A named member is below `EXPEDITION_MIN_HP_FRACTION` of max HP. Sending a
   hurt program on a twenty-fight trip is the mistake the abort rule below
   cannot save you from, because it fires on the first battle.
5. The dispatch would leave the base with **no** staff at all. Production
   stops dead and a raid lands on an empty base; this is the same category of
   guard as `max_deployed`.
6. The provisioning cost is not in base stock.

Only then: `stock::spend_from_base` takes the provisioning cost off a shelf
(a teleport off the shelf is right here — this is a base cost paid at the
Relay, not a build a body walks to), the record is written, and the dispatch
line is logged.

**The record stores the whole resolved target**, never a board index or an
id. `ActiveContract` stores the whole resolved `ContractDef` for exactly this
reason: a board that rotates while the squad is out, or an
`assets/expeditions/` file edited between sessions, must not be able to
rewrite or strand a trip already in flight.

## Part 5 — The trip

`Game::run_expedition` is a **`Game` method, not a bevy system** —
`run_dig_crew` and `run_repair_bays`' reason. It names programs through
`creature_label`, it logs, and it damages through `apply_damage`; a bevy
system would have to be a second copy of all three.

### Duration

Derived, never authored — `BuildSite::required_ticks`' rule, which comes off
the stored cost and is never written beside it.

```
duration = EXPEDITION_TRAVEL_BASE_TICKS
         + EXPEDITION_TRAVEL_PER_RISK_TICKS * risk_offset
         + EXPEDITION_TICKS_PER_BATTLE      * battles
```

**`risk_offset` is the site's own step above the zone baseline, never the
absolute danger band.** The opposition is drawn at `danger_steps() +
risk_offset`, so a site stays as dangerous *relative to the run* in zone 9 as
it was in zone 1 — but the duration term reads the offset alone. Written
against the absolute band instead, every trip in a deep zone would take
enormously longer for no reason the player could name, and the feature would
quietly stop being usable late.

With the constants proposed below:

| Site | Risk offset | Battles | Ticks |
|---|---|---|---|
| Quiet | 0 | 6 | 270 |
| Middling | 1 | 12 | 465 |
| Nasty | 2 | 20 | 700 |

Travel dominates, which reads correctly — a fight is quick and getting there
is not. The middle case sits near half a caravan visit interval
(`CARAVAN_VISIT_INTERVAL_TICKS` is 900), which was the requested reference
point; it is a reference and not a derivation, and nothing computes one from
the other.

**Two guards on the formula.**

*The board quotes the duration through the same call the trip uses.* One
`Game::expedition_duration(...)`, read by the Relay screen and by
`dispatch_expedition` alike — `views::BuildOrderRow`'s rule that every figure
on a screen is a call rather than a copy. A screen quoting one number while
the countdown runs another is precisely the failure that rule exists for.

*Roster strength must never shorten a trip.* No term for member count, level
or power. A stronger squad shows up as better **outcomes** — more survivors,
more loot, fewer casualties — and never as a faster cycle, or the feature
becomes a throughput multiplier that scales with itself. Duration is a
property of the place, the way `BASE_ROCK_DURABILITY` is never scaled by the
player.

### Resolving a battle

Battles fire at even intervals across the middle of the trip, with travel
split half out and half back.

**Each battle resolves atomically inside a single tick**: spawn, fight,
despawn, all before `run_expedition` returns. This is the load-bearing
decision of the whole feature. No bevy system runs mid-method, so the
hostiles are never observed by the map, the examine ray, `cull_to_cap`,
`ensure_local_population` or anything else — which means the feature does not
have to teach four systems about a new space, and cannot reintroduce the
"which space is this?" bug class. **A hostile that outlives its battle is a
defect**, not a tuning question.

One battle:

1. Pick a species from `habitat_pools` at `danger_steps() + risk_offset`.
2. `spawn_group` the pack at an off-map sentinel position.
3. Run rounds until one side is out.
4. Despawn every surviving hostile.
5. Award XP per kill and accumulate loot into the record.

### Why the fights are real

`battle::resolve_attack` is reachable with no battle open:
`Game::resolve_and_apply_attack(attacker, defender, swing)` takes **only
entities** and `combat_damage.rs` names `BattleState` twice, neither on the
swing path. So hit chance, the crit/hit/fumble/miss ladder, the fumble rungs,
damage bands and mitigation are all the real ones, by construction rather
than by a comment.

`Game::use_ability(ability, actor, name, recipients)` is likewise
`BattleState`-free, so **Specials are real invocations** — real damage bands,
real affinity scaling, real Power costs, real cooldowns.

What *is* coupled to `BattleState` is the trained enemy policy's
**selection**: `choose_wild_action_at` reaches `basic_attacks_that_reach`,
`living_targets` and `roll_enemy_target`, all of which read it. The policy is
therefore **not used here**, and that is a design decision rather than a
shortcut: it exists to make fights against *the player* interesting, and
ships with three features pinned to zero specifically to stop it learning to
hunt them. Off-screen it would be modelling an audience that is not present.

Both sides instead use one stated rule: **run the highest-priority Special
you can afford that is off cooldown, else a basic attack; target the front.**
Simple, stated, and — critically — it selects among real abilities and
resolves them through the real doors.

**An attack/defence bonus derived from a program's routine loadout was
considered and rejected.** It would be a second model of what routines are
worth, with a player-facing surface: the Relay screen would price three
Specials one way while the battle screen priced them another, and no test
could catch the drift. `CLAUDE.md` records four occasions where exactly this
bit this repo. Since `use_ability` is free, such a bonus would be inventing
an approximation of something that can simply be called.

### Why the yield is lower

The cap is earned rather than tuned, from two mechanisms already in the game:

- **Power does not recover in the field.** `power_regen_system` is
  `Query<&mut PowerReserve, With<Player>>` and returns early outside
  `Locale::Base` — companions never regen from a Recharger at all. So Specials
  taper across a trip: the opening fights have them and the closing fights are
  basic attacks. This is what makes a long high-band trip genuinely harder
  rather than merely longer.
- **No rest out there.** HP persists across the whole run of battles, offset
  only by provisions.

An explicit `EXPEDITION_XP_MULTIPLIER` below 1.0 is the third lever and the
only tuned one. It exists so the cap can be adjusted without disturbing the
two mechanisms above.

### Attrition and the abort rule

**Provisions restore HP between battles** at a flat rate
(`EXPEDITION_PROVISION_HEAL`), through `restore_hp`. This gives the
provisioning cost a mechanical role instead of being a toll, and it is the
single dial that decides whether a twenty-fight trip is survivable.

**The trip aborts on the first casualty.** The moment a member goes down,
remaining battles are skipped, survivors keep the loot earned so far, and
**the return travel still runs** — the countdown was always going to take
that long, and there is no teleport home.

One rule, two meanings, for free:

- **Forgiving** — `bench_or_dissolve` benches at HP 1 with `Downed`, keeping
  the roster slot. The squad comes home early with partial loot and everyone
  lives; the worst case is a wasted trip.
- **Permadeath** — the program that dropped is gone, but the trip aborts
  before a second is lost. The safeguard caps the disaster at one.

Casualties go through `Game::bench_or_dissolve` and nothing else — it is
already the one door and already branches on `DifficultyMode`, so this needs
no new branch.

Squad size still matters, and for the right reason: more bodies means more
damage out, means fewer rounds, means less incoming per member. A large squad
is what stops anyone dropping, not what absorbs the drop.

## Part 6 — Return and the report

The record is dropped, and members become `Staff` again **by omission**.

A Forgiving casualty comes home `Downed` and then walks itself to a Repair
Bay through the existing `Downed` arm of `drift_idle_staff`, which already
outranks the `OffShift` arm. No new recovery path.

Loot lands through `stock::return_to_depots`. **What does not fit is logged,
never dropped in silence** — that function's existing rule.

Dispatch and return each log one line. The summary is
`views::ExpeditionReport`, derived from the stored record: who went, what they
fought, what came back, who was hurt. Every figure a call, `BuildOrderRow`'s
rule.

## Part 7 — Save format

`resources::Expeditions` is a new saved resource holding the in-flight
records: members, the fully resolved target, ticks elapsed and total, battles
done and total, and accumulated loot.

**Additive behind `#[serde(default)]`, so no `SAVE_FORMAT_VERSION` bump.**
The save is field-named RON and an additive change costs no version bump.

Member entities follow whatever encoding `Party` already uses; the record is
a **named struct, never a positional tuple** — the one shape field-named RON
does not save you from.

## Part 8 — Tuning

All in `tuning.rs`, in a labelled section, never inline in a formula:

`EXPEDITION_SALT` is its own constant and never a reused one, following
`CARAVAN_SALT`'s idiom. `EXPEDITION_PROVISION_HEAL_FRACTION` is a fraction of
`max_hp` rather than flat HP, so provisioning keeps meaning something at the
level cap. `EXPEDITION_BOARD_ROTATION_TICKS` is longer than the longest trip,
so a board cannot rotate twice while you are deliberating over it.

```
EXPEDITION_TRAVEL_BASE_TICKS        = 150
EXPEDITION_TRAVEL_PER_RISK_TICKS    =  75
EXPEDITION_TICKS_PER_BATTLE         =  20
EXPEDITION_BOARD_ROTATION_TICKS     = 1200
EXPEDITION_BOARD_SLOTS              =    3
EXPEDITION_SALT                     = 0xE7ED_1710_5EED_0003
EXPEDITION_MIN_HP_FRACTION          = 0.5
EXPEDITION_PROVISION_HEAL_FRACTION  = 0.15
EXPEDITION_XP_MULTIPLIER            = 0.6
```

## Part 9 — Testing

Beyond per-function unit tests, the properties worth pinning:

**Role.** An expedition member is never posted by `schedule_base_labour`,
never moved by `drift_idle_staff`, never sealed by `base_entropy_system`, and
not drawn on the surface map or nameable by the examine ray. Five assertions,
because each is a different mechanism.

**Atomicity.** Entity count before and after `run_expedition` is equal, and
no `Creature` exists at the sentinel position afterwards. This is the one
that catches the whole "which space is this?" bug class.

**Refusals spend nothing.** Base stock is unchanged after each of the six
refusals. Asserted per refusal, not once.

**The board.** Identical across a save/load round trip; changes when the
epoch advances; drawing it advances `GameRng` by zero — asserted by comparing
the stream, since a test that only checks the board is stable passes against
a board that draws and discards.

**Abort.** A squad whose first member drops at battle 4 of 20 skips battles
5-20, keeps the loot from 1-4, and returns at the originally computed tick —
not earlier.

**Difficulty split.** The same wipe benches under Forgiving and dissolves
under Permadeath, and the Forgiving casualty keeps its roster slot.

**Empty catalogue.** With `assets/expeditions/` absent, the game runs, the
Relay row does not offer, and nothing panics.

**Balance.** Expedition XP per tick must sit below what the same programs
earn fighting alongside the player. `balance_sim` models no base production
and no abilities, so it cannot gate this — the assertion belongs in the
expedition tests, over the real assets.

## Part 10 — Deliberately not in scope

- **Recall.** Aborting a trip in progress needs a screen, and the return leg
  means it would not be instant anyway. It buys less than it costs.
- **"Missing" as distinct from "gone".** A recoverable-later program is real
  new machinery — where is it, what recovers it, does it decay. Permadeath
  dissolves.
- **The trained enemy policy off-screen.** See Part 5.
- **Seconding party members.** You unparty first.
- **Choosing a provisioning level at dispatch.** Flat rate; the dispatch
  screen stays a squad picker rather than a config page.

## Open decisions

1. **The name.** `expedition` versus `Sortie` — see Naming. Settle before
   implementation.
2. **The risk axis.** The board varies sites by a risk offset applied to
   `Game::danger_steps`, reusing the existing curve. The alternative is an authored per-site risk
   tier mapping onto a band, which gives finer authoring control at the cost
   of a second difficulty vocabulary. This spec assumes `danger_band`.
3. **The Relay's own name and cost**, and which research node gates it —
   content decisions, settled in the `.ron` files rather than here.
