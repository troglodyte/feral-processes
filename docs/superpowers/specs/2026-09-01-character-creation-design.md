# Character creation

**Status:** designed, not implemented.

A run currently begins with two keystrokes: `[n]` on the main menu, then a
difficulty. The player who arrives on the map is the same player every
time — `@` in br cyan, `tuning::PLAYER_BASE_STATS`, a hardcoded four-item
inventory, `Perks::default()`, and one routine slot deliberately left
empty. This adds a creation stage that decides who that is.

Everything below was verified against the source on 2026-09-01, not
remembered. Line references are to that day's `main`.

## What the player chooses

Seven steps, in order. Esc walks back one; Esc on the first returns to the
main menu.

| Step | Decides |
|---|---|
| Difficulty | `DifficultyMode::Permadeath` / `Forgiving` |
| Class | One of the five `AffinityClass` variants |
| Look | Icon (a glyph *and* a sprite name) and colour |
| Points | Stat allocation, opening on a roll |
| Routine | The starter routine that fills the free slot |
| Name | The player's own name |
| Summary | Nothing — it confirms, and shows what the profile will pay |

`[R]` on any step rolls every remaining choice and jumps to Summary, for
players who would rather not walk the wizard.

## One mode, seven steps

The whole wizard is a single `Mode::CreateCharacter`. `App` holds the
in-progress `CharacterChoice` and a `CreationStep` cursor;
`handle_creation_key` dispatches on the step, and the renderer draws the
step the cursor names.

Seven `Mode` variants would each owe a row in `ALL_MODES` (today
`[Mode; 86]`, `render/mod.rs:1162`), in `needs_status_banner`, and in the
refusal census that walks them. `Mode::Transfer` is the precedent for
collapsing: its own doc comment says it is one screen rather than two
because the alternative was "two screens with a mirrored key table". A
wizard is one linear flow with one back button, which is the same case.

`ALL_MODES` therefore stays at `[Mode; 86]`: this adds one variant and
deletes another.

**The refusal census must walk `CreationStep::ALL`, not just the mode.**
Otherwise it asserts against one step and passes while six others cannot
say why they refused something. `CreationStep::ALL` is an exhaustive
const array for `cell_mark`'s reason — a step added as a `_ =>` arm ships
undrawable.

**Difficulty is folded in** rather than kept as `Mode::DifficultyPick`, so
the mode count is net zero against today and starting a run reads as one
decision instead of two screens that do not know about each other.
`Mode::DifficultyPick` and `handle_difficulty_key` are deleted.

## Class

### What it grants

Affinities, and nothing else.

`Game::ability_affinity` (`crates/engine/src/game/combat.rs:930`) already
branches on the player and already clamps:

```rust
if actor == self.player_entity() {
    let affinity =
        AFFINITY_NEUTRAL + crate::perks::affinity_bonus(self.player_perks(), kind);
    return affinity.min(AFFINITY_MAX);
}
```

The player is hardcoded neutral on all five axes, with the five affinity
perks the only thing that moves any of them. A class is a starting spread
landing on that one expression:

```rust
let affinity = self.player_class_affinity(kind)
    + crate::perks::affinity_bonus(self.player_perks(), kind);
return affinity.min(AFFINITY_MAX);
```

Additive rather than multiplicative, matching the shape the player arm
already has. The clamp is unchanged, so a class plus a maxed affinity perk
cannot exceed the bound `tuning.rs` reasons about everywhere else — it
only reaches it sooner, which is a real cost of the class and not a bug.

**No talent tree.** `assets/talents/` ships a tree per class, but they are
the *companion's* axis: `ability_affinity`'s own comment states the seam —
"Talents are the companion's axis, in the creature arm only: perks are the
player's, and the two never stack." The trees are also priced against
Kernel Rings, which the player has no source of. The player's tree is
Perks and stays Perks.

**No stat shape.** `ClassShape` (`species.rs:449`) exists to generate
*species* stat blocks. Applying it to the player would entangle the class
choice with the points choice; keeping them orthogonal is what makes the
two steps worth having separately. Class answers "what am I good at",
points answer "how tough am I".

