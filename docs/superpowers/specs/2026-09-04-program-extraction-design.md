# Program extraction

**Status:** phases 1, 2, 3 and 5 shipped; phase 4 (§10) approved,
unimplemented. Git and `docs/superpowers/INDEX.md` are the authority on
what landed — this line is a signpost, not a record.

Kills stop paying materials directly. A defeated wild program is left as a
carried, instanced **downed program**; carried **tools** extract materials,
parts and routines from it. The extraction replaces `SpeciesDef::
work_resource`'s drop; gear drops are untouched until phase 5.

## Decisions taken

Recorded so they are not relitigated. Each was chosen against a named
alternative.

1. **A downed program is instanced, not a stackable item.** It carries the
   kill's species, level, rarity, boss flag and condition, so a level-30
   Prismatic kill is not interchangeable with a level-2 one. Rejected: an
   `ItemId` per species (stacks, but every kill of a type is identical).
2. **It lives in a third store, not in `Inventory`.** `Inventory` is
   `Vec<(ItemId, u32)>` whose `count`/`take` read the *first* matching row
   (`components.rs:549`, `:573`), and `components.rs:477` states the seam:
   `Inventory` is by definition the plain-copy store, which is what lets
   recipes, `Stock`, `assembler_system`, hauling and banking read it with no
   instance rule. `GearCopies` is the existing sibling for exactly this
   problem; `DownedPrograms` is the second.
3. **One tool consumes the whole program.** Choosing the tool *is* the
   decision. Rejected: layer-stripping (richest, most per-program state).
4. **The tool decides what kinds come out; the program's quality decides how
   good and how many; the tool's tier decides both again.** No tool x species
   table.
5. **Species contributes one bonus part**, `rich_in`, defaulting to that
   species' existing `work_resource` — zero new authoring across the 17
   shipped species, and the already-balanced mapping survives.
6. **Tools mirror routines rung for rung** (catalogue, component, slot
   formula, research unlock, forge, install). The pattern is proven here and
   documented; a second shape for the same idea is a second thing to get
   wrong.
7. **Extraction works anywhere** — field, base, Stack. A structure improves
   yield and speed; it is not a gate. The starting tool is useless otherwise.
8. **Phase 1 is drop-neutral by construction.** The starter tool's expected
   yield per kill is tuned to today's `WORK_RESOURCE_DROP` expected value.
   Replacing material drops routes the whole early economy through one
   untested door; a mistuned starter tool is a dead run, not difficulty.
9. **A full store refuses the drop with a log line**, rather than discarding
   the worst held program. Never destroy something the player chose to keep.
10. **"Extract" is the verb.** `Game::extract_routine` already breaks a
    program down with that word; "decompile" is taken (it means capture).

## 1. The object

```rust
// crates/engine/src/items.rs, beside GearCopy
pub struct DownedProgram {
    pub species: SpeciesId,
    pub level: u32,
    pub rarity: Rarity,
    pub boss: bool,
    pub condition: u8,   // 0..=100
}
```

`components::DownedPrograms(pub Vec<DownedProgram>)`, player-only, capped at
`tuning::MAX_DOWNED_PROGRAMS`.

**Condition** is rolled once, at the kill:

```
condition = clamp(CONDITION_BASE
                  + CONDITION_PER_RARITY_STEP * rarity.index()
                  + boss as u8 * CONDITION_BOSS_BONUS
                  + FIGHT_CONDITION_WEIGHT * overkill_term,
                  0, 100)
```

`overkill_term` is how far the killing blow went past zero as a fraction of
`max_hp`, negated — a clean kill leaves more. `FIGHT_CONDITION_WEIGHT` is a
`tuning.rs` constant and **may be 0.0**, which switches the fight axis off
without removing the field. Ships at 0.0 until played.

`DownedProgram::grade() -> f32` is the one derivation of "how good is this
program", folding condition, rarity and level. Every yield formula calls it;
none re-folds the axes.

## 2. Tools

New moddable catalogue `assets/tools/*.ron`, loaded by `ToolDb::load_dir`
following `AbilityDb`'s pattern exactly — malformed file skipped with a
logged warning, absent directory loads silently empty, `iter` sorted by id.

```ron
(
    id: "salvage_clamp",
    name: "Salvage Clamp",
    description: "Prises the loose material off a downed process.",
    category: Materials,                 // Materials | Parts | Cores | Routines
    yields: [("core_fragment", 1.0), ("bytecode_block", 0.4)],
    tier: 1,
    ticks: 20,
)
```

`category` is a fixed enum — it groups the tool screen and is what "a tool
reaches a different part" means. `yields` is `(ItemId, weight)`; weights pick
which item per unit drawn. A `Routines`-category tool has an empty `yields`
and takes the routine branch instead.

**Slots.** `components::Tools(pub Vec<ToolId>)` on the player, bounded by
`tools::player_tool_slots(level)` — the same shape as
`abilities::player_routine_slots`, with `TOOL_SLOT_BASE`,
`TOOL_SLOT_PER_LEVEL`, `TOOL_SLOT_CAP` in `tuning.rs`. No Power cost.

