# Tactical squads — implementation plan

> **For agentic workers:** one task per dispatch, TDD, a commit per green
> step. Read the spec beside this file; the plan argues from it and does not
> restate it.

**Goal:** five wild programs of one species fold into one 2x2 body with a
combined stat block, two actions a turn and a `^` mark, on battle maps only.

**Spec:** `docs/superpowers/specs/2026-09-16-tactical-squads-design.md`

**Crates:** `feral-processes-engine`, `feral-processes-gui`, plus one
`dev-arenas/` scenario. **No save or schema change** — a squad entity is
never `Tamed` and holds no world `Position`, which is what keeps it out of
`save.rs`, the same omission that keeps a summon out.

## Global constraints

- `FORMATIONS` is difficulty tuning, so it lives in
  `crates/engine/src/tuning.rs` as a `pub const`, not in `assets/`
  (CLAUDE.md's moddability rule draws that line explicitly).
- A single body is **not** a formation row. Every reader treats the absence
  of `components::Squad` as footprint 1, actions 1, swing share 1.
- `tactical/` must not import `crate::components::Position`. That omission is
  the whole enforcement of the battle-map coordinate seam.
- `Game::apply_damage` stays the only path that damages a creature. Nothing
  in this feature writes a member's `Stats::hp` to express damage.
- Every rule below must be seen to fail with its rule deleted. A test that
  passes against the mutation is not coverage.

## Gates

Per task: `cargo test -p feral-processes-engine <name>`, then
`cargo clippy --workspace --all-targets` and `cargo fmt`.
Per phase: `cargo test --workspace`.
`cargo test -p feral-processes-engine balance_sim` after any `tuning.rs`
edit — a moved curve is the signal, not a broken test.

## What the ground looks like today

Verified by census on 2026-09-17, not remembered.

- `TacticalBattle::bodies: Vec<(Entity, (i32, i32))>` — one anchor per body,
  and the assumption every reader below inherits.
- `TacticalBattle::cell_of(body) -> Option<(i32, i32)>`,
  `occupant(cell) -> Option<Entity>`, `place`, `move_to`, `remove`,
  `bodies() -> impl Iterator<Item = (Entity, (i32, i32))>` — all in
  `crates/engine/src/tactical/mod.rs`.
- `TacticalBattle::acted: bool`, read through `acted()` and written through
  `mark_acted()`, cleared in `begin_turn`.

---

## Task 1: footprint geometry, with every body still one cell

**Deliverable:** `cells_of` and `gap` exist and every reader in the census
below calls one of them. Behaviour is unchanged, because every body's
footprint is still 1. **The existing suite must stay green with no test
edits** — that is this task's real gate, and a test that needed editing
means a reader changed meaning rather than moving.

**Files:**
- Modify: `crates/engine/src/tactical/mod.rs` — add `cells_of`, keep
  `bodies` as it is (an anchor list; the footprint is looked up per body).
- Modify: `crates/engine/src/tactical/reach.rs` — add `gap`; convert
  `movement_field`, `swing_reaches`, `in_range`, `cover_between`,
  `line_of_sight`'s callers, `shape_cells`, `recipients`/`recipients_from`,
  `path_to`.
- Modify: `crates/engine/src/tactical/turn.rs`, `ai.rs`, `view.rs`,
  `deploy.rs`, `map.rs`.
- Modify: `crates/engine/src/game/combat_damage.rs`, `game/tamper.rs`,
  `game/combat.rs`, `game/combat_round.rs`.
- Test: `crates/engine/src/tests/tactical.rs`.

**Interfaces produced:**

```rust
// TacticalBattle
pub fn cells_of(&self, body: Entity) -> Vec<(i32, i32)>;
pub fn footprint_of(&self, body: Entity) -> u8;   // 1 without `Squad`

// reach
pub fn gap(a: &[(i32, i32)], b: &[(i32, i32)]) -> u32;
```

`cells_of` needs the formation, which lives on a component, so it takes the
world — decide the shape at implementation time between a method on `Game`
and a `&World` parameter, and say which in the commit message. The anchor
stays top-left and `bodies` keeps its type; nothing stores four cells.

**The reader table.** Each of these becomes a call. Line numbers are from
the census and will drift — find the function, not the line.

| reader | file | the new rule |
|---|---|---|
| `occupant(cell)` | `mod.rs:225` | the body whose *footprint* contains the cell |
| `place` / `move_to` refusals | `mod.rs:207,233` | refuse if any footprint cell is taken |
| `movement_field`'s `occupied` set | `reach.rs:81` | every cell of every *other* body is a wall; the anchor is legal only if all its footprint cells are walkable and free |
| `distance` | `reach.rs:176` | callers measure with `gap`; `distance` itself stays cell-to-cell |
| `swing_reaches` | `reach.rs:115` | `gap` for the range test, sight from any cell to any cell |
| `in_range` | `reach.rs:181` | `gap` |
| `cover_between` | `reach.rs:214` | measured against the defender's nearest footprint cell |
| `line_of_sight` callers | `reach.rs:234` and six sites | sight exists if any cell of one footprint sees any cell of the other; the function itself stays cell-to-cell |
| `shape_cells` | `reach.rs:287` | a `Line`/`Cone` is anchored on the invoker's nearest cell to the aim; a `Radius` is unchanged |
| `recipients_from` | `reach.rs:368` | a body is caught if **any** of its cells is covered, and caught **once** |
| `path_to` | `reach.rs:138` | unchanged — it descends a cost field of anchors |
| `tactical_step` bump | `turn.rs:196` | a step into **any** cell of a hostile is a swing at it |
| board-edge departure | `turn.rs:183` | a body departs if **any** footprint cell leaves the board |
| `tactical_attack` | `turn.rs:444` | `gap` for range, footprint-to-footprint sight |
| `reactors` / `provoke` | `turn.rs:295,373` | `gap` |
| `best_aim`/`best_swing`/`cell_merit` | `ai.rs:1004,1094,167` | `gap`; a candidate anchor is legal only if its whole footprint is |
| `tactical_sides` | `ai.rs:705` | unchanged (discards the cell) |
| `hallucination_cells` | `tamper.rs:106` | a decoy may not land on any occupied cell |
| `deploy::plan` / `rank` / `nearest_free` | `deploy.rs:48,64,95` | seats a body needing a clear NxN block |
| `BattleSpec::bodies` | `map.rs:70` | counts a body as `footprint^2` when sizing the board |
| `TacticalBody::cell` + `footprint` | `view.rs:42,460` | the anchor plus the side |

**Steps:**

1. Write `cells_of`, `footprint_of` and `gap` with tests that pin them for a
   footprint of 1 and of 2 — `gap` of two adjacent 2x2 blocks is 1, of a 2x2
   and a single cell diagonally touching it is 1, of a body with itself is 0.
2. Run them; they fail to compile.
3. Convert the readers table above, one commit per file.
4. `cargo test --workspace` green **with no test file edited**. If a test
   needed an edit, stop and report which and why.
5. Add a test that two bodies cannot be placed overlapping, and one that a
   blast covering two cells of one footprint hits it once. Both must fail
   with the rule deleted.

---

## Task 2: actions per turn

**Deliverable:** `acted: bool` becomes `actions_left: u8`. A body with no
`Squad` still gets exactly one action, so nothing observable changes.

**Files:**
- Modify: `crates/engine/src/tactical/mod.rs` — the field, `acted()` →
  `actions_left()`, `mark_acted()` → spend one.
- Modify: `crates/engine/src/tactical/turn.rs` (`hand_on_turn`,
  `tactical_attack`, `tactical_use_routine`, `run_tactical_routine`),
  `ai.rs`, `view.rs`, `game/tamper.rs`.
- Test: `crates/engine/src/tests/tactical.rs`, `tests/tamper.rs`.

**Interfaces produced:**

```rust
// TacticalBattle
pub fn actions_left(&self) -> u8;
pub fn spend_action(&mut self);
// Game
pub fn actions_per_turn(&self, body: Entity) -> u8;   // formation's, or 1
```

`begin_turn` sets `actions_left` from `actions_per_turn`.

**The rules, each its own test:**

- `hand_on_turn` hands the turn on **only** when `actions_left` reaches
  zero. The existing guard — that the body which acted is still the one
  acting — stays, and the new one sits inside `hand_on_turn` beside it. The
  per-action `mark_acted`/`hand_on_turn(actor, round_before)` call pairs stay
  where they are; do not hoist them.
- `tactical_attack`'s refusal reads `actions_left == 0` where it read
  `acted`.
- The movement allowance is untouched: a squad walks once a turn, however
  many actions it has.
- After the first action `run_tactical_beat` clears `walk` and plans again
  rather than ending the turn. `walk` is an `Option`, and `None` means "has
  not chosen yet" — clearing it to `None` is the point.
- Things that tick on hand-on still tick **once** a turn: tamper ageing
  (`game/tamper.rs`) and decoy settling. Cooldowns arm per action, so one
  routine cannot fire twice in a turn.
- The player's party always gets 1.

**The trap to hold a test against:** a body killed by its own fumble or its
own blast has already left the order, and `TacticalBattle::remove` handed
the turn on as it went. `actions_left` must not resurrect its turn. Test a
squad that fumbles fatally on its first of two actions.

---

## Task 3: the `effective_atk` door

**The spec is stale here too, in the cheap direction.** It says "if today's
damage paths read `Stats::atk` in more than one place, the first step is to
extract that door". They do not. **`Game::effective_atk(entity) -> i32`
already exists** at `crates/engine/src/game/combat_round.rs:1744`, is
`pub(crate)`, and its doc comment already calls itself the door "for damage
purposes". It folds base `atk`, `CombatBuff::Atk`, `FieldBuffKind::Atk`,
and for the player `wielded_stat_bonus` and
`battle::power_attack_multiplier`. **There is no extraction to do.** This
task is one arm inside a function that already exists.

**Files:**
- Modify: `crates/engine/src/game/combat_round.rs` — the `Squad` arm.
- Test: `crates/engine/src/tests/tactical.rs`.

**The rule:** a squad's damage is `Stats::atk * hp / max_hp`, so it weakens
as it is worn down, the way a pack thins when bodies drop. A body with no
`Squad` is untouched.

**What the door already covers**, verified by census — add the arm and all
of these inherit it:
- `combat_damage.rs:161` `combatant_profile`, the **one** production site
  that builds a `battle::Combatant`, so `resolve_attack`,
  `resolve_free_attack` and `resolve_and_apply_attack` all flow through it;
- every `use_ability` `Damage` and `Drain` effect, which call
  `resolve_and_apply_attack`;
- `combat.rs:195` `swing_damage` (`attack_nest`, `strike_rock`);
- `ai.rs:899` `expected_reaction_damage`, the AI's walk-risk forecast **and**
  the gui's movement telegraph — so the telegraph and the roll cannot
  disagree, which is the failure this task exists to prevent.

**The one path it does not reach is `balance_sim.rs`**, which builds
synthetic `Stats`/`Fighter` blocks with no `Entity` to ask. That is correct
and needs no work: `balance_sim` models no battle maps, so a squad never
appears in it. Do not thread a squad term into the sim.

**Deliberately left raw**, and not to be "fixed" into calls: `Stats::power()`
(a rating, feeding `difficulty_color` and `kill_xp`), `copy_power` /
`copy_bonus` (rated against a synthetic reference wearer), and the hostile
roster row `enemy_row` / `program_manifest`, whose asymmetry with the
player's `effective_atk` is documented at `views.rs:2168`. A squad's UI row
showing its raw summed `atk` while it swings for less is the existing
convention, not a bug.

---

## Task 4: formations, spawning, death, capture, disbanding

**Deliverable:** squads actually form and fight. This is the largest task
and the one carrying every new rule.

**Files:**
- Modify: `crates/engine/src/tuning.rs` — `Formation` and `FORMATIONS`.
- Create: `crates/engine/src/tactical/squads.rs` — `plan`.
- Modify: `crates/engine/src/components.rs` — `Squad`.
- Modify: `crates/engine/src/game/` — `spawn_squad`, `disband_squad`.
- Modify: `crates/engine/src/tactical/turn.rs` — `open_tactical_battle_at`,
  `reap_tactical_dead`, `settle_tactical`, the capture arm.
- Modify: `crates/engine/src/arena/` — `stage` calls the same `plan`.
- Test: `crates/engine/src/tests/tactical.rs`.

**Interfaces produced:**

```rust
// tuning
pub struct Formation { members, footprint, actions, swing_share, noun, mark }
pub const FORMATIONS: &[Formation];

// tactical::squads
pub fn plan(pack: &[Entity], world: &World) -> Vec<Piece>;   // pure, no RNG

// components
pub struct Squad { pub members: Vec<Entity>, pub formation: usize }

// Game
pub fn spawn_squad(&mut self, members: &[Entity], formation: usize) -> Entity;
pub fn disband_squad(&mut self, squad: Entity);
```

**`squads::plan`** — pure, draws no RNG. Per species, in the pack's order,
take `FORMATIONS` largest-first and cut `count / members` sets, leaving the
remainder as single bodies. Each set's **lead** is the body the player
bumped if the set contains it, otherwise the first member in pack order.
It runs inside `open_tactical_battle_at` **before** `BattleSpec` sizes the
board. Bosses and nest guardians never reach a battle map, so no exclusion
is needed.

**`spawn_squad` is the one place a squad's component list is written** —
`spawn_structure`'s rule. The stat block is the spec's table: summed
`max_hp`/`hp`, summed `atk` times `swing_share`, the members' highest
mitigation, highest level and highest `Rarity`, and the lead's routines and
cooldowns. **No world `Position` and no `Tamed`.** Members keep their own
`Position` and `Stats` and are absent from the board's piece list, which is
what keeps them from being targeted, drawn or given a turn.

**The `Position`-less squad is safe inside `tactical/` and unsafe at the
door.** Census confirms `crates/engine/src/tactical/` contains **zero**
reads of `Position` — every in-battle reader works off `TacticalBattle`'s
own `bodies`, and `view.rs`'s `body_view`/`turn_row` read every component
through `.get()` with a graceful default, never a panic. `Tamed` is read
nowhere on the tactical path at all; side is decided by `Game::is_hostile`,
which asks only for `Hostile`. **The two dangerous sites are both at the
moment the fight opens, and both degrade silently rather than crashing:**

- `Game::gather_pack(anchor)` (`game/combat.rs:269`) returns a pack of
  **exactly one** if the anchor has no `Position`. A squad used as an anchor
  reads as a lone-body encounter, with no error.
- `Game::open_tactical_battle(pack)` (`tactical/turn.rs:49`) derives its
  bearing from `pack[0]`'s `Position`, falling back to the player's own tile
  — a degenerate zero vector and a wrong-facing opening formation, again
  silently.

Neither can fire if `squads::plan` runs **inside `open_tactical_battle_at`,
after the bearing is derived from the original pack** — which is where the
spec puts it, and now there is a reason in the plan for why it goes there
and not one call earlier. `arena::stage` already passes its bearing
explicitly and is safe by construction. **Do not fold a pack before its
bearing is taken.** A test must assert the opening bearing is unchanged by
folding.

Give the squad `Creature` as well as the components in the table:
`group_pack` silently drops a member with no `Creature`, and while a squad
should never reach the group model, an entity that would vanish there is a
trap for whoever later wires one up.

**Death pays each member's own payout.** `reap_tactical_dead` calls
`finish_hostile` for **each remaining member**, never for the squad entity,
then despawns the squad. `overkill_term` per member reads the squad's
overkill shared evenly, routed through a **parameter** — writing a member's
`Stats` to express it would put a second damage path beside
`Game::apply_damage`.

**A capture yields one member.** `decompile_body` aimed at a squad rolls
`capture_chance` as though for the lead at the squad's Integrity fraction.
On success the lead leaves `Squad::members` and is captured as an ordinary
program at that same fraction; the squad takes
`max_hp / members_at_formation` damage **through `apply_damage`** and
continues. `members` running out is a second way a squad dies, so a squad
supplies at most five captures. If the capture damage drops it to zero, the
remaining members pay out as above. Capture stays refused at the player's
door, where a swing at your own is friendly fire and stays legal.

**A surviving squad disbands.** If the fight ends with a squad standing —
the player jacks out, loses, or the squad departs the board —
`disband_squad` sets each remaining member's `hp` to its own `max_hp` times
the squad's Integrity fraction and despawns the squad. **The world outside a
fight never contains a squad.** This is not damage (no member is being hit),
so it does not go through `apply_damage`; say so in a doc comment, because
the next reader will ask.

A squad keeps its formation to the end: 2x2, marked, two actions, whatever
its Integrity.

**Tests, each seen to fail with its rule deleted:**

- `plan` on 9 of a species (one squad, four singles), 10 (two squads), 4
  (no squad), and a mixed pack; the bumped body leads its set.
- A squad's stats are the sums and maxima above.
- **A squad is not saved**, and its members are unchanged, across a save
  made mid-fight. (`#[serde(skip)]` cannot be caught by a RON round trip —
  this needs a real save→load test.)
- Two actions, then the turn is handed on; a single body still gets one.
- Attack at half Integrity is half of full.
- Movement refuses an anchor whose footprint overlaps a wall or another
  body; a blast covering one footprint cell hits once.
- Death pays five kills' worth of XP and loot.
- A capture yields one member and removes a fifth of `max_hp`; the sixth
  capture attempt cannot happen.
- A surviving squad disbands into members at its Integrity fraction.

---

## Task 5: drawing

**Files:**
- Modify: `crates/engine/src/tactical/view.rs` — `TacticalBody` gains
  `footprint: u8` and `squad: Option<SquadView>`.
- Modify: `crates/gui/src/render/tactical.rs` — `draw_body`, the washes,
  the turn arrow, the aim cursor, `acting_body`.
- Modify: `crates/gui/src/render/marks.rs` — `squad_mark_rect`.
- Modify: `crates/gui/src/fx.rs` — `battle_center` centres on the footprint.
- Test: the gui test module in `render/tactical.rs`.

**Interfaces produced:**

```rust
pub struct SquadView { pub members: usize, pub mark: char, pub noun: &'static str }
```

- `draw_body` draws over the whole footprint rectangle; the glyph or sprite
  scales with it. **A sprite substitutes for the glyph and never draws
  beside it** — the overdraw trap in CLAUDE.md's drawing seam applies
  unchanged at 2x2, so the test asserts the mesh *and* the absent glyph.
- **The mark goes in the bottom-right corner of the footprint, not the
  top-right.** The spec says top-right and **the spec is stale**: it was
  written before cover shipped, and `draw_body` now spends the top-right on
  the in-cover mark, whose doc comment says in as many words that the battle
  map alone leaves that corner free. It does not any more. Every other
  channel on a battle-map tile is spoken for — the rarity bar owns the top
  edge, the con earmark the top-left, cover the top-right, the HP bar the
  bottom edge — so the squad mark takes the **bottom-right**, which is the
  surface map's own convention for the one corner nothing else claims
  (`patrol_mark_rect`), and it **lifts above the HP bar** the way the
  top-corner marks drop below the rarity bar. `squad_mark_rect` goes in
  `render/marks.rs` beside the other four and carries that reasoning in its
  doc comment. A test must place a squad that is *also* in cover and assert
  both marks are drawn.
- The HP bar is the squad's own, so gui sums nothing. It spans the
  footprint's width.
- The turn strip, forecast and examine line name it `"<species> squad (5)"`,
  **built in the engine** — a name built in a renderer is how a drop line
  and the next screen come to disagree.
- The turn arrow and aim cursor sit on the footprint and stay
  bounds-checked; the battle map passes no lag clamp.
- `draw_cell_field`'s boundary logic tests single-cell adjacency; a
  footprint's washed cells must read as one region.

---

## Task 6: the arena scenario and the measurement

**Files:**
- Create: `dev-arenas/squad.ron` — five of one species against a mid-grade
  party, `model: Tactical`.
- Create: `dev-arenas/squad-control.ron` — four of that species plus one of
  another, which does not fold. Same party, same seed.
- Create: `docs/measurements/2026-09-17-tactical-squads.md`.
- Modify: `dev-arenas/README.md` if a new row is needed.

`balance_sim` models no battle maps, so the arena is the only instrument.
Comparing the two win rates and fight lengths is what `swing_share` will be
retuned against, and the spec expects it to move. Record the commands, the
numbers and what the run was blind to, per `docs/measurements/README.md`.

**Arena numbers compare within one build only** — a moved baseline is a
reshuffled RNG stream, not a difficulty change. Run both scenarios in the
same build.

---

## Landing

- `CHANGELOG.md` section and the workspace version bump happen **at the
  merge**, not on the branch.
- Three seam writes if this adds a seam, in order: the argument to the
  memory graph, the trap to `.claude/skills/seams/references/combat.md`,
  the one-sentence rule to `CLAUDE.md`. The candidates are the formation
  table, the footprint doors and the two-action turn.
- Archive the spec on landing rather than by a later sweep, and add its row
  to `docs/superpowers/INDEX.md`.
- Delete this plan at the landing. A plan lives exactly as long as the work
  it directs.
