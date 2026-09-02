# Item power, wagon grouping, and the caravan basket

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

## What the bugs turned out to be

All three were filed against "the trade screen". There are three, and the
gaps are not where the numbering suggests:

| | `Mode::Trade` (post) | `Mode::StackMarket` | `Mode::Caravan` (wagon) |
|---|---|---|---|
| grouped | sorted, no headers | sorted, no headers | sorted; **offers unsorted** |
| stats | `equip_preview_tag` + `[I]` | `[I]` only | `[I]` on sells only |
| sell one | `[S]` sells one already | `[S]` sells the stack | `[S]` stack, Enter → quantity page |

The trading post already answers all three. **The work is the wagon**, and
bug 11 was re-scoped in the brainstorm from "show stats" to "derive a single
power scalar", which reaches every screen that names gear rather than the
wagon alone.

## Decisions taken

Recorded so they are not relitigated. Each was a choice between live
options, not a default.

1. **Absolute rating, not scaled to the wearer.** A copy's power is a
   property of the copy. Two saves reading the same copy read the same
   number.
2. **Formula A — reference-block delta**, over a tuned weight table (B) and
   a `balance_sim` time-to-kill projection (C). B was rejected under
   CLAUDE.md's "must be a call, not a copy" rule, which exists because this
   class of drift has already bitten `balance_sim.rs` four times. C was
   rejected as blind to any item whose value is a granted routine.
3. **Two surfaces**: a fixed-width column on list rows, and a per-axis
   breakdown on the `[I]` inspect page.
4. **An em dash, not a zero**, for a copy with no combat axis.
5. **Both wagon sections in one basket**, committed by Enter, screen stays
   open.
6. **One commit costs one tick.** See "The tick" below — this is the one
   decision here that changes the game rather than the screen.

## 1. `Game::copy_power`

A second derivation *from* `Game::copy_bonus`, never a parallel one. Beside
it, so the "one expression for what gear is worth" seam stays one function
with one caller-visible extension.

```
pub fn copy_power(&self, copy: &GearCopy) -> Option<ItemPower>
```

`ItemPower` carries the total and the per-axis contributions, because the
inspect page draws the breakdown and a screen that re-derived it would be a
second copy of the weighting. The engine returns the numbers; the renderer
formats them.

Input is `copy_bonus(copy, POWER_REFERENCE_LEVEL)` — a fixed level is what
makes the rating absolute, and what leaves affix, quality and fusion tier as
the only things that move it.

**Four terms, each a call into a formula that already governs play:**

- `atk` and `mitigation` → `Stats::power(reference + delta) −
  Stats::power(reference)`. Mitigation is percentage points and must not be
  summed as a scalar; `Stats::power` already prices it as the effective HP
  it buys, and its clamp to `MAX_MITIGATION_PERCENT` is what keeps the
  denominator off zero.
