# Data-defined, stacking status effects

**Status:** approved design, unbuilt. TODO #107 ("poison type effects,
status effects").

## Goal

Battle status conditions become content: a modder adds a status by dropping
a `.ron` in `assets/statuses/`, and a body can carry several different
statuses at once, some of which stack. Ships Poison, Throttled and Locked
alongside the existing Bleed, Stun and Exposed.

## Decided, and out of scope

- **Battle-scoped only.** Conditions are still wiped by
  `clear_battle_status_effects`; nothing lingers on the map. No save-format
  change: `StatusEffects` is not serialized today and stays that way.
- **Resistance stays one stat.** `Derived::status_resist` shortens every
  duration via `progression::resisted_duration`, applied in `arm_status`
  only. No per-kind immunities.
- **Status-move pricing is not fixed here.** The trained enemy policy never
  picks status moves (`finding:trained-policy-ignores-status-effect-moves`).
  This spec measures whether Poison is used (§6); repricing is a follow-up
  if the measurement says so.
- **Untouched systems:** `CombatBuff`, `Cloaked`, `Tampered`, `Emulation`,
  field buffs. They differ in kind (wanted buffs, targeting, AI tampering)
  and folding them in is not asked for.

## 1. Schema: `StatusDef`

One file per status in `assets/statuses/`, loaded by `StatusDb::load_dir`
following the `*Db::load_dir` pattern (malformed file → logged warning,
skipped; duplicate id → warning, first wins).

```ron
StatusDef(
    id: "poison",
    name: "Poisoned",          // party panel: "Poisoned (3)"
    tag: "PSN",                // battle map / tactical tag, ≤ 4 chars
    stacking: Stack(max: 5),   // or Refresh
    behaviours: [DamagePerRound],
    inflict: "{target} is poisoned!",
    tick: "{target} loses {n} Integrity to poison.",   // default ""
    expire: "{target} flushes the poison.",
)
```

`StatusBehaviour` is a closed Rust enum; modders compose, not extend:

| Behaviour | Effect, per stack | Replaces |
| --- | --- | --- |
| `DamagePerRound` | `power × stacks` damage at end of round; logs `tick` with `{n}` | Bleed's hard-coded tick |
| `SkipTurn` | body loses its action | `is_stunned`'s kind check |
| `EvasionCut(percent)` | evasion cut by `percent × stacks` | `StatusKind::Exposed` check, `EXPOSED_EVASION_PERCENT` |
| `AtkPercent(i32)` | attack scaled by `percent × stacks` | new |
| `MitigationPercent(i32)` | mitigation scaled by `percent × stacks` | new |
| `HealBlock` | heals restore 0 | new |

`StatusStacking`: `Refresh` | `Stack { max: u32 }`. Percent effects are
clamped so a stack can never invert a stat (attack/mitigation floor 0,
evasion cut capped at 100).

`EXPOSED_EVASION_PERCENT` moves from `tuning.rs` into `exposed.ron` as
`EvasionCut(n)` — it is content, not tuning.

### Referencing a status

`StatusKind` (components.rs) is deleted. A `StatusId(String)` newtype
replaces it in `species::MoveEffect::kind` and
`abilities::AbilityEffect::Debuff::kind`. All in-repo `.ron` references
(39 lines across `assets/species/` and `assets/abilities/`) are rewritten
`kind: Bleed` → `kind: "bleed"`. A modded file still using the bare form
fails to parse and is skipped with a warning — a one-time schema break,
accepted (saves unaffected).

**Startup cross-check:** after all DBs load, every `StatusId` named by a
move or ability must resolve in `StatusDb`. An unresolved id logs a
warning and its rider is dropped (the move/ability still loads and does its
direct effect). Arming an unknown id at runtime is a logged no-op.

**Engine-named ids:** the fumble ladder (`combat_damage.rs` ~:243, :256)
names `exposed` and `stun`. They become `tuning.rs` constants
`FUMBLE_EXPOSED_STATUS` / `FUMBLE_CRASH_STATUS`; missing from `StatusDb` →
the rung arms nothing, warned once at load.

### Messages

