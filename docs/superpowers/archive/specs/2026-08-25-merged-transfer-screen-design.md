# Merged transfer screen

**Status:** implemented. Audited 2026-09-02 against the source tree, not against this header. See `../../INDEX.md`.

`c` and `P` become one screen. A row is an item, its amount is signed, and
the sign is the direction: negative puts into an adjacent Depot, positive
takes off an adjacent `Stock`.

## Why

The two pickers already share `app/basket.rs` — the cursor, `[A]`/`[N]`,
the digits and all four modifier verbs are one key table. What they do not
share is the direction, and that split is what the player pays for: at a
Depot holding 190 Core Fragment with 302 in the pack, rebalancing means two
screens, two commits and two ticks, with no single place that shows both
numbers.

It also closes the report this came from. Both Depots in the reporting
save sit at exactly 200/200, so `deposit_room()` is 0, `P` opens a picker
whose every row reads `0 / 0`, and the only thing on screen saying why is
one header line. On the merged screen that state is a row reading
`-0 .. +190`: the put end is visibly pinned while the take end is not, in
the same row, and the shelf being full is the reason you would be taking
rather than putting anyway.

## Decisions

Settled in the brainstorm, recorded so they are not relitigated:

- **One signed row per item**, not two lists and not two tabs. An item that
  is both on the shelf and in the pack is one row and one net decision.
- **Left puts in, Right takes out.** The row prints its own range with the
  put end on the left, so `Shift`+`Left` is "all the way to the Depot" and
  `Shift`+`Right` is "all the way to me". This *inverts collect*, which is
  the more-used of the two screens — accepted knowingly, because the
  alternative runs the arrows backwards against the range the row draws.
  The game's standing Left-adds/Right-removes inversion does not survive
  the merge in either mapping: it means "more of this transaction", and
  there is now one axis rather than two transactions.
- **Digits follow the row's current sign, and a row at zero types a take.**
  No new key. Typing an exact put quantity costs one arrow press to cross
  zero first.
- **`[A]` takes everything, `[N]` clears.** There is deliberately no
  put-all verb: `Shift`+`Left` per row does it, and a third fill verb buys
  one keypress at the cost of a fourth hint line on a screen that already
  has three.
- **No separate full-shelves notification.** The `Depot room remaining`
  header and the pinned put end carry it. This was asked for and then
  withdrawn once the merged screen subsumed it; a log line on the base
  filling up is a separate feature if it is ever wanted.

## The screen

One `Mode::Transfer`, replacing `Mode::Collect` and `Mode::Deposit`. Title
"Transfer". Opened by `c`; `P` leaves the map key table.

Body, in order:

```
Depot room remaining: 12                  <- only when a Depot is adjacent
Up/Down pick a row; digits and Backspace type an amount
Left puts in, Right takes out; Shift for the end, Ctrl halves the gap
[A] take everything  [N] clear  Enter to transfer  Esc to leave

Blank Substrate           +0     -0 .. +8
Core Fragment          +190     -302 .. +190
Credits                   +0     -12 .. +0
```

The room line is **omitted entirely** when no adjacent `Stock` has
`stores` — a Mining Node has no room to report, and a line reading 0 there
would claim the base is full when it has no shelf at all. That is why
`App::basket_room` stays an `Option<u32>` rather than collapsing to a plain
number now that there is one screen: `None` is "no Depot beside you" and
`Some(0)` is "a Depot with nothing left", and those are the two states the
report this came from could not tell apart.

The suffix column is one string: the signed amount, then the row's **live**
availables. Live, not the static shelf and pack figures, because the put
end is a shared budget — a row reading `-0` while the pack still holds
units is the screen saying the other rows have spent the room, which is
what the deposit screen already draws today.

## Rows

`Game::transfer_offer() -> Vec<TransferRow>`, `&self`, no tick, no log, no
RNG. `TransferRow { item: ItemId, on_shelves: u32, in_pack: u32 }`, in
`views.rs` with the other view types.

