# Siege — design

**Status:** implemented, feat/siege, 2026-09-22.

A siege is a new event: a pack of the sector's wild programs walks in
through the base's one door, steals what it can carry, wrecks what it
cannot, and withdraws when enough of it is down. It is fought on a
tactical board built from the base itself — every cut cell, every
machine, every body standing in it.

It is **separate from the GC Entropy Sweep**. The sweep shipped its own
clock in 0.13.222 and is untouched by this: same interval, same warning,
same abstract damage, same hostile-town raiders. The two events share
nothing but the shape of their clock and the `raid_defense` a structure
contributes, which a turret keeps paying into.

---

## Decisions taken in dialog

Each of these was chosen by the user against alternatives; the reasoning
is recorded because the alternative is what a later reader will think of
first.

1. **The whole base is the board**, not a window onto it. A window was
   recommended and overruled. The payoff is that where you dug and where
   you built *is* the battlefield; the cost is a board with no fixed
   size, which §4 bounds by reachability rather than by a constant.
2. **A siege is always tactical**, regardless of the `tactical_battles`
   profile toggle. A player who has turned battle maps off still fights
   this one on a map.
3. **All four stakes are live**: machine durability, structures
   destroyed outright, staff killed, stores stolen.
4. **Turrets are a `StructureDef` field**, never a hardcoded kind, so a
   mod ships one as a file.
5. **Away from the base, a siege resolves abstractly** and reports. The
   warning is long enough to get home, within reason.
6. **Raiders come through the door only.** Rock is not breached.
7. **Everyone in base space is on the board.** Staff fight under the
   existing AI; the player commands themselves and their party.
8. **Raiders are the sector's wild programs**, scaled — no new species
   content.
9. **They steal, they wreck, and they withdraw on a morale break** — no
   quota and no round limit.
10. **The board is saveable.** A siege survives a quit.

---

## 1. The clock

Its own meter, built to the pattern `resources::RaidPressure` established
and **not** that resource: `SiegePressure { level, next_at, warned }`,
accruing per tick scaled by sector against a target drawn once per
interval with jitter on the target rather than on the accrual.

Three properties carry over from the sweep's clock because each closed a
real failure there, and all three apply identically here:

- **The clock is spent by the event, not by reaching its threshold.** A
  tick that decides a siege happens and then cannot stage one (no base,
  nothing standing, the player mid-fight elsewhere) must *hold* its
  pressure, or the meter rewinds on a no-op and the interval is never
  served.
- **A minimum-sector gate applies to accrual, not to firing.** Gating
  firing lets the opening sector bank pressure it cannot spend and
  ambushes the player the instant they cross into the sector that can.
- **Jitter is one draw on the target.** Per-tick jitter averages out to a
  metronome over the thousands of ticks an interval takes, and costs a
  draw a tick besides.

The interval is **much longer than the sweep's**. The sweep is 15–25
minutes in sector 2; a siege should be an event a player meets a handful
of times in a run, not a chore. Exact figures are a tuning question for
the plan, anchored the way the sweep's were: state the interval in
minutes at sector 2 and derive the constant from the tick rate, never the
other way round.

A siege must not open while any other fight is running. That is a hold,
not a skip.

## 2. The warning

Latched, one line, one attention row, exactly as the sweep's is — and
with one difference that is the whole reason this section exists.

**The warning window has a wall-clock floor.** The sweep warns at a
percentage of its drawn interval, so its window shrinks with depth along
with the interval. That is right for the sweep, whose answer is "post
someone at the machine you care about". It is wrong here, because the
player's answer to a siege is *travel home*, and the trip is longest in
exactly the sectors where a percentage window is shortest. So the siege
warns at whichever is earlier: a share of its interval, or a fixed number
of ticks before it lands.

The floor's value is a play question, not an arithmetic one, and nothing
in this repo can measure it: it has to cover a walk home from a deep
Stack frame. The plan should pick a defensible number, and the seam entry
should record that it is unvalidated until someone plays it.

## 3. Away from the base

A siege that fires while the player is elsewhere resolves abstractly and
is reported when they next reach the base.

This is **not** `run_raid`'s body. A sweep is one machine taking one hit;
an abstract siege has to price all four stakes, because a player who
missed one should find stores gone, machines broken and possibly staff
dead. It reads the same defence the tactical fight would — turrets,
posted staff, `total_raid_defense` — and pays out against them.

Two properties it must hold:

