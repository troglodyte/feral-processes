# Tactical surface battles

**Status:** design approved, not implemented
**Date:** 2026-09-09

An opt-in second combat model for surface fights: a disposable, generated
battle map with per-body movement, individual initiative, and routines that
resolve as cones, lines and radii. The Stack keeps the existing abstract
model unchanged. So does the arena, and so do nests, lairs and raids.

The reference point is Pool of Radiance: you walk the grid, you position, and
where you stand decides what you can reach and who catches your blast.

---

## 1. What is settled

| | |
|---|---|
| **Battlefield** | A battle map generated per encounter and destroyed at teardown. Not the world map. |
| **Coordinates** | Live in the `TacticalBattle` resource. No world `Position` is ever written. |
| **Control** | The player moves and acts for every party body. No auto-resolve. |
| **Turn order** | Individual initiative, rolled once at fight start, re-sorted as bodies die. |
| **Turn economy** | Up to *M* steps, then one action. The action ends the turn. |
| **Groups** | Dissolve. Every wild body is its own figure with its own HP, individually targetable. |
| **Friendly fire** | Full. The radius is the radius. |
| **Movement allowance** | Derived from the Speed stat that already feeds `combat_speed`, overridable per species in RON. |
| **Routine shapes** | `Single` / `Line` / `Cone` / `Radius` plus min–max range, authored in RON, ignored in the Stack. |
| **Enemy AI** | Positional scoring, in its own module. |
| **World clock** | Frozen for the duration of the fight. |
| **Disengage** | Walk a body off the board edge; it leaves the fight. |
| **Toggle** | Opt-in, default off, set on a new options screen, stored in `profile.ron`. |
| **Balance** | Explicitly allowed to be less tuned than the abstract model. |
| **Saves** | No format change. Fights are not serialized today and still are not. |

### In scope for tactical

Wandering surface encounters and town patrols.

### Out of scope — stays abstract

Everything in the Stack, nest guardians, lair fights, raids and Entropy
Sweeps, and the arena. A tactical arena is deferred until the mode works.

---

## 2. Architecture

No adapter type, no trait. Two resources, never both present.

```
crates/engine/src/tactical/
  mod.rs      TacticalBattle resource; the public verbs
  map.rs      BattleCell; the generator, pure in a spec
  deploy.rs   gather_pack bearings -> starting cells
  reach.rs    movement field; line/cone/radius resolution
  ai.rs       positional scoring
  view.rs     TacticalView
```

### The axis of change

Exactly three things differ between the models: **who acts and in what
order**, **what a targeting decision resolves to**, and **what the renderer
is handed**. Everything else — damage, the hit/crit/fumble bands, mitigation,
Power, cooldowns, passives, status effects, XP, drops, memories, telemetry,
`bench_or_dissolve` — is identical and is shared by construction rather than
by abstraction.

### The one shared door

`Game::ability_recipients(actor, target, chosen) -> Vec<Entity>`
(`game/combat_round.rs:906`) is already the sole converter from a targeting
decision to a list of entities, and `Game::use_ability(ability, actor, name,
recipients: &[Entity])` already consumes nothing else. Tactical mode does not
need a new seam; it needs a second *input* shape producing the same output.
`ability_recipients` gains a tactical arm and nothing below it changes.

That existing signature — `use_ability` taking `&[Entity]` rather than a
target — is the single reason this is a module and not a rewrite.

### Why not an adapter over `BattleState`

The two interfaces disagree on facts, not shape. A body on a cell has no
group letter, is not one HP bar shared with four others, has a *distance*
rather than an `engaged: bool`, has no `attackers_in_group` count, and needs
no `round_targets`/`retarget` repair because there are no group indices to
reshuffle. An adapter would have to invent six answers; each invented answer
is a lie the far side then acts on, and the far side needs its branch anyway.

### Why not a trait

Resolution mutates the world, so no method can meaningfully take `&self` —
threading `&mut Game` into a trait whose implementors live inside that
`Game`'s world is a borrow-checker fight this repo's guidance says to solve
with small functions. Two implementors, one crate, all call sites in `Game`.