The union of the two existing offers, sorted by `ItemId` — the order
`Stock`'s own `BTreeMap` yields, which both `collectable_adjacent` and
`depositable` already produce:

- `on_shelves`: pooled non-zero `output` across `adjacent_stock()`.
- `in_pack`: the player's `Inventory` row, if the item is not
  `ItemDef::banked`, and 0 when no adjacent `Stock` has `stores`.

Both existing filters survive unchanged and for their existing reasons: a
bank is not cargo, and a `Stock` without `stores` must not be stuffed with
the player's pack the way a Mining Node's output would be.

Guards are today's, in today's order, answering with an empty offer for
each: game over, an active battle, `require_base` failing. Like
`collectable_adjacent` it needs no `require_surface` — `require_base` is
the stronger statement.

## Ceilings

Two functions, because the two ceilings are genuinely different shapes and
collapsing them is what would drift:

- `take_available(row)` is `on_shelves`. Per row, independent.
- `put_available(row)` is `basket_room.unwrap_or(0)` less what **the other**
  rows are giving, then capped at `in_pack`. Shared across every row, and 0
  for every row when no Depot is adjacent.

Subtracting only the other rows is `basket_available`'s existing rule and
is what lets the highlighted row be lowered and raised while it is being
edited; counting itself makes every key a no-op the moment the basket
reaches the budget.

The put budget deliberately does **not** count pending takes as freeing
room. A take may come off a machine that is not a Depot, so crediting it
would over-offer; under-offering is safe and the commit order below means
it is never the binding constraint.

## Keys

`app/basket.rs` keeps its one key table. `basket_amounts` becomes
`Vec<i64>`; each entry is clamped to `[-put_available(row), take_available(row)]`.

| Key | Effect |
| --- | --- |
| `Left` | one step toward the put end |
| `Right` | one step toward the take end |
| `Shift`+`Left` | `-put_available(row)` |
| `Shift`+`Right` | `+take_available(row)` |
| `Ctrl`+`Left` | half the remaining gap to `-put_available(row)` |
| `Ctrl`+`Right` | half the remaining gap to `+take_available(row)` |
| digit | `n*10 + d` in the row's current sign; a row at zero types positive |
| `Backspace` | magnitude `/10`, sign kept |
| `[A]` | every row to `+take_available` |
| `[N]` | every row to zero |
| `Enter` | commit, then leave |
| `Esc` | leave |

`Enter` closes the screen whether or not the basket moved anything, which
is what both screens do today. An all-zero basket never reaches the engine:
`transfer_items` already makes that a no-op, but two places both keeping
that true is how a no-op stops being one.

Shift is a target and Ctrl a step toward that same target — the existing
distinction, generalised so each modifier pair points at the end its
unmodified arrow heads for. The Ctrl step is `div_ceil` on the **magnitude**
of the gap, which is what makes it terminate: rounded down, a gap of one
gives a step of zero and the key goes dead with the row neither full nor
empty.

`[A]` fills row by row through the same available call rather than zipping
straight across, because under the shared budget each row's ceiling depends
on what the rows before it took. `[A]` has no bearing on the put side, so it
cannot blow the budget — but it must not silently clear a row the player
had set to give, either: it writes the take ceiling over every row, giving
and all, which is what "take everything" says.

## Commit

`Game::transfer_items(take: &[(ItemId, u32)], give: &[(ItemId, u32)])`, one
new door. **Take first, then give**, so a rebalance that empties a shelf
before filling it is not refused for want of room.

One tick and one pair of log lines for the whole basket, because one commit
is one action — the existing `Loot` line for what came and the existing
base line for what went. An empty or all-zero basket is a silent no-op:
nothing moved, nothing said, no turn spent.

`collect_items` and `deposit_items` each split into a pure mover — no tick,
no log, returns what actually moved — plus the public wrapper that logs and
ticks. `transfer_items` calls both movers. A **call, not a copy**: two
independent copies of the clamping is exactly the drift the one-taking-path
and one-giving-path rules exist to prevent.

