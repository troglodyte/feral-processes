# What a program was worth to the thing it became

A structure eats a tamed program. Today it does not matter *which* — any
program of sufficient depth files the same order and raises the same machine.
This spec makes the program's individual quality decide how well the machine
runs, in both directions: a poor builder leaves a machine slower than the one
that ships, a good one leaves it faster.

**The choice must not compete with combat value.** That is the whole design
constraint, and it decides the shape. Every existing quality axis on a program
— `Rarity`, `hp_roll`, `atk_roll`, `def_roll`, `growth_roll` — measures how
good it is in a fight. Hang build quality on any of those and spending your
best fighter on a Mining Node is strictly correct, the picker becomes a
punishment, and the feature is a tax on the roster rather than a use for it.
So the axis is new: two rolls that do nothing at all while the program is
alive, and are read at exactly one moment.

## Context

`BuildSite::program: Option<CreatureSave>` already holds the committed program
from the moment an order is filed until the crew finishes it — see
`docs/superpowers/specs/2026-09-07-tamed-program-build-requirement-design.md`,
whose §2 lifecycle table this spec extends by one column. That spec closed with
*"no structure remembers what it ate"* listed as deliberately out of scope.
This is that scope, reopened with a reason: not flavour, a number.

Three facts about the code decide most of what follows.

**`Potential` is already the game's individual-quality axis.**
`components.rs:1401` carries four rolls over
`MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL` (0.8–1.2, `tuning.rs:948`), and
`quality_label()` already speaks the vocabulary this feature needs — *Poor,
Below Average, Average, Above Average, Excellent*. It is two-sided around a
neutral 1.0. `Rarity` is not: `Ordinary` is `stat_mult() == 1.0` and every rung
above multiplies up, so a rarity-driven effect can only ever be a bonus.

**A species already carries two aptitudes, and neither is on `Stats`.**
`SpeciesDef::base_int` (`species.rs:271`, drawn to the player as *"Analysis"*)
is extraction aptitude; `base_speed` is cycle-length aptitude. The field's own
doc comment states why they are species-level: *"a stat grows on level-up, so a
level-20 bruiser would out-think a level-1 specialist and role would collapse
back into tier — the confound this field exists to remove."* The two new rolls
are the **individual** variation of that same idea, standing to a species
aptitude exactly as `hp_roll` stands to `base_hp`.

**There is one formula for how long a work cycle takes.**
`systems::work_ticks_at_speed` (`systems.rs:308`), one non-test caller
(`Game::work_ticks_for`, `game/base/building.rs:1072`). Its doc comment already
refuses the obvious shortcut: `class_scale` is a parameter *"applied here
rather than at `Game::work_ticks_for` so there is still one formula — `views`
documents a machine's shipped `ticks_per_unit` against this function, and a
scale applied at the caller would make the displayed figure and the real one
two different numbers."* That sentence decides where this feature's term goes.

## Decisions taken

| Question | Decision |
|---|---|
| Quality axis | `Potential`, two new rolls. Not `Rarity` — it has no rung below neutral. |
| Rarity's part | A modest additive lift on top, `grade()`'s principle. |
| Which roll applies | Read off the structure's def: `work` → extraction, otherwise assembly. |
| What it changes | Cycle length, through `work_ticks_at_speed`. |
| Structures with no cycle | Unaffected. Home, Depot, Shield and amenities eat a program and ignore its quality. |
| Upgrades | The new program's quality replaces the old figure outright. |
| The blended `quality_percent` | Unchanged — stays the four combat rolls. |
| Visibility | Everywhere `quality_label` shows: roster, inspect, build picker. |
| Save version | No `SAVE_FORMAT_VERSION` bump. Additive, RON, defaulted. |

## 1. Two new rolls

`components::Potential` goes from four fields to six:

