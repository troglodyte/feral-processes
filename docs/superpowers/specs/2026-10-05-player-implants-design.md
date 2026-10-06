# Player Implants — design

**Date:** 2026-10-05 · **Status:** spec, reviewed; plan written · **Weight:**
spec-and-plan (engine + app-core + gui, new content kind, new structure, new
save field)

## Intent

Things the player builds into their own avatar. Everything else that raises the
player is either swappable (gear, which programs can also wear) or only ever
adds (perks, Stat Points). An implant is the one axis that **trades**:
always on, paid for continuously, and expensive to take back out.

What the user decided:

- **Limit: a Neural Load budget**, not slots. Each implant costs Load; a cap
  grows with level. Exceeding the cap is allowed and has consequences.
- **Every implant drains Power**, scaled by its Load. A further downside is
  **optional per implant**. Rejection glitches happen only while over the cap.
- **Crafted at the base** from what the player's own lines produce.
- **Installed and removed at a new structure**. Removal is possible and costs
  something.
- **Effects are both data and code:** stat deltas and hooks from a closed list
  that a `.ron` file combines freely, plus an optional perk-style
  **signature** variant for behaviour the list cannot express.
- **About six implants** in the first set.

Out of scope: implants on programs (talents are their axis), implants from
the Stack or towns, grafting from decompiled programs, cross-run persistence
(achievements stay the only thing that outlives a run).

## Components

### 1. Content kind — `assets/implants/<id>.ron`

```ron
(
    id: "ripper_fibers",
    name: "Ripper Fibers",
    description: "…",
    load: 2,
    stats: (atk: 3),                      // ImplantStats, every field defaulted
    hooks: [],                            // Vec<ImplantHook>, closed list
    signature: None,                      // Option<ImplantSignature>, closed list
    downside: None,                       // Option<ImplantDownside>, closed list
)
```

- `ImplantDef` / `ImplantDb::load_dir` follow the `*Db::load_dir` pattern:
  a malformed file is skipped with a logged warning. Deleting the directory
  leaves the game valid and inert. Schema in `assets/implants/README.md`.
- `ImplantId` is a string newtype like `ItemId`.
- **An implant is also an item.** The crafted thing is an `ItemDef` in
  `assets/items/` (a recipe needs an item result, and it sits in `Inventory`
  until installed), linked by id: the item `ripper_fibers` installs the
  implant `ripper_fibers`. A census in `tests/assets.rs` asserts every
  implant has its item and every implant item has its implant. Open for the
  plan: a field on `ItemDef` (`implant: Option<ImplantId>`, `#[serde(default)]`)
  versus the id convention alone. A field is preferred, since "never
  assume" beats a naming rule.
- **`stats`** are absolute deltas over the axes `recompute_derived` already
  writes (`max_hp`, `atk`, `mitigation`, `max_power`, `crit`,
  `status_resist`, `decompiler`) plus the accuracy/evasion pair gear
  carries. A negative value is legal. This is how a stat-penalty downside
  is written when it needs no separate concept.
- **`hooks`** (closed enum, variant order is save-neutral because it lives
  in data, not in `PlayerSave`): `CaptureOdds(pct)`, `DropBoost(pct)`,
  `RoutineSlots(n)`, `TraceDamp(pct)`, `XpBoost(pct)`. The first set uses
  three of these. Each is read at the existing seam where its field-buff
  twin or perk is already read; see §4.
- **`signature`** (closed enum, perk-pattern): one named query per variant
  in `implants.rs`, and a test that fails to compile if a variant has none
  (the same guard `perks.rs` uses). First variant: `DeadMansSwitch`.
- **`downside`** (closed enum, optional):
  - `TraceRise(pct)`: Trace rises faster.
  - `BattleStartStatus(StatusId, chance)`: rolled at battle start.
- Magnitudes the *design* fixes (cap curve, drain per Load, rejection odds,
  removal price per Load) are `tuning.rs` constants. Magnitudes an implant
  *is* (its `+3 atk`, its `20%`) are data, as gear's are.

### 2. Player state — `Implants` component

- `Implants { installed: Vec<ImplantId> }` on the player only, spawned
  empty in `spawn_player`. The spawn bundle is at bevy's 15-element tuple
  limit, so it goes into the inner tuple.
- `PlayerSave::implants: Vec<ImplantId>`, `#[serde(default)]`. An old
  save loads with none. The plan confirms against `save.rs`'s own notes
  whether this needs a `SAVE_FORMAT_VERSION` bump.
- An id whose def has gone missing (mod removed) is kept in the save but
  contributes nothing and costs nothing. It shows in the UI as unknown, so it can
  still be removed.

### 3. Neural Load

- `Game::neural_load(player) -> u32`: the sum of installed `load`.
- `Game::load_cap(player) -> u32`: `tuning::IMPLANT_LOAD_BASE` (4) plus
  `level / tuning::IMPLANT_LOAD_LEVELS_PER_POINT`. Two or three implants
  early, all six around the mid-game level band. Exact numbers come from the plan,
  checked against `balance_sim`.
- **Overload** = `load - cap` when positive. Installing past the cap is
  allowed, with a confirm that names the rejection risk.

### 4. Where effects are read (one seam each)

