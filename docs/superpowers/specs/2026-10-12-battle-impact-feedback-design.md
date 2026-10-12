# Battle impact feedback

The group battle screen narrates a fight well but does not *show* one: a
blow lands as a line of text and a bar that steps. The floats and ghost
bars that exist are inferred from HP deltas between roster frames, which
cannot see a miss or a crit, misattribute a kill (the next member's HP
swaps in), re-key when a dead group re-letters the rest, merge a skipped
round into one number, and stack every float at one x. This change makes
each narrated line carry what it did, and draws that.

Scope is the group (non-tactical) battle screen only. Pacing, line count,
narration wording and balance are untouched.

## Decisions

| Question | Decision |
|---|---|
| Symptom | "Can't see impact" — not the crawl, not text volume, not planning. |
| Approach | Structured `impact` on `LogLine`, released with the reveal (A). |
| Rejected | GUI-only fix of the HP-diff floats (B): misses/crits stay invisible, kill swap and skip merge stay wrong. Picture window cutting to the target (C): changes what the window means ("who is acting"). |
| Cues | Floating numbers, miss/fumble marks, row flash, picture-window reaction. All four. |
| Save format | Unchanged. `LogLine`, `BattleState` and `EnemyGroup` are not serialised. |

## Engine — `crates/engine`

`LogLine` (`resources.rs:443`) gains `impact: Option<Impact>`:

```rust
pub struct Impact { pub target: ImpactTarget, pub kind: ImpactKind }
pub enum ImpactTarget { Party(usize), Group(GroupId) }
pub enum ImpactKind { Damage(i32), Crit(i32), Heal(i32), Miss, Fumble, Kill }
```

- `Party(slot)` is the `BattleState::planned` index; slots cannot reorder
  mid-battle, so it is stable for the fight.
- `GroupId` is new: `EnemyGroup` (`battle.rs:435`) gains an `id`, assigned
  from a counter on `BattleState` when a group is formed, never reused in a
  fight. `EnemyGroupView` carries it so the renderer keys rows by id, not
  by letter or index.
- **One impact per line.** A killing swing carries `Damage`/`Crit` on the
  swing line; the existing kill line carries `Kill`. No line is added or
  removed, so reveal pacing, `RosterFrame` counts, sound cues and
  `condense` see exactly what they see today.
- Set at the logging site: `log_swing` (`game/turn.rs:66`) takes the
  impact alongside its `SwingOutcome`; heal, drain, status-tick damage and
  kill lines set it through the same path. A line with no target (round
  header, "stalls out", status expiry) carries `None`.

## App-core — `crates/app-core`

- `App` gains `pending_impacts: Vec<Impact>`, filled exactly where
  `pending_sounds` is: `advance_reveal` (`app/input.rs:432`) pushes each
  released line's impact.
- `skip_reveal` (`:770`) pushes **every** skipped line's impact (sound
  stays one loudest cue — the seam rule is about sound, not sight).
- The GUI drains it with a `take_pending_impacts()` the way it drains
  sounds. App-core knows nothing about pixels.

## GUI — `crates/gui`

All drawing through `Painter` (`.claude/rules/drawing-seam.md`); panes take
their origin from a `Rect`.

- **Floats.** Replace the HP-diff `spawn_float` calls in
  `render/battle.rs` with impact-driven ones, positioned at the target's
  roster row (found by `ImpactTarget`) and staggered in x when several
  land on one row inside a float's lifetime. Styles: damage red `-N`;
  crit larger, white, `CRIT -N`; heal green `+N`; miss/fumble dim `miss` /
  `fumble`; kill `KO`. The ghost bar stays as is; its key moves from group
  index to `GroupId`.
- **Row flash.** A new `Fx` pulse keyed by `ImpactTarget`: the row's
  background tints (red hit, white crit, green heal) and fades over
  ~0.3 s. Misses do not flash.
- **Picture window.** `draw_battle_portrait` (`render/battle.rs:880`) gains
  a brief nudge plus flash on each `Damage`/`Crit` impact (the portrait is
  the acting program by construction, so no actor field is needed), and a
  `KO` stamp over the portrait on `Kill`. The window keeps
  showing whoever is acting.
- Durations and sizes are named constants beside the existing float
  constants in `fx.rs`.

## Testing

- **Engine:** a party hit, an enemy hit, a crit, a miss, a fumble, a heal,
  a status-tick damage line and a killing swing each produce a line with
  the expected `Impact`; the kill swing's damage and the kill line's
  `Kill` are on separate lines; a header line has `None`. `GroupId`
  survives a front group dying (the next group keeps its id, not its
  letter).
- **App-core:** impacts arrive one per revealed line in step with the
  reveal; a skip releases all skipped impacts in order; line-count pacing
  tests stay green unchanged.
- **GUI (headless, as `fx.rs`'s existing tests):** a float lands on the
  row its `ImpactTarget` names, including after a group re-letters; two
  impacts on one row stagger; a flash fades to nothing.
- Mutation-check the stable-id and skip-releases-all tests.
- `cargo test --workspace` gate. `balance_sim` is unaffected (no numbers
  move) but runs as part of the suite.

## Out of scope

Reveal speed and line condensing; the tactical screen (it has its own cell
marks); enemy variety (the follow-on "species identity" work); sound.
