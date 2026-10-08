# Charge Routines — plan

Spec: `docs/superpowers/specs/2026-10-08-charge-routines-design.md`. The spec
holds the rules. This file gives the order of work, the files, the decisions
the spec left open, and the gates. Branch: `charge-routines`. **Where this
plan and the spec disagree, the plan wins** (decisions 1–4 were checked
against the source and correct the spec).

**Every phase:**
- Use TDD and commit at each green step.
- Leave `cargo fmt` and `cargo clippy --workspace --all-targets` clean.
- Unset `FERAL_DEV_NO_SIEGES` before running tests.
- Never push. Never `git checkout`, `stash` or `reset` over uncommitted
  work. Stage explicit paths only. The tree has unrelated `art/` and
  `assets/sprites/glitch.*` changes: do not touch or stage them.
- Before editing, read `.claude/rules/seams-combat.md`. Phases 3–5 also
  read `seams-tactical.md`. Phase 5 also reads `drawing-seam.md`,
  `seams-screens.md` and `content-schema.md`.
- Each phase fits one fresh subagent. Hand it this file and the spec, not
  the history. The phases run in order, because each one consumes the
  previous phase's API.

## Decisions (verified 2026-10-08)

1. **Companions are player-commanded, not AI-driven.** Neither model runs
   AI for a companion (`tactical_ai_actor`, `tactical/ai.rs:413`;
   `slot_is_commanded`, `game/combat.rs:880`).
   - A charging companion gets the same HOLD/RELEASE choice as the lead.
   - The AI choice (decision 8) applies to hostiles, summons, siege staff,
     and party slots under auto-attack/`[R]` (`party_routine_intent`).
2. **A stun cancels at the point where it lands.** The battle map never
   reads stun at turn start (`tactical/turn.rs:721`).
   - Hook `arm_status` (`game/combat_status.rs:89`): if the armed status
     stuns and the body has `Charging`, call `cancel_charge`. This one hook
     covers both models.
3. **Names.** Every routine name must end in its scope word
   (`tests/assets.rs:1065`), and "Overclock Single" already exists.
   - Melee: **Haymaker Single** (`haymaker.ron`).
   - Ranged: **Long Compile Single** (`long_compile.ron`).
   - Each is a new family that exists at Single only, which the
     contiguity census (`:1207`) allows.
4. **Hostile carrier: `sub_process`.** Its level-4 slot
   (`assets/species/sub_process.ron:17`) changes from `rollback_v1` to
   `haymaker`.
   - Haymaker therefore needs `wild_weight: 0` (census `:721`).
   - Long Compile is player-only.
5. **Firing scales a copy of the def.** Add
   `AbilityDef::charged(&self, k, n) -> AbilityDef`, which returns a clone
   with the `Damage` `power` and `spread` scaled by k/N.
   - The status rider is untouched.
   - Fire it through the existing `use_ability` (group model) or
     `run_tactical_routine` (battle map). There is no new damage path and
     no signature change.
   - Unit-test the scaling as a pure function.
6. **Cooldown and Power.** The start pays through `spend_power` but does
   **not** arm the cooldown. The cooldown is armed once, by
   `Game::end_charge(entity, Fired|Cancelled)`, through the model's
   existing helper:
   - `arm_cooldown` (`combat_round.rs:273`) in the group model;
   - `arm_tactical_cooldown` (`tactical/turn.rs:1811`) on the battle map;
   - the `ENEMY_ROUTINE_MIN_COOLDOWN` floor for hostiles, as
     `wild_retaliate` applies it.

   `end_charge` is the one place `Charging` is removed. Both
   `cancel_charge` and firing go through it.
7. **Group model has no cursor and no per-hostile row.**
   - A charger's action is decided at its slot in `battle_resolve_round`
     (`combat_round.rs:52`).
   - At k = N, it auto-fires before the `:154` stun skip, because a stun
     would already have cancelled the charge (decision 2).
   - A hostile charger is handled ahead of `wild_retaliate`'s routine pick.
   - The UI tag rides on `EnemyGroupView` (`enemy_row`, `:868`) as "some
     member of this group is charging" and shows the highest k/N.
