# Tactical squads: five of a kind fight as one

**Status:** design approved, not implemented. TODO #99.

On a battle map, every five wild programs of one species fold into a
**squad**: one program with a combined stat block, filling a 2x2 block of
cells, marked `^`, taking up to two actions a turn. Mechanically it is a
single enhanced entity. "Squad" is what the prose calls it.

Nine of a species make one squad and four single bodies. Ten make two squads.

Battle maps only. The abstract group model already fights a pack as groups
and is not touched. There is no save or schema change.

## Formations are a table

The squad is the first row of a table of formation sizes, not a special case,
so a larger set later is another row rather than a second code path.

```rust
// tuning.rs
pub struct Formation {
    pub members: usize,      // exact set size that folds into one
    pub footprint: u8,       // side of the square it occupies, in cells
    pub actions: u8,         // actions per turn
    pub swing_share: f32,    // fraction of the members' summed atk it swings with
    pub noun: &'static str,  // "squad"
    pub mark: char,          // '^'
}

pub const FORMATIONS: &[Formation] = &[Formation {
    members: 5, footprint: 2, actions: 2, swing_share: 0.5,
    noun: "squad", mark: '^',
}];
```

This is difficulty tuning, so it is code, per CLAUDE.md's moddability rule.
`swing_share` is expected to move once squads have been played. The
single-body case is *not* a row: an ordinary body has no `Squad` component
and every reader treats its absence as one cell, one action, a share of 1.

## Forming squads

`tactical::squads::plan(pack) -> Vec<Piece>` is pure and draws no RNG. For
each species, in the pack's order, it takes `FORMATIONS` largest-first and
cuts `count / members` sets of that size, leaving the remainder as single
bodies. Each set's **lead** is the body the player bumped if the set contains
it, otherwise its first member in pack order.

It runs inside `open_tactical_battle_at`, before `BattleSpec` sizes the board,
and `arena::stage` calls the same function so a scenario can field squads.
Bosses and nest guardians never reach a battle map, so no exclusion is
needed there.

## What a squad is

**A freshly spawned wild entity of the members' species, and the members wait
off the board untouched.**