### Every class damps something

`ClassShape` gives each class one axis held below neutral — Striker damps
Heal, Medic and Bastion damp Damage. Shipped class files follow that, so a
class is a trade rather than a bonus.

**There is no Unaligned option.** Every player picks one of the five, and
so every player is worse at one kind of routine than today's player. This
is affordable because of what affinity does and does not reach:

- **`battle::expected_damage` has no affinity term** (`battle.rs:297` —
  hit chance, band mean, atk, crit, and nothing else). `balance_sim`
  *calls* that function rather than keeping a copy, so the balance gate is
  already blind to affinity. Class spreads move nothing it measures.
- **The player's ordinary swing never touches `ability_affinity`.** It is
  `attack_range(entity, PLAYER_UNARMED_DAMAGE)` into `resolve_attack`.
  Affinity is consulted only in `combat_round.rs`, for authored
  `AbilityEffect` magnitudes. A Medic damping Damage at 0.80 loses a fifth
  of their damage *routines'* authored power, not a fifth of their attack.

So classes join the set CLAUDE.md already names as ungated by the sim,
alongside the Power economy. **The instrument for them is the arena**,
which runs real abilities — one scenario per class after the numbers land.

### Class data

`assets/classes/`, one file per variant, loaded by `ClassDb` in a new
`crates/engine/src/classes.rs`. `PerkDb`'s exact pattern and the same
contract every other db in this repo honours: an absent directory loads
empty, a malformed file is skipped with a logged warning rather than a
panic, `iter` is sorted by id because every caller walks it.

```
(
  class: Medic,
  name: "Medic",
  description: "...",
  affinities: (heal: 1.3, damage: 0.8),
  kit: [("core_fragment", 5), ("power_cell", 4)],
)
```

The five `AffinityClass` variants stay in Rust — they are load-bearing
across talent trees, base-post behaviour and species stat shapes, and are
not a content surface. The catalogue carries only what is authored: name,
blurb, spread, kit. This is the perks seam exactly.

**The player stores the class, not the spread**, and re-resolves through
`ClassDb` on every read. A retune of a class file therefore reaches runs
already in progress. `ActiveContract` stores its whole resolved def for
the opposite reason and the distinction is deliberate: a contract is a
signed agreement that must not be rewritten under the player, a class is
an identity.

**An empty `assets/classes/` is a supported install** — the same property
`assets/memories/` and `assets/needs/` hold. With no classes loaded the
step offers nothing, the choice resolves to neutral, and the run is
today's game. Both ends hold it: the resolver returns neutral for a class
it cannot resolve, and the screen skips a step with no rows.

### The kit

`ClassDef::kit` replaces the hardcoded `Inventory` in `Game::new`
(`ICE_BREAKER` 3, `POWER_CELL` 3, `CORE_FRAGMENT` 5, `OUTLET` 2). That
list survives as the fallback for a choice with no class — which is what
`CharacterChoice::default()` is, and therefore what every existing test
gets.

A census asserts every id in every shipped kit resolves in `ItemDb`. A kit
naming an item that does not exist must fail the build, not silently hand
the player an empty pack.

## Look

### Icon

Each option is a **(glyph, sprite name) pair**. `crates/gui/src/render/
base.rs:1249` currently picks the sprite name by role:

```rust
let sprite = actor.and_then(|ev| {
    if ev.is_player { Some("player") } else if ev.is_anchor { Some("anchor") } else { None }
});
```

The player arm takes the name from the choice instead. The seam is
unchanged and is what makes this cheap: a sprite **substitutes** for the
glyph and a name the table has nothing under returns `false`, so the
caller draws the glyph. Every option therefore works today on its glyph
alone and upgrades in place as art arrives, per option. Art blocks
nothing.

New sprites are 16x16 RGBA PNG like the two already shipped;
`the_shipped_sprites_are_one_cell` refuses anything else at load. Author
them near-white — the renderer passes the colour as a **multiplying** tint,
which is exactly what makes the colour choice below work on the art for
free.

### Colour