```rust
/// How good this individual is at *building* a machine that consumes
/// input — an assembler, a Teardown Rig. Read at exactly one moment: when
/// this program is spent on a build (`Game::build_quality_of`). It changes
/// nothing about the program itself, ever.
///
/// Deliberately independent of every other roll. The other four measure
/// combat value, and a build axis correlated with those would make
/// spending your best fighter strictly correct — the confound
/// `species::AptitudeFault` polices for the species axis, applied to the
/// individual one.
pub assembly_roll: f32,

/// The same, for a machine that produces from nothing — anything
/// declaring `StructureDef::work`. Two rolls rather than one because a
/// player should be able to hold a good tapper and a poor assembler and
/// have that mean something at the build screen.
pub extraction_roll: f32,
```

Both roll over `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`, the same range as
the other four, in `Game::roll_potential` (`game/spawning.rs:574`).

`Potential::NEUTRAL` gains both at `1.0`. `Potential::averaged`
(`components.rs:1445`) must average both — **this is the change most likely to
be missed**, because `Game::fuse_companions` hand-writes its own component list
and nothing fails to compile when a new field is left out of the fold. A fusion
that silently rolls its child's build aptitude back to a partial average reads
as fusion being bad, not as a missing line.

`quality_percent()` and `quality_label()` are **not** touched. They keep
averaging the four combat rolls, so every screen that shows a program's tier
today keeps meaning exactly what it means, and a program can read `Excellent`
as a fighter and `Poor` as a builder. That tension is the feature.

### Player-facing words

`assembly_roll` draws as **"Assembly"**; `extraction_roll` as **"Extraction"**.
Both sit in the same register as `base_int`'s *"Analysis"*, and both are words
this game already uses for the machines in question — an assembler consumes,
and `SpeciesDef::base_int` is described in source as *"the extraction
aptitude"*.

Each gets its own label off the same ladder `quality_label` uses, via a shared
helper so a roll and its word cannot drift:

```rust
impl Potential {
    /// The `Poor`..`Excellent` rung one roll sits on. `quality_label` is
    /// this applied to the average of the four combat rolls; the two build
    /// rolls each apply it to themselves.
    pub fn roll_label(roll: f32) -> &'static str;
}
```

`quality_label` is rewritten to call it rather than keeping its own `match`,
because two copies of a five-rung ladder is the copy that drifts.

## 2. What a build's quality resolves to

One function on `Game`, the single definition:

```rust
/// What the program committed to a build of `def` is worth to it —
/// `1.0` being the machine exactly as its `.ron` file describes it.
///
/// Which roll is read comes off the structure's own def and nothing else,
/// the same `(work, assembles)` split `work_ticks_for` already makes to
/// find a machine's base rate. No new asset field.
pub(crate) fn build_quality_of(def: &StructureDef, program: &CreatureSave) -> f32
```

```
roll    = if def.work.is_some() { extraction_roll } else { assembly_roll }
quality = roll + rarity.rank() as f32 * BUILD_QUALITY_PER_RARITY_RUNG
        = 0.80 .. 1.32
```

Rarity is an additive lift and a small one. This is `DownedProgram::grade()`'s
principle applied verbatim, and `GRADE_PER_RARITY_RUNG`'s comment is the
argument: *"rarity already pays out through the drop table, so grade's job is
to reward condition and level, with rarity as a secondary lift rather than the
dominant term."* Here the same holds — rarity already multiplies four stats, so
letting it dominate build quality too would collapse this axis back into "spend
your best program", which is the one outcome the design exists to prevent.

A Prismatic program that rolled Poor Assembly still builds a machine slightly
below spec. That is intended: rarity insures, it does not rescue.

**Fourteen of the thirty-one shipped structures declare neither `work` nor
`assembles`.** They fall to `assembly_roll` by the `else`, and it is then never
read, because a structure with no cycle never reaches `work_ticks_at_speed`.
The `else` is written that way rather than as a three-armed match returning
`Option` because the caller would have nothing to do with the `None` — see §4,
which is where that fraction is argued rather than merely noted.

## 3. Where the number lives, and the one formula it changes

A new component:

```rust
/// How well this structure was built — the quality of the program spent
/// raising it, or the last one spent upgrading it. See
/// `Game::build_quality_of`.
///
/// **Absent means `1.0`.** That covers the Home (exempt from the program
/// cost entirely), every test fixture that hand-spawns a structure, and
/// every save written before this feature — all three want the machine
/// exactly as its `.ron` file describes it, and all three get it with no
/// branch at the read site. `Potential::NEUTRAL`'s idiom.
#[derive(Component, Clone, Copy, Debug)]
pub struct BuildQuality(pub f32);
```

