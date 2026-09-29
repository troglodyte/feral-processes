# Level-up stat allocation on derived attributes — plan

Spec: `docs/superpowers/specs/2026-09-29-level-up-stat-allocation-design.md`
(project 1 of 3). Branch: `level-up-stat-allocation`. TDD, a commit per green
step, and every new test is mutation-checked with the fix committed.
`SAVE_FORMAT_VERSION` 32 → 33, so this is a **major** release.

## Shape

There are five phases. Phases 1–4 are one sonnet dispatch each, run serially
because they share files. Every dispatch **forbids push** and stages explicit
paths. Phase 5 is done inline by the controller. Each phase ends with
`cargo check --workspace` compiling, green on `cargo test -p <crates
touched>`, `cargo clippy --workspace --all-targets` and `cargo fmt`. Phases 1
and 4 also run `cargo test -p feral-processes-engine balance_sim`. Per-task
review gates are off. The final review is opus, over the whole branch.

## Anchors (verified 2026-09-29; engine paths under `crates/engine/src`)

| What | Where |
|---|---|
| `AttributeDef` (7 required fields, later ones `#[serde(default)]`), `AttributeDb` | `attributes.rs:78`, `:103` |
| `attributes::mint`, `player_seed` | `attributes.rs:220`, `:199` |
| `Attributes` component | `components.rs:1887` |
| Player attributes minted at creation; load fallback mint | `game/creation.rs:274-288`; `game/lifecycle.rs:1532-1560` |
| `PLAYER_BASE_STATS`; `HP_PER_LEVEL`/`ATK_PER_LEVEL`; `BASELINE_GROWTH_MULTIPLIER` | `tuning.rs:35`; `:196`/`:197`; `:203` |
| `CREATION_STAT_POINTS`, `CREATION_COST_*`, `CREATION_GAIN_INTEGRITY` | `tuning.rs:64`, `:68-93`, `:98` |
| `DECOMPILER_SKILL_PER_LEVEL` (and doc references to it) | `tuning.rs:445`; `tuning.rs:178`, `taming.rs:241` |
| `LevelGain`, `stats_after_levels`, `add_xp` | `progression.rs:27`, `:213`, `:255` |
| `add_xp` callers | `game/combat_rewards.rs:1015` (player), `:1157`; `systems.rs:1506`; `arena/mod.rs:98` |
| `award_player_xp`; `XpTally` | `game/combat_rewards.rs:997`; `resources.rs:1484` |
| `CharacterChoice` (`stats: [u32; 4]`), `apply_creation_stats` | `game/creation.rs:33`, `:223` |
| `MainStat` + `all()` | `achievements.rs:92` |
| `BoughtStats`; its writers | `components.rs:3045`; `game/unlocks.rs:225`, `game/talents.rs:226`, `game/respec.rs:170` |
| `emulated_base` | `game/kit.rs:97` |
| `retier_rarity` scales `Stats` (programs only today; leave it) | `game/combat_teardown.rs:711` |
| `PowerReserve` + its internal `POWER_MAX` clamps | `components.rs:196-265` |
| Non-test `POWER_MAX` readers | gui `render/stack.rs` 15, `base.rs` 5, `battle.rs` 4, `terrain.rs` 2, `hud/log_frame.rs` 2; engine `save.rs`, `systems.rs`, `lib.rs`, `combat_round.rs`, `lifecycle.rs` |
| `arm_status` (the only arming writer) | `game/combat_status.rs:64` |
| `mining_success_chance`; `resolve_gather_cycle`/`CycleModifiers`; outpost caller | `systems.rs:251`, `:417`; `game/outposts.rs:441` |
| `attention`; the Perk Points `'p'` rows | `game/inspection.rs:1733`, `:1850` |
| `PlayerSave` stat fields, `attributes` | `save.rs:18-35`, `:235`; `SAVE_FORMAT_VERSION` `save.rs:1918` |
| `balance_sim` player growth sites (`:285` is level 1, unchanged) | `balance_sim.rs:472`, `:761` |
| Arena player levelling | `arena/mod.rs:85-105`; scenario `arena/scenario.rs:170-215` |
| app-core creation: `stat_cost`, `stat_value`, roll spread, Points rows, leave refusal, key handler, `spend_on_row` | `app-core/src/app/creation.rs:114`, `:127`, `:172`, `:345`, `:569`, `:532`, `:889` |
| `CreationRow::Stat`; `Mode` (`Copy`, unit variants) | `app-core/src/lib.rs:1396`, `:1436` |
| `handle_level_up_key`; LevelUp opens; `handle_perks_key` | `app/level_up.rs:16`; `app/lifecycle.rs:520`; `app/progression.rs:9` |
| gui Points row; level-up page | `gui/src/render/creation.rs:212`; `gui/src/render/level_up.rs` |

## Where the spec is wrong against the code, or left a gap

1. **The spec's "direct `StatusEffects` write at `combat_round.rs:1763`" is
   `AbilityEffect::Cleanse`, which *clears* a status.** `arm_status` is
   already the only writer that arms one. `combat_status.rs:110` is the tick
   clearing `landed_this_round`. Nothing needs rerouting. Resist lives in
   `arm_status` only.