A new `palette::PLAYER_CHOICES`, separate from `palette::PLAYER`. That
constant stays where it is — it also means "an upgradeable item" — and the
map's player glyph reads the chosen colour instead.

`every_content_hue_is_separable_from_the_others` (`palette.rs:209`)
currently walks `GlyphColor::ALL` plus `PLAYER` and asserts every pair is
more than 0.25 apart. **It extends to walk `PLAYER_CHOICES`**, so a colour
that collides with a content hue — the red that means a fight will kill
you, the blue reserved for a Nemesis — fails the build rather than
shipping. That test is the whole of the rule; there is no second place the
player's separability is enforced.

### Reaching the renderer

The chosen glyph lives in the player's existing `Glyph.ch`. The colour
cannot live in `Glyph.color`: `GlyphColor` is the eleven-hue *content*
palette and the player's choices are deliberately not in it. So the
colour index and the sprite name ride the view — `views::` gains both
fields on the actor event the map already reads `is_player` off.

## Points

### The exchange rate

`MainStat` already exists (`achievements.rs:92`) with exactly the four
axes: `Atk`, `Def`, `Integrity`, `Decompiler`. `Reward::RandomMainStat(n)`
already applies each (`lifecycle.rs:1901`). It is re-exported for this
feature rather than duplicated.

But it adds `n` **uniformly** to all four, and the four are not
commensurate: `Def` is a percentage point on a base of 2 that levelling
never raises, while `Integrity` is one HP out of 90. The profile survives
this because its totals are small and *random*. A screen where the player
chooses makes `Def` strictly dominant, so creation prices each axis:

| Axis | Cost | Grants |
|---|---|---|
| Integrity | 1 | +6 `max_hp`, and `hp` with it |
| Atk | 1 | +1 `atk` |
| Decompiler | 1 | +1 `skill` |
| Def | 3 | +1 mitigation point |

Rates and pool size are `tuning.rs` constants, in the player baseline
section beside `PLAYER_BASE_STATS`. `MAX_CREATION_STAT_POINTS` gets a
ceiling assertion, mirroring `MAX_PROFILE_STAT_POINTS`' reason: a
permanent buff with no ceiling is a shape this design has closed off
before.

`Integrity` must raise `hp` alongside `max_hp` or the run starts damaged —
the trap `MainStat::Integrity`'s own doc comment records.

### Additive, never redistributive

The pool is spent **on top of** `PLAYER_BASE_STATS`, which is unchanged.
Every build is therefore at or above the floor `balance_sim` models, so
its curves stay a valid lower bound and the level cap's correctness bound
— below which a geared party cannot clear a zone at any level it can reach
— is untouched.

The profile's own `RandomMainStat` grant lands on the same `Stats`
afterwards, in `grant_profile_rewards`, and stacks additively. Nothing
needs to change there.

### Roll, then reallocate

The step opens on a rolled spread rather than a blank form, and the player
redistributes freely from there. The roll spends exactly the pool, so it
can never beat point-buy and there is no reason to reroll for size —
`[R]` rerolls for shape.

Key handling is `Mode::Transfer`'s, which is the closest existing screen:
the cursor moves on Up/Down through `App::scroll`, and Left/Right adjusts
the highlighted row. `ShiftLeft`/`ShiftRight` and `CtrlLeft`/`CtrlRight`
already exist as a target and a halving step, and `App::handle_key`'s fold
is the list of screens allowed to see one — `Mode::CreateCharacter` joins
it.

## Starter routine

The free slot is already there and already documented.
`tuning::PLAYER_ROUTINE_SLOT_BASE` is 2, and its comment says why:
"decompile occupies one — a new game pre-installs that ability, so the
player starts with one free slot rather than having to reach
`PLAYER_ROUTINE_SLOT_PER_LEVEL` for it." This step fills exactly that
slot. It adds no slot and moves no constant.

### The gate

`AbilityTarget` has exactly two single-entity variants (`abilities.rs:160`),
so the rule is `matches!(target, OneAlly | OneEnemyGroupFront)`. 42 of the
86 shipped abilities qualify.

