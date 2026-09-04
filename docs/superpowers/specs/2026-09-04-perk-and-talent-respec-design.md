# Perk and talent respec

**Status:** implemented

Buying a perk or a talent is permanent. This adds one way to take it all
back: a flat Credit price wipes every purchase on one ledger, refunds every
point spent, and subtracts the stats those purchases baked in.

Two screens, one rule: `Mode::Perks` for the player's perks, and
`Mode::DevelopProgram` for one companion's talents.

## The problem this has to solve

Refunding the *points* is trivial — `Perks::points` is a stored number and
`talent_points` derives `spent` from the length of `Talents`. Taking back
what a purchase did is not, because four purchases write into `Stats` and
none of them is invertible from what is stored today:

| Purchase | What it writes | Why it can't be undone |
|---|---|---|
| `Perk::Attacker` | `stats.atk += ATTACKER_BONUS_PER_LEVEL` | Flat, but a retune rewrites history |
| `Perk::Defender` | `stats.mitigation += DEFENDER_BONUS_PER_LEVEL` | Same |
| `Perk::Buffer` | `max(round(max_hp * BUFFER_BONUS_PERCENT_PER_LEVEL), BUFFER_MIN_BONUS_PER_LEVEL)` | Percentage of the *then-current* max, and the floor makes it many-to-one |
| `TalentNode::Stat` | `refactor::raised(stat, percent)` | Same shape, same floor |

A respec that refunds points without un-baking is an infinite stat printer:
buy `Buffer`, respec, buy `Buffer` again, each cycle compounding off a
larger maximum. So the stats must come back out, exactly.

## Approach: a grant receipt

A new component records what purchases have granted, so a respec has exactly
one thing to subtract.

```rust
/// Every stat point on this creature that came from a spendable choice —
/// a perk level or a `TalentNode::Stat` — and can therefore be taken back.
///
/// The receipt exists because neither grant is invertible from `Stats`:
/// `Perk::Buffer` and `TalentNode::Stat` are percentages of a maximum that
/// has since moved, and both floor at a whole point, so reversing the
/// arithmetic is off by a point per level in the common case and silently
/// rewrites history whenever a tuning constant is retuned.
///
/// One component serves both ledgers: the player has no `Talents` and a
/// companion has no `Perks`, so the two can never write it at once.
///
/// Absent means nothing bought, like `KernelRing` and `Refactors`.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct BoughtStats {
    pub atk: i32,
    pub mitigation: i32,
    pub max_hp: i32,
    /// How many purchases this creature has *ever* made, which a respec
    /// does not reset — see `convert_overflow_xp` below.
    pub ever_bought: u32,
}
```

Rejected alternatives, so they are not re-proposed:

- **Per-purchase records** (`unlocked: Vec<(Perk, StatGain)>`). Supports
  refunding one purchase at a time, which the design does not want, and
  `PlayerSave::unlocked_perks` is bincode-positional save format, so
  changing its element type is a genuine save break.
- **Inverting the arithmetic at respec time.** No new storage, but both
  grants floor at a whole point, so the inversion is many-to-one; and a
  retune of `BUFFER_BONUS_PERCENT_PER_LEVEL` or a talent's `percent`
  silently changes what an old save's respec hands back.

## The overflow-XP exploit

`Game::convert_overflow_xp` (`game/unlocks.rs:118`) mints Perk Points from
banked cap XP, pricing each at
`OVERFLOW_XP_BASE + OVERFLOW_XP_STEP * held`, where `held` is
`perks.unlocked.len()`. That escalator is the only thing keeping banked XP
from being an unbounded linear power source — its own doc comment says so.

A respec empties `unlocked`, which resets the escalator to the opening rate.
At the level cap that is a loop: buy perks, respec, mint the rest cheap.

**The fix is one line at the read site**: `held` becomes
`BoughtStats::ever_bought`, which is monotonic by construction. For a save
that has never respecced the two are equal, so the curve does not move for
anyone who does not use the feature.

`ever_bought` is incremented by perk purchases only. A companion's talents
have no bearing on the player's XP drain, and counting them there would be
the drift a shared field invites.

## Engine interface

```rust
// crates/engine/src/game/respec.rs — a new module beside game/unlocks.rs

impl Game {
    /// Refunds every perk level for a flat Credit price.
    pub fn respec_perks(&mut self) -> Result<(), String>;

    /// Refunds every talent `entity` has taken, same price.
    pub fn respec_talents(&mut self, entity: Entity) -> Result<(), String>;

    /// What a respec costs and what it would hand back, for the screens'
    /// prompt and the confirm page. Derived, never stored.
    pub fn respec_quote(&self, subject: RespecSubject) -> RespecQuote;

    /// Subtracts the receipt from `entity`'s `Stats` and zeroes it.
    /// The one writer, shared by both doors above.
    fn unbake_bought_stats(&mut self, entity: Entity);
}
```

