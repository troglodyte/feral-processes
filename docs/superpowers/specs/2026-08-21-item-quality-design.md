# Item quality

**Status:** approved, not implemented
**Date:** 2026-08-21

A fourth per-copy axis on gear: how well *this particular copy* was
compiled, as a percentage of the item's authored bonus. Crafting rolls it
from inputs the player builds toward; drops roll it from a poor flat floor.

## Why

`GearCopy` already carries three axes that make two copies of one item
worth comparing — `rarity`, fusion `tier`, and `affix`. All three enter the
game through `Game::grant_gear_drop`, which is deliberately the only door
above `Ordinary`. Crafting and buying produce `GearCopy::plain`.

So every blade you compile is byte-identical to the last, and the entire
"is this one better than mine" question belongs to the loot table. The
design intent recorded here is the opposite: **gearing should be a base
activity.** A developed base with an upgraded bench should out-produce the
world reliably, and a field drop should be a lottery ticket rather than the
gearing path.

Quality is the axis that expresses that, because it is the one axis whose
inputs can be things the player builds.

## The field

```rust
pub struct GearCopy {
    pub item: ItemId,
    pub rarity: Rarity,
    pub tier: u32,
    pub affix: Option<AffixId>,
    /// How well this copy was compiled, as a percentage of the item's
    /// authored bonus. 100 is "exactly as designed".
    #[serde(default = "default_quality")]
    pub quality: u8,
}
```

**It must be an integer.** `GearCopy` is the key of the `GearCopies`
ledger — `add`, `count` and `take` all find rows by `==`, and
`EquippedItem` holds the same key for a worn copy. An `f32` breaks `Eq`
and with it the "keyed by value rather than by position" seam that
`components::GearCopies` documents. `u8` percent; the band lives in
`tuning.rs`.

**The default is 100, not 0.** A bare `#[serde(default)]` yields `0`, which
would silently reduce every piece of gear in every existing save to 0%
quality — a total loss of stats presenting as a balance bug. It needs
`default = "default_quality"`.

**No `SAVE_FORMAT_VERSION` bump.** An additive `#[serde(default)]` field on
a *named* struct costs no version bump, per the save seam. A RON round-trip
test cannot catch a defaulting fault (see the standing note on
`#[serde(skip)]`), so this needs a real save → load test asserting a
pre-quality save loads its gear at 100.

### What rolls quality

Only items with an `equipment` def. The predicate is the one
`grant_gear_drop` already uses — `Game::equipment_of(&item).is_none()`
returns a plain copy and spends no draw. A material or consumable stays at
100, stays plain, and keeps stacking in `Inventory`.

### `is_plain` gains its fourth `&&`

```rust
pub fn is_plain(&self) -> bool {
    self.rarity == Rarity::Ordinary
        && self.tier == 0
        && self.affix.is_none()
        && self.quality == QUALITY_DEFAULT
}
```

This is exactly what that function's doc comment anticipated ("A fourth
property added to a copy joins the `&&` here and nowhere else"). It stays
the single definition; `count_copies` / `take_copies` / `add_copies` are
untouched.

**Consequence, accepted deliberately:** crafted equipment stops stacking.
Compiling five blades yields up to five rows in `GearCopies`. That is the
feature working — the player is meant to compare them and keep the best —
but every list that names gear gets more rows, and each row gets wider.

## Where it lands in `copy_bonus`

`Game::copy_bonus` is the one expression for what gear is worth and the
order of its axes is load-bearing. The new order:

```
affix folded into base
  -> scaled_for_level
  -> for_quality      (new)
  -> fused_for_tier
  -> for_rarity
```

**`for_quality` carries no floor**, unlike `fused_for_tier` and
`for_rarity`. Their per-step floors exist to make a *discrete rung*
observable at the magnitudes gear ships at. Quality is continuous and is
supposed to be a fine gradient; a floor would turn a 70–130 band into a
flat +60 on a 4-point stat.

Being floor-free is also why it cannot go last. Gear ships at 1–4 points a
stat, so a bare percentage applied last is eaten by rounding, and worse:
with base atk 4, `Silver` at 70% computes to 4 while `Ordinary` at 130%
computes to 5 — the row colour becomes a lie about which copy is better.

Applying it after `scaled_for_level` gives it a level-scaled number with
enough resolution to bite, and keeping the two floored axes last preserves
the honest form of the guarantee: **a rare tier's floor is guaranteed
against a copy of equal quality**, not globally.

It follows `for_rarity`'s other rules: a stat sitting at zero stays at
zero (quality sharpens what an item does, it does not hand it a new stat),
and a negative component from a drawback affix is left where it is, so
improving a copy never deepens its penalty.

## The roll

