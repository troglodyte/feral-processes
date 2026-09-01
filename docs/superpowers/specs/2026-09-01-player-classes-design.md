# Player-only classes

**Status:** designed, not implemented.

A player's class is currently one of `species::AffinityClass`'s five
variants — Striker, Bastion, Medic, Saboteur, Leech — and that enum is
fixed in Rust because it is load-bearing for things that have nothing to
do with the player: a *species'* derived role, `TalentDb`'s tree keys,
`ClassShape`'s stat blocks, and the base job a posted program does.
`ClassDb` is keyed by it, so there is no sixth slot to put a player-only
class in.

This adds three classes that no species can ever be — a decompile
specialist, a routine specialist and a base-throughput specialist — by
giving the player their own class enum and giving each new class one
effect hook at a door that already exists.

Everything below was verified against the source on 2026-09-01, not
remembered. Line references are to that day's `main` (`886f0a7d`).

## What a class is today, and what has to change

`ClassDef` (`crates/engine/src/classes.rs`) grants exactly two things: an
**affinity spread** that scales the authored magnitude of the player's own
routines, and a **starting kit**. Neither "decompiler-focused" nor
"routine-focused" is an affinity axis, so both need a grant kind that does
not exist.

The user's decision on 2026-09-01: **these classes do not have to be
moddable.** That settles the seam, and it settles it the way `perks.rs`
already did. A perk's *catalogue entry* — name, description, cost — is
data in `assets/perks/*.ron`; a perk's *effect* is a named query in Rust,
because an effect is a hook into a particular formula with no shared shape
to express as data. Player classes take the same split:
`assets/classes/*.ron` stays the catalogue (name, description, affinity
spread, kit) and each new class's effect is a named query in `classes.rs`.

## The mechanism

### `PlayerClass`

A new enum in `crates/engine/src/classes.rs`, eight variants **in this
order**:

```
Striker, Bastion, Medic, Saboteur, Leech, Decompiler, Invoker, Fabricator
```

The first five keep `AffinityClass`'s names *and positions*. It replaces
`AffinityClass` as the type of:

| Site | Field |
|---|---|
| `components::PlayerIdentity` | `class` |
| `save::PlayerSave` | `class` |
| `game::creation::CharacterChoice` | `class` |
| `views::ClassRow` | `class` |
| `classes::ClassDb` | the map key |
| `arena::scenario::CharacterSpec` | `class` |
| `app_core::CreationRow::Class` | carried whole, no change of its own |

`AffinityClass` itself is **not touched**. Every exhaustive match on it —
`AffinityClass::of_axis`, `ClassShape`, `TalentDb`'s tree keys,
`render/manifest.rs::base_job_label`, `systems::CycleModifiers::class` —
compiles unchanged, and no species, companion or posted worker can ever be
a Fabricator. That separation is the point of the change and is the thing
a later "simplification" will try to undo; it needs a `docs/seams.md`
entry and a line in the `seams` skill.

### No save-format bump

The `.bin` save is **positional bincode** (`save.rs:1088`): it has no field
names, and `#[serde(default)]` does nothing for it. An enum is encoded by
variant *index*. Because the first five variants keep their names and
positions and the three new ones append, `PlayerSave::class` encodes
identically for every existing save, and `SAVE_FORMAT_VERSION` does not
move.

The five shipped `.ron` files need **no edit at all** — `class: Striker`
parses as `PlayerClass::Striker` exactly as it parsed as
`AffinityClass::Striker`. So does `dev-arenas/class-*.ron`, and so do the
`dev-saves/*.ron` templates, which carry no `class` key at all.

**This makes variant order save format, the same as `Perk`'s.** Append,
never reorder. That belongs in a doc comment on the enum and in a
round-trip test that saves and loads a run carrying `Fabricator`.

### The effect seam

One named query per class effect in `classes.rs`, each **exhaustive over
`PlayerClass`** so a ninth variant fails to compile rather than shipping
inert — `cell_mark`'s rule, and `perks.rs`'s
`every_perk_has_a_query_that_answers_what_it_is_worth` precedent. A class
with no effect of a given kind returns the neutral value.

Magnitudes are `pub const`s in `tuning.rs` with doc comments, per the
"content is moddable; how hard the game is, is not" rule. Nothing about a
class effect is authored in `.ron`.

## The three classes

