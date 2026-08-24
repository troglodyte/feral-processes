# Gear fusion across quality and affixes

**Status:** approved, not implemented
**Date:** 2026-08-24
**Origin:** bug #2 on the user's TODO list — "you should be able to fuse
items with different quality and affixes. the quality should average, and
the affixes should carry over", extended in conversation to "if there's a
secondary effect, they should be carried forward even if that makes
duplicate or multiple effects".

## The problem

`Game::fuse_copy` requires the two copies to be equal as whole `GearCopy`
values — item, rarity, tier, affix **and** quality. Quality shipped as a
fourth axis rolled in `QUALITY_STEP`s across `QUALITY_MIN..=QUALITY_MAX`,
which is thirteen buckets; affixes are a fifth. The practical effect is
that two field-found copies of one item almost never match, so fusion —
the game's only sink for spare gear — stopped firing for anything that
was not crafted or bought.

Relaxing the match is half the fix. The other half is that a copy carries
**one** `affix: Option<AffixId>`, so a fusion of two affixed copies has
nowhere to put the second. The decision taken is that affixes accumulate:
a copy carries a list, duplicates included.

## Decisions taken

| Question | Answer |
|---|---|
| What must still match | item, rarity, tier. Quality and affixes go free. |
| Which copy is the survivor | the one the player pressed `[U]` on. |
| Which copy is consumed | automatically the **best** eligible partner — highest quality first. |
| Two different affixes | both carry forward; the result holds the union, duplicates kept. |
| Quality of the result | the two averaged, snapped to the nearest `QUALITY_STEP`, ties **down**. |
| Ceiling on affixes | none. |
| Save format | compat shim, **no `SAVE_FORMAT_VERSION` bump**. |
| Name of a multi-affix copy | first prefix, first suffix, then `+N`. |

Rarity stays matched. The existing argument holds and is unchanged: there
is no midpoint rare tier for a Gold-plus-Ordinary fuse to land on, so
either parent's tier would be laundered into or out of the result
depending on which one won. `a_rare_copy_will_not_fuse_with_a_plain_one`
stays green untouched, and is the test that says this was considered.

## The data shape

`items::GearCopy::affix: Option<AffixId>` becomes
`affixes: Vec<AffixId>`.

**The list is kept sorted, always.** `GearCopy` is the key of the
`components::GearCopies` ledger, of `EquippedItem`, and of a trader's
buyback shelf; all three find rows by `==`. `[A, B]` and `[B, A]` are the
same copy to a player and must be the same copy to `Eq`, or one is
written to a row and looked up at another — which reads as gear vanishing
out of cargo, the failure `GearCopy`'s own doc comment is written to
prevent. Sorting is the canonical form and there is exactly one place
that builds a list from two: the fusion. Duplicates survive sorting, and
are the point.

`GearCopy::is_plain` gains `self.affixes.is_empty()` in place of
`self.affix.is_none()` — the doc's "a fourth property joins the `&&` here
and nowhere else" is unchanged in spirit.

`Game::affix_of` becomes `Game::affixes_of(&self, copy) -> Vec<&AffixDef>`,
and keeps its whole compatibility story: an id the build no longer knows
resolves to nothing and is skipped, so a save naming a deleted affix
loads as a copy with one fewer effect rather than failing. Every reader
still goes through it.

## Fusion

Eligibility is a new predicate beside `is_plain`:

```rust
/// Whether two copies may be fused into one.
pub fn fusable_with(&self, other: &Self) -> bool {
    self.item == other.item && self.rarity == other.rarity && self.tier == other.tier
}
```

`fuse_copy` then:

1. Refuses as it does today for a non-equippable and for
   `tier + 1 > MAX_FUSIONS`, both still **above** the first `take_copies`,
   so a refused fusion spends nothing.
2. Builds the candidate partners: every eligible copy in cargo, plus the
   copy worn in that slot if it is eligible. The survivor's own stack
   contributes a partner only when it holds two or more.
3. Picks the best partner by a **total** order — highest `quality`, then
   cargo before worn, then fewest affixes, then the affix ids themselves.
   A total order matters because `GearCopies::copies` is a `Vec` in
   insertion order, which is play-history dependent; without the full
   tie-break the same cargo could fuse differently between two saves.
   "Fewest affixes" preserves the more interesting spare for a later
   fusion when quality ties.