Both clamps stay where they are and neither becomes a refusal: an over-ask
is clamped because buffers can shrink between a screen opening and the
commit, and the room is checked before anything leaves the pack, because
taking from the pack first and clamping the write after lets a full base eat
cargo the player never gets back.

## Refusals

`Game::refuse_transfer(&mut self)`, called by app-core when
`transfer_offer()` comes back empty — the same shape as `c` today, where
app-core routes its empty case back through the engine rather than growing a
second copy of a sentence.

Two sentences, because they leave the player different errands:

- No adjacent `Stock` at all — "There is nothing here to take from or put
  into."
- Adjacent `Stock`, but every shelf empty and nothing puttable in the pack —
  "There is nothing to move here."

The three sentences being replaced ("There is nothing to collect here.",
"There is nowhere here to put anything.", "You have nothing to put away.")
all go. The guards refuse **silently**, as they always have: an action taken
during a battle or from the surface is not the base telling you its shelves
are bare.

## Files

**engine**
- `game/base/transfer.rs` — new. `transfer_offer`, `refuse_transfer`,
  `transfer_items`.
- `game/base/collect.rs` — `collect_items` splits into mover + wrapper.
- `game/base/deposit.rs` — `deposit_items` splits into mover + wrapper.
  `depositable`, `deposit_room` and `adjacent_depots` stay.
- `views.rs` — `TransferRow`.

**app-core**
- `Mode::Collect` + `Mode::Deposit` → `Mode::Transfer`.
- `app/basket.rs` — signed amounts, two available functions, the key table
  above.
- `app/collect.rs` + `app/deposit.rs` → `app/transfer.rs`.
- `app/playing.rs` — the `c` arm rewritten, the `P` arm deleted.
- `app/input.rs` — the modifier fold names `Mode::Transfer`.
- `lib.rs` — `basket_rows: Vec<TransferRow>`, `basket_amounts: Vec<i64>`,
  `basket_room: Option<u32>` (unchanged in type, changed in meaning).
- `tests/collect.rs` + `tests/deposit.rs` → `tests/transfer.rs`.

**gui**
- `render/collect.rs` + `render/deposit.rs` → `render/transfer.rs`.
- `render/mod.rs` — the wiring and the all-`Mode`s refusal census.

**assets**
- `structures/depot.ron` — the description names one key.
- `help/20-controls.md` — two key lines become one.

No save-format change: nothing here is stored.

## Tests

Intent, not code. The engine ones go beside the existing collect and
deposit suites; the app-core ones replace both test files.

- `transfer_offer` unions the two sides and an item on both is **one** row
  carrying both figures.
- A banked item never gets a put end; an item on a non-`stores` `Stock`
  never gets one either.
- `in_pack` is 0 for every row when no adjacent `Stock` has `stores`, and
  the room header is absent rather than reading 0 — the two states a full
  Depot and no Depot must not collapse into.
- The put budget is shared: filling one row lowers `put_available` on the
  others, and the highlighted row keeps its own amount while being edited.
- **A full Depot** — capacity exactly reached — leaves every put end at 0
  while the take ends are untouched. This is the reported case and there is
  no test at 200/200 today, only at 195/200.
- `Ctrl` terminates at both ends: a gap of one closes rather than stranding.
- Digits type into the row's current sign, and into a take at zero.
- A commit with both halves ticks **once** and emits both log lines.
- Take runs before give: a basket that empties a full Depot and refills it
  from the pack lands both halves.
- The two refusals fire on their own cases and the guards stay silent.
- gui: `no_transfer_row_overflows_its_popup`, the width census, with
  `u32::MAX` at both ends of the range — nothing bounds a modded Depot's
  capacity and `draw_row` clips vertically only.
- gui: the all-`Mode`s refusal census still counts exactly one refusal per
  screen with `Mode::Transfer` in place of the two.

## Out of scope

- A log line when the base's last Depot fills up.
- Any change to what haulers, `hauling::deposit` or the work-order
  scheduler do with a full Depot.
- Depot capacity, `max_deployed`, or an upgrade path for the Depot.
