# Program permadeath

A tamed program whose HP reaches 0 is destroyed and removed from the world.
Its installed routines die with it. There is no revival and no difficulty
gate.

## Why

Today a program at 0 HP is only knocked offline. In battle it keeps its slot
until the fight ends, then `end_battle` drops it from `Party` while the
entity survives; resting at base sets `hp = max_hp` on every program the
player owns, so the loss costs nothing but a walk home. A raid defender
knocked to 0 likewise just sheds its cronjob. Nothing the player owns can
ever be taken from them by a fight.

## The rule

- Any tamed program reaching 0 HP dies, is despawned, and is gone for good.
- Its `Routines` are destroyed with it. Nothing is returned to inventory —
  there is no death drop.
- This applies in both `DifficultyMode::Permadeath` and
  `DifficultyMode::Forgiving`. `DifficultyMode` continues to mean only
  "what happens when *you* hit 0"; player death stays with
  `difficulty::death_handling_system` and is untouched here.
- It applies outside battle as well as in it. A cronjob worker that dies
  defending a structure during a raid is lost even though the player was
  not present. Programs have no passive HP regen, so raid chip damage
  accumulates until the player walks home and rests — attrition is the
  intended cost.

## Coverage

Every write to `Stats::hp` was audited. The only one that *lowers* a tamed
program's HP is `Game::apply_damage` (`crates/engine/src/game/combat_status.rs:306`);
the others are heals, the two full-heals (`turn.rs:390`/`:407` for rest,
`unlocks.rs:99` for level-up), and `needs_decay_system`, which is
`With<Player>`. `apply_damage` has six non-test callers: four in battle
(`combat_round.rs:219` direct hit, `combat_status.rs:257` single-target
ability, `combat_round.rs:710` and `:726` multi-target ability), one status
tick (`combat_status.rs:380`), and one raid defence (`upkeep.rs:164`). Two
reap sites therefore cover the whole surface.

## Reap site 1 — battle, deferred

Death is **detected inside `apply_damage`**, on the `> 0` to `0` transition,
guarded on the target being a member of `Party`. That guard is what keeps
the announcement off hostiles (which have their own `finish_member` path)
and off the player (`death_handling_system`). Detecting at the chokepoint
rather than at each of the four battle call sites is the same argument the
Coverage section makes: one site that provably cannot be bypassed, instead
of four that a fifth caller could later miss.

A party member can never be the raid defender — `add_companion` strips
`Task`, and `raid_check` finds its defender by `Task` — so the two reap
sites cannot both fire for the same program.

The announcement is a `MessageKind::Outcome` line naming the program and the
routines lost with it (read from the existing `Game::extractable_routines`).

The entity itself is **not** despawned at that moment. It keeps its party
slot for the rest of the fight, unable to act, exactly as today —
`BattleState::planned` indexes `Party` positionally (see `actor_entity`), so
removing a member mid-battle shifts every member behind it into the wrong
slot. `crates/engine/src/game/combat_status.rs:610` already documents this
constraint for the existing deferred party removal.

`Game::end_battle` then calls `dissolve_tamed_program` on each dead member
in place of the current `Party::retain`.

Ordering inside `end_battle` is load-bearing and gets its own test: the reap
must run **before** `retain_outcomes_since_battle()`. `dissolve_tamed_program`
logs its own detachment lines ("leaves your battle party") at ordinary log
kind, so running it first means those lines are pruned and only the `Outcome`
death line follows the player onto the map. Reversing the order would leave a
redundant departure line trailing the death.

## Reap site 2 — raid, immediate

`Game::raid_check` (`crates/engine/src/game/upkeep.rs:164`) currently strips
the defender's `Task` and logs that it stood down. That branch becomes a
death: a `MessageKind::Raid` line, then `dissolve_tamed_program`. Nothing
indexes positionally here, so the despawn happens on the spot.

