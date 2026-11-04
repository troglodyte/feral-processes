# Field auto-recast — design

**Date:** 2026-11-04 · **Branch:** `worktree-field-auto-recast`

## Intent

The user runs Field Regen and Recharge builds all the time. These are timed
field buffs (`FieldBuffKind::Regen` / `Trickle`) that count down and fade.
Recasting them by hand every time they fade is busywork. Instead, a
per-routine option recasts the buff on its own when it runs out.

## Goals

- On the field routine list, a key turns auto-recast on or off for the
  highlighted routine. The row shows whether it is on.
- When a timed field buff fades and its routine is flagged, the routine's
  holder recasts it straight away on the same target and pays Power as
  usual.
- If the recast can't happen, the buff fades, the log says why, and the
  routine's flag turns off.
- The flag is saved with the game.

## Non-goals

- **No recast when a rest clears a buff.** The user chose "when it runs out
  only". Until-rest kinds (Atk, Mitigation, CaptureBoost, XpBoost,
  EncounterDamp, DropBoost) never run out, so they are out of scope.
- **No recast during or after a rest.** The user's words: "rest should
  reset clear them all. if the player wants them after the rest, they can
  recast them." See *Rest clears timed routine buffs* below.
- No auto-recast for battle Specials, heals, Phase, Jump or Symlink.
- No global on/off setting. The flag is per routine only (the user's
  choice).
- No "wait until affordable" retry (the user chose skip-and-turn-off).

## Anti-goals

- A recast that silently drains Power, or silently stops. Every recast and
  every failed recast gets a log line.
- A flag that comes back on by itself, for example after the routine is
  reinstalled.

## Decided by the user

| Question | Answer |
|---|---|
| Which routines | Anything long-running that runs outside battle. In practice, that means timed field buffs. |
| When it fires | Only when the buff runs out |
| Where the toggle lives | Per routine, on the field routine menu |
| Not enough Power | Skip it, say so, and turn the flag off |
| Rest | Rest clears every routine buff, timed ones included. Nothing recasts. |
| One-ally target no longer in the party | Fade and log it, but **leave the flag on** |

## Design

### Eligibility

A routine can be flagged when it is field-runnable, its effect is
`AbilityEffect::FieldBuff`, and its `kind.runs_until_rest()` is false. Today
that covers crafted Field Regen and Recharge builds (`routine_craft/compile.rs`
`field_kind`) plus any asset-defined timed FieldBuff. The rule is one
predicate. The menu, the toggle and the recast all call it.

### Where the flag lives (Claude's call)

The flag belongs to the **holder**: you, or the program the routine is
installed on. It is stored as a set of `AbilityId`s per holder.

- Runtime: a small per-holder component next to `Routines`
  (`components/staff.rs`). It follows the entity the way `Routines` does,
  so that party, bench and save mapping all come along with no extra work.
- Save: an additive `#[serde(default, skip_serializing_if = "Vec::is_empty")]`
  `auto_recast: Vec<AbilityId>` on `PlayerSave` and `CreatureSave`. There is
  **no `SAVE_FORMAT_VERSION` bump** (`.claude/rules/seams-screens.md`: an
  additive serde-default field costs none). The struct-literal sites in
  `save/tests.rs` need the new field. A save→load test is required (memory:
  ron-round-trip-cannot-catch-a-skipped-field).
- Uninstalling a routine clears its flag on that holder, so a reinstall
  starts with the flag off.

### Knowing which routine armed a buff (Claude's call)

`ActiveFieldBuff` holds only a display `name`, so nothing can look a def
up by it. Add `#[serde(default)] ability: Option<AbilityId>`, set by
`run_field_routine` and left `None` for consumables and older saves. A
buff with `None` never recasts.

### The recast

In `tick_field_buffs`, the expiry pass collects `(recipient, ability)` for
every expired buff that has `Some(ability)`, instead of only names. Once
the borrow is dropped:

1. Group the expired buffs by ability. A WholeParty routine expires on
   every recipient on the same tick and must recast **once**.
2. For each ability, the holder is the first entry in `routine_holders()`
   order that has the ability installed and flagged. If there is no such
   holder (for example the routine was uninstalled), the buff fades
   quietly as it does today. *Claude's call:* the casting holder is not
   recorded. The only case where this matters is the same routine
   installed on two holders with only one flagged, and in that case the
   flagged holder recasts and pays. Recording the caster would need a
   saveable holder reference, and nothing saved has one.
3. The target follows the routine's `AbilityTarget`. OneAlly →
   `FieldRoutineTarget::Ally(recipient)`. WholeParty → `None`. Every
   eligible kind is Creature-scoped (all the Run-scoped kinds last until
   rest), so the Run-scope branch is never reached here. If only some of
   a WholeParty routine's recipients expire on a tick, it still recasts
   once on the whole party, and `arm_field_buff` replaces the rest.