**Acquisition**, mirroring routines:

| Routines | Tools |
|---|---|
| `ResearchDef::unlocks_abilities` | `ResearchDef::unlocks_tools` |
| `KnownRoutines` | `KnownTools` |
| `etch_disk` -> `ItemId::etched(id)` | `forge_tool` -> `ItemId::tool(id)` |
| `install_disk` consumes into a slot | `install_tool` consumes into a slot |

`ItemId::etched` is already a synthetic id with no `.ron` behind it, so tool
carrier items need no item files either. `uninstall_tool` does not hand the
carrier back — what is in the slot *is* the tool, `install_disk`'s rule.

The starter tool (`tuning::STARTER_TOOL_ID`) is forged into slot 1 at
`Game::new`, the way `grant_starting_program` works. It is granted at
creation and never at `Game::load`, the profile rule.

## 3. Yield

```
Game::extraction_yield(&DownedProgram, &ToolDef, structure_tier: u32)
    -> Vec<(ItemId, u32)>
```

One derivation, called by the act and by the preview the screen draws — the
`BuildOrderRow` rule, so a quoted figure and a granted figure cannot differ.

```
units = round(TOOL_BASE_UNITS * tier_scale(tool.tier + structure_tier)
              * program.grade())
```

Each unit draws one item from `yields` by weight. Then `rich_in` adds
`RICH_IN_UNITS` of the species' bonus part, on top, from any tool.

`SpeciesDef::rich_in: Option<ItemId>` is `#[serde(default)]` and **falls back
to `work_resource`** when absent, so no shipped species file changes.

**Amended 2026-09-05, when phase 3 built it.** Two things above are not what
shipped, and both are deliberate:

1. **There is no `structure_tier` parameter.** `Game::extraction_yield`
   takes `(&DownedProgram, &ToolDef)` and reads
   `Game::extraction_bench_tier()` itself. A parameter is a thing a caller
   can get wrong, and this function has two callers — the screen's preview
   and the act — whose whole reason for sharing one derivation is that a
   quoted figure and a granted figure must not be able to differ. The
   argument would be the crack they differed through. `Game::extraction_ticks`
   reads the tier the same way, for the same reason.
2. **The bench enters the yield as `tier - 1`, not `tier`.**
   `Game::best_structure_tier` reads a never-upgraded structure as tier 1,
   so `tier_scale(tool.tier + structure_tier)` as written would pay
   `TOOL_TIER_SCALE_STEP`'s full step — +50% of the material economy on the
   shipped `0.5` — for merely having built a Compiler. The upgrade is what
   sells yield, matching the rule the craft quality floor already takes off
   the same accessor. The *speed* half (`tuning::EXTRACT_BENCH_TICK_STEP`,
   a divisor floored at one tick) does use the full tier, so standing the
   bench is worth time and upgrading it is worth materials.

A phase 5 that re-derives the spec's literal formula would silently undo
both. The shipped shape is the one in `docs/seams.md`.

## 4. The act

```
Game::extract_program(index: usize, tool: &ToolId) -> Result<(), String>
```

The one door. Order, following `commit_caravan_basket`: every refusal lands
before anything is spent — game-over, active battle, no such program, tool
not installed, routine branch's "already known". Then the program is removed,
the yield granted through `grant_loot` with a new `LootSource::Extract`, the
line logged, and `self.tick()` spends `tool.ticks`.

Standing on a structure whose `StructureDef::extracts_programs` is set passes
its `StructureTier` as `structure_tier`. Reuses `can_extract_routines`'
"is one standing?" rule (`routines.rs:456`) rather than proximity.

**Routine category.** A `Routines` tool run on a downed program takes
`extract_routine`'s two branches — ordinary yields the knowledge into
`KnownRoutines`, exclusive pops the etched disk back out. Verified safe:
`routine_is_exclusive` reads `AbilityDef::exclusive` alone
(`routines.rs:188`), so nothing about the one-copy invariant rests on the
program having been tamed. The tamed-program path stays; both call one shared
inner function, not two copies — shipped 2026-09-05 as `Game::take_routine`,
which owns the effect alone. Each door keeps its own refusals: the tamed one
requires an extraction bench and checks ownership, the tool one requires
neither, because decision 7 forbids the structure being a gate.

**Amended 2026-09-05: "verified safe" above is wrong, and the shipped code
does something else.** The one-copy invariant never rested on
`routine_is_exclusive`'s inputs. It rests on a disk having been *consumed*:
`install_disk` spends one to put an exclusive routine on a tamed program, so
`take_routine` popping it back out is a return, and the run's copy count does
not move. Nothing is spent to put a routine in a *downed* program's pool —
`Game::routine_candidates` derives it from `SpeciesDef::abilities` — so two
kills of one species would have pressed two disks of something the exclusive
pool exists to keep at one. That no shipped species kit names an exclusive
routine made it unreachable, not safe; `nothing_a_new_game_ships_with_
teaches_an_exclusive_routine` is a census of today's files, not a
construction.