2. **`Reward::RandomMainStat` writes player `Stats`/`Decompiler` directly**
   at new-run start (`game/lifecycle.rs:3276-3295`). Recompute would erase it.
   **This needs the user's decision (see D1).** The spec does not mention it.
3. **Player attributes are minted *with* spread** (`creation.rs:283`, and
   the load fallback). The spec says the player gets no spread. Add
   `attributes::authored_or_base(db, authored) -> Attributes` and use it at
   both player sites. `mint` stays for programs. Delete `player_seed` if it
   ends up with no callers.
4. **Recompute vs companions.** `BoughtStats` writers (perks, talents,
   respec) also serve companions, which have no `Derived` in this project.
   So `recompute_derived(e)` is a **no-op without `Derived`**, and those
   writers stay as they are and add a `recompute_derived` call afterwards.
   For the player the result equals what they wrote, so recompute is
   idempotent. **Emulation:** while the player emulates, recompute takes
   atk/mitigation from `emulated_base` (whatever marker `kit.rs` reads), not
   from derive.
5. **The arena needs a player spend.** Today the arena levels the player with
   `Auto` growth. The spec's arena gate compares spends, so a `Fresh` arena
   player levels with `Points` and spends `ArenaScenario.player_spend:
   Option<BTreeMap<AttributeId, u32>>`. That is a **per-level** pattern, and
   `None` means `canonical_spend`. A pattern that does not sum to
   `STAT_POINTS_PER_LEVEL`, or that names a non-buyable attribute, is a
   scenario **error**, never silently ignored (the arena-fields-fail-silently
   trap).

### D1: what `RandomMainStat` becomes (needs the user's decision)

- **(a) Recommended: bank `n` stat points** at run start. This fits the
  feature, because the player chooses. The profile's rolled `MainStat` is
  then dead, so delete the roll, `roll_main_stat`, the views field
  (`views.rs:2971`) and `MainStat` if nothing else uses it.
  `profile.ron`'s old field must still parse (check it is not
  `deny_unknown_fields`). Update `tuning.rs:71`/`:3430`.
- (b) Keep the roll and map each `MainStat` to an attribute, raising it by
  `n`. Integrity→Parity then pays 6n HP instead of n, so the values need
  retuning.

The plan assumes (a). Only phase 1 task 1.7 changes if (b) is chosen.

## Phase 1: engine derive, recompute, levelling (sonnet)

1.1 **Schema and assets.** `AttributeEffect { stat: DerivedStat, per_point:
f32 }`, and `DerivedStat` with 7 variants (spec §Engine). `AttributeDef`
gains `#[serde(default)] effects` and `#[serde(default)] does: String`. Add
`assets/attributes/analysis.ron` (base 10, spread 3). Add effects and `does`
to parity, bandwidth, footprint, analysis and persistence (the values are
the spec's table). Add `AttributeDef::buyable()` (effects non-empty). Tests:
every shipped file parses; Entropy is not buyable; a file with no `effects`
still parses.

1.2 **`progression::derive(&DerivedBase, &Attributes, &AttributeDb) ->
DerivedStats`**, plus `DerivedBase::player()` from the tuning constants. Add
the new constants to `tuning.rs`: `MIN_MAX_POWER`, the StatusResist range,
`MINING_EXTRACTION_CAP` 0.10 and `STAT_POINTS_PER_LEVEL` 6, with the
compile-time assert against `HP_PER_LEVEL`/`ATK_PER_LEVEL`/
`DECOMPILER_SKILL_PER_LEVEL` taken *before* that last constant is deleted in
1.5. Also add `progression::canonical_spend(levels) -> BTreeMap<AttributeId,
u32>`. Tests (spec §Testing, engine bullets 1–2): at catalogue bases derive
equals `PLAYER_BASE_STATS`; each effect moves only its own stats; rounding
happens once per stat; clamps hold; an absent attribute counts as its base.

1.3 **`Derived` component, `recompute_derived`, `Game::max_power`.** Follow
spec §Recompute and gap 4. Insert `Derived` on the player at spawn/creation.
Use `authored_or_base` (gap 3). Tests: correct with gear worn; `BoughtStats`
is not double-counted; recompute is a no-op on a companion; recompute is
idempotent after a perk purchase; `hp` and Power are clamped and never
refilled; emulation keeps its atk/mitigation.

1.4 **Call sites.** Call recompute after the perk, talent and respec
`BoughtStats` writers and after creation. Creation then sets `hp = max_hp`.
Leave creation's *spend* on the old `[u32; 4]` for now: phase 4 replaces
it. The load path calls recompute too (full save work is phase 3).

1.5 **`Growth { Auto { multiplier }, Points }`** on `add_xp`, and
`LevelGain::stat_points`. `award_player_xp` passes `Points`, adds to a new
`StatPoints(u32)` component, and drops its Decompiler grant. Delete
`DECOMPILER_SKILL_PER_LEVEL` and fix the docs that cite it. `XpTally` carries
stat points in place of decompiler. Companions, workers and sorties pass
`Auto`. Arena: gap 5. **Expect a wave of engine tests that assumed the
player grows on level-up.** Rewrite each one to assert banked points, or to
spend `canonical_spend` via 1.6. Do not delete them.

1.6 **`Game::spend_stat_points(StatOwner, &[(AttributeId, u32)]) ->
Result<DerivedStats, SpendError>`** (spec §Spending). `StatOwner` lives in
`progression.rs`, re-exported. Also add `Game::attention` row `'p'` naming
`S`. Tests: an overspend writes nothing; `NotBuyable`/`NoSuchTarget`;
`BoughtStats` is untouched; a perk respec does not refund a spend; attention
flags unspent points.

1.7 **D1** (as decided).

1.8 **`balance_sim`:** the player sites use `PLAYER_BASE_STATS` plus
`derive(canonical_spend(n))`. Add the equality test against the old
`stats_after_levels(PLAYER_BASE_STATS, n, BASELINE_GROWTH_MULTIPLIER)` for
every n in 0..=cap. `balance_sim` must be green with **no curve moved**.

## Phase 2: effects (sonnet)

2.1 **Max Power per entity.** `PowerReserve`'s clamping methods (`new`,
`restore`, `fill`, `raise_to_at_least`) take `max: f32`. Engine callers pass
`Game::max_power(e)`. Systems that have no `Game` query `Option<&Derived>`
through one free helper that both call, not a copy. Views carry `max_power`,
and gui bars read it from the view. Afterwards `POWER_MAX` is left only in
`tuning`/`components` as the fallback. Tests: max Power follows Bandwidth,
and the reserve clamps when it drops.