4. Cast through a new inner function, factored out of
   `run_field_routine` (`game/field.rs:227`), that takes
   `(holder, &AbilityId, FieldRoutineTarget)`. *Claude's call:* it runs
   **every** gate a manual cast runs: the row's `unavailable` reason
   (find the row by `(holder, ability)` in `field_routines()`, so the two
   paths share one builder), `field_recipients`, magnitude, `spend_power`
   and `arm_field_buff`. The one exception is the "not during a battle"
   refusal, which stays only in `run_field_routine`'s outer guard. The
   inner function does **not** call `tick()`, because the caller is
   already inside `tick_inner` and that would recurse.
   `run_field_routine` becomes "look up the row by index → inner fn → log
   → tick", so a manual cast and a recast cannot price or arm
   differently.
5. Success: log `"{holder} reruns {name}."` in place of `"{name} fades."`.
6. Failure. The two failure kinds need different outcomes, so the inner
   function's error has to tell them apart (an error enum, not a bare
   `String`):
   - **The target is gone.** `field_recipients` refuses (the ally is not
     in the party, or is down). Log `"{name} fades — couldn't rerun it:
     {reason}."` and **leave the flag on** (user decision).
   - **Anything else** (not enough Power, or the row is unavailable).
     Log `"{name} fades — couldn't rerun it: {reason}. Auto-recast is
     off."` and clear the flag on that holder (user decision for Power;
     *Claude's call* to treat the other reasons the same way).

### Rest clears timed routine buffs (user decision)

Today, `rest` drops only the until-rest buffs (`drop_until_rest_buffs`,
`game/combat_status.rs` ~400), and a timed buff keeps ticking through
rest's `tick_inner(false)` loop. The change: rest drops **every
`BuffSource::Routine` field buff**, timed ones included, **before** its
tick loop runs. Nothing is left to expire during the rest, so no recast
can fire then. Afterwards the player recasts by hand. The flag stays on
and takes effect the next time the buff runs out.

- *Claude's call:* consumable-sourced buffs (`BuffSource::Consumable`,
  from items such as `sustain_cell`) are left as they are. The user's
  "them" referred to routines.
- A Forgiving reboot also calls `drop_until_rest_buffs`. *Claude's call:*
  it gets the same treatment, since it stands in for a rest. The builder
  must check this against its caller.
- The function's name and doc comment must change to match its new
  scope. Update every reader that relies on "timed buffs survive a rest",
  including tests in `engine/src/tests/turn.rs` and the `runs_until_rest`
  doc comments. `runs_until_rest` itself still means "doesn't count
  down".

Following CLAUDE.md's functional-core rule, a pure decide step takes the
gathered expiries, holders and flags and returns which recasts to try. The
apply step casts them and writes the logs. The tests cover the decide
step directly. This path draws no RNG.

### The menu

- `FieldRoutineView` gains `auto_recast: Option<bool>`. `None` means not
  eligible, and `Some(on)` gives the current state. Set it only in
  `Game::field_routines()`, which is the single builder.
- Engine: `Game::toggle_field_auto_recast(index) -> Result<bool, String>`,
  using the same index space as `run_field_routine`. An ineligible row
  returns an `Err` with a plain sentence, for example `"{name} doesn't run
  out on its own."`. The key is an action key, not a row label, so it
  doesn't conflict with "lists are highlight + Enter".
- app-core (`app/field.rs` `handle_field_routine_key`): a letter key,
  checked before `selected_index`, toggles the highlighted row and stays
  on the list. Success goes through `report` ("Auto-recast on for {name}."
  / "off"). An `Err` goes through `refuse`. The pattern to follow is
  `App::toggle_party_member` (`app/party.rs:133`). Claude proposes **A**,
  but the builder should check it is free on this screen and follow the
  case convention (memory: key-hint-case-is-the-handler-case).
- gui (`render/field.rs:213`): eligible rows show `[auto]` after the name
  when the flag is on, and nothing when it is off. Add `"A auto"` to the
  footer chips at `:244`. The row count does not change.

## Left to the builder

- Check whether `tick_inner` runs while a battle is active. If it does,
  the recast still goes ahead there (Claude's call: it is upkeep, not an
  action, and the manual-cast "not during battle" refusal does not apply).
  Write it down whichever way it turns out.
- The exact names of the new component, predicate and inner function.
- Which test files the tests go in. They follow the existing ones:
  `engine/src/tests/turn.rs` for expiry, `app-core/src/tests/field.rs`
  for the key, and `gui/src/render/tests` for the row.

## Constraints

- Engine changes follow CLAUDE.md (gather, then decide, then apply; no new
  `world.get` sprawl beyond the holder component).
- Run `cargo test --workspace`, `clippy --all-targets` and `fmt`. No
  `tuning/` change is expected, so `balance_sim` should be unaffected, but
  run it anyway, because `run_field_routine` gets refactored.
- CHANGELOG entry at release time. The manual and README are left alone
  (memory).