One formula, one clamp, every term a named constant in `tuning.rs`:

```
floor   = QUALITY_BASE + bench_term + perk_term + care_term
quality = clamp(floor + roll(0..=QUALITY_SPREAD), QUALITY_MIN, QUALITY_MAX)
```

Better inputs raise the floor; every compile still rolls its spread. All
four inputs are terms in one legible expression rather than four
mechanisms.

The spread is drawn **in steps**, not drawn and then rounded:
`roll(0..=QUALITY_SPREAD / QUALITY_STEP) * QUALITY_STEP`. Every term is
already a multiple of `QUALITY_STEP`, so the sum is too and the clamp
cannot produce an off-step value. Rounding a fine draw afterwards would
bias the ends of the band.

### Proposed constants

| Constant | Value | What it is |
| --- | --- | --- |
| `QUALITY_DEFAULT` | 100 | the authored bonus; what every existing copy loads as |
| `QUALITY_MIN` / `QUALITY_MAX` | 70 / 130 | the clamp |
| `QUALITY_STEP` | 5 | 13 distinct values across the band |
| `QUALITY_SPREAD` | 20 | the luck term, 0..=20 in steps of 5 |
| `QUALITY_BASE` | 80 | craft floor at a tier-1 bench, no perk, not careful |
| `QUALITY_BENCH_PER_TIER` | 5 | per tier **above 1** |
| `QUALITY_PERK_PER_LEVEL` | 5 | per perk level |
| `QUALITY_CAREFUL_BONUS` | 10 | the careful-compile toggle |
| `QUALITY_DROP_BASE` | 70 | drop floor |

Which gives: a fresh player's craft 80–100, a developed base's craft
110–130 (clamped), a field drop 70–90.

**Stated balance consequence:** early crafted gear is *weaker* than it is
today, where every craft is exactly 100. That is the intent — a base is
what earns good gear, and crafting still beats the drop table from turn one
— but it is a real early-game nerf and should be felt in a session before
it is called correct. `QUALITY_BASE` = 90 is the softer variant, centring a
baseline craft on today's numbers and making the base's contribution purely
upside.

### The terms

- **Bench tier.** A recipe already names `CraftableDef::requires_structure`,
  and `components::StructureTier` already exists on any structure whose def
  sets `upgrade`. A new `Game::best_structure_tier(kind) -> Option<u32>`
  reads the best deployed one. The term is
  `QUALITY_BENCH_PER_TIER * (tier - 1)`, so a structure with no
  `StructureTier` component reads as tier 1 and contributes 0, and a recipe
  naming no bench contributes 0 as well. This is what gives a bench upgrade
  a purpose beyond unlocking recipe rows.
- **Perk.** One *appended* `Perk` variant plus a hook in the roll. `Perk`'s
  variant order is save format (bincode encodes enums positionally and
  `PlayerSave::unlocked_perks` holds indices), so it must be appended, never
  inserted. Its name, description and Perk Point cost go in
  `assets/perks/*.ron`; `PerkDef` has no `effect` field and does not gain
  one.
- **Careful compile.** A `bool` on `Game::craft`: spend extra ingredients
  for a floor bump. A toggle, not a slider — one keypress, one constant.
  YAGNI on a graduated version until someone wants it.
- **Luck.** The spread, rolled **per unit**. Compiling five rolls five
  times, which is what creates the compile-a-batch-and-keep-the-best loop.

### Player level is deliberately not a term

`scaled_for_level` already scales every piece of gear to its wearer, so a
level term inside quality would double-dip on the same input and make
late-game crafting compound against itself. The perk is the player-agency
half of the same idea without the collision.

## Drops

Drops roll quality too, from a **poor flat floor** — `QUALITY_DROP_BASE`,
below `QUALITY_BASE`. The world does not make good gear; your base does.

```
crafted, tier-3 bench, perk, careful   floor 100 + spread -> 100..130
field drop                             floor  70 + spread ->  70..100
```

A developed base beats the world reliably, and a lucky drop can still
surprise you. Leaving drops at a flat 100 was rejected: an average field
drop would beat a bad craft, which cuts against the whole intent. Giving
drops the crafting band was rejected for the same reason — the base would
confer no reliability advantage.

`grant_gear_drop` gains the roll alongside its existing rarity and affix
rolls. It must keep its current property of spending **no** draw on a
non-equippable, so a seeded run that picks up a material lands in the same
place it does today.

## The crafter seam

**The named axis of change is *who is compiling, and where*.** The player
at a bench today; a base-roster program at a bench in a follow-up feature
that is explicitly out of scope here but was named as the reason to build
the seam now.

```rust
pub(crate) struct CraftOrder {
    bench_tier: u32,
    perk_level: u32,
    careful: bool,
}
```