It is written in `Game::spawn_structure` (`game/base/building.rs:410`), which
CLAUDE.md already names as *the one place a structure's component list is
written*. The signature gains one parameter:

```rust
fn spawn_structure(&mut self, def: &StructureDef, x: i32, y: i32, quality: Option<f32>) -> Entity
```

Two non-test callers:

| Caller | Passes |
|---|---|
| `game/base/building.rs:225` — founding the Home | `None`. The Home is exempt from the program cost, so there is nothing to read. |
| `game/base/construction.rs:500` — the crew finishing `BuildGoal::New` | `build_quality_of(&def, program)` off the site. |

`BuildGoal::Upgrade` does not call `spawn_structure` — it resolves the standing
structure by tile and raises its `StructureTier`. That arm
(`construction.rs:503`) inserts the new `BuildQuality`, overwriting whatever
was there. One figure per structure, always the last program spent.

### The formula

`systems::work_ticks_at_speed` gains a fourth argument beside `class_scale`:

```rust
pub(crate) fn work_ticks_at_speed(
    base_ticks: u32,
    speed: i32,
    class_scale: f64,
    build_scale: f64,
) -> u32
```

```
build_scale = 1.0 - (quality - 1.0) * BUILD_QUALITY_TICK_WEIGHT
```

It goes **inside** this function and not at the call site, for the reason the
function's own doc comment already gives about `class_scale`: `views` documents
a machine's shipped `ticks_per_unit` against this function, so a scale applied
at the caller makes the displayed rate and the real one two different numbers.
The existing `.max(1.0)` floor covers the new term too.

`Game::work_ticks_for` reads `BuildQuality` off the structure entity it already
holds, defaulting to `1.0` when absent, and passes it through. It is the only
non-test caller.

**This composes with `base_speed` rather than colliding with it.**
`base_speed` is the *posted worker's* effect on a cycle; `BuildQuality` is the
*builder's*, baked into the machine at the moment it was raised. That is the
same "whose is this" discipline `systems::CycleModifiers`' doc comment enforces
field by field, and the reason the two multiply cleanly.

### Tuning

```rust
/// What one rung of `Rarity::rank` adds to a build's quality, on top of
/// the program's own roll. Small for `GRADE_PER_RARITY_RUNG`'s reason:
/// rarity already multiplies four stats, so letting it dominate here would
/// collapse build quality back into "spend your best program", which is
/// the outcome this axis exists to prevent.
pub const BUILD_QUALITY_PER_RARITY_RUNG: f32 = 0.03;

/// How much of a build's quality reaches the cycle length. At 0.5 the axis
/// spans about ±10% on a machine's rate, plus the rarity lift.
///
/// Deliberately half of what `base_speed` is worth through
/// `WORK_TICKS_PER_SPEED`, because the two multiply: a slow program posted
/// to a badly-built machine would otherwise compound to 1.44x the shipped
/// rate, and a machine's authored `ticks_per_unit` would stop meaning
/// anything a player could plan against.
pub const BUILD_QUALITY_TICK_WEIGHT: f64 = 0.5;
```

Worked ends, against a Mining Node's shipped rate and a baseline worker:

| Program | quality | cycle vs shipped |
|---|---|---|
| Ordinary, Poor | 0.80 | 1.10x — 10% slower |
| Ordinary, Average | 1.00 | 1.00x — exactly as authored |
| Ordinary, Excellent | 1.20 | 0.90x — 10% faster |
| Prismatic, Excellent | 1.32 | 0.84x |

## 4. Structures with no cycle are unaffected, and that is a tactic

**Fourteen of the thirty-one shipped structures run no work cycle**, so cycle
length cannot touch them: `black_market`, `contract_broker`, `data_cache`,
`defrag_bay`, `depot`, `home`, `line_driver`, `patch_node`, `portal`,
`recharger_node`, `relay`, `repair_bay`, `sandbox`, `shield`. Every one still
costs a program; the quality of it changes nothing about them.