### `Game::start_battle` is where the model is chosen

`start_battle` is already "the only path that caps a pack" and is the entry
point for every in-scope fight. It becomes the router: with the toggle on, on
the surface, and with no `NestGuardian` in the pack, it opens a
`TacticalBattle`; otherwise it calls `begin_battle` as today.

This is load-bearing for two reasons:

- The pursuit path (`game/turn.rs:480,523`) is shared by nest guardians and
  town patrols, so the scope split **cannot** be a per-call-site decision.
  Inspecting the pack is what separates them.
- `arena::stage` calls `begin_battle` directly, bypassing `start_battle`
  entirely. The arena therefore stays abstract **with no code written**.

---

## 3. The battle map

`BattleCell` — four kinds, the whole vocabulary:

| Kind | Movement | Sight |
|---|---|---|
| `Open` | 1 | clear |
| `Rough` | 2 | clear |
| `Cover` | blocked | blocks line and cone |
| `Blocked` | blocked | clear |

The four are the complete 2x2 of "can you cross it" against "can you shoot
through it", which is what keeps them from being an arbitrary list: `Blocked`
is a chasm you can see over and cannot cross, `Cover` is a boulder that stops
both. Note this repo's existing warning that `walkable()` and
`blocks_sight()` are not complements — the same asymmetry, already
established in the Stack.

Generated from `(biome, zone, encounter seed)`, pure in a spec the way
`stack::generate` is: reproducible, unit-testable, never saved. Tinted with
the encounter site's `biome_tint` so a marsh fight looks like a marsh.

**The map is sized for manoeuvre; the deployment is sized for contact.** Board
extent scales with body count so a single creature is not fought across a
stadium. Deployment places the two sides a few cells apart, on the real
bearing `gather_pack` found them at — walk into a pack from the side and you
start flanked — so no fight opens with a long walk toward the enemy.

### The reach field

`game::pursuit::walk_field` (`pursuit.rs:37`) is already space-agnostic: it
takes an origin, a radius and a step predicate, and its own doc states it
"deliberately knows nothing about which grid." It already serves the world
map (`pursuit_field`, `caravan.rs:856`) and base space (`base_space.rs:59`,
`base/hauling.rs:295,341`). The tactical reach field is a fifth caller with a
closure over `BattleCell`.

**One widening is required.** `walk_field` hardcodes uniform cost —
`.map(|n| (n, 1u32))`, "all eight directions cost the same single step" — so
`Rough` costing 2 does not fit. The step rule widens from `FnMut(coord) ->
bool` to `FnMut(coord) -> Option<u32>`, where `None` is blocked and `Some(c)`
is the cost. All four existing callers adapt as `|p| pred(p).then_some(1)`,
preserving behaviour exactly.

