# Putting items into a Depot

**Status:** approved, not implemented.

A Depot is the only structure the player can currently take *out* of and
never put *into*. `c` empties every adjacent output buffer into the pack;
nothing goes the other way. This adds the second half: a picker that moves
plain cargo out of `Inventory` and into an adjacent Depot's `Stock::output`.

## Why

Two things it buys, and they are the same mechanism seen from either end.

A pack fills up. Everything a run produces that is not immediately spent
rides in `Inventory` forever, and the only sinks are a bench, a trader and
a breach. A Depot beside the anchor is the obvious place to leave 60 Cache
Grain, and the game currently has no gesture for it.

More important: `base_holding` and `work_orders::feeders_for` already count
depot buffers. So a deposit is not a stash — it is *handing the base your
materials*. A work order that stalls for want of an ingredient the player is
personally carrying is a real and currently unfixable state, and this is the
verb that fixes it. That the two readings are the same code path is the
argument for depositing into `output` rather than growing a third buffer.

## What the player does

Stand beside a Depot in base space. Press `P`. A row per item in the pack, a
quantity per row, Enter commits. One tick, one log line, and the goods are
in the base's hands.

Refused with a sentence when there is no Depot adjacent, or when the pack
holds nothing depositable. Refused silently — as every other base verb is —
mid-battle, on the surface, or underground.

## The shape of the decision

Four settled choices, so they are not relitigated:

**A Depot, not any `Stock`.** `adjacent_stock()` returns every neighbouring
buffer, and mirroring collect exactly would let the player push Cache Grain
into a Mining Node's output. That is the same objection CLAUDE.md already
records against the dig crew's put-back: *a unit pushed into a machine's
output reads as something that machine produced*. `StructureDef::stores` is
the filter, and it is the field that already distinguishes a Depot for the
hauling system.

**Plain copies only.** `Stock` keys by `ItemId` alone, so a rare or fused or
high-quality copy put into one would come back out ordinary. `Inventory` is
by definition the plain-copy store and `GearCopies` holds everything else,
which is exactly why "nothing puts a player's copy into a `Stock`" is a
standing seam — the production chain has no rarity rule because it can never
meet a special copy. Reading `Inventory` and never `GearCopies` keeps that
true by construction rather than by a check.

**Banked items excluded.** `PlayerStatus::inventory` already filters
`ItemDef::banked`, and this list must too. Research Data is the game's one
banked item; a bank is not cargo, and putting it in a depot would make it
spendable by the base as though it were.

**Into `output`.** See Why. It is what makes the deposit reach
`base_holding`, `feeders_for`, `collect_adjacent` and the stock strip with no
new plumbing at any of them.

## Engine

A new `crates/engine/src/game/base/deposit.rs`, mirroring `collect.rs`
function for function. The mirroring is the point: every rule collect states
about ordering, clamping, ticking and refusing has a twin here, and two
modules that read as reflections of each other are two modules a reader can
check against one another.

### `adjacent_depots(&self) -> Vec<Entity>`

`adjacent_stock()` filtered to structures whose `StructureDef::stores` is
set, preserving its `(x, y)` sort. The sort carries collect's reason
unchanged: a *partial* fill across two adjacent Depots has to fill them in
the same order every run, or an identical save answers the same keypress
differently between two runs. Filling everything could not see this.

`pub(crate)`, so the tests can pin the order directly.

### `Game::depositable(&self) -> Vec<(ItemId, u32)>`

What the player may put in: the `Inventory` rows that are not `banked`,
**sorted** into `ItemId` order — the order `Stock`'s own `BTreeMap` yields and
the order the deposit log will print in. The sort is explicit, unlike
`collectable_adjacent`'s: that function pools into a `BTreeMap` and gets the
order for free, while `Inventory::items` is a `Vec` in insertion order, so an
un-sorted list here would put the rows in the order the player happened to
pick things up.