- **It reports as an event, not as a number.** The log should name what
  was taken and what was broken, the way a town raid names its author.
- **It is not cheaper than defending.** If skipping a siege is
  consistently better than fighting one, the feature is an incentive to
  leave home, which inverts the whole point.

## 4. The board

**Flood-fill from `BASE_EXIT_CELL` through walkable base-space cells; the
bounding box of that fill is the board.**

- `BaseGrid` solid rock is `BattleCell::Blocked`. Rock is never breached,
  so the fill's frontier is the board's wall.
- Laid floor is `Open`.
- Sealed-off pockets are not in the fill and so not in the fight — a
  player who walls off their far works has genuinely protected them.
- Structures stand on their anchors as obstacles that can be attacked
  (§6). A structure's footprint is a derivation on read, never stored,
  and the board must read it through the existing door rather than
  restating it.

`Board` is a square of `side` cells and is constructible cell by cell, so
a non-square fill is seated inside the bounding box with the remainder
`Blocked`. A `BattleSpec` is still minted for the fight, because
`TacticalBattle::open` takes one — but the board is **built**, not
`generate`d, and that is the one place a siege departs from every other
tactical fight.

**Raiders enter at the door.** `BASE_EXIT_CELL` is the single entrance —
`base_space.rs` says outright that base space has one door, not many —
and the Home stands on it. One body to a cell means they file in. The
fight is therefore about what covers the approach: the corridor you cut,
the turrets sighted down it, who you posted where. A second entrance is a
possible future structure and is out of scope here.

## 5. Who stands on it

Every body in base space, seated **where it already stands**. Nobody is
deployed; the base's arrangement at the moment the siege opens is the
opening position, which is the payoff for making the base the board.

- **The player and their party** are commanded as in any tactical fight.
- **Base staff** act under the AI that already drives a side, through the
  one door every tactical AI entry point reads. They are not commanded
  and there is no defence-roster screen.
- **Turrets** fire themselves (§7) and hold no initiative slot.

**The round-length problem, and the mitigation.** `TacticalBattle`
documents that a fight holds at most thirteen bodies and keeps its body
list as a linear-scanned `Vec` on that basis. A developed base holds a
hundred. The mitigation is that **a staff body with nothing in reach
takes no turn at all** — it is on the board, it can be attacked, it will
act the moment something is near it, but it does not consume a turn to
decide to stand still. The order thins to the bodies actually in contact.

This is a design constraint, not an optimisation: it is what makes
"everyone is on the board" and "a round you can sit through" both true.
It also has a visible consequence the plan must accept — a body four
rooms from the fighting does not walk toward it. Staff do not
reinforce; they defend where they are.

## 6. What raiders do

The pack is drawn from the sector's habitat species at the sector's
danger band — the same derivation a wild pack spawn already uses, with no
new species content. Pack size scales with sector.

**They steal and they wreck, opportunistically.** No quota, no objective
cell, no timer:

- A raider adjacent to a shelf or a machine's buffer takes what it can
  carry and makes for the door. Carrying is the hauler's one
  `(item, qty)` pair, reused — a raider is a body carrying something,
  and the cap is what keeps that one pair honest.
- A raider adjacent to a machine with nothing to take swings at it.
  Structure damage on the board goes through the existing damage door, so
  a structure destroyed in a siege is destroyed the way any other one is.
- A raider that reaches the door with cargo leaves, and what it carried
  is gone.
- **Killing a carrier before it reaches the door drops the goods**, which
  is what makes interception the shape of the fight rather than a race.

**They withdraw on a morale break**, when enough of the pack is down.
Withdrawal is a walk to the door and off the board — a step off the edge
is already a departure rather than a refusal, so a raider leaving is the
mechanism that already exists. A withdrawal is not a rout to chase: a
raider that leaves is gone, with whatever it was carrying.

The fight therefore ends by one of the endings the tactical model already
knows — the roster emptied, the party gone, or the player jacking out —
and its length tracks how fast the defence kills, which the user chose
over a quota with that trade-off stated.

## 7. Turrets

A `StructureDef` field, additive behind `serde(default)`, so a turret is
a `.ron` file and the engine names no turret id.

- **In a siege** it fires once a round at the nearest hostile in range
  with line of sight. It has no `Stats`, no initiative slot, and no body:
  it is a property of a structure, resolved at a point in the round.
