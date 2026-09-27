# Drop pods — design

**Status:** approved in brainstorm 2026-09-27; interface amended the same day (see *Interface with work assignments*).
**Depends on:** the work assignments deliverable (separate spec, not yet
written). This spec assumes it has shipped and names only the one thing it
needs from it — see *Interface with work assignments*.

## Intent

A base-built structure that lets the player call one base staff program into
a **tactical** fight already under way: "call for reinforcements." The pod is
spent on use and recharged by the base crew, so the number of calls a fight
can make is the number of charged pods the base keeps. Pods may later carry
trade goods; that is out of scope here.

## Decisions

| Question | Decision |
|---|---|
| Which fights | Every tactical fight — surface and Stack — **except a siege**, whose staff are already on the board. Group-model fights never (the effect is `tactical_only`). |
| Who drops | A program the player has assigned **Drop Trooper** on the work assignments screen, on shift as base staff. Chosen at call time, not loaded in advance. |
| Which trooper | The one **closest to the pod that fires**; ties by the assignments table's order. No `GameRng` draw. |
| While waiting | A trooper works normally — posted, scheduled, earning. The assignment only says who *may* be called. |
| On arrival | Stunned for one round (reorienting). |
| Party cap | A reinforcement **may exceed** `MAX_PARTY_SIZE`. It leaves the party when the fight ends. |
| After the fight | Back to base staff. Death follows the ordinary rules (`bench_or_dissolve`). XP earned is kept. |
| Spent pod | One structure, recharged — never despawned by use. |
| Recharge bill | 2 Core Fragments + 1 Cache Grain, walked to the terminal by the crew. |
| Learning the routine | A synthesised routine research node, prerequisite the terminal; installed in a slot, priced in Power. |

## Components

### 1. Structure (content + schema)

- `assets/structures/drop_pod_terminal.ron`, `footprint: 2`, research-gated
  like other buildings, no upgrade path (so a recharge and an upgrade never
  compete for the one cell a `BuildSite` names).
- New `StructureDef::drop_pod: Option<DropPodDef>`, `#[serde(default)]`.
  `DropPodDef { recharge_cost: Vec<(ItemId, u32)> }`. A structure *is* a pod
  because it declares this field, never because of its id —
  `dispatches_sorties`' rule, so a mod's second pod structure works.
  Recharge time comes from the ordinary build-tick derivation over the bill
  (`BuildSite::required_ticks` is derived, never stored).
- `assets/structures/README.md` documents the field.
- Build cost: in the range of its peers (`core_fragment` ~20 plus a
  substrate); a tuning call made at implementation, not a design one.

### 2. Charge state

- `components::DropPod { charged: bool }` on the terminal entity, written by
  `Game::spawn_structure` (the one place a structure's component list is
  written) whenever the def declares `drop_pod`. A freshly raised terminal is
  **charged**.
- Saved on the structure's save record as `#[serde(default)]`; an absent
  value loads as charged. No `SAVE_FORMAT_VERSION` bump.

### 3. Recharge is a build request

- `BuildGoal` gains a third variant, `Recharge`. Per `BuildGoal`'s own doc,
  the axis of change is *what a finished site does*: the bill, the fetch, the
  walk, the delivery, both latches, the reach check and the cancel refund are
  inherited unchanged. `raise_one_tick`'s completion arm sets
  `DropPod::charged = true` on the structure resolved **by tile**, exactly as
  `Upgrade` resolves its machine.
- `BuildSite::program` is `None`: a recharge runs no job, so it costs no
  program (the Shield/shelf rule).
- Filed **automatically** the moment a pod is spent, at the bottom of the
  build wants' prepended block like any build request. Cancelling it by hand
  is allowed and refunds as any cancel does; the pod then stays empty until
  the player files a recharge from the structure's menu.
- `views::BuildOrderRow` and the examine line read it through the existing
  derivation; the goal needs one label ("recharging").

### 4. The routine

- `AbilityEffect::Reinforce` (no fields). `tactical_only()` answers `true`
  for it — the match is exhaustive, so this is a compile-time census. It is
  **not** `Summon`: `routine_tree::gets_node` deliberately excludes `Summon`,
  and a reinforcement is a real roster body, not a fork.
- `assets/abilities/call_reinforcements.ron`, untargeted (no aim;
  the landing cell is chosen by the engine), a Power cost and a cooldown in
  the band of its peers. README updated.
- Research: synthesised node from `AbilityDb`, gated on owning a structure
  with `drop_pod` — confirm at plan time how a routine node declares a
  structure prerequisite; if it cannot, that gate is the one schema addition
  to the routine tree and belongs in phase B2.
- Four new `RoutineRefusal` variants, checked in `ability_unavailable` before
  the Power check so the refusal and the charge quote one price:
  `NoPodReady`, `NoTrooperAvailable`, `InSiege`,
  `NoLandingCell` (no free cell adjacent to the player). Each is asserted
  **per refusal** to spend nothing.