Beyond that the pool is a new `#[serde(default)] starter: bool` on
`AbilityDef` — **opt-in**, the idiom `exclusive` and `wild_weight` already
use, so the pool is defined by the files that ask to be in it rather than
by a module listing them. A modder's routine joins by authoring one field.

Three censuses in `tests/assets.rs`:

- every `starter: true` ability is single-target,
- every `starter: true` ability is `exclusive: false` — an exclusive
  routine may never enter `KnownRoutines` at all, and creation must not
  become a fourth way around that,
- at least one starter exists per affinity axis. A `#[serde(default)]`
  field authored nowhere ships documented and dead, which this repo has
  already shipped once (`spread`, used by 0 of 77 ability files).

Shipped candidates, all single-target, non-exclusive and cheap:
`stack_smash` (Damage), `checksum_repair` (Heal), `hyperthread` (Buff),
`hard_lock` (Debuff), `siphon_cycles` (Drain).

### The pool is not filtered by class

A Medic may take a damage routine. The rows show each routine's numbers
**through the chosen class's affinity**, so picking Medic and then reading
a damage routine at 0.80 teaches the affinity system at the one moment it
costs the player nothing. Filtering the list would hide exactly that.

### A new door into `KnownRoutines`

The choice grants *knowledge* — a `KnownRoutines` entry — and installs it
into the free slot. Knowledge, not just the install, so the routine can be
etched onto a disk later like any other thing the player knows.

This is the first door into `KnownRoutines` that spends no item.
CLAUDE.md's seam list records "installing a routine is the one place a
`KnownRoutines` entry meets an item, and the item is spent last"; that
sentence needs amending, in all three places a seam is written.

## Name, and the save list

The player's name reuses `components::CustomName` (`components.rs:85`)
rather than a new type. Text entry follows `Mode::FuseName`.

The save picker currently identifies runs by filename. It shows the name
instead, which is most of why the name is worth having.

## Summary, and the profile preview

The last step draws the whole character and waits for Enter. It is also
where the profile preview lives: what the player's achievement record is
about to grant this run — stat points, Perk Points, a starting program.

Today that is one `MessageKind::Outcome` line logged inside
`grant_profile_rewards` and then scrolled past. Cross-run progression is
the one thing a player earns *between* runs, and the moment it means
something is the moment they are deciding who to be.

**The wrinkle:** the reward list is derived from `Profile` plus
`AchievementDb`, and the db loads inside `Game::new` — which has not been
called yet when the wizard is running. `App` already owns `help_db`, so it
loads an `AchievementDb` of its own by the same precedent. The derivation
itself must be one function both the preview and `grant_profile_rewards`
call, never a copy: a preview that disagrees with what is actually paid is
worse than no preview, and this repo has four recorded occasions of a doc
comment promising a mirror while holding a copy that drifted.

## The engine seam

`Game::new(seed, difficulty, assets_dir)` **keeps its signature** and
delegates:

```rust
pub fn new(seed: u32, difficulty: DifficultyMode, assets_dir: &Path) -> io::Result<Self> {
    Self::new_with(seed, difficulty, assets_dir, &CharacterChoice::default())
}
```

There are 1,633 `Game::new(` call sites, roughly 1,600 of them tests.
Adding a parameter is a 1,600-line mechanical diff for no benefit;
delegation is zero churn.

`CharacterChoice::default()` is **today's player exactly** — no class and
so neutral on all five axes, `@` in `palette::PLAYER`, no name, no starter
routine, the hardcoded kit. The wizard does not offer "no class", but the
code keeps a neutral baseline: it is what the test suite constructs and
what `balance_sim`'s modelled floor corresponds to.

```rust
pub struct CharacterChoice {
    pub name: String,
    pub class: Option<AffinityClass>,
    pub glyph: char,
    pub sprite: String,
    pub colour: u8,
    pub stats: [u32; 4],      // points spent, indexed as MainStat::all()
    pub routine: Option<AbilityId>,
}
```

Difficulty stays a separate parameter — `Game::new` already takes it, and
it reaches a resource rather than the player.