| Effect | Seam | Note |
|---|---|---|
| `stats` | `Game::recompute_derived` (`game/derived.rs`) | Added beside the `BoughtStats` receipt, before gear goes back on. Install/remove calls `recompute_derived`. |
| Power drain | `needs_tick_system` (`systems.rs`) and the crafting-side reader of `power_drain_multiplier` (`game/crafting.rs`) | One pure function `implants::power_drain_add(load) -> f32` both call. Additive to the perk multiplier's product, so `LowPowerMode` cannot zero implant upkeep. Open for the plan: confirm that is wanted. |
| `CaptureOdds` | where `CaptureBoost` field buff is folded into `taming::capture_chance` | |
| `DropBoost` | `combat_rewards.rs` beside `FieldBuffKind::DropBoost` | Stack-only for Black Ledger? See open questions. |
| `RoutineSlots` | `Game::routine_slots` player arm (`game/combat.rs`) | Added past the clamp, like the class bonus. |
| `TraceRise` / `TraceDamp` | `Game::raise_trace` (`game/trace.rs`) | Where sources meet, beside `Obfuscation`. Obfuscation's floor-at-1 still applies after. |
| `BattleStartStatus`, rejection | `Game::begin_battle` (`game/combat.rs`), through `arm_status` + `log_status_landing` | One roll site for both. |
| `DeadMansSwitch` | the site where a hit takes the player to 0 HP; the plan locates it | Once per battle; a per-battle flag, not saved. |

If a hook turns out to need the same check at several sites, the seam is
wrong. Stop and move it (`content-schema.md`).

### 5. Rejection

At battle start, while overloaded: chance
`min(overload × tuning::REJECTION_CHANCE_PER_LOAD, tuning::REJECTION_CHANCE_MAX)`
to arm one status picked uniformly from `tuning::REJECTION_STATUSES`
(proposed: `throttled`, `exposed`, `stun`), for that battle only. Status
resist (Persistence) applies as it does to any status. No permanent harm.
Rejection is a gamble the player chooses, not a slow trap.

### 6. Station — Splice Rig

- `assets/structures/splice_rig.ron`: a gating structure like the Mod Bench
  (`work: None`, small `power_draw`). One anywhere in the base is enough.
- Research node `wetware_splicing` (requires a low-tier node, zone 1–2):
  unlocks the Splice Rig and the first three recipes. A second node
  (zone 3, requires the first) unlocks the other three. Recipes cost
  materials from the player's own production lines and are gated by zone like gear
  (the materials censuses apply).
- **Install** consumes the implant item from `Inventory`.
- **Remove** returns the item and costs `core_fragment ×
  (load × tuning::IMPLANT_REMOVAL_FRAGMENTS_PER_LOAD)`. Install materials
  are never refunded. Every refusal (no rig, no fragments, unknown id)
  happens before anything is spent, per the "one door" rule.
- `Game` API: `implant_view()`, `install_implant(item)`,
  `remove_implant(id)`. The renderer never touches the world.

### 7. UI

- app-core: a Mode for the Splice Rig screen, opened by walking up to the
  rig, the way the Mod Bench opens.
- gui: one screen listing installed implants, Load used/cap (overload in
  warning colour), per-implant upkeep and downside, and installable items
  from cargo. Drawing goes through `Painter` only (`drawing-seam.md`).
- The player's stat/Points screen shows the Load meter. HUD: unchanged
  unless overloaded (one glyph). Plan decides.
- Help page in `assets/help/`.

## First set

| Implant | Load | Benefit | Kind | Extra downside |
|---|---|---|---|---|
| **Ripper Fibers** | 2 | +ATK | stats | none |
| **Dermal Lattice** | 3 | +mitigation, +max HP | stats | −evasion (negative stat) |
| **Ghost Handshake** | 2 | `CaptureOdds` | hook | `TraceRise` |
| **Black Ledger** | 2 | `DropBoost` | hook | `TraceRise` |
| **Overclock Spine** | 4 | `RoutineSlots(1)` | hook | `BattleStartStatus(throttled, 20%)` |
| **Dead Man's Switch** | 3 | once per battle, survive a lethal hit at 1 HP (costs Power) | signature | none |

Total Load 16. Names checked against `assets/research` (`neural_amp` is
"Neural Interfacing", `cortex` is "Cortex Hacking"; neither collides).

## Testing

- Unit tests per seam in §4. A failing test comes first for each.
- `implants.rs` pure functions: cap curve, drain, rejection chance.
- Install/remove round trip: stats restored exactly, item returned,
  fragments spent, refusals spend nothing.
- Save round trip with implants, an old save loading with none, and a save
  naming a missing implant.
- `tests/assets.rs` census: six implants, every implant ↔ item link,
  recipes zone-gated.
- `balance_sim` must not move: implants are opt-in and not in its loadouts.
  Add an implant-loadout curve only if the plan finds a reason.
- Seeded RNG only, for rejection and `BattleStartStatus` rolls.

## Decisions (2026-10-06)

The user answered the open questions: `DropBoost` applies only in the Stack;
implant upkeep is multiplied by `LowPowerMode`, not added; items link through
`ItemDef::implant`; the seam is written last. Checking the spec against the
source corrected §1 and §4 (accuracy/evasion, where the deltas are added), §6
(recipes live in research nodes) and §7 (the rig opens by adjacency, like the
Teardown Rig). All of these are in
`docs/superpowers/plans/2026-10-06-player-implants.md`, which takes precedence
where it differs.
