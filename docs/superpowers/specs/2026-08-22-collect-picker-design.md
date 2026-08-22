# The collect picker

**Status:** approved, not implemented
**Date:** 2026-08-22

`c` next to a base structure currently empties every adjacent output
buffer into the player's pack, wholesale. This replaces that with a
window: one row per collectable item, a quantity per row, and one commit
that takes exactly the basket.

## Why

`Game::collect_adjacent` takes everything, and **there is no
player-facing deposit verb** — nothing in the game puts units back into a
base buffer. `hauling::deposit` exists, but its only callers are the
hauling system's own errands; a player who has collected has no way to
undo it.

So a misfired `c` beside a working line is not a convenience cost, it is
a permanent one: the ingredients a chain was about to pull are now in the
player's pack, and the only route back is to re-produce them. The base's
buffers are shared state that the player can strip but not restore.

Wanting ten Cache Grain for a bench recipe should not mean holding the
whole depot.

## What the player does

`c` opens a popup listing every item on offer within reach, pooled across
all adjacent structures. Every row starts at **zero** — nothing is
selected by default, which is the whole point of the change. The player
moves a cursor down the rows, sets an amount on the ones they want, and
commits. Taking everything is still one keystroke (`[A]`) followed by
Enter.

Abandoning the screen takes nothing and costs no turn, the same way a
collect that finds nothing costs no turn today.

## Engine

All of it in `crates/engine/src/game/base/collect.rs`.

### `adjacent_stock(&self) -> Vec<Entity>`

Private. The four orthogonally touching tiles via the existing
`ORTHOGONAL` constant — unchanged as the one reach rule, shared with
`hauling::touching` and the pull phase — filtered to entities carrying a
`Stock`, **sorted by `(x, y)`**.

The sort is load-bearing and is `assembler_system`'s reason in a second
place: bevy's query iteration order is not stable, and with two
neighbours holding the same item a *partial* take must drain them in the
same order every run. Unsorted, the same keypress leaves a different
buffer non-empty between two runs of an identical save. Taking everything
could not see this, which is why the existing code has no sort.

### `Game::collectable_adjacent(&self) -> Vec<(ItemId, u32)>`

What is on offer, pooled across neighbours, in `ItemId` order — the order
`Stock`'s `BTreeMap` already yields and the order the existing collect
log already prints, so the screen and the log line cannot disagree about
how a haul is ordered.

`&self`. No tick, no log, no RNG. It holds the same four guards
`collect_adjacent` holds today and returns empty for each: game over, an
active battle, `require_base` failing, and `base_pos` returning `None`.
It is a claim about what is beside the party, so like `base_stock` it
needs no `require_surface` of its own — `require_base` is the stronger
statement.

### `Game::collect_items(&mut self, want: &[(ItemId, u32)]) -> Vec<(ItemId, u32)>`

The one taking path. For each requested `(item, qty)`, walks the sorted
neighbours drawing through `hauling::take_from` until the request is met
or the buffers are dry, adds what came to `Inventory`, and reports what
actually landed.

`take_from` rather than a second remove-or-decrement: its doc comment
already names itself "the one way units leave a buffer by hand", and a
second copy is how a buffer ends up holding a zero entry that every
reader then has to know to skip. Today's `collect_adjacent` bypasses it
with a bare `output.remove(&item)`, which is correct only because it
always takes the whole entry. **This makes collect its fourth caller**
and the bypass goes.

An over-ask is **clamped, not refused** — the buffers can only shrink
between the screen opening and the commit (a raid, a hauler, an
assembler pull), and a basket that becomes briefly optimistic should
hand over what is there rather than fail.

Ticks **once** for the whole basket, and logs one `MessageKind::Loot`
line summarising it, built the way the current one is. An empty or
all-zero request takes nothing, logs nothing, and **costs no turn** — it
is a refusal, and a refusal has never spent a tick here.

Returning what actually landed rather than what was asked for is
`apply_damage`'s rule: a log line printing the requested figure claims
goods the player never received.

### `Game::collect_adjacent(&mut self) -> Vec<(ItemId, u32)>`

Kept, and reduced to `self.collect_items(&self.collectable_adjacent())`.

Take-all becomes literally "select everything, then commit", so there is
one taking path rather than two that could drift on clamping, logging or
the tick. Its existing behaviour is preserved exactly, including the
"There is nothing to collect here." refusal, which stays stated in this
one place — app-core must not grow a second copy of that sentence.

## app-core

### `Mode::Collect`

The `c` arm of `handle_playing_key` asks `collectable_adjacent()`. If it
is empty it calls `collect_adjacent()` unchanged, so the refusal keeps
its own words and costs no turn, and the existing test at
`crates/app-core/src/tests/playing.rs:106` keeps its meaning. Otherwise
it snapshots the rows and opens the screen.

Two new `App` fields, written together at open time so they cannot drift
apart:

```rust
/// What is on offer, snapshotted when `Mode::Collect` opens.
pub collect_rows: Vec<(ItemId, u32)>,
/// How much of each row the player has asked for. Indexed by
/// `collect_rows`; all zeroes on open.
pub collect_basket: Vec<u32>,
```

Snapshotted rather than re-derived per keypress, which is the opposite of
what the trade screen does. The reason is that the basket is pending
state indexed into the row list: re-deriving opens a gap where the two
lengths disagree. Nothing ticks while a menu is open, so the snapshot
cannot go stale — the commit is the first tick.

### Keys

**This screen cannot use `App::selected_index`.** There, digits pick a
row; here digits are a quantity. The cursor therefore moves on Up/Down
only, and it drives the same `App::menu_selected` field, so
`popup_layout`'s window follows it and the screen scrolls for free.