## Save format

`PlayerSave` gains `name`, `class`, `glyph`, `sprite` and `colour`, every
one `#[serde(default)]`. The save is field-named RON and an additive change
behind a default costs **no `SAVE_FORMAT_VERSION` bump**.

Nothing else is needed. Stats are already saved as final numbers and
`KnownRoutines` and `Routines` are already saved, so the points and the
starter routine are receipts — the same shape a `Stat` talent has, where
the save holds the raised numbers and load must not re-apply the grant.
**Load must not re-apply the class kit, the points, or the routine.**

A round trip through RON cannot catch a field that is skipped on write, so
the new fields need a real save-to-disk-and-load test, not only a RON
round trip.

## Files

**New**

- `crates/engine/src/classes.rs` — `ClassDef`, `ClassDb`
- `crates/gui/src/render/creation.rs` — the seven step draws
- `assets/classes/*.ron` ×5, plus `assets/classes/README.md`
- `assets/help/` — one page on creation

**Changed**

- `crates/engine/src/game/lifecycle.rs` — `new_with`, applying the choice
- `crates/engine/src/game/combat.rs:930` — the class term in the player arm
- `crates/engine/src/components.rs` — the player identity component
- `crates/engine/src/abilities.rs` — `starter: bool`
- `crates/engine/src/views.rs` — colour index and sprite name on the actor event
- `crates/engine/src/save.rs` — five `PlayerSave` fields
- `crates/engine/src/tuning.rs` — pool, exchange rates, ceiling
- `crates/app-core/src/lib.rs` — `Mode::CreateCharacter`, `CreationStep`, `App` fields; `Mode::DifficultyPick` deleted
- `crates/app-core/src/app/menus.rs` — creation handlers; `handle_difficulty_key` deleted
- `crates/app-core/src/app/lifecycle.rs` — `start_new_game` takes the choice
- `crates/gui/src/render/mod.rs` — dispatch, `ALL_MODES` unchanged at 86
- `crates/gui/src/render/hud/palette.rs` — `PLAYER_CHOICES` and the walk
- `crates/gui/src/render/base.rs:1249` — sprite name and colour from the view
- `assets/abilities/*.ron` — `starter: true` on the chosen few
- `assets/abilities/README.md`, `assets/sprites/README.md`
- `CHANGELOG.md`

Three crates and four asset directories, so this takes the full
spec-and-plan pipeline rather than inline TDD, per CLAUDE.md's process
weight rule.

## Censuses and tests

- every `AffinityClass::ALL` variant has a class file — exhaustive, so a
  sixth class fails the build rather than shipping unpickable
- every kit item id resolves in `ItemDb`
- every `starter: true` ability is single-target and non-exclusive; at
  least one per affinity axis
- `every_content_hue_is_separable_from_the_others` walks `PLAYER_CHOICES`
- the refusal census walks `CreationStep::ALL`
- the tallest creation step fits its screen at 1280x720 — the wizard has
  no scroll, so height is a layout constraint, and this is the one place
  it is checked
- a save→load round trip through a real file for the five new fields
- loading a save does not re-apply the kit, the points or the routine
- an empty `assets/classes/` loads and plays as today's game
- `MAX_CREATION_STAT_POINTS` ceiling

## What a green suite will not tell you

Whether the five classes feel distinct. Whether the pool is the right
size. Whether the colours read against the map's biome tint and vignette.
Whether seven steps is a wizard or a slog.

The arena is the instrument for the class numbers —
`cargo run --bin arena` runs real abilities where `balance_sim` runs none.
Everything else needs the screen in front of a person.

## Deliberately not in this version

- **Seed entry.** Would make worlds shareable and sector traits observable
  for the first time. Considered and cut from v1.
- **Picking a starting program.** The largest lever available, and
  `grant_starting_program` is already built. Held until the flow has been
  played, and it needs a rule for how it meets the profile's own
  `Reward::StartingProgram`.
- **A rival named at creation.** `NemesisDb` exists. Its own feature.
- **A player talent tree.** Breaks the perks/talents seam on purpose; see
  Class above.