`&self`: no tick, no log, no RNG. It holds the same guards `deposit_items`
does and answers with an empty offer for each — game over, an active battle,
`require_base` failing, or no adjacent Depot. It is a claim about what is
beside the party, so like `collectable_adjacent` it needs no `require_surface`
of its own; `require_base` is the stronger statement.

The banked filter is the idiom `party.rs` already uses:
`!db.get(item.as_str()).is_some_and(|d| d.banked)`.

### `Game::deposit_room(&self) -> u32`

Summed `Stock::output_room()` across `adjacent_depots()`. Exists because the
picker needs a shared budget it can enforce live; see the app-core section.
Same guards, same empty answer.

### `Game::deposit_items(&mut self, give: &[(ItemId, u32)]) -> Vec<(ItemId, u32)>`

The one giving path, and `deposit_adjacent` below is this with everything
offered, so put-all and put-some cannot drift on clamping, logging or the
tick.

Clamped **twice**, and neither clamp is a refusal:

- against what the player actually holds, and
- against `output_room()`, per Depot, as the fill walks them in `(x, y)`
  order.

The second is `hauling::deposit`'s rule and must go through the same ceiling:
never past `capacity`, because an over-capacity write would make that field a
suggestion and a full Depot is a decided failure mode rather than an
exception to one. A basket that has gone briefly optimistic — a hauler
arrived, an assembler pulled, a raid landed between the screen opening and
Enter — hands over what fits and says so.

Reporting what landed rather than what was asked for is `apply_damage`'s
rule: a log line printing the requested figure claims the base received goods
it never did.

Units leave the pack through `Inventory::take`, never a second decrement —
the same reason `hauling::take_from` is collect's only taking door. It already
clamps to what is held and drops a slot that reaches zero rather than leaving
an `(item, 0)` behind, so the pack-side clamp above is `take`'s own return
value and not a second check in front of it.

One tick and one log line for the whole basket, because one commit is one
action. `MessageKind::Loot` is wrong here (nothing was looted); the line is
base news, so `log_base` with the default kind. An empty or all-zero request
is a no-op: nothing moved, nothing said, no turn spent. It does not speak the
"nothing to put away" sentence either — that belongs to `deposit_adjacent`,
stated once.

### `Game::deposit_adjacent(&mut self) -> Vec<(ItemId, u32)>`

Everything the pack is offering, through `deposit_items`. Sequenced into a
local first, since `deposit_items(&self.depositable())` borrows `self` both
ways at once.

**The refusals live here and nowhere else.** Two of them, because they leave
the player different errands:

- no adjacent Depot — *"There is nowhere here to put anything."*
- a Depot but nothing depositable — *"You have nothing to put away."*

A third state, a Depot with no room, is not a refusal: `deposit_items`
clamps to zero and reports it, and the log line already says nothing landed.

Like `collect_adjacent`, this is only ever *reached* from the key handler
when the offer is empty — the picker takes every other case. It exists so the
sentences have one home rather than a copy in app-core, where a copy of an
engine message reads as the key doing nothing.

The guards come first and refuse *silently*, as they always have: an action
taken during a battle or from the surface is not the base telling you its
shelves are full.

## The one genuinely new problem: the budget is shared

Collect gives every row an independent ceiling — what that item is sitting on
the shelf. A Depot has **one** `output_room()` across every row, so filling
one row lowers every other row's ceiling. This is the only place the mirror
does not hold, and it needs deciding at both ends.

**In the picker**, a row's available is
`min(own qty, room - sum of every other row)`.

That keeps the property `handle_collect_key` deliberately has and says so in
its doc comment: *a number that cannot exceed what is on the shelf is worth
having by construction rather than by a check at the commit*. Every key that
raises a number — the digits, Left, ShiftLeft, CtrlLeft — passes through
`edit_row` and so through this one expression; no key needs its own rule.

**In the engine**, clamp anyway. Nothing ticks while a menu is open, so the
snapshot cannot go stale from the base's own systems — but `deposit_items` is
`pub` and the picker is not its only possible caller, and the clamp is what
lets the function state its contract without reference to who called it.