That is close to half the catalogue, and it is the honest cost of choosing one
seam over a per-kind multiplier. It is stated here in full rather than left for
a player to infer, because "my build choice did nothing" is indistinguishable
from a bug unless the screen says otherwise — which is why the build picker
must say so at the point of spending (§6), not merely omit the figure.

Read the other way it is a tactic rather than a gap: spending your worst
builders on storage and walls is a real decision, and it gives the programs
this axis marks as poor somewhere to go. The alternative considered and
rejected was a second effect — durability for structures with no cycle — which
is a second formula with its own balance argument, aimed at a number invisible
until a raid lands.

## 5. Save

Three persisted fields, all additive, all RON. The save is field-named RON
since 0.8.0 and a pre-0.8.0 bincode save is *refused* rather than parsed
(`save.rs:1436`), so `#[serde(default)]` is what carries an additive field
today and **none of this costs a `SAVE_FORMAT_VERSION` bump** — `save.rs:539`
states exactly that. The "bincode has no granular field-level compatibility"
comments scattered near `CreatureSave` are historical and describe a format
this build cannot read at all.

| Struct | Field |
|---|---|
| `CreatureSave` | `assembly_roll: f32`, `extraction_roll: f32` |
| `StructureSave` | `build_quality: f32` |

`StructureSave` sits beside `durability` and `tier` — the fields that are live
per-instance state rather than properties of the def. `BuildQuality` belongs
with them and **must be restored from the save, never re-derived on load**.
`Game::load` deliberately rebuilds some structure components from the def so a
modder's retune reaches structures already standing (`Stock::capacity` is the
stated example), and `ResourceNode::level` already needed carving out of that
rule — it was re-derived, which brought a Mk3 back extracting like a Mk1. The
program that raised this machine is gone; there is nothing on the def to
re-derive its quality from, so re-deriving means silently resetting every
machine in the base to neutral on load.

### The trap: a defaulted `f32` is `0.0`, not `1.0`

`#[serde(default)]` on an `f32` yields zero. Every one of these three fields
must instead use `#[serde(default = "…")]` returning the neutral `1.0`,
`default_output_capacity`'s precedent (`structures.rs:274`).

Left as a bare `#[serde(default)]`, a save written before this feature loads
with every program the player owns at `0.0` on both build rolls — permanently
the worst builder in the game, with no way to tell and nothing in the log — and
every standing structure at `build_quality: 0.0`, which drives `build_scale` to
1.5 — half again the ticks on every cycle in the base, on the load that was
supposed to change nothing. Both are silent, and the second is the kind of
regression that arrives a week later as "the base feels wrong".

A RON round trip cannot catch a field that fails to serialise. This needs a
real save-to-file, load-from-file test.

## 6. Display

Both rolls show everywhere `quality_label` already does.

| Surface | Change |
|---|---|
| `views::ManifestPotential` | Two fields, plus their labels |
| The inspect / manifest page | Two rows beside the existing quality tier |
| `companion_row_lines` (`gui/src/render/party.rs:243`) | The roster row gains the relevant tag |
| The build picker (`Mode::BuildProgram`) | Each row shows **what this program does to this machine**, resolved |

The picker showing only the applicable roll is the point: at a Mining Node the
rows rank by Extraction, at a Lathe by Assembly, so the screen answers the
question the player is actually asking rather than making them do the lookup.

### The picker states the effect, not the aptitude

A row reading `Assembly: Excellent` makes the player do the conversion — they
would have to know the roll range, the rarity lift and the tick weight to turn
that word into anything they can act on. The row states the consequence
instead:

```
[a] Scrapper 3 Lv7  Assembly: Excellent    Lathe cycle 20 -> 18 ticks
[b] Cipher 2  Lv4   Assembly: Poor         Lathe cycle 20 -> 22 ticks
[c] Construct 2 Lv9 Assembly: Average      Lathe cycle 20 -> 20 ticks
```

Two rules govern that figure, and both are load-bearing.