4. Refuses with a count over the **whole eligible group** if there is no
   partner. Today's `Need 2 X to fuse (have 1)` counts exact matches only
   and would now be a lie.
5. Produces one copy: the survivor's `item`/`rarity`, `tier + 1`,
   `affixes` = both lists concatenated and sorted, `quality` = the two
   averaged.
6. Consumes the survivor and the partner. If either was the worn copy the
   result takes the slot, with the equip delta swapped through
   `worn_bonus` exactly as today, so the player is never left bare.
   Otherwise it lands in cargo.

Quality averaging, in integers, ties down:

```rust
(a as u32 + b as u32 + QUALITY_STEP as u32 - 1) / (2 * QUALITY_STEP as u32) * QUALITY_STEP as u32
```

75+90 → 80, 85+85 → 85, 90+95 → 90, 70+130 → 100. It never rounds up, so
no fusion buys quality, and it always lands on a `QUALITY_STEP` multiple —
which every drop and every craft roll already guarantees, so no screen
learns to show a figure no roll could produce.

`fuse_all_items` needs no rule of its own. It iterates a snapshot of
tier-0 rows taken before any fusing starts, so it pairs greedily inside a
group and still cannot cascade; `fusing_all_pairs_promotes_every_stack_once`
holds unchanged.

**Nothing else gains a second affix.** `roll_affix` still rolls at most
one, still spends no draw on an empty pool, and its RNG-stream position is
untouched. Fusion is the only source of a multi-affix copy.

## Naming

`Game::copy_name` stays the one place a copy's name is built. Over the
resolvable affixes in sorted order it takes the **first prefix** and the
**first suffix**, and appends `+N` for however many resolvable affixes
were not named:

> `Overclocked Honed Arc Lance of Static +3 (85%)`

`+N` goes after the affix decoration and before the quality figure, which
stays last for the reason it already is. It is omitted at zero, the call
`Rarity::label` makes for `Ordinary` and `copy_name` already makes for a
copy at spec — a copy with one prefix and one suffix names both and gains
nothing, so no existing name moves.

`SWAP_NAME_COLUMN` is 57, documented as wide enough for the longest name
the shipped assets can build. `+N` adds up to three cells and N is
bounded by the fusion arithmetic — `ITEM_FUSION_COST` 2 to the power of
`MAX_FUSIONS` 3 is eight source copies, so `+7` is the widest suffix
possible. `no_shipped_copy_name_outgrows_the_swap_name_column` must be
extended to sweep multi-affix copies at the longest prefix and longest
suffix together, and the constant raised to whatever that measures.
`the_widest_swap_row_still_fits_its_popup` is the gui-side gate and stays
the authority on real text.

## The inspect page

`[I]` is the only screen that can tell the player what a copy's affixes
actually do, and with stacking it has to: an affix may be a trade-off
whose stats include negatives, so a hidden one is a drawback the player
cannot account for.

`views::GearDetailView` gains an `affixes: Vec<String>` — one line per
affix, built in the engine, **duplicates folded** as `of Static ×3`.
Folding is not cosmetic: eight affixes drawn from a pool of nine will
usually be three or four distinct ones, so the common worst case collapses
to a handful of rows.

The page still does not scroll. `draw_popup` pages a `Row::Item` span and
this page has none, so a row past the bottom is dropped in silence — the
trap `the_tallest_gear_page_fits_its_popup` exists to catch. Rather than
give the page a scroll, the affix block is **capped by what fits**, in
`cap_entries`' idiom: trade-off affixes (any negative stat) sort first so
a drawback is never the line that falls off, and the overflow reads
`+N more`. Storage stays uncapped, as decided; only the drawing is
bounded.

The height census must be extended: it currently sweeps
`GearCopy::plain(def.id)` for every item, which has no affixes at all and
so measures nothing about this page. It needs the worst case — the
tallest item's page carrying a full affix block.

## Save compatibility

No `SAVE_FORMAT_VERSION` bump. The shim lives entirely on the save
structs, never on `GearCopy` itself — a legacy field on the live type
would join its `Eq` and split the stores, which is the failure this whole
type exists to prevent.

