# Item quality — phase roadmap

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement each phase task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Give every carried copy of gear a fourth axis — how well *this
copy* was compiled — whose inputs are things the player builds, so gearing
becomes a base activity rather than a loot-table lottery.

**Architecture:** A `u8` percentage on `items::GearCopy`, defaulted to 100
so every existing save is unchanged. `EquipmentStats::for_quality` joins
the three scaling axes inside `Game::copy_bonus`, third of four. Crafting
rolls it from a floor built out of bench tier, a perk, a careful-compile
toggle and luck; drops roll it from a poorer flat floor. The renderer reads
a band off a pure engine function and paints the category tag with it.

**Tech Stack:** Rust, `bevy_ecs` 0.19 (engine), `bevy` + `bevy_egui` (gui),
RON assets, `serde`.

**Spec:** `docs/superpowers/specs/2026-08-21-item-quality-design.md` — read
it alongside this plan; every decision below argues from it.

## Global Constraints

Copied verbatim from the spec. Every task's requirements implicitly include
this section.

- **The field is an integer.** `GearCopy` is the key of the `GearCopies`
  ledger (`add`/`count`/`take` find rows by `==`) and `EquippedItem` holds
  the same key. An `f32` breaks `Eq`. `u8` percent.
- **The default is 100, not 0** — `#[serde(default = "default_quality")]`,
  never a bare `#[serde(default)]`, which would load every existing save's
  gear at 0% and read as a balance bug.
- **No `SAVE_FORMAT_VERSION` bump.** Additive `#[serde(default)]` field on
  a *named* struct. A RON round-trip cannot catch a defaulting fault, so
  the proof is a real save → load test.
- **Chain order in `copy_bonus` is load-bearing:** affix folded into base →
  `scaled_for_level` → `for_quality` → `fused_for_tier` → `for_rarity`.
- **`for_quality` carries no floor**, unlike the two axes after it. A stat
  sitting at zero stays at zero; a negative component (a drawback affix) is
  left where it is.
- **The roll is one formula, one clamp**, every term a named constant in
  `tuning.rs`: `floor = QUALITY_BASE + bench + perk + care`, then
  `clamp(floor + roll(0..=QUALITY_SPREAD), QUALITY_MIN, QUALITY_MAX)`. The
  spread is drawn **in steps** of `QUALITY_STEP`, never drawn fine and
  rounded.
- **Only items with an `equipment` def roll quality.** A non-equippable
  spends **no** `GameRng` draw — the property `grant_gear_drop` already has.
- **World generation rules still bind:** nothing here may draw from
  `resources::GameRng` outside the two roll sites, and the crafting roll
  runs on `GameRng` deliberately (it is a player action, not world gen).
- **`Game::copy_name` stays the one place a copy's name is built**, and
  `Game::copy_bonus` the one expression for what gear is worth.
- **The engine buckets, the renderer paints:** a pure `items::quality_band`
  owns the thresholds; the colour and weight are the renderer's.
- **No new module.** `for_quality` and `quality_band` sit in `items.rs`
  beside their siblings. The roll split in two when Phase 2 landed: the
  shared spread-and-clamp is `Game::roll_quality` in `game/spawning.rs`
  beside `roll_gear_rarity`, because two files roll the same axis and the
  ladder belongs where both reach it; Phase 3's floor assembly out of a
  `CraftOrder` still sits in `game/crafting.rs` beside `craft`.
- **`balance_sim` is outside this**, by the same documented exclusion that
  keeps `Rarity` out of it. If its curves move, the exclusion is wrong, not
  the test.
- **Every new test gets the mutation check** — delete the fix, watch the
  test fail, restore. A green test that passes without its fix is the
  failure mode this repo has been bitten by.
- **Docs:** `docs/seams.md` gets the argument, `CLAUDE.md` gets the rule,
  and `CLAUDE.md` is copied over `AGENTS.md` in the same commit. Do **not**
  touch `docs/manual.md`, root `README.md` or `TODO.md`. `CHANGELOG.md` and
  the version bump happen once, at the merge, not per phase.

## Gates for every phase

```sh
cargo fmt
cargo clippy --workspace          # fix warnings, don't silence them
cargo test --workspace            # the final gate for the phase
```

Iterate inside a phase with `cargo test -p feral-processes-engine <name>`.
Note that a single-crate run and a `--workspace` run are different builds
and can shift a seeded RNG stream — a seeded test that fails in one and
passes in the other is that, not a regression.

## The phases

Each phase is a shippable, testable chunk that leaves the game green and
coherent. Task-level detail is written into its own file when the phase
starts, so no one pays for detail they are not executing yet.

| # | Phase | Deliverable | Plan file |
| --- | --- | --- | --- |
| 1 | ✅ **The axis** | The field, its default, `for_quality` in the chain, `quality_band`, the save-load guard. Nothing rolls it yet, so no behaviour changes. | `2026-08-21-item-quality-phase-1.md` |
| 2 | ✅ **Drops roll it, and it reads** | `QUALITY_DROP_BASE`, the stepped spread through `Game::roll_quality`, the figure in `copy_name`, the row on the gear inspect page, and the swap row's stat column lifted out of its un-wrappable head — which the roadmap did not price and measurement forced. | `2026-08-21-item-quality-phase-2.md` |
| 3 | ✅ **Crafting rolls it** | `CraftOrder`, `Game::best_structure_tier`, the bench on `CraftRecipe`, per-unit rolls routed through `add_copies`, the careful-compile toggle through app-core and the Compile screen — plus the upgrade path the two compile benches turned out not to have, without which the bench term was inert on every shipped recipe. | `2026-08-21-item-quality-phase-3.md` |
| 4 | **The perk term** | One *appended* `Perk` variant, its hook in the roll, its `assets/perks/*.ron`. `CraftOrder` gains its `perk_level` field here — Phase 3 shipped without it rather than carrying a term that was always 0. | phase-4 |
| 5 | **The tag column** | `Row::Item::tag` as a reserved, painted column; the five hand-formatted tag sites lifted into it; the four-band emphasis ramp; the width tests re-baselined. | phase-5 |

**Why this order.** Phase 1 is inert by construction — every copy is still
100 — so it can land and be reviewed on the seam alone. Phase 2 makes the
axis visible, because names are already built in one place. (It was
expected to need *no* UI restructuring; measurement said otherwise — the
quality figure's seven cells push the swap row's head 35.6px past its
popup, so the stat column had to become a shed-able tag. The numbers are
in that phase's plan and in `docs/seams.md`.) Phase 3 is the design intent (a base out-produces the world) and
is the phase to play before calling the numbers correct. Phase 4 depends on
Phase 3's `CraftOrder`. Phase 5 is presentation and touches the most
renderer sites, so it goes last where a re-baseline cannot mask a
behavioural fault.

**Stated balance consequence, to be felt before it is called correct:**
with `QUALITY_BASE` at 80, early crafted gear is *weaker* than today, where
every craft is exactly 100. `QUALITY_BASE = 90` is the softer variant. The
instruments are `dev-arenas/` and a session; arena numbers compare within
one build only.