The `match kind` log lines in `combat_status.rs` (apply, expire),
`combat_round.rs:1647` (ability landing) and `party.rs:950` (panel label)
are replaced by the def's `inflict` / `expire` / `name`. Moves and abilities
now share one landing line; the ability-specific phrasing ("springs a leak
in X") is dropped. `{target}` is the only placeholder in `inflict` and
`expire`; `tick` adds `{n}`.

## 2. Runtime

```rust
pub struct ActiveStatus {
    pub id: StatusId,
    pub remaining: u32,
    pub power: i32,
    pub stacks: u32,
    pub landed_this_round: bool,
}
pub struct StatusEffects { pub active: Vec<ActiveStatus> } // ≤ 1 entry per id
```

- **`arm_status(entity, &StatusId, duration, power)`** stays the sole
  writer besides the two clearers (Cleanse, battle teardown) and still
  applies `resisted_duration`. Not present → push with `stacks: 1`.
  Present:
  - `Refresh`: `remaining = max(old, new)`, `power = max(old, new)`.
  - `Stack{max}`: `stacks = min(stacks + 1, max)`, `remaining = new`,
    `power = new`.
  - `landed_this_round = true` on a freshly pushed entry and on a `Refresh`
    re-apply. A `Stack` re-dose leaves the flag alone: a poisoner re-dosing
    every round would otherwise never deal damage, since each re-flag skips
    that round's tick.
- **`tick_status_effects`** walks every entry at the existing end-of-round
  hook (`tick_one_combatant`, both battle models): landed-this-round entries
  only clear the flag; others apply `DamagePerRound`, decrement, and are
  removed with their `expire` line at 0.
- **Behaviour queries** replace kind checks. One helper —
  `status_sum(entity, |b| …)` or equivalent — answers "how much of behaviour
  X does this body carry", used by:
  - `is_stunned` (`SkipTurn` present),
  - `combatant_profile` (`combat_damage.rs:132`): evasion cut, and new
    attack/mitigation scaling,
  - `combat_policy.rs:230,266`: the existing `stun`/`bleed` features map to
    "move carries / target has a `SkipTurn` / `DamagePerRound` status", so
    trained weights keep meaning.
- **HealBlock** is checked in `Game::restore_hp` (`combat_damage.rs:430`),
  the path every in-battle heal and drain takes; blocked → returns 0 and
  the caller's log reports it. Out-of-battle callers (repair, sortie) never
  see a status, since teardown clears them.
- **Cleanse** clears the whole Vec.

## 3. Views and UI

- `status_effect: Option<String>` on the battle views (`views.rs` ~:1915,
  :1946, :2590) becomes `statuses: Vec<StatusTagView { tag, stacks,
  remaining }>` — data, not a pre-formatted string, so the renderer chooses
  the layout.
- `gui/src/render/battle.rs` `status_tag` draws each, `PSN×3` when
  `stacks > 1`; the tactical map draws the same list beside `TamperTag`.
  Both go through `Painter`; the drawing seam is unaffected.
- The party panel line (`party.rs:950`) lists every active status as
  `"{name} ({remaining})"`, comma-joined.

## 4. Content

| File | Behaviours | Stacking | Notes |
| --- | --- | --- | --- |
| `bleed.ron` | `DamagePerRound` | `Refresh` | current messages ("starts leaking!", "leak is plugged") |
| `stun.ron` | `SkipTurn` | `Refresh` | |
| `exposed.ron` | `EvasionCut(EXPOSED_EVASION_PERCENT's value)` | `Refresh` | |
| `poison.ron` | `DamagePerRound` | `Stack(max: 5)` | new |
| `throttled.ron` | `AtkPercent(-25)` | `Refresh` | new |
| `locked.ron` | `HealBlock` | `Refresh` | new |

`Refresh` for the three existing kinds keeps their single-application
numbers identical; what changes is that they now coexist.

- `data_poisoning.ron` (and its group sibling if one exists) switches
  `Bleed` → `"poison"`; description updated.
- One new Debuff ability each for Throttled and Locked, numbers set by the
  plan against existing Debuff siblings' `power_cost`/`cooldown`, added to
  a Saboteur (Throttled) and a Leech (Locked) species kit. The species-class
  census test must stay green.

## 5. Docs

`assets/statuses/README.md` (new schema); `assets/species/README.md` and
`assets/abilities/README.md` (`kind` is now an id); `docs/content-gaps.md`
:103-105 (new status kinds are now data); `CHANGELOG.md` at release. Seam:
`arm_status` as sole writer and `restore_hp` as the HealBlock chokepoint are
load-bearing — record both via the `seams` skill.

## 6. Tests and gates

- **Unchanged behaviour:** existing bleed/stun/exposed tests pass with only
  the `StatusKind` → id rename.
- **New engine tests:** two different statuses coexist and tick
  independently; `Refresh` takes max of remaining/power; `Stack` adds a
  stack, caps at `max`, scales damage by stacks; landed-this-round exempts
  per entry; `HealBlock` zeroes heal and drain heals; `AtkPercent` and
  `MitigationPercent` reach `combatant_profile`; Cleanse clears all;
  teardown clears all; unknown id → rider dropped with warning; malformed
  status file skipped.
- **Asset census:** every shipped status referenced by at least one move or
  ability; every `StatusId` in assets resolves.
- **Gates:** `cargo test --workspace`, clippy `--all-targets`, `balance_sim`
  unchanged (it models no abilities; any move is a bug).
- **Measurement:** one arena batch with a Poison-stacking species vs a
  baseline, recording whether the trained policy uses it, written up in
  `docs/measurements/` if it clears that directory's README bar.

## Process weight

Three crates (engine, app-core views, gui) and an asset schema change →
spec and plan. No save-format change, so a minor/patch release per
`CHANGELOG.md`'s preamble.
