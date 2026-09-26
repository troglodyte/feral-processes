# Auto-attack invokes routines — plan

Spec: `docs/superpowers/specs/2026-09-26-auto-attack-routines-design.md` (#103).
Branch: `auto-attack-routines`. TDD throughout, a commit per green step.

Three phases, each one dispatch (sonnet), each ending green on
`cargo test -p <crate>` + `cargo clippy --workspace --all-targets` + `cargo fmt`.
Phase 1 is a pure refactor so phases 2 and 3 never touch the string sites.
Full `cargo test --workspace` once, after phase 3. Final review: opus, whole
branch.

## Anchors (verified 2026-09-26)

| What | Where |
|---|---|
| `Intent` enum (`Swing{range}`, `Routine`) | `crates/engine/src/tactical/ai.rs:44` |
| `ForecastAction` | `tactical/ai.rs:142` |
| `tactical_ai_beat` / `tactical_auto_beat` / `tactical_drive_turn` | `tactical/ai.rs:489 / 515 / 571` |
| `run_tactical_beat` (the `match intent` that acts) | `tactical/ai.rs` ~600–680 |
| `tactical_intent` (`Hostile` gate on the routine arm) | `tactical/ai.rs:685` |
| `run_tactical_intent`, `chosen_target`, `best_aim` | `tactical/ai.rs:1032 / ~1060 / 1102` |
| `run_tactical_routine` (charges, never asks the gate) | `tactical/turn.rs:1239` |
| `wild_routine_ready` (returns first uncooled) | `game/combat_enemy.rs:74` |
| `ability_unavailable -> Option<String>` (15 call sites, 7 files) | `game/combat.rs:1432` |
| `consume_item(who, id) -> bool` (refusal spends nothing) | `game/turn.rs:1297` |
| `battle_usable_items` | `game/combat.rs:1401` |
| drive-turn callers | `game/auto_resolve.rs:82`, `arena/run.rs:99`, tests in `tests/{auto_resolve,tactical,cloak,tamper}.rs` |
| app-core tactical keys (`A` auto, `R` resolve; `U`/`u` free) | `crates/app-core/src/app/tactical.rs:57,113` |
| group model's item picker to copy the shape of | `app-core/src/app/battle.rs:508` (`Mode::BattleItem`) |
| `Mode` enum / `ALL_MODES` | `app-core/src/lib.rs:1427` / grep `ALL_MODES` |

## Phase 1 — `RoutineRefusal` (engine only, no behaviour change)

- New `pub(crate) enum RoutineRefusal` beside `ability_unavailable`, one
  variant per current `return Some(..)` arm; `Power { cost }` carries the
  figure the string printed. `Display` renders **exactly** today's strings
  (`"needs {cost:.0} PWR"`, `"{n} more rounds"`, …).
- `ability_unavailable` returns `Option<RoutineRefusal>`; every caller that
  wants a string calls `.to_string()` (or `map(|r| r.to_string())`). Don't
  change any caller's logic.
- Tests: existing string assertions stay untouched and green — that *is* the
  test. Add one: an unaffordable routine refuses with `RoutineRefusal::Power`.
- Gate: `cargo test -p feral-processes-engine`.

## Phase 2 — the planner's party arm and the cell (engine)

1. **Candidate iterator.** Factor `wild_routine_ready`'s filter chain into a
   `fn routine_candidates(&self, e) -> impl Iterator<Item = AbilityDef>` (or a
   `Vec`); `wild_routine_ready` becomes `.next()` on it. Hostile behaviour
   byte-identical.
2. **`PartyTurns { Invoke, SwingOnly }`** (engine `pub`, since app-core
   never names it — keep `pub(crate)` if nothing outside needs it). Thread it
   `tactical_auto_beat` → `Invoke`; `tactical_drive_turn(turns)` →
   `auto_resolve_battle_with` passes `Invoke`, `arena/run.rs` and the
   existing tests that assert arena parity pass `SwingOnly`;
   `tactical_ai_beat` passes `SwingOnly` (irrelevant: it only reaches
   hostile/summoned/taken-over bodies, whose arm is unchanged). Plumb through
   `run_tactical_turn` → `run_tactical_beat` → `tactical_intent`.
3. **`tactical_intent` party arm**, only when `Invoke` and the actor is not
   `Hostile`:
   - first candidate with `ability_unavailable(actor, def).is_none()` →
     `Intent::Routine`;
   - else, if the first candidate whose *only* refusal is `Power { cost }` exists
     (i.e. `ability_unavailable` returns `Power` — Power is checked after
     cooldown/tactical-only, before the Decompile/Emulate/Teleport arms, so
     confirm those later arms would also pass; simplest is a helper that asks
     the gate with the Power check skipped, or reorder so Power is last and
     document why), and `power_cell_for(actor, shortfall)` finds one →
     `Intent::UseItem(ItemId)`;
   - else `Intent::Swing`.
   Summoned/taken-over bodies keep today's behaviour — decide from what
   `acts_for_hostiles` says and write the reason in the doc comment.
4. **`power_cell_for(shortfall) -> Option<ItemId>`**: pack items whose def
   `consume` has `power > 0` and `prebattle_buff.is_none()`; smallest `power`
   ≥ shortfall, else largest. Data-driven, no id.
5. **`Intent::UseItem(ItemId)`** through every `match` on `Intent`: `band()`
   (any distance), `helpful`, range, `chosen_target` (`None`/self),
   `ForecastAction` (add `UseItem(ItemId)` if a forecast can be built for a
   party body; otherwise map it to the nearest honest variant and say why).
   In `run_tactical_beat` skip `walk_to_best_cell` for `UseItem` and act via
   the new `tactical_use_item` (below) for `actor`.
6. **`Game::tactical_use_item(&mut self, id: &ItemId) -> bool`** in
   `tactical/turn.rs`, beside `tactical_defend`: acting body only, refusals
   (no fight, not a party body, not usable/none in pack) return `false`
   before anything is spent; else `consume_item(actor, id)`, spend one
   action, end through `hand_on_turn` exactly as `tactical_defend` does.
   This is the door `[U]` uses in phase 3.

Tests (new, `tests/tactical.rs` unless a file fits better; use
`tests/support.rs` fixtures):
- auto beat invokes a ready, affordable routine (Power drops, cooldown armed);
- a cooling routine / an unaffordable routine with no cell → swings;
- Power is the sole refusal + a `power_cell` in pack → cell drunk, Power up,
  one action spent, pack −1; next turn invokes;
- a consumable with `prebattle_buff` (sustain/backfeed-style) is never drunk;
  empty pack → swing;
- `SwingOnly` (`tactical_drive_turn(SwingOnly)`) never invokes nor drinks —
  hold the existing arena-parity tests green unchanged;
- `[R]`: `tests/auto_resolve.rs` — auto-resolve with a ready routine invokes it;
- `tactical_use_item` refusals spend nothing (per refusal, not one test).
Mutation-check the sole-refusal gate and the `prebattle_buff` filter.
Gate: `cargo test -p feral-processes-engine`, and
`cargo run --bin arena -- dev-arenas/opening-fight.ron` number unchanged
against a pre-branch run in the same build (arena numbers compare within one
build only).

## Phase 3 — `[U]` on the battle map (app-core + gui)

- `Mode::TacticalItem` (name to match sibling tactical modes). `U` in
  `app/tactical.rs` opens it only when `tactical_awaits_input` and
  `battle_usable_items` is non-empty (else `App::refuse` with a sentence).
- Handler modelled on `handle_battle_item_key`: `Esc` back to the tactical
  mode, lowercase row selector, pick → `game.tactical_use_item(&id)`; a
  `false` → `refuse`, mode restored, nothing spent. No `T`-throw easter egg
  here.
- Add to `ALL_MODES` (does **not** fail to compile — grep it) and the gui draw
  match (ends in `_ => {}`); draw with the same popup builder the group
  item picker uses, over the tactical map. Add `[U]` to the tactical key strip
  / help line wherever `[A]`/`[R]` are listed.
- Tests: app-core — `[U]` then a row spends one action and one cell; a refused
  item spends nothing and leaves the mode sane; `Esc` spends nothing. gui —
  the `ALL_MODES` census covers the mode; optionally a screenshot
  (`--template <tactical one> --keys "U" --screenshot`) read back.
- Gate: `cargo test --workspace`, clippy, fmt.

## Docs (last commit of phase 3)

- Rewrite the doc comments on `tactical_auto_beat`, `tactical_drive_turn`,
  `tactical_intent`, `run_tactical_beat`'s swing-only paragraph.
- CLAUDE.md seam line "Auto-attack is app-core answering for the party…"
  → one sentence: auto-attack and `[R]` invoke through `tactical_intent`'s
  party arm gated on `ability_unavailable`, and the arena passes
  `PartyTurns::SwingOnly`. Mirror in the `seams` skill reference and the
  graph (`seam:` entry), per the `seams` skill's order.
- `CHANGELOG.md` is written at the merge (deploy), not here.

## Not in scope

Group model, hostile AI, save format, schema, `.ron`, `balance_sim`.