| Key | Effect |
|-----|--------|
| Up / Down | move the row cursor |
| digit | append to the highlighted row's amount, clamped to that row's available |
| Backspace | drop the last digit of the highlighted row's amount |
| Left / Right | −1 / +1 on the highlighted row |
| `[A]` | fill every row to its maximum |
| `[N]` | clear the basket |
| Enter | commit the basket — one action, one turn |
| Esc | leave, take nothing, no turn |

Clamping happens as the digit is typed rather than at commit: typing
`50` against 12 available leaves the row reading 12. The alternative —
ignoring a digit that would overflow — is silent, and a row that stops
responding to a keypress reads as the screen being broken.

Amounts are held as `u32` and edited numerically (`n * 10 + d`, then
clamp) rather than as the `String` buffer the craft and trade quantity
pages use. Those pages have no ceiling to clamp against; this one does,
and a number that cannot exceed what is on the shelf is worth having by
construction rather than by a check at commit.

Uppercase for the two screen actions, matching the reserved-uppercase
convention (`[M]`, `[C]`, `[S]`, `[B]`). Nothing on this screen picks a
row by letter, so the reservation costs nothing here, but the convention
is what makes uppercase readable as "acts" across every screen.

Enter with an all-zero basket is the same no-op as Esc, and returns to
`Mode::Playing` without a tick.

A new `Mode` variant lands in the dispatch matches on both sides —
`App::handle_key`, `close_screen`, and `render/mod.rs`'s draw match. Those
are exhaustive, so the compiler names every site; nothing here is a
`_ =>` arm that could ship the screen unreachable.

### Commit

Enter calls `game.collect_items(&pairs)` where `pairs` zips
`collect_rows` with `collect_basket`, dropping zeroes. Both fields are
cleared and the mode returns to `Playing`. The engine has already
logged, so no `status_line` is set — the log pane is where a haul is
reported today and stays so.

## gui

`crates/gui/src/render/collect.rs`, a new popup drawn like the other list
screens. One row per item: the item name, and a suffix column reading
`taken / available`.

The page **scrolls** — the cursor drives `menu_selected` and
`popup_layout` keeps the selected row visible — so unlike the memories
and gear-inspect pages it needs no height census. It does need a width
one: `draw_row` clips vertically only, so an over-wide row is drawn off
the panel in silence.

The suffix goes through the row's own suffix column rather than being
`format!`ed into the name, for the reason the category tag was made a
column: measuring a row without the column makes `suffix_x` drop its
suffix on the row's own tail, and a wrap then budgets for a row narrower
than it draws.

## Testing

Every new test gets the delete-the-fix-and-watch-it-fail treatment.

**Engine**

- `collectable_adjacent` pools two neighbours holding the same item into
  one row, in `ItemId` order.
- It returns empty for each of the four guards — in a battle, off-base,
  underground, game over — and takes no tick doing it.
- `collect_items` takes exactly what is asked and leaves the remainder in
  the buffer.
- An over-ask is clamped to what is there.
- A partial take spanning two structures drains them in `(x, y)` order,
  leaving the *later* tile holding the remainder. This is the test the
  sort exists for; without it the assertion is a coin flip.
- A commit ticks exactly once and logs exactly one line, whatever the
  basket's size.
- An empty request takes nothing, logs nothing and does not tick.
- `collect_adjacent`'s existing tests in `crates/engine/src/tests/collect.rs`
  and `hauling.rs` stay green untouched — that is the evidence the
  wrapper preserves behaviour.

**app-core**

- `c` beside a stocked structure opens `Mode::Collect` with a row per
  item and an all-zero basket.
- `c` with nothing adjacent still refuses and costs no turn
  (`tests/playing.rs:106`, unchanged).
- A digit sets the highlighted row; a digit that would overflow clamps to
  available.
- Backspace drops a digit; Left/Right step by one and cannot go below
  zero or above available.
- `[A]` fills every row; `[N]` clears every row.
- Enter commits exactly the basket and returns to `Playing`.
- Enter on an all-zero basket and Esc both take nothing and spend no
  turn.

**gui**

- `no_collect_row_overflows_its_popup`, measured through
  `paint::with_painter` against the widest shipped item name.

Full `cargo test --workspace` is the gate, plus
`cargo test -p feral-processes-engine balance_sim` — this touches no
tuning constant, so a moved curve would mean something unintended.

## Documentation

- `CHANGELOG.md` — its own `## X.Y.Z` section at the merge. Minor, not
  patch: it is a feature and no save stops loading.
- `assets/help/20-controls.md:25` and `assets/help/60-your-base.md:54`
  both name `c` as taking from everything adjacent, and both become
  false.
- `docs/seams.md` — a new entry under **The base** for the collect seam,
  with its one-line summary in `CLAUDE.md` beside the `ORTHOGONAL` line
  that is already there.

Not `docs/manual.md`, not the root `README.md`, not `TODO.md`.

## Rejected

- **A per-row quantity page** (pick a row, type an amount, land back on
  the list), matching the craft and trade idiom. Fewer new concepts, but
  it costs a screen round trip per item and the whole complaint is about
  a base visit being tedious.
- **Grouping the list by structure.** More honest about the base's
  layout, but a longer screen, and section-offset row arithmetic is the
  exact bug class `trade_row` exists to prevent.
- **Ticking once per visit rather than once per commit.** Rejected as a
  new rule nothing else follows; with a basket, one commit already *is*
  one action, so per-commit and per-visit only differ if the player
  commits twice, and then two actions should cost two turns.
- **Leaving `c` as take-all and binding the picker elsewhere.** Costs a
  second map binding to preserve the destructive default.