### 5. Arrival — `Game::call_reinforcement`

One door, called from `use_ability`'s `Reinforce` arm:

1. Candidates: programs with the Drop Trooper assignment whose
   `Game::program_role` is `Staff` and that `Game::is_on_shift` (a `Downed`
   or off-shift body is excluded by that predicate, not by a second check).
2. Pod: among terminals with `DropPod::charged`, pair each candidate with its
   nearest charged pod (base-space Chebyshev distance); take the pair with
   the smallest distance, ties by assignments-table order.
3. Spend the pod's charge; file its `Recharge` request.
4. Push the trooper into `resources::Party` **without** the `MAX_PARTY_SIZE`
   check; insert `components::Reinforcement` on it.
5. `TacticalBattle::place` on a free cell adjacent to the player, then
   `insert_after_cursor` — the summon path's two calls, not its `Summoned`
   marker, so the trooper is commanded by the player rather than handed to
   `tactical_ai_actor`.
6. It loses its very next turn — **amended at B2**: not a `Stun`, which
   costs no turn on a battle map, but `Reinforcement::reorienting`, passed
   by `hand_on_turn`.
7. Log line naming the trooper through `creature_label`, and a fx cue.

Posting: **amended at B2** — the role stops reading `Staff`, but the
scheduler keeps a non-staff body's `Task` post covered and does not run
mid-fight, so `call_reinforcement` strips `Task` and `Carrying` itself.

### 6. After the fight

- `finish_fight` removes every `Reinforcement` body from `Party` and strips
  the marker, **alive or dead**, after the dead-party loop has run
  `bench_or_dissolve` on the fallen. The survivor's role reads `Staff` again
  and the scheduler reposts it.
- Its base-space `Position` was never written (a battle map never writes
  `Position`), so it resumes where it stood. Death-then-bench follows the
  Bay's existing path.
- XP is awarded as to any party member.
- `Reinforcement` is not saved. An ordinary tactical fight is not saved
  mid-fight — verify at plan time; if it is, the marker needs a
  `CreatureSave` field in B2.

## Interface with work assignments

Drop pods need exactly one query from that deliverable:

    Game::drop_troopers() -> impl Iterator<Item = Entity>  // table order

and one constraint on its data model: **Drop Trooper coexists with a labor
assignment** on the same program (a program may be a Miner *and* a Drop
Trooper), because a trooper keeps working while it waits. If the assignments
model is one-assignment-per-program, Drop Trooper is a separate flag beside
it, shown as its own column.

The work assignments spec settled this: its model is a set of job-kind
checkboxes per program, and **the Drop Trooper flag, its column and
`drop_troopers()` ship here, in drop pods phase 1**, and the column draws
once a pod structure stands.

**Amended 2026-09-27: the flag is not a `Duties` field.** Work assignments
shipped `Duties` as a *denied* set (`DepotFilter`'s precedent — absent or
empty means every column checked), and Drop Trooper is opt-in: as a fifth
`Duty` every program, and every save written before this, would be a
trooper by default. So it is its own marker, `components::DropTrooper`,
saved as a `CreatureSave` bool behind `#[serde(default)]` (absent → not a
trooper, no version bump), and `drop_troopers()` is a query over that
marker ordered by `StaffRank`. The Base staff table draws it as a column
beside the `Duty` columns but it is not one of them — `duty_admits` never
reads it.

## Phases

| Phase | Covers | Testable without |
|---|---|---|
| **B1** | `DropPodDef`, `DropPod` component + save, `BuildGoal::Recharge`, terminal `.ron`, map mark (pod drawn on its cell only while charged) | combat |
| **B2** | `AbilityEffect::Reinforce`, routine `.ron`, research node, refusals, `call_reinforcement`, fight teardown | new UI |
| **B3** | Examine line, structure menu "recharge" row, help page, CHANGELOG | — |

Each phase is one crate-local TDD pass and lands on its own.

## Tests (intent)

- B1: a spent pod files exactly one `Recharge` site with the def's bill and
  no program; completing it charges the pod and spawns nothing; a save
  without the field loads charged; a save with an empty pod round-trips
  (save→load, not only RON, per the skipped-field trap).
- B2: each refusal spends no Power and no charge; the closest trooper drops
  and ties go to table order; the trooper loses exactly its next turn; a
  sixth body sits in a full party; after the fight the trooper is `Staff`
  again, posted, and the party is back to its size; a dead reinforcement is
  benched on Forgiving; the arena/group model never offers the routine.
- Mutation-check the closest-pod pairing and the cap bypass.

## Out of scope

Pods carrying trade goods; a trooper chosen by picker at call time; a
distance-from-base gate; pods in sieges or group fights.