`Game::spawn_squad(members, formation)` is the one place a squad's component
list is written (`spawn_structure`'s rule). It carries:

| component | value |
|---|---|
| species, `Hostile` | the members' species |
| `Stats::max_hp`, `Stats::hp` | sums over the members |
| `Stats::atk` | summed `atk` times `swing_share` |
| `Stats::mitigation` | the members' highest |
| level, `Rarity` | the members' highest |
| routines and cooldowns | the lead's; members share a species, so they know the same routines |
| `components::Squad` | `{ members: Vec<Entity>, formation: usize }` |

It has **no world `Position`**, since a battle map keeps its own coordinates,
and **no `Tamed`**, so the save never writes it. That is the same omission
that keeps a summon out of the save. The members keep their `Position` and
`Stats` and are removed from the board's piece list, which is what keeps them
from being targeted, drawn or given a turn.

The plan's first task is a census of every reader that assumes a hostile on a
battle map has a world `Position` or the other components the wild spawner
writes. A reader that silently misses one is the likely failure.

## Attack falls with Integrity

A squad's damage is `Stats::atk * hp / max_hp`, so it weakens as it is worn
down, just as a pack thins when bodies drop one at a time.

This scaling goes in **one door** that every damage path reads for the
attacker's attack: `Game::effective_atk(attacker)`. It covers a swing, a
routine that scales off `atk`, and anything else. If today's damage paths
read `Stats::atk` in more than one place, the first step is to extract that
door and make each of them a call to it. A scale applied at only some of those
sites would compile and quietly miss the others.

At `swing_share = 0.5` and two actions, a fresh squad deals per turn what its
five members would deal fighting alone. The remaining differences are:

- two large hits lose a smaller share to mitigation than many small ones;
- the damage lands on one target;
- a squad's Integrity is one pool, so nothing is wasted on overkill between
  members.

## Two actions a turn

`TacticalBattle::acted: bool` becomes `actions_left: u8`, set in `begin_turn`
from `Game::actions_per_turn(body)`, which is the formation's `actions` or 1.

- An action spends one. `hand_on_turn` hands the turn on only when
  `actions_left` reaches zero; the per-action `mark_acted` and
  `hand_on_turn(actor, round_before)` calls stay where they are, and the
  guard sits inside `hand_on_turn`.
- `tactical_attack`'s refusal reads `actions_left == 0` in place of `acted`.
- The movement allowance is untouched, so a squad walks once a turn.
- After the first action, `run_tactical_beat` clears `walk` and plans again
  rather than ending the turn.
- Things that tick on hand-on still tick once a turn: tamper ageing and decoy
  settling. Cooldowns are armed per action, so one routine cannot fire twice.
- The player's party always gets 1, so nothing changes for the keyboard.

## The 2x2 footprint

`TacticalBattle::bodies` keeps its one **anchor** cell per body (top-left).
The footprint comes from the body's formation. Two doors replace every
one-cell assumption:

- `TacticalBattle::cells_of(body)` returns every cell the body covers.
- `reach::gap(a, b)` returns the shortest distance between two footprints.

Every current reader of `cell_of` becomes a call to one of them:

| reader | rule |
|---|---|
| `occupant(cell)` | the body whose footprint contains the cell |
| `movement_field` / `walk_field` | the cost function answers for the anchor and refuses unless all footprint cells are walkable and free of *other* bodies |
| `distance`, `swing_reaches`, `tactical_attack` | measured with `gap` |
| `line_of_sight` | sight exists if any cell of one footprint sees any cell of the other |
| `shape_cells` / `recipients_from` | a body is hit if any of its cells is covered, and hit once |
| `best_aim`, `best_swing`, `path_to` | aim at and path to the nearest footprint cell |
| `deploy::plan` | finds a clear 2x2 block; `BattleSpec` counts a squad as four bodies when sizing the board |
| a step off the board edge | a squad departs if any footprint cell leaves the board |
| the player walking into a hostile | a bump into any cell of a squad is a swing at it |

The step rule stays a cost function (CLAUDE.md, "there is one Dijkstra walk
on the surface"); only the question it asks for a cell changes.

A squad keeps its formation to the end. It stays 2x2, marked, with two
actions, whatever its Integrity.

## Death, rewards and capture

**Death pays each member's own payout.** When a squad reaches zero,
`reap_tactical_dead` calls `finish_hostile` for **each remaining member**,
not for the squad entity, and then despawns the squad. XP, loot, a patrol
kill's standing charge, and every other per-body consequence are exactly
those of five separate kills. `overkill_term` for each member reads the
squad's overkill, shared evenly. The plan must route that through a
parameter rather than writing a member's `Stats`, because
`Game::apply_damage` is the only path that damages a creature.

**A capture yields one member.** `decompile_body` aimed at a squad rolls
`capture_chance` as though for its lead at the squad's Integrity fraction. On
success:

- the lead is removed from `Squad::members` and captured as an ordinary
  program, at that same Integrity fraction;
- the squad takes `max_hp / members_at_formation` damage through
  `apply_damage`, and continues;
- `Squad::members` running out is a second way a squad dies, so a squad
  supplies at most `members` captures. That is five for a squad.

If the capture damage drops the squad to zero, the remaining members pay out
as above.

Capturing is refused at the player's door, as it is today. It aims at
something hostile.

## When a squad survives the fight

If the fight ends with a squad still standing (the player jacks out, loses,
or the squad departs the board), `Game::disband_squad` sets each remaining
member's `hp` to its own `max_hp` times the squad's Integrity fraction and
despawns the squad entity. The world outside a fight never contains a squad.

Setting a surviving member's `hp` is not damage, since no member is being
hit. The plan should still check whether `apply_damage`'s seam wants it to
go through a named door.

## Drawing

`TacticalBody` gains `footprint: u8` and `squad: Option<SquadView>`, where
`SquadView` holds `{ members, mark, noun }`.

- `draw_body` draws over the whole footprint rectangle, and the glyph or
  sprite scales with it.
- The mark goes in the top-right corner through a new
  `marks::squad_mark_rect`. The battle map draws nothing there today.
- The HP bar is the squad's own, so no summing happens in gui.
- The turn strip, forecast and examine line name it
  `"<species> squad (5)"`, built in the engine.
- The turn arrow and aim cursor sit on the footprint, and remain
  bounds-checked.

## Balance measurement

`balance_sim` models no battle maps, so the arena is the instrument. A new
`dev-arenas/squad.ron` fields five of one species against a mid-grade party,
alongside a copy with four of the species plus one of another, which does
not fold. Comparing the two win rates and fight lengths measures what the
formation is worth. Record the result in `docs/measurements/`, because
`swing_share` will be retuned against it.

## Delivery

1. **Engine: footprint geometry.** Add `cells_of`, `gap` and the reader
   table above, with every body still one cell. The existing suite must stay
   green without any test edits.
2. **Engine: actions.** Replace `acted` with `actions_left` and add
   `actions_per_turn`. Unchanged behaviour for a count of 1.
3. **Engine: the `effective_atk` door**, extracted if needed.
4. **Engine: formations.** `FORMATIONS`, `squads::plan`, `spawn_squad`,
   reaping, capture, disbanding, and the component census.
5. **gui: drawing**, plus the engine-built names.
6. **Arena scenario and measurement.**

Crates touched: engine and gui, plus one `dev-arenas/` scenario file.

## Tests that carry the rules

- `plan` on 9, 10 and 4 of a species, and on mixed packs; the bumped body
  leads.
- A squad's stats are the sums and maxima above.
- A squad is not saved, and its members are unchanged, across a save made
  mid-fight.
- Two actions, then the turn is handed on; a single body still gets one.
- Attack at half Integrity is half of full.
- Movement refuses an anchor whose footprint overlaps a wall or another body;
  a blast covering one footprint cell hits once.
- Death pays five kills' worth of XP and loot; a capture yields one member
  and removes a fifth of `max_hp`; the sixth capture attempt cannot happen.
- A surviving squad disbands into members at its Integrity fraction.
- gui: the mark is drawn, and the body fills the footprint.

Each rule's test must be seen to fail with its rule deleted.