The existing `Task` strip stays and runs *before* the dissolve. That is what
keeps the log clean: `raid_check` finds its defender by `Task.target ==
target`, so the program is always working the very structure the death line
already names, and `sale_detachments` would otherwise add a redundant "stops
working the Mining Node" line directly under "is destroyed defending the
Mining Node". With the `Task` gone first, `dissolve_tamed_program`
contributes only the `Party` retain and the despawn, and the single `Raid`
line stands alone.

## Reusing `dissolve_tamed_program`

`crates/engine/src/game/trade.rs:164` already logs detachments, drops the
program from `Party`, removes its `Task`, and despawns it. Its doc comment
names it the single call that keeps program-destruction sequences from
drifting apart; `sell_companion` and `routines::extract_routine` are its two
existing callers. Death becomes the third, rather than a fourth hand-rolled
despawn that a doc comment would have to promise matched the others.

## Save format

Unchanged. `BattleState` is not persisted, and a dead program is simply
absent from the saved roster.

## UI — the danger marker

Permadeath without a visible "this one dies next hit" cue is easy to blunder
into, so two draw sites flag a program in danger. Both call one helper,
`hp_critical(hp: i32, max_hp: i32) -> bool` in
`crates/gui/src/render/mod.rs`, integer maths, true at or below one third
(`hp * 3 <= max_hp`) — one function, so the threshold cannot drift into two
copies.

The threshold lives beside `RED`/`GREEN`/`CYAN` in `render/mod.rs`, not in
`tuning.rs`. It is a readout, not a difficulty knob; the same call was made
for `REVEAL_LINES_PER_SECOND` living in app-core.

- **Battle party rows** (`crates/gui/src/render/battle.rs:302`): the bar is
  drawn `RED` when critical, overriding both `GREEN` and the active member's
  `CYAN`. The active member remains marked by its `>` prefix and
  `bold: active`, so spending the colour on the warning costs nothing.
- **Party menu** (`crates/gui/src/render/party.rs:27`): a new
  `critical_item_row` in `popup.rs`, alongside the existing `spent_item_row`
  and `creature_row`, colouring the row `RED` — plus a ` — CRITICAL` suffix
  in the row text so the cue is not colour-only.

Deliberately unmarked: the manifest screen and the fuse candidate lists.
Neither is a moment where the player can get a program killed.

## Docs

Three claims become false and are corrected in the same change:

- `docs/manual.md:683` — "to 0 HP stands down automatically — it isn't lost,
  just no longer active".
- `docs/manual.md:976` — the raid defender saying the same.
- `docs/manual.md:124` — "to shed a program for good, sell it at a Market
  (`t`) or fuse it (`f`)"; death is now a third way.

Plus a CHANGELOG entry under Unreleased. The README's permadeath sentence
describes *player* death and stays accurate as written.

## Tests

Engine:

- A companion killed in battle is absent from the world and from
  `owned_pets` once `end_battle` has run.
- Its installed routine did **not** land in the player's inventory.
- The death line is `MessageKind::Outcome` and survives
  `retain_outcomes_since_battle` — this is the test that pins the ordering
  inside `end_battle`.
- A raid defender killed in `raid_check` is despawned along with its `Task`.
- The player at 0 HP is untouched by this path and still routes through
  `death_handling_system`.
- A hostile brought to 0 by the same `apply_damage` call still routes
  through `finish_member` and is not double-announced by the new guard.
- `crates/engine/src/tests/party.rs:409` currently asserts only that the
  party is empty after the battle. Strengthened to assert the entity is
  actually gone: "removed from party" and "deleted" are now different
  claims and the weaker one no longer proves the stronger.

GUI:

- `hp_critical` at the boundary (exactly one third is critical, one point
  above is not).

Gates:

- `cargo test --workspace`.
- `cargo test -p feral-processes-engine balance_sim`. The curves are not
  expected to move — the sim's damage maths is untouched — but confirming
  that is the gate's job, not an assumption.

Not covered by tests, per standing policy: the actual look of a red bar in
the battle pane and party menu. That needs the user's eyes after the build.