What shipped: `routine_candidates` **excludes exclusive routines outright**,
so the Reader teaches knowledge and never mints a disk, and the downed door
cannot reach `take_routine`'s exclusive branch at all. `take_routine` keeps
both branches and stays shared — the tamed door still needs the exclusive
one. `an_exclusive_routine_is_never_in_a_downed_programs_pool` replaced the
test that asserted the old reading.

## 5. Sources

| Site | Today | After |
|---|---|---|
| `award_loot` (`combat_rewards.rs:544`) | `roll_work_resource_drop` | `leave_downed_program` |
| `grant_nest_cache` (`zone.rs:88`) | same `work_resource` roll | leaves `NEST_CACHE_PROGRAM_COUNT` programs |
| boss kill | gear / fragments / ring | those, plus a program at a rarity and condition floor |
| sortie | `Sortie::loot` | plus `Sortie::programs` |

`roll_work_resource_drop` is deleted, not left dead. Its `Perk::Teardown`
term moves into the yield formula — that perk must keep being worth
something, and it is the trap the function was extracted to close.

Save: `DownedPrograms`, `Tools`, `KnownTools`, `SortieSave::programs`, all
additive behind `#[serde(default)]`. **No `SAVE_FORMAT_VERSION` bump.**

## 6. Screens

- `Mode::DownedPrograms` — the store, reached from the pack. One row per
  program: species, level, rarity, condition, boss tag. Selecting one offers
  the installed tools and the yield each would give, then extracts.
- `Mode::Tools` — the slots, one row each, reached from the party menu.

Both are read-only row lists with **no scroll**, so height is a layout
constraint: `MAX_DOWNED_PROGRAMS` and `TOOL_SLOT_CAP` must both fit at
1280x720, asserted by test and verified by mutation. Both join `ALL_MODES`
only — **amended 2026-09-04**: `needs_status_banner` is an allowlist of the
few non-popup modes that need a bottom-edge banner because a popup would
cover their own refusal line; a popup screen already has a refusal slot of
its own, and adding one to the allowlist makes
`every_screen_draws_a_refusal_exactly_once` paint the refusal twice.

`Mode::DownedPrograms` shipped in phase 1 (task 7) and correctly joined only
`ALL_MODES`, never `needs_status_banner` — implementation never acted on
this sentence's error. `Mode::Tools` is phase 2's screen and does not exist
yet; this amendment is here so whoever builds it doesn't act on the error
either.

The engine owns the row count and gui draws it; any per-row transform lives
in the engine, the `message_history` rule.

## 7. Testing

Censuses in `tests/assets.rs`, each failing the build:

- every shipped tool's `yields` resolve to real items
- every shipped tool's `category` is reachable and non-`Routines` tools have
  a non-empty `yields`
- `STARTER_TOOL_ID` resolves to a shipped tool
- every `rich_in` (authored or fallen back) resolves to a real item
- every `unlocks_tools` id resolves

Engine tests:

- a kill leaves exactly one program carrying that creature's species, level
  and rarity
- a full store refuses the drop, logs, and destroys nothing
- extraction removes the program and grants the yield; the previewed figure
  equals the granted one
- every refusal path spends nothing — asserted **per refusal**, since one
  test over one path passes against the others
- an exclusive routine is never offered off a downed program at all —
  **amended 2026-09-05**, see section 4; this read "extracted from a downed
  program leaves exactly one copy in the run", which the shipped exclusion
  makes unreachable rather than true
- `FIGHT_CONDITION_WEIGHT = 0.0` makes condition independent of the killing
  blow
- a save round-trip preserves a store of programs and a tool loadout (a RON
  round-trip alone cannot catch a `#[serde(skip)]`, so this is save->load)

`balance_sim` models no loot and gates none of this. The economy gate is
decision 8's drop-neutrality, asserted as a test comparing the starter
tool's expected yield to `WORK_RESOURCE_DROP`'s mean.

## 8. Phases

Each is its own plan and its own release.

1. **The loop.** Object, store, its screen, `ToolDb`, slots, starter tool,
   `extract_program`, drops replaced at kill and nest and boss. One or two
   shipped tools. No research, no structures, no routine category.
2. **The kit.** `unlocks_tools`, `KnownTools`, `forge_tool`, `install_tool`,
   the `Mode::Tools` screen, the full shipped tool set, slots growing with
   level.
3. **The base.** `extracts_programs` structure and its tier scaling; the
   routine category unified with `extract_routine`; sortie programs.
4. **Automation.** The bulk path: a machine takes downed programs off your
   hands and strips them while you are away. **Amended 2026-09-06, when
   phase 4 was brainstormed: `Carrying`, `Stock`,
   `collect::plan_adjacent_take` and work orders do *not* become
   instance-aware, and work orders do not change at all.** This bullet
   originally described making the whole production chain carry instances,
   which is the one thing decision 2 exists to prevent. See §10, which is
   the authority on this phase; this line is kept only so the amendment is
   visible from the phase list.