**It is a call, not a copy.** `Game::extraction_yield` is this repo's precedent
and its argument applies verbatim: it is *the one derivation of what a tool
draws out of a downed program, shared by the act and the screen's preview*, so
a quoted figure and a granted figure cannot differ. The picker's preview must
reach the real `systems::work_ticks_at_speed` through `Game::build_quality_of`
— never a percentage re-derived in a view or a renderer from
`BUILD_QUALITY_TICK_WEIGHT`. A second expression of this formula is the copy
that drifts, and the symptom is a screen that promises a faster machine than
the base delivers.

**It is ticks, not a percentage, and that is the whole reason it is correct.**
`work_ticks_at_speed` rounds to whole ticks and floors at one. On a short cycle
the rounding eats the effect outright: a 5-tick machine at `build_scale` 0.91
computes 4.55, rounds to 5, and is *not one tick faster*. A row advertising
`-9%` there would be quoting a change that does not happen — precisely the
quoted-versus-granted failure the previous rule exists to prevent, arriving
through arithmetic rather than through a duplicated formula. Showing both tick
figures makes the rounding visible instead of hiding it, and a row honestly
reading `5 -> 5 ticks` tells the player something true about small machines
that no percentage could.

The preview holds the worker at `DEFAULT_BASE_SPEED`, because no program is
posted yet at the moment of the build. The figure is therefore the machine's
rate as built, not a promise about whoever ends up standing at it, and the
screen should not imply otherwise.

**Three states, not a number and a zero.** `PowerCell`
(`gui/src/render/popup.rs:85`) is the shape to follow — `Rated(n)` is a rating,
`Unrated` an em dash (*no answer*, not a bad answer), `Blank` a row that is not
an item.

But `PowerCell` is a `pub(super)` **renderer** type, and this one cannot be,
which splits the work across the two crates along a line this repo already
draws:

```rust
// engine, crates/engine/src/views.rs — what the answer IS
pub enum BuildEffect {
    /// This machine runs a cycle: its shipped rate, and its rate as this
    /// program would build it. Both are real figures out of
    /// `systems::work_ticks_at_speed`, never a percentage.
    Cycle { shipped: u32, built: u32 },
    /// This structure runs no cycle, so quality cannot reach it (§4).
    NoCycle,
}
```

The variant is the engine's because resolving it requires `build_quality_of`,
the structure's def and the tick formula — none of which gui may reach for, and
all of which the call-not-a-copy rule above puts in exactly one place. Drawing
it is gui's: the em dash for `NoCycle`, and a fixed-width column for the quote
so a long figure grows its own row rather than losing a digit, which is what
`POWER_COLUMN_WIDTH` exists for on the row beside it.

This is the same division CLAUDE.md states for read-only screens — *any per-row
transform must live in the engine*, because a figure folded in the renderer is
a figure app-core cannot count, rank or test.

Collapsing `NoCycle` into `Cycle { shipped: n, built: n }` would draw a Depot
as though quality applied and happened to change nothing, which is the reading
§4 spends a paragraph refusing — and it would collide with test 14, where
`n -> n` is the *true* answer for a short-cycle machine.

**And for the fourteen structures that run no cycle it must say so outright** —
one line on the picker, not an omitted column. Those builds ignore quality
entirely (§4), and a screen that silently drops the figure teaches the player
nothing; a screen that says *this build does not care which you spend* turns
the same fact into the tactic it is. Rows do not grey: every program is
equally valid here, which is exactly the message.

**Width is the real risk on this screen, and it is testable headlessly.** The
picker reuses `companion_row_lines`, which is also the roster row, so anything
added there lands on both — and the picker row now carries two additions, an
aptitude tag *and* a two-figure cycle quote, on top of a row that already
spends its width on name, level, HP, ATK, MIT, PWR, gear and rarity. Popup body
width has bitten this repo before: the swap row's quality figure put the joined
form 35.6px past a 1243.2px body and was lost in silence.

That precedent also says *how* to add it. The category tag on a swap row is a
**column on the row, not a substring of it** — `Row::Item::tag` carries the
token and its lead, and `draw_row` lays the row out as three `ui_runs` pieces
so no row moves. The cycle quote wants the same treatment rather than being
appended to the head, because `wrapped_row_lines` never breaks the head and an
over-long head is lost rather than wrapped. `paint::with_painter` measures real
text, so this is pinned by test rather than by eye.