- `Game::player_craft_order(recipe, careful) -> CraftOrder` gathers it
  today.
- `Game::program_craft_order(..)` is the follow-up's second gatherer.
- The roll consumes a `&CraftOrder` and never learns there are two.

The direct version — `craft()` reading the perk level and bench tier inline
— was the null hypothesis and would normally win at one implementor. It
loses here only because the second implementor is named and requested. That
is the condition, and it should not be generalised into a habit.

**The four terms are not an axis.** They are terms in one formula; a trait
with four implementors would be the over-engineered version of this.

**No new module.** `for_quality` sits in `items.rs` beside its three
siblings; the roll sits in `game/crafting.rs` beside `craft`, the way
`roll_affix` sits beside `grant_gear_drop`.

## Naming and display

`Game::copy_name` is the one place a copy's name is built and stays so. It
gains the quality figure, which is what lets two otherwise identical copies
be told apart in a list — the whole point of the axis.

**This is the known trap.** Adding to `copy_name` widens every row on every
screen that names gear, and the repo has been bitten here before:

- The map's status column cannot grow and `draw_row` clips vertically only.
- `the_tallest_gear_page_fits_its_popup` is what says the gear inspect page
  fits, because that page has no scroll — a row past the bottom is dropped
  in silence.
- `the_widest_swap_row_still_fits_its_popup` covers the swap picker.

Both width tests must be re-run and extended to a worst-case name: longest
item name + longest affix decoration + rarity word + quality figure. A
width test that skips non-`Item` rows measures nothing here.

The format is a parenthesised percentage appended last, after the rarity
word and the affix decoration:

```
Overclocked Arc Lance of Static (115%)
```

A copy at exactly `QUALITY_DEFAULT` shows **no** quality figure, the way
`Rarity::label` returns `None` for `Ordinary`. Everything that exists in a
current save is at 100, so nothing already on screen gets wider.

`Game::gear_detail` — the `[I]` inspect page — gains a quality row. It is
one derivation and every figure on it is a call, so it picks the change up
through `copy_bonus` for free; only the explicit row is new.

### The category tag carries the quality read

The `WEP` / `ARM` / `MOD` tag on a list row is styled by the copy's
quality. **The tag only** — the rest of the row
keeps `tier_color`, which already means fusion-then-rarity and must go on
meaning that.

This is a third colour axis on a row, and it is the one `Row::Item`'s
`icon` field already argues for: its colour "is deliberately a *second*
axis from the row's `color`", because the row colour already means
something else per screen. A tag colour is that same argument for a token
that is already visually set apart, so the two axes never collide on one
glyph.

**The ramp is emphasis, not hue.** Four bands running from under spec to
over spec, each strictly more emphatic than the last:

| Quality | Colour | Weight | Reads as |
| --- | --- | --- | --- |
| 70–90 | `GRAY` | normal | under spec |
| 95–105 | `TEXT` (default) | normal | as designed |
| 110–120 | `TEXT` (default) | bold | above spec |
| 125–130 | `GOLD` | bold | exceptional |

Only the two extremes draw the eye; the middle two differ by weight alone.
Rejected in favour of this: a green→red con ramp, which spends four hues on
an axis the row colour is already using two of, and which paints an
alarming colour on gear that is merely ordinary.

**The second band is literally no change** — default colour, default
weight. Every copy in every existing save is at 100 and so lands there, so
this repaints nothing that is already on screen. That is a property the con
ramp did not have.

**The top band stays bold.** With the third band bold, a gold-but-normal
top band would read as *less* emphatic than the one below it. Emphasis has
to be monotone or the ramp inverts at its own peak.

**Known collision, accepted:** `GOLD` is already `rarity_color`'s colour for
`Rarity::Gold`, so an Overclocked exceptional copy shows a gold name and a
gold tag meaning two different things. They are different glyphs in
different columns and each colour means exactly one thing *in its own
column*, which is the distinction `Row::Item`'s `icon` field already draws.
Worth a look in a session; `YELLOW` is the alternative, though it sits close
enough to `GOLD` that telling them apart may be worse than the collision.

Only equipment carries quality, so `USE` / `MAT` / `CUR` tags keep their
current treatment and are unaffected.

**No shared con ladder.** An earlier draft of this spec had the quality read
and `game::inspection::difficulty_color` indexing one extracted four-rung
ladder, to satisfy the "a mirror must be a call, not a copy" rule. This
palette shares nothing with the con colours, so that extraction is not
needed and should not be built — `difficulty_color` is left exactly as it
is.