Each spikes one thing through one hook, and gives up something authored in
its own `.ron` `affinities` field — no second hook. The damped-axis
convention `assets/classes/README.md` already documents carries over; what
changes is that the *raised* axis is no longer always an affinity.

### Decompiler

**Spike:** a flat percentage-point boost to every decompile attempt.

**Hook:** `Game::player_decompiler_bonuses` (`game/unlocks.rs:31`) — the
one place the player's side of a decompile is assembled, read by both call
sites that *show* odds and the one that *rolls* them. The class term sums
into `DecompilerBonuses::capture_boost_pct` beside
`field_buff_power(FieldBuffKind::CaptureBoost)`.

`capture_boost_pct` rather than `skill`, deliberately: `skill` enters
`capture_chance` as `1.0 + skill * DECOMPILER_SKILL_BONUS`, and skill
already grows on level-up and off equipment, so a flat class addend there
dilutes all run. `capture_boost_pct` multiplies the whole attempt and is
worth the same at level 30 as at level 1. Its doc comment on
`DecompilerBonuses` currently says the field means "a running `CaptureBoost`
field buff"; it becomes the one place the two whole-attempt boosts are
summed, and must say so.

**Trade:** `affinities: (damage: 0.8)`. The class raises no affinity axis
at all — its spike is the hook — so it damps one and raises none.

**Starting constant:** `CLASS_DECOMPILE_BOOST_PCT: i32 = 15`.

### Invoker

**Spike:** +2 routine slots.

**Hook:** `Game::routine_slots` (`game/combat.rs:673`), the one door for
"how many routines can this body hold". The class term is added **at that
caller's player arm**, exactly mirroring what its companion arm already
does:

```
if entity == self.player_entity() {
    abilities::player_routine_slots(level) + <class term>
} else {
    abilities::companion_routine_slots(level) + self.talent_routine_slots(entity)
}
```

`abilities::player_routine_slots` therefore **stays a pure function of
level**, which is the reason its own doc comment gives for
`companion_routine_slots` staying one: several tests and `balance_sim`
read it as a pure curve.

**The bonus is added after the clamp, and is not re-clamped**, which is
the existing behaviour of the companion arm — `talent_routine_slots`
already pushes a companion past `COMPANION_ROUTINE_SLOT_CAP`. So the cap
bounds the *level curve*, not the total, and an Invoker keeps its +2 for
the whole run: 4 slots at level 1 against everyone else's 2, and 14 at the
cap against 12.

Threading the term into `player_routine_slots` before its `.clamp(1, cap)`
was the first design and is rejected: it converges to nothing at the cap,
which would make the one thing the class is named for stop existing in the
late run, and it would break the purity two other readers depend on.

**This needs a row census the other two hooks don't.** 14 is a count no
player screen has ever had to draw. `MAX_SECTION_ROWS` caps the manifest's
ROUTINES box at 6 with a `+N more` indicator, and that overflow path is
itself unverified in play — so the plan must check what the player's own
routine list does at 14 rows before this number ships.

**Trade:** `affinities: (buff: 1.15, debuff: 1.15, damage: 0.85)`. Breadth
over force — more routines loaded, each hitting softer on the offensive
axis.

**Starting constant:** `CLASS_ROUTINE_SLOT_BONUS: u32 = 2`.

### Fabricator

**Spike:** every work cycle in the base finishes in fewer ticks.

**Hook:** `systems::work_ticks_at_speed` (`systems.rs:297`), supplied from
`Game::work_ticks_for` (`game/base/building.rs:752`) — already the one
door for "how long is a cycle at this machine", shared by `assign_cronjob`
and `work_structure`. So the speed-up reaches a posted program's cycles
*and* the player working a node by hand, and the two cannot drift.

The term goes in `work_ticks_at_speed` rather than in `work_ticks_for` so
there is still one formula: `views.rs` documents the shipped
`ticks_per_unit` against that function, and a scale applied at the caller
would make the displayed figure and the real one two different numbers.
The signature takes a scale, not a `PlayerClass` — the function knows
about speed, and `work_ticks_for` is what resolves the player's class into
a number.