This changes a documented seam ("There is one Dijkstra walk on the surface,
and the step rule is a parameter"), so it carries the three writes:
`docs/seams.md`, the `seams` skill, and the rule in `CLAUDE.md`.

*Fallback if the widening is judged too invasive:* drop `Rough`'s movement
cost and leave `walk_field` untouched. This costs the terrain a dimension.

---

## 4. Turn model

One body acts at a time. Initiative is rolled once at fight start — unlike
`roll_initiative`, which rolls fresh each round — so the turn-order strip is
stable enough to plan against. It is re-sorted as bodies die.

On a body's turn: step with the arrow keys, one cell per press, until the
allowance runs out or you commit; then a lowercase letter picks an action
from the existing option list; a shaped routine opens a cell cursor to aim.
The action ends the turn.

Three pacing affordances, because a mopping-up round otherwise costs a full
decision per body:

- end this body's turn
- end the whole party's turn (mirrors the existing party-wide `[A]`/`[D]`)
- attack-nearest: step toward the closest reachable enemy and swing

Lowercase remains row selection; every new screen action is uppercase.

---

## 5. Shapes and ranges

Two new `#[serde(default)]` fields on `AbilityDef`, so every existing `.ron`
file and every mod keeps parsing untouched:

- `shape:` — `Single` | `Line(len)` | `Cone(len, degrees)` | `Radius(r)`
- `range:` — a min and a max

Both are read only in tactical mode and ignored entirely in the Stack.
`Cover` blocks line and cone through the existing `blocks_sight` predicate.
Recipients inside the shape take the effect regardless of side: **full
friendly fire**.

### The derived default, and its census

A routine with no `shape:` — which is every shipped routine on day one —
derives one from its existing `AbilityTarget`:

| `AbilityTarget` | Derived shape |
|---|---|
| `OneEnemyGroupFront` | `Single`, melee range |
| `OneAlly` | `Single`, melee range |
| `WholeParty` | `Radius`, centred on the caster |
| `WholeEnemyGroup` | `Radius`, centred on a chosen cell |
| `AllEnemies` | `Radius`, centred on a chosen cell |

A census in `tests/assets.rs`, **exhaustive on `AbilityTarget`**, pins that
mapping so a new variant cannot ship shapeless. This repo has already been
bitten by a `serde(default)` asset field with no census: the failure is
silent, and a routine that quietly becomes single-target reads as a nerf
rather than a bug.

---

## 6. Enemy AI

`tactical/ai.rs`, kept separate from the existing decision code.

Scores candidate destination cells from the reach field: can it reach a
target from there, does it hold range if it is ranged, does it avoid
clustering into an obvious area routine. Moves to the best cell, then hands
off to the existing choice of what to actually do.

`combat_policy.rs`'s trained weights speak group-index vocabulary and cannot
contribute here. The tactical AI is entirely new and hand-written.

---

## 7. Presentation

The existing `regions` layout is kept — `map_pane`, info column, log pane,
stock strip — with **battle tiles drawn in `map_pane` instead of
`view_tiles_at`**. That reuses `tile_origin_px`, the camera and its easing,
the glyph palette, con colours and sprites for free, rather than building a
second grid renderer that must stay in sync with the map's forever.

The turn-order strip and the action bar layer on top. The existing "watch"
camera override, which already aims the camera at a non-player body, is what
follows the active combatant.

### What does not transfer

`BattleTimeline` / `RosterFrame` snapshot *table rows* once per narrated log
line so the pane can rewind in lockstep with the reveal, and `SwingOutcome`
is keyed to raw lines. A grid fight resolving one body at a time in front of
the player is a different concept and needs its own answer. This is an open
design question, deferred to the implementation plan.

---

## 8. Prerequisite: the options screen

There is no settings screen in the game today. This feature needs one, and it
should be built and landed first.

- A new `Mode` plus its screen, its `ALL_MODES` entry, and its refusal-census
  row. **`ALL_MODES` is hand-written and the draw dispatch ends in `_ =>
  {}`**, so a new `Mode` compiles clean and ships as a blank screen. Known
  trap, not hypothetical.
- One toggle only, per YAGNI. The second option teaches what the screen
  actually needs to be.
- Stored on `achievements::Profile` (`achievements.rs:236`) as an additive
  `#[serde(default)]` field, beside `seen_notifications` and `player_icon`.
  No `SAVE_FORMAT_VERSION` movement.
- **`Profile::load` discards the entire profile when it cannot parse** —
  achievements included. The new field must be inert-safe for the same reason
  `seen_notifications` holds plain strings and `player_icon` holds an encoded
  string.

---

## 9. Seams touched

Each of these needs the standard three writes — the argument to
`docs/seams.md`, the trap to the `seams` skill, the one-sentence rule to
`CLAUDE.md`:

1. **There are two combat models, and `ability_recipients` is the one door
   they share.** (new)
2. **`Game::start_battle` is where the model is chosen, by inspecting the
   pack** — which is also why the arena stays abstract without code. (new)
3. **A battle map's coordinates live in `TacticalBattle`; `Position` is never
   written** — the third instance of the rule the Stack and base space
   already settled. (new)
4. **`walk_field`'s step rule is a cost function, not a predicate.**
   (amends an existing rule)

---

## 10. Accepted drawbacks

**`balance_sim` cannot see this mode.** It is the balance regression gate and
it models `attackers_in_group`, `slot_aggro_weight` and `expected_damage` —
group-model throughput. Tactical throughput differs in ways it structurally
cannot reach: turns spent moving rather than swinging, range gating, friendly
fire, and two large ones — **forced focus fire disappears** (today only a
group's front can be hit; on a grid damage spreads across every body) and
**`ENGAGED_GROUPS = 2` disappears** (today a group at index >= 2 cannot act
at all unless `ranged`; on a grid everything walks up and swings). The
expectation is that tactical fights hit harder and are swingier than the same
pack abstract. Nothing in the repo can confirm that before the mode is
played. This is accepted: the mode is opt-in and explicitly allowed to be
less tuned.

**Three combat models on one surface.** A pack is tactical; a nest, a lair
and a guardian chase are not. Being caught by a pursuing guardian and by a
town patrol are the same code path and will open different screens. This is a
staging decision and should be stated in the toggle's own description rather
than discovered.

**Content debt.** All shipped abilities get *derived* shapes. They work; none
is *designed* for a grid. Authoring real shapes for the shipped set is a
content pass that is not part of this work.

**Micro-fatigue** at full `MAX_PACK_BODIES`, even with the three pacing keys.

**No playtesting during implementation.** There is no display in the agent
environment. Everything ships unseen until played, which makes the
`dev-saves` template — a save parked next to a pack with the toggle on — a
requirement rather than a nicety.

---

## 11. Verification

Engine unit tests over the tactical module: map generation determinism, reach
fields including `Rough` costs, shape resolution against `Cover`, deployment
bearings and bounds, initiative ordering, leash-edge departure, and the
`AbilityTarget` -> shape census.

A **tactical arena** — `dev-arenas/*.ron` extended with placement, run
headlessly through the existing `arena` binary — is the only way to watch a
tactical fight resolve without a display. It is deferred until the mode
works, by decision.

---

## 12. Phasing

Two or more crates and a schema change, so this takes the full
spec-and-plan pipeline rather than inline TDD.

1. Options screen (prerequisite; lands on its own)
2. `tactical/` module: `BattleCell`, generator, `TacticalBattle`, deployment
3. `walk_field` cost widening + the tactical reach field
4. Turn model: initiative, move-then-act, teardown parity
5. Targeting: `shape`/`range` fields, derived defaults, census, the tactical
   arm of `ability_recipients`, friendly fire
6. `tactical/ai.rs`
7. Presentation: app-core modes and key handlers, battle tiles in `map_pane`,
   turn strip, action bar, cell cursor
8. Pacing keys and the `dev-saves` template
9. *(deferred)* tactical arena

Scope is comparable to the Stack or settlements.

---

## 13. Facts this design rests on

Confirmed directly against source while writing this:

- `walk_field` — `crates/engine/src/game/pursuit.rs:37`; space-agnostic,
  uniform cost `1u32`, callers at `pursuit.rs:82`, `caravan.rs:856`,
  `base_space.rs:59`, `base/hauling.rs:295,341`.
- `start_battle` sites in `game/turn.rs`: 480 and 523 (pursuit — guardians
  and patrols share both), 632 (bumping a creature), 933 (ambush), 1221
  (rest interrupt).
- `achievements::Profile` — `crates/engine/src/achievements.rs:236`;
  `load` discards the whole profile on a parse failure.

Reported by survey and worth re-confirming before the code is written:

- `BattleState` — `resources.rs:974`; `ability_recipients` —
  `combat_round.rs:906`; `use_ability` — `combat_round.rs:1003`;
  `ENGAGED_GROUPS = 2` — `tuning.rs:706`; `Position` read in combat only by
  `gather_pack` — `game/combat.rs:156`.
- `ALL_MODES` — `crates/gui/src/render/mod.rs:1402`, a test fixture, not a
  runtime dispatch table; draw dispatch at `mod.rs:570`.