**The engine buckets, the renderer paints.** A pure `items::quality_band`
returns a four-variant `QualityBand`; the renderer maps a band to its
colour and weight. The engine owns the thresholds because five renderer
sites build this tag and an engine-owned rule is what stops them drifting —
the argument `Rarity::label` and `Game::copy_name` make. The renderer owns
the palette because it owns every other colour in the game, and because a
band carrying a weight as well as a hue is not expressible as a
`GlyphColor`.

**The painter already does this.** `TextRun { text, bold, color }` and
`Painter::ui_runs` draw mixed weight and colour within one line, and
`render/mod.rs::emphasize_numbers` is a working, tested precedent for
building exactly such a run inside a row's text. No new painter capability
is needed.

**Width is unchanged in all four bands.** No characters are added, and the
UI face is monospace, so bold carries the same advance. The width tests
should be re-run to confirm the bold advance rather than to absorb a new
number.

### The tag becomes a column

Those five sites each `format!` the tag into the middle of the row string
by hand, e.g.

```rust
format!("[{shortcut}] {} {}  {}", qty_column(qty), short_label(), copy_name())
```

so there is no span for a renderer to colour without re-parsing a string it
just built — which is the anti-pattern this repo avoids everywhere else.

The recommendation is to **lift the tag out of `text` into its own
`Row::Item` field**, `tag: Option<(String, Color)>`, drawn as a reserved
column the way `ICON_SLOT` already reserves a slot and paints the glyph
over it. That turns five hand-formatted insertions into one drawn column,
which is where the colour can live and is also what makes the width tests
mean something.

The direct alternative — keep the `format!` and paint over a byte offset
recorded at build time — was considered and rejected: the offset is only
knowable at each of the five call sites, so it is the same five-way
duplication with a subtler failure mode.

**Cost to be paid, not discovered:** the five screens' column layouts shift
slightly and both popup width tests need re-baselining. `inventory.rs` is
the awkward one — `inventory_row_lines` returns `Vec<String>` and only its
head line becomes a `tier_row`, so the tag must be lifted before the wrap
rather than after it.

## Balance

`balance_sim` names `Rarity` nowhere, by the documented exclusion that lets
its curves stay ignorant of a per-copy axis. Quality sits outside its gate
for the same reason and by the same argument. If its curves move because of
this change, the exclusion is wrong, not the test.

The instruments for this feature are `dev-arenas/` and a session. Numbers
compare within one build only — a moved baseline is a reshuffled RNG
stream, not a difficulty change.

## Testing

- `for_quality` at the band's ends, at 100 (identity), on a zero stat
  (stays zero) and on a negative component (unchanged).
- The chain order: a `Silver` copy at low quality never reads as worse than
  an `Ordinary` copy of *equal* quality on any stat.
- `is_plain` false at any quality but the default; a crafted piece of
  equipment lands in `GearCopies` and a crafted material still lands in
  `Inventory` and still stacks.
- Save → load: a save written before the field loads its gear at 100. A RON
  round-trip alone does not prove this.
- The roll: each term moves the floor in the right direction; the clamp
  holds at both ends; a non-equippable spends no draw.
- Per-unit rolls: compiling N can produce more than one distinct copy.
- Both popup width tests, extended to the worst-case name and re-baselined
  for the tag column.
- `quality_band` at every band boundary, and that 100 lands in the
  no-change band; a non-equipment tag is left untouched.
- Emphasis is monotone across the four bands — the pinned two-weight
  assertion `TextRun` already carries is the model.
- Every new test gets the mutation check — delete the fix, watch it fail,
  restore.

## Blast radius

| Crate / asset | What changes |
| --- | --- |
| `engine/items.rs` | the field, `default_quality`, `for_quality`, `is_plain` |
| `engine/game/crafting.rs` | `CraftOrder`, the roll, `craft` signature, `copy_bonus` chain |
| `engine/items.rs` (2) | `quality_band` and its thresholds |
| `engine/game/combat_rewards.rs` | `grant_gear_drop` roll, `copy_name` |
| `engine/game/catalog.rs` | `best_structure_tier` |
| `engine/perks.rs` | one appended variant + hook |
| `engine/tuning.rs` | the band, step, base, spread and per-term constants |
| `app-core` | the careful-compile keypress and craft dispatch |
| `gui` | Compile screen row + toggle, gear inspect row, the tag column across five sites, width tests |
| `assets/perks/` | one new `.ron` |

Two crates plus a save-shape change, so this takes the full spec-and-plan
pipeline rather than inline TDD.

## Out of scope

- **Crafting from the base roster.** Named as the reason for `CraftOrder`
  and deliberately not built here.
- Buying and buying back still produce plain copies, as they do today.
- A graduated careful-compile.
- Any change to the rarity spawn chances or the affix table.