5. **The economy.** A gear-yielding extraction tool. **Amended 2026-09-05,
   when phase 5 was brainstormed: gear drops do *not* move behind
   extraction and `equipment_drops_for` does *not* retire.** The tool is
   additive and every existing gear door stays open. See §9, which is the
   authority on this phase; this line is kept only so the amendment is
   visible from the phase list.

## 9. Phase 5: the Gear tool

Brainstormed 2026-09-05, and it changed the phase. §8's bullet described
gear drops *moving* behind extraction with `equipment_drops_for` retiring.
Neither is what phase 5 does, and the bullet is amended above. Six
decisions, each taken against a named alternative that would otherwise look
attractive on a fresh read.

**9.1 The lottery moves; it does not become a choice.** A `Gear` tool rolls
the species' authored chances at extraction. It does not pay a guaranteed
piece. Chosen against *certain, rationed by slots* — one piece every time,
throttled by the store's ten rows and the four tool slots — which would have
made gear a decision rather than a draw, and would have set the game's gear
rate from scratch with no instrument in the repo able to check it. Also
chosen against a **condition-driven** axis, which would have put
`FIGHT_CONDITION_WEIGHT` to work; that weight ships at `0.0` deliberately
and wants a play session behind any other value, so phase 5 does not spend
it.

**9.2 Every existing gear door stays open.** `equipment_drops_for` does not
retire. It *cannot*: the Gear tool needs exactly its merged table, both
schema directions and all.

**Corrected 2026-09-05 while phase 5 was being built.** An earlier draft of
this paragraph called the four gear doors four "callers" of
`equipment_drops_for`. They are not. The function has **two** production
callers — the kill (`award_loot`, `combat_rewards.rs:667`) and the nest
cache (`zone.rs:132`) — and both are untouched. The other two doors reach
gear by a different route: the Stack feature cache
(`stack_features.rs:103`) and the surface boss (`combat_rewards.rs:771`, at
`SURFACE_BOSS_LOOT_RARITY_FLOOR`) both call `grant_gear_drop` directly with
their own item sources. All four doors stay open; only two of them run
through this table. The distinction matters to whoever closes one later:
two are a deletion at a `equipment_drops_for` loop, two are not.

The tool is therefore **additive**, and the consequence is recorded here
rather than discovered: a player who forges it sees a second, independent
roll at the same table on any program they bother to haul home. That is the
accepted cost of not setting a new economy blind. The door list is the one
thing that changes it, and closing a door later is a deletion at that
callsite, not a redesign. Note what the compensating lever is *not*: 9.5
introduces no scale constant to turn down. If closing a door needs the tool
to pay more, the honest levers are the authored chances in the species and
item assets, or a scale constant introduced at that point with the play
session that justified it.