**The class is the player's, not the worker's**, which is the asymmetry
`CycleModifiers` already documents ("the perk is the player's wherever the
cycle is being run, while the aptitude and the class belong to whoever is
standing at the machine"). A Fabricator's bonus applies at every machine
in their base regardless of who is posted there.

The result **bakes into `Task::required` at assignment**, which is how the
existing tier speed-up already behaves (`systems.rs:1242`). Safe here
because a class is chosen once at creation and never rerolled.

**Trade:** `affinities: (buff: 1.2, damage: 0.8)`.

**Starting constant:** `CLASS_WORK_TICK_SCALE: f64 = 0.8`.

## The creation screen

`crates/gui/src/render/creation.rs:122` builds a class row as
`"{name} - {axes} [{kit}]"`, and `axes` comes from
`classes::format_axes(&affinities)` alone. All three new classes spike
something that is not an affinity, so as written the Decompiler advertises
itself as `Decompiler - -Damage [...]` — a class with a downside and no
upside.

**`format_axes` gains a second input**: a named query returning each
class's non-affinity spike as a label, prepended to the affinity terms.

| Class | Label |
|---|---|
| Decompiler | `+Decompiling` |
| Invoker | `+2 Routines` |
| Fabricator | `+Cycle speed` |

Built in the engine beside `format_axes` and `format_kit`, for the reason
those two exist: two renderers must not word one class's trade
differently.

**The wizard has no scroll**, but `draw_popup` grows to
`popup::popup_max_rows` — 28 at 1280x720 — so eight class rows fit with
room. Width is the real constraint and already has a census:
`no_creation_row_runs_past_the_popup_body` measures every row of every
step against `popup_body_width` at both shipped window shapes. The three
new rows must clear it, which bounds how long a new class's name, label
set and kit can be together.

## What holds it up

- **A census over `PlayerClass`**, exhaustive, asserting every variant has
  a shipped `.ron` file in `assets/classes/` — `cell_mark`'s rule, and the
  reason a new variant cannot ship blank.
- **One test per hook**, each asserting the bonus moves the number it
  claims to: capture odds through `capture_chance`, slot count through
  `Game::routine_slots`, cycle length through `work_ticks_for`. Each must
  fail with the class term removed — `a-test-that-passes-with-the-fix-removed`.
- **A save round trip** carrying `PlayerClass::Fabricator`, plus an
  assertion that an existing save with `class: Some(Medic)` still loads.
- **The width census** above, extended by the three new rows.
- **`no_creation_row_runs_past_the_popup_body` and the height census** both
  re-run against eight classes.

## What this is invisible to

**`balance_sim`.** It models no abilities, no base and no decompiling, and
`assets/classes/README.md` already records that a class's spread is
invisible to it. All three new effects are equally invisible: the slot
count, the capture boost and the tick scale are none of them terms in
`expected_damage`. **The arena is the instrument** if these need retuning,
and `dev-arenas/class-*.ron` is where the existing five are measured.

That means every number in this spec is a starting value argued from the
shipped curves, not a measured one. Expect to move all three.

## The property that changes shape

`assets/classes/README.md` promises that deleting the directory is a
supported install: every affinity resolves neutral, the hardcoded kit
applies, and you get the pre-class game. That still holds for the
catalogue half.

It does **not** hold for the hooks. A save already carrying
`PlayerClass::Invoker` keeps its +2 slots with `assets/classes/` deleted,
because the effect is Rust. Nobody can *pick* the class in that state —
the wizard has no rows to offer — so this is only reachable by deleting
the directory mid-run.

This is the perk seam's behaviour exactly (catalogue is data, hook is
code) and is the right answer: an effect that vanished when a *display*
catalogue went missing would be the surprising one. But it is a real
amendment to what that README claims today, and the README must say so.

## Blast radius

Three crates, no new module, no save-format bump.

| Crate | Change |
|---|---|
| `engine` | `classes.rs` grows the enum and the named queries; three hooks gain one term each; `components`/`save`/`creation`/`views`/`arena` are a type swap |
| `app-core` | type swap only — `CreationRow::Class` carries `ClassRow` whole |
| `gui` | type swap; `format_axes`' new input is consumed, not built, here |
| `assets` | three new `.ron` files; `README.md` amended for the label query and the property above |
| `docs` | `docs/seams.md` entry for the `PlayerClass`/`AffinityClass` split, a line in the `seams` skill, a rule in `CLAUDE.md` |

## Out of scope

- **Retuning the existing five.** They keep identical spreads and kits.
- **Making class effects moddable.** Explicitly decided against.
- **A ninth class.** The census makes adding one a known, bounded job.
- **Raising `PLAYER_ROUTINE_SLOT_CAP`.** The Invoker's +2 is added past
  it, the way talents already add past the companion cap; the constant
  itself does not move.