8. **AI choice is one pure function.** Add `charge_choice(k, n, aimed:
   &[AimedVictim]) -> ChargeChoice { Hold, Release, Cancel }`.
   - Kill prediction calls `battle::expected_damage`. It is a call, not a
     copy.
   - With no hostile-to-it victim aimed, the result is Cancel.
   - At k = N the auto-fire runs first, so the AI is never asked at N. A
     gone aimed group at N retargets (spec).
9. **Battle map root.**
   - `movement_allowance` (`tactical/reach.rs:65`) returns 0 for a body
     with `Charging`.
   - `tactical_step` also refuses the bump-swing for a charger
     (`turn.rs:~277`), because the bump runs before the allowance check.
10. **Battle map turn shape.**
    - Hold, release and auto-fire each end the turn through
      `forfeit_actions` plus the single `hand_on_turn`, so squads with
      several actions (`turn.rs:219`) are covered.
    - Auto-fire at N is hooked in the shared beat loop
      (`tactical_drive_turn` / `run_tactical_beat`, `ai.rs:~680`), ahead
      of the awaits-input check, so it fires for party bodies too.
    - If that loop does not run on party turns, hook the arrival in
      `hand_on_turn` instead, and record which one was used in the commit.
    - Firing goes through `run_tactical_routine`, which hands on itself,
      so the beat must not hand on again (`battle.actor() == Some(actor)`
      guard).
11. **Aim cells** are taken at the start from `reach::shape_cells` through
    `Game::routine_tactical_shape/range` (`game/unlocks.rs:43/57`). They
    are stored absolute.
    - At release, recipients are whoever stands in those cells, filtered
      through the same hostile-to-actor rule `reach::recipients` uses.
      Extract that rule if it is inline.
    - Teleport and Jump leave the cells alone.
12. **Flee.**
    - Group model: `battle_flee` (`combat_teardown.rs:49`) cancels every
      party charge on the attempt, whether it succeeds or fails.
    - Battle map: fleeing is a step off the edge, so a charger cannot flee.
      RELEASE is always offered from the turn after the start, so the
      player is held for at most one turn.
    - The spec's "flee cancels" holds for the group model only.
13. **Teardown.** Add `remove_charging` as a fifth helper beside
    `disarm_reach_charge` in all three loops of
    `clear_battle_status_effects` (`combat_teardown.rs:164`).
    - Name the component `Charging`; `ReachCharge` is an unrelated
      weapon state.
14. **fx plays on the battle map only.** `queue_routine_cue` is the only
    `fx` consumer, and the group model plays no routine cue for any
    routine.
15. **Keys.**
    - Battle map: `H` holds and `X` releases. Both are unbound today
      (`app-core/src/app/tactical.rs:41`).
    - Group model: a charging slot's row list (`battle_action_options`,
      `game/combat.rs:1805`) becomes `h` HOLD / `x` RELEASE, as rows.
      Party commands stay uppercase.
    - Add `BattleAction::ChargeHold` and `ChargeRelease` (`battle.rs:530`).
16. **Tuning.** The arena bin never has the player invoke a routine
    (`arena/scenario.rs:215`), so:
    - the bin tunes Haymaker on the hostile side;
    - the player side is checked by hand on the played arena screen
      (`FERAL_DEV_ARENA=1`).
    - Read `docs/measurements/` first.
17. **New seam** (three writes, in the order the `seams` skill gives:
    graph, skill, `seams-combat.md`): "A charge is cancelled where the stun
    lands, and its cooldown arms only in `end_charge`." The reason is that
    the battle map never reads stun at turn start, and an armed-at-start
    cooldown would tick down during the wind-up.

## Phase 1: Data (engine)

**Files:**
- `abilities.rs`:
  - add `ChargeSpec { rounds }` and `AbilityDef::charge` with
    `#[serde(default)]`;
  - add `charge_mismatch()`, which rejects a non-`Damage` effect and
    `rounds < 2`, wired into the `load_dir` chain (`:1531`);
  - add `charged(k, n)` (decision 5).
- `components.rs`: add `Charging { ability, rounds, aim: ChargeAim }` and
  `ChargeAim { Group(Entity), Cells(Vec<(i32,i32)>) }`.
- `assets/abilities/README.md`: document `charge`.

**Tests:** scaling (power, spread and an unscaled rider); `load_dir`
skips both malformed cases with a warning.

**Gate:** `cargo test -p feral-processes-engine abilities`.

## Phase 2: Group model (engine)