2.2 **Status resist** in `arm_status` (spec formula, floor 1, no RNG).
Tests: resist shortens a status, negative resist lengthens it, the floor
is 1, and a companion without `Derived` is unchanged.

2.3 **Extraction.** `CycleModifiers` gains `extraction: f64`, and
`mining_success_chance` gains a trailing `extraction` argument, clamped by
`MINING_EXTRACTION_CAP`. `player_gather_system` passes `Derived::extraction`
with `base_int = DEFAULT_BASE_INT`. Programs and outposts pass 0.0. Test
call sites get `0.0` mechanically. Tests: Extraction raises the player's
chance, it is capped, and programs are unchanged.

## Phase 3: save v33 (sonnet)

`SAVE_FORMAT_VERSION` becomes 33, with a doc line in the version history.
`PlayerSave` drops `max_hp`/`atk`/`mitigation`/`decompiler`, keeps `hp` and
`power`, and gains `stat_points`. Load calls recompute, then restores `hp`
and power clamped. Required: a **save→load** test (not a RON round-trip) that
keeps attributes, points and derived values, with gear worn and a perk
bought. Regenerate every `dev-saves/` template (`savetool` `capture` or the
existing regen path), then check each one loads.

## Phase 4: creation, Points screen, flow, gui (sonnet)

4.1 **Engine creation:** `CharacterChoice.stats: BTreeMap<AttributeId,
u32>`, and `cost()` uses `CREATION_COST_PER_ATTRIBUTE_POINT`. Delete the
`CREATION_COST_*` table and `CREATION_GAIN_INTEGRITY` if they end up with no
callers. `apply_creation_stats` raises attributes, then recomputes, then
fills `hp`. Tests: creation commits the expected attributes, and an
overspent choice gets no spend.

4.2 **app-core `StatAllocation`** (spec §App-core), with
`App.stat_allocation`. Add `CreationRow::Attribute` replacing `Stat`. The
preview calls `derive`: delete `stat_value`'s copy. The roll spread spends
exactly the pool over buyable attributes. Add `Mode::AllocateStats` and wire
it into `all_modes` (mind the `all-modes` traps). Flow per spec §Flow, the
Perks `S` refusal via `App::refuse`, and the Level Up report after-column.
Tests: spec §Testing app-core bullets.

4.3 **gui:** the Points drawing takes the `StatAllocation` view, and
creation and `AllocateStats` share it. Rows show name + legacy, value, and
effects before→after. All drawing goes through `Painter`.

## Phase 5: controller, inline

1. Run `cargo test --workspace` once, then `balance_sim`.
2. **Arena gate:** read `docs/measurements/README.md` and
   `2026-09-01-creation-stat-pool-exchange-rates.md` first. At levels 10, 15
   and 20, compare the canonical spend with all-Footprint, all-Parity and
   all-Bandwidth spends, and compare each class's level-1 start. Record the
   results in `docs/measurements/`. **If one spend or class dominates, stop
   and bring the numbers to the user.**
3. `--screenshot` the level-up Points screen and the creation Points screen,
   then Read both PNGs.
4. Docs: `assets/attributes/README.md` (effects, `does`, the buyable rule,
   the relaxed `meaning` rule) and `assets/classes/README.md`. The CHANGELOG
   entry is written at release, not on the branch.
5. Final opus review over the whole branch, with the diff given as a file.
   Ask it to re-derive the canonical-spend equality and the recompute order
   independently.