**Corrected 2026-09-05, after the final review.** "Roughly double" understated
the range and was written before the ceiling in 9.5 was checked against
`compiler.ron`. The actual range: at a fresh bench (or none at all) the tool
adds the authored chance on top of the kill's own unchanged roll — the
"roughly double" case, and the floor of the range, not its whole story. At a
fully upgraded tier-5 Compiler, that added roll runs up to **3x** the
authored chance (9.5's corrected ceiling), not merely double.

**9.3 `ToolCategory::Gear`, a fifth variant.** Pool-less, exactly the
Routine Reader's shape: no `yields`, output derived from the program.
Chosen against a **second `Parts` tool**, which needs no enum arm but leaves
`Parts` meaning two different things — pool-driven and species-driven — with
the fork inside `extract_program` keying off the *absence* of a `yields`
pool, an implicit signal where a declared one is available. Also chosen
against **retiring the category branch** for a declared `ToolDef` effect,
with category demoted to a display label. That is the change that stops this
fork growing an arm per phase, and it is the right one to revisit if phase 4
wants to read an effect; it is not worth doing for a third arm alone.

**9.4 A whiff pays nothing.** No consolation scrap, no pity timer. The
program is consumed and the ticks are spent whether or not anything lands.
Chosen against a **scrap floor** (a small `yields` pool paid every time),
which would have kept the slot from ever being dead weight but would have
given this tool both a pool and a derived output — the one thing that makes
the Routine Reader's shape clean. Also chosen against a **pity timer**,
which bounds the worst run at the cost of a new run-save field and a
mechanic the game has nowhere else.

**9.5 Tier scales the chance, and the baseline needs no new constant.**

```
Game::gear_chances(&DownedProgram, &ToolDef) -> Vec<(ItemId, f32)>
```

Beside `extraction_yield` in `extraction.rs`, and reading the bench the same
way — `Game::extraction_bench_tier()` internally, **never a parameter**, for
§3's amended reason. It takes `equipment_drops_for`'s table and scales every
chance by `tier_scale(tool.tier + bench)`, where `bench` is the bench tier
minus one, exactly as `extraction_yield` computes it.

The baseline falls out of the shipped curve. `tier_scale(t) = 1.0 + (t-1) ×
TOOL_TIER_SCALE_STEP`, so a tier-1 Gear tool on a never-upgraded Compiler is
`tier_scale(1)` = **1.0**: the authored chance, untouched. A tier-3 Compiler
doubles it. No `GEAR_TOOL_CHANCE_SCALE` is introduced, and the neutrality
claim is therefore testable as an identity rather than as a number.

**Corrected 2026-09-05, after the final review.** The tier-3 figure above is
correct but is not the ceiling. `assets/structures/compiler.ron` ships
`max_tier: 5`, so a fully upgraded Compiler contributes `bench = 5 - 1 = 4`
to the scale's argument. A tier-1 Harness Puller worked there rolls
`tier_scale(1 + 4)` = **3.0** — three times the authored chance, not two.
Any future tuning pass against "roughly doubles" should read against this
figure instead.

Two deliberate divergences from `equipment_drops_for`:

1. **`gear_chances` clamps to 1.0 inside.** Its source returns chances
   unclamped on purpose, because each of its *drop* callers clamps before
   rolling. (Corrected 2026-09-05: that sentence said "its one caller" —
   `equipment_drops_for` has three callers now, `award_loot`, the nest
   cache and `gear_chances` itself.) This one has two callers — the preview
   and the pull — and a value clamped twice in two places is a crack they
   could differ through.
2. **Grade and level do not enter.** `program.grade()` already sells
   materials in `extraction_yield`. Leaving it out of the chance is what
   makes "baseline is the authored chance" literally true rather than
   approximately.

**9.6 The preview quotes named odds.** `Mode::DownedPrograms` grows a row
shape for chance-quoting tools: one line per candidate item with its live
chance — `Kinetic Plating 12%` — computed by `gear_chances`, the same call
the pull makes. Chosen against **names without numbers**, which keeps
percentages off screen but makes a Compiler upgrade invisible (an upgraded
bench would read identically to a fresh one), and against a **banded
phrase** in the game's own vocabulary, which costs a banding table and hides
any change that does not cross a boundary.

**Corrected 2026-09-05, during implementation.** This paragraph originally
claimed a percentage would be the first on any screen in the game. It is
not: `render/battle.rs` already prints decompile odds as `{:.0}%`, and
`render/inventory.rs` prints a hit chance the same way. What is true is
narrower — it is the first on an *extraction* screen, and it follows the
formatting convention those two already set rather than inventing one. The
reason for a number rather than a name list stands unchanged: an upgraded
Compiler must not read identically to a fresh one.

### The act

`extract_program` grows a third arm. Roll each chance from `gear_chances`
against the shared `GameRng`, and grant each hit through
`Game::grant_gear_drop(item, Rarity::Ordinary)` — the one door a copy above
`Ordinary` enters the game through, so found-gear-beats-crafted-gear still
binds and `crafted_gear_is_never_rare` is untouched. Nothing lands means no
log line and nothing granted.

`DownedProgram::boss` does **not** carry `SURFACE_BOSS_LOOT_RARITY_FLOOR`
through to the pull: a hauled-home boss pulls at `Ordinary`, because the
boss's own door stays open (9.2) and is still paying its floor at the kill.

### Content

One asset — `assets/tools/harness_puller.ron`, name open — tier 1, no
`yields`, `ticks` in the Routine Reader's band, since stripping a program's
kit off it is slow work. Research: `field_ops` is the candidate node, with
`runtime_patching` the alternative; the pick is a plan-time census of
whether the node already grants something and whether it sits at the right
zone depth. `forge_cost` is a guessed number like every other tool's, and
should say so in its own asset comment.

### Testing

- **The neutrality identity.** A tier-1 Gear tool on an un-upgraded bench
  quotes exactly `equipment_drops_for`'s figures for the same species. This
  fails loudly the day anyone inserts a constant between the table and the
  roll.
- **Preview equals pull** on one program, the §3 invariant applied to the
  new derivation.
- **No above-`Ordinary` copy is minted outside `grant_gear_drop`.** An
  absence, so it is asserted rather than left invisible.
- A census that `ToolCategory::Gear`'s new arm is reachable — a fifth
  variant is exactly the shape that ships as a blank.

### Recorded interactions

A running `DropBoost` field buff applies at extraction, because
`equipment_drops_for` folds it in at the end and extraction requires no
bench. A player can arm a buff and then strip. This reads as a fine synergy
and is left in; it is written down so it is a decision rather than a
surprise.

The two multipliers compound rather than substitute: `DropBoost` scales the
chance inside `equipment_drops_for` first, and `gear_chances` applies
`tier_scale` on top of that already-boosted figure. A maxed bench (9.5's 3x)
stacked with an armed buff pushes most candidates straight to the 1.0 clamp.

### Not in phase 5

Closing any gear door (9.2), the boss floor travelling with a downed
program, `FIGHT_CONDITION_WEIGHT`, and a help page for extraction.

## 10. Phase 4: the Teardown Rig

Brainstormed 2026-09-06, and it changed the phase. §8's bullet described
`Carrying`, `Stock`, `collect::plan_adjacent_take` and work orders all
becoming instance-aware. None of that is what phase 4 does, and the bullet
is amended above. Taken literally it would spend decision 2 — the seam
`components.rs:477` states, that `Inventory` is *by definition* the
plain-copy store, which is what lets recipes, `Stock`, `assembler_system`,
hauling and banking read it with no instance rule. The loop the phase is
actually for — fill the pack, come home, hand the kills over, go back out —
does not need that seam spent.

Six decisions, each against a named alternative that would otherwise look
attractive on a fresh read.

**10.1 A machine eats a queue; the chain never sees an instance.** The
player loads a hopper on one structure; that machine strips programs over
ticks and puts **plain items** into its `Stock::output`, where haulers,
depots and `collect` already pick them up with no change whatever. Chosen
against the §8 bullet's *instance-aware chain* — `Carrying` becoming
item-or-program, `plan_adjacent_take` walking a second store, a work order
naming an extraction — which is larger than phases 1-3 together, gives
every existing reader of `Stock` and `Carrying` an instance case, and buys
a kind of automation nobody asked for. Also chosen against a **standing
order on the player's own store**, auto-extracting as turns pass: cheapest
of the three, but it is a convenience key rather than automation, nothing
about the base changes, and it makes the store cap less interesting rather
than more.

The consequence is the phase's load-bearing claim, and it is what every
later reader should check a change against: **a `DownedProgram` exists in
exactly two places, the player's pack and one machine's private hopper.**
Anything that would put one anywhere else is out of phase 4 by
construction.

**10.2 Work orders do not change.** `producers_of` keys on
`produced_item(def)`, a fixed item per machine. A rig's output varies by
tool and by species, so it can declare none, and no work order can target
it. That is correct rather than a gap: "hold 3 Core Fragments" must not
make the base go and kill things. The phase keeps the name *automation* and
loses *the work-order path*.

**10.3 A new structure, not a fourth job on the Compiler.** The Compiler
already assembles ICE Breakers off a posted worker, extracts routines from
an owned program, and speeds and enriches manual teardown; a fourth errand
is where a machine stops reading as one thing, and it forces a rationing
rule between two jobs on one worker that has to be explained on a screen.
Chosen against all three ways to ration that worker — *extraction
pre-empts assembly* (a player dumps ten programs and quietly stalls their
ICE Breaker line), *assembly first and extraction fills idle time* (a
well-fed Compiler never touches the hopper, so the feature reads as broken
on exactly the base that is running well), and *a second posted worker*
(nothing today distinguishes two workers posted to one machine, so
`TaskKind` or the posting door has to grow a way to say which errand).

A separate structure dissolves the question instead of answering it, and
costs one asset, one research node, and no new mechanic: staffing, power,
upgrade, status and the output buffer are all existing machinery.

**The Compiler keeps `extracts_programs`.** The rig carries it too.
`standing_extraction_bench` already takes the max tier across every flagged
structure — "a mod's second one standing at a higher tier is what this
exists for" — so manual extraction simply gains a second possible bench.
Moving the flag *off* the Compiler would silently downgrade manual
extraction for any run in progress that has been upgrading one.

**10.4 Only pool-yielding tools may be queued.** `Routines` grants
knowledge and `Gear` grants a `GearCopy`; neither is a plain item, so
neither can land in a `Stock::output`, and both would need a second,
player-facing hand-back door to be worth anything. Both refuse at the
deposit with a line and stay hand work. Chosen against **holding the
result until the player next visits**, which is that second door in
disguise and puts a `GearCopy` in a machine — the thing 10.1 forbids.

**10.5 The rig steps in a `&mut Game` pass, not a bevy system.** This is
forced rather than chosen. `Game::extraction_yield` and
`Game::extraction_ticks` are `&Game` methods folding perks, `SpeciesDb`,
`ItemDb` and `best_structure_tier`; a bevy system cannot call them and
would have to re-derive the formula, which is exactly the crack §3's "one
derivation" exists to prevent. `run_teardown_rigs` joins `turn.rs`'s
existing `run_dig_crew` / `run_build_crew` / `run_repair_bays` /
`run_sorties` / `run_routes` family, so the rig calls the same two
functions the player's own extraction calls and an automated yield and a
previewed one cannot differ.

**10.6 Two verbs on the screen that already exists.** `Mode::
DownedPrograms` is already a two-page shape — the list, then one program's
tool-and-yield page. Loading is a second verb on the same pages rather than
a screen of its own. Chosen against **a load screen on the rig**,
`transfer.rs`'s two-sided shape, which shows the queue best and lets a
program be pulled back out, but costs a new `Mode` (which ships as a blank
screen until every draw arm is written), a second place the tool preview
lives, and a height census of its own. Also chosen against **bulk-only**,
which is one fewer key to document but costs the mixed case: a player who
wants the Prismatic level-30 stripped by hand and the rest fed to the rig
would have to extract that one first.

### The machine

`assets/structures/teardown_rig.ron` — the **Teardown Rig**, borrowing the
vocabulary `Perk::Teardown` already established. Ordinary structure schema
throughout: `build_cost`, `capacity` (its `Stock::output`), `power_draw`,
`upgrade: Some((max_tier: 5, ...))`, and `extracts_programs: true` so it is
a manual bench as well.

One new schema field, mirroring `assembles: Option<AssembleDef>` rung for
rung — decision 6's rule that a second shape for the same idea is a second
thing to get wrong:

```rust
// crates/engine/src/structures.rs, beside `assembles`
#[serde(default)]
pub strips: Option<StripDef>,

pub struct StripDef {
    /// How many downed programs the hopper holds.
    pub hopper: u32,
}
```

The hopper size is authored per machine rather than living in `tuning.rs`,
for `StructureDef::capacity`'s reason: it is how big *this* box is, and a
modder's second rig should be able to differ.

Unlocked by `assets/research/teardown.ron`, `requires: ["automation"]` —
the node that already unlocks the Compiler.

`build_cost`, `power_draw`, `capacity`, `hopper` and the research `cost`
are guessed numbers, like every other structure's and every tool's
`forge_cost`, and the asset says so in its own comment. They are the first
thing a play session should push on, and none of them is load-bearing on
anything above.

Two plan-time censuses, flagged rather than discovered: a free glyph and
colour for the rig, and whether `idle_machine_system` reaches a machine
declaring `strips` and neither `work` nor `assembles`. A new gated branch
shipping green and unreachable is a trap this repo has hit.

### The store and the deposit

```rust
// crates/engine/src/components.rs
pub struct Hopper {
    pub queue: Vec<HopperEntry>,
    /// Ticks spent on the head entry.
    pub progress: u64,
}
pub struct HopperEntry { pub program: DownedProgram, pub tool: ToolId }
```

A named struct, never a tuple: `WorkOrder`'s own doc records that RON parses
a `(` in a struct position as named fields, so a `Vec<(A, B)>` can never be
widened, and two shipped fields (`PlayerSave::fused_gear`,
`SaveData::buyback`) had to be drained into named successors already.

One door, bulk-shaped so the per-row verb and the bulk verb share one set of
refusals — `commit_caravan_basket`'s rule that every refusal lands before
anything is spent:

```
Game::load_teardown_rig(indices: &[usize], tool: &ToolId) -> Result<(), String>
```

Refusals in order: the run is over or a battle is active; the party is not
in base or no rig is adjacent; the tool is not installed; the tool's
category is `Routines` or `Gear`; the hopper has no room at all; `indices`
names no held program.

**An over-ask is clamped, not refused** — `take_from_adjacent`'s own rule.
A bulk load of ten into a hopper with six free slots takes six and leaves
four in the pack, saying so in the line. Nothing is destroyed, decision 9.

Adjacency rather than ownership, unlike `can_extract_routines`: this is a
physical handover, and `take_from_adjacent` / `give_to_adjacent` are the
rule for those.

### The screen

No new `Mode`. Both verbs land on `Mode::DownedPrograms`' existing two
pages, and both are **uppercase**: lowercase letters are row selectors
everywhere in this game, and a new action bound to one would make a single
keypress both pick a row and fire it.

- **On the tool page**, lowercase still extracts that program by hand,
  unchanged. Uppercase on the same row queues that one program with that
  tool instead.
- **On the list page**, `L` opens the tool page in *bulk intent* — a flag
  on the page, not a second page — where picking a tool row queues every
  held program with it. The header says which intent is showing, because
  the row keys mean different things under each.

So the dump-and-go flow is four keys: walk to the rig, `D`, `L`, one tool
key.

Loading costs whatever turn `transfer_items` charges for a handover, and
for its reason — handing cargo across is the same errand. It is not priced
in `extraction_ticks`: the rig pays those, not the player, and charging
both would make automation cost more than doing it by hand.

The list page's rows are unchanged in count and shape, so
`MAX_DOWNED_PROGRAMS`' existing no-scroll height census still covers it.
Any hint line the bulk header adds is new height on a page that has none to
spare, so it goes through that census rather than around it.

### The step

`Game::run_teardown_rigs`, per 10.5. Rigs are walked **in tile order** —
bevy's query iteration order is not stable, and `assembler_system` and
`run_repair_bays` both sort for this reason.

Per rig, the gates mirror `assembler_system`'s, each writing `MachineStatus`
on the transition alone (`set_machine_status`' rule that entering a state is
news and staying in it is not):

- dark on the Grid — skip, writing no status, `assembler_system`'s own
  handling
- no posted worker — `Unstaffed`
- empty queue — nothing, since `Idle` belongs to `idle_machine_system`
- otherwise `Running`

Then `progress += 1`, and the head entry completes when `progress` reaches
`Game::extraction_ticks(&tool)` — the same derivation the player's own
extraction is priced by, so upgrading the rig speeds its own work through
the bench term it supplies itself.

**The completion gate is `output_room() >= the yield's total`, not `> 0`.**
The assembler can use `> 0` because it makes one unit at a time; a program
pays several, and clamping to the room available would destroy units. A rig
that cannot hold the whole payout **holds the program** and reads `Clogged`.

One log line per completed program, unlike the assembler, which logs no
per-unit line. The reason they differ: an ICE Breaker is a known constant
output, and each program's yield is unique, unrepeatable information the
player has no other record of.

The rig runs while the party is in a zone. `run_repair_bays` gates only on
game over and an active battle, and that is the entire loop: fill the pack,
come home, load the rig, go back out.

### Save

`Hopper` is additive on the structure entity, `#[serde(default)]`, and
there is **no `SAVE_FORMAT_VERSION` bump** — §5's rule, and phases 1-3's
practice. A RON round-trip cannot see a `#[serde(skip)]` field, so the
hopper gets a real save-then-load test covering the queue and `progress`
both.

### Testing

Censuses in `tests/assets.rs`, each failing the build:

- `teardown_rig` resolves and its `strips.hopper` is non-zero
- the research node's `unlocks_structures` resolves
- anything declaring `strips` has a non-zero output `capacity`

Engine tests:

- every refusal path spends nothing — asserted **per refusal**, §7's rule,
  since one test over one path passes against the others
- the identity: what a rig produces for a `(program, tool)` pair equals
  `extraction_yield`'s quote for the same pair. This is §3's invariant
  applied to the third caller, and it fails loudly the day anyone
  re-derives the formula inside the rig
- a bulk load into a nearly-full hopper clamps, and the remainder is still
  in the pack afterwards
- no posted worker, no power, and a clogged output each advance nothing
- the completion gate: a rig whose output cannot hold the whole yield holds
  the program rather than dropping units
- a `Routines` or `Gear` tool refuses at the deposit
- a save-then-load preserves a loaded hopper, queue and `progress` both
- a reachability census that the `strips` arm is actually reached

app-core tests:

- the bulk verb queues **every** held program, not the selected one
- the per-row verb queues exactly one and leaves the rest in the pack
- the new uppercase keys collide with no row selector on either page, and
  a lowercase key on the tool page still extracts by hand

### Not in phase 4

Instance-aware `Carrying`, `Stock` and `plan_adjacent_take`, and any change
to work orders — retired from the phase by 10.1 and 10.2, and recorded here
so nobody rebuilds them later off the old bullet. A worker hauling programs
to the rig: the player carries them. Pulling a program back out of a loaded
hopper. And the tuning questions, which want a play session.

## Open, deliberately

- Whether the fight axis (`FIGHT_CONDITION_WEIGHT`) is worth turning on.
  **Confirmed 2026-09-04: the field ships and the weight ships at 0.0**, so
  the axis exists, is tunable, and has no effect until played. Do not delete
  the field for being unused, and do not fit a non-zero value without a play
  session behind it.
- **The cap is a count now and a weight budget later.** Confirmed
  2026-09-04: `MAX_DOWNED_PROGRAMS` as a flat row count is what phase 1
  ships, with the intent to move to a carried-weight metric once the store
  has been lived with. That later move wants a `DownedProgram::weight()`
  *derived* from species and grade — the "derived, never stored" rule — so
  no save field is added now and none is needed then. Whatever replaces the
  cap keeps decision 9's refusal: a full pack refuses the drop and destroys
  nothing already held. **Amended 2026-09-06: the cap stays a count, and
  phase 4 is what you do about it.** The Teardown Rig is the pressure valve
  the flat cap was uncomfortable without, so the move to a weight budget is
  no longer the obvious next step — it wants a play session that finds the
  count wanting even with a rig standing.
- **Phase 4's real cost.** Answered 2026-09-06 by §10, and it is far below
  the §8 bullet's estimate of "larger than 1-3 together": one structure
  asset, one research node, one `StructureDef` field, one component, one
  `&mut Game` pass joining an existing family, one engine door, and two
  verbs on a screen that already exists. The saving is decision 10.1 — the
  chain never sees an instance, so nothing outside the rig changes.
- **Which gear doors close.** Confirmed 2026-09-05: none, for now. Phase 5
  ships a Gear tool alongside every existing door rather than in place of
  any (§9.2), which raises the gear rate on an extracted program by the
  species' own authored chance at a fresh bench and by up to three times it
  at a fully upgraded Compiler (corrected 2026-09-05 — this line said
  "roughly doubles" while §9.2 and §9.5 carried the corrected figure)
  and is accepted knowingly. Closing a door is the tuning lever, and it
  wants a play session behind it — as does any constant introduced to
  compensate the tool when one closes (§9.5 ships none).