- `damage` → the band's mean **replacing** the reference band, never added
  to it. A weapon overrides the wielder's natural attack (`Game::
  attack_range`); adding would price a weapon as if the fists came too.
- `accuracy` and `evasion` → the shift in `battle::hit_chance` against
  `balance_sim::median_ordinary_species`, converted into the same currency
  as the terms above **proportionally**: accuracy is worth the reference's
  own offensive term scaled by `new_hit / old_hit - 1`, evasion the
  reference's own soak scaled by `old_incoming / new_incoming - 1`. A
  probability is not a quantity of anything and cannot be summed into a
  total — the same mistake as summing a mitigation percentage, which is why
  `Stats::power` prices that one as the effective HP it buys. `hit_chance`
  is the ratio form and is scale-free, so the reference zone changes the
  opponent's stats but not the shape.
- `decompiler` → **no term.** It buys taming, not combat.

`None` when every combat axis is zero.

### Why `None` and not `0`

`basket_room`'s rule: `None` is "there is no answer here" and `Some(0)` is
"the answer is nothing". A Decompiler module, a consumable and a material
have no combat axis at all, and a `0` beside a module the player paid
Credits for reads as a broken column rather than as a fact about the item.
Drawn as an em dash.

## 2. The reference block

`tuning.rs`, in its own labelled section. Difficulty and balance are code,
not data — this is a balance opinion and belongs with the others.

It needs, and nothing else may invent a second copy of:

- a `Stats` block: max HP, atk, mitigation;
- a natural damage band for the weapon term to displace;
- `POWER_REFERENCE_LEVEL`, the gear level `copy_bonus` is asked at;
- `POWER_REFERENCE_ZONE`, the zone the median species is drawn at.

Every one is a number somebody has to choose. That is the honest cost of
formula A, and the reason they sit together rather than inline: a reference
spread through the formula is a reference nobody can retune.

## 3. Presentation

### The column

`ItemTag` today lays a row out as `lead + category + GAP + name`, and
`item_text` is the one join. It gains a **power piece between the category
and the name**, so it inherits the fixed-width `row_lead` and the figures
form a straight edge down the list. That edge is the whole feature — a
staggered number cannot be scanned.

**It must not be the `suffix` field.** `suffix_x` places a suffix one inset
past each row's *own* right edge, so power figures would stagger with the
name lengths above them.

`a_tagged_rows_pieces_join_back_into_its_text` must still hold: the pieces
`draw_row` hands the painter have to join back into exactly the string every
width test measures. A row measured from one set of pieces and drawn from
another is a row whose suffix lands on its own tail.

Every screen already calling `with_tag` gets the column at once — inventory,
swap picker, all three traders, crafting.

A row naming no item at all — the wagon's Routine and Program offers, which
pass a blank category today — draws a **blank**, not an em dash. The dash
means "this item has no combat axis"; a blank means "this row is not an
item". A dash on a Routine Disk would claim the disk was rated and found
wanting.

### The breakdown

`Game::gear_detail` gains a block naming what each axis contributed.

**That page has no scroll.** `draw_popup` pages a `Row::Item` span and the
inspect page has none, so a row past the bottom is dropped in silence.
`the_tallest_gear_page_fits_its_popup` is what says it fits, and it has to
grow with the block.

### The trap: an absolute column disagrees with the swap delta

CLAUDE.md pins that a worn item and a candidate are scaled at two different
levels, and that this is the point — gear locks in `EquippedItem::level`.

The power column is a property of the **copy**. The existing `stat_summary`
delta is a property of the **swap**. On `Mode::EquipSwap` they can point in
opposite directions, and that is correct: the column says which piece is
better in the abstract, the delta says what changes if you put it on now.

Nothing should "fix" this by making the column contextual. That would undo
decision 1 and give the same copy two different numbers on two screens.

## 4. Grouping the wagon (bug 10)

Sell rows already arrive category-sorted through `player_status().
inventory`, which sorts by `(category_sort_key, rarity, tier)`. Offers do
not — `caravan_shelf` re-reads its weights per row, so the drawn order is
shuffled by construction.

**Both lists get category headers; the offer list also gets sorted.**

The group key is over `CaravanOfferKind`, not `ItemCategory`, because two of
the four kinds are not items:

- `Gear(copy)` → that item's `WEP` / `ARM` / `MOD`
- `Material(item)` → `MAT`
- `Routine(_)` → its own head
- `Program(_)` → its own head

**Exhaustive on the kind**, `cell_mark`'s rule. As a `_ =>` arm a fifth kind
ships into an unlabelled run and nothing fails.

Two things that make this safe, both verified in source:

- **`CaravanOffer::index` is assigned by `.enumerate()` before display** and
  `caravan_spent` keys on it; `buy_caravan_offer` resolves by
  `find(|o| o.index == index)`. Re-sorting the returned `Vec` moves rows on
  screen and moves no shelf identity.
- **Headers are `Row::TextColored`**, so they never touch the `idx` that
  `caravan_row` resolves — that counter increments only on item rows.

## 5. The caravan basket (bug 12)

`Mode::CaravanQuantity` is retired. One amount per row, edited in place,
modelled on `app/basket.rs`.

### The two ceilings, and why they differ

The same asymmetry the transfer picker has, for the same reason:

- **Sell rows: per row and static**, clamped at `held`. What you are holding
  of that item is not affected by what you set on another row.
- **Offer rows: 0..1.** A shelf slot is spent whole. `CaravanOffer::qty` is
  part of the price the player was quoted, so there is no per-unit amount to
  choose.
- **Buys share one budget**: `credits + proceeds of pending sells − other
  pending buys`. Subtracting only the *other* rows is what lets the
  highlighted row be lowered and raised while it is being edited.

### The commit order

**Sells land before buys.** `transfer_items`' take-before-give rule, and
here it is what lets a basket be funded by its own sales — the whole reason
the two sections are one basket. The other order clamps the buy to zero
*silently*, which is the failure mode that rule exists to prevent.

Every refusal must land before anything is spent. `buy_caravan_offer`
already holds that ordering and states why: a purchase that took the
Credits and then failed is the one bug the player cannot undo, and a caravan
has no buyback to put it right with. A basket makes this stricter, not
looser — a partially committed basket is the same bug wearing a bigger coat.

### Keys

Enter commits, clears the amounts and rebuilds the view; the screen **stays
open**. Esc leaves. `[N]` clears everything, `[I]` inspect unchanged.
Left/Right step, Shift to an end, Ctrl halves the gap.

**`[A]` fills the sell rows only.** On the picker it writes the take ceiling
over every row, and the take side is the one with a per-row ceiling; here
that is the sell side. Filling the offer side would spend the entire purse
on one keypress, on a screen with no buyback to undo it.

**Editing costs no tick.** Only the commit does, so a wagon cannot roll away
between two arrow presses.

`half_way_to`'s `div_ceil` is what makes the Ctrl step terminate — rounded
down, a gap of one gives a step of zero and the key goes dead with the row
neither full nor empty.

**Right increases, Left decreases — not the picker's inversion.** That
inversion is specified for a single row that spans both directions and can
be signed. Here the sign is fixed by which section the row is in, so
inverting would read as a slip rather than as a specification.
`left_puts_in_and_right_takes_out` is about `Mode::Transfer` and is
unaffected.

### The modifier fold

`Mode::Caravan` joins the one condition at `app/input.rs:130`:

```
_ if self.mode == Mode::Transfer => key,
```

Miss it and the four modified-arrow variants are folded to bare `Left` /
`Right` before the caravan handler sees them — Shift and Ctrl silently
become plain steps. Widening this one condition is the documented way a
second screen takes a modifier; doing it in the renderer instead puts "what
a modifier means" on the far side of the seam from the mode that decides it.

### The tick

**One commit costs one tick**, where today N trades cost N ticks.

A caravan departs on a tick, so a large basket is now cheaper in trader-time
than the same trades made one at a time. This is deliberate: the basket is
the visit, and charging per line would make the screen's whole point — decide
everything, then commit — the expensive way to use it.

It is an economy change and not only a UI one, and it is recorded here so
that a later reading of "a trade costs a tick" does not mistake it for an
oversight. `close_if_gone` still runs after the commit: the tick spent may
be the one the trader leaves on, or the one that starves the player.

## 6. Testing

TDD throughout, failing test first, `cargo test --workspace` as the gate.

**Engine**
- `copy_power` per axis: atk, mitigation-as-soak, a weapon band replacing
  rather than adding, accuracy and evasion through `hit_chance`.
- A census that every shipped equipment item rates `Some`.
- A Decompiler module, a consumable and a material each rate `None`.
- Mutation-proved: delete each term, watch the matching test fail.

**app-core**
- Both ceilings, including that setting one sell row does not move another's.
- A basket funded by its own sales commits whole — the case that fails if
  the order is reversed.
- Enter leaves the screen open with amounts cleared.
- `caravan_row` still maps drawn rows to sections with headers present.

**gui**
- `no_caravan_row_overflows_its_popup` extended for the new column.
- `the_tallest_gear_page_fits_its_popup` extended for the breakdown block.
- `a_tagged_rows_pieces_join_back_into_its_text` still holds with three
  pieces.

Both width and height censuses are already tight — the map's status column
holds 38.5 monospace cells and the widest shipped buff row spends all but
3.8 of them. A new column is a real budget spend, not a free line.

## Out of scope

- `Mode::Trade` and `Mode::StackMarket` keep their current sell keys. They
  gain the power column for free through `with_tag`, and nothing else.
- No save-format change. `ItemPower` is derived on every read, never stored
  — `Platform`'s radius rule and `Memories`' intensity rule.
- No new asset schema. The reference block is tuning, which is code.