`views::RespecQuote` carries `cost`, `credits`, `points_returned`,
`purchases`, and `refusal: Option<String>` — every figure a call, none
stored, `views::BuildOrderRow`'s rule.

Two doors rather than one generic `refund(entity)`, because what comes back
besides stats differs — points against a derived count, routines against
none. One shared helper, because two `Stats` writes would be two chances to
disagree.

### `unbake_bought_stats`

```rust
let gear = self.gear_bonus(entity);
self.apply_equipment_delta(entity, gear, -1);
{
    let mut stats = self.world.get_mut::<Stats>(entity).unwrap();
    let bought = /* the receipt */;
    stats.atk -= bought.atk;
    stats.mitigation -= bought.mitigation;
    stats.max_hp -= bought.max_hp;
    stats.hp = stats.hp.min(stats.max_hp);
}
self.apply_equipment_delta(entity, gear, 1);
```

Gear is lifted and put back around the write, exactly as `bake_talent_stat`
and `refactor_companion` do it, and for CLAUDE.md's stated reason: a bonus
sitting in `Stats` during the operation is scaled, and the later unequip
subtracts only the unscaled amount, welding the difference in permanently.

Current HP clamps to the new maximum rather than refilling. A respec must
not be the strongest heal in the game — `bake_talent_stat`'s own argument,
one direction over.

### `respec_perks`

Refusals, all before a Credit moves:

1. `is_game_over().is_some()` or `has_active_battle()` → "Can't do that right now."
2. `Perks::unlocked` empty → nothing to take back.
3. Credits held below `RESPEC_CREDIT_COST` → the shortfall, named.
4. `pet_count()` exceeds `pet_capacity() - roster_slot_bonus(perks)` →
   refuses rather than silently over-filling the roster, since
   `Perk::ProcessPool` is the one term in `pet_capacity` a respec takes
   away. Computed at the call site from the two existing calls rather than
   given a helper of its own — one caller, and a named
   `pet_capacity_without_process_pool` invites a second reader who means
   something else by it. The message names how many programs must be
   released first.

Then, in order: take the Credits with `Inventory::take`; `unbake_bought_stats`;
`perks.points += sum of each unlocked perk's current catalogue cost`;
`perks.unlocked.clear()`; log. Points refund at the catalogue's *current*
price — a perk whose `.ron` cost was retuned refunds what it would cost
today, which is the only price the player can see.

No `note_deed`. `unlock_perk` records `Deed::UnlockedPerk` for contracts,
and a respec is not an unlock; recording one here would let a wipe-and-rebuy
loop farm a contract objective.

`ever_bought` is untouched.

### `respec_talents`

