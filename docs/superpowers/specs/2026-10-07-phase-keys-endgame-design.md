# Phase Keys and the Basin Exit: endgame design

**Date:** 2026-10-07 · **Branch:** `phase-keys-endgame` · **Status:** spec, not built

## Goal

Give the game a storyline ending. The player recovers ten **Phase Keys**,
one from the Stack guardians of each zone 1–10. Each key grants a permanent
benefit the moment it is found (in the spirit of the Bard's Tale 2 pieces).
Holding all ten lets the player build the **Basin Exit**; using it plays an
ending, marks the story complete, and play continues.

The setting is renamed from "the Grid" to **the Phase-Manifold Basin**
("the Basin" once established), to drop the Tron association.

Decisions taken in brainstorming (user's answers, not assumptions):

- Keys gate **both** the Zone Portal (zones 1–10) and the Basin Exit.
- Each key has a unique passive; effects apply immediately on pickup.
- Keys live in a new **Phase Keys tab of the inventory screen**. "Contracts"
  is already the Contract Broker system and is not reused.
- Drop: 33% per eligible guardian kill, guaranteed on the third.
- The escape is a structure; the ending does not end the game.
- **No save migration.** Saves already past zone 1 cannot finish the story.
- Magnitude stats scale by **percent**; chance stats stay **flat points**.
- Modders may change what a key *is*, never whether the game is completable.
- Claude drafts the ten effects; the user iterates on them later.

## Non-goals

- New Game+ or any post-ending carry-over.
- Keys from surface bosses, trade, caravans or markets.
- Migrating existing saves.
- Final balance of the ten effects (numbers below are placeholders).

## 1. Data and storage

### Code owns the structure, data owns the content

Fixed in Rust (`tuning.rs` constants, engine logic):

- Exactly `PHASE_KEY_COUNT = 10` slots, one per zone 1..=10.
- The drop rule, the guarantee, the portal gate and the exit requirement.

Data in `assets/phase_keys/*.ron` (new asset kind, `README.md` schema):

```ron
(
    zone: 3,
    name: "Seam Lens",
    flavour: "Shows where the Basin was stitched together.",
    effect: (
        stat_pct: (),                     // optional, percent of final stat
        stats: (accuracy: 5, crit: 3),    // optional, flat points
        hooks: [],                        // optional, ImplantHook list
        signature: None,                  // optional, ImplantSignature
    ),
)
```

`PhaseKeyDb::load_dir` follows the `*Db::load_dir` pattern, then fills each
slot 1..=10:

- Malformed file: skipped with a logged warning.
- `zone` outside 1..=10: ignored with a warning.
- Two files for one zone: first by file name wins, warning logged.
- Empty slot after loading: a built-in fallback key ("Phase Key N", no
  effect) with a warning. The game stays completable.

### Effect vocabulary

`KeyEffect` reuses the implant vocabulary (`crates/engine/src/implants.rs`)
rather than inventing one:

- `stats`: flat deltas, implant `stats` shape. Intended for the chance
  stats `crit`, `accuracy`, `evasion`, `status_resist`.
- `stat_pct`: **new**. Whole-percent bonuses on the magnitude stats
  `max_hp`, `atk`, `mitigation`, `max_power`, `decompiler`. Applied to
  the player's final stat (after level, gear, implants). Percents from
  all held keys sum before applying (two keys at 5% and 10% give 15%,
  not 1.05 × 1.10). A held key with a nonzero percent adds at least +1.
- `hooks`: `ImplantHook` (`CaptureOdds`, `DropBoost`, `RoutineSlots`,
  `TraceDamp`, `XpBoost`), already percentages or counts.
- `signature`: `ImplantSignature`. A signature also present on an
  installed implant does not double: `DeadMansSwitch` still fires once
  per battle.

Keys cost no Neural Load and no Power upkeep.

All key bonuses are **derived on read** from the held set, never baked
into stored stats (trap: `a-baked-stat-needs-a-receipt`).

### Player state

A `PhaseKeys` component on the player:

- `held`: 10-slot bitset.
- `misses`: eligible guardian kills in the current zone that did not drop.
- `story_complete: bool`.

Saved; `SAVE_FORMAT_VERSION` 35 → 36 (breaking, per `CHANGELOG.md`
preamble). Needs a save→load test, not only a RON round-trip.

### Renderer API

`Game::phase_keys() -> PhaseKeysView`: per slot, held (name, flavour,
one-line effect) or missing (zone only), plus `story_complete`. The
one-line effect is produced by the same formatting function implants use,
called rather than copied.

## 2. Drops and the guarantee

In `award_loot` (`crates/engine/src/game/combat_rewards.rs`), on the
branch that already identifies a Stack lair guardian kill for
`pay_stack_boss_fragments`:

- Eligible when the zone is 1..=10 and its key is not held.
- Drops if the roll is under `PHASE_KEY_DROP_CHANCE = 0.33`, or when
  `misses + 1 == PHASE_KEY_GUARANTEE_KILLS` (3). Otherwise `misses += 1`.
- `misses` clears on breach (breach is one-way and gated on the key, so
  one counter suffices).
- The roll is a deterministic hash of (save seed, zone, kill index), not
  a draw from the shared game RNG, so adding it does not shift the RNG
  stream (trap: `rng-stream-shift-exposes-seed-luck-tests`).
- Surface bosses never drop keys.

On a drop the player gets:

- A **modal alert** they dismiss (the existing popup flow) with the key's
  name, flavour and effect.
- A notification and a message-log line.
- That key's achievement.

## 3. Gating

One function, `phase_key_gate(zone) -> Result<(), Blocked>`, used everywhere:

- **Zone Portal, zones 1..=10:** building it is refused without the
  current zone's key: "Needs the Zone N Phase Key. Guardians at the bottom
  of the Stack carry it." Also checked again on stepping onto the
  portal, so a portal already standing in an old save cannot bypass it.
- Zones 11+: no gate.
- **Basin Exit:** buildable only with all ten keys held, in any zone ≥ 10.

The exit's requirement lives in code, not in its structure file. If
`assets/structures/basin_exit.ron` is missing or malformed, a built-in
definition is used, the same protection as the keys.

### Dev tooling

- `savetool warp` grants the key of every zone it skips past.
- `dev-saves/` templates beyond zone 1 are recaptured with keys held.

## 4. Inventory tab

- The inventory screen gains two tabs, **Items** (today's list) and
  **Phase Keys**. The tab-switch key follows the existing tabbed-screen
  convention. It is not a letter: lowercase letters select rows and
  uppercase are actions.
- Phase Keys tab: always ten rows. Held: name and one-line effect;
  selecting a row shows the flavour. Missing: `Zone N: not recovered`.
  Footer: `N / 10 recovered`; at ten, "the Basin Exit can be built".
- No actions on the tab; it never offers sell, erase or equip.
- Rendering through `Painter` in `crates/gui/src/render/inventory.rs`;
  active tab is app-core inventory state. Rows sized from the longest
  name in data (trap: `gui-text-never-clips-or-wraps`); widths tested
  headlessly.

Keys are not in the normal inventory, so sell, erase, trade, caravan,
market and consume paths never see them.

## 5. Ending and post-story

- Building the Basin Exit does **not** consume the keys; their effects
  stay.
- Stepping on it asks for confirmation: "Leave the Phase-Manifold Basin?
  (The game continues afterward.)"
- On confirmation: a new mode shows the ending text over a few screens.
  The text is an asset file, so it can be modded. Then:
  - `story_complete` is set;
  - the escape achievement is unlocked;
  - the structure is removed;
  - normal play resumes.
- After the ending: play continues as before. A small "Escaped" marker
  appears on the character sheet or HUD. Building the exit again is
  refused.

### Achievements (data, `assets/achievements/`)

- One per key (ten), via a new condition `PhaseKeyFound(zone)`.
- One for all ten keys.
- One for escaping (`StoryComplete`).

## 6. The Grid rename

- About 25 setting uses of "the Grid" in `assets/` (help pages,
  achievements, item and structure text, descriptions) and about 5 crate
  strings and comments. Each page's first mention becomes "the
  Phase-Manifold Basin"; later mentions become "the Basin".
- **Unchanged:** Power Grid, `[GRID]`, Grid Tapping, the Open Grid
  biome, and code identifiers (`base_grid`, `ICON_GRID`, tactical grids).
- `CHANGELOG.md` history is left as written.
- A test asserts no setting-sense "the Grid" remains in `assets/`, and
  that the Power Grid terms are still present.
- Own commit, separate from the key mechanics.
- Also fix the stale `assets/items/core_fragment.ron` text: it says
  fragments do not survive a breach, but they do.

## 7. The ten keys (draft, placeholder numbers)

| Zone | Name | Flavour | Effect |
|---|---|---|---|
| 1 | Origin Vector | The coordinate you woke up at, still warm. | `max_hp +5%` |
| 2 | Drift Compass | Always points toward the edge. | `TraceDamp(10)` |
| 3 | Seam Lens | Shows where the Basin was stitched together. | `accuracy +5`, `crit +3` |
| 4 | Ledger Fragment | A page of the Basin's own accounts. | `XpBoost(10)` |
| 5 | Damping Plate | Absorbs the phase shear between tiers. | `mitigation +10%`, `status_resist +10` |
| 6 | Handshake Echo | Every program it has met still answers. | `CaptureOdds(10)`, `decompiler +10%` |
| 7 | Overflow Shard | Room the Basin didn't know it had. | `RoutineSlots(1)` |
| 8 | Salvage Vector | Leads to what the guardians hid. | `DropBoost(15)` |
| 9 | Phase Spine | You move half a step out of phase. | `atk +8%`, `max_power +10%`, `evasion +5` |
| 10 | Last Boundary | The Basin will not let you go quietly, so it can't let you end either. | `DeadMansSwitch`, `max_hp +10%` |

Base growth alone (`max_hp` 90 + 24/level, `atk` 6 + 2/level), at each
zone's level cap:

| Level | max_hp | key 1 (+5%) | keys 1+10 (+15%) | atk | key 9 (+8%) |
|---|---|---|---|---|---|
| 6 (zone 1) | 210 | +10 | n/a | 16 | n/a |
| 45 (zone 5) | 1146 | +57 | n/a | 94 | n/a |
| 89 (zone 9) | 2202 | +110 | n/a | 182 | +14 |
| 100 (zone 10) | 2466 | +123 | +370 | 204 | +16 |

The user will iterate on names and numbers. The plan includes a pass that
checks them against the per-zone level curve, and a `balance_sim` case
with keys held, so a moved curve shows the progression change.

## Testing

- `PhaseKeyDb`: fallback fill for missing, malformed, out-of-range and
  duplicate files; census test that the shipped `assets/phase_keys/` fills
  all ten slots with no fallbacks.
- Drop: eligibility (zone ≤ 10, not held, guardian only); guarantee on the
  third eligible kill; `misses` clears on breach; determinism (same seed,
  same outcome); the shared RNG stream is unaffected.
- Effects: percent applied to the final stat, summed across keys, minimum
  +1; flat points; hooks; non-doubling signature; derived, not baked.
- Gate: portal build and portal step both blocked without the key in
  zones 1..=10; open in zone 11+; exit needs ten; exit fallback definition.
- Ending: confirmation, flag, achievement, structure removed, rebuild
  refused, play continues.
- Save: v36 save→load preserves held, misses and story_complete.
- UI: tab switching; ten rows; no item actions on the keys tab; row
  widths headless.
- Rename: setting-sense "Grid" absent from `assets/`; Power Grid intact.
- `cargo test --workspace` and `balance_sim` as gates.

## Process

Multiple crates (engine, app-core, gui, launcher's savetool) plus a save
format change: spec-and-plan pipeline. Release is a minor bump with a
breaking save-format note in `CHANGELOG.md`.