- **Outside a siege** it is an ordinary structure and keeps contributing
  `raid_defense` to the sweep. That is the only place the two events
  touch, and it is deliberate: a fortified base is better against both.
- A turret's line of sight is the board's, checked through the existing
  door rather than a second copy of the test.

Whether a turret needs power, staffing or ammunition is a content
question for the def, not a rule in Rust.

## 8. Saving a siege

**A siege is the one tactical board that survives a save, and it costs no
save-format bump.**

The precedent is the sortie, and it is exact. `TacticalBattle` holds
`Entity` values, and entity ids are not stable across a save — which is
why a sortie's membership rides `CreatureSave::sortie_index` and
`SortieSave` carries no member list. A siege does the same:

- Each body's board cell and its place in the initiative order ride
  `CreatureSave`, additive behind `serde(default)`.
- A new `SiegeSave` holds only what is not per-body: the board, the
  spec, the round, the turn cursor, the pack's state.
- A save written before this feature loads as "no siege in progress",
  which is what that run is. No `SAVE_FORMAT_VERSION` move, no
  `dev-saves/` recapture.

**The invariant that must survive.** `TacticalBattle` is bare
`#[derive(Resource)]` with no `Serialize` *specifically* because a fight
is never saved. Making the siege the exception has to be written so that
an ordinary fight still cannot be saved — a `Serialize` bolted onto
`TacticalBattle` itself would quietly make every fight saveable and the
first symptom would be a Stack battle surviving a reload with its board
intact. The siege's save must be assembled from the board rather than
derived by serialising the resource.

## 9. Dev console

Two entries behind `FERAL_DEV_CONSOLE`, following exactly the rule the
sweep's pair already follows — **the console fires the real thing, never
a copy of it**, so what a developer sees is evidence about the siege a
player meets:

- **Fire a siege now**, calling the same function the clock calls.
- **Wind the siege clock to its warning**, setting the level as a share
  of the drawn target rather than to a literal, so one press works
  whatever the jitter rolled.

A `dev-saves/` template with a base worth besieging is the other half of
being able to test this at all, and the plan should capture one.

## 10. Vocabulary

The event is a **siege**. Nothing else in the game is called one, so the
bare word is unambiguous and no qualifier is needed.

"Sweep" stays the GC Entropy Sweep's word and is never used for this.
"Raid" remains the code's word for the sweep. The attacking bodies are
**raiders** only where that does not collide with the sweep's hostile
town raiders; prefer *besiegers* in player-facing text if a collision
appears in a real sentence.

---

## Risks and open questions

- **Raiders are a third kind of body carrying a base-space `Position`**,
  after a posted program and a `DigSite`. That seam already records that
  `Structure` no longer answers "which space is this?" on its own; a
  saveable siege sharpens it, because these bodies must survive a reload
  standing where they stood.
- **Round length is mitigated, not solved.** The no-turn-when-nothing-is-
  in-reach rule bounds a round to the bodies in contact, but a siege in a
  large base is still the longest fight the game asks anyone to sit
  through. It should be played before the numbers are trusted.
- **The warning floor is unmeasured.** Nothing in the repo models travel
  time home from a deep Stack frame.
- **`balance_sim` has no raid term and no siege term.** Every number in
  this feature is reasoned from the tick rate, not measured — the same
  blind spot the raid clock's seam already records.
- **Abstract resolution must not be the better option.** If missing a
  siege is cheaper than fighting one, the incentive is inverted. This is
  a play question and a tuning one.
- **A turret's power draw is undecided** and is deliberately left to the
  def.

## Testing and gates

- Engine unit tests for: the clock's three holds; the warning's floor and
  its latch; the flood-fill board (a sealed pocket is excluded, the door
  is the only entrance, rock is `Blocked`); a carrier killed before the
  door dropping its goods; a carrier reaching the door removing them; the
  morale break; a turret firing without holding a turn.
- A save→load round trip **with a siege in progress**, asserting board
  cells, body placement and initiative order survive — a RON round trip
  alone cannot catch a skipped field.
- A test that an ordinary tactical fight is still not saved.
- An abstract-resolution test per stake.
- `cargo test --workspace` is the final gate; `cargo clippy --workspace
  --all-targets` and `cargo fmt` after every change.
- Playtesting is the user's: agents cannot run the game here, and a green
  suite is not evidence of play.

## Out of scope

A second base entrance; breaching rock; a defence-roster screen;
authored GC species; sieges anywhere but the base; a siege the player can
decline.