## app-core

### `Mode::Deposit`

A mirror of `Mode::Collect`: `App::deposit_rows: Vec<(ItemId, u32)>` snapshot
on open, `App::deposit_basket: Vec<u32>` written in the same breath so the two
lengths cannot disagree, `App::deposit_room: u32` snapshotted with them.

Snapshotted rather than re-derived per keypress, for `collect_rows`' reason:
the basket is pending state *indexed into* the row list, so re-deriving opens
a gap where the two lengths disagree. Nothing ticks while a menu is open, so
the snapshot cannot go stale — the commit is the first tick.

`Mode::Deposit` joins `Mode::Collect` in the map-mode list in `lib.rs`, with
the same note: opened from the map only, so it never layers over a fight, and
the engine refuses a deposit mid-battle anyway.

### The extraction

`handle_collect_key` is about sixty lines of deliberately subtle key
semantics — the inverted Left/Right that is *specified* to be inverted, the
`div_ceil` that is what makes the Ctrl step terminate, the saturating digit
accumulation that lets a held key reach the clamp rather than overflow. A
Deposit handler is the same table with one substitution.

Two copies of that will drift, and the inverted Left/Right is precisely the
thing someone "restores" in one of them — the existing doc comment says as
much, naming the test that catches it. Only one copy would be under that test.

So the two pickers share one key table, and the **only** thing that differs is
how a row's available is computed: Collect hands back the row's own quantity,
Deposit hands back the shared-budget expression above. Everything else — the
cursor, `A`/`N`, the digits, the four modifier verbs, Esc, Enter — is stated
once.

The exact shape of that seam is deliberately left to the implementation: it is
a second variant of an existing thing, so the `design-patterns` dialog runs on
it before the code is written. What the dialog may **not** conclude is that
two copies are fine.

### Keys

`P` on the map, in the same surface-only block as `c`, for the same reason
that block exists: a base's buffers are something you walk up to, and the
engine refuses it underground anyway.

Uppercase because every mnemonic lowercase letter is taken — `p` is the party
menu, `d` demolish, `s` save — and the four free lowercase letters
(`n`, `w`, `y`, `z`) name nothing. A shift-slip from `p` opens the party menu,
which costs an Esc.

The handler mirrors `c` exactly: ask `depositable()`, and if it is empty call
`deposit_adjacent()` straight back through the engine so the engine speaks its
own refusal and spends no turn; otherwise hand the offer out past the
`self.game` borrow the way `refusal` is, because opening a screen is not an
action.

### Commit

Zip rows against basket, drop the zeroes, hand the rest to `deposit_items`.
An all-zero basket never reaches the engine — `deposit_items` already makes
that a no-op, so calling through would be harmless today, but then two places
would both have to keep the no-op true.

No `status_line`: the engine has logged it, and the log pane is where the base
reports what it received.

One teardown both exits use, clearing all three fields, which is what stops a
reopened screen showing a stale pack.

## gui

`crates/gui/src/render/deposit.rs`, mirroring `render/collect.rs`.

Same shape: no shortcut lead, because a digit here is a quantity and a menu
that advertises `[1]` for a row while `1` sets an amount is a menu lying about
its own keys. The figures ride the row's **suffix column** rather than being
`format!`ed into the name, for the reason six screens got wrong with the
category tag: measuring a row without its column makes `suffix_x` drop the
suffix onto the row's own tail, and a wrap then budgets for a row narrower
than it draws.

Two differences from collect:

- A header line for remaining room, since the ceiling is shared and otherwise
  invisible: the player needs to see *why* a row stopped rising.
- The suffix reads `given / available`, where available is the live
  shared-budget figure and so moves as other rows fill. That movement is the
  feature — it is what shows the budget being spent.

The hint lines say which arrow does which, Shift and Ctrl included, exactly as
collect's do: a player who guesses from the rest of the UI guesses wrong, and
a modifier is invisible until named. They stay no wider than the `[A]`/`[N]`
line.