Refusals, same ordering rule: game over or battle; not the player's program
(`take_talent`'s wording); no talents taken; not enough Credits.

Then: take the Credits; `unbake_bought_stats(entity)`; `Talents::0.clear()`;
**re-derive routines**; log. Points need no refund — `talent_points` derives
`spent` from the length of `Talents`, so clearing the list *is* the refund.
That is the payoff of "points are derived, never stored."

Re-deriving routines is required, not cosmetic, because two node kinds
reach outside `Stats`:

- `TalentNode::RoutineSlot` widens `Game::routine_slots`, so revoking it
  can leave more routines installed than there are slots.
- `TalentNode::Ability` installed a routine through
  `install_unlocked_routines`, which may have evicted
  `FALLBACK_ABILITY_ID` to make room.

**Corrected during implementation.** This section first said "clear
`Routines` and re-run `install_innate_routines`". That is wrong:
`install_innate_routines` treats whatever is already in `Routines` as
`carried` — what the program was found holding in the field — and the
player also installs routines by hand from disks. Clearing the component
destroys both, and the tree has no claim on either.

What ships instead, `rebuild_routines_after_talent_loss`: ask
`talent_abilities` what the tree granted **before** the list is cleared,
`retain` everything else, `truncate` to the now-narrower
`routine_slots(entity)`, then call `install_innate_routines` to refill from
the species kit if the truncate left room and to put the placeholder back
if the program is left holding nothing. `a_talent_respec_leaves_a_hand_
installed_routine_alone` is the test that pins it.

## App-core

A confirm step, not a bare keypress. Both screens' other keys are
single-press purchases, and a full wipe is the one action on either that
cannot be undone by pressing it again.

- Two new `Mode` variants: `RespecPerksConfirm` and `RespecTalentsConfirm`.
  Both join `ALL_MODES` (88 → 90) and every other mode census.
- **`X` opens the confirm from both screens.** Uppercase is forced:
  `selected_index` maps lowercase letters to rows past the digits
  (`DIGIT_ROWS + (c - 'a')`) and the perk picker has eighteen rows, so a
  lowercase key would both pick a perk and fire the wipe. Not `R` — that
  is already the kernel ring on `Mode::DevelopProgram`.
- The confirm takes `y` to commit and `Esc`/anything else to back out to
  the screen it came from. `pending_develop_target` carries the companion
  through, exactly as it does for `take_talent`.
- Outcomes go through `App::report`, so a refusal lands on both surfaces —
  `App::refuse`'s rule.

## GUI

- **Neither picker gains a row.** The spec first said "a footer row on
  each"; both popups have no scroll, and the perk screen already lists
  eighteen perks. So the perk screen's price rides the instruction line it
  already had (`PopupSize::Large` carries ~114 monospace cells, well clear
  of it), and the talent ladder's row is drawn only when `points.spent > 0`
  — otherwise the capstone tier pays for a line saying there is nothing to
  unwind.
- One `draw_respec_confirm` for both wipes, drawn from `RespecQuote` alone.
- `ALL_MODES` is a hand-written list, so a new `Mode` variant does **not**
  fail to compile against it — the `cell_mark` trap one screen over. Both
  variants are added (88 → 90), with `RespecTalentsConfirm` in
  `NEEDS_PENDING_STATE` because the census app sets no
  `pending_develop_target`.

## Tuning

`RESPEC_CREDIT_COST: u32` in `tuning.rs`, one constant for both ledgers.
Flat by decision — a price that scales with what is being undone reads as a
tax on experimenting, and an escalating one needs a stored counter per
subject.

## Save

Additive only, behind `#[serde(default)]`, so **no `SAVE_FORMAT_VERSION`
bump** — the save is field-named RON and that is what retired migrations.

- `PlayerSave`: `bought_stats: BoughtStats`
- `CreatureSave`: `bought_stats: BoughtStats`

An existing save loads with the receipt at zero, which is honest for talents
and perks bought before this shipped: their stats stay, and a respec hands
back the points without the stats. That is a real seam and the tests must
state it rather than pretend otherwise.

`ever_bought` defaults to 0, which would read an old save's overflow price
lower than it was. Load seeds it as
`max(saved.ever_bought, unlocked_perks.len())` — **not** `unlocked_perks.len()`
alone, which would be exact for an old save and wrong for every new one: a
save taken *after* a respec has an empty `unlocked` and a non-zero count,
and the plain assignment would throw the count away and re-open the exploit
across a save/load.

A `#[serde(skip)]`-shaped mistake here would leave a RON round-trip test
green while the field never persists, so each of the two fields needs a
save→load test, not just a round-trip.

## Tests

Engine:

- A `Buffer` level bought, then respecced, returns `max_hp` to exactly its
  pre-purchase value — the test the whole receipt exists for, and it must
  fail if `unbake_bought_stats` is removed.
- Buy, respec, buy again, twice over: `max_hp` after the second purchase
  equals `max_hp` after the first. The compounding printer, closed.
- The same pair for a `TalentNode::Stat` companion.
- Gear equipped across a respec: the bonus is neither scaled nor welded in
  (equip, respec, unequip, compare to base).
- HP does not rise on respec.
- Talent respec with a `RoutineSlot` talent taken and every slot filled:
  routines afterwards equal a fresh companion's of that species and level.
- Each refusal spends nothing — asserted **per refusal**, one test each,
  since a single test over one of five passes against four paths that never
  spend anyway. `commit_caravan_basket`'s rule.
- `convert_overflow_xp` prices off `ever_bought`: buy, respec, and the next
  minted point costs what it would have without the respec.
- Roster over capacity refuses.
- A save round trip carries both receipts, and a load seeds `ever_bought`.
- The pre-existing `the_overflow_price_rises_with_perks_held` wrote
  `Perks::unlocked` directly, so the knob it turned is no longer the one the
  price reads. Renamed to `..._with_perks_bought` and given the receipt; its
  assertion is untouched, because the behaviour it states has not changed.

App-core:

- `X` opens each confirm; `y` commits; `Esc` returns to the right screen.
- Lowercase `x` is still a row label and never the wipe.
- A refusal never backs out of the confirm.

GUI:

- The confirm page fits its screen at 1280x720.

## Out of scope

Refunding a single purchase; a respec for the player's *class* or
`Refactors`; any change to what perks or talents cost to buy.