- `SaveData::gear_copies: Vec<(GearCopy, u32)>` becomes
  `Vec<(GearCopySave, u32)>`. `GearCopySave` is flat and named with the
  same field names, plus `#[serde(default)] affixes` and the retained
  `#[serde(default)] affix`. RON absorbs this because the tuple's first
  element is still a field-named struct and the one new field defaults.
  This is `EquippedItemSave`'s own trick, applied for its own stated
  reason.
- `EquippedItemSave` gains `affixes`, keeps `affix`.
- `PlayerSave`'s three flat `weapon_affix`/`armor_affix`/`module_affix`
  fields gain `*_affixes` counterparts and keep the singular ones.

Load takes `affixes` when non-empty and otherwise lifts `affix` into a
one-element list. Save writes `affixes` and leaves the legacy field
empty. The legacy fields are then written by nothing and read on load
only — `PlayerSave::fused_gear` is the precedent, and
`Experience::xp_to_next` the shape.

One consequence worth writing down: a save written by this build and
opened by an older worktree (the RON round-trip downgrade) loses its
affixes silently rather than failing, because the old build reads only
the singular field. That is the price of not bumping.

`arena::scenario::EquipSpec::affix` becomes `affixes: Vec<AffixId>`
outright, no shim. Nothing in `dev-arenas/` authors it today, and a
scenario field that is silently ignored is a known trap in this repo —
so the rename is safer than accepting both. `dev-arenas/README.md`
changes with it.

## Balance

Stacking flat affix bonuses is a real power move, not just a data change.
Affix stats are added to the **base** before all four scaling axes, so
eight of them on a T3 copy compound through `scaled_for_level`,
`for_quality`, `fused_for_tier` and `for_rarity` together.

`balance_sim` models gear stats but no abilities, and it does not model
fusion at all, so it will not see this. The instrument is `dev-arenas/`:
stage the same fight with a plain copy, a one-affix copy and a
four-affix copy and compare within one build. Recorded here so that
"the curves did not move" is not mistaken for evidence.

This is a gate on shipping the feature, not on writing it.

## Testing

Engine, in `tests/equipment.rs` unless noted:

- two copies differing only in quality fuse, and the result is the
  average snapped down — a table over the interesting pairs, including
  the two tie cases.
- two copies with different affixes fuse and the result carries **both**,
  sorted.
- two copies with the *same* affix fuse and the result carries it
  **twice** — the duplicate case, which is what was asked for.
- `copy_bonus` sums every affix, and a duplicate counts twice.
- an affix id no build knows is skipped by `affixes_of` and by
  `copy_bonus`, and the rest of the list still resolves.
- the partner chosen is the highest-quality one, with a fixture holding
  three eligible spares so "it picked the only other one" cannot pass.
- a worn eligible copy is folded in and the player is still wearing the
  result.
- a rare copy still refuses a plain one, and the refusal spends nothing.
- `fuse_all_items` pairs across quality and affixes, one pass, no
  cascade.
- inline in `save.rs`, beside the `GearCopyProbe` tests that already
  parse RON fragments by hand: a v-current save carrying the legacy
  singular `affix` loads with a one-element list, and a fragment
  carrying `affixes` loads with all of them. In `tests/equipment.rs`:
  a `Game::save`/`Game::load` round trip of a two-affix copy preserves
  both, and `[A, B]` and `[B, A]` are one row in `GearCopies`, not two.
  A RON round trip alone cannot catch a skipped field, so the save/load
  pair is the one that counts.

app-core: `no_shipped_copy_name_outgrows_the_swap_name_column` extended
to multi-affix copies.

gui: `the_tallest_gear_page_fits_its_popup` extended to a full affix
block; `no_gear_row_overflows_its_popup` in the same shape as the memory
page's width census.

Every new test is mutation-proved — delete the fix, watch it fail — per
the standing evidence rule.

## Out of scope

- No cap on affixes, no diminishing returns, no burn-out. That is the
  parked item-synergy work (`2026-08-17-item-synergy-burnout-parked.md`)
  and this change deliberately does not open it.
- Drops still roll at most one affix. Fusion is the only way to stack.
- No partner picker screen. The partner is chosen automatically; if that
  turns out to feel wrong in play, the picker is a separate change with
  its own `Mode`.