No new key. The picker's rows are selectors and lowercase letters are row
selectors, per the standing rule.

## 7. Costs, named rather than discovered

**`roll_potential` goes from four `GameRng` draws to six.** That shifts the
seeded stream for every creature spawn, and seeded tests across the suite will
need re-baselining. It is a known event in this repo, not a hidden one, and the
implementation should expect it rather than debug it.

The alternative — deriving both rolls from the existing four — was rejected. It
would make a program good at HP automatically good at building, which is
precisely the fault `species::AptitudeFault` exists to police for the species
axis: *"the point of `base_int` is a species axis that is **not** the ladder
wearing another name."*

**`Potential::averaged` is the silent failure.** See §1.

**`balance_sim` gates none of this.** It models no base at all, so both
constants are unmeasured — the same standing as the Power economy, and for the
same reason. The check is play, not a curve. `cargo test -p
feral-processes-engine balance_sim` should be run anyway to confirm the curves
have *not* moved, since nothing here should touch them.

**Nothing here will have been on a screen.** A green suite is not evidence of
play, and the build picker is a screen an agent cannot see.

## 8. Tests

Engine:

1. `build_quality_of` reads `extraction_roll` for a `work` structure and
   `assembly_roll` for an `assembles` one.
2. Rarity lifts the figure by exactly one rung's worth, and a Prismatic Poor
   program still lands below neutral.
3. A machine built by an Excellent builder cycles in fewer ticks than one built
   by a Poor builder, same worker, same def.
4. A structure with no `BuildQuality` cycles at exactly its shipped
   `ticks_per_unit` — the absent-means-neutral rule.
5. The Home spawns with no `BuildQuality` and nothing reads one.
6. An upgrade overwrites the figure with the new program's, and does not
   average or keep the better.
7. `Potential::averaged` folds all six rolls; a fusion of two Excellent
   builders is an Excellent builder.
8. `roll_potential` puts both new rolls inside
   `MIN_INDIVIDUAL_ROLL..=MAX_INDIVIDUAL_ROLL`.
9. Save → load → the machine cycles at the same rate, and the roster's build
   rolls survive (file round trip, not RON).
10. A save file with no `assembly_roll`/`extraction_roll`/`build_quality` keys
    loads every one of them at `1.0` — the defaulted-`f32` trap, asserted on a
    hand-written save missing the keys.
11. A cancelled build order refunds a program with both build rolls intact.

App-core / gui:

12. The picker ranks by the roll the pending structure will actually read.
13. **The quoted figure equals the granted one.** Read the picker's
    `BuildEffect` for a program, file the build with that same program, run it
    to completion, and assert the finished machine's real cycle length equals
    the `built` figure the row advertised. This is the test the whole §6
    call-not-a-copy rule exists for, and it must be asserted end to end rather
    than by comparing two expressions.
14. A machine short enough that rounding eats the effect quotes `n -> n` and
    genuinely cycles at `n` — the case a percentage would have got wrong.
15. A Depot's rows report `BuildEffect::NoCycle`, and the screen says so rather
    than drawing an unchanged number.
16. The roster row and the picker row both fit their width with the aptitude
    tag and the cycle quote — measured through `paint::with_painter`, not
    asserted on a length.

Each must fail with the change removed. Test 10 in particular is vacuous if
written against a save this build wrote, since `Game::save` always writes the
keys.

## Open, deliberately

- **No species-level build aptitude.** `base_speed` already gives a species a
  cycle axis; a second species field for building would need authoring across
  every species file to express something the individual roll already says. If
  species ever need to differ at building, that field is the shape it takes.
- **The two rolls are invisible until spent, and that is unusual.** A stat that
  does nothing until you destroy the creature carrying it has no precedent in
  this game. It is the price of not competing with combat value, and it is
  worth watching in play: if it reads as dead weight rather than as a second
  way to value a program, the fix is to give the rolls a live effect (the
  program's own work cycle when *posted*), not to fold them back into combat.
- **No structure names the program it ate.** Still out of scope, still flavour.