**Files:** `game/combat_round.rs`, `game/combat_enemy.rs`,
`game/combat_status.rs`, `game/combat_teardown.rs`, `game/combat.rs`,
`battle.rs`, and a new `game/charge.rs`. The new file holds
`start_charge`, `end_charge`, `cancel_charge`, `fire_charge` and
`charge_choice`, so the shared lifecycle has one home.

**Steps:**
- Start: the `Special` arm of `resolve_one_action` branches on
  `def.charge`.
- Slot handling: decision 7.
- Stun hook: decision 2.
- Flee: decision 12.
- Teardown: decision 13.
- Hostile charge: decisions 7 and 8.
- `EnemyGroupView` and the party slot view gain a charge tag:
  `(k, n, target_label)`.

**Tests:** these are the spec's group items, plus:
- no landing on the start turn;
- a stun cancels, with Power spent and the cooldown armed;
- the aimed group is gone at release, and the hit retargets;
- the AI releases on a kill, cancels on an empty aim, and otherwise holds;
- teardown clears `Charging`;
- a failed flee cancels.

**Gate:** `cargo test -p feral-processes-engine`.

## Phase 3: Battle map (engine)

**Files:**
- `tactical/reach.rs`: decision 9.
- `tactical/turn.rs`:
  - the start goes through `tactical_use_routine`;
  - add `tactical_charge_hold` and `tactical_charge_release`, the player
    doors;
  - refuse the bump-swing (decision 9).
- `tactical/ai.rs`: decisions 8 and 10.
- `tactical/view.rs`:
  - `TacticalBody.charge: Option<(u32,u32)>`, built at `:532` and copied
    in `frozen()` at `:253`;
  - `TacticalView.charge_aims: Vec<ChargeAimView { cells, party_side:
    bool }>`.
- `game/charge.rs`: add the cells variant of fire and choice.

**Tests:** the spec's battle-map items:
- a charger cannot step or bump;
- the AI charger stays on its cell;
- a victim who steps out is missed, and one who steps in is hit;
- the charger dies mid-charge and the cursor holds;
- a squad charger forfeits its remaining actions;
- displacement keeps the aim cells;
- the AI hands on exactly once;
- auto-fire at N for a party body.

**Gate:** `cargo test -p feral-processes-engine`.

## Phase 4: Input (app-core)

**Files:**
- `app/tactical.rs`: while the active body is charging, only `H` and `X`
  (and `Esc` and the help keys) act. Every other action key is refused
  with the existing refusal message path.
- `app/battle.rs` and the rows: decision 15.

**Tests:**
- in tactical, `H` or `X` while charging reaches the engine door, and a
  move key is refused;
- in a group battle, the charging slot shows only the hold and release
  rows.

**Gate:** `cargo test -p feral-processes-app-core`.

## Phase 5: Render, content, arena, seam

**Files:**
- `gui/src/render/tactical.rs`:
  - a `CHG k/N` tag beside the status tags in `draw_body` (`:930`);
  - a pulsing outline;
  - `Wash::ChargeParty` and `Wash::ChargeHostile` in `board_washes`
    (`:191`) and the legend (`:247`).
- `gui/src/render/battle.rs`: the group row reads `charging k/N →
  <group>` (`:~376` hostile, `:~540` party).
- `assets/abilities/haymaker.ron` and `long_compile.ron`:
  - each has `charge`, `cooldown`, `research_zone` and `shape`/`range`;
  - Haymaker has `wild_weight: 0`;
  - confirm both get a node through `routine_tree::gets_node`.
- `assets/species/sub_process.ron`: decision 4.
- `dev-arenas/charge-haymaker.ron` (Tactical, `character.routine:
  Some("haymaker")`, opponents `sub_process`) and
  `charge-long-compile.ron`, then tuning (decision 16).
- The seam: decision 17.

**Tests:** the asset census passes, including a charge census (`Damage`,
`rounds >= 2`). Screenshot through the arena per the
`battle-screenshot-goes-through-the-arena` rule: take it mid-charge, read
the PNG, and check the tag, the wash and the legend.

**Gate:** `cargo test --workspace` and `cargo clippy --workspace
--all-targets`.

## After Phase 5

Run the final whole-branch review with Opus, giving it the diff as a file.
Then land the branch through the `deploy` skill, which writes the
`CHANGELOG.md` entry and the version bump. There is no save-format change.