The page needs a **width** census and no height one: the rows are `Row::Item`
spans, so `popup_layout` keeps the selected row visible and a long pack
scrolls. `draw_row` clips vertically only, which is what makes width the axis
that fails in silence.

Drawing goes through `Painter` and takes its origin from the caller's `Rect`.

## Testing

Engine, in `crates/engine/src/tests/deposit.rs`:

- The offer excludes banked items, and excludes `GearCopies` entirely.
- The offer is empty with no adjacent Depot, and non-empty with one — pinned
  against an adjacent *machine* with a `Stock`, which must not accept.
- Deposited goods land in `output` and are visible to `base_holding` and to
  `collectable_adjacent` — the point of the feature, asserted end to end.
- An over-ask against the pack is clamped to what is held.
- An over-ask against room is clamped to what fits, and `capacity` is never
  exceeded.
- Two adjacent Depots fill in `(x, y)` order, with the fixture spawning them
  in the *opposite* order to their positions — `assembler_system`'s test's
  trick, and the only way the sort is actually load-bearing.
- One tick per commit; an empty or all-zero basket spends none.
- The two refusal sentences, each from its own state.
- Refused silently mid-battle, on the surface, and underground.

app-core, in `crates/app-core/src/tests/deposit.rs`:

- The shared budget: filling row 1 lowers row 2's ceiling, and no combination
  of keys can push the basket past `deposit_room`.
- Left adds and Right removes, saturating at both ends.
- ShiftLeft is idempotent under key repeat; CtrlLeft closes half the gap and
  terminates on a gap of one.
- A modified arrow reaching any other mode folds back to a bare arrow —
  `App::handle_key`'s existing condition needs `Mode::Deposit` in it, or the
  modified arrows are dead keys nothing catches.
- The commit clears all three fields.

gui: a width census over the deposit rows, in the file the collect one lives
in.

Every test mutation-proved: delete the fix, watch it fail, restore. The four
that matter most are the shared-room clamp, the banked exclusion, the `(x, y)`
fill order, and the full-Depot ceiling — each is a case where a green suite
would otherwise be measuring nothing.

`cargo test --workspace` is the gate, plus `cargo clippy --workspace` and
`cargo fmt`.

## Documentation

- `assets/help/20-controls.md` — the `P` key beside `c`.
- `assets/structures/depot.ron` — its description reads *"Collect from it
  with c."* and now states both halves.
- `CHANGELOG.md` at the merge, with the version bump and tag.
- CLAUDE.md's collect seam entry becomes a collect-and-deposit one, and
  `docs/seams.md` takes the argument: the shared budget is the trap worth
  writing down, because it is the one place the mirror does not hold.
- Not `docs/manual.md` and not the root `README.md`, both carved out.

No schema change and no save-format change: `Stock` and `Inventory` are
untouched, and nothing new is persisted.

## Rejected

**Mirroring collect exactly and accepting any adjacent `Stock`.** Simplest
code, and wrong: it makes a machine's output a place the player stuffs things,
which is the reading the base's whole directionality depends on not being
true.

**A third buffer on `Stock` for a pure stash.** Considered and dropped once
the purpose question was settled. It would keep deposited goods away from the
base's own systems, which is the opposite of what this is for, and every
reader of `output` would grow a second case.

**Gear copies.** Would need `Stock` to key by `GearCopy` or a parallel
per-Depot ledger, plus a save-format change, and would put a rarity rule into
the production chain for the first time. A separate feature if it is ever
wanted.

**A Take/Put toggle inside `Mode::Collect`.** One key and one screen, but the
mode then branches on a direction flag through every handler and the whole
renderer, and the two directions genuinely differ — a shared budget on one
side, independent ceilings on the other. Two modes sharing a key table is the
smaller seam than one mode carrying an axis.

**Reusing `c` with a Take/Put chooser.** An extra keypress on a key pressed
every few turns while walking, which is the tax `c`, `t` and `a` are flat to
avoid.
